use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use base64::encode;

#[derive(Serialize)]
pub struct LoginRequest {
    username: String,
    password: String,
    // api_key: String
}

#[derive(Serialize)]
pub struct LoginServerRequest {
    server_name: String,
    username: String,
    password: String,
    api_key: Option<String>
}

#[derive(Deserialize)]
pub struct LoginResponse {
    // Define fields based on your API's response
    token: String,
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

pub async fn login_new_server(server_name: String, username: String, password: String) -> Result<LoginResponse, anyhow::Error> {
    let credentials = encode(format!("{}:{}", username, password));
    let auth_header = format!("Basic {}", credentials);

    let url = format!("{}/api/data/get_key", server_name);

    // Make the GET request with Authorization header
    let response = Request::get(&url)
        .header("Authorization", &auth_header)
        .send()
        .await?;


    if response.ok() {
        let login_response = response.json::<LoginResponse>().await?;
        Ok(login_response)
    } else {
        // Attempt to read the error message from the response
        match response.json::<ServerErrorResponse>().await {
            Ok(error_response) => {
                // Use the server's error message
                Err(anyhow::Error::msg(error_response.error_message))
            }
            Err(_) => {
                // If parsing the error message failed, fall back to a generic error
                Err(anyhow::Error::msg("Login failed due to server error"))
            }
        }
    }
}

#[derive(Deserialize)]
struct ServerErrorResponse {
    error_message: String,
    // Include other fields if your server's error response contains more data
}