//! Downloads and the media server end to end, against a local HTTP server.

use crate::downloads::{DownloadRequest, Downloads, EVENT};
use crate::library::Library;
use crate::media_server::MediaServer;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Listener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const AUDIO_LEN: usize = 300_000;

/// Serves `/audio` (audio/mp4), `/slow` (trickles forever), `/art` (png) and 404 otherwise.
fn start_origin() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let addr = listener.local_addr().unwrap();
    tauri::async_runtime::spawn(async move {
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();
        loop {
            let Ok((mut sock, _)) = listener.accept().await else { continue };
            tokio::spawn(async move {
                let mut buf = [0u8; 2048];
                let n = sock.read(&mut buf).await.unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                match path.as_str() {
                    "/audio" => {
                        let head = format!("HTTP/1.1 200 OK\r\nContent-Type: audio/mp4\r\nContent-Length: {AUDIO_LEN}\r\nConnection: close\r\n\r\n");
                        let _ = sock.write_all(head.as_bytes()).await;
                        let _ = sock.write_all(&vec![7u8; AUDIO_LEN]).await;
                    }
                    "/art" => {
                        let _ = sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: 3\r\nConnection: close\r\n\r\npng").await;
                    }
                    "/slow" => {
                        let _ = sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nContent-Length: 100000000\r\nConnection: close\r\n\r\n").await;
                        loop {
                            if sock.write_all(&[1u8; 1024]).await.is_err() {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(20)).await;
                        }
                    }
                    _ => {
                        let _ = sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                    }
                }
            });
        }
    });
    format!("http://{addr}")
}

fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pinepods-pipe-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Harness {
    app: tauri::App<tauri::test::MockRuntime>,
    events: Arc<Mutex<Vec<Value>>>,
    library: Arc<Mutex<Library>>,
    downloads: Arc<Downloads>,
    dir: PathBuf,
}

fn harness(name: &str) -> Harness {
    let app = tauri::test::mock_app();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    app.handle().listen_any(EVENT, move |e| {
        sink.lock().unwrap().push(serde_json::from_str(e.payload()).unwrap());
    });
    let dir = tmpdir(name);
    Harness {
        app,
        events,
        library: Arc::new(Mutex::new(Library::open(&dir))),
        downloads: Arc::new(Downloads::new()),
        dir,
    }
}

impl Harness {
    fn start(&self, id: i32, media_url: String, artwork_url: Option<String>) {
        let request = DownloadRequest {
            episode: json!({ "episodeid": id, "podcastid": 3, "episodetitle": format!("Episode {id}") }),
            podcast: None,
            media_url,
            artwork_url,
            automatic: true,
        };
        assert!(self.downloads.start(self.app.handle().clone(), self.library.clone(), request).unwrap());
    }

    /// Wait for the episode's final event (complete, failed or cancelled).
    fn wait_end(&self, id: i32) -> Value {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let found = self.events.lock().unwrap().iter().rev().find(|e| {
                e["episode_id"] == json!(id)
                    && matches!(e["phase"].as_str(), Some("complete" | "failed" | "cancelled"))
            }).cloned();
            if let Some(e) = found {
                return e;
            }
            assert!(Instant::now() < deadline, "no final event for {id}: {:?}", self.events.lock().unwrap());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn phases(&self, id: i32) -> Vec<String> {
        self.events.lock().unwrap().iter()
            .filter(|e| e["episode_id"] == json!(id))
            .map(|e| e["phase"].as_str().unwrap().to_string())
            .collect()
    }
}

#[test]
fn downloads_rename_and_index() {
    let origin = start_origin();
    let h = harness("ok");
    h.start(1, format!("{origin}/audio"), Some(format!("{origin}/art")));
    let end = h.wait_end(1);
    assert_eq!(end["phase"], "complete", "{end}");
    assert_eq!(end["received"], json!(AUDIO_LEN));
    let phases = h.phases(1);
    assert_eq!(phases.first().map(String::as_str), Some("queued"));
    assert!(phases.contains(&"downloading".to_string()));

    let lib = h.library.lock().unwrap();
    let rec = &lib.episodes()[0];
    assert_eq!(rec.file, "1.m4a");
    assert_eq!(rec.artwork_file.as_deref(), Some("1-art.png"));
    assert!(rec.automatic, "the automatic flag is kept");
    assert_eq!(std::fs::metadata(h.dir.join("downloads/1.m4a")).unwrap().len(), AUDIO_LEN as u64);
    assert!(!h.dir.join("downloads/1.part").exists());
    assert!(h.downloads.active().is_empty());
}

#[test]
fn server_error_fails_cleanly() {
    let origin = start_origin();
    let h = harness("fail");
    h.start(2, format!("{origin}/missing"), None);
    let end = h.wait_end(2);
    assert_eq!(end["phase"], "failed");
    assert!(end["error"].as_str().unwrap().contains("404"), "{end}");
    assert!(h.library.lock().unwrap().episodes().is_empty());
    assert!(!h.dir.join("downloads/2.part").exists());
}

#[test]
fn cancel_stops_and_cleans_up() {
    let origin = start_origin();
    let h = harness("cancel");
    h.start(3, format!("{origin}/slow"), None);
    // A second request for the same episode is ignored while the first runs.
    let again = DownloadRequest {
        episode: json!({ "episodeid": 3 }),
        podcast: None,
        media_url: format!("{origin}/slow"),
        artwork_url: None,
        automatic: false,
    };
    assert!(!h.downloads.start(h.app.handle().clone(), h.library.clone(), again).unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    while !h.phases(3).contains(&"downloading".to_string()) {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(300));
    h.downloads.cancel(3);
    let end = h.wait_end(3);
    assert_eq!(end["phase"], "cancelled");
    std::thread::sleep(Duration::from_millis(100));
    assert!(!h.dir.join("downloads/3.part").exists());
    assert!(h.library.lock().unwrap().episodes().is_empty());
}

#[test]
fn media_server_serves_only_downloads_with_ranges() {
    let dir = tmpdir("media");
    std::fs::write(dir.join("secret.json"), b"api key").unwrap();
    let downloads = dir.join("downloads");
    std::fs::create_dir_all(&downloads).unwrap();
    std::fs::write(downloads.join("5.mp3"), b"0123456789abcdef").unwrap();
    let server = MediaServer::start(downloads).unwrap();
    let base = server.base_url().to_string();
    let root = base.rsplit_once('/').unwrap().0.to_string();

    tauri::async_runtime::block_on(async move {
        let client = reqwest::Client::new();
        // Give the listener a moment to start accepting.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let r = client.get(format!("{base}/5.mp3")).header("Range", "bytes=2-5").send().await.unwrap();
        assert_eq!(r.status(), 206);
        assert_eq!(r.headers()["access-control-allow-origin"], "*");
        assert_eq!(r.bytes().await.unwrap().as_ref(), b"2345");

        for url in [format!("{root}/5.mp3"), format!("{base}/../secret.json"), format!("{root}/x/5.mp3")] {
            let status = client.get(&url).send().await.unwrap().status();
            assert_eq!(status, 404, "{url}");
        }
    });
}
