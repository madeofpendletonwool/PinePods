use crate::components::click_events::create_on_title_click;
use crate::components::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, NotificationState, PodcastState, SearchState, UIState};
use crate::components::gen_funcs::format_error_message;
use crate::components::safehtml::SafeHtml;
use crate::requests::pod_req::{
    call_add_podcast, call_check_podcast, call_remove_podcasts_name, PodcastDetails, PodcastValues,
    RemovePodcastValuesName,
};
use crate::requests::search_pods::UnifiedPodcast;
use crate::components::app_drawer::App_drawer;
use i18nrs::yew::use_translation;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew::{function_component, html, Callback, Html};
use yew_router::history::BrowserHistory;
use yewdux::dispatch::Dispatch;
use yewdux::use_store;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ClickedFeedURL {
    pub podcastid: i32,
    pub podcastname: String,
    pub feedurl: String,
    pub description: String,
    pub author: String,
    pub artworkurl: String,
    pub explicit: bool,
    pub episodecount: i32,
    pub categories: Option<HashMap<String, String>>,
    pub websiteurl: String,
    pub podcastindexid: i32,
    pub is_youtube: Option<bool>,
}

impl ClickedFeedURL {
    pub fn into_podcast_details(self) -> PodcastDetails {
        PodcastDetails {
            podcastid: self.podcastid as i32,
            podcastname: self.podcastname,
            artworkurl: self.artworkurl,
            author: self.author,
            categories: self.categories.unwrap_or_default(),
            description: self.description,
            episodecount: self.episodecount,
            feedurl: self.feedurl,
            websiteurl: self.websiteurl,
            explicit: self.explicit,
            userid: 0, // Default value since it's not in ClickedFeedURL
            podcastindexid: self.podcastindexid,
            is_youtube: self.is_youtube.unwrap_or(false),
        }
    }
}

