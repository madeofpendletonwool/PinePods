use axum::{
    extract::{Path, Query, State, Request},
    response::Response,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    AppState,
};

#[derive(Deserialize)]
pub struct FeedQuery {
    pub api_key: String,
    pub limit: Option<i32>,
    pub podcast_id: Option<i32>,
    #[serde(rename = "type")]
    pub source_type: Option<String>,
}

// Get RSS feed for user - matches Python get_user_feed function exactly
pub async fn get_user_feed(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Query(query): Query<FeedQuery>,
    request: Request<axum::body::Body>,
) -> Result<Response<String>, AppError> {
    let api_key = &query.api_key;
    let limit = query.limit.unwrap_or(1000);
    let podcast_id = query.podcast_id;
    let source_type = query.source_type.as_deref();
    
    // Get domain from request
    let domain = extract_domain_from_request(&request);

    // Convert single podcast_id to list format if provided
    let podcast_id_list = if let Some(id) = podcast_id {
        Some(vec![id])
    } else {
        None
    };

    // Get RSS key validation
    let rss_key = state.db_pool.get_rss_key_if_valid(api_key, podcast_id_list.as_ref()).await?;
    
    let rss_key = if let Some(key) = rss_key {
        key
    } else {
        let key_id = state.db_pool.get_user_id_from_api_key(api_key).await?;
        if key_id == 0 {
            return Err(AppError::forbidden("Invalid API key"));
        }
        
        // Create a backwards compatibility RSS key structure
        RssKeyInfo {
            podcast_ids: vec![-1],
            user_id: key_id,
            key: api_key.to_string(),
        }
    };

    let feed_content = state.db_pool.generate_podcast_rss(
        rss_key,
        limit,
        source_type,
        &domain,
        podcast_id_list.as_ref(),
    ).await?;
    
    Ok(Response::builder()
        .header("content-type", "application/rss+xml")
        .body(feed_content)
        .map_err(|e| AppError::internal(&format!("Failed to create response: {}", e)))?)
}

#[derive(Debug, Clone)]
pub struct RssKeyInfo {
    pub podcast_ids: Vec<i32>,
    pub user_id: i32,
    pub key: String,
}

fn extract_domain_from_request(request: &Request<axum::body::Body>) -> String {
    // Check SERVER_URL environment variable first (includes scheme and port)
    // Note: We use SERVER_URL instead of HOSTNAME because Docker automatically sets HOSTNAME to the container ID
    // The startup script saves the user's HOSTNAME value to SERVER_URL before Docker overwrites it
    if let Ok(server_url) = std::env::var("SERVER_URL") {
        tracing::info!("Using SERVER_URL env var: {}", server_url);
        return server_url;
    }

    // Try to get domain from Host header
    if let Some(host) = request.headers().get("host") {
        if let Ok(host_str) = host.to_str() {
            // Determine scheme - check for X-Forwarded-Proto or assume http
            let scheme = request.headers()
                .get("x-forwarded-proto")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("http");

            let domain = format!("{}://{}", scheme, host_str);
            tracing::info!("Using Host header: {}", domain);
            return domain;
        }
    }

    // Fallback
    tracing::info!("Using fallback domain");
    "http://localhost:8041".to_string()
}