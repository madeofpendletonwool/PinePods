use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::gen_funcs::format_error_message;
use crate::requests::pod_req::{
    call_check_youtube_channel, call_remove_youtube_channel, call_subscribe_to_channel,
    RemoveYouTubeChannelValues,
};
use crate::requests::search_pods::YouTubeChannel;
use gloo::events::EventListener;
use i18nrs::yew::use_translation;
use std::collections::HashMap;
use web_sys::MouseEvent;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct YouTubeChannelItemProps {
    pub channel: YouTubeChannel,
}

#[function_component(YouTubeLayout)]
pub fn youtube_layout() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();

    // Track window width to apply responsive columns
    let columns = use_state(|| 2); // Default to 2 columns

    // Pre-capture translation strings
    let youtube_channels_title = i18n.t("youtube_layout.youtube_channels");
    let no_channels_found_msg = i18n.t("youtube_layout.no_channels_found");
    let try_different_keywords_msg = i18n.t("youtube_layout.try_different_keywords");
    let search_youtube_channels_msg = i18n.t("youtube_layout.search_youtube_channels");
    let enter_channel_name_msg = i18n.t("youtube_layout.enter_channel_name");

    {
        let columns = columns.clone();

        use_effect_with((), move |_| {
            let update_columns = {
                let columns = columns.clone();

                Callback::from(move |_| {
                    if let Some(window) = web_sys::window() {
                        let width = window.inner_width().unwrap().as_f64().unwrap();

                        // Progressive breakpoints for different screen sizes
                        let new_columns = if width < 640.0 {
                            2 // Extra small screens: 2 columns
                        } else if width < 1024.0 {
                            2 // Small to medium screens: 2 columns
                        } else if width < 1280.0 {
                            3 // Large screens: 3 columns
                        } else {
                            4 // Extra large screens: 4 columns
                        };

                        columns.set(new_columns);
                    }
                })
            };

            // Initial update
            update_columns.emit(());

            // Add resize listener
            let window = web_sys::window().unwrap();
            let listener = EventListener::new(&window, "resize", move |_| {
                update_columns.emit(());
            });

            // Cleanup
            move || drop(listener)
        });
    }

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                <h1 class="item_container-text text-2xl font-bold my-6 text-center">{youtube_channels_title}</h1>
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

                        if !unique_channels.is_empty() {
                            let column_width = format!("calc({}% - {}px)", 100.0 / *columns as f32, 16);

                            html! {
                                <div class="youtube-flex-container" style="display: flex; flex-wrap: wrap; gap: 16px; padding: 0 12px 24px; width: 100%;">
                                    { for unique_channels.iter().map(|channel| html! {
                                        <div style={format!("width: {}; margin-bottom: 16px;", column_width)}>
                                            <YouTubeChannelItem channel={channel.clone()} />
                                        </div>
                                    })}
                                </div>
                            }
                        } else {
                            empty_message(
                                &no_channels_found_msg,
                                &try_different_keywords_msg
                            )
                        }
                    } else {
                        empty_message(
                            &search_youtube_channels_msg,
                            &enter_channel_name_msg
                        )
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
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let channel = &props.channel;
    let set_loading = use_state(|| false);
    let is_subscribed = use_state(|| false);
    let is_description_expanded = use_state(|| false);

    // Pre-capture translation strings for async blocks
    let successfully_subscribed_msg = i18n.t("youtube_layout.successfully_subscribed");
    let failed_to_subscribe_msg = i18n.t("youtube_layout.failed_to_subscribe");
    let successfully_unsubscribed_msg = i18n.t("youtube_layout.successfully_unsubscribed");
    let failed_to_unsubscribe_msg = i18n.t("youtube_layout.failed_to_unsubscribe");
    let show_less_text = i18n.t("youtube_layout.show_less");
    let show_more_text = i18n.t("youtube_layout.show_more");
    let channel_thumbnail_text = i18n.t("youtube_layout.channel_thumbnail");

    let server_name = state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone())
        .unwrap();
    let api_key = state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone())
        .unwrap()
        .unwrap();
    let user_id = state
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
        let dispatch = dispatch.clone();
        let successfully_subscribed_msg_clone = successfully_subscribed_msg.clone();
        let failed_to_subscribe_msg_clone = failed_to_subscribe_msg.clone();
        let successfully_unsubscribed_msg_clone = successfully_unsubscribed_msg.clone();
        let failed_to_unsubscribe_msg_clone = failed_to_unsubscribe_msg.clone();

        Callback::from(move |_: MouseEvent| {
            let channel = channel.clone();
            let set_loading = set_loading.clone();
            let is_subscribed = is_subscribed.clone();
            let dispatch = dispatch.clone();
            let server_name_wasm = server_name.clone();
            let api_key_wasm = api_key.clone();
            let user_id_wasm = user_id.clone();
            let successfully_subscribed_msg = successfully_subscribed_msg_clone.clone();
            let failed_to_subscribe_msg = failed_to_subscribe_msg_clone.clone();
            let successfully_unsubscribed_msg = successfully_unsubscribed_msg_clone.clone();
            let failed_to_unsubscribe_msg = failed_to_unsubscribe_msg_clone.clone();

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
                                state.info_message = Some(successfully_subscribed_msg.clone());
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}: {}",
                                    failed_to_subscribe_msg,
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
                                state.info_message = Some(successfully_unsubscribed_msg.clone());
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}: {}",
                                    failed_to_unsubscribe_msg,
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

    // Toggle description expansion
    let toggle_description = {
        let is_description_expanded = is_description_expanded.clone();

        Callback::from(move |_: MouseEvent| {
            is_description_expanded.set(!*is_description_expanded);
        })
    };

    // Use channel ID for thumbnail
    let thumbnail = if !channel.thumbnail_url.is_empty() {
        channel.thumbnail_url.clone()
    } else {
        "/api/placeholder/400/320".to_string()
    };

    let button_icon = if *set_loading {
        "ph-spinner-gap animate-spin"
    } else if *is_subscribed {
        "ph-trash"
    } else {
        "ph-plus-circle"
    };

    html! {
        <div class="search-item-container border-solid border rounded-lg overflow-hidden shadow-md flex flex-col h-full">
            <div class="relative w-full search-podcast-image-container" style="aspect-ratio: 1/1; padding-bottom: 100%;">
                <FallbackImage
                    src={thumbnail}
                    alt={format!("{} {}", channel.name, channel_thumbnail_text)}
                    class="absolute inset-0 w-full h-full object-cover transition-transform duration-200 hover:scale-105 cursor-pointer"
                />
            </div>

            <div class="p-4 flex flex-col flex-grow">
                <div class="flex justify-between items-start mb-2">
                    <h3 class="item_container-text text-xl font-semibold line-clamp-2 hover:text-opacity-80 transition-colors">
                        {&channel.name}
                    </h3>

                    <button
                        class="item-container-button selector-button flex items-center justify-center rounded-full ml-3 flex-shrink-0 transition-all duration-200 ease-in-out hover:bg-opacity-80"
                        style="width: 40px; height: 40px;"
                        onclick={on_button_click}
                        disabled={*set_loading}
                    >
                        <i class={format!("ph {} text-2xl", button_icon)}></i>
                    </button>
                </div>

                // Display description if available
                {
                    if !channel.description.is_empty() {
                        html! {
                            <>
                                <div
                                    class={if *is_description_expanded { "item_container-text text-sm mb-3" } else { "item_container-text text-sm mb-3 line-clamp-3" }}
                                    onclick={toggle_description.clone()}
                                >
                                    { &channel.description }
                                </div>

                                {
                                    if channel.description.len() > 150 {
                                        html! {
                                            <button
                                                class="text-sm font-medium mb-3 text-left hover:underline item_container-text opacity-80"
                                                onclick={toggle_description}
                                            >
                                                {if *is_description_expanded { show_less_text.clone() } else { show_more_text.clone() }}
                                            </button>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </div>
    }
}
