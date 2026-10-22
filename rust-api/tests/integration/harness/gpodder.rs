//! The gpodder protocol under test, spoken over real HTTP.
//!
//! Which server answers is chosen by `PINEPODS_TEST_GPODDER`:
//!
//! - `rust` (default): the rust-api router itself, served on a real socket.
//! - `mock`: the in-memory reference server in [`super::mock_gpodder`]. It has no Pinepods
//!   library behind it, so tests that look at the library skip themselves; everything else
//!   must pass, which is how the protocol tests themselves are checked.

use std::future::Future;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{json, Value};

use super::app::{TestApp, TestUser};
use super::mock_gpodder::MockGpodder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Impl {
    Rust,
    Mock,
}

impl Impl {
    pub fn current() -> Impl {
        match std::env::var("PINEPODS_TEST_GPODDER").unwrap_or_default().to_lowercase().as_str() {
            "" | "rust" => Impl::Rust,
            "mock" => Impl::Mock,
            other => panic!("PINEPODS_TEST_GPODDER={other}: expected rust or mock"),
        }
    }
}

/// Run one protocol check, naming it in the failure message.
pub async fn conformance<Fut>(check: &str, body: Fut)
where
    Fut: Future<Output = ()> + Send + 'static,
{
    if let Err(e) = tokio::spawn(body).await {
        eprintln!("[gpodder check failed: {check}]");
        std::panic::resume_unwind(e.into_panic());
    }
}

// ---- servers ------------------------------------------------------------------------------

/// A running gpodder server for one test.
pub struct GpodderServer {
    pub imp: Impl,
    pub base_url: String,
    serve: Option<tokio::task::JoinHandle<()>>,
    pub mock: Option<MockGpodder>,
}

impl Drop for GpodderServer {
    fn drop(&mut self) {
        if let Some(h) = self.serve.take() {
            h.abort();
        }
    }
}

impl TestApp {
    /// Start the gpodder server under test for this app's database.
    pub async fn gpodder_server(&self) -> GpodderServer {
        let imp = Impl::current();
        match imp {
            Impl::Rust => self.rust_gpodder_server().await,
            Impl::Mock => {
                let mock = MockGpodder::start().await;
                GpodderServer { imp, base_url: mock.base_url.clone(), serve: None, mock: Some(mock) }
            }
        }
    }

    /// The rust-api router on a real socket, whatever `PINEPODS_TEST_GPODDER` says.
    pub async fn rust_gpodder_server(&self) -> GpodderServer {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let router = self.router.clone();
        let serve = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        GpodderServer { imp: Impl::Rust, base_url: format!("http://127.0.0.1:{port}"), serve: Some(serve), mock: None }
    }

    /// Turn on the gpodder server for `user` the way the Settings page does. Returns the
    /// password apps sign in with: a new app password (the mock takes the account password).
    pub async fn enable_gpodder(&self, server: &GpodderServer, user: &TestUser) -> String {
        match server.imp {
            Impl::Rust => {
                self.post("/api/data/gpodder/toggle").key(user).json(&json!({ "enabled": true })).send().await.assert_ok();
                self.app_password(user).await
            }
            Impl::Mock => {
                server.mock.as_ref().unwrap().add_user(&user.username, &user.password);
                user.password.clone()
            }
        }
    }

    /// A new app password for `user`'s gpodder apps.
    pub async fn app_password(&self, user: &TestUser) -> String {
        let created = self.post("/api/sync/server/app-passwords").key(user).json(&json!({ "label": "test app" })).send().await.ok_json();
        created["password"].as_str().unwrap_or_else(|| panic!("no password in {created}")).to_string()
    }
}

