//! Episodes a person appears in on shows outside the user's library, cached per person in
//! `PersonExternalEpisodes` from PodPeopleDB credits and the Podcast Index person search.
//! Refreshed in the background: when a stale person page is opened, and on a schedule for
//! followed people.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::Deserialize;

use super::sql::{execute, fetch_all, fetch_optional};
use super::{Endpoints, ExternalEpisode, PersonPodcast};
use crate::database::DatabasePool;
use crate::error::AppResult;

/// How long a person's external episode list is considered fresh.
pub const TTL_HOURS: i64 = 12;
const MAX_EPISODES: usize = 200;

static IN_FLIGHT: Mutex<Option<HashSet<i32>>> = Mutex::new(None);

struct Found {
    feed_url: String,
    podcast_title: Option<String>,
    podcast_artwork: Option<String>,
    podcastindex_id: Option<i64>,
    guid: Option<String>,
    title: String,
    description: Option<String>,
    url: String,
    artwork: Option<String>,
    pub_date: Option<chrono::NaiveDateTime>,
    duration: Option<i64>,
    role: Option<String>,
    source: &'static str,
}

async fn from_podpeople(http: &reqwest::Client, api: &Endpoints, podpeople_id: i32) -> Vec<Found> {
    #[derive(Deserialize)]
    struct Detail {
        #[serde(default)]
        recent_episodes: Vec<Ep>,
    }
    #[derive(Deserialize)]
    struct Ep {
        guid: Option<String>,
        enclosure_url: Option<String>,
        title: String,
        description: Option<String>,
        image: Option<String>,
        pub_date: Option<String>,
        duration: Option<i64>,
        podcast_title: String,
        podcast_image: Option<String>,
        podcastindex_id: Option<i64>,
        feed_url: Option<String>,
        role: String,
    }
    let url = format!("{}/api/v2/people/{}", api.people, podpeople_id);
    let Ok(resp) = http.get(url).send().await else { return Vec::new() };
    if !resp.status().is_success() {
        return Vec::new();
    }
    let Ok(detail) = resp.json::<Detail>().await else { return Vec::new() };
    detail
        .recent_episodes
        .into_iter()
        .filter_map(|e| {
            Some(Found {
                feed_url: e.feed_url?,
                podcast_title: Some(e.podcast_title),
                podcast_artwork: e.podcast_image,
                podcastindex_id: e.podcastindex_id,
                guid: e.guid,
                title: e.title,
                description: e.description,
                url: e.enclosure_url?,
                artwork: e.image,
                pub_date: e.pub_date.as_deref().and_then(parse_date),
                duration: e.duration,
                role: Some(e.role),
                source: "podpeople",
            })
        })
        .collect()
}

fn parse_date(s: &str) -> Option<chrono::NaiveDateTime> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.naive_utc())
        .ok()
        .or_else(|| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
        .or_else(|| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S").ok())
}

/// Podcast Index episodes tagged with (or mentioning) the person, through the PinePods search API.
async fn from_index(http: &reqwest::Client, api: &Endpoints, name: &str) -> Vec<Found> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Item {
        title: Option<String>,
        description: Option<String>,
        enclosure_url: Option<String>,
        date_published: Option<i64>,
        duration: Option<i64>,
        image: Option<String>,
        feed_image: Option<String>,
        feed_title: Option<String>,
        feed_url: Option<String>,
        feed_id: Option<i64>,
        guid: Option<String>,
    }
    #[derive(Deserialize)]
    struct Resp {
        #[serde(default)]
        items: Vec<Item>,
    }
    if api.search.is_empty() {
        return Vec::new();
    }
    let req = http.get(&api.search).query(&[("query", name), ("index", "person"), ("search_type", "person")]);
    let Ok(resp) = req.send().await else { return Vec::new() };
    let Ok(body) = resp.json::<Resp>().await else { return Vec::new() };
    body.items
        .into_iter()
        .filter_map(|i| {
            Some(Found {
                feed_url: i.feed_url.filter(|u| !u.is_empty())?,
                podcast_title: i.feed_title,
                podcast_artwork: i.feed_image.clone(),
                podcastindex_id: i.feed_id,
                guid: i.guid,
                title: i.title.filter(|t| !t.is_empty())?,
                description: i.description,
                url: i.enclosure_url.filter(|u| !u.is_empty())?,
                artwork: i.image.filter(|s| !s.is_empty()).or(i.feed_image),
                pub_date: i.date_published.and_then(|t| chrono::DateTime::from_timestamp(t, 0)).map(|d| d.naive_utc()),
                duration: i.duration,
                role: None,
                source: "index",
            })
        })
        .collect()
}

fn url_hash(url: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(url.as_bytes()))
}

