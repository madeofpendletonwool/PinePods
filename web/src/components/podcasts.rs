use std::collections::HashMap;
use std::rc::Rc;

use super::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::episodes_layout::SafeHtml;
use crate::components::gen_components::{Search_nav, UseScrollToTop};
use crate::requests::login_requests::use_check_authentication;
use crate::requests::pod_req;
use crate::requests::pod_req::{call_remove_podcasts, PodcastResponse, RemovePodcastValues};
use wasm_bindgen::prelude::*;
use web_sys::console;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

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
                    podcasts.pods = Some(
                        podcasts
                            .pods
                            .as_ref()
                            .unwrap_or(&vec![])
                            .iter()
                            .filter(|p| p.podcastid != podcast_id)
                            .cloned()
                            .collect(),
                    );
                }
            }
        }

        state
    }
}

#[function_component(Podcasts)]
pub fn podcasts() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let podcast_feed_return = state.podcast_feed_return.clone();

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

            if navigation_type == 1 {
                // 1 stands for reload
                let session_storage = window.session_storage().unwrap().unwrap();
                session_storage
                    .set_item("isAuthenticated", "false")
                    .unwrap();
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

    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    // Fetch episodes on component mount
    {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        // let episodes = episodes.clone();

        let server_name_effect = server_name.clone();
        let user_id_effect = user_id.clone();
        let api_key_effect = api_key.clone();
        let effect_dispatch = dispatch.clone();

        use_effect_with(
            (api_key_effect, user_id_effect, server_name_effect),
            move |_| {
                // let episodes_clone = episodes.clone();
                // let error_clone = error.clone();

                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let dispatch = effect_dispatch.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_podcasts(&server_name, &api_key, &user_id).await {
                            Ok(fetched_podcasts) => {
                                dispatch.reduce_mut(move |state| {
                                    state.podcast_feed_return = Some(PodcastResponse {
                                        pods: Some(fetched_podcasts),
                                    });
                                });
                            }
                            Err(e) => console::log_1(
                                &format!("Unable to parse Podcasts: {:?}", &e).into(),
                            ),
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
                            // let state_ep = state.clone();
                            // let audio_state_ep = audio_state.clone();
                            let api_key_iter = api_key.clone();
                            let server_name_iter = server_name.clone().unwrap();
                            let history = history_clone.clone();

                            // let id_string = &podcast.PodcastID.to_string();

                            let dispatch = dispatch.clone();
                            let podcast_id_loop = podcast.podcastid.clone();
                            // let podcast_url_clone = podcast.FeedURL.clone();
                            // let podcast_title_clone = podcast.PodcastName.clone();
                            // let podcast_ep_count = podcast.EpisodeCount.clone();
                            // let podcast_artwork_clone = podcast.ArtworkURL.clone();
                            let podcast_description_clone = podcast.description.clone();
                            // let categories: HashMap<String, String> = serde_json::from_str(&podcast_categories_clone).unwrap_or_else(|_| HashMap::new());
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
                                        for _podcast in &podcasts.pods {
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
                                                            dispatch_clone.reduce_mut(|state| {
                                                                state.info_message = Some("Podcast successfully removed".to_string())
                                                            });
                                                        } else {
                                                            dispatch_clone.reduce_mut(|state| {
                                                                state.error_message = Some("Failed to remove podcast".to_string())
                                                            });
                                                        }
                                                    },
                                                    Err(e) => {
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
                            let categories: HashMap<String, String> = serde_json::from_str(&podcast.categories)
                                .unwrap_or_else(|_| HashMap::new());
                            let on_title_click = create_on_title_click(
                                dispatch.clone(),
                                server_name_iter,
                                api_key_iter,
                                &history,
                                podcast.podcastname.clone(),
                                podcast.feedurl.clone(),
                                podcast.description.clone().unwrap_or_else(|| String::from("No Description Provided")),
                                podcast.author.clone().unwrap_or_else(|| String::from("Unknown Author")),
                                podcast.artworkurl.clone().unwrap_or_else(|| String::from("default_artwork_url.png")),
                                podcast.explicit.clone(),
                                podcast.episodecount.clone(),
                                Some(categories),
                                podcast.websiteurl.clone().unwrap_or_else(|| String::from("No Website Provided")),

                                user_id.unwrap(),
                            );

                            let id_string = &podcast.podcastid.clone().to_string();
                            let desc_expanded = desc_state.expanded_descriptions.contains(id_string);
                            #[wasm_bindgen]
                            extern "C" {
                                #[wasm_bindgen(js_namespace = window)]
                                fn toggleDescription(guid: &str, expanded: bool);
                            }
                            let toggle_expanded = {
                                let desc_dispatch = desc_dispatch.clone();
                                let desc_state = desc_state.clone();
                                let episode_guid = podcast.podcastid.clone().to_string();

                                Callback::from(move |_: MouseEvent| {
                                    let guid = episode_guid.clone();
                                    desc_dispatch.reduce_mut(move |state| {
                                        if state.expanded_descriptions.contains(&guid) {
                                            state.expanded_descriptions.remove(&guid); // Collapse the description
                                            toggleDescription(&guid, false); // Call JavaScript function
                                        } else {
                                            state.expanded_descriptions.insert(guid.clone()); // Expand the description
                                            toggleDescription(&guid, true); // Call JavaScript function
                                        }
                                    });
                                })
                            };

                            let description_class = if desc_expanded {
                                "desc-expanded".to_string()
                            } else {
                                "desc-collapsed".to_string()
                            };
                            html! {
                                <div>
                                <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full">
                                        <div class="flex flex-col w-auto object-cover pl-4">
                                            <img
                                                src={podcast.artworkurl.clone()}
                                                onclick={on_title_click.clone()}
                                                alt={format!("Cover for {}", podcast.podcastname.clone())}
                                                class="object-cover align-top-cover w-full item-container img"
                                            />
                                        </div>
                                        <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                                            <p class="item_container-text text-xl font-semibold cursor-pointer" onclick={on_title_click}>
                                                { &podcast.podcastname }
                                            </p>
                                            <hr class="my-2 border-t hidden md:block"/>
                                            {
                                                html! {
                                                    <div class="item-container-text hidden md:block">
                                                        <div class={format!("item_container-text episode-description-container {}", description_class)}>
                                                            <SafeHtml html={podcast_description_clone.unwrap_or_default()} />
                                                        </div>
                                                        <a class="link hover:underline cursor-pointer mt-4" onclick={toggle_expanded}>
                                                            { if desc_expanded { "See Less" } else { "See More" } }
                                                        </a>
                                                    </div>
                                                }
                                            }
                                            <p class="item_container-text">{ format!("Episode Count: {}", &podcast.episodecount) }</p>
                                        </div>
                                        <button class={"item-container-button border selector-button font-bold py-2 px-4 rounded-full self-center mr-8"} style="width: 60px; height: 60px;">
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
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} /> }
            } else {
                html! {}
            }
        }
        <App_drawer />
        </>
    }
}