impl TestApp {
    /// Sessions on this test's database sitting inside a transaction they never finished
    /// (Postgres `idle in transaction`, InnoDB transactions of sleeping connections).
    pub async fn abandoned_transactions(&self) -> i64 {
        use pinepods_api::database::DatabasePool;
        match self.db() {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(
                "SELECT COUNT(*) FROM pg_stat_activity \
                 WHERE datname = current_database() AND state LIKE 'idle in transaction%'",
            )
            .fetch_one(pool)
            .await
            .unwrap(),
            DatabasePool::MySQL(pool) => sqlx::query_scalar(
                "SELECT COUNT(*) FROM information_schema.innodb_trx t \
                 JOIN information_schema.processlist p ON p.id = t.trx_mysql_thread_id \
                 WHERE p.db = DATABASE() AND p.command = 'Sleep'",
            )
            .fetch_one(pool)
            .await
            .unwrap(),
        }
    }
}

// ---- scene ------------------------------------------------------------------------------

/// One Pinepods instance with a user who has the internal gpodder server switched on.
/// `password` is what that user's apps sign in with.
pub struct GpodderScene {
    pub server: GpodderServer,
    pub app: TestApp,
    pub user: TestUser,
    pub password: String,
}

impl GpodderScene {
    pub async fn start() -> GpodderScene {
        let app = TestApp::new().await;
        let user = app.user("listener").await;
        let server = app.gpodder_server().await;
        let password = app.enable_gpodder(&server, &user).await;
        GpodderScene { server, app, user, password }
    }

    /// True when the server shares Pinepods' database, so library state can be asserted.
    pub fn has_library(&self) -> bool {
        self.server.imp != Impl::Mock
    }

    /// A logged-in client for `device`, as AntennaPod sets one up.
    pub async fn device(&self, device: &str) -> GpodderClient {
        let c = GpodderClient::new(&self.server.base_url, &self.user.username, &self.password, device);
        c.login().await.assert_ok();
        c
    }

    /// A second Pinepods user with gpodder enabled, on the same server, and their app password.
    pub async fn other_user(&self, name: &str) -> (TestUser, String) {
        let u = self.app.user(name).await;
        let password = self.app.enable_gpodder(&self.server, &u).await;
        (u, password)
    }
}

// ---- client -----------------------------------------------------------------------------

/// What AntennaPod's sync client sends: log in once with HTTP Basic, then ride the session
/// cookie; subscription paths carry the `.json` suffix; actions are uploaded in batches of 30.
pub struct GpodderClient {
    http: reqwest::Client,
    pub base: String,
    pub username: String,
    pub password: String,
    pub device: String,
}

pub const UPLOAD_BULK_SIZE: usize = 30;

pub struct GResp {
    pub status: u16,
    pub body: String,
    pub elapsed: Duration,
    pub what: String,
}

impl GResp {
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.body)
            .unwrap_or_else(|e| panic!("{}: body is not one JSON document ({e}): {}", self.what, self.body))
    }
    pub fn assert_status(&self, expected: u16) -> &Self {
        assert_eq!(self.status, expected, "{}: expected {expected}, got {}: {}", self.what, self.status, self.body);
        self
    }
    pub fn assert_ok(&self) -> &Self {
        self.assert_status(200)
    }
    pub fn ok_json(&self) -> Value {
        self.assert_ok();
        self.json()
    }
}

impl GpodderClient {
    pub fn new(base: &str, username: &str, password: &str, device: &str) -> Self {
        let http = reqwest::Client::builder()
            .cookie_store(true)
            .no_proxy()
            .timeout(Duration::from_secs(30)) // AntennaPod's read timeout
            .build()
            .unwrap();
        GpodderClient {
            http,
            base: base.trim_end_matches('/').to_string(),
            username: username.to_string(),
            password: password.to_string(),
            device: device.to_string(),
        }
    }

