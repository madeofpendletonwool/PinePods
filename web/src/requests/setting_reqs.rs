use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::console;
use std::collections::HashMap;
use serde_json::to_string;

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
#[allow(non_snake_case)]
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
    pub(crate) username: String,
    pub(crate) email: String,
    pub(crate) hash_pw: String,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct AddUserResponse {
    detail: String,
}


pub async fn call_add_user(server_name: String, api_key: String, add_user: &AddSettingsUserRequest) -> Result<bool, Error> {
    let server = server_name.clone();
    let url = format!("{}/api/data/add_user", server);
    console::log_1(&format!("Request URL: {}", url.clone()).into());
    console::log_1(&format!("API Key: {}", api_key.clone()).into());


    // let add_user_req = add_user.as_ref().unwrap();

    // Serialize `add_user` into JSON
    let json_body = serde_json::to_string(&add_user)?;
    console::log_1(&format!("Request Body: {}", json_body.clone()).into());

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

pub async fn call_set_password(server_name: String, api_key: String, user_id: i32, hash_pw: String) -> Result<DetailResponse, Error> {
    let url = format!("{}/api/data/set_password/{}", server_name, user_id);
    let body = serde_json::json!({ "hash_pw": hash_pw });

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
#[allow(dead_code)]
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

#[derive(Deserialize, PartialEq)]
pub struct FinalAdminResponse {
    pub(crate) final_admin: bool,
}


#[allow(dead_code)]
pub async fn call_check_admin(server_name: String, api_key: String, user_id: i32) -> Result<FinalAdminResponse, Error> {
    let url = format!("{}/api/data/user/final_admin/{}", server_name, user_id);

    let response = Request::get(&url)
        .header("Api-Key", &api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network error: {}", e)))?;

    if response.ok() {
        response.json::<FinalAdminResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error getting admin status: {}", response.status_text())))
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
    pub(crate) to_email: String,
    pub(crate) subject : String,
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
        response.json::<EmailSendResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error sending email: {}", response.status_text())))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Default)]
#[allow(non_snake_case)]
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

// User Setting Requests

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct APIInfo {
    pub(crate) APIKeyID: i32,
    pub(crate) UserID: i32,
    pub(crate) Username: String,
    pub(crate) LastFourDigits: String,
    pub(crate) Created: String,
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
        response.json::<APIInfoResponse>().await.map_err(|e| Error::msg(format!("Error parsing JSON: {}", e)))
    } else {
        Err(Error::msg(format!("Error retrieving API info: {}", response.status_text())))
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
        response.json::<CreateAPIKeyResponse>().await.map_err(anyhow::Error::msg)
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
        response.json::<DeleteAPIKeyResponse>().await.map_err(anyhow::Error::msg)
    } else {
        Err(anyhow::Error::msg("Error creating API key"))
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
) -> Result<String, anyhow::Error> { // Assuming the OPML content is returned as a plain string
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
        Err(anyhow::Error::msg(format!("Error backing up server data: {}", response.status_text())))
    }
}

#[derive(Serialize, Deserialize)]
struct RestoreServerRequest {
    database_pass: String,
    server_restore_data: String,
}

pub async fn call_restore_server(
    server_name: &str,
    database_pass: &str,
    server_restore_data: &str,
    api_key: &str,
) -> Result<String, Error> {
    let url = format!("{}/api/data/restore_server", server_name);
    let request_body = RestoreServerRequest {
        database_pass: database_pass.to_string().clone(),
        server_restore_data: server_restore_data.to_string().clone(),
    };
    log::info!("db: {:?}", database_pass.to_string().clone());
    log::info!("restore: {:?}", server_restore_data.to_string().clone());

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(serde_json::to_string(&request_body)?)?
        .send()
        .await
        .map_err(Error::msg)?;

    if response.ok() {
        response.text().await.map_err(Error::msg)
    } else {
        Err(Error::msg(format!("Error restoring server data: {}", response.status_text())))
    }
}

#[derive(Deserialize, Debug)]
pub struct GenerateMFAResponse {
    pub(crate) secret: String,
    pub(crate) qr_code_svg: String,
}

// Then adjust the function to return this struct:
pub async fn call_generate_mfa_secret(server_name: String, api_key: String, user_id: i32) -> Result<GenerateMFAResponse, anyhow::Error> {
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
        console::log_1(&format!("Error generating MFA secret: {}", response.status_text()).into());
        Err(anyhow::Error::msg(format!("Error generating MFA secret. Server Response: {}", response.status_text())))
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
        response.json::<VerifyTempMFAResponse>().await.map_err(Error::msg)
    } else {
        let status_text = response.status_text();
        let error_text = response.text().await.unwrap_or_default();
        Err(Error::msg(format!("Error verifying temp MFA: {} - {}", status_text, error_text)))
    }
}


#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct GetMFAResponse {
    mfa_enabled: bool,
}
pub async fn call_mfa_settings(server_name: String, api_key: String, user_id: i32) -> Result<bool, anyhow::Error> {
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
        console::log_1(&format!("Error getting MFA status: {}", response.status_text()).into());
        Err(Error::msg(format!("Error getting MFA status. Is the server reachable? Server Response: {}", response.status_text())))
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
pub async fn call_save_mfa_secret(server_name: &String, api_key: &String, user_id: i32, mfa_secret: String) -> Result<SaveMFASecretResponse, anyhow::Error> {
    let url = format!("{}/api/data/save_mfa_secret", server_name);
    let api_key_ref = api_key.as_str();
    let body = SaveMFASecretRequest { user_id, mfa_secret };
    let json_body = serde_json::to_string(&body)?;

    // Log the request body for debugging
    console::log_1(&format!("Saving MFA Secret, Request Body: {:?}", &body).into());


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
        console::log_1(&format!("Error saving MFA secret: {}", response.status_text()).into());
        Err(Error::msg(format!("Error saving MFA secret. Is the server reachable? Server Response: {}", response.status_text())))
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
        console::log_1(&format!("Error saving Nextcloud Server Info: {}", response.status_text()).into());
        Err(Error::msg(format!("Error saving Nextcloud Server Info. Is the server reachable? Server Response: {}", response.status_text())))
    }
}

#[derive(Deserialize, Debug)]
pub struct NextcloudCheckResponse {
    pub(crate) data: bool,
    // Define additional fields as needed
}

pub async fn call_check_nextcloud_server(
    server_name: &String,
    api_key: &String,
    user_id: i32
) -> Result<NextcloudCheckResponse, anyhow::Error> {
    let url = format!("{}/api/data/check_gpodder_settings/{}", server_name, user_id);
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
        console::log_1(&format!("Error pulling Nextcloud Server Info: {}", response.status_text()).into());
        Err(Error::msg(format!("Error pulling Nextcloud Server Info. Is the server reachable? Server Response: {}", response.status_text())))
    }
}

#[derive(Deserialize, Debug)]
pub struct NextcloudGetResponse {
    pub(crate) data: Vec<String>, // Assuming the response always wraps the gpodder_url and token in a list under "data"
}


pub async fn call_get_nextcloud_server(
    server_name: &String,
    api_key: &String,
    user_id: i32
) -> Result<String, anyhow::Error> {
    let url = format!("{}/api/data/get_gpodder_settings/{}", server_name, user_id);
    let api_key_ref = api_key.as_str();

    let response = Request::get(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    console::log_1(&format!("Request URL: {}", url).into());

    // Here we are using the .ok() method from the Yew Fetch service response,
    // which is equivalent to checking if the status is in the 200-299 range
    if response.ok() {
        let response_body = response.json::<NextcloudGetResponse>().await?;
        if !response_body.data.is_empty() {
            console::log_1(&format!("Gpodder URL: {}", response_body.data[0]).into());
            // Assuming the first element is always the URL and the second is the token, you might want to check this
            let gpodder_url = response_body.data.get(0).cloned().unwrap_or_default(); 
            // Now you can use gpodder_url and gpodder_token as needed
            Ok(gpodder_url) // Adjust this line based on how you plan to use the response
        } else {
            console::log_1(&"Error: Received empty 'data' array in response".into());
            Err(anyhow::Error::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Received empty 'data' array in Nextcloud server response",
            )))
        }
    } else {
        let error_text = response.text().await.unwrap_or_default(); // Get error text if any
        console::log_1(&format!("Error pulling Nextcloud Server Info: {}, {}", response.status_text(), error_text).into());
        Err(anyhow::Error::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Error pulling Nextcloud Server Info. Is the server reachable? Server Response: {}, {}", response.status_text(), error_text),
        )))
    }
}