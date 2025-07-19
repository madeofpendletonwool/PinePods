pub mod auth;
pub mod health;
pub mod podcasts;
pub mod episodes;
pub mod playlists;
pub mod users;
pub mod websocket;
pub mod async_tasks_examples;
pub mod refresh;
pub mod proxy;

// Common handler utilities
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
};
use crate::{
    error::{AppError, AppResult},
    models::PaginationParams,
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

// Extract and validate pagination parameters
pub fn extract_pagination(Query(params): Query<PaginationParams>) -> (i32, i32) {
    let page = params.page.unwrap_or(1).max(1);
    let per_page = params.per_page.unwrap_or(50).min(100).max(1); // Limit to 100 per page
    (page, per_page)
}

// Calculate offset for SQL queries
pub fn calculate_offset(page: i32, per_page: i32) -> i32 {
    (page - 1) * per_page
}

// Common response helpers
pub fn success_response() -> (StatusCode, &'static str) {
    (StatusCode::OK, "success")
}

pub fn created_response() -> (StatusCode, &'static str) {
    (StatusCode::CREATED, "created")
}

pub fn no_content_response() -> StatusCode {
    StatusCode::NO_CONTENT
}