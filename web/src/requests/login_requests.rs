use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Serialize)]
pub struct LoginServerRequest {
    server_name: String,
    username: String,
    password: String,
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
    let login_request = LoginServerRequest { server_name, username, password };
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
