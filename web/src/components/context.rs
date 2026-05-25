use crate::components::audio::AudioPlayerProps;
use crate::components::notification_center::TaskProgress;
use crate::pages::podcast_layout::ClickedFeedURL;
use crate::pages::podcasts::PodcastLayout;
use crate::requests::episode::Episode;
use crate::requests::login_requests::AddUserRequest;
use crate::requests::login_requests::GetUserDetails;
use crate::requests::login_requests::LoginServerRequest;
use crate::requests::login_requests::{GetApiDetails, TimeZoneInfo};
use crate::requests::pod_req::PodcastResponseExtra;

use crate::requests::pod_req::{
    Chapter, EpisodeDownloadResponse, Funding, HistoryDataResponse, HomeOverview, Person, Playlist,
    PlaylistInfo, Podcast, PodcastResponse, PodrollItem, QueuedEpisodesResponse, RecentEps,
    RefreshProgress, SavedEpisodesResponse, SharedEpisodeResponse, Transcript, Value,
};
use crate::requests::search_pods::{
    PeopleFeedResult, PodcastFeedResult, PodcastSearchResult, SearchResponse, YouTubeChannel,
    YouTubeSearchResults,
};
use crate::requests::setting_reqs::{AddSettingsUserRequest, EditSettingsUserRequest};
use crate::requests::stat_reqs::{UserStats, ExtendedUserStats};
use serde::Deserialize;
use serde_json::{from_str, json};
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::window;
use web_sys::{HtmlAudioElement, HtmlVideoElement, HtmlMediaElement};
use yewdux::prelude::*;
use js_sys;

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
#[allow(dead_code)]
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
    pub search_results: Option<PodcastSearchResult>,
    pub podcast_feed_results: Option<PodcastFeedResult>,
    pub people_feed_results: Option<PeopleFeedResult>,
    pub server_feed_results: Option<RecentEps>,
    pub episode_history: Option<HistoryDataResponse>,
    pub search_episodes: Option<SearchResponse>,
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
    pub fetched_episode: Option<Episode>,
    pub shared_fetched_episode: Option<SharedEpisodeResponse>,
    pub selected_episode_id: Option<i32>,
    pub selected_episode_url: Option<String>,
    pub selected_episode_audio_url: Option<String>,
    pub selected_podcast_title: Option<String>,
    pub person_episode: Option<bool>,
    #[serde(default)]
    pub selected_is_youtube: bool,
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
    pub podcast_layout: Option<PodcastLayout>,
    pub youtube_search_results: Option<YouTubeSearchResults>,
    pub selected_youtube_channel: Option<YouTubeChannel>,
    pub is_youtube_loading: Option<bool>,
    pub show_transcript_modal: Option<bool>,
    pub current_transcripts: Option<Vec<Transcript>>,
    pub home_overview: Option<HomeOverview>,
    pub playlists: Option<Vec<Playlist>>,
    pub current_playlist_info: Option<PlaylistInfo>,
    pub current_playlist_episodes: Option<Vec<Episode>>,
}


/// Notification-only state, kept separate from AppState so that episode list pages
/// do not re-render when a toast fires.
#[derive(Default, Clone, PartialEq, Store, Debug)]
pub struct NotificationState {
    pub info_message: Option<String>,
    pub error_message: Option<String>,
    pub active_tasks: Option<Vec<TaskProgress>>,
    pub refresh_progress: Option<RefreshProgress>,
}

/// Episode-specific status state kept separate from AppState so that the
/// ~50+ components subscribing to AppState do NOT re-render on every
/// save/download/queue/complete action.
#[derive(Default, Deserialize, Clone, PartialEq, Store, Debug)]
pub struct EpisodeStatusState {
    #[serde(default)]
    pub saved_episodes: Vec<Episode>,
    #[serde(default)]
    pub downloaded_episodes: DownloadedEpisodeRecords,
    pub queued_episodes: Option<QueuedEpisodesResponse>,
    pub queued_episode_ids: Option<Vec<i32>>,
    pub completed_episodes: Option<Vec<i32>>,
}

impl EpisodeStatusState {
    pub fn saved_episode_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.saved_episodes.iter().map(|e| e.episodeid)
    }
}

/// A collection of records for episodes downloaded either locally or on the server.
/// Mutating this collection does not affect the filesystem and episodes will need
/// to be downloaded or deleted to match changes made here.
#[derive(Default, Deserialize, Clone, PartialEq, Debug)]
pub struct DownloadedEpisodeRecords {
    episodes: Vec<Episode>,
    local_ids: HashSet<i32>,
    server_ids: HashSet<i32>,
}

