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

#[derive(Debug, Deserialize)]
pub struct UpdateGpodderSyncRequest {
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct RemoveSyncRequest {
    pub user_id: i32,
}

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
    Ok(Json(serde_json::json!(devices)))
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
    Ok(Json(serde_json::json!(devices)))
}

// Force sync gPodder - performs initial full sync without timestamps (like setup)
pub async fn gpodder_force_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Get user's sync settings to determine which sync method to use
    let sync_settings = state.db_pool.get_user_sync_settings(user_id).await?;
    if sync_settings.is_none() {
        return Ok(Json(serde_json::json!({
            "success": false,
            "message": "No sync configured for this user",
            "data": null
        })));
    }
    
    let settings = sync_settings.unwrap();
    let device_name = state.db_pool.get_or_create_default_device(user_id).await?;
    
    // Perform initial full sync (without timestamps) based on sync type
    let sync_result = match settings.sync_type.as_str() {
        "gpodder" => {
            // Internal gPodder API - call initial full sync
            state.db_pool.call_gpodder_initial_full_sync(user_id, "http://localhost:8042", &settings.username, &settings.token, &device_name).await
        }
        "nextcloud" => {
            // Nextcloud initial sync
            state.db_pool.call_nextcloud_initial_full_sync(user_id, &settings.url, &settings.username, &settings.token).await
        }
        "external" => {
            // External gPodder server - decrypt token first then call initial full sync
            let decrypted_token = state.db_pool.decrypt_password(&settings.token).await.unwrap_or_default();
            state.db_pool.call_gpodder_initial_full_sync(user_id, &settings.url, &settings.username, &decrypted_token, &device_name).await
        }
        "both" => {
            // Both internal and external - call initial sync for both
            let internal_result = state.db_pool.call_gpodder_initial_full_sync(user_id, "http://localhost:8042", &settings.username, &settings.token, &device_name).await;
            let decrypted_token = state.db_pool.decrypt_password(&settings.token).await.unwrap_or_default();
            let external_result = state.db_pool.call_gpodder_initial_full_sync(user_id, &settings.url, &settings.username, &decrypted_token, &device_name).await;
            
            match (internal_result, external_result) {
                (Ok(internal_success), Ok(external_success)) => Ok(internal_success || external_success),
                (Ok(internal_success), Err(external_err)) => {
                    tracing::warn!("External sync failed: {}, but internal sync succeeded: {}", external_err, internal_success);
                    Ok(internal_success)
                }
                (Err(internal_err), Ok(external_success)) => {
                    tracing::warn!("Internal sync failed: {}, but external sync succeeded: {}", internal_err, external_success);
                    Ok(external_success)
                }
                (Err(internal_err), Err(external_err)) => {
                    tracing::error!("Both internal and external sync failed: internal={}, external={}", internal_err, external_err);
                    Err(internal_err)
                }
            }
        }
        _ => Ok(false)
    };
    
    let (success, error_message) = match sync_result {
        Ok(result) => (result, None),
        Err(e) => {
            tracing::error!("Sync failed with error: {}", e);
            (false, Some(e.to_string()))
        }
    };
    
    if success {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Initial sync completed successfully - all data refreshed",
            "data": null
        })))
    } else {
        let message = error_message.unwrap_or_else(|| "Initial sync failed - please check your sync configuration".to_string());
        Ok(Json(serde_json::json!({
            "success": false,
            "message": format!("Initial sync failed: {}", message),
            "data": null
        })))
    }
}

// Regular gPodder sync - performs standard incremental sync with timestamps (like tasks.rs)
pub async fn gpodder_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Use the same sync process as the scheduler (tasks.rs) which uses proper API calls with timestamps
    let sync_result = state.db_pool.refresh_gpodder_subscription_background(user_id).await?;
    
    if sync_result {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Sync completed successfully",
            "data": null
        })))
    } else {
        Ok(Json(serde_json::json!({
            "success": false,
            "message": "Sync failed or no changes detected - check your sync configuration",
            "data": null
        })))
    }
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
        "sync_type": status.sync_type,
        "gpodder_enabled": status.sync_type == "gpodder" || status.sync_type == "both" || status.sync_type == "external",
        "external_enabled": status.sync_type == "external" || status.sync_type == "both",
        "external_url": status.gpodder_url,
        "api_url": "http://localhost:8042" 
    })))
}

