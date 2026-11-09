// Refreshing one podcast now, and turning its automatic refreshes on or off. Works the same for
// feeds, YouTube channels and local folders.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, podcasts::Episode, refresh::PodcastRefresh, require_valid_api_key},
    AppState,
};

async fn caller(state: &AppState, headers: &HeaderMap) -> AppResult<i32> {
    let api_key = extract_api_key(headers)?;
    require_valid_api_key(state, &api_key).await?;
    state.db_pool.get_user_id_from_api_key(&api_key).await
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct AutoRefresh {
    /// The scheduled refresh and Refresh all check this podcast.
    pub auto_refresh: bool,
}

#[utoipa::path(
    get,
    path = "/podcast/{podcast_id}/auto_refresh",
    tag = "podcasts",
    summary = "Whether a podcast refreshes automatically",
    params(("podcast_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = AutoRefresh),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such podcast"),
    ),
)]
pub async fn get_auto_refresh(
    State(state): State<AppState>,
    Path(podcast_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<AutoRefresh>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let auto_refresh = state
        .db_pool
        .get_auto_refresh(podcast_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Podcast not found"))?;
    Ok(Json(AutoRefresh { auto_refresh }))
}

#[utoipa::path(
    put,
    path = "/podcast/{podcast_id}/auto_refresh",
    tag = "podcasts",
    summary = "Turn a podcast's automatic refreshes on or off",
    description = "Off: the scheduled refresh and Refresh all skip the podcast, and it leaves the feed problems list. Refreshing it on its own still works.",
    params(("podcast_id" = i32, Path)),
    request_body = AutoRefresh,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = AutoRefresh),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such podcast"),
    ),
)]
pub async fn set_auto_refresh(
    State(state): State<AppState>,
    Path(podcast_id): Path<i32>,
    headers: HeaderMap,
    Json(request): Json<AutoRefresh>,
) -> Result<Json<AutoRefresh>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !state.db_pool.set_auto_refresh(podcast_id, user_id, request.auto_refresh).await? {
        return Err(AppError::not_found("Podcast not found"));
    }
    Ok(Json(request))
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct PodcastRefreshResult {
    /// `refreshed`, `throttled` (the feed's host asked Pinepods to wait) or `failed`.
    pub status: String,
    /// What went wrong, when it failed.
    pub detail: Option<String>,
    /// The episodes it added. YouTube channels report none here; their videos show on the
    /// podcast's page.
    pub new_episodes: Vec<Episode>,
}

#[utoipa::path(
    post,
    path = "/podcast/{podcast_id}/refresh",
    tag = "podcasts",
    summary = "Refresh one podcast now",
    description = "Checks the podcast for new episodes straight away: reads its feed or YouTube channel, or looks for new files in its local folder. Works whether or not its automatic refreshes are on.",
    params(("podcast_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = PodcastRefreshResult),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such podcast"),
    ),
)]
pub async fn refresh_podcast(
    State(state): State<AppState>,
    Path(podcast_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<PodcastRefreshResult>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let result = match crate::handlers::refresh::refresh_podcast_now(&state, user_id, podcast_id).await? {
        PodcastRefresh::NotFound => return Err(AppError::not_found("Podcast not found")),
        PodcastRefresh::Throttled => PodcastRefreshResult {
            status: "throttled".into(),
            detail: Some("The feed's host asked Pinepods to wait before reading it again".into()),
            new_episodes: Vec::new(),
        },
        PodcastRefresh::Done(new_episodes) => PodcastRefreshResult { status: "refreshed".into(), detail: None, new_episodes },
        PodcastRefresh::Failed(e) => PodcastRefreshResult { status: "failed".into(), detail: Some(e.to_string()), new_episodes: Vec::new() },
    };
    Ok(Json(result))
}
