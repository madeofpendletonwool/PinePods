use crate::components::context::{AppState, UIState};
#[cfg(not(feature = "server_build"))]
use crate::components::downloads_tauri::start_local_file_server;
use crate::components::gen_funcs::format_time_rm_hour;
#[cfg(not(feature = "server_build"))]
use crate::requests::pod_req::EpisodeDownload;
use crate::requests::pod_req::FetchPodcasting2DataRequest;
use crate::requests::pod_req::{
    call_add_history, call_check_episode_in_db, call_fetch_podcasting_2_data,
    call_get_auto_skip_times, call_get_episode_id, call_get_podcast_id_from_ep,
    call_get_queued_episodes, call_increment_listen_time, call_increment_played,
    call_mark_episode_completed, call_queue_episode, call_record_listen_duration,
    call_remove_queued_episode, HistoryAddRequest, MarkEpisodeCompletedRequest,
    QueuePodcastRequest, RecordListenDurationRequest,
};
use gloo_timers::callback::Interval;
use js_sys::Array;
use std::cell::Cell;
#[cfg(not(feature = "server_build"))]
use std::path::Path;
use std::rc::Rc;
use std::string::String;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, HtmlAudioElement, HtmlElement, HtmlInputElement, Navigator};
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

    html! {
        <div class="speed-control-container">
            <button
                onclick={toggle_open}
                class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center"
            >
                <span class="material-icons">{"speed"}</span>
            </button>
            <div class={classes!("speed-slider-container", "item_container-bg", (*is_open).then(|| "visible"))}>
                <div class="speed-control-content item_container-bg">
                    <div class="speed-text">
                        {format!("{}x", props.speed)}
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

    let volume_icon = match props.volume as i32 {
        0 => "volume_off",
        1..=33 => "volume_mute",
        34..=66 => "volume_down",
        _ => "volume_up",
    };

    html! {
        <div class="volume-control-container">
            <button
                onclick={toggle_open}
                class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center"
            >
                <span class="material-icons">{volume_icon}</span>
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
    let history = BrowserHistory::new();
    let history_clone = history.clone();
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

    // Get episode chapters if available
    use_effect_with(
        (
            episode_id.clone(),
            user_id.clone(),
            api_key.clone(),
            server_name.clone(),
        ),
        {
            let dispatch = _audio_dispatch.clone();
            move |(episode_id, user_id, api_key, server_name)| {
                if let (Some(episode_id), Some(user_id), Some(api_key), Some(server_name)) =
                    (episode_id, user_id, api_key, server_name)
                {
                    let episode_id = *episode_id; // Dereference the option
                    let user_id = *user_id; // Dereference the option
                    let api_key = api_key.clone(); // Clone to make it owned
                    let server_name = server_name.clone(); // Clone to make it owned

                    // Only proceed if the episode_id is not zero
                    if episode_id != 0 {
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
            web_sys::console::log_1(&"Setting up interval".into());
            //print the ep id
            web_sys::console::log_1(&format!("Episode ID: {:?}", closure_episode_id).into());
            let interval_handle: Rc<Cell<Option<Interval>>> = Rc::new(Cell::new(None));
            let interval_handle_clone = interval_handle.clone();
            let interval = Interval::new(1000, move || {
                if let Some(audio_element) = state_clone.audio_element.as_ref() {
                    let time_in_seconds = audio_element.current_time();
                    let duration = audio_element.duration(); // Assuming you can get the duration from the audio_element
                    let end_pos_sec = end_pos.clone(); // Get the end position
                    web_sys::console::log_1(&format!("Time: {}", time_in_seconds).into());
                    web_sys::console::log_1(&format!("Duration: {}", duration).into());
                    web_sys::console::log_1(&format!("End Pos: {:?}", end_pos_sec).into());
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
                                        user_id: *complete_user_id,       // Dereference the option
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
                    } else {
                        web_sys::console::log_1(&format!("Time: {}", time_in_seconds).into());
                        let hours = (time_in_seconds / 3600.0).floor() as i32;
                        let minutes = ((time_in_seconds % 3600.0) / 60.0).floor() as i32;
                        let seconds = (time_in_seconds % 60.0).floor() as i32;
                        let formatted_time = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
                        web_sys::console::log_1(
                            &format!("Formatted Time: {}", formatted_time).into(),
                        );
                        // Calculate progress as a percentage
                        let progress_percentage = if duration > 0.0 {
                            time_in_seconds / duration * 100.0
                        } else {
                            0.0
                        };
                        web_sys::console::log_1(
                            &format!("Progress: {}", progress_percentage).into(),
                        );

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
                } else {
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

    {
        let audio_state = audio_state.clone();
        let audio_dispatch = _audio_dispatch.clone();

        use_effect_with(audio_state.clone(), move |_| {
            if let Some(window) = web_sys::window() {
                let navigator: Navigator = window.navigator();

                // Set up media session
                if let Ok(media_session) =
                    js_sys::Reflect::get(&navigator, &JsValue::from_str("mediaSession"))
                {
                    let media_session: web_sys::MediaSession = media_session.dyn_into().unwrap();

                    // Update metadata
                    if let Some(audio_props) = &audio_state.currently_playing {
                        let metadata = web_sys::MediaMetadata::new().unwrap();
                        metadata.set_title(&audio_props.title);

                        // Create a JavaScript array for the artwork
                        let artwork_array = Array::new();
                        let artwork_object = js_sys::Object::new();
                        js_sys::Reflect::set(
                            &artwork_object,
                            &"src".into(),
                            &audio_props.artwork_url.clone().into(),
                        )
                        .unwrap();
                        js_sys::Reflect::set(&artwork_object, &"sizes".into(), &"512x512".into())
                            .unwrap();
                        js_sys::Reflect::set(&artwork_object, &"type".into(), &"image/jpeg".into())
                            .unwrap();
                        artwork_array.push(&artwork_object);

                        // Set the artwork using the JavaScript array
                        metadata.set_artwork(&artwork_array.into());

                        media_session.set_metadata(Some(&metadata));
                    }
                    let audio_dispatch_play = audio_dispatch.clone();
                    // Set up action handlers
                    let play_pause_callback = Closure::wrap(Box::new(move || {
                        audio_dispatch_play.reduce_mut(UIState::toggle_playback);
                    })
                        as Box<dyn FnMut()>);
                    media_session.set_action_handler(
                        web_sys::MediaSessionAction::Play,
                        Some(play_pause_callback.as_ref().unchecked_ref()),
                    );
                    media_session.set_action_handler(
                        web_sys::MediaSessionAction::Pause,
                        Some(play_pause_callback.as_ref().unchecked_ref()),
                    );
                    play_pause_callback.forget();
                    let audio_state_back = audio_state.clone();
                    let audio_dispatch_back = audio_dispatch.clone();
                    let seek_backward_callback = Closure::wrap(Box::new(move || {
                        if let Some(audio_element) = audio_state_back.audio_element.as_ref() {
                            let new_time = audio_element.current_time() - 15.0;
                            audio_element.set_current_time(new_time);
                            audio_dispatch_back
                                .reduce_mut(|state| state.update_current_time(new_time));
                        }
                    })
                        as Box<dyn FnMut()>);
                    media_session.set_action_handler(
                        web_sys::MediaSessionAction::Seekbackward,
                        Some(seek_backward_callback.as_ref().unchecked_ref()),
                    );
                    seek_backward_callback.forget();

                    let seek_forward_callback = Closure::wrap(Box::new(move || {
                        if let Some(audio_element) = audio_state.audio_element.as_ref() {
                            let new_time = audio_element.current_time() + 15.0;
                            audio_element.set_current_time(new_time);
                            audio_dispatch.reduce_mut(|state| state.update_current_time(new_time));
                        }
                    })
                        as Box<dyn FnMut()>);
                    media_session.set_action_handler(
                        web_sys::MediaSessionAction::Seekforward,
                        Some(seek_forward_callback.as_ref().unchecked_ref()),
                    );
                    seek_forward_callback.forget();
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

    let chapter_select_modal = html! {
        <div id="chapter-select-modal" tabindex="-1" aria-hidden="true"
            class="chapter-select-modal fixed top-0 right-0 left-0 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow">
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
                            chapters.iter().map(|chapter| {
                                let start_time_click = chapter.startTime.clone().unwrap_or_default();
                                let start_time = format_time_rm_hour(chapter.startTime.clone().unwrap_or_default() as f64);
                                let click_start_time = start_time_click.clone();
                                let on_chapter_click = on_chapter_click.clone();

                                html! {
                                    <div class="chapter-item"
                                        onclick={Callback::from(move |_| on_chapter_click.emit(click_start_time.clone()))}>
                                        <button class="chapter-play-button">
                                            <span class="material-icons text-xl">{"play_arrow"}</span>
                                        </button>
                                        <div class="chapter-info">
                                            <span class="chapter-title">{ &chapter.title }</span>
                                            <span class="chapter-time">{ start_time }</span>
                                        </div>
                                    </div>
                                }
                            }).collect::<Html>()
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
            let history = history_clone.clone();
            let dispatch = _dispatch.clone();
            let episode_id = audio_state
                .currently_playing
                .as_ref()
                .map(|audio_props| audio_props.episode_id);

            Callback::from(move |_: MouseEvent| {
                let dispatch_clone = dispatch.clone();
                let history_clone = history.clone();
                if let Some(episode_id) = episode_id {
                    wasm_bindgen_futures::spawn_local(async move {
                        dispatch_clone.reduce_mut(move |state| {
                            state.selected_episode_id = Some(episode_id);
                            state.fetched_episode = None;
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
                "background: linear-gradient(to right, #1db954 0%, #1db954 {}%, var(--prog-bar-color) {}%, var(--prog-bar-color) 100%);",
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
                        <span class="material-icons">{"expand_more"}</span>
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
                            <span class="material-icons">{"fast_rewind"}</span>
                        </button>
                        <button onclick={toggle_playback.clone()} class="pronounce-mobile audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">
                                { if audio_state.audio_playing.unwrap_or(false) { "pause" } else { "play_arrow" } }
                            </span>
                        </button>
                        <button onclick={skip_forward.clone()} class="pronounce-mobile skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">{"fast_forward"}</span>
                        </button>
                        <button onclick={skip_episode.clone()} class="skip-button audio-top-button selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center">
                            <span class="material-icons">{"skip_next"}</span>
                        </button>
                    </div>

                    <div class="episode-button-container flex items-center justify-center">
                    {
                        if episode_in_db {
                            html! {
                                <>
                                <button onclick={Callback::from(move |e: MouseEvent| {
                                    on_shownotes_click.emit(e.clone());
                                    title_click_emit.emit(e);
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
                <div class="left-group">
                    <div onclick={title_click.clone()} class="artwork-container">
                        <img class={artwork_class} src={audio_props.artwork_url.clone()} />
                    </div>
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
            </>
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
        web_sys::console::log_1(&JsValue::from_str("Checking if episode is in db"));
        web_sys::console::log_1(&JsValue::from_str(&episode_title_for_wasm));
        web_sys::console::log_1(&JsValue::from_str(&episode_url_for_wasm));
        web_sys::console::log_1(&JsValue::from_str(&episode_artwork_for_wasm));
        web_sys::console::log_1(&JsValue::from_str(&episode_duration_for_wasm.to_string()));
        web_sys::console::log_1(&JsValue::from_str(&episode_id_for_wasm.to_string()));
        // web_sys::console::log_1(&JsValue::from_str(&listen_duration_for_closure.to_string()));
        web_sys::console::log_1(&JsValue::from_str(&api_key));
        web_sys::console::log_1(&JsValue::from_str(&user_id.to_string()));
        web_sys::console::log_1(&JsValue::from_str(&server_name));

        web_sys::console::log_1(&JsValue::from_str(&episode_id_for_wasm.to_string()));
        spawn_local(async move {
            // First, check if the episode exists in the database
            let mut episode_exists = call_check_episode_in_db(
                &check_server_name.clone(),
                &check_api_key.clone(),
                check_user_id.clone(),
                &episode_title.clone(),
                &episode_url.clone(),
            )
            .await
            .unwrap_or(false); // Default to false if the call fails

            let mut episode_id = episode_id_for_wasm;

            // If the episode exists but the current `episode_id` is `0`, retrieve the correct `episode_id`
            if episode_exists && episode_id == 0 {
                match call_get_episode_id(
                    &check_server_name,
                    &check_api_key,
                    &check_user_id,
                    &episode_title,
                    &episode_url,
                )
                .await
                {
                    Ok(new_episode_id) => {
                        if new_episode_id == 0 {
                            // Handle the case where the episode ID is still 0 (None scenario)
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
                        // If the call failed, assume the episode doesn't exist
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
            // If the episode exists, update the global state with the episode ID
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
                };

                let add_history_future =
                    call_add_history(&history_server_name, history_api_key, &history_add);
                match add_history_future.await {
                    Ok(_) => {
                        web_sys::console::log_1(&JsValue::from_str("History successfully added"));
                    }
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
                    web_sys::console::log_1(&"Successfully incremented playcount".into());
                }
                Err(_e) => {
                    web_sys::console::log_1(&format!("Failed to increment: {:?}", _e).into());
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
        web_sys::console::log_1(&JsValue::from_str("about to not run pod id if 0"));

        wasm_bindgen_futures::spawn_local(async move {
            if episode_id != 0 {
                web_sys::console::log_1(&JsValue::from_str("must not be zero"));
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
            } else {
                // Directly play the episode without skip times
                web_sys::console::log_1(&JsValue::from_str("must be zero"));
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
                        start_pos_sec: 0.0,
                        end_pos_sec: 0.0,
                        offline: false,
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
pub fn on_play_click_offline(
    episode_info: EpisodeDownload,
    audio_dispatch: Dispatch<UIState>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        let episode_info_for_closure = episode_info.clone();
        let audio_dispatch = audio_dispatch.clone();

        // Early return if downloadedlocation is None
        let file_path = match episode_info_for_closure.downloadedlocation {
            Some(path) => path,
            None => {
                // Maybe dispatch an error message here if needed
                audio_dispatch.reduce_mut(|state| {
                    state.error_message = Some("Episode file location not found".to_string());
                });
                return;
            }
        };

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
                        .unwrap_or("");
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

pub fn on_play_click_shared(
    episode_url: String,
    episode_title: String,
    episode_artwork: String,
    episode_duration: i32,
    episode_id: i32,
    audio_dispatch: Dispatch<UIState>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        let episode_url = episode_url.clone();
        let episode_title = episode_title.clone();
        let episode_artwork = episode_artwork.clone();
        let episode_duration = episode_duration.clone();
        let episode_id = episode_id.clone();
        let audio_dispatch = audio_dispatch.clone();

        web_sys::console::log_1(&JsValue::from_str("Playing shared episode..."));
        web_sys::console::log_1(&JsValue::from_str(&episode_title));
        web_sys::console::log_1(&JsValue::from_str(&episode_url));
        web_sys::console::log_1(&JsValue::from_str(&episode_artwork));
        web_sys::console::log_1(&JsValue::from_str(&episode_duration.to_string()));
        web_sys::console::log_1(&JsValue::from_str(&episode_id.to_string()));

        // No user-specific checks or DB operations needed, just play the episode
        wasm_bindgen_futures::spawn_local(async move {
            audio_dispatch.reduce_mut(move |audio_state| {
                audio_state.audio_playing = Some(true);
                audio_state.playback_speed = 1.0;
                audio_state.audio_volume = 100.0;
                audio_state.offline = Some(false);
                audio_state.currently_playing = Some(AudioPlayerProps {
                    src: episode_url.clone(),
                    title: episode_title.clone(),
                    artwork_url: episode_artwork.clone(),
                    duration: episode_duration.to_string(),
                    episode_id: episode_id,
                    duration_sec: episode_duration as f64,
                    start_pos_sec: 0.0, // Start playing from the beginning
                    end_pos_sec: 0.0,
                    offline: true,
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
