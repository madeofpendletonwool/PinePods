use anyhow::{Context, Error};
use gloo_net::http::Request;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    struct BoolOrIntVisitor;

    impl<'de> serde::de::Visitor<'de> for BoolOrIntVisitor {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a boolean or an integer")
        }

        fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
            Ok(value)
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value != 0)
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            Ok(value != 0)
        }
    }

    deserializer.deserialize_any(BoolOrIntVisitor)
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
pub struct Episode {
    pub podcastname: String,
    pub episodetitle: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub completed: bool,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct RecentEps {
    pub episodes: Option<Vec<Episode>>,
}

pub async fn call_get_recent_eps(
    server_name: &String,
    api_key: &Option<String>,
    user_id: &i32,
) -> Result<Vec<Episode>, anyhow::Error> {
    let url = format!("{}/api/data/return_episodes/{}", server_name, user_id);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to fetch episodes: {}",
            response.status_text()
        )));
    }

    // First, capture the response text for diagnostic purposes
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to get response text".to_string());
    // Try to deserialize the response text
    match serde_json::from_str::<RecentEps>(&response_text) {
        Ok(response_body) => Ok(response_body.episodes.unwrap_or_else(Vec::new)),
        Err(_e) => Err(anyhow::Error::msg("Failed to deserialize response")),
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
    pub user_id: i32,
}

#[derive(serde::Deserialize)]
struct PodcastStatusResponse {
    success: bool,
    // Include other fields if your response contains more data
}

