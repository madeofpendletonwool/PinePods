use std::collections::HashMap;
use std::rc::Rc;
use serde::{Deserialize, Serialize};
use web_sys::MouseEvent;
use yew::{Callback, function_component, Html, html};
use yew_router::history::{BrowserHistory, History};
use yewdux::Dispatch;
use super::app_drawer::App_drawer;
use super::gen_components::Search_nav;
use crate::components::context::{AppState};
use crate::requests::search_pods::{call_get_podcast_info, call_parse_podcast_url, test_connection};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ClickedFeedURL {
    // Add fields according to your API's JSON response
    pub podcast_title: String,
    pub podcast_url: String,
    pub podcast_description: String,
    pub podcast_author: String,
    pub podcast_artwork: String,
    pub podcast_last_update: i64,
    pub podcast_explicit: bool,
    pub podcast_episode_count: i32,
    pub podcast_categories: Option<HashMap<String, String>>,
    pub podcast_link: String
}

// let on_add_click = {
//     let pod_values = clicked_podcast_info.clone();
//
//     let pod_title = pod_values.unwrap().podcast_title.clone();
//     let pod_artwork = pod_values.unwrap().podcast_title.clone();
//     let pod_author = pod_values.unwrap().podcast_author.clone();
//     let categories = pod_values.unwrap().podcast_categories.clone();
//     let pod_description = pod_values.unwrap().podcast_description.clone();
//     let pod_episode_count = pod_values.unwrap().podcast_episode_count.clone();
//     let pod_feed_url = pod_values.unwrap().podcast_url.clone();
//     let pod_website = pod_values.unwrap().podcast_link.clone();
//     let user_id = user_id.unwrap().clone();
//
//
//     fn parse_duration_to_seconds(duration_convert: &str) -> f64 {
//         let parts: Vec<&str> = duration_convert.split(':').collect();
//         let parts: Vec<f64> = parts.iter().map(|part| part.parse::<f64>().unwrap_or(0.0)).collect();
//
//         let seconds = match parts.len() {
//             3 => parts[0] * 3600.0 + parts[1] * 60.0 + parts[2],
//             2 => parts[0] * 60.0 + parts[1],
//             1 => parts[0],
//             _ => 0.0,
//         };
//
//         seconds
//     }

#[function_component(PodLayout)]
pub fn pod_layout() -> Html {
    let dispatch = Dispatch::<AppState>::global();
    let state: Rc<AppState> = dispatch.get();
    let search_results = state.search_results.clone();
    let history = BrowserHistory::new();
    let history_clone = history.clone();

    html! {
    <div class="main-container">
        <Search_nav />
        <h1 class="text-2xl font-bold my-4 center-text">{ "Podcast Search Results" }</h1>
        {
            if let Some(results) = search_results {
                html! {
                    <div>
                        { for results.feeds.iter().map(|podcast| {
                            //Get podcast info for state
                            let podcast_title_clone = podcast.title.clone();
                            let podcast_url_clone = podcast.url.clone();
                            let podcast_description_clone = podcast.description.clone();
                            let podcast_author_clone = podcast.author.clone();
                            let podcast_artwork_clone = podcast.artwork.clone();
                            let podcast_last_update_clone = podcast.lastUpdateTime.clone();
                            let podcast_explicit_clone = podcast.explicit.clone();
                            let podcast_episode_count_clone = podcast.episodeCount.clone();
                            let podcast_categories_clone = podcast.categories.clone();
                            let podcast_link_clone = podcast.link.clone();

                            let dispatch_clone = dispatch.clone(); // Clone the dispatch here
                            let history = history_clone.clone();
                            let on_title_click = {
                                let dispatch = dispatch.clone();
                                let history = history.clone(); // Clone history for use inside the closure

                                Callback::from(move |e: MouseEvent| {
                                    let podcast_title = podcast_title_clone.clone();
                                    let podcast_url = podcast_url_clone.clone();
                                    let podcast_description = podcast_description_clone.clone();
                                    let podcast_author = podcast_author_clone.clone();
                                    let podcast_artwork = podcast_artwork_clone.clone();
                                    let podcast_last_update = podcast_last_update_clone.clone();
                                    let podcast_explicit = podcast_explicit_clone.clone();
                                    let podcast_episode_count = podcast_episode_count_clone.clone();
                                    let podcast_categories = podcast_categories_clone.clone();
                                    let podcast_link = podcast_link_clone.clone();
                                    e.prevent_default(); // Prevent the default anchor behavior
                                    let podcast_values = ClickedFeedURL {
                                        podcast_title,
                                        podcast_url: podcast_url.clone(),
                                        podcast_description,
                                        podcast_author,
                                        podcast_artwork,
                                        podcast_last_update,
                                        podcast_explicit,
                                        podcast_episode_count,
                                        podcast_categories,
                                        podcast_link,
                                    };
                                    let dispatch = dispatch.clone();
                                    let history = history.clone(); // Clone again for use inside async block
                                    wasm_bindgen_futures::spawn_local(async move {
                                        match call_parse_podcast_url(&podcast_url).await {
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
                            };
                            html! {
                                <div key={podcast.id.to_string()} class="item-container flex items-center mb-4 bg-white shadow-md rounded-lg overflow-hidden">
                                    <img src={podcast.image.clone()} alt={format!("Cover for {}", &podcast.title)} class="w-1/4 object-cover"/>
                                    <div class="flex flex-col p-4 space-y-2 w-7/12">
                                        <a onclick={on_title_click} class="item-container-text-link text-xl font-semibold hover:underline">{ &podcast.title }</a>
                                        <p class="item-container-text">{ &podcast.description }</p>
                                    </div>
                                    <button class="item-container-button selector-button w-1/4 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded">
                                        {"Add"}
                                    </button>
                                </div>
                            }
                        })}
                    </div>
                }
            } else {
                html! {
                    <>
                        <div class="empty-episodes-container">
                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                            <h1>{ "No Podcast Search Results Found" }</h1>
                            <p>{"Try searching again with a different set of keywords."}</p>
                        </div>
                    </>
                }
            }
        }
        <App_drawer />
    </div>
}

}