//! An in-memory gpodder v2 + Nextcloud gpoddersync server.
//!
//! Two jobs:
//! - **Reference server** for the protocol tests (`PINEPODS_TEST_GPODDER=mock`): it follows
//!   the cursor rules below, so the tests are checked against
//!   something correct while the Rust port doesn't exist yet.
//! - **Stand-in external server** for the external sync engine: Pinepods talks to
//!   it as it would to oPodSync or a Nextcloud with the gpoddersync app. Every request is
//!   recorded, and failures can be injected.
//!
//! Rules it implements:
//! - Every change is stamped with the server's receive time (unix seconds). `since` selects
//!   changes stamped `>= since`, and a response's `timestamp` is taken *before* the query, so
//!   a change arriving in the same second is resent rather than lost.
//! - An upload's response `timestamp` is the cursor this client was last given by a download
//!   (per device for subscriptions, per session for episodes; now if it never downloaded).
//!   AntennaPod uses it as its next `since`, so a change another device made between
//!   AntennaPod's download and upload is still delivered.
//! - Subscription changes are folded per URL: a URL appears in `add` or `remove`, never both.
//! - `PUT` of a device's subscription list only changes what that device itself reported.
//! - Outgoing `play` actions always carry `started`, `position` and `total`.
//! - A batch with an invalid action is rejected whole (400); nothing from it is stored.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::{header, HeaderMap, Method, StatusCode},
    response::Response,
    Router,
};
use base64::Engine as _;
use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub struct Recorded {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub body: String,
    pub status: u16,
}

#[derive(Clone, Debug)]
struct SubEntry {
    ts: i64,
    device: String,
    url: String,
    add: bool,
}

#[derive(Clone, Debug)]
struct ActionEntry {
    ts: i64,
    device: String,
    action: Value,
}

#[derive(Default)]
struct UserData {
    password: String,
    devices: BTreeMap<String, (String, String)>, // id -> (caption, type)
    subs: Vec<SubEntry>,
    actions: Vec<ActionEntry>,
    /// Last cursor handed out by a download, keyed "subs:<device>" / "eps:<client>".
    last_download: HashMap<String, i64>,
}

#[derive(Default)]
struct Inner {
    users: HashMap<String, UserData>,
    sessions: HashMap<String, String>,
    log: Vec<Recorded>,
    /// (path prefix, status, remaining count)
    failures: Vec<(String, u16, usize)>,
}

type Reply = (u16, Value, Option<String>);

#[derive(Clone)]
pub struct MockGpodder {
    pub base_url: String,
    inner: Arc<Mutex<Inner>>,
    task: Arc<tokio::task::JoinHandle<()>>,
}

impl Drop for MockGpodder {
    fn drop(&mut self) {
        if Arc::strong_count(&self.task) == 1 {
            self.task.abort();
        }
    }
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

impl MockGpodder {
    pub async fn start() -> MockGpodder {
        let inner = Arc::new(Mutex::new(Inner::default()));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().fallback(handle).with_state(inner.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        MockGpodder { base_url: format!("http://{addr}"), inner, task: Arc::new(task) }
    }

    pub fn add_user(&self, username: &str, password: &str) {
        self.inner
            .lock()
            .unwrap()
            .users
            .insert(username.to_lowercase(), UserData { password: password.to_string(), ..Default::default() });
    }

    /// Every request so far.
    pub fn requests(&self) -> Vec<Recorded> {
        self.inner.lock().unwrap().log.clone()
    }

    /// Answer the next `count` requests whose path starts with `path_prefix` with `status`.
    pub fn fail_next(&self, path_prefix: &str, status: u16, count: usize) {
        self.inner.lock().unwrap().failures.push((path_prefix.to_string(), status, count));
    }

    /// The user's current subscriptions, folded from every device's changes.
    pub fn subscriptions(&self, username: &str) -> Vec<String> {
        let inner = self.inner.lock().unwrap();
        inner.users.get(&username.to_lowercase()).map(current_subs).unwrap_or_default().into_iter().collect()
    }

    /// Every stored episode action for the user, as it would be served.
    pub fn episode_actions(&self, username: &str) -> Vec<Value> {
        let inner = self.inner.lock().unwrap();
        inner
            .users
            .get(&username.to_lowercase())
            .map(|u| u.actions.iter().map(served_action).collect())
            .unwrap_or_default()
    }
}

fn fold_subs<'a>(entries: impl Iterator<Item = &'a SubEntry>) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for e in entries {
        if e.add {
            set.insert(e.url.clone());
        } else {
            set.remove(&e.url);
        }
    }
    set
}

