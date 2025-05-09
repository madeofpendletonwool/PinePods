use super::gen_components::{
    on_shownotes_click, ContextButton, EpisodeModal, EpisodeTrait, FallbackImage,
};
use super::gen_funcs::{format_datetime, match_date_format, parse_date};
use crate::components::audio::on_play_pause;
use crate::components::context::{AppState, UIState};
use crate::components::gen_funcs::{
    convert_time_to_seconds, sanitize_html_with_blank_target, truncate_description,
};
use crate::components::gen_funcs::{format_time, strip_images_from_html};
use crate::components::safehtml::SafeHtml;
use crate::requests::search_pods::Episode;
use gloo::events::EventListener;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::{window, Element, HtmlElement, MouseEvent};
use yew::prelude::*;
use yew::Properties;
use yew::{function_component, html, use_effect_with, use_node_ref, Callback, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

#[derive(Properties, PartialEq)]
pub struct PodcastEpisodeVirtualListProps {
    pub episodes: Vec<Episode>,
    pub item_height: f64,
    pub podcast_added: bool,
    pub search_state: Rc<AppState>,
    pub search_ui_state: Rc<UIState>,
    pub dispatch: Dispatch<UIState>,
    pub search_dispatch: Dispatch<AppState>,
    pub history: BrowserHistory,
    pub server_name: Option<String>,
    pub user_id: Option<i32>,
    pub api_key: Option<Option<String>>,
    pub podcast_link: String,
    pub podcast_title: String,
}

#[function_component(PodcastEpisodeVirtualList)]
pub fn podcast_episode_virtual_list(props: &PodcastEpisodeVirtualListProps) -> Html {
    let scroll_pos = use_state(|| 0.0);
    let container_ref = use_node_ref();
    let container_height = use_state(|| 0.0);
    let item_height = use_state(|| 234.0); // Default item height
    let container_item_height = use_state(|| 221.0); // Actual container height, separate from spacing
    let force_update = use_state(|| 0);
    let selected_episode_index = use_state(|| None::<usize>);

    // Effect to set initial container height, item height, and listen for window resize
    {
        let container_height = container_height.clone();
        let item_height = item_height.clone();
        let container_item_height = container_item_height.clone();
        let force_update = force_update.clone();

        use_effect_with((), move |_| {
            let window = window().expect("no global `window` exists");
            let window_clone = window.clone();

            let update_sizes = Callback::from(move |_| {
                let height = window_clone.inner_height().unwrap().as_f64().unwrap();
                container_height.set(height - 100.0);

                let width = window_clone.inner_width().unwrap().as_f64().unwrap();
                // Set both the total item height (with margin) and container height
                let (new_item_height, new_container_height) = if width <= 530.0 {
                    (122.0 + 16.0, 122.0)
                } else if width <= 768.0 {
                    (150.0 + 16.0, 150.0)
                } else {
                    (221.0 + 16.0, 221.0)
                };

                item_height.set(new_item_height);
                container_item_height.set(new_container_height);
                force_update.set(*force_update + 1);
            });

            update_sizes.emit(());

            let listener = EventListener::new(&window, "resize", move |_| {
                update_sizes.emit(());
            });

            move || drop(listener)
        });
    }

    // Effect for scroll handling
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

    let on_modal_close = {
        let selected_episode_index = selected_episode_index.clone();
        Callback::from(move |_: MouseEvent| selected_episode_index.set(None))
    };

    let visible_episodes = (start_index..end_index)
        .map(|index| {
            // Replace the modal open/close callbacks with:
            let on_modal_open = {
                let selected_episode_index = selected_episode_index.clone();
                let index = index; // This is your loop index
                Callback::from(move |_: MouseEvent| selected_episode_index.set(Some(index)))
            };

            let episode = &props.episodes[index];
            let dispatch = props.dispatch.clone();
            let search_state_clone = props.search_state.clone();
            let search_ui_state_clone = props.search_ui_state.clone();

            let episode_url_clone = episode.enclosure_url.clone().unwrap_or_default();
            let episode_title_clone = episode.title.clone().unwrap_or_default();
            let episode_description_clone = episode.description.clone().unwrap_or_default();
            let episode_artwork_clone = episode.artwork.clone().unwrap_or_default();
            let episode_duration_clone = episode.duration.clone().unwrap_or_default();
            let episode_is_youtube = episode.is_youtube.clone();
            let episode_duration_in_seconds = match convert_time_to_seconds(&episode_duration_clone) {
                Ok(seconds) => seconds as i32,
                Err(e) => {
                    eprintln!("Failed to convert time to seconds: {}", e);
                    0
                }
            };
            let episode_id_clone = episode.episode_id.unwrap_or(0);

            let server_name_play = props.server_name.clone();
            let user_id_play = props.user_id;
            let api_key_play = props.api_key.clone();

            let is_expanded = search_state_clone.expanded_descriptions.contains(&episode.guid.clone().unwrap());

            let sanitized_description = sanitize_html_with_blank_target(&episode.description.clone().unwrap_or_default());
            let (description, _is_truncated) = if is_expanded {
                (sanitized_description, false)
            } else {
                truncate_description(sanitized_description, 300)
            };

            let date_format = match_date_format(search_state_clone.date_format.as_deref());
            let datetime = parse_date(&episode.pub_date.clone().unwrap_or_default(), &search_state_clone.user_tz);
            let format_release = format!("{}", format_datetime(&datetime, &search_state_clone.hour_preference, date_format));

            let on_play_pause = on_play_pause(
                episode_url_clone.clone(),
                episode_title_clone.clone(),
                episode_description_clone.clone(),
                format_release.clone(),
                episode_artwork_clone.clone(),
                episode_duration_in_seconds,
                episode_id_clone.clone(),
                Some(0),
                api_key_play.unwrap().unwrap(),
                user_id_play.unwrap(),
                server_name_play.unwrap(),
                dispatch.clone(),
                search_ui_state_clone.clone(),
                None,
                episode_is_youtube,
            );

            let boxed_episode = Box::new(episode.clone()) as Box<dyn EpisodeTrait>;
            let formatted_duration = format_time(episode_duration_in_seconds.into());
            let is_current_episode = props.search_ui_state
                .currently_playing
                .as_ref()
                .map_or(false, |current| {
                    // Compare both title and URL for uniqueness since we don't have IDs
                    current.title == episode.title.clone().unwrap_or_default() &&
                    current.src == episode.enclosure_url.clone().unwrap_or_default()
                });

            let is_playing = props.search_ui_state.audio_playing.unwrap_or(false);


            let episode_url_for_ep_item = episode_url_clone.clone();
            let should_show_buttons = !episode_url_for_ep_item.is_empty();
            let preview_description = strip_images_from_html(&description);

            let make_shownotes_callback = {
                let history = props.history.clone();
                let search_dispatch = props.search_dispatch.clone();
                let podcast_link = props.podcast_link.clone();
                let podcast_title = props.podcast_title.clone();
                let episode_id = episode.episode_id.unwrap_or(0);
                let episode_url = episode.enclosure_url.clone().unwrap_or_default();
                let is_youtube = episode.is_youtube.clone();

                Callback::from(move |_: MouseEvent| {
                    on_shownotes_click(
                        history.clone(),
                        search_dispatch.clone(),
                        Some(episode_id),
                        Some(podcast_link.clone()),
                        Some(episode_url.clone()),
                        Some(podcast_title.clone()),
                        true,
                        None,
                        is_youtube
                    ).emit(MouseEvent::new("click").unwrap());
                })
            };

            html! {
                <>
                <div
                    key={format!("{}-{}", episode.episode_id.unwrap_or(0), *force_update)}
                    class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg"
                    style={format!("height: {}px; overflow: hidden;", *container_item_height)}
                >
                    <div class="flex flex-col w-auto object-cover pl-4">
                        <FallbackImage
                            src={episode.artwork.clone().unwrap_or_default()}
                            alt={format!("Cover for {}", &episode.title.clone().unwrap_or_default())}
                            class="episode-image"
                        />
                    </div>
                    <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                        <div class="flex items-center space-x-2 cursor-pointer" onclick={make_shownotes_callback.clone()}>
                            <p class="item_container-text episode-title font-semibold line-clamp-2">
                                { &episode.title.clone().unwrap_or_default() }
                            </p>
                            {
                                if episode.completed.unwrap_or(false) {
                                    html! {
                                        <i class="ph ph-check-circle text-2xl text-green-500"></i>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                        </div>
                        {
                            html! {
                                <div class="item-description-text cursor-pointer hidden md:block"
                                     onclick={on_modal_open.clone()}>
                                    <div class="item_container-text line-clamp-2">
                                        <SafeHtml html={preview_description} />
                                    </div>
                                </div>
                            }
                        }
                        <div class="episode-time-badge-container" style="max-width: 100%; overflow: hidden;">
                            <span
                                class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2"
                                style="flex-grow: 0; flex-shrink: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
                            >
                                <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                    <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                                </svg>
                                { format_release.clone() }
                            </span>
                        </div>

                        {
                            if episode.completed.unwrap_or(false) {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{ formatted_duration }</span>
                                        <span class="item_container-text">{ "-  Completed" }</span>
                                    </div>
                                }
                            } else {
                                if let Some(listen_duration) = episode.listen_duration {
                                    let listen_duration_percentage = if episode_duration_in_seconds > 0 {
                                        (listen_duration as f64 / episode_duration_in_seconds as f64) * 100.0
                                    } else {
                                        0.0
                                    };
                                    html! {
                                        <div class="flex items-center space-x-2">
                                            <span class="item_container-text">{ format_time(listen_duration as f64) }</span>
                                            <div class="progress-bar-container">
                                                <div class="progress-bar" style={ format!("width: {}%;", listen_duration_percentage) }></div>
                                            </div>
                                            <span class="item_container-text">{ formatted_duration }</span>
                                        </div>
                                    }
                                } else {
                                    html! {
                                        <span class="item_container-text">{ formatted_duration }</span>
                                    }
                                }
                            }
                        }

                    </div>
                    {
                        html! {
                            <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                                if should_show_buttons {
                                    <button
                                        class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                        onclick={on_play_pause}
                                    >
                                        {
                                            if is_current_episode && is_playing {
                                                html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                                            } else {
                                                html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                                            }
                                        }
                                    </button>
                                    {
                                        if props.podcast_added {
                                            let page_type = "episode_layout".to_string();
                                            html! {
                                                <div class="hidden sm:block">
                                                    <ContextButton episode={boxed_episode} page_type={page_type.clone()} />
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                }
                            </div>
                        }
                    }
                </div>
                </>
            }
        })
        .collect::<Html>();

    let total_height = props.episodes.len() as f64 * *item_height;
    let offset_y = start_index as f64 * *item_height;

    html! {
        <>
        <div
            ref={container_ref}
            class="virtual-list-container flex-grow overflow-y-auto"
            style="height: calc(100vh - 100px);"
        >
            <div style={format!("height: {}px; position: relative;", total_height)}>
                <div style={format!("position: absolute; top: {}px; left: 0; right: 0;", offset_y)}>
                    { visible_episodes }
                </div>
            </div>
        </div>
        {
            if let Some(index) = *selected_episode_index {
                let episode = &props.episodes[index];
                let sanitized_description = sanitize_html_with_blank_target(&episode.description.clone().unwrap_or_default());
                let description = sanitized_description;
                let date_format = match_date_format(props.search_state.date_format.as_deref());
                let datetime = parse_date(&episode.pub_date.clone().unwrap_or_default(), &props.search_state.user_tz);
                let format_release = format_datetime(&datetime, &props.search_state.hour_preference, date_format);

                // Create the callback here where we have access to index
                let modal_shownotes_callback = {
                    let history = props.history.clone();
                    let search_dispatch = props.search_dispatch.clone();
                    let podcast_link = props.podcast_link.clone();
                    let podcast_title = props.podcast_title.clone();
                    let episode_id = episode.episode_id.unwrap_or(0);
                    let episode_url = episode.enclosure_url.clone().unwrap_or_default();
                    let is_youtube = episode.is_youtube.clone();

                    Callback::from(move |_: MouseEvent| {
                        on_shownotes_click(
                            history.clone(),
                            search_dispatch.clone(),
                            Some(episode_id),
                            Some(podcast_link.clone()),
                            Some(episode_url.clone()),
                            Some(podcast_title.clone()),
                            true,
                            None,
                            is_youtube,
                        ).emit(MouseEvent::new("click").unwrap());
                    })
                };

                html! {
                    <EpisodeModal
                        episode_id={episode.episode_id.unwrap_or(0)}
                        episode_url={episode.enclosure_url.clone().unwrap()}
                        episode_artwork={episode.artwork.clone().unwrap_or_default()}
                        episode_title={episode.title.clone().unwrap_or_default()}
                        description={description}
                        format_release={format_release}
                        duration={episode.duration.clone().unwrap_or_default().parse().unwrap_or(0) as i32}
                        on_close={on_modal_close.clone()}
                        on_show_notes={modal_shownotes_callback}
                        listen_duration_percentage={0.0}
                        is_youtube={episode.is_youtube.unwrap()}
                    />
                }
            } else {
                html! {}
            }
        }
        </>
    }
}
