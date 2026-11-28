use serde::{Deserialize, Serialize};
use std::env::{self, VarError};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;
use std::str::FromStr;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database: DatabaseConfig,
    /// Valkey/Redis; `None` keeps that state in memory (one server process only).
    pub redis: Option<RedisConfig>,
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub email: EmailConfig,
    pub oidc: OIDCConfig,
    pub api: ApiConfig,
}

/// The hosted search and podcast-people services, used unless SEARCH_API_URL / PEOPLE_API_URL
/// point elsewhere.
pub const DEFAULT_SEARCH_API_URL: &str = "https://search.pinepods.online/api/search";
pub const DEFAULT_PEOPLE_API_URL: &str = "https://people.pinepods.online";

/// An environment variable, treating blank as unset (the process supervisor passes unset
/// variables through as empty strings).
pub fn env_value(name: &str) -> Option<String> {
    env::var(name).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// VALKEY_<suffix>, else REDIS_<suffix>.
fn valkey_env(suffix: &str) -> Option<String> {
    env_value(&format!("VALKEY_{suffix}")).or_else(|| env_value(&format!("REDIS_{suffix}")))
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
    pub listen: Listen,
}

pub const DEFAULT_LISTEN: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 8032);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Listen {
    Tcp(SocketAddr),
    #[cfg(unix)]
    Unix(PathBuf),
    #[cfg(target_os = "linux")]
    Systemd,
}

impl FromStr for Listen {
    type Err = AppError;

    fn from_str(s: &str) -> AppResult<Self> {
        match s {
            "systemd" => cfg_select! {
                target_os = "linux" => Ok(Listen::Systemd),
                _ => Err(AppError::Config(format!(
                    "PINEPODS_LISTEN '{s}': systemd socket activation is not supported on this platform"
                ))),
            },
            _ if let Ok(addr) = s.parse::<SocketAddr>() => Ok(Listen::Tcp(addr)),
            _ if Path::new(s).is_absolute() => cfg_select! {
                unix => Ok(Listen::Unix(PathBuf::from(s))),
                _ => Err(AppError::Config(format!(
                    "PINEPODS_LISTEN '{s}': unix sockets are not supported on this platform"
                ))),
            },
            _ => Err(AppError::Config(format!(
                "Invalid PINEPODS_LISTEN '{s}': expected IP:PORT, [IPv6]:PORT, an absolute unix socket path, or systemd"
            ))),
        }
    }
}

