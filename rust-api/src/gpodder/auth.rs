// Authentication: the `sessionid` cookie from login.json, or HTTP Basic with one of the user's
// app passwords (never the account password). Only users with the server enabled get in, and
// only to their own paths.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine as _;
use rand::RngExt as _;
use serde_json::json;
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use super::{GResult, GpodderError, NameExt};
use crate::database::DatabasePool;
use crate::AppState;

const SESSION_COOKIE: &str = "sessionid";
const SESSION_MAX_AGE: i64 = 30 * 24 * 3600;

// Verified Basic credentials (user, app password used), so argon2 doesn't run on every request.
const BASIC_CACHE_TTL: Duration = Duration::from_secs(600);
type Verified = (i32, Option<i32>);
static BASIC_CACHE: LazyLock<Mutex<HashMap<[u8; 32], (Verified, Instant)>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Who a request is from. `session` is set when it came with a valid session cookie,
/// `app_password` when it came with Basic credentials.
#[derive(Debug, Clone)]
pub(crate) struct Authed {
    pub user_id: i32,
    pub session: Option<i32>,
    pub app_password: Option<i32>,
}

fn cache_key(username: &str, password: &str) -> [u8; 32] {
    Sha256::digest(format!("{}:{}", username.to_lowercase(), password).as_bytes()).into()
}

fn cached(key: &[u8; 32]) -> Option<Verified> {
    let mut cache = BASIC_CACHE.lock().unwrap();
    match cache.get(key) {
        Some((verified, at)) if at.elapsed() < BASIC_CACHE_TTL => Some(*verified),
        Some(_) => {
            cache.remove(key);
            None
        }
        None => None,
    }
}

fn remember(key: [u8; 32], verified: Verified) {
    let mut cache = BASIC_CACHE.lock().unwrap();
    cache.retain(|_, (_, at)| at.elapsed() < BASIC_CACHE_TTL);
    cache.insert(key, (verified, Instant::now()));
}

/// Drop the user's cached Basic logins, so a revoked app password stops working at once.
pub fn forget_user(user_id: i32) {
    BASIC_CACHE.lock().unwrap().retain(|_, ((user, _), _)| *user != user_id);
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    headers.get_all(header::COOKIE).iter().filter_map(|v| v.to_str().ok()).find_map(|cookies| {
        cookies
            .split(';')
            .find_map(|c| c.trim().strip_prefix("sessionid=").map(str::to_string))
            .filter(|v| !v.is_empty())
    })
}

fn basic_credentials(headers: &HeaderMap) -> Option<(String, String)> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let (scheme, b64) = value.trim().split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }
    let raw = base64::engine::general_purpose::STANDARD.decode(b64.trim()).ok()?;
    let (u, p) = String::from_utf8(raw).ok()?.split_once(':').map(|(u, p)| (u.to_string(), p.to_string()))?;
    Some((u, p))
}

async fn verify_hash(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || crate::services::auth::verify_password(&password, &hash).unwrap_or(false))
        .await
        .unwrap_or(false)
}

/// Basic credentials checked against the user's app passwords.
async fn basic_user(db: &DatabasePool, username: &str, password: &str, path_user: &str) -> GResult<Option<Verified>> {
    if !username.eq_ignore_ascii_case(path_user) {
        return Ok(None);
    }
    let Some(user) = db.gpodder_user(username).await? else { return Ok(None) };
    if !user.enabled {
        warn!("gpodder login for user {} refused: the sync server is not enabled for them", user.user_id);
        return Ok(None);
    }
    let key = cache_key(username, password);
    if let Some(verified) = cached(&key).filter(|v| v.0 == user.user_id) {
        return Ok(Some(verified));
    }
    for (id, hash) in db.gpodder_app_passwords(user.user_id).await? {
        if verify_hash(password.to_string(), hash).await {
            if let Err(e) = db.gpodder_touch_app_password(id).await {
                warn!("Recording app password use for user {} failed: {}", user.user_id, e);
            }
            remember(key, (user.user_id, Some(id)));
            return Ok(Some((user.user_id, Some(id))));
        }
    }
    warn!("gpodder login for user {} failed: not one of their app passwords", user.user_id);
    Ok(None)
}

