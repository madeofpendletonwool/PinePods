// SQL for the sync core, per backend. Event writes take a connection so they can run inside the
// caller's transaction; everything else is a DatabasePool method.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{MySqlConnection, PgConnection, Row};

use super::model::{ActionKind, EpisodeAction};
use super::resolver::ResolveKey;
use super::{PINEPODS_DEVICE_CAPTION, PINEPODS_DEVICE_NAME, PINEPODS_DEVICE_TYPE, PINEPODS_ROLE};
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};

// Receive time from the DB clock, evaluated once per statement. PG's NOW() is the transaction
// start, so CLOCK_TIMESTAMP(); FLOOR because a plain ::bigint cast rounds to nearest.
const PG_NOW: &str = "(SELECT FLOOR(EXTRACT(EPOCH FROM CLOCK_TIMESTAMP()))::BIGINT AS t) n";

// Rows per resolver query; MySQL binds 7 values per key and caps a statement at 65535.
const RESOLVE_CHUNK: usize = 500;

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Local state of one episode for one user, as apply needs it.
#[derive(Debug, Clone)]
pub struct LocalEpisodeState {
    pub duration: Option<i32>,
    pub completed: bool,
    pub position: Option<i32>,
    /// `UserEpisodeHistory.ListenDate`: last progress/completion change (stored as UTC).
    pub listen_date: Option<DateTime<Utc>>,
}

/// What an episode event needs to know about the episode.
#[derive(Debug, Clone)]
pub struct EpisodeForEvent {
    pub feed_url: String,
    pub episode_url: String,
    pub guid: Option<String>,
    pub duration: Option<i32>,
    pub shareable: bool,
}

/// An episode-level log event.
#[derive(Debug, Clone, Copy)]
pub enum EpisodeEvent {
    /// Playback position in seconds.
    Progress(i32),
    Completed,
    New,
}

/// One unmatched action in the log.
#[derive(Debug, Clone)]
pub struct LoggedAction {
    pub action_id: i64,
    pub action: EpisodeAction,
}

fn device_name_candidates(user_id: i32) -> [String; 3] {
    [PINEPODS_DEVICE_NAME.to_string(), "pinepods-server".to_string(), format!("pinepods-{user_id}")]
}

fn legacy_device_name(user_id: i32) -> String {
    format!("pinepods-internal-{user_id}")
}

// ---------------------------------------------------------------------------------------------
// Events (inside the caller's transaction)
// ---------------------------------------------------------------------------------------------

/// The user's sync targets, share-locked by a log writer.
#[derive(Debug, Clone, Copy, Default)]
pub struct LockedTargets {
    /// The highest ActionsPushCursor (0 without targets).
    pub max_push_cursor: i64,
    /// An enabled target exists, so a new row is waiting to be pushed.
    pub any_enabled: bool,
}

