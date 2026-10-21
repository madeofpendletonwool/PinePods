//! A loopback HTTP server that plays the outside world: podcast RSS feeds, enclosure audio,
//! artwork, and the external APIs (search, podpeople) the server talks to.
//!
//! One server per test process, on its own thread + runtime so it outlives each test's
//! runtime. Tests isolate themselves by path: [`Fixtures`] (from `TestApp::fixtures()`)
//! prefixes every path with the test's unique namespace. Responses can be replaced at any
//! time (e.g. publish a new episode, then refresh) and every request is recorded.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, Method, StatusCode, Uri},
    response::Response,
    Router,
};

#[derive(Clone, Debug)]
pub struct Fixture {
    pub status: u16,
    pub content_type: String,
    pub body: Bytes,
    /// When set, served as `ETag` and honoured for `If-None-Match` → 304.
    pub etag: Option<String>,
    pub extra_headers: Vec<(String, String)>,
}

impl Fixture {
    pub fn new(content_type: &str, body: impl Into<Bytes>) -> Self {
        Fixture {
            status: 200,
            content_type: content_type.to_string(),
            body: body.into(),
            etag: None,
            extra_headers: Vec::new(),
        }
    }
    pub fn status(mut self, status: u16) -> Self {
        self.status = status;
        self
    }
    pub fn etag(mut self, etag: &str) -> Self {
        self.etag = Some(etag.to_string());
        self
    }
    pub fn header(mut self, k: &str, v: &str) -> Self {
        self.extra_headers.push((k.to_string(), v.to_string()));
        self
    }
}

#[derive(Clone, Debug)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    pub query: Option<String>,
    pub headers: HashMap<String, String>,
}

#[derive(Default)]
struct Inner {
    routes: HashMap<String, Fixture>,
    log: Vec<RecordedRequest>,
}

#[derive(Clone)]
pub struct FixtureServer {
    pub addr: SocketAddr,
    inner: Arc<Mutex<Inner>>,
}

impl FixtureServer {
    pub fn start() -> Self {
        let inner = Arc::new(Mutex::new(Inner::default()));
        let (tx, rx) = std::sync::mpsc::channel();
        let state = inner.clone();
        std::thread::Builder::new()
            .name("fixture-server".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async move {
                    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                    tx.send(listener.local_addr().unwrap()).unwrap();
                    let app = Router::new().fallback(serve).with_state(state);
                    axum::serve(listener, app).await.unwrap();
                });
            })
            .unwrap();
        let addr = rx.recv().expect("fixture server failed to start");
        FixtureServer { addr, inner }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn put(&self, path: &str, fixture: Fixture) {
        self.inner.lock().unwrap().routes.insert(path.to_string(), fixture);
    }

    pub fn remove(&self, path: &str) {
        self.inner.lock().unwrap().routes.remove(path);
    }

    /// Paths requested under `/{namespace}{prefix}`, in arrival order, with that prefix removed.
    pub fn request_order(&self, namespace: &str, prefix: &str) -> Vec<String> {
        let full = format!("/{namespace}{prefix}");
        self.inner
            .lock()
            .unwrap()
            .log
            .iter()
            .filter_map(|r| r.path.strip_prefix(&full).map(str::to_string))
            .collect()
    }

    pub fn requests_to(&self, path: &str) -> Vec<RecordedRequest> {
        self.inner
            .lock()
            .unwrap()
            .log
            .iter()
            .filter(|r| r.path == path)
            .cloned()
            .collect()
    }
}

async fn serve(
    State(inner): State<Arc<Mutex<Inner>>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let path = uri.path().to_string();
    let fixture = {
        let mut inner = inner.lock().unwrap();
        inner.log.push(RecordedRequest {
            method: method.to_string(),
            path: path.clone(),
            query: uri.query().map(str::to_string),
            headers: headers
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or_default().to_string()))
                .collect(),
        });
        inner.routes.get(&path).cloned()
    };

    let Some(f) = fixture else {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from(format!("no fixture registered for {path}")))
            .unwrap();
    };

    if let (Some(etag), Some(inm)) = (&f.etag, headers.get("if-none-match")) {
        if inm.to_str().ok() == Some(etag.as_str()) {
            return Response::builder()
                .status(StatusCode::NOT_MODIFIED)
                .header("ETag", etag.as_str())
                .body(Body::empty())
                .unwrap();
        }
    }

    let mut resp = Response::builder()
        .status(StatusCode::from_u16(f.status).unwrap())
        .header("Content-Type", f.content_type.as_str())
        .header("Content-Length", f.body.len().to_string());
    if let Some(etag) = &f.etag {
        resp = resp.header("ETag", etag.as_str());
    }
    for (k, v) in &f.extra_headers {
        resp = resp.header(k.as_str(), HeaderValue::from_str(v).unwrap());
    }
    let body = if method == Method::HEAD { Body::empty() } else { Body::from(f.body) };
    resp.body(body).unwrap()
}

/// A test's private view of the fixture server: every path is namespaced so parallel tests
/// can't see each other's feeds.
#[derive(Clone)]
pub struct Fixtures {
    pub server: FixtureServer,
    pub namespace: String,
}

impl Fixtures {
    fn full_path(&self, path: &str) -> String {
        format!("/{}/{}", self.namespace, path.trim_start_matches('/'))
    }

    /// Serve `fixture` at `path` (relative to this test's namespace); returns its URL.
    pub fn serve(&self, path: &str, fixture: Fixture) -> String {
        let full = self.full_path(path);
        self.server.put(&full, fixture);
        format!("{}{}", self.server.base_url(), full)
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{}", self.server.base_url(), self.full_path(path))
    }

    pub fn remove(&self, path: &str) {
        self.server.remove(&self.full_path(path));
    }

    pub fn hits(&self, path: &str) -> Vec<RecordedRequest> {
        self.server.requests_to(&self.full_path(path))
    }

    /// Serve an RSS document (see [`super::rss::Feed`]); returns the feed URL.
    pub fn serve_feed(&self, path: &str, xml: String) -> String {
        self.serve(path, Fixture::new("application/rss+xml; charset=utf-8", xml))
    }
}
