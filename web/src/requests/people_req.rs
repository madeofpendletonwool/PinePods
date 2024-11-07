use anyhow::Error;
use gloo::net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone)]
pub struct PersonSubscription {
    pub personid: i32,
    pub userid: i32,
    pub name: String,
    pub peopledbid: Option<i32>,
    pub associatedpodcasts: Option<String>,
}

pub async fn call_subscribe_to_person(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    person_id: i32,
    person_name: &str,
    podcast_id: i32,
) -> Result<(), Error> {
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
    Ok(())
}

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
struct SubscriptionsResponse {
    subscriptions: Vec<PersonSubscription>,
}

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
