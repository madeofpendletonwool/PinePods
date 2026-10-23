//! PodPeopleDB client: the background credits sync, contributions made from PinePods, and
//! people search. Nothing here runs on a page load except an explicit search or contribution.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use super::sql::{execute, fetch_all, fetch_optional};
use super::store::{self, episode_keys, insert_credits, resolve_podpeople_person, CreditRow, PersonSeed, WRITE_LOCK};
use super::{CreditedPerson, Endpoints, SOURCE_PODPEOPLE};
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::podcasting2::normalize_role;

const BATCH: usize = 200;

/// `PEOPLE_SYNC_ENABLED=false` turns off the background sync (feed credits still work).
pub fn sync_enabled() -> bool {
    !matches!(
        std::env::var("PEOPLE_SYNC_ENABLED").unwrap_or_default().trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "no" | "off"
    )
}

fn client(timeout_secs: u64) -> AppResult<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .user_agent(concat!("PinePods/", env!("CARGO_PKG_VERSION")))
        .build()?)
}

const CLIENT_HEADER: (&str, &str) = ("X-PodPeople-Client", concat!("pinepods/", env!("CARGO_PKG_VERSION")));

#[derive(Debug, Clone, Deserialize)]
struct PersonRef {
    id: i64,
    name: String,
    img: Option<String>,
}

impl PersonRef {
    fn seed(&self) -> PersonSeed {
        PersonSeed { name: self.name.clone(), img: self.img.clone(), href: None, podpeople_id: i32::try_from(self.id).ok() }
    }
}

#[derive(Debug, Deserialize)]
struct PodcastPerson {
    person: PersonRef,
    roles: Vec<String>,
    #[serde(default)]
    group: String,
    scope: String,
    #[serde(default)]
    sources: Vec<String>,
    #[serde(default)]
    verified: bool,
}

#[derive(Debug, Deserialize)]
struct EpisodeCredit {
    person: PersonRef,
    role: String,
    #[serde(default)]
    group: String,
    episode_guid: Option<String>,
    enclosure_url: Option<String>,
    #[serde(default)]
    verified: bool,
}

#[derive(Debug, Deserialize)]
struct BatchResult {
    index: usize,
    people: Vec<PodcastPerson>,
    #[serde(default)]
    episode_credits: Option<Vec<EpisodeCredit>>,
}