fn current_subs(u: &UserData) -> BTreeSet<String> {
    fold_subs(u.subs.iter())
}

fn device_subs(u: &UserData, device: &str) -> BTreeSet<String> {
    fold_subs(u.subs.iter().filter(|e| e.device == device))
}

/// What a stored action looks like on the way out.
fn served_action(e: &ActionEntry) -> Value {
    let mut a = e.action.clone();
    a["device"] = json!(e.device);
    if a["action"] == "play" {
        let position = a["position"].as_i64().unwrap_or(0).max(0);
        if a.get("started").and_then(Value::as_i64).is_none_or(|s| s < 0) {
            a["started"] = json!(0);
        }
        if a.get("total").and_then(Value::as_i64).is_none_or(|t| t <= 0) {
            a["total"] = json!(position.max(1));
        }
    }
    a
}

/// Parse an action timestamp: ISO-8601 with or without offset, or unix seconds.
fn parse_ts(v: &Value) -> Result<Option<i64>, String> {
    match v {
        Value::Null => Ok(None),
        Value::Number(n) => n.as_i64().map(Some).ok_or_else(|| format!("bad timestamp {n}")),
        Value::String(s) => {
            if let Ok(n) = s.parse::<i64>() {
                return Ok(Some(n));
            }
            if let Ok(t) = chrono::DateTime::parse_from_rfc3339(s) {
                return Ok(Some(t.timestamp()));
            }
            let head = s.trim_end_matches('Z');
            let head = head.split('.').next().unwrap_or(head);
            chrono::NaiveDateTime::parse_from_str(head, "%Y-%m-%dT%H:%M:%S")
                .map(|t| Some(t.and_utc().timestamp()))
                .map_err(|_| format!("bad timestamp {s:?}"))
        }
        other => Err(format!("bad timestamp {other}")),
    }
}

/// Validate and normalise one uploaded action → (device, action).
fn normalise_action(a: &Value) -> Result<(Option<String>, Value), String> {
    let s = |k: &str| a.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string);
    let podcast = s("podcast").ok_or("action without podcast")?;
    let episode = s("episode").ok_or("action without episode")?;
    let action = s("action").ok_or("action without action")?.to_lowercase();
    if !["new", "download", "play", "delete", "flattr"].contains(&action.as_str()) {
        return Err(format!("unknown action {action:?}"));
    }
    let ts = parse_ts(a.get("timestamp").unwrap_or(&Value::Null))?.unwrap_or_else(now);
    let mut out = json!({
        "podcast": podcast,
        "episode": episode,
        "action": action,
        "timestamp": chrono::DateTime::from_timestamp(ts, 0).unwrap().format("%Y-%m-%dT%H:%M:%S").to_string(),
    });
    if let Some(g) = s("guid") {
        out["guid"] = json!(g);
    }
    if action == "play" {
        for k in ["started", "position", "total"] {
            match a.get(k) {
                None | Some(Value::Null) => {}
                Some(v) => {
                    let n = v.as_i64().ok_or_else(|| format!("{k} is not an integer: {v}"))?;
                    if !(-1..=i32::MAX as i64).contains(&n) {
                        return Err(format!("{k} out of range: {n}"));
                    }
                    out[k] = json!(n);
                }
            }
        }
        if out.get("position").is_none() {
            return Err("play action without position".into());
        }
    }
    Ok((s("device"), out))
}

