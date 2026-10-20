// Now-playing sessions + cross-device remote control (web client).
//
// A single persistent, authenticated WebSocket per browser tab reports this
// device's playback to the server (best-effort mirror) and receives the user's
// device list plus advisory remote-control commands. The local player stays
// authoritative — inbound commands are applied to it, never the other way around.

use futures::channel::mpsc;
use futures::{SinkExt, StreamExt};
use gloo::net::websocket::{futures::WebSocket, Message, WebSocketError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cell::RefCell;
use wasm_bindgen_futures::spawn_local;
use web_sys::console;
use yewdux::prelude::*;

use crate::components::context::{NotificationState, UIState};
use crate::requests::notify::toast_error;

/// Close code the server sends when a newer connection took over our device_id
/// (see rust-api handlers/nowplaying.rs `CLOSE_REPLACED`).
const CLOSE_REPLACED: u16 = 4000;

/// Mirror of the server's `NowPlayingSnapshot`.
#[derive(Deserialize, Serialize, Clone, PartialEq, Debug, Default)]
pub struct NowPlayingDevice {
    #[serde(default)]
    pub device_id: String,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub device_type: String,
    #[serde(default)]
    pub episode_id: i32,
    #[serde(default)]
    pub is_youtube: bool,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artwork_url: String,
    #[serde(default)]
    pub position_sec: f64,
    #[serde(default)]
    pub duration_sec: f64,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub speed: f64,
    #[serde(default)]
    pub updated_at: i64,
    /// Actions the device says it applies. Empty: it predates the `capabilities`
    /// connect parameter, so it handles `LEGACY_ACTIONS`.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// What every client applied before devices could list their actions.
const LEGACY_ACTIONS: &[&str] = &[
    "play",
    "pause",
    "seek",
    "skip_forward",
    "skip_back",
    "set_speed",
    "play_episode",
];

/// The actions `apply_command` handles, announced on connect.
const CAPABILITIES: &str = "play,pause,seek,skip_forward,skip_back,set_speed,play_episode";

impl NowPlayingDevice {
    /// Whether this device applies `action`, so UIs can hide controls it lacks.
    pub fn supports(&self, action: &str) -> bool {
        if self.capabilities.is_empty() {
            LEGACY_ACTIONS.contains(&action)
        } else {
            self.capabilities.iter().any(|c| c == action)
        }
    }
}

/// A request to start an episode on THIS device — either from the local "Play
/// here" button (carrying the source device's live position so we resume exactly
/// where it left off) or from an inbound remote `play_episode` command (which
/// carries `position_sec` for a handoff, or nothing → server-stored position).
#[derive(Clone, PartialEq, Debug)]
pub struct PendingRemotePlay {
    pub episode_id: i32,
    pub is_youtube: bool,
    /// Where to resume, in seconds. `None` → fall back to the server-stored
    /// listen position (used for inbound commands with no position hint).
    pub resume_pos_sec: Option<f64>,
}

/// Reactive store for the remote-control UI. `devices` excludes this device.
#[derive(Default, Clone, PartialEq, Store)]
pub struct NowPlayingState {
    pub self_device_id: String,
    pub devices: Vec<NowPlayingDevice>,
    pub connected: bool,
    /// Set when an episode should start on this device (local "Play here" or an
    /// inbound command); the audio component observes this, plays, and clears it.
    pub pending_remote_play: Option<PendingRemotePlay>,
    /// Whether the devices drawer is open. Lives in the store (not component
    /// state) so the full-screen player can open it too.
    pub panel_open: bool,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg {
    Devices {
        devices: Vec<NowPlayingDevice>,
    },
    Command {
        #[allow(dead_code)]
        from_device_id: String,
        action: String,
        #[serde(default)]
        args: Value,
    },
    Ack {
        ok: bool,
        #[allow(dead_code)]
        detail: String,
        #[serde(default)]
        cmd_id: Option<String>,
        #[serde(default)]
        target_device_id: String,
    },
}

thread_local! {
    // Outbound queue sender for the live socket; None when disconnected.
    static NP_TX: RefCell<Option<mpsc::UnboundedSender<String>>> = const { RefCell::new(None) };
    // True once the connection loop is running, so repeated bootstrap calls
    // (e.g. re-render / creds arriving late) don't spawn duplicate sockets.
    static NP_RUNNING: RefCell<bool> = const { RefCell::new(false) };
    // Monotonic per-tab counter for command correlation ids.
    static NP_CMD_SEQ: RefCell<u64> = const { RefCell::new(0) };
    // cmd_id of an in-flight "Send to" handoff; we pause locally only once the
    // server acks delivery, so an offline target doesn't strand playback.
    static NP_PENDING_HANDOFF: RefCell<Option<String>> = const { RefCell::new(None) };
}

const DEVICE_ID_KEY: &str = "pinepods_device_id";

/// Stable per-TAB device id. Each tab is its own player, so the id lives in
/// sessionStorage: it survives reloads of the tab but isn't shared with other tabs
/// (a shared localStorage id made tabs evict each other's registration). A
/// duplicated tab inherits sessionStorage; the server then closes the older socket
/// with `CLOSE_REPLACED` and that tab rotates its id via [`rotate_device_id`].
pub fn get_or_create_device_id() -> String {
    if let Some(win) = web_sys::window() {
        if let Ok(Some(storage)) = win.session_storage() {
            if let Ok(Some(id)) = storage.get_item(DEVICE_ID_KEY) {
                if !id.is_empty() {
                    return id;
                }
            }
            let id = generate_id();
            let _ = storage.set_item(DEVICE_ID_KEY, &id);
            return id;
        }
    }
    generate_id()
}

/// Replace this tab's device id with a fresh one (after another tab took it over).
fn rotate_device_id() -> String {
    let id = generate_id();
    if let Some(Ok(Some(storage))) = web_sys::window().map(|w| w.session_storage()) {
        let _ = storage.set_item(DEVICE_ID_KEY, &id);
    }
    id
}

fn generate_id() -> String {
    let mut s = String::new();
    for _ in 0..4 {
        let n = (js_sys::Math::random() * 4_294_967_296.0) as u32;
        s.push_str(&format!("{:08x}", n));
    }
    s
}

fn device_name() -> String {
    let ua = web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .unwrap_or_default();
    let browser = if ua.contains("Edg") {
        "Edge"
    } else if ua.contains("Firefox") {
        "Firefox"
    } else if ua.contains("Chrome") {
        "Chrome"
    } else if ua.contains("Safari") {
        "Safari"
    } else {
        "Browser"
    };
    format!("Web · {}", browser)
}

fn enc(s: &str) -> String {
    String::from(js_sys::encode_uri_component(s))
}

/// Open the persistent now-playing socket and keep it open (auto-reconnecting).
/// Safe to call repeatedly — a thread-local guard ensures only one connection
/// loop runs per tab, so the caller can invoke it whenever credentials become
/// available without spawning duplicates. Reconnection is best-effort; a dropped
/// socket costs remote control, never local playback.
pub fn connect_now_playing_ws(server_name: String, user_id: i32, api_key: String) {
    // Only one connection loop per tab.
    let already_running = NP_RUNNING.with(|c| {
        let was = *c.borrow();
        *c.borrow_mut() = true;
        was
    });
    if already_running {
        return;
    }

    let mut device_id = get_or_create_device_id();
    let dev_name = device_name();
    let dev_type = "Web".to_string();

    let clean = server_name
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .to_string();
    let proto = if server_name.starts_with("https://") {
        "wss://"
    } else {
        "ws://"
    };

    spawn_local(async move {
        // Reconnect loop: a WebSocket can drop on a server restart, an nginx idle
        // timeout, or a transient network blip. Without this the device panel goes
        // permanently stale until a full page reload (the bug users hit).
        loop {
            Dispatch::<NowPlayingState>::global()
                .reduce_mut(|s| s.self_device_id = device_id.clone());
            let url = format!(
                "{}{}/ws/api/nowplaying/{}?api_key={}&device_id={}&device_name={}&device_type={}&capabilities={}",
                proto,
                clean,
                user_id,
                enc(&api_key),
                enc(&device_id),
                enc(&dev_name),
                enc(&dev_type),
                enc(CAPABILITIES),
            );
            match WebSocket::open(&url) {
                Ok(ws) => {
                    let (mut write, mut read) = ws.split();
                    let (tx, mut rx) = mpsc::unbounded::<String>();
                    NP_TX.with(|c| *c.borrow_mut() = Some(tx));
                    Dispatch::<NowPlayingState>::global().reduce_mut(|s| s.connected = true);

                    // Writer: drain outbound queue to the socket.
                    spawn_local(async move {
                        while let Some(msg) = rx.next().await {
                            if write.send(Message::Text(msg)).await.is_err() {
                                break;
                            }
                        }
                    });

                    // Reader: dispatch inbound frames until the socket closes.
                    let mut replaced = false;
                    while let Some(frame) = read.next().await {
                        match frame {
                            Ok(Message::Text(text)) => handle_inbound(&text),
                            Ok(_) => {}
                            Err(WebSocketError::ConnectionClose(ev)) => {
                                replaced = ev.code == CLOSE_REPLACED;
                                break;
                            }
                            Err(_) => break,
                        }
                    }

                    // Disconnected — clear state, then fall through to reconnect.
                    NP_TX.with(|c| *c.borrow_mut() = None);
                    Dispatch::<NowPlayingState>::global().reduce_mut(|s| {
                        s.connected = false;
                        s.devices.clear();
                    });

                    // Another tab (e.g. a duplicated one) took over our id: take a
                    // fresh one so the two tabs stop evicting each other.
                    if replaced {
                        device_id = rotate_device_id();
                        continue;
                    }
                }
                Err(e) => {
                    console::warn_1(&format!("now-playing WebSocket failed: {:?}", e).into());
                }
            }

            // Best-effort backoff before reconnecting.
            gloo_timers::future::TimeoutFuture::new(5_000).await;
        }
    });
}

fn send_raw(json: String) {
    NP_TX.with(|c| {
        if let Some(tx) = c.borrow().as_ref() {
            let _ = tx.unbounded_send(json);
        }
    });
}

pub fn is_connected() -> bool {
    NP_TX.with(|c| c.borrow().is_some())
}

/// Report this device's current playback state (best-effort).
#[allow(clippy::too_many_arguments)]
pub fn send_report(
    episode_id: i32,
    is_youtube: bool,
    title: &str,
    artwork_url: &str,
    position_sec: f64,
    duration_sec: f64,
    playing: bool,
    speed: f64,
) {
    send_raw(
        json!({
            "type": "report",
            "episode_id": episode_id,
            "is_youtube": is_youtube,
            "title": title,
            "artwork_url": artwork_url,
            "position_sec": position_sec,
            "duration_sec": duration_sec,
            "playing": playing,
            "speed": speed,
        })
        .to_string(),
    );
}

pub fn send_heartbeat() {
    send_raw(json!({ "type": "heartbeat" }).to_string());
}

/// Send an advisory remote-control command to another device. If the server
/// reports the target offline, the ack surfaces as an error toast.
/// Send an advisory command to another device. Returns the correlation id the
/// server echoes back in its `ack`.
pub fn send_command(target_device_id: &str, action: &str, args: Value) -> String {
    let cmd_id = NP_CMD_SEQ.with(|c| {
        let mut c = c.borrow_mut();
        *c += 1;
        c.to_string()
    });
    send_raw(
        json!({
            "type": "command",
            "target_device_id": target_device_id,
            "action": action,
            "args": args,
            "cmd_id": cmd_id,
        })
        .to_string(),
    );
    cmd_id
}

/// "Play on ▸": start an episode on another device. With no position the target
/// resumes from the server-stored listen position.
pub fn play_on_device(target_device_id: &str, episode_id: i32, is_youtube: bool) {
    send_command(
        target_device_id,
        "play_episode",
        json!({ "episode_id": episode_id, "is_youtube": is_youtube }),
    );
}

/// "Send to ▸": hand off what this tab is playing to another device at our live
/// position — the push mirror of "Play here". The local player pauses once the
/// server acks delivery. Returns false when nothing is playing here.
pub fn send_playback_to(target_device_id: &str) -> bool {
    let state = Dispatch::<UIState>::global().get();
    let Some(cp) = state.currently_playing.as_ref() else {
        return false;
    };
    let position = state
        .media_element
        .as_ref()
        .map(|m| m.current_time())
        .unwrap_or(state.current_time_seconds);
    let cmd_id = send_command(
        target_device_id,
        "play_episode",
        json!({
            "episode_id": cp.episode_id,
            "is_youtube": cp.is_youtube,
            "position_sec": position,
        }),
    );
    NP_PENDING_HANDOFF.with(|h| *h.borrow_mut() = Some(cmd_id));
    true
}

/// Take (and clear) the pending handoff if `cmd_id` is the one it's waiting on.
fn take_handoff(cmd_id: &Option<String>) -> bool {
    NP_PENDING_HANDOFF.with(|h| {
        let mut h = h.borrow_mut();
        if cmd_id.is_some() && *h == *cmd_id {
            *h = None;
            true
        } else {
            false
        }
    })
}

fn handle_inbound(text: &str) {
    match serde_json::from_str::<ServerMsg>(text) {
        Ok(ServerMsg::Devices { devices }) => {
            Dispatch::<NowPlayingState>::global().reduce_mut(|s| {
                let self_id = s.self_device_id.clone();
                s.devices = devices
                    .into_iter()
                    .filter(|d| d.device_id != self_id)
                    .collect();
            });
        }
        Ok(ServerMsg::Command { action, args, .. }) => apply_command(&action, &args),
        Ok(ServerMsg::Ack {
            ok: false,
            cmd_id,
            target_device_id,
            ..
        }) => {
            // A failed handoff leaves local playback running.
            take_handoff(&cmd_id);
            let name = Dispatch::<NowPlayingState>::global()
                .get()
                .devices
                .iter()
                .find(|d| d.device_id == target_device_id)
                .map(|d| d.device_name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "That device".to_string());
            toast_error(
                &Dispatch::<NotificationState>::global(),
                format!("{} is offline — command not delivered", name),
            );
        }
        Ok(ServerMsg::Ack {
            ok: true, cmd_id, ..
        }) => {
            if take_handoff(&cmd_id) {
                apply_command("pause", &Value::Null);
            }
        }
        Err(_) => {}
    }
}

/// Apply an inbound remote command to the local player. Transport commands act
/// directly on the media element; `play_episode` is handed to the audio component
/// via `pending_remote_play` (it owns the fetch + start-playback flow).
fn apply_command(action: &str, args: &Value) {
    let ui = Dispatch::<UIState>::global();
    match action {
        "play" => {
            if let Some(me) = ui.get().media_element.as_ref() {
                let _ = me.play();
            }
            ui.reduce_mut(|s| s.audio_playing = Some(true));
        }
        "pause" => {
            if let Some(me) = ui.get().media_element.as_ref() {
                let _ = me.pause();
            }
            ui.reduce_mut(|s| s.audio_playing = Some(false));
        }
        "seek" => {
            if let Some(sec) = args.get("position_sec").and_then(Value::as_f64) {
                if let Some(me) = ui.get().media_element.as_ref() {
                    me.set_current_time(sec.max(0.0));
                }
            }
        }
        "skip_forward" => {
            let by = args.get("seconds").and_then(Value::as_f64).unwrap_or(30.0);
            seek_relative(&ui, by);
        }
        "skip_back" => {
            let by = args.get("seconds").and_then(Value::as_f64).unwrap_or(15.0);
            seek_relative(&ui, -by);
        }
        "set_speed" => {
            if let Some(rate) = args.get("speed").and_then(Value::as_f64) {
                if let Some(me) = ui.get().media_element.as_ref() {
                    me.set_playback_rate(rate);
                }
                ui.reduce_mut(|s| s.playback_speed = rate);
            }
        }
        "play_episode" => {
            if let Some(id) = args.get("episode_id").and_then(Value::as_i64) {
                let yt = args
                    .get("is_youtube")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                // A handoff carries the sender's live position; without one,
                // resume from the server-stored listen position.
                let resume_pos_sec = args
                    .get("position_sec")
                    .and_then(Value::as_f64)
                    .filter(|p| p.is_finite() && *p > 0.0);
                Dispatch::<NowPlayingState>::global().reduce_mut(|s| {
                    s.pending_remote_play = Some(PendingRemotePlay {
                        episode_id: id as i32,
                        is_youtube: yt,
                        resume_pos_sec,
                    })
                });
            }
        }
        _ => {}
    }
}

fn seek_relative(ui: &Dispatch<UIState>, delta: f64) {
    let state = ui.get();
    if let Some(me) = state.media_element.as_ref() {
        let target = (state.current_time_seconds + delta).max(0.0);
        me.set_current_time(target);
    }
}
