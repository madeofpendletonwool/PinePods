use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use base64::{Engine as _, engine::general_purpose::STANDARD};

use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, check_user_or_admin_access},
    AppState,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

// Global storage for password-verified sessions pending MFA
// Key: session_token, Value: (user_id, timestamp)
lazy_static::lazy_static! {
    static ref PENDING_MFA_SESSIONS: Arc<Mutex<HashMap<String, (i32, u64)>>> = 
        Arc::new(Mutex::new(HashMap::new()));
}

#[derive(Serialize)]
pub struct LoginResponse {
    status: String,
    retrieved_key: Option<String>,
    mfa_required: Option<bool>,
    user_id: Option<i32>,
    mfa_session_token: Option<String>,
}

#[derive(Serialize)]
pub struct MfaRequiredResponse {
    status: String,
    mfa_required: bool,
    user_id: i32,
}

#[derive(Serialize)]
pub struct VerifyKeyResponse {
    status: String,
}

#[derive(Serialize)]
pub struct GetUserResponse {
    status: String,
    retrieved_id: i32,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
pub struct UserDetails {
    pub UserID: i32,
    pub Fullname: Option<String>,
    pub Username: Option<String>,
    pub Email: Option<String>,
    pub Hashed_PW: Option<String>,
    pub Salt: Option<String>,
}

#[derive(Serialize)]
pub struct SelfServiceStatusResponse {
    pub status: bool,
    pub first_admin_created: bool,
}

#[derive(Serialize)]
pub struct PublicOidcProvidersResponse {
    pub providers: Vec<PublicOidcProviderResponse>,
}

#[derive(Serialize)]
pub struct PublicOidcProviderResponse {
    pub provider_id: i32,
    pub provider_name: String,
    pub client_id: String,
    pub authorization_url: String,
    pub scope: String,
    pub button_color: String,
    pub button_text: String,
    pub button_text_color: String,
    pub icon_svg: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateFirstAdminRequest {
    pub username: String,
    pub password: String,
    pub email: String,
    pub fullname: String,
}

#[derive(Deserialize)]
pub struct TimeZoneInfo {
    pub user_id: i32,
    pub timezone: String,
    pub hour_pref: i32,
    pub date_format: String,
}

#[derive(Deserialize)]
pub struct UpdateTimezoneRequest {
    pub user_id: i32,
    pub timezone: String,
}

#[derive(Deserialize)]
pub struct UpdateDateFormatRequest {
    pub user_id: i32,
    pub date_format: String,
}

#[derive(Deserialize)]
pub struct UpdateTimeFormatRequest {
    pub user_id: i32,
    pub hour_pref: i32,
}
#[derive(Deserialize)]
pub struct UpdateAutoCompleteSecondsRequest {
    pub user_id: i32,
    pub seconds: i32,
}

#[derive(Deserialize)]
pub struct OPMLImportRequest {
    pub podcasts: Vec<String>,
    pub user_id: i32,
}

#[derive(Serialize)]
pub struct CreateFirstAdminResponse {
    pub message: String,
    pub user_id: i32,
}

#[derive(Serialize)]
pub struct ConfigResponse {
    pub api_url: String,
    pub proxy_url: String,
    pub proxy_host: String,
    pub proxy_port: String,
    pub proxy_protocol: String,
    pub reverse_proxy: String,
    pub people_url: String,
}

// Extract basic auth credentials from Authorization header
fn extract_basic_auth(headers: &HeaderMap) -> AppResult<(String, String)> {
    let auth_header = headers
        .get("Authorization")
        .ok_or_else(|| AppError::unauthorized("Missing Authorization header"))?
        .to_str()
        .map_err(|_| AppError::unauthorized("Invalid Authorization header"))?;

    if !auth_header.starts_with("Basic ") {
        return Err(AppError::unauthorized("Invalid Authorization scheme"));
    }

    let encoded = &auth_header[6..]; // Remove "Basic " prefix
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| AppError::unauthorized("Invalid base64 encoding"))?;
    
    let credentials = String::from_utf8(decoded)
        .map_err(|_| AppError::unauthorized("Invalid UTF-8 in credentials"))?;
    
    let parts: Vec<&str> = credentials.splitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(AppError::unauthorized("Invalid credentials format"));
    }

    Ok((parts[0].to_lowercase(), parts[1].to_string()))
}

// Get API key with basic authentication (username/password)
// Now includes MFA security check - API key only returned after MFA verification if enabled
pub async fn get_key(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<LoginResponse>, AppError> {
    let (username, password) = extract_basic_auth(&headers)?;
    
    // Verify password
    let is_valid = state.db_pool.verify_password(&username, &password).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid username or password"));
    }

    // Get user ID from username first
    let user_id = state.db_pool.get_user_id_from_username(&username).await?;
    
    // Check if MFA is enabled for this user - CRITICAL SECURITY CHECK
    let mfa_enabled = state.db_pool.check_mfa_enabled(user_id).await?;
    
    if mfa_enabled {
        // MFA is enabled - create secure session token and DO NOT return API key yet
        // Generate cryptographically secure session token
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::rng();
        let session_token: String = (0..32)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect();
        
        // Store session with timestamp (expires in 5 minutes)
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        {
            let mut sessions = PENDING_MFA_SESSIONS.lock()
                .map_err(|e| AppError::internal(&format!("Failed to lock MFA sessions: {}", e)))?;
            sessions.insert(session_token.clone(), (user_id, timestamp));
        }
        
        // User must complete MFA verification first using this session token
        return Ok(Json(LoginResponse {
            status: "mfa_required".to_string(),
            retrieved_key: None,
            mfa_required: Some(true),
            user_id: Some(user_id),
            mfa_session_token: Some(session_token),
        }));
    }
    
    // MFA not enabled - proceed with normal flow
    let api_key = state.db_pool.create_or_get_api_key(user_id).await?;
    
    Ok(Json(LoginResponse {
        status: "success".to_string(),
        retrieved_key: Some(api_key),
        mfa_required: Some(false),
        user_id: Some(user_id),
        mfa_session_token: None,
    }))
}

