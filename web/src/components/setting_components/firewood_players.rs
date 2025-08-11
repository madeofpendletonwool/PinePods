// src/components/setting_components/firewood_players.rs

use crate::components::context::AppState;
use yewdux::prelude::Dispatch;
use crate::components::gen_funcs::format_error_message;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::prelude::*;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;
use wasm_bindgen_futures::spawn_local;
use urlencoding;
// chrono removed - using String for timestamps for simpler serialization

// New database-backed Firewood server models
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FirewoodServer {
    pub firewood_server_id: i32,
    pub server_name: String,
    pub server_address: String,
    pub server_status: String,
    pub last_checked: String, // Changed to String for simpler serialization
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFirewoodServerRequest {
    pub server_name: String,
    pub server_address: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFirewoodServerRequest {
    pub server_name: Option<String>,
    pub server_address: Option<String>,
    pub is_active: Option<bool>,
}

// Legacy models for backward compatibility (if needed elsewhere)
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FirewoodPlaybackStatus {
    pub is_playing: bool,
    pub current_episode: Option<FirewoodCurrentEpisode>,
    pub position: i64,
    pub duration: i64,
    pub volume: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FirewoodCurrentEpisode {
    pub episode_id: Option<i64>,
    pub episode_title: String,
    pub podcast_name: String,
    pub episode_artwork: Option<String>,
    pub duration: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewoodPlayerInfo {
    pub name: String,
    pub version: String,
    pub server_url: String,
    pub user_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewoodApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub message: Option<String>,
}

// Firewood remote control request types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkipRequest {
    pub seconds: i64, // positive = forward, negative = backward
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeekRequest {
    pub position: i64, // seconds from start
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeRequest {
    pub volume: f32, // 0.0 to 1.0
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

// New database-backed API functions

// Get all user's Firewood servers from database
async fn get_firewood_servers(api_key: &Option<String>, server_name: &String) -> Result<Vec<FirewoodServer>, gloo_net::Error> {
    let url = format!("{}/api/firewood/servers", server_name);
    
    let response = Request::get(&url)
        .header("Api-Key", api_key.as_ref().ok_or_else(|| gloo_net::Error::GlooError("API key not available".to_string()))?)
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to get Firewood servers", response.status())));
    }
    
    let servers: Vec<FirewoodServer> = response.json().await?;
    Ok(servers)
}

// Create a new Firewood server
async fn create_firewood_server(api_key: &Option<String>, server_name: &String, request: CreateFirewoodServerRequest) -> Result<FirewoodServer, gloo_net::Error> {
    let url = format!("{}/api/firewood/servers", server_name);
    
    let response = Request::post(&url)
        .header("Api-Key", api_key.as_ref().ok_or_else(|| gloo_net::Error::GlooError("API key not available".to_string()))?)
        .header("Content-Type", "application/json")
        .json(&request)?
        .send()
        .await?;
        
    if !response.ok() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: {}", response.status(), error_text)));
    }
    
    let server: FirewoodServer = response.json().await?;
    Ok(server)
}

// Refresh status of all user's Firewood servers (for real-time updates on settings page)
async fn refresh_firewood_servers_status(api_key: &Option<String>, server_name: &String) -> Result<Vec<FirewoodServer>, gloo_net::Error> {
    let url = format!("{}/api/firewood/servers/refresh", server_name);
    
    let response = Request::post(&url)
        .header("Api-Key", api_key.as_ref().ok_or_else(|| gloo_net::Error::GlooError("API key not available".to_string()))?)
        .header("Content-Type", "application/json")
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to refresh Firewood servers", response.status())));
    }
    
    let servers: Vec<FirewoodServer> = response.json().await?;
    Ok(servers)
}

// Delete a Firewood server
async fn delete_firewood_server(api_key: &Option<String>, server_name: &String, server_id: i32) -> Result<String, gloo_net::Error> {
    let url = format!("{}/api/firewood/servers/{}", server_name, server_id);
    
    let response = Request::delete(&url)
        .header("Api-Key", api_key.as_ref().ok_or_else(|| gloo_net::Error::GlooError("API key not available".to_string()))?)
        .send()
        .await?;
        
    if !response.ok() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: {}", response.status(), error_text)));
    }
    
    let result: String = response.json().await?;
    Ok(result)
}

// Send episode to Firewood player via backend (updated for database-backed approach)
pub async fn play_episode_on_firewood(
    api_key: &Option<String>,
    server_name: &String,
    server_id: i32,
    episode_request: &PlayEpisodeRequest,
) -> Result<String, Box<dyn std::error::Error>> {
    let url = format!("{}/api/firewood/servers/{}/beam", server_name, server_id);
    
    let response = Request::post(&url)
        .header("Api-Key", api_key.as_ref().ok_or_else(|| gloo_net::Error::GlooError("API key not available".to_string()))?)
        .header("Content-Type", "application/json")
        .json(episode_request)?
        .send()
        .await?;
    
    if response.ok() {
        let result: String = response.json().await?;
        Ok(result)
    } else {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        Err(format!("Failed to play episode: {}", error_text).into())
    }
}

// Load Firewood servers into global state - for app-wide access
pub async fn load_firewood_servers_into_state(
    api_key: &Option<String>, 
    server_name: &String,
    dispatch: &Dispatch<AppState>
) {
    match get_firewood_servers(api_key, server_name).await {
        Ok(servers) => {
            dispatch.reduce_mut(|state| {
                state.firewood_servers = Some(servers);
            });
        }
        Err(_) => {
            // Silent failure - don't show error for background loading
            dispatch.reduce_mut(|state| {
                state.firewood_servers = Some(Vec::new());
            });
        }
    }
}

// FIREWOOD DIRECT HTTP API FUNCTIONS (for real-time status and control)

// Get Firewood player info directly from Firewood server
pub async fn get_firewood_player_info(firewood_url: &str) -> Result<FirewoodPlayerInfo, gloo_net::Error> {
    let response = Request::get(firewood_url)
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to get player info", response.status())));
    }
    
    let api_response: FirewoodApiResponse<FirewoodPlayerInfo> = response.json().await?;
    if api_response.success {
        api_response.data.ok_or_else(|| gloo_net::Error::GlooError("No data in response".to_string()))
    } else {
        Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())))
    }
}

