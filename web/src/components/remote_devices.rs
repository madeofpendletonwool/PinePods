// Remote-devices panel: shows the user's other active devices and what they're
// playing, with advisory transport controls, "Play here" (pull) and "Send here"
// (push this tab's playback). Sends commands over the now-playing WebSocket; the
// target device applies them to its local player.
//
// Split in two: `RemoteDevices` is just the topbar button (desktop layout only),
// while `RemoteDevicesPanel` — the drawer plus the inbound remote-play observer —
// is mounted once regardless of layout, so narrow screens still receive beams and
// the full-screen player can open the drawer via `NowPlayingState::panel_open`.

use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

use crate::components::audio::on_play_click;
use crate::components::context::{AppState, UIState};
use crate::requests::now_playing::{
    send_command, send_playback_to, NowPlayingDevice, NowPlayingState, PendingRemotePlay,
};
use crate::requests::notify::toast_info;
use crate::components::context::NotificationState;
use crate::requests::pod_req::{call_get_episode_metadata, EpisodeRequest};
use i18nrs::yew::use_translation;

fn fmt_time(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "00:00".to_string();
    }
    let total = secs as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{:02}:{:02}", m, s)
    }
}

/// Phosphor icon for a device row, from the free-form `device_type`.
pub fn device_icon(device_type: &str) -> &'static str {
    match device_type.to_ascii_lowercase().as_str() {
        "web" => "ph ph-browser",
        "ios" | "android" | "mobile" => "ph ph-device-mobile",
        "firewood" => "ph ph-terminal-window",
        _ => "ph ph-monitor",
    }
}

/// Display label for a device row / menu entry.
pub fn device_label(device: &NowPlayingDevice) -> String {
    if device.device_name.is_empty() {
        device.device_type.clone()
    } else {
        device.device_name.clone()
    }
}

/// Topbar button that toggles the devices drawer.
#[function_component(RemoteDevices)]
pub fn remote_devices() -> Html {
    let (np_state, np_dispatch) = use_store::<NowPlayingState>();
    let toggle = Callback::from(move |e: MouseEvent| {
        e.stop_propagation();
        np_dispatch.reduce_mut(|s| s.panel_open = !s.panel_open);
    });
    let device_count = np_state.devices.len();
    html! {
        <div class="remote-devices">
            <button
                type="button"
                class="iconbtn"
                title="Other devices"
                onclick={toggle}
            >
                <i class="ph ph-broadcast text-2xl"></i>
                if device_count > 0 {
                    <span class="nc-bell-badge">
                        { if device_count > 9 { "9+".to_string() } else { device_count.to_string() } }
                    </span>
                }
            </button>
        </div>
    }
}

