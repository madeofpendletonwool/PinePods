use std::collections::HashMap;
use std::rc::Rc;
use serde::{Deserialize, Serialize};
use web_sys::{console, MouseEvent};
use yew::{Callback, function_component, Html, html};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::Dispatch;
use yewdux::use_store;
use super::app_drawer::App_drawer;
use super::gen_components::Search_nav;
use crate::components::context::{AppState};
use crate::requests::search_pods::{call_parse_podcast_url, Podcast};
use crate::requests::pod_req::{call_check_podcast, call_add_podcast, call_remove_podcasts_name, RemovePodcastValuesName, PodcastValues};
use std::collections::HashSet;
use crate::requests::login_requests::use_check_authentication;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ClickedFeedURL {
    // Add fields according to your API's JSON response
    pub podcast_title: String,
    pub podcast_url: String,
    pub podcast_description: String,
    pub podcast_author: String,
    pub podcast_artwork: String,
    pub podcast_explicit: bool,
    pub podcast_episode_count: i32,
    pub podcast_categories: Option<HashMap<String, String>>,
    pub podcast_link: String
    
}

#[function_component(PodLayout)]
pub fn pod_layout() -> Html {
    // let dispatch = Dispatch::<AppState>::global();
    // let state: Rc<AppState> = dispatch.get();
    let (state, dispatch) = use_store::<AppState>();
    let search_results = state.search_results.clone();    

    let session_dispatch = dispatch.clone();
    let session_state = state.clone();

    use_effect_with((), move |_| {
        // Check if the page reload action has already occurred to prevent redundant execution
        if session_state.reload_occured.unwrap_or(false) {
            // Logic for the case where reload has already been processed
        } else {
            // Normal effect logic for handling page reload
            let window = web_sys::window().expect("no global `window` exists");
            let performance = window.performance().expect("should have performance");
            let navigation_type = performance.navigation().type_();
            
            if navigation_type == 1 { // 1 stands for reload
                let session_storage = window.session_storage().unwrap().unwrap();
                session_storage.set_item("isAuthenticated", "false").unwrap();
            }
    
            // Always check authentication status
            let current_route = window.location().href().unwrap_or_default();
            use_check_authentication(session_dispatch.clone(), &current_route);
    
            // Mark that the page reload handling has occurred
            session_dispatch.reduce_mut(|state| {
                state.reload_occured = Some(true);
                state.clone() // Return the modified state
            });
        }
    
        || ()
    });

    html! {
    <div class="main-container">
        <Search_nav />
        <h1 class="item_container-text text-2xl font-bold my-4 center-text">{ "Podcast Search Results" }</h1>
        {
            if let Some(results) = search_results {
                {
                    html! {
                        <div>
                            { for results.feeds.iter().map(|podcast| {
                                html! {
                                    <PodcastItem podcast={podcast.clone()} />
                                }
                            }) }
                        </div>
                    }
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

#[derive(Properties, PartialEq, Clone)]
pub struct PodcastProps {
    pub podcast: Podcast, // Assuming Podcast is a struct that holds podcast details
}

// Assuming you have a PodcastItem component
#[function_component(PodcastItem)]
pub fn podcast_item(props: &PodcastProps) -> Html {
    // Local state to track if this particular podcast is added
    let is_added = use_state(|| false);
    let podcast = props.podcast.clone();
    let (state, dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let history = BrowserHistory::new();
    let history_clone = history.clone();

        // Use a Set to track added podcast URLs for efficiency
    let added_podcasts = use_state(|| HashSet::new());

    // On mount, check if the podcast is in the database
    let effect_user_id = user_id.unwrap().clone();
    let effect_api_key = api_key.clone();
    let added_clone = added_podcasts.clone();
    {
        let is_added = is_added.clone();
        let podcast = podcast.clone();
        let user_id = effect_user_id.clone();
        let api_key = effect_api_key.clone();
        let server_name = server_name.clone();
        let added_podcasts = added_clone.clone(); // Clone this for use in the effect

        use_effect_with(
            &(),
            move |_| {
                let is_added = is_added.clone();
                let podcast = podcast.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let added = call_check_podcast(&server_name.unwrap(), &api_key.unwrap().unwrap(), user_id, &podcast.title, &podcast.url).await.unwrap_or_default().exists;
                    console::log_1(&format!("{} added: {}", podcast.title, added).into());
                    is_added.set(added);
                    let mut new_set = (*added_podcasts).clone();
                    if added {
                        new_set.insert(podcast.url.clone());
                    } else {
                        new_set.remove(&podcast.url);
                    }
                    added_podcasts.set(new_set);
                });
                || ()
            },
        );
    }

    let podcast_add = podcast.clone();

    let toggle_podcast = {
        let podcast_add = podcast_add.clone();

        let pod_title_og = podcast_add.title.clone();
        let pod_artwork_og = podcast_add.artwork.clone();
        let pod_author_og = podcast_add.author.clone();
        let categories_og = podcast_add.categories.unwrap_or_default().clone();
        let pod_description_og = podcast_add.description.clone();
        let pod_episode_count_og = podcast_add.episodeCount.clone();
        let pod_feed_url_og = podcast_add.url.clone();
        let pod_website_og = podcast_add.link.clone();
        let pod_explicit_og = podcast_add.explicit.clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let user_id_clone = user_id.clone();

        let added_podcasts = added_podcasts.clone();
        let dispatch = dispatch.clone(); // Clone the dispatch for updating global state after removing
        let podcast_url = podcast.url.clone(); // The URL of the podcast to toggle
        let pod_title_og_clone = pod_title_og.clone();

        Callback::from(move |_: MouseEvent| {
            
            // Create a new set from the current state for modifications.
            let user_id = user_id_clone.clone();
            let api_key = api_key_clone.clone();
            let server_name = server_name_clone.clone();

            let current_set = (*added_podcasts).clone();

            let dispatch = dispatch.clone();
            let added_podcasts = added_podcasts.clone();
            let podcast_url = podcast_url.clone();
            
            if current_set.contains(&podcast_url) {
                // If the podcast was added, remove it from the set and call remove_podcast.
                // Call remove_podcast asynchronously.
                let pod_title_og = pod_title_og_clone.clone();
                let pod_feed_url_og = pod_feed_url_og.clone();
                let value_id = user_id.clone().unwrap();
                let podcast_url = podcast_url.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let pod_title = pod_title_og.clone();
                    let pod_feed_url = pod_feed_url_og.clone();
                    let podcast_url = podcast_url.clone();
                    let podcast_values = RemovePodcastValuesName {
                        podcast_name: pod_title,
                        podcast_url: pod_feed_url,
                        user_id: value_id
                    };
                    match call_remove_podcasts_name(&server_name.unwrap(), &api_key.unwrap(), &podcast_values).await {
                        Ok(_) => {
                            // If successful, update the state to remove the podcast
                            let mut new_set = current_set.clone();
                            new_set.remove(&podcast_url);
                            added_podcasts.set(new_set);
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some("Podcast successfully removed".to_string());
                            });
                        },
                        Err(e) => {
                            console::log_1(&format!("Error removing podcast: {:?}", e).into());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Error removing podcast: {:?}", e));
                            });
                        }
                    }
                });
            } else {
                // If the podcast was not added, add it to the set and call add_podcast.
                let pod_title_og = pod_title_og.clone();
                let pod_artwork_og = pod_artwork_og.clone();
                let pod_author_og = pod_author_og.clone();
                let categories_og = categories_og.clone();
                let pod_description_og = pod_description_og.clone();
                let pod_episode_count_og = pod_episode_count_og.clone();
                let pod_feed_url_og = pod_feed_url_og.clone();
                let pod_website_og = pod_website_og.clone();
                let pod_explicit_og = pod_explicit_og.clone();
                
                wasm_bindgen_futures::spawn_local(async move {
                    let pod_title = pod_title_og.clone();
                    let pod_artwork = pod_artwork_og.clone();
                    let pod_author = pod_author_og.clone();
                    let categories = categories_og.clone();
                    let pod_description = pod_description_og.clone();
                    let pod_episode_count = pod_episode_count_og.clone();
                    let pod_feed_url = pod_feed_url_og.clone();
                    let pod_website = pod_website_og.clone();
                    let pod_explicit = pod_explicit_og.clone();
                    let value_id = user_id.clone().unwrap();
                    let podcast_values = PodcastValues {
                        pod_title,
                        pod_artwork,
                        pod_author,
                        categories,
                        pod_description,
                        pod_episode_count,
                        pod_feed_url,
                        pod_website,
                        pod_explicit,
                        user_id: value_id
                    };
                    match call_add_podcast(&server_name.unwrap(), &api_key.unwrap(), user_id.unwrap(), &podcast_values).await {
                        Ok(_) => {
                            // If successful, update the state to add the podcast
                            let mut new_set = current_set.clone();
                            new_set.insert(podcast_url.clone());
                            added_podcasts.set(new_set);
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some("Podcast successfully added".to_string());
                            });
                        },
                        Err(e) => {
                            console::log_1(&format!("Error adding podcast: {:?}", e).into());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Error adding podcast: {:?}", e));
                            });
                        }
                    }
                });
            }
        })
    };

    let podcast_title_clone = podcast.title.clone();
    let podcast_url_clone = podcast.url.clone();
    let podcast_description_clone = podcast.description.clone();
    let podcast_author_clone = podcast.author.clone();
    let podcast_artwork_clone = podcast.artwork.clone();
    let podcast_explicit_clone = podcast.explicit.clone();
    let podcast_episode_count_clone = podcast.episodeCount.clone();
    let podcast_categories_clone = podcast.categories.clone();
    let podcast_link_clone = podcast.link.clone();
    let history = history_clone.clone();
    // let is_added = added_podcasts.contains(&podcast.url);
    // let button_text = if is_added { "Remove" } else { "Add" };
    // let button_class = if is_added { "bg-red-500" } else { "bg-blue-500" };
    let is_added = added_podcasts.contains(&podcast.url);
    let button_text = if is_added { "delete" } else { "add" };
    let button_class = if is_added { "bg-red-500" } else { "bg-blue-500" };
    console::log_1(&format!("Is added: {}", button_text.clone()).into());
    console::log_1(&format!("Is added: {}", button_class.clone()).into());
    
    let on_title_click = {
        let dispatch = dispatch.clone();
        let history = history.clone(); // Clone history for use inside the closure

        Callback::from(move |e: MouseEvent| {
            let podcast_title = podcast_title_clone.clone();
            let podcast_url = podcast_url_clone.clone();
            let podcast_description = podcast_description_clone.clone();
            let podcast_author = podcast_author_clone.clone();
            let podcast_artwork = podcast_artwork_clone.clone();
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
        <div>
            {
                html! {
                    <div key={podcast.id.to_string()} class="item-container flex mb-4 shadow-md rounded-lg overflow-hidden">
                        <img src={podcast.image.clone()} alt={format!("Cover for {}", &podcast.title)} class="w-1/6 items-center object-cover"/>
                        <div class="flex items-start flex-col p-4 space-y-2 w-11/12">
                            <a onclick={on_title_click.clone()} class="item-container-text-link text-xl font-semibold hover:underline">{ &podcast.title }</a>
                            <p class="item_container-text">{ &podcast.description }</p>
                            <p class="header-text">{ format!("Episode Count: {}", &podcast.episodeCount) }</p>
                        </div>
                        <div class="button-container flex justify-center items-center w-1/4"> // Modified for better clarity
                            <button class={format!("selector-button font-bold py-2 px-4 rounded {}", button_class)} style={"min-width: 35px;"}>
                                <span class="material-icons" onclick={toggle_podcast}>{ button_text }</span>
                                // { button_text }
                            </button>
                        </div>
                    </div>
                }
            }
        </div>
    }
    
    
}