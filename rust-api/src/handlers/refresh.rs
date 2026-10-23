use axum::{
    extract::{Query, Path, State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{WebSocket, Message};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, LazyLock};
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};
use sqlx::Row;
use tracing::{debug, info, warn};

use crate::{
    error::{AppError, AppResult},
    handlers::check_user_access,
    services::feed_health::{self, FeedErrorKind},
    services::refresh_schedule::{self, FeedHints, Schedule},
    services::sync::engine::{self, RunMode},
    services::sync::engine_db::TargetKind,
    AppState,
};

#[derive(Deserialize)]
pub struct RefreshQuery {
    pub api_key: Option<String>,
    pub nextcloud_refresh: Option<bool>,
}

#[derive(Serialize)]
pub struct RefreshProgress {
    pub current: u32,
    pub total: u32,
    pub current_podcast: String,
}

#[derive(Serialize)]
pub struct RefreshStatus {
    pub progress: RefreshProgress,
}

#[derive(Serialize)]
pub struct NewEpisode {
    pub new_episode: crate::handlers::podcasts::Episode,
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum RefreshMessage {
    Status(RefreshStatus),
    NewEpisode(NewEpisode),
    Error { detail: String },
}

// Store locks per user to prevent concurrent refresh jobs
type UserLocks = Arc<RwLock<HashMap<i32, Arc<Mutex<()>>>>>;

// Store active WebSocket connections
type ActiveWebSockets = Arc<RwLock<HashMap<i32, Vec<tokio::sync::mpsc::Sender<RefreshMessage>>>>>;

// Global state for refresh management
lazy_static::lazy_static! {
    static ref USER_LOCKS: UserLocks = Arc::new(RwLock::new(HashMap::new()));
    static ref ACTIVE_WEBSOCKETS: ActiveWebSockets = Arc::new(RwLock::new(HashMap::new()));
}

// Admin refresh endpoint (background task) - matches Python refresh_pods function exactly
#[utoipa::path(
    get,
    path = "/refresh_pods",
    tag = "tasks",
    summary = "Refresh all podcasts",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Refresh triggered", body = serde_json::Value),
    ),
)]
pub async fn refresh_pods_admin(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    // Server-wide fan-out over every user: admin (or background key) only.
    crate::handlers::require_admin(&state, &headers).await?;
    info!("Starting admin refresh process - background task (no WebSocket)");
    
    // This is the background task version - NO WebSocket, just direct refresh like Python
    let state_clone = state.clone();
    tokio::spawn(async move {
        // This matches the Python refresh_pods function exactly
        if let Err(e) = refresh_all_podcasts_background(&state_clone).await {
            tracing::error!("Background refresh failed: {}", e);
        }
    });

    Ok(axum::Json(serde_json::json!({
        "detail": "Refresh initiated."
    })))
}

// Separate endpoint for gPodder refresh (scheduled separately like Python)
#[utoipa::path(
    get,
    path = "/refresh_gpodder_subscriptions",
    tag = "tasks",
    summary = "Refresh gpodder subscriptions",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Refresh triggered", body = serde_json::Value),
    ),
)]
pub async fn refresh_gpodder_subscriptions_admin(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    // Server-wide fan-out over every user: admin (or background key) only.
    crate::handlers::require_admin(&state, &headers).await?;
    info!("Starting admin gPodder sync process for all users");
    
    let state_clone = state.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_gpodder_subscriptions".to_string(),
        0, // System user
        move |reporter| async move {
            let state = state_clone;
            reporter.update_progress(10.0, Some("Starting gPodder sync for all users...".to_string())).await?;
            
            // Every user syncing with an external gpodder server
            let gpodder_users = state.db_pool.sync_target_users(Some(TargetKind::Gpodder)).await
                .map_err(|e| AppError::internal(&format!("Failed to get gPodder users: {}", e)))?;
            
            debug!("Found {} users with gPodder sync enabled", gpodder_users.len());
            
            let mut successful_syncs = 0;
            let mut failed_syncs = 0;
            let mut total_synced_podcasts = 0;
            
            for (index, user_id) in gpodder_users.iter().enumerate() {
                let progress = 10.0 + (80.0 * (index as f64) / (gpodder_users.len() as f64));
                reporter.update_progress(progress, Some(format!("Syncing user {}/{}", index + 1, gpodder_users.len()))).await?;
                
                match run_admin_gpodder_sync(&state, *user_id).await {
                    Ok(sync_result) => {
                        successful_syncs += 1;
                        total_synced_podcasts += sync_result.synced_podcasts;
                        debug!("gPodder sync successful for user {}: {} podcasts", user_id, sync_result.synced_podcasts);
                    }
                    Err(e) => {
                        failed_syncs += 1;
                        debug!("gPodder sync failed for user {}: {}", user_id, e);
                    }
                }
            }
            
            info!("Admin gPodder sync completed: {}/{} users successful, {} total podcasts synced", 
                successful_syncs, gpodder_users.len(), total_synced_podcasts);
            
            reporter.update_progress(100.0, Some(format!(
                "gPodder sync completed: {}/{} users, {} podcasts", 
                successful_syncs, gpodder_users.len(), total_synced_podcasts
            ))).await?;
            
            Ok(serde_json::json!({
                "success": true,
                "users_synced": successful_syncs,
                "users_failed": failed_syncs,
                "total_podcasts": total_synced_podcasts
            }))
        },
    ).await?;

    Ok(axum::Json(serde_json::json!({
        "detail": "gPodder sync for all users initiated.",
        "task_id": task_id
    })))
}

// Max feeds refreshed in parallel. Shared by the background and per-user refresh paths.
const REFRESH_CONCURRENCY: usize = 10;
// Max feeds one scheduled tick refreshes; the rest wait for the next tick.
const REFRESH_TICK_MAX_FEEDS: i64 = 250;

// One refresh of a feed at a time, scheduled or manual. Keyed like the refresh groups
// (URL + credentials); YouTube channels by channel URL.
type FeedKey = (String, Option<String>, Option<String>);
static FEED_LOCKS: LazyLock<std::sync::Mutex<HashMap<FeedKey, Arc<Mutex<()>>>>> = LazyLock::new(Default::default);

