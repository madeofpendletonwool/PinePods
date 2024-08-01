use super::app_drawer::App_drawer;
use super::gen_components::ContextButton;
use super::gen_components::{on_shownotes_click, EpisodeTrait, Search_nav, UseScrollToTop};
use super::gen_funcs::{format_datetime, match_date_format, parse_date};
use crate::components::audio::{on_play_click, AudioPlayer};
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, UIState};
use crate::components::gen_funcs::format_time;
use crate::components::gen_funcs::{
    convert_time_to_seconds, sanitize_html_with_blank_target, truncate_description,
};
use crate::components::podcast_layout::ClickedFeedURL;
use crate::requests::login_requests::use_check_authentication;
use crate::requests::pod_req::{
    call_add_podcast, call_adjust_skip_times, call_check_podcast, call_download_all_podcast,
    call_enable_auto_download, call_fetch_podcasting_2_pod_data, call_get_auto_download_status,
    call_get_auto_skip_times, call_get_podcast_id_from_ep, call_get_podcast_id_from_ep_name,
    call_remove_podcasts_name, AutoDownloadRequest, DownloadAllPodcastRequest,
    FetchPodcasting2PodDataRequest, Person, PodcastValues, RemovePodcastValuesName,
    SkipTimesRequest,
};
use crate::requests::search_pods::call_get_podcast_details_dynamic;
use htmlentity::entity::decode;
use htmlentity::entity::ICodedDataTrait;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Event, HtmlInputElement, MouseEvent, UrlSearchParams};
use yew::prelude::*;
use yew::Properties;
use yew::{
    function_component, html, use_effect, use_effect_with, use_node_ref, Callback, Html, TargetCast,
};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

fn add_icon() -> Html {
    html! {
        <span class="material-icons">{ "add_box" }</span>
    }
}

fn payments_icon() -> Html {
    html! {
        <span class="material-icons">{ "payments" }</span>
    }
}

fn website_icon() -> Html {
    html! {
        <span class="material-icons">{ "language" }</span>
    }
}