// Get Firewood playback status directly from Firewood server
pub async fn get_firewood_playback_status(firewood_url: &str) -> Result<FirewoodPlaybackStatus, gloo_net::Error> {
    let url = format!("{}/status", firewood_url);
    let response = Request::get(&url)
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to get playback status", response.status())));
    }
    
    let api_response: FirewoodApiResponse<FirewoodPlaybackStatus> = response.json().await?;
    if api_response.success {
        api_response.data.ok_or_else(|| gloo_net::Error::GlooError("No data in response".to_string()))
    } else {
        Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())))
    }
}

// Pause Firewood playback
pub async fn pause_firewood_playback(firewood_url: &str) -> Result<(), gloo_net::Error> {
    let url = format!("{}/pause", firewood_url);
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to pause playback", response.status())));
    }
    
    let api_response: FirewoodApiResponse<()> = response.json().await?;
    if !api_response.success {
        return Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())));
    }
    
    Ok(())
}

// Resume Firewood playback
pub async fn resume_firewood_playback(firewood_url: &str) -> Result<(), gloo_net::Error> {
    let url = format!("{}/resume", firewood_url);
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to resume playback", response.status())));
    }
    
    let api_response: FirewoodApiResponse<()> = response.json().await?;
    if !api_response.success {
        return Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())));
    }
    
    Ok(())
}

// Stop Firewood playback
pub async fn stop_firewood_playback(firewood_url: &str) -> Result<(), gloo_net::Error> {
    let url = format!("{}/stop", firewood_url);
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to stop playback", response.status())));
    }
    
    let api_response: FirewoodApiResponse<()> = response.json().await?;
    if !api_response.success {
        return Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())));
    }
    
    Ok(())
}

// Skip seconds in Firewood playback
pub async fn skip_firewood_playback(firewood_url: &str, seconds: i64) -> Result<(), gloo_net::Error> {
    let url = format!("{}/skip", firewood_url);
    let request = SkipRequest { seconds };
    
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&request)?
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to skip playback", response.status())));
    }
    
    let api_response: FirewoodApiResponse<()> = response.json().await?;
    if !api_response.success {
        return Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())));
    }
    
    Ok(())
}

