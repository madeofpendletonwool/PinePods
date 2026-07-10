use axum::{
    extract::{Query, Path, State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{WebSocket, Message};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use sqlx::Row;
use tracing::{debug, info, warn};

use crate::{
    error::{AppError, AppResult},
    handlers::check_user_access,
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
) -> Result<axum::Json<serde_json::Value>, AppError> {
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
) -> Result<axum::Json<serde_json::Value>, AppError> {
    info!("Starting admin gPodder sync process for all users");
    
    let state_clone = state.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_gpodder_subscriptions".to_string(),
        0, // System user
        move |reporter| async move {
            let state = state_clone;
            reporter.update_progress(10.0, Some("Starting gPodder sync for all users...".to_string())).await?;
            
            // Get all users who have gPodder sync enabled
            let gpodder_users = state.db_pool.get_all_users_with_gpodder_sync().await
                .map_err(|e| AppError::internal(&format!("Failed to get gPodder users: {}", e)))?;
            
            debug!("Found {} users with gPodder sync enabled", gpodder_users.len());
            
            let mut successful_syncs = 0;
            let mut failed_syncs = 0;
            let mut total_synced_podcasts = 0;
            
            for (index, user_id) in gpodder_users.iter().enumerate() {
                let progress = 10.0 + (80.0 * (index as f64) / (gpodder_users.len() as f64));
                reporter.update_progress(progress, Some(format!("Syncing user {}/{}", index + 1, gpodder_users.len()))).await?;
                
                info!("Running gPodder sync for user {} ({}/{})", user_id, index + 1, gpodder_users.len());
                
                // Get user's sync type
                let gpodder_status = state.db_pool.gpodder_get_status(*user_id).await
                    .map_err(|e| AppError::internal(&format!("Failed to get status for user {}: {}", user_id, e)))?;
                
                if gpodder_status.sync_type != "None" && !gpodder_status.sync_type.is_empty() {
                    match run_admin_gpodder_sync(&state, *user_id, &gpodder_status.sync_type).await {
                        Ok(sync_result) => {
                            successful_syncs += 1;
                            total_synced_podcasts += sync_result.synced_podcasts;
                            info!("gPodder sync successful for user {}: {} podcasts", 
                                user_id, sync_result.synced_podcasts);
                        }
                        Err(e) => {
                            failed_syncs += 1;
                            warn!("gPodder sync failed for user {}: {}", user_id, e);
                            tracing::error!("gPodder sync failed for user {}: {}", user_id, e);
                            // Continue with other users
                        }
                    }
                } else {
                    info!("gPodder sync not properly configured for user {}", user_id);
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

// One podcast row to refresh. Defined at module scope so the grouped/concurrent helpers below
// can take owned values without borrowing the DB connection.
#[derive(Clone)]
struct PodcastRefreshItem {
    podcast_id: i32,
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
            .queue_episode(episode.episodeid, user_id, episode.is_youtube)
            .await
        {
            Ok(()) => debug!("Auto-queued episode {} for user {}", episode.episodeid, user_id),
            Err(e) => warn!("Failed to auto-queue episode {}: {}", episode.episodeid, e),
        }
    }
}

/// Refresh one feed that may be shared by several subscriber podcasts (cross-user dedup). The feed
/// is fetched (with conditional GET) and parsed ONCE, then applied to every subscriber's podcast
/// row. Cache validators and success/failure state are recorded for the whole group.
async fn refresh_feed_group(state: &AppState, group: &[PodcastRefreshItem]) -> usize {
    let rep = &group[0];
    let ids: Vec<i32> = group.iter().map(|i| i.podcast_id).collect();
    let mut total_new = 0usize;

    // Reuse any stored validator from the group (they converge after the first unified cycle).
    let etag = group.iter().find_map(|i| i.etag.clone());
    let last_modified = group.iter().find_map(|i| i.last_modified.clone());

    match state
        .db_pool
        .fetch_feed_conditional(
            &rep.feed_url,
            rep.username.as_deref(),
            rep.password.as_deref(),
            etag.as_deref(),
            last_modified.as_deref(),
        )
        .await
    {
        Ok(crate::database::FeedFetch::NotModified) => {
            debug!("Feed unchanged (304), skipping parse: {}", rep.feed_url);
            let _ = state.db_pool.record_refresh_success(&ids).await;
        }
        Ok(crate::database::FeedFetch::Fetched { body, etag, last_modified }) => {
            let artwork = rep.artwork_url.clone().unwrap_or_default();
            match state.db_pool.parse_feed_body(&body, rep.podcast_id, &artwork).await {
                Ok(parsed) => {
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
                                    total_new += new_eps.len();
                                }
                                queue_auto_downloads(state, item.user_id, item.auto_download, &new_eps).await;
                                queue_auto_new_episodes(state, item.user_id, item.auto_queue, &new_eps).await;
                                // Auto-transcribe new episodes for opted-in podcasts (independent
                                // of downloads — the pipeline fetches audio on demand).
                                for ep in &new_eps {
                                    crate::services::transcription::maybe_transcribe_episode(state.db_pool.clone(), ep.episodeid);
                                }
                            }
                            Err(e) => warn!("Error applying feed to podcast {}: {}", item.podcast_id, e),
                        }
                    }
                    let _ = state
                        .db_pool
                        .store_feed_cache_validators(&ids, etag.as_deref(), last_modified.as_deref())
                        .await;
                    let _ = state.db_pool.record_refresh_success(&ids).await;
                }
                Err(e) => {
                    warn!("Error parsing feed {}: {}", rep.feed_url, e);
                    let _ = state.db_pool.record_refresh_failure(&ids, &e.to_string()).await;
                }
            }
        }
        Err(e) => {
            warn!("Error fetching feed {}: {}", rep.feed_url, e);
            let _ = state.db_pool.record_refresh_failure(&ids, &e.to_string()).await;
        }
    }

    total_new
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
            let _ = state.db_pool.record_refresh_success(&ids).await;
        }
        Err(e) => {
            warn!("Error refreshing YouTube channel {}: {}", item.podcast_id, e);
            let _ = state.db_pool.record_refresh_failure(&ids, &e.to_string()).await;
        }
    }
}

