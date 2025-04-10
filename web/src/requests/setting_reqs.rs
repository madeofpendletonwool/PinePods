use crate::requests::pod_req::Podcast;
use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::JsValue;
use web_sys::FormData;

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct GetThemeResponse {
    theme: String,
}
pub async fn call_get_theme(
    server_name: String,
    api_key: String,
    user_id: &i32,
) -> Result<String, anyhow::Error> {
    let url = format!("{}/api/data/get_theme/{}", server_name, user_id);
    let api_key_ref = api_key.as_str();

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<GetThemeResponse>().await?;
        Ok(response_body.theme)
    } else {
        Err(Error::msg(format!(
            "Error getting theme. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct SetThemeRequest {
    pub(crate) user_id: i32,
    pub(crate) new_theme: String,
}
#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct SetThemeResponse {
    message: String,
}

pub async fn call_set_theme(
    server_name: &Option<String>,
    api_key: &Option<String>,
    set_theme: &SetThemeRequest,
) -> Result<bool, Error> {
    let server = server_name.clone().unwrap();
    let url = format!("{}/api/data/user/set_theme", server);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

    // Serialize `added_podcast` into JSON
    let json_body = serde_json::to_string(set_theme)?;

    let response = Request::put(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<SetThemeResponse>().await?;
        Ok(response_body.message == "Success")
    } else {
        Err(Error::msg(format!(
            "Error updating theme: {}",
            response.status_text()
        )))
    }
}

// Admin Only API Calls

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct UserInfoResponse {
    user_info: HashMap<String, String>,
}
#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
pub struct SettingsUser {
    pub userid: i32,
    pub fullname: String,
    pub username: String,
    pub email: String,
    pub isadmin: i32,
}

pub async fn call_get_user_info(
    server_name: String,
    api_key: String,
) -> Result<Vec<SettingsUser>, anyhow::Error> {
    let url = format!("{}/api/data/get_user_info", server_name);
    let api_key_ref = api_key.as_str();

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if response.ok() {
        let response_text = response.text().await?;
        let users: Vec<SettingsUser> = serde_json::from_str(&response_text)?;
        Ok(users)
    } else {
        Err(Error::msg(format!(
            "Error getting user info. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct MyUserInfo {
    pub userid: i32,
    pub fullname: String,
    pub username: String,
    pub email: String,
    pub isadmin: i32,
}

pub async fn call_get_my_user_info(
    server_name: &String,
    api_key: String,
    user_id: i32,
) -> Result<MyUserInfo, anyhow::Error> {
    let url = format!("{}/api/data/my_user_info/{}", server_name, user_id);

    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if response.ok() {
        let response_text = response.text().await?;
        let user_info: MyUserInfo = serde_json::from_str(&response_text)?;
        Ok(user_info)
    } else {
        Err(Error::msg(format!(
            "Error getting user info. Status: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct AddSettingsUserRequest {
    pub(crate) fullname: String,
    pub(crate) username: String,
    pub(crate) email: String,
    pub(crate) hash_pw: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct AddUserResponse {
    detail: String,
    user_id: Option<i32>,
}

#[derive(Deserialize, Debug)]
struct ErrorResponse {
    detail: String,
}

pub async fn call_add_user(
    server_name: String,
    api_key: String,
    add_user: &AddSettingsUserRequest,
) -> Result<bool, Error> {
    let server = server_name.clone();
    let url = format!("{}/api/data/add_user", server);
    let json_body = serde_json::to_string(&add_user)?;

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<AddUserResponse>().await?;
        Ok(response_body.detail == "Success")
    } else {
        // Try to get a detailed error message
        let error_text = response.text().await?;

        // Try to parse as JSON error response
        match serde_json::from_str::<ErrorResponse>(&error_text) {
            Ok(error_response) => Err(Error::msg(error_response.detail)),
            Err(_) => {
                // If we can't parse the JSON, return the raw error text
                Err(Error::msg(error_text))
            }
        }
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct DetailResponse {
    detail: String,
}

pub async fn call_set_fullname(
    server_name: String,
    api_key: String,
    user_id: i32,
    new_name: String,
) -> Result<DetailResponse, Error> {
    let url = format!(
        "{}/api/data/set_fullname/{}?new_name={}",
        server_name, user_id, new_name
    );
    let response = Request::put(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<DetailResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error setting fullname: {}",
            response.status_text()
        )))
    }
}

pub async fn call_set_password(
    server_name: String,
    api_key: String,
    user_id: i32,
    hash_pw: String,
) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/set_password/{}", server_name, user_id);
    let body = serde_json::json!({ "hash_pw": hash_pw });

    let response = Request::put(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<DetailResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error setting password: {}",
            response.status_text()
        )))
    }
}

#[derive(Debug, Deserialize)]
pub struct DeleteUserResponse {
    pub status: String,
}

pub async fn call_delete_user(
    server_name: String,
    api_key: String,
    user_id: i32,
) -> Result<DeleteUserResponse, Error> {
    let url = format!("{}/api/data/user/delete/{}", server_name, user_id);

    let response = Request::delete(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<DeleteUserResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        Err(Error::msg(format!(
            "Error deleting user - Status: {}, Body: {}",
            status, error_text
        )))
    }
}

pub async fn call_set_email(
    server_name: String,
    api_key: String,
    user_id: i32,
    new_email: String,
) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/user/set_email", server_name);
    let body = serde_json::json!({ "user_id": user_id, "new_email": new_email });

    let response = Request::put(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(&body.to_string())?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<DetailResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error setting email: {}",
            response.status_text()
        )))
    }
}

pub async fn call_set_username(
    server_name: String,
    api_key: String,
    user_id: i32,
    new_username: String,
) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/user/set_username", server_name);
    let body = serde_json::json!({ "user_id": user_id, "new_username": new_username });

    let response = Request::put(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(&body.to_string())?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<DetailResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error setting username: {}",
            response.status_text()
        )))
    }
}
#[allow(dead_code)]
pub async fn call_set_isadmin(
    server_name: String,
    api_key: String,
    user_id: i32,
    isadmin: bool,
) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/user/set_isadmin", server_name);
    let body = serde_json::json!({ "user_id": user_id, "isadmin": isadmin });

    let response = Request::put(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(&body.to_string())?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<DetailResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error setting admin status: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, PartialEq)]
