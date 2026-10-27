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
    pub oidc: OIDCConfig,
    pub api: ApiConfig,
}

pub const YOUTUBE_FEED_URL: &str = "https://www.youtube.com/feeds/videos.xml?channel_id={channel_id}";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub search_api_url: String,
    pub people_api_url: String,
    /// A YouTube channel's RSS feed, with `{channel_id}` in place of the channel.
    pub youtube_feed_url: String,
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
    pub password: Option<String>,
    pub username: Option<String>,
    pub database: Option<u8>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OIDCConfig {
    pub disable_standard_login: bool,
    pub provider_name: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub authorization_url: Option<String>,
    pub token_url: Option<String>,
    pub user_info_url: Option<String>,
    // Issuer or .well-known URL; fills in whichever endpoint URLs above are unset.
    pub discovery_url: Option<String>,
    pub button_text: Option<String>,
    pub scope: Option<String>,
    pub button_color: Option<String>,
    pub button_text_color: Option<String>,
    pub icon_svg: Option<String>,
    pub name_claim: Option<String>,
    pub email_claim: Option<String>,
    pub username_claim: Option<String>,
    pub roles_claim: Option<String>,
    pub user_role: Option<String>,
    pub admin_role: Option<String>,
}

impl OIDCConfig {
    /// Endpoint URLs are either all given explicitly or resolved from discovery at startup.
    fn endpoints_available(&self) -> bool {
        self.discovery_url.is_some()
            || (self.authorization_url.is_some() && self.token_url.is_some() && self.user_info_url.is_some())
    }

    pub fn is_configured(&self) -> bool {
        self.provider_name.as_ref().map_or(false, |s| !s.trim().is_empty()) &&
        self.client_id.as_ref().map_or(false, |s| !s.trim().is_empty()) &&
        self.client_secret.as_ref().map_or(false, |s| !s.trim().is_empty()) &&
        self.endpoints_available() &&
        self.button_text.as_ref().map_or(false, |s| !s.trim().is_empty()) &&
        self.scope.as_ref().map_or(false, |s| !s.trim().is_empty()) &&
        self.button_color.as_ref().map_or(false, |s| !s.trim().is_empty()) &&
        self.button_text_color.as_ref().map_or(false, |s| !s.trim().is_empty())
    }

    pub fn validate(&self) -> Result<(), String> {
        let mut required_fields = vec![
            (&self.provider_name, "OIDC_PROVIDER_NAME"),
            (&self.client_id, "OIDC_CLIENT_ID"),
            (&self.client_secret, "OIDC_CLIENT_SECRET"),
            (&self.button_text, "OIDC_BUTTON_TEXT"),
            (&self.scope, "OIDC_SCOPE"),
            (&self.button_color, "OIDC_BUTTON_COLOR"),
            (&self.button_text_color, "OIDC_BUTTON_TEXT_COLOR"),
        ];
        if self.discovery_url.is_none() {
            required_fields.extend([
                (&self.authorization_url, "OIDC_AUTHORIZATION_URL"),
                (&self.token_url, "OIDC_TOKEN_URL"),
                (&self.user_info_url, "OIDC_USER_INFO_URL"),
            ]);
        }

        let missing_fields: Vec<&str> = required_fields
            .iter()
            .filter_map(|(field, name)| if field.is_none() { Some(*name) } else { None })
            .collect();

        // Check if any OIDC fields are set
        let any_oidc_set = self.discovery_url.is_some()
            || required_fields.iter().any(|(field, _)| field.is_some());

        if any_oidc_set && !missing_fields.is_empty() {
            return Err(format!(
                "Incomplete OIDC configuration. When setting up OIDC, all required environment variables must be provided (the endpoint URLs can be replaced by OIDC_DISCOVERY_URL). Missing: {}",
                missing_fields.join(", ")
            ));
        }

        if self.disable_standard_login && !self.is_configured() {
            return Err("OIDC_DISABLE_STANDARD_LOGIN is set to true, but OIDC is not properly configured. All OIDC environment variables must be set when disabling standard login.".to_string());
        }

        Ok(())
    }
}

