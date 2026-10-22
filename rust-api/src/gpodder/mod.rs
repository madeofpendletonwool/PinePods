// gpodder.net v2 + Simple API server. Uploads go to the sync log and are applied with
// `SyncOrigin::Client`; download cursors are server receive time (`ServerTs`). Not in the
// OpenAPI document.

mod auth;
mod db;
mod devices;
mod episodes;
mod misc;
mod subscriptions;

pub use db::{GpodderStats, GpodderStatsAction, GpodderStatsDevice};

use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

use crate::error::AppError;
use crate::AppState;

/// Upload bodies are JSON lists; a first sync of a big library is a few hundred KB.
const BODY_LIMIT: usize = 10 * 1024 * 1024;

pub fn router() -> Router<AppState> {
    Router::new()
        // v2
        .route("/api/2/auth/{user}/login.json", post(auth::login))
        .route("/api/2/auth/{user}/logout.json", post(auth::logout))
        .route("/api/2/devices/{user}", get(devices::list))
        .route("/api/2/devices/{user}/{device}", post(devices::update))
        .route("/api/2/updates/{user}/{device}", get(devices::updates))
        .route("/api/2/subscriptions/{user}", get(subscriptions::all))
        .route(
            "/api/2/subscriptions/{user}/{device}",
            get(subscriptions::changes).post(subscriptions::upload_changes).put(subscriptions::put_list),
        )
        .route("/api/2/episodes/{user}", get(episodes::download).post(episodes::upload))
        .route("/api/2/settings/{user}/{scope}", get(misc::get_settings).post(misc::save_settings))
        .route("/api/2/sync-devices/{user}", get(misc::sync_devices).post(misc::update_sync_devices))
        // Not ported (directory, lists, favorites): valid empty answers, see misc.rs.
        .route("/api/2/lists/{user}", get(misc::empty_for_user))
        .route("/api/2/favorites/{user}", get(misc::empty_for_user))
        .route("/api/2/tags/{count}", get(misc::empty_list))
        .route("/api/2/tag/{tag}/{count}", get(misc::empty_list))
        // Simple API
        .route("/subscriptions/{user}", get(subscriptions::simple_all))
        .route("/subscriptions/{user}/{device}", get(subscriptions::simple_get).put(subscriptions::simple_put))
        .route("/suggestions/{count}", get(misc::empty_suggestions))
        .route("/toplist/{count}", get(misc::empty_list))
        .route("/search.json", get(misc::empty_list))
        .route("/search.jsonp", get(misc::empty_list))
        .route("/search.xml", get(misc::empty_list))
        .route("/search.opml", get(misc::empty_list))
        .route("/search.txt", get(misc::empty_list))
        .layer(DefaultBodyLimit::max(BODY_LIMIT))
}

// ---- path segments ------------------------------------------------------------------------

/// Response formats of the Simple API. v2 is always JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Format {
    Json,
    Jsonp,
    Txt,
    Opml,
    Xml,
}

impl Format {
    fn parse(ext: &str) -> Option<Format> {
        match ext.to_ascii_lowercase().as_str() {
            "json" => Some(Format::Json),
            "jsonp" => Some(Format::Jsonp),
            "txt" => Some(Format::Txt),
            "opml" => Some(Format::Opml),
            "xml" => Some(Format::Xml),
            _ => None,
        }
    }
}

/// A `{name}.{ext}` path segment (`bob.json`, `phone.opml`). Names may contain dots
/// (`john.doe.json`), so only a known format after the last dot counts as the extension; a
/// segment without one is a bare name (Go accepted v2 paths without `.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NameExt {
    pub name: String,
    pub format: Option<Format>,
}

impl NameExt {
    pub fn parse(segment: &str) -> NameExt {
        if let Some((name, ext)) = segment.rsplit_once('.') {
            if let Some(format) = Format::parse(ext).filter(|_| !name.is_empty()) {
                return NameExt { name: name.to_string(), format: Some(format) };
            }
        }
        NameExt { name: segment.to_string(), format: None }
    }

    /// v2 endpoints only speak JSON.
    pub fn json_only(segment: &str) -> GResult<String> {
        let parsed = NameExt::parse(segment);
        match parsed.format {
            None | Some(Format::Json) => Ok(parsed.name),
            Some(_) => Err(GpodderError::bad_request("only the json format is supported here")),
        }
    }
}

