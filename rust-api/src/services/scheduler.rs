use crate::{
    error::AppResult,
    handlers::{refresh, tasks},
    AppState,
};
use std::sync::Arc;
use chrono::Utc;
use croner::parser::{CronParser, Seconds};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, error, warn};

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

        // Schedule podcast refresh. Interval is configurable via PINEPODS_REFRESH_CRON (a 6-field
        // cron expression, sec min hour dom mon dow); defaults to every 30 minutes.
        let refresh_cron = std::env::var("PINEPODS_REFRESH_CRON")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "0 */30 * * * *".to_string());
        info!("📅 Podcast refresh schedule: {}", refresh_cron);
        let refresh_state = app_state.clone();
        let refresh_job = Job::new_async(refresh_cron.as_str(), move |_uuid, _l| {
            let state = refresh_state.clone();
            Box::pin(async move {
                info!("🔄 Running scheduled podcast refresh");
                if let Err(e) = Self::run_refresh_pods(state.clone()).await {
                    error!("❌ Scheduled podcast refresh failed: {}", e);
                } else {
                    info!("✅ Scheduled podcast refresh completed");
                }
            })
        })?;

        // Schedule nightly tasks at midnight
        let nightly_state = app_state.clone();
        let nightly_job = Job::new_async("0 0 0 * * *", move |_uuid, _l| {
            let state = nightly_state.clone();
            Box::pin(async move {
                info!("🌙 Running scheduled nightly tasks");
                if let Err(e) = Self::run_nightly_tasks(state.clone()).await {
                    error!("❌ Scheduled nightly tasks failed: {}", e);
                } else {
                    info!("✅ Scheduled nightly tasks completed");
                }
            })
        })?;

        // Schedule cleanup tasks every 6 hours
        let cleanup_state = app_state.clone();
        let cleanup_job = Job::new_async("0 0 */6 * * *", move |_uuid, _l| {
            let state = cleanup_state.clone();
            Box::pin(async move {
                info!("🧹 Running scheduled cleanup tasks");
                if let Err(e) = Self::run_cleanup_tasks(state.clone()).await {
                    error!("❌ Scheduled cleanup tasks failed: {}", e);
                } else {
                    info!("✅ Scheduled cleanup tasks completed");
                }
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

        // Add jobs to scheduler
        self.scheduler.add(refresh_job).await?;
        self.scheduler.add(nightly_job).await?;
        self.scheduler.add(cleanup_job).await?;
        self.scheduler.add(backup_job).await?;

        // Start the scheduler
        self.scheduler.start().await?;
        info!("✅ Background task scheduler started successfully");

        Ok(())
    }

    // Direct function calls instead of HTTP requests
    async fn run_refresh_pods(state: Arc<AppState>) -> AppResult<()> {
        // Call refresh_pods function directly
        match refresh::refresh_pods_admin_internal(&state).await {
            Ok(_) => {
                info!("✅ Podcast refresh completed");
                
                // Also run gpodder sync  
                if let Err(e) = refresh::refresh_gpodder_subscriptions_admin_internal(&state).await {
                    warn!("⚠️ GPodder sync failed during scheduled refresh: {}", e);
                }
                
                // Also run nextcloud sync
                if let Err(e) = refresh::refresh_nextcloud_subscriptions_admin_internal(&state).await {
                    warn!("⚠️ Nextcloud sync failed during scheduled refresh: {}", e);
                }
                
                // Update playlist episode counts (replaces complex playlist content updates)
                if let Err(e) = state.db_pool.update_playlist_episode_counts().await {
                    warn!("⚠️ Playlist episode count update failed during scheduled refresh: {}", e);
                }

                // Auto-add freshly-ingested episodes into collections with a category rule
                if let Err(e) = state.db_pool.refresh_category_collections().await {
                    warn!("⚠️ Category collection auto-add failed during scheduled refresh: {}", e);
                }

                // Propagate new podcast episodes into PeopleEpisodes for subscribed people
                if let Err(e) = state.db_pool.refresh_people_episodes_from_podcasts().await {
                    warn!("⚠️ People episodes sync failed during scheduled refresh: {}", e);
                }
            }
            Err(e) => {
                error!("❌ Podcast refresh failed: {}", e);
                return Err(e);
            }
        }
        Ok(())
    }

    async fn run_nightly_tasks(state: Arc<AppState>) -> AppResult<()> {
        // Call nightly tasks directly
        if let Err(e) = tasks::refresh_hosts_internal(&state).await {
            warn!("⚠️ Refresh hosts failed during nightly tasks: {}", e);
        }
        
        if let Err(e) = tasks::auto_complete_episodes_internal(&state).await {
            warn!("⚠️ Auto complete episodes failed during nightly tasks: {}", e);
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
        let parser = CronParser::builder().seconds(Seconds::Optional).build();

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

        // Run an immediate refresh to ensure data is current on startup
        if let Err(e) = Self::run_refresh_pods(state.clone()).await {
            warn!("⚠️ Initial startup refresh failed: {}", e);
        }
        
        info!("✅ Startup tasks completed");
        Ok(())
    }
}