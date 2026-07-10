use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    trace::TraceLayer,
    compression::CompressionLayer,
    services::ServeDir,
};
use tracing::{debug, error, info, warn};

mod config;
mod database;
mod error;
mod handlers;
mod models;
mod openapi;
mod redis_client;
mod redis_manager;
mod services;

use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_scalar::{Scalar, Servable};

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
    /// Set while a full server restore is running. Used to reject concurrent restores
    /// and to block first-admin creation (which would otherwise race the restore and
    /// corrupt it). Shared across clones via Arc.
    pub restore_in_progress: Arc<std::sync::atomic::AtomicBool>,
    /// Whether the optional pinepods-ai sidecar is reachable. Kept fresh by a
    /// background health monitor; handlers gate AI features on this.
    pub ai_available: crate::services::ai_client::AiAvailability,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Offline OpenAPI dump: `pinepods-api --dump-openapi [path]` writes the spec and exits.
    // Builds the document without touching the database/Redis so it can run in CI.
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--dump-openapi") {
        let path = args.get(pos + 1).map(String::as_str).unwrap_or("openapi.json");
        let spec = openapi_router().split_for_parts().1;
        let json = spec.to_pretty_json().expect("serialize OpenAPI document");
        std::fs::write(path, &json).unwrap_or_else(|e| panic!("write {path}: {e}"));
        println!("Wrote OpenAPI spec to {path}");
        return Ok(());
    }

    // Initialize tracing with explicit level if RUST_LOG is not set
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::EnvFilter::from_default_env()
    } else {
        tracing_subscriber::EnvFilter::new("info")
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .init();

    info!("🚀 Starting PinePods Rust API...");
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
    let notification_manager = Arc::new(NotificationManager::new());
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
        restore_in_progress: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        ai_available: crate::services::ai_client::AiAvailability::new(),
    };

    // Start the AI sidecar health monitor (no-op if PINEPODS_AI_URL is unset).
    crate::services::ai_client::spawn_health_monitor(app_state.ai_available.clone());

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
    info!("🌐 PinePods Rust API listening on http://{}", addr);
    info!("📊 Health check available at: http://{}/api/health", addr);
    debug!("🔍 API check available at: http://{}/api/pinepods_check", addr);
    info!("Server listening on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    debug!("✅ PinePods Rust API server started successfully!");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Builds the spec-aware router. Handlers annotated with `#[utoipa::path]` and
/// registered via `routes!` are collected into the OpenAPI document; route groups
/// nested as plain `Router`s still serve but are not yet in the spec. Migrate a group
/// by converting its `create_*_routes()` to an `OpenApiRouter` and using `routes!`.
fn openapi_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(openapi::ApiDoc::openapi())
        // Health check endpoints (annotated — appear in the spec).
        // One routes!() call per distinct path; routes!() groups methods that share a path.
        .routes(routes!(handlers::health::pinepods_check))
        .routes(routes!(handlers::health::health_check))
        // Partially-migrated groups (annotated handlers appear in the spec; the rest serve as plain routes)
        .nest("/api/data", create_data_routes())
        .nest("/api/episodes", create_episode_routes())
        // Not-yet-migrated groups (served as plain routers; not in the spec)
        .nest("/api/init", create_init_routes())
        .nest("/api/podcasts", OpenApiRouter::from(create_podcast_routes()))
        .nest("/api/playlists", OpenApiRouter::from(create_playlist_routes()))
        .nest("/api/tasks", create_task_routes())
        .nest("/api/async", OpenApiRouter::from(create_async_routes()))
        .nest("/api/proxy", create_proxy_routes())
        .nest("/api/gpodder", create_gpodder_routes())
        .nest("/api/feed", create_feed_routes())
        .nest("/api/auth", create_auth_routes())
        .nest("/ws", OpenApiRouter::from(create_websocket_routes()))
}

