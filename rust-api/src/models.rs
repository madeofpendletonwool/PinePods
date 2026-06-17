use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// PinePods check response
#[derive(Debug, Serialize, Deserialize)]
pub struct PinepodsCheckResponse {
    pub status_code: u16,
    pub pinepods_instance: bool,
}

// Health check response
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub database: bool,
    pub redis: bool,
    pub timestamp: DateTime<Utc>,
}

// Request models
#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub user_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub podcast_ids: Option<Vec<i32>>,
    pub include_unplayed: bool,
    pub include_partially_played: bool,
    pub include_played: bool,
    pub play_progress_min: Option<f64>,
    pub play_progress_max: Option<f64>,
    pub time_filter_hours: Option<i32>,
    pub min_duration: Option<i32>,
    pub max_duration: Option<i32>,
    pub sort_order: String,
    pub group_by_podcast: bool,
    pub max_episodes: Option<i32>,
    pub icon_name: String,
}

#[derive(Debug, Serialize)]
pub struct CreatePlaylistResponse {
    pub detail: String,
    pub playlist_id: i32,
}

#[derive(Debug, Deserialize)]
pub struct DeletePlaylistRequest {
    pub user_id: i32,
    pub playlist_id: i32,
}

#[derive(Debug, Serialize)]
pub struct DeletePlaylistResponse {
    pub detail: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlaylistRequest {
    pub user_id: i32,
    pub playlist_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub podcast_ids: Option<Vec<i32>>,
    pub include_unplayed: bool,
    pub include_partially_played: bool,
    pub include_played: bool,
    pub play_progress_min: Option<f64>,
    pub play_progress_max: Option<f64>,
    pub time_filter_hours: Option<i32>,
    pub min_duration: Option<i32>,
    pub max_duration: Option<i32>,
    pub sort_order: String,
    pub group_by_podcast: bool,
    pub max_episodes: Option<i32>,
    pub icon_name: String,
}

#[derive(Debug, Serialize)]
pub struct UpdatePlaylistResponse {
    pub detail: String,
}

// Language models
#[derive(Debug, Serialize, Deserialize)]
pub struct AvailableLanguage {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LanguageUpdateRequest {
    pub user_id: i32,
    pub language: String,
}

#[derive(Debug, Serialize)]
pub struct UserLanguageResponse {
    pub language: String,
}

#[derive(Debug, Serialize)]
pub struct AvailableLanguagesResponse {
    pub languages: Vec<AvailableLanguage>,
}

// API-specific podcast models to match Python responses
#[derive(Debug, Serialize, Deserialize)]
pub struct PodcastResponse {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: Option<String>,
    pub description: Option<String>,
    pub episodecount: Option<i32>,
    pub websiteurl: Option<String>,
    pub feedurl: String,
    pub author: Option<String>,
    pub categories: Option<std::collections::HashMap<String, String>>,
    pub explicit: bool,
    pub podcastindexid: Option<i64>,
    pub is_favorite: bool,
    pub is_video: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PodcastExtraResponse {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: Option<String>,
    pub description: Option<String>,
    pub episodecount: Option<i32>,
    pub websiteurl: Option<String>,
    pub feedurl: String,
    pub author: Option<String>,
    pub categories: Option<std::collections::HashMap<String, String>>,
    pub explicit: bool,
    pub podcastindexid: Option<i64>,
    pub play_count: i64,
    pub episodes_played: i32,
    pub oldest_episode_date: Option<String>,
    pub is_youtube: bool,
    pub is_video: bool,
    pub is_favorite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PodcastListResponse {
    pub pods: Vec<PodcastResponse>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PodcastExtraListResponse {
    pub pods: Vec<PodcastExtraResponse>,
}

// Remove podcast request model
#[derive(Debug, Deserialize)]
pub struct RemovePodcastByNameRequest {
    pub user_id: i32,
    pub podcast_name: String,
    pub podcast_url: String,
}

// Time info response model
#[derive(Debug, Serialize, Deserialize)]
pub struct TimeInfoResponse {
    pub timezone: String,
    pub hour_pref: i32,
    pub date_format: Option<String>,
}

// Check podcast response model  
#[derive(Debug, Serialize, Deserialize)]
pub struct CheckPodcastResponse {
    pub exists: bool,
}

// Check episode in database response model
#[derive(Debug, Serialize, Deserialize)]
pub struct EpisodeInDbResponse {
    pub episode_in_db: bool,
}

// Queue-related models
#[derive(Debug, Deserialize)]
pub struct QueuePodcastRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub is_youtube: bool,
}

#[derive(Debug, Deserialize)]
pub struct ClearQueueRequest {
    pub user_id: i32,
}

// Saved episodes models
#[derive(Debug, Deserialize)]
pub struct SavePodcastRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub is_youtube: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedEpisode {
    pub episodetitle: String,
    pub podcastname: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub websiteurl: String,
    pub completed: bool,
    pub saved: bool,
    pub queued: bool,
    pub downloaded: bool,
    pub is_youtube: bool,
    pub podcastid: Option<i32>,
    pub savedate: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedEpisodesResponse {
    pub saved_episodes: Vec<SavedEpisode>,
    pub total: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaylistInfo {
    pub name: String,
    pub description: String,
    pub episode_count: i32,
    pub icon_name: String,
    pub is_system_playlist: bool,
    pub podcast_ids: Option<Vec<i32>>,
    pub include_unplayed: bool,
    pub include_partially_played: bool,
    pub include_played: bool,
    pub min_duration: Option<i32>,
    pub max_duration: Option<i32>,
    pub sort_order: String,
    pub group_by_podcast: bool,
    pub max_episodes: Option<i32>,
    pub play_progress_min: Option<f64>,
    pub play_progress_max: Option<f64>,
    pub time_filter_hours: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PlaylistEpisodesResponse {
    pub episodes: Vec<SavedEpisode>,
    pub playlist_info: PlaylistInfo,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct SaveEpisodeResponse {
    pub detail: String,
}

// History models
#[derive(Debug, Deserialize)]
pub struct HistoryAddRequest {
    pub episode_id: i32,
    pub episode_pos: f32,
    pub user_id: i32,
    pub is_youtube: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryEpisode {
    pub episodetitle: String,
    pub podcastname: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub episodeduration: i32,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub completed: bool,
    pub listendate: Option<String>,
    pub is_youtube: bool,
}

#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueResponse {
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueuedEpisode {
    pub episodetitle: String,
    pub podcastname: String,
    pub episodepubdate: String,
    pub episodedescription: String,
    pub episodeartwork: String,
    pub episodeurl: String,
    pub queueposition: Option<i32>,
    pub episodeduration: i32,
    pub queuedate: String,
    pub listenduration: Option<i32>,
    pub episodeid: i32,
    pub completed: bool,
    pub saved: bool,
    pub queued: bool,
    pub downloaded: bool,
    pub is_youtube: bool,
    pub is_video: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueuedEpisodesResponse {
    pub data: Vec<QueuedEpisode>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderQueueRequest {
    pub episode_ids: Vec<i32>,
}

#[derive(Debug, Serialize)]
pub struct ReorderQueueResponse {
    pub message: String,
}

// Bulk episode action models - flexible episode ID lists
#[derive(Debug, Deserialize)]
pub struct BulkEpisodeActionRequest {
    pub episode_ids: Vec<i32>,
    pub user_id: i32,
    pub is_youtube: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct BulkEpisodeActionResponse {
    pub message: String,
    pub processed_count: i32,
    pub failed_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CustomTheme {
    pub themeid: i32,
    pub name: String,
    pub background_color: String,
    pub button_color: String,
    pub container_button_color: String,
    pub button_text_color: String,
    pub text_color: String,
    pub text_secondary_color: String,
    pub border_color: String,
    pub accent_color: String,
    pub prog_bar_color: String,
    pub error_color: String,
    pub bonus_color: String,
    pub secondary_background: String,
    pub container_background: String,
    pub standout_color: String,
    pub hover_color: String,
    pub link_color: String,
    pub thumb_color: String,
    pub unfilled_color: String,
    pub check_box_color: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateCustomThemeRequest {
    pub user_id: i32,
    pub name: String,
    pub background_color: String,
    pub button_color: String,
    pub container_button_color: String,
    pub button_text_color: String,
    pub text_color: String,
    pub text_secondary_color: String,
    pub border_color: String,
    pub accent_color: String,
    pub prog_bar_color: String,
    pub error_color: String,
    pub bonus_color: String,
    pub secondary_background: String,
    pub container_background: String,
    pub standout_color: String,
    pub hover_color: String,
    pub link_color: String,
    pub thumb_color: String,
    pub unfilled_color: String,
    pub check_box_color: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteCustomThemeRequest {
    pub user_id: i32,
    pub theme_id: i32,
}