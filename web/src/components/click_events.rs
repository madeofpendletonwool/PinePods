use crate::components::context::{PageLoadState, PodcastFeedState, SearchState};
use crate::pages::podcast_layout::ClickedFeedURL;
use crate::requests::pod_req::{call_check_podcast, call_get_podcast_id};
use crate::requests::search_pods::{
    call_get_podcast_episodes, call_get_youtube_episodes, call_parse_podcast_url,
};
use std::collections::HashMap;
use web_sys::MouseEvent;
use yew::Callback;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

pub fn create_on_title_click(
    server_name: String,
    api_key: Option<Option<String>>,
    history: &BrowserHistory,
    podcast_id: i32,
    podcast_index_id: i32,
    podcast_title: String,
    podcast_url: String,
    podcast_description: String,
    podcast_author: String,
    podcast_artwork: String,
    podcast_explicit: bool,
    podcast_episode_count: i32,
    podcast_categories: Option<String>,
    podcast_link: String,
    user_id: i32,
    is_youtube: bool,
) -> Callback<MouseEvent> {
    let history = history.clone();
    Callback::from(move |e: MouseEvent| {
        e.prevent_default();

        let api_clone = api_key.clone().unwrap();

        // Convert the categories string to a HashMap with integer keys
        let podcast_categories_map: Option<HashMap<String, String>> =
            podcast_categories.as_ref().map(|cats| {
                cats.split(", ")
                    .enumerate()
                    .map(|(i, cat)| (i.to_string(), cat.to_string()))
                    .collect()
            });

        let podcast_values = ClickedFeedURL {
            podcastid: podcast_id,
            podcastname: podcast_title.clone(),
            feedurl: podcast_url.clone(),
            description: podcast_description.clone(),
            author: podcast_author.clone(),
            artworkurl: podcast_artwork.clone(),
            explicit: podcast_explicit,
            episodecount: podcast_episode_count,
            categories: podcast_categories_map,
            websiteurl: podcast_link.clone(),
            podcastindexid: podcast_index_id,
            is_youtube: Some(is_youtube),
        };

        // Clear stale episode data so episode_layout shows a clean loading state
        Dispatch::<SearchState>::global().reduce_mut(|state| {
            state.podcast_feed_results = None;
        });
        // Publish podcast info and initial subscription status immediately
        Dispatch::<PodcastFeedState>::global().reduce_mut(|state| {
            state.podcast_added = Some(podcast_id > 0);
            state.clicked_podcast_info = Some(podcast_values.clone());
        });
        Dispatch::<PageLoadState>::global().reduce_mut(|state| {
            state.is_loading = Some(true);
        });

        // Navigate FIRST — episode_layout mounts and shows its loading state immediately.
        // Episode data is then fetched in the background below.
        let history = history.clone();
        history.push("/episode_layout");

        let server_clone = server_name.clone();
        let podcast_url_call = podcast_url.clone();
        let title_wasm = podcast_title.clone();

        wasm_bindgen_futures::spawn_local(async move {
            if podcast_id > 0 {
                // Fast path: podcast is already in the user's library.
                // The id is known — skip call_check_podcast and call_get_podcast_id entirely.
                let fetch_result = if is_youtube {
                    call_get_youtube_episodes(&server_clone, &api_clone, &user_id, &podcast_id)
                        .await
                } else {
                    call_get_podcast_episodes(
                        &server_clone,
                        &api_clone,
                        &user_id,
                        &podcast_id,
                        Some(50),
                        Some(0),
                        None,
                        None,
                        None,
                        None,
                    )
                    .await
                };

                match fetch_result {
                    Ok(mut results) => {
                        for episode in &mut results.episodes {
                            episode.is_youtube = is_youtube;
                        }
                        Dispatch::<SearchState>::global().reduce_mut(move |state| {
                            state.podcast_feed_results = Some(results);
                        });
                        Dispatch::<PodcastFeedState>::global().reduce_mut(|state| {
                            state.podcast_added = Some(true);
                        });
                    }
                    Err(e) => {
                        web_sys::console::log_1(
                            &format!("Error fetching episodes: {:?}", e).into(),
                        );
                    }
                }
                Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                    state.is_loading = Some(false);
                });
            } else {
                // Slow path: unknown subscription status (search results, external links).
                // Still need to determine if the podcast is in the DB.
                match call_check_podcast(
                    &server_clone,
                    &api_clone.clone().unwrap_or_default(),
                    user_id,
                    &title_wasm,
                    &podcast_url_call,
                )
                .await
                {
                    Ok(response) => {
                        if response.exists {
                            match call_get_podcast_id(
                                &server_clone,
                                &api_clone,
                                &user_id,
                                &podcast_url_call,
                                &title_wasm,
                            )
                            .await
                            {
                                Ok(db_podcast_id) => {
                                    let fetch_result = if is_youtube {
                                        call_get_youtube_episodes(
                                            &server_clone,
                                            &api_clone,
                                            &user_id,
                                            &db_podcast_id,
                                        )
                                        .await
                                    } else {
                                        call_get_podcast_episodes(
                                            &server_clone,
                                            &api_clone,
                                            &user_id,
                                            &db_podcast_id,
                                            Some(50),
                                            Some(0),
                                            None,
                                            None,
                                            None,
                                            None,
                                        )
                                        .await
                                    };

                                    match fetch_result {
                                        Ok(mut results) => {
                                            for episode in &mut results.episodes {
                                                episode.is_youtube = is_youtube;
                                            }
                                            Dispatch::<SearchState>::global().reduce_mut(
                                                move |state| {
                                                    state.podcast_feed_results = Some(results);
                                                },
                                            );
                                            // Update clicked_podcast_info with the real DB id
                                            Dispatch::<PodcastFeedState>::global().reduce_mut(
                                                move |state| {
                                                    state.podcast_added = Some(true);
                                                    if let Some(ref mut info) =
                                                        state.clicked_podcast_info
                                                    {
                                                        info.podcastid = db_podcast_id;
                                                    }
                                                },
                                            );
                                        }
                                        Err(e) => {
                                            web_sys::console::log_1(
                                                &format!("Error fetching episodes: {:?}", e)
                                                    .into(),
                                            );
                                        }
                                    }
                                }
                                Err(e) => {
                                    web_sys::console::log_1(
                                        &format!("Error fetching podcast ID: {:?}", e).into(),
                                    );
                                }
                            }
                        } else {
                            // Not in DB — parse the feed URL directly
                            match call_parse_podcast_url(
                                server_clone,
                                &api_clone,
                                &podcast_url_call,
                            )
                            .await
                            {
                                Ok(results) => {
                                    Dispatch::<SearchState>::global().reduce_mut(move |state| {
                                        state.podcast_feed_results = Some(results);
                                    });
                                    Dispatch::<PodcastFeedState>::global().reduce_mut(
                                        move |state| {
                                            state.podcast_added = Some(false);
                                        },
                                    );
                                }
                                Err(_) => {}
                            }
                        }
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Error checking podcast: {}", e).into());
                    }
                }
                Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                    state.is_loading = Some(false);
                });
            }
        });
    })
}
