use super::app_drawer::App_drawer;
use super::gen_components::{
    empty_message, on_shownotes_click, use_long_press, virtual_episode_item, Search_nav,
    UseScrollToTop,
};
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::episodes_layout::AppStateMsg;
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
    get_filter_preference, set_filter_preference, get_default_sort_direction,
};
use crate::requests::pod_req;
use crate::requests::pod_req::SavedEpisodesResponse;
use gloo::events::EventListener;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::{Element, HtmlElement};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

use wasm_bindgen::prelude::wasm_bindgen;

#[derive(Clone, PartialEq)]
pub enum SavedSortDirection {
    NewestFirst,
    OldestFirst,
    ShortestFirst,
    LongestFirst,
    TitleAZ,
    TitleZA,
}

#[function_component(Saved)]
pub fn saved() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let history = BrowserHistory::new();

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let dropdown_open = use_state(|| false);
    let loading = use_state(|| true);

    let episode_search_term = use_state(|| String::new());
    
    // Initialize sort direction from local storage or default to newest first
    let episode_sort_direction = use_state(|| {
        let saved_preference = get_filter_preference("saved");
        match saved_preference.as_deref() {
            Some("newest") => Some(SavedSortDirection::NewestFirst),
            Some("oldest") => Some(SavedSortDirection::OldestFirst),
            Some("shortest") => Some(SavedSortDirection::ShortestFirst),
            Some("longest") => Some(SavedSortDirection::LongestFirst),
            Some("title_az") => Some(SavedSortDirection::TitleAZ),
            Some("title_za") => Some(SavedSortDirection::TitleZA),
            _ => Some(SavedSortDirection::NewestFirst), // Default to newest first
        }
    });
    
    let show_completed = use_state(|| false); // Toggle for showing completed episodes only
    let show_in_progress = use_state(|| false); // Toggle for showing in-progress episodes only

    let _toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |_: MouseEvent| {
            dropdown_open.set(!*dropdown_open);
        })
    };

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

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let dispatch = effect_dispatch.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_saved_episodes(&server_name, &api_key, &user_id)
                            .await
                        {
                            Ok(fetched_episodes) => {
                                let completed_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();

                                dispatch.reduce_mut(move |state| {
                                    state.saved_episodes = Some(SavedEpisodesResponse {
                                        episodes: fetched_episodes,
                                    });
                                    state.completed_episodes = Some(completed_episode_ids);
                                });
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

    let filtered_episodes = use_memo(
        (
            state.saved_episodes.clone(),
            episode_search_term.clone(),
            episode_sort_direction.clone(),
            show_completed.clone(),
            show_in_progress.clone(),
        ),
        |(saved_eps, search, sort_dir, show_completed, show_in_progress)| {
            if let Some(saved_episodes) = saved_eps {
                let mut filtered = saved_episodes
                    .episodes
                    .iter()
                    .filter(|episode| {
                        // Search filter
                        let matches_search = if !search.is_empty() {
                            episode
                                .episodetitle
                                .to_lowercase()
                                .contains(&search.to_lowercase())
                                || episode
                                    .episodedescription
                                    .to_lowercase()
                                    .contains(&search.to_lowercase())
                        } else {
                            true
                        };

                        // Completion status filter
                        let matches_status = if **show_completed {
                            episode.completed
                        } else if **show_in_progress {
                            episode.listenduration.is_some() && !episode.completed
                        } else {
                            true // Show all if no filter is active
                        };

                        matches_search && matches_status
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                // Apply sorting
                if let Some(direction) = (*sort_dir).as_ref() {
                    filtered.sort_by(|a, b| match direction {
                        SavedSortDirection::NewestFirst => b.episodepubdate.cmp(&a.episodepubdate),
                        SavedSortDirection::OldestFirst => a.episodepubdate.cmp(&b.episodepubdate),
                        SavedSortDirection::ShortestFirst => {
                            a.episodeduration.cmp(&b.episodeduration)
                        }
                        SavedSortDirection::LongestFirst => {
                            b.episodeduration.cmp(&a.episodeduration)
                        }
                        SavedSortDirection::TitleAZ => a.episodetitle.cmp(&b.episodetitle),
                        SavedSortDirection::TitleZA => b.episodetitle.cmp(&a.episodetitle),
                    });
                }
                filtered
            } else {
                vec![]
            }
        },
    );
    let show_in_prog_button = show_in_progress.clone();

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
                    html! {
                        <>
                            // Modern mobile-friendly filter bar with tab-style page title
                            <div class="mb-6 space-y-4 mt-4">
                                // Combined search and sort bar with tab-style title (seamless design)
                                <div class="flex gap-0 h-12 relative">
                                    // Tab-style page indicator
                                    <div class="page-tab-indicator">
                                        <i class="ph ph-bookmark tab-icon"></i>
                                        {"Saved"}
                                    </div>
                                    // Search input (left half)
                                    <div class="flex-1 relative">
                                        <input
                                            type="text"
                                            class="search-input"
                                            placeholder="Search saved episodes..."
                                            value={(*episode_search_term).clone()}
                                            oninput={let episode_search_term = episode_search_term.clone();
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
                                                    set_filter_preference("saved", &value);
                                                    
                                                    match value.as_str() {
                                                        "newest" => episode_sort_direction.set(Some(SavedSortDirection::NewestFirst)),
                                                        "oldest" => episode_sort_direction.set(Some(SavedSortDirection::OldestFirst)),
                                                        "shortest" => episode_sort_direction.set(Some(SavedSortDirection::ShortestFirst)),
                                                        "longest" => episode_sort_direction.set(Some(SavedSortDirection::LongestFirst)),
                                                        "title_az" => episode_sort_direction.set(Some(SavedSortDirection::TitleAZ)),
                                                        "title_za" => episode_sort_direction.set(Some(SavedSortDirection::TitleZA)),
                                                        _ => episode_sort_direction.set(None),
                                                    }
                                                })
                                            }
                                        >
                                            <option value="newest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "newest"}>{"Newest First"}</option>
                                            <option value="oldest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "oldest"}>{"Oldest First"}</option>
                                            <option value="shortest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "shortest"}>{"Shortest First"}</option>
                                            <option value="longest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "longest"}>{"Longest First"}</option>
                                            <option value="title_az" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_az"}>{"Title A-Z"}</option>
                                            <option value="title_za" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_za"}>{"Title Z-A"}</option>
                                        </select>
                                        <i class="ph ph-caret-down dropdown-arrow"></i>
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
                                        class="filter-chip"
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
                                                if *show_completed {
                                                    show_in_progress.set(false);
                                                }
                                            })
                                        }
                                        class={classes!(
                                            "filter-chip",
                                            if *show_completed { "filter-chip-active" } else { "" }
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
                                                if *show_in_progress {
                                                    show_completed.set(false);
                                                }
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
                                </div>
                            </div>

                            {
                                if let Some(_saved_eps) = state.saved_episodes.clone() {
                                    if (*filtered_episodes).is_empty() {
                                        empty_message(
                                            "No Saved Episodes Found",
                                            "You can save episodes by clicking the context button on each episode and clicking 'Save Episode'. Doing this will save episodes here for easy access when you want to return to them."
                                        )
                                    } else {
                                        html! {
                                            <VirtualList
                                                episodes={(*filtered_episodes).clone()}
                                                page_type="saved"
                                            />
                                        }
                                    }
                                } else {
                                    empty_message(
                                        "No Saved Episodes Found",
                                        "You can save episodes by clicking the context button on each episode and clicking 'Save Episode'. Doing this will save episodes here for easy access when you want to return to them."
                                    )
                                }
                            }
                        </>
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
    pub episodes: Vec<pod_req::SavedEpisode>,
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
                // Add 16px (mb-4) to each height value for the virtual list calculations
                let new_item_height = if width <= 530.0 {
                    122.0 + 16.0 // Base height + margin
                } else if width <= 768.0 {
                    150.0 + 16.0 // Base height + margin
                } else {
                    221.0 + 16.0 // Base height + margin
                };

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

    let visible_episodes = (buffered_start..buffered_end)
        .map(|index| {
            let episode = props.episodes[index].clone();
            html! {
                <SavedEpisode
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
pub struct SavedEpisodeProps {
    pub episode: pod_req::SavedEpisode,
    pub page_type: String,
}

#[function_component(SavedEpisode)]
pub fn saved_episode(props: &SavedEpisodeProps) -> Html {
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
    let on_modal_close = Callback::from(move |_: MouseEvent| show_clonedal2.set(false));
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

    let desc_expanded = state.expanded_descriptions.contains(id_string);

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

    // Update container height based on screen width
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