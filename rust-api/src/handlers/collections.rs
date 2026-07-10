use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use std::collections::HashMap;

use crate::{
    error::{AppError, AppResult},
    handlers::{check_user_access, extract_api_key, validate_api_key},
    models::{
        BulkAddCollectionRequest, CollectionDetailResponse, CollectionEpisodeRequest,
        CollectionsResponse, CreateCollectionRequest, CreateCollectionResponse,
        EpisodeCollectionsResponse, SavedEpisodesResponse, UpdateCollectionRequest,
        UserCategoriesResponse,
    },
    AppState,
};

/// Resolve the api-key's user, erroring if the key is invalid.
async fn auth_user(state: &AppState, headers: &HeaderMap) -> AppResult<(String, i32, bool)> {
    let api_key = extract_api_key(headers)?;
    if !validate_api_key(state, &api_key).await? {
        return Err(AppError::unauthorized(
            "Your API key is either invalid or does not have correct permission",
        ));
    }
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    Ok((api_key, user_id, is_web_key))
}

#[utoipa::path(
    post,
    path = "/collections/create",
    tag = "collections",
    summary = "Create a collection",
    request_body = CreateCollectionRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Collection created", body = CreateCollectionResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot create a collection for another user"),
        (status = 409, description = "A collection with that name already exists"),
    ),
)]
pub async fn create_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCollectionRequest>,
) -> AppResult<Json<CreateCollectionResponse>> {
    let (_key, user_id, is_web_key) = auth_user(&state, &headers).await?;
    if user_id != req.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only create collections for yourself!"));
    }

    let collection_id = state.db_pool.create_collection(&req).await?;
    // Backfill existing matching episodes when requested (categories present + toggle on).
    if req.backfill.unwrap_or(false) {
        if let Err(e) = state.db_pool.auto_add_category_episodes(collection_id).await {
            tracing::warn!("Backfill for new collection {} failed: {}", collection_id, e);
        }
    }
    Ok(Json(CreateCollectionResponse {
        detail: "Collection created successfully".to_string(),
        collection_id,
    }))
}

#[utoipa::path(
    get,
    path = "/collections/user/{user_id}",
    tag = "collections",
    summary = "List a user's collections",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Collections", body = CollectionsResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot list another user's collections"),
    ),
)]
pub async fn list_collections(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<i32>,
) -> AppResult<Json<CollectionsResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only list your own collections!"));
    }

    let collections = state.db_pool.get_collections(user_id).await?;
    Ok(Json(CollectionsResponse { collections }))
}

#[utoipa::path(
    get,
    path = "/collections/categories/{user_id}",
    tag = "collections",
    summary = "List the distinct podcast categories across a user's subscriptions",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Categories", body = UserCategoriesResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot list another user's categories"),
    ),
)]
pub async fn get_user_categories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<i32>,
) -> AppResult<Json<UserCategoriesResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only list your own categories!"));
    }

    let categories = state.db_pool.get_user_categories(user_id).await?;
    Ok(Json(UserCategoriesResponse { categories }))
}

#[utoipa::path(
    delete,
    path = "/collections/{collection_id}",
    tag = "collections",
    summary = "Delete a collection",
    params(("collection_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Collection deleted", body = CollectionDetailResponse),
        (status = 400, description = "Cannot delete the default collection"),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot delete another user's collection"),
    ),
)]
pub async fn delete_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(collection_id): Path<i32>,
) -> AppResult<Json<CollectionDetailResponse>> {
    let (_key, user_id, _is_web_key) = auth_user(&state, &headers).await?;
    state.db_pool.delete_collection(user_id, collection_id).await?;
    Ok(Json(CollectionDetailResponse {
        detail: "Collection deleted successfully".to_string(),
    }))
}

