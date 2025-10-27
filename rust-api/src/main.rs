use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::net::SocketAddr;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::{info, warn, error};

mod config;
mod database;
mod error;
mod handlers;
mod models;
mod redis_client;
mod redis_manager;
mod services;

use config::Config;
use database::DatabasePool;
use error::AppResult;
use redis_client::RedisClient;
use services::{scheduler::BackgroundScheduler, task_manager::TaskManager, tasks::TaskSpawner};
use handlers::websocket::WebSocketManager;
use redis_manager::{ImportProgressManager, NotificationManager};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: DatabasePool,
    pub redis_client: RedisClient,
    pub config: Config,
    pub task_manager: Arc<TaskManager>,
    pub task_spawner: Arc<TaskSpawner>,
    pub websocket_manager: Arc<WebSocketManager>,
    pub import_progress_manager: Arc<ImportProgressManager>,
    pub notification_manager: Arc<NotificationManager>,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Initialize tracing with explicit level if RUST_LOG is not set
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::EnvFilter::from_default_env()
    } else {
        tracing_subscriber::EnvFilter::new("info")
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();

    println!("🚀 Starting PinePods Rust API...");
    info!("Starting PinePods Rust API");

    // Load configuration
    let config = Config::new()?;
    info!("Configuration loaded");
    info!("Database config: host={}, port={}, user={}, db={}, type={}", 
          config.database.host, config.database.port, config.database.username, 
          config.database.name, config.database.db_type);

    // Initialize database pool
    let db_pool = DatabasePool::new(&config).await?;
    info!("Database pool initialized");

    // Initialize Redis client
    let redis_client = RedisClient::new(&config).await?;
    info!("Redis/Valkey client initialized");

    // Initialize task management
    let task_manager = Arc::new(TaskManager::new(redis_client.clone()));
    let task_spawner = Arc::new(TaskSpawner::new(task_manager.clone(), db_pool.clone()));
    let websocket_manager = Arc::new(WebSocketManager::new());
    let import_progress_manager = Arc::new(ImportProgressManager::new(redis_client.clone()));
    let notification_manager = Arc::new(NotificationManager::new(redis_client.clone()));
    info!("Task management system initialized");

    // Create shared application state
    let app_state = AppState {
        db_pool,
        redis_client,
        config: config.clone(),
        task_manager,
        task_spawner,
        websocket_manager,
        import_progress_manager,
        notification_manager,
    };

    // Build the application with routes
    let app = create_app(app_state.clone());

    // Initialize and start background scheduler
    info!("🕒 Initializing background task scheduler...");
    let scheduler = BackgroundScheduler::new().await?;
    let scheduler_state = Arc::new(app_state.clone());
    
    // Start the scheduler with background tasks
    scheduler.start(scheduler_state.clone()).await?;
    
    // Run initial startup tasks immediately
    tokio::spawn({
        let startup_state = scheduler_state.clone();
        async move {
            if let Err(e) = BackgroundScheduler::run_startup_tasks(startup_state).await {
                error!("❌ Startup tasks failed: {}", e);
            }
        }
    });

    // Determine the address to bind to
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    println!("🌐 PinePods Rust API listening on http://{}", addr);
    println!("📊 Health check available at: http://{}/api/health", addr);
    println!("🔍 API check available at: http://{}/api/pinepods_check", addr);
    info!("Server listening on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("✅ PinePods Rust API server started successfully!");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn create_app(state: AppState) -> Router {
    Router::new()
        // Health check endpoints
        .route("/api/pinepods_check", get(handlers::health::pinepods_check))
        .route("/api/health", get(handlers::health::health_check))
        
        // API routes (to be implemented)
        .nest("/api/data", create_data_routes())
        .nest("/api/init", create_init_routes())
        .nest("/api/podcasts", create_podcast_routes())
        .nest("/api/episodes", create_episode_routes())
        .nest("/api/playlists", create_playlist_routes())
        .nest("/api/tasks", create_task_routes())
        .nest("/api/async", create_async_routes())
        .nest("/api/proxy", create_proxy_routes())
        .nest("/api/gpodder", create_gpodder_routes())
        .nest("/api/feed", create_feed_routes())
        .nest("/api/auth", create_auth_routes())
        .nest("/ws", create_websocket_routes())
        
        // Middleware stack
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(tower_http::trace::DefaultMakeSpan::new()
                            .level(tracing::Level::INFO))
                        .on_response(tower_http::trace::DefaultOnResponse::new()
                            .level(tracing::Level::INFO))
                )
                .layer(CompressionLayer::new())
                .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024 * 1024)) // 2GB limit for massive backup files
        )
        .with_state(state)
}

