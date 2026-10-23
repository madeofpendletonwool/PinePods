use axum::{
    extract::State,
    http::HeaderMap,
    response::Json,
};
use serde::Deserialize;
use serde_json;

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

#[derive(Deserialize, utoipa::ToSchema)]
pub struct InitRequest {
    pub api_key: String,
}

// Startup tasks endpoint - matches Python startup_tasks function exactly
#[utoipa::path(
    post,
    path = "/startup_tasks",
    tag = "tasks",
    summary = "Startup tasks",
    request_body = InitRequest,
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
    ),
)]
pub async fn startup_tasks(
    State(state): State<AppState>,
    Json(request): Json<InitRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Verify if the API key is valid
    let is_valid = validate_api_key(&state, &request.api_key).await?;
    if !is_valid {
        return Err(AppError::forbidden("Invalid or unauthorized API key"));
    }

    // Check if the provided API key is from the background_tasks user (UserID 1)
    let api_user_id = state.db_pool.get_user_id_from_api_key(&request.api_key).await?;
    if api_user_id != 1 {
        return Err(AppError::forbidden("Invalid or unauthorized API key"));
    }

    // Execute the startup tasks
    state.db_pool.add_news_feed_if_not_added().await?;
    
    // Create default playlists for any users that might be missing them
    state.db_pool.create_missing_default_playlists().await?;

    Ok(Json(serde_json::json!({"status": "Startup tasks completed successfully."})))
}

// Cleanup tasks endpoint - matches Python cleanup_tasks function exactly
#[utoipa::path(
    get,
    path = "/cleanup_tasks",
    tag = "tasks",
    summary = "Cleanup tasks",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/update_playlists",
    tag = "tasks",
    summary = "Update playlists",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/refresh_hosts",
    tag = "tasks",
    summary = "Refresh hosts",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

    let db_pool = state.db_pool.clone();
    let api = crate::services::people::Endpoints::from_config(&state.config);
    let task_id = state.task_spawner.spawn_progress_task(
        "refresh_hosts".to_string(),
        0, // System user
        move |reporter| async move {
            reporter.update_progress(10.0, Some("Syncing people from PodPeopleDB...".to_string())).await?;
            let result = people_sync_internal(&db_pool, &api).await?;
            reporter.update_progress(100.0, Some("People sync completed".to_string())).await?;
            Ok(result)
        },
    ).await?;

    Ok(Json(serde_json::json!({
        "detail": "People sync initiated.",
        "task_id": task_id
    })))
}

/// Pull community credits from PodPeopleDB for every podcast, then refresh the episodes that
/// followed people appear in on shows outside each user's library.
pub async fn people_sync_internal(
    db_pool: &crate::database::DatabasePool,
    api: &crate::services::people::Endpoints,
) -> AppResult<serde_json::Value> {
    use crate::services::people::{external, podpeople};
    let podcasts_updated = if podpeople::sync_enabled() { podpeople::sync(db_pool, api, None).await? } else { 0 };
    let people_refreshed = external::refresh_followed(db_pool, api).await?;
    tracing::info!(
        "People sync: {} podcasts got new PodPeopleDB credits, {} followed people refreshed",
        podcasts_updated,
        people_refreshed
    );
    Ok(serde_json::json!({
        "success": true,
        "podcasts_updated": podcasts_updated,
        "people_refreshed": people_refreshed,
    }))
}

// Auto-complete episodes based on user settings - nightly task
#[utoipa::path(
    get,
    path = "/auto_complete_episodes",
    tag = "tasks",
    summary = "Auto complete episodes",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn auto_complete_episodes(
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

    // Get all users who have auto_complete_seconds > 0
    let users_with_auto_complete = state.db_pool.get_users_with_auto_complete_enabled().await?;
    let mut total_completed = 0;

    for user in users_with_auto_complete {
        let completed_count = state.db_pool.auto_complete_user_episodes(user.user_id, user.auto_complete_seconds).await.unwrap_or(0);
        total_completed += completed_count;
    }

    Ok(Json(serde_json::json!({
        "status": "Auto-complete task completed successfully",
        "episodes_completed": total_completed
    })))
}

// Internal functions for scheduler (no HTTP context needed)
pub async fn cleanup_tasks_internal(state: &AppState) -> AppResult<()> {
    tracing::info!("Starting internal cleanup tasks (scheduler)");
    
    state.db_pool.cleanup_old_episodes().await?;

    // Auto-delete server downloads past their per-user/per-podcast retention window (#655)
    if let Err(e) = state.db_pool.auto_delete_old_downloads().await {
        tracing::error!("Auto-delete old downloads failed during cleanup tasks: {}", e);
    }

    if let Err(e) = state.db_pool.cleanup_expired_gpodder_sessions().await {
        tracing::warn!("Expired gpodder session cleanup failed: {}", e);
    }

    if let Err(e) = state.db_pool.delete_orphan_feeds().await {
        tracing::warn!("Deleting feeds without subscribers failed: {}", e);
    }

    tracing::info!("Cleanup tasks completed successfully");

    Ok(())
}

pub async fn refresh_hosts_internal(state: &AppState) -> AppResult<()> {
    people_sync_internal(&state.db_pool, &crate::services::people::Endpoints::from_config(&state.config)).await.map(|_| ())
}

pub async fn auto_complete_episodes_internal(state: &AppState) -> AppResult<()> {
    tracing::info!("Starting internal auto-complete episodes (scheduler)");
    
    // Get all users who have auto_complete_seconds > 0
    let users_with_auto_complete = state.db_pool.get_users_with_auto_complete_enabled().await?;
    let mut total_completed = 0;

    for user in users_with_auto_complete {
        let completed_count = state.db_pool.auto_complete_user_episodes(user.user_id, user.auto_complete_seconds).await.unwrap_or(0);
        total_completed += completed_count;
    }
    
    tracing::info!("Auto-complete task completed: {} episodes completed", total_completed);
    Ok(())
}