// Local writes → sync log. The DB functions behind subscribe, unsubscribe, progress,
// complete and uncomplete call these with their origin, inside their own transaction where they
// have one. An event never fails the write it belongs to: it runs in a savepoint (on PG a failed
// statement would otherwise abort the caller's transaction) and a failure is logged and dropped.

use sqlx::{Connection, MySqlConnection, PgConnection};

use super::db;
use super::{sync_enabled, SyncOrigin};
use crate::database::DatabasePool;
use crate::error::AppResult;

pub use super::db::EpisodeEvent;

/// Whether a write by `origin` for `user_id` should be logged: Pinepods' own changes, for users
/// who sync. Check this before starting work that only exists for the event.
pub async fn should_log(db: &DatabasePool, user_id: i32, origin: SyncOrigin) -> bool {
    origin == SyncOrigin::Pinepods && sync_enabled(db, user_id).await
}

fn dropped(what: &str, user_id: i32, e: impl std::fmt::Display) {
    tracing::warn!("Sync event ({what}) for user {user_id} not recorded: {e}");
}

/// Subscription `add`/`remove` for a shareable feed (the caller checks `shareable_feed`).
pub async fn subscription_pg(conn: &mut PgConnection, user_id: i32, feed_url: &str, action: &str) {
    let result: AppResult<()> = async {
        let mut sp = conn.begin().await?;
        db::pg_log_subscription(&mut sp, user_id, feed_url, action).await?;
        sp.commit().await?;
        Ok(())
    }
    .await;
    if let Err(e) = result {
        dropped(action, user_id, e);
    }
}

pub async fn subscription_mysql(conn: &mut MySqlConnection, user_id: i32, feed_url: &str, action: &str) {
    let result: AppResult<()> = async {
        let mut sp = conn.begin().await?;
        db::mysql_log_subscription(&mut sp, user_id, feed_url, action).await?;
        sp.commit().await?;
        Ok(())
    }
    .await;
    if let Err(e) = result {
        dropped(action, user_id, e);
    }
}

fn event_name(event: EpisodeEvent) -> &'static str {
    match event {
        EpisodeEvent::Progress(_) => "progress",
        EpisodeEvent::Completed => "completed",
        EpisodeEvent::New => "new",
    }
}

/// Episode event, skipped for episodes of feeds that aren't shared (private, local, YouTube).
pub async fn episode_pg(conn: &mut PgConnection, user_id: i32, episode_id: i32, event: EpisodeEvent) {
    let result: AppResult<()> = async {
        let mut sp = conn.begin().await?;
        if let Some(ep) = db::pg_episode_for_event(&mut sp, user_id, episode_id).await?.filter(|e| e.shareable) {
            db::pg_log_episode(&mut sp, user_id, episode_id, &ep, event).await?;
        }
        sp.commit().await?;
        Ok(())
    }
    .await;
    if let Err(e) = result {
        dropped(event_name(event), user_id, e);
    }
}

pub async fn episode_mysql(conn: &mut MySqlConnection, user_id: i32, episode_id: i32, event: EpisodeEvent) {
    let result: AppResult<()> = async {
        let mut sp = conn.begin().await?;
        if let Some(ep) = db::mysql_episode_for_event(&mut sp, user_id, episode_id).await?.filter(|e| e.shareable) {
            db::mysql_log_episode(&mut sp, user_id, episode_id, &ep, event).await?;
        }
        sp.commit().await?;
        Ok(())
    }
    .await;
    if let Err(e) = result {
        dropped(event_name(event), user_id, e);
    }
}
