// Episode ids that no longer exist because their episode was merged into the copy another
// subscriber of the same feed had (EpisodeIDAliases). Apps can hold the old ids for a long time
// (offline progress queues, downloads, bookmarked pages), so requests carrying one are pointed at
// the episode it became before they reach a handler.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::middleware::Next;
use axum::response::Response;
use serde_json::Value;
use tracing::{debug, warn};

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::AppState;

// Bodies larger than this (uploads, backups) are passed through untouched.
const MAX_REWRITTEN_BODY: usize = 1024 * 1024;
const ID_KEYS: [&str; 3] = ["episode_id", "episodeid", "episode_ids"];
// Routes that take an episode id in the path: (prefix, segments after the prefix, index of the id).
const PATH_IDS: [(&str, usize, usize); 5] = [
    ("/api/data/stream/", 1, 0),
    ("/api/data/share_episode/", 1, 0),
    ("/api/data/people/episode/", 1, 0),
    ("/api/data/episode_collections/", 2, 1),
    ("/api/episodes/", 2, 0),
];

/// Whether this server has merged episode ids at all, remembered once known. Aliases are created
/// by migrations (and come back with a restored backup, which resets this).
#[derive(Clone, Default)]
pub struct MergedEpisodes(Arc<AtomicU8>);

const UNKNOWN: u8 = 0;
const NONE: u8 = 1;
const SOME: u8 = 2;

impl MergedEpisodes {
    pub fn reset(&self) {
        self.0.store(UNKNOWN, Ordering::Relaxed);
    }

    async fn any(&self, db: &DatabasePool) -> bool {
        match self.0.load(Ordering::Relaxed) {
            NONE => return false,
            SOME => return true,
            _ => {}
        }
        let found = match db {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar::<_, i32>(r#"SELECT 1 FROM "EpisodeIDAliases" LIMIT 1"#).fetch_optional(pool).await
            }
            DatabasePool::MySQL(pool) => sqlx::query_scalar::<_, i32>("SELECT 1 FROM EpisodeIDAliases LIMIT 1").fetch_optional(pool).await,
        };
        match found {
            Ok(row) => {
                self.0.store(if row.is_some() { SOME } else { NONE }, Ordering::Relaxed);
                row.is_some()
            }
            Err(e) => {
                warn!("Checking for merged episode ids failed: {}", e);
                false
            }
        }
    }
}

