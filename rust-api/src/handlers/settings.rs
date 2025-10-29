use axum::{
    extract::{Path, Query, State, Multipart, Json},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key, check_user_access},
    models::{AvailableLanguage, LanguageUpdateRequest, UserLanguageResponse, AvailableLanguagesResponse},
    AppState,
};
use sqlx::{Row, ValueRef};

// Request struct for set_theme
#[derive(Deserialize)]
pub struct SetThemeRequest {
    pub user_id: i32,
    pub new_theme: String,
}

// Request struct for set_playback_speed - matches Python SetPlaybackSpeedUser model exactly
#[derive(Deserialize)]
pub struct SetPlaybackSpeedUser {
    pub user_id: i32,
    pub playback_speed: f64,
}

// Set user theme - matches Python api_set_theme function exactly
pub async fn set_theme(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetThemeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only set their own theme
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only set your own theme!"));
    }

    state.db_pool.set_theme(request.user_id, &request.new_theme).await?;

    Ok(Json(serde_json::json!({ "message": "Theme updated successfully" })))
}

// Set user playback speed - matches Python api_set_playback_speed_user function exactly
pub async fn set_playback_speed_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetPlaybackSpeedUser>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only set their own playback speed
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own settings."));
    }

    state.db_pool.set_playback_speed_user(request.user_id, request.playback_speed).await?;

    Ok(Json(serde_json::json!({ "detail": "Default playback speed updated." })))
}

// User info response struct
#[derive(Serialize)]
pub struct UserInfo {
    pub userid: i32,
    pub fullname: String,
    pub username: String,
    pub email: String,
    #[serde(serialize_with = "bool_to_int")]
    pub isadmin: bool,
}

// Helper function to serialize boolean as integer for Python compatibility
fn bool_to_int<S>(value: &bool, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_i32(if *value { 1 } else { 0 })
}

// Get all users info - matches Python api_get_user_info function exactly (admin only)
pub async fn get_user_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserInfo>>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    let user_info = state.db_pool.get_user_info().await?;
    Ok(Json(user_info))
}

// Get specific user info - matches Python api_get_my_user_info function exactly
pub async fn get_my_user_info(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only get their own info
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only retrieve your own user information!"));
    }

    let user_info = state.db_pool.get_my_user_info(user_id).await?;
    match user_info {
        Some(info) => Ok(Json(info)),
        None => Err(AppError::not_found("User not found")),
    }
}

// Request struct for add_user
#[derive(Deserialize)]
pub struct AddUserRequest {
    pub fullname: String,
    pub username: String,
    pub email: String,
    pub hash_pw: String,
}

// Add user - matches Python api_add_user function exactly (admin only)
pub async fn add_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(user_values): Json<AddUserRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    match state.db_pool.add_user(&user_values.fullname, &user_values.username.to_lowercase(), &user_values.email, &user_values.hash_pw).await {
        Ok(user_id) => Ok(Json(serde_json::json!({ "detail": "Success", "user_id": user_id }))),
        Err(e) => {
            let error_msg = format!("{}", e);
            if error_msg.contains("username") && error_msg.contains("duplicate") {
                Err(AppError::Conflict("This username is already taken. Please choose a different username.".to_string()))
            } else if error_msg.contains("email") && error_msg.contains("duplicate") {
                Err(AppError::Conflict("This email is already in use. Please use a different email address.".to_string()))
            } else {
                Err(AppError::internal("Failed to create user"))
            }
        }
    }
}

// Add login user - matches Python api_add_user (add_login_user endpoint) function exactly (self-service)
pub async fn add_login_user(
    State(state): State<AppState>,
    Json(user_values): Json<AddUserRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Check if self-service user registration is enabled (matches Python check_self_service)
    let self_service_status = state.db_pool.self_service_status().await?;
    
    if !self_service_status.status {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }

    match state.db_pool.add_user(&user_values.fullname, &user_values.username.to_lowercase(), &user_values.email, &user_values.hash_pw).await {
        Ok(user_id) => Ok(Json(serde_json::json!({ "detail": "User added successfully", "user_id": user_id }))),
        Err(e) => {
            let error_msg = format!("{}", e);
            if error_msg.contains("username") && error_msg.contains("duplicate") {
                Err(AppError::Conflict("This username is already taken. Please choose a different username.".to_string()))
            } else if error_msg.contains("email") && error_msg.contains("duplicate") {
                Err(AppError::Conflict("This email address is already registered. Please use a different email.".to_string()))
            } else {
                Err(AppError::internal("An unexpected error occurred while creating the user"))
            }
        }
    }
}

// Set fullname - matches Python api_set_fullname function exactly
pub async fn set_fullname(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Query(params): Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let new_name = params.get("new_name")
        .ok_or_else(|| AppError::bad_request("Missing new_name parameter"))?;

    // Check authorization - admins can edit other users, users can edit themselves
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id_from_api_key).await?;

    if user_id != user_id_from_api_key && !is_admin {
        return Err(AppError::forbidden("You can only update your own full name"));
    }

    state.db_pool.set_fullname(user_id, new_name).await?;
    Ok(Json(serde_json::json!({ "detail": "Fullname updated." })))
}

// Request struct for set_password
#[derive(Deserialize)]
pub struct PasswordUpdateRequest {
    pub hash_pw: String,
}

// Set password - matches Python api_set_password function exactly
pub async fn set_password(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    Json(request): Json<PasswordUpdateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - admins can edit other users, users can edit themselves
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id_from_api_key).await?;

    if user_id != user_id_from_api_key && !is_admin {
        return Err(AppError::forbidden("You can only update your own password"));
    }

    state.db_pool.set_password(user_id, &request.hash_pw).await?;
    Ok(Json(serde_json::json!({ "detail": "Password updated." })))
}

// Delete user - matches Python api_delete_user function exactly (admin only)
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    state.db_pool.delete_user(user_id).await?;
    Ok(Json(serde_json::json!({ "status": "User deleted" })))
}

// Request struct for set_email
#[derive(Deserialize)]
pub struct SetEmailRequest {
    pub user_id: i32,
    pub new_email: String,
}

// Set email - matches Python api_set_email function exactly
pub async fn set_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetEmailRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - admins can edit other users, users can edit themselves
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id_from_api_key).await?;

    if request.user_id != user_id_from_api_key && !is_admin {
        return Err(AppError::forbidden("You can only update your own email"));
    }

    state.db_pool.set_email(request.user_id, &request.new_email).await?;
    Ok(Json(serde_json::json!({ "detail": "Email updated." })))
}

// Request struct for set_username  
#[derive(Deserialize)]
pub struct SetUsernameRequest {
    pub user_id: i32,
    pub new_username: String,
}

// Set username - matches Python api_set_username function exactly
pub async fn set_username(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetUsernameRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - admins can edit other users, users can edit themselves
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id_from_api_key).await?;

    if request.user_id != user_id_from_api_key && !is_admin {
        return Err(AppError::forbidden("You can only update your own username"));
    }

    state.db_pool.set_username(request.user_id, &request.new_username.to_lowercase()).await?;
    Ok(Json(serde_json::json!({ "detail": "Username updated." })))
}

// Request struct for set_isadmin
#[derive(Deserialize)]
pub struct SetIsAdminRequest {
    pub user_id: i32,
    pub isadmin: bool,
}

// Set isadmin - matches Python api_set_isadmin function exactly (admin only)
pub async fn set_isadmin(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetIsAdminRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    state.db_pool.set_isadmin(request.user_id, request.isadmin).await?;
    Ok(Json(serde_json::json!({ "detail": "IsAdmin status updated." })))
}

// Final admin check - matches Python api_final_admin function exactly (admin only)
pub async fn final_admin(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    let is_final_admin = state.db_pool.final_admin(user_id).await?;
    Ok(Json(serde_json::json!({ "final_admin": is_final_admin })))
}

// Enable/disable guest - matches Python api_enable_disable_guest function exactly (admin only)
pub async fn enable_disable_guest(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    state.db_pool.enable_disable_guest().await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// Enable/disable downloads - matches Python api_enable_disable_downloads function exactly (admin only)
pub async fn enable_disable_downloads(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    state.db_pool.enable_disable_downloads().await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// Enable/disable self service - matches Python api_enable_disable_self_service function exactly (admin only)
pub async fn enable_disable_self_service(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    state.db_pool.enable_disable_self_service().await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// Get guest status - matches Python api_guest_status function exactly
pub async fn guest_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<bool>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let result = state.db_pool.guest_status().await?;
    Ok(Json(result))
}

// Get RSS feed status - matches Python get_rss_feed_status function exactly
pub async fn rss_feed_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<bool>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let result = state.db_pool.get_rss_feed_status(user_id).await?;
    Ok(Json(result))
}

// Toggle RSS feeds - matches Python toggle_rss_feeds function exactly
pub async fn toggle_rss_feeds(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let new_status = state.db_pool.toggle_rss_feeds(user_id).await?;
    Ok(Json(serde_json::json!({ "success": true, "enabled": new_status })))
}

// Get download status - matches Python api_download_status function exactly
pub async fn download_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<bool>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let result = state.db_pool.download_status().await?;
    Ok(Json(result))
}

// Get self service status - matches Python api_self_service_status function exactly  
pub async fn self_service_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let result = state.db_pool.self_service_status().await?;
    Ok(Json(serde_json::json!({
        "status": result.status,
        "first_admin_created": result.admin_exists
    })))
}

// Request struct for save_email_settings
#[derive(Deserialize)]
pub struct SaveEmailSettingsRequest {
    pub email_settings: EmailSettings,
}

#[derive(Deserialize)]
pub struct EmailSettings {
    pub server_name: String,
    #[serde(deserialize_with = "deserialize_string_to_i32")]
    pub server_port: i32,
    pub from_email: String,
    pub send_mode: String,
    pub encryption: String,
    #[serde(deserialize_with = "deserialize_bool_to_i32")]
    pub auth_required: i32,
    pub email_username: String,
    pub email_password: String,
}

// Helper function to deserialize string to i32
fn deserialize_string_to_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where 
    D: serde::Deserializer<'de>
{
    use serde::de::Error;
    
    let s = String::deserialize(deserializer)?;
    s.parse::<i32>().map_err(D::Error::custom)
}

// Helper function to deserialize bool to i32
fn deserialize_bool_to_i32<'de, D>(deserializer: D) -> Result<i32, D::Error>
where 
    D: serde::Deserializer<'de>
{
    let b = bool::deserialize(deserializer)?;
    Ok(if b { 1 } else { 0 })
}

// Save email settings - matches Python api_save_email_settings function exactly (admin only)
pub async fn save_email_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SaveEmailSettingsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    state.db_pool.save_email_settings(&request.email_settings).await?;
    Ok(Json(serde_json::json!({ "detail": "Email settings saved." })))
}

// Email settings response struct
#[derive(Serialize)]
pub struct EmailSettingsResponse {
    #[serde(rename = "Emailsettingsid")]
    pub emailsettingsid: i32,
    #[serde(rename = "ServerName")]
    pub server_name: String,
    #[serde(rename = "ServerPort")]
    pub server_port: i32,
    #[serde(rename = "FromEmail")]
    pub from_email: String,
    #[serde(rename = "SendMode")]
    pub send_mode: String,
    #[serde(rename = "Encryption")]
    pub encryption: String,
    #[serde(rename = "AuthRequired")]
    pub auth_required: i32,
    #[serde(rename = "Username")]
    pub username: String,
    #[serde(rename = "Password")]
    pub password: String,
}

// Get email settings - matches Python api_get_email_settings function exactly (admin only)
pub async fn get_email_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<EmailSettingsResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    let settings = state.db_pool.get_email_settings().await?;
    match settings {
        Some(settings) => Ok(Json(settings)),
        None => Err(AppError::not_found("Email settings not found")),
    }
}

// Request struct for send_test_email
#[derive(Deserialize)]
pub struct SendTestEmailRequest {
    pub server_name: String,
    pub server_port: String,
    pub from_email: String,
    pub send_mode: String,
    pub encryption: String,
    pub auth_required: bool,
    pub email_username: String,
    pub email_password: String,
    pub to_email: String,
    pub message: String,
}

// Send test email - matches Python api_send_email function exactly (admin only)
pub async fn send_test_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SendTestEmailRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    let email_status = send_email_internal(&request).await?;
    Ok(Json(serde_json::json!({ "email_status": email_status })))
}

// HTML email template functions
async fn read_logo_as_base64() -> Result<String, AppError> {
    use std::path::Path;
    use tokio::fs;
    
    let logo_path = Path::new("/var/www/html/static/assets/favicon.png");
    
    if !logo_path.exists() {
        return Err(AppError::internal("Logo file not found"));
    }
    
    let logo_bytes = fs::read(logo_path).await
        .map_err(|e| AppError::internal(&format!("Failed to read logo file: {}", e)))?;
    
    let base64_logo = base64::encode(&logo_bytes);
    Ok(base64_logo)
}

