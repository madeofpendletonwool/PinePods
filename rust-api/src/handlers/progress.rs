//! Listening progress: `POST /progress`, and the rules that map the endpoints older apps still
//! call (`record_listen_duration`, `record_podcast_history`, `increment_listen_time`) onto the
//! same writer.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use axum::{extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};

use super::extract_api_key;
use crate::database::{ListenClient, Observed, ProgressWrite, TimeSaved, WriteRule};
use crate::error::{AppError, AppResult};
use crate::services::sync::SyncOrigin;
use crate::AppState;

/// Updates accepted in one request.
const MAX_UPDATES: usize = 500;
/// An update can't claim to be older than this.
const MAX_AGE_SECS: i64 = 90 * 24 * 3600;

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ProgressRequest {
    /// The client's clock when it sent this request, in unix milliseconds. Only its difference
    /// from each update's `at` is used, so the client's clock doesn't have to be right.
    #[serde(default)]
    pub sent_at: Option<i64>,
    /// The app sending this: `web`, `desktop`, `android` or `ios`. Recorded with the listening
    /// time for stats; anything else counts as unknown.
    #[serde(default)]
    pub client: Option<String>,
    pub updates: Vec<ProgressUpdate>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ProgressUpdate {
    pub episode_id: i32,
    #[serde(default)]
    pub is_youtube: bool,
    /// Seconds into the episode.
    pub position: f64,
    /// The client's clock when it observed this position, in unix milliseconds. Now if absent.
    #[serde(default)]
    pub at: Option<i64>,
    /// Seconds actually played since this client's previous report for the episode.
    #[serde(default)]
    pub listened: f64,
    /// The player's length for the episode in seconds, if it knows it.
    #[serde(default)]
    pub duration: Option<f64>,
    /// The episode was played to the end.
    #[serde(default)]
    pub finished: bool,
    /// Of the `listened` seconds: episode time gained by playing faster than 1x (negative when
    /// slower).
    #[serde(default)]
    pub speed_saved: f64,
    /// Seconds of silence skipped since the previous report.
    #[serde(default)]
    pub silence_skipped: f64,
    /// Seconds of ads skipped since the previous report.
    #[serde(default)]
    pub ads_skipped: f64,    /// Played from a file on the device rather than streamed.
    #[serde(default)]
    pub downloaded: bool,
}

/// The stored state of the episode after the update.
#[derive(Serialize, utoipa::ToSchema)]
pub struct ProgressResult {
    pub episode_id: i32,
    pub is_youtube: bool,
    /// False when the episode isn't in the user's library (any more).
    pub found: bool,
    /// Whether this update's position became the stored one. False when a newer one was
    /// already stored; `position` is then that newer one.
    pub applied: bool,
    pub position: i32,
    pub completed: bool,
    /// When the stored position was observed, in unix seconds on the server's clock.
    pub updated_at: Option<i64>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ProgressResponse {
    pub results: Vec<ProgressResult>,
}

/// Rounded to whole seconds, either sign.
fn signed_seconds(value: f64) -> i32 {
    if value.is_finite() {
        value.round().clamp(i32::MIN as f64, i32::MAX as f64) as i32
    } else {
        0
    }
}

fn whole_seconds(value: f64) -> i32 {
    if value.is_finite() {
        value.clamp(0.0, i32::MAX as f64) as i32
    } else {
        0
    }
}

/// Seconds between an update's observation and the request being sent.
fn age_secs(sent_at: Option<i64>, at: Option<i64>) -> i64 {
    match (sent_at, at) {
        (Some(sent), Some(at)) => (sent.saturating_sub(at) / 1000).clamp(0, MAX_AGE_SECS),
        _ => 0,
    }
}

/// Record playback positions, listened time and finished episodes for the key's user. The
/// newest observation of an episode wins, whichever device reports it and in whatever order
/// the reports arrive.
#[utoipa::path(
    post,
    path = "/progress",
    tag = "podcasts",
    summary = "Record listening progress",
    request_body = ProgressRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "The stored state of each episode after its update", body = ProgressResponse),
        (status = 400, description = "Too many updates in one request"),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn record_progress(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ProgressRequest>,
) -> Result<Json<ProgressResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if request.updates.len() > MAX_UPDATES {
        return Err(AppError::bad_request(&format!("At most {MAX_UPDATES} updates per request")));
    }

    // Oldest first, so several updates for one episode end on its newest.
    let mut updates: Vec<(i64, &ProgressUpdate)> = request.updates.iter().map(|u| (age_secs(request.sent_at, u.at), u)).collect();
    updates.sort_by_key(|(age, _)| std::cmp::Reverse(*age));

    let client = request.client.as_deref().map_or(ListenClient::Unknown, ListenClient::parse);
    let mut results = Vec::with_capacity(updates.len());
    let mut listened_total = 0i64;
    for (age, update) in updates {
        let listened = whole_seconds(update.listened.round());
        let write = ProgressWrite {
            episode_id: update.episode_id,
            is_youtube: update.is_youtube,
            position: whole_seconds(update.position),
            observed: Observed::Ago(age),
            rule: WriteRule::Newest,
            listened,
            finished: update.finished,
            duration: update.duration.filter(|d| d.is_finite() && *d >= 1.0).map(|d| d.round() as i32),
            client,
            saved: TimeSaved {
                speed: signed_seconds(update.speed_saved),
                silence: whole_seconds(update.silence_skipped.round()),
                ads: whole_seconds(update.ads_skipped.round()),
            },
            downloaded: update.downloaded,
        };
        let outcome = if update.episode_id > 0 { state.db_pool.write_progress(user_id, write, SyncOrigin::Pinepods).await? } else { None };
        if outcome.is_some() {
            listened_total += i64::from(listened.min(86_400));
        }
        results.push(ProgressResult {
            episode_id: update.episode_id,
            is_youtube: update.is_youtube,
            found: outcome.is_some(),
            applied: outcome.is_some_and(|o| o.applied),
            position: outcome.map_or(0, |o| o.position),
            completed: outcome.is_some_and(|o| o.completed),
            updated_at: outcome.and_then(|o| o.updated_at),
        });
    }
    state.db_pool.add_listen_seconds(user_id, listened_total.min(i64::from(i32::MAX)) as i32).await?;
    Ok(Json(ProgressResponse { results }))
}