/// The episode each of `ids` became, for the ids that were merged away.
pub async fn current_ids(merged: &MergedEpisodes, db: &DatabasePool, ids: &[i32]) -> AppResult<HashMap<i32, i32>> {
    if ids.is_empty() || !merged.any(db).await {
        return Ok(HashMap::new());
    }
    let rows: Vec<(i32, i32)> = match db {
        DatabasePool::Postgres(pool) => {
            sqlx::query_as(r#"SELECT oldepisodeid, episodeid FROM "EpisodeIDAliases" WHERE oldepisodeid = ANY($1)"#)
                .bind(ids)
                .fetch_all(pool)
                .await?
        }
        DatabasePool::MySQL(pool) => {
            let marks = vec!["?"; ids.len()].join(", ");
            let sql = format!("SELECT OldEpisodeID, EpisodeID FROM EpisodeIDAliases WHERE OldEpisodeID IN ({marks})");
            let mut q = sqlx::query_as(sqlx::AssertSqlSafe(sql));
            for id in ids {
                q = q.bind(id);
            }
            q.fetch_all(pool).await?
        }
    };
    Ok(rows.into_iter().collect())
}

fn is_youtube(value: Option<&Value>) -> bool {
    matches!(value, Some(Value::Bool(true))) || matches!(value, Some(Value::String(s)) if s == "true")
}

fn collect_json(value: &Value, out: &mut Vec<i32>) {
    match value {
        Value::Object(map) => {
            let youtube = is_youtube(map.get("is_youtube"));
            for (key, v) in map {
                if !youtube && ID_KEYS.contains(&key.as_str()) {
                    match v {
                        Value::Number(n) => out.extend(n.as_i64().and_then(|n| i32::try_from(n).ok())),
                        Value::Array(items) => out.extend(items.iter().filter_map(|i| i.as_i64().and_then(|n| i32::try_from(n).ok()))),
                        _ => {}
                    }
                } else {
                    collect_json(v, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect_json(i, out)),
        _ => {}
    }
}

fn rewrite_json(value: &mut Value, map: &HashMap<i32, i32>) {
    let swap = |v: &mut Value| {
        if let Some(new) = v.as_i64().and_then(|n| i32::try_from(n).ok()).and_then(|n| map.get(&n)) {
            *v = Value::from(*new);
        }
    };
    match value {
        Value::Object(obj) => {
            let youtube = is_youtube(obj.get("is_youtube"));
            for (key, v) in obj.iter_mut() {
                if !youtube && ID_KEYS.contains(&key.as_str()) {
                    match v {
                        Value::Array(items) => items.iter_mut().for_each(swap),
                        other => swap(other),
                    }
                } else {
                    rewrite_json(v, map);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|i| rewrite_json(i, map)),
        _ => {}
    }
}

fn query_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (p.to_string(), String::new()),
        })
        .collect()
}

/// (path segment index, id) of an episode id in the path, for the routes that take one.
fn path_id(path: &str) -> Option<(usize, i32)> {
    for (prefix, segments, index) in PATH_IDS {
        let Some(rest) = path.strip_prefix(prefix) else { continue };
        let parts: Vec<&str> = rest.split('/').collect();
        if parts.len() != segments || (prefix == "/api/episodes/" && parts[1] != "download") {
            continue;
        }
        let id = parts[index].parse().ok()?;
        return Some((prefix.matches('/').count() - 1 + index, id));
    }
    None
}

/// Point episode ids in the path, query string and JSON body of a request at the episode a
/// merged copy became. Does nothing on servers that never merged episodes.
pub async fn follow_merged_episodes(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let db = &state.db_pool;
    if !state.merged_episodes.any(db).await {
        return next.run(req).await;
    }
    let (mut parts, body) = req.into_parts();

    let path = parts.uri.path().to_string();
    let query = parts.uri.query().map(query_pairs).unwrap_or_default();
    let query_youtube = query.iter().any(|(k, v)| k == "is_youtube" && v == "true");
    let mut ids: Vec<i32> = Vec::new();
    let in_path = if query_youtube { None } else { path_id(&path) };
    ids.extend(in_path.map(|(_, id)| id));
    if !query_youtube {
        ids.extend(query.iter().filter(|(k, _)| ID_KEYS.contains(&k.as_str())).filter_map(|(_, v)| v.parse::<i32>().ok()));
    }

    let json = parts.headers.get(CONTENT_TYPE).and_then(|v| v.to_str().ok()).is_some_and(|v| v.contains("json"));
    let small = axum::body::HttpBody::size_hint(&body).upper().is_some_and(|n| n <= MAX_REWRITTEN_BODY as u64);
    let (body, mut parsed) = if json && small {
        match to_bytes(body, MAX_REWRITTEN_BODY).await {
            Ok(bytes) => {
                let value = serde_json::from_slice::<Value>(&bytes).ok();
                if let Some(v) = &value {
                    collect_json(v, &mut ids);
                }
                (Body::from(bytes), value)
            }
            Err(_) => (Body::empty(), None),
        }
    } else {
        (body, None)
    };

    let map = match current_ids(&state.merged_episodes, db, &ids).await {
        Ok(map) => map,
        Err(e) => {
            warn!("Looking up merged episode ids failed: {}", e);
            HashMap::new()
        }
    };
    if map.is_empty() {
        return next.run(Request::from_parts(parts, body)).await;
    }
    debug!("Request {} uses merged episode ids {:?}", path, map);

    let mut new_path = path.clone();
    if let Some((segment, id)) = in_path {
        if let Some(new) = map.get(&id) {
            let mut segs: Vec<String> = path.split('/').map(str::to_string).collect();
            if let Some(s) = segs.get_mut(segment + 1) {
                *s = new.to_string();
            }
            new_path = segs.join("/");
        }
    }
    let new_query = (!query.is_empty()).then(|| {
        query
            .iter()
            .map(|(k, v)| {
                let v = match (query_youtube, ID_KEYS.contains(&k.as_str()), v.parse::<i32>()) {
                    (false, true, Ok(id)) => map.get(&id).map(i32::to_string).unwrap_or_else(|| v.clone()),
                    _ => v.clone(),
                };
                if v.is_empty() && !k.is_empty() { k.clone() } else { format!("{k}={v}") }
            })
            .collect::<Vec<_>>()
            .join("&")
    });
    let uri = match &new_query {
        Some(q) => format!("{new_path}?{q}"),
        None => new_path,
    };
    if let Ok(uri) = uri.parse() {
        parts.uri = uri;
    }

    let body = match parsed.as_mut() {
        Some(value) => {
            rewrite_json(value, &map);
            let bytes = serde_json::to_vec(value).unwrap_or_default();
            parts.headers.insert(CONTENT_LENGTH, bytes.len().into());
            Body::from(bytes)
        }
        None => body,
    };
    next.run(Request::from_parts(parts, body)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_ids_in_paths() {
        assert_eq!(path_id("/api/data/stream/42"), Some((3, 42)));
        assert_eq!(path_id("/api/episodes/42/download"), Some((2, 42)));
        assert_eq!(path_id("/api/data/episode_collections/7/42"), Some((4, 42)));
        assert_eq!(path_id("/api/episodes/42/other"), None);
        assert_eq!(path_id("/api/data/return_pods/1"), None);
    }

    #[test]
    fn rewrites_podcast_episode_ids_but_not_videos() {
        let mut body = serde_json::json!({"episode_id": 5, "user_id": 5, "items": [{"episode_ids": [5, 6]}], "video": {"episode_id": 5, "is_youtube": true}});
        let mut ids = Vec::new();
        collect_json(&body, &mut ids);
        assert_eq!(ids, vec![5, 5, 6]);
        rewrite_json(&mut body, &HashMap::from([(5, 9)]));
        assert_eq!(body, serde_json::json!({"episode_id": 9, "user_id": 5, "items": [{"episode_ids": [9, 6]}], "video": {"episode_id": 5, "is_youtube": true}}));
    }
}