// Verify API key validity
pub async fn verify_api_key_endpoint(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<VerifyKeyResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    Ok(Json(VerifyKeyResponse {
        status: "success".to_string(),
    }))
}

// Get user ID from API key
pub async fn get_user(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<GetUserResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    Ok(Json(GetUserResponse {
        status: "success".to_string(),
        retrieved_id: user_id,
    }))
}

// Get user details by user ID
pub async fn get_user_details_by_id(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<UserDetails>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    let is_valid = state.db_pool.verify_api_key(&api_key).await?;
    if !is_valid {
        return Err(AppError::unauthorized("Invalid API key"));
    }

    // Check authorization: users can only get their own details, have web key access, or be admin
    if !check_user_or_admin_access(&state, &api_key, user_id).await? {
        return Err(AppError::forbidden("Access denied to user details"));
    }

    // Get user details
    let user_details = state.db_pool.get_user_details_by_id(user_id).await?;
    
    Ok(Json(user_details))
}

// Get self-service status - matches Python api_self_service_status
pub async fn get_self_service_status(
    State(state): State<AppState>,
) -> Result<Json<SelfServiceStatusResponse>, AppError> {
    let status = state.db_pool.get_self_service_status().await?;
    
    Ok(Json(SelfServiceStatusResponse {
        status: status.status,
        first_admin_created: status.admin_exists,
    }))
}

// Get public OIDC providers - matches Python api_public_oidc_providers
pub async fn get_public_oidc_providers(
    State(state): State<AppState>,
) -> Result<Json<PublicOidcProvidersResponse>, AppError> {
    let providers = state.db_pool.get_public_oidc_providers().await?;
    
    let response_providers: Vec<PublicOidcProviderResponse> = providers
        .into_iter()
        .map(|p| PublicOidcProviderResponse {
            provider_id: p.provider_id,
            provider_name: p.provider_name,
            client_id: p.client_id,
            authorization_url: p.authorization_url,
            scope: p.scope,
            button_color: p.button_color,
            button_text: p.button_text,
            button_text_color: p.button_text_color,
            icon_svg: p.icon_svg,
        })
        .collect();
    
    Ok(Json(PublicOidcProvidersResponse {
        providers: response_providers,
    }))
}

// Create first admin - matches Python create_first_admin
pub async fn create_first_admin(
    State(state): State<AppState>,
    Json(request): Json<CreateFirstAdminRequest>,
) -> Result<Json<CreateFirstAdminResponse>, AppError> {
    // Check if admin already exists
    if state.db_pool.check_admin_exists().await? {
        return Err(AppError::forbidden("An admin user already exists"));
    }
    
    // Add the admin user
    let user_id = state.db_pool.add_admin_user(
        &request.fullname,
        &request.username.to_lowercase(),
        &request.email,
        &request.password, // Password should already be hashed by frontend
    ).await?;
    
    // Add PinePods news feed to admin users (matches Python startup tasks)
    if let Err(e) = state.db_pool.add_news_feed_if_not_added().await {
        eprintln!("Failed to add PinePods news feed during first admin creation: {}", e);
        // Don't fail the admin creation if news feed addition fails
    }
    
    Ok(Json(CreateFirstAdminResponse {
        message: "Admin user created successfully".to_string(),
        user_id,
    }))
}

// Get configuration - matches Python api_config
pub async fn get_config(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<ConfigResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get configuration from environment variables (same as Python)
    let proxy_host = std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string());
    let proxy_port = std::env::var("PINEPODS_PORT").unwrap_or_else(|_| "8040".to_string());
    let proxy_protocol = std::env::var("PROXY_PROTOCOL").unwrap_or_else(|_| "http".to_string());
    let reverse_proxy = std::env::var("REVERSE_PROXY").unwrap_or_else(|_| "False".to_string());
    let api_url = std::env::var("SEARCH_API_URL").unwrap_or_else(|_| "https://search.pinepods.online/api/search".to_string());
    let people_url = std::env::var("PEOPLE_API_URL").unwrap_or_else(|_| "https://people.pinepods.online".to_string());
    
    // Build proxy URL based on reverse proxy setting
    let proxy_url = if reverse_proxy == "True" {
        format!("{}://{}/mover/?url=", proxy_protocol, proxy_host)
    } else {
        format!("{}://{}:{}/mover/?url=", proxy_protocol, proxy_host, proxy_port)
    };
    
    Ok(Json(ConfigResponse {
        api_url,
        proxy_url,
        proxy_host,
        proxy_port,
        proxy_protocol,
        reverse_proxy,
        people_url,
    }))
}

// First login done - matches Python first_login_done
pub async fn first_login_done(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let key_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user (Python checks this)
    if key_user_id != user_id {
        return Err(AppError::forbidden("You can only run first login for yourself!"));
    }
    
    let first_login_status = state.db_pool.first_login_done(user_id).await?;
    
    Ok(Json(json!({
        "FirstLogin": first_login_status
    })))
}