fn create_data_routes() -> Router<AppState> {
    Router::new()
        .route("/get_key", get(handlers::auth::get_key))
        .route("/verify_mfa_and_get_key", post(handlers::auth::verify_mfa_and_get_key))
        .route("/verify_key", get(handlers::auth::verify_api_key_endpoint))
        .route("/get_user", get(handlers::auth::get_user))
        .route("/user_details_id/{user_id}", get(handlers::auth::get_user_details_by_id))
        .route("/self_service_status", get(handlers::auth::get_self_service_status))
        .route("/public_oidc_providers", get(handlers::auth::get_public_oidc_providers))
        .route("/create_first", post(handlers::auth::create_first_admin))
        .route("/config", get(handlers::auth::get_config))
        .route("/first_login_done/{user_id}", get(handlers::auth::first_login_done))
        .route("/get_theme/{user_id}", get(handlers::auth::get_theme))
        .route("/setup_time_info", post(handlers::auth::setup_time_info))
        .route("/update_timezone", put(handlers::auth::update_timezone))
        .route("/update_date_format", put(handlers::auth::update_date_format))
        .route("/update_time_format", put(handlers::auth::update_time_format))
        .route("/get_auto_complete_seconds/{user_id}", get(handlers::auth::get_auto_complete_seconds))
        .route("/update_auto_complete_seconds", put(handlers::auth::update_auto_complete_seconds))
        .route("/user_admin_check/{user_id}", get(handlers::auth::user_admin_check))
        .route("/import_opml", post(handlers::auth::import_opml))
        .route("/import_progress/{user_id}", get(handlers::auth::import_progress))
        .route("/return_episodes/{user_id}", get(handlers::podcasts::return_episodes))
        .route("/user_history/{user_id}", get(handlers::podcasts::user_history))
        .route("/increment_listen_time/{user_id}", put(handlers::podcasts::increment_listen_time))
        .route("/get_playback_speed", post(handlers::podcasts::get_playback_speed))
        .route("/add_podcast", post(handlers::podcasts::add_podcast))
        .route("/update_podcast_info", put(handlers::podcasts::update_podcast_info))
        .route("/{podcast_id}/merge", post(handlers::podcasts::merge_podcasts))
        .route("/{podcast_id}/unmerge/{target_podcast_id}", post(handlers::podcasts::unmerge_podcast))
        .route("/{podcast_id}/merged", get(handlers::podcasts::get_merged_podcasts))
        .route("/remove_podcast", post(handlers::podcasts::remove_podcast))
        .route("/remove_podcast_id", post(handlers::podcasts::remove_podcast_id))
        .route("/remove_podcast_name", post(handlers::podcasts::remove_podcast_by_name))
        .route("/return_pods/{user_id}", get(handlers::podcasts::return_pods))
        .route("/return_pods_extra/{user_id}", get(handlers::podcasts::return_pods_extra))
        .route("/get_time_info", get(handlers::podcasts::get_time_info))
        .route("/check_podcast", get(handlers::podcasts::check_podcast))
        .route("/check_episode_in_db/{user_id}", get(handlers::podcasts::check_episode_in_db))
        .route("/queue_pod", post(handlers::podcasts::queue_episode))
        .route("/remove_queued_pod", post(handlers::podcasts::remove_queued_episode))
        .route("/get_queued_episodes", get(handlers::podcasts::get_queued_episodes))
        .route("/reorder_queue", post(handlers::podcasts::reorder_queue))
        .route("/save_episode", post(handlers::podcasts::save_episode))
        .route("/remove_saved_episode", post(handlers::podcasts::remove_saved_episode))
        .route("/saved_episode_list/{user_id}", get(handlers::podcasts::get_saved_episodes))
        .route("/record_podcast_history", post(handlers::podcasts::add_history))
        .route("/get_podcast_id", get(handlers::podcasts::get_podcast_id))
        .route("/download_episode_list", get(handlers::podcasts::download_episode_list))
        .route("/download_podcast", post(handlers::podcasts::download_podcast))
        .route("/delete_episode", post(handlers::podcasts::delete_episode))
        .route("/download_all_podcast", post(handlers::podcasts::download_all_podcast))
        .route("/download_status/{user_id}", get(handlers::podcasts::download_status))
        .route("/podcast_episodes", get(handlers::podcasts::podcast_episodes))
        .route("/get_podcast_id_from_ep_name", get(handlers::podcasts::get_podcast_id_from_ep_name))
        .route("/get_episode_id_ep_name", get(handlers::podcasts::get_episode_id_ep_name))
        .route("/get_episode_metadata", post(handlers::podcasts::get_episode_metadata))
        .route("/fetch_podcasting_2_data", get(handlers::podcasts::fetch_podcasting_2_data))
        .route("/get_auto_download_status", post(handlers::podcasts::get_auto_download_status))
        .route("/get_feed_cutoff_days", get(handlers::podcasts::get_feed_cutoff_days))
        .route("/get_play_episode_details", post(handlers::podcasts::get_play_episode_details))
        .route("/fetch_podcasting_2_pod_data", get(handlers::podcasts::fetch_podcasting_2_pod_data))
        .route("/mark_episode_completed", post(handlers::podcasts::mark_episode_completed))
        .route("/update_episode_duration", post(handlers::podcasts::update_episode_duration))
        // Bulk episode operations
        .route("/bulk_mark_episodes_completed", post(handlers::episodes::bulk_mark_episodes_completed))
        .route("/bulk_save_episodes", post(handlers::episodes::bulk_save_episodes))
        .route("/bulk_queue_episodes", post(handlers::episodes::bulk_queue_episodes))
        .route("/bulk_download_episodes", post(handlers::episodes::bulk_download_episodes))
        .route("/bulk_delete_downloaded_episodes", post(handlers::episodes::bulk_delete_downloaded_episodes))
        .route("/share_episode/{episode_id}", post(handlers::episodes::share_episode))
        .route("/episode_by_url/{url_key}", get(handlers::episodes::get_episode_by_url_key))
        .route("/increment_played/{user_id}", put(handlers::podcasts::increment_played))
        .route("/record_listen_duration", post(handlers::podcasts::record_listen_duration))
        .route("/get_podcast_id_from_ep_id", get(handlers::podcasts::get_podcast_id_from_ep_id))
        .route("/get_stats", get(handlers::podcasts::get_stats))
        .route("/get_pinepods_version", get(handlers::podcasts::get_pinepods_version))
        .route("/search_data", post(handlers::podcasts::search_data))
        .route("/fetch_transcript", post(handlers::podcasts::fetch_transcript))
        .route("/home_overview", get(handlers::podcasts::home_overview))
        .route("/get_playlists", get(handlers::podcasts::get_playlists))
        .route("/get_playlist_episodes", get(handlers::podcasts::get_playlist_episodes))
        .route("/create_playlist", post(handlers::playlists::create_playlist))
        .route("/delete_playlist", delete(handlers::playlists::delete_playlist))
        .route("/get_podcast_details", get(handlers::podcasts::get_podcast_details))
        .route("/get_podcast_details_dynamic", get(handlers::podcasts::get_podcast_details_dynamic))
        .route("/podpeople/host_podcasts", get(handlers::podcasts::get_host_podcasts))
        .route("/update_feed_cutoff_days", post(handlers::podcasts::update_feed_cutoff_days))
        .route("/fetch_podcast_feed", get(handlers::podcasts::fetch_podcast_feed))
        .route("/youtube_episodes", get(handlers::podcasts::youtube_episodes))
        .route("/remove_youtube_channel", post(handlers::podcasts::remove_youtube_channel))
        .route("/stream/{episode_id}", get(handlers::podcasts::stream_episode))
        .route("/get_rss_key", get(handlers::podcasts::get_rss_key))
        .route("/mark_episode_uncompleted", post(handlers::podcasts::mark_episode_uncompleted))
        .route("/user/set_theme", put(handlers::settings::set_theme))
        .route("/get_user_info", get(handlers::settings::get_user_info))
        .route("/my_user_info/{user_id}", get(handlers::settings::get_my_user_info))
        .route("/add_user", post(handlers::settings::add_user))
        .route("/add_login_user", post(handlers::settings::add_login_user))
        .route("/set_fullname/{user_id}", put(handlers::settings::set_fullname))
        .route("/set_password/{user_id}", put(handlers::settings::set_password))
        .route("/user/delete/{user_id}", delete(handlers::settings::delete_user))
        .route("/user/set_email", put(handlers::settings::set_email))
        .route("/user/set_username", put(handlers::settings::set_username))
        .route("/user/set_isadmin", put(handlers::settings::set_isadmin))
        .route("/user/final_admin/{user_id}", get(handlers::settings::final_admin))
        .route("/enable_disable_guest", post(handlers::settings::enable_disable_guest))
        .route("/enable_disable_downloads", post(handlers::settings::enable_disable_downloads))
        .route("/enable_disable_self_service", post(handlers::settings::enable_disable_self_service))
        .route("/guest_status", get(handlers::settings::guest_status))
        .route("/rss_feed_status", get(handlers::settings::rss_feed_status))
        .route("/toggle_rss_feeds", post(handlers::settings::toggle_rss_feeds))
        .route("/download_status", get(handlers::settings::download_status))
        .route("/admin_self_service_status", get(handlers::settings::self_service_status))
        .route("/save_email_settings", post(handlers::settings::save_email_settings))
        .route("/get_email_settings", get(handlers::settings::get_email_settings))
        .route("/send_test_email", post(handlers::settings::send_test_email))
        .route("/send_email", post(handlers::settings::send_email))
        .route("/reset_password_create_code", post(handlers::auth::reset_password_create_code))
        .route("/verify_and_reset_password", post(handlers::auth::verify_and_reset_password))
        .route("/get_api_info/{user_id}", get(handlers::settings::get_api_info))
        .route("/create_api_key", post(handlers::settings::create_api_key))
        .route("/delete_api_key", delete(handlers::settings::delete_api_key))
        .route("/backup_user", post(handlers::settings::backup_user))
        .route("/backup_server", post(handlers::settings::backup_server))
        .route("/restore_server", post(handlers::settings::restore_server))
        .route("/generate_mfa_secret/{user_id}", get(handlers::settings::generate_mfa_secret))
        .route("/verify_temp_mfa", post(handlers::settings::verify_temp_mfa))
        .route("/check_mfa_enabled/{user_id}", get(handlers::settings::check_mfa_enabled))
        .route("/save_mfa_secret", post(handlers::settings::save_mfa_secret))
        .route("/delete_mfa", delete(handlers::settings::delete_mfa))
        .route("/initiate_nextcloud_login", post(handlers::settings::initiate_nextcloud_login))
        .route("/add_nextcloud_server", post(handlers::settings::add_nextcloud_server))
        .route("/verify_gpodder_auth", post(handlers::settings::verify_gpodder_auth))
        .route("/add_gpodder_server", post(handlers::settings::add_gpodder_server))
        .route("/get_gpodder_settings/{user_id}", get(handlers::settings::get_gpodder_settings))
        .route("/check_gpodder_settings/{user_id}", get(handlers::settings::check_gpodder_settings))
        .route("/remove_podcast_sync", delete(handlers::settings::remove_podcast_sync))
        .route("/gpodder/status", get(handlers::sync::gpodder_status))
        .route("/gpodder/toggle", post(handlers::sync::gpodder_toggle))
        .route("/refresh_pods", get(handlers::refresh::refresh_pods_admin))
        .route("/refresh_gpodder_subscriptions", get(handlers::refresh::refresh_gpodder_subscriptions_admin))
        .route("/refresh_nextcloud_subscriptions", get(handlers::refresh::refresh_nextcloud_subscriptions_admin))
        .route("/refresh_hosts", get(handlers::tasks::refresh_hosts))
        .route("/cleanup_tasks", get(handlers::tasks::cleanup_tasks))
        .route("/auto_complete_episodes", get(handlers::tasks::auto_complete_episodes))
        .route("/update_playlists", get(handlers::tasks::update_playlists))
        .route("/add_custom_podcast", post(handlers::settings::add_custom_podcast))
        .route("/user/notification_settings", get(handlers::settings::get_notification_settings))
        .route("/user/notification_settings", put(handlers::settings::update_notification_settings))
        .route("/user/set_playback_speed", post(handlers::settings::set_playback_speed_user))
        .route("/user/set_global_podcast_cover_preference", post(handlers::settings::set_global_podcast_cover_preference))
        .route("/user/get_podcast_cover_preference", get(handlers::settings::get_global_podcast_cover_preference))
        .route("/user/test_notification", post(handlers::settings::test_notification))
        .route("/add_oidc_provider", post(handlers::settings::add_oidc_provider))
        .route("/update_oidc_provider/{provider_id}", put(handlers::settings::update_oidc_provider))
        .route("/list_oidc_providers", get(handlers::settings::list_oidc_providers))
        .route("/remove_oidc_provider", post(handlers::settings::remove_oidc_provider))
        .route("/startpage", get(handlers::settings::get_startpage))
        .route("/startpage", post(handlers::settings::update_startpage))
        .route("/person/subscribe/{user_id}/{person_id}", post(handlers::settings::subscribe_to_person))
        .route("/person/unsubscribe/{user_id}/{person_id}", delete(handlers::settings::unsubscribe_from_person))
        .route("/person/subscriptions/{user_id}", get(handlers::settings::get_person_subscriptions))
        .route("/person/episodes/{user_id}/{person_id}", get(handlers::settings::get_person_episodes))
        .route("/search_youtube_channels", get(handlers::youtube::search_youtube_channels))
        .route("/youtube/subscribe", post(handlers::youtube::subscribe_to_youtube_channel))
        .route("/check_youtube_channel", get(handlers::youtube::check_youtube_channel))
        .route("/enable_auto_download", post(handlers::settings::enable_auto_download))
        .route("/adjust_skip_times", post(handlers::settings::adjust_skip_times))
        .route("/remove_category", post(handlers::settings::remove_category))
        .route("/add_category", post(handlers::settings::add_category))
        .route("/podcast/set_playback_speed", post(handlers::settings::set_podcast_playback_speed))
        .route("/podcast/set_cover_preference", post(handlers::settings::set_podcast_cover_preference))
        .route("/podcast/clear_cover_preference", post(handlers::settings::clear_podcast_cover_preference))
        .route("/podcast/toggle_notifications", put(handlers::settings::toggle_podcast_notifications))
        .route("/podcast/notification_status", post(handlers::podcasts::get_notification_status))
        .route("/rss_key", get(handlers::settings::get_user_rss_key))
        .route("/verify_mfa", post(handlers::settings::verify_mfa))
        .route("/schedule_backup", post(handlers::settings::schedule_backup))
        .route("/get_scheduled_backup", post(handlers::settings::get_scheduled_backup))
        .route("/list_backup_files", post(handlers::settings::list_backup_files))
        .route("/restore_backup_file", post(handlers::settings::restore_from_backup_file))
        .route("/manual_backup_to_directory", post(handlers::settings::manual_backup_to_directory))
        .route("/get_unmatched_podcasts", post(handlers::settings::get_unmatched_podcasts))
        .route("/update_podcast_index_id", post(handlers::settings::update_podcast_index_id))
        .route("/ignore_podcast_index_id", post(handlers::settings::ignore_podcast_index_id))
        .route("/get_ignored_podcasts", post(handlers::settings::get_ignored_podcasts))
        // Language preference endpoints
        .route("/get_user_language", get(handlers::settings::get_user_language))
        .route("/update_user_language", put(handlers::settings::update_user_language))
        .route("/get_available_languages", get(handlers::settings::get_available_languages))
        .route("/get_server_default_language", get(handlers::settings::get_server_default_language))
        // Add more data routes as needed
}

