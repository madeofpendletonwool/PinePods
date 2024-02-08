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