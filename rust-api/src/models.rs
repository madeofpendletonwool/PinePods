use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use utoipa::ToSchema;

// PinePods check response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PinepodsCheckResponse {
    pub status_code: u16,
    pub pinepods_instance: bool,
}

// Health check response
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub database: bool,
    pub redis: bool,
    pub timestamp: DateTime<Utc>,
}

// Request models
#[derive(Debug, Deserialize, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
pub struct CreatePlaylistResponse {
    pub detail: String,
    pub playlist_id: i32,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeletePlaylistRequest {
    pub user_id: i32,
    pub playlist_id: i32,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeletePlaylistResponse {
    pub detail: String,
}

#[derive(Debug, Deserialize, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
pub struct UpdatePlaylistResponse {
    pub detail: String,
}

// Language models
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AvailableLanguage {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct LanguageUpdateRequest {
    pub user_id: i32,
    pub language: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UserLanguageResponse {
    pub language: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AvailableLanguagesResponse {
    pub languages: Vec<AvailableLanguage>,
}

// API-specific podcast models to match Python responses
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
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

// Lean per-subscription inputs for the recommendation taste profile (#103). Deliberately
// selects only columns that are guaranteed to exist on the Podcasts table, avoiding the
// heavier return_pods_extra query (which references a non-existent p.isyoutube column).
#[derive(Debug, Clone)]
pub struct RecommendationTasteInput {
    pub podcastname: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub categories: Option<std::collections::HashMap<String, String>>,
    pub podcastindexid: Option<i64>,
    pub feedurl: String,
    pub is_favorite: bool,
    pub play_count: i64,
}

// One recommended (not-yet-subscribed) podcast for the Discover page (#103). Built by
// services/recommendations.rs from PodcastIndex trending candidates ranked against the
// user's taste profile. `score` is the raw blended ranking score; `reason` is the
// human-facing explanation (e.g. "Because you listen to Technology").
#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
pub struct RecommendedPodcast {
    pub podcastindexid: Option<i64>,
    pub title: String,
    pub author: Option<String>,
    pub image: Option<String>,
    pub description: Option<String>,
    pub feedurl: Option<String>,
    pub categories: std::collections::HashMap<String, String>,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PodcastListResponse {
    pub pods: Vec<PodcastResponse>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PodcastExtraListResponse {
    pub pods: Vec<PodcastExtraResponse>,
}

// Time info response model
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TimeInfoResponse {
    pub timezone: String,
    pub hour_pref: i32,
    pub date_format: Option<String>,
}

// Check podcast response model  
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CheckPodcastResponse {
    pub exists: bool,
}

// Check episode in database response model
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct EpisodeInDbResponse {
    pub episode_in_db: bool,
}

// Queue-related models
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct QueuePodcastRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub is_youtube: bool,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ClearQueueRequest {
    pub user_id: i32,
}

// Saved episodes models
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct SavePodcastRequest {
    pub episode_id: i32,
    pub user_id: i32,
    pub is_youtube: bool,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SavedEpisodesResponse {
    pub saved_episodes: Vec<SavedEpisode>,
    pub total: i64,
}

// ---- Collections ----------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Collection {
    pub collection_id: i32,
    pub user_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub is_default: bool,
    pub icon: String,
    pub created_at: String,
    pub last_updated: String,
    pub episode_count: i64,
    /// Podcast categories whose episodes are auto-added to this collection (None = disabled).
    pub auto_add_categories: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct CollectionsResponse {
    pub collections: Vec<Collection>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CreateCollectionRequest {
    pub user_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    /// Podcast categories to auto-add episodes from (None/empty = disabled).
    pub auto_add_categories: Option<Vec<String>>,
    /// When true, immediately backfill existing matching episodes after saving.
    pub backfill: Option<bool>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CreateCollectionResponse {
    pub detail: String,
    pub collection_id: i32,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct UpdateCollectionRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    /// Podcast categories to auto-add episodes from (empty vec clears the rule).
    pub auto_add_categories: Option<Vec<String>>,
    /// When true, immediately backfill existing matching episodes after saving.
    pub backfill: Option<bool>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UserCategoriesResponse {
    pub categories: Vec<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CollectionDetailResponse {
    pub detail: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct CollectionEpisodeRequest {
    pub user_id: i32,
    pub episode_id: i32,
    pub is_youtube: bool,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct BulkAddCollectionRequest {
    pub user_id: i32,
    pub collection_id: i32,
    /// Each entry is (episode_id, is_youtube)
    pub episodes: Vec<(i32, bool)>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct EpisodeCollectionsResponse {
    pub collection_ids: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct PlaylistEpisodesResponse {
    pub episodes: Vec<SavedEpisode>,
    pub playlist_info: PlaylistInfo,
    pub total: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SaveEpisodeResponse {
    pub detail: String,
}

// History models
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct HistoryAddRequest {
    pub episode_id: i32,
    pub episode_pos: f32,
    pub user_id: i32,
    pub is_youtube: bool,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct HistoryResponse {
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct QueueResponse {
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct QueuedEpisode {
    pub episodetitle: String,
    pub podcastname: String,
    pub podcastid: i32,
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

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct QueuedEpisodesResponse {
    pub data: Vec<QueuedEpisode>,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ReorderQueueRequest {
    pub episode_ids: Vec<i32>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReorderQueueResponse {
    pub message: String,
}

// Bulk episode action models - flexible episode ID lists
#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct BulkEpisodeActionRequest {
    pub episode_ids: Vec<i32>,
    pub user_id: i32,
    pub is_youtube: Option<bool>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct BulkEpisodeActionResponse {
    pub message: String,
    pub processed_count: i32,
    pub failed_count: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema)]
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

#[derive(Debug, Deserialize, ToSchema)]
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteCustomThemeRequest {
    pub user_id: i32,
    pub theme_id: i32,
}