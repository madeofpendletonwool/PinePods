use super::app_drawer::App_drawer;
use super::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, PodcastState, UIState};
use crate::components::gen_funcs::format_error_message;
use crate::components::safehtml::SafeHtml;
use crate::requests::pod_req::{
    call_add_podcast, call_check_podcast, call_remove_podcasts_name, PodcastDetails, PodcastValues,
    RemovePodcastValuesName,
};
use crate::requests::search_pods::{call_parse_podcast_url, UnifiedPodcast};
use gloo::events::EventListener;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew::{function_component, html, Callback, Html};
use yew_router::history::{BrowserHistory, History};
use yewdux::use_store;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ClickedFeedURL {
    pub podcastid: i64,
    pub podcastname: String,
    pub feedurl: String,
    pub description: String,
    pub author: String,
    pub artworkurl: String,
    pub explicit: bool,
    pub episodecount: i32,
    pub categories: Option<HashMap<String, String>>,
    pub websiteurl: String,
    pub podcastindexid: i64,
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
            podcastindexid: Some(self.podcastindexid),
            is_youtube: self.is_youtube.unwrap_or(false),
        }
    }
}

#[function_component(PodLayout)]
pub fn pod_layout() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let search_results = state.search_results.clone();

    // Track window width to apply responsive columns
    let columns = use_state(|| 2); // Default to 2 columns

    {
        let columns = columns.clone();

        use_effect_with((), move |_| {
            let update_columns = {
                let columns = columns.clone();

                Callback::from(move |_| {
                    if let Some(window) = web_sys::window() {
                        let width = window.inner_width().unwrap().as_f64().unwrap();

                        // Progressive breakpoints for different screen sizes
                        let new_columns = if width < 640.0 {
                            2 // Extra small screens: 2 columns
                        } else if width < 1024.0 {
                            2 // Small to medium screens: 2 columns
                        } else if width < 1280.0 {
                            3 // Large screens: 3 columns
                        } else {
                            4 // Extra large screens: 4 columns
                        };

                        columns.set(new_columns);
                    }
                })
            };

            // Initial update
            update_columns.emit(());

            // Add resize listener
            let window = web_sys::window().unwrap();
            let listener = EventListener::new(&window, "resize", move |_| {
                update_columns.emit(());
            });

            // Cleanup
            move || drop(listener)
        });
    }

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                <h1 class="item_container-text text-2xl font-bold my-6 text-center">{ "Podcast Search Results" }</h1>

                {
                    if let Some(results) = search_results {
                        let podcasts = results.feeds.as_ref().map_or_else(
                            || results.results.as_ref().map(|r| r.iter().filter(|item| item.feedUrl.is_some()).map(|item| item.clone().into()).collect::<Vec<UnifiedPodcast>>()),
                            |f| Some(f.iter().map(|item| item.clone().into()).collect::<Vec<UnifiedPodcast>>())
                        );

                        if let Some(podcasts) = podcasts {
                            if !podcasts.is_empty() {
                                let column_width = format!("calc({}% - {}px)", 100.0 / *columns as f32, 16);

                                html! {
                                    <div class="podcast-flex-container" style="display: flex; flex-wrap: wrap; gap: 16px; padding: 0 12px 24px; width: 100%;">
                                        { for podcasts.iter().map(|podcast| html! {
                                            <div style={format!("width: {}; margin-bottom: 16px;", column_width)}>
                                                <PodcastItem podcast={podcast.clone()} />
                                            </div>
                                        })}
                                    </div>
                                }
                            } else {
                                empty_message(
                                    "No Podcast Search Results Found",
                                    "Try searching again with a different set of keywords."
                                )
                            }
                        } else {
                            empty_message(
                                "No Podcast Search Results Found",
                                "Try searching again with a different set of keywords."
                            )
                        }
                    } else {
                        empty_message(
                            "No Podcast Search Results Found",
                            "Try searching again with a different set of keywords."
                        )
                    }
                }
                <App_drawer />
            </div>
            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
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
    let podcast = props.podcast.clone();
    let podcast_url = podcast.url.clone();
    let podcast_title = podcast.title.clone();

    let (state, dispatch) = use_store::<AppState>();
    let (podcast_state, podcast_dispatch) = use_store::<PodcastState>();

    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let history = BrowserHistory::new();

    // State to track loading and description expansion
    let is_loading = use_state(|| false);
    let is_description_expanded = use_state(|| false);

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

        Callback::from(move |_: MouseEvent| {
            let podcast = podcast_clone.clone();
            let podcast_url = podcast.url.clone();
            let is_loading = is_loading.clone();
            let podcast_dispatch = podcast_dispatch.clone();
            let dispatch = dispatch.clone();

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
                            dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Podcast successfully removed".to_string());
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error removing podcast: {:?}", formatted_error));
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
                        pod_artwork: podcast.artwork.clone(),
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
                        Some(podcast.id),
                    )
                    .await
                    {
                        Ok(_) => {
                            podcast_dispatch.reduce_mut(|state| {
                                let mut new_set = state.added_podcast_urls.clone();
                                new_set.insert(podcast_url.clone());
                                state.added_podcast_urls = new_set;
                            });
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some("Podcast successfully added".to_string());
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error adding podcast: {:?}", formatted_error));
                            });
                        }
                    }

                    is_loading.set(false);
                });
            }
        })
    };

    let podcast_id_clone = podcast.id.clone();
    let podcast_index_clone = podcast.index_id.clone();
    let podcast_title_clone = podcast.title.clone();
    let podcast_url_clone = podcast.url.clone();
    let podcast_description_clone = podcast.description.clone();
    let podcast_author_clone = podcast.author.clone();
    let podcast_artwork_clone = podcast.artwork.clone();
    let podcast_explicit_clone = podcast.explicit.clone();
    let podcast_episode_count_clone = podcast.episodeCount.clone();
    let podcast_categories_clone = podcast.categories.clone();
    let podcast_link_clone = podcast.link.clone();

    // Create callback to open podcast details
    // This is the original on_title_click function to restore
    let on_title_click = {
        let dispatch = dispatch.clone();
        let history = history.clone(); // Clone history for use inside the closure

        Callback::from(move |e: MouseEvent| {
            dispatch.reduce_mut(|state| state.is_loading = Some(true));
            let server_name_click = server_name.clone();
            let api_key_click = api_key.clone();
            let podcast_id = podcast_id_clone.clone();
            let podcast_title = podcast_title_clone.clone();
            let podcast_url = podcast_url_clone.clone();
            let podcast_description = podcast_description_clone.clone();
            let podcast_author = podcast_author_clone.clone();
            let podcast_artwork = podcast_artwork_clone.clone();
            let podcast_explicit = podcast_explicit_clone.clone();
            let podcast_episode_count = podcast_episode_count_clone.clone();
            let podcast_categories = podcast_categories_clone.clone();
            let podcast_link = podcast_link_clone.clone();
            let podcast_index_id = podcast_index_clone.clone();
            e.prevent_default(); // Prevent the default anchor behavior
            let podcast_values = ClickedFeedURL {
                podcastid: podcast_id,
                podcastname: podcast_title,
                feedurl: podcast_url.clone(),
                description: podcast_description,
                author: podcast_author,
                artworkurl: podcast_artwork,
                explicit: podcast_explicit,
                episodecount: podcast_episode_count,
                categories: podcast_categories,
                websiteurl: podcast_link,
                podcastindexid: podcast_index_id,
                is_youtube: Some(false),
            };
            let dispatch = dispatch.clone();
            let history = history.clone(); // Clone again for use inside async block
            wasm_bindgen_futures::spawn_local(async move {
                match call_parse_podcast_url(
                    server_name_click.unwrap(),
                    &api_key_click.unwrap(),
                    &podcast_url,
                )
                .await
                {
                    Ok(podcast_feed_results) => {
                        dispatch.reduce_mut(move |state| {
                            state.podcast_feed_results = Some(podcast_feed_results);
                            state.clicked_podcast_info = Some(podcast_values);
                        });
                        dispatch.reduce_mut(|state| state.is_loading = Some(false));
                        history.push("/episode_layout"); // Navigate to episode_layout
                    }
                    Err(_e) => {
                        // web_sys::console::log_1(&format!("Error: {}", e).into());
                    }
                }
            });
        })
    };

    // Toggle description expansion
    let toggle_description = {
        let is_description_expanded = is_description_expanded.clone();

        Callback::from(move |_: MouseEvent| {
            is_description_expanded.set(!*is_description_expanded);
        })
    };

    // Determine button text based on podcast state
    let is_added = podcast_state.added_podcast_urls.contains(&podcast.url);
    let button_icon = if *is_loading {
        "ph-spinner-gap animate-spin"
    } else if is_added {
        "ph-trash"
    } else {
        "ph-plus-circle"
    };

    html! {
        <div class="search-item-container border-solid border rounded-lg overflow-hidden shadow-md flex flex-col h-full">
            <div class="relative w-full search-podcast-image-container" style="aspect-ratio: 1/1; padding-bottom: 100%;">
                <FallbackImage
                    src={podcast.image.clone()}
                    alt={format!("Cover for {}", podcast.title)}
                    class="absolute inset-0 w-full h-full object-cover transition-transform duration-200 hover:scale-105 cursor-pointer"
                    onclick={on_title_click.clone()}
                />
            </div>

            <div class="p-4 flex flex-col flex-grow">
                <div class="flex justify-between items-start mb-2">
                    <h3
                        class="item_container-text text-xl font-semibold cursor-pointer line-clamp-2 hover:text-opacity-80 transition-colors"
                        onclick={on_title_click}
                    >
                        {&podcast.title}
                    </h3>

                    <button
                        class="item-container-button selector-button flex items-center justify-center rounded-full ml-3 flex-shrink-0 transition-all duration-200 ease-in-out hover:bg-opacity-80"
                        style="width: 40px; height: 40px;"
                        onclick={toggle_podcast}
                        disabled={*is_loading}
                    >
                        <i class={format!("ph {} text-2xl", button_icon)}></i>
                    </button>
                </div>

                {
                    if !podcast.author.is_empty() {
                        html! {
                            <p class="item_container-text text-sm mb-2 opacity-80">{"By "}{&podcast.author}</p>
                        }
                    } else {
                        html! {}
                    }
                }

                <div
                    class={if *is_description_expanded { "item_container-text text-sm mb-3" } else { "item_container-text text-sm mb-3 line-clamp-3" }}
                    onclick={toggle_description.clone()}
                >
                    <SafeHtml html={podcast.description.clone()} />
                </div>

                {
                    if podcast.description.len() > 150 {
                        html! {
                            <button
                                class="text-sm font-medium mb-3 text-left hover:underline item_container-text opacity-80"
                                onclick={toggle_description}
                            >
                                {if *is_description_expanded { "Show less" } else { "Show more" }}
                            </button>
                        }
                    } else {
                        html! {}
                    }
                }

                <div class="mt-auto">
                    <div class="flex items-center">
                        <i class="ph ph-microphone text-lg mr-2 item_container-text"></i>
                        <span class="item_container-text text-sm">
                            {format!("{} episode{}", podcast.episodeCount, if podcast.episodeCount == 1 { "" } else { "s" })}
                        </span>
                    </div>

                    {
                        if podcast.explicit {
                            html! {
                                <div class="flex items-center mt-1">
                                    <span class="bg-red-600 text-white text-xs px-1.5 py-0.5 rounded">{"E"}</span>
                                    <span class="item_container-text text-sm ml-2">{"Explicit"}</span>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </div>
    }
}