/// The authenticated user for a request on `path_user`'s data, or a 401.
pub(crate) async fn authenticate(state: &AppState, headers: &HeaderMap, path_user: &str) -> GResult<Authed> {
    let db = &state.db_pool;
    if let Some(token) = session_cookie(headers) {
        if let Some(s) = db.gpodder_session(&token).await? {
            if s.enabled && s.username.eq_ignore_ascii_case(path_user) {
                return Ok(Authed { user_id: s.user_id, session: Some(s.session_id), app_password: None });
            }
        }
    }
    if let Some((u, p)) = basic_credentials(headers) {
        if let Some((user_id, app_password)) = basic_user(db, &u, &p, path_user).await? {
            return Ok(Authed { user_id, session: None, app_password });
        }
    } else {
        debug!("gpodder request without valid credentials");
    }
    Err(GpodderError::unauthorized())
}

fn new_token() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok()).map(str::trim).filter(|s| !s.is_empty())
}

/// The client reached us over https (the first `X-Forwarded-Proto` entry, set by the bundled
/// nginx or an outer proxy).
fn forwarded_https(headers: &HeaderMap) -> bool {
    header_str(headers, "x-forwarded-proto").and_then(|v| v.split(',').next()).is_some_and(|p| p.trim().eq_ignore_ascii_case("https"))
}

/// POST /api/2/auth/{user}/login.json. A request that already has a valid session keeps it.
pub(crate) async fn login(State(state): State<AppState>, Path(user): Path<String>, headers: HeaderMap) -> GResult<Response> {
    let user = NameExt::json_only(&user)?;
    let authed = authenticate(&state, &headers, &user).await?;
    if authed.session.is_some() {
        return Ok(Json(json!({ "status": "success" })).into_response());
    }
    let token = new_token();
    let client_ip = header_str(&headers, "x-real-ip")
        .or_else(|| header_str(&headers, "x-forwarded-for").and_then(|v| v.split(',').next()).map(str::trim));
    state
        .db_pool
        .gpodder_create_session(authed.user_id, &token, header_str(&headers, "user-agent"), client_ip, authed.app_password)
        .await?;
    let secure = forwarded_https(&headers);
    let cookie = format!(
        "{SESSION_COOKIE}={token}; Path=/; Max-Age={SESSION_MAX_AGE}; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    );
    let mut resp = Json(json!({ "status": "success" })).into_response();
    resp.headers_mut().insert(header::SET_COOKIE, HeaderValue::from_str(&cookie).map_err(|_| GpodderError::bad_request("bad cookie"))?);
    Ok(resp)
}

/// POST /api/2/auth/{user}/logout.json: ends the cookie's session, if any.
pub(crate) async fn logout(State(state): State<AppState>, Path(_user): Path<String>, headers: HeaderMap) -> GResult<Response> {
    if let Some(token) = session_cookie(&headers) {
        state.db_pool.gpodder_delete_session(&token).await?;
    }
    let mut resp = Json(json!({ "status": "success" })).into_response();
    resp.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("sessionid=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"),
    );
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_from_headers() {
        let mut h = HeaderMap::new();
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Basic Ym9iOnB3OndpdGg6Y29sb25z"));
        assert_eq!(basic_credentials(&h), Some(("bob".into(), "pw:with:colons".into())));
        h.insert(header::COOKIE, HeaderValue::from_static("a=1; sessionid=abc; b=2"));
        assert_eq!(session_cookie(&h).as_deref(), Some("abc"));
        h.insert(header::COOKIE, HeaderValue::from_static("sessionid="));
        assert_eq!(session_cookie(&h), None);
        h.insert(header::AUTHORIZATION, HeaderValue::from_static("Bearer xyz"));
        assert_eq!(basic_credentials(&h), None);
    }

    #[test]
    fn https_from_forwarded_proto() {
        let mut h = HeaderMap::new();
        assert!(!forwarded_https(&h));
        h.insert("x-forwarded-proto", HeaderValue::from_static("http"));
        assert!(!forwarded_https(&h));
        h.insert("x-forwarded-proto", HeaderValue::from_static("HTTPS, http"));
        assert!(forwarded_https(&h));
    }

    #[test]
    fn cache_keys_ignore_username_case_only() {
        assert_eq!(cache_key("Bob", "pw"), cache_key("bob", "pw"));
        assert_ne!(cache_key("bob", "pw"), cache_key("bob", "PW"));
        let key = cache_key("cache-test-user", "pw");
        assert_eq!(cached(&key), None);
        remember(key, (42, Some(7)));
        assert_eq!(cached(&key), Some((42, Some(7))));
        forget_user(42);
        assert_eq!(cached(&key), None);
    }
}
