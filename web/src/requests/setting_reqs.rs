use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::console;
use crate::requests::pod_req::PodcastValues;

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
    let url = format!("{}/api/data/user/set_theme/", server);
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