use std::collections::HashMap;
use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Deserializer, Serialize};
use web_sys::console;
use wasm_bindgen::JsValue;

fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
    where
        D: Deserializer<'de>,
{
    let value: i32 = Deserialize::deserialize(deserializer)?;
    Ok(value != 0)
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct Episode {
    pub PodcastName: String,
    pub EpisodeTitle: String,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
    pub EpisodeArtwork: String,
    pub EpisodeURL: String,
    pub EpisodeDuration: i32,
    pub ListenDuration: Option<i32>,
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
    pub pod_explicit: bool,
    pub user_id: i32
}

#[derive(serde::Deserialize)]
struct PodcastStatusResponse {
    success: bool,
    // Include other fields if your response contains more data
}

pub async fn call_add_podcast(server_name: &str, api_key: &Option<String>, _user_id: i32, added_podcast: &PodcastValues) -> Result<bool, Error> {
    let url = format!("{}/api/data/add_podcast", server_name);
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

    let response_text = response.text().await.unwrap_or_else(|_| "Failed to get response text".to_string());
    console::log_1(&format!("Response Text: {}", response_text).into());


    if response.ok() {
        match serde_json::from_str::<PodcastStatusResponse>(&response_text) {
            Ok(parsed_response) => Ok(parsed_response.success),
            Err(parse_error) => {
                console::log_1(&format!("Error parsing response: {:?}", parse_error).into());
                Err(anyhow::Error::msg("Failed to parse response"))
            }
        }
    } else {
        console::log_1(&format!("Error removing podcast: {}", response.status_text()).into());
        Err(anyhow::Error::msg(format!("Error removing podcast: {}", response.status_text())))
    }
}


#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PodcastResponse {
    pub pods: Option<Vec<Podcast>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
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
    #[serde(deserialize_with = "bool_from_int")]
    pub Explicit: bool,
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

// Queue calls

#[derive(Serialize, Deserialize, Debug)]
pub struct QueuePodcastRequest {
    pub episode_title: String,
    pub ep_url: String,
    pub user_id: i32,
}

pub async fn call_queue_episode(
    server_name: &String, 
    api_key: &Option<String>, 
    request_data: &QueuePodcastRequest
) -> Result<String, Error> {
    let url = format!("{}/api/data/queue_pod", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data).map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Read the success message from the response body as text
        let success_message = response.text().await.unwrap_or_else(|_| String::from("Episode queued successfully"));
        Ok(success_message)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response.text().await.unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!("Failed to queue episode: {} - {}", response.status_text(), error_text)))
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct QueuedEpisodesResponse {
    pub episodes: Vec<QueuedEpisode>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct QueuedEpisode {
    pub EpisodeTitle: String,
    pub PodcastName: String,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
    pub EpisodeArtwork: String,
    pub EpisodeURL: String,
    #[serde(default)]
    pub QueuePosition: Option<i32>,
    pub EpisodeDuration: i32,
    pub QueueDate: String,
    pub ListenDuration: Option<i32>,
    pub EpisodeID: i32,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DataResponse {
    pub data: Vec<QueuedEpisode>,
}

pub async fn call_get_queued_episodes(
    server_name: &str, 
    api_key: &Option<String>, 
    user_id: &i32
) -> Result<Vec<QueuedEpisode>, anyhow::Error> {
    // Append the user_id as a query parameter
    let url = format!("{}/api/data/get_queued_episodes?user_id={}", server_name, user_id);

    console::log_1(&format!("URL: {}", url).into());

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to fetch queued episodes: {}", response.status_text())));
    }

    console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
    let response_text = response.text().await?;

    console::log_1(&format!("HTTP Response Body: {}", &response_text).into());
    
    let response_data: DataResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.data)
}

// Save episode calls


#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct SavedEpisodesResponse {
    pub episodes: Vec<SavedEpisode>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct SavedEpisode {
    pub EpisodeTitle: String,
    pub PodcastName: String,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
    pub EpisodeArtwork: String,
    pub EpisodeURL: String,
    pub EpisodeDuration: i32,
    pub ListenDuration: Option<i32>,
    pub EpisodeID: i32,
    pub WebsiteURL: String,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct SavedDataResponse {
    pub saved_episodes: Vec<SavedEpisode>,
}

pub async fn call_get_saved_episodes(
    server_name: &str, 
    api_key: &Option<String>, 
    user_id: &i32
) -> Result<Vec<SavedEpisode>, anyhow::Error> {
    // Append the user_id as a query parameter
    let url = format!("{}/api/data/saved_episode_list/{}", server_name, user_id);

    console::log_1(&format!("URL: {}", url).into());

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to fetch saved episodes: {}", response.status_text())));
    }

    console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
    let response_text = response.text().await?;

    console::log_1(&format!("HTTP Response Body: {}", &response_text).into());
    
    let response_data: SavedDataResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.saved_episodes)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SavePodcastRequest {
    pub episode_id: i32,
    pub user_id: i32,
}

pub async fn call_save_episode(
    server_name: &String, 
    api_key: &Option<String>, 
    request_data: &SavePodcastRequest
) -> Result<String, Error> {
    let url = format!("{}/api/data/save_episode", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data).map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Read the success message from the response body as text
        let success_message = response.text().await.unwrap_or_else(|_| String::from("Episode saved successfully"));
        Ok(success_message)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response.text().await.unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!("Failed to save episode: {} - {}", response.status_text(), error_text)))
    }
}

