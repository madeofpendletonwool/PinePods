// Subscriptions: v2 changes (GET/POST), whole-list PUT, the all-subscriptions list and the
// Simple API.

use std::collections::{HashMap, HashSet};

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use quick_xml::events::Event;
use serde::Deserialize;
use serde_json::{json, Value};

use super::auth::{authenticate, Authed};
use super::{clean_urls, download_cursor, is_http, parse_since, Format, GResult, GpodderError, NameExt};
use crate::services::sync::{apply_subscription_changes, SyncOrigin};
use crate::AppState;

/// GET /api/2/subscriptions/{user}/{device}.json?since=: `since=0` is the whole library as
/// `add`; otherwise other devices' changes since then, folded to the last action per feed.
pub(crate) async fn changes(
    State(state): State<AppState>,
    Path((user, device)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let device = NameExt::json_only(&device)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let since = parse_since(q.get("since").map(String::as_str))?;
    let db = &state.db_pool;
    let device = db.gpodder_ensure_device(auth.user_id, &device).await?;
    let timestamp = download_cursor(db).await?;
    let (add, remove): (Vec<String>, Vec<String>) = if since == 0 {
        (db.gpodder_library(auth.user_id).await?, Vec::new())
    } else {
        let changes = db.gpodder_subscription_changes(auth.user_id, since, device.id).await?;
        let (add, remove): (Vec<_>, Vec<_>) = changes.into_iter().partition(|(_, a)| *a);
        (add.into_iter().map(|(u, _)| u).collect(), remove.into_iter().map(|(u, _)| u).collect())
    };
    db.gpodder_set_subs_cursor(device.id, timestamp).await?;
    Ok(Json(json!({ "add": add, "remove": remove, "timestamp": timestamp })))
}

#[derive(Deserialize)]
struct ChangesBody {
    #[serde(default)]
    add: Option<Vec<String>>,
    #[serde(default)]
    remove: Option<Vec<String>>,
}

/// Log a device's changes, update its list and apply them (adds are queued imports). Adds that
/// aren't http(s) are refused and reported back as `[url, ""]` in `update_urls`.
async fn record(state: &AppState, user_id: i32, device_id: i32, add: Vec<String>, remove: Vec<String>) -> GResult<Vec<(String, String)>> {
    let (add, refused): (Vec<String>, Vec<String>) = add.into_iter().partition(|u| is_http(u));
    let remove: Vec<String> = remove.into_iter().filter(|u| is_http(u)).collect();
    state.db_pool.gpodder_record_subscriptions(user_id, device_id, &add, &remove).await?;
    apply_subscription_changes(&state.db_pool, &state.task_spawner, user_id, &add, &remove, SyncOrigin::Client).await?;
    Ok(refused.into_iter().map(|u| (u, String::new())).collect())
}

/// POST /api/2/subscriptions/{user}/{device}.json with `{"add": [...], "remove": [...]}`. The
/// response timestamp is the cursor this device last downloaded with, so a change another
/// device made in between still reaches it.
pub(crate) async fn upload_changes(
    State(state): State<AppState>,
    Path((user, device)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let device = NameExt::json_only(&device)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let body: ChangesBody = serde_json::from_slice(&body).map_err(|_| GpodderError::bad_request("expected {\"add\": [...], \"remove\": [...]}"))?;
    let add = clean_urls(&body.add.unwrap_or_default());
    let remove = clean_urls(&body.remove.unwrap_or_default());
    let removing: HashSet<&String> = remove.iter().collect();
    if add.iter().any(|u| removing.contains(u)) {
        return Err(GpodderError::bad_request("a URL is in both add and remove"));
    }
    let db = &state.db_pool;
    let device = db.gpodder_ensure_device(auth.user_id, &device).await?;
    let update_urls = record(&state, auth.user_id, device.id, add, remove).await?;
    let timestamp = match device.subs_cursor {
        Some(c) => c,
        None => download_cursor(db).await?,
    };
    Ok(Json(json!({ "timestamp": timestamp, "update_urls": update_urls })))
}

/// Replace a device's list: only what differs from what this device reported before changes,
/// so other devices' and Pinepods' own subscriptions are never removed by it.
async fn put_device_list(state: &AppState, auth: &Authed, device: &str, urls: Vec<String>) -> GResult<()> {
    let db = &state.db_pool;
    let device = db.gpodder_ensure_device(auth.user_id, device).await?;
    let current = db.gpodder_device_subscriptions(device.id).await?;
    let want = clean_urls(&urls);
    let wanted: HashSet<&String> = want.iter().collect();
    let add: Vec<String> = want.iter().filter(|u| !current.contains(*u)).cloned().collect();
    let mut remove: Vec<String> = current.iter().filter(|u| !wanted.contains(u)).cloned().collect();
    remove.sort();
    record(state, auth.user_id, device.id, add, remove).await?;
    Ok(())
}

/// PUT /api/2/subscriptions/{user}/{device}.json with a JSON list of feed URLs.
pub(crate) async fn put_list(
    State(state): State<AppState>,
    Path((user, device)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let device = NameExt::json_only(&device)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let urls: Vec<String> = serde_json::from_slice(&body).map_err(|_| GpodderError::bad_request("expected a JSON list of URLs"))?;
    put_device_list(&state, &auth, &device, urls).await?;
    Ok(Json(json!({})))
}

/// GET /api/2/subscriptions/{user}.json: every feed in the library.
pub(crate) async fn all(State(state): State<AppState>, Path(user): Path<String>, headers: HeaderMap) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let auth = authenticate(&state, &headers, &user).await?;
    Ok(Json(json!(state.db_pool.gpodder_library(auth.user_id).await?)))
}

// ---- Simple API ---------------------------------------------------------------------------

fn jsonp_callback(q: &HashMap<String, String>) -> GResult<String> {
    let cb = q.get("jsonp").map(String::as_str).unwrap_or("callback");
    if cb.is_empty() || !cb.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$')) {
        return Err(GpodderError::bad_request("invalid jsonp callback"));
    }
    Ok(cb.to_string())
}

fn escape(s: &str) -> String {
    quick_xml::escape::escape(s).into_owned()
}

/// A list of feeds in a Simple API format.
async fn render(state: &AppState, user_id: i32, format: Format, q: &HashMap<String, String>, urls: Vec<String>) -> GResult<Response> {
    Ok(match format {
        Format::Json => Json(json!(urls)).into_response(),
        Format::Jsonp => {
            let body = format!("{}({})", jsonp_callback(q)?, json!(urls));
            ([(header::CONTENT_TYPE, "application/javascript; charset=utf-8")], body).into_response()
        }
        Format::Txt => {
            let body: String = urls.iter().map(|u| format!("{u}\n")).collect();
            ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response()
        }
        Format::Opml | Format::Xml => {
            let info = state.db_pool.gpodder_podcast_info(user_id).await?;
            let mut body = String::from(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<opml version=\"2.0\">\n  <head><title>PinePods subscriptions</title></head>\n  <body>\n",
            );
            for url in &urls {
                let title = info.get(url).and_then(|i| i.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| url.clone());
                body.push_str(&format!(
                    "    <outline text=\"{t}\" title=\"{t}\" type=\"rss\" xmlUrl=\"{u}\"/>\n",
                    t = escape(&title),
                    u = escape(url)
                ));
            }
            body.push_str("  </body>\n</opml>\n");
            ([(header::CONTENT_TYPE, "text/x-opml; charset=utf-8")], body).into_response()
        }
    })
}

/// GET /subscriptions/{user}.{format}
pub(crate) async fn simple_all(
    State(state): State<AppState>,
    Path(user): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> GResult<Response> {
    let user = NameExt::parse(&user);
    let auth = authenticate(&state, &headers, &user.name).await?;
    let urls = state.db_pool.gpodder_library(auth.user_id).await?;
    render(&state, auth.user_id, user.format.unwrap_or(Format::Json), &q, urls).await
}

/// GET /subscriptions/{user}/{device}.{format}: the library (every device shares it).
pub(crate) async fn simple_get(
    State(state): State<AppState>,
    Path((user, device)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> GResult<Response> {
    let device = NameExt::parse(&device);
    let auth = authenticate(&state, &headers, &user).await?;
    let urls = state.db_pool.gpodder_library(auth.user_id).await?;
    render(&state, auth.user_id, device.format.unwrap_or(Format::Json), &q, urls).await
}

fn opml_urls(body: &str) -> GResult<Vec<String>> {
    let mut reader = quick_xml::Reader::from_str(body);
    let mut urls = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if e.local_name().as_ref() == "outline" => {
                let url = e
                    .attributes()
                    .flatten()
                    .find(|a| a.key.as_ref() == "xmlUrl")
                    .and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.trim().to_string()));
                if let Some(url) = url.filter(|u| !u.is_empty()) {
                    urls.push(url);
                }
            }
            Ok(Event::Eof) => return Ok(urls),
            Err(_) => return Err(GpodderError::bad_request("invalid OPML")),
            _ => {}
        }
    }
}

