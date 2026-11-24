// The inbox: new episodes the user hasn't dealt with yet, and the labels they sort episodes with.
// Every call acts for the user the API key belongs to.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    database::inbox::{EpisodeLabel, InboxFilter, LabelAssignment, INBOX_MAX_DAYS, LABEL_COLORS},
    error::{AppError, AppResult},
    handlers::{extract_api_key, podcasts::EpisodesResponse, require_valid_api_key},
    AppState,
};

async fn caller(state: &AppState, headers: &HeaderMap) -> AppResult<i32> {
    let api_key = extract_api_key(headers)?;
    require_valid_api_key(state, &api_key).await?;
    state.db_pool.get_user_id_from_api_key(&api_key).await
}

/// One episode or YouTube video.
#[derive(Serialize, Deserialize, Clone, Copy, utoipa::ToSchema)]
pub struct EpisodeRef {
    pub episode_id: i32,
    #[serde(default)]
    pub is_youtube: bool,
}

fn refs(episodes: &[EpisodeRef]) -> AppResult<Vec<(i32, bool)>> {
    if episodes.len() > 5000 {
        return Err(AppError::bad_request("At most 5000 episodes at a time"));
    }
    Ok(episodes.iter().map(|e| (e.episode_id, e.is_youtube)).collect())
}

#[derive(Deserialize, Default, utoipa::IntoParams)]
pub struct InboxQuery {
    /// Page size, at most 200 (default 50).
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    /// Only episodes with this label.
    pub label_id: Option<i32>,
    /// Only episodes without any label.
    #[serde(default)]
    pub unlabeled: bool,
    pub podcast_id: Option<i32>,
    /// `newest` (default) or `oldest`.
    pub sort: Option<String>,
}

impl InboxQuery {
    fn filter(&self) -> InboxFilter {
        InboxFilter {
            label_id: self.label_id,
            unlabeled: self.unlabeled,
            podcast_id: self.podcast_id,
            oldest_first: self.sort.as_deref() == Some("oldest"),
        }
    }
}

#[utoipa::path(
    get,
    path = "/inbox",
    tag = "inbox",
    summary = "List the inbox",
    description = "Episodes of subscribed podcasts released in the last `days` days (see the inbox settings) that the user hasn't played, finished, queued, saved, downloaded or dismissed. Podcasts with their inbox turned off are left out. Newest first unless `sort=oldest`.",
    params(InboxQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = EpisodesResponse),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
    headers: HeaderMap,
) -> Result<Json<EpisodesResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let (episodes, total) = state.db_pool.get_inbox(user_id, &query.filter(), limit, offset).await?;
    Ok(Json(EpisodesResponse { episodes, total }))
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct InboxCount {
    pub count: i64,
}

#[utoipa::path(
    get,
    path = "/inbox/count",
    tag = "inbox",
    summary = "How many episodes are in the inbox",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = InboxCount),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_inbox_count(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<InboxCount>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(InboxCount { count: state.db_pool.inbox_count(user_id).await? }))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct InboxEpisodesRequest {
    pub episodes: Vec<EpisodeRef>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct InboxChanged {
    /// How many episodes changed.
    pub changed: u64,
}

#[utoipa::path(
    post,
    path = "/inbox/dismiss",
    tag = "inbox",
    summary = "Take episodes out of the inbox",
    request_body = InboxEpisodesRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = InboxChanged),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn dismiss_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InboxEpisodesRequest>,
) -> Result<Json<InboxChanged>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let changed = state.db_pool.dismiss_inbox_episodes(user_id, &refs(&request.episodes)?).await?;
    Ok(Json(InboxChanged { changed }))
}

#[utoipa::path(
    post,
    path = "/inbox/restore",
    tag = "inbox",
    summary = "Undo dismissing episodes",
    description = "They show in the inbox again if nothing else (a play, queue entry, save, ...) took them out.",
    request_body = InboxEpisodesRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = InboxChanged),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn restore_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InboxEpisodesRequest>,
) -> Result<Json<InboxChanged>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let changed = state.db_pool.restore_inbox_episodes(user_id, &refs(&request.episodes)?).await?;
    Ok(Json(InboxChanged { changed }))
}

