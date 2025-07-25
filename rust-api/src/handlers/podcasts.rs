use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key, check_user_access},
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

// Separate struct for downloaded episodes that exactly matches Python implementation
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct DownloadedEpisode {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: Option<String>,
    pub episodeid: i32,
    pub episodetitle: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: Option<String>,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub podcastindexid: Option<i32>,
    pub websiteurl: Option<String>,
    pub downloadedlocation: String,
    pub listenduration: Option<i32>,
    pub completed: bool,
    pub saved: bool,
    pub queued: bool,
    pub downloaded: bool,  // Always true for downloaded episodes
    pub is_youtube: bool,
}

// Response struct for downloaded episodes
#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadedEpisodesResponse {
    pub downloaded_episodes: Vec<DownloadedEpisode>,
}

// Separate struct for podcast_episodes endpoint that matches frontend expectations
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct PodcastEpisode {
    pub podcastname: String,
    #[serde(rename = "Episodetitle")]
    pub episodetitle: String,
    #[serde(rename = "Episodepubdate")]
    pub episodepubdate: String,
    #[serde(rename = "Episodedescription")]
    pub episodedescription: String,
    #[serde(rename = "Episodeartwork")]
    pub episodeartwork: String,
    #[serde(rename = "Episodeurl")]
    pub episodeurl: String,
    #[serde(rename = "Episodeduration")]
    pub episodeduration: i32,
    #[serde(rename = "Listenduration")]
    pub listenduration: Option<i32>,
    #[serde(rename = "Episodeid")]
    pub episodeid: i32,
    #[serde(rename = "Completed")]
    pub completed: bool,
    pub saved: bool,
    pub queued: bool,
    pub downloaded: bool,
    pub is_youtube: bool,
}

#[derive(Serialize)]
pub struct PodcastEpisodesResponse {
    pub episodes: Vec<PodcastEpisode>,
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

// Query struct for get_podcast_details - matches Python endpoint
#[derive(Deserialize)]
pub struct GetPodcastDetailsQuery {
    pub user_id: i32,
    pub podcast_id: i32,
}

// Response struct for get_podcast_details - matches Python ClickedFeedURL model
#[derive(Serialize, Deserialize, Debug, Clone)]
#[allow(non_snake_case)]
pub struct PodcastDetails {
    pub podcastid: i32,
    pub podcastname: String,
    pub feedurl: String,
    pub description: String,
    pub author: String,
    pub artworkurl: String,
    pub explicit: bool,
    pub episodecount: i32,
    pub categories: Option<HashMap<String, String>>,
    pub websiteurl: String,
    pub podcastindexid: i32,
    pub is_youtube: Option<bool>,
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, user_id).await? {
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
    
    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, requesting_user_id).await? {
        return Err(AppError::forbidden("You can only add podcasts for yourself!"));
    }

    // Add podcast to database
    let (podcast_id, first_episode_id) = state.db_pool.add_podcast(
        &request.podcast_values,
        request.podcast_index_id.unwrap_or(0),
        None, // username
        None, // password
    ).await?;
    
    Ok(Json(PodcastStatusResponse {
        success: true,
        podcast_id,
        first_episode_id: first_episode_id.unwrap_or(0),
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
    
    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, requesting_user_id).await? {
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
    
    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, requesting_user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, query.user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, query.user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, request.user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, request.user_id).await? {
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
    
    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, query.user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, query.user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, request.user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, request.user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, request.user_id).await? {
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

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only get your own history!"));
    }

    // Get user history from database
    let data = state.db_pool.get_user_history(user_id).await?;
    
    Ok(Json(crate::models::UserHistoryResponse { data }))
}

// Query parameters for get_podcast_id
#[derive(Deserialize)]
pub struct GetPodcastIdQuery {
    pub user_id: i32,
    pub podcast_feed: String,
    pub podcast_title: String,
}

// Get podcast ID - matches Python get_podcast_id endpoint
pub async fn get_podcast_id(
    Query(query): Query<GetPodcastIdQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, query.user_id).await? {
        return Err(AppError::forbidden("You can only return pocast ids of your own podcasts!"));
    }

    // Get podcast ID from database
    let podcast_id = state.db_pool.get_podcast_id(query.user_id, &query.podcast_feed, &query.podcast_title).await?;
    
    // Return single podcast_id or null, matching Python behavior
    let episodes = podcast_id;
    
    Ok(Json(serde_json::json!({ "episodes": episodes })))
}

// Query parameters for download_episode_list
#[derive(Deserialize)]
pub struct DownloadEpisodeListQuery {
    pub user_id: i32,
}

// Get downloaded episodes list - matches Python download_episode_list endpoint
pub async fn download_episode_list(
    Query(query): Query<DownloadEpisodeListQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<DownloadedEpisodesResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, query.user_id).await? {
        return Err(AppError::forbidden("You can only return downloaded episodes for yourself!"));
    }

