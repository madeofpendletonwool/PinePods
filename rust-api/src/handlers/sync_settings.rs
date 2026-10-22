// Sync settings for the calling user: the PinePods gpodder server (on/off, the URL apps use, app
// passwords, devices), the external sync server (gpodder or Nextcloud) with its status, manual
// runs, and statistics read from the sync log.

use argon2::PasswordHasher;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use chrono::{NaiveDateTime, Utc};
use rand::RngExt as _;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, require_valid_api_key},
    services::scheduler::failure_backoff,
    services::sync::engine::{self, RunMode, RunSummary},
    services::sync::engine_db::{SyncTargetRow, TargetKind},
    AppState,
};

/// Overrides the server URL shown for gpodder apps (behind a reverse proxy, the URL users
/// reach Pinepods at). Without it the URL is worked out from the request.
pub const PUBLIC_URL_ENV: &str = "PINEPODS_PUBLIC_URL";

const APP_PASSWORD_LABEL: &str = "App password";
const MAX_LABEL: usize = 255;

async fn caller(state: &AppState, headers: &HeaderMap) -> AppResult<i32> {
    let api_key = extract_api_key(headers)?;
    require_valid_api_key(state, &api_key).await?;
    state.db_pool.get_user_id_from_api_key(&api_key).await
}

fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|v| v.to_str().ok()).and_then(|v| v.split(',').next()).map(str::trim).filter(|v| !v.is_empty())
}

/// The URL gpodder apps should use: `PINEPODS_PUBLIC_URL`, else the forwarded/Host headers of
/// this request. The flag says whether it was configured.
pub fn public_server_url(headers: &HeaderMap) -> (String, bool) {
    if let Some(url) = std::env::var(PUBLIC_URL_ENV).ok().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| !u.is_empty()) {
        return (url, true);
    }
    let host = header(headers, "x-forwarded-host").or_else(|| header(headers, "host")).unwrap_or("localhost");
    let scheme = header(headers, "x-forwarded-proto").filter(|p| p.eq_ignore_ascii_case("https")).map_or("http", |_| "https");
    (format!("{scheme}://{host}"), false)
}

fn epoch(t: Option<NaiveDateTime>) -> Option<i64> {
    t.map(|t| t.and_utc().timestamp())
}

// ---- the PinePods gpodder server ----------------------------------------------------------------

#[derive(Serialize, utoipa::ToSchema)]
pub struct SyncServerInfo {
    pub enabled: bool,
    /// The username to enter in apps.
    pub username: String,
    /// The server URL to enter in apps.
    pub server_url: String,
    /// `server_url` comes from `PINEPODS_PUBLIC_URL`; otherwise it was guessed from the request
    /// and a client may prefer the URL it reached Pinepods at.
    pub server_url_configured: bool,
    /// A notice for the settings page, e.g. `reenter_external`: the external sync server has to
    /// be entered again.
    pub notice: Option<String>,
}

async fn server_info(state: &AppState, headers: &HeaderMap, user_id: i32) -> AppResult<SyncServerInfo> {
    let db = &state.db_pool;
    let (server_url, server_url_configured) = public_server_url(headers);
    Ok(SyncServerInfo {
        enabled: db.sync_server_enabled(user_id).await?,
        username: db.gpodder_username(user_id).await?.unwrap_or_default(),
        server_url,
        server_url_configured,
        notice: db.sync_notice(user_id).await?,
    })
}

#[utoipa::path(
    get,
    path = "/server",
    tag = "sync",
    summary = "PinePods gpodder server settings",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SyncServerInfo),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_server(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<SyncServerInfo>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(server_info(&state, &headers, user_id).await?))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct SetServerRequest {
    pub enabled: bool,
}

