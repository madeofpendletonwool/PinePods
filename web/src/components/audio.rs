use std::cell::Cell;
use std::rc::Rc;
use gloo_timers::callback::Interval;
use yew::{Callback, function_component, Html, html};
use yew::prelude::*;
// use yew_router::history::{History};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use web_sys::{HtmlAudioElement, HtmlInputElement, window};
use std::string::String;
use gloo_utils::document;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use std::cell::RefCell;
use web_sys::HtmlElement;
use std::borrow::Borrow;
use web_sys::Element;

#[derive(Properties, PartialEq, Debug, Clone)]
pub struct AudioPlayerProps {
    pub src: String,
    pub title: String,
    pub artwork_url: String,
    pub duration: String,
    pub episode_id: i32,
    pub duration_sec: f64
}

#[function_component(AudioPlayer)]
pub fn audio_player(props: &AudioPlayerProps) -> Html {
    let audio_ref = use_node_ref();
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let artwork_class = if audio_state.audio_playing.unwrap_or(false) {
        classes!("artwork", "playing")
    } else {
        classes!("artwork")
    };

    let container_ref = use_node_ref();
    let container_ref_clone1 = container_ref.clone();
    let container_ref_clone2 = container_ref.clone();

    let title_click = {
        let audio_dispatch = _audio_dispatch.clone();
        let container_ref = container_ref.clone();
        Callback::from(move |_: MouseEvent| {
            audio_dispatch.reduce_mut(UIState::toggle_expanded);

            // Scroll to the top of the container
            if let Some(container) = container_ref.cast::<HtmlElement>() {
                container.scroll_into_view();
            }
        })
    };
    // dragging attempt
    // let audio_state_drag = audio_state.clone();
    // let audio_dispatch_drag = _audio_dispatch.clone();
    // let is_dragging = Rc::new(Cell::new(false));
    // let start_y = Rc::new(Cell::new(0.0));
    // let initial_height = Rc::new(Cell::new(0.0));
    // let audio_player: Option<HtmlElement> = container_ref_clone2.cast::<HtmlElement>();

    // let audio_player_clone = audio_player.clone();
    // let is_dragging_clone = is_dragging.clone();
    // let start_y_clone = start_y.clone();
    // let initial_height_clone = initial_height.clone();
    // let audio_state_clone = audio_state.clone();
    // let mousedown_callback = Closure::wrap(Box::new(move |event: MouseEvent| {
    //     event.prevent_default(); // prevent text selection
    //     if audio_state_clone.is_expanded { // Only enable dragging when the audio player is expanded
    //         is_dragging_clone.set(true);
    //         start_y_clone.set(event.client_y() as f64);
    //     }
    // }) as Box<dyn FnMut(_)>);

    // if let Some(audio_player) = &audio_player {
    //     audio_player.add_event_listener_with_callback("mousedown", mousedown_callback.as_ref().unchecked_ref()).unwrap();
    // }
    // mousedown_callback.forget();

    // // let audio_player_clone = audio_player.clone();
    // // let is_dragging_clone2 = is_dragging.clone();
    // let start_y_clone2 = start_y.clone();
    // let initial_height_clone2 = initial_height.clone();
    // let audio_player_clone = audio_player.clone();
    // let is_dragging_clone2 = is_dragging.clone();
    // let audio_player_element: Option<web_sys::HtmlElement> = audio_player.as_ref().map(|audio_player| audio_player.clone().dyn_into::<web_sys::HtmlElement>().unwrap());
    // let audio_player_element_clone = audio_player_element.clone(); // Clone the audio_player_element

    // let max_height = web_sys::window().unwrap().inner_height().unwrap().as_f64().unwrap(); // Get the window's inner height

    // let mousemove_callback = Closure::wrap(Box::new(move |event: MouseEvent| {
    //     if !is_dragging_clone2.get() || !audio_state_drag.is_expanded {
    //         return;
    //     }
    //     if let Some(audio_player_element) = audio_player_element.as_ref() {
    //         let rect = audio_player_element.get_bounding_client_rect();
    //         let delta_y = start_y_clone2.get() - event.client_y() as f64;
    //         let new_height = (initial_height_clone2.get() + delta_y).max(60.0).min(max_height);
    //         if delta_y > 0.0 { // User is dragging upwards
    //             audio_player_element.style().set_property("height", &format!("{}px", max_height)).unwrap();
    //         } else { // User is dragging downwards
    //             audio_player_element.style().set_property("height", "60px").unwrap();
    //         }
    //     }
    // }) as Box<dyn FnMut(_)>);
    // let document = web_sys::window().unwrap().document().unwrap();
    // document.clone().add_event_listener_with_callback("mousemove", mousemove_callback.as_ref().unchecked_ref()).unwrap();
    // mousemove_callback.forget();

    // let is_dragging_clone = is_dragging.clone();
    // let mouseup_callback = Closure::wrap(Box::new(move |_: web_sys::Event| {
    //     is_dragging_clone.set(false);
    // }) as Box<dyn FnMut(web_sys::Event)>);

    // document.add_event_listener_with_callback("mouseup", mouseup_callback.as_ref().unchecked_ref()).unwrap();
    // mouseup_callback.forget();
    // end attempt
    // Initialize state for current time and duration
// Initialize state for current time and duration
    let current_time = use_state(|| "00:00:00".to_string());
    let duration = use_state(|| 0.0);
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
    // Update playing state when Spacebar is pressed
    let audio_dispatch_effect = _audio_dispatch.clone();
    use_effect_with(
        (),
        move |_| {
            let keydown_handler = {
                let audio_info = audio_dispatch_effect.clone();
                Closure::wrap(Box::new(move |event: KeyboardEvent| {
                    if event.key() == " " {
                        // Prevent the default behavior of the spacebar key
                        event.prevent_default();
                        // Toggle `audio_playing` here
                        audio_info.reduce_mut(|state| {
                            state.toggle_playback()
                        });
                    }
                }) as Box<dyn FnMut(_)>)
            };
            window().unwrap().add_event_listener_with_callback("keydown", keydown_handler.as_ref().unchecked_ref()).unwrap();
            keydown_handler.forget(); // Note: this will make the listener permanent
            || ()
        }
    );

// Effect for setting up an interval to update the current playback time
    // Effect for setting up an interval to update the current playback time
    // Clone `audio_ref` for `use_effect_with`
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


    // Toggle playback
    let toggle_playback = {
        let dispatch = _dispatch.clone();
        let dispatch = _audio_dispatch.clone();
        web_sys::console::log_1(&format!("Current playing state: {:?}", &audio_state.audio_playing).into());
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
        // let dispatch = _dispatch.clone();
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
        let duration_hours = (audio_props.duration_sec / 3600.0).floor() as i32;
        let duration_minutes = ((audio_props.duration_sec % 3600.0) / 60.0).floor() as i32;
        let duration_seconds = (audio_props.duration_sec % 60.0).floor() as i32;
        let formatted_duration = format!("{:02}:{:02}:{:02}", duration_hours, duration_minutes, duration_seconds);
        let on_shownotes_click = {
            let history = history_clone.clone();
            let dispatch = _dispatch.clone();
            let episode_id = audio_state.currently_playing.as_ref().map(|audio_props| audio_props.episode_id);
        
            Callback::from(move |_: MouseEvent| {
                let dispatch_clone = dispatch.clone();
                let history_clone = history.clone();
                if let Some(episode_id) = episode_id {
                    wasm_bindgen_futures::spawn_local(async move {
                        dispatch_clone.reduce_mut(move |state| {
                            state.selected_episode_id = Some((episode_id));
                        });
                        history_clone.push("/episode"); // Use the route path
                    });
                }
            })
        };
        let audio_bar_class = classes!("audio-player", "border", "border-solid", "border-color", "fixed", "bottom-0", "z-50", "w-full", if audio_state.is_expanded { "expanded" } else { "" });
        let expanded = audio_state.is_expanded.clone();
        html! {
            <div class={audio_bar_class} ref={container_ref.clone()}>
                <div class="top-section">
                    <button onclick={title_click.clone()} class="retract-button">
                        <span class="material-icons">{"expand_more"}</span>
                    </button>
                    <img src={audio_props.artwork_url.clone()} />
                    <div class="title" onclick={title_click.clone()}>{ &audio_props.title }
                    </div>
                    <div class="scrub-bar">
                        <span>{audio_state.current_time_formatted.clone()}</span>
                        <input type="range"
                            class="flex-grow h-1 cursor-pointer"
                            min="0.0"
                            max={audio_props.duration_sec.to_string().clone()}
                            value={audio_state.current_time_seconds.to_string()}
                            oninput={update_time.clone()} />
                        <span>{formatted_duration.clone()}</span>
                    </div>

                    <div class="button-container flex items-center justify-center">
                        <button class="rewind-button item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">{"fast_rewind"}</span>
                        </button>
                        <button onclick={toggle_playback.clone()} class="item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">
                                { if audio_state.audio_playing.unwrap_or(false) { "pause" } else { "play_arrow" } }
                            </span>
                        </button>
                        <button onclick={skip_forward.clone()} class="skip-button item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">{"fast_forward"}</span>
                        </button>
                    </div>
                    <div class="button-container flex items-center justify-center">
                    <button onclick={on_shownotes_click} class="item-container-button audio-full-button border-solid border selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center">{ "Shownotes" }</button>
                    </div>
                </div>
                <div class="line-content">
                <div class="left-group">
                    <img class={artwork_class} src={audio_props.artwork_url.clone()} />
                    <div class="title" onclick={title_click.clone()}>
                        <span>{ &audio_props.title }</span>
                    </div>
                </div>
                <div class="right-group">
                    <button onclick={toggle_playback} class="item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                        <span class="material-icons">
                            { if audio_state.audio_playing.unwrap_or(false) { "pause" } else { "play_arrow" } }
                        </span>
                    </button>
                    <button onclick={skip_forward} class="item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                        <span class="material-icons">{"fast_forward"}</span>
                    </button>
                    <div class="flex-grow flex items-center sm:block hidden">
                        <div class="flex items-center flex-nowrap">
                            <span class="px-2">{audio_state.current_time_formatted.clone()}</span>
                            <input type="range"
                                class="flex-grow h-1 cursor-pointer"
                                min="0.0"
                                max={audio_props.duration_sec.to_string().clone()}
                                value={audio_state.current_time_seconds.to_string()}
                                oninput={update_time.clone()} />
                            <span class="px-2">{formatted_duration}</span>
                        </div>
                    </div>
                </div>
            </div>
            </div>
        }
    } else {
        html! {}
    }
}