    pub fn basic_header(username: &str, password: &str) -> String {
        format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}")))
    }

    /// Send any request. `auth` adds HTTP Basic with this client's credentials on top of the
    /// session cookie.
    pub async fn send(&self, method: reqwest::Method, path: &str, body: Option<&Value>, auth: Option<String>) -> GResp {
        let what = format!("{method} {path}");
        let mut req = self.http.request(method, format!("{}{}", self.base, path));
        if let Some(h) = auth {
            req = req.header("Authorization", h);
        }
        if let Some(b) = body {
            req = req.json(b);
        }
        let start = Instant::now();
        let resp = req.send().await.unwrap_or_else(|e| panic!("{what}: {e}"));
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        GResp { status, body, elapsed: start.elapsed(), what }
    }

    pub async fn get(&self, path: &str) -> GResp {
        self.send(reqwest::Method::GET, path, None, None).await
    }
    pub async fn post(&self, path: &str, body: &Value) -> GResp {
        self.send(reqwest::Method::POST, path, Some(body), None).await
    }
    pub async fn put(&self, path: &str, body: &Value) -> GResp {
        self.send(reqwest::Method::PUT, path, Some(body), None).await
    }

    pub async fn login(&self) -> GResp {
        let what = format!("POST /api/2/auth/{}/login.json", self.username);
        let start = Instant::now();
        let resp = self
            .http
            .post(format!("{}/api/2/auth/{}/login.json", self.base, self.username))
            .header("Authorization", Self::basic_header(&self.username, &self.password))
            .header("Content-Type", "text/plain")
            .body("")
            .send()
            .await
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();
        GResp { status, body, elapsed: start.elapsed(), what }
    }

    pub async fn devices(&self) -> GResp {
        self.get(&format!("/api/2/devices/{}.json", self.username)).await
    }

    pub async fn configure_device(&self, caption: &str, kind: &str) -> GResp {
        self.post(
            &format!("/api/2/devices/{}/{}.json", self.username, self.device),
            &json!({ "caption": caption, "type": kind }),
        )
        .await
    }

    pub async fn subscription_changes(&self, since: i64) -> GResp {
        self.get(&format!("/api/2/subscriptions/{}/{}.json?since={since}", self.username, self.device)).await
    }

    pub async fn upload_subscription_changes(&self, add: &[&str], remove: &[&str]) -> GResp {
        self.post(
            &format!("/api/2/subscriptions/{}/{}.json", self.username, self.device),
            &json!({ "add": add, "remove": remove }),
        )
        .await
    }

    pub async fn episode_actions(&self, since: i64) -> GResp {
        self.get(&format!("/api/2/episodes/{}.json?since={since}", self.username)).await
    }

    /// Upload one batch as-is (no device stamping, no splitting).
    pub async fn upload_episode_batch(&self, actions: &[Value]) -> GResp {
        self.post(&format!("/api/2/episodes/{}.json", self.username), &Value::Array(actions.to_vec())).await
    }

    /// AntennaPod's upload: stamp each action with this device, POST in batches of 30, and
    /// return every batch's response (AntennaPod keeps the last one's timestamp).
    pub async fn upload_episode_actions(&self, actions: &[Value]) -> Vec<GResp> {
        let mut out = Vec::new();
        for chunk in actions.chunks(UPLOAD_BULK_SIZE) {
            let batch: Vec<Value> = chunk
                .iter()
                .map(|a| {
                    let mut a = a.clone();
                    a["device"] = json!(self.device);
                    a
                })
                .collect();
            let r = self.upload_episode_batch(&batch).await;
            r.assert_ok();
            out.push(r);
        }
        out
    }
}

/// A `play` action the way AntennaPod writes it (UTC timestamp without offset).
pub fn play_action(podcast: &str, episode: &str, ts: chrono::DateTime<chrono::Utc>, started: i64, position: i64, total: i64) -> Value {
    json!({
        "podcast": podcast,
        "episode": episode,
        "action": "play",
        "timestamp": ts.format("%Y-%m-%dT%H:%M:%S").to_string(),
        "started": started,
        "position": position,
        "total": total,
    })
}

pub fn simple_action(podcast: &str, episode: &str, action: &str, ts: chrono::DateTime<chrono::Utc>) -> Value {
    json!({
        "podcast": podcast,
        "episode": episode,
        "action": action,
        "timestamp": ts.format("%Y-%m-%dT%H:%M:%S").to_string(),
    })
}

/// Sleep until just after the next wall-clock second starts, leaving ~950ms of "same second".
pub async fn align_to_second_start() {
    let now = chrono::Utc::now();
    let ms_into = now.timestamp_subsec_millis() as u64;
    tokio::time::sleep(Duration::from_millis(1000 - ms_into + 20)).await;
}
