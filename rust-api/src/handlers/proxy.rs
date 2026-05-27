use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ImageProxyQuery {
    pub url: String,
}

// Image proxy endpoint - matches Python proxy_image endpoint
pub async fn proxy_image(
    Query(query): Query<ImageProxyQuery>,
) -> Result<Response, StatusCode> {
    tracing::info!("Image proxy request received for URL: {}", query.url);

    if !is_valid_image_url(&query.url) {
        tracing::error!("Invalid image URL: {}", query.url);
        return Err(StatusCode::BAD_REQUEST);
    }

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    tracing::info!("Fetching image from: {}", query.url);
    
    let response = client
        .get(&query.url)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    tracing::info!("Image fetch response status: {}", response.status());

    if !response.status().is_success() {
        return Err(StatusCode::BAD_GATEWAY);
    }

    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|ct| ct.to_str().ok())
        .unwrap_or("")
        .to_string();

    tracing::info!("Content type: {}", content_type);

    if !content_type.starts_with("image/") && content_type != "application/octet-stream" {
        tracing::error!("Invalid content type: {}", content_type);
        return Err(StatusCode::BAD_REQUEST);
    }

    let bytes = response.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    let mut headers = HeaderMap::new();
    headers.insert("content-type", content_type.parse().unwrap());
    headers.insert("cache-control", "public, max-age=86400".parse().unwrap());
    headers.insert("access-control-allow-origin", "*".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());

    tracing::info!("Returning image response");

    Ok((headers, bytes).into_response())
}

fn is_valid_image_url(url: &str) -> bool {
    // Basic URL validation - check if it's a valid URL and uses http/https
    if let Ok(parsed_url) = url::Url::parse(url) {
        matches!(parsed_url.scheme(), "http" | "https")
    } else {
        false
    }
}

// Returns a simple SVG placeholder image at the requested dimensions
pub async fn placeholder_image(
    Path((width, height)): Path<(u32, u32)>,
) -> impl IntoResponse {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"><rect width="{w}" height="{h}" fill="#1a1a2e"/><text x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" font-family="sans-serif" font-size="14" fill="#4a4a6a">{w}x{h}</text></svg>"##,
        w = width,
        h = height
    );

    let mut headers = HeaderMap::new();
    headers.insert("content-type", "image/svg+xml".parse().unwrap());
    headers.insert("cache-control", "public, max-age=86400".parse().unwrap());
    (headers, svg)
}