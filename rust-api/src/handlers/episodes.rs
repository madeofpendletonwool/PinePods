use axum::{extract::State, http::HeaderMap, response::Json};
use serde_json::json;
use crate::{
    error::AppResult,
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

// Episode endpoints will be implemented here to match clientapi.py
// Examples: get_episodes, mark_episode_completed, save_episode, etc.