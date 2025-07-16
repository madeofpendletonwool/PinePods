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

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    services::task_manager::TaskManager,
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

// Admin refresh endpoint (background task)
pub async fn refresh_pods_admin(
    State(state): State<AppState>,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    // This would be called by admin/system - spawn background task
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_all_pods".to_string(),
        0, // System user
        move |reporter| async move {
            reporter.update_progress(10.0, Some("Starting system-wide refresh...".to_string())).await?;
            
            // TODO: Implement system-wide podcast refresh
            // This would iterate through all users and refresh their podcasts
            
            reporter.update_progress(100.0, Some("System refresh completed".to_string())).await?;
            Ok(serde_json::json!({"success": true}))
        },
    ).await?;

    Ok(axum::Json(serde_json::json!({
        "detail": "Refresh initiated.",
        "task_id": task_id
    })))
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

    // Check authorization
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    // TODO: Add web key check for elevated access
    if requesting_user_id != user_id {
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
        }
    });

    // Wait for any task to complete
    tokio::select! {
        _ = send_task => {},
        _ = receive_task => {},
        _ = refresh_task => {},
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
    // Get total podcast count
    let total_podcasts = state.db_pool.get_user_podcast_count(user_id).await?;
    
    // Send initial progress
    let _ = tx.send(RefreshMessage::Status(RefreshStatus {
        progress: RefreshProgress {
            current: 0,
            total: total_podcasts,
            current_podcast: "".to_string(),
        },
    })).await;

    // Get user's podcasts for refresh
    let podcasts = state.db_pool.get_user_podcasts_for_refresh(user_id).await?;
    
    let mut current = 0;
    
    for podcast in podcasts {
        current += 1;
        
        // Send progress update
        let _ = tx.send(RefreshMessage::Status(RefreshStatus {
            progress: RefreshProgress {
                current,
                total: total_podcasts,
                current_podcast: podcast.name.clone(),
            },
        })).await;

        // Refresh individual podcast
        match refresh_single_podcast(&state, &podcast, user_id, nextcloud_refresh).await {
            Ok(new_episodes) => {
                // Send new episodes through WebSocket
                for episode in new_episodes {
                    let _ = tx.send(RefreshMessage::NewEpisode(NewEpisode {
                        new_episode: episode,
                    })).await;
                }
            }
            Err(e) => {
                tracing::error!("Error refreshing podcast {}: {}", podcast.id, e);
                // Continue with other podcasts
            }
        }

        // Small delay to prevent overwhelming the system
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Handle GPodder sync if needed
    if nextcloud_refresh {
        let _ = tx.send(RefreshMessage::Status(RefreshStatus {
            progress: RefreshProgress {
                current: total_podcasts,
                total: total_podcasts,
                current_podcast: "Syncing with GPodder...".to_string(),
            },
        })).await;

        if let Err(e) = handle_gpodder_sync(&state, user_id).await {
            tracing::error!("GPodder sync failed for user {}: {}", user_id, e);
        }
    }

    // Final completion message
    let _ = tx.send(RefreshMessage::Status(RefreshStatus {
        progress: RefreshProgress {
            current: total_podcasts,
            total: total_podcasts,
            current_podcast: "Refresh completed".to_string(),
        },
    })).await;

    Ok(())
}

#[derive(Debug)]
pub struct PodcastForRefresh {
    pub id: i32,
    pub name: String,
    pub feed_url: String,
    pub is_youtube: bool,
    pub auto_download: bool,
    pub username: Option<String>,
    pub password: Option<String>,
    pub feed_cutoff_days: Option<i32>,
}

async fn refresh_single_podcast(
    state: &AppState,
    podcast: &PodcastForRefresh,
    user_id: i32,
    _nextcloud_refresh: bool,
) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
    // This is a simplified version - the full implementation would:
    // 1. Fetch the RSS feed
    // 2. Parse episodes
    // 3. Check for new episodes since last refresh
    // 4. Insert new episodes into database
    // 5. Handle YouTube channels differently
    // 6. Handle auto-download if enabled
    // 7. Apply feed cutoff days filter
    
    tracing::info!("Refreshing podcast: {} (ID: {})", podcast.name, podcast.id);
    
    if podcast.is_youtube {
        // Handle YouTube channel refresh
        refresh_youtube_channel(state, podcast, user_id).await
    } else {
        // Handle regular RSS feed refresh
        refresh_rss_feed(state, podcast, user_id).await
    }
}

async fn refresh_rss_feed(
    state: &AppState,
    podcast: &PodcastForRefresh,
    user_id: i32,
) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
    // TODO: Implement RSS feed parsing and episode extraction
    // This would use the feed-rs crate to parse the RSS feed
    // For now, return empty vector
    
    tracing::info!("Refreshing RSS feed for podcast: {}", podcast.name);
    
    // Placeholder implementation
    Ok(Vec::new())
}

async fn refresh_youtube_channel(
    state: &AppState,
    podcast: &PodcastForRefresh,
    user_id: i32,
) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
    // TODO: Implement YouTube channel refresh
    // This would use YouTube Data API to get new videos
    
    tracing::info!("Refreshing YouTube channel: {}", podcast.name);
    
    // Placeholder implementation
    Ok(Vec::new())
}

async fn handle_gpodder_sync(state: &AppState, user_id: i32) -> AppResult<()> {
    // TODO: Implement GPodder synchronization
    // This would:
    // 1. Get user's GPodder settings
    // 2. Determine sync type (nextcloud, gpodder, both, etc.)
    // 3. Sync subscriptions and episode states
    // 4. Handle device management
    
    tracing::info!("Starting GPodder sync for user: {}", user_id);
    
    // Placeholder implementation
    Ok(())
}