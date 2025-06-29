use super::app_drawer::App_drawer;
use super::gen_components::{
    download_episode_item, empty_message, on_shownotes_click, FallbackImage, Search_nav,
    UseScrollToTop,
};
use crate::components::audio::_AudioPlayerProps::is_youtube;
use crate::components::audio::on_play_pause_offline;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::pod_req::{
    call_remove_downloaded_episode, DownloadEpisodeRequest, EpisodeDownload,
    EpisodeDownloadResponse, EpisodeInfo, Podcast, PodcastDetails, PodcastResponse,
};
use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::window;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

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
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;

    let tauri = match js_sys::Reflect::has(&window, &JsValue::from_str("__TAURI__"))? {
        true => js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))?,
        false => return Ok(()),
    };

    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))?;
    let invoke = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))?;
    let invoke_fn = invoke
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("invoke is not a function"))?;

    let args = js_sys::Object::new();
    js_sys::Reflect::set(&args, &JsValue::from_str("url"), &JsValue::from_str(&url))?;
    js_sys::Reflect::set(
        &args,
        &JsValue::from_str("filename"),
        &JsValue::from_str(&filename),
    )?;

    let command = JsValue::from_str("download_file");
    let promise = invoke_fn.call2(&core, &command, &args)?; // Note: changed tauri to core here
    wasm_bindgen_futures::JsFuture::from(promise.dyn_into::<js_sys::Promise>()?).await?;

    Ok(())
}

pub async fn start_local_file_server(file_path: &str) -> Result<String, JsValue> {
    // Get window object
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;

    // Check if __TAURI__ exists
    let tauri = match js_sys::Reflect::has(&window, &JsValue::from_str("__TAURI__"))? {
        true => js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))?,
        false => return Ok(String::new()), // Return empty string if Tauri isn't available
    };

    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))?;
    let invoke = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))?;
    let invoke_fn = invoke
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("invoke is not a function"))?;

    // Create arguments object
    let args = js_sys::Object::new();
    js_sys::Reflect::set(
        &args,
        &JsValue::from_str("filepath"),
        &JsValue::from_str(file_path),
    )?;

    // Make the call
    let command = JsValue::from_str("start_file_server");
    let promise = invoke_fn.call2(&core, &command, &args)?;
    let result =
        wasm_bindgen_futures::JsFuture::from(promise.dyn_into::<js_sys::Promise>()?).await?;

    // Convert result to String
    match result.as_string() {
        Some(url) => Ok(url),
        None => Ok(String::new()),
    }
}

pub async fn update_local_database(episode_info: EpisodeInfo) -> Result<(), JsValue> {
    // Get window object
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;

    // Debug: Print the episode info before serialization
    web_sys::console::log_1(
        &format!("Episode info before serialization: {:?}", episode_info).into(),
    );

    // Check if TAURI exists
    let tauri = match js_sys::Reflect::has(&window, &JsValue::from_str("__TAURI__"))? {
        true => js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))?,
        false => return Ok(()),
    };

    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))?;
    let invoke = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))?;
    let invoke_fn = invoke
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("invoke is not a function"))?;

    // Create arguments object with episodeInfo field
    let args = js_sys::Object::new();
    let episode_info_value = serde_wasm_bindgen::to_value(&episode_info)?;

    // Debug: Print the serialized value
    web_sys::console::log_1(&format!("Serialized value: {:?}", episode_info_value).into());

    js_sys::Reflect::set(
        &args,
        &JsValue::from_str("episodeInfo"),
        &episode_info_value,
    )?;

    let command = JsValue::from_str("update_local_db");
    let promise = invoke_fn.call2(&core, &command, &args)?;
    wasm_bindgen_futures::JsFuture::from(promise.dyn_into::<js_sys::Promise>()?).await?;

    Ok(())
}

pub async fn remove_episode_from_local_db(episode_id: i32) -> Result<(), JsValue> {
    // Get window object
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;

    // Check if __TAURI__ exists
    let tauri = match js_sys::Reflect::has(&window, &JsValue::from_str("__TAURI__"))? {
        true => js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))?,
        false => return Ok(()), // Return early if Tauri isn't available
    };

    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))?;
    let invoke = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))?;
    let invoke_fn = invoke
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("invoke is not a function"))?;

    // Create arguments object
    let args = js_sys::Object::new();
    js_sys::Reflect::set(
        &args,
        &JsValue::from_str("episodeid"),
        &JsValue::from_f64(episode_id as f64),
    )?;

    // Make the call
    let command = JsValue::from_str("remove_from_local_db");
    let promise = invoke_fn.call2(&core, &command, &args)?;
    wasm_bindgen_futures::JsFuture::from(promise.dyn_into::<js_sys::Promise>()?).await?;

    Ok(())
}

