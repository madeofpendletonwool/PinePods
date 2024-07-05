use super::app_drawer::App_drawer;
use super::gen_components::{
    download_episode_item, empty_message, episode_item, on_shownotes_click, Search_nav,
    UseScrollToTop,
};
use crate::components::audio::on_play_click_offline;
use crate::components::audio::AudioPlayer;
use crate::components::context::AppStateMsg;
use crate::components::context::{AppState, UIState};
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
    truncate_description,
};
use crate::requests::pod_req::{
    call_get_episode_downloads, call_get_podcasts, call_remove_downloaded_episode,
    DownloadAllPodcastRequest, DownloadEpisodeRequest, EpisodeDownload, EpisodeDownloadResponse,
    EpisodeInfo, Podcast, PodcastDetails, PodcastResponse,
};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;
// use crate::components::gen_funcs::check_auth;
use crate::components::episodes_layout::UIStateMsg;
use crate::requests::login_requests::use_check_authentication;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::from_value;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;
use tauri_sys::tauri;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
#[cfg(feature = "default")]
use wasm_bindgen_futures::spawn_local;
use web_sys::window;

fn group_episodes_by_podcast(episodes: Vec<EpisodeDownload>) -> HashMap<i32, Vec<EpisodeDownload>> {
    let mut grouped: HashMap<i32, Vec<EpisodeDownload>> = HashMap::new();
    for episode in episodes {
        grouped
            .entry(episode.podcastid)
            .or_insert_with(Vec::new)
            .push(episode);
    }
    grouped
}

pub async fn download_file(url: String, filename: String) -> Result<(), JsValue> {
    // Create a wrapper struct to explicitly define the argument names
    #[derive(Serialize)]
    struct DownloadFileArgs {
        url: String,
        filename: String,
    }

    // Wrap the parameters in the struct with the expected argument names
    let args = DownloadFileArgs { url, filename };

    // Serialize and invoke the Tauri command
    let serialized_data = serde_wasm_bindgen::to_value(&args).unwrap();
    web_sys::console::log_1(&serialized_data); // Debug print

    tauri::invoke::<_, ()>("download_file", &args)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to invoke download: {}", e)))
}

pub async fn fetch_local_file(file_path: &str) -> Result<Vec<u8>, JsValue> {
    #[derive(Serialize)]
    struct GetLocalFileArgs {
        filepath: String,
    }

    let args = GetLocalFileArgs {
        filepath: file_path.to_string(),
    };

    let response = tauri::invoke::<_, Vec<u8>>("get_local_file", &args)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to fetch local file: {}", e)))?;
    Ok(response)
}

pub async fn start_local_file_server(file_path: &str) -> Result<String, JsValue> {
    #[derive(Serialize)]
    struct StartFileServerArgs {
        filepath: String,
    }

    let args = StartFileServerArgs {
        filepath: file_path.to_string(),
    };

    tauri::invoke::<_, String>("start_file_server", &args)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to start local file server: {}", e)))
}

pub async fn update_local_database(episode_info: EpisodeInfo) -> Result<(), JsValue> {
    // Create a wrapper struct to explicitly define the argument name
    #[derive(Serialize)]
    struct UpdateLocalDbArgs {
        episodeInfo: EpisodeInfo,
    }

    // Wrap the episode_info in the struct with the expected argument name
    let args = UpdateLocalDbArgs {
        episodeInfo: episode_info,
    };

    // Serialize and invoke the Tauri command
    let serialized_data = serde_wasm_bindgen::to_value(&args).unwrap();
    web_sys::console::log_1(&serialized_data); // Debug print

    tauri::invoke::<_, ()>("update_local_db", &args)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to update local DB: {}", e)))
}

