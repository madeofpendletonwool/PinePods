use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PodClip {
    pub clipid: i32,
    pub userid: i32,
    pub episodeid: Option<i32>,
    pub videoid: Option<i32>,
    pub cliptitle: String,
    pub starttime: f32,
    pub endtime: f32,
    pub clipduration: f32,
    pub cliplocation: String,
    pub clipdate: String,
    pub isyoutube: bool,
}

#[derive(Serialize)]
pub struct CreateClipRequest {
    pub user_id: i32,
    pub episode_id: Option<i32>,
    pub video_id: Option<i32>,
    pub clip_title: String,
    pub start_time: f32,
    pub end_time: f32,
    pub is_youtube: bool,
}

#[derive(Deserialize, Debug)]
pub struct ClipResponse {
    pub detail: String,
    pub clip_id: Option<i32>,
}

#[derive(Deserialize, Debug)]
pub struct UserClipsResponse {
    pub clips: Vec<PodClip>,
}

// Create a new clip
pub async fn call_create_clip(
    server_name: &str,
    api_key: &str,
    request: CreateClipRequest,
) -> Result<ClipResponse, anyhow::Error> {
    let url = format!("{}/api/data/create_clip", server_name);

    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&request)?
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to create clip: {}",
            response.status_text()
        )));
    }

    let clip_response: ClipResponse = response.json().await?;
    Ok(clip_response)
}

// Get all clips for a user
pub async fn call_get_user_clips(
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<Vec<PodClip>, anyhow::Error> {
    let url = format!("{}/api/data/user_clips/{}", server_name, user_id);

    let response = Request::get(&url)
        .header("Api-Key", api_key)
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to fetch clips: {}",
            response.status_text()
        )));
    }

    let clips_response: UserClipsResponse = response.json().await?;
    Ok(clips_response.clips)
}

// Delete a clip
pub async fn call_delete_clip(
    server_name: &str,
    api_key: &str,
    clip_id: i32,
    user_id: i32,
) -> Result<ClipResponse, anyhow::Error> {
    let url = format!("{}/api/data/delete_clip/{}", server_name, clip_id);

    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "user_id": user_id }))?
        .send()
        .await?;

    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to delete clip: {}",
            response.status_text()
        )));
    }

    let clip_response: ClipResponse = response.json().await?;
    Ok(clip_response)
}

// Get clip download URL
pub fn get_clip_download_url(server_name: &str, clip_id: i32, api_key: &str) -> String {
    format!(
        "{}/api/data/clips/{}/download?api_key={}",
        server_name, clip_id, api_key
    )
}