/// Share-lock the user's sync targets until the caller commits. Every writer to the log calls
/// this first: the external sync engine advances a push cursor under an exclusive lock on the
/// same rows, after re-reading what it sent, so a row written or rewritten while it pushed is
/// never skipped.
pub async fn pg_lock_targets_shared(conn: &mut PgConnection, user_id: i32) -> AppResult<LockedTargets> {
    let rows: Vec<(i64, bool)> = sqlx::query_as(r#"SELECT actionspushcursor, enabled FROM "SyncTargets" WHERE userid = $1 FOR SHARE"#)
        .bind(user_id)
        .fetch_all(&mut *conn)
        .await?;
    Ok(locked(rows))
}

pub async fn mysql_lock_targets_shared(conn: &mut MySqlConnection, user_id: i32) -> AppResult<LockedTargets> {
    let rows: Vec<(i64, bool)> = sqlx::query_as("SELECT ActionsPushCursor, Enabled FROM SyncTargets WHERE UserID = ? LOCK IN SHARE MODE")
        .bind(user_id)
        .fetch_all(&mut *conn)
        .await?;
    Ok(locked(rows))
}

fn locked(rows: Vec<(i64, bool)>) -> LockedTargets {
    LockedTargets {
        max_push_cursor: rows.iter().map(|r| r.0).max().unwrap_or(0),
        any_enabled: rows.iter().any(|r| r.1),
    }
}

/// The user's pinepods device, created on first use. The pre-overhaul internal device
/// (`pinepods-internal-<uid>`) is claimed if nothing has the role yet, so Pinepods keeps one
/// device. A client may already own the name `pinepods` (UNIQUE(UserID, DeviceName)), so other
/// names are tried after it.
pub async fn pg_pinepods_device(conn: &mut PgConnection, user_id: i32) -> AppResult<i32> {
    let by_role = r#"SELECT deviceid FROM "GpodderDevices" WHERE userid = $1 AND systemrole = 'pinepods'"#;
    if let Some(id) = sqlx::query_scalar::<_, i32>(by_role).bind(user_id).fetch_optional(&mut *conn).await? {
        return Ok(id);
    }
    let claimed: Option<i32> = sqlx::query_scalar(
        r#"UPDATE "GpodderDevices" SET systemrole = 'pinepods'
           WHERE userid = $1 AND devicename = $2 AND systemrole IS NULL
             AND NOT EXISTS (SELECT 1 FROM "GpodderDevices" d WHERE d.userid = $1 AND d.systemrole = 'pinepods')
           RETURNING deviceid"#,
    )
    .bind(user_id)
    .bind(legacy_device_name(user_id))
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(id) = claimed {
        return Ok(id);
    }
    for name in device_name_candidates(user_id) {
        let created: Option<i32> = sqlx::query_scalar(
            r#"INSERT INTO "GpodderDevices" (userid, devicename, devicetype, devicecaption, isdefault, isactive, systemrole)
               VALUES ($1, $2, $3, $4, FALSE, TRUE, $5)
               ON CONFLICT DO NOTHING
               RETURNING deviceid"#,
        )
        .bind(user_id)
        .bind(&name)
        .bind(PINEPODS_DEVICE_TYPE)
        .bind(PINEPODS_DEVICE_CAPTION)
        .bind(PINEPODS_ROLE)
        .fetch_optional(&mut *conn)
        .await?;
        if let Some(id) = created {
            return Ok(id);
        }
        // Either someone else just created the role device, or a client owns this name.
        if let Some(id) = sqlx::query_scalar::<_, i32>(by_role).bind(user_id).fetch_optional(&mut *conn).await? {
            return Ok(id);
        }
    }
    Err(AppError::internal(&format!("no free device name for user {user_id}'s pinepods device")))
}

pub async fn mysql_pinepods_device(conn: &mut MySqlConnection, user_id: i32) -> AppResult<i32> {
    let by_role = "SELECT DeviceID FROM GpodderDevices WHERE UserID = ? AND SystemRole = 'pinepods'";
    if let Some(id) = sqlx::query_scalar::<_, i32>(by_role).bind(user_id).fetch_optional(&mut *conn).await? {
        return Ok(id);
    }
    let legacy: Option<i32> = sqlx::query_scalar(
        "SELECT DeviceID FROM GpodderDevices WHERE UserID = ? AND DeviceName = ? AND SystemRole IS NULL",
    )
    .bind(user_id)
    .bind(legacy_device_name(user_id))
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(id) = legacy {
        // The unique (UserID, SystemRole) index makes a racing claim fail instead of doubling up.
        let claimed = sqlx::query("UPDATE IGNORE GpodderDevices SET SystemRole = 'pinepods' WHERE DeviceID = ? AND SystemRole IS NULL")
            .bind(id)
            .execute(&mut *conn)
            .await?;
        if claimed.rows_affected() == 1 {
            return Ok(id);
        }
    }
    for name in device_name_candidates(user_id) {
        let created = sqlx::query(
            "INSERT IGNORE INTO GpodderDevices (UserID, DeviceName, DeviceType, DeviceCaption, IsDefault, IsActive, SystemRole)
             VALUES (?, ?, ?, ?, FALSE, TRUE, ?)",
        )
        .bind(user_id)
        .bind(&name)
        .bind(PINEPODS_DEVICE_TYPE)
        .bind(PINEPODS_DEVICE_CAPTION)
        .bind(PINEPODS_ROLE)
        .execute(&mut *conn)
        .await?;
        if created.rows_affected() == 1 {
            return Ok(created.last_insert_id() as i32);
        }
        if let Some(id) = sqlx::query_scalar::<_, i32>(by_role).bind(user_id).fetch_optional(&mut *conn).await? {
            return Ok(id);
        }
    }
    Err(AppError::internal(&format!("no free device name for user {user_id}'s pinepods device")))
}

pub async fn pg_log_subscription(conn: &mut PgConnection, user_id: i32, feed_url: &str, action: &str) -> AppResult<bool> {
    let targets = pg_lock_targets_shared(conn, user_id).await?;
    let device_id = pg_pinepods_device(conn, user_id).await?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"INSERT INTO "GpodderSyncSubscriptions" (userid, deviceid, podcasturl, action, timestamp, serverts, origin)
           SELECT $1, $2, $3, $4, n.t, n.t, 'pinepods' FROM {PG_NOW}"#
    )))
    .bind(user_id)
    .bind(device_id)
    .bind(feed_url)
    .bind(action)
    .execute(&mut *conn)
    .await?;
    Ok(targets.any_enabled)
}

pub async fn mysql_log_subscription(conn: &mut MySqlConnection, user_id: i32, feed_url: &str, action: &str) -> AppResult<bool> {
    let targets = mysql_lock_targets_shared(conn, user_id).await?;
    let device_id = mysql_pinepods_device(conn, user_id).await?;
    sqlx::query(
        "INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp, ServerTs, Origin)
         VALUES (?, ?, ?, ?, UNIX_TIMESTAMP(), UNIX_TIMESTAMP(), 'pinepods')",
    )
    .bind(user_id)
    .bind(device_id)
    .bind(feed_url)
    .bind(action)
    .execute(&mut *conn)
    .await?;
    Ok(targets.any_enabled)
}

pub async fn pg_episode_for_event(conn: &mut PgConnection, user_id: i32, episode_id: i32) -> AppResult<Option<EpisodeForEvent>> {
    let row = sqlx::query(
        r#"SELECT p.feedurl, p.username, p.password, COALESCE(p.isyoutubechannel, FALSE) AS yt,
                  e.episodeurl, e.episodeguid, e.episodeduration
           FROM "Episodes" e JOIN "Podcasts" p ON p.podcastid = e.podcastid
           WHERE e.episodeid = $1 AND p.userid = $2"#,
    )
    .bind(episode_id)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else { return Ok(None) };
    let feed_url: String = row.try_get::<Option<String>, _>("feedurl")?.unwrap_or_default();
    let username: Option<String> = row.try_get("username")?;
    let password: Option<String> = row.try_get("password")?;
    let yt: bool = row.try_get("yt")?;
    Ok(Some(EpisodeForEvent {
        shareable: super::shareable_feed(&feed_url, username.as_deref(), password.as_deref(), yt),
        feed_url,
        episode_url: row.try_get::<Option<String>, _>("episodeurl")?.unwrap_or_default(),
        guid: row.try_get::<Option<String>, _>("episodeguid")?.filter(|g| !g.is_empty()),
        duration: row.try_get("episodeduration")?,
    }))
}

