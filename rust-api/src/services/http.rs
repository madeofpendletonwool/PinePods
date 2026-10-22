// Shared outbound HTTP clients. A reqwest::Client holds a connection pool, so building one per
// request (what the refresh path used to do) threw away keep-alive and TLS sessions on every
// feed. These are process-wide and cheap to clone; DatabasePool methods use them directly since
// they have no AppState.

use std::sync::LazyLock;
use std::time::Duration;

pub const PINEPODS_USER_AGENT: &str = "PinePods/1.0";

/// Some hosts block podcast-app user agents; feeds retry once with this on 403/406.
pub const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

const FEED_TIMEOUT: Duration = Duration::from_secs(30);
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

static FEED_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| feed_builder(PINEPODS_USER_AGENT).build().expect("feed HTTP client"));

static BROWSER_FEED_CLIENT: LazyLock<reqwest::Client> =
    LazyLock::new(|| feed_builder(BROWSER_USER_AGENT).build().expect("browser feed HTTP client"));

static PROBE_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(BROWSER_USER_AGENT)
        .timeout(PROBE_TIMEOUT)
        .redirect(crate::services::url_guard::guarded_redirect_policy())
        // HEAD probes read Content-Length, which reqwest drops when it decodes a compressed body.
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .build()
        .expect("probe HTTP client")
});

static SERVICE_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .user_agent(PINEPODS_USER_AGENT)
        .timeout(FEED_TIMEOUT)
        .build()
        .expect("service HTTP client")
});

fn feed_builder(user_agent: &str) -> reqwest::ClientBuilder {
    // gzip/brotli/deflate/zstd are enabled crate-wide, so Accept-Encoding is sent and bodies are
    // decoded transparently.
    reqwest::Client::builder()
        .user_agent(user_agent)
        .timeout(FEED_TIMEOUT)
        .redirect(crate::services::url_guard::guarded_redirect_policy())
}

/// RSS feed fetches: PinePods UA, 30s timeout, redirects re-checked by the SSRF guard.
pub fn feed_client() -> &'static reqwest::Client {
    &FEED_CLIENT
}

/// Feed fetches retried with a browser UA after a 403/406.
pub fn browser_feed_client() -> &'static reqwest::Client {
    &BROWSER_FEED_CLIENT
}

/// HEAD requests against enclosure URLs (duration estimates): 10s timeout, no decompression.
pub fn probe_client() -> &'static reqwest::Client {
    &PROBE_CLIENT
}

/// Calls to user-configured services (ntfy, Gotify). These are often on the LAN, so redirects are
/// not SSRF-guarded; callers set a per-request timeout.
pub fn service_client() -> &'static reqwest::Client {
    &SERVICE_CLIENT
}

/// Builder for clients that need their own state, e.g. a cookie jar per sync run.
pub fn sync_client_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(PINEPODS_USER_AGENT)
        .timeout(FEED_TIMEOUT)
        .cookie_store(true)
}

/// Longest `Retry-After` we honour; anything beyond is clamped.
pub const MAX_RETRY_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// Parse a `Retry-After` header (delta-seconds or an HTTP date) into a wait from now.
pub fn parse_retry_after(value: &str, now: chrono::DateTime<chrono::Utc>) -> Option<Duration> {
    let value = value.trim();
    let wait = if let Ok(secs) = value.parse::<u64>() {
        Duration::from_secs(secs)
    } else {
        let at = chrono::DateTime::parse_from_rfc2822(value).ok()?.with_timezone(&chrono::Utc);
        (at - now).to_std().unwrap_or(Duration::ZERO)
    };
    Some(wait.min(MAX_RETRY_AFTER))
}

/// Hex SHA-256 of a feed body, compared across refreshes to skip unchanged feeds whose host
/// sends no usable ETag/Last-Modified.
pub fn content_hash(body: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(body.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_after_seconds_and_dates() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-21T12:00:00Z").unwrap().with_timezone(&chrono::Utc);
        assert_eq!(parse_retry_after("120", now), Some(Duration::from_secs(120)));
        assert_eq!(parse_retry_after(" 0 ", now), Some(Duration::ZERO));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2026 12:05:00 GMT", now), Some(Duration::from_secs(300)));
        assert_eq!(parse_retry_after("Wed, 21 Oct 2026 11:00:00 GMT", now), Some(Duration::ZERO), "past dates mean now");
        assert_eq!(parse_retry_after("999999999", now), Some(MAX_RETRY_AFTER));
        assert_eq!(parse_retry_after("soon", now), None);
    }

    #[test]
    fn content_hash_is_stable_hex_sha256() {
        assert_eq!(content_hash("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    }
}
