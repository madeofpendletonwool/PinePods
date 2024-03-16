use std::collections::HashSet;
use std::rc::Rc;
use serde::Deserialize;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use crate::requests::login_requests::AddUserRequest;
use crate::requests::login_requests::GetUserDetails;
use crate::requests::login_requests::LoginServerRequest;
use crate::requests::login_requests::{GetApiDetails, TimeZoneInfo};
use crate::components::audio::AudioPlayerProps;
use crate::requests::setting_reqs::{AddSettingsUserRequest, EditSettingsUserRequest};
use crate::requests::search_pods::{PodcastFeedResult, PodcastSearchResult, SearchResponse};
use crate::requests::pod_req::{Episode, RecentEps, Podcast, PodcastResponse, QueuedEpisodesResponse, SavedEpisodesResponse, HistoryDataResponse, EpisodeDownloadResponse, EpisodeMetadataResponse};
use yewdux::prelude::*;
use web_sys::HtmlAudioElement;
use serde_json::{json, from_str};
use web_sys::window;
use crate::components::podcast_layout::ClickedFeedURL;
use crate::requests::stat_reqs::UserStats;

#[allow(dead_code)]
#[allow(dead_code)]
pub enum AppStateMsg {
    ExpandEpisode(String),
    CollapseEpisode(String),
    SetLoading(bool),
    UpdateSelectedEpisodesForDeletion(i32), // Add this line
    DeleteSelectedEpisodes, // Add this line
}

impl Reducer<AppState> for AppStateMsg {
    fn apply(self, state: Rc<AppState>) -> Rc<AppState> {
        let mut load_state = state.clone();
        let state_mut = Rc::make_mut(&mut load_state);

        match self {
            AppStateMsg::ExpandEpisode(guid) => {
                state_mut.expanded_descriptions.insert(guid);
            },
            AppStateMsg::CollapseEpisode(guid) => {
                state_mut.expanded_descriptions.remove(&guid);
            },
            AppStateMsg::SetLoading(is_loading) => {
                state_mut.is_loading = Option::from(is_loading);
            },
            AppStateMsg::UpdateSelectedEpisodesForDeletion(episode_id) => { // Add this block
                state_mut.selected_episodes_for_deletion.insert(episode_id);
            },
            AppStateMsg::DeleteSelectedEpisodes => { // Add this block
                // Here you can delete the selected episodes from your state
                // For now, let's just clear the selected episodes
                state_mut.selected_episodes_for_deletion.clear();
            },
        }

        state
    }
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
    pub is_loading: Option<bool>,
    pub gravatar_url: Option<String>,
    #[serde(default)]
    pub expanded_descriptions: HashSet<String>,
    pub selected_theme: Option<String>,
    pub fetched_episode: Option<EpisodeMetadataResponse>,
    pub selected_episode_id: Option<i32>,
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
}

#[derive(Default, Deserialize, Clone, PartialEq, Store, Debug)]
pub struct UserStatsStore {
    pub stats: Option<UserStats>,
}

#[derive(Default, Deserialize, Clone, PartialEq, Store, Debug)]
pub struct SettingsState {
    pub active_tab: Option<String>,
}

#[derive(Default, Clone, PartialEq, Store)]
pub struct UIState {
    pub audio_playing: Option<bool>,
    pub currently_playing: Option<AudioPlayerProps>,
    pub audio_element: Option<HtmlAudioElement>,
    pub current_time_seconds: f64,
    pub current_time_formatted: String,
    pub duration: f64,
    pub duration_formatted: String,
    pub error_message: Option<String>,
    pub info_message: Option<String>,
    pub is_expanded: bool,
    pub episode_in_db: Option<bool>,
    // pub start_pos_sec: f64,
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
        web_sys::console::log_1(&format!("Current playing state: {:?}", self.audio_playing).into());
        if let Some(audio) = &self.audio_element {
            if self.audio_playing.unwrap_or(false) {
                let _ = audio.pause();
                self.audio_playing = Some(false);
                web_sys::console::log_1(&"Paused audio".into());
            } else {
                let _ = audio.play();
                self.audio_playing = Some(true);
                web_sys::console::log_1(&"Playing audio".into());
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
                audio.add_event_listener_with_callback("canplay", closure.as_ref().unchecked_ref()).unwrap();
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
                let _ = local_storage.set_item(user_key, &user_state);
                let _ = local_storage.set_item(auth_key, &auth_state);
                let _ = local_storage.set_item(server_key, &server_state);
            }
        }
    }

    // pub fn load_app_state(key: &str) -> Option<AppState> {
    //     if let Some(window) = window() {
    //         if let Some(local_storage) = window.local_storage().unwrap() {
    //             if let Ok(Some(serialized_state)) = local_storage.get_item(key) {
    //                 return AppState::deserialize(&serialized_state).ok();
    //             }
    //         }
    //     }
    //     None
    // }

}