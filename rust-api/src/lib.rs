//! PinePods API library crate.
//!
//! Everything the server is made of lives here so the binary (`main.rs`) stays a thin
//! startup shim and integration tests (`tests/`) can build the exact same router via
//! [`create_app`] against a real database.

use axum::{
    routing::{get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    trace::TraceLayer,
    compression::{
        predicate::{DefaultPredicate, NotForContentType, Predicate},
        CompressionLayer,
    },
    services::ServeDir,
};

pub mod config;
pub mod database;
pub mod error;
pub mod gpodder;
pub mod handlers;
pub mod models;
pub mod openapi;
pub mod redis_client;
pub mod redis_manager;
pub mod services;

use utoipa::OpenApi;
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_scalar::{Scalar, Servable};

use config::Config;
use database::DatabasePool;
use redis_client::RedisClient;
use services::{task_manager::TaskManager, tasks::TaskSpawner};
use handlers::websocket::WebSocketManager;
use redis_manager::{ImportProgressManager, NotificationManager};
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db_pool: DatabasePool,
    pub redis_client: RedisClient,
    pub config: Config,
    pub task_manager: Arc<TaskManager>,
    pub task_spawner: Arc<TaskSpawner>,
    pub websocket_manager: Arc<WebSocketManager>,
    pub now_playing_manager: Arc<handlers::nowplaying::NowPlayingManager>,
    /// Unique id for this API process, used by the now-playing pub/sub bridge to
    /// ignore the messages it published itself (it already handled them locally).
    pub instance_id: Arc<str>,
    pub import_progress_manager: Arc<ImportProgressManager>,
    pub notification_manager: Arc<NotificationManager>,
    /// Set while a full server restore is running. Used to reject concurrent restores
    /// and to block first-admin creation (which would otherwise race the restore and
    /// corrupt it). Shared across clones via Arc.
    pub restore_in_progress: Arc<std::sync::atomic::AtomicBool>,
    /// Whether the optional pinepods-ai sidecar is reachable. Kept fresh by a
    /// background health monitor; handlers gate AI features on this.
    pub ai_available: crate::services::ai_client::AiAvailability,
    /// Whether episode ids of merged copies exist, for following them in requests.
    pub merged_episodes: crate::services::episode_aliases::MergedEpisodes,
}

/// Builds the spec-aware router. Handlers annotated with `#[utoipa::path]` and
/// registered via `routes!` are collected into the OpenAPI document; route groups
/// nested as plain `Router`s still serve but are not yet in the spec. Migrate a group
/// by converting its `create_*_routes()` to an `OpenApiRouter` and using `routes!`.
pub fn openapi_router() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(openapi::ApiDoc::openapi())
        // Health check endpoints (annotated — appear in the spec).
        // One routes!() call per distinct path; routes!() groups methods that share a path.
        .routes(routes!(handlers::health::pinepods_check))
        .routes(routes!(handlers::health::health_check))
        // Partially-migrated groups (annotated handlers appear in the spec; the rest serve as plain routes)
        .nest("/api/data", create_data_routes())
        .nest("/api/episodes", create_episode_routes())
        // Not-yet-migrated groups (served as plain routers; not in the spec)
        .nest("/api/init", create_init_routes())
        .nest("/api/podcasts", OpenApiRouter::from(create_podcast_routes()))
        .nest("/api/playlists", OpenApiRouter::from(create_playlist_routes()))
        .nest("/api/tasks", create_task_routes())
        .nest("/api/async", OpenApiRouter::from(create_async_routes()))
        .nest("/api/proxy", create_proxy_routes())
        .nest("/api/gpodder", create_gpodder_routes())
        .nest("/api/sync", create_sync_routes())
        .nest("/api/feed", create_feed_routes())
        .nest("/api/auth", create_auth_routes())
        .nest("/ws", OpenApiRouter::from(create_websocket_routes()))
}