// ---------------------------------------------------------------------------------------------
// Older apps
// ---------------------------------------------------------------------------------------------
//
// They report a bare position: no time, no device. What each API key last reported for an
// episode is remembered here, which is enough to tell a paused player repeating itself from a
// moving one, and a player moving its own position back from one that is merely behind.

#[derive(Clone, Copy)]
struct Report {
    position: i32,
    at: Instant,
}

#[derive(Default)]
struct Reports {
    last: HashMap<(String, bool, i32), Report>,
    /// When each key last reported a position different from its previous one.
    moved: HashMap<String, Instant>,
}

const REPORT_TTL: Duration = Duration::from_secs(6 * 3600);
const REPORTS_BEFORE_PRUNE: usize = 5000;
/// A listened minute counts if the key's position moved this recently.
const PLAYING_WITHIN: Duration = Duration::from_secs(90);

static REPORTS: LazyLock<Mutex<Reports>> = LazyLock::new(|| Mutex::new(Reports::default()));

/// Remember `position` as the key's latest report for the episode; returns the previous one.
fn remember(api_key: &str, is_youtube: bool, episode_id: i32, position: i32) -> Option<Report> {
    let now = Instant::now();
    let mut reports = REPORTS.lock().unwrap();
    if reports.last.len() >= REPORTS_BEFORE_PRUNE {
        reports.last.retain(|_, r| now.duration_since(r.at) < REPORT_TTL);
        reports.moved.retain(|_, at| now.duration_since(*at) < REPORT_TTL);
    }
    let previous = reports.last.insert((api_key.to_string(), is_youtube, episode_id), Report { position, at: now });
    if previous.is_some_and(|p| p.position != position) {
        reports.moved.insert(api_key.to_string(), now);
    }
    previous
}