/// Fetch and store a person's external episodes now.
pub async fn refresh(db: &DatabasePool, api: &Endpoints, person_id: i32) -> AppResult<usize> {
    let Some((name, podpeople_id)) = fetch_optional!(
        db,
        r#"SELECT name AS name, podpeopleid AS podpeopleid FROM "Persons" WHERE personid = $1"#,
        [person_id],
        |r| (r.str("name")?, r.opt_int("podpeopleid")?.map(|v| v as i32))
    ) else {
        return Ok(0);
    };
    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .user_agent(concat!("PinePods/", env!("CARGO_PKG_VERSION")))
        .build()?;
    let (community, index) = tokio::join!(
        async {
            match podpeople_id {
                Some(id) => from_podpeople(&http, api, id).await,
                None => Vec::new(),
            }
        },
        from_index(&http, api, &name)
    );

    // PodPeopleDB credits first: they carry the role.
    let mut seen = HashSet::new();
    let mut found: Vec<Found> = community.into_iter().chain(index).filter(|f| seen.insert(f.url.clone())).collect();
    found.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));
    found.truncate(MAX_EPISODES);

    execute!(db, r#"DELETE FROM "PersonExternalEpisodes" WHERE personid = $1"#, [person_id]);
    for f in &found {
        let cut = |s: &Option<String>, n: usize| s.as_ref().map(|v| v.chars().take(n).collect::<String>());
        let res = match db {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"INSERT INTO "PersonExternalEpisodes"
                   (personid, feedurl, podcasttitle, podcastartwork, podcastindexid, episodeguid, episodetitle,
                    episodedescription, episodeurl, episodeartwork, episodepubdate, episodeduration, role, source)
                   VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                   ON CONFLICT (personid, episodeurl) DO NOTHING"#,
            )
            .bind(person_id)
            .bind(&f.feed_url)
            .bind(&f.podcast_title)
            .bind(&f.podcast_artwork)
            .bind(f.podcastindex_id.and_then(|v| i32::try_from(v).ok()))
            .bind(&f.guid)
            .bind(&f.title)
            .bind(&f.description)
            .bind(&f.url)
            .bind(&f.artwork)
            .bind(f.pub_date)
            .bind(f.duration.and_then(|v| i32::try_from(v).ok()))
            .bind(cut(&f.role, 64))
            .bind(f.source)
            .execute(pool)
            .await
            .map(|_| ()),
            DatabasePool::MySQL(pool) => sqlx::query(
                "INSERT IGNORE INTO PersonExternalEpisodes
                 (PersonID, FeedURL, PodcastTitle, PodcastArtwork, PodcastIndexID, EpisodeGUID, EpisodeTitle,
                  EpisodeDescription, EpisodeURL, EpisodeURLHash, EpisodeArtwork, EpisodePubDate, EpisodeDuration, Role, Source)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(person_id)
            .bind(&f.feed_url)
            .bind(&f.podcast_title)
            .bind(&f.podcast_artwork)
            .bind(f.podcastindex_id.and_then(|v| i32::try_from(v).ok()))
            .bind(&f.guid)
            .bind(&f.title)
            .bind(&f.description)
            .bind(&f.url)
            .bind(url_hash(&f.url))
            .bind(&f.artwork)
            .bind(f.pub_date)
            .bind(f.duration.and_then(|v| i32::try_from(v).ok()))
            .bind(cut(&f.role, 64))
            .bind(f.source)
            .execute(pool)
            .await
            .map(|_| ()),
        };
        if let Err(e) = res {
            tracing::debug!("Skipping external episode {} for person {}: {}", f.url, person_id, e);
        }
    }
    execute!(db, r#"UPDATE "Persons" SET externalrefreshedat = CURRENT_TIMESTAMP WHERE personid = $1"#, [person_id]);
    Ok(found.len())
}

/// True when the person's external episodes were never fetched or are older than the TTL.
pub async fn is_stale(db: &DatabasePool, person_id: i32) -> AppResult<bool> {
    let at = fetch_optional!(
        db,
        r#"SELECT externalrefreshedat AS at FROM "Persons" WHERE personid = $1"#,
        [person_id],
        |r| r.ts("at")?
    )
    .flatten();
    Ok(at.is_none_or(|t| chrono::Utc::now().naive_utc() - t > chrono::Duration::hours(TTL_HOURS)))
}

/// Refresh in the background unless a refresh for this person is already running. Returns
/// true when one is running after the call.
pub fn refresh_soon(db: DatabasePool, api: Endpoints, person_id: i32) -> bool {
    {
        let mut guard = IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
        let set = guard.get_or_insert_with(HashSet::new);
        if !set.insert(person_id) {
            return true;
        }
    }
    tokio::spawn(async move {
        if let Err(e) = refresh(&db, &api, person_id).await {
            tracing::warn!("Refreshing external episodes for person {} failed: {}", person_id, e);
        }
        let mut guard = IN_FLIGHT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(set) = guard.as_mut() {
            set.remove(&person_id);
        }
    });
    true
}

/// Refresh stale lists of everyone someone follows. Run by the scheduler.
pub async fn refresh_followed(db: &DatabasePool, api: &Endpoints) -> AppResult<usize> {
    let cutoff = chrono::Utc::now().naive_utc() - chrono::Duration::hours(TTL_HOURS);
    let ids = fetch_all!(
        db,
        r#"SELECT DISTINCT pe.personid AS personid FROM "Persons" pe JOIN "PersonFollows" f ON f.personid = pe.personid
           WHERE pe.externalrefreshedat IS NULL OR pe.externalrefreshedat < $1"#,
        [cutoff],
        |r| r.int("personid")? as i32
    );
    let mut n = 0;
    for id in ids {
        if refresh(db, api, id).await.is_ok() {
            n += 1;
        }
    }
    Ok(n)
}

/// The person's recent episodes on shows the user doesn't follow, newest first.
pub async fn episodes_for_user(db: &DatabasePool, person_id: i32, user_id: i32, limit: i64) -> AppResult<Vec<ExternalEpisode>> {
    Ok(fetch_all!(
        db,
        r#"SELECT x.id AS id, x.feedurl AS feedurl, x.podcasttitle AS podcasttitle, x.podcastartwork AS podcastartwork,
                  x.podcastindexid AS podcastindexid, x.episodetitle AS episodetitle, x.episodedescription AS episodedescription,
                  x.episodeurl AS episodeurl, x.episodeartwork AS episodeartwork, x.episodepubdate AS episodepubdate,
                  x.episodeduration AS episodeduration, x.role AS role
           FROM "PersonExternalEpisodes" x
           WHERE x.personid = $1
             AND NOT EXISTS (SELECT 1 FROM "Podcasts" p WHERE p.userid = $2 AND p.feedurl = x.feedurl)
           ORDER BY x.episodepubdate DESC
           LIMIT $3"#,
        [person_id, user_id, limit],
        |r| {
            let podcast_art = r.opt_str("podcastartwork")?.unwrap_or_default();
            ExternalEpisode {
                episodeid: -1,
                podcastid: 0,
                podcastname: r.opt_str("podcasttitle")?.unwrap_or_default(),
                feedurl: r.str("feedurl")?,
                podcastindexid: r.opt_int("podcastindexid")?,
                episodetitle: r.str("episodetitle")?,
                episodepubdate: r.ts("episodepubdate")?.map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string()).unwrap_or_default(),
                episodedescription: r.opt_str("episodedescription")?.unwrap_or_default(),
                episodeartwork: r.opt_str("episodeartwork")?.filter(|s| !s.is_empty()).unwrap_or(podcast_art),
                episodeurl: r.str("episodeurl")?,
                episodeduration: r.int("episodeduration")? as i32,
                listenduration: None,
                completed: false,
                saved: false,
                queued: false,
                downloaded: false,
                is_youtube: false,
                is_video: false,
                role: r.opt_str("role")?,
            }
        }
    ))
}