// Seek to position in Firewood playback
pub async fn seek_firewood_playback(firewood_url: &str, position: i64) -> Result<(), gloo_net::Error> {
    let url = format!("{}/seek", firewood_url);
    let request = SeekRequest { position };
    
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&request)?
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to seek playback", response.status())));
    }
    
    let api_response: FirewoodApiResponse<()> = response.json().await?;
    if !api_response.success {
        return Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())));
    }
    
    Ok(())
}

// Set Firewood volume
pub async fn set_firewood_volume(firewood_url: &str, volume: f32) -> Result<(), gloo_net::Error> {
    let url = format!("{}/volume", firewood_url);
    let request = VolumeRequest { volume };
    
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&request)?
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to set volume", response.status())));
    }
    
    let api_response: FirewoodApiResponse<()> = response.json().await?;
    if !api_response.success {
        return Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())));
    }
    
    Ok(())
}

// Play episode directly on Firewood server (direct HTTP API call)
pub async fn play_episode_on_firewood_direct(firewood_url: &str, episode_request: &PlayEpisodeRequest) -> Result<(), gloo_net::Error> {
    let url = format!("{}/play", firewood_url);
    
    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(episode_request)?
        .send()
        .await?;
        
    if !response.ok() {
        return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to play episode", response.status())));
    }
    
    let api_response: FirewoodApiResponse<()> = response.json().await?;
    if !api_response.success {
        return Err(gloo_net::Error::GlooError(api_response.message.unwrap_or("API call failed".to_string())));
    }
    
    Ok(())
}

// FIREWOOD STATUS MANAGEMENT FUNCTIONS

// Poll all Firewood servers and update global status
pub async fn poll_firewood_status_for_all_servers(
    servers: &[FirewoodServer],
    dispatch: &Dispatch<AppState>
) {
    let mut status_map = HashMap::new();
    
    for server in servers.iter() {
        if server.server_status == "online" && server.is_active {
            match get_firewood_playback_status(&server.server_address).await {
                Ok(status) => {
                    status_map.insert(server.firewood_server_id, (server.server_address.clone(), status));
                }
                Err(_) => {
                    // Server is not responding, skip it
                }
            }
        }
    }
    
    dispatch.reduce_mut(|state| {
        state.firewood_status = Some(status_map);
    });
}

// Poll specific Firewood server and update global status
pub async fn poll_firewood_status_for_server(
    server_id: i32,
    server_address: &str,
    dispatch: &Dispatch<AppState>
) -> Result<(), gloo_net::Error> {
    let status = get_firewood_playback_status(server_address).await?;
    
    dispatch.reduce_mut(|state| {
        if let Some(ref mut status_map) = state.firewood_status {
            status_map.insert(server_id, (server_address.to_string(), status));
        } else {
            let mut new_map = HashMap::new();
            new_map.insert(server_id, (server_address.to_string(), status));
            state.firewood_status = Some(new_map);
        }
    });
    
    Ok(())
}

// Set active Firewood server (the one currently being controlled)
pub fn set_active_firewood_server(dispatch: &Dispatch<AppState>, server_id: Option<i32>) {
    dispatch.reduce_mut(|state| {
        state.active_firewood_server = server_id;
    });
}

// Get Firewood server by ID from global state
pub fn get_firewood_server_by_id(servers: &[FirewoodServer], server_id: i32) -> Option<&FirewoodServer> {
    servers.iter().find(|s| s.firewood_server_id == server_id)
}

// Sync Firewood playback progress to PinePods server
pub async fn sync_firewood_progress_to_pinepods(
    api_key: &Option<String>,
    server_name: &str,
    episode_id: Option<i64>,
    position_seconds: i64,
) -> Result<(), gloo_net::Error> {
    if let Some(ep_id) = episode_id {
        // Use the existing PinePods API to record listen duration
        let url = format!("{}/api/data/record_listen_duration", server_name);
        
        let request_body = serde_json::json!({
            "episode_id": ep_id,
            "listen_duration": position_seconds
        });
        
        let response = Request::post(&url)
            .header("Api-Key", api_key.as_ref().ok_or_else(|| gloo_net::Error::GlooError("API key not available".to_string()))?)
            .header("Content-Type", "application/json")
            .body(request_body.to_string())?
            .send()
            .await?;
            
        if !response.ok() {
            return Err(gloo_net::Error::GlooError(format!("HTTP {}: Failed to sync progress", response.status())));
        }
    }
    
    Ok(())
}