/// A position from an app that reports without a time (`record_listen_duration`):
/// - the same position as the key's last report is a paused player repeating itself: ignored;
/// - a position ahead of the stored one is applied;
/// - a position behind it is applied only if the stored one is what this key last reported.
pub async fn record_untimed_position(
    state: &AppState,
    api_key: &str,
    user_id: i32,
    episode_id: i32,
    is_youtube: bool,
    position: f64,
    duration: Option<f64>,
) -> AppResult<()> {
    if !position.is_finite() || position < 0.0 {
        return Ok(());
    }
    let position = whole_seconds(position);
    let previous = remember(api_key, is_youtube, episode_id, position);
    if previous.is_some_and(|p| p.position == position) {
        return Ok(());
    }
    // Time played is credited from forward movement, never more than the time that passed.
    let listened = previous.map_or(0, |p| {
        let elapsed = p.at.elapsed().as_secs().min(86_400) as i32 + 5;
        (position - p.position).clamp(0, elapsed)
    });
    let write = ProgressWrite {
        listened,
        duration: duration.filter(|d| d.is_finite() && *d >= 1.0).map(|d| d.round() as i32),
        ..ProgressWrite::now(episode_id, is_youtube, position, previous.map_or(WriteRule::Forward, |p| WriteRule::ForwardOrOwn(p.position)))
    };
    match state.db_pool.write_progress(user_id, write, SyncOrigin::Pinepods).await? {
        Some(_) => Ok(()),
        None => Err(AppError::not_found("Episode not found")),
    }
}

/// `record_podcast_history`: older players call it as playback starts, with the position they
/// are starting from (the old web player always sent 0). It puts the episode in the history and
/// never moves a stored position back.
pub async fn record_play_start(state: &AppState, user_id: i32, episode_id: i32, is_youtube: bool, position: f64) -> AppResult<()> {
    let position = if position.is_finite() { whole_seconds(position) } else { 0 };
    let write = ProgressWrite::now(episode_id, is_youtube, position, WriteRule::Forward);
    match state.db_pool.write_progress(user_id, write, SyncOrigin::Pinepods).await? {
        Some(_) => Ok(()),
        None => Err(AppError::not_found("Episode not found")),
    }
}

/// `increment_listen_time`: older apps call it every minute whether or not anything is
/// playing, so the minute counts only if the key's position has been moving.
pub async fn record_untimed_minute(state: &AppState, api_key: &str, user_id: i32) -> AppResult<()> {
    let playing = REPORTS.lock().unwrap().moved.get(api_key).is_some_and(|at| at.elapsed() < PLAYING_WITHIN);
    if playing {
        state.db_pool.add_listen_seconds(user_id, 60).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_is_the_gap_between_the_clients_two_readings() {
        assert_eq!(age_secs(Some(1_000_000), Some(940_000)), 60);
        // A clock a day ahead or behind gives the same gap.
        let day = 86_400_000;
        assert_eq!(age_secs(Some(1_000_000 + day), Some(940_000 + day)), 60);
        assert_eq!(age_secs(Some(1_000_000 - day), Some(940_000 - day)), 60);
    }

    #[test]
    fn age_is_never_negative_or_unbounded() {
        assert_eq!(age_secs(Some(1_000), Some(9_000)), 0, "observed after it was sent");
        assert_eq!(age_secs(Some(i64::MAX), Some(0)), MAX_AGE_SECS);
        assert_eq!(age_secs(None, Some(5)), 0);
        assert_eq!(age_secs(Some(5), None), 0);
    }

    #[test]
    fn reports_are_remembered_per_key_and_episode() {
        assert!(remember("key-a", false, 900_001, 100).is_none());
        assert_eq!(remember("key-a", false, 900_001, 130).map(|r| r.position), Some(100));
        assert!(remember("key-b", false, 900_001, 50).is_none(), "another key has its own history");
        assert!(remember("key-a", true, 900_001, 50).is_none(), "a video with the same id is another item");
    }

    #[test]
    fn only_a_moving_position_counts_as_playing() {
        let moved = |key: &str| REPORTS.lock().unwrap().moved.contains_key(key);
        remember("key-paused", false, 900_002, 100);
        remember("key-paused", false, 900_002, 100);
        assert!(!moved("key-paused"));
        remember("key-playing", false, 900_002, 100);
        remember("key-playing", false, 900_002, 115);
        assert!(moved("key-playing"));
    }
}
