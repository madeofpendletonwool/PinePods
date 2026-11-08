use std::sync::Arc;

use redis::{aio::ConnectionManager, AsyncCommands, Client, FromRedisValue, ToRedisArgs};

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::memory_store::MemoryStore;

/// Valkey/Redis, or (with no Valkey configured) an in-process store. In-process works for one
/// server process; several replicas sharing tasks, notifications and now-playing need Valkey.
#[derive(Clone)]
pub struct RedisClient {
    backend: Backend,
}

#[derive(Clone)]
enum Backend {
    Valkey {
        connection: ConnectionManager,
        /// Kept so we can open dedicated pub/sub connections (a subscribing
        /// connection can't be shared/multiplexed like the ConnectionManager).
        client: Client,
    },
    Memory(Arc<MemoryStore>),
}

/// A value as Valkey would store it (one argument).
fn encode<T: ToRedisArgs>(value: &T) -> Vec<u8> {
    value.to_redis_args().into_iter().next().unwrap_or_default()
}

/// A stored value read back as Valkey's client would decode it.
fn decode<T: FromRedisValue>(raw: Option<Vec<u8>>) -> AppResult<Option<T>> {
    match raw {
        None => Ok(None),
        Some(bytes) => T::from_redis_value(redis::Value::BulkString(bytes))
            .map(Some)
            .map_err(|e| AppError::internal(format!("decode stored value: {e}"))),
    }
}

fn secs(seconds: u64) -> std::time::Duration {
    std::time::Duration::from_secs(seconds)
}

impl RedisClient {
    pub async fn new(config: &Config) -> AppResult<Self> {
        let Some(redis_url) = config.redis_url() else {
            tracing::info!("No Valkey configured: caches, task progress and now-playing are kept in memory");
            return Ok(Self::in_memory());
        };

        let client = Client::open(redis_url)?;
        let connection = ConnectionManager::new(client.clone()).await?;

        tracing::info!("Successfully connected to Redis/Valkey");

        Ok(RedisClient { backend: Backend::Valkey { connection, client } })
    }

    /// The in-process store.
    pub fn in_memory() -> Self {
        RedisClient { backend: Backend::Memory(MemoryStore::new()) }
    }

    /// Whether this is a Valkey server other processes can share, so the cross-replica
    /// pub/sub bridges have something to do.
    pub fn is_shared(&self) -> bool {
        matches!(self.backend, Backend::Valkey { .. })
    }

