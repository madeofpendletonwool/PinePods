//! The gpodder protocol under test, spoken over real HTTP.
//!
//! Which server answers is chosen by `PINEPODS_TEST_GPODDER`:
//!
//! - `go` (default): the current Go service (`gpodder-api/`), built once per test process and
//!   started per test against that test's database, exactly as production runs it next to
//!   rust-api.
//! - `rust`: the rust-api router itself, served on a real socket (the Phase 5 port).
//! - `mock`: the in-memory reference server in [`super::mock_gpodder`]. It has no Pinepods
//!   library behind it, so tests that look at the library skip themselves; everything else
//!   must pass, which is how the protocol tests themselves are checked.
//!
//! Tests describe findings the current server is known to fail with [`conformance`], so the
//! suite stays green while documenting them, and flips to a hard failure the moment a finding
//! is fixed (so its entry gets removed) or a new one appears.

use std::future::Future;
use std::io::Write as _;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use base64::Engine as _;
use serde_json::{json, Value};

use super::app::{TestApp, TestUser};
use super::env::{repo_root, Backend};
use super::mock_gpodder::MockGpodder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Impl {
    Go,
    Rust,
    Mock,
}

impl Impl {
    pub fn current() -> Impl {
        match std::env::var("PINEPODS_TEST_GPODDER").unwrap_or_default().to_lowercase().as_str() {
            "" | "go" => Impl::Go,
            "rust" => Impl::Rust,
            "mock" => Impl::Mock,
            other => panic!("PINEPODS_TEST_GPODDER={other}: expected go, rust or mock"),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Impl::Go => "go",
            Impl::Rust => "rust",
            Impl::Mock => "mock",
        }
    }
}

// ---- known failures ----------------------------------------------------------------------

/// Where a finding is known to fail today.
#[derive(Clone, Copy, Debug)]
pub enum Known {
    /// Expected to pass everywhere.
    None,
    /// The Go service fails it on every backend.
    Go,
    /// The Go service fails it on MariaDB only.
    GoMariaDb,
}

impl Known {
    fn applies(self, imp: Impl, backend: Backend) -> bool {
        match self {
            Known::None => false,
            Known::Go => imp == Impl::Go,
            Known::GoMariaDb => imp == Impl::Go && backend == Backend::MariaDb,
        }
    }
}

/// Run one protocol check. If `known` says the current server fails `finding`, a failure is
/// recorded (stderr + `target/gpodder-conformance/<impl>-<backend>.txt`) instead of failing
/// the test, and an unexpected pass fails it so the stale entry gets removed.
/// `PINEPODS_TEST_GPODDER_STRICT=1` turns the tolerance off.
pub async fn conformance<Fut>(finding: &str, known: Known, body: Fut)
where
    Fut: Future<Output = ()> + Send + 'static,
{
    let imp = Impl::current();
    let backend = super::env::env().backend;
    let tolerated = known.applies(imp, backend) && std::env::var("PINEPODS_TEST_GPODDER_STRICT").is_err();
    let outcome = tokio::spawn(body).await;
    match (outcome, tolerated) {
        (Ok(()), false) => {}
        (Ok(()), true) => panic!(
            "{finding} now passes on {}/{}: remove it from this test's known failures",
            imp.name(),
            backend.db_type()
        ),
        (Err(e), true) if e.is_panic() => {
            let payload = e.into_panic();
            let msg = payload
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "<non-string panic>".into());
            let first_line = msg.lines().next().unwrap_or_default();
            eprintln!("[known failure {finding} on {}/{}] {first_line}", imp.name(), backend.db_type());
            record_known_failure(imp, backend, finding, first_line);
        }
        (Err(e), _) => std::panic::resume_unwind(e.into_panic()),
    }
}

fn record_known_failure(imp: Impl, backend: Backend, finding: &str, msg: &str) {
    let dir = repo_root().join("rust-api/target/gpodder-conformance");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{}-{}.txt", imp.name(), backend.db_type())))
    {
        // One write per line: tests run in parallel and append to the same file.
        let _ = f.write_all(format!("{finding}\t{msg}\n").as_bytes());
    }
}

// ---- servers ------------------------------------------------------------------------------

/// A running gpodder server for one test. Declare it before the `TestApp` it serves so it is
/// dropped (and the Go process killed, releasing its connections) before the database is.
pub struct GpodderServer {
    pub imp: Impl,
    pub base_url: String,
    go: Option<GoProcess>,
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

struct GoProcess {
    child: Child,
    log: PathBuf,
}

impl Drop for GoProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl GoProcess {
    fn log_tail(&self) -> String {
        let s = std::fs::read_to_string(&self.log).unwrap_or_default();
        s.lines().rev().take(30).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")
    }
}

/// The Go binary, built once per test process (`go build` is incremental, so this is cheap
/// after the first run). `PINEPODS_TEST_GPODDER_BIN` points at a prebuilt one instead.
fn go_binary() -> &'static PathBuf {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        if let Ok(p) = std::env::var("PINEPODS_TEST_GPODDER_BIN") {
            return PathBuf::from(p);
        }
        std::thread::spawn(|| {
            let root = repo_root();
            let out = root.join("rust-api/target/gpodder-api-test");
            let status = Command::new("go")
                .args(["build", "-o"])
                .arg(&out)
                .arg("./cmd/server")
                .current_dir(root.join("gpodder-api"))
                // The test process points every proxy variable at a dead port (env.rs); the
                // build itself may need to download modules.
                .env_remove("HTTP_PROXY")
                .env_remove("HTTPS_PROXY")
                .env_remove("ALL_PROXY")
                .env_remove("http_proxy")
                .env_remove("https_proxy")
                .env_remove("all_proxy")
                .status()
                .unwrap_or_else(|e| {
                    panic!("could not run `go build` for gpodder-api ({e}); install Go or set PINEPODS_TEST_GPODDER_BIN")
                });
            assert!(status.success(), "go build of gpodder-api failed");
            out
        })
        .join()
        .expect("building gpodder-api")
    })
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