pub async fn mysql_episode_for_event(conn: &mut MySqlConnection, user_id: i32, episode_id: i32) -> AppResult<Option<EpisodeForEvent>> {
    let row = sqlx::query(
        "SELECT p.FeedURL, p.Username, p.Password, p.IsYouTubeChannel AS yt,
                e.EpisodeURL, e.EpisodeGUID, e.EpisodeDuration
         FROM Episodes e JOIN Podcasts p ON p.PodcastID = e.PodcastID
         WHERE e.EpisodeID = ? AND p.UserID = ?",
    )
    .bind(episode_id)
    .bind(user_id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(row) = row else { return Ok(None) };
    let feed_url: String = row.try_get::<Option<String>, _>("FeedURL")?.unwrap_or_default();
    let username: Option<String> = row.try_get("Username")?;
    let password: Option<String> = row.try_get("Password")?;
    let yt: bool = row.try_get::<Option<bool>, _>("yt")?.unwrap_or(false);
    Ok(Some(EpisodeForEvent {
        shareable: super::shareable_feed(&feed_url, username.as_deref(), password.as_deref(), yt),
        feed_url,
        episode_url: row.try_get::<Option<String>, _>("EpisodeURL")?.unwrap_or_default(),
        guid: row.try_get::<Option<String>, _>("EpisodeGUID")?.filter(|g| !g.is_empty()),
        duration: row.try_get("EpisodeDuration")?,
    }))
}

/// `(position, total)` for a play event, or None when there's nothing worth sending.
fn play_values(ep: &EpisodeForEvent, event: EpisodeEvent) -> Option<(i32, i32)> {
    let duration = ep.duration.filter(|d| *d > 0);
    let position = match event {
        EpisodeEvent::Progress(p) => p,
        EpisodeEvent::Completed => duration?,
        EpisodeEvent::New => return None,
    };
    (position > 0).then(|| (position, duration.unwrap_or(position).max(position)))
}

/// Write one episode event. Progress and completion update the latest unpushed `pinepods` play
/// row for the episode (coalescing: playback saves every few seconds) and bump its timestamps so
/// cursor readers see it again; otherwise a new row is inserted. "Unpushed" means above every
/// target's ActionsPushCursor, so an external target never misses a position.
pub async fn pg_log_episode(conn: &mut PgConnection, user_id: i32, episode_id: i32, ep: &EpisodeForEvent, event: EpisodeEvent) -> AppResult<bool> {
    let targets = pg_lock_targets_shared(conn, user_id).await?;
    let pushed = targets.max_push_cursor;
    let device_id = pg_pinepods_device(conn, user_id).await?;
    if let EpisodeEvent::New = event {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"INSERT INTO "GpodderSyncEpisodeActions"
                 (userid, deviceid, podcasturl, episodeurl, action, timestamp, serverts, guid, episodeid, origin)
               SELECT $1, $2, $3, $4, 'new', n.t, n.t, $5, $6, 'pinepods' FROM {PG_NOW}"#
        )))
        .bind(user_id)
        .bind(device_id)
        .bind(&ep.feed_url)
        .bind(&ep.episode_url)
        .bind(&ep.guid)
        .bind(episode_id)
        .execute(&mut *conn)
        .await?;
        return Ok(targets.any_enabled);
    }
    let Some((position, total)) = play_values(ep, event) else { return Ok(false) };
    let latest = sqlx::query(
        r#"SELECT a.actionid, a.action FROM "GpodderSyncEpisodeActions" a
           WHERE a.episodeid = $1 AND a.userid = $2 AND a.origin = 'pinepods' AND a.actionid > $3
           ORDER BY a.actionid DESC LIMIT 1"#,
    )
    .bind(episode_id)
    .bind(user_id)
    .bind(pushed)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = latest {
        let action: String = row.try_get("action")?;
        if action == "play" {
            let action_id: i32 = row.try_get("actionid")?;
            sqlx::query(sqlx::AssertSqlSafe(format!(
                r#"UPDATE "GpodderSyncEpisodeActions" a
                   SET started = 0, position = $1, total = $2, timestamp = n.t, serverts = n.t,
                       deviceid = $3, podcasturl = $4, episodeurl = $5, guid = $6
                   FROM {PG_NOW} WHERE a.actionid = $7"#
            )))
            .bind(position)
            .bind(total)
            .bind(device_id)
            .bind(&ep.feed_url)
            .bind(&ep.episode_url)
            .bind(&ep.guid)
            .bind(action_id)
            .execute(&mut *conn)
            .await?;
            return Ok(targets.any_enabled);
        }
    }
    sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"INSERT INTO "GpodderSyncEpisodeActions"
             (userid, deviceid, podcasturl, episodeurl, action, timestamp, serverts, started, position, total, guid, episodeid, origin)
           SELECT $1, $2, $3, $4, 'play', n.t, n.t, 0, $5, $6, $7, $8, 'pinepods' FROM {PG_NOW}"#
    )))
    .bind(user_id)
    .bind(device_id)
    .bind(&ep.feed_url)
    .bind(&ep.episode_url)
    .bind(position)
    .bind(total)
    .bind(&ep.guid)
    .bind(episode_id)
    .execute(&mut *conn)
    .await?;
    Ok(targets.any_enabled)
}