pub struct FinalAdminResponse {
    pub(crate) final_admin: bool,
}

#[allow(dead_code)]
pub async fn call_check_admin(
    server_name: String,
    api_key: String,
    user_id: i32,
) -> Result<FinalAdminResponse, Error> {
    let url = format!("{}/api/data/user/final_admin/{}", server_name, user_id);

    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<FinalAdminResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error getting admin status: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct EditSettingsUserRequest {
    pub(crate) fullname: String,
    pub(crate) new_username: String,
    pub(crate) email: String,
    pub(crate) hash_pw: String,
    pub(crate) admin_status: bool,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct EditUserResponse {
    detail: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct SuccessResponse {
    success: bool,
}

pub async fn call_enable_disable_guest(
    server_name: String,
    api_key: String,
) -> Result<SuccessResponse, Error> {
    let url = format!("{}/api/data/enable_disable_guest", server_name);

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<SuccessResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error enabling/disabling guest access: {}",
            response.status_text()
        )))
    }
}

pub async fn call_enable_disable_downloads(
    server_name: String,
    api_key: String,
) -> Result<SuccessResponse, Error> {
    let url = format!("{}/api/data/enable_disable_downloads", server_name);

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<SuccessResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error enabling/disabling downloads: {}",
            response.status_text()
        )))
    }
}

pub async fn call_enable_disable_self_service(
    server_name: String,
    api_key: String,
) -> Result<SuccessResponse, Error> {
    let url = format!("{}/api/data/enable_disable_self_service", server_name);

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<SuccessResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error enabling/disabling self service: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct GuestStatusResponse {
    pub guest_status: bool,
}

pub async fn call_guest_status(server_name: String, api_key: String) -> Result<bool, Error> {
    let url = format!("{}/api/data/guest_status", server_name);

    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<bool>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error fetching guest status: {}",
            response.status_text()
        )))
    }
}

// setting_reqs.rs

pub async fn call_rss_feed_status(server_name: String, api_key: String) -> Result<bool, Error> {
    let url = format!("{}/api/data/rss_feed_status", server_name);
    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<bool>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error getting RSS feed status: {}",
            response.status_text()
        )))
    }
}

pub async fn call_toggle_rss_feeds(
    server_name: String,
    api_key: String,
) -> Result<SuccessResponse, Error> {
    let url = format!("{}/api/data/toggle_rss_feeds", server_name);
    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<SuccessResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error toggling RSS feeds: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct DownloadStatusResponse {
    download_status: bool,
}

pub async fn call_download_status(server_name: String, api_key: String) -> Result<bool, Error> {
    let url = format!("{}/api/data/download_status", server_name);

    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<bool>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error fetching download status: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct SelfServiceStatusResponse {
    status: bool,
}

pub async fn call_self_service_status(server_name: String, api_key: String) -> Result<bool, Error> {
    let url = format!("{}/api/data/self_service_status", server_name);

    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        let status_response: SelfServiceStatusResponse = response
            .json()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))?;
        Ok(status_response.status)
    } else {
        Err(Error::msg(format!(
            "Error fetching self service status: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmailSettingsRequest {
    email_settings: EmailSettings,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EmailSettings {
    pub(crate) server_name: String,
    pub(crate) server_port: String,
    pub(crate) from_email: String,
    pub(crate) send_mode: String,
    pub(crate) encryption: String,
    pub(crate) auth_required: bool,
    pub(crate) email_username: String,
    pub(crate) email_password: String,
}

pub async fn call_save_email_settings(
    server_name: String,
    api_key: String,
    email_settings: EmailSettings,
) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/save_email_settings", server_name);
    let body = EmailSettingsRequest { email_settings };

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body)?)?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<DetailResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error saving email settings: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TestEmailSettings {
    pub(crate) server_name: String,
    pub(crate) server_port: String,
    pub(crate) from_email: String,
    pub(crate) send_mode: String,
    pub(crate) encryption: String,
    pub(crate) auth_required: bool,
    pub(crate) email_username: String,
    pub(crate) email_password: String,
    pub(crate) to_email: String,
    pub(crate) message: String,
}

#[derive(Deserialize, Debug)]
pub struct EmailSendResponse {
    #[allow(dead_code)]
    email_status: String,
}

pub async fn call_send_test_email(
    server_name: String,
    api_key: String,
    email_settings: TestEmailSettings,
) -> Result<EmailSendResponse, Error> {
    let url = format!("{}/api/data/send_test_email", server_name);
    let body = serde_json::to_string(&email_settings)?;

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(&body)?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<EmailSendResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error sending email: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SendEmailSettings {
    pub(crate) to_email: String,
    pub(crate) subject: String,
    pub(crate) message: String,
}

pub async fn call_send_email(
    server_name: String,
    api_key: String,
    email_settings: SendEmailSettings,
) -> Result<EmailSendResponse, Error> {
    let url = format!("{}/api/data/send_email", server_name);
    let body = email_settings;

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body)?)?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<EmailSendResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error sending email: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Default)]
#[allow(non_snake_case)]
#[serde(rename_all = "PascalCase")]
pub struct EmailSettingsResponse {
    pub(crate) Emailsettingsid: i32,
    pub(crate) ServerName: String,
    pub(crate) ServerPort: i32,
    pub(crate) FromEmail: String,
    pub(crate) SendMode: String,
    pub(crate) Encryption: String,
    pub(crate) AuthRequired: i32,
    pub(crate) Username: String,
    pub(crate) Password: String,
}

pub async fn call_get_email_settings(
    server_name: String,
    api_key: String,
) -> Result<EmailSettingsResponse, Error> {
    let url = format!("{}/api/data/get_email_settings", server_name);

    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        let response_text = response
            .text()
            .await
            .map_err(|e| Error::msg(format!("Error getting response text: {}", e)))?;
        serde_json::from_str::<EmailSettingsResponse>(&response_text)
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error retrieving email settings: {}",
            response.status_text()
        )))
    }
}

// User Setting Requests

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "lowercase")]
pub struct APIInfo {
    pub apikeyid: i32,
    pub userid: i32,
    pub username: String,
    pub lastfourdigits: String,
    pub created: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct APIInfoResponse {
    pub(crate) api_info: Vec<APIInfo>,
}

pub async fn call_get_api_info(
    server_name: String,
    user_id: i32,
    api_key: String,
) -> Result<APIInfoResponse, Error> {
    let url = format!("{}/api/data/get_api_info/{}", server_name, user_id);

    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<APIInfoResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error retrieving API info: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct CreateAPIKeyResponse {
    pub api_key: String,
}

pub async fn call_create_api_key(
    server_name: &str,
    user_id: i32,
    api_key: &str,
) -> Result<CreateAPIKeyResponse, anyhow::Error> {
    let url = format!("{}/api/data/create_api_key", server_name);
    let request_body = serde_json::json!({ "user_id": user_id });

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(serde_json::to_string(&request_body)?)?
        .send()
        .await
        .map_err(anyhow::Error::msg)?;

    if response.ok() {
        response
            .json::<CreateAPIKeyResponse>()
            .await
            .map_err(anyhow::Error::msg)
    } else {
        Err(anyhow::Error::msg("Error creating API key"))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct DeleteAPIKeyResponse {
    pub detail: String,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct DeleteAPIRequest {
    pub(crate) api_id: String,
    pub(crate) user_id: String,
}

pub async fn call_delete_api_key(
    server_name: &str,
    request_body: DeleteAPIRequest,
    api_key: &str,
) -> Result<DeleteAPIKeyResponse, anyhow::Error> {
    let url = format!("{}/api/data/delete_api_key", server_name);
    let body = request_body;

    let response = Request::delete(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(serde_json::to_string(&body)?)?
        .send()
        .await
        .map_err(anyhow::Error::msg)?;

    if response.ok() {
        response
            .json::<DeleteAPIKeyResponse>()
            .await
            .map_err(anyhow::Error::msg)
    } else {
        // If the response is not ok(), read the response body to extract the error message
        let error_message = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        Err(anyhow::Error::msg(format!(
            "Error deleting API key: {}",
            error_message
        )))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupUserRequest {
    pub user_id: i32,
}

pub async fn call_backup_user(
    server_name: &str,
    user_id: i32,
    api_key: &str,
) -> Result<String, anyhow::Error> {
    // Assuming the OPML content is returned as a plain string
    let url = format!("{}/api/data/backup_user", server_name);
    let request_body = BackupUserRequest { user_id };

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(serde_json::to_string(&request_body)?)?
        .send()
        .await
        .map_err(anyhow::Error::msg)?;

    if response.ok() {
        response.text().await.map_err(anyhow::Error::msg)
    } else {
        Err(anyhow::Error::msg("Error backing up user data"))
    }
}

pub async fn call_backup_server(
    server_name: &str,
    database_pass: &str,
    api_key: &str,
) -> Result<String, anyhow::Error> {
    let url = format!("{}/api/data/backup_server", server_name);
    let request_body = serde_json::json!({
        "database_pass": database_pass
    });

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(serde_json::to_string(&request_body)?)?
        .send()
        .await
        .map_err(anyhow::Error::msg)?;

    if response.ok() {
        response.text().await.map_err(anyhow::Error::msg)
    } else {
        Err(anyhow::Error::msg(format!(
            "Error backing up server data: {}",
            response.status_text()
        )))
    }
}

pub async fn call_restore_server(
    server_name: &str,
    form_data: FormData,
    api_key: &str,
) -> Result<String, Error> {
    let url = format!("{}/api/data/restore_server", server_name);

    let response = Request::post(&url)
        .header("Api-Key", api_key)
        // Using form_data directly here instead of JSON
        .body(form_data)?
        .send()
        .await
        .map_err(Error::msg)?;

    if response.ok() {
        response.text().await.map_err(Error::msg)
    } else {
        Err(Error::msg(format!(
            "Error restoring server data: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug)]
pub struct GenerateMFAResponse {
    pub(crate) secret: String,
    pub(crate) qr_code_svg: String,
}

// Then adjust the function to return this struct:
pub async fn call_generate_mfa_secret(
    server_name: String,
    api_key: String,
    user_id: i32,
) -> Result<GenerateMFAResponse, anyhow::Error> {
    let url = format!("{}/api/data/generate_mfa_secret/{}", server_name, user_id);
    let api_key_ref = &api_key;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<GenerateMFAResponse>().await?;
        Ok(response_body)
    } else {
        Err(anyhow::Error::msg(format!(
            "Error generating MFA secret. Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize)]
struct VerifyTempMFABody {
    user_id: i32,
    mfa_code: String,
}

#[derive(Deserialize, Debug)]
pub struct VerifyTempMFAResponse {
    pub verified: bool,
}

pub async fn call_verify_temp_mfa(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    mfa_code: String,
) -> Result<VerifyTempMFAResponse, Error> {
    let url = format!("{}/api/data/verify_temp_mfa", server_name);
    let body = VerifyTempMFABody { user_id, mfa_code };
    let request_body = serde_json::to_string(&body).map_err(Error::msg)?;

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(request_body)?
        .send()
        .await
        .map_err(Error::msg)?;

    if response.ok() {
        response
            .json::<VerifyTempMFAResponse>()
            .await
            .map_err(Error::msg)
    } else {
        let status_text = response.status_text();
        let error_text = response.text().await.unwrap_or_default();
        Err(Error::msg(format!(
            "Error verifying temp MFA: {} - {}",
            status_text, error_text
        )))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct GetMFAResponse {
    mfa_enabled: bool,
}
pub async fn call_mfa_settings(
    server_name: String,
    api_key: String,
    user_id: i32,
) -> Result<bool, anyhow::Error> {
    let url = format!("{}/api/data/check_mfa_enabled/{}", server_name, user_id);
    let api_key_ref = api_key.as_str();

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<GetMFAResponse>().await?;
        Ok(response_body.mfa_enabled)
    } else {
        Err(Error::msg(format!(
            "Error getting MFA status. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Debug)]
pub struct SaveMFASecretRequest {
    pub(crate) user_id: i32,
    pub(crate) mfa_secret: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct SaveMFASecretResponse {
    pub(crate) status: String,
}
#[allow(dead_code)]
pub async fn call_save_mfa_secret(
    server_name: &String,
    api_key: &String,
    user_id: i32,
    mfa_secret: String,
) -> Result<SaveMFASecretResponse, anyhow::Error> {
    let url = format!("{}/api/data/save_mfa_secret", server_name);
    let api_key_ref = api_key.as_str();
    let body = SaveMFASecretRequest {
        user_id,
        mfa_secret,
    };
    let json_body = serde_json::to_string(&body)?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<SaveMFASecretResponse>().await?;
        Ok(response_body)
    } else {
        Err(Error::msg(format!(
            "Error saving MFA secret. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize)]
pub struct DeleteMFARequest {
    pub user_id: i32,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct DeleteMFAResponse {
    pub deleted: bool,
}

#[allow(dead_code)]
pub async fn call_disable_mfa(
    server_name: &String,
    api_key: &String,
    user_id: i32,
) -> Result<DeleteMFAResponse, anyhow::Error> {
    let url = format!("{}/api/data/delete_mfa", server_name);
    let api_key_ref = api_key.as_str();
    let body = DeleteMFARequest { user_id };
    let json_body = serde_json::to_string(&body)?;

    let response = Request::delete(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<DeleteMFAResponse>().await?;
        Ok(response_body)
    } else {
        Err(Error::msg(format!(
            "Error disabling MFA. Is the server reachable? Server Response: {}",
            response.status()
        )))
    }
}

// #[derive(Deserialize, Debug, PartialEq, Clone)]
// pub struct NextcloudInitiateResponse {
//     pub(crate) token: String,
//     pub(crate) poll_endpoint: String,
//     pub(crate) nextcloud_url: String,
// }

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct Poll {
    pub(crate) token: String,
    pub(crate) endpoint: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct NextcloudInitiateResponse {
    pub(crate) poll: Poll,
    pub(crate) login: String,
}

#[derive(Serialize, Debug)]
pub struct LoginInitiateRequest {
    user_id: i32,
    nextcloud_url: String,
}

pub async fn initiate_nextcloud_login(
    nextcloud_url: &str,
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<NextcloudInitiateResponse, Error> {
    // Construct the URL with query parameters
    let url = format!("{}/api/data/initiate_nextcloud_login", server_name);
    let request_body = LoginInitiateRequest {
        user_id,
        nextcloud_url: nextcloud_url.to_string(),
    };
    let json_body = serde_json::to_string(&request_body)
        .map_err(|e| Error::msg(format!("Failed to serialize request body: {}", e)))?;

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(json_body)?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    // Get the response body as text
    let response_body = response.text().await?;

    // Parse the JSON response into the NextcloudLoginResponse struct
    if response.ok() {
        serde_json::from_str::<NextcloudInitiateResponse>(&response_body)
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error initiating Nextcloud login: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize)]
pub struct NextcloudAuthRequest {
    pub(crate) user_id: i32,
    pub(crate) token: String,
    pub(crate) poll_endpoint: String,
    pub(crate) nextcloud_url: String,
}

#[derive(Deserialize, Debug)]
pub struct NextcloudAuthResponse {
    // Define additional fields as needed
}

pub async fn call_add_nextcloud_server(
    server_name: &String,
    api_key: &String,
    auth_request: NextcloudAuthRequest,
) -> Result<NextcloudAuthResponse, anyhow::Error> {
    let url = format!("{}/api/data/add_nextcloud_server", server_name);
    let api_key_ref = api_key.as_str();
    let request_body = serde_json::to_string(&auth_request)?;

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key_ref)
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<NextcloudAuthResponse>().await?;
        Ok(response_body)
    } else {
        Err(Error::msg(format!(
            "Error saving Nextcloud Server Info. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize)]
pub struct GpodderCheckRequest {
    pub gpodder_url: String,
    pub gpodder_username: String,
    pub gpodder_password: String,
}

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct GpodderCheckResponse {
    pub status: String,
    pub message: String,
}

pub async fn call_verify_gpodder_auth(
    server_name: &String,
    auth_request: GpodderCheckRequest,
) -> Result<GpodderCheckResponse, Error> {
    let url = format!("{}/api/data/verify_gpodder_auth", server_name);
    let request_body = serde_json::to_string(&auth_request)?;

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<GpodderCheckResponse>().await?;
        Ok(response_body)
    } else {
        Err(Error::msg(format!(
            "Error verifying gPodder auth. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize)]
pub struct GpodderAuthRequest {
    pub(crate) user_id: i32,
    pub(crate) gpodder_url: String,
    pub(crate) gpodder_username: String,
    pub(crate) gpodder_password: String,
}

#[derive(Deserialize, Debug)]
pub struct GpodderAuthResponse {
    // Define additional fields as needed
}

pub async fn call_add_gpodder_server(
    server_name: &String,
    api_key: &String,
    auth_request: GpodderAuthRequest,
) -> Result<NextcloudAuthResponse, anyhow::Error> {
    let url = format!("{}/api/data/add_gpodder_server", server_name);
    let api_key_ref = api_key.as_str();
    let request_body = serde_json::to_string(&auth_request)?;

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key_ref)
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<NextcloudAuthResponse>().await?;
        Ok(response_body)
    } else {
        Err(Error::msg(format!(
            "Error saving Nextcloud Server Info. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

// Get user's default GPodder device - matches the EXACT route in your backend
pub async fn call_get_default_gpodder_device(
    server_name: &str,
    api_key: &str,
) -> Result<GpodderDevice, gloo::net::Error> {
    // Use the exact same route as defined in your backend
    let url = format!("{}/api/gpodder/default_device", server_name);

    // Log the request URL for debugging
    web_sys::console::log_1(&format!("Fetching default device from: {}", url).into());

    let response = gloo::net::http::Request::get(&url)
        .header("Api-Key", api_key)
        .send()
        .await?
        .json::<GpodderDevice>()
        .await?;

    Ok(response)
}

// Set a device as the default - with support for remote devices
pub async fn call_set_default_gpodder_device(
    server_name: &str,
    api_key: &str,
    device_id: i32,
    device_name: Option<String>,
    is_remote: bool,
) -> Result<ApiResponse<()>, gloo::net::Error> {
    let mut url = format!("{}/api/gpodder/set_default/{}", server_name, device_id);

    // Add query parameters for remote devices
    if device_id < 0 || is_remote {
        if let Some(name) = &device_name {
            url = format!(
                "{}?device_name={}&is_remote=true",
                url,
                js_sys::encode_uri_component(name)
            );
        }
    }

    // Log the request URL for debugging
    web_sys::console::log_1(&format!("Setting default device at: {}", url).into());

    let response = gloo::net::http::Request::post(&url)
        .header("Api-Key", api_key)
        .send()
        .await?
        .json::<ApiResponse<()>>()
        .await?;

    Ok(response)
}

// API structures
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GpodderDevice {
    pub id: i32,
    pub name: String,
    pub r#type: String, // Using r# prefix because "type" is a reserved keyword
    pub caption: Option<String>,
    pub last_sync: Option<String>,
    pub is_active: bool,
    pub is_remote: Option<bool>,
    pub is_default: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateDeviceRequest {
    pub user_id: i32,
    pub device_name: String,
    pub device_type: String,
    pub device_caption: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SyncRequest {
    pub user_id: i32,
    pub device_id: Option<i32>,
    pub device_name: Option<String>,
    pub is_remote: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

// Get user's GPodder devices
pub async fn call_get_gpodder_devices(
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<Vec<GpodderDevice>, gloo::net::Error> {
    let url = format!("{}/api/gpodder/devices/{}", server_name, user_id);

    let response = gloo::net::http::Request::get(&url)
        .header("Api-Key", api_key)
        .send()
        .await?
        .json::<Vec<GpodderDevice>>()
        .await?;

    Ok(response)
}

// Create a new GPodder device
pub async fn call_create_gpodder_device(
    server_name: &str,
    api_key: &str,
    request: CreateDeviceRequest,
) -> Result<GpodderDevice, gloo::net::Error> {
    let url = format!("{}/api/gpodder/devices", server_name);

    let response = gloo::net::http::Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&request)?
        .send()
        .await?
        .json::<GpodderDevice>()
        .await?;

    Ok(response)
}

// Force full sync with GPodder
pub async fn call_force_full_sync(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    device_id: Option<i32>,
    device_name: Option<String>,
    is_remote: bool,
) -> Result<ApiResponse<()>, gloo::net::Error> {
    let url = format!("{}/api/gpodder/sync/force", server_name);

    let request = SyncRequest {
        user_id,
        device_id,
        device_name,
        is_remote,
    };

    let response = gloo::net::http::Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&request)?
        .send()
        .await?
        .json::<ApiResponse<()>>()
        .await?;

    Ok(response)
}

// Sync from GPodder
pub async fn call_sync_with_gpodder(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    device_id: Option<i32>,
    device_name: Option<String>,
    is_remote: bool,
) -> Result<ApiResponse<()>, gloo::net::Error> {
    let url = format!("{}/api/gpodder/sync", server_name);

    // Create the request with all necessary fields
    let request = SyncRequest {
        user_id,
        device_id,
        device_name,
        is_remote,
    };

    // Log the request for debugging
    web_sys::console::log_1(&format!("Sending sync request: {:?}", &request).into());

    let response = gloo::net::http::Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&request)?
        .send()
        .await?
        .json::<ApiResponse<()>>()
        .await?;

    Ok(response)
}

// Test GPodder connection
pub async fn call_test_gpodder_connection(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    gpodder_url: &str,
    gpodder_username: &str,
    gpodder_password: &str,
) -> Result<ApiResponse<serde_json::Value>, gloo::net::Error> {
    let url = format!(
        "{}/api/gpodder/test-connection?user_id={}&gpodder_url={}&gpodder_username={}&gpodder_password={}",
        server_name,
        user_id,
        urlencoding::encode(gpodder_url),
        urlencoding::encode(gpodder_username),
        urlencoding::encode(gpodder_password)
    );

    let response = gloo::net::http::Request::get(&url)
        .header("Api-Key", api_key)
        .send()
        .await?
        .json::<ApiResponse<serde_json::Value>>()
        .await?;

    Ok(response)
}

#[derive(Deserialize, Debug)]
pub struct NextcloudCheckResponse {
    pub(crate) data: bool,
    // Define additional fields as needed
}

pub async fn call_check_nextcloud_server(
    server_name: &String,
    api_key: &String,
    user_id: i32,
) -> Result<NextcloudCheckResponse, anyhow::Error> {
    let url = format!(
        "{}/api/data/check_gpodder_settings/{}",
        server_name, user_id
    );
    let api_key_ref = api_key.as_str();

    let response = Request::get(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<NextcloudCheckResponse>().await?;
        Ok(response_body)
    } else {
        Err(Error::msg(format!(
            "Error pulling Nextcloud Server Info. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug, Serialize)]
pub struct NextcloudGetResponse {
    pub data: GpodderData,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct GpodderData {
    pub gpodderurl: String,
    pub gpoddertoken: String,
}
pub async fn call_get_nextcloud_server(
    server_name: &String,
    api_key: &String,
    user_id: i32,
) -> Result<String, anyhow::Error> {
    let url = format!("{}/api/data/get_gpodder_settings/{}", server_name, user_id);
    let api_key_ref = api_key.as_str();

    let response = Request::get(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    let status_text = response.status_text();
    let response_text = response.text().await.unwrap_or_default();

    if response.ok() {
        match serde_json::from_str::<NextcloudGetResponse>(&response_text) {
            Ok(response_body) => {
                if !response_body.data.gpodderurl.trim().is_empty() {
                    Ok(response_body.data.gpodderurl.clone())
                } else {
                    Ok(String::from("Not currently syncing with Nextcloud server"))
                }
            }
            Err(e) => {
                web_sys::console::error_1(&JsValue::from_str(&format!(
                    "Error parsing JSON: {:?}",
                    e
                )));
                Err(anyhow::Error::new(e))
            }
        }
    } else {
        Err(anyhow::Error::msg(format!(
            "Failed to get Nextcloud settings: {}",
            status_text
        )))
    }
}

#[derive(Serialize)]
pub struct RemoveSyncRequest {
    pub user_id: i32,
}

#[derive(Deserialize)]
struct RemoveSyncResponse {
    success: bool,
    message: String,
}

// Function to remove podcast sync settings (Nextcloud or gPodder)
pub async fn call_remove_podcast_sync(
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<bool, Error> {
    let url = format!("{}/api/data/remove_podcast_sync", server_name);

    let request_data = RemoveSyncRequest { user_id };

    let response = Request::delete(&url)
        .header("Api-Key", api_key)
        .json(&request_data)?
        .send()
        .await?;

    if response.ok() {
        let response_data: RemoveSyncResponse = response.json().await?;
        Ok(response_data.success)
    } else {
        Err(Error::msg(format!(
            "Failed to remove podcast sync settings: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Debug)]
pub struct AdminCheckResponse {
    pub is_admin: bool,
}

pub async fn call_user_admin_check(
    server_name: &String,
    api_key: &String,
    user_id: i32,
) -> Result<AdminCheckResponse, anyhow::Error> {
    let url = format!("{}/api/data/user_admin_check/{}", server_name, user_id);
    let api_key_ref = api_key.as_str();

    let response = Request::get(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<AdminCheckResponse>().await?;
        Ok(response_body)
    } else {
        Err(anyhow::Error::msg(format!(
            "Error checking admin status. Is the server reachable? Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize)]
struct CustomFeedRequest {
    feed_url: String,
    user_id: i32,
    username: Option<String>,
    password: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct AddCustomPodcastResponse {
    data: Podcast,
}

pub async fn call_add_custom_feed(
    server_name: &str,
    feed_url: &str,
    user_id: &i32,
    api_key: &str,
    username: Option<String>,
    password: Option<String>,
) -> Result<Podcast, Error> {
    let url = format!("{}/api/data/add_custom_podcast", server_name);
    let request_body = CustomFeedRequest {
        feed_url: feed_url.to_string(),
        user_id: *user_id,
        username,
        password,
    };

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(serde_json::to_string(&request_body)?)?
        .send()
        .await
        .map_err(Error::msg)?;

    if response.ok() {
        let response_text = response.text().await.map_err(Error::msg)?;

        let add_custom_podcast_response: AddCustomPodcastResponse =
            serde_json::from_str(&response_text).map_err(Error::msg)?;
        Ok(add_custom_podcast_response.data)
    } else {
        Err(Error::msg(format!(
            "Error adding feed: {}",
            response.status_text()
        )))
    }
}

pub async fn call_podcast_opml_import(
    server_name: &str,
    api_key: &Option<String>,
    user_id: i32,
    podcasts: Vec<String>,
) -> Result<(), Error> {
    let url = format!("{}/api/data/import_opml", server_name);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

    let request_body = serde_json::json!({
        "podcasts": podcasts,
        "user_id": user_id
    });

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&request_body)?)?
        .send()
        .await?;

    if response.ok() {
        Ok(())
    } else {
        Err(Error::msg(format!(
            "Error importing OPML: {}",
            response.status_text()
        )))
    }
}

#[derive(Debug, Deserialize)]
pub struct ImportProgressResponse {
    current: i32,
    total: i32,
    current_podcast: String,
}

pub async fn fetch_import_progress(
    server_name: &str,
    api_key: &Option<String>,
    user_id: i32,
) -> Result<(i32, i32, String), Error> {
    let url = format!("{}/api/data/import_progress/{}", server_name, user_id);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;
    if response.ok() {
        let progress_response: ImportProgressResponse = response.json().await?;
        Ok((
            progress_response.current,
            progress_response.total,
            progress_response.current_podcast,
        ))
    } else {
        Err(Error::msg(format!(
            "Error fetching import progress: {}",
            response.status_text()
        )))
    }
}

// Notification Calls

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NotificationSettings {
    pub platform: String,
    pub enabled: bool,
    pub ntfy_topic: Option<String>,
    pub ntfy_server_url: Option<String>,
    pub gotify_url: Option<String>,
    pub gotify_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NotificationSettingsResponse {
    pub settings: Vec<NotificationSettings>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotificationResponse {
    pub detail: String,
}

// Fetch user's notification settings
// In notification_reqs.rs
pub async fn call_get_notification_settings(
    server_name: String,
    api_key: String,
    user_id: i32,
) -> Result<NotificationSettingsResponse, Error> {
    let url = format!(
        "{}/api/data/user/notification_settings?user_id={}",
        server_name, user_id
    ); // Added user_id as query param
    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<NotificationSettingsResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        let error_text = response.text().await.unwrap_or_default();
        Err(Error::msg(format!(
            "Error fetching notification settings: {}",
            error_text
        )))
    }
}

// Update notification settings
pub async fn call_update_notification_settings(
    server_name: String,
    api_key: String,
    user_id: i32,
    settings: NotificationSettings,
) -> Result<NotificationResponse, Error> {
    let url = format!("{}/api/data/user/notification_settings", server_name);
    let body = serde_json::json!({
        "user_id": user_id,
        "platform": settings.platform,
        "enabled": settings.enabled,
        "ntfy_topic": settings.ntfy_topic,
        "ntfy_server_url": settings.ntfy_server_url,
        "gotify_url": settings.gotify_url,
        "gotify_token": settings.gotify_token,
    });

    let response = Request::put(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| Error::msg(format!("Failed to create request: {}", e)))?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<NotificationResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        let error_text = response.text().await.unwrap_or_default();
        Err(Error::msg(format!(
            "Error updating notification settings: {}",
            error_text
        )))
    }
}

pub async fn call_test_notification(
    server_name: String,
    api_key: String,
    user_id: i32,
    platform: String,
) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/user/test_notification", server_name);
    let body = serde_json::json!({
        "user_id": user_id,
        "platform": platform,
    });

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .map_err(|e| Error::msg(format!("Failed to create request: {}", e)))?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response
            .json::<DetailResponse>()
            .await
            .map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        let error_text = response.text().await.unwrap_or_default();
        Err(Error::msg(format!(
            "Error sending test notification: {}",
            error_text
        )))
    }
}

// OIDC Settings

#[derive(Debug, Serialize)]
pub struct AddOIDCProviderRequest {
    pub provider_name: String,
    pub client_id: String,
    pub client_secret: String,
    pub authorization_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub button_text: String,
    pub scope: Option<String>,
    pub button_color: Option<String>,
    pub button_text_color: Option<String>,
    pub icon_svg: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OIDCProvider {
    pub provider_id: i32,
    pub provider_name: String,
    pub client_id: String,
    pub authorization_url: String,
    pub token_url: String,
    pub user_info_url: String,
    pub button_text: String,
    pub scope: String,
    pub button_color: String,
    pub button_text_color: String,
    pub icon_svg: Option<String>,
    pub enabled: bool,
}

pub async fn call_add_oidc_provider(
    server_name: String,
    api_key: String,
    provider: AddOIDCProviderRequest,
) -> Result<i32, Error> {
    let url = format!("{}/api/data/add_oidc_provider", server_name);
    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .json(&provider)?
        .send()
        .await?;

    if response.ok() {
        let result: serde_json::Value = response.json().await?;
        let provider_id = result["provider_id"]
            .as_i64()
            .ok_or_else(|| Error::msg("No provider ID in response"))?;
        Ok(provider_id as i32)
    } else {
        Err(Error::msg(format!(
            "Failed to add OIDC provider: {}",
            response.status_text()
        )))
    }
}

pub async fn call_remove_oidc_provider(
    server_name: String,
    api_key: String,
    provider_id: i32,
) -> Result<(), Error> {
    let url = format!("{}/api/data/remove_oidc_provider", server_name);
    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .json(&provider_id)? // Send the provider_id directly
        .send()
        .await?;
    if response.ok() {
        Ok(())
    } else {
        Err(Error::msg(format!(
            "Failed to remove OIDC provider: {}",
            response.status_text()
        )))
    }
}

// First, create a struct to match the actual JSON structure
#[derive(Deserialize)]
struct OIDCProvidersResponse {
    providers: Vec<OIDCProvider>,
}

pub async fn call_list_oidc_providers(
    server_name: String,
    api_key: String,
) -> Result<Vec<OIDCProvider>, Error> {
    let url = format!("{}/api/data/list_oidc_providers", server_name);
    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .send()
        .await?;
    if response.ok() {
        let response_data: OIDCProvidersResponse = response.json().await?;
        Ok(response_data.providers)
    } else {
        Err(Error::msg(format!(
            "Failed to list OIDC providers: {}",
            response.status_text()
        )))
    }
}

// First, create a struct to match the JSON response for getting startpage
#[derive(Deserialize)]
struct StartPageResponse {
    StartPage: String,
}

// Function to get the user's startpage
pub async fn call_get_startpage(
    server_name: &str,
    api_key: &str,
    user_id: &i32,
) -> Result<String, Error> {
    let url = format!("{}/api/data/startpage?user_id={}", server_name, user_id);
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;

    if response.ok() {
        let response_data: StartPageResponse = response.json().await?;
        Ok(response_data.StartPage)
    } else {
        Err(Error::msg(format!(
            "Failed to get startpage: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize)]
pub struct SetStartPageRequest {
    pub user_id: i32,
    pub new_startpage: String,
}

// Struct for the set startpage response
#[derive(Deserialize)]
#[allow(non_snake_case)]
struct SetStartPageResponse {
    success: bool,
    message: String,
}

// Function to set the user's startpage
pub async fn call_set_startpage(
    server_name: &str,
    api_key: &str,
    user_id: &i32,
    startpage: &str,
) -> Result<bool, Error> {
    // Build the URL with query parameters
    let url = format!(
        "{}/api/data/startpage?user_id={}&startpage={}",
        server_name, user_id, startpage
    );

    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .send()
        .await?;

    if response.ok() {
        let response_data: SetStartPageResponse = response.json().await?;
        Ok(response_data.success)
    } else {
        Err(Error::msg(format!(
            "Failed to set startpage: {}",
            response.status_text()
        )))
    }
}