// Check MFA enabled - matches Python check_mfa_enabled
pub async fn check_mfa_enabled(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let key_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user (Python checks this)
    if key_user_id != user_id {
        return Err(AppError::forbidden("You are not authorized to check mfa status for other users."));
    }
    
    let is_enabled = state.db_pool.check_mfa_enabled(user_id).await?;
    
    Ok(Json(json!({
        "mfa_enabled": is_enabled
    })))
}

// NEW SECURE MFA ENDPOINT: Verify MFA code and return API key during login
// CRITICAL SECURITY: This is the second phase of secure MFA authentication flow
// It REQUIRES a valid session token from successful password authentication
#[derive(Deserialize)]
pub struct VerifyMfaLoginRequest {
    pub mfa_session_token: String,
    pub mfa_code: String,
}

#[derive(Serialize)]
pub struct VerifyMfaLoginResponse {
    pub status: String,
    pub retrieved_key: Option<String>,
    pub verified: bool,
}

// Helper function to clean expired MFA sessions
fn cleanup_expired_mfa_sessions() -> Result<(), AppError> {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    let mut sessions = PENDING_MFA_SESSIONS.lock()
        .map_err(|e| AppError::internal(&format!("Failed to lock MFA sessions: {}", e)))?;
    
    sessions.retain(|_, (_, timestamp)| {
        current_time - *timestamp < 300 // Keep sessions newer than 5 minutes
    });
    
    Ok(())
}

// Verify MFA code during login and return API key - SECURE TWO-FACTOR AUTHENTICATION
// CRITICAL: This endpoint REQUIRES a valid session token proving password was verified first
pub async fn verify_mfa_and_get_key(
    State(state): State<AppState>,
    Json(request): Json<VerifyMfaLoginRequest>,
) -> Result<Json<VerifyMfaLoginResponse>, AppError> {
    // Clean up expired sessions first
    cleanup_expired_mfa_sessions()?;
    
    // CRITICAL SECURITY CHECK: Validate session token from password authentication
    let user_id = {
        let mut sessions = PENDING_MFA_SESSIONS.lock()
            .map_err(|e| AppError::internal(&format!("Failed to lock MFA sessions: {}", e)))?;
        
        match sessions.remove(&request.mfa_session_token) {
            Some((user_id, timestamp)) => {
                // Check if session is still valid (5 minutes)
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                if current_time - timestamp > 300 {
                    return Ok(Json(VerifyMfaLoginResponse {
                        status: "session_expired".to_string(),
                        retrieved_key: None,
                        verified: false,
                    }));
                }
                
                user_id
            }
            None => {
                return Ok(Json(VerifyMfaLoginResponse {
                    status: "invalid_session".to_string(),
                    retrieved_key: None,
                    verified: false,
                }));
            }
        }
    };

    // Get MFA secret for user - matches existing verify_mfa function exactly
    let mfa_secret = match state.db_pool.get_mfa_secret(user_id).await? {
        Some(secret) => secret,
        None => {
            return Ok(Json(VerifyMfaLoginResponse {
                status: "no_mfa_secret".to_string(),
                retrieved_key: None,
                verified: false,
            }));
        }
    };

    // Verify MFA code - matches existing verify_mfa function EXACTLY
    use totp_rs::{Algorithm, Secret, TOTP};
    
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1, 
        30,
        Secret::Encoded(mfa_secret.clone()).to_bytes()
            .map_err(|e| AppError::internal(&format!("Invalid MFA secret format: {}", e)))?,
        Some("Pinepods".to_string()), // Matches existing function exactly
        "login".to_string(),          // Matches existing function exactly
    ).map_err(|e| AppError::internal(&format!("TOTP creation failed: {}", e)))?;

    let verified = totp.check_current(&request.mfa_code)
        .map_err(|e| AppError::internal(&format!("TOTP verification failed: {}", e)))?;

    if verified {
        // MFA verification successful - now safe to return API key
        // Session token was consumed above, preventing replay attacks
        let api_key = state.db_pool.create_or_get_api_key(user_id).await?;
        
        Ok(Json(VerifyMfaLoginResponse {
            status: "success".to_string(),
            retrieved_key: Some(api_key),
            verified: true,
        }))
    } else {
        // MFA verification failed
        Ok(Json(VerifyMfaLoginResponse {
            status: "invalid_code".to_string(),
            retrieved_key: None,
            verified: false,
        }))
    }
}

// Get theme - matches Python get_theme
pub async fn get_theme(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let key_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user (Python checks this)
    if key_user_id != user_id {
        return Err(AppError::forbidden("You can only get themes for yourself!"));
    }
    
    let theme = state.db_pool.get_theme(user_id).await?;
    
    Ok(Json(json!({
        "theme": theme
    })))
}

// Get user startpage - matches Python get_user_startpage
pub async fn get_user_startpage(
    Query(params): Query<std::collections::HashMap<String, String>>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user_id from query parameter
    let user_id: i32 = params.get("user_id")
        .ok_or_else(|| AppError::bad_request("Missing user_id parameter"))?
        .parse()
        .map_err(|_| AppError::bad_request("Invalid user_id parameter"))?;
    
    // Get user ID from API key for authorization check
    let key_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user (Python checks this)
    if key_user_id != user_id {
        return Err(AppError::forbidden("You can only view your own StartPage setting!"));
    }
    
    let startpage = state.db_pool.get_user_startpage(user_id).await?;
    
    Ok(Json(json!({
        "StartPage": startpage
    })))
}

