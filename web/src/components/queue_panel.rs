use crate::components::audio::on_play_pause;
use crate::components::context::{AppState, EpisodeStatusState, UIState};
use crate::components::gen_components::FallbackImage;
use crate::components::queue_manage_modal::QueueManageModal;
use crate::requests::pod_req::{
    call_clear_queue, call_get_queued_episodes, call_remove_queued_episode, call_reorder_queue,
    QueuePodcastRequest, QueuedEpisodesResponse,
};
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use web_sys::{DragEvent, MouseEvent};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[function_component(QueuePanel)]
pub fn queue_panel() -> Html {
    let (i18n, _) = use_translation();
    let i18n_up_next = i18n.t("queue_panel.up_next").to_string();
    let i18n_now_playing = i18n.t("queue_panel.now_playing").to_string();
    let i18n_queue_is_empty = i18n.t("queue_panel.queue_is_empty").to_string();
    let i18n_add_episodes_hint = i18n.t("queue_panel.add_episodes_hint").to_string();
    let (ui_state, ui_dispatch) = use_store::<UIState>();
    let (app_state, _app_dispatch) = use_store::<AppState>();
    let (ep_status, ep_dispatch) = use_store::<EpisodeStatusState>();

    let is_open = ui_state.queue_panel_open;
    let dragging_id = use_state(|| None::<i32>);
    let manage_open = use_state(|| false);

    let open_manage = {
        let manage_open = manage_open.clone();
        Callback::from(move |_: MouseEvent| manage_open.set(true))
    };

    let close = {
        let ui_dispatch = ui_dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            ui_dispatch.reduce_mut(|s| s.queue_panel_open = false);
        })
    };

    let on_clear_queue = {
        let ep_dispatch = ep_dispatch.clone();
        let app_state = app_state.clone();
        Callback::from(move |_: MouseEvent| {
            if let (Some(auth), Some(user)) = (
                app_state.auth_details.as_ref(),
                app_state.user_details.as_ref(),
            ) {
                let server = auth.server_name.clone();
                let key = auth.api_key.clone();
                let uid = user.UserID;
                let dispatch = ep_dispatch.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if call_clear_queue(&server, &key, &uid).await.is_ok() {
                        dispatch.reduce_mut(|s| {
                            s.queued_episodes =
                                Some(QueuedEpisodesResponse { episodes: vec![] });
                            s.queued_episode_ids = Some(vec![]);
                        });
                    }
                });
            }
        })
    };

    let on_scrim_click = close.clone();

    // Fetch and sort queue whenever the panel opens.
    {
        let ep_dispatch = ep_dispatch.clone();
        let app_state = app_state.clone();
        use_effect_with(is_open, move |&open| {
            if open {
                if let (Some(auth), Some(user)) = (
                    app_state.auth_details.as_ref(),
                    app_state.user_details.as_ref(),
                ) {
                    let server = auth.server_name.clone();
                    let key = auth.api_key.clone();
                    let uid = user.UserID;
                    let dispatch = ep_dispatch.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(mut eps) =
                            call_get_queued_episodes(&server, &key, &uid).await
                        {
                            eps.sort_by_key(|e| e.queueposition.unwrap_or(i32::MAX));
                            dispatch.reduce_mut(|s| {
                                s.queued_episodes =
                                    Some(QueuedEpisodesResponse { episodes: eps });
                            });
                        }
                    });
                }
            }
            || ()
        });
    }

    // Do NOT re-sort here — episodes are sorted by queueposition on fetch, and
    // drag-reorder maintains the correct Vec order optimistically. Re-sorting
    // would undo the visual reorder since queueposition fields aren't mutated.
    let queued = ep_status
        .queued_episodes
        .as_ref()
        .map(|r| r.episodes.clone())
        .unwrap_or_default();

    let now_playing = ui_state.currently_playing.clone();
    let now_playing_art = now_playing.as_ref().map(|p| p.artwork_url.clone());
    let now_playing_title = now_playing.as_ref().map(|p| p.title.clone());
    let is_playing = ui_state.audio_playing.unwrap_or(false);
    let currently_playing_id = ui_state.currently_playing.as_ref().map(|p| p.episode_id);

    let total_secs: i32 = queued.iter().map(|e| e.episodeduration).sum();
    let total_h = total_secs / 3600;
    let total_m = (total_secs % 3600) / 60;
    let total_label = if total_h > 0 {
        format!("{}h {}m", total_h, total_m)
    } else {
        format!("{}m", total_m)
    };

    // ── Drag-and-drop ─────────────────────────────────────────────────────────

    let ondragstart = {
        let dragging_id = dragging_id.clone();
        Callback::from(move |e: DragEvent| {
            if let Some(tgt) = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                let mut el = tgt;
                for _ in 0..8 {
                    if let Some(id_str) = el.get_attribute("data-id") {
                        if let Ok(id) = id_str.parse::<i32>() {
                            dragging_id.set(Some(id));
                            if let Some(dt) = e.data_transfer() {
                                let _ = dt.set_data("text/plain", &id_str);
                                dt.set_effect_allowed("move");
                            }
                        }
                        break;
                    }
                    if let Some(parent) = el.parent_element() {
                        el = parent;
                    } else {
                        break;
                    }
                }
            }
        })
    };

    let ondragover = Callback::from(|e: DragEvent| {
        e.prevent_default();
        if let Some(dt) = e.data_transfer() {
            dt.set_drop_effect("move");
        }
    });

    let ondragend = {
        let dragging_id = dragging_id.clone();
        Callback::from(move |_: DragEvent| dragging_id.set(None))
    };

    let ondrop = {
        let dragging_id = dragging_id.clone();
        let ep_dispatch = ep_dispatch.clone();
        let ep_status = ep_status.clone();
        let app_state = app_state.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            let dragged = match *dragging_id {
                Some(id) => id,
                None => return,
            };

            let mut target_id = None::<i32>;
            if let Some(tgt) = e.target().and_then(|t| t.dyn_into::<web_sys::Element>().ok()) {
                let mut el = tgt;
                for _ in 0..8 {
                    if let Some(id_str) = el.get_attribute("data-id") {
                        target_id = id_str.parse::<i32>().ok();
                        break;
                    }
                    if let Some(parent) = el.parent_element() {
                        el = parent;
                    } else {
                        break;
                    }
                }
            }

            if let Some(tid) = target_id {
                if tid != dragged {
                    if let Some(queued) = ep_status.queued_episodes.as_ref() {
                        let mut episodes = queued.episodes.clone();
                        if let (Some(from), Some(to)) = (
                            episodes.iter().position(|ep| ep.episodeid == dragged),
                            episodes.iter().position(|ep| ep.episodeid == tid),
                        ) {
                            let item = episodes.remove(from);
                            episodes.insert(to, item);

                            let episode_ids: Vec<i32> =
                                episodes.iter().map(|ep| ep.episodeid).collect();

                            ep_dispatch.reduce_mut(|s| {
                                s.queued_episodes =
                                    Some(QueuedEpisodesResponse { episodes: episodes });
                            });

                            if let (Some(auth), Some(user)) = (
                                app_state.auth_details.as_ref(),
                                app_state.user_details.as_ref(),
                            ) {
                                let server = auth.server_name.clone();
                                let key = auth.api_key.clone();
                                let uid = user.UserID;
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Err(err) = call_reorder_queue(
                                        &server,
                                        &key,
                                        &uid,
                                        &episode_ids,
                                    )
                                    .await
                                    {
                                        web_sys::console::log_1(
                                            &format!("Queue reorder failed: {:?}", err).into(),
                                        );
                                    }
                                });
                            }
                        }
                    }
                }
            }

            dragging_id.set(None);
        })
    };

    let current_dragging = *dragging_id;

    // Queue Management modal (issue #661). Mounted conditionally so it re-snapshots the queue on
    // each open and re-renders live when the store changes (e.g. after a bulk remove).
    let manage_modal = if *manage_open {
        if let (Some(auth), Some(user)) = (
            app_state.auth_details.as_ref(),
            app_state.user_details.as_ref(),
        ) {
            let on_close = {
                let manage_open = manage_open.clone();
                Callback::from(move |_| manage_open.set(false))
            };
            html! {
                <QueueManageModal
                    episodes={queued.clone()}
                    server={auth.server_name.clone()}
                    api_key={auth.api_key.clone()}
                    user_id={user.UserID}
                    on_close={on_close}
                />
            }
        } else {
            Html::default()
        }
    } else {
        Html::default()
    };

    html! {
        <>
            <div
                class={classes!("queue-scrim", if is_open { "is-open" } else { "" })}
                onclick={on_scrim_click}
            />
            <aside class={classes!("queue-panel", if is_open { "is-open" } else { "" })} aria-label="Queue">
                <div class="queue-head">
                    <div>
                        <div class="queue-title">{ &i18n_up_next }</div>
                        <div class="queue-sub">
                            { format!("{} episode{} \u{00B7} {}", queued.len(),
                                if queued.len() == 1 { "" } else { "s" }, total_label) }
                        </div>
                    </div>
                    <div class="queue-head-actions">
                        <button class="player-btn" onclick={open_manage} title="Manage queue">
                            <i class="ph ph-sliders-horizontal"></i>
                        </button>
                        <button class="player-btn" onclick={on_clear_queue} title="Clear queue">
                            <i class="ph ph-broom"></i>
                        </button>
                        <button class="player-btn" onclick={close.clone()} title="Close">
                            <i class="ph ph-x"></i>
                        </button>
                    </div>
                </div>

                if let (Some(art), Some(title)) = (now_playing_art, now_playing_title) {
                    <div class="queue-nowplaying">
                        <FallbackImage
                            src={art}
                            alt="Now playing art"
                            class="queue-nowplaying-art"
                        />
                        <div style="min-width: 0; flex: 1;">
                            <div class="queue-now-eyebrow">{ &i18n_now_playing }</div>
                            <div class="queue-now-title">{ title }</div>
                        </div>
                        if is_playing {
                            <div class="queue-now-equalizer" aria-hidden="true">
                                <span></span><span></span><span></span>
                            </div>
                        }
                    </div>
                }

                <div class="queue-list">
                    if queued.is_empty() {
                        <div class="queue-empty">
                            <i class="ph ph-queue"></i>
                            <div class="queue-empty-title">{ &i18n_queue_is_empty }</div>
                            <div class="queue-empty-sub">{ &i18n_add_episodes_hint }</div>
                        </div>
                    } else {
                        { for queued.iter().enumerate().map(|(_i, ep)| {
                            let ep = ep.clone();
                            let ep_for_remove = ep.clone();
                            let ep_for_art = ep.clone();
                            let auth = app_state.auth_details.clone();
                            let user = app_state.user_details.clone();
                            let app_dispatch_remove = ep_dispatch.clone();

                            let ep_id = ep.episodeid;
                            let is_this_playing = currently_playing_id == Some(ep_id) && is_playing;
                            let is_this_current = currently_playing_id == Some(ep_id);
                            let is_dragged = current_dragging == Some(ep_id);

                            // Artwork click: play/pause toggle via on_play_pause.
                            // Only close the panel when starting a new episode.
                            let on_art_click = {
                                let auth = app_state.auth_details.clone();
                                let user = app_state.user_details.clone();
                                let ui_dispatch_art = ui_dispatch.clone();
                                let ui_state_art = ui_state.clone();
                                let app_state_art = app_state.clone();
                                Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    if let (Some(auth), Some(user)) =
                                        (auth.as_ref(), user.as_ref())
                                    {
                                        let api_key = auth.api_key.clone().unwrap_or_default();
                                        let user_id = user.UserID;
                                        let server_name = auth.server_name.clone();
                                        if !is_this_current {
                                            ui_dispatch_art
                                                .reduce_mut(|s| s.queue_panel_open = false);
                                        }
                                        on_play_pause(
                                            ep_for_art.clone(),
                                            api_key,
                                            user_id,
                                            server_name,
                                            ui_dispatch_art.clone(),
                                            ui_state_art.clone(),
                                            app_state_art.clone(),
                                        )
                                        .emit(e);
                                    }
                                })
                            };

                            // Title click: navigate to episode page.
                            let ep_is_youtube = ep.is_youtube;
                            let on_title_click = {
                                let ui_dispatch_nav = ui_dispatch.clone();
                                Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    ui_dispatch_nav
                                        .reduce_mut(|s| s.queue_panel_open = false);
                                    let url = if ep_is_youtube {
                                        format!("/episode?episode_id={}&youtube=true", ep_id)
                                    } else {
                                        format!("/episode?episode_id={}", ep_id)
                                    };
                                    BrowserHistory::new().push(url);
                                })
                            };

                            let on_remove = {
                                Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    if let (Some(auth), Some(user)) =
                                        (auth.as_ref(), user.as_ref())
                                    {
                                        let server = auth.server_name.clone();
                                        let key = auth.api_key.clone();
                                        let uid = user.UserID;
                                        let ep_id = ep_for_remove.episodeid;
                                        let is_yt = ep_for_remove.is_youtube;
                                        let dispatch = app_dispatch_remove.clone();
                                        wasm_bindgen_futures::spawn_local(async move {
                                            let req = QueuePodcastRequest {
                                                episode_id: ep_id,
                                                user_id: uid,
                                                is_youtube: is_yt,
                                            };
                                            let _ = call_remove_queued_episode(
                                                &server, &key, &req,
                                            )
                                            .await;
                                            if let Ok(mut eps) = call_get_queued_episodes(
                                                &server, &key, &uid,
                                            )
                                            .await
                                            {
                                                eps.sort_by_key(|e| {
                                                    e.queueposition.unwrap_or(i32::MAX)
                                                });
                                                dispatch.reduce_mut(|s| {
                                                    s.queued_episodes = Some(
                                                        QueuedEpisodesResponse { episodes: eps },
                                                    );
                                                });
                                            }
                                        });
                                    }
                                })
                            };

                            let ep_art = ep.episodeartwork.clone();
                            let ep_title = ep.episodetitle.clone();
                            let ep_duration =
                                crate::components::gen_funcs::format_time(ep.episodeduration);

                            html! {
                                <div
                                    class={classes!(
                                        "queue-item",
                                        if is_dragged { "is-dragging" } else { "" }
                                    )}
                                    data-id={ep_id.to_string()}
                                    draggable="true"
                                    ondragstart={ondragstart.clone()}
                                    ondragover={ondragover.clone()}
                                    ondragend={ondragend.clone()}
                                    ondrop={ondrop.clone()}
                                    key={ep_id}
                                >
                                    <div class="queue-drag-handle">
                                        <i class="ph ph-dots-six-vertical"></i>
                                    </div>

                                    // Artwork with play/pause overlay
                                    <div class="queue-item-art-wrap" onclick={on_art_click}>
                                        <FallbackImage
                                            src={ep_art}
                                            alt="Episode art"
                                            class="queue-item-art"
                                        />
                                        <div class={classes!(
                                            "queue-item-art-overlay",
                                            if is_this_playing { "is-playing" } else { "" }
                                        )}>
                                            if is_this_playing {
                                                <i class="ph ph-pause"></i>
                                            } else {
                                                <i class="ph ph-play"></i>
                                            }
                                        </div>
                                    </div>

                                    <div class="queue-item-body">
                                        <div
                                            class="queue-item-title queue-item-title-link"
                                            onclick={on_title_click}
                                        >
                                            { ep_title }
                                        </div>
                                        <div class="queue-item-sub">
                                            <span>{ ep_duration }</span>
                                        </div>
                                    </div>
                                    <button
                                        class="queue-item-remove"
                                        onclick={on_remove}
                                        title="Remove"
                                    >
                                        <i class="ph ph-x"></i>
                                    </button>
                                </div>
                            }
                        }) }
                    }
                </div>
            </aside>
            { manage_modal }
        </>
    }
}
