use axum::{extract::State, http::HeaderMap, response::Json};
use serde_json::json;
use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    models::{BulkEpisodeActionRequest, BulkEpisodeActionResponse},
    AppState,
};

// Bulk episode action handlers for efficient mass operations

// Bulk mark episodes as completed
pub async fn bulk_mark_episodes_completed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only mark episodes as completed for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let (processed_count, failed_count) = state.db_pool
        .bulk_mark_episodes_completed(request.episode_ids, request.user_id, is_youtube)
        .await?;

    let message = if failed_count > 0 {
        format!("Marked {} episodes as completed, {} failed", processed_count, failed_count)
    } else {
        format!("Successfully marked {} episodes as completed", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Bulk save episodes
pub async fn bulk_save_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only save episodes for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let (processed_count, failed_count) = state.db_pool
        .bulk_save_episodes(request.episode_ids, request.user_id, is_youtube)
        .await?;

    let message = if failed_count > 0 {
        format!("Saved {} episodes, {} failed or already saved", processed_count, failed_count)
    } else {
        format!("Successfully saved {} episodes", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Bulk queue episodes
pub async fn bulk_queue_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only queue episodes for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let (processed_count, failed_count) = state.db_pool
        .bulk_queue_episodes(request.episode_ids, request.user_id, is_youtube)
        .await?;

    let message = if failed_count > 0 {
        format!("Queued {} episodes, {} failed or already queued", processed_count, failed_count)
    } else {
        format!("Successfully queued {} episodes", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Bulk download episodes - triggers download tasks
pub async fn bulk_download_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only download episodes for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let mut processed_count = 0;
    let mut failed_count = 0;

    // Check if episodes are already downloaded and queue download tasks
    for episode_id in request.episode_ids {
        let is_downloaded = state.db_pool
            .check_downloaded(request.user_id, episode_id, is_youtube)
            .await?;

        if !is_downloaded {
            let result = if is_youtube {
                state.task_spawner.spawn_download_youtube_video(episode_id, request.user_id).await
            } else {
                state.task_spawner.spawn_download_podcast_episode(episode_id, request.user_id).await
            };

            match result {
                Ok(_) => processed_count += 1,
                Err(_) => failed_count += 1,
            }
        }
    }

    let message = if failed_count > 0 {
        format!("Queued {} episodes for download, {} failed or already downloaded", processed_count, failed_count)
    } else {
        format!("Successfully queued {} episodes for download", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Bulk delete downloaded episodes - removes multiple downloaded episodes at once
pub async fn bulk_delete_downloaded_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only delete your own downloaded episodes!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let (processed_count, failed_count) = state.db_pool
        .bulk_delete_downloaded_episodes(request.episode_ids, request.user_id, is_youtube)
        .await?;

    let message = if failed_count > 0 {
        format!("Deleted {} downloaded episodes, {} failed or were not found", processed_count, failed_count)
    } else {
        format!("Successfully deleted {} downloaded episodes", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}