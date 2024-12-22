use super::app_drawer::App_drawer;
use super::gen_components::_EpisodeModalProps::format_release;
use super::gen_components::{
    empty_message, on_shownotes_click, virtual_episode_item, Search_nav, UseScrollToTop,
};
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::pod_req;
use crate::requests::pod_req::Episode as EpisodeData;
use crate::requests::pod_req::RecentEps;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;
// use crate::components::gen_funcs::check_auth;
use crate::components::episodes_layout::UIStateMsg;
use crate::requests::login_requests::use_check_authentication;
use gloo::events::EventListener;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::window;
use web_sys::{Element, HtmlElement};

use wasm_bindgen::prelude::*;

#[function_component(Home)]
pub fn home() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let effect_dispatch = dispatch.clone();

    let session_dispatch = effect_dispatch.clone();
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

    let error = use_state(|| None);
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();
    let loading = use_state(|| true);

    {
        let ui_dispatch = audio_dispatch.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                ui_dispatch.apply(UIStateMsg::ClearErrorMessage);
                ui_dispatch.apply(UIStateMsg::ClearInfoMessage);
            }) as Box<dyn Fn(_)>);

            document
                .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                .unwrap();

            // Return cleanup function
            move || {
                document
                    .remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                    .unwrap();
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }

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
                        match pod_req::call_get_recent_eps(&server_name, &api_key, &user_id).await {
                            Ok(fetched_episodes) => {
                                let completed_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let saved_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.saved)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let queued_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.queued)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                let downloaded_episode_ids: Vec<i32> = fetched_episodes
                                    .iter()
                                    .filter(|ep| ep.downloaded)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                dispatch.reduce_mut(move |state| {
                                    state.server_feed_results = Some(RecentEps {
                                        episodes: Some(fetched_episodes),
                                    });
                                    state.completed_episodes = Some(completed_episode_ids);
                                    state.saved_episode_ids = Some(saved_episode_ids);
                                    state.queued_episode_ids = Some(queued_episode_ids);
                                    state.downloaded_episode_ids = Some(downloaded_episode_ids);
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

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                if *loading { // If loading is true, display the loading animation
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
                } else {
                    if let Some(recent_eps) = state.server_feed_results.clone() {
                        let int_recent_eps = recent_eps.clone();
                        if let Some(episodes) = int_recent_eps.episodes {

                            if episodes.is_empty() {
                                // Render "No Recent Episodes Found" if episodes list is empty
                                empty_message(
                                    "No Recent Episodes Found",
                                    "You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."
                                )
                            } else {
                                html! {
                                    <VirtualList
                                        episodes={episodes}
                                        page_type="home"
                                    />
                                }
                            }
                        } else {
                            empty_message(
                                "No Recent Episodes Found",
                                "You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."
                            )
                        }
                    } else {
                        empty_message(
                            "No Recent Episodes Found",
                            "You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."
                        )
                    }
                }
            }
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} /> }
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

#[derive(Properties, PartialEq)]
pub struct VirtualListProps {
    pub episodes: Vec<EpisodeData>,
    pub page_type: String,
}

