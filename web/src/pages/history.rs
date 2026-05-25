use crate::components::app_drawer::App_drawer;
use crate::components::gen_components::{
    empty_message, on_shownotes_click, use_long_press, Search_nav, UseScrollToTop,
};
use crate::components::loading::Loading;

use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, EpisodeStatusState, FilterState};
use crate::components::gen_funcs::{
    get_default_sort_direction, get_filter_preference, set_filter_preference,
};

use crate::components::episode_list_item::EpisodeListItem;
use crate::requests::pod_req::{self, HistoryDataResponse};
use gloo::events::EventListener;
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::{Element, HtmlElement};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

use wasm_bindgen::prelude::*;

#[derive(Clone, PartialEq)]
pub enum HistorySortDirection {
    NewestFirst,
    OldestFirst,
    ShortestFirst,
    LongestFirst,
    TitleAZ,
    TitleZA,
}

#[function_component(PodHistory)]
pub fn history() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let (filter_state, _filter_dispatch) = use_store::<FilterState>();

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let loading = use_state(|| true);

    // Capture i18n strings before they get moved
    let i18n_history = i18n.t("history.history").to_string();
    let i18n_search_listening_history = i18n.t("history.search_listening_history").to_string();
    let i18n_newest_first = i18n.t("common.newest_first").to_string();
    let i18n_oldest_first = i18n.t("common.oldest_first").to_string();
    let i18n_shortest_first = i18n.t("common.shortest_first").to_string();
    let i18n_longest_first = i18n.t("common.longest_first").to_string();
    let i18n_title_az = i18n.t("common.title_az").to_string();
    let i18n_title_za = i18n.t("common.title_za").to_string();
    let i18n_clear_all = i18n.t("downloads.clear_all").to_string();
    let i18n_completed = i18n.t("downloads.completed").to_string();
    let i18n_in_progress = i18n.t("downloads.in_progress").to_string();
    let i18n_no_episode_history_found = i18n.t("history.no_episode_history_found").to_string();
    let i18n_no_episode_history_description =
        i18n.t("history.no_episode_history_description").to_string();

    let episode_search_term = use_state(|| String::new());

    // Initialize sort direction from local storage or default to newest first
    let episode_sort_direction = use_state(|| {
        let saved_preference = get_filter_preference("history");
        match saved_preference.as_deref() {
            Some("newest") => Some(HistorySortDirection::NewestFirst),
            Some("oldest") => Some(HistorySortDirection::OldestFirst),
            Some("shortest") => Some(HistorySortDirection::ShortestFirst),
            Some("longest") => Some(HistorySortDirection::LongestFirst),
            Some("title_az") => Some(HistorySortDirection::TitleAZ),
            Some("title_za") => Some(HistorySortDirection::TitleZA),
            _ => Some(HistorySortDirection::NewestFirst), // Default to newest first
        }
    });

    let show_completed = use_state(|| false); // Toggle for showing completed episodes only
    let show_in_progress = use_state(|| false); // Toggle for showing in-progress episodes only

    // Fetch episodes on component mount
    let loading_ep = loading.clone();
    {
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
                        match pod_req::call_get_user_history(&server_name, &api_key, &user_id).await
                        {
                            Ok(fetched_episodes) => {
                                let completed_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                dispatch.reduce_mut(move |state| {
                                    state.episode_history = Some(HistoryDataResponse {
                                        data: fetched_episodes,
                                    });
                                });
                                Dispatch::<EpisodeStatusState>::global().reduce_mut(move |state| {
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
            state.episode_history.clone(),
            episode_search_term.clone(),
            episode_sort_direction.clone(),
            show_completed.clone(),
            show_in_progress.clone(),
        ),
        |(history_eps, search, sort_dir, show_completed, show_in_progress)| {
            if let Some(history_episodes) = history_eps {
                let mut filtered = history_episodes
                    .data
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
                        HistorySortDirection::NewestFirst => {
                            b.listendate.as_deref().unwrap_or("").cmp(a.listendate.as_deref().unwrap_or(""))
                        }
                        HistorySortDirection::OldestFirst => {
                            a.listendate.as_deref().unwrap_or("").cmp(b.listendate.as_deref().unwrap_or(""))
                        }
                        HistorySortDirection::ShortestFirst => {
                            a.episodeduration.cmp(&b.episodeduration)
                        }
                        HistorySortDirection::LongestFirst => {
                            b.episodeduration.cmp(&a.episodeduration)
                        }
                        HistorySortDirection::TitleAZ => a.episodetitle.cmp(&b.episodetitle),
                        HistorySortDirection::TitleZA => b.episodetitle.cmp(&a.episodetitle),
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
                    html! { html! { <Loading/> } }
                } else {
                    html! {
                        <>
                            // Modern mobile-friendly filter bar with tab-style page title
                            <div class="mb-6 space-y-4 mt-4">
                                // Combined search and sort bar with tab-style title (seamless design)
                                <div class="flex gap-0 h-12 relative">
                                    // Tab-style page indicator
                                    <div class="page-tab-indicator">
                                        <i class="ph ph-clock-clockwise tab-icon"></i>
                                        {&i18n_history}
                                    </div>
                                    // Search input (left half)
                                    <div class="flex-1 relative">
                                        <input
                                            type="text"
                                            class="search-input"
                                            placeholder={i18n_search_listening_history.clone()}
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
                                                    set_filter_preference("history", &value);

                                                    match value.as_str() {
                                                        "newest" => episode_sort_direction.set(Some(HistorySortDirection::NewestFirst)),
                                                        "oldest" => episode_sort_direction.set(Some(HistorySortDirection::OldestFirst)),
                                                        "shortest" => episode_sort_direction.set(Some(HistorySortDirection::ShortestFirst)),
                                                        "longest" => episode_sort_direction.set(Some(HistorySortDirection::LongestFirst)),
                                                        "title_az" => episode_sort_direction.set(Some(HistorySortDirection::TitleAZ)),
                                                        "title_za" => episode_sort_direction.set(Some(HistorySortDirection::TitleZA)),
                                                        _ => episode_sort_direction.set(None),
                                                    }
                                                })
                                            }
                                        >
                                            <option value="newest" selected={get_filter_preference("history").unwrap_or_else(|| get_default_sort_direction().to_string()) == "newest"}>{&i18n_newest_first}</option>
                                            <option value="oldest" selected={get_filter_preference("history").unwrap_or_else(|| get_default_sort_direction().to_string()) == "oldest"}>{&i18n_oldest_first}</option>
                                            <option value="shortest" selected={get_filter_preference("history").unwrap_or_else(|| get_default_sort_direction().to_string()) == "shortest"}>{&i18n_shortest_first}</option>
                                            <option value="longest" selected={get_filter_preference("history").unwrap_or_else(|| get_default_sort_direction().to_string()) == "longest"}>{&i18n_longest_first}</option>
                                            <option value="title_az" selected={get_filter_preference("history").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_az"}>{&i18n_title_az}</option>
                                            <option value="title_za" selected={get_filter_preference("history").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_za"}>{&i18n_title_za}</option>
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
                                        <span class="text-sm font-medium">{&i18n_clear_all}</span>
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
                                        <span class="text-sm font-medium">{&i18n_completed}</span>
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
                                        <span class="text-sm font-medium">{&i18n_in_progress}</span>
                                    </button>
                                </div>
                            </div>

                            {
                                if let Some(_history_eps) = state.episode_history.clone() {
                                    let favorite_podcast_ids: std::collections::HashSet<i32> = state
                                        .podcast_feed_return_extra
                                        .as_ref()
                                        .and_then(|pr| pr.pods.as_ref())
                                        .map(|pods| {
                                            pods.iter()
                                                .filter(|p| p.is_favorite)
                                                .map(|p| p.podcastid)
                                                .collect()
                                        })
                                        .unwrap_or_default();

                                    let display_episodes: Vec<_> = (*filtered_episodes)
                                        .iter()
                                        .filter(|ep| {
                                            if !filter_state.favorites_only {
                                                return true;
                                            }
                                            favorite_podcast_ids.contains(&ep.podcastid)
                                        })
                                        .cloned()
                                        .collect();

                                    if display_episodes.is_empty() {
                                        empty_message(
                                            &i18n_no_episode_history_found,
                                            &i18n_no_episode_history_description
                                        )
                                    } else {
                                        html! {
                                            <div class="flex-grow overflow-y-auto">
                                                { for display_episodes.iter().map(|ep| html! {
                                                    <EpisodeListItem key={ep.episodeid} episode={ep.clone()} />
                                                }) }
                                            </div>
                                        }
                                    }
                                } else {
                                    empty_message(
                                        &i18n_no_episode_history_found,
                                        &i18n_no_episode_history_description
                                    )
                                }
                            }
                        </>
                    }
                }
            }
            <AudioPlayerBar />
        </div>
        <App_drawer />
        </>
    }
}
