// Event-driven push to external targets. A local change (or a client upload) for a user with an
// enabled target schedules an incremental run, debounced per user: it fires once the user has
// been quiet for `quiet`, but no later than `max_wait` after the first unpushed change, so
// playback (a progress save every few seconds) still reaches the target while it continues.
//
// One timer per user, in memory (single process). Scheduling never waits on the database or a
// running sync; the engine's per-user lock serialises the run with any other.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, Weak};
use std::time::Duration;

use tokio::time::Instant;

use super::engine;
use crate::database::DatabasePool;
use crate::services::tasks::TaskSpawner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PushDelays {
    pub quiet: Duration,
    pub max_wait: Duration,
}

impl Default for PushDelays {
    fn default() -> Self {
        PushDelays { quiet: Duration::from_secs(30), max_wait: Duration::from_secs(120) }
    }
}

impl PushDelays {
    /// The defaults, overridden by `PINEPODS_SYNC_PUSH_DELAY` / `PINEPODS_SYNC_PUSH_MAX_DELAY`
    /// (seconds).
    pub fn from_env() -> Self {
        let secs = |name: &str| std::env::var(name).ok().and_then(|v| v.trim().parse::<u64>().ok()).map(Duration::from_secs);
        let d = PushDelays::default();
        let quiet = secs("PINEPODS_SYNC_PUSH_DELAY").unwrap_or(d.quiet);
        let max_wait = secs("PINEPODS_SYNC_PUSH_MAX_DELAY").unwrap_or(d.max_wait).max(quiet);
        PushDelays { quiet, max_wait }
    }

    fn due(&self, p: &Pending) -> Instant {
        (p.last + self.quiet).min(p.first + self.max_wait)
    }
}

struct Pending {
    first: Instant,
    last: Instant,
}

#[derive(Clone)]
struct Pusher {
    // Weak: a test app's pool shouldn't outlive the app.
    tasks: Weak<TaskSpawner>,
    delays: PushDelays,
}

// Keyed by database (one in production; tests run several apps in one process).
static PUSHERS: LazyLock<Mutex<HashMap<String, Pusher>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static PENDING: LazyLock<Mutex<HashMap<(String, i32), Pending>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Turn on event-driven pushes for this database. Called once at startup.
pub fn init(tasks: &std::sync::Arc<TaskSpawner>, delays: PushDelays) {
    let key = tasks.db_pool().instance_key();
    PUSHERS.lock().unwrap().insert(key, Pusher { tasks: std::sync::Arc::downgrade(tasks), delays });
}

/// Schedule a push for the user (the caller knows an enabled target exists). Cheap, never fails,
/// and a burst of calls shares one timer.
pub fn schedule(db: &DatabasePool, user_id: i32) {
    let key = db.instance_key();
    let Some(pusher) = PUSHERS.lock().unwrap().get(&key).cloned() else { return };
    let Ok(runtime) = tokio::runtime::Handle::try_current() else { return };
    let now = Instant::now();
    let pending_key = (key, user_id);
    {
        let mut pending = PENDING.lock().unwrap();
        if let Some(p) = pending.get_mut(&pending_key) {
            p.last = now;
            return;
        }
        pending.insert(pending_key.clone(), Pending { first: now, last: now });
    }
    runtime.spawn(async move {
        loop {
            let due = {
                let mut pending = PENDING.lock().unwrap();
                let Some(p) = pending.get(&pending_key) else { return };
                let due = pusher.delays.due(p);
                if due <= Instant::now() {
                    // Changes from here on start a new timer.
                    pending.remove(&pending_key);
                    break;
                }
                due
            };
            tokio::time::sleep_until(due).await;
        }
        let Some(tasks) = pusher.tasks.upgrade() else { return };
        let db = tasks.db_pool().clone();
        tracing::debug!("Pushing local changes for user {}", user_id);
        if let Err(e) = engine::run_due_for_user(&db, &tasks, user_id).await {
            tracing::warn!("Push for user {} not started: {}", user_id, e);
        }
    });
}

/// Whether a push for the user is waiting to fire.
pub fn pending(db: &DatabasePool, user_id: i32) -> bool {
    PENDING.lock().unwrap().contains_key(&(db.instance_key(), user_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn due_is_quiet_after_last_change_capped_by_first() {
        let d = PushDelays { quiet: Duration::from_secs(30), max_wait: Duration::from_secs(120) };
        let t0 = Instant::now();
        let burst = Pending { first: t0, last: t0 + Duration::from_secs(5) };
        assert_eq!(d.due(&burst), t0 + Duration::from_secs(35));
        let playing = Pending { first: t0, last: t0 + Duration::from_secs(110) };
        assert_eq!(d.due(&playing), t0 + Duration::from_secs(120));
    }
}
