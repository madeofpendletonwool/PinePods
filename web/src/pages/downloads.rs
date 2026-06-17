use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, EpisodeNavigationState, NotificationState, UIState};
use crate::components::context_menu_button::PageType;
use crate::components::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;

use crate::components::episode_list_view::EpisodeListView;
use crate::components::virtual_list::ScrollSource;
use crate::requests::episode::Episode;
use crate::requests::pod_req::{
    call_bulk_delete_downloaded_episodes, call_get_podcast_download_summary,
    call_get_podcast_downloads_paged, BulkEpisodeActionRequest, PodcastDownloadSummary,
};

use i18nrs::yew::use_translation;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::rc::Rc;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

/// Collapse the two mutually-exclusive completion chips into the backend's filter vocabulary
/// ("all" | "completed" | "in_progress"). Kept identical to the values the paged/summary
/// download endpoints understand.
fn downloads_filter(show_completed: bool, show_in_progress: bool) -> &'static str {
    if show_completed {
        "completed"
    } else if show_in_progress {
        "in_progress"
    } else {
        "all"
    }
}

#[derive(Clone, PartialEq)]
struct PodcastEpisodeState {
    episodes: Vec<Episode>,
    offset: i64,
    total: i64,
    loading_more: bool,
}

impl PodcastEpisodeState {
    fn new() -> Self {
        Self {
            episodes: Vec::new(),
            offset: 0,
            total: 0,
            loading_more: false,
        }
    }
}

