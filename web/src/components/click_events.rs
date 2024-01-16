use std::collections::HashMap;
use yew::Callback;
use web_sys::MouseEvent;
use yewdux::prelude::*; // or wherever your Dispatch type is defined
use crate::components::context::{AppState};
use yew_router::history::{BrowserHistory, History};
use crate::components::podcast_layout::ClickedFeedURL;
use crate::requests::search_pods::call_parse_podcast_url;

pub fn create_on_title_click(
    dispatch: Dispatch<AppState>,
    history: &BrowserHistory,
    podcast_title: String,
    podcast_url: String,
    podcast_description: String,
    podcast_author: String,
    podcast_artwork: String,
    podcast_explicit: bool,
    podcast_episode_count: i32,
    podcast_categories: Option<HashMap<String, String>>,
    podcast_link: String
    // ... other podcast-specific parameters ...
) -> Callback<MouseEvent> {
    let history = history.clone();
    Callback::from(move |e: MouseEvent| {
        e.prevent_default(); // Prevent default anchor behavior
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
            match call_parse_podcast_url(&podcast_url_call).await {
                Ok(podcast_feed_results) => {
                    dispatch.reduce_mut(move |state| {
                        state.podcast_feed_results = Some(podcast_feed_results);
                        state.clicked_podcast_info = Some(podcast_values);
                    });
                    history.push("/episode_layout"); // Navigate to episode_layout
                },
                Err(e) => {
                    web_sys::console::log_1(&format!("Error: {}", e).into());
                }
            }
        });
    })
}