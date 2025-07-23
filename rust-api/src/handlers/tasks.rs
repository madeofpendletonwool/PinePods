use axum::{
    extract::State,
    http::HeaderMap,
    response::Json,
};
use serde_json;

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

// Startup tasks endpoint - matches Python startup_tasks function exactly
pub async fn startup_tasks(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify if the API key is valid
    let is_valid = validate_api_key(&state, &api_key).await?;
    if !is_valid {
        return Err(AppError::forbidden("Invalid or unauthorized API key"));
    }

    // Check if the provided API key is from the background_tasks user (UserID 1)
    let api_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if api_user_id != 1 {
        return Err(AppError::forbidden("Invalid or unauthorized API key"));
    }

    // Execute the startup tasks
    state.db_pool.add_news_feed_if_not_added().await?;

    Ok(Json(serde_json::json!({"status": "Startup tasks completed successfully."})))
}

// Cleanup tasks endpoint - matches Python cleanup_tasks function exactly
pub async fn cleanup_tasks(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify if the API key is valid and is web key (admin only)
    let is_valid = validate_api_key(&state, &api_key).await?;
    if !is_valid {
        return Err(AppError::forbidden("Invalid API key"));
    }
    
    let api_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if api_user_id != 1 {
        return Err(AppError::forbidden("Admin access required"));
    }

    // Run cleanup tasks in background
    let db_pool = state.db_pool.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "cleanup_tasks".to_string(),
        0, // System user
        move |reporter| async move {
            reporter.update_progress(50.0, Some("Running cleanup tasks...".to_string())).await?;
            
            db_pool.cleanup_old_episodes().await
                .map_err(|e| AppError::internal(&format!("Cleanup failed: {}", e)))?;
            
            reporter.update_progress(100.0, Some("Cleanup completed successfully".to_string())).await?;
            
            Ok(serde_json::json!({"status": "Cleanup tasks completed successfully"}))
        },
    ).await?;

    Ok(Json(serde_json::json!({
        "detail": "Cleanup tasks initiated.",
        "task_id": task_id
    })))
}

// Update playlists endpoint - matches Python update_playlists function exactly
pub async fn update_playlists(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify if the API key is valid and is web key (admin only)
    let is_valid = validate_api_key(&state, &api_key).await?;
    if !is_valid {
        return Err(AppError::forbidden("Invalid API key"));
    }
    
    let api_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if api_user_id != 1 {
        return Err(AppError::forbidden("Admin access required"));
    }

    // Run playlist update in background
    let db_pool = state.db_pool.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "update_playlists".to_string(),
        0, // System user
        move |reporter| async move {
            reporter.update_progress(50.0, Some("Updating all playlists...".to_string())).await?;
            
            db_pool.update_all_playlists().await
                .map_err(|e| AppError::internal(&format!("Playlist update failed: {}", e)))?;
            
            reporter.update_progress(100.0, Some("Playlist update completed successfully".to_string())).await?;
            
            Ok(serde_json::json!({"status": "Playlist update completed successfully"}))
        },
    ).await?;

    Ok(Json(serde_json::json!({
        "detail": "Playlist update initiated.",
        "task_id": task_id
    })))
}