pub async fn mysql_log_episode(conn: &mut MySqlConnection, user_id: i32, episode_id: i32, ep: &EpisodeForEvent, event: EpisodeEvent) -> AppResult<bool> {
    let targets = mysql_lock_targets_shared(conn, user_id).await?;
    let pushed = targets.max_push_cursor;
    let device_id = mysql_pinepods_device(conn, user_id).await?;
    if let EpisodeEvent::New = event {
        sqlx::query(
            "INSERT INTO GpodderSyncEpisodeActions
               (UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Guid, EpisodeID, Origin)
             VALUES (?, ?, ?, ?, 'new', UNIX_TIMESTAMP(), UNIX_TIMESTAMP(), ?, ?, 'pinepods')",
        )
        .bind(user_id)
        .bind(device_id)
        .bind(&ep.feed_url)
        .bind(&ep.episode_url)
        .bind(&ep.guid)
        .bind(episode_id)
        .execute(&mut *conn)
        .await?;
        return Ok(targets.any_enabled);
    }
    let Some((position, total)) = play_values(ep, event) else { return Ok(false) };
    let latest = sqlx::query(
        "SELECT a.ActionID, a.Action FROM GpodderSyncEpisodeActions a
         WHERE a.EpisodeID = ? AND a.UserID = ? AND a.Origin = 'pinepods' AND a.ActionID > ?
         ORDER BY a.ActionID DESC LIMIT 1",
    )
    .bind(episode_id)
    .bind(user_id)
    .bind(pushed)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some(row) = latest {
        let action: String = row.try_get("Action")?;
        if action == "play" {
            let action_id: i32 = row.try_get("ActionID")?;
            sqlx::query(
                "UPDATE GpodderSyncEpisodeActions
                 SET Started = 0, Position = ?, Total = ?, Timestamp = UNIX_TIMESTAMP(), ServerTs = UNIX_TIMESTAMP(),
                     DeviceID = ?, PodcastURL = ?, EpisodeURL = ?, Guid = ?
                 WHERE ActionID = ?",
            )
            .bind(position)
            .bind(total)
            .bind(device_id)
            .bind(&ep.feed_url)
            .bind(&ep.episode_url)
            .bind(&ep.guid)
            .bind(action_id)
            .execute(&mut *conn)
            .await?;
            return Ok(targets.any_enabled);
        }
    }
    sqlx::query(
        "INSERT INTO GpodderSyncEpisodeActions
           (UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Started, Position, Total, Guid, EpisodeID, Origin)
         VALUES (?, ?, ?, ?, 'play', UNIX_TIMESTAMP(), UNIX_TIMESTAMP(), 0, ?, ?, ?, ?, 'pinepods')",
    )
    .bind(user_id)
    .bind(device_id)
    .bind(&ep.feed_url)
    .bind(&ep.episode_url)
    .bind(position)
    .bind(total)
    .bind(&ep.guid)
    .bind(episode_id)
    .execute(&mut *conn)
    .await?;
    Ok(targets.any_enabled)
}

// ---------------------------------------------------------------------------------------------
// Pool-level queries
// ---------------------------------------------------------------------------------------------