fn create_html_email_template(subject: &str, content: &str, logo_base64: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
            margin: 0;
            padding: 0;
            background-color: #f8f9fa;
            color: #333333;
        }}
        .email-container {{
            max-width: 600px;
            margin: 0 auto;
            background-color: #ffffff;
            border-radius: 8px;
            box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
            overflow: hidden;
        }}
        .header {{
            background: linear-gradient(135deg, #539e8a 0%, #4a8b7a 100%);
            padding: 32px 24px;
            text-align: center;
        }}
        .logo {{
            width: 64px;
            height: 64px;
            margin: 0 auto 16px;
            display: block;
            border-radius: 12px;
            background-color: rgba(255, 255, 255, 0.1);
            padding: 8px;
        }}
        .header h1 {{
            color: #ffffff;
            margin: 0;
            font-size: 28px;
            font-weight: 600;
            text-shadow: 0 1px 2px rgba(0, 0, 0, 0.1);
        }}
        .content {{
            padding: 32px 24px;
            line-height: 1.6;
        }}
        .content h2 {{
            color: #539e8a;
            margin: 0 0 16px 0;
            font-size: 22px;
            font-weight: 600;
        }}
        .content p {{
            margin: 0 0 16px 0;
            font-size: 16px;
        }}
        .code-block {{
            background-color: #f8f9fa;
            border: 1px solid #e9ecef;
            border-radius: 6px;
            padding: 16px;
            font-family: 'Courier New', Consolas, monospace;
            font-size: 18px;
            font-weight: 600;
            color: #539e8a;
            text-align: center;
            margin: 24px 0;
            letter-spacing: 2px;
        }}
        .footer {{
            background-color: #f8f9fa;
            padding: 24px;
            text-align: center;
            border-top: 1px solid #e9ecef;
        }}
        .footer p {{
            margin: 0;
            font-size: 14px;
            color: #6c757d;
        }}
        .footer a {{
            color: #539e8a;
            text-decoration: none;
        }}
        .footer a:hover {{
            text-decoration: underline;
        }}
    </style>
</head>
<body>
    <div class="email-container">
        <div class="header">
            <img src="data:image/png;base64,{}" alt="PinePods Logo" class="logo">
            <h1>PinePods</h1>
        </div>
        <div class="content">
            {}
        </div>
        <div class="footer">
            <p>This email was sent from your PinePods server.</p>
            <p>Visit <a href="https://github.com/madeofpendletonwool/PinePods">PinePods on GitHub</a> for more information.</p>
        </div>
    </div>
</body>
</html>"#, subject, logo_base64, content)
}

// Internal email sending function using lettre
async fn send_email_internal(request: &SendTestEmailRequest) -> Result<String, AppError> {
    use lettre::{
        message::{header::ContentType, Message},
        transport::smtp::{authentication::Credentials, client::Tls, client::TlsParameters},
        AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    };
    use tokio::time::{timeout, Duration};

    // Parse server port
    let port: u16 = request.server_port.parse()
        .map_err(|_| AppError::bad_request("Invalid server port"))?;

    // Read logo and create HTML content
    let logo_base64 = read_logo_as_base64().await.unwrap_or_default();
    let html_content = format!(r#"
        <h2>📧 Test Email</h2>
        <p>This is a test email from your PinePods server to verify your email configuration is working correctly.</p>
        <p><strong>Your message:</strong></p>
        <p style="background-color: #f8f9fa; padding: 16px; border-radius: 6px; border-left: 4px solid #539e8a;">{}</p>
        <p>If you received this email, your email settings are configured properly! 🎉</p>
    "#, request.message);
    
    let html_body = create_html_email_template("Test Email", &html_content, &logo_base64);

    // Create email message with HTML
    let email = Message::builder()
        .from(request.from_email.parse()
            .map_err(|_| AppError::bad_request("Invalid from email"))?)
        .to(request.to_email.parse()
            .map_err(|_| AppError::bad_request("Invalid to email"))?)
        .subject("PinePods - Test Email")
        .header(ContentType::TEXT_HTML)
        .body(html_body)
        .map_err(|e| AppError::internal(&format!("Failed to build email: {}", e)))?;

    // Configure SMTP transport based on encryption
    let mailer = match request.encryption.as_str() {
        "SSL/TLS" => {
            let tls = TlsParameters::new(request.server_name.clone())
                .map_err(|e| AppError::internal(&format!("TLS configuration failed: {}", e)))?;
            
            if request.auth_required {
                let creds = Credentials::new(request.email_username.clone(), request.email_password.clone());
                AsyncSmtpTransport::<Tokio1Executor>::relay(&request.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(port)
                    .tls(Tls::Wrapper(tls))
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&request.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(port)
                    .tls(Tls::Wrapper(tls))
                    .build()
            }
        }
        "StartTLS" => {
            let tls = TlsParameters::new(request.server_name.clone())
                .map_err(|e| AppError::internal(&format!("TLS configuration failed: {}", e)))?;
            
            if request.auth_required {
                let creds = Credentials::new(request.email_username.clone(), request.email_password.clone());
                AsyncSmtpTransport::<Tokio1Executor>::relay(&request.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(port)
                    .tls(Tls::Required(tls))
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&request.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(port)
                    .tls(Tls::Required(tls))
                    .build()
            }
        }
        _ => {
            // No encryption - use builder_dangerous for unencrypted connections
            if request.auth_required {
                let creds = Credentials::new(request.email_username.clone(), request.email_password.clone());
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&request.server_name)
                    .port(port)
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&request.server_name)
                    .port(port)
                    .build()
            }
        }
    };

    // Send the email with timeout
    let email_future = mailer.send(email);
    match timeout(Duration::from_secs(30), email_future).await {
        Ok(Ok(_)) => Ok("Email sent successfully".to_string()),
        Ok(Err(e)) => {
            let error_msg = format!("{}", e);
            
            // Provide more helpful error messages for common issues
            if error_msg.contains("InvalidContentType") || error_msg.contains("corrupt message") {
                let suggestion = if port == 587 {
                    "Port 587 typically requires StartTLS encryption, not SSL/TLS. Try changing encryption to 'StartTLS'."
                } else if port == 465 {
                    "Port 465 typically requires SSL/TLS encryption."
                } else {
                    "This may be a TLS/SSL configuration issue. Verify your encryption settings match your SMTP server requirements."
                };
                Err(AppError::internal(&format!("SMTP connection failed: {}. {}. Original error: {}", 
                    "TLS/SSL handshake error", suggestion, error_msg)))
            } else if error_msg.contains("authentication") || error_msg.contains("auth") {
                Err(AppError::internal(&format!("SMTP authentication failed: {}. Please verify your username and password.", error_msg)))
            } else if error_msg.contains("connection") || error_msg.contains("timeout") {
                Err(AppError::internal(&format!("SMTP connection failed: {}. Please verify server name and port.", error_msg)))
            } else {
                Err(AppError::internal(&format!("Failed to send email: {}", error_msg)))
            }
        },
        Err(_) => Err(AppError::internal("Email sending timed out after 30 seconds. Please check your SMTP server settings and network connectivity.".to_string())),
    }
}

// Request struct for send_email (using database settings)
#[derive(Deserialize)]
pub struct SendEmailRequest {
    pub to_email: String,
    pub subject: String,
    pub message: String,
}

// Send email using database settings - matches Python api_send_email function exactly
pub async fn send_email(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SendEmailRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Get email settings from database
    let email_settings = state.db_pool.get_email_settings().await?;
    let settings = match email_settings {
        Some(settings) => settings,
        None => return Err(AppError::not_found("Email settings not found")),
    };

    let email_status = send_email_with_settings(&settings, &request).await?;
    Ok(Json(serde_json::json!({ "email_status": email_status })))
}

// Send email using database settings
pub async fn send_email_with_settings(
    settings: &EmailSettingsResponse,
    request: &SendEmailRequest,
) -> Result<String, AppError> {
    use lettre::{
        message::{header::ContentType, Message},
        transport::smtp::{authentication::Credentials, client::Tls, client::TlsParameters},
        AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    };
    use tokio::time::{timeout, Duration};

    // Read logo and create HTML content
    let logo_base64 = read_logo_as_base64().await.unwrap_or_default();
    
    // Check if this is a password reset email and format accordingly
    let (html_content, final_subject) = if request.subject.contains("Password Reset") {
        // Extract the reset code from the message
        let reset_code = request.message.trim_start_matches("Your password reset code is ");
        let content = format!(r#"
            <h2>🔐 Password Reset Request</h2>
            <p>You have requested a password reset for your PinePods account.</p>
            <p>Please use the following code to reset your password:</p>
            <div class="code-block">{}</div>
            <p><strong>Important:</strong></p>
            <ul style="margin: 16px 0; padding-left: 20px;">
                <li>This code will expire in <strong>10 minutes</strong></li>
                <li>Only use this code if you requested a password reset</li>
                <li>If you didn't request this, you can safely ignore this email</li>
            </ul>
            <p>For security reasons, never share this code with anyone.</p>
        "#, reset_code);
        (content, "PinePods - Password Reset Code".to_string())
    } else {
        // For other emails, wrap the message content
        let content = format!(r#"
            <h2>📧 {}</h2>
            <div style="background-color: #f8f9fa; padding: 16px; border-radius: 6px; border-left: 4px solid #539e8a;">
                {}
            </div>
        "#, request.subject, request.message.replace("\n", "<br>"));
        (content, request.subject.clone())
    };
    
    let html_body = create_html_email_template(&final_subject, &html_content, &logo_base64);

    // Create email message with HTML
    let email = Message::builder()
        .from(settings.from_email.parse()
            .map_err(|_| AppError::bad_request("Invalid from email in settings"))?)
        .to(request.to_email.parse()
            .map_err(|_| AppError::bad_request("Invalid to email"))?)
        .subject(&final_subject)
        .header(ContentType::TEXT_HTML)
        .body(html_body)
        .map_err(|e| AppError::internal(&format!("Failed to build email: {}", e)))?;

    // Configure SMTP transport based on encryption
    let mailer = match settings.encryption.as_str() {
        "SSL/TLS" => {
            let tls = TlsParameters::new(settings.server_name.clone())
                .map_err(|e| AppError::internal(&format!("TLS configuration failed: {}", e)))?;
            
            if settings.auth_required == 1 {
                let creds = Credentials::new(settings.username.clone(), settings.password.clone());
                AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(settings.server_port as u16)
                    .tls(Tls::Wrapper(tls))
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(settings.server_port as u16)
                    .tls(Tls::Wrapper(tls))
                    .build()
            }
        }
        "StartTLS" => {
            let tls = TlsParameters::new(settings.server_name.clone())
                .map_err(|e| AppError::internal(&format!("TLS configuration failed: {}", e)))?;
            
            if settings.auth_required == 1 {
                let creds = Credentials::new(settings.username.clone(), settings.password.clone());
                AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(settings.server_port as u16)
                    .tls(Tls::Required(tls))
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(settings.server_port as u16)
                    .tls(Tls::Required(tls))
                    .build()
            }
        }
        _ => {
            // No encryption - use builder_dangerous for unencrypted connections
            if settings.auth_required == 1 {
                let creds = Credentials::new(settings.username.clone(), settings.password.clone());
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.server_name)
                    .port(settings.server_port as u16)
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&settings.server_name)
                    .port(settings.server_port as u16)
                    .build()
            }
        }
    };

    // Send the email with timeout
    let email_future = mailer.send(email);
    match timeout(Duration::from_secs(30), email_future).await {
        Ok(Ok(_)) => Ok("Email sent successfully".to_string()),
        Ok(Err(e)) => {
            let error_msg = format!("{}", e);
            let port = settings.server_port as u16;
            
            // Provide more helpful error messages for common issues
            if error_msg.contains("InvalidContentType") || error_msg.contains("corrupt message") {
                let suggestion = if port == 587 {
                    "Port 587 typically requires StartTLS encryption, not SSL/TLS. Try changing encryption to 'StartTLS'."
                } else if port == 465 {
                    "Port 465 typically requires SSL/TLS encryption."
                } else {
                    "This may be a TLS/SSL configuration issue. Verify your encryption settings match your SMTP server requirements."
                };
                Err(AppError::internal(&format!("SMTP connection failed: {}. {}. Original error: {}", 
                    "TLS/SSL handshake error", suggestion, error_msg)))
            } else if error_msg.contains("authentication") || error_msg.contains("auth") {
                Err(AppError::internal(&format!("SMTP authentication failed: {}. Please verify your username and password.", error_msg)))
            } else if error_msg.contains("connection") || error_msg.contains("timeout") {
                Err(AppError::internal(&format!("SMTP connection failed: {}. Please verify server name and port.", error_msg)))
            } else {
                Err(AppError::internal(&format!("Failed to send email: {}", error_msg)))
            }
        },
        Err(_) => Err(AppError::internal("Email sending timed out after 30 seconds. Please check your SMTP server settings and network connectivity.".to_string())),
    }
}


// API info response struct - matches Python get_api_info response exactly  
#[derive(Serialize)]
pub struct ApiInfo {
    pub apikeyid: i32,
    pub userid: i32,
    pub username: String,
    pub lastfourdigits: String,
    pub created: String,
    pub podcastids: Vec<i32>,
}

// Get API info - matches Python api_get_api_info function exactly
pub async fn get_api_info(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization (elevated access or own user)
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if !is_web_key && user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to access these user details"));
    }

    let api_information = state.db_pool.get_api_info(user_id).await?;
    match api_information {
        Some(info) => Ok(Json(serde_json::json!({ "api_info": info }))),
        None => Err(AppError::not_found("User not found")),
    }
}

