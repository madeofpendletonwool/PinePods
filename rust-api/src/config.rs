use serde::{Deserialize, Serialize};
use std::env;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    pub redis: RedisConfig,
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub email: EmailConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub db_type: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub name: String,
    pub max_connections: u32,
    pub min_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub max_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub api_key_header: String,
    pub jwt_secret: String,
    pub password_salt_rounds: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_server: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub from_email: Option<String>,
}

impl Config {
    pub fn new() -> AppResult<Self> {
        // Load environment variables
        dotenvy::dotenv().ok();

        let database = DatabaseConfig {
            db_type: env::var("DB_TYPE").unwrap_or_else(|_| "mariadb".to_string()),
            host: env::var("DB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: env::var("DB_PORT")
                .unwrap_or_else(|_| "3306".to_string())
                .parse()
                .map_err(|_| AppError::Config("Invalid DB_PORT".to_string()))?,
            username: env::var("DB_USER").unwrap_or_else(|_| "root".to_string()),
            password: env::var("DB_PASSWORD").unwrap_or_else(|_| "password".to_string()),
            name: env::var("DB_NAME").unwrap_or_else(|_| "pypods_database".to_string()),
            max_connections: 32,
            min_connections: 1,
        };

        let redis = RedisConfig {
            host: env::var("VALKEY_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: env::var("VALKEY_PORT")
                .unwrap_or_else(|_| "6379".to_string())
                .parse()
                .unwrap_or(6379),
            max_connections: 32,
        };

        let server = ServerConfig {
            port: 8032, // Fixed port for internal API
            host: "0.0.0.0".to_string(),
        };

        let security = SecurityConfig {
            api_key_header: "pinepods_api".to_string(),
            jwt_secret: "pinepods-default-secret".to_string(),
            password_salt_rounds: 12,
        };

        let email = EmailConfig {
            smtp_server: env::var("SMTP_SERVER").ok(),
            smtp_port: env::var("SMTP_PORT").ok().and_then(|p| p.parse().ok()),
            smtp_username: env::var("SMTP_USERNAME").ok(),
            smtp_password: env::var("SMTP_PASSWORD").ok(),
            from_email: env::var("FROM_EMAIL").ok(),
        };

        Ok(Config {
            database,
            redis,
            server,
            security,
            email,
        })
    }

    pub fn database_url(&self) -> String {
        match self.database.db_type.as_str() {
            "postgresql" => format!(
                "postgresql://{}:{}@{}:{}/{}",
                self.database.username,
                self.database.password,
                self.database.host,
                self.database.port,
                self.database.name
            ),
            _ => format!(
                "mysql://{}:{}@{}:{}/{}",
                self.database.username,
                self.database.password,
                self.database.host,
                self.database.port,
                self.database.name
            ),
        }
    }

    pub fn redis_url(&self) -> String {
        format!("redis://{}:{}", self.redis.host, self.redis.port)
    }
}