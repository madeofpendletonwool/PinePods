use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, EpisodeStatusState, FilterState, PodcastFeedState};
use crate::components::episode_list_view::EpisodeListView;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::virtual_list::ScrollSource;
use crate::components::gen_funcs::{
    get_default_sort_direction, get_filter_preference, set_filter_preference,
};
use crate::components::loading::Loading;
use crate::requests::episode::Episode;
use crate::requests::pod_req;
use gloo_timers::future::TimeoutFuture;
use i18nrs::yew::use_translation;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

#[function_component(PodHistory)]
pub fn history() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let (podcast_state, _) = use_store::<PodcastFeedState>();
    let (filter_state, _filter_dispatch) = use_store::<FilterState>();

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

    let api_key_sel = use_selector(|s: &AppState| {
        s.auth_details.as_ref().map(|ud| ud.api_key.clone())
    });
    let user_id_sel = use_selector(|s: &AppState| {
        s.user_details.as_ref().map(|ud| ud.UserID.clone())
    });
    let server_name_sel = use_selector(|s: &AppState| {
        s.auth_details.as_ref().map(|ud| ud.server_name.clone())
    });
    let api_key = (*api_key_sel).clone();
    let user_id = (*user_id_sel).clone();
    let server_name = (*server_name_sel).clone();

    let episodes = use_state(|| Rc::new(Vec::<Episode>::new()));
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading = use_state(|| true);
    let loading_more = use_state(|| false);
    let scroll_ref = use_node_ref();

    let episode_search_term = use_state(|| String::new());

    let sort_value = use_state(|| {
        get_filter_preference("history").unwrap_or_else(|| get_default_sort_direction().to_string())
    });
    let filter_value = use_state(|| {
        get_filter_preference("history_filter").unwrap_or_else(|| "all".to_string())
    });

    let reload_trigger = use_state(|| 0u32);

    fn sort_to_params(sort: &str) -> (&'static str, &'static str) {
        match sort {
            "oldest"   => ("date", "asc"),
            "shortest" => ("duration", "asc"),
            "longest"  => ("duration", "desc"),
            "title_az" => ("title", "asc"),
            "title_za" => ("title", "desc"),
            _          => ("date", "desc"),
        }
    }

    // Initial page fetch (and reload when sort/filter changes)
    {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading = loading.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let sort_value = sort_value.clone();
        let filter_value = filter_value.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone(), *reload_trigger),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let episodes = episodes.clone();
                    let total = total.clone();
                    let offset = offset.clone();
                    let loading = loading.clone();
                    let sort_str = (*sort_value).clone();
                    let filter_str = (*filter_value).clone();

                    episodes.set(Rc::new(Vec::new()));
                    offset.set(0);
                    total.set(0);
                    loading.set(true);

                    wasm_bindgen_futures::spawn_local(async move {
                        let (sort_by, sort_order) = sort_to_params(&sort_str);
                        match pod_req::call_get_user_history_paged(
                            &server_name,
                            &api_key,
                            &user_id,
                            PAGE_SIZE,
                            0,
                            sort_by,
                            sort_order,
                            &filter_str,
                        )
                        .await
                        {
                            Ok(page) => {
                                let completed_ids: std::collections::HashSet<i32> = page
                                    .data
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                    s.completed_episodes = completed_ids;
                                });

                                let new_offset = page.data.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                episodes.set(Rc::new(page.data));
                                loading.set(false);
                            }
                            Err(_) => {
                                loading.set(false);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    // Load-more handler. EpisodeListView owns the sentinel/observer/display-count/ramp; this
    // callback fires only when the view runs out of buffered episodes and the parent reports
    // `backend_can_load_more`. Sort and filter values are read fresh on every fire.
    let on_load_more = {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        let sort_value = sort_value.clone();
        let filter_value = filter_value.clone();
        use_callback((), move |_: (), _| {
            if *loading_more {
                return;
            }
            let current_offset = *offset;
            let current_total = *total;
            if current_offset >= current_total {
                return;
            }
            let Some(api_key) = api_key.clone() else { return; };
            let Some(user_id) = user_id.clone() else { return; };
            let Some(server_name) = server_name.clone() else { return; };
            let sort_str = (*sort_value).clone();
            let filter_str = (*filter_value).clone();
            loading_more.set(true);
            let episodes = episodes.clone();
            let total = total.clone();
            let offset = offset.clone();
            let loading_more = loading_more.clone();
            spawn_local(async move {
                let (sort_by, sort_order) = sort_to_params(&sort_str);
                if let Ok(page) = pod_req::call_get_user_history_paged(
                    &server_name,
                    &api_key,
                    &user_id,
                    PAGE_SIZE,
                    current_offset,
                    sort_by,
                    sort_order,
                    &filter_str,
                )
                .await
                {
                    TimeoutFuture::new(0).await;
                    let new_offset = current_offset + page.data.len() as i64;
                    let mut all = (**episodes).clone();
                    all.extend(page.data);
                    total.set(page.total);
                    offset.set(new_offset);
                    episodes.set(Rc::new(all));
                    TimeoutFuture::new(0).await;
                }
                loading_more.set(false);
            });
        })
    };

    let favorite_podcast_ids: std::collections::HashSet<i32> = podcast_state
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

    let search_term = (*episode_search_term).clone();
    let has_client_filter = !search_term.is_empty()
        || (filter_state.favorites_only && !favorite_podcast_ids.is_empty());
    let display_episodes_rc: Rc<Vec<Episode>> = if has_client_filter {
        let term = search_term.to_lowercase();
        Rc::new(
            (*episodes)
                .iter()
                .filter(|ep| {
                    if filter_state.favorites_only && !favorite_podcast_ids.contains(&ep.podcastid)
                    {
                        return false;
                    }
                    if term.is_empty() {
                        return true;
                    }
                    ep.episodetitle.to_lowercase().contains(&term)
                        || ep.episodedescription.to_lowercase().contains(&term)
                })
                .cloned()
                .collect(),
        )
    } else {
        (*episodes).clone()
    };
    let display_empty = display_episodes_rc.is_empty();
    let backend_can_load_more = *offset < *total;

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                if *loading {
                    html! { <Loading/> }
                } else {
                    html! {
                        <>
                            <div class="mb-6 space-y-4 mt-4">
                                <div class="flex gap-0 h-12 relative">
                                    <div class="page-tab-indicator">
                                        <i class="ph ph-clock-clockwise tab-icon"></i>
                                        {&i18n_history}
                                    </div>
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
                                    <div class="flex-shrink-0 relative min-w-[160px]">
                                        <select
                                            class="sort-dropdown"
                                            onchange={
                                                let sort_value = sort_value.clone();
                                                let reload_trigger = reload_trigger.clone();
                                                Callback::from(move |e: Event| {
                                                    let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                    let value = target.value();
                                                    set_filter_preference("history", &value);
                                                    sort_value.set(value);
                                                    reload_trigger.set(*reload_trigger + 1);
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

                                <div class="flex gap-3 overflow-x-auto pb-2 md:pb-0 scrollbar-hide">
                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                set_filter_preference("history_filter", "all");
                                                filter_value.set("all".to_string());
                                                reload_trigger.set(*reload_trigger + 1);
                                            })
                                        }
                                        class="filter-chip"
                                    >
                                        <i class="ph ph-broom text-lg"></i>
                                        <span class="text-sm font-medium">{&i18n_clear_all}</span>
                                    </button>

                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                let next = if *filter_value == "completed" { "all" } else { "completed" };
                                                set_filter_preference("history_filter", next);
                                                filter_value.set(next.to_string());
                                                reload_trigger.set(*reload_trigger + 1);
                                            })
                                        }
                                        class={classes!(
                                            "filter-chip",
                                            if *filter_value == "completed" { "filter-chip-active" } else { "" }
                                        )}
                                    >
                                        <i class="ph ph-check-circle text-lg"></i>
                                        <span class="text-sm font-medium">{&i18n_completed}</span>
                                    </button>

                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                let next = if *filter_value == "in_progress" { "all" } else { "in_progress" };
                                                set_filter_preference("history_filter", next);
                                                filter_value.set(next.to_string());
                                                reload_trigger.set(*reload_trigger + 1);
                                            })
                                        }
                                        class={classes!(
                                            "filter-chip",
                                            if *filter_value == "in_progress" { "filter-chip-active" } else { "" }
                                        )}
                                    >
                                        <i class="ph ph-hourglass-medium text-lg"></i>
                                        <span class="text-sm font-medium">{&i18n_in_progress}</span>
                                    </button>
                                </div>
                            </div>

                            {
                                if display_empty {
                                    empty_message(
                                        &i18n_no_episode_history_found,
                                        &i18n_no_episode_history_description
                                    )
                                } else {
                                    html! {
                                        <div ref={scroll_ref.clone()} class="flex-grow overflow-y-auto">
                                            <EpisodeListView
                                                episodes={display_episodes_rc}
                                                backend_can_load_more={backend_can_load_more}
                                                loading_more={*loading_more}
                                                on_load_more={on_load_more.clone()}
                                                scroll_source={ScrollSource::Container(scroll_ref.clone())}
                                            />
                                        </div>
                                    }
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
