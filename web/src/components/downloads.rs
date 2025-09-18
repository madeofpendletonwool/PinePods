use super::app_drawer::App_drawer;
use super::gen_components::{
    download_episode_item, empty_message, on_shownotes_click, FallbackImage, Search_nav,
    UseScrollToTop,
};
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::pod_req::{
    call_bulk_delete_downloaded_episodes, call_get_episode_downloads, call_get_podcasts,
    BulkEpisodeActionRequest, EpisodeDownload, EpisodeDownloadResponse, Podcast, PodcastResponse,
};
use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::wasm_bindgen;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

fn group_episodes_by_podcast(episodes: Vec<EpisodeDownload>) -> HashMap<i32, Vec<EpisodeDownload>> {
    let mut grouped: HashMap<i32, Vec<EpisodeDownload>> = HashMap::new();
    for episode in episodes {
        grouped
            .entry(episode.podcastid)
            .or_insert_with(Vec::new)
            .push(episode);
    }
    grouped
}

#[function_component(Downloads)]
pub fn downloads() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();

    let expanded_state = use_state(HashMap::new);
    let show_modal = use_state(|| false);

    // Filter state for episodes
    let episode_search_term = use_state(|| String::new());
    let show_completed = use_state(|| false);
    let show_in_progress = use_state(|| false);
    let show_clonedal = show_modal.clone();
    let show_clonedal2 = show_modal.clone();
    let on_modal_open = Callback::from(move |_: MouseEvent| show_clonedal.set(true));

    let on_modal_close = Callback::from(move |_: MouseEvent| show_clonedal2.set(false));

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let page_state = use_state(|| PageState::Normal);
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let loading = use_state(|| true);

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
                        match call_get_podcasts(&server_name, &api_key, &user_id).await {
                            Ok(fetched_podcasts) => {
                                dispatch.reduce_mut(move |state| {
                                    state.podcast_feed_return = Some(PodcastResponse {
                                        pods: Some(fetched_podcasts),
                                    });
                                });
                            }
                            Err(e) => web_sys::console::log_1(
                                &format!("Unable to parse Podcasts: {:?}", &e).into(),
                            ),
                        }

                        match call_get_episode_downloads(&server_name, &api_key, &user_id).await {
                            Ok(fetched_episodes) => {
                                let completed_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                dispatch.reduce_mut(move |state| {
                                    state.downloaded_episodes = Some(EpisodeDownloadResponse {
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

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Delete,
        Normal,
    }

    // Define the function to Enter Delete Mode
    let delete_mode_enable = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Delete);
        })
    };

    // Define the function to Exit Delete Mode
    let delete_mode_disable = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Normal);
        })
    };

    let on_checkbox_change = {
        let dispatch = dispatch.clone();
        Callback::from(move |episode_id: i32| {
            dispatch.reduce_mut(move |state| {
                // If the episode is already selected, remove it
                // If it's not selected, add it
                if state.selected_episodes_for_deletion.contains(&episode_id) {
                    state.selected_episodes_for_deletion.remove(&episode_id);
                } else {
                    state.selected_episodes_for_deletion.insert(episode_id);
                }
            });
        })
    };

    let delete_selected_episodes = {
        let dispatch = dispatch.clone();
        let page_state = page_state.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();

        Callback::from(move |_: MouseEvent| {
            let dispatch_cloned = dispatch.clone();
            let page_state_cloned = page_state.clone();
            let server_name_cloned = server_name.clone().unwrap();
            let api_key_cloned = api_key.clone().unwrap();
            let user_id_cloned = user_id.unwrap();

            dispatch.reduce_mut(move |state| {
                let selected_episodes: Vec<i32> = state
                    .selected_episodes_for_deletion
                    .iter()
                    .cloned()
                    .collect();
                let is_youtube = state.selected_is_youtube.unwrap_or(false);

                // Clear the selected episodes for deletion right away to prevent re-deletion
                state.selected_episodes_for_deletion.clear();

                if !selected_episodes.is_empty() {
                    let bulk_request = BulkEpisodeActionRequest {
                        episode_ids: selected_episodes.clone(),
                        user_id: user_id_cloned,
                        is_youtube: Some(is_youtube),
                    };

                    let dispatch_for_future = dispatch_cloned.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_bulk_delete_downloaded_episodes(
                            &server_name_cloned,
                            &api_key_cloned,
                            &bulk_request,
                        )
                        .await
                        {
                            Ok(success_message) => {
                                dispatch_for_future.reduce_mut(|state| {
                                    // Remove deleted episodes from the state
                                    if let Some(downloaded_episodes) =
                                        &mut state.downloaded_episodes
                                    {
                                        downloaded_episodes.episodes.retain(|ep| {
                                            !selected_episodes.contains(&ep.episodeid)
                                        });
                                    }
                                    state.info_message = Some(success_message);
                                });
                            }
                            Err(e) => {
                                dispatch_for_future.reduce_mut(|state| {
                                    state.error_message =
                                        Some(format!("Failed to delete episodes: {}", e));
                                });
                            }
                        }
                    });
                }

                page_state_cloned.set(PageState::Normal);
            });
        })
    };

    let is_delete_mode = **page_state.borrow() == PageState::Delete; // Add this line

    let toggle_pod_expanded = {
        let expanded_state = expanded_state.clone();
        Callback::from(move |podcast_id: i32| {
            expanded_state.set({
                let mut new_state = (*expanded_state).clone();
                new_state.insert(podcast_id, !new_state.get(&podcast_id).unwrap_or(&false));
                new_state
            });
        })
    };

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
                                // Tab-style page indicator with compact action buttons
                                <div class="relative mb-6">
                                    // <div class="page-tab-indicator">
                                    //     <i class="ph ph-download tab-icon"></i>
                                    //     {"Downloads"}
                                    // </div>
                                    <div class="flex gap-2 justify-end">
                                        {
                                            if **page_state.borrow() == PageState::Normal {
                                                html! {
                                                    <button class="filter-chip"
                                                        onclick={delete_mode_enable.clone()}>
                                                        <i class="ph ph-lasso text-lg"></i>
                                                        <span class="text-sm font-medium">{"Select"}</span>
                                                    </button>
                                                }
                                            } else {
                                                html! {
                                                    <>
                                                        <button class="filter-chip"
                                                            onclick={delete_mode_disable.clone()}>
                                                            <i class="ph ph-prohibit text-lg"></i>
                                                            <span class="text-sm font-medium">{"Cancel"}</span>
                                                        </button>
                                                        <button class="filter-chip filter-chip-alert"
                                                            onclick={delete_selected_episodes.clone()}>
                                                            <i class="ph ph-trash text-lg"></i>
                                                            <span class="text-sm font-medium">{"Delete"}</span>
                                                        </button>
                                                    </>
                                                }
                                            }
                                        }
                                    </div>
                                </div>

                                // Modern mobile-friendly filter bar
                                <div class="mb-6 space-y-4">
                                    // Search bar (full width with proper rounded corners)
                                    <div class="w-full">
                                        <div class="relative">
                                            <input
                                                type="text"
                                                class="downloads-search-input"
                                                placeholder="Search downloaded episodes..."
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
                                                    if *show_in_progress {
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
                                                    if *show_completed {
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
                            </div>
                        }
                    }

                    {
                    if let Some(download_eps) = state.downloaded_episodes.clone() {
                        let int_download_eps = download_eps.clone();
                            let render_state = post_state.clone();
                            let dispatch_cloned = dispatch.clone();

                            if int_download_eps.episodes.is_empty() {
                                // Render "No Recent Episodes Found" if episodes list is empty
                                empty_message(
                                    "No Downloaded Episodes Found",
                                    "This is where episode downloads will appear. To download an episode you can open the context menu on an episode and select Download Episode. It will then download to the server and show up here!"
                                )
                            } else {
                                let grouped_episodes = group_episodes_by_podcast(int_download_eps.episodes);

                                // Create filtered episodes
                                let filtered_grouped_episodes = {
                                    let mut filtered_map: HashMap<i32, Vec<EpisodeDownload>> = HashMap::new();

                                    for (podcast_id, episodes) in grouped_episodes.iter() {
                                        let filtered_episodes: Vec<EpisodeDownload> = episodes.iter()
                                            .filter(|episode| {
                                                // Search filter
                                                let matches_search = if !episode_search_term.is_empty() {
                                                    episode.episodetitle.to_lowercase().contains(&episode_search_term.to_lowercase())
                                                } else {
                                                    true
                                                };

                                                // Completion filter
                                                let matches_completion = if *show_completed && *show_in_progress {
                                                    true // Both filters active = show all
                                                } else if *show_completed {
                                                    episode.completed
                                                } else if *show_in_progress {
                                                    !episode.completed && episode.listenduration.is_some() && episode.listenduration.unwrap() > 0
                                                } else {
                                                    true // No filters = show all
                                                };

                                                matches_search && matches_completion
                                            })
                                            .cloned()
                                            .collect();

                                        if !filtered_episodes.is_empty() {
                                            filtered_map.insert(*podcast_id, filtered_episodes);
                                        }
                                    }

                                    filtered_map
                                };

                                html! {
                                    <>
                                        { for state.podcast_feed_return.as_ref().unwrap().pods.as_ref().unwrap().iter().filter_map(|podcast| {
                                            let episodes = filtered_grouped_episodes.get(&podcast.podcastid).unwrap_or(&Vec::new()).clone();
                                            if episodes.is_empty() {
                                                None
                                            } else {
                                                let downloaded_episode_count = episodes.len();
                                                let is_expanded = *expanded_state.get(&podcast.podcastid).unwrap_or(&false);
                                                let toggle_expanded_closure = {
                                                    let podcast_id = podcast.podcastid;
                                                    toggle_pod_expanded.reform(move |_| podcast_id)
                                                };

                                                let render_state_cloned = render_state.clone();
                                                let dispatch_cloned_cloned = dispatch_cloned.clone();
                                                let audio_state_cloned = audio_state.clone();
                                                let audio_dispatch_cloned = audio_dispatch.clone();
                                                let on_checkbox_change_cloned = on_checkbox_change.clone();

                                                Some(render_podcast_with_episodes(
                                                    podcast,
                                                    episodes,
                                                    downloaded_episode_count,
                                                    is_expanded,
                                                    toggle_expanded_closure,
                                                    render_state_cloned,
                                                    dispatch_cloned_cloned,
                                                    is_delete_mode,
                                                    audio_state_cloned,
                                                    desc_state.clone(),
                                                    desc_dispatch.clone(),
                                                    audio_dispatch_cloned,
                                                    on_checkbox_change_cloned,
                                                    *show_modal,
                                                    on_modal_open.clone(),
                                                    on_modal_close.clone(),
                                                ))
                                            }
                                        }) }
                                    </>
                                }

                            }


                        } else {
                            empty_message(
                                "No Episode Downloads Found",
                                "This is where episode downloads will appear. To download an episode you can open the context menu on an episode and select Download Episode. It will then download to the server and show up here!"
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

pub fn render_podcast_with_episodes(
    podcast: &Podcast,
    episodes: Vec<EpisodeDownload>,
    downloaded_episode_count: usize,
    is_expanded: bool,
    toggle_pod_expanded: Callback<MouseEvent>,
    state: Rc<AppState>,
    dispatch: Dispatch<AppState>,
    is_delete_mode: bool,
    audio_state: Rc<UIState>,
    desc_rc: Rc<ExpandedDescriptions>,
    desc_state: Dispatch<ExpandedDescriptions>,
    audio_dispatch: Dispatch<UIState>,
    on_checkbox_change: Callback<i32>,
    show_modal: bool,
    on_modal_open: Callback<MouseEvent>,
    on_modal_close: Callback<MouseEvent>,
) -> Html {
    let history_clone = BrowserHistory::new();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let on_podcast_checkbox_change = {
        let episodes = episodes.clone();
        let on_checkbox_change = on_checkbox_change.clone();
        let dispatch_clone = dispatch.clone();
        let episode_ids: Vec<i32> = episodes.iter().map(|ep| ep.episodeid).collect();

        Callback::from(move |e: Event| {
            let is_checked = e
                .target_dyn_into::<web_sys::HtmlInputElement>()
                .map(|input| input.checked())
                .unwrap_or(false);

            // Access current state during callback execution
            let selected_episodes = &dispatch_clone.get().selected_episodes_for_deletion;

            for episode_id in &episode_ids {
                let is_episode_selected = selected_episodes.contains(episode_id);
                if is_checked && !is_episode_selected {
                    // Select episodes that aren't already selected
                    on_checkbox_change.emit(*episode_id);
                } else if !is_checked && is_episode_selected {
                    // Deselect episodes that are currently selected
                    on_checkbox_change.emit(*episode_id);
                }
            }
        })
    };
    html! {
        <div key={podcast.podcastid}>
            <div class="podcast-dropdown-header">
                <div class="podcast-dropdown-content" onclick={toggle_pod_expanded}>
                    {if is_delete_mode {
                        html! {
                            <div onclick={|e: MouseEvent| e.stop_propagation()}>
                                <input
                                    type="checkbox"
                                    class="podcast-dropdown-checkbox"
                                    onchange={on_podcast_checkbox_change}
                                />
                            </div>
                        }
                    } else {
                        html! {}
                    }}

                    <FallbackImage
                        src={podcast.artworkurl.clone().unwrap()}
                        alt={format!("Cover for {}", podcast.podcastname.clone())}
                        class="podcast-dropdown-image"
                    />

                    <div class="podcast-dropdown-info">
                        <p class="podcast-dropdown-title item_container-text">
                            { &podcast.podcastname }
                        </p>
                        <p class="podcast-dropdown-count item_container-text">
                            { format!("{} Downloaded Episodes", downloaded_episode_count) }
                        </p>
                    </div>

                    <div class={classes!("podcast-dropdown-arrow", is_expanded.then(|| "expanded"))}>
                        <i class="ph ph-caret-down text-2xl"></i>
                    </div>
                </div>
            </div>

            { if is_expanded {
                html! {
                    <div class="podcast-episodes-container expanded">
                        <div class="podcast-episodes-inner">
                            { for episodes.into_iter().map(|episode| {
                                let id_string = &episode.episodeid.to_string();

                                let dispatch = dispatch.clone();

                                let episode_url_clone = episode.episodeurl.clone();
                                let episode_title_clone = episode.episodetitle.clone();
                                let episode_description_clone = episode.episodedescription.clone();
                                let episode_artwork_clone = episode.episodeartwork.clone();
                                let episode_duration_clone = episode.episodeduration.clone();
                                let episode_id_clone = episode.episodeid.clone();
                                let episode_listened_clone = episode.listenduration.clone();
                                let episode_is_youtube = Some(episode.is_youtube.clone());
                                let _completed = episode.completed;
                                let desc_expanded = desc_rc.expanded_descriptions.contains(id_string);
                                #[wasm_bindgen]
                                extern "C" {
                                    #[wasm_bindgen(js_namespace = window)]
                                    fn toggleDescription(guid: &str, expanded: bool);
                                }
                                let toggle_expanded = {
                                    let desc_dispatch = desc_state.clone();
                                    let episode_guid = episode.episodeid.clone().to_string();

                                    Callback::from(move |_: MouseEvent| {
                                        let guid = episode_guid.clone();
                                        desc_dispatch.reduce_mut(move |state| {
                                            if state.expanded_descriptions.contains(&guid) {
                                                state.expanded_descriptions.remove(&guid); // Collapse the description
                                                toggleDescription(&guid, false); // Call JavaScript function
                                            } else {
                                                state.expanded_descriptions.insert(guid.clone()); // Expand the description
                                                toggleDescription(&guid, true); // Call JavaScript function
                                            }
                                        });
                                    })
                                };

                                let episode_url_for_closure = episode_url_clone.clone();
                                let episode_title_for_closure = episode_title_clone.clone();
                                let episode_description_for_closure = episode_description_clone.clone();
                                let episode_artwork_for_closure = episode_artwork_clone.clone();
                                let episode_duration_for_closure = episode_duration_clone.clone();
                                let listener_duration_for_closure = episode_listened_clone.clone();
                                let episode_id_for_closure = episode_id_clone.clone();
                                let user_id_play = user_id.clone();
                                let server_name_play = server_name.clone();
                                let api_key_play = api_key.clone();
                                let audio_dispatch = audio_dispatch.clone();
                                let is_local = Option::from(true);

                                let is_current_episode = audio_state
                                                                .currently_playing
                                                                .as_ref()
                                                                .map_or(false, |current| current.episode_id == episode.episodeid);
                                let is_playing = audio_state.audio_playing.unwrap_or(false);

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
                                    is_local,
                                    episode_is_youtube,
                                );

                                let on_shownotes_click = on_shownotes_click(
                                    history_clone.clone(),
                                    dispatch.clone(),
                                    Some(episode_id_for_closure.clone()),
                                    Some(String::from("Not needed")),
                                    Some(String::from("Not needed")),
                                    Some(String::from("Not needed")),
                                    true,
                                    None,
                                    episode_is_youtube,
                                );

                                let on_checkbox_change_cloned = on_checkbox_change.clone();
                                let episode_url_for_ep_item = episode_url_clone.clone();
                                let sanitized_description =
                                    sanitize_html_with_blank_target(&episode.episodedescription.clone());

                                let check_episode_id = &episode.episodeid.clone();
                                let is_completed = state
                                    .completed_episodes
                                    .as_ref()
                                    .unwrap_or(&vec![])
                                    .contains(&check_episode_id);
                                download_episode_item(
                                    Box::new(episode),
                                    sanitized_description.clone(),
                                    desc_expanded,
                                    &format_release,
                                    on_play_pause,
                                    on_shownotes_click,
                                    toggle_expanded,
                                    episode_duration_clone,
                                    episode_listened_clone,
                                    "downloads",
                                    on_checkbox_change_cloned, // Add this line
                                    is_delete_mode, // Add this line
                                    episode_url_for_ep_item,
                                    is_completed,
                                    show_modal,
                                    on_modal_open.clone(),
                                    on_modal_close.clone(),
                                    is_current_episode,
                                    is_playing,
                                    state.clone()
                                )
                            }) }
                        </div>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
