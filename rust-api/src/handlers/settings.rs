use axum::{
    extract::{Path, Query, State, Multipart, Json},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key, check_user_access},
    AppState,
};
use std::collections::HashMap;
use sqlx::{Row, Column, ValueRef};

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
    Ok(Json(serde_json::json!({ "message": "Email settings saved." })))
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

    // Create email message
    let email = Message::builder()
        .from(request.from_email.parse()
            .map_err(|_| AppError::bad_request("Invalid from email"))?)
        .to(request.to_email.parse()
            .map_err(|_| AppError::bad_request("Invalid to email"))?)
        .subject("Test Email")
        .header(ContentType::TEXT_PLAIN)
        .body(request.message.clone())
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
        Ok(Err(e)) => Err(AppError::internal(&format!("Failed to send email: {}", e))),
        Err(_) => Err(AppError::internal("Email sending timed out after 30 seconds. Please check your SMTP server settings.".to_string())),
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
async fn send_email_with_settings(
    settings: &EmailSettingsResponse,
    request: &SendEmailRequest,
) -> Result<String, AppError> {
    use lettre::{
        message::{header::ContentType, Message},
        transport::smtp::{authentication::Credentials, client::Tls, client::TlsParameters},
        AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    };
    use tokio::time::{timeout, Duration};

    // Create email message
    let email = Message::builder()
        .from(settings.from_email.parse()
            .map_err(|_| AppError::bad_request("Invalid from email in settings"))?)
        .to(request.to_email.parse()
            .map_err(|_| AppError::bad_request("Invalid to email"))?)
        .subject(&request.subject)
        .header(ContentType::TEXT_PLAIN)
        .body(request.message.clone())
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
        Ok(Err(e)) => Err(AppError::internal(&format!("Failed to send email: {}", e))),
        Err(_) => Err(AppError::internal("Email sending timed out after 30 seconds. Please check your SMTP server settings.".to_string())),
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
               .arg("--single-transaction")
               .arg("--no-create-info")
               .arg("--disable-keys")
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

    let stream = ReaderStream::new(stdout);
    let body = Body::from_stream(stream);

    // Spawn a task to wait for the process and handle errors
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) if status.success() => {
                println!("Backup process completed successfully");
            }
            Ok(status) => {
                println!("Backup process failed with status: {}", status);
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

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let login_data = state.db_pool.initiate_nextcloud_login(user_id, &request.nextcloud_url).await?;
    
    Ok(Json(serde_json::json!({
        "login": login_data.login_url,
        "token": login_data.token
    })))
}

// Request struct for add_nextcloud_server
#[derive(Deserialize)]
pub struct AddNextcloudServerRequest {
    pub nextcloud_url: String,
    pub token: String,
}

// Add Nextcloud server - matches Python add_nextcloud_server function exactly
pub async fn add_nextcloud_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddNextcloudServerRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let success = state.db_pool.add_nextcloud_server(user_id, &request.nextcloud_url, &request.token).await?;
    
    if success {
        Ok(Json(serde_json::json!({ "status": "success" })))
    } else {
        Err(AppError::internal("Failed to add Nextcloud server"))
    }
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

    let verified = state.db_pool.verify_gpodder_auth(&request.gpodder_url, &request.gpodder_username, &request.gpodder_password).await?;
    Ok(Json(serde_json::json!({ "verified": verified })))
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
        Some(settings) => Ok(Json(settings)),
        None => Err(AppError::not_found("gPodder settings not found")),
    }
}

// Check gPodder settings - matches Python check_gpodder_settings function exactly
pub async fn check_gpodder_settings(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<bool>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or own user
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if user_id != user_id_from_api_key && !is_web_key {
        return Err(AppError::forbidden("You can only check your own gPodder settings!"));
    }

    let has_settings = state.db_pool.check_gpodder_settings(user_id).await?;
    Ok(Json(has_settings))
}


// Remove podcast sync - matches Python remove_podcast_sync function exactly
pub async fn remove_podcast_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let success = state.db_pool.remove_podcast_sync(user_id).await?;
    
    if success {
        Ok(Json(serde_json::json!({ "status": "success" })))
    } else {
        Err(AppError::internal("Failed to remove podcast sync"))
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
        30
    ).await?;

    // Get complete podcast details for response
    let podcast_details = state.db_pool.get_podcast_details(request.user_id, podcast_id).await?;

    Ok(Json(serde_json::json!({ "data": podcast_details })))
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
                        30  // feed_cutoff
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
        request.gotify_token.as_deref()
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
    let settings_json = serde_json::to_value(&settings)?;
    let success = state.notification_manager.send_test_notification(request.user_id, &request.platform, &settings_json).await?;
    
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
        request.admin_role.as_deref().unwrap_or("")
    ).await?;
    Ok(Json(serde_json::json!({ "provider_id": provider_id })))
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

    Ok(Json(serde_json::json!(success)))
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