pub fn on_play_click(
    episode_url_for_closure: String,
    episode_title_for_closure: String,
    episode_artwork_for_closure: String,
    episode_duration_for_closure: i32,
    episode_id_for_closure: i32,
    audio_dispatch: Dispatch<UIState>,
) -> Callback<MouseEvent> {
    fn parse_duration_to_seconds(duration_convert: &i32) -> f64 {
        let dur_string = duration_convert.to_string();
        let parts: Vec<&str> = dur_string.split(':').collect();
        let parts: Vec<f64> = parts.iter().map(|part| part.parse::<f64>().unwrap_or(0.0)).collect();

        let seconds = match parts.len() {
            3 => parts[0] * 3600.0 + parts[1] * 60.0 + parts[2],
            2 => parts[0] * 60.0 + parts[1],
            1 => parts[0],
            _ => 0.0,
        };

        seconds
    }


    Callback::from(move |_: MouseEvent| {
        let episode_url_for_closure = episode_url_for_closure.clone();
        let episode_title_for_closure = episode_title_for_closure.clone();
        let episode_artwork_for_closure = episode_artwork_for_closure.clone();
        let episode_duration_for_closure = episode_duration_for_closure.clone();
        let episode_id_for_closure = episode_id_for_closure.clone();
        web_sys::console::log_1(&format!("duration: {}", &episode_duration_for_closure).into());
        let audio_dispatch = audio_dispatch.clone();
    
        let formatted_duration = parse_duration_to_seconds(&episode_duration_for_closure);
        audio_dispatch.reduce_mut(move |audio_state| {
            audio_state.audio_playing = Some(true);
            audio_state.currently_playing = Some(AudioPlayerProps {
                src: episode_url_for_closure.clone(),
                title: episode_title_for_closure.clone(),
                artwork_url: episode_artwork_for_closure.clone(),
                duration: episode_duration_for_closure.clone().to_string(),
                episode_id: episode_id_for_closure.clone(),
                duration_sec: formatted_duration,
            });
            audio_state.set_audio_source(episode_url_for_closure.to_string());
            if let Some(audio) = &audio_state.audio_element {
                let _ = audio.play();
            }
            audio_state.audio_playing = Some(true);
        });
    })
}