// Background refresh function that matches Python refresh_pods exactly - NO WebSocket
async fn refresh_all_podcasts_background(state: &AppState) -> AppResult<()> {
    info!("Running refresh");
    
    // Get ALL podcasts from ALL users - matches Python exactly
    // Handle the different database types properly
    let total_podcasts = match &state.db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            let count_row = sqlx::query(r#"SELECT COUNT(*) as total FROM "Podcasts""#)
                .fetch_one(pool)
                .await?;
            count_row.try_get::<i64, _>("total")? as usize
        }
        crate::database::DatabasePool::MySQL(pool) => {
            let count_row = sqlx::query("SELECT COUNT(*) as total FROM Podcasts")
                .fetch_one(pool)
                .await?;
            count_row.try_get::<i64, _>("total")? as usize
        }
    };
    
    // Collect podcast rows into owned structs, skipping feeds that are in failure backoff. The
    // backoff window grows with the consecutive-failure count (linear, capped at 24h) and is
    // computed in SQL so we don't have to read/normalize timestamps across Postgres/MySQL.
    let mut refresh_items: Vec<PodcastRefreshItem> = Vec::with_capacity(total_podcasts);

    match &state.db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            let rows = sqlx::query(
                r#"SELECT podcastid, feedurl, artworkurl, autodownload, autoqueue, username, password,
                          isyoutubechannel, userid, feedcutoffdays, feedetag, feedlastmodified
                   FROM "Podcasts"
                   WHERE COALESCE(refreshpodcast, TRUE) = TRUE
                     AND (COALESCE(consecutivefailures, 0) = 0
                          OR lastrefreshattempt IS NULL
                          OR lastrefreshattempt < NOW() - (INTERVAL '1 minute' * (30 * LEAST(COALESCE(consecutivefailures, 0), 48))))"#
            )
            .fetch_all(pool)
            .await?;

            for result in rows {
                let podcast_id: i32 = result.try_get("podcastid")?;
                refresh_items.push(PodcastRefreshItem {
                    podcast_id,
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
                });
            }
        }
        crate::database::DatabasePool::MySQL(pool) => {
            let rows = sqlx::query(
                "SELECT PodcastID, FeedURL, ArtworkURL, AutoDownload, AutoQueue, Username, Password,
                        IsYouTubeChannel, UserID, FeedCutoffDays, FeedETag, FeedLastModified
                 FROM Podcasts
                 WHERE COALESCE(RefreshPodcast, 1) = 1
                   AND (COALESCE(ConsecutiveFailures, 0) = 0
                        OR LastRefreshAttempt IS NULL
                        OR LastRefreshAttempt < NOW() - INTERVAL (30 * LEAST(COALESCE(ConsecutiveFailures, 0), 48)) MINUTE)"
            )
            .fetch_all(pool)
            .await?;

            for result in rows {
                let podcast_id: i32 = result.try_get("PodcastID")?;
                refresh_items.push(PodcastRefreshItem {
                    podcast_id,
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
                });
            }
        }
    }

    // Partition into YouTube items (refreshed per-podcast) and RSS feeds grouped by
    // (feed_url, username, password) so a feed subscribed by many users is fetched + parsed ONCE.
    use std::collections::HashMap;
    let mut youtube_items: Vec<PodcastRefreshItem> = Vec::new();
    let mut feed_groups: HashMap<(String, Option<String>, Option<String>), Vec<PodcastRefreshItem>> = HashMap::new();

    for item in refresh_items {
        if item.feed_url.starts_with("local://") {
            debug!("Skipping local podcast {} - not an RSS feed", item.podcast_id);
            continue;
        }
        if item.is_youtube {
            youtube_items.push(item);
        } else {
            let key = (item.feed_url.clone(), item.username.clone(), item.password.clone());
            feed_groups.entry(key).or_default().push(item);
        }
    }

    use futures::stream::{self, StreamExt};

    // Refresh unique RSS feeds with bounded concurrency.
    let groups: Vec<Vec<PodcastRefreshItem>> = feed_groups.into_values().collect();
    let subscription_count: usize = groups.iter().map(|g| g.len()).sum();
    info!(
        "Running refresh: {} unique RSS feeds ({} subscriptions), {} YouTube channels (concurrency={})",
        groups.len(),
        subscription_count,
        youtube_items.len(),
        REFRESH_CONCURRENCY
    );
    stream::iter(groups)
        .for_each_concurrent(Some(REFRESH_CONCURRENCY), |group| async move {
            refresh_feed_group(state, &group).await;
        })
        .await;

    // Refresh YouTube channels with bounded concurrency.
    stream::iter(youtube_items)
        .for_each_concurrent(Some(REFRESH_CONCURRENCY), |item| async move {
            refresh_youtube_item(state, &item).await;
        })
        .await;

    // Run auto-complete check for all users with auto-complete enabled after episode refresh
    info!("Running auto-complete threshold check for all users...");
    match state.db_pool.get_users_with_auto_complete_enabled().await {
        Ok(users_with_auto_complete) => {
            let mut total_completed = 0;
            for user_auto_complete in users_with_auto_complete {
                match state.db_pool.auto_complete_user_episodes(
                    user_auto_complete.user_id, 
                    user_auto_complete.auto_complete_seconds
                ).await {
                    Ok(completed_count) => {
                        if completed_count > 0 {
                            info!("Auto-completed {} episodes for user {} (threshold: {}s)", 
                                    completed_count, user_auto_complete.user_id, user_auto_complete.auto_complete_seconds);
                        }
                        total_completed += completed_count;
                    }
                    Err(e) => {
                        warn!("Failed to run auto-complete for user {}: {}", user_auto_complete.user_id, e);
                    }
                }
            }
            if total_completed > 0 {
                info!("Auto-complete threshold check completed: {} total episodes marked complete", total_completed);
            } else {
                info!("Auto-complete threshold check completed: no episodes needed completion");
            }
        }
        Err(e) => {
            warn!("Failed to get users with auto-complete enabled: {}", e);
        }
    }
    
    info!("Refresh completed");
    Ok(())
}

