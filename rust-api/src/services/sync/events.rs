// Local writes → sync log. The DB functions behind subscribe, unsubscribe, progress,
// complete and uncomplete call these with their origin, inside their own transaction where they
// have one. An event never fails the write it belongs to: it runs in a savepoint (on PG a failed
// statement would otherwise abort the caller's transaction) and a failure is logged and dropped.
// A logged event for a user with an enabled external target schedules a push.

use crate::database::dialect::dialect_fn;
use sqlx::{Connection, MySql, MySqlConnection, MySqlPool, Transaction};

use super::db;
use super::{push, sync_enabled, SyncOrigin};
use crate::database::DatabasePool;
use crate::error::AppResult;

pub use super::db::EpisodeEvent;

/// Whether a write by `origin` for `user_id` should be logged: Pinepods' own changes, for users
/// who sync. Check this before starting work that only exists for the event.
pub async fn should_log(db: &DatabasePool, user_id: i32, origin: SyncOrigin) -> bool {
    origin == SyncOrigin::Pinepods && sync_enabled(db, user_id).await
}

/// Begin a MySQL transaction for a write that may log an event. MariaDB aborts a transaction
/// whose locking read finds a row changed after its snapshot (error 1020), and sync runs update
/// the user's SyncTargets rows, so the share lock the event takes is taken first, before any
/// snapshot exists; once held, nothing changes the rows until commit.
pub async fn begin_mysql(pool: &MySqlPool, user_id: i32, log: bool) -> AppResult<Transaction<'static, MySql>> {
    let mut tx = pool.begin().await?;
    if log {
        db::mysql_lock_targets_shared(&mut tx, user_id).await?;
    }
    Ok(tx)
}

fn settle(pool: &DatabasePool, user_id: i32, what: &str, result: AppResult<bool>) {
    match result {
        Ok(true) => push::schedule(pool, user_id),
        Ok(false) => {}
        Err(e) => tracing::warn!("Sync event ({what}) for user {user_id} not recorded: {e}"),
    }
}

// Subscription `add`/`remove` for a shareable feed (the caller checks `shareable_feed`).
dialect_fn!([subscription_pg, subscription_lite]
    pub async fn(pool: &DatabasePool, conn: &mut q::Conn, user_id: i32, feed_url: &str, action: &str)  {
    let result: AppResult<bool> = async {
        let mut sp = conn.begin().await?;
        let push = q::pick(db::pg_log_subscription, db::lite_log_subscription)(&mut sp, user_id, feed_url, action).await?;
        sp.commit().await?;
        Ok(push)
    }
    .await;
    settle(pool, user_id, action, result);
});

pub async fn subscription_mysql(pool: &DatabasePool, conn: &mut MySqlConnection, user_id: i32, feed_url: &str, action: &str) {
    let result: AppResult<bool> = async {
        let mut sp = conn.begin().await?;
        let push = db::mysql_log_subscription(&mut sp, user_id, feed_url, action).await?;
        sp.commit().await?;
        Ok(push)
    }
    .await;
    settle(pool, user_id, action, result);
}

fn event_name(event: EpisodeEvent) -> &'static str {
    match event {
        EpisodeEvent::Progress(_) => "progress",
        EpisodeEvent::Completed => "completed",
        EpisodeEvent::New => "new",
    }
}

// Episode event, skipped for episodes of feeds that aren't shared (private, local, YouTube).
dialect_fn!([episode_pg, episode_lite]
    pub async fn(pool: &DatabasePool, conn: &mut q::Conn, user_id: i32, episode_id: i32, event: EpisodeEvent)  {
    let result: AppResult<bool> = async {
        let mut sp = conn.begin().await?;
        let mut push = false;
        if let Some(ep) = q::pick(db::pg_episode_for_event, db::lite_episode_for_event)(&mut sp, user_id, episode_id).await?.filter(|e| e.shareable) {
            push = q::pick(db::pg_log_episode, db::lite_log_episode)(&mut sp, user_id, episode_id, &ep, event).await?;
        }
        sp.commit().await?;
        Ok(push)
    }
    .await;
    settle(pool, user_id, event_name(event), result);
});

pub async fn episode_mysql(pool: &DatabasePool, conn: &mut MySqlConnection, user_id: i32, episode_id: i32, event: EpisodeEvent) {
    let result: AppResult<bool> = async {
        let mut sp = conn.begin().await?;
        let mut push = false;
        if let Some(ep) = db::mysql_episode_for_event(&mut sp, user_id, episode_id).await?.filter(|e| e.shareable) {
            push = db::mysql_log_episode(&mut sp, user_id, episode_id, &ep, event).await?;
        }
        sp.commit().await?;
        Ok(push)
    }
    .await;
    settle(pool, user_id, event_name(event), result);
}