struct FeedLock {
    key: FeedKey,
    guard: Option<OwnedMutexGuard<()>>,
}

impl FeedLock {
    fn key(feed_url: &str, username: Option<&str>, password: Option<&str>) -> FeedKey {
        (feed_url.to_string(), username.map(str::to_string), password.map(str::to_string))
    }

    /// Wait for any other refresh of the feed to finish (the scheduled refresh).
    async fn acquire(key: FeedKey) -> FeedLock {
        let lock = FEED_LOCKS.lock().unwrap().entry(key.clone()).or_default().clone();
        FeedLock { key, guard: Some(lock.lock_owned().await) }
    }

    /// None while another refresh has the feed (the manual refresh skips it).
    fn try_acquire(key: FeedKey) -> Option<FeedLock> {
        let lock = FEED_LOCKS.lock().unwrap().entry(key.clone()).or_default().clone();
        let guard = lock.try_lock_owned().ok()?;
        Some(FeedLock { key, guard: Some(guard) })
    }
}

impl Drop for FeedLock {
    fn drop(&mut self) {
        let mut locks = FEED_LOCKS.lock().unwrap();
        self.guard.take();
        // Nobody else holds or waits for it: forget the feed.
        if locks.get(&self.key).is_some_and(|l| Arc::strong_count(l) == 1) {
            locks.remove(&self.key);
        }
    }
}

// One podcast row to refresh. Defined at module scope so the grouped/concurrent helpers below
// can take owned values without borrowing the DB connection.
#[derive(Clone)]
struct PodcastRefreshItem {
    podcast_id: i32,
    feed_id: Option<i32>,
    feed_url: String,
    artwork_url: Option<String>,
    auto_download: bool,
    auto_queue: bool,
    username: Option<String>,
    password: Option<String>,
    is_youtube: bool,
    user_id: i32,
    feed_cutoff: Option<i32>,
    etag: Option<String>,
    last_modified: Option<String>,
    content_hash: Option<String>,
    refresh_interval: Option<i32>,
}

/// Queue server-side auto-downloads for newly inserted episodes, if the podcast has auto-download
/// enabled and server downloads are turned on. Shared by every refresh path.
async fn queue_auto_downloads(
    state: &AppState,
    user_id: i32,
    auto_download: bool,
    new_episodes: &[crate::handlers::podcasts::Episode],
) {
    if !auto_download || new_episodes.is_empty() {
        return;
    }
    if !state.db_pool.download_status().await.unwrap_or(false) {
        debug!("Skipping auto-download — server downloads disabled");
        return;
    }
    for episode in new_episodes {
        let is_yt = episode.episodeurl.contains("youtube.com") || episode.episodeurl.contains("youtu.be");
        let task_result = if is_yt {
            state.task_spawner.spawn_download_youtube_video(episode.episodeid, user_id).await
        } else {
            state.task_spawner.spawn_download_podcast_episode(episode.episodeid, user_id).await
        };
        match task_result {
            Ok(task_id) => debug!("Auto-download task queued with ID: {}", task_id),
            Err(e) => warn!("Failed to queue auto-download: {}", e),
        }
    }
}

/// Append newly inserted episodes to the owning user's play queue, if the podcast has
/// auto-queue enabled (#648). Unlike auto-download this is NOT gated on server downloads.
/// `queue_episode` is idempotent (a re-run won't create duplicates). Episodes are enqueued
/// oldest-first so a multi-episode feed drop lands in chronological listening order — DB
/// pubdate strings render as "YYYY-MM-DD HH:MM:SS", which sorts lexically by time.
async fn queue_auto_new_episodes(
    state: &AppState,
    user_id: i32,
    auto_queue: bool,
    new_episodes: &[crate::handlers::podcasts::Episode],
) {
    if !auto_queue || new_episodes.is_empty() {
        return;
    }
    let mut ordered: Vec<&crate::handlers::podcasts::Episode> = new_episodes.iter().collect();
    ordered.sort_by(|a, b| a.episodepubdate.cmp(&b.episodepubdate));
    for episode in ordered {
        match state
            .db_pool
            // Auto-queued feed episodes append to the bottom (chronological build-up),
            // unlike user-initiated adds which go to the top / under the playing item.
            .queue_episode(
                episode.episodeid,
                user_id,
                episode.is_youtube,
                crate::database::QueuePlacement::Bottom,
            )
            .await
        {
            Ok(()) => debug!("Auto-queued episode {} for user {}", episode.episodeid, user_id),
            Err(e) => warn!("Failed to auto-queue episode {}: {}", episode.episodeid, e),
        }
    }
}

