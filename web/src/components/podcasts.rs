use std::collections::HashMap;
use std::rc::Rc;

use yew::{function_component, Html, html};
use yew::prelude::*;
use yewdux::prelude::*;
use super::app_drawer::App_drawer;
use crate::components::gen_components::Search_nav;
use crate::requests::pod_req::{PodcastResponse, RemovePodcastValues, call_remove_podcasts};
use crate::requests::pod_req;
use web_sys::console;
use crate::components::context::{AppState, UIState};
use web_sys::console::error;
use yew_router::history::BrowserHistory;
use crate::components::click_events::create_on_title_click;

enum AppStateMsg {
    // ... other messages ...
    RemovePodcast(i32), // Add this line
}

impl Reducer<AppState> for AppStateMsg {
    fn apply(self, mut state: Rc<AppState>) -> Rc<AppState> {
        let state_mut = Rc::make_mut(&mut state);

        match self {
            // ... other cases ...
            AppStateMsg::RemovePodcast(podcast_id) => {
                if let Some(podcasts) = &mut state_mut.podcast_feed_return {
                    web_sys::console::log_1(&format!("podcast pod pre-change: {:?}", &podcasts.pods).into());
                    podcasts.pods = Some(
                        podcasts.pods
                            .as_ref()
                            .unwrap_or(&vec![])
                            .iter()
                            .filter(|p| p.PodcastID != podcast_id)
                            .cloned()
                            .collect()
                    );
                    web_sys::console::log_1(&format!("podcast pod state: {:?}", &podcasts.pods).into());
                }
            }
        }

        state
    }
}

