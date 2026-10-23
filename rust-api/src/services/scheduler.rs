use crate::{
    error::AppResult,
    handlers::{refresh, tasks},
    services::sync::engine,
    AppState,
};
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock};
use chrono::Utc;
use croner::parser::{CronParser, Seconds};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, info, error, warn};

/// Failure backoff shared by feed refresh and external sync: 30 minutes per consecutive failure,
/// at most 24 hours.
pub const BACKOFF_STEP_MINUTES: i64 = 30;
pub const BACKOFF_MAX_STEPS: i32 = 48;

pub fn failure_backoff(failures: i32) -> chrono::Duration {
    chrono::Duration::minutes(BACKOFF_STEP_MINUTES * i64::from(failures.clamp(0, BACKOFF_MAX_STEPS)))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobKind {
    Refresh,
    Sync,
    Nightly,
    Cleanup,
}

impl JobKind {
    fn name(self) -> &'static str {
        match self {
            JobKind::Refresh => "podcast refresh",
            JobKind::Sync => "sync",
            JobKind::Nightly => "nightly tasks",
            JobKind::Cleanup => "cleanup",
        }
    }
}

/// At most one run per job kind at a time; a run that would overlap the previous one is skipped.
#[derive(Default)]
pub struct SingleFlight {
    running: [AtomicBool; 4],
}

impl SingleFlight {
    /// None when a run of `job` was already going.
    pub async fn run<Fut: Future<Output = AppResult<()>>>(&self, job: JobKind, fut: Fut) -> Option<AppResult<()>> {
        struct Done<'a>(&'a AtomicBool);
        impl Drop for Done<'_> {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
            }
        }
        let flag = &self.running[job as usize];
        if flag.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            debug!("Skipping scheduled {}: the previous run is still going", job.name());
            return None;
        }
        let _done = Done(flag);
        Some(fut.await)
    }
}

static JOBS: LazyLock<SingleFlight> = LazyLock::new(SingleFlight::default);

/// Run a scheduled job unless the previous run of its kind is still going.
async fn guarded<Fut: Future<Output = AppResult<()>>>(job: JobKind, fut: Fut) {
    match JOBS.run(job, fut).await {
        None => {}
        Some(Ok(())) => debug!("Scheduled {} completed", job.name()),
        Some(Err(e)) => error!("❌ Scheduled {} failed: {}", job.name(), e),
    }
}

pub struct BackgroundScheduler {
    scheduler: JobScheduler,
}

impl BackgroundScheduler {
    pub async fn new() -> AppResult<Self> {
        let scheduler = JobScheduler::new().await?;
        Ok(Self { scheduler })
    }

    pub async fn start(&self, app_state: Arc<AppState>) -> AppResult<()> {
        info!("🕒 Starting background task scheduler...");

        // Every minute, refresh the feeds that are due. Each feed's next refresh follows how often
        // it publishes, at most PINEPODS_REFRESH_MAX_INTERVAL apart (read now, so a deprecated or
        // invalid setting is reported at startup).
        crate::services::refresh_schedule::max_interval();
        let refresh_state = app_state.clone();
        let refresh_job = Job::new_async("0 * * * * *", move |_uuid, _l| {
            let state = refresh_state.clone();
            Box::pin(async move {
                guarded(JobKind::Refresh, run_refresh_job(&state)).await;
            })
        })?;

        // External sync (gpodder servers, Nextcloud), separate from the refresh. Configurable via
        // PINEPODS_SYNC_CRON (same format); defaults to every 5 minutes. Local changes are also
        // pushed shortly after they happen.
        let sync_cron = std::env::var("PINEPODS_SYNC_CRON")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "0 */5 * * * *".to_string());
        info!("📅 External sync schedule: {}", sync_cron);
        let sync_state = app_state.clone();
        let sync_job = Job::new_async(sync_cron.as_str(), move |_uuid, _l| {
            let state = sync_state.clone();
            Box::pin(async move {
                guarded(JobKind::Sync, run_sync_job(&state)).await;
            })
        })?;

        // Schedule nightly tasks at midnight
        let nightly_state = app_state.clone();
        let nightly_job = Job::new_async("0 0 0 * * *", move |_uuid, _l| {
            let state = nightly_state.clone();
            Box::pin(async move {
                info!("🌙 Running scheduled nightly tasks");
                guarded(JobKind::Nightly, Self::run_nightly_tasks(state.clone())).await;
            })
        })?;