fn create_app(state: AppState) -> Router {
    let (router, api) = openapi_router().split_for_parts();
    let spec = api.clone();

    router
        // Routes/services that intentionally stay out of the API spec
        .route("/api/placeholder/{width}/{height}", get(handlers::proxy::placeholder_image))
        .nest_service("/api/local-media", ServeDir::new("/opt/pinepods/local-media"))
        // Raw spec for tooling/docs builds, plus the interactive Scalar UI at /api/docs
        .route("/api/openapi.json", get(move || {
            let spec = spec.clone();
            async move { axum::Json(spec) }
        }))
        .merge(Scalar::with_url("/api/docs", api))
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

fn create_data_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::auth::get_key))
        .routes(routes!(handlers::auth::verify_mfa_and_get_key))
        .routes(routes!(handlers::auth::verify_api_key_endpoint))
        .routes(routes!(handlers::auth::get_user))
        .routes(routes!(handlers::auth::get_user_details_by_id))
        .routes(routes!(handlers::auth::get_self_service_status))
        .routes(routes!(handlers::auth::get_public_oidc_providers))
        .routes(routes!(handlers::auth::create_first_admin))
        .routes(routes!(handlers::auth::get_config))
        .routes(routes!(handlers::auth::first_login_done))
        .routes(routes!(handlers::auth::get_theme))
        .routes(routes!(handlers::auth::setup_time_info))
        .routes(routes!(handlers::auth::update_timezone))
        .routes(routes!(handlers::auth::update_date_format))
        .routes(routes!(handlers::auth::update_time_format))
        .routes(routes!(handlers::auth::get_auto_complete_seconds))
        .routes(routes!(handlers::auth::update_auto_complete_seconds))
        .routes(routes!(handlers::auth::user_admin_check))
        .routes(routes!(handlers::auth::import_opml))
        .routes(routes!(handlers::auth::import_progress))
        .routes(routes!(handlers::podcasts::return_episodes))
        .routes(routes!(handlers::podcasts::user_history))
        .routes(routes!(handlers::podcasts::increment_listen_time))
        .routes(routes!(handlers::podcasts::get_playback_speed))
        .routes(routes!(handlers::podcasts::get_auto_download_delete_days))
        .routes(routes!(handlers::podcasts::get_default_volume))
        .routes(routes!(handlers::podcasts::add_podcast))
        .routes(routes!(handlers::podcasts::update_podcast_info))
        .routes(routes!(handlers::podcasts::merge_podcasts))
        .routes(routes!(handlers::podcasts::unmerge_podcast))
        .routes(routes!(handlers::podcasts::get_merged_podcasts))
        .routes(routes!(handlers::podcasts::remove_podcast))
        .routes(routes!(handlers::podcasts::remove_podcast_id))
        .routes(routes!(handlers::podcasts::return_pods))
        .routes(routes!(handlers::podcasts::return_pods_extra))
        .routes(routes!(handlers::podcasts::get_time_info))
        .routes(routes!(handlers::podcasts::check_podcast))
        .routes(routes!(handlers::podcasts::check_episode_in_db))
        .routes(routes!(handlers::podcasts::queue_episode))
        .routes(routes!(handlers::podcasts::remove_queued_episode))
        .routes(routes!(handlers::podcasts::get_queued_episodes))
        .routes(routes!(handlers::podcasts::reorder_queue))
        .routes(routes!(handlers::podcasts::clear_all_queue))
        .routes(routes!(handlers::podcasts::save_episode))
        .routes(routes!(handlers::podcasts::remove_saved_episode))
        .routes(routes!(handlers::podcasts::get_saved_episodes))
        .routes(routes!(handlers::podcasts::add_history))
        .routes(routes!(handlers::podcasts::get_podcast_id))
        .routes(routes!(handlers::podcasts::download_episode_list))
        .routes(routes!(handlers::podcasts::get_podcast_download_summary))
        .routes(routes!(handlers::podcasts::get_podcast_downloads_paged))
        .routes(routes!(handlers::podcasts::download_podcast))
        .routes(routes!(handlers::podcasts::delete_episode))
        .routes(routes!(handlers::podcasts::download_all_podcast))
        .routes(routes!(handlers::podcasts::download_status))
        .routes(routes!(handlers::podcasts::podcast_episodes))
        .routes(routes!(handlers::podcasts::get_podcast_id_from_ep_name))
        .routes(routes!(handlers::podcasts::get_episode_id_ep_name))
        .routes(routes!(handlers::podcasts::get_episode_metadata))
        .routes(routes!(handlers::podcasts::fetch_podcasting_2_data))
        .routes(routes!(handlers::podcasts::get_auto_download_status))
        .routes(routes!(handlers::podcasts::get_auto_queue_status))
        .routes(routes!(handlers::podcasts::get_auto_play_next_status))
        .routes(routes!(handlers::podcasts::get_next_podcast_episode))
        .routes(routes!(handlers::podcasts::get_next_playlist_episode))
        .routes(routes!(handlers::podcasts::get_feed_cutoff_days))
        .routes(routes!(handlers::podcasts::get_play_episode_details))
        .routes(routes!(handlers::podcasts::fetch_podcasting_2_pod_data))
        .routes(routes!(handlers::podcasts::mark_episode_completed))
        .routes(routes!(handlers::podcasts::update_episode_duration))
        // Bulk episode operations
        .routes(routes!(handlers::episodes::bulk_mark_episodes_completed))
        .routes(routes!(handlers::episodes::bulk_save_episodes))
        .routes(routes!(handlers::episodes::bulk_queue_episodes))
        .routes(routes!(handlers::episodes::bulk_download_episodes))
        .routes(routes!(handlers::episodes::bulk_delete_downloaded_episodes))
        .routes(routes!(handlers::episodes::share_episode))
        .routes(routes!(handlers::episodes::get_episode_by_url_key))
        .routes(routes!(handlers::settings::get_user_shared_links))
        .routes(routes!(handlers::settings::delete_shared_link))
        .routes(routes!(handlers::settings::extend_shared_link))
        .routes(routes!(handlers::podcasts::increment_played))
        .routes(routes!(handlers::podcasts::record_listen_duration))
        .routes(routes!(handlers::podcasts::get_podcast_id_from_ep_id))
        .routes(routes!(handlers::podcasts::get_stats))
        .routes(routes!(handlers::podcasts::get_extended_stats))
        .routes(routes!(handlers::podcasts::get_pinepods_version))
        .routes(routes!(handlers::podcasts::search_data))
        .routes(routes!(handlers::podcasts::proxy_search))
        .routes(routes!(handlers::podcasts::proxy_trending))
        .routes(routes!(handlers::podcasts::proxy_categories))
        .routes(routes!(handlers::podcasts::get_recommendations))
        .routes(routes!(handlers::podcasts::fetch_transcript))
        .routes(routes!(handlers::podcasts::home_overview))
        .routes(routes!(handlers::podcasts::get_playlists))
        .routes(routes!(handlers::podcasts::get_playlist_episodes))
        .routes(routes!(handlers::playlists::create_playlist))
        .routes(routes!(handlers::playlists::delete_playlist))
        .routes(routes!(handlers::playlists::update_playlist))
        .routes(routes!(handlers::collections::create_collection))
        .routes(routes!(handlers::collections::list_collections))
        .routes(routes!(handlers::collections::get_user_categories))
        .routes(routes!(handlers::collections::delete_collection))
        .routes(routes!(handlers::collections::update_collection))
        .routes(routes!(handlers::collections::add_episode_to_collection))
        .routes(routes!(handlers::collections::remove_episode_from_collection))
        .routes(routes!(handlers::collections::bulk_add_collection))
        .routes(routes!(handlers::collections::get_collection_episodes))
        .routes(routes!(handlers::collections::get_episode_collections))
        .routes(routes!(handlers::collections::get_collection_add_ui))
        .routes(routes!(handlers::collections::set_collection_add_ui))
        .routes(routes!(handlers::podcasts::get_podcast_details))
        .routes(routes!(handlers::podcasts::get_podcast_details_dynamic))
        .routes(routes!(handlers::podcasts::get_host_podcasts))
        .routes(routes!(handlers::podcasts::get_podpeople_discover))
        .routes(routes!(handlers::podcasts::update_feed_cutoff_days))
        .routes(routes!(handlers::podcasts::fetch_podcast_feed))
        .routes(routes!(handlers::podcasts::youtube_episodes))
        .routes(routes!(handlers::podcasts::remove_youtube_channel))
        .routes(routes!(handlers::podcasts::stream_episode))
        .routes(routes!(handlers::podcasts::get_rss_key))
        .routes(routes!(handlers::podcasts::mark_episode_uncompleted))
        .routes(routes!(handlers::settings::set_theme))
        .routes(routes!(handlers::settings::get_custom_themes))
        .routes(routes!(handlers::settings::create_custom_theme, handlers::settings::delete_custom_theme))
        .routes(routes!(handlers::settings::get_user_info))
        .routes(routes!(handlers::settings::get_my_user_info))
        .routes(routes!(handlers::settings::add_user))
        .routes(routes!(handlers::settings::add_login_user))
        .routes(routes!(handlers::settings::set_fullname))
        .routes(routes!(handlers::settings::set_password))
        .routes(routes!(handlers::settings::delete_user))
        .routes(routes!(handlers::settings::set_email))
        .routes(routes!(handlers::settings::set_username))
        .routes(routes!(handlers::settings::set_isadmin))
        .routes(routes!(handlers::settings::final_admin))
        .routes(routes!(handlers::settings::enable_disable_guest))
        .routes(routes!(handlers::settings::enable_disable_downloads))
        .routes(routes!(handlers::settings::enable_disable_self_service))
        .routes(routes!(handlers::settings::guest_status))
        .routes(routes!(handlers::settings::rss_feed_status))
        .routes(routes!(handlers::settings::toggle_rss_feeds))
        .routes(routes!(handlers::settings::download_status))
        .routes(routes!(
            handlers::settings::get_download_metadata_settings,
            handlers::settings::set_download_metadata_settings
        ))
        .routes(routes!(handlers::settings::self_service_status))
        .routes(routes!(handlers::settings::save_email_settings))
        .routes(routes!(handlers::settings::get_email_settings))
        .routes(routes!(handlers::settings::send_test_email))
        .routes(routes!(handlers::settings::send_email))
        .routes(routes!(handlers::auth::reset_password_create_code))
        .routes(routes!(handlers::auth::verify_and_reset_password))
        .routes(routes!(handlers::settings::get_api_info))
        .routes(routes!(handlers::settings::create_api_key))
        .routes(routes!(handlers::settings::delete_api_key))
        .routes(routes!(handlers::settings::backup_user))
        .routes(routes!(handlers::settings::backup_server))
        .routes(routes!(handlers::settings::restore_server))
        .routes(routes!(handlers::settings::restore_status))
        .routes(routes!(handlers::settings::generate_mfa_secret))
        .routes(routes!(handlers::settings::verify_temp_mfa))
        .routes(routes!(handlers::settings::check_mfa_enabled))
        .routes(routes!(handlers::settings::save_mfa_secret))
        .routes(routes!(handlers::settings::delete_mfa))
        .routes(routes!(handlers::settings::initiate_nextcloud_login))
        .routes(routes!(handlers::settings::add_nextcloud_server))
        .routes(routes!(handlers::settings::verify_gpodder_auth))
        .routes(routes!(handlers::settings::add_gpodder_server))
        .routes(routes!(handlers::settings::get_gpodder_settings))
        .routes(routes!(handlers::settings::check_gpodder_settings))
        .routes(routes!(handlers::settings::remove_podcast_sync))
        .routes(routes!(handlers::sync::gpodder_status))
        .routes(routes!(handlers::sync::gpodder_toggle))
        .routes(routes!(handlers::refresh::refresh_pods_admin))
        .routes(routes!(handlers::refresh::refresh_gpodder_subscriptions_admin))
        .routes(routes!(handlers::refresh::refresh_nextcloud_subscriptions_admin))
        .routes(routes!(handlers::tasks::refresh_hosts))
        .routes(routes!(handlers::tasks::cleanup_tasks))
        .routes(routes!(handlers::tasks::auto_complete_episodes))
        .routes(routes!(handlers::tasks::update_playlists))
        .routes(routes!(handlers::settings::add_custom_podcast))
        .routes(routes!(handlers::local_podcast::add_local_podcast))
        .routes(routes!(handlers::local_podcast::add_local_podcast_artwork))
        .routes(routes!(handlers::local_podcast::refresh_local_podcast))
        .routes(routes!(handlers::local_podcast::list_local_directories))
        .routes(routes!(handlers::local_podcast::detect_local_cover))
        .routes(routes!(handlers::settings::get_notification_settings, handlers::settings::update_notification_settings))
        .routes(routes!(handlers::settings::set_playback_speed_user))
        .routes(routes!(handlers::settings::set_default_volume_user))
        .routes(routes!(handlers::settings::set_auto_download_delete_days_user))
        .routes(routes!(handlers::settings::set_global_podcast_cover_preference))
        .routes(routes!(handlers::settings::get_global_podcast_cover_preference))
        .routes(routes!(handlers::settings::test_notification))
        .routes(routes!(handlers::settings::add_oidc_provider))
        .routes(routes!(handlers::settings::update_oidc_provider))
        .routes(routes!(handlers::settings::list_oidc_providers))
        .routes(routes!(handlers::settings::remove_oidc_provider))
        .routes(routes!(handlers::settings::get_startpage, handlers::settings::update_startpage))
        .routes(routes!(handlers::settings::subscribe_to_person))
        .routes(routes!(handlers::settings::unsubscribe_from_person))
        .routes(routes!(handlers::settings::get_person_subscriptions))
        .routes(routes!(handlers::settings::get_person_episodes))
        .routes(routes!(handlers::settings::get_host_feed))
        .routes(routes!(handlers::youtube::search_youtube_channels))
        .routes(routes!(handlers::youtube::subscribe_to_youtube_channel))
        .routes(routes!(handlers::youtube::check_youtube_channel))
        .routes(routes!(handlers::settings::enable_auto_download))
        .routes(routes!(handlers::settings::enable_auto_queue))
        .routes(routes!(handlers::settings::enable_auto_play_next))
        .routes(routes!(handlers::settings::adjust_skip_times))
        .routes(routes!(handlers::settings::ai_status))
        .routes(routes!(handlers::settings::transcribe_episode))
        .routes(routes!(handlers::settings::get_episode_transcript))
        .routes(routes!(handlers::settings::adjust_auto_transcribe))
        .routes(routes!(handlers::settings::get_auto_transcribe))
        .routes(routes!(handlers::settings::adjust_silence_trim))
        .routes(routes!(handlers::settings::get_silence_trim))
        .routes(routes!(handlers::settings::get_episode_skip_segments))
        .routes(routes!(handlers::settings::detect_silence))
        .routes(routes!(handlers::settings::detect_ads))
        .routes(routes!(handlers::settings::adjust_ad_segment_review))
        .routes(routes!(handlers::settings::adjust_auto_ad_detect))
        .routes(routes!(handlers::settings::get_auto_ad_detect))
        .routes(routes!(handlers::settings::adjust_ad_skip_auto_activate))
        .routes(routes!(handlers::settings::get_ad_skip_auto_activate))
        .routes(routes!(handlers::settings::get_ai_settings, handlers::settings::update_ai_settings))
        .routes(routes!(handlers::settings::get_ai_models))
        .routes(routes!(handlers::settings::ai_pull_model))
        .routes(routes!(handlers::settings::remove_category))
        .routes(routes!(handlers::settings::add_category))
        .routes(routes!(handlers::settings::set_podcast_playback_speed))
        .routes(routes!(handlers::settings::clear_podcast_playback_speed))
        .routes(routes!(handlers::settings::set_podcast_auto_download_delete_days))
        .routes(routes!(handlers::settings::clear_podcast_auto_download_delete_days))
        .routes(routes!(handlers::settings::set_podcast_cover_preference))
        .routes(routes!(handlers::settings::clear_podcast_cover_preference))
        .routes(routes!(handlers::settings::toggle_podcast_notifications))
        .routes(routes!(handlers::podcasts::get_notification_status))
        .routes(routes!(handlers::settings::toggle_podcast_favorite))
        .routes(routes!(handlers::podcasts::get_podcast_favorite_status))
        .routes(routes!(handlers::settings::get_user_rss_key))
        .routes(routes!(handlers::settings::verify_mfa))
        .routes(routes!(handlers::settings::schedule_backup))
        .routes(routes!(handlers::settings::get_scheduled_backup))
        .routes(routes!(handlers::settings::list_backup_files))
        .routes(routes!(handlers::settings::restore_from_backup_file))
        .routes(routes!(handlers::settings::manual_backup_to_directory))
        .routes(routes!(handlers::settings::delete_backup_file))
        .routes(routes!(handlers::settings::get_unmatched_podcasts))
        .routes(routes!(handlers::settings::update_podcast_index_id))
        .routes(routes!(handlers::settings::ignore_podcast_index_id))
        .routes(routes!(handlers::settings::get_ignored_podcasts))
        // Language preference endpoints
        .routes(routes!(handlers::settings::get_user_language))
        .routes(routes!(handlers::settings::update_user_language))
        .routes(routes!(handlers::settings::get_available_languages))
        .routes(routes!(handlers::settings::get_server_default_language))
        // Add more data routes as needed
}

