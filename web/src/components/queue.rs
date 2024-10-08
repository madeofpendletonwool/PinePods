use super::app_drawer::App_drawer;
use super::gen_components::{
    empty_message, on_shownotes_click, queue_episode_item, Search_nav, UseScrollToTop,
};
use crate::components::audio::on_play_click;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::AppStateMsg;
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
    truncate_description,
};
use crate::requests::pod_req;
use crate::requests::pod_req::QueuedEpisodesResponse;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;
// use crate::components::gen_funcs::check_auth;
use crate::components::episodes_layout::UIStateMsg;
use crate::requests::login_requests::use_check_authentication;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::Element;
use web_sys::{window, DragEvent, HtmlElement};

#[function_component(Queue)]
pub fn queue() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let history = BrowserHistory::new();

    // check_auth(effect_dispatch);

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();

    let session_dispatch = _post_dispatch.clone();
    let session_state = post_state.clone();
    let loading = use_state(|| true);
    let dragging = use_state(|| None);
    let touch_start_y = use_state(|| 0.0);
    let touch_start_x = use_state(|| 0.0);
    let is_dragging = use_state(|| false);
    let dragged_element = use_state(|| None::<web_sys::Element>);

    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

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
                            <div>
                            <h1 class="text-2xl item_container-text font-bold text-center mb-6">{"Queue"}</h1>
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
                                            // web_sys::console::log_1(format!("dragged: {}, target: {}", dragged_id, target_id).into()
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
                                                if let Some(element) = target.dyn_ref::<Element>() {
                                                    if element.closest(".item-container").is_ok() {
                                                        touch_start_y.set(touch.client_y() as f64);
                                                        touch_start_x.set(touch.client_x() as f64);
                                                        is_dragging.set(true);
                                                        dragged_element.set(element.closest(".item-container").ok().flatten());
                                                        e.prevent_default();
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
                                    Callback::from(move |e: TouchEvent| {
                                        e.stop_propagation();
                                        e.prevent_default();
                                        if *is_dragging {
                                            if let Some(touch) = e.touches().get(0) {
                                                let current_y = touch.client_y() as f64;
                                                let current_x = touch.client_x() as f64;
                                                let delta_y = current_y - *touch_start_y;
                                                let delta_x = current_x - *touch_start_x;

                                                if let Some(element) = (*dragged_element).clone() {
                                                    let style = element.dyn_ref::<HtmlElement>().unwrap().style();
                                                    style.set_property("transform", &format!("translate({}px, {}px)", delta_x, delta_y)).unwrap();
                                                    style.set_property("z-index", "1000").unwrap();
                                                }
                                            }
                                            e.prevent_default();
                                        }
                                    })
                                };


                                let ontouchend = {
                                    let dragged_element = dragged_element.clone();
                                    let is_dragging = is_dragging.clone();
                                    let dispatch = dispatch.clone();
                                    let episodes = queued_eps.clone();
                                    Callback::from(move |e: TouchEvent| {
                                        if *is_dragging {
                                            if let Some(dragged) = (*dragged_element).clone() {
                                                let dragged_rect = dragged.get_bounding_client_rect();
                                                let dragged_center_y = dragged_rect.top() + dragged_rect.height() / 2.0;

                                                let mut closest_element = None;
                                                let mut min_distance = f64::MAX;

                                                // Find the closest element
                                                if let Some(parent) = dragged.parent_element() {
                                                    if let Ok(children) = parent.query_selector_all(".item-container") {
                                                        let length = children.length();
                                                        for i in 0..length {
                                                            if let Some(node) = children.item(i) {
                                                                if let Some(element) = node.dyn_ref::<Element>() {
                                                                    if element != &dragged {
                                                                        let rect = element.get_bounding_client_rect();
                                                                        let center_y = rect.top() + rect.height() / 2.0;
                                                                        let distance = (center_y - dragged_center_y).abs();

                                                                        if distance < min_distance {
                                                                            min_distance = distance;
                                                                            closest_element = Some(element.clone());
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // Reorder the elements
                                                if let Some(target) = closest_element {
                                                    let dragged_id = dragged.get_attribute("data-id").unwrap().parse::<i32>().unwrap();
                                                    let target_id = target.get_attribute("data-id").unwrap().parse::<i32>().unwrap();

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
                                                    let server_name = server_name.clone();
                                                    let api_key = api_key.clone();
                                                    let user_id = user_id.clone();
                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        if let Err(err) = pod_req::call_reorder_queue(&server_name.unwrap(), &api_key.unwrap(), &user_id.unwrap(), &episode_ids).await {
                                                            web_sys::console::log_1(&format!("Failed to update order on server: {:?}", err).into());
                                                        }
                                                    });
                                                }

                                                // Reset the dragged element's style
                                                if let Some(element) = dragged.dyn_ref::<HtmlElement>() {
                                                    element.style().set_property("transform", "none").unwrap();
                                                    element.style().set_property("z-index", "auto").unwrap();
                                                }
                                            }
                                            e.prevent_default();
                                        }
                                        is_dragging.set(false);
                                        dragged_element.set(None);
                                    })
                                };



                                queued_eps.episodes.into_iter().map(|episode| {
                            let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
                            let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
                            let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
                            let history_clone = history.clone();
                            let id_string = &episode.episodeid.to_string();

                            let is_expanded = state.expanded_descriptions.contains(id_string);

                            let dispatch = dispatch.clone();

                            let episode_url_clone = episode.episodeurl.clone();
                            let episode_title_clone = episode.episodetitle.clone();
                            let episode_artwork_clone = episode.episodeartwork.clone();
                            let episode_duration_clone = episode.episodeduration.clone();
                            let episode_id_clone = episode.episodeid.clone();
                            let episode_listened_clone = episode.listenduration.clone();

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

                            let on_shownotes_click = on_shownotes_click(
                                history_clone.clone(),
                                dispatch.clone(),
                                Some(episode_id_for_closure.clone()),
                                Some(String::from("queue")),
                                Some(String::from("queue")),
                                Some(String::from("queue")),
                                true,
                            );
                            let episode_url_for_ep_item = episode_url_clone.clone();
                            let date_format = match_date_format(state.date_format.as_deref());
                            let datetime = parse_date(&episode.episodepubdate, &state.user_tz);
                            let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));
                            let check_episode_id = &episode.episodeid.clone();
                            let is_completed = state
                                .completed_episodes
                                .as_ref()
                                .unwrap_or(&vec![])
                                .contains(&check_episode_id);
                            let item = queue_episode_item(
                                Box::new(episode),
                                description.clone(),
                                is_expanded,
                                &format_release,
                                on_play_click,
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
