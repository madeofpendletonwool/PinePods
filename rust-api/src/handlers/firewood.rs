use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use chrono::Utc;
use reqwest;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, error, info, warn};
use futures;

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    models::{
        CreateFirewoodServerRequest, FirewoodServer, FirewoodServerStatusResponse,
        UpdateFirewoodServerRequest,
    },
    AppState,
};

// Legacy models for backward compatibility with existing frontend
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FirewoodPlayer {
    pub name: String,
    pub address: String,
    pub host: String,
    pub port: u16,
    pub version: Option<String>,
    pub server_url: Option<String>,
    pub status: PlayerStatus,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlayerStatus {
    Online,
    Offline,
    Discovering,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayEpisodeRequest {
    pub episode_id: Option<i64>,
    pub episode_url: String,
    pub episode_title: String,
    pub podcast_name: String,
    pub episode_duration: i64,
    pub episode_artwork: Option<String>,
    pub start_position: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FirewoodDiscoveryResponse {
    success: bool,
    data: PlayerInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayerInfo {
    name: String,
    version: String,
    server_url: Option<String>,
}

/// Get all Firewood servers for a user (replaces discovery)
pub async fn get_user_firewood_servers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<FirewoodServerStatusResponse>>> {
    let api_key = extract_api_key(&headers)?;
    
    if !validate_api_key(&state, &api_key).await? {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    info!("Getting Firewood servers for user {}", user_id);
    
    let servers = state.db_pool.get_user_firewood_servers(user_id).await?;
    let response_servers: Vec<FirewoodServerStatusResponse> = servers.into_iter().map(Into::into).collect();
    Ok(Json(response_servers))
}

/// Create a new Firewood server for a user
pub async fn create_firewood_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateFirewoodServerRequest>,
) -> AppResult<Json<FirewoodServerStatusResponse>> {
    let api_key = extract_api_key(&headers)?;
    
    if !validate_api_key(&state, &api_key).await? {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Validate the server address format
    if !request.server_address.starts_with("http://") && !request.server_address.starts_with("https://") {
        return Err(AppError::bad_request("Server address must start with http:// or https://"));
    }

    info!("Creating Firewood server '{}' at {} for user {}", request.server_name, request.server_address, user_id);

    let server_id = state.db_pool.create_firewood_server(user_id, &request).await?;
    
    let server = state.db_pool.get_firewood_server_by_id(server_id, user_id).await?
        .ok_or_else(|| AppError::internal("Failed to fetch created server"))?;

    // Check server status asynchronously (don't wait for it)
    let address = request.server_address.clone();
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = check_and_update_server_status(&state_clone, server_id, &address).await {
            warn!("Failed to check status for newly created server {}: {}", server_id, e);
        }
    });

    Ok(Json(server.into()))
}

/// Update a Firewood server
pub async fn update_firewood_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<i32>,
    Json(request): Json<UpdateFirewoodServerRequest>,
) -> AppResult<Json<FirewoodServerStatusResponse>> {
    let api_key = extract_api_key(&headers)?;
    
    if !validate_api_key(&state, &api_key).await? {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Verify the server belongs to this user
    let _server = state.db_pool.get_firewood_server_by_id(server_id, user_id).await?
        .ok_or_else(|| AppError::not_found("Firewood server not found"))?;

    // Validate server address if provided
    if let Some(address) = &request.server_address {
        if !address.starts_with("http://") && !address.starts_with("https://") {
            return Err(AppError::bad_request("Server address must start with http:// or https://"));
        }
    }

    state.db_pool.update_firewood_server(server_id, user_id, &request).await?;

    // Fetch the updated server
    let updated_server = state.db_pool.get_firewood_server_by_id(server_id, user_id).await?
        .ok_or_else(|| AppError::internal("Failed to fetch updated server"))?;

    Ok(Json(updated_server.into()))
}

/// Delete a Firewood server
pub async fn delete_firewood_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<i32>,
) -> AppResult<Json<String>> {
    let api_key = extract_api_key(&headers)?;
    
    if !validate_api_key(&state, &api_key).await? {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Verify the server belongs to this user and delete it
    let deleted = state.db_pool.delete_firewood_server(server_id, user_id).await?;
    
    if !deleted {
        return Err(AppError::not_found("Firewood server not found"));
    }

    info!("Deleted Firewood server {} for user {}", server_id, user_id);
    Ok(Json("Firewood server deleted successfully".to_string()))
}

/// Refresh status of all user's Firewood servers
pub async fn refresh_firewood_server_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> AppResult<Json<Vec<FirewoodServerStatusResponse>>> {
    let api_key = extract_api_key(&headers)?;
    
    if !validate_api_key(&state, &api_key).await? {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    let servers = state.db_pool.get_user_firewood_servers(user_id).await?;

    // Check status of all servers concurrently
    let mut tasks = Vec::new();
    for server in &servers {
        let state_clone = state.clone();
        let server_id = server.firewood_server_id;
        let address = server.server_address.clone();
        
        tasks.push(tokio::spawn(async move {
            check_and_update_server_status(&state_clone, server_id, &address).await
        }));
    }

    // Wait for all status checks to complete
    for task in tasks {
        if let Err(e) = task.await {
            warn!("Status check task failed: {}", e);
        }
    }

    // Fetch updated servers
    let updated_servers = state.db_pool.get_user_firewood_servers(user_id).await?;

    let response_servers: Vec<FirewoodServerStatusResponse> = updated_servers.into_iter().map(Into::into).collect();
    Ok(Json(response_servers))
}

/// Beam an episode to a specific Firewood server
pub async fn beam_episode_to_firewood_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(server_id): Path<i32>,
    Json(episode_request): Json<PlayEpisodeRequest>,
) -> AppResult<Json<String>> {
    let api_key = extract_api_key(&headers)?;
    
    if !validate_api_key(&state, &api_key).await? {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Get the server details
    let server = state.db_pool.get_firewood_server_by_id(server_id, user_id).await?
        .ok_or_else(|| AppError::not_found("Firewood server not found or inactive"))?;

    let url = format!("{}/play", server.server_address);
    
    let client = reqwest::Client::new();
    let response = timeout(
        Duration::from_secs(10),
        client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&episode_request)
            .send()
    ).await
    .map_err(|_| AppError::internal("Request timeout when contacting Firewood player"))?
    .map_err(|e| AppError::internal(&format!("Failed to contact Firewood player: {}", e)))?;
    
    if response.status().is_success() {
        info!("Successfully sent episode '{}' to Firewood server '{}' at {}", 
              episode_request.episode_title, server.server_name, server.server_address);
        Ok(Json("Episode sent to Firewood player successfully".to_string()))
    } else {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        error!("Failed to send episode to Firewood server '{}': {}", server.server_name, error_text);
        Err(AppError::internal(&format!("Firewood player rejected request: {}", error_text)))
    }
}

/// Check and update the status of a Firewood server in the database
async fn check_and_update_server_status(
    state: &AppState,
    server_id: i32,
    address: &str,
) -> Result<(), AppError> {
    let status = if let Some(_player_info) = check_firewood_endpoint(address).await {
        "online"
    } else {
        "offline"
    };

    state.db_pool.update_firewood_server_status(server_id, status).await?;

    debug!("Updated server {} status to: {}", server_id, status);
    Ok(())
}

/// Check if an endpoint is a Firewood player
async fn check_firewood_endpoint(address: &str) -> Option<FirewoodPlayer> {
    let url = format!("{}/", address);
    
    debug!("Checking potential Firewood endpoint: {}", url);
    
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2)) // Short timeout for status check
        .build()
        .ok()?;
    
    let response = timeout(Duration::from_millis(1500), client.get(&url).send())
        .await
        .ok()?
        .ok()?
        .error_for_status()
        .ok()?;
    
    let discovery_response: FirewoodDiscoveryResponse = response.json().await.ok()?;
    
    if !discovery_response.success {
        return None;
    }
    
    let url_parts = address.replace("http://", "").replace("https://", "");
    let (host, port_str) = if let Some(pos) = url_parts.find(':') {
        (&url_parts[..pos], &url_parts[pos+1..])
    } else {
        (url_parts.as_str(), "80")
    };
    
    let port = port_str.parse::<u16>().unwrap_or(80);
    
    debug!("Found Firewood player: {} at {}", discovery_response.data.name, address);
    
    Some(FirewoodPlayer {
        name: discovery_response.data.name,
        address: address.to_string(),
        host: host.to_string(),
        port,
        version: Some(discovery_response.data.version),
        server_url: discovery_response.data.server_url,
        status: PlayerStatus::Online,
        last_seen: chrono::Utc::now().to_rfc3339(),
    })
}

/// Background task to check all Firewood server statuses (called every 30 minutes)
pub async fn background_check_all_firewood_servers(state: &AppState) -> Result<(), AppError> {
    info!("Starting background check of all Firewood servers");
    
    let servers = state.db_pool.get_all_active_firewood_servers().await?;

    let mut tasks = Vec::new();
    for server in servers {
        let state_clone = state.clone();
        let server_id = server.firewood_server_id;
        let address = server.server_address.clone();
        
        tasks.push(tokio::spawn(async move {
            if let Err(e) = check_and_update_server_status(&state_clone, server_id, &address).await {
                warn!("Failed to check status for server {}: {}", server_id, e);
            }
        }));
    }

    // Wait for all status checks to complete (with reasonable timeout)
    let results = timeout(Duration::from_secs(60), futures::future::join_all(tasks)).await;
    
    match results {
        Ok(_) => info!("Background Firewood server status check completed"),
        Err(_) => warn!("Background Firewood server status check timed out after 60 seconds"),
    }

    Ok(())
}