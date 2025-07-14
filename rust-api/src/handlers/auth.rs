use axum::{
    extract::State,
    http::HeaderMap,
    response::Json,
};
use serde_json::json;
use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

/// Verify API key endpoint - matches Python API exactly
/// GET /api/data/verify_key
pub async fn verify_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let api_key = extract_api_key(&headers)?;
    
    let is_valid = validate_api_key(&state, &api_key).await?;
    
    if is_valid {
        Ok(Json(json!({"status": "success"})))
    } else {
        Err(AppError::Auth("Your API key is either invalid or does not have correct permission".to_string()))
    }
}

/// Get user associated with API key - matches Python API  
/// GET /api/data/get_user
pub async fn get_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let api_key = extract_api_key(&headers)?;
    
    let is_valid = validate_api_key(&state, &api_key).await?;
    
    if !is_valid {
        return Err(AppError::Auth("Your api-key appears to be incorrect.".to_string()));
    }

    // Get the user ID for this API key
    let user_id = state.db_pool.get_api_user(&api_key).await?;
    
    Ok(Json(json!({
        "status": "success",
        "retrieved_id": user_id
    })))
}