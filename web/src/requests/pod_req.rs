use std::collections::HashMap;
use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use web_sys::console;

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct Episode {
    pub PodcastName: String,
    pub EpisodeTitle: String,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
    pub EpisodeArtwork: String,
    pub EpisodeURL: String,
    pub EpisodeDuration: i32,
    pub ListenDuration: Option<String>,
    pub EpisodeID: i32,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct RecentEps {
    pub episodes: Option<Vec<Episode>>,
}

pub async fn call_get_recent_eps(server_name: &String, api_key: &Option<String>, user_id: &i32) -> Result<Vec<Episode>, anyhow::Error> {
    let url = format!("{}/api/data/return_episodes/{}", server_name, user_id);

    console::log_1(&format!("URL: {}", url).into());

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to fetch episodes: {}", response.status_text())));
    }

    console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
    
    // First, capture the response text for diagnostic purposes
    let response_text = response.text().await.unwrap_or_else(|_| "Failed to get response text".to_string());
    console::log_1(&format!("HTTP Response Body: {}", response_text).into());

    // Try to deserialize the response text
    match serde_json::from_str::<RecentEps>(&response_text) {
        Ok(response_body) => {
            console::log_1(&format!("Deserialized Response Body: {:?}", response_body).into());
            Ok(response_body.episodes.unwrap_or_else(Vec::new))
        }
        Err(e) => {
            console::log_1(&format!("Deserialization Error: {:?}", e).into());
            Err(anyhow::Error::msg("Failed to deserialize response"))
        }
    }
}



#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PodcastValues {
    pub pod_title: String,
    pub pod_artwork: String,
    pub pod_author: String,
    pub categories: HashMap<String, String>,
    pub pod_description: String,
    pub pod_episode_count: i32,
    pub pod_feed_url: String,
    pub pod_website: String,
    pub user_id: i32
}

#[derive(serde::Deserialize)]
struct PodcastStatusResponse {
    success: bool,
    // Include other fields if your response contains more data
}

pub async fn call_add_podcast(server_name: &str, api_key: &Option<String>, user_id: i32, added_podcast: &PodcastValues) -> Result<bool, Error> {
    let url = format!("{}/api/data/add_podcast/", server_name);
    let api_key_ref = api_key.as_deref().ok_or_else(|| Error::msg("API key is missing"))?;

    // Serialize `added_podcast` into JSON
    let json_body = serde_json::to_string(added_podcast)?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<PodcastStatusResponse>().await?;
        Ok(response_body.success)
    } else {
        console::log_1(&format!("Error adding podcast: {}", response.status_text()).into());
        Err(Error::msg(format!("Error adding podcast: {}", response.status_text())))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemovePodcastValues {
    pub podcast_id: i32,
    pub user_id: i32
}

pub async fn call_remove_podcasts(server_name: &String, api_key: &Option<String>, remove_podcast: &RemovePodcastValues) -> Result<bool, Error> {
    let url = format!("{}/api/data/remove_podcast_id", server_name);

    console::log_1(&format!("URL: {}", url).into());

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    // Serialize `added_podcast` into JSON
    let json_body = serde_json::to_string(remove_podcast)?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<PodcastStatusResponse>().await?;
        Ok(response_body.success)
    } else {
        console::log_1(&format!("Error removing podcast: {}", response.status_text()).into());
        Err(Error::msg(format!("Error adding podcast: {}", response.status_text())))
    }
}


#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PodcastResponse {
    pub pods: Option<Vec<Podcast>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Podcast {
    pub PodcastID: i32,
    pub PodcastName: String,
    pub ArtworkURL: String,
    pub Description: String,
    pub EpisodeCount: i32,
    pub WebsiteURL: String,
    pub FeedURL: String,
    pub Author: String,
    pub Categories: String, // Assuming categories are key-value pairs
}

pub async fn call_get_podcasts(server_name: &String, api_key: &Option<String>, user_id: &i32) -> Result<Vec<Podcast>, anyhow::Error> {
    let url = format!("{}/api/data/return_pods/{}", server_name, user_id);

    console::log_1(&format!("URL: {}", url).into());

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to fetch podcasts: {}", response.status_text())));
    }

    console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
    
    // First, capture the response text for diagnostic purposes
    let response_text = response.text().await.unwrap_or_else(|_| "Failed to get response text".to_string());
    console::log_1(&format!("HTTP Response Body: {}", response_text).into());

    // Try to deserialize the response text
    match serde_json::from_str::<PodcastResponse>(&response_text) {
        Ok(response_body) => {
            console::log_1(&format!("Deserialized Response Body: {:?}", response_body).into());
            Ok(response_body.pods.unwrap_or_else(Vec::new))
        }
        Err(e) => {
            console::log_1(&format!("Deserialization Error: {:?}", e).into());
            Err(anyhow::Error::msg("Failed to deserialize response"))
        }
    }
}