async fn wait_listening(port: u16, mut alive: impl FnMut() -> Result<(), String>) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }
        if let Err(e) = alive() {
            panic!("gpodder server exited before listening: {e}");
        }
        if Instant::now() > deadline {
            panic!("gpodder server did not start listening on {port}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

impl TestApp {
    /// Start the gpodder server under test for this app's database.
    pub async fn gpodder_server(&self) -> GpodderServer {
        let imp = Impl::current();
        match imp {
            Impl::Go => {
                let env = self.env;
                let port = free_port();
                let log_dir = repo_root().join("rust-api/target/gpodder-test-logs");
                std::fs::create_dir_all(&log_dir).unwrap();
                let log = log_dir.join(format!("{}.log", self.db_name));
                let out = std::fs::File::create(&log).unwrap();
                let child = Command::new(go_binary())
                    // Its .env loader reads the working directory; keep a developer's out of it.
                    .current_dir(&log_dir)
                    // Production passes the user's DB_TYPE through unchanged ("mariadb" for
                    // MariaDB installs), so the tests do too.
                    .env("DB_TYPE", env.backend.db_type())
                    .env("DB_HOST", &env.db_host)
                    .env("DB_PORT", env.db_port.to_string())
                    .env("DB_USER", &env.db_user)
                    .env("DB_PASSWORD", &env.db_password)
                    .env("DB_NAME", &self.db_name)
                    .env("SERVER_PORT", port.to_string())
                    .env_remove("DEBUG_MODE")
                    .stdin(Stdio::null())
                    .stdout(out.try_clone().unwrap())
                    .stderr(out)
                    .spawn()
                    .expect("start gpodder-api");
                let mut proc = GoProcess { child, log };
                wait_listening(port, || match proc.child.try_wait() {
                    Ok(Some(status)) => Err(format!("{status}\n{}", proc.log_tail())),
                    _ => Ok(()),
                })
                .await;
                GpodderServer { imp, base_url: format!("http://127.0.0.1:{port}"), go: Some(proc), serve: None, mock: None }
            }
            Impl::Rust => {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let port = listener.local_addr().unwrap().port();
                let router = self.router.clone();
                let serve = tokio::spawn(async move {
                    axum::serve(listener, router).await.unwrap();
                });
                GpodderServer { imp, base_url: format!("http://127.0.0.1:{port}"), go: None, serve: Some(serve), mock: None }
            }
            Impl::Mock => {
                let mock = MockGpodder::start().await;
                GpodderServer { imp, base_url: mock.base_url.clone(), go: None, serve: None, mock: Some(mock) }
            }
        }
    }

    /// Turn on the internal gpodder server for `user` the way enabling it in Settings does,
    /// minus the HTTP calls rust-api then makes to the hard-coded `localhost:8042`.
    pub async fn enable_gpodder(&self, server: &GpodderServer, user: &TestUser) {
        use pinepods_api::database::DatabasePool;
        match server.imp {
            Impl::Go => {
                let token: String = {
                    use rand::RngExt as _;
                    rand::rng().sample_iter(rand::distr::Alphanumeric).take(64).map(char::from).collect()
                };
                match self.db() {
                    DatabasePool::Postgres(pool) => sqlx::query(
                        r#"UPDATE "Users" SET gpodderurl = 'http://localhost:8042', gpoddertoken = $1,
                           gpodderloginname = $2, pod_sync_type = 'gpodder' WHERE userid = $3"#,
                    )
                    .bind(&token)
                    .bind(&user.username)
                    .bind(user.id)
                    .execute(pool)
                    .await
                    .map(|_| ())
                    .unwrap(),
                    DatabasePool::MySQL(pool) => sqlx::query(
                        "UPDATE Users SET GpodderUrl = 'http://localhost:8042', GpodderToken = ?, \
                         GpodderLoginName = ?, Pod_Sync_Type = 'gpodder' WHERE UserID = ?",
                    )
                    .bind(&token)
                    .bind(&user.username)
                    .bind(user.id)
                    .execute(pool)
                    .await
                    .map(|_| ())
                    .unwrap(),
                };
            }
            Impl::Rust => unimplemented!("Phase 5: enable the Rust gpodder server through its settings endpoint"),
            Impl::Mock => server.mock.as_ref().unwrap().add_user(&user.username, &user.password),
        }
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
pub struct GpodderScene {
    pub server: GpodderServer,
    pub app: TestApp,
    pub user: TestUser,
}

impl GpodderScene {
    pub async fn start() -> GpodderScene {
        let app = TestApp::new().await;
        let user = app.user("listener").await;
        let server = app.gpodder_server().await;
        app.enable_gpodder(&server, &user).await;
        GpodderScene { server, app, user }
    }

    /// True when the server shares Pinepods' database, so library state can be asserted.
    pub fn has_library(&self) -> bool {
        self.server.imp != Impl::Mock
    }

    /// A logged-in client for `device`, as AntennaPod sets one up.
    pub async fn device(&self, device: &str) -> GpodderClient {
        let c = GpodderClient::new(&self.server.base_url, &self.user.username, &self.user.password, device);
        c.login().await.assert_ok();
        c
    }

    /// A second Pinepods user with gpodder enabled, on the same server.
    pub async fn other_user(&self, name: &str) -> TestUser {
        let u = self.app.user(name).await;
        self.app.enable_gpodder(&self.server, &u).await;
        u
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
