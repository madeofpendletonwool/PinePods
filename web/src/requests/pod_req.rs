use gloo_net::http::Request;
use serde::Deserialize;
use web_sys::console;

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

pub async fn call_get_recent_eps(server_name: Option<String>, api_key: Option<String>, user_id: i32) -> Result<RecentEps, anyhow::Error> {
    // Check if server_name is Some and unwrap, otherwise return an error
    let server = server_name.ok_or_else(|| anyhow::Error::msg("Server name is missing"))?;
    let url = format!("{}/api/data/return_pods/{}", server, user_id);

    console::log_1(&format!("URL: {}", url).into());
    // Convert Option<String> to Option<&str> and handle the None case
    let api_key_ref = api_key.as_ref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;


    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;

    if response.ok() {
        let recent_eps: RecentEps = response.json().await?;
        Ok(recent_eps)
    } else {
        Err(anyhow::Error::msg("Failed to get recent episodes"))
    }
}