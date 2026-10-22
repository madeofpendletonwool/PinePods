// External sync: one run against a user's gpodder server or Nextcloud. In order: log in, pull
// subscriptions and episode actions and apply them as the target's origin (so they're never
// pushed back), push the log rows that didn't come from the target, then save the server's pull
// cursors. Any failure ends the run: the pull cursors stay put, and a push cursor has only moved
// past batches the server accepted.
//
// A full sync (new target, or "Force full sync") pulls from 0 and pushes the current library and
// listening state instead of the log. It merges: nothing is removed on either side.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock, Mutex};

use chrono::{NaiveDateTime, Utc};
use futures::stream::{self, StreamExt};
use serde::Serialize;
use tracing::{debug, warn};

use super::apply::{apply_resolved, apply_subscription_changes, in_flight_adds};
use super::engine_db::{Outbox, SyncTargetRow, TargetKind};
use super::model::{ActionKind, EpisodeAction, OutboundAction};
use super::resolver::{resolve, ResolveKey};
use super::targets::gpodder::{device_to_use, GpodderTarget, RemoteDevice};
use super::targets::nextcloud::NextcloudTarget;
use super::targets::SyncTarget;
use super::SyncOrigin;
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::scheduler::failure_backoff;
use crate::services::tasks::{ProgressReporter, TaskSpawner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Incremental,
    Full,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct RunSummary {
    pub full: bool,
    pub subscriptions_queued: usize,
    pub subscriptions_removed: usize,
    pub subscriptions_pushed: usize,
    /// Pulled actions that weren't already known locally.
    pub actions_pulled: usize,
    pub actions_applied: usize,
    pub actions_pushed: usize,
}

impl RunSummary {
    fn add(&mut self, o: &RunSummary) {
        self.full |= o.full;
        self.subscriptions_queued += o.subscriptions_queued;
        self.subscriptions_removed += o.subscriptions_removed;
        self.subscriptions_pushed += o.subscriptions_pushed;
        self.actions_pulled += o.actions_pulled;
        self.actions_applied += o.actions_applied;
        self.actions_pushed += o.actions_pushed;
    }
}

// One run per user at a time (single process).
static USER_LOCKS: LazyLock<Mutex<HashMap<i32, Arc<tokio::sync::Mutex<()>>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn user_lock(user_id: i32) -> Arc<tokio::sync::Mutex<()>> {
    USER_LOCKS.lock().unwrap().entry(user_id).or_default().clone()
}

/// Sync every enabled target of the user. None when the user has none.
pub async fn run_for_user(db: &DatabasePool, tasks: &TaskSpawner, user_id: i32, mode: RunMode) -> AppResult<Option<RunSummary>> {
    let targets: Vec<SyncTargetRow> = db.sync_targets_for_user(user_id).await?.into_iter().filter(|t| t.enabled).collect();
    if targets.is_empty() {
        return Ok(None);
    }
    let mut total = RunSummary::default();
    let mut failed = None;
    for t in targets {
        match run(db, tasks, user_id, t.target_id, mode).await {
            Ok(s) => total.add(&s),
            Err(e) => failed = failed.or(Some(e)),
        }
    }
    match failed {
        Some(e) => Err(e),
        None => Ok(Some(total)),
    }
}

impl SyncTargetRow {
    /// Still waiting out its failure backoff. Only automatic runs honour it.
    pub fn in_backoff(&self, now: NaiveDateTime) -> bool {
        self.consecutive_failures > 0 && self.last_attempt_at.is_some_and(|at| at + failure_backoff(self.consecutive_failures) > now)
    }
}

/// Users synced at once by the scheduled sync.
const SCHEDULED_CONCURRENCY: usize = 4;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScheduledSummary {
    pub synced: usize,
    pub failed: usize,
    /// Skipped while in failure backoff.
    pub held: usize,
}

/// The scheduled sync: every enabled target that isn't in failure backoff, a few users at a time.
pub async fn run_due(db: &DatabasePool, tasks: &TaskSpawner) -> AppResult<ScheduledSummary> {
    let now = Utc::now().naive_utc();
    let (held, due): (Vec<_>, Vec<_>) = db.sync_enabled_targets().await?.into_iter().partition(|t| t.in_backoff(now));
    for t in &held {
        debug!("Sync for user {} held back after {} failures", t.user_id, t.consecutive_failures);
    }
    let ok: Vec<bool> = stream::iter(due)
        .map(|t| async move { run(db, tasks, t.user_id, t.target_id, RunMode::Incremental).await.is_ok() })
        .buffer_unordered(SCHEDULED_CONCURRENCY)
        .collect()
        .await;
    let synced = ok.iter().filter(|o| **o).count();
    Ok(ScheduledSummary { synced, failed: ok.len() - synced, held: held.len() })
}

/// An incremental run of the user's targets that aren't in failure backoff (the push after a
/// local change). Failures are recorded and logged by `run`.
pub async fn run_due_for_user(db: &DatabasePool, tasks: &TaskSpawner, user_id: i32) -> AppResult<()> {
    let now = Utc::now().naive_utc();
    for t in db.sync_targets_for_user(user_id).await? {
        if t.enabled && !t.in_backoff(now) {
            let _ = run(db, tasks, user_id, t.target_id, RunMode::Incremental).await;
        }
    }
    Ok(())
}

/// One run against one of the user's targets. Records LastAttemptAt, then LastSuccessAt or
/// LastError + ConsecutiveFailures. The returned error is the stored one (no credentials).
pub async fn run(db: &DatabasePool, tasks: &TaskSpawner, user_id: i32, target_id: i32, mode: RunMode) -> AppResult<RunSummary> {
    run_reporting(db, tasks, user_id, target_id, mode, None).await
}

/// `run`, reporting each step to a task (the first sync of a new target).
pub async fn run_reporting(
    db: &DatabasePool,
    tasks: &TaskSpawner,
    user_id: i32,
    target_id: i32,
    mode: RunMode,
    progress: Option<&dyn ProgressReporter>,
) -> AppResult<RunSummary> {
    let lock = user_lock(user_id);
    let _guard = lock.lock().await;
    // Read under the lock: a run that just finished may have moved the cursors.
    let target = db
        .sync_target(target_id)
        .await?
        .filter(|t| t.user_id == user_id)
        .ok_or_else(|| AppError::not_found("Sync target not found"))?;
    db.sync_target_record_attempt(target_id).await?;

    let mut secret = String::new();
    let result = async {
        secret = db
            .decrypt_password(&target.encrypted_secret)
            .await
            .map_err(|_| AppError::internal("The stored password for the sync server can't be decrypted; enter it again"))?;
        run_locked(db, tasks, &target, &secret, mode, progress).await
    }
    .await;

    match result {
        Ok(summary) => {
            debug!("External sync for user {} ({}): {:?}", user_id, target.kind.as_str(), summary);
            Ok(summary)
        }
        Err(e) => {
            let message = scrub(&error_message(&e), &secret);
            if let Err(db_err) = db.sync_target_record_failure(target_id, &message).await {
                warn!("Recording the sync failure for user {} failed: {}", user_id, db_err);
            }
            warn!("External sync for user {} ({}) failed: {}", user_id, target.kind.as_str(), message);
            Err(AppError::internal(message))
        }
    }
}

/// The message of an error, without the variant's prefix.
pub fn error_message(e: &AppError) -> String {
    match e {
        AppError::Internal(m)
        | AppError::Auth(m)
        | AppError::BadRequest(m)
        | AppError::NotFound(m)
        | AppError::Validation(m)
        | AppError::ServiceUnavailable(m) => m.clone(),
        other => other.to_string(),
    }
}

/// Belt and braces: the messages are built without credentials, but never store the secret.
fn scrub(message: &str, secret: &str) -> String {
    if secret.is_empty() {
        message.to_string()
    } else {
        message.replace(secret, "***")
    }
}

fn target_for(t: &SyncTargetRow, secret: &str) -> AppResult<Box<dyn SyncTarget>> {
    Ok(match t.kind {
        TargetKind::Gpodder => Box::new(GpodderTarget::new(&t.url, &t.username, secret, t.device_name.clone())?),
        TargetKind::Nextcloud => Box::new(NextcloudTarget::new(&t.url, &t.username, secret)?),
    })
}

async fn step(progress: Option<&dyn ProgressReporter>, percent: f64, message: &str) {
    if let Some(p) = progress {
        let _ = p.update_progress(percent, Some(message.to_string())).await;
    }
}

async fn run_locked(
    db: &DatabasePool,
    tasks: &TaskSpawner,
    t: &SyncTargetRow,
    secret: &str,
    mode: RunMode,
    progress: Option<&dyn ProgressReporter>,
) -> AppResult<RunSummary> {
    let user_id = t.user_id;
    let full = mode == RunMode::Full || t.needs_full_sync;
    let origin = SyncOrigin::Target(t.target_id);
    let origin_value = origin.log_value().unwrap_or_default();
    let mut summary = RunSummary { full, ..Default::default() };

    let mut remote = target_for(t, secret)?;
    step(progress, 10.0, "Signing in to the sync server").await;
    remote.login().await?;
    if let Some(device) = remote.prepare_device(full).await? {
        db.sync_target_set_device(t.target_id, &device).await?;
    }

    // A target that never synced gets the current state; the log before it is history.
    let fresh = full && t.last_success_at.is_none();
    let unpushed: HashSet<String> = if fresh {
        HashSet::new()
    } else {
        db.sync_pending_subscriptions(user_id, &origin_value, t.subs_push_cursor).await?.into_iter().map(|p| p.url).collect()
    };

    // Subscriptions. A local change the target hasn't had yet wins; it's pushed below.
    step(progress, 25.0, "Fetching subscriptions").await;
    let subs = remote.pull_subscriptions(if full { 0 } else { t.subs_pull_cursor }).await?;
    let mut local = db.sync_user_feed_urls(user_id).await?;
    local.extend(in_flight_adds(user_id));
    let add: Vec<String> = subs.add.iter().filter(|u| !unpushed.contains(*u) && !local.contains(*u)).cloned().collect();
    let remove: Vec<String> = if full {
        Vec::new()
    } else {
        subs.remove.iter().filter(|u| !unpushed.contains(*u) && local.contains(*u)).cloned().collect()
    };
    if !add.is_empty() || !remove.is_empty() {
        let applied = apply_subscription_changes(db, tasks, user_id, &add, &remove, origin).await?;
        summary.subscriptions_queued = applied.queued;
        summary.subscriptions_removed = applied.removed;
    }

    // Episode actions.
    step(progress, 45.0, "Fetching listening progress").await;
    let pulled = remote.pull_actions(if full { 0 } else { t.actions_pull_cursor }).await?;
    if pulled.unreadable > 0 {
        debug!("Skipped {} unreadable episode actions from user {}'s sync server", pulled.unreadable, user_id);
    }
    let fresh_actions = drop_echoes(db, user_id, remote.own_device(), pulled.actions).await?;
    summary.actions_pulled = fresh_actions.actions.len();
    if !fresh_actions.actions.is_empty() {
        db.sync_log_remote_actions(user_id, &fresh_actions.actions, &fresh_actions.resolved, &origin_value).await?;
        summary.actions_applied = apply_resolved(db, user_id, &fresh_actions.actions, &fresh_actions.resolved, origin).await?.applied;
    }

    // Push.
    step(progress, 70.0, "Sending local changes").await;
    if !fresh {
        summary.subscriptions_pushed += push_subscription_log(db, &*remote, t, &origin_value).await?;
        summary.actions_pushed += push_action_log(db, &*remote, t, &origin_value).await?;
    }
    if full {
        db.sync_reset_push_cursors(t.target_id, user_id).await?;
        let batch = remote.push_batch_size();
        let on_remote: HashSet<&str> = subs.add.iter().map(String::as_str).collect();
        let to_add: Vec<String> =
            db.sync_shareable_feed_urls(user_id).await?.into_iter().filter(|u| !on_remote.contains(u.as_str())).collect();
        for chunk in to_add.chunks(batch) {
            remote.push_subscriptions(chunk, &[]).await?;
        }
        summary.subscriptions_pushed += to_add.len();
        let state: Vec<OutboundAction> = db.sync_episode_states(user_id).await?.iter().filter_map(|(a, d)| a.outbound(*d)).collect();
        for chunk in state.chunks(batch) {
            remote.push_actions(chunk).await?;
        }
        summary.actions_pushed += state.len();
    }

    db.sync_target_record_success(t.target_id, subs.timestamp, pulled.timestamp, full).await?;
    Ok(summary)
}

struct Resolved {
    actions: Vec<EpisodeAction>,
    resolved: Vec<Option<i32>>,
}

/// Leave out pulled actions Pinepods already has: sent from its own device, already in the log
/// (pushed from here, or sent again by a `>=` cursor), or exactly the local state (the echo of a
/// full sync's state push on servers without devices).
async fn drop_echoes(db: &DatabasePool, user_id: i32, own_device: Option<&str>, actions: Vec<EpisodeAction>) -> AppResult<Resolved> {
    let actions: Vec<EpisodeAction> =
        actions.into_iter().filter(|a| own_device.is_none() || a.device.as_deref() != own_device).collect();
    let known = db.sync_known_actions(user_id, &actions).await?;
    let actions: Vec<EpisodeAction> = actions.into_iter().zip(known).filter(|(_, k)| !k).map(|(a, _)| a).collect();
    let keys: Vec<ResolveKey> = actions.iter().map(ResolveKey::of).collect();
    let resolved = resolve(db, user_id, &keys).await?;
    let ids: Vec<i32> = resolved.iter().flatten().copied().collect();
    let states = db.sync_local_states(user_id, &ids).await?;
    let (actions, resolved) = actions
        .into_iter()
        .zip(resolved)
        .filter(|(a, id)| {
            let Some(state) = id.and_then(|id| states.get(&id)) else { return true };
            let same_time = state.listen_date.is_some_and(|d| d.timestamp() == a.timestamp.timestamp());
            let same_place = a.position == state.position || (state.completed && a.total.is_some_and(|t| a.position >= Some(t)));
            !(a.action == ActionKind::Play && same_time && same_place)
        })
        .unzip();
    Ok(Resolved { actions, resolved })
}

/// Push the subscription log after the cursor, folded to each URL's last change.
async fn push_subscription_log(db: &DatabasePool, remote: &dyn SyncTarget, t: &SyncTargetRow, origin_value: &str) -> AppResult<usize> {
    let rows = db.sync_pending_subscriptions(t.user_id, origin_value, t.subs_push_cursor).await?;
    if rows.is_empty() {
        return Ok(0);
    }
    let mut last: HashMap<&str, bool> = HashMap::new();
    let mut order: Vec<&str> = Vec::new();
    for r in &rows {
        if last.insert(&r.url, r.add).is_none() {
            order.push(&r.url);
        }
    }
    let add: Vec<String> = order.iter().filter(|u| last[*u]).map(|u| u.to_string()).collect();
    let remove: Vec<String> = order.iter().filter(|u| !last[*u]).map(|u| u.to_string()).collect();
    let batch = remote.push_batch_size();
    for chunk in add.chunks(batch) {
        remote.push_subscriptions(chunk, &[]).await?;
    }
    for chunk in remove.chunks(batch) {
        remote.push_subscriptions(&[], chunk).await?;
    }
    let sent: Vec<(i64, String)> = rows.iter().map(|r| (r.id, r.fingerprint.clone())).collect();
    db.sync_advance_push_cursor(t.target_id, t.user_id, Outbox::Subscriptions, origin_value, t.subs_push_cursor, &sent).await?;
    Ok(add.len() + remove.len())
}

/// Push the episode-action log after the cursor in batches, advancing the cursor after each
/// batch the server accepted. A row rewritten while its batch was in flight is sent again.
async fn push_action_log(db: &DatabasePool, remote: &dyn SyncTarget, t: &SyncTargetRow, origin_value: &str) -> AppResult<usize> {
    let batch = remote.push_batch_size();
    let mut cursor = t.actions_push_cursor;
    let mut pushed = 0;
    let mut stalls = 0;
    loop {
        let rows = db.sync_pending_actions(t.user_id, origin_value, cursor, batch).await?;
        let Some(last) = rows.last().map(|r| r.id) else { break };
        let out: Vec<OutboundAction> = rows.iter().filter_map(|r| r.action.as_ref()?.outbound(r.duration)).collect();
        if !out.is_empty() {
            remote.push_actions(&out).await?;
            pushed += out.len();
        }
        let sent: Vec<(i64, String)> = rows.iter().map(|r| (r.id, r.fingerprint.clone())).collect();
        let next = db.sync_advance_push_cursor(t.target_id, t.user_id, Outbox::Actions, origin_value, cursor, &sent).await?;
        if next < last {
            // Something changed under us; resend from there, but don't chase a row that keeps
            // changing (playback in progress): the next run picks it up.
            stalls += 1;
            if stalls >= 3 {
                break;
            }
        } else {
            stalls = 0;
            if rows.len() < batch {
                break;
            }
        }
        cursor = next;
    }
    Ok(pushed)
}

/// What a connection test found.
#[derive(Debug, Clone, Default)]
pub struct ConnectionCheck {
    /// The account's devices (gpodder servers only).
    pub devices: Vec<RemoteDevice>,
    /// The device Pinepods would sync as (gpodder servers only).
    pub device_name: Option<String>,
}

/// Check credentials without saving anything or creating a device. gpodder: log in and list the
/// devices; Nextcloud (no login endpoint): pull subscription changes from now, which is empty.
pub async fn test_connection(kind: TargetKind, url: &str, username: &str, password: &str, device: Option<&str>) -> AppResult<ConnectionCheck> {
    let result = async {
        match kind {
            TargetKind::Gpodder => {
                let mut remote = GpodderTarget::new(url, username, password, None)?;
                remote.login().await?;
                let devices = remote.devices().await?;
                let device_name = Some(device_to_use(device, &devices));
                Ok(ConnectionCheck { devices, device_name })
            }
            TargetKind::Nextcloud => {
                let remote = NextcloudTarget::new(url, username, password)?;
                remote.pull_subscriptions(Utc::now().timestamp()).await?;
                Ok(ConnectionCheck::default())
            }
        }
    }
    .await;
    result.map_err(|e| match e {
        AppError::Auth(m) => AppError::Auth(scrub(&m, password)),
        AppError::BadRequest(m) => AppError::BadRequest(scrub(&m, password)),
        other => AppError::internal(scrub(&error_message(&other), password)),
    })
}

#[cfg(test)]
mod tests {
    use super::scrub;

    #[test]
    fn errors_never_keep_the_secret() {
        assert_eq!(scrub("POST /x: HTTP 500 hunter2", "hunter2"), "POST /x: HTTP 500 ***");
        assert_eq!(scrub("GET /y: timed out", ""), "GET /y: timed out");
    }
}
