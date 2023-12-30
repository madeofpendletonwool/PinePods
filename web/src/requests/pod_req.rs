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
    pub episodes: Option<Vec<Episode>>,
}

pub async fn call_get_recent_eps(server_name: &String, api_key: &Option<String>, user_id: i32) -> Result<Vec<Episode>, anyhow::Error> {
    let url = format!("{}/api/data/return_pods/{}", server_name, user_id);

    console::log_1(&format!("URL: {}", url).into());

    // Convert Option<String> to Option<&str>
    let api_key_ref = api_key.as_deref().ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .send()
        .await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to fetch episodes: {}", response.status_text())));
    }

    let response_body = response.json::<RecentEps>().await?;
    Ok(response_body.episodes.unwrap_or_else(Vec::new))
}
