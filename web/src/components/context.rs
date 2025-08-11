use crate::components::audio::AudioPlayerProps;
use crate::components::notification_center::TaskProgress;
use crate::components::podcast_layout::ClickedFeedURL;
use crate::components::podcasts::PodcastLayout;
use crate::requests::login_requests::AddUserRequest;
use crate::requests::login_requests::GetUserDetails;
use crate::requests::login_requests::LoginServerRequest;
use crate::requests::login_requests::{GetApiDetails, TimeZoneInfo};
use crate::requests::pod_req::PodcastResponseExtra;
use crate::requests::pod_req::{
    Chapter, Episode, EpisodeDownloadResponse, EpisodeMetadataResponse, Funding,
    HistoryDataResponse, HomeOverview, Person, Playlist, PlaylistInfo, Podcast, PodcastResponse,
    PodrollItem, QueuedEpisodesResponse, RecentEps, RefreshProgress, SavedEpisodesResponse,
    SharedEpisodeResponse, Transcript, Value,
};
use crate::components::setting_components::firewood_players::{FirewoodServer, FirewoodPlaybackStatus};
use std::collections::HashMap;
use crate::requests::search_pods::{
    PeopleFeedResult, PodcastFeedResult, PodcastSearchResult, SearchResponse, YouTubeChannel,
    YouTubeSearchResults,
};
use crate::requests::setting_reqs::{AddSettingsUserRequest, EditSettingsUserRequest};
use crate::requests::stat_reqs::UserStats;
use serde::Deserialize;
use serde_json::{from_str, json};
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::HtmlAudioElement;
use yewdux::prelude::*;

#[allow(dead_code)]
#[allow(dead_code)]
pub enum AppStateMsg {
    ExpandEpisode(String),
    CollapseEpisode(String),
    SetLoading(bool),
    UpdateSelectedEpisodesForDeletion(i32), // Add this line
    DeleteSelectedEpisodes,                 // Add this line
}

impl Reducer<AppState> for AppStateMsg {
    fn apply(self, state: Rc<AppState>) -> Rc<AppState> {
        let mut load_state = state.clone();
        let state_mut = Rc::make_mut(&mut load_state);

        match self {
            AppStateMsg::ExpandEpisode(guid) => {
                state_mut.expanded_descriptions.insert(guid);
            }
            AppStateMsg::CollapseEpisode(guid) => {
                state_mut.expanded_descriptions.remove(&guid);
            }
            AppStateMsg::SetLoading(is_loading) => {
                state_mut.is_loading = Option::from(is_loading);
            }
            AppStateMsg::UpdateSelectedEpisodesForDeletion(episode_id) => {
                // Add this block
                state_mut.selected_episodes_for_deletion.insert(episode_id);
            }
            AppStateMsg::DeleteSelectedEpisodes => {
                state_mut.selected_episodes_for_deletion.clear();
            }
        }

        state
    }
}

#[derive(Default, PartialEq, Clone, Store)]
pub struct ExpandedDescriptions {
    pub expanded_descriptions: HashSet<String>,
}

#[derive(Default, Clone, PartialEq, Store)]
pub struct PlaylistState {
    pub include_unplayed: bool,
    pub include_partially_played: bool,
    pub include_played: bool,
    pub name: String,
    pub description: String,
    pub min_duration: String,
    pub max_duration: String,
    pub sort_order: String,
    pub group_by_podcast: bool,
    pub max_episodes: String,
    pub icon_name: String,
    pub play_progress_min: String,
    pub play_progress_max: String,
    pub time_filter_hours: String,
}

