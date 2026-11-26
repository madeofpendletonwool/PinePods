//! Downloading episodes to this computer.
//!
//! Each download streams into `<id>.part` and is renamed only once complete, so a failed or
//! cancelled download never looks finished. Progress goes to the app as `desktop://download`
//! events; at most `PARALLEL` downloads run at once and the rest wait their turn.

use crate::library::{DownloadedEpisode, Library};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Runtime};
use tokio::io::AsyncWriteExt;
use tokio::sync::Semaphore;

pub const EVENT: &str = "desktop://download";
const PARALLEL: usize = 2;
/// A download that receives nothing for this long is treated as failed.
const STALL: Duration = Duration::from_secs(60);
const MAX_ARTWORK: usize = 15 * 1024 * 1024;

#[derive(Deserialize)]
pub struct DownloadRequest {
    pub episode: Value,
    #[serde(default)]
    pub podcast: Option<Value>,
    pub media_url: String,
    #[serde(default)]
    pub artwork_url: Option<String>,
    /// Started by automatic downloads rather than by the listener.
    #[serde(default)]
    pub automatic: bool,
}

#[derive(Serialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Queued,
    Downloading,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Serialize, Clone, Debug)]
pub struct Progress {
    pub episode_id: i32,
    pub title: String,
    pub phase: Phase,
    pub received: u64,
    pub total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

struct Job {
    cancel: Arc<AtomicBool>,
    progress: Progress,
}

pub struct Downloads {
    client: reqwest::Client,
    slots: Arc<Semaphore>,
    jobs: Mutex<HashMap<i32, Job>>,
}

impl Downloads {
    pub fn new() -> Downloads {
        let client = reqwest::Client::builder()
            .user_agent(concat!("Pinepods/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(20))
            .build()
            .expect("HTTP client");
        Downloads { client, slots: Arc::new(Semaphore::new(PARALLEL)), jobs: Mutex::new(HashMap::new()) }
    }

    pub fn active(&self) -> Vec<Progress> {
        self.jobs.lock().unwrap().values().map(|j| j.progress.clone()).collect()
    }

    pub fn cancel(&self, episode_id: i32) {
        if let Some(job) = self.jobs.lock().unwrap().get(&episode_id) {
            job.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Queue a download. Returns false if this episode is already downloading.
    pub fn start<R: Runtime>(
        self: &Arc<Self>,
        app: AppHandle<R>,
        library: Arc<Mutex<Library>>,
        request: DownloadRequest,
    ) -> Result<bool, String> {
        let episode_id = int_field(&request.episode, "episodeid").ok_or("episode has no episodeid")?;
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut jobs = self.jobs.lock().unwrap();
            if jobs.contains_key(&episode_id) {
                return Ok(false);
            }
            let title = request.episode.get("episodetitle").and_then(Value::as_str).unwrap_or_default().to_string();
            let progress = Progress { episode_id, title, phase: Phase::Queued, received: 0, total: None, error: None };
            let _ = app.emit(EVENT, &progress);
            jobs.insert(episode_id, Job { cancel: cancel.clone(), progress });
        }
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let _slot = this.slots.clone().acquire_owned().await;
            let result = if cancel.load(Ordering::Relaxed) {
                Err(Outcome::Cancelled)
            } else {
                this.run(&app, &library, &request, episode_id, &cancel).await
            };
            let mut last = this.jobs.lock().unwrap().remove(&episode_id).map(|j| j.progress).unwrap_or(
                Progress { episode_id, title: String::new(), phase: Phase::Queued, received: 0, total: None, error: None },
            );
            match result {
                Ok(()) => last.phase = Phase::Complete,
                Err(Outcome::Cancelled) => last.phase = Phase::Cancelled,
                Err(Outcome::Failed(e)) => {
                    log::warn!("Download of episode {episode_id} failed: {e}");
                    last.phase = Phase::Failed;
                    last.error = Some(e);
                }
            }
            let _ = app.emit(EVENT, &last);
        });
        Ok(true)
    }

    fn report<R: Runtime>(&self, app: &AppHandle<R>, episode_id: i32, update: impl FnOnce(&mut Progress)) {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(job) = jobs.get_mut(&episode_id) {
            update(&mut job.progress);
            let _ = app.emit(EVENT, &job.progress);
        }
    }

    async fn run<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        library: &Arc<Mutex<Library>>,
        request: &DownloadRequest,
        episode_id: i32,
        cancel: &AtomicBool,
    ) -> Result<(), Outcome> {
        let dir = library.lock().unwrap().downloads_dir().to_path_buf();
        self.report(app, episode_id, |p| p.phase = Phase::Downloading);

        let mut response = self
            .client
            .get(&request.media_url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(|e| Outcome::Failed(describe(&e)))?;
        let total = response.content_length();
        let ext = media_extension(
            response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
            response.url().path(),
        );
        self.report(app, episode_id, |p| p.total = total);

        let part = dir.join(format!("{episode_id}.part"));
        let mut file = tokio::fs::File::create(&part).await.map_err(io_failed)?;
        let mut received: u64 = 0;
        let mut last_report = Instant::now();
        let result: Result<(), Outcome> = async {
            loop {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Outcome::Cancelled);
                }
                let chunk = tokio::time::timeout(STALL, response.chunk())
                    .await
                    .map_err(|_| Outcome::Failed("the download stalled".into()))?
                    .map_err(|e| Outcome::Failed(describe(&e)))?;
                let Some(chunk) = chunk else { break };
                file.write_all(&chunk).await.map_err(io_failed)?;
                received += chunk.len() as u64;
                if last_report.elapsed() > Duration::from_millis(250) {
                    last_report = Instant::now();
                    self.report(app, episode_id, |p| p.received = received);
                }
            }
            if total.is_some_and(|t| received < t) {
                return Err(Outcome::Failed("the connection closed before the download finished".into()));
            }
            file.flush().await.map_err(io_failed)?;
            file.sync_all().await.map_err(io_failed)?;
            Ok(())
        }
        .await;
        drop(file);
        if let Err(e) = result {
            let _ = tokio::fs::remove_file(&part).await;
            return Err(e);
        }

