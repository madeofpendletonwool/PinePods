use anyhow::Error;
use gloo_net::http::Request;
use serde::Deserialize;
use web_sys::console;

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct UserStats {
    pub(crate) UserCreated: String,
    pub(crate) PodcastsPlayed: i32,
    pub(crate) TimeListened: i32,
    pub(crate) PodcastsAdded: i32,
    pub(crate) EpisodesSaved: i32,
    pub(crate) EpisodesDownloaded: i32,
}
pub async fn call_get_stats(server_name: String, api_key: Option<String>, user_id: &i32) -> Result<UserStats, anyhow::Error> {
    let url = format!("{}/api/data/get_stats?user_id={}", server_name, user_id);
    let api_key_ref = api_key.as_deref().ok_or_else(|| Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to get stats: {}", response.status_text())));
    }

    console::log_1(&format!("HTTP Response Status: {}", response.status()).into());

    // First, capture the response text for diagnostic purposes
    let response_text = response.text().await.unwrap_or_else(|_| "Failed to get response text".to_string());
    console::log_1(&format!("HTTP Response Body: {}", response_text).into());

    // Try to deserialize the response text
    match serde_json::from_str::<UserStats>(&response_text) {
        Ok(response_body) => {
            console::log_1(&format!("Deserialized Response Body: {:?}", response_body).into());
            Ok(response_body)
        }
        Err(e) => {
            console::log_1(&format!("Deserialization Error: {:?}", e).into());
            Err(anyhow::Error::msg("Failed to deserialize response"))
        }
    }
}

