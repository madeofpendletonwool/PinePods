//! HTTP client + availability gating for the optional `pinepods-ai` sidecar (#726).
//!
//! The sidecar is optional: when `PINEPODS_AI_URL` is unset, or the service is unreachable, AI
//! features (transcription now; ad-detection/RAG later) are disabled. `AiAvailability` holds a
//! shared flag that a periodic health check updates, so handlers can cheaply gate on it.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
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

/// Which LLM backend the sidecar should use for ad detection, resolved from `AISettings`
/// and sent per request so the sidecar stays stateless.
#[derive(Debug, Clone, Serialize)]
pub struct LlmSpec {
    pub backend: String, // "local" (bundled GGUF) | "remote" (OpenAI-compatible)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

/// One detected ad time range (seconds).
#[derive(Debug, Deserialize)]
pub struct AdSpan {
    pub start: f64,
    pub end: f64,
}

/// The sidecar's `/detect_ads` response.
#[derive(Debug, Deserialize)]
pub struct AdDetectResult {
    pub segments: Vec<AdSpan>,
}

/// The sidecar's `/models` listing.
#[derive(Debug, Deserialize, Serialize)]
pub struct ModelsInfo {
    #[serde(default)]
    pub whisper: Vec<String>,
    #[serde(default)]
    pub llm_local: Vec<String>,
    #[serde(default)]
    pub llm_remote: Vec<String>,
    #[serde(default)]
    pub models_dir: String,
    #[serde(default)]
    pub disk: Option<DiskInfo>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DiskInfo {
    pub total: u64,
    pub used: u64,
    pub free: u64,
}

/// A model-pull request forwarded to the sidecar's `/models/pull`.
#[derive(Debug, Clone, Serialize)]
pub struct PullSpec {
    pub kind: String, // "whisper" | "gguf" | "ollama"
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
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

/// POST `body` to `{base}{path}` and consume the sidecar's NDJSON stream, invoking
/// `on_progress` with each 0.0–1.0 fraction and returning the final `result` line's JSON.
/// Shared by `/transcribe`, `/detect_ads`, and `/models/pull`. No overall timeout — long
/// jobs can take many minutes.
async fn post_ndjson(
    path: &str,
    body: serde_json::Value,
    mut on_progress: impl FnMut(f64),
) -> Result<serde_json::Value, String> {
    let base = ai_base_url().ok_or_else(|| "AI service not configured".to_string())?;
    let stream_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = stream_client.post(format!("{}{}", base, path)).json(&body);
    if let Some(token) = ai_token() {
        req = req.header("X-AI-Token", token);
    }
    let mut resp = req.send().await.map_err(|e| format!("AI request failed: {}", e))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("AI {} returned {}: {}", path, status, body));
    }

    // Parse newline-delimited JSON as chunks arrive.
    let mut buf: Vec<u8> = Vec::new();
    let mut result: Option<serde_json::Value> = None;
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
                    result = Some(v);
                }
                _ => {}
            }
        }
    }
    result.ok_or_else(|| "AI returned no result".to_string())
}

/// Request a transcription of a locally-downloaded file. `file_path` must be a path the sidecar
/// can read (it shares the downloads mount). `model` overrides the sidecar's default whisper model
/// (None uses its configured default). `on_progress` is called with each 0.0–1.0 fraction.
pub async fn transcribe(
    file_path: &str,
    language: Option<&str>,
    model: Option<&str>,
    on_progress: impl FnMut(f64),
) -> Result<TranscribeResult, String> {
    let body = serde_json::json!({ "file_path": file_path, "language": language, "model": model });
    let v = post_ndjson("/transcribe", body, on_progress).await?;
    serde_json::from_value::<TranscribeResult>(v).map_err(|e| format!("Failed to parse AI result: {}", e))
}

/// Detect ad/sponsor spans in an already-generated transcript. `segments` are the stored
/// `{start,end,text}` timings; `llm` selects the backend (resolved from `AISettings`).
pub async fn detect_ads(
    segments: &[AiSegment],
    language: Option<&str>,
    llm: &LlmSpec,
    on_progress: impl FnMut(f64),
) -> Result<AdDetectResult, String> {
    let seg_json: Vec<serde_json::Value> = segments
        .iter()
        .map(|s| serde_json::json!({ "start": s.start, "end": s.end, "text": s.text }))
        .collect();
    let body = serde_json::json!({ "segments": seg_json, "language": language, "llm": llm });
    let v = post_ndjson("/detect_ads", body, on_progress).await?;
    serde_json::from_value::<AdDetectResult>(v).map_err(|e| format!("Failed to parse AI ad result: {}", e))
}

/// Pull a model into the sidecar's models volume, streaming progress via `on_progress`.
pub async fn pull_model(spec: &PullSpec, on_progress: impl FnMut(f64)) -> Result<(), String> {
    let body = serde_json::to_value(spec).map_err(|e| e.to_string())?;
    post_ndjson("/models/pull", body, on_progress).await.map(|_| ())
}

/// List the models the sidecar has installed (and, when `remote_url` is given, those an
/// external OpenAI-compatible / Ollama endpoint exposes).
pub async fn list_models(remote_url: Option<&str>) -> Result<ModelsInfo, String> {
    let base = ai_base_url().ok_or_else(|| "AI service not configured".to_string())?;
    let mut req = client().get(format!("{}/models", base)).timeout(Duration::from_secs(15));
    if let Some(url) = remote_url {
        req = req.query(&[("remote_url", url)]);
    }
    if let Some(token) = ai_token() {
        req = req.header("X-AI-Token", token);
    }
    let resp = req.send().await.map_err(|e| format!("AI request failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("AI /models returned {}", resp.status()));
    }
    resp.json::<ModelsInfo>().await.map_err(|e| format!("Failed to parse models list: {}", e))
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
