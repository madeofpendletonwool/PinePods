use yew::prelude::*;
use std::rc::Rc;
use yew::{Callback, function_component, Html, html, props};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::Dispatch;
use yewdux::prelude::*;
use crate::components::context::{AppState};
use web_sys::{HtmlAudioElement};

#[derive(Properties, PartialEq, Debug, Clone)]
pub struct AudioPlayerProps {
    pub src: String,
    pub title: String,
}

#[function_component(AudioPlayer)]
pub fn audio_player(props: &AudioPlayerProps) -> Html {
    let audio_ref = use_node_ref();
    let (state, _dispatch) = use_store::<AppState>();

    // Clone the state reference for use inside the closure
    let audio_ref_clone = audio_ref.clone();
    let state_clone_for_closure = state.clone();
    let src_clone = props.src.clone();

    // Update the audio source when `src` changes
    use_effect_with(src_clone.clone(), {
        let src = src_clone.clone();
        let audio_ref = audio_ref.clone();
        web_sys::console::log_1(&"Inside use effect".into());
        move |_| {
            if let Some(audio_element) = audio_ref.cast::<HtmlAudioElement>() {
                web_sys::console::log_1(&"Audio element found".into());
                audio_element.set_src(&src);
            } else {
                web_sys::console::log_1(&"Audio element not found".into());
            }
            || ()
        }
    });

    // Clone for use inside the toggle_playback closure
    let audio_ref_clone = audio_ref.clone();
    let state_clone_for_toggle = state.clone();


    // Toggle playback
    let toggle_playback = {
        let dispatch = _dispatch.clone();
        Callback::from(move |_| {
            web_sys::console::log_1(&"Toggling playback".into());
            dispatch.reduce_mut(AppState::toggle_playback);
        })
    };


    let state = _dispatch.get();


    // Check if there is an audio player prop set in AppState
    if let Some(audio_props) = state.currently_playing.as_ref() {
        web_sys::console::log_1(&format!("Rendering AudioPlayer: {}", &audio_props.title).into());
        web_sys::console::log_1(&format!("Rendering AudioPlayer: {}", &audio_props.src).into());
        html! {
            <div class="audio-player">
                <button onclick={toggle_playback}>
                    { if state.audio_playing.unwrap_or(false) { "Pause" } else { "Play" } }
                </button>
                <span>{ &audio_props.title }</span>
                // Add more controls like skip button, slider, etc.
            </div>
        }
    } else {
        web_sys::console::log_1(&"Rendering AudioPlayer: No current audio".into());
        html! {}
    }
}