#[derive(Debug, Deserialize)]
struct BatchResponse {
    results: Vec<BatchResult>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct PodcastKey {
    #[serde(skip_serializing_if = "Option::is_none")]
    podcastindex_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    feed_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guid: Option<String>,
}

/// What PodPeopleDB says about one feed, ready to store.
#[derive(Debug, Default)]
struct Community {
    /// (person, role, group, verified)
    podcast: Vec<(PersonRef, String, Option<String>, bool)>,
    /// (person, role, group, verified, guid, enclosure url)
    episodes: Vec<(PersonRef, String, Option<String>, bool, Option<String>, Option<String>)>,
}

fn group(g: &str) -> Option<String> {
    (!g.trim().is_empty()).then(|| g.trim().to_string())
}

impl From<BatchResult> for Community {
    fn from(r: BatchResult) -> Self {
        let mut c = Community::default();
        for p in r.people {
            // Feed credits we read from the feed ourselves; only take what the community added.
            if p.scope != "podcast" || !p.sources.iter().any(|s| s == "community") {
                continue;
            }
            for role in &p.roles {
                c.podcast.push((p.person.clone(), normalize_role(Some(role)), group(&p.group), p.verified));
            }
        }
        for e in r.episode_credits.unwrap_or_default() {
            c.episodes.push((e.person, normalize_role(Some(&e.role)), group(&e.group), e.verified, e.episode_guid, e.enclosure_url));
        }
        c
    }
}

/// Replace one podcast's PodPeopleDB credits, writing only when they changed.
async fn store_community(db: &DatabasePool, podcast_id: i32, community: &Community) -> AppResult<bool> {
    let _guard = WRITE_LOCK.lock().await;
    let mut ids: HashMap<i64, i32> = HashMap::new();
    for p in community.podcast.iter().map(|c| &c.0).chain(community.episodes.iter().map(|c| &c.0)) {
        if !ids.contains_key(&p.id) {
            ids.insert(p.id, resolve_podpeople_person(db, &p.seed()).await?);
        }
    }

    let mut podcast_rows: Vec<CreditRow> = community
        .podcast
        .iter()
        .map(|(p, role, group, verified)| (podcast_id, ids[&p.id], role.clone(), group.clone(), *verified))
        .collect();
    let keys = episode_keys(db, podcast_id).await?;
    let mut episode_rows: Vec<CreditRow> = community
        .episodes
        .iter()
        .filter_map(|(p, role, group, verified, guid, url)| {
            Some((keys.find(guid.as_deref(), url.as_deref())?, ids[&p.id], role.clone(), group.clone(), *verified))
        })
        .collect();

    let mut existing_podcast = fetch_all!(
        db,
        r#"SELECT podcastid AS owner, personid AS personid, role AS role, persongroup AS persongroup, verified AS verified
           FROM "PodcastPeople" WHERE podcastid = $1 AND source = 'podpeople'"#,
        [podcast_id],
        |r| (r.int("owner")? as i32, r.int("personid")? as i32, r.str("role")?, r.opt_str("persongroup")?, r.flag("verified")?)
    );
    let mut existing_episodes = fetch_all!(
        db,
        r#"SELECT ep.episodeid AS owner, ep.personid AS personid, ep.role AS role, ep.persongroup AS persongroup, ep.verified AS verified
           FROM "EpisodePeople" ep JOIN "Episodes" e ON e.episodeid = ep.episodeid
           WHERE e.feedid = (SELECT feedid FROM "Podcasts" WHERE podcastid = $1) AND ep.source = 'podpeople'"#,
        [podcast_id],
        |r| (r.int("owner")? as i32, r.int("personid")? as i32, r.str("role")?, r.opt_str("persongroup")?, r.flag("verified")?)
    );
    for rows in [&mut podcast_rows, &mut episode_rows, &mut existing_podcast, &mut existing_episodes] {
        rows.sort();
        rows.dedup_by(|a, b| (a.0, a.1, &a.2) == (b.0, b.1, &b.2));
    }
    if podcast_rows == existing_podcast && episode_rows == existing_episodes {
        return Ok(false);
    }
    execute!(db, r#"DELETE FROM "PodcastPeople" WHERE podcastid = $1 AND source = 'podpeople'"#, [podcast_id]);
    insert_credits(db, "PodcastPeople", "podcastid", SOURCE_PODPEOPLE, &podcast_rows).await?;
    execute!(
        db,
        r#"DELETE FROM "EpisodePeople" WHERE source = 'podpeople' AND episodeid IN (SELECT episodeid FROM "Episodes" WHERE feedid = (SELECT feedid FROM "Podcasts" WHERE podcastid = $1))"#,
        [podcast_id]
    );
    insert_credits(db, "EpisodePeople", "episodeid", SOURCE_PODPEOPLE, &episode_rows).await?;
    Ok(true)
}

struct Feed {
    key: PodcastKey,
    podcast_ids: Vec<i32>,
}

/// Every RSS podcast on the server, grouped by feed (several users can follow one feed).
async fn feeds(db: &DatabasePool, only_podcast: Option<i32>) -> AppResult<Vec<Feed>> {
    let rows = fetch_all!(
        db,
        r#"SELECT podcastid AS podcastid, feedurl AS feedurl, podcastindexid AS podcastindexid, podcastguid AS podcastguid
           FROM "Podcasts" WHERE COALESCE(isyoutubechannel, FALSE) = FALSE AND feedurl LIKE 'http%'"#,
        [],
        |r| (r.int("podcastid")? as i32, r.str("feedurl")?, r.opt_int("podcastindexid")?, r.opt_str("podcastguid")?)
    );
    let wanted_feed = only_podcast.and_then(|id| rows.iter().find(|r| r.0 == id).map(|r| r.1.clone()));
    if only_podcast.is_some() && wanted_feed.is_none() {
        return Ok(Vec::new());
    }
    let mut out: Vec<Feed> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (id, feed_url, index_id, guid) in rows {
        if wanted_feed.as_ref().is_some_and(|f| f != &feed_url) {
            continue;
        }
        let i = *index.entry(feed_url.clone()).or_insert_with(|| {
            out.push(Feed { key: PodcastKey { feed_url: Some(feed_url.clone()), ..Default::default() }, podcast_ids: Vec::new() });
            out.len() - 1
        });
        let feed = &mut out[i];
        feed.podcast_ids.push(id);
        if feed.key.podcastindex_id.is_none() {
            feed.key.podcastindex_id = index_id.filter(|v| *v > 0);
        }
        if feed.key.guid.is_none() {
            feed.key.guid = guid;
        }
    }
    Ok(out)
}

#[derive(Debug)]
enum BatchError {
    /// The server predates /api/v2.
    Unsupported,
    Other(String),
}

async fn fetch_batch(http: &reqwest::Client, api: &Endpoints, keys: &[PodcastKey]) -> Result<HashMap<usize, Community>, BatchError> {
    let resp = http
        .post(format!("{}/api/v2/podcasts/batch", api.people))
        .header(CLIENT_HEADER.0, CLIENT_HEADER.1)
        .json(&serde_json::json!({ "podcasts": keys, "include_episode_credits": true }))
        .send()
        .await
        .map_err(|e| BatchError::Other(e.to_string()))?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND || resp.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
        return Err(BatchError::Unsupported);
    }
    if !resp.status().is_success() {
        return Err(BatchError::Other(format!("HTTP {}", resp.status())));
    }
    let body: BatchResponse = resp.json().await.map_err(|e| BatchError::Other(e.to_string()))?;
    Ok(body.results.into_iter().map(|r| (r.index, Community::from(r))).collect())
}

/// PodPeopleDB v1 (`/api/hosts/{podcastindex id}`): approved hosts only.
async fn fetch_v1(http: &reqwest::Client, api: &Endpoints, index_id: i64) -> AppResult<Community> {
    #[derive(Deserialize)]
    struct Host {
        id: Option<i64>,
        name: String,
        role: Option<String>,
        img: Option<String>,
    }
    let resp = http.get(format!("{}/api/hosts/{}", api.people, index_id)).send().await?;
    if !resp.status().is_success() {
        return Ok(Community::default());
    }
    let hosts: Vec<Host> = resp.json().await.unwrap_or_default();
    let mut c = Community::default();
    for h in hosts {
        let Some(id) = h.id.filter(|v| *v > 0) else { continue };
        c.podcast.push((
            PersonRef { id, name: h.name, img: h.img.filter(|s| !s.is_empty()) },
            normalize_role(h.role.as_deref()),
            None,
            true,
        ));
    }
    Ok(c)
}

/// Pull community credits for every podcast on the server (or just `only_podcast`'s feed).
/// Returns how many podcasts changed.
pub async fn sync(db: &DatabasePool, api: &Endpoints, only_podcast: Option<i32>) -> AppResult<usize> {
    let feeds = feeds(db, only_podcast).await?;
    if feeds.is_empty() {
        return Ok(0);
    }
    let http = client(30)?;
    let mut changed = 0;
    for chunk in feeds.chunks(BATCH) {
        let keys: Vec<PodcastKey> = chunk.iter().map(|f| f.key.clone()).collect();
        let results = match fetch_batch(&http, api, &keys).await {
            Ok(r) => r,
            Err(BatchError::Unsupported) => {
                tracing::debug!("PodPeopleDB has no /api/v2; syncing hosts through v1");
                let mut r = HashMap::new();
                for (i, f) in chunk.iter().enumerate() {
                    if let Some(index_id) = f.key.podcastindex_id {
                        match fetch_v1(&http, api, index_id).await {
                            Ok(c) => {
                                r.insert(i, c);
                            }
                            Err(e) => tracing::debug!("PodPeopleDB v1 lookup for {} failed: {}", index_id, e),
                        }
                    }
                }
                r
            }
            Err(BatchError::Other(e)) => {
                // Leave stored credits alone when PodPeopleDB can't be reached.
                tracing::warn!("PodPeopleDB sync failed: {}", e);
                return Ok(changed);
            }
        };
        let empty = Community::default();
        for (i, feed) in chunk.iter().enumerate() {
            let community = results.get(&i).unwrap_or(&empty);
            for podcast_id in &feed.podcast_ids {
                match store_community(db, *podcast_id, community).await {
                    Ok(true) => changed += 1,
                    Ok(false) => {}
                    Err(e) => tracing::warn!("Storing PodPeopleDB credits for podcast {} failed: {}", podcast_id, e),
                }
            }
        }
    }
    if changed > 0 {
        tracing::info!("PodPeopleDB sync updated credits on {} podcasts", changed);
    }
    Ok(changed)
}

static PENDING: std::sync::Mutex<Vec<i32>> = std::sync::Mutex::new(Vec::new());

/// Sync a newly added podcast soon (within a minute), from whichever path added it.
pub fn queue_sync(podcast_id: i32) {
    if sync_enabled() {
        PENDING.lock().unwrap_or_else(|e| e.into_inner()).push(podcast_id);
    }
}

/// Sync the podcasts queued by [`queue_sync`]. Run every minute by the people startup task.
pub async fn sync_pending(db: &DatabasePool, api: &Endpoints) {
    let mut ids = std::mem::take(&mut *PENDING.lock().unwrap_or_else(|e| e.into_inner()));
    ids.sort_unstable();
    ids.dedup();
    for id in ids {
        if let Err(e) = sync(db, api, Some(id)).await {
            tracing::debug!("PodPeopleDB sync for podcast {} failed: {}", id, e);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Contributions
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ContributePerson {
    /// A person PinePods already knows (from a credit or search result).
    pub person_id: Option<i32>,
    /// A PodPeopleDB person (from search results).
    pub podpeople_id: Option<i64>,
    /// Or just a name.
    pub name: Option<String>,
    /// Defaults to "host" on a podcast and "guest" on an episode.
    pub role: Option<String>,
}

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct ContributeRequest {
    pub podcast_id: i32,
    /// Credit these people on one episode instead of the whole show.
    pub episode_id: Option<i32>,
    pub people: Vec<ContributePerson>,
}

#[derive(Debug, Deserialize)]
struct AddedCredit {
    person: PersonRef,
    role: String,
}

#[derive(Debug, Deserialize)]
struct SubmissionResult {
    #[serde(default)]
    credits: Vec<AddedCredit>,
}

/// Identifies this PinePods server to PodPeopleDB without revealing anything about it.
async fn instance_id(db: &DatabasePool) -> Option<String> {
    use sha2::Digest;
    let key = db.get_encryption_key().await.ok()?;
    Some(hex::encode(&sha2::Sha256::digest(format!("pinepods-instance:{key}").as_bytes())[..16]))
}

/// Send people to PodPeopleDB for the user's podcast (or one of its episodes), then store the
/// returned credits locally so they show up right away. Returns the podcast's or episode's
/// people afterwards.
pub async fn contribute(db: &DatabasePool, api: &Endpoints, user_id: i32, req: &ContributeRequest) -> AppResult<Vec<CreditedPerson>> {
    if req.people.is_empty() {
        return Err(AppError::bad_request("Add at least one person"));
    }
    if req.people.len() > 25 {
        return Err(AppError::bad_request("At most 25 people at a time"));
    }
    store::owned_podcast(db, req.podcast_id, user_id).await?;
    if let Some(episode_id) = req.episode_id {
        if store::owned_episode_podcast(db, episode_id, user_id).await? != req.podcast_id {
            return Err(AppError::bad_request("Episode is not from this podcast"));
        }
    }

    let podcast = fetch_optional!(
        db,
        r#"SELECT feedurl AS feedurl, podcastindexid AS podcastindexid, podcastguid AS podcastguid, podcastname AS podcastname,
                  artworkurl AS artworkurl, author AS author, websiteurl AS websiteurl
           FROM "Podcasts" WHERE podcastid = $1"#,
        [req.podcast_id],
        |r| serde_json::json!({
            "podcastindex_id": r.opt_int("podcastindexid")?.filter(|v| *v > 0),
            "feed_url": r.opt_str("feedurl")?.filter(|u| u.starts_with("http")),
            "guid": r.opt_str("podcastguid")?,
            "title": r.opt_str("podcastname")?,
            "image": r.opt_str("artworkurl")?,
            "author": r.opt_str("author")?,
            "link": r.opt_str("websiteurl")?,
        })
    )
    .ok_or_else(|| AppError::not_found("Podcast not found"))?;

    let episode = match req.episode_id {
        Some(id) => fetch_optional!(
            db,
            r#"SELECT episodeguid AS guid, episodeurl AS url, episodetitle AS title, episodepubdate AS pubdate FROM "Episodes" WHERE episodeid = $1"#,
            [id],
            |r| serde_json::json!({
                "guid": r.opt_str("guid")?,
                "enclosure_url": r.opt_str("url")?,
                "title": r.opt_str("title")?,
                "pub_date": r.ts("pubdate")?.map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            })
        ),
        None => None,
    };

    let mut people = Vec::new();
    for p in &req.people {
        let mut entry = serde_json::Map::new();
        let mut podpeople_id = p.podpeople_id;
        let mut name = p.name.clone().map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
        if let Some(local) = p.person_id {
            let found = store::person(db, local, user_id).await?;
            podpeople_id = podpeople_id.or(found.podpeople_id.map(i64::from));
            name = name.or(Some(found.name));
        }
        match (podpeople_id, name) {
            (Some(id), _) => entry.insert("person_id".into(), id.into()),
            (None, Some(n)) => entry.insert("name".into(), n.into()),
            (None, None) => return Err(AppError::bad_request("Each person needs a name")),
        };
        if let Some(role) = p.role.as_deref().map(str::trim).filter(|r| !r.is_empty()) {
            entry.insert("role".into(), role.into());
        }
        people.push(serde_json::Value::Object(entry));
    }

    let body = serde_json::json!({
        "podcast": podcast,
        "episode": episode,
        "people": people,
        "instance_id": instance_id(db).await,
    });
    let resp = client(20)?
        .post(format!("{}/api/v2/submissions", api.people))
        .header(CLIENT_HEADER.0, CLIENT_HEADER.1)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::external_error(&format!("Couldn't reach PodPeopleDB: {e}")))?;
    let status = resp.status();
    if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
        return Err(AppError::bad_request("PodPeopleDB is getting a lot of submissions from this server; try again in a while"));
    }
    if !status.is_success() {
        let detail = resp.text().await.unwrap_or_default();
        let detail = serde_json::from_str::<serde_json::Value>(&detail)
            .ok()
            .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
            .unwrap_or_else(|| format!("HTTP {status}"));
        return Err(AppError::bad_request(format!("PodPeopleDB didn't accept that: {detail}")));
    }
    let result: SubmissionResult = resp
        .json()
        .await
        .map_err(|e| AppError::external_error(&format!("Unexpected PodPeopleDB response: {e}")))?;

    // Store what was added right away (unverified until an admin reviews it); the next sync
    // brings the same rows back from PodPeopleDB.
    {
        let _guard = WRITE_LOCK.lock().await;
        let mut rows: Vec<CreditRow> = Vec::new();
        for c in &result.credits {
            let person_id = resolve_podpeople_person(db, &c.person.seed()).await?;
            let owner = req.episode_id.unwrap_or(req.podcast_id);
            rows.push((owner, person_id, normalize_role(Some(&c.role)), None, false));
        }
        let podcast_ids: Vec<i32> = match req.episode_id {
            Some(_) => vec![req.podcast_id],
            None => same_feed_podcasts(db, req.podcast_id).await?,
        };
        for row in rows {
            let exists_sql = if req.episode_id.is_some() {
                r#"SELECT personid AS personid FROM "EpisodePeople" WHERE episodeid = $1 AND personid = $2 AND role = $3 AND source = 'podpeople'"#
            } else {
                r#"SELECT personid AS personid FROM "PodcastPeople" WHERE podcastid = $1 AND personid = $2 AND role = $3 AND source = 'podpeople'"#
            };
            let owners: Vec<i32> = if req.episode_id.is_some() { vec![row.0] } else { podcast_ids.clone() };
            for owner in owners {
                let exists = fetch_optional!(db, exists_sql, [owner, row.1, row.2.clone()], |r| r.int("personid")?);
                if exists.is_none() {
                    let one = [(owner, row.1, row.2.clone(), row.3.clone(), row.4)];
                    if req.episode_id.is_some() {
                        insert_credits(db, "EpisodePeople", "episodeid", SOURCE_PODPEOPLE, &one).await?;
                    } else {
                        insert_credits(db, "PodcastPeople", "podcastid", SOURCE_PODPEOPLE, &one).await?;
                    }
                }
            }
        }
    }
    tracing::info!("User {} added {} credits to PodPeopleDB for podcast {}", user_id, result.credits.len(), req.podcast_id);

    match req.episode_id {
        Some(episode_id) => store::episode_credits(db, episode_id, user_id).await,
        None => {
            let credits = store::podcast_credits(db, req.podcast_id, user_id).await?;
            Ok(credits.people.into_iter().chain(credits.guests).collect())
        }
    }
}

/// Every podcast row (any user) that follows the same feed as `podcast_id`.
async fn same_feed_podcasts(db: &DatabasePool, podcast_id: i32) -> AppResult<Vec<i32>> {
    let ids = fetch_all!(
        db,
        r#"SELECT p.podcastid AS podcastid FROM "Podcasts" p
           WHERE p.feedurl = (SELECT q.feedurl FROM "Podcasts" q WHERE q.podcastid = $1)"#,
        [podcast_id],
        |r| r.int("podcastid")? as i32
    );
    Ok(if ids.is_empty() { vec![podcast_id] } else { ids })
}

// ---------------------------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PersonHit {
    /// PinePods person id, when this server already knows them.
    pub person_id: Option<i32>,
    pub podpeople_id: Option<i64>,
    pub name: String,
    pub img: Option<String>,
    /// A few podcasts they're known for, to tell namesakes apart.
    pub known_for: Option<String>,
    pub followed: bool,
}

#[derive(Debug, Deserialize)]
struct RemoteHit {
    id: i64,
    name: String,
    img: Option<String>,
    known_for: Option<String>,
}

/// People matching `query`: this server's people first, then PodPeopleDB's (merged by id).
pub async fn search(db: &DatabasePool, api: &Endpoints, user_id: i32, query: &str) -> AppResult<Vec<PersonHit>> {
    let query = query.trim();
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let local = store::search(db, query, user_id, 10).await?;
    let mut hits: Vec<PersonHit> = local
        .into_iter()
        .map(|(p, _)| PersonHit {
            person_id: Some(p.id),
            podpeople_id: p.podpeople_id.map(i64::from),
            name: p.name,
            img: p.img,
            known_for: None,
            followed: p.followed,
        })
        .collect();
    let known: HashSet<i64> = hits.iter().filter_map(|h| h.podpeople_id).collect();
    let remote = async {
        let resp = client(6)?
            .get(format!("{}/api/v2/people/search", api.people))
            .query(&[("q", query), ("limit", "10")])
            .send()
            .await?;
        if !resp.status().is_success() {
            return Ok::<Vec<RemoteHit>, AppError>(Vec::new());
        }
        Ok(resp.json::<Vec<RemoteHit>>().await.unwrap_or_default())
    }
    .await;
    match remote {
        Ok(remote) => {
            for r in remote {
                // A local person not linked to PodPeopleDB yet (feed credits before the next sync)
                // is the same entry as a same-named remote hit, not a second suggestion.
                let linked = hits.iter().position(|h| h.podpeople_id == Some(r.id)).or_else(|| {
                    hits.iter().position(|h| h.podpeople_id.is_none() && h.name.eq_ignore_ascii_case(&r.name))
                });
                if let Some(local) = linked.map(|i| &mut hits[i]) {
                    local.podpeople_id = Some(r.id);
                    local.known_for = r.known_for;
                    if local.img.is_none() {
                        local.img = r.img;
                    }
                } else if !known.contains(&r.id) {
                    hits.push(PersonHit { person_id: None, podpeople_id: Some(r.id), name: r.name, img: r.img, known_for: r.known_for, followed: false });
                }
            }
        }
        Err(e) => tracing::debug!("PodPeopleDB people search failed: {}", e),
    }
    Ok(hits)
}

/// A PinePods person for a PodPeopleDB person (creating them), so a search hit can be opened
/// and followed.
pub async fn person_for_podpeople(db: &DatabasePool, api: &Endpoints, podpeople_id: i64) -> AppResult<i32> {
    if let Some(id) = fetch_optional!(
        db,
        r#"SELECT personid AS personid FROM "Persons" WHERE podpeopleid = $1"#,
        [i32::try_from(podpeople_id).map_err(|_| AppError::bad_request("Bad PodPeopleDB id"))?],
        |r| r.int("personid")? as i32
    ) {
        return Ok(id);
    }
    #[derive(Deserialize)]
    struct Detail {
        person: RemotePerson,
    }
    #[derive(Deserialize)]
    struct RemotePerson {
        id: i64,
        name: String,
        img: Option<String>,
        website: Option<String>,
    }
    let resp = client(10)?.get(format!("{}/api/v2/people/{}", api.people, podpeople_id)).send().await?;
    if !resp.status().is_success() {
        return Err(AppError::not_found("Person not found on PodPeopleDB"));
    }
    let detail: Detail = resp.json().await.map_err(|e| AppError::external_error(&format!("Unexpected PodPeopleDB response: {e}")))?;
    let _guard = WRITE_LOCK.lock().await;
    resolve_podpeople_person(
        db,
        &PersonSeed {
            name: detail.person.name,
            img: detail.person.img,
            href: detail.person.website,
            podpeople_id: i32::try_from(detail.person.id).ok(),
        },
    )
    .await
}
