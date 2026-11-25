//! Music Assistant settings, discovery, and control.
//!
//! Players also show up as now-playing devices (`device_type` `music_assistant`)
//! so the existing remote-device socket can play, pause, seek, and `play_episode`
//! them. Volume and grouping are the same actions over this HTTP command, or over
//! the relay.

use axum::{extract::Query, extract::State, http::HeaderMap, Json};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;
use crate::handlers::{extract_api_key, require_user_access, require_valid_api_key};
use crate::services::music_assistant::{self, MaPlayer, MaSettingsUpdate as SettingsUpdate};
use crate::AppState;

#[derive(Deserialize, utoipa::IntoParams)]
pub struct MaUserQuery {
    pub user_id: i32,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MaSettingsResponse {
    pub enabled: bool,
    pub server_url: Option<String>,
    pub external_base_url: Option<String>,
    pub has_token: bool,
    pub connected: bool,
    pub server_version: Option<String>,
    pub schema_version: Option<i64>,
    pub warning: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MaSettingsBody {
    pub user_id: i32,
    /// Turn the bridge on. Requires a server URL, token, and external base URL
    /// (newly supplied or already stored).
    #[serde(default)]
    pub enabled: bool,
    pub server_url: Option<String>,
    /// Origin Music Assistant uses to fetch PinePods (`http://lan-host:port`). Not localhost.
    pub external_base_url: Option<String>,
    /// Long-lived Music Assistant token. Omit or send an empty string to keep the stored token.
    pub token: Option<String>,
    #[serde(default)]
    pub clear_token: bool,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MaTestBody {
    pub user_id: i32,
    /// Overrides the stored URL for this probe only.
    pub server_url: Option<String>,
    /// Overrides the stored token for this probe only. Never written.
    pub token: Option<String>,
    /// When set, validated the same way as a saved external base URL.
    /// Loopback is rejected here so the settings form can surface it on Test
    /// without writing a row. Omitted means the probe does not check it.
    #[serde(default)]
    pub external_base_url: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MaTestResponse {
    pub ok: bool,
    pub detail: String,
    pub server_version: Option<String>,
    pub schema_version: Option<i64>,
    pub player_count: i64,
    pub warning: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MaPlayerResponse {
    pub player_id: String,
    /// Now-playing device id (`ma:{user_id}:{player_id}`) for the existing relay.
    pub device_id: String,
    pub name: String,
    /// `player` or `group`.
    pub kind: String,
    pub available: bool,
    pub state: String,
    pub volume_level: Option<f64>,
    pub volume_muted: bool,
    pub group_members: Vec<String>,
    pub synced_to: Option<String>,
    pub can_group_with: Vec<String>,
    /// Queue id to target. Equals `player_id` unless the player is grouped.
    pub queue_id: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MaPlayersResponse {
    pub enabled: bool,
    pub connected: bool,
    pub players: Vec<MaPlayerResponse>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct MaCommandBody {
    pub user_id: i32,
    /// Music Assistant player id, or the relay device id `ma:{user_id}:{player_id}`.
    pub player_id: String,
    /// `play`, `pause`, `stop`, `seek`, `skip_forward`, `skip_back`,
    /// `play_episode`, `volume_set`, `volume_mute`, `group`, `group_many`,
    /// `ungroup`, `set_members`, `group_volume`. `set_speed` is accepted and
    /// ignored: Music Assistant is not asked to change tempo.
    pub action: String,
    #[serde(default)]
    pub args: Value,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct MaCommandResponse {
    pub ok: bool,
    pub detail: String,
    pub warnings: Vec<String>,
}

async fn authorize(state: &AppState, headers: &HeaderMap, user_id: i32) -> Result<(), AppError> {
    let api_key = extract_api_key(headers)?;
    require_valid_api_key(state, &api_key).await?;
    require_user_access(state, &api_key, user_id).await?;
    Ok(())
}

fn settings_response(view: music_assistant::MaSettingsView) -> MaSettingsResponse {
    MaSettingsResponse {
        enabled: view.enabled,
        server_url: view.server_url,
        external_base_url: view.external_base_url,
        has_token: view.has_token,
        connected: view.connected,
        server_version: view.server_version,
        schema_version: view.schema_version,
        warning: view.warning,
        last_error: view.last_error,
    }
}

fn player_response(user_id: i32, player: MaPlayer) -> MaPlayerResponse {
    let queue_id = player
        .active_source
        .clone()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| player.player_id.clone());
    MaPlayerResponse {
        device_id: music_assistant::device_id_for(user_id, &player.player_id),
        player_id: player.player_id,
        name: player.name,
        kind: player.kind.as_str().to_string(),
        available: player.available,
        state: player.state,
        volume_level: player.volume_level,
        volume_muted: player.volume_muted,
        group_members: player.group_members,
        synced_to: player.synced_to,
        can_group_with: player.can_group_with,
        queue_id,
    }
}

#[utoipa::path(
    get,
    path = "/music_assistant/settings",
    tag = "music_assistant",
    summary = "Get Music Assistant settings",
    params(MaUserQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Per-user Music Assistant configuration. The token is never returned.", body = MaSettingsResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn get_music_assistant_settings(
    Query(query): Query<MaUserQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<MaSettingsResponse>, AppError> {
    authorize(&state, &headers, query.user_id).await?;
    let view = music_assistant::load_settings_view(&state, query.user_id).await?;
    Ok(Json(settings_response(view)))
}

#[utoipa::path(
    put,
    path = "/music_assistant/settings",
    tag = "music_assistant",
    summary = "Save Music Assistant settings",
    request_body = MaSettingsBody,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Saved configuration (token redacted)", body = MaSettingsResponse),
        (status = 400, description = "Invalid URL or missing fields required to enable"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn update_music_assistant_settings(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<MaSettingsBody>,
) -> Result<Json<MaSettingsResponse>, AppError> {
    authorize(&state, &headers, body.user_id).await?;
    let view = music_assistant::save_settings(
        &state,
        SettingsUpdate {
            user_id: body.user_id,
            enabled: body.enabled,
            server_url: body.server_url,
            external_base_url: body.external_base_url,
            token: body.token,
            clear_token: body.clear_token,
        },
    )
    .await?;
    Ok(Json(settings_response(view)))
}

#[utoipa::path(
    post,
    path = "/music_assistant/test",
    tag = "music_assistant",
    summary = "Test a Music Assistant connection",
    request_body = MaTestBody,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Probe result. ok is false when MA rejected the token or the version is too old; the HTTP call itself succeeded.", body = MaTestResponse),
        (status = 400, description = "Server URL or external base URL is not usable. Localhost external base URLs are rejected because Music Assistant cannot fetch them."),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn test_music_assistant(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<MaTestBody>,
) -> Result<Json<MaTestResponse>, AppError> {
    authorize(&state, &headers, body.user_id).await?;
    if let Some(raw) = body
        .external_base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        // Same rules as save: localhost and link-local cannot be the stream origin.
        music_assistant::normalize_external_base(raw).map_err(AppError::bad_request)?;
    }
    let report = music_assistant::test_connection(
        &state,
        body.user_id,
        body.server_url.as_deref(),
        body.token.as_deref(),
    )
    .await?;
    Ok(Json(MaTestResponse {
        ok: report.ok,
        detail: report.detail,
        server_version: report.server_version,
        schema_version: report.schema_version,
        player_count: report.player_count,
        warning: report.warning,
    }))
}

#[utoipa::path(
    get,
    path = "/music_assistant/players",
    tag = "music_assistant",
    summary = "List Music Assistant players and groups",
    params(MaUserQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Players from the live session. Empty when disabled or not yet connected.", body = MaPlayersResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn list_music_assistant_players(
    Query(query): Query<MaUserQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<MaPlayersResponse>, AppError> {
    authorize(&state, &headers, query.user_id).await?;
    let view = music_assistant::load_settings_view(&state, query.user_id).await?;
    let players = state
        .music_assistant
        .players(query.user_id)
        .await
        .into_iter()
        .map(|p| player_response(query.user_id, p))
        .collect();
    Ok(Json(MaPlayersResponse {
        enabled: view.enabled,
        connected: view.connected,
        players,
    }))
}

#[utoipa::path(
    post,
    path = "/music_assistant/command",
    tag = "music_assistant",
    summary = "Control a Music Assistant player",
    request_body = MaCommandBody,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Command accepted by Music Assistant. warnings lists follow-up steps (seek) that did not apply. set_speed is ignored and is not a warning.", body = MaCommandResponse),
        (status = 400, description = "Unknown action, missing args, or Music Assistant rejected the command"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
        (status = 503, description = "Music Assistant is not connected"),
    ),
)]
pub async fn music_assistant_command(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(body): Json<MaCommandBody>,
) -> Result<Json<MaCommandResponse>, AppError> {
    authorize(&state, &headers, body.user_id).await?;
    let outcome = music_assistant::dispatch(
        &state,
        body.user_id,
        &body.player_id,
        &body.action,
        &body.args,
    )
    .await?;
    Ok(Json(MaCommandResponse {
        ok: true,
        detail: outcome.detail,
        warnings: outcome.warnings,
    }))
}
