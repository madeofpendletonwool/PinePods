use sqlx::{MySql, Pool, Postgres, Row};
use std::time::Duration;
use crate::{config::Config, error::{AppError, AppResult}};

#[derive(Clone)]
pub enum DatabasePool {
    Postgres(Pool<Postgres>),
    MySQL(Pool<MySql>),
}

impl DatabasePool {
    pub async fn new(config: &Config) -> AppResult<Self> {
        let database_url = config.database_url();
        
        match config.database.db_type.as_str() {
            "postgresql" => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(config.database.max_connections)
                    .min_connections(config.database.min_connections)
                    .acquire_timeout(Duration::from_secs(30))
                    .connect(&database_url)
                    .await?;

                // Run migrations if needed
                sqlx::migrate!("./migrations/postgres").run(&pool).await?;
                
                Ok(DatabasePool::Postgres(pool))
            }
            _ => {
                let pool = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(config.database.max_connections)
                    .min_connections(config.database.min_connections)
                    .acquire_timeout(Duration::from_secs(30))
                    .connect(&database_url)
                    .await?;

                // Run migrations if needed
                sqlx::migrate!("./migrations/mysql").run(&pool).await?;
                
                Ok(DatabasePool::MySQL(pool))
            }
        }
    }

    pub async fn health_check(&self) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row: (i32,) = sqlx::query_as("SELECT 1")
                    .fetch_one(pool)
                    .await?;
                Ok(row.0 == 1)
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT 1 as health")
                    .fetch_one(pool)
                    .await?;
                let health: i32 = row.try_get("health")?;
                Ok(health == 1)
            }
        }
    }

    // Helper methods for database operations
    pub async fn verify_api_key(&self, api_key: &str) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let result = sqlx::query!(
                    r#"SELECT COUNT(*) as count FROM "UserSettings" WHERE api_key = $1"#,
                    api_key
                )
                .fetch_one(pool)
                .await?;
                
                Ok(result.count.unwrap_or(0) > 0)
            }
            DatabasePool::MySQL(pool) => {
                let result = sqlx::query!(
                    "SELECT COUNT(*) as count FROM UserSettings WHERE api_key = ?",
                    api_key
                )
                .fetch_one(pool)
                .await?;
                
                Ok(result.count > 0)
            }
        }
    }

    pub async fn get_user_by_credentials(&self, username: &str) -> AppResult<Option<UserCredentials>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let result = sqlx::query_as!(
                    UserCredentials,
                    r#"SELECT "UserID" as user_id, "Username" as username, "Hashed_PW" as hashed_password, "Email" as email
                     FROM "Users" WHERE "Username" = $1"#,
                    username
                )
                .fetch_optional(pool)
                .await?;
                
                Ok(result)
            }
            DatabasePool::MySQL(pool) => {
                let result = sqlx::query_as!(
                    UserCredentials,
                    "SELECT UserID as user_id, Username as username, Hashed_PW as hashed_password, Email as email
                     FROM Users WHERE Username = ?",
                    username
                )
                .fetch_optional(pool)
                .await?;
                
                Ok(result)
            }
        }
    }

    pub async fn get_user_settings_by_user_id(&self, user_id: i32) -> AppResult<Option<UserSettings>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let result = sqlx::query_as!(
                    UserSettings,
                    r#"SELECT user_id, api_key, theme, auto_download_episodes, auto_delete_episodes, 
                              api_key as "api_key!", theme as "theme!", auto_download_episodes as "auto_download_episodes!", 
                              auto_delete_episodes as "auto_delete_episodes!"
                       FROM "UserSettings" WHERE user_id = $1"#,
                    user_id
                )
                .fetch_optional(pool)
                .await?;
                
                Ok(result)
            }
            DatabasePool::MySQL(pool) => {
                let result = sqlx::query_as!(
                    UserSettings,
                    "SELECT user_id, api_key, theme, auto_download_episodes, auto_delete_episodes
                     FROM UserSettings WHERE user_id = ?",
                    user_id
                )
                .fetch_optional(pool)
                .await?;
                
                Ok(result)
            }
        }
    }

    // Get user ID by API key - matches Python get_api_user function
    pub async fn get_api_user(&self, api_key: &str) -> AppResult<i32> {
        match self {
            DatabasePool::Postgres(pool) => {
                let result = sqlx::query!(
                    r#"SELECT user_id FROM "UserSettings" WHERE api_key = $1"#,
                    api_key
                )
                .fetch_one(pool)
                .await?;
                
                Ok(result.user_id)
            }
            DatabasePool::MySQL(pool) => {
                let result = sqlx::query!(
                    "SELECT user_id FROM UserSettings WHERE api_key = ?",
                    api_key
                )
                .fetch_one(pool)
                .await?;
                
                Ok(result.user_id)
            }
        }
    }

    // Add more database operations as needed...
}

#[derive(Debug, Clone)]
pub struct UserCredentials {
    pub user_id: i32,
    pub username: String,
    pub hashed_password: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserSettings {
    pub user_id: i32,
    pub api_key: String,
    pub theme: String,
    pub auto_download_episodes: bool,
    pub auto_delete_episodes: bool,
}

// Migration runner for setup
pub async fn run_migrations(pool: &DatabasePool) -> AppResult<()> {
    match pool {
        DatabasePool::Postgres(pool) => {
            sqlx::migrate!("./migrations/postgres").run(pool).await?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::migrate!("./migrations/mysql").run(pool).await?;
        }
    }
    Ok(())
}