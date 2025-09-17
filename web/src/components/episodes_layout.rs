use super::app_drawer::App_drawer;
use super::gen_components::{FallbackImage, Search_nav, UseScrollToTop};
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, UIState};
use crate::components::gen_funcs::{
    format_error_message, get_filter_preference, set_filter_preference, get_default_sort_direction,
};
use crate::components::host_component::HostDropdown;
use crate::components::podcast_layout::ClickedFeedURL;
use crate::components::virtual_list::PodcastEpisodeVirtualList;
use crate::requests::pod_req::{
    call_add_category, call_add_podcast, call_adjust_skip_times, call_bulk_download_episodes,
    call_bulk_mark_episodes_completed, call_bulk_queue_episodes, call_bulk_save_episodes,
    call_check_podcast, call_clear_playback_speed, call_download_all_podcast,
    call_enable_auto_download, call_fetch_podcasting_2_pod_data, call_get_auto_download_status,
    call_get_feed_cutoff_days, call_get_play_episode_details, call_get_podcast_id_from_ep,
    call_get_podcast_id_from_ep_name, call_get_podcast_notifications_status, call_get_rss_key,
    call_remove_category, call_remove_podcasts_name, call_remove_youtube_channel,
    call_set_playback_speed, call_toggle_podcast_notifications, call_update_feed_cutoff_days,
    AddCategoryRequest, AutoDownloadRequest, BulkEpisodeActionRequest,
    ClearPlaybackSpeedRequest, DownloadAllPodcastRequest, FetchPodcasting2PodDataRequest,
    PlaybackSpeedRequest, PodcastValues, RemoveCategoryRequest, RemovePodcastValuesName,
    RemoveYouTubeChannelValues, SkipTimesRequest, UpdateFeedCutoffDaysRequest,
};
use crate::requests::search_pods::call_get_podcast_details_dynamic;
use crate::requests::search_pods::call_get_podcast_episodes;
use htmlentity::entity::decode;
use htmlentity::entity::ICodedDataTrait;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::Element;
use web_sys::{window, Event, HtmlInputElement, MouseEvent, UrlSearchParams};
use yew::prelude::*;
use yew::Properties;
use yew::{function_component, html, use_effect_with, use_node_ref, Callback, Html, TargetCast};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

fn add_icon() -> Html {
    html! {
        <i class="ph ph-plus-circle text-2xl"></i>
    }
}

fn payments_icon() -> Html {
    html! {
        <i class="ph ph-money-wavy text-2xl"></i>
    }
}

fn rss_icon() -> Html {
    html! {
        <i class="ph ph-rss text-2xl"></i>
    }
}

fn website_icon() -> Html {
    html! {
        <i class="ph ph-globe text-2xl"></i>
    }
}

fn trash_icon() -> Html {
    html! {
        <i class="ph ph-trash text-2xl"></i>

    }
}
fn settings_icon() -> Html {
    html! {
        <i class="ph ph-gear text-2xl"></i>

    }
}
fn download_icon() -> Html {
    html! {
        <i class="ph ph-download text-2xl"></i>

    }
}
fn no_icon() -> Html {
    html! {}
}

#[allow(dead_code)]
fn play_icon() -> Html {
    html! {
    <svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 -960 960 960" width="24"><path d="m380-300 280-180-280-180v360ZM480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Zm0-80q134 0 227-93t93-227q0-134-93-227t-227-93q-134 0-227 93t-93 227q0 134 93 227t227 93Zm0-320Z"/></svg>
        }
}

#[allow(dead_code)]
fn pause_icon() -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 -960 960 960" width="24"><path d="M360-320h80v-320h-80v320Zm160 0h80v-320h-80v320ZM480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Zm0-80q134 0 227-93t93-227q0-134-93-227t-227-93q-134 0-227 93t-93 227q0 134 93 227t227 93Zm0-320Z"/></svg>
    }
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub html: String,
}

fn sanitize_html(html: &str) -> String {
    let cleaned_html = ammonia::clean(html);
    let decoded_data = decode(cleaned_html.as_bytes());
    match decoded_data.to_string() {
        Ok(decoded_html) => decoded_html,
        Err(_) => String::from("Invalid HTML content"),
    }
}

fn get_rss_base_url() -> String {
    let window = window().expect("no global `window` exists");
    let location = window.location();
    let current_url = location
        .href()
        .unwrap_or_else(|_| "Unable to retrieve URL".to_string());

    if let Some(storage) = window.local_storage().ok().flatten() {
        if let Ok(Some(auth_state)) = storage.get_item("userAuthState") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&auth_state) {
                if let Some(server_name) = json
                    .get("auth_details")
                    .and_then(|auth| auth.get("server_name"))
                    .and_then(|name| name.as_str())
                {
                    return format!("{}/rss", server_name);
                }
            }
        }
    }
    // Fallback to using the current URL's origin
    format!(
        "{}/rss",
        current_url.split('/').take(3).collect::<Vec<_>>().join("/")
    )
}

pub enum AppStateMsg {
    ExpandEpisode(String),
    CollapseEpisode(String),
}

impl Reducer<AppState> for AppStateMsg {
    fn apply(self, mut state: Rc<AppState>) -> Rc<AppState> {
        let state_mut = Rc::make_mut(&mut state);

        match self {
            AppStateMsg::ExpandEpisode(guid) => {
                state_mut.expanded_descriptions.insert(guid);
            }
            AppStateMsg::CollapseEpisode(guid) => {
                state_mut.expanded_descriptions.remove(&guid);
            }
        }

        // Return the Rc itself, not a reference to it
        state
    }
}

#[derive(Clone, PartialEq)]
pub enum EpisodeSortDirection {
    NewestFirst,
    OldestFirst,
    ShortestFirst,
    LongestFirst,
    TitleAZ,
    TitleZA,
}