// Start background polling for all active Firewood servers
pub fn start_background_firewood_polling(dispatch: Dispatch<AppState>) {
    spawn_local(async move {
        // Poll every 30 seconds for all servers (background monitoring)
        loop {
            TimeoutFuture::new(30_000).await;
            
            let state = dispatch.get();
            if let Some(servers) = &state.firewood_servers {
                let active_servers: Vec<_> = servers.iter()
                    .filter(|s| s.server_status == "online" && s.is_active)
                    .cloned()
                    .collect();
                
                for server in active_servers {
                    let _ = poll_firewood_status_for_server(
                        server.firewood_server_id,
                        &server.server_address,
                        &dispatch
                    ).await;
                }
            }
        }
    });
}

// Switch active Firewood server (for multi-device control)
pub async fn switch_active_firewood_server(
    dispatch: &Dispatch<AppState>,
    new_server_id: i32
) -> Result<(), String> {
    let state = dispatch.get();
    
    // Check if the server exists and is online
    if let Some(servers) = &state.firewood_servers {
        if let Some(server) = get_firewood_server_by_id(servers, new_server_id) {
            if server.server_status == "online" && server.is_active {
                // Stop any current server and set the new active server
                set_active_firewood_server(dispatch, Some(new_server_id));
                
                // Start intensive polling for the new active server
                start_firewood_status_polling(
                    new_server_id,
                    server.server_address.clone(),
                    dispatch.clone()
                );
                
                Ok(())
            } else {
                Err("Server is not online or not active".to_string())
            }
        } else {
            Err("Server not found".to_string())
        }
    } else {
        Err("No servers available".to_string())
    }
}

// Stop and clear active Firewood server
pub async fn stop_active_firewood_server(dispatch: &Dispatch<AppState>) -> Result<(), gloo_net::Error> {
    let state = dispatch.get();
    
    if let (Some(active_server_id), Some(status_map)) = (&state.active_firewood_server, &state.firewood_status) {
        if let Some((server_address, _)) = status_map.get(active_server_id) {
            // Stop playback on the active server
            stop_firewood_playback(server_address).await?;
        }
    }
    
    // Clear active server
    set_active_firewood_server(dispatch, None);
    
    Ok(())
}

// Get all Firewood servers with their current status
pub fn get_all_firewood_servers_with_status(state: &AppState) -> Vec<(FirewoodServer, Option<FirewoodPlaybackStatus>)> {
    let mut servers_with_status = Vec::new();
    
    if let Some(servers) = &state.firewood_servers {
        for server in servers.iter() {
            let status = if let Some(status_map) = &state.firewood_status {
                status_map.get(&server.firewood_server_id).map(|(_, status)| status.clone())
            } else {
                None
            };
            
            servers_with_status.push((server.clone(), status));
        }
    }
    
    servers_with_status
}