#[function_component(Downloads)]
pub fn downloads() -> Html {
    let (i18n, _) = use_translation();
    let (_state, dispatch) = use_store::<AppState>();
    let expanded_state: UseStateHandle<HashMap<i32, bool>> = use_state(HashMap::new);
    let per_podcast_state: UseStateHandle<HashMap<i32, PodcastEpisodeState>> = use_state(HashMap::new);
    let podcast_summaries: UseStateHandle<Vec<PodcastDownloadSummary>> = use_state(Vec::new);

    let i18n_select = i18n.t("downloads.select").to_string();
    let i18n_cancel = i18n.t("common.cancel").to_string();
    let i18n_delete = i18n.t("common.delete").to_string();
    let i18n_clear_all = i18n.t("downloads.clear_all").to_string();
    let i18n_completed = i18n.t("downloads.completed").to_string();
    let i18n_in_progress = i18n.t("downloads.in_progress").to_string();
    let i18n_search_downloaded_episodes =
        i18n.t("downloads.search_downloaded_episodes").to_string();
    let _i18n_no_downloaded_episodes_found =
        i18n.t("downloads.no_downloaded_episodes_found").to_string();
    let i18n_no_downloaded_episodes_description = i18n
        .t("downloads.no_downloaded_episodes_description")
        .to_string();
    let i18n_no_episode_downloads_found =
        i18n.t("downloads.no_episode_downloads_found").to_string();
    let i18n_load_more = i18n.t("downloads.load_more").to_string();

    let episode_search_term = use_state(|| String::new());
    // Debounced view of episode_search_term. Keystrokes update episode_search_term immediately
    // for input responsiveness; a 300 ms timer copies it into debounced_search_term, which the
    // backend-reload effect watches. Search/filter run on the backend so the podcast list and
    // its (paginated) episodes filter correctly even when episodes aren't loaded yet.
    let debounced_search_term = use_state(|| String::new());
    let show_completed = use_state(|| false);
    let show_in_progress = use_state(|| false);

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
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

    // Debounce the raw search input: a 300 ms timer copies episode_search_term into
    // debounced_search_term, and only the latter feeds the backend-reload effect. Otherwise
    // every keystroke would fire its own request.
    {
        let debounced_search_term = debounced_search_term.clone();
        let term = (*episode_search_term).clone();
        use_effect_with(term, move |t| {
            let t = t.clone();
            let debounced = debounced_search_term.clone();
            let timeout = gloo_timers::callback::Timeout::new(300, move || {
                debounced.set(t);
            });
            move || drop(timeout)
        });
    }

    // Load (and reload) podcast summaries whenever auth, the debounced search term, or the
    // completion filter changes. The summary endpoint applies the same search/filter as the
    // paged endpoint, so the podcast list narrows to relevant podcasts and the counts reflect
    // matching episodes. Any currently-expanded podcasts are refetched (sequentially, then set
    // once) so their loaded episodes stay consistent with the active search/filter.
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
        let podcast_summaries = podcast_summaries.clone();
        let per_podcast_state = per_podcast_state.clone();
        let expanded_state = expanded_state.clone();
        let search_dep = (*debounced_search_term).clone();
        let show_completed_dep = *show_completed;
        let show_in_progress_dep = *show_in_progress;

        use_effect_with(
            (
                api_key.clone(),
                user_id.clone(),
                server_name.clone(),
                search_dep,
                show_completed_dep,
                show_in_progress_dep,
            ),
            move |(api_key, user_id, server_name, search, show_completed, show_in_progress)| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let podcast_summaries = podcast_summaries.clone();
                    let per_podcast_state = per_podcast_state.clone();
                    let search = search.clone();
                    let filter = downloads_filter(*show_completed, *show_in_progress).to_string();
                    // Snapshot which podcasts are currently expanded so we can refetch their
                    // first page with the new search/filter.
                    let expanded_ids: Vec<i32> = (*expanded_state)
                        .iter()
                        .filter_map(|(id, open)| if *open { Some(*id) } else { None })
                        .collect();
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_get_podcast_download_summary(
                            &server_name,
                            &api_key,
                            &user_id,
                            &search,
                            &filter,
                        )
                        .await
                        {
                            Ok(response) => {
                                podcast_summaries.set(response.podcasts);
                                loading_ep.set(false);
                            }
                            Err(e) => {
                                error_clone.set(Some(e.to_string()));
                                loading_ep.set(false);
                            }
                        }

                        // Refetch expanded podcasts sequentially and commit once to avoid
                        // concurrent set() calls clobbering each other.
                        if !expanded_ids.is_empty() {
                            let mut new_state: HashMap<i32, PodcastEpisodeState> = HashMap::new();
                            for pid in expanded_ids {
                                if let Ok(page) = call_get_podcast_downloads_paged(
                                    &server_name,
                                    &api_key,
                                    &user_id,
                                    pid,
                                    PAGE_SIZE,
                                    0,
                                    &search,
                                    &filter,
                                )
                                .await
                                {
                                    new_state.insert(
                                        pid,
                                        PodcastEpisodeState {
                                            offset: page.episodes.len() as i64,
                                            total: page.total,
                                            episodes: page.episodes,
                                            loading_more: false,
                                        },
                                    );
                                }
                            }
                            per_podcast_state.set(new_state);
                        } else {
                            // No podcasts open — drop any stale per-podcast episodes so the next
                            // expand refetches with the current search/filter.
                            if !per_podcast_state.is_empty() {
                                per_podcast_state.set(HashMap::new());
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    #[derive(Clone, PartialEq)]
    enum PageState {
        Delete,
        Normal,
    }

    let delete_mode_enable = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Delete);
        })
    };

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
        let per_podcast_state = per_podcast_state.clone();

        Callback::from(move |_: MouseEvent| {
            let _dispatch_cloned = dispatch.clone();
            let page_state_cloned = page_state.clone();
            let server_name_cloned = server_name.clone().unwrap();
            let api_key_cloned = api_key.clone().unwrap();
            let user_id_cloned = user_id.unwrap();
            let per_podcast_state_cloned = per_podcast_state.clone();

            dispatch.reduce_mut(move |state| {
                let selected_episode_ids: Vec<i32> = state
                    .selected_episodes_for_deletion
                    .iter()
                    .cloned()
                    .collect();
                let is_youtube = Dispatch::<EpisodeNavigationState>::global().get().selected_is_youtube;

                state.selected_episodes_for_deletion.clear();

                if !selected_episode_ids.is_empty() {
                    let bulk_request = BulkEpisodeActionRequest {
                        episode_ids: selected_episode_ids.clone(),
                        user_id: user_id_cloned,
                        is_youtube: Some(is_youtube),
                    };

                    wasm_bindgen_futures::spawn_local(async move {
                        match call_bulk_delete_downloaded_episodes(
                            &server_name_cloned,
                            &api_key_cloned,
                            &bulk_request,
                        )
                        .await
                        {
                            Ok(success_message) => {
                                // Remove deleted episodes from per-podcast state
                                let mut new_state = (*per_podcast_state_cloned).clone();
                                for podcast_state in new_state.values_mut() {
                                    podcast_state.episodes.retain(|ep| !selected_episode_ids.contains(&ep.episodeid));
                                    podcast_state.total -= selected_episode_ids.len() as i64;
                                    if podcast_state.total < 0 {
                                        podcast_state.total = 0;
                                    }
                                    podcast_state.offset = podcast_state.episodes.len() as i64;
                                }
                                per_podcast_state_cloned.set(new_state);

                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.info_message = Some(success_message);
                                });
                            }
                            Err(e) => {
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
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

    let is_delete_mode = **page_state.borrow() == PageState::Delete;

    // Toggle expand and trigger lazy-load on first expand
    let toggle_pod_expanded = {
        let expanded_state = expanded_state.clone();
        let per_podcast_state = per_podcast_state.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let debounced_search_term = debounced_search_term.clone();
        let show_completed = show_completed.clone();
        let show_in_progress = show_in_progress.clone();

        Callback::from(move |podcast_id: i32| {
            let currently_expanded = *expanded_state.get(&podcast_id).unwrap_or(&false);
            let new_expanded = !currently_expanded;
            let search = (*debounced_search_term).clone();
            let filter = downloads_filter(*show_completed, *show_in_progress).to_string();

            // Update expansion state
            expanded_state.set({
                let mut new_state = (*expanded_state).clone();
                new_state.insert(podcast_id, new_expanded);
                new_state
            });

            // On first expand, load the first page of episodes
            if new_expanded && !per_podcast_state.contains_key(&podcast_id) {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let per_podcast_state = per_podcast_state.clone();

                    // Mark loading
                    {
                        let mut new_state = (*per_podcast_state).clone();
                        new_state.insert(podcast_id, PodcastEpisodeState {
                            loading_more: true,
                            ..PodcastEpisodeState::new()
                        });
                        per_podcast_state.set(new_state);
                    }

                    wasm_bindgen_futures::spawn_local(async move {
                        match call_get_podcast_downloads_paged(&server_name, &api_key, &user_id, podcast_id, PAGE_SIZE, 0, &search, &filter).await {
                            Ok(page) => {
                                let mut new_state = (*per_podcast_state).clone();
                                new_state.insert(podcast_id, PodcastEpisodeState {
                                    offset: page.episodes.len() as i64,
                                    total: page.total,
                                    episodes: page.episodes,
                                    loading_more: false,
                                });
                                per_podcast_state.set(new_state);
                            }
                            Err(e) => {
                                web_sys::console::log_1(&format!("Failed to load episodes: {:?}", e).into());
                                let mut new_state = (*per_podcast_state).clone();
                                new_state.insert(podcast_id, PodcastEpisodeState::new());
                                per_podcast_state.set(new_state);
                            }
                        }
                    });
                }
            }
        })
    };

    // Load-more callback per podcast
    let load_more = {
        let per_podcast_state = per_podcast_state.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let debounced_search_term = debounced_search_term.clone();
        let show_completed = show_completed.clone();
        let show_in_progress = show_in_progress.clone();

        Callback::from(move |podcast_id: i32| {
            if let (Some(api_key), Some(user_id), Some(server_name)) =
                (api_key.clone(), user_id.clone(), server_name.clone())
            {
                let search = (*debounced_search_term).clone();
                let filter = downloads_filter(*show_completed, *show_in_progress).to_string();
                let current_offset = per_podcast_state
                    .get(&podcast_id)
                    .map(|s| s.offset)
                    .unwrap_or(0);
                let is_loading = per_podcast_state
                    .get(&podcast_id)
                    .map(|s| s.loading_more)
                    .unwrap_or(false);

                if is_loading {
                    return;
                }

                let per_podcast_state = per_podcast_state.clone();

                // Mark loading_more
                {
                    let mut new_state = (*per_podcast_state).clone();
                    if let Some(pod_state) = new_state.get_mut(&podcast_id) {
                        pod_state.loading_more = true;
                    }
                    per_podcast_state.set(new_state);
                }

                wasm_bindgen_futures::spawn_local(async move {
                    match call_get_podcast_downloads_paged(&server_name, &api_key, &user_id, podcast_id, PAGE_SIZE, current_offset, &search, &filter).await {
                        Ok(page) => {
                            let mut new_state = (*per_podcast_state).clone();
                            if let Some(pod_state) = new_state.get_mut(&podcast_id) {
                                pod_state.episodes.extend(page.episodes);
                                pod_state.offset = pod_state.episodes.len() as i64;
                                pod_state.total = page.total;
                                pod_state.loading_more = false;
                            }
                            per_podcast_state.set(new_state);
                        }
                        Err(e) => {
                            web_sys::console::log_1(&format!("Failed to load more episodes: {:?}", e).into());
                            let mut new_state = (*per_podcast_state).clone();
                            if let Some(pod_state) = new_state.get_mut(&podcast_id) {
                                pod_state.loading_more = false;
                            }
                            per_podcast_state.set(new_state);
                        }
                    }
                });
            }
        })
    };

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
                if *loading {
                    {
                        html! { <Loading/> }
                    }
                } else {
                    {
                        html! {
                            <div>
                                <div class="relative mb-6">
                                    <div class="flex gap-2 justify-end">
                                        {
                                            if **page_state.borrow() == PageState::Normal {
                                                html! {
                                                    <button class="sp-chip"
                                                        onclick={delete_mode_enable.clone()}>
                                                        <i class="ph ph-lasso"></i>
                                                        <span>{&i18n_select}</span>
                                                    </button>
                                                }
                                            } else {
                                                html! {
                                                    <>
                                                        <button class="sp-chip"
                                                            onclick={delete_mode_disable.clone()}>
                                                            <i class="ph ph-prohibit"></i>
                                                            <span>{&i18n_cancel}</span>
                                                        </button>
                                                        <button class="sp-chip is-alert"
                                                            onclick={delete_selected_episodes.clone()}>
                                                            <i class="ph ph-trash"></i>
                                                            <span>{&i18n_delete}</span>
                                                        </button>
                                                    </>
                                                }
                                            }
                                        }
                                    </div>
                                </div>

                                <div class="pfb-section">
                                    <div class="pfb-bar">
                                        <div class="sp-input">
                                            <i class="ph ph-download-simple sp-search-ico"></i>
                                            <input
                                                type="text"
                                                placeholder={i18n_search_downloaded_episodes.clone()}
                                                value={(*episode_search_term).clone()}
                                                oninput={let episode_search_term = episode_search_term.clone();
                                                    Callback::from(move |e: InputEvent| {
                                                        if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                            episode_search_term.set(input.value());
                                                        }
                                                    })
                                                }
                                            />
                                        </div>
                                    </div>
                                    <div class="sp-chips pfb-chips">
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
                                            class="sp-chip"
                                        >
                                            <i class="ph ph-broom"></i>
                                            <span>{&i18n_clear_all}</span>
                                        </button>
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
                                            class={classes!("sp-chip", if *show_completed { "is-active" } else { "" })}
                                        >
                                            <i class="ph ph-check-circle"></i>
                                            <span>{&i18n_completed}</span>
                                        </button>
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
                                            class={classes!("sp-chip", if *show_in_progress { "is-active" } else { "" })}
                                        >
                                            <i class="ph ph-hourglass-medium"></i>
                                            <span>{&i18n_in_progress}</span>
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }
                    }

                    {
                        if !podcast_summaries.is_empty() {
                            let dispatch_cloned = dispatch.clone();

                            let visible_summaries: Vec<PodcastDownloadSummary> = podcast_summaries
                                .iter()
                                .cloned()
                                .collect();

                            html! {
                                <>
                                    { for visible_summaries.into_iter().map(|summary| {
                                        let podcast_id = summary.podcastid;
                                        let is_expanded = *expanded_state.get(&podcast_id).unwrap_or(&false);

                                        let pod_state = per_podcast_state.get(&podcast_id).cloned();
                                        let episodes_loaded = pod_state.as_ref().map(|s| s.episodes.clone()).unwrap_or_default();
                                        let total = pod_state.as_ref().map(|s| s.total).unwrap_or(summary.episode_count);
                                        let offset = pod_state.as_ref().map(|s| s.offset).unwrap_or(0);
                                        let loading_more = pod_state.as_ref().map(|s| s.loading_more).unwrap_or(false);

                                        // Search/filter run on the backend (across the full
                                        // paginated set), so the loaded episodes are already the
                                        // filtered ones — no client-side re-filtering needed.
                                        let filtered_episodes: Vec<Episode> = episodes_loaded;

                                        let toggle_expanded_closure = {
                                            toggle_pod_expanded.reform(move |_: MouseEvent| podcast_id)
                                        };

                                        let load_more_closure = {
                                            load_more.reform(move |_: MouseEvent| podcast_id)
                                        };

                                        let has_more = offset < total;
                                        let on_checkbox_change_cloned = on_checkbox_change.clone();

                                        render_podcast_with_episodes(
                                            &summary,
                                            filtered_episodes,
                                            total,
                                            is_expanded,
                                            toggle_expanded_closure,
                                            dispatch_cloned.clone(),
                                            is_delete_mode,
                                            on_checkbox_change_cloned,
                                            loading_more,
                                            has_more,
                                            load_more_closure,
                                            i18n_load_more.clone(),
                                        )
                                    }) }
                                </>
                            }
                        } else {
                            empty_message(
                                &i18n_no_episode_downloads_found,
                                &i18n_no_downloaded_episodes_description
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

/// The expanded episode list for a single downloaded podcast.
///
/// Exists as its own component (rather than inline markup in
/// [`render_podcast_with_episodes`]) so it can own a `use_node_ref` for the
/// `.podcast-episodes-inner` scroll box. That box has `overflow-y: auto; max-height: 850px`,
/// so the list scrolls locally — `VirtualList` must read its `scrollTop`/`clientHeight` via
/// [`ScrollSource::Container`], not the window. Without this, window-based windowing math
/// only ever renders the first viewport of cards and the rest collapse into the bottom
/// spacer (blank screen after ~10 episodes).
#[derive(Properties, PartialEq)]
pub struct DownloadEpisodesProps {
    pub episodes: Rc<Vec<Episode>>,
    pub podcastid: i32,
    pub is_delete_mode: bool,
    pub on_checkbox_change: Callback<i32>,
    pub loading_more: bool,
    pub has_more: bool,
    pub load_more: Callback<MouseEvent>,
    pub load_more_label: String,
}

#[function_component(DownloadEpisodes)]
pub fn download_episodes(props: &DownloadEpisodesProps) -> Html {
    let container_ref = use_node_ref();
    html! {
        <div class="podcast-episodes-inner" ref={container_ref.clone()}>
            <EpisodeListView
                key={format!("downloads-{}", props.podcastid)}
                episodes={props.episodes.clone()}
                backend_can_load_more={false}
                loading_more={false}
                page_type={PageType::Downloads}
                is_delete_mode={props.is_delete_mode}
                on_checkbox_change={props.on_checkbox_change.clone()}
                disable_sentinel={true}
                scroll_source={ScrollSource::Container(container_ref.clone())}
            />
            { if props.loading_more {
                html! {
                    <div class="flex justify-center py-4">
                        <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-current"></div>
                    </div>
                }
            } else if props.has_more {
                html! {
                    <div class="flex justify-center py-4">
                        <button class="sp-chip" onclick={props.load_more.clone()}>
                            <i class="ph ph-arrow-down"></i>
                            <span>{ &props.load_more_label }</span>
                        </button>
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}

pub fn render_podcast_with_episodes(
    summary: &PodcastDownloadSummary,
    episodes: Vec<Episode>,
    total: i64,
    is_expanded: bool,
    toggle_pod_expanded: Callback<MouseEvent>,
    dispatch: Dispatch<AppState>,
    is_delete_mode: bool,
    on_checkbox_change: Callback<i32>,
    loading_more: bool,
    has_more: bool,
    load_more: Callback<MouseEvent>,
    load_more_label: String,
) -> Html {
    let on_podcast_checkbox_change = {
        let on_checkbox_change = on_checkbox_change.clone();
        let dispatch_clone = dispatch.clone();
        let episode_ids: Vec<i32> = episodes.iter().map(|ep| ep.episodeid).collect();

        Callback::from(move |e: Event| {
            let is_checked = e
                .target_dyn_into::<web_sys::HtmlInputElement>()
                .map(|input| input.checked())
                .unwrap_or(false);

            let selected_episodes = &dispatch_clone.get().selected_episodes_for_deletion;

            for episode_id in &episode_ids {
                let is_episode_selected = selected_episodes.contains(episode_id);
                if is_checked && !is_episode_selected {
                    on_checkbox_change.emit(*episode_id);
                } else if !is_checked && is_episode_selected {
                    on_checkbox_change.emit(*episode_id);
                }
            }
        })
    };

    html! {
        <div key={summary.podcastid}>
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
                        src={summary.artworkurl.clone().unwrap_or_default()}
                        alt={format!("Cover for {}", summary.podcastname.clone())}
                        class="podcast-dropdown-image"
                    />

                    <div class="podcast-dropdown-info">
                        <p class="podcast-dropdown-title item_container-text">
                            { &summary.podcastname }
                        </p>
                        <p class="podcast-dropdown-count item_container-text">
                            { format!("{} Downloaded Episodes", total) }
                        </p>
                    </div>

                    <div class={classes!("podcast-dropdown-arrow", is_expanded.then(|| "expanded"))}>
                        <i class="ph ph-caret-down text-2xl"></i>
                    </div>
                </div>
            </div>

            { if is_expanded {
                // Wrap the per-podcast loaded episodes in EpisodeListView. The view is now
                // backed by VirtualList (true windowing), so even hundreds of locally-loaded
                // episodes mount at viewport-bound cost. `disable_sentinel=true` plus the
                // explicit "Load more" button below: the page is a list-of-podcasts, so a
                // sentinel inside one expanded podcast would auto-fetch more episodes for
                // THIS podcast every time the user scrolls past it toward the next podcast.
                let episodes_rc: Rc<Vec<Episode>> = Rc::new(episodes);
                html! {
                    <div class="podcast-episodes-container expanded">
                        <DownloadEpisodes
                            episodes={episodes_rc}
                            podcastid={summary.podcastid}
                            is_delete_mode={is_delete_mode}
                            on_checkbox_change={on_checkbox_change.clone()}
                            loading_more={loading_more}
                            has_more={has_more}
                            load_more={load_more.clone()}
                            load_more_label={load_more_label.clone()}
                        />
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
