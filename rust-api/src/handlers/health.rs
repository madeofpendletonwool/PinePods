use axum::{extract::State, response::Json};
use chrono::Utc;
use crate::{
    error::AppResult,
    models::{HealthResponse, PinepodsCheckResponse},
    AppState,
};

/// PinePods instance check endpoint - matches Python API exactly
/// GET /api/pinepods_check
pub async fn pinepods_check() -> Json<PinepodsCheckResponse> {
    Json(PinepodsCheckResponse {
        status_code: 200,
        pinepods_instance: true,
    })
}

/// Health check endpoint with database and Redis status
/// GET /api/health
pub async fn health_check(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    // Check database health
    let database_healthy = state.db_pool.health_check().await.unwrap_or(false);
    
    // Check Redis health
    let redis_healthy = state.redis_client.health_check().await.unwrap_or(false);
    
    let overall_status = if database_healthy && redis_healthy {
        "healthy"
    } else {
        "unhealthy"
    };

    Ok(Json(HealthResponse {
        status: overall_status.to_string(),
        database: database_healthy,
        redis: redis_healthy,
        timestamp: Utc::now(),
    }))
}