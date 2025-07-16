use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    services::tasks::{download_episode_task, import_opml_task, refresh_all_feeds_task},
    AppState,
};

#[derive(Deserialize)]
pub struct DownloadEpisodeRequest {
    pub episode_id: String,
    pub url: String,
}

#[derive(Deserialize)]
pub struct ImportOpmlRequest {
    pub opml_content: String,
}

#[derive(Serialize)]
pub struct TaskResponse {
    pub task_id: String,
    pub message: String,
}

// Download episode - returns immediately with task ID
pub async fn download_episode(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<DownloadEpisodeRequest>,
) -> Result<Json<TaskResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // This returns immediately and starts the download in the background
    let task_id = state
        .task_spawner
        .spawn_progress_task(
            "download_episode".to_string(),
            1, // TODO: Extract user_id from API key
            move |reporter| {
                download_episode_task(request.episode_id, request.url, reporter)
            },
        )
        .await?;

    Ok(Json(TaskResponse {
        task_id,
        message: "Download started. Check progress via WebSocket or task status endpoint.".to_string(),
    }))
}

// Import OPML - returns immediately with task ID
pub async fn import_opml(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<ImportOpmlRequest>,
) -> Result<Json<TaskResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // This returns immediately and starts the import in the background
    let task_id = state
        .task_spawner
        .spawn_progress_task(
            "import_opml".to_string(),
            1, // TODO: Extract user_id from API key
            move |reporter| {
                import_opml_task(request.opml_content, reporter)
            },
        )
        .await?;

    Ok(Json(TaskResponse {
        task_id,
        message: "OPML import started. Check progress via WebSocket or task status endpoint.".to_string(),
    }))
}

// Refresh all feeds for user - returns immediately with task ID
pub async fn refresh_all_feeds(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<TaskResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = 1; // TODO: Extract user_id from API key

    // This returns immediately and starts the refresh in the background
    let task_id = state
        .task_spawner
        .spawn_progress_task(
            "refresh_all_feeds".to_string(),
            user_id,
            move |reporter| {
                refresh_all_feeds_task(user_id, reporter)
            },
        )
        .await?;

    Ok(Json(TaskResponse {
        task_id,
        message: "Feed refresh started. Check progress via WebSocket or task status endpoint.".to_string(),
    }))
}

// Example of a simple async operation (no progress tracking needed)
pub async fn quick_metadata_fetch(
    headers: HeaderMap,
    State(state): State<AppState>,
    Path(episode_id): Path<String>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // This is a quick operation that can be done directly without background tasks
    // But it's still async and non-blocking
    let metadata = fetch_episode_metadata(&episode_id).await?;

    Ok(Json(metadata))
}

// Helper function - demonstrates async I/O without blocking
async fn fetch_episode_metadata(episode_id: &str) -> AppResult<serde_json::Value> {
    // Simulate async I/O operation (database lookup, HTTP request, etc.)
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok(json!({
        "episode_id": episode_id,
        "title": "Example Episode",
        "duration": 3600,
        "file_size": 52428800,
        "status": "ready"
    }))
}