// Start continuous status polling for a specific server (call this after beaming an episode)
pub fn start_firewood_status_polling(
    server_id: i32,
    server_address: String,
    dispatch: Dispatch<AppState>
) {
    spawn_local(async move {
        // Poll every 3 seconds for active server status
        let mut interval_count = 0;
        let mut last_synced_position = 0i64;
        
        loop {
            match poll_firewood_status_for_server(server_id, &server_address, &dispatch).await {
                Ok(_) => {
                    // Successfully polled status - now check if we need to sync progress
                    if let Some(status_map) = dispatch.get().firewood_status.as_ref() {
                        if let Some((_, status)) = status_map.get(&server_id) {
                            // Sync progress to PinePods every 30 seconds or when position changes significantly
                            let position_diff = (status.position - last_synced_position).abs();
                            if interval_count % 10 == 0 || position_diff > 30 { // Every 30 seconds or 30+ second jump
                                if let Some(episode) = &status.current_episode {
                                    // Get auth details from state to sync progress
                                    let state = dispatch.get();
                                    if let Some(auth_details) = &state.auth_details {
                                        let api_key = &auth_details.api_key;
                                        let server_name = &auth_details.server_name;
                                        
                                        match sync_firewood_progress_to_pinepods(
                                            api_key,
                                            server_name,
                                            episode.episode_id,
                                            status.position
                                        ).await {
                                            Ok(_) => {
                                                last_synced_position = status.position;
                                                web_sys::console::log_1(&format!(
                                                    "Synced Firewood progress: {} seconds for episode {:?}", 
                                                    status.position, episode.episode_id
                                                ).into());
                                            }
                                            Err(e) => {
                                                web_sys::console::warn_1(&format!(
                                                    "Failed to sync Firewood progress: {:?}", e
                                                ).into());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    interval_count += 1;
                    
                    // After 60 seconds (20 intervals), reduce polling frequency to every 10 seconds
                    if interval_count > 20 {
                        TimeoutFuture::new(10_000).await;
                    } else {
                        TimeoutFuture::new(3_000).await;
                    }
                }
                Err(_) => {
                    // Error polling, try again in 5 seconds
                    TimeoutFuture::new(5_000).await;
                    
                    // If we've had errors for too long, stop polling
                    interval_count += 1;
                    if interval_count > 100 { // Stop after ~5 minutes of errors
                        break;
                    }
                }
            }
        }
    });
}

#[function_component(FirewoodPlayers)]
pub fn firewood_players() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let post_state = state.clone(); // Get reference for API calls
    
    // Use database-backed servers instead of localStorage players
    let servers = use_state(|| Vec::<FirewoodServer>::new());
    let is_refreshing = use_state(|| false);
    let server_name = use_state(|| String::new());
    let server_address = use_state(|| String::new());
    let show_success = use_state(|| false);
    let success_message = use_state(|| String::new());
    
    // Load servers from database on component mount
    {
        let servers = servers.clone();
        let post_state = post_state.clone();
        
        use_effect_with((), move |_| {
            let servers = servers.clone();
            
            spawn_local(async move {
                // Get API details for backend call
                if let Some(auth_details) = &post_state.auth_details {
                    // Load existing servers from database
                    match get_firewood_servers(&auth_details.api_key, &auth_details.server_name).await {
                        Ok(server_list) => {
                            servers.set(server_list);
                        }
                        Err(_) => {
                            // Silent failure for loading servers
                        }
                    }
                }
            });
            || ()
        });
    }

    // Auto-refresh server status every 10 seconds when on this page
    {
        let servers = servers.clone();
        let is_refreshing = is_refreshing.clone();
        let post_state = post_state.clone();
        
        use_effect_with(servers.clone(), move |_| {
            let servers = servers.clone();
            let is_refreshing = is_refreshing.clone();
            let post_state = post_state.clone();
            
            let interval = gloo_timers::callback::Interval::new(10_000, move || {
                if *is_refreshing {
                    return; // Don't refresh if already in progress
                }
                
                let servers = servers.clone();
                let is_refreshing = is_refreshing.clone();
                let post_state = post_state.clone();
                
                spawn_local(async move {
                    if let Some(auth_details) = &post_state.auth_details {
                        is_refreshing.set(true);
                        
                        match refresh_firewood_servers_status(&auth_details.api_key, &auth_details.server_name).await {
                            Ok(updated_servers) => {
                                servers.set(updated_servers);
                            }
                            Err(_) => {
                                // Silent failure for auto-refresh
                            }
                        }
                        
                        is_refreshing.set(false);
                    }
                });
            });
            
            move || {
                interval.cancel();
            }
        });
    }
    
    // Manual refresh of server status
    let on_refresh_click = {
        let servers = servers.clone();
        let is_refreshing = is_refreshing.clone();
        let post_state = post_state.clone();
        let dispatch = dispatch.clone();
        
        Callback::from(move |_: MouseEvent| {
            let servers = servers.clone();
            let is_refreshing = is_refreshing.clone();
            let post_state = post_state.clone();
            let dispatch = dispatch.clone();
            
            spawn_local(async move {
                if let Some(auth_details) = &post_state.auth_details {
                    is_refreshing.set(true);
                    
                    match refresh_firewood_servers_status(&auth_details.api_key, &auth_details.server_name).await {
                        Ok(updated_servers) => {
                            servers.set(updated_servers);
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to refresh server status: {:?}",
                                    e
                                ));
                            });
                        }
                    }
                    
                    is_refreshing.set(false);
                }
            });
        })
    };
    
    // Server input handlers
    let on_server_name_change = {
        let server_name = server_name.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            server_name.set(target.value());
        })
    };
    
    let on_server_address_change = {
        let server_address = server_address.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            server_address.set(target.value());
        })
    };
    
    // Add server
    let on_add_server = {
        let server_name = server_name.clone();
        let server_address = server_address.clone();
        let servers = servers.clone();
        let show_success = show_success.clone();
        let success_message = success_message.clone();
        let dispatch = dispatch.clone();
        let post_state = post_state.clone();

        Callback::from(move |_: MouseEvent| {
            let name = (*server_name).clone();
            let address = (*server_address).clone();
            let post_call_state = post_state.clone();
            
            if name.is_empty() {
                dispatch.reduce_mut(|state| {
                    state.error_message = Some("Please enter a server name".to_string());
                });
                return;
            }
            
            if address.is_empty() {
                dispatch.reduce_mut(|state| {
                    state.error_message = Some("Please enter a server address".to_string());
                });
                return;
            }
            
            if !address.starts_with("http://") && !address.starts_with("https://") {
                dispatch.reduce_mut(|state| {
                    state.error_message = Some("Server address must start with http:// or https://".to_string());
                });
                return;
            }
            
            let servers = servers.clone();
            let show_success = show_success.clone();
            let success_message = success_message.clone();
            let dispatch = dispatch.clone();
            let server_name = server_name.clone();
            let server_address = server_address.clone();
            
            spawn_local(async move {
                // Get API details for backend call
                if let Some(auth_details) = &post_call_state.auth_details {
                    let request = CreateFirewoodServerRequest {
                        server_name: name,
                        server_address: address,
                    };
                    
                    match create_firewood_server(&auth_details.api_key, &auth_details.server_name, request).await {
                        Ok(new_server) => {
                            // Add to current servers list
                            let mut current_servers = (*servers).clone();
                            current_servers.push(new_server);
                            servers.set(current_servers);
                            
                            show_success.set(true);
                            success_message.set("Firewood server added successfully".to_string());
                            server_name.set(String::new());
                            server_address.set(String::new());
                            
                            // Auto-hide success message
                            let show_success_clone = show_success.clone();
                            gloo_timers::callback::Timeout::new(3000, move || {
                                show_success_clone.set(false);
                            }).forget();
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to add Firewood server: {}",
                                    e
                                ));
                            });
                        }
                    }
                } else {
                    dispatch.reduce_mut(|state| {
                        state.error_message = Some("Authentication not available".to_string());
                    });
                }
            });
        })
    };
    
    // Remove server
    let on_remove_server = {
        let servers = servers.clone();
        let dispatch = dispatch.clone();
        let post_state = post_state.clone();
        
        Callback::from(move |server_id: i32| {
            let servers = servers.clone();
            let dispatch = dispatch.clone();
            let post_state = post_state.clone();
            
            spawn_local(async move {
                if let Some(auth_details) = &post_state.auth_details {
                    match delete_firewood_server(&auth_details.api_key, &auth_details.server_name, server_id).await {
                        Ok(_) => {
                            // Remove from current servers list
                            let mut current_servers = (*servers).clone();
                            current_servers.retain(|s| s.firewood_server_id != server_id);
                            servers.set(current_servers);
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to remove Firewood server: {}",
                                    e
                                ));
                            });
                        }
                    }
                }
            });
        })
    };
    
    html! {
        <div class="firewood-players-container">
            <div class="settings-description mb-4">
                <p class="item_container-text">
                    {"Manage your Firewood remote players. Add servers that can receive and play episodes remotely."}
                </p>
            </div>
            
            // Server Management Controls
            <div class="server-controls mb-6">
                <div class="flex items-center space-x-4 mb-4">
                    <button
                        class="firewood-refresh-button"
                        onclick={on_refresh_click}
                        disabled={*is_refreshing}
                    >
                        if *is_refreshing {
                            <i class="ph ph-spinner ph-spin mr-2"></i>
                            {"Refreshing Status..."}
                        } else {
                            <i class="ph ph-arrow-clockwise mr-2"></i>
                            {"Refresh Status"}
                        }
                    </button>
                </div>
                
                // Add Server Section
                <div class="manual-add-section firewood-manual-section p-4 rounded-lg mb-4">
                    <h4 class="item_container-text font-medium mb-3">{"Add Firewood Server"}</h4>
                    <div class="space-y-3">
                        <input
                            type="text"
                            placeholder="My Firewood Player"
                            value={(*server_name).clone()}
                            oninput={on_server_name_change}
                            class="form-input w-full"
                        />
                        <input
                            type="text"
                            placeholder="http://192.168.1.100:8042"
                            value={(*server_address).clone()}
                            oninput={on_server_address_change}
                            class="form-input w-full"
                        />
                        <button
                            class="firewood-add-button"
                            onclick={on_add_server}
                        >
                            <i class="ph ph-plus mr-1"></i>
                            {"Add Server"}
                        </button>
                    </div>
                    <p class="text-xs firewood-help-text mt-2">
                        {"Enter a name and the full HTTP address of your Firewood server (e.g., http://192.168.1.100:8042)"}
                    </p>
                </div>
            </div>
            
            // Servers List
            <div class="servers-list">
                <h4 class="item_container-text font-medium mb-3">
                    {"Firewood Servers"} 
                    <span class="firewood-server-count ml-2">
                        {format!("({})", servers.len())}
                    </span>
                </h4>
                
                if servers.is_empty() {
                    <div class="no-servers-message firewood-empty-state p-6 text-center rounded-lg">
                        <i class="ph ph-broadcast text-4xl firewood-empty-icon mb-3"></i>
                        <p class="item_container-text mb-2">{"No Firewood servers configured"}</p>
                        <p class="firewood-help-text">
                            {"Add your first Firewood server using the form above."}
                        </p>
                    </div>
                } else {
                    <div class="firewood-servers-grid">
                        { for servers.iter().map(|server| {
                            let server_id = server.firewood_server_id;
                            let remove_callback = {
                                let on_remove = on_remove_server.clone();
                                Callback::from(move |_: MouseEvent| {
                                    on_remove.emit(server_id);
                                })
                            };
                            
                            html! {
                                <div key={server.firewood_server_id.to_string()} class="firewood-server-card p-4 rounded-lg border">
                                    <div class="flex items-start justify-between">
                                        <div class="flex-1">
                                            <div class="flex items-center space-x-2 mb-2">
                                                <i class="ph ph-desktop firewood-server-icon"></i>
                                                <h5 class="item_container-text font-medium">{&server.server_name}</h5>
                                                <span class={format!("firewood-status-badge status-{}", 
                                                    match server.server_status.as_str() {
                                                        "online" => "online",
                                                        "offline" => "offline",
                                                        _ => "unknown",
                                                    })}>
                                                    {match server.server_status.as_str() {
                                                        "online" => "Online",
                                                        "offline" => "Offline", 
                                                        _ => "Unknown",
                                                    }}
                                                </span>
                                            </div>
                                            <div class="firewood-server-details space-y-1">
                                                <div class="firewood-detail-item">
                                                    <span class="firewood-detail-label">{"Address:"}</span>
                                                    <span class="firewood-detail-value">{&server.server_address}</span>
                                                </div>
                                                <div class="firewood-detail-item">
                                                    <span class="firewood-detail-label">{"Last Checked:"}</span>
                                                    <span class="firewood-detail-value">{&server.last_checked}</span>
                                                </div>
                                                <div class="firewood-detail-item">
                                                    <span class="firewood-detail-label">{"Active:"}</span>
                                                    <span class="firewood-detail-value">{if server.is_active { "Yes" } else { "No" }}</span>
                                                </div>
                                            </div>
                                        </div>
                                        <button
                                            class="firewood-remove-button"
                                            onclick={remove_callback}
                                            title="Remove server"
                                        >
                                            <i class="ph ph-trash"></i>
                                        </button>
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                }
            </div>
            
            if *show_success {
                <div class="success-message mt-4">
                    {(*success_message).clone()}
                </div>
            }
        </div>
    }
}