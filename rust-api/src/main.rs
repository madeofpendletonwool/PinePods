use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
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

#[derive(Clone)]
pub struct AppState {
    pub db_pool: DatabasePool,
    pub redis_client: RedisClient,
    pub config: Config,
}

#[tokio::main]
async fn main() -> AppResult<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting PinePods Rust API");

    // Load configuration
    let config = Config::new()?;
    info!("Configuration loaded");

    // Initialize database pool
    let db_pool = DatabasePool::new(&config).await?;
    info!("Database pool initialized");

    // Initialize Redis client
    let redis_client = RedisClient::new(&config).await?;
    info!("Redis/Valkey client initialized");

    // Create shared application state
    let app_state = AppState {
        db_pool,
        redis_client,
        config: config.clone(),
    };

    // Build the application with routes
    let app = create_app(app_state);

    // Determine the address to bind to
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    info!("Server listening on {}", addr);

    // Start the server
    let listener = tokio::net::TcpListener::bind(addr).await?;
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
        
        // Middleware stack
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
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
        .route("/verify_key", get(handlers::auth::verify_api_key))
        .route("/get_user", get(handlers::auth::get_user))
        // Add more data routes as needed
}

fn create_podcast_routes() -> Router<AppState> {
    Router::new()
        // Add podcast routes as needed
}

fn create_episode_routes() -> Router<AppState> {
    Router::new()
        // Add episode routes as needed
}

fn create_playlist_routes() -> Router<AppState> {
    Router::new()
        // Add playlist routes as needed
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