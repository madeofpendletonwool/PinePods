use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::gen_funcs::format_error_message;
use crate::requests::pod_req::{
    call_check_youtube_channel, call_remove_youtube_channel, call_subscribe_to_channel,
    RemoveYouTubeChannelValues,
};
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
    let (audio_state, _audio_dispatch) = use_store::<UIState>();

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
    let is_subscribed = use_state(|| false);
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

    // On mount, check if the channel is in the database
    let effect_server_name = server_name.clone();
    let effect_api_key = api_key.clone();
    let effect_user_id = user_id.clone();
    {
        let is_subscribed = is_subscribed.clone();
        let channel_name = channel.name.clone();
        let channel_url = format!("https://www.youtube.com/channel/{}", channel.channel_id);

        use_effect_with((channel_name, channel_url), move |(name, url)| {
            let name = name.clone();
            let url = url.clone();
            let is_subscribed = is_subscribed.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let subscribed = call_check_youtube_channel(
                    &effect_server_name,
                    &effect_api_key,
                    effect_user_id,
                    &name,
                    &url,
                )
                .await
                .unwrap_or_default()
                .exists;
                is_subscribed.set(subscribed);
            });
            || ()
        });
    }

    let on_button_click = {
        let channel = channel.clone();
        let set_loading = set_loading.clone();
        let is_subscribed = is_subscribed.clone();
        let dispatch = _dispatch.clone();

        Callback::from(move |_: MouseEvent| {
            let channel = channel.clone();
            let set_loading = set_loading.clone();
            let is_subscribed = is_subscribed.clone();
            let dispatch = dispatch.clone();
            let server_name_wasm = server_name.clone();
            let api_key_wasm = api_key.clone();
            let user_id_wasm = user_id.clone();

            wasm_bindgen_futures::spawn_local(async move {
                set_loading.set(true);

                if !*is_subscribed {
                    // Subscribe
                    match call_subscribe_to_channel(
                        &server_name_wasm,
                        &api_key_wasm,
                        user_id_wasm,
                        &channel.channel_id,
                    )
                    .await
                    {
                        Ok(_) => {
                            is_subscribed.set(true);
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some(
                                    "Successfully subscribed to channel. Videos will be processed in background."
                                        .to_string()
                                );
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to subscribe to channel: {}",
                                    formatted_error
                                ));
                            });
                        }
                    }
                } else {
                    // Unsubscribe
                    let remove_channel = RemoveYouTubeChannelValues {
                        user_id,
                        channel_name: channel.name.clone(),
                        channel_url: format!(
                            "https://www.youtube.com/channel/{}",
                            channel.channel_id
                        ),
                    };

                    match call_remove_youtube_channel(
                        &server_name_wasm,
                        &Some(api_key_wasm),
                        &remove_channel,
                    )
                    .await
                    {
                        Ok(_) => {
                            is_subscribed.set(false);
                            dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Successfully unsubscribed from channel.".to_string());
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to unsubscribe from channel: {}",
                                    formatted_error
                                ));
                            });
                        }
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

    let button_text = if *is_subscribed {
        html! { <i class="ph ph-trash text-4xl"></i> }
    } else {
        html! { <i class="ph ph-plus-circle text-4xl"></i> }
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
                onclick={on_button_click}
                disabled={*set_loading}
            >
                {
                    if *set_loading {
                        html! { <i class="ph ph-spinner-ball animate-spin text-4xl"></i> }
                    } else {
                        button_text
                    }
                }
            </button>
        </div>
    }
}
