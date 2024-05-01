use std::collections::HashMap;
use std::f64::consts::E;
use yew::Callback;
use web_sys::MouseEvent;
use yewdux::prelude::*; // or wherever your Dispatch type is defined
use crate::components::context::{AppState};
use yew_router::history::{BrowserHistory, History};
use crate::components::podcast_layout::ClickedFeedURL;
use crate::requests::search_pods::{call_get_podcast_episodes, call_parse_podcast_url};
use crate::requests::pod_req::{call_get_podcast_id, call_check_podcast};

pub fn create_on_title_click(
    dispatch: Dispatch<AppState>,
    server_name: String,
    api_key: Option<Option<String>>,
    history: &BrowserHistory,
    podcast_title: String,
    podcast_url: String,
    podcast_description: String,
    podcast_author: String,
    podcast_artwork: String,
    podcast_explicit: bool,
    podcast_episode_count: i32,
    podcast_categories: Option<HashMap<String, String>>,
    podcast_link: String,
    user_id: i32,
    // ... other podcast-specific parameters ...
) -> Callback<MouseEvent> {
    let history = history.clone();
    Callback::from(move |e: MouseEvent| {
        e.prevent_default(); // Prevent default anchor behavior
        dispatch.reduce_mut(|state| {
            state.is_loading = Some(true);
            state.podcast_added = Some(false); // Set podcast_added to false here
        });
        let title_wasm = podcast_title.clone();
        let server_clone = server_name.clone();
        let api_clone = api_key.clone().unwrap();
        let podcast_url_call = podcast_url.clone();
        let podcast_values = ClickedFeedURL {
            podcast_title: podcast_title.clone(),
            podcast_url: podcast_url.clone(),
            podcast_description: podcast_description.clone(),
            podcast_author: podcast_author.clone(),
            podcast_artwork: podcast_artwork.clone(),
            podcast_explicit: podcast_explicit.clone(),
            podcast_episode_count: podcast_episode_count.clone(),
            podcast_categories: podcast_categories.clone(),
            podcast_link: podcast_link.clone(),
        };


        let dispatch = dispatch.clone();
        let history = history.clone(); // Clone again for use inside async block
        wasm_bindgen_futures::spawn_local(async move {
            match call_check_podcast(&server_clone, &api_clone.clone().unwrap(), user_id, &title_wasm, &podcast_url_call).await {
                Ok(response) => {
                    if response.exists {
                        // The podcast exists in the database
                        // Get the podcast id
                        web_sys::console::log_1(&format!("Podcast exists: {:?}", response).into());
                        match call_get_podcast_id(&server_clone, &api_clone, &user_id, &podcast_url_call, &title_wasm).await {
                            Ok(podcast_id) => {
                                web_sys::console::log_1(&format!("Podcast IDs: {:?}", podcast_id).into());

                                match call_get_podcast_episodes(&server_clone, &api_clone, &user_id, &podcast_id).await {
                                    Ok(podcast_feed_results) => {
                                        dispatch.reduce_mut(move |state| {
                                            state.podcast_added = Some(true);
                                            state.podcast_feed_results = Some(podcast_feed_results);
                                            state.clicked_podcast_info = Some(podcast_values);
                                        });
                                        dispatch.reduce_mut(|state| state.is_loading = Some(false));
                                        history.push("/episode_layout"); // Navigate to episode_layout
                                    },
                                    Err(e) => {
                                        web_sys::console::log_1(&format!("Error fetching episodes: {:?}", e).into());
                                    }
                                }

                            },
                            Err(e) => {
                                web_sys::console::log_1(&format!("Error fetching podcast ID: {:?}", e).into());
                            }
                        }
                    } else {
                        match call_parse_podcast_url(server_clone, &api_clone, &podcast_url_call).await {
                            Ok(podcast_feed_results) => {
                                dispatch.reduce_mut(move |state| {
                                    state.podcast_added = Some(false);
                                    state.podcast_feed_results = Some(podcast_feed_results);
                                    state.clicked_podcast_info = Some(podcast_values);
                                });
                                dispatch.reduce_mut(|state| state.is_loading = Some(false));
                                history.push("/episode_layout"); // Navigate to episode_layout
                            },
                            Err(_e) => {
                                // web_sys::console::log_1(&format!("Error: {}", e).into());
                            }
                        }
                    }
                }, Err(e) => {
                    web_sys::console::log_1(&format!("Error: {}", e).into());
                }
            };
        });
    })
}