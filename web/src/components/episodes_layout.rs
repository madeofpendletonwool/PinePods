use std::rc::Rc;
use yew::{Callback, function_component, Html, html};
use web_sys::MouseEvent;
use yew_router::history::BrowserHistory;
use yewdux::Dispatch;
use yewdux::prelude::*;
use crate::components::context::{AppState};
use crate::components::audio::{AudioPlayerProps, AudioPlayer};
use super::gen_components::Search_nav;
use super::app_drawer::App_drawer;

#[function_component(EpisodeLayout)]
pub fn episode_layout() -> Html {
    // let dispatch = Dispatch::<AppState>::global();
    // // let (state, _dispatch) = use_store::<AppState>();
    // let state: Rc<AppState> = dispatch.get();
    let (state, _dispatch) = use_store::<AppState>();
    let podcast_feed_results = state.podcast_feed_results.clone();
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    web_sys::console::log_1(&format!("Search Results: {:?}", podcast_feed_results).into());

    html! {
        <div>
            <Search_nav />
            <h1 class="text-2xl font-bold my-4 center-text">{ "Podcast Episode Results" }</h1>
            {
                if let Some(results) = podcast_feed_results {
                    html! {
                        <div>
                            { for results.episodes.iter().map(|episode| {
                                let dispatch = _dispatch.clone();
                                let history = history_clone.clone();

                                // Clone the variables outside the closure
                                let episode_url_clone = episode.enclosure_url.clone().unwrap_or_default();
                                let episode_title_clone = episode.title.clone().unwrap_or_default();

                                let on_play_click = {
                                    web_sys::console::log_1(&format!("Play Clicked with URL: {}", &episode_url_clone).into());
                                    let episode_url_for_closure = episode_url_clone.clone();
                                    let episode_title_for_closure = episode_title_clone.clone();
                                    let dispatch = dispatch.clone();

                                    Callback::from(move |_: MouseEvent| { // Ensure this is triggered only by a MouseEvent
                                        web_sys::console::log_1(&"Play Clicked".to_string().into());
                                        let episode_url_for_closure = episode_url_for_closure.clone();
                                        let episode_title_for_closure = episode_title_clone.clone();
                                        let dispatch = dispatch.clone();
                                        dispatch.reduce_mut(move |state| {
                                            web_sys::console::log_1(&format!("Before state update: {:?}", state.audio_playing).into());
                                            state.audio_playing = Some(true);
                                            state.currently_playing = Some(AudioPlayerProps {
                                                src: episode_url_for_closure.clone(),
                                                title: episode_title_for_closure.clone(),
                                            });
                                            state.set_audio_source(episode_url_for_closure.to_string()); // Set the audio source here
                                            state.toggle_playback();
                                            web_sys::console::log_1(&format!("After state update: {:?}", state.audio_playing).into());
                                            web_sys::console::log_1(&format!("After state update: {:?}", state.currently_playing).into());
                                        });
                                    })
                                };

                                html! {
                                    <div class="flex items-center mb-4 bg-white shadow-md rounded-lg overflow-hidden">
                                        <img src={episode.artwork.clone().unwrap_or_default()} alt={format!("Cover for {}", &episode.title.clone().unwrap_or_default())} class="w-1/4 object-cover"/>
                                        <div class="flex flex-col p-4 space-y-2 w-7/12">
                                            <p class="text-xl font-semibold">{ &episode.title.clone().unwrap_or_default() }</p>
                                            <p class="text-gray-600">{ &episode.description.clone().unwrap_or_default() }</p>
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
                        <div class="empty-episodes-container">
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
                web_sys::console::log_1(&"Running audio props".into());
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} /> }
            } else {
                web_sys::console::log_1(&"Player not loading".into());
                html! {}
            }
        }
        </div>

    }
}

