use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::net::IpAddr;

use crate::AppState;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ImageProxyQuery {
    pub url: String,
}

// Server-side image cache: 1-day TTL, skip caching anything larger than this so a few huge images
// can't blow out Redis. Cached entries are stored as two keys: bytes + content-type.
const IMAGE_CACHE_TTL_SECS: u64 = 86_400;
const IMAGE_CACHE_MAX_BYTES: usize = 5 * 1024 * 1024;

fn image_cache_keys(url: &str) -> (String, String) {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let h = hasher.finish();
    (format!("imgproxy:{:x}:data", h), format!("imgproxy:{:x}:ct", h))
}

fn image_response(content_type: &str, bytes: axum::body::Bytes) -> Response {
    let mut headers = HeaderMap::new();
    if let Ok(ct) = content_type.parse() {
        headers.insert("content-type", ct);
    }
    headers.insert("cache-control", "public, max-age=86400".parse().unwrap());
    headers.insert("access-control-allow-origin", "*".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    (headers, bytes).into_response()
}

// Image proxy endpoint - matches Python proxy_image endpoint
#[utoipa::path(
    get,
    path = "/image",
    tag = "proxy",
    summary = "Proxy image",
    params(ImageProxyQuery),
    responses(
        (status = 200, description = "Proxied image bytes", content_type = "application/octet-stream"),
    ),
)]
pub async fn proxy_image(
    State(state): State<AppState>,
    Query(query): Query<ImageProxyQuery>,
) -> Result<Response, StatusCode> {
    tracing::debug!("Image proxy request received for URL: {}", query.url);

    if !is_valid_image_url(&query.url) {
        tracing::error!("Invalid image URL: {}", query.url);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Serve from the server-side cache when present (avoids re-fetching upstream for every cold
    // client). Best-effort: any Redis hiccup just falls through to a live fetch.
    let (data_key, ct_key) = image_cache_keys(&query.url);
    if let Ok(Some(cached)) = state.redis_client.get::<Vec<u8>>(&data_key).await {
        if !cached.is_empty() {
            let ct = state
                .redis_client
                .get::<String>(&ct_key)
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "application/octet-stream".to_string());
            tracing::debug!("Image proxy cache hit for {}", query.url);
            return Ok(image_response(&ct, axum::body::Bytes::from(cached)));
        }
    }

    // SSRF guard: refuse to fetch from loopback/private/link-local/etc. addresses
    // (e.g. cloud metadata at 169.254.169.254 or internal-network services).
    if !host_is_public(&query.url).await {
        tracing::warn!("Blocked image proxy request to non-public host");
        return Err(StatusCode::FORBIDDEN);
    }

    let client = reqwest::Client::builder()
        // Re-validate redirect targets so a public URL can't bounce us to an IP-literal
        // internal address. (Hostname-based rebinding on redirect is a residual edge case.)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.error("too many redirects");
            }
            match attempt.url().host() {
                Some(url::Host::Ipv4(ip)) if is_blocked_ip(IpAddr::V4(ip)) => {
                    attempt.error("redirect to blocked address")
                }
                Some(url::Host::Ipv6(ip)) if is_blocked_ip(IpAddr::V6(ip)) => {
                    attempt.error("redirect to blocked address")
                }
                _ => attempt.follow(),
            }
        }))
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

    tracing::debug!("Content type: {}", content_type);

    if !content_type.starts_with("image/") && content_type != "application/octet-stream" {
        tracing::error!("Invalid content type: {}", content_type);
        return Err(StatusCode::BAD_REQUEST);
    }

    let bytes = response.bytes().await.map_err(|_| StatusCode::BAD_GATEWAY)?;

    // Populate the cache (best-effort) for reasonably sized images.
    if !bytes.is_empty() && bytes.len() <= IMAGE_CACHE_MAX_BYTES {
        let _ = state
            .redis_client
            .set_ex(&data_key, bytes.to_vec(), IMAGE_CACHE_TTL_SECS)
            .await;
        let _ = state
            .redis_client
            .set_ex(&ct_key, content_type.clone(), IMAGE_CACHE_TTL_SECS)
            .await;
    }

    tracing::debug!("Returning freshly fetched image response");
    Ok(image_response(&content_type, bytes))
}

fn is_valid_image_url(url: &str) -> bool {
    // Basic URL validation - check if it's a valid URL and uses http/https
    if let Ok(parsed_url) = url::Url::parse(url) {
        matches!(parsed_url.scheme(), "http" | "https")
    } else {
        false
    }
}

/// IPs the proxy must never reach (SSRF protection): loopback, private/RFC1918,
/// link-local (incl. 169.254.169.254 cloud metadata), CGNAT, unique-local, etc.
fn is_blocked_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.is_documentation()
                || v4.octets()[0] == 0 // 0.0.0.0/8
                || (v4.octets()[0] == 100 && (v4.octets()[1] & 0xc0) == 64) // 100.64.0.0/10 CGNAT
        }
        IpAddr::V6(v6) => {
            if let Some(mapped) = v6.to_ipv4_mapped() {
                return is_blocked_ip(IpAddr::V4(mapped));
            }
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00 // fc00::/7 unique-local
                || (v6.segments()[0] & 0xffc0) == 0xfe80 // fe80::/10 link-local
        }
    }
}

/// Resolve the URL's host and ensure EVERY resolved address is public.
async fn host_is_public(url_str: &str) -> bool {
    let parsed = match url::Url::parse(url_str) {
        Ok(u) => u,
        Err(_) => return false,
    };
    let host = match parsed.host_str() {
        Some(h) => h.to_string(),
        None => return false,
    };
    let port = parsed.port_or_known_default().unwrap_or(80);
    // url's host_str() already brackets IPv6 literals, so "host:port" is valid here.
    let authority = format!("{}:{}", host, port);

    match tokio::net::lookup_host(authority).await {
        Ok(addrs) => {
            let mut resolved_any = false;
            for addr in addrs {
                resolved_any = true;
                if is_blocked_ip(addr.ip()) {
                    return false;
                }
            }
            resolved_any
        }
        Err(_) => false,
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