use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use base64::encode;
use crate::components::context;
use yew::prelude::*;
use web_sys::console;
// Add imports for your context modules
use crate::components::context::{AppState};

#[derive(Serialize)]
pub struct LoginRequest {
    username: String,
    password: String,
    // api_key: String
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct LoginServerRequest {
    pub(crate) server_name: String,
    pub(crate) username: String,
    pub(crate) password: String,
    pub(crate) api_key: Option<String>
}

#[derive(Deserialize)]
pub struct LoginResponse {
    status: String,
    retrieved_key: String,
}

#[derive(Deserialize)]
struct PinepodsCheckResponse {
    pinepods_instance: Option<bool>,
}

pub async fn verify_pinepods_instance(server_name: &str) -> Result<PinepodsCheckResponse, anyhow::Error> {
    let url = format!("{}/api/pinepods_check", server_name);
    let response = Request::get(&url).send().await?;

    if response.ok() {
        let check_data: PinepodsCheckResponse = response.json().await?;
        if check_data.pinepods_instance.unwrap_or(false) {
            Ok((check_data))
        } else {
            Err(anyhow::Error::msg("Pinepods instance not found"))
        }
    } else {
        Err(anyhow::Error::msg("Failed to verify Pinepods instance"))
    }
}

#[derive(Deserialize, Debug)]
pub struct KeyVerification {
    // Add fields according to your API's JSON response
    pub status: String
}

pub async fn call_verify_key(server_name: &str, api_key: &str) -> Result<crate::requests::login_requests::KeyVerification, anyhow::Error> {
    let url = format!("{}/api/data/verify_key", server_name);

    let response = Request::get(&url)
        .header("Api-Key", api_key)
        .send()
        .await?;

    if response.ok() {
        let key_verify: crate::requests::login_requests::KeyVerification = response.json().await?;
        Ok(key_verify)
    } else {
        Err(anyhow::Error::msg("Failed to get user data"))
    }
}
#[derive(Deserialize, Debug)]
pub struct GetUserResponse {
    // Add fields according to your API's JSON response
    pub status: String,
    pub user_id: Option<String>,
    // ... other fields ...
}

pub async fn call_get_user(server_name: &str, api_key: &str) -> Result<GetUserResponse, anyhow::Error> {
    let url = format!("{}/api/data/get_user", server_name);

    let response = Request::get(&url)
        .header("Api-Key", api_key)
        .send()
        .await?;

    if response.ok() {
        let user_data: GetUserResponse = response.json().await?;
        Ok(user_data)
    } else {
        Err(anyhow::Error::msg("Failed to get user data"))
    }
}

#[derive(Deserialize, Debug)]
pub struct GetUserIdResponse {
    // Add fields according to your API's JSON response
    pub status: String,
    pub retrieved_id: Option<i32>,
}

pub async fn call_get_user_id(server_name: &str, api_key: &str) -> Result<GetUserIdResponse, anyhow::Error> {
    let url = format!("{}/api/data/get_user", server_name);

    let response = Request::get(&url)
        .header("Api-Key", api_key)
        .send()
        .await?;

    if response.ok() {
        let user_id_data: GetUserIdResponse = response.json().await?;
        Ok(user_id_data)
    } else {
        Err(anyhow::Error::msg("Failed to get user ID"))
    }
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct GetUserDetails {
    // Add fields according to your API's JSON response
    pub UserID: i32,
    pub Fullname: Option<String>,
    pub Username: Option<String>,
    pub Email: Option<String>,
    pub Hashed_PW: Option<String>,
    pub Salt: Option<String>
}

pub async fn call_get_user_details(server_name: &str, api_key: &str, user_id: &i32) -> Result<crate::requests::login_requests::GetUserDetails, anyhow::Error> {
    let url = format!("{}/api/data/user_details_id/{}", server_name, user_id);

    let response = Request::get(&url)
        .header("Api-Key", api_key)
        .send()
        .await?;

    if response.ok() {
        let user_data: crate::requests::login_requests::GetUserDetails = response.json().await?;
        Ok(user_data)
    } else {
        Err(anyhow::Error::msg("Failed to get user information"))
    }
}

pub async fn login(username: String, password: String) -> Result<LoginResponse, anyhow::Error> {
    let login_request = LoginRequest { username, password };
    let response = Request::post("/api/login")
        .json(&login_request)?
        .send()
        .await?;

    if response.ok() {
        let login_response = response.json::<LoginResponse>().await?;
        Ok(login_response)
    } else {
        // Handle HTTP error
        Err(anyhow::Error::msg("Login failed"))
    }
}

pub async fn login_new_server(server_name: String, username: String, password: String) -> Result<(GetUserDetails, LoginServerRequest), anyhow::Error> {
    let credentials = encode(format!("{}:{}", username, password));
    let auth_header = format!("Basic {}", credentials);
    let url = format!("{}/api/data/get_key", server_name);

    // Step 1: Verify Server
    match verify_pinepods_instance(&server_name).await {
        Ok(check_data) => {
            if !check_data.pinepods_instance.unwrap_or(false) {
                return Err(anyhow::Error::msg("Pinepods instance not found at specified server"));
            }
            // Step 2: Get API key
            let response = Request::get(&url)
                .header("Authorization", &auth_header)
                .send()
                .await?;

            if !response.ok() {
                return Err(anyhow::Error::msg("Failed to authenticate user. Incorrect credentials?"));
            }

            let api_key = response.json::<LoginResponse>().await?.retrieved_key;

            // Step 2: Verify the API key
            let verify_response = call_verify_key(&server_name, &api_key).await?;
            if verify_response.status != "success" {
                return Err(anyhow::Error::msg("API key verification failed"));
            }

            // Step 3: Get user ID
            let user_id_response = call_get_user_id(&server_name, &api_key).await?;
            if user_id_response.status != "success" {
                return Err(anyhow::Error::msg("Failed to get user ID"));
            }

            let login_request = LoginServerRequest {
                server_name: server_name.clone(),
                username: username.clone(),
                password: password.clone(),
                api_key: Some(api_key.clone()), // or None, depending on the context
            };


            // Step 4: Get user details
            let user_details = call_get_user_details(&server_name, &api_key, &user_id_response.retrieved_id.unwrap()).await?;
            if user_details.Username.is_none() {
                return Err(anyhow::Error::msg("Failed to get user details"));
            }


            Ok((user_details, login_request))
        },
        Err(e) => {
            // Directly propagate the error from verify_pinepods_instance
            return Err(e);
        }
    }
}

#[derive(Deserialize)]
struct ServerErrorResponse {
    error_message: String,
    // Include other fields if your server's error response contains more data
}