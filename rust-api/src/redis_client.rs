use redis::{aio::ConnectionManager, AsyncCommands, Client};
use crate::{config::Config, error::AppResult};

#[derive(Clone)]
pub struct RedisClient {
    connection: ConnectionManager,
    /// Kept so we can open dedicated pub/sub connections (a subscribing
    /// connection can't be shared/multiplexed like the ConnectionManager).
    client: Client,
}

impl RedisClient {
    pub async fn new(config: &Config) -> AppResult<Self> {
        let redis_url = config.redis_url();

        let client = Client::open(redis_url)?;
        let connection = ConnectionManager::new(client.clone()).await?;

        tracing::info!("Successfully connected to Redis/Valkey");

        Ok(RedisClient {
            connection,
            client,
        })
    }

    /// Publish a message on a channel (fire-and-forget fan-out). Uses the shared
    /// connection — publishing, unlike subscribing, doesn't need a dedicated one.
    /// Returns the number of subscribers that received it.
    pub async fn publish(&self, channel: &str, payload: &str) -> AppResult<i64> {
        let mut conn = self.connection.clone();
        let count: i64 = conn.publish(channel, payload).await?;
        Ok(count)
    }

    /// Open a dedicated pub/sub connection for subscribing. Separate from the
    /// multiplexed ConnectionManager because a subscribing connection can't also
    /// serve normal commands.
    pub async fn get_pubsub(&self) -> AppResult<redis::aio::PubSub> {
        Ok(self.client.get_async_pubsub().await?)
    }

    pub async fn health_check(&self) -> AppResult<bool> {
        let mut conn = self.connection.clone();
        let result: String = redis::cmd("PING").query_async(&mut conn).await?;
        Ok(result == "PONG")
    }

    pub async fn get<T>(&self, key: &str) -> AppResult<Option<T>>
    where
        T: redis::FromRedisValue,
    {
        let mut conn = self.connection.clone();
        let result: Option<T> = conn.get(key).await?;
        Ok(result)
    }

    pub async fn set<T>(&self, key: &str, value: T) -> AppResult<()>
    where
        T: redis::ToRedisArgs + redis::ToSingleRedisArg + Send + Sync,
    {
        let mut conn = self.connection.clone();
        let _: () = conn.set(key, value).await?;
        Ok(())
    }

