use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct Episode {
    pub podcastname: String,
    pub episodetitle: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub completed: bool,
    pub saved: bool,
    pub queued: bool,
    pub downloaded: bool,
    pub is_youtube: bool,
}

#[derive(Serialize)]
pub struct EpisodesResponse {
    pub episodes: Vec<Episode>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PodcastValues {
    pub pod_title: String,
    pub pod_artwork: String,
    pub pod_author: String,
    pub categories: HashMap<String, String>,
    pub pod_description: String,
    pub pod_episode_count: i32,
    pub pod_feed_url: String,
    pub pod_website: String,
    pub pod_explicit: bool,
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct AddPodcastRequest {
    pub podcast_values: PodcastValues,
    pub podcast_index_id: Option<i64>,
}

#[derive(Serialize)]
pub struct PodcastStatusResponse {
    pub success: bool,
    pub podcast_id: i32,
    pub first_episode_id: i32,
}

#[derive(Deserialize)]
pub struct RemovePodcastRequest {
    pub user_id: i32,
    pub podcast_name: String,
    pub podcast_url: String,
}

#[derive(Serialize)]
pub struct RemovePodcastResponse {
    pub success: bool,
}

// Get episodes for a user - matches Python return_episodes endpoint
pub async fn return_episodes(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<EpisodesResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != user_id {
        return Err(AppError::forbidden("You can only return episodes of your own!"));
    }

    // Get episodes from database
    let episodes = state.db_pool.return_episodes(user_id).await?;
    
    Ok(Json(EpisodesResponse { episodes }))
}

// Add a new podcast - matches Python add_podcast endpoint
pub async fn add_podcast(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<AddPodcastRequest>,
) -> Result<Json<PodcastStatusResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only add podcasts for themselves
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != request.podcast_values.user_id {
        return Err(AppError::forbidden("You can only add podcasts for yourself!"));
    }

    // Add podcast to database
    let (podcast_id, first_episode_id) = state.db_pool.add_podcast(
        &request.podcast_values,
        request.podcast_index_id.unwrap_or(0),
    ).await?;
    
    Ok(Json(PodcastStatusResponse {
        success: true,
        podcast_id,
        first_episode_id,
    }))
}

// Remove a podcast - matches Python remove_podcast endpoint
pub async fn remove_podcast(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<RemovePodcastRequest>,
) -> Result<Json<RemovePodcastResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only remove their own podcasts
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != request.user_id {
        return Err(AppError::forbidden("You can only remove your own podcasts!"));
    }

    // Remove podcast from database
    state.db_pool.remove_podcast(
        &request.podcast_name,
        &request.podcast_url,
        request.user_id,
    ).await?;
    
    Ok(Json(RemovePodcastResponse { success: true }))
}

// Remove podcast by name and URL - matches call_remove_podcasts_name from frontend
pub async fn remove_podcast_by_name(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<crate::models::RemovePodcastByNameRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only remove their own podcasts
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != request.user_id {
        return Err(AppError::forbidden("You can only remove your own podcasts!"));
    }

    // Remove podcast from database using the comprehensive method
    state.db_pool.remove_podcast_by_name_url(
        &request.podcast_name,
        &request.podcast_url,
        request.user_id,
    ).await?;
    
    Ok(Json(serde_json::json!({ "success": true })))
}

// Get podcasts for a user - matches call_get_podcasts from frontend
pub async fn return_pods(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::PodcastListResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own podcasts
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != user_id {
        return Err(AppError::forbidden("You can only return podcasts of your own!"));
    }

    // Get podcasts from database
    let pods = state.db_pool.return_pods(user_id).await?;
    
    Ok(Json(crate::models::PodcastListResponse { pods }))
}

// Get podcasts with extra stats for a user - matches call_get_podcasts_extra from frontend
pub async fn return_pods_extra(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::PodcastExtraListResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own podcasts
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != user_id {
        return Err(AppError::forbidden("You can only return podcasts of your own!"));
    }

    // Get podcasts with extra stats from database
    let pods = state.db_pool.return_pods_extra(user_id).await?;
    
    Ok(Json(crate::models::PodcastExtraListResponse { pods }))
}

// Query parameters for check operations
#[derive(Deserialize)]
pub struct CheckPodcastQuery {
    pub user_id: i32,
    pub podcast_name: String,
    pub podcast_url: String,
}

#[derive(Deserialize)]
pub struct CheckEpisodeQuery {
    pub episode_title: String,
    pub episode_url: String,
}

#[derive(Deserialize)]
pub struct TimeInfoQuery {
    pub user_id: i32,
}

// Get time info for a user - matches call_get_time_info from frontend
pub async fn get_time_info(
    Query(query): Query<TimeInfoQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::TimeInfoResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own time info
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != query.user_id {
        return Err(AppError::forbidden("You can only get your own time info!"));
    }

    // Get time info from database
    let time_info = state.db_pool.get_time_info(query.user_id).await?;
    
    Ok(Json(time_info))
}

// Check if podcast exists - matches call_check_podcast from frontend
pub async fn check_podcast(
    Query(query): Query<CheckPodcastQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::CheckPodcastResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only check their own podcasts
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != query.user_id {
        return Err(AppError::forbidden("You can only check your own podcasts!"));
    }

    // Check if podcast exists in database
    let exists = state.db_pool.check_podcast(query.user_id, &query.podcast_name, &query.podcast_url).await?;
    
    Ok(Json(crate::models::CheckPodcastResponse { exists }))
}

