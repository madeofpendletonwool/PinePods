use gloo_timers::callback::Interval;
// use serde::__private::de::Content::String;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew::events::*;
use yew::{Callback, function_component, Html, html, props};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use web_sys::{HtmlAudioElement, HtmlInputElement};
use std::string::String;


#[derive(Properties, PartialEq, Debug, Clone)]
pub struct AudioPlayerProps {
    pub src: String,
    pub title: String,
    pub duration: String,
    pub duration_sec: f64
}

#[function_component(AudioPlayer)]
pub fn audio_player(props: &AudioPlayerProps) -> Html {
    let audio_ref = use_node_ref();
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();

    // Initialize state for current time and duration
// Initialize state for current time and duration
    let current_time = use_state(|| "00:00:00".to_string());
    let duration = use_state(|| 0.0);

    // Clone the state reference for use inside the closure
    let audio_ref_clone = audio_ref.clone();
    let state_clone_for_closure = state.clone();
    let src_clone = props.src.clone();

    // Update the audio source when `src` changes
    use_effect_with(src_clone.clone(), {
        let src = src_clone.clone();
        let audio_ref = audio_ref.clone();
        move |_| {
            if let Some(audio_element) = audio_ref.cast::<HtmlAudioElement>() {
                audio_element.set_src(&src);
            } else {
            }
            || ()
        }
    });

// Effect for setting up an interval to update the current playback time
    // Effect for setting up an interval to update the current playback time
    // Clone `audio_ref` for `use_effect_with`
    let audio_ref_clone = audio_ref.clone();
    let current_time_clone = current_time.clone();
    let state_clone = audio_state.clone();
    use_effect_with((), {
        let dispatch = _dispatch.clone();
        let audio_dispatch = _audio_dispatch.clone();
        move |_| {
            let interval_handle = Interval::new(1000, move || {
                if let Some(audio_element) = state_clone.audio_element.as_ref() {
                    let time_in_seconds = audio_element.current_time();
                    let hours = (time_in_seconds / 3600.0).floor() as i32;
                    let minutes = ((time_in_seconds % 3600.0) / 60.0).floor() as i32;
                    let seconds = (time_in_seconds % 60.0).floor() as i32;
                    let formatted_time = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

                    audio_dispatch.reduce_mut(move |state_clone| {
                        // Update the current time in your state here
                        state_clone.current_time_seconds = time_in_seconds;
                        state_clone.current_time_formatted = formatted_time;
                    });
                }
            });

            move || drop(interval_handle)
        }
    });

    // Clone for use inside the toggle_playback closure
    let audio_ref_clone = audio_ref.clone();
    let state_clone_for_toggle = state.clone();


    // Toggle playback
    let toggle_playback = {
        let dispatch = _dispatch.clone();
        let dispatch = _audio_dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(UIState::toggle_playback);
        })
    };

    // Update current time and duration
// // Keep the existing use_state for the formatted time
//     let current_time_formatted = use_state(|| "00:00:00".to_string());
//
// // Add a new state for the current time in seconds
//     let current_time_seconds = use_state(|| 0.0);

    let update_time = {
        let dispatch = _dispatch.clone();
        let audio_dispatch = _audio_dispatch.clone();
        Callback::from(move |e: InputEvent| {
            // Get the value from the target of the InputEvent
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(value) = input.value().parse::<f64>() {
                    // Update the state using dispatch
                    audio_dispatch.reduce_mut(move |state| {
                        if let Some(audio_element) = state.audio_element.as_ref() {
                            audio_element.set_current_time(value);
                            state.current_time_seconds = value;

                            // Update formatted time
                            let hours = (value / 3600.0).floor() as i32;
                            let minutes = ((value % 3600.0) / 60.0).floor() as i32;
                            let seconds = (value % 60.0).floor() as i32;
                            state.current_time_formatted = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
                        }
                    });
                }
            }
        })
    };


// Skip forward
    let skip_forward = {
        let dispatch = _dispatch.clone();
        let audio_dispatch = _audio_dispatch.clone();
        Callback::from(move |_| {
            if let Some(audio_element) = audio_state.audio_element.as_ref() {
                let new_time = audio_element.current_time() + 15.0;
                audio_element.set_current_time(new_time);
                audio_dispatch.reduce_mut(|state| state.update_current_time(new_time));
            }
        })
    };


    let audio_state = _audio_dispatch.get();


    // Check if there is an audio player prop set in AppState

    // web_sys::console::log_1(&format!("duration format: {}", &state.sr).into());
    if let Some(audio_props) = audio_state.currently_playing.as_ref() {
        html! {
            <div class="audio-player">
                <span>{ &audio_props.title }</span>
                <div>
                    <button onclick={toggle_playback}>
                        { if audio_state.audio_playing.unwrap_or(false) { "Pause" } else { "Play" } }
                    </button>
                    <button onclick={skip_forward}>{"Skip 15s"}</button>
                    <span>{audio_state.current_time_formatted.clone()}</span>
                    <input type="range"
                        min="0.0"
                        max={audio_props.duration_sec.to_string().clone()}
                        value={audio_state.current_time_seconds.to_string()}
                        oninput={update_time.clone()} />
                    <span>{&audio_props.duration}</span>
                </div>
            </div>
        }
    } else {
        html! {}
    }
}