pub fn create_app(state: AppState) -> Router {
    let (router, api) = openapi_router().split_for_parts();
    let spec = api.clone();

    router
        // Routes/services that intentionally stay out of the API spec
        .route("/api/placeholder/{width}/{height}", get(handlers::proxy::placeholder_image))
        .merge(gpodder::router())
        .nest_service("/api/local-media", ServeDir::new("/opt/pinepods/local-media"))
        // Raw spec for tooling/docs builds, plus the interactive Scalar UI at /api/docs
        .route("/api/openapi.json", get(move || {
            let spec = spec.clone();
            async move { axum::Json(spec) }
        }))
        .merge(Scalar::with_url("/api/docs", api))
        // Episode ids of merged copies point at the episode they became.
        .layer(axum::middleware::from_fn_with_state(state.clone(), services::episode_aliases::follow_merged_episodes))
        // Middleware stack
        .layer(
            ServiceBuilder::new()
                .layer(
                    TraceLayer::new_for_http()
                        .make_span_with(|req: &axum::http::Request<_>| {
                            tracing::info_span!("request", method = %req.method(), uri = %redacted_uri(req.uri()), version = ?req.version())
                        })
                        .on_response(tower_http::trace::DefaultOnResponse::new()
                            .level(tracing::Level::INFO))
                )
                // Media is already compressed, and gzipping a full-file response
                // also strips Accept-Ranges, which players use to decide whether
                // they can seek.
                .layer(CompressionLayer::new().compress_when(
                    DefaultPredicate::new()
                        .and(NotForContentType::const_new("audio/"))
                        .and(NotForContentType::const_new("video/")),
                ))
                .layer(axum::extract::DefaultBodyLimit::max(2 * 1024 * 1024 * 1024)) // 2GB limit for massive backup files
        )
        .with_state(state)
}

/// The request URI for logs, with the values of credential-like query parameters (websockets
/// pass `api_key` in the query) replaced.
fn redacted_uri(uri: &axum::http::Uri) -> String {
    let Some(query) = uri.query() else { return uri.path().to_string() };
    let query = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((name, _)) if ["key", "token", "password", "secret"].iter().any(|s| name.to_ascii_lowercase().contains(s)) => {
                format!("{name}=<redacted>")
            }
            _ => pair.to_string(),
        })
        .collect::<Vec<_>>()
        .join("&");
    format!("{}?{}", uri.path(), query)
}

#[cfg(test)]
mod tests {
    #[test]
    fn credentials_are_redacted_from_logged_uris() {
        let uri = "/ws/api/tasks/2?api_key=secret123&device_name=Web&Token=x".parse().unwrap();
        assert_eq!(super::redacted_uri(&uri), "/ws/api/tasks/2?api_key=<redacted>&device_name=Web&Token=<redacted>");
        assert_eq!(super::redacted_uri(&"/api/health".parse().unwrap()), "/api/health");
    }
}

