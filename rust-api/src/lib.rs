// Library exports for PinePods Rust API
// This allows the modules to be tested

pub mod config;
pub mod database;
pub mod error;
pub mod handlers;
pub mod models;
pub mod redis_client;
pub mod redis_manager;
pub mod services;

pub use config::Config;
pub use database::DatabasePool;
pub use error::{AppError, AppResult};
pub use redis_client::RedisClient;

use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: DatabasePool,
    pub redis_client: RedisClient,
    pub config: Config,
    pub task_manager: Arc<services::task_manager::TaskManager>,
    pub task_spawner: Arc<services::tasks::TaskSpawner>,
    pub websocket_manager: Arc<handlers::websocket::WebSocketManager>,
    pub import_progress_manager: Arc<redis_manager::ImportProgressManager>,
    pub notification_manager: Arc<redis_manager::NotificationManager>,
}
