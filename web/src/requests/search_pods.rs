use crate::components::podcast_layout::ClickedFeedURL;
use anyhow::Error;
use chrono::DateTime;
use gloo_net::http::Request;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use wasm_bindgen::JsValue;

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
pub struct RecentEps {
    pub episodes: Vec<Episode>,
}

#[derive(Deserialize, Debug, PartialEq, Clone, Serialize)]
#[allow(non_snake_case)]
pub struct PodcastSearchResult {
    pub status: Option<String>,              // for PodcastIndex
    pub resultCount: Option<i32>,            // for iTunes
    pub feeds: Option<Vec<Podcast>>,         // for PodcastIndex
    pub results: Option<Vec<ITunesPodcast>>, // for iTunes
}

#[derive(Deserialize, Debug, PartialEq, Clone, Serialize)]
#[allow(non_snake_case)]
pub struct UnifiedPodcast {
    pub(crate) id: i64,
    pub(crate) index_id: i64,
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

// Implement conversions from Podcast and ITunesPodcast to UnifiedPodcast
impl From<Podcast> for UnifiedPodcast {
    fn from(podcast: Podcast) -> Self {
        UnifiedPodcast {
            id: podcast.id,
            index_id: podcast.id,
            title: podcast.title,
            url: podcast.url,
            originalUrl: podcast.originalUrl,
            author: podcast.author,
            ownerName: podcast.ownerName,
            description: podcast.description,
            image: podcast.image, // Assuming artwork is the image you want to use
            link: podcast.link,
            artwork: podcast.artwork,
            lastUpdateTime: podcast.lastUpdateTime,
            categories: podcast.categories,
            explicit: podcast.explicit,
            episodeCount: podcast.episodeCount,
        }
    }
}

impl From<ITunesPodcast> for UnifiedPodcast {
    fn from(podcast: ITunesPodcast) -> Self {
        let genre_map: HashMap<String, String> = podcast
            .genres
            .into_iter()
            .enumerate()
            .map(|(index, genre)| (index.to_string(), genre))
            .collect();

        let parsed_date = DateTime::parse_from_rfc3339(&podcast.releaseDate)
            .map(|dt| dt.timestamp())
            .unwrap_or(0); // Default to 0 or choose a more sensible default

        UnifiedPodcast {
            id: podcast.trackId,
            index_id: 0,
            title: podcast.trackName,
            url: podcast.feedUrl.clone().unwrap_or_default(),
            originalUrl: podcast.feedUrl.unwrap_or_default(),
            author: podcast.artistName.clone(),
            ownerName: podcast.artistName,
            description: String::from("Descriptions not provided by iTunes"),
            image: podcast.artworkUrl100.clone(),
            link: podcast.collectionViewUrl,
            artwork: podcast.artworkUrl100,
            lastUpdateTime: parsed_date,
            categories: Some(genre_map),
            explicit: match podcast.collectionExplicitness.as_str() {
                "explicit" => true,
                "notExplicit" => false,
                _ => false,
            },
            episodeCount: podcast.trackCount.unwrap_or(0),
            // Map other fields as necessary
        }
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone, Serialize)]
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

#[derive(Deserialize, Debug, PartialEq, Clone, Serialize)]
#[allow(non_snake_case)]
pub struct ITunesPodcast {
    pub wrapperType: String,
    pub kind: String,
    pub collectionId: i64,
    pub trackId: i64,
    pub(crate) artistName: String,
    pub(crate) trackName: String,
    pub(crate) collectionViewUrl: String,
    pub(crate) feedUrl: Option<String>,
    pub(crate) artworkUrl100: String,
    pub(crate) releaseDate: String,
    pub(crate) genres: Vec<String>,
    pub(crate) collectionExplicitness: String,
    #[allow(non_snake_case)]
    pub(crate) trackCount: Option<i32>,
    // add other fields as needed
}

fn deserialize_string_or_int<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct StringOrIntVisitor;

    impl<'de> Visitor<'de> for StringOrIntVisitor {
        type Value = Option<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string, an integer or null")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_owned()))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(value.to_string()))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            deserializer.deserialize_any(self)
        }
    }

    deserializer.deserialize_option(StringOrIntVisitor)
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Episode {
    #[serde(rename = "Episodetitle")]
    pub title: Option<String>,
    #[serde(rename = "Episodedescription")]
    pub description: Option<String>,
    #[serde(rename = "Episodepubdate")]
    pub pub_date: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub links: Vec<String>,
    #[serde(rename = "Episodeurl")]
    pub enclosure_url: Option<String>,
    pub enclosure_length: Option<String>,
    #[serde(rename = "Episodeartwork")]
    pub artwork: Option<String>,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub authors: Vec<String>,
    pub guid: Option<String>,
    #[serde(
        rename = "Episodeduration",
        deserialize_with = "deserialize_string_or_int"
    )]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "Episodeid")]
    pub episode_id: Option<i32>,
    #[serde(default)]
    pub is_youtube: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "Completed")]
    pub completed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "Listenduration")]
    pub listen_duration: Option<i32>,
}

