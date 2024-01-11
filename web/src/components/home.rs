use yew::{function_component, Html, html};
use yew::prelude::*;
use super::app_drawer::{App_drawer};
use super::gen_components::Search_nav;
use crate::requests::pod_req;
use web_sys::console;
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use std::rc::Rc;
use html2md::parse_html;
use markdown::to_html;
use serde::de::Unexpected::Option;
use crate::components::episodes_layout::SafeHtml;
use crate::components::audio::AudioPlayerProps;
use crate::requests::pod_req::{RecentEps, Episode};

enum AppStateMsg {
    ExpandEpisode(String),
    CollapseEpisode(String),
}

impl Reducer<AppState> for AppStateMsg {
    fn apply(self, mut state: Rc<AppState>) -> Rc<AppState> {
        let state_mut = Rc::make_mut(&mut state);

        match self {
            AppStateMsg::ExpandEpisode(guid) => {
                state_mut.expanded_descriptions.insert(guid);
            },
            AppStateMsg::CollapseEpisode(guid) => {
                state_mut.expanded_descriptions.remove(&guid);
            },
        }

        // Return the Rc itself, not a reference to it
        state
    }
}


#[function_component(Home)]
pub fn home() -> Html {
    let error = use_state(|| None);
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    console::log_1(&format!("User Context in Home: {:?}", &state.user_details).into());

    fn truncate_description(description: &str, max_length: usize) -> (String, bool) {
        // Convert HTML to Markdown
        let markdown = parse_html(description);

        // Check if the Markdown string is longer than the maximum length
        let is_truncated = markdown.len() > max_length;

        // Truncate the Markdown string if it's too long
        let truncated_markdown = if is_truncated {
            markdown.chars().take(max_length).collect::<String>() + "..."
        } else {
            markdown
        };

        // Convert truncated Markdown back to HTML
        let html = to_html(&truncated_markdown);

        (html, is_truncated)
    }
    // Fetch episodes on component mount
    {
        // let episodes = episodes.clone();
        let error = error.clone();
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        console::log_1(&"Test log on home".to_string().into());
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

        console::log_1(&format!("apikey: {:?}", &api_key).into());
        console::log_1(&format!("userid: {:?}", &user_id).into());
        console::log_1(&format!("servername: {:?}", &server_name).into());

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
                        match pod_req::call_get_recent_eps(&server_name, &api_key, &user_id).await {
                            Ok(fetched_episodes) => {
                                dispatch.reduce_mut(move |state| {
                                    state.server_feed_results = Some(RecentEps { episodes: Some(fetched_episodes) });
                                });
                            },
                            Err(e) => error_clone.set(Some(e.to_string())),
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
                if let Some(recent_eps) = state.server_feed_results.clone() {
                    let int_recent_eps = recent_eps.clone();
                    if let Some(episodes) = int_recent_eps.episodes.clone() {
                        episodes.into_iter().map(|episode| {
                            let state_ep = state.clone();
                            let audio_state_ep = audio_state.clone();

                            let id_string = &episode.EpisodeID.to_string();
    
                            let is_expanded = state.expanded_descriptions.contains(id_string);
    
                            let dispatch = dispatch.clone();
    
                            let episode_url_clone = episode.EpisodeURL.clone();
                            let episode_title_clone = episode.EpisodeTitle.clone();
                            let episode_duration_clone = episode.EpisodeDuration.clone();
    
                            let (description, is_truncated) = if is_expanded {
                                (episode.EpisodeDescription.clone(), false)
                            } else {
                                truncate_description(&episode.EpisodeDescription, 300)
                            };
    
                            let toggle_expanded = {
                                let search_dispatch_clone = dispatch.clone();
                                let state_clone = state.clone();
                                let episode_guid = episode.EpisodeID.clone();
    
                                Callback::from(move |_: MouseEvent| {
                                    let guid_clone = episode_guid.to_string().clone();
                                    let search_dispatch_call = search_dispatch_clone.clone();
    
                                    if state_clone.expanded_descriptions.contains(&guid_clone) {
                                        search_dispatch_call.apply(AppStateMsg::CollapseEpisode(guid_clone));
                                    } else {
                                        search_dispatch_call.apply(AppStateMsg::ExpandEpisode(guid_clone));
                                    }
                                })
                            };
    
                            let on_play_click = {
                                let episode_url_for_closure = episode_url_clone.clone();
                                let episode_title_for_closure = episode_title_clone.clone();
                                let episode_duration_for_closure = episode_duration_clone.clone();
                                let dispatch = dispatch.clone();
                                let audio_dispatch = audio_dispatch.clone();
                                let play_state = state_ep.clone();
                                let audio_play_state = audio_state_ep.clone();
    
                                fn parse_duration_to_seconds(duration_convert: &i32) -> f64 {
                                    let dur_string = duration_convert.to_string();
                                    let parts: Vec<&str> = dur_string.split(':').collect();
                                    let parts: Vec<f64> = parts.iter().map(|part| part.parse::<f64>().unwrap_or(0.0)).collect();
    
                                    let seconds = match parts.len() {
                                        3 => parts[0] * 3600.0 + parts[1] * 60.0 + parts[2],
                                        2 => parts[0] * 60.0 + parts[1],
                                        1 => parts[0],
                                        _ => 0.0,
                                    };
    
                                    seconds
                                }
    
                                Callback::from(move |_: MouseEvent| {
                                    web_sys::console::log_1(&"Play Clicked".to_string().into());
                                    let episode_url_for_closure = episode_url_for_closure.clone();
                                    let episode_title_for_closure = episode_title_clone.clone();
                                    let episode_duration_for_closure = episode_duration_clone.clone();
                                    web_sys::console::log_1(&format!("duration: {}", &episode_duration_for_closure).into());
                                    let dispatch = dispatch.clone();
                                    let audio_dispatch = audio_dispatch.clone();
                                    let call_play_state = play_state.clone();
                                
                                    let formatted_duration = parse_duration_to_seconds(&episode_duration_for_closure);
                                    web_sys::console::log_1(&format!("duration sec: {}", &formatted_duration).into());
                                    audio_dispatch.reduce_mut(move |audio_state| {
                                        audio_state.audio_playing = Some(true);
                                        audio_state.currently_playing = Some(AudioPlayerProps {
                                            src: episode_url_for_closure.clone(),
                                            title: episode_title_for_closure.clone(),
                                            duration: episode_duration_for_closure.clone().to_string(),
                                            duration_sec: formatted_duration,
                                        });
                                        audio_state.set_audio_source(episode_url_for_closure.to_string());
                                        if let Some(audio) = &audio_state.audio_element {
                                            let _ = audio.play();
                                        }
                                        audio_state.audio_playing = Some(true);
                                    });
                                })
                            };
    
                            html! {
                                <div>
                                    <div class="item-container flex items-center mb-4 bg-white shadow-md rounded-lg overflow-hidden">
                                        <img src={episode.EpisodeArtwork.clone()} alt={format!("Cover for {}", &episode.EpisodeTitle)} class="w-2/12 object-cover"/>
                                        <div class="flex flex-col p-4 space-y-2 w-9/12">
                                            <p class="item_container-text text-xl font-semibold">{ &episode.EpisodeTitle }</p>
                                            {
                                                html! {
                                                    <div class="item_container-text episode-description-container">
                                                        <div>
                                                            <SafeHtml html={description} />
                                                        </div>
                                                        <button class="item-container-button selector-button w-1/4 hover:bg-blue-700 font-bold py-1 px-2 rounded" onclick={toggle_expanded}>
                                                            { if is_expanded { "See Less" } else { "See More" } }
                                                        </button>
                                                    </div>
                                                }
                                            }
                                            <p class="item-container-text">{ &episode.EpisodePubDate }</p>
                                        </div>
                                        <button class="item-container-button selector-button w-1/12 font-bold py-2 px-4 rounded" onclick={on_play_click}>
                                            <span class="material-icons">{"play_arrow"}</span>
                                        </button>
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                    } else {
                        html! {
                            <div class="empty-episodes-container">
                                <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                <h1>{ "No Recent Episodes Found" }</h1>
                                <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                            </div>
                        }
                    }
                } else {
                    html! {
                        <div class="empty-episodes-container">
                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                            <h1>{ "No Recent Episodes Found" }</h1>
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