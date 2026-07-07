use axum::{extract::State, http::HeaderMap, response::Json};
use crate::{
    database,
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    models::{CreatePlaylistRequest, CreatePlaylistResponse, DeletePlaylistRequest, DeletePlaylistResponse, UpdatePlaylistRequest, UpdatePlaylistResponse},
    AppState,
};

#[utoipa::path(
    post,
    path = "/create_playlist",
    tag = "playlists",
    summary = "Create a playlist",
    request_body = CreatePlaylistRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Playlist created", body = CreatePlaylistResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot create a playlist for another user"),
    ),
)]
pub async fn create_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(playlist_data): Json<CreatePlaylistRequest>,
) -> AppResult<Json<CreatePlaylistResponse>> {
    let api_key = extract_api_key(&headers)?;
    let is_valid = validate_api_key(&state, &api_key).await?;
    
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }
    
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    
    if user_id != playlist_data.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only create playlists for yourself!"));
    }

    let playlist_id = database::create_playlist(&state.db_pool, &state.config, &playlist_data).await?;

    Ok(Json(CreatePlaylistResponse {
        detail: "Playlist created successfully".to_string(),
        playlist_id,
    }))
}

#[utoipa::path(
    delete,
    path = "/delete_playlist",
    tag = "playlists",
    summary = "Delete a playlist",
    request_body = DeletePlaylistRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Playlist deleted", body = DeletePlaylistResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot delete another user's playlist"),
    ),
)]
pub async fn delete_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(playlist_data): Json<DeletePlaylistRequest>,
) -> AppResult<Json<DeletePlaylistResponse>> {
    let api_key = extract_api_key(&headers)?;
    let is_valid = validate_api_key(&state, &api_key).await?;
    
    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }
    
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    
    if user_id != playlist_data.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only delete your own playlists!"));
    }

    database::delete_playlist(&state.db_pool, &state.config, &playlist_data).await?;

    Ok(Json(DeletePlaylistResponse {
        detail: "Playlist deleted successfully".to_string(),
    }))
}

#[utoipa::path(
    patch,
    path = "/update_playlist",
    tag = "playlists",
    summary = "Update a playlist",
    request_body = UpdatePlaylistRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Playlist updated", body = UpdatePlaylistResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot edit another user's playlist"),
    ),
)]
pub async fn update_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(playlist_data): Json<UpdatePlaylistRequest>,
) -> AppResult<Json<UpdatePlaylistResponse>> {
    let api_key = extract_api_key(&headers)?;
    let is_valid = validate_api_key(&state, &api_key).await?;

    if !is_valid {
        return Err(AppError::unauthorized("Your API key is either invalid or does not have correct permission"));
    }

    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if user_id != playlist_data.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only edit your own playlists!"));
    }

    database::update_playlist(&state.db_pool, &state.config, &playlist_data).await?;

    Ok(Json(UpdatePlaylistResponse {
        detail: "Playlist updated successfully".to_string(),
    }))
}