pub async fn remove_episode_from_local_db(episode_id: i32) -> Result<(), JsValue> {
    #[derive(Serialize)]
    struct RemoveEpisodeFromLocalDbArgs {
        episodeid: i32,
    }

    let args = RemoveEpisodeFromLocalDbArgs {
        episodeid: episode_id,
    };

    tauri::invoke::<_, ()>("remove_from_local_db", &args)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to start local file server: {}", e)))
}

pub async fn fetch_local_episodes() -> Result<Vec<EpisodeDownload>, JsValue> {
    tauri::invoke::<_, Vec<EpisodeDownload>>("get_local_episodes", &())
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to fetch local episodes: {:?}", e)))
}

pub async fn update_podcast_database(podcast_details: PodcastDetails) -> Result<(), JsValue> {
    // Create a wrapper struct to explicitly define the argument name
    #[derive(Serialize)]
    struct UpdatePodcastDbArgs {
        podcastDetails: PodcastDetails,
    }

    // Wrap the podcast_details in the struct with the expected argument name
    let args = UpdatePodcastDbArgs {
        podcastDetails: podcast_details,
    };

    // Serialize and invoke the Tauri command
    let serialized_data = serde_wasm_bindgen::to_value(&args).unwrap();
    web_sys::console::log_1(&serialized_data); // Debug print

    tauri::invoke::<_, ()>("update_podcast_db", &args)
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to update podcast DB: {:?}", e)))
}

pub async fn fetch_local_podcasts() -> Result<Vec<Podcast>, JsValue> {
    tauri::invoke::<_, Vec<Podcast>>("get_local_podcasts", &())
        .await
        .map_err(|e| JsValue::from_str(&format!("Failed to fetch local podcasts: {:?}", e)))
}

// Define the arguments for the Tauri command
#[derive(Serialize, Deserialize)]
struct ListDirArgs<'a> {
    path: &'a str,
}

// Define the structure for the file entries
#[derive(Deserialize)]
struct FileEntry {
    path: String,
}

// Function to invoke the Tauri command and get the result
async fn invoke_list_home_dir() -> Result<Vec<FileEntry>, JsValue> {
    let args = ListDirArgs { path: "~" };
    tauri::invoke::<_, Vec<FileEntry>>("list_dir", &args)
        .await
        .map_err(|e| JsValue::from_str(&e.to_string()))
}

fn deserialize_js_value<T: for<'de> serde::Deserialize<'de>>(
    value: wasm_bindgen::JsValue,
) -> Result<T, serde_wasm_bindgen::Error> {
    serde_wasm_bindgen::from_value(value)
}

