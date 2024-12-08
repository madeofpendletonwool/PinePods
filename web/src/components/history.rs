use super::app_drawer::App_drawer;
use super::gen_components::{
    empty_message, episode_item, on_shownotes_click, Search_nav, UseScrollToTop,
};
use crate::components::audio::on_play_click;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::AppStateMsg;
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::login_requests::use_check_authentication;
use crate::requests::pod_req::{self, HistoryDataResponse};
use gloo_events::EventListener;
use web_sys::window;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;
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
    let active_modal = use_state(|| None::<i32>);
    let show_modal = use_state(|| false);
    let active_clonedal = active_modal.clone();
    let active_modal_clone = active_modal.clone();
    let on_modal_open = Callback::from(move |episode_id: i32| {
        active_modal_clone.set(Some(episode_id));
    });
    let active_modal_clone = active_modal.clone();
    let on_modal_close = Callback::from(move |_| {
        active_modal_clone.set(None);
    });

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

    let _toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |_: MouseEvent| {
            dropdown_open.set(!*dropdown_open);
        })
    };

    // Fetch episodes on component mount
    let loading_ep = loading.clone();
    {
        // let episodes = episodes.clone();
        let error = error.clone();
        let api_key = post_state
            .auth_details
            .as_ref()
            .map(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = post_state
            .auth_details
            .as_ref()
            .map(|ud| ud.server_name.clone());

        let effect_dispatch = dispatch.clone();

        // fetch_episodes(api_key.flatten(), user_id, server_name, dispatch, error, pod_req::call_get_recent_eps);

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let dispatch = effect_dispatch.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_user_history(&server_name, &api_key, &user_id).await
                        {
                            Ok(fetched_episodes) => {
                                let completed_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                dispatch.reduce_mut(move |state| {
                                    state.episode_history = Some(HistoryDataResponse {
                                        data: fetched_episodes,
                                    });
                                    state.completed_episodes = Some(completed_episode_ids);
                                });
                                loading_ep.set(false);
                            }
                            Err(e) => {
                                error_clone.set(Some(e.to_string()));
                                loading_ep.set(false);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }
    let container_height = use_state(|| "221px".to_string()); // Add this state

    {
        let container_height = container_height.clone();
        use_effect_with((), move |_| {
            let update_height = {
                let container_height = container_height.clone();
                Callback::from(move |_| {
                    if let Some(window) = window() {
                        if let Ok(width) = window.inner_width() {
                            if let Some(width) = width.as_f64() {
                                let new_height = if width <= 530.0 {
                                    "122px"
                                } else if width <= 768.0 {
                                    "162px"
                                } else {
                                    "221px"
                                };
                                container_height.set(new_height.to_string());
                            }
                        }
                    }
                })
            };

            // Set initial height
            update_height.emit(());

            // Add resize listener
            let listener = EventListener::new(&window().unwrap(), "resize", move |_| {
                update_height.emit(());
            });

            move || drop(listener)
        });
    }

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
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
                                        let id_string = &episode.episodeid.to_string();

                                        let is_expanded = state.expanded_descriptions.contains(id_string);

                                        let dispatch = dispatch.clone();

                                        let episode_url_clone = episode.episodeurl.clone();
                                        let episode_title_clone = episode.episodetitle.clone();
                                        let episode_artwork_clone = episode.episodeartwork.clone();
                                        let episode_duration_clone = episode.episodeduration.clone();
                                        let episode_id_clone = episode.episodeid.clone();
                                        let episode_listened_clone = episode.listenduration.clone();
                                        let sanitized_description = sanitize_html_with_blank_target(&episode.episodedescription.clone());

                                        let toggle_expanded = {
                                            let search_dispatch_clone = dispatch.clone();
                                            let state_clone = state.clone();
                                            let episode_guid = episode.episodeid.clone();

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
                                            Some(episode_id_for_closure.clone()),
                                            Some(String::from("history")),
                                            Some(String::from("history")),
                                            Some(String::from("history")),
                                            true,
                                            None,
                                        );

                                        let date_format = match_date_format(state.date_format.as_deref());
                                        let datetime = parse_date(&episode.episodepubdate, &state.user_tz);
                                        let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));
                                        let episode_url_for_ep_item = episode_url_clone.clone();
                                        let check_episode_id = &episode.episodeid.clone();
                                        let is_completed = state
                                            .completed_episodes
                                            .as_ref()
                                            .unwrap_or(&vec![])
                                            .contains(&check_episode_id);
                                        let episode_id_clone = Some(episode.episodeid).clone();
                                        let item = episode_item(
                                            Box::new(episode),
                                            sanitized_description,
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
                                            episode_url_for_ep_item,
                                            is_completed,
                                            *active_modal == episode_id_clone,
                                            on_modal_open.clone(),
                                            on_modal_close.clone(),
                                            (*container_height).clone(),
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
                    html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} /> }
                } else {
                    html! {}
                }
            }
        </div>
        <App_drawer />
        </>
    }
}