fn trash_icon() -> Html {
    html! {
        <span class="material-icons">{ "delete" }</span>

    }
}
fn settings_icon() -> Html {
    html! {
        <span class="material-icons">{ "more_vert" }</span>

    }
}
fn download_icon() -> Html {
    html! {
        <span class="material-icons">{ "download_for_offline" }</span>

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

#[function_component(SafeHtml)]
pub fn safe_html(props: &Props) -> Html {
    let div = gloo_utils::document().create_element("div").unwrap();
    div.set_inner_html(&props.html.clone());

    Html::VRef(div.into())
}

fn sanitize_html(html: &str) -> String {
    let cleaned_html = ammonia::clean(html);
    let decoded_data = decode(cleaned_html.as_bytes());
    match decoded_data.to_string() {
        Ok(decoded_html) => decoded_html,
        Err(_) => String::from("Invalid HTML content"),
    }
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

pub enum UIStateMsg {
    ClearErrorMessage,
    ClearInfoMessage,
}

impl Reducer<UIState> for UIStateMsg {
    fn apply(self, mut state: Rc<UIState>) -> Rc<UIState> {
        let state = Rc::make_mut(&mut state);

        match self {
            UIStateMsg::ClearErrorMessage => {
                state.error_message = None;
            }
            UIStateMsg::ClearInfoMessage => {
                state.info_message = None;
            }
        }

        (*state).clone().into()
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct HostDropdownProps {
    pub title: String,
    pub hosts: Vec<Person>,
}

#[function_component(HostDropdown)]
pub fn host_dropdown(HostDropdownProps { title, hosts }: &HostDropdownProps) -> Html {
    let is_open = use_state(|| false);
    let toggle = {
        let is_open = is_open.clone();
        Callback::from(move |_| is_open.set(!*is_open))
    };

    let arrow_rotation_class = if *is_open { "rotate-180" } else { "rotate-0" };

    html! {
        <div class="inline-block">
            <button
                class="flex items-center text-gray-700 dark:text-gray-300 focus:outline-none"
                onclick={toggle}
            >
                <span class="header-text">{ title }</span>
                <svg
                    class={format!("w-3 h-3 transition-transform duration-300 accordion-arrow hosts-arrow {}", arrow_rotation_class)}
                    xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6"
                >
                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5 5 1 1 5"/>
                </svg>
            </button>
            if *is_open {
                <div class="flex space-x-4 mt-2">
                    { for hosts.iter().map(|host| html! {
                        <div class="flex flex-col items-center">
                            { if let Some(img) = &host.img {
                                html! { <img src={img.clone()} alt={host.name.clone()} class="w-12 h-12 rounded-full" /> }
                            } else {
                                html! {}
                            }}
                            <a href={host.href.clone().unwrap_or_default()} class="text-center text-blue-500 hover:underline mt-1">{ &host.name }</a>
                        </div>
                    })}
                </div>
            }
        </div>
    }
}

#[function_component(EpisodeLayout)]
pub fn episode_layout() -> Html {
    let is_added = use_state(|| false);
    let (state, _dispatch) = use_store::<UIState>();
    let (search_state, _search_dispatch) = use_store::<AppState>();
    let podcast_feed_results = search_state.podcast_feed_results.clone();
    let clicked_podcast_info = search_state.clicked_podcast_info.clone();
    let loading = use_state(|| true);

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

    let session_dispatch = _search_dispatch.clone();
    let session_state = search_state.clone();
    let podcast_added = search_state.podcast_added.unwrap_or_default();
    let pod_url = use_state(|| String::new());

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

                    let update_url_with_params = |title: &str, url: &str| {
                        let window = web_sys::window().expect("no global window exists");
                        let history = window.history().expect("should have a history");
                        let location = window.location();

                        let mut new_url = location.origin().unwrap();
                        new_url.push_str(&location.pathname().unwrap());
                        new_url.push_str("?podcast_title=");
                        new_url.push_str(&urlencoding::encode(title));
                        new_url.push_str("&podcast_url=");
                        new_url.push_str(&urlencoding::encode(url));

                        history
                            .push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&new_url))
                            .expect("should push state");
                    };

                    if podcast.is_none() {
                        let window = web_sys::window().expect("no global window exists");
                        let search_params = window.location().search().unwrap();
                        let url_params = UrlSearchParams::new_with_str(&search_params).unwrap();

                        let podcast_title = url_params.get("podcast_title").unwrap_or_default();
                        let podcast_url = url_params.get("podcast_url").unwrap_or_default();

                        if !podcast_title.is_empty() && !podcast_url.is_empty() {
                            let podcast_info = ClickedFeedURL {
                                podcast_title: podcast_title.clone(),
                                podcast_url: podcast_url.clone(),
                                podcast_description: String::new(),
                                podcast_author: String::new(),
                                podcast_artwork: String::new(),
                                podcast_explicit: false,
                                podcast_episode_count: 0,
                                podcast_categories: None,
                                podcast_link: String::new(),
                            };

                            let api_key = api_key.clone();
                            let user_id = user_id.clone();
                            let server_name = server_name.clone();
                            spawn_local(async move {
                                let added = call_check_podcast(
                                    &server_name,
                                    &api_key.clone().unwrap(),
                                    user_id,
                                    podcast_info.podcast_title.as_str(),
                                    podcast_info.podcast_url.as_str(),
                                )
                                .await
                                .unwrap_or_default()
                                .exists;
                                is_added.set(added);

                                let podcast_details = call_get_podcast_details_dynamic(
                                    &server_name,
                                    &api_key.clone().unwrap(),
                                    user_id,
                                    podcast_info.podcast_title.as_str(),
                                    podcast_info.podcast_url.as_str(),
                                    added,
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
                                    categories_to_string(podcast_details.podcast_categories);

                                // Execute the same process as when a podcast is clicked
                                let on_title_click = create_on_title_click(
                                    click_dispatch,
                                    server_name,
                                    Some(Some(api_key.clone().unwrap())),
                                    &click_history,
                                    podcast_details.podcast_title,
                                    podcast_details.podcast_url,
                                    podcast_details.podcast_description,
                                    podcast_details.podcast_author,
                                    podcast_details.podcast_artwork,
                                    podcast_details.podcast_explicit,
                                    podcast_details.podcast_episode_count,
                                    podcast_categories_str, // assuming no categories in local storage
                                    podcast_details.podcast_link,
                                    user_id,
                                );
                                emit_click(on_title_click);
                                let window = web_sys::window().expect("no global window exists");
                                let history = window.history().expect("should have a history");
                                let location = window.location();

                                let mut new_url = location.origin().unwrap();
                                new_url.push_str(&location.pathname().unwrap());
                                new_url.push_str("?podcast_title=");
                                new_url.push_str(&urlencoding::encode(&podcast_info.podcast_title));
                                new_url.push_str("&podcast_url=");
                                new_url.push_str(&urlencoding::encode(&podcast_info.podcast_url));
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
                        new_url.push_str(&urlencoding::encode(&podcast.podcast_title));
                        new_url.push_str("&podcast_url=");
                        new_url.push_str(&urlencoding::encode(&podcast.podcast_url));

                        history
                            .push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&new_url))
                            .expect("should push state");

                        let api_key = api_key.clone();
                        let user_id = user_id.clone();
                        let server_name = server_name.clone();
                        spawn_local(async move {
                            let added = call_check_podcast(
                                &server_name,
                                &api_key.unwrap(),
                                user_id,
                                podcast.podcast_title.as_str(),
                                podcast.podcast_url.as_str(),
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
                new_url.push_str(&urlencoding::encode(&info.podcast_title));
                new_url.push_str("&podcast_url=");
                new_url.push_str(&urlencoding::encode(&info.podcast_url));
                pod_url.set(new_url.clone());
                load_link.set(false);

                history
                    .push_state_with_url(&JsValue::NULL, "", Some(&new_url))
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
        // let episode_name = episode_name_pre.clone();
        // let episode_url = episode_url_pre.clone();
        let user_id = search_state.user_details.as_ref().map(|ud| ud.UserID);
        let effect_start_skip = start_skip.clone();
        let effect_end_skip = end_skip.clone();
        let effect_added = is_added.clone();
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
                                        match call_get_auto_skip_times(
                                            &server_name,
                                            &Some(api_key.clone().unwrap()),
                                            user_id,
                                            id,
                                        )
                                        .await
                                        {
                                            Ok((start, end)) => {
                                                effect_start_skip.set(start);
                                                effect_end_skip.set(end);
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
                                                    &format!("Error fetching chapters: {}", e)
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

    {
        let dispatch = _dispatch.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                dispatch.apply(UIStateMsg::ClearErrorMessage);
                dispatch.apply(UIStateMsg::ClearInfoMessage);
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

    let delete_all_click = {
        let add_dispatch = _dispatch.clone();
        let pod_values = clicked_podcast_info.clone();

        let user_id_og = user_id.clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let app_dispatch = _search_dispatch.clone();
        let call_is_added = is_added.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let pod_title_og = pod_values.clone().unwrap().podcast_title.clone();
            let pod_feed_url_og = pod_values.clone().unwrap().podcast_url.clone();
            app_dispatch.reduce_mut(|state| state.is_loading = Some(true));
            let is_added_inner = call_is_added.clone();
            let call_dispatch = add_dispatch.clone();
            let pod_title = pod_title_og.clone();
            let pod_feed_url = pod_feed_url_og.clone();
            let user_id = user_id_og.clone().unwrap();
            let podcast_values = RemovePodcastValuesName {
                podcast_name: pod_title,
                podcast_url: pod_feed_url,
                user_id: user_id,
            };
            let api_key_call = api_key_clone.clone();
            let server_name_call = server_name_clone.clone();
            let app_dispatch = app_dispatch.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let dispatch_wasm = call_dispatch.clone();
                let api_key_wasm = api_key_call.clone().unwrap();
                let server_name_wasm = server_name_call.clone();
                let pod_values_clone = podcast_values.clone(); // Make sure you clone the podcast values
                match call_remove_podcasts_name(
                    &server_name_wasm.unwrap(),
                    &api_key_wasm,
                    &pod_values_clone,
                )
                .await
                {
                    Ok(success) => {
                        if success {
                            dispatch_wasm.reduce_mut(|state| {
                                state.info_message =
                                    Option::from("Podcast successfully removed".to_string())
                            });
                            app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                            is_added_inner.set(false);
                        } else {
                            dispatch_wasm.reduce_mut(|state| {
                                state.error_message =
                                    Option::from("Failed to remove podcast".to_string())
                            });
                            app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                        }
                    }
                    Err(e) => {
                        dispatch_wasm.reduce_mut(|state| {
                            state.error_message =
                                Option::from(format!("Error removing podcast: {:?}", e))
                        });
                        app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                    }
                }
            });
        })
    };

    let download_server_name = server_name.clone();
    let download_api_key = api_key.clone();
    let download_dispatch = _dispatch.clone();
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
                let ep_api_key = api_key.clone();
                let ep_server_name = server_name.clone();
                let ep_user_id = user_id_copy.clone();
                match call_get_podcast_id_from_ep(
                    &ep_server_name.unwrap(),
                    &ep_api_key.unwrap(),
                    episode_id,
                    ep_user_id.unwrap(),
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
                                call_down_dispatch.reduce_mut(|state| {
                                    state.error_message = Option::from(format!("{}", e))
                                });
                            }
                        }
                    }
                    Err(e) => {
                        call_down_dispatch.reduce_mut(|state| {
                            state.error_message =
                                Option::from(format!("Failed to get podcast ID: {}", e))
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

    let website_icon = { website_icon() };

    let page_state = use_state(|| PageState::Hidden);

    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
        })
    };

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

    let start_skip_call = start_skip.clone();
    let end_skip_call = end_skip.clone();
    let start_skip_call_button = start_skip.clone();
    let end_skip_call_button = end_skip.clone();
    let skip_dispatch = _dispatch.clone();

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

    // Define the modal components
    let podcast_option_model = html! {
        <div id="podcast_option_model" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow">
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
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define the modal components
    let download_all_model = html! {
        <div id="download_all_modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow">
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
        <div id="delete_pod_model" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow">
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
        let add_dispatch = _dispatch.clone();
        let pod_values = clicked_podcast_info.clone();
        let user_id_og = user_id.clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let user_id_clone = user_id.clone();
        let app_dispatch = _search_dispatch.clone();

        let is_added = is_added.clone();

        if *is_added == true {
            toggle_delete
        } else {
            Callback::from(move |_: MouseEvent| {
                // Ensure this is triggered only by a MouseEvent
                let pod_title_og = pod_values.clone().unwrap().podcast_title.clone();
                let pod_artwork_og = pod_values.clone().unwrap().podcast_artwork.clone();
                let pod_author_og = pod_values.clone().unwrap().podcast_author.clone();
                let categories_og = pod_values
                    .clone()
                    .unwrap()
                    .podcast_categories
                    .unwrap()
                    .clone();
                let pod_description_og = pod_values.clone().unwrap().podcast_description.clone();
                let pod_episode_count_og =
                    pod_values.clone().unwrap().podcast_episode_count.clone();
                let pod_feed_url_og = pod_values.clone().unwrap().podcast_url.clone();
                let pod_website_og = pod_values.clone().unwrap().podcast_link.clone();
                let pod_explicit_og = pod_values.clone().unwrap().podcast_explicit.clone();
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
                        &server_name_wasm.unwrap(),
                        &api_key_wasm,
                        user_id_wasm,
                        &pod_values_clone,
                    )
                    .await
                    {
                        Ok(success) => {
                            if success {
                                dispatch_wasm.reduce_mut(|state| {
                                    state.info_message =
                                        Option::from("Podcast successfully added".to_string())
                                });
                                app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                                is_added_inner.set(true);
                            } else {
                                dispatch_wasm.reduce_mut(|state| {
                                    state.error_message =
                                        Option::from("Failed to add podcast".to_string())
                                });
                                app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                            }
                        }
                        Err(e) => {
                            dispatch_wasm.reduce_mut(|state| {
                                state.error_message =
                                    Option::from(format!("Error adding podcast: {:?}", e))
                            });
                            app_dispatch.reduce_mut(|state| state.is_loading = Some(false));
                        }
                    }
                });
            })
        }
    };

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggle_description(guid: &str);
    }

    let web_link = open_in_new_tab.clone();
    let pod_layout_data = clicked_podcast_info.clone();
    html! {
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                match *page_state {
                PageState::Shown => podcast_option_model,
                PageState::Download => download_all_model,
                PageState::Delete => delete_pod_model,
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
                                    let sanitized_title = podcast_info.podcast_title.replace(|c: char| !c.is_alphanumeric(), "-");
                                    let desc_id = format!("desc-{}", sanitized_title);

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
                                    let sanitized_description = sanitize_html(&podcast_info.podcast_description);
                                    let layout = if state.is_mobile.unwrap_or(false) {
                                        html! {
                                            <div class="mobile-layout">
                                                <div class="button-container">
                                                    <button onclick={Callback::from(move |_| web_link.emit(podcast_info.podcast_link.clone()))}
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
                                                    <button onclick={toggle_download} title="Click to download all episodes for this podcast" class="item-container-button font-bold rounded-full self-center mr-4">
                                                        { download_all }
                                                    </button>
                                                    <button onclick={toggle_podcast} title="Click to add or remove podcast from feed" class="item-container-button font-bold rounded-full self-center mr-4">
                                                        { button_content }
                                                    </button>
                                                    <button onclick={toggle_settings} title="Click to setup podcast specific settings" class="item-container-button font-bold rounded-full self-center mr-4">
                                                        { setting_content }
                                                    </button>
                                                </div>
                                                <div class="item-header-mobile-cover-container">
                                                    <img src={podcast_info.podcast_artwork.clone()} alt={format!("Cover for {}", &podcast_info.podcast_title)} class="item-header-mobile-cover"/>
                                                </div>

                                                <h2 class="item-header-title">{ &podcast_info.podcast_title }</h2>
                                                <div class="item-header-description desc-collapsed" id={desc_id.clone()} onclick={toggle_description.clone()}>
                                                    { sanitized_description }
                                                    <button class="toggle-desc-btn" onclick={toggle_description}>{ "" }</button>
                                                </div>
                                                <p class="header-info">{ format!("Episode Count: {}", &podcast_info.podcast_episode_count) }</p>
                                                <p class="header-info">{ format!("Authors: {}", &podcast_info.podcast_author) }</p>
                                                <p class="header-info">{ format!("Explicit: {}", if podcast_info.podcast_explicit { "Yes" } else { "No" }) }</p>
                                                {
                                                    if let Some(people) = &state.podcast_people {
                                                        if !people.is_empty() {
                                                            html! {
                                                                <div class="header-info">
                                                                    <HostDropdown title="Hosts" hosts={people.clone()} />
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
                                                <div class="categories-container">
                                                {
                                                    if let Some(categories) = &podcast_info.podcast_categories {
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
                                        html! {
                                            <div class="item-header">
                                                <img src={podcast_info.podcast_artwork.clone()} alt={format!("Cover for {}", &podcast_info.podcast_title)} class="item-header-cover"/>
                                                <div class="item-header-info">
                                                    <div class="title-button-container">
                                                        <h2 class="item-header-title">{ &podcast_info.podcast_title }</h2>
                                                        <button onclick={toggle_download} title="Click to download all episodes for this podcast" class={"item-container-button selector-button font-bold py-2 px-4 rounded-full self-center mr-8"} style="width: 60px; height: 60px;">
                                                            { download_all }
                                                        </button>
                                                        <button onclick={toggle_podcast} title="Click to add or remove podcast from feed" class={"item-container-button selector-button font-bold py-2 px-4 rounded-full self-center mr-8"} style="width: 60px; height: 60px;">
                                                            { button_content }
                                                        </button>
                                                        <button onclick={toggle_settings} title="Click to setup podcast specific settings" class={"item-container-button selector-button font-bold py-2 px-4 rounded-full self-center mr-8"} style="width: 60px; height: 60px;">
                                                            { setting_content }
                                                        </button>
                                                    </div>

                                                    // <p class="item-header-description">{ &podcast_info.podcast_description }</p>
                                                    <div class="item-header-description desc-collapsed" id={desc_id.clone()} onclick={toggle_description.clone()}>
                                                        { sanitized_description }
                                                        <button class="toggle-desc-btn" onclick={toggle_description}>{ "" }</button>
                                                    </div>
                                                    <button
                                                        onclick={Callback::from(move |_| web_link.clone().emit(podcast_info.podcast_link.to_string()))}
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
                                                                                class="item-container-button font-bold rounded-full self-center mr-8"
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
                                                    <div class="item-header-info">

                                                        <p class="header-text">{ format!("Episode Count: {}", &podcast_info.podcast_episode_count) }</p>
                                                        <p class="header-text">{ format!("Authors: {}", &podcast_info.podcast_author) }</p>
                                                        <p class="header-text">{ format!("Explicit: {}", if podcast_info.podcast_explicit { "Yes" } else { "No" }) }</p>
                                                        {
                                                            if let Some(people) = &state.podcast_people {
                                                                if !people.is_empty() {
                                                                    html! {
                                                                        <HostDropdown title="Hosts" hosts={people.clone()} />
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
                                                                if let Some(categories) = &podcast_info.podcast_categories {
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
                                if let Some(results) = podcast_feed_results {
                                    let podcast_link_clone = clicked_podcast_info.clone().unwrap().podcast_url.clone();
                                    let podcast_title = clicked_podcast_info.clone().unwrap().podcast_title.clone();
                                    html! {
                                        <div>
                                            { for results.episodes.iter().map(|episode| {
                                                let history_clone = history.clone();
                                                let dispatch = _dispatch.clone();
                                                let search_dispatch = _search_dispatch.clone();
                                                let search_state_clone = search_state.clone(); // Clone search_state

                                                // Clone the variables outside the closure
                                                let podcast_link_clone = podcast_link_clone.clone();
                                                let podcast_title = podcast_title.clone();
                                                let episode_url_clone = episode.enclosure_url.clone().unwrap_or_default();
                                                let episode_title_clone = episode.title.clone().unwrap_or_default();
                                                let episode_artwork_clone = episode.artwork.clone().unwrap_or_default();
                                                // let episode_duration_clone = episode.duration.clone().unwrap_or_default();
                                                let episode_duration_clone = episode.duration.clone().unwrap_or_default();
                                                let episode_duration_in_seconds = match convert_time_to_seconds(&episode_duration_clone) {
                                                    Ok(seconds) => seconds as i32,
                                                    Err(e) => {
                                                        eprintln!("Failed to convert time to seconds: {}", e);
                                                        0
                                                    }
                                                };
                                                let episode_id_clone = episode.episode_id.unwrap_or(0);
                                                let mut db_added = false;
                                                if episode_id_clone == 0 {

                                                } else {
                                                    db_added = true;
                                                }
                                                let episode_id_shownotes = episode_id_clone.clone();
                                                let server_name_play = server_name.clone();
                                                let user_id_play = user_id.clone();
                                                let api_key_play = api_key.clone();

                                                let is_expanded = search_state.expanded_descriptions.contains(
                                                    &episode.guid.clone().unwrap()
                                                );


                                                let sanitized_description = sanitize_html_with_blank_target(&episode.description.clone().unwrap_or_default());

                                                let (description, _is_truncated) = if is_expanded {
                                                    (sanitized_description, false)
                                                } else {
                                                    truncate_description(sanitized_description, 300)
                                                };

                                                let search_state_toggle = search_state_clone.clone();
                                                let toggle_expanded = {
                                                    let search_dispatch_clone = search_dispatch.clone();
                                                    let episode_guid = episode.guid.clone().unwrap();
                                                    Callback::from(move |_: MouseEvent| {
                                                        let guid_clone = episode_guid.clone();
                                                        let search_dispatch_call = search_dispatch_clone.clone();

                                                        if search_state_toggle.expanded_descriptions.contains(&guid_clone) {
                                                            search_dispatch_call.apply(AppStateMsg::CollapseEpisode(guid_clone));
                                                        } else {
                                                            search_dispatch_call.apply(AppStateMsg::ExpandEpisode(guid_clone));
                                                        }

                                                    })
                                                };


                                                let state = state.clone();
                                                let on_play_click = on_play_click(
                                                    episode_url_clone.clone(),
                                                    episode_title_clone.clone(),
                                                    episode_artwork_clone.clone(),
                                                    episode_duration_in_seconds,
                                                    episode_id_clone.clone(),
                                                    Some(0),
                                                    api_key_play.unwrap().unwrap(),
                                                    user_id_play.unwrap(),
                                                    server_name_play.unwrap(),
                                                    dispatch.clone(),
                                                    state.clone(),
                                                    None,
                                                );

                                                let description_class = if is_expanded {
                                                    "desc-expanded".to_string()
                                                } else {
                                                    "desc-collapsed".to_string()
                                                };

                                                let date_format = match_date_format(search_state_clone.date_format.as_deref());
                                                let datetime = parse_date(&episode.pub_date.clone().unwrap_or_default(), &search_state_clone.user_tz);
                                                let format_release = format!("{}", format_datetime(&datetime, &search_state_clone.hour_preference, date_format));
                                                let boxed_episode = Box::new(episode.clone()) as Box<dyn EpisodeTrait>;
                                                let formatted_duration = format_time(episode_duration_in_seconds.into());

                                                let episode_url_for_ep_item = episode_url_clone.clone();
                                                let shownotes_episode_url = episode_url_clone.clone();
                                                let should_show_buttons = !episode_url_for_ep_item.is_empty();
                                                html! {
                                                    <div class="item-container flex items-center mb-4 shadow-md rounded-lg">
                                                        <img
                                                            src={episode.artwork.clone().unwrap_or_default()}
                                                            alt={format!("Cover for {}", &episode.title.clone().unwrap_or_default())}
                                                            class="episode-image"/>
                                                        <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                                                            <p class="item_container-text episode-title font-semibold" onclick={on_shownotes_click(history_clone.clone(), search_dispatch.clone(), Some(episode_id_shownotes), Some(podcast_link_clone), Some(shownotes_episode_url), Some(podcast_title), db_added)}>{ &episode.title.clone().unwrap_or_default() }</p>
                                                            // <p class="text-gray-600">{ &episode.description.clone().unwrap_or_default() }</p>
                                                            {
                                                                html! {
                                                                    <div class="item-description-text hidden md:block">
                                                                        <div class={format!("item_container-text episode-description-container {}", description_class)}>
                                                                            <SafeHtml html={description} />
                                                                        </div>
                                                                        <a class="link hover:underline cursor-pointer mt-4" onclick={toggle_expanded}>
                                                                            { if is_expanded { "See Less" } else { "See More" } }
                                                                        </a>
                                                                    </div>
                                                                }
                                                            }
                                                            <span class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2">
                                                                <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                                                    <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                                                                </svg>
                                                                { format_release }
                                                            </span>
                                                            {
                                                                    html! {
                                                                        <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                                                                    }
                                                            }
                                                        </div>
                                                        {
                                                            html! {
                                                                <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;"> // Add align-self: center; heren medium and larger screens
                                                                    if should_show_buttons {
                                                                        <button
                                                                            class="item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                                                            onclick={on_play_click}
                                                                        >
                                                                        <span class="material-bonus-color material-icons large-material-icons md:text-6xl text-4xl">{"play_arrow"}</span>
                                                                        </button>
                                                                        {
                                                                            if podcast_added {
                                                                                let page_type = "episode_layout".to_string();

                                                                                let context_button = html! {
                                                                                    <ContextButton episode={boxed_episode} page_type={page_type.clone()} />
                                                                                };


                                                                                context_button

                                                                            } else {
                                                                                html! {}
                                                                            }
                                                                        }
                                                                    }
                                                                </div>
                                                            }
                                                        }


                                                    </div>
                                                }
                                            })}
                                        </div>
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
        // Conditional rendering for the error banner
        {
            if state.error_message.as_ref().map_or(false, |msg| !msg.is_empty()) {
                html! { <div class="error-snackbar">{ &state.error_message }</div> }
            } else {
                html! {}
            }
        }
        {
        if state.info_message.as_ref().map_or(false, |msg| !msg.is_empty()) {
                html! { <div class="info-snackbar">{ &state.info_message }</div> }
            } else {
                html! {}
            }
        }
        {
            if let Some(audio_props) = &state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} /> }
            } else {
                html! {}
            }
        }
        </div>

    }
}
