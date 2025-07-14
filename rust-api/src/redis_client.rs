use redis::{aio::ConnectionManager, AsyncCommands, Client};
use std::time::Duration;
use crate::{config::Config, error::{AppError, AppResult}};

#[derive(Clone)]
pub struct RedisClient {
    connection_manager: ConnectionManager,
}

impl RedisClient {
    pub async fn new(config: &Config) -> AppResult<Self> {
        let redis_url = config.redis_url();
        
        let client = Client::open(redis_url)?;
        let connection_manager = ConnectionManager::new(client).await?;

        // Test the connection
        let mut conn = connection_manager.clone();
        let _: String = redis::cmd("PING").query_async(&mut conn).await?;
        
        tracing::info!("Successfully connected to Redis/Valkey");
        
        Ok(RedisClient {
            connection_manager,
        })
    }

    pub async fn health_check(&self) -> AppResult<bool> {
        let mut conn = self.connection_manager.clone();
        let result: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(result == "PONG")
    }

    pub async fn get<T>(&self, key: &str) -> AppResult<Option<T>>
    where
        T: redis::FromRedisValue,
    {
        let mut conn = self.connection_manager.clone();
        let result: Option<T> = conn.get(key).await?;
        Ok(result)
    }

    pub async fn set<T>(&self, key: &str, value: T) -> AppResult<()>
    where
        T: redis::ToRedisArgs,
    {
        let mut conn = self.connection_manager.clone();
        conn.set(key, value).await?;
        Ok(())
    }

    pub async fn set_ex<T>(&self, key: &str, value: T, seconds: u64) -> AppResult<()>
    where
        T: redis::ToRedisArgs,
    {
        let mut conn = self.connection_manager.clone();
        conn.set_ex(key, value, seconds).await?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> AppResult<bool> {
        let mut conn = self.connection_manager.clone();
        let result: bool = conn.del(key).await?;
        Ok(result)
    }

    pub async fn exists(&self, key: &str) -> AppResult<bool> {
        let mut conn = self.connection_manager.clone();
        let result: bool = conn.exists(key).await?;
        Ok(result)
    }

    pub async fn expire(&self, key: &str, seconds: u64) -> AppResult<bool> {
        let mut conn = self.connection_manager.clone();
        let result: bool = conn.expire(key, seconds).await?;
        Ok(result)
    }

    pub async fn incr(&self, key: &str) -> AppResult<i64> {
        let mut conn = self.connection_manager.clone();
        let result: i64 = conn.incr(key, 1).await?;
        Ok(result)
    }

    pub async fn decr(&self, key: &str) -> AppResult<i64> {
        let mut conn = self.connection_manager.clone();
        let result: i64 = conn.decr(key, 1).await?;
        Ok(result)
    }

    // Session management
    pub async fn store_session(&self, session_id: &str, user_id: i32, ttl_seconds: u64) -> AppResult<()> {
        let session_key = format!("session:{}", session_id);
        self.set_ex(&session_key, user_id, ttl_seconds).await
    }

    pub async fn get_session(&self, session_id: &str) -> AppResult<Option<i32>> {
        let session_key = format!("session:{}", session_id);
        self.get(&session_key).await
    }

    pub async fn delete_session(&self, session_id: &str) -> AppResult<bool> {
        let session_key = format!("session:{}", session_id);
        self.delete(&session_key).await
    }

    // API key caching
    pub async fn cache_api_key_validation(&self, api_key: &str, is_valid: bool, ttl_seconds: u64) -> AppResult<()> {
        let cache_key = format!("api_key:{}", api_key);
        self.set_ex(&cache_key, is_valid, ttl_seconds).await
    }

    pub async fn get_cached_api_key_validation(&self, api_key: &str) -> AppResult<Option<bool>> {
        let cache_key = format!("api_key:{}", api_key);
        self.get(&cache_key).await
    }

    // Rate limiting
    pub async fn check_rate_limit(&self, identifier: &str, limit: u32, window_seconds: u64) -> AppResult<bool> {
        let rate_key = format!("rate_limit:{}", identifier);
        
        let mut conn = self.connection_manager.clone();
        let current_count: i64 = conn.incr(&rate_key, 1).await?;
        
        if current_count == 1 {
            conn.expire(&rate_key, window_seconds).await?;
        }
        
        Ok(current_count <= limit as i64)
    }

    // Background task tracking
    pub async fn store_task_status(&self, task_id: &str, status: &str, ttl_seconds: u64) -> AppResult<()> {
        let task_key = format!("task:{}", task_id);
        self.set_ex(&task_key, status, ttl_seconds).await
    }

    pub async fn get_task_status(&self, task_id: &str) -> AppResult<Option<String>> {
        let task_key = format!("task:{}", task_id);
        self.get(&task_key).await
    }

    // Podcast refresh tracking
    pub async fn set_podcast_refreshing(&self, podcast_id: i32) -> AppResult<()> {
        let refresh_key = format!("refreshing:{}", podcast_id);
        self.set_ex(&refresh_key, true, 300).await // 5 minute timeout
    }

    pub async fn is_podcast_refreshing(&self, podcast_id: i32) -> AppResult<bool> {
        let refresh_key = format!("refreshing:{}", podcast_id);
        Ok(self.exists(&refresh_key).await.unwrap_or(false))
    }

    pub async fn clear_podcast_refreshing(&self, podcast_id: i32) -> AppResult<bool> {
        let refresh_key = format!("refreshing:{}", podcast_id);
        self.delete(&refresh_key).await
    }
}