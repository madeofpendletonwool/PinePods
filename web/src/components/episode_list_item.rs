use crate::components::audio::on_play_click;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::gen_components::{on_shownotes_click, EpisodeModal, FallbackImage};

use crate::components::context_menu_button::{ContextMenuButton, PageType};
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target, use_long_press,
};
use crate::components::gen_funcs::{format_time, strip_images_from_html};
use crate::components::safehtml::SafeHtml;
use crate::components::virtual_list::DragCallbacks;
use crate::requests::episode::Episode;
use yew_router::history::{BrowserHistory, History};
use gloo_events::EventListener;
use wasm_bindgen::prelude::*;
use web_sys::{window, MouseEvent};
use yew::prelude::*;
use yew::Callback;
use yewdux::prelude::*;

#[allow(dead_code)]
#[derive(Properties, PartialEq, Clone)]
pub struct EpisodeListItemProps {
    pub episode: Episode,
    // pub _is_expanded: bool,
    // pub toggle_expanded: Callback<MouseEvent>,
    #[prop_or(PageType::Default)]
    pub page_type: PageType,
    #[prop_or_default]
    pub on_checkbox_change: Callback<i32>,
    #[prop_or_default]
    pub drag_callbacks: DragCallbacks,
    #[prop_or_default]
    pub is_delete_mode: bool,
}