    pub async fn set_ex<T>(&self, key: &str, value: T, seconds: u64) -> AppResult<()>
    where
        T: redis::ToRedisArgs + redis::ToSingleRedisArg + Send + Sync,
    {
        let mut conn = self.connection.clone();
        let _: () = conn.set_ex(key, value, seconds).await?;
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> AppResult<bool> {
        let mut conn = self.connection.clone();
        let result: bool = conn.del(key).await?;
        Ok(result)
    }

    pub async fn exists(&self, key: &str) -> AppResult<bool> {
        let mut conn = self.connection.clone();
        let result: bool = conn.exists(key).await?;
        Ok(result)
    }

    pub async fn expire(&self, key: &str, seconds: u64) -> AppResult<bool> {
        let mut conn = self.connection.clone();
        let result: bool = conn.expire(key, seconds as i64).await?;
        Ok(result)
    }

    pub async fn incr(&self, key: &str) -> AppResult<i64> {
        let mut conn = self.connection.clone();
        let result: i64 = conn.incr(key, 1).await?;
        Ok(result)
    }

    pub async fn decr(&self, key: &str) -> AppResult<i64> {
        let mut conn = self.connection.clone();
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
        
        let mut conn = self.connection.clone();
        let current_count: i64 = conn.incr(&rate_key, 1).await?;
        
        if current_count == 1 {
            let _: () = conn.expire(&rate_key, window_seconds as i64).await?;
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

    // Atomic get and delete operation - critical for OIDC state management
    pub async fn get_del(&self, key: &str) -> AppResult<Option<String>> {
        let mut conn = self.connection.clone();
        let result: Option<String> = redis::cmd("GETDEL").arg(key).query_async(&mut conn).await?;
        Ok(result)
    }

    // Get a connection for direct Redis operations
    pub async fn get_connection(&self) -> AppResult<ConnectionManager> {
        Ok(self.connection.clone())
    }

    // ----- Now-playing sessions (ephemeral, best-effort mirror) -----
    //
    // Layout: `nowplaying:{user_id}:{device_id}` holds a JSON snapshot with a short
    // TTL (refreshed by each report/heartbeat). `nowplaying:devices:{user_id}` is a
    // SET indexing that user's device_ids so we can enumerate without SCAN; stale
    // members are pruned lazily on read when their snapshot key has expired.

    const NOW_PLAYING_TTL_SECONDS: u64 = 45;

    fn now_playing_key(user_id: i32, device_id: &str) -> String {
        format!("nowplaying:{}:{}", user_id, device_id)
    }

    fn now_playing_index_key(user_id: i32) -> String {
        format!("nowplaying:devices:{}", user_id)
    }

    /// Write/refresh a device's now-playing snapshot with a TTL, and index the
    /// device under the user. `updated_at` is stamped here so ordering is
    /// server-authoritative regardless of client clock skew.
    pub async fn upsert_now_playing(
        &self,
        user_id: i32,
        mut snapshot: crate::models::NowPlayingSnapshot,
    ) -> AppResult<()> {
        snapshot.updated_at = chrono::Utc::now().timestamp();
        let device_id = snapshot.device_id.clone();
        let json = serde_json::to_string(&snapshot)
            .map_err(|e| crate::error::AppError::internal(format!("serialize now-playing: {}", e)))?;

        let key = Self::now_playing_key(user_id, &device_id);
        let index = Self::now_playing_index_key(user_id);
        let mut conn = self.connection.clone();
        let _: () = conn.set_ex(&key, json, Self::NOW_PLAYING_TTL_SECONDS).await?;
        let _: () = conn.sadd(&index, &device_id).await?;
        // Keep the index around a bit longer than a single snapshot so a briefly
        // paused/quiet device isn't dropped from the set prematurely; pruning on
        // read handles genuinely-expired devices.
        let _: () = conn
            .expire(&index, (Self::NOW_PLAYING_TTL_SECONDS * 4) as i64)
            .await?;
        Ok(())
    }

    /// List a user's active devices and their snapshots, pruning any whose snapshot
    /// key has expired.
    pub async fn get_now_playing_devices(
        &self,
        user_id: i32,
    ) -> AppResult<Vec<crate::models::NowPlayingSnapshot>> {
        let index = Self::now_playing_index_key(user_id);
        let mut conn = self.connection.clone();
        let device_ids: Vec<String> = conn.smembers(&index).await?;

        let mut out = Vec::with_capacity(device_ids.len());
        for device_id in device_ids {
            let key = Self::now_playing_key(user_id, &device_id);
            let raw: Option<String> = conn.get(&key).await?;
            match raw {
                Some(json) => {
                    match serde_json::from_str::<crate::models::NowPlayingSnapshot>(&json) {
                        Ok(snapshot) => out.push(snapshot),
                        // Corrupt/legacy entry: drop it from the index.
                        Err(_) => {
                            let _: () = conn.srem(&index, &device_id).await?;
                        }
                    }
                }
                // Snapshot expired: prune the stale index member.
                None => {
                    let _: () = conn.srem(&index, &device_id).await?;
                }
            }
        }
        Ok(out)
    }

    /// List a device with `snapshot` only if it has no live snapshot yet (SET NX),
    /// so a freshly-connected or long-quiet device is visible/targetable as idle
    /// without clobbering a fresher report.
    pub async fn ensure_now_playing_present(
        &self,
        user_id: i32,
        mut snapshot: crate::models::NowPlayingSnapshot,
    ) -> AppResult<()> {
        snapshot.updated_at = chrono::Utc::now().timestamp();
        let device_id = snapshot.device_id.clone();
        let json = serde_json::to_string(&snapshot)
            .map_err(|e| crate::error::AppError::internal(format!("serialize now-playing: {}", e)))?;

        let key = Self::now_playing_key(user_id, &device_id);
        let index = Self::now_playing_index_key(user_id);
        let mut conn = self.connection.clone();
        let _: Option<String> = redis::cmd("SET")
            .arg(&key)
            .arg(json)
            .arg("NX")
            .arg("EX")
            .arg(Self::NOW_PLAYING_TTL_SECONDS)
            .query_async(&mut conn)
            .await?;
        let _: () = conn.sadd(&index, &device_id).await?;
        let _: () = conn
            .expire(&index, (Self::NOW_PLAYING_TTL_SECONDS * 4) as i64)
            .await?;
        Ok(())
    }

    /// Extend a device's snapshot TTL without changing its contents (heartbeat while
    /// paused). Returns false if the snapshot had already expired (nothing extended).
    pub async fn touch_now_playing(&self, user_id: i32, device_id: &str) -> AppResult<bool> {
        let key = Self::now_playing_key(user_id, device_id);
        let index = Self::now_playing_index_key(user_id);
        let mut conn = self.connection.clone();
        let alive: bool = conn.expire(&key, Self::NOW_PLAYING_TTL_SECONDS as i64).await?;
        if alive {
            // Re-index too, in case the index set expired on its own.
            let _: () = conn.sadd(&index, device_id).await?;
        }
        let _: () = conn
            .expire(&index, (Self::NOW_PLAYING_TTL_SECONDS * 4) as i64)
            .await?;
        Ok(alive)
    }

    /// Whether a device currently has a live snapshot (i.e. is connected somewhere).
    pub async fn now_playing_exists(&self, user_id: i32, device_id: &str) -> AppResult<bool> {
        self.exists(&Self::now_playing_key(user_id, device_id)).await
    }

    /// Remove a device's snapshot (e.g. on socket disconnect).
    pub async fn remove_now_playing(&self, user_id: i32, device_id: &str) -> AppResult<()> {
        let key = Self::now_playing_key(user_id, device_id);
        let index = Self::now_playing_index_key(user_id);
        let mut conn = self.connection.clone();
        let _: () = conn.del(&key).await?;
        let _: () = conn.srem(&index, device_id).await?;
        Ok(())
    }

    /// The user's active session anchor for the queue: the current episode to
    /// insert new queue items under when a client didn't pass its own hint.
    /// Prefers a device that is actively playing; otherwise falls back to the most
    /// recently active device that still has an episode loaded (a paused episode is
    /// still the current one and remains the anchor).
    pub async fn active_session_anchor(&self, user_id: i32) -> AppResult<Option<(i32, bool)>> {
        let devices = self.get_now_playing_devices(user_id).await?;
        let anchor = devices
            .iter()
            .filter(|d| d.playing)
            .max_by_key(|d| d.updated_at)
            .or_else(|| {
                devices
                    .iter()
                    .filter(|d| d.episode_id != 0)
                    .max_by_key(|d| d.updated_at)
            });
        Ok(anchor.map(|d| (d.episode_id, d.is_youtube)))
    }
}