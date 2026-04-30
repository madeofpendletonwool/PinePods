use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::components::virtual_list::VirtualList;
use crate::requests::episode::Episode;
use crate::requests::pod_req;

use crate::requests::pod_req::RecentEps;
use gloo::events::EventListener;
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::{Element, HtmlElement};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

// Helper function to calculate responsive item height - MUST be synchronous and accurate
#[allow(dead_code)]
fn calculate_item_height(window_width: f64) -> f64 {
    // CRITICAL: Must match the exact height that episodes render at, including margin
    // Episodes render at container_height + mb-4 margin (16px)
    let height = if window_width <= 530.0 {
        122.0 + 16.0 // Mobile: episode container 122px + mb-4 margin
    } else if window_width <= 768.0 {
        150.0 + 16.0 // Tablet: episode container 150px + mb-4 margin
    } else {
        221.0 + 16.0 // Desktop: episode container 221px + mb-4 margin
    };

    web_sys::console::log_1(
        &format!(
            "FEED HEIGHT CALC: width={}, calculated_height={}",
            window_width, height
        )
        .into(),
    );

    height
}

#[function_component(Feed)]
pub fn feed() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let loading = use_state(|| true);

    // Capture i18n strings before they get moved
    let i18n_no_recent_episodes_found = i18n.t("feed.no_recent_episodes_found").to_string();
    let i18n_no_recent_episodes_description =
        i18n.t("feed.no_recent_episodes_description").to_string();

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
                        match pod_req::call_get_recent_eps(&server_name, &api_key, &user_id).await {
                            Ok(fetched_episodes) => {
                                let completed_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let saved_episodes: Vec<Episode> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.saved)
                                    .map(|e| e.clone())
                                    .collect();
                                let queued_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.queued)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                dispatch.reduce_mut(move |state| {
                                    state.server_feed_results = Some(RecentEps {
                                        episodes: Some(fetched_episodes),
                                    });
                                    state.completed_episodes = Some(completed_episode_ids);
                                    state.saved_episodes = saved_episodes;
                                    state.queued_episode_ids = Some(queued_episode_ids);
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

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                if *loading { // If loading is true, display the loading animation
                    html! { <Loading/> }
                } else {
                    if let Some(recent_eps) = state.server_feed_results.clone() {
                        let int_recent_eps = recent_eps.clone();
                        if let Some(episodes) = int_recent_eps.episodes {

                            if episodes.is_empty() {
                                // Render "No Recent Episodes Found" if episodes list is empty
                                empty_message(
                                    &i18n_no_recent_episodes_found,
                                    &i18n_no_recent_episodes_description
                                )
                            } else {
                                html! {
                                    <VirtualList
                                        episodes={episodes}
                                    />
                                }
                            }
                        } else {
                            empty_message(
                                "No Recent Episodes Found",
                                "You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."
                            )
                        }
                    } else {
                        empty_message(
                            "No Recent Episodes Found",
                            "You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."
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