/// Refresh one feed that may be shared by several subscriber podcasts (cross-user dedup). The feed
/// is fetched (conditional GET + body hash) and parsed ONCE, then applied to every subscriber's
/// podcast row; new episodes are auto-downloaded/queued/transcribed and added to matching
/// category collections (sync rematch runs inside `apply_parsed_episodes`). Cache validators and
/// success/failure state are recorded for the whole group. The caller holds the feed's lock.
/// Returns every newly inserted episode across the group; Err only when the feed itself failed.
async fn refresh_feed_group(
    state: &AppState,
    group: &[PodcastRefreshItem],
) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
    let rep = &group[0];
    let ids: Vec<i32> = group.iter().map(|i| i.podcast_id).collect();
    let mut all_new = Vec::new();

    // Reuse any stored validator from the group (they converge after the first unified cycle).
    // The body hash is only trusted when every subscriber has it: a row that never applied this
    // body (e.g. its first import failed) must not be skipped because another row did.
    let etag = group.iter().find_map(|i| i.etag.clone());
    let last_modified = group.iter().find_map(|i| i.last_modified.clone());
    let content_hash = rep
        .content_hash
        .clone()
        .filter(|h| group.iter().all(|i| i.content_hash.as_deref() == Some(h.as_str())));
    let validators = crate::database::FeedValidators {
        etag: etag.as_deref(),
        last_modified: last_modified.as_deref(),
        content_hash: content_hash.as_deref(),
    };

    let fetched = state
        .db_pool
        .fetch_feed_conditional(&rep.feed_url, rep.username.as_deref(), rep.password.as_deref(), validators)
        .await;
    match fetched {
        Ok(crate::database::FeedFetch::NotModified { etag, last_modified, max_age }) => {
            debug!("Feed unchanged, skipping parse: {}", rep.feed_url);
            if etag.is_some() || last_modified.is_some() {
                let _ = state
                    .db_pool
                    .store_feed_cache_validators(&ids, etag.as_deref(), last_modified.as_deref(), None)
                    .await;
            }
            let next = unchanged_schedule(state, rep, max_age).await;
            mark_refreshed(state, &ids, next).await;
        }
        Ok(crate::database::FeedFetch::Fetched { body, etag, last_modified, content_hash, max_age }) => {
            let artwork = rep.artwork_url.clone().unwrap_or_default();
            let parsed = match state.db_pool.parse_feed_body(&body, rep.podcast_id, &artwork).await {
                Ok(parsed) => parsed,
                Err(e) => {
                    warn!("Error parsing feed {}: {}", rep.feed_url, e);
                    mark_failed(state, &ids, FeedErrorKind::NotFeed, &e.to_string(), None).await;
                    return Err(e);
                }
            };
            let mut all_applied = true;
            for item in group {
                let art = item.artwork_url.as_deref().unwrap_or("");
                match state
                    .db_pool
                    .apply_parsed_episodes(item.podcast_id, &parsed, art, item.feed_cutoff, true)
                    .await
                {
                    Ok(new_eps) => {
                        if !new_eps.is_empty() {
                            info!("Podcast {}: {} new episodes", item.podcast_id, new_eps.len());
                        }
                        queue_auto_downloads(state, item.user_id, item.auto_download, &new_eps).await;
                        queue_auto_new_episodes(state, item.user_id, item.auto_queue, &new_eps).await;
                        let ids: Vec<i32> = new_eps.iter().map(|e| e.episodeid).collect();
                        if let Err(e) = state.db_pool.auto_add_category_episodes_for(item.user_id, item.podcast_id, &ids).await {
                            warn!("Category auto-add for podcast {} failed: {}", item.podcast_id, e);
                        }
                        // Auto-transcribe new episodes for opted-in podcasts (independent
                        // of downloads — the pipeline fetches audio on demand).
                        for ep in &new_eps {
                            crate::services::transcription::maybe_transcribe_episode(state.db_pool.clone(), ep.episodeid);
                        }
                        all_new.extend(new_eps);
                    }
                    Err(e) => {
                        all_applied = false;
                        warn!("Error applying feed to podcast {}: {}", item.podcast_id, e);
                    }
                }
            }
            state.db_pool.store_feed_tags(&ids, &body).await;
            // Only remember the body hash once every subscriber has it applied; otherwise the
            // next cycle would skip the row that failed.
            let hash = all_applied.then_some(content_hash.as_str());
            let _ = state
                .db_pool
                .store_feed_cache_validators(&ids, etag.as_deref(), last_modified.as_deref(), hash)
                .await;
            let dates: Vec<_> = parsed.iter().map(|e| e.pub_date).collect();
            let hints = FeedHints { max_age, ..refresh_schedule::parse_hints(&body) };
            mark_refreshed(state, &ids, refresh_schedule::schedule(&dates, &hints, chrono::Utc::now())).await;
        }
        Ok(crate::database::FeedFetch::Throttled { status, retry_after }) => {
            let msg = format!("Feed host throttled refresh (HTTP {})", status);
            mark_failed(state, &ids, FeedErrorKind::Throttled, &msg, retry_after).await;
            return Err(AppError::external_error(&msg));
        }
        Err(e) => {
            warn!("Error fetching feed {}: {}", rep.feed_url, e);
            mark_failed(state, &ids, FeedErrorKind::of(&e), &e.to_string(), None).await;
            return Err(e);
        }
    }

    Ok(all_new)
}

/// The next refresh of a feed whose body hasn't changed: it keeps the interval it had (at least
/// the host's max-age), unless it has gone quiet since.
async fn unchanged_schedule(state: &AppState, rep: &PodcastRefreshItem, max_age: Option<chrono::Duration>) -> Schedule {
    let now = chrono::Utc::now();
    let max = refresh_schedule::max_interval();
    let dates = state.db_pool.recent_pub_dates(rep.podcast_id, false, 10).await.unwrap_or_default();
    let hints = FeedHints { max_age, ..Default::default() };
    let interval = match rep.refresh_interval {
        Some(minutes) if !refresh_schedule::is_dormant(&dates, now) => {
            let floor = max_age.unwrap_or_default().min(refresh_schedule::ACTIVE_MAX_INTERVAL);
            chrono::Duration::minutes(minutes.into()).max(floor).min(max)
        }
        _ => refresh_schedule::interval(&dates, &hints, now, max),
    };
    refresh_schedule::schedule_with(interval, &dates, &hints, now, max)
}

/// The next refresh of a YouTube channel, from its videos.
async fn youtube_schedule(state: &AppState, podcast_id: i32) -> Schedule {
    let dates = state.db_pool.recent_pub_dates(podcast_id, true, 10).await.unwrap_or_default();
    refresh_schedule::schedule(&dates, &FeedHints::default(), chrono::Utc::now())
}

/// Record a successful refresh and schedule the next, telling users whose feed had been
/// reported as failing.
async fn mark_refreshed(state: &AppState, ids: &[i32], next: Schedule) {
    let recovered = state.db_pool.feed_problems_reported(ids).await.unwrap_or_default();
    if let Err(e) = state.db_pool.record_refresh_success(ids, next).await {
        warn!("Recording a successful refresh of {:?} failed: {}", ids, e);
        return;
    }
    feed_health::after_success(&state.task_manager, recovered).await;
}

/// Import a newly added YouTube channel's videos and schedule its refreshes. Holds the channel's
/// refresh lock so a scheduled refresh can't run alongside.
pub async fn import_added_channel(state: &AppState, podcast_id: i32, channel_id: &str, feed_cutoff: i32) -> AppResult<()> {
    let url = format!("https://www.youtube.com/channel/{}", channel_id);
    let _lock = FeedLock::acquire(FeedLock::key(&url, None, None)).await;
    crate::handlers::youtube::process_youtube_channel(podcast_id, channel_id, feed_cutoff, state).await?;
    mark_refreshed(state, &[podcast_id], youtube_schedule(state, podcast_id).await).await;
    Ok(())
}

