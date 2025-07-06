use super::app_drawer::App_drawer;
use super::gen_components::{
    empty_message, on_shownotes_click, queue_episode_item, Search_nav, UseScrollToTop,
};
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::AppStateMsg;
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::pod_req;
use crate::requests::pod_req::QueuedEpisodesResponse;
use gloo_utils::document;
use wasm_bindgen::JsCast;
use web_sys::Element;
use web_sys::{window, DragEvent, HtmlElement};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

// Add this at the top of your file
const SCROLL_THRESHOLD: f64 = 150.0; // Increased threshold for easier activation
const SCROLL_SPEED: f64 = 15.0; // Increased speed

#[derive(Clone, Debug)]
struct ScrollState {
    interval_id: Option<i32>,
}

fn stop_auto_scroll(interval_id: i32) {
    if let Some(window) = window() {
        window.clear_interval_with_handle(interval_id);
    }
}

#[function_component(Queue)]
pub fn queue() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let history = BrowserHistory::new();

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();

    let loading = use_state(|| true);
    let dragging = use_state(|| None);
    let touch_start_y = use_state(|| 0.0);
    let touch_start_x = use_state(|| 0.0);
    let is_dragging = use_state(|| false);
    let active_modal = use_state(|| None::<i32>);
    let active_modal_clone = active_modal.clone();
    let on_modal_open = Callback::from(move |episode_id: i32| {
        active_modal_clone.set(Some(episode_id));
    });
    let active_modal_clone = active_modal.clone();
    let on_modal_close = Callback::from(move |_| {
        active_modal_clone.set(None);
    });
    let dragged_element = use_state(|| None::<web_sys::Element>);
    let scroll_state = use_state(|| ScrollState { interval_id: None });

    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

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
                            Ok(fetched_episodes) => {
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
                            // Modern mobile-friendly queue page with tab-style page title
                            <div class="mb-2">
                                // Tab-style page indicator
                                <div class="page-tab-indicator">
                                    <i class="ph ph-queue tab-icon"></i>
                                    {"Queue"}
                                </div>
                            </div>
                        }
                    }

                    {
                        if let Some(queued_eps) = state.queued_episodes.clone() {
                            if queued_eps.episodes.is_empty() {
                                // Render "No Queued Episodes Found" if episodes list is empty
                                empty_message(
                                    "No Queued Episodes Found",
                                    "You can queue episodes by clicking the context button on each episode and clicking 'Queue Episode'. Doing this will play episodes in order of the queue after the currently playing episode is complete."
                                )
                            } else {
                                let ondragstart = {
                                    let dragging = dragging.clone();
                                    Callback::from(move |e: DragEvent| {
                                        let target = e.target().unwrap();
                                        let id = target
                                            .dyn_ref::<HtmlElement>()
                                            .unwrap()
                                            .get_attribute("data-id")
                                            .unwrap();
                                        dragging.set(Some(id.parse::<i32>().unwrap()));
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
                                    let scroll_speed = 20;

                                    let window = web_sys::window().expect("should have a Window");

                                    // Scroll up if the cursor is near the top of the viewport
                                    if y < 50 {
                                        window.scroll_by_with_x_and_y(0.0, -scroll_speed as f64);
                                    }

                                    // Scroll down if the cursor is near the bottom of the viewport
                                    let window_height = window.inner_height().unwrap().as_f64().unwrap();
                                    if y > (window_height - 50.0) as i32 {
                                        window.scroll_by_with_x_and_y(0.0, scroll_speed as f64);
                                    }

                                });

                                let ondrop = {
                                    let dragging = dragging.clone();
                                    let dispatch = dispatch.clone();
                                    let episodes = queued_eps.clone();
                                    let server_drop = server_name.clone();
                                    let api_drop = api_key.clone();
                                    Callback::from(move |e: DragEvent| {
                                        let user_id = user_id.clone();
                                        let server_name = server_drop.clone();
                                        let api_key = api_drop.clone();
                                        e.prevent_default();
                                        let mut target = e.target().unwrap().dyn_into::<web_sys::Element>().unwrap();
                                        while !target.has_attribute("data-id") {
                                            target = target.parent_element().unwrap();
                                        }
                                        let target_id = target
                                            .get_attribute("data-id")
                                            .unwrap()
                                            .parse::<i32>()
                                            .unwrap();

                                        if let Some(dragged_id) = *dragging {
                                            let mut episodes_vec = episodes.episodes.clone();
                                            let dragged_index = episodes_vec
                                                .iter()
                                                .position(|x| x.episodeid == dragged_id)
                                                .unwrap();
                                            let target_index = episodes_vec
                                                .iter()
                                                .position(|x| x.episodeid == target_id)
                                                .unwrap();

                                            // Remove the dragged item and reinsert it at the target position
                                            let dragged_item = episodes_vec.remove(dragged_index);
                                            episodes_vec.insert(target_index, dragged_item);
                                            // Extract episode IDs
                                            let episode_ids: Vec<i32> = episodes_vec.iter().map(|ep| ep.episodeid).collect();

                                            dispatch.reduce_mut(|state| {
                                                state.queued_episodes = Some(QueuedEpisodesResponse {
                                                    episodes: episodes_vec.clone(),
                                                });
                                            });

                                            // Make a backend call to update the order on the server side
                                            wasm_bindgen_futures::spawn_local(async move {
                                                if let Err(err) = pod_req::call_reorder_queue(&server_name.unwrap(), &api_key.unwrap(), &user_id.unwrap(), &episode_ids).await {
                                                    web_sys::console::log_1(&format!("Failed to update order on server: {:?}", err).into());
                                                } else {
                                                }
                                            });
                                        }

                                        dragging.set(None);
                                    })
                                };



                                let ontouchstart = {
                                    let touch_start_y = touch_start_y.clone();
                                    let touch_start_x = touch_start_x.clone();
                                    let is_dragging = is_dragging.clone();
                                    let dragged_element = dragged_element.clone();

                                    Callback::from(move |e: TouchEvent| {
                                        if let Some(touch) = e.touches().get(0) {
                                            if let Some(target) = e.target() {
                                                // Cast the EventTarget to Element
                                                if let Some(element) = target.dyn_ref::<Element>() {
                                                    // Check if the touch started on or near the drag handle
                                                    if let Ok(Some(_)) = element.closest(".drag-handle-wrapper") {
                                                        // Only if we're touching the drag handle
                                                        if let Ok(Some(container)) = element.closest(".item-container") {
                                                            touch_start_y.set(touch.client_y() as f64);
                                                            touch_start_x.set(touch.client_x() as f64);
                                                            is_dragging.set(true);
                                                            dragged_element.set(Some(container));
                                                            e.prevent_default(); // Only prevent default if we're starting a drag
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    })
                                };


                                let ontouchmove = {
                                    let touch_start_y = touch_start_y.clone();
                                    let touch_start_x = touch_start_x.clone();
                                    let is_dragging = is_dragging.clone();
                                    let dragged_element = dragged_element.clone();
                                    let scroll_state = scroll_state.clone();

                                    Callback::from(move |e: TouchEvent| {
                                        if *is_dragging {
                                            if let Some(touch) = e.touches().get(0) {
                                                let current_y = touch.client_y() as f64;
                                                let current_x = touch.client_x() as f64;
                                                let window = window().unwrap();
                                                let viewport_height = window.inner_height().unwrap().as_f64().unwrap();
                                                let document = window.document().unwrap();
                                                let document_height = document.document_element().unwrap().scroll_height() as f64;
                                                let current_scroll = window.scroll_y().unwrap();

                                                // Store initial scroll position if not set
                                                let initial_scroll = scroll_state.interval_id.unwrap_or_else(|| {
                                                    let id = current_scroll as i32;
                                                    scroll_state.set(ScrollState {
                                                        interval_id: Some(id),
                                                    });
                                                    id
                                                }) as f64;

                                                // Calculate delta with scroll offset compensation
                                                let scroll_offset = current_scroll - initial_scroll;
                                                let delta_y = (current_y - *touch_start_y) + scroll_offset;
                                                let delta_x = current_x - *touch_start_x;

                                                // Handle scrolling logic
                                                let new_scroll_direction = if current_y < SCROLL_THRESHOLD && current_scroll > 0.0 {
                                                    -1.0
                                                } else if current_y > viewport_height - SCROLL_THRESHOLD
                                                    && current_scroll < document_height - viewport_height {
                                                    1.0
                                                } else {
                                                    0.0
                                                };

                                                if new_scroll_direction != 0.0 {
                                                    window.scroll_by_with_x_and_y(0.0, new_scroll_direction * SCROLL_SPEED);
                                                }

                                                // Update dragged element position
                                                if let Some(element) = (*dragged_element).clone() {
                                                    if let Some(html_element) = element.dyn_ref::<HtmlElement>() {
                                                        html_element
                                                            .style()
                                                            .set_property("transform", &format!("translate({}px, {}px)", delta_x, delta_y))
                                                            .unwrap();
                                                        html_element
                                                            .style()
                                                            .set_property("z-index", "1000")
                                                            .unwrap();
                                                    }
                                                }
                                                e.prevent_default();
                                                e.stop_propagation();
                                            }
                                        }
                                    })
                                };

                                // The key issue is in the touchend handler. Here's the fixed version:
                                let ontouchend = {
                                    let dragged_element = dragged_element.clone();
                                    let is_dragging = is_dragging.clone();
                                    let scroll_state = scroll_state.clone();
                                    let dispatch = dispatch.clone();
                                    let episodes = queued_eps.clone();
                                    let server_name = server_name.clone();
                                    let api_key = api_key.clone();
                                    let user_id = user_id.clone();

                                    Callback::from(move |e: TouchEvent| {
                                        // Stop any ongoing scrolling
                                        if let Some(interval_id) = scroll_state.interval_id {
                                            stop_auto_scroll(interval_id);
                                            scroll_state.set(ScrollState {
                                                interval_id: None,
                                            });
                                        }

                                        if *is_dragging {
                                            if let Some(dragged) = (*dragged_element).clone() {
                                                let dragged_rect = dragged.get_bounding_client_rect();
                                                let dragged_center_y = dragged_rect.top() + dragged_rect.height() / 2.0;
                                                let window = window().unwrap();
                                                let scroll_y = window.scroll_y().unwrap();

                                                let mut closest_element = None;
                                                let mut min_distance = f64::MAX;

                                                // Find all item containers and determine the closest one
                                                if let Ok(containers) = document().query_selector_all(".item-container") {
                                                    for i in 0..containers.length() {
                                                        if let Some(container) = containers.get(i) {
                                                            if let Some(element) = container.dyn_ref::<Element>() {
                                                                if element != &dragged {
                                                                    let rect = element.get_bounding_client_rect();
                                                                    // Adjust for scroll position
                                                                    let actual_center_y = rect.top() + rect.height() / 2.0 + scroll_y;
                                                                    let distance = (actual_center_y - (dragged_center_y + scroll_y)).abs();

                                                                    if distance < min_distance {
                                                                        min_distance = distance;
                                                                        closest_element = Some(element.clone());
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // Reorder the elements if we found a closest element
                                                if let Some(target) = closest_element {
                                                    let dragged_id = dragged
                                                        .get_attribute("data-id")
                                                        .unwrap_or_default()
                                                        .parse::<i32>()
                                                        .unwrap_or_default();
                                                    let target_id = target
                                                        .get_attribute("data-id")
                                                        .unwrap_or_default()
                                                        .parse::<i32>()
                                                        .unwrap_or_default();

                                                    // Only proceed if we have valid IDs
                                                    if dragged_id != 0 && target_id != 0 && dragged_id != target_id {
                                                        let mut episodes_vec = episodes.episodes.clone();
                                                        if let (Some(dragged_index), Some(target_index)) = (
                                                            episodes_vec.iter().position(|x| x.episodeid == dragged_id),
                                                            episodes_vec.iter().position(|x| x.episodeid == target_id),
                                                        ) {
                                                            // Remove and reinsert at the correct position
                                                            let dragged_item = episodes_vec.remove(dragged_index);
                                                            episodes_vec.insert(target_index, dragged_item);

                                                            // Update the state
                                                            let episode_ids: Vec<i32> = episodes_vec.iter().map(|ep| ep.episodeid).collect();
                                                            dispatch.reduce_mut(|state| {
                                                                state.queued_episodes = Some(QueuedEpisodesResponse {
                                                                    episodes: episodes_vec.clone(),
                                                                });
                                                            });

                                                            // Update server
                                                            let server_name = server_name.clone();
                                                            let api_key = api_key.clone();
                                                            let user_id = user_id.clone();
                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                if let Err(err) = pod_req::call_reorder_queue(
                                                                    &server_name.unwrap(),
                                                                    &api_key.unwrap(),
                                                                    &user_id.unwrap(),
                                                                    &episode_ids
                                                                ).await {
                                                                    web_sys::console::log_1(&format!("Failed to update order on server: {:?}", err).into());
                                                                }
                                                            });
                                                        }
                                                    }
                                                }

                                                // Reset the dragged element's style
                                                if let Some(element) = dragged.dyn_ref::<HtmlElement>() {
                                                    element.style().set_property("transform", "none").unwrap();
                                                    element.style().set_property("z-index", "auto").unwrap();
                                                }
                                            }
                                            e.prevent_default();
                                        }

                                        // Always reset the drag state
                                        is_dragging.set(false);
                                        dragged_element.set(None);
                                    })
                                };



                                queued_eps.episodes.into_iter().map(|episode| {
                            let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
                            let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
                            let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());

                            let is_current_episode = audio_state
                                .currently_playing
                                .as_ref()
                                .map_or(false, |current| current.episode_id == episode.episodeid);
                            let is_playing = audio_state.audio_playing.unwrap_or(false);


                            let history_clone = history.clone();
                            let id_string = &episode.episodeid.to_string();

                            let is_expanded = state.expanded_descriptions.contains(id_string);

                            let dispatch = dispatch.clone();

                            let episode_url_clone = episode.episodeurl.clone();
                            let episode_title_clone = episode.episodetitle.clone();
                            let episode_description_clone = episode.episodedescription.clone();
                            let episode_artwork_clone = episode.episodeartwork.clone();
                            let episode_duration_clone = episode.episodeduration.clone();
                            let episode_id_clone = episode.episodeid.clone();
                            let episode_listened_clone = episode.listenduration.clone();
                            let episode_is_youtube = Some(episode.is_youtube.clone());

                            let sanitized_description = sanitize_html_with_blank_target(&episode.episodedescription.clone());

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
                            let episode_description_for_closure = episode_description_clone.clone();
                            let episode_artwork_for_closure = episode_artwork_clone.clone();
                            let episode_duration_for_closure = episode_duration_clone.clone();
                            let episode_id_for_closure = episode_id_clone.clone();
                            let listener_duration_for_closure = episode_listened_clone.clone();

                            let user_id_play = user_id.clone();
                            let server_name_play = server_name.clone();
                            let api_key_play = api_key.clone();
                            let audio_dispatch = audio_dispatch.clone();

                            let date_format = match_date_format(state.date_format.as_deref());
                            let datetime = parse_date(&episode.episodepubdate, &state.user_tz);
                            let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));

                            let on_play_pause = on_play_pause(
                                episode_url_for_closure.clone(),
                                episode_title_for_closure.clone(),
                                episode_description_for_closure.clone(),
                                format_release.clone(),
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
                                episode_is_youtube.clone(),
                            );

                            let on_shownotes_click = on_shownotes_click(
                                history_clone.clone(),
                                dispatch.clone(),
                                Some(episode_id_for_closure.clone()),
                                Some(String::from("queue")),
                                Some(String::from("queue")),
                                Some(String::from("queue")),
                                true,
                                None,
                                episode_is_youtube,
                            );
                            let episode_url_for_ep_item = episode_url_clone.clone();
                            let check_episode_id = &episode.episodeid.clone();
                            let is_completed = state
                                .completed_episodes
                                .as_ref()
                                .unwrap_or(&vec![])
                                .contains(&check_episode_id);
                            let episode_id_clone = Some(episode.episodeid).clone();
                            let item = queue_episode_item(
                                Box::new(episode),
                                sanitized_description,
                                is_expanded,
                                &format_release,
                                on_play_pause,
                                on_shownotes_click,
                                toggle_expanded,
                                episode_duration_clone,
                                episode_listened_clone,
                                "queue",
                                Callback::from(|_| {}),
                                false,
                                episode_url_for_ep_item,
                                is_completed,
                                ondragstart.clone(),
                                ondragenter.clone(),
                                ondragover.clone(),
                                ondrop.clone(),
                                ontouchstart.clone(),
                                ontouchmove.clone(),
                                ontouchend.clone(),
                                *active_modal == episode_id_clone,
                                on_modal_open.clone(),
                                on_modal_close.clone(),
                                is_current_episode,
                                is_playing,
                            );

                            item
                        }).collect::<Html>()
                        }
                    } else {
                        empty_message(
                            "No Queued Episodes Found - State is None",
                            "You can queue episodes by clicking the context button on each episode and clicking 'Queue Episode'. Doing this will play episodes in order of the queue after the currently playing episode is complete."
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
        <App_drawer />
        </>
    }
}
