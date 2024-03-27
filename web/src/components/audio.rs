use gloo_timers::callback::Interval;
use yew::{Callback, function_component, Html, html};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use web_sys::{console, window, HtmlAudioElement, HtmlInputElement};
use std::string::String;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;
use std::rc::Rc;
use crate::requests::pod_req::{call_add_history, HistoryAddRequest, call_record_listen_duration, RecordListenDurationRequest, call_increment_listen_time, call_increment_played, call_get_queued_episodes, call_remove_queued_episode, QueuePodcastRequest, call_queue_episode, call_check_episode_in_db};
use futures_util::stream::StreamExt;


#[derive(Properties, PartialEq, Debug, Clone)]
pub struct AudioPlayerProps {
    pub src: String,
    pub title: String,
    pub artwork_url: String,
    pub duration: String,
    pub episode_id: i32,
    pub duration_sec: f64,
    pub start_pos_sec: f64,
}

#[function_component(AudioPlayer)]
pub fn audio_player(props: &AudioPlayerProps) -> Html {
    let audio_ref = use_node_ref();
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let episode_id = audio_state.currently_playing.as_ref().map(|props| props.episode_id);
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let episode_in_db = audio_state.episode_in_db.unwrap_or_default();
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
    // Clone `audio_ref` for `use_effect_with`
    let state_clone = audio_state.clone();
    use_effect_with((), {
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
                        // Update the global state with the current time
                        state_clone.current_time_seconds = time_in_seconds;
                        state_clone.current_time_formatted = formatted_time;
                    });
                }
            });
    
            // Return a cleanup function that will be run when the component unmounts or the dependencies of the effect change
            move || drop(interval_handle)
        }
    });

    // Effect for recording the listen duration

    let state_clone_the_squeakuel = audio_state.clone();
    use_effect_with((), {
        let server_name = server_name.clone(); // Assuming this is defined elsewhere in your component
        let api_key = api_key.clone(); // Assuming this is defined elsewhere in your component
        let episode_id = episode_id.clone(); // Assuming this is defined elsewhere in your component
        let user_id = user_id.clone(); // Assuming this is defined elsewhere in your component
        // let episode_in_db_effect = audio_state.episode_in_db.unwrap_or_default();
    
        move |_| {
        // Spawn a new async task
            let future = async move {
                let mut interval = gloo_timers::future::IntervalStream::new(30_000);
                loop {
                    interval.next().await; // Wait for the next interval tick
                    // Check if audio is playing before proceeding
                    if state_clone_the_squeakuel.audio_playing.unwrap_or_default() {
                        if let Some(audio_element) = state_clone_the_squeakuel.audio_element.as_ref() {
                            // Use the local_current_seconds here
                            // let listen_duration = (*local_current_seconds); // Dereference and get the value
                            let listen_duration = audio_element.current_time();

                            let request_data = RecordListenDurationRequest {
                                episode_id: episode_id.unwrap().clone(),
                                user_id: user_id.unwrap().clone(),
                                listen_duration,
                            };
    
                            // Perform the API call to record the listen duration
                            match call_record_listen_duration(&server_name.clone().unwrap(), &api_key.clone().unwrap().unwrap(), request_data).await {
                                Ok(response) => {
                                    web_sys::console::log_1(&format!("Listen duration recorded: {:?}", response).into());
                                },
                                Err(e) => {
                                    web_sys::console::log_1(&format!("Failed to record listen duration: {:?}", e).into());
                                }
                            }
                        }
                    }
                }
            };
    
            // Using wasm_bindgen_futures to spawn the local future
            wasm_bindgen_futures::spawn_local(future);
    
            // Cleanup function (currently no cleanup required for this future)
            || ()
        }
    });
    
    
    // Effect for incrementing user listen time
    // Effect for incrementing user listen time
    let state_increment_clone = audio_state.clone();
    use_effect_with((), {
        let server_name = server_name.clone(); // Make sure `server_name` is cloned from the parent scope
        let api_key = api_key.clone(); // Make sure `api_key` is cloned from the parent scope
        let user_id = user_id.clone(); // Make sure `user_id` is cloned from the parent scope

        move |_| {
            let interval_handle = Interval::new(60000, move || {
                // Check if audio is playing before making the API call
                if state_increment_clone.audio_playing.unwrap_or_default() {
                    let server_name = server_name.clone();
                    let api_key = api_key.clone();
                    let user_id = user_id.clone();
                    
                    // Spawn a new async task for the API call
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_increment_listen_time(&server_name.unwrap(), &api_key.unwrap().unwrap(), user_id.unwrap()).await {
                            Ok(response) => {
                                web_sys::console::log_1(&format!("Listen time incremented: {:?}", response).into());
                            },
                            Err(e) => {
                                web_sys::console::log_1(&format!("Failed to increment listen time: {:?}", e).into());
                            }
                        }
                    });
                } else {
                    // Optionally log that the audio is not playing and thus listen time was not incremented
                    web_sys::console::log_1(&"Audio is not playing, listen time not incremented".into());
                }
            });

            // Return a cleanup function that will be run when the component unmounts or the dependencies of the effect change
            move || drop(interval_handle)
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
                    // Closure::wrap(Box::new(move |_| {
                    wasm_bindgen_futures::spawn_local(async move {
                        let queued_episodes_result = call_get_queued_episodes(&server_name.clone().unwrap(), &api_key.clone().unwrap(), &user_id.clone().unwrap()).await;
                        match queued_episodes_result {
                            Ok(episodes) => {
                                if let Some(current_episode) = episodes.iter().find(|ep| ep.EpisodeID == current_episode_id.unwrap()) {
                                    let current_queue_position = current_episode.QueuePosition.unwrap_or_default();
                                    // Remove the currently playing episode from the queue
                                    let request = QueuePodcastRequest {
                                        episode_id: current_episode_id.clone().unwrap(),
                                        user_id: user_id.clone().unwrap(), // replace with the actual user ID
                                    };
                                    let remove_result = call_remove_queued_episode(&server_name.clone().unwrap(), &api_key.clone().unwrap(), &request).await;
                                    match remove_result {
                                        Ok(_) => {
                                            web_sys::console::log_1(&"Successfully removed episode from queue".into());
                                        },
                                        Err(e) => {
                                            web_sys::console::log_1(&format!("Failed to remove episode from queue: {:?}", e).into());
                                        }
                                    }
                                    if let Some(next_episode) = episodes.iter().find(|ep| ep.QueuePosition == Some(current_queue_position + 1)) {
                                        on_play_click(
                                            next_episode.EpisodeURL.clone(),
                                            next_episode.EpisodeTitle.clone(),
                                            next_episode.EpisodeArtwork.clone(),
                                            next_episode.EpisodeDuration,
                                            next_episode.EpisodeID,
                                            next_episode.ListenDuration,
                                            api_key.clone().unwrap().unwrap(),
                                            user_id.unwrap(),
                                            server_name.clone().unwrap(),
                                            audio_dispatch.clone(),
                                            audio_state.clone(),
                                            None,
                                        ).emit(MouseEvent::new("click").unwrap());
                                    } else {
                                        audio_dispatch.reduce_mut(|state| {
                                            state.audio_playing = Some(false);
                                        });
                                    }
                                }
                            },
                            Err(e) => {
                                web_sys::console::log_1(&format!("Failed to fetch queued episodes: {:?}", e).into());
                            }
                        }
                    });
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
                            state.selected_episode_id = Some(episode_id);
                        });
                        history_clone.push("/episode"); // Use the route path
                    });
                }
            })
        };
        let audio_bar_class = classes!("audio-player", "border", "border-solid", "border-color", "fixed", "bottom-0", "z-50", "w-full", if audio_state.is_expanded { "expanded" } else { "" });
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
                    {
                        if episode_in_db {
                            html! {
                                <button onclick={Callback::from(move |e: MouseEvent| {
                                    on_shownotes_click.emit(e.clone());
                                    title_click_emit.emit(e);
                                })} class="item-container-button audio-full-button border-solid border selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center">
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
    listen_duration_for_closure: Option<i32>,
    api_key: String,
    user_id: i32,
    server_name: String,
    audio_dispatch: Dispatch<UIState>,
    _audio_state: Rc<UIState>,
    is_local: Option<bool>,
) -> Callback<MouseEvent> {

    Callback::from(move |_: MouseEvent| {
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
    
        let episode_url_for_closure = episode_url_for_closure.clone();
        let episode_title_for_closure = episode_title_for_closure.clone();
        let episode_artwork_for_closure = episode_artwork_for_closure.clone();
        let episode_duration_for_closure = episode_duration_for_closure.clone();
        let listen_duration_for_closure = listen_duration_for_closure.clone();
        let episode_id_for_closure = episode_id_for_closure.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        web_sys::console::log_1(&format!("duration: {}", &episode_duration_for_closure).into());
        let audio_dispatch = audio_dispatch.clone();
    
        let formatted_duration = parse_duration_to_seconds(&episode_duration_for_closure);
        let episode_pos: f32 = 0.0;
        let episode_id = episode_id_for_closure.clone();
        web_sys::console::log_1(&"Adding hisotry".to_string().into());
        
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
                &episode_url.clone()
            ).await.unwrap_or(false); // Default to false if the call fails
            console::log_1(&format!("Episode exists: {:?}", episode_exists).into());
            app_dispatch.reduce_mut(move |global_state| {
                global_state.episode_in_db = Some(episode_exists);
            });
            console::log_1(&format!("Episode exists - Now running hsitory add: {:?}", episode_exists).into());
            if episode_exists {
                console::log_1(&"Episode exists - History ran".to_string().into());
                let history_server_name = check_server_name.clone();
                let history_api_key = check_api_key.clone();

                let history_add = HistoryAddRequest{
                    episode_id,
                    episode_pos,
                    user_id,
                };

                let add_history_future = call_add_history(
                    &history_server_name,
                    history_api_key, 
                    &history_add
                );
                match add_history_future.await {
                    Ok(_) => {
                        web_sys::console::log_1(&"Successfully added history".into());
                    },
                    Err(e) => {
                        web_sys::console::log_1(&format!("Failed to add history: {:?}", e).into());
                    }
                }

                let queue_server_name = check_server_name.clone();
                let queue_api_key = check_api_key.clone();
        
                let request = QueuePodcastRequest {
                    episode_id,
                    user_id, // replace with the actual user ID
                };


                let queue_api = Option::from(queue_api_key);

                let add_queue_future = call_queue_episode(
                    &queue_server_name,
                    &queue_api, 
                    &request
                );
                match add_queue_future.await {
                    Ok(_) => {
                        web_sys::console::log_1(&"Successfully Added Episode to Queue".into());
                    },
                    Err(e) => {
                        web_sys::console::log_1(&format!("Failed to add to queue: {:?}", e).into());
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
                increment_user_id
            );
            match add_history_future.await {
                Ok(_) => {
                    web_sys::console::log_1(&"Successfully incremented playcount".into());
                },
                Err(e) => {
                    web_sys::console::log_1(&format!("Failed to increment: {:?}", e).into());
                }
            }
        });
        let src = String::new();
        let src = if let Some(local) = is_local {
            // Construct the URL for streaming from the local server
            let src = format!("{}/api/data/stream/{}?api_key={}&user_id={}", server_name, episode_id, api_key, user_id);
            console::log_1(&format!("Local URL: {:?}", src.clone()).into());
            src
        } else {
            // Use the provided URL for streaming
            let src = episode_url_for_wasm.clone();
            src
        };

        audio_dispatch.reduce_mut(move |audio_state| {
            audio_state.audio_playing = Some(true);
            audio_state.currently_playing = Some(AudioPlayerProps {
                src: src.clone(),
                title: episode_title_for_wasm.clone(),
                artwork_url: episode_artwork_for_wasm.clone(),
                duration: episode_duration_for_wasm.clone().to_string(),
                episode_id: episode_id_for_wasm.clone(),
                duration_sec: formatted_duration,
                start_pos_sec: listen_duration_for_closure.unwrap_or(0) as f64, 
            });
            audio_state.set_audio_source(src.to_string());
            if let Some(audio) = &audio_state.audio_element {
                audio.set_current_time(listen_duration_for_closure.unwrap_or(0) as f64);
                let _ = audio.play();
            }
            audio_state.audio_playing = Some(true);
        });
    })
}