// Setup time info - matches Python setup_timezone_info
pub async fn setup_time_info(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<TimeZoneInfo>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user (Python checks this)
    if data.user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to access these user details"));
    }
    
    let success = state.db_pool.setup_timezone_info(
        data.user_id,
        &data.timezone,
        data.hour_pref,
        &data.date_format,
    ).await?;
    
    if success {
        Ok(Json(json!({
            "success": success
        })))
    } else {
        Err(AppError::not_found("User not found"))
    }
}

// User admin check - matches Python api_user_admin_check_route
pub async fn user_admin_check(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user (Python checks this)
    if user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to check admin status for other users"));
    }
    
    let is_admin = state.db_pool.user_admin_check(user_id).await?;
    
    Ok(Json(json!({
        "is_admin": is_admin
    })))
}

// Import progress - matches Python api_import_progress
pub async fn import_progress(
    Path(user_id): Path<i32>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Invalid API key"));
    }
    
    // Get user ID from API key for authorization check
    let key_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user
    if key_user_id != user_id {
        return Err(AppError::forbidden("You can only check import progress for yourself!"));
    }
    
    // Get progress from Redis
    let progress_key = format!("import_progress:{}", user_id);
    let progress_data: Option<String> = state.redis_client.get(&progress_key).await?;
    
    if let Some(data) = progress_data {
        let progress: serde_json::Value = serde_json::from_str(&data)?;
        Ok(Json(progress))
    } else {
        // No progress data found - import not running or completed
        Ok(Json(json!({
            "current": 0,
            "total": 0,
            "current_podcast": ""
        })))
    }
}

// Import OPML - matches Python api_import_opml
pub async fn import_opml(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(import_request): Json<OPMLImportRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Invalid API key"));
    }
    
    // Get user ID from API key for authorization check
    let key_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user (Python checks this)
    if key_user_id != import_request.user_id {
        return Err(AppError::forbidden("You can only import podcasts for yourself!"));
    }
    
    // Create background task for OPML import
    let _total_podcasts = import_request.podcasts.len();
    let task_id = state.task_manager.create_task(
        "opml_import".to_string(),
        import_request.user_id,
    ).await?;
    
    // Spawn the import task
    let task_spawner = state.task_spawner.clone();
    let db_pool = state.db_pool.clone();
    let task_manager = state.task_manager.clone();
    let redis_client = state.redis_client.clone();
    let task_id_clone = task_id.clone();
    
    tokio::spawn(async move {
        process_opml_import(
            import_request,
            task_id_clone,
            db_pool,
            task_manager,
            redis_client,
        ).await;
    });
    
    Ok(Json(json!({
        "success": true,
        "message": "Import process started",
        "task_id": task_id
    })))
}