fn create_data_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        // Feeds that keep failing and failed sync imports
        .routes(routes!(handlers::feed_problems::get_feed_problems))
        .routes(routes!(handlers::feed_problems::refresh_failing_feed))
        .routes(routes!(handlers::feed_problems::retry_import))
        .routes(routes!(handlers::feed_problems::remove_import))
        .routes(routes!(handlers::feed_problems::dismiss_import))
        // Search service and Podping (admin)
        .routes(routes!(handlers::update_services::get_update_services))
        .routes(routes!(handlers::update_services::set_podping))
        // Durable in-app notifications (Activity center messages)
        .routes(routes!(handlers::notifications::list_notifications))
        .routes(routes!(handlers::notifications::mark_notification_read))
        .routes(routes!(handlers::notifications::dismiss_notification))
        .routes(routes!(handlers::notifications::clear_notifications))
        .routes(routes!(handlers::auth::get_key))
        .routes(routes!(handlers::auth::verify_mfa_and_get_key))
        .routes(routes!(handlers::auth::verify_api_key_endpoint))
        .routes(routes!(handlers::auth::get_user))
        .routes(routes!(handlers::auth::get_user_details_by_id))
        .routes(routes!(handlers::auth::get_self_service_status))
        .routes(routes!(handlers::auth::get_public_oidc_providers))
        .routes(routes!(handlers::auth::create_first_admin))
        .routes(routes!(handlers::auth::get_config))
        .routes(routes!(handlers::auth::first_login_done))
        .routes(routes!(handlers::auth::get_theme))
        .routes(routes!(handlers::auth::setup_time_info))
        .routes(routes!(handlers::auth::update_timezone))
        .routes(routes!(handlers::auth::update_date_format))
        .routes(routes!(handlers::auth::update_time_format))
        .routes(routes!(handlers::auth::get_auto_complete_seconds))
        .routes(routes!(handlers::auth::update_auto_complete_seconds))
        .routes(routes!(handlers::auth::user_admin_check))
        .routes(routes!(handlers::auth::import_opml))
        .routes(routes!(handlers::auth::import_progress))
        .routes(routes!(handlers::podcasts::return_episodes))
        .routes(routes!(handlers::podcasts::user_history))
        .routes(routes!(handlers::podcasts::increment_listen_time))
        .routes(routes!(handlers::podcasts::get_playback_speed))
        .routes(routes!(handlers::podcasts::get_auto_download_delete_days))
        .routes(routes!(handlers::podcasts::get_default_volume))
        .routes(routes!(handlers::podcasts::add_podcast))
        .routes(routes!(handlers::podcasts::update_podcast_info))
        .routes(routes!(handlers::podcasts::merge_podcasts))
        .routes(routes!(handlers::podcasts::unmerge_podcast))
        .routes(routes!(handlers::podcasts::get_merged_podcasts))
        .routes(routes!(handlers::podcasts::remove_podcast))
        .routes(routes!(handlers::podcasts::remove_podcast_id))
        .routes(routes!(handlers::podcasts::return_pods))
        .routes(routes!(handlers::podcasts::return_pods_extra))
        .routes(routes!(handlers::podcasts::get_time_info))
        .routes(routes!(handlers::podcasts::check_podcast))
        .routes(routes!(handlers::podcasts::check_episode_in_db))
        .routes(routes!(handlers::podcasts::queue_episode))
        .routes(routes!(handlers::podcasts::queue_episode_to_top))
        .routes(routes!(handlers::nowplaying::now_playing_devices))
        .routes(routes!(handlers::podcasts::remove_queued_episode))
        .routes(routes!(handlers::podcasts::get_queued_episodes))
        .routes(routes!(handlers::podcasts::reorder_queue))
        .routes(routes!(handlers::podcasts::clear_all_queue))
        .routes(routes!(handlers::podcasts::save_episode))
        .routes(routes!(handlers::podcasts::remove_saved_episode))
        .routes(routes!(handlers::podcasts::get_saved_episodes))
        .routes(routes!(handlers::podcasts::add_history))
        .routes(routes!(handlers::podcasts::get_podcast_id))
        .routes(routes!(handlers::podcasts::download_episode_list))
        .routes(routes!(handlers::podcasts::get_podcast_download_summary))
        .routes(routes!(handlers::podcasts::get_podcast_downloads_paged))
        .routes(routes!(handlers::podcasts::download_podcast))
        .routes(routes!(handlers::podcasts::delete_episode))
        .routes(routes!(handlers::podcasts::download_all_podcast))
        .routes(routes!(handlers::podcasts::download_status))
        .routes(routes!(handlers::podcasts::podcast_episodes))
        .routes(routes!(handlers::podcasts::get_podcast_id_from_ep_name))
        .routes(routes!(handlers::podcasts::get_episode_id_ep_name))
        .routes(routes!(handlers::podcasts::get_episode_metadata))
        .routes(routes!(handlers::podcasts::fetch_podcasting_2_data))
        .routes(routes!(handlers::podcasts::get_auto_download_status))
        .routes(routes!(handlers::podcasts::get_auto_queue_status))
        .routes(routes!(handlers::podcasts::get_auto_play_next_status))
        .routes(routes!(handlers::podcasts::get_next_podcast_episode))
        .routes(routes!(handlers::podcasts::get_next_playlist_episode))
        .routes(routes!(handlers::podcasts::get_feed_cutoff_days))
        .routes(routes!(handlers::podcasts::get_play_episode_details))
        .routes(routes!(handlers::podcasts::fetch_podcasting_2_pod_data))
        .routes(routes!(handlers::podcasts::mark_episode_completed))
        .routes(routes!(handlers::podcasts::update_episode_duration))
        // Bulk episode operations
        .routes(routes!(handlers::episodes::bulk_mark_episodes_completed))
        .routes(routes!(handlers::episodes::bulk_save_episodes))
        .routes(routes!(handlers::episodes::bulk_queue_episodes))
        .routes(routes!(handlers::episodes::bulk_download_episodes))
        .routes(routes!(handlers::episodes::bulk_delete_downloaded_episodes))
        .routes(routes!(handlers::episodes::share_episode))
        .routes(routes!(handlers::episodes::get_episode_by_url_key))
        .routes(routes!(handlers::settings::get_user_shared_links))
        .routes(routes!(handlers::settings::delete_shared_link))
        .routes(routes!(handlers::settings::extend_shared_link))
        .routes(routes!(handlers::podcasts::increment_played))
        .routes(routes!(handlers::podcasts::record_listen_duration))
        .routes(routes!(handlers::podcasts::get_podcast_id_from_ep_id))
        .routes(routes!(handlers::podcasts::get_stats))
        .routes(routes!(handlers::podcasts::get_extended_stats))
        .routes(routes!(handlers::podcasts::get_pinepods_version))
        .routes(routes!(handlers::podcasts::search_data))
        .routes(routes!(handlers::podcasts::proxy_search))
        .routes(routes!(handlers::podcasts::proxy_trending))
        .routes(routes!(handlers::podcasts::proxy_categories))
        .routes(routes!(handlers::podcasts::get_recommendations))
        .routes(routes!(handlers::podcasts::fetch_transcript))
        .routes(routes!(handlers::podcasts::home_overview))
        .routes(routes!(handlers::podcasts::get_playlists))
        .routes(routes!(handlers::podcasts::get_playlist_episodes))
        .routes(routes!(handlers::playlists::create_playlist))
        .routes(routes!(handlers::playlists::delete_playlist))
        .routes(routes!(handlers::playlists::update_playlist))
        .routes(routes!(handlers::collections::create_collection))
        .routes(routes!(handlers::collections::list_collections))
        .routes(routes!(handlers::collections::get_user_categories))
        .routes(routes!(handlers::collections::delete_collection))
        .routes(routes!(handlers::collections::update_collection))
        .routes(routes!(handlers::collections::add_episode_to_collection))
        .routes(routes!(handlers::collections::remove_episode_from_collection))
        .routes(routes!(handlers::collections::bulk_add_collection))
        .routes(routes!(handlers::collections::get_collection_episodes))
        .routes(routes!(handlers::collections::get_episode_collections))
        .routes(routes!(handlers::collections::get_collection_add_ui))
        .routes(routes!(handlers::collections::set_collection_add_ui))
        .routes(routes!(handlers::podcasts::get_podcast_details))
        .routes(routes!(handlers::podcasts::get_podcast_details_dynamic))
        .routes(routes!(handlers::podcasts::get_podpeople_discover))
        .routes(routes!(handlers::people::podcast_people))
        .routes(routes!(handlers::people::episode_people))
        .routes(routes!(handlers::people::search_people))
        .routes(routes!(handlers::people::lookup_person))
        .routes(routes!(handlers::people::followed_people))
        .routes(routes!(handlers::people::contribute_people))
        .routes(routes!(handlers::people::person_profile))
        .routes(routes!(handlers::people::follow_person, handlers::people::unfollow_person))
        .routes(routes!(handlers::podcasts::update_feed_cutoff_days))
        .routes(routes!(handlers::podcasts::fetch_podcast_feed))
        .routes(routes!(handlers::podcasts::youtube_episodes))
        .routes(routes!(handlers::podcasts::remove_youtube_channel))
        .routes(routes!(handlers::podcasts::stream_episode))
        .routes(routes!(handlers::podcasts::get_rss_key))
        .routes(routes!(handlers::podcasts::mark_episode_uncompleted))
        .routes(routes!(handlers::settings::set_theme))
        .routes(routes!(handlers::settings::get_custom_themes))
        .routes(routes!(handlers::settings::create_custom_theme, handlers::settings::delete_custom_theme))
        .routes(routes!(handlers::settings::get_user_info))
        .routes(routes!(handlers::settings::get_my_user_info))
        .routes(routes!(handlers::settings::add_user))
        .routes(routes!(handlers::settings::add_login_user))
        .routes(routes!(handlers::settings::set_fullname))
        .routes(routes!(handlers::settings::set_password))
        .routes(routes!(handlers::settings::delete_user))
        .routes(routes!(handlers::settings::set_email))
        .routes(routes!(handlers::settings::set_username))
        .routes(routes!(handlers::settings::set_isadmin))
        .routes(routes!(handlers::settings::final_admin))
        .routes(routes!(handlers::settings::enable_disable_downloads))
        .routes(routes!(handlers::settings::enable_disable_self_service))
        .routes(routes!(handlers::settings::rss_feed_status))
        .routes(routes!(handlers::settings::toggle_rss_feeds))
        .routes(routes!(handlers::settings::download_status))
        .routes(routes!(
            handlers::settings::get_download_metadata_settings,
            handlers::settings::set_download_metadata_settings
        ))
        .routes(routes!(handlers::settings::self_service_status))
        .routes(routes!(handlers::settings::save_email_settings))
        .routes(routes!(handlers::settings::get_email_settings))
        .routes(routes!(handlers::settings::send_test_email))
        .routes(routes!(handlers::settings::send_email))
        .routes(routes!(handlers::auth::reset_password_create_code))
        .routes(routes!(handlers::auth::verify_and_reset_password))
        .routes(routes!(handlers::settings::get_api_info))
        .routes(routes!(handlers::settings::create_api_key))
        .routes(routes!(handlers::settings::delete_api_key))
        .routes(routes!(handlers::settings::backup_user))
        .routes(routes!(handlers::settings::backup_server))
        .routes(routes!(handlers::settings::restore_server))
        .routes(routes!(handlers::settings::restore_status))
        .routes(routes!(handlers::settings::generate_mfa_secret))
        .routes(routes!(handlers::settings::verify_temp_mfa))
        .routes(routes!(handlers::settings::check_mfa_enabled))
        .routes(routes!(handlers::settings::save_mfa_secret))
        .routes(routes!(handlers::settings::delete_mfa))
        .routes(routes!(handlers::settings::initiate_nextcloud_login))
        .routes(routes!(handlers::settings::add_nextcloud_server))
        .routes(routes!(handlers::settings::verify_gpodder_auth))
        .routes(routes!(handlers::settings::add_gpodder_server))
        .routes(routes!(handlers::settings::get_gpodder_settings))
        .routes(routes!(handlers::settings::check_gpodder_settings))
        .routes(routes!(handlers::settings::remove_podcast_sync))
        .routes(routes!(handlers::sync::gpodder_status))
        .routes(routes!(handlers::sync::gpodder_toggle))
        .routes(routes!(handlers::refresh::refresh_pods_admin))
        .routes(routes!(handlers::refresh::refresh_gpodder_subscriptions_admin))
        .routes(routes!(handlers::refresh::refresh_nextcloud_subscriptions_admin))
        .routes(routes!(handlers::tasks::refresh_hosts))
        .routes(routes!(handlers::tasks::cleanup_tasks))
        .routes(routes!(handlers::tasks::auto_complete_episodes))
        .routes(routes!(handlers::tasks::update_playlists))
        .routes(routes!(handlers::settings::add_custom_podcast))
        .routes(routes!(handlers::local_podcast::add_local_podcast))
        .routes(routes!(handlers::local_podcast::add_local_podcast_artwork))
        .routes(routes!(handlers::local_podcast::refresh_local_podcast))
        .routes(routes!(handlers::local_podcast::list_local_directories))
        .routes(routes!(handlers::local_podcast::detect_local_cover))
        .routes(routes!(handlers::settings::get_notification_settings, handlers::settings::update_notification_settings))
        .routes(routes!(handlers::settings::set_playback_speed_user))
        .routes(routes!(handlers::settings::set_default_volume_user))
        .routes(routes!(handlers::settings::set_auto_download_delete_days_user))
        .routes(routes!(handlers::settings::set_global_podcast_cover_preference))
        .routes(routes!(handlers::settings::get_global_podcast_cover_preference))
        .routes(routes!(handlers::settings::test_notification))
        .routes(routes!(handlers::settings::add_oidc_provider))
        .routes(routes!(handlers::settings::update_oidc_provider))
        .routes(routes!(handlers::settings::list_oidc_providers))
        .routes(routes!(handlers::settings::remove_oidc_provider))
        .routes(routes!(handlers::settings::get_startpage, handlers::settings::update_startpage))
        .routes(routes!(handlers::youtube::search_youtube_channels))
        .routes(routes!(handlers::youtube::subscribe_to_youtube_channel))
        .routes(routes!(handlers::youtube::check_youtube_channel))
        .routes(routes!(handlers::youtube::get_youtube_video_filter))
        .routes(routes!(handlers::youtube::update_youtube_video_filter))
        .routes(routes!(handlers::settings::enable_auto_download))
        .routes(routes!(handlers::settings::enable_auto_queue))
        .routes(routes!(handlers::settings::enable_auto_play_next))
        .routes(routes!(handlers::settings::adjust_skip_times))
        .routes(routes!(handlers::settings::ai_status))
        .routes(routes!(handlers::settings::transcribe_episode))
        .routes(routes!(handlers::settings::get_episode_transcript))
        .routes(routes!(handlers::settings::adjust_auto_transcribe))
        .routes(routes!(handlers::settings::get_auto_transcribe))
        .routes(routes!(handlers::settings::adjust_silence_trim))
        .routes(routes!(handlers::settings::get_silence_trim))
        .routes(routes!(handlers::settings::get_episode_skip_segments))
        .routes(routes!(handlers::settings::detect_silence))
        .routes(routes!(handlers::settings::detect_ads))
        .routes(routes!(handlers::settings::adjust_ad_segment_review))
        .routes(routes!(handlers::settings::adjust_auto_ad_detect))
        .routes(routes!(handlers::settings::get_auto_ad_detect))
        .routes(routes!(handlers::settings::adjust_ad_skip_auto_activate))
        .routes(routes!(handlers::settings::get_ad_skip_auto_activate))
        .routes(routes!(handlers::settings::get_ai_settings, handlers::settings::update_ai_settings))
        .routes(routes!(handlers::settings::get_ytdlp_settings, handlers::settings::update_ytdlp_settings))
        .routes(routes!(handlers::settings::trigger_ytdlp_update))
        .routes(routes!(handlers::settings::get_ai_models))
        .routes(routes!(handlers::settings::ai_pull_model))
        .routes(routes!(handlers::settings::remove_category))
        .routes(routes!(handlers::settings::add_category))
        .routes(routes!(handlers::settings::set_podcast_playback_speed))
        .routes(routes!(handlers::settings::clear_podcast_playback_speed))
        .routes(routes!(handlers::settings::set_podcast_auto_download_delete_days))
        .routes(routes!(handlers::settings::clear_podcast_auto_download_delete_days))
        .routes(routes!(handlers::settings::set_podcast_cover_preference))
        .routes(routes!(handlers::settings::clear_podcast_cover_preference))
        .routes(routes!(handlers::settings::toggle_podcast_notifications))
        .routes(routes!(handlers::podcasts::get_notification_status))
        .routes(routes!(handlers::settings::toggle_podcast_favorite))
        .routes(routes!(handlers::podcasts::get_podcast_favorite_status))
        .routes(routes!(handlers::settings::get_user_rss_key))
        .routes(routes!(handlers::settings::verify_mfa))
        .routes(routes!(handlers::settings::schedule_backup))
        .routes(routes!(handlers::settings::get_scheduled_backup))
        .routes(routes!(handlers::settings::list_backup_files))
        .routes(routes!(handlers::settings::restore_from_backup_file))
        .routes(routes!(handlers::settings::manual_backup_to_directory))
        .routes(routes!(handlers::settings::delete_backup_file))
        .routes(routes!(handlers::settings::get_unmatched_podcasts))
        .routes(routes!(handlers::settings::update_podcast_index_id))
        .routes(routes!(handlers::settings::ignore_podcast_index_id))
        .routes(routes!(handlers::settings::get_ignored_podcasts))
        // Language preference endpoints
        .routes(routes!(handlers::settings::get_user_language))
        .routes(routes!(handlers::settings::update_user_language))
        .routes(routes!(handlers::settings::get_available_languages))
        .routes(routes!(handlers::settings::get_server_default_language))
        // Add more data routes as needed
}

