// Remote → local (D4): the one path that applies gpodder episode actions and subscription
// changes, whether they came from a client of the Pinepods server or an external target.
// State changes go through the same DB functions as the UI, passing the origin, so they aren't
// logged as Pinepods' own changes and never echo back.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use tracing::{debug, warn};

use super::model::{ActionKind, EpisodeAction};
use super::resolver::{resolve, ResolveKey};
use super::{sync_enabled, SyncOrigin};
use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::services::tasks::TaskSpawner;

/// Completion threshold when the user's auto-complete setting is off (0).
const DEFAULT_COMPLETE_WITHIN: i32 = 60;

/// Feed adds fetched at once by queued subscription adds (a first sync can bring hundreds).
const ADD_CONCURRENCY: usize = 3;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ApplySummary {
    pub received: usize,
    pub matched: usize,
    /// Episodes whose local state was changed.
    pub applied: usize,
    /// Episodes whose newest action was older than the local state (last-write-wins).
    pub older: usize,
}

/// Apply episode actions for `user_id`. Actions are resolved in one batch; for an external
/// target they're also written to the log as `target:<id>` (matched or not), which is how the
/// internal server's clients see them and how rematch finds them later. Clients of the internal
/// server have already logged theirs.
pub async fn apply_episode_actions(db: &DatabasePool, user_id: i32, actions: &[EpisodeAction], origin: SyncOrigin) -> AppResult<ApplySummary> {
    let keys: Vec<ResolveKey> = actions.iter().map(ResolveKey::of).collect();
    let resolved = resolve(db, user_id, &keys).await?;
    if let (SyncOrigin::Target(_), Some(value)) = (origin, origin.log_value()) {
        db.sync_log_remote_actions(user_id, actions, &resolved, &value).await?;
    }
    apply_resolved(db, user_id, actions, &resolved, origin).await
}

/// Apply already-resolved actions (`resolved[i]` is `actions[i]`'s EpisodeID). For callers that
/// log the actions themselves (the gpodder server, rematch).
///
/// - Only `play` and `new` change state; `download`/`delete` are ignored (N2).
/// - Per episode only the newest action (by action time) counts; on a tie the later one wins.
/// - Last-write-wins: it's applied only if newer than the local `ListenDate` (progress or
///   completion time). Positions can go backwards.
/// - `play` at or past `total - AutoCompleteSeconds` (60s when that's off) completes the
///   episode; `total` is the action's, else the local duration. A local duration of 0 is
///   replaced by the action's total.
/// - `new` uncompletes and resets the position.
pub async fn apply_resolved(
    db: &DatabasePool,
    user_id: i32,
    actions: &[EpisodeAction],
    resolved: &[Option<i32>],
    origin: SyncOrigin,
) -> AppResult<ApplySummary> {
    let mut summary = ApplySummary { received: actions.len(), matched: resolved.iter().flatten().count(), ..Default::default() };

    let mut newest: HashMap<i32, &EpisodeAction> = HashMap::new();
    for (action, episode_id) in actions.iter().zip(resolved) {
        let Some(episode_id) = *episode_id else { continue };
        if !matches!(action.action, ActionKind::Play | ActionKind::New) {
            continue;
        }
        let keep = newest.get(&episode_id).is_none_or(|current| action.timestamp >= current.timestamp);
        if keep {
            newest.insert(episode_id, action);
        }
    }
    if newest.is_empty() {
        return Ok(summary);
    }

    let ids: Vec<i32> = newest.keys().copied().collect();
    let states = db.sync_local_states(user_id, &ids).await?;
    let complete_within = match db.get_user_auto_complete_seconds(user_id).await {
        Ok(secs) if secs > 0 => secs,
        _ => DEFAULT_COMPLETE_WITHIN,
    };

    for (episode_id, action) in newest {
        // Not one of the user's episodes (can't happen through the resolver, but cheap to check).
        let Some(state) = states.get(&episode_id) else { continue };
        if state.listen_date.is_some_and(|local| action.timestamp <= local) {
            summary.older += 1;
            continue;
        }
        match action.action {
            ActionKind::Play => {
                let Some(position) = action.position.filter(|p| *p >= 0) else { continue };
                let local_duration = state.duration.filter(|d| *d > 0);
                let action_total = action.total.filter(|t| *t > 0);
                if let (None, Some(total)) = (local_duration, action_total) {
                    db.set_episode_duration_if_missing(episode_id, total).await?;
                }
                let total = action_total.or(local_duration);
                if total.is_some_and(|t| position >= t - complete_within) {
                    db.mark_episode_completed(episode_id, user_id, false, origin).await?;
                } else {
                    db.set_listen_position(user_id, episode_id, position, action.timestamp, origin).await?;
                }
            }
            ActionKind::New => db.mark_episode_uncompleted(episode_id, user_id, false, origin).await?,
            ActionKind::Download | ActionKind::Delete => continue,
        }
        // The change is as of the action's time, not ours, for the next comparison.
        db.sync_set_listen_date(user_id, episode_id, action.timestamp).await?;
        summary.applied += 1;
    }
    debug!(
        "Applied episode actions for user {}: {} received, {} matched, {} applied, {} older than local state",
        user_id, summary.received, summary.matched, summary.applied, summary.older
    );
    Ok(summary)
}

