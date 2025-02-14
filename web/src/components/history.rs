use super::app_drawer::App_drawer;
use super::gen_components::{
    empty_message, episode_item, on_shownotes_click, Search_nav, UseScrollToTop,
};
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::AppStateMsg;
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::login_requests::use_check_authentication;
use crate::requests::pod_req::{self, HistoryDataResponse};
use gloo_events::EventListener;
use web_sys::window;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;
// use crate::components::gen_funcs::check_auth;

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
    let (state, dispatch) = use_store::<AppState>();
    let history = BrowserHistory::new();

    // check_auth(effect_dispatch);

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let dropdown_open = use_state(|| false);
    let session_dispatch = _post_dispatch.clone();
    let session_state = post_state.clone();
    let loading = use_state(|| true);
    let active_modal = use_state(|| None::<i32>);
    let active_modal_clone = active_modal.clone();
    let on_modal_open = Callback::from(move |episode_id: i32| {
        active_modal_clone.set(Some(episode_id));
    });
    let active_modal_clone = active_modal.clone();
    let on_modal_close = Callback::from(move |_| {
        active_modal_clone.set(None);
    });

    let episode_search_term = use_state(|| String::new());
    let episode_sort_direction = use_state(|| Some(HistorySortDirection::NewestFirst)); // Default to newest first
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
    let container_height = use_state(|| "221px".to_string()); // Add this state

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
                                    "162px"
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

    let filtered_episodes = use_memo(
        (
            state.episode_history.clone(), // Changed from saved_episodes to episode_history
            episode_search_term.clone(),
            episode_sort_direction.clone(),
            show_completed.clone(),
            show_in_progress.clone(),
        ),
        |(history_eps, search, sort_dir, show_completed, show_in_progress)| {
            if let Some(history_episodes) = history_eps {
                let mut filtered = history_episodes
                    .data // Note: accessing .data instead of .episodes
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
                        HistorySortDirection::NewestFirst => {
                            b.episodepubdate.cmp(&a.episodepubdate)
                        }
                        HistorySortDirection::OldestFirst => {
                            a.episodepubdate.cmp(&b.episodepubdate)
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
    let show_in_prog_button = show_in_progress.clone();

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
                            <h1 class="text-2xl item_container-text font-bold text-center mb-6">{"History"}</h1>
                            </div>
                        }
                    }


                    {
                                            html! {
                                                <div class="flex justify-between items-center mb-4">
                                                    <div class="flex gap-4">
                                                        // Search input
                                                        <div class="filter-dropdown filter-button relative">
                                                            <input
                                                                type="text"
                                                                class="filter-input appearance-none pr-8"
                                                                placeholder="Search"
                                                                value={(*episode_search_term).clone()}
                                                                oninput={let episode_search_term = episode_search_term.clone();
                                                                    Callback::from(move |e: InputEvent| {
                                                                        if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                                            episode_search_term.set(input.value());
                                                                        }
                                                                    })
                                                                }
                                                            />
                                                            <i class="ph ph-magnifying-glass absolute right-2 top-1/2 -translate-y-1/2 pointer-events-none"></i>
                                                        </div>

                                                        // Filter buttons
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
                                                            class="filter-button font-medium py-2 px-2 rounded inline-flex items-center"
                                                        >
                                                            <i class="ph ph-broom text-2xl"></i>
                                                            <span class="text-lg ml-2 hidden md:inline">{"Clear"}</span>
                                                        </button>
                                                        <button
                                                            onclick={let show_completed = show_completed.clone();
                                                                Callback::from(move |_| {
                                                                    show_completed.set(!*show_completed);
                                                                    // Ensure only one filter is active at a time
                                                                    if *show_completed {
                                                                        show_in_prog_button.set(false);
                                                                    }
                                                                })
                                                            }
                                                            class={classes!(
                                                                "filter-button",
                                                                "font-medium",
                                                                "py-2",
                                                                "px-2",
                                                                "rounded",
                                                                "inline-flex",
                                                                "items-center",
                                                                if *show_completed { "bg-accent-color" } else { "" }
                                                            )}
                                                        >
                                                            <i class="ph ph-check-circle text-2xl"></i>
                                                            <span class="text-lg ml-2 hidden md:inline">{"Completed"}</span>
                                                        </button>

                                                        <button
                                                            onclick={let show_in_progress = show_in_progress.clone();
                                                                Callback::from(move |_| {
                                                                    show_in_progress.set(!*show_in_progress);
                                                                    // Ensure only one filter is active at a time
                                                                    if *show_in_progress {
                                                                        show_completed.set(false);
                                                                    }
                                                                })
                                                            }
                                                            class={classes!(
                                                                "filter-button",
                                                                "font-medium",
                                                                "py-2",
                                                                "px-2",
                                                                "rounded",
                                                                "inline-flex",
                                                                "items-center",
                                                                if *show_in_progress { "bg-accent-color" } else { "" }
                                                            )}
                                                        >
                                                            <i class="ph ph-hourglass-medium text-2xl"></i>
                                                            <span class="text-lg ml-2 hidden md:inline">{"In Progress"}</span>
                                                        </button>

                                                        // Sort dropdown
                                                        <div class="filter-dropdown font-medium rounded relative">
                                                            // Normal select for screens > 530px
                                                            <select
                                                                class="category-select appearance-none pr-8 hidden sm:block"
                                                                onchange={
                                                                    let episode_sort_direction = episode_sort_direction.clone();
                                                                    Callback::from(move |e: Event| {
                                                                        let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                                        let value = target.value();
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
                                                                <option value="newest" selected=true>{"Newest First"}</option>
                                                                <option value="oldest">{"Oldest First"}</option>
                                                                <option value="shortest">{"Shortest First"}</option>
                                                                <option value="longest">{"Longest First"}</option>
                                                                <option value="title_az">{"Title A to Z"}</option>
                                                                <option value="title_za">{"Title Z to A"}</option>
                                                            </select>

                                                            // Icon button with dropdown for screens <= 530px
                                                            <div class="block sm:hidden relative">
                                                                <select
                                                                    class="category-select appearance-none pr-8 pl-8 w-20"
                                                                    onchange={
                                                                        let episode_sort_direction = episode_sort_direction.clone();
                                                                        Callback::from(move |e: Event| {
                                                                            let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                                            let value = target.value();
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
                                                                    style="background-image: none;"
                                                                >
                                                                    <i class="ph ph-sort-ascending absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 text-2xl pointer-events-none"></i>
                                                                    <option value="newest" selected=true>{"Newest"}</option>
                                                                    <option value="oldest">{"Oldest"}</option>
                                                                    <option value="shortest">{"Shortest"}</option>
                                                                    <option value="longest">{"Longest"}</option>
                                                                    <option value="title_az">{"A -> Z"}</option>
                                                                    <option value="title_za">{"Z -> A"}</option>
                                                                </select>
                                                            </div>
                                                        </div>
                                                    </div>
                                                </div>
                                            }
                                        }

                        {
                            if let Some(_history_eps) = state.episode_history.clone() {
                                if (*filtered_episodes).is_empty() {
                                    empty_message(
                                        "No Episode History Found",
                                        "This one is pretty straightforward. You should get listening! Podcasts you listen to will show up here!."
                                    )
                                } else {

                                    (*filtered_episodes).iter().map(|episode| {
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
                                        let episode_is_youtube = episode.is_youtube.clone();
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
                                            Some(episode_is_youtube.clone()),
                                        );

                                        let on_shownotes_click = on_shownotes_click(
                                            history_clone.clone(),
                                            dispatch.clone(),
                                            Some(episode_id_for_closure.clone()),
                                            Some(String::from("history")),
                                            Some(String::from("history")),
                                            Some(String::from("history")),
                                            true,
                                            None,
                                            Some(episode_is_youtube),
                                        );

                                        let episode_url_for_ep_item = episode_url_clone.clone();
                                        let check_episode_id = &episode.episodeid.clone();
                                        let is_completed = state
                                            .completed_episodes
                                            .as_ref()
                                            .unwrap_or(&vec![])
                                            .contains(&check_episode_id);
                                        let episode_id_clone = Some(episode.episodeid).clone();
                                        let item = episode_item(
                                            Box::new(episode.clone()),
                                            sanitized_description,
                                            is_expanded,
                                            &format_release,
                                            on_play_pause,
                                            on_shownotes_click,
                                            toggle_expanded,
                                            episode_duration_clone,
                                            episode_listened_clone,
                                            "history",
                                            Callback::from(|_| {}),
                                            false,
                                            episode_url_for_ep_item,
                                            is_completed,
                                            *active_modal == episode_id_clone,
                                            on_modal_open.clone(),
                                            on_modal_close.clone(),
                                            (*container_height).clone(),
                                            is_current_episode,
                                            is_playing,
                                        );

                                        item
                                    }).collect::<Html>()
                                }

                            } else {
                                empty_message(
                                    "No Episode History Found",
                                    "This one is pretty straightforward. You should get listening! Podcasts you listen to will show up here!."
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
