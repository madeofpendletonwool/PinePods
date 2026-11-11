//! Stands in for the `pinepods-ai` sidecar: `/health`, `/transcribe`, `/detect_ads`, `/models`
//! and `/models/pull`, speaking its real NDJSON protocol over a real socket. One per test
//! process, because the server finds it through `PINEPODS_AI_URL`.
//!
//! Tests pick the outcome through what the sidecar sees. Audio starting with [`AUDIO_ERROR`]
//! reports a failure, [`AUDIO_CRASH`] drops the stream mid-job, and an LLM model named
//! [`MODEL_WHOLE_EPISODE`] claims the whole episode is an ad.

use std::sync::{Arc, Mutex};

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};

pub const AUDIO_ERROR: &[u8] = b"AI-ERROR";
pub const AUDIO_CRASH: &[u8] = b"AI-CRASH";
pub const MODEL_WHOLE_EPISODE: &str = "whole-episode-model";
pub const SPONSOR_LINE: &str = "this episode is sponsored by";

#[derive(Clone, Debug)]
pub struct AiRequest {
    pub path: String,
    pub body: Value,
    pub token: Option<String>,
    /// For `/transcribe`: the file the server pointed at, read when the request arrived.
    pub audio: Option<Vec<u8>>,
}

#[derive(Clone)]
pub struct MockAi {
    pub base_url: String,
    log: Arc<Mutex<Vec<AiRequest>>>,
}

impl MockAi {
    pub fn start() -> MockAi {
        let log: Arc<Mutex<Vec<AiRequest>>> = Arc::default();
        let (tx, rx) = std::sync::mpsc::channel();
        let state = log.clone();
        std::thread::Builder::new()
            .name("mock-ai".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    tx.send(listener.local_addr().unwrap()).unwrap();
                    let app = Router::new()
                        .route("/health", get(|| async { Json(json!({ "status": "ok" })) }))
                        .route("/models", get(models))
                        .route("/transcribe", post(transcribe))
                        .route("/detect_ads", post(detect_ads))
                        .route("/models/pull", post(pull))
                        .with_state(state);
                    axum::serve(listener, app).await.unwrap();
                });
            })
            .unwrap();
        let addr = rx.recv().expect("mock AI failed to start");
        MockAi { base_url: format!("http://{addr}"), log }
    }

    pub fn requests(&self, path: &str) -> Vec<AiRequest> {
        self.log.lock().unwrap().iter().filter(|r| r.path == path).cloned().collect()
    }
}

type Log = State<Arc<Mutex<Vec<AiRequest>>>>;

fn record(log: &Log, path: &str, headers: &HeaderMap, body: Value, audio: Option<Vec<u8>>) {
    let token = headers.get("x-ai-token").and_then(|v| v.to_str().ok()).map(str::to_string);
    log.0.lock().unwrap().push(AiRequest { path: path.to_string(), body, token, audio });
}

fn ndjson(lines: Vec<Value>) -> Response {
    let mut text = String::new();
    for line in lines {
        text.push_str(&line.to_string());
        text.push('\n');
    }
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/x-ndjson")
        .body(Body::from(Bytes::from(text)))
        .unwrap()
}

async fn transcribe(log: Log, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    let audio = body["file_path"].as_str().and_then(|p| std::fs::read(p).ok());
    record(&log, "/transcribe", &headers, body.clone(), audio.clone());
    let audio = audio.unwrap_or_default();

    if audio.starts_with(AUDIO_ERROR) {
        return ndjson(vec![json!({ "type": "error", "error": "whisper could not decode the file" })]);
    }
    if audio.starts_with(AUDIO_CRASH) {
        return ndjson(vec![json!({ "type": "progress", "progress": 0.5 })]);
    }
    let segments = json!([
        { "start": 0.0, "end": 10.0, "text": "welcome to the show" },
        { "start": 10.0, "end": 20.0, "text": SPONSOR_LINE },
        { "start": 20.0, "end": 30.0, "text": "back to the conversation" },
    ]);
    ndjson(vec![
        json!({ "type": "progress", "progress": 0.5 }),
        json!({
            "type": "result",
            "language": "en",
            "text": format!("welcome to the show {SPONSOR_LINE} back to the conversation"),
            "segments": segments,
            "model": body["model"].as_str().unwrap_or("base"),
            "duration": 30.0,
        }),
    ])
}

async fn detect_ads(log: Log, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    record(&log, "/detect_ads", &headers, body.clone(), None);
    let segments = body["segments"].as_array().cloned().unwrap_or_default();
    let spans: Vec<Value> = if body["llm"]["model"] == MODEL_WHOLE_EPISODE {
        vec![json!({ "start": 0.0, "end": segments.last().map_or(0.0, |s| s["end"].as_f64().unwrap()) })]
    } else {
        segments
            .iter()
            .filter(|s| s["text"].as_str().is_some_and(|t| t.contains(SPONSOR_LINE)))
            .map(|s| json!({ "start": s["start"], "end": s["end"] }))
            .collect()
    };
    ndjson(vec![
        json!({ "type": "progress", "progress": 1.0 }),
        json!({ "type": "result", "segments": spans }),
    ])
}

async fn models(log: Log, headers: HeaderMap) -> Json<Value> {
    record(&log, "/models", &headers, Value::Null, None);
    Json(json!({
        "whisper": ["base"],
        "llm_local": ["local.gguf"],
        "llm_remote": [],
        "models_dir": "/models",
        "disk": { "total": 100, "used": 40, "free": 60 },
    }))
}

async fn pull(log: Log, headers: HeaderMap, Json(body): Json<Value>) -> Response {
    record(&log, "/models/pull", &headers, body, None);
    ndjson(vec![
        json!({ "type": "progress", "progress": 0.5 }),
        json!({ "type": "result", "status": "done" }),
    ])
}