/// The devices drawer + the observer that starts inbound remote plays on this
/// device. Mount exactly once, outside any responsive branch.
#[function_component(RemoteDevicesPanel)]
pub fn remote_devices_panel() -> Html {
    let (np_state, np_dispatch) = use_store::<NowPlayingState>();
    let (app_state, _) = use_store::<AppState>();
    let (ui_state, ui_dispatch) = use_store::<UIState>();
    let notif_dispatch = Dispatch::<NotificationState>::global();
    let open = np_state.panel_open;
    // Only the fact that something is playing here matters for "Send here".
    let local_playing = ui_state.currently_playing.is_some();

    let (i18n, _) = use_translation();
    let i18n_pause = i18n.t("remote_devices.pause").to_string();
    let i18n_play = i18n.t("remote_devices.play").to_string();
    let i18n_play_here = i18n.t("remote_devices.play_here").to_string();
    let i18n_send_here = i18n.t("remote_devices.send_here").to_string();
    let i18n_sent_to = i18n.t("remote_devices.sent_to").to_string();
    let i18n_other_devices = i18n.t("remote_devices.other_devices").to_string();
    let i18n_no_other_devices = i18n.t("remote_devices.no_other_devices").to_string();
    let i18n_no_devices_description = i18n.t("remote_devices.no_devices_description").to_string();

    // Observe "Play here" / inbound remote-play requests and start playback on THIS
    // device. This lives here (not in AudioPlayer) because AudioPlayer only mounts
    // once something is already playing locally — an idle tab must still be able to
    // START remote playback, and this component is always mounted in the topbar.
    {
        let np_dispatch = np_dispatch.clone();
        let server_name = app_state
            .auth_details
            .as_ref()
            .map(|ad| ad.server_name.clone());
        let api_key = app_state.auth_details.as_ref().map(|ad| ad.api_key.clone());
        let user_id = app_state.user_details.as_ref().map(|ud| ud.UserID);
        let ui_dispatch = ui_dispatch.clone();
        let ui_state = ui_state.clone();
        use_effect_with(np_state.pending_remote_play.clone(), move |pending| {
            if let Some(pending) = pending.clone() {
                let ep_id = pending.episode_id;
                let is_youtube = pending.is_youtube;
                let resume_pos_sec = pending.resume_pos_sec;
                // Clear immediately so it fires once.
                np_dispatch.reduce_mut(|s| s.pending_remote_play = None);
                if let (Some(server_name), Some(Some(api_key)), Some(user_id)) =
                    (server_name.clone(), api_key.clone(), user_id)
                {
                    let ui_dispatch = ui_dispatch.clone();
                    let ui_state = ui_state.clone();
                    spawn_local(async move {
                        let req = EpisodeRequest {
                            episode_id: ep_id,
                            user_id,
                            person_episode: false,
                            is_youtube,
                        };
                        match call_get_episode_metadata(&server_name, Some(api_key.clone()), &req)
                            .await
                        {
                            Ok(mut ep) => {
                                // Resume where the source device left off: on_play_click
                                // starts at episode.listenduration, so override it with the
                                // handed-off live position (fresher than the server's).
                                if let Some(pos) = resume_pos_sec {
                                    if pos.is_finite() && pos > 0.0 {
                                        ep.listenduration = pos as i32;
                                    }
                                }
                                on_play_click(
                                    ep,
                                    api_key,
                                    user_id,
                                    server_name,
                                    ui_dispatch,
                                    ui_state,
                                    false,
                                    false,
                                    None,
                                )
                                .emit(MouseEvent::new("click").unwrap());
                            }
                            Err(e) => web_sys::console::warn_1(
                                &format!("remote play_episode failed: {:?}", e).into(),
                            ),
                        }
                    });
                }
            }
            || ()
        });
    }

    let close = {
        let np_dispatch = np_dispatch.clone();
        Callback::from(move |_: MouseEvent| np_dispatch.reduce_mut(|s| s.panel_open = false))
    };
    // Stop clicks inside the panel from bubbling to the topbar / closing it.
    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    let render_device = {
        let np_dispatch = np_dispatch.clone();
        let i18n_pause = i18n_pause.clone();
        let i18n_play = i18n_play.clone();
        let i18n_play_here = i18n_play_here.clone();
        let i18n_send_here = i18n_send_here.clone();
        let i18n_sent_to = i18n_sent_to.clone();
        let notif_dispatch = notif_dispatch.clone();
        move |device: &NowPlayingDevice| -> Html {
            let id = device.device_id.clone();
            let is_playing = device.playing;
            // Connected but nothing loaded (episode_id 0): listed so it can be a
            // target, but there's nothing to transport-control or take over.
            let is_idle = device.episode_id == 0;

            let on_play_pause = {
                let id = id.clone();
                Callback::from(move |_: MouseEvent| {
                    let action = if is_playing { "pause" } else { "play" };
                    send_command(&id, action, json!({}));
                })
            };
            let on_back = {
                let id = id.clone();
                Callback::from(move |_: MouseEvent| {
                    send_command(&id, "skip_back", json!({ "seconds": 15 }));
                })
            };
            let on_fwd = {
                let id = id.clone();
                Callback::from(move |_: MouseEvent| {
                    send_command(&id, "skip_forward", json!({ "seconds": 30 }));
                })
            };
            let on_play_here = {
                let np_dispatch = np_dispatch.clone();
                let episode_id = device.episode_id;
                let is_youtube = device.is_youtube;
                let position_sec = device.position_sec;
                let source_id = id.clone();
                let source_playing = is_playing;
                Callback::from(move |_: MouseEvent| {
                    // Taking over playback: pause the source device (best-effort,
                    // advisory) so we're not playing the same thing in two places.
                    if source_playing {
                        send_command(&source_id, "pause", json!({}));
                    }
                    // Reuse the audio component's remote-play flow: it observes this
                    // field, fetches the episode, and starts it locally — resuming at
                    // the source's live position rather than the (staler) server one.
                    np_dispatch.reduce_mut(|s| {
                        s.pending_remote_play = Some(PendingRemotePlay {
                            episode_id,
                            is_youtube,
                            resume_pos_sec: Some(position_sec),
                        })
                    });
                })
            };

            let label = device_label(device);
            let on_send_here = {
                let id = id.clone();
                let np_dispatch = np_dispatch.clone();
                let notif_dispatch = notif_dispatch.clone();
                let toast = i18n_sent_to.replace("{name}", &label);
                Callback::from(move |_: MouseEvent| {
                    if send_playback_to(&id) {
                        toast_info(&notif_dispatch, toast.clone());
                        np_dispatch.reduce_mut(|s| s.panel_open = false);
                    }
                })
            };

            let track = if device.title.is_empty() {
                "Idle".to_string()
            } else {
                device.title.clone()
            };
            let progress = if device.duration_sec > 0.0 {
                format!("{} / {}", fmt_time(device.position_sec), fmt_time(device.duration_sec))
            } else {
                fmt_time(device.position_sec)
            };

            html! {
                <div class="rd-row">
                    <div class="rd-row-top">
                        <span class="rd-name">
                            <i class={ device_icon(&device.device_type) }></i>{ label }
                        </span>
                        <span class={classes!("rd-state", is_playing.then_some("is-playing"))}>
                            { if is_idle { "idle" } else if is_playing { "▶ playing" } else { "⏸ paused" } }
                        </span>
                    </div>
                    <span class="rd-track">{ track }</span>
                    if !is_idle {
                        <span class="rd-progress">{ progress }</span>
                    }
                    if !is_idle || local_playing {
                    <div class="rd-controls">
                        // type="button" on every one: a bare <button> defaults to
                        // submit, and this drawer used to render inside the topbar
                        // search <form> (an empty-query search ate the click).
                        if !is_idle {
                        <button type="button" class="iconbtn" title="Skip back 15s" onclick={on_back}>
                            <i class="ph ph-skip-back"></i>
                        </button>
                        <button type="button" class="iconbtn" title={ if is_playing { i18n_pause.clone() } else { i18n_play.clone() } } onclick={on_play_pause}>
                            <i class={ if is_playing { "ph ph-pause" } else { "ph ph-play" } }></i>
                        </button>
                        <button type="button" class="iconbtn" title="Skip forward 30s" onclick={on_fwd}>
                            <i class="ph ph-skip-forward"></i>
                        </button>
                        }
                        <div class="rd-actions">
                            if local_playing {
                                <button type="button" class="rd-play-here" title="Move what's playing here to this device" onclick={on_send_here}>
                                    <i class="ph ph-arrow-square-out mr-1"></i>{ &i18n_send_here }
                                </button>
                            }
                            if !is_idle {
                                <button type="button" class="rd-play-here" title="Play this here" onclick={on_play_here}>
                                    <i class="ph ph-monitor-play mr-1"></i>{ &i18n_play_here }
                                </button>
                            }
                        </div>
                    </div>
                    }
                </div>
            }
        }
    };

    html! {
        <>
            // Scrim + panel stay mounted; `.is-open` drives the two-way CSS
            // transition (mirrors the Activity drawer) so closing animates too.
            <div class={classes!("nc-scrim", open.then_some("is-open"))} onclick={close.clone()}></div>
            <div class={classes!("nc-panel", "nc-form-drawer", open.then_some("is-open"))}
                 data-density="compact" onclick={stop}>
                <div class="nc-header">
                    <h3 class="nc-h-title">{ &i18n_other_devices }</h3>
                    <div class="nc-h-spacer"></div>
                    <button type="button" class="nc-h-btn" title="Close" onclick={close.clone()}>
                        <i class="ph ph-arrow-line-right"></i>
                    </button>
                </div>
                <div class="nc-body">
                    if np_state.devices.is_empty() {
                        <div class="nc-empty">
                            <span class="nc-empty-ico"><i class="ph ph-broadcast"></i></span>
                            <div class="nc-empty-t">{ &i18n_no_other_devices }</div>
                            <div class="nc-empty-s">{ &i18n_no_devices_description }</div>
                        </div>
                    } else {
                        { for np_state.devices.iter().map(|d| render_device(d)) }
                    }
                </div>
            </div>
        </>
    }
}
