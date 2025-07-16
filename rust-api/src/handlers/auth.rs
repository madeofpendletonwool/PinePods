use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    services::auth::{hash_password, verify_password},
    AppState,
};

#[derive(Serialize)]
pub struct LoginResponse {
    status: String,
    retrieved_key: String,
}

#[derive(Serialize)]
pub struct VerifyKeyResponse {
    status: String,
}

#[derive(Serialize)]
pub struct GetUserResponse {
    status: String,
    retrieved_id: i32,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
pub struct UserDetails {
    pub UserID: i32,
    pub Fullname: Option<String>,
    pub Username: Option<String>,
    pub Email: Option<String>,
    pub Hashed_PW: Option<String>,
    pub Salt: Option<String>,
}

// Extract basic auth credentials from Authorization header
fn extract_basic_auth(headers: &HeaderMap) -> AppResult<(String, String)> {
    let auth_header = headers
        .get("Authorization")
        .ok_or_else(|| AppError::unauthorized("Missing Authorization header"))?
        .to_str()
        .map_err(|_| AppError::unauthorized("Invalid Authorization header"))?;

    if !auth_header.starts_with("Basic ") {
        return Err(AppError::unauthorized("Invalid Authorization scheme"));
    }

    let encoded = &auth_header[6..]; // Remove "Basic " prefix
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| AppError::unauthorized("Invalid base64 encoding"))?;
    
    let credentials = String::from_utf8(decoded)
        .map_err(|_| AppError::unauthorized("Invalid UTF-8 in credentials"))?;
    
    let parts: Vec<&str> = credentials.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(AppError::unauthorized("Invalid credentials format"));
    }

    Ok((parts[0].to_lowercase(), parts[1].to_string()))
}

// Get API key with basic authentication (username/password)
pub async fn get_key(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<LoginResponse>, AppError> {
    let (username, password) = extract_basic_auth(&headers)?;
    
    // Verify password
    let is_valid = state.db_pool.verify_password(&username, &password).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid username or password"));
    }

    // Get API key for user
    let api_key = state.db_pool.get_api_key(&username).await?;
    
    Ok(Json(LoginResponse {
        status: "success".to_string(),
        retrieved_key: api_key,
    }))
}

// Verify API key validity
pub async fn verify_api_key_endpoint(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<VerifyKeyResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    Ok(Json(VerifyKeyResponse {
        status: "success".to_string(),
    }))
}

// Get user ID from API key
pub async fn get_user(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<GetUserResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    Ok(Json(GetUserResponse {
        status: "success".to_string(),
        retrieved_id: user_id,
    }))
}

// Get user details by user ID
pub async fn get_user_details_by_id(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<UserDetails>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Get user ID from API key for authorization check
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Basic authorization: users can only get their own details (unless admin)
    // TODO: Add admin check
    if requesting_user_id != user_id {
        return Err(AppError::forbidden("Access denied to user details"));
    }

    // Get user details
    let user_details = state.db_pool.get_user_details_by_id(user_id).await?;
    
    Ok(Json(user_details))
}