#[derive(Deserialize, Debug, PartialEq, Clone, Serialize)]
pub struct PodcastFeedResult {
    pub(crate) episodes: Vec<Episode>,
}

pub async fn call_get_podcast_info(
    podcast_value: &String,
    search_api_url: &Option<String>,
    search_index: &str,
) -> Result<PodcastSearchResult, anyhow::Error> {
    let url = if let Some(api_url) = search_api_url {
        format!(
            "{}?query={}&index={}",
            api_url,
            urlencoding::encode(podcast_value),
            search_index
        )
    } else {
        return Err(anyhow::Error::msg("API URL is not provided"));
    };
    let response = Request::get(&url).send().await.map_err(|err| {
        web_sys::console::log_1(&JsValue::from_str(&format!("Request error: {:?}", err)));
        anyhow::Error::new(err)
    })?;
    if response.ok() {
        let response_text = response.text().await.map_err(|err| {
            web_sys::console::log_1(&JsValue::from_str(&format!(
                "Text parsing error: {:?}",
                err
            )));
            anyhow::Error::new(err)
        })?;

        // Try to parse and log any deserialization errors
        match serde_json::from_str::<PodcastSearchResult>(&response_text) {
            Ok(search_results) => Ok(search_results),
            Err(err) => {
                web_sys::console::log_1(&JsValue::from_str(&format!(
                    "Deserialization error: {:?}\nResponse text: {}",
                    err, response_text
                )));
                Err(anyhow::Error::msg(format!(
                    "Failed to parse response: {}",
                    err
                )))
            }
        }
    } else {
        let error_msg = format!("Failed to fetch podcast info: {}", response.status_text());
        web_sys::console::log_1(&JsValue::from_str(&error_msg));
        Err(anyhow::Error::msg(error_msg))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[allow(non_snake_case)]
pub struct PeopleEpisode {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: Option<i32>,
    pub title: Option<String>,
    pub link: Option<String>,
    pub description: Option<String>,
    pub guid: Option<String>,
    pub datePublished: Option<i64>, // Unix timestamp for publication date
    pub dateCrawled: Option<i64>,   // Unix timestamp for when it was crawled
    pub enclosureUrl: Option<String>,
    pub enclosureType: Option<String>,
    pub enclosureLength: Option<i64>, // Length of the enclosure in bytes
    pub duration: Option<i32>,        // Duration in seconds
    pub explicit: Option<i32>,        // Explicit flag, 0 or 1
    pub feedImage: Option<String>,
    pub feedId: Option<i64>,
    pub feedTitle: Option<String>,
    pub feedUrl: Option<String>,
}

// Custom deserialization function to truncate the ID
fn deserialize_id<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: Deserializer<'de>,
{
    let id = i64::deserialize(deserializer)?;
    Ok(Some((id as u32) as i32))
}

#[derive(Deserialize, Debug, PartialEq, Clone, Serialize)]
pub struct PeopleFeedResult {
    pub status: Option<String>,
    pub items: Vec<PeopleEpisode>,
}

#[allow(dead_code)]
pub async fn call_get_person_info(
    person_name: &String,
    search_api_url: &Option<String>,
    search_index: &str,
) -> Result<PeopleFeedResult, anyhow::Error> {
    let url = if let Some(api_url) = search_api_url {
        format!(
            "{}?query={}&index={}&search_type=person",
            api_url,
            urlencoding::encode(person_name),
            search_index
        )
    } else {
        return Err(anyhow::Error::msg("API URL is not provided"));
    };
    let response = Request::get(&url)
        .send()
        .await
        .map_err(|err| anyhow::Error::new(err))?;

    if response.ok() {
        let response_text = response
            .text()
            .await
            .map_err(|err| anyhow::Error::new(err))?;

        let search_results: PeopleFeedResult = serde_json::from_str(&response_text)?;

        Ok(search_results)
    } else {
        Err(anyhow::Error::msg(format!(
            "Failed to fetch person info: {}",
            response.status_text()
        )))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PodPeoplePodcast {
    pub podcastid: i32,
    pub podcastname: String,
    pub feedurl: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PodPeopleResponse {
    pub success: bool,
    pub podcasts: Vec<PodPeoplePodcast>,
}

// New function to call podpeople API
#[allow(dead_code)]
pub async fn call_get_podpeople_podcasts(
    hostname: &String,
    api_url: &Option<String>,
    api_key: &str,
) -> Result<PodPeopleResponse, anyhow::Error> {
    let url = if let Some(base_url) = api_url {
        format!(
            "{}/api/data/podpeople/host_podcasts?hostname={}",
            base_url, hostname
        )
    } else {
        return Err(anyhow::Error::msg("API URL is not provided"));
    };

    let response = Request::get(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|err| anyhow::Error::new(err))?;

    if response.ok() {
        let response_text = response
            .text()
            .await
            .map_err(|err| anyhow::Error::new(err))?;

        web_sys::console::log_1(&JsValue::from_str(&response_text));

        let podpeople_results: PodPeopleResponse = serde_json::from_str(&response_text)?;
        Ok(podpeople_results)
    } else {
        Err(anyhow::Error::msg(format!(
            "Failed to fetch podpeople podcasts: {}",
            response.status_text()
        )))
    }
}

pub async fn test_connection(search_api_url: &Option<String>) -> Result<(), Error> {
    let url = search_api_url
        .as_ref()
        .ok_or_else(|| Error::msg("API URL is missing"))?;

    match Request::get(url).send().await {
        Ok(response) => {
            if response.ok() {
                Ok(())
            } else {
                Err(Error::msg(format!(
                    "HTTP error occurred: {}",
                    response.status_text()
                )))
            }
        }
        Err(err) => Err(Error::new(err)),
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct PodcastEpisodesResponse {
    pub episodes: Vec<Episode>,
}

#[allow(dead_code)]
pub async fn call_get_podcast_episodes(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32,
    podcast_id: &i32,
) -> Result<PodcastFeedResult, anyhow::Error> {
    let url = format!(
        "{}/api/data/podcast_episodes?user_id={}&podcast_id={}",
        server_name, user_id, podcast_id
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
            "Failed to get podcast episodes: {}",
            response.status_text()
        )));
    }
    let response_text = response.text().await?;
    let response_data: PodcastEpisodesResponse = serde_json::from_str(&response_text)?;
    let episodes = response_data
        .episodes
        .into_iter()
        .map(|mut episode| {
            episode.guid = episode
                .guid
                .or_else(|| episode.episode_id.map(|id| id.to_string()));
            episode.is_youtube = Some(false); // Set is_youtube to false for regular episodes
                                              // Default values for new fields if not present
            episode
        })
        .collect::<Vec<_>>();
    Ok(PodcastFeedResult { episodes })
}

#[allow(dead_code)]
pub async fn call_get_youtube_episodes(
    server_name: &str,
    api_key: &Option<String>,
    user_id: &i32,
    podcast_id: &i32,
) -> Result<PodcastFeedResult, anyhow::Error> {
    let url = format!(
        "{}/api/data/youtube_episodes?user_id={}&podcast_id={}",
        server_name, user_id, podcast_id
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
            "Failed to get podcast episodes: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;

    let response_data: PodcastEpisodesResponse = serde_json::from_str(&response_text)?;

    let episodes = response_data
        .episodes
        .into_iter()
        .map(|mut episode| {
            episode.guid = episode
                .guid
                .or_else(|| episode.episode_id.map(|id| id.to_string()));
            episode.is_youtube = Some(true); // Set is_youtube to true for YouTube episodes
            episode
        })
        .collect::<Vec<_>>();
    Ok(PodcastFeedResult { episodes })
}

#[allow(dead_code)]
pub async fn call_parse_podcast_url(
    server_name: String,
    api_key: &Option<String>,
    podcast_url: &str,
) -> Result<PodcastFeedResult, Error> {
    let encoded_podcast_url = urlencoding::encode(podcast_url);
    let endpoint = format!(
        "{}/api/data/fetch_podcast_feed?podcast_feed={}",
        server_name, encoded_podcast_url
    );
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;
    let request = Request::get(&endpoint)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key_ref)
        .send()
        .await?;
    if request.ok() {
        let response_text = request.text().await?;

        // Parse JSON response from backend (feed-rs parsed data)
        let json_response: serde_json::Value = serde_json::from_str(&response_text)?;
        let episodes_json = json_response["episodes"]
            .as_array()
            .ok_or_else(|| anyhow::Error::msg("Invalid response format: missing episodes array"))?;

        // Convert JSON episodes to Episode structs
        let mut episodes: Vec<Episode> = episodes_json
            .iter()
            .map(|episode_json| Episode {
                title: episode_json["title"].as_str().map(|s| s.to_string()),
                description: episode_json["description"].as_str().map(|s| s.to_string()),
                content: episode_json["content"].as_str().map(|s| s.to_string()),
                enclosure_url: episode_json["enclosure_url"]
                    .as_str()
                    .map(|s| s.to_string()),
                enclosure_length: episode_json["enclosure_length"]
                    .as_str()
                    .map(|s| s.to_string()),
                pub_date: episode_json["pub_date"].as_str().map(|s| s.to_string()),
                artwork: episode_json["artwork"].as_str().map(|s| s.to_string()),
                duration: episode_json["duration"].as_i64().map(|d| d.to_string()),
                links: vec![],
                authors: vec![],
                guid: episode_json["guid"].as_str().map(|s| s.to_string()),
                episode_id: None,
                is_youtube: Some(false),
                completed: None,
                listen_duration: None,
            })
            .collect();

        // Sort episodes by publication date (newest first)
        episodes.sort_by(|a, b| {
            fn parse_date(date_str: &Option<String>) -> Option<chrono::DateTime<chrono::Utc>> {
                date_str.as_ref().and_then(|d| {
                    chrono::DateTime::parse_from_rfc3339(d)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .ok()
                })
            }

            let a_date = parse_date(&a.pub_date);
            let b_date = parse_date(&b.pub_date);

            match (a_date, b_date) {
                (Some(a), Some(b)) => b.cmp(&a), // Newest first
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });

        Ok(PodcastFeedResult { episodes })
    } else {
        Err(anyhow::Error::msg(format!(
            "Failed to fetch podcast feed: HTTP {}",
            request.status()
        )))
    }
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct PodcastInfo {
    pub title: String,
    pub description: String,
    pub artwork_url: Option<String>,
    pub author: String,
    pub website: String,
    pub categories: Vec<String>,
    pub explicit: bool,
    pub episode_count: i32,
}

#[allow(dead_code)]
pub async fn call_get_podcast_details_dynamic(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    podcast_title: &str,
    podcast_url: &str,
    podcast_index_id: i64,
    added: bool,
    display_only: Option<bool>,
) -> Result<PodcastDetailsResponse, Error> {
    let display_only = display_only.unwrap_or(false);
    let url = format!(
        "{}/api/data/get_podcast_details_dynamic?user_id={}&podcast_title={}&podcast_url={}&podcast_index_id={}&added={}&display_only={}",
        server_name, user_id, podcast_title, podcast_url, podcast_index_id, added, display_only
    );

    let response = Request::get(&url)
        .header("Content-Type", "application/json")
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network request error: {}", e)))?;

    if response.ok() {
        let clicked_feed: ClickedFeedURL = response
            .json()
            .await
            .map_err(|e| Error::msg(format!("Failed to parse response: {}", e)))?;

        // Wrap the response in a "details" structure to match the other endpoint
        Ok(PodcastDetailsResponse {
            details: clicked_feed,
        })
    } else {
        Err(Error::msg(format!(
            "Error retrieving podcast details. Server response: {}",
            response.status_text()
        )))
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct PodcastDetailsResponse {
    pub details: ClickedFeedURL,
}

// In Databases

#[derive(Serialize, Deserialize, Debug)]
pub struct SearchRequest {
    pub search_term: String,
    pub user_id: i32,
}

#[derive(Deserialize, Debug, PartialEq, Clone)]
pub struct SearchResponse {
    pub data: Vec<SearchEpisode>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
#[serde(rename_all = "lowercase")]
pub struct SearchEpisode {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: String,
    pub author: String,
    pub categories: Option<HashMap<String, String>>,
    pub description: String,
    pub episodecount: Option<i32>,
    pub feedurl: String,
    pub websiteurl: String,
    pub explicit: i32,
    pub userid: i32,
    pub episodeid: i32,
    pub episodetitle: String,
    pub episodedescription: String,
    pub episodeurl: String,
    pub episodeartwork: String,
    pub episodepubdate: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub completed: bool,
    pub saved: bool,      // Added field
    pub queued: bool,     // Added field
    pub downloaded: bool, // Added field
    pub is_youtube: bool,
}

#[allow(dead_code)]
pub async fn call_search_database(
    server_name: &String,
    api_key: &Option<String>,
    request_data: &SearchRequest,
) -> Result<Vec<SearchEpisode>, Error> {
    let url = format!("{}/api/data/search_data", server_name);
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

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to search database: {}",
            response.status_text()
        )));
    }

    // Get the response text first for debugging
    let response_text = response.text().await?;
    // Try to parse as SearchResponse and log any errors
    // Try to parse as SearchResponse and log any errors
    match serde_json::from_str::<SearchResponse>(&response_text) {
        Ok(search_response) => Ok(search_response.data),
        Err(e) => {
            web_sys::console::log_1(&format!("Deserialization error: {}", e).into());
            web_sys::console::log_1(&format!("Error location: column {}", e.column()).into());
            web_sys::console::log_1(&format!("Error line: {}", e.line()).into());
            Err(anyhow::Error::msg(format!("Deserialization failed: {}", e)))
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct YouTubeVideo {
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,
    pub url: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct YouTubeChannel {
    #[serde(rename = "channelId")]
    pub channel_id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "subscriberCount")]
    pub subscriber_count: Option<i64>,
    pub url: String,
    #[serde(rename = "videoCount")]
    pub video_count: Option<i32>,
    #[serde(rename = "recentVideos", default)]
    pub recent_videos: Vec<YouTubeVideo>,
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct YouTubeSearchResults {
    pub channels: Vec<YouTubeChannel>,
    pub videos: Vec<YouTubeVideo>,
}

#[derive(Deserialize, Debug)]
pub struct YouTubeSearchResponse {
    pub results: Vec<YouTubeChannel>,
}

pub async fn call_youtube_search(
    query: &str,
    search_api_url: &Option<String>,
) -> Result<YouTubeSearchResponse, Error> {
    let url = if let Some(api_url) = search_api_url {
        format!(
            "{}?query={}&index=youtube",
            api_url,
            urlencoding::encode(query)
        )
    } else {
        return Err(anyhow::Error::msg("Search API URL is not provided"));
    };

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network request error: {}", e)))?;

    if response.ok() {
        let text = response
            .text()
            .await
            .map_err(|e| Error::msg(format!("Failed to get response text: {}", e)))?;

        let search_results: YouTubeSearchResponse = serde_json::from_str(&text).map_err(|e| {
            web_sys::console::log_1(&format!("Parse error: {}", e).into());
            Error::msg(format!(
                "Failed to parse response: {} - Raw text: {}",
                e, text
            ))
        })?;

        Ok(search_results)
    } else {
        let error_text = response.status_text();
        Err(Error::msg(format!(
            "Error searching YouTube. Server response: {}",
            error_text
        )))
    }
}

// YouTube channel details response for when user clicks a channel
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct YouTubeChannelDetails {
    #[serde(rename = "channelId")]
    pub channel_id: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "thumbnailUrl")]
    pub thumbnail_url: String,
    pub url: String,
    #[serde(rename = "subscriberCount")]
    pub subscriber_count: Option<i64>,
    #[serde(rename = "videoCount")]
    pub video_count: Option<i64>,
    #[serde(rename = "recentVideos")]
    pub recent_videos: Vec<YouTubeVideoDetails>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct YouTubeVideoDetails {
    pub id: String,
    pub title: String,
    pub description: String,
    pub url: String,
    pub thumbnail: String,
    #[serde(rename = "publishedAt")]
    pub published_at: String,
    pub duration: Option<String>,
}

#[allow(dead_code)]
pub async fn call_youtube_channel_details(
    channel_id: &str,
    search_api_url: &Option<String>,
) -> Result<YouTubeChannelDetails, Error> {
    let url = if let Some(api_url) = search_api_url {
        format!("{}/youtube/channel?id={}", api_url, channel_id)
    } else {
        return Err(anyhow::Error::msg("Search API URL is not provided"));
    };

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| Error::msg(format!("Network request error: {}", e)))?;

    if response.ok() {
        let text = response
            .text()
            .await
            .map_err(|e| Error::msg(format!("Failed to get response text: {}", e)))?;

        let channel_details: YouTubeChannelDetails = serde_json::from_str(&text).map_err(|e| {
            web_sys::console::log_1(&format!("Parse error: {}", e).into());
            Error::msg(format!(
                "Failed to parse channel details: {} - Raw text: {}",
                e, text
            ))
        })?;

        Ok(channel_details)
    } else {
        let error_text = response.status_text();
        Err(Error::msg(format!(
            "Error getting YouTube channel details. Server response: {}",
            error_text
        )))
    }
}
