//! Inbox and episode label calls. The server acts for the user the API key belongs to.

use crate::requests::episode::Episode;
use crate::requests::pod_req::FeedPage;
use anyhow::Error;
use gloo_net::http::{Request, Response};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub struct EpisodeRef {
    pub episode_id: i32,
    pub is_youtube: bool,
}

impl From<&Episode> for EpisodeRef {
    fn from(ep: &Episode) -> Self {
        EpisodeRef { episode_id: ep.episodeid, is_youtube: ep.is_youtube }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct EpisodeLabel {
    pub label_id: i32,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub episode_count: i64,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct LabelAssignment {
    pub label_id: i32,
    pub episode_id: i32,
    pub is_youtube: bool,
}

/// Which part of the inbox to list or clear.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InboxQuery {
    pub label_id: Option<i32>,
    pub unlabeled: bool,
    pub podcast_id: Option<i32>,
    pub oldest_first: bool,
}

/// Colours a label can have (the server accepts only these).
pub const LABEL_COLORS: &[&str] = &["blue", "green", "yellow", "orange", "red", "purple", "pink", "teal", "gray"];

async fn fail(response: Response) -> Error {
    #[derive(Deserialize)]
    struct Body {
        message: Option<String>,
        detail: Option<String>,
    }
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let msg = serde_json::from_str::<Body>(&text)
        .ok()
        .and_then(|b| b.message.or(b.detail))
        .unwrap_or_else(|| format!("Request failed ({status})"));
    Error::msg(msg)
}

async fn get<T: DeserializeOwned>(url: &str, api_key: &str) -> Result<T, Error> {
    let response = Request::get(url).header("Api-Key", api_key).send().await?;
    if response.ok() {
        Ok(response.json::<T>().await?)
    } else {
        Err(fail(response).await)
    }
}

async fn send<B: Serialize, T: DeserializeOwned>(method: &str, url: &str, api_key: &str, body: &B) -> Result<T, Error> {
    let builder = match method {
        "PUT" => Request::put(url),
        "DELETE" => Request::delete(url),
        _ => Request::post(url),
    };
    let response = builder
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(body)?)?
        .send()
        .await?;
    if response.ok() {
        Ok(response.json::<T>().await?)
    } else {
        Err(fail(response).await)
    }
}

#[derive(Deserialize)]
struct Changed {
    changed: u64,
}

pub async fn call_get_inbox(server: &str, api_key: &str, query: &InboxQuery, limit: i64, offset: i64) -> Result<FeedPage, Error> {
    let mut url = format!("{server}/api/data/inbox?limit={limit}&offset={offset}");
    if let Some(id) = query.label_id {
        url.push_str(&format!("&label_id={id}"));
    } else if query.unlabeled {
        url.push_str("&unlabeled=true");
    }
    if let Some(id) = query.podcast_id {
        url.push_str(&format!("&podcast_id={id}"));
    }
    if query.oldest_first {
        url.push_str("&sort=oldest");
    }
    get(&url, api_key).await
}

pub async fn call_get_inbox_count(server: &str, api_key: &str) -> Result<i64, Error> {
    #[derive(Deserialize)]
    struct Count {
        count: i64,
    }
    Ok(get::<Count>(&format!("{server}/api/data/inbox/count"), api_key).await?.count)
}

pub async fn call_dismiss_inbox(server: &str, api_key: &str, episodes: &[EpisodeRef]) -> Result<u64, Error> {
    let body = serde_json::json!({ "episodes": episodes });
    Ok(send::<_, Changed>("POST", &format!("{server}/api/data/inbox/dismiss"), api_key, &body).await?.changed)
}

pub async fn call_restore_inbox(server: &str, api_key: &str, episodes: &[EpisodeRef]) -> Result<u64, Error> {
    let body = serde_json::json!({ "episodes": episodes });
    Ok(send::<_, Changed>("POST", &format!("{server}/api/data/inbox/restore"), api_key, &body).await?.changed)
}

pub async fn call_clear_inbox(server: &str, api_key: &str, query: &InboxQuery) -> Result<u64, Error> {
    let body = serde_json::json!({
        "podcast_id": query.podcast_id,
        "label_id": query.label_id,
        "unlabeled": query.label_id.is_none() && query.unlabeled,
    });
    Ok(send::<_, Changed>("POST", &format!("{server}/api/data/inbox/clear"), api_key, &body).await?.changed)
}

#[derive(Serialize, Deserialize)]
struct InboxSettings {
    days: i32,
}

pub async fn call_get_inbox_days(server: &str, api_key: &str) -> Result<i32, Error> {
    Ok(get::<InboxSettings>(&format!("{server}/api/data/inbox/settings"), api_key).await?.days)
}

pub async fn call_set_inbox_days(server: &str, api_key: &str, days: i32) -> Result<i32, Error> {
    Ok(send::<_, InboxSettings>("PUT", &format!("{server}/api/data/inbox/settings"), api_key, &InboxSettings { days }).await?.days)
}

#[derive(Serialize, Deserialize)]
struct PodcastInbox {
    inbox: bool,
}

pub async fn call_get_podcast_inbox(server: &str, api_key: &str, podcast_id: i32) -> Result<bool, Error> {
    Ok(get::<PodcastInbox>(&format!("{server}/api/data/podcast/{podcast_id}/inbox"), api_key).await?.inbox)
}

pub async fn call_set_podcast_inbox(server: &str, api_key: &str, podcast_id: i32, inbox: bool) -> Result<(), Error> {
    send::<_, PodcastInbox>("PUT", &format!("{server}/api/data/podcast/{podcast_id}/inbox"), api_key, &PodcastInbox { inbox }).await?;
    Ok(())
}

pub async fn call_get_labels(server: &str, api_key: &str) -> Result<Vec<EpisodeLabel>, Error> {
    get(&format!("{server}/api/data/labels"), api_key).await
}

pub async fn call_get_label_assignments(server: &str, api_key: &str) -> Result<Vec<LabelAssignment>, Error> {
    get(&format!("{server}/api/data/labels/assignments"), api_key).await
}

pub async fn call_create_label(server: &str, api_key: &str, name: &str, color: &str) -> Result<EpisodeLabel, Error> {
    let body = serde_json::json!({ "name": name, "color": color });
    send("POST", &format!("{server}/api/data/labels"), api_key, &body).await
}

pub async fn call_update_label(server: &str, api_key: &str, label_id: i32, name: &str, color: &str) -> Result<(), Error> {
    let body = serde_json::json!({ "name": name, "color": color });
    send::<_, serde_json::Value>("PUT", &format!("{server}/api/data/labels/{label_id}"), api_key, &body).await?;
    Ok(())
}

pub async fn call_delete_label(server: &str, api_key: &str, label_id: i32) -> Result<(), Error> {
    let response = Request::delete(&format!("{server}/api/data/labels/{label_id}"))
        .header("Api-Key", api_key)
        .send()
        .await?;
    if response.ok() {
        Ok(())
    } else {
        Err(fail(response).await)
    }
}

pub async fn call_set_label_episodes(server: &str, api_key: &str, label_id: i32, episodes: &[EpisodeRef], assigned: bool) -> Result<u64, Error> {
    let body = serde_json::json!({ "episodes": episodes, "assigned": assigned });
    Ok(send::<_, Changed>("POST", &format!("{server}/api/data/labels/{label_id}/episodes"), api_key, &body).await?.changed)
}
