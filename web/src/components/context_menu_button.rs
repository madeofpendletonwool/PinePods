use crate::components::context::{AppState, CollectionModalState, ContextMenuState, EpisodeStatusState, NotificationState};
#[cfg(not(feature = "server_build"))]
use crate::components::context::UIState;
#[cfg(not(feature = "server_build"))]
use crate::pages::downloads_tauri::{
    download_file, remove_episode_from_local_db, update_local_database, update_podcast_database,
};
use crate::requests::episode::Episode;

use crate::components::gen_funcs::format_error_message;
use crate::requests::pod_req::{
    call_add_episode_to_collection, call_download_episode, call_get_collection_add_ui,
    call_get_collections, call_get_episode_collections, call_mark_episode_completed,
    call_mark_episode_uncompleted, call_queue_episode, call_remove_downloaded_episode,
    call_remove_episode_from_collection, call_remove_queued_episode, call_remove_saved_episode,
    call_save_episode, Collection, CollectionEpisodeRequest, DownloadEpisodeRequest,
    MarkEpisodeCompletedRequest, QueuePodcastRequest, SavePodcastRequest,
};
use std::collections::HashSet;
#[cfg(not(feature = "server_build"))]
use crate::requests::pod_req::{
    call_get_episode_metadata, call_get_podcast_details, EpisodeRequest,
};
use gloo_events::{EventListener, EventListenerOptions};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlElement;
use i18nrs::yew::use_translation;
use web_sys::{window, MouseEvent, TouchEvent};
use yew::prelude::*;
use yew::Callback;
use yewdux::prelude::*;

/// Specific page types for unique ctx menu implementations
#[derive(Clone, PartialEq)]
pub enum PageType {
    Saved,
    Queue,
    Downloads,
    LocalDownloads,
    Default,
}

thread_local! {
    /// Monotonic counter used to give each ContextMenuButton a unique id so the
    /// global ContextMenuState can track which single menu is open.
    static CTX_MENU_SEQ: std::cell::Cell<u64> = std::cell::Cell::new(0);
}

/// Compute the fixed-position placement `(top, right_edge)` for the dropdown,
/// anchored to the button's bottom-right. When the menu height is known and it
/// would overflow the bottom of the viewport (with more room above than below),
/// it flips to open upward. `right_edge` is the button's right edge; the caller
/// renders it as `right: calc(100vw - {right_edge}px)`.
fn compute_menu_placement(button: &HtmlElement, menu: Option<&HtmlElement>) -> (i32, i32) {
    let btn = button.get_bounding_client_rect();
    let right_edge = btn.right();
    let viewport_h = window()
        .and_then(|w| w.inner_height().ok())
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let menu_h = menu
        .map(|m| m.get_bounding_client_rect().height())
        .unwrap_or(0.0);

    let space_below = viewport_h - btn.bottom();
    let space_above = btn.top();

    let top = if menu_h > 0.0 && menu_h + 4.0 > space_below && space_above > space_below {
        // Flip up: anchor the menu's bottom just above the button, clamped to the top edge.
        (btn.top() - menu_h - 4.0).max(4.0)
    } else {
        btn.bottom() + 4.0
    };

    (top as i32, right_edge as i32)
}

#[derive(Properties, Clone, PartialEq)]
pub struct ContextButtonProps {
    pub episode: Episode,
    pub page_type: PageType,
    #[prop_or(false)]
    pub show_menu_only: bool,
    #[prop_or(None)]
    pub position: Option<(i32, i32)>,
    #[prop_or(None)]
    pub on_close: Option<Callback<()>>,
}

#[function_component(ContextMenuButton)]
pub fn context_button(props: &ContextButtonProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_remove_downloaded_episode = i18n.t("context_menu_button.remove_downloaded_episode").to_string();
    let i18n_download_episode = i18n.t("context_menu_button.download_episode").to_string();
    #[cfg(not(feature = "server_build"))]
    let i18n_delete_local_download = i18n.t("context_menu_button.delete_local_download").to_string();
    #[cfg(not(feature = "server_build"))]
    let i18n_local_download = i18n.t("context_menu_button.local_download").to_string();
    let i18n_remove_from_queue = i18n.t("context_menu_button.remove_from_queue").to_string();
    let i18n_queue_episode = i18n.t("context_menu_button.queue_episode").to_string();
    let i18n_remove_from_saved_episodes = i18n.t("context_menu_button.remove_from_saved_episodes").to_string();
    let i18n_save_episode = i18n.t("context_menu_button.save_episode").to_string();
    let i18n_mark_episode_incomplete = i18n.t("context_menu_button.mark_episode_incomplete").to_string();
    let i18n_mark_episode_complete = i18n.t("context_menu_button.mark_episode_complete").to_string();
    let i18n_add_to_collection = i18n.t("collections.add_to_collection").to_string();
    // None = closed; Some((top, right_edge)) = open at this fixed viewport position.
    // Single state ensures the dropdown never renders at a stale position.
    let dropdown_state = use_state(|| Option::<(i32, i32)>::None);
    let dropdown_open = dropdown_state.is_some();
    // Unique id for this menu instance + the globally-open menu id, so opening
    // one menu closes any other (see effect below).
    let instance_id = *use_state(|| CTX_MENU_SEQ.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        n
    }));
    let active_menu_id = use_selector(|state: &ContextMenuState| state.open_id);
    let check_episode_id = props.episode.episodeid;
    // Auth selectors — only re-render on login/logout, not on episode actions
    let api_key_sel = use_selector(|state: &AppState| {
        state.auth_details.as_ref().map(|ud| ud.api_key.clone())
    });
    let user_id_sel = use_selector(|state: &AppState| {
        state.user_details.as_ref().map(|ud| ud.UserID.clone())
    });
    let server_name_sel = use_selector(|state: &AppState| {
        state.auth_details.as_ref().map(|ud| ud.server_name.clone())
    });
    let api_key = (*api_key_sel).clone();
    let user_id = (*user_id_sel).clone();
    let server_name = (*server_name_sel).clone();
    // Per-episode boolean selectors — only THIS card re-renders when ITS state changes
    let is_queued_sel = use_selector(move |state: &EpisodeStatusState| {
        state.queued_episode_ids.as_ref().map_or(false, |ids| ids.contains(&check_episode_id))
    });
    let is_saved_sel = use_selector(move |state: &EpisodeStatusState| {
        state.saved_episodes.iter().any(|e| e.episodeid == check_episode_id)
    });
    let is_downloaded_sel = use_selector(move |state: &EpisodeStatusState| {
        state.downloaded_episodes.is_server_download(check_episode_id)
    });
    #[cfg(not(feature = "server_build"))]
    let is_locally_downloaded_sel = use_selector(move |state: &EpisodeStatusState| {
        state.downloaded_episodes.is_local_download(check_episode_id)
    });
    let is_completed_sel = use_selector(move |state: &EpisodeStatusState| {
        state.completed_episodes.contains(&check_episode_id)
    });
    let dropdown_ref = use_node_ref();
    let button_ref = use_node_ref();

    // Concrete auth values for collection actions
    let cm_server = server_name.clone().unwrap_or_default();
    let cm_api_key = api_key.clone().flatten().unwrap_or_default();
    let cm_user_id = user_id.unwrap_or(0);

    // "Add to Collection" UX: 'modal' (default) or 'submenu'. Fetched when a menu opens.
    let add_ui_mode = use_state(|| "modal".to_string());
    let submenu_open = use_state(|| false);
    let submenu_collections = use_state(|| Vec::<Collection>::new());
    let submenu_members = use_state(|| HashSet::<i32>::new());
    let submenu_loaded = use_state(|| false);

    // Load the add-UI preference once per menu open
    {
        let add_ui_mode = add_ui_mode.clone();
        let server = cm_server.clone();
        let api_key = cm_api_key.clone();
        let uid = cm_user_id;
        use_effect_with(dropdown_open, move |open| {
            if *open && !server.is_empty() && !api_key.is_empty() {
                spawn_local(async move {
                    if let Ok(mode) = call_get_collection_add_ui(&server, &api_key, uid).await {
                        add_ui_mode.set(mode);
                    }
                });
            }
            || ()
        });
    }

    // Update dropdown_state if show_menu_only prop changes
    {
        let dropdown_state = dropdown_state.clone();
        use_effect_with(props.show_menu_only, move |show_menu_only| {
            if *show_menu_only {
                // Position is supplied via props.position for the long-press path
                dropdown_state.set(Some((0, 0)));
            }
            || ()
        });
    }

    let toggle_dropdown = {
        let dropdown_state = dropdown_state.clone();
        let button_ref = button_ref.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            if dropdown_state.is_some() {
                dropdown_state.set(None);
                Dispatch::<ContextMenuState>::global().reduce_mut(|s| {
                    if s.open_id == Some(instance_id) {
                        s.open_id = None;
                    }
                });
            } else if let Some(btn) = button_ref.cast::<web_sys::HtmlElement>() {
                // Provisional placement (menu not yet rendered so height is unknown);
                // the post-mount effect re-measures and flips up if needed.
                dropdown_state.set(Some(compute_menu_placement(&btn, None)));
                Dispatch::<ContextMenuState>::global()
                    .reduce_mut(|s| s.open_id = Some(instance_id));
            }
        })
    };

    // Close this menu when another menu becomes the active one. Skipped for the
    // long-press (show_menu_only) instance, which is coordinated by its parent.
    {
        let dropdown_state = dropdown_state.clone();
        let show_menu_only = props.show_menu_only;
        use_effect_with(*active_menu_id, move |active| {
            if !show_menu_only && *active != Some(instance_id) && dropdown_state.is_some() {
                dropdown_state.set(None);
            }
            || ()
        });
    }

    // After the menu mounts (or the window/scroll position changes), re-measure
    // and reposition so it tracks its button and flips up near the viewport edge.
    {
        let dropdown_state = dropdown_state.clone();
        let dropdown_ref = dropdown_ref.clone();
        let button_ref = button_ref.clone();
        use_effect_with(dropdown_open, move |open| {
            let mut listeners: Vec<EventListener> = Vec::new();
            if *open {
                let reposition = {
                    let dropdown_state = dropdown_state.clone();
                    let dropdown_ref = dropdown_ref.clone();
                    let button_ref = button_ref.clone();
                    move || {
                        if let Some(btn) = button_ref.cast::<HtmlElement>() {
                            let menu = dropdown_ref.cast::<HtmlElement>();
                            let pos = compute_menu_placement(&btn, menu.as_ref());
                            if *dropdown_state != Some(pos) {
                                dropdown_state.set(Some(pos));
                            }
                        }
                    }
                };
                // Run once now that the menu is measurable (applies flip-up).
                reposition();

                if let Some(win) = window() {
                    let capture = EventListenerOptions::run_in_capture_phase();
                    // Capture phase catches scrolls from any container (VirtualList
                    // may scroll the window or a nested element); scroll doesn't bubble.
                    let scroll_cb = reposition.clone();
                    listeners.push(EventListener::new_with_options(
                        &win,
                        "scroll",
                        capture,
                        move |_| scroll_cb(),
                    ));
                    let resize_cb = reposition.clone();
                    listeners.push(EventListener::new(&win, "resize", move |_| resize_cb()));
                }
            }
            move || drop(listeners)
        });
    }

    // Close dropdown when clicking outside
    {
        let dropdown_state = dropdown_state.clone();
        let dropdown_ref = dropdown_ref.clone();
        let button_ref = button_ref.clone();
        let on_close = props.on_close.clone();
        let show_menu_only = props.show_menu_only;

        use_effect_with((dropdown_open, ()), move |_| {
            let document = window().unwrap().document().unwrap();
            let dropdown_state = dropdown_state.clone();
            let dropdown_ref = dropdown_ref.clone();
            let button_ref = button_ref.clone();
            let on_close = on_close.clone();
            let show_menu_only = show_menu_only;

            // Handle outside clicks/touches to dismiss menu
            let handle_outside_interaction = {
                let dropdown_state = dropdown_state.clone();
                let dropdown_ref = dropdown_ref.clone();
                let button_ref = button_ref.clone();
                let on_close = on_close.clone();

                move |event: &web_sys::Event| {
                    if dropdown_state.is_some() {
                        if let Ok(target) = event.target().unwrap().dyn_into::<HtmlElement>() {
                            if let Some(dropdown_element) = dropdown_ref.cast::<HtmlElement>() {
                                let outside_dropdown = !dropdown_element.contains(Some(&target));

                                let outside_button = if let Some(button_element) =
                                    button_ref.cast::<HtmlElement>()
                                {
                                    !button_element.contains(Some(&target))
                                } else {
                                    true
                                };

                                if outside_dropdown && outside_button {
                                    dropdown_state.set(None);
                                    Dispatch::<ContextMenuState>::global().reduce_mut(|s| {
                                        if s.open_id == Some(instance_id) {
                                            s.open_id = None;
                                        }
                                    });
                                    if show_menu_only {
                                        if let Some(on_close) = &on_close {
                                            on_close.emit(());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };

            // Add click listener for desktop
            let click_handler = handle_outside_interaction.clone();
            let click_listener = EventListener::new(&document, "click", move |event| {
                click_handler(event);
            });

            // Use touchstart for mobile: avoids the lift-after-long-press immediately
            // triggering dismissal (touchend from the long press would close the menu instantly).
            let touch_handler = handle_outside_interaction.clone();
            let touch_listener = EventListener::new(&document, "touchstart", move |event| {
                touch_handler(event);
            });

            move || {
                drop(click_listener);
                drop(touch_listener);
            }
        });
    }

    let queue_api_key = api_key.clone();
    let queue_server_name = server_name.clone();
    // let server_name = server_name.clone();
    let on_add_to_queue = {
        let episode = props.episode.clone();
        Callback::from(move |_| {
            let server_name_copy = queue_server_name.clone();
            let api_key_copy = queue_api_key.clone();
            let episode_clone = episode.clone();
            let request = QueuePodcastRequest {
                episode_id: episode.episodeid,
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                match call_queue_episode(&server_name.unwrap(), &api_key.flatten(), &request).await
                {
                    Ok(success_message) => {
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            if let Some(ref mut queued_episodes) = state.queued_episode_ids {
                                queued_episodes.push(episode_clone.episodeid);
                            }
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Option::from(format!("{}", success_message));
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", formatted_error))
                        });
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let remove_queue_api_key = api_key.clone();
    let remove_queue_server_name = server_name.clone();
    // let server_name = server_name.clone();
    let on_remove_queued_episode = {
        let episode = props.episode.clone();
        let episode_id = props.episode.episodeid;
        Callback::from(move |_: MouseEvent| {
            let server_name_copy = remove_queue_server_name.clone();
            let api_key_copy = remove_queue_api_key.clone();
            let request = QueuePodcastRequest {
                episode_id: episode.episodeid,
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                match call_remove_queued_episode(
                    &server_name.unwrap(),
                    &api_key.flatten(),
                    &request,
                )
                .await
                {
                    Ok(success_message) => {
                        let formatted_info = format_error_message(&success_message.to_string());

                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            if let Some(ref mut queued_episodes) = state.queued_episodes {
                                queued_episodes
                                    .episodes
                                    .retain(|ep| ep.episodeid != episode_id);
                            }
                            if let Some(ref mut queued_episode_ids) = state.queued_episode_ids {
                                queued_episode_ids.retain(|&id| id != episode_id);
                            }
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(format!("{}", formatted_info).to_string());
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", formatted_error))
                        });
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let is_queued = *is_queued_sel;

    let on_toggle_queue = {
        let on_add_to_queue = on_add_to_queue.clone();
        let on_remove_queued_episode = on_remove_queued_episode.clone();
        Callback::from(move |e: MouseEvent| {
            if is_queued {
                on_remove_queued_episode.emit(e);
            } else {
                on_add_to_queue.emit(());
            }
        })
    };

    let saved_api_key = api_key.clone();
    let saved_server_name = server_name.clone();
    let on_save_episode = {
        let episode = props.episode.clone();
        Callback::from(move |_| {
            let server_name_copy = saved_server_name.clone();
            let api_key_copy = saved_api_key.clone();
            let request = SavePodcastRequest {
                episode_id: episode.episodeid, // changed from episode_title
                user_id: user_id.unwrap(),
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let ep = episode.clone();
            let future = async move {
                // let return_mes = call_save_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode saved successfully")));
                match call_save_episode(&server_name.unwrap(), &api_key.flatten(), &request).await {
                    Ok(success_message) => {
                        let formatted_info = format_error_message(&success_message.to_string());
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            if !state.saved_episode_ids().any(|id| id == episode.episodeid) {
                                state.saved_episodes.push(ep);
                            }
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Option::from(format!("{}", formatted_info));
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", formatted_error))
                        });
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let remove_saved_api_key = api_key.clone();
    let remove_saved_server_name = server_name.clone();
    let on_remove_saved_episode = {
        let episode = props.episode.clone();
        let _episode_id = props.episode.episodeid;
        Callback::from(move |_| {
            let server_name_copy = remove_saved_server_name.clone();
            let api_key_copy = remove_saved_api_key.clone();
            let request = SavePodcastRequest {
                episode_id: episode.episodeid,
                user_id: user_id.unwrap(),
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                match call_remove_saved_episode(&server_name.unwrap(), &api_key.flatten(), &request)
                    .await
                {
                    Ok(success_message) => {
                        let formatted_info = format_error_message(&success_message.to_string());

                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            state
                                .saved_episodes
                                .retain(|e| e.episodeid != episode.episodeid);
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(format!("{}", formatted_info).to_string());
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", formatted_error))
                        });
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let is_saved = *is_saved_sel;

    let on_toggle_save = {
        let on_save_episode = on_save_episode.clone();
        let on_remove_saved_episode = on_remove_saved_episode.clone();
        Callback::from(move |_| {
            if is_saved {
                on_remove_saved_episode.emit(());
            } else {
                on_save_episode.emit(());
            }
        })
    };

    let download_api_key = api_key.clone();
    let download_server_name = server_name.clone();
    let on_server_download_episode = {
        let episode = props.episode.clone();
        Callback::from(move |_| {
            let server_name_copy = download_server_name.clone();
            let api_key_copy = download_api_key.clone();
            let request = DownloadEpisodeRequest {
                episode_id: episode.episodeid,
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let episode = episode.clone();
            let future = async move {
                match call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request)
                    .await
                {
                    Ok(success_message) => {
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            state.downloaded_episodes.push_server(episode);
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Option::from(format!("{}", success_message));
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", formatted_error))
                        });
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let remove_download_api_key = api_key.clone();
    let remove_download_server_name = server_name.clone();
    let on_remove_downloaded_episode = {
        let episode = props.episode.clone();
        let _episode_id = props.episode.episodeid;
        Callback::from(move |_| {
            let server_name_copy = remove_download_server_name.clone();
            let api_key_copy = remove_download_api_key.clone();
            let request = DownloadEpisodeRequest {
                episode_id: episode.episodeid,
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                match call_remove_downloaded_episode(
                    &server_name.unwrap(),
                    &api_key.flatten(),
                    &request,
                )
                .await
                {
                    Ok(success_message) => {
                        let formatted_info = format_error_message(&success_message.to_string());

                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            state.downloaded_episodes.remove_local(episode.episodeid);
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(format!("{}", formatted_info).to_string());
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", formatted_error))
                        });
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let is_downloaded = *is_downloaded_sel;

    #[cfg(not(feature = "server_build"))]
    let is_locally_downloaded = *is_locally_downloaded_sel;

    let on_toggle_download = {
        let on_download = on_server_download_episode.clone();
        let on_remove_download = on_remove_downloaded_episode.clone();
        Callback::from(move |_| {
            if is_downloaded {
                on_remove_download.emit(());
            } else {
                on_download.emit(());
            }
        })
    };

    #[cfg(not(feature = "server_build"))]
    let on_local_episode_download = {
        let episode = props.episode.clone();
        let server_name_copy = server_name.clone();
        let api_key_copy = api_key.clone();
        let user_id_copy = user_id.clone();

        Callback::from(move |_| {
            let episode_id = episode.episodeid;
            let request = EpisodeRequest {
                episode_id,
                user_id: user_id_copy.unwrap(),
                person_episode: false,
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy.clone().unwrap();
            let ep_api_key = api_key_copy.clone().flatten();
            let api_key = api_key_copy.clone().flatten();

            let episode = episode.clone();
            let future = async move {
                match call_get_episode_metadata(&server_name, ep_api_key, &request).await {
                    Ok(episode_info) => {
                        let audio_url = episode_info.episodeurl.clone();
                        let artwork_url = episode_info.episodeartwork.clone();
                        let podcast_id = episode_info.podcastid.clone();
                        let filename = format!("episode_{}.mp3", episode_id);
                        let artwork_filename = format!("artwork_{}.jpg", episode_id);
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            state.downloaded_episodes.push_local(episode);
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(format!("Episode download queued!"));
                        });
                        // Download audio
                        match download_file(audio_url, filename.clone()).await {
                            Ok(_) => {}
                            Err(e) => {
                                let formatted_error = format_error_message(&format!("{:?}", e));
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message = Some(format!(
                                        "Failed to download episode audio: {}",
                                        formatted_error.clone()
                                    ))
                                });
                                web_sys::console::log_1(&format!("audio fail: {:?}", e).into());
                            }
                        }

                        // Download artwork
                        if let Err(e) = download_file(artwork_url, artwork_filename.clone()).await {
                            let formatted_error = format_error_message(&format!("{:?}", e));
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to download episode artwork: {}",
                                    formatted_error.clone()
                                ))
                            });
                            web_sys::console::log_1(&format!("art fail: {:?}", e).into());
                        }

                        // Update local JSON database
                        if let Err(e) = update_local_database(episode_info.clone()).await {
                            let formatted_error = format_error_message(&format!("{:?}", e));
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to update local database: {}",
                                    formatted_error.clone()
                                ))
                            });
                            web_sys::console::log_1(
                                &format!("Unable to parse Podcasts: {:?}", e).into(),
                            );
                        }

                        // Fetch and update local podcast metadata
                        match call_get_podcast_details(
                            &server_name,
                            &api_key.unwrap(),
                            user_id_copy.unwrap(),
                            podcast_id,
                        )
                        .await
                        {
                            Ok(podcast_details) => {
                                if let Err(e) = update_podcast_database(podcast_details).await {
                                    let formatted_error =
                                        format_error_message(&format!("{:?}", e));
                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                        state.error_message = Some(format!(
                                            "Failed to update podcast database: {}",
                                            formatted_error
                                        ))
                                    });
                                }
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message = Some(format!(
                                        "Failed to fetch podcast metadata: {:?}",
                                        formatted_error
                                    ))
                                });
                            }
                        }
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!("s {:?}", formatted_error))
                        });
                    }
                }
            };

            wasm_bindgen_futures::spawn_local(future);
        })
    };

    #[cfg(not(feature = "server_build"))]
    let on_remove_locally_downloaded_episode = {
        let episode = props.episode.clone();

        Callback::from(move |_: MouseEvent| {
            let ui_state = Dispatch::<UIState>::global();
            let episode_id = episode.episodeid;

            let future = async move {
                let filename = format!("episode_{}.mp3", episode_id);

                match remove_episode_from_local_db(episode_id).await {
                    Ok(_) => {
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            state.downloaded_episodes.remove_local(episode_id);
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message =
                                Some(format!("Local episode {} deleted!", filename));
                        });

                        // Update local_download_increment in ui_state
                        ui_state.reduce_mut(|state| {
                            if let Some(increment) = state.local_download_increment.as_mut() {
                                *increment += 1;
                            } else {
                                state.local_download_increment = Some(1);
                            }
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&format!("{:?}", e));
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!(
                                "Failed to download episode audio: {}",
                                formatted_error
                            ))
                        });
                    }
                }
            };

            wasm_bindgen_futures::spawn_local(future);
        })
    };

    let uncomplete_api_key = api_key.clone();
    let uncomplete_server_name = server_name.clone();
    let on_uncomplete_episode = {
        let episode = props.episode.clone();
        let episode_id = props.episode.episodeid;
        Callback::from(move |_| {
            let server_name_copy = uncomplete_server_name.clone();
            let api_key_copy = uncomplete_api_key.clone();
            let request = MarkEpisodeCompletedRequest {
                episode_id: episode.episodeid,
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                match call_mark_episode_uncompleted(
                    &server_name.unwrap(),
                    &api_key.flatten(),
                    &request,
                )
                .await
                {
                    Ok(success_message) => {
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            if !state.completed_episodes.remove(&episode_id) {
                                state.completed_episodes.insert(episode_id);
                            }
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(format!("{}", success_message));
                        });
                    }
                    Err(e) => {
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", e))
                        });
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let complete_api_key = api_key.clone();
    let complete_server_name = server_name.clone();
    let on_complete_episode = {
        let episode = props.episode.clone();
        let episode_id = props.episode.episodeid;
        Callback::from(move |_| {
            let server_name_copy = complete_server_name.clone();
            let api_key_copy = complete_api_key.clone();
            let request = MarkEpisodeCompletedRequest {
                episode_id: episode.episodeid,
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.is_youtube,
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                match call_mark_episode_completed(
                    &server_name.unwrap(),
                    &api_key.flatten(),
                    &request,
                )
                .await
                {
                    Ok(success_message) => {
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                            if !state.completed_episodes.remove(&episode_id) {
                                state.completed_episodes.insert(episode_id);
                            }
                        });
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(format!("{}", success_message));
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", formatted_error))
                        });
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let is_completed = *is_completed_sel;

    let on_toggle_complete = {
        let on_complete_episode = on_complete_episode.clone();
        let on_uncomplete_episode = on_uncomplete_episode.clone();

        Callback::from(move |_| {
            if is_completed {
                on_uncomplete_episode.emit(());
            } else {
                on_complete_episode.emit(());
            }
        })
    };

    let close_dropdown = {
        let dropdown_state = dropdown_state.clone();
        let on_close = props.on_close.clone();
        let show_menu_only = props.show_menu_only;

        Callback::from(move |_| {
            dropdown_state.set(None);
            Dispatch::<ContextMenuState>::global().reduce_mut(|s| {
                if s.open_id == Some(instance_id) {
                    s.open_id = None;
                }
            });

            if show_menu_only {
                if let Some(on_close) = &on_close {
                    on_close.emit(());
                }
            }
        })
    };

    let wrap_action = |action: Callback<MouseEvent>| {
        let close = close_dropdown.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            action.emit(e);
            close.emit(());
        })
    };

    #[cfg(feature = "server_build")]
    let download_button = html! {
        <li class="dropdown-option" onclick={wrap_action(on_toggle_download.clone())}>
            { if is_downloaded { &i18n_remove_downloaded_episode } else { &i18n_download_episode } }
        </li>
    };

    #[cfg(not(feature = "server_build"))]
    let download_button = html! {
        <>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_download.clone())}>
                { if is_downloaded { &i18n_remove_downloaded_episode } else { &i18n_download_episode } }
            </li>
            {
                if is_locally_downloaded {
                    html! {
                        <li class="dropdown-option" onclick={wrap_action(on_remove_locally_downloaded_episode.clone())}>
                            { &i18n_delete_local_download }
                        </li>
                    }
                } else {
                    html! {
                        <li class="dropdown-option" onclick={wrap_action(on_local_episode_download.clone())}>
                            { &i18n_local_download }
                        </li>
                    }
                }
            }
        </>
    };

    #[cfg(not(feature = "server_build"))]
    let local_download_options = html! {
        <>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                { if is_queued { &i18n_remove_from_queue } else { &i18n_queue_episode } }
            </li>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                { if is_saved { &i18n_remove_from_saved_episodes } else { &i18n_save_episode } }
            </li>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_download.clone())}>
                { if is_downloaded { &i18n_remove_downloaded_episode } else { &i18n_download_episode } }
            </li>
            {
                if is_locally_downloaded {
                    html! {
                        <li class="dropdown-option" onclick={wrap_action(on_remove_locally_downloaded_episode.clone())}>
                            { &i18n_delete_local_download }
                        </li>
                    }
                } else {
                    html! {
                        <li class="dropdown-option" onclick={wrap_action(on_local_episode_download.clone())}>
                            { &i18n_local_download }
                        </li>
                    }
                }
            }
            <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>{ if is_completed { &i18n_mark_episode_incomplete } else { &i18n_mark_episode_complete } }</li>
        </>
    };

    #[cfg(feature = "server_build")]
    let local_download_options = html! {};

    // ---- Add to Collection (modal or submenu) ----
    // Modal mode opens the single global picker overlay via its store.
    let open_picker = {
        let dropdown_state = dropdown_state.clone();
        let episode = props.episode.clone();
        Callback::from(move |_: ()| {
            dropdown_state.set(None);
            let episode = episode.clone();
            Dispatch::<CollectionModalState>::global().reduce_mut(move |s| {
                s.open = true;
                s.episode = Some(episode);
            });
        })
    };

    let on_add_to_collection_click = {
        let add_ui_mode = add_ui_mode.clone();
        let submenu_open = submenu_open.clone();
        let submenu_loaded = submenu_loaded.clone();
        let submenu_collections = submenu_collections.clone();
        let submenu_members = submenu_members.clone();
        let open_picker = open_picker.clone();
        let server = cm_server.clone();
        let api_key = cm_api_key.clone();
        let uid = cm_user_id;
        let ep_id = props.episode.episodeid;
        let is_yt = props.episode.is_youtube;
        Callback::from(move |_: ()| {
            if *add_ui_mode == "submenu" {
                let opening = !*submenu_open;
                submenu_open.set(opening);
                if opening && !*submenu_loaded {
                    let submenu_collections = submenu_collections.clone();
                    let submenu_members = submenu_members.clone();
                    let submenu_loaded = submenu_loaded.clone();
                    let server = server.clone();
                    let api_key = api_key.clone();
                    spawn_local(async move {
                        let cols = call_get_collections(&server, &api_key, uid).await.unwrap_or_default();
                        let members = call_get_episode_collections(&server, &api_key, uid, ep_id, is_yt)
                            .await.unwrap_or_default();
                        submenu_collections.set(cols);
                        submenu_members.set(members.into_iter().collect());
                        submenu_loaded.set(true);
                    });
                }
            } else {
                open_picker.emit(());
            }
        })
    };

    let add_click_onclick = {
        let cb = on_add_to_collection_click.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            cb.emit(());
        })
    };

    // Produces a per-row toggle callback for submenu membership
    let toggle_member = {
        let submenu_members = submenu_members.clone();
        let submenu_collections = submenu_collections.clone();
        let server = cm_server.clone();
        let api_key = cm_api_key.clone();
        let uid = cm_user_id;
        let ep_id = props.episode.episodeid;
        let is_yt = props.episode.is_youtube;
        let episode = props.episode.clone();
        move |col_id: i32, is_member: bool| -> Callback<MouseEvent> {
            let submenu_members = submenu_members.clone();
            let is_default = submenu_collections.iter()
                .find(|c| c.collection_id == col_id)
                .map(|c| c.is_default)
                .unwrap_or(false);
            let server = server.clone();
            let api_key = api_key.clone();
            let episode = episode.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                let mut set = (*submenu_members).clone();
                let now_member = if is_member {
                    set.remove(&col_id);
                    false
                } else {
                    set.insert(col_id);
                    true
                };
                submenu_members.set(set);
                let req = CollectionEpisodeRequest { user_id: uid, episode_id: ep_id, is_youtube: is_yt };
                let server = server.clone();
                let api_key = api_key.clone();
                let episode = episode.clone();
                spawn_local(async move {
                    if now_member {
                        let _ = call_add_episode_to_collection(&server, &api_key, col_id, &req).await;
                    } else {
                        let _ = call_remove_episode_from_collection(&server, &api_key, col_id, &req).await;
                    }
                    if is_default {
                        if now_member {
                            let ep = episode.clone();
                            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                if !s.saved_episode_ids().any(|id| id == ep_id) {
                                    s.saved_episodes.push(ep);
                                }
                            });
                        } else {
                            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                s.saved_episodes.retain(|e| e.episodeid != ep_id);
                            });
                        }
                    }
                });
            })
        }
    };

    let new_from_submenu = {
        let open_picker = open_picker.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            open_picker.emit(());
        })
    };

    let is_submenu = *add_ui_mode == "submenu";
    let add_to_collection_section = html! {
        <>
            <li class="dropdown-option" onclick={add_click_onclick.clone()}>
                { &i18n_add_to_collection }
                { if is_submenu { html! { <i class="ph ph-caret-right" style="float:right"></i> } } else { html! {} } }
            </li>
            {
                if is_submenu && *submenu_open {
                    html! {
                        <>
                            {
                                for submenu_collections.iter().map(|c| {
                                    let is_member = submenu_members.contains(&c.collection_id);
                                    let onclick = toggle_member(c.collection_id, is_member);
                                    html! {
                                        <li class="dropdown-option flex items-center gap-2" style="padding-left:1.5rem;" {onclick}>
                                            <i class={classes!("ph", if is_member { "ph-check-square" } else { "ph-square" })}></i>
                                            <span>{ c.name.clone() }</span>
                                        </li>
                                    }
                                })
                            }
                            <li class="dropdown-option flex items-center gap-2" style="padding-left:1.5rem;" onclick={new_from_submenu}>
                                <i class="ph ph-plus"></i>
                                <span>{ i18n.t("collections.new_collection") }</span>
                            </li>
                        </>
                    }
                } else {
                    html! {}
                }
            }
        </>
    };

    let action_buttons = match props.page_type {
        PageType::Saved => html! {
            <>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                    { if is_queued { &i18n_remove_from_queue } else { &i18n_queue_episode } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                    { &i18n_remove_from_saved_episodes }
                </li>
                { add_to_collection_section.clone() }
                {
                    download_button.clone()
                }
                <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>
                    { if is_completed { &i18n_mark_episode_incomplete } else { &i18n_mark_episode_complete } }
                </li>
            </>
        },
        PageType::Queue => html! {
            <>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                    { if is_saved { &i18n_remove_from_saved_episodes } else { &i18n_save_episode } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_remove_queued_episode.clone())}>
                    { &i18n_remove_from_queue }
                </li>
                {
                    download_button.clone()
                }
                <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>{ if is_completed { &i18n_mark_episode_incomplete } else { &i18n_mark_episode_complete } }</li>
            </>
        },
        PageType::Downloads => html! {
            <>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                    { if is_queued { &i18n_remove_from_queue } else { &i18n_queue_episode } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                    { if is_saved { &i18n_remove_from_saved_episodes } else { &i18n_save_episode } }
                </li>
                { add_to_collection_section.clone() }
                {
                    download_button.clone()
                }
                <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>{ if is_completed { &i18n_mark_episode_incomplete } else { &i18n_mark_episode_complete } }</li>
            </>
        },
        PageType::LocalDownloads => html! {
            <>
                { local_download_options }
                { add_to_collection_section.clone() }
            </>
        },
        PageType::Default => html! {
            <>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                    { if is_queued { &i18n_remove_from_queue } else { &i18n_queue_episode } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                    { if is_saved { &i18n_remove_from_saved_episodes } else { &i18n_save_episode } }
                </li>
                { add_to_collection_section.clone() }
                {
                    download_button.clone()
                }
                <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>{ if is_completed { &i18n_mark_episode_incomplete } else { &i18n_mark_episode_complete } }</li>
            </>
        },
    };

    let block_row_activation = Callback::from(|e: MouseEvent| e.stop_propagation());
    let block_row_touch = Callback::from(|e: TouchEvent| e.stop_propagation());

    let dropdown_html = if props.show_menu_only {
        if let Some((x, y)) = props.position {
            html! {
                <div
                    ref={dropdown_ref.clone()}
                    class="ep-context-menu"
                    style={format!("position: fixed; top: {}px; left: {}px;", y, x)}
                    onmousedown={block_row_activation.clone()}
                    ontouchstart={block_row_touch.clone()}
                >
                    <ul class="ep-context-menu-list">{ action_buttons }</ul>
                </div>
            }
        } else {
            html! {}
        }
    } else if let Some((top, right_edge)) = *dropdown_state {
        html! {
            <div
                ref={dropdown_ref.clone()}
                class="ep-context-menu"
                style={format!("position: fixed; top: {}px; right: calc(100vw - {}px);", top, right_edge)}
                onmousedown={block_row_activation}
                ontouchstart={block_row_touch}
            >
                <ul class="ep-context-menu-list">{ action_buttons }</ul>
            </div>
        }
    } else {
        html! {}
    };

    html! {
        <div class="context-button-wrapper">
            if !props.show_menu_only {
                <button
                    ref={button_ref.clone()}
                    onclick={toggle_dropdown.clone()}
                    class="ico"
                >
                    <i class="ph ph-dots-three"></i>
                </button>
            }
            { dropdown_html }
        </div>
    }
}