        let file_name = format!("{episode_id}.{ext}");
        tokio::fs::rename(&part, dir.join(&file_name)).await.map_err(io_failed)?;
        self.report(app, episode_id, |p| p.received = received);

        let artwork_file = match &request.artwork_url {
            Some(url) if !url.is_empty() => self.fetch_artwork(url, &dir, episode_id).await,
            _ => None,
        };

        let record = DownloadedEpisode {
            episode_id,
            podcast_id: int_field(&request.episode, "podcastid").unwrap_or(0),
            file: file_name,
            artwork_file,
            size: received,
            downloaded_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            episode: request.episode.clone(),
            automatic: request.automatic,
        };
        library.lock().unwrap().upsert(record, request.podcast.clone());
        Ok(())
    }

    /// Artwork is optional: a failure here leaves the episode downloaded without it.
    async fn fetch_artwork(&self, url: &str, dir: &Path, episode_id: i32) -> Option<String> {
        let response = self.client.get(url).timeout(Duration::from_secs(60)).send().await.ok()?;
        let response = response.error_for_status().ok()?;
        let ext = match response.headers().get(reqwest::header::CONTENT_TYPE)?.to_str().ok()? {
            t if t.contains("png") => "png",
            t if t.contains("webp") => "webp",
            t if t.contains("jpeg") || t.contains("jpg") => "jpg",
            _ => return None,
        };
        let bytes = response.bytes().await.ok()?;
        if bytes.len() > MAX_ARTWORK {
            return None;
        }
        let name = format!("{episode_id}-art.{ext}");
        tokio::fs::write(dir.join(&name), &bytes).await.ok()?;
        Some(name)
    }
}

enum Outcome {
    Cancelled,
    Failed(String),
}

fn io_failed(e: std::io::Error) -> Outcome {
    Outcome::Failed(format!("could not write the file: {e}"))
}

fn describe(e: &reqwest::Error) -> String {
    if let Some(status) = e.status() {
        format!("the server answered {status}")
    } else if e.is_timeout() {
        "the connection timed out".into()
    } else if e.is_connect() {
        "could not connect".into()
    } else {
        e.to_string()
    }
}

fn int_field(v: &Value, key: &str) -> Option<i32> {
    v.get(key).and_then(Value::as_i64).map(|n| n as i32)
}

/// The file extension for a download, from its content type, else its URL.
pub fn media_extension(content_type: Option<&str>, url_path: &str) -> &'static str {
    const KNOWN: [&str; 10] = ["mp3", "m4a", "mp4", "aac", "ogg", "opus", "oga", "wav", "flac", "webm"];
    let mime = content_type.unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    let from_mime = match mime.as_str() {
        "audio/mpeg" | "audio/mp3" | "audio/mpeg3" => Some("mp3"),
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" => Some("m4a"),
        "video/mp4" | "video/x-m4v" => Some("mp4"),
        "audio/aac" | "audio/x-aac" => Some("aac"),
        "audio/ogg" | "application/ogg" => Some("ogg"),
        "audio/opus" => Some("opus"),
        "audio/wav" | "audio/x-wav" | "audio/wave" => Some("wav"),
        "audio/flac" | "audio/x-flac" => Some("flac"),
        "audio/webm" | "video/webm" => Some("webm"),
        _ => None,
    };
    from_mime
        .or_else(|| {
            let ext = url_path.rsplit('/').next()?.rsplit_once('.')?.1.to_ascii_lowercase();
            KNOWN.iter().copied().find(|k| *k == ext)
        })
        .unwrap_or("mp3")
}

#[cfg(test)]
mod tests {
    use super::media_extension;

    #[test]
    fn picks_extension_from_type_then_url() {
        assert_eq!(media_extension(Some("audio/mpeg"), "/x"), "mp3");
        assert_eq!(media_extension(Some("audio/x-m4a; charset=binary"), "/x.mp3"), "m4a");
        assert_eq!(media_extension(Some("video/webm"), "/x"), "webm");
        assert_eq!(media_extension(Some("application/octet-stream"), "/ep/123.M4A"), "m4a");
        assert_eq!(media_extension(None, "/ep/file.exe"), "mp3");
        assert_eq!(media_extension(None, "/"), "mp3");
    }
}
