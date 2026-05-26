use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, EpisodeStatusState, FilterState};
use crate::components::context_menu_button::PageType;
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_components::{
    empty_message, Search_nav, UseScrollToTop,
};
use crate::components::gen_funcs::{
    get_default_sort_direction, get_filter_preference, set_filter_preference,
};
use crate::components::loading::Loading;
use crate::requests::episode::Episode;
use crate::requests::pod_req;
use i18nrs::yew::use_translation;
use js_sys::Array;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::{IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

#[function_component(Saved)]
pub fn saved() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let (filter_state, _filter_dispatch) = use_store::<FilterState>();

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

    let episodes = use_state(|| Vec::<Episode>::new());
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading = use_state(|| true);
    let loading_more = use_state(|| false);
    let sentinel_ref = use_node_ref();

    let episode_search_term = use_state(|| String::new());

    // Sort/filter — persisted in localStorage
    let sort_pref = get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string());
    let sort_value = use_state(|| sort_pref.clone());
    // Completion filter stored as a separate key
    let filter_value = use_state(|| {
        get_filter_preference("saved_filter").unwrap_or_else(|| "all".to_string())
    });

    // Derive API sort_by / sort_order from the sort_value string
    fn sort_to_params(sort: &str) -> (&'static str, &'static str) {
        match sort {
            "oldest"   => ("date", "asc"),
            "shortest" => ("duration", "asc"),
            "longest"  => ("duration", "desc"),
            "title_az" => ("title", "asc"),
            "title_za" => ("title", "desc"),
            _          => ("date", "desc"), // "newest" or default
        }
    }

    // Trigger for reloading when sort or filter changes
    let reload_trigger = use_state(|| 0u32);

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

                    episodes.set(Vec::new());
                    offset.set(0);
                    total.set(0);
                    loading.set(true);

                    wasm_bindgen_futures::spawn_local(async move {
                        let (sort_by, sort_order) = sort_to_params(&sort_str);
                        match pod_req::call_get_saved_episodes_paged(
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
                                let completed_ids: Vec<i32> = page
                                    .saved_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let saved_eps = page.saved_episodes.clone();
                                Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                    s.saved_episodes = saved_eps;
                                    s.completed_episodes = Some(completed_ids);
                                });

                                #[cfg(not(feature = "server_build"))]
                                {
                                    wasm_bindgen_futures::spawn_local(async move {
                                        if let Ok(mut local_episodes) =
                                            crate::pages::downloads_tauri::fetch_local_episodes().await
                                        {
                                            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                                s.downloaded_episodes.clear_local();
                                                for ep in local_episodes.drain(..) {
                                                    s.downloaded_episodes.push_local(ep);
                                                }
                                            });
                                        }
                                    });
                                }

                                let new_offset = page.saved_episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                episodes.set(page.saved_episodes);
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

    // IntersectionObserver: load next page when sentinel comes into view
    {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let sentinel_ref = sentinel_ref.clone();
        let sort_value = sort_value.clone();
        let filter_value = filter_value.clone();

        use_effect_with(
            (sentinel_ref.clone(), *offset, *total),
            move |(sentinel_ref, _, _)| {
                let sentinel_el = match sentinel_ref.cast::<web_sys::Element>() {
                    Some(el) => el,
                    None => return Box::new(|| ()) as Box<dyn FnOnce()>,
                };

                let episodes = episodes.clone();
                let total = total.clone();
                let offset = offset.clone();
                let loading_more = loading_more.clone();
                let api_key = api_key.clone();
                let user_id = user_id.clone();
                let server_name = server_name.clone();
                let sort_value = sort_value.clone();
                let filter_value = filter_value.clone();

                let callback = Closure::<dyn Fn(Array)>::wrap(Box::new(move |entries: Array| {
                    let entry: IntersectionObserverEntry = entries.get(0).unchecked_into();
                    if !entry.is_intersecting() {
                        return;
                    }
                    let current_offset = *offset;
                    let current_total = *total;
                    if *loading_more || current_offset >= current_total {
                        return;
                    }

                    let episodes = episodes.clone();
                    let total = total.clone();
                    let offset = offset.clone();
                    let loading_more = loading_more.clone();
                    let api_key = api_key.clone();
                    let user_id = user_id.clone();
                    let server_name = server_name.clone();
                    let sort_str = (*sort_value).clone();
                    let filter_str = (*filter_value).clone();

                    if let (Some(api_key), Some(user_id), Some(server_name)) =
                        (api_key.clone(), user_id.clone(), server_name.clone())
                    {
                        loading_more.set(true);
                        wasm_bindgen_futures::spawn_local(async move {
                            let (sort_by, sort_order) = sort_to_params(&sort_str);
                            if let Ok(page) = pod_req::call_get_saved_episodes_paged(
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
                                let new_offset = current_offset + page.saved_episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                episodes.set({
                                    let mut all = (*episodes).clone();
                                    all.extend(page.saved_episodes);
                                    all
                                });
                            }
                            loading_more.set(false);
                        });
                    }
                }));

                let mut opts = IntersectionObserverInit::new();
                opts.root_margin("200px");
                let observer =
                    IntersectionObserver::new_with_options(callback.as_ref().unchecked_ref(), &opts)
                        .expect("IntersectionObserver creation failed");
                observer.observe(&sentinel_el);
                callback.forget();

                let observer_clone = observer.clone();
                Box::new(move || {
                    observer_clone.disconnect();
                }) as Box<dyn FnOnce()>
            },
        );
    }

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

    // Client-side search filter only
    let search_term = (*episode_search_term).clone();
    let display_episodes: Vec<Episode> = (*episodes)
        .iter()
        .filter(|ep| {
            if filter_state.favorites_only && !favorite_podcast_ids.contains(&ep.podcastid) {
                return false;
            }
            if search_term.is_empty() {
                return true;
            }
            let term = search_term.to_lowercase();
            ep.episodetitle.to_lowercase().contains(&term)
                || ep.episodedescription.to_lowercase().contains(&term)
        })
        .cloned()
        .collect();

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
                                        <i class="ph ph-bookmark tab-icon"></i>
                                        {&i18n.t("saved.saved")}
                                    </div>
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
                                    <div class="flex-shrink-0 relative min-w-[160px]">
                                        <select
                                            class="sort-dropdown"
                                            onchange={
                                                let sort_value = sort_value.clone();
                                                let reload_trigger = reload_trigger.clone();
                                                Callback::from(move |e: Event| {
                                                    let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                    let value = target.value();
                                                    set_filter_preference("saved", &value);
                                                    sort_value.set(value);
                                                    reload_trigger.set(*reload_trigger + 1);
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

                                <div class="flex gap-3 overflow-x-auto pb-2 md:pb-0 scrollbar-hide">
                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                set_filter_preference("saved_filter", "all");
                                                filter_value.set("all".to_string());
                                                reload_trigger.set(*reload_trigger + 1);
                                            })
                                        }
                                        class="filter-chip"
                                    >
                                        <i class="ph ph-broom text-lg"></i>
                                        <span class="text-sm font-medium">{&i18n.t("saved.clear_all")}</span>
                                    </button>

                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                let next = if *filter_value == "completed" { "all" } else { "completed" };
                                                set_filter_preference("saved_filter", next);
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
                                        <span class="text-sm font-medium">{&i18n.t("saved.completed")}</span>
                                    </button>

                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                let next = if *filter_value == "in_progress" { "all" } else { "in_progress" };
                                                set_filter_preference("saved_filter", next);
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
                                        <span class="text-sm font-medium">{&i18n.t("saved.in_progress")}</span>
                                    </button>
                                </div>
                            </div>

                            {
                                if display_episodes.is_empty() {
                                    empty_message(
                                        &i18n.t("saved.no_saved_episodes"),
                                        &i18n.t("saved.save_episodes_instructions")
                                    )
                                } else {
                                    html! {
                                        <div class="flex-grow overflow-y-auto">
                                            { for display_episodes.iter().map(|ep| html! {
                                                <EpisodeListItem key={ep.episodeid} episode={ep.clone()} page_type={PageType::Saved} />
                                            }) }
                                            <div ref={sentinel_ref.clone()} style="height: 1px;" />
                                            if *loading_more {
                                                <Loading />
                                            }
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