/// Shows outside the library the person appears on, from their external episodes.
pub fn podcasts_from(episodes: &[ExternalEpisode], art: &HashMap<String, String>) -> Vec<PersonPodcast> {
    let mut out: Vec<PersonPodcast> = Vec::new();
    for e in episodes {
        match out.iter_mut().find(|p| p.feedurl == e.feedurl) {
            Some(p) => {
                p.episode_count += 1;
                if let Some(role) = &e.role {
                    if !p.roles.contains(role) {
                        p.roles.push(role.clone());
                    }
                }
            }
            None => out.push(PersonPodcast {
                podcastid: None,
                podcastname: e.podcastname.clone(),
                artworkurl: art.get(&e.feedurl).cloned().or_else(|| Some(e.episodeartwork.clone())),
                feedurl: e.feedurl.clone(),
                podcastindexid: e.podcastindexid,
                roles: e.role.iter().cloned().collect(),
                podcast_level: false,
                episode_count: 1,
            }),
        }
    }
    out
}

/// Podcast artwork per feed for a person's external episodes.
pub async fn podcast_art(db: &DatabasePool, person_id: i32) -> AppResult<HashMap<String, String>> {
    Ok(fetch_all!(
        db,
        r#"SELECT feedurl AS feedurl, podcastartwork AS art FROM "PersonExternalEpisodes" WHERE personid = $1 AND podcastartwork IS NOT NULL"#,
        [person_id],
        |r| (r.str("feedurl")?, r.str("art")?)
    )
    .into_iter()
    .collect())
}