#[utoipa::path(
    patch,
    path = "/collections/{collection_id}",
    tag = "collections",
    summary = "Update a collection",
    params(("collection_id" = i32, Path)),
    request_body = UpdateCollectionRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Collection updated", body = CollectionDetailResponse),
        (status = 400, description = "Cannot edit the default collection"),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot edit another user's collection"),
        (status = 409, description = "A collection with that name already exists"),
    ),
)]
pub async fn update_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(collection_id): Path<i32>,
    Json(req): Json<UpdateCollectionRequest>,
) -> AppResult<Json<CollectionDetailResponse>> {
    let (_key, user_id, _is_web_key) = auth_user(&state, &headers).await?;
    let backfill = req.backfill.unwrap_or(false);
    state.db_pool.update_collection(user_id, collection_id, &req).await?;
    // Backfill existing matching episodes when requested (categories present + toggle on).
    if backfill {
        if let Err(e) = state.db_pool.auto_add_category_episodes(collection_id).await {
            tracing::warn!("Backfill for collection {} failed: {}", collection_id, e);
        }
    }
    Ok(Json(CollectionDetailResponse {
        detail: "Collection updated successfully".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/collections/{collection_id}/add_episode",
    tag = "collections",
    summary = "Add an episode to a collection",
    params(("collection_id" = i32, Path)),
    request_body = CollectionEpisodeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Episode added", body = CollectionDetailResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot modify another user's collection"),
    ),
)]
pub async fn add_episode_to_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(collection_id): Path<i32>,
    Json(req): Json<CollectionEpisodeRequest>,
) -> AppResult<Json<CollectionDetailResponse>> {
    let (_key, user_id, is_web_key) = auth_user(&state, &headers).await?;
    if user_id != req.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own collections!"));
    }
    state
        .db_pool
        .add_episode_to_collection(req.user_id, collection_id, req.episode_id, req.is_youtube)
        .await?;
    Ok(Json(CollectionDetailResponse {
        detail: "Episode added to collection".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/collections/{collection_id}/remove_episode",
    tag = "collections",
    summary = "Remove an episode from a collection",
    params(("collection_id" = i32, Path)),
    request_body = CollectionEpisodeRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Episode removed", body = CollectionDetailResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot modify another user's collection"),
    ),
)]
pub async fn remove_episode_from_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(collection_id): Path<i32>,
    Json(req): Json<CollectionEpisodeRequest>,
) -> AppResult<Json<CollectionDetailResponse>> {
    let (_key, user_id, is_web_key) = auth_user(&state, &headers).await?;
    if user_id != req.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own collections!"));
    }
    state
        .db_pool
        .remove_episode_from_collection(req.user_id, collection_id, req.episode_id, req.is_youtube)
        .await?;
    Ok(Json(CollectionDetailResponse {
        detail: "Episode removed from collection".to_string(),
    }))
}

#[utoipa::path(
    post,
    path = "/collections/bulk_add",
    tag = "collections",
    summary = "Add multiple episodes to a collection",
    request_body = BulkAddCollectionRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Episodes added", body = CollectionDetailResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot modify another user's collection"),
    ),
)]
pub async fn bulk_add_collection(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BulkAddCollectionRequest>,
) -> AppResult<Json<CollectionDetailResponse>> {
    let (_key, user_id, is_web_key) = auth_user(&state, &headers).await?;
    if user_id != req.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only modify your own collections!"));
    }
    state
        .db_pool
        .bulk_add_to_collection(req.user_id, req.collection_id, &req.episodes)
        .await?;
    Ok(Json(CollectionDetailResponse {
        detail: "Episodes added to collection".to_string(),
    }))
}

