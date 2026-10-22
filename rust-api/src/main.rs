use std::net::SocketAddr;
use tokio::signal;
use tracing::{debug, error, info, warn};

use pinepods_api::{
    config::Config,
    create_app,
    database::DatabasePool,
    error::AppResult,
    handlers,
    openapi_router,
    redis_client::RedisClient,
    redis_manager::{ImportProgressManager, NotificationManager},
    services::{self, scheduler::BackgroundScheduler, task_manager::TaskManager, tasks::TaskSpawner},
    AppState,
};
use handlers::websocket::WebSocketManager;
use std::sync::Arc;

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

    // Unique id for this API process — created before the task manager so it can
    // stamp its cross-replica bridge packets and ignore its own echoes.
    let instance_id: Arc<str> = Arc::from(uuid::Uuid::new_v4().to_string().as_str());

    // Initialize task management
    let task_manager = Arc::new(TaskManager::new(redis_client.clone(), instance_id.clone()));
    let task_spawner = Arc::new(TaskSpawner::new(task_manager.clone(), db_pool.clone()));
    let websocket_manager = Arc::new(WebSocketManager::new());
    let now_playing_manager = Arc::new(handlers::nowplaying::NowPlayingManager::new());
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
        now_playing_manager,
        instance_id,
        import_progress_manager,
        notification_manager,
        restore_in_progress: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        ai_available: services::ai_client::AiAvailability::new(),
    };

    // Start the AI sidecar health monitor (no-op if PINEPODS_AI_URL is unset).
    services::ai_client::spawn_health_monitor(app_state.ai_available.clone());

    // Bridge now-playing reports/commands across API replicas via Valkey pub/sub.
    // Degrades cleanly to in-process fan-out if the subscribe connection can't be
    // established (the default single-container deploy needs no bridge).
    handlers::nowplaying::spawn_nowplaying_bridge(app_state.clone());

    // Bridge task-progress + notification events across API replicas via Valkey
    // pub/sub. Same best-effort pattern: re-injects remote events into the local
    // broadcast so sockets on this replica see tasks/messages raised elsewhere.
    handlers::websocket::spawn_task_bridge(app_state.clone());

    // Push local changes to users' external sync servers shortly after they happen.
    services::sync::push::init(&app_state.task_spawner, services::sync::push::PushDelays::from_env());

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

    // The startup refresh runs in the background once requests are accepted.
    tokio::spawn(BackgroundScheduler::run_startup_refresh(scheduler_state.clone()));
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
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