// Toggle gPodder sync - matches Python toggle_gpodder_sync function exactly  
pub async fn gpodder_toggle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateGpodderSyncRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Get current user status to match Python logic
    let user_status = state.db_pool.gpodder_get_status(user_id).await?;
    let current_sync_type = &user_status.sync_type;
    
    let mut device_info: Option<serde_json::Value> = None;
    
    if request.enabled {
        // Enable gpodder sync - call function that matches Python set_gpodder_internal_sync
        if let Ok(result) = state.db_pool.set_gpodder_internal_sync(user_id).await {
            device_info = Some(result);
        } else {
            return Err(AppError::internal("Failed to enable gpodder sync"));
        }
        
        // Add background task for subscription refresh (matches Python background_tasks.add_task)
        let db_pool = state.db_pool.clone();
        let _task_id = state.task_spawner.spawn_progress_task(
            "gpodder_subscription_refresh".to_string(),
            user_id,
            move |reporter| async move {
                reporter.update_progress(10.0, Some("Starting GPodder subscription refresh...".to_string())).await?;
                
                let success = db_pool.refresh_gpodder_subscription_background(user_id).await
                    .map_err(|e| AppError::internal(&format!("GPodder sync failed: {}", e)))?;
                
                if success {
                    reporter.update_progress(100.0, Some("GPodder subscription refresh completed successfully".to_string())).await?;
                    Ok(serde_json::json!({"status": "GPodder subscription refresh completed successfully"}))
                } else {
                    reporter.update_progress(100.0, Some("GPodder subscription refresh completed with no changes".to_string())).await?;
                    Ok(serde_json::json!({"status": "No sync performed"}))
                }
            },
        ).await?;
    } else {
        // Disable gpodder sync - call function that matches Python disable_gpodder_internal_sync  
        if !state.db_pool.disable_gpodder_internal_sync(user_id).await? {
            return Err(AppError::internal("Failed to disable gpodder sync"));
        }
    }
    
    // Get updated status after changes
    let updated_status = state.db_pool.gpodder_get_status(user_id).await?;
    let new_sync_type = &updated_status.sync_type;
    
    let mut response = serde_json::json!({
        "sync_type": new_sync_type,
        "gpodder_enabled": new_sync_type == "gpodder" || new_sync_type == "both",
        "external_enabled": new_sync_type == "external" || new_sync_type == "both", 
        "external_url": if new_sync_type == "external" || new_sync_type == "both" {
            updated_status.gpodder_url
        } else {
            None::<String>
        },
        "api_url": if new_sync_type == "gpodder" || new_sync_type == "both" {
            Some("http://localhost:8042")
        } else {
            None
        }
    });
    
    // Add device information if available (matches Python logic)
    if let Some(device_data) = device_info {
        if request.enabled {
            if let Some(device_name) = device_data.get("device_name") {
                response["device_name"] = device_name.clone();
            }
            if let Some(device_id) = device_data.get("device_id") {
                response["device_id"] = device_id.clone();
            }
        }
    }
    
    Ok(Json(response))
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

    // Direct HTTP call to match Python implementation exactly
    let client = reqwest::Client::new();
    let auth_url = format!("{}/api/2/auth/{}/login.json", 
                          gpodder_url.trim_end_matches('/'), 
                          gpodder_username);
    
    let verified = match client
        .post(&auth_url)
        .basic_auth(gpodder_username, Some(gpodder_password))
        .send()
        .await
    {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    };
    
    if verified {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Successfully connected to GPodder server and verified access.",
            "data": {
                "auth_type": "session",
                "has_devices": true
            }
        })))
    } else {
        Ok(Json(serde_json::json!({
            "success": false,
            "message": "Failed to connect to GPodder server",
            "data": null
        })))
    }
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
    
    Ok(Json(serde_json::json!(default_device)))
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
    
    // Get user's GPodder sync settings
    let settings = state.db_pool.get_user_sync_settings(user_id).await?
        .ok_or_else(|| AppError::BadRequest("User not found or GPodder sync not configured".to_string()))?;
    
    // Validate that GPodder sync is enabled
    if settings.sync_type != "gpodder" && settings.sync_type != "both" {
        return Err(AppError::BadRequest("GPodder sync is not enabled for this user".to_string()));
    }
    
    // Create device via GPodder API (uses proper auth for internal/external)
    let device_id = state.db_pool.create_device_via_gpodder_api(
        &settings.url,
        &settings.username, 
        &settings.token,
        &request.device_name
    ).await.map_err(|e| AppError::Internal(format!("Failed to create device via GPodder API: {}", e)))?;
    
    // Return GPodder API standard format
    Ok(Json(serde_json::json!({ 
        "id": device_id,  // GPodder device ID (string)
        "name": request.device_name,
        "type": request.device_type,
        "caption": request.device_caption.unwrap_or_else(|| request.device_name.clone()),
        "last_sync": Option::<String>::None,
        "is_active": true,
        "is_remote": true,
        "is_default": false
    })))
}

// GPodder Statistics - real server-side stats from GPodder API
#[derive(Serialize)]
pub struct GpodderStatistics {
    pub server_url: String,
    pub sync_type: String,
    pub sync_enabled: bool,
    pub server_devices: Vec<ServerDevice>,
    pub total_devices: i32,
    pub server_subscriptions: Vec<ServerSubscription>,
    pub total_subscriptions: i32,
    pub recent_episode_actions: Vec<ServerEpisodeAction>,
    pub total_episode_actions: i32,
    pub connection_status: String,
    pub last_sync_timestamp: Option<String>,
    pub api_endpoints_tested: Vec<EndpointTest>,
}

#[derive(Serialize, Clone)]
pub struct ServerDevice {
    pub id: String,
    pub caption: String,
    pub device_type: String,
    pub subscriptions: i32,
}

#[derive(Serialize, Clone)]
pub struct ServerSubscription {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ServerEpisodeAction {
    pub podcast: String,
    pub episode: String,
    pub action: String,
    pub timestamp: String,
    pub position: Option<i32>,
    pub device: Option<String>,
}

#[derive(Serialize)]
pub struct EndpointTest {
    pub endpoint: String,
    pub status: String, // "success", "failed", "not_tested"
    pub response_time_ms: Option<i64>,
    pub error: Option<String>,
}

pub async fn gpodder_get_statistics(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<GpodderStatistics>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Check if GPodder is enabled for this user
    let gpodder_status = state.db_pool.gpodder_get_status(user_id).await?;
    
    if gpodder_status.sync_type == "None" {
        return Ok(Json(GpodderStatistics {
            server_url: "No sync configured".to_string(),
            sync_type: "None".to_string(),
            sync_enabled: false,
            server_devices: vec![],
            total_devices: 0,
            server_subscriptions: vec![],
            total_subscriptions: 0,
            recent_episode_actions: vec![],
            total_episode_actions: 0,
            connection_status: "Not configured".to_string(),
            last_sync_timestamp: None,
            api_endpoints_tested: vec![],
        }));
    }

    // Get real statistics from GPodder server
    let statistics = state.db_pool.get_gpodder_server_statistics(user_id).await?;
    
    Ok(Json(statistics))
}