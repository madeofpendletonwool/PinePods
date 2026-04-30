use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::context_menu_button::PageType;
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_components::{
    empty_message, on_shownotes_click, use_long_press, Search_nav, UseScrollToTop,
};
use crate::components::gen_funcs::{
    format_datetime, get_default_sort_direction, get_filter_preference, match_date_format,
    parse_date, sanitize_html_with_blank_target, set_filter_preference,
};
use crate::components::loading::Loading;
use crate::components::virtual_list::VirtualList;
use crate::requests::pod_req;
use crate::requests::pod_req::SavedEpisodesResponse;
use gloo::events::EventListener;
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::{Element, HtmlElement};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

use wasm_bindgen::prelude::wasm_bindgen;

#[derive(Clone, PartialEq)]
#[allow(dead_code)]
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
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
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
                                    state.saved_episodes = fetched_episodes;
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
            if saved_eps.len() > 0 {
                let mut filtered = saved_eps
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
                            episode.listenduration > 0 && !episode.completed
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
                        SavedSortDirection::NewestFirst => b.savedate.as_deref().unwrap_or("").cmp(a.savedate.as_deref().unwrap_or("")),
                        SavedSortDirection::OldestFirst => a.savedate.as_deref().unwrap_or("").cmp(b.savedate.as_deref().unwrap_or("")),
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

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                if *loading { // If loading is true, display the loading animation
                    html! { <Loading/> }
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
                                        {&i18n.t("saved.saved")}
                                    </div>
                                    // Search input (left half)
                                    <div class="flex-1 relative">
                                        <input
                                            type="text"
                                            class="search-input"
                                            placeholder={i18n.t("saved.search_placeholder")}
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
                                            <option value="newest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "newest"}>{&i18n.t("saved.newest_first")}</option>
                                            <option value="oldest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "oldest"}>{&i18n.t("saved.oldest_first")}</option>
                                            <option value="shortest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "shortest"}>{&i18n.t("saved.shortest_first")}</option>
                                            <option value="longest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "longest"}>{&i18n.t("saved.longest_first")}</option>
                                            <option value="title_az" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_az"}>{&i18n.t("saved.title_az")}</option>
                                            <option value="title_za" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_za"}>{&i18n.t("saved.title_za")}</option>
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
                                        <span class="text-sm font-medium">{&i18n.t("saved.clear_all")}</span>
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
                                        <span class="text-sm font-medium">{&i18n.t("saved.completed")}</span>
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
                                        <span class="text-sm font-medium">{&i18n.t("saved.in_progress")}</span>
                                    </button>
                                </div>
                            </div>

                            {
                                if state.saved_episodes.len() > 0 {
                                    if (*filtered_episodes).is_empty() {
                                        empty_message(
                                            &i18n.t("saved.no_saved_episodes"),
                                            &i18n.t("saved.save_episodes_instructions")
                                        )
                                    } else {
                                        html! {
                                            <VirtualList
                                                episodes={(*filtered_episodes).clone()}
                                                page_type= { PageType::Saved }
                                            />
                                        }
                                    }
                                } else {
                                    empty_message(
                                        &i18n.t("saved.no_saved_episodes"),
                                        &i18n.t("saved.save_episodes_instructions")
                                    )
                                }
                            }
                        </>
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