fn create_podcast_routes() -> Router<AppState> {
    Router::new()
        .route("/notification_status", post(handlers::podcasts::get_notification_status))
}

fn create_episode_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::episodes::download_episode_file))
}

fn create_playlist_routes() -> Router<AppState> {
    Router::new()
        // Add playlist routes as needed
}

fn create_task_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::websocket::get_user_tasks))
        .routes(routes!(handlers::websocket::get_active_tasks))
        .routes(routes!(handlers::websocket::get_task_status))
}

fn create_async_routes() -> Router<AppState> {
    Router::new()
        // .route("/download_episode", post(handlers::tasks::download_episode))
        // .route("/import_opml", post(handlers::tasks::import_opml))
        // .route("/refresh_feeds", post(handlers::tasks::refresh_all_feeds))
        // .route("/episode/{episode_id}/metadata", get(handlers::tasks::quick_metadata_fetch))
}

fn create_proxy_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::proxy::proxy_image))
}

fn create_gpodder_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::sync::gpodder_test_connection))
        .routes(routes!(handlers::sync::gpodder_set_default))
        .routes(routes!(handlers::sync::gpodder_get_user_devices))
        .routes(routes!(handlers::sync::gpodder_get_all_devices, handlers::sync::gpodder_create_device))
        .routes(routes!(handlers::sync::gpodder_get_default_device))
        .routes(routes!(handlers::sync::gpodder_force_sync))
        .routes(routes!(handlers::sync::gpodder_sync))
        .routes(routes!(handlers::sync::gpodder_get_statistics))
}

