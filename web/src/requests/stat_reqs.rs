use anyhow::Error;
use gloo_net::http::Request;
use serde::Deserialize;
use serde::Serialize;

#[derive(Deserialize, Debug, PartialEq, Clone)]
#[allow(non_snake_case)]
pub struct UserStats {
    pub UserCreated: String,
    pub PodcastsPlayed: i32,
    pub TimeListened: i32,
    pub PodcastsAdded: i32,
    pub EpisodesSaved: i32,
    pub EpisodesDownloaded: i32,
    pub GpodderUrl: String,
    pub Pod_Sync_Type: String,
    // Newer servers also describe the current sync setup.
    #[serde(default)]
    pub SyncServerEnabled: Option<bool>,
    #[serde(default)]
    pub SyncServerUrl: Option<String>,
    #[serde(default)]
    pub SyncTargetKind: Option<String>,
    #[serde(default)]
    pub SyncTargetUrl: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct TopPodcastStat {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: String,
    pub total_seconds: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct CategoryStat {
    pub name: String,
    pub total_seconds: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct LongestEpisodeStat {
    pub episodeid: i32,
    pub episodetitle: String,
    pub episodeduration: i32,
    pub podcastname: String,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct ExtendedUserStats {
    pub top_podcasts: Vec<TopPodcastStat>,
    pub favorite_categories: Vec<CategoryStat>,
    pub completion_rate: i32,
    pub longest_episode: Option<LongestEpisodeStat>,
    pub listening_badge: String,
    pub total_downloaded_bytes: i64,
    pub total_downloaded_formatted: String,
    pub current_streak: i32,
    pub listening_by_dow: Vec<i64>,
    // Newer servers also send these; older ones leave them at their defaults.
    #[serde(default)]
    pub listening_by_hour: Vec<i64>,
    #[serde(default)]
    pub listening_by_month: Vec<MonthStat>,
    #[serde(default)]
    pub daily_listening: Vec<DayStat>,
    #[serde(default)]
    pub active_days: i64,
    #[serde(default)]
    pub longest_streak: i32,
    #[serde(default)]
    pub year_seconds: i64,
    #[serde(default)]
    pub peak_hour: Option<i32>,
    #[serde(default)]
    pub podcasts_listened: i64,
    #[serde(default)]
    pub backlog_episodes: i64,
    #[serde(default)]
    pub backlog_seconds: i64,
    #[serde(default)]
    pub episodes_completed: Option<i64>,
    #[serde(default)]
    pub episodes_listened: Option<i64>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub time_saved: Option<TimeSaved>,
}

/// Seconds saved by playback speed (negative when slower than 1x) and by skipping.
#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Default)]
pub struct TimeSaved {
    pub speed: i64,
    pub silence: i64,
    pub ads: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct MonthStat {
    /// `YYYY-MM`
    pub month: String,
    pub seconds: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone)]
pub struct DayStat {
    /// `YYYY-MM-DD` in the user's timezone
    pub date: String,
    pub seconds: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Default)]
#[serde(default)]
pub struct ReportPodcast {
    pub podcast_id: Option<i64>,
    pub name: String,
    pub artwork: String,
    pub seconds: i64,
    pub episodes: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Default)]
#[serde(default)]
pub struct ReportEpisode {
    pub episode_id: Option<i64>,
    pub is_youtube: bool,
    pub title: String,
    pub podcast: String,
    pub artwork: String,
    pub seconds: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Default)]
#[serde(default)]
pub struct ReportPerson {
    pub person_id: i64,
    pub name: String,
    pub image: Option<String>,
    pub seconds: i64,
    pub episodes: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Default)]
#[serde(default)]
pub struct NamedSeconds {
    pub name: String,
    pub seconds: i64,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Default)]
#[serde(default)]
pub struct SessionStats {
    pub count: i64,
    pub average_seconds: i64,
    pub longest_seconds: i64,
    pub longest_started_at: Option<i64>,
}

#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Default)]
#[serde(default)]
pub struct ReleaseLag {
    pub median_days: f64,
    pub within_day: f64,
    pub within_week: f64,
    pub episodes: i64,
}

/// `GET /api/data/listening_report`: a period's (or all time's) listening.
#[derive(Deserialize, Serialize, Debug, PartialEq, Clone, Default)]
#[serde(default)]
pub struct ListeningReport {
    pub start: Option<String>,
    pub end: Option<String>,
    pub timezone: String,
    pub recorded_since: Option<String>,
    pub months: Vec<String>,
    pub seconds: i64,
    pub active_days: i64,
    pub episodes: i64,
    pub podcasts: i64,
    pub finished: i64,
    pub top_podcasts: Vec<ReportPodcast>,
    pub top_episodes: Vec<ReportEpisode>,
    pub top_categories: Vec<NamedSeconds>,
    pub top_people: Vec<ReportPerson>,
    pub by_hour: Vec<i64>,
    pub by_dow: Vec<i64>,
    pub daily: Vec<DayStat>,
    pub by_month: Vec<MonthStat>,
    pub biggest_day: Option<DayStat>,
    pub busiest_week: Option<DayStat>,
    pub longest_streak: i64,
    pub sessions: SessionStats,
    pub average_speed: Option<f64>,
    pub time_saved: TimeSaved,
    pub ad_heavy: Vec<ReportPodcast>,
    pub by_client: Vec<NamedSeconds>,
    pub downloaded_seconds: i64,
    pub release_lag: Option<ReleaseLag>,
}

/// The listening report for local days `start..=end`, or all time.
pub async fn call_get_listening_report(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    range: Option<(String, String)>,
) -> Result<ListeningReport, anyhow::Error> {
    let mut url = format!("{server_name}/api/data/listening_report?user_id={user_id}");
    if let Some((start, end)) = range {
        url.push_str(&format!("&start={start}&end={end}"));
    }
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!("Failed to get the listening report: {}", response.status())));
    }
    Ok(response.json::<ListeningReport>().await?)
}

#[allow(dead_code)]
pub async fn call_get_extended_stats(
    server_name: String,
    api_key: Option<String>,
    user_id: &i32,
) -> Result<ExtendedUserStats, anyhow::Error> {
    let url = format!("{}/api/data/get_extended_stats?user_id={}", server_name, user_id);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to get extended stats: {}",
            response.status_text()
        )));
    }

    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to get response text".to_string());

    match serde_json::from_str::<ExtendedUserStats>(&response_text) {
        Ok(body) => Ok(body),
        Err(_) => Err(anyhow::Error::msg("Failed to deserialize extended stats")),
    }
}

#[allow(dead_code)]
pub async fn call_get_stats(
    server_name: String,
    api_key: Option<String>,
    user_id: &i32,
) -> Result<UserStats, anyhow::Error> {
    let url = format!("{}/api/data/get_stats?user_id={}", server_name, user_id);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| Error::msg("API key is missing"))?;

    let response = Request::get(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .send()
        .await?;
    if !response.ok() {
        return Err(anyhow::Error::msg(format!(
            "Failed to get stats: {}",
            response.status_text()
        )));
    }

    // First, capture the response text for diagnostic purposes
    let response_text = response
        .text()
        .await
        .unwrap_or_else(|_| "Failed to get response text".to_string());

    // Try to deserialize the response text
    match serde_json::from_str::<UserStats>(&response_text) {
        Ok(response_body) => Ok(response_body),
        Err(_e) => Err(anyhow::Error::msg("Failed to deserialize response")),
    }
}