/// Record a failed refresh and report feeds that have now been failing long enough.
async fn mark_failed(state: &AppState, ids: &[i32], kind: FeedErrorKind, error: &str, retry_after: Option<std::time::Duration>) {
    if let Err(e) = state.db_pool.record_refresh_failure(ids, kind, error, retry_after).await {
        warn!("Recording a failed refresh of {:?} failed: {}", ids, e);
        return;
    }
    feed_health::after_failure(&state.db_pool, &state.task_manager, ids).await;
}

/// Refresh a single YouTube channel podcast.
async fn refresh_youtube_item(state: &AppState, item: &PodcastRefreshItem) {
    let channel_id = if item.feed_url.contains("channel/") {
        item.feed_url
            .split("channel/")
            .nth(1)
            .unwrap_or(&item.feed_url)
            .split('/')
            .next()
            .unwrap_or(&item.feed_url)
            .split('?')
            .next()
            .unwrap_or(&item.feed_url)
            .to_string()
    } else {
        item.feed_url.clone()
    };
    let ids = [item.podcast_id];
    match crate::handlers::youtube::process_youtube_channel(
        item.podcast_id,
        &channel_id,
        item.feed_cutoff.unwrap_or(30),
        state,
    )
    .await
    {
        Ok(_) => {
            info!("Successfully refreshed YouTube channel {}", item.podcast_id);
            mark_refreshed(state, &ids, youtube_schedule(state, item.podcast_id).await).await;
        }
        Err(e) => {
            warn!("Error refreshing YouTube channel {}: {}", item.podcast_id, e);
            mark_failed(state, &ids, FeedErrorKind::of(&e), &e.to_string(), None).await;
        }
    }
}

/// The podcast rows to refresh (all, or those of the `only` feeds), skipping feeds a host asked us
/// to leave alone (Retry-After) and, unless `ignore_backoff`, feeds in failure backoff.
async fn load_refresh_items(
    state: &AppState,
    only: Option<&[String]>,
    feeds: Option<&[i32]>,
    ignore_backoff: bool,
) -> AppResult<Vec<PodcastRefreshItem>> {
    use crate::services::scheduler::{BACKOFF_MAX_STEPS, BACKOFF_STEP_MINUTES};
    let mut refresh_items: Vec<PodcastRefreshItem> = Vec::new();

    match &state.db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
                r#"SELECT podcastid, feedid, feedurl, artworkurl, autodownload, autoqueue, username, password,
                          isyoutubechannel, userid, feedcutoffdays, feedetag, feedlastmodified, feedcontenthash,
                          refreshintervalminutes
                   FROM "Podcasts"
                   WHERE COALESCE(refreshpodcast, TRUE) = TRUE
                     AND (refreshnotbefore IS NULL OR refreshnotbefore <= NOW())
                     AND ($1 OR COALESCE(consecutivefailures, 0) = 0
                          OR lastrefreshattempt IS NULL
                          OR lastrefreshattempt < NOW() - (INTERVAL '1 minute' * ({BACKOFF_STEP_MINUTES} * LEAST(COALESCE(consecutivefailures, 0), {BACKOFF_MAX_STEPS}))))
                     AND ($2::TEXT[] IS NULL OR feedurl = ANY($2))
                     AND ($3::INT[] IS NULL OR feedid = ANY($3))"#
            )))
            .bind(ignore_backoff)
            .bind(only)
            .bind(feeds)
            .fetch_all(pool)
            .await?;

            for result in rows {
                let podcast_id: i32 = result.try_get("podcastid")?;
                refresh_items.push(PodcastRefreshItem {
                    podcast_id,
                    feed_id: result.try_get("feedid")?,
                    feed_url: result.try_get("feedurl")?,
                    artwork_url: result.try_get("artworkurl").ok(),
                    auto_download: result.try_get("autodownload")?,
                    auto_queue: result.try_get("autoqueue").unwrap_or(false),
                    username: result.try_get("username").ok(),
                    password: result.try_get("password").ok(),
                    is_youtube: result.try_get("isyoutubechannel")?,
                    user_id: result.try_get("userid")?,
                    feed_cutoff: result.try_get("feedcutoffdays").ok(),
                    etag: result.try_get::<Option<String>, _>("feedetag").ok().flatten(),
                    last_modified: result.try_get::<Option<String>, _>("feedlastmodified").ok().flatten(),
                    content_hash: result.try_get::<Option<String>, _>("feedcontenthash").ok().flatten(),
                    refresh_interval: result.try_get::<Option<i32>, _>("refreshintervalminutes").ok().flatten(),
                });
            }
        }
        crate::database::DatabasePool::MySQL(pool) => {
            let in_list = |column: &str, n: Option<usize>| match n {
                Some(n) if n > 0 => format!("AND {column} IN ({})", vec!["?"; n].join(", ")),
                Some(_) => "AND FALSE".to_string(),
                None => String::new(),
            };
            let urls = in_list("FeedURL", only.map(<[String]>::len));
            let feed_ids = in_list("FeedID", feeds.map(<[i32]>::len));
            let mut query = sqlx::query(sqlx::AssertSqlSafe(format!(
                "SELECT PodcastID, FeedID, FeedURL, ArtworkURL, AutoDownload, AutoQueue, Username, Password,
                        IsYouTubeChannel, UserID, FeedCutoffDays, FeedETag, FeedLastModified, FeedContentHash,
                        RefreshIntervalMinutes
                 FROM Podcasts
                 WHERE COALESCE(RefreshPodcast, 1) = 1
                   AND (RefreshNotBefore IS NULL OR RefreshNotBefore <= NOW())
                   AND (? OR COALESCE(ConsecutiveFailures, 0) = 0
                        OR LastRefreshAttempt IS NULL
                        OR LastRefreshAttempt < NOW() - INTERVAL ({BACKOFF_STEP_MINUTES} * LEAST(COALESCE(ConsecutiveFailures, 0), {BACKOFF_MAX_STEPS})) MINUTE)
                   {urls} {feed_ids}"
            )))
            .bind(ignore_backoff);
            for url in only.unwrap_or_default() {
                query = query.bind(url);
            }
            for id in feeds.unwrap_or_default() {
                query = query.bind(id);
            }
            let rows = query.fetch_all(pool).await?;

            for result in rows {
                let podcast_id: i32 = result.try_get("PodcastID")?;
                refresh_items.push(PodcastRefreshItem {
                    podcast_id,
                    feed_id: result.try_get("FeedID")?,
                    feed_url: result.try_get("FeedURL")?,
                    artwork_url: result.try_get("ArtworkURL").ok(),
                    auto_download: result.try_get("AutoDownload")?,
                    auto_queue: result.try_get("AutoQueue").unwrap_or(false),
                    username: result.try_get("Username").ok(),
                    password: result.try_get("Password").ok(),
                    is_youtube: result.try_get("IsYouTubeChannel")?,
                    user_id: result.try_get("UserID")?,
                    feed_cutoff: result.try_get("FeedCutoffDays").ok(),
                    etag: result.try_get::<Option<String>, _>("FeedETag").ok().flatten(),
                    last_modified: result.try_get::<Option<String>, _>("FeedLastModified").ok().flatten(),
                    content_hash: result.try_get::<Option<String>, _>("FeedContentHash").ok().flatten(),
                    refresh_interval: result.try_get::<Option<i32>, _>("RefreshIntervalMinutes").ok().flatten(),
                });
            }
        }
    }


    Ok(refresh_items)
}