        // Schedule cleanup tasks every 6 hours
        let cleanup_state = app_state.clone();
        let cleanup_job = Job::new_async("0 0 */6 * * *", move |_uuid, _l| {
            let state = cleanup_state.clone();
            Box::pin(async move {
                info!("🧹 Running scheduled cleanup tasks");
                guarded(JobKind::Cleanup, Self::run_cleanup_tasks(state.clone())).await;
            })
        })?;

        // Evaluate user-configured scheduled backups every minute. A poll-and-evaluate job is
        // used (rather than dynamically registering per-user cron jobs) so it survives restarts
        // and automatically picks up schedule/enable changes within a minute.
        let backup_state = app_state.clone();
        let backup_job = Job::new_async("0 * * * * *", move |_uuid, _l| {
            let state = backup_state.clone();
            Box::pin(async move {
                if let Err(e) = Self::run_scheduled_backups(state.clone()).await {
                    error!("❌ Scheduled backup run failed: {}", e);
                }
            })
        })?;

        // Daily yt-dlp self-update (#793). YouTube breaks whenever Google changes its internals;
        // keeping yt-dlp current on a schedule decouples YouTube fixes from PinePods releases.
        // Guarded by the admin auto-update toggle inside the run method.
        let ytdlp_state = app_state.clone();
        let ytdlp_job = Job::new_async("0 30 3 * * *", move |_uuid, _l| {
            let state = ytdlp_state.clone();
            Box::pin(async move {
                if let Err(e) = Self::run_ytdlp_update(state.clone()).await {
                    error!("❌ Scheduled yt-dlp update failed: {}", e);
                }
            })
        })?;

        // Add jobs to scheduler
        self.scheduler.add(refresh_job).await?;
        self.scheduler.add(sync_job).await?;
        self.scheduler.add(nightly_job).await?;
        self.scheduler.add(cleanup_job).await?;
        self.scheduler.add(backup_job).await?;
        self.scheduler.add(ytdlp_job).await?;

        // Start the scheduler
        self.scheduler.start().await?;
        info!("✅ Background task scheduler started successfully");