impl Config {
    pub fn new() -> AppResult<Self> {
        // Load environment variables
        dotenvy::dotenv().ok();
        
        // Validate required database environment variables
        let db_required_vars = [
            ("DB_TYPE", "Database type (e.g., postgresql, mariadb)"),
            ("DB_HOST", "Database host (e.g., localhost, db)"),
            ("DB_PORT", "Database port (e.g., 5432 for PostgreSQL, 3306 for MariaDB)"),
            ("DB_USER", "Database username"),
            ("DB_PASSWORD", "Database password"),
            ("DB_NAME", "Database name"),
        ];

        let mut missing_db_vars = Vec::new();
        for (var_name, description) in &db_required_vars {
            if env::var(var_name).is_err() {
                missing_db_vars.push(format!("  {} - {}", var_name, description));
            }
        }

        if !missing_db_vars.is_empty() {
            return Err(AppError::Config(format!(
                "Missing required database environment variables:\n{}\n\nPlease set these variables in your docker-compose.yml or environment.",
                missing_db_vars.join("\n")
            )));
        }

        // Validate required API URLs
        let api_required_vars = [
            ("SEARCH_API_URL", "Search API URL (e.g., https://search.pinepods.online/api/search)"),
            ("PEOPLE_API_URL", "People API URL (e.g., https://people.pinepods.online)"),
        ];

        let mut missing_api_vars = Vec::new();
        for (var_name, description) in &api_required_vars {
            if env::var(var_name).is_err() {
                missing_api_vars.push(format!("  {} - {}", var_name, description));
            }
        }

        if !missing_api_vars.is_empty() {
            return Err(AppError::Config(format!(
                "Missing required API environment variables:\n{}\n\nPlease set these variables in your docker-compose.yml or environment.",
                missing_api_vars.join("\n")
            )));
        }

        // Validate Valkey/Redis configuration - either URL or individual variables (support both VALKEY_* and REDIS_* naming)
        let has_valkey_url = env::var("VALKEY_URL").is_ok();
        let has_redis_url = env::var("REDIS_URL").is_ok();
        let has_valkey_vars = env::var("VALKEY_HOST").is_ok() && env::var("VALKEY_PORT").is_ok();
        let has_redis_vars = env::var("REDIS_HOST").is_ok() && env::var("REDIS_PORT").is_ok();

        if !has_valkey_url && !has_redis_url && !has_valkey_vars && !has_redis_vars {
            return Err(AppError::Config(format!(
                "Missing required Valkey/Redis configuration. Please provide either:\n  Option 1: VALKEY_URL or REDIS_URL - Complete connection URL\n  Option 2: VALKEY_HOST/VALKEY_PORT or REDIS_HOST/REDIS_PORT - Individual connection parameters\n\nExample URL: VALKEY_URL=redis://localhost:6379\nExample individual: VALKEY_HOST=localhost, VALKEY_PORT=6379"
            )));
        }

        let database = DatabaseConfig {
            db_type: env::var("DB_TYPE").unwrap(),
            host: env::var("DB_HOST").unwrap(),
            port: {
                let port_str = env::var("DB_PORT").unwrap();
                port_str.trim().parse()
                    .map_err(|_e| AppError::Config(format!("Invalid DB_PORT '{}': Must be a valid port number (e.g., 5432 for PostgreSQL, 3306 for MariaDB)", port_str)))?
            },
            username: env::var("DB_USER").unwrap(),
            password: env::var("DB_PASSWORD").unwrap(),
            name: env::var("DB_NAME").unwrap(),
            max_connections: 32,
            min_connections: 1,
        };

        let redis = if let Some(url) = env::var("VALKEY_URL").ok().or_else(|| env::var("REDIS_URL").ok()) {
            // Parse VALKEY_URL or REDIS_URL
            match url::Url::parse(&url) {
                Ok(parsed_url) => {
                    let host = parsed_url.host_str().unwrap_or("localhost").to_string();
                    let port = parsed_url.port().unwrap_or(6379);
                    let username = if parsed_url.username().is_empty() { 
                        None 
                    } else { 
                        Some(parsed_url.username().to_string()) 
                    };
                    let password = parsed_url.password().map(|p| p.to_string());
                    let database = if parsed_url.path().len() > 1 {
                        parsed_url.path().trim_start_matches('/').parse().ok()
                    } else {
                        None
                    };

                    RedisConfig {
                        host,
                        port,
                        max_connections: 32,
                        password,
                        username,
                        database,
                    }
                }
                Err(e) => {
                    return Err(AppError::Config(format!("Invalid URL format: {}", e)));
                }
            }
        } else {
            // Use individual variables - support both VALKEY_* and REDIS_* (VALKEY_* takes precedence)
            let host = env::var("VALKEY_HOST").or_else(|_| env::var("REDIS_HOST")).unwrap();
            let port_str = env::var("VALKEY_PORT").or_else(|_| env::var("REDIS_PORT")).unwrap();
            let port = port_str.trim().parse()
                .map_err(|_e| AppError::Config(format!("Invalid port '{}': Must be a valid port number (e.g., 6379)", port_str)))?;
            let password = env::var("VALKEY_PASSWORD").ok().or_else(|| env::var("REDIS_PASSWORD").ok());
            let username = env::var("VALKEY_USERNAME").ok().or_else(|| env::var("REDIS_USERNAME").ok());
            let database = env::var("VALKEY_DATABASE").ok()
                .or_else(|| env::var("REDIS_DATABASE").ok())
                .and_then(|d| d.parse().ok());

            RedisConfig {
                host,
                port,
                max_connections: 32,
                password,
                username,
                database,
            }
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

        // startup.sh exports every OIDC variable, so an unset one arrives as "" and must count as unset.
        let oidc_env = |name: &str| env::var(name).ok().filter(|s| !s.trim().is_empty());

        // Check if essential OIDC fields are present before setting any defaults
        let oidc_essentials_present = oidc_env("OIDC_PROVIDER_NAME").is_some() &&
            oidc_env("OIDC_CLIENT_ID").is_some() &&
            oidc_env("OIDC_CLIENT_SECRET").is_some() &&
            (oidc_env("OIDC_DISCOVERY_URL").is_some() || (
                oidc_env("OIDC_AUTHORIZATION_URL").is_some() &&
                oidc_env("OIDC_TOKEN_URL").is_some() &&
                oidc_env("OIDC_USER_INFO_URL").is_some()
            ));
        let oidc_env_or = |name: &str, default: &str| {
            oidc_env(name).or_else(|| oidc_essentials_present.then(|| default.to_string()))
        };

        let oidc = OIDCConfig {
            disable_standard_login: oidc_env("OIDC_DISABLE_STANDARD_LOGIN")
                .map_or(false, |v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes")),
            provider_name: oidc_env("OIDC_PROVIDER_NAME"),
            client_id: oidc_env("OIDC_CLIENT_ID"),
            client_secret: oidc_env("OIDC_CLIENT_SECRET"),
            authorization_url: oidc_env("OIDC_AUTHORIZATION_URL"),
            token_url: oidc_env("OIDC_TOKEN_URL"),
            user_info_url: oidc_env("OIDC_USER_INFO_URL"),
            discovery_url: oidc_env("OIDC_DISCOVERY_URL"),
            button_text: oidc_env_or("OIDC_BUTTON_TEXT", "Login with OIDC"),
            scope: oidc_env_or("OIDC_SCOPE", "openid email profile"),
            button_color: oidc_env_or("OIDC_BUTTON_COLOR", "#000000"),
            button_text_color: oidc_env_or("OIDC_BUTTON_TEXT_COLOR", "#FFFFFF"),
            icon_svg: oidc_env("OIDC_ICON_SVG"),
            name_claim: oidc_env("OIDC_NAME_CLAIM"),
            email_claim: oidc_env("OIDC_EMAIL_CLAIM"),
            username_claim: oidc_env("OIDC_USERNAME_CLAIM"),
            roles_claim: oidc_env("OIDC_ROLES_CLAIM"),
            user_role: oidc_env("OIDC_USER_ROLE"),
            admin_role: oidc_env("OIDC_ADMIN_ROLE"),
        };

        let api = ApiConfig {
            search_api_url: env::var("SEARCH_API_URL").unwrap(),
            people_api_url: env::var("PEOPLE_API_URL").unwrap(),
            youtube_feed_url: YOUTUBE_FEED_URL.to_string(),
        };

        // Validate OIDC configuration
        if let Err(validation_error) = oidc.validate() {
            return Err(AppError::Config(validation_error));
        }

        Ok(Config {
            database,
            redis,
            server,
            security,
            email,
            oidc,
            api,
        })
    }

    pub fn redis_url(&self) -> String {
        let mut url = String::from("redis://");
        
        // Add authentication if provided
        if let (Some(username), Some(password)) = (&self.redis.username, &self.redis.password) {
            url.push_str(&format!("{}:{}@", 
                urlencoding::encode(username), 
                urlencoding::encode(password)
            ));
        } else if let Some(password) = &self.redis.password {
            url.push_str(&format!(":{}@", urlencoding::encode(password)));
        }
        
        // Add host and port
        url.push_str(&format!("{}:{}", self.redis.host, self.redis.port));
        
        // Add database if specified
        if let Some(database) = self.redis.database {
            url.push_str(&format!("/{}", database));
        }
        
        url
    }
}