use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use crate::requests::login_requests::GetUserDetails;
use crate::requests::login_requests::LoginServerRequest;
use crate::requests::login_requests::GetApiDetails;
use crate::components::audio::AudioPlayerProps;
use crate::requests::search_pods::{PodcastFeedResult, PodcastSearchResult};
use yewdux::prelude::*;
use web_sys::HtmlAudioElement;

#[derive(Default, Clone, PartialEq, Store)]
pub struct AppState {
    pub user_details: Option<GetUserDetails>,
    pub auth_details: Option<LoginServerRequest>,
    pub server_details: Option<GetApiDetails>,
    pub error_message: Option<String>,
    pub search_results: Option<PodcastSearchResult>,
    pub podcast_feed_results: Option<PodcastFeedResult>,
    pub audio_playing: Option<bool>,
    pub currently_playing: Option<AudioPlayerProps>,
    pub audio_element: Option<HtmlAudioElement>,
    pub current_time_seconds: f64,
    pub current_time_formatted: String,
    pub duration: f64,
    pub duration_formatted: String,
}

impl AppState {

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