// Process OPML import in background - matches Python process_opml_import
async fn process_opml_import(
    import_request: OPMLImportRequest,
    task_id: String,
    db_pool: crate::database::DatabasePool,
    task_manager: std::sync::Arc<crate::services::task_manager::TaskManager>,
    redis_client: crate::redis_client::RedisClient,
) {
    let total_podcasts = import_request.podcasts.len();
    let progress_key = format!("import_progress:{}", import_request.user_id);
    
    // Initialize progress in Redis
    let _ = redis_client.set_ex(&progress_key, &json!({
        "current": 0,
        "total": total_podcasts,
        "current_podcast": ""
    }).to_string(), 3600).await; // 1 hour timeout
    
    // Update task status to running
    let _ = task_manager.update_task_progress(
        &task_id,
        0.0,
        Some("Starting OPML import".to_string()),
    ).await;
    
    for (index, podcast_url) in import_request.podcasts.iter().enumerate() {
        // Update progress in Redis
        let _ = redis_client.set_ex(&progress_key, &json!({
            "current": index + 1,
            "total": total_podcasts,
            "current_podcast": podcast_url
        }).to_string(), 3600).await;
        
        // Update progress
        let progress = ((index + 1) as f64 / total_podcasts as f64) * 100.0;
        let _ = task_manager.update_task_progress(
            &task_id,
            progress,
            Some(format!("Processing podcast {}/{}: {}", index + 1, total_podcasts, podcast_url)),
        ).await;
        
        // Try to get podcast values and add podcast with robust error handling
        match get_podcast_values_from_url(podcast_url).await {
            Ok(mut podcast_values) => {
                podcast_values.user_id = import_request.user_id;
                match db_pool.add_podcast(&podcast_values, 0, None, None).await {
                    Ok(_) => {
                        tracing::info!("✅ Successfully imported podcast: {}", podcast_url);
                    }
                    Err(e) => {
                        tracing::error!("❌ Database error importing podcast {}: {} - Continuing with next podcast", podcast_url, e);
                    }
                }
            }
            Err(e) => {
                tracing::error!("❌ Feed parsing error for {}: {} - Continuing with next podcast", podcast_url, e);
            }
        }
        
        // Small delay to allow other requests to be processed
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    
    // Mark task as completed
    let _ = task_manager.update_task_progress(
        &task_id,
        100.0,
        Some("OPML import completed".to_string()),
    ).await;
    
    // Clear progress from Redis
    let _ = redis_client.delete(&progress_key).await;
}

// Get podcast values from URL - simplified version of Python get_podcast_values
async fn get_podcast_values_from_url(url: &str) -> Result<crate::handlers::podcasts::PodcastValues, AppError> {
    use std::collections::HashMap;
    
    let client = reqwest::Client::new();
    let response = client.get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .send()
        .await
        .map_err(|e| AppError::Http(e))?;
    
    let content = response.text().await.map_err(|e| AppError::Http(e))?;
    
    // Parse RSS feed to extract podcast information with Python-style comprehensive fallbacks
    use quick_xml::Reader;
    use quick_xml::events::Event;
    
    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);
    
    let mut metadata: HashMap<String, String> = HashMap::new();
    let mut current_tag = String::new();
    let mut current_text = String::new();
    let mut current_attrs: HashMap<String, String> = HashMap::new();
    let mut in_channel = false;
    let mut categories: HashMap<String, String> = HashMap::new();
    let mut category_counter = 0;
    
    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_text.clear();
                current_attrs.clear();
                
                // Track when we're in the channel section (not in items)
                if current_tag == "channel" {
                    in_channel = true;
                } else if current_tag == "item" {
                    in_channel = false;
                }
                
                // Store attributes
                for attr in e.attributes() {
                    if let Ok(attr) = attr {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let value = String::from_utf8_lossy(&attr.value).to_string();
                        current_attrs.insert(key, value);
                    }
                }
                
                // Handle iTunes image with href attribute (priority for artwork)
                if (current_tag == "itunes:image" || current_tag == "image") && in_channel {
                    if let Some(href) = current_attrs.get("href") {
                        if !href.trim().is_empty() {
                            metadata.insert("itunes_image_href".to_string(), href.clone());
                        }
                    }
                }
                
                // Handle iTunes category attributes  
                if current_tag == "itunes:category" && in_channel {
                    if let Some(text) = current_attrs.get("text") {
                        categories.insert(category_counter.to_string(), text.clone());
                        category_counter += 1;
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                // Handle self-closing tags
                current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_attrs.clear();
                
                // Store attributes from self-closing tag
                for attr in e.attributes() {
                    if let Ok(attr) = attr {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let value = String::from_utf8_lossy(&attr.value).to_string();
                        current_attrs.insert(key, value);
                    }
                }
                
                // Handle iTunes image with href attribute
                if (current_tag == "itunes:image" || current_tag == "image") && in_channel {
                    if let Some(href) = current_attrs.get("href") {
                        if !href.trim().is_empty() {
                            metadata.insert("itunes_image_href".to_string(), href.clone());
                        }
                    }
                }
                
                // Handle iTunes category attributes
                if current_tag == "itunes:category" && in_channel {
                    if let Some(text) = current_attrs.get("text") {
                        categories.insert(category_counter.to_string(), text.clone());
                        category_counter += 1;
                    }
                }
            }
            Ok(Event::Text(e)) => {
                current_text = e.decode().unwrap_or_default().into_owned();
            }
            Ok(Event::CData(e)) => {
                current_text = e.decode().unwrap_or_default().into_owned();
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                
                // Only store channel-level metadata, not item-level
                if in_channel && !current_text.trim().is_empty() {
                    metadata.insert(tag.clone(), current_text.clone());
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }
    
    // Apply Python-style comprehensive fallback logic for each field
    
    // Title - required field with robust fallbacks
    let podcast_title = metadata.get("title")
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "Unknown Podcast".to_string());
    
    // Author - multiple fallback sources like Python version
    let podcast_author = metadata.get("itunes:author")
        .or_else(|| metadata.get("author"))
        .or_else(|| metadata.get("managingEditor"))
        .or_else(|| metadata.get("dc:creator"))
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "Unknown Author".to_string());
    
    // Artwork - comprehensive fallback chain like Python version
    let podcast_artwork = metadata.get("itunes_image_href")
        .or_else(|| metadata.get("image_href"))
        .or_else(|| metadata.get("url"))  // From <image><url> tags
        .or_else(|| metadata.get("href")) // From <image href=""> attributes
        .filter(|s| !s.trim().is_empty() && s.starts_with("http"))
        .cloned()
        .unwrap_or_else(|| String::new());
    
    // Description - multiple fallback sources like Python version
    let podcast_description = metadata.get("itunes:summary")
        .or_else(|| metadata.get("description"))
        .or_else(|| metadata.get("subtitle"))
        .or_else(|| metadata.get("itunes:subtitle"))
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "No description available".to_string());
    
    // Website - link field
    let podcast_website = metadata.get("link")
        .filter(|s| !s.trim().is_empty() && s.starts_with("http"))
        .cloned()
        .unwrap_or_else(|| String::new());
    
    // Explicit - handle both string and boolean values like Python
    let podcast_explicit = metadata.get("itunes:explicit")
        .map(|s| {
            let lower = s.to_lowercase();
            lower == "yes" || lower == "true" || lower == "explicit" || lower == "1"
        })
        .unwrap_or(false);
    
    println!("🎙️  Parsed podcast: title='{}', author='{}', artwork='{}', description_len={}, website='{}', explicit={}, categories_count={}", 
        podcast_title, podcast_author, podcast_artwork, podcast_description.len(), podcast_website, podcast_explicit, categories.len());
    
    Ok(crate::handlers::podcasts::PodcastValues {
        pod_title: podcast_title,
        pod_artwork: podcast_artwork,
        pod_author: podcast_author,
        categories: categories,
        pod_description: podcast_description,
        pod_episode_count: 0,
        pod_feed_url: url.to_string(),
        pod_website: podcast_website,
        pod_explicit: podcast_explicit,
        user_id: 0, // Will be set by the caller
    })
}

// OIDC Authentication Flow Endpoints

// Store OIDC state - enhanced to capture user's current URL
#[derive(Deserialize)]
pub struct StoreStateRequest {
    pub state: String,
    pub client_id: String,
    pub origin_url: Option<String>, // URL user was on when they clicked OIDC login
}

#[derive(Serialize, Deserialize)]
struct StoredOidcState {
    client_id: String,
    origin_url: Option<String>,
}

