use yew::{function_component, Html, html};
use yew::prelude::*;
use super::app_drawer::App_drawer;
use super::gen_components::{Search_nav, empty_message, episode_item};
use crate::requests::pod_req;
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use crate::components::audio::AudioPlayer;
use crate::components::gen_funcs::{sanitize_html_with_blank_target, truncate_description};
use crate::requests::pod_req::{EpisodeRequest, EpisodeMetadataResponse};
use crate::components::audio::on_play_click;
use crate::components::episodes_layout::AppStateMsg;
use crate::components::gen_funcs::check_auth;

#[function_component(Episode)]
pub fn epsiode() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let effect_dispatch = dispatch.clone();

    check_auth(effect_dispatch);

    let error = use_state(|| None);
    let (post_state, post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let dropdown_open = use_state(|| false);
    let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());


    // Fetch episode on component mount
    {
        // let episodes = episodes.clone();
        let error = error.clone();
        let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());

        let server_name_effect = server_name.clone();
        let user_id_effect = user_id.clone();
        let api_key_effect = api_key.clone();
        let effect_dispatch = dispatch.clone();

        let episode_id = state.selected_episode_id.clone();

        // fetch_episodes(api_key.flatten(), user_id, server_name, dispatch, error, pod_req::call_get_recent_eps);

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) = (api_key.clone(), user_id.clone(), server_name.clone()) {
                    let dispatch = effect_dispatch.clone();
    
                    let episode_request = EpisodeRequest {
                        episode_id: episode_id.clone().unwrap(),
                        user_id: user_id.clone(),
                    };
        
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_episode_metadata(&server_name, api_key, &episode_request).await {
                            Ok(fetched_episode) => {
                                web_sys::console::log_1(&format!("Fetched episode: {:?}", fetched_episode).into()); // Log fetched episode
                                dispatch.reduce_mut(move |state| {
                                    state.fetched_episode = Some(EpisodeMetadataResponse { episode: fetched_episode });
                                });
                                // web_sys::console::log_1(&format!("State after update: {:?}", state).into()); // Log state after update
                            },
                            Err(e) => {
                                web_sys::console::log_1(&format!("Error fetching episode: {:?}", e).into()); // Log error
                                error_clone.set(Some(e.to_string()));
                            },
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
                if let Some(episode) = state.fetched_episode.clone() {
                    web_sys::console::log_1(&format!("Fetched episode: {:?}", episode).into()); // Log fetched episode
    
                    let state_ep = state.clone();
                    let id_string = &episode.episode.EpisodeID.to_string();
    
                    let is_expanded = state.expanded_descriptions.contains(id_string);
    
                    let dispatch = dispatch.clone();
    
                    let episode_url_clone = episode.episode.EpisodeURL.clone();
                    let episode_title_clone = episode.episode.EpisodeTitle.clone();
                    let episode_artwork_clone = episode.episode.EpisodeArtwork.clone();
                    let episode_duration_clone = episode.episode.EpisodeDuration.clone();
                    let episode_id_clone = episode.episode.EpisodeID.clone();
    
                    let sanitized_description = sanitize_html_with_blank_target(&episode.episode.EpisodeDescription.clone());
                    let description = sanitized_description;
    
                    let toggle_expanded = {
                        let search_dispatch_clone = dispatch.clone();
                        let state_clone = state.clone();
                        let episode_guid = episode.episode.EpisodeID.clone();
    
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
    
                    let episode_url_for_closure = episode_url_clone.clone();
                    let episode_title_for_closure = episode_title_clone.clone();
                    let episode_artwork_for_closure = episode_artwork_clone.clone();
                    let episode_duration_for_closure = episode_duration_clone.clone();
                    let episode_id_for_closure = episode_id_clone.clone();
    
                    let user_id_play = user_id.clone();
                    let server_name_play = server_name.clone();
                    let api_key_play = api_key.clone();
                    let audio_dispatch = audio_dispatch.clone();
                    let play_state = state_ep.clone();

                    let on_play_click = on_play_click(
                        episode_url_for_closure.clone(),
                        episode_title_for_closure.clone(),
                        episode_artwork_for_closure.clone(),
                        episode_duration_for_closure.clone(),
                        episode_id_for_closure.clone(),
                        api_key_play.unwrap().unwrap(),
                        user_id_play.unwrap(),
                        server_name_play.unwrap(),
                        audio_dispatch.clone(),
                    );
                    
                    let format_duration = format!("Duration: {} minutes", episode.episode.EpisodeDuration / 60); // Assuming duration is in seconds
                    let format_release = format!("Released on: {}", &episode.episode.EpisodePubDate);
    
                    html! {
                        <div class="episode-layout-container">
                            <div class="episode-top-info">
                                <img src={episode.episode.EpisodeArtwork.clone()} class="episode-artwork" />
                                <div class="episode-details">
                                    <h1 class="podcast-title">{ &episode.episode.PodcastName }</h1>
                                    <h2 class="episode-title">{ &episode.episode.EpisodeTitle }</h2>
                                    <p class="episode-duration">{ format_duration }</p>
                                    <p class="episode-release-date">{ format_release }</p>
                                </div>
                            </div>
                            <div class="episode-action-buttons">
                                <button onclick={on_play_click} class="play-button">{"Play"}</button>
                                <button class="queue-button">{"Queue"}</button>
                                <button class="save-button">{"Save"}</button>
                                <button class="download-button">{"Download"}</button>
                            </div>
                            <hr class="episode-divider" />
                            <div class="episode-description">
                            <p>{ description }</p> 
                            </div>
                        </div>
                    }
                    // item

                } else {
                    empty_message(
                        "Unable to display episode",
                        "Something seems to have gone wrong. A straightup server disconnect maybe? Did you browse here directly? That's not how this app works. It needs the context to browse around. I honestly don't have anything else for you as this shouldn't happen. This is embarrasing."
                    )
                }
            }
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} /> }
            } else {
                html! {}
            }
        }
        </div>
        <App_drawer />
        </>
    }
}