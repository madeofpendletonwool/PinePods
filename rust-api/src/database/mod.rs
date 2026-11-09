//! Database access. Every query is a method on `DatabasePool`; the methods are split by area
//! into the submodules below, with shared types and helpers kept here.

use sqlx::{MySql, Pool, Postgres, Row, Sqlite};
use std::time::Duration;
use crate::{config::{Config, OIDCConfig}, error::{AppError, AppResult}};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use std::collections::HashMap;
use bigdecimal::ToPrimitive;
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;
use base64;
use tracing::{debug, error, info, warn};

pub mod dialect;
use dialect::{dialect_fn, dialect_match, dialect_method};

mod auth;
mod mfa;
mod oidc;
mod users;
mod user_settings;
mod server_settings;
mod podcasts;
mod podcast_settings;
mod episodes;
mod queue;
mod saved;
mod history;
mod stats;
mod home;
mod playlists;
mod collections;
mod refresh;
mod feed_parsing;
mod notifications;
mod in_app_notifications;
pub use in_app_notifications::StoredInAppNotification;
mod downloads;
mod transcripts;
pub use transcripts::{TranscriptMatch, TranscriptSearchEpisode};
mod youtube;
mod rss;
mod gpodder;
mod backup;
mod sqlite_backup;
mod shared_episodes;
mod recommendations;
mod local_podcasts;

pub use mfa::*;
pub use oidc::*;
pub use users::*;
pub use user_settings::*;
pub(crate) use podcasts::*;
pub use queue::*;
pub use playlists::*;
pub use refresh::*;
pub(crate) use notifications::*;
pub use gpodder::*;
pub use backup::*;

#[derive(Clone)]
pub enum DatabasePool {
    Postgres(Pool<Postgres>),
    MySQL(Pool<MySql>),
    /// Runs the PostgreSQL query code through [`dialect`].
    Sqlite(Pool<Sqlite>),
}

/// A transaction rolled back because it collided with a concurrent one: a deadlock or
/// serialization failure, or MariaDB's "record has changed since last read" (1020, snapshot
/// isolation). Running it again is safe.
fn is_write_conflict(e: &AppError) -> bool {
    let AppError::Database(sqlx::Error::Database(db)) = e else { return false };
    if matches!(db.code().as_deref(), Some("40001") | Some("40P01")) {
        return true;
    }
    db.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>().is_some_and(|m| matches!(m.number(), 1020 | 1213))
}

/// `Users.GpodderUrl` of users on the PinePods gpodder server. Not a real address any more;
/// clients and the UI read it as "internal".
pub const INTERNAL_GPODDER_URL: &str = "http://localhost:8042";

impl DatabasePool {
    /// Identifies the database this pool points at (for in-process state kept per database).
    pub fn instance_key(&self) -> String {
        dialect_match!(self, {
            pg(pool) => {
                let o = pool.connect_options();
                format!("pg:{}:{}/{}", o.get_host(), o.get_port(), o.get_database().unwrap_or_default())
            },
            sqlite(pool) => format!("sqlite:{}", pool.connect_options().get_filename().display()),
            mysql(pool) => {
                let o = pool.connect_options();
                format!("mysql:{}:{}/{}", o.get_host(), o.get_port(), o.get_database().unwrap_or_default())
            },
        })
    }

    pub async fn new(config: &Config) -> AppResult<Self> {
        let db = &config.database;
        // A DB_HOST beginning with '/' is treated as a Unix domain socket
        // directory rather than a TCP hostname (e.g. /var/run/postgresql).
        let host_is_socket = db.host.starts_with('/');

        match db.db_type.as_str() {
            "sqlite" => {
                // The migration runner creates the file; WAL lets readers run beside the one
                // writer, and the busy timeout queues writers instead of failing them.
                let options = sqlx::sqlite::SqliteConnectOptions::new()
                    .filename(&db.name)
                    .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
                    .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
                    .busy_timeout(Duration::from_secs(30))
                    .foreign_keys(true);
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(db.max_connections)
                    .min_connections(db.min_connections)
                    .acquire_timeout(Duration::from_secs(30))
                    .connect_with(options)
                    .await?;
                Ok(DatabasePool::Sqlite(pool))
            }
            "postgresql" => {
                let mut options = sqlx::postgres::PgConnectOptions::new()
                    .username(&db.username)
                    .password(&db.password)
                    .database(&db.name)
                    // Port is still relevant for sockets: PG names the socket
                    // file `.s.PGSQL.<port>`.
                    .port(db.port);
                options = if host_is_socket {
                    options.socket(&db.host)
                } else {
                    options.host(&db.host)
                };

                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(db.max_connections)
                    .min_connections(db.min_connections)
                    .acquire_timeout(Duration::from_secs(30))
                    .connect_with(options)
                    .await?;

                // Skip migrations for now - database already exists

                Ok(DatabasePool::Postgres(pool))
            }
            _ => {
                let mut options = sqlx::mysql::MySqlConnectOptions::new()
                    .username(&db.username)
                    .password(&db.password)
                    .database(&db.name)
                    .port(db.port);
                options = if host_is_socket {
                    options.socket(&db.host)
                } else {
                    options.host(&db.host)
                };

                let pool = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(db.max_connections)
                    .min_connections(db.min_connections)
                    .acquire_timeout(Duration::from_secs(30))
                    .connect_with(options)
                    .await?;

                // Skip migrations for now - database already exists

                Ok(DatabasePool::MySQL(pool))
            }
        }
    }

    pub async fn health_check(&self) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query("SELECT 1 as health")
                    .fetch_one(pool)
                    .await?;
                let health: i32 = row.try_get("health")?;
                Ok(health == 1)
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT 1 as health")
                    .fetch_one(pool)
                    .await?;
                let health: i32 = row.try_get("health")?;
                Ok(health == 1)
            },
        })
    }

    // Helper function to parse categories JSON string into HashMap - matches Python version
    fn parse_categories_json(&self, categories_str: &str) -> Option<std::collections::HashMap<String, String>> {
        if categories_str.is_empty() {
            return Some(std::collections::HashMap::new());
        }
        
        if categories_str.starts_with('{') {
            // Try to parse as JSON first
            if let Ok(parsed) = serde_json::from_str::<std::collections::HashMap<String, String>>(categories_str) {
                return Some(clean_categories(parsed));
            }
        } else {
            // Fall back to comma-separated parsing like Python version.
            // Skip empty segments so blank/whitespace categories aren't created.
            let mut result = std::collections::HashMap::new();
            for cat in categories_str.split(',') {
                let trimmed = cat.trim();
                if trimmed.is_empty() {
                    continue;
                }
                result.insert(result.len().to_string(), trimmed.to_string());
            }
            return Some(clean_categories(result));
        }
        
        // Return empty map if parsing fails
        Some(std::collections::HashMap::new())
    }
}