/// Turn the PinePods gpodder server on or off. Turning it on publishes the current listening
/// state for apps; an external sync server is left as it is either way.
#[utoipa::path(
    put,
    path = "/server",
    tag = "sync",
    summary = "Turn the PinePods gpodder server on or off",
    request_body = SetServerRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SyncServerInfo),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn put_server(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SetServerRequest>,
) -> Result<Json<SyncServerInfo>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let db = &state.db_pool;
    let enabled = db.sync_server_enabled(user_id).await?;
    if request.enabled && !enabled {
        db.set_gpodder_internal_sync(user_id).await?;
    } else if !request.enabled && enabled {
        db.disable_gpodder_internal_sync(user_id).await?;
    }
    Ok(Json(server_info(&state, &headers, user_id).await?))
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct AppPasswordEntry {
    pub id: i32,
    pub label: String,
    /// Unix seconds.
    pub created_at: Option<i64>,
    /// Unix seconds; null if never used.
    pub last_used_at: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/server/app-passwords",
    tag = "sync",
    summary = "List app passwords",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = Vec<AppPasswordEntry>),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn list_app_passwords(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Vec<AppPasswordEntry>>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let list = state.db_pool.gpodder_app_password_list(user_id).await?;
    Ok(Json(
        list.into_iter()
            .map(|p| AppPasswordEntry { id: p.id, label: p.label, created_at: p.created_at, last_used_at: p.last_used_at })
            .collect(),
    ))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct CreateAppPasswordRequest {
    /// What it's for, e.g. "AntennaPod on my phone".
    pub label: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct CreatedAppPassword {
    pub id: i32,
    pub label: String,
    /// Shown this once; only a hash is stored.
    pub password: String,
}

fn new_app_password() -> String {
    const CHARSET: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::rng();
    let chars: Vec<char> = (0..20).map(|_| CHARSET[rng.random_range(0..CHARSET.len())] as char).collect();
    chars.chunks(4).map(|c| c.iter().collect::<String>()).collect::<Vec<_>>().join("-")
}

#[utoipa::path(
    post,
    path = "/server/app-passwords",
    tag = "sync",
    summary = "Create an app password",
    description = "Creates a password for a gpodder app. The password is in this response only.",
    request_body = CreateAppPasswordRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = CreatedAppPassword),
        (status = 400, description = "Label too long"),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn create_app_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateAppPasswordRequest>,
) -> Result<Json<CreatedAppPassword>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let label = request.label.as_deref().map(str::trim).filter(|l| !l.is_empty()).unwrap_or(APP_PASSWORD_LABEL).to_string();
    if label.chars().count() > MAX_LABEL {
        return Err(AppError::bad_request("The label is too long"));
    }
    let password = new_app_password();
    let to_hash = password.clone();
    let hash = tokio::task::spawn_blocking(move || argon2::Argon2::default().hash_password(to_hash.as_bytes()).map(|h| h.to_string()))
        .await
        .map_err(|_| AppError::internal("Hashing the app password failed"))?
        .map_err(|_| AppError::internal("Hashing the app password failed"))?;
    let id = state.db_pool.gpodder_create_app_password(user_id, &label, &hash).await?;
    Ok(Json(CreatedAppPassword { id, label, password }))
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SuccessResponse {
    pub success: bool,
}

#[utoipa::path(
    delete,
    path = "/server/app-passwords/{app_password_id}",
    tag = "sync",
    summary = "Revoke an app password",
    params(("app_password_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SuccessResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 404, description = "No such app password"),
    ),
)]
pub async fn revoke_app_password(
    State(state): State<AppState>,
    Path(app_password_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    if !state.db_pool.gpodder_revoke_app_password(user_id, app_password_id).await? {
        return Err(AppError::not_found("App password not found"));
    }
    crate::gpodder::forget_user(user_id);
    Ok(Json(SuccessResponse { success: true }))
}

#[utoipa::path(
    delete,
    path = "/notice",
    tag = "sync",
    summary = "Dismiss the sync settings notice",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SuccessResponse),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn dismiss_notice(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<SuccessResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    state.db_pool.sync_clear_notice(user_id).await?;
    Ok(Json(SuccessResponse { success: true }))
}

// ---- devices on the PinePods server ------------------------------------------------------------

#[derive(Serialize, utoipa::ToSchema)]
pub struct SyncDeviceEntry {
    pub id: i32,
    /// The id the app registered (what it addresses the device by).
    pub name: String,
    pub caption: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// Unix seconds.
    pub last_seen: Option<i64>,
    /// Pinepods' own device: can't be renamed or removed.
    pub protected: bool,
}

#[utoipa::path(
    get,
    path = "/devices",
    tag = "sync",
    summary = "Devices on the PinePods gpodder server",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = Vec<SyncDeviceEntry>),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn list_devices(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<Vec<SyncDeviceEntry>>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let devices = state.db_pool.gpodder_device_list(user_id).await?;
    Ok(Json(
        devices
            .into_iter()
            .map(|d| SyncDeviceEntry {
                id: d.id,
                caption: d.caption.unwrap_or_else(|| d.name.clone()),
                kind: d.kind.unwrap_or_else(|| "other".into()),
                name: d.name,
                last_seen: d.last_seen,
                protected: d.system,
            })
            .collect(),
    ))
}

async fn client_device(state: &AppState, user_id: i32, device_id: i32) -> AppResult<()> {
    match state.db_pool.gpodder_device_by_id(user_id, device_id).await? {
        None => Err(AppError::not_found("Device not found")),
        Some(d) if d.system => Err(AppError::forbidden("PinePods' own device can't be renamed or removed")),
        Some(_) => Ok(()),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct RenameDeviceRequest {
    pub caption: String,
}

#[utoipa::path(
    put,
    path = "/devices/{device_id}",
    tag = "sync",
    summary = "Rename a device",
    params(("device_id" = i32, Path)),
    request_body = RenameDeviceRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SuccessResponse),
        (status = 400, description = "Empty or too long name"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "PinePods' own device"),
        (status = 404, description = "No such device"),
    ),
)]
pub async fn rename_device(
    State(state): State<AppState>,
    Path(device_id): Path<i32>,
    headers: HeaderMap,
    Json(request): Json<RenameDeviceRequest>,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let caption = request.caption.trim();
    if caption.is_empty() || caption.chars().count() > MAX_LABEL {
        return Err(AppError::bad_request("Enter a name of up to 255 characters"));
    }
    client_device(&state, user_id, device_id).await?;
    state.db_pool.gpodder_rename_device(user_id, device_id, caption).await?;
    Ok(Json(SuccessResponse { success: true }))
}

#[utoipa::path(
    delete,
    path = "/devices/{device_id}",
    tag = "sync",
    summary = "Remove a device",
    description = "Removes an app's device. Its history stays; the app gets a new device the next time it syncs.",
    params(("device_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SuccessResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "PinePods' own device"),
        (status = 404, description = "No such device"),
    ),
)]
pub async fn remove_device(
    State(state): State<AppState>,
    Path(device_id): Path<i32>,
    headers: HeaderMap,
) -> Result<Json<SuccessResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    client_device(&state, user_id, device_id).await?;
    state.db_pool.gpodder_remove_device(user_id, device_id).await?;
    Ok(Json(SuccessResponse { success: true }))
}

// ---- the external sync server -------------------------------------------------------------------

#[derive(Serialize, utoipa::ToSchema)]
pub struct SyncTargetInfo {
    /// `gpodder` or `nextcloud`.
    pub kind: String,
    pub url: String,
    pub username: String,
    /// Pinepods' device on a gpodder server.
    pub device_name: Option<String>,
    pub enabled: bool,
    /// The next run is a full sync (new server, or a full sync that hasn't succeeded yet).
    pub needs_full_sync: bool,
    /// Unix seconds.
    pub last_attempt_at: Option<i64>,
    /// Unix seconds.
    pub last_success_at: Option<i64>,
    pub last_error: Option<String>,
    pub consecutive_failures: i32,
    /// Automatic syncs are paused after failures until `retry_at`; "Sync now" still runs.
    pub in_backoff: bool,
    /// Unix seconds.
    pub retry_at: Option<i64>,
}

pub fn target_info(t: &SyncTargetRow) -> SyncTargetInfo {
    let in_backoff = t.in_backoff(Utc::now().naive_utc());
    SyncTargetInfo {
        kind: t.kind.as_str().to_string(),
        url: t.url.clone(),
        username: t.username.clone(),
        device_name: t.device_name.clone(),
        enabled: t.enabled,
        needs_full_sync: t.needs_full_sync,
        last_attempt_at: epoch(t.last_attempt_at),
        last_success_at: epoch(t.last_success_at),
        last_error: t.last_error.clone(),
        consecutive_failures: t.consecutive_failures,
        in_backoff,
        retry_at: in_backoff.then(|| epoch(t.last_attempt_at.map(|at| at + failure_backoff(t.consecutive_failures)))).flatten(),
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SyncTargetResponse {
    /// Null when no external sync server is set up.
    pub target: Option<SyncTargetInfo>,
}

async fn current_target(state: &AppState, user_id: i32) -> AppResult<Option<SyncTargetRow>> {
    Ok(state.db_pool.sync_targets_for_user(user_id).await?.into_iter().next())
}

#[utoipa::path(
    get,
    path = "/target",
    tag = "sync",
    summary = "External sync server and its status",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SyncTargetResponse),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_target(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<SyncTargetResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(SyncTargetResponse { target: current_target(&state, user_id).await?.as_ref().map(target_info) }))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct TargetCredentials {
    /// `gpodder` or `nextcloud`.
    pub kind: String,
    pub url: String,
    pub username: String,
    /// Leave out to reuse the stored password of the same server and account.
    pub password: Option<String>,
    /// gpodder only: the device Pinepods syncs as. Left out: keep the current one, or pick one.
    pub device_name: Option<String>,
}

struct Resolved {
    kind: TargetKind,
    url: String,
    username: String,
    password: String,
    device_name: Option<String>,
}

/// Validate the fields and find the password: the one given, else the stored one for the same
/// server and account.
async fn resolve(state: &AppState, user_id: i32, c: TargetCredentials) -> AppResult<Resolved> {
    let kind = TargetKind::parse(c.kind.trim()).ok_or_else(|| AppError::bad_request("The kind must be gpodder or nextcloud"))?;
    let url = c.url.trim().trim_end_matches('/').to_string();
    let username = c.username.trim().to_string();
    if url.is_empty() || username.is_empty() {
        return Err(AppError::bad_request("Enter the server URL and username"));
    }
    let device_name = c.device_name.map(|d| d.trim().to_string()).filter(|d| !d.is_empty() && kind == TargetKind::Gpodder);
    if device_name.as_ref().is_some_and(|d| d.chars().count() > MAX_LABEL) {
        return Err(AppError::bad_request("The device name is too long"));
    }
    let password = match c.password.filter(|p| !p.is_empty()) {
        Some(p) => p,
        None => {
            let stored = current_target(state, user_id)
                .await?
                .filter(|t| t.kind == kind && t.url.trim_end_matches('/') == url && t.username == username)
                .ok_or_else(|| AppError::bad_request("Enter the password"))?;
            state
                .db_pool
                .decrypt_password(&stored.encrypted_secret)
                .await
                .map_err(|_| AppError::bad_request("The stored password can't be read; enter it again"))?
        }
    };
    Ok(Resolved { kind, url, username, password, device_name })
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct RemoteDeviceEntry {
    pub id: String,
    pub caption: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub subscriptions: i64,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct TestTargetResponse {
    pub success: bool,
    pub message: String,
    /// gpodder only: the account's devices.
    pub devices: Vec<RemoteDeviceEntry>,
    /// gpodder only: the device Pinepods would sync as.
    pub device_name: Option<String>,
}

/// Check a server's credentials. Saves nothing and creates no device.
#[utoipa::path(
    post,
    path = "/target/test",
    tag = "sync",
    summary = "Test an external sync server",
    description = "Signs in with the given credentials (gpodder: and lists the devices) without saving anything or creating a device.",
    request_body = TargetCredentials,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Result of the test", body = TestTargetResponse),
        (status = 400, description = "Missing or invalid fields"),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn test_target(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TargetCredentials>,
) -> Result<Json<TestTargetResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let r = resolve(&state, user_id, request).await?;
    Ok(Json(match engine::test_connection(r.kind, &r.url, &r.username, &r.password, r.device_name.as_deref()).await {
        Ok(check) => TestTargetResponse {
            success: true,
            message: "Connected".into(),
            devices: check
                .devices
                .into_iter()
                .map(|d| RemoteDeviceEntry { id: d.id, caption: d.caption, kind: d.kind, subscriptions: d.subscriptions })
                .collect(),
            device_name: check.device_name,
        },
        Err(e) => TestTargetResponse { success: false, message: engine::error_message(&e), devices: Vec::new(), device_name: None },
    }))
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SaveTargetResponse {
    pub target: SyncTargetInfo,
    /// The background task running the first (full) sync.
    pub task_id: Option<String>,
}

/// Set up the external sync server, replacing any other. The credentials are checked first;
/// the first (full) sync then runs in the background as a task.
#[utoipa::path(
    put,
    path = "/target",
    tag = "sync",
    summary = "Set up the external sync server",
    request_body = TargetCredentials,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Saved; the first sync is running", body = SaveTargetResponse),
        (status = 400, description = "Invalid fields, or the server refused the credentials"),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn put_target(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TargetCredentials>,
) -> Result<Json<SaveTargetResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let r = resolve(&state, user_id, request).await?;
    engine::test_connection(r.kind, &r.url, &r.username, &r.password, r.device_name.as_deref())
        .await
        .map_err(|e| AppError::bad_request(engine::error_message(&e)))?;

    let db = &state.db_pool;
    let target_id = db.sync_target_save(user_id, r.kind, &r.url, &r.username, &r.password).await?;
    if let Some(device) = &r.device_name {
        db.sync_target_set_device(target_id, device).await?;
    }

    // The first sync runs as a task, so its progress shows with the other background tasks.
    let (db_bg, tasks) = (db.clone(), state.task_spawner.clone());
    let task_id = match state
        .task_spawner
        .spawn_progress_task("sync_setup".to_string(), user_id, move |reporter| async move {
            let summary = engine::run_reporting(&db_bg, &tasks, user_id, target_id, RunMode::Full, Some(&*reporter)).await?;
            reporter.update_progress(100.0, Some("First sync finished".to_string())).await?;
            Ok(serde_json::to_value(summary)?)
        })
        .await
    {
        Ok(id) => Some(id),
        Err(e) => {
            warn!("Couldn't start the first sync for user {} as a task: {}", user_id, e);
            let (db_bg, tasks) = (db.clone(), state.task_spawner.clone());
            tokio::spawn(async move {
                let _ = engine::run(&db_bg, &tasks, user_id, target_id, RunMode::Full).await;
            });
            None
        }
    };

    let target = db.sync_target(target_id).await?.ok_or_else(|| AppError::internal("The sync server wasn't saved"))?;
    Ok(Json(SaveTargetResponse { target: target_info(&target), task_id }))
}

#[utoipa::path(
    delete,
    path = "/target",
    tag = "sync",
    summary = "Remove the external sync server",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SuccessResponse),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn delete_target(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<SuccessResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    state.db_pool.sync_target_delete(user_id).await?;
    Ok(Json(SuccessResponse { success: true }))
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct RunQuery {
    /// Full sync: pull everything again and send the whole library and listening state.
    pub full: Option<bool>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct RunSummaryInfo {
    pub full: bool,
    pub subscriptions_queued: usize,
    pub subscriptions_removed: usize,
    pub subscriptions_pushed: usize,
    pub actions_pulled: usize,
    pub actions_applied: usize,
    pub actions_pushed: usize,
}

impl From<RunSummary> for RunSummaryInfo {
    fn from(s: RunSummary) -> Self {
        RunSummaryInfo {
            full: s.full,
            subscriptions_queued: s.subscriptions_queued,
            subscriptions_removed: s.subscriptions_removed,
            subscriptions_pushed: s.subscriptions_pushed,
            actions_pulled: s.actions_pulled,
            actions_applied: s.actions_applied,
            actions_pushed: s.actions_pushed,
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct RunResponse {
    pub success: bool,
    /// The error, when the run failed (also stored on the target).
    pub message: Option<String>,
    pub summary: Option<RunSummaryInfo>,
    pub target: Option<SyncTargetInfo>,
}

/// Sync with the external server now ("Sync now"), or run a full sync. Ignores failure backoff.
#[utoipa::path(
    post,
    path = "/run",
    tag = "sync",
    summary = "Sync with the external server now",
    params(RunQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "The run finished (see success)", body = RunResponse),
        (status = 400, description = "No external sync server is set up"),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn run_sync(
    State(state): State<AppState>,
    Query(query): Query<RunQuery>,
    headers: HeaderMap,
) -> Result<Json<RunResponse>, AppError> {
    let user_id = caller(&state, &headers).await?;
    let mode = if query.full.unwrap_or(false) { RunMode::Full } else { RunMode::Incremental };
    let result = engine::run_for_user(&state.db_pool, &state.task_spawner, user_id, mode).await;
    let target = current_target(&state, user_id).await?.as_ref().map(target_info);
    Ok(Json(match result {
        Ok(Some(summary)) => RunResponse { success: true, message: None, summary: Some(summary.into()), target },
        Ok(None) => return Err(AppError::bad_request("No external sync server is set up")),
        Err(e) => RunResponse { success: false, message: Some(engine::error_message(&e)), summary: None, target },
    }))
}

// ---- statistics ---------------------------------------------------------------------------------

#[derive(Serialize, utoipa::ToSchema)]
pub struct SyncActionEntry {
    pub podcast: String,
    pub episode: String,
    pub action: String,
    /// Unix seconds (the action's own time).
    pub timestamp: i64,
    pub position: Option<i32>,
    pub device: Option<String>,
    /// `pinepods`, `client` (an app on the PinePods server) or `target` (the external server).
    pub origin: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SyncTargetQueue {
    pub kind: String,
    pub url: String,
    /// Unix seconds.
    pub last_success_at: Option<i64>,
    /// Changes waiting to be sent; null until the first sync has succeeded.
    pub pending_subscriptions: Option<i64>,
    pub pending_actions: Option<i64>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct SyncStats {
    pub server_enabled: bool,
    /// Podcasts shared with apps and the external server.
    pub subscriptions: usize,
    /// Apps on the PinePods server (not counting Pinepods itself).
    pub devices: usize,
    pub total_episode_actions: i64,
    /// Unix seconds: when the newest episode action was received.
    pub last_change: Option<i64>,
    pub recent_episode_actions: Vec<SyncActionEntry>,
    pub target: Option<SyncTargetQueue>,
}

fn origin_kind(origin: &str) -> &'static str {
    match origin {
        "pinepods" => "pinepods",
        o if o.starts_with("target:") => "target",
        _ => "client",
    }
}

pub async fn build_stats(state: &AppState, user_id: i32) -> AppResult<SyncStats> {
    let db = &state.db_pool;
    let stats = db.gpodder_server_stats(user_id).await?;
    let system: Vec<String> = db.gpodder_device_list(user_id).await?.into_iter().filter(|d| d.system).map(|d| d.name).collect();
    let target = match current_target(state, user_id).await? {
        Some(t) => {
            let pending = if t.last_success_at.is_some() { Some(db.sync_pending_counts(&t).await?) } else { None };
            Some(SyncTargetQueue {
                kind: t.kind.as_str().to_string(),
                url: t.url.clone(),
                last_success_at: epoch(t.last_success_at),
                pending_subscriptions: pending.map(|p| p.0),
                pending_actions: pending.map(|p| p.1),
            })
        }
        None => None,
    };
    Ok(SyncStats {
        server_enabled: db.sync_server_enabled(user_id).await?,
        subscriptions: stats.library.len(),
        devices: stats.devices.iter().filter(|d| !system.contains(&d.name)).count(),
        total_episode_actions: stats.total_actions,
        last_change: stats.last_change,
        recent_episode_actions: stats
            .recent_actions
            .into_iter()
            .map(|a| SyncActionEntry {
                origin: origin_kind(&a.origin).to_string(),
                podcast: a.podcast,
                episode: a.episode,
                action: a.action,
                timestamp: a.timestamp,
                position: a.position,
                device: a.device,
            })
            .collect(),
        target,
    })
}

#[utoipa::path(
    get,
    path = "/stats",
    tag = "sync",
    operation_id = "sync_stats",
    summary = "Sync statistics",
    description = "Read from the sync log; no server is contacted.",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = SyncStats),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn get_stats(State(state): State<AppState>, headers: HeaderMap) -> Result<Json<SyncStats>, AppError> {
    let user_id = caller(&state, &headers).await?;
    Ok(Json(build_stats(&state, user_id).await?))
}

/// The sync fields of the user stats. Older clients read `Pod_Sync_Type` and `GpodderUrl` and
/// only know 'gpodder' (internal when the URL is the internal marker, else an external server),
/// 'nextcloud' and 'None'; the other fields describe the current setup. `SyncServerUrl` is only
/// set when configured: otherwise the client's own server address is the better answer.
pub async fn user_stats_sync_fields(state: &AppState, headers: &HeaderMap, user_id: i32) -> AppResult<serde_json::Map<String, serde_json::Value>> {
    let db = &state.db_pool;
    let internal = db.sync_server_enabled(user_id).await?;
    let target = current_target(state, user_id).await?;
    let (legacy_type, legacy_url) = match (&target, internal) {
        (_, true) => ("gpodder", crate::database::INTERNAL_GPODDER_URL.to_string()),
        (Some(t), false) if t.kind == TargetKind::Gpodder => ("gpodder", t.url.clone()),
        (Some(t), false) => ("nextcloud", t.url.clone()),
        (None, false) => ("None", String::new()),
    };
    let fields = serde_json::json!({
        "Pod_Sync_Type": legacy_type,
        "GpodderUrl": legacy_url,
        "SyncServerEnabled": internal,
        "SyncServerUrl": Some(public_server_url(headers)).filter(|(_, configured)| internal && *configured).map(|(url, _)| url),
        "SyncTargetKind": target.as_ref().map(|t| t.kind.as_str()),
        "SyncTargetUrl": target.as_ref().map(|t| t.url.clone()),
    });
    Ok(match fields {
        serde_json::Value::Object(m) => m,
        _ => serde_json::Map::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_passwords_are_grouped_and_unambiguous() {
        let p = new_app_password();
        assert_eq!(p.len(), 24);
        assert_eq!(p.split('-').count(), 5);
        assert!(p.chars().all(|c| c == '-' || (c.is_ascii_alphanumeric() && !"01lo".contains(c))));
        assert_ne!(new_app_password(), p);
    }

    #[test]
    fn server_url_from_forwarded_headers() {
        let mut h = HeaderMap::new();
        h.insert("host", "pinepods:8040".parse().unwrap());
        assert_eq!(public_server_url(&h).0, "http://pinepods:8040");
        h.insert("x-forwarded-host", "pods.example.com".parse().unwrap());
        h.insert("x-forwarded-proto", "https".parse().unwrap());
        assert_eq!(public_server_url(&h).0, "https://pods.example.com");
        // Chained proxies: the first entry is the client-facing one.
        h.insert("x-forwarded-host", "pods.example.com:8443, pinepods:8040".parse().unwrap());
        h.insert("x-forwarded-proto", "https, http".parse().unwrap());
        assert_eq!(public_server_url(&h).0, "https://pods.example.com:8443");
    }
}