pub async fn call_add_podcast(
    server_name: &str,
    api_key: &Option<String>,
    _user_id: i32,
    added_podcast: &PodcastValues,
) -> Result<bool, Error> {
    let url = format!("{}/api/data/add_podcast", server_name);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

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
        Err(Error::msg(format!(
            "Error adding podcast: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemovePodcastValues {
    pub podcast_id: i32,
    pub user_id: i32,
}

pub async fn call_remove_podcasts(
    server_name: &String,
    api_key: &Option<String>,
    remove_podcast: &RemovePodcastValues,
) -> Result<bool, Error> {
    let url = format!("{}/api/data/remove_podcast_id", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    // Serialize `added_podcast` into JSON
    let json_body = serde_json::to_string(remove_podcast)?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to get response text".to_string());

    if response.ok() {
        match serde_json::from_str::<PodcastStatusResponse>(&response_text) {
            Ok(parsed_response) => Ok(parsed_response.success),
            Err(_parse_error) => Err(anyhow::Error::msg("Failed to parse response")),
        }
    } else {
        Err(anyhow::Error::msg(format!(
            "Error removing podcast: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RemovePodcastValuesName {
    pub user_id: i32,
    pub podcast_name: String,
    pub podcast_url: String,
}

pub async fn call_remove_podcasts_name(
    server_name: &String,
    api_key: &Option<String>,
    remove_podcast: &RemovePodcastValuesName,
) -> Result<bool, Error> {
    let url = format!("{}/api/data/remove_podcast", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    // Serialize `added_podcast` into JSON
    let json_body = serde_json::to_string(remove_podcast)?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(json_body)?
        .send()
        .await?;

    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to get response text".to_string());

    if response.ok() {
        match serde_json::from_str::<PodcastStatusResponse>(&response_text) {
            Ok(parsed_response) => Ok(parsed_response.success),
            Err(_parse_error) => Err(anyhow::Error::msg("Failed to parse response")),
        }
    } else {
        Err(anyhow::Error::msg(format!(
            "Error removing podcast: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct PodcastResponse {
    pub pods: Option<Vec<Podcast>>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
pub struct Podcast {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: Option<String>,
    pub description: Option<String>,
    pub episodecount: i32,
    pub websiteurl: Option<String>,
    pub feedurl: String,
    pub author: Option<String>,
    pub categories: String, // Keeping as String since it's handled as empty string "{}" or "{}"
    #[serde(deserialize_with = "bool_from_int")]
    pub explicit: bool,
}

pub async fn call_get_podcasts(
    server_name: &String,
    api_key: &Option<String>,
    user_id: &i32,
) -> Result<Vec<Podcast>, anyhow::Error> {
    let url = format!("{}/api/data/return_pods/{}", server_name, user_id);

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to fetch podcasts: {}",
            response.status_text()
        )));
    }

    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to get response text".to_string());

    match serde_json::from_str::<PodcastResponse>(&response_text) {
        Ok(response_body) => Ok(response_body.pods.unwrap_or_else(Vec::new)),
        Err(e) => {
            web_sys::console::log_1(
                &format!("Unable to parse Podcasts: {}", &response_text).into(),
            );
            Err(anyhow::Error::msg(format!(
                "Failed to deserialize response: {}",
                e
            )))
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct TimeInfoResponse {
    pub timezone: String,
    pub hour_pref: i32,
}
#[allow(dead_code)]
pub async fn call_get_time_info(
    server: &str,
    key: String,
    user_id: i32,
) -> Result<TimeInfoResponse, anyhow::Error> {
    let endpoint = format!("{}/api/data/get_time_info?user_id={}", server, user_id);

    let resp = Request::get(&endpoint)
        .header("Api-Key", key.as_str())
        .send()
        .await
        .context("Network Request Error")?;

    if resp.ok() {
        resp.json::<TimeInfoResponse>()
            .await
            .context("Response Parsing Error")
    } else {
        Err(anyhow::anyhow!(
            "Error fetching time info. Server Response: {}",
            resp.status_text()
        ))
    }
}

#[derive(Default, Deserialize, Debug)]
pub struct CheckPodcastResponse {
    pub exists: bool,
}

pub async fn call_check_podcast(
    server: &str,
    api_key: &str,
    user_id: i32,
    podcast_name: &str,
    podcast_url: &str,
) -> Result<CheckPodcastResponse, Error> {
    let encoded_name = utf8_percent_encode(podcast_name, NON_ALPHANUMERIC).to_string();
    let encoded_url = utf8_percent_encode(podcast_url, NON_ALPHANUMERIC).to_string();
    let endpoint = format!(
        "{}/api/data/check_podcast?user_id={}&podcast_name={}&podcast_url={}",
        server, user_id, encoded_name, encoded_url
    );

    let resp = Request::get(&endpoint)
        .header("Api-Key", api_key)
        .send()
        .await
        .context("Network Request Error")?;

    if resp.ok() {
        resp.json::<CheckPodcastResponse>()
            .await
            .context("Response Parsing Error")
    } else {
        Err(anyhow::anyhow!(
            "Error checking podcast. Server Response: {}",
            resp.status_text()
        ))
    }
}

#[derive(Deserialize, Debug)]
pub struct EpisodeInDbResponse {
    pub episode_in_db: bool,
}
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};

pub async fn call_check_episode_in_db(
    server: &str,
    api_key: &str,
    user_id: i32,
    episode_title: &str,
    episode_url: &str,
) -> Result<bool, anyhow::Error> {
    let encoded_title = utf8_percent_encode(episode_title, NON_ALPHANUMERIC).to_string();
    let encoded_url = utf8_percent_encode(episode_url, NON_ALPHANUMERIC).to_string();
    let endpoint = format!(
        "{}/api/data/check_episode_in_db/{}?episode_title={}&episode_url={}",
        server, user_id, encoded_title, encoded_url
    );

    let resp = Request::get(&endpoint)
        .header("Api-Key", api_key)
        .send()
        .await
        .context("Network Request Error")?;

    if resp.ok() {
        let episode_in_db_response = resp
            .json::<EpisodeInDbResponse>()
            .await
            .context("Response Parsing Error")?;
        Ok(episode_in_db_response.episode_in_db)
    } else {
        Err(anyhow::anyhow!(
            "Error checking episode in db. Server Response: {}",
            resp.status_text()
        ))
    }
}

// Queue calls

#[derive(Serialize, Deserialize, Debug)]
pub struct QueuePodcastRequest {
    pub episode_id: i32,
    pub user_id: i32,
}

// Define a struct to match the response JSON structure
#[derive(Deserialize, Debug)]
struct QueueResponse {
    data: String,
}

pub async fn call_queue_episode(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &QueuePodcastRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/queue_pod", server_name);

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Deserialize the JSON response into QueueResponse
        let response_body: QueueResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        // Extract and return the data string
        Ok(response_body.data)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to queue episode: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

pub async fn call_remove_queued_episode(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &QueuePodcastRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/remove_queued_pod", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Read the success message from the response body as text
        let success_message = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Episode queued successfully"));
        Ok(success_message)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to queue episode: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct QueuedEpisodesResponse {
    pub episodes: Vec<QueuedEpisode>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
pub struct QueuedEpisode {
    pub episodetitle: String,
    pub podcastname: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    #[serde(default)]
    pub queueposition: Option<i32>,
    pub episodeduration: i32,
    pub queuedate: String,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub completed: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DataResponse {
    pub data: Vec<QueuedEpisode>,
}

pub async fn call_get_queued_episodes(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32,
) -> Result<Vec<QueuedEpisode>, anyhow::Error> {
    // Append the user_id as a query parameter
    let url = format!(
        "{}/api/data/get_queued_episodes?user_id={}",
        server_name, user_id
    );

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to fetch queued episodes: {}",
            response.status_text()
        )));
    }
    let response_text = response.text().await?;

    let response_data: DataResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.data)
}

#[derive(Serialize)]
struct ReorderPayload {
    episode_ids: Vec<i32>,
}

pub async fn call_reorder_queue(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32,
    episode_ids: &Vec<i32>,
) -> Result<(), Error> {
    // Build the URL
    let url = format!("{}/api/data/reorder_queue?user_id={}", server_name, user_id);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    // Create the payload
    let payload = ReorderPayload {
        episode_ids: episode_ids.clone(),
    };

    // Send the request
    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .json(&payload)?
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to reorder queue: {}",
            response.status_text()
        )));
    }

    Ok(())
}

// Save episode calls

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct SavedEpisodesResponse {
    pub episodes: Vec<SavedEpisode>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
pub struct SavedEpisode {
    pub episodetitle: String,
    pub podcastname: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub websiteurl: String,
    pub completed: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct SavedDataResponse {
    pub saved_episodes: Vec<SavedEpisode>,
}

pub async fn call_get_saved_episodes(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32,
) -> Result<Vec<SavedEpisode>, anyhow::Error> {
    // Append the user_id as a query parameter
    let url = format!("{}/api/data/saved_episode_list/{}", server_name, user_id);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to fetch saved episodes: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;
    // let response_text = response.text().await?;

    let response_data: SavedDataResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.saved_episodes)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SavePodcastRequest {
    pub episode_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize, Debug)]
struct SaveResponse {
    detail: String,
}

pub async fn call_save_episode(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &SavePodcastRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/save_episode", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Deserialize the JSON response into SaveResponse
        let response_body: SaveResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        // Extract and return the detail string
        Ok(response_body.detail)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to save episode: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

pub async fn call_remove_saved_episode(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &SavePodcastRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/remove_saved_episode", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Read the success message from the response body as text
        let success_message = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Episode saved successfully"));
        Ok(success_message)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to save episode: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

// History calls

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct HistoryEpisodesResponse {
    pub episodes: Vec<HistoryEpisode>,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
pub struct HistoryEpisode {
    pub episodetitle: String,
    pub podcastname: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub completed: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct HistoryDataResponse {
    pub data: Vec<HistoryEpisode>,
}

pub async fn call_get_user_history(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32,
) -> Result<Vec<HistoryEpisode>, anyhow::Error> {
    // Append the user_id as a query parameter
    let url = format!("{}/api/data/user_history/{}", server_name, user_id);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to fetch history: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;

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
    request_data: &HistoryAddRequest,
) -> Result<(), Error> {
    let url = format!("{}/api/data/record_podcast_history", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_str();

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to record history: {}",
            response.status_text()
        )));
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
#[serde(rename_all = "lowercase")]
pub struct EpisodeDownload {
    pub episodetitle: String,
    pub podcastname: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub downloadedlocation: String,
    pub podcastid: i32,
    pub completed: bool,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct DownloadDataResponse {
    #[serde(rename = "downloaded_episodes")]
    pub episodes: Vec<EpisodeDownload>,
}

pub async fn call_get_episode_downloads(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32,
) -> Result<Vec<EpisodeDownload>, anyhow::Error> {
    // Append the user_id as a query parameter
    let url = format!(
        "{}/api/data/download_episode_list?user_id={}",
        server_name, user_id
    );

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to episode downloads: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;

    let response_data: DownloadDataResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.episodes)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadEpisodeRequest {
    pub episode_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize, Debug)]
struct DownloadResponse {
    detail: String,
}

pub async fn call_download_episode(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &DownloadEpisodeRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/download_podcast", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Deserialize the JSON response into DownloadResponse
        let response_body: DownloadResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        // Extract and return the detail string
        Ok(response_body.detail)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to download episode: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DownloadAllPodcastRequest {
    pub podcast_id: i32,
    pub user_id: i32,
}

pub async fn call_download_all_podcast(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &DownloadAllPodcastRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/download_all_podcast", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Deserialize the JSON response into DownloadResponse
        let response_body: DownloadResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        // Extract and return the detail string
        Ok(response_body.detail)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to download all episodes: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

pub async fn call_remove_downloaded_episode(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &DownloadEpisodeRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/delete_episode", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        // Read the success message from the response body as text
        let success_message = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Episode downloaded successfully"));
        Ok(success_message)
    } else {
        // Read the error response body as text to include in the error
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to download episode: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

// Get Single Epsiode

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct EpisodeInfo {
    pub episodetitle: String,
    pub podcastname: String,
    pub podcastid: i32,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub completed: bool,
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

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(episode_request)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to episode downloads: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;

    let response_data: EpisodeMetadataResponse = serde_json::from_str(&response_text)
        .map_err(|e| anyhow::Error::msg(format!("Deserialization Error: {}", e)))?;

    Ok(response_data.episode)
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
#[allow(non_snake_case)]
pub struct Chapter {
    pub startTime: Option<i32>, // Changed to Option<String>
    pub title: String,
    pub url: Option<String>,
    pub img: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Transcript {
    pub url: String,
    pub mime_type: String,
    pub language: Option<String>,
    pub rel: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Person {
    pub name: String,
    pub role: Option<String>,
    pub group: Option<String>,
    pub img: Option<String>,
    pub href: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Podcasting2Data {
    pub chapters: Vec<Chapter>,
    pub transcripts: Vec<Transcript>,
    pub people: Vec<Person>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FetchPodcasting2DataRequest {
    pub episode_id: i32,
    pub user_id: i32,
}

pub async fn call_fetch_podcasting_2_data(
    server_name: &str,
    api_key: &Option<String>,
    episode_request: &FetchPodcasting2DataRequest,
) -> Result<Podcasting2Data, Error> {
    let url = format!(
        "{}/api/data/fetch_podcasting_2_data?episode_id={}&user_id={}",
        server_name, episode_request.episode_id, episode_request.user_id
    );

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Request Error: {}", e)))?;

    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch podcasting 2.0 data: {}",
            response.status()
        )));
    }

    let response_text = response
        .text()
        .await
        .map_err(|e| Error::msg(format!("Failed to read response text: {}", e)))?;

    let response_data: Podcasting2Data = serde_json::from_str(&response_text)
        .map_err(|e| Error::msg(format!("Deserialization Error: {}", e)))?;

    Ok(response_data)
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct PodrollItem {
    pub feed_guid: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Funding {
    pub url: String,
    pub description: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct ValueRecipient {
    pub name: String,
    pub r#type: String,
    pub address: String,
    pub split: String,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Value {
    pub r#type: String,
    pub method: String,
    pub suggested: Option<String>,
    pub recipients: Vec<ValueRecipient>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Podcasting2PodData {
    pub people: Vec<Person>,
    pub podroll: Vec<PodrollItem>,
    pub funding: Vec<Funding>,
    pub value: Vec<Value>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FetchPodcasting2PodDataRequest {
    pub podcast_id: i32,
    pub user_id: i32,
}

pub async fn call_fetch_podcasting_2_pod_data(
    server_name: &str,
    api_key: &Option<String>,
    podcast_request: &FetchPodcasting2PodDataRequest,
) -> Result<Podcasting2PodData, Error> {
    let url = format!(
        "{}/api/data/fetch_podcasting_2_pod_data?podcast_id={}&user_id={}",
        server_name, podcast_request.podcast_id, podcast_request.user_id
    );

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| Error::msg(format!("Request Error: {}", e)))?;

    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch podcasting 2.0 pod data: {}",
            response.status()
        )));
    }

    let response_text = response
        .text()
        .await
        .map_err(|e| Error::msg(format!("Failed to read response text: {}", e)))?;

    let response_data: Podcasting2PodData = serde_json::from_str(&response_text)
        .map_err(|e| Error::msg(format!("Deserialization Error: {}", e)))?;

    Ok(response_data)
}

#[derive(Serialize)]
pub struct RecordListenDurationRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub listen_duration: f64, // Assuming float is appropriate here; adjust the type if necessary
}

#[derive(Deserialize, Debug)]
pub struct RecordListenDurationResponse {
    pub detail: String, // Assuming a simple status response; adjust according to actual API response
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
        response
            .json::<RecordListenDurationResponse>()
            .await
            .map_err(|e| Error::msg(format!("Response Parsing Error: {}", e)))
    } else {
        Err(Error::msg(format!(
            "Error recording listen duration. Server Response: {}",
            response.status_text()
        )))
    }
}

pub async fn call_increment_listen_time(
    server_name: &str,
    api_key: &str,
    user_id: i32, // Assuming user_id is an integer based on your endpoint definition
) -> Result<String, Error> {
    let url = format!("{}/api/data/increment_listen_time/{}", server_name, user_id);

    let response = Request::put(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network Request Error: {}", e)))?;

    if response.ok() {
        Ok("Listen time incremented.".to_string()) // Assuming a simple success message for now
    } else {
        Err(Error::msg(format!(
            "Error incrementing listen time. Server Response: {}",
            response.status_text()
        )))
    }
}

pub async fn call_increment_played(
    server_name: &str,
    api_key: &str,
    user_id: i32, // Assuming user_id is an integer based on your endpoint definition
) -> Result<String, Error> {
    let url = format!("{}/api/data/increment_played/{}", server_name, user_id);

    let response = Request::put(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network Request Error: {}", e)))?;

    if response.ok() {
        Ok("Played count incremented.".to_string()) // Assuming a simple success message for now
    } else {
        Err(Error::msg(format!(
            "Error incrementing played count. Server Response: {}",
            response.status_text()
        )))
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct PodcastIdResponse {
    pub episodes: i32,
}

pub async fn call_get_podcast_id(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32,
    podcast_feed: &str,
    podcast_title: &str,
) -> Result<i32, anyhow::Error> {
    // Append the user_id, podcast_feed, and podcast_title as query parameters
    let encoded_feed = utf8_percent_encode(podcast_feed, NON_ALPHANUMERIC).to_string();
    let encoded_title = utf8_percent_encode(podcast_title, NON_ALPHANUMERIC).to_string();
    let url = format!(
        "{}/api/data/get_podcast_id?user_id={}&podcast_feed={}&podcast_title={}",
        server_name, user_id, encoded_feed, encoded_title
    );

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to get podcast id: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;

    let response_data: PodcastIdResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.episodes)
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct PodcastIdEpResponse {
    pub podcast_id: i32,
}

pub async fn call_get_podcast_id_from_ep(
    server_name: &str,
    api_key: &Option<String>,
    episode_id: i32,
    user_id: i32,
) -> Result<i32, Error> {
    let url = format!(
        "{}/api/data/get_podcast_id_from_ep_id?episode_id={}&user_id={}",
        server_name, episode_id, user_id
    );

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to get podcast id: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;

    let response_data: PodcastIdEpResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.podcast_id)
}

pub async fn call_get_podcast_id_from_ep_name(
    server_name: &str,
    api_key: &Option<String>,
    episode_name: String,
    episode_url: String,
    user_id: i32,
) -> Result<i32, Error> {
    let url = format!(
        "{}/api/data/get_podcast_id_from_ep_name?episode_name={}&episode_url={}&user_id={}",
        server_name,
        urlencoding::encode(&episode_name),
        urlencoding::encode(&episode_url),
        user_id
    );

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to get podcast id: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;

    let response_data: PodcastIdEpResponse = serde_json::from_str(&response_text)?;
    Ok(response_data.podcast_id)
}

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct PodcastDetails {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: String,
    pub author: String,
    pub categories: String,
    pub description: String,
    pub episodecount: i32,
    pub feedurl: String,
    pub websiteurl: String,
    pub explicit: bool,
    pub userid: i32,
}

#[derive(Deserialize)]
struct PodcastDetailsResponse {
    details: PodcastDetails,
}

pub async fn call_get_podcast_details(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    podcast_id: &i32,
) -> Result<PodcastDetails, Error> {
    let url = format!(
        "{}/api/data/get_podcast_details?user_id={}&podcast_id={}",
        server_name, user_id, podcast_id
    );

    let response = Request::get(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network request error: {}", e)))?;

    if response.ok() {
        let response_data: PodcastDetailsResponse = response
            .json()
            .await
            .map_err(|e| Error::msg(format!("Failed to parse response: {}", e)))?;

        Ok(response_data.details)
    } else {
        Err(Error::msg(format!(
            "Error retrieving podcast details. Server response: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MarkEpisodeCompletedRequest {
    pub episode_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize, Debug)]
struct MarkEpisodeCompletedResponse {
    detail: String,
}

pub async fn call_mark_episode_completed(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &MarkEpisodeCompletedRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/mark_episode_completed", server_name);

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;
    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body: MarkEpisodeCompletedResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.detail)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to mark episode completed: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

pub async fn call_mark_episode_uncompleted(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &MarkEpisodeCompletedRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/mark_episode_uncompleted", server_name);

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;
    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body: MarkEpisodeCompletedResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.detail)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to mark episode completed: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoDownloadRequest {
    pub podcast_id: i32,
    pub user_id: i32,
    pub auto_download: bool,
}

#[derive(Deserialize, Debug)]
struct AutoDownloadResponse {
    detail: String,
}

pub async fn call_enable_auto_download(
    server_name: &String,
    api_key: &String,
    request_data: &AutoDownloadRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/enable_auto_download", server_name);

    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body: AutoDownloadResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.detail)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to enable auto-download: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoDownloadStatusRequest {
    pub podcast_id: i32,
    user_id: i32,
}

#[derive(Deserialize, Debug)]
pub struct AutoDownloadStatusResponse {
    pub auto_download: bool,
}

pub async fn call_get_auto_download_status(
    server_name: &str,
    user_id: i32,
    api_key: &Option<String>,
    podcast_id: i32,
) -> Result<bool, anyhow::Error> {
    let url = format!("{}/api/data/get_auto_download_status", server_name);

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;
    let request_body = serde_json::to_string(&AutoDownloadStatusRequest {
        podcast_id,
        user_id,
    })
    .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body: AutoDownloadStatusResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.auto_download)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to get auto-download status: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SkipTimesRequest {
    pub podcast_id: i32,
    pub start_skip: i32,
    pub end_skip: i32,
    pub user_id: i32,
}

#[derive(Deserialize, Debug)]
struct SkipTimesResponse {
    detail: String,
}

pub async fn call_adjust_skip_times(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &SkipTimesRequest,
) -> Result<String, Error> {
    let url = format!("{}/api/data/adjust_skip_times", server_name);

    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;
    let request_body = serde_json::to_string(request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body: SkipTimesResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.detail)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to adjust skip times: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AutoSkipTimesRequest {
    pub podcast_id: i32,
    pub user_id: i32,
}

#[derive(Deserialize, Debug)]
pub struct AutoSkipTimesResponse {
    pub start_skip: i32,
    pub end_skip: i32,
}

pub async fn call_get_auto_skip_times(
    server_name: &str,
    api_key: &Option<String>,
    user_id: i32,
    podcast_id: i32,
) -> Result<(i32, i32), Error> {
    let url = format!("{}/api/data/get_auto_skip_times", server_name);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(&AutoSkipTimesRequest {
        podcast_id,
        user_id,
    })?;

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key_ref)
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_data: AutoSkipTimesResponse = response.json().await?;
        Ok((response_data.start_skip, response_data.end_skip))
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Failed to read error message".to_string());
        Err(Error::msg(format!(
            "Failed to get auto skip times: {}",
            error_text
        )))
    }
}
