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
}

impl AppState {
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
        }
        if let Some(audio) = &self.audio_element {
            audio.set_src(&src);
        }
    }
}