#[allow(dead_code)]
impl DownloadedEpisodeRecords {
    /// Creates an iterator of all downloaded &Episode
    pub fn episodes(&self) -> impl Iterator<Item = &Episode> + '_ {
        self.episodes.iter()
    }

    /// Creates an unordered iterator over ids for episodes downloaded locally
    pub fn local_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.local_ids.iter().map(|id| id.clone())
    }

    /// Creates an unordered iterator over ids for episodes downloaded to the server
    pub fn server_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.server_ids.iter().map(|id| id.clone())
    }

    /// Checks if episode is downloaded to the server
    pub fn is_server_download(&self, id: i32) -> bool {
        self.server_ids.contains(&id)
    }

    /// Checks if episode is downloaded to the server
    pub fn is_local_download(&self, id: i32) -> bool {
        self.local_ids.contains(&id)
    }

    /// Checks if episode is downloaded to either the server or locally
    pub fn is_download(&self, id: i32) -> bool {
        return self.is_local_download(id) || self.is_server_download(id);
    }

    /// Add a record of an Episode downloaded locally
    pub fn push_local(&mut self, episode: Episode) {
        let id = episode.episodeid;
        // only add Episode if the id doesn't exist in either set
        if !self.server_ids.contains(&episode.episodeid)
            && !self.local_ids.contains(&episode.episodeid)
        {
            self.episodes.push(episode);
        }

        self.local_ids.insert(id);
    }

    /// Add a record of an Episode downloaded to the server
    pub fn push_server(&mut self, episode: Episode) {
        let id = episode.episodeid;
        // only add Episode if the id doesn't exist in either set
        if !self.server_ids.contains(&episode.episodeid)
            && !self.local_ids.contains(&episode.episodeid)
        {
            self.episodes.push(episode);
        }

        self.server_ids.insert(id);
    }

    /// Remove the record of an Episode downloaded locally
    pub fn remove_local(&mut self, id: i32) {
        self.local_ids.remove(&id);

        // remove the ep if it isn't also downloaded on the server
        if !self.server_ids.contains(&id) {
            self.episodes.retain(|ep| ep.episodeid != id);
        }
    }

    /// Remove the record of an Episode downloaded on the server
    pub fn remove_server(&mut self, id: i32) {
        self.server_ids.remove(&id);

        // remove the ep if it isn't also downloaded locally
        if !self.local_ids.contains(&id) {
            self.episodes.retain(|ep| ep.episodeid != id);
        }
    }

    pub fn clear(&mut self) {
        self.episodes.clear();
        self.server_ids.clear();
        self.local_ids.clear();
    }

    /// Remove all records of episodes downloaded locally
    pub fn clear_local(&mut self) {
        for id in self.local_ids.drain() {
            if !self.server_ids.contains(&id) {
                self.episodes.retain(|ep| ep.episodeid != id);
            }
        }
    }

    /// Remove all records of episodes downloaded on the server
    pub fn clear_server(&mut self) {
        for id in self.local_ids.drain() {
            if !self.server_ids.contains(&id) {
                self.episodes.retain(|ep| ep.episodeid != id);
            }
        }
    }

    pub fn len(&self) -> usize {
        self.episodes.len()
    }
}

#[derive(Default, Deserialize, Clone, PartialEq, Store, Debug)]
pub struct UserStatsStore {
    pub stats: Option<UserStats>,
    pub extended_stats: Option<ExtendedUserStats>,
    pub pinepods_version: Option<String>,
}

#[derive(Default, Deserialize, Clone, PartialEq, Store, Debug)]
pub struct SettingsState {
    pub active_tab: Option<String>,
}

// MediaElement wrapper to handle both audio and video elements polymorphically
#[derive(Clone, PartialEq, Debug)]
pub enum MediaElement {
    Audio(HtmlAudioElement),
    Video(HtmlVideoElement),
}

impl MediaElement {
    pub fn as_media_element(&self) -> &HtmlMediaElement {
        match self {
            MediaElement::Audio(audio) => audio.unchecked_ref(),
            MediaElement::Video(video) => video.unchecked_ref(),
        }
    }

    pub fn current_time(&self) -> f64 {
        self.as_media_element().current_time()
    }

    pub fn set_current_time(&self, time: f64) {
        self.as_media_element().set_current_time(time);
    }

    pub fn duration(&self) -> f64 {
        self.as_media_element().duration()
    }

    pub fn pause(&self) -> Result<(), JsValue> {
        self.as_media_element().pause()
    }

    pub fn play(&self) -> Result<js_sys::Promise, JsValue> {
        self.as_media_element().play()
    }

    pub fn set_src(&self, src: &str) {
        self.as_media_element().set_src(src);
    }

    pub fn set_volume(&self, volume: f64) {
        self.as_media_element().set_volume(volume);
    }

    pub fn set_playback_rate(&self, rate: f64) {
        self.as_media_element().set_playback_rate(rate);
    }

    pub fn set_onended(&self, callback: Option<&js_sys::Function>) {
        self.as_media_element().set_onended(callback);
    }

    pub fn dispatch_event(&self, event: &web_sys::Event) -> Result<bool, JsValue> {
        self.as_media_element().dispatch_event(event)
    }