#[function_component(Downloads)]
pub fn downloads() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let effect_dispatch = dispatch.clone();
    let history = BrowserHistory::new();

    let session_dispatch = effect_dispatch.clone();
    let session_state = state.clone();
    let expanded_state = use_state(HashMap::new);

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
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();
    let app_offline_mode = audio_state.app_offline_mode;
    let page_state = use_state(|| PageState::Normal);
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let loading = use_state(|| true);

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

    // Fetch episodes on component mount
    let loading_ep = loading.clone();
    let local_download_increment = audio_state.local_download_increment;
    {
        let error = error.clone();
        let effect_dispatch = dispatch.clone();

        use_effect_with((local_download_increment,), move |_| {
            let error_clone = error.clone();
            let dispatch = effect_dispatch.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match fetch_local_podcasts().await {
                    Ok(fetched_podcasts) => {
                        web_sys::console::log_1(
                            &format!("Fetched Podcasts: {:?}", fetched_podcasts).into(),
                        );
                        dispatch.reduce_mut(move |state| {
                            state.podcast_feed_return = Some(PodcastResponse {
                                pods: Some(fetched_podcasts),
                            });
                        });
                    }
                    Err(e) => {
                        web_sys::console::log_1(
                            &format!("Unable to parse Podcasts: {:?}", e).into(),
                        );
                    }
                }
                web_sys::console::log_1(&"Fetching Episodes".into());

                match fetch_local_episodes().await {
                    Ok(fetched_episodes) => {
                        web_sys::console::log_1(
                            &format!("Fetched Episodes: {:?}", fetched_episodes).into(),
                        );
                        let completed_episode_ids: Vec<i32> = fetched_episodes
                            .iter()
                            .filter(|ep| ep.listenduration.is_some())
                            .map(|ep| ep.episodeid)
                            .collect();
                        dispatch.reduce_mut(move |state| {
                            state.downloaded_episodes = Some(EpisodeDownloadResponse {
                                episodes: fetched_episodes,
                            });
                            state.completed_episodes = Some(completed_episode_ids);
                        });
                        loading_ep.set(false);
                    }
                    Err(e) => {
                        web_sys::console::log_1(
                            &format!("Unable to parse Episodes: {:?}", e).into(),
                        );
                        error_clone.set(Some(format!("{:?}", e)));
                        loading_ep.set(false);
                    }
                }
            });

            || ()
        });
    }

    // // Fetch episodes on component mount
    // let loading_ep = loading.clone();
    // {
    //     let error = error.clone();
    //     let api_key = post_state
    //         .auth_details
    //         .as_ref()
    //         .map(|ud| ud.api_key.clone());
    //     let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    //     let server_name = post_state
    //         .auth_details
    //         .as_ref()
    //         .map(|ud| ud.server_name.clone());

    //     let effect_dispatch = dispatch.clone();

    //     // fetch_episodes(api_key.flatten(), user_id, server_name, dispatch, error, pod_req::call_get_recent_eps);

    //     use_effect_with(
    //         (api_key.clone(), user_id.clone(), server_name.clone()),
    //         move |_| {
    //             let error_clone = error.clone();
    //             if let (Some(api_key), Some(user_id), Some(server_name)) =
    //                 (api_key.clone(), user_id.clone(), server_name.clone())
    //             {
    //                 let dispatch = effect_dispatch.clone();

    //                 wasm_bindgen_futures::spawn_local(async move {
    //                     match call_get_podcasts(&server_name, &api_key, &user_id).await {
    //                         Ok(fetched_podcasts) => {
    //                             dispatch.reduce_mut(move |state| {
    //                                 state.podcast_feed_return = Some(PodcastResponse {
    //                                     pods: Some(fetched_podcasts),
    //                                 });
    //                             });
    //                         }
    //                         Err(e) => web_sys::console::log_1(
    //                             &format!("Unable to parse Podcasts: {:?}", &e).into(),
    //                         ),
    //                     }

    //                     match call_get_episode_downloads(&server_name, &api_key, &user_id).await {
    //                         Ok(fetched_episodes) => {
    //                             let completed_episode_ids: Vec<i32> = fetched_episodes
    //                                 .iter()
    //                                 .filter(|ep| ep.completed)
    //                                 .map(|ep| ep.episodeid)
    //                                 .collect();
    //                             dispatch.reduce_mut(move |state| {
    //                                 state.downloaded_episodes = Some(EpisodeDownloadResponse {
    //                                     episodes: fetched_episodes,
    //                                 });
    //                                 state.completed_episodes = Some(completed_episode_ids);
    //                             });
    //                             loading_ep.set(false);
    //                             // web_sys::console::log_1(&format!("State after update: {:?}", state).into()); // Log state after update
    //                         }
    //                         Err(e) => {
    //                             error_clone.set(Some(e.to_string()));
    //                             loading_ep.set(false);
    //                         }
    //                     }
    //                 });
    //             }
    //             || ()
    //         },
    //     );
    // }

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Delete,
        Normal,
    }

    // Define the function to Enter Delete Mode
    let delete_mode_enable = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Delete);
        })
    };

    // Define the function to Exit Delete Mode
    let delete_mode_disable = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Normal);
        })
    };

    let on_checkbox_change = {
        let dispatch = dispatch.clone();
        Callback::from(move |episode_id: i32| {
            dispatch.reduce_mut(move |state| {
                // Update the state of the selected episodes for deletion
                state.selected_episodes_for_deletion.insert(episode_id);
            });
        })
    };

    let delete_selected_episodes = {
        let dispatch = dispatch.clone();
        let page_state = page_state.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone(); // Make sure this is cloned from a state or props where it's guaranteed to exist.

        Callback::from(move |_: MouseEvent| {
            // Clone values for use inside the async block
            let dispatch_cloned = dispatch.clone();
            let page_state_cloned = page_state.clone();
            let server_name_cloned = server_name.clone().unwrap(); // Assuming you've ensured these are present
            let api_key_cloned = api_key.clone().unwrap();
            let user_id_cloned = user_id.unwrap();

            dispatch.reduce_mut(move |state| {
                let selected_episodes = state.selected_episodes_for_deletion.clone();
                // Clear the selected episodes for deletion right away to prevent re-deletion in case of re-render
                state.selected_episodes_for_deletion.clear();

                for &episode_id in &selected_episodes {
                    let request = DownloadEpisodeRequest {
                        episode_id,
                        user_id: user_id_cloned,
                    };
                    let server_name_cloned = server_name_cloned.clone();
                    let api_key_cloned = api_key_cloned.clone();
                    let future = async move {
                        match call_remove_downloaded_episode(
                            &server_name_cloned,
                            &api_key_cloned,
                            &request,
                        )
                        .await
                        {
                            Ok(success_message) => Some((success_message, episode_id)),
                            Err(_) => None,
                        }
                    };

                    let dispatch_for_future = dispatch_cloned.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some((success_message, episode_id)) = future.await {
                            dispatch_for_future.reduce_mut(|state| {
                                if let Some(downloaded_episodes) = &mut state.downloaded_episodes {
                                    downloaded_episodes
                                        .episodes
                                        .retain(|ep| ep.episodeid != episode_id);
                                }
                                state.info_message = Some(success_message);
                            });
                        }
                    });
                }

                page_state_cloned.set(PageState::Normal); // Return to normal state after operations
            });
        })
    };

    let is_delete_mode = **page_state.borrow() == PageState::Delete; // Add this line

    let toggle_expanded = {
        let expanded_state = expanded_state.clone();
        Callback::from(move |podcast_id: i32| {
            expanded_state.set({
                let mut new_state = (*expanded_state).clone();
                new_state.insert(podcast_id, !new_state.get(&podcast_id).unwrap_or(&false));
                new_state
            });
        })
    };

    let search_options = if app_offline_mode.unwrap_or(false) {
        html! {}
    } else {
        html! {
            <Search_nav />
        }
    };
    let drawer_options = if app_offline_mode.unwrap_or(false) {
        html! {}
    } else {
        html! {
            <App_drawer />
        }
    };
    let h1_top = if app_offline_mode.unwrap_or(false) {
        html! {
            <h1 class="text-2xl item_container-text font-bold text-center mb-6 pt-6">{"Locally Downloaded Episodes"}</h1>
        }
    } else {
        html! {
            <h1 class="text-2xl item_container-text font-bold text-center mb-6">{"Locally Downloaded Episodes"}</h1>
        }
    };

    html! {
        <>
        <div class="main-container">
            {search_options}
            <UseScrollToTop />
                if *loading { // If loading is true, display the loading animation
                    {
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
                    }
                } else {
                    {
                        html! {
                            <div>
                                {h1_top}
                                <div class="flex justify-between">
                                    {
                                        if **page_state.borrow() == PageState::Normal {
                                            html! {
                                                <button class="download-button font-bold py-2 px-4 rounded inline-flex items-center"
                                                    onclick={delete_mode_enable.clone()}>
                                                    <span class="material-icons icon-space">{"check_box"}</span>
                                                    <span class="text-lg">{"Select Multiple"}</span>
                                                </button>
                                            }
                                        } else {
                                            html! {
                                                <>
                                                <button class="download-button font-bold py-2 px-4 rounded inline-flex items-center"
                                                    onclick={delete_mode_disable.clone()}>
                                                    <span class="material-icons icon-space">{"cancel"}</span>
                                                    <span class="text-lg">{"Cancel"}</span>
                                                </button>
                                                <button class="download-button font-bold py-2 px-4 rounded inline-flex items-center"
                                                    onclick={delete_selected_episodes.clone()}>
                                                    <span class="material-icons icon-space">{"delete"}</span>
                                                    <span class="text-lg">{"Delete"}</span>
                                                </button>
                                                </>
                                            }
                                        }
                                    }
                                </div>
                            </div>
                        }
                    }

                    {
                    if let Some(download_eps) = state.downloaded_episodes.clone() {
                        let int_download_eps = download_eps.clone();
                            let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
                            let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
                            let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
                            let history_clone = history.clone();
                            let render_state = post_state.clone();
                            let dispatch_cloned = dispatch.clone();

                            if int_download_eps.episodes.is_empty() {
                                // Render "No Recent Episodes Found" if episodes list is empty
                                empty_message(
                                    "No Downloaded Episodes Found",
                                    "This is where episode downloads will appear. To download an episode you can open the context menu on an episode and select Download Episode. It will then download the the server and show up here!"
                                )
                            } else {
                                let grouped_episodes = group_episodes_by_podcast(int_download_eps.episodes);

                                html! {
                                    <>
                                        { for state.podcast_feed_return.as_ref().unwrap().pods.as_ref().unwrap().iter().filter_map(|podcast| {
                                            let episodes = grouped_episodes.get(&podcast.podcastid).unwrap_or(&Vec::new()).clone();
                                            if episodes.is_empty() {
                                                None
                                            } else {
                                                let is_expanded = *expanded_state.get(&podcast.podcastid).unwrap_or(&false);
                                                let toggle_expanded_closure = {
                                                    let podcast_id = podcast.podcastid;
                                                    toggle_expanded.reform(move |_| podcast_id)
                                                };

                                                let render_state_cloned = render_state.clone();
                                                let dispatch_cloned_cloned = dispatch_cloned.clone();
                                                let audio_state_cloned = audio_state.clone();
                                                let audio_dispatch_cloned = audio_dispatch.clone();
                                                let on_checkbox_change_cloned = on_checkbox_change.clone();

                                                Some(render_podcast_with_episodes(
                                                    podcast,
                                                    episodes,
                                                    is_expanded,
                                                    toggle_expanded_closure,
                                                    render_state_cloned,
                                                    dispatch_cloned_cloned,
                                                    is_delete_mode,
                                                    audio_state_cloned,
                                                    audio_dispatch_cloned,
                                                    on_checkbox_change_cloned,
                                                ))
                                            }
                                        }) }
                                    </>
                                }

                            }


                        } else {
                            empty_message(
                                "No Episode Downloads Found",
                                "This is where episode downloads will appear. To download an episode you can open the context menu on an episode and select Download Episode. It will then download to the server and show up here!"
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
        {drawer_options}
        </>
    }
}

pub fn render_podcast_with_episodes(
    podcast: &Podcast,
    episodes: Vec<EpisodeDownload>,
    is_expanded: bool,
    toggle_expanded: Callback<MouseEvent>,
    state: Rc<AppState>,
    dispatch: Dispatch<AppState>,
    is_delete_mode: bool,
    audio_state: Rc<UIState>,
    audio_dispatch: Dispatch<UIState>,
    on_checkbox_change: Callback<i32>,
) -> Html {
    let history_clone = BrowserHistory::new();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    html! {
        <div key={podcast.podcastid}>
            <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full" onclick={toggle_expanded}>
                <div class="flex flex-col w-auto object-cover pl-4">
                    <img
                        src={podcast.artworkurl.clone()}
                        alt={format!("Cover for {}", podcast.podcastname.clone())}
                        class="object-cover align-top-cover w-full item-container img"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <p class="item_container-text text-xl font-semibold cursor-pointer">
                        { &podcast.podcastname }
                    </p>
                    <hr class="my-2 border-t hidden md:block"/>
                    <p class="item_container-text">{ format!("Episode Count: {}", &podcast.episodecount) }</p>
                </div>
            </div>
            { if is_expanded {
                html! {
                    <div class="episodes-dropdown, pl-4">
                        { for episodes.into_iter().map(|episode| {
                            let id_string = &episode.episodeid.to_string();

                            let is_expanded = state.expanded_descriptions.contains(id_string);

                            let dispatch = dispatch.clone();

                            let episode_url_clone = episode.episodeurl.clone();
                            let episode_title_clone = episode.episodetitle.clone();
                            let episode_artwork_clone = episode.episodeartwork.clone();
                            let episode_duration_clone = episode.episodeduration.clone();
                            let episode_id_clone = episode.episodeid.clone();
                            let episode_listened_clone = episode.listenduration.clone();
                            let completed = episode.completed;

                            let sanitized_description = sanitize_html_with_blank_target(&episode.episodedescription.clone());

                            let (description, _is_truncated) = if is_expanded {
                                (sanitized_description, false)
                            } else {
                                truncate_description(sanitized_description, 300)
                            };

                            let toggle_expanded = {
                                let search_dispatch_clone = dispatch.clone();
                                let state_clone = state.clone();
                                let episode_guid = episode.episodeid.clone();

                                Callback::from(move |_: MouseEvent| {
                                    let guid_clone = episode_guid.to_string().clone();
                                    let search_dispatch_call = search_dispatch_clone.clone();

                                    if state_clone.expanded_descriptions.contains(&guid_clone) {
                                        search_dispatch_call.apply(AppStateMsg::CollapseEpisode(guid_clone));
                                    } else {
                                        search_dispatch_call.apply(AppStateMsg::ExpandEpisode(guid_clone));
                                    }
                                })
                            };

                            let episode_url_for_closure = episode_url_clone.clone();
                            let episode_title_for_closure = episode_title_clone.clone();
                            let episode_artwork_for_closure = episode_artwork_clone.clone();
                            let episode_duration_for_closure = episode_duration_clone.clone();
                            let listener_duration_for_closure = episode_listened_clone.clone();
                            let episode_id_for_closure = episode_id_clone.clone();
                            let user_id_play = user_id.clone();
                            let server_name_play = server_name.clone();
                            let api_key_play = api_key.clone();
                            let audio_dispatch = audio_dispatch.clone();
                            let is_local = Option::from(true);

                            let on_play_click = on_play_click_offline(episode.clone(), audio_dispatch);

                            let on_shownotes_click = on_shownotes_click(
                                history_clone.clone(),
                                dispatch.clone(),
                                episode_id_for_closure.clone(),
                            );

                            let date_format = match_date_format(state.date_format.as_deref());
                            let datetime = parse_date(&episode.episodepubdate, &state.user_tz);
                            let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));
                            let on_checkbox_change_cloned = on_checkbox_change.clone();
                            let episode_url_for_ep_item = episode_url_clone.clone();
                            let check_episode_id = &episode.episodeid.clone();
                            let is_completed = state
                                .completed_episodes
                                .as_ref()
                                .unwrap_or(&vec![])
                                .contains(&check_episode_id);
                            download_episode_item(
                                Box::new(episode),
                                description.clone(),
                                is_expanded,
                                &format_release,
                                on_play_click,
                                on_shownotes_click,
                                toggle_expanded,
                                episode_duration_clone,
                                episode_listened_clone,
                                "local_downloads",
                                on_checkbox_change_cloned, // Add this line
                                is_delete_mode, // Add this line
                                episode_url_for_ep_item,
                                is_completed,
                            )
                        }) }
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
