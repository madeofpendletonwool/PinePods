use axum::{
    extract::{Path, State},
    http::HeaderMap,
    Json,
};
use crate::{
    error::AppError,
    models::*,
    AppState,
};
use super::{extract_api_key, check_user_access};

// Get user's saved folders
pub async fn get_saved_folders(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<SavedFoldersResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only access your own folders!"));
    }

    // Get folders from database
    let folders = state.db_pool.get_saved_folders(user_id).await?;

    Ok(Json(SavedFoldersResponse { folders }))
}

// Create a new saved folder
pub async fn create_saved_folder(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<CreateSavedFolderRequest>,
) -> Result<Json<SavedFolderResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only create folders for yourself!"));
    }

    // Create folder
    let folder_id = state.db_pool.create_saved_folder(
        request.user_id,
        &request.folder_name,
        request.folder_color.as_deref(),
        request.icon_name.as_deref().unwrap_or("ph-folder"),
        request.auto_add_category.as_deref(),
        request.position.unwrap_or(0),
    ).await?;

    Ok(Json(SavedFolderResponse {
        detail: "Folder created successfully".to_string(),
        folder_id: Some(folder_id),
    }))
}

// Update a saved folder
pub async fn update_saved_folder(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<UpdateSavedFolderRequest>,
) -> Result<Json<SavedFolderResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only update your own folders!"));
    }

    // Update folder
    state.db_pool.update_saved_folder(
        request.folder_id,
        request.user_id,
        request.folder_name.as_deref(),
        request.folder_color.as_deref(),
        request.icon_name.as_deref(),
        request.auto_add_category.as_deref(),
        request.position,
    ).await?;

    Ok(Json(SavedFolderResponse {
        detail: "Folder updated successfully".to_string(),
        folder_id: Some(request.folder_id),
    }))
}

// Delete a saved folder
pub async fn delete_saved_folder(
    Path(folder_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<SavedFolderResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let user_id = request["user_id"].as_i64().ok_or_else(|| {
        AppError::bad_request("user_id is required")
    })? as i32;

    // Check authorization
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only delete your own folders!"));
    }

    // Delete folder
    state.db_pool.delete_saved_folder(folder_id, user_id).await?;

    Ok(Json(SavedFolderResponse {
        detail: "Folder deleted successfully".to_string(),
        folder_id: Some(folder_id),
    }))
}

// Add episode to folder
pub async fn add_episode_to_folder(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<AddEpisodeToFolderRequest>,
) -> Result<Json<SavedFolderResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only manage your own folders!"));
    }

    // Add episode to folder
    state.db_pool.add_episode_to_folder(request.save_id, request.folder_id).await?;

    Ok(Json(SavedFolderResponse {
        detail: "Episode added to folder successfully".to_string(),
        folder_id: Some(request.folder_id),
    }))
}

// Remove episode from folder
pub async fn remove_episode_from_folder(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<RemoveEpisodeFromFolderRequest>,
) -> Result<Json<SavedFolderResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only manage your own folders!"));
    }

    // Remove episode from folder
    state.db_pool.remove_episode_from_folder(request.save_id, request.folder_id).await?;

    Ok(Json(SavedFolderResponse {
        detail: "Episode removed from folder successfully".to_string(),
        folder_id: Some(request.folder_id),
    }))
}

// Bulk add episodes to folder
pub async fn bulk_add_episodes_to_folder(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<BulkAddEpisodesToFolderRequest>,
) -> Result<Json<SavedFolderResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only manage your own folders!"));
    }

    // Add all episodes to folder
    for save_id in request.save_ids {
        state.db_pool.add_episode_to_folder(save_id, request.folder_id).await?;
    }

    Ok(Json(SavedFolderResponse {
        detail: "Episodes added to folder successfully".to_string(),
        folder_id: Some(request.folder_id),
    }))
}

// Get episodes in a specific folder
pub async fn get_folder_episodes(
    Path(folder_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<FolderEpisodesResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let user_id = request["user_id"].as_i64().ok_or_else(|| {
        AppError::bad_request("user_id is required")
    })? as i32;

    // Check authorization
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only access your own folders!"));
    }

    // Get folder info
    let folders = state.db_pool.get_saved_folders(user_id).await?;
    let folder = folders.into_iter()
        .find(|f| f.folderid == folder_id)
        .ok_or_else(|| AppError::not_found("Folder not found"))?;

    // Get episodes
    let episodes = state.db_pool.get_folder_episodes(folder_id, user_id).await?;

    Ok(Json(FolderEpisodesResponse { episodes, folder }))
}

// Get save_id helper endpoint
pub async fn get_save_id_endpoint(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;

    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let episode_id = request["episode_id"].as_i64().ok_or_else(|| {
        AppError::bad_request("episode_id is required")
    })? as i32;

    let user_id = request["user_id"].as_i64().ok_or_else(|| {
        AppError::bad_request("user_id is required")
    })? as i32;

    let is_youtube = request["is_youtube"].as_bool().unwrap_or(false);

    // Check authorization
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only access your own data!"));
    }

    // Get save_id
    let save_id = state.db_pool.get_save_id(episode_id, user_id, is_youtube).await?;

    Ok(Json(serde_json::json!({
        "save_id": save_id
    })))
}
