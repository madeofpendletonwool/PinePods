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
use crate::requests::pod_req::{QueuedEpisode, QueuedEpisodesResponse};
use gloo_events::EventListener;
use gloo_utils::document;
use wasm_bindgen::JsCast;
use web_sys::Element;
use web_sys::{window, DragEvent, HtmlElement, TouchEvent};
use yew::prelude::*;
use yew::{function_component, html, Html, UseStateHandle};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

// Add this at the top of your file
const SCROLL_THRESHOLD: f64 = 150.0; // Increased threshold for easier activation
const SCROLL_SPEED: f64 = 15.0; // Increased speed

// Helper function to calculate responsive item height including all spacing
fn calculate_item_height(window_width: f64) -> f64 {
    // Try to measure actual height from DOM first
    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
        if let Ok(Some(first_item)) = document.query_selector(".item-container") {
            if let Some(element) = first_item.dyn_ref::<web_sys::HtmlElement>() {
                let rect = element.get_bounding_client_rect();
                let actual_height = rect.height();
                let margin_bottom = 16.0; // mb-4 = 1rem = 16px
                let total_height = actual_height + margin_bottom;
                
                web_sys::console::log_1(&format!(
                    "MEASURED: width={}, container_height={}, margin={}, total={}", 
                    window_width, actual_height, margin_bottom, total_height
                ).into());
                
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
    let active_modal = use_state(|| None::<i32>);
    let active_modal_clone = active_modal.clone();
    let on_modal_open = Callback::from(move |episode_id: i32| {
        active_modal_clone.set(Some(episode_id));
    });
    let active_modal_clone = active_modal.clone();
    let on_modal_close = Callback::from(move |_: MouseEvent| {
        active_modal_clone.set(None);
    });

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

                                // Fetch local episode IDs for Tauri mode
                                #[cfg(not(feature = "server_build"))]
                                {
                                    let dispatch_local = dispatch.clone();
                                    wasm_bindgen_futures::spawn_local(async move {
                                        if let Ok(local_episodes) = crate::components::downloads_tauri::fetch_local_episodes().await {
                                            let local_episode_ids: Vec<i32> = local_episodes
                                                .iter()
                                                .map(|ep| ep.episodeid)
                                                .collect();
                                            dispatch_local.reduce_mut(move |state| {
                                                state.locally_downloaded_episodes = Some(local_episode_ids);
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
                                html! {
                                    <VirtualQueueList
                                        episodes={queued_eps.episodes.clone()}
                                    />
                                }
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

#[derive(Properties, PartialEq)]
pub struct VirtualQueueListProps {
    pub episodes: Vec<QueuedEpisode>,
}

#[function_component(VirtualQueueList)]
pub fn virtual_queue_list(props: &VirtualQueueListProps) -> Html {
    let scroll_pos = use_state(|| 0.0);
    let container_ref = use_node_ref();
    let container_height = use_state(|| 0.0);
    let item_height = use_state(|| 234.0); // Default item height
    let force_update = use_state(|| 0);
    
    // Shared drag state for all episodes
    let dragging = use_state(|| None::<i32>);
    
    // Effect to set initial container height, item height, and listen for window resize
    {
        let container_height = container_height.clone();
        let item_height = item_height.clone();
        let force_update = force_update.clone();

        use_effect_with((), move |_| {
            let window = window().expect("no global `window` exists");
            let window_clone = window.clone();

            let update_sizes = Callback::from(move |_| {
                let height = window_clone.inner_height().unwrap().as_f64().unwrap();
                container_height.set(height - 100.0);

                let width = window_clone.inner_width().unwrap().as_f64().unwrap();
                let new_item_height = calculate_item_height(width);

                web_sys::console::log_1(&format!("Virtual list: width={}, item_height={}", width, new_item_height).into());
                item_height.set(new_item_height);
                force_update.set(*force_update + 1);
            });

            update_sizes.emit(());

            let listener = EventListener::new(&window, "resize", move |_| {
                update_sizes.emit(());
            });

            move || drop(listener)
        });
    }

    // Effect for scroll handling - prevent feedback loop with debouncing
    {
        let scroll_pos = scroll_pos.clone();
        let container_ref = container_ref.clone();
        use_effect_with(container_ref.clone(), move |container_ref| {
            if let Some(container) = container_ref.cast::<HtmlElement>() {
                let scroll_pos_clone = scroll_pos.clone();
                let is_updating = std::rc::Rc::new(std::cell::RefCell::new(false));
                
                let scroll_listener = EventListener::new(&container, "scroll", move |event| {
                    // Prevent re-entrant calls that cause feedback loops
                    if *is_updating.borrow() {
                        return;
                    }
                    
                    if let Some(target) = event.target() {
                        if let Ok(element) = target.dyn_into::<Element>() {
                            let new_scroll_top = element.scroll_top() as f64;
                            let old_scroll_top = *scroll_pos_clone;
                            
                            // Only update if there's a significant change
                            if (new_scroll_top - old_scroll_top).abs() >= 5.0 {
                                *is_updating.borrow_mut() = true;
                                
                                // Use requestAnimationFrame to batch updates and prevent feedback
                                let scroll_pos_clone2 = scroll_pos_clone.clone();
                                let is_updating_clone = is_updating.clone();
                                let callback = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                                    scroll_pos_clone2.set(new_scroll_top);
                                    *is_updating_clone.borrow_mut() = false;
                                }) as Box<dyn FnMut()>);
                                
                                web_sys::window().unwrap().request_animation_frame(callback.as_ref().unchecked_ref()).unwrap();
                                callback.forget();
                            }
                        }
                    }
                });
                
                Box::new(move || {
                    drop(scroll_listener);
                }) as Box<dyn FnOnce()>
            } else {
                Box::new(|| {}) as Box<dyn FnOnce()>
            }
        });
    }

    let start_index = (*scroll_pos / *item_height).floor() as usize;
    let visible_count = ((*container_height / *item_height).ceil() as usize) + 1;
    let end_index = (start_index + visible_count).min(props.episodes.len());
    
    // Debug logging to see what's happening
    web_sys::console::log_1(&format!(
        "Virtual list debug: scroll_pos={}, item_height={}, container_height={}, start_index={}, visible_count={}, end_index={}, total_episodes={}", 
        *scroll_pos, *item_height, *container_height, start_index, visible_count, end_index, props.episodes.len()
    ).into());

    let visible_episodes = (start_index..end_index)
        .map(|index| {
            let episode = props.episodes[index].clone();
            html! {
                <QueueEpisode
                    key={format!("{}-{}", episode.episodeid, *force_update)}
                    episode={episode.clone()}
                    all_episodes={props.episodes.clone()}
                    dragging={dragging.clone()}
                />
            }
        })
        .collect::<Html>();

    let total_height = props.episodes.len() as f64 * *item_height;
    let offset_y = start_index as f64 * *item_height;
    
    // Debug the offset calculation specifically
    web_sys::console::log_1(&format!(
        "Offset debug: total_height={}, offset_y={}, start_index={}", 
        total_height, offset_y, start_index
    ).into());

    html! {
        <div
            ref={container_ref}
            class="virtual-list-container flex-grow overflow-y-auto"
            style="height: calc(100vh - 100px); -webkit-overflow-scrolling: touch; overscroll-behavior-y: contain;"
        >
            // Top spacer to push content down without using transforms
            <div style={format!("height: {}px; flex-shrink: 0;", offset_y)}></div>
            
            // Visible episodes 
            <div>
                { visible_episodes }
            </div>
            
            // Bottom spacer to maintain total height
            <div style={format!("height: {}px; flex-shrink: 0;", total_height - offset_y - (end_index - start_index) as f64 * *item_height)}></div>
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct QueueEpisodeProps {
    pub episode: QueuedEpisode,
    pub all_episodes: Vec<QueuedEpisode>,
    pub dragging: UseStateHandle<Option<i32>>,
}

#[function_component(QueueEpisode)]
pub fn queue_episode(props: &QueueEpisodeProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let history = BrowserHistory::new();
    
    // Drag and drop state - use shared dragging from props
    let dragging = &props.dragging;
    let touch_start_y = use_state(|| 0.0);
    let touch_start_x = use_state(|| 0.0);
    let is_dragging = use_state(|| false);
    let active_modal = use_state(|| None::<i32>);
    let dragged_element = use_state(|| None::<web_sys::Element>);
    let scroll_state = use_state(|| ScrollState { interval_id: None });
    
    let active_modal_clone = active_modal.clone();
    let on_modal_open = Callback::from(move |episode_id: i32| {
        active_modal_clone.set(Some(episode_id));
    });
    let active_modal_clone = active_modal.clone();
    let on_modal_close = Callback::from(move |_: MouseEvent| {
        active_modal_clone.set(None);
    });

    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    // All the drag handlers from the original queue.rs
    let ondragstart = {
        let dragging = dragging.clone();
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
        let scroll_speed = 20;

        // Find the virtual list container to scroll it instead of the window
        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
            if let Ok(Some(container)) = document.query_selector(".virtual-list-container") {
                if let Some(container_element) = container.dyn_ref::<web_sys::HtmlElement>() {
                    let container_rect = container_element.get_bounding_client_rect();
                    let container_top = container_rect.top();
                    let container_bottom = container_rect.bottom();
                    
                    // Scroll up if cursor is near the top of the container
                    if (y as f64) < container_top + 50.0 {
                        container_element.set_scroll_top(
                            (container_element.scroll_top() - scroll_speed).max(0)
                        );
                    }
                    
                    // Scroll down if cursor is near the bottom of the container
                    if (y as f64) > container_bottom - 50.0 {
                        container_element.set_scroll_top(
                            container_element.scroll_top() + scroll_speed
                        );
                    }
                }
            }
        }
    });

    let ondrop = {
        let dragging = dragging.clone();
        let dispatch = dispatch.clone();
        let all_episodes = props.all_episodes.clone();
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
                while !target.has_attribute("data-id") && target.parent_element().is_some() && attempts < 10 {
                    target = target.parent_element().unwrap();
                    attempts += 1;
                }
                
                if let Some(target_id_str) = target.get_attribute("data-id") {
                    web_sys::console::log_1(&format!("Found target element with ID: {}", target_id_str).into());
                    if let Ok(target_id) = target_id_str.parse::<i32>() {
                        if target_id != dragged_id {
                            target_index = all_episodes.iter().position(|x| x.episodeid == target_id);
                        }
                    }
                } else {
                    // No visible target found - calculate virtual drop position using mouse coordinates
                    web_sys::console::log_1(&"No visible target found, calculating virtual position".into());
                    let client_y = e.client_y() as f64;
                    
                    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                        if let Ok(Some(container)) = document.query_selector(".virtual-list-container") {
                            if let Some(container_element) = container.dyn_ref::<web_sys::HtmlElement>() {
                                let container_rect = container_element.get_bounding_client_rect();
                                let scroll_top = container_element.scroll_top() as f64;
                                
                                // Calculate which episode index the drop position corresponds to
                                // Use responsive item height calculation
                                let window_width = web_sys::window().unwrap().inner_width().unwrap().as_f64().unwrap();
                                let item_height = calculate_item_height(window_width);
                                let relative_y = (client_y - container_rect.top()) + scroll_top;
                                let virtual_index = (relative_y / item_height).floor() as usize;
                                
                                web_sys::console::log_1(&format!("Virtual drop index calculated: {}", virtual_index).into());
                                
                                // Clamp to valid range
                                target_index = Some(virtual_index.min(all_episodes.len().saturating_sub(1)));
                            }
                        }
                    }
                }

                // Perform the reorder if we have a valid target
                if let Some(target_idx) = target_index {
                    web_sys::console::log_1(&format!("Reordering: dragged {} to position {}", dragged_id, target_idx).into());
                    
                    let mut episodes_vec = all_episodes.clone();
                    if let Some(dragged_index) = episodes_vec.iter().position(|x| x.episodeid == dragged_id) {
                        if dragged_index != target_idx {
                            // Remove and reinsert at the correct position
                            let dragged_item = episodes_vec.remove(dragged_index);
                            let insert_idx = if dragged_index < target_idx { target_idx } else { target_idx };
                            episodes_vec.insert(insert_idx.min(episodes_vec.len()), dragged_item);
                            
                            web_sys::console::log_1(&"Calling reorder queue API".into());
                            
                            // Extract episode IDs
                            let episode_ids: Vec<i32> = episodes_vec.iter().map(|ep| ep.episodeid).collect();

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
                                if let Err(err) = pod_req::call_reorder_queue(&server_name.unwrap(), &api_key.unwrap(), &user_id.unwrap(), &episode_ids).await {
                                    web_sys::console::log_1(&format!("Failed to update order on server: {:?}", err).into());
                                } else {
                                    web_sys::console::log_1(&"Reorder queue API call successful".into());
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

                    // Handle scrolling logic for virtual container
                    if let Ok(Some(container)) = document.query_selector(".virtual-list-container") {
                        if let Some(container_element) = container.dyn_ref::<web_sys::HtmlElement>() {
                            let container_rect = container_element.get_bounding_client_rect();
                            let container_top = container_rect.top();
                            let container_bottom = container_rect.bottom();
                            
                            let new_scroll_direction = if current_y < container_top + SCROLL_THRESHOLD {
                                -1.0
                            } else if current_y > container_bottom - SCROLL_THRESHOLD {
                                1.0
                            } else {
                                0.0
                            };

                            if new_scroll_direction != 0.0 {
                                let current_scroll_top = container_element.scroll_top();
                                let new_scroll_top = if new_scroll_direction < 0.0 {
                                    (current_scroll_top as f64 - SCROLL_SPEED).max(0.0) as i32
                                } else {
                                    current_scroll_top + SCROLL_SPEED as i32
                                };
                                container_element.set_scroll_top(new_scroll_top);
                            }
                        }
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
        let all_episodes = props.all_episodes.clone();
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

                    let dragged_id = dragged
                        .get_attribute("data-id")
                        .unwrap_or_default()
                        .parse::<i32>()
                        .unwrap_or_default();

                    if dragged_id != 0 {
                        let mut target_index = None;
                        
                        // If we found a closest element, use that
                        if let Some(target) = closest_element {
                            let target_id = target
                                .get_attribute("data-id")
                                .unwrap_or_default()
                                .parse::<i32>()
                                .unwrap_or_default();
                            
                            if target_id != 0 && target_id != dragged_id {
                                target_index = all_episodes.iter().position(|x| x.episodeid == target_id);
                            }
                        } else {
                            // No visible element found - calculate virtual drop position
                            if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                                if let Ok(Some(container)) = document.query_selector(".virtual-list-container") {
                                    if let Some(container_element) = container.dyn_ref::<web_sys::HtmlElement>() {
                                        let container_rect = container_element.get_bounding_client_rect();
                                        let scroll_top = container_element.scroll_top() as f64;
                                        
                                        // Calculate which episode index the drop position corresponds to  
                                        // Use responsive item height calculation
                                        let window_width = window.inner_width().unwrap().as_f64().unwrap();
                                        let item_height = calculate_item_height(window_width);
                                        let relative_y = (dragged_center_y - container_rect.top()) + scroll_top;
                                        let virtual_index = (relative_y / item_height).floor() as usize;
                                        
                                        // Clamp to valid range
                                        target_index = Some(virtual_index.min(all_episodes.len().saturating_sub(1)));
                                    }
                                }
                            }
                        }

                        // Perform the reorder if we have a valid target
                        if let Some(target_idx) = target_index {
                            let mut episodes_vec = all_episodes.clone();
                            if let Some(dragged_index) = episodes_vec.iter().position(|x| x.episodeid == dragged_id) {
                                if dragged_index != target_idx {
                                    // Remove and reinsert at the correct position
                                    let dragged_item = episodes_vec.remove(dragged_index);
                                    let insert_idx = if dragged_index < target_idx { target_idx } else { target_idx };
                                    episodes_vec.insert(insert_idx.min(episodes_vec.len()), dragged_item);

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
    
    // Generate the episode item using the same logic as the original queue.rs
    let is_current_episode = audio_state
        .currently_playing
        .as_ref()
        .map_or(false, |current| current.episode_id == props.episode.episodeid);
    let is_playing = audio_state.audio_playing.unwrap_or(false);

    let history_clone = history.clone();
    let id_string = &props.episode.episodeid.to_string();
    
    let is_expanded = state.expanded_descriptions.contains(id_string);
    
    let episode_url_clone = props.episode.episodeurl.clone();
    let episode_title_clone = props.episode.episodetitle.clone();
    let episode_description_clone = props.episode.episodedescription.clone();
    let episode_artwork_clone = props.episode.episodeartwork.clone();
    let episode_duration_clone = props.episode.episodeduration.clone();
    let episode_id_clone = props.episode.episodeid.clone();
    let episode_listened_clone = props.episode.listenduration.clone();
    let episode_is_youtube = Some(props.episode.is_youtube.clone());
    
    let sanitized_description = sanitize_html_with_blank_target(&props.episode.episodedescription.clone());
    
    let toggle_expanded = {
        let search_dispatch_clone = dispatch.clone();
        let state_clone = state.clone();
        let episode_guid = props.episode.episodeid.clone();
        
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
    
    let date_format = match_date_format(state.date_format.as_deref());
    let datetime = parse_date(&props.episode.episodepubdate, &state.user_tz);
    let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));
    
    let on_play_pause = on_play_pause(
        episode_url_clone.clone(),
        episode_title_clone.clone(),
        episode_description_clone.clone(),
        format_release.clone(),
        episode_artwork_clone.clone(),
        episode_duration_clone.clone(),
        episode_id_clone.clone(),
        episode_listened_clone.clone(),
        api_key.unwrap().unwrap(),
        user_id.unwrap(),
        server_name.unwrap(),
        audio_dispatch.clone(),
        audio_state.clone(),
        None,
        episode_is_youtube.clone(),
    );
    
    let on_shownotes_click = on_shownotes_click(
        history_clone.clone(),
        dispatch.clone(),
        Some(episode_id_clone.clone()),
        Some(String::from("queue")),
        Some(String::from("queue")),
        Some(String::from("queue")),
        true,
        None,
        episode_is_youtube,
    );
    
    let episode_url_for_ep_item = episode_url_clone.clone();
    let check_episode_id = &props.episode.episodeid.clone();
    let is_completed = state
        .completed_episodes
        .as_ref()
        .unwrap_or(&vec![])
        .contains(&check_episode_id);
    let episode_id_clone = Some(props.episode.episodeid).clone();
    
    let item = queue_episode_item(
        Box::new(props.episode.clone()),
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
}
