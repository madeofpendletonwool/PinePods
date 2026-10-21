//! Database reads and writes for people and credits. Queries are written once in PostgreSQL
//! form (see `sql.rs`); the few that need backend-specific syntax match on the pool directly.

use std::collections::{BTreeSet, HashMap, HashSet};

use super::sql::{execute, fetch_all, fetch_optional, in_list, values_placeholders, Bind};
use super::{
    is_host_role, role_rank, CreditedPerson, Person, PersonPodcast, PodcastCredits, SOURCE_FEED,
};
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::podcasting2::{normalize_name, normalize_role, FeedPerson, FeedTags};

/// Bump to make every podcast re-apply its stored tags on the next refresh (when the way tags
/// are written changes).
const TAGS_VERSION: &str = "1";
const CHUNK: usize = 400;

/// Serializes person creation and credit replacement, so two refreshes never create the same
/// person twice or interleave a delete-and-insert of one podcast's credits.
pub(crate) static WRITE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn cut(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn url_base(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

// ---------------------------------------------------------------------------------------------
// Persons
// ---------------------------------------------------------------------------------------------

/// Details to create or fill in a person with.
#[derive(Debug, Clone, Default)]
pub struct PersonSeed {
    pub name: String,
    pub img: Option<String>,
    pub href: Option<String>,
    pub podpeople_id: Option<i32>,
}

async fn insert_person(db: &DatabasePool, seed: &PersonSeed, norm: &str) -> AppResult<i32> {
    let name = cut(&seed.name, 255);
    let norm = cut(norm, 255);
    match db {
        DatabasePool::Postgres(pool) => Ok(sqlx::query_scalar::<_, i32>(
            r#"INSERT INTO "Persons" (name, namenormalized, img, href, podpeopleid) VALUES ($1, $2, $3, $4, $5) RETURNING personid"#,
        )
        .bind(name)
        .bind(norm)
        .bind(&seed.img)
        .bind(&seed.href)
        .bind(seed.podpeople_id)
        .fetch_one(pool)
        .await?),
        DatabasePool::MySQL(pool) => {
            let res = sqlx::query("INSERT INTO Persons (Name, NameNormalized, Img, Href, PodPeopleID) VALUES (?, ?, ?, ?, ?)")
                .bind(name)
                .bind(norm)
                .bind(&seed.img)
                .bind(&seed.href)
                .bind(seed.podpeople_id)
                .execute(pool)
                .await?;
            Ok(res.last_insert_id() as i32)
        }
    }
}

struct ExistingPerson {
    id: i32,
    norm: String,
    img: Option<String>,
    href: Option<String>,
    podpeople_id: Option<i32>,
}

async fn persons_by_norm(db: &DatabasePool, norms: &[String]) -> AppResult<Vec<ExistingPerson>> {
    let mut out = Vec::new();
    for chunk in norms.chunks(CHUNK) {
        let sql = format!(
            r#"SELECT personid AS personid, namenormalized AS norm, img AS img, href AS href, podpeopleid AS podpeopleid
               FROM "Persons" WHERE namenormalized IN ({}) ORDER BY personid"#,
            in_list(1, chunk.len())
        );
        let binds: Vec<Bind> = chunk.iter().map(|n| Bind::Text(Some(n.clone()))).collect();
        out.extend(fetch_all!(db, sql, dyn binds, |r| ExistingPerson {
            id: r.int("personid")? as i32,
            norm: r.str("norm")?,
            img: r.opt_str("img")?,
            href: r.opt_str("href")?,
            podpeople_id: r.opt_int("podpeopleid")?.map(|v| v as i32),
        }));
    }
    Ok(out)
}

async fn fill_missing(db: &DatabasePool, existing: &ExistingPerson, seed: &PersonSeed) -> AppResult<()> {
    let img = existing.img.is_none().then(|| seed.img.clone()).flatten();
    let href = existing.href.is_none().then(|| seed.href.clone()).flatten();
    if img.is_some() || href.is_some() {
        execute!(
            db,
            r#"UPDATE "Persons" SET img = COALESCE(img, $1), href = COALESCE(href, $2) WHERE personid = $3"#,
            [img, href, existing.id]
        );
    }
    Ok(())
}

/// Person ids for names seen in a feed, creating people that don't exist yet. Keyed by
/// normalized name. Caller holds [`WRITE_LOCK`].
async fn resolve_by_name(db: &DatabasePool, seeds: &[PersonSeed]) -> AppResult<HashMap<String, i32>> {
    // One seed per normalized name, keeping the first image/link any mention offers.
    let mut by_norm: HashMap<String, PersonSeed> = HashMap::new();
    let mut order = Vec::new();
    for seed in seeds {
        let norm = normalize_name(&seed.name);
        if norm.is_empty() {
            continue;
        }
        match by_norm.get_mut(&norm) {
            Some(s) => {
                if s.img.is_none() {
                    s.img = seed.img.clone();
                }
                if s.href.is_none() {
                    s.href = seed.href.clone();
                }
            }
            None => {
                order.push(norm.clone());
                by_norm.insert(norm, seed.clone());
            }
        }
    }

    let mut ids = HashMap::new();
    for existing in persons_by_norm(db, &order).await? {
        if ids.contains_key(&existing.norm) {
            continue;
        }
        if let Some(seed) = by_norm.get(&existing.norm) {
            fill_missing(db, &existing, seed).await?;
        }
        ids.insert(existing.norm.clone(), existing.id);
    }
    for norm in order {
        if !ids.contains_key(&norm) {
            let id = insert_person(db, &by_norm[&norm], &norm).await?;
            ids.insert(norm, id);
        }
    }
    Ok(ids)
}

/// The person for a PodPeopleDB person: matched by PodPeopleDB id, else adopted by name (a
/// feed-only person gets the id), else created. Name and image follow PodPeopleDB, which is
/// curated. Caller holds [`WRITE_LOCK`].
pub(crate) async fn resolve_podpeople_person(db: &DatabasePool, seed: &PersonSeed) -> AppResult<i32> {
    let Some(pp_id) = seed.podpeople_id else {
        return Err(AppError::internal("PodPeopleDB person without an id"));
    };
    let found = fetch_optional!(
        db,
        r#"SELECT personid AS personid, img AS img, href AS href FROM "Persons" WHERE podpeopleid = $1"#,
        [pp_id],
        |r| (r.int("personid")? as i32, r.opt_str("img")?, r.opt_str("href")?)
    );
    if let Some((id, img, href)) = found {
        if (seed.img.is_some() && seed.img != img) || (href.is_none() && seed.href.is_some()) {
            execute!(
                db,
                r#"UPDATE "Persons" SET img = COALESCE($1, img), href = COALESCE(href, $2) WHERE personid = $3"#,
                [seed.img.clone(), seed.href.clone(), id]
            );
        }
        return Ok(id);
    }
    let norm = normalize_name(&seed.name);
    let adoptable = persons_by_norm(db, std::slice::from_ref(&norm))
        .await?
        .into_iter()
        .find(|p| p.podpeople_id.is_none());
    if let Some(existing) = adoptable {
        execute!(
            db,
            r#"UPDATE "Persons" SET podpeopleid = $1, img = COALESCE($2, img), href = COALESCE(href, $3) WHERE personid = $4"#,
            [pp_id, seed.img.clone(), seed.href.clone(), existing.id]
        );
        return Ok(existing.id);
    }
    insert_person(db, seed, &norm).await
}

// ---------------------------------------------------------------------------------------------
// Credits
// ---------------------------------------------------------------------------------------------

/// One credit row: (podcast or episode id, person id, role, group, verified).
pub(crate) type CreditRow = (i32, i32, String, Option<String>, bool);

/// Insert credits into `PodcastPeople` (owner column `podcastid`) or `EpisodePeople`
/// (`episodeid`). Duplicates of the primary key are dropped first.
pub(crate) async fn insert_credits(db: &DatabasePool, table: &str, owner: &str, source: &str, rows: &[CreditRow]) -> AppResult<()> {
    let mut seen = HashSet::new();
    let rows: Vec<&CreditRow> = rows.iter().filter(|r| seen.insert((r.0, r.1, r.2.clone()))).collect();
    for chunk in rows.chunks(CHUNK) {
        let sql = format!(
            r#"INSERT INTO "{table}" ({owner}, personid, role, persongroup, source, verified) VALUES {}"#,
            values_placeholders(chunk.len(), 6)
        );
        let mut binds = Vec::with_capacity(chunk.len() * 6);
        for (owner_id, person_id, role, group, verified) in chunk {
            binds.push(Bind::Int(*owner_id));
            binds.push(Bind::Int(*person_id));
            binds.push(Bind::Text(Some(cut(role, 64))));
            binds.push(Bind::Text(group.as_deref().map(|g| cut(g, 64))));
            binds.push(Bind::Text(Some(source.to_string())));
            binds.push(Bind::Bool(*verified));
        }
        execute!(db, sql, dyn binds);
    }
    Ok(())
}

pub(crate) struct EpisodeKeys {
    pub by_guid: HashMap<String, i32>,
    pub by_url: HashMap<String, i32>,
    /// episode id -> (chapters url, transcripts json) as stored.
    pub stored: HashMap<i32, (Option<String>, Option<String>)>,
}

impl EpisodeKeys {
    pub fn find(&self, guid: Option<&str>, url: Option<&str>) -> Option<i32> {
        guid.and_then(|g| self.by_guid.get(g).copied())
            .or_else(|| url.and_then(|u| self.by_url.get(url_base(u)).copied()))
    }
}

pub(crate) async fn episode_keys(db: &DatabasePool, podcast_id: i32) -> AppResult<EpisodeKeys> {
    let rows = fetch_all!(
        db,
        r#"SELECT episodeid AS episodeid, episodeguid AS guid, episodeurl AS url, chaptersurl AS chapters, transcriptsjson AS transcripts
           FROM "Episodes" WHERE podcastid = $1"#,
        [podcast_id],
        |r| (r.int("episodeid")? as i32, r.opt_str("guid")?, r.opt_str("url")?, r.opt_str("chapters")?, r.opt_str("transcripts")?)
    );
    let mut keys = EpisodeKeys { by_guid: HashMap::new(), by_url: HashMap::new(), stored: HashMap::new() };
    for (id, guid, url, chapters, transcripts) in rows {
        if let Some(g) = guid.filter(|g| !g.is_empty()) {
            keys.by_guid.entry(g).or_insert(id);
        }
        if let Some(u) = url.filter(|u| !u.is_empty()) {
            keys.by_url.entry(url_base(&u).to_string()).or_insert(id);
        }
        keys.stored.insert(id, (chapters, transcripts));
    }
    Ok(keys)
}

/// Store a feed's Podcasting 2.0 tags for one podcast: people credited on the show and its
/// episodes, episode chapters/transcript links, and channel extras (podroll, funding, value).
/// Run after the feed's episodes are applied, so new episodes get their credits. A feed whose
/// tags haven't changed since the last call costs one read. Returns true when anything was written.
pub async fn apply_feed_tags(db: &DatabasePool, podcast_id: i32, tags: &FeedTags) -> AppResult<bool> {
    let fingerprint = format!("{TAGS_VERSION}:{}", tags.fingerprint());
    let stored = fetch_optional!(
        db,
        r#"SELECT podcasting2hash AS hash FROM "Podcasts" WHERE podcastid = $1"#,
        [podcast_id],
        |r| r.opt_str("hash")?
    );
    match stored {
        None => return Ok(false),
        Some(Some(h)) if h == fingerprint => return Ok(false),
        _ => {}
    }

    let _guard = WRITE_LOCK.lock().await;

    let seeds: Vec<PersonSeed> = tags
        .people
        .iter()
        .chain(tags.items.iter().flat_map(|i| i.people.iter()))
        .map(|p: &FeedPerson| PersonSeed { name: p.name.clone(), img: p.img.clone(), href: p.href.clone(), podpeople_id: None })
        .collect();
    let ids = resolve_by_name(db, &seeds).await?;
    let person = |p: &FeedPerson| ids.get(&normalize_name(&p.name)).copied();

    let podcast_rows: Vec<CreditRow> = tags
        .people
        .iter()
        .filter_map(|p| Some((podcast_id, person(p)?, normalize_role(p.role.as_deref()), p.group.clone(), true)))
        .collect();
    execute!(db, r#"DELETE FROM "PodcastPeople" WHERE podcastid = $1 AND source = 'feed'"#, [podcast_id]);
    insert_credits(db, "PodcastPeople", "podcastid", SOURCE_FEED, &podcast_rows).await?;

    let keys = episode_keys(db, podcast_id).await?;
    let mut episode_rows: Vec<CreditRow> = Vec::new();
    let mut wanted: HashMap<i32, (Option<String>, Option<String>)> = HashMap::new();
    for item in &tags.items {
        let Some(episode_id) = keys.find(item.guid.as_deref(), item.enclosure_url.as_deref()) else { continue };
        for p in &item.people {
            if let Some(pid) = person(p) {
                episode_rows.push((episode_id, pid, normalize_role(p.role.as_deref()), p.group.clone(), true));
            }
        }
        let transcripts = (!item.transcripts.is_empty()).then(|| serde_json::to_string(&item.transcripts).unwrap_or_default());
        wanted.entry(episode_id).or_insert((item.chapters_url.clone(), transcripts));
    }
    execute!(
        db,
        r#"DELETE FROM "EpisodePeople" WHERE source = 'feed' AND episodeid IN (SELECT episodeid FROM "Episodes" WHERE podcastid = $1)"#,
        [podcast_id]
    );
    insert_credits(db, "EpisodePeople", "episodeid", SOURCE_FEED, &episode_rows).await?;

    for (episode_id, stored) in &keys.stored {
        let want = wanted.remove(episode_id).unwrap_or((None, None));
        if &want != stored {
            execute!(
                db,
                r#"UPDATE "Episodes" SET chaptersurl = $1, transcriptsjson = $2 WHERE episodeid = $3"#,
                [want.0, want.1, *episode_id]
            );
        }
    }

    let extras = serde_json::to_string(&tags.extras).unwrap_or_else(|_| "{}".into());
    let guid = tags.podcast_guid.as_deref().map(|g| cut(g, 255));
    execute!(
        db,
        r#"UPDATE "Podcasts" SET podcasting2data = $1, podcasting2hash = $2, podcastguid = $3 WHERE podcastid = $4"#,
        [extras, fingerprint, guid, podcast_id]
    );
    tracing::debug!(
        "Stored Podcasting 2.0 tags for podcast {}: {} show credits, {} episode credits",
        podcast_id,
        podcast_rows.len(),
        episode_rows.len()
    );
    Ok(true)
}

// ---------------------------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------------------------

#[derive(Clone)]
struct RawCredit {
    person_id: i32,
    podpeople_id: Option<i32>,
    name: String,
    img: Option<String>,
    href: Option<String>,
    role: String,
    group: Option<String>,
    source: String,
    verified: bool,
    /// None for a podcast-level credit.
    episode_id: Option<i32>,
}

const PERSON_COLS: &str = "pe.personid AS personid, pe.podpeopleid AS podpeopleid, pe.name AS name, pe.img AS img, pe.href AS href";

macro_rules! raw_credit {
    ($r:ident, $episode:expr) => {
        RawCredit {
            person_id: $r.int("personid")? as i32,
            podpeople_id: $r.opt_int("podpeopleid")?.map(|v| v as i32),
            name: $r.str("name")?,
            img: $r.opt_str("img")?,
            href: $r.opt_str("href")?,
            role: $r.str("role")?,
            group: $r.opt_str("persongroup")?,
            source: $r.str("source")?,
            verified: $r.flag("verified")?,
            episode_id: $episode,
        }
    };
}

pub async fn followed_ids(db: &DatabasePool, user_id: i32) -> AppResult<HashSet<i32>> {
    Ok(fetch_all!(db, r#"SELECT personid AS personid FROM "PersonFollows" WHERE userid = $1"#, [user_id], |r| r.int("personid")? as i32)
        .into_iter()
        .collect())
}

/// The user's podcast, or not found (also for another user's podcast).
pub async fn owned_podcast(db: &DatabasePool, podcast_id: i32, user_id: i32) -> AppResult<()> {
    fetch_optional!(
        db,
        r#"SELECT podcastid AS podcastid FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#,
        [podcast_id, user_id],
        |r| r.int("podcastid")?
    )
    .map(|_| ())
    .ok_or_else(|| AppError::not_found("Podcast not found"))
}

/// The podcast of the user's episode, or not found.
pub async fn owned_episode_podcast(db: &DatabasePool, episode_id: i32, user_id: i32) -> AppResult<i32> {
    fetch_optional!(
        db,
        r#"SELECT e.podcastid AS podcastid FROM "Episodes" e JOIN "Podcasts" p ON p.podcastid = e.podcastid
           WHERE e.episodeid = $1 AND p.userid = $2"#,
        [episode_id, user_id],
        |r| r.int("podcastid")? as i32
    )
    .ok_or_else(|| AppError::not_found("Episode not found"))
}

struct Agg {
    first: RawCredit,
    roles: BTreeSet<(u8, String)>,
    group: Option<String>,
    sources: BTreeSet<String>,
    verified: bool,
    podcast_level: bool,
    episodes: HashSet<i32>,
    host_episodes: HashSet<i32>,
}

fn aggregate(raws: Vec<RawCredit>) -> Vec<Agg> {
    let mut order: Vec<i32> = Vec::new();
    let mut by_person: HashMap<i32, Agg> = HashMap::new();
    for c in raws {
        let agg = by_person.entry(c.person_id).or_insert_with(|| {
            order.push(c.person_id);
            Agg {
                first: c.clone(),
                roles: BTreeSet::new(),
                group: None,
                sources: BTreeSet::new(),
                verified: false,
                podcast_level: false,
                episodes: HashSet::new(),
                host_episodes: HashSet::new(),
            }
        });
        agg.roles.insert(role_rank(&c.role));
        if agg.group.is_none() {
            agg.group = c.group.clone();
        }
        agg.sources.insert(c.source.clone());
        agg.verified |= c.verified;
        match c.episode_id {
            None => agg.podcast_level = true,
            Some(e) => {
                agg.episodes.insert(e);
                if is_host_role(&c.role) {
                    agg.host_episodes.insert(e);
                }
            }
        }
    }
    order.into_iter().filter_map(|id| by_person.remove(&id)).collect()
}

fn credited(agg: &Agg, scope: &str, followed: &HashSet<i32>) -> CreditedPerson {
    let roles: Vec<String> = agg.roles.iter().map(|(_, r)| r.clone()).collect();
    CreditedPerson {
        id: agg.first.person_id,
        podpeople_id: agg.first.podpeople_id,
        name: agg.first.name.clone(),
        img: agg.first.img.clone(),
        href: agg.first.href.clone(),
        role: roles.first().cloned().unwrap_or_else(|| "host".into()),
        roles,
        group: agg.group.clone(),
        scope: scope.to_string(),
        episode_count: agg.episodes.len() as i64,
        sources: agg.sources.iter().cloned().collect(),
        verified: agg.verified,
        followed: followed.contains(&agg.first.person_id),
    }
}

async fn podcast_raw_credits(db: &DatabasePool, podcast_id: i32) -> AppResult<Vec<RawCredit>> {
    let mut raws = fetch_all!(
        db,
        format!(
            r#"SELECT {PERSON_COLS}, pp.role AS role, pp.persongroup AS persongroup, pp.source AS source, pp.verified AS verified
               FROM "PodcastPeople" pp JOIN "Persons" pe ON pe.personid = pp.personid
               WHERE pp.podcastid = $1"#
        ),
        [podcast_id],
        |r| raw_credit!(r, None)
    );
    raws.extend(fetch_all!(
        db,
        format!(
            r#"SELECT {PERSON_COLS}, ep.episodeid AS episodeid, ep.role AS role, ep.persongroup AS persongroup, ep.source AS source, ep.verified AS verified
               FROM "EpisodePeople" ep
               JOIN "Episodes" e ON e.episodeid = ep.episodeid
               JOIN "Persons" pe ON pe.personid = ep.personid
               WHERE e.podcastid = $1"#
        ),
        [podcast_id],
        |r| raw_credit!(r, Some(r.int("episodeid")? as i32))
    ));
    Ok(raws)
}

/// Split a podcast's credits into regulars and guests. A regular is credited on the show itself,
/// or is a host/co-host on enough of the tagged episodes (feeds that only tag items).
fn split_regulars(raws: Vec<RawCredit>, followed: &HashSet<i32>) -> PodcastCredits {
    let tagged: HashSet<i32> = raws.iter().filter_map(|c| c.episode_id).collect();
    let threshold = ((tagged.len() as f64 * 0.2).ceil() as usize).max(2).min(tagged.len().max(1));
    let mut people = Vec::new();
    let mut guests = Vec::new();
    for agg in aggregate(raws) {
        if agg.podcast_level || agg.host_episodes.len() >= threshold {
            people.push((agg.podcast_level, credited(&agg, "podcast", followed)));
        } else {
            guests.push(credited(&agg, "episodes", followed));
        }
    }
    people.sort_by(|(a_level, a), (b_level, b)| {
        b_level
            .cmp(a_level)
            .then_with(|| role_rank(&a.role).cmp(&role_rank(&b.role)))
            .then_with(|| b.episode_count.cmp(&a.episode_count))
            .then_with(|| a.name.cmp(&b.name))
    });
    guests.sort_by(|a, b| b.episode_count.cmp(&a.episode_count).then_with(|| a.name.cmp(&b.name)));
    PodcastCredits { people: people.into_iter().map(|(_, p)| p).collect(), guests }
}

/// Everyone credited on the user's podcast.
pub async fn podcast_credits(db: &DatabasePool, podcast_id: i32, user_id: i32) -> AppResult<PodcastCredits> {
    owned_podcast(db, podcast_id, user_id).await?;
    let raws = podcast_raw_credits(db, podcast_id).await?;
    let followed = followed_ids(db, user_id).await?;
    Ok(split_regulars(raws, &followed))
}

/// People on one of the user's episodes: the episode's own credits, plus the show's regulars
/// unless the episode names its own hosts.
pub async fn episode_credits(db: &DatabasePool, episode_id: i32, user_id: i32) -> AppResult<Vec<CreditedPerson>> {
    let podcast_id = owned_episode_podcast(db, episode_id, user_id).await?;
    let followed = followed_ids(db, user_id).await?;
    let own = fetch_all!(
        db,
        format!(
            r#"SELECT {PERSON_COLS}, ep.role AS role, ep.persongroup AS persongroup, ep.source AS source, ep.verified AS verified
               FROM "EpisodePeople" ep JOIN "Persons" pe ON pe.personid = ep.personid
               WHERE ep.episodeid = $1"#
        ),
        [episode_id],
        |r| raw_credit!(r, Some(episode_id))
    );
    let names_hosts = own.iter().any(|c| is_host_role(&c.role));
    let mut out: Vec<CreditedPerson> = aggregate(own).iter().map(|a| credited(a, "episode", &followed)).collect();
    if !names_hosts {
        let show = split_regulars(podcast_raw_credits(db, podcast_id).await?, &followed);
        let have: HashSet<i32> = out.iter().map(|p| p.id).collect();
        let regulars: Vec<CreditedPerson> = show.people.into_iter().filter(|p| !have.contains(&p.id)).collect();
        out.splice(0..0, regulars);
    }
    out.sort_by_key(|p| role_rank(&p.role).0);
    Ok(out)
}

pub async fn person(db: &DatabasePool, person_id: i32, user_id: i32) -> AppResult<Person> {
    let followed = followed_ids(db, user_id).await?;
    fetch_optional!(
        db,
        r#"SELECT personid AS personid, podpeopleid AS podpeopleid, name AS name, img AS img, href AS href FROM "Persons" WHERE personid = $1"#,
        [person_id],
        |r| Person {
            id: r.int("personid")? as i32,
            podpeople_id: r.opt_int("podpeopleid")?.map(|v| v as i32),
            name: r.str("name")?,
            img: r.opt_str("img")?,
            href: r.opt_str("href")?,
            followed: followed.contains(&person_id),
        }
    )
    .ok_or_else(|| AppError::not_found("Person not found"))
}

/// Find people by name (for linking a name with no id, e.g. from an older client).
pub async fn find_by_name(db: &DatabasePool, name: &str) -> AppResult<Option<i32>> {
    let norm = normalize_name(name);
    if norm.is_empty() {
        return Ok(None);
    }
    Ok(persons_by_norm(db, &[norm]).await?.first().map(|p| p.id))
}

/// Search the server's people by name (prefix or substring of the normalized name).
pub async fn search(db: &DatabasePool, query: &str, user_id: i32, limit: i64) -> AppResult<Vec<(Person, i64)>> {
    let norm = normalize_name(query);
    if norm.is_empty() {
        return Ok(Vec::new());
    }
    let followed = followed_ids(db, user_id).await?;
    let pattern = format!("%{}%", norm.replace('%', "").replace('_', ""));
    let prefix = format!("{}%", norm.replace('%', "").replace('_', ""));
    Ok(fetch_all!(
        db,
        r#"SELECT pe.personid AS personid, pe.podpeopleid AS podpeopleid, pe.name AS name, pe.img AS img, pe.href AS href,
                  (SELECT COUNT(*) FROM "PodcastPeople" pp WHERE pp.personid = pe.personid) AS credits
           FROM "Persons" pe
           WHERE pe.namenormalized LIKE $1
           ORDER BY CASE WHEN pe.namenormalized LIKE $2 THEN 0 ELSE 1 END, pe.name
           LIMIT $3"#,
        [pattern, prefix, limit],
        |r| {
            let id = r.int("personid")? as i32;
            (
                Person {
                    id,
                    podpeople_id: r.opt_int("podpeopleid")?.map(|v| v as i32),
                    name: r.str("name")?,
                    img: r.opt_str("img")?,
                    href: r.opt_str("href")?,
                    followed: followed.contains(&id),
                },
                r.int("credits")?,
            )
        }
    ))
}

/// The user's podcasts this person is credited on.
pub async fn library_podcasts(db: &DatabasePool, person_id: i32, user_id: i32) -> AppResult<Vec<PersonPodcast>> {
    struct Row {
        podcast_id: i32,
        name: String,
        artwork: Option<String>,
        feed: String,
        index_id: Option<i64>,
        role: String,
        episode_id: Option<i32>,
    }
    let cols = "p.podcastid AS podcastid, p.podcastname AS podcastname, p.artworkurl AS artworkurl, p.feedurl AS feedurl, p.podcastindexid AS podcastindexid";
    let map = |r: Row, acc: &mut Vec<(i32, PersonPodcast, BTreeSet<(u8, String)>, HashSet<i32>)>| {
        let idx = match acc.iter().position(|(id, ..)| *id == r.podcast_id) {
            Some(i) => i,
            None => {
                acc.push((
                    r.podcast_id,
                    PersonPodcast {
                        podcastid: Some(r.podcast_id),
                        podcastname: r.name,
                        artworkurl: r.artwork,
                        feedurl: r.feed,
                        podcastindexid: r.index_id.filter(|v| *v > 0),
                        roles: Vec::new(),
                        podcast_level: false,
                        episode_count: 0,
                    },
                    BTreeSet::new(),
                    HashSet::new(),
                ));
                acc.len() - 1
            }
        };
        let entry = &mut acc[idx];
        entry.2.insert(role_rank(&r.role));
        match r.episode_id {
            None => entry.1.podcast_level = true,
            Some(e) => {
                entry.3.insert(e);
            }
        }
    };
    let mut acc = Vec::new();
    for r in fetch_all!(
        db,
        format!(
            r#"SELECT {cols}, pp.role AS role FROM "PodcastPeople" pp JOIN "Podcasts" p ON p.podcastid = pp.podcastid
               WHERE pp.personid = $1 AND p.userid = $2"#
        ),
        [person_id, user_id],
        |r| Row {
            podcast_id: r.int("podcastid")? as i32,
            name: r.str("podcastname")?,
            artwork: r.opt_str("artworkurl")?,
            feed: r.str("feedurl")?,
            index_id: r.opt_int("podcastindexid")?,
            role: r.str("role")?,
            episode_id: None,
        }
    ) {
        map(r, &mut acc);
    }
    for r in fetch_all!(
        db,
        format!(
            r#"SELECT {cols}, ep.role AS role, ep.episodeid AS episodeid FROM "EpisodePeople" ep
               JOIN "Episodes" e ON e.episodeid = ep.episodeid
               JOIN "Podcasts" p ON p.podcastid = e.podcastid
               WHERE ep.personid = $1 AND p.userid = $2"#
        ),
        [person_id, user_id],
        |r| Row {
            podcast_id: r.int("podcastid")? as i32,
            name: r.str("podcastname")?,
            artwork: r.opt_str("artworkurl")?,
            feed: r.str("feedurl")?,
            index_id: r.opt_int("podcastindexid")?,
            role: r.str("role")?,
            episode_id: Some(r.int("episodeid")? as i32),
        }
    ) {
        map(r, &mut acc);
    }
    let mut out: Vec<PersonPodcast> = acc
        .into_iter()
        .map(|(_, mut p, roles, eps)| {
            p.roles = roles.into_iter().map(|(_, r)| r).collect();
            p.episode_count = eps.len() as i64;
            p
        })
        .collect();
    out.sort_by(|a, b| b.podcast_level.cmp(&a.podcast_level).then_with(|| b.episode_count.cmp(&a.episode_count)));
    Ok(out)
}

/// Library episodes the person is credited on directly, or that belong to a show they host,
/// newest first.
pub async fn library_episodes(
    db: &DatabasePool,
    person_id: i32,
    user_id: i32,
    limit: i64,
    offset: i64,
) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
    // The one column named differently per backend.
    let is_video = match db {
        DatabasePool::Postgres(_) => "e.is_video",
        DatabasePool::MySQL(_) => "e.IsVideo",
    };
    Ok(fetch_all!(
        db,
        format!(r#"SELECT e.episodeid AS episodeid, p.podcastid AS podcastid, p.podcastname AS podcastname,
                  e.episodetitle AS episodetitle, e.episodepubdate AS episodepubdate,
                  e.episodedescription AS episodedescription,
                  CASE WHEN p.usepodcastcoverscustomized = TRUE AND p.usepodcastcovers = TRUE THEN p.artworkurl
                       WHEN u.usepodcastcovers = TRUE THEN p.artworkurl
                       WHEN e.episodeartwork IS NULL OR e.episodeartwork = '' THEN p.artworkurl
                       ELSE e.episodeartwork END AS episodeartwork,
                  e.episodeurl AS episodeurl, e.episodeduration AS episodeduration,
                  e.completed AS completed, {is_video} AS is_video, h.listenduration AS listenduration,
                  CASE WHEN s.episodeid IS NULL THEN 0 ELSE 1 END AS saved,
                  CASE WHEN q.episodeid IS NULL THEN 0 ELSE 1 END AS queued,
                  CASE WHEN d.episodeid IS NULL THEN 0 ELSE 1 END AS downloaded
           FROM "Episodes" e
           JOIN "Podcasts" p ON p.podcastid = e.podcastid
           LEFT JOIN "Users" u ON u.userid = p.userid
           LEFT JOIN "UserEpisodeHistory" h ON h.episodeid = e.episodeid AND h.userid = $1
           LEFT JOIN "SavedEpisodes" s ON s.episodeid = e.episodeid AND s.userid = $2
           LEFT JOIN "EpisodeQueue" q ON q.episodeid = e.episodeid AND q.userid = $3 AND q.is_youtube = FALSE
           LEFT JOIN "DownloadedEpisodes" d ON d.episodeid = e.episodeid AND d.userid = $4
           WHERE p.userid = $5 AND (
               e.episodeid IN (SELECT ep.episodeid FROM "EpisodePeople" ep WHERE ep.personid = $6)
               OR e.podcastid IN (SELECT pp.podcastid FROM "PodcastPeople" pp WHERE pp.personid = $7 AND pp.role IN ('host', 'co-host'))
           )
           ORDER BY e.episodepubdate DESC
           LIMIT $8 OFFSET $9"#),
        [user_id, user_id, user_id, user_id, user_id, person_id, person_id, limit, offset],
        |r| crate::handlers::podcasts::Episode {
            podcastid: r.int("podcastid")? as i32,
            podcastname: r.str("podcastname")?,
            episodetitle: r.str("episodetitle")?,
            episodepubdate: r.ts("episodepubdate")?.map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string()).unwrap_or_default(),
            episodedescription: r.str("episodedescription")?,
            episodeartwork: r.str("episodeartwork")?,
            episodeurl: r.str("episodeurl")?,
            episodeduration: r.int("episodeduration")? as i32,
            listenduration: r.opt_int("listenduration")?.map(|v| v as i32),
            episodeid: r.int("episodeid")? as i32,
            completed: r.flag("completed")?,
            saved: r.flag("saved")?,
            queued: r.flag("queued")?,
            downloaded: r.flag("downloaded")?,
            is_youtube: false,
            is_video: r.flag("is_video")?,
        }
    ))
}

pub async fn follow(db: &DatabasePool, user_id: i32, person_id: i32) -> AppResult<()> {
    fetch_optional!(db, r#"SELECT personid AS personid FROM "Persons" WHERE personid = $1"#, [person_id], |r| r.int("personid")?)
        .ok_or_else(|| AppError::not_found("Person not found"))?;
    let exists = fetch_optional!(
        db,
        r#"SELECT personid AS personid FROM "PersonFollows" WHERE userid = $1 AND personid = $2"#,
        [user_id, person_id],
        |r| r.int("personid")?
    );
    if exists.is_none() {
        // A concurrent follow can race us to the primary key; that's the outcome we want anyway.
        let res: AppResult<u64> = async {
            Ok(execute!(db, r#"INSERT INTO "PersonFollows" (userid, personid) VALUES ($1, $2)"#, [user_id, person_id]))
        }
        .await;
        if let Err(e) = res {
            if fetch_optional!(
                db,
                r#"SELECT personid AS personid FROM "PersonFollows" WHERE userid = $1 AND personid = $2"#,
                [user_id, person_id],
                |r| r.int("personid")?
            )
            .is_none()
            {
                return Err(e);
            }
        }
    }
    Ok(())
}

pub async fn unfollow(db: &DatabasePool, user_id: i32, person_id: i32) -> AppResult<bool> {
    Ok(execute!(db, r#"DELETE FROM "PersonFollows" WHERE userid = $1 AND personid = $2"#, [user_id, person_id]) > 0)
}

/// People the user follows, with how many of their library episodes credit each.
pub async fn followed(db: &DatabasePool, user_id: i32) -> AppResult<Vec<Person>> {
    Ok(fetch_all!(
        db,
        r#"SELECT pe.personid AS personid, pe.podpeopleid AS podpeopleid, pe.name AS name, pe.img AS img, pe.href AS href
           FROM "PersonFollows" f JOIN "Persons" pe ON pe.personid = f.personid
           WHERE f.userid = $1 ORDER BY pe.name"#,
        [user_id],
        |r| Person {
            id: r.int("personid")? as i32,
            podpeople_id: r.opt_int("podpeopleid")?.map(|v| v as i32),
            name: r.str("name")?,
            img: r.opt_str("img")?,
            href: r.opt_str("href")?,
            followed: true,
        }
    ))
}

/// Stored channel extras (podroll, funding, value) for the user's podcast.
pub async fn channel_extras(db: &DatabasePool, podcast_id: i32) -> AppResult<crate::services::podcasting2::ChannelExtras> {
    let raw = fetch_optional!(
        db,
        r#"SELECT podcasting2data AS data FROM "Podcasts" WHERE podcastid = $1"#,
        [podcast_id],
        |r| r.opt_str("data")?
    )
    .flatten();
    Ok(raw.and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default())
}

/// Stored chapters URL and transcripts for an episode.
pub async fn episode_extras(
    db: &DatabasePool,
    episode_id: i32,
) -> AppResult<(Option<String>, Vec<crate::services::podcasting2::Transcript>)> {
    let row = fetch_optional!(
        db,
        r#"SELECT chaptersurl AS chapters, transcriptsjson AS transcripts FROM "Episodes" WHERE episodeid = $1"#,
        [episode_id],
        |r| (r.opt_str("chapters")?, r.opt_str("transcripts")?)
    );
    let (chapters, transcripts) = row.unwrap_or((None, None));
    Ok((chapters, transcripts.and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()))
}

/// Podcasts whose feed tags have never been stored (added before this version, or never
/// refreshed since), for the one-time startup backfill.
pub async fn podcasts_missing_tags(db: &DatabasePool) -> AppResult<Vec<(i32, String, Option<String>, Option<String>)>> {
    Ok(fetch_all!(
        db,
        r#"SELECT podcastid AS podcastid, feedurl AS feedurl, username AS username, password AS password
           FROM "Podcasts"
           WHERE podcasting2hash IS NULL AND COALESCE(isyoutubechannel, FALSE) = FALSE AND feedurl LIKE 'http%'"#,
        [],
        |r| (r.int("podcastid")? as i32, r.str("feedurl")?, r.opt_str("username")?, r.opt_str("password")?)
    ))
}