#[function_component(Podcasts)]
pub fn podcasts() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    console::log_1(&format!("User Context in podcasts: {:?}", &state.user_details).into());
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let podcast_feed_return = state.podcast_feed_return.clone();

    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    // Fetch episodes on component mount
    {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        // let episodes = episodes.clone();

        console::log_1(&"Test log on podcasts".to_string().into());
        if let Some(api_key) = &api_key {
            console::log_1(&format!("API Key: {:?}", api_key).into());
        }
        if let Some(user_id) = user_id {
            console::log_1(&format!("User ID: {}", user_id).into());
        }
        if let Some(server_name) = &server_name {
            console::log_1(&format!("Server Name: {}", server_name).into());
        }

        // Dependencies for use_effect_with
        let dependencies = (
            state.auth_details.as_ref().map(|ud| ud.api_key.clone()),
            state.user_details.as_ref().map(|ud| ud.UserID.clone()),
            state.auth_details.as_ref().map(|ud| ud.server_name.clone()),
        );

        let server_name_effect = server_name.clone();
        let user_id_effect = user_id.clone();
        let api_key_effect = api_key.clone();
        let effect_dispatch = dispatch.clone();

        console::log_1(&format!("server_name: {:?}", &server_name_effect).into());
        console::log_1(&format!("user_id: {:?}", &user_id_effect).into());
        console::log_1(&format!("api_key: {:?}", &api_key_effect).into());

        use_effect_with(
            (api_key_effect, user_id_effect, server_name_effect),
            move |_| {
                console::log_1(&format!("User effect running: {:?}", &server_name).into());
                // let episodes_clone = episodes.clone();
                let error_clone = error.clone();

                if let (Some(api_key), Some(user_id), Some(server_name)) = (api_key.clone(), user_id.clone(), server_name.clone()) {
                    let dispatch = effect_dispatch.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_podcasts(&server_name, &api_key, &user_id).await {
                            Ok(fetched_podcasts) => {
                                dispatch.reduce_mut(move |state| {
                                    state.podcast_feed_return = Some(PodcastResponse { pods: Some(fetched_podcasts) });
                                });
                            },
                            Err(e) => console::log_1(&format!("Unable to parse Podcasts: {:?}", &e).into()),
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
            {
                if let Some(podcasts) = state.podcast_feed_return.clone() {
                    let int_podcasts = podcasts.clone();
                    if let Some(pods) = int_podcasts.pods.clone() {
                        if pods.is_empty() {
                                                    // Render "No Recent Episodes Found" if episodes list is empty
                            html! {
                        <div class="empty-episodes-container">
                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                            <h1>{ "No Podcasts Found" }</h1>
                            <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                        </div>
                            }
                        } else {
                        pods.into_iter().map(|podcast| {
                            let state_ep = state.clone();
                            let audio_state_ep = audio_state.clone();
                            let api_key_iter = api_key.clone();
                            let history = history_clone.clone();

                            let id_string = &podcast.PodcastID.to_string();
    
                            let dispatch = dispatch.clone();
                            let podcast_id_loop = podcast.PodcastID.clone();
                            let podcast_url_clone = podcast.FeedURL.clone();
                            let podcast_title_clone = podcast.PodcastName.clone();
                            let podcast_ep_count = podcast.EpisodeCount.clone();
                            // let podcast_artwork_clone = podcast.ArtworkURL.clone();
                            let podcast_description_clone = podcast.Description.clone();
                            let podcast_website_clone = podcast.WebsiteURL.clone();
                            let podcast_author_clone = podcast.Author.clone();
                            let podcast_categories_clone = podcast.Categories.clone();
                            let categories: HashMap<String, String> = serde_json::from_str(&podcast_categories_clone).unwrap_or_else(|_| HashMap::new());
                            let on_remove_click = {
                                let dispatch_remove = dispatch.clone();
                                let podcast_feed_return = podcast_feed_return.clone();
                                let user_id = user_id.unwrap();

                                let api_key_rm = api_key_iter.clone();
                                let server_name = server_name.clone();

                                Callback::from(move |_: MouseEvent| {
                                    let dispatch_call = dispatch_remove.clone();
                                    let api_key_call = api_key_rm.clone();
                                    let server_name_call = server_name.clone();
                                    let user_id = user_id;

                                    if let Some(podcasts) = &podcast_feed_return {
                                        for podcast in &podcasts.pods {
                                            let dispatch_for = dispatch_call.clone();
                                            let api_key_for = api_key_call.clone();
                                            let server_name_for = server_name_call.clone();
                                            let podcast_id = podcast_id_loop.clone(); // Use the correct podcast ID

                                            let remove_values = RemovePodcastValues {
                                                podcast_id,
                                                user_id,
                                            };

                                            wasm_bindgen_futures::spawn_local(async move {
                                                let dispatch_clone = dispatch_for.clone();
                                                let api_key_wasm = api_key_for.clone();
                                                let server_name_wasm = server_name_for.clone();
                                                match call_remove_podcasts(&server_name_wasm.unwrap(), &api_key_wasm.unwrap(), &remove_values).await {
                                                    Ok(success) => {
                                                        if success {
                                                            dispatch_clone.apply(AppStateMsg::RemovePodcast(podcast_id));
                                                            console::log_1(&"Podcast successfully removed".into());
                                                            dispatch_clone.reduce_mut(|state| {
                                                                state.info_message = Some("Podcast successfully removed".to_string())
                                                            });
                                                        } else {
                                                            console::log_1(&"Failed to remove podcast".into());
                                                            dispatch_clone.reduce_mut(|state| {
                                                                state.error_message = Some("Failed to remove podcast".to_string())
                                                            });
                                                        }
                                                    },
                                                    Err(e) => {
                                                        console::log_1(&format!("Error removing podcast: {:?}", e).into());
                                                        dispatch_clone.reduce_mut(|state| {
                                                            state.error_message = Some(format!("Error removing podcast: {:?}", e))
                                                        });
                                                    }
                                                }
                                            });
                                        }
                                    }
                                })
                            };
                            let categories: HashMap<String, String> = serde_json::from_str(&podcast.Categories)
                                .unwrap_or_else(|_| HashMap::new());
                            let on_title_click = create_on_title_click(
                                dispatch.clone(),
                                &history,
                                podcast.PodcastName.clone(),
                                podcast.FeedURL.clone(),
                                podcast.Description.clone(),
                                podcast.Author.clone(),
                                podcast.ArtworkURL.clone(),
                                podcast.PodcastExplicit.clone(),
                                podcast.EpisodeCount.clone(),
                                Some(categories),
                                podcast.WebsiteURL.clone(),
                            );
    
                            html! {
                                <div>
                                    <div class="item-container flex items-center mb-4 bg-white shadow-md rounded-lg overflow-hidden">
                                        <img src={podcast.ArtworkURL.clone()} alt={format!("Cover for {}", &podcast.PodcastName)} class="w-2/12 object-cover"/>
                                        <div class="flex flex-col p-4 space-y-2 w-9/12">
                                            <a onclick={on_title_click} class="item-container-text-link text-xl font-semibold hover:underline">{ &podcast.PodcastName }</a>
                                            {
                                                html! {
                                                    <div class="item_container-text episode-description-container">
                                                        <div>
                                                            <p> {podcast_description_clone} </p>
                                                        </div>
                                                    </div>
                                                }
                                            }
                                            <p class="item-container-text">{ &podcast.EpisodeCount }</p>
                                        </div>
                                        <button class="item-container-button selector-button w-1/12 font-bold py-2 px-4 rounded">
                                            <span class="material-icons" onclick={on_remove_click}>{"delete"}</span>
                                        </button>
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                        }
                    } else {
                        html! {
                            <div class="empty-episodes-container">
                                <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                <h1>{ "No Podcasts Found" }</h1>
                                <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                            </div>
                        }
                    }
                } else {
                    html! {
                        <div class="empty-episodes-container">
                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                            <h1>{ "No Podcasts Found" }</h1>
                            <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                        </div>
                    }
                }
            }
        </div>
        <App_drawer />
        </>
    }
}