fn parse_listen(value: Result<String, VarError>) -> AppResult<Listen> {
    match value {
        Err(VarError::NotPresent) => Ok(Listen::Tcp(DEFAULT_LISTEN)),
        Err(VarError::NotUnicode(_)) => Err(AppError::Config(
            "PINEPODS_LISTEN is not valid UTF-8".to_string(),
        )),
        Ok(s) => match s.trim() {
            "" => Ok(Listen::Tcp(DEFAULT_LISTEN)),
            trimmed => trimmed.parse(),
        },
    }
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

        // SQLite needs only DB_TYPE and the database file (DB_PATH). With no database server
        // configured at all, SQLite is the default.
        let db_type = env_value("DB_TYPE").or_else(|| env_value("DB_HOST").is_none().then(|| "sqlite".to_string()));
        let is_sqlite = db_type.as_deref().is_some_and(|t| t.eq_ignore_ascii_case("sqlite"));
        let db_required_vars: &[(&str, &str)] = if is_sqlite { &[] } else { &db_required_vars };

        let mut missing_db_vars = Vec::new();
        for (var_name, description) in db_required_vars {
            if env_value(var_name).is_none() {
                missing_db_vars.push(format!("  {} - {}", var_name, description));
            }
        }

        if !missing_db_vars.is_empty() {
            return Err(AppError::Config(format!(
                "Missing required database environment variables:\n{}\n\nPlease set these variables in your docker-compose.yml or environment.",
                missing_db_vars.join("\n")
            )));
        }

        let database = if is_sqlite {
            DatabaseConfig {
                db_type: "sqlite".to_string(),
                host: String::new(),
                port: 0,
                username: String::new(),
                password: String::new(),
                // For SQLite, `name` is the database file.
                name: env_value("DB_PATH").unwrap_or_else(|| "/opt/pinepods/data/pinepods.db".to_string()),
                max_connections: 8,
                min_connections: 1,
            }
        } else { DatabaseConfig {
            db_type: env_value("DB_TYPE").unwrap(),
            host: env_value("DB_HOST").unwrap(),
            port: {
                let port_str = env_value("DB_PORT").unwrap();
                port_str.trim().parse()
                    .map_err(|_e| AppError::Config(format!("Invalid DB_PORT '{}': Must be a valid port number (e.g., 5432 for PostgreSQL, 3306 for MariaDB)", port_str)))?
            },
            username: env_value("DB_USER").unwrap(),
            password: env_value("DB_PASSWORD").unwrap(),
            name: env_value("DB_NAME").unwrap(),
            max_connections: 32,
            min_connections: 1,
        } };

        let redis = Self::redis_from_env(is_sqlite)?;

        let server = ServerConfig {
            listen: parse_listen(env::var("PINEPODS_LISTEN"))?,
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
            search_api_url: env_value("SEARCH_API_URL").unwrap_or_else(|| DEFAULT_SEARCH_API_URL.to_string()),
            people_api_url: env_value("PEOPLE_API_URL").unwrap_or_else(|| DEFAULT_PEOPLE_API_URL.to_string()),
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

    /// Valkey/Redis settings, from VALKEY_* (or REDIS_*; VALKEY_* wins). `VALKEY_MODE` is
    /// `memory` or `valkey`; unset, a configured URL or host means Valkey, and otherwise SQLite
    /// installs keep this state in memory while the others require Valkey.
    fn redis_from_env(is_sqlite: bool) -> AppResult<Option<RedisConfig>> {
        let configured = valkey_env("URL").is_some() || valkey_env("HOST").is_some();
        let mode = valkey_env("MODE").map(|m| m.to_ascii_lowercase());
        match mode.as_deref() {
            Some("memory") => return Ok(None),
            Some("valkey") | Some("redis") => {}
            Some(other) => {
                return Err(AppError::Config(format!("Invalid VALKEY_MODE '{other}': use 'memory' or 'valkey'")));
            }
            None if !configured && is_sqlite => return Ok(None),
            None => {}
        }
        if !configured {
            return Err(AppError::Config(
                "Missing Valkey/Redis configuration. Set VALKEY_URL (e.g. redis://valkey:6379) or VALKEY_HOST \
                 (and VALKEY_PORT, default 6379), or VALKEY_MODE=memory to keep that state in memory \
                 (a single server process only)."
                    .to_string(),
            ));
        }

        let config = if let Some(url) = valkey_env("URL") {
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
            let host = valkey_env("HOST").expect("checked above");
            let port_str = valkey_env("PORT").unwrap_or_else(|| "6379".to_string());
            let port = port_str.trim().parse()
                .map_err(|_e| AppError::Config(format!("Invalid port '{}': Must be a valid port number (e.g., 6379)", port_str)))?;
            let password = valkey_env("PASSWORD");
            let username = valkey_env("USERNAME");
            let database = valkey_env("DATABASE").and_then(|d| d.parse().ok());

            RedisConfig {
                host,
                port,
                max_connections: 32,
                password,
                username,
                database,
            }
        };

        Ok(Some(config))
    }

    /// `None` without Valkey (the in-memory store).
    pub fn redis_url(&self) -> Option<String> {
        let redis = self.redis.as_ref()?;
        let mut url = String::from("redis://");
        
        // Add authentication if provided
        if let (Some(username), Some(password)) = (&redis.username, &redis.password) {
            url.push_str(&format!("{}:{}@", 
                urlencoding::encode(username), 
                urlencoding::encode(password)
            ));
        } else if let Some(password) = &redis.password {
            url.push_str(&format!(":{}@", urlencoding::encode(password)));
        }
        
        // Add host and port
        url.push_str(&format!("{}:{}", redis.host, redis.port));
        
        // Add database if specified
        if let Some(database) = redis.database {
            url.push_str(&format!("/{}", database));
        }
        
        Some(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;

    #[test]
    fn listen_accepts_ipv4() {
        assert_eq!(
            parse_listen(Ok("127.0.0.1:8032".to_string())).unwrap(),
            Listen::Tcp(SocketAddr::from((Ipv4Addr::LOCALHOST, 8032))),
        );
    }

    #[test]
    fn listen_accepts_ipv6() {
        assert_eq!(
            parse_listen(Ok("[::1]:8032".to_string())).unwrap(),
            Listen::Tcp(SocketAddr::from((Ipv6Addr::LOCALHOST, 8032))),
        );
    }

    #[cfg(unix)]
    #[test]
    fn listen_accepts_an_absolute_path() {
        assert_eq!(
            parse_listen(Ok("/run/pinepods/api.sock".to_string())).unwrap(),
            Listen::Unix(PathBuf::from("/run/pinepods/api.sock")),
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn listen_accepts_systemd() {
        assert_eq!(parse_listen(Ok("systemd".to_string())).unwrap(), Listen::Systemd);
    }

    #[test]
    fn listen_rejects_anything_else() {
        for value in ["localhost:8032", "8032", "0.0.0.0", "::1:8032", "api.sock"] {
            assert!(
                matches!(parse_listen(Ok(value.to_string())), Err(AppError::Config(_))),
                "{value}",
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn listen_rejects_non_unicode() {
        use std::os::unix::ffi::OsStringExt;
        let value = std::ffi::OsString::from_vec(vec![b'/', 0xff]);
        assert!(matches!(
            parse_listen(Err(VarError::NotUnicode(value))),
            Err(AppError::Config(_)),
        ));
    }

    #[test]
    fn listen_defaults_when_unset_or_empty() {
        for value in [Err(VarError::NotPresent), Ok(String::new()), Ok("  ".to_string())] {
            assert_eq!(parse_listen(value).unwrap(), Listen::Tcp(DEFAULT_LISTEN));
        }
    }
}
