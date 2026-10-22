// External sync servers the engine talks to. Errors carry the method, the path and the status
// or transport failure only: they end up in SyncTargets.LastError, so never URLs with
// credentials, headers or response bodies.

pub mod gpodder;
pub mod nextcloud;

use async_trait::async_trait;
use reqwest::header::HeaderMap;
use reqwest::{Method, StatusCode, Url};
use serde_json::Value;

use super::model::{EpisodeAction, OutboundAction};
use crate::error::{AppError, AppResult};

/// Subscription changes since a cursor, and the server's cursor for the next pull.
#[derive(Debug, Default, Clone)]
pub struct SubChanges {
    pub add: Vec<String>,
    pub remove: Vec<String>,
    pub timestamp: i64,
}

/// Episode actions since a cursor and the server's next cursor. Actions that don't parse
/// (unknown kinds, garbage) are left out and counted instead of failing the pull.
#[derive(Debug, Default, Clone)]
pub struct PulledActions {
    pub actions: Vec<EpisodeAction>,
    pub timestamp: i64,
    pub unreadable: usize,
}

#[async_trait]
pub trait SyncTarget: Send + Sync {
    /// Once per run, before anything else.
    async fn login(&mut self) -> AppResult<()>;
    /// Make sure Pinepods' device exists on the server. Returns the device name if it had to be
    /// chosen or changed, so the caller can store it. `full` = a full sync (always configure it).
    async fn prepare_device(&mut self, _full: bool) -> AppResult<Option<String>> {
        Ok(None)
    }
    async fn pull_subscriptions(&self, since: i64) -> AppResult<SubChanges>;
    async fn push_subscriptions(&self, add: &[String], remove: &[String]) -> AppResult<()>;
    async fn pull_actions(&self, since: i64) -> AppResult<PulledActions>;
    async fn push_actions(&self, batch: &[OutboundAction]) -> AppResult<()>;
    fn push_batch_size(&self) -> usize;
    /// Pinepods' own device on the server, if the server tracks devices.
    fn own_device(&self) -> Option<&str> {
        None
    }
}

/// HTTP for one run: one client (cookie jar, timeouts), Basic credentials and the base URL.
pub(crate) struct Http {
    client: reqwest::Client,
    base: Url,
    pub username: String,
    password: String,
    /// Send Basic on every request (no session cookie).
    pub basic: bool,
}

impl Http {
    pub fn new(base_url: &str, username: &str, password: &str) -> AppResult<Self> {
        let trimmed = base_url.trim().trim_end_matches('/');
        let mut base = Url::parse(trimmed).map_err(|_| AppError::bad_request("The sync server URL isn't a valid URL"))?;
        if !matches!(base.scheme(), "http" | "https") {
            return Err(AppError::bad_request("The sync server URL must start with http:// or https://"));
        }
        // Credentials only ever travel as Basic auth.
        let _ = base.set_username("");
        let _ = base.set_password(None);
        let client = crate::services::http::sync_client_builder()
            .build()
            .map_err(|_| AppError::internal("Building the sync HTTP client failed"))?;
        Ok(Http { client, base, username: username.to_string(), password: password.to_string(), basic: true })
    }

    /// `base` + path segments (each percent-encoded).
    pub fn url(&self, segments: &[&str], query: &[(&str, String)]) -> Url {
        let mut url = self.base.clone();
        if let Ok(mut path) = url.path_segments_mut() {
            path.pop_if_empty().extend(segments);
        }
        if !query.is_empty() {
            url.query_pairs_mut().extend_pairs(query);
        }
        url
    }

    fn describe(method: &Method, url: &Url) -> String {
        format!("{method} {}", url.path())
    }

    /// Send a request and return the status, headers and body text. Transport failures are errors.
    pub async fn send_raw(&self, method: Method, url: Url, body: Option<&Value>, basic: bool) -> AppResult<(StatusCode, HeaderMap, String)> {
        let what = Self::describe(&method, &url);
        let mut req = self.client.request(method, url).header(reqwest::header::ACCEPT, "application/json");
        if basic {
            req = req.basic_auth(&self.username, Some(&self.password));
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        let resp = req.send().await.map_err(|e| AppError::internal(format!("{what}: {}", transport_error(&e))))?;
        let status = resp.status();
        let headers = resp.headers().clone();
        let text = resp.text().await.map_err(|e| AppError::internal(format!("{what}: {}", transport_error(&e))))?;
        Ok((status, headers, text))
    }

    /// Send a request; any non-2xx status is an error. An empty body is `null`.
    pub async fn send(&self, method: Method, url: Url, body: Option<&Value>) -> AppResult<Value> {
        let what = Self::describe(&method, &url);
        let (status, _, text) = self.send_raw(method, url, body, self.basic).await?;
        if !status.is_success() {
            return Err(AppError::internal(format!("{what}: HTTP {}", status.as_u16())));
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).map_err(|_| AppError::internal(format!("{what}: the response isn't JSON")))
    }
}

fn transport_error(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "timed out".into()
    } else if e.is_connect() {
        "couldn't connect".into()
    } else if e.is_decode() || e.is_body() {
        "couldn't read the response".into()
    } else {
        "request failed".into()
    }
}

/// A cursor from a response: an integer, or an integer in a string.
pub(crate) fn timestamp_of(v: &Value, what: &str) -> AppResult<i64> {
    match v.get("timestamp") {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    }
    .ok_or_else(|| AppError::internal(format!("{what}: the response has no timestamp")))
}

/// `{add, remove, timestamp}`.
pub(crate) fn sub_changes(v: &Value, what: &str) -> AppResult<SubChanges> {
    let list = |k: &str| -> Vec<String> {
        v.get(k)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default()
    };
    Ok(SubChanges { add: list("add"), remove: list("remove"), timestamp: timestamp_of(v, what)? })
}

/// `{actions: [...], timestamp}`.
pub(crate) fn pulled_actions(v: &Value, what: &str) -> AppResult<PulledActions> {
    let mut out = PulledActions { timestamp: timestamp_of(v, what)?, ..Default::default() };
    for a in v.get("actions").and_then(Value::as_array).into_iter().flatten() {
        match serde_json::from_value::<EpisodeAction>(a.clone()) {
            Ok(action) => out.actions.push(action),
            Err(_) => out.unreadable += 1,
        }
    }
    Ok(out)
}
