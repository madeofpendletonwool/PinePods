// Feed problems for the calling user: their podcasts whose refresh keeps failing, and feeds a sync
// app subscribed to that Pinepods couldn't import, with what can be done about each.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::Json,
};
use serde::Serialize;

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, require_valid_api_key, sync_settings::SuccessResponse},
    services::feed_health::FailingFeed,
    services::sync::imports::{self, ImportFailure},
    AppState,
};

async fn caller(state: &AppState, headers: &HeaderMap) -> AppResult<i32> {
    let api_key = extract_api_key(headers)?;
    require_valid_api_key(state, &api_key).await?;
    state.db_pool.get_user_id_from_api_key(&api_key).await
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct FeedProblems {
    /// Podcasts whose last refresh failed, longest-failing first.
    pub feeds: Vec<FailingFeed>,
    /// Feeds from sync apps that couldn't be added, oldest first.
    pub imports: Vec<ImportFailure>,
}

#[utoipa::path(
    get,
    path = "/feed_problems",
    tag = "podcasts",
    summary = "Feeds that keep failing",
    description = "The caller's podcasts whose last refresh failed (with the kind of error) and feeds a sync app subscribed to that couldn't be added.",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = FeedProblems),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_feed_problems(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<FeedProblems>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(FeedProblems {
        feeds: state.db_pool.failing_feeds(user_id).await?,
        imports: state.db_pool.sync_import_failures(user_id).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/feed_problems/{podcast_id}/refresh",
    tag = "podcasts",
    summary = "Refresh a failing podcast now",
    description = "Refreshes one of the caller's podcasts straight away and returns its state afterwards: absent from `feeds` when it worked.",
    params(("podcast_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = FeedProblems),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such podcast"),
    ),
)]
pub async fn refresh_failing_feed(
    State(state): State<AppState>,
    Path(podcast_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<FeedProblems>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !crate::handlers::refresh::refresh_podcast_now(&state, user_id, podcast_id).await? {
        return Err(AppError::not_found("Podcast not found"));
    }
    get_feed_problems(State(state), headers).await
}

async fn own_import(state: &AppState, user_id: i32, id: i32) -> AppResult<ImportFailure> {
    state.db_pool.sync_import_failure(user_id, id).await?.ok_or_else(|| AppError::not_found("No such failed import"))
}

#[utoipa::path(
    post,
    path = "/feed_problems/imports/{id}/retry",
    tag = "podcasts",
    summary = "Try adding a failed sync import again",
    description = "Starts the import now (also after Pinepods stopped retrying). It shows in the activity panel.",
    params(("id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SuccessResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such failed import"),
    ),
)]
pub async fn retry_import(State(state): State<AppState>, Path(id): Path<i32>, headers: HeaderMap) -> Result<Json<SuccessResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let failure = own_import(&state, user_id, id).await?;
    // False when it's importing already, which is just as good.
    crate::services::sync::apply::queue_import(&state.task_spawner, user_id, &failure.feed_url, imports::parse_origin(&failure.origin)).await;
    Ok(Json(SuccessResponse { success: true }))
}

#[utoipa::path(
    post,
    path = "/feed_problems/imports/{id}/remove",
    tag = "podcasts",
    summary = "Unsubscribe from a feed that couldn't be added",
    description = "Forgets the failed import and records an unsubscribe, so sync apps and the external sync server drop the feed too.",
    params(("id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SuccessResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such failed import"),
    ),
)]
pub async fn remove_import(State(state): State<AppState>, Path(id): Path<i32>, headers: HeaderMap) -> Result<Json<SuccessResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !state.db_pool.sync_import_remove(user_id, id).await? {
        return Err(AppError::not_found("No such failed import"));
    }
    Ok(Json(SuccessResponse { success: true }))
}

#[utoipa::path(
    delete,
    path = "/feed_problems/imports/{id}",
    tag = "podcasts",
    summary = "Dismiss a failed sync import",
    description = "Forgets the failed import without unsubscribing: the feed stays subscribed in the sync apps.",
    params(("id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SuccessResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such failed import"),
    ),
)]
pub async fn dismiss_import(State(state): State<AppState>, Path(id): Path<i32>, headers: HeaderMap) -> Result<Json<SuccessResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !state.db_pool.sync_import_dismiss(user_id, id).await? {
        return Err(AppError::not_found("No such failed import"));
    }
    Ok(Json(SuccessResponse { success: true }))
}