/// After new episodes were inserted for `podcast_id`, match the log's unmatched actions for that
/// feed and apply the newest per episode. Covers actions that arrived before the episode was
/// ingested (a client syncs a play right after subscribing; the feed is fetched later).
pub async fn rematch(db: &DatabasePool, podcast_id: i32) -> AppResult<usize> {
    let Some((user_id, feed_url)) = db.sync_podcast_owner(podcast_id).await? else { return Ok(0) };
    if feed_url.is_empty() || !sync_enabled(db, user_id).await {
        return Ok(0);
    }
    let logged = db.sync_unmatched_actions(user_id, &feed_url).await?;
    if logged.is_empty() {
        return Ok(0);
    }
    let actions: Vec<EpisodeAction> = logged.iter().map(|l| l.action.clone()).collect();
    let keys: Vec<ResolveKey> = actions.iter().map(ResolveKey::of).collect();
    let resolved = resolve(db, user_id, &keys).await?;
    let matches: Vec<(i64, i32)> = logged.iter().zip(&resolved).filter_map(|(l, id)| id.map(|id| (l.action_id, id))).collect();
    if matches.is_empty() {
        return Ok(0);
    }
    db.sync_set_action_episodes(&matches).await?;
    // These actions are already in the log, so the writes are unlogged.
    let summary = apply_resolved(db, user_id, &actions, &resolved, SyncOrigin::Unlogged).await?;
    debug!("Rematched {} logged actions for podcast {} (user {}), applied {}", matches.len(), podcast_id, user_id, summary.applied);
    Ok(matches.len())
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SubscriptionSummary {
    /// Adds queued for import (already-subscribed and in-flight ones are skipped).
    pub queued: usize,
    pub removed: usize,
    /// URLs in both lists or not http(s); ignored.
    pub rejected: usize,
}

// Subscription adds in flight, so the same feed isn't imported twice for a user when two
// devices (or two sync runs) report it before the first import finishes.
static IN_FLIGHT_ADDS: LazyLock<Mutex<HashSet<(i32, String)>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
static ADD_PERMITS: LazyLock<Arc<tokio::sync::Semaphore>> = LazyLock::new(|| Arc::new(tokio::sync::Semaphore::new(ADD_CONCURRENCY)));

/// Releases an in-flight add when the import task ends, however it ends.
pub struct InFlightAdd(i32, String);

impl Drop for InFlightAdd {
    fn drop(&mut self) {
        IN_FLIGHT_ADDS.lock().unwrap().remove(&(self.0, std::mem::take(&mut self.1)));
    }
}

fn claim_add(user_id: i32, url: &str) -> Option<InFlightAdd> {
    IN_FLIGHT_ADDS
        .lock()
        .unwrap()
        .insert((user_id, url.to_string()))
        .then(|| InFlightAdd(user_id, url.to_string()))
}

/// Limits concurrent sync imports; held by the import task while it fetches.
pub async fn add_permit() -> tokio::sync::OwnedSemaphorePermit {
    ADD_PERMITS.clone().acquire_owned().await.expect("sync add semaphore is never closed")
}

/// Apply subscription changes for `user_id`. Adds are queued as background imports (never
/// fetched inline, D6), deduplicated per (user, url) and skipped when already subscribed.
/// Removes go through the shared unsubscribe cascade. For an external target the changes are
/// also logged as `target:<id>`.
pub async fn apply_subscription_changes(
    db: &DatabasePool,
    tasks: &TaskSpawner,
    user_id: i32,
    add: &[String],
    remove: &[String],
    origin: SyncOrigin,
) -> AppResult<SubscriptionSummary> {
    let mut summary = SubscriptionSummary::default();
    let clean = |urls: &[String]| -> Vec<String> {
        let mut seen = HashSet::new();
        urls.iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty() && seen.insert(u.clone())).collect()
    };
    let (add, remove) = (clean(add), clean(remove));
    let both: HashSet<&String> = add.iter().filter(|u| remove.contains(u)).collect();
    if !both.is_empty() {
        warn!("Ignoring {} subscription changes for user {} that both add and remove the same feed", both.len(), user_id);
    }
    let is_http = |u: &str| {
        let l = u.to_ascii_lowercase();
        l.starts_with("http://") || l.starts_with("https://")
    };
    let (add, not_http): (Vec<String>, Vec<String>) = add.iter().filter(|u| !both.contains(u)).cloned().partition(|u| is_http(u));
    let remove: Vec<String> = remove.iter().filter(|u| !both.contains(u)).cloned().collect();
    summary.rejected = both.len() + not_http.len();

    if let (SyncOrigin::Target(_), Some(value)) = (origin, origin.log_value()) {
        db.sync_log_remote_subscriptions(user_id, &add, "add", &value).await?;
        db.sync_log_remote_subscriptions(user_id, &remove, "remove", &value).await?;
    }

    for url in &remove {
        db.remove_podcast_by_url(user_id, url, origin).await?;
        summary.removed += 1;
    }
    for url in add {
        if db.podcast_exists_for_user(user_id, &url).await? {
            continue;
        }
        let Some(claim) = claim_add(user_id, &url) else { continue };
        tasks.spawn_sync_add_podcast(user_id, url, origin, claim).await?;
        summary.queued += 1;
    }
    Ok(summary)
}
