use super::app_drawer::App_drawer;
use super::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::audio::on_play_click;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::{SafeHtml, UIStateMsg};
use crate::components::gen_funcs::{
    convert_time_to_seconds, format_datetime, format_time, match_date_format, parse_date,
    sanitize_html_with_blank_target,
};
use crate::components::host_component::HostDropdown;
use crate::requests::login_requests::use_check_authentication;
use crate::requests::pod_req;
use crate::requests::pod_req::{
    call_create_share_link, call_download_episode, call_fetch_podcasting_2_data,
    call_get_episode_id, call_mark_episode_completed, call_mark_episode_uncompleted,
    call_queue_episode, call_save_episode, DownloadEpisodeRequest, EpisodeInfo,
    EpisodeMetadataResponse, EpisodeRequest, FetchPodcasting2DataRequest,
    MarkEpisodeCompletedRequest, QueuePodcastRequest, SavePodcastRequest,
};
use crate::requests::search_pods::call_parse_podcast_url;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::UrlSearchParams;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

fn get_current_url() -> String {
    let window = window().expect("no global `window` exists");
    let location = window.location();
    location
        .href()
        .unwrap_or_else(|_| "Unable to retrieve URL".to_string())
}

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

            if navigation_type == 1 {
                // 1 stands for reload
                let session_storage = window.session_storage().unwrap().unwrap();
                session_storage
                    .set_item("isAuthenticated", "false")
                    .unwrap();
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
    let shared_url = use_state(|| Option::<String>::None);

    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();
    let history = BrowserHistory::new();
    let episode_id = state.selected_episode_id.clone();
    let ep_in_db = use_state(|| false);
    let loading = use_state(|| true); // Initial loading state set to true

    {
        let audio_dispatch = audio_dispatch.clone();

        // Initial check when the component is mounted
        {
            let window = window().unwrap();
            let width = window.inner_width().unwrap().as_f64().unwrap();
            let new_is_mobile = width < 768.0;
            audio_dispatch.reduce_mut(|state| state.is_mobile = Some(new_is_mobile));
        }

        // Resize event listener
        use_effect_with((), move |_| {
            let window = window().unwrap();
            let closure_window = window.clone();
            let closure = Closure::wrap(Box::new(move || {
                let width = closure_window.inner_width().unwrap().as_f64().unwrap();
                let new_is_mobile = width < 768.0;
                audio_dispatch.reduce_mut(|state| state.is_mobile = Some(new_is_mobile));
            }) as Box<dyn Fn()>);

            window
                .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
                .unwrap();

            closure.forget(); // Ensure the closure is not dropped prematurely

            || ()
        });
    }

    {
        let ui_dispatch = audio_dispatch.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                ui_dispatch.apply(UIStateMsg::ClearErrorMessage);
                ui_dispatch.apply(UIStateMsg::ClearInfoMessage);
            }) as Box<dyn Fn(_)>);

            document
                .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                .unwrap();

            // Return cleanup function
            move || {
                document
                    .remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                    .unwrap();
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }

    // Fetch episode on component mount
    {
        // let episodes = episodes.clone();
        let error = error.clone();
        let api_key = post_state
            .auth_details
            .as_ref()
            .map(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = post_state
            .auth_details
            .as_ref()
            .map(|ud| ud.server_name.clone());
        let effect_dispatch = dispatch.clone();
        let effect_pod_state = state.clone();
        let loading_clone = loading.clone();

        let episode_id = state.selected_episode_id.clone();
        // fetch_episodes(api_key.flatten(), user_id, server_name, dispatch, error, pod_req::call_get_recent_eps);
        let effect_ep_in_db = ep_in_db.clone();
        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    web_sys::console::log_1(&"Fetching episode...".into());
                    let dispatch = effect_dispatch.clone();

                    // Check if the URL contains the parameters for the episode
                    let window = web_sys::window().expect("no global window exists");
                    let search_params = window.location().search().unwrap();
                    let url_params = UrlSearchParams::new_with_str(&search_params).unwrap();

                    let podcast_title = url_params.get("podcast_title").unwrap_or_default();
                    let episode_url = url_params.get("episode_url").unwrap_or_default();
                    let audio_url = url_params.get("audio_url").unwrap_or_default();

                    if !podcast_title.is_empty() && !episode_url.is_empty() && !audio_url.is_empty()
                    {
                        web_sys::console::log_1(&"Episode parameters found".into());
                        // URL contains episode parameters, handle the episode setup
                        let podcast_title_clone = podcast_title.clone();
                        let episode_url_clone = episode_url.clone();
                        let audio_url_clone = audio_url.clone();

                        wasm_bindgen_futures::spawn_local(async move {
                            match call_parse_podcast_url(
                                server_name.clone(),
                                &api_key,
                                &episode_url_clone,
                            )
                            .await
                            {
                                Ok(result) => {
                                    if let Some(ep) = result
                                        .episodes
                                        .iter()
                                        .find(|ep| {
                                            ep.enclosure_url.as_ref() == Some(&audio_url_clone)
                                        })
                                        .cloned()
                                    {
                                        let time_sec = convert_time_to_seconds(
                                            ep.duration.unwrap_or_default().as_str(),
                                        );
                                        if let Ok(episodeduration) = time_sec {
                                            let ep_url = episode_url_clone.clone();
                                            let aud_url = audio_url_clone.clone();
                                            let podcast_title = podcast_title_clone.clone();
                                            let episodeduration: i32 =
                                                episodeduration.try_into().unwrap_or(0);

                                            // Fetch the episode ID using the provided API
                                            match call_get_episode_id(
                                                &server_name,
                                                &api_key.unwrap(),
                                                &user_id,
                                                &podcast_title,
                                                &aud_url,
                                            )
                                            .await
                                            {
                                                Ok(fetched_episode_id) => {
                                                    web_sys::console::log_1(
                                                        &"Episode ID fetched".into(),
                                                    );
                                                    dispatch.reduce_mut(move |state| {
                                                        state.fetched_episode =
                                                            Some(EpisodeMetadataResponse {
                                                                episode: EpisodeInfo {
                                                                    episodetitle: ep
                                                                        .title
                                                                        .unwrap_or_default(),
                                                                    podcastname: podcast_title
                                                                        .clone(),
                                                                    podcastid: 0,
                                                                    feedurl: ep_url.clone(),
                                                                    episodepubdate: ep
                                                                        .pub_date
                                                                        .unwrap_or_default(),
                                                                    episodedescription: ep
                                                                        .description
                                                                        .unwrap_or_default(),
                                                                    episodeartwork: ep
                                                                        .artwork
                                                                        .unwrap_or_default(),
                                                                    episodeurl: ep_url.clone(),
                                                                    episodeduration,
                                                                    listenduration: Some(
                                                                        episodeduration,
                                                                    ),
                                                                    episodeid: fetched_episode_id, // Set the fetched episode ID here
                                                                    completed: false,
                                                                },
                                                            });
                                                        state.selected_episode_id =
                                                            Some(fetched_episode_id); // Set the episode ID here
                                                        state.selected_episode_url =
                                                            Some(ep_url.clone());
                                                        state.selected_episode_audio_url =
                                                            Some(aud_url.clone());
                                                        state.selected_podcast_title =
                                                            Some(podcast_title.clone());
                                                    });

                                                    // Update the URL with the parameters if they are not already there
                                                    let mut new_url =
                                                        window.location().origin().unwrap();
                                                    new_url.push_str(
                                                        &window.location().pathname().unwrap(),
                                                    );
                                                    new_url.push_str("?podcast_title=");
                                                    new_url.push_str(&urlencoding::encode(
                                                        &podcast_title_clone,
                                                    ));
                                                    new_url.push_str("&episode_url=");
                                                    new_url.push_str(&urlencoding::encode(
                                                        &episode_url_clone,
                                                    ));
                                                    new_url.push_str("&audio_url=");
                                                    new_url.push_str(&urlencoding::encode(
                                                        &audio_url_clone,
                                                    ));

                                                    window
                                                        .history()
                                                        .expect("should have a history")
                                                        .push_state_with_url(
                                                            &wasm_bindgen::JsValue::NULL,
                                                            "",
                                                            Some(&new_url),
                                                        )
                                                        .expect("should push state");
                                                    // setting fetched_episode url and title
                                                    // Update the Yewdux state as needed
                                                    dispatch.reduce_mut(move |state| {
                                                        if let Some(episode) =
                                                            state.fetched_episode.clone()
                                                        {
                                                            state.selected_episode_url = Some(
                                                                episode.episode.episodeurl.clone(),
                                                            );
                                                            state.selected_podcast_title = Some(
                                                                episode
                                                                    .episode
                                                                    .episodetitle
                                                                    .clone(),
                                                            );
                                                        }
                                                    });
                                                    effect_ep_in_db.set(true);
                                                    loading_clone.set(false);
                                                }
                                                Err(_) => {
                                                    // If fetching the episode ID fails, set it to 0
                                                    web_sys::console::log_1(
                                                        &"Episode ID fetch failed".into(),
                                                    );
                                                    dispatch.reduce_mut(move |state| {
                                                        state.fetched_episode =
                                                            Some(EpisodeMetadataResponse {
                                                                episode: EpisodeInfo {
                                                                    episodetitle: ep
                                                                        .title
                                                                        .unwrap_or_default(),
                                                                    podcastname: podcast_title
                                                                        .clone(),
                                                                    podcastid: 0,
                                                                    feedurl: ep_url.clone(),
                                                                    episodepubdate: ep
                                                                        .pub_date
                                                                        .unwrap_or_default(),
                                                                    episodedescription: ep
                                                                        .description
                                                                        .unwrap_or_default(),
                                                                    episodeartwork: ep
                                                                        .artwork
                                                                        .unwrap_or_default(),
                                                                    episodeurl: ep_url.clone(),
                                                                    episodeduration,
                                                                    listenduration: Some(
                                                                        episodeduration,
                                                                    ),
                                                                    episodeid: 0, // Set the episode ID to 0
                                                                    completed: false,
                                                                },
                                                            });
                                                        state.selected_episode_id = Some(0); // Set the episode ID to 0
                                                        state.selected_episode_url =
                                                            Some(ep_url.clone());
                                                        state.selected_episode_audio_url =
                                                            Some(aud_url.clone());
                                                        state.selected_podcast_title =
                                                            Some(podcast_title.clone());
                                                    });
                                                    // Update the URL with the parameters if they are not already there
                                                    let mut new_url =
                                                        window.location().origin().unwrap();
                                                    new_url.push_str(
                                                        &window.location().pathname().unwrap(),
                                                    );
                                                    new_url.push_str("?podcast_title=");
                                                    new_url.push_str(&urlencoding::encode(
                                                        &podcast_title_clone,
                                                    ));
                                                    new_url.push_str("&episode_url=");
                                                    new_url.push_str(&urlencoding::encode(
                                                        &episode_url_clone,
                                                    ));
                                                    new_url.push_str("&audio_url=");
                                                    new_url.push_str(&urlencoding::encode(
                                                        &audio_url_clone,
                                                    ));

                                                    window
                                                        .history()
                                                        .expect("should have a history")
                                                        .push_state_with_url(
                                                            &wasm_bindgen::JsValue::NULL,
                                                            "",
                                                            Some(&new_url),
                                                        )
                                                        .expect("should push state");
                                                    dispatch.reduce_mut(move |state| {
                                                        if let Some(episode) =
                                                            state.fetched_episode.clone()
                                                        {
                                                            state.selected_episode_url = Some(
                                                                episode.episode.episodeurl.clone(),
                                                            );
                                                            state.selected_podcast_title = Some(
                                                                episode
                                                                    .episode
                                                                    .episodetitle
                                                                    .clone(),
                                                            );
                                                        }
                                                    });
                                                    loading_clone.set(false);
                                                }
                                            }
                                        } else {
                                            error_clone
                                                .set(Some("Failed to parse duration".to_string()));
                                        }
                                    }
                                }
                                Err(e) => {
                                    error_clone.set(Some(e.to_string()));
                                }
                            }
                        });
                    } else if let Some(id) = episode_id {
                        // Handle the case where no URL parameters are provided (original behavior)
                        if id == 0 {
                            web_sys::console::log_1(&"Episode ID is 0".into());
                            let feed_url = effect_pod_state.selected_episode_url.clone().unwrap();
                            let podcast_title =
                                effect_pod_state.selected_podcast_title.clone().unwrap();
                            let audio_url =
                                effect_pod_state.selected_episode_audio_url.clone().unwrap();
                            // Update the URL with the parameters if they are not already there
                            let mut new_url = window.location().origin().unwrap();
                            new_url.push_str(&window.location().pathname().unwrap());
                            new_url.push_str("?podcast_title=");
                            new_url.push_str(&urlencoding::encode(&podcast_title));
                            new_url.push_str("&episode_url=");
                            new_url.push_str(&urlencoding::encode(&feed_url));
                            new_url.push_str("&audio_url=");
                            new_url.push_str(&urlencoding::encode(&audio_url));

                            window
                                .history()
                                .expect("should have a history")
                                .push_state_with_url(
                                    &wasm_bindgen::JsValue::NULL,
                                    "",
                                    Some(&new_url),
                                )
                                .expect("should push state");

                            wasm_bindgen_futures::spawn_local(async move {
                                match call_parse_podcast_url(server_name, &api_key, &feed_url).await
                                {
                                    Ok(result) => {
                                        if let Some(ep) = result
                                            .episodes
                                            .iter()
                                            .find(|ep| {
                                                ep.enclosure_url.as_ref()
                                                    == Some(&audio_url.clone())
                                            })
                                            .cloned()
                                        {
                                            let time_sec = convert_time_to_seconds(
                                                ep.duration.unwrap_or_default().as_str(),
                                            );
                                            if let Ok(episodeduration) = time_sec {
                                                let episodeduration: i32 =
                                                    episodeduration.try_into().unwrap_or(0);
                                                dispatch.reduce_mut(move |state| {
                                                    state.fetched_episode =
                                                        Some(EpisodeMetadataResponse {
                                                            episode: EpisodeInfo {
                                                                episodetitle: ep
                                                                    .title
                                                                    .unwrap_or_default(),
                                                                podcastname: podcast_title.clone(),
                                                                podcastid: 0,
                                                                feedurl: feed_url.clone(),
                                                                episodepubdate: ep
                                                                    .pub_date
                                                                    .unwrap_or_default(),
                                                                episodedescription: ep
                                                                    .description
                                                                    .unwrap_or_default(),
                                                                episodeartwork: ep
                                                                    .artwork
                                                                    .unwrap_or_default(),
                                                                episodeurl: feed_url.clone(),
                                                                episodeduration,
                                                                listenduration: Some(
                                                                    episodeduration,
                                                                ),
                                                                episodeid: 0,
                                                                completed: false,
                                                            },
                                                        });
                                                });
                                                loading_clone.set(false);
                                            } else {
                                                error_clone.set(Some(
                                                    "Failed to parse duration".to_string(),
                                                ));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error_clone.set(Some(e.to_string()));
                                    }
                                }
                            });
                        } else {
                            web_sys::console::log_1(&"Episode ID is not 0".into());
                            let episode_request = EpisodeRequest {
                                episode_id: id,
                                user_id: user_id.clone(),
                            };
                            effect_ep_in_db.set(true);

                            wasm_bindgen_futures::spawn_local(async move {
                                match pod_req::call_get_episode_metadata(
                                    &server_name,
                                    api_key,
                                    &episode_request,
                                )
                                .await
                                {
                                    Ok(fetched_episode) => {
                                        let episode_url = fetched_episode.feedurl.clone();
                                        let podcast_title = fetched_episode.podcastname.clone();
                                        let audio_url = fetched_episode.episodeurl.clone();

                                        dispatch.reduce_mut(move |state| {
                                            state.fetched_episode = Some(EpisodeMetadataResponse {
                                                episode: fetched_episode,
                                            });
                                        });

                                        // Add URL parameters
                                        let window =
                                            web_sys::window().expect("no global window exists");
                                        let mut new_url = window.location().origin().unwrap();
                                        new_url.push_str(&window.location().pathname().unwrap());
                                        new_url.push_str("?podcast_title=");
                                        new_url.push_str(&urlencoding::encode(&podcast_title));
                                        new_url.push_str("&episode_url=");
                                        new_url.push_str(&urlencoding::encode(&episode_url));
                                        new_url.push_str("&audio_url=");
                                        new_url.push_str(&urlencoding::encode(&audio_url));

                                        window
                                            .history()
                                            .expect("should have a history")
                                            .push_state_with_url(
                                                &wasm_bindgen::JsValue::NULL,
                                                "",
                                                Some(&new_url),
                                            )
                                            .expect("should push state");
                                        loading_clone.set(false);
                                    }
                                    Err(e) => {
                                        error_clone.set(Some(e.to_string()));
                                    }
                                }
                            });
                        }
                    }
                }
                || ()
            },
        );
    }

    // Get pocasting 2.0 data if available
    {
        use_effect_with(
            (
                episode_id.clone(),
                user_id.clone(),
                api_key.clone(),
                server_name.clone(),
            ),
            {
                let dispatch = audio_dispatch.clone();
                move |(episode_id, user_id, api_key, server_name)| {
                    if let (Some(episode_id), Some(user_id), Some(api_key), Some(server_name)) =
                        (episode_id, user_id, api_key, server_name)
                    {
                        let episode_id = *episode_id; // Dereference the option
                        let user_id = *user_id; // Dereference the option
                        let api_key = api_key.clone(); // Clone to make it owned
                        let server_name = server_name.clone(); // Clone to make it owned

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
                                    let transcripts = response.transcripts.clone(); // Clone transcripts to avoid move issue
                                    let people = response.people.clone(); // Clone people to avoid move issue
                                    dispatch.reduce_mut(|state| {
                                        state.episode_page_transcript = Some(transcripts);
                                        state.episode_page_people = Some(people);
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
                    || ()
                }
            },
        );
    }

    let completion_status = use_state(|| false); // State to track completion status

    // let copy_to_clipboard = Callback::from(move |_| {
    //     let window = window().expect("global window does not exist");
    //     let document = window.document().expect("document not available");

    //     if let Some(input_element) = document
    //         .get_element_by_id("share_link")
    //         .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
    //     {
    //         input_element.select();

    //         // Access the clipboard API directly
    //         let navigator = window.navigator();
    //         if let Some(clipboard) = navigator.clipboard() {
    //             let value_to_copy = input_element.value();
    //             spawn_local(async move {
    //                 match wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&value_to_copy)).await {
    //                     Ok(_) => {
    //                         web_sys::console::log_1(&"Link copied to clipboard!".into());
    //                     }
    //                     Err(err) => {
    //                         web_sys::console::log_1(&format!("Error copying to clipboard: {:?}", err).into());
    //                     }
    //                 }
    //             });
    //         } else {
    //             web_sys::console::log_1(&"Clipboard API not available".into());
    //         }
    //     }
    // });

    {
        let state = state.clone();
        let completion_status = completion_status.clone();

        // Update the completion status when the fetched episode changes
        use_effect_with(state.fetched_episode.clone(), move |_| {
            if let Some(episode) = &state.fetched_episode {
                completion_status.set(episode.episode.completed);
            }
            || ()
        });
    }

    let page_state = use_state(|| PageState::Hidden);

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
    }

    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
        })
    };

    let on_background_click = {
        let on_close_modal = on_close_modal.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            if element.tag_name() == "DIV" {
                on_close_modal.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let shared_url_copy = shared_url.clone();
    let create_share_link = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let episode_id = episode_id.clone();
        let page_state_create = page_state.clone(); // Handle modal visibility
        let shared_url_create = shared_url_copy.clone(); // Store the created shared link

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let ep_id_deref = episode_id.clone().unwrap();
            let shared_url_copy = shared_url_create.clone();
            let page_state_copy = page_state_create.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let api_key_copy = api_key.clone();
                if let (Some(_api_key), Some(server_name)) =
                    (api_key.as_ref(), server_name.as_ref())
                {
                    match call_create_share_link(
                        &server_name,
                        &api_key_copy.unwrap().unwrap(),
                        ep_id_deref,
                    )
                    .await
                    {
                        Ok(url_key) => {
                            let full_url = format!("{}/shared_episode/{}", server_name, url_key);
                            shared_url_copy.set(Some(full_url)); // Store the generated link
                            page_state_copy.set(PageState::Shown); // Show the modal
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error creating share link: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    // Define the modal for showing the shareable link
    let share_url_modal = html! {
        <div id="share_url_modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Copy Shared Link"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <div>
                            <label for="share_link" class="block mb-2 text-sm font-medium">{"Share this link with anyone you'd like to be able to listen to this episode. They don't even need an account on the server to use this!"}</label>
                            <input type="text" id="share_link" class="input-black w-full px-3 py-2 border border-gray-300 rounded-md" value={shared_url.as_ref().map(|url| url.clone()).unwrap_or_else(|| "".to_string())} readonly=true />
                            // <button class="copy-button" onclick={copy_to_clipboard.clone()}>{ "Copy to clipboard" }</button>
                        </div>
                        <div>
                            <label for="share_link" class="block mb-2 text-sm font-medium">{"If they do have an account you can just send the user the current link on this web page:"}</label>
                            <input type="text" id="share_link" class="input-black w-full px-3 py-2 border border-gray-300 rounded-md" value={get_current_url()} readonly=true />
                            // <button class="copy-button" onclick={copy_to_clipboard.clone()}>{ "Copy to clipboard" }</button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    };

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                match *page_state {
                PageState::Shown => share_url_modal,
                _ => html! {},
                }
            }
            {
                if *loading { // If loading is true, display the loading animation
                    html! {
                        <div class="loading-animation">
                            <div class="frame1"></div>
                            <div class="frame2"></div>
                            <div class="frame3"></div>
                            <div class="frame4"></div>
                            <div class="frame5"></div>
                            <div class="frame6"></div>
                        </div>
                    }
                } else {
                    if let Some(episode) = state.fetched_episode.clone() {
                        let episode_url_clone = episode.episode.episodeurl.clone();
                        let episode_title_clone = episode.episode.episodetitle.clone();
                        let episode_artwork_clone = episode.episode.episodeartwork.clone();
                        let episode_duration_clone = episode.episode.episodeduration.clone();
                        let podcast_of_episode = episode.episode.podcastid.clone();
                        let episode_listened_clone = Option::from(0);
                        let episode_id_clone = episode.episode.episodeid.clone();

                        let sanitized_description = sanitize_html_with_blank_target(&episode.episode.episodedescription.clone());
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

                        let complete_server_name = server_name.clone();
                        let complete_api_key = api_key.clone();
                        let complete_post = dispatch.clone();
                        let complete_status_clone = completion_status.clone();
                        let user_id_complete = user_id.clone();

                        let on_complete_episode = {
                            Callback::from(move |_| {
                                let completion_status = complete_status_clone.clone();
                                let post_dispatch = complete_post.clone();
                                let server_name_copy = complete_server_name.clone();
                                let api_key_copy = complete_api_key.clone();
                                let request = MarkEpisodeCompletedRequest {
                                    episode_id: episode_id_for_closure,
                                    user_id: user_id_complete.unwrap(), // replace with the actual user ID
                                };
                                let server_name = server_name_copy; // replace with the actual server name
                                let api_key = api_key_copy; // replace with the actual API key
                                let future = async move {
                                    // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                                    // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                                    match call_mark_episode_completed(
                                        &server_name.unwrap(),
                                        &api_key.flatten(),
                                        &request,
                                    )
                                    .await
                                    {
                                        Ok(success_message) => {
                                            completion_status.set(true);
                                            // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                                            post_dispatch.reduce_mut(|state| {
                                                if let Some(completed_episodes) = state.completed_episodes.as_mut() {
                                                    if let Some(pos) =
                                                        completed_episodes.iter().position(|&id| id == episode_id_for_closure)
                                                    {
                                                        completed_episodes.remove(pos);
                                                    } else {
                                                        completed_episodes.push(episode_id_for_closure);
                                                    }
                                                } else {
                                                    state.completed_episodes = Some(vec![episode_id_for_closure]);
                                                }
                                                state.info_message = Some(format!("{}", success_message));
                                            });
                                        }
                                        Err(e) => {
                                            post_dispatch.reduce_mut(|state| {
                                                state.error_message = Option::from(format!("{}", e))
                                            });
                                            // Handle error, e.g., display the error message
                                        }
                                    }
                                };
                                wasm_bindgen_futures::spawn_local(future);
                                // dropdown_open.set(false);
                            })
                        };

                        let uncomplete_server_name = server_name.clone();
                        let uncomplete_api_key = api_key.clone();
                        let uncomplete_post = dispatch.clone();
                        let user_id_uncomplete = user_id.clone();
                        let uncomplete_status_clone = completion_status.clone();

                        let on_uncomplete_episode = {
                            Callback::from(move |_| {
                                let completion_status = uncomplete_status_clone.clone();
                                let post_dispatch = uncomplete_post.clone();
                                let server_name_copy = uncomplete_server_name.clone();
                                let api_key_copy = uncomplete_api_key.clone();
                                let request = MarkEpisodeCompletedRequest {
                                    episode_id: episode_id_for_closure,
                                    user_id: user_id_uncomplete.unwrap(), // replace with the actual user ID
                                };
                                let server_name = server_name_copy; // replace with the actual server name
                                let api_key = api_key_copy; // replace with the actual API key
                                let future = async move {
                                    // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                                    // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                                    match call_mark_episode_uncompleted(
                                        &server_name.unwrap(),
                                        &api_key.flatten(),
                                        &request,
                                    )
                                    .await
                                    {
                                        Ok(success_message) => {
                                            completion_status.set(false);
                                            // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                                            post_dispatch.reduce_mut(|state| {
                                                if let Some(completed_episodes) = state.completed_episodes.as_mut() {
                                                    if let Some(pos) =
                                                        completed_episodes.iter().position(|&id| id == episode_id_for_closure)
                                                    {
                                                        completed_episodes.remove(pos);
                                                    } else {
                                                        completed_episodes.push(episode_id_for_closure);
                                                    }
                                                } else {
                                                    state.completed_episodes = Some(vec![episode_id_for_closure]);
                                                }
                                                state.info_message = Some(format!("{}", success_message));
                                            });
                                        }
                                        Err(e) => {
                                            post_dispatch.reduce_mut(|state| {
                                                state.error_message = Option::from(format!("{}", e))
                                            });
                                            // Handle error, e.g., display the error message
                                        }
                                    }
                                };
                                wasm_bindgen_futures::spawn_local(future);
                                // dropdown_open.set(false);
                            })
                        };

                        let toggle_completion = {
                            let completion_status = completion_status.clone();
                            Callback::from(move |_| {
                                // Toggle the completion status
                                if *completion_status {
                                    on_uncomplete_episode.emit(());
                                } else {
                                    on_complete_episode.emit(());
                                }
                            })
                        };

                        let datetime = parse_date(&episode.episode.episodepubdate, &state.user_tz);
                        let date_format = match_date_format(state.date_format.as_deref());
                        let format_duration = format_time(episode.episode.episodeduration as f64);
                        let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));

                        let on_title_click = {
                            let dispatch = dispatch.clone();
                            let server_name = server_name.clone();
                            let api_key = api_key.clone();
                            let podcast_id = podcast_of_episode.clone();
                            let user_id = user_id.clone();
                            let history = history.clone();

                            Callback::from(move |event: MouseEvent| {
                                let dispatch = dispatch.clone();
                                let server_name = server_name.clone();
                                let api_key = api_key.clone();
                                let podcast_id = podcast_id.clone();
                                let user_id = user_id.clone();
                                let history = history.clone();

                                wasm_bindgen_futures::spawn_local(async move {
                                    match pod_req::call_get_podcast_details(&server_name.clone().unwrap(), &api_key.clone().unwrap().unwrap(), user_id.unwrap(), &podcast_id).await {
                                        Ok(details) => {
                                            // Assuming details contain all necessary podcast info
                                            let final_click_action = create_on_title_click(
                                                dispatch.clone(),
                                                server_name.unwrap(),
                                                api_key,
                                                &history,
                                                details.podcastname,
                                                details.feedurl,
                                                details.description,
                                                details.author,
                                                details.artworkurl,
                                                details.explicit,
                                                details.episodecount,
                                                Some(details.categories),
                                                details.websiteurl,
                                                user_id.unwrap(),
                                            );

                                            // Execute the action created by create_on_title_click
                                            final_click_action.emit(event);
                                        },
                                        Err(error) => {
                                            web_sys::console::log_1(&format!("Error fetching podcast details: {}", error).into());
                                            dispatch.reduce_mut(move |state| {
                                                state.error_message = Some(format!("Failed to load details: {}", error));
                                            });
                                        }
                                    }
                                });
                            })
                        };
                        let episode_url_check = episode_url_clone;
                        let should_show_buttons = !episode_url_check.is_empty();
                        web_sys::console::log_1(&format!("Episode URL: {}", episode_url_check).into());
                        web_sys::console::log_1(&format!("Should show buttons: {}", should_show_buttons).into());

                        let open_in_new_tab = Callback::from(move |url: String| {
                            let window = web_sys::window().unwrap();
                            window.open_with_url_and_target(&url, "_blank").unwrap();
                        });
                        // let format_duration = format!("Duration: {} minutes", e / 60); // Assuming duration is in seconds
                        // let format_release = format!("Released on: {}", &episode.episode.EpisodePubDate);
                        let layout = if audio_state.is_mobile.unwrap_or(false) {
                            html! {
                                <div class="mobile-layout">
                                <div class="episode-layout-container">
                                        <div class="item-header-mobile-cover-container">
                                        <img src={episode.episode.episodeartwork.clone()} class="episode-artwork" />
                                        </div>
                                            <div class="episode-details">
                                            <p class="item-header-pod justify-center items-center" onclick={on_title_click.clone()}>{ &episode.episode.podcastname }</p>
                                            <div class="items-center space-x-2 cursor-pointer">
                                                <h2 class="episode-title item-header-title">
                                                    { &episode.episode.episodetitle }
                                                    {
                                                        if *completion_status.clone() {
                                                            html! {
                                                                <span class="material-bonus-color item_container-text material-icons text-md text-green-500">{"check_circle"}</span>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        html! {
                                                            <button onclick={create_share_link.clone()} class="play-button">
                                                                <i class="material-icons">{ "share" }</i>
                                                            </button>
                                                        }
                                                    }
                                                </h2>
                                            </div>
                                            // <h2 class="episode-title">{ &episode.episode.episodetitle }</h2>
                                            <div class="flex justify-center items-center item-header-details">
                                                <p class="episode-duration">{ format_duration }</p>
                                                <span class="episode-duration">{"\u{00a0}-\u{00a0}"}</span>
                                                <p class="episode-release-date">{ format_release }</p>
                                            </div>




                                            {
                                                if let Some(transcript) = &audio_state.episode_page_transcript {
                                                    if !transcript.is_empty() {
                                                        let transcript_clone = transcript.clone();
                                                        html! {
                                                            <>
                                                            { for transcript_clone.iter().map(|transcript| {
                                                                let open_in_new_tab = open_in_new_tab.clone();
                                                                let url = transcript.url.clone();
                                                                html! {
                                                                    <div class="header-info pb-2 pt-2">
                                                                        <button
                                                                            onclick={Callback::from(move |_| open_in_new_tab.emit(url.clone()))}
                                                                            title={"Transcript"}
                                                                            class="font-bold item-container-button"
                                                                        >
                                                                            { "Episode Transcript" }
                                                                        </button>
                                                                    </div>
                                                                }
                                                            })}
                                                            </>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            {
                                                if let Some(people) = &audio_state.episode_page_people {
                                                    if !people.is_empty() {
                                                        html! {
                                                            <div class="header-info-episode">
                                                                <HostDropdown title="In This Episode" hosts={people.clone()} podcast_feed_url={episode.episode.episodeurl} podcast_id={episode.episode.podcastid} />
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                    <div class="episode-action-buttons">
                                    {
                                        if should_show_buttons {
                                            if *ep_in_db {
                                                html! {
                                                    <>
                                                    <div class="button-row">
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
                                                    </div>
                                                    <div class="button-row">
                                                        <button onclick={on_download_episode} class="download-button-ep">
                                                            <i class="material-icons">{ "download" }</i>
                                                            {"Download"}
                                                        </button>
                                                        <button onclick={toggle_completion} class="download-button-ep">
                                                            <i class="material-icons">{ if *completion_status { "check_circle_outline" } else { "check_circle" } }</i>
                                                            { if *completion_status { "Mark Incomplete" } else { "Mark Complete" } }
                                                        </button>
                                                    </div>
                                                    </>
                                                }
                                            } else {
                                                html! {
                                                    <p class="no-media-warning item_container-text play-button">
                                                        {"Add podcast to display actions"}
                                                    </p>
                                                }
                                            }
                                        } else {
                                            html! {
                                                <p class="no-media-warning item_container-text play-button">
                                                    {"This item contains no media file"}
                                                </p>
                                            }
                                        }
                                    }

                                    </div>
                                    <div class="episode-single-desc episode-description">
                                    // <p>{ description }</p>
                                    <div class="item_container-text episode-description-container">
                                        <SafeHtml html={description} />
                                    </div>
                                    </div>
                                </div>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="episode-layout-container">
                                    <div class="episode-top-info">
                                        <img src={episode.episode.episodeartwork.clone()} class="episode-artwork" />
                                        <div class="episode-details">
                                            <h1 class="podcast-title" onclick={on_title_click.clone()}>{ &episode.episode.podcastname }</h1>
                                            <div class="flex items-center space-x-2 cursor-pointer">
                                                <h2 class="episode-title">{ &episode.episode.episodetitle }</h2>
                                                {
                                                    if *completion_status.clone() {
                                                        html! {
                                                            <span class="material-bonus-color item_container-text material-icons text-md text-green-500">{"check_circle"}</span>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </div>
                                            // <h2 class="episode-title">{ &episode.episode.episodetitle }</h2>
                                            <p class="episode-duration">{ format_duration }</p>
                                            <p class="episode-release-date">{ format_release }</p>
                                            {
                                                if let Some(transcript) = &audio_state.episode_page_transcript {
                                                    if !transcript.is_empty() {
                                                        let transcript_clone = transcript.clone();
                                                        html! {
                                                            <>
                                                            { for transcript_clone.iter().map(|transcript| {
                                                                let open_in_new_tab = open_in_new_tab.clone();
                                                                let url = transcript.url.clone();
                                                                html! {
                                                                    <div class="header-info pb-2 pt-2">
                                                                        <button
                                                                            onclick={Callback::from(move |_| open_in_new_tab.emit(url.clone()))}
                                                                            title={"Transcript"}
                                                                            class="font-bold item-container-button"
                                                                        >
                                                                            { "Episode Transcript" }
                                                                        </button>
                                                                    </div>
                                                                }
                                                            })}
                                                            </>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            {
                                                if let Some(people) = &audio_state.episode_page_people {
                                                    if !people.is_empty() {
                                                        html! {
                                                            <div class="header-info">
                                                                <HostDropdown title="In This Episode" hosts={people.clone()} podcast_feed_url={episode.episode.episodeurl} podcast_id={episode.episode.podcastid} />
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            <button
                                                class="share-button font-bold py-2 px-4 rounded"
                                                onclick={create_share_link.clone()}
                                            >
                                                {"Share Episode"}
                                            </button>
                                        </div>
                                    </div>
                                    <div class="episode-action-buttons">
                                    {
                                        if should_show_buttons {
                                            if *ep_in_db {
                                            html! {
                                                <>
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
                                                <button onclick={toggle_completion} class="download-button-ep">
                                                    <i class="material-icons">{ if *completion_status { "check_circle_outline" } else { "check_circle" } }</i>
                                                    { if *completion_status { "Mark Episode Incomplete" } else { "Mark Episode Complete" } }
                                                </button>
                                                </>
                                            }
                                            } else {
                                                html! {
                                                    <p class="no-media-warning item_container-text play-button">
                                                        {"Add podcast to display actions"}
                                                    </p>
                                                }
                                            }
                                        } else {
                                            html! {
                                                <p class="no-media-warning item_container-text play-button">
                                                    {"This item contains no media file"}
                                                </p>
                                            }
                                        }
                                    }

                                    </div>
                                    <hr class="episode-divider" />
                                    <div class="episode-single-desc episode-description">
                                    // <p>{ description }</p>
                                    <div class="item_container-text episode-description-container">
                                        <SafeHtml html={description} />
                                    </div>
                                    </div>
                                </div>
                            }
                        };  // Add semicolon here
                        // item

                        layout
                    } else {
                        empty_message(
                            "Unable to display episode",
                            "Something seems to have gone wrong. A straightup server disconnect maybe? Did you browse here directly? That's not how this app works. It needs the context to browse around. I honestly don't have anything else for you as this shouldn't happen. This is embarrasing."
                        )
                    }
                }
            }
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} /> }
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
