use yew::{function_component, Html, html};
use yew::prelude::*;
use super::app_drawer::App_drawer;
use super::gen_components::{Search_nav, empty_message, episode_item};
use crate::requests::pod_req;
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use crate::components::audio::AudioPlayer;
use crate::components::gen_funcs::{sanitize_html_with_blank_target, truncate_description};
use crate::requests::pod_req::SavedEpisodesResponse;
use crate::components::audio::on_play_click;
use crate::components::episodes_layout::AppStateMsg;
use crate::components::gen_funcs::check_auth;
use crate::components::episodes_layout::UIStateMsg;
use wasm_bindgen::closure::Closure;
use web_sys::window;
use wasm_bindgen::JsCast;

#[function_component(Saved)]
pub fn saved() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let effect_dispatch = dispatch.clone();

    check_auth(effect_dispatch);

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();
    let dropdown_open = use_state(|| false);

    let _toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |_: MouseEvent| {
            web_sys::console::log_1(&format!("Dropdown toggled: {}", !*dropdown_open).into()); // Log for debugging
            dropdown_open.set(!*dropdown_open);
        })
    };

    {
        let ui_dispatch = audio_dispatch.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                ui_dispatch.apply(UIStateMsg::ClearErrorMessage);
                ui_dispatch.apply(UIStateMsg::ClearInfoMessage);
            }) as Box<dyn Fn(_)>);

            document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();

            // Return cleanup function
            move || {
                document.remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }


    // Fetch episodes on component mount
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
                        match pod_req::call_get_saved_episodes(&server_name, &api_key, &user_id).await {
                            Ok(fetched_episodes) => {
                                web_sys::console::log_1(&format!("Fetched episodes: {:?}", fetched_episodes).into()); // Log fetched episodes
                                dispatch.reduce_mut(move |state| {
                                    state.saved_episodes = Some(SavedEpisodesResponse { episodes: fetched_episodes });
                                });
                                // web_sys::console::log_1(&format!("State after update: {:?}", state).into()); // Log state after update
                            },
                            Err(e) => {
                                web_sys::console::log_1(&format!("Error fetching episodes: {:?}", e).into()); // Log error
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
                if let Some(saved_eps) = state.saved_episodes.clone() {
                    web_sys::console::log_1(&format!("Saved episodes in state: {:?}", saved_eps).into()); // Log queued episodes in state
                    if saved_eps.episodes.is_empty() {
                        // Render "No Queued Episodes Found" if episodes list is empty
                        empty_message(
                            "No Saved Episodes Found",
                            "You can save episodes by clicking the context button on each episode and clicking 'Save Episode'. Doing this will save episodes here for easy access when you want to return to them."
                        )
                    } else {
                        saved_eps.episodes.into_iter().map(|episode| {
                            let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
                            let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
                            let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
                    
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
                            );
 
                            let format_release = format!("Released on: {}", &episode.EpisodePubDate);
                            let item = episode_item(
                                Box::new(episode),
                                description.clone(),
                                is_expanded,
                                &format_release,
                                on_play_click,
                                toggle_expanded,
                                episode_duration_clone,
                                episode_listened_clone,
                                "saved",
                            );

                            item
                        }).collect::<Html>()
                        }

                } else {
                    empty_message(
                        "No Saved Episodes Found",
                        "You can save episodes by clicking the context button on each episode and clicking 'Save Episode'. Doing this will save episodes here for easy access when you want to return to them."
                    )
                }
            }
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} /> }
            } else {
                html! {}
            }
        }
        // Conditional rendering for the error banner
        if let Some(error) = error_message {
            <div class="error-snackbar">{ error }</div>
        }
        if let Some(info) = info_message {
            <div class="info-snackbar">{ info }</div>
        }
        </div>
        <App_drawer />
        </>
    }
}
