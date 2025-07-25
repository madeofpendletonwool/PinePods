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
    handlers::{extract_api_key, validate_api_key, check_user_access},
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

// Admin refresh endpoint (background task) - matches Python refresh_pods function
pub async fn refresh_pods_admin(
    State(state): State<AppState>,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    println!("Starting admin refresh process for all users");
    
    // This would be called by admin/system - spawn background task
    let state_clone = state.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_all_pods".to_string(),
        0, // System user
        move |reporter| async move {
            let state = state_clone;
            reporter.update_progress(10.0, Some("Starting system-wide refresh...".to_string())).await?;
            
            // Get all users who have podcasts to refresh
            let all_users = state.db_pool.get_all_users_with_podcasts().await
                .map_err(|e| AppError::internal(&format!("Failed to get users: {}", e)))?;
            
            println!("Found {} users with podcasts to refresh", all_users.len());
            
            let mut successful_users = 0;
            let mut failed_users = 0;
            let mut total_podcasts_refreshed = 0;
            let mut total_new_episodes = 0;
            
            for (index, user_id) in all_users.iter().enumerate() {
                let progress = 10.0 + (80.0 * (index as f64) / (all_users.len() as f64));
                reporter.update_progress(progress, Some(format!("Refreshing user {}/{}", index + 1, all_users.len()))).await?;
                
                println!("Refreshing podcasts for user {} ({}/{})", user_id, index + 1, all_users.len());
                
                match refresh_user_podcasts_admin(&state, *user_id).await {
                    Ok((podcast_count, episode_count)) => {
                        successful_users += 1;
                        total_podcasts_refreshed += podcast_count;
                        total_new_episodes += episode_count;
                        println!("Successfully refreshed user {}: {} podcasts, {} new episodes", 
                            user_id, podcast_count, episode_count);
                    }
                    Err(e) => {
                        failed_users += 1;
                        println!("Failed to refresh user {}: {}", user_id, e);
                        tracing::error!("Failed to refresh user {}: {}", user_id, e);
                        // Continue with other users
                    }
                }
            }
            
            println!("Admin refresh completed: {}/{} users successful, {} podcasts refreshed, {} new episodes", 
                successful_users, all_users.len(), total_podcasts_refreshed, total_new_episodes);
            
            reporter.update_progress(100.0, Some(format!(
                "System refresh completed: {}/{} users, {} podcasts, {} episodes", 
                successful_users, all_users.len(), total_podcasts_refreshed, total_new_episodes
            ))).await?;
            
            Ok(serde_json::json!({
                "success": true,
                "users_refreshed": successful_users,
                "users_failed": failed_users,
                "total_podcasts": total_podcasts_refreshed,
                "total_new_episodes": total_new_episodes
            }))
        },
    ).await?;

    Ok(axum::Json(serde_json::json!({
        "detail": "System-wide refresh initiated.",
        "task_id": task_id
    })))
}

