use crate::{
    error::AppResult,
    handlers::{refresh, tasks},
    AppState,
};
use std::sync::Arc;
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

        // Schedule podcast refresh every 30 minutes
        let refresh_state = app_state.clone();
        let refresh_job = Job::new_async("0 */30 * * * *", move |_uuid, _l| {
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

        // Add jobs to scheduler
        self.scheduler.add(refresh_job).await?;
        self.scheduler.add(nightly_job).await?;
        self.scheduler.add(cleanup_job).await?;

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

        info!("✅ Nightly tasks completed");
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
        
        // Run an immediate refresh to ensure data is current on startup
        if let Err(e) = Self::run_refresh_pods(state.clone()).await {
            warn!("⚠️ Initial startup refresh failed: {}", e);
        }
        
        info!("✅ Startup tasks completed");
        Ok(())
    }
}