impl DatabasePool {
    /// Internal gpodder server on, or an enabled external target. See `sync::sync_enabled`.
    pub async fn sync_enabled_uncached(&self, user_id: i32) -> AppResult<bool> {
        let enabled: Option<bool> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(
                r#"SELECT u.gpodderserverenabled
                          OR EXISTS (SELECT 1 FROM "SyncTargets" t WHERE t.userid = u.userid AND t.enabled)
                   FROM "Users" u WHERE u.userid = $1"#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_scalar::<_, i64>(
                "SELECT (u.GpodderServerEnabled
                         OR EXISTS (SELECT 1 FROM SyncTargets t WHERE t.UserID = u.UserID AND t.Enabled)) + 0
                 FROM Users u WHERE u.UserID = ?",
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .map(|v| v != 0),
        };
        Ok(enabled.unwrap_or(false))
    }

    /// One resolver query per chunk of keys; see `sync::resolver` for the match order.
    pub async fn sync_resolve(&self, user_id: i32, keys: &[ResolveKey<'_>]) -> AppResult<Vec<Option<i32>>> {
        let mut out = vec![None; keys.len()];
        for (chunk_no, chunk) in keys.chunks(RESOLVE_CHUNK).enumerate() {
            let offset = chunk_no * RESOLVE_CHUNK;
            let found = match self {
                DatabasePool::Postgres(pool) => {
                    let purl: Vec<&str> = chunk.iter().map(|k| k.podcast_url).collect();
                    let eurl: Vec<&str> = chunk.iter().map(|k| k.episode_url).collect();
                    let guid: Vec<&str> = chunk.iter().map(|k| k.guid.unwrap_or("")).collect();
                    let base: Vec<&str> = chunk.iter().map(|k| super::resolver::url_base(k.episode_url)).collect();
                    let rows = sqlx::query(
                        r#"SELECT i.idx, COALESCE(
                             (SELECT MIN(e.episodeid) FROM "Episodes" e JOIN "Podcasts" p ON p.podcastid = e.podcastid
                               WHERE p.userid = $1 AND p.feedurl = i.purl AND i.guid <> '' AND e.episodeguid = i.guid),
                             (SELECT MIN(e.episodeid) FROM "Episodes" e JOIN "Podcasts" p ON p.podcastid = e.podcastid
                               WHERE p.userid = $1 AND e.episodeurl = i.eurl),
                             (SELECT MIN(e.episodeid) FROM "Episodes" e JOIN "Podcasts" p ON p.podcastid = e.podcastid
                               WHERE p.userid = $1 AND i.base <> i.eurl AND e.episodeurl = i.base),
                             (SELECT MIN(e.episodeid) FROM "Episodes" e JOIN "Podcasts" p ON p.podcastid = e.podcastid
                               WHERE p.userid = $1 AND p.feedurl = i.purl AND split_part(e.episodeurl, '?', 1) = i.base)
                           ) AS episode_id
                           FROM UNNEST($2::text[], $3::text[], $4::text[], $5::text[]) WITH ORDINALITY AS i(purl, eurl, guid, base, idx)"#,
                    )
                    .bind(user_id)
                    .bind(&purl)
                    .bind(&eurl)
                    .bind(&guid)
                    .bind(&base)
                    .fetch_all(pool)
                    .await?;
                    rows.iter()
                        .map(|r| Ok((r.try_get::<i64, _>("idx")? as usize - 1, r.try_get::<Option<i32>, _>("episode_id")?)))
                        .collect::<AppResult<Vec<_>>>()?
                }
                DatabasePool::MySQL(pool) => {
                    // No arrays: the keys become a derived table. Hashes are computed here so the
                    // lookups hit the EpisodeURLHash / EpisodeGUIDHash indexes ('' = no guid).
                    let row_sql = "SELECT ? AS idx, ? AS purl, ? AS eurl, ? AS base, ? AS ghash, ? AS uhash, ? AS bhash";
                    let derived = vec![row_sql; chunk.len()].join(" UNION ALL ");
                    let sql = format!(
                        "SELECT d.idx, COALESCE(
                           (SELECT MIN(e.EpisodeID) FROM Episodes e JOIN Podcasts p ON p.PodcastID = e.PodcastID
                             WHERE p.UserID = ? AND p.FeedURL = d.purl AND d.ghash <> '' AND e.EpisodeGUIDHash = d.ghash),
                           (SELECT MIN(e.EpisodeID) FROM Episodes e JOIN Podcasts p ON p.PodcastID = e.PodcastID
                             WHERE p.UserID = ? AND e.EpisodeURLHash = d.uhash AND e.EpisodeURL = d.eurl),
                           (SELECT MIN(e.EpisodeID) FROM Episodes e JOIN Podcasts p ON p.PodcastID = e.PodcastID
                             WHERE p.UserID = ? AND d.base <> d.eurl AND e.EpisodeURLHash = d.bhash AND e.EpisodeURL = d.base),
                           (SELECT MIN(e.EpisodeID) FROM Episodes e JOIN Podcasts p ON p.PodcastID = e.PodcastID
                             WHERE p.UserID = ? AND p.FeedURL = d.purl AND SUBSTRING_INDEX(e.EpisodeURL, '?', 1) = d.base)
                         ) AS episode_id
                         FROM ({derived}) d"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
                    for _ in 0..4 {
                        q = q.bind(user_id);
                    }
                    for (i, k) in chunk.iter().enumerate() {
                        let base = super::resolver::url_base(k.episode_url);
                        q = q
                            .bind(i as i64)
                            .bind(k.podcast_url)
                            .bind(k.episode_url)
                            .bind(base)
                            .bind(k.guid.map(sha256_hex).unwrap_or_default())
                            .bind(sha256_hex(k.episode_url))
                            .bind(sha256_hex(base));
                    }
                    let rows = q.fetch_all(pool).await?;
                    rows.iter()
                        .map(|r| Ok((r.try_get::<i64, _>("idx")? as usize, r.try_get::<Option<i32>, _>("episode_id")?)))
                        .collect::<AppResult<Vec<_>>>()?
                }
            };
            for (i, id) in found {
                if let Some(slot) = out.get_mut(offset + i) {
                    *slot = id;
                }
            }
        }
        Ok(out)
    }

    /// Local state of `episode_ids` for `user_id`; episodes that aren't the user's are left out.
    pub async fn sync_local_states(&self, user_id: i32, episode_ids: &[i32]) -> AppResult<HashMap<i32, LocalEpisodeState>> {
        let mut out = HashMap::new();
        if episode_ids.is_empty() {
            return Ok(out);
        }
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT e.episodeid, e.episodeduration, e.completed, h.listenduration, h.listendate
                       FROM "Episodes" e
                       JOIN "Podcasts" p ON p.podcastid = e.podcastid AND p.userid = $1
                       LEFT JOIN "UserEpisodeHistory" h ON h.episodeid = e.episodeid AND h.userid = $1
                       WHERE e.episodeid = ANY($2)"#,
                )
                .bind(user_id)
                .bind(episode_ids)
                .fetch_all(pool)
                .await?;
                for r in rows {
                    out.insert(
                        r.try_get("episodeid")?,
                        LocalEpisodeState {
                            duration: r.try_get("episodeduration")?,
                            completed: r.try_get::<Option<bool>, _>("completed")?.unwrap_or(false),
                            position: r.try_get("listenduration")?,
                            listen_date: r.try_get::<Option<NaiveDateTime>, _>("listendate")?.map(|d| d.and_utc()),
                        },
                    );
                }
            }
            DatabasePool::MySQL(pool) => {
                for chunk in episode_ids.chunks(1000) {
                    let marks = vec!["?"; chunk.len()].join(",");
                    let sql = format!(
                        "SELECT e.EpisodeID, e.EpisodeDuration, e.Completed, h.ListenDuration, h.ListenDate
                         FROM Episodes e
                         JOIN Podcasts p ON p.PodcastID = e.PodcastID AND p.UserID = ?
                         LEFT JOIN UserEpisodeHistory h ON h.EpisodeID = e.EpisodeID AND h.UserID = ?
                         WHERE e.EpisodeID IN ({marks})"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(user_id).bind(user_id);
                    for id in chunk {
                        q = q.bind(id);
                    }
                    for r in q.fetch_all(pool).await? {
                        out.insert(
                            r.try_get("EpisodeID")?,
                            LocalEpisodeState {
                                duration: r.try_get("EpisodeDuration")?,
                                completed: r.try_get::<Option<bool>, _>("Completed")?.unwrap_or(false),
                                position: r.try_get("ListenDuration")?,
                                listen_date: r.try_get::<Option<NaiveDateTime>, _>("ListenDate")?.map(|d| d.and_utc()),
                            },
                        );
                    }
                }
            }
        }
        Ok(out)
    }

    /// Set the listen position, backwards too (unlike `record_listen_duration`, which only moves
    /// forward). Used by apply after last-write-wins has decided the remote position is newer.
    pub async fn set_listen_position(&self, user_id: i32, episode_id: i32, position: i32, at: DateTime<Utc>, origin: super::SyncOrigin) -> AppResult<()> {
        let log = super::events::should_log(self, user_id, origin).await;
        let at = at.naive_utc();
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(
                    r#"INSERT INTO "UserEpisodeHistory" (userid, episodeid, listendate, listenduration)
                       VALUES ($1, $2, $3, $4)
                       ON CONFLICT (userid, episodeid) DO UPDATE SET listenduration = $4, listendate = $3"#,
                )
                .bind(user_id)
                .bind(episode_id)
                .bind(at)
                .bind(position)
                .execute(&mut *tx)
                .await?;
                if log {
                    super::events::episode_pg(self, &mut tx, user_id, episode_id, EpisodeEvent::Progress(position)).await;
                }
                tx.commit().await?;
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = super::events::begin_mysql(pool, user_id, log).await?;
                sqlx::query(
                    "INSERT INTO UserEpisodeHistory (UserID, EpisodeID, ListenDate, ListenDuration)
                     VALUES (?, ?, ?, ?)
                     ON DUPLICATE KEY UPDATE ListenDuration = VALUES(ListenDuration), ListenDate = VALUES(ListenDate)",
                )
                .bind(user_id)
                .bind(episode_id)
                .bind(at)
                .bind(position)
                .execute(&mut *tx)
                .await?;
                if log {
                    super::events::episode_mysql(self, &mut tx, user_id, episode_id, EpisodeEvent::Progress(position)).await;
                }
                tx.commit().await?;
            }
        }
        Ok(())
    }

    /// Set `UserEpisodeHistory.ListenDate` to the time of the change that was applied, so the
    /// next last-write-wins comparison uses the action's time, not the time we applied it.
    pub async fn sync_set_listen_date(&self, user_id: i32, episode_id: i32, at: DateTime<Utc>) -> AppResult<()> {
        let at = at.naive_utc();
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "UserEpisodeHistory" SET listendate = $1 WHERE userid = $2 AND episodeid = $3"#)
                    .bind(at)
                    .bind(user_id)
                    .bind(episode_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE UserEpisodeHistory SET ListenDate = ? WHERE UserID = ? AND EpisodeID = ?")
                    .bind(at)
                    .bind(user_id)
                    .bind(episode_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// Log actions pulled from an external target, as `origin` (`target:<id>`), so the internal
    /// server's clients see them and the push step can skip them. `episode_ids` is the
    /// resolver's answer per action (NULL = unmatched, rematched after ingest).
    pub async fn sync_log_remote_actions(&self, user_id: i32, actions: &[EpisodeAction], episode_ids: &[Option<i32>], origin: &str) -> AppResult<()> {
        if actions.is_empty() {
            return Ok(());
        }
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                pg_lock_targets_shared(&mut tx, user_id).await?;
                let now: i64 = sqlx::query_scalar("SELECT FLOOR(EXTRACT(EPOCH FROM CLOCK_TIMESTAMP()))::BIGINT")
                    .fetch_one(&mut *tx)
                    .await?;
                let col = |f: &dyn Fn(&EpisodeAction) -> Option<i32>| actions.iter().map(f).collect::<Vec<_>>();
                sqlx::query(
                    r#"INSERT INTO "GpodderSyncEpisodeActions"
                         (userid, deviceid, podcasturl, episodeurl, action, timestamp, serverts, started, position, total, guid, episodeid, origin)
                       SELECT $1, NULL, u.purl, u.eurl, u.action, u.ts, $2, u.started, u.position, u.total, u.guid, u.episodeid, $3
                       FROM UNNEST($4::text[], $5::text[], $6::text[], $7::bigint[], $8::int[], $9::int[], $10::int[], $11::text[], $12::int[])
                         AS u(purl, eurl, action, ts, started, position, total, guid, episodeid)"#,
                )
                .bind(user_id)
                .bind(now)
                .bind(origin)
                .bind(actions.iter().map(|a| a.podcast_url.as_str()).collect::<Vec<_>>())
                .bind(actions.iter().map(|a| a.episode_url.as_str()).collect::<Vec<_>>())
                .bind(actions.iter().map(|a| a.action.as_str()).collect::<Vec<_>>())
                .bind(actions.iter().map(|a| a.timestamp.timestamp()).collect::<Vec<_>>())
                .bind(col(&|a| a.started))
                .bind(col(&|a| a.position))
                .bind(col(&|a| a.total))
                .bind(actions.iter().map(|a| a.guid.clone()).collect::<Vec<_>>())
                .bind(episode_ids)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                mysql_lock_targets_shared(&mut tx, user_id).await?;
                let now: i64 = sqlx::query_scalar("SELECT UNIX_TIMESTAMP()").fetch_one(&mut *tx).await?;
                for (chunk_no, chunk) in actions.chunks(500).enumerate() {
                    let values = vec!["(?, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"; chunk.len()].join(", ");
                    let sql = format!(
                        "INSERT INTO GpodderSyncEpisodeActions
                           (UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Started, Position, Total, Guid, EpisodeID, Origin)
                         VALUES {values}"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
                    for (i, a) in chunk.iter().enumerate() {
                        q = q
                            .bind(user_id)
                            .bind(&a.podcast_url)
                            .bind(&a.episode_url)
                            .bind(a.action.as_str())
                            .bind(a.timestamp.timestamp())
                            .bind(now)
                            .bind(a.started)
                            .bind(a.position)
                            .bind(a.total)
                            .bind(&a.guid)
                            .bind(episode_ids.get(chunk_no * 500 + i).copied().flatten())
                            .bind(origin);
                    }
                    q.execute(&mut *tx).await?;
                }
                tx.commit().await?;
            }
        }
        Ok(())
    }

    /// Log subscription changes pulled from an external target, as `origin`.
    pub async fn sync_log_remote_subscriptions(&self, user_id: i32, urls: &[String], action: &str, origin: &str) -> AppResult<()> {
        if urls.is_empty() {
            return Ok(());
        }
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                pg_lock_targets_shared(&mut tx, user_id).await?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    r#"INSERT INTO "GpodderSyncSubscriptions" (userid, deviceid, podcasturl, action, timestamp, serverts, origin)
                       SELECT $1, NULL, u.url, $2, n.t, n.t, $3 FROM UNNEST($4::text[]) AS u(url), {PG_NOW}"#
                )))
                .bind(user_id)
                .bind(action)
                .bind(origin)
                .bind(urls)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                mysql_lock_targets_shared(&mut tx, user_id).await?;
                for url in urls {
                    sqlx::query(
                        "INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp, ServerTs, Origin)
                         VALUES (?, NULL, ?, ?, UNIX_TIMESTAMP(), UNIX_TIMESTAMP(), ?)",
                    )
                    .bind(user_id)
                    .bind(url)
                    .bind(action)
                    .bind(origin)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
        }
        Ok(())
    }

    /// The podcast's owner and feed URL.
    pub async fn sync_podcast_owner(&self, podcast_id: i32) -> AppResult<Option<(i32, String)>> {
        let row = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as::<_, (i32, Option<String>)>(r#"SELECT userid, feedurl FROM "Podcasts" WHERE podcastid = $1"#)
                .bind(podcast_id)
                .fetch_optional(pool)
                .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as::<_, (i32, Option<String>)>("SELECT UserID, FeedURL FROM Podcasts WHERE PodcastID = ?")
                .bind(podcast_id)
                .fetch_optional(pool)
                .await?,
        };
        Ok(row.map(|(user, url)| (user, url.unwrap_or_default())))
    }

    /// Unmatched (EpisodeID NULL) log actions for one of the user's feeds, oldest first.
    pub async fn sync_unmatched_actions(&self, user_id: i32, podcast_url: &str) -> AppResult<Vec<LoggedAction>> {
        let rows = match self {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"SELECT actionid AS id, podcasturl AS purl, episodeurl AS eurl, guid, action, timestamp AS ts,
                          started, position, total
                   FROM "GpodderSyncEpisodeActions"
                   WHERE userid = $1 AND episodeid IS NULL AND podcasturl = $2
                   ORDER BY actionid"#,
            )
            .bind(user_id)
            .bind(podcast_url)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| logged_action(&r))
            .collect::<AppResult<Vec<_>>>()?,
            DatabasePool::MySQL(pool) => sqlx::query(
                "SELECT ActionID AS id, PodcastURL AS purl, EpisodeURL AS eurl, Guid AS guid, Action AS action,
                        Timestamp AS ts, Started AS started, Position AS position, Total AS total
                 FROM GpodderSyncEpisodeActions
                 WHERE UserID = ? AND EpisodeID IS NULL AND PodcastURL = ?
                 ORDER BY ActionID",
            )
            .bind(user_id)
            .bind(podcast_url)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| logged_action(&r))
            .collect::<AppResult<Vec<_>>>()?,
        };
        Ok(rows.into_iter().flatten().collect())
    }

    /// Record the resolver's answer on log rows (`(ActionID, EpisodeID)`).
    pub async fn sync_set_action_episodes(&self, matches: &[(i64, i32)]) -> AppResult<()> {
        if matches.is_empty() {
            return Ok(());
        }
        match self {
            DatabasePool::Postgres(pool) => {
                let ids: Vec<i64> = matches.iter().map(|m| m.0).collect();
                let eps: Vec<i32> = matches.iter().map(|m| m.1).collect();
                sqlx::query(
                    r#"UPDATE "GpodderSyncEpisodeActions" a SET episodeid = u.ep
                       FROM UNNEST($1::bigint[], $2::int[]) AS u(id, ep)
                       WHERE a.actionid = u.id AND a.episodeid IS NULL"#,
                )
                .bind(&ids)
                .bind(&eps)
                .execute(pool)
                .await?;
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                for (action_id, episode_id) in matches {
                    sqlx::query("UPDATE GpodderSyncEpisodeActions SET EpisodeID = ? WHERE ActionID = ? AND EpisodeID IS NULL")
                        .bind(episode_id)
                        .bind(action_id)
                        .execute(&mut *tx)
                        .await?;
                }
                tx.commit().await?;
            }
        }
        Ok(())
    }

    /// Every feed URL the user is subscribed to.
    pub async fn sync_user_feed_urls(&self, user_id: i32) -> AppResult<std::collections::HashSet<String>> {
        let urls: Vec<Option<String>> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(r#"SELECT feedurl FROM "Podcasts" WHERE userid = $1"#)
                .bind(user_id)
                .fetch_all(pool)
                .await?,
            DatabasePool::MySQL(pool) => sqlx::query_scalar("SELECT FeedURL FROM Podcasts WHERE UserID = ?")
                .bind(user_id)
                .fetch_all(pool)
                .await?,
        };
        Ok(urls.into_iter().flatten().collect())
    }

    /// The user's pinepods device name, creating the device if needed.
    pub async fn sync_ensure_pinepods_device(&self, user_id: i32) -> AppResult<String> {
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let id = pg_pinepods_device(&mut tx, user_id).await?;
                let name: String = sqlx::query_scalar(r#"SELECT devicename FROM "GpodderDevices" WHERE deviceid = $1"#)
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok(name)
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                let id = mysql_pinepods_device(&mut tx, user_id).await?;
                let name: String = sqlx::query_scalar("SELECT DeviceName FROM GpodderDevices WHERE DeviceID = ?")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok(name)
            }
        }
    }
}