// Check if episode exists in database - matches call_check_episode_in_db from frontend
pub async fn check_episode_in_db(
    Path(user_id): Path<i32>,
    Query(query): Query<CheckEpisodeQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::EpisodeInDbResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only check episodes in their own podcasts
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != user_id {
        return Err(AppError::forbidden("You can only check episodes in your own podcasts!"));
    }

    // Check if episode exists in database
    let episode_in_db = state.db_pool.check_episode_exists(user_id, &query.episode_title, &query.episode_url).await?;
    
    Ok(Json(crate::models::EpisodeInDbResponse { episode_in_db }))
}

// Queue episode - matches call_queue_episode from frontend
pub async fn queue_episode(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<crate::models::QueuePodcastRequest>,
) -> Result<Json<crate::models::QueueResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only queue episodes for themselves
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != request.user_id {
        return Err(AppError::forbidden("You can only queue episodes for yourself!"));
    }

    // Queue the episode
    state.db_pool.queue_episode(request.episode_id, request.user_id, request.is_youtube).await?;
    
    let message = if request.is_youtube {
        "Video queued successfully"
    } else {
        "Episode queued successfully"
    };
    
    Ok(Json(crate::models::QueueResponse {
        data: message.to_string(),
    }))
}

// Remove queued episode - matches call_remove_queued_episode from frontend
pub async fn remove_queued_episode(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<crate::models::QueuePodcastRequest>,
) -> Result<Json<crate::models::QueueResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only remove their own queued episodes
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != request.user_id {
        return Err(AppError::forbidden("You can only remove your own queued episodes!"));
    }

    // Remove the episode from queue
    state.db_pool.remove_queued_episode(request.episode_id, request.user_id, request.is_youtube).await?;
    
    Ok(Json(crate::models::QueueResponse {
        data: "Successfully Removed Episode From Queue".to_string(),
    }))
}

// Get queued episodes - matches call_get_queued_episodes from frontend
pub async fn get_queued_episodes(
    Query(query): Query<TimeInfoQuery>, // Reuse TimeInfoQuery since it just needs user_id
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::QueuedEpisodesResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own queued episodes
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != query.user_id {
        return Err(AppError::forbidden("You can only get your own queued episodes!"));
    }

    // Get queued episodes from database
    let data = state.db_pool.get_queued_episodes(query.user_id).await?;
    
    Ok(Json(crate::models::QueuedEpisodesResponse { data }))
}

// Reorder queue - matches call_reorder_queue from frontend
pub async fn reorder_queue(
    Query(query): Query<TimeInfoQuery>, // Reuse TimeInfoQuery since it just needs user_id
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<crate::models::ReorderQueueRequest>,
) -> Result<Json<crate::models::ReorderQueueResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only reorder their own queue
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != query.user_id {
        return Err(AppError::forbidden("You can only reorder your own queue!"));
    }

    // Reorder the queue
    state.db_pool.reorder_queue(query.user_id, request.episode_ids).await?;
    
    Ok(Json(crate::models::ReorderQueueResponse {
        message: "Queue reordered successfully".to_string(),
    }))
}

// Save episode - matches call_save_episode from frontend
pub async fn save_episode(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<crate::models::SavePodcastRequest>,
) -> Result<Json<crate::models::SaveEpisodeResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only save episodes for themselves
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != request.user_id {
        return Err(AppError::forbidden("You can only save episodes for yourself!"));
    }

    // Save the episode
    state.db_pool.save_episode(request.episode_id, request.user_id, request.is_youtube).await?;
    
    let message = if request.is_youtube {
        "Video saved!"
    } else {
        "Episode saved!"
    };
    
    Ok(Json(crate::models::SaveEpisodeResponse {
        detail: message.to_string(),
    }))
}

// Remove saved episode - matches call_remove_saved_episode from frontend
pub async fn remove_saved_episode(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<crate::models::SavePodcastRequest>,
) -> Result<Json<crate::models::SaveEpisodeResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only remove their own saved episodes
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != request.user_id {
        return Err(AppError::forbidden("You can only remove your own saved episodes!"));
    }

    // Remove the saved episode
    state.db_pool.remove_saved_episode(request.episode_id, request.user_id, request.is_youtube).await?;
    
    let message = if request.is_youtube {
        "Saved video removed."
    } else {
        "Saved episode removed."
    };
    
    Ok(Json(crate::models::SaveEpisodeResponse {
        detail: message.to_string(),
    }))
}

// Get saved episodes - matches call_get_saved_episodes from frontend
pub async fn get_saved_episodes(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::SavedEpisodesResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own saved episodes
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != user_id {
        return Err(AppError::forbidden("You can only get your own saved episodes!"));
    }

    // Get saved episodes from database
    let saved_episodes = state.db_pool.get_saved_episodes(user_id).await?;
    
    Ok(Json(crate::models::SavedEpisodesResponse { saved_episodes }))
}

// Add history - matches call_add_history from frontend
pub async fn add_history(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<crate::models::HistoryAddRequest>,
) -> Result<Json<crate::models::HistoryResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only add history for themselves
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != request.user_id {
        return Err(AppError::forbidden("You can only add history for yourself!"));
    }

    // Record the history
    state.db_pool.record_podcast_history(
        request.episode_id, 
        request.user_id, 
        request.episode_pos, 
        request.is_youtube
    ).await?;
    
    Ok(Json(crate::models::HistoryResponse {
        detail: "History recorded successfully.".to_string(),
    }))
}

// Get user history - matches call_get_user_history from frontend
pub async fn get_user_history(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::UserHistoryResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own history
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // TODO: Add web key check for elevated access
    if requesting_user_id != user_id {
        return Err(AppError::forbidden("You can only get your own history!"));
    }

    // Get user history from database
    let data = state.db_pool.get_user_history(user_id).await?;
    
    Ok(Json(crate::models::UserHistoryResponse { data }))
}