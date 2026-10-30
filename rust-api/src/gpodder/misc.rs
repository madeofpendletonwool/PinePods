// Settings, device sync groups, and the parts of the API Pinepods doesn't implement
// (directory, lists, favorites, suggestions), which answer with valid empty lists.

use std::collections::HashMap;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::auth::authenticate;
use super::db::SettingsKey;
use super::{GResult, GpodderError, NameExt};
use crate::AppState;

const MAX_SETTINGS: usize = 100;

async fn settings_key<'a>(state: &AppState, user_id: i32, scope: &'a str, q: &'a HashMap<String, String>) -> GResult<SettingsKey<'a>> {
    let param = |k: &str| q.get(k).map(|v| v.trim()).filter(|v| !v.is_empty());
    let mut key = SettingsKey { scope, device_id: None, podcast: None, episode: None };
    match scope {
        "account" => {}
        "device" => {
            let name = param("device").ok_or_else(|| GpodderError::bad_request("device is required"))?;
            key.device_id = Some(state.db_pool.gpodder_ensure_device(user_id, name).await?.id);
        }
        "podcast" => key.podcast = Some(param("podcast").ok_or_else(|| GpodderError::bad_request("podcast is required"))?),
        "episode" => {
            key.podcast = Some(param("podcast").ok_or_else(|| GpodderError::bad_request("podcast is required"))?);
            key.episode = Some(param("episode").ok_or_else(|| GpodderError::bad_request("episode is required"))?);
        }
        _ => return Err(GpodderError::bad_request("scope must be account, device, podcast or episode")),
    }
    Ok(key)
}

async fn settings_map(state: &AppState, user_id: i32, key: &SettingsKey<'_>) -> GResult<Value> {
    let mut map = Map::new();
    for (k, v) in state.db_pool.gpodder_settings(user_id, key).await? {
        let v = v.map(|s| serde_json::from_str(&s).unwrap_or(Value::String(s))).unwrap_or(Value::Null);
        map.insert(k, v);
    }
    Ok(Value::Object(map))
}

/// GET /api/2/settings/{user}/{scope}.json?device=&podcast=&episode=
pub(crate) async fn get_settings(
    State(state): State<AppState>,
    Path((user, scope)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let scope = NameExt::json_only(&scope)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let key = settings_key(&state, auth.user_id, &scope, &q).await?;
    Ok(Json(settings_map(&state, auth.user_id, &key).await?))
}

#[derive(Deserialize)]
struct SettingsBody {
    #[serde(default)]
    set: Map<String, Value>,
    #[serde(default)]
    remove: Vec<String>,
}

/// POST /api/2/settings/{user}/{scope}.json with `{"set": {...}, "remove": [...]}`; answers
/// with the settings after the change.
pub(crate) async fn save_settings(
    State(state): State<AppState>,
    Path((user, scope)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
    body: Bytes,
) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let scope = NameExt::json_only(&scope)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let body: SettingsBody = serde_json::from_slice(&body).map_err(|_| GpodderError::bad_request("expected {\"set\": {...}, \"remove\": [...]}"))?;
    if body.set.len() + body.remove.len() > MAX_SETTINGS {
        return Err(GpodderError::bad_request("too many settings in one request"));
    }
    if body.set.keys().chain(body.remove.iter()).any(|k| k.is_empty() || k.len() > 255) {
        return Err(GpodderError::bad_request("invalid setting key"));
    }
    let key = settings_key(&state, auth.user_id, &scope, &q).await?;
    let set: Vec<(String, String)> = body.set.into_iter().map(|(k, v)| (k, v.to_string())).collect();
    state.db_pool.gpodder_save_settings(auth.user_id, &key, &set, &body.remove).await?;
    Ok(Json(settings_map(&state, auth.user_id, &key).await?))
}

/// All of a user's devices share one library, so they're always one synchronized group.
async fn sync_groups(state: &AppState, headers: &HeaderMap, user: &str) -> GResult<Json<Value>> {
    let user = NameExt::json_only(user)?;
    let auth = authenticate(state, headers, &user).await?;
    let names: Vec<String> = state.db_pool.gpodder_devices(auth.user_id).await?.into_iter().map(|d| d.name).collect();
    Ok(Json(if names.len() > 1 {
        json!({ "synchronized": [names], "not-synchronized": [] })
    } else {
        json!({ "synchronized": [], "not-synchronized": names })
    }))
}

/// GET /api/2/sync-devices/{user}.json
pub(crate) async fn sync_devices(State(state): State<AppState>, Path(user): Path<String>, headers: HeaderMap) -> GResult<Json<Value>> {
    sync_groups(&state, &headers, &user).await
}

/// POST /api/2/sync-devices/{user}.json: accepted and ignored (the grouping can't change).
pub(crate) async fn update_sync_devices(State(state): State<AppState>, Path(user): Path<String>, headers: HeaderMap) -> GResult<Json<Value>> {
    sync_groups(&state, &headers, &user).await
}

/// Lists and favorites of a user: always empty.
pub(crate) async fn empty_for_user(State(state): State<AppState>, Path(user): Path<String>, headers: HeaderMap) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    authenticate(&state, &headers, &user).await?;
    Ok(Json(json!([])))
}

/// GET /suggestions/{count}.{format}: always empty.
pub(crate) async fn empty_suggestions(State(state): State<AppState>, headers: HeaderMap) -> GResult<Json<Value>> {
    // The username isn't in the path; any enabled user's Basic credentials will do.
    let user = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split_once(' '))
        .and_then(|(_, b64)| base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64.trim()).ok())
        .and_then(|raw| String::from_utf8(raw).ok())
        .and_then(|s| s.split_once(':').map(|(u, _)| u.to_string()))
        .ok_or_else(GpodderError::unauthorized)?;
    authenticate(&state, &headers, &user).await?;
    Ok(Json(json!([])))
}

/// Directory endpoints (toplist, search, tags): always empty.
pub(crate) async fn empty_list() -> Json<Value> {
    Json(json!([]))
}
