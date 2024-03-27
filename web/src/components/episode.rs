use yew::{function_component, Html, html};
use yew::prelude::*;
use super::app_drawer::App_drawer;
use super::gen_components::{Search_nav, empty_message};
use crate::requests::pod_req;
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use crate::components::audio::AudioPlayer;
use crate::components::gen_funcs::{sanitize_html_with_blank_target, format_datetime, format_time, match_date_format, parse_date};
use crate::requests::pod_req::{EpisodeRequest, EpisodeMetadataResponse, QueuePodcastRequest, call_queue_episode, SavePodcastRequest, call_save_episode, DownloadEpisodeRequest, call_download_episode};
use crate::components::audio::on_play_click;
use crate::components::episodes_layout::SafeHtml;
use crate::components::episodes_layout::UIStateMsg;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::window;
use crate::requests::login_requests::use_check_authentication;

#[function_component(Episode)]
pub fn epsiode() -> Html {
    let (state, dispatch) = use_store::<AppState>();

    let session_dispatch = dispatch.clone();
    let session_state = state.clone();

    use_effect_with((), move |_| {
        // Check if the page reload action has already occurred to prevent redundant execution
        if session_state.reload_occured.unwrap_or(false) {
            // Logic for the case where reload has already been processed
        } else {
            // Normal effect logic for handling page reload
            let window = web_sys::window().expect("no global `window` exists");
            let performance = window.performance().expect("should have performance");
            let navigation_type = performance.navigation().type_();
            
            if navigation_type == 1 { // 1 stands for reload
                let session_storage = window.session_storage().unwrap().unwrap();
                session_storage.set_item("isAuthenticated", "false").unwrap();
            }
    
            // Always check authentication status
            let current_route = window.location().href().unwrap_or_default();
            use_check_authentication(session_dispatch.clone(), &current_route);
    
            // Mark that the page reload handling has occurred
            session_dispatch.reduce_mut(|state| {
                state.reload_occured = Some(true);
                state.clone() // Return the modified state
            });
        }
    
        || ()
    });


    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();

    {
        let ui_dispatch = audio_dispatch.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                ui_dispatch.apply(UIStateMsg::ClearErrorMessage);
                ui_dispatch.apply(UIStateMsg::ClearInfoMessage);
            }) as Box<dyn Fn(_)>);

            document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();

            // Return cleanup function
            move || {
                document.remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }

    // Fetch episode on component mount
    {
        // let episodes = episodes.clone();
        let error = error.clone();
        let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        let effect_dispatch = dispatch.clone();

        let episode_id = state.selected_episode_id.clone();

        // fetch_episodes(api_key.flatten(), user_id, server_name, dispatch, error, pod_req::call_get_recent_eps);

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) = (api_key.clone(), user_id.clone(), server_name.clone()) {
                    let dispatch = effect_dispatch.clone();
    
                    let episode_request = EpisodeRequest {
                        episode_id: episode_id.clone().unwrap(),
                        user_id: user_id.clone(),
                    };
        
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_episode_metadata(&server_name, api_key, &episode_request).await {
                            Ok(fetched_episode) => {
                                web_sys::console::log_1(&format!("Fetched episode: {:?}", fetched_episode).into()); // Log fetched episode
                                dispatch.reduce_mut(move |state| {
                                    state.fetched_episode = Some(EpisodeMetadataResponse { episode: fetched_episode });
                                });
                                // web_sys::console::log_1(&format!("State after update: {:?}", state).into()); // Log state after update
                            },
                            Err(e) => {
                                web_sys::console::log_1(&format!("Error fetching episode: {:?}", e).into()); // Log error
                                error_clone.set(Some(e.to_string()));
                            },
                        }
                    });
                }
                || ()
            },
        );
    }

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            {
                if let Some(episode) = state.fetched_episode.clone() {
                    web_sys::console::log_1(&format!("Fetched episode: {:?}", episode).into()); // Log fetched episode
    
                    let episode_url_clone = episode.episode.EpisodeURL.clone();
                    let episode_title_clone = episode.episode.EpisodeTitle.clone();
                    let episode_artwork_clone = episode.episode.EpisodeArtwork.clone();
                    let episode_duration_clone = episode.episode.EpisodeDuration.clone();
                    let episode_listened_clone = Option::from(0);
                    let episode_id_clone = episode.episode.EpisodeID.clone();
    
                    let sanitized_description = sanitize_html_with_blank_target(&episode.episode.EpisodeDescription.clone());
                    let description = sanitized_description;
    
                    let episode_url_for_closure = episode_url_clone.clone();
                    let episode_title_for_closure = episode_title_clone.clone();
                    let episode_artwork_for_closure = episode_artwork_clone.clone();
                    let episode_duration_for_closure = episode_duration_clone.clone();
                    let episode_id_for_closure = episode_id_clone.clone();
                    let listener_duration_for_closure = episode_listened_clone.clone();

                    let user_id_play = user_id.clone();
                    let server_name_play = server_name.clone();
                    let api_key_play = api_key.clone();
                    let audio_dispatch = audio_dispatch.clone();

                    let on_play_click = on_play_click(
                        episode_url_for_closure.clone(),
                        episode_title_for_closure.clone(),
                        episode_artwork_for_closure.clone(),
                        episode_duration_for_closure.clone(),
                        episode_id_for_closure.clone(),
                        listener_duration_for_closure.clone(),
                        api_key_play.unwrap().unwrap(),
                        user_id_play.unwrap(),
                        server_name_play.unwrap(),
                        audio_dispatch.clone(),
                        audio_state.clone(),
                        None,
                    );

                    let user_id_queue = user_id.clone();
                    let server_name_queue = server_name.clone();
                    let api_key_queue = api_key.clone();
                    let audio_dispatch_queue = audio_dispatch.clone();

                    let on_add_to_queue = {
                        Callback::from(move |_: MouseEvent| {
                            let server_name_copy = server_name_queue.clone();
                            let api_key_copy = api_key_queue.clone();
                            let queue_post = audio_dispatch_queue.clone();
                            let request = QueuePodcastRequest {
                                episode_id: episode_id_for_closure,
                                user_id: user_id_queue.unwrap(), // replace with the actual user ID
                            };
                            let server_name = server_name_copy; // replace with the actual server name
                            let api_key = api_key_copy; // replace with the actual API key
                            let future = async move {
                                // let _ = call_queue_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                                // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("Episode added to Queue!")));
                                match call_queue_episode(&server_name.unwrap(), &api_key.flatten(), &request).await {
                                    Ok(success_message) => {
                                        queue_post.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                                    },
                                    Err(e) => {
                                        queue_post.reduce_mut(|state| state.error_message = Option::from(format!("{}", e)));
                                        // Handle error, e.g., display the error message
                                    }
                                }
                            };
                            wasm_bindgen_futures::spawn_local(future);
                            // dropdown_open.set(false);
                        })
                    };

                    let saved_server_name = server_name.clone();
                    let saved_api_key = api_key.clone();
                    let save_post = audio_dispatch.clone();
                    let user_id_save = user_id.clone();

                    let on_save_episode = {
                        Callback::from(move |_: MouseEvent| {
                            let server_name_copy = saved_server_name.clone();
                            let api_key_copy = saved_api_key.clone();
                            let post_state = save_post.clone();
                            let request = SavePodcastRequest {
                                episode_id: episode_id_for_closure, // changed from episode_title
                                user_id: user_id_save.unwrap(), // replace with the actual user ID
                            };
                            let server_name = server_name_copy; // replace with the actual server name
                            let api_key = api_key_copy; // replace with the actual API key
                            let future = async move {
                                // let return_mes = call_save_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode saved successfully")));
                                match call_save_episode(&server_name.unwrap(), &api_key.flatten(), &request).await {
                                    Ok(success_message) => {
                                        post_state.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                                    },
                                    Err(e) => {
                                        post_state.reduce_mut(|state| state.error_message = Option::from(format!("{}", e)));
                                        // Handle error, e.g., display the error message
                                    }
                                }
                            };
                            wasm_bindgen_futures::spawn_local(future);
                            // dropdown_open.set(false);
                        })
                    };

                    let download_server_name = server_name.clone();
                    let download_api_key = api_key.clone();
                    let download_post = audio_dispatch.clone();
                    let user_id_download = user_id.clone();

                    let on_download_episode = {
                        Callback::from(move |_: MouseEvent| {
                            let post_state = download_post.clone();
                            let server_name_copy = download_server_name.clone();
                            let api_key_copy = download_api_key.clone();
                            let request = DownloadEpisodeRequest {
                                episode_id: episode_id_for_closure,
                                user_id: user_id_download.unwrap(), // replace with the actual user ID
                            };
                            let server_name = server_name_copy; // replace with the actual server name
                            let api_key = api_key_copy; // replace with the actual API key
                            let future = async move {
                                // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                                match call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await {
                                    Ok(success_message) => {
                                        post_state.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                                    },
                                    Err(e) => {
                                        post_state.reduce_mut(|state| state.error_message = Option::from(format!("{}", e)));
                                        // Handle error, e.g., display the error message
                                    }
                                }
                            };
                            wasm_bindgen_futures::spawn_local(future);
                            // dropdown_open.set(false);
                        })
                    };

                    let datetime = parse_date(&episode.episode.EpisodePubDate, &state.user_tz);
                    let date_format = match_date_format(state.date_format.as_deref());
                    let format_duration = format_time(episode.episode.EpisodeDuration as f64);
                    let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));
                    
                    // let format_duration = format!("Duration: {} minutes", e / 60); // Assuming duration is in seconds
                    // let format_release = format!("Released on: {}", &episode.episode.EpisodePubDate);
    
                    html! {
                        <div class="episode-layout-container">
                            <div class="episode-top-info">
                                <img src={episode.episode.EpisodeArtwork.clone()} class="episode-artwork" />
                                <div class="episode-details">
                                    <h1 class="podcast-title">{ &episode.episode.PodcastName }</h1>
                                    <h2 class="episode-title">{ &episode.episode.EpisodeTitle }</h2>
                                    <p class="episode-duration">{ format_duration }</p>
                                    <p class="episode-release-date">{ format_release }</p>
                                </div>
                            </div>
                            <div class="episode-action-buttons">
                                <button onclick={on_play_click} class="play-button">
                                    <i class="material-icons">{ "play_arrow" }</i>
                                    {"Play"}
                                </button>
                                <button onclick={on_add_to_queue} class="queue-button">
                                    <i class="material-icons">{ "playlist_add" }</i>
                                    {"Queue"}
                                </button>
                                <button onclick={on_save_episode} class="save-button">
                                    <i class="material-icons">{ "favorite" }</i>
                                    {"Save"}
                                </button>
                                <button onclick={on_download_episode} class="download-button-ep">
                                    <i class="material-icons">{ "download" }</i>
                                    {"Download"}
                                </button>
                            </div>
                            <hr class="episode-divider" />
                            <div class="episode-description">
                            // <p>{ description }</p> 
                            <div class="item_container-text episode-description-container">
                                <SafeHtml html={description} />
                            </div>
                            </div>
                        </div>
                    }
                    // item

                } else {
                    empty_message(
                        "Unable to display episode",
                        "Something seems to have gone wrong. A straightup server disconnect maybe? Did you browse here directly? That's not how this app works. It needs the context to browse around. I honestly don't have anything else for you as this shouldn't happen. This is embarrasing."
                    )
                }
            }
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} /> }
            } else {
                html! {}
            }
        }
        // Conditional rendering for the error banner
        if let Some(error) = error_message {
            <div class="error-snackbar">{ error }</div>
        }
        if let Some(info) = info_message {
            <div class="info-snackbar">{ info }</div>
        }
        </div>
        <App_drawer />
        </>
    }
}