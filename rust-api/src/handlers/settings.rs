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

// Request struct for set_theme
#[derive(Deserialize)]
pub struct SetThemeRequest {
    pub user_id: i32,
    pub new_theme: String,
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

// User info response struct
#[derive(Serialize)]
pub struct UserInfo {
    pub userid: i32,
    pub fullname: String,
    pub username: String,
    pub email: String,
    pub isadmin: bool,
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
) -> Result<Json<UserInfo>, AppError> {
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