fn reply(status: u16, body: Value) -> Reply {
    (status, body, None)
}

fn basic_credentials(headers: &HeaderMap) -> Option<(String, String)> {
    let h = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let b64 = h.strip_prefix("Basic ")?;
    let raw = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let raw = String::from_utf8(raw).ok()?;
    let (u, p) = raw.split_once(':')?;
    Some((u.to_string(), p.to_string()))
}

fn session_cookie(headers: &HeaderMap) -> Option<String> {
    let cookies = headers.get(header::COOKIE)?.to_str().ok()?;
    cookies.split(';').find_map(|c| c.trim().strip_prefix("sessionid=").map(str::to_string))
}

/// The authenticated user (lower-cased) if the request may act as `path_user`.
fn authenticate(inner: &Inner, headers: &HeaderMap, path_user: &str, allow_session: bool) -> Option<String> {
    let path_user = path_user.to_lowercase();
    if let Some((u, p)) = basic_credentials(headers) {
        let u = u.to_lowercase();
        let ok = inner.users.get(&u).is_some_and(|d| d.password == p);
        return (ok && u == path_user).then_some(u);
    }
    if allow_session {
        if let Some(u) = session_cookie(headers).and_then(|sid| inner.sessions.get(&sid)) {
            return (*u == path_user).then(|| u.clone());
        }
    }
    None
}

fn strip_ext<'a>(s: &'a str, ext: &str) -> &'a str {
    s.strip_suffix(ext).unwrap_or(s)
}

fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    query?.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=').unwrap_or((kv, ""));
        (k == key).then(|| urlencoding::decode(v).map(|c| c.into_owned()).unwrap_or_default())
    })
}

fn since(query: Option<&str>) -> Result<i64, Reply> {
    match query_param(query, "since") {
        None => Ok(0),
        Some(s) => s.parse().map_err(|_| reply(400, json!({ "error": "since must be an integer" }))),
    }
}

async fn handle(State(inner): State<Arc<Mutex<Inner>>>, req: Request) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query = req.uri().query().map(str::to_string);
    let headers = req.headers().clone();
    let body = to_bytes(req.into_body(), 64 * 1024 * 1024).await.unwrap_or_default();
    let body_text = String::from_utf8_lossy(&body).into_owned();

    let mut inner = inner.lock().unwrap();
    let injected = inner.failures.iter_mut().find(|(p, _, n)| *n > 0 && path.starts_with(p.as_str())).map(|f| {
        f.2 -= 1;
        f.1
    });
    let (status, out, cookie) = match injected {
        Some(status) => reply(status, json!({ "error": "injected failure" })),
        None => route(&mut inner, &method, &path, query.as_deref(), &headers, &body_text),
    };
    inner.log.push(Recorded { method: method.to_string(), path, query, body: body_text, status });
    drop(inner);

    let mut resp = Response::builder().status(StatusCode::from_u16(status).unwrap());
    if let Some(c) = cookie {
        resp = resp.header(header::SET_COOKIE, format!("sessionid={c}; Path=/; HttpOnly"));
    }
    match out {
        Value::String(text) => resp.header(header::CONTENT_TYPE, "text/plain").body(Body::from(text)).unwrap(),
        v => resp.header(header::CONTENT_TYPE, "application/json").body(Body::from(v.to_string())).unwrap(),
    }
}

fn parse_json(body: &str) -> Result<Value, Reply> {
    serde_json::from_str::<Value>(body).map_err(|_| reply(400, json!({ "error": "invalid JSON" })))
}