#[utoipa::path(
    get,
    path = "/collections/{collection_id}/episodes",
    tag = "collections",
    summary = "Get a collection's episodes (paginated)",
    params(
        ("collection_id" = i32, Path),
        ("limit" = i64, Query),
        ("offset" = i64, Query),
        ("sort_by" = String, Query),
        ("sort_order" = String, Query),
        ("filter" = String, Query),
    ),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Episodes", body = SavedEpisodesResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot view another user's collection"),
    ),
)]
pub async fn get_collection_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(collection_id): Path<i32>,
    Query(query): Query<HashMap<String, String>>,
) -> AppResult<Json<SavedEpisodesResponse>> {
    let (_key, user_id, is_web_key) = auth_user(&state, &headers).await?;

    // Verify ownership of the collection and learn whether it's the default one
    let (owner_id, is_default) = state.db_pool.get_collection_meta(collection_id).await?;
    if owner_id != user_id && !is_web_key {
        return Err(AppError::forbidden("You can only view your own collections!"));
    }

    let limit = query.get("limit").and_then(|s| s.parse::<i64>().ok()).unwrap_or(50);
    let offset = query.get("offset").and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
    let sort_by = query.get("sort_by").map(String::as_str).unwrap_or("date");
    let sort_order = query.get("sort_order").map(String::as_str).unwrap_or("desc");
    let filter = query.get("filter").map(String::as_str).unwrap_or("");

    // The default collection is backed by SavedEpisodes/SavedVideos
    let (saved_episodes, total) = if is_default {
        state
            .db_pool
            .get_saved_episodes(owner_id, limit, offset, sort_by, sort_order, filter)
            .await?
    } else {
        state
            .db_pool
            .get_collection_episodes(owner_id, collection_id, limit, offset, sort_by, sort_order, filter)
            .await?
    };

    Ok(Json(SavedEpisodesResponse { saved_episodes, total }))
}

#[utoipa::path(
    get,
    path = "/episode_collections/{user_id}/{episode_id}",
    tag = "collections",
    summary = "Which collections contain this episode",
    params(
        ("user_id" = i32, Path),
        ("episode_id" = i32, Path),
        ("is_youtube" = bool, Query),
    ),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Collection IDs", body = EpisodeCollectionsResponse),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot access another user's collections"),
    ),
)]
pub async fn get_episode_collections(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, episode_id)): Path<(i32, i32)>,
    Query(query): Query<HashMap<String, String>>,
) -> AppResult<Json<EpisodeCollectionsResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only access your own collections!"));
    }

    let is_youtube = query
        .get("is_youtube")
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);

    let collection_ids = state
        .db_pool
        .get_episode_collections(user_id, episode_id, is_youtube)
        .await?;
    Ok(Json(EpisodeCollectionsResponse { collection_ids }))
}

// ---- collection_add_ui user preference ------------------------------------

#[derive(serde::Serialize, utoipa::ToSchema)]
pub struct CollectionAddUiResponse {
    pub collection_add_ui: String,
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub struct SetCollectionAddUiRequest {
    pub user_id: i32,
    pub mode: String,
}

#[utoipa::path(
    get,
    path = "/collection_add_ui",
    tag = "collections",
    summary = "Get the collection add-UI preference",
    params(("user_id" = i32, Query)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Preference", body = CollectionAddUiResponse),
        (status = 401, description = "Invalid API key"),
    ),
)]
pub async fn get_collection_add_ui(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<HashMap<String, String>>,
) -> AppResult<Json<CollectionAddUiResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    let user_id = query
        .get("user_id")
        .and_then(|s| s.parse::<i32>().ok())
        .ok_or_else(|| AppError::bad_request("Missing user_id"))?;
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("You can only access your own settings!"));
    }
    let collection_add_ui = state.db_pool.get_collection_add_ui(user_id).await?;
    Ok(Json(CollectionAddUiResponse { collection_add_ui }))
}

#[utoipa::path(
    post,
    path = "/collection_add_ui",
    tag = "collections",
    summary = "Set the collection add-UI preference",
    request_body = SetCollectionAddUiRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Updated", body = serde_json::Value),
        (status = 401, description = "Invalid API key"),
        (status = 403, description = "Cannot set another user's setting"),
    ),
)]
pub async fn set_collection_add_ui(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<SetCollectionAddUiRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let (_key, user_id, is_web_key) = auth_user(&state, &headers).await?;
    if user_id != req.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only set your own setting!"));
    }
    state.db_pool.set_collection_add_ui(req.user_id, &req.mode).await?;
    Ok(Json(serde_json::json!({ "success": true })))
}