#[function_component(EpisodeListItem)]
pub fn episode_list_item(props: &EpisodeListItemProps) -> Html {
    // Use selective subscriptions to only re-render when relevant state changes
    let episode_id = props.episode.episodeid;

    // Only subscribe to the specific fields we need for RENDERING
    let is_completed = use_selector(move |state: &AppState| {
        state.completed_episodes
            .as_ref()
            .unwrap_or(&vec![])
            .contains(&episode_id)
    });
    let auth_details = use_selector(|state: &AppState| state.auth_details.clone());
    let user_details = use_selector(|state: &AppState| state.user_details.clone());
    let date_format = use_selector(|state: &AppState| state.date_format.clone());
    let user_tz = use_selector(|state: &AppState| state.user_tz.clone());
    let hour_preference = use_selector(|state: &AppState| state.hour_preference);
    let selected_for_deletion = use_selector(move |state: &AppState| {
        state.selected_episodes_for_deletion.contains(&episode_id)
    });
    let podcast_added = use_selector(|state: &AppState| state.podcast_added);
    let is_downloaded_server = use_selector(move |state: &AppState| {
        state.downloaded_episodes.is_server_download(episode_id)
    });
    let is_downloaded_local = use_selector(move |state: &AppState| {
        state.downloaded_episodes.is_local_download(episode_id)
    });

    // We still need the dispatcher for actions
    let (_app_state, app_dispatch) = use_store::<AppState>();

    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();

    // DEBUG: Log re-renders to confirm the fix works
    web_sys::console::log_1(&format!("EpisodeListItem render: {}", props.episode.episodetitle).into());

    /*
    Item Shape
    */
    let container_height: UseStateHandle<String> = use_state(|| "221px".to_string()); // Should be em?

    let is_narrow_viewport = {
        let window = web_sys::window().expect("no global window exists");
        window.inner_width().unwrap().as_f64().unwrap() < 500.0
    };

    // resize evt listener
    {
        let container_height = container_height.clone();
        use_effect_with((), move |_| {
            let update_height = {
                let container_height = container_height.clone();
                Callback::from(move |_| {
                    if let Some(window) = window() {
                        if let Ok(width) = window.inner_width() {
                            if let Some(width) = width.as_f64() {
                                let new_height = if width <= 530.0 {
                                    "122px"
                                } else if width <= 768.0 {
                                    "150px"
                                } else {
                                    "221px"
                                };
                                container_height.set(new_height.to_string());
                            }
                        }
                    }
                })
            };

            update_height.emit(());

            let listener = EventListener::new(&window().unwrap(), "resize", move |_| {
                update_height.emit(());
            });

            move || drop(listener)
        });
    }

    // let desc_expanded = desc_state
    //     .expanded_descriptions
    //     .contains(&props.episode.episodeid.to_string());

    // #[wasm_bindgen]
    // extern "C" {
    //     #[wasm_bindgen(js_namespace = window)]
    //     fn toggleDescription(guid: &str, expanded: bool);
    // }
    // let toggle_expanded = {
    //     let desc_dispatch = desc_dispatch.clone();
    //     let episode_guid = props.episode.episodeid.clone().to_string();

    //     Callback::from(move |_: MouseEvent| {
    //         let guid = episode_guid.clone();
    //         desc_dispatch.reduce_mut(move |state| {
    //             if state.expanded_descriptions.contains(&guid) {
    //                 state.expanded_descriptions.remove(&guid);
    //                 toggleDescription(&guid, false);
    //             } else {
    //                 state.expanded_descriptions.insert(guid.clone());
    //                 toggleDescription(&guid, true);
    //             }
    //         });
    //     })
    // };

    /*
    Modal interactions
    */
    let show_modal = use_state(|| false);

    let on_modal_open = {
        let show_modal = show_modal.clone();
        Callback::from(move |_: i32| show_modal.set(true))
    };

    let on_modal_close: Callback<MouseEvent> = {
        let show_modal = show_modal.clone();
        Callback::from(move |_: MouseEvent| show_modal.set(false))
    };

    /*
    Audio Player
    */
    let is_current_episode = audio_state
        .currently_playing
        .as_ref()
        .map_or(false, |current| {
            current.episode_id == props.episode.episodeid
        });

    let is_playing = audio_state.audio_playing.unwrap_or(false);

    let formatted_pub_date = {
        let date_format = match_date_format(date_format.as_deref());
        let datetime = parse_date(&props.episode.episodepubdate, &user_tz);
        format_datetime(&datetime, &hour_preference, date_format)
    };

    let api_key = auth_details
        .as_ref()
        .as_ref()
        .map(|ud| ud.api_key.clone().unwrap())
        .unwrap();
    let user_id = user_details
        .as_ref()
        .as_ref()
        .map(|ud| ud.UserID.clone())
        .unwrap();
    let server_name = auth_details
        .as_ref()
        .as_ref()
        .map(|ud| ud.server_name.clone())
        .unwrap();

    // Compute is_local inline instead of passing it via app_state
    let is_local = if podcast_added.unwrap_or(false) && props.episode.episodeid != 0 {
        *is_downloaded_server || {
            #[cfg(not(feature = "server_build"))]
            {
                *is_downloaded_local
            }
            #[cfg(feature = "server_build")]
            {
                false
            }
        }
    } else {
        false
    };

    // Inline on_play_pause logic to avoid needing app_state
    let on_play_pause = {
        let episode = props.episode.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let audio_dispatch = audio_dispatch.clone();
        let audio_state = audio_state.clone();
        let is_local = is_local;

        Callback::from(move |e: MouseEvent| {
            let is_current = audio_state
                .currently_playing
                .as_ref()
                .map_or(false, |current| current.episode_id == episode.episodeid);
            if is_current {
                audio_dispatch.reduce_mut(|state| {
                    let currently_playing = state.audio_playing.unwrap_or(false);
                    state.audio_playing = Some(!currently_playing);
                    if let Some(audio) = &state.audio_element {
                        if currently_playing {
                            let _ = audio.pause();
                        } else {
                            let _ = audio.play();
                        }
                    }
                });
            } else {
                on_play_click(
                    episode.clone(),
                    api_key.clone(),
                    user_id,
                    server_name.clone(),
                    audio_dispatch.clone(),
                    audio_state.clone(),
                    is_local,
                )
                .emit(e);
            }
        })
    };

    /*
    Episode Information
    */

    let episode_description = sanitize_html_with_blank_target(&props.episode.episodedescription);

    let (_listen_duration_str, listen_duration_percentage) = {
        let lds = format_time(props.episode.listenduration);
        let ldp = if props.episode.listenduration > 0 {
            ((props.episode.listenduration * 100) / props.episode.episodeduration).min(100)
        } else {
            0
        };
        (lds, ldp)
    };

    let episode_duration_str = format_time(props.episode.episodeduration);

    // is_completed is already defined via use_selector above

    let checkbox_ep = props.episode.episodeid;

    // Handle context menu position
    // let context_menu_style = if props.show_context_menu {
    //     format!(
    //         "position: fixed; top: {}px; left: {}px; z-index: 1000;",
    //         props.context_menu_position.1, props.context_menu_position.0
    //     )
    // } else {
    //     String::new()
    // };

    /*
    Set up Context Menu
    */
    let show_context_menu = use_state(|| false);
    let context_menu_position = use_state(|| (0, 0));

    // Long press handler - simulate clicking the context button
    let context_button_ref = use_node_ref();
    let on_long_press = {
        let context_button_ref = context_button_ref.clone();
        let show_context_menu = show_context_menu.clone();
        let context_menu_position = context_menu_position.clone();

        Callback::from(move |event: TouchEvent| {
            if let Some(touch) = event.touches().get(0) {
                event.prevent_default();
                // Record position for the context menu
                context_menu_position.set((touch.client_x(), touch.client_y()));

                // Find and click the context button (if it exists)
                if let Some(button) = context_button_ref.cast::<web_sys::HtmlElement>() {
                    button.click();
                } else {
                    // If the button doesn't exist (maybe on mobile where it's hidden)
                    // we'll just set our state to show the menu
                    show_context_menu.set(true);
                }
            }
        })
    };

    // Close context menu callback
    let close_context_menu: Callback<()> = {
        let show_context_menu = show_context_menu.clone();
        Callback::from(move |_| {
            show_context_menu.set(false);
        })
    };

    // Setup long press detection
    let (on_touch_start, on_touch_end, on_touch_move, is_long_press_state, is_pressing_state) =
        use_long_press(on_long_press, Some(600)); // 600ms for long press

    // When long press is detected through the hook, update our state
    {
        let show_context_menu = show_context_menu.clone();
        use_effect_with(is_long_press_state, move |is_pressed| {
            if **is_pressed {
                show_context_menu.set(true);
            }
            || ()
        });
    }

    /*
    Show-Notes
    */
    let browser_history = BrowserHistory::new();
    let on_shownotes_click = {
        let is_local_for_shownotes = *is_downloaded_local;
        let src = if props.episode.episodeurl.contains("youtube.com") {
            format!(
                "{}/api/data/stream/{}?api_key={}&user_id={}&type=youtube",
                server_name, props.episode.episodeid, api_key, user_id
            )
        } else if is_local_for_shownotes {
            format!(
                "{}/api/data/stream/{}?api_key={}&user_id={}",
                server_name, props.episode.episodeid, api_key, user_id
            )
        } else {
            props.episode.episodeurl.clone()
        };

        on_shownotes_click(
            browser_history.clone(),
            app_dispatch.clone(),
            props.episode.episodeid.clone(),
            props.episode.feedurl.clone(),
            src,
            props.episode.episodetitle.clone(),
            true,
            false,
            props.episode.is_youtube,
        )
    };

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggleDescription(guid: &str, expanded: bool);
    }

    html! {
        <div>
            <div
                class={classes!(
                    "item-container", "border-solid", "border", "flex", "items-start", "mb-4",
                    "shadow-md", "rounded-lg", "touch-manipulation", "transition-all", "duration-150",
                    if *is_pressing_state {
                        "bg-accent-color bg-opacity-20 transform scale-[0.98]"
                    } else {
                        ""
                    }
                )}
                style={format!("height: {}; overflow: hidden; user-select: {};",
                    *container_height,
                    if *is_pressing_state { "none" } else { "auto" }
                )}
                ontouchstart={ on_touch_start.clone() }
                ontouchend={ on_touch_end.clone() }
                ontouchmove={ on_touch_move.clone() }

                draggable={ props.drag_callbacks.draggable().to_string() }
                ondragstart={ props.drag_callbacks.ondragstart.clone() }
                ondragenter={ props.drag_callbacks.ondragenter.clone() }
                ondragover={ props.drag_callbacks.ondragover.clone() }
                ondrop={ props.drag_callbacks.ondrop.clone() }

                data-id={ props.episode.episodeid.to_string() }
            >

                {
                    if props.drag_callbacks.draggable()
                    {
                        html!{
                            <div class="drag-handle-wrapper flex items-center justify-center w-10 h-full touch-none">
                                <button class="drag-handle cursor-grab">
                                    <i class="ph ph-dots-six-vertical text-2xl"></i>
                                </button>
                            </div>
                        }
                    } else {
                        html!{ }
                    }
                }

                {
                    if props.is_delete_mode {
                    html! {
                        <div class="flex items-center pl-4">
                            <input
                                type="checkbox"
                                checked={*selected_for_deletion}
                                class="podcast-dropdown-checkbox h-5 w-5 rounded border-2 text-primary focus:ring-primary focus:ring-offset-0 cursor-pointer appearance-none checked:bg-primary checked:border-primary"
                                onchange={props.on_checkbox_change.reform(move |_| checkbox_ep)}
                            />
                        </div>
                    }
                } else {
                    html! {}
                    }
                }

                <div class="flex flex-col w-auto object-cover pl-4">
                    <FallbackImage
                        src={props.episode.episodeartwork.clone()}
                        alt={format!("Cover for {}", props.episode.episodetitle)}
                        class="episode-image"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12 self-start">
                    <div class="flex items-center space-x-2 cursor-pointer" onclick={on_shownotes_click.clone()}>
                    <p class="item_container-text episode-title font-semibold line-clamp-2">
                        {props.episode.episodetitle.clone()}
                    </p>
                    {
                        if *is_completed {
                            html! {
                                <i class="ph ph-check-circle text-2xl text-green-500"></i>
                            }
                        } else {
                            html! {}
                        }
                    }
                    </div>
                    <hr class="my-2 border-t hidden md:block"/>
                    {
                        html! {
                            <div class="item-description-text cursor-pointer hidden md:block"
                                onclick={let episode_id = props.episode.episodeid;
                                        let omo = on_modal_open.clone();
                                        Callback::from(move |e: MouseEvent| {
                                            e.prevent_default();
                                            omo.emit(episode_id);
                                        })}>
                                <div class="item_container-text line-clamp-2">
                                    <SafeHtml html={strip_images_from_html(&episode_description)} />
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
                            { formatted_pub_date.clone() }
                        </span>
                    </div>
                    {
                        if *is_completed {
                            if is_narrow_viewport {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{"Completed"}</span>
                                    </div>
                                }
                            } else {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{ episode_duration_str }</span>
                                        <span class="item_container-text">{ "- Completed" }</span>
                                    </div>
                                }
                            }
                        } else {
                            if props.episode.listenduration > 0 {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        {
                                            if !is_narrow_viewport {
                                                html! {
                                                    <span class="item_container-text">{ format_time(props.episode.listenduration) }</span>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        <div class="progress-bar-container">
                                            <div class="progress-bar" style={ format!("width: {}%;", listen_duration_percentage) }></div>
                                        </div>
                                        <span class="item_container-text">{ episode_duration_str }</span>
                                    </div>
                                }
                            } else {
                                html! {
                                    <span class="item_container-text">{ episode_duration_str }</span>
                                }
                            }
                        }
                    }
                </div>
                {
                    html! {
                        <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                            // only show links if there is a url to link to
                            if !props.episode.episodeurl.is_empty() {
                                <button
                                    class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                    onclick={on_play_pause.clone()}
                                >
                                    {
                                        if is_current_episode && is_playing {
                                            html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                                        } else {
                                            html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                                        }
                                    }
                                </button>

                                <div class="hidden sm:block"> // Standard desktop context button
                                    <div ref={context_button_ref.clone()}>
                                        <ContextMenuButton episode={props.episode.clone()} page_type={props.page_type.clone()} />
                                    </div>
                                </div>
                            }
                        </div>
                    }
                }
            </div>

            // This shows the context menu via long press
            {
                if *show_context_menu {
                    html! {
                        <ContextMenuButton
                            episode={props.episode.clone()}
                            page_type={props.page_type.clone()}
                            show_menu_only={true}
                            position={Some(*context_menu_position)}
                            on_close={close_context_menu.clone()}
                        />
                    }
                } else {
                    html! {}
                }
            }

            if *show_modal {
                <EpisodeModal
                    episode_id={props.episode.episodeid}
                    episode_url={props.episode.episodeurl.clone()}
                    episode_artwork={props.episode.episodeartwork.clone()}
                    episode_title={props.episode.episodetitle.clone()}
                    description={episode_description.clone()}
                    format_release={formatted_pub_date.to_string()}
                    duration={props.episode.episodeduration}
                    on_close={on_modal_close.clone()}
                    on_show_notes={on_shownotes_click.clone()}
                    listen_duration_percentage={listen_duration_percentage}
                    is_youtube={props.episode.is_youtube}
                />
            }
        </div>
    }
}