fn route(inner: &mut Inner, method: &Method, path: &str, query: Option<&str>, headers: &HeaderMap, body: &str) -> Reply {
    let segs: Vec<&str> = path.trim_matches('/').split('/').collect();
    let unauthorized = || reply(401, json!({ "error": "unauthorized" }));

    // Nextcloud gpoddersync (Basic auth only; the user is whoever authenticates).
    if let ["index.php", "apps", "gpoddersync", rest @ ..] = segs.as_slice() {
        let Some((user, _)) = basic_credentials(headers) else { return unauthorized() };
        let Some(user) = authenticate(inner, headers, &user, false) else { return unauthorized() };
        let ud = inner.users.get_mut(&user).unwrap();
        return nextcloud_route(ud, method, rest, query, body).unwrap_or_else(|e| e);
    }

    // gpodder v2 lives under /api/2; the Simple API at the root.
    let v2 = segs.first() == Some(&"api") && segs.get(1) == Some(&"2");
    let segs: &[&str] = if v2 { &segs[2..] } else { &segs };

    match (method, segs) {
        (&Method::POST, ["auth", user, "login.json"]) if v2 => {
            let Some(u) = authenticate(inner, headers, user, false) else { return unauthorized() };
            let sid = uuid::Uuid::new_v4().simple().to_string();
            inner.sessions.insert(sid.clone(), u);
            (200, json!({}), Some(sid))
        }
        (&Method::POST, ["auth", _, "logout.json"]) if v2 => {
            if let Some(sid) = session_cookie(headers) {
                inner.sessions.remove(&sid);
            }
            reply(200, json!({}))
        }
        (_, [kind, user_file, ..]) if v2 && ["devices", "subscriptions", "episodes"].contains(kind) => {
            let user = strip_ext(user_file, ".json");
            let Some(u) = authenticate(inner, headers, user, true) else { return unauthorized() };
            // Who is syncing: AntennaPod's episode downloads carry no device, but its
            // requests share a session cookie.
            let client = session_cookie(headers).unwrap_or_else(|| format!("basic:{u}"));
            let ud = inner.users.get_mut(&u).unwrap();
            v2_route(ud, &client, method, segs, query, body).unwrap_or_else(|e| e)
        }
        (_, ["subscriptions", user, device_file]) if !v2 => {
            let Some(u) = authenticate(inner, headers, user, true) else { return unauthorized() };
            let ud = inner.users.get_mut(&u).unwrap();
            simple_route(ud, method, device_file, body).unwrap_or_else(|e| e)
        }
        _ => reply(404, json!({ "error": "not found" })),
    }
}

fn nextcloud_route(ud: &mut UserData, method: &Method, rest: &[&str], query: Option<&str>, body: &str) -> Result<Reply, Reply> {
    Ok(match (method, rest) {
        (&Method::GET, ["subscriptions"]) => {
            let s = since(query)?;
            let t = now();
            reply(200, fold_changes(ud, s, None, t))
        }
        (&Method::POST, ["subscription_change", "create"]) => {
            let t = record_sub_changes(ud, "nextcloud", &parse_json(body)?)?;
            reply(200, json!({ "timestamp": t }))
        }
        (&Method::GET, ["episode_action"]) => {
            let s = since(query)?;
            let t = now();
            let actions: Vec<Value> = ud.actions.iter().filter(|a| a.ts >= s).map(served_action).collect();
            reply(200, json!({ "actions": actions, "timestamp": t }))
        }
        (&Method::POST, ["episode_action", "create"]) => {
            let t = record_actions(ud, "nextcloud", &parse_json(body)?)?;
            reply(200, json!({ "timestamp": t }))
        }
        _ => reply(404, json!({ "error": "not found" })),
    })
}