// Background refresh of every user's podcasts (no WebSocket), due or not.
async fn refresh_all_podcasts_background(state: &AppState) -> AppResult<()> {
    info!("Running refresh");
    let items = load_refresh_items(state, None, None, false).await?;
    refresh_items(state, items).await;
    info!("Refresh completed");
    Ok(())
}

/// Refresh the feeds whose next refresh is due, the longest overdue first, at most
/// `REFRESH_TICK_MAX_FEEDS` of them. Returns how many feeds were refreshed.
pub async fn refresh_due_podcasts(state: &AppState) -> AppResult<usize> {
    // Subscriptions added or moved since the last tick join their feed first.
    if let Err(e) = state.db_pool.link_unlinked_feeds().await {
        warn!("Linking podcasts to their feeds failed: {}", e);
    }
    let due = state.db_pool.due_refresh_feeds(REFRESH_TICK_MAX_FEEDS).await?;
    if due.is_empty() {
        return Ok(0);
    }
    let items = load_refresh_items(state, None, Some(&due), false).await?;
    debug!("Refreshing {} due feeds", due.len());
    Ok(refresh_items(state, items).await)
}

/// Refresh these feeds for every subscriber now (a Podping announced an update). Failure backoff
/// is skipped: the host says there's something new. Returns how many feeds were refreshed.
pub async fn refresh_feed_urls(state: &AppState, urls: &[String]) -> AppResult<usize> {
    if urls.is_empty() {
        return Ok(0);
    }
    let items = load_refresh_items(state, Some(urls), None, true).await?;
    Ok(refresh_items(state, items).await)
}

/// Refresh podcast rows: RSS feeds once per (feed, credentials) for all their subscribers,
/// YouTube channels one by one, then playlist counts of users with new episodes. Returns the
/// number of feeds and channels refreshed.
async fn refresh_items(state: &AppState, refresh_items: Vec<PodcastRefreshItem>) -> usize {
    // Partition into YouTube items (refreshed per-podcast) and RSS feeds grouped by feed, so a
    // feed subscribed by many users is fetched + parsed ONCE. A row not linked to its feed yet
    // groups by address and credentials, which is what a feed is.
    use std::collections::HashMap;
    #[derive(PartialEq, Eq, Hash)]
    enum Group {
        Feed(i32),
        Unlinked(FeedKey),
    }
    let mut youtube_items: Vec<PodcastRefreshItem> = Vec::new();
    let mut feed_groups: HashMap<Group, Vec<PodcastRefreshItem>> = HashMap::new();

    let owners: HashMap<i32, i32> = refresh_items.iter().map(|i| (i.podcast_id, i.user_id)).collect();
    for item in refresh_items {
        if item.feed_url.starts_with("local://") {
            debug!("Skipping local podcast {} - not an RSS feed", item.podcast_id);
            continue;
        }
        if item.is_youtube {
            youtube_items.push(item);
        } else {
            let key = match item.feed_id {
                Some(id) => Group::Feed(id),
                None => Group::Unlinked(FeedLock::key(&item.feed_url, item.username.as_deref(), item.password.as_deref())),
            };
            feed_groups.entry(key).or_default().push(item);
        }
    }

    use futures::stream::{self, StreamExt};

    // Refresh unique RSS feeds with bounded concurrency.
    let groups: Vec<Vec<PodcastRefreshItem>> = feed_groups.into_values().collect();
    let feeds = groups.len() + youtube_items.len();
    let subscription_count: usize = groups.iter().map(|g| g.len()).sum();
    debug!(
        "Running refresh: {} unique RSS feeds ({} subscriptions), {} YouTube channels (concurrency={})",
        groups.len(),
        subscription_count,
        youtube_items.len(),
        REFRESH_CONCURRENCY
    );
    // Users whose podcasts got new episodes, for their playlist counts.
    let touched: std::sync::Mutex<HashSet<i32>> = Default::default();
    stream::iter(groups)
        .for_each_concurrent(Some(REFRESH_CONCURRENCY), |group| {
            let (owners, touched) = (&owners, &touched);
            async move {
                let rep = &group[0];
                let _lock = FeedLock::acquire(FeedLock::key(&rep.feed_url, rep.username.as_deref(), rep.password.as_deref())).await;
                // Failures are recorded on the podcast rows and logged inside.
                if let Ok(new_eps) = refresh_feed_group(state, &group).await {
                    touched.lock().unwrap().extend(new_eps.iter().filter_map(|e| owners.get(&e.podcastid)));
                }
            }
        })
        .await;

    // Refresh YouTube channels with bounded concurrency.
    stream::iter(youtube_items)
        .for_each_concurrent(Some(REFRESH_CONCURRENCY), |item| async move {
            let _lock = FeedLock::acquire(FeedLock::key(&item.feed_url, None, None)).await;
            refresh_youtube_item(state, &item).await;
        })
        .await;

    let touched = touched.into_inner().unwrap();
    for user_id in &touched {
        if let Err(e) = state.db_pool.update_user_playlist_counts(*user_id).await {
            warn!("Updating playlist counts for user {} failed: {}", user_id, e);
        }
    }

    feeds
}

