// Devices and the per-device updates endpoint.

use std::collections::HashMap;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde_json::{json, Value};

use super::auth::authenticate;
use super::db::PodcastInfo;
use super::{download_cursor, parse_since, GResult, GpodderError, NameExt};
use crate::AppState;

/// GET /api/2/devices/{user}.json. Every device shares the library, so each reports its size.
pub(crate) async fn list(State(state): State<AppState>, Path(user): Path<String>, headers: HeaderMap) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let library = state.db_pool.gpodder_library(auth.user_id).await?.len();
    let devices: Vec<Value> = state
        .db_pool
        .gpodder_devices(auth.user_id)
        .await?
        .into_iter()
        .map(|d| {
            json!({
                "id": d.name,
                "caption": d.caption.filter(|c| !c.is_empty()).unwrap_or_else(|| d.name.clone()),
                "type": d.kind.filter(|k| !k.is_empty()).unwrap_or_else(|| "other".into()),
                "subscriptions": library,
            })
        })
        .collect();
    Ok(Json(Value::Array(devices)))
}

/// POST /api/2/devices/{user}/{device}.json with `{"caption": ..., "type": ...}`.
pub(crate) async fn update(
    State(state): State<AppState>,
    Path((user, device)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let device = NameExt::json_only(&device)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let data: Value = if body.iter().all(u8::is_ascii_whitespace) {
        json!({})
    } else {
        serde_json::from_slice(&body).map_err(|_| GpodderError::bad_request("invalid JSON"))?
    };
    let field = |k: &str| data.get(k).and_then(Value::as_str).map(str::trim);
    state.db_pool.gpodder_update_device(auth.user_id, &device, field("caption"), field("type").filter(|t| !t.is_empty())).await?;
    Ok(Json(json!({})))
}

pub(crate) fn podcast_object(url: &str, info: Option<&PodcastInfo>) -> Value {
    json!({
        "url": url,
        "title": info.and_then(|i| i.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| url.to_string()),
        "description": info.and_then(|i| i.description.clone()).unwrap_or_default(),
        "author": info.and_then(|i| i.author.clone()).unwrap_or_default(),
        "subscribers": 1,
        "logo_url": info.and_then(|i| i.logo_url.clone()).unwrap_or_default(),
        "website": info.and_then(|i| i.website.clone()).unwrap_or_default(),
        "mygpo_link": "",
    })
}

/// GET /api/2/updates/{user}/{device}.json?since=&include_actions=: subscription changes from
/// other devices plus episodes published since `since` in the user's podcasts.
pub(crate) async fn updates(
    State(state): State<AppState>,
    Path((user, device)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let device = NameExt::json_only(&device)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let since = parse_since(q.get("since").map(String::as_str))?;
    let include_actions = q.get("include_actions").is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1");
    let db = &state.db_pool;

    let device = db.gpodder_ensure_device(auth.user_id, &device).await?;
    let timestamp = download_cursor(db).await?;
    let info = db.gpodder_podcast_info(auth.user_id).await?;
    let (add, remove): (Vec<Value>, Vec<String>) = if since == 0 {
        let library = db.gpodder_library(auth.user_id).await?;
        (library.iter().map(|u| podcast_object(u, info.get(u))).collect(), Vec::new())
    } else {
        let changes = db.gpodder_subscription_changes(auth.user_id, since, device.id).await?;
        (
            changes.iter().filter(|(_, a)| *a).map(|(u, _)| podcast_object(u, info.get(u))).collect(),
            changes.into_iter().filter(|(_, a)| !*a).map(|(u, _)| u).collect(),
        )
    };

    let episodes = db.gpodder_new_episodes(auth.user_id, since).await?;
    let status = if include_actions {
        let ids: Vec<i32> = episodes.iter().map(|e| e.episode_id).collect();
        db.gpodder_latest_actions(auth.user_id, &ids).await?
    } else {
        HashMap::new()
    };
    let updates: Vec<Value> = episodes
        .into_iter()
        .map(|e| {
            let mut v = json!({
                "title": e.title.unwrap_or_default(),
                "url": e.url,
                "podcast_title": e.podcast_title.unwrap_or_default(),
                "podcast_url": e.podcast_url,
                "description": e.description.unwrap_or_default(),
                "website": e.website.unwrap_or_default(),
                "mygpo_link": "",
                "released": e.released.map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string()),
            });
            if include_actions {
                v["status"] = json!(status.get(&e.episode_id).cloned().unwrap_or_else(|| "new".into()));
            }
            v
        })
        .collect();
    Ok(Json(json!({ "add": add, "remove": remove, "updates": updates, "timestamp": timestamp })))
}
