use crate::components::app_drawer::App_drawer;
use crate::components::context_menu_button::PageType;
use crate::components::gen_components::{
    empty_message, on_shownotes_click, Search_nav, UseScrollToTop,
};
use crate::components::loading::Loading;

use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::pages::episode_layout::AppStateMsg;
use crate::requests::episode::Episode;

use crate::components::virtual_list::DragCallbacks;
use crate::requests::pod_req::QueuedEpisodesResponse;
use crate::requests::pod_req::{self};
use gloo_events::EventListener;
use gloo_utils::document;
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use web_sys::Element;
use web_sys::{window, DragEvent, HtmlElement, TouchEvent};
use yew::prelude::*;
use yew::{function_component, html, Html, UseStateHandle};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

// Add this at the top of your file
#[allow(dead_code)]
const SCROLL_THRESHOLD: f64 = 150.0; // Increased threshold for easier activation
#[allow(dead_code)]
const SCROLL_SPEED: f64 = 15.0; // Increased speed

// Helper function to calculate responsive item height including all spacing
#[allow(dead_code)]
fn calculate_item_height(window_width: f64) -> f64 {
    // Try to measure actual height from DOM first
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Ok(Some(first_item)) = document.query_selector(".item-container") {
            if let Some(element) = first_item.dyn_ref::<web_sys::HtmlElement>() {
                let rect = element.get_bounding_client_rect();
                let actual_height = rect.height();
                let margin_bottom = 16.0; // mb-4 = 1rem = 16px
                let total_height = actual_height + margin_bottom;

                web_sys::console::log_1(
                    &format!(
                        "MEASURED: width={}, container_height={}, margin={}, total={}",
                        window_width, actual_height, margin_bottom, total_height
                    )
                    .into(),
                );

                return total_height;
            }
        }
    }

    // Fallback to estimated heights if measurement fails
    if window_width <= 530.0 {
        122.0 + 16.0 // Mobile: base height + mb-4
    } else if window_width <= 768.0 {
        150.0 + 16.0 // Tablet: base height + mb-4
    } else {
        221.0 + 16.0 // Desktop: base height + mb-4
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
struct ScrollState {
    interval_id: Option<i32>,
}

#[allow(dead_code)]
fn stop_auto_scroll(interval_id: i32) {
    if let Some(window) = window() {
        window.clear_interval_with_handle(interval_id);
    }
}

#[function_component(Queue)]
pub fn queue() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();

    let loading = use_state(|| true);

    // Fetch episodes on component mount
    let loading_ep = loading.clone();
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

        // fetch_episodes(api_key.flatten(), user_id, server_name, dispatch, error, pod_req::call_get_recent_eps);

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let dispatch = effect_dispatch.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_queued_episodes(&server_name, &api_key, &user_id)
                            .await
                        {
                            Ok(mut fetched_episodes) => {
                                fetched_episodes
                                    .sort_by_key(|ep| ep.queueposition.unwrap_or(i32::MAX));

                                let completed_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();

                                dispatch.reduce_mut(move |state| {
                                    state.queued_episodes = Some(QueuedEpisodesResponse {
                                        episodes: fetched_episodes,
                                    });
                                    state.completed_episodes = Some(completed_episode_ids);
                                });

                                // Fetch local episode IDs for Tauri mode
                                #[cfg(not(feature = "server_build"))]
                                {
                                    let dispatch_local = dispatch.clone();
                                    wasm_bindgen_futures::spawn_local(async move {
                                        if let Ok(mut local_episodes) =
                                            crate::pages::downloads_tauri::fetch_local_episodes()
                                                .await
                                        {
                                            dispatch_local.reduce_mut(move |state| {
                                                state.downloaded_episodes.clear_local();
                                                for ep in local_episodes.drain(..) {
                                                    state.downloaded_episodes.push_local(ep);
                                                }
                                            });
                                        }
                                    });
                                }

                                loading_ep.set(false);
                                // web_sys::console::log_1(&format!("State after update: {:?}", state).into()); // Log state after update
                            }
                            Err(e) => {
                                error_clone.set(Some(e.to_string()));
                                loading_ep.set(false);
                            }
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
            <UseScrollToTop />
                if *loading { // If loading is true, display the loading animation
                    {
                        html! { <Loading/> }
                    }
                } else {
                    {
                        html! {
                            // Modern mobile-friendly queue page with tab-style page title
                            <div class="mb-2">
                                // Tab-style page indicator
                                <div class="page-tab-indicator">
                                    <i class="ph ph-queue tab-icon"></i>
                                    {&i18n.t("queue.queue")}
                                </div>
                            </div>
                        }
                    }

                    {
                        if let Some(queued_eps) = state.queued_episodes.clone() {
                            if queued_eps.episodes.is_empty() {
                                // Render "No Queued Episodes Found" if episodes list is empty
                                empty_message(
                                    &i18n.t("queue.no_queued_episodes_found"),
                                    &i18n.t("queue.queue_episodes_instructions")
                                )
                            } else {
                                html! {
                                    <VirtualQueueList
                                        episodes={queued_eps.episodes.clone()}
                                    />
                                }
                            }
                        } else {
                            empty_message(
                                &i18n.t("queue.no_queued_episodes_found_state_none"),
                                &i18n.t("queue.queue_episodes_instructions")
                            )
                        }
                }
            }
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! {
                    <AudioPlayer
                        episode={audio_props.episode.clone()}
                        src={audio_props.src.clone()}
                        title={audio_props.title.clone()}
                        description={audio_props.description.clone()}
                        release_date={audio_props.release_date.clone()}
                        artwork_url={audio_props.artwork_url.clone()}
                        duration={audio_props.duration.clone()}
                        episode_id={audio_props.episode_id.clone()}
                        duration_sec={audio_props.duration_sec.clone()}
                        start_pos_sec={audio_props.start_pos_sec.clone()}
                        end_pos_sec={audio_props.end_pos_sec.clone()}
                        offline={audio_props.offline.clone()}
                        is_youtube={audio_props.is_youtube.clone()}
                        is_video={audio_props.is_video.clone()}
                    />
                }
            } else {
                html! {}
            }
        }
        </div>
        <App_drawer />
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct VirtualQueueListProps {
    pub episodes: Vec<Episode>,
}

