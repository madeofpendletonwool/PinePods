//! Serves downloaded episodes to the app's player.
//!
//! The webview can't play files straight off disk on every platform, so the downloads folder
//! is served over HTTP on a random local port. Nothing else is served, and every URL starts
//! with a random token made at launch, so other programs on the machine can't browse it.

use std::net::TcpListener;
use std::path::PathBuf;
use warp::Filter;

pub struct MediaServer {
    base_url: String,
}

impl MediaServer {
    pub fn start(downloads_dir: PathBuf) -> std::io::Result<MediaServer> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let token = random_token();

        // warp::fs answers Range requests, so seeking works.
        let routes = warp::path(token.clone())
            .and(warp::fs::dir(downloads_dir))
            // Artwork colours are read through a canvas, which needs CORS.
            .with(warp::reply::with::header("Access-Control-Allow-Origin", "*"))
            .with(warp::reply::with::header("Cache-Control", "no-store"));

        tauri::async_runtime::spawn(async move {
            match tokio::net::TcpListener::from_std(listener) {
                Ok(listener) => warp::serve(routes).incoming(listener).run().await,
                Err(e) => log::error!("Media server could not start: {e}"),
            }
        });
        Ok(MediaServer { base_url: format!("http://127.0.0.1:{port}/{token}") })
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

fn random_token() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("system random number generator");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
