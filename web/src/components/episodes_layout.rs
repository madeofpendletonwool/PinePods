use std::io::BufRead;
use std::rc::Rc;
use yew::{Callback, function_component, Html, html, NodeRef, TargetCast, use_effect_with, use_force_update, use_node_ref};
use web_sys::MouseEvent;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use crate::components::audio::{AudioPlayerProps, AudioPlayer};
use crate::components::audio::_AudioPlayerProps::duration;
use super::gen_components::Search_nav;
use super::app_drawer::App_drawer;
use html2md::parse_html;
use markdown::to_html;
use wasm_bindgen::JsCast;
use yew::{Properties};

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





#[function_component(EpisodeLayout)]
pub fn episode_layout() -> Html {
    // let dispatch = Dispatch::<AppState>::global();
    // // let (state, _dispatch) = use_store::<AppState>();
    // let state: Rc<AppState> = dispatch.get();
    let (state, _dispatch) = use_store::<UIState>();
    let (search_state, _search_dispatch) = use_store::<AppState>();
    let podcast_feed_results = search_state.podcast_feed_results.clone();
    let clicked_podcast_info = search_state.clicked_podcast_info.clone();
    let history = BrowserHistory::new();
    let trigger = use_force_update();
    let history_clone = history.clone();
    // let node_ref = use_node_ref();

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



    html! {
        <div>
            <Search_nav />
            <h1 class="text-2xl font-bold my-4 center-text">{ "Podcast Episode Results" }</h1>
        {
            if let Some(podcast_info) = clicked_podcast_info {
                html! {
                    <div class="podcast-header">
                        <img src={podcast_info.podcast_artwork.clone()} alt={format!("Cover for {}", &podcast_info.podcast_title)} class="podcast-cover"/>
                        <div class="podcast-info">
                            <h2 class="podcast-title">{ &podcast_info.podcast_title }</h2>

                            <p class="podcast-description">{ &podcast_info.podcast_description }</p>
                            <div class="podcast-info">
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
                                let episode_duration_clone = episode.duration.clone().unwrap_or_default();

                                let is_expanded = search_state.expanded_descriptions.contains(&episode.guid);

                                let (description, is_truncated) = if is_expanded {
                                    (episode.description.clone().unwrap_or_default(), false)
                                } else {
                                    truncate_description(&episode.description.clone().unwrap_or_default(), 300)
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
                                    let episode_title_for_closure = episode_title_clone.clone();
                                    let episode_duration_for_closure = episode_duration_clone.clone();
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
                                        let episode_duration_for_closure = episode_duration_clone.clone();
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
                                                duration: episode_duration_for_closure.clone(),
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

                                html! {
                                    <div class="flex items-center mb-4 bg-white shadow-md rounded-lg overflow-hidden">
                                        <img src={episode.artwork.clone().unwrap_or_default()} alt={format!("Cover for {}", &episode.title.clone().unwrap_or_default())} class="w-1/4 object-cover"/>
                                        <div class="flex flex-col p-4 space-y-2 w-7/12">
                                            <p class="text-xl font-semibold">{ &episode.title.clone().unwrap_or_default() }</p>
                                            // <p class="text-gray-600">{ &episode.description.clone().unwrap_or_default() }</p>
                                            {
                                            html! {
                                                <div class="episode-description-container">
                                                    <div>
                                                        <SafeHtml html={description} />
                                                    </div>
                                                    <button class="toggle-description-button" onclick={toggle_expanded}>
                                                        { if is_expanded { "See Less" } else { "See More" } }
                                                    </button>
                                                </div>
                                            }
                                                }
                                            <p class="text-gray-500">{ &episode.pub_date.clone().unwrap_or_default() }</p>
                                        </div>
                                        <button class="play-button" onclick={on_play_click}>{"Play"}</button>

                                    </div>
                                }
                            })}
                        </div>
                    }
                } else {
                    html! {
                        <div class="empty-episodes-container" id="episode-container">
                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                            <h1>{ "No Episodes Found" }</h1>
                            <p>{"This podcast strangely doesn't have any episodes. Try a more mainstream one maybe?"}</p>
                        </div>
                    }
                }
            }
        <App_drawer />
        {
            if let Some(audio_props) = &state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} duration={audio_props.duration.clone()} duration_sec={audio_props.duration_sec.clone()} /> }
            } else {
                html! {}
            }
        }
        </div>

    }
}