    pub fn add_event_listener_with_callback(
        &self,
        event: &str,
        callback: &js_sys::Function,
    ) -> Result<(), JsValue> {
        self.as_media_element()
            .add_event_listener_with_callback(event, callback)
    }
}

#[derive(Default, Clone, PartialEq, Store, Debug)]
pub struct UIState {
    pub audio_playing: Option<bool>,
    pub currently_playing: Option<AudioPlayerProps>,
    pub audio_element: Option<HtmlAudioElement>,
    pub media_element: Option<MediaElement>,
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
    pub queue_panel_open: bool,
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
        // Support both new media_element and legacy audio_element
        if let Some(media) = &self.media_element {
            if self.audio_playing.unwrap_or(false) {
                let _ = media.pause();
                self.audio_playing = Some(false);
            } else {
                let _ = media.play();
                self.audio_playing = Some(true);
            }
        } else if let Some(audio) = &self.audio_element {
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

    pub fn set_media_source(&mut self, src: String, is_video: bool, dispatch: Dispatch<UIState>) {
        if self.media_element.is_none() {
            self.media_element = if is_video {
                // Create video element using DOM API
                if let Some(window) = window() {
                    if let Some(document) = window.document() {
                        document.create_element("video")
                            .ok()
                            .and_then(|elem| elem.dyn_into::<HtmlVideoElement>().ok())
                            .map(MediaElement::Video)
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                HtmlAudioElement::new().ok().map(MediaElement::Audio)
            };

            if let Some(media) = &self.media_element {
                // Canplay event
                let canplay_closure = Closure::wrap(Box::new(move || {
                    // Code to handle the media being ready to play
                }) as Box<dyn Fn()>);
                let _ = media.add_event_listener_with_callback("canplay", canplay_closure.as_ref().unchecked_ref());
                canplay_closure.forget();

                // Play event - update state when media starts playing
                let play_dispatch = dispatch.clone();
                let play_closure = {
                    Closure::wrap(Box::new(move || {
                        play_dispatch.reduce_mut(|state| {
                            state.audio_playing = Some(true);
                        });
                    }) as Box<dyn Fn()>)
                };
                let _ = media.add_event_listener_with_callback("play", play_closure.as_ref().unchecked_ref());
                play_closure.forget();

                // Pause event - update state when media pauses
                let pause_dispatch = dispatch.clone();
                let pause_closure = {
                    Closure::wrap(Box::new(move || {
                        pause_dispatch.reduce_mut(|state| {
                            state.audio_playing = Some(false);
                        });
                    }) as Box<dyn Fn()>)
                };
                let _ = media.add_event_listener_with_callback("pause", pause_closure.as_ref().unchecked_ref());
                pause_closure.forget();
            }
        }

        if let Some(media) = &self.media_element {
            media.set_src(&src);
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
        let user_state = json!({ "user_details": self.user_details }).to_string();
        let auth_state = json!({"auth_details": self.auth_details}).to_string();
        let server_state = json!({"server_details":self.server_details}).to_string();

        // Try to use Tauri storage first (for desktop/Flatpak)
        if Self::is_tauri() {
            wasm_bindgen_futures::spawn_local(async move {
                use wasm_bindgen::prelude::*;

                #[wasm_bindgen]
                extern "C" {
                    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
                    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
                }

                let _ = invoke("store_credentials",
                    serde_wasm_bindgen::to_value(&serde_json::json!({
                        "key": "userState",
                        "value": user_state
                    })).unwrap()
                ).await;

                let _ = invoke("store_credentials",
                    serde_wasm_bindgen::to_value(&serde_json::json!({
                        "key": "userAuthState",
                        "value": auth_state
                    })).unwrap()
                ).await;

                let _ = invoke("store_credentials",
                    serde_wasm_bindgen::to_value(&serde_json::json!({
                        "key": "serverState",
                        "value": server_state
                    })).unwrap()
                ).await;
            });
        } else {
            // Fall back to localStorage for web version
            if let Some(window) = window() {
                if let Some(local_storage) = window.local_storage().unwrap() {
                    let _ = local_storage.set_item("userState", &user_state);
                    let _ = local_storage.set_item("userAuthState", &auth_state);
                    let _ = local_storage.set_item("serverState", &server_state);
                }
            }
        }
    }

    fn is_tauri() -> bool {
        if let Some(window) = window() {
            let result = js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"));
            result.is_ok() && !result.unwrap().is_undefined()
        } else {
            false
        }
    }
}

#[derive(Default, Clone, PartialEq, Store, Debug)]
#[allow(dead_code)]
pub struct FilterState {
    pub selected_category: Option<String>,
    pub category_filter_list: Option<Vec<String>>,
    pub favorites_only: bool,
}

// Add this alongside your other state structs
#[derive(Default, Clone, PartialEq, Store)]
#[allow(dead_code)]
pub struct PodcastState {
    pub added_podcast_urls: HashSet<String>,
}