        Ok(())
    }

    async fn run_nightly_tasks(state: Arc<AppState>) -> AppResult<()> {
        // Call nightly tasks directly
        if let Err(e) = tasks::refresh_hosts_internal(&state).await {
            warn!("⚠️ People sync failed during nightly tasks: {}", e);
        }
        
        if let Err(e) = tasks::auto_complete_episodes_internal(&state).await {
            warn!("⚠️ Auto complete episodes failed during nightly tasks: {}", e);
        }

        if let Err(e) = crate::services::sync::compact::run(&state.db_pool).await {
            warn!("⚠️ Sync log compaction failed during nightly tasks: {}", e);
        }

        // Refresh Discover-page recommendations for users who already have a cached set,
        // so their next visit is instant. New users are generated on demand by the endpoint.
        if let Err(e) = Self::refresh_recommendations_internal(&state).await {
            warn!("⚠️ Recommendation refresh failed during nightly tasks: {}", e);
        }

        info!("✅ Nightly tasks completed");
        Ok(())
    }

    // Regenerate and re-cache recommendations for every user with an existing cache entry.
    async fn refresh_recommendations_internal(state: &Arc<AppState>) -> AppResult<()> {
        let user_ids = state.db_pool.get_recommendation_cache_user_ids().await?;
        for uid in user_ids {
            match crate::services::recommendations::generate_recommendations(&state.db_pool, uid, 24).await {
                Ok(recs) => {
                    if let Ok(json) = serde_json::to_string(&recs) {
                        if let Err(e) = state.db_pool.upsert_recommendation_cache(uid, &json).await {
                            warn!("⚠️ Failed to cache recommendations for user {}: {}", uid, e);
                        }
                    }
                }
                Err(e) => warn!("⚠️ Recommendation refresh failed for user {}: {}", uid, e),
            }
        }
        Ok(())
    }

    async fn run_cleanup_tasks(state: Arc<AppState>) -> AppResult<()> {
        // Call cleanup tasks directly
        match tasks::cleanup_tasks_internal(&state).await {
            Ok(_) => {
                info!("✅ Cleanup tasks completed");
            }
            Err(e) => {
                error!("❌ Cleanup tasks failed: {}", e);
                return Err(e);
            }
        }
        Ok(())
    }

    // Evaluate enabled scheduled backups and run a backup if any are due.
    async fn run_scheduled_backups(state: Arc<AppState>) -> AppResult<()> {
        let schedules = state.db_pool.get_enabled_scheduled_backups().await?;
        if schedules.is_empty() {
            return Ok(());
        }

        let now = Utc::now();
        // Seconds::Optional accepts both 5-field and 6-field (with seconds) cron expressions.
        // sloppy_ranges keeps croner 3.x's shorthand steps ("5/5 * * * *") valid: the settings
        // endpoint validates schedules via tokio-cron-scheduler, which still uses croner 3.x,
        // and existing saved schedules may use that form.
        let parser = CronParser::builder()
            .seconds(Seconds::Optional)
            .sloppy_ranges(true)
            .build();

        // Establish a baseline for any schedule that has never run, so we never fire a backlog
        // of missed backups the moment a schedule is first enabled.
        for s in &schedules {
            if s.last_run.is_none() {
                if let Err(e) = state.db_pool.update_scheduled_backup_last_run(s.user_id, now).await {
                    warn!("Failed to set initial backup baseline for user {}: {}", s.user_id, e);
                }
            }
        }

        // Determine which schedules are due (their next occurrence after last_run has passed).
        let mut due = Vec::new();
        for s in &schedules {
            let baseline = s.last_run.unwrap_or(now);
            let cron = match parser.parse(&s.cron_schedule) {
                Ok(c) => c,
                Err(e) => {
                    warn!("Invalid backup cron '{}' for user {}: {}", s.cron_schedule, s.user_id, e);
                    continue;
                }
            };
            match cron.find_next_occurrence(&baseline, false) {
                Ok(next) if next <= now => due.push(s),
                Ok(_) => {}
                Err(e) => warn!("Could not compute next backup time for user {}: {}", s.user_id, e),
            }
        }

        if due.is_empty() {
            return Ok(());
        }

        // A full database dump is identical regardless of which user triggered it, so run once.
        info!("🗄️ Running scheduled backup ({} schedule(s) due)", due.len());
        match state.db_pool.execute_scheduled_backup(due[0].user_id).await {
            Ok(filename) => {
                info!("✅ Scheduled backup created: {}", filename);
                for s in &due {
                    if let Err(e) = state.db_pool.update_scheduled_backup_last_run(s.user_id, now).await {
                        warn!("Failed to update backup last_run for user {}: {}", s.user_id, e);
                    }
                }
                // Enforce retention using the largest configured count among due schedules
                // (None/0 = keep all). Only auto-generated scheduled backups are pruned.
                let keep = due.iter().filter_map(|s| s.retention_count).max().unwrap_or(0);
                if keep > 0 {
                    Self::prune_scheduled_backups(keep);
                }
            }
            Err(e) => {
                error!("❌ Scheduled backup failed: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }

    // Delete oldest scheduled_backup_*.sql files beyond `keep`. Never touches manual backups.
    fn prune_scheduled_backups(keep: i32) {
        if keep <= 0 {
            return;
        }
        let backup_dir = "/opt/pinepods/backups";
        let mut files: Vec<(std::path::PathBuf, std::time::SystemTime)> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(backup_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("scheduled_backup_") && name.ends_with(".sql") {
                        let modified = entry
                            .metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .unwrap_or(std::time::UNIX_EPOCH);
                        files.push((path, modified));
                    }
                }
            }
        }
        // Newest first; remove everything past the retention count.
        files.sort_by(|a, b| b.1.cmp(&a.1));
        for (path, _) in files.into_iter().skip(keep as usize) {
            match std::fs::remove_file(&path) {
                Ok(_) => info!("🧹 Pruned old scheduled backup: {}", path.display()),
                Err(e) => warn!("Failed to prune backup {}: {}", path.display(), e),
            }
        }
    }

    // Self-update yt-dlp to the admin-configured channel, if auto-update is enabled (#793).
    // Shared by the daily job and the startup task.
    async fn run_ytdlp_update(state: Arc<AppState>) -> AppResult<()> {
        let settings = crate::services::ytdlp::get_settings(&state.db_pool)
            .await
            .unwrap_or_default();
        if !settings.auto_update {
            info!("🎬 yt-dlp auto-update is disabled; skipping");
            return Ok(());
        }
        info!("🎬 Running yt-dlp self-update ({} channel)", settings.channel);
        match crate::services::ytdlp::update_and_record(&state.db_pool).await {
            Ok(v) => info!("✅ yt-dlp auto-update complete: {}", v),
            Err(e) => warn!("⚠️ yt-dlp auto-update failed (keeping existing binary): {}", e),
        }
        Ok(())
    }

    // Run initial startup tasks immediately
    pub async fn run_startup_tasks(state: Arc<AppState>) -> AppResult<()> {
        info!("🚀 Running initial startup tasks...");
        
        // Initialize OIDC provider from environment variables if configured
        if let Err(e) = state.db_pool.init_oidc_from_env(&state.config.oidc).await {
            warn!("⚠️ OIDC initialization failed: {}", e);
        }
        
        // Create missing default playlists for existing users
        if let Err(e) = state.db_pool.create_missing_default_playlists().await {
            warn!("⚠️ Creating missing default playlists failed: {}", e);
        }

        // Strip any blank/whitespace categories left on existing podcasts
        if let Err(e) = state.db_pool.cleanup_blank_categories().await {
            warn!("⚠️ Cleaning blank podcast categories failed: {}", e);
        }

        info!("✅ Startup tasks completed");
        Ok(())
    }

    // Started once the server is listening: yt-dlp update, a refresh of the feeds that are due,
    // then people sync.
    pub async fn run_startup_refresh(state: Arc<AppState>) {
        // Log the yt-dlp version and self-update on startup (#793), so a freshly recreated
        // container pulls the latest YouTube fix before the refresh below pulls YouTube feeds.
        crate::services::ytdlp::log_version_at_startup().await;
        if let Err(e) = Self::run_ytdlp_update(state.clone()).await {
            warn!("⚠️ Startup yt-dlp update failed: {}", e);
        }

        guarded(JobKind::Refresh, run_refresh_job(&state)).await;

        // People: drop old host-feed placeholders, backfill stored tags, first PodPeopleDB sync.
        tokio::spawn(crate::services::people::startup(
            state.db_pool.clone(),
            crate::services::people::Endpoints::from_config(&state.config),
        ));
    }
}

/// The scheduled refresh: the feeds that are due, nothing else. Sync runs on its own schedule.
pub async fn run_refresh_job(state: &AppState) -> AppResult<()> {
    refresh::refresh_due_podcasts(state).await.map(|_| ())
}

/// The scheduled external sync.
pub async fn run_sync_job(state: &AppState) -> AppResult<()> {
    if let Err(e) = crate::services::sync::imports::retry_due(&state.db_pool, &state.task_spawner).await {
        warn!("Retrying failed sync imports failed: {}", e);
    }
    let s = engine::run_due(&state.db_pool, &state.task_spawner).await?;
    if s.synced + s.failed + s.held > 0 {
        debug!("Scheduled sync: {} synced, {} failed, {} in backoff", s.synced, s.failed, s.held);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_linearly_and_caps_at_a_day() {
        assert_eq!(failure_backoff(0), chrono::Duration::zero());
        assert_eq!(failure_backoff(1), chrono::Duration::minutes(30));
        assert_eq!(failure_backoff(4), chrono::Duration::hours(2));
        assert_eq!(failure_backoff(500), chrono::Duration::hours(24));
    }

    #[tokio::test]
    async fn single_flight_skips_an_overlapping_run() {
        let jobs = SingleFlight::default();
        let (release, wait) = tokio::sync::oneshot::channel::<()>();
        let first = jobs.run(JobKind::Refresh, async {
            let _ = wait.await;
            Ok(())
        });
        let second = async {
            tokio::task::yield_now().await;
            let overlapping = jobs.run(JobKind::Refresh, async { Ok(()) }).await;
            let other_kind = jobs.run(JobKind::Sync, async { Ok(()) }).await;
            let _ = release.send(());
            (overlapping.is_none(), other_kind.is_some())
        };
        let (first, (skipped, other_ran)) = tokio::join!(first, second);
        assert!(first.is_some() && skipped && other_ran);
        assert!(jobs.run(JobKind::Refresh, async { Ok(()) }).await.is_some(), "runs again once the first finished");
    }
}