#[function_component(PodLayout)]
pub fn pod_layout() -> Html {
    let (i18n, _) = use_translation();
    let (_state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let (search_state, _) = use_store::<SearchState>();
    let search_results = search_state.search_results.clone();

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                <h1 class="item_container-text text-2xl font-bold my-6 text-center">{ &i18n.t("podcast_layout.search_results") }</h1>

                {
                    if let Some(results) = search_results {
                        let podcasts = results.feeds.as_ref().map_or_else(
                            || results.results.as_ref().map(|r| r.iter().filter(|item| item.feedUrl.is_some()).map(|item| item.clone().into()).collect::<Vec<UnifiedPodcast>>()),
                            |f| Some(f.iter().map(|item| item.clone().into()).collect::<Vec<UnifiedPodcast>>())
                        );

                        if let Some(podcasts) = podcasts {
                            if !podcasts.is_empty() {
                                html! {
                                    <div class="podcast-search-grid">
                                        { for podcasts.iter().map(|podcast| html! {
                                            <PodcastItem podcast={podcast.clone()} />
                                        })}
                                    </div>
                                }
                            } else {
                                empty_message(
                                    &i18n.t("podcast_layout.no_results_found"),
                                    &i18n.t("podcast_layout.try_different_keywords")
                                )
                            }
                        } else {
                            empty_message(
                                &i18n.t("podcast_layout.no_results_found"),
                                &i18n.t("podcast_layout.try_different_keywords")
                            )
                        }
                    } else {
                        empty_message(
                            &i18n.t("podcast_layout.no_results_found"),
                            &i18n.t("podcast_layout.try_different_keywords")
                        )
                    }
                }
                <App_drawer />
            </div>
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
        </>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct PodcastProps {
    pub podcast: UnifiedPodcast,
}

#[function_component(PodcastItem)]
pub fn podcast_item(props: &PodcastProps) -> Html {
    let (i18n, _) = use_translation();
    let podcast = props.podcast.clone();
    let podcast_url = podcast.url.clone();
    let podcast_title = podcast.title.clone();

    let (state, dispatch) = use_store::<AppState>();
    let (podcast_state, podcast_dispatch) = use_store::<PodcastState>();

    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let history = BrowserHistory::new();

    // State to track loading
    let is_loading = use_state(|| false);

    // Pre-capture translation strings for async blocks
    let podcast_removed_msg = i18n.t("podcast_layout.podcast_removed");
    let remove_error_msg = i18n.t("podcast_layout.remove_error");
    let podcast_added_msg = i18n.t("podcast_layout.podcast_added");
    let add_error_msg = i18n.t("podcast_layout.add_error");

    // Check if podcast is already added
    let eff_server_name = server_name.clone();
    let eff_api_key = api_key.clone();
    {
        let podcast_dispatch = podcast_dispatch.clone();
        let podcast_url = podcast_url.clone();
        let podcast_title = podcast_title.clone();

        use_effect_with((), move |_| {
            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                eff_api_key.clone(),
                user_id.clone(),
                eff_server_name.clone(),
            ) {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(response) = call_check_podcast(
                        &server_name,
                        &api_key.clone().unwrap(),
                        user_id,
                        &podcast_title,
                        &podcast_url,
                    )
                    .await
                    {
                        if response.exists {
                            podcast_dispatch.reduce_mut(|state| {
                                let mut new_set = state.added_podcast_urls.clone();
                                new_set.insert(podcast_url);
                                state.added_podcast_urls = new_set;
                            });
                        }
                    }
                });
            }
            || ()
        });
    }

    // Create callback to toggle podcast (add/remove)
    let tog_server_name = server_name.clone();
    let tog_api_key = api_key.clone();
    let toggle_podcast = {
        let podcast_clone = podcast.clone();
        let is_loading = is_loading.clone();
        let podcast_dispatch = podcast_dispatch.clone();
        let dispatch = dispatch.clone();
        let pod_state = podcast_state.clone();
        let podcast_removed_msg = podcast_removed_msg.clone();
        let remove_error_msg = remove_error_msg.clone();
        let podcast_added_msg = podcast_added_msg.clone();
        let add_error_msg = add_error_msg.clone();

        Callback::from(move |_: MouseEvent| {
            let podcast = podcast_clone.clone();
            let podcast_url = podcast.url.clone();
            let is_loading = is_loading.clone();
            let podcast_dispatch = podcast_dispatch.clone();
            let _dispatch = dispatch.clone();
            let podcast_removed_msg = podcast_removed_msg.clone();
            let remove_error_msg = remove_error_msg.clone();
            let podcast_added_msg = podcast_added_msg.clone();
            let add_error_msg = add_error_msg.clone();

            // Get all necessary values for API calls
            let api_key = tog_api_key.clone();
            let server_name = tog_server_name.clone();
            let user_id = user_id.clone();

            is_loading.set(true);

            if pod_state.added_podcast_urls.contains(&podcast_url) {
                // Remove podcast logic
                wasm_bindgen_futures::spawn_local(async move {
                    let podcast_values = RemovePodcastValuesName {
                        podcast_name: podcast.title.clone(),
                        podcast_url: podcast.url.clone(),
                        user_id: user_id.unwrap(),
                    };

                    match call_remove_podcasts_name(
                        &server_name.unwrap(),
                        &api_key.unwrap(),
                        &podcast_values,
                    )
                    .await
                    {
                        Ok(_) => {
                            podcast_dispatch.reduce_mut(|state| {
                                state.added_podcast_urls.remove(&podcast_url);
                            });
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Some(podcast_removed_msg);
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("{}: {:?}", remove_error_msg, formatted_error));
                            });
                        }
                    }

                    is_loading.set(false);
                });
            } else {
                // Add podcast logic
                wasm_bindgen_futures::spawn_local(async move {
                    let podcast_values = PodcastValues {
                        pod_title: podcast.title.clone(),
                        pod_artwork: if podcast.artwork.is_empty() { podcast.image.clone() } else { podcast.artwork.clone() },
                        pod_author: podcast.author.clone(),
                        categories: podcast.categories.unwrap_or_default().clone(),
                        pod_description: podcast.description.clone(),
                        pod_episode_count: podcast.episodeCount.clone(),
                        pod_feed_url: podcast.url.clone(),
                        pod_website: podcast.link.clone(),
                        pod_explicit: podcast.explicit.clone(),
                        user_id: user_id.unwrap(),
                    };

                    match call_add_podcast(
                        &server_name.unwrap(),
                        &api_key.unwrap(),
                        user_id.unwrap(),
                        &podcast_values,
                        podcast.id,
                    )
                    .await
                    {
                        Ok(_) => {
                            podcast_dispatch.reduce_mut(|state| {
                                let mut new_set = state.added_podcast_urls.clone();
                                new_set.insert(podcast_url.clone());
                                state.added_podcast_urls = new_set;
                            });
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Some(podcast_added_msg);
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("{}: {:?}", add_error_msg, formatted_error));
                            });
                        }
                    }

                    is_loading.set(false);
                });
            }
        })
    };

    let podcast_index_clone = podcast.index_id.clone();
    let podcast_title_clone = podcast.title.clone();
    let podcast_url_clone = podcast.url.clone();
    let podcast_description_clone = podcast.description.clone();
    let podcast_author_clone = podcast.author.clone();
    let podcast_artwork_clone = if podcast.artwork.is_empty() {
        podcast.image.clone()
    } else {
        podcast.artwork.clone()
    };
    let podcast_explicit_clone = podcast.explicit.clone();
    let podcast_episode_count_clone = podcast.episodeCount.clone();
    let podcast_categories_clone = podcast.categories.clone();
    let podcast_link_clone = podcast.link.clone();

    // Navigate to episode_layout with full subscription-check via create_on_title_click.
    // This ensures podcast_added is set correctly for both subscribed and unsubscribed podcasts.
    let on_title_click = {
        let categories_str = podcast_categories_clone.clone().map(|cats| {
            cats.values().cloned().collect::<Vec<_>>().join(", ")
        });
        if let (Some(server), Some(uid)) = (server_name.clone(), user_id) {
            create_on_title_click(
                server,
                api_key.clone(),
                &history,
                // Search results carry an external Podcast Index / iTunes id in
                // `podcast.id`, not a DB id. Pass 0 so create_on_title_click takes
                // the slow path (call_check_podcast) and resolves the real DB id
                // for subscribed podcasts / parses the feed for unsubscribed ones.
                0,
                podcast_index_clone,
                podcast_title_clone,
                podcast_url_clone,
                podcast_description_clone,
                podcast_author_clone,
                podcast_artwork_clone,
                podcast_explicit_clone,
                podcast_episode_count_clone,
                categories_str,
                podcast_link_clone,
                uid,
                false, // search results are never YouTube
            )
        } else {
            Callback::from(|_: MouseEvent| {})
        }
    };

    // Determine button icon based on podcast state
    let is_added = podcast_state.added_podcast_urls.contains(&podcast.url);
    let button_icon = if *is_loading {
        "ph-spinner-gap animate-spin"
    } else if is_added {
        "ph-check"
    } else {
        "ph-plus"
    };
    let action_title = if is_added {
        i18n.t("podcast_layout.remove")
    } else {
        i18n.t("podcast_layout.add")
    };

    html! {
        <div class="podcast-card">
            <div class="podcast-card__cover">
                <FallbackImage
                    src={podcast.image.clone()}
                    alt={format!("Cover for {}", podcast.title)}
                    class="podcast-card__img"
                    onclick={on_title_click.clone()}
                />
                {
                    if podcast.explicit {
                        html! {
                            <span class="podcast-card__explicit" title={i18n.t("podcast_layout.explicit")}>{"E"}</span>
                        }
                    } else {
                        html! {}
                    }
                }
                <button
                    class="podcast-card__action"
                    onclick={toggle_podcast}
                    disabled={*is_loading}
                    title={action_title.clone()}
                    aria-label={action_title}
                >
                    <i class={format!("ph {}", button_icon)}></i>
                </button>
            </div>

            <div class="podcast-card__meta">
                <h3 class="podcast-card__title" onclick={on_title_click} title={podcast.title.clone()}>
                    {&podcast.title}
                </h3>

                {
                    if !podcast.author.is_empty() {
                        html! {
                            <p class="podcast-card__author">{format!("{} {}", i18n.t("podcast_layout.by"), podcast.author)}</p>
                        }
                    } else {
                        html! {}
                    }
                }

                <div class="podcast-card__desc">
                    <SafeHtml html={podcast.description.clone()} />
                </div>

                <div class="podcast-card__footer">
                    <i class="ph ph-microphone"></i>
                    <span>
                        {format!("{} {}", podcast.episodeCount, if podcast.episodeCount == 1 { i18n.t("podcast_layout.episode") } else { i18n.t("podcast_layout.episodes") })}
                    </span>
                </div>
            </div>
        </div>
    }
}