// Sync one user with their external server through the sync engine. Users of the internal
// server only have nothing to sync: it works on the same database.
async fn run_admin_gpodder_sync(state: &AppState, user_id: i32) -> AppResult<SyncResult> {
    let summary = engine::run_for_user(&state.db_pool, &state.task_spawner, user_id, RunMode::Incremental).await?;
    Ok(summary
        .map(|s| SyncResult {
            synced_podcasts: (s.subscriptions_queued + s.subscriptions_removed) as i32,
            synced_episodes: s.actions_applied as i32,
        })
        .unwrap_or(SyncResult { synced_podcasts: 0, synced_episodes: 0 }))
}

// User-specific refresh via WebSocket with real-time progress
pub async fn websocket_refresh_episodes(
    ws: WebSocketUpgrade,
    Path(user_id): Path<i32>,
    Query(query): Query<RefreshQuery>,
    State(state): State<AppState>,
) -> Result<Response, AppError> {
    // Validate API key
    let api_key = query.api_key.ok_or_else(|| AppError::unauthorized("Missing API key"))?;
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only refresh your own podcasts"));
    }

    let nextcloud_refresh = query.nextcloud_refresh.unwrap_or(false);

    Ok(ws.on_upgrade(move |socket| {
        handle_refresh_websocket(socket, user_id, nextcloud_refresh, state)
    }))
}

async fn handle_refresh_websocket(
    socket: WebSocket,
    user_id: i32,
    nextcloud_refresh: bool,
    state: AppState,
) {
    // Check if refresh is already running for this user
    {
        let locks = USER_LOCKS.read().await;
        if locks.contains_key(&user_id) {
            let _ = send_error_and_close(socket, "Refresh job already running for this user.").await;
            return;
        }
    }

    // Create user lock
    let user_lock = {
        let mut locks = USER_LOCKS.write().await;
        let lock = Arc::new(Mutex::new(()));
        locks.insert(user_id, lock.clone());
        lock
    };

    let _guard = user_lock.lock().await;

    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<RefreshMessage>(100);

    // Add WebSocket to active connections
    {
        let mut connections = ACTIVE_WEBSOCKETS.write().await;
        connections.entry(user_id).or_insert_with(Vec::new).push(tx.clone());
    }

    // Task to send messages through WebSocket
    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            let json = serde_json::to_string(&message).unwrap_or_else(|_| "{}".to_string());
            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
        // When the channel is closed (refresh complete), close the websocket
        let _ = sender.close().await;
    });

    // Task to handle incoming WebSocket messages (keep alive)
    let receive_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(_)) => {
                    // Keep connection alive
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    });

    // Main refresh task
    let refresh_task = tokio::spawn({
        let state = state.clone();
        let tx = tx.clone();
        async move {
            if let Err(e) = run_refresh_process(user_id, nextcloud_refresh, tx.clone(), state).await {
                let _ = tx.send(RefreshMessage::Error { 
                    detail: format!("Error during refresh: {}", e) 
                }).await;
            }
            // Signal completion by dropping the sender
            drop(tx);
        }
    });

    // Wait for any task to complete
    tokio::select! {
        _ = send_task => {},
        _ = receive_task => {},
        _ = refresh_task => {
            // Refresh task completed - websocket will be closed when channel closes
        },
    }

    // Cleanup
    {
        let mut locks = USER_LOCKS.write().await;
        locks.remove(&user_id);
    }

    {
        let mut connections = ACTIVE_WEBSOCKETS.write().await;
        if let Some(user_connections) = connections.get_mut(&user_id) {
            user_connections.retain(|conn| !conn.is_closed());
            if user_connections.is_empty() {
                connections.remove(&user_id);
            }
        }
    }
}

async fn send_error_and_close(mut socket: WebSocket, error: &str) -> Result<(), AppError> {
    let error_msg = RefreshMessage::Error { detail: error.to_string() };
    let json = serde_json::to_string(&error_msg)?;
    let _ = socket.send(Message::Text(json.into())).await;
    let _ = socket.close().await;
    Ok(())
}

