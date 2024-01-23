use yew::{function_component, Html, html};
use yew::prelude::*;
use super::app_drawer::{App_drawer};
use super::gen_components::{Search_nav, ContextButton};
use crate::requests::pod_req;
use web_sys::console;
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use crate::components::audio::{AudioPlayerProps, AudioPlayer};
use std::rc::Rc;
use crate::components::episodes_layout::SafeHtml;
use crate::components::gen_funcs::{sanitize_html_with_blank_target, truncate_description};
use crate::requests::login_requests::use_check_authentication;
use crate::requests::pod_req::{RecentEps};


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
    let (state, dispatch) = use_store::<AppState>();

    let effect_dispatch = dispatch.clone();

    use_effect_with(
        (),
        move |_| {
            let effect_dispatch_clone = effect_dispatch.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let window = web_sys::window().expect("no global `window` exists");
                let location = window.location();
                let current_route = location.href().expect("should be able to get href");
                use_check_authentication(effect_dispatch_clone, &current_route);
            });

            || ()
        }
    );

    let error = use_state(|| None);
    let (post_state, post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();

    let dropdown_open = use_state(|| false);

    let toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |_: MouseEvent| {
            web_sys::console::log_1(&format!("Dropdown toggled: {}", !*dropdown_open).into()); // Log for debugging
            dropdown_open.set(!*dropdown_open);
        })
    };


    // Fetch episodes on component mount
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

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
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
                    if let Some(episodes) = int_recent_eps.episodes {
                        if episodes.is_empty() {
                            // Render "No Recent Episodes Found" if episodes list is empty
                            html! {
                                <div class="empty-episodes-container">
                                    <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                    <h1>{ "No Recent Episodes Found" }</h1>
                                    <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                                </div>
                            }
                        } else {
                        episodes.into_iter().map(|episode| {
                            let state_ep = state.clone();
                            let id_string = &episode.EpisodeID.to_string();
    
                            let is_expanded = state.expanded_descriptions.contains(id_string);
    
                            let dispatch = dispatch.clone();
    
                            let episode_url_clone = episode.EpisodeURL.clone();
                            let episode_title_clone = episode.EpisodeTitle.clone();
                            let episode_duration_clone = episode.EpisodeDuration.clone();

                            let sanitized_description = sanitize_html_with_blank_target(&episode.EpisodeDescription.clone());

                            let (description, is_truncated) = if is_expanded {
                                (sanitized_description, false)
                            } else {
                                truncate_description(sanitized_description, 300)
                            };
                            // let (description, _is_truncated) = if is_expanded {
                            //     (episode.EpisodeDescription.clone(), false)
                            // } else {
                            //     truncate_description(&episode.EpisodeDescription, 300)
                            // };
    
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
                                // let episode_title_for_closure = episode_title_clone.clone();
                                // let episode_duration_for_closure = episode_duration_clone.clone();
                                let audio_dispatch = audio_dispatch.clone();
                                let play_state = state_ep.clone();
                                // let audio_play_state = audio_state_ep.clone();
    
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
                                    let episode_url_for_closure = episode_url_for_closure.clone();
                                    let episode_title_for_closure = episode_title_clone.clone();
                                    let episode_duration_for_closure = episode_duration_clone.clone();
                                    web_sys::console::log_1(&format!("duration: {}", &episode_duration_for_closure).into());
                                    let audio_dispatch = audio_dispatch.clone();
                                
                                    let formatted_duration = parse_duration_to_seconds(&episode_duration_for_closure);
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
                            let format_release = format!("Released on: {}", &episode.EpisodePubDate);
html! {
    <div>
        <div class="item-container border-solid border flex items-center mb-4 shadow-md rounded-lg h-full">
            <img 
                src={episode.EpisodeArtwork.clone()} 
                alt={format!("Cover for {}", &episode.EpisodeTitle)} 
                class="w-2/12 md:w-4/12 object-cover pl-4"
            />
    
            <div class="flex flex-col p-4 space-y-2 flex-grow md:w-5/12">
                <p class="item_container-text text-xl font-semibold">{ &episode.EpisodeTitle }</p>
                <hr class="my-2 border-t hidden md:block"/>
                {
                    html! {
                        <div class="item_container-text hidden md:block">
                            <div class="item_container-text episode-description-container">
                                <SafeHtml html={description} />
                            </div>
                            <a class="link hover:underline cursor-pointer mt-4" onclick={toggle_expanded}>
                                { if is_expanded { "See Less" } else { "See More" } }
                            </a>
                        </div>
                    }
                }
                <span class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2 border" style="flex-grow: 0; flex-shrink: 0; width: auto;">
                    <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                        <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                    </svg>
                    { &format_release }
                </span>
            </div>
            <div class="flex flex-col md:flex-row space-y-6 md:space-y-0 md:space-x-6 items-center h-full w-2/12 md:w-2/12 px-2 md:px-4">
                <button
                    class="item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full w-10 h-10 flex items-center justify-center"
                    onclick={on_play_click}
                >
                    <span class="material-icons">{"play_arrow"}</span>
                </button>
                <ContextButton />
            </div>
        </div>
    </div>
}
                        }).collect::<Html>()
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
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} duration={audio_props.duration.clone()} duration_sec={audio_props.duration_sec.clone()} /> }
            } else {
                html! {}
            }
        }
        </div>
        <App_drawer />
        </>
    }
}