//! gpodder protocol conformance.
//!
//! These tests speak HTTP to whichever gpodder server `PINEPODS_TEST_GPODDER` selects (see
//! `harness/gpodder.rs`): the rust-api router, or the in-memory reference server that keeps
//! the tests themselves honest.
//!
//! `scripts/integration-tests.sh --gpodder mock -- gpodder::` runs only these against the mock.

mod antennapod_rules;
mod mock_server;
mod protocol;
mod scenarios;

use serde_json::Value;

/// The actions in a GET response for `episode`.
pub fn actions_for<'a>(body: &'a Value, episode: &str) -> Vec<&'a Value> {
    body["actions"].as_array().map(|a| a.iter().filter(|x| x["episode"] == episode).collect()).unwrap_or_default()
}

/// The strings in `body[key]`.
pub fn strings(body: &Value, key: &str) -> Vec<String> {
    body[key].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

/// A TCP server that accepts connections and never answers: a feed host that hangs.
pub async fn stalled_host() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((sock, _)) = listener.accept().await {
            held.push(sock);
        }
    });
    format!("http://{addr}")
}
