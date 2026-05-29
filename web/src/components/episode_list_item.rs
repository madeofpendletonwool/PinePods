use crate::components::audio::on_play_click;
use crate::components::context::{AppState, EpisodeStatusState, UIState};
use crate::components::gen_components::{on_shownotes_click, EpisodeModal, FallbackImage};

use crate::components::context_menu_button::{ContextMenuButton, PageType};
use crate::components::gen_funcs::{
    format_datetime, match_date_format, parse_date, sanitize_html_with_blank_target, use_long_press,
};
use crate::components::gen_funcs::{format_time, strip_images_from_html};
use crate::components::safehtml::SafeHtml;
use crate::components::virtual_list::DragCallbacks;
use crate::requests::episode::Episode;
use i18nrs::yew::use_translation;
use yew_router::history::{BrowserHistory, History};
use wasm_bindgen::prelude::*;
use web_sys::MouseEvent;
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
    #[prop_or_default]
    pub is_selected: Option<bool>,
    #[prop_or_default]
    pub on_select_above: Callback<i32>,
    #[prop_or_default]
    pub on_select_below: Callback<i32>,
}

#[function_component(EpisodeListItem)]
pub fn episode_list_item(props: &EpisodeListItemProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_completed = i18n.t("episode_list_item.completed").to_string();
    // Use selective subscriptions to only re-render when relevant state changes
    let episode_id = props.episode.episodeid;

    // Only subscribe to the specific fields we need for RENDERING
    let is_completed = use_selector(move |state: &EpisodeStatusState| {
        state.completed_episodes.contains(&episode_id)
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
    let is_downloaded_server = use_selector(move |state: &EpisodeStatusState| {
        state.downloaded_episodes.is_server_download(episode_id)
    });
    let is_downloaded_local = use_selector(move |state: &EpisodeStatusState| {
        state.downloaded_episodes.is_local_download(episode_id)
    });

    // Selector returns (is_current, is_active_and_playing, is_loading) for THIS episode only.
    // Only this episode re-renders when its own play state changes; other episodes are unaffected.
    let play_state = use_selector(move |state: &UIState| {
        let is_current = state
            .currently_playing
            .as_ref()
            .map_or(false, |cp| cp.episode_id == episode_id);
        let is_loading = state.loading_episode_id == Some(episode_id);
        (is_current, is_current && state.audio_playing.unwrap_or(false), is_loading)
    });
    let (is_current_episode, is_active_and_playing, is_loading) = *play_state;

    /*
    Item Shape
    */
    let is_narrow_viewport = *use_memo((), |_| {
        web_sys::window().expect("no global window exists")
            .inner_width().unwrap().as_f64().unwrap() < 500.0
    });

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
    // is_current_episode and is_active_and_playing come from the play_state selector above

    let formatted_pub_date = use_memo(
        (
            props.episode.episodepubdate.clone(),
            (*user_tz).clone(),
            (*date_format).clone(),
            *hour_preference,
        ),
        |(pubdate, tz, fmt, hour)| {
            let date_fmt = match_date_format(fmt.as_deref());
            let datetime = parse_date(pubdate, tz);
            format_datetime(&datetime, hour, date_fmt)
        },
    );

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

    let on_play_pause = {
        let episode = props.episode.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let is_local = is_local;

        Callback::from(move |e: MouseEvent| {
            let audio_dispatch = Dispatch::<UIState>::global();
            let current_state = audio_dispatch.get();
            let is_current = current_state
                .currently_playing
                .as_ref()
                .map_or(false, |cp| cp.episode_id == episode.episodeid);
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
                let episode_id = episode.episodeid;
                audio_dispatch.reduce_mut(move |state| {
                    state.loading_episode_id = Some(episode_id);
                });
                on_play_click(
                    episode.clone(),
                    api_key.clone(),
                    user_id,
                    server_name.clone(),
                    audio_dispatch.clone(),
                    current_state.clone(),
                    is_local,
                    false,
                    None,
                )
                .emit(e);
            }
        })
    };

    /*
    Episode Information
    */

    let episode_description = use_memo(
        props.episode.episodedescription.clone(),
        |desc| sanitize_html_with_blank_target(desc),
    );

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
    // Episodes with episodeid <= 0 come from external feeds (unsubscribed podcasts).
    // Only show the context menu for episodes the user can actually interact with.
    let can_use_context_menu = props.episode.episodeid > 0;

    let show_context_menu = use_state(|| false);
    let context_menu_position = use_state(|| (0, 0));

    // Long press handler - simulate clicking the context button
    let context_button_ref = use_node_ref();
    let on_long_press = {
        let context_button_ref = context_button_ref.clone();
        let show_context_menu = show_context_menu.clone();
        let context_menu_position = context_menu_position.clone();

        Callback::from(move |event: TouchEvent| {
            if !can_use_context_menu { return; }
            if let Some(touch) = event.touches().get(0) {
                event.prevent_default();
                // Record position for the context menu
                context_menu_position.set((touch.client_x(), touch.client_y()));

                // Find and click the context button (if it exists)
                if let Some(button) = context_button_ref.cast::<web_sys::HtmlElement>() {
                    button.click();
                } else {
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
            if **is_pressed && can_use_context_menu {
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
            Dispatch::<AppState>::global(),
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
                    "ep-row",
                    if *is_pressing_state { "ep-row--pressing" } else { "" },
                    if props.is_delete_mode { "ep-row--select" } else { "" }
                )}
                style={format!("user-select: {};",
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
                    if props.drag_callbacks.draggable() {
                        html!{
                            <div class="drag-handle-wrapper flex items-center justify-center w-10 h-full touch-none">
                                <button class="drag-handle cursor-grab">
                                    <i class="ph ph-dots-six-vertical"></i>
                                </button>
                            </div>
                        }
                    } else {
                        html!{ }
                    }
                }

                {
                    if props.is_delete_mode {
                        let is_checked = props.is_selected.unwrap_or(*selected_for_deletion);
                        let ep_id = props.episode.episodeid;
                        let on_above = props.on_select_above.reform(move |_: MouseEvent| ep_id);
                        let on_below = props.on_select_below.reform(move |_: MouseEvent| ep_id);
                        html! {
                            <div class="ep-select-col">
                                <button class="ep-range-btn" onclick={on_above} title="Select all above">
                                    <i class="ph ph-caret-up"></i>
                                </button>
                                <input
                                    type="checkbox"
                                    checked={is_checked}
                                    class="podcast-dropdown-checkbox h-5 w-5 rounded border-2 cursor-pointer"
                                    onchange={props.on_checkbox_change.reform(move |_| checkbox_ep)}
                                />
                                <button class="ep-range-btn" onclick={on_below} title="Select all below">
                                    <i class="ph ph-caret-down"></i>
                                </button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                <div class="ep-art-wrap">
                    <FallbackImage
                        src={props.episode.episodeartwork.clone()}
                        alt={format!("Cover for {}", props.episode.episodetitle)}
                        class="ep-art-img"
                    />
                    if *is_completed {
                        <div class="ep-art-badge"><i class="ph ph-check-circle"></i></div>
                    }
                </div>

                <div class="ep-body">
                    <div class="ep-title cursor-pointer" onclick={on_shownotes_click.clone()}>
                        { props.episode.episodetitle.clone() }
                    </div>
                    <hr class="ep-divider" />
                    <div class="ep-desc cursor-pointer"
                        onclick={
                            let episode_id = props.episode.episodeid;
                            let omo = on_modal_open.clone();
                            Callback::from(move |e: MouseEvent| {
                                e.prevent_default();
                                omo.emit(episode_id);
                            })
                        }
                    >
                        <SafeHtml html={strip_images_from_html(&*episode_description)} />
                    </div>
                    <div class="ep-meta">
                        <span>{ (*formatted_pub_date).clone() }</span>
                        {
                            if *is_completed {
                                if is_narrow_viewport {
                                    html! { <span>{ &i18n_completed }</span> }
                                } else {
                                    html! { <span>{ format!("{} \u{2014} {}", episode_duration_str, &i18n_completed) }</span> }
                                }
                            } else if props.episode.listenduration > 0 {
                                html! {
                                    <>
                                        <div class="ep-progress">
                                            <div class="ep-progress-fill" style={ format!("width: {}%;", listen_duration_percentage) }></div>
                                        </div>
                                        <span>{ episode_duration_str }</span>
                                    </>
                                }
                            } else {
                                html! { <span>{ episode_duration_str }</span> }
                            }
                        }
                    </div>
                </div>

                if !props.episode.episodeurl.is_empty() {
                    <div class="ep-actions">
                        <button
                            class="ico"
                            onclick={on_play_pause.clone()}
                            title="Play / Pause"
                        >
                            {
                                if is_loading {
                                    html! { <i class="ph ph-spinner" style="animation: spin 1s linear infinite;"></i> }
                                } else if is_active_and_playing {
                                    html! { <i class="ph ph-pause"></i> }
                                } else {
                                    html! { <i class="ph ph-play"></i> }
                                }
                            }
                        </button>
                        if can_use_context_menu {
                            <div ref={context_button_ref.clone()}>
                                <ContextMenuButton episode={props.episode.clone()} page_type={props.page_type.clone()} />
                            </div>
                        }
                    </div>
                }
            </div>

            {
                if *show_context_menu && can_use_context_menu {
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
                    description={(*episode_description).clone()}
                    format_release={(*formatted_pub_date).clone()}
                    duration={props.episode.episodeduration}
                    on_close={on_modal_close.clone()}
                    on_show_notes={on_shownotes_click.clone()}
                    listen_duration_percentage={listen_duration_percentage}
                    is_youtube={props.episode.is_youtube}
                    is_video={props.episode.is_video}
                />
            }
        </div>
    }
}
