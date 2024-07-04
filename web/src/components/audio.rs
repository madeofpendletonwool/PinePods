use crate::components::context::{AppState, UIState};
#[cfg(not(feature = "server_build"))]
use crate::components::downloads_tauri::{fetch_local_file, start_local_file_server};
use crate::requests::pod_req::{
    call_add_history, call_check_episode_in_db, call_get_auto_skip_times,
    call_get_podcast_id_from_ep, call_get_queued_episodes, call_increment_listen_time,
    call_increment_played, call_mark_episode_completed, call_queue_episode,
    call_record_listen_duration, call_remove_queued_episode, EpisodeDownload, HistoryAddRequest,
    MarkEpisodeCompletedRequest, QueuePodcastRequest, RecordListenDurationRequest,
};
use futures_util::stream::StreamExt;
use gloo_timers::callback::Interval;
use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;
use std::string::String;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlElement;
use web_sys::{window, HtmlAudioElement, HtmlInputElement};
use yew::prelude::*;
use yew::{function_component, html, Callback, Html};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[derive(Properties, PartialEq, Debug, Clone)]
pub struct AudioPlayerProps {
    pub src: String,
    pub title: String,
    pub artwork_url: String,
    pub duration: String,
    pub episode_id: i32,
    pub duration_sec: f64,
    pub start_pos_sec: f64,
    pub end_pos_sec: f64,
    pub offline: bool,
}

#[function_component(AudioPlayer)]
pub fn audio_player(props: &AudioPlayerProps) -> Html {
    let audio_ref = use_node_ref();
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let episode_id = audio_state
        .currently_playing
        .as_ref()
        .map(|props| props.episode_id);
    web_sys::console::log_1(&JsValue::from_str(&format!("Episode ID: {:?}", episode_id)));
    let end_pos = audio_state
        .currently_playing
        .as_ref()
        .map(|props| props.end_pos_sec);
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let episode_in_db = audio_state.episode_in_db.unwrap_or_default();
    let progress: UseStateHandle<f64> = use_state(|| 0.0);
    let offline_status = audio_state
        .currently_playing
        .as_ref()
        .map(|props| props.offline);
    web_sys::console::log_1(&JsValue::from_str(&format!(
        "Offline Status: {:?}",
        offline_status
    )));
    let artwork_class = if audio_state.audio_playing.unwrap_or(false) {
        classes!("artwork", "playing")
    } else {
        classes!("artwork")
    };

    let container_ref = use_node_ref();

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
    let title_click_emit = title_click.clone();
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
    use_effect_with((), move |_| {
        let keydown_handler = {
            let audio_info = audio_dispatch_effect.clone();
            Closure::wrap(Box::new(move |event: KeyboardEvent| {
                // Check if the event target is not an input or textarea
                let target = event
                    .target()
                    .unwrap()
                    .dyn_into::<web_sys::HtmlElement>()
                    .unwrap();
                if !(target.tag_name().eq_ignore_ascii_case("input")
                    || target.tag_name().eq_ignore_ascii_case("textarea"))
                {
                    if event.key() == " " {
                        // Prevent the default behavior of the spacebar key
                        event.prevent_default();
                        // Toggle `audio_playing` here
                        audio_info.reduce_mut(|state| state.toggle_playback());
                    }
                }
            }) as Box<dyn FnMut(_)>)
        };
        window()
            .unwrap()
            .add_event_listener_with_callback("keydown", keydown_handler.as_ref().unchecked_ref())
            .unwrap();
        keydown_handler.forget(); // Note: this will make the listener permanent
        || ()
    });

    // Effect for setting up an interval to update the current playback time
    // Clone `audio_ref` for `use_effect_with`
    let state_clone = audio_state.clone();
    use_effect_with((offline_status.clone(), episode_id.clone()), {
        let audio_dispatch = _audio_dispatch.clone();
        let progress = progress.clone(); // Clone for the interval closure
        let closure_api_key = api_key.clone();
        let closure_server_name = server_name.clone();
        let closure_user_id = user_id.clone();
        let closure_episode_id = episode_id.clone();
        let offline_status = offline_status.clone();
        move |_| {
            let interval_handle: Rc<Cell<Option<Interval>>> = Rc::new(Cell::new(None));
            let interval_handle_clone = interval_handle.clone();
            let interval = Interval::new(1000, move || {
                if let Some(audio_element) = state_clone.audio_element.as_ref() {
                    let time_in_seconds = audio_element.current_time();
                    let duration = audio_element.duration(); // Assuming you can get the duration from the audio_element
                    let end_pos_sec = end_pos.clone(); // Get the end position
                    let complete_api_key = closure_api_key.clone();
                    let complete_server_name = closure_server_name.clone();
                    let complete_user_id = closure_user_id.clone();
                    let complete_episode_id = closure_episode_id.clone();
                    let offline_status_loop = offline_status.unwrap_or(false);
                    if time_in_seconds >= (duration - end_pos_sec.unwrap()) {
                        audio_element.pause().unwrap_or(());
                        // Manually trigger the `ended` event
                        let event = web_sys::Event::new("ended").unwrap();
                        audio_element.dispatch_event(&event).unwrap();
                        // Call the endpoint to mark episode as completed
                        if offline_status_loop {
                            // If offline, store the episode in the local database
                            web_sys::console::log_1(
                                &"Offline mode enabled. Not recording completion status.".into(),
                            );
                        } else {
                            // If online, call the endpoint
                            wasm_bindgen_futures::spawn_local(async move {
                                if let (
                                    Some(complete_api_key),
                                    Some(complete_server_name),
                                    Some(complete_user_id),
                                    Some(complete_episode_id),
                                ) = (
                                    complete_api_key.as_ref(),
                                    complete_server_name.as_ref(),
                                    complete_user_id.as_ref(),
                                    complete_episode_id.as_ref(),
                                ) {
                                    let request = MarkEpisodeCompletedRequest {
                                        episode_id: *complete_episode_id, // Dereference the option
                                        user_id: *complete_user_id,       // Dereference the option
                                    };

                                    match call_mark_episode_completed(
                                        &complete_server_name,
                                        &complete_api_key,
                                        &request,
                                    )
                                    .await
                                    {
                                        Ok(message) => {
                                            web_sys::console::log_1(
                                                &format!("Success: {}", message).into(),
                                            );
                                        }
                                        Err(e) => {
                                            web_sys::console::log_1(
                                                &format!("Error: {}", e).into(),
                                            );
                                        }
                                    }
                                }
                            });
                        }

                        // Stop the interval
                        if let Some(handle) = interval_handle.take() {
                            handle.cancel();
                            interval_handle.set(None);
                        }
                    } else {
                        let hours = (time_in_seconds / 3600.0).floor() as i32;
                        let minutes = ((time_in_seconds % 3600.0) / 60.0).floor() as i32;
                        let seconds = (time_in_seconds % 60.0).floor() as i32;
                        let formatted_time = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

                        // Calculate progress as a percentage
                        let progress_percentage = if duration > 0.0 {
                            time_in_seconds / duration * 100.0
                        } else {
                            0.0
                        };

                        audio_dispatch.reduce_mut(move |state_clone| {
                            // Update the global state with the current time
                            state_clone.current_time_seconds = time_in_seconds;
                            state_clone.current_time_formatted = formatted_time;
                        });

                        progress.set(progress_percentage);
                    }
                }
            });

            interval_handle_clone.set(Some(interval));
            let interval_handle = interval_handle_clone;
            // Return a cleanup function that will be run when the component unmounts or the dependencies of the effect change
            move || {
                if let Some(handle) = interval_handle.take() {
                    handle.cancel();
                }
            }
        }
    });

    // Effect for recording the listen duration
    let audio_state_clone = audio_state.clone();
    use_effect_with((offline_status.clone(), episode_id.clone()), {
        let server_name = server_name.clone(); // Assuming this is defined elsewhere in your component
        let api_key = api_key.clone(); // Assuming this is defined elsewhere in your component
        let user_id = user_id.clone(); // Assuming this is defined elsewhere in your component
        let offline_status = offline_status.clone();
        let episode_id = episode_id.clone();

        move |_| {
            // Create an interval task
            let interval_handle = Rc::new(Cell::new(None));
            let interval_handle_clone = interval_handle.clone();

            let interval = gloo_timers::callback::Interval::new(30_000, move || {
                let state_clone = audio_state_clone.clone(); // Access the latest state
                let offline_status_loop = offline_status.unwrap_or(false);
                let episode_id_loop = episode_id.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();

                if offline_status_loop {
                    web_sys::console::log_1(
                        &"Offline mode enabled. Not recording listen duration.".into(),
                    );
                    web_sys::console::log_1(&JsValue::from_str(&format!(
                        "Offline Status in task: {:?}",
                        offline_status_loop
                    )));
                } else {
                    web_sys::console::log_1(&"Online mode enabled. ".into());
                    web_sys::console::log_1(&JsValue::from_str(&format!(
                        "Offline Status in task: {:?}",
                        offline_status_loop
                    )));
                    web_sys::console::log_1(&JsValue::from_str(&format!(
                        "ep id Status in task: {:?}",
                        episode_id_loop
                    )));
                    if state_clone.audio_playing.unwrap_or_default() {
                        if let Some(audio_element) = state_clone.audio_element.as_ref() {
                            let listen_duration = audio_element.current_time();
                            let request_data = RecordListenDurationRequest {
                                episode_id: episode_id_loop.unwrap().clone(),
                                user_id: user_id.unwrap().clone(),
                                listen_duration,
                            };

                            wasm_bindgen_futures::spawn_local(async move {
                                match call_record_listen_duration(
                                    &server_name.clone().unwrap(),
                                    &api_key.clone().unwrap().unwrap(),
                                    request_data,
                                )
                                .await
                                {
                                    Ok(_response) => {}
                                    Err(_e) => {}
                                }
                            });
                        }
                    }
                }
            });

            interval_handle_clone.set(Some(interval));

            // Cleanup function to cancel the interval task when dependencies change
            move || {
                if let Some(interval) = interval_handle.take() {
                    interval.cancel();
                }
            }
        }
    });

    // Effect for incrementing user listen time
    let state_increment_clone = audio_state.clone();
    use_effect_with((offline_status.clone(), episode_id.clone()), {
        let server_name = server_name.clone(); // Make sure `server_name` is cloned from the parent scope
        let api_key = api_key.clone(); // Make sure `api_key` is cloned from the parent scope
        let user_id = user_id.clone(); // Make sure `user_id` is cloned from the parent scope
        let offline_status = offline_status.clone();

        move |_| {
            let interval_handle: Rc<Cell<Option<Interval>>> = Rc::new(Cell::new(None));
            let interval_handle_clone = interval_handle.clone();

            let interval = Interval::new(60000, move || {
                let offline_status_loop = offline_status.unwrap_or(false);
                // Check if audio is playing before making the API call
                if offline_status_loop {
                    web_sys::console::log_1(
                        &"Offline mode enabled. Not incrementing listen time.".into(),
                    );
                } else {
                    if state_increment_clone.audio_playing.unwrap_or_default() {
                        let server_name = server_name.clone();
                        let api_key = api_key.clone();
                        let user_id = user_id.clone();

                        // Spawn a new async task for the API call
                        wasm_bindgen_futures::spawn_local(async move {
                            match call_increment_listen_time(
                                &server_name.unwrap(),
                                &api_key.unwrap().unwrap(),
                                user_id.unwrap(),
                            )
                            .await
                            {
                                Ok(_response) => {}
                                Err(_e) => {}
                            }
                        });
                    }
                }
            });

            interval_handle_clone.set(Some(interval));
            let interval_handle = interval_handle_clone;
            // Return a cleanup function that will be run when the component unmounts or the dependencies of the effect change
            move || {
                if let Some(handle) = interval_handle.take() {
                    handle.cancel();
                }
            }
        }
    });

    // Effect for managing queued episodes
    use_effect_with(audio_ref.clone(), {
        let audio_dispatch = _audio_dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let current_episode_id = episode_id.clone(); // Assuming this is correctly obtained elsewhere
        let audio_state = audio_state.clone();
        let audio_state_cloned = audio_state.clone();
        let offline_status = offline_status.clone();

        move |_| {
            if let Some(audio_element) = audio_state_cloned.audio_element.clone() {
                // if let Some(audio_element) = audio_ref.cast::<HtmlAudioElement>() {
                // Clone all necessary data to be used inside the closure to avoid FnOnce limitation.

                let ended_closure = Closure::wrap(Box::new(move || {
                    let server_name = server_name.clone();
                    let api_key = api_key.clone();
                    let user_id = user_id.clone();
                    let audio_dispatch = audio_dispatch.clone();
                    let current_episode_id = current_episode_id.clone();
                    let audio_state = audio_state.clone();
                    let offline_status_loop = offline_status.unwrap_or(false);
                    // Closure::wrap(Box::new(move |_| {
                    if offline_status_loop {
                        // If offline, do not perform any action
                        web_sys::console::log_1(
                            &"Offline mode enabled. Not managing queue.".into(),
                        );
                    } else {
                        wasm_bindgen_futures::spawn_local(async move {
                            let queued_episodes_result = call_get_queued_episodes(
                                &server_name.clone().unwrap(),
                                &api_key.clone().unwrap(),
                                &user_id.clone().unwrap(),
                            )
                            .await;
                            match queued_episodes_result {
                                Ok(episodes) => {
                                    if let Some(current_episode) = episodes
                                        .iter()
                                        .find(|ep| ep.episodeid == current_episode_id.unwrap())
                                    {
                                        let current_queue_position =
                                            current_episode.queueposition.unwrap_or_default();
                                        // Remove the currently playing episode from the queue
                                        let request = QueuePodcastRequest {
                                            episode_id: current_episode_id.clone().unwrap(),
                                            user_id: user_id.clone().unwrap(), // replace with the actual user ID
                                        };
                                        let remove_result = call_remove_queued_episode(
                                            &server_name.clone().unwrap(),
                                            &api_key.clone().unwrap(),
                                            &request,
                                        )
                                        .await;
                                        match remove_result {
                                            Ok(_) => {
                                                // web_sys::console::log_1(&"Successfully removed episode from queue".into());
                                            }
                                            Err(_e) => {
                                                // web_sys::console::log_1(&format!("Failed to remove episode from queue: {:?}", e).into());
                                            }
                                        }
                                        if let Some(next_episode) = episodes.iter().find(|ep| {
                                            ep.queueposition == Some(current_queue_position + 1)
                                        }) {
                                            on_play_click(
                                                next_episode.episodeurl.clone(),
                                                next_episode.episodetitle.clone(),
                                                next_episode.episodeartwork.clone(),
                                                next_episode.episodeduration,
                                                next_episode.episodeid,
                                                next_episode.listenduration,
                                                api_key.clone().unwrap().unwrap(),
                                                user_id.unwrap(),
                                                server_name.clone().unwrap(),
                                                audio_dispatch.clone(),
                                                audio_state.clone(),
                                                None,
                                            )
                                            .emit(MouseEvent::new("click").unwrap());
                                        } else {
                                            audio_dispatch.reduce_mut(|state| {
                                                state.audio_playing = Some(false);
                                            });
                                        }
                                    }
                                }
                                Err(_e) => {
                                    // web_sys::console::log_1(&format!("Failed to fetch queued episodes: {:?}", e).into());
                                }
                            }
                        });
                    }
                    // }) as Box<dyn FnMut()>);
                }) as Box<dyn FnMut()>);
                // Setting and forgetting the closure must be done within the same scope
                audio_element.set_onended(Some(ended_closure.as_ref().unchecked_ref()));
                ended_closure.forget(); // This will indeed cause a memory leak if the component mounts multiple times
            }

            || ()
        }
    });

    // Toggle playback
    let toggle_playback = {
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
                            state.current_time_formatted =
                                format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
                        }
                    });
                }
            }
        })
    };
    let speed_dispatch = _audio_dispatch.clone();

    // Adjust the playback speed based on a slider value
    let update_playback_speed = {
        Callback::from(move |speed: f64| {
            speed_dispatch.reduce_mut(|speed_state| {
                speed_state.playback_speed = speed;
                if let Some(audio_element) = &speed_state.audio_element {
                    audio_element.set_playback_rate(speed);
                }
            });
        })
    };

    let volume_dispatch = _audio_dispatch.clone();

    // Adjust the volume based on a slider value
    let update_playback_volume = {
        let audio_dispatch = volume_dispatch.clone();
        Callback::from(move |volume: f64| {
            audio_dispatch.reduce_mut(|audio_state| {
                audio_state.audio_volume = volume;
                if let Some(audio_element) = &audio_state.audio_element {
                    audio_element.set_volume(volume / 100.0); // Set volume as a percentage
                }
            });
        })
    };

    let slider_visibility: UseStateHandle<bool> = use_state(|| false);
    let toggle_slider_visibility = {
        let slider_visibility = slider_visibility.clone();
        Callback::from(move |_| slider_visibility.set(!*slider_visibility))
    };

    let volume_slider: UseStateHandle<bool> = use_state(|| false);
    let on_volume_control_click = {
        let volume_slider = volume_slider.clone();
        Callback::from(move |_| volume_slider.set(!*volume_slider))
    };

    // Skip forward
    let skip_state = audio_state.clone();
    let skip_forward = {
        // let dispatch = _dispatch.clone();
        let audio_dispatch = _audio_dispatch.clone();
        Callback::from(move |_| {
            if let Some(audio_element) = skip_state.audio_element.as_ref() {
                let new_time = audio_element.current_time() + 15.0;
                audio_element.set_current_time(new_time);
                audio_dispatch.reduce_mut(|state| state.update_current_time(new_time));
            }
        })
    };

    let backward_state = audio_state.clone();
    let skip_backward = {
        // let dispatch = _dispatch.clone();
        let audio_dispatch = _audio_dispatch.clone();
        Callback::from(move |_| {
            if let Some(audio_element) = backward_state.audio_element.as_ref() {
                let new_time = audio_element.current_time() - 15.0;
                audio_element.set_current_time(new_time);
                audio_dispatch.reduce_mut(|state| state.update_current_time(new_time));
            }
        })
    };

    let skip_episode = {
        let audio_dispatch = _audio_dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let current_episode_id = episode_id.clone(); // Assuming this is correctly obtained elsewhere
        let audio_state = audio_state.clone();

        Callback::from(move |_: MouseEvent| {
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let audio_dispatch = audio_dispatch.clone();
            let audio_state = audio_state.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let episodes_result = call_get_queued_episodes(
                    &server_name.clone().unwrap(),
                    &api_key.clone().unwrap(),
                    &user_id.clone().unwrap(),
                )
                .await;
                if let Ok(episodes) = episodes_result {
                    if let Some(current_episode) = episodes
                        .iter()
                        .find(|ep| ep.episodeid == current_episode_id.unwrap())
                    {
                        let current_queue_position =
                            current_episode.queueposition.unwrap_or_default();

                        if let Some(next_episode) = episodes
                            .iter()
                            .find(|ep| ep.queueposition == Some(current_queue_position + 1))
                        {
                            on_play_click(
                                next_episode.episodeurl.clone(),
                                next_episode.episodetitle.clone(),
                                next_episode.episodeartwork.clone(),
                                next_episode.episodeduration,
                                next_episode.episodeid,
                                next_episode.listenduration,
                                api_key.clone().unwrap().unwrap(),
                                user_id.unwrap(),
                                server_name.clone().unwrap(),
                                audio_dispatch.clone(),
                                audio_state.clone(),
                                None,
                            )
                            .emit(MouseEvent::new("click").unwrap());
                        } else {
                            audio_dispatch.reduce_mut(|state| {
                                state.audio_playing = Some(false);
                            });
                        }
                    }
                } else {
                    // Handle the error, maybe log it or show a user-facing message
                    web_sys::console::log_1(&"Failed to fetch queued episodes".into());
                }
            });
        })
    };

    let audio_state = _audio_dispatch.get();

    // Check if there is an audio player prop set in AppState

    // web_sys::console::log_1(&format!("duration format: {}", &state.sr).into());
    if let Some(audio_props) = audio_state.currently_playing.as_ref() {
        let duration_hours = (audio_props.duration_sec / 3600.0).floor() as i32;
        let duration_minutes = ((audio_props.duration_sec % 3600.0) / 60.0).floor() as i32;
        let duration_seconds = (audio_props.duration_sec % 60.0).floor() as i32;
        let formatted_duration = format!(
            "{:02}:{:02}:{:02}",
            duration_hours, duration_minutes, duration_seconds
        );
        let on_shownotes_click = {
            let history = history_clone.clone();
            let dispatch = _dispatch.clone();
            let episode_id = audio_state
                .currently_playing
                .as_ref()
                .map(|audio_props| audio_props.episode_id);

            Callback::from(move |_: MouseEvent| {
                web_sys::console::log_1(&format!("episode_id: {:?}", episode_id).into());
                let dispatch_clone = dispatch.clone();
                let history_clone = history.clone();
                if let Some(episode_id) = episode_id {
                    wasm_bindgen_futures::spawn_local(async move {
                        dispatch_clone.reduce_mut(move |state| {
                            state.selected_episode_id = Some(episode_id);
                        });
                        history_clone.push("/episode"); // Use the route path
                    });
                }
            })
        };

        // let progress: f64 = 0.0; // Assuming 'progress' is defined here as an example
        let track_width_px: f64 = 300.0; // Explicitly typing the variable
        let pixel_offset: f64 = 60.0; // Explicitly typing the variable
        let offset_percentage: f64 = (pixel_offset / track_width_px) * 100.0; // This will also be f64

        let progress_style = {
            let progress_percentage: f64 = *progress; // Ensure this variable is typed as f64
            let start: f64 = (progress_percentage - offset_percentage).max(0.0); // Using max on f64
            let end: f64 = (progress_percentage + offset_percentage).min(100.0); // Using min on f64
            format!(
                "background: linear-gradient(to right, #007BFF {}%, #E9ECEF {}%);",
                start, end
            )
        };

        let audio_bar_class = classes!(
            "audio-player",
            "border",
            "border-solid",
            "border-color",
            "fixed",
            "bottom-0",
            "z-50",
            "w-full",
            if audio_state.is_expanded {
                "expanded"
            } else {
                ""
            }
        );
        let update_volume_closure = update_playback_volume.clone();
        let update_playback_closure = update_playback_speed.clone();
        html! {
            <div class={audio_bar_class} ref={container_ref.clone()}>
                <div class="top-section">
                    <div>
                    <button onclick={title_click.clone()} class="retract-button">
                        <span class="material-icons">{"expand_more"}</span>
                    </button>
                    <div class="audio-image-container">
                    <img onclick={title_click.clone()} src={audio_props.artwork_url.clone()} />
                    </div>
                    <div class="title" onclick={title_click.clone()}>{ &audio_props.title }
                    </div>
                    <div class="scrub-bar">
                        <span>{audio_state.current_time_formatted.clone()}</span>
                        <input type="range"
                            class="flex-grow h-1 cursor-pointer"
                            min="0.0"
                            max={audio_props.duration_sec.to_string().clone()}
                            value={audio_state.current_time_seconds.to_string()}
                            oninput={update_time.clone()}
                            style={progress_style}
                        />
                        <span>{formatted_duration.clone()}</span>
                    </div>

                    <div class="episode-button-container flex items-center justify-center">
                        // <button onclick={change_speed.clone()} class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                        //     <span class="material-icons">{"speed"}</span>
                        // </button>
                        {
                            html! {
                                <>
                                    <button onclick={toggle_slider_visibility.clone()} class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                                        <span class="material-icons">{"speed"}</span>
                                    </button>
                                </>
                            }
                        }
                        <button onclick={skip_backward.clone()} class="rewind-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">{"fast_rewind"}</span>
                        </button>
                        <button onclick={toggle_playback.clone()} class="audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">
                                { if audio_state.audio_playing.unwrap_or(false) { "pause" } else { "play_arrow" } }
                            </span>
                        </button>
                        <button onclick={skip_forward.clone()} class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">{"fast_forward"}</span>
                        </button>
                        <button onclick={skip_episode.clone()} class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">{"skip_next"}</span>
                        </button>
                    </div>
                    <div class="episode-button-container flex items-center justify-center">
                    // Other buttons as before
                        <div class={classes!("playback-speed-display", if *slider_visibility {"visible"} else {"hidden"})}>
                            <div class="speed-display-container">
                                <div class="speed-text"> // Use inline styles or a class
                                    {format!("{}x", audio_state.playback_speed)}
                                </div>
                                <input
                                    type="range"
                                    class="slider"  // Center this slider independently
                                    min="0.5"
                                    max="2.0"
                                    step="0.1"
                                    value={audio_state.playback_speed.to_string()}
                                    oninput={Callback::from(move |event: InputEvent| {
                                        let input: HtmlInputElement = event.target_unchecked_into();
                                        let speed = input.value_as_number();
                                        update_playback_closure.emit(speed);
                                    })}
                                />
                            </div>
                        </div>
                    </div>

                    <div class="episode-button-container flex items-center justify-center">
                    {
                        if episode_in_db {
                            html! {
                                <button onclick={Callback::from(move |e: MouseEvent| {
                                    on_shownotes_click.emit(e.clone());
                                    title_click_emit.emit(e);
                                })} class="audio-top-button audio-full-button border-solid border selector-button font-bold py-2 px-4 mt-3 rounded-full flex items-center justify-center">
                                    { "Shownotes" }
                                </button>
                            }
                        } else {
                            html! {
                                <button disabled=true class="item-container-button audio-full-button border-solid border selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center opacity-50 cursor-not-allowed">
                                    { "Shownotes (Unavailable)" }
                                </button>
                            }
                        }
                    }
                    <button onclick={on_volume_control_click.clone()} class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center custom-volume-button">
                        <span class="material-icons">{"volume_up"}</span>
                    </button>
                    <div class={classes!("volume-control-display", if *volume_slider {"visible"} else {"hidden"})}>
                        <div class="volume-display-container">
                            <div class="volume-text"> // Use inline styles or a class
                                {format!("{}", audio_state.audio_volume)}
                            </div>
                            <input
                                type="range"
                                class="slider"  // Center this slider independently
                                min="1"
                                max="100"
                                step="1"
                                value={audio_state.audio_volume.to_string()}
                                oninput={Callback::from(move |event: InputEvent| {
                                    let input: HtmlInputElement = event.target_unchecked_into();
                                    let volume = input.value_as_number();
                                    update_volume_closure.emit(volume);
                                })}
                            />
                        </div>
                    </div>
                    </div>
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
                    <button onclick={toggle_playback} class="audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                        <span class="material-icons">
                            { if audio_state.audio_playing.unwrap_or(false) { "pause" } else { "play_arrow" } }
                        </span>
                    </button>
                    <button onclick={skip_forward} class="audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                        <span class="material-icons">{"fast_forward"}</span>
                    </button>
                    <div class="flex-grow flex items-center sm:block hidden">
                        <div class="flex items-center flex-nowrap">
                            <span class="time-display px-2">{audio_state.current_time_formatted.clone()}</span>
                            <input type="range"
                                class="flex-grow h-1 cursor-pointer"
                                min="0.0"
                                max={audio_props.duration_sec.to_string().clone()}
                                value={audio_state.current_time_seconds.to_string()}
                                oninput={update_time.clone()} />
                            <span class="time-display px-2">{formatted_duration}</span>
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
    listen_duration_for_closure: Option<i32>,
    api_key: String,
    user_id: i32,
    server_name: String,
    audio_dispatch: Dispatch<UIState>,
    _audio_state: Rc<UIState>,
    is_local: Option<bool>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        web_sys::console::log_1(&JsValue::from_str("Play button clicked"));
        let episode_url_for_closure = episode_url_for_closure.clone();
        let episode_title_for_closure = episode_title_for_closure.clone();
        let episode_artwork_for_closure = episode_artwork_for_closure.clone();
        let episode_duration_for_closure = episode_duration_for_closure.clone();
        let listen_duration_for_closure = listen_duration_for_closure.clone();
        let episode_id_for_closure = episode_id_for_closure.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let audio_dispatch = audio_dispatch.clone();

        let episode_pos: f32 = 0.0;
        let episode_id = episode_id_for_closure.clone();

        let call_ep_url = episode_url_for_closure.clone();
        let check_server_name = server_name.clone();
        let check_api_key = api_key.clone();
        let check_user_id = user_id.clone();
        let episode_title_for_wasm = episode_title_for_closure.clone();
        let episode_url_for_wasm = call_ep_url.clone();
        let episode_artwork_for_wasm = episode_artwork_for_closure.clone();
        let episode_duration_for_wasm = episode_duration_for_closure.clone();
        let episode_id_for_wasm = episode_id_for_closure.clone();
        let app_dispatch = audio_dispatch.clone();
        let episode_url = episode_url_for_wasm.clone();
        let episode_title = episode_title_for_wasm.clone();
        spawn_local(async move {
            let episode_exists = call_check_episode_in_db(
                &check_server_name.clone(),
                &check_api_key.clone(),
                check_user_id.clone(),
                &episode_title.clone(),
                &episode_url.clone(),
            )
            .await
            .unwrap_or(false); // Default to false if the call fails
            app_dispatch.reduce_mut(move |global_state| {
                global_state.episode_in_db = Some(episode_exists);
            });
            if episode_exists {
                let history_server_name = check_server_name.clone();
                let history_api_key = check_api_key.clone();

                let history_add = HistoryAddRequest {
                    episode_id,
                    episode_pos,
                    user_id,
                };

                let add_history_future =
                    call_add_history(&history_server_name, history_api_key, &history_add);
                match add_history_future.await {
                    Ok(_) => {
                        // web_sys::console::log_1(&"Successfully added history".into());
                    }
                    Err(_e) => {
                        // web_sys::console::log_1(&format!("Failed to add history: {:?}", e).into());
                    }
                }

                let queue_server_name = check_server_name.clone();
                let queue_api_key = check_api_key.clone();

                let request = QueuePodcastRequest {
                    episode_id,
                    user_id, // replace with the actual user ID
                };

                let queue_api = Option::from(queue_api_key);

                let add_queue_future = call_queue_episode(&queue_server_name, &queue_api, &request);
                match add_queue_future.await {
                    Ok(_) => {
                        // web_sys::console::log_1(&"Successfully Added Episode to Queue".into());
                    }
                    Err(_e) => {
                        // web_sys::console::log_1(&format!("Failed to add to queue: {:?}", e).into());
                    }
                }
            }
        });

        let increment_server_name = server_name.clone();
        let increment_api_key = api_key.clone();
        let increment_user_id = user_id.clone();
        spawn_local(async move {
            let add_history_future = call_increment_played(
                &increment_server_name,
                &increment_api_key,
                increment_user_id,
            );
            match add_history_future.await {
                Ok(_) => {
                    // web_sys::console::log_1(&"Successfully incremented playcount".into());
                }
                Err(_e) => {
                    // web_sys::console::log_1(&format!("Failed to increment: {:?}", e).into());
                }
            }
        });
        let src = if let Some(_local) = is_local {
            // Construct the URL for streaming from the local server
            let src = format!(
                "{}/api/data/stream/{}?api_key={}&user_id={}",
                server_name, episode_id, api_key, user_id
            );
            src
        } else {
            // Use the provided URL for streaming
            let src = episode_url_for_wasm.clone();
            src
        };

        wasm_bindgen_futures::spawn_local(async move {
            match call_get_podcast_id_from_ep(
                &server_name,
                &Some(api_key.clone()),
                episode_id,
                user_id,
            )
            .await
            {
                Ok(podcast_id) => {
                    match call_get_auto_skip_times(
                        &server_name,
                        &Some(api_key.clone()),
                        user_id,
                        podcast_id,
                    )
                    .await
                    {
                        Ok((start_skip, end_skip)) => {
                            let start_pos_sec =
                                listen_duration_for_closure.unwrap_or(0).max(start_skip) as f64;
                            let end_pos_sec = end_skip as f64;

                            audio_dispatch.reduce_mut(move |audio_state| {
                                audio_state.audio_playing = Some(true);
                                audio_state.playback_speed = 1.0;
                                audio_state.audio_volume = 100.0;
                                audio_state.offline = Some(false);
                                audio_state.currently_playing = Some(AudioPlayerProps {
                                    src: src.clone(),
                                    title: episode_title_for_wasm.clone(),
                                    artwork_url: episode_artwork_for_wasm.clone(),
                                    duration: episode_duration_for_wasm.clone().to_string(),
                                    episode_id: episode_id_for_wasm.clone(),
                                    duration_sec: episode_duration_for_wasm.clone() as f64,
                                    start_pos_sec,
                                    end_pos_sec: end_pos_sec as f64,
                                    offline: false,
                                });
                                audio_state.set_audio_source(src.to_string());
                                if let Some(audio) = &audio_state.audio_element {
                                    audio.set_current_time(start_pos_sec);
                                    let _ = audio.play();
                                }
                                audio_state.audio_playing = Some(true);
                            });
                        }

                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error getting skip times: {}", e).into(),
                            );
                        }
                    }
                }
                Err(e) => {
                    web_sys::console::log_1(&format!("Error getting podcast ID: {}", e).into());
                }
            };
        });
    })
}

#[cfg(not(feature = "server_build"))]
pub fn on_play_click_offline(
    episode_info: EpisodeDownload,
    audio_dispatch: Dispatch<UIState>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        web_sys::console::log_1(&JsValue::from_str("Play button clicked offline"));
        let episode_info_for_closure = episode_info.clone();
        let audio_dispatch = audio_dispatch.clone();

        let file_path = episode_info_for_closure.downloadedlocation.clone();
        let episode_title_for_wasm = episode_info_for_closure.episodetitle.clone();
        let episode_artwork_for_wasm = episode_info_for_closure.episodeartwork.clone();
        let episode_duration_for_wasm = episode_info_for_closure.episodeduration.clone();
        let episode_id_for_wasm = episode_info_for_closure.episodeid.clone();
        let listen_duration_for_closure = episode_info_for_closure.listenduration.clone();

        wasm_bindgen_futures::spawn_local(async move {
            match start_local_file_server(&file_path).await {
                Ok(server_url) => {
                    let file_name = Path::new(&file_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or(""); // Extract the file name from the path

                    web_sys::console::log_1(&format!("Server URL: {}", server_url).into());
                    web_sys::console::log_1(&format!("{}/{}", server_url, file_name).into());
                    let src = format!("{}/{}", server_url, file_name);

                    audio_dispatch.reduce_mut(move |audio_state| {
                        audio_state.audio_playing = Some(true);
                        audio_state.playback_speed = 1.0;
                        audio_state.audio_volume = 100.0;
                        audio_state.offline = Some(true);
                        audio_state.currently_playing = Some(AudioPlayerProps {
                            src: src.clone(),
                            title: episode_title_for_wasm.clone(),
                            artwork_url: episode_artwork_for_wasm.clone(),
                            duration: episode_duration_for_wasm.clone().to_string(),
                            episode_id: episode_id_for_wasm.clone(),
                            duration_sec: episode_duration_for_wasm.clone() as f64,
                            start_pos_sec: listen_duration_for_closure.unwrap_or(0) as f64,
                            end_pos_sec: 0.0,
                            offline: true,
                        });
                        audio_state.set_audio_source(src.to_string());
                        if let Some(audio) = &audio_state.audio_element {
                            audio.set_current_time(listen_duration_for_closure.unwrap_or(0) as f64);
                            let _ = audio.play();
                        }
                        audio_state.audio_playing = Some(true);
                    });
                }
                Err(e) => {
                    web_sys::console::log_1(
                        &format!("Error starting local file server: {:?}", e).into(),
                    );
                }
            }
        });
    })
}
