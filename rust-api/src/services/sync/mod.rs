// Sync core. Local writes are recorded in the gpodder
// subscription/episode-action logs as they happen (events), and remote changes are applied
// through one path (apply), with episodes matched by one resolver. The logs double as the outbox
// for external sync targets.
//
// Origins: every write that can reach the logs says where it came from. Only `Pinepods` writes
// event rows; `Client` changes were already logged by the gpodder server, and apply logs a
// target's changes itself as `target:<id>`. So nothing is echoed back to where it came from.

pub mod apply;
pub mod db;
pub mod engine;
pub mod engine_db;
pub mod events;
pub mod model;
pub mod resolver;
pub mod targets;

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use crate::database::DatabasePool;

pub use apply::{apply_episode_actions, apply_subscription_changes, rematch, ApplySummary};
pub use model::{ActionKind, EpisodeAction};
pub use resolver::{resolve, ResolveKey};

/// Where a change came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOrigin {
    /// Made in Pinepods itself (web/mobile UI, API, auto-complete). Logged as `pinepods`.
    Pinepods,
    /// Uploaded by a gpodder client of the Pinepods server; already in the log.
    Client,
    /// Pulled from an external sync target (`SyncTargets.TargetID`). Logged by apply.
    Target(i32),
    /// Not a change other devices should hear about: the pre-overhaul sync paths, account
    /// deletion, housekeeping. Never logged.
    Unlogged,
}

impl SyncOrigin {
    /// The `Origin` column value for rows this origin writes to the log.
    pub fn log_value(self) -> Option<String> {
        match self {
            SyncOrigin::Pinepods => Some("pinepods".into()),
            SyncOrigin::Client => Some("client".into()),
            SyncOrigin::Target(id) => Some(format!("target:{id}")),
            SyncOrigin::Unlogged => None,
        }
    }
}

/// The pinepods device: `GpodderDevices.SystemRole` value and preferred name/caption/type.
pub const PINEPODS_ROLE: &str = "pinepods";
pub const PINEPODS_DEVICE_NAME: &str = "pinepods";
pub const PINEPODS_DEVICE_CAPTION: &str = "PinePods";
pub const PINEPODS_DEVICE_TYPE: &str = "server";

/// Feeds that can be shared with other devices: http(s), no stored credentials. YouTube
/// channels are stored with an https channel URL but aren't feeds, so they're left out too.
pub fn shareable_feed(feed_url: &str, username: Option<&str>, password: Option<&str>, is_youtube: bool) -> bool {
    let url = feed_url.trim().to_ascii_lowercase();
    (url.starts_with("http://") || url.starts_with("https://"))
        && username.is_none_or(str::is_empty)
        && password.is_none_or(str::is_empty)
        && !is_youtube
}

// Progress saves arrive every few seconds during playback, so whether a user syncs at all is
// cached. Settings changes call `invalidate_sync_enabled`; the TTL covers anything that doesn't.
const SYNC_ENABLED_TTL: Duration = Duration::from_secs(60);
static SYNC_ENABLED: LazyLock<Mutex<HashMap<i32, (bool, Instant)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// True if the user uses the Pinepods gpodder server or has an enabled external target.
/// A lookup failure counts as "no" (and is logged): events are best effort.
pub async fn sync_enabled(db: &DatabasePool, user_id: i32) -> bool {
    if let Some((enabled, at)) = SYNC_ENABLED.lock().unwrap().get(&user_id).copied() {
        if at.elapsed() < SYNC_ENABLED_TTL {
            return enabled;
        }
    }
    match db.sync_enabled_uncached(user_id).await {
        Ok(enabled) => {
            SYNC_ENABLED.lock().unwrap().insert(user_id, (enabled, Instant::now()));
            enabled
        }
        Err(e) => {
            tracing::warn!("Checking whether user {} syncs failed: {}", user_id, e);
            false
        }
    }
}

/// Forget the cached `sync_enabled` answer after the user's sync settings change.
pub fn invalidate_sync_enabled(user_id: i32) {
    SYNC_ENABLED.lock().unwrap().remove(&user_id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shareable_feeds() {
        assert!(shareable_feed("https://example.com/feed.xml", None, Some(""), false));
        assert!(shareable_feed("HTTP://example.com/feed", Some(""), None, false));
        assert!(!shareable_feed("https://example.com/feed", Some("bob"), Some("pw"), false));
        assert!(!shareable_feed("file:///podcasts/local", None, None, false));
        assert!(!shareable_feed("https://www.youtube.com/channel/UC123", None, None, true));
    }

    #[test]
    fn origin_log_values() {
        assert_eq!(SyncOrigin::Pinepods.log_value().as_deref(), Some("pinepods"));
        assert_eq!(SyncOrigin::Target(7).log_value().as_deref(), Some("target:7"));
        assert_eq!(SyncOrigin::Unlogged.log_value(), None);
    }
}
