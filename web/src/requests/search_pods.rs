use std::collections::HashMap;
use gloo_net::http::Request;
use serde::Deserialize;
use web_sys::console;
use anyhow::Error;
use std::io::Cursor;
use feed_rs::parser;
use chrono::DateTime;

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
    pub enclosure_length: Option<u64>,
    pub artwork: Option<String>,
    // ... other item fields ...
    pub content: Option<String>,
    pub authors: Vec<String>,
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
    web_sys::console::log_1(&format!("Error: {:?}", &podcast_url).into());
    let response_text = Request::get(podcast_url).send().await?.text().await?;

    web_sys::console::log_1(&format!("Error: {:?}", &response_text).into());

    let cursor = Cursor::new(response_text);
    let feed = parser::parse(cursor)?;
    web_sys::console::log_1(&format!("Error: {:?}", &feed).into());

    // Assuming 'feed' is your parsed feed object and it has an 'image' field
    let podcast_artwork_url = feed.logo.map(|img| img.uri);


    let episodes = feed.entries.into_iter().map(|entry| {
        let episode_artwork_url = entry.links.iter()
            .find(|link| link.rel.as_deref() == Some("episode")) // Adjust the condition based on actual feed structure
            .map(|link| link.href.clone())
            .or_else(|| podcast_artwork_url.clone()); // Fallback to podcast artwork if episode-specific artwork is not found
        let links: Vec<String> = entry.links.iter()
            .map(|link| link.href.clone())
            .collect();
        let authors: Vec<String> = entry.authors.iter()
            .map(|person| person.name.clone())
            .collect();

            Episode {
            title: Option::from(entry.title.map(|t| t.content).unwrap_or_default()),
            description: Option::from(entry.summary.map(|d| d.content).unwrap_or_default()),
            content: entry.content.as_ref().and_then(|c| c.body.clone()),
            enclosure_url: entry.content.as_ref().and_then(|c| c.src.as_ref().map(|src| src.href.clone())),
            // enclosure_url: Option::from(entry.content.map(|t| t.content).unwrap_or_default()),
            enclosure_length: entry.content.as_ref().and_then(|c| c.length),
            pub_date: entry.published.map(|p| p.to_rfc3339()),
            authors,
            links,
            artwork: episode_artwork_url
            // itunes_title: Option::from(entry.title.map(|t| t.content).unwrap_or_default()),
            // itunes_author: Option::from(entry.title.map(|t| t.content).unwrap_or_default()),
            // itunes_duration: Option::from(entry.title.map(|t| t.content).unwrap_or_default()),
            // itunes_episode: Option::from(entry.title.map(|t| t.content).unwrap_or_default()),
            // itunes_explicit: Option::from(entry.title.map(|t| t.content).unwrap_or_default()),
            // itunes_summary: Option::from(entry.title.map(|t| t.content).unwrap_or_default()),
            // Map other fields of entry to Episode fields
        }
    }).collect();
    web_sys::console::log_1(&format!("Error: {:?}", &episodes).into());

    Ok(PodcastFeedResult { episodes })
}