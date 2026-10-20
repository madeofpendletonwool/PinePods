//! In-process HTTP: build a request, push it through the production router, inspect the reply.

use axum::{
    body::Body,
    http::{HeaderMap, Method, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use super::app::{basic, TestUser};

pub struct RequestBuilder {
    router: Router,
    method: Method,
    path: String,
    query: Vec<(String, String)>,
    headers: Vec<(String, String)>,
    body: Option<(Vec<u8>, &'static str)>,
}

impl RequestBuilder {
    pub fn new(router: Router, method: Method, path: &str) -> Self {
        RequestBuilder {
            router,
            method,
            path: path.to_string(),
            query: Vec::new(),
            headers: Vec::new(),
            body: None,
        }
    }

    /// Authenticate as `user` (the `Api-Key` header every client sends).
    pub fn key(self, user: &TestUser) -> Self {
        self.api_key(&user.api_key)
    }

    pub fn api_key(self, key: &str) -> Self {
        self.header("Api-Key", key)
    }

    pub fn basic_auth(self, username: &str, password: &str) -> Self {
        self.header("Authorization", &basic(username, password))
    }

    pub fn header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.to_string(), v.to_string()));
        self
    }

    pub fn query(mut self, k: &str, v: impl ToString) -> Self {
        self.query.push((k.to_string(), v.to_string()));
        self
    }

    pub fn json(mut self, v: &Value) -> Self {
        self.body = Some((serde_json::to_vec(v).unwrap(), "application/json"));
        self
    }

    pub fn raw_body(mut self, bytes: Vec<u8>, content_type: &'static str) -> Self {
        self.body = Some((bytes, content_type));
        self
    }

    pub fn uri(&self) -> String {
        if self.query.is_empty() {
            self.path.clone()
        } else {
            let qs = url::form_urlencoded::Serializer::new(String::new())
                .extend_pairs(self.query.iter())
                .finish();
            format!("{}?{}", self.path, qs)
        }
    }

    pub async fn send(self) -> TestResponse {
        let uri = self.uri();
        let mut req = Request::builder().method(self.method.clone()).uri(&uri);
        for (k, v) in &self.headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let body = match self.body {
            Some((bytes, ct)) => {
                req = req.header("Content-Type", ct);
                Body::from(bytes)
            }
            None => Body::empty(),
        };
        let resp = self.router.oneshot(req.body(body).unwrap()).await.expect("router is infallible");
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.expect("read body").to_bytes().to_vec();
        TestResponse { method: self.method, uri, status, headers, body: bytes }
    }
}

pub struct TestResponse {
    pub method: Method,
    pub uri: String,
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl TestResponse {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    /// Parsed JSON body; panics with the raw body if it isn't JSON.
    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("{} {} → {}: body is not JSON ({e}): {}", self.method, self.uri, self.status, self.text()))
    }

    pub fn json_as<T: serde::de::DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.body)
            .unwrap_or_else(|e| panic!("{} {} → {}: unexpected JSON shape ({e}): {}", self.method, self.uri, self.status, self.text()))
    }

    #[track_caller]
    pub fn assert_status(&self, expected: u16) -> &Self {
        assert_eq!(
            self.status.as_u16(),
            expected,
            "{} {} → expected {expected}, got {}: {}",
            self.method,
            self.uri,
            self.status,
            self.text()
        );
        self
    }

    #[track_caller]
    pub fn assert_ok(&self) -> &Self {
        self.assert_status(200)
    }

    /// Assert 200 and return the JSON body.
    #[track_caller]
    pub fn ok_json(&self) -> Value {
        self.assert_ok();
        self.json()
    }

    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}