    // Get downloaded episodes from database
    let downloaded_episodes = state.db_pool.download_episode_list(query.user_id).await?;
    
    Ok(Json(DownloadedEpisodesResponse { downloaded_episodes }))
}

// Request models for download operations
#[derive(Deserialize)]
pub struct DownloadPodcastRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub is_youtube: Option<bool>,
}

#[derive(Deserialize)]
pub struct DeleteEpisodeRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub is_youtube: Option<bool>,
}

#[derive(Deserialize)]
pub struct DownloadAllPodcastRequest {
    pub podcast_id: i32,
    pub user_id: i32,
    pub is_youtube: Option<bool>,
}

// Download a single episode - matches Python download_podcast endpoint
pub async fn download_podcast(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<DownloadPodcastRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only download content for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    
    // Check if already downloaded
    let is_downloaded = state.db_pool.check_downloaded(request.user_id, request.episode_id, is_youtube).await?;
    if is_downloaded {
        return Ok(Json(serde_json::json!({ "detail": "Content already downloaded." })));
    }

    // Queue the download task using the task system
    let task_id = if is_youtube {
        state.task_spawner.spawn_download_youtube_video(request.episode_id, request.user_id).await?
    } else {
        state.task_spawner.spawn_download_podcast_episode(request.episode_id, request.user_id).await?
    };

    let content_type = if is_youtube { "YouTube video" } else { "Podcast episode" };
    
    Ok(Json(serde_json::json!({
        "detail": format!("{} download has been queued and will process in the background.", content_type),
        "task_id": task_id
    })))
}

// Delete a downloaded episode - matches Python delete_episode endpoint  
pub async fn delete_episode(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<DeleteEpisodeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only delete your own downloads!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    
    // Delete the episode
    state.db_pool.delete_episode(request.user_id, request.episode_id, is_youtube).await?;
    
    let content_type = if is_youtube { "Video" } else { "Episode" };
    
    Ok(Json(serde_json::json!({
        "detail": format!("{} deleted successfully.", content_type)
    })))
}

// Download all episodes of a podcast - matches Python download_all_podcast endpoint
pub async fn download_all_podcast(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<DownloadAllPodcastRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only download content for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    
    // Queue the download all task using the task system
    let task_id = if is_youtube {
        state.task_spawner.spawn_download_all_youtube_videos(request.podcast_id, request.user_id).await?
    } else {
        state.task_spawner.spawn_download_all_podcast_episodes(request.podcast_id, request.user_id).await?
    };

    let content_type = if is_youtube { "YouTube channel" } else { "Podcast" };
    
    Ok(Json(serde_json::json!({
        "detail": format!("All {} downloads have been queued and will process in the background.", content_type),
        "task_id": task_id
    })))
}

// Get download status for a user - matches Python download_status endpoint
pub async fn download_status(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only get your own download status!"));
    }

    // Get download status from database
    let status = state.db_pool.get_download_status(user_id).await?;
    
    Ok(Json(serde_json::json!(status)))
}

// Query parameters for podcast_episodes
#[derive(Deserialize)]
pub struct PodcastEpisodesQuery {
    pub user_id: i32,
    pub podcast_id: i32,
}

