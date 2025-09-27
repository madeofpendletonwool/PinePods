use super::app_drawer::App_drawer;
use super::gen_components::{
    empty_message, on_shownotes_click, use_long_press, virtual_episode_item, Search_nav,
    UseScrollToTop,
};
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::pod_req;
use crate::requests::pod_req::Episode as EpisodeData;
use crate::requests::pod_req::RecentEps;
use gloo::events::EventListener;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::{Element, HtmlElement};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

use wasm_bindgen::prelude::*;

// Helper function to calculate responsive item height - MUST be synchronous and accurate
fn calculate_item_height(window_width: f64) -> f64 {
    // CRITICAL: Must match the exact height that episodes render at, including margin
    // Episodes render at container_height + mb-4 margin (16px)
    let height = if window_width <= 530.0 {
        122.0 + 16.0 // Mobile: episode container 122px + mb-4 margin
    } else if window_width <= 768.0 {
        150.0 + 16.0 // Tablet: episode container 150px + mb-4 margin  
    } else {
        221.0 + 16.0 // Desktop: episode container 221px + mb-4 margin
    };
    
    web_sys::console::log_1(&format!(
        "FEED HEIGHT CALC: width={}, calculated_height={}", 
        window_width, height
    ).into());
    
    height
}

#[function_component(Feed)]
pub fn feed() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let loading = use_state(|| true);
    
    // Capture i18n strings before they get moved
    let i18n_no_recent_episodes_found = i18n.t("feed.no_recent_episodes_found").to_string();
    let i18n_no_recent_episodes_description = i18n.t("feed.no_recent_episodes_description").to_string();

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
                        match pod_req::call_get_recent_eps(&server_name, &api_key, &user_id).await {
                            Ok(fetched_episodes) => {
                                let completed_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let saved_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.saved)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let queued_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.queued)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let downloaded_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.downloaded)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                dispatch.reduce_mut(move |state| {
                                    state.server_feed_results = Some(RecentEps {
                                        episodes: Some(fetched_episodes),
                                    });
                                    state.completed_episodes = Some(completed_episode_ids);
                                    state.saved_episode_ids = Some(saved_episode_ids);
                                    state.queued_episode_ids = Some(queued_episode_ids);
                                    state.downloaded_episode_ids = Some(downloaded_episode_ids);
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
                    if let Some(recent_eps) = state.server_feed_results.clone() {
                        let int_recent_eps = recent_eps.clone();
                        if let Some(episodes) = int_recent_eps.episodes {

                            if episodes.is_empty() {
                                // Render "No Recent Episodes Found" if episodes list is empty
                                empty_message(
                                    &i18n_no_recent_episodes_found,
                                    &i18n_no_recent_episodes_description
                                )
                            } else {
                                html! {
                                    <VirtualList
                                        episodes={episodes}
                                        page_type="home"
                                    />
                                }
                            }
                        } else {
                            empty_message(
                                "No Recent Episodes Found",
                                "You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."
                            )
                        }
                    } else {
                        empty_message(
                            "No Recent Episodes Found",
                            "You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."
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
pub struct VirtualListProps {
    pub episodes: Vec<EpisodeData>,
    pub page_type: String,
}

#[function_component(VirtualList)]
pub fn virtual_list(props: &VirtualListProps) -> Html {
    let scroll_pos = use_state(|| 0.0);
    let container_ref = use_node_ref();
    let container_height = use_state(|| 0.0);
    let item_height = use_state(|| 234.0); // Default item height
    let force_update = use_state(|| 0);

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
                            
                            // Always update scroll position for smoothest scrolling
                            if new_scroll_top != old_scroll_top {
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
    
    // Add buffer episodes above and below for smooth scrolling
    let buffer_size = 2; // Render 2 extra episodes above and below
    let buffered_start = start_index.saturating_sub(buffer_size);
    let buffered_end = (start_index + visible_count + buffer_size).min(props.episodes.len());
    
    // Debug logging to see what's happening
    web_sys::console::log_1(&format!(
        "Virtual list debug: scroll_pos={}, item_height={}, container_height={}, start_index={}, visible_count={}, buffered_start={}, buffered_end={}, total_episodes={}", 
        *scroll_pos, *item_height, *container_height, start_index, visible_count, buffered_start, buffered_end, props.episodes.len()
    ).into());

    let visible_episodes = (buffered_start..buffered_end)
        .map(|index| {
            let episode = props.episodes[index].clone();
            html! {
                <Episode
                    key={format!("{}-{}", episode.episodeid, *force_update)}
                    episode={episode.clone()}
                    page_type={props.page_type.clone()}
                />
            }
        })
        .collect::<Html>();

    let total_height = props.episodes.len() as f64 * *item_height;
    let offset_y = buffered_start as f64 * *item_height;

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
            <div style={format!("height: {}px; flex-shrink: 0;", total_height - offset_y - (buffered_end - buffered_start) as f64 * *item_height)}></div>
        </div>
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn toggleDescription(guid: &str, expanded: bool);
}
#[derive(Properties, PartialEq, Clone)]
pub struct EpisodeProps {
    pub episode: EpisodeData,
    pub page_type: String, // New prop to determine the context (e.g., "home", "saved")
}

#[function_component(Episode)]
pub fn episode(props: &EpisodeProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let id_string = &props.episode.episodeid.to_string();
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let show_modal = use_state(|| false);
    let show_clonedal = show_modal.clone();
    let show_clonedal2 = show_modal.clone();
    let on_modal_open = Callback::from(move |_: MouseEvent| show_clonedal.set(true));
    let container_height = use_state(|| "221px".to_string());

    // This will track if we're showing the context menu from a long press
    let show_context_menu = use_state(|| false);
    let context_menu_position = use_state(|| (0, 0));

    // Long press handler - simulate clicking the context button
    let context_button_ref = use_node_ref();
    let on_long_press = {
        let context_button_ref = context_button_ref.clone();
        let show_context_menu = show_context_menu.clone();
        let context_menu_position = context_menu_position.clone();

        Callback::from(move |event: TouchEvent| {
            if let Some(touch) = event.touches().get(0) {
                // Record position for the context menu
                context_menu_position.set((touch.client_x(), touch.client_y()));

                // Find and click the context button (if it exists)
                if let Some(button) = context_button_ref.cast::<web_sys::HtmlElement>() {
                    button.click();
                } else {
                    // If the button doesn't exist (maybe on mobile where it's hidden)
                    // we'll just set our state to show the menu
                    show_context_menu.set(true);
                }
            }
        })
    };

    // Setup long press detection
    let (on_touch_start, on_touch_end, on_touch_move, is_long_press_state, is_pressing_state) =
        use_long_press(on_long_press, Some(600)); // 600ms for long press

    let is_long_press = is_long_press_state;
    let is_pressing = is_pressing_state;

    // When long press is detected through the hook, update our state
    {
        let show_context_menu = show_context_menu.clone();
        use_effect_with(is_long_press, move |is_pressed| {
            if *is_pressed {
                show_context_menu.set(true);
            }
            || ()
        });
    }

    let on_modal_close = Callback::from(move |_: MouseEvent| show_clonedal2.set(false));

    let desc_expanded = desc_state.expanded_descriptions.contains(id_string);

    let toggle_expanded = {
        let desc_dispatch = desc_dispatch.clone();
        let episode_guid = props.episode.episodeid.clone().to_string();

        Callback::from(move |_: MouseEvent| {
            let guid = episode_guid.clone();
            desc_dispatch.reduce_mut(move |state| {
                if state.expanded_descriptions.contains(&guid) {
                    state.expanded_descriptions.remove(&guid);
                    toggleDescription(&guid, false);
                } else {
                    state.expanded_descriptions.insert(guid.clone());
                    toggleDescription(&guid, true);
                }
            });
        })
    };

    let is_current_episode = audio_state
        .currently_playing
        .as_ref()
        .map_or(false, |current| {
            current.episode_id == props.episode.episodeid
        });

    let is_playing = audio_state.audio_playing.unwrap_or(false);

    {
        let container_height = container_height.clone();
        use_effect_with((), move |_| {
            let update_height = {
                let container_height = container_height.clone();
                Callback::from(move |_| {
                    if let Some(window) = window() {
                        if let Ok(width) = window.inner_width() {
                            if let Some(width) = width.as_f64() {
                                let new_height = if width <= 530.0 {
                                    "122px"
                                } else if width <= 768.0 {
                                    "150px"
                                } else {
                                    "221px"
                                };
                                container_height.set(new_height.to_string());
                            }
                        }
                    }
                })
            };

            // Set initial height
            update_height.emit(());

            // Add resize listener
            let listener = EventListener::new(&window().unwrap(), "resize", move |_| {
                update_height.emit(());
            });

            move || drop(listener)
        });
    }

    let date_format = match_date_format(state.date_format.as_deref());
    let datetime = parse_date(&props.episode.episodepubdate, &state.user_tz);
    let formatted_date = format!(
        "{}",
        format_datetime(&datetime, &state.hour_preference, date_format)
    );

    let on_play_pause = on_play_pause(
        props.episode.episodeurl.clone(),
        props.episode.episodetitle.clone(),
        props.episode.episodedescription.clone(),
        formatted_date.clone(),
        props.episode.episodeartwork.clone(),
        props.episode.episodeduration.clone(),
        props.episode.episodeid.clone(),
        props.episode.listenduration.clone(),
        api_key.unwrap().unwrap(),
        user_id.unwrap(),
        server_name.unwrap(),
        audio_dispatch.clone(),
        audio_state.clone(),
        None,
        Some(props.episode.is_youtube.clone()),
    );

    let on_shownotes_click = {
        on_shownotes_click(
            history_clone.clone(),
            dispatch.clone(),
            Some(props.episode.episodeid.clone()),
            Some(props.page_type.clone()),
            Some(props.page_type.clone()),
            Some(props.page_type.clone()),
            true,
            None,
            Some(props.episode.is_youtube.clone()),
        )
    };

    let is_completed = state
        .completed_episodes
        .as_ref()
        .unwrap_or(&vec![])
        .contains(&props.episode.episodeid);

    // Close context menu callback
    let close_context_menu = {
        let show_context_menu = show_context_menu.clone();
        Callback::from(move |_| {
            show_context_menu.set(false);
        })
    };

    let item = virtual_episode_item(
        Box::new(props.episode.clone()),
        sanitize_html_with_blank_target(&props.episode.episodedescription),
        desc_expanded,
        &formatted_date,
        on_play_pause,
        on_shownotes_click,
        toggle_expanded,
        props.episode.episodeduration,
        props.episode.listenduration,
        &props.page_type,
        Callback::from(|_| {}),
        false,
        props.episode.episodeurl.clone(),
        is_completed,
        *show_modal,
        on_modal_open.clone(),
        on_modal_close.clone(),
        (*container_height).clone(),
        is_current_episode,
        is_playing,
        // Add new params for touch events
        on_touch_start,
        on_touch_end,
        on_touch_move,
        *show_context_menu,
        *context_menu_position,
        close_context_menu,
        context_button_ref,
        is_pressing,
    );

    item
}