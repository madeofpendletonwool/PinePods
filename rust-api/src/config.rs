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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub search_api_url: String,
    pub people_api_url: String,
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
    pub fn is_configured(&self) -> bool {
        self.provider_name.is_some() &&
        self.client_id.is_some() &&
        self.client_secret.is_some() &&
        self.authorization_url.is_some() &&
        self.token_url.is_some() &&
        self.user_info_url.is_some() &&
        self.button_text.is_some() &&
        self.scope.is_some() &&
        self.button_color.is_some() &&
        self.button_text_color.is_some()
    }

    pub fn validate(&self) -> Result<(), String> {
        let required_fields = [
            (&self.provider_name, "OIDC_PROVIDER_NAME"),
            (&self.client_id, "OIDC_CLIENT_ID"),
            (&self.client_secret, "OIDC_CLIENT_SECRET"),
            (&self.authorization_url, "OIDC_AUTHORIZATION_URL"),
            (&self.token_url, "OIDC_TOKEN_URL"),
            (&self.user_info_url, "OIDC_USER_INFO_URL"),
            (&self.button_text, "OIDC_BUTTON_TEXT"),
            (&self.scope, "OIDC_SCOPE"),
            (&self.button_color, "OIDC_BUTTON_COLOR"),
            (&self.button_text_color, "OIDC_BUTTON_TEXT_COLOR"),
        ];

        let missing_fields: Vec<&str> = required_fields
            .iter()
            .filter_map(|(field, name)| if field.is_none() { Some(*name) } else { None })
            .collect();

        // Check if any OIDC fields are set
        let any_oidc_set = required_fields.iter().any(|(field, _)| field.is_some());

        if any_oidc_set && !missing_fields.is_empty() {
            return Err(format!(
                "Incomplete OIDC configuration. When setting up OIDC, all required environment variables must be provided. Missing: {}",
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
                    .map_err(|e| AppError::Config(format!("Invalid DB_PORT '{}': Must be a valid port number (e.g., 5432 for PostgreSQL, 3306 for MariaDB)", port_str)))?
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
                .map_err(|e| AppError::Config(format!("Invalid port '{}': Must be a valid port number (e.g., 6379)", port_str)))?;
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

        let oidc = OIDCConfig {
            disable_standard_login: env::var("OIDC_DISABLE_STANDARD_LOGIN")
                .unwrap_or_else(|_| "false".to_string())
                .parse()
                .unwrap_or(false),
            provider_name: env::var("OIDC_PROVIDER_NAME").ok(),
            client_id: env::var("OIDC_CLIENT_ID").ok(),
            client_secret: env::var("OIDC_CLIENT_SECRET").ok(),
            authorization_url: env::var("OIDC_AUTHORIZATION_URL").ok(),
            token_url: env::var("OIDC_TOKEN_URL").ok(),
            user_info_url: env::var("OIDC_USER_INFO_URL").ok(),
            button_text: env::var("OIDC_BUTTON_TEXT").ok(),
            scope: env::var("OIDC_SCOPE").or_else(|_| {
                if env::var("OIDC_PROVIDER_NAME").is_ok() {
                    Ok("openid email profile".to_string())
                } else {
                    Err(env::VarError::NotPresent)
                }
            }).ok(),
            button_color: env::var("OIDC_BUTTON_COLOR").or_else(|_| {
                if env::var("OIDC_PROVIDER_NAME").is_ok() {
                    Ok("#000000".to_string())
                } else {
                    Err(env::VarError::NotPresent)
                }
            }).ok(),
            button_text_color: env::var("OIDC_BUTTON_TEXT_COLOR").or_else(|_| {
                if env::var("OIDC_PROVIDER_NAME").is_ok() {
                    Ok("#FFFFFF".to_string())
                } else {
                    Err(env::VarError::NotPresent)
                }
            }).ok(),
            icon_svg: env::var("OIDC_ICON_SVG").ok(),
            name_claim: env::var("OIDC_NAME_CLAIM").ok(),
            email_claim: env::var("OIDC_EMAIL_CLAIM").ok(),
            username_claim: env::var("OIDC_USERNAME_CLAIM").ok(),
            roles_claim: env::var("OIDC_ROLES_CLAIM").ok(),
            user_role: env::var("OIDC_USER_ROLE").ok(),
            admin_role: env::var("OIDC_ADMIN_ROLE").ok(),
        };

        let api = ApiConfig {
            search_api_url: env::var("SEARCH_API_URL").unwrap(),
            people_api_url: env::var("PEOPLE_API_URL").unwrap(),
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

    pub fn database_url(&self) -> String {
        // URL encode username and password to handle special characters
        let encoded_username = urlencoding::encode(&self.database.username);
        let encoded_password = urlencoding::encode(&self.database.password);
        
        let url = match self.database.db_type.as_str() {
            "postgresql" => format!(
                "postgresql://{}:{}@{}:{}/{}",
                encoded_username,
                encoded_password,
                self.database.host,
                self.database.port,
                self.database.name
            ),
            _ => format!(
                "mysql://{}:{}@{}:{}/{}",
                encoded_username,
                encoded_password,
                self.database.host,
                self.database.port,
                self.database.name
            ),
        };
        url
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