#[derive(Deserialize, Default, utoipa::ToSchema)]
pub struct ClearInboxRequest {
    /// Only this podcast's episodes.
    pub podcast_id: Option<i32>,
    /// Only episodes with this label.
    pub label_id: Option<i32>,
    /// Only episodes without a label.
    #[serde(default)]
    pub unlabeled: bool,
}

#[utoipa::path(
    post,
    path = "/inbox/clear",
    tag = "inbox",
    summary = "Dismiss everything in the inbox",
    description = "With `podcast_id`, `label_id` or `unlabeled`, only what that filter shows.",
    request_body = ClearInboxRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = InboxChanged),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn clear_inbox(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClearInboxRequest>,
) -> Result<Json<InboxChanged>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let filter = InboxFilter {
        label_id: request.label_id,
        unlabeled: request.unlabeled,
        podcast_id: request.podcast_id,
        oldest_first: false,
    };
    let changed = state.db_pool.clear_inbox(user_id, &filter).await?;
    Ok(Json(InboxChanged { changed }))
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct InboxSettings {
    /// How many days back the inbox reaches, 1 to 365 (default 30).
    pub days: i32,
}

#[utoipa::path(
    get,
    path = "/inbox/settings",
    tag = "inbox",
    summary = "Get the inbox settings",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = InboxSettings),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_inbox_settings(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<InboxSettings>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(InboxSettings { days: state.db_pool.get_inbox_days(user_id).await? }))
}

#[utoipa::path(
    put,
    path = "/inbox/settings",
    tag = "inbox",
    summary = "Change the inbox settings",
    request_body = InboxSettings,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = InboxSettings),
        (status = 400, description = "Days out of range"),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn set_inbox_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<InboxSettings>,
) -> Result<Json<InboxSettings>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !(1..=INBOX_MAX_DAYS).contains(&request.days) {
        return Err(AppError::bad_request(format!("Days must be between 1 and {INBOX_MAX_DAYS}")));
    }
    let days = state.db_pool.set_inbox_days(user_id, request.days).await?;
    Ok(Json(InboxSettings { days }))
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct PodcastInbox {
    /// New episodes of the podcast show in the inbox.
    pub inbox: bool,
}

#[utoipa::path(
    get,
    path = "/podcast/{podcast_id}/inbox",
    tag = "inbox",
    summary = "Whether a podcast's episodes show in the inbox",
    params(("podcast_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = PodcastInbox),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such podcast"),
    ),
)]
pub async fn get_podcast_inbox(
    State(state): State<AppState>,
    Path(podcast_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<PodcastInbox>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let inbox = state
        .db_pool
        .get_podcast_inbox(podcast_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Podcast not found"))?;
    Ok(Json(PodcastInbox { inbox }))
}

#[utoipa::path(
    put,
    path = "/podcast/{podcast_id}/inbox",
    tag = "inbox",
    summary = "Show or hide a podcast's episodes in the inbox",
    params(("podcast_id" = i32, Path)),
    request_body = PodcastInbox,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = PodcastInbox),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such podcast"),
    ),
)]
pub async fn set_podcast_inbox(
    State(state): State<AppState>,
    Path(podcast_id): Path<i32>,
    headers: HeaderMap,
    Json(request): Json<PodcastInbox>,
) -> Result<Json<PodcastInbox>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !state.db_pool.set_podcast_inbox(podcast_id, user_id, request.inbox).await? {
        return Err(AppError::not_found("Podcast not found"));
    }
    Ok(Json(request))
}

// ---- Labels ----

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LabelRequest {
    /// 1 to 64 characters, unique per user ignoring case.
    pub name: String,
    /// One of blue, green, yellow, orange, red, purple, pink, teal, gray (default blue).
    pub color: Option<String>,
}

