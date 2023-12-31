use yew::prelude::*;
use std::rc::Rc;
use yew::{Callback, function_component, Html, html, props};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::Dispatch;
use yewdux::prelude::*;
use crate::components::context::{AppState};
use web_sys::{HtmlAudioElement};

#[derive(Properties, PartialEq, Clone)]
pub struct AudioPlayerProps {
    pub src: Rc<String>,
    pub title: String,
}

#[function_component(AudioPlayer)]
pub fn audio_player(props: &AudioPlayerProps) -> Html {
    let audio_ref = use_node_ref();
    let (state, _dispatch) = use_store::<AppState>();

    // Clone the state reference for use inside the closure
    let state_clone_for_closure = state.clone();

    // Update the audio source when `src` changes
    use_effect_with(
        props.src.clone(),
        {
            let src = props.src.clone();
            move |_| {
                if let Some(audio_element) = audio_ref.cast::<HtmlAudioElement>() {
                    audio_element.set_src(&src);
                }
                || ()
            }
        }
    );


    let toggle_playback = {
        let _dispatch = _dispatch.clone();
        Callback::from(move |_| {
            let mut new_state = state_clone_for_closure.as_ref().clone();
            new_state.audio_playing = Some(!new_state.audio_playing.unwrap_or(false));
            _dispatch.set(new_state);
        })
    };


    // Check if there is an audio player prop set in AppState
    if let Some(audio_props) = state.currently_playing.as_ref() {
        web_sys::console::log_1(&format!("Rendering AudioPlayer: {}", &audio_props.title).into());
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