fn create_sync_routes() -> OpenApiRouter<AppState> {
    use handlers::sync_settings as s;
    OpenApiRouter::new()
        .routes(routes!(s::get_server, s::put_server))
        .routes(routes!(s::list_app_passwords, s::create_app_password))
        .routes(routes!(s::revoke_app_password))
        .routes(routes!(s::dismiss_notice))
        .routes(routes!(s::list_devices))
        .routes(routes!(s::rename_device, s::remove_device))
        .routes(routes!(s::get_target, s::put_target, s::delete_target))
        .routes(routes!(s::test_target))
        .routes(routes!(s::run_sync))
        .routes(routes!(s::get_stats))
}

fn create_init_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::tasks::startup_tasks))
}

fn create_feed_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::feed::get_user_feed))
}

fn create_websocket_routes() -> Router<AppState> {
    Router::new()
        .route("/api/tasks/{user_id}", get(handlers::websocket::task_progress_websocket))
        .route("/api/data/episodes/{user_id}", get(handlers::refresh::websocket_refresh_episodes))
        .route("/api/nowplaying/{user_id}", get(handlers::nowplaying::now_playing_websocket))
}

fn create_auth_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(handlers::auth::store_oidc_state))
        .routes(routes!(handlers::auth::oidc_callback))
}
