use std::collections::HashSet;
use serde::Deserialize;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use crate::requests::login_requests::GetUserDetails;
use crate::requests::login_requests::LoginServerRequest;
use crate::requests::login_requests::GetApiDetails;
use crate::components::audio::AudioPlayerProps;
use crate::requests::search_pods::{PodcastFeedResult, PodcastSearchResult};
use yewdux::prelude::*;
use web_sys::HtmlAudioElement;
use serde_json::{json, from_str};
use web_sys::window;
use crate::components::podcast_layout::ClickedFeedURL;


#[derive(Default, Deserialize, Clone, PartialEq, Store)]
pub struct AppState {
    pub user_details: Option<GetUserDetails>,
    pub auth_details: Option<LoginServerRequest>,
    pub server_details: Option<GetApiDetails>,
    pub error_message: Option<String>,
    pub search_results: Option<PodcastSearchResult>,
    pub podcast_feed_results: Option<PodcastFeedResult>,
    pub clicked_podcast_info: Option<ClickedFeedURL>,
    // pub expanded_episodes: HashSet<i64>,
    #[serde(default)]
    pub expanded_descriptions: HashSet<String>,
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
}

impl UIState {
    pub fn set_duration(&mut self, new_duration: f64) {
        self.duration = new_duration;
    }

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
}

impl AppState {
    // pub fn serialize(data_to_serialize: ) -> String {
    //     // Serialize only the necessary fields
    //     json!({
    //         "user_details": self.user_details,
    //         "auth_details": self.auth_details,
    //         "server_details": self.server_details,
    //         // ... other fields you want to serialize
    //     }).to_string()
    // }

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

    pub fn load_app_state(key: &str) -> Option<AppState> {
        if let Some(window) = window() {
            if let Some(local_storage) = window.local_storage().unwrap() {
                if let Ok(Some(serialized_state)) = local_storage.get_item(key) {
                    return AppState::deserialize(&serialized_state).ok();
                }
            }
        }
        None
    }

}