use super::gen_components::{empty_message, UseScrollToTop};
use crate::components::audio::on_play_click_shared;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::{SafeHtml, UIStateMsg};
use crate::components::gen_funcs::{
    format_datetime, format_time, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::requests::pod_req::call_get_episode_by_url_key;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::window;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::prelude::*;
use yewdux::prelude::*;

#[derive(Clone, PartialEq, Routable)]
pub enum Route {
    #[at("/shared_episode/:url_key")]
    Person { url_key: String },
    #[at("/")]
    Home,
}

#[derive(Clone, Properties, PartialEq)]
pub struct SharedProps {
    pub url_key: String,
}

#[function_component(SharedEpisode)]
pub fn shared_episode(_props: &SharedProps) -> Html {
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str("shared ep hit"));
    let (state, dispatch) = use_store::<AppState>();

    let error = use_state(|| None);

    let (_post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    // let server_name = post_state
    //     .auth_details
    //     .as_ref()
    //     .map(|ud| ud.server_name.clone());
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();
    let loading = use_state(|| true); // Initial loading state set to true

    {
        let audio_dispatch = audio_dispatch.clone();

        // Initial check when the component is mounted
        {
            let window = window().unwrap();
            let width = window.inner_width().unwrap().as_f64().unwrap();
            let new_is_mobile = width < 768.0;
            audio_dispatch.reduce_mut(|state| state.is_mobile = Some(new_is_mobile));
        }

        // Resize event listener
        use_effect_with((), move |_| {
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str("resize event"));
            let window = window().unwrap();
            let closure_window = window.clone();
            let closure = Closure::wrap(Box::new(move || {
                let width = closure_window.inner_width().unwrap().as_f64().unwrap();
                let new_is_mobile = width < 768.0;
                audio_dispatch.reduce_mut(|state| state.is_mobile = Some(new_is_mobile));
            }) as Box<dyn Fn()>);

            window
                .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
                .unwrap();

            closure.forget(); // Ensure the closure is not dropped prematurely

            || ()
        });
    }
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str("window type check"));

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
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str("messaging setup"));

    // Fetch episode on component mount
    {
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str("in ep setup"));
        let error = error.clone();
        let effect_dispatch = dispatch.clone();
        let loading_clone = loading.clone();

        use_effect_with((), move |_| {
            let error_clone = error.clone();

            // Fetch the server name from the current URL
            let window = web_sys::window().expect("no global window exists");
            let location = window.location();
            let server_name = format!(
                "{}//{}",
                location.protocol().unwrap(),
                location.host().unwrap()
            ); // Extracts the protocol and host

            web_sys::console::log_1(&format!("Server name: {}", server_name).into());

            let dispatch = effect_dispatch.clone();

            // Fetch the URL key from the current window location
            let url_pathname = location.pathname().unwrap();
            let url_key = url_pathname
                .split('/')
                .last()
                .unwrap_or_default()
                .to_string();

            // Ensure the URL key is valid before proceeding
            if !url_key.is_empty() {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_get_episode_by_url_key(&server_name, &url_key).await {
                        Ok(shared_episode_data) => {
                            web_sys::console::log_1(&"Shared episode data fetched".into());

                            dispatch.reduce_mut(move |state| {
                                state.shared_fetched_episode = Some(shared_episode_data);
                            });

                            loading_clone.set(false);
                        }
                        Err(e) => {
                            error_clone.set(Some(format!("Error fetching shared episode: {}", e)));
                        }
                    }
                });
            } else {
                web_sys::console::log_1(&"No URL key found".into());
            }

            || ()
        });
    }
    web_sys::console::log_1(&wasm_bindgen::JsValue::from_str("ep setup"));

    // let completion_status = use_state(|| false); // State to track completion status

    html! {
        <>
        <div class="main-container">
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
                    if let Some(episode) = state.shared_fetched_episode.clone() {
                        let episode_url_clone = episode.episode.episodeurl.clone();
                        let episode_title_clone = episode.episode.episodetitle.clone();
                        let episode_description_clone = episode.episode.episodedescription.clone();
                        let episode_release_date_clone = episode.episode.episodepubdate.clone();
                        let episode_artwork_clone = episode.episode.episodeartwork.clone();
                        let episode_duration_clone = episode.episode.episodeduration.clone();
                        let episode_id_clone = episode.episode.episodeid.clone();

                        let sanitized_description = sanitize_html_with_blank_target(&episode.episode.episodedescription.clone());
                        let description = sanitized_description;

                        let episode_url_for_closure = episode_url_clone.clone();
                        let episode_title_for_closure = episode_title_clone.clone();
                        let episode_desc_for_closure = episode_description_clone.clone();
                        let episode_release_clone = episode_release_date_clone.clone();
                        let episode_artwork_for_closure = episode_artwork_clone.clone();
                        let episode_duration_for_closure = episode_duration_clone.clone();
                        let episode_id_for_closure = episode_id_clone.clone();
                        let audio_dispatch = audio_dispatch.clone();

                        let on_play_click = on_play_click_shared(
                            episode_url_for_closure.clone(),
                            episode_title_for_closure.clone(),
                            episode_desc_for_closure.clone(),
                            episode_release_clone.clone(),
                            episode_artwork_for_closure.clone(),
                            episode_duration_for_closure.clone(),
                            episode_id_for_closure.clone(),
                            audio_dispatch.clone(),
                        );

                        let datetime = parse_date(&episode.episode.episodepubdate, &state.user_tz);
                        let date_format = match_date_format(state.date_format.as_deref());
                        let format_duration = format_time(episode.episode.episodeduration as f64);
                        let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));

                        let episode_url_check = episode_url_clone;
                        let should_show_buttons = !episode_url_check.is_empty();
                        web_sys::console::log_1(&format!("Episode URL: {}", episode_url_check).into());
                        web_sys::console::log_1(&format!("Should show buttons: {}", should_show_buttons).into());

                        let open_in_new_tab = Callback::from(move |url: String| {
                            let window = web_sys::window().unwrap();
                            window.open_with_url_and_target(&url, "_blank").unwrap();
                        });
                        // let format_duration = format!("Duration: {} minutes", e / 60); // Assuming duration is in seconds
                        // let format_release = format!("Released on: {}", &episode.episode.EpisodePubDate);
                        let layout = if audio_state.is_mobile.unwrap_or(false) {
                            html! {
                                <div class="mobile-layout">
                                <div class="episode-layout-container">
                                        <div class="item-header-mobile-cover-container">
                                        <img src={episode.episode.episodeartwork.clone()} class="episode-artwork" />
                                        </div>
                                            <div class="episode-details">
                                            <p class="item-header-pod justify-center items-center">{ &episode.episode.podcastname }</p>
                                            <div class="items-center space-x-2 cursor-pointer">
                                                <h2 class="episode-title item-header-title">
                                                    { &episode.episode.episodetitle }
                                                </h2>
                                            </div>
                                            // <h2 class="episode-title">{ &episode.episode.episodetitle }</h2>
                                            <div class="flex justify-center items-center item-header-details">
                                                <p class="episode-duration">{ format_duration }</p>
                                                <span class="episode-duration">{"\u{00a0}-\u{00a0}"}</span>
                                                <p class="episode-release-date">{ format_release }</p>
                                            </div>
                                        </div>
                                    <div class="episode-action-buttons">
                                        <div class="button-row">
                                            <button onclick={on_play_click} class="play-button">
                                            // <button class="play-button">
                                                <i class="ph ph-play"></i>
                                                {"Play"}
                                            </button>
                                        </div>
                                    </div>
                                    <div class="episode-single-desc episode-description">
                                    // <p>{ description }</p>
                                    <div class="item_container-text episode-description-container">
                                        <SafeHtml html={description} />
                                    </div>
                                    </div>
                                </div>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="episode-layout-container-shared" style="padding-top: 20px;">
                                    <div class="episode-top-info">
                                        <img src={episode.episode.episodeartwork.clone()} class="episode-artwork" />
                                        <div class="episode-details">
                                            <h1 class="podcast-title">{ &episode.episode.podcastname }</h1>
                                            <div class="flex items-center space-x-2 cursor-pointer">
                                                <h2 class="episode-title">{ &episode.episode.episodetitle }</h2>
                                            </div>
                                            // <h2 class="episode-title">{ &episode.episode.episodetitle }</h2>
                                            <p class="episode-duration">{ format_duration }</p>
                                            <p class="episode-release-date">{ format_release }</p>
                                            {
                                                if let Some(transcript) = &audio_state.episode_page_transcript {
                                                    if !transcript.is_empty() {
                                                        let transcript_clone = transcript.clone();
                                                        html! {
                                                            <>
                                                            { for transcript_clone.iter().map(|transcript| {
                                                                let open_in_new_tab = open_in_new_tab.clone();
                                                                let url = transcript.url.clone();
                                                                html! {
                                                                    <div class="header-info pb-2 pt-2">
                                                                        <button
                                                                            onclick={Callback::from(move |_| open_in_new_tab.emit(url.clone()))}
                                                                            title={"Transcript"}
                                                                            class="font-bold item-container-button"
                                                                        >
                                                                            { "Episode Transcript" }
                                                                        </button>
                                                                    </div>
                                                                }
                                                            })}
                                                            </>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            // {
                                            //     if let Some(people) = &audio_state.episode_page_people {
                                            //         if !people.is_empty() {
                                            //             html! {
                                            //                 <div class="header-info">
                                            //                     <HostDropdown title="In This Episode" hosts={people.clone()} podcast_feed_url={episode.episode.episodeurl} />
                                            //                 </div>
                                            //             }
                                            //         } else {
                                            //             html! {}
                                            //         }
                                            //     } else {
                                            //         html! {}
                                            //     }
                                            // }
                                        </div>
                                    </div>
                                    <div class="episode-action-buttons">
                                    <button onclick={on_play_click} class="play-button">
                                    // <button class="play-button">
                                        <i class="ph ph-play"></i>
                                        {"Play"}
                                    </button>

                                    </div>
                                    <hr class="episode-divider" />
                                    <div class="episode-single-desc episode-description">
                                    // <p>{ description }</p>
                                    <div class="item_container-text episode-description-container">
                                        <SafeHtml html={description} />
                                    </div>
                                    </div>
                                </div>
                            }
                        };  // Add semicolon here
                        // item

                        layout
                    } else {
                        empty_message(
                            "Unable to display episode",
                            "Something seems to have gone wrong. A straightup server disconnect maybe? Did you browse here directly? That's not how this app works. It needs the context to browse around. I honestly don't have anything else for you as this shouldn't happen. This is embarrasing."
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
        </>
    }
}