pub async fn store_oidc_state(
    State(state): State<crate::AppState>,
    Json(request): Json<StoreStateRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Store state in Redis with 10-minute expiration
    let state_key = format!("oidc_state:{}", request.state);
    
    let stored_state = StoredOidcState {
        client_id: request.client_id,
        origin_url: request.origin_url,
    };
    
    let state_json = serde_json::to_string(&stored_state)
        .map_err(|e| AppError::internal(&format!("Failed to serialize OIDC state: {}", e)))?;
    
    state.redis_client.set_ex(&state_key, &state_json, 600).await
        .map_err(|e| AppError::internal(&format!("Failed to store OIDC state: {}", e)))?;
    
    Ok(Json(serde_json::json!({ "status": "success" })))
}

// Helper function to create proper redirect URLs for both web and mobile
fn create_oidc_redirect_url(frontend_base: &str, params: &str) -> String {
    if frontend_base.starts_with("pinepods://auth/callback") {
        // Mobile deep link - append params directly
        format!("{}?{}", frontend_base, params)
    } else {
        // Web callback - use traditional path
        format!("{}/oauth/callback?{}", frontend_base, params)
    }
}

// OIDC callback handler - matches Python /api/auth/callback endpoint
#[derive(Deserialize)]
pub struct OIDCCallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