impl LabelRequest {
    fn validated(&self) -> AppResult<(String, String)> {
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(AppError::bad_request("A label name needs 1 to 64 characters"));
        }
        let color = self.color.as_deref().unwrap_or("blue").to_lowercase();
        if !LABEL_COLORS.contains(&color.as_str()) {
            return Err(AppError::bad_request(format!("Color must be one of {}", LABEL_COLORS.join(", "))));
        }
        Ok((name.to_string(), color))
    }
}

#[utoipa::path(
    get,
    path = "/labels",
    tag = "inbox",
    summary = "List the user's episode labels",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = Vec<EpisodeLabel>),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn list_labels(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Vec<EpisodeLabel>>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(state.db_pool.get_episode_labels(user_id).await?))
}

#[utoipa::path(
    post,
    path = "/labels",
    tag = "inbox",
    summary = "Create an episode label",
    request_body = LabelRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = EpisodeLabel),
        (status = 400, description = "Bad name or color"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 409, description = "A label with that name exists"),
    ),
)]
pub async fn create_label(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<LabelRequest>,
) -> Result<Json<EpisodeLabel>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let (name, color) = request.validated()?;
    Ok(Json(state.db_pool.create_episode_label(user_id, &name, &color).await?))
}

#[utoipa::path(
    put,
    path = "/labels/{label_id}",
    tag = "inbox",
    summary = "Rename or recolor an episode label",
    params(("label_id" = i32, Path)),
    request_body = LabelRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success"),
        (status = 400, description = "Bad name or color"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such label"),
        (status = 409, description = "A label with that name exists"),
    ),
)]
pub async fn update_label(
    State(state): State<AppState>,
    Path(label_id): Path<i32>,
    headers: HeaderMap,
    Json(request): Json<LabelRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let (name, color) = request.validated()?;
    if !state.db_pool.update_episode_label(user_id, label_id, &name, &color).await? {
        return Err(AppError::not_found("Label not found"));
    }
    Ok(Json(serde_json::json!({ "label_id": label_id, "name": name, "color": color })))
}

#[utoipa::path(
    delete,
    path = "/labels/{label_id}",
    tag = "inbox",
    summary = "Delete an episode label",
    description = "Takes it off every episode. Smart playlists filtering on it stop doing so.",
    params(("label_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such label"),
    ),
)]
pub async fn delete_label(
    State(state): State<AppState>,
    Path(label_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !state.db_pool.delete_episode_label(user_id, label_id).await? {
        return Err(AppError::not_found("Label not found"));
    }
    Ok(Json(serde_json::json!({ "detail": "Label deleted" })))
}

#[utoipa::path(
    get,
    path = "/labels/assignments",
    tag = "inbox",
    summary = "Which labels are on which episodes",
    description = "Every label on every episode of the user's, for showing labels on episode lists.",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = Vec<LabelAssignment>),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn list_label_assignments(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Vec<LabelAssignment>>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(state.db_pool.get_label_assignments(user_id).await?))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct LabelEpisodesRequest {
    pub episodes: Vec<EpisodeRef>,
    /// True puts the label on, false takes it off.
    pub assigned: bool,
}

#[utoipa::path(
    post,
    path = "/labels/{label_id}/episodes",
    tag = "inbox",
    summary = "Put a label on episodes or take it off",
    params(("label_id" = i32, Path)),
    request_body = LabelEpisodesRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = InboxChanged),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such label, or an episode that isn't the user's"),
    ),
)]
pub async fn set_label_episodes(
    State(state): State<AppState>,
    Path(label_id): Path<i32>,
    headers: HeaderMap,
    Json(request): Json<LabelEpisodesRequest>,
) -> Result<Json<InboxChanged>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !state.db_pool.label_owned(user_id, label_id).await? {
        return Err(AppError::not_found("Label not found"));
    }
    let episodes = refs(&request.episodes)?;
    if request.assigned {
        for (id, yt) in &episodes {
            if !state.db_pool.episode_belongs_to_user(*id, user_id, *yt).await? {
                return Err(AppError::not_found(format!("Episode {id} not found")));
            }
        }
    }
    let changed = state.db_pool.set_label_on_episodes(user_id, label_id, &episodes, request.assigned).await?;
    Ok(Json(InboxChanged { changed }))
}