fn create_podcast_routes() -> Router<AppState> {
    Router::new()
        .route("/notification_status", post(handlers::podcasts::get_notification_status))
}

fn create_episode_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::episodes::download_episode_file))
}

fn create_playlist_routes() -> Router<AppState> {
    Router::new()
        // Add playlist routes as needed
}

fn create_task_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::websocket::get_user_tasks))
        .routes(routes!(handlers::websocket::get_active_tasks))
        .routes(routes!(handlers::websocket::get_task_status))
}

fn create_async_routes() -> Router<AppState> {
    Router::new()
        // .route("/download_episode", post(handlers::tasks::download_episode))
        // .route("/import_opml", post(handlers::tasks::import_opml))
        // .route("/refresh_feeds", post(handlers::tasks::refresh_all_feeds))
        // .route("/episode/{episode_id}/metadata", get(handlers::tasks::quick_metadata_fetch))
}

fn create_proxy_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::proxy::proxy_image))
}

fn create_gpodder_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::sync::gpodder_test_connection))
        .routes(routes!(handlers::sync::gpodder_set_default))
        .routes(routes!(handlers::sync::gpodder_get_user_devices))
        .routes(routes!(handlers::sync::gpodder_get_all_devices, handlers::sync::gpodder_create_device))
        .routes(routes!(handlers::sync::gpodder_get_default_device))
        .routes(routes!(handlers::sync::gpodder_force_sync))
        .routes(routes!(handlers::sync::gpodder_sync))
        .routes(routes!(handlers::sync::gpodder_get_statistics))
}

fn create_init_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::tasks::startup_tasks))
}

fn create_feed_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::feed::get_user_feed))
}

fn create_websocket_routes() -> Router<AppState> {
    Router::new()
        .route("/api/tasks/{user_id}", get(handlers::websocket::task_progress_websocket))
        .route("/api/data/episodes/{user_id}", get(handlers::refresh::websocket_refresh_episodes))
}

fn create_auth_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::auth::store_oidc_state))
        .routes(routes!(handlers::auth::oidc_callback))
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