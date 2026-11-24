use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, EpisodeStatusState, FilterState, PodcastFeedState};
use crate::components::episode_list_view::EpisodeListView;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::components::page_cache;
use crate::requests::episode::Episode;
use crate::requests::pod_req;
use crate::requests::pod_req::PodcastResponseExtra;
use gloo_timers::future::TimeoutFuture;
use crate::components::i18n::use_translation;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

/// First page of the feed as last seen, shown while the fresh copy loads.
#[derive(Clone, Serialize, Deserialize)]
struct CachedFeed {
    episodes: Vec<Episode>,
    total: i64,
}

/// Seed the shared saved/queued/completed state from a page of episodes.
fn apply_episode_status(episodes: &[Episode]) {
    let completed: std::collections::HashSet<i32> =
        episodes.iter().filter(|ep| ep.completed).map(|ep| ep.episodeid).collect();
    let saved: Vec<Episode> = episodes.iter().filter(|ep| ep.saved).cloned().collect();
    let queued: Vec<i32> = episodes.iter().filter(|ep| ep.queued).map(|ep| ep.episodeid).collect();
    Dispatch::<EpisodeStatusState>::global().reduce_mut(move |state| {
        state.completed_episodes = completed;
        state.saved_episodes = saved;
        state.queued_episode_ids = Some(queued);
    });
}

