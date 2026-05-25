use anyhow::Error;
use gloo_net::http::Request;
use serde::Deserialize;
use serde::Serialize;

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

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct TopPodcastStat {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: String,
    pub total_seconds: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct CategoryStat {
    pub name: String,
    pub total_seconds: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct LongestEpisodeStat {
    pub episodeid: i32,
    pub episodetitle: String,
    pub episodeduration: i32,
    pub podcastname: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct ExtendedUserStats {
    pub top_podcasts: Vec<TopPodcastStat>,
    pub favorite_categories: Vec<CategoryStat>,
    pub completion_rate: i32,
    pub longest_episode: Option<LongestEpisodeStat>,
    pub listening_badge: String,
    pub total_downloaded_bytes: i64,
    pub total_downloaded_formatted: String,
    pub current_streak: i32,
    pub listening_by_dow: Vec<i64>,
}

#[allow(dead_code)]
pub async fn call_get_extended_stats(
    server_name: String,
    api_key: Option<String>,
    user_id: &i32,
) -> Result<ExtendedUserStats, anyhow::Error> {
    let url = format!("{}/api/data/get_extended_stats?user_id={}", server_name, user_id);
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
            "Failed to get extended stats: {}",
            response.status_text()
        )));
    }

    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to get response text".to_string());

    match serde_json::from_str::<ExtendedUserStats>(&response_text) {
        Ok(body) => Ok(body),
        Err(_) => Err(anyhow::Error::msg("Failed to deserialize extended stats")),
    }
}

#[allow(dead_code)]
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