#[function_component(EpisodeLayout)]
pub fn episode_layout() -> Html {
    let is_added = use_state(|| false);
    let (state, _dispatch) = use_store::<UIState>();
    let (search_state, _search_dispatch) = use_store::<AppState>();
    let podcast_feed_results = search_state.podcast_feed_results.clone();
    let clicked_podcast_info = search_state.clicked_podcast_info.clone();
    let loading = use_state(|| true);
    let page_state = use_state(|| PageState::Hidden);
    let episode_search_term = use_state(|| String::new());
    
    // Initialize sort direction from local storage or default to newest first
    let episode_sort_direction = use_state(|| {
        let saved_preference = get_filter_preference("episodes");
        match saved_preference.as_deref() {
            Some("newest") => Some(EpisodeSortDirection::NewestFirst),
            Some("oldest") => Some(EpisodeSortDirection::OldestFirst),
            Some("shortest") => Some(EpisodeSortDirection::ShortestFirst),
            Some("longest") => Some(EpisodeSortDirection::LongestFirst),
            Some("title_az") => Some(EpisodeSortDirection::TitleAZ),
            Some("title_za") => Some(EpisodeSortDirection::TitleZA),
            _ => Some(EpisodeSortDirection::NewestFirst), // Default to newest first
        }
    });
    
    let completed_filter_state = use_state(|| CompletedFilter::ShowAll);
    let show_in_progress = use_state(|| false);
    let notification_status = use_state(|| false);
    let feed_cutoff_days = use_state(|| 0);
    let feed_cutoff_days_input = use_state(|| "0".to_string());
    let playback_speed = use_state(|| 1.0);
    let playback_speed_input = playback_speed.clone();
    let playback_speed_clone = playback_speed.clone();
    let rss_key_state = use_state(|| None::<String>);
    
    // Bulk selection state
    let selected_episodes = use_state(|| HashSet::<i32>::new());
    let is_selecting = use_state(|| false);

    let history = BrowserHistory::new();
    // let node_ref = use_node_ref();
    let user_id = search_state
        .user_details
        .as_ref()
        .map(|ud| ud.UserID.clone());
    let api_key = search_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let server_name = search_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let podcast_added = search_state.podcast_added.unwrap_or_default();
    let pod_url = use_state(|| String::new());
    let new_category = use_state(|| String::new());

    let new_cat_in = new_category.clone();
    let new_category_input = Callback::from(move |e: InputEvent| {
        if let Some(input_element) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
            let value = input_element.value(); // Get the value as a String
            new_cat_in.set(value); // Set the state with the String
        }
    });

    // Add this near the start of the component
    let audio_dispatch = _dispatch.clone();

    // Clear podcast metadata when component mounts
    use_effect_with((), move |_| {
        audio_dispatch.reduce_mut(|state| {
            state.podcast_value4value = None;
            state.podcast_funding = None;
            state.podcast_podroll = None;
            state.podcast_people = None;
        });
        || ()
    });

    {
        let audio_dispatch = _dispatch.clone();

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

    // On mount, check if the podcast is in the database
    let effect_user_id = user_id.clone();
    let effect_api_key = api_key.clone();
    let loading_ep = loading.clone();

    {
        let is_added = is_added.clone();
        let podcast = clicked_podcast_info.clone();
        let user_id = effect_user_id.clone();
        let api_key = effect_api_key.clone();
        let server_name = server_name.clone();
        let click_dispatch = _search_dispatch.clone();
        let click_history = history.clone();
        let pod_load_url = pod_url.clone();
        let pod_loading_ep = loading.clone();

        fn emit_click(callback: Callback<MouseEvent>) {
            callback.emit(MouseEvent::new("click").unwrap());
        }

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |(api_key, user_id, server_name)| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let is_added = is_added.clone();

                    if podcast.is_none() {
                        let window = web_sys::window().expect("no global window exists");
                        let search_params = window.location().search().unwrap();
                        let url_params = UrlSearchParams::new_with_str(&search_params).unwrap();

                        let podcast_title = url_params.get("podcast_title").unwrap_or_default();
                        let podcast_url = url_params.get("podcast_url").unwrap_or_default();
                        let podcast_index_id = 0;
                        if !podcast_title.is_empty() && !podcast_url.is_empty() {
                            let podcast_info = ClickedFeedURL {
                                podcastid: 0,
                                podcastname: podcast_title.clone(),
                                feedurl: podcast_url.clone(),
                                description: String::new(),
                                author: String::new(),
                                artworkurl: String::new(),
                                explicit: false,
                                episodecount: 0,
                                categories: None,
                                websiteurl: String::new(),
                                podcastindexid: podcast_index_id,
                                is_youtube: Some(false),
                            };

                            let api_key = api_key.clone();
                            let user_id = user_id.clone();
                            let server_name = server_name.clone();
                            spawn_local(async move {
                                let added = call_check_podcast(
                                    &server_name,
                                    &api_key.clone().unwrap(),
                                    user_id,
                                    podcast_info.podcastname.as_str(),
                                    podcast_info.feedurl.as_str(),
                                )
                                .await
                                .unwrap_or_default()
                                .exists;
                                is_added.set(added);

                                let podcast_details = call_get_podcast_details_dynamic(
                                    &server_name,
                                    &api_key.clone().unwrap(),
                                    user_id,
                                    podcast_info.podcastname.as_str(),
                                    podcast_info.feedurl.as_str(),
                                    podcast_info.podcastindexid,
                                    added,
                                    Some(false),
                                )
                                .await
                                .unwrap();

                                fn categories_to_string(
                                    categories: Option<HashMap<String, String>>,
                                ) -> Option<String> {
                                    categories.map(|map| {
                                        map.values().cloned().collect::<Vec<String>>().join(", ")
                                    })
                                }
                                let podcast_categories_str =
                                    categories_to_string(podcast_details.details.categories);

                                // Execute the same process as when a podcast is clicked
                                let on_title_click = create_on_title_click(
                                    click_dispatch,
                                    server_name,
                                    Some(Some(api_key.clone().unwrap())),
                                    &click_history,
                                    podcast_details.details.podcastindexid,
                                    podcast_details.details.podcastname,
                                    podcast_details.details.feedurl,
                                    podcast_details.details.description,
                                    podcast_details.details.author,
                                    podcast_details.details.artworkurl,
                                    podcast_details.details.explicit,
                                    podcast_details.details.episodecount,
                                    podcast_categories_str, // assuming no categories in local storage
                                    podcast_details.details.websiteurl,
                                    user_id,
                                    podcast_details.details.is_youtube.unwrap(),
                                );
                                emit_click(on_title_click);
                                let window = web_sys::window().expect("no global window exists");
                                let location = window.location();

                                let mut new_url = location.origin().unwrap();
                                new_url.push_str(&location.pathname().unwrap());
                                new_url.push_str("?podcast_title=");
                                new_url.push_str(&urlencoding::encode(&podcast_info.podcastname));
                                new_url.push_str("&podcast_url=");
                                new_url.push_str(&urlencoding::encode(&podcast_info.feedurl));
                                pod_load_url.set(new_url.clone());
                            });
                        }
                    } else {
                        let podcast = podcast.unwrap();

                        // Update the URL with query parameters
                        let window = web_sys::window().expect("no global window exists");
                        let history = window.history().expect("should have a history");
                        let location = window.location();

                        let mut new_url = location.origin().unwrap();
                        new_url.push_str(&location.pathname().unwrap());
                        new_url.push_str("?podcast_title=");
                        new_url.push_str(&urlencoding::encode(&podcast.podcastname));
                        new_url.push_str("&podcast_url=");
                        new_url.push_str(&urlencoding::encode(&podcast.feedurl));

                        history
                            .replace_state_with_url(
                                &wasm_bindgen::JsValue::NULL,
                                "",
                                Some(&new_url),
                            )
                            .expect("should push state");

                        let api_key = api_key.clone();
                        let user_id = user_id.clone();
                        let server_name = server_name.clone();
                        spawn_local(async move {
                            let added = call_check_podcast(
                                &server_name,
                                &api_key.unwrap(),
                                user_id,
                                podcast.podcastname.as_str(),
                                podcast.feedurl.as_str(),
                            )
                            .await
                            .unwrap_or_default()
                            .exists;
                            is_added.set(added);
                            if *is_added.clone() != true {
                                pod_loading_ep.set(false);
                            }
                        });
                    }
                }
                || ()
            },
        );
    }

    let podcast_info = search_state.clicked_podcast_info.clone();
    let load_link = loading.clone();

    use_effect_with(podcast_info.clone(), {
        let pod_url = pod_url.clone();
        move |podcast_info| {
            if let Some(info) = podcast_info {
                let window = window().expect("no global window exists");
                let history = window.history().expect("should have a history");
                let location = window.location();

                let mut new_url = location.origin().unwrap();
                new_url.push_str(&location.pathname().unwrap());
                new_url.push_str("?podcast_title=");
                new_url.push_str(&urlencoding::encode(&info.podcastname));
                new_url.push_str("&podcast_url=");
                new_url.push_str(&urlencoding::encode(&info.feedurl));
                pod_url.set(new_url.clone());
                load_link.set(false);

                history
                    .replace_state_with_url(&JsValue::NULL, "", Some(&new_url))
                    .expect("should push state");
            }
            || {}
        }
    });

    let download_status = use_state(|| false);
    let podcast_id = use_state(|| 0);
    let start_skip = use_state(|| 0);
    let end_skip = use_state(|| 0);

    {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let download_status = download_status.clone();
        let notification_effect = notification_status.clone();
        // let episode_name = episode_name_pre.clone();
        // let episode_url = episode_url_pre.clone();
        let user_id = search_state.user_details.as_ref().map(|ud| ud.UserID);
        let effect_start_skip = start_skip.clone();
        let effect_end_skip = end_skip.clone();
        let effect_playback_speed = playback_speed.clone();
        let effect_added = is_added.clone();
        let feed_cutoff_days = feed_cutoff_days.clone();
        let feed_cutoff_days_input = feed_cutoff_days_input.clone();
        let audio_dispatch = _dispatch.clone();
        let click_state = search_state.clone();

        use_effect_with(
            (
                click_state.podcast_feed_results.clone(),
                effect_added.clone(),
            ),
            move |_| {
                let episode_name: Option<String> = click_state
                    .podcast_feed_results
                    .as_ref()
                    .and_then(|results| results.episodes.get(0))
                    .and_then(|episode| episode.title.clone());
                let episode_url: Option<String> = click_state
                    .podcast_feed_results
                    .as_ref()
                    .and_then(|results| results.episodes.get(0))
                    .and_then(|episode| episode.enclosure_url.clone());

                let bool_true = *effect_added; // Dereference here

                if !bool_true {
                } else {
                    let api_key = api_key.clone();
                    let server_name = server_name.clone();
                    let podcast_id = podcast_id.clone();
                    let download_status = download_status.clone();
                    let episode_name = episode_name;
                    let episode_url = episode_url;
                    let user_id = user_id.unwrap();

                    if episode_name.is_some() && episode_url.is_some() {
                        wasm_bindgen_futures::spawn_local(async move {
                            if let (Some(api_key), Some(server_name)) =
                                (api_key.as_ref(), server_name.as_ref())
                            {
                                match call_get_podcast_id_from_ep_name(
                                    &server_name,
                                    &api_key,
                                    episode_name.unwrap(),
                                    episode_url.unwrap(),
                                    user_id,
                                )
                                .await
                                {
                                    Ok(id) => {
                                        podcast_id.set(id);

                                        match call_get_auto_download_status(
                                            &server_name,
                                            user_id,
                                            &Some(api_key.clone().unwrap()),
                                            id,
                                        )
                                        .await
                                        {
                                            Ok(status) => {
                                                download_status.set(status);
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(
                                                    &format!(
                                                        "Error getting auto-download status: {}",
                                                        e
                                                    )
                                                    .into(),
                                                );
                                            }
                                        }
                                        match call_get_feed_cutoff_days(
                                            &server_name,
                                            &Some(api_key.clone().unwrap()),
                                            id,
                                            user_id,
                                        )
                                        .await
                                        {
                                            Ok(days) => {
                                                feed_cutoff_days.set(days);
                                                feed_cutoff_days_input.set(days.to_string());
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(
                                                    &format!(
                                                        "Error getting feed cutoff days: {}",
                                                        e
                                                    )
                                                    .into(),
                                                );
                                            }
                                        }
                                        // Add notification status check here
                                        match call_get_podcast_notifications_status(
                                            server_name.clone(),
                                            api_key.clone().unwrap(),
                                            user_id,
                                            id,
                                        )
                                        .await
                                        {
                                            Ok(status) => {
                                                notification_effect.set(status);
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(
                                                    &format!(
                                                        "Error getting notification status: {}",
                                                        e
                                                    )
                                                    .into(),
                                                );
                                            }
                                        }
                                        match call_get_play_episode_details(
                                            &server_name,
                                            &Some(api_key.clone().unwrap()),
                                            user_id,
                                            id,    // podcast_id
                                            false, // is_youtube (probably false for most podcasts, adjust if needed)
                                        )
                                        .await
                                        {
                                            Ok((speed, start, end)) => {
                                                effect_start_skip.set(start);
                                                effect_end_skip.set(end);
                                                effect_playback_speed.set(speed as f64);
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(
                                                    &format!(
                                                        "Error getting auto-skip times: {}",
                                                        e
                                                    )
                                                    .into(),
                                                );
                                            }
                                        }
                                        loading_ep.set(false);
                                        let chap_request = FetchPodcasting2PodDataRequest {
                                            podcast_id: id,
                                            user_id,
                                        };
                                        match call_fetch_podcasting_2_pod_data(
                                            &server_name,
                                            &api_key,
                                            &chap_request,
                                        )
                                        .await
                                        {
                                            Ok(response) => {
                                                // let chapters = response.chapters.clone(); // Clone chapters to avoid move issue
                                                let value = response.value.clone();
                                                let funding = response.funding.clone();
                                                let podroll = response.podroll.clone();
                                                let people = response.people.clone();
                                                audio_dispatch.reduce_mut(|state| {
                                                    state.podcast_value4value = Some(value);
                                                    state.podcast_funding = Some(funding);
                                                    state.podcast_podroll = Some(podroll);
                                                    state.podcast_people = Some(people);
                                                });
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(
                                                    &format!("Error fetching 2.0 data: {}", e)
                                                        .into(),
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(
                                            &format!("Error getting podcast ID: {}", e).into(),
                                        );
                                    }
                                }
                            }
                        });
                    }
                }
                || ()
            },
        );
    }

    let open_in_new_tab = Callback::from(move |url: String| {
        let window = web_sys::window().unwrap();
        window.open_with_url_and_target(&url, "_blank").unwrap();
    });

    // Function to handle link clicks
    let history_handle = history.clone();
    let handle_click = Callback::from(move |event: MouseEvent| {
        if let Some(target) = event.target_dyn_into::<web_sys::HtmlElement>() {
            if let Some(href) = target.get_attribute("href") {
                event.prevent_default();
                if href.starts_with("http") {
                    // External link, open in a new tab
                    web_sys::window()
                        .unwrap()
                        .open_with_url_and_target(&href, "_blank")
                        .unwrap();
                } else {
                    // Internal link, use Yew Router to navigate
                    history_handle.push(href);
                }
            }
        }
    });

    let node_ref = use_node_ref();

    use_effect_with((), move |_| {
        if let Some(container) = node_ref.cast::<web_sys::HtmlElement>() {
            if let Ok(links) = container.query_selector_all("a") {
                for i in 0..links.length() {
                    if let Some(link) = links.item(i) {
                        let link = link.dyn_into::<web_sys::HtmlElement>().unwrap();
                        let handle_click_clone = handle_click.clone();
                        let listener =
                            gloo_events::EventListener::new(&link, "click", move |event| {
                                handle_click_clone
                                    .emit(event.clone().dyn_into::<web_sys::MouseEvent>().unwrap());
                            });
                        listener.forget(); // Prevent listener from being dropped
                    }
                }
            }
        }

        || ()
    });

    let delete_history = history.clone();
    let delete_all_click = {
        let add_dispatch = _search_dispatch.clone();
        let pod_values = clicked_podcast_info.clone();

        let user_id_og = user_id.clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let app_dispatch = _search_dispatch.clone();
        let call_is_added = is_added.clone();
        let page_state = page_state.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let hist = delete_history.clone();
            let page_state = page_state.clone();
            let pod_title_og = pod_values.clone().unwrap().podcastname.clone();
            let pod_feed_url_og = pod_values.clone().unwrap().feedurl.clone();
            let is_youtube = pod_values.clone().unwrap().is_youtube.unwrap_or(false);
            app_dispatch.reduce_mut(|state| state.is_loading = Some(true));
            let is_added_inner = call_is_added.clone();
            let call_dispatch = add_dispatch.clone();
            let pod_title = pod_title_og.clone();
            let pod_title_yt = pod_title_og.clone();
            let pod_feed_url = pod_feed_url_og.clone();
            let pod_feed_url_yt = pod_feed_url_og.clone();
            let pod_feed_url_check = pod_feed_url_og.clone();
            let user_id = user_id_og.clone().unwrap();
            let podcast_values = RemovePodcastValuesName {
                podcast_name: pod_title,
                podcast_url: pod_feed_url,
                user_id,
            };
            let remove_channel = RemoveYouTubeChannelValues {
                user_id,
                channel_name: pod_title_yt,
                channel_url: pod_feed_url_yt,
            };
            let api_key_call = api_key_clone.clone();
            let server_name_call = server_name_clone.clone();
            let app_dispatch = app_dispatch.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let dispatch_wasm = call_dispatch.clone();
                let api_key_wasm = api_key_call.clone().unwrap();
                let server_name_wasm = server_name_call.clone();

                let result = if pod_feed_url_check.starts_with("https://www.youtube.com") {
                    call_remove_youtube_channel(
                        &server_name_wasm.unwrap(),
                        &api_key_wasm,
                        &remove_channel,
                    )
                    .await
                } else {
                    call_remove_podcasts_name(
                        &server_name_wasm.unwrap(),
                        &api_key_wasm,
                        &podcast_values,
                    )
                    .await
                };

                match result {
                    Ok(success) => {
                        if success {
                            dispatch_wasm.reduce_mut(|state| {
                                state.info_message = Some(
                                    if pod_feed_url_check.starts_with("https://www.youtube.com") {
                                        "YouTube channel successfully removed".to_string()
                                    } else {
                                        "Podcast successfully removed".to_string()
                                    },
                                )
                            });
                            app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                            is_added_inner.set(false);
                            app_dispatch.reduce_mut(|state| {
                                state.podcast_added = Some(podcast_added);
                            });

                            if pod_feed_url_check.starts_with("https://www.youtube.com") {
                                hist.push("/podcasts");
                            }
                        } else {
                            dispatch_wasm.reduce_mut(|state| {
                                state.error_message = Some(if is_youtube {
                                    "Failed to remove YouTube channel".to_string()
                                } else {
                                    "Failed to remove podcast".to_string()
                                })
                            });
                            app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                        }
                        page_state.set(PageState::Hidden);
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        dispatch_wasm.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Error removing content: {:?}", formatted_error))
                        });
                        app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                    }
                }
            });
        })
    };

    let download_server_name = server_name.clone();
    let download_api_key = api_key.clone();
    let download_dispatch = _search_dispatch.clone();
    let app_state = search_state.clone();

    let download_all_click = {
        let call_dispatch = download_dispatch.clone();
        let server_name_copy = download_server_name.clone();
        let api_key_copy = download_api_key.clone();
        let user_id_copy = user_id.clone();
        let search_call_state = app_state.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let server_name = server_name_copy.clone();
            let api_key = api_key_copy.clone();
            let search_state = search_call_state.clone();
            let call_down_dispatch = call_dispatch.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let episode_id = match search_state
                    .podcast_feed_results
                    .as_ref()
                    .and_then(|results| results.episodes.get(0))
                    .and_then(|episode| episode.episode_id)
                {
                    Some(id) => id,
                    None => {
                        eprintln!("No episode_id found");
                        return;
                    }
                };
                let is_youtube = match search_state
                    .podcast_feed_results
                    .as_ref()
                    .and_then(|results| results.episodes.get(0))
                    .and_then(|episode| episode.is_youtube)
                {
                    Some(id) => id,
                    None => {
                        eprintln!("No is_youtube info found");
                        return;
                    }
                };
                let ep_api_key = api_key.clone();
                let ep_server_name = server_name.clone();
                let ep_user_id = user_id_copy.clone();
                match call_get_podcast_id_from_ep(
                    &ep_server_name.unwrap(),
                    &ep_api_key.unwrap(),
                    episode_id,
                    ep_user_id.unwrap(),
                    Some(is_youtube),
                )
                .await
                {
                    Ok(podcast_id) => {
                        let request = DownloadAllPodcastRequest {
                            podcast_id,
                            user_id: user_id_copy.unwrap(),
                        };

                        match call_download_all_podcast(
                            &server_name.unwrap(),
                            &api_key.flatten(),
                            &request,
                        )
                        .await
                        {
                            Ok(success_message) => {
                                call_down_dispatch.reduce_mut(|state| {
                                    state.info_message =
                                        Option::from(format!("{}", success_message))
                                });
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                call_down_dispatch.reduce_mut(|state| {
                                    state.error_message =
                                        Option::from(format!("{}", formatted_error))
                                });
                            }
                        }
                    }
                    Err(e) => {
                        call_down_dispatch.reduce_mut(|state| {
                            let formatted_error = format_error_message(&e.to_string());
                            state.error_message = Option::from(format!(
                                "Failed to get podcast ID: {}",
                                formatted_error
                            ))
                        });
                    }
                }
            });
        })
    };

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
        Download,
        Delete,
        RSSFeed,
    }

    let button_content = if *is_added { trash_icon() } else { add_icon() };

    let setting_content = if *is_added {
        settings_icon()
    } else {
        no_icon()
    };
    let download_all = if *is_added {
        download_icon()
    } else {
        no_icon()
    };

    let payment_icon = { payments_icon() };
    let rss_icon = { rss_icon() };

    let website_icon = { website_icon() };

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

    let toggle_download = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let download_status = download_status.clone();
        let podcast_id = podcast_id.clone();
        let user_id = user_id.clone();

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let download_status = download_status.clone();
            let auto_download = !*download_status;
            let pod_id_deref = *podcast_id.clone();
            let user_id = user_id.clone().unwrap();

            let request_data = AutoDownloadRequest {
                podcast_id: pod_id_deref, // Replace with the actual podcast ID
                user_id,
                auto_download,
            };

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    match call_enable_auto_download(
                        &server_name,
                        &api_key.clone().unwrap(),
                        &request_data,
                    )
                    .await
                    {
                        Ok(_) => {
                            download_status.set(auto_download);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error enabling/disabling downloads: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    let playback_speed_input_handler = Callback::from(move |e: InputEvent| {
        if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
            let value = input.value().parse::<f64>().unwrap_or(1.0);
            // Constrain to reasonable values (0.5 to 3.0)
            let value = value.max(0.5).min(2.0);
            playback_speed_input.set(value);
        }
    });

    // Create the save playback speed function
    let save_playback_speed = {
        let playback_speed = playback_speed.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let dispatch = _search_dispatch.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let call_dispatch = dispatch.clone();
            let speed = *playback_speed;
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap();
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    let request = PlaybackSpeedRequest {
                        podcast_id,
                        user_id,
                        playback_speed: speed,
                    };

                    match call_set_playback_speed(&server_name, &api_key, &request).await {
                        Ok(_) => {
                            call_dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Option::from("Playback speed updated".to_string())
                            });
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error updating playback speed: {}", e).into(),
                            );
                            call_dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Option::from("Error updating playback speed".to_string())
                            });
                        }
                    }
                }
            });
        })
    };

    // Create the clear playback speed function
    let clear_playback_speed = {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let dispatch = _search_dispatch.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let call_dispatch = dispatch.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap();
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;
            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    let request = ClearPlaybackSpeedRequest {
                        podcast_id,
                        user_id,
                    };
                    match call_clear_playback_speed(&server_name, &api_key, &request).await {
                        Ok(_) => {
                            call_dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Option::from("Playback speed reset to default".to_string())
                            });
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error resetting playback speed: {}", e).into(),
                            );
                            call_dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Option::from("Error resetting playback speed".to_string())
                            });
                        }
                    }
                }
            });
        })
    };

    // Add this callback for handling input changes
    let feed_cutoff_days_input_handler = {
        let feed_cutoff_days_input = feed_cutoff_days_input.clone();

        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                feed_cutoff_days_input.set(input.value());
            }
        })
    };

    // Add this callback for saving the feed cutoff days
    let save_feed_cutoff_days = {
        let dispatch_vid = _search_dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let podcast_id = podcast_id.clone();
        let feed_cutoff_days_input = feed_cutoff_days_input.clone();
        let feed_cutoff_days = feed_cutoff_days.clone();
        let user_id = search_state.user_details.as_ref().map(|ud| ud.UserID);

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let dispatch_wasm = dispatch_vid.clone();

            // Extract the values directly without creating intermediate variables
            if let (Some(server_val), Some(key_val), Some(user_val)) = (
                server_name.as_ref(),
                api_key.as_ref().and_then(|k| k.as_ref()),
                user_id,
            ) {
                let pod_id = *podcast_id;
                let days_str = (*feed_cutoff_days_input).clone();
                let days = days_str.parse::<i32>().unwrap_or(0);
                let request_data = UpdateFeedCutoffDaysRequest {
                    podcast_id: pod_id,
                    user_id: user_val,
                    feed_cutoff_days: days,
                };

                // Clone everything needed for the async block
                let server_val = server_val.clone();
                let key_val = key_val.clone();
                let feed_cutoff_days = feed_cutoff_days.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_update_feed_cutoff_days(&server_val, &Some(key_val), &request_data)
                        .await
                    {
                        Ok(_) => {
                            feed_cutoff_days.set(days);
                            dispatch_wasm.reduce_mut(|state| {
                                state.info_message =
                                    Option::from("Youtube Episode Limit Updated!".to_string())
                            });
                            // No need to update a ClickedFeedURL or PodcastInfo struct
                            // Just update the state
                        }
                        Err(err) => {
                            web_sys::console::log_1(
                                &format!("Error updating feed cutoff days: {}", err).into(),
                            );
                            dispatch_wasm.reduce_mut(|state| {
                                state.error_message = Option::from(format!(
                                    "Error updating feed cutoff days: {:?}",
                                    err
                                ))
                            });
                        }
                    }
                });
            }
        })
    };

    let toggle_notifications = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let notification_status = notification_status.clone();
        let podcast_id = podcast_id.clone();
        let user_id = user_id.clone();
        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let notification_status = notification_status.clone();
            let enabled = !*notification_status;
            let pod_id_deref = *podcast_id.clone();
            let user_id = user_id.clone().unwrap();

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    match call_toggle_podcast_notifications(
                        server_name.clone(),
                        api_key.clone().unwrap(),
                        user_id,
                        pod_id_deref,
                        enabled,
                    )
                    .await
                    {
                        Ok(_) => {
                            notification_status.set(enabled);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error toggling notifications: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    let start_skip_call = start_skip.clone();
    let end_skip_call = end_skip.clone();
    let start_skip_call_button = start_skip.clone();
    let end_skip_call_button = end_skip.clone();
    let skip_dispatch = _search_dispatch.clone();

    // Save the skip times to the server
    let save_skip_times = {
        let start_skip = start_skip.clone();
        let end_skip = end_skip.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let skip_dispatch = skip_dispatch.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let skip_call_dispatch = skip_dispatch.clone();
            let start_skip = *start_skip;
            let end_skip = *end_skip;
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap();
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    let request = SkipTimesRequest {
                        podcast_id,
                        start_skip,
                        end_skip,
                        user_id,
                    };

                    match call_adjust_skip_times(&server_name, &api_key, &request).await {
                        Ok(_) => {
                            skip_call_dispatch.reduce_mut(|state| {
                                state.info_message = Option::from("Skip times Adjusted".to_string())
                            });
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error updating skip times: {}", e).into(),
                            );
                            skip_call_dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Option::from("Error Adjusting Skip Times".to_string())
                            });
                        }
                    }
                }
            });
        })
    };

    // let onclick_cat = new_category
    let app_dispatch_add = _search_dispatch.clone();
    let onclick_add = {
        // let dispatch = dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone(); // Assuming user_id is an Option<i32> or similar
        let podcast_id = podcast_id.clone(); // Assuming this is available in your context
        let new_category = new_category.clone(); // Assuming this is a state that stores the new category input

        Callback::from(move |event: web_sys::MouseEvent| {
            event.prevent_default(); // Prevent the default form submit or page reload behavior
            let app_dispatch = app_dispatch_add.clone();
            if new_category.is_empty() {
                web_sys::console::log_1(&"Category name cannot be empty".into());
                return;
            }

            // let dispatch = dispatch.clone();
            let server_name = server_name.clone().unwrap();
            let api_key = api_key.clone().unwrap();
            let user_id = user_id.clone().unwrap(); // Assuming user_id is Some(i32)
            let podcast_id = *podcast_id; // Assuming podcast_id is Some(i32)
            let category_name = (*new_category).clone();
            let cat_name_dis = category_name.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let request_data = AddCategoryRequest {
                    podcast_id,
                    user_id,
                    category: category_name,
                };

                // Await the async function call
                let response = call_add_category(&server_name, &api_key, &request_data).await;

                // Match on the awaited response
                match response {
                    Ok(_) => {
                        app_dispatch.reduce_mut(|state| {
                            if let Some(ref mut podcast_info) = state.clicked_podcast_info {
                                if let Some(ref mut categories) = podcast_info.categories {
                                    // Add the new category to the HashMap
                                    categories.insert(cat_name_dis.clone(), cat_name_dis.clone());
                                } else {
                                    // Initialize the HashMap if it's None
                                    let mut new_map = HashMap::new();
                                    new_map.insert(cat_name_dis.clone(), cat_name_dis);
                                    podcast_info.categories = Some(new_map);
                                }
                            }
                        });
                    }
                    Err(err) => {
                        web_sys::console::log_1(&format!("Error adding category: {}", err).into());
                    }
                }
            });
        })
    };

    let category_to_remove = use_state(|| None::<String>);
    let onclick_remove = {
        let category_to_remove = category_to_remove.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            let target = event.target_unchecked_into::<Element>();
            let closest_button = target.closest("button").unwrap();
            if let Some(button) = closest_button {
                if let Some(category) = button.get_attribute("data-category") {
                    category_to_remove.set(Some(category));
                }
            }
        })
    };

    let app_dispatch = _search_dispatch.clone();

    {
        let category_to_remove = category_to_remove.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id;
        let podcast_id = *podcast_id;

        use_effect_with(category_to_remove, move |category_to_remove| {
            if let Some(category) = (**category_to_remove).clone() {
                let server_name = server_name.clone().unwrap();
                let api_key = api_key.clone().unwrap();
                let user_id = user_id.unwrap();
                let category_request = category.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let request_data = RemoveCategoryRequest {
                        podcast_id,
                        user_id,
                        category,
                    };
                    // Your API call here
                    let response =
                        call_remove_category(&server_name, &api_key, &request_data).await;
                    match response {
                        Ok(_) => {
                            app_dispatch.reduce_mut(|state| {
                                if let Some(ref mut podcast_info) = state.clicked_podcast_info {
                                    if let Some(ref mut categories) = podcast_info.categories {
                                        // Filter the HashMap and collect back into HashMap
                                        *categories = categories
                                            .clone()
                                            .into_iter()
                                            .filter(|(_, cat)| cat != &category_request) // Ensure you're comparing correctly
                                            .collect();
                                    }
                                }
                            });
                        }
                        Err(err) => {
                            web_sys::console::log_1(
                                &format!("Error removing category: {}", err).into(),
                            );
                        }
                    }
                });
            }
            || ()
        });
    }

    // Fetch RSS key when RSS feed modal is shown
    {
        let rss_key_state = rss_key_state.clone();
        let server_name = search_state
            .auth_details
            .as_ref()
            .map(|ud| ud.server_name.clone());
        let api_key = api_key.clone().flatten();
        let page_state_clone = page_state.clone();

        use_effect_with(
            (page_state_clone.clone(), rss_key_state.is_none()),
            move |(current_page_state, rss_key_is_none)| {
                if matches!(**current_page_state, PageState::RSSFeed) && *rss_key_is_none {
                    if let (Some(server_name), Some(api_key), Some(user_id)) =
                        (server_name.clone(), api_key.clone(), user_id.clone())
                    {
                        let rss_key_state = rss_key_state.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match call_get_rss_key(&server_name, &Some(api_key), user_id).await {
                                Ok(rss_key) => {
                                    rss_key_state.set(Some(rss_key));
                                }
                                Err(e) => {
                                    web_sys::console::log_1(
                                        &format!("Failed to fetch RSS key: {}", e).into(),
                                    );
                                }
                            }
                        });
                    }
                }
                || ()
            },
        );
    }

    let rss_feed_modal = {
        let rss_key_state_clone = rss_key_state.clone();

        let rss_url = match (*rss_key_state_clone).as_ref() {
            Some(rss_key) => format!(
                "{}/{}?api_key={}&podcast_id={}",
                get_rss_base_url(),
                user_id.clone().unwrap_or_default(),
                rss_key,
                *podcast_id
            ),
            None => "Loading RSS key...".to_string(),
        };

        let copy_onclick = {
            let rss_url = rss_url.clone();
            Callback::from(move |_| {
                if let Some(window) = web_sys::window() {
                    let _ = window.navigator().clipboard().write_text(&rss_url);
                }
            })
        };

        html! {
            <div id="rss_feed_modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
                <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                    <div class="modal-container relative rounded-lg shadow">
                        <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                            <h3 class="text-xl font-semibold">
                                {"RSS Feed URL"}
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
                                <label for="rss_link" class="block mb-2 text-sm font-medium">{"NOTE: You must have RSS feeds enabled in your settings for the link below to work"}</label>
                                <label for="rss_link" class="block mb-2 text-sm font-medium">{"Use this RSS feed URL in your favorite podcast app to subscribe to this podcast:"}</label>
                                <div class="relative">
                                    <input
                                        type="text"
                                        id="rss_link"
                                        class="input-black w-full px-3 py-2 border border-gray-300 rounded-md pr-20"
                                        value={rss_url}
                                        readonly=true
                                    />
                                    <button
                                        onclick={copy_onclick}
                                        class="absolute right-2 top-1/2 transform -translate-y-1/2 px-4 py-1 text-sm text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300"
                                    >
                                        {"Copy"}
                                    </button>
                                </div>
                                <p class="mt-2 text-sm text-gray-500">{"This URL includes your API key, so keep it private. This is going to change very shortly in a future release to a temporary key so that you can share these links."}</p>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        }
    };

    // Define the modal components
    let clicked_feed = clicked_podcast_info.clone();
    let podcast_option_model = html! {
        <div id="podcast_option_model" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Podcast Options"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{"Download Future Episodes Automatically:"}</label>
                                <label class="inline-flex relative items-center cursor-pointer">
                                    <input type="checkbox" checked={*download_status} class="sr-only peer" onclick={toggle_download} />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                </label>
                            </div>
                            <div>
                                <label for="notification_settings" class="block mb-2 text-sm font-medium">{"Get Notifications For New Episodes:"}</label>
                                <label class="inline-flex relative items-center cursor-pointer">
                                    <input
                                        type="checkbox"
                                        checked={*notification_status}
                                        class="sr-only peer"
                                        onclick={toggle_notifications}
                                    />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                </label>
                            </div>

                            <div class="mt-4">
                                <label for="playback-speed" class="block mb-2 text-sm font-medium">{"Default Playback Speed:"}</label>
                                <div class="flex items-center space-x-2">
                                    <input
                                        type="number"
                                        id="playback-speed"
                                        value={format!("{:.1}", *playback_speed_clone)} // Format to 1 decimal place
                                        class="email-input border text-sm rounded-lg p-2.5 w-20"
                                        oninput={playback_speed_input_handler}
                                        min="0.5"
                                        max="2.0"
                                        step="0.1"
                                    />
                                    <span class="text-sm">{"x"}</span>
                                    <button
                                        class="save-button font-bold py-2 px-4 rounded"
                                        onclick={save_playback_speed}
                                    >
                                        {"Save"}
                                    </button>
                                    <button
                                        class="clear-button bg-gray-300 hover:bg-gray-400 text-gray-800 font-bold py-2 px-4 rounded"
                                        onclick={clear_playback_speed}
                                    >
                                        {"Reset"}
                                    </button>
                                </div>
                                <p class="text-xs text-gray-500 mt-1">{"Sets the default playback speed for this podcast. Range: 0.5x - 2.0x. Reset to use your global default."}</p>
                            </div>

                            <div class="mt-4">
                                <label for="auto-skip" class="block mb-2 text-sm font-medium">{"Auto Skip Intros and Outros:"}</label>
                                <div class="flex items-center space-x-2">
                                    <div class="flex items-center space-x-2">
                                        <label for="start-skip" class="block text-sm font-medium">{"Start Skip (seconds):"}</label>
                                        <input
                                            type="number"
                                            id="start-skip"
                                            value={start_skip_call_button.to_string()}
                                            class="email-input border text-sm rounded-lg p-2.5 w-16"
                                            oninput={Callback::from(move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    let value = input.value().parse::<i32>().unwrap_or(0);
                                                    start_skip_call.set(value);
                                                }
                                            })}
                                        />
                                    </div>
                                    <div class="flex items-center space-x-2">
                                        <label for="end-skip" class="block text-sm font-medium">{"End Skip (seconds):"}</label>
                                        <input
                                            type="number"
                                            id="end-skip"
                                            value={end_skip_call_button.to_string()}
                                            class="email-input border text-sm rounded-lg p-2.5 w-16"
                                            oninput={Callback::from(move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    let value = input.value().parse::<i32>().unwrap_or(0);
                                                    end_skip_call.set(value);
                                                }
                                            })}
                                        />
                                    </div>
                                    <button
                                        class="download-button font-bold py-2 px-4 rounded"
                                        onclick={save_skip_times}
                                    >
                                        {"Confirm"}
                                    </button>
                                </div>
                            </div>

                            {
                                if let Some(info) = &podcast_info {
                                    if info.is_youtube.unwrap_or(false) {
                                        html! {
                                            <div class="mt-4">
                                                <label for="feed-cutoff" class="block mb-2 text-sm font-medium">{"Youtube Download Episode Limit (days):"}</label>
                                                <div class="flex items-center space-x-2">
                                                    <input
                                                        type="number"
                                                        id="feed-cutoff"
                                                        value={(*feed_cutoff_days_input).clone()}
                                                        class="email-input border text-sm rounded-lg p-2.5 w-24"
                                                        oninput={feed_cutoff_days_input_handler}
                                                        min="0"
                                                    />
                                                    <span class="text-sm text-gray-500">{"0 = No limit"}</span>
                                                    <button
                                                        class="download-button font-bold py-2 px-4 rounded"
                                                        onclick={save_feed_cutoff_days}
                                                    >
                                                        {"Save"}
                                                    </button>
                                                </div>
                                                <p class="text-xs text-gray-500 mt-1">{"Adjusts how long Youtube Feed audio is retained when downloaded to be streamed via the server. Youtube episodes will be removed after to free up space."}</p>
                                            </div>
                                        }
                                    } else {
                                        html! {}  // Render nothing if it's not a YouTube podcast
                                    }
                                } else {
                                    html! {}  // Render nothing if podcast_info is None
                                }
                            }
                            // Categories section of the modal
                            <div>
                                <label for="category_adjust" class="block mb-2 text-sm font-medium">
                                    {"Adjust Podcast Categories:"}
                                </label>
                                <div class="flex flex-wrap gap-2">
                                {
                                    if let Some(feed) = clicked_feed.as_ref() {
                                        if let Some(categories) = &feed.categories {
                                            html! {
                                                <>
                                                    { categories.iter().map(|(_, category_name)| {
                                                        let category_name = category_name.clone();
                                                        let onclick_remove = onclick_remove.clone();
                                                        let category_to_remove = category_to_remove.clone();

                                                        let remove_callback = {
                                                            let category_name = category_name.clone();
                                                            let onclick_remove = onclick_remove.clone();
                                                            Callback::from(move |e: MouseEvent| {
                                                                e.prevent_default();
                                                                onclick_remove.emit(e);
                                                                category_to_remove.set(Some(category_name.clone()));
                                                            })
                                                        };

                                                        html! {
                                                            <div class="category-tag">
                                                                <span>{&category_name}</span>
                                                                <button
                                                                    class="category-remove-btn"
                                                                    onclick={remove_callback}
                                                                    data-category={category_name.clone()}
                                                                >
                                                                    <i class="ph ph-trash text-lg" />
                                                                </button>
                                                            </div>
                                                        }
                                                    }).collect::<Html>() }
                                                </>
                                            }
                                        } else {
                                            html! { <p class="text-sm text-muted">{ "No categories available" }</p> }
                                        }
                                    } else {
                                        html! { <p class="text-sm text-muted">{ "Loading..." }</p> }
                                    }
                                }
                                </div>

                                <div class="relative mt-4">
                                    <input
                                        type="text"
                                        id="new_category"
                                        class="category-input w-full px-4 py-3 pr-24 rounded-lg border"
                                        placeholder="New category"
                                        value={(*new_category).clone()}
                                        oninput={new_category_input}
                                    />
                                    <button
                                        class="category-add-btn"
                                        onclick={onclick_add}
                                    >
                                        <i class="ph ph-plus text-lg" />
                                        <span class="hidden md:inline">{"Add"}</span>
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define the modal components
    let download_all_model = html! {
        <div id="download_all_modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Verify Downloads"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{"Are you sure you want to download all episodes from the current podcast to the server? If the podcast has a lot of episodes this might take awhile."}</label>
                                <div class="flex justify-between space-x-4">
                                    <button onclick={download_all_click} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {"Yes, Download All"}
                                    </button>
                                    <button onclick={on_close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {"No, take me back"}
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define the modal components
    let delete_pod_model = html! {
        <div id="delete_pod_model" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Delete Podcast"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{"Are you sure you want to delete the podcast from the database? This will remove it from every aspect of the app. Meaning this will remove any saved, downloaded, or queued episodes for this podcast. It will also remove any history that includes it."}</label>
                                <div class="flex justify-between space-x-4">
                                    <button onclick={delete_all_click} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {"Yes, Delete Podcast"}
                                    </button>
                                    <button onclick={on_close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {"No, take me back"}
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define the callback functions
    let toggle_settings = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Shown);
        })
    };

    let toggle_download = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Download);
        })
    };

    let toggle_delete = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Delete);
        })
    };

    let toggle_podcast = {
        let add_dispatch = _search_dispatch.clone();
        let pod_values = clicked_podcast_info.clone();
        let user_id_og = user_id.clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let user_id_clone = user_id.clone();
        let app_dispatch = _search_dispatch.clone();

        let is_added = is_added.clone();
        let added_id = podcast_id.clone();

        if *is_added == true {
            toggle_delete
        } else {
            Callback::from(move |_: MouseEvent| {
                // Ensure this is triggered only by a MouseEvent
                let callback_podcast_id = added_id.clone();
                let podcast_id_og = Some(pod_values.clone().unwrap().podcastid.clone());
                let pod_title_og = pod_values.clone().unwrap().podcastname.clone();
                let pod_artwork_og = pod_values.clone().unwrap().artworkurl.clone();
                let pod_author_og = pod_values.clone().unwrap().author.clone();
                let categories_og = pod_values.clone().unwrap().categories.unwrap().clone();
                let pod_description_og = pod_values.clone().unwrap().description.clone();
                let pod_episode_count_og = pod_values.clone().unwrap().episodecount.clone();
                let pod_feed_url_og = pod_values.clone().unwrap().feedurl.clone();
                let pod_website_og = pod_values.clone().unwrap().websiteurl.clone();
                let pod_explicit_og = pod_values.clone().unwrap().explicit.clone();
                let app_dispatch = app_dispatch.clone();
                app_dispatch.reduce_mut(|state| state.is_loading = Some(true));
                let is_added_inner = is_added.clone();
                let call_dispatch = add_dispatch.clone();
                let pod_title = pod_title_og.clone();
                let pod_artwork = pod_artwork_og.clone();
                let pod_author = pod_author_og.clone();
                let categories = categories_og.clone();
                let pod_description = pod_description_og.clone();
                let pod_episode_count = pod_episode_count_og.clone();
                let pod_feed_url = pod_feed_url_og.clone();
                let pod_website = pod_website_og.clone();
                let pod_explicit = pod_explicit_og.clone();
                let user_id = user_id_og.clone().unwrap();
                let podcast_values = PodcastValues {
                    pod_title,
                    pod_artwork,
                    pod_author,
                    categories,
                    pod_description,
                    pod_episode_count,
                    pod_feed_url,
                    pod_website,
                    pod_explicit,
                    user_id,
                };
                let api_key_call = api_key_clone.clone();
                let server_name_call = server_name_clone.clone();
                let user_id_call = user_id_clone.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let dispatch_wasm = call_dispatch.clone();
                    let api_key_wasm = api_key_call.clone().unwrap();
                    let user_id_wasm = user_id_call.clone().unwrap();
                    let server_name_wasm = server_name_call.clone();
                    let pod_values_clone = podcast_values.clone(); // Make sure you clone the podcast values

                    match call_add_podcast(
                        &server_name_wasm.clone().unwrap(),
                        &api_key_wasm,
                        user_id_wasm,
                        &pod_values_clone,
                        podcast_id_og,
                    )
                    .await
                    {
                        Ok(response_body) => {
                            // Replace the problematic sections in episodes_layout.rs with this code:

                            // First issue: response_body.podcast_id is now i32, not Option<i32>
                            if response_body.success {
                                dispatch_wasm.reduce_mut(|state| {
                                    state.info_message =
                                        Option::from("Podcast successfully added".to_string())
                                });
                                app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                                is_added_inner.set(true);

                                // podcast_id is now directly an i32, not an Option
                                let call_podcast_id = response_body.podcast_id;
                                callback_podcast_id.set(call_podcast_id);

                                // Since first_episode_id is now an i32, use it directly
                                let episode_id = Some(response_body.first_episode_id);
                                // Use the episode_id for further processing
                                app_dispatch.reduce_mut(|state| {
                                    state.selected_episode_id = episode_id;
                                    // Now this matches Option<i32>
                                });

                                // Fetch episodes - podcast_id is now direct i32
                                match call_get_podcast_episodes(
                                    &server_name_wasm.unwrap(),
                                    &api_key_wasm,
                                    &user_id_wasm,
                                    &call_podcast_id,
                                )
                                .await
                                {
                                    Ok(podcast_feed_results) => {
                                        app_dispatch.reduce_mut(move |state| {
                                            state.podcast_feed_results = Some(podcast_feed_results);
                                        });
                                        app_dispatch
                                            .reduce_mut(|state| state.is_loading = Some(false));
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(
                                            &format!("Error fetching episodes: {:?}", e).into(),
                                        );
                                    }
                                }

                                app_dispatch.reduce_mut(|state| {
                                    state.podcast_added = Some(podcast_added);
                                });
                            } else {
                                dispatch_wasm.reduce_mut(|state| {
                                    state.error_message =
                                        Option::from("Failed to add podcast".to_string())
                                });
                                app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                            }
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch_wasm.reduce_mut(|state| {
                                state.error_message = Option::from(format!(
                                    "Error adding podcast: {:?}",
                                    formatted_error
                                ))
                            });
                            app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                        }
                    }
                });
            })
        }
    };

    #[derive(Clone, PartialEq)]
    enum CompletedFilter {
        ShowAll,
        ShowOnly,
        Hide,
    }

    let filtered_episodes = use_memo(
        (
            podcast_feed_results.clone(),
            episode_search_term.clone(),
            episode_sort_direction.clone(),
            completed_filter_state.clone(), // Changed from show_completed
            show_in_progress.clone(),
        ),
        |(episodes, search, sort_dir, _show_completed, show_in_progress)| {
            if let Some(results) = episodes {
                let mut filtered = results
                    .episodes
                    .iter()
                    .filter(|episode| {
                        // Search filter
                        let matches_search = if !search.is_empty() {
                            episode.title.as_ref().map_or(false, |title| {
                                title.to_lowercase().contains(&search.to_lowercase())
                            }) || episode.description.as_ref().map_or(false, |desc| {
                                desc.to_lowercase().contains(&search.to_lowercase())
                            })
                        } else {
                            true
                        };

                        // Status filter
                        let matches_status = if **show_in_progress {
                            !episode.completed.unwrap_or(false)
                                && episode.listen_duration.unwrap_or(0) > 0
                        } else {
                            match *completed_filter_state {
                                CompletedFilter::ShowOnly => episode.completed.unwrap_or(false),
                                CompletedFilter::Hide => !episode.completed.unwrap_or(false),
                                CompletedFilter::ShowAll => true,
                            }
                        };

                        matches_search && matches_status
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                // Sort logic
                if let Some(direction) = (*sort_dir).as_ref() {
                    filtered.sort_by(|a, b| match direction {
                        EpisodeSortDirection::NewestFirst => b.pub_date.cmp(&a.pub_date),
                        EpisodeSortDirection::OldestFirst => a.pub_date.cmp(&b.pub_date),
                        EpisodeSortDirection::ShortestFirst => a.duration.cmp(&b.duration),
                        EpisodeSortDirection::LongestFirst => b.duration.cmp(&a.duration),
                        EpisodeSortDirection::TitleAZ => a.title.cmp(&b.title),
                        EpisodeSortDirection::TitleZA => b.title.cmp(&a.title),
                    });
                }
                filtered
            } else {
                vec![]
            }
        },
    );

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggle_description(guid: &str);
    }

    let web_link = open_in_new_tab.clone();
    let pod_layout_data = clicked_podcast_info.clone();


    let (completed_icon, completed_text, completed_title) = match *completed_filter_state {
        CompletedFilter::ShowOnly => (
            "ph-check-circle",
            "Show Only",
            "Showing only completed episodes",
        ),
        CompletedFilter::Hide => ("ph-x-circle", "Hide", "Hiding completed episodes"),
        CompletedFilter::ShowAll => ("ph-circle", "All", "Showing all episodes"),
    };

    html! {
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                match *page_state {
                PageState::Shown => podcast_option_model,
                PageState::Download => download_all_model,
                PageState::Delete => delete_pod_model,
                PageState::RSSFeed => rss_feed_modal,
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
                    html! {
                        <>
                            {
                                if let Some(podcast_info) = pod_layout_data {
                                    let sanitized_title = podcast_info.podcastname.replace(|c: char| !c.is_alphanumeric(), "-");
                                    let desc_id = format!("desc-{}", sanitized_title);
                                    let pod_link = podcast_info.websiteurl.clone();

                                    let toggle_description = {
                                        let desc_id = desc_id.clone();
                                        Callback::from(move |_| {
                                            let desc_id = desc_id.clone();
                                            wasm_bindgen_futures::spawn_local(async move {
                                                let window = web_sys::window().expect("no global `window` exists");
                                                let function = window
                                                    .get("toggle_description")
                                                    .expect("should have `toggle_description` as a function")
                                                    .dyn_into::<js_sys::Function>()
                                                    .unwrap();
                                                let this = JsValue::NULL;
                                                let guid = JsValue::from_str(&desc_id);
                                                function.call1(&this, &guid).unwrap();
                                            });
                                        })
                                    };
                                    let sanitized_description = sanitize_html(&podcast_info.description);
                                    let layout = if state.is_mobile.unwrap_or(false) {
                                        html! {
                                            <div class="mobile-layout">
                                                <div class="button-container">
                                                    <button
                                                        onclick={
                                                            let pod_link = pod_link.clone();
                                                            Callback::from(move |_| web_link.emit(pod_link.clone()))
                                                        }
                                                        title="Visit external podcast website" class="item-container-button font-bold rounded-full self-center mr-4">
                                                        { website_icon }
                                                    </button>
                                                    {
                                                        if let Some(funding_list) = &state.podcast_funding {
                                                            if !funding_list.is_empty() {
                                                                let funding_list_clone = funding_list.clone();
                                                                html! {
                                                                    <>
                                                                    { for funding_list_clone.iter().map(|funding| {
                                                                        let open_in_new_tab = open_in_new_tab.clone();
                                                                        let payment_icon = payment_icon.clone();
                                                                        let url = funding.url.clone();
                                                                        html! {
                                                                            <button
                                                                                onclick={Callback::from(move |_| open_in_new_tab.emit(url.clone()))}
                                                                                title={funding.description.clone()}
                                                                                class="item-container-button font-bold rounded-full self-center mr-4"
                                                                            >
                                                                                { payment_icon } // Replace with your payment_icon component
                                                                            </button>
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
                                                        if search_state.podcast_added.unwrap() {
                                                            html! {
                                                                <button
                                                                    onclick={
                                                                        let page_state = page_state.clone();
                                                                        Callback::from(move |_| {
                                                                            page_state.set(PageState::RSSFeed);
                                                                        })
                                                                    }
                                                                    title="Get RSS Feed URL"
                                                                    class="item-container-button font-bold rounded-full self-center mr-4"
                                                                    style="width: 30px; height: 30px;"
                                                                >
                                                                    { rss_icon }
                                                                </button>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        if search_state.podcast_added.unwrap() {
                                                            html! {
                                                                <button onclick={toggle_download} title="Click to download all episodes for this podcast" class="item-container-button font-bold rounded-full self-center mr-4">
                                                                    { download_all }
                                                                </button>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    <button onclick={toggle_podcast} title="Click to add or remove podcast from feed" class="item-container-button font-bold rounded-full self-center mr-4">
                                                        { button_content }
                                                    </button>
                                                    {
                                                        if search_state.podcast_added.unwrap() {
                                                            html! {
                                                                <button onclick={toggle_settings} title="Click to setup podcast specific settings" class="item-container-button font-bold rounded-full self-center mr-4">
                                                                    { setting_content }
                                                                </button>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>
                                                <div class="item-header-mobile-cover-container">
                                                    <FallbackImage
                                                        src={podcast_info.artworkurl.clone()}
                                                        // onclick={on_title_click.clone()}
                                                        alt={format!("Cover for {}", &podcast_info.podcastname)}
                                                        class={"item-header-mobile-cover"}
                                                    />
                                                </div>

                                                <h2 class="item-header-title">{ &podcast_info.podcastname }</h2>
                                                <div class="item-header-description desc-collapsed" id={desc_id.clone()} onclick={toggle_description.clone()}>
                                                    { sanitized_description }
                                                    <button class="toggle-desc-btn" onclick={toggle_description}>{ "" }</button>
                                                </div>
                                                <p class="header-info">{ format!("Episode Count: {}", &podcast_info.episodecount) }</p>
                                                <p class="header-info">{ format!("Authors: {}", &podcast_info.author) }</p>
                                                <p class="header-info">{ format!("Explicit: {}", if podcast_info.explicit { "Yes" } else { "No" }) }</p>
                                                {
                                                    if !podcast_info.is_youtube.unwrap_or(false) {  // Only show if not a YouTube channel
                                                        if podcast_info.podcastindexid == 0 {
                                                            html! {
                                                                <div class="import-box mt-2">
                                                                    <p class="item_container-text text-sm">
                                                                        {"⚠️ This podcast isn't matched to Podcast Index. "}
                                                                        <a href="/settings#podcast-index-matching" class="item_container-text underline hover:opacity-80 font-semibold">
                                                                            {"Match it here"}
                                                                        </a>
                                                                        {" to enable host and guest information."}
                                                                    </p>
                                                                </div>
                                                            }
                                                        } else if let Some(people) = &state.podcast_people {
                                                            if !people.is_empty() {
                                                                html! {
                                                                    <div class="header-info relative">
                                                                        <div class="max-w-full overflow-x-auto">
                                                                            <HostDropdown
                                                                                title="Hosts"
                                                                                hosts={people.clone()}
                                                                                podcast_feed_url={podcast_info.feedurl}
                                                                                podcast_id={*podcast_id}
                                                                                podcast_index_id={podcast_info.podcastindexid}
                                                                            />
                                                                        </div>
                                                                    </div>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                                <div>
                                                <div class="categories-container">
                                                {
                                                    if let Some(categories) = &podcast_info.categories {
                                                        html! {
                                                            for categories.iter().map(|(_, category_name)| {
                                                                html! { <span class="category-box">{ category_name }</span> }
                                                            })
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                                </div>

                                                </div>
                                            </div>
                                        }
                                    } else {
                                        let pod_link = podcast_info.feedurl.clone();
                                        html! {
                                            <div class="item-header">
                                                <FallbackImage
                                                    src={podcast_info.artworkurl.clone()}
                                                    // onclick={on_title_click.clone()}
                                                    alt={format!("Cover for {}", &podcast_info.podcastname)}
                                                    class={"item-header-cover"}
                                                />
                                                <div class="item-header-info">
                                                    <div class="title-button-container">
                                                        <h2 class="item-header-title">{ &podcast_info.podcastname }</h2>
                                                        {
                                                            if search_state.podcast_added.unwrap() {
                                                                html! {
                                                                    <button onclick={toggle_download} title="Click to download all episodes for this podcast" class="item-container-button font-bold rounded-full self-center mr-4">
                                                                        { download_all }
                                                                    </button>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                        <button onclick={toggle_podcast} title="Click to add or remove podcast from feed" class={"item-container-button font-bold py-2 px-4 rounded-full self-center mr-4"} style="width: 60px; height: 60px;">
                                                            { button_content }
                                                        </button>
                                                        {
                                                            if search_state.podcast_added.unwrap() {
                                                                html! {
                                                                    <button onclick={toggle_settings} title="Click to setup podcast specific settings" class="item-container-button font-bold rounded-full self-center mr-4">
                                                                        { setting_content }
                                                                    </button>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                    </div>

                                                    // <p class="item-header-description">{ &podcast_info.podcast_description }</p>
                                                    <div class="item-header-description desc-collapsed" id={desc_id.clone()} onclick={toggle_description.clone()}>
                                                        { sanitized_description }
                                                        <button class="toggle-desc-btn" onclick={toggle_description}>{ "" }</button>
                                                    </div>
                                                    <button
                                                        onclick={Callback::from(move |_| web_link.clone().emit(pod_link.to_string()))}
                                                        title="Visit external podcast website" class={"item-container-button font-bold rounded-full self-center mr-4"} style="width: 30px; height: 30px;">
                                                        { website_icon }
                                                    </button>
                                                    {
                                                        if let Some(funding_list) = &state.podcast_funding {
                                                            if !funding_list.is_empty() {
                                                                let funding_list_clone = funding_list.clone();
                                                                html! {
                                                                    <>
                                                                    { for funding_list_clone.iter().map(|funding| {
                                                                        let open_in_new_tab = open_in_new_tab.clone();
                                                                        let payment_icon = payment_icon.clone();
                                                                        let url = funding.url.clone();
                                                                        html! {
                                                                            <button
                                                                                onclick={Callback::from(move |_| open_in_new_tab.emit(url.clone()))}
                                                                                title={funding.description.clone()}
                                                                                class="item-container-button font-bold rounded-full self-center mr-4"
                                                                                style="width: 30px; height: 30px;"
                                                                            >
                                                                                { payment_icon } // Replace with your payment_icon component
                                                                            </button>
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
                                                        if search_state.podcast_added.unwrap_or(false) {
                                                            html! {
                                                                <button
                                                                    onclick={
                                                                        let page_state = page_state.clone();
                                                                        Callback::from(move |_| {
                                                                            page_state.set(PageState::RSSFeed);
                                                                        })
                                                                    }
                                                                    title="Get RSS Feed URL"
                                                                    class={"item-container-button font-bold rounded-full self-center mr-4"}
                                                                    style="width: 30px; height: 30px;">
                                                                    { rss_icon }
                                                                </button>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    <div class="item-header-info">

                                                        <p class="header-text">{ format!("Episode Count: {}", &podcast_info.episodecount) }</p>
                                                        <p class="header-text">{ format!("Authors: {}", &podcast_info.author) }</p>
                                                        <p class="header-text">{ format!("Explicit: {}", if podcast_info.explicit { "Yes" } else { "No" }) }</p>
                                                    {
                                                        if podcast_info.podcastindexid == 0 {
                                                            html! {
                                                                <div class="import-box mt-2">
                                                                    <p class="item_container-text text-sm">
                                                                        {"⚠️ This podcast isn't matched to Podcast Index. "}
                                                                        <a href="/settings#podcast-index-matching" class="item_container-text underline hover:opacity-80 font-semibold">
                                                                            {"Match it here"}
                                                                        </a>
                                                                        {" to enable host and guest information."}
                                                                    </p>
                                                                </div>
                                                            }
                                                        } else if let Some(people) = &state.podcast_people {
                                                            if !people.is_empty() {
                                                                html! {
                                                                    <div class="header-info relative" style="max-width: 100%; min-width: 0;">  // Added min-width: 0 to allow shrinking
                                                                        <div class="max-w-full overflow-x-auto">
                                                                            <HostDropdown
                                                                                title="Hosts"
                                                                                hosts={people.clone()}
                                                                                podcast_feed_url={podcast_info.feedurl}
                                                                                podcast_id={*podcast_id}
                                                                                podcast_index_id={podcast_info.podcastindexid}
                                                                            />
                                                                        </div>
                                                                    </div>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                        <div>
                                                            {
                                                                if let Some(categories) = &podcast_info.categories {
                                                                    html! {
                                                                        for categories.values().map(|category_name| {
                                                                            html! { <span class="category-box">{ category_name }</span> }
                                                                        })
                                                                    }
                                                                } else {
                                                                    html! {}
                                                                }
                                                            }
                                                        </div>

                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    };

                                    layout
                                } else {
                                    html! {}
                                }
                            }
                            {
                                // Modern mobile-friendly filter bar
                                html! {
                                    <div class="mb-6 space-y-4">
                                        // Combined search and sort bar (seamless design)
                                        <div class="flex gap-0 h-12">
                                            // Search input (left half)
                                            <div class="flex-1 relative">
                                                <input
                                                    type="text"
                                                    class="search-input"
                                                    placeholder="Search podcast episodes..."
                                                    value={(*episode_search_term).clone()}
                                                    oninput={
                                                        let episode_search_term = episode_search_term.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                                episode_search_term.set(input.value());
                                                            }
                                                        })
                                                    }
                                                />
                                                <i class="ph ph-magnifying-glass search-icon"></i>
                                            </div>

                                            // Sort dropdown (right half)
                                            <div class="flex-shrink-0 relative min-w-[160px]">
                                                <select
                                                    class="sort-dropdown"
                                                    onchange={
                                                        let episode_sort_direction = episode_sort_direction.clone();
                                                        Callback::from(move |e: Event| {
                                                            let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                            let value = target.value();
                                                            
                                                            // Save preference to local storage
                                                            set_filter_preference("episodes", &value);
                                                            
                                                            match value.as_str() {
                                                                "newest" => episode_sort_direction.set(Some(EpisodeSortDirection::NewestFirst)),
                                                                "oldest" => episode_sort_direction.set(Some(EpisodeSortDirection::OldestFirst)),
                                                                "shortest" => episode_sort_direction.set(Some(EpisodeSortDirection::ShortestFirst)),
                                                                "longest" => episode_sort_direction.set(Some(EpisodeSortDirection::LongestFirst)),
                                                                "title_az" => episode_sort_direction.set(Some(EpisodeSortDirection::TitleAZ)),
                                                                "title_za" => episode_sort_direction.set(Some(EpisodeSortDirection::TitleZA)),
                                                                _ => episode_sort_direction.set(None),
                                                            }
                                                        })
                                                    }
                                                >
                                                    <option value="newest" selected={get_filter_preference("episodes").unwrap_or_else(|| get_default_sort_direction().to_string()) == "newest"}>{"Newest First"}</option>
                                                    <option value="oldest" selected={get_filter_preference("episodes").unwrap_or_else(|| get_default_sort_direction().to_string()) == "oldest"}>{"Oldest First"}</option>
                                                    <option value="shortest" selected={get_filter_preference("episodes").unwrap_or_else(|| get_default_sort_direction().to_string()) == "shortest"}>{"Shortest First"}</option>
                                                    <option value="longest" selected={get_filter_preference("episodes").unwrap_or_else(|| get_default_sort_direction().to_string()) == "longest"}>{"Longest First"}</option>
                                                    <option value="title_az" selected={get_filter_preference("episodes").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_az"}>{"Title A-Z"}</option>
                                                    <option value="title_za" selected={get_filter_preference("episodes").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_za"}>{"Title Z-A"}</option>
                                                </select>
                                                <i class="ph ph-caret-down dropdown-arrow"></i>
                                            </div>
                                        </div>

                                        // Filter chips (horizontal scroll on mobile)
                                        <div class="flex gap-3 overflow-x-auto pb-2 md:pb-0 scrollbar-hide">
                                            // Clear all filters
                                            <button
                                                onclick={
                                                    let show_in_progress = show_in_progress.clone();
                                                    let episode_search_term = episode_search_term.clone();
                                                    let completed_filter_state = completed_filter_state.clone();
                                                    Callback::from(move |_| {
                                                        completed_filter_state.set(CompletedFilter::ShowAll);
                                                        show_in_progress.set(false);
                                                        episode_search_term.set(String::new());
                                                    })
                                                }
                                                class="filter-chip"
                                            >
                                                <i class="ph ph-broom text-lg"></i>
                                                <span class="text-sm font-medium">{"Clear All"}</span>
                                            </button>

                                            // Completed filter chip (3-state)
                                            <button
                                                onclick={
                                                    let completed_filter_state = completed_filter_state.clone();
                                                    Callback::from(move |_| {
                                                        completed_filter_state.set(match *completed_filter_state {
                                                            CompletedFilter::ShowAll => CompletedFilter::ShowOnly,
                                                            CompletedFilter::ShowOnly => CompletedFilter::Hide,
                                                            CompletedFilter::Hide => CompletedFilter::ShowAll,
                                                        });
                                                    })
                                                }
                                                title={completed_title}
                                                class={classes!(
                                                    "filter-chip",
                                                    match *completed_filter_state {
                                                        CompletedFilter::ShowOnly => "filter-chip-active",
                                                        CompletedFilter::Hide => "filter-chip-alert",
                                                        CompletedFilter::ShowAll => ""
                                                    }
                                                )}
                                            >
                                                <i class={classes!("ph", completed_icon, "text-lg")}></i>
                                                <span class="text-sm font-medium">{completed_text}</span>
                                            </button>

                                            // In progress filter chip
                                            <button
                                                onclick={
                                                    let show_in_progress = show_in_progress.clone();
                                                    Callback::from(move |_| {
                                                        show_in_progress.set(!*show_in_progress);
                                                    })
                                                }
                                                class={classes!(
                                                    "filter-chip",
                                                    if *show_in_progress { "filter-chip-active" } else { "" }
                                                )}
                                            >
                                                <i class="ph ph-hourglass-medium text-lg"></i>
                                                <span class="text-sm font-medium">{"In Progress"}</span>
                                            </button>
                                            
                                            // Selection mode toggle
                                            <button
                                                onclick={
                                                    let is_selecting = is_selecting.clone();
                                                    let selected_episodes = selected_episodes.clone();
                                                    Callback::from(move |_| {
                                                        if *is_selecting {
                                                            // Exit selection mode and clear selections
                                                            selected_episodes.set(HashSet::new());
                                                        }
                                                        is_selecting.set(!*is_selecting);
                                                    })
                                                }
                                                class={classes!(
                                                    "filter-chip",
                                                    if *is_selecting { "filter-chip-active" } else { "" }
                                                )}
                                            >
                                                <i class="ph ph-check-square text-lg"></i>
                                                <span class="text-sm font-medium">
                                                    {if *is_selecting { "Exit Select" } else { "Select" }}
                                                </span>
                                            </button>
                                        </div>
                                        
                                        // Smart selection buttons when in selection mode
                                        {
                                            if *is_selecting {
                                                let filtered_episodes_clone = filtered_episodes.clone();
                                                let selected_episodes_clone = selected_episodes.clone();
                                                
                                                html! {
                                                    <div class="flex gap-2 mt-4 flex-wrap">
                                                        // Select All / Deselect All
<button
    onclick={
        let filtered_episodes = filtered_episodes_clone.clone();
        let selected_episodes = selected_episodes_clone.clone();
        Callback::from(move |_| {
            let all_ids: HashSet<i32> = filtered_episodes.iter()
                .filter_map(|ep| ep.episode_id)
                .collect();
            
            let current = (*selected_episodes).clone();
            if current.len() == all_ids.len() && all_ids.iter().all(|id| current.contains(id)) {
                // Deselect all
                selected_episodes.set(HashSet::new());
            } else {
                // Select all
                selected_episodes.set(all_ids);
            }
        })
    }
    class="bulk-select-button"
>
    {
        {
            // this extra block is an expression, so valid
            let all_ids: HashSet<i32> = filtered_episodes_clone.iter()
                .filter_map(|ep| ep.episode_id)
                .collect();
            let current = (*selected_episodes_clone).clone();
            if current.len() == all_ids.len() && all_ids.iter().all(|id| current.contains(id)) {
                "Deselect All"
            } else {
                "Select All"
            }
        }
    }
</button>

                                                        
                                                        // Select Unplayed Only
                                                        <button
                                                            onclick={
                                                                let filtered_episodes = filtered_episodes_clone.clone();
                                                                let selected_episodes = selected_episodes_clone.clone();
                                                                Callback::from(move |_| {
                                                                    let unplayed_ids: HashSet<i32> = filtered_episodes.iter()
                                                                        .filter(|ep| !ep.completed.unwrap_or(false))
                                                                        .filter_map(|ep| ep.episode_id)
                                                                        .collect();
                                                                    selected_episodes.set(unplayed_ids);
                                                                })
                                                            }
                                                            class="bulk-filter-button"
                                                        >
                                                            {"Select Unplayed"}
                                                        </button>
                                                    </div>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                }
                            }
                            
                            
                            // Bulk action toolbar
                            {
                                if *is_selecting && !selected_episodes.is_empty() {
                                    let selected_count = selected_episodes.len();
                                    let selected_ids: Vec<i32> = selected_episodes.iter().cloned().collect();
                                    let user_id_value = user_id.unwrap_or(0);
                                    
                                    html! {
                                        <div class="bg-blue-50 border border-blue-200 rounded-lg p-4 mb-4">
                                            <div class="flex items-center justify-between">
                                                <div class="text-sm text-blue-800 flex items-center">
                                                    <i class="ph ph-check-circle text-lg mr-2"></i>
                                                    {format!("{} episode{} selected", selected_count, if selected_count == 1 { "" } else { "s" })}
                                                </div>
                                                <div class="flex gap-2 flex-wrap">
                                                    // Mark Complete button
                                                    <button
                                                        onclick={
                                                            let selected_ids = selected_ids.clone();
                                                            let api_key = api_key.clone();
                                                            let server_name = server_name.clone();
                                                            let dispatch = _search_dispatch.clone();
                                                            let selected_episodes = selected_episodes.clone();
                                                            Callback::from(move |_| {
                                                                let selected_ids = selected_ids.clone();
                                                                let api_key = api_key.clone();
                                                                let server_name = server_name.clone();
                                                                let dispatch = dispatch.clone();
                                                                let selected_episodes = selected_episodes.clone();
                                                                spawn_local(async move {
                                                                    let request = BulkEpisodeActionRequest {
                                                                        episode_ids: selected_ids,
                                                                        user_id: user_id_value,
                                                                        is_youtube: None,
                                                                    };
                                                                    match call_bulk_mark_episodes_completed(
                                                                        &server_name.unwrap_or_default(),
                                                                        &api_key.flatten(),
                                                                        &request
                                                                    ).await {
                                                                        Ok(message) => {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.info_message = Some(message);
                                                                            });
                                                                            selected_episodes.set(HashSet::new());
                                                                        }
                                                                        Err(e) => {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.error_message = Some(format!("Error: {}", e));
                                                                            });
                                                                        }
                                                                    }
                                                                });
                                                            })
                                                        }
                                                        class="bulk-action-success"
                                                    >
                                                        {"Mark Complete"}
                                                    </button>
                                                    
                                                    // Save button
                                                    <button
                                                        onclick={
                                                            let selected_ids = selected_ids.clone();
                                                            let api_key = api_key.clone();
                                                            let server_name = server_name.clone();
                                                            let dispatch = _search_dispatch.clone();
                                                            let selected_episodes = selected_episodes.clone();
                                                            Callback::from(move |_| {
                                                                let selected_ids = selected_ids.clone();
                                                                let api_key = api_key.clone();
                                                                let server_name = server_name.clone();
                                                                let dispatch = dispatch.clone();
                                                                let selected_episodes = selected_episodes.clone();
                                                                spawn_local(async move {
                                                                    let request = BulkEpisodeActionRequest {
                                                                        episode_ids: selected_ids,
                                                                        user_id: user_id_value,
                                                                        is_youtube: None,
                                                                    };
                                                                    match call_bulk_save_episodes(
                                                                        &server_name.unwrap_or_default(),
                                                                        &api_key.flatten(),
                                                                        &request
                                                                    ).await {
                                                                        Ok(message) => {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.info_message = Some(message);
                                                                            });
                                                                            selected_episodes.set(HashSet::new());
                                                                        }
                                                                        Err(e) => {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.error_message = Some(format!("Error: {}", e));
                                                                            });
                                                                        }
                                                                    }
                                                                });
                                                            })
                                                        }
                                                        class="bulk-action-primary"
                                                    >
                                                        {"Save"}
                                                    </button>
                                                    
                                                    // Queue button
                                                    <button
                                                        onclick={
                                                            let selected_ids = selected_ids.clone();
                                                            let api_key = api_key.clone();
                                                            let server_name = server_name.clone();
                                                            let dispatch = _search_dispatch.clone();
                                                            let selected_episodes = selected_episodes.clone();
                                                            Callback::from(move |_| {
                                                                let selected_ids = selected_ids.clone();
                                                                let api_key = api_key.clone();
                                                                let server_name = server_name.clone();
                                                                let dispatch = dispatch.clone();
                                                                let selected_episodes = selected_episodes.clone();
                                                                spawn_local(async move {
                                                                    let request = BulkEpisodeActionRequest {
                                                                        episode_ids: selected_ids,
                                                                        user_id: user_id_value,
                                                                        is_youtube: None,
                                                                    };
                                                                    match call_bulk_queue_episodes(
                                                                        &server_name.unwrap_or_default(),
                                                                        &api_key.flatten(),
                                                                        &request
                                                                    ).await {
                                                                        Ok(message) => {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.info_message = Some(message);
                                                                            });
                                                                            selected_episodes.set(HashSet::new());
                                                                        }
                                                                        Err(e) => {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.error_message = Some(format!("Error: {}", e));
                                                                            });
                                                                        }
                                                                    }
                                                                });
                                                            })
                                                        }
                                                        class="px-3 py-1 text-xs bg-purple-600 hover:bg-purple-700 text-white rounded-md"
                                                    >
                                                        {"Queue"}
                                                    </button>
                                                    
                                                    // Download button
                                                    <button
                                                        onclick={
                                                            let selected_ids = selected_ids.clone();
                                                            let api_key = api_key.clone();
                                                            let server_name = server_name.clone();
                                                            let dispatch = _search_dispatch.clone();
                                                            let selected_episodes = selected_episodes.clone();
                                                            Callback::from(move |_| {
                                                                let selected_ids = selected_ids.clone();
                                                                let api_key = api_key.clone();
                                                                let server_name = server_name.clone();
                                                                let dispatch = dispatch.clone();
                                                                let selected_episodes = selected_episodes.clone();
                                                                spawn_local(async move {
                                                                    let request = BulkEpisodeActionRequest {
                                                                        episode_ids: selected_ids,
                                                                        user_id: user_id_value,
                                                                        is_youtube: None,
                                                                    };
                                                                    match call_bulk_download_episodes(
                                                                        &server_name.unwrap_or_default(),
                                                                        &api_key.flatten(),
                                                                        &request
                                                                    ).await {
                                                                        Ok(message) => {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.info_message = Some(message);
                                                                            });
                                                                            selected_episodes.set(HashSet::new());
                                                                        }
                                                                        Err(e) => {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.error_message = Some(format!("Error: {}", e));
                                                                            });
                                                                        }
                                                                    }
                                                                });
                                                            })
                                                        }
                                                        class="px-3 py-1 text-xs bg-orange-600 hover:bg-orange-700 text-white rounded-md"
                                                    >
                                                        {"Download"}
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            
                            {
                                if let (Some(_), Some(podcast_info)) = (podcast_feed_results, &clicked_podcast_info) {
                                    let podcast_link_clone = podcast_info.feedurl.clone();
                                    let podcast_title = podcast_info.podcastname.clone();
                                    
                                    // Episode selection callback
                                    let selected_episodes_clone = selected_episodes.clone();
                                    let on_episode_select = Callback::from(move |(episode_id, is_selected): (i32, bool)| {
                                        selected_episodes_clone.set({
                                            let mut current = (*selected_episodes_clone).clone();
                                            if is_selected {
                                                current.insert(episode_id);
                                            } else {
                                                current.remove(&episode_id);
                                            }
                                            current
                                        });
                                    });

                                    // Select all older episodes callback
                                    let filtered_episodes_older = filtered_episodes.clone();
                                    let selected_episodes_older = selected_episodes.clone();
                                    let on_select_older = Callback::from(move |cutoff_episode_id: i32| {
                                        let cutoff_index = filtered_episodes_older.iter()
                                            .position(|ep| ep.episode_id == Some(cutoff_episode_id))
                                            .unwrap_or(0);
                                        
                                        let older_ids: HashSet<i32> = filtered_episodes_older.iter()
                                            .skip(cutoff_index) // Include the cutoff episode and all after it (older in reverse chronological order)
                                            .filter_map(|ep| ep.episode_id)
                                            .collect();
                                        
                                        selected_episodes_older.set({
                                            let mut current = (*selected_episodes_older).clone();
                                            current.extend(older_ids);
                                            current
                                        });
                                    });

                                    // Select all newer episodes callback
                                    let filtered_episodes_newer = filtered_episodes.clone();
                                    let selected_episodes_newer = selected_episodes.clone();
                                    let on_select_newer = Callback::from(move |cutoff_episode_id: i32| {
                                        let cutoff_index = filtered_episodes_newer.iter()
                                            .position(|ep| ep.episode_id == Some(cutoff_episode_id))
                                            .unwrap_or(0);
                                        
                                        let newer_ids: HashSet<i32> = filtered_episodes_newer.iter()
                                            .take(cutoff_index + 1) // Include episodes before the cutoff (newer in reverse chronological order)
                                            .filter_map(|ep| ep.episode_id)
                                            .collect();
                                        
                                        selected_episodes_newer.set({
                                            let mut current = (*selected_episodes_newer).clone();
                                            current.extend(newer_ids);
                                            current
                                        });
                                    });

                                    html! {
                                        <PodcastEpisodeVirtualList
                                            episodes={(*filtered_episodes).clone()}
                                            item_height={220.0} // Adjust this based on your actual episode item height
                                            podcast_added={podcast_added}
                                            search_state={search_state.clone()}
                                            search_ui_state={state.clone()}
                                            dispatch={_dispatch.clone()}
                                            search_dispatch={_search_dispatch.clone()}
                                            history={history.clone()}
                                            server_name={server_name.clone()}
                                            user_id={user_id}
                                            api_key={api_key.clone()}
                                            podcast_link={podcast_link_clone}
                                            podcast_title={podcast_title}
                                            selected_episodes={Some(Rc::new((*selected_episodes).clone()))}
                                            is_selecting={Some(*is_selecting)}
                                            on_episode_select={Some(on_episode_select)}
                                            on_select_older={Some(on_select_older)}
                                            on_select_newer={Some(on_select_newer)}
                                        />
                                    }

                                } else {
                                    html! {
                                        <div class="empty-episodes-container" id="episode-container">
                                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                            <h1 class="page-subtitles">{ "No Episodes Found" }</h1>
                                            <p class="page-paragraphs">{"This podcast strangely doesn't have any episodes. Try a more mainstream one maybe?"}</p>
                                        </div>
                                    }
                                }
                            }
                        </>
                    }
                }
            }
        <App_drawer />
        {
            if let Some(audio_props) = &state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
            } else {
                html! {}
            }
        }
        </div>

    }
}