fn create_podcast_routes() -> Router<AppState> {
    Router::new()
        .route("/notification_status", post(handlers::podcasts::get_notification_status))
}

fn create_episode_routes() -> Router<AppState> {
    Router::new()
        .route("/{episode_id}/download", get(handlers::episodes::download_episode_file))
}

fn create_playlist_routes() -> Router<AppState> {
    Router::new()
        // Add playlist routes as needed
}

fn create_task_routes() -> Router<AppState> {
    Router::new()
        .route("/user/{user_id}", get(handlers::websocket::get_user_tasks))
        .route("/active", get(handlers::websocket::get_active_tasks))
        .route("/{task_id}", get(handlers::websocket::get_task_status))
}

fn create_async_routes() -> Router<AppState> {
    Router::new()
        // .route("/download_episode", post(handlers::tasks::download_episode))
        // .route("/import_opml", post(handlers::tasks::import_opml))
        // .route("/refresh_feeds", post(handlers::tasks::refresh_all_feeds))
        // .route("/episode/{episode_id}/metadata", get(handlers::tasks::quick_metadata_fetch))
}

fn create_proxy_routes() -> Router<AppState> {
    Router::new()
        .route("/image", get(handlers::proxy::proxy_image))
}

fn create_gpodder_routes() -> Router<AppState> {
    Router::new()
        .route("/test-connection", get(handlers::sync::gpodder_test_connection))
        .route("/set_default/{device_id}", post(handlers::sync::gpodder_set_default))
        .route("/devices/{user_id}", get(handlers::sync::gpodder_get_user_devices))
        .route("/devices", get(handlers::sync::gpodder_get_all_devices))
        .route("/default_device", get(handlers::sync::gpodder_get_default_device))
        .route("/devices", post(handlers::sync::gpodder_create_device))
        .route("/sync/force", post(handlers::sync::gpodder_force_sync))
        .route("/sync", post(handlers::sync::gpodder_sync))
        .route("/gpodder_statistics", get(handlers::sync::gpodder_get_statistics))
}

fn create_init_routes() -> Router<AppState> {
    Router::new()
        .route("/startup_tasks", post(handlers::tasks::startup_tasks))
}

fn create_feed_routes() -> Router<AppState> {
    Router::new()
        .route("/{user_id}", get(handlers::feed::get_user_feed))
}

fn create_websocket_routes() -> Router<AppState> {
    Router::new()
        .route("/api/tasks/{user_id}", get(handlers::websocket::task_progress_websocket))
        .route("/api/data/episodes/{user_id}", get(handlers::refresh::websocket_refresh_episodes))
}

fn create_auth_routes() -> Router<AppState> {
    Router::new()
        .route("/store_state", post(handlers::auth::store_oidc_state))
        .route("/callback", get(handlers::auth::oidc_callback))
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            warn!("Received Ctrl+C, shutting down gracefully");
        },
        _ = terminate => {
            warn!("Received SIGTERM, shutting down gracefully");
        },
    }
}