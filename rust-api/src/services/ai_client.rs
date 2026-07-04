//! HTTP client + availability gating for the optional `pinepods-ai` sidecar (#726).
//!
//! The sidecar is optional: when `PINEPODS_AI_URL` is unset, or the service is unreachable, AI
//! features (transcription now; ad-detection/RAG later) are disabled. `AiAvailability` holds a
//! shared flag that a periodic health check updates, so handlers can cheaply gate on it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;
use tracing::{debug, info, warn};

/// Shared, cheaply-cloneable availability flag for the AI sidecar.
#[derive(Clone)]
pub struct AiAvailability {
    available: Arc<AtomicBool>,
}

impl AiAvailability {
    pub fn new() -> Self {
        Self { available: Arc::new(AtomicBool::new(false)) }
    }

    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
    }

    fn set(&self, value: bool) {
        self.available.store(value, Ordering::Relaxed);
    }
}

impl Default for AiAvailability {
    fn default() -> Self {
        Self::new()
    }
}

/// The configured base URL of the sidecar, or `None` if the feature is off.
pub fn ai_base_url() -> Option<String> {
    match std::env::var("PINEPODS_AI_URL") {
        Ok(v) if !v.trim().is_empty() => Some(v.trim().trim_end_matches('/').to_string()),
        _ => None,
    }
}

/// Optional shared secret sent as `X-AI-Token` (matches the sidecar's `PINEPODS_AI_TOKEN`).
fn ai_token() -> Option<String> {
    std::env::var("PINEPODS_AI_TOKEN").ok().filter(|v| !v.is_empty())
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(600)) // transcription of a long episode can take a while
        .build()
        .unwrap_or_default()
}

/// One transcript segment as returned by the sidecar.
#[derive(Debug, Deserialize)]
pub struct AiSegment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// The sidecar's `/transcribe` response.
#[derive(Debug, Deserialize)]
pub struct TranscribeResult {
    pub language: String,
    pub text: String,
    pub segments: Vec<AiSegment>,
    pub model: String,
    #[allow(dead_code)]
    pub duration: f64,
}

/// Probe the sidecar's `/health`. Returns false if unconfigured or unreachable.
pub async fn check_health() -> bool {
    let Some(base) = ai_base_url() else { return false };
    match client().get(format!("{}/health", base)).timeout(Duration::from_secs(5)).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(e) => {
            debug!("AI sidecar health check failed: {}", e);
            false
        }
    }
}

/// Request a transcription of a locally-downloaded file. `file_path` must be a path the sidecar
/// can read (it shares the downloads mount). The sidecar streams NDJSON progress lines as it
/// works; `on_progress` is called with a 0.0–1.0 fraction for each, and the final result is
/// returned. No overall timeout — long episodes can take many minutes.
pub async fn transcribe(
    file_path: &str,
    language: Option<&str>,
    mut on_progress: impl FnMut(f64),
) -> Result<TranscribeResult, String> {
    let base = ai_base_url().ok_or_else(|| "AI service not configured".to_string())?;
    let stream_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = stream_client
        .post(format!("{}/transcribe", base))
        .json(&serde_json::json!({ "file_path": file_path, "language": language }));
    if let Some(token) = ai_token() {
        req = req.header("X-AI-Token", token);
    }
    let mut resp = req.send().await.map_err(|e| format!("AI request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("AI transcribe returned {}: {}", status, body));
    }

    // Parse newline-delimited JSON as chunks arrive.
    let mut buf: Vec<u8> = Vec::new();
    let mut result: Option<TranscribeResult> = None;
    while let Some(chunk) = resp.chunk().await.map_err(|e| format!("AI stream error: {}", e))? {
        buf.extend_from_slice(&chunk);
        while let Some(pos) = buf.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = buf.drain(..=pos).collect();
            let line = String::from_utf8_lossy(&line);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let v: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue, // ignore partial/garbage lines defensively
            };
            match v.get("type").and_then(|t| t.as_str()) {
                Some("progress") => {
                    if let Some(p) = v.get("progress").and_then(|p| p.as_f64()) {
                        on_progress(p);
                    }
                }
                Some("error") => {
                    return Err(v.get("error").and_then(|e| e.as_str()).unwrap_or("AI error").to_string());
                }
                Some("result") => {
                    result = Some(serde_json::from_value::<TranscribeResult>(v)
                        .map_err(|e| format!("Failed to parse AI result: {}", e))?);
                }
                _ => {}
            }
        }
    }
    result.ok_or_else(|| "AI returned no result".to_string())
}

/// Spawn a background loop that keeps `availability` in sync with the sidecar's health.
/// No-op logging churn: only transitions are logged.
pub fn spawn_health_monitor(availability: AiAvailability) {
    if ai_base_url().is_none() {
        info!("AI sidecar not configured (PINEPODS_AI_URL unset); AI features disabled");
        return;
    }
    tokio::spawn(async move {
        let mut last: Option<bool> = None;
        loop {
            let ok = check_health().await;
            if last != Some(ok) {
                if ok {
                    info!("AI sidecar is available");
                } else {
                    warn!("AI sidecar is unavailable");
                }
                last = Some(ok);
            }
            availability.set(ok);
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}