fn logged_action<R: Row>(r: &R) -> AppResult<Option<LoggedAction>>
where
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    i32: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let id: i64 = r.try_get::<i32, _>("id").map(i64::from).or_else(|_| r.try_get::<i64, _>("id"))?;
    let Some(action) = ActionKind::parse(&r.try_get::<String, _>("action")?) else { return Ok(None) };
    let ts: i64 = r.try_get("ts")?;
    Ok(Some(LoggedAction {
        action_id: id,
        action: EpisodeAction {
            podcast_url: r.try_get("purl")?,
            episode_url: r.try_get("eurl")?,
            guid: r.try_get::<Option<String>, _>("guid")?.filter(|g| !g.is_empty()),
            action,
            timestamp: DateTime::from_timestamp(ts, 0).unwrap_or_default(),
            started: r.try_get("started")?,
            position: r.try_get("position")?,
            total: r.try_get("total")?,
            device: None,
        },
    }))
}

// ---- compaction -----------------------------------------------------------------------------

fn action_key(episode_id: Option<i32>, podcast_url: &str, episode_url: &str, action: &str) -> String {
    match episode_id {
        Some(id) => format!("e:{id}|{}", action.to_ascii_lowercase()),
        None => format!("u:{podcast_url}\n{episode_url}|{}", action.to_ascii_lowercase()),
    }
}

