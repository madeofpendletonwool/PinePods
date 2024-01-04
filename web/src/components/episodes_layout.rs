use std::io::BufRead;
use yew::{Callback, function_component, Html, html};
use web_sys::MouseEvent;
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use crate::components::audio::{AudioPlayerProps, AudioPlayer};
use crate::components::audio::_AudioPlayerProps::duration;
use super::gen_components::Search_nav;
use super::app_drawer::App_drawer;

#[function_component(EpisodeLayout)]
pub fn episode_layout() -> Html {
    // let dispatch = Dispatch::<AppState>::global();
    // // let (state, _dispatch) = use_store::<AppState>();
    // let state: Rc<AppState> = dispatch.get();
    let (state, _dispatch) = use_store::<UIState>();
    let (search_state, _search_dispatch) = use_store::<AppState>();
    let podcast_feed_results = search_state.podcast_feed_results.clone();
    let history = BrowserHistory::new();
    let history_clone = history.clone();

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
                                let episode_duration_clone = episode.duration.clone().unwrap_or_default();

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
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} duration={audio_props.duration.clone()} duration_sec={audio_props.duration_sec.clone()} /> }
            } else {
                html! {}
            }
        }
        </div>

    }
}

