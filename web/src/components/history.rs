use yew::{function_component, Html, html};
use yew::prelude::*;
use super::app_drawer::App_drawer;
use super::gen_components::{Search_nav, empty_message, episode_item, on_shownotes_click};
use crate::requests::pod_req::{self, HistoryDataResponse};
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use yew_router::history::BrowserHistory;
use crate::components::audio::AudioPlayer;
use crate::components::gen_funcs::{sanitize_html_with_blank_target, truncate_description};
use crate::components::audio::on_play_click;
use crate::components::episodes_layout::AppStateMsg;
use crate::requests::login_requests::use_check_authentication;
// use crate::components::gen_funcs::check_auth;

#[function_component(PodHistory)]
pub fn history() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let history = BrowserHistory::new();

    // check_auth(effect_dispatch);

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let dropdown_open = use_state(|| false);
    let session_dispatch = _post_dispatch.clone();
    let session_state = post_state.clone();
    let loading = use_state(|| true);

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

    let _toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |_: MouseEvent| {
            web_sys::console::log_1(&format!("Dropdown toggled: {}", !*dropdown_open).into()); // Log for debugging
            dropdown_open.set(!*dropdown_open);
        })
    };


    // Fetch episodes on component mount
    let loading_ep = loading.clone();
    {
        // let episodes = episodes.clone();
        let error = error.clone();
        let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());

        let effect_dispatch = dispatch.clone();

        // fetch_episodes(api_key.flatten(), user_id, server_name, dispatch, error, pod_req::call_get_recent_eps);

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) = (api_key.clone(), user_id.clone(), server_name.clone()) {
                    let dispatch = effect_dispatch.clone();
    
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_user_history(&server_name, &api_key, &user_id).await {
                            Ok(fetched_episodes) => {
                                web_sys::console::log_1(&format!("Fetched episodes: {:?}", fetched_episodes).into()); // Log fetched episodes
                                dispatch.reduce_mut(move |state| {
                                    state.episode_history = Some(HistoryDataResponse { data: fetched_episodes });
                                });
                                loading_ep.set(false);
                                // web_sys::console::log_1(&format!("State after update: {:?}", state).into()); // Log state after update
                            },
                            Err(e) => {
                                web_sys::console::log_1(&format!("Error fetching episodes: {:?}", e).into()); // Log error
                                error_clone.set(Some(e.to_string()));
                                loading_ep.set(false);
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
            
                if *loading { // If loading is true, display the loading animation
                    {
                        html! {
                            <div class="loading-animation">
                                <div class="frame1"></div>
                                <div class="frame2"></div>
                                <div class="frame3"></div>
                                <div class="frame4"></div>
                                <div class="frame5"></div>
                                <div class="frame6"></div>
                            </div>
                        }
                    }
                } else {
                    {                           
                        html! {
                            <div>
                            <h1 class="text-2xl item_container-text font-bold text-center mb-6">{"History"}</h1>
                            </div>
                        }
                    }
                    

                        {
                            

                            if let Some(history_eps) = state.episode_history.clone() {
                                web_sys::console::log_1(&format!("Episode History in state: {:?}", history_eps).into());
                                if history_eps.data.is_empty() {
                                    empty_message(
                                        "No Episode History Found",
                                        "This one is pretty straightforward. You should get listening! Podcasts you listen to will show up here!."
                                    )
                                } else {
                                    
                                    history_eps.data.into_iter().map(|episode| {
                                        let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
                                        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
                                        let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
                                        let history_clone = history.clone();
                                        let id_string = &episode.EpisodeID.to_string();
                
                                        let is_expanded = state.expanded_descriptions.contains(id_string);
                
                                        let dispatch = dispatch.clone();
                
                                        let episode_url_clone = episode.EpisodeURL.clone();
                                        let episode_title_clone = episode.EpisodeTitle.clone();
                                        let episode_artwork_clone = episode.EpisodeArtwork.clone();
                                        let episode_duration_clone = episode.EpisodeDuration.clone();
                                        let episode_id_clone = episode.EpisodeID.clone();
                                        let episode_listened_clone = episode.ListenDuration.clone();

                                        let sanitized_description = sanitize_html_with_blank_target(&episode.EpisodeDescription.clone());

                                        let (description, _is_truncated) = if is_expanded {
                                            (sanitized_description, false)
                                        } else {
                                            truncate_description(sanitized_description, 300)
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

                                        let episode_url_for_closure = episode_url_clone.clone();
                                        let episode_title_for_closure = episode_title_clone.clone();
                                        let episode_artwork_for_closure = episode_artwork_clone.clone();
                                        let episode_duration_for_closure = episode_duration_clone.clone();
                                        let episode_id_for_closure = episode_id_clone.clone();
                                        let listener_duration_for_closure = episode_listened_clone.clone();

                                        let user_id_play = user_id.clone();
                                        let server_name_play = server_name.clone();
                                        let api_key_play = api_key.clone();
                                        let audio_dispatch = audio_dispatch.clone();

                                        let on_play_click = on_play_click(
                                            episode_url_for_closure.clone(),
                                            episode_title_for_closure.clone(),
                                            episode_artwork_for_closure.clone(),
                                            episode_duration_for_closure.clone(),
                                            episode_id_for_closure.clone(),
                                            listener_duration_for_closure.clone(),
                                            api_key_play.unwrap().unwrap(),
                                            user_id_play.unwrap(),
                                            server_name_play.unwrap(),
                                            audio_dispatch.clone(),
                                            audio_state.clone(),
                                            None,
                                        );

                                        let on_shownotes_click = on_shownotes_click(
                                            history_clone.clone(),
                                            dispatch.clone(),
                                            episode_id_for_closure.clone(),
                                        );
            
                                        let format_release = format!("Released on: {}", &episode.EpisodePubDate);
                                        let item = episode_item(
                                            Box::new(episode),
                                            description.clone(),
                                            is_expanded,
                                            &format_release,
                                            on_play_click,
                                            on_shownotes_click,
                                            toggle_expanded,
                                            episode_duration_clone,
                                            episode_listened_clone,
                                            "history",
                                            Callback::from(|_| {}), 
                                            false,
                                        );

                                        item
                                    }).collect::<Html>()
                                }

                            } else {
                                empty_message(
                                    "No Episode History Found",
                                    "This one is pretty straightforward. You should get listening! Podcasts you listen to will show up here!."
                                )
                            }
                        }
                    }
                
            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} /> }
                } else {
                    html! {}
                }
            }
        </div>
        <App_drawer />
        </>
    }
}