// ---- errors -------------------------------------------------------------------------------

/// An error answered as one JSON document `{"error": "..."}` with no handler output after it.
#[derive(Debug)]
pub(crate) struct GpodderError {
    status: StatusCode,
    message: String,
}

pub(crate) type GResult<T> = Result<T, GpodderError>;

impl GpodderError {
    pub fn unauthorized() -> Self {
        GpodderError { status: StatusCode::UNAUTHORIZED, message: "unauthorized".into() }
    }
    pub fn bad_request(msg: impl Into<String>) -> Self {
        GpodderError { status: StatusCode::BAD_REQUEST, message: msg.into() }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        GpodderError { status: StatusCode::NOT_FOUND, message: msg.into() }
    }
}

impl From<AppError> for GpodderError {
    fn from(e: AppError) -> Self {
        match e {
            AppError::BadRequest(m) | AppError::Validation(m) => GpodderError::bad_request(m),
            AppError::NotFound(m) => GpodderError::not_found(m),
            other => {
                tracing::error!("gpodder request failed: {}", other);
                GpodderError { status: StatusCode::INTERNAL_SERVER_ERROR, message: "internal server error".into() }
            }
        }
    }
}

impl From<sqlx::Error> for GpodderError {
    fn from(e: sqlx::Error) -> Self {
        AppError::from(e).into()
    }
}

impl IntoResponse for GpodderError {
    fn into_response(self) -> Response {
        let mut resp = (self.status, Json(json!({ "error": self.message }))).into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            resp.headers_mut().insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Basic realm=\"PinePods\""));
        }
        resp
    }
}

// ---- helpers ------------------------------------------------------------------------------

/// `since` from the query string: absent or empty is 0, anything but an integer is a 400.
pub(crate) fn parse_since(since: Option<&str>) -> GResult<i64> {
    match since.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(0),
        Some(s) => s.parse::<i64>().map(|n| n.max(0)).map_err(|_| GpodderError::bad_request("since must be an integer")),
    }
}

/// Trimmed, non-empty, de-duplicated URLs in their original order.
pub(crate) fn clean_urls(urls: &[String]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    urls.iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty() && seen.insert(u.clone())).collect()
}

/// The cursor a download hands out: one second behind the DB clock, so a row stamped just
/// before a slow commit is still sent next time (`>=` makes the repeat harmless).
pub(crate) async fn download_cursor(db: &crate::database::DatabasePool) -> GResult<i64> {
    Ok(db.gpodder_now().await? - 1)
}

pub(crate) fn is_http(url: &str) -> bool {
    let l = url.to_ascii_lowercase();
    l.starts_with("http://") || l.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ext_segments() {
        assert_eq!(NameExt::parse("bob.json"), NameExt { name: "bob".into(), format: Some(Format::Json) });
        assert_eq!(NameExt::parse("john.doe.opml"), NameExt { name: "john.doe".into(), format: Some(Format::Opml) });
        assert_eq!(NameExt::parse("my.phone"), NameExt { name: "my.phone".into(), format: None });
        assert_eq!(NameExt::parse("phone"), NameExt { name: "phone".into(), format: None });
        assert_eq!(NameExt::parse(".json"), NameExt { name: ".json".into(), format: None });
        assert_eq!(NameExt::json_only("bob.json").unwrap(), "bob");
        assert_eq!(NameExt::json_only("bob").unwrap(), "bob");
        assert!(NameExt::json_only("bob.txt").is_err());
    }

    #[test]
    fn since_and_urls() {
        assert_eq!(parse_since(None).unwrap(), 0);
        assert_eq!(parse_since(Some("")).unwrap(), 0);
        assert_eq!(parse_since(Some("1700000000")).unwrap(), 1_700_000_000);
        assert!(parse_since(Some("yesterday")).is_err());
        let urls = clean_urls(&[" https://a/feed ".into(), "https://a/feed".into(), "".into(), "https://b".into()]);
        assert_eq!(urls, vec!["https://a/feed".to_string(), "https://b".to_string()]);
        assert!(is_http("HTTPS://x") && !is_http("ftp://x"));
    }
}
