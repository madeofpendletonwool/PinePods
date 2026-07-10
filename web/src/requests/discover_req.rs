// Request layer for the Discover page's podcast discovery (#103): personalized
// recommendations, category-filtered trending, and the category taxonomy — all proxied
// through the PinePods backend (rust-api), which forwards to the search microservice.

use crate::requests::search_pods::UnifiedPodcast;
use anyhow::Error;
use gloo_net::http::Request;
use serde::Deserialize;
use std::collections::HashMap;

// PodcastIndex /podcasts/trending returns a *reduced* feed object (no originalUrl/link/
// ownerName/lastUpdateTime/explicit/episodeCount, and newestItemPublishTime instead of
// lastUpdateTime). It can't deserialize into the strict search `Podcast` struct, so trending
// gets its own lenient shape. Every field defaults so partial feeds still parse.
#[derive(Deserialize, Default)]
struct TrendingResponse {
    #[serde(default)]
    feeds: Vec<TrendingFeed>,
}

#[derive(Deserialize, Default)]
#[allow(non_snake_case)]
struct TrendingFeed {
    #[serde(default)]
    id: i32,
    #[serde(default)]
    title: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    image: String,
    #[serde(default)]
    artwork: String,
    #[serde(default)]
    categories: Option<HashMap<String, String>>,
    #[serde(default)]
    newestItemPublishTime: i64,
}

impl TrendingFeed {
    fn to_unified(self) -> UnifiedPodcast {
        let image = if self.image.is_empty() {
            self.artwork.clone()
        } else {
            self.image.clone()
        };
        let artwork = if self.artwork.is_empty() {
            image.clone()
        } else {
            self.artwork
        };
        UnifiedPodcast {
            id: self.id,
            index_id: self.id,
            title: self.title,
            url: self.url.clone(),
            originalUrl: self.url,
            link: String::new(),
            description: self.description,
            author: self.author.clone(),
            ownerName: self.author,
            image,
            artwork,
            lastUpdateTime: self.newestItemPublishTime,
            categories: self.categories,
            explicit: false,
            episodeCount: 0,
        }
    }
}

// Mirrors rust-api's models::RecommendedPodcast (GET /api/data/recommendations).
#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct RecommendedPodcast {
    #[serde(default)]
    pub podcastindexid: Option<i64>,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub feedurl: Option<String>,
    #[serde(default)]
    pub categories: HashMap<String, String>,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub reason: String,
}

impl RecommendedPodcast {
    // Adapt a recommendation into the UnifiedPodcast the shared PodcastItem card renders.
    // The external PodcastIndex feed id goes into id/index_id so the add flow subscribes correctly.
    pub fn to_unified(&self) -> UnifiedPodcast {
        let idx = self.podcastindexid.unwrap_or(0) as i32;
        let img = self.image.clone().unwrap_or_default();
        let url = self.feedurl.clone().unwrap_or_default();
        let author = self.author.clone().unwrap_or_default();
        UnifiedPodcast {
            id: idx,
            index_id: idx,
            title: self.title.clone(),
            url: url.clone(),
            originalUrl: url,
            link: String::new(),
            description: self.description.clone().unwrap_or_default(),
            author: author.clone(),
            ownerName: author,
            image: img.clone(),
            artwork: img,
            lastUpdateTime: 0,
            categories: Some(self.categories.clone()),
            explicit: false,
            episodeCount: 0,
        }
    }
}

// One category from PodcastIndex /categories/list (proxied via /api/data/proxy_categories).
#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct PodcastCategory {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub name: String,
}

#[derive(Deserialize, Default)]
struct CategoriesResponse {
    #[serde(default)]
    feeds: Vec<PodcastCategory>,
}

// Personalized "podcasts you might like". refresh=true forces a server-side recompute.
pub async fn call_get_recommendations(
    server_name: &str,
    api_key: &str,
    refresh: bool,
) -> Result<Vec<RecommendedPodcast>, Error> {
    let mut url = format!("{}/api/data/recommendations", server_name);
    if refresh {
        url.push_str("?refresh=1");
    }
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch recommendations: {}",
            response.status_text()
        )));
    }
    Ok(response.json().await?)
}

// Trending podcasts, optionally filtered to a single category. Returns UnifiedPodcast so the
// shared PodcastItem card (with its subscribe toggle) can render the results directly.
pub async fn call_get_trending(
    server_name: &str,
    api_key: &str,
    category: Option<&str>,
    max: i32,
) -> Result<Vec<UnifiedPodcast>, Error> {
    let mut url = format!("{}/api/data/proxy_trending?max={}", server_name, max);
    if let Some(cat) = category {
        if !cat.is_empty() {
            url.push_str(&format!("&cat={}", urlencoding::encode(cat)));
        }
    }
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch trending: {}",
            response.status_text()
        )));
    }
    let result: TrendingResponse = response.json().await?;
    Ok(result.feeds.into_iter().map(|f| f.to_unified()).collect())
}

// The canonical PodcastIndex category taxonomy for the "Browse by category" chips.
pub async fn call_get_categories(
    server_name: &str,
    api_key: &str,
) -> Result<Vec<PodcastCategory>, Error> {
    let url = format!("{}/api/data/proxy_categories", server_name);
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch categories: {}",
            response.status_text()
        )));
    }
    let result: CategoriesResponse = response.json().await?;
    Ok(result.feeds)
}