/// A user's manual refresh (what the refresh websocket runs), streaming progress and new
/// episodes into `tx`.
pub async fn run_refresh_process(
    user_id: i32,
    nextcloud_refresh: bool,
    tx: tokio::sync::mpsc::Sender<RefreshMessage>,
    state: AppState,
) -> AppResult<()> {
    info!("Starting refresh process for user_id: {}, nextcloud_refresh: {}", user_id, nextcloud_refresh);
    
    // External sync runs alongside the refresh, so a slow sync server can't hold it up. Its
    // result goes into the final message (clients stop reading there).
    let sync = if nextcloud_refresh && state.db_pool.sync_targets_for_user(user_id).await?.iter().any(|t| t.enabled) {
        let state = state.clone();
        Some(tokio::spawn(async move { run_admin_gpodder_sync(&state, user_id).await }))
    } else {
        None
    };

    // Get total podcast count for progress tracking
    let total_podcasts = state.db_pool.get_user_podcast_count(user_id).await?;
    debug!("Found {} podcasts to refresh for user {}", total_podcasts, user_id);
    
    // Send initial progress
    let _ = tx.send(RefreshMessage::Status(RefreshStatus {
        progress: RefreshProgress {
            current: 0,
            total: total_podcasts,
            current_podcast: "Starting podcast refresh...".to_string(),
        },
    })).await;

    // Get user's podcasts for refresh
    let podcasts = state.db_pool.get_user_podcasts_for_refresh(user_id).await?;
    info!("Retrieved {} podcast details for refresh", podcasts.len());
    
    // Refresh the user's podcasts with bounded concurrency (previously sequential with a 100ms
    // sleep between each). Progress + new episodes are streamed over the websocket as each podcast
    // finishes; the shared refresh_single_podcast path applies cutoff, conditional-GET validators,
    // and failure tracking identically to the background refresh.
    use futures::stream::StreamExt;
    use std::sync::atomic::{AtomicU32, Ordering};

    let total = podcasts.len() as u32;
    let completed = AtomicU32::new(0);
    let state_ref = &state;
    let completed_ref = &completed;
    let tx_ref = &tx;

    let outcomes: Vec<(bool, usize)> = futures::stream::iter(podcasts.into_iter())
        .map(|podcast| async move {
            let result = refresh_single_podcast(state_ref, &podcast, user_id, nextcloud_refresh).await;
            let done = completed_ref.fetch_add(1, Ordering::SeqCst) + 1;

            let _ = tx_ref
                .send(RefreshMessage::Status(RefreshStatus {
                    progress: RefreshProgress {
                        current: done,
                        total,
                        current_podcast: podcast.name.clone(),
                    },
                }))
                .await;

            match result {
                Ok(new_episodes) => {
                    let count = new_episodes.len();
                    for episode in new_episodes {
                        let _ = tx_ref
                            .send(RefreshMessage::NewEpisode(NewEpisode { new_episode: episode }))
                            .await;
                    }
                    (true, count)
                }
                Err(e) => {
                    warn!("Error refreshing podcast '{}' (ID: {}): {}", podcast.name, podcast.id, e);
                    (false, 0)
                }
            }
        })
        .buffer_unordered(REFRESH_CONCURRENCY)
        .collect()
        .await;

    let successful_refreshes = outcomes.iter().filter(|(ok, _)| *ok).count();
    let failed_refreshes = outcomes.len() - successful_refreshes;
    let total_new_episodes: usize = outcomes.iter().map(|(_, n)| n).sum();

    info!("Refresh completed for user {}: {}/{} podcasts successful, {} failed, {} total new episodes", 
        user_id, successful_refreshes, total_podcasts, failed_refreshes, total_new_episodes);

    if total_new_episodes > 0 {
        if let Err(e) = state.db_pool.update_user_playlist_counts(user_id).await {
            warn!("Updating playlist counts for user {} failed: {}", user_id, e);
        }
    }

    let mut summary = format!(
        "Refresh completed: {}/{} successful, {} new episodes",
        successful_refreshes, total_podcasts, total_new_episodes
    );
    if let Some(mut sync) = sync {
        summary.push_str(&match tokio::time::timeout(SYNC_REPORT_WAIT, &mut sync).await {
            Ok(Ok(Ok(r))) => format!("; sync: {} podcasts, {} episodes", r.synced_podcasts, r.synced_episodes),
            Ok(Ok(Err(e))) => format!("; sync failed: {}", e),
            Ok(Err(_)) => "; sync failed".to_string(),
            Err(_) => "; sync still running in the background".to_string(),
        });
    }
    let _ = tx.send(RefreshMessage::Status(RefreshStatus {
        progress: RefreshProgress {
            current: total_podcasts,
            total: total_podcasts,
            current_podcast: summary,
        },
    })).await;

    Ok(())
}

// How long the manual refresh waits, once done, for the sync it started to report.
const SYNC_REPORT_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Debug, Clone)]
pub struct PodcastForRefresh {
    pub id: i32,
    pub feed_id: Option<i32>,
    pub name: String,
    pub feed_url: String,
    pub artwork_url: Option<String>,
    pub is_youtube: bool,
    pub auto_download: bool,
    pub auto_queue: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub feed_cutoff_days: Option<i32>,
    pub user_id: i32,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_hash: Option<String>,
    pub refresh_interval: Option<i32>,
    /// The background refresh succeeded on this feed in the last 2 minutes.
    pub recently_refreshed: bool,
    /// The host's Retry-After hold hasn't expired yet.
    pub throttled: bool,
}

/// Refresh one of the user's podcasts now (the feed problems list's "Refresh now"). Same path
/// and skips as the manual refresh. `Ok(false)` when the user has no such podcast.
pub async fn refresh_podcast_now(state: &AppState, user_id: i32, podcast_id: i32) -> AppResult<bool> {
    let podcasts = state.db_pool.get_user_podcasts_for_refresh(user_id).await?;
    let Some(podcast) = podcasts.iter().find(|p| p.id == podcast_id) else { return Ok(false) };
    // A failure is recorded and shown with the feed; the caller reads the new state.
    if let Err(e) = refresh_single_podcast(state, podcast, user_id, false).await {
        debug!("Refreshing podcast {} now failed: {}", podcast_id, e);
    }
    Ok(true)
}

async fn refresh_single_podcast(
    state: &AppState,
    podcast: &PodcastForRefresh,
    user_id: i32,
    _nextcloud_refresh: bool,
) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
    info!("Refreshing podcast: {} (ID: {})", podcast.name, podcast.id);

    if podcast.is_youtube {
        refresh_youtube_channel(state, podcast).await
    } else {
        refresh_rss_feed(state, podcast, user_id).await
    }
}