#[function_component(VirtualList)]
pub fn virtual_list(props: &VirtualListProps) -> Html {
    let scroll_pos = use_state(|| 0.0);
    let container_ref = use_node_ref();
    let container_height = use_state(|| 0.0);
    let item_height = use_state(|| 234.0); // Default item height
    let force_update = use_state(|| 0);

    // Effect to set initial container height, item height, and listen for window resize
    {
        let container_height = container_height.clone();
        let item_height = item_height.clone();
        let force_update = force_update.clone();

        use_effect_with((), move |_| {
            let window = window().expect("no global `window` exists");
            let window_clone = window.clone();

            let update_sizes = Callback::from(move |_| {
                let height = window_clone.inner_height().unwrap().as_f64().unwrap();
                container_height.set(height - 100.0);

                let width = window_clone.inner_width().unwrap().as_f64().unwrap();
                // Add 16px (mb-4) to each height value for the virtual list calculations
                let new_item_height = if width <= 530.0 {
                    122.0 + 16.0 // Base height + margin
                } else if width <= 768.0 {
                    162.0 + 16.0 // Base height + margin
                } else {
                    221.0 + 16.0 // Base height + margin
                };

                item_height.set(new_item_height);
                force_update.set(*force_update + 1);
            });

            update_sizes.emit(());

            let listener = EventListener::new(&window, "resize", move |_| {
                update_sizes.emit(());
            });

            move || drop(listener)
        });
    }

    // Effect for scroll handling remains the same
    {
        let scroll_pos = scroll_pos.clone();
        let container_ref = container_ref.clone();
        use_effect_with(container_ref.clone(), move |container_ref| {
            let container = container_ref.cast::<HtmlElement>().unwrap();
            let listener = EventListener::new(&container, "scroll", move |event| {
                let target = event.target().unwrap().unchecked_into::<Element>();
                scroll_pos.set(target.scroll_top() as f64);
            });
            move || drop(listener)
        });
    }

    let start_index = (*scroll_pos / *item_height).floor() as usize;
    let visible_count = ((*container_height / *item_height).ceil() as usize) + 1;
    let end_index = (start_index + visible_count).min(props.episodes.len());

    let visible_episodes = (start_index..end_index)
        .map(|index| {
            let episode = props.episodes[index].clone();
            html! {
                <Episode
                    key={format!("{}-{}", episode.episodeid, *force_update)}
                    episode={episode.clone()}
                    page_type={props.page_type.clone()}
                />
            }
        })
        .collect::<Html>();

    let total_height = props.episodes.len() as f64 * *item_height;
    let offset_y = start_index as f64 * *item_height;

    html! {
        <div
            ref={container_ref}
            class="virtual-list-container flex-grow overflow-y-auto"
            style="height: calc(100vh - 100px);" // Subtract height of header/nav
        >
            <div style={format!("height: {}px; position: relative;", total_height)}>
                <div style={format!("position: absolute; top: {}px; left: 0; right: 0;", offset_y)}>
                    { visible_episodes }
                </div>
            </div>
        </div>
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn toggleDescription(guid: &str, expanded: bool);
}
#[derive(Properties, PartialEq, Clone)]
pub struct EpisodeProps {
    pub episode: EpisodeData,
    pub page_type: String, // New prop to determine the context (e.g., "home", "saved")
}

#[function_component(Episode)]
pub fn episode(props: &EpisodeProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let id_string = &props.episode.episodeid.to_string();
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let show_modal = use_state(|| false);
    let show_clonedal = show_modal.clone();
    let show_clonedal2 = show_modal.clone();
    let on_modal_open = Callback::from(move |_: MouseEvent| show_clonedal.set(true));
    let container_height = use_state(|| "221px".to_string()); // Add this state

    let on_modal_close = Callback::from(move |_: MouseEvent| show_clonedal2.set(false));

    let desc_expanded = desc_state.expanded_descriptions.contains(id_string);

    let toggle_expanded = {
        let desc_dispatch = desc_dispatch.clone();
        let episode_guid = props.episode.episodeid.clone().to_string();

        Callback::from(move |_: MouseEvent| {
            let guid = episode_guid.clone();
            desc_dispatch.reduce_mut(move |state| {
                if state.expanded_descriptions.contains(&guid) {
                    state.expanded_descriptions.remove(&guid);
                    toggleDescription(&guid, false);
                } else {
                    state.expanded_descriptions.insert(guid.clone());
                    toggleDescription(&guid, true);
                }
            });
        })
    };

    let is_current_episode = audio_state
        .currently_playing
        .as_ref()
        .map_or(false, |current| {
            current.episode_id == props.episode.episodeid
        });

    let is_playing = audio_state.audio_playing.unwrap_or(false);

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

    let date_format = match_date_format(state.date_format.as_deref());
    let datetime = parse_date(&props.episode.episodepubdate, &state.user_tz);
    let formatted_date = format!(
        "{}",
        format_datetime(&datetime, &state.hour_preference, date_format)
    );

    let on_play_pause = on_play_pause(
        props.episode.episodeurl.clone(),
        props.episode.episodetitle.clone(),
        props.episode.episodedescription.clone(),
        formatted_date.clone(),
        props.episode.episodeartwork.clone(),
        props.episode.episodeduration.clone(),
        props.episode.episodeid.clone(),
        props.episode.listenduration.clone(),
        api_key.unwrap().unwrap(),
        user_id.unwrap(),
        server_name.unwrap(),
        audio_dispatch.clone(),
        audio_state.clone(),
        None,
    );

    let on_shownotes_click = on_shownotes_click(
        history_clone.clone(),
        dispatch.clone(),
        Some(props.episode.episodeid.clone()),
        Some(props.page_type.clone()),
        Some(props.page_type.clone()),
        Some(props.page_type.clone()),
        true,
        None,
    );

    let is_completed = state
        .completed_episodes
        .as_ref()
        .unwrap_or(&vec![])
        .contains(&props.episode.episodeid);

    let item = virtual_episode_item(
        Box::new(props.episode.clone()),
        sanitize_html_with_blank_target(&props.episode.episodedescription),
        desc_expanded,
        &formatted_date,
        on_play_pause,
        on_shownotes_click,
        toggle_expanded,
        props.episode.episodeduration,
        props.episode.listenduration,
        &props.page_type,
        Callback::from(|_| {}),
        false,
        props.episode.episodeurl.clone(),
        is_completed,
        *show_modal,
        on_modal_open.clone(),
        on_modal_close.clone(),
        (*container_height).clone(),
        is_current_episode,
        is_playing,
    );

    item
}
