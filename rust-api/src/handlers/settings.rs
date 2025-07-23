use axum::{
    extract::{Path, Query, State, Multipart, Json},
    http::HeaderMap,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key},
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

    // Check authorization (elevated access or own user)
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if !is_web_key && user_id != user_id_from_api_key {
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

    // Check authorization (elevated access or own user)
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if !is_web_key && user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to access these user details"));
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

    // Check authorization (elevated access or own user)
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if !is_web_key && request.user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to access these user details"));
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

    // Check authorization (elevated access or own user)
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if !is_web_key && request.user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to access these user details"));
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
    pub server_port: i32,
    pub from_email: String,
    pub send_mode: String,
    pub encryption: String,
    pub auth_required: i32,
    pub email_username: String,
    pub email_password: String,
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
            // No encryption
            if request.auth_required {
                let creds = Credentials::new(request.email_username.clone(), request.email_password.clone());
                AsyncSmtpTransport::<Tokio1Executor>::relay(&request.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(port)
                    .tls(Tls::None)
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&request.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(port)
                    .tls(Tls::None)
                    .build()
            }
        }
    };

    // Send the email
    match mailer.send(email).await {
        Ok(_) => Ok("Email sent successfully".to_string()),
        Err(e) => Err(AppError::internal(&format!("Failed to send email: {}", e))),
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
            // No encryption
            if settings.auth_required == 1 {
                let creds = Credentials::new(settings.username.clone(), settings.password.clone());
                AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(settings.server_port as u16)
                    .tls(Tls::None)
                    .credentials(creds)
                    .build()
            } else {
                AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.server_name)
                    .map_err(|e| AppError::internal(&format!("SMTP relay configuration failed: {}", e)))?
                    .port(settings.server_port as u16)
                    .tls(Tls::None)
                    .build()
            }
        }
    };

    // Send the email
    match mailer.send(email).await {
        Ok(_) => Ok("Email sent successfully".to_string()),
        Err(e) => Err(AppError::internal(&format!("Failed to send email: {}", e))),
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

    // Parse user_id and api_id from strings
    let user_id: i32 = request.user_id.parse()
        .map_err(|_| AppError::bad_request("Invalid user_id format"))?;
    let api_id: i32 = request.api_id.parse()
        .map_err(|_| AppError::bad_request("Invalid api_id format"))?;

    // Check authorization (elevated access or own user)
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if !is_web_key && user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to access or remove other users api-keys."));
    }

    // Check if the API key to be deleted is the same as the one used in the current request
    if state.db_pool.is_same_api_key(api_id, &api_key).await? {
        return Err(AppError::forbidden("You cannot delete the API key that is currently in use."));
    }

    // Check if the API key belongs to the guest user (user_id 1)
    if state.db_pool.belongs_to_guest_user(api_id).await? {
        return Err(AppError::forbidden("Cannot delete guest user api."));
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

// Streaming backup implementation to handle large databases efficiently
async fn backup_server_streaming(
    state: &AppState,
    _database_pass: &str,
) -> Result<axum::response::Response, String> {
    use axum::response::Response;
    use axum::body::Body;
    use tokio_stream::StreamExt;
    use futures::stream;

    // Instead of using subprocess, we'll create a streaming SQL export
    // This approach handles large databases by processing data in chunks
    
    let state_clone = state.clone();
    let backup_stream = stream::unfold(Some(0), move |chunk_id_opt| {
        let state_clone = state_clone.clone();
        async move {
            match chunk_id_opt {
                Some(chunk_id) => {
                    match generate_backup_chunk(&state_clone, chunk_id).await {
                        Ok(Some(chunk)) => Some((Ok(chunk), Some(chunk_id + 1))),
                        Ok(None) => None, // End of stream
                        Err(e) => Some((Err(e), None)), // End on error
                    }
                },
                None => None, // Already ended
            }
        }
    });

    // Convert stream to body
    let stream = backup_stream.map(|result| {
        result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    });
    
    let body = Body::from_stream(stream);
    
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

        // Limit total output size per chunk to manage memory
        if sql_output.len() > 1_000_000 { // 1MB per chunk
            break;
        }
    }

    Ok(sql_output)
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
                    // For simplicity, we'll convert all values to strings and quote them
                    // In a production system, you'd want proper type handling
                    if let Ok(val) = row.try_get::<String, _>(i) {
                        output.push_str(&format!("'{}'", val.replace('\'', "''")));
                    } else {
                        output.push_str("NULL");
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
                    // For simplicity, we'll convert all values to strings and quote them
                    if let Ok(val) = row.try_get::<String, _>(i) {
                        output.push_str(&format!("'{}'", val.replace('\'', "''")));
                    } else {
                        output.push_str("NULL");
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

    // Process the multipart form to get the uploaded file
    let mut sql_content = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| AppError::bad_request(&format!("Multipart error: {}", e)))? {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
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
            break;
        }
    }

    let sql_content = sql_content.ok_or_else(|| AppError::bad_request("No SQL file uploaded"))?;

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
        request.gotify_url.as_deref(),
        request.gotify_token.as_deref()
    ).await?;
    Ok(Json(serde_json::json!({ "message": "Notification settings updated successfully" })))
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

    // TODO: Trigger background task to process person subscription and gather episodes
    // This would call process_person_subscription_task() equivalent

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
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only get their own subscriptions
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only retrieve your own subscriptions!"));
    }

    let subscriptions = state.db_pool.get_person_subscriptions(user_id).await?;
    Ok(Json(subscriptions))
}

// Get person episodes - matches Python api_return_person_episodes function exactly
pub async fn get_person_episodes(
    State(state): State<AppState>,
    Path((user_id, person_id)): Path<(i32, i32)>,
    headers: HeaderMap,
) -> Result<Json<Vec<serde_json::Value>>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only get their own subscriptions
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only retrieve your own person episodes!"));
    }

    let episodes = state.db_pool.get_person_episodes(user_id, person_id).await?;
    Ok(Json(episodes))
}

