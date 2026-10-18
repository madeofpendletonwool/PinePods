use axum::{
    body::Body,
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::Response,
};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::error::AppError;

// The caller must authorize access to this path before serving it.
pub(super) async fn serve_episode_file(
    path: &str,
    method: Method,
    headers: &HeaderMap,
) -> Result<Response, AppError> {
    let mut request = Request::builder().method(method.clone()).uri("/");
    for name in [
        header::RANGE,
        header::IF_RANGE,
        header::IF_MODIFIED_SINCE,
        header::IF_NONE_MATCH,
        header::IF_UNMODIFIED_SINCE,
        header::IF_MATCH,
    ] {
        for value in headers.get_all(&name) {
            request = request.header(name.clone(), value);
        }
    }
    let request = request
        .body(Body::empty())
        .map_err(|e| AppError::external_error(format!("Failed to build request: {e}")))?;
    let retry_headers = request.headers().clone();
    let response = ServeFile::new(path)
        .oneshot(request)
        .await
        .map_err(|e| AppError::external_error(format!("Failed to serve file: {e}")))?;
    // ServeFile evaluates cache preconditions but does not implement If-Range.
    // Only a matching strong validator permits combining partial responses.
    if let Some(validator) = headers.get(header::IF_RANGE) {
        let matching = !validator.as_bytes().starts_with(b"W/")
            && (response.headers().get(header::ETAG) == Some(validator)
                || response.headers().get(header::LAST_MODIFIED) == Some(validator));
        if !matching
            && matches!(
                response.status(),
                StatusCode::PARTIAL_CONTENT | StatusCode::RANGE_NOT_SATISFIABLE
            )
        {
            let mut request = Request::builder()
                .method(method)
                .uri("/")
                .body(Body::empty())
                .map_err(|e| AppError::external_error(format!("Failed to build request: {e}")))?;
            *request.headers_mut() = retry_headers;
            request.headers_mut().remove(header::RANGE);
            request.headers_mut().remove(header::IF_RANGE);
            return ServeFile::new(path)
                .oneshot(request)
                .await
                .map(|response| response.map(Body::new))
                .map_err(|e| AppError::external_error(format!("Failed to serve file: {e}")));
        }
    }
    Ok(response.map(Body::new))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::to_bytes, http::StatusCode};
    use std::path::PathBuf;

    struct EpisodeFile(PathBuf);

    impl EpisodeFile {
        async fn new() -> Self {
            let path = std::env::temp_dir().join(format!("pinepods-{}.mp3", uuid::Uuid::new_v4()));
            tokio::fs::write(&path, b"0123456789").await.unwrap();
            Self(path)
        }

        async fn request(&self, method: Method, pairs: &[(&str, &str)]) -> Response {
            let mut headers = HeaderMap::new();
            for &(name, value) in pairs {
                headers.append(
                    name.parse::<header::HeaderName>().unwrap(),
                    value.parse().unwrap(),
                );
            }
            serve_episode_file(self.0.to_str().unwrap(), method, &headers)
                .await
                .unwrap()
        }
    }

    impl Drop for EpisodeFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[tokio::test]
    async fn full_file_and_seek_ranges_return_the_requested_bytes() {
        let file = EpisodeFile::new().await;
        let full = file.request(Method::GET, &[]).await;
        assert_eq!(full.status(), StatusCode::OK);
        assert_eq!(full.headers()[header::CONTENT_LENGTH], "10");
        assert_eq!(to_bytes(full.into_body(), 10).await.unwrap(), "0123456789");

        for (range, content_range, expected) in [
            ("bytes=2-5", "bytes 2-5/10", "2345"),
            ("bytes=7-", "bytes 7-9/10", "789"),
            ("bytes=-2", "bytes 8-9/10", "89"),
        ] {
            let response = file.request(Method::GET, &[("range", range)]).await;
            assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
            assert_eq!(response.headers()[header::CONTENT_RANGE], content_range);
            assert_eq!(to_bytes(response.into_body(), 10).await.unwrap(), expected);
        }
    }

    #[tokio::test]
    async fn out_of_bounds_range_is_rejected() {
        let file = EpisodeFile::new().await;
        let response = file.request(Method::GET, &[("range", "bytes=20-")]).await;
        assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */10");
    }

    #[tokio::test]
    async fn stale_if_range_returns_the_full_resource() {
        let file = EpisodeFile::new().await;
        let response = file
            .request(
                Method::GET,
                &[
                    ("range", "bytes=2-5"),
                    ("if-range", "Thu, 01 Jan 1970 00:00:00 GMT"),
                ],
            )
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(!response.headers().contains_key(header::CONTENT_RANGE));
        assert_eq!(
            to_bytes(response.into_body(), 10).await.unwrap(),
            "0123456789"
        );
    }

    #[tokio::test]
    async fn matching_validator_and_head_preserve_file_semantics() {
        let file = EpisodeFile::new().await;
        let response = file.request(Method::HEAD, &[]).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_LENGTH], "10");
        let modified = response.headers()[header::LAST_MODIFIED]
            .to_str()
            .unwrap()
            .to_owned();
        assert!(to_bytes(response.into_body(), 10).await.unwrap().is_empty());

        let partial = file
            .request(
                Method::GET,
                &[("range", "bytes=2-5"), ("if-range", &modified)],
            )
            .await;
        assert_eq!(partial.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(to_bytes(partial.into_body(), 10).await.unwrap(), "2345");

        let cached = file
            .request(Method::GET, &[("if-modified-since", &modified)])
            .await;
        assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
        assert!(to_bytes(cached.into_body(), 10).await.unwrap().is_empty());
    }
}
