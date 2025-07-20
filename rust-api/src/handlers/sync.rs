use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

// Set default gPodder device - matches Python set_default_device function exactly
pub async fn gpodder_set_default(
    State(state): State<AppState>,
    Path(device_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let success = state.db_pool.gpodder_set_default_device(user_id, device_id).await?;
    
    if success {
        Ok(Json(serde_json::json!({ "status": "success" })))
    } else {
        Err(AppError::internal("Failed to set default device"))
    }
}

// Get gPodder devices for user - matches Python get_devices function exactly
pub async fn gpodder_get_user_devices(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or own user
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only view your own devices!"));
    }

    let devices = state.db_pool.gpodder_get_user_devices(user_id).await?;
    Ok(Json(serde_json::json!({ "devices": devices })))
}

// Get all gPodder devices - matches Python get_all_devices function exactly
pub async fn gpodder_get_all_devices(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let devices = state.db_pool.gpodder_get_user_devices(user_id).await?;
    Ok(Json(serde_json::json!({ "devices": devices })))
}

// Force sync gPodder - matches Python force_sync function exactly
pub async fn gpodder_force_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let success = state.db_pool.gpodder_force_sync(user_id).await?;
    
    if success {
        Ok(Json(serde_json::json!({ "status": "success" })))
    } else {
        Err(AppError::internal("Failed to force sync"))
    }
}

// Regular gPodder sync - matches Python sync function exactly
pub async fn gpodder_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let sync_result = state.db_pool.gpodder_sync(user_id).await?;
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "synced_podcasts": sync_result.synced_podcasts,
        "synced_episodes": sync_result.synced_episodes
    })))
}

// Get gPodder status - matches Python get_gpodder_status function exactly
pub async fn gpodder_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let status = state.db_pool.gpodder_get_status(user_id).await?;
    
    Ok(Json(serde_json::json!({
        "gpodder_enabled": status.enabled,
        "sync_type": status.sync_type,
        "last_sync": status.last_sync
    })))
}

// Toggle gPodder sync - matches Python toggle_gpodder function exactly
pub async fn gpodder_toggle(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let new_status = state.db_pool.gpodder_toggle_sync(user_id).await?;
    
    Ok(Json(serde_json::json!({
        "status": "success",
        "enabled": new_status
    })))
}

// gPodder test connection - matches Python test connection functionality
pub async fn gpodder_test_connection(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = params.get("user_id")
        .ok_or_else(|| AppError::bad_request("Missing user_id parameter"))?
        .parse::<i32>()
        .map_err(|_| AppError::bad_request("Invalid user_id format"))?;
    
    let gpodder_url = params.get("gpodder_url")
        .ok_or_else(|| AppError::bad_request("Missing gpodder_url parameter"))?;
    let gpodder_username = params.get("gpodder_username")
        .ok_or_else(|| AppError::bad_request("Missing gpodder_username parameter"))?;
    let gpodder_password = params.get("gpodder_password")
        .ok_or_else(|| AppError::bad_request("Missing gpodder_password parameter"))?;

    // Check authorization
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only test connections for yourself!"));
    }

    let verified = state.db_pool.verify_gpodder_auth(gpodder_url, gpodder_username, gpodder_password).await?;
    Ok(Json(serde_json::json!({ "verified": verified })))
}

// Get default gPodder device - matches Python get_default_device function exactly
pub async fn gpodder_get_default_device(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let default_device = state.db_pool.gpodder_get_default_device(user_id).await?;
    
    Ok(Json(serde_json::json!({ "default_device": default_device })))
}

// Create gPodder device - matches Python create_device function exactly
#[derive(serde::Deserialize)]
pub struct CreateDeviceRequest {
    pub device_name: String,
    pub device_type: String,
    pub device_caption: Option<String>,
}

pub async fn gpodder_create_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateDeviceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let device_id = state.db_pool.gpodder_create_device_with_caption(
        user_id, 
        &request.device_name, 
        &request.device_type, 
        request.device_caption.as_deref(),
        false
    ).await?;
    
    Ok(Json(serde_json::json!({ 
        "status": "success",
        "device_id": device_id,
        "device_name": request.device_name 
    })))
}