#[function_component(VirtualQueueList)]
pub fn virtual_queue_list(props: &VirtualQueueListProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let history = BrowserHistory::new();

    let dragging_state = use_state(|| None::<i32>);
    let is_dragging = use_state(|| false);

    let ondragstart = {
        let dragging = dragging_state.clone();
        Callback::from(move |e: DragEvent| {
            web_sys::console::log_1(&"Desktop ondragstart triggered".into());
            let target = e.target().unwrap();
            let id = target
                .dyn_ref::<HtmlElement>()
                .unwrap()
                .get_attribute("data-id")
                .unwrap();
            let parsed_id = id.parse::<i32>().unwrap();
            web_sys::console::log_1(&format!("Setting dragging state to: {}", parsed_id).into());
            dragging.set(Some(parsed_id));
            e.data_transfer()
                .unwrap()
                .set_data("text/plain", &id)
                .unwrap();
            e.data_transfer().unwrap().set_effect_allowed("move");
        })
    };

    let ondragenter = Callback::from(|e: DragEvent| {
        e.prevent_default();
        e.data_transfer().unwrap().set_drop_effect("move");
    });

    let ondragover = Callback::from(move |e: DragEvent| {
        e.prevent_default();
        let y = e.client_y();
        let scroll_speed = 20.0;

        // Find the virtual list container to scroll it instead of the window
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Ok(Some(container)) = document.query_selector(".virtual-list-container") {
                if let Some(container_element) = container.dyn_ref::<web_sys::HtmlElement>() {
                    let container_rect = container_element.get_bounding_client_rect();
                    let container_top = container_rect.top();
                    let container_bottom = container_rect.bottom();

                    // Scroll up if cursor is near the top of the container
                    if (y as f64) < container_top + 50.0 {
                        container_element
                            .set_scroll_top((container_element.scroll_top() - scroll_speed as i32).max(0));
                    }

                    // Scroll down if cursor is near the bottom of the container
                    if (y as f64) > container_bottom - 50.0 {
                        container_element
                            .set_scroll_top(container_element.scroll_top() + scroll_speed as i32);
                    }
                }
            }
        }
    });

    let ondrop = {
        let dragging = dragging_state.clone();
        let dispatch = dispatch.clone();
        let all_episodes = props.episodes.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default();

            web_sys::console::log_1(&"Desktop ondrop triggered".into());

            if let Some(dragged_id) = *dragging {
                web_sys::console::log_1(&format!("Dragged ID: {}", dragged_id).into());

                let mut target_index = None;

                // First try to find a visible target element
                let mut target = e.target().unwrap().dyn_into::<web_sys::Element>().unwrap();
                let mut attempts = 0;
                while !target.has_attribute("data-id")
                    && target.parent_element().is_some()
                    && attempts < 10
                {
                    target = target.parent_element().unwrap();
                    attempts += 1;
                }

                if let Some(target_id_str) = target.get_attribute("data-id") {
                    web_sys::console::log_1(
                        &format!("Found target element with ID: {}", target_id_str).into(),
                    );
                    if let Ok(target_id) = target_id_str.parse::<i32>() {
                        if target_id != dragged_id {
                            target_index =
                                all_episodes.iter().position(|x| x.episodeid == target_id);
                        }
                    }
                } else {
                    // No visible target found - calculate virtual drop position using mouse coordinates
                    web_sys::console::log_1(
                        &"No visible target found, calculating virtual position".into(),
                    );
                    let client_y = e.client_y() as f64;

                    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                        if let Ok(Some(container)) =
                            document.query_selector(".virtual-list-container")
                        {
                            if let Some(container_element) =
                                container.dyn_ref::<web_sys::HtmlElement>()
                            {
                                let container_rect = container_element.get_bounding_client_rect();
                                let scroll_top = container_element.scroll_top() as f64;

                                // Calculate which episode index the drop position corresponds to
                                // Use responsive item height calculation
                                let window_width = web_sys::window()
                                    .unwrap()
                                    .inner_width()
                                    .unwrap()
                                    .as_f64()
                                    .unwrap();
                                let item_height = calculate_item_height(window_width);
                                let relative_y = (client_y - container_rect.top()) + scroll_top;
                                let virtual_index = (relative_y / item_height).floor() as usize;

                                web_sys::console::log_1(
                                    &format!("Virtual drop index calculated: {}", virtual_index)
                                        .into(),
                                );

                                // Clamp to valid range
                                target_index =
                                    Some(virtual_index.min(all_episodes.len().saturating_sub(1)));
                            }
                        }
                    }
                }

                // Perform the reorder if we have a valid target
                if let Some(target_idx) = target_index {
                    web_sys::console::log_1(
                        &format!(
                            "Reordering: dragged {} to position {}",
                            dragged_id, target_idx
                        )
                        .into(),
                    );

                    let mut episodes_vec = all_episodes.clone();
                    if let Some(dragged_index) =
                        episodes_vec.iter().position(|x| x.episodeid == dragged_id)
                    {
                        if dragged_index != target_idx {
                            // Remove and reinsert at the correct position
                            let dragged_item = episodes_vec.remove(dragged_index);
                            let insert_idx = if dragged_index < target_idx {
                                target_idx
                            } else {
                                target_idx
                            };
                            episodes_vec.insert(insert_idx.min(episodes_vec.len()), dragged_item);

                            web_sys::console::log_1(&"Calling reorder queue API".into());

                            // Extract episode IDs
                            let episode_ids: Vec<i32> =
                                episodes_vec.iter().map(|ep| ep.episodeid).collect();

                            dispatch.reduce_mut(|state| {
                                state.queued_episodes = Some(QueuedEpisodesResponse {
                                    episodes: episodes_vec.clone(),
                                });
                            });

                            // Make a backend call to update the order on the server side
                            let server_name = server_name.clone();
                            let api_key = api_key.clone();
                            let user_id = user_id.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                if let Err(err) = pod_req::call_reorder_queue(
                                    &server_name.unwrap(),
                                    &api_key.unwrap(),
                                    &user_id.unwrap(),
                                    &episode_ids,
                                )
                                .await
                                {
                                    web_sys::console::log_1(
                                        &format!("Failed to update order on server: {:?}", err)
                                            .into(),
                                    );
                                } else {
                                    web_sys::console::log_1(
                                        &"Reorder queue API call successful".into(),
                                    );
                                }
                            });
                        } else {
                            web_sys::console::log_1(&"Same position, no reorder needed".into());
                        }
                    }
                } else {
                    web_sys::console::log_1(&"No valid target index found".into());
                }
            } else {
                web_sys::console::log_1(&"No dragged ID found".into());
            }

            dragging.set(None);
        })
    };

    html! {
        <crate::components::virtual_list::VirtualList
            episodes={ props.episodes.clone() }
            page_type={ PageType::Queue }
            drag_callbacks={ DragCallbacks{ ondragstart: Some(ondragstart), ondragenter: Some(ondragenter), ondragover: Some(ondragover), ondrop: Some(ondrop) } }
            />
    }
}