// Request struct for create_api_key
#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub user_id: i32,
    pub rssonly: bool,
    pub podcast_ids: Option<Vec<i32>>,
}

// Create API key - matches Python api_create_api_key function exactly
pub async fn create_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or own user
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }

    if request.rssonly {
        let new_key = state.db_pool.create_rss_key(request.user_id, request.podcast_ids).await?;
        Ok(Json(serde_json::json!({ "rss_key": new_key })))
    } else {
        let new_key = state.db_pool.create_api_key(request.user_id).await?;
        Ok(Json(serde_json::json!({ "api_key": new_key })))
    }
}

// Request struct for delete_api_key
#[derive(Deserialize)]
pub struct DeleteApiKeyRequest {
    pub api_id: String,
    pub user_id: String,
}

// Delete API key - matches Python api_delete_api_key function exactly
pub async fn delete_api_key(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeleteApiKeyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Parse api_id from string (user_id not used for authorization)
    let api_id: i32 = request.api_id.parse()
        .map_err(|_| AppError::bad_request("Invalid api_id format"))?;

    // Check authorization - admins can delete any key (except user ID 1), users can only delete their own keys
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_requesting_user_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    // Get the owner of the API key being deleted
    let api_key_owner = state.db_pool.get_api_key_owner(api_id).await?;
    
    if api_key_owner.is_none() {
        return Err(AppError::not_found("API key not found"));
    }
    
    let api_key_owner = api_key_owner.unwrap();

    // For debugging - log the values
    println!("🔐 delete_api_key: requesting_user={}, api_key_owner={}, is_admin={}, api_id={}", 
        requesting_user_id, api_key_owner, is_requesting_user_admin, api_id);

    // Authorization logic:
    // - Admin users can delete any key EXCEPT keys belonging to user ID 1 (background tasks)
    // - Regular users can only delete their own keys
    if !is_requesting_user_admin && requesting_user_id != api_key_owner {
        return Err(AppError::forbidden("You are not authorized to access or remove other users api-keys."));
    }

    // Check if the API key to be deleted is the same as the one used in the current request
    if state.db_pool.is_same_api_key(api_id, &api_key).await? {
        return Err(AppError::forbidden("You cannot delete the API key that is currently in use."));
    }

    // Check if the API key belongs to the background task user (user_id 1) - no one can delete these
    if api_key_owner == 1 {
        return Err(AppError::forbidden("Cannot delete background task API key - would break refreshing."));
    }

    // CRITICAL SAFETY CHECK: Ensure the API key owner has at least one other API key (would prevent logins)
    let remaining_keys_count = state.db_pool.count_user_api_keys_excluding(api_key_owner, api_id).await?;
    if remaining_keys_count == 0 {
        if requesting_user_id == api_key_owner {
            return Err(AppError::forbidden("Cannot delete your final API key - you must have at least one key to maintain access."));
        } else {
            return Err(AppError::forbidden("Cannot delete the user's final API key - they must have at least one key to maintain access."));
        }
    }

    // Proceed with deletion if the checks pass
    state.db_pool.delete_api_key(api_id).await?;
    Ok(Json(serde_json::json!({ "detail": "API key deleted." })))
}

// Request struct for backup_user
#[derive(Deserialize)]
pub struct BackupUserRequest {
    pub user_id: i32,
}

// Backup user data - matches Python backup_user function exactly
pub async fn backup_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BackupUserRequest>,
) -> Result<String, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or own user
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only make backups for yourself!"));
    }

    let opml_data = state.db_pool.backup_user(request.user_id).await?;
    Ok(opml_data)
}

// Request struct for backup_server
#[derive(Deserialize)]
pub struct BackupServerRequest {
    pub database_pass: String,
}

// Backup server data - improved streaming approach for large databases
pub async fn backup_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BackupServerRequest>,
) -> Result<axum::response::Response, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin (matches Python check_if_admin dependency)
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    // For large databases, we'll implement streaming export instead of subprocess
    // This avoids loading the entire database into memory at once
    match backup_server_streaming(&state, &request.database_pass).await {
        Ok(response) => Ok(response),
        Err(e) => Err(AppError::internal(&format!("Backup failed: {}", e))),
    }
}

// Use actual pg_dump/mysqldump for reliable backups
async fn backup_server_streaming(
    state: &AppState,
    database_pass: &str,
) -> Result<axum::response::Response, String> {
    use axum::response::Response;
    use axum::body::Body;
    use tokio::process::Command;
    use tokio_util::io::ReaderStream;

    // Get database connection info from config
    let mut cmd = match &state.db_pool {
        crate::database::DatabasePool::Postgres(_) => {
            // Extract connection details from DATABASE_URL or config
            let host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
            let port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
            let database = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods".to_string());
            let username = std::env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
            
            // Use pg_dump with data-only options (no schema)
            let mut cmd = Command::new("pg_dump");
            cmd.arg("--host").arg(&host)
               .arg("--port").arg(&port)
               .arg("--username").arg(&username)
               .arg("--no-password")
               .arg("--verbose")
               .arg("--data-only")
               .arg("--disable-triggers")
               .arg("--format=plain")
               .arg(&database);
            
            // Set password via environment variable
            cmd.env("PGPASSWORD", database_pass);
            
            cmd
        }
        crate::database::DatabasePool::MySQL(_) => {
            let host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
            let port = std::env::var("DB_PORT").unwrap_or_else(|_| "3306".to_string());
            let database = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods".to_string());
            let username = std::env::var("DB_USER").unwrap_or_else(|_| "root".to_string());
            
            let mut cmd = Command::new("mysqldump");
            cmd.arg("--host").arg(&host)
               .arg("--port").arg(&port)
               .arg("--user").arg(&username)
               .arg(format!("--password={}", database_pass))
               .arg("--skip-ssl")
               .arg("--default-auth=mysql_native_password")
               .arg("--single-transaction")
               .arg("--routines")
               .arg("--triggers")
               .arg("--complete-insert")
               .arg(&database);
            
            cmd
        }
    };

    let mut child = cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start backup process: {}", e))?;

    let stdout = child.stdout.take()
        .ok_or("Failed to get stdout from backup process")?;
    
    let stderr = child.stderr.take()
        .ok_or("Failed to get stderr from backup process")?;

    let stream = ReaderStream::new(stdout);
    let body = Body::from_stream(stream);

    // Spawn a task to wait for the process and handle errors
    tokio::spawn(async move {
        // Read stderr to capture error messages
        let mut stderr_reader = tokio::io::BufReader::new(stderr);
        let mut stderr_output = String::new();
        use tokio::io::AsyncBufReadExt;
        
        // Read stderr line by line
        let mut lines = stderr_reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            stderr_output.push_str(&line);
            stderr_output.push('\n');
        }
        
        match child.wait().await {
            Ok(status) if status.success() => {
                println!("Backup process completed successfully");
            }
            Ok(status) => {
                println!("Backup process failed with status: {}", status);
                if !stderr_output.is_empty() {
                    println!("Mysqldump stderr output: {}", stderr_output);
                }
            }
            Err(e) => {
                println!("Failed to wait for backup process: {}", e);
            }
        }
    });

    Ok(Response::builder()
        .status(200)
        .header("content-type", "text/plain; charset=utf-8")
        .header("content-disposition", "attachment; filename=\"pinepods_backup.sql\"")
        .body(body)
        .map_err(|e| format!("Failed to build response: {}", e))?)
}

// Generate backup chunks to handle large databases efficiently
async fn generate_backup_chunk(state: &AppState, chunk_id: usize) -> Result<Option<String>, String> {
    // Define tables in order of dependencies (foreign keys) - complete list from migrations
    let tables = match &state.db_pool {
        crate::database::DatabasePool::Postgres(_) => vec![
            "Users", "OIDCProviders", "APIKeys", "RssKeys", "RssKeyMap", 
            "AppSettings", "EmailSettings", "UserStats", "UserSettings",
            "Podcasts", "Episodes", "YouTubeVideos", "UserEpisodeHistory", "UserVideoHistory",
            "EpisodeQueue", "SavedEpisodes", "SavedVideos", "DownloadedEpisodes", "DownloadedVideos",
            "GpodderDevices", "GpodderSyncState", "People", "PeopleEpisodes", "SharedEpisodes",
            "Playlists", "PlaylistContents", "Sessions", "UserNotificationSettings"
        ],
        crate::database::DatabasePool::MySQL(_) => vec![
            "Users", "OIDCProviders", "APIKeys", "RssKeys", "RssKeyMap",
            "AppSettings", "EmailSettings", "UserStats", "UserSettings", 
            "Podcasts", "Episodes", "YouTubeVideos", "UserEpisodeHistory", "UserVideoHistory",
            "EpisodeQueue", "SavedEpisodes", "SavedVideos", "DownloadedEpisodes", "DownloadedVideos",
            "GpodderDevices", "GpodderSyncState", "People", "PeopleEpisodes", "SharedEpisodes",
            "Playlists", "PlaylistContents", "Sessions", "UserNotificationSettings"
        ],
    };

    // Header chunk
    if chunk_id == 0 {
        return Ok(Some(generate_backup_header()));
    }

    // Table chunks (one table per chunk to keep memory usage low)
    let table_index = chunk_id - 1;
    if table_index < tables.len() {
        let table_name = tables[table_index];
        match export_table_data(state, table_name).await {
            Ok(data) => Ok(Some(data)),
            Err(e) => Err(format!("Failed to export table {}: {}", table_name, e)),
        }
    } else {
        // End of stream
        Ok(None)
    }
}

// Generate SQL backup header
fn generate_backup_header() -> String {
    format!(
        "-- PinePods Database Backup\n-- Generated: {}\n-- Rust API Backup System\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    )
}

// Export individual table data efficiently
async fn export_table_data(state: &AppState, table_name: &str) -> Result<String, String> {
    const BATCH_SIZE: i64 = 1000; // Process 1000 rows at a time
    let mut sql_output = format!("\n-- Exporting table: {}\n", table_name);
    
    // First, export the CREATE TABLE statement
    let create_statement = match &state.db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            export_postgres_table_schema(pool, table_name).await?
        }
        crate::database::DatabasePool::MySQL(pool) => {
            export_mysql_table_schema(pool, table_name).await?
        }
    };
    
    sql_output.push_str(&create_statement);
    sql_output.push('\n');
    
    // Then export the data
    let mut offset = 0;
    loop {
        let batch_data = match &state.db_pool {
            crate::database::DatabasePool::Postgres(pool) => {
                export_postgres_table_batch(pool, table_name, offset, BATCH_SIZE).await?
            }
            crate::database::DatabasePool::MySQL(pool) => {
                export_mysql_table_batch(pool, table_name, offset, BATCH_SIZE).await?
            }
        };

        if batch_data.is_empty() {
            break; // No more data
        }

        sql_output.push_str(&batch_data);
        offset += BATCH_SIZE;

        // Don't artificially limit chunk size - complete the entire table
        // Each table is processed as one complete chunk to ensure valid SQL
    }

    Ok(sql_output)
}

// Export PostgreSQL table schema using pg_dump-like approach
async fn export_postgres_table_schema(
    pool: &sqlx::PgPool,
    table_name: &str,
) -> Result<String, String> {
    // Get table definition from PostgreSQL system catalogs with proper ARRAY handling
    let query = r#"
        SELECT 
            'CREATE TABLE "' || schemaname || '"."' || tablename || '" (' AS create_start,
            string_agg(
                '"' || column_name || '" ' || 
                CASE 
                    WHEN data_type = 'ARRAY' THEN 
                        CASE 
                            WHEN udt_name = '_int4' THEN 'INTEGER[]'
                            WHEN udt_name = '_text' THEN 'TEXT[]'
                            WHEN udt_name = '_varchar' THEN 'VARCHAR[]'
                            WHEN udt_name = '_int8' THEN 'BIGINT[]'
                            WHEN udt_name = '_bool' THEN 'BOOLEAN[]'
                            ELSE udt_name || '[]'
                        END
                    WHEN data_type = 'character varying' THEN 'VARCHAR(' || COALESCE(character_maximum_length::text, '255') || ')'
                    WHEN data_type = 'character' THEN 'CHAR(' || character_maximum_length || ')'
                    WHEN data_type = 'numeric' THEN 'NUMERIC(' || numeric_precision || ',' || numeric_scale || ')'
                    WHEN data_type = 'integer' THEN 'INTEGER'
                    WHEN data_type = 'bigint' THEN 'BIGINT'
                    WHEN data_type = 'boolean' THEN 'BOOLEAN'
                    WHEN data_type = 'timestamp without time zone' THEN 'TIMESTAMP'
                    WHEN data_type = 'timestamp with time zone' THEN 'TIMESTAMPTZ'
                    WHEN data_type = 'date' THEN 'DATE'
                    WHEN data_type = 'text' THEN 'TEXT'
                    WHEN data_type = 'double precision' THEN 'DOUBLE PRECISION'
                    WHEN data_type = 'real' THEN 'REAL'
                    WHEN data_type = 'smallint' THEN 'SMALLINT'
                    WHEN data_type = 'uuid' THEN 'UUID'
                    WHEN data_type = 'json' THEN 'JSON'
                    WHEN data_type = 'jsonb' THEN 'JSONB'
                    ELSE UPPER(data_type)
                END ||
                CASE WHEN is_nullable = 'NO' THEN ' NOT NULL' ELSE '' END,
                ', '
                ORDER BY ordinal_position
            ) AS columns,
            ');' AS create_end
        FROM information_schema.columns c
        JOIN pg_tables t ON t.tablename = c.table_name
        WHERE c.table_name = $1 AND c.table_schema = 'public'
        GROUP BY schemaname, tablename
    "#;

    let row = sqlx::query(query)
        .bind(table_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Schema query failed: {}", e))?;

    if let Some(row) = row {
        let create_start: String = row.try_get("create_start").map_err(|e| format!("Column error: {}", e))?;
        let columns: String = row.try_get("columns").map_err(|e| format!("Column error: {}", e))?;
        let create_end: String = row.try_get("create_end").map_err(|e| format!("Column error: {}", e))?;
        
        Ok(format!("{}\n    {}\n{}\n", create_start, columns, create_end))
    } else {
        Err(format!("Table {} not found", table_name))
    }
}

// Export MySQL table schema
async fn export_mysql_table_schema(
    pool: &sqlx::MySqlPool,
    table_name: &str,
) -> Result<String, String> {
    // Use SHOW CREATE TABLE for MySQL
    let query = format!("SHOW CREATE TABLE {}", table_name);
    
    let row = sqlx::query(&query)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Schema query failed: {}", e))?;

    if let Some(row) = row {
        let create_table: String = row.try_get(1).map_err(|e| format!("Column error: {}", e))?;
        Ok(format!("{};\n", create_table))
    } else {
        Err(format!("Table {} not found", table_name))
    }
}

// Export PostgreSQL table batch
async fn export_postgres_table_batch(
    pool: &sqlx::PgPool,
    table_name: &str,
    offset: i64,
    limit: i64,
) -> Result<String, String> {
    // Use quoted table names for PostgreSQL
    let query = format!(
        r#"SELECT * FROM "{}" ORDER BY 1 LIMIT {} OFFSET {}"#,
        table_name, limit, offset
    );
    
    let rows = sqlx::query(&query)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Query failed: {}", e))?;

    if rows.is_empty() {
        return Ok(String::new());
    }

    let mut output = format!("INSERT INTO \"{}\" VALUES\n", table_name);
    let mut first_row = true;

    for row in rows {
        if !first_row {
            output.push_str(",\n");
        }
        first_row = false;

        output.push('(');
        let column_count = row.columns().len();
        for i in 0..column_count {
            if i > 0 {
                output.push_str(", ");
            }
            
            // Handle different PostgreSQL data types safely
            match row.try_get_raw(i) {
                Ok(value) if value.is_null() => output.push_str("NULL"),
                Ok(_) => {
                    // Try different data types in order of likelihood
                    if let Ok(val) = row.try_get::<String, _>(i) {
                        // Properly escape strings for PostgreSQL
                        let escaped = val.replace('\'', "''").replace('\\', "\\\\");
                        output.push_str(&format!("'{}'", escaped));
                    } else if let Ok(val) = row.try_get::<i32, _>(i) {
                        output.push_str(&val.to_string());
                    } else if let Ok(val) = row.try_get::<i64, _>(i) {
                        output.push_str(&val.to_string());
                    } else if let Ok(val) = row.try_get::<bool, _>(i) {
                        output.push_str(if val { "true" } else { "false" });
                    } else if let Ok(val) = row.try_get::<f64, _>(i) {
                        output.push_str(&val.to_string());
                    } else if let Ok(val) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(i) {
                        output.push_str(&format!("'{}'", val.format("%Y-%m-%d %H:%M:%S%.6f%z")));
                    } else {
                        // Fallback: try to get as text
                        match row.try_get::<String, _>(i) {
                            Ok(val) => {
                                let escaped = val.replace('\'', "''").replace('\\', "\\\\");
                                output.push_str(&format!("'{}'", escaped));
                            },
                            Err(_) => output.push_str("NULL"),
                        }
                    }
                }
                Err(_) => output.push_str("NULL"),
            }
        }
        output.push(')');
    }
    output.push_str(";\n");

    Ok(output)
}

// Export MySQL table batch  
async fn export_mysql_table_batch(
    pool: &sqlx::MySqlPool,
    table_name: &str,
    offset: i64,
    limit: i64,
) -> Result<String, String> {
    let query = format!(
        "SELECT * FROM {} ORDER BY 1 LIMIT {} OFFSET {}",
        table_name, limit, offset
    );
    
    let rows = sqlx::query(&query)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Query failed: {}", e))?;

    if rows.is_empty() {
        return Ok(String::new());
    }

    let mut output = format!("INSERT INTO {} VALUES\n", table_name);
    let mut first_row = true;

    for row in rows {
        if !first_row {
            output.push_str(",\n");
        }
        first_row = false;

        output.push('(');
        let column_count = row.columns().len();
        for i in 0..column_count {
            if i > 0 {
                output.push_str(", ");
            }
            
            // Handle different MySQL data types safely
            match row.try_get_raw(i) {
                Ok(value) if value.is_null() => output.push_str("NULL"),
                Ok(_) => {
                    // Try different data types in order of likelihood
                    if let Ok(val) = row.try_get::<String, _>(i) {
                        // Properly escape strings for MySQL
                        let escaped = val.replace('\'', "''").replace('\\', "\\\\");
                        output.push_str(&format!("'{}'", escaped));
                    } else if let Ok(val) = row.try_get::<i32, _>(i) {
                        output.push_str(&val.to_string());
                    } else if let Ok(val) = row.try_get::<i64, _>(i) {
                        output.push_str(&val.to_string());
                    } else if let Ok(val) = row.try_get::<bool, _>(i) {
                        output.push_str(&val.to_string());
                    } else if let Ok(val) = row.try_get::<f64, _>(i) {
                        output.push_str(&val.to_string());
                    } else if let Ok(val) = row.try_get::<chrono::DateTime<chrono::Utc>, _>(i) {
                        output.push_str(&format!("'{}'", val.format("%Y-%m-%d %H:%M:%S")));
                    } else {
                        // Fallback: try to get as text
                        match row.try_get::<String, _>(i) {
                            Ok(val) => {
                                let escaped = val.replace('\'', "''").replace('\\', "\\\\");
                                output.push_str(&format!("'{}'", escaped));
                            },
                            Err(_) => output.push_str("NULL"),
                        }
                    }
                }
                Err(_) => output.push_str("NULL"),
            }
        }
        output.push(')');
    }
    output.push_str(";\n");

    Ok(output)
}

pub async fn restore_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id).await?;

    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    // Process the multipart form to get the uploaded file and database password
    let mut sql_content = None;
    let mut _database_password = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::bad_request(&format!("Multipart error: {}", e)))? {
        let name = field.name().unwrap_or("").to_string();

        if name == "backup_file" {
            let filename = field.file_name().unwrap_or("").to_string();

            // Validate file extension
            if !filename.ends_with(".sql") {
                return Err(AppError::bad_request("Only SQL files are allowed"));
            }

            let data = field.bytes().await.map_err(|e| AppError::bad_request(&format!("Failed to read file: {}", e)))?;

            // Check file size (limit to 100MB)
            if data.len() > 100 * 1024 * 1024 {
                return Err(AppError::bad_request("File too large (max 100MB)"));
            }

            sql_content = Some(String::from_utf8(data.to_vec()).map_err(|_| AppError::bad_request("Invalid UTF-8 content"))?);
        } else if name == "database_pass" {
            let password_data = field.bytes().await.map_err(|e| AppError::bad_request(&format!("Failed to read password: {}", e)))?;
            _database_password = Some(String::from_utf8(password_data.to_vec()).map_err(|_| AppError::bad_request("Invalid UTF-8 password"))?);
        }
    }

    let sql_content = sql_content.ok_or_else(|| AppError::bad_request("No SQL file uploaded"))?;
    let _database_password = _database_password.ok_or_else(|| AppError::bad_request("Database password is required"))?;

    // Process the restore in the background to prevent timeouts
    let db_pool = state.db_pool.clone();
    tokio::spawn(async move {
        if let Err(e) = db_pool.restore_server_data(&sql_content).await {
            tracing::error!("Restore failed: {}", e);
        }
    });

    Ok(Json(serde_json::json!({
        "message": "Server restore started successfully"
    })))
}

// Generate MFA secret - matches Python generate_mfa_secret function exactly
pub async fn generate_mfa_secret(
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
        return Err(AppError::forbidden("You can only generate MFA secrets for yourself!"));
    }

    let (secret, qr_code_svg) = state.db_pool.generate_mfa_secret(user_id).await?;
    Ok(Json(serde_json::json!({
        "secret": secret,
        "qr_code_svg": qr_code_svg
    })))
}

// Request struct for verify_temp_mfa
#[derive(Deserialize)]
pub struct VerifyTempMfaRequest {
    pub user_id: i32,
    pub mfa_code: String,
}

// Verify temporary MFA code - matches Python verify_temp_mfa function exactly
pub async fn verify_temp_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<VerifyTempMfaRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or own user
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only verify MFA codes for yourself!"));
    }

    let verified = state.db_pool.verify_temp_mfa(request.user_id, &request.mfa_code).await?;
    Ok(Json(serde_json::json!({ "verified": verified })))
}

// Check MFA enabled - matches Python check_mfa_enabled function exactly  
pub async fn check_mfa_enabled(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check for elevated access (admin/web key)
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // If not elevated access, user can only check their own MFA status
    if !is_web_key && user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to check mfa status for other users."));
    }

    let is_enabled = state.db_pool.check_mfa_enabled(user_id).await?;
    Ok(Json(serde_json::json!({"mfa_enabled": is_enabled})))
}

// Request struct for save_mfa_secret
#[derive(Deserialize)]
pub struct SaveMfaSecretRequest {
    pub user_id: i32,
    pub mfa_secret: String,
}

// Save MFA secret - matches Python save_mfa_secret function exactly
pub async fn save_mfa_secret(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SaveMfaSecretRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or own user
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only save MFA secrets for yourself!"));
    }

    let success = state.db_pool.save_mfa_secret(request.user_id, &request.mfa_secret).await?;
    Ok(Json(serde_json::json!({ "success": success })))
}

// Delete MFA - matches Python delete_mfa function exactly
pub async fn delete_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let success = state.db_pool.delete_mfa_secret(user_id).await?;
    Ok(Json(serde_json::json!({ "success": success })))
}

// Request struct for initiate_nextcloud_login
#[derive(Deserialize)]
pub struct InitiateNextcloudLoginRequest {
    pub user_id: i32,
    pub nextcloud_url: String,
}

// Initiate Nextcloud login - matches Python initiate_nextcloud_login function exactly
pub async fn initiate_nextcloud_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InitiateNextcloudLoginRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    // Allow the action only if the API key belongs to the user
    if key_id != request.user_id {
        return Err(AppError::forbidden("You are not authorized to initiate this action."));
    }

    let login_data = state.db_pool.initiate_nextcloud_login(request.user_id, &request.nextcloud_url).await?;
    
    Ok(Json(login_data.raw_response))
}

// Request struct for add_nextcloud_server
#[derive(Deserialize, Clone)]
pub struct AddNextcloudServerRequest {
    pub user_id: i32,
    pub token: String,
    pub poll_endpoint: String,
    pub nextcloud_url: String,
}

// Add Nextcloud server - matches Python add_nextcloud_server function exactly
pub async fn add_nextcloud_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddNextcloudServerRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    // Allow the action only if the API key belongs to the user
    if key_id != request.user_id {
        return Err(AppError::forbidden("You are not authorized to access these user details"));
    }

    // Reset gPodder settings to default like Python version
    state.db_pool.remove_podcast_sync(request.user_id).await?;

    // Create a task for the Nextcloud authentication polling  
    let task_id = state.task_manager.create_task("nextcloud_auth".to_string(), request.user_id).await?;
    
    // Start background polling task using TaskManager
    let state_clone = state.clone();
    let request_clone = request.clone();
    let task_id_clone = task_id.clone();
    tokio::spawn(async move {
        poll_for_auth_completion_background(state_clone, request_clone, task_id_clone).await;
    });

    // Return 200 status code before starting to poll (like Python version)
    Ok(Json(serde_json::json!({ "status": "polling", "task_id": task_id })))
}

// Background task for polling Nextcloud auth completion
async fn poll_for_auth_completion_background(state: AppState, request: AddNextcloudServerRequest, task_id: String) {
    // Update task to indicate polling has started
    if let Err(e) = state.task_manager.update_task_progress(&task_id, 10.0, Some("Starting Nextcloud authentication polling...".to_string())).await {
        eprintln!("Failed to update task progress: {}", e);
    }

    match poll_for_auth_completion(&request.poll_endpoint, &request.token, &state.task_manager, &task_id).await {
        Ok(credentials) => {
            println!("Nextcloud authentication successful: {:?}", credentials);
            
            // Update task progress
            if let Err(e) = state.task_manager.update_task_progress(&task_id, 90.0, Some("Authentication successful, saving credentials...".to_string())).await {
                eprintln!("Failed to update task progress: {}", e);
            }
            
            // Extract credentials from the response
            if let (Some(app_password), Some(login_name)) = (
                credentials.get("appPassword").and_then(|v| v.as_str()),
                credentials.get("loginName").and_then(|v| v.as_str())
            ) {
                // Save the real credentials using the database method
                match state.db_pool.save_nextcloud_credentials(request.user_id, &request.nextcloud_url, app_password, login_name).await {
                    Ok(_) => {
                        println!("Successfully added Nextcloud settings for user {}", request.user_id);
                        if let Err(e) = state.task_manager.complete_task(&task_id, 
                            Some(serde_json::json!({"status": "success", "message": "Nextcloud authentication completed"})), 
                            Some("Nextcloud authentication completed successfully".to_string())).await {
                            eprintln!("Failed to complete task: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to add Nextcloud settings: {}", e);
                        if let Err(e) = state.task_manager.fail_task(&task_id, format!("Failed to save Nextcloud settings: {}", e)).await {
                            eprintln!("Failed to fail task: {}", e);
                        }
                    }
                }
            } else {
                eprintln!("Missing appPassword or loginName in credentials");
                if let Err(e) = state.task_manager.fail_task(&task_id, "Missing credentials in Nextcloud response".to_string()).await {
                    eprintln!("Failed to fail task: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Nextcloud authentication failed: {}", e);
            if let Err(e) = state.task_manager.fail_task(&task_id, format!("Authentication failed: {}", e)).await {
                eprintln!("Failed to fail task: {}", e);
            }
        }
    }
}

// Poll for auth completion - matches Python poll_for_auth_completion function
async fn poll_for_auth_completion(
    endpoint: &str, 
    token: &str, 
    task_manager: &crate::services::task_manager::TaskManager,
    task_id: &str
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let payload = serde_json::json!({ "token": token });
    let timeout = std::time::Duration::from_secs(20 * 60); // 20 minutes timeout
    let start_time = std::time::Instant::now();

    let mut poll_count = 0;
    while start_time.elapsed() < timeout {
        poll_count += 1;
        
        // Update progress based on time elapsed (up to 80% during polling)
        let elapsed_secs = start_time.elapsed().as_secs();
        let progress = 10.0 + ((elapsed_secs as f64 / (20.0 * 60.0)) * 70.0).min(70.0);
        let message = format!("Waiting for user to complete authentication... (attempt {})", poll_count);
        
        if let Err(e) = task_manager.update_task_progress(task_id, progress, Some(message)).await {
            eprintln!("Failed to update task progress during polling: {}", e);
        }
        
        match client
            .post(endpoint)
            .json(&payload)
            .header("Content-Type", "application/json")
            .send()
            .await
        {
            Ok(response) => {
                match response.status().as_u16() {
                    200 => {
                        let credentials = response.json::<serde_json::Value>().await?;
                        println!("Authentication successful: {:?}", credentials);
                        return Ok(credentials);
                    }
                    404 => {
                        // User hasn't completed auth yet, continue polling
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                    status => {
                        println!("Polling failed with status code {}", status);
                        return Err(format!("Polling for Nextcloud authentication failed with status {}", status).into());
                    }
                }
            }
            Err(e) => {
                println!("Connection error, retrying: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }

    Err("Polling timeout reached".into())
}

// Helper function to save Nextcloud credentials directly to database
async fn save_nextcloud_credentials(
    db_pool: &crate::database::DatabasePool,
    user_id: i32,
    nextcloud_url: &str,
    app_password: &str,
    login_name: &str
) -> crate::error::AppResult<()> {
    // Encrypt the app password
    let encrypted_password = db_pool.encrypt_password(app_password).await?;
    
    // Store Nextcloud credentials
    match db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            sqlx::query(r#"UPDATE "Users" SET gpodderurl = $1, gpodderloginname = $2, gpoddertoken = $3, pod_sync_type = 'nextcloud' WHERE userid = $4"#)
                .bind(nextcloud_url)
                .bind(login_name)
                .bind(&encrypted_password)
                .bind(user_id)
                .execute(pool)
                .await?;
        }
        crate::database::DatabasePool::MySQL(pool) => {
            sqlx::query("UPDATE Users SET GpodderUrl = ?, GpodderLoginName = ?, GpodderToken = ?, Pod_Sync_Type = 'nextcloud' WHERE UserID = ?")
                .bind(nextcloud_url)
                .bind(login_name)
                .bind(&encrypted_password)
                .bind(user_id)
                .execute(pool)
                .await?;
        }
    }
    
    Ok(())
}

// Request struct for verify_gpodder_auth
#[derive(Deserialize)]
pub struct VerifyGpodderAuthRequest {
    pub gpodder_url: String,
    pub gpodder_username: String,
    pub gpodder_password: String,
}

// Verify gPodder authentication - matches Python verify_gpodder_auth function exactly
pub async fn verify_gpodder_auth(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<VerifyGpodderAuthRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Direct HTTP call to match Python implementation exactly
    let client = reqwest::Client::new();
    let auth_url = format!("{}/api/2/auth/{}/login.json", 
                          request.gpodder_url.trim_end_matches('/'), 
                          request.gpodder_username);
    
    match client
        .post(&auth_url)
        .basic_auth(&request.gpodder_username, Some(&request.gpodder_password))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                Ok(Json(serde_json::json!({"status": "success", "message": "Logged in!"})))
            } else {
                Err(AppError::unauthorized("Authentication failed"))
            }
        }
        Err(_) => {
            Err(AppError::internal("Internal Server Error"))
        }
    }
}

// Request struct for add_gpodder_server
#[derive(Deserialize)]
pub struct AddGpodderServerRequest {
    pub gpodder_url: String,
    pub gpodder_username: String,
    pub gpodder_password: String,
}

// Add gPodder server - matches Python add_gpodder_server function exactly
pub async fn add_gpodder_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddGpodderServerRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let success = state.db_pool.add_gpodder_server(user_id, &request.gpodder_url, &request.gpodder_username, &request.gpodder_password).await?;
    
    if success {
        Ok(Json(serde_json::json!({ "status": "success" })))
    } else {
        Err(AppError::internal("Failed to add gPodder server"))
    }
}

// Get gPodder settings - matches Python get_gpodder_settings function exactly
pub async fn get_gpodder_settings(
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
        return Err(AppError::forbidden("You can only view your own gPodder settings!"));
    }

    let settings = state.db_pool.get_gpodder_settings(user_id).await?;
    match settings {
        Some(settings) => Ok(Json(serde_json::json!({ "data": settings }))),
        None => Err(AppError::not_found("gPodder settings not found")),
    }
}

// Check gPodder settings - matches Python check_gpodder_settings function exactly
pub async fn check_gpodder_settings(
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
        return Err(AppError::forbidden("You can only check your own gPodder settings!"));
    }

    let has_settings = state.db_pool.check_gpodder_settings(user_id).await?;
    Ok(Json(serde_json::json!({ "data": has_settings })))
}


// Remove podcast sync - matches Python remove_podcast_sync function exactly
#[derive(Debug, serde::Deserialize)]
pub struct RemoveSyncRequest {
    pub user_id: i32,
}

pub async fn remove_podcast_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RemoveSyncRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if the user has permission to modify this user's data
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You are not authorized to modify these user settings"));
    }

    // Remove the sync settings
    let success = state.db_pool.remove_gpodder_settings(request.user_id).await?;
    
    if success {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Podcast sync settings removed successfully"
        })))
    } else {
        Err(AppError::internal("Failed to remove podcast sync settings"))
    }
}

// === NEW ENDPOINTS - REMAINING SETTINGS ===

// Request struct for add_custom_podcast
#[derive(Deserialize)]
pub struct CustomPodcastRequest {
    pub feed_url: String,
    pub user_id: i32,
    pub username: Option<String>,
    pub password: Option<String>,
    pub youtube_channel: Option<bool>,
    pub feed_cutoff: Option<i32>,
}

// Request struct for import_opml
#[derive(Deserialize)]
pub struct OpmlImportRequest {
    pub podcasts: Vec<String>,
    pub user_id: i32,
}

// Response struct for import_progress
#[derive(Serialize)]
pub struct ImportProgressResponse {
    pub current: i32,
    pub total: i32,
    pub current_podcast: String,
}

// Request struct for notification_settings
#[derive(Deserialize)]
pub struct NotificationSettingsRequest {
    pub user_id: i32,
    pub platform: String,
    pub enabled: bool,
    pub ntfy_topic: Option<String>,
    pub ntfy_server_url: Option<String>,
    pub ntfy_username: Option<String>,
    pub ntfy_password: Option<String>,
    pub ntfy_access_token: Option<String>,
    pub gotify_url: Option<String>,
    pub gotify_token: Option<String>,
    pub http_url: Option<String>,
    pub http_token: Option<String>,
    pub http_method: Option<String>,
}

// Request struct for test_notification
#[derive(Deserialize)]
pub struct NotificationTestRequest {
    pub user_id: i32,
    pub platform: String,
}

// Request struct for add_oidc_provider
#[derive(Deserialize)]
pub struct OidcProviderRequest {
    pub provider_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub authorization_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub button_text: String,
    pub scope: String,
    pub button_color: String,
    pub button_text_color: String,
    pub icon_svg: Option<String>,
    pub name_claim: Option<String>,
    pub email_claim: Option<String>,
    pub username_claim: Option<String>,
    pub roles_claim: Option<String>,
    pub user_role: Option<String>,
    pub admin_role: Option<String>,
}

// Query structs for user_id parameters
#[derive(Deserialize)]
pub struct UserIdQuery {
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct StartpageQuery {
    pub user_id: i32,
    pub startpage: Option<String>,
}

// Add custom podcast - matches Python add_custom_podcast function exactly
pub async fn add_custom_podcast(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CustomPodcastRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only add podcasts for themselves
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only add podcasts for yourself!"));
    }

    // Check if this is a YouTube channel request
    if request.youtube_channel.unwrap_or(false) {
        // Extract channel ID from YouTube URL
        let channel_id = extract_youtube_channel_id(&request.feed_url)?;

        // Check if channel already exists
        let existing_id = state.db_pool.check_existing_channel_subscription(
            &channel_id,
            request.user_id,
        ).await?;

        if let Some(podcast_id) = existing_id {
            // Channel already subscribed, return existing podcast details
            let podcast_details = state.db_pool.get_podcast_details(request.user_id, podcast_id).await?;
            return Ok(Json(serde_json::json!({ "data": podcast_details })));
        }

        // Get channel info using yt-dlp (bypasses Google API limits)
        let channel_info = crate::handlers::youtube::get_youtube_channel_info(&channel_id).await?;

        let feed_cutoff = request.feed_cutoff.unwrap_or(30);

        // Add YouTube channel to database
        let podcast_id = state.db_pool.add_youtube_channel(
            &channel_info,
            request.user_id,
            feed_cutoff,
        ).await?;

        // Spawn background task to process YouTube videos
        let state_clone = state.clone();
        let channel_id_clone = channel_id.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::handlers::youtube::process_youtube_channel(
                podcast_id,
                &channel_id_clone,
                feed_cutoff,
                &state_clone
            ).await {
                println!("Error processing YouTube channel {}: {}", channel_id_clone, e);
            }
        });

        // Get complete podcast details for response
        let podcast_details = state.db_pool.get_podcast_details(request.user_id, podcast_id).await?;

        return Ok(Json(serde_json::json!({ "data": podcast_details })));
    }

    // Regular podcast feed handling
    // Get podcast values from feed URL
    let podcast_values = state.db_pool.get_podcast_values(
        &request.feed_url,
        request.user_id,
        request.username.as_deref(),
        request.password.as_deref()
    ).await?;

    // Add podcast with 30 episode cutoff (matches Python default)
    let (podcast_id, _) = state.db_pool.add_podcast_from_values(
        &podcast_values,
        request.user_id,
        30,
        request.username.as_deref(),
        request.password.as_deref()
    ).await?;

    // Get complete podcast details for response
    let podcast_details = state.db_pool.get_podcast_details(request.user_id, podcast_id).await?;

    Ok(Json(serde_json::json!({ "data": podcast_details })))
}

// Helper function to extract YouTube channel ID from various URL formats
fn extract_youtube_channel_id(url: &str) -> Result<String, AppError> {
    // Support various YouTube URL formats:
    // - https://www.youtube.com/channel/UC...
    // - https://youtube.com/channel/UC...
    // - https://www.youtube.com/@channelname
    // - youtube.com/@channelname
    // - Just the channel ID itself: UC...

    let url_lower = url.to_lowercase();

    // If it's already a channel ID (starts with UC)
    if url.starts_with("UC") && !url.contains('/') && !url.contains('.') {
        return Ok(url.to_string());
    }

    // Extract from /channel/ URLs
    if url_lower.contains("/channel/") {
        if let Some(channel_part) = url.split("/channel/").nth(1) {
            let channel_id = channel_part.split(&['/', '?', '&'][..]).next().unwrap_or("");
            if !channel_id.is_empty() {
                return Ok(channel_id.to_string());
            }
        }
    }

    // For @handle URLs, we need to use yt-dlp to resolve the channel ID
    // This will be handled by get_youtube_channel_info, so we return the URL as-is
    if url_lower.contains("/@") || url.starts_with('@') {
        return Ok(url.to_string());
    }

    Err(AppError::bad_request(&format!(
        "Invalid YouTube channel URL. Expected format: https://www.youtube.com/channel/UC... or https://www.youtube.com/@channelname or just the channel ID. Got: {}",
        url
    )))
}

// Import OPML - matches Python import_opml function exactly with background processing
pub async fn import_opml(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OpmlImportRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only import OPML for yourself!"));
    }

    let total_podcasts = request.podcasts.len();

    // Initialize progress tracking in Redis/Valkey
    state.import_progress_manager.start_import(request.user_id, total_podcasts as i32).await?;

    // Spawn background task for OPML processing
    let state_clone = state.clone();
    let podcasts = request.podcasts.clone();
    let user_id = request.user_id;

    tokio::spawn(async move {
        for (index, feed_url) in podcasts.iter().enumerate() {
            // Update progress
            let _ = state_clone.import_progress_manager.update_progress(
                user_id,
                index as i32,
                feed_url
            ).await;

            // Process podcast (with error handling to continue on failures)
            match state_clone.db_pool.get_podcast_values(feed_url, user_id, None, None).await {
                Ok(podcast_values) => {
                    let _ = state_clone.db_pool.add_podcast_from_values(
                        &podcast_values,
                        user_id,
                        30,  // feed_cutoff
                        None, // username
                        None  // password
                    ).await;
                }
                Err(e) => {
                    tracing::error!("Failed to import podcast {}: {}", feed_url, e);
                    // Continue with next podcast
                }
            }

            // Small delay between imports (matches Python 0.1s delay)
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Clear progress when complete
        let _ = state_clone.import_progress_manager.clear_progress(user_id).await;
    });

    Ok(Json(serde_json::json!({
        "message": "OPML import started",
        "total": total_podcasts
    })))
}

// Import progress webhook - matches Python import_progress function exactly
pub async fn import_progress(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<ImportProgressResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only check their own progress
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only check your own import progress!"));
    }

    let (current, total, current_podcast) = state.import_progress_manager.get_progress(user_id).await?;
    let progress = ImportProgressResponse {
        current,
        total,
        current_podcast,
    };
    Ok(Json(progress))
}

// Get notification settings - matches Python notification_settings GET function exactly
pub async fn get_notification_settings(
    State(state): State<AppState>,
    Query(query): Query<UserIdQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if query.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only view your own notification settings!"));
    }

    let settings = state.db_pool.get_notification_settings(query.user_id).await?;
    Ok(Json(serde_json::json!({ "settings": settings })))
}

// Update notification settings - matches Python notification_settings PUT function exactly
pub async fn update_notification_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<NotificationSettingsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only update your own notification settings!"));
    }

    state.db_pool.update_notification_settings(
        request.user_id,
        &request.platform,
        request.enabled,
        request.ntfy_topic.as_deref(),
        request.ntfy_server_url.as_deref(),
        request.ntfy_username.as_deref(),
        request.ntfy_password.as_deref(),
        request.ntfy_access_token.as_deref(),
        request.gotify_url.as_deref(),
        request.gotify_token.as_deref(),
        request.http_url.as_deref(),
        request.http_token.as_deref(),
        request.http_method.as_deref()
    ).await?;
    Ok(Json(serde_json::json!({ "detail": "Notification settings updated successfully" })))
}

// Test notification - matches Python test_notification function exactly
pub async fn test_notification(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<NotificationTestRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if request.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only test your own notifications!"));
    }

    // Get notification settings and send test notification
    let settings = state.db_pool.get_notification_settings(request.user_id).await?;
    
    // Find settings for the specific platform
    let platform_settings = settings.iter()
        .find(|s| s.get("platform").and_then(|p| p.as_str()) == Some(&request.platform))
        .ok_or_else(|| AppError::bad_request(&format!("No settings found for platform: {}", request.platform)))?;
    
    let success = state.notification_manager.send_test_notification(request.user_id, &request.platform, platform_settings).await?;
    
    if success {
        Ok(Json(serde_json::json!({ "detail": "Test notification sent successfully" })))
    } else {
        Err(AppError::bad_request("Failed to send test notification - check your settings"))
    }
}

// Add OIDC provider - matches Python add_oidc_provider function exactly  
pub async fn add_oidc_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<OidcProviderRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin - OIDC provider management requires admin access
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id).await?;

    if !is_admin {
        return Err(AppError::forbidden("Admin access required to add OIDC providers"));
    }

    let provider_id = state.db_pool.add_oidc_provider(
        &request.provider_name,
        &request.client_id,
        &request.client_secret,
        &request.authorization_url,
        &request.token_url,
        &request.user_info_url,
        &request.button_text,
        &request.scope,
        &request.button_color,
        &request.button_text_color,
        request.icon_svg.as_deref().unwrap_or(""),
        request.name_claim.as_deref().unwrap_or("name"),
        request.email_claim.as_deref().unwrap_or("email"),
        request.username_claim.as_deref().unwrap_or("username"),
        request.roles_claim.as_deref().unwrap_or(""),
        request.user_role.as_deref().unwrap_or(""),
        request.admin_role.as_deref().unwrap_or(""),
        false // initialized_from_env = false (added via UI)
    ).await?;
    Ok(Json(serde_json::json!({ "provider_id": provider_id })))
}

// Update OIDC provider - updates an existing provider
pub async fn update_oidc_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(provider_id): Path<i32>,
    Json(request): Json<OidcProviderRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin - OIDC provider management requires admin access
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id).await?;

    if !is_admin {
        return Err(AppError::forbidden("Admin access required to update OIDC providers"));
    }

    // Only update client_secret if it's not empty
    let client_secret_to_update = if request.client_secret.is_empty() {
        None
    } else {
        Some(request.client_secret.as_str())
    };

    let success = state.db_pool.update_oidc_provider(
        provider_id,
        &request.provider_name,
        &request.client_id,
        client_secret_to_update,
        &request.authorization_url,
        &request.token_url,
        &request.user_info_url,
        &request.button_text,
        &request.scope,
        &request.button_color,
        &request.button_text_color,
        request.icon_svg.as_deref().unwrap_or(""),
        request.name_claim.as_deref().unwrap_or("name"),
        request.email_claim.as_deref().unwrap_or("email"),
        request.username_claim.as_deref().unwrap_or("username"),
        request.roles_claim.as_deref().unwrap_or(""),
        request.user_role.as_deref().unwrap_or(""),
        request.admin_role.as_deref().unwrap_or("")
    ).await?;

    if success {
        Ok(Json(serde_json::json!({ "message": "OIDC provider updated successfully" })))
    } else {
        Err(AppError::not_found("OIDC provider not found"))
    }
}

// List OIDC providers - matches Python list_oidc_providers function exactly
pub async fn list_oidc_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let providers = state.db_pool.list_oidc_providers().await?;
    Ok(Json(serde_json::json!({ "providers": providers })))
}

// Remove OIDC provider - matches Python remove_oidc_provider function exactly
pub async fn remove_oidc_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(provider_id): Json<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin - OIDC provider management requires admin access
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(user_id).await?;

    if !is_admin {
        return Err(AppError::forbidden("Admin access required to remove OIDC providers"));
    }

    // Check if provider was initialized from environment variables
    let is_env_initialized = state.db_pool.is_oidc_provider_env_initialized(provider_id).await?;
    if is_env_initialized {
        return Err(AppError::forbidden("Cannot remove OIDC provider that was initialized from environment variables. Providers created from docker-compose environment variables are protected from removal to prevent login issues."));
    }

    let success = state.db_pool.remove_oidc_provider(provider_id).await?;
    
    if success {
        Ok(Json(serde_json::json!({ "message": "OIDC provider removed successfully" })))
    } else {
        Err(AppError::not_found("OIDC provider not found"))
    }
}

// Get startpage - matches Python startpage GET function exactly
pub async fn get_startpage(
    State(state): State<AppState>,
    Query(query): Query<UserIdQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if query.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only view your own startpage setting!"));
    }

    let startpage = state.db_pool.get_startpage(query.user_id).await?;
    Ok(Json(serde_json::json!({ "StartPage": startpage })))
}

// Update startpage - matches Python startpage POST function exactly
pub async fn update_startpage(
    State(state): State<AppState>,
    Query(query): Query<StartpageQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if query.user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only update your own startpage setting!"));
    }

    let startpage = query.startpage.unwrap_or_else(|| "home".to_string());
    state.db_pool.update_startpage(query.user_id, &startpage).await?;
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "StartPage updated successfully"
    })))
}

// Request struct for person subscribe
#[derive(Deserialize)]
pub struct PersonSubscribeRequest {
    pub person_name: String,
    pub person_img: String,
    pub podcast_id: i32,
}

// Request struct for person unsubscribe
#[derive(Deserialize)]
pub struct PersonUnsubscribeRequest {
    pub person_name: String,
}

// Subscribe to person - matches Python api_subscribe_to_person function exactly
pub async fn subscribe_to_person(
    State(state): State<AppState>,
    Path((user_id, person_id)): Path<(i32, i32)>,
    headers: HeaderMap,
    Json(request): Json<PersonSubscribeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only subscribe for themselves
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only subscribe for yourself!"));
    }

    let person_db_id = state.db_pool.subscribe_to_person(
        user_id,
        person_id,
        &request.person_name,
        &request.person_img,
        request.podcast_id,
    ).await?;

    // Trigger immediate background task to process person subscription and gather episodes
    let db_pool = state.db_pool.clone();
    let person_name = request.person_name.clone();
    tokio::spawn(async move {
        match db_pool.process_person_subscription(user_id, person_db_id, person_name.clone()).await {
            Ok(_) => {
                tracing::info!("Successfully processed immediate person subscription for {}", person_name);
            }
            Err(e) => {
                tracing::error!("Failed to process immediate person subscription for {}: {}", person_name, e);
            }
        }
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "message": "Successfully subscribed to person",
        "person_id": person_db_id
    })))
}

// Unsubscribe from person - matches Python api_unsubscribe_from_person function exactly
pub async fn unsubscribe_from_person(
    State(state): State<AppState>,
    Path((user_id, person_id)): Path<(i32, i32)>,
    headers: HeaderMap,
    Json(request): Json<PersonUnsubscribeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only unsubscribe for themselves
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only unsubscribe for yourself!"));
    }

    let success = state.db_pool.unsubscribe_from_person(
        user_id,
        person_id,
        &request.person_name,
    ).await?;

    if success {
        Ok(Json(serde_json::json!({
            "success": true,
            "message": "Successfully unsubscribed from person"
        })))
    } else {
        Ok(Json(serde_json::json!({
            "success": false,
            "message": "Person subscription not found"
        })))
    }
}

// Get person subscriptions - matches Python api_get_person_subscriptions function exactly
pub async fn get_person_subscriptions(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only get their own subscriptions
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only retrieve your own subscriptions!"));
    }

    let subscriptions = state.db_pool.get_person_subscriptions(user_id).await?;
    Ok(Json(serde_json::json!({
        "subscriptions": subscriptions
    })))
}

// Get person episodes - matches Python api_return_person_episodes function exactly
pub async fn get_person_episodes(
    State(state): State<AppState>,
    Path((user_id, person_id)): Path<(i32, i32)>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only get their own subscriptions
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only retrieve your own person episodes!"));
    }

    let episodes = state.db_pool.get_person_episodes(user_id, person_id).await?;
    Ok(Json(serde_json::json!({
        "episodes": episodes
    })))
}

// Request struct for set_podcast_playback_speed - matches Python SetPlaybackSpeedPodcast model
#[derive(Deserialize)]
pub struct SetPlaybackSpeedPodcast {
    pub user_id: i32,
    pub podcast_id: i32,
    pub playback_speed: f64,
}

// Set podcast playback speed - matches Python api_set_podcast_playback_speed endpoint
pub async fn set_podcast_playback_speed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetPlaybackSpeedPodcast>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.set_podcast_playback_speed(request.user_id, request.podcast_id, request.playback_speed).await?;

    Ok(Json(serde_json::json!({ "detail": "Default podcast playback speed updated." })))
}

// Request struct for enable_auto_download - matches Python AutoDownloadRequest model
#[derive(Deserialize)]
pub struct AutoDownloadRequest {
    pub podcast_id: i32,
    pub auto_download: bool,
    pub user_id: i32,
}

// Enable/disable auto download for podcast - matches Python api_enable_auto_download endpoint
pub async fn enable_auto_download(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AutoDownloadRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id != request.user_id {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.enable_auto_download(request.podcast_id, request.auto_download, request.user_id).await?;

    Ok(Json(serde_json::json!({ "detail": "Auto-download status updated." })))
}

// Request struct for toggle_podcast_notifications - matches Python TogglePodcastNotificationData model
#[derive(Deserialize)]
pub struct TogglePodcastNotificationData {
    pub user_id: i32,
    pub podcast_id: i32,
    pub enabled: bool,
}

// Toggle podcast notifications - matches Python api_toggle_podcast_notifications endpoint
pub async fn toggle_podcast_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TogglePodcastNotificationData>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("Invalid API key"));
    }

    let success = state.db_pool.toggle_podcast_notifications(request.user_id, request.podcast_id, request.enabled).await?;

    if success {
        Ok(Json(serde_json::json!({ "detail": "Notification settings updated successfully" })))
    } else {
        Ok(Json(serde_json::json!({ "detail": "Failed to update notification settings" })))
    }
}

// Request struct for adjust_skip_times - matches Python SkipTimesRequest model
#[derive(Deserialize)]
pub struct SkipTimesRequest {
    pub podcast_id: i32,
    #[serde(default)]
    pub start_skip: i32,
    #[serde(default)]
    pub end_skip: i32,
    pub user_id: i32,
}

// Adjust skip times for podcast - matches Python api_adjust_skip_times endpoint
pub async fn adjust_skip_times(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SkipTimesRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.adjust_skip_times(request.podcast_id, request.start_skip, request.end_skip, request.user_id).await?;

    Ok(Json(serde_json::json!({ "detail": "Skip times updated." })))
}

// Request struct for remove_category - matches Python RemoveCategoryData model
#[derive(Deserialize)]
pub struct RemoveCategoryData {
    pub podcast_id: i32,
    pub user_id: i32,
    pub category: String,
}

// Remove category from podcast - matches Python api_remove_category endpoint
pub async fn remove_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RemoveCategoryData>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id != request.user_id {
        return Err(AppError::forbidden("You can only modify categories of your own podcasts!"));
    }

    state.db_pool.remove_category(request.podcast_id, request.user_id, &request.category).await?;

    Ok(Json(serde_json::json!({ "detail": "Category removed." })))
}

// Request struct for add_category - matches Python AddCategoryData model
#[derive(Deserialize)]
pub struct AddCategoryData {
    pub podcast_id: i32,
    pub user_id: i32,
    pub category: String,
}

// Add category to podcast - matches Python api_add_category endpoint
pub async fn add_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddCategoryData>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify categories of your own podcasts!"));
    }

    let result = state.db_pool.add_category(request.podcast_id, request.user_id, &request.category).await?;

    Ok(Json(serde_json::json!({ "detail": result })))
}

// Get user RSS key - matches Python get_user_rss_key endpoint
pub async fn get_user_rss_key(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if key_id == 0 {
        return Err(AppError::forbidden("Invalid API key"));
    }

    let rss_key = state.db_pool.get_user_rss_key(key_id).await?;
    if let Some(key) = rss_key {
        Ok(Json(serde_json::json!({ "rss_key": key })))
    } else {
        Err(AppError::not_found("No RSS key found. Please enable RSS feeds first."))
    }
}

#[derive(Deserialize)]
pub struct VerifyMfaRequest {
    pub user_id: i32,
    pub mfa_code: String,
}

// Verify MFA code - matches Python verify_mfa endpoint
pub async fn verify_mfa(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<VerifyMfaRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization
    if !check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only verify your own login code!"));
    }

    // Get the stored MFA secret
    let secret = state.db_pool.get_mfa_secret(request.user_id).await?;
    
    if let Some(secret_str) = secret {
        // Verify the TOTP code
        use totp_rs::{Algorithm, TOTP, Secret};
        
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            Secret::Encoded(secret_str.clone()).to_bytes().unwrap(),
            Some("Pinepods".to_string()),
            "login".to_string(),
        ).map_err(|e| AppError::internal(&format!("Failed to create TOTP: {}", e)))?;
        
        let is_valid = totp.check_current(&request.mfa_code)
            .map_err(|e| AppError::internal(&format!("Failed to verify TOTP: {}", e)))?;
        
        Ok(Json(serde_json::json!({ "verified": is_valid })))
    } else {
        Ok(Json(serde_json::json!({ "verified": false })))
    }
}

// Scheduled backup management
#[derive(Deserialize)]
pub struct ScheduleBackupRequest {
    pub user_id: i32,
    pub cron_schedule: String, // e.g., "0 2 * * *" for daily at 2 AM
    pub enabled: bool,
}

#[derive(Deserialize)]
pub struct GetScheduledBackupRequest {
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct ListBackupFilesRequest {
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct RestoreBackupFileRequest {
    pub user_id: i32,
    pub backup_filename: String,
}

// Schedule automatic backup - admin only
pub async fn schedule_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ScheduleBackupRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    // Validate cron expression using tokio-cron-scheduler
    use tokio_cron_scheduler::Job;
    if let Err(_) = Job::new(&request.cron_schedule, |_uuid, _lock| {}) {
        return Err(AppError::bad_request("Invalid cron schedule format"));
    }

    // Store the schedule in database
    state.db_pool.set_scheduled_backup(request.user_id, &request.cron_schedule, request.enabled).await?;

    Ok(Json(serde_json::json!({ 
        "detail": "Backup schedule updated successfully",
        "schedule": request.cron_schedule,
        "enabled": request.enabled
    })))
}

// Get scheduled backup settings - admin only
pub async fn get_scheduled_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<GetScheduledBackupRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    let schedule_info = state.db_pool.get_scheduled_backup(request.user_id).await?;
    
    Ok(Json(serde_json::json!(schedule_info)))
}

// List backup files in mounted backup directory - admin only
pub async fn list_backup_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ListBackupFilesRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    use std::fs;
    
    let backup_dir = "/opt/pinepods/backups";
    let backup_files = match fs::read_dir(backup_dir) {
        Ok(entries) => {
            let mut files = Vec::new();
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |ext| ext == "sql") {
                        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                            let metadata = entry.metadata().ok();
                            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                            let modified = metadata.as_ref()
                                .and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            
                            files.push(serde_json::json!({
                                "filename": filename,
                                "size": size,
                                "modified": modified
                            }));
                        }
                    }
                }
            }
            files.sort_by(|a, b| {
                let a_modified = a["modified"].as_u64().unwrap_or(0);
                let b_modified = b["modified"].as_u64().unwrap_or(0);
                b_modified.cmp(&a_modified) // Sort by modified date desc (newest first)
            });
            files
        }
        Err(_) => {
            return Err(AppError::internal("Failed to read backup directory"));
        }
    };

    Ok(Json(serde_json::json!({
        "backup_files": backup_files
    })))
}

// Restore from backup file in mounted directory - admin only
pub async fn restore_from_backup_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RestoreBackupFileRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    // Validate filename to prevent path traversal
    let backup_filename = request.backup_filename.clone();
    if backup_filename.contains("..") || backup_filename.contains("/") || !backup_filename.ends_with(".sql") {
        return Err(AppError::bad_request("Invalid backup filename"));
    }

    let backup_path = format!("/opt/pinepods/backups/{}", backup_filename);
    
    // Check if file exists
    if !std::path::Path::new(&backup_path).exists() {
        return Err(AppError::not_found("Backup file not found"));
    }

    // Clone for the async closure
    let backup_filename_for_closure = backup_filename.clone();

    // Spawn restoration task
    let task_id = state.task_spawner.spawn_progress_task(
        "restore_from_backup_file".to_string(),
        0, // System user
        move |reporter| {
            let backup_path = backup_path.clone();
            let backup_filename = backup_filename_for_closure;
            async move {
                reporter.update_progress(10.0, Some("Starting restoration from backup file...".to_string())).await?;
                
                // Get database password from environment
                let db_password = std::env::var("DB_PASSWORD")
                    .map_err(|_| AppError::internal("Database password not found in environment"))?;
                
                reporter.update_progress(50.0, Some("Restoring database...".to_string())).await?;

                // Execute restoration based on database type
                use tokio::process::Command;
                let db_type = std::env::var("DB_TYPE").unwrap_or_else(|_| "postgresql".to_string());
                let db_host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
                let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods_database".to_string());
                
                let output = if db_type.to_lowercase().contains("mysql") || db_type.to_lowercase().contains("mariadb") {
                    let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "3306".to_string());
                    let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "root".to_string());
                    
                    let mut cmd = Command::new("mysql");
                    cmd.arg("-h").arg(&db_host)
                       .arg("-P").arg(&db_port)
                       .arg("-u").arg(&db_user)
                       .arg(&format!("-p{}", db_password))
                       .arg("--ssl-verify-server-cert=0")
                       .arg(&db_name);
                    
                    // For MySQL, we need to pipe the file content to stdin
                    cmd.stdin(std::process::Stdio::piped());
                    let mut child = cmd.spawn()
                        .map_err(|e| AppError::internal(&format!("Failed to execute mysql: {}", e)))?;
                    
                    // Read the backup file and send to mysql stdin
                    let backup_content = tokio::fs::read_to_string(&backup_path).await
                        .map_err(|e| AppError::internal(&format!("Failed to read backup file: {}", e)))?;
                    
                    if let Some(stdin) = child.stdin.as_mut() {
                        use tokio::io::AsyncWriteExt;
                        stdin.write_all(backup_content.as_bytes()).await
                            .map_err(|e| AppError::internal(&format!("Failed to write to mysql stdin: {}", e)))?;
                    }
                    
                    child.wait_with_output().await
                        .map_err(|e| AppError::internal(&format!("Failed to wait for mysql: {}", e)))?
                } else {
                    // PostgreSQL
                    let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
                    let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
                    
                    let mut cmd = Command::new("psql");
                    cmd.arg("-h").arg(&db_host)
                       .arg("-p").arg(&db_port)
                       .arg("-U").arg(&db_user)
                       .arg("-d").arg(&db_name)
                       .arg("-f").arg(&backup_path)
                       .env("PGPASSWORD", &db_password);
                    
                    cmd.output().await
                        .map_err(|e| AppError::internal(&format!("Failed to execute psql: {}", e)))?
                };

                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    return Err(AppError::internal(&format!("Restore failed: {}", error_msg)));
                }

                reporter.update_progress(100.0, Some("Restoration completed successfully".to_string())).await?;
                
                Ok(serde_json::json!({
                    "status": "Restoration completed successfully",
                    "backup_file": backup_filename
                }))
            }
        }
    ).await?;

    Ok(Json(serde_json::json!({
        "detail": "Restoration started",
        "task_id": task_id
    })))
}

// Request struct for manual backup to directory
#[derive(Deserialize)]
pub struct ManualBackupRequest {
    pub user_id: i32,
}

// Manual backup to directory - admin only
pub async fn manual_backup_to_directory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ManualBackupRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check if user is admin
    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    // Generate filename with timestamp
    let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
    let backup_filename = format!("manual_backup_{}.sql", timestamp);
    let backup_path = format!("/opt/pinepods/backups/{}", backup_filename);

    // Ensure backup directory exists
    if let Err(e) = std::fs::create_dir_all("/opt/pinepods/backups") {
        return Err(AppError::internal(&format!("Failed to create backup directory: {}", e)));
    }
    
    // Set ownership using PUID/PGID environment variables
    let puid: u32 = std::env::var("PUID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
    let pgid: u32 = std::env::var("PGID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
    
    // Set directory ownership (ignore errors for NFS mounts)
    let _ = std::process::Command::new("chown")
        .args(&[format!("{}:{}", puid, pgid), "/opt/pinepods/backups".to_string()])
        .output();

    // Clone for the async closure
    let backup_filename_for_closure = backup_filename.clone();

    // Spawn backup task
    let task_id = state.task_spawner.spawn_progress_task(
        "manual_backup_to_directory".to_string(),
        0, // System user
        move |reporter| {
            let backup_path = backup_path.clone();
            let backup_filename = backup_filename_for_closure;
            async move {
                reporter.update_progress(10.0, Some("Starting manual backup...".to_string())).await?;
                
                // Get database credentials from environment
                let db_type = std::env::var("DB_TYPE").unwrap_or_else(|_| "postgresql".to_string());
                let db_host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
                let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods_database".to_string());
                let db_password = std::env::var("DB_PASSWORD")
                    .map_err(|_| AppError::internal("Database password not found in environment"))?;
                
                reporter.update_progress(30.0, Some("Creating database backup...".to_string())).await?;
                
                // Use appropriate backup command based on database type
                let output = if db_type.to_lowercase().contains("mysql") || db_type.to_lowercase().contains("mariadb") {
                    let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "3306".to_string());
                    let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "root".to_string());
                    
                    tokio::process::Command::new("mysqldump")
                        .args(&[
                            "-h", &db_host,
                            "-P", &db_port,
                            "-u", &db_user,
                            &format!("-p{}", db_password),
                            "--single-transaction",
                            "--routines",
                            "--triggers",
                            "--ssl-verify-server-cert=0",
                            "--result-file", &backup_path,
                            &db_name
                        ])
                        .output()
                        .await
                        .map_err(|e| AppError::internal(&format!("Failed to execute mysqldump: {}", e)))?
                } else {
                    // PostgreSQL
                    let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
                    let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
                    
                    tokio::process::Command::new("pg_dump")
                        .env("PGPASSWORD", db_password)
                        .args(&[
                            "-h", &db_host,
                            "-p", &db_port,
                            "-U", &db_user,
                            "-d", &db_name,
                            "--clean",
                            "--if-exists",
                            "--no-owner",
                            "--no-privileges",
                            "-f", &backup_path
                        ])
                        .output()
                        .await
                        .map_err(|e| AppError::internal(&format!("Failed to execute pg_dump: {}", e)))?
                };

                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    return Err(AppError::internal(&format!("Backup failed: {}", error_msg)));
                }

                reporter.update_progress(90.0, Some("Finalizing backup...".to_string())).await?;

                // Set file ownership using PUID/PGID environment variables
                let puid: u32 = std::env::var("PUID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
                let pgid: u32 = std::env::var("PGID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
                
                // Set backup file ownership (ignore errors for NFS mounts)
                let _ = std::process::Command::new("chown")
                    .args(&[format!("{}:{}", puid, pgid), backup_path.clone()])
                    .output();

                // Check if backup file was created and get its size
                let backup_info = match std::fs::metadata(&backup_path) {
                    Ok(metadata) => serde_json::json!({
                        "filename": backup_filename,
                        "size": metadata.len(),
                        "path": backup_path
                    }),
                    Err(_) => {
                        return Err(AppError::internal("Backup file was not created"));
                    }
                };

                reporter.update_progress(100.0, Some("Manual backup completed successfully".to_string())).await?;
                
                Ok(serde_json::json!({
                    "status": "Manual backup completed successfully",
                    "backup_info": backup_info
                }))
            }
        }
    ).await?;

    Ok(Json(serde_json::json!({
        "detail": "Manual backup started",
        "task_id": task_id,
        "filename": backup_filename
    })))
}

// Request for getting podcasts with podcast_index_id = 0
#[derive(Deserialize)]
pub struct GetUnmatchedPodcastsRequest {
    pub user_id: i32,
}

// Get podcasts that have podcast_index_id = 0 (imported via OPML without podcast index match)
pub async fn get_unmatched_podcasts(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<GetUnmatchedPodcastsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check if it's web key or user's own key
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == request.user_id || is_web_key {
        let podcasts = state.db_pool.get_unmatched_podcasts(request.user_id).await?;
        Ok(Json(serde_json::json!({"podcasts": podcasts})))
    } else {
        Err(AppError::forbidden("You can only access your own podcasts"))
    }
}

// Request for updating podcast index ID
#[derive(Deserialize)]
pub struct UpdatePodcastIndexIdRequest {
    pub user_id: i32,
    pub podcast_id: i32,
    pub podcast_index_id: i32,
}

// Update a podcast's podcast_index_id
pub async fn update_podcast_index_id(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<UpdatePodcastIndexIdRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check if it's web key or user's own key
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == request.user_id || is_web_key {
        state.db_pool.update_podcast_index_id(
            request.user_id,
            request.podcast_id,
            request.podcast_index_id
        ).await?;
        
        Ok(Json(serde_json::json!({
            "detail": "Podcast index ID updated successfully"
        })))
    } else {
        Err(AppError::forbidden("You can only update your own podcasts"))
    }
}

// Request for ignoring a podcast index ID
#[derive(Deserialize)]
pub struct IgnorePodcastIndexIdRequest {
    pub user_id: i32,
    pub podcast_id: i32,
    pub ignore: bool,
}

#[derive(Deserialize)]
pub struct GetIgnoredPodcastsRequest {
    pub user_id: i32,
}

// Ignore/unignore a podcast's index ID requirement
pub async fn ignore_podcast_index_id(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<IgnorePodcastIndexIdRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check if it's web key or user's own key
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == request.user_id || is_web_key {
        state.db_pool.ignore_podcast_index_id(
            request.user_id,
            request.podcast_id,
            request.ignore
        ).await?;
        
        let action = if request.ignore { "ignored" } else { "unignored" };
        Ok(Json(serde_json::json!({
            "detail": format!("Podcast index ID requirement {}", action)
        })))
    } else {
        Err(AppError::forbidden("You can only update your own podcasts"))
    }
}

// Get podcasts that are ignored from podcast index matching
pub async fn get_ignored_podcasts(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(request): Json<GetIgnoredPodcastsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check if it's web key or user's own key
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id == request.user_id || is_web_key {
        let podcasts = state.db_pool.get_ignored_podcasts(request.user_id).await?;
        
        Ok(Json(serde_json::json!({
            "podcasts": podcasts
        })))
    } else {
        Err(AppError::forbidden("You can only view your own podcasts"))
    }
}

// Get user's language preference
pub async fn get_user_language(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<UserLanguageResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let user_id: i32 = params
        .get("user_id")
        .ok_or_else(|| AppError::bad_request("Missing user_id parameter"))?
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user_id format"))?;
    
    check_user_access(&state, &api_key, user_id).await?;
    
    let language = state.db_pool.get_user_language(user_id).await?;
    
    Ok(Json(UserLanguageResponse { language }))
}

// Update user's language preference
pub async fn update_user_language(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LanguageUpdateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    check_user_access(&state, &api_key, request.user_id).await?;
    
    let success = state.db_pool.update_user_language(request.user_id, &request.language).await?;
    
    if success {
        Ok(Json(serde_json::json!({
            "success": true,
            "language": request.language
        })))
    } else {
        Err(AppError::not_found("User not found"))
    }
}

// Get available languages by scanning translation files
pub async fn get_available_languages() -> Result<Json<AvailableLanguagesResponse>, AppError> {
    let translations_dir = std::path::Path::new("/var/www/html/static/translations");
    
    let mut languages = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(translations_dir) {
        for entry in entries.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.ends_with(".json") {
                    let lang_code = file_name.strip_suffix(".json").unwrap_or("");
                    
                    // Map language codes to human-readable names
                    let lang_name = match lang_code {
                        "en" => "English",
                        "ar" => "العربية",
                        "be" => "Беларуская",
                        "bg" => "Български",
                        "bn" => "বাংলা",
                        "ca" => "Català",
                        "cs" => "Čeština",
                        "da" => "Dansk",
                        "de" => "Deutsch",
                        "es" => "Español",
                        "et" => "Eesti",
                        "eu" => "Euskera",
                        "fa" => "فارسی",
                        "fi" => "Suomi",
                        "fr" => "Français",
                        "gu" => "ગુજરાતી",
                        "he" => "עברית",
                        "hi" => "हिन्दी",
                        "hr" => "Hrvatski",
                        "hu" => "Magyar",
                        "it" => "Italiano",
                        "ja" => "日本語",
                        "ko" => "한국어",
                        "lt" => "Lietuvių",
                        "nb" => "Norsk Bokmål",
                        "nl" => "Nederlands",
                        "pl" => "Polski",
                        "pt" => "Português",
                        "pt-BR" => "Português (Brasil)",
                        "ro" => "Română",
                        "ru" => "Русский",
                        "sk" => "Slovenčina",
                        "sl" => "Slovenščina",
                        "sv" => "Svenska",
                        "tr" => "Türkçe",
                        "uk" => "Українська",
                        "vi" => "Tiếng Việt",
                        "zh" => "中文",
                        "zh-Hans" => "中文 (简体)",
                        "zh-Hant" => "中文 (繁體)",
                        "test" => "Test Language",
                        _ => lang_code, // Fallback to code if name not mapped
                    };
                    
                    // Validate that the translation file contains valid JSON
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if serde_json::from_str::<serde_json::Value>(&content).is_ok() {
                            languages.push(AvailableLanguage {
                                code: lang_code.to_string(),
                                name: lang_name.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }
    
    // Sort by language code for consistent ordering
    languages.sort_by(|a, b| a.code.cmp(&b.code));
    
    // Ensure English is always first if present
    if let Some(en_index) = languages.iter().position(|l| l.code == "en") {
        if en_index != 0 {
            let en_lang = languages.remove(en_index);
            languages.insert(0, en_lang);
        }
    }
    
    Ok(Json(AvailableLanguagesResponse { languages }))
}

// Get server default language (no authentication required)
pub async fn get_server_default_language() -> Result<Json<serde_json::Value>, AppError> {
    // Get default language from environment variable, fallback to 'en'
    let default_language = std::env::var("DEFAULT_LANGUAGE").unwrap_or_else(|_| "en".to_string());
    
    // Validate language code (basic validation)
    let default_language = if default_language.len() > 10 || default_language.is_empty() {
        "en"
    } else {
        &default_language
    };
    
    Ok(Json(serde_json::json!({
        "default_language": default_language
    })))
}

// Request struct for set_global_podcast_cover_preference - matches playback speed pattern
#[derive(Deserialize)]
pub struct SetGlobalPodcastCoverPreference {
    pub user_id: i32,
    pub use_podcast_covers: bool,
    pub podcast_id: Option<i32>,
}

// Set global podcast cover preference - matches Python api_set_global_podcast_cover_preference function
pub async fn set_global_podcast_cover_preference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetGlobalPodcastCoverPreference>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only set their own preference
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own settings."));
    }

    // If podcast_id is provided, set per-podcast preference; otherwise set global preference
    if let Some(podcast_id) = request.podcast_id {
        state.db_pool.set_podcast_cover_preference(request.user_id, podcast_id, request.use_podcast_covers).await?;
        Ok(Json(serde_json::json!({ "detail": "Podcast cover preference updated." })))
    } else {
        state.db_pool.set_global_podcast_cover_preference(request.user_id, request.use_podcast_covers).await?;
        Ok(Json(serde_json::json!({ "detail": "Global podcast cover preference updated." })))
    }
}

// Request struct for set_podcast_cover_preference - matches podcast playback speed pattern
#[derive(Deserialize)]
pub struct SetPodcastCoverPreference {
    pub user_id: i32,
    pub podcast_id: i32,
    pub use_podcast_covers: bool,
}

// Set podcast cover preference - matches Python api_set_podcast_cover_preference function
pub async fn set_podcast_cover_preference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetPodcastCoverPreference>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.set_podcast_cover_preference(request.user_id, request.podcast_id, request.use_podcast_covers).await?;

    Ok(Json(serde_json::json!({ "detail": "Podcast cover preference updated." })))
}

// Request struct for clear_podcast_cover_preference - matches clear playback speed pattern
#[derive(Deserialize)]
pub struct ClearPodcastCoverPreference {
    pub user_id: i32,
    pub podcast_id: i32,
}

// Clear podcast cover preference - matches Python api_clear_podcast_cover_preference function
pub async fn clear_podcast_cover_preference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClearPodcastCoverPreference>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.clear_podcast_cover_preference(request.user_id, request.podcast_id).await?;

    Ok(Json(serde_json::json!({ "detail": "Podcast cover preference cleared." })))
}

// Get global podcast cover preference
pub async fn get_global_podcast_cover_preference(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let user_id: i32 = params
        .get("user_id")
        .ok_or_else(|| AppError::bad_request("Missing user_id parameter"))?
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user_id format"))?;
        
    // Check authorization - users can only access their own settings
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if user_id_from_api_key != user_id {
        return Err(AppError::forbidden("You can only access your own settings."));
    }

    // If podcast_id is provided, get per-podcast preference; otherwise get global preference
    let use_podcast_covers = if let Some(podcast_id_str) = params.get("podcast_id") {
        let podcast_id: i32 = podcast_id_str
            .parse()
            .map_err(|_| AppError::bad_request("Invalid podcast_id format"))?;
            
        let per_podcast_preference = state.db_pool.get_podcast_cover_preference(user_id, podcast_id).await?;
        
        // If no per-podcast preference is set, fall back to global preference
        match per_podcast_preference {
            Some(preference) => preference,
            None => state.db_pool.get_global_podcast_cover_preference(user_id).await?,
        }
    } else {
        state.db_pool.get_global_podcast_cover_preference(user_id).await?
    };

    Ok(Json(serde_json::json!({
        "use_podcast_covers": use_podcast_covers
    })))
}

