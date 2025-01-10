use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::requests::pod_req::call_subscribe_to_channel;
use crate::requests::search_pods::YouTubeChannel;
use std::collections::HashMap;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct YouTubeChannelItemProps {
    pub channel: YouTubeChannel,
}

#[function_component(YouTubeLayout)]
pub fn youtube_layout() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                <h1 class="item_container-text text-2xl font-bold my-4 center-text">{ "YouTube Channels" }</h1>
                {
                    if let Some(results) = &state.youtube_search_results {
                        // Deduplicate channels based on channel_id
                        let unique_channels: Vec<_> = results.channels
                            .iter()
                            .fold(HashMap::new(), |mut map, channel| {
                                map.entry(channel.channel_id.clone())
                                    .or_insert_with(|| channel.clone());
                                map
                            })
                            .values()
                            .cloned()
                            .collect();

                        html! {
                            <div>
                                if !unique_channels.is_empty() {
                                    { for unique_channels.iter().map(|channel| html! {
                                        <YouTubeChannelItem channel={channel.clone()} />
                                    })}
                                } else {
                                    { empty_message(
                                        "No Channels Found",
                                        "Try searching with different keywords."
                                    )}
                                }
                            </div>
                        }
                    } else {
                        { empty_message(
                            "Search for YouTube Channels",
                            "Enter a channel name in the search bar above."
                        )}
                    }
                }
            // Conditional rendering for the error banner
            if let Some(error) = error_message {
                <div class="error-snackbar">{ error }</div>
            }
            if let Some(info) = info_message {
                <div class="info-snackbar">{ info }</div>
            }
            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
                } else {
                    html! {}
                }
            }
            </div>
            <App_drawer />
        </>
    }
}

#[function_component(YouTubeChannelItem)]
fn youtube_channel_item(props: &YouTubeChannelItemProps) -> Html {
    let (_state, _dispatch) = use_store::<AppState>();
    let channel = &props.channel;
    let set_loading = use_state(|| false);

    let on_subscribe = {
        let channel = channel.clone();
        let set_loading = set_loading.clone();

        Callback::from(move |_: MouseEvent| {
            let channel = channel.clone();
            let set_loading = set_loading.clone();
            let dispatch = _dispatch.clone();

            // Get necessary info from state
            let server_name = _state
                .auth_details
                .as_ref()
                .map(|ud| ud.server_name.clone())
                .unwrap();
            let api_key = _state
                .auth_details
                .as_ref()
                .map(|ud| ud.api_key.clone())
                .unwrap()
                .unwrap();
            let user_id = _state
                .user_details
                .as_ref()
                .map(|ud| ud.UserID.clone())
                .unwrap();

            wasm_bindgen_futures::spawn_local(async move {
                set_loading.set(true);

                match call_subscribe_to_channel(
                    &server_name,
                    &api_key,
                    user_id,
                    &channel.channel_id,
                )
                .await
                {
                    Ok(response) => {
                        dispatch.reduce_mut(|state| {
                            state.info_message = Some(
                                "Successfully subscribed to channel. Videos will be processed in background."
                                    .to_string()
                            );
                        });
                    }
                    Err(e) => {
                        dispatch.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Failed to subscribe to channel: {}", e));
                        });
                    }
                }

                set_loading.set(false);
            });
        })
    };

    // Use channel ID for thumbnail
    let thumbnail = if !channel.thumbnail_url.is_empty() {
        channel.thumbnail_url.clone()
    } else {
        "/api/placeholder/400/320".to_string()
    };

    html! {
        <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg">
            <div class="flex flex-col w-auto object-cover pl-4">
                <img
                    src={thumbnail}
                    alt={format!("{} channel thumbnail", channel.name)}
                    class="object-cover align-top-cover w-full item-container img"
                />
            </div>
            <div class="flex items-start flex-col p-4 space-y-2 w-11/12">
                <p class="item_container-text text-xl font-semibold">{ &channel.name }</p>
                <p class="item_container-text">{ &channel.description }</p>
                // Recent videos section
                if !channel.recent_videos.is_empty() {
                    <div class="mt-4">
                        <p class="header-text font-semibold">{"Recent Videos:"}</p>
                        { for channel.recent_videos.iter().take(3).map(|video| html! {
                            <p class="header-text text-sm">{&video.title}</p>
                        })}
                    </div>
                }
            </div>
            <button
                class="item-container-button selector-button font-bold rounded-full self-center mr-8 flex items-center justify-center"
                style="width: 180px; height: 180px;"
                onclick={on_subscribe}
            >
                <i class="ph ph-plus text-4xl"></i>
            </button>
        </div>
    }
}
