use std::collections::HashMap;
use gloo_net::http::Request;
use serde::Deserialize;
use anyhow::Error;
use rss::Channel;
use rss::extension::itunes::ITunesItemExtension;

// #[derive(Deserialize, Debug)]
// pub struct Episode {
//     pub PodcastName: String,
//     pub EpisodeTitle: String,
//     pub EpisodePubDate: String,
//     pub EpisodeDescription: String,
// }

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

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct Episode {
    pub title: Option<String>,
    pub description: Option<String>,
    pub pub_date: Option<String>,
    pub links: Vec<String>,
    // Enclosure for audio file URL
    pub enclosure_url: Option<String>,
    pub enclosure_length: Option<String>,
    pub artwork: Option<String>,
    // ... other item fields ...
    pub content: Option<String>,
    pub authors: Vec<String>,
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

pub async fn call_parse_podcast_url(podcast_url: &str) -> Result<PodcastFeedResult, Error> {
    let response_text = Request::get(podcast_url).send().await?.text().await?;
    let channel = Channel::read_from(response_text.as_bytes())?;

    let podcast_artwork_url = channel.image().map(|img| img.url().to_string());

    let episodes = channel.items().iter().map(|item| {
        let episode_artwork_url = item.itunes_ext().and_then(|ext| ext.image()).map(|url| url.to_string()).or_else(|| podcast_artwork_url.clone());
        let audio_url = item.enclosure().map(|enclosure| enclosure.url().to_string());
        let itunes_extension = item.itunes_ext();
        let duration = itunes_extension.and_then(|ext| ext.duration()).map(|d| d.to_string());

        Episode {
            title: Option::from(item.title().map(|t| t.to_string()).unwrap_or_default()),
            description: Option::from(item.description().map(|d| d.to_string()).unwrap_or_default()),
            content: item.content().map(|c| c.to_string()),
            enclosure_url: audio_url,
            enclosure_length: item.enclosure().map(|e| e.length().to_string()),
            pub_date: item.pub_date().map(|p| p.to_string()),
            authors: item.author().map(|a| vec![a.to_string()]).unwrap_or_default(),
            links: item.link().map(|l| vec![l.to_string()]).unwrap_or_default(),
            artwork: episode_artwork_url,
            duration

            // Map other necessary fields
        }
    }).collect();

    let feed_result = PodcastFeedResult {
        episodes,
        // Add other fields from Channel if necessary
    };

    Ok(feed_result)
}