#[function_component(Feed)]
pub fn feed() -> Html {
    let (i18n, _) = use_translation();
    let (filter_state, _filter_dispatch) = use_store::<FilterState>();

    let i18n_no_recent_episodes_found = i18n.t("feed.no_recent_episodes_found").to_string();
    let i18n_no_recent_episodes_description =
        i18n.t("feed.no_recent_episodes_description").to_string();

    // Selective subscriptions — feed page only re-renders when auth or podcast list changes,
    // NOT when individual episode saved/downloaded/queued state changes.
    let api_key_sel = use_selector(|state: &AppState| {
        state.auth_details.as_ref().map(|ud| ud.api_key.clone())
    });
    let user_id_sel = use_selector(|state: &AppState| {
        state.user_details.as_ref().map(|ud| ud.UserID.clone())
    });
    let server_name_sel = use_selector(|state: &AppState| {
        state.auth_details.as_ref().map(|ud| ud.server_name.clone())
    });
    let podcast_feed_extra = use_selector(|state: &PodcastFeedState| {
        state.podcast_feed_return_extra.clone()
    });
    let api_key = (*api_key_sel).clone();
    let user_id = (*user_id_sel).clone();
    let server_name = (*server_name_sel).clone();

    let episodes = use_state(|| Rc::new(Vec::<Episode>::new()));
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading = use_state(|| true);
    let loading_more = use_state(|| false);
    let error = use_state(|| None::<String>);
    // False while showing the cached first page; paging waits for the fresh one.
    let refreshed = use_state(|| false);

    // Initial page fetch
    {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading = loading.clone();
        let error = error.clone();
        let refreshed = refreshed.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    // Fetch podcast extras in parallel
                    {
                        let server_name_pods = server_name.clone();
                        let api_key_pods = api_key.clone();
                        let user_id_pods = user_id.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(fetched_pods) = pod_req::call_get_podcasts_extra(
                                &server_name_pods,
                                &api_key_pods,
                                &user_id_pods,
                            )
                            .await
                            {
                                Dispatch::<PodcastFeedState>::global().reduce_mut(move |state| {
                                    state.podcast_feed_return_extra = Some(PodcastResponseExtra {
                                        pods: Some(fetched_pods),
                                    });
                                });
                            }
                        });
                    }

                    let cache_key = page_cache::key("feed", &server_name, user_id);
                    let network_done = Rc::new(Cell::new(false));
                    let show_cached = {
                        let episodes = episodes.clone();
                        let total = total.clone();
                        let offset = offset.clone();
                        let loading = loading.clone();
                        move |cached: CachedFeed| {
                            apply_episode_status(&cached.episodes);
                            offset.set(cached.episodes.len() as i64);
                            total.set(cached.total);
                            episodes.set(Rc::new(cached.episodes));
                            loading.set(false);
                        }
                    };
                    if let Some(cached) = page_cache::memory::<CachedFeed>(&cache_key) {
                        show_cached(cached);
                    } else {
                        let cache_key = cache_key.clone();
                        let network_done = network_done.clone();
                        spawn_local(async move {
                            if let Some(cached) = page_cache::stored::<CachedFeed>(&cache_key).await {
                                if !network_done.get() {
                                    show_cached(cached);
                                }
                            }
                        });
                    }

                    let episodes = episodes.clone();
                    let total = total.clone();
                    let offset = offset.clone();
                    let loading = loading.clone();
                    let error = error.clone();
                    let refreshed = refreshed.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_recent_eps_paged(
                            &server_name,
                            &api_key,
                            &user_id,
                            PAGE_SIZE,
                            0,
                        )
                        .await
                        {
                            Ok(page) => {
                                network_done.set(true);
                                apply_episode_status(&page.episodes);
                                page_cache::put(
                                    &cache_key,
                                    &CachedFeed { episodes: page.episodes.clone(), total: page.total },
                                );

                                #[cfg(not(feature = "server_build"))]
                                {
                                    wasm_bindgen_futures::spawn_local(async move {
                                        if let Ok(mut local_episodes) =
                                            crate::pages::downloads_tauri::fetch_local_episodes()
                                                .await
                                        {
                                            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |state| {
                                                state.downloaded_episodes.clear_local();
                                                for ep in local_episodes.drain(..) {
                                                    state.downloaded_episodes.push_local(ep);
                                                }
                                            });
                                        }
                                    });
                                }

                                let new_offset = page.episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                episodes.set(Rc::new(page.episodes));
                                loading.set(false);
                                refreshed.set(true);
                            }
                            Err(e) => {
                                network_done.set(true);
                                error.set(Some(e.to_string()));
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
    // callback only fires when the view runs out of buffered episodes and the parent reports
    // `backend_can_load_more`. The two TimeoutFuture::new(0).await yields keep the page
    // responsive while the spinner paints and the new cards mount.
    let on_load_more = {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
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
            loading_more.set(true);
            let episodes = episodes.clone();
            let total = total.clone();
            let offset = offset.clone();
            let loading_more = loading_more.clone();
            spawn_local(async move {
                if let Ok(page) = pod_req::call_get_recent_eps_paged(
                    &server_name,
                    &api_key,
                    &user_id,
                    PAGE_SIZE,
                    current_offset,
                )
                .await
                {
                    TimeoutFuture::new(0).await;
                    let new_offset = current_offset + page.episodes.len() as i64;
                    let mut all = (**episodes).clone();
                    all.extend(page.episodes);
                    total.set(page.total);
                    offset.set(new_offset);
                    episodes.set(Rc::new(all));
                    TimeoutFuture::new(0).await;
                }
                loading_more.set(false);
            });
        })
    };

    let favorite_podcast_ids: std::collections::HashSet<i32> = (*podcast_feed_extra)
        .as_ref()
        .and_then(|pr| pr.pods.as_ref())
        .map(|pods| {
            pods.iter()
                .filter(|p| p.is_favorite)
                .map(|p| p.podcastid)
                .collect()
        })
        .unwrap_or_default();

    // Skip the clone when no client filter is active — just hand the parent's Rc<Vec<Episode>>
    // straight to the view. Filtering only allocates a new Vec when favorites_only is on AND
    // there's at least one favorite podcast to filter against.
    let filtered_episodes_rc: Rc<Vec<Episode>> =
        if filter_state.favorites_only && !favorite_podcast_ids.is_empty() {
            Rc::new(
                (*episodes)
                    .iter()
                    .filter(|ep| favorite_podcast_ids.contains(&ep.podcastid))
                    .cloned()
                    .collect(),
            )
        } else {
            (*episodes).clone()
        };
    let backend_can_load_more = *refreshed && *offset < *total;
    let episodes_empty = filtered_episodes_rc.is_empty();

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                if *loading {
                    html! { <Loading/> }
                } else if episodes_empty {
                    empty_message(
                        &i18n_no_recent_episodes_found,
                        &i18n_no_recent_episodes_description
                    )
                } else {
                    html! {
                        <div class="flex-grow overflow-y-auto">
                            <EpisodeListView
                                episodes={filtered_episodes_rc}
                                backend_can_load_more={backend_can_load_more}
                                loading_more={*loading_more}
                                on_load_more={on_load_more.clone()}
                            />
                        </div>
                    }
                }
            }
            <AudioPlayerBar />
        </div>
        <App_drawer />
        </>
    }
}
