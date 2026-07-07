use anyhow::Error;
use gloo::net::http::Request;
use serde::Deserialize;

use crate::requests::episode::Episode;

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct PersonSubscription {
    pub personid: i32,
    pub userid: i32,
    pub name: String,
    pub image: String,
    pub peopledbid: Option<i32>,
    pub associatedpodcasts: usize,
    pub episode_count: i32,
}

#[allow(dead_code)]
#[derive(Deserialize)]
pub struct SubscribeResponse {
    pub message: String,
    pub person_id: i32,
}

#[allow(dead_code)]
pub async fn call_subscribe_to_person(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    person_id: i32,
    person_name: &str,
    person_img: &Option<String>,
    podcast_id: i32,
) -> Result<SubscribeResponse, Error> {
    let url = format!(
        "{}/api/data/person/subscribe/{}/{}",
        server_name, user_id, person_id
    );
    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({
                "person_name": person_name,
                "person_img": person_img,
                "podcast_id": podcast_id
            })
            .to_string(),
        )?
        .send()
        .await?;

    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to subscribe to person: {}",
            response.status_text()
        )));
    }

    let subscribe_response = response.json::<SubscribeResponse>().await?;
    Ok(subscribe_response)
}

#[allow(dead_code)]
pub async fn call_unsubscribe_from_person(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    person_id: i32,
    person_name: String,
) -> Result<(), Error> {
    let url = format!(
        "{}/api/data/person/unsubscribe/{}/{}",
        server_name, user_id, person_id
    );
    let response = Request::delete(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(
            serde_json::json!({
                "person_name": person_name
            })
            .to_string(),
        )?
        .send()
        .await?;
    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to unsubscribe from person: {}",
            response.status_text()
        )));
    }
    Ok(())
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct SubscriptionsResponse {
    subscriptions: Vec<PersonSubscription>,
}

#[allow(dead_code)]
pub async fn call_get_person_subscriptions(
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<Vec<PersonSubscription>, Error> {
    let url = format!("{}/api/data/person/subscriptions/{}", server_name, user_id);
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch person subscriptions: {}",
            response.status_text()
        )));
    }
    let response_text = response.text().await?;
    let subscriptions_response: SubscriptionsResponse = serde_json::from_str(&response_text)?;
    Ok(subscriptions_response.subscriptions)
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct PersonEpisodesResponse {
    episodes: Vec<Episode>,
}

#[allow(dead_code)]
pub async fn call_get_person_episodes(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    person_id: i32,
) -> Result<Vec<Episode>, Error> {
    let url = format!(
        "{}/api/data/person/episodes/{}/{}?limit=50&offset=0",
        server_name, user_id, person_id
    );

    let response = Request::get(&url).header("Api-Key", api_key).send().await?;

    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch person episodes: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;
    web_sys::console::log_1(&format!("Raw response: {}", response_text).into());

    let episodes_response: PersonEpisodesResponse = serde_json::from_str(&response_text)?;
    web_sys::console::log_1(&format!("Parsed episodes: {:?}", episodes_response.episodes).into());

    Ok(episodes_response.episodes)
}

// Unified host feed — one endpoint that returns the shows a host appears in (from both the
// Podcast Index person index and PodPeopleDB) plus the merged, artwork-resolved episode list,
// with per-episode interaction state for podcasts the user is subscribed to.
#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct HostFeedPerson {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub image: Option<String>,
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct HostFeedPodcast {
    #[serde(default)]
    pub podcastname: String,
    #[serde(default)]
    pub feedurl: String,
    #[serde(default)]
    pub artworkurl: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub websiteurl: String,
    #[serde(default)]
    pub episodecount: i32,
    #[serde(default)]
    pub podcastindexid: i32,
    #[serde(default)]
    pub explicit: bool,
    #[serde(default)]
    pub is_subscribed: bool,
}

#[derive(Deserialize, Default)]
pub struct HostFeedResponse {
    #[serde(default)]
    pub person: HostFeedPerson,
    #[serde(default)]
    pub podcasts: Vec<HostFeedPodcast>,
    #[serde(default)]
    pub episodes: Vec<Episode>,
}

#[allow(dead_code)]
pub async fn call_get_host_feed(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    name: &str,
    person_id: Option<i32>,
    include_podcasts: bool,
) -> Result<HostFeedResponse, Error> {
    let mut url = format!(
        "{}/api/data/person/feed/{}?name={}&include_podcasts={}",
        server_name,
        user_id,
        urlencoding::encode(name),
        include_podcasts
    );
    if let Some(pid) = person_id {
        url.push_str(&format!("&person_id={}", pid));
    }

    let response = Request::get(&url).header("Api-Key", api_key).send().await?;

    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch host feed: {}",
            response.status_text()
        )));
    }

    let response_text = response.text().await?;
    let feed: HostFeedResponse = serde_json::from_str(&response_text)?;
    Ok(feed)
}

// --- Discovery (PodPeopleDB, proxied through the PinePods backend) ---

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct DiscoverHost {
    #[serde(default)]
    pub id: i32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub img: String,
    #[serde(default, alias = "podcastCount")]
    pub podcast_count: i32,
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct DiscoverPodcast {
    #[serde(default)]
    pub podcast_id: i32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub host_count: i32,
    #[serde(default)]
    pub description: String,
}

#[derive(Deserialize, Clone, PartialEq, Debug, Default)]
pub struct DiscoverStats {
    #[serde(default)]
    pub total_hosts: i32,
    #[serde(default)]
    pub total_podcasts: i32,
    #[serde(default)]
    pub approved_hosts: i32,
    #[serde(default)]
    pub episodes_with_guests: i32,
}

async fn discover_get(server_name: &str, api_key: &str, kind: &str, limit: Option<i32>) -> Result<String, Error> {
    let mut url = format!(
        "{}/api/data/podpeople/discover?kind={}",
        server_name, kind
    );
    if let Some(l) = limit {
        url.push_str(&format!("&limit={}", l));
    }
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(Error::msg(format!(
            "Failed to fetch discover {}: {}",
            kind,
            response.status_text()
        )));
    }
    Ok(response.text().await?)
}

#[allow(dead_code)]
pub async fn call_discover_top_hosts(server_name: &str, api_key: &str, limit: i32) -> Result<Vec<DiscoverHost>, Error> {
    let text = discover_get(server_name, api_key, "top-hosts", Some(limit)).await?;
    Ok(serde_json::from_str(&text)?)
}

#[allow(dead_code)]
pub async fn call_discover_recent_hosts(server_name: &str, api_key: &str, limit: i32) -> Result<Vec<DiscoverHost>, Error> {
    let text = discover_get(server_name, api_key, "recent-hosts", Some(limit)).await?;
    Ok(serde_json::from_str(&text)?)
}

#[allow(dead_code)]
pub async fn call_discover_popular_podcasts(server_name: &str, api_key: &str, limit: i32) -> Result<Vec<DiscoverPodcast>, Error> {
    let text = discover_get(server_name, api_key, "popular-podcasts", Some(limit)).await?;
    Ok(serde_json::from_str(&text)?)
}

#[allow(dead_code)]
pub async fn call_discover_stats(server_name: &str, api_key: &str) -> Result<DiscoverStats, Error> {
    let text = discover_get(server_name, api_key, "stats", None).await?;
    Ok(serde_json::from_str(&text)?)
}
