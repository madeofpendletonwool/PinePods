// Episode actions: download since a cursor, upload a batch.

use std::collections::HashMap;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::HeaderMap,
    Json,
};
use serde_json::{json, Value};
use tracing::{debug, error};

use super::auth::authenticate;
use super::db::StoredAction;
use super::{download_cursor, parse_since, GResult, GpodderError, NameExt};
use crate::services::sync::{apply::apply_resolved, resolve, ActionKind, EpisodeAction, ResolveKey, SyncOrigin};
use crate::AppState;

fn flag(q: &HashMap<String, String>, key: &str) -> bool {
    q.get(key).is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1")
}

/// Newest action per episode by action time (ties: the later upload).
fn aggregate(actions: Vec<StoredAction>) -> Vec<StoredAction> {
    let mut newest: HashMap<(String, String), StoredAction> = HashMap::new();
    for a in actions {
        let key = (a.action.podcast_url.clone(), a.action.episode_url.clone());
        let keep = newest.get(&key).is_none_or(|cur| (a.action.timestamp, a.id) >= (cur.action.timestamp, cur.id));
        if keep {
            newest.insert(key, a);
        }
    }
    let mut out: Vec<StoredAction> = newest.into_values().collect();
    out.sort_by_key(|a| a.id);
    out
}

/// GET /api/2/episodes/{user}.json?since=&podcast=&device=&aggregated=
pub(crate) async fn download(
    State(state): State<AppState>,
    Path(user): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let since = parse_since(q.get("since").map(String::as_str))?;
    let podcast = q.get("podcast").map(|p| p.trim()).filter(|p| !p.is_empty());
    let db = &state.db_pool;

    let device_id = match q.get("device").map(|d| d.trim()).filter(|d| !d.is_empty()) {
        Some(name) => match db.gpodder_device_id(auth.user_id, name).await? {
            Some(id) => Some(id),
            None => return Ok(Json(json!({ "actions": [], "timestamp": download_cursor(db).await? }))),
        },
        None => None,
    };
    let timestamp = download_cursor(db).await?;
    let mut actions = db.gpodder_episode_actions(auth.user_id, since, podcast, device_id).await?;
    if flag(&q, "aggregated") {
        actions = aggregate(actions);
    }
    let out: Vec<Value> = actions
        .iter()
        .filter_map(|a| a.action.outbound(a.known_duration))
        .map(|a| serde_json::to_value(a).unwrap_or(Value::Null))
        .collect();
    if let Some(session) = auth.session {
        db.gpodder_set_episodes_cursor(session, timestamp).await?;
    }
    Ok(Json(json!({ "actions": out, "timestamp": timestamp })))
}

/// `started`/`position`/`total` must fit the log's INT columns; a client sending more gets a 400
/// for the batch rather than a silently clamped value.
fn check_range(action: &Value) -> GResult<()> {
    for key in ["started", "position", "total"] {
        let n = match action.get(key) {
            Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
            Some(Value::String(s)) => s.trim().parse::<f64>().ok().map(|f| f as i64),
            _ => None,
        };
        if n.is_some_and(|n| n < i32::MIN as i64 || n > i32::MAX as i64) {
            return Err(GpodderError::bad_request(format!("{key} out of range")));
        }
    }
    Ok(())
}

/// The actions in an upload body (a list, or `{"actions": [...]}`). Actions Pinepods doesn't
/// track (e.g. `flattr`) are skipped; anything malformed rejects the whole batch.
fn parse_upload(body: &[u8]) -> GResult<Vec<EpisodeAction>> {
    let value: Value = serde_json::from_slice(body).map_err(|_| GpodderError::bad_request("invalid JSON"))?;
    let list = match value {
        Value::Array(a) => a,
        Value::Object(mut o) => match o.remove("actions") {
            Some(Value::Array(a)) => a,
            _ => return Err(GpodderError::bad_request("expected a list of episode actions")),
        },
        _ => return Err(GpodderError::bad_request("expected a list of episode actions")),
    };
    let mut out = Vec::with_capacity(list.len());
    for item in list {
        let kind = item.get("action").and_then(Value::as_str).unwrap_or_default();
        if ActionKind::parse(kind).is_none() && !kind.trim().is_empty() {
            debug!("Skipping unsupported episode action {:?}", kind);
            continue;
        }
        check_range(&item)?;
        let action: EpisodeAction = serde_json::from_value(item).map_err(|e| GpodderError::bad_request(format!("invalid episode action: {e}")))?;
        if action.podcast_url.trim().is_empty() || action.episode_url.trim().is_empty() {
            return Err(GpodderError::bad_request("episode action without podcast or episode"));
        }
        out.push(action);
    }
    Ok(out)
}

/// POST /api/2/episodes/{user}.json. The batch is logged in one transaction with the client's
/// action times, then applied to Pinepods. The response timestamp is the cursor this session
/// last downloaded with (the request time without a session or download).
pub(crate) async fn upload(State(state): State<AppState>, Path(user): Path<String>, headers: HeaderMap, body: Bytes) -> GResult<Json<Value>> {
    let user = NameExt::json_only(&user)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let actions = parse_upload(&body)?;
    let db = &state.db_pool;

    let keys: Vec<ResolveKey> = actions.iter().map(ResolveKey::of).collect();
    let resolved = resolve(db, auth.user_id, &keys).await?;
    db.gpodder_record_episode_actions(auth.user_id, &actions, &resolved).await?;
    if let Err(e) = apply_resolved(db, auth.user_id, &actions, &resolved, SyncOrigin::Client).await {
        error!("Applying uploaded episode actions for user {} failed: {}", auth.user_id, e);
        return Err(e.into());
    }

    let cursor = match auth.session {
        Some(session) => db.gpodder_episodes_cursor(session).await?,
        None => None,
    };
    let timestamp = match cursor {
        Some(c) => c,
        None => download_cursor(db).await?,
    };
    Ok(Json(json!({ "timestamp": timestamp, "update_urls": [] })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uploads_skip_untracked_actions_and_reject_bad_ones() {
        let body = json!([
            {"podcast": "https://f/feed", "episode": "https://f/1.mp3", "action": "play", "position": 10},
            {"podcast": "https://f/feed", "episode": "https://f/1.mp3", "action": "flattr"},
        ]);
        assert_eq!(parse_upload(body.to_string().as_bytes()).unwrap().len(), 1);
        let wrapped = json!({"actions": [{"podcast": "p", "episode": "e", "action": "new"}]});
        assert_eq!(parse_upload(wrapped.to_string().as_bytes()).unwrap().len(), 1);
        let too_big = json!([{"podcast": "p", "episode": "e", "action": "play", "position": 3_000_000_000_i64}]);
        assert!(parse_upload(too_big.to_string().as_bytes()).is_err());
        let no_episode = json!([{"podcast": "p", "episode": "", "action": "new"}]);
        assert!(parse_upload(no_episode.to_string().as_bytes()).is_err());
        assert!(parse_upload(b"{}").is_err());
    }
}