#[derive(Default, Deserialize, Clone, PartialEq, Store, Debug)]
pub struct AppState {
    pub user_details: Option<GetUserDetails>,
    pub auth_details: Option<LoginServerRequest>,
    pub server_details: Option<GetApiDetails>,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub search_results: Option<PodcastSearchResult>,
    pub podcast_feed_results: Option<PodcastFeedResult>,
    pub people_feed_results: Option<PeopleFeedResult>,
    pub server_feed_results: Option<RecentEps>,
    pub queued_episodes: Option<QueuedEpisodesResponse>,
    pub saved_episodes: Option<SavedEpisodesResponse>,
    pub episode_history: Option<HistoryDataResponse>,
    pub downloaded_episodes: Option<EpisodeDownloadResponse>,
    pub search_episodes: Option<SearchResponse>,
    pub episodes: Option<Episode>,
    pub clicked_podcast_info: Option<ClickedFeedURL>,
    pub pods: Option<Podcast>,
    pub podcast_feed_return: Option<PodcastResponse>,
    pub podcast_feed_return_extra: Option<PodcastResponseExtra>,
    pub is_loading: Option<bool>,
    pub is_refreshing: Option<bool>,
    pub gravatar_url: Option<String>,
    #[serde(default)]
    pub expanded_descriptions: HashSet<String>,
    pub selected_theme: Option<String>,
    pub fetched_episode: Option<EpisodeMetadataResponse>,
    pub shared_fetched_episode: Option<SharedEpisodeResponse>,
    pub selected_episode_id: Option<i32>,
    pub selected_episode_url: Option<String>,
    pub selected_episode_audio_url: Option<String>,
    pub selected_podcast_title: Option<String>,
    pub person_episode: Option<bool>,
    pub selected_is_youtube: Option<bool>,
    pub add_user_request: Option<AddUserRequest>,
    pub time_zone_setup: Option<TimeZoneInfo>,
    pub add_settings_user_reqeust: Option<AddSettingsUserRequest>,
    pub edit_settings_user_reqeust: Option<EditSettingsUserRequest>,
    #[serde(default)]
    pub selected_episodes_for_deletion: HashSet<i32>,
    pub reload_occured: Option<bool>,
    pub user_tz: Option<String>,
    pub hour_preference: Option<i16>,
    pub date_format: Option<String>,
    pub podcast_added: Option<bool>,
    pub completed_episodes: Option<Vec<i32>>,
    pub saved_episode_ids: Option<Vec<i32>>,
    pub queued_episode_ids: Option<Vec<i32>>,
    pub downloaded_episode_ids: Option<Vec<i32>>,
    pub locally_downloaded_episodes: Option<Vec<i32>>,
    pub podcast_layout: Option<PodcastLayout>,
    pub refresh_progress: Option<RefreshProgress>,
    pub youtube_search_results: Option<YouTubeSearchResults>,
    pub selected_youtube_channel: Option<YouTubeChannel>,
    pub is_youtube_loading: Option<bool>,
    pub show_transcript_modal: Option<bool>,
    pub current_transcripts: Option<Vec<Transcript>>,
    pub home_overview: Option<HomeOverview>,
    pub playlists: Option<Vec<Playlist>>,
    pub current_playlist_info: Option<PlaylistInfo>,
    pub current_playlist_episodes: Option<Vec<Episode>>,
    pub active_tasks: Option<Vec<TaskProgress>>,
    pub firewood_servers: Option<Vec<FirewoodServer>>,
    // Firewood status tracking - maps server_id to (server_address, playback_status)
    pub firewood_status: Option<HashMap<i32, (String, FirewoodPlaybackStatus)>>,
    pub active_firewood_server: Option<i32>, // Currently controlled Firewood server ID
}

#[derive(Default, Deserialize, Clone, PartialEq, Store, Debug)]
pub struct UserStatsStore {
    pub stats: Option<UserStats>,
    pub pinepods_version: Option<String>,
}

#[derive(Default, Deserialize, Clone, PartialEq, Store, Debug)]
pub struct SettingsState {
    pub active_tab: Option<String>,
}

