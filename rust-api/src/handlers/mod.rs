pub mod auth;
pub mod health;
pub mod podcasts;
pub mod episodes;
pub mod playlists;
pub mod collections;
pub mod websocket;
pub mod notifications;
// pub mod async_tasks_examples;  // File was deleted
pub mod refresh;
pub mod proxy;
pub mod settings;
pub mod sync;
pub mod youtube;
pub mod tasks;
pub mod feed;
pub mod local_podcast;
pub mod nowplaying;

// Common handler utilities
use axum::http::HeaderMap;
use crate::{
    error::{AppError, AppResult},
    AppState,
};

// Extract API key from headers (matches Python API behavior)
pub fn extract_api_key(headers: &HeaderMap) -> AppResult<String> {
    headers
        .get("Api-Key")
        .or_else(|| headers.get("api-key"))
        .or_else(|| headers.get("X-API-Key"))
        .and_then(|header| header.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| AppError::unauthorized("Missing API key"))
}

// Validate API key against database/cache
pub async fn validate_api_key(state: &AppState, api_key: &str) -> AppResult<bool> {
    // First check Redis cache
    if let Ok(Some(is_valid)) = state.redis_client.get_cached_api_key_validation(api_key).await {
        return Ok(is_valid);
    }

    // If not in cache, check database
    let is_valid = state.db_pool.verify_api_key(api_key).await?;
    
    // Cache the result for 5 minutes
    if let Err(e) = state.redis_client.cache_api_key_validation(api_key, is_valid, 300).await {
        tracing::warn!("Failed to cache API key validation: {}", e);
    }

    Ok(is_valid)
}

/// Fail closed: `Err(401)` unless `api_key` is a real key. Use this instead of
/// `validate_api_key(..).await?;`, which discards the bool and lets *any* key through.
pub async fn require_valid_api_key(state: &AppState, api_key: &str) -> AppResult<()> {
    if validate_api_key(state, api_key).await? {
        Ok(())
    } else {
        Err(AppError::unauthorized("Invalid API key"))
    }
}

/// Fail closed for admin-only endpoints: returns the caller's user id if they're an admin
/// (or the background/web key, user 1), `Err(401/403)` otherwise.
pub async fn require_admin(state: &AppState, headers: &HeaderMap) -> AppResult<i32> {
    let api_key = extract_api_key(headers)?;
    require_valid_api_key(state, &api_key).await?;
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if user_id == 1 || state.db_pool.user_admin_check(user_id).await? {
        Ok(user_id)
    } else {
        Err(AppError::forbidden("Admin access required"))
    }
}

/// Fail closed: `Err(403)` unless the key's owner is `target_user_id` (or the web key).
pub async fn require_user_access(state: &AppState, api_key: &str, target_user_id: i32) -> AppResult<()> {
    if check_user_access(state, api_key, target_user_id).await? {
        Ok(())
    } else {
        Err(AppError::forbidden("You can only access your own data"))
    }
}

/// Fail closed: `Err(404)` unless the episode (or YouTube video) is in one of `user_id`'s
/// podcasts. Call after establishing the caller may act as `user_id`. 404 rather than 403 so
/// other users' (sequential) episode ids can't be probed for existence.
pub async fn require_episode_owner(state: &AppState, user_id: i32, episode_id: i32, is_youtube: bool) -> AppResult<()> {
    if state.db_pool.episode_belongs_to_user(episode_id, user_id, is_youtube).await? {
        Ok(())
    } else {
        Err(AppError::not_found("Episode not found"))
    }
}

/// [`require_episode_owner`] for every id in a bulk request; one foreign id rejects the batch.
pub async fn require_episodes_owner(state: &AppState, user_id: i32, episode_ids: &[i32], is_youtube: bool) -> AppResult<()> {
    for &id in episode_ids {
        require_episode_owner(state, user_id, id, is_youtube).await?;
    }
    Ok(())
}

// Check if user has permission (either owns the resource or has web key/admin access)
pub async fn check_user_access(state: &AppState, api_key: &str, target_user_id: i32) -> AppResult<bool> {
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(api_key).await?;
    
    // Allow if user is accessing their own data or if they are user ID 1 (admin/web key)
    Ok(requesting_user_id == target_user_id || requesting_user_id == 1)
}

// Check if user has permission (either owns the resource, has web key access, or is admin)
pub async fn check_user_or_admin_access(state: &AppState, api_key: &str, target_user_id: i32) -> AppResult<bool> {
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(api_key).await?;
    
    // Allow if user is accessing their own data, has web key access, or is admin
    if requesting_user_id == target_user_id || requesting_user_id == 1 {
        Ok(true)
    } else {
        // Check if user is admin
        state.db_pool.user_admin_check(requesting_user_id).await
    }
}