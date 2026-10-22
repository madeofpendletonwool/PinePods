// The outside services Pinepods uses to find and update podcasts, for the admin settings: whether
// the search service answers, and Podping (on/off, the relay's state, what this server did).

use std::time::{Duration, Instant};

use axum::{extract::State, http::HeaderMap, response::Json};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    handlers::require_admin,
    services::podping::{self, ClientStatus},
    AppState,
};

#[derive(Serialize, utoipa::ToSchema)]
pub struct SearchServiceStatus {
    /// The search service's address (`SEARCH_API_URL` without its path).
    pub url: String,
    pub reachable: bool,
    pub latency_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct PodpingInfo {
    pub enabled: bool,
    /// Where announcements are read from (`PINEPODS_PODPING_URL`, else the search service).
    pub relay_url: String,
    /// The relay's own status (`following`: it keeps up with Hive), when it answered.
    #[schema(value_type = Option<Object>)]
    pub relay: Option<serde_json::Value>,
    /// Why the relay's status couldn't be read (e.g. a search service without the relay).
    pub relay_error: Option<String>,
    pub client: ClientStatus,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct UpdateServices {
    pub search: SearchServiceStatus,
    pub podping: PodpingInfo,
}

async fn get_json(url: &str) -> Result<(serde_json::Value, u64), String> {
    let started = Instant::now();
    let resp = crate::services::http::service_client()
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let ms = started.elapsed().as_millis() as u64;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let v = resp.json().await.map_err(|e| e.to_string())?;
    Ok((v, ms))
}

async fn status(state: &AppState) -> AppResult<UpdateServices> {
    let base = podping::search_service_base();
    let relay_url = podping::relay_url();
    let (stats_url, relay_status_url) = (format!("{base}/api/stats"), format!("{relay_url}/api/podping/status"));
    let (search, relay) = tokio::join!(get_json(&stats_url), get_json(&relay_status_url));
    Ok(UpdateServices {
        search: match search {
            Ok((_, ms)) => SearchServiceStatus { url: base, reachable: true, latency_ms: Some(ms), error: None },
            Err(e) => SearchServiceStatus { url: base, reachable: false, latency_ms: None, error: Some(e) },
        },
        podping: PodpingInfo {
            enabled: state.db_pool.podping_enabled().await?,
            relay_url,
            relay: relay.as_ref().ok().map(|(v, _)| v.clone()),
            relay_error: relay.err(),
            client: podping::client_status(),
        },
    })
}

#[utoipa::path(
    get,
    path = "/update_services",
    tag = "settings",
    summary = "Search service and Podping status",
    description = "Admin only. Checks that the search service answers and reports Podping: whether it's on, the relay's state and what this server did with its announcements.",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = UpdateServices),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not an admin"),
    ),
)]
pub async fn get_update_services(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<UpdateServices>, AppError> {
    require_admin(&state, &headers).await?;
    Ok(Json(status(&state).await?))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetPodping {
    pub enabled: bool,
}

#[utoipa::path(
    put,
    path = "/update_services/podping",
    tag = "settings",
    summary = "Turn Podping on or off",
    description = "Admin only. When on, Pinepods reads which feeds Podping announced from the relay and refreshes the ones it has right away. Only announcements are fetched; what this server subscribes to is never sent.",
    request_body = SetPodping,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = UpdateServices),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not an admin"),
    ),
)]
pub async fn set_podping(State(state): State<AppState>, headers: HeaderMap, Json(request): Json<SetPodping>) -> Result<Json<UpdateServices>, AppError> {
    require_admin(&state, &headers).await?;
    state.db_pool.set_podping_enabled(request.enabled).await?;
    if request.enabled {
        // Join the relay now instead of at the next poll, so the status shows whether it works.
        if let Err(e) = podping::poll(&state).await {
            tracing::debug!("First Podping poll failed: {}", e);
        }
    }
    Ok(Json(status(&state).await?))
}
