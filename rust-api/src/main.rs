use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post, put},
    Router,
};
use std::net::SocketAddr;
use tokio::signal;
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::TraceLayer,
    compression::CompressionLayer,
};
use tracing::{info, warn};

mod config;
mod database;
mod error;
mod handlers;
mod models;
mod redis_client;
mod services;

use config::Config;
use database::DatabasePool;
use error::AppResult;
use redis_client::RedisClient;
use services::{task_manager::TaskManager, tasks::TaskSpawner};
use handlers::websocket::WebSocketManager;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: DatabasePool,
    pub redis_client: RedisClient,
    pub config: Config,
    pub task_manager: Arc<TaskManager>,
    pub task_spawner: Arc<TaskSpawner>,
    pub websocket_manager: Arc<WebSocketManager>,
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
    let task_spawner = Arc::new(TaskSpawner::new(task_manager.clone()));
    let websocket_manager = Arc::new(WebSocketManager::new());
    info!("Task management system initialized");

    // Create shared application state
    let app_state = AppState {
        db_pool,
        redis_client,
        config: config.clone(),
        task_manager,
        task_spawner,
        websocket_manager,
    };

    // Build the application with routes
    let app = create_app(app_state);

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
        .nest("/api/podcasts", create_podcast_routes())
        .nest("/api/episodes", create_episode_routes())
        .nest("/api/playlists", create_playlist_routes())
        .nest("/api/tasks", create_task_routes())
        .nest("/api/async", create_async_routes())
        .nest("/api/proxy", create_proxy_routes())
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
                .layer(
                    CorsLayer::new()
                        .allow_origin(Any)
                        .allow_methods(Any)
                        .allow_headers(Any),
                )
        )
        .with_state(state)
}

fn create_data_routes() -> Router<AppState> {
    Router::new()
        .route("/get_key", get(handlers::auth::get_key))
        .route("/verify_key", get(handlers::auth::verify_api_key_endpoint))
        .route("/get_user", get(handlers::auth::get_user))
        .route("/user_details_id/{user_id}", get(handlers::auth::get_user_details_by_id))
        .route("/self_service_status", get(handlers::auth::get_self_service_status))
        .route("/public_oidc_providers", get(handlers::auth::get_public_oidc_providers))
        .route("/create_first", post(handlers::auth::create_first_admin))
        .route("/config", get(handlers::auth::get_config))
        .route("/first_login_done/{user_id}", get(handlers::auth::first_login_done))
        .route("/check_mfa_enabled/{user_id}", get(handlers::auth::check_mfa_enabled))
        .route("/get_theme/{user_id}", get(handlers::auth::get_theme))
        .route("/startpage", get(handlers::auth::get_user_startpage))
        .route("/setup_time_info", post(handlers::auth::setup_time_info))
        .route("/user_admin_check/{user_id}", get(handlers::auth::user_admin_check))
        .route("/import_opml", post(handlers::auth::import_opml))
        .route("/import_progress/{user_id}", get(handlers::auth::import_progress))
        .route("/return_episodes/{user_id}", get(handlers::podcasts::return_episodes))
        .route("/add_podcast", post(handlers::podcasts::add_podcast))
        .route("/remove_podcast", post(handlers::podcasts::remove_podcast))
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
        .route("/user_history/{user_id}", get(handlers::podcasts::get_user_history))
        .route("/get_podcast_id", get(handlers::podcasts::get_podcast_id))
        .route("/download_episode_list", get(handlers::podcasts::download_episode_list))
        .route("/download_podcast", post(handlers::podcasts::download_podcast))
        .route("/delete_episode", post(handlers::podcasts::delete_episode))
        .route("/download_all_podcast", post(handlers::podcasts::download_all_podcast))
        .route("/download_status/{user_id}", get(handlers::podcasts::download_status))
        .route("/podcast_episodes", get(handlers::podcasts::podcast_episodes))
        .route("/get_podcast_id_from_ep_name", get(handlers::podcasts::get_podcast_id_from_ep_name))
        .route("/get_episode_metadata", post(handlers::podcasts::get_episode_metadata))
        .route("/fetch_podcasting_2_data", get(handlers::podcasts::fetch_podcasting_2_data))
        .route("/get_auto_download_status", post(handlers::podcasts::get_auto_download_status))
        .route("/get_feed_cutoff_days", get(handlers::podcasts::get_feed_cutoff_days))
        .route("/get_play_episode_details", post(handlers::podcasts::get_play_episode_details))
        .route("/fetch_podcasting_2_pod_data", get(handlers::podcasts::fetch_podcasting_2_pod_data))
        .route("/mark_episode_completed", post(handlers::podcasts::mark_episode_completed))
        .route("/increment_played/{user_id}", put(handlers::podcasts::increment_played))
        .route("/get_podcast_id_from_ep_id", get(handlers::podcasts::get_podcast_id_from_ep_id))
        .route("/refresh_pods", post(handlers::refresh::refresh_pods_admin))
        // Add more data routes as needed
}

fn create_podcast_routes() -> Router<AppState> {
    Router::new()
        .route("/notification_status", post(handlers::podcasts::get_notification_status))
}

fn create_episode_routes() -> Router<AppState> {
    Router::new()
        // Add episode routes as needed
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
        .route("/download_episode", post(handlers::async_tasks_examples::download_episode))
        .route("/import_opml", post(handlers::async_tasks_examples::import_opml))
        .route("/refresh_feeds", post(handlers::async_tasks_examples::refresh_all_feeds))
        .route("/episode/{episode_id}/metadata", get(handlers::async_tasks_examples::quick_metadata_fetch))
}

fn create_proxy_routes() -> Router<AppState> {
    Router::new()
        .route("/image", get(handlers::proxy::proxy_image))
}

fn create_websocket_routes() -> Router<AppState> {
    Router::new()
        .route("/tasks/{user_id}", get(handlers::websocket::task_progress_websocket))
        .route("/api/data/episodes/{user_id}", get(handlers::refresh::websocket_refresh_episodes))
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