// History calls

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct HistoryEpisodesResponse {
    pub episodes: Vec<HistoryEpisode>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct HistoryEpisode {
    pub EpisodeTitle: String,
    pub PodcastName: String,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
    pub EpisodeArtwork: String,
    pub EpisodeURL: String,
    pub EpisodeDuration: i32,
    pub ListenDuration: Option<i32>,
    pub EpisodeID: i32,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct HistoryDataResponse {
    pub data: Vec<HistoryEpisode>,
}

pub async fn call_get_user_history(
    server_name: &str, 
    api_key: &Option<String>, 
    user_id: &i32
) -> Result<Vec<HistoryEpisode>, anyhow::Error> {
    // Append the user_id as a query parameter
    let url = format!("{}/api/data/user_history/{}", server_name, user_id);

    console::log_1(&format!("URL: {}", url).into());

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to fetch history: {}", response.status_text())));
    }

    console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
    let response_text = response.text().await?;

    console::log_1(&format!("HTTP Response Body: {}", &response_text).into());
    
    let response_data: HistoryDataResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.data)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HistoryAddRequest {
    pub episode_id: i32,
    pub episode_pos: f32,
    pub user_id: i32,
}

pub async fn call_add_history(
    server_name: &String, 
    api_key: String, 
    request_data: &HistoryAddRequest
) -> Result<(), Error> {
    let url = format!("{}/api/data/record_podcast_history", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_str();

    let request_body = serde_json::to_string(request_data).map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to record history: {}", response.status_text())));
    }

    Ok(())
}

// Download calls

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct EpisodeDownloadResponse {
    pub episodes: Vec<EpisodeDownload>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct EpisodeDownload {
    pub EpisodeTitle: String,
    pub PodcastName: String,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
    pub EpisodeArtwork: String,
    pub EpisodeURL: String,
    pub EpisodeDuration: i32,
    pub ListenDuration: Option<i32>,
    pub EpisodeID: i32,
    pub DownloadedLocation: String,
    pub PodcastID: i32,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DownloadDataResponse {
    #[serde(rename = "downloaded_episodes")]
    pub episodes: Vec<EpisodeDownload>,
}

pub async fn call_get_episode_downloads(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32
) -> Result<Vec<EpisodeDownload>, anyhow::Error> {
    // Append the user_id as a query parameter
    let url = format!("{}/api/data/download_episode_list?user_id={}", server_name, user_id);


    console::log_1(&format!("URL: {}", url).into());

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to episode downloads: {}", response.status_text())));
    }

    console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
    let response_text = response.text().await?;

    console::log_1(&format!("HTTP Response Body: {}", &response_text).into());
    
    let response_data: DownloadDataResponse = serde_json::from_str(&response_text)?;
    console::log_1(&format!("Downloaded Episodes: {:?}", response_data.episodes.clone()).into());
    Ok(response_data.episodes)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadEpisodeRequest {
    pub episode_id: i32,
    pub user_id: i32,
}

pub async fn call_download_episode (
    server_name: &String, 
    api_key: &Option<String>, 
    request_data: &DownloadEpisodeRequest
) -> Result<String, Error> {
    let url = format!("{}/api/data/download_podcast", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data).map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Read the success message from the response body as text
        let success_message = response.text().await.unwrap_or_else(|_| String::from("Episode downloaded successfully"));
        Ok(success_message)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response.text().await.unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!("Failed to download episode: {} - {}", response.status_text(), error_text)))
    }
}

// Get Single Epsiode

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct EpisodeInfo {
    pub EpisodeTitle: String,
    pub PodcastName: String,
    pub PodcastID: i32,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
    pub EpisodeArtwork: String,
    pub EpisodeURL: String,
    pub EpisodeDuration: i32,
    pub ListenDuration: Option<i32>,
    pub EpisodeID: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EpisodeRequest {
    pub episode_id: i32,
    pub user_id: i32,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct EpisodeMetadataResponse {
    pub episode: EpisodeInfo,
}

pub async fn call_get_episode_metadata(
    server_name: &str, 
    api_key: Option<String>, 
    episode_request: &EpisodeRequest,
) -> Result<EpisodeInfo, anyhow::Error> {
    let url = format!("{}/api/data/get_episode_metadata", server_name);

    console::log_1(&format!("URL: {}", url).into());

    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;
    
    let request_body = serde_json::to_string(episode_request).map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to episode downloads: {}", response.status_text())));
    }

    console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
    let response_text = response.text().await?;

    console::log_1(&format!("HTTP Response Body: {}", &response_text).into());

    let response_data: EpisodeMetadataResponse = serde_json::from_str(&response_text)
        .map_err(|e| anyhow::Error::msg(format!("Deserialization Error: {}", e)))?;

    Ok(response_data.episode)
}

#[derive(Serialize)]
pub struct RecordListenDurationRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub listen_duration: f32, // Assuming float is appropriate here; adjust the type if necessary
}

#[derive(Deserialize, Debug)]
pub struct RecordListenDurationResponse {
    pub status: String, // Assuming a simple status response; adjust according to actual API response
}


pub async fn call_record_listen_duration(
    server_name: &str,
    api_key: &str,
    request_data: RecordListenDurationRequest,
) -> Result<RecordListenDurationResponse, Error> {
    let url = format!("{}/api/data/record_listen_duration", server_name);
    let request_body = serde_json::to_string(&request_data)
        .map_err(|e| Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .body(&request_body)
        .map_err(|e| Error::msg(format!("Request Building Error: {}", e)))?
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network Request Error: {}", e)))?;

    if response.ok() {
        response.json::<RecordListenDurationResponse>()
            .await
            .map_err(|e| Error::msg(format!("Response Parsing Error: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error recording listen duration. Server Response: {}",
            response.status_text()
        )))
    }
}