/// Simple API: `/subscriptions/{user}/{device}.{json,txt,opml}`.
fn simple_route(ud: &mut UserData, method: &Method, device_file: &str, body: &str) -> Result<Reply, Reply> {
    let (device, format) = device_file.rsplit_once('.').unwrap_or((device_file, "json"));
    let unsupported = || reply(400, json!({ "error": "unsupported format" }));
    Ok(match *method {
        Method::GET => {
            let subs: Vec<String> = current_subs(ud).into_iter().collect();
            match format {
                "json" => reply(200, json!(subs)),
                "txt" => reply(200, Value::String(subs.iter().map(|s| format!("{s}\n")).collect())),
                "opml" => {
                    let outlines: String =
                        subs.iter().map(|s| format!("<outline type=\"rss\" text=\"{s}\" xmlUrl=\"{s}\"/>")).collect();
                    reply(
                        200,
                        Value::String(format!(
                            "<?xml version=\"1.0\"?><opml version=\"2.0\"><head><title>subscriptions</title></head><body>{outlines}</body></opml>"
                        )),
                    )
                }
                _ => unsupported(),
            }
        }
        Method::PUT => {
            let list: Vec<String> = match format {
                "json" => match parse_json(body)? {
                    Value::Array(a) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
                    _ => return Err(reply(400, json!({ "error": "expected a JSON array" }))),
                },
                "txt" => body.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect(),
                _ => return Err(unsupported()),
            };
            put_device_subs(ud, device, &list);
            reply(200, Value::String(String::new()))
        }
        _ => reply(405, json!({ "error": "method not allowed" })),
    })
}

fn ensure_device(ud: &mut UserData, device: &str) {
    ud.devices.entry(device.to_string()).or_insert_with(|| (device.to_string(), "other".to_string()));
}

fn put_device_subs(ud: &mut UserData, device: &str, list: &[String]) {
    ensure_device(ud, device);
    let mine = device_subs(ud, device);
    let want: BTreeSet<String> = list.iter().cloned().collect();
    let t = now();
    for url in want.difference(&mine) {
        ud.subs.push(SubEntry { ts: t, device: device.into(), url: url.clone(), add: true });
    }
    for url in mine.difference(&want) {
        ud.subs.push(SubEntry { ts: t, device: device.into(), url: url.clone(), add: false });
    }
}

/// Changes since `since` from devices other than `device`, folded per URL.
fn fold_changes(ud: &UserData, since: i64, device: Option<&str>, t: i64) -> Value {
    if since == 0 {
        let add: Vec<String> = current_subs(ud).into_iter().collect();
        return json!({ "add": add, "remove": [], "timestamp": t });
    }
    let mut last: BTreeMap<&str, bool> = BTreeMap::new();
    for e in ud.subs.iter().filter(|e| e.ts >= since && Some(e.device.as_str()) != device) {
        last.insert(&e.url, e.add);
    }
    let add: Vec<&str> = last.iter().filter(|(_, a)| **a).map(|(u, _)| *u).collect();
    let remove: Vec<&str> = last.iter().filter(|(_, a)| !**a).map(|(u, _)| *u).collect();
    json!({ "add": add, "remove": remove, "timestamp": t })
}

fn record_sub_changes(ud: &mut UserData, device: &str, v: &Value) -> Result<i64, Reply> {
    let list = |k: &str| -> Result<Vec<String>, Reply> {
        match v.get(k) {
            None | Some(Value::Null) => Ok(Vec::new()),
            Some(Value::Array(a)) => Ok(a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()),
            _ => Err(reply(400, json!({ "error": format!("{k} must be an array") }))),
        }
    };
    let add = list("add")?;
    let remove = list("remove")?;
    if add.iter().any(|u| remove.contains(u)) {
        return Err(reply(400, json!({ "error": "a URL is in both add and remove" })));
    }
    ensure_device(ud, device);
    let t = now();
    for url in add {
        ud.subs.push(SubEntry { ts: t, device: device.into(), url, add: true });
    }
    for url in remove {
        ud.subs.push(SubEntry { ts: t, device: device.into(), url, add: false });
    }
    Ok(t)
}

