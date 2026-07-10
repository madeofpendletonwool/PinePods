use axum::{
    extract::{Path, Query, State, Request},
    response::Response,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    AppState,
};

#[derive(Deserialize, utoipa::IntoParams)]
pub struct FeedQuery {
    pub api_key: String,
    pub limit: Option<i32>,
    pub podcast_id: Option<i32>,
    #[serde(rename = "type")]
    pub source_type: Option<String>,
    /// Which collection to build the feed from. Absent = all subscriptions (default).
    /// One of: saved, queue, playlist, collection, downloads, history.
    pub source: Option<String>,
    /// Playlist or collection id (required when `source` is `playlist` or `collection`).
    pub id: Option<i32>,
}

/// The set of episodes a feed is built from.
#[derive(Debug, Clone)]
pub enum FeedSource {
    /// All of the user's subscriptions (optionally filtered by podcast_id / type).
    Subscriptions,
    Saved,
    Queue,
    Playlist(i32),
    Collection(i32),
    Downloads,
    History,
}

impl FeedSource {
    /// Parse the `source`/`id` query params into a FeedSource.
    fn from_query(source: Option<&str>, id: Option<i32>) -> Result<Self, AppError> {
        match source.map(|s| s.to_ascii_lowercase()).as_deref() {
            None | Some("") | Some("subscriptions") | Some("all") => Ok(FeedSource::Subscriptions),
            Some("saved") => Ok(FeedSource::Saved),
            Some("queue") => Ok(FeedSource::Queue),
            Some("downloads") => Ok(FeedSource::Downloads),
            Some("history") => Ok(FeedSource::History),
            Some("playlist") => id
                .map(FeedSource::Playlist)
                .ok_or_else(|| AppError::bad_request("source=playlist requires an id parameter")),
            Some("collection") => id
                .map(FeedSource::Collection)
                .ok_or_else(|| AppError::bad_request("source=collection requires an id parameter")),
            Some(other) => Err(AppError::bad_request(format!("Unknown feed source: {}", other))),
        }
    }
}

// Get RSS feed for user - matches Python get_user_feed function exactly
#[utoipa::path(
    get,
    path = "/{user_id}",
    tag = "feed",
    summary = "Get user feed",
    params(FeedQuery, ("user_id" = i32, Path)),
    responses(
        (status = 200, description = "RSS feed XML", content_type = "application/rss+xml"),
    ),
)]
pub async fn get_user_feed(
    State(state): State<AppState>,
    Path(_user_id): Path<i32>,
    Query(query): Query<FeedQuery>,
    request: Request<axum::body::Body>,
) -> Result<Response<String>, AppError> {
    let api_key = &query.api_key;
    let limit = query.limit.unwrap_or(1000);
    let podcast_id = query.podcast_id;
    let source_type = query.source_type.as_deref();

    // Determine which collection the feed is built from.
    let feed_source = FeedSource::from_query(query.source.as_deref(), query.id)?;

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

    let feed_content = match feed_source {
        FeedSource::Subscriptions => {
            state.db_pool.generate_podcast_rss(
                rss_key,
                limit,
                source_type,
                &domain,
                podcast_id_list.as_ref(),
            ).await?
        }
        other => {
            state.db_pool.generate_collection_rss(
                rss_key.user_id,
                other,
                limit,
                &domain,
            ).await?
        }
    };

    Ok(Response::builder()
        .header("content-type", "application/rss+xml")
        .body(feed_content)
        .map_err(|e| AppError::internal(&format!("Failed to create response: {}", e)))?)
}

#[derive(Debug, Clone, utoipa::ToSchema)]
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