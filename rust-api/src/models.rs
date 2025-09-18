use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

// Response models to match Python API
#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status_code: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            status_code: 200,
            message: None,
            data: Some(data),
        }
    }

    pub fn success_with_message(data: T, message: String) -> Self {
        Self {
            status_code: 200,
            message: Some(message),
            data: Some(data),
        }
    }

    pub fn error(status_code: u16, message: String) -> ApiResponse<()> {
        ApiResponse {
            status_code,
            message: Some(message),
            data: None,
        }
    }
}

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

// Authentication models
#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub status: String,
    pub user_id: Option<i32>,
    pub api_key: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiKeyValidationResponse {
    pub status: String,
}

// User models
#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    pub user_id: i32,
    pub username: String,
    pub email: Option<String>,
    pub is_admin: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserSettings {
    pub user_id: i32,
    pub theme: String,
    pub auto_download_episodes: bool,
    pub auto_delete_episodes: bool,
    pub download_location: Option<String>,
}

// Podcast models
#[derive(Debug, Serialize, Deserialize)]
pub struct Podcast {
    pub podcast_id: i32,
    pub podcast_name: String,
    pub feed_url: String,
    pub artwork_url: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub website_url: Option<String>,
    pub explicit: bool,
    pub episode_count: i32,
    pub categories: Option<String>,
    pub user_id: i32,
    pub auto_download: bool,
    pub date_created: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Episode {
    pub episode_id: i32,
    pub podcast_id: i32,
    pub episode_title: String,
    pub episode_description: Option<String>,
    pub episode_url: Option<String>,
    pub episode_artwork: Option<String>,
    pub episode_pub_date: DateTime<Utc>,
    pub episode_duration: i32,
    pub completed: bool,
    pub listen_duration: i32,
    pub downloaded: bool,
    pub saved: bool,
}

// Playlist models
#[derive(Debug, Serialize, Deserialize)]
pub struct Playlist {
    pub playlist_id: i32,
    pub user_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_system_playlist: bool,
    pub episode_count: i32,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

// Request models
#[derive(Debug, Deserialize)]
pub struct CreatePodcastRequest {
    pub feed_url: String,
    pub auto_download: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateEpisodeRequest {
    pub listen_duration: Option<i32>,
    pub completed: Option<bool>,
    pub saved: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub user_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub podcast_ids: Option<Vec<i32>>,
    pub include_unplayed: bool,
    pub include_partially_played: bool,
    pub include_played: bool,
    pub play_progress_min: Option<f32>,
    pub play_progress_max: Option<f32>,
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

// Search models
#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    pub search_type: Option<String>, // "podcasts", "episodes", "all"
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub podcasts: Vec<Podcast>,
    pub episodes: Vec<Episode>,
    pub total_count: i32,
}

// Statistics models
#[derive(Debug, Serialize, Deserialize)]
pub struct UserStats {
    pub total_podcasts: i32,
    pub total_episodes: i32,
    pub total_listen_time: i32,
    pub completed_episodes: i32,
    pub saved_episodes: i32,
    pub downloaded_episodes: i32,
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
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SavedEpisodesResponse {
    pub saved_episodes: Vec<SavedEpisode>,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct UserHistoryResponse {
    pub data: Vec<HistoryEpisode>,
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

// Background task models
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub task_id: String,
    pub status: String,
    pub progress: Option<f32>,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

// Import/Export models
#[derive(Debug, Serialize, Deserialize)]
pub struct OpmlImportRequest {
    pub opml_content: String,
    pub auto_download: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportProgress {
    pub total_feeds: i32,
    pub processed_feeds: i32,
    pub successful_imports: i32,
    pub failed_imports: i32,
    pub current_feed: Option<String>,
}

// Pagination models
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: Option<i32>,
    pub per_page: Option<i32>,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: Some(1),
            per_page: Some(50),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total_count: i32,
    pub page: i32,
    pub per_page: i32,
    pub total_pages: i32,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total_count: i32, page: i32, per_page: i32) -> Self {
        let total_pages = (total_count + per_page - 1) / per_page; // Ceiling division
        Self {
            data,
            total_count,
            page,
            per_page,
            total_pages,
        }
    }
}