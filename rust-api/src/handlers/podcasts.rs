use axum::{extract::State, http::HeaderMap, response::Json};
use serde_json::json;
use crate::{
    error::AppResult,
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

// Podcast endpoints will be implemented here to match clientapi.py
// Examples: get_podcasts, add_podcast, remove_podcast, etc.