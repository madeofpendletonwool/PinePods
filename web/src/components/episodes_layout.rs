use std::rc::Rc;
use yew::{Callback, function_component, Html, html, TargetCast, use_effect, use_effect_with, use_force_update, use_node_ref};
use web_sys::{console, Event, MouseEvent, window};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use crate::components::audio::{AudioPlayerProps, AudioPlayer};
use super::gen_components::Search_nav;
use super::app_drawer::App_drawer;
use crate::requests::pod_req::{call_add_podcast, PodcastValues};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use yew::{Properties};
use crate::components::gen_funcs::{sanitize_html_with_blank_target, truncate_description};

#[derive(Properties, PartialEq)]
pub struct Props {
    pub html: String,
}

#[function_component(SafeHtml)]
pub fn safe_html(props: &Props) -> Html {
    let div = gloo_utils::document().create_element("div").unwrap();
    div.set_inner_html(&props.html.clone());

    Html::VRef(div.into())
}

pub enum AppStateMsg {
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

pub enum UIStateMsg {
    ClearErrorMessage,
    ClearInfoMessage,
}

impl Reducer<UIState> for UIStateMsg {
    fn apply(self, mut state: Rc<UIState>) -> Rc<UIState> {
        let state = Rc::make_mut(&mut state);

        match self {
            UIStateMsg::ClearErrorMessage => {
                state.error_message = None;
            },
            UIStateMsg::ClearInfoMessage => {
                state.info_message = None;
            },
        }

        (*state).clone().into()
    }
}




#[function_component(EpisodeLayout)]
pub fn episode_layout() -> Html {
    // let dispatch = Dispatch::<AppState>::global();
    // // let (state, _dispatch) = use_store::<AppState>();
    // let state: Rc<AppState> = dispatch.get();
    let (state, _dispatch) = use_store::<UIState>();
    let (search_state, _search_dispatch) = use_store::<AppState>();
    let podcast_feed_results = search_state.podcast_feed_results.clone();
    let clicked_podcast_info = search_state.clicked_podcast_info.clone();
    let error_message = state.error_message.clone();
    let info_message = state.info_message.clone();
    let history = BrowserHistory::new();
    let trigger = use_force_update();
    let history_clone = history.clone();
    // let node_ref = use_node_ref();
    let user_id = search_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let api_key = search_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = search_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    // Function to handle link clicks
    let handle_click = Callback::from(move |event: MouseEvent| {
        web_sys::console::log_1(&"click handle".to_string().into());
        if let Some(target) = event.target_dyn_into::<web_sys::HtmlElement>() {
            if let Some(href) = target.get_attribute("href") {
                event.prevent_default();
                if href.starts_with("http") {
                    web_sys::console::log_1(&"running external".to_string().into());
                    // External link, open in a new tab
                    web_sys::window()
                        .unwrap()
                        .open_with_url_and_target(&href, "_blank")
                        .unwrap();
                } else {
                    // Internal link, use Yew Router to navigate
                    history.push(href);
                }
            }
        }
    });

    let node_ref = use_node_ref();

    use_effect_with((), move |_| {
        if let Some(container) = node_ref.cast::<web_sys::HtmlElement>() {
            if let Ok(links) = container.query_selector_all("a") {
                for i in 0..links.length() {
                    if let Some(link) = links.item(i) {
                        let link = link.dyn_into::<web_sys::HtmlElement>().unwrap();
                        let handle_click_clone = handle_click.clone();
                        let listener = gloo_events::EventListener::new(&link, "click", move |event| {
                            handle_click_clone.emit(event.clone().dyn_into::<web_sys::MouseEvent>().unwrap());
                        });
                        listener.forget(); // Prevent listener from being dropped
                    }
                }
            }
        }

        || ()
    });

    {
        let dispatch = _dispatch.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                dispatch.apply(UIStateMsg::ClearErrorMessage);
                dispatch.apply(UIStateMsg::ClearInfoMessage);
            }) as Box<dyn Fn(_)>);

            document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();

            // Return cleanup function
            move || {
                document.remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }


    let on_add_click = {
        let add_dispatch = _dispatch.clone();
        let pod_values = clicked_podcast_info.clone();

        let pod_title_og = pod_values.clone().unwrap().podcast_title.clone();
        let pod_artwork_og = pod_values.clone().unwrap().podcast_artwork.clone();
        let pod_author_og = pod_values.clone().unwrap().podcast_author.clone();
        let categories_og = pod_values.clone().unwrap().podcast_categories.unwrap().clone();
        let pod_description_og = pod_values.clone().unwrap().podcast_description.clone();
        let pod_episode_count_og = pod_values.clone().unwrap().podcast_episode_count.clone();
        let pod_feed_url_og = pod_values.clone().unwrap().podcast_url.clone();
        let pod_website_og = pod_values.clone().unwrap().podcast_link.clone();
        let pod_explicit_og = pod_values.clone().unwrap().podcast_explicit.clone();
        let user_id_og = user_id.unwrap().clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let user_id_clone = user_id.clone();


        Callback::from(move |_: MouseEvent| { // Ensure this is triggered only by a MouseEvent
            let call_dispatch = add_dispatch.clone();
            let pod_title = pod_title_og.clone();
            let pod_artwork = pod_artwork_og.clone();
            let pod_author = pod_author_og.clone();
            let categories = categories_og.clone();
            let pod_description = pod_description_og.clone();
            let pod_episode_count = pod_episode_count_og.clone();
            let pod_feed_url = pod_feed_url_og.clone();
            let pod_website = pod_website_og.clone();
            let pod_explicit = pod_explicit_og.clone();
            let user_id = user_id_og.clone();
            web_sys::console::log_1(&"Add Clicked".to_string().into());
            let podcast_values = PodcastValues {
                pod_title,
                pod_artwork,
                pod_author,
                categories,
                pod_description,
                pod_episode_count,
                pod_feed_url,
                pod_website,
                pod_explicit,
                user_id
            };
            let api_key_call = api_key_clone.clone();
            let server_name_call = server_name_clone.clone();
            let user_id_call = user_id_clone.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let dispatch_wasm = call_dispatch.clone();
                let api_key_wasm = api_key_call.clone().unwrap();
                let user_id_wasm = user_id_call.clone().unwrap();
                let server_name_wasm = server_name_call.clone();
                let pod_values_clone = podcast_values.clone(); // Make sure you clone the podcast values

                match call_add_podcast(&server_name_wasm.unwrap(), &api_key_wasm, user_id_wasm, &pod_values_clone).await {
                    Ok(success) => {
                        if success {
                            console::log_1(&"Podcast successfully added".into());
                            dispatch_wasm.reduce_mut(|state| state.info_message = Option::from("Podcast successfully added".to_string()));
                        } else {
                            console::log_1(&"Failed to add podcast".into());
                            dispatch_wasm.reduce_mut(|state| state.error_message = Option::from("Failed to add podcast".to_string()));
                        }
                    },
                    Err(e) => {
                        console::log_1(&format!("Error adding podcast: {:?}", e).into());
                        dispatch_wasm.reduce_mut(|state| state.error_message = Option::from(format!("Error adding podcast: {:?}", e)));
                    }
                }
            });
        })
    };



    html! {
        <div class="main-container">
            <Search_nav />
            <h1 class="page_header text-2xl font-bold my-4 text-center">{ "Podcast Episode Results" }</h1>
        {
            if let Some(podcast_info) = clicked_podcast_info {
                html! {
                    <div class="item-header">
                        <img src={podcast_info.podcast_artwork.clone()} alt={format!("Cover for {}", &podcast_info.podcast_title)} class="item-header-cover"/>
                        <div class="item-header-info">
                            <h2 class="item-header-title">{ &podcast_info.podcast_title }</h2>

                            <p class="item-header-description">{ &podcast_info.podcast_description }</p>
                            <div class="item-header-info">
                                <p class="header-text">{ format!("Episode Count: {}", &podcast_info.podcast_episode_count) }</p>
                                <p class="header-text">{ format!("Authors: {}", &podcast_info.podcast_author) }</p>
                                <p class="header-text">{ format!("Explicit: {}", &podcast_info.podcast_explicit) }</p>

                                <div>
                                    {
                                        if let Some(categories) = &podcast_info.podcast_categories {
                                            html! {
                                                for categories.values().map(|category_name| {
                                                    html! { <span class="category-box">{ category_name }</span> }
                                                })
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>




                            </div>
                        </div>
                            <button class="item-header-button selector-button font-bold py-2 px-4 rounded" title="Add Podcast" onclick={on_add_click}>
                                <span class="material-icons">{"add"}</span>
                            </button>
                    </div>
                }
            } else {
                html! {}
            }
        }
        {
                if let Some(results) = podcast_feed_results {
                    html! {
                        <div>
                            { for results.episodes.iter().map(|episode| {
                                let dispatch = _dispatch.clone();
                                let search_dispatch = _search_dispatch.clone();
                                let history = history_clone.clone();
                                let search_state_clone = search_state.clone(); // Clone search_state

                                // Clone the variables outside the closure
                                let episode_url_clone = episode.enclosure_url.clone().unwrap_or_default();
                                let episode_title_clone = episode.title.clone().unwrap_or_default();
                                let episode_artwork_clone = episode.artwork.clone().unwrap_or_default();
                                let episode_duration_clone = episode.duration.clone().unwrap_or_default();
                                let episode_id_clone = 40;

                                let is_expanded = search_state.expanded_descriptions.contains(&episode.guid);

                                let sanitized_description = sanitize_html_with_blank_target(&episode.description.clone().unwrap_or_default());

                                let (description, is_truncated) = if is_expanded {
                                    (sanitized_description, false)
                                } else {
                                    truncate_description(sanitized_description, 300)
                                };

                                let toggle_expanded = {
                                    let search_dispatch_clone = search_dispatch.clone();
                                    let episode_guid = episode.guid.clone();

                                    Callback::from(move |_: MouseEvent| {
                                        let guid_clone = episode_guid.clone();
                                        let search_dispatch_call = search_dispatch_clone.clone();

                                        if search_state_clone.expanded_descriptions.contains(&guid_clone) {
                                            search_dispatch_call.apply(AppStateMsg::CollapseEpisode(guid_clone));
                                        } else {
                                            search_dispatch_call.apply(AppStateMsg::ExpandEpisode(guid_clone));
                                        }

                                    })
                                };

                                let on_play_click = {
                                    let episode_url_for_closure = episode_url_clone.clone();
                                    let dispatch = dispatch.clone();

                                    fn parse_duration_to_seconds(duration_convert: &str) -> f64 {
                                        let parts: Vec<&str> = duration_convert.split(':').collect();
                                        let parts: Vec<f64> = parts.iter().map(|part| part.parse::<f64>().unwrap_or(0.0)).collect();

                                        let seconds = match parts.len() {
                                            3 => parts[0] * 3600.0 + parts[1] * 60.0 + parts[2],
                                            2 => parts[0] * 60.0 + parts[1],
                                            1 => parts[0],
                                            _ => 0.0,
                                        };

                                        seconds
                                    }



                                    Callback::from(move |_: MouseEvent| { // Ensure this is triggered only by a MouseEvent
                                        web_sys::console::log_1(&"Play Clicked".to_string().into());
                                        let episode_url_for_closure = episode_url_for_closure.clone();
                                        let episode_title_for_closure = episode_title_clone.clone();
                                        let episode_artwork_for_closure = episode_artwork_clone.clone();
                                        let episode_duration_for_closure = episode_duration_clone.clone();
                                        let episode_id_for_closure = episode_id_clone.clone();
                                        web_sys::console::log_1(&format!("duration: {}", &episode_duration_for_closure).into());
                                        let dispatch = dispatch.clone();
                                        // let duration = episode_duration_for_closure;
                                        let formatted_duration = parse_duration_to_seconds(&episode_duration_for_closure);
                                        web_sys::console::log_1(&format!("duration format: {}", &episode_duration_for_closure).into());
                                        web_sys::console::log_1(&format!("duration sec: {}", &formatted_duration).into());
                                        dispatch.reduce_mut(move |state| {
                                            state.audio_playing = Some(true);
                                            state.currently_playing = Some(AudioPlayerProps {
                                                src: episode_url_for_closure.clone(),
                                                title: episode_title_for_closure.clone(),
                                                artwork_url: episode_artwork_for_closure.clone(),
                                                duration: episode_duration_for_closure.clone(),
                                                episode_id: episode_id_for_closure.clone(),
                                                duration_sec: formatted_duration,
                                            });
                                            state.set_audio_source(episode_url_for_closure.to_string()); // Set the audio source here
                                            // if !state.audio_playing.unwrap_or(false) {
                                            //     state.audio_playing = Some(true);
                                            //     // state.toggle_playback(); // Ensure this only plays if not already playing
                                            // }
                                            if let Some(audio) = &state.audio_element {
                                                let _ = audio.play();
                                            }
                                            state.audio_playing = Some(true);
                                        });
                                    })
                                };
                                let format_release = format!("Released on: {}", &episode.pub_date.clone().unwrap_or_default());
                                html! {
                                    <div class="item-container flex items-center mb-4 bg-white shadow-md rounded-lg overflow-hidden">
                                        <img src={episode.artwork.clone().unwrap_or_default()} alt={format!("Cover for {}", &episode.title.clone().unwrap_or_default())} class="w-2/12 object-cover"/>
                                        <div class="flex flex-col p-4 space-y-2 w-9/12">
                                            <p class="item-container-text text-xl font-semibold">{ &episode.title.clone().unwrap_or_default() }</p>
                                            // <p class="text-gray-600">{ &episode.description.clone().unwrap_or_default() }</p>
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
                                            <p class="item-container-text">{ format_release.clone() }</p>
                                        </div>
                                        <button class="item-container-button selector-button w-1/12 font-bold py-2 px-4 rounded" onclick={on_play_click}>
                                            <span class="material-icons">{"play_arrow"}</span>
                                        </button>


                                    </div>
                                }
                            })}
                        </div>
                    }
                } else {
                    html! {
                        <div class="empty-episodes-container" id="episode-container">
                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                            <h1 class="page-subtitles">{ "No Episodes Found" }</h1>
                            <p class="page-paragraphs">{"This podcast strangely doesn't have any episodes. Try a more mainstream one maybe?"}</p>
                        </div>
                    }
                }
            }
        <App_drawer />
        // Conditional rendering for the error banner
        {
            if state.error_message.as_ref().map_or(false, |msg| !msg.is_empty()) {
                html! { <div class="error-snackbar">{ &state.error_message }</div> }
            } else {
                html! {}
            }
        }
        //     if !state.error_message.is_empty() {
        //         html! { <div class="error-snackbar">{ &state.error_message }</div> }
        //     } else {
        //         html! {}
        //     }
        // }
        //     // Conditional rendering for the info banner
        {
        if state.info_message.as_ref().map_or(false, |msg| !msg.is_empty()) {
                html! { <div class="info-snackbar">{ &state.info_message }</div> }
            } else {
                html! {}
            }
        }
        // {
        //     if !state.info_message.is_empty() {
        //         html! { <div class="info-snackbar">{ &state.info_message }</div> }
        //     } else {
        //         html! {}
        //     }
        // }
        {
            if let Some(audio_props) = &state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} /> }
            } else {
                html! {}
            }
        }
        </div>

    }
}

