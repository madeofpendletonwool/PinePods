// Feeds a sync app or external server subscribed to that Pinepods couldn't import. They're kept
// per user, retried with backoff, and listed with the feed problems until they import, or the user
// removes (unsubscribes everywhere) or dismisses them.

use serde::Serialize;
use tracing::{debug, warn};

use super::SyncOrigin;
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::feed_health::{FeedErrorKind, PROBLEMS_LINK};
use crate::services::task_manager::{MsgSeverity, TaskManager};
use crate::services::tasks::TaskSpawner;

/// Activity message category: title = the feed URL, body = the error kind.
pub const CATEGORY_GAVE_UP: &str = "sync_import_failed";

/// Wait before the next attempt after `attempts` failures; `None` gives up.
pub fn retry_delay(attempts: i32) -> Option<chrono::Duration> {
    match attempts {
        ..=0 => Some(chrono::Duration::zero()),
        1 => Some(chrono::Duration::hours(1)),
        2 => Some(chrono::Duration::hours(6)),
        3..=9 => Some(chrono::Duration::hours(24)),
        _ => None,
    }
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct ImportFailure {
    pub id: i32,
    pub feed_url: String,
    /// Where the subscription came from: `client` (an app on the PinePods gpodder server) or
    /// `target` (the external sync server).
    pub source: String,
    pub kind: FeedErrorKind,
    pub error: Option<String>,
    pub attempts: i32,
    /// Unix seconds.
    pub first_failed_at: i64,
    pub last_attempt_at: i64,
    /// `None`: Pinepods stopped retrying.
    pub next_attempt_at: Option<i64>,
    /// The stored origin, used when retrying.
    #[serde(skip)]
    pub origin: String,
}

pub fn origin_value(origin: SyncOrigin) -> String {
    origin.log_value().unwrap_or_else(|| "pinepods".into())
}

pub fn parse_origin(value: &str) -> SyncOrigin {
    match value {
        "client" => SyncOrigin::Client,
        "pinepods" => SyncOrigin::Pinepods,
        v => v.strip_prefix("target:").and_then(|id| id.parse().ok()).map(SyncOrigin::Target).unwrap_or(SyncOrigin::Client),
    }
}

/// An import failed: remember it, schedule the next attempt, and tell the user once Pinepods
/// gives up.
pub async fn failed(db: &DatabasePool, tasks: &TaskManager, user_id: i32, feed_url: &str, origin: SyncOrigin, error: &AppError) {
    let kind = FeedErrorKind::of(error);
    match db.sync_import_failed(user_id, feed_url, &origin_value(origin), kind, &error.to_string()).await {
        Ok(gave_up) if gave_up => {
            let push = tasks
                .push_notification(
                    user_id,
                    CATEGORY_GAVE_UP,
                    MsgSeverity::Warning,
                    feed_url.to_string(),
                    Some(kind.as_str().to_string()),
                    None,
                    None,
                    None,
                    Some(PROBLEMS_LINK.to_string()),
                )
                .await;
            if let Err(e) = push {
                warn!("Telling user {} about a failed import failed: {}", user_id, e);
            }
        }
        Ok(_) => {}
        Err(e) => warn!("Recording the failed import of {} for user {} failed: {}", feed_url, user_id, e),
    }
}

/// Start the imports whose next attempt is due. Runs with the scheduled sync.
pub async fn retry_due(db: &DatabasePool, tasks: &TaskSpawner) -> AppResult<usize> {
    let due = db.sync_imports_due(50).await?;
    let mut started = 0;
    for (user_id, feed_url, origin) in due {
        if super::apply::queue_import(tasks, user_id, &feed_url, parse_origin(&origin)).await {
            started += 1;
        }
    }
    if started > 0 {
        debug!("Retrying {} failed sync imports", started);
    }
    Ok(started)
}
