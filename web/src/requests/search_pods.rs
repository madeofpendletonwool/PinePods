use std::collections::HashMap;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use anyhow::Error;
use rss::Channel;
use web_sys::console;

#[derive(Deserialize, Debug)]
pub struct RecentEps {
    pub episodes: Vec<Episode>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct PodcastSearchResult {
    pub(crate) status: String,
    pub(crate) feeds: Vec<Podcast>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct Podcast {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) url: String,
    #[allow(non_snake_case)]
    pub(crate) originalUrl: String,
    pub(crate) link: String,
    pub(crate) description: String,
    pub(crate) author: String,
    #[allow(non_snake_case)]
    pub(crate) ownerName: String,
    pub(crate) image: String,
    pub(crate) artwork: String,
    #[allow(non_snake_case)]
    pub(crate) lastUpdateTime: i64,
    pub(crate) categories: Option<HashMap<String, String>>,
    pub(crate) explicit: bool,
    #[allow(non_snake_case)]
    pub(crate) episodeCount: i32,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct Episode {
    pub title: Option<String>,
    pub description: Option<String>,
    pub pub_date: Option<String>,
    pub links: Vec<String>,
    pub enclosure_url: Option<String>,
    pub enclosure_length: Option<String>,
    pub artwork: Option<String>,
    pub content: Option<String>,
    pub authors: Vec<String>,
    pub guid: String,
    pub duration: Option<String>
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct PodcastFeedResult {
    // ... other fields ...
    pub(crate) episodes: Vec<Episode>,
}

pub async fn call_get_podcast_info(podcast_value: &String, search_api_url: &Option<String>, search_index: &str) -> Result<PodcastSearchResult, anyhow::Error> {
    let url = if let Some(api_url) = search_api_url {
        format!("{}?query={}&index={}", api_url, podcast_value, search_index)
    } else {
        return Err(anyhow::Error::msg("API URL is not provided"));
    };

    let response = Request::get(&url).send().await.map_err(|err| anyhow::Error::new(err))?;

    if response.ok() {
        let response_text = response.text().await.map_err(|err| anyhow::Error::new(err))?;
        web_sys::console::log_1(&format!("Raw Response: {}", response_text).into());

        let search_results: PodcastSearchResult = serde_json::from_str(&response_text)?;
        Ok(search_results)
    } else {
        Err(anyhow::Error::msg(format!("Failed to fetch podcast info: {}", response.status_text())))
    }
}

pub async fn test_connection(search_api_url: &Option<String>) -> Result<(), Error> {
    let url = search_api_url.as_ref().ok_or_else(|| Error::msg("API URL is missing"))?;

    match Request::get(url).send().await {
        Ok(response) => {
            if response.ok() {
                Ok(())
            } else {
                Err(Error::msg(format!("HTTP error occurred: {}", response.status_text())))
            }
        }
        Err(err) => Err(Error::new(err)),
    }
}

pub async fn call_parse_podcast_url(podcast_url: &str) -> Result<PodcastFeedResult, Error> {
    let response_text = Request::get(podcast_url).send().await?.text().await?;
    let channel = Channel::read_from(response_text.as_bytes())?;

    let podcast_artwork_url = channel.image().map(|img| img.url().to_string());

    let episodes = channel.items().iter().map(|item| {
        let episode_artwork_url = item.itunes_ext().and_then(|ext| ext.image()).map(|url| url.to_string()).or_else(|| podcast_artwork_url.clone());
        let audio_url = item.enclosure().map(|enclosure| enclosure.url().to_string());
        let itunes_extension = item.itunes_ext();
        let duration = itunes_extension.and_then(|ext| ext.duration()).map(|d| d.to_string());
        let description = if let Some(encoded_content) = item.content() {
            Option::from(encoded_content.to_string())
        } else {
            Option::from(item.description().unwrap_or_default().to_string())
        };
        Episode {
            title: Option::from(item.title().map(|t| t.to_string()).unwrap_or_default()),
            description,
            content: item.content().map(|c| c.to_string()),
            enclosure_url: audio_url,
            enclosure_length: item.enclosure().map(|e| e.length().to_string()),
            pub_date: item.pub_date().map(|p| p.to_string()),
            authors: item.author().map(|a| vec![a.to_string()]).unwrap_or_default(),
            links: item.link().map(|l| vec![l.to_string()]).unwrap_or_default(),
            artwork: episode_artwork_url,
            guid: item.title().map(|t| t.to_string()).unwrap_or_default(),
            duration
        }
    }).collect();

    let feed_result = PodcastFeedResult {
        episodes,
    };

    Ok(feed_result)
}

// In Databases

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchRequest {
    pub search_term: String,
    pub user_id: i32,
}

#[derive(Deserialize, Debug)]
pub struct SearchResponse {
    data: Vec<SearchEpisode>,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct SearchEpisode {
    pub PodcastID: i32,
    pub PodcastName: String,
    pub ArtworkURL: String,
    pub Author: String,
    pub Categories: String, // or change to appropriate type if you plan to parse the categories
    pub Description: String,
    pub EpisodeCount: i32,
    pub FeedURL: String,
    pub WebsiteURL: String,
    pub Explicit: i32, // or bool if it always contains 0 or 1
    pub UserID: i32,
    pub EpisodeID: i32,
    pub EpisodeTitle: String,
    pub EpisodeDescription: String,
    pub EpisodeURL: String,
    pub EpisodeArtwork: String,
    pub EpisodePubDate: String,
    pub EpisodeDuration: i32,
    // Existing fields
    pub ListenDuration: Option<String>,
}

pub async fn call_search_database (
    server_name: &String, 
    api_key: &Option<String>, 
    request_data: &SearchRequest
) -> Result<Vec<SearchEpisode>, Error> {
    let url = format!("{}/api/data/search_data", server_name);

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_body = serde_json::to_string(request_data).map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

        if !response.ok() {
            return Err(anyhow::Error::msg(format!("Failed to search database: {}", response.status_text())));
        }
        // console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
        // let response_text = response.text().await?;
        
        // console::log_1(&format!("HTTP Response Body: {}", &response_text).into());
            
        // console::log_1(&format!("HTTP Response Status: {}", response.status()).into());
        // let response_text = response.text().await?;
    
        // console::log_1(&format!("HTTP Response Body: {}", &response_text).into());
        
    
    // Deserialize the response body into a SearchResponse
        let search_response: SearchResponse = response.json().await?;

        // Extract the vector of episodes from the SearchResponse
        let results = search_response.data;

        Ok(results)
}