    /// Publish a message on a channel (fire-and-forget fan-out). Uses the shared
    /// connection — publishing, unlike subscribing, doesn't need a dedicated one.
    /// Returns the number of subscribers that received it (none in memory: there are no
    /// other processes).
    pub async fn publish(&self, channel: &str, payload: &str) -> AppResult<i64> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                Ok(conn.publish(channel, payload).await?)
            }
            Backend::Memory(_) => Ok(0),
        }
    }

    /// Open a dedicated pub/sub connection for subscribing. Separate from the
    /// multiplexed ConnectionManager because a subscribing connection can't also
    /// serve normal commands.
    pub async fn get_pubsub(&self) -> AppResult<redis::aio::PubSub> {
        match &self.backend {
            Backend::Valkey { client, .. } => Ok(client.get_async_pubsub().await?),
            Backend::Memory(_) => Err(AppError::internal("pub/sub needs Valkey")),
        }
    }

    pub async fn health_check(&self) -> AppResult<bool> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                let result: String = redis::cmd("PING").query_async(&mut conn).await?;
                Ok(result == "PONG")
            }
            Backend::Memory(_) => Ok(true),
        }
    }

    pub async fn get<T>(&self, key: &str) -> AppResult<Option<T>>
    where
        T: FromRedisValue,
    {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                Ok(conn.get(key).await?)
            }
            Backend::Memory(m) => decode(m.get(key)),
        }
    }

    pub async fn set<T>(&self, key: &str, value: T) -> AppResult<()>
    where
        T: ToRedisArgs + redis::ToSingleRedisArg + Send + Sync,
    {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                let _: () = conn.set(key, value).await?;
            }
            Backend::Memory(m) => {
                m.set(key, encode(&value), None, false);
            }
        }
        Ok(())
    }

    pub async fn set_ex<T>(&self, key: &str, value: T, seconds: u64) -> AppResult<()>
    where
        T: ToRedisArgs + redis::ToSingleRedisArg + Send + Sync,
    {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                let _: () = conn.set_ex(key, value, seconds).await?;
            }
            Backend::Memory(m) => {
                m.set(key, encode(&value), Some(secs(seconds)), false);
            }
        }
        Ok(())
    }

    /// SET NX EX: set only if the key doesn't exist yet. Whether it was set.
    pub async fn set_nx_ex<T>(&self, key: &str, value: T, seconds: u64) -> AppResult<bool>
    where
        T: ToRedisArgs + redis::ToSingleRedisArg + Send + Sync,
    {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                let set: Option<String> = redis::cmd("SET")
                    .arg(key)
                    .arg(value)
                    .arg("NX")
                    .arg("EX")
                    .arg(seconds)
                    .query_async(&mut conn)
                    .await?;
                Ok(set.is_some())
            }
            Backend::Memory(m) => Ok(m.set(key, encode(&value), Some(secs(seconds)), true)),
        }
    }

    pub async fn delete(&self, key: &str) -> AppResult<bool> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                Ok(conn.del(key).await?)
            }
            Backend::Memory(m) => Ok(m.del(key)),
        }
    }

    pub async fn exists(&self, key: &str) -> AppResult<bool> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                Ok(conn.exists(key).await?)
            }
            Backend::Memory(m) => Ok(m.exists(key)),
        }
    }

    pub async fn expire(&self, key: &str, seconds: u64) -> AppResult<bool> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                Ok(conn.expire(key, seconds as i64).await?)
            }
            Backend::Memory(m) => Ok(m.expire(key, secs(seconds))),
        }
    }

    async fn incr_by(&self, key: &str, by: i64) -> AppResult<i64> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                Ok(conn.incr(key, by).await?)
            }
            Backend::Memory(m) => m.incr_by(key, by).map_err(AppError::internal),
        }
    }

    pub async fn incr(&self, key: &str) -> AppResult<i64> {
        self.incr_by(key, 1).await
    }

    pub async fn decr(&self, key: &str) -> AppResult<i64> {
        self.incr_by(key, -1).await
    }

    pub async fn sadd(&self, key: &str, member: &str) -> AppResult<()> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                let _: () = conn.sadd(key, member).await?;
            }
            Backend::Memory(m) => {
                m.sadd(key, member);
            }
        }
        Ok(())
    }

    pub async fn srem(&self, key: &str, member: &str) -> AppResult<()> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                let _: () = conn.srem(key, member).await?;
            }
            Backend::Memory(m) => {
                m.srem(key, member);
            }
        }
        Ok(())
    }

    pub async fn smembers(&self, key: &str) -> AppResult<Vec<String>> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                Ok(conn.smembers(key).await?)
            }
            Backend::Memory(m) => Ok(m.smembers(key)),
        }
    }

    // Atomic get and delete operation - critical for OIDC state management
    pub async fn get_del(&self, key: &str) -> AppResult<Option<String>> {
        match &self.backend {
            Backend::Valkey { connection, .. } => {
                let mut conn = connection.clone();
                Ok(redis::cmd("GETDEL").arg(key).query_async(&mut conn).await?)
            }
            Backend::Memory(m) => decode(m.get_del(key)),
        }
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
        let current_count = self.incr(&rate_key).await?;
        if current_count == 1 {
            self.expire(&rate_key, window_seconds).await?;
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
            .map_err(|e| AppError::internal(format!("serialize now-playing: {}", e)))?;

        let key = Self::now_playing_key(user_id, &device_id);
        let index = Self::now_playing_index_key(user_id);
        self.set_ex(&key, json, Self::NOW_PLAYING_TTL_SECONDS).await?;
        self.sadd(&index, &device_id).await?;
        // Keep the index around a bit longer than a single snapshot so a briefly
        // paused/quiet device isn't dropped from the set prematurely; pruning on
        // read handles genuinely-expired devices.
        self.expire(&index, Self::NOW_PLAYING_TTL_SECONDS * 4).await?;
        Ok(())
    }

    /// List a user's active devices and their snapshots, pruning any whose snapshot
    /// key has expired.
    pub async fn get_now_playing_devices(
        &self,
        user_id: i32,
    ) -> AppResult<Vec<crate::models::NowPlayingSnapshot>> {
        let index = Self::now_playing_index_key(user_id);
        let device_ids = self.smembers(&index).await?;

        let mut out = Vec::with_capacity(device_ids.len());
        for device_id in device_ids {
            let key = Self::now_playing_key(user_id, &device_id);
            let raw: Option<String> = self.get(&key).await?;
            match raw {
                Some(json) => {
                    match serde_json::from_str::<crate::models::NowPlayingSnapshot>(&json) {
                        Ok(snapshot) => out.push(snapshot),
                        // Corrupt/legacy entry: drop it from the index.
                        Err(_) => self.srem(&index, &device_id).await?,
                    }
                }
                // Snapshot expired: prune the stale index member.
                None => self.srem(&index, &device_id).await?,
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
            .map_err(|e| AppError::internal(format!("serialize now-playing: {}", e)))?;

        let key = Self::now_playing_key(user_id, &device_id);
        let index = Self::now_playing_index_key(user_id);
        self.set_nx_ex(&key, json, Self::NOW_PLAYING_TTL_SECONDS).await?;
        self.sadd(&index, &device_id).await?;
        self.expire(&index, Self::NOW_PLAYING_TTL_SECONDS * 4).await?;
        Ok(())
    }

    /// Extend a device's snapshot TTL without changing its contents (heartbeat while
    /// paused). Returns false if the snapshot had already expired (nothing extended).
    pub async fn touch_now_playing(&self, user_id: i32, device_id: &str) -> AppResult<bool> {
        let key = Self::now_playing_key(user_id, device_id);
        let index = Self::now_playing_index_key(user_id);
        let alive = self.expire(&key, Self::NOW_PLAYING_TTL_SECONDS).await?;
        if alive {
            // Re-index too, in case the index set expired on its own.
            self.sadd(&index, device_id).await?;
        }
        self.expire(&index, Self::NOW_PLAYING_TTL_SECONDS * 4).await?;
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
        self.delete(&key).await?;
        self.srem(&index, device_id).await?;
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

#[cfg(test)]
mod tests {
    use super::RedisClient;

    /// The in-memory store hands values back as Valkey's client would decode them.
    #[tokio::test]
    async fn memory_values_round_trip_like_valkey() {
        let c = RedisClient::in_memory();
        c.set_ex("b", true, 60).await.unwrap();
        assert_eq!(c.get::<bool>("b").await.unwrap(), Some(true));
        c.set("i", 42i32).await.unwrap();
        assert_eq!(c.get::<i32>("i").await.unwrap(), Some(42));
        assert_eq!(c.incr("i").await.unwrap(), 43);
        c.set("bytes", vec![0u8, 255, 7]).await.unwrap();
        assert_eq!(c.get::<Vec<u8>>("bytes").await.unwrap(), Some(vec![0u8, 255, 7]));
        c.set("s", "text").await.unwrap();
        assert_eq!(c.get_del("s").await.unwrap().as_deref(), Some("text"));
        assert_eq!(c.get::<String>("s").await.unwrap(), None);
        assert!(c.set_nx_ex("nx", "a", 60).await.unwrap());
        assert!(!c.set_nx_ex("nx", "b", 60).await.unwrap());
        assert!(c.check_rate_limit("ip", 2, 60).await.unwrap());
        assert!(c.check_rate_limit("ip", 2, 60).await.unwrap());
        assert!(!c.check_rate_limit("ip", 2, 60).await.unwrap());
    }
}
