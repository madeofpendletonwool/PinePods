use std::collections::HashMap;
use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use serde_json::json;
use web_sys::console;

#[derive(Deserialize, Debug)]
pub struct Episode {
    pub PodcastName: String,
    pub EpisodeTitle: String,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
}

#[derive(Deserialize, Debug)]
pub struct RecentEps {
    pub episodes: Option<Vec<Episode>>,
}

pub async fn call_get_recent_eps(server_name: &String, api_key: &Option<String>, user_id: &i32) -> Result<Vec<Episode>, anyhow::Error> {
    let url = format!("{}/api/data/return_pods/{}", server_name, user_id);

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

    let response_body = response.json::<RecentEps>().await?;
    Ok(response_body.episodes.unwrap_or_else(Vec::new))
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
struct AddPodcastResponse {
    success: bool,
    // Include other fields if your response contains more data
}

pub async fn call_add_podcast(server_name: &String, api_key: &Option<String>, user_id: &i32, added_podcast: &PodcastValues) -> Result<bool, Error> {
    let url = format!("{}/api/data/add_podcast/", server_name);
    let api_key_ref = api_key.as_deref().ok_or_else(|| Error::msg("API key is missing"))?;

    let data = json!({
        "podcast_values": added_podcast,
        "user_id": user_id
    });

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .json(&data)?
        .send()
        .await?;

    if response.ok() {
        let response_body = response.json::<AddPodcastResponse>().await?;
        Ok(response_body.success)
    } else {
        console::log_1(&format!("Error adding podcast: {}", response.status_text()).into());
        Err(Error::msg(format!("Error adding podcast: {}", response.status_text())))
    }
}