#[derive(Default, Clone, PartialEq, Store, Debug)]
pub struct UIState {
    pub audio_playing: Option<bool>,
    pub currently_playing: Option<AudioPlayerProps>,
    pub audio_element: Option<HtmlAudioElement>,
    pub current_time_seconds: f64,
    pub current_time_formatted: String,
    pub duration: f64,
    pub duration_formatted: String,
    // pub error_message: Option<String>,
    // pub info_message: Option<String>,
    pub is_expanded: bool,
    pub episode_in_db: Option<bool>,
    pub playback_speed: f64,
    pub audio_volume: f64,
    pub start_skip_sec: f64,
    pub end_skip_sec: f64,
    pub offline: Option<bool>,
    pub app_offline_mode: Option<bool>,
    pub local_download_increment: Option<i32>,
    pub episode_chapters: Option<Vec<Chapter>>,
    pub current_chapter_index: Option<usize>,
    pub podcast_people: Option<Vec<Person>>,
    pub episode_people: Option<Vec<Person>>,
    pub episode_transcript: Option<Vec<Transcript>>,
    pub episode_page_people: Option<Vec<Person>>,
    pub episode_page_transcript: Option<Vec<Transcript>>,
    pub podcast_funding: Option<Vec<Funding>>,
    pub podcast_podroll: Option<Vec<PodrollItem>>,
    pub podcast_value4value: Option<Vec<Value>>,
    pub is_mobile: Option<bool>,
}

impl UIState {
    pub fn update_current_time(&mut self, new_time_seconds: f64) {
        self.current_time_seconds = new_time_seconds;

        // Calculate formatted time
        let hours = (new_time_seconds / 3600.0).floor() as i32;
        let minutes = ((new_time_seconds % 3600.0) / 60.0).floor() as i32;
        let seconds = (new_time_seconds % 60.0).floor() as i32;
        self.current_time_formatted = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
    }
    pub fn toggle_playback(&mut self) {
        if let Some(audio) = &self.audio_element {
            if self.audio_playing.unwrap_or(false) {
                let _ = audio.pause();
                self.audio_playing = Some(false);
            } else {
                let _ = audio.play();
                self.audio_playing = Some(true);
            }
        }
    }

    pub fn set_audio_source(&mut self, src: String) {
        if self.audio_element.is_none() {
            self.audio_element = HtmlAudioElement::new().ok();
            if let Some(audio) = &self.audio_element {
                let closure = Closure::wrap(Box::new(move || {
                    // Code to handle the audio being ready to play
                }) as Box<dyn Fn()>);
                audio
                    .add_event_listener_with_callback("canplay", closure.as_ref().unchecked_ref())
                    .unwrap();
                closure.forget(); // Prevents the closure from being garbage collected
            }
        }
        if let Some(audio) = &self.audio_element {
            audio.set_src(&src);
        }
    }

    pub fn toggle_expanded(&mut self) {
        self.is_expanded = !self.is_expanded;
    }
}

impl AppState {
    pub fn deserialize(serialized_state: &str) -> Result<Self, serde_json::Error> {
        from_str(serialized_state)
    }

    pub fn store_app_state(&self) {
        if let Some(window) = window() {
            if let Some(local_storage) = window.local_storage().unwrap() {
                let user_key = "userState";
                let user_state = json!({ "user_details": self.user_details }).to_string();
                let auth_key = "userAuthState";
                let auth_state = json!({"auth_details": self.auth_details}).to_string();
                let server_key = "serverState";
                let server_state = json!({"server_details":self.server_details}).to_string();
                let firewood_key = "firewoodPlayers";
                // Firewood servers are now database-backed, no localStorage needed
                let _ = local_storage.set_item(user_key, &user_state);
                let _ = local_storage.set_item(auth_key, &auth_state);
                let _ = local_storage.set_item(server_key, &server_state);
                // Firewood localStorage removed - now database-backed
            }
        }
    }
    
    // Firewood servers are now database-backed - no localStorage loading needed
}

#[derive(Default, Clone, PartialEq, Store, Debug)]
pub struct FilterState {
    pub selected_category: Option<String>,
    pub category_filter_list: Option<Vec<String>>,
}

// Add this alongside your other state structs
#[derive(Default, Clone, PartialEq, Store)]
pub struct PodcastState {
    pub added_podcast_urls: HashSet<String>,
}