async fn refresh_rss_feed(
    state: &AppState,
    podcast: &PodcastForRefresh,
    user_id: i32,
) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
    debug!("Refreshing RSS feed for podcast: {}", podcast.name);

    if podcast.feed_url.starts_with("local://") {
        return Ok(Vec::new());
    }
    // Nothing can have changed since a refresh moments ago, and a throttled host asked us to
    // wait; both count as checked.
    if podcast.recently_refreshed || podcast.throttled {
        debug!(
            "Skipping manual refresh of podcast {} (recently refreshed: {}, throttled: {})",
            podcast.id, podcast.recently_refreshed, podcast.throttled
        );
        return Ok(Vec::new());
    }

    // Same path as the scheduled refresh: conditional GET + body hash, the podcast's
    // FeedCutoffDays, auto-download/queue/transcribe, failure and Retry-After tracking.
    let item = PodcastRefreshItem {
        podcast_id: podcast.id,
        feed_id: podcast.feed_id,
        feed_url: podcast.feed_url.clone(),
        artwork_url: podcast.artwork_url.clone(),
        auto_download: podcast.auto_download,
        auto_queue: podcast.auto_queue,
        username: podcast.username.clone(),
        password: podcast.password.clone(),
        is_youtube: false,
        user_id,
        feed_cutoff: podcast.feed_cutoff_days,
        etag: podcast.etag.clone(),
        last_modified: podcast.last_modified.clone(),
        content_hash: podcast.content_hash.clone(),
        refresh_interval: podcast.refresh_interval,
    };
    let Some(_lock) = FeedLock::try_acquire(FeedLock::key(&item.feed_url, item.username.as_deref(), item.password.as_deref())) else {
        debug!("Skipping manual refresh of podcast {}: it's being refreshed right now", podcast.id);
        return Ok(Vec::new());
    };
    // The feed is refreshed for everyone subscribed to it: its schedule and validators are
    // shared, so refreshing one subscription alone would leave the others behind.
    let mut group = match podcast.feed_id {
        Some(feed_id) => load_refresh_items(state, None, Some(&[feed_id]), true).await?,
        None => Vec::new(),
    };
    if !group.iter().any(|i| i.podcast_id == item.podcast_id) {
        group.insert(0, item);
    }
    let mut new_episodes = refresh_feed_group(state, &group).await?;
    new_episodes.retain(|e| e.podcastid == podcast.id);

    if !new_episodes.is_empty() {
        info!("Refreshed podcast '{}' for user {}: {} new episodes", podcast.name, user_id, new_episodes.len());
    }

    Ok(new_episodes)
}

async fn refresh_youtube_channel(
    state: &AppState,
    podcast: &PodcastForRefresh,
) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
    debug!("Refreshing YouTube channel: {}", podcast.name);

    let channel_id = if podcast.feed_url.contains("channel/") {
        podcast.feed_url.split("channel/").nth(1).unwrap_or(&podcast.feed_url).split('/').next().unwrap_or(&podcast.feed_url).split('?').next().unwrap_or(&podcast.feed_url)
    } else {
        &podcast.feed_url
    };

    let Some(_lock) = FeedLock::try_acquire(FeedLock::key(&podcast.feed_url, None, None)) else {
        debug!("Skipping manual refresh of channel {}: it's being refreshed right now", podcast.id);
        return Ok(Vec::new());
    };
    let ids = [podcast.id];
    match crate::handlers::youtube::process_youtube_channel(
        podcast.id,
        channel_id,
        podcast.feed_cutoff_days.unwrap_or(30),
        state,
    ).await {
        Ok(_) => {
            info!("Successfully refreshed YouTube channel: {}", podcast.name);
            mark_refreshed(state, &ids, youtube_schedule(state, podcast.id).await).await;
            // YouTube new-episode surfacing over the websocket is not yet wired up
            // (process_youtube_channel does not return the inserted rows); the channel is still
            // refreshed and new videos appear on the next list load.
            Ok(Vec::new())
        }
        Err(e) => {
            warn!("Error refreshing YouTube channel {}: {}", podcast.name, e);
            mark_failed(state, &ids, FeedErrorKind::of(&e), &e.to_string(), None).await;
            Err(e)
        }
    }
}

// Define sync result structure to match our database return type
#[derive(Debug)]
pub struct SyncResult {
    pub synced_podcasts: i32,
    pub synced_episodes: i32,
}

/// Refresh every feed now, whether it's due or not.
pub async fn refresh_pods_admin_internal(state: &AppState) -> AppResult<()> {
    refresh_all_podcasts_background(state).await
}

// Separate endpoint for actual Nextcloud refresh (different from GPodder)
#[utoipa::path(
    get,
    path = "/refresh_nextcloud_subscriptions",
    tag = "tasks",
    summary = "Refresh Nextcloud subscriptions",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Refresh triggered", body = serde_json::Value),
    ),
)]
pub async fn refresh_nextcloud_subscriptions_admin(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    // Server-wide fan-out over every user: admin (or background key) only.
    crate::handlers::require_admin(&state, &headers).await?;
    info!("Starting admin Nextcloud sync process for all users");
    
    let state_clone = state.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_nextcloud_subscriptions".to_string(),
        0, // System user
        move |reporter| async move {
            let state = state_clone;
            reporter.update_progress(10.0, Some("Starting Nextcloud sync for all users...".to_string())).await?;
            
            // Every user syncing with Nextcloud
            let nextcloud_users = state.db_pool.sync_target_users(Some(TargetKind::Nextcloud)).await
                .map_err(|e| AppError::internal(&format!("Failed to get Nextcloud users: {}", e)))?;
            
            debug!("Found {} users with Nextcloud sync enabled", nextcloud_users.len());
            
            let mut successful_syncs = 0;
            let mut failed_syncs = 0;
            
            let total_users = nextcloud_users.len();
            if total_users == 0 {
                reporter.update_progress(100.0, Some("No users with Nextcloud sync found".to_string())).await?;
                return Ok(serde_json::json!({
                    "status": "No users found",
                    "successful_syncs": 0,
                    "failed_syncs": 0,
                    "total_users": 0
                }));
            }
            
            for (index, user_id) in nextcloud_users.iter().enumerate() {
                let progress = 10.0 + ((index as f64 / total_users as f64) * 80.0);
                reporter.update_progress(progress, Some(format!("Running Nextcloud sync for user {}", user_id))).await?;
                
                match run_admin_gpodder_sync(&state, *user_id).await {
                    Ok(_) => successful_syncs += 1,
                    Err(e) => {
                        failed_syncs += 1;
                        debug!("Nextcloud sync failed for user {}: {}", user_id, e);
                    }
                }
            }
            
            reporter.update_progress(100.0, Some(format!("Nextcloud sync completed: {}/{} users successful", successful_syncs, total_users))).await?;
            
            Ok(serde_json::json!({
                "status": "Nextcloud sync completed successfully",
                "successful_syncs": successful_syncs,
                "failed_syncs": failed_syncs,
                "total_users": total_users
            }))
        },
    ).await?;

    Ok(axum::Json(serde_json::json!({
        "detail": "Nextcloud sync initiated",
        "task_id": task_id
    })))
}


