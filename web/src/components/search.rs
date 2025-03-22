use super::app_drawer::App_drawer;
use super::gen_components::{
    empty_message, episode_item, on_shownotes_click, Search_nav, UseScrollToTop,
};
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::AppStateMsg;
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::search_pods::{call_search_database, SearchRequest, SearchResponse};
use async_std::task::sleep;
use gloo_events::EventListener;
use std::time::Duration;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use web_sys::HtmlElement;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew::{function_component, html, use_node_ref, Callback, Html, MouseEvent, Properties};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

#[derive(Properties, Clone, PartialEq)]
pub struct SearchProps {
    pub on_search: Callback<String>,
}

#[function_component(Search)]
pub fn search(_props: &SearchProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let search_dispatch = dispatch.clone();
    let active_modal = use_state(|| None::<i32>);
    let active_modal_clone = active_modal.clone();
    let on_modal_open = Callback::from(move |episode_id: i32| {
        active_modal_clone.set(Some(episode_id));
    });
    let active_modal_clone = active_modal.clone();
    let on_modal_close = Callback::from(move |_| {
        active_modal_clone.set(None);
    });

    // let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let error_message = post_state.error_message.clone();
    let info_message = post_state.info_message.clone();
    let history = BrowserHistory::new();

    let input_ref = use_node_ref();
    let input_ref_clone1 = input_ref.clone();
    let input_ref_clone2 = input_ref.clone();
    let form_ref = NodeRef::default();
    let form_ref_clone1 = form_ref.clone();
    let container_ref = use_node_ref();
    let container_ref_clone1 = container_ref.clone();

    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    // let on_click = Callback::from(move |_| {
    //     if let Some(form) = input_ref_clone1.cast::<HtmlElement>() {
    //         form.class_list().add_1("move-to-top").unwrap();
    //     }
    // });

    let api_key_submit = api_key.clone();
    let user_id_submit = user_id.clone();
    let server_name_submit = server_name.clone();

    let on_submit = Callback::from(move |event: SubmitEvent| {
        event.prevent_default();
        event.prevent_default();
        let container_ref_submit_clone1 = container_ref_clone1.clone();

        if let Some(form) = form_ref_clone1.cast::<HtmlElement>() {
            form.class_list().add_1("move-to-top").unwrap();
        }

        if let Some(form) = input_ref_clone1.cast::<HtmlElement>() {
            form.class_list().add_1("move-to-top").unwrap();
        }

        // Clone the necessary variables
        let server_name_submit = server_name_submit.clone();
        let api_key_submit = api_key_submit.clone();
        let user_id_submit = user_id_submit.clone();
        // let search_results = search_results_clone.clone();
        let mut search_request = None;
        if let Some(input_element) = input_ref_clone2.cast::<HtmlInputElement>() {
            let search_term = input_element.value();
            search_request = Some(SearchRequest {
                search_term,
                user_id: user_id_submit.unwrap(), // replace with the actual user id
            });
        } else {
            // web_sys::console::log_1(&"input_ref_clone2 is not an HtmlInputElement".into());
        }
        let future_dispatch = search_dispatch.clone();
        let future = async move {
            sleep(Duration::from_secs(1)).await;
            if let Some(container) = container_ref_submit_clone1.cast::<HtmlElement>() {
                container.class_list().add_1("shrink-input").unwrap();
            }
            if let Some(search_request) = search_request {
                let dispatch = future_dispatch.clone();
                match call_search_database(
                    &server_name_submit.unwrap(),
                    &api_key_submit.flatten(),
                    &search_request,
                )
                .await
                {
                    Ok(results) => {
                        dispatch.reduce_mut(move |state| {
                            state.search_episodes = Some(SearchResponse { data: results });
                        });
                        // Update the search results state
                        // search_results.set(results);
                    }
                    Err(e) => {
                        // Handle the error
                        web_sys::console::log_1(
                            &format!("Failed to search database: {:?}", e).into(),
                        ); // Log for debugging
                    }
                }
            }
        };
        spawn_local(future);
    });

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
        <div class="search-page-container">
            <Search_nav />
            <UseScrollToTop />
            <div class="search-container" ref={container_ref.clone()}>
                <form class="search-page-input" onsubmit={on_submit} ref={form_ref.clone()}>
                    <label for="search" class="mb-2 text-sm font-medium text-gray-900 sr-only dark:text-white">{ "Search" }</label>
                    <div class="relative">
                        <div class="absolute inset-y-0 start-0 flex items-center ps-3 pointer-events-none">
                            <svg class="w-4 h-4 text-gray-500 dark:text-gray-400" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 20 20">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m19 19-4-4m0-7A7 7 0 1 1 1 8a7 7 0 0 1 14 0Z"/>
                            </svg>
                        </div>
                        <input type="search" id="search" class="search-bar-input block w-full p-4 ps-10 text-sm border rounded-lg" placeholder="Search for a podcast, episode, or description" ref={input_ref.clone()}/>
                        <button class="search-page-button absolute end-2.5 bottom-2.5 focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-4 py-2">{ "Search" }</button>
                    </div>
                </form>
            </div>
            {
                if let Some(search_eps) = state.search_episodes.clone() {
                    let int_search_eps = search_eps.clone();
                    let episodes = int_search_eps.data;
                    if episodes.is_empty() {
                                // Render "No Recent Episodes Found" if episodes list is empty
                                empty_message(
                                    "No Search Results Found",
                                    "Perhaps try again, but search for something slightly different :/"
                                )
                            } else {
                                episodes.into_iter().map(|episode| {
                                    let id_string = &episode.episodeid.to_string();

                                    let is_expanded = state.expanded_descriptions.contains(id_string);

                                    let dispatch = dispatch.clone();

                                    let episode_url_clone = episode.episodeurl.clone();
                                    let episode_title_clone = episode.episodetitle.clone();
                                    let episode_description_clone = episode.episodedescription.clone();
                                    let episode_artwork_clone = episode.episodeartwork.clone();
                                    let episode_duration_clone = episode.episodeduration.clone();
                                    let episode_id_clone = episode.episodeid.clone();
                                    let episode_is_youtube = Some(episode.is_youtube.clone());
                                    let episode_listened_clone = episode.listenduration.clone();
                                    let history_clone = history.clone();
                                    let sanitized_description = sanitize_html_with_blank_target(&episode.episodedescription.clone());
                                    let is_current_episode = audio_state
                                        .currently_playing
                                        .as_ref()
                                        .map_or(false, |current| current.episode_id == episode.episodeid);
                                    let is_playing = audio_state.audio_playing.unwrap_or(false);

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
                                    let episode_description_for_closure = episode_description_clone.clone();
                                    let episode_artwork_for_closure = episode_artwork_clone.clone();
                                    let episode_duration_for_closure = episode_duration_clone.clone();
                                    let episode_id_for_closure = episode_id_clone.clone();
                                    let listener_duration_for_closure = episode_listened_clone.clone();

                                    let user_id_play = user_id.clone();
                                    let server_name_play = server_name.clone();
                                    let api_key_play = api_key.clone();
                                    let audio_dispatch = audio_dispatch.clone();

                                    let date_format = match_date_format(state.date_format.as_deref());
                                    let datetime = parse_date(&episode.episodepubdate, &state.user_tz);
                                    let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));

                                    let on_play_pause = on_play_pause(
                                        episode_url_for_closure.clone(),
                                        episode_title_for_closure.clone(),
                                        episode_description_for_closure.clone(),
                                        format_release.clone(),
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
                                        episode_is_youtube,
                                    );

                                    let on_shownotes_click = on_shownotes_click(
                                        history_clone.clone(),
                                        dispatch.clone(),
                                        Some(episode_id_for_closure.clone()),
                                        Some(String::from("search")),
                                        Some(String::from("search")),
                                        Some(String::from("search")),
                                        true,
                                        None,
                                        episode_is_youtube,
                                    );

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
                                        on_play_pause,
                                        on_shownotes_click,
                                        toggle_expanded,
                                        episode_duration_clone,
                                        episode_listened_clone,
                                        "search",
                                        Callback::from(|_| {}),
                                        false,
                                        episode_url_for_ep_item,
                                        is_completed,
                                        *active_modal == episode_id_clone,
                                        on_modal_open.clone(),
                                        on_modal_close.clone(),
                                        (*container_height).clone(),
                                        is_current_episode,
                                        is_playing,
                                    );

                                    item
                                }).collect::<Html>()
                            }
                    // } else {
                    //     empty_message(
                    //         "No Recent Episodes Found",
                    //         "You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."
                    //     )
                    // }
                } else {
                    html! {}
                }
            }
            <App_drawer />
            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
                } else {
                    html! {}
                }
            }
        </div>
        </>
    }
}
