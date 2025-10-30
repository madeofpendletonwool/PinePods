// Common test utilities for integration tests
#![allow(dead_code)]

use std::sync::Arc;
use axum::{
    body::Body,
    http::{Request, Method, header},
    Router,
};
use tower::ServiceExt;
use http_body_util::BodyExt;

// Re-export pinepods-api types for tests
pub use pinepods_api::*;

/// Create a test configuration with sensible defaults
pub fn test_config() -> Config {
    use pinepods_api::config::{DatabaseConfig, RedisConfig, ServerConfig, SecurityConfig, EmailConfig, OIDCConfig, ApiConfig};

    Config {
        database: DatabaseConfig {
            db_type: std::env::var("TEST_DB_TYPE").unwrap_or_else(|_| "postgresql".to_string()),
            host: std::env::var("TEST_DB_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("TEST_DB_PORT")
                .unwrap_or_else(|_| "5432".to_string())
                .parse()
                .unwrap_or(5432),
            username: std::env::var("TEST_DB_USER").unwrap_or_else(|_| "test_user".to_string()),
            password: std::env::var("TEST_DB_PASSWORD").unwrap_or_else(|_| "test_password".to_string()),
            name: std::env::var("TEST_DB_NAME").unwrap_or_else(|_| "test_db".to_string()),
            max_connections: 5,
            min_connections: 1,
        },
        redis: RedisConfig {
            host: std::env::var("TEST_REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
            port: std::env::var("TEST_REDIS_PORT")
                .unwrap_or_else(|_| "6379".to_string())
                .parse()
                .unwrap_or(6379),
            max_connections: 5,
            password: std::env::var("TEST_REDIS_PASSWORD").ok(),
            username: std::env::var("TEST_REDIS_USERNAME").ok(),
            database: std::env::var("TEST_REDIS_DATABASE").ok().and_then(|d| d.parse().ok()),
        },
        server: ServerConfig {
            port: 8032,
            host: "127.0.0.1".to_string(),
        },
        security: SecurityConfig {
            api_key_header: "pinepods_api".to_string(),
            jwt_secret: "test-secret-key".to_string(),
            password_salt_rounds: 4, // Lower for faster tests
        },
        email: EmailConfig {
            smtp_server: None,
            smtp_port: None,
            smtp_username: None,
            smtp_password: None,
            from_email: None,
        },
        oidc: OIDCConfig {
            disable_standard_login: false,
            provider_name: None,
            client_id: None,
            client_secret: None,
            authorization_url: None,
            token_url: None,
            user_info_url: None,
            button_text: None,
            scope: None,
            button_color: None,
            button_text_color: None,
            icon_svg: None,
            name_claim: None,
            email_claim: None,
            username_claim: None,
            roles_claim: None,
            user_role: None,
            admin_role: None,
        },
        api: ApiConfig {
            search_api_url: "https://search.pinepods.online/api/search".to_string(),
            people_api_url: "https://people.pinepods.online".to_string(),
        },
    }
}

/// Helper to create HTTP requests
pub struct TestRequest {
    method: Method,
    uri: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
}

impl TestRequest {
    pub fn new(method: Method, uri: &str) -> Self {
        Self {
            method,
            uri: uri.to_string(),
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn get(uri: &str) -> Self {
        Self::new(Method::GET, uri)
    }

    pub fn post(uri: &str) -> Self {
        Self::new(Method::POST, uri)
    }

    pub fn put(uri: &str) -> Self {
        Self::new(Method::PUT, uri)
    }

    pub fn delete(uri: &str) -> Self {
        Self::new(Method::DELETE, uri)
    }

    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((key.into(), value.into()));
        self
    }

    pub fn api_key(self, key: &str) -> Self {
        self.header("Api-Key", key)
    }

    pub fn basic_auth(self, username: &str, password: &str) -> Self {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let credentials = format!("{}:{}", username, password);
        let encoded = STANDARD.encode(credentials.as_bytes());
        self.header("Authorization", format!("Basic {}", encoded))
    }

    pub fn json_body(mut self, body: impl serde::Serialize) -> Self {
        self.body = Some(serde_json::to_string(&body).unwrap());
        self.header("Content-Type", "application/json")
    }

    pub fn body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    pub fn build(self) -> Request<Body> {
        let mut request = Request::builder()
            .method(self.method)
            .uri(self.uri);

        for (key, value) in self.headers {
            request = request.header(key, value);
        }

        let body = self.body.unwrap_or_default();
        request.body(Body::from(body)).unwrap()
    }

    pub async fn send(self, app: &Router) -> TestResponse {
        let request = self.build();
        let response = app.clone().oneshot(request).await.unwrap();

        let status = response.status();
        let headers = response.headers().clone();
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body_text = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();

        TestResponse {
            status,
            headers,
            body: body_text,
        }
    }
}

/// Helper for response assertions
pub struct TestResponse {
    pub status: axum::http::StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: String,
}

impl TestResponse {
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> T {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("Failed to parse JSON response: {}. Body: {}", e, self.body))
    }

    pub fn assert_status(&self, expected: axum::http::StatusCode) {
        assert_eq!(
            self.status, expected,
            "Expected status {}, got {}. Body: {}",
            expected, self.status, self.body
        );
    }

    pub fn assert_ok(&self) {
        self.assert_status(axum::http::StatusCode::OK);
    }

    pub fn assert_unauthorized(&self) {
        self.assert_status(axum::http::StatusCode::UNAUTHORIZED);
    }

    pub fn assert_forbidden(&self) {
        self.assert_status(axum::http::StatusCode::FORBIDDEN);
    }

    pub fn assert_not_found(&self) {
        self.assert_status(axum::http::StatusCode::NOT_FOUND);
    }

    pub fn assert_bad_request(&self) {
        self.assert_status(axum::http::StatusCode::BAD_REQUEST);
    }
}

/// Generate a random test username
pub fn random_username() -> String {
    use fake::{Fake, faker::internet::en::Username};
    Username().fake()
}

/// Generate a random test email
pub fn random_email() -> String {
    use fake::{Fake, faker::internet::en::SafeEmail};
    SafeEmail().fake()
}

/// Generate a random test password
pub fn random_password() -> String {
    use fake::{Fake, faker::lorem::en::Word};
    let word: String = Word().fake();
    format!("{}123!Test", word)
}

/// Public endpoints that don't require authentication
pub fn public_endpoints() -> Vec<(&'static str, Method)> {
    vec![
        ("/api/pinepods_check", Method::GET),
        ("/api/health", Method::GET),
        ("/api/data/self_service_status", Method::GET),
        ("/api/data/public_oidc_providers", Method::GET),
        ("/api/data/create_first", Method::POST),
        ("/api/auth/store_state", Method::POST),
        ("/api/auth/callback", Method::GET),
        ("/api/data/reset_password_create_code", Method::POST),
        ("/api/data/verify_and_reset_password", Method::POST),
    ]
}

/// All authenticated endpoints that should require an API key
/// This is a comprehensive list extracted from main.rs route definitions
pub fn authenticated_endpoints() -> Vec<(&'static str, Method)> {
    vec![
        // Auth endpoints that require API key
        ("/api/data/verify_key", Method::GET),
        ("/api/data/get_user", Method::GET),
        ("/api/data/user_details_id/1", Method::GET),
        ("/api/data/config", Method::GET),
        ("/api/data/first_login_done/1", Method::GET),
        ("/api/data/get_theme/1", Method::GET),
        ("/api/data/setup_time_info", Method::POST),
        ("/api/data/update_timezone", Method::PUT),
        ("/api/data/update_date_format", Method::PUT),
        ("/api/data/update_time_format", Method::PUT),
        ("/api/data/get_auto_complete_seconds/1", Method::GET),
        ("/api/data/update_auto_complete_seconds", Method::PUT),
        ("/api/data/user_admin_check/1", Method::GET),
        ("/api/data/import_opml", Method::POST),
        ("/api/data/import_progress/1", Method::GET),

        // Podcast endpoints
        ("/api/data/return_episodes/1", Method::GET),
        ("/api/data/user_history/1", Method::GET),
        ("/api/data/increment_listen_time/1", Method::PUT),
        ("/api/data/get_playback_speed", Method::POST),
        ("/api/data/add_podcast", Method::POST),
        ("/api/data/update_podcast_info", Method::PUT),
        ("/api/data/1/merge", Method::POST),
        ("/api/data/1/unmerge/2", Method::POST),
        ("/api/data/1/merged", Method::GET),
        ("/api/data/remove_podcast", Method::POST),
        ("/api/data/remove_podcast_id", Method::POST),
        ("/api/data/remove_podcast_name", Method::POST),
        ("/api/data/return_pods/1", Method::GET),
        ("/api/data/return_pods_extra/1", Method::GET),
        ("/api/data/get_time_info", Method::GET),
        ("/api/data/check_podcast", Method::GET),
        ("/api/data/check_episode_in_db/1", Method::GET),
        ("/api/data/queue_pod", Method::POST),
        ("/api/data/remove_queued_pod", Method::POST),
        ("/api/data/get_queued_episodes", Method::GET),
        ("/api/data/reorder_queue", Method::POST),
        ("/api/data/save_episode", Method::POST),
        ("/api/data/remove_saved_episode", Method::POST),
        ("/api/data/saved_episode_list/1", Method::GET),
        ("/api/data/record_podcast_history", Method::POST),
        ("/api/data/get_podcast_id", Method::GET),
        ("/api/data/download_episode_list", Method::GET),
        ("/api/data/download_podcast", Method::POST),
        ("/api/data/delete_episode", Method::POST),
        ("/api/data/download_all_podcast", Method::POST),
        ("/api/data/download_status/1", Method::GET),
        ("/api/data/podcast_episodes", Method::GET),
        ("/api/data/get_podcast_id_from_ep_name", Method::GET),
        ("/api/data/get_episode_id_ep_name", Method::GET),
        ("/api/data/get_episode_metadata", Method::POST),
        ("/api/data/fetch_podcasting_2_data", Method::GET),
        ("/api/data/get_auto_download_status", Method::POST),
        ("/api/data/get_feed_cutoff_days", Method::GET),
        ("/api/data/get_play_episode_details", Method::POST),
        ("/api/data/fetch_podcasting_2_pod_data", Method::GET),
        ("/api/data/mark_episode_completed", Method::POST),
        ("/api/data/update_episode_duration", Method::POST),

        // Bulk operations
        ("/api/data/bulk_mark_episodes_completed", Method::POST),
        ("/api/data/bulk_save_episodes", Method::POST),
        ("/api/data/bulk_queue_episodes", Method::POST),
        ("/api/data/bulk_download_episodes", Method::POST),
        ("/api/data/bulk_delete_downloaded_episodes", Method::POST),
        ("/api/data/share_episode/1", Method::POST),
        ("/api/data/episode_by_url/testkey", Method::GET),
        ("/api/data/increment_played/1", Method::PUT),
        ("/api/data/record_listen_duration", Method::POST),
        ("/api/data/get_podcast_id_from_ep_id", Method::GET),
        ("/api/data/get_stats", Method::GET),
        ("/api/data/get_pinepods_version", Method::GET),
        ("/api/data/search_data", Method::POST),
        ("/api/data/fetch_transcript", Method::POST),
        ("/api/data/home_overview", Method::GET),
        ("/api/data/get_playlists", Method::GET),
        ("/api/data/get_playlist_episodes", Method::GET),
        ("/api/data/create_playlist", Method::POST),
        ("/api/data/delete_playlist", Method::DELETE),
        ("/api/data/get_podcast_details", Method::GET),
        ("/api/data/get_podcast_details_dynamic", Method::GET),
        ("/api/data/podpeople/host_podcasts", Method::GET),
        ("/api/data/update_feed_cutoff_days", Method::POST),
        ("/api/data/fetch_podcast_feed", Method::GET),
        ("/api/data/youtube_episodes", Method::GET),
        ("/api/data/remove_youtube_channel", Method::POST),
        ("/api/data/stream/1", Method::GET),
        ("/api/data/get_rss_key", Method::GET),
        ("/api/data/mark_episode_uncompleted", Method::POST),

        // Settings endpoints
        ("/api/data/user/set_theme", Method::PUT),
        ("/api/data/get_user_info", Method::GET),
        ("/api/data/my_user_info/1", Method::GET),
        ("/api/data/add_user", Method::POST),
        ("/api/data/add_login_user", Method::POST),
        ("/api/data/set_fullname/1", Method::PUT),
        ("/api/data/set_password/1", Method::PUT),
        ("/api/data/user/delete/1", Method::DELETE),
        ("/api/data/user/set_email", Method::PUT),
        ("/api/data/user/set_username", Method::PUT),
        ("/api/data/user/set_isadmin", Method::PUT),
        ("/api/data/user/final_admin/1", Method::GET),
        ("/api/data/enable_disable_guest", Method::POST),
        ("/api/data/enable_disable_downloads", Method::POST),
        ("/api/data/enable_disable_self_service", Method::POST),
        ("/api/data/guest_status", Method::GET),
        ("/api/data/rss_feed_status", Method::GET),
        ("/api/data/toggle_rss_feeds", Method::POST),
        ("/api/data/download_status", Method::GET),
        ("/api/data/admin_self_service_status", Method::GET),
        ("/api/data/save_email_settings", Method::POST),
        ("/api/data/get_email_settings", Method::GET),
        ("/api/data/send_test_email", Method::POST),
        ("/api/data/send_email", Method::POST),
        ("/api/data/get_api_info/1", Method::GET),
        ("/api/data/create_api_key", Method::POST),
        ("/api/data/delete_api_key", Method::DELETE),
        ("/api/data/backup_user", Method::POST),
        ("/api/data/backup_server", Method::POST),
        ("/api/data/restore_server", Method::POST),
        ("/api/data/generate_mfa_secret/1", Method::GET),
        ("/api/data/verify_temp_mfa", Method::POST),
        ("/api/data/check_mfa_enabled/1", Method::GET),
        ("/api/data/save_mfa_secret", Method::POST),
        ("/api/data/delete_mfa", Method::DELETE),
        ("/api/data/initiate_nextcloud_login", Method::POST),
        ("/api/data/add_nextcloud_server", Method::POST),
        ("/api/data/verify_gpodder_auth", Method::POST),
        ("/api/data/add_gpodder_server", Method::POST),
        ("/api/data/get_gpodder_settings/1", Method::GET),
        ("/api/data/check_gpodder_settings/1", Method::GET),
        ("/api/data/remove_podcast_sync", Method::DELETE),
        ("/api/data/gpodder/status", Method::GET),
        ("/api/data/gpodder/toggle", Method::POST),
        ("/api/data/refresh_pods", Method::GET),
        ("/api/data/refresh_gpodder_subscriptions", Method::GET),
        ("/api/data/refresh_nextcloud_subscriptions", Method::GET),
        ("/api/data/refresh_hosts", Method::GET),
        ("/api/data/cleanup_tasks", Method::GET),
        ("/api/data/auto_complete_episodes", Method::GET),
        ("/api/data/update_playlists", Method::GET),
        ("/api/data/add_custom_podcast", Method::POST),
        ("/api/data/user/notification_settings", Method::GET),
        ("/api/data/user/notification_settings", Method::PUT),
        ("/api/data/user/set_playback_speed", Method::POST),
        ("/api/data/user/set_global_podcast_cover_preference", Method::POST),
        ("/api/data/user/get_podcast_cover_preference", Method::GET),
        ("/api/data/user/test_notification", Method::POST),
        ("/api/data/add_oidc_provider", Method::POST),
        ("/api/data/update_oidc_provider/1", Method::PUT),
        ("/api/data/list_oidc_providers", Method::GET),
        ("/api/data/remove_oidc_provider", Method::POST),
        ("/api/data/startpage", Method::GET),
        ("/api/data/startpage", Method::POST),
        ("/api/data/person/subscribe/1/1", Method::POST),
        ("/api/data/person/unsubscribe/1/1", Method::DELETE),
        ("/api/data/person/subscriptions/1", Method::GET),
        ("/api/data/person/episodes/1/1", Method::GET),
        ("/api/data/search_youtube_channels", Method::GET),
        ("/api/data/youtube/subscribe", Method::POST),
        ("/api/data/check_youtube_channel", Method::GET),
        ("/api/data/enable_auto_download", Method::POST),
        ("/api/data/adjust_skip_times", Method::POST),
        ("/api/data/remove_category", Method::POST),
        ("/api/data/add_category", Method::POST),
        ("/api/data/podcast/set_playback_speed", Method::POST),
        ("/api/data/podcast/set_cover_preference", Method::POST),
        ("/api/data/podcast/clear_cover_preference", Method::POST),
        ("/api/data/podcast/toggle_notifications", Method::PUT),
        ("/api/data/podcast/notification_status", Method::POST),
        ("/api/data/rss_key", Method::GET),
        ("/api/data/verify_mfa", Method::POST),
        ("/api/data/schedule_backup", Method::POST),
        ("/api/data/get_scheduled_backup", Method::POST),
        ("/api/data/list_backup_files", Method::POST),
        ("/api/data/restore_backup_file", Method::POST),
        ("/api/data/manual_backup_to_directory", Method::POST),
        ("/api/data/get_unmatched_podcasts", Method::POST),
        ("/api/data/update_podcast_index_id", Method::POST),
        ("/api/data/ignore_podcast_index_id", Method::POST),
        ("/api/data/get_ignored_podcasts", Method::POST),
        ("/api/data/get_user_language", Method::GET),
        ("/api/data/update_user_language", Method::PUT),
        ("/api/data/get_available_languages", Method::GET),
        ("/api/data/get_server_default_language", Method::GET),

        // Podcast routes under /api/podcasts
        ("/api/podcasts/notification_status", Method::POST),

        // Episode routes under /api/episodes
        ("/api/episodes/1/download", Method::GET),

        // Task routes
        ("/api/tasks/user/1", Method::GET),
        ("/api/tasks/active", Method::GET),
        ("/api/tasks/1", Method::GET),

        // Proxy routes
        ("/api/proxy/image", Method::GET),

        // Gpodder routes
        ("/api/gpodder/test-connection", Method::GET),
        ("/api/gpodder/set_default/1", Method::POST),
        ("/api/gpodder/devices/1", Method::GET),
        ("/api/gpodder/devices", Method::GET),
        ("/api/gpodder/devices", Method::POST),
        ("/api/gpodder/default_device", Method::GET),
        ("/api/gpodder/sync/force", Method::POST),
        ("/api/gpodder/sync", Method::POST),
        ("/api/gpodder/gpodder_statistics", Method::GET),

        // Init/startup routes
        ("/api/init/startup_tasks", Method::POST),

        // Feed routes
        ("/api/feed/1", Method::GET),
    ]
}