// Separate endpoint for gPodder refresh (scheduled separately like Python)
pub async fn refresh_nextcloud_subscriptions_admin(
    State(state): State<AppState>,
) -> Result<axum::Json<serde_json::Value>, AppError> {
    println!("Starting admin gPodder sync process for all users");
    
    let state_clone = state.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_nextcloud_subscriptions".to_string(),
        0, // System user
        move |reporter| async move {
            let state = state_clone;
            reporter.update_progress(10.0, Some("Starting gPodder sync for all users...".to_string())).await?;
            
            // Get all users who have gPodder sync enabled
            let gpodder_users = state.db_pool.get_all_users_with_gpodder_sync().await
                .map_err(|e| AppError::internal(&format!("Failed to get gPodder users: {}", e)))?;
            
            println!("Found {} users with gPodder sync enabled", gpodder_users.len());
            
            let mut successful_syncs = 0;
            let mut failed_syncs = 0;
            let mut total_synced_podcasts = 0;
            
            for (index, user_id) in gpodder_users.iter().enumerate() {
                let progress = 10.0 + (80.0 * (index as f64) / (gpodder_users.len() as f64));
                reporter.update_progress(progress, Some(format!("Syncing user {}/{}", index + 1, gpodder_users.len()))).await?;
                
                println!("Running gPodder sync for user {} ({}/{})", user_id, index + 1, gpodder_users.len());
                
                // Get user's sync type
                let gpodder_status = state.db_pool.gpodder_get_status(*user_id).await
                    .map_err(|e| AppError::internal(&format!("Failed to get status for user {}: {}", user_id, e)))?;
                
                if gpodder_status.sync_type != "None" && !gpodder_status.sync_type.is_empty() {
                    match run_admin_gpodder_sync(&state, *user_id, &gpodder_status.sync_type).await {
                        Ok(sync_result) => {
                            successful_syncs += 1;
                            total_synced_podcasts += sync_result.synced_podcasts;
                            println!("gPodder sync successful for user {}: {} podcasts", 
                                user_id, sync_result.synced_podcasts);
                        }
                        Err(e) => {
                            failed_syncs += 1;
                            println!("gPodder sync failed for user {}: {}", user_id, e);
                            tracing::error!("gPodder sync failed for user {}: {}", user_id, e);
                            // Continue with other users
                        }
                    }
                } else {
                    println!("gPodder sync not properly configured for user {}", user_id);
                }
            }
            
            println!("Admin gPodder sync completed: {}/{} users successful, {} total podcasts synced", 
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

// Helper function to refresh podcasts for a single user (admin refresh)
async fn refresh_user_podcasts_admin(state: &AppState, user_id: i32) -> AppResult<(i32, i32)> {
    let podcasts = state.db_pool.get_user_podcasts_for_refresh(user_id).await?;
    let mut successful_podcasts = 0;
    let mut total_new_episodes = 0;
    
    for podcast in podcasts {
        match refresh_single_podcast(state, &podcast, user_id, false).await {
            Ok(new_episodes) => {
                successful_podcasts += 1;
                total_new_episodes += new_episodes.len() as i32;
                println!("Refreshed podcast '{}': {} new episodes", podcast.name, new_episodes.len());
            }
            Err(e) => {
                println!("Failed to refresh podcast '{}': {}", podcast.name, e);
                // Continue with other podcasts
            }
        }
        
        // Small delay to prevent overwhelming
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }
    
    Ok((successful_podcasts, total_new_episodes))
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
    println!("Starting refresh process for user_id: {}, nextcloud_refresh: {}", user_id, nextcloud_refresh);
    
    // PRE-REFRESH GPODDER SYNC - matches Python implementation exactly
    if nextcloud_refresh {
        println!("Pre-refresh gPodder sync requested for user {}", user_id);
        
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
            println!("gPodder sync is enabled for user {}, sync_type: {}", user_id, gpodder_status.sync_type);
            
            let _ = tx.send(RefreshMessage::Status(RefreshStatus {
                progress: RefreshProgress {
                    current: 0,
                    total: 1,
                    current_podcast: format!("Syncing with gPodder ({})...", gpodder_status.sync_type),
                },
            })).await;

            match handle_gpodder_sync(&state, user_id, &gpodder_status.sync_type).await {
                Ok(sync_result) => {
                    println!("gPodder sync successful for user {}: {} podcasts, {} episodes", 
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
                    println!("gPodder sync failed for user {}: {}", user_id, e);
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
            println!("gPodder sync not enabled for user {} (enabled: {}, type: {})", 
                user_id, gpodder_status.sync_type != "None" && !gpodder_status.sync_type.is_empty(), gpodder_status.sync_type);
        }
    }

    // Get total podcast count for progress tracking
    let total_podcasts = state.db_pool.get_user_podcast_count(user_id).await?;
    println!("Found {} podcasts to refresh for user {}", total_podcasts, user_id);
    
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
    println!("Retrieved {} podcast details for refresh", podcasts.len());
    
    let mut current = 0;
    let mut successful_refreshes = 0;
    let mut failed_refreshes = 0;
    let mut total_new_episodes = 0;
    
    for podcast in podcasts {
        current += 1;
        
        println!("Refreshing podcast {}/{}: {} (ID: {}, is_youtube: {})", 
            current, total_podcasts, podcast.name, podcast.id, podcast.is_youtube);
        
        // Send progress update
        let _ = tx.send(RefreshMessage::Status(RefreshStatus {
            progress: RefreshProgress {
                current,
                total: total_podcasts,
                current_podcast: podcast.name.clone(),
            },
        })).await;

        // Refresh individual podcast with error handling like Python version
        match refresh_single_podcast(&state, &podcast, user_id, nextcloud_refresh).await {
            Ok(new_episodes) => {
                let episode_count = new_episodes.len();
                total_new_episodes += episode_count;
                successful_refreshes += 1;
                
                println!("Successfully refreshed podcast '{}': {} new episodes", podcast.name, episode_count);
                
                // Send new episodes through WebSocket
                for episode in new_episodes {
                    let _ = tx.send(RefreshMessage::NewEpisode(NewEpisode {
                        new_episode: episode,
                    })).await;
                }
            }
            Err(e) => {
                failed_refreshes += 1;
                println!("Error refreshing podcast '{}' (ID: {}): {}", podcast.name, podcast.id, e);
                tracing::error!("Error refreshing podcast '{}' (ID: {}): {}", podcast.name, podcast.id, e);
                // Continue with other podcasts - matches Python error handling
            }
        }

        // Small delay to prevent overwhelming the system
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    // Final completion summary - matches Python logging
    println!("Refresh completed for user {}: {}/{} podcasts successful, {} failed, {} total new episodes", 
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

#[derive(Debug)]
pub struct PodcastForRefresh {
    pub id: i32,
    pub name: String,
    pub feed_url: String,
    pub artwork_url: String,
    pub is_youtube: bool,
    pub auto_download: bool,
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

// Define sync result structure to match our database return type
#[derive(Debug)]
pub struct SyncResult {
    pub synced_podcasts: i32,
    pub synced_episodes: i32,
}

async fn handle_gpodder_sync(state: &AppState, user_id: i32, sync_type: &str) -> AppResult<SyncResult> {
    println!("Starting gPodder sync for user {}, sync_type: {}", user_id, sync_type);
    
    // Determine which sync function to call based on sync type - matches Python logic exactly
    match sync_type {
        "nextcloud" => {
            println!("Performing Nextcloud gPodder sync for user {}", user_id);
            
            // Use the nextcloud sync functionality - this handles the /index.php/apps/gpoddersync endpoints
            match state.db_pool.sync_with_nextcloud_for_user(user_id).await {
                Ok(success) => {
                    if success {
                        println!("Nextcloud sync successful for user {}", user_id);
                        Ok(SyncResult { synced_podcasts: 1, synced_episodes: 0 })
                    } else {
                        println!("Nextcloud sync returned false for user {}", user_id);
                        Ok(SyncResult { synced_podcasts: 0, synced_episodes: 0 })
                    }
                }
                Err(e) => {
                    println!("Nextcloud sync failed for user {}: {}", user_id, e);
                    Err(e)
                }
            }
        }
        "gpodder" | "external" | "both" => {
            println!("Performing standard gPodder sync for user {}, type: {}", user_id, sync_type);
            
            // Use the standard gPodder sync functionality
            match state.db_pool.gpodder_sync(user_id).await {
                Ok(sync_result) => {
                    println!("Standard gPodder sync successful for user {}: {} podcasts, {} episodes", 
                        user_id, sync_result.synced_podcasts, sync_result.synced_episodes);
                    
                    Ok(SyncResult {
                        synced_podcasts: sync_result.synced_podcasts,
                        synced_episodes: sync_result.synced_episodes,
                    })
                }
                Err(e) => {
                    println!("Standard gPodder sync failed for user {}: {}", user_id, e);
                    Err(e)
                }
            }
        }
        _ => {
            println!("Unknown sync type '{}' for user {}, skipping sync", sync_type, user_id);
            Ok(SyncResult { synced_podcasts: 0, synced_episodes: 0 })
        }
    }
}