use super::gen_components::{on_shownotes_click, ContextButton, EpisodeModal, EpisodeTrait};
use super::gen_funcs::{format_datetime, match_date_format, parse_date};
use crate::components::audio::on_play_click;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::{AppStateMsg, SafeHtml};
use crate::components::gen_funcs::format_time;
use crate::components::gen_funcs::{
    convert_time_to_seconds, sanitize_html_with_blank_target, truncate_description,
};
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
    let show_modal = use_state(|| false);
    let show_clonedal = show_modal.clone();
    let show_clonedal2 = show_modal.clone();
    let on_modal_open = Callback::from(move |_: MouseEvent| show_clonedal.set(true));

    let on_modal_close = Callback::from(move |_: MouseEvent| show_clonedal2.set(false));

    // Effect to set initial container height and listen for window resize
    {
        let container_height = container_height.clone();
        use_effect_with((), move |_| {
            let window = window().expect("no global `window` exists");
            let window_clone = window.clone();

            let update_height = Callback::from(move |_| {
                let height = window_clone.inner_height().unwrap().as_f64().unwrap();
                container_height.set(height - 100.0); // Adjust 100 based on your layout
            });

            update_height.emit(());

            let listener = EventListener::new(&window, "resize", move |_| {
                update_height.emit(());
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

    let start_index = (*scroll_pos / props.item_height).floor() as usize;
    let end_index = (((*scroll_pos + *container_height) / props.item_height).ceil() as usize)
        .min(props.episodes.len());

    let visible_episodes = (start_index..end_index)
        .map(|index| {
            let episode = &props.episodes[index];
            let history_clone = props.history.clone();
            let dispatch = props.dispatch.clone();
            let search_dispatch = props.search_dispatch.clone();
            let search_state_clone = props.search_state.clone();
            let search_ui_state_clone = props.search_ui_state.clone();

            let episode_url_clone = episode.enclosure_url.clone().unwrap_or_default();
            let episode_title_clone = episode.title.clone().unwrap_or_default();
            let episode_artwork_clone = episode.artwork.clone().unwrap_or_default();
            let episode_duration_clone = episode.duration.clone().unwrap_or_default();
            let episode_duration_in_seconds = match convert_time_to_seconds(&episode_duration_clone) {
                Ok(seconds) => seconds as i32,
                Err(e) => {
                    eprintln!("Failed to convert time to seconds: {}", e);
                    0
                }
            };
            let episode_id_clone = episode.episode_id.unwrap_or(0);
            let db_added = episode_id_clone != 0;

            let episode_id_shownotes = episode_id_clone.clone();
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

            let search_state_toggle = search_state_clone.clone();
            let toggle_expanded = {
                let search_dispatch_clone = search_dispatch.clone();
                let episode_guid = episode.guid.clone().unwrap();
                Callback::from(move |_: MouseEvent| {
                    let guid_clone = episode_guid.clone();
                    let search_dispatch_call = search_dispatch_clone.clone();

                    if search_state_toggle.expanded_descriptions.contains(&guid_clone) {
                        search_dispatch_call.apply(AppStateMsg::CollapseEpisode(guid_clone));
                    } else {
                        search_dispatch_call.apply(AppStateMsg::ExpandEpisode(guid_clone));
                    }
                })
            };

            let on_play_click = on_play_click(
                episode_url_clone.clone(),
                episode_title_clone.clone(),
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
            );

            let description_class = if is_expanded {
                "desc-expanded".to_string()
            } else {
                "desc-collapsed".to_string()
            };

            let date_format = match_date_format(search_state_clone.date_format.as_deref());
            let datetime = parse_date(&episode.pub_date.clone().unwrap_or_default(), &search_state_clone.user_tz);
            let format_release = format!("{}", format_datetime(&datetime, &search_state_clone.hour_preference, date_format));
            let boxed_episode = Box::new(episode.clone()) as Box<dyn EpisodeTrait>;
            let formatted_duration = format_time(episode_duration_in_seconds.into());

            let episode_url_for_ep_item = episode_url_clone.clone();
            let shownotes_episode_url = episode_url_clone.clone();
            let should_show_buttons = !episode_url_for_ep_item.is_empty();
            let make_shownotes_callback = {
                let history = history_clone.clone();
                let search_dispatch = search_dispatch.clone();
                let podcast_link = props.podcast_link.clone();
                let podcast_title = props.podcast_title.clone();
                let episode_id = episode_id_clone;
                let episode_url = episode_url_clone.clone();

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
                    ).emit(MouseEvent::new("click").unwrap());
                })
            };

            html! {
                <div class="item-container flex items-center mb-4 shadow-md rounded-lg">
                    <img
                        src={episode.artwork.clone().unwrap_or_default()}
                        alt={format!("Cover for {}", &episode.title.clone().unwrap_or_default())}
                        class="episode-image"/>
                    <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                        <p class="item_container-text episode-title font-semibold" onclick={on_shownotes_click(history_clone.clone(), search_dispatch.clone(), Some(episode_id_shownotes), Some(props.podcast_link.clone()), Some(shownotes_episode_url), Some(props.podcast_title.clone()), db_added, None)}>{ &episode.title.clone().unwrap_or_default() }</p>
                        {
                            html! {
                                <div class="item-description-text cursor-pointer md:block"
                                        onclick={on_modal_open.clone()}>
                                    <div class="item_container-text line-clamp-2">
                                        <SafeHtml html={description.clone()} />
                                    </div>
                                </div>
                            }
                        }
                        <span class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2">
                            <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                            </svg>
                            { format_release.clone() }
                        </span>
                        {
                            html! {
                                <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                            }
                        }
                    </div>
                    {
                        html! {
                            <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                                if should_show_buttons {
                                    <button
                                        class="item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                        onclick={on_play_click}
                                    >
                                    <span class="material-bonus-color material-icons large-material-icons md:text-6xl text-4xl">{"play_arrow"}</span>
                                    </button>
                                    {
                                        if props.podcast_added {
                                            let page_type = "episode_layout".to_string();
                                            html! {
                                                <ContextButton episode={boxed_episode} page_type={page_type.clone()} />
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                }
                            </div>
                        }
                    }
                    if *show_modal {
                        <EpisodeModal
                            episode_id={episode.episode_id.unwrap_or(0)}
                            episode_artwork={episode.artwork.clone().unwrap_or_default()}
                            episode_title={episode.title.clone().unwrap_or_default()}
                            description={description.clone()}
                            format_release={format_release.to_string()}
                            duration={formatted_duration}
                            on_close={on_modal_close.clone()}
                            on_show_notes={make_shownotes_callback}
                            listen_duration_percentage={0.0}
                        />
                    }
                </div>
            }
        })
        .collect::<Html>();

    let total_height = props.episodes.len() as f64 * props.item_height;
    let offset_y = start_index as f64 * props.item_height;

    html! {
        <div ref={container_ref} style={format!("height: {}px; overflow-y: auto;", *container_height)}>
            <div style={format!("height: {}px; position: relative;", total_height)}>
                <div style={format!("position: absolute; top: {}px; left: 0; right: 0;", offset_y)}>
                    { visible_episodes }
                </div>
            </div>
        </div>
    }
}
