use anyhow::Error;
use gloo_net::http::Request;
use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct UserStats {
    pub UserCreated: String,
    pub PodcastsPlayed: i32,
    pub TimeListened: i32,
    pub PodcastsAdded: i32,
    pub EpisodesSaved: i32,
    pub EpisodesDownloaded: i32,
    pub GpodderUrl: String,
    pub Pod_Sync_Type: String,
}
pub async fn call_get_stats(
    server_name: String,
    api_key: Option<String>,
    user_id: &i32,
) -> Result<UserStats, anyhow::Error> {
    let url = format!("{}/api/data/get_stats?user_id={}", server_name, user_id);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to get stats: {}",
            response.status_text()
        )));
    }

    // First, capture the response text for diagnostic purposes
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to get response text".to_string());

    // Try to deserialize the response text
    match serde_json::from_str::<UserStats>(&response_text) {
        Ok(response_body) => Ok(response_body),
        Err(_e) => Err(anyhow::Error::msg("Failed to deserialize response")),
    }
}
