//! People credited on podcasts and episodes, person pages, follows, and contributing people to
//! PodPeopleDB. The caller is always the API key's owner.

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    handlers::{extract_api_key, require_valid_api_key},
    services::people::{
        external, podpeople, store, CreditedPerson, Endpoints, Person, PersonProfile, PodcastCredits,
    },
    AppState,
};

async fn caller(state: &AppState, headers: &HeaderMap) -> Result<i32, AppError> {
    let api_key = extract_api_key(headers)?;
    require_valid_api_key(state, &api_key).await?;
    state.db_pool.get_user_id_from_api_key(&api_key).await
}

/// Everyone credited on one of your podcasts.
#[utoipa::path(
    get,
    path = "/people/podcast/{podcast_id}",
    tag = "people",
    summary = "People on a podcast",
    params(("podcast_id" = i32, Path, description = "Your podcast id")),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Regulars in `people`, episode-only credits in `guests`", body = PodcastCredits),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "Not one of your podcasts"),
    ),
)]
pub async fn podcast_people(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(podcast_id): Path<i32>,
) -> Result<Json<PodcastCredits>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(store::podcast_credits(&state.db_pool, podcast_id, user_id).await?))
}

/// People on one of your episodes.
#[utoipa::path(
    get,
    path = "/people/episode/{episode_id}",
    tag = "people",
    summary = "People on an episode",
    params(("episode_id" = i32, Path, description = "Your episode id")),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "The episode's own credits plus the show's regulars", body = Vec<CreditedPerson>),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "Not one of your episodes"),
    ),
)]
pub async fn episode_people(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(episode_id): Path<i32>,
) -> Result<Json<Vec<CreditedPerson>>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(store::episode_credits(&state.db_pool, episode_id, user_id).await?))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct ProfileQuery {
    /// Library episodes per page (default 50, max 200).
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// A person's page: who they are, their shows, and their episodes in and outside your library.
#[utoipa::path(
    get,
    path = "/people/{person_id}",
    tag = "people",
    summary = "Person profile",
    params(("person_id" = i32, Path, description = "PinePods person id"), ProfileQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Profile", body = PersonProfile),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "Unknown person"),
    ),
)]
pub async fn person_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(person_id): Path<i32>,
    Query(q): Query<ProfileQuery>,
) -> Result<Json<PersonProfile>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let db = &state.db_pool;
    let person = store::person(db, person_id, user_id).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0).max(0);

    let refreshing = if external::is_stale(db, person_id).await? {
        external::refresh_soon(db.clone(), Endpoints::from_config(&state.config), person_id)
    } else {
        false
    };
    let mut podcasts = store::library_podcasts(db, person_id, user_id).await?;
    let episodes = store::library_episodes(db, person_id, user_id, limit, offset).await?;
    let external_episodes = external::episodes_for_user(db, person_id, user_id, 100).await?;
    let art = external::podcast_art(db, person_id).await?;
    podcasts.extend(external::podcasts_from(&external_episodes, &art));

    Ok(Json(PersonProfile { person, podcasts, episodes, external_episodes, refreshing }))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct LookupQuery {
    /// A PodPeopleDB person id (from search results).
    pub podpeople_id: Option<i64>,
    /// Or a name.
    pub name: Option<String>,
}

/// Resolve a PodPeopleDB id or a name to a PinePods person id, so it can be opened or followed.
#[utoipa::path(
    get,
    path = "/people/lookup",
    tag = "people",
    summary = "Find a person id",
    params(LookupQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "`{ \"person_id\": 12 }`", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such person"),
    ),
)]
pub async fn lookup_person(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<LookupQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    caller(&state, &headers).await?;
    let id = match (q.podpeople_id, q.name.as_deref()) {
        (Some(pp), _) => podpeople::person_for_podpeople(&state.db_pool, &Endpoints::from_config(&state.config), pp).await?,
        (None, Some(name)) => store::find_by_name(&state.db_pool, name)
            .await?
            .ok_or_else(|| AppError::not_found("Person not found"))?,
        (None, None) => return Err(AppError::bad_request("Give podpeople_id or name")),
    };
    Ok(Json(serde_json::json!({ "person_id": id })))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct SearchQuery {
    pub q: String,
}

/// Search people on this server and PodPeopleDB.
#[utoipa::path(
    get,
    path = "/people/search",
    tag = "people",
    summary = "Search people",
    params(SearchQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Matches", body = Vec<podpeople::PersonHit>),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn search_people(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Vec<podpeople::PersonHit>>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(podpeople::search(&state.db_pool, &Endpoints::from_config(&state.config), user_id, &q.q).await?))
}

/// Follow a person: their new episodes, on any show, show up for you.
#[utoipa::path(
    post,
    path = "/people/{person_id}/follow",
    tag = "people",
    summary = "Follow a person",
    params(("person_id" = i32, Path, description = "PinePods person id")),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Following", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "Unknown person"),
    ),
)]
pub async fn follow_person(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(person_id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = caller(&state, &headers).await?;
    store::follow(&state.db_pool, user_id, person_id).await?;
    if external::is_stale(&state.db_pool, person_id).await? {
        external::refresh_soon(state.db_pool.clone(), Endpoints::from_config(&state.config), person_id);
    }
    Ok(Json(serde_json::json!({ "following": true })))
}

#[utoipa::path(
    delete,
    path = "/people/{person_id}/follow",
    tag = "people",
    summary = "Unfollow a person",
    params(("person_id" = i32, Path, description = "PinePods person id")),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Not following", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn unfollow_person(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(person_id): Path<i32>,
) -> Result<Json<serde_json::Value>, AppError> {
    let user_id = caller(&state, &headers).await?;
    store::unfollow(&state.db_pool, user_id, person_id).await?;
    Ok(Json(serde_json::json!({ "following": false })))
}

/// People you follow.
#[utoipa::path(
    get,
    path = "/people/followed",
    tag = "people",
    summary = "Followed people",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "People", body = Vec<Person>),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn followed_people(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<Person>>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(store::followed(&state.db_pool, user_id).await?))
}

/// Add people to one of your podcasts (or one episode). Sent to PodPeopleDB, where they go live
/// right away for everyone, and shown here immediately.
#[utoipa::path(
    post,
    path = "/people/contribute",
    tag = "people",
    summary = "Add people to a podcast or episode",
    request_body = podpeople::ContributeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Everyone on the podcast or episode afterwards", body = Vec<CreditedPerson>),
        (status = 400, description = "Rejected (PodPeopleDB's reason is in the message)"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "Not one of your podcasts"),
    ),
)]
pub async fn contribute_people(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<podpeople::ContributeRequest>,
) -> Result<Json<Vec<CreditedPerson>>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(podpeople::contribute(&state.db_pool, &Endpoints::from_config(&state.config), user_id, &req).await?))
}
