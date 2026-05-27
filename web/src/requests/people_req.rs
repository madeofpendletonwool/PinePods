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