/// PUT /subscriptions/{user}/{device}.{json,txt,opml}: replace the device's list.
pub(crate) async fn simple_put(
    State(state): State<AppState>,
    Path((user, device)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> GResult<Response> {
    let device = NameExt::parse(&device);
    let auth = authenticate(&state, &headers, &user).await?;
    let text = std::str::from_utf8(&body).map_err(|_| GpodderError::bad_request("body is not UTF-8"))?;
    let urls: Vec<String> = match device.format.unwrap_or(Format::Json) {
        Format::Json => serde_json::from_str(text).map_err(|_| GpodderError::bad_request("expected a JSON list of URLs"))?,
        Format::Txt => text.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect(),
        Format::Opml | Format::Xml => opml_urls(text)?,
        Format::Jsonp => return Err(GpodderError::bad_request("unsupported format")),
    };
    put_device_list(&state, &auth, &device.name, urls).await?;
    Ok(([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "").into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opml_outlines() {
        let body = r#"<?xml version="1.0"?><opml version="2.0"><body>
            <outline text="A &amp; B" xmlUrl="https://a.example/feed?x=1&amp;y=2"/>
            <outline text="folder"><outline type="rss" xmlUrl="https://b.example/rss"></outline></outline>
            <outline text="no url"/>
        </body></opml>"#;
        assert_eq!(opml_urls(body).unwrap(), vec!["https://a.example/feed?x=1&y=2".to_string(), "https://b.example/rss".to_string()]);
    }

    #[test]
    fn jsonp_callback_names() {
        let mut q = HashMap::new();
        assert_eq!(jsonp_callback(&q).unwrap(), "callback");
        q.insert("jsonp".to_string(), "my.cb_1".to_string());
        assert_eq!(jsonp_callback(&q).unwrap(), "my.cb_1");
        q.insert("jsonp".to_string(), "alert(1)".to_string());
        assert!(jsonp_callback(&q).is_err());
        assert_eq!(escape("a&\"<"), "a&amp;&quot;&lt;");
    }
}
