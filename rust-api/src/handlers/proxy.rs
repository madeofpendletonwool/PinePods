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
    /// Return a thumbnail at most this many pixels wide (rounded up to one of a few fixed
    /// sizes, max 1024). Images already that small, or that can't be decoded, come back as-is.
    pub w: Option<u32>,
}

/// Thumbnail widths the proxy produces; a requested width is rounded up to the next one so
/// the cache holds a handful of variants per image rather than one per pixel width.
const THUMB_WIDTHS: [u32; 7] = [64, 128, 192, 256, 384, 512, 1024];

fn thumb_width(requested: u32) -> u32 {
    THUMB_WIDTHS
        .iter()
        .copied()
        .find(|w| *w >= requested)
        .unwrap_or(*THUMB_WIDTHS.last().unwrap())
}

/// Downscale `bytes` to `width` pixels wide. JPEG for opaque images, PNG when there's
/// transparency. None when the image is already narrow enough or can't be decoded.
fn make_thumbnail(bytes: &[u8], width: u32) -> Option<(&'static str, Vec<u8>)> {
    use image::{codecs::jpeg::JpegEncoder, ImageReader, Limits};
    use std::io::Cursor;

    let mut reader = ImageReader::new(Cursor::new(bytes)).with_guessed_format().ok()?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(12_000);
    limits.max_image_height = Some(12_000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().ok()?;
    if img.width() <= width {
        return None;
    }
    let height = ((img.height() as u64 * width as u64) / img.width() as u64).max(1) as u32;
    let thumb = img.thumbnail(width, height);

    let mut out = Vec::new();
    if thumb.color().has_alpha() {
        thumb
            .write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png)
            .ok()?;
        Some(("image/png", out))
    } else {
        JpegEncoder::new_with_quality(&mut out, 82)
            .encode_image(&thumb.to_rgb8())
            .ok()?;
        Some(("image/jpeg", out))
    }
}

// Server-side image cache: 1-day TTL, skip caching anything larger than this so a few huge images
// can't blow out Redis. Cached entries are stored as two keys: bytes + content-type.
const IMAGE_CACHE_TTL_SECS: u64 = 86_400;
const IMAGE_CACHE_MAX_BYTES: usize = 5 * 1024 * 1024;

fn image_cache_keys(url: &str, width: Option<u32>) -> (String, String) {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let h = hasher.finish();
    let variant = width.map(|w| format!(":w{w}")).unwrap_or_default();
    (
        format!("imgproxy:{:x}{}:data", h, variant),
        format!("imgproxy:{:x}{}:ct", h, variant),
    )
}

async fn cached_image(state: &AppState, url: &str, width: Option<u32>) -> Option<(String, Vec<u8>)> {
    let (data_key, ct_key) = image_cache_keys(url, width);
    let cached = state.redis_client.get::<Vec<u8>>(&data_key).await.ok().flatten()?;
    if cached.is_empty() {
        return None;
    }
    let ct = state
        .redis_client
        .get::<String>(&ct_key)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "application/octet-stream".to_string());
    Some((ct, cached))
}

async fn cache_image(state: &AppState, url: &str, width: Option<u32>, content_type: &str, bytes: &[u8]) {
    if bytes.is_empty() || bytes.len() > IMAGE_CACHE_MAX_BYTES {
        return;
    }
    let (data_key, ct_key) = image_cache_keys(url, width);
    let _ = state.redis_client.set_ex(&data_key, bytes.to_vec(), IMAGE_CACHE_TTL_SECS).await;
    let _ = state
        .redis_client
        .set_ex(&ct_key, content_type.to_string(), IMAGE_CACHE_TTL_SECS)
        .await;
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

    let Some(width) = query.w.map(thumb_width) else {
        let (content_type, bytes) = original_image(&state, &query.url).await?;
        return Ok(image_response(&content_type, bytes));
    };

    if let Some((ct, bytes)) = cached_image(&state, &query.url, Some(width)).await {
        return Ok(image_response(&ct, axum::body::Bytes::from(bytes)));
    }
    let (content_type, original) = original_image(&state, &query.url).await?;
    let source = original.clone();
    let thumb = tokio::task::spawn_blocking(move || make_thumbnail(&source, width))
        .await
        .ok()
        .flatten();
    match thumb {
        Some((ct, bytes)) => {
            cache_image(&state, &query.url, Some(width), ct, &bytes).await;
            Ok(image_response(ct, axum::body::Bytes::from(bytes)))
        }
        // Already small enough, or not something we can decode: send the original.
        None => Ok(image_response(&content_type, original)),
    }
}

/// The full-size image, from the cache or fetched (and then cached).
async fn original_image(
    state: &AppState,
    url: &str,
) -> Result<(String, axum::body::Bytes), StatusCode> {
    // Serve from the server-side cache when present (avoids re-fetching upstream for every cold
    // client). Best-effort: any Redis hiccup just falls through to a live fetch.
    if let Some((ct, cached)) = cached_image(state, url, None).await {
        tracing::debug!("Image proxy cache hit for {}", url);
        return Ok((ct, axum::body::Bytes::from(cached)));
    }

    // SSRF guard: refuse to fetch from loopback/private/link-local/etc. addresses
    // (e.g. cloud metadata at 169.254.169.254 or internal-network services).
    if !host_is_public(url).await {
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

    tracing::debug!("Fetching image from: {}", url);
    
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    tracing::debug!("Image fetch response status: {}", response.status());

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

    cache_image(state, url, None, &content_type, &bytes).await;
    Ok((content_type, bytes))
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
#[cfg(test)]
mod thumbnail_tests {
    use super::*;
    use image::{ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
    use std::io::Cursor;

    fn encoded(img: image::DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), format).unwrap();
        out
    }

    #[test]
    fn widths_round_up_to_a_fixed_size() {
        assert_eq!(thumb_width(1), 64);
        assert_eq!(thumb_width(64), 64);
        assert_eq!(thumb_width(200), 256);
        assert_eq!(thumb_width(5000), 1024);
    }

    #[test]
    fn opaque_image_becomes_a_smaller_jpeg() {
        let src = encoded(RgbImage::from_pixel(1400, 1400, Rgb([200, 80, 40])).into(), ImageFormat::Png);
        let (ct, bytes) = make_thumbnail(&src, 256).expect("thumbnail");
        assert_eq!(ct, "image/jpeg");
        let out = image::load_from_memory(&bytes).unwrap();
        assert_eq!((out.width(), out.height()), (256, 256));
        assert!(bytes.len() < src.len());
    }

    #[test]
    fn keeps_aspect_ratio() {
        let src = encoded(RgbImage::from_pixel(1200, 600, Rgb([10, 10, 10])).into(), ImageFormat::Jpeg);
        let (_, bytes) = make_thumbnail(&src, 384).expect("thumbnail");
        let out = image::load_from_memory(&bytes).unwrap();
        assert_eq!((out.width(), out.height()), (384, 192));
    }

    #[test]
    fn transparent_image_stays_png() {
        let src = encoded(RgbaImage::from_pixel(800, 800, Rgba([0, 0, 0, 0])).into(), ImageFormat::Png);
        let (ct, _) = make_thumbnail(&src, 128).expect("thumbnail");
        assert_eq!(ct, "image/png");
    }

    #[test]
    fn small_or_broken_images_are_left_alone() {
        let small = encoded(RgbImage::from_pixel(100, 100, Rgb([1, 2, 3])).into(), ImageFormat::Png);
        assert!(make_thumbnail(&small, 256).is_none());
        assert!(make_thumbnail(b"not an image", 256).is_none());
    }
}