pub async fn oidc_callback(
    State(state): State<crate::AppState>,
    headers: HeaderMap,
    Query(query): Query<OIDCCallbackQuery>,
) -> Result<axum::response::Redirect, AppError> {
    // Construct base URL from request like Python version - EXACT match
    let base_url = construct_base_url_from_request(&headers)?;
    let default_frontend_base = base_url.replace("/api", "");
    
    // Handle OAuth errors first - EXACT match to Python
    if let Some(error) = query.error {
        let error_desc = query.error_description.unwrap_or_else(|| "Unknown error".to_string());
        tracing::error!("OIDC provider error: {} - {}", error, error_desc);
        return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=provider_error&description={}", 
            default_frontend_base, urlencoding::encode(&error_desc))));
    }

    // Validate required parameters - EXACT match to Python
    let auth_code = query.code.ok_or_else(|| AppError::bad_request("Missing authorization code"))?;
    let state_param = query.state.ok_or_else(|| AppError::bad_request("Missing state parameter"))?;

    // Get client_id and origin_url from state
    let (client_id, stored_origin_url) = match state.redis_client.get_del(&format!("oidc_state:{}", state_param)).await {
        Ok(Some(state_json)) => {
            tracing::info!("OIDC Debug - Retrieved stored state: {}", state_json);
            // Try to parse as new JSON format first
            if let Ok(stored_state) = serde_json::from_str::<StoredOidcState>(&state_json) {
                tracing::info!("OIDC Debug - Parsed stored state: client_id={}, origin_url={:?}", stored_state.client_id, stored_state.origin_url);
                (stored_state.client_id, stored_state.origin_url)
            } else {
                // Fallback to old format (just client_id string) for backwards compatibility
                tracing::info!("OIDC Debug - Using fallback format, client_id={}", state_json);
                (state_json, None)
            }
        },
        Ok(None) => {
            return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=invalid_state", default_frontend_base)));
        }
        Err(_) => {
            return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=internal_error", default_frontend_base)));
        }
    };

    // Use stored origin URL if available, otherwise fall back to constructed URL
    let frontend_base = if let Some(ref origin_url) = stored_origin_url {
        tracing::info!("OIDC Debug - Using stored origin_url: {}", origin_url);
        // Check if this is a mobile deep link callback
        if origin_url.starts_with("pinepods://auth/callback") {
            tracing::info!("OIDC Debug - Detected mobile deep link origin");
            // For mobile deep links, use the full URL directly - don't try to parse as HTTP
            origin_url.clone()
        } else {
            tracing::info!("OIDC Debug - Detected web origin, parsing base URL");
            // Extract just the base part (scheme + host + port) from the stored origin URL for web
            // Simple string parsing to avoid adding url dependency
            if let Some(protocol_end) = origin_url.find("://") {
                let after_protocol = &origin_url[protocol_end + 3..];
                if let Some(path_start) = after_protocol.find('/') {
                    origin_url[..protocol_end + 3 + path_start].to_string()
                } else {
                    origin_url.clone()
                }
            } else {
                origin_url.clone()
            }
        }
    } else {
        tracing::info!("OIDC Debug - No stored origin_url, using default: {}", default_frontend_base);
        default_frontend_base.clone()
    };
    
    tracing::info!("OIDC Debug - Final frontend_base: {}", frontend_base);

    let registered_redirect_uri = format!("{}/api/auth/callback", base_url);

    // Get OIDC provider details - EXACT match to Python get_oidc_provider returning tuple
    let provider_tuple = match state.db_pool.get_oidc_provider(&client_id).await {
        Ok(Some(provider)) => provider,
        Ok(None) => {
            return Ok(axum::response::Redirect::to(&create_oidc_redirect_url(&frontend_base, "error=invalid_provider")));
        }
        Err(_) => {
            return Ok(axum::response::Redirect::to(&create_oidc_redirect_url(&frontend_base, "error=internal_error")));
        }
    };

    // Unpack provider details - EXACT match to Python unpacking
    let (provider_id, _client_id, client_secret, token_url, userinfo_url, name_claim, email_claim, username_claim, roles_claim, user_role, admin_role) = provider_tuple;

    // Exchange authorization code for access token - EXACT match to Python
    let client = reqwest::Client::new();
    let token_response = match client.post(&token_url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &auth_code),
            ("redirect_uri", &registered_redirect_uri),
            ("client_id", &client_id),
            ("client_secret", &client_secret),
        ])
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<serde_json::Value>().await {
                Ok(token_data) => token_data,
                Err(_) => return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=token_exchange_failed", frontend_base))),
            }
        }
        _ => return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=token_exchange_failed", frontend_base))),
    };

    let access_token = match token_response.get("access_token").and_then(|v| v.as_str()) {
        Some(token) => token,
        None => return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=token_exchange_failed", frontend_base))),
    };

    // Get user info from OIDC provider - EXACT match to Python
    let userinfo_response = match client.get(&userinfo_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("User-Agent", "PinePods/1.0")
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            match response.json::<serde_json::Value>().await {
                Ok(user_info) => user_info,
                Err(_) => return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=userinfo_failed", frontend_base))),
            }
        }
        _ => return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=userinfo_failed", frontend_base))),
    };

    // Extract email with GitHub special handling - EXACT match to Python
    let email_field = email_claim
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("email");
    
    tracing::info!("OIDC Debug - email_claim: {:?}, email_field: {}, userinfo_response: {:?}", email_claim, email_field, userinfo_response);
    
    let mut email = userinfo_response.get(email_field).and_then(|v| v.as_str()).map(|s| s.to_string());
    
    // GitHub email handling - EXACT match to Python
    if email.is_none() && userinfo_url.contains("api.github.com") {
        if let Ok(emails_response) = client.get("https://api.github.com/user/emails")
            .header("Authorization", format!("Bearer {}", access_token))
            .header("User-Agent", "PinePods/1.0")
            .header("Accept", "application/json")
            .send()
            .await
        {
            if emails_response.status().is_success() {
                if let Ok(emails) = emails_response.json::<Vec<serde_json::Value>>().await {
                    // Find primary email
                    for email_obj in &emails {
                        if email_obj.get("primary").and_then(|v| v.as_bool()).unwrap_or(false) && 
                           email_obj.get("verified").and_then(|v| v.as_bool()).unwrap_or(false) {
                            email = email_obj.get("email").and_then(|v| v.as_str()).map(|s| s.to_string());
                            break;
                        }
                    }
                    // If no primary, take first verified
                    if email.is_none() {
                        for email_obj in &emails {
                            if email_obj.get("verified").and_then(|v| v.as_bool()).unwrap_or(false) {
                                email = email_obj.get("email").and_then(|v| v.as_str()).map(|s| s.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    let email = match email {
        Some(e) => e,
        None => return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=email_required", frontend_base))),
    };

    // Role verification - EXACT match to Python
    if let (Some(roles_claim), Some(user_role)) = (roles_claim.as_ref().filter(|s| !s.is_empty()), user_role.as_ref().filter(|s| !s.is_empty())) {
        if let Some(roles) = userinfo_response.get(roles_claim).and_then(|v| v.as_array()) {
            let has_user_role = roles.iter().any(|r| r.as_str() == Some(user_role));
            let has_admin_role = admin_role.as_ref().map_or(false, |admin_role| {
                roles.iter().any(|r| r.as_str() == Some(admin_role))
            });
            
            if !has_user_role && !has_admin_role {
                return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=no_access", frontend_base)));
            }
        } else {
            return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=no_access&details=invalid_roles", frontend_base)));
        }
    }

    // Check if user exists - EXACT match to Python
    let existing_user = state.db_pool.get_user_by_email(&email).await?;
    
    let name_field = name_claim
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("name");
    let fullname = userinfo_response.get(name_field)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Username claim validation - EXACT match to Python
    if let Some(username_claim) = username_claim.as_ref().filter(|s| !s.is_empty()) {
        if !userinfo_response.get(username_claim).is_some() {
            return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=user_creation_failed&details=username_claim_missing", frontend_base)));
        }
    }

    let username_field = username_claim
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("preferred_username");
    let username = userinfo_response.get(username_field)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let user_id = if let Some((user_id, _email, current_username, _fullname, _is_admin)) = existing_user {
        // Existing user - EXACT match to Python
        let api_key = match state.db_pool.get_user_api_key(user_id).await? {
            Some(key) => key,
            None => state.db_pool.create_api_key(user_id).await?,
        };

        // Update user info - EXACT match to Python
        state.db_pool.set_fullname(user_id, &fullname).await?;

        // Update username if changed - EXACT match to Python
        if let (Some(username_claim), Some(new_username)) = (username_claim.as_ref().filter(|s| !s.is_empty()), username.as_ref()) {
            if Some(new_username) != current_username.as_ref() {
                if !state.db_pool.check_usernames(new_username).await? {
                    state.db_pool.set_username(user_id, new_username).await?;
                }
            }
        }

        // Update admin role - EXACT match to Python
        if let (Some(roles_claim), Some(admin_role)) = (roles_claim.as_ref().filter(|s| !s.is_empty()), admin_role.as_ref().filter(|s| !s.is_empty())) {
            if let Some(roles) = userinfo_response.get(roles_claim).and_then(|v| v.as_array()) {
                let is_admin = roles.iter().any(|r| r.as_str() == Some(admin_role));
                state.db_pool.set_isadmin(user_id, is_admin).await?;
            }
        }

        let redirect_url = create_oidc_redirect_url(&frontend_base, &format!("api_key={}", api_key));
        tracing::info!("OIDC Debug - Final redirect URL (existing user): {}", redirect_url);
        return Ok(axum::response::Redirect::to(&redirect_url));
    } else {
        // Create new user - EXACT match to Python
        let mut final_username = username.unwrap_or_else(|| email.split('@').next().unwrap_or(&email).to_lowercase());
        
        // Username conflict resolution - EXACT match to Python
        if state.db_pool.check_usernames(&final_username).await? {
            let base_username = final_username.clone();
            let mut counter = 1;
            const MAX_ATTEMPTS: i32 = 10;
            
            while counter <= MAX_ATTEMPTS {
                final_username = format!("{}_{}", base_username, counter);
                if !state.db_pool.check_usernames(&final_username).await? {
                    break;
                }
                counter += 1;
                if counter > MAX_ATTEMPTS {
                    return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=username_conflict", frontend_base)));
                }
            }
        }

        // Create user - EXACT match to Python
        match state.db_pool.create_oidc_user(&email, &fullname, &final_username).await {
            Ok(user_id) => {
                let api_key = state.db_pool.create_api_key(user_id).await?;
                
                // Set admin role for new user - EXACT match to Python
                if let (Some(roles_claim), Some(admin_role)) = (roles_claim.as_ref().filter(|s| !s.is_empty()), admin_role.as_ref().filter(|s| !s.is_empty())) {
                    if let Some(roles) = userinfo_response.get(roles_claim).and_then(|v| v.as_array()) {
                        let is_admin = roles.iter().any(|r| r.as_str() == Some(admin_role));
                        state.db_pool.set_isadmin(user_id, is_admin).await?;
                    }
                }
                
                user_id
            }
            Err(_) => return Ok(axum::response::Redirect::to(&format!("{}/oauth/callback?error=user_creation_failed", frontend_base))),
        }
    };

    let api_key = match state.db_pool.get_user_api_key(user_id).await? {
        Some(key) => key,
        None => state.db_pool.create_api_key(user_id).await?,
    };

    // Success - handle both web and mobile redirects
    let redirect_url = create_oidc_redirect_url(&frontend_base, &format!("api_key={}", api_key));
    tracing::info!("OIDC Debug - Final redirect URL: {}", redirect_url);
    Ok(axum::response::Redirect::to(&redirect_url))
}

// Update user timezone
pub async fn update_timezone(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<UpdateTimezoneRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user
    if data.user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to update timezone for other users"));
    }
    
    let success = state.db_pool.update_user_timezone(data.user_id, &data.timezone).await?;
    
    if success {
        Ok(Json(json!({
            "success": true,
            "message": "Timezone updated successfully"
        })))
    } else {
        Err(AppError::not_found("User not found"))
    }
}

// Update user date format
pub async fn update_date_format(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<UpdateDateFormatRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user
    if data.user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to update date format for other users"));
    }
    
    let success = state.db_pool.update_user_date_format(data.user_id, &data.date_format).await?;
    
    if success {
        Ok(Json(json!({
            "success": true,
            "message": "Date format updated successfully"
        })))
    } else {
        Err(AppError::not_found("User not found"))
    }
}

// Update user time format
pub async fn update_time_format(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<UpdateTimeFormatRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user
    if data.user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to update time format for other users"));
    }
    
    let success = state.db_pool.update_user_time_format(data.user_id, data.hour_pref).await?;
    
    if success {
        Ok(Json(json!({
            "success": true,
            "message": "Time format updated successfully"
        })))
    } else {
        Err(AppError::not_found("User not found"))
    }
}

// Get user auto complete seconds
pub async fn get_auto_complete_seconds(
    headers: HeaderMap,
    Path(user_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user
    if user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to view auto complete seconds for other users"));
    }
    
    let auto_complete_seconds = state.db_pool.get_user_auto_complete_seconds(user_id).await?;
    
    Ok(Json(json!({
        "auto_complete_seconds": auto_complete_seconds
    })))
}

// Update user auto complete seconds
pub async fn update_auto_complete_seconds(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(data): Json<UpdateAutoCompleteSecondsRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    
    // Verify API key
    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::forbidden("Your API key is either invalid or does not have correct permission"));
    }
    
    // Get user ID from API key for authorization check
    let user_id_from_api_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Allow the action if the API key belongs to the user
    if data.user_id != user_id_from_api_key {
        return Err(AppError::forbidden("You are not authorized to update auto complete seconds for other users"));
    }
    
    let success = state.db_pool.set_user_auto_complete_seconds(data.user_id, data.seconds).await?;
    
    if success {
        Ok(Json(json!({
            "success": true,
            "message": "Auto complete seconds updated successfully"
        })))
    } else {
        Err(AppError::not_found("User not found"))
    }
}

// Construct base URL from request headers (matches Python request.base_url)
fn construct_base_url_from_request(headers: &HeaderMap) -> Result<String, AppError> {
    // Get Host header (required)
    let host = headers
        .get("host")
        .ok_or_else(|| AppError::bad_request("Missing Host header"))?
        .to_str()
        .map_err(|_| AppError::bad_request("Invalid Host header"))?;

    // Check for X-Forwarded-Proto header to determine scheme
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");

    let mut base_url = format!("{}://{}", scheme, host);
    
    tracing::info!("OIDC Debug - Headers: Host={}, X-Forwarded-Proto={:?}, X-Forwarded-Host={:?}, X-Forwarded-Port={:?}, constructed base_url={}", 
        host, 
        headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()),
        headers.get("x-forwarded-host").and_then(|v| v.to_str().ok()),
        headers.get("x-forwarded-port").and_then(|v| v.to_str().ok()),
        base_url
    );

    // Force HTTPS if running in production (not localhost)
    if !base_url.starts_with("http://localhost") && base_url.starts_with("http:") {
        base_url = format!("https:{}", &base_url[5..]);
    }

    Ok(base_url)
}