pub async fn fetch_local_episodes() -> Result<Vec<EpisodeDownload>, JsValue> {
    // Get window object
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;

    // Check if __TAURI__ exists
    let tauri = match js_sys::Reflect::has(&window, &JsValue::from_str("__TAURI__"))? {
        true => js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))?,
        false => return Ok(Vec::new()), // Return empty vector if __TAURI__ doesn't exist
    };

    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))?;
    let invoke = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))?;
    let invoke_fn = invoke
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("invoke is not a function"))?;

    // Create arguments
    let command = JsValue::from_str("get_local_episodes");
    let args = js_sys::Object::new();

    // Make the call
    let promise = invoke_fn.call2(&core, &command, &args)?;
    let result =
        wasm_bindgen_futures::JsFuture::from(promise.dyn_into::<js_sys::Promise>()?).await?;

    match serde_wasm_bindgen::from_value::<Vec<EpisodeDownload>>(result) {
        Ok(episodes) => Ok(episodes),
        Err(_) => Ok(Vec::new()),
    }
}

pub async fn update_podcast_database(podcast_details: PodcastDetails) -> Result<(), JsValue> {
    // Get window object
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;

    // Check if __TAURI__ exists
    let tauri = match js_sys::Reflect::has(&window, &JsValue::from_str("__TAURI__"))? {
        true => js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))?,
        false => return Ok(()), // Return early if Tauri isn't available
    };

    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))?;
    let invoke = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))?;
    let invoke_fn = invoke
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("invoke is not a function"))?;

    // Create arguments object with podcastDetails field
    let args = js_sys::Object::new();
    let podcast_details_value = serde_wasm_bindgen::to_value(&podcast_details)?;
    js_sys::Reflect::set(
        &args,
        &JsValue::from_str("podcastDetails"),
        &podcast_details_value,
    )?;

    // Make the call
    let command = JsValue::from_str("update_podcast_db");
    let promise = invoke_fn.call2(&core, &command, &args)?;
    wasm_bindgen_futures::JsFuture::from(promise.dyn_into::<js_sys::Promise>()?).await?;

    Ok(())
}

pub async fn fetch_local_podcasts() -> Result<Vec<Podcast>, JsValue> {
    // Get window object
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window object found"))?;

    // Check if __TAURI__ exists
    let tauri = match js_sys::Reflect::has(&window, &JsValue::from_str("__TAURI__"))? {
        true => js_sys::Reflect::get(&window, &JsValue::from_str("__TAURI__"))?,
        false => return Ok(Vec::new()), // Return empty vector if __TAURI__ doesn't exist
    };

    let core = js_sys::Reflect::get(&tauri, &JsValue::from_str("core"))?;
    let invoke = js_sys::Reflect::get(&core, &JsValue::from_str("invoke"))?;
    let invoke_fn = invoke
        .dyn_ref::<js_sys::Function>()
        .ok_or_else(|| JsValue::from_str("invoke is not a function"))?;

    // Create arguments
    let command = JsValue::from_str("get_local_podcasts");
    let args = js_sys::Object::new();

    // Make the call
    let promise = invoke_fn.call2(&core, &command, &args)?;
    let result =
        wasm_bindgen_futures::JsFuture::from(promise.dyn_into::<js_sys::Promise>()?).await?;

    match serde_wasm_bindgen::from_value::<Vec<Podcast>>(result) {
        Ok(podcasts) => Ok(podcasts),
        Err(_) => Ok(Vec::new()),
    }
}

// Define the arguments for the Tauri command
#[derive(Serialize, Deserialize)]
struct ListDirArgs<'a> {
    path: &'a str,
}

// Define the structure for the file entries
#[derive(Deserialize)]
struct FileEntry {
    #[allow(dead_code)]
    path: String,
}

#[function_component(Downloads)]
pub fn downloads() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (ui_state, ui_dispatch) = use_store::<UIState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let effect_dispatch = dispatch.clone();
    let history = BrowserHistory::new();
    let session_dispatch = effect_dispatch.clone();
    let session_state = state.clone();
    let expanded_state = use_state(HashMap::new);
    let show_modal = use_state(|| false);
    let show_clonedal = show_modal.clone();
    let show_clonedal2 = show_modal.clone();
    let on_modal_open = Callback::from(move |_: MouseEvent| show_clonedal.set(true));

    let on_modal_close = Callback::from(move |_: MouseEvent| show_clonedal2.set(false));

    let error = use_state(|| None::<String>);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
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

    // Filter state for episodes
    let episode_search_term = use_state(|| String::new());
    let show_completed = use_state(|| false);
    let show_in_progress = use_state(|| false);

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
                // First ensure we have a valid podcast feed state, even if empty
                dispatch.reduce_mut(move |state| {
                    state.podcast_feed_return = Some(PodcastResponse {
                        pods: Some(Vec::new()),
                    });
                });

                // Then try to fetch and update
                match fetch_local_podcasts().await {
                    Ok(fetched_podcasts) => {
                        web_sys::console::log_1(
                            &format!("Fetched podcasts: {:?}", fetched_podcasts).into(),
                        );
                        dispatch.reduce_mut(move |state| {
                            state.podcast_feed_return = Some(PodcastResponse {
                                pods: Some(fetched_podcasts),
                            });
                        });
                    }
                    Err(e) => {
                        web_sys::console::log_1(
                            &format!("Failed to fetch podcasts: {:?}", e).into(),
                        );
                    }
                }
                // Similar pattern for episodes
                if let Ok(fetched_episodes) = fetch_local_episodes().await {
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
                } else {
                    web_sys::console::log_1(&"Critical CATDOG mistake".into());
                }

                loading_ep.set(false);
            });

            || ()
        });
    }

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
        let user_id = user_id.clone();

        Callback::from(move |_: MouseEvent| {
            let dispatch_cloned = dispatch.clone();
            let page_state_cloned = page_state.clone();
            let server_name_cloned = server_name.clone().unwrap();
            let api_key_cloned = api_key.clone().unwrap();
            let user_id_cloned = user_id.unwrap();

            dispatch.reduce_mut(move |state| {
                let selected_episodes = state.selected_episodes_for_deletion.clone();
                state.selected_episodes_for_deletion.clear();

                if let Some(downloaded_eps) = &state.downloaded_episodes {
                    for &episode_id in &selected_episodes {
                        // Find the episode to get its is_youtube value
                        if let Some(episode) = downloaded_eps
                            .episodes
                            .iter()
                            .find(|ep| ep.episodeid == episode_id)
                        {
                            let request = DownloadEpisodeRequest {
                                episode_id,
                                user_id: user_id_cloned,
                                is_youtube: episode.is_youtube, // Use the actual is_youtube value from the episode
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
                                        if let Some(downloaded_episodes) =
                                            &mut state.downloaded_episodes
                                        {
                                            downloaded_episodes
                                                .episodes
                                                .retain(|ep| ep.episodeid != episode_id);
                                        }
                                        state.info_message = Some(success_message);
                                    });
                                }
                            });
                        }
                    }
                }
                page_state_cloned.set(PageState::Normal);
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

    let online_button = {
        let dispatch = ui_dispatch.clone();

        Callback::from(move |_| {
            dispatch.reduce_mut(|state| state.app_offline_mode = Some(false));
            history.push("/");
        })
    };
    let online_mode_banner = if app_offline_mode.unwrap_or(false) {
        html! {
            <div class="w-full p-4 mb-4">
                <button
                    onclick={online_button}
                    class="download-button font-bold py-2 px-4 rounded inline-flex items-center w-full justify-center"
                >
                    <i class="ph ph-cloud text-2xl"></i>
                    <span>{"Switch to Online Mode (Sign In Required)"}</span>
                </button>
            </div>
        }
    } else {
        html! {}
    };

    web_sys::console::log_1(
        &format!(
            "Podcast feed count: {:?}",
            state
                .podcast_feed_return
                .as_ref()
                .and_then(|pf| pf.pods.as_ref().map(|pods| pods.len()))
                .unwrap_or(0)
        )
        .into(),
    );
    web_sys::console::log_1(
        &format!(
            "Downloaded episodes count: {:?}",
            state
                .downloaded_episodes
                .as_ref()
                .map(|de| de.episodes.len())
                .unwrap_or(0)
        )
        .into(),
    );
    if let Some(download_eps) = state.downloaded_episodes.clone() {
        let grouped = group_episodes_by_podcast(download_eps.episodes.clone());
        web_sys::console::log_1(&format!("Grouped podcast count: {:?}", grouped.len()).into());
        web_sys::console::log_1(
            &format!(
                "Grouped podcast IDs: {:?}",
                grouped.keys().collect::<Vec<_>>()
            )
            .into(),
        );

        if let Some(podcast_feed) = state.podcast_feed_return.as_ref() {
            if let Some(pods) = podcast_feed.pods.as_ref() {
                web_sys::console::log_1(
                    &format!(
                        "Podcast IDs in feed: {:?}",
                        pods.iter().map(|p| p.podcastid).collect::<Vec<_>>()
                    )
                    .into(),
                );

                // Check matches
                for podcast in pods.iter() {
                    let has_episodes = grouped.get(&podcast.podcastid).is_some();
                    web_sys::console::log_1(
                        &format!(
                            "Podcast ID {} has episodes: {}",
                            podcast.podcastid, has_episodes
                        )
                        .into(),
                    );
                }
            }
        }
    }

    html! {
        <>
        <div class="main-container">
            {search_options}
            <UseScrollToTop />
            {online_mode_banner}
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
                                                    <i class="ph ph-lasso text-2xl"></i>
                                                    <span class="text-lg ml-2">{"Select Multiple"}</span>
                                                </button>
                                            }
                                        } else {
                                            html! {
                                                <>
                                                <button class="download-button font-bold py-2 px-4 rounded inline-flex items-center"
                                                    onclick={delete_mode_disable.clone()}>
                                                    <i class="ph ph-prohibit text-2xl"></i>
                                                    <span class="text-lg ml-2">{"Cancel"}</span>
                                                </button>
                                                <button class="download-button font-bold py-2 px-4 rounded inline-flex items-center"
                                                    onclick={delete_selected_episodes.clone()}>
                                                    <i class="ph ph-trash text-2xl"></i>
                                                    <span class="text-lg ml-2">{"Delete"}</span>
                                                </button>
                                                </>
                                            }
                                        }
                                    }
                                </div>

                                // Modern mobile-friendly filter bar
                                <div class="mb-6 space-y-4">
                                    // Search bar (full width - no sort dropdown for downloads)
                                    <div class="w-full">
                                        <div class="relative">
                                            <input
                                                type="text"
                                                class="w-full h-12 pl-4 pr-12 text-base rounded-xl border-2 border-color bg-background-color text-text-color placeholder-text-color-muted focus:outline-none focus:border-accent-color transition-colors"
                                                placeholder="Search downloaded episodes..."
                                                value={(*episode_search_term).clone()}
                                                oninput={let episode_search_term = episode_search_term.clone();
                                                    Callback::from(move |e: InputEvent| {
                                                        if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                            episode_search_term.set(input.value());
                                                        }
                                                    })
                                                }
                                            />
                                            <i class="ph ph-magnifying-glass absolute right-4 top-1/2 -translate-y-1/2 text-xl text-text-color-muted pointer-events-none"></i>
                                        </div>
                                    </div>

                                    // Filter chips (horizontal scroll on mobile)
                                    <div class="flex gap-3 overflow-x-auto pb-2 md:pb-0 scrollbar-hide">
                                        // Clear all filters
                                        <button
                                            onclick={
                                                let show_completed = show_completed.clone();
                                                let show_in_progress = show_in_progress.clone();
                                                let episode_search_term = episode_search_term.clone();
                                                Callback::from(move |_| {
                                                    show_completed.set(false);
                                                    show_in_progress.set(false);
                                                    episode_search_term.set(String::new());
                                                })
                                            }
                                            class="filter-chip flex items-center gap-2 px-4 py-2 rounded-full border-2 border-color bg-background-color text-text-color hover:bg-accent-color hover:text-white transition-all duration-200 whitespace-nowrap min-h-[44px]"
                                        >
                                            <i class="ph ph-broom text-lg"></i>
                                            <span class="text-sm font-medium">{"Clear All"}</span>
                                        </button>

                                        // Completed filter chip
                                        <button
                                            onclick={let show_completed = show_completed.clone();
                                                let show_in_progress = show_in_progress.clone();
                                                Callback::from(move |_| {
                                                    show_completed.set(!*show_completed);
                                                    if *show_in_progress {
                                                        show_in_progress.set(false);
                                                    }
                                                })
                                            }
                                            class={classes!(
                                                "filter-chip", "flex", "items-center", "gap-2", "px-4", "py-2",
                                                "rounded-full", "border-2", "transition-all", "duration-200",
                                                "whitespace-nowrap", "min-h-[44px]",
                                                if *show_completed {
                                                    "bg-accent-color text-white border-accent-color"
                                                } else {
                                                    "border-color bg-background-color text-text-color hover:bg-accent-color hover:text-white"
                                                }
                                            )}
                                        >
                                            <i class="ph ph-check-circle text-lg"></i>
                                            <span class="text-sm font-medium">{"Completed"}</span>
                                        </button>

                                        // In progress filter chip
                                        <button
                                            onclick={let show_in_progress = show_in_progress.clone();
                                                let show_completed = show_completed.clone();
                                                Callback::from(move |_| {
                                                    show_in_progress.set(!*show_in_progress);
                                                    if *show_completed {
                                                        show_completed.set(false);
                                                    }
                                                })
                                            }
                                            class={classes!(
                                                "filter-chip", "flex", "items-center", "gap-2", "px-4", "py-2",
                                                "rounded-full", "border-2", "transition-all", "duration-200",
                                                "whitespace-nowrap", "min-h-[44px]",
                                                if *show_in_progress {
                                                    "bg-accent-color text-white border-accent-color"
                                                } else {
                                                    "border-color bg-background-color text-text-color hover:bg-accent-color hover:text-white"
                                                }
                                            )}
                                        >
                                            <i class="ph ph-hourglass-medium text-lg"></i>
                                            <span class="text-sm font-medium">{"In Progress"}</span>
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }
                    }

                    {
                    if let Some(download_eps) = state.downloaded_episodes.clone() {
                        let int_download_eps = download_eps.clone();
                            let render_state = post_state.clone();
                            let dispatch_cloned = dispatch.clone();

                            if int_download_eps.episodes.is_empty() {
                                // Render "No Recent Episodes Found" if episodes list is empty
                                empty_message(
                                    "No Downloaded Episodes Found",
                                    "This is where local episode downloads will appear. To download an episode you can open the context menu on an episode and select Local Download. It will then download to your device locally and show up here!"
                                )
                            } else {
                                let grouped_episodes = group_episodes_by_podcast(int_download_eps.episodes);

                                // Create filtered episodes
                                let filtered_grouped_episodes = {
                                    let mut filtered_map: HashMap<i32, Vec<EpisodeDownload>> = HashMap::new();

                                    for (podcast_id, episodes) in grouped_episodes.iter() {
                                        let filtered_episodes: Vec<EpisodeDownload> = episodes.iter()
                                            .filter(|episode| {
                                                // Search filter
                                                let matches_search = if !episode_search_term.is_empty() {
                                                    episode.episodetitle.to_lowercase().contains(&episode_search_term.to_lowercase())
                                                } else {
                                                    true
                                                };

                                                // Completion filter
                                                let matches_completion = if *show_completed && *show_in_progress {
                                                    true // Both filters active = show all
                                                } else if *show_completed {
                                                    episode.completed
                                                } else if *show_in_progress {
                                                    !episode.completed && episode.listenduration.is_some() && episode.listenduration.unwrap() > 0
                                                } else {
                                                    true // No filters = show all
                                                };

                                                matches_search && matches_completion
                                            })
                                            .cloned()
                                            .collect();

                                        if !filtered_episodes.is_empty() {
                                            filtered_map.insert(*podcast_id, filtered_episodes);
                                        }
                                    }

                                    filtered_map
                                };

                                html! {
                                    <>
                                    {
                                        if let Some(podcast_feed) = state.podcast_feed_return.as_ref() {
                                            if let Some(pods) = podcast_feed.pods.as_ref() {
                                                html! {
                                                    <>
                                                        { for pods.iter().filter_map(|podcast| {
                                                            let episodes = filtered_grouped_episodes.get(&podcast.podcastid).unwrap_or(&Vec::new()).clone();
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
                                                                let audio_dispatch_cloned = audio_dispatch.clone();
                                                                let audio_state_cloned = audio_state.clone();
                                                                let on_checkbox_change_cloned = on_checkbox_change.clone();

                                                                Some(render_podcast_with_episodes(
                                                                    podcast,
                                                                    episodes,
                                                                    is_expanded,
                                                                    toggle_expanded_closure,
                                                                    render_state_cloned,
                                                                    dispatch_cloned_cloned,
                                                                    is_delete_mode,
                                                                    desc_state.clone(),
                                                                    desc_dispatch.clone(),
                                                                    audio_dispatch_cloned,
                                                                    audio_state_cloned,
                                                                    on_checkbox_change_cloned,
                                                                    *show_modal,
                                                                    on_modal_open.clone(),
                                                                    on_modal_close.clone(),
                                                                ))
                                                            }
                                                        }) }
                                                    </>
                                                }
                                            } else {
                                                empty_message(
                                                    "No Downloaded Episodes Found",
                                                    "This is where episode downloads will appear. To download an episode you can open the context menu on an episode and select Download Episode."
                                                )
                                            }
                                        } else {
                                            empty_message(
                                                "No Downloaded Episodes Found",
                                                "This is where episode downloads will appear. To download an episode you can open the context menu on an episode and select Download Episode."
                                            )
                                        }
                                    }
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
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
            } else {
                html! {}
            }
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
    desc_rc: Rc<ExpandedDescriptions>,
    desc_state: Dispatch<ExpandedDescriptions>,
    audio_dispatch: Dispatch<UIState>,
    audio_state: Rc<UIState>,
    on_checkbox_change: Callback<i32>,
    show_modal: bool,
    on_modal_open: Callback<MouseEvent>,
    on_modal_close: Callback<MouseEvent>,
) -> Html {
    let history_clone = BrowserHistory::new();

    let on_podcast_checkbox_change = {
        let episodes = episodes.clone();
        let on_checkbox_change = on_checkbox_change.clone();
        let dispatch_clone = dispatch.clone();
        let episode_ids: Vec<i32> = episodes.iter().map(|ep| ep.episodeid).collect();

        Callback::from(move |e: Event| {
            let is_checked = e
                .target_dyn_into::<web_sys::HtmlInputElement>()
                .map(|input| input.checked())
                .unwrap_or(false);

            // Access current state during callback execution
            let selected_episodes = &dispatch_clone.get().selected_episodes_for_deletion;

            for episode_id in &episode_ids {
                let is_episode_selected = selected_episodes.contains(episode_id);
                if is_checked && !is_episode_selected {
                    // Select episodes that aren't already selected
                    on_checkbox_change.emit(*episode_id);
                } else if !is_checked && is_episode_selected {
                    // Deselect episodes that are currently selected
                    on_checkbox_change.emit(*episode_id);
                }
            }
        })
    };

    let html_dispatch = dispatch.clone();

    html! {
        <div key={podcast.podcastid}>
            {if is_delete_mode {
                html! {
                    <div class="flex items-center pl-4" onclick={|e: MouseEvent| e.stop_propagation()}>
                        <input
                            type="checkbox"
                            class="h-5 w-5 rounded border-2 border-gray-400 text-primary focus:ring-primary focus:ring-offset-0 cursor-pointer appearance-none checked:bg-primary checked:border-primary relative
                            before:content-[''] before:block before:w-full before:h-full before:checked:bg-[url('data:image/svg+xml;base64,PHN2ZyB2aWV3Qm94PScwIDAgMTYgMTYnIGZpbGw9JyNmZmYnIHhtbG5zPSdodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2Zyc+PHBhdGggZD0nTTEyLjIwNyA0Ljc5M2ExIDEgMCAwIDEgMCAxLjQxNGwtNSA1YTEgMSAwIDAgMS0xLjQxNCAwbC0yLTJhMSAxIDAgMCAxIDEuNDE0LTEuNDE0TDYuNSA5LjA4NmwzLjc5My0zLjc5M2ExIDEgMCAwIDEgMS40MTQgMHonLz48L3N2Zz4=')] before:checked:bg-no-repeat before:checked:bg-center"
                            onchange={on_podcast_checkbox_change}
                        />
                    </div>
                }
            } else {
                html! {}
            }}
            <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full" onclick={toggle_expanded}>
                <div class="flex flex-col w-auto object-cover pl-4">
                    <FallbackImage
                        src={podcast.artworkurl.clone().unwrap()}
                        // onclick={on_title_click.clone()}
                        alt={format!("Cover for {}", podcast.podcastname.clone())}
                        class="object-cover align-top-cover w-full item-container img"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <p class="item_container-text text-xl font-semibold cursor-pointer">
                        { &podcast.podcastname }
                    </p>
                    <hr class="my-2 border-t hidden md:block"/>
                    <p class="item_container-text">{ format!("Episode Count: {}", podcast.episodecount.unwrap_or(0)) }</p>
                </div>
            </div>
            { if is_expanded {
                html! {
                    <div class="episodes-dropdown, pl-4">
                        { for episodes.into_iter().map(|episode| {
                            let id_string = &episode.episodeid.to_string();

                            let app_dispatch = html_dispatch.clone();

                            let episode_url_clone = episode.episodeurl.clone();
                            let episode_duration_clone = episode.episodeduration.clone();
                            let episode_id_clone = episode.episodeid.clone();
                            let episode_listened_clone = episode.listenduration.clone();
                            let episode_is_youtube = Some(episode.is_youtube.clone());
                            let desc_expanded = desc_rc.expanded_descriptions.contains(id_string);

                            #[wasm_bindgen]
                            extern "C" {
                                #[wasm_bindgen(js_namespace = window)]
                                fn toggleDescription(guid: &str, expanded: bool);
                            }
                            let toggle_expanded = {
                                let desc_dispatch = desc_state.clone();
                                let episode_guid = episode.episodeid.clone().to_string();

                                Callback::from(move |_: MouseEvent| {
                                    let guid = episode_guid.clone();
                                    desc_dispatch.reduce_mut(move |state| {
                                        if state.expanded_descriptions.contains(&guid) {
                                            state.expanded_descriptions.remove(&guid); // Collapse the description
                                            toggleDescription(&guid, false); // Call JavaScript function
                                        } else {
                                            state.expanded_descriptions.insert(guid.clone()); // Expand the description
                                            toggleDescription(&guid, true); // Call JavaScript function
                                        }
                                    });
                                })
                            };

                            let episode_id_for_closure = episode_id_clone.clone();
                            let audio_dispatch = audio_dispatch.clone();
                            let audio_state = audio_state.clone();

                            let is_current_episode = audio_state
                                                            .currently_playing
                                                            .as_ref()
                                                            .map_or(false, |current| current.episode_id == episode.episodeid);
                            let is_playing = audio_state.audio_playing.unwrap_or(false);

                            let date_format = match_date_format(state.date_format.as_deref());
                            let datetime = parse_date(&episode.episodepubdate, &state.user_tz);
                            let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));


                            let on_play_pause = on_play_pause_offline(episode.clone(), audio_dispatch, audio_state, app_dispatch.clone());

                            let on_shownotes_click = on_shownotes_click(
                                history_clone.clone(),
                                app_dispatch.clone(),
                                Some(episode_id_for_closure.clone()),
                                Some(String::from("Not needed")),
                                Some(String::from("Not needed")),
                                Some(String::from("Not needed")),
                                true,
                                None,
                                episode_is_youtube,
                            );

                            let on_checkbox_change_cloned = on_checkbox_change.clone();
                            let episode_url_for_ep_item = episode_url_clone.clone();
                            let sanitized_description =
                                sanitize_html_with_blank_target(&episode.episodedescription.clone());

                            let check_episode_id = &episode.episodeid.clone();
                            let is_completed = state
                                .completed_episodes
                                .as_ref()
                                .unwrap_or(&vec![])
                                .contains(&check_episode_id);
                            download_episode_item(
                                Box::new(episode),
                                sanitized_description.clone(),
                                desc_expanded,
                                &format_release,
                                on_play_pause,
                                on_shownotes_click,
                                toggle_expanded,
                                episode_duration_clone,
                                episode_listened_clone,
                                "local_downloads",
                                on_checkbox_change_cloned, // Add this line
                                is_delete_mode, // Add this line
                                episode_url_for_ep_item,
                                is_completed,
                                show_modal,
                                on_modal_open.clone(),
                                on_modal_close.clone(),
                                is_current_episode,
                                is_playing,
                                state.clone()
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
