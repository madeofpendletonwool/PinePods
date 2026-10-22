// SQL for the external sync engine, per backend: SyncTargets rows, the outbox reads behind a
// push, cursor advances, and the settings writes that create or remove a target.

use std::collections::HashSet;

use chrono::{DateTime, NaiveDateTime, Utc};
use sqlx::{Connection, Row};

use super::model::{ActionKind, EpisodeAction};
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetKind {
    Gpodder,
    Nextcloud,
}

impl TargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            TargetKind::Gpodder => "gpodder",
            TargetKind::Nextcloud => "nextcloud",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "gpodder" => Some(TargetKind::Gpodder),
            "nextcloud" => Some(TargetKind::Nextcloud),
            _ => None,
        }
    }
}

/// One `SyncTargets` row.
#[derive(Debug, Clone)]
pub struct SyncTargetRow {
    pub target_id: i32,
    pub user_id: i32,
    pub kind: TargetKind,
    pub url: String,
    pub username: String,
    pub encrypted_secret: String,
    pub device_name: Option<String>,
    pub enabled: bool,
    pub subs_pull_cursor: i64,
    pub actions_pull_cursor: i64,
    pub subs_push_cursor: i64,
    pub actions_push_cursor: i64,
    pub needs_full_sync: bool,
    pub last_attempt_at: Option<NaiveDateTime>,
    pub last_success_at: Option<NaiveDateTime>,
    pub last_error: Option<String>,
    pub consecutive_failures: i32,
}

/// Which push cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outbox {
    Subscriptions,
    Actions,
}