// Get episodes for a specific podcast - matches Python podcast_episodes endpoint
pub async fn podcast_episodes(
    Query(query): Query<PodcastEpisodesQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<PodcastEpisodesResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, query.user_id).await? {
        return Err(AppError::forbidden("You can only return episodes of your own!"));
    }

    // Get podcast episodes from database 
    let episodes = state.db_pool.return_podcast_episodes_capitalized(query.user_id, query.podcast_id).await?;
    
    Ok(Json(PodcastEpisodesResponse { episodes }))
}

// Query parameters for get_podcast_id_from_ep_name
#[derive(Deserialize)]
pub struct GetPodcastIdFromEpNameQuery {
    pub episode_name: String,
    pub episode_url: String,
    pub user_id: i32,
}

// Get podcast ID from episode name and URL - matches Python get_podcast_id_from_ep_name endpoint
pub async fn get_podcast_id_from_ep_name(
    Query(query): Query<GetPodcastIdFromEpNameQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization - users can only get their own episodes or have web key access (user ID 1)
    if !check_user_access(&state, &api_key, query.user_id).await? {
        return Err(AppError::forbidden("You can only return podcast ids of your own episodes!"));
    }

    // Get podcast ID from episode name and URL
    let podcast_id = state.db_pool.get_podcast_id_from_episode_name(&query.episode_name, &query.episode_url, query.user_id).await?;
    
    Ok(Json(serde_json::json!({ "podcast_id": podcast_id })))
}

// Request for get_episode_metadata - matches Python EpisodeMetadata model
#[derive(Deserialize)]
pub struct EpisodeMetadataRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub person_episode: Option<bool>,
    pub is_youtube: Option<bool>,
}

// Get episode metadata - matches Python get_episode_metadata endpoint exactly
pub async fn get_episode_metadata(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<EpisodeMetadataRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    // Check if it's web key or user's own key
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == request.user_id || is_web_key {
        let episode = state.db_pool.get_episode_metadata(
            request.episode_id,
            request.user_id,
            request.person_episode.unwrap_or(false),
            request.is_youtube.unwrap_or(false)
        ).await?;
        
        Ok(Json(serde_json::json!({"episode": episode})))
    } else {
        Err(AppError::forbidden("You can only get metadata for yourself!"))
    }
}

// Query parameters for fetch_podcasting_2_data
#[derive(Deserialize)]
pub struct FetchPodcasting2DataQuery {
    pub episode_id: i32,
    pub user_id: i32,
}

// Fetch podcasting 2.0 data for episode - matches Python fetch_podcasting_2_data endpoint exactly
pub async fn fetch_podcasting_2_data(
    Query(query): Query<FetchPodcasting2DataQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key or insufficient permissions"));
    }

    // Get the episode_id and user_id from query parameters  
    let episode_id = query.episode_id;
    let user_id = query.user_id;
    
    // Call the database method to fetch podcasting 2.0 data
    let data = state.db_pool.fetch_podcasting_2_data(episode_id, user_id).await?;
    
    Ok(Json(data))
}

// Request for get_auto_download_status - matches Python AutoDownloadStatusRequest
#[derive(Deserialize)]
pub struct AutoDownloadStatusRequest {
    pub podcast_id: i32,
    pub user_id: i32,
}

// Response for auto download status - matches Python AutoDownloadStatusResponse
#[derive(Serialize)]
pub struct AutoDownloadStatusResponse {
    pub auto_download: bool,
}

// Get auto download status - matches Python get_auto_download_status endpoint exactly
pub async fn get_auto_download_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<AutoDownloadStatusRequest>,
) -> Result<Json<AutoDownloadStatusResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if key_id != request.user_id {
        return Err(AppError::forbidden("You can only get the status for your own podcast."));
    }

    let status = state.db_pool.call_get_auto_download_status(request.podcast_id, request.user_id).await?;
    if status.is_none() {
        return Err(AppError::not_found("Podcast not found"));
    }

    Ok(Json(AutoDownloadStatusResponse {
        auto_download: status.unwrap()
    }))
}

// Query parameters for get_feed_cutoff_days
#[derive(Deserialize)]
pub struct FeedCutoffDaysQuery {
    pub podcast_id: i32,
    pub user_id: i32,
}

