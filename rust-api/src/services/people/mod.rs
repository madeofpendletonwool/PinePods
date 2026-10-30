//! People credited on podcasts and episodes: one shared identity per person (`Persons`), credits
//! from the feed's `<podcast:person>` tags (written at refresh) and from PodPeopleDB (written by
//! the background sync), and per-user follows. Pages read everything from the database; nothing
//! here fetches a feed or calls PodPeopleDB on a page load.

pub mod external;
pub mod podpeople;
pub mod sql;
pub mod store;

use serde::{Deserialize, Serialize};

/// Where PodPeopleDB and the PinePods search API (Podcast Index) live, from the server config.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub people: String,
    pub search: String,
}

impl Endpoints {
    pub fn from_config(config: &crate::config::Config) -> Self {
        Endpoints {
            people: config.api.people_api_url.trim_end_matches('/').to_string(),
            search: config.api.search_api_url.clone(),
        }
    }
}

/// Credit sources, stored in `PodcastPeople.Source` / `EpisodePeople.Source`.
pub const SOURCE_FEED: &str = "feed";
pub const SOURCE_PODPEOPLE: &str = "podpeople";

/// A person as shown on a podcast or episode page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CreditedPerson {
    /// PinePods person id (`Persons.PersonID`); use it for the person page and follows.
    pub id: i32,
    pub podpeople_id: Option<i32>,
    pub name: String,
    pub img: Option<String>,
    pub href: Option<String>,
    /// Most significant role ("host", "co-host", "guest", ...).
    pub role: String,
    /// Every role this person has here, most significant first.
    pub roles: Vec<String>,
    pub group: Option<String>,
    /// "podcast": credited on the show itself or a regular across its episodes.
    /// "episodes": credited only on some episodes. "episode": credited on this episode.
    pub scope: String,
    /// Episodes of this podcast the person is credited on.
    pub episode_count: i64,
    /// "feed" and/or "podpeople".
    pub sources: Vec<String>,
    /// False when every credit is an unreviewed community submission.
    pub verified: bool,
    pub followed: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PodcastCredits {
    /// Hosts, co-hosts and anyone else credited on the show or appearing as a regular.
    pub people: Vec<CreditedPerson>,
    /// Guests and other episode-only credits, most frequent first.
    pub guests: Vec<CreditedPerson>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Person {
    pub id: i32,
    pub podpeople_id: Option<i32>,
    pub name: String,
    pub img: Option<String>,
    pub href: Option<String>,
    pub followed: bool,
}

/// A podcast a person is credited on.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonPodcast {
    /// The user's podcast id, or None for a show outside their library.
    pub podcastid: Option<i32>,
    pub podcastname: String,
    pub artworkurl: Option<String>,
    pub feedurl: String,
    pub podcastindexid: Option<i64>,
    pub roles: Vec<String>,
    pub podcast_level: bool,
    pub episode_count: i64,
}

/// An episode from a show outside the user's library.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ExternalEpisode {
    pub episodeid: i32,
    pub podcastid: i32,
    pub podcastname: String,
    pub feedurl: String,
    pub podcastindexid: Option<i64>,
    pub episodetitle: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub completed: bool,
    pub saved: bool,
    pub queued: bool,
    pub downloaded: bool,
    pub is_youtube: bool,
    pub is_video: bool,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonProfile {
    pub person: Person,
    pub podcasts: Vec<PersonPodcast>,
    /// Library episodes the person is credited on (directly, or as a podcast's host), newest first.
    pub episodes: Vec<crate::handlers::podcasts::Episode>,
    /// Recent episodes from shows outside the library.
    pub external_episodes: Vec<ExternalEpisode>,
    /// True while the external episode list is being refreshed in the background.
    pub refreshing: bool,
}

/// Role order for display: host, co-host, guest, then everything else alphabetically.
pub fn role_rank(role: &str) -> (u8, String) {
    let rank = match role {
        "host" => 0,
        "co-host" => 1,
        "guest" => 2,
        _ => 3,
    };
    (rank, role.to_string())
}

pub fn is_host_role(role: &str) -> bool {
    role == "host" || role == "co-host"
}

/// One-time work after upgrading: remove the placeholder podcasts the old host feeds created
/// under the system user, and store Podcasting 2.0 tags for podcasts that have none yet (the
/// regular refresh skips unchanged feeds, so it would never fill them in). Then a first
/// PodPeopleDB sync.
pub async fn startup(db: crate::database::DatabasePool, api: Endpoints) {
    use futures::StreamExt;
    use sql::fetch_all;

    let placeholders: crate::error::AppResult<Vec<i32>> = async {
        Ok(fetch_all!(&db, r#"SELECT podcastid AS podcastid FROM "Podcasts" WHERE userid = 1"#, [], |r| r.int("podcastid")? as i32))
    }
    .await;
    match placeholders {
        Ok(ids) if !ids.is_empty() => {
            let mut removed = 0;
            for id in &ids {
                match db.remove_podcast_id(*id, 1, crate::services::sync::SyncOrigin::Unlogged).await {
                    Ok(()) => removed += 1,
                    Err(e) => tracing::warn!("Removing old host-feed podcast {} failed: {}", id, e),
                }
            }
            tracing::info!("Removed {} placeholder podcasts left by the old host feeds", removed);
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("Looking up old host-feed podcasts failed: {}", e),
    }

    match store::podcasts_missing_tags(&db).await {
        Ok(missing) if !missing.is_empty() => {
            tracing::info!("Storing Podcasting 2.0 tags for {} podcasts", missing.len());
            futures::stream::iter(missing)
                .for_each_concurrent(4, |(podcast_id, feed_url, username, password)| {
                    let db = db.clone();
                    async move {
                        match db.try_fetch_feed(&feed_url, username.as_deref(), password.as_deref()).await {
                            Ok(body) => db.store_feed_tags(&[podcast_id], &body).await,
                            Err(e) => tracing::debug!("Backfilling tags for podcast {} failed: {}", podcast_id, e),
                        }
                    }
                })
                .await;
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("Looking up podcasts without stored tags failed: {}", e),
    }

    if podpeople::sync_enabled() {
        if let Err(e) = podpeople::sync(&db, &api, None).await {
            tracing::warn!("Startup PodPeopleDB sync failed: {}", e);
        }
    }

    // Podcasts added since: synced shortly after they're added, wherever they were added from.
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        podpeople::sync_pending(&db, &api).await;
    }
}
