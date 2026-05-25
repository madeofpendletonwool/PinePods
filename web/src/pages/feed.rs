use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, FilterState};
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::requests::episode::Episode;
use crate::requests::pod_req;
use crate::requests::pod_req::PodcastResponseExtra;
use i18nrs::yew::use_translation;
use js_sys::Array;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::{IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

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
    let podcast_feed_extra = use_selector(|state: &AppState| {
        state.podcast_feed_return_extra.clone()
    });
    let api_key = (*api_key_sel).clone();
    let user_id = (*user_id_sel).clone();
    let server_name = (*server_name_sel).clone();

    let episodes = use_state(|| Vec::<Episode>::new());
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading = use_state(|| true);
    let loading_more = use_state(|| false);
    let error = use_state(|| None::<String>);
    let sentinel_ref = use_node_ref();

    // Initial page fetch
    {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading = loading.clone();
        let error = error.clone();
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
                                Dispatch::<AppState>::global().reduce_mut(move |state| {
                                    state.podcast_feed_return_extra = Some(PodcastResponseExtra {
                                        pods: Some(fetched_pods),
                                    });
                                });
                            }
                        });
                    }

                    let episodes = episodes.clone();
                    let total = total.clone();
                    let offset = offset.clone();
                    let loading = loading.clone();
                    let error = error.clone();

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
                                let completed_episode_ids: Vec<i32> = page
                                    .episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let saved_episodes: Vec<Episode> = page
                                    .episodes
                                    .iter()
                                    .filter(|ep| ep.saved)
                                    .cloned()
                                    .collect();
                                let queued_episode_ids: Vec<i32> = page
                                    .episodes
                                    .iter()
                                    .filter(|ep| ep.queued)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                Dispatch::<AppState>::global().reduce_mut(move |state| {
                                    state.completed_episodes = Some(completed_episode_ids);
                                    state.saved_episodes = saved_episodes;
                                    state.queued_episode_ids = Some(queued_episode_ids);
                                });

                                #[cfg(not(feature = "server_build"))]
                                {
                                    wasm_bindgen_futures::spawn_local(async move {
                                        if let Ok(mut local_episodes) =
                                            crate::pages::downloads_tauri::fetch_local_episodes()
                                                .await
                                        {
                                            Dispatch::<AppState>::global().reduce_mut(move |state| {
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
                                episodes.set(page.episodes);
                                loading.set(false);
                            }
                            Err(e) => {
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

                    if let (Some(api_key), Some(user_id), Some(server_name)) =
                        (api_key.clone(), user_id.clone(), server_name.clone())
                    {
                        loading_more.set(true);
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(page) = pod_req::call_get_recent_eps_paged(
                                &server_name,
                                &api_key,
                                &user_id,
                                PAGE_SIZE,
                                current_offset,
                            )
                            .await
                            {
                                let new_offset = current_offset + page.episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                episodes.set({
                                    let mut all = (*episodes).clone();
                                    all.extend(page.episodes);
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

    let filtered_episodes: Vec<Episode> = (*episodes)
        .iter()
        .filter(|ep| {
            if !filter_state.favorites_only {
                return true;
            }
            favorite_podcast_ids.contains(&ep.podcastid)
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
                } else if filtered_episodes.is_empty() {
                    empty_message(
                        &i18n_no_recent_episodes_found,
                        &i18n_no_recent_episodes_description
                    )
                } else {
                    html! {
                        <div class="flex-grow overflow-y-auto">
                            { for filtered_episodes.iter().map(|episode| {
                                html! {
                                    <EpisodeListItem
                                        key={episode.episodeid}
                                        episode={episode.clone()}
                                    />
                                }
                            }) }
                            <div ref={sentinel_ref.clone()} style="height: 1px;" />
                            if *loading_more {
                                <Loading />
                            }
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