/// A log row waiting to be pushed. `fingerprint` is compared again when the cursor advances, to
/// catch rows rewritten (coalesced progress) while the push was in flight.
#[derive(Debug, Clone)]
pub struct PendingSubscription {
    pub id: i64,
    pub url: String,
    pub add: bool,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct PendingAction {
    pub id: i64,
    /// None for a kind Pinepods doesn't send (it still moves the cursor).
    pub action: Option<EpisodeAction>,
    pub duration: Option<i32>,
    pub fingerprint: String,
}

const TARGET_COLUMNS: &str = "TargetID, UserID, Kind, Url, Username, EncryptedSecret, DeviceName, Enabled, SubsPullCursor, \
     ActionsPullCursor, SubsPushCursor, ActionsPushCursor, NeedsFullSync, LastAttemptAt, LastSuccessAt, LastError, ConsecutiveFailures";

fn target_row<R: Row>(r: &R) -> AppResult<SyncTargetRow>
where
    usize: sqlx::ColumnIndex<R>,
    i32: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    i64: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    bool: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    String: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    NaiveDateTime: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
{
    let kind: String = r.try_get(2)?;
    Ok(SyncTargetRow {
        target_id: r.try_get(0)?,
        user_id: r.try_get(1)?,
        kind: TargetKind::parse(&kind).ok_or_else(|| AppError::internal(format!("unknown sync target kind {kind:?}")))?,
        url: r.try_get(3)?,
        username: r.try_get(4)?,
        encrypted_secret: r.try_get(5)?,
        device_name: r.try_get::<Option<String>, _>(6)?.filter(|d| !d.trim().is_empty()),
        enabled: r.try_get(7)?,
        subs_pull_cursor: r.try_get(8)?,
        actions_pull_cursor: r.try_get(9)?,
        subs_push_cursor: r.try_get(10)?,
        actions_push_cursor: r.try_get(11)?,
        needs_full_sync: r.try_get(12)?,
        last_attempt_at: r.try_get(13)?,
        last_success_at: r.try_get(14)?,
        last_error: r.try_get(15)?,
        consecutive_failures: r.try_get(16)?,
    })
}

fn subscription_fingerprint(url: &str, action: &str) -> String {
    format!("{action}|{url}")
}

#[allow(clippy::too_many_arguments)]
fn action_fingerprint(purl: &str, eurl: &str, action: &str, ts: i64, started: Option<i32>, position: Option<i32>, total: Option<i32>) -> String {
    format!("{action}|{ts}|{started:?}|{position:?}|{total:?}|{purl}|{eurl}")
}

fn pending_action<R: Row>(r: &R) -> AppResult<PendingAction>
where
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    i32: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    i64: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
    String: for<'a> sqlx::Decode<'a, R::Database> + sqlx::Type<R::Database>,
{
    let id: i32 = r.try_get("id")?;
    let purl: String = r.try_get("purl")?;
    let eurl: String = r.try_get("eurl")?;
    let action: String = r.try_get("action")?;
    let ts: i64 = r.try_get("ts")?;
    let (started, position, total): (Option<i32>, Option<i32>, Option<i32>) = (r.try_get("started")?, r.try_get("position")?, r.try_get("total")?);
    let fingerprint = action_fingerprint(&purl, &eurl, &action, ts, started, position, total);
    let guid = r.try_get::<Option<String>, _>("guid")?.filter(|g| !g.is_empty());
    let action = ActionKind::parse(&action).map(|kind| EpisodeAction {
        podcast_url: purl,
        episode_url: eurl,
        guid,
        action: kind,
        timestamp: DateTime::from_timestamp(ts, 0).unwrap_or_default(),
        started,
        position,
        total,
        device: None,
    });
    Ok(PendingAction { id: i64::from(id), action, duration: r.try_get("duration")?, fingerprint })
}

/// Where a cursor may advance to: the highest sent id, unless a row at or below it is new
/// (committed after we read) or no longer what we sent; then just below the first such row.
fn verified_cursor(old: i64, sent: &[(i64, String)], current: &[(i64, String)]) -> i64 {
    let max_sent = sent.iter().map(|(id, _)| *id).max().unwrap_or(old);
    let sent: std::collections::HashMap<i64, &str> = sent.iter().map(|(id, f)| (*id, f.as_str())).collect();
    let mut changed: Vec<i64> = current.iter().filter(|(id, f)| sent.get(id) != Some(&f.as_str())).map(|(id, _)| *id).collect();
    changed.sort_unstable();
    match changed.first() {
        Some(first) => (first - 1).max(old),
        None => max_sent.max(old),
    }
}

impl DatabasePool {
    pub async fn sync_target(&self, target_id: i32) -> AppResult<Option<SyncTargetRow>> {
        let row = match self {
            DatabasePool::Postgres(pool) => sqlx::query(sqlx::AssertSqlSafe(format!(r#"SELECT {TARGET_COLUMNS} FROM "SyncTargets" WHERE targetid = $1"#)))
                .bind(target_id)
                .fetch_optional(pool)
                .await?
                .map(|r| target_row(&r))
                .transpose()?,
            DatabasePool::MySQL(pool) => sqlx::query(sqlx::AssertSqlSafe(format!("SELECT {TARGET_COLUMNS} FROM SyncTargets WHERE TargetID = ?")))
                .bind(target_id)
                .fetch_optional(pool)
                .await?
                .map(|r| target_row(&r))
                .transpose()?,
        };
        Ok(row)
    }

    /// The user's targets, enabled or not.
    pub async fn sync_targets_for_user(&self, user_id: i32) -> AppResult<Vec<SyncTargetRow>> {
        match self {
            DatabasePool::Postgres(pool) => sqlx::query(sqlx::AssertSqlSafe(format!(
                r#"SELECT {TARGET_COLUMNS} FROM "SyncTargets" WHERE userid = $1 ORDER BY targetid"#
            )))
            .bind(user_id)
            .fetch_all(pool)
            .await?
            .iter()
            .map(target_row)
            .collect(),
            DatabasePool::MySQL(pool) => sqlx::query(sqlx::AssertSqlSafe(format!("SELECT {TARGET_COLUMNS} FROM SyncTargets WHERE UserID = ? ORDER BY TargetID")))
                .bind(user_id)
                .fetch_all(pool)
                .await?
                .iter()
                .map(target_row)
                .collect(),
        }
    }

    /// Users with an enabled target, optionally of one kind.
    pub async fn sync_target_users(&self, kind: Option<TargetKind>) -> AppResult<Vec<i32>> {
        let kind = kind.map(TargetKind::as_str);
        let users: Vec<i32> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(
                r#"SELECT DISTINCT userid FROM "SyncTargets" WHERE enabled AND ($1::text IS NULL OR kind = $1) ORDER BY userid"#,
            )
            .bind(kind)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_scalar(
                "SELECT DISTINCT UserID FROM SyncTargets WHERE Enabled AND (? IS NULL OR Kind = ?) ORDER BY UserID",
            )
            .bind(kind)
            .bind(kind)
            .fetch_all(pool)
            .await?,
        };
        Ok(users)
    }

    pub async fn sync_target_set_device(&self, target_id: i32, device_name: &str) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "SyncTargets" SET devicename = $1 WHERE targetid = $2"#)
                    .bind(device_name)
                    .bind(target_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE SyncTargets SET DeviceName = ? WHERE TargetID = ?")
                    .bind(device_name)
                    .bind(target_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn sync_target_record_attempt(&self, target_id: i32) -> AppResult<()> {
        let now = Utc::now().naive_utc();
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "SyncTargets" SET lastattemptat = $1 WHERE targetid = $2"#)
                    .bind(now)
                    .bind(target_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE SyncTargets SET LastAttemptAt = ? WHERE TargetID = ?")
                    .bind(now)
                    .bind(target_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// A run finished: save the server's cursors, clear the error, and clear NeedsFullSync if
    /// this was the full sync.
    pub async fn sync_target_record_success(&self, target_id: i32, subs_pull: i64, actions_pull: i64, was_full: bool) -> AppResult<()> {
        let now = Utc::now().naive_utc();
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(
                    r#"UPDATE "SyncTargets"
                       SET subspullcursor = $1, actionspullcursor = $2, lastsuccessat = $3, lasterror = NULL,
                           consecutivefailures = 0, needsfullsync = needsfullsync AND NOT $4
                       WHERE targetid = $5"#,
                )
                .bind(subs_pull)
                .bind(actions_pull)
                .bind(now)
                .bind(was_full)
                .bind(target_id)
                .execute(pool)
                .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(
                    "UPDATE SyncTargets
                     SET SubsPullCursor = ?, ActionsPullCursor = ?, LastSuccessAt = ?, LastError = NULL,
                         ConsecutiveFailures = 0, NeedsFullSync = NeedsFullSync AND NOT ?
                     WHERE TargetID = ?",
                )
                .bind(subs_pull)
                .bind(actions_pull)
                .bind(now)
                .bind(was_full)
                .bind(target_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn sync_target_record_failure(&self, target_id: i32, error: &str) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "SyncTargets" SET lasterror = $1, consecutivefailures = consecutivefailures + 1 WHERE targetid = $2"#)
                    .bind(error)
                    .bind(target_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE SyncTargets SET LastError = ?, ConsecutiveFailures = ConsecutiveFailures + 1 WHERE TargetID = ?")
                    .bind(error)
                    .bind(target_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// Subscription log rows after `after` that didn't come from `origin`, oldest first.
    pub async fn sync_pending_subscriptions(&self, user_id: i32, origin: &str, after: i64) -> AppResult<Vec<PendingSubscription>> {
        let rows: Vec<(i32, String, String)> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT subscriptionid, podcasturl, action FROM "GpodderSyncSubscriptions"
                   WHERE userid = $1 AND subscriptionid > $2 AND origin <> $3 ORDER BY subscriptionid"#,
            )
            .bind(user_id)
            .bind(after)
            .bind(origin)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as(
                "SELECT SubscriptionID, PodcastURL, Action FROM GpodderSyncSubscriptions
                 WHERE UserID = ? AND SubscriptionID > ? AND Origin <> ? ORDER BY SubscriptionID",
            )
            .bind(user_id)
            .bind(after)
            .bind(origin)
            .fetch_all(pool)
            .await?,
        };
        Ok(rows
            .into_iter()
            .map(|(id, url, action)| PendingSubscription {
                id: i64::from(id),
                fingerprint: subscription_fingerprint(&url, &action),
                add: action == "add",
                url,
            })
            .collect())
    }

    /// Up to `limit` episode-action log rows after `after` that didn't come from `origin`, oldest
    /// first, with the episode's duration and GUID when the row is matched.
    pub async fn sync_pending_actions(&self, user_id: i32, origin: &str, after: i64, limit: usize) -> AppResult<Vec<PendingAction>> {
        let limit = limit as i64;
        let rows = match self {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"SELECT a.actionid AS id, a.podcasturl AS purl, a.episodeurl AS eurl, a.action, a.timestamp AS ts,
                          a.started, a.position, a.total, COALESCE(NULLIF(a.guid, ''), NULLIF(e.episodeguid, '')) AS guid,
                          e.episodeduration AS duration
                   FROM "GpodderSyncEpisodeActions" a LEFT JOIN "Episodes" e ON e.episodeid = a.episodeid
                   WHERE a.userid = $1 AND a.actionid > $2 AND a.origin <> $3
                   ORDER BY a.actionid LIMIT $4"#,
            )
            .bind(user_id)
            .bind(after)
            .bind(origin)
            .bind(limit)
            .fetch_all(pool)
            .await?
            .iter()
            .map(pending_action)
            .collect::<AppResult<Vec<_>>>()?,
            DatabasePool::MySQL(pool) => sqlx::query(
                "SELECT a.ActionID AS id, a.PodcastURL AS purl, a.EpisodeURL AS eurl, a.Action AS action, a.Timestamp AS ts,
                        a.Started AS started, a.Position AS position, a.Total AS total,
                        COALESCE(NULLIF(a.Guid, ''), NULLIF(e.EpisodeGUID, '')) AS guid, e.EpisodeDuration AS duration
                 FROM GpodderSyncEpisodeActions a LEFT JOIN Episodes e ON e.EpisodeID = a.EpisodeID
                 WHERE a.UserID = ? AND a.ActionID > ? AND a.Origin <> ?
                 ORDER BY a.ActionID LIMIT ?",
            )
            .bind(user_id)
            .bind(after)
            .bind(origin)
            .bind(limit)
            .fetch_all(pool)
            .await?
            .iter()
            .map(pending_action)
            .collect::<AppResult<Vec<_>>>()?,
        };
        Ok(rows)
    }

    /// Advance a push cursor after a successful push of `sent` (`(id, fingerprint)`). Runs under
    /// an exclusive lock on the target row, which waits for log writers (they share-lock it) to
    /// commit; then the rows up to the highest sent id are read again, and the cursor stops just
    /// below any row that wasn't sent or has changed since. Returns the new cursor.
    pub async fn sync_advance_push_cursor(&self, target_id: i32, user_id: i32, outbox: Outbox, origin: &str, old: i64, sent: &[(i64, String)]) -> AppResult<i64> {
        let Some(max_sent) = sent.iter().map(|(id, _)| *id).max() else { return Ok(old) };
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(r#"SELECT targetid FROM "SyncTargets" WHERE targetid = $1 FOR UPDATE"#)
                    .bind(target_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                let current: Vec<(i64, String)> = match outbox {
                    Outbox::Subscriptions => sqlx::query_as::<_, (i32, String, String)>(
                        r#"SELECT subscriptionid, podcasturl, action FROM "GpodderSyncSubscriptions"
                           WHERE userid = $1 AND subscriptionid > $2 AND subscriptionid <= $3 AND origin <> $4"#,
                    )
                    .bind(user_id)
                    .bind(old)
                    .bind(max_sent)
                    .bind(origin)
                    .fetch_all(&mut *tx)
                    .await?
                    .into_iter()
                    .map(|(id, url, action)| (i64::from(id), subscription_fingerprint(&url, &action)))
                    .collect(),
                    Outbox::Actions => sqlx::query_as::<_, (i32, String, String, String, i64, Option<i32>, Option<i32>, Option<i32>)>(
                        r#"SELECT actionid, podcasturl, episodeurl, action, timestamp, started, position, total
                           FROM "GpodderSyncEpisodeActions"
                           WHERE userid = $1 AND actionid > $2 AND actionid <= $3 AND origin <> $4"#,
                    )
                    .bind(user_id)
                    .bind(old)
                    .bind(max_sent)
                    .bind(origin)
                    .fetch_all(&mut *tx)
                    .await?
                    .into_iter()
                    .map(|(id, p, e, a, ts, s, pos, t)| (i64::from(id), action_fingerprint(&p, &e, &a, ts, s, pos, t)))
                    .collect(),
                };
                let cursor = verified_cursor(old, sent, &current);
                let column = match outbox {
                    Outbox::Subscriptions => "subspushcursor",
                    Outbox::Actions => "actionspushcursor",
                };
                sqlx::query(sqlx::AssertSqlSafe(format!(r#"UPDATE "SyncTargets" SET {column} = $1 WHERE targetid = $2"#)))
                    .bind(cursor)
                    .bind(target_id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok(cursor)
            }
            DatabasePool::MySQL(pool) => {
                // READ COMMITTED, so the re-read sees rows committed while we waited for the lock.
                let mut conn = pool.acquire().await?;
                sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").execute(&mut *conn).await?;
                let mut tx = conn.begin().await?;
                sqlx::query("SELECT TargetID FROM SyncTargets WHERE TargetID = ? FOR UPDATE")
                    .bind(target_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                let current: Vec<(i64, String)> = match outbox {
                    Outbox::Subscriptions => sqlx::query_as::<_, (i32, String, String)>(
                        "SELECT SubscriptionID, PodcastURL, Action FROM GpodderSyncSubscriptions
                         WHERE UserID = ? AND SubscriptionID > ? AND SubscriptionID <= ? AND Origin <> ?",
                    )
                    .bind(user_id)
                    .bind(old)
                    .bind(max_sent)
                    .bind(origin)
                    .fetch_all(&mut *tx)
                    .await?
                    .into_iter()
                    .map(|(id, url, action)| (i64::from(id), subscription_fingerprint(&url, &action)))
                    .collect(),
                    Outbox::Actions => sqlx::query_as::<_, (i32, String, String, String, i64, Option<i32>, Option<i32>, Option<i32>)>(
                        "SELECT ActionID, PodcastURL, EpisodeURL, Action, Timestamp, Started, Position, Total
                         FROM GpodderSyncEpisodeActions
                         WHERE UserID = ? AND ActionID > ? AND ActionID <= ? AND Origin <> ?",
                    )
                    .bind(user_id)
                    .bind(old)
                    .bind(max_sent)
                    .bind(origin)
                    .fetch_all(&mut *tx)
                    .await?
                    .into_iter()
                    .map(|(id, p, e, a, ts, s, pos, t)| (i64::from(id), action_fingerprint(&p, &e, &a, ts, s, pos, t)))
                    .collect(),
                };
                let cursor = verified_cursor(old, sent, &current);
                let column = match outbox {
                    Outbox::Subscriptions => "SubsPushCursor",
                    Outbox::Actions => "ActionsPushCursor",
                };
                sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE SyncTargets SET {column} = ? WHERE TargetID = ?")))
                    .bind(cursor)
                    .bind(target_id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok(cursor)
            }
        }
    }

    /// Move both push cursors to the end of the log (a full sync pushes the current state
    /// instead). Under the same exclusive lock as an advance, so a writer either committed before
    /// (its change is in the state read next) or sees the new cursor and starts a new row.
    pub async fn sync_reset_push_cursors(&self, target_id: i32, user_id: i32) -> AppResult<(i64, i64)> {
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(r#"SELECT targetid FROM "SyncTargets" WHERE targetid = $1 FOR UPDATE"#)
                    .bind(target_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                let subs: i64 = sqlx::query_scalar(r#"SELECT COALESCE(MAX(subscriptionid), 0)::BIGINT FROM "GpodderSyncSubscriptions" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
                let actions: i64 = sqlx::query_scalar(r#"SELECT COALESCE(MAX(actionid), 0)::BIGINT FROM "GpodderSyncEpisodeActions" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
                sqlx::query(r#"UPDATE "SyncTargets" SET subspushcursor = $1, actionspushcursor = $2 WHERE targetid = $3"#)
                    .bind(subs)
                    .bind(actions)
                    .bind(target_id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok((subs, actions))
            }
            DatabasePool::MySQL(pool) => {
                let mut conn = pool.acquire().await?;
                sqlx::query("SET TRANSACTION ISOLATION LEVEL READ COMMITTED").execute(&mut *conn).await?;
                let mut tx = conn.begin().await?;
                sqlx::query("SELECT TargetID FROM SyncTargets WHERE TargetID = ? FOR UPDATE")
                    .bind(target_id)
                    .fetch_optional(&mut *tx)
                    .await?;
                let subs: i64 = sqlx::query_scalar("SELECT CAST(COALESCE(MAX(SubscriptionID), 0) AS SIGNED) FROM GpodderSyncSubscriptions WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
                let actions: i64 = sqlx::query_scalar("SELECT CAST(COALESCE(MAX(ActionID), 0) AS SIGNED) FROM GpodderSyncEpisodeActions WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(&mut *tx)
                    .await?;
                sqlx::query("UPDATE SyncTargets SET SubsPushCursor = ?, ActionsPushCursor = ? WHERE TargetID = ?")
                    .bind(subs)
                    .bind(actions)
                    .bind(target_id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                Ok((subs, actions))
            }
        }
    }

    /// For each action, whether the user's log already holds the same action (episode URL,
    /// kind, action time, position). Pulls use it to drop what they already know: actions this
    /// Pinepods pushed, and the ones a `>=` cursor sends again.
    pub async fn sync_known_actions(&self, user_id: i32, actions: &[EpisodeAction]) -> AppResult<Vec<bool>> {
        type Key = (String, String, i64, Option<i32>);
        let mut stamps: Vec<i64> = actions.iter().map(|a| a.timestamp.timestamp()).collect();
        stamps.sort_unstable();
        stamps.dedup();
        let mut known: HashSet<Key> = HashSet::new();
        for chunk in stamps.chunks(1000) {
            let rows: Vec<(String, String, i64, Option<i32>)> = match self {
                DatabasePool::Postgres(pool) => sqlx::query_as(
                    r#"SELECT episodeurl, action, timestamp, position FROM "GpodderSyncEpisodeActions"
                       WHERE userid = $1 AND timestamp = ANY($2)"#,
                )
                .bind(user_id)
                .bind(chunk)
                .fetch_all(pool)
                .await?,
                DatabasePool::MySQL(pool) => {
                    let marks = vec!["?"; chunk.len()].join(",");
                    let sql = format!(
                        "SELECT EpisodeURL, Action, Timestamp, Position FROM GpodderSyncEpisodeActions WHERE UserID = ? AND Timestamp IN ({marks})"
                    );
                    let mut q = sqlx::query_as(sqlx::AssertSqlSafe(sql)).bind(user_id);
                    for t in chunk {
                        q = q.bind(t);
                    }
                    q.fetch_all(pool).await?
                }
            };
            known.extend(rows);
        }
        Ok(actions
            .iter()
            .map(|a| {
                let position = if a.action == ActionKind::Play { a.position } else { None };
                known.contains(&(a.episode_url.clone(), a.action.as_str().to_string(), a.timestamp.timestamp(), position))
            })
            .collect())
    }

    /// The user's shareable feeds (what other devices may see).
    pub async fn sync_shareable_feed_urls(&self, user_id: i32) -> AppResult<Vec<String>> {
        let rows: Vec<(Option<String>, Option<String>, Option<String>, Option<bool>)> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT feedurl, username, password, isyoutubechannel FROM "Podcasts" WHERE userid = $1 ORDER BY podcastid"#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as(
                "SELECT FeedURL, Username, Password, IsYouTubeChannel FROM Podcasts WHERE UserID = ? ORDER BY PodcastID",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
        };
        Ok(rows
            .into_iter()
            .filter_map(|(url, user, pass, yt)| {
                let url = url?;
                super::shareable_feed(&url, user.as_deref(), pass.as_deref(), yt.unwrap_or(false)).then_some(url)
            })
            .collect())
    }

    /// The user's current listening state as `play` actions (position, or the duration when
    /// completed), with the episode's duration, for a full sync. Shareable feeds only.
    pub async fn sync_episode_states(&self, user_id: i32) -> AppResult<Vec<(EpisodeAction, Option<i32>)>> {
        type StateRow = (String, String, Option<String>, Option<i32>, i64, Option<i64>);
        let rows: Vec<StateRow> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT p.feedurl, e.episodeurl, NULLIF(e.episodeguid, ''), e.episodeduration,
                          CASE WHEN COALESCE(e.completed, FALSE) AND COALESCE(e.episodeduration, 0) > 0
                               THEN e.episodeduration ELSE COALESCE(h.listenduration, 0) END::BIGINT,
                          FLOOR(EXTRACT(EPOCH FROM h.listendate))::BIGINT
                   FROM "Episodes" e
                   JOIN "Podcasts" p ON p.podcastid = e.podcastid
                   LEFT JOIN "UserEpisodeHistory" h ON h.episodeid = e.episodeid AND h.userid = $1
                   WHERE p.userid = $1 AND e.episodeurl IS NOT NULL
                     AND (LOWER(p.feedurl) LIKE 'http://%' OR LOWER(p.feedurl) LIKE 'https://%')
                     AND COALESCE(p.username, '') = '' AND COALESCE(p.password, '') = ''
                     AND NOT COALESCE(p.isyoutubechannel, FALSE)
                     AND (COALESCE(h.listenduration, 0) > 0 OR (COALESCE(e.completed, FALSE) AND COALESCE(e.episodeduration, 0) > 0))
                   ORDER BY e.episodeid"#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as(
                "SELECT p.FeedURL, e.EpisodeURL, NULLIF(e.EpisodeGUID, ''), e.EpisodeDuration,
                        CAST(CASE WHEN COALESCE(e.Completed, FALSE) AND COALESCE(e.EpisodeDuration, 0) > 0
                             THEN e.EpisodeDuration ELSE COALESCE(h.ListenDuration, 0) END AS SIGNED),
                        TIMESTAMPDIFF(SECOND, '1970-01-01 00:00:00', h.ListenDate)
                 FROM Episodes e
                 JOIN Podcasts p ON p.PodcastID = e.PodcastID
                 LEFT JOIN UserEpisodeHistory h ON h.EpisodeID = e.EpisodeID AND h.UserID = ?
                 WHERE p.UserID = ? AND e.EpisodeURL IS NOT NULL
                   AND (LOWER(p.FeedURL) LIKE 'http://%' OR LOWER(p.FeedURL) LIKE 'https://%')
                   AND COALESCE(p.Username, '') = '' AND COALESCE(p.Password, '') = ''
                   AND NOT COALESCE(p.IsYouTubeChannel, FALSE)
                   AND (COALESCE(h.ListenDuration, 0) > 0 OR (COALESCE(e.Completed, FALSE) AND COALESCE(e.EpisodeDuration, 0) > 0))
                 ORDER BY e.EpisodeID",
            )
            .bind(user_id)
            .bind(user_id)
            .fetch_all(pool)
            .await?,
        };
        Ok(rows
            .into_iter()
            .map(|(feed, episode, guid, duration, position, at)| {
                let action = EpisodeAction {
                    podcast_url: feed,
                    episode_url: episode,
                    guid,
                    action: ActionKind::Play,
                    timestamp: at.and_then(|t| DateTime::from_timestamp(t, 0)).unwrap_or_else(|| DateTime::from_timestamp(Utc::now().timestamp(), 0).unwrap_or_default()),
                    started: Some(0),
                    position: Some(position.clamp(0, i32::MAX as i64) as i32),
                    total: duration.filter(|d| *d > 0),
                    device: None,
                };
                (action, duration)
            })
            .collect())
    }

    // ---- settings ------------------------------------------------------------------------------

    /// Save the user's external server, replacing any existing one: a new row (new TargetID, so a
    /// run still in flight for the old one can't write its cursors into it) with the secret
    /// encrypted, enabled, NeedsFullSync. The device name is kept when the same server account
    /// is saved again. Clears Users.SyncNotice and updates Pod_Sync_Type.
    pub async fn sync_target_save(&self, user_id: i32, kind: TargetKind, url: &str, username: &str, secret: &str) -> AppResult<i32> {
        let url = url.trim().trim_end_matches('/').to_string();
        let encrypted = self.encrypt_password(secret).await?;
        let previous = self.sync_targets_for_user(user_id).await?;
        let device_name = previous
            .iter()
            .find(|t| t.kind == kind && t.url.trim_end_matches('/') == url && t.username == username)
            .and_then(|t| t.device_name.clone());
        let target_id = match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(r#"DELETE FROM "SyncTargets" WHERE userid = $1"#).bind(user_id).execute(&mut *tx).await?;
                let id: i32 = sqlx::query_scalar(
                    r#"INSERT INTO "SyncTargets" (userid, kind, url, username, encryptedsecret, devicename, enabled, needsfullsync)
                       VALUES ($1, $2, $3, $4, $5, $6, TRUE, TRUE) RETURNING targetid"#,
                )
                .bind(user_id)
                .bind(kind.as_str())
                .bind(&url)
                .bind(username)
                .bind(&encrypted)
                .bind(&device_name)
                .fetch_one(&mut *tx)
                .await?;
                sqlx::query(r#"UPDATE "Users" SET syncnotice = NULL WHERE userid = $1"#).bind(user_id).execute(&mut *tx).await?;
                tx.commit().await?;
                id
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query("DELETE FROM SyncTargets WHERE UserID = ?").bind(user_id).execute(&mut *tx).await?;
                let id = sqlx::query(
                    "INSERT INTO SyncTargets (UserID, Kind, Url, Username, EncryptedSecret, DeviceName, Enabled, NeedsFullSync)
                     VALUES (?, ?, ?, ?, ?, ?, TRUE, TRUE)",
                )
                .bind(user_id)
                .bind(kind.as_str())
                .bind(&url)
                .bind(username)
                .bind(&encrypted)
                .bind(&device_name)
                .execute(&mut *tx)
                .await?
                .last_insert_id() as i32;
                sqlx::query("UPDATE Users SET SyncNotice = NULL WHERE UserID = ?").bind(user_id).execute(&mut *tx).await?;
                tx.commit().await?;
                id
            }
        };
        self.sync_mirror_sync_type(user_id).await?;
        super::invalidate_sync_enabled(user_id);
        Ok(target_id)
    }

    /// Remove the user's external server(s).
    pub async fn sync_target_delete(&self, user_id: i32) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"DELETE FROM "SyncTargets" WHERE userid = $1"#).bind(user_id).execute(pool).await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("DELETE FROM SyncTargets WHERE UserID = ?").bind(user_id).execute(pool).await?;
            }
        }
        super::invalidate_sync_enabled(user_id);
        Ok(())
    }

    /// Keep the legacy Users.Pod_Sync_Type in step with the new settings, for the old settings
    /// UI: 'both' = internal server on + an external target, else 'external' / 'nextcloud' /
    /// 'gpodder' / 'None'.
    pub async fn sync_mirror_sync_type(&self, user_id: i32) -> AppResult<()> {
        let kind = self.sync_targets_for_user(user_id).await?.first().map(|t| t.kind);
        let internal: bool = match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(r#"SELECT gpodderserverenabled FROM "Users" WHERE userid = $1"#)
                .bind(user_id)
                .fetch_optional(pool)
                .await?
                .unwrap_or(false),
            DatabasePool::MySQL(pool) => sqlx::query_scalar("SELECT GpodderServerEnabled FROM Users WHERE UserID = ?")
                .bind(user_id)
                .fetch_optional(pool)
                .await?
                .unwrap_or(false),
        };
        let sync_type = match (internal, kind) {
            (true, Some(_)) => "both",
            (false, Some(TargetKind::Gpodder)) => "external",
            (false, Some(TargetKind::Nextcloud)) => "nextcloud",
            (true, None) => "gpodder",
            (false, None) => "None",
        };
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "Users" SET pod_sync_type = $1 WHERE userid = $2"#)
                    .bind(sync_type)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE Users SET Pod_Sync_Type = ? WHERE UserID = ?")
                    .bind(sync_type)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::verified_cursor;

    fn s(v: &[(i64, &str)]) -> Vec<(i64, String)> {
        v.iter().map(|(i, f)| (*i, f.to_string())).collect()
    }

    #[test]
    fn cursor_stops_below_rows_that_changed_or_arrived_late() {
        let sent = s(&[(11, "a"), (12, "b"), (14, "d")]);
        assert_eq!(verified_cursor(10, &sent, &s(&[(11, "a"), (12, "b"), (14, "d")])), 14);
        // row 12 was rewritten (coalesced progress) after we read it
        assert_eq!(verified_cursor(10, &sent, &s(&[(11, "a"), (12, "b2"), (14, "d")])), 11);
        // row 13 committed after our read, below the highest id we sent
        assert_eq!(verified_cursor(10, &sent, &s(&[(11, "a"), (12, "b"), (13, "c"), (14, "d")])), 12);
        // never moves backwards
        assert_eq!(verified_cursor(10, &sent, &s(&[(11, "x")])), 10);
    }
}
