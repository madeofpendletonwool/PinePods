use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};
use crate::{
    error::AppResult,
    services::homepage,
    AppState,
};

#[derive(Debug, Deserialize)]
pub struct TrendingQuery {
    pub max: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TrendingResponse {
    pub podcasts: Vec<homepage::TrendingPodcast>,
    pub count: usize,
}

/// Get trending podcasts for homepage
/// GET /api/data/homepage/trending
pub async fn get_trending(
    State(state): State<AppState>,
    Query(params): Query<TrendingQuery>,
) -> AppResult<Json<TrendingResponse>> {
    // Default to 6 podcasts if no max specified
    let max = params.max.or(Some(6));

    let podcasts = homepage::get_trending_podcasts(&state.redis_client, max).await?;
    let count = podcasts.len();

    Ok(Json(TrendingResponse { podcasts, count }))
}

/// Invalidate trending cache (admin only)
/// POST /api/data/homepage/trending/invalidate
pub async fn invalidate_trending_cache(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> AppResult<Json<serde_json::Value>> {
    let api_key = super::extract_api_key(&headers)?;

    let is_valid = super::validate_api_key(&state, &api_key).await?;
    if !is_valid {
        return Err(crate::error::AppError::forbidden("Invalid API key"));
    }

    // Check if user is admin
    let is_admin = super::check_admin_access(&state, &api_key).await?;
    if !is_admin {
        return Err(crate::error::AppError::forbidden("Admin access required"));
    }

    homepage::invalidate_trending_cache(&state.redis_client).await?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Trending cache invalidated"
    })))
}

#[derive(Debug, Serialize)]
pub struct RecommendedResponse {
    pub podcasts: Vec<crate::models::PodcastRecommendation>,
    pub count: usize,
}

/// Get recommended podcasts for a user
/// GET /api/data/homepage/recommended?user_id=123
#[derive(Debug, Deserialize)]
pub struct RecommendedQuery {
    pub user_id: i32,
    pub limit: Option<i32>,
}

pub async fn get_recommended(
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(params): Query<RecommendedQuery>,
) -> AppResult<Json<RecommendedResponse>> {
    let api_key = super::extract_api_key(&headers)?;

    let is_valid = super::validate_api_key(&state, &api_key).await?;
    if !is_valid {
        return Err(crate::error::AppError::forbidden("Invalid API key"));
    }

    // Check if user has access to this data
    let has_access = super::check_user_or_admin_access(&state, &api_key, params.user_id).await?;
    if !has_access {
        return Err(crate::error::AppError::forbidden("Access denied"));
    }

    // Default to 6 podcasts if no limit specified
    let limit = params.limit.or(Some(6));

    let recommendations = state.db_pool.get_user_recommendations(params.user_id, limit).await?;
    let count = recommendations.len();

    Ok(Json(RecommendedResponse {
        podcasts: recommendations,
        count,
    }))
}

/// Generate recommendations for a specific user (admin or user themselves)
/// POST /api/data/homepage/recommended/generate
#[derive(Debug, Deserialize)]
pub struct GenerateRecommendationsRequest {
    pub user_id: i32,
}

pub async fn generate_recommendations(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<GenerateRecommendationsRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let api_key = super::extract_api_key(&headers)?;

    let is_valid = super::validate_api_key(&state, &api_key).await?;
    if !is_valid {
        return Err(crate::error::AppError::forbidden("Invalid API key"));
    }

    // Check if user has access
    let has_access = super::check_user_or_admin_access(&state, &api_key, req.user_id).await?;
    if !has_access {
        return Err(crate::error::AppError::forbidden("Access denied"));
    }

    homepage::generate_user_recommendations(&state.db_pool, &state.redis_client, req.user_id).await?;

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Recommendations generated"
    })))
}