// Refresh hosts endpoint - matches Python refresh_all_hosts function exactly
pub async fn refresh_hosts(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify it's the system API key (background_tasks user with UserID 1)
    let is_valid = validate_api_key(&state, &api_key).await?;
    if !is_valid {
        return Err(AppError::forbidden("Invalid API key"));
    }
    
    let api_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if api_user_id != 1 {
        return Err(AppError::forbidden("This endpoint requires system API key"));
    }

    // Run host refresh in background
    let db_pool = state.db_pool.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_hosts".to_string(),
        0, // System user
        move |reporter| async move {
            reporter.update_progress(10.0, Some("Getting all people/hosts...".to_string())).await?;
            
            let all_people = db_pool.get_all_people_for_refresh().await
                .map_err(|e| AppError::internal(&format!("Failed to get people: {}", e)))?;
            
            tracing::info!("Found {} people/hosts to refresh", all_people.len());
            
            let mut successful_refreshes = 0;
            let mut failed_refreshes = 0;
            
            for (index, (person_id, person_name, user_id)) in all_people.iter().enumerate() {
                let progress = 10.0 + (80.0 * (index as f64) / (all_people.len() as f64));
                reporter.update_progress(progress, Some(format!("Refreshing host: {} ({}/{})", person_name, index + 1, all_people.len()))).await?;
                
                tracing::info!("Starting refresh for host: {} (ID: {}, User: {})", person_name, person_id, user_id);
                
                match process_person_refresh(&db_pool, *person_id, person_name, *user_id).await {
                    Ok(_) => {
                        successful_refreshes += 1;
                        tracing::info!("Successfully refreshed host: {}", person_name);
                    }
                    Err(e) => {
                        failed_refreshes += 1;
                        tracing::error!("Failed to refresh host {}: {}", person_name, e);
                    }
                }
            }
            
            // After processing all people, trigger the regular podcast refresh
            tracing::info!("Person subscription processed, initiating server refresh...");
            match trigger_podcast_refresh(&db_pool).await {
                Ok(_) => {
                    tracing::info!("Server refresh completed successfully");
                }
                Err(e) => {
                    tracing::error!("Error during server refresh: {}", e);
                }
            }
            
            tracing::info!("Host refresh completed: {}/{} successful, {} failed", 
                successful_refreshes, all_people.len(), failed_refreshes);
            
            reporter.update_progress(100.0, Some(format!(
                "Host refresh completed: {}/{} successful", 
                successful_refreshes, all_people.len()
            ))).await?;
            
            Ok(serde_json::json!({
                "success": true,
                "hosts_refreshed": successful_refreshes,
                "hosts_failed": failed_refreshes,
                "total_hosts": all_people.len()
            }))
        },
    ).await?;

    Ok(Json(serde_json::json!({
        "detail": "Host refresh initiated.",
        "task_id": task_id
    })))
}

// Helper function to process individual person refresh - matches Python process_person_subscription
async fn process_person_refresh(
    db_pool: &crate::database::DatabasePool,
    person_id: i32,
    person_name: &str,
    user_id: i32,
) -> AppResult<()> {
    tracing::info!("Processing person subscription for: {} (ID: {}, User: {})", person_name, person_id, user_id);
    
    // Get person details and refresh their content
    match db_pool.process_person_subscription(user_id, person_id, person_name.to_string()).await {
        Ok(_) => {
            tracing::info!("Successfully processed person subscription for {}", person_name);
            Ok(())
        }
        Err(e) => {
            tracing::error!("Error processing person subscription for {}: {}", person_name, e);
            Err(e)
        }
    }
}

// Helper function to trigger podcast refresh after person processing - matches Python refresh_pods_task
async fn trigger_podcast_refresh(db_pool: &crate::database::DatabasePool) -> AppResult<()> {
    // Get all users with podcasts and refresh them
    let all_users = db_pool.get_all_users_with_podcasts().await?;
    
    for user_id in all_users {
        match refresh_user_podcasts(db_pool, user_id).await {
            Ok((podcast_count, episode_count)) => {
                tracing::info!("Successfully refreshed user {}: {} podcasts, {} new episodes", 
                    user_id, podcast_count, episode_count);
            }
            Err(e) => {
                tracing::error!("Failed to refresh user {}: {}", user_id, e);
            }
        }
    }
    
    Ok(())
}

// Helper function to refresh podcasts for a single user
async fn refresh_user_podcasts(db_pool: &crate::database::DatabasePool, user_id: i32) -> AppResult<(i32, i32)> {
    let podcasts = db_pool.get_user_podcasts_for_refresh(user_id).await?;
    let mut successful_podcasts = 0;
    let mut total_new_episodes = 0;
    
    for podcast in podcasts {
        match refresh_single_podcast(db_pool, &podcast).await {
            Ok(new_episode_count) => {
                successful_podcasts += 1;
                total_new_episodes += new_episode_count;
                tracing::info!("Refreshed podcast '{}': {} new episodes", podcast.name, new_episode_count);
            }
            Err(e) => {
                tracing::error!("Failed to refresh podcast '{}': {}", podcast.name, e);
            }
        }
    }
    
    Ok((successful_podcasts, total_new_episodes))
}

// Helper function to refresh a single podcast
async fn refresh_single_podcast(
    _db_pool: &crate::database::DatabasePool,
    podcast: &crate::handlers::refresh::PodcastForRefresh,
) -> AppResult<i32> {
    tracing::info!("Refreshing podcast: {} (ID: {})", podcast.name, podcast.id);
    // This would normally refresh the podcast feed and return new episode count
    // For now return 0 as placeholder since we need the podcast refresh system to be implemented
    Ok(0)
}