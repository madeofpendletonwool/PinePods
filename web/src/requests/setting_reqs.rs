use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::console;
use crate::requests::pod_req::PodcastValues;
use std::collections::HashMap;
use wasm_bindgen::JsValue;

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct GetThemeResponse {
    theme: String,
}
pub async fn call_get_theme(server_name: String, api_key: String, user_id: &i32) -> Result<String, anyhow::Error> {
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
        console::log_1(&format!("Error getting theme: {}", response.status_text()).into());
        Err(Error::msg(format!("Error getting theme. Is the server reachable? Server Response: {}", response.status_text())))
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

pub async fn call_set_theme(server_name: &Option<String>, api_key: &Option<String>, set_theme: &SetThemeRequest) -> Result<bool, Error> {
    let server = server_name.clone().unwrap();
    let url = format!("{}/api/data/user/set_theme", server);
    let api_key_ref = api_key.as_deref().ok_or_else(|| Error::msg("API key is missing"))?;

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
        console::log_1(&format!("Error updating theme: {}", response.status_text()).into());
        Err(Error::msg(format!("Error updating theme: {}", response.status_text())))
    }
}

// Admin Only API Calls

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct UserInfoResponse {
    user_info: HashMap<String, String>,
}
#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct SettingsUser {
    pub UserID: i32,
    pub Fullname: String,
    pub Username: String,
    pub Email: String,
    pub IsAdmin: i32,
}

pub async fn call_get_user_info(server_name: String, api_key: String) -> Result<Vec<SettingsUser>, anyhow::Error> {
    let url = format!("{}/api/data/get_user_info", server_name);
    let api_key_ref = api_key.as_str();

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if response.ok() {
        let response_text = response.text().await?;
        console::log_1(&format!("Response body: {}", response_text).into());
        console::log_1(&"Button clicked1".into());
        let users: Vec<SettingsUser> = serde_json::from_str(&response_text)?;
        Ok(users)
    } else {
        console::log_1(&format!("Error getting user info: {}", response.status_text()).into());
        Err(Error::msg(format!("Error getting user info. Is the server reachable? Server Response: {}", response.status_text())))
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct AddSettingsUserRequest {
    pub(crate) fullname: String,
    pub(crate) new_username: String,
    pub(crate) email: String,
    pub(crate) hash_pw: String,
    pub(crate) salt: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct AddUserResponse {
    detail: String,
}


pub async fn call_add_user(server_name: String, add_user: &Option<AddSettingsUserRequest>) -> Result<bool, Error> {
    let server = server_name.clone();
    let url = format!("{}/api/data/add_user", server);
    let add_user_req = add_user.as_ref().unwrap();

    // Serialize `add_user` into JSON
    let json_body = serde_json::to_string(&add_user_req)?;

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<AddUserResponse>().await?;
        Ok(response_body.detail == "Success")
    } else {
        console::log_1(&format!("Error adding user: {}", response.status_text()).into());
        Err(Error::msg(format!("Error adding user: {}", response.status_text())))
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct DetailResponse {
    detail: String,
}

pub async fn call_set_fullname(server_name: String, api_key: String, user_id: i32, new_name: String) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/set_fullname/{}?new_name={}", server_name, user_id, new_name);
    let response = Request::put(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response.json::<DetailResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error setting fullname: {}", response.status_text())))
    }
}

pub async fn call_set_password(server_name: String, api_key: String, user_id: i32, salt: String, hash_pw: String) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/set_password/{}", server_name, user_id);
    let body = serde_json::json!({ "salt": salt, "hash_pw": hash_pw });

    let response = Request::put(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(&body.to_string())?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response.json::<DetailResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error setting password: {}", response.status_text())))
    }
}

pub async fn call_set_email(server_name: String, api_key: String, user_id: i32, new_email: String) -> Result<DetailResponse, Error> {
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
        response.json::<DetailResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error setting email: {}", response.status_text())))
    }
}

pub async fn call_set_username(server_name: String, api_key: String, user_id: i32, new_username: String) -> Result<DetailResponse, Error> {
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
        response.json::<DetailResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error setting username: {}", response.status_text())))
    }
}

pub async fn call_set_isadmin(server_name: String, api_key: String, user_id: i32, isadmin: bool) -> Result<DetailResponse, Error> {
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
        response.json::<DetailResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error setting admin status: {}", response.status_text())))
    }
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct EditSettingsUserRequest {
    pub(crate) fullname: String,
    pub(crate) new_username: String,
    pub(crate) email: String,
    pub(crate) hash_pw: String,
    pub(crate) salt: String,
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

pub async fn call_enable_disable_guest(server_name: String, api_key: String) -> Result<SuccessResponse, Error> {
    let url = format!("{}/api/data/enable_disable_guest", server_name);

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response.json::<SuccessResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error enabling/disabling guest access: {}", response.status_text())))
    }
}

pub async fn call_enable_disable_downloads(server_name: String, api_key: String) -> Result<SuccessResponse, Error> {
    let url = format!("{}/api/data/enable_disable_downloads", server_name);

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response.json::<SuccessResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error enabling/disabling downloads: {}", response.status_text())))
    }
}

pub async fn call_enable_disable_self_service(server_name: String, api_key: String) -> Result<SuccessResponse, Error> {
    let url = format!("{}/api/data/enable_disable_self_service", server_name);

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response.json::<SuccessResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error enabling/disabling self service: {}", response.status_text())))
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
        response.json::<bool>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error fetching guest status: {}", response.status_text())))
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
        response.json::<bool>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error fetching download status: {}", response.status_text())))
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
        let status_response: SelfServiceStatusResponse = response.json().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))?;
        Ok(status_response.status)
    } else {
        Err(Error::msg(format!("Error fetching self service status: {}", response.status_text())))
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
    let body = EmailSettingsRequest {
        email_settings,
    };

    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(&body)?)?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response.json::<DetailResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error saving email settings: {}", response.status_text())))
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
    email_status: String,
}


use serde_json::to_string;

pub async fn call_send_test_email(
    server_name: String,
    api_key: String,
    email_settings: TestEmailSettings,
) -> Result<EmailSendResponse, Error> {
    let url = format!("{}/api/data/send_test_email", server_name);
    let body = serde_json::to_string(&email_settings)?;

    // Serialize and log the email settings
    match to_string(&body) {
        Ok(serialized_body) => {
            console::log_1(&format!("Sending test email with settings: {}", serialized_body).into());
        },
        Err(e) => {
            console::log_1(&format!("Error serializing email settings: {}", e).into());
        }
    }


    let response = Request::post(&url)
        .header("Api-Key", &api_key)
        .header("Content-Type", "application/json")
        .body(&body)?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response.json::<EmailSendResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error sending email: {}", response.status_text())))
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SendEmailSettings {
    pub(crate) message: String,
}

pub async fn call_send_email(
    server_name: String,
    api_key: String,
    email_settings: SendEmailSettings,
) -> Result<DetailResponse, Error> {
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
        response.json::<DetailResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error sending email: {}", response.status_text())))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Default)]
pub struct EmailSettingsResponse {
    pub(crate) EmailSettingsID: i32,
    pub(crate) Server_Name: String,
    pub(crate) Server_Port: i32,
    pub(crate) From_Email: String,
    pub(crate) Send_Mode: String,
    pub(crate) Encryption: String,
    pub(crate) Auth_Required: i32,
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
        let response_text = response.text().await.map_err(|e| Error::msg(format!("Error getting response text: {}", e)))?;
        println!("Response text: {}", response_text);
        serde_json::from_str::<EmailSettingsResponse>(&response_text).map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error retrieving email settings: {}", response.status_text())))
    }
}