// Response for feed cutoff days - matches Python response format
#[derive(Serialize)]
pub struct FeedCutoffDaysResponse {
    pub podcast_id: i32,
    pub user_id: i32,
    pub feed_cutoff_days: i32,
}

// Get feed cutoff days - matches Python get_feed_cutoff_days endpoint exactly
pub async fn get_feed_cutoff_days(
    Query(query): Query<FeedCutoffDaysQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<FeedCutoffDaysResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    // Check if it's web key or user's own key
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == query.user_id || is_web_key {
        let feed_cutoff_days = state.db_pool.get_feed_cutoff_days(query.podcast_id, query.user_id).await?;
        if let Some(cutoff_days) = feed_cutoff_days {
            Ok(Json(FeedCutoffDaysResponse {
                podcast_id: query.podcast_id,
                user_id: query.user_id,
                feed_cutoff_days: cutoff_days
            }))
        } else {
            Err(AppError::not_found("Podcast not found or does not belong to the user."))
        }
    } else {
        Err(AppError::forbidden("You can only access settings of your own podcasts!"))
    }
}

// Request for podcast notification status - matches Python PodcastNotificationStatusData
#[derive(Deserialize)]
pub struct PodcastNotificationStatusRequest {
    pub user_id: i32,
    pub podcast_id: i32,
}

// Response for notification status
#[derive(Serialize)]
pub struct NotificationStatusResponse {
    pub enabled: bool,
}

// Get podcast notification status - matches Python podcast/notification_status endpoint exactly
pub async fn get_notification_status(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<PodcastNotificationStatusRequest>,
) -> Result<Json<NotificationStatusResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == request.user_id || is_web_key {
        let enabled = state.db_pool.get_podcast_notification_status(
            request.podcast_id,
            request.user_id
        ).await?;
        Ok(Json(NotificationStatusResponse { enabled }))
    } else {
        Err(AppError::forbidden("You can only check your own podcast settings"))
    }
}

// Request for get_play_episode_details - matches Python PlayEpisodeDetailsRequest
#[derive(Deserialize)]
pub struct PlayEpisodeDetailsRequest {
    pub podcast_id: i32,
    pub user_id: i32,
    pub is_youtube: Option<bool>,
}

// Response for play episode details - matches Python PlayEpisodeDetailsResponse
#[derive(Serialize)]
pub struct PlayEpisodeDetailsResponse {
    pub playback_speed: f64,
    pub start_skip: i32,
    pub end_skip: i32,
}

// Get play episode details - matches Python get_play_episode_details endpoint exactly
pub async fn get_play_episode_details(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<PlayEpisodeDetailsRequest>,
) -> Result<Json<PlayEpisodeDetailsResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == request.user_id || is_web_key {
        // Get all details in one function call
        let (playback_speed, start_skip, end_skip) = state.db_pool.get_play_episode_details(
            request.user_id,
            request.podcast_id,
            request.is_youtube.unwrap_or(false)
        ).await?;

        Ok(Json(PlayEpisodeDetailsResponse {
            playback_speed,
            start_skip,
            end_skip
        }))
    } else {
        Err(AppError::forbidden("You can only get metadata for yourself!"))
    }
}

// Query parameters for fetch_podcasting_2_pod_data
#[derive(Deserialize)]
pub struct FetchPodcasting2PodDataQuery {
    pub podcast_id: i32,
    pub user_id: i32,
}

// Fetch podcasting 2.0 podcast data - matches Python fetch_podcasting_2_pod_data endpoint exactly
pub async fn fetch_podcasting_2_pod_data(
    Query(query): Query<FetchPodcasting2PodDataQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key or insufficient permissions"));
    }

    // Fetch podcasting 2.0 podcast data
    let data = state.db_pool.fetch_podcasting_2_pod_data(query.podcast_id, query.user_id).await?;
    
    Ok(Json(data))
}

// Request for mark_episode_completed - matches Python MarkEpisodeCompletedData
#[derive(Deserialize)]
pub struct MarkEpisodeCompletedRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub is_youtube: Option<bool>,
}

// Mark episode as completed - matches Python mark_episode_completed endpoint exactly
pub async fn mark_episode_completed(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<MarkEpisodeCompletedRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == request.user_id || is_web_key {
        state.db_pool.mark_episode_completed(
            request.episode_id,
            request.user_id,
            request.is_youtube.unwrap_or(false)
        ).await?;
        
        Ok(Json(serde_json::json!({ "detail": "Episode marked as completed." })))
    } else {
        Err(AppError::forbidden("You can only mark episodes as completed for yourself."))
    }
}

// Increment played count - matches Python increment_played endpoint exactly
pub async fn increment_played(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == user_id || is_web_key {
        state.db_pool.increment_played(user_id).await?;
        
        Ok(Json(serde_json::json!({ "detail": "Played count incremented." })))
    } else {
        Err(AppError::forbidden("You can only increment your own play count."))
    }
}

// Query parameters for get_podcast_id_from_ep_id
#[derive(Deserialize)]
pub struct GetPodcastIdFromEpIdQuery {
    pub episode_id: i32,
    pub user_id: i32,
    pub is_youtube: Option<bool>,
}

// Get podcast ID from episode ID - matches Python get_podcast_id_from_ep_id endpoint exactly
pub async fn get_podcast_id_from_ep_id(
    Query(query): Query<GetPodcastIdFromEpIdQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == query.user_id || is_web_key {
        let podcast_id = state.db_pool.get_podcast_id_from_episode(
            query.episode_id,
            query.user_id,
            query.is_youtube.unwrap_or(false)
        ).await?;
        
        if let Some(podcast_id) = podcast_id {
            Ok(Json(serde_json::json!({ "podcast_id": podcast_id })))
        } else {
            Err(AppError::not_found("Episode not found or does not belong to user"))
        }
    } else {
        Err(AppError::forbidden("You can only return podcast ids of your own podcasts!"))
    }
}

// Query parameters for get_stats
#[derive(Deserialize)]
pub struct GetStatsQuery {
    pub user_id: i32,
}

// Get user stats - matches Python get_stats endpoint exactly
pub async fn get_stats(
    Query(query): Query<GetStatsQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == query.user_id || is_web_key {
        let stats = state.db_pool.get_stats(query.user_id).await?;
        
        if let Some(stats) = stats {
            Ok(Json(stats))
        } else {
            Err(AppError::not_found("Stats not found for the given user ID"))
        }
    } else {
        Err(AppError::forbidden("You can only get stats for your own account."))
    }
}

// Get PinePods version - matches Python get_pinepods_version endpoint exactly
pub async fn get_pinepods_version(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let version = state.db_pool.get_pinepods_version().await?;
    
    Ok(Json(serde_json::json!({ "data": version })))
}

// Request for search_data - matches Python SearchPodcastData
#[derive(Deserialize)]
pub struct SearchDataRequest {
    pub search_term: String,
    pub user_id: i32,
}

// Search data - matches Python search_data endpoint exactly
pub async fn search_data(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<SearchDataRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let result = state.db_pool.search_data(&request.search_term, request.user_id).await?;
    
    Ok(Json(serde_json::json!({ "data": result })))
}

// Query struct for home_overview
#[derive(Deserialize)]
pub struct HomeOverviewQuery {
    pub user_id: i32,
}

// Get home overview - matches Python api_home_overview function
pub async fn home_overview(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<HomeOverviewQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only access their own data
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id != query.user_id {
        return Err(AppError::forbidden("You can only view your own home overview!"));
    }

    let home_data = state.db_pool.get_home_overview(query.user_id).await?;
    
    Ok(Json(home_data))
}

// Query struct for get_playlists
#[derive(Deserialize)]
pub struct GetPlaylistsQuery {
    pub user_id: i32,
}

// Get playlists - matches Python api_get_playlists function
pub async fn get_playlists(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<GetPlaylistsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only access their own data
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id != query.user_id {
        return Err(AppError::forbidden("You can only view your own playlists!"));
    }

    let playlists = state.db_pool.get_playlists(query.user_id).await?;
    
    Ok(Json(serde_json::json!({ "playlists": playlists })))
}

// Request struct for mark_episode_uncompleted
#[derive(Deserialize)]
pub struct MarkEpisodeUncompletedRequest {
    pub episode_id: i32,
    pub user_id: i32,
    #[serde(default)]
    pub is_youtube: bool,
}

// Mark episode as uncompleted - matches Python api_mark_episode_uncompleted function
pub async fn mark_episode_uncompleted(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(request): Json<MarkEpisodeUncompletedRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only mark their own episodes as uncompleted
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id != request.user_id {
        return Err(AppError::forbidden("You can only mark episodes as uncompleted for yourself."));
    }

    state.db_pool.mark_episode_uncompleted(request.episode_id, request.user_id, request.is_youtube).await?;
    
    Ok(Json(serde_json::json!({ "detail": "Episode marked as uncompleted." })))
}

// Request struct for record_listen_duration
#[derive(Deserialize)]
pub struct RecordListenDurationRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub listen_duration: f64,
    #[serde(default)]
    pub is_youtube: bool,
}

// Record listen duration - matches Python api record_listen_duration function exactly
pub async fn record_listen_duration(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Json(data): Json<RecordListenDurationRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Ignore listen duration for episodes with ID 0
    if data.episode_id == 0 {
        return Ok(Json(serde_json::json!({ "detail": "Listen duration for episode ID 0 is ignored." })));
    }

    // Check authorization - web key or user can only record their own duration
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != data.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only record your own listen duration"));
    }

    if data.is_youtube {
        state.db_pool.record_youtube_listen_duration(data.episode_id, data.user_id, data.listen_duration).await?;
    } else {
        state.db_pool.record_listen_duration(data.episode_id, data.user_id, data.listen_duration).await?;
    }

    Ok(Json(serde_json::json!({ "detail": "Listen duration recorded." })))
}

// Get user history - matches Python user_history endpoint exactly
pub async fn user_history(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only get their own history
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only return history for yourself!"));
    }

    let history = state.db_pool.user_history(user_id).await?;
    Ok(Json(serde_json::json!({ "data": history })))
}

// Increment listen time - matches Python increment_listen_time endpoint exactly
pub async fn increment_listen_time(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only increment their own time
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only increment your own listen time."));
    }

    state.db_pool.increment_listen_time(user_id).await?;
    Ok(Json(serde_json::json!({ "detail": "Listen time incremented." })))
}

// Request struct for get_playback_speed
#[derive(Deserialize)]
pub struct GetPlaybackSpeedRequest {
    pub user_id: i32,
    pub podcast_id: Option<i32>,
}

// Get playback speed - matches Python get_playback_speed endpoint exactly
pub async fn get_playback_speed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(data): Json<GetPlaybackSpeedRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only get their own playback speed
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != data.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only get metadata for yourself!"));
    }

    let playback_speed = state.db_pool.get_playback_speed(data.user_id, false, data.podcast_id).await?;
    Ok(Json(serde_json::json!({ "playback_speed": playback_speed })))
}

// Query struct for get_playlist_episodes
#[derive(Deserialize)]
pub struct GetPlaylistEpisodesQuery {
    pub user_id: i32,
    pub playlist_id: i32,
}

// Get playlist episodes - matches Python api_get_playlist_episodes function
pub async fn get_playlist_episodes(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<GetPlaylistEpisodesQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only access their own playlists
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id != query.user_id {
        return Err(AppError::forbidden("You can only view your own playlist episodes!"));
    }

    let playlist_episodes = state.db_pool.get_playlist_episodes(query.user_id, query.playlist_id).await?;
    
    Ok(Json(playlist_episodes))
}

// Get podcast details - matches Python get_podcast_details endpoint
pub async fn get_podcast_details(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<GetPodcastDetailsQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    // Check authorization - user can only access their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    if key_id != query.user_id {
        return Err(AppError::forbidden("You can only view your own podcast details!"));
    }
    
    let podcast_details = state.db_pool.get_podcast_details(query.user_id, query.podcast_id).await?;
    
    Ok(Json(podcast_details))
}