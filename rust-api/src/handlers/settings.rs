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
use tracing::{debug, error, info, warn};

// Request struct for set_theme
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetThemeRequest {
    pub user_id: i32,
    pub new_theme: String,
}

// Request struct for set_playback_speed - matches Python SetPlaybackSpeedUser model exactly
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetPlaybackSpeedUser {
    pub user_id: i32,
    pub playback_speed: f64,
}

// Set user theme - matches Python api_set_theme function exactly
#[utoipa::path(
    put,
    path = "/user/set_theme",
    tag = "settings",
    summary = "Set theme",
    request_body = SetThemeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/user/set_playback_speed",
    tag = "settings",
    summary = "Set playback speed user",
    request_body = SetPlaybackSpeedUser,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

// Request struct for set_default_volume_user (#828)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetDefaultVolumeUser {
    pub user_id: i32,
    pub volume: i32,
}

// Set per-user default playback volume (0-100) (#828) - mirrors set_playback_speed_user
#[utoipa::path(
    post,
    path = "/user/set_default_volume",
    tag = "settings",
    summary = "Set default volume (user default)",
    request_body = SetDefaultVolumeUser,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn set_default_volume_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetDefaultVolumeUser>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only set their own default volume
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own settings."));
    }

    let volume = request.volume.clamp(0, 100);
    state.db_pool.set_default_volume(request.user_id, volume).await?;

    Ok(Json(serde_json::json!({ "detail": "Default volume updated." })))
}

// Request struct for set_auto_download_delete_days_user (#655)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetAutoDownloadDeleteDaysUser {
    pub user_id: i32,
    pub days: i32,
}

// Set per-user default server-download retention window (#655)
#[utoipa::path(
    post,
    path = "/user/set_auto_download_delete_days",
    tag = "settings",
    summary = "Set auto-delete downloads days (user default)",
    request_body = SetAutoDownloadDeleteDaysUser,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn set_auto_download_delete_days_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetAutoDownloadDeleteDaysUser>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own settings
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own settings."));
    }

    let days = request.days.max(0);
    state.db_pool.set_auto_download_delete_days_user(request.user_id, days).await?;

    Ok(Json(serde_json::json!({ "detail": "Default auto-delete downloads setting updated." })))
}

// User info response struct
#[derive(Serialize, utoipa::ToSchema)]
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
#[utoipa::path(
    get,
    path = "/get_user_info",
    tag = "settings",
    summary = "Get user info",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = Vec<UserInfo>),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/my_user_info/{user_id}",
    tag = "settings",
    summary = "Get my user info",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddUserRequest {
    pub fullname: String,
    pub username: String,
    pub email: String,
    pub hash_pw: String,
}

// Add user - matches Python api_add_user function exactly (admin only)
#[utoipa::path(
    post,
    path = "/add_user",
    tag = "settings",
    summary = "Add user",
    request_body = AddUserRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/add_login_user",
    tag = "settings",
    summary = "Add login user",
    request_body = AddUserRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    put,
    path = "/set_fullname/{user_id}",
    tag = "settings",
    summary = "Set fullname",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct PasswordUpdateRequest {
    pub hash_pw: String,
}

// Set password - matches Python api_set_password function exactly
#[utoipa::path(
    put,
    path = "/set_password/{user_id}",
    tag = "settings",
    summary = "Set password",
    params(("user_id" = i32, Path)),
    request_body = PasswordUpdateRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    delete,
    path = "/user/delete/{user_id}",
    tag = "settings",
    summary = "Delete user",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetEmailRequest {
    pub user_id: i32,
    pub new_email: String,
}

// Set email - matches Python api_set_email function exactly
#[utoipa::path(
    put,
    path = "/user/set_email",
    tag = "settings",
    summary = "Set email",
    request_body = SetEmailRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetUsernameRequest {
    pub user_id: i32,
    pub new_username: String,
}

// Set username - matches Python api_set_username function exactly
#[utoipa::path(
    put,
    path = "/user/set_username",
    tag = "settings",
    summary = "Set username",
    request_body = SetUsernameRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetIsAdminRequest {
    pub user_id: i32,
    pub isadmin: bool,
}

// Set isadmin - matches Python api_set_isadmin function exactly (admin only)
#[utoipa::path(
    put,
    path = "/user/set_isadmin",
    tag = "settings",
    summary = "Set isadmin",
    request_body = SetIsAdminRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/user/final_admin/{user_id}",
    tag = "settings",
    summary = "Final admin",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/enable_disable_guest",
    tag = "settings",
    summary = "Enable disable guest",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/enable_disable_downloads",
    tag = "settings",
    summary = "Enable disable downloads",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/enable_disable_self_service",
    tag = "settings",
    summary = "Enable disable self service",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/guest_status",
    tag = "settings",
    summary = "Guest status",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = bool),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/rss_feed_status",
    tag = "settings",
    summary = "Rss feed status",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = bool),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/toggle_rss_feeds",
    tag = "settings",
    summary = "Toggle rss feeds",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/download_status",
    tag = "settings",
    summary = "Download status",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = bool),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn download_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<bool>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let result = state.db_pool.download_status().await?;
    Ok(Json(result))
}

// Admin download-metadata settings payload (#451/#533/#658). Used for both the
// GET response and the POST request body.
#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct DownloadMetadataSettings {
    pub folder_cover: bool,
    pub episode_cover: bool,
    pub metadata_sidecar: bool,
    pub metadata_format: String,
    pub metadata_subfolder: bool,
}

impl From<crate::services::download_metadata::DownloadSettings> for DownloadMetadataSettings {
    fn from(s: crate::services::download_metadata::DownloadSettings) -> Self {
        Self {
            folder_cover: s.folder_cover,
            episode_cover: s.episode_cover,
            metadata_sidecar: s.metadata_sidecar,
            metadata_format: s.metadata_format,
            metadata_subfolder: s.metadata_subfolder,
        }
    }
}

impl From<DownloadMetadataSettings> for crate::services::download_metadata::DownloadSettings {
    fn from(s: DownloadMetadataSettings) -> Self {
        Self {
            folder_cover: s.folder_cover,
            episode_cover: s.episode_cover,
            metadata_sidecar: s.metadata_sidecar,
            metadata_format: s.metadata_format,
            metadata_subfolder: s.metadata_subfolder,
        }
    }
}

// Get the admin download-metadata settings (#451/#533/#658).
#[utoipa::path(
    get,
    path = "/download_metadata_settings",
    tag = "settings",
    summary = "Get download metadata settings",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = DownloadMetadataSettings),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_download_metadata_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<DownloadMetadataSettings>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let settings = state.db_pool.get_download_settings().await?;
    Ok(Json(settings.into()))
}

// Update the admin download-metadata settings (#451/#533/#658). Admin only.
#[utoipa::path(
    post,
    path = "/download_metadata_settings",
    tag = "settings",
    summary = "Set download metadata settings",
    request_body = DownloadMetadataSettings,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn set_download_metadata_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<DownloadMetadataSettings>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let requesting_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_admin = state.db_pool.user_admin_check(requesting_user_id).await?;
    if !is_admin {
        return Err(AppError::forbidden("Admin access required"));
    }

    state.db_pool.set_download_settings(&payload.into()).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}

// Get self service status - matches Python api_self_service_status function exactly
#[utoipa::path(
    get,
    path = "/admin_self_service_status",
    tag = "settings",
    summary = "Self service status",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SaveEmailSettingsRequest {
    pub email_settings: EmailSettings,
}

#[derive(Deserialize, utoipa::ToSchema)]
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
#[utoipa::path(
    post,
    path = "/save_email_settings",
    tag = "settings",
    summary = "Save email settings",
    request_body = SaveEmailSettingsRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Serialize, utoipa::ToSchema)]
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
#[utoipa::path(
    get,
    path = "/get_email_settings",
    tag = "settings",
    summary = "Get email settings",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = EmailSettingsResponse),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SendTestEmailRequest {
    pub server_name: String,
    pub server_port: String,
    pub from_email: String,
    pub encryption: String,
    pub auth_required: bool,
    pub email_username: String,
    pub email_password: String,
    pub to_email: String,
    pub message: String,
}

// Send test email - matches Python api_send_email function exactly (admin only)
#[utoipa::path(
    post,
    path = "/send_test_email",
    tag = "settings",
    summary = "Send test email",
    request_body = SendTestEmailRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
    
    use base64::Engine;
    let base64_logo = base64::engine::general_purpose::STANDARD.encode(&logo_bytes);
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SendEmailRequest {
    pub to_email: String,
    pub subject: String,
    pub message: String,
}

// Send email using database settings - matches Python api_send_email function exactly
#[utoipa::path(
    post,
    path = "/send_email",
    tag = "settings",
    summary = "Send email",
    request_body = SendEmailRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Serialize, utoipa::ToSchema)]
pub struct ApiInfo {
    pub apikeyid: i32,
    pub userid: i32,
    pub username: String,
    pub lastfourdigits: String,
    pub created: String,
    pub podcastids: Vec<i32>,
}

// Get API info - matches Python api_get_api_info function exactly
#[utoipa::path(
    get,
    path = "/get_api_info/{user_id}",
    tag = "settings",
    summary = "Get api info",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateApiKeyRequest {
    pub user_id: i32,
    pub rssonly: bool,
    pub podcast_ids: Option<Vec<i32>>,
}

// Create API key - matches Python api_create_api_key function exactly
#[utoipa::path(
    post,
    path = "/create_api_key",
    tag = "settings",
    summary = "Create api key",
    request_body = CreateApiKeyRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct DeleteApiKeyRequest {
    pub api_id: String,
}

// Delete API key - matches Python api_delete_api_key function exactly
#[utoipa::path(
    delete,
    path = "/delete_api_key",
    tag = "settings",
    summary = "Delete api key",
    request_body = DeleteApiKeyRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
    info!("🔐 delete_api_key: requesting_user={}, api_key_owner={}, is_admin={}, api_id={}", 
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct BackupUserRequest {
    pub user_id: i32,
}

// Backup user data - matches Python backup_user function exactly
#[utoipa::path(
    post,
    path = "/backup_user",
    tag = "settings",
    summary = "Backup user",
    request_body = BackupUserRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = String),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct BackupServerRequest {
    pub database_pass: String,
}

// Backup server data - improved streaming approach for large databases
#[utoipa::path(
    post,
    path = "/backup_server",
    tag = "settings",
    summary = "Backup server",
    request_body = BackupServerRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Backup file download", content_type = "application/octet-stream"),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
               // Transient login-token tables: never worth backing up (security) and a
               // frequent source of cross-version schema drift on restore.
               .arg("--exclude-table-data=public.\"Sessions\"")
               .arg("--exclude-table-data=public.\"GpodderSessions\"")
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
               // Transient login-token tables: never worth backing up (security) and a
               // frequent source of cross-version schema drift on restore.
               .arg(format!("--ignore-table={}.Sessions", database))
               .arg(format!("--ignore-table={}.GpodderSessions", database))
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
        let stderr_reader = tokio::io::BufReader::new(stderr);
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
                info!("Backup process completed successfully");
            }
            Ok(status) => {
                warn!("Backup process failed with status: {}", status);
                if !stderr_output.is_empty() {
                    info!("Mysqldump stderr output: {}", stderr_output);
                }
            }
            Err(e) => {
                warn!("Failed to wait for backup process: {}", e);
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

/// RAII guard for the global "restore in progress" flag. Resets the flag on drop so a
/// panic or early return in the restore task can't leave restores permanently blocked.
pub struct RestoreGuard(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl RestoreGuard {
    /// Acquire the guard, or return None if a restore is already running.
    pub fn try_acquire(flag: &std::sync::Arc<std::sync::atomic::AtomicBool>) -> Option<Self> {
        use std::sync::atomic::Ordering;
        match flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => Some(RestoreGuard(flag.clone())),
            Err(_) => None,
        }
    }
}

impl Drop for RestoreGuard {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

#[utoipa::path(
    post,
    path = "/restore_server",
    tag = "settings",
    summary = "Restore server",
    request_body(content = String, content_type = "multipart/form-data", description = "Uploaded file"),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

    // Refuse to start a second restore while one is already running.
    let restore_guard = RestoreGuard::try_acquire(&state.restore_in_progress)
        .ok_or_else(|| AppError::conflict("A restore is already in progress"))?;

    // Stream the uploaded file to a temp file on the backups volume so memory usage
    // stays bounded for large dumps (a real instance backup can be hundreds of MB).
    // A ".tmp" extension keeps it out of list_backup_files (which only lists ".sql").
    let backup_dir = std::path::Path::new("/opt/pinepods/backups");
    tokio::fs::create_dir_all(backup_dir).await
        .map_err(|e| AppError::internal(&format!("Failed to create backup directory: {}", e)))?;
    let tmp_path = backup_dir.join(format!(".restore_upload_{}.tmp", uuid::Uuid::new_v4()));

    // Process the multipart form to get the uploaded file and (unused) database password.
    let mut have_file = false;
    let mut _have_password = false;

    while let Some(mut field) = multipart.next_field().await.map_err(|e| AppError::bad_request(&format!("Multipart error: {}", e)))? {
        let name = field.name().unwrap_or("").to_string();

        if name == "backup_file" {
            let filename = field.file_name().unwrap_or("").to_string();

            // Validate file extension
            if !filename.ends_with(".sql") {
                return Err(AppError::bad_request("Only SQL files are allowed"));
            }

            let mut file = tokio::fs::File::create(&tmp_path).await
                .map_err(|e| AppError::internal(&format!("Failed to create temp restore file: {}", e)))?;

            use tokio::io::AsyncWriteExt;
            while let Some(chunk) = field.chunk().await
                .map_err(|e| AppError::bad_request(&format!("Failed to read upload: {}", e)))? {
                if let Err(e) = file.write_all(&chunk).await {
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    return Err(AppError::internal(&format!("Failed to write upload: {}", e)));
                }
            }
            file.flush().await
                .map_err(|e| AppError::internal(&format!("Failed to flush temp restore file: {}", e)))?;
            have_file = true;
        } else if name == "database_pass" {
            // The uploaded password is ignored; restore uses the DB_PASSWORD env var.
            let _ = field.bytes().await;
            _have_password = true;
        }
    }

    if !have_file {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        return Err(AppError::bad_request("No SQL file uploaded"));
    }

    // Run the restore as a tracked progress task so the UI sees real completion (the
    // upload itself can take a while; the restore then runs against the streamed file).
    // The guard is moved into the task and released (via Drop) when it finishes.
    let db_pool = state.db_pool.clone();
    let task_id = state.task_spawner.spawn_progress_task(
        "restore_server".to_string(),
        user_id,
        move |reporter| {
            let db_pool = db_pool.clone();
            let tmp_path = tmp_path.clone();
            let _restore_guard = restore_guard;
            async move {
                reporter.update_progress(10.0, Some("Starting restore...".to_string())).await?;
                reporter.update_progress(50.0, Some("Restoring database...".to_string())).await?;

                let result = db_pool.restore_server_data_from_path(&tmp_path).await;

                // Always clean up the temp upload, success or failure.
                if let Err(e) = tokio::fs::remove_file(&tmp_path).await {
                    tracing::warn!("Failed to remove temp restore file {}: {}", tmp_path.display(), e);
                }
                result?;

                reporter.update_progress(100.0, Some("Restore completed successfully".to_string())).await?;
                Ok(serde_json::json!({ "status": "Restore completed successfully" }))
            }
        }
    ).await?;

    Ok(Json(serde_json::json!({
        "message": "Server restore started successfully",
        "task_id": task_id
    })))
}

/// Lightweight restore-status probe. Deliberately reads ONLY the in-memory flag and does
/// no database work and no auth, so it stays responsive even while a restore holds table
/// locks (which blocks every DB-backed request). The frontend polls this to show a
/// full-page "restore in progress" overlay and to auto-reload when it finishes.
#[utoipa::path(
    get,
    path = "/restore_status",
    tag = "settings",
    summary = "Restore status",
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
    ),
)]
pub async fn restore_status(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let in_progress = state.restore_in_progress.load(std::sync::atomic::Ordering::SeqCst);
    Json(serde_json::json!({ "restore_in_progress": in_progress }))
}

// Generate MFA secret - matches Python generate_mfa_secret function exactly
#[utoipa::path(
    get,
    path = "/generate_mfa_secret/{user_id}",
    tag = "settings",
    summary = "Generate mfa secret",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct VerifyTempMfaRequest {
    pub user_id: i32,
    pub mfa_code: String,
}

// Verify temporary MFA code - matches Python verify_temp_mfa function exactly
#[utoipa::path(
    post,
    path = "/verify_temp_mfa",
    tag = "settings",
    summary = "Verify temp mfa",
    request_body = VerifyTempMfaRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/check_mfa_enabled/{user_id}",
    tag = "settings",
    summary = "Check mfa enabled",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SaveMfaSecretRequest {
    pub user_id: i32,
    pub mfa_secret: String,
}

// Save MFA secret - matches Python save_mfa_secret function exactly
#[utoipa::path(
    post,
    path = "/save_mfa_secret",
    tag = "settings",
    summary = "Save mfa secret",
    request_body = SaveMfaSecretRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    delete,
    path = "/delete_mfa",
    tag = "settings",
    summary = "Delete mfa",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct InitiateNextcloudLoginRequest {
    pub user_id: i32,
    pub nextcloud_url: String,
}

// Initiate Nextcloud login - matches Python initiate_nextcloud_login function exactly
#[utoipa::path(
    post,
    path = "/initiate_nextcloud_login",
    tag = "settings",
    summary = "Initiate nextcloud login",
    request_body = InitiateNextcloudLoginRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, Clone, utoipa::ToSchema)]
pub struct AddNextcloudServerRequest {
    pub user_id: i32,
    pub token: String,
    pub poll_endpoint: String,
    pub nextcloud_url: String,
}

// Add Nextcloud server - matches Python add_nextcloud_server function exactly
#[utoipa::path(
    post,
    path = "/add_nextcloud_server",
    tag = "settings",
    summary = "Add nextcloud server",
    request_body = AddNextcloudServerRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
        error!("Failed to update task progress: {}", e);
    }

    match poll_for_auth_completion(&request.poll_endpoint, &request.token, &state.task_manager, &task_id).await {
        Ok(credentials) => {
            info!("Nextcloud authentication successful: {:?}", credentials);
            
            // Update task progress
            if let Err(e) = state.task_manager.update_task_progress(&task_id, 90.0, Some("Authentication successful, saving credentials...".to_string())).await {
                error!("Failed to update task progress: {}", e);
            }
            
            // Extract credentials from the response
            if let (Some(app_password), Some(login_name)) = (
                credentials.get("appPassword").and_then(|v| v.as_str()),
                credentials.get("loginName").and_then(|v| v.as_str())
            ) {
                // Save the real credentials using the database method
                match state.db_pool.save_nextcloud_credentials(request.user_id, &request.nextcloud_url, app_password, login_name).await {
                    Ok(_) => {
                        debug!("Successfully added Nextcloud settings for user {}", request.user_id);
                        if let Err(e) = state.task_manager.complete_task(&task_id, 
                            Some(serde_json::json!({"status": "success", "message": "Nextcloud authentication completed"})), 
                            Some("Nextcloud authentication completed successfully".to_string())).await {
                            error!("Failed to complete task: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to add Nextcloud settings: {}", e);
                        if let Err(e) = state.task_manager.fail_task(&task_id, format!("Failed to save Nextcloud settings: {}", e)).await {
                            error!("Failed to fail task: {}", e);
                        }
                    }
                }
            } else {
                error!("Missing appPassword or loginName in credentials");
                if let Err(e) = state.task_manager.fail_task(&task_id, "Missing credentials in Nextcloud response".to_string()).await {
                    error!("Failed to fail task: {}", e);
                }
            }
        }
        Err(e) => {
            error!("Nextcloud authentication failed: {}", e);
            if let Err(e) = state.task_manager.fail_task(&task_id, format!("Authentication failed: {}", e)).await {
                error!("Failed to fail task: {}", e);
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
            error!("Failed to update task progress during polling: {}", e);
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
                        info!("Authentication successful: {:?}", credentials);
                        return Ok(credentials);
                    }
                    404 => {
                        // User hasn't completed auth yet, continue polling
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                    status => {
                        warn!("Polling failed with status code {}", status);
                        return Err(format!("Polling for Nextcloud authentication failed with status {}", status).into());
                    }
                }
            }
            Err(e) => {
                warn!("Connection error, retrying: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }

    Err("Polling timeout reached".into())
}

// Request struct for verify_gpodder_auth
#[derive(Deserialize, utoipa::ToSchema)]
pub struct VerifyGpodderAuthRequest {
    pub gpodder_url: String,
    pub gpodder_username: String,
    pub gpodder_password: String,
}

// Verify gPodder authentication - matches Python verify_gpodder_auth function exactly
#[utoipa::path(
    post,
    path = "/verify_gpodder_auth",
    tag = "settings",
    summary = "Verify gpodder auth",
    request_body = VerifyGpodderAuthRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddGpodderServerRequest {
    pub gpodder_url: String,
    pub gpodder_username: String,
    pub gpodder_password: String,
}

// Add gPodder server - matches Python add_gpodder_server function exactly
#[utoipa::path(
    post,
    path = "/add_gpodder_server",
    tag = "settings",
    summary = "Add gpodder server",
    request_body = AddGpodderServerRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/get_gpodder_settings/{user_id}",
    tag = "settings",
    summary = "Get gpodder settings",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/check_gpodder_settings/{user_id}",
    tag = "settings",
    summary = "Check gpodder settings",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Debug, serde::Deserialize, utoipa::ToSchema)]
pub struct RemoveSyncRequest {
    pub user_id: i32,
}

#[utoipa::path(
    delete,
    path = "/remove_podcast_sync",
    tag = "settings",
    summary = "Remove podcast sync",
    request_body = RemoveSyncRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct CustomPodcastRequest {
    pub feed_url: String,
    pub user_id: i32,
    pub username: Option<String>,
    pub password: Option<String>,
    pub youtube_channel: Option<bool>,
    pub feed_cutoff: Option<i32>,
}

// Request struct for notification_settings
#[derive(Deserialize, utoipa::ToSchema)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct NotificationTestRequest {
    pub user_id: i32,
    pub platform: String,
}

// Request struct for add_oidc_provider
#[derive(Deserialize, utoipa::ToSchema)]
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
#[derive(Deserialize, utoipa::IntoParams)]
pub struct UserIdQuery {
    pub user_id: i32,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct StartpageQuery {
    pub user_id: i32,
    pub startpage: Option<String>,
}

// Add custom podcast - matches Python add_custom_podcast function exactly
#[utoipa::path(
    post,
    path = "/add_custom_podcast",
    tag = "settings",
    summary = "Add custom podcast",
    request_body = CustomPodcastRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
                warn!("Error processing YouTube channel {}: {}", channel_id_clone, e);
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

// Get notification settings - matches Python notification_settings GET function exactly
#[utoipa::path(
    get,
    path = "/user/notification_settings",
    tag = "settings",
    summary = "Get notification settings",
    params(UserIdQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    put,
    path = "/user/notification_settings",
    tag = "settings",
    summary = "Update notification settings",
    request_body = NotificationSettingsRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/user/test_notification",
    tag = "settings",
    summary = "Test notification",
    request_body = NotificationTestRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/add_oidc_provider",
    tag = "settings",
    summary = "Add oidc provider",
    request_body = OidcProviderRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    put,
    path = "/update_oidc_provider/{provider_id}",
    tag = "settings",
    summary = "Update oidc provider",
    params(("provider_id" = i32, Path)),
    request_body = OidcProviderRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/list_oidc_providers",
    tag = "settings",
    summary = "List oidc providers",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/remove_oidc_provider",
    tag = "settings",
    summary = "Remove oidc provider",
    request_body = i32,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/startpage",
    tag = "settings",
    summary = "Get startpage",
    params(UserIdQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/startpage",
    tag = "settings",
    summary = "Update startpage",
    params(StartpageQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct PersonSubscribeRequest {
    pub person_name: String,
    pub person_img: String,
    pub podcast_id: i32,
}

// Request struct for person unsubscribe
#[derive(Deserialize, utoipa::ToSchema)]
pub struct PersonUnsubscribeRequest {
    pub person_name: String,
}

// Subscribe to person - matches Python api_subscribe_to_person function exactly
#[utoipa::path(
    post,
    path = "/person/subscribe/{user_id}/{person_id}",
    tag = "settings",
    summary = "Subscribe to person",
    params(("user_id" = i32, Path), ("person_id" = i32, Path)),
    request_body = PersonSubscribeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    delete,
    path = "/person/unsubscribe/{user_id}/{person_id}",
    tag = "settings",
    summary = "Unsubscribe from person",
    params(("user_id" = i32, Path), ("person_id" = i32, Path)),
    request_body = PersonUnsubscribeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/person/subscriptions/{user_id}",
    tag = "settings",
    summary = "Get person subscriptions",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

#[derive(Deserialize, utoipa::IntoParams)]
pub struct PersonEpisodesQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

// Get person episodes - matches Python api_return_person_episodes function exactly
#[utoipa::path(
    get,
    path = "/person/episodes/{user_id}/{person_id}",
    tag = "settings",
    summary = "Get person episodes",
    params(PersonEpisodesQuery, ("user_id" = i32, Path), ("person_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_person_episodes(
    State(state): State<AppState>,
    Path((user_id, person_id)): Path<(i32, i32)>,
    Query(params): Query<PersonEpisodesQuery>,
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

    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let episodes = state.db_pool.get_person_episodes(user_id, person_id, limit, offset).await?;
    Ok(Json(serde_json::json!({
        "episodes": episodes
    })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct HostFeedQuery {
    pub name: String,
    pub person_id: Option<i32>,
    pub include_podcasts: Option<bool>,
}

fn empty_host_feed(name: &str) -> serde_json::Value {
    serde_json::json!({ "person": { "name": name, "image": null }, "podcasts": [], "episodes": [] })
}

// Unified host feed — the single source of a host's episodes (and the shows they appear in),
// drawn from BOTH the Podcast Index person index and PodPeopleDB, viewable whether or not the
// user is subscribed. Returns { person, podcasts, episodes }.
//
// - Subscribed, episodes-only callers (the subscribed-people list) take a fast path served from
//   the cached PeopleEpisodes table.
// - The host profile page takes the live path: a short-lived Redis cache holds the shared
//   (non-user) feed by host name, and per-user interaction state is overlaid per request so the
//   cache can be shared across users.
#[utoipa::path(
    get,
    path = "/person/feed/{user_id}",
    tag = "settings",
    summary = "Get host feed",
    params(HostFeedQuery, ("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_host_feed(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Query(params): Query<HostFeedQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only retrieve your own host feed!"));
    }

    let include_podcasts = params.include_podcasts.unwrap_or(true);

    // Fast path: subscribed host, episodes only — serve from the prebuilt PeopleEpisodes table.
    if !include_podcasts {
        if let Some(person_id) = params.person_id {
            if person_id > 0 {
                let episodes = state.db_pool.get_person_episodes(user_id, person_id, 300, 0).await?;
                return Ok(Json(serde_json::json!({
                    "person": { "name": params.name, "image": null },
                    "podcasts": [],
                    "episodes": episodes
                })));
            }
        }
    }

    // Live path: serve the shared (non-user-specific) feed from a two-layer cache, then overlay
    // this user's interaction state. Layers, fastest first:
    //   1. Redis hot cache (5 min) — absorbs repeat visits.
    //   2. HostFeedCache DB warm cache (24h) — survives Redis expiry/restart so a cold entry
    //      doesn't force a full N-feed rebuild.
    //   3. Live build (fetch + parse every feed the host appears in), then write both caches.
    let cache_key = format!("host_feed:{}:{}", include_podcasts, params.name.to_lowercase());
    const REDIS_TTL_SECS: u64 = 300;
    const DB_CACHE_MAX_AGE_SECS: i64 = 86_400;

    let mut feed: serde_json::Value = if let Ok(Some(cached)) = state.redis_client.get::<String>(&cache_key).await {
        serde_json::from_str(&cached).unwrap_or_else(|_| empty_host_feed(&params.name))
    } else if let Ok(Some(warm)) = state.db_pool.get_cached_host_feed(&cache_key, DB_CACHE_MAX_AGE_SECS).await {
        // Warm DB hit — repopulate Redis so subsequent hits stay hot.
        if let Ok(serialized) = serde_json::to_string(&warm) {
            let _ = state.redis_client.set_ex(&cache_key, serialized, REDIS_TTL_SECS).await;
        }
        warm
    } else {
        let built = state.db_pool.build_host_feed_shared(&params.name, include_podcasts).await?;
        if let Ok(serialized) = serde_json::to_string(&built) {
            let _ = state.redis_client.set_ex(&cache_key, serialized.clone(), REDIS_TTL_SECS).await;
            let _ = state.db_pool.set_cached_host_feed(&cache_key, &serialized).await;
        }
        built
    };

    state.db_pool.overlay_host_feed_user_state(user_id, &mut feed).await?;

    Ok(Json(feed))
}

// Request struct for set_podcast_playback_speed - matches Python SetPlaybackSpeedPodcast model
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetPlaybackSpeedPodcast {
    pub user_id: i32,
    pub podcast_id: i32,
    pub playback_speed: f64,
}

// Set podcast playback speed - matches Python api_set_podcast_playback_speed endpoint
#[utoipa::path(
    post,
    path = "/podcast/set_playback_speed",
    tag = "settings",
    summary = "Set podcast playback speed",
    request_body = SetPlaybackSpeedPodcast,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

// Request struct for clear_podcast_playback_speed
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ClearPlaybackSpeedPodcast {
    pub user_id: i32,
    pub podcast_id: i32,
}

// Clear podcast playback speed - resets the podcast back to the global default
#[utoipa::path(
    post,
    path = "/clear_podcast_playback_speed",
    tag = "settings",
    summary = "Clear podcast playback speed",
    request_body = ClearPlaybackSpeedPodcast,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn clear_podcast_playback_speed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClearPlaybackSpeedPodcast>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.clear_podcast_playback_speed(request.user_id, request.podcast_id).await?;

    Ok(Json(serde_json::json!({ "message": "Podcast playback speed reset to global default." })))
}

// Request struct for set_podcast_auto_download_delete_days (#655)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetAutoDownloadDeleteDaysPodcast {
    pub user_id: i32,
    pub podcast_id: i32,
    pub days: i32,
}

// Set per-podcast server-download retention override (#655)
#[utoipa::path(
    post,
    path = "/podcast/set_auto_download_delete_days",
    tag = "settings",
    summary = "Set podcast auto-delete downloads days",
    request_body = SetAutoDownloadDeleteDaysPodcast,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn set_podcast_auto_download_delete_days(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetAutoDownloadDeleteDaysPodcast>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    let days = request.days.max(0);
    state.db_pool.set_podcast_auto_download_delete_days(request.user_id, request.podcast_id, days).await?;

    Ok(Json(serde_json::json!({ "detail": "Podcast auto-delete downloads setting updated." })))
}

// Request struct for clear_podcast_auto_download_delete_days (#655)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ClearAutoDownloadDeleteDaysPodcast {
    pub user_id: i32,
    pub podcast_id: i32,
}

// Clear per-podcast auto-delete override - resets the podcast back to the global default (#655)
#[utoipa::path(
    post,
    path = "/clear_podcast_auto_download_delete_days",
    tag = "settings",
    summary = "Clear podcast auto-delete downloads days",
    request_body = ClearAutoDownloadDeleteDaysPodcast,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn clear_podcast_auto_download_delete_days(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClearAutoDownloadDeleteDaysPodcast>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.clear_podcast_auto_download_delete_days(request.user_id, request.podcast_id).await?;

    Ok(Json(serde_json::json!({ "message": "Podcast auto-delete downloads reset to global default." })))
}

// Request struct for enable_auto_download - matches Python AutoDownloadRequest model
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AutoDownloadRequest {
    pub podcast_id: i32,
    pub auto_download: bool,
    pub user_id: i32,
}

// Enable/disable auto download for podcast - matches Python api_enable_auto_download endpoint
#[utoipa::path(
    post,
    path = "/enable_auto_download",
    tag = "settings",
    summary = "Enable auto download",
    request_body = AutoDownloadRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

// Request struct for enable_auto_queue (#648)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AutoQueueRequest {
    pub podcast_id: i32,
    pub auto_queue: bool,
    pub user_id: i32,
}

// Enable/disable auto-queue new episodes for podcast (#648)
#[utoipa::path(
    post,
    path = "/enable_auto_queue",
    tag = "settings",
    summary = "Enable auto queue",
    request_body = AutoQueueRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn enable_auto_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AutoQueueRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - user can only modify their own podcasts
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id != request.user_id {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.enable_auto_queue(request.podcast_id, request.auto_queue, request.user_id).await?;

    Ok(Json(serde_json::json!({ "detail": "Auto-queue status updated." })))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AutoPlayNextRequest {
    pub podcast_id: i32,
    pub auto_play_next: bool,
    pub user_id: i32,
}

// Enable/disable auto play next for podcast
#[utoipa::path(
    post,
    path = "/enable_auto_play_next",
    tag = "settings",
    summary = "Enable auto play next",
    request_body = AutoPlayNextRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn enable_auto_play_next(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AutoPlayNextRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;

    if key_id != request.user_id {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    state.db_pool.enable_auto_play_next(request.podcast_id, request.auto_play_next, request.user_id).await?;

    Ok(Json(serde_json::json!({ "detail": "Auto-play-next status updated." })))
}

// Request struct for toggle_podcast_notifications - matches Python TogglePodcastNotificationData model
#[derive(Deserialize, utoipa::ToSchema)]
pub struct TogglePodcastNotificationData {
    pub user_id: i32,
    pub podcast_id: i32,
    pub enabled: bool,
}

// Toggle podcast notifications - matches Python api_toggle_podcast_notifications endpoint
#[utoipa::path(
    put,
    path = "/podcast/toggle_notifications",
    tag = "settings",
    summary = "Toggle podcast notifications",
    request_body = TogglePodcastNotificationData,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

// Request struct for toggle_podcast_favorite
#[derive(Deserialize, utoipa::ToSchema)]
pub struct TogglePodcastFavoriteData {
    pub user_id: i32,
    pub podcast_id: i32,
    pub is_favorite: bool,
}

// Toggle podcast favorite status
#[utoipa::path(
    put,
    path = "/podcast/toggle_favorite",
    tag = "settings",
    summary = "Toggle podcast favorite",
    request_body = TogglePodcastFavoriteData,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn toggle_podcast_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TogglePodcastFavoriteData>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("Invalid API key"));
    }

    let success = state.db_pool.toggle_podcast_favorite(request.user_id, request.podcast_id, request.is_favorite).await?;

    if success {
        Ok(Json(serde_json::json!({ "detail": "Favorite status updated successfully" })))
    } else {
        Ok(Json(serde_json::json!({ "detail": "Failed to update favorite status" })))
    }
}

// Request struct for adjust_skip_times - matches Python SkipTimesRequest model
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SkipTimesRequest {
    pub podcast_id: i32,
    #[serde(default)]
    pub start_skip: i32,
    #[serde(default)]
    pub end_skip: i32,
    pub user_id: i32,
}

// Adjust skip times for podcast - matches Python api_adjust_skip_times endpoint
#[utoipa::path(
    post,
    path = "/adjust_skip_times",
    tag = "settings",
    summary = "Adjust skip times",
    request_body = SkipTimesRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

// Report whether the optional AI sidecar (#726) is available, so clients can
// show/hide AI features (transcription, later ad-detection/RAG).
#[utoipa::path(
    get,
    path = "/ai_status",
    tag = "settings",
    summary = "Whether the optional AI sidecar is available",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn ai_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    let available = state.ai_available.is_available();
    // Per-capability readiness: transcription needs a whisper model (always defaulted); ad removal
    // needs a configured LLM backend (a local model, or a remote URL).
    let (transcription_ready, ad_removal_ready) = if available {
        match crate::services::ai_settings::get_ai_settings(&state.db_pool).await {
            Ok(s) => {
                let tr = !s.transcription_model.is_empty();
                let ad = match s.llm_backend.as_str() {
                    "remote" | "anthropic" => s.llm_url.as_deref().map(|u| !u.is_empty()).unwrap_or(false),
                    _ => s.llm_model.as_deref().map(|m| !m.is_empty()).unwrap_or(false),
                };
                (tr, ad)
            }
            Err(_) => (available, false),
        }
    } else {
        (false, false)
    };
    Ok(Json(serde_json::json!({
        "available": available,
        "transcription_ready": transcription_ready,
        "ad_removal_ready": ad_removal_ready,
    })))
}

// ---- Transcription endpoints (#726) ----

// Manually (re-)transcribe an episode via the AI sidecar
#[derive(Deserialize, utoipa::ToSchema)]
pub struct TranscribeEpisodeRequest {
    pub episode_id: i32,
    pub user_id: i32,
    #[serde(default)]
    pub force: bool,
}

#[utoipa::path(
    post,
    path = "/transcribe_episode",
    tag = "settings",
    summary = "Transcribe an episode (AI sidecar)",
    request_body = TranscribeEpisodeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
        (status = 503, description = "AI service unavailable"),
    ),
)]
pub async fn transcribe_episode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TranscribeEpisodeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only process your own episodes."));
    }
    if !state.ai_available.is_available() {
        return Err(AppError::service_unavailable("AI service is not available."));
    }

    let task_id = state
        .task_spawner
        .spawn_transcribe_episode(request.episode_id, request.user_id, request.force)
        .await?;

    Ok(Json(serde_json::json!({ "task_id": task_id, "detail": "Transcription started." })))
}

// Read the stored generated transcript for an episode
#[derive(Deserialize, utoipa::IntoParams)]
pub struct EpisodeTranscriptQuery {
    pub episode_id: i32,
    pub user_id: i32,
}

#[utoipa::path(
    get,
    path = "/episode_transcript",
    tag = "settings",
    summary = "Get the stored (generated) transcript for an episode",
    params(EpisodeTranscriptQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_episode_transcript(
    State(state): State<AppState>,
    Query(query): Query<EpisodeTranscriptQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only view your own episodes."));
    }

    let transcript = crate::services::transcription::get_episode_transcript(&state.db_pool, query.episode_id)
        .await
        .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "transcript": transcript })))
}

// Per-podcast auto-transcribe opt-in (get + set)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AutoTranscribeRequest {
    pub podcast_id: i32,
    pub user_id: i32,
    #[serde(default)]
    pub enabled: bool,
}

#[utoipa::path(
    post,
    path = "/adjust_auto_transcribe",
    tag = "settings",
    summary = "Set per-podcast auto-transcribe",
    request_body = AutoTranscribeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn adjust_auto_transcribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AutoTranscribeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    crate::services::transcription::set_auto_transcribe(
        &state.db_pool, request.podcast_id, request.user_id, request.enabled,
    )
    .await
    .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "detail": "Auto-transcribe updated." })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct AutoTranscribeQuery {
    pub podcast_id: i32,
    pub user_id: i32,
}

#[utoipa::path(
    get,
    path = "/get_auto_transcribe",
    tag = "settings",
    summary = "Get per-podcast auto-transcribe setting",
    params(AutoTranscribeQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_auto_transcribe(
    State(state): State<AppState>,
    Query(query): Query<AutoTranscribeQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only view your own podcasts."));
    }

    let enabled = crate::services::transcription::get_auto_transcribe(&state.db_pool, query.podcast_id)
        .await
        .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "enabled": enabled })))
}

// ---- Silence-trim / skip-segment endpoints (#727) ----

fn default_silence_threshold() -> i32 { 2 }

// Per-podcast silence-trim settings
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SilenceTrimRequest {
    pub podcast_id: i32,
    pub user_id: i32,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_silence_threshold")]
    pub threshold: i32,
}

#[utoipa::path(
    post,
    path = "/adjust_silence_trim",
    tag = "settings",
    summary = "Set per-podcast silence-trim (enable + threshold)",
    request_body = SilenceTrimRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn adjust_silence_trim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SilenceTrimRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    crate::services::audio_processing::set_trim_silence(
        &state.db_pool, request.podcast_id, request.user_id, request.enabled, request.threshold,
    )
    .await
    .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "detail": "Silence trim settings updated." })))
}

// Read per-podcast silence-trim settings
#[derive(Deserialize, utoipa::IntoParams)]
pub struct SilenceTrimQuery {
    pub podcast_id: i32,
    pub user_id: i32,
}

#[utoipa::path(
    get,
    path = "/get_silence_trim",
    tag = "settings",
    summary = "Get per-podcast silence-trim settings",
    params(SilenceTrimQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_silence_trim(
    State(state): State<AppState>,
    Query(query): Query<SilenceTrimQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only view your own podcasts."));
    }

    let (enabled, threshold) =
        crate::services::audio_processing::get_trim_silence(&state.db_pool, query.podcast_id)
            .await
            .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "enabled": enabled, "threshold": threshold })))
}

// Read all skip segments (silence, and later ads) the player should auto-skip for an episode
#[derive(Deserialize, utoipa::IntoParams)]
pub struct SkipSegmentsQuery {
    pub episode_id: i32,
    pub user_id: i32,
}

#[utoipa::path(
    get,
    path = "/episode_skip_segments",
    tag = "settings",
    summary = "Get auto-skip segments for an episode",
    params(SkipSegmentsQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_episode_skip_segments(
    State(state): State<AppState>,
    Query(query): Query<SkipSegmentsQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only view your own episodes."));
    }

    // Per-user view: enriches each segment with its DB id and (for ads) the requesting user's
    // effective status, so the player and transcript review UI share one shape.
    let segments = crate::services::ad_detection::get_episode_skip_segments_for_user(
        &state.db_pool, query.user_id, query.episode_id,
    )
    .await
    .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "segments": segments })))
}

// Manually (re-)run silence detection for one episode as a tracked background task
#[derive(Deserialize, utoipa::ToSchema)]
pub struct DetectSilenceRequest {
    pub episode_id: i32,
    pub user_id: i32,
    #[serde(default)]
    pub force: bool,
}

#[utoipa::path(
    post,
    path = "/detect_silence",
    tag = "settings",
    summary = "Run silence detection for an episode",
    request_body = DetectSilenceRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn detect_silence(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DetectSilenceRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only process your own episodes."));
    }

    let task_id = state
        .task_spawner
        .spawn_detect_silence(request.episode_id, request.user_id, request.force)
        .await?;

    Ok(Json(serde_json::json!({ "task_id": task_id, "detail": "Silence detection started." })))
}

// ---- Ad-detection endpoints (#790) ----

// Manually (re-)detect ads for an episode via the AI sidecar (transcribes first if needed)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct DetectAdsRequest {
    pub episode_id: i32,
    pub user_id: i32,
    #[serde(default)]
    pub force: bool,
}

#[utoipa::path(
    post,
    path = "/detect_ads",
    tag = "settings",
    summary = "Detect ads for an episode (AI sidecar)",
    request_body = DetectAdsRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
        (status = 503, description = "AI service unavailable"),
    ),
)]
pub async fn detect_ads(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DetectAdsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only process your own episodes."));
    }
    if !state.ai_available.is_available() {
        return Err(AppError::service_unavailable("AI service is not available."));
    }

    let task_id = state
        .task_spawner
        .spawn_detect_ads(request.episode_id, request.user_id, request.force)
        .await?;

    Ok(Json(serde_json::json!({ "task_id": task_id, "detail": "Ad detection started." })))
}

// Per-user confirm/deny of a detected ad segment
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AdSegmentReviewRequest {
    pub segment_id: i32,
    pub user_id: i32,
    pub status: String, // "confirmed" | "rejected"
}

#[utoipa::path(
    post,
    path = "/adjust_ad_segment_review",
    tag = "settings",
    summary = "Confirm or deny a detected ad segment (per-user)",
    request_body = AdSegmentReviewRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn adjust_ad_segment_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AdSegmentReviewRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only review your own ad segments."));
    }

    crate::services::ad_detection::set_ad_segment_review(
        &state.db_pool, request.user_id, request.segment_id, &request.status,
    )
    .await
    .map_err(|e| AppError::bad_request(e))?;

    Ok(Json(serde_json::json!({ "detail": "Ad review updated." })))
}

// Per-podcast auto-ad-detect opt-in (get + set)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AutoAdDetectRequest {
    pub podcast_id: i32,
    pub user_id: i32,
    #[serde(default)]
    pub enabled: bool,
}

#[utoipa::path(
    post,
    path = "/adjust_auto_ad_detect",
    tag = "settings",
    summary = "Set per-podcast auto-ad-detect",
    request_body = AutoAdDetectRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn adjust_auto_ad_detect(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AutoAdDetectRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    crate::services::ad_detection::set_auto_ad_detect(
        &state.db_pool, request.podcast_id, request.user_id, request.enabled,
    )
    .await
    .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "detail": "Auto ad-detect updated." })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct AutoAdDetectQuery {
    pub podcast_id: i32,
    pub user_id: i32,
}

#[utoipa::path(
    get,
    path = "/get_auto_ad_detect",
    tag = "settings",
    summary = "Get per-podcast auto-ad-detect setting",
    params(AutoAdDetectQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_auto_ad_detect(
    State(state): State<AppState>,
    Query(query): Query<AutoAdDetectQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only view your own podcasts."));
    }

    let enabled = crate::services::ad_detection::get_auto_ad_detect(&state.db_pool, query.podcast_id)
        .await
        .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "enabled": enabled })))
}

// Per-podcast ad-skip auto-activate (skip immediately vs. require confirmation)
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AdSkipAutoActivateRequest {
    pub podcast_id: i32,
    pub user_id: i32,
    #[serde(default)]
    pub enabled: bool,
}

#[utoipa::path(
    post,
    path = "/adjust_ad_skip_auto_activate",
    tag = "settings",
    summary = "Set per-podcast ad-skip auto-activate",
    request_body = AdSkipAutoActivateRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn adjust_ad_skip_auto_activate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AdSkipAutoActivateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own podcasts."));
    }

    crate::services::ad_detection::set_ad_skip_auto_activate(
        &state.db_pool, request.podcast_id, request.user_id, request.enabled,
    )
    .await
    .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "detail": "Ad-skip auto-activate updated." })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct AdSkipAutoActivateQuery {
    pub podcast_id: i32,
    pub user_id: i32,
}

#[utoipa::path(
    get,
    path = "/get_ad_skip_auto_activate",
    tag = "settings",
    summary = "Get per-podcast ad-skip auto-activate setting",
    params(AdSkipAutoActivateQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_ad_skip_auto_activate(
    State(state): State<AppState>,
    Query(query): Query<AdSkipAutoActivateQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only view your own podcasts."));
    }

    let enabled = crate::services::ad_detection::get_ad_skip_auto_activate(&state.db_pool, query.podcast_id)
        .await
        .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "enabled": enabled })))
}

// ---- AI settings + model management (admin-only) ----

#[utoipa::path(
    get,
    path = "/ai_settings",
    tag = "settings",
    summary = "Get global AI settings (admin)",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn get_ai_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if !state.db_pool.user_admin_check(key_id).await? {
        return Err(AppError::forbidden("Admin access required."));
    }

    let settings = crate::services::ai_settings::get_ai_settings(&state.db_pool)
        .await
        .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "settings": settings })))
}

#[utoipa::path(
    post,
    path = "/ai_settings",
    tag = "settings",
    summary = "Update global AI settings (admin)",
    request_body = crate::services::ai_settings::AiSettingsUpdate,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Admin access required"),
    ),
)]
pub async fn update_ai_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<crate::services::ai_settings::AiSettingsUpdate>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if !state.db_pool.user_admin_check(key_id).await? {
        return Err(AppError::forbidden("Admin access required."));
    }

    crate::services::ai_settings::set_ai_settings(&state.db_pool, request)
        .await
        .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "detail": "AI settings updated." })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct AiModelsQuery {
    pub remote_url: Option<String>,
}

#[utoipa::path(
    get,
    path = "/ai_models",
    tag = "settings",
    summary = "List models installed in the AI sidecar (admin)",
    params(AiModelsQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Admin access required"),
        (status = 503, description = "AI service unavailable"),
    ),
)]
pub async fn get_ai_models(
    State(state): State<AppState>,
    Query(query): Query<AiModelsQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if !state.db_pool.user_admin_check(key_id).await? {
        return Err(AppError::forbidden("Admin access required."));
    }
    if !state.ai_available.is_available() {
        return Err(AppError::service_unavailable("AI service is not available."));
    }

    let models = crate::services::ai_client::list_models(query.remote_url.as_deref())
        .await
        .map_err(|e| AppError::internal(&e))?;

    Ok(Json(serde_json::json!({ "models": models })))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct AiPullModelRequest {
    pub kind: String, // "whisper" | "gguf" | "ollama"
    pub model: String,
    pub repo: Option<String>,
    pub filename: Option<String>,
    pub url: Option<String>,
}

#[utoipa::path(
    post,
    path = "/ai_pull_model",
    tag = "settings",
    summary = "Pull a model into the AI sidecar (admin)",
    request_body = AiPullModelRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Admin access required"),
        (status = 503, description = "AI service unavailable"),
    ),
)]
pub async fn ai_pull_model(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AiPullModelRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if !state.db_pool.user_admin_check(key_id).await? {
        return Err(AppError::forbidden("Admin access required."));
    }
    if !state.ai_available.is_available() {
        return Err(AppError::service_unavailable("AI service is not available."));
    }

    let spec = crate::services::ai_client::PullSpec {
        kind: request.kind,
        model: request.model,
        repo: request.repo,
        filename: request.filename,
        url: request.url,
    };
    let task_id = state.task_spawner.spawn_pull_model(spec, key_id).await?;

    Ok(Json(serde_json::json!({ "task_id": task_id, "detail": "Model pull started." })))
}

// Request struct for remove_category - matches Python RemoveCategoryData model
#[derive(Deserialize, utoipa::ToSchema)]
pub struct RemoveCategoryData {
    pub podcast_id: i32,
    pub user_id: i32,
    pub category: String,
}

// Remove category from podcast - matches Python api_remove_category endpoint
#[utoipa::path(
    post,
    path = "/remove_category",
    tag = "settings",
    summary = "Remove category",
    request_body = RemoveCategoryData,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct AddCategoryData {
    pub podcast_id: i32,
    pub user_id: i32,
    pub category: String,
}

// Add category to podcast - matches Python api_add_category endpoint
#[utoipa::path(
    post,
    path = "/add_category",
    tag = "settings",
    summary = "Add category",
    request_body = AddCategoryData,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/rss_key",
    tag = "settings",
    summary = "Get user rss key",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

#[derive(Deserialize, utoipa::ToSchema)]
pub struct VerifyMfaRequest {
    pub user_id: i32,
    pub mfa_code: String,
}

// Verify MFA code - matches Python verify_mfa endpoint
#[utoipa::path(
    post,
    path = "/verify_mfa",
    tag = "settings",
    summary = "Verify mfa",
    request_body = VerifyMfaRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ScheduleBackupRequest {
    pub user_id: i32,
    pub cron_schedule: String, // e.g., "0 2 * * *" for daily at 2 AM
    pub enabled: bool,
    // Number of scheduled backups to keep; None/0 = keep all
    #[serde(default)]
    pub retention_count: Option<i32>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct DeleteBackupFileRequest {
    pub backup_filename: String,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct GetScheduledBackupRequest {
    pub user_id: i32,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ListBackupFilesRequest {}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RestoreBackupFileRequest {
    pub backup_filename: String,
}

// Schedule automatic backup - admin only
#[utoipa::path(
    post,
    path = "/schedule_backup",
    tag = "settings",
    summary = "Schedule backup",
    request_body = ScheduleBackupRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
    state.db_pool.set_scheduled_backup(request.user_id, &request.cron_schedule, request.enabled, request.retention_count).await?;

    Ok(Json(serde_json::json!({
        "detail": "Backup schedule updated successfully",
        "schedule": request.cron_schedule,
        "enabled": request.enabled,
        "retention_count": request.retention_count
    })))
}

// Get scheduled backup settings - admin only
#[utoipa::path(
    post,
    path = "/get_scheduled_backup",
    tag = "settings",
    summary = "Get scheduled backup",
    request_body = GetScheduledBackupRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/list_backup_files",
    tag = "settings",
    summary = "List backup files",
    request_body = ListBackupFilesRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn list_backup_files(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_request): Json<ListBackupFilesRequest>,
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
#[utoipa::path(
    post,
    path = "/restore_backup_file",
    tag = "settings",
    summary = "Restore from backup file",
    request_body = RestoreBackupFileRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

    // Refuse to start a second restore while one is already running.
    let restore_guard = RestoreGuard::try_acquire(&state.restore_in_progress)
        .ok_or_else(|| AppError::conflict("A restore is already in progress"))?;

    // Clone for the async closure
    let backup_filename_for_closure = backup_filename.clone();
    let db_pool = state.db_pool.clone();

    // Spawn restoration task
    let task_id = state.task_spawner.spawn_progress_task(
        "restore_from_backup_file".to_string(),
        0, // System user
        move |reporter| {
            let backup_path = backup_path.clone();
            let backup_filename = backup_filename_for_closure;
            let db_pool = db_pool.clone();
            // Released (via Drop) when the restore task finishes.
            let _restore_guard = restore_guard;
            async move {
                reporter.update_progress(10.0, Some("Starting restoration from backup file...".to_string())).await?;
                reporter.update_progress(50.0, Some("Restoring database...".to_string())).await?;

                // Clears existing data, then streams the dump into psql/mysql.
                db_pool.restore_server_data_from_path(std::path::Path::new(&backup_path)).await?;

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

// Delete a backup file from the mounted backup directory - admin only
#[utoipa::path(
    post,
    path = "/delete_backup_file",
    tag = "settings",
    summary = "Delete backup file",
    request_body = DeleteBackupFileRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn delete_backup_file(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeleteBackupFileRequest>,
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
    if backup_filename.contains("..") || backup_filename.contains('/') || !backup_filename.ends_with(".sql") {
        return Err(AppError::bad_request("Invalid backup filename"));
    }

    let backup_path = format!("/opt/pinepods/backups/{}", backup_filename);

    if !std::path::Path::new(&backup_path).exists() {
        return Err(AppError::not_found("Backup file not found"));
    }

    std::fs::remove_file(&backup_path)
        .map_err(|e| AppError::internal(&format!("Failed to delete backup file: {}", e)))?;

    Ok(Json(serde_json::json!({
        "detail": "Backup file deleted",
        "filename": backup_filename
    })))
}

// Request struct for manual backup to directory
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ManualBackupRequest {}

// Manual backup to directory - admin only
#[utoipa::path(
    post,
    path = "/manual_backup_to_directory",
    tag = "settings",
    summary = "Manual backup to directory",
    request_body = ManualBackupRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn manual_backup_to_directory(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(_request): Json<ManualBackupRequest>,
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
    // Ownership is handled by running the process as PUID:PGID (see startup.sh); no chown needed.

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
                            // Transient login-token tables: excluded for security and to avoid
                            // cross-version schema drift on restore.
                            &format!("--ignore-table={}.Sessions", db_name),
                            &format!("--ignore-table={}.GpodderSessions", db_name),
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
                            // Transient login-token tables: excluded for security and to avoid
                            // cross-version schema drift on restore.
                            "--exclude-table-data=public.\"Sessions\"",
                            "--exclude-table-data=public.\"GpodderSessions\"",
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
                // Ownership is handled by running the process as PUID:PGID (see startup.sh); no chown needed.

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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct GetUnmatchedPodcastsRequest {
    pub user_id: i32,
}

// Get podcasts that have podcast_index_id = 0 (imported via OPML without podcast index match)
#[utoipa::path(
    post,
    path = "/get_unmatched_podcasts",
    tag = "settings",
    summary = "Get unmatched podcasts",
    request_body = GetUnmatchedPodcastsRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdatePodcastIndexIdRequest {
    pub user_id: i32,
    pub podcast_id: i32,
    pub podcast_index_id: i32,
}

// Update a podcast's podcast_index_id
#[utoipa::path(
    post,
    path = "/update_podcast_index_id",
    tag = "settings",
    summary = "Update podcast index id",
    request_body = UpdatePodcastIndexIdRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct IgnorePodcastIndexIdRequest {
    pub user_id: i32,
    pub podcast_id: i32,
    pub ignore: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct GetIgnoredPodcastsRequest {
    pub user_id: i32,
}

// Ignore/unignore a podcast's index ID requirement
#[utoipa::path(
    post,
    path = "/ignore_podcast_index_id",
    tag = "settings",
    summary = "Ignore podcast index id",
    request_body = IgnorePodcastIndexIdRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    post,
    path = "/get_ignored_podcasts",
    tag = "settings",
    summary = "Get ignored podcasts",
    request_body = GetIgnoredPodcastsRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/get_user_language",
    tag = "settings",
    summary = "Get user language",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = UserLanguageResponse),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    put,
    path = "/update_user_language",
    tag = "settings",
    summary = "Update user language",
    request_body = crate::models::LanguageUpdateRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/get_available_languages",
    tag = "settings",
    summary = "Get available languages",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = AvailableLanguagesResponse),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/get_server_default_language",
    tag = "settings",
    summary = "Get server default language",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetGlobalPodcastCoverPreference {
    pub user_id: i32,
    pub use_podcast_covers: bool,
    pub podcast_id: Option<i32>,
}

// Set global podcast cover preference - matches Python api_set_global_podcast_cover_preference function
#[utoipa::path(
    post,
    path = "/user/set_global_podcast_cover_preference",
    tag = "settings",
    summary = "Set global podcast cover preference",
    request_body = SetGlobalPodcastCoverPreference,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetPodcastCoverPreference {
    pub user_id: i32,
    pub podcast_id: i32,
    pub use_podcast_covers: bool,
}

// Set podcast cover preference - matches Python api_set_podcast_cover_preference function
#[utoipa::path(
    post,
    path = "/podcast/set_cover_preference",
    tag = "settings",
    summary = "Set podcast cover preference",
    request_body = SetPodcastCoverPreference,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[derive(Deserialize, utoipa::ToSchema)]
pub struct ClearPodcastCoverPreference {
    pub user_id: i32,
    pub podcast_id: i32,
}

// Clear podcast cover preference - matches Python api_clear_podcast_cover_preference function
#[utoipa::path(
    post,
    path = "/podcast/clear_cover_preference",
    tag = "settings",
    summary = "Clear podcast cover preference",
    request_body = ClearPodcastCoverPreference,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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
#[utoipa::path(
    get,
    path = "/user/get_podcast_cover_preference",
    tag = "settings",
    summary = "Get global podcast cover preference",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
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

// Get all shared links for the authenticated user
#[utoipa::path(
    get,
    path = "/get_user_shared_links/{user_id}",
    tag = "settings",
    summary = "Get user shared links",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_user_shared_links(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if user_id_from_api_key != user_id {
        return Err(AppError::forbidden("You can only access your own shared links."));
    }

    let links = state.db_pool.get_user_shared_episodes(user_id).await?;
    Ok(Json(serde_json::json!({ "shared_links": links })))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct DeleteSharedLinkRequest {
    pub share_code: String,
}

// Delete a shared link owned by the authenticated user
#[utoipa::path(
    delete,
    path = "/delete_shared_link",
    tag = "settings",
    summary = "Delete shared link",
    request_body = DeleteSharedLinkRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn delete_shared_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<DeleteSharedLinkRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let deleted = state.db_pool.delete_user_shared_episode(&request.share_code, user_id).await?;

    if deleted {
        Ok(Json(serde_json::json!({ "detail": "Shared link deleted." })))
    } else {
        Err(AppError::not_found("Shared link not found or not owned by you."))
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ExtendSharedLinkRequest {
    pub share_code: String,
    pub days: i64,
}

// Extend the expiry of a shared link owned by the authenticated user
#[utoipa::path(
    put,
    path = "/extend_shared_link",
    tag = "settings",
    summary = "Extend shared link",
    request_body = ExtendSharedLinkRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn extend_shared_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ExtendSharedLinkRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    if request.days < 1 || request.days > 365 {
        return Err(AppError::bad_request("Days must be between 1 and 365."));
    }

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let extended = state.db_pool.extend_user_shared_episode(&request.share_code, user_id, request.days).await?;

    if extended {
        Ok(Json(serde_json::json!({ "detail": "Shared link extended." })))
    } else {
        Err(AppError::not_found("Shared link not found or not owned by you."))
    }
}

#[utoipa::path(
    get,
    path = "/user/custom_themes/{user_id}",
    tag = "settings",
    summary = "Get custom themes",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_custom_themes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only view your own custom themes!"));
    }

    let themes = state.db_pool.get_custom_themes(user_id).await?;
    Ok(Json(serde_json::json!({ "themes": themes })))
}

#[utoipa::path(
    post,
    path = "/user/custom_themes",
    tag = "settings",
    summary = "Create custom theme",
    request_body = crate::models::CreateCustomThemeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn create_custom_theme(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<crate::models::CreateCustomThemeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only create themes for yourself!"));
    }

    let theme = state.db_pool.create_custom_theme(request.user_id, &request).await?;
    Ok(Json(serde_json::json!({ "theme": theme })))
}

#[utoipa::path(
    delete,
    path = "/user/custom_themes",
    tag = "settings",
    summary = "Delete custom theme",
    request_body = crate::models::DeleteCustomThemeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn delete_custom_theme(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<crate::models::DeleteCustomThemeRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != request.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only delete your own custom themes!"));
    }

    state.db_pool.delete_custom_theme(request.theme_id, request.user_id).await?;
    Ok(Json(serde_json::json!({ "message": "Custom theme deleted successfully" })))
}