fn record_actions(ud: &mut UserData, default_device: &str, v: &Value) -> Result<i64, Reply> {
    let list = match v {
        Value::Array(a) => a.clone(),
        Value::Object(o) => o.get("actions").and_then(Value::as_array).cloned().unwrap_or_default(),
        _ => return Err(reply(400, json!({ "error": "expected a list of actions" }))),
    };
    let parsed = list
        .iter()
        .map(normalise_action)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| reply(400, json!({ "error": e })))?;
    let t = now();
    for (device, action) in parsed {
        let device = device.unwrap_or_else(|| default_device.to_string());
        ensure_device(ud, &device);
        ud.actions.push(ActionEntry { ts: t, device, action });
    }
    Ok(t)
}

fn v2_route(ud: &mut UserData, client: &str, method: &Method, segs: &[&str], query: Option<&str>, body: &str) -> Result<Reply, Reply> {
    Ok(match (method, segs) {
        (&Method::GET, ["devices", _]) => {
            let n = current_subs(ud).len();
            let list: Vec<Value> = ud
                .devices
                .iter()
                .map(|(id, (caption, kind))| json!({ "id": id, "caption": caption, "type": kind, "subscriptions": n }))
                .collect();
            reply(200, json!(list))
        }
        (&Method::POST, ["devices", _, device_file]) => {
            let device = strip_ext(device_file, ".json");
            let v = parse_json(body).unwrap_or(json!({}));
            ensure_device(ud, device);
            let entry = ud.devices.get_mut(device).unwrap();
            if let Some(c) = v.get("caption").and_then(Value::as_str) {
                entry.0 = c.to_string();
            }
            if let Some(k) = v.get("type").and_then(Value::as_str) {
                entry.1 = k.to_string();
            }
            reply(200, json!({}))
        }
        (&Method::GET, ["subscriptions", _]) => {
            let subs: Vec<String> = current_subs(ud).into_iter().collect();
            reply(200, json!(subs))
        }
        (&Method::GET, ["subscriptions", _, device_file]) => {
            let device = strip_ext(device_file, ".json");
            let s = since(query)?;
            ensure_device(ud, device);
            let t = now();
            ud.last_download.insert(format!("subs:{device}"), t);
            reply(200, fold_changes(ud, s, Some(device), t))
        }
        (&Method::POST, ["subscriptions", _, device_file]) => {
            let device = strip_ext(device_file, ".json");
            let t = record_sub_changes(ud, device, &parse_json(body)?)?;
            let cursor = ud.last_download.get(&format!("subs:{device}")).copied().unwrap_or(t);
            reply(200, json!({ "timestamp": cursor, "update_urls": [] }))
        }
        (&Method::PUT, ["subscriptions", _, device_file]) => {
            let device = strip_ext(device_file, ".json");
            match parse_json(body)? {
                Value::Array(a) => {
                    let list: Vec<String> = a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
                    put_device_subs(ud, device, &list);
                    reply(200, json!({}))
                }
                _ => reply(400, json!({ "error": "expected a JSON array" })),
            }
        }
        (&Method::GET, ["episodes", _]) => {
            let s = since(query)?;
            let t = now();
            let podcast = query_param(query, "podcast");
            ud.last_download.insert(format!("eps:{client}"), t);
            let actions: Vec<Value> = ud
                .actions
                .iter()
                .filter(|a| a.ts >= s)
                .filter(|a| podcast.as_deref().is_none_or(|p| a.action["podcast"] == p))
                .map(served_action)
                .collect();
            reply(200, json!({ "actions": actions, "timestamp": t }))
        }
        (&Method::POST, ["episodes", _]) => {
            let t = record_actions(ud, "default", &parse_json(body)?)?;
            let cursor = ud.last_download.get(&format!("eps:{client}")).copied().unwrap_or(t);
            reply(200, json!({ "timestamp": cursor, "update_urls": [] }))
        }
        _ => reply(404, json!({ "error": "not found" })),
    })
}