// Helper function for admin gPodder sync
async fn run_admin_gpodder_sync(state: &AppState, user_id: i32, sync_type: &str) -> AppResult<SyncResult> {
    match sync_type {
        "nextcloud" => {
            match state.db_pool.sync_with_nextcloud_for_user(user_id).await {
                Ok(success) => {
                    if success {
                        Ok(SyncResult { synced_podcasts: 1, synced_episodes: 0 })
                    } else {
                        Ok(SyncResult { synced_podcasts: 0, synced_episodes: 0 })
                    }
                }
                Err(e) => Err(e)
            }
        }
        "gpodder" | "external" | "both" => {
            match state.db_pool.gpodder_sync(user_id).await {
                Ok(sync_result) => {
                    Ok(SyncResult {
                        synced_podcasts: sync_result.synced_podcasts,
                        synced_episodes: sync_result.synced_episodes,
                    })
                }
                Err(e) => Err(e)
            }
        }
        _ => Ok(SyncResult { synced_podcasts: 0, synced_episodes: 0 })
    }
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

async fn run_refresh_process(
    user_id: i32,
    nextcloud_refresh: bool,
    tx: tokio::sync::mpsc::Sender<RefreshMessage>,
    state: AppState,
) -> AppResult<()> {
    info!("Starting refresh process for user_id: {}, nextcloud_refresh: {}", user_id, nextcloud_refresh);
    
    // PRE-REFRESH GPODDER SYNC - matches Python implementation exactly
    if nextcloud_refresh {
        info!("Pre-refresh gPodder sync requested for user {}", user_id);
        
        let _ = tx.send(RefreshMessage::Status(RefreshStatus {
            progress: RefreshProgress {
                current: 0,
                total: 1,
                current_podcast: "Checking gPodder sync settings...".to_string(),
            },
        })).await;

        // Check if user has gPodder sync configured
        let gpodder_status = state.db_pool.gpodder_get_status(user_id).await?;
        
        if gpodder_status.sync_type != "None" && !gpodder_status.sync_type.is_empty() {
            info!("gPodder sync is enabled for user {}, sync_type: {}", user_id, gpodder_status.sync_type);
            
            let _ = tx.send(RefreshMessage::Status(RefreshStatus {
                progress: RefreshProgress {
                    current: 0,
                    total: 1,
                    current_podcast: format!("Syncing with gPodder ({})...", gpodder_status.sync_type),
                },
            })).await;

            match handle_gpodder_sync(&state, user_id, &gpodder_status.sync_type).await {
                Ok(sync_result) => {
                    info!("gPodder sync successful for user {}: {} podcasts, {} episodes", 
                        user_id, sync_result.synced_podcasts, sync_result.synced_episodes);
                    
                    let _ = tx.send(RefreshMessage::Status(RefreshStatus {
                        progress: RefreshProgress {
                            current: 0,
                            total: 1,
                            current_podcast: format!("gPodder sync completed: {} podcasts, {} episodes", 
                                sync_result.synced_podcasts, sync_result.synced_episodes),
                        },
                    })).await;
                }
                Err(e) => {
                    warn!("gPodder sync failed for user {}: {}", user_id, e);
                    tracing::error!("gPodder sync failed for user {}: {}", user_id, e);
                    
                    let _ = tx.send(RefreshMessage::Status(RefreshStatus {
                        progress: RefreshProgress {
                            current: 0,
                            total: 1,
                            current_podcast: format!("gPodder sync failed: {}", e),
                        },
                    })).await;
                    
                    // Continue with regular refresh even if gPodder sync fails
                }
            }
        } else {
            info!("gPodder sync not enabled for user {} (enabled: {}, type: {})", 
                user_id, gpodder_status.sync_type != "None" && !gpodder_status.sync_type.is_empty(), gpodder_status.sync_type);
        }
    }

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

    // Final completion summary - matches Python logging
    warn!("Refresh completed for user {}: {}/{} podcasts successful, {} failed, {} total new episodes", 
        user_id, successful_refreshes, total_podcasts, failed_refreshes, total_new_episodes);

    let _ = tx.send(RefreshMessage::Status(RefreshStatus {
        progress: RefreshProgress {
            current: total_podcasts,
            total: total_podcasts,
            current_podcast: format!("Refresh completed: {}/{} successful, {} new episodes", 
                successful_refreshes, total_podcasts, total_new_episodes),
        },
    })).await;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct PodcastForRefresh {
    pub id: i32,
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

    let ids = [podcast.id];
    let artwork = podcast.artwork_url.as_deref().unwrap_or("");

    // A user-initiated refresh always does a full fetch (no stored validators passed) — the user
    // explicitly asked to check for new episodes — but we still capture fresh ETag/Last-Modified
    // so the scheduled background refresh can revalidate cheaply afterwards. The whole feed is
    // fetched + parsed once and applied through the shared dedup/insert path, with the podcast's
    // FeedCutoffDays honored (previously ignored for RSS).
    let new_episodes = match state
        .db_pool
        .fetch_feed_conditional(
            &podcast.feed_url,
            podcast.username.as_deref(),
            podcast.password.as_deref(),
            None,
            None,
        )
        .await
    {
        Ok(crate::database::FeedFetch::NotModified) => {
            let _ = state.db_pool.record_refresh_success(&ids).await;
            Vec::new()
        }
        Ok(crate::database::FeedFetch::Fetched { body, etag, last_modified }) => {
            let parsed = state.db_pool.parse_feed_body(&body, podcast.id, artwork).await?;
            let new_eps = state
                .db_pool
                .apply_parsed_episodes(podcast.id, &parsed, artwork, podcast.feed_cutoff_days, true)
                .await?;
            let _ = state
                .db_pool
                .store_feed_cache_validators(&ids, etag.as_deref(), last_modified.as_deref())
                .await;
            let _ = state.db_pool.record_refresh_success(&ids).await;
            new_eps
        }
        Err(e) => {
            let _ = state.db_pool.record_refresh_failure(&ids, &e.to_string()).await;
            return Err(e);
        }
    };

    queue_auto_downloads(state, user_id, podcast.auto_download, &new_episodes).await;
    queue_auto_new_episodes(state, user_id, podcast.auto_queue, &new_episodes).await;

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

    let ids = [podcast.id];
    match crate::handlers::youtube::process_youtube_channel(
        podcast.id,
        channel_id,
        podcast.feed_cutoff_days.unwrap_or(30),
        state,
    ).await {
        Ok(_) => {
            info!("Successfully refreshed YouTube channel: {}", podcast.name);
            let _ = state.db_pool.record_refresh_success(&ids).await;
            // YouTube new-episode surfacing over the websocket is not yet wired up
            // (process_youtube_channel does not return the inserted rows); the channel is still
            // refreshed and new videos appear on the next list load.
            Ok(Vec::new())
        }
        Err(e) => {
            warn!("Error refreshing YouTube channel {}: {}", podcast.name, e);
            let _ = state.db_pool.record_refresh_failure(&ids, &e.to_string()).await;
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

async fn handle_gpodder_sync(state: &AppState, user_id: i32, sync_type: &str) -> AppResult<SyncResult> {
    info!("Starting gPodder sync for user {}, sync_type: {}", user_id, sync_type);
    
    // Determine which sync function to call based on sync type - matches Python logic exactly
    match sync_type {
        "nextcloud" => {
            info!("Performing Nextcloud gPodder sync for user {}", user_id);
            
            // Use the nextcloud sync functionality - this handles the /index.php/apps/gpoddersync endpoints
            match state.db_pool.sync_with_nextcloud_for_user(user_id).await {
                Ok(success) => {
                    if success {
                        info!("Nextcloud sync successful for user {}", user_id);
                        Ok(SyncResult { synced_podcasts: 1, synced_episodes: 0 })
                    } else {
                        info!("Nextcloud sync returned false for user {}", user_id);
                        Ok(SyncResult { synced_podcasts: 0, synced_episodes: 0 })
                    }
                }
                Err(e) => {
                    warn!("Nextcloud sync failed for user {}: {}", user_id, e);
                    Err(e)
                }
            }
        }
        "gpodder" | "external" | "both" => {
            info!("Performing standard gPodder sync for user {}, type: {}", user_id, sync_type);
            
            // Use the standard gPodder sync functionality
            match state.db_pool.gpodder_sync(user_id).await {
                Ok(sync_result) => {
                    info!("Standard gPodder sync successful for user {}: {} podcasts, {} episodes", 
                        user_id, sync_result.synced_podcasts, sync_result.synced_episodes);
                    
                    Ok(SyncResult {
                        synced_podcasts: sync_result.synced_podcasts,
                        synced_episodes: sync_result.synced_episodes,
                    })
                }
                Err(e) => {
                    warn!("Standard gPodder sync failed for user {}: {}", user_id, e);
                    Err(e)
                }
            }
        }
        _ => {
            debug!("Unknown sync type '{}' for user {}, skipping sync", sync_type, user_id);
            Ok(SyncResult { synced_podcasts: 0, synced_episodes: 0 })
        }
    }
}

// Internal functions for scheduler (no HTTP context needed)
pub async fn refresh_pods_admin_internal(state: &AppState) -> AppResult<()> {
    tracing::info!("Starting internal podcast refresh (scheduler)");
    refresh_all_podcasts_background(state).await
}

pub async fn refresh_gpodder_subscriptions_admin_internal(state: &AppState) -> AppResult<()> {
    tracing::info!("Starting internal GPodder sync (scheduler)");
    
    // Wait for GPodder service to be ready (5 second delay on startup)
    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    tracing::info!("GPodder service startup delay completed");
    
    // Get all users who have gPodder sync enabled (internal, external, both - NOT nextcloud)
    let gpodder_users = state.db_pool.get_all_users_with_gpodder_sync().await?;
    tracing::info!("Found {} users with GPodder sync enabled", gpodder_users.len());
    
    let mut successful_syncs = 0;
    let mut failed_syncs = 0;
    
    for user_id in gpodder_users.iter() {
        tracing::info!("Running GPodder sync for user {}", user_id);
        
        // Get user's sync type
        let gpodder_status = state.db_pool.gpodder_get_status(*user_id).await?;
        
        // Only sync GPodder types (internal, external, both) - NOT nextcloud
        if gpodder_status.sync_type != "None" && gpodder_status.sync_type != "nextcloud" && !gpodder_status.sync_type.is_empty() {
            match run_admin_gpodder_sync(state, *user_id, &gpodder_status.sync_type).await {
                Ok(_) => {
                    successful_syncs += 1;
                    tracing::info!("GPodder sync successful for user {}", user_id);
                }
                Err(e) => {
                    failed_syncs += 1;
                    tracing::error!("GPodder sync failed for user {}: {}", user_id, e);
                }
            }
        }
    }
    
    tracing::info!("Internal GPodder sync completed: {}/{} users successful ({} failed)",
        successful_syncs, gpodder_users.len(), failed_syncs);
    
    Ok(())
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
) -> Result<axum::Json<serde_json::Value>, AppError> {
    info!("Starting admin Nextcloud sync process for all users");
    
    let state_clone = state.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_nextcloud_subscriptions".to_string(),
        0, // System user
        move |reporter| async move {
            let state = state_clone;
            reporter.update_progress(10.0, Some("Starting Nextcloud sync for all users...".to_string())).await?;
            
            // Get all users who have Nextcloud sync enabled
            let nextcloud_users = state.db_pool.get_all_users_with_nextcloud_sync().await
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
                
                match state.db_pool.sync_with_nextcloud_for_user(*user_id).await {
                    Ok(true) => {
                        successful_syncs += 1;
                        info!("Nextcloud sync successful for user {}", user_id);
                    }
                    Ok(false) => {
                        info!("Nextcloud sync for user {} - no changes", user_id);
                        successful_syncs += 1; // Count as success
                    }
                    Err(e) => {
                        failed_syncs += 1;
                        warn!("Nextcloud sync failed for user {}: {}", user_id, e);
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

pub async fn refresh_nextcloud_subscriptions_admin_internal(state: &AppState) -> AppResult<()> {
    tracing::info!("Starting internal Nextcloud sync (scheduler)");
    
    // Get all users who have Nextcloud sync enabled
    let nextcloud_users = state.db_pool.get_all_users_with_nextcloud_sync().await?;
    tracing::info!("Found {} users with Nextcloud sync enabled", nextcloud_users.len());
    
    let mut successful_syncs = 0;
    let mut failed_syncs = 0;
    
    for user_id in nextcloud_users.iter() {
        tracing::info!("Running Nextcloud sync for user {}", user_id);
        
        match state.db_pool.sync_with_nextcloud_for_user(*user_id).await {
            Ok(true) => {
                successful_syncs += 1;
                tracing::info!("Nextcloud sync successful for user {}", user_id);
            }
            Ok(false) => {
                tracing::info!("Nextcloud sync for user {} - no changes", user_id);
                successful_syncs += 1; // Count as success
            }
            Err(e) => {
                failed_syncs += 1;
                tracing::error!("Nextcloud sync failed for user {}: {}", user_id, e);
            }
        }
    }
    
    tracing::info!("Internal Nextcloud sync completed: {}/{} users successful ({} failed)",
        successful_syncs, nextcloud_users.len(), failed_syncs);
    
    Ok(())
}