impl DatabasePool {
    /// Users with log rows that arrived before `cutoff` (unix seconds).
    pub async fn sync_log_users_before(&self, cutoff: i64) -> AppResult<Vec<i32>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(
                r#"SELECT userid FROM "GpodderSyncSubscriptions" WHERE serverts < $1
                   UNION SELECT userid FROM "GpodderSyncEpisodeActions" WHERE COALESCE(serverts, 0) < $1"#,
            )
            .bind(cutoff)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_scalar(
                "SELECT UserID FROM GpodderSyncSubscriptions WHERE ServerTs < ?
                 UNION SELECT UserID FROM GpodderSyncEpisodeActions WHERE COALESCE(ServerTs, 0) < ?",
            )
            .bind(cutoff)
            .bind(cutoff)
            .fetch_all(pool)
            .await?,
        })
    }

    /// Delete the user's superseded log rows older than `cutoff` that every target has pushed.
    /// Runs with the targets share-locked, so no push cursor moves while it decides.
    pub async fn sync_compact_user(&self, user_id: i32, cutoff: i64) -> AppResult<super::compact::CompactSummary> {
        use super::compact::{superseded, CompactSummary, LogRow};
        let lowest = |cursors: Vec<(i64, i64)>| -> (Option<i64>, Option<i64>) {
            (cursors.iter().map(|c| c.0).min(), cursors.iter().map(|c| c.1).min())
        };
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let cursors: Vec<(i64, i64)> = sqlx::query_as(
                    r#"SELECT subspushcursor, actionspushcursor FROM "SyncTargets" WHERE userid = $1 FOR SHARE"#,
                )
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?;
                let (subs_pushed, actions_pushed) = lowest(cursors);
                let subs: Vec<(i64, String, i64)> = sqlx::query_as(
                    r#"SELECT subscriptionid::BIGINT, podcasturl, serverts FROM "GpodderSyncSubscriptions" WHERE userid = $1"#,
                )
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?;
                let subs: Vec<LogRow> = subs.into_iter().map(|(id, url, ts)| LogRow { id, key: url, server_ts: ts }).collect();
                let actions: Vec<(i64, Option<i32>, String, String, String, i64)> = sqlx::query_as(
                    r#"SELECT actionid::BIGINT, episodeid, podcasturl, episodeurl, action, COALESCE(serverts, 0)
                       FROM "GpodderSyncEpisodeActions" WHERE userid = $1"#,
                )
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?;
                let actions: Vec<LogRow> = actions
                    .into_iter()
                    .map(|(id, ep, purl, eurl, action, ts)| LogRow { id, key: action_key(ep, &purl, &eurl, &action), server_ts: ts })
                    .collect();
                let mut summary = CompactSummary::default();
                for chunk in superseded(&subs, cutoff, subs_pushed).chunks(1000) {
                    summary.subscriptions += sqlx::query(r#"DELETE FROM "GpodderSyncSubscriptions" WHERE subscriptionid = ANY($1)"#)
                        .bind(chunk.iter().map(|id| *id as i32).collect::<Vec<i32>>())
                        .execute(&mut *tx)
                        .await?
                        .rows_affected();
                }
                for chunk in superseded(&actions, cutoff, actions_pushed).chunks(1000) {
                    summary.actions += sqlx::query(r#"DELETE FROM "GpodderSyncEpisodeActions" WHERE actionid = ANY($1)"#)
                        .bind(chunk.iter().map(|id| *id as i32).collect::<Vec<i32>>())
                        .execute(&mut *tx)
                        .await?
                        .rows_affected();
                }
                tx.commit().await?;
                Ok(summary)
            }
            DatabasePool::MySQL(pool) => {
                // The share lock is the first statement (MariaDB snapshot isolation).
                let mut tx = pool.begin().await?;
                let cursors: Vec<(i64, i64)> = sqlx::query_as(
                    "SELECT SubsPushCursor, ActionsPushCursor FROM SyncTargets WHERE UserID = ? LOCK IN SHARE MODE",
                )
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?;
                let (subs_pushed, actions_pushed) = lowest(cursors);
                let subs: Vec<(i64, String, i64)> = sqlx::query_as(
                    "SELECT CAST(SubscriptionID AS SIGNED), PodcastURL, ServerTs FROM GpodderSyncSubscriptions WHERE UserID = ?",
                )
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?;
                let subs: Vec<LogRow> = subs.into_iter().map(|(id, url, ts)| LogRow { id, key: url, server_ts: ts }).collect();
                let actions: Vec<(i64, Option<i32>, String, String, String, i64)> = sqlx::query_as(
                    "SELECT CAST(ActionID AS SIGNED), EpisodeID, PodcastURL, EpisodeURL, Action, CAST(COALESCE(ServerTs, 0) AS SIGNED)
                     FROM GpodderSyncEpisodeActions WHERE UserID = ?",
                )
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?;
                let actions: Vec<LogRow> = actions
                    .into_iter()
                    .map(|(id, ep, purl, eurl, action, ts)| LogRow { id, key: action_key(ep, &purl, &eurl, &action), server_ts: ts })
                    .collect();
                let mut summary = CompactSummary::default();
                for (table, column, ids, count) in [
                    ("GpodderSyncSubscriptions", "SubscriptionID", superseded(&subs, cutoff, subs_pushed), &mut summary.subscriptions),
                    ("GpodderSyncEpisodeActions", "ActionID", superseded(&actions, cutoff, actions_pushed), &mut summary.actions),
                ] {
                    for chunk in ids.chunks(1000) {
                        let sql = format!("DELETE FROM {table} WHERE {column} IN ({})", vec!["?"; chunk.len()].join(", "));
                        let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()));
                        for id in chunk {
                            q = q.bind(id);
                        }
                        *count += q.execute(&mut *tx).await?.rows_affected();
                    }
                }
                tx.commit().await?;
                Ok(summary)
            }
        }
    }
}
