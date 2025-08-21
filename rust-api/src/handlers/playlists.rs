use axum::{extract::State, http::HeaderMap, response::Json};
use crate::{
    database,
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    models::{CreatePlaylistRequest, CreatePlaylistResponse},
    AppState,
};

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