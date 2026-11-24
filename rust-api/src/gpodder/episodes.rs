// Episode actions: download since a cursor, upload a batch.

use std::collections::HashMap;

use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{header, HeaderMap},
    response::{IntoResponse, Response},
    Json,
};
use futures::stream::{self, StreamExt};
use serde_json::{json, Value};
use tracing::{debug, error, warn};

use super::auth::authenticate;
use super::db::{ActionFilter, ActionPage, Collapse};
use super::{download_cursor, parse_since, GResult, GpodderError, NameExt};
use crate::services::sync::{apply::apply_resolved, resolve, ActionKind, EpisodeAction, ResolveKey, SyncOrigin};
use crate::database::DatabasePool;
use crate::AppState;

fn flag(q: &HashMap<String, String>, key: &str) -> bool {
    q.get(key).is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1")
}

/// Log rows read per database query while streaming a download.
const PAGE_ROWS: i64 = 2000;

/// GET /api/2/episodes/{user}.json?since=&podcast=&device=&aggregated=
///
/// The response is written page by page as the log is read, so a large history starts arriving
/// at once instead of after the whole log is loaded (clients give up on a silent connection). A
/// download from `since=0` sends only the newest action of each kind per episode: a client
/// starting from nothing has no use for positions that later ones replaced.
pub(crate) async fn download(
    State(state): State<AppState>,
    Path(user): Path<String>,
    Query(q): Query<HashMap<String, String>>,
    headers: HeaderMap,
) -> GResult<Response> {
    let user = NameExt::json_only(&user)?;
    let auth = authenticate(&state, &headers, &user).await?;
    let since = parse_since(q.get("since").map(String::as_str))?;
    let podcast = q.get("podcast").map(|p| p.trim()).filter(|p| !p.is_empty()).map(str::to_string);
    let db = state.db_pool.clone();

    let device_id = match q.get("device").map(|d| d.trim()).filter(|d| !d.is_empty()) {
        Some(name) => match db.gpodder_device_id(auth.user_id, name).await? {
            Some(id) => Some(id),
            None => return Ok(Json(json!({ "actions": [], "timestamp": download_cursor(&db).await? })).into_response()),
        },
        None => None,
    };
    let collapse = if flag(&q, "aggregated") {
        Collapse::PerEpisode
    } else if since == 0 {
        Collapse::PerAction
    } else {
        Collapse::None
    };
    let filter = ActionFilter { user_id: auth.user_id, since, podcast, device_id, collapse };
    let timestamp = download_cursor(&db).await?;
    // The first page is read before answering, so a failing query is still a clean error.
    let first = db.gpodder_episode_actions_page(&filter, 0, PAGE_ROWS).await?;

    let feed = Feed { db, filter, step: Step::Page(first), wrote_any: false, timestamp, session: auth.session };
    let head = stream::once(async { Ok::<_, std::io::Error>(Bytes::from_static(b"{\"actions\":[")) });
    let body = Body::from_stream(head.chain(stream::unfold(feed, Feed::next)).fuse());
    Ok(([(header::CONTENT_TYPE, "application/json")], body).into_response())
}

enum Step {
    Page(ActionPage),
    Fetch(i32),
    Finish,
    Done,
}

/// The state of a download being written.
struct Feed {
    db: DatabasePool,
    filter: ActionFilter,
    step: Step,
    wrote_any: bool,
    timestamp: i64,
    session: Option<i32>,
}

impl Feed {
    /// The next chunk of the response body, or None once it's complete.
    async fn next(mut self) -> Option<(Result<Bytes, std::io::Error>, Self)> {
        loop {
            match std::mem::replace(&mut self.step, Step::Done) {
                Step::Page(page) => {
                    self.step = match (page.full, page.last_id) {
                        (true, Some(last)) => Step::Fetch(last),
                        _ => Step::Finish,
                    };
                    let mut buf = Vec::new();
                    for a in &page.actions {
                        let Some(out) = a.action.outbound(a.known_duration) else { continue };
                        if self.wrote_any {
                            buf.push(b',');
                        }
                        if let Err(e) = serde_json::to_writer(&mut buf, &out) {
                            error!("Writing episode action {} failed: {}", a.id, e);
                            self.step = Step::Done;
                            return Some((Err(std::io::Error::other(e)), self));
                        }
                        self.wrote_any = true;
                    }
                    // A page whose rows were all replaced still sends a byte (whitespace is valid
                    // JSON here), so a long run of them can't look like a stalled connection.
                    if buf.is_empty() {
                        buf.push(b' ');
                    }
                    return Some((Ok(Bytes::from(buf)), self));
                }
                Step::Fetch(after) => match self.db.gpodder_episode_actions_page(&self.filter, after, PAGE_ROWS).await {
                    Ok(page) => self.step = Step::Page(page),
                    Err(e) => {
                        error!("Reading episode actions for user {} failed mid-download: {}", self.filter.user_id, e);
                        return Some((Err(std::io::Error::other(e.to_string())), self));
                    }
                },
                Step::Finish => {
                    if let Some(session) = self.session {
                        if let Err(e) = self.db.gpodder_set_episodes_cursor(session, self.timestamp).await {
                            warn!("Saving the episode download cursor of session {} failed: {}", session, e);
                        }
                    }
                    let tail = format!("],\"timestamp\":{}}}", self.timestamp);
                    return Some((Ok(Bytes::from(tail)), self));
                }
                Step::Done => return None,
            }
        }
    }
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
