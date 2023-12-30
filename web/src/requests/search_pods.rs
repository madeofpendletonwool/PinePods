use std::collections::HashMap;
use gloo_net::http::Request;
use serde::Deserialize;
use web_sys::console;
use anyhow::Error;

#[derive(Deserialize, Debug)]
pub struct Episode {
    pub PodcastName: String,
    pub EpisodeTitle: String,
    pub EpisodePubDate: String,
    pub EpisodeDescription: String,
}

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
pub struct Podcast {
    pub(crate) id: i64,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) originalUrl: String,
    pub(crate) link: String,
    pub(crate) description: String,
    pub(crate) author: String,
    pub(crate) ownerName: String,
    pub(crate) image: String,
    pub(crate) artwork: String,
    pub(crate) lastUpdateTime: i64,
    // ... other fields as needed ...
    pub(crate) categories: HashMap<String, String>,
    pub(crate) explicit: bool,
    pub(crate) episodeCount: i32,
    // ... other fields as needed ...
}

pub async fn call_get_podcast_info(podcast_value: &String, search_api_url: &Option<String>, search_index: &str) -> Result<PodcastSearchResult, anyhow::Error> {
    let url = if let Some(api_url) = search_api_url {
        format!("{}?query={}&index={}", api_url, podcast_value, search_index)
    } else {
        return Err(anyhow::Error::msg("API URL is not provided"));
    };
    web_sys::console::log_1(&format!("Error: {}", &url).into());

    let response = Request::get(&url).send().await.map_err(|err| anyhow::Error::new(err))?;
    web_sys::console::log_1(&format!("Error: {:?}", &response).into());

    if response.ok() {
        let search_results: PodcastSearchResult = response.json().await.map_err(|err| anyhow::Error::new(err))?;
        web_sys::console::log_1(&format!("Search Results: {:?}", &search_results).into());
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