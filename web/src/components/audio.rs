use crate::components::context::{AppState, UIState};
#[cfg(not(feature = "server_build"))]
use crate::components::downloads_tauri::start_local_file_server;
use crate::components::gen_components::{EpisodeModal, FallbackImage};
use crate::components::gen_funcs::format_time_rm_hour;
#[cfg(not(feature = "server_build"))]
use crate::requests::pod_req::EpisodeDownload;
use crate::requests::pod_req::FetchPodcasting2DataRequest;
use crate::requests::pod_req::{
    call_add_history, call_check_episode_in_db, call_fetch_podcasting_2_data,
    call_get_auto_skip_times, call_get_episode_id, call_get_play_episode_details,
    call_get_podcast_id_from_ep, call_get_queued_episodes, call_increment_listen_time,
    call_increment_played, call_mark_episode_completed, call_queue_episode,
    call_record_listen_duration, call_remove_queued_episode, HistoryAddRequest,
    MarkEpisodeCompletedRequest, QueuePodcastRequest, RecordListenDurationRequest,
};
use gloo_timers::callback::Interval;
use js_sys::Array;
use js_sys::Object;
use std::cell::Cell;
#[cfg(not(feature = "server_build"))]
use std::path::Path;
use std::rc::Rc;
use std::string::String;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::{
    window, HtmlAudioElement, HtmlElement, HtmlInputElement, MediaPositionState,
    MediaSessionPlaybackState, Navigator,
};
use yew::prelude::*;
use yew::{function_component, html, Callback, Html};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[derive(Properties, PartialEq, Debug, Clone)]
pub struct AudioPlayerProps {
    pub src: String,
    pub title: String,
    pub description: String,
    pub release_date: String,
    pub artwork_url: String,
    pub duration: String,
    pub episode_id: i32,
    pub duration_sec: f64,
    pub start_pos_sec: f64,
    pub end_pos_sec: f64,
    pub offline: bool,
    pub is_youtube: bool,
}

#[derive(Properties, PartialEq)]
pub struct PlaybackControlProps {
    pub speed: f64,
    pub on_speed_change: Callback<f64>,
}

#[function_component(PlaybackControl)]
pub fn playback_control(props: &PlaybackControlProps) -> Html {
    let is_open = use_state(|| false);
    let toggle_open = {
        let is_open = is_open.clone();
        Callback::from(move |_: MouseEvent| {
            is_open.set(!*is_open);
        })
    };
    let on_speed_change = {
        let on_speed_change = props.on_speed_change.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Ok(speed) = input.value().parse::<f64>() {
                on_speed_change.emit(speed);
            }
        })
    };

    // Format the playback speed to show just one decimal place
    let display_speed = format!("{:.1}x", props.speed);

    html! {
        <div class="speed-control-container">
            <button
                onclick={toggle_open}
                class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center"
            >
                <i class="ph ph-speedometer text-2xl"></i>
            </button>
            <div class={classes!("speed-slider-container", "item_container-bg", (*is_open).then(|| "visible"))}>
                <div class="speed-control-content item_container-bg">
                    <div class="speed-text">
                        {display_speed}
                    </div>
                    <input
                        type="range"
                        class="speed-slider"
                        min="0.5"
                        max="2.0"
                        step="0.1"
                        value={props.speed.to_string()}
                        oninput={on_speed_change}
                    />
                </div>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct VolumeControlProps {
    pub volume: f64,
    pub on_volume_change: Callback<f64>,
}

#[function_component(VolumeControl)]
pub fn volume_control(props: &VolumeControlProps) -> Html {
    let is_open = use_state(|| false);

    let toggle_open = {
        let is_open = is_open.clone();
        Callback::from(move |_: MouseEvent| {
            is_open.set(!*is_open);
        })
    };

    let on_volume_change = {
        let on_volume_change = props.on_volume_change.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Ok(volume) = input.value().parse::<f64>() {
                on_volume_change.emit(volume);
            }
        })
    };

    html! {
        <div class="volume-control-container">
            <button
                onclick={toggle_open}
                class="skip-button audio-top-button selector-button font-bold py-2 px-4 mt-3 rounded-full w-10 h-10 flex items-center justify-center"
            >
                <i class="ph ph-speaker-high text-2xl"></i>
            </button>

            <div class={classes!("volume-slider-container", (*is_open).then(|| "visible"))}>
                <div class="volume-text">
                    {format!("{}%", (props.volume as i32))}
                </div>
                <input
                    type="range"
                    class="volume-slider"
                    min="0"
                    max="100"
                    step="1"
                    value={props.volume.to_string()}
                    oninput={on_volume_change}
                />
            </div>
        </div>
    }
}

#[function_component(AudioPlayer)]
pub fn audio_player(props: &AudioPlayerProps) -> Html {
    let audio_ref = use_node_ref();
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let show_modal = use_state(|| false);
    let on_modal_close = {
        let show_modal = show_modal.clone();
        Callback::from(move |_: MouseEvent| show_modal.set(false))
    };

    // Add error handling state
    let last_playback_position = use_state(|| 0.0);

    // Add periodic state saving
    {
        let props = props.clone();
        let audio_ref = audio_ref.clone();
        let last_position = last_playback_position.clone();

        use_effect_with((), move |_| {
            let props = props.clone();
            let audio_ref = audio_ref.clone();
            let last_position = last_position.clone();

            let interval = Interval::new(5000, move || {
                if let Some(audio) = audio_ref.cast::<HtmlAudioElement>() {
                    last_position.set(audio.current_time());

                    if let Some(window) = web_sys::window() {
                        if let Ok(Some(storage)) = window.local_storage() {
                            let _ = storage.set_item(
                                &format!("audio_position_{}", props.episode_id),
                                &audio.current_time().to_string(),
                            );
                        }
                    }
                }
            });

            move || {
                interval.cancel();
            }
        });
    }

    // Restore previous state on mount
    use_effect_with((), {
        let audio_ref = audio_ref.clone();
        let props = props.clone();

        move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(position)) =
                        storage.get_item(&format!("audio_position_{}", props.episode_id))
                    {
                        if let Ok(position) = position.parse::<f64>() {
                            if let Some(audio) = audio_ref.cast::<HtmlAudioElement>() {
                                audio.set_current_time(position);
                            }
                        }
                    }
                }
            }
            || ()
        }
    });

    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let episode_id = audio_state
        .currently_playing
        .as_ref()
        .map(|props| props.episode_id);
    let end_pos = audio_state
        .currently_playing
        .as_ref()
        .map(|props| props.end_pos_sec);
    let is_youtube_vid = audio_state
        .currently_playing
        .as_ref()
        .map(|props| props.is_youtube)
        .unwrap_or(false);
    let history = BrowserHistory::new();
    let episode_in_db = audio_state.episode_in_db.unwrap_or_default();
    let progress: UseStateHandle<f64> = use_state(|| 0.0);
    let offline_status = audio_state
        .currently_playing
        .as_ref()
        .map(|props| props.offline);
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

    let current_chapter_image = use_state(|| {
        audio_state
            .currently_playing
            .as_ref()
            .map(|props| props.artwork_url.clone())
            .unwrap_or_else(|| props.artwork_url.clone())
    });

    {
        let current_chapter_image = current_chapter_image.clone();
        let audio_state = audio_state.clone();
        let original_image_url = props.artwork_url.clone();

        use_effect_with(
            audio_state.current_time_seconds,
            move |&current_time_seconds| {
                if let Some(chapters) = &audio_state.episode_chapters {
                    let mut image_updated = false;
                    for chapter in chapters.iter().rev() {
                        if let Some(start_time) = chapter.startTime {
                            if start_time as f64 <= current_time_seconds {
                                if let Some(img) = &chapter.img {
                                    current_chapter_image.set(img.clone());
                                    image_updated = true;
                                }
                                break;
                            }
                        }
                    }
                    if !image_updated {
                        current_chapter_image.set(original_image_url.clone());
                    }
                } else {
                    current_chapter_image.set(original_image_url.clone());
                }
                || ()
            },
        );
    }

    {
        let current_chapter_image = current_chapter_image.clone();
        let audio_state = audio_state.clone();

        use_effect_with(
            audio_state.currently_playing.clone(),
            move |currently_playing| {
                if let Some(props) = currently_playing {
                    // Update the chapter image when a new episode starts playing
                    current_chapter_image.set(props.artwork_url.clone());
                }
                || ()
            },
        );
    }

    // Get episode chapters if available
    use_effect_with(
        (
            episode_id.clone(),
            user_id.clone(),
            api_key.clone(),
            server_name.clone(),
            is_youtube_vid.clone(),
        ),
        {
            let dispatch = _audio_dispatch.clone();
            move |(episode_id, user_id, api_key, server_name, is_youtube_vid)| {
                if let (Some(episode_id), Some(user_id), Some(api_key), Some(server_name)) =
                    (episode_id, user_id, api_key, server_name)
                {
                    let episode_id = *episode_id; // Dereference the option
                    let user_id = *user_id; // Dereference the option
                    let api_key = api_key.clone(); // Clone to make it owned
                    let server_name = server_name.clone(); // Clone to make it owned

                    // Only proceed if the episode_id is not zero
                    if episode_id != 0 && !is_youtube_vid {
                        wasm_bindgen_futures::spawn_local(async move {
                            let chap_request = FetchPodcasting2DataRequest {
                                episode_id,
                                user_id,
                            };
                            match call_fetch_podcasting_2_data(
                                &server_name,
                                &api_key,
                                &chap_request,
                            )
                            .await
                            {
                                Ok(response) => {
                                    let chapters = response.chapters.clone(); // Clone chapters to avoid move issue
                                    let transcripts = response.transcripts.clone(); // Clone transcripts to avoid move issue
                                    let people = response.people.clone(); // Clone people to avoid move issue
                                    dispatch.reduce_mut(|state| {
                                        state.episode_chapters = Some(chapters);
                                        state.episode_transcript = Some(transcripts);
                                        state.episode_people = Some(people);
                                    });
                                }
                                Err(e) => {
                                    web_sys::console::log_1(
                                        &format!("Error fetching chapters: {}", e).into(),
                                    );
                                }
                            }
                        });
                    }
                }
                || ()
            }
        },
    );

    // Add keyboard controls
    {
        let audio_dispatch_effect = _audio_dispatch.clone();
        let audio_state_effect = audio_state.clone();

        use_effect_with((), move |_| {
            let keydown_handler = {
                let audio_info = audio_dispatch_effect.clone();
                let state = audio_state_effect.clone();

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
                        match event.key().as_str() {
                            " " => {
                                event.prevent_default();
                                audio_info.reduce_mut(|state| state.toggle_playback());
                            }
                            "ArrowRight" => {
                                event.prevent_default();
                                if let Some(audio_element) = state.audio_element.as_ref() {
                                    let new_time = audio_element.current_time() + 15.0;
                                    audio_element.set_current_time(new_time);
                                    audio_info
                                        .reduce_mut(|state| state.update_current_time(new_time));
                                }
                            }
                            "ArrowLeft" => {
                                event.prevent_default();
                                if let Some(audio_element) = state.audio_element.as_ref() {
                                    let new_time = (audio_element.current_time() - 15.0).max(0.0);
                                    audio_element.set_current_time(new_time);
                                    audio_info
                                        .reduce_mut(|state| state.update_current_time(new_time));
                                }
                            }
                            _ => {}
                        }
                    }
                }) as Box<dyn FnMut(_)>)
            };

            window()
                .unwrap()
                .add_event_listener_with_callback(
                    "keydown",
                    keydown_handler.as_ref().unchecked_ref(),
                )
                .unwrap();

            move || {
                keydown_handler.forget();
            }
        });
    }

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
            //print the ep id
            let interval_handle: Rc<Cell<Option<Interval>>> = Rc::new(Cell::new(None));
            let interval_handle_clone = interval_handle.clone();
            let interval = Interval::new(1000, move || {
                if let Some(audio_element) = state_clone.audio_element.as_ref() {
                    let time_in_seconds = audio_element.current_time();
                    let duration = audio_element.duration();

                    // Time updates happen regardless of duration
                    let hours = (time_in_seconds / 3600.0).floor() as i32;
                    let minutes = ((time_in_seconds % 3600.0) / 60.0).floor() as i32;
                    let seconds = (time_in_seconds % 60.0).floor() as i32;
                    let formatted_time = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

                    let progress_percentage = if duration > 0.0 && !duration.is_nan() {
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

                    // Episode completion check only happens when we have valid duration
                    if !duration.is_nan() && duration > 0.0 {
                        let end_pos_sec = end_pos.clone();
                        let complete_api_key = closure_api_key.clone();
                        let complete_server_name = closure_server_name.clone();
                        let complete_user_id = closure_user_id.clone();
                        let complete_episode_id = closure_episode_id.clone();
                        let offline_status_loop = offline_status.unwrap_or(false);
                        if time_in_seconds >= (duration - end_pos_sec.unwrap()) {
                            web_sys::console::log_1(&"Episode completed".into());
                            audio_element.pause().unwrap_or(());
                            // Manually trigger the `ended` event
                            let event = web_sys::Event::new("ended").unwrap();
                            audio_element.dispatch_event(&event).unwrap();
                            // Call the endpoint to mark episode as completed
                            if offline_status_loop {
                                // If offline, store the episode in the local database
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
                                            user_id: *complete_user_id, // Dereference the option
                                            is_youtube: is_youtube_vid,
                                        };

                                        match call_mark_episode_completed(
                                            &complete_server_name,
                                            &complete_api_key,
                                            &request,
                                        )
                                        .await
                                        {
                                            Ok(_) => {}
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
                        }
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
                } else {
                    if state_clone.audio_playing.unwrap_or_default() {
                        if let Some(audio_element) = state_clone.audio_element.as_ref() {
                            let listen_duration = audio_element.current_time();
                            let request_data = RecordListenDurationRequest {
                                episode_id: episode_id_loop.unwrap().clone(),
                                user_id: user_id.unwrap().clone(),
                                listen_duration,
                                is_youtube: Some(is_youtube_vid),
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
                                            is_youtube: current_episode.is_youtube,
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
                                                next_episode.episodedescription.clone(),
                                                next_episode.episodepubdate.clone(),
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
                                                Some(next_episode.is_youtube.clone()),
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

    {
        let audio_state = audio_state.clone();
        let audio_dispatch = _audio_dispatch.clone();
        let audio_state_clone = audio_state.clone();

        use_effect_with(audio_state.clone(), move |_| {
            if let Some(window) = web_sys::window() {
                let navigator: Navigator = window.navigator();

                // Try to get media session
                if let Ok(media_session) =
                    js_sys::Reflect::get(&navigator, &JsValue::from_str("mediaSession"))
                {
                    // Safely attempt to convert to MediaSession
                    if let Ok(media_session) = media_session.dyn_into::<web_sys::MediaSession>() {
                        // Update metadata if we have something playing
                        if let Some(audio_props) = &audio_state.currently_playing {
                            // Try to create new metadata
                            if let Ok(metadata) = web_sys::MediaMetadata::new() {
                                metadata.set_title(&audio_props.title);

                                // Create artwork array
                                let artwork_array = Array::new();
                                let artwork_object = Object::new();

                                // Set up artwork properties
                                let _ = js_sys::Reflect::set(
                                    &artwork_object,
                                    &"src".into(),
                                    &audio_props.artwork_url.clone().into(),
                                );
                                let _ = js_sys::Reflect::set(
                                    &artwork_object,
                                    &"sizes".into(),
                                    &"512x512".into(),
                                );
                                let _ = js_sys::Reflect::set(
                                    &artwork_object,
                                    &"type".into(),
                                    &"image/jpeg".into(),
                                );

                                artwork_array.push(&artwork_object);
                                metadata.set_artwork(&artwork_array.into());
                                media_session.set_metadata(Some(&metadata));

                                // Set playback state
                                if audio_state_clone.audio_playing.unwrap() {
                                    media_session
                                        .set_playback_state(MediaSessionPlaybackState::Playing);
                                } else {
                                    media_session
                                        .set_playback_state(MediaSessionPlaybackState::Paused);
                                }

                                // Update position state
                                if let Some(audio_element) = &audio_state_clone.audio_element {
                                    let duration = audio_props.duration_sec;
                                    if !duration.is_nan() && duration > 0.0 {
                                        let position_state = MediaPositionState::new();
                                        position_state.set_duration(duration);
                                        position_state
                                            .set_playback_rate(audio_state_clone.playback_speed);
                                        position_state.set_position(audio_element.current_time());
                                        let _ = media_session
                                            .set_position_state_with_state(&position_state);
                                    }
                                }
                            }
                            // Inside your use_effect_with block, after setting up the initial position state:
                            // Inside your media session setup use_effect:
                            if let Some(audio_element) = &audio_state_clone.audio_element {
                                let media_session_clone = media_session.clone();
                                let audio_state_for_callback = audio_state_clone.clone();
                                let audio_element_clone = audio_element.clone();
                                let timeupdate_callback = Closure::wrap(Box::new(move || {
                                    let duration = audio_element_clone.duration();
                                    // Only update position state if we have a valid duration
                                    if !duration.is_nan() && duration > 0.0 {
                                        let position_state = MediaPositionState::new();
                                        position_state.set_duration(duration);
                                        position_state.set_playback_rate(
                                            audio_state_for_callback.playback_speed,
                                        );
                                        position_state
                                            .set_position(audio_element_clone.current_time());
                                        let _ = media_session_clone
                                            .set_position_state_with_state(&position_state);
                                    }
                                })
                                    as Box<dyn FnMut()>);

                                audio_element.set_ontimeupdate(Some(
                                    timeupdate_callback.as_ref().unchecked_ref(),
                                ));
                                timeupdate_callback.forget();
                            }
                        }

                        // Set up action handlers
                        let audio_dispatch_play = audio_dispatch.clone();
                        let play_pause_callback = Closure::wrap(Box::new(move || {
                            audio_dispatch_play.reduce_mut(UIState::toggle_playback);
                        })
                            as Box<dyn FnMut()>);

                        // Set play/pause handlers
                        let _ = media_session.set_action_handler(
                            web_sys::MediaSessionAction::Play,
                            Some(play_pause_callback.as_ref().unchecked_ref()),
                        );
                        let _ = media_session.set_action_handler(
                            web_sys::MediaSessionAction::Pause,
                            Some(play_pause_callback.as_ref().unchecked_ref()),
                        );
                        play_pause_callback.forget();

                        // Set up seek backward handler
                        let audio_state_back = audio_state.clone();
                        let audio_dispatch_back = audio_dispatch.clone();
                        let seek_backward_callback = Closure::wrap(Box::new(move || {
                            if let Some(audio_element) = audio_state_back.audio_element.as_ref() {
                                let new_time = audio_element.current_time() - 15.0;
                                let _ = audio_element.set_current_time(new_time);
                                audio_dispatch_back
                                    .reduce_mut(|state| state.update_current_time(new_time));
                            }
                        })
                            as Box<dyn FnMut()>);

                        let _ = media_session.set_action_handler(
                            web_sys::MediaSessionAction::Seekbackward,
                            Some(seek_backward_callback.as_ref().unchecked_ref()),
                        );
                        seek_backward_callback.forget();

                        // Set up seek forward handler
                        let audio_state_fwd = audio_state.clone();
                        let audio_dispatch_fwd = audio_dispatch.clone();
                        let seek_forward_callback = Closure::wrap(Box::new(move || {
                            if let Some(audio_element) = audio_state_fwd.audio_element.as_ref() {
                                let new_time = audio_element.current_time() + 15.0;
                                let _ = audio_element.set_current_time(new_time);
                                audio_dispatch_fwd
                                    .reduce_mut(|state| state.update_current_time(new_time));
                            }
                        })
                            as Box<dyn FnMut()>);

                        let _ = media_session.set_action_handler(
                            web_sys::MediaSessionAction::Seekforward,
                            Some(seek_forward_callback.as_ref().unchecked_ref()),
                        );
                        seek_forward_callback.forget();
                    }
                }
            }

            || ()
        });
    }

    // Toggle playback
    let toggle_playback = {
        let dispatch = _audio_dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(UIState::toggle_playback);
        })
    };

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
                                next_episode.episodedescription.clone(),
                                next_episode.episodepubdate.clone(),
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
                                Some(next_episode.is_youtube.clone()),
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

    let on_chapter_click = {
        let audio_dispatch = _audio_dispatch.clone();
        Callback::from(move |start_time: i32| {
            let start_time = start_time as f64;
            audio_dispatch.reduce_mut(|state| {
                if let Some(audio_element) = state.audio_element.as_ref() {
                    audio_element.set_current_time(start_time);
                    state.current_time_seconds = start_time;

                    // Update formatted time
                    let hours = (start_time / 3600.0).floor() as i32;
                    let minutes = ((start_time % 3600.0) / 60.0).floor() as i32;
                    let seconds = (start_time % 60.0).floor() as i32;
                    state.current_time_formatted =
                        format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
                }
            });
        })
    };

    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
    }

    let page_state = use_state(|| PageState::Hidden);

    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
        })
    };

    let on_chapter_select = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Shown);
        })
    };
    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });
    let audio_dispatch = _audio_dispatch.clone();
    let chapter_select_modal = html! {
        <div id="chapter-select-modal" tabindex="-1" aria-hidden="true"
            class="chapter-select-modal fixed top-0 right-0 left-0 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25"
            onclick={on_close_modal.clone()}>  // Add this onclick handler
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow"
                onclick={stop_propagation.clone()}>  // Add this to prevent clicks inside the modal from closing it
                <div class="modal-container relative rounded-lg shadow">
                    // Header remains the same
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">{"Chapters"}</h3>
                        <button onclick={on_close_modal.clone()}
                            class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>

                    // Updated chapters list
                    <div class="p-4 md:p-5 max-h-[70vh] overflow-y-auto">
                        { if let Some(chapters) = &audio_state.episode_chapters {
                            if let Some(audio_props) = &audio_state.currently_playing {
                                chapters.iter().enumerate().map(|(index, chapter)| {
                                    let start_time = chapter.startTime.unwrap_or_default() as f64;
                                    let end_time = if index < chapters.len() - 1 {
                                        chapters[index + 1].startTime.unwrap_or_default() as f64
                                    } else {
                                        audio_props.duration_sec
                                    };
                                    let chapter_duration = end_time - start_time;

                                    // Calculate if this is the current chapter
                                    let is_current_chapter = audio_state.current_time_seconds >= start_time
                                        && audio_state.current_time_seconds < end_time;

                                    // Calculate progress for this chapter
                                    let chapter_progress = if is_current_chapter {
                                        ((audio_state.current_time_seconds - start_time) / chapter_duration * 100.0)
                                            .clamp(0.0, 100.0)
                                    } else if audio_state.current_time_seconds >= end_time {
                                        100.0
                                    } else {
                                        0.0
                                    };

                                    let formatted_start = format_time_rm_hour(start_time);
                                    let click_start_time = start_time;
                                    let on_chapter_click = on_chapter_click.clone();
                                    let on_chapter_click_button = on_chapter_click.clone();

                                    let click_handler = {
                                        let dispatch = audio_dispatch.clone();
                                        Callback::from(move |_| {
                                            if is_current_chapter {
                                                dispatch.reduce_mut(UIState::toggle_playback);
                                            } else {
                                                on_chapter_click.emit(click_start_time as i32);
                                            }
                                        })
                                    };
                                    let button_click_handler = {
                                        let dispatch = audio_dispatch.clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.stop_propagation();
                                            if is_current_chapter {
                                                dispatch.reduce_mut(UIState::toggle_playback);
                                            } else {
                                                on_chapter_click_button.emit(click_start_time as i32);
                                            }
                                        })
                                    };

                                    html! {
                                        <div
                                            class={classes!(
                                                "chapter-item",
                                                is_current_chapter.then(|| "current-chapter")
                                            )}
                                            onclick={click_handler}
                                        >
                                            <button
                                                class="chapter-play-button"
                                                onclick={button_click_handler}
                                            >
                                                if is_current_chapter && audio_state.audio_playing.unwrap_or(false) {
                                                    <i class="ph ph-pause text-xl"></i>
                                                } else {
                                                    <i class="ph ph-play text-xl"></i>
                                                }
                                            </button>
                                            <div class="chapter-info">
                                                <span class="chapter-title">{ &chapter.title }</span>
                                                <span class="chapter-time">{ formatted_start }</span>
                                                // Progress bar
                                                <div class="chapter-progress-container">
                                                    <div
                                                        class="chapter-progress-bar"
                                                        style={format!("width: {}%", chapter_progress)}
                                                    />
                                                </div>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Html>()
                            } else {
                                html! { <div class="text-center p-4">{"No audio playing"}</div> }
                            }
                        } else {
                            html! { <div class="text-center p-4">{"No chapters available"}</div> }
                        }}
                    </div>
                </div>
            </div>
        </div>
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
            let show_modal = show_modal.clone();

            Callback::from(move |_: MouseEvent| {
                show_modal.set(true); // Show modal instead of navigating
            })
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
            <>
            {
                match *page_state {
                PageState::Shown => chapter_select_modal,
                _ => html! {},
                }
            }
            <div class={audio_bar_class} ref={container_ref.clone()}>
                <div class="top-section">
                    <div>
                    <button onclick={title_click.clone()} class="retract-button">
                        <div class="drawer-text flex items-center justify-center">
                            <i class="ph ph-caret-circle-down text-4xl"></i>
                        </div>
                    </button>
                    <div onclick={title_click.clone()} class="audio-image-container">
                        <img src={(*current_chapter_image).clone()} />
                    </div>
                    <div class="title" onclick={title_click.clone()}>{ &audio_props.title }
                    </div>
                    // Desktop scrubber
                    <div class="flex-grow flex items-center sm:block hidden">
                        <div class="flex items-center flex-nowrap">
                            <span class="time-display px-2">{audio_state.current_time_formatted.clone()}</span>
                            <input type="range"
                                class="flex-grow h-1 cursor-pointer"
                                min="0.0"
                                max={audio_props.duration_sec.to_string().clone()}
                                value={audio_state.current_time_seconds.to_string()}
                                oninput={update_time.clone()} />
                            <span class="time-display px-2">{formatted_duration.clone()}</span>
                        </div>
                    </div>
                    // Mobile scrubber
                    <div class="w-full flex items-center justify-center sm:hidden">
                        <div class="flex items-center flex-nowrap w-full px-4">
                            <span class="time-display px-2">{audio_state.current_time_formatted.clone()}</span>
                            <input type="range"
                                class="flex-grow h-1 cursor-pointer"
                                min="0.0"
                                max={audio_props.duration_sec.to_string().clone()}
                                value={audio_state.current_time_seconds.to_string()}
                                oninput={update_time.clone()} />
                            <span class="time-display px-2">{formatted_duration.clone()}</span>
                        </div>
                    </div>

                    <div class="episode-button-container flex items-center justify-center">
                        {
                            html! {
                                <>
                                    <PlaybackControl
                                        speed={audio_state.playback_speed}
                                        on_speed_change={update_playback_closure}
                                    />
                                </>
                            }
                        }
                        <button onclick={skip_backward.clone()} class="pronounce-mobile rewind-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <i class="ph ph-rewind md:text-2xl text-4xl"></i>
                        </button>
                        <button onclick={toggle_playback.clone()} class="pronounce-mobile audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                                { if audio_state.audio_playing.unwrap_or(false) {
                                    html! { <i class="ph ph-pause md:text-2xl text-4xl"></i> }
                                } else {
                                    html! { <i class="ph ph-play md:text-2xl text-4xl"></i> }
                                }}
                        </button>
                        <button onclick={skip_forward.clone()} class="pronounce-mobile skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <i class="ph ph-fast-forward md:text-2xl text-4xl"></i>
                        </button>
                        <button onclick={skip_episode.clone()} class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <i class="ph ph-skip-forward text-2xl"></i>
                        </button>
                    </div>

                    <div class="episode-button-container flex items-center justify-center">
                    {
                        if episode_in_db {
                            html! {
                                <>
                                <button onclick={Callback::from(move |e: MouseEvent| {
                                    on_shownotes_click.emit(e.clone());
                                    // title_click_emit.emit(e);
                                })} class="audio-top-button audio-full-button border-solid border selector-button font-bold py-2 px-4 mt-3 rounded-full flex items-center justify-center">
                                    { "Shownotes" }
                                </button>
                                {
                                    if let Some(chapters) = &audio_state.episode_chapters {
                                        if !chapters.is_empty() {
                                            html! {
                                                <button onclick={Callback::from(move |_: MouseEvent| {
                                                    on_chapter_select.emit(());
                                                })} class="audio-top-button audio-full-button border-solid border selector-button font-bold py-2 px-4 mt-3 rounded-full flex items-center justify-center">
                                                    { "Chapters" }
                                                </button>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                </>
                            }
                        } else {
                            html! {
                                <button disabled=true class="item-container-button audio-full-button border-solid border selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center opacity-50 cursor-not-allowed">
                                    { "Shownotes (Unavailable)" }
                                </button>
                            }
                        }
                    }
                    <VolumeControl
                        volume={audio_state.audio_volume}
                        on_volume_change={update_volume_closure}
                    />
                    </div>
                    </div>

                </div>
                <div class="line-content">
                <div class="mobile-progress-container md:hidden">
                    <div
                        class="mobile-progress-bar"
                        style={format!("width: {}%",
                            (audio_state.current_time_seconds / audio_props.duration_sec * 100.0).clamp(0.0, 100.0)
                        )}
                    />
                </div>
                <div class="left-group">
                    <div onclick={title_click.clone()} class="artwork-container">
                        <FallbackImage
                            src={audio_props.artwork_url.clone()}
                            alt={format!("Cover for audio")}
                            class={Some(artwork_class.to_string())}
                        />
                    </div>
                    <div class="title" onclick={title_click.clone()}>
                        <span>{ &audio_props.title }</span>
                    </div>
                </div>
                <div class="right-group">
                    <button onclick={toggle_playback} class="audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                        { if audio_state.audio_playing.unwrap_or(false) {
                            html! { <i class="ph ph-pause text-2xl"></i> }
                        } else {
                            html! { <i class="ph ph-play text-2xl"></i> }
                        }}
                    </button>
                    <button onclick={skip_forward} class="audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                        <i class="ph ph-fast-forward text-2xl"></i>
                    </button>
                    <div class="flex-grow flex items-center md:block hidden">
                        <div class="flex items-center flex-nowrap">
                            <span class="time-display px-2">{audio_state.current_time_formatted.clone()}</span>
                            <input type="range"
                                class="flex-grow h-1 cursor-pointer"
                                min="0.0"
                                max={audio_props.duration_sec.to_string().clone()}
                                value={audio_state.current_time_seconds.to_string()}
                                oninput={update_time.clone()} />
                            <span class="time-display px-2">{formatted_duration.clone()}</span>
                        </div>
                    </div>
                </div>
            </div>
            </div>
            {
                if *show_modal && audio_state.currently_playing.is_some() {
                    let props = audio_state.currently_playing.as_ref().unwrap();
                    let listen_duration_percentage = if props.duration_sec > 0.0 {
                        (audio_state.current_time_seconds / props.duration_sec) * 100.0
                    } else {
                        0.0
                    };

                    // Navigation callback for the "Go to Episode Page" button
                    let nav_to_episode = {
                        let history = history.clone();
                        let dispatch = _dispatch.clone();
                        let episode_id = props.episode_id;
                        let show_modal = show_modal.clone();
                        let title_click = title_click.clone();

                        Callback::from(move |e: MouseEvent| {
                            show_modal.set(false);  // Close modal before navigation
                            title_click.emit(e);
                            let dispatch_clone = dispatch.clone();
                            let history_clone = history.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                dispatch_clone.reduce_mut(move |state| {
                                    state.selected_episode_id = Some(episode_id);
                                    state.fetched_episode = None;
                                });
                                history_clone.push("/episode");
                            });
                        })
                    };

                    html! {
                        <EpisodeModal
                            episode_id={props.episode_id}
                            episode_url={props.src.clone()}
                            episode_artwork={props.artwork_url.clone()}
                            episode_title={props.title.clone()}
                            description={props.description.clone()}  // You might need to fetch this
                            format_release={props.release_date.clone()}
                            duration={audio_props.duration_sec as i32}
                            on_close={on_modal_close.clone()}
                            on_show_notes={nav_to_episode}
                            listen_duration_percentage={listen_duration_percentage}
                            is_youtube={props.is_youtube}
                        />
                    }
                } else {
                    html! {}
                }
            }
            </>
        }
    } else {
        html! {}
    }
}

pub fn on_play_pause(
    episode_url_for_closure: String,
    episode_title_for_closure: String,
    episode_description_for_closure: String,
    episode_release_date_for_closure: String,
    episode_artwork_for_closure: String,
    episode_duration_for_closure: i32,
    episode_id_for_closure: i32,
    listen_duration_for_closure: Option<i32>,
    api_key: String,
    user_id: i32,
    server_name: String,
    audio_dispatch: Dispatch<UIState>,
    audio_state: Rc<UIState>,
    is_local: Option<bool>,
    is_youtube_vid: Option<bool>,
) -> Callback<MouseEvent> {
    Callback::from(move |e: MouseEvent| {
        let episode_url_for_play = episode_url_for_closure.clone();
        let episode_title_for_play = episode_title_for_closure.clone();
        let episode_description_for_play = episode_description_for_closure.clone();
        let episode_release_date_for_play = episode_release_date_for_closure.clone();
        let episode_artwork_for_play = episode_artwork_for_closure.clone();
        let episode_duration_for_play = episode_duration_for_closure.clone();
        let episode_id_for_play = episode_id_for_closure.clone();
        let server_play = server_name.clone();
        let api_play = api_key.clone();
        let audio_dis_play = audio_dispatch.clone();
        let audio_state_play = audio_state.clone();
        // Changed from '_' to 'e'
        let is_current = audio_state
            .currently_playing
            .as_ref()
            .map_or(false, |current| {
                current.episode_id == episode_id_for_closure
            });
        if is_current {
            audio_dispatch.reduce_mut(|state| {
                let currently_playing = state.audio_playing.unwrap_or(false);
                state.audio_playing = Some(!currently_playing);
                if let Some(audio) = &state.audio_element {
                    if currently_playing {
                        let _ = audio.pause();
                    } else {
                        let _ = audio.play();
                    }
                }
            });
        } else {
            on_play_click(
                episode_url_for_play,
                episode_title_for_play,
                episode_description_for_play,
                episode_release_date_for_play,
                episode_artwork_for_play,
                episode_duration_for_play,
                episode_id_for_play,
                listen_duration_for_closure,
                api_play,
                user_id,
                server_play,
                audio_dis_play,
                audio_state_play,
                is_local,
                is_youtube_vid,
            )
            .emit(e); // Pass the event instead of '_'
        }
    })
}

pub fn on_play_click(
    episode_url_for_closure: String,
    episode_title_for_closure: String,
    episode_description_for_closure: String,
    episode_release_date_for_closure: String,
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
    is_youtube_vid: Option<bool>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        let episode_url_for_closure = episode_url_for_closure.clone();
        let episode_title_for_closure = episode_title_for_closure.clone();
        let episode_description_for_closure = episode_description_for_closure.clone();
        let episode_release_date_for_closure = episode_release_date_for_closure.clone();
        let episode_artwork_for_closure = episode_artwork_for_closure.clone();
        let episode_duration_for_closure = episode_duration_for_closure.clone();
        let listen_duration_for_closure = listen_duration_for_closure.clone();
        let episode_id_for_closure = episode_id_for_closure.clone();
        let episode_is_youtube = is_youtube_vid.clone().unwrap();
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
        let episode_description_for_wasm = episode_description_for_closure.clone();
        let episode_release_date_for_wasm = episode_release_date_for_closure.clone();
        let episode_url_for_wasm = call_ep_url.clone();
        let episode_artwork_for_wasm = episode_artwork_for_closure.clone();
        let episode_duration_for_wasm = episode_duration_for_closure.clone();
        let episode_id_for_wasm = episode_id_for_closure.clone();
        let app_dispatch = audio_dispatch.clone();
        let episode_url = episode_url_for_wasm.clone();
        let episode_title = episode_title_for_wasm.clone();

        web_sys::console::log_1(&JsValue::from_str(&episode_id_for_wasm.to_string()));
        spawn_local(async move {
            // Check if the episode exists in the database (your existing code)
            let mut episode_exists = call_check_episode_in_db(
                &check_server_name.clone(),
                &check_api_key.clone(),
                check_user_id.clone(),
                &episode_title.clone(),
                &episode_url.clone(),
            )
            .await
            .unwrap_or(false);

            let mut episode_id = episode_id_for_wasm;

            // If the episode exists but the current `episode_id` is `0`, retrieve the correct `episode_id`
            if episode_exists && episode_id == 0 {
                match call_get_episode_id(
                    &check_server_name,
                    &check_api_key,
                    &check_user_id,
                    &episode_title,
                    &episode_url,
                    episode_is_youtube,
                )
                .await
                {
                    Ok(new_episode_id) => {
                        if new_episode_id == 0 {
                            web_sys::console::log_1(&JsValue::from_str(
                                "Episode ID returned is still 0, setting episode_exists to false",
                            ));
                            episode_exists = false;
                        } else {
                            web_sys::console::log_1(&JsValue::from_str(&format!(
                                "New episode ID: {}",
                                new_episode_id
                            )));
                            episode_id = new_episode_id;
                        }
                    }
                    Err(_) => {
                        web_sys::console::log_1(&JsValue::from_str(
                            "Failed to get episode ID, setting episode_exists to false",
                        ));
                        episode_exists = false;
                    }
                }
            }
            web_sys::console::log_1(&JsValue::from_str(&format!(
                "post episode ID: {}",
                episode_id
            )));

            web_sys::console::log_1(&JsValue::from_str(&format!(
                "Episode exists: {}",
                episode_exists
            )));

            // Update the global state to indicate whether the episode exists in the DB
            app_dispatch.reduce_mut(move |global_state| {
                global_state.episode_in_db = Some(episode_exists);
            });

            // Now proceed with adding the history entry if the episode exists
            if episode_exists {
                let history_server_name = check_server_name.clone();
                let history_api_key = check_api_key.clone();

                let history_add = HistoryAddRequest {
                    episode_id,
                    episode_pos,
                    user_id,
                    is_youtube: episode_is_youtube,
                };

                let add_history_future =
                    call_add_history(&history_server_name, history_api_key, &history_add);
                match add_history_future.await {
                    Ok(_) => {}
                    Err(e) => {
                        web_sys::console::log_1(&JsValue::from_str(&format!(
                            "Failed to add history: {:?}",
                            e
                        )));
                    }
                }

                let queue_server_name = check_server_name.clone();
                let queue_api_key = check_api_key.clone();

                let request = QueuePodcastRequest {
                    episode_id,
                    user_id,
                    is_youtube: episode_is_youtube,
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
                    web_sys::console::log_1(&format!("Failed to increment: {:?}", _e).into());
                }
            }
        });

        // Determine the source URL
        let src = if episode_url_for_wasm.contains("youtube.com") {
            format!(
                "{}/api/data/stream/{}?api_key={}&user_id={}&type=youtube",
                server_name, episode_id, api_key, user_id
            )
        } else if is_local.unwrap_or(false) {
            format!(
                "{}/api/data/stream/{}?api_key={}&user_id={}",
                server_name, episode_id, api_key, user_id
            )
        } else {
            episode_url_for_wasm.clone()
        };

        // NEW CODE: Analyze the actual audio duration before playing
        let src_for_analysis = src.clone();
        let audio_dispatch_for_duration = audio_dispatch.clone();
        let server_name_for_player = server_name.clone();
        let api_key_for_player = api_key.clone();

        wasm_bindgen_futures::spawn_local(async move {
            // Function to get actual duration from audio file
            async fn get_actual_duration(audio_src: &str) -> Option<f64> {
                use wasm_bindgen::JsCast;
                use wasm_bindgen_futures::JsFuture;

                // Create a temporary audio element
                let window = web_sys::window()?;
                let document = window.document()?;
                let audio_element = document.create_element("audio").ok()?;
                let audio: HtmlAudioElement = audio_element.dyn_into().ok()?;

                // Set the source
                audio.set_src(audio_src);

                // Create a promise that resolves when metadata is loaded
                let promise = js_sys::Promise::new(&mut |resolve, reject| {
                    let resolve_clone = resolve.clone();
                    let reject_clone = reject.clone();
                    let big_audio = audio.clone();
                    // Set up loadedmetadata event listener
                    let onloadedmetadata = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        let duration = big_audio.duration();
                        if !duration.is_nan() && duration > 0.0 {
                            resolve_clone
                                .call1(&JsValue::UNDEFINED, &JsValue::from_f64(duration))
                                .unwrap();
                        } else {
                            reject_clone
                                .call1(&JsValue::UNDEFINED, &JsValue::from_str("Invalid duration"))
                                .unwrap();
                        }
                    })
                        as Box<dyn FnMut(_)>);

                    // Set up error handler
                    let onerror = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        reject
                            .call1(
                                &JsValue::UNDEFINED,
                                &JsValue::from_str("Failed to load metadata"),
                            )
                            .unwrap();
                    }) as Box<dyn FnMut(_)>);

                    audio.set_onloadedmetadata(Some(onloadedmetadata.as_ref().unchecked_ref()));
                    audio.set_onerror(Some(onerror.as_ref().unchecked_ref()));

                    // Preload metadata only
                    audio.set_preload("metadata");
                    audio.load();

                    // Prevent closures from being dropped
                    onloadedmetadata.forget();
                    onerror.forget();
                });

                // Convert promise to future and await it with timeout
                match JsFuture::from(promise).await {
                    Ok(value) => value.as_f64(),
                    Err(_) => None,
                }
            }

            // Get the actual duration
            let actual_duration_sec = get_actual_duration(&src_for_analysis).await;

            // Use the actual duration if available, otherwise fall back to provided duration
            let final_duration_sec =
                actual_duration_sec.unwrap_or(episode_duration_for_wasm as f64);

            web_sys::console::log_1(&JsValue::from_str(&format!(
                "Original duration: {}s, Actual duration: {}s",
                episode_duration_for_wasm, final_duration_sec
            )));

            // Continue with the rest of your existing code...
            if episode_id != 0 {
                match call_get_podcast_id_from_ep(
                    &server_name_for_player,
                    &Some(api_key_for_player.clone()),
                    episode_id,
                    user_id,
                    Some(episode_is_youtube.clone()),
                )
                .await
                {
                    Ok(podcast_id) => {
                        match call_get_play_episode_details(
                            &server_name_for_player,
                            &Some(api_key_for_player.clone()),
                            user_id,
                            podcast_id,
                            episode_is_youtube,
                        )
                        .await
                        {
                            Ok((playback_speed, start_skip, end_skip)) => {
                                let start_pos_sec =
                                    listen_duration_for_closure.unwrap_or(0).max(start_skip) as f64;
                                let end_pos_sec = end_skip as f64;

                                audio_dispatch_for_duration.reduce_mut(move |audio_state| {
                                    audio_state.audio_playing = Some(true);
                                    // Use the returned playback speed instead of hardcoded 1.0
                                    audio_state.playback_speed = playback_speed as f64;
                                    audio_state.audio_volume = 100.0;
                                    audio_state.offline = Some(false);
                                    audio_state.currently_playing = Some(AudioPlayerProps {
                                        src: src.clone(),
                                        title: episode_title_for_wasm.clone(),
                                        description: episode_description_for_wasm.clone(),
                                        release_date: episode_release_date_for_wasm.clone(),
                                        artwork_url: episode_artwork_for_wasm.clone(),
                                        duration: format!("{}", final_duration_sec as i32), // Use actual duration
                                        episode_id: episode_id_for_wasm.clone(),
                                        duration_sec: final_duration_sec, // Use actual duration
                                        start_pos_sec,
                                        end_pos_sec: end_pos_sec as f64,
                                        offline: false,
                                        is_youtube: episode_is_youtube.clone(),
                                    });
                                    audio_state.set_audio_source(src.to_string());
                                    if let Some(audio) = &audio_state.audio_element {
                                        audio.set_current_time(start_pos_sec);
                                        // Set the playback speed on the audio element as well
                                        audio.set_playback_rate(playback_speed as f64);
                                        let _ = audio.play();
                                    }
                                    audio_state.audio_playing = Some(true);
                                });
                            }

                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error getting episode detail: {}", e).into(),
                                );
                            }
                        }
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Error getting podcast ID: {}", e).into());
                    }
                };
            } else {
                // Directly play the episode without skip times
                audio_dispatch_for_duration.reduce_mut(move |audio_state| {
                    audio_state.audio_playing = Some(true);
                    audio_state.playback_speed = 1.0;
                    audio_state.audio_volume = 100.0;
                    audio_state.offline = Some(false);
                    audio_state.currently_playing = Some(AudioPlayerProps {
                        src: src.clone(),
                        title: episode_title_for_wasm.clone(),
                        description: episode_description_for_wasm.clone(),
                        release_date: episode_release_date_for_wasm.clone(),
                        artwork_url: episode_artwork_for_wasm.clone(),
                        duration: format!("{}", final_duration_sec as i32), // Use actual duration
                        episode_id: episode_id_for_wasm.clone(),
                        duration_sec: final_duration_sec, // Use actual duration
                        start_pos_sec: 0.0,
                        end_pos_sec: 0.0,
                        offline: false,
                        is_youtube: episode_is_youtube.clone(),
                    });
                    audio_state.set_audio_source(src.to_string());
                    if let Some(audio) = &audio_state.audio_element {
                        let _ = audio.play();
                    }
                    audio_state.audio_playing = Some(true);
                });
            }
        });
    })
}

#[cfg(not(feature = "server_build"))]
pub fn on_play_pause_offline(
    episode_info: EpisodeDownload,
    audio_dispatch: Dispatch<UIState>,
    audio_state: Rc<UIState>,
    app_state: Dispatch<AppState>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        let episode_info_for_closure = episode_info.clone();
        let audio_dispatch = audio_dispatch.clone();
        let app_state = app_state.clone();

        let is_current = audio_state
            .currently_playing
            .as_ref()
            .map_or(false, |current| {
                current.episode_id == episode_info.episodeid
            });

        if is_current {
            audio_dispatch.reduce_mut(|state| {
                let currently_playing = state.audio_playing.unwrap_or(false);
                state.audio_playing = Some(!currently_playing);
                if let Some(audio) = &state.audio_element {
                    if currently_playing {
                        let _ = audio.pause();
                    } else {
                        let _ = audio.play();
                    }
                }
            });
        } else {
            on_play_click_offline(episode_info_for_closure, audio_dispatch, app_state)
                .emit(MouseEvent::new("click").unwrap());
        }
    })
}

#[cfg(not(feature = "server_build"))]
pub fn on_play_click_offline(
    episode_info: EpisodeDownload,
    audio_dispatch: Dispatch<UIState>,
    app_dispatch: Dispatch<AppState>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        let episode_info_for_closure = episode_info.clone();
        let audio_dispatch = audio_dispatch.clone();
        let app_dispatch = app_dispatch.clone();

        // Early return if downloadedlocation is None
        let file_path = match episode_info_for_closure.downloadedlocation {
            Some(path) => path,
            None => {
                app_dispatch.reduce_mut(|state| {
                    state.error_message = Some("Episode file location not found".to_string());
                });
                return;
            }
        };

        let episode_title_for_wasm = episode_info_for_closure.episodetitle.clone();
        let episode_description_for_wasm = episode_info_for_closure.episodedescription.clone();
        let episode_release_date_for_wasm = episode_info_for_closure.episodepubdate.clone();
        let episode_artwork_for_wasm = episode_info_for_closure.episodeartwork.clone();
        let episode_duration_for_wasm = episode_info_for_closure.episodeduration.clone();
        let episode_id_for_wasm = episode_info_for_closure.episodeid.clone();
        let listen_duration_for_closure = episode_info_for_closure.listenduration.clone();
        let episode_is_youtube_for_wasm = episode_info.is_youtube.clone();

        wasm_bindgen_futures::spawn_local(async move {
            match start_local_file_server(&file_path).await {
                Ok(server_url) => {
                    let file_name = Path::new(&file_path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("");
                    let src = format!("{}/{}", server_url, file_name);

                    // NEW: Analyze duration before playing
                    let src_for_analysis = src.clone();
                    let audio_dispatch_for_duration = audio_dispatch.clone();

                    // Function to get actual duration from audio file
                    async fn get_actual_duration(audio_src: &str) -> Option<f64> {
                        use wasm_bindgen::JsCast;
                        use wasm_bindgen_futures::JsFuture;

                        // Create a temporary audio element
                        let window = web_sys::window()?;
                        let document = window.document()?;
                        let audio_element = document.create_element("audio").ok()?;
                        let audio: HtmlAudioElement = audio_element.dyn_into().ok()?;

                        // Set the source
                        audio.set_src(audio_src);

                        // Create a promise that resolves when metadata is loaded
                        let promise = js_sys::Promise::new(&mut |resolve, reject| {
                            let resolve_clone = resolve.clone();
                            let reject_clone = reject.clone();
                            let src_audio = audio.clone();
                            // Set up loadedmetadata event listener
                            let onloadedmetadata =
                                Closure::wrap(Box::new(move |_event: web_sys::Event| {
                                    let duration = src_audio.duration();
                                    if !duration.is_nan() && duration > 0.0 {
                                        resolve_clone
                                            .call1(
                                                &JsValue::UNDEFINED,
                                                &JsValue::from_f64(duration),
                                            )
                                            .unwrap();
                                    } else {
                                        reject_clone
                                            .call1(
                                                &JsValue::UNDEFINED,
                                                &JsValue::from_str("Invalid duration"),
                                            )
                                            .unwrap();
                                    }
                                })
                                    as Box<dyn FnMut(_)>);

                            // Set up error handler
                            let onerror = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                                reject
                                    .call1(
                                        &JsValue::UNDEFINED,
                                        &JsValue::from_str("Failed to load metadata"),
                                    )
                                    .unwrap();
                            })
                                as Box<dyn FnMut(_)>);

                            audio.set_onloadedmetadata(Some(
                                onloadedmetadata.as_ref().unchecked_ref(),
                            ));
                            audio.set_onerror(Some(onerror.as_ref().unchecked_ref()));

                            // Preload metadata only
                            audio.set_preload("metadata");
                            audio.load();

                            // Prevent closures from being dropped
                            onloadedmetadata.forget();
                            onerror.forget();
                        });

                        // Convert promise to future and await it
                        match JsFuture::from(promise).await {
                            Ok(value) => value.as_f64(),
                            Err(_) => None,
                        }
                    }

                    // Get the actual duration
                    let actual_duration_sec = get_actual_duration(&src_for_analysis).await;

                    // Use the actual duration if available, otherwise fall back to provided duration
                    let final_duration_sec =
                        actual_duration_sec.unwrap_or(episode_duration_for_wasm as f64);

                    web_sys::console::log_1(&JsValue::from_str(&format!(
                        "Offline - Original duration: {}s, Actual duration: {}s",
                        episode_duration_for_wasm, final_duration_sec
                    )));

                    audio_dispatch_for_duration.reduce_mut(move |audio_state| {
                        audio_state.audio_playing = Some(true);
                        audio_state.playback_speed = 1.0;
                        audio_state.audio_volume = 100.0;
                        audio_state.offline = Some(true);
                        audio_state.currently_playing = Some(AudioPlayerProps {
                            src: src.clone(),
                            title: episode_title_for_wasm.clone(),
                            description: episode_description_for_wasm.clone(),
                            release_date: episode_release_date_for_wasm.clone(),
                            artwork_url: episode_artwork_for_wasm.clone(),
                            duration: format!("{}", final_duration_sec as i32), // Use actual duration
                            episode_id: episode_id_for_wasm.clone(),
                            duration_sec: final_duration_sec, // Use actual duration
                            start_pos_sec: listen_duration_for_closure.unwrap_or(0) as f64,
                            end_pos_sec: 0.0,
                            offline: true,
                            is_youtube: episode_is_youtube_for_wasm,
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

pub fn on_play_click_shared(
    episode_url: String,
    episode_title: String,
    episode_description: String,
    episode_release_date: String,
    episode_artwork: String,
    episode_duration: i32,
    episode_id: i32,
    audio_dispatch: Dispatch<UIState>,
    is_youtube_vid: bool,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        let episode_url = episode_url.clone();
        let episode_title = episode_title.clone();
        let episode_description = episode_description.clone();
        let episode_release_date = episode_release_date.clone();
        let episode_artwork = episode_artwork.clone();
        let episode_duration = episode_duration.clone();
        let episode_is_youtube = is_youtube_vid.clone();
        let episode_id = episode_id.clone();
        let audio_dispatch = audio_dispatch.clone();

        // NEW: Analyze duration before playing
        let audio_dispatch_for_duration = audio_dispatch.clone();
        let episode_url_for_analysis = episode_url.clone();

        wasm_bindgen_futures::spawn_local(async move {
            // Function to get actual duration from audio file
            async fn get_actual_duration(audio_src: &str) -> Option<f64> {
                use wasm_bindgen::JsCast;
                use wasm_bindgen_futures::JsFuture;

                // Create a temporary audio element
                let window = web_sys::window()?;
                let document = window.document()?;
                let audio_element = document.create_element("audio").ok()?;
                let audio: HtmlAudioElement = audio_element.dyn_into().ok()?;

                // Set the source
                audio.set_src(audio_src);

                // Create a promise that resolves when metadata is loaded
                let promise = js_sys::Promise::new(&mut |resolve, reject| {
                    let resolve_clone = resolve.clone();
                    let reject_clone = reject.clone();
                    let src_audio = audio.clone();
                    // Set up loadedmetadata event listener
                    let onloadedmetadata = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        let duration = src_audio.duration();
                        if !duration.is_nan() && duration > 0.0 {
                            resolve_clone
                                .call1(&JsValue::UNDEFINED, &JsValue::from_f64(duration))
                                .unwrap();
                        } else {
                            reject_clone
                                .call1(&JsValue::UNDEFINED, &JsValue::from_str("Invalid duration"))
                                .unwrap();
                        }
                    })
                        as Box<dyn FnMut(_)>);

                    // Set up error handler
                    let onerror = Closure::wrap(Box::new(move |_event: web_sys::Event| {
                        reject
                            .call1(
                                &JsValue::UNDEFINED,
                                &JsValue::from_str("Failed to load metadata"),
                            )
                            .unwrap();
                    }) as Box<dyn FnMut(_)>);

                    audio.set_onloadedmetadata(Some(onloadedmetadata.as_ref().unchecked_ref()));
                    audio.set_onerror(Some(onerror.as_ref().unchecked_ref()));

                    // Preload metadata only
                    audio.set_preload("metadata");
                    audio.load();

                    // Prevent closures from being dropped
                    onloadedmetadata.forget();
                    onerror.forget();
                });

                // Convert promise to future and await it
                match JsFuture::from(promise).await {
                    Ok(value) => value.as_f64(),
                    Err(_) => None,
                }
            }

            // Get the actual duration
            let actual_duration_sec = get_actual_duration(&episode_url_for_analysis).await;

            // Use the actual duration if available, otherwise fall back to provided duration
            let final_duration_sec = actual_duration_sec.unwrap_or(episode_duration as f64);

            web_sys::console::log_1(&JsValue::from_str(&format!(
                "Shared - Original duration: {}s, Actual duration: {}s",
                episode_duration, final_duration_sec
            )));

            audio_dispatch_for_duration.reduce_mut(move |audio_state| {
                audio_state.audio_playing = Some(true);
                audio_state.playback_speed = 1.0;
                audio_state.audio_volume = 100.0;
                audio_state.offline = Some(false);
                audio_state.currently_playing = Some(AudioPlayerProps {
                    src: episode_url.clone(),
                    title: episode_title.clone(),
                    description: episode_description.clone(),
                    release_date: episode_release_date.clone(),
                    artwork_url: episode_artwork.clone(),
                    duration: format!("{}", final_duration_sec as i32), // Use actual duration
                    episode_id,
                    duration_sec: final_duration_sec, // Use actual duration
                    start_pos_sec: 0.0,               // Start playing from the beginning
                    end_pos_sec: 0.0,
                    offline: true,
                    is_youtube: episode_is_youtube,
                });
                audio_state.set_audio_source(episode_url.clone());
                if let Some(audio) = &audio_state.audio_element {
                    let _ = audio.play();
                }
                audio_state.audio_playing = Some(true);
            });
        });
    })
}
