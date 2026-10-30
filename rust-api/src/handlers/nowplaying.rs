//! Now-playing sessions + cross-device remote control.
//!
//! Every client (web tab, mobile app, Firewood, …) holds one full-duplex socket at
//! `/ws/api/nowplaying/{user_id}?api_key=…&device_id=…&device_name=…&device_type=…`.
//! The server is a relay ("aware, not owns"): devices report what they are playing,
//! the server mirrors that into Valkey with a short TTL and fans the device list out,
//! and remote-control commands are passed through to the target device, which
//! applies them to its own (authoritative) player. The server never sits in the
//! playback path — a dead socket costs remote control, never local playback.
//!
//! ## Connection
//! - Auth: `api_key` must belong to `user_id` (or be the system user 1). Browsers
//!   can't set WS headers, hence the query param.
//! - `device_id`: stable, client-generated, unique per player instance. A second
//!   connection with the same `device_id` *replaces* the first; the old socket is
//!   closed with code [`CLOSE_REPLACED`] (4000) and should not blindly reconnect
//!   with the same id.
//! - `device_name` / `device_type`: free-form display strings (`"Web"`, `"iOS"`,
//!   `"Android"`, `"Firewood"`, …). The server never interprets them.
//! - `capabilities` (optional): comma-separated action names the device applies,
//!   e.g. `play,pause,seek,skip_forward,skip_back,set_speed,play_episode`. Echoed in
//!   its snapshot as a `capabilities` list so other UIs can hide controls it lacks.
//!   Absent or empty means unknown: assume the actions listed below, which every
//!   client predating the parameter supports. The server doesn't interpret it.
//! - On connect the device is immediately listed as idle (`episode_id: 0`) so it can
//!   be targeted (e.g. `play_episode`) before it has played anything.
//! - The server pings every 30s and drops a socket that has sent nothing (including
//!   pongs) for 75s. Clients must answer pings (browsers and most WS libs do this
//!   automatically).
//!
//! ## Client → server (JSON, `type`-tagged)
//! - `{"type":"report","episode_id":123,"is_youtube":false,"title":"…","artwork_url":"…",
//!    "position_sec":812.4,"duration_sec":3600,"playing":true,"speed":1.0}` —
//!   current playback state. Send on every state change and periodically (well
//!   inside the 45s snapshot TTL). `episode_id: 0` means idle.
//! - `{"type":"heartbeat"}` — keep the snapshot alive without a state change;
//!   re-lists the device as idle if its snapshot already expired.
//! - `{"type":"command","target_device_id":"…","action":"pause","args":{},"cmd_id":"…"}`
//!   — advisory command for another of the user's devices. `cmd_id` is optional
//!   and echoed back in the ack.
//!
//! ## Server → client
//! - `{"type":"devices","devices":[NowPlayingSnapshot…]}` — the user's devices
//!   (including the receiver; filter yourself out by `device_id`).
//! - `{"type":"command","from_device_id":"…","action":"…","args":{…}}` — apply to
//!   the local player; ignore unknown actions.
//! - `{"type":"ack","ok":true,"detail":"relayed","cmd_id":"…","target_device_id":"…"}`
//!   — `ok:false` means the target is offline. Delivery is not execution.
//!
//! ## Actions (args)
//! `play`, `pause`, `seek {position_sec}`, `skip_forward {seconds=30}`,
//! `skip_back {seconds=15}`, `set_speed {speed}`,
//! `play_episode {episode_id, is_youtube, position_sec?}` — start an episode on the
//! target; with `position_sec` resume exactly there (handoff), otherwise from the
//! server-stored listen position.

use axum::{
    extract::{
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::HeaderMap,
    response::{Json, Response},
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};

use crate::{
    error::AppError,
    handlers::{check_user_access, extract_api_key},
    models::NowPlayingSnapshot,
    AppState,
};

/// Close code sent to a socket whose `device_id` was taken over by a newer
/// connection (application range 4000-4999).
pub const CLOSE_REPLACED: u16 = 4000;

/// How often the server pings each socket, and how long a socket may stay silent
/// (no frames, no pongs) before it is treated as dead (half-open TCP, sleeping
/// laptop, dropped NAT mapping) and torn down.
const PING_INTERVAL: Duration = Duration::from_secs(30);
const IDLE_TIMEOUT: Duration = Duration::from_secs(75);

// ============================ HTTP: list devices ============================

#[derive(Deserialize, utoipa::IntoParams)]
pub struct NowPlayingDevicesQuery {
    pub user_id: i32,
}

/// List the user's active devices and what each is currently playing. Backed by
/// ephemeral Valkey snapshots with a TTL, so a device that stops reporting drops
/// off automatically. Powers the "see / control my other devices" UI for clients
/// that aren't holding a live socket.
#[utoipa::path(
    get,
    path = "/now_playing_devices",
    tag = "nowplaying",
    params(NowPlayingDevicesQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Active devices and their now-playing state", body = crate::models::NowPlayingDevicesResponse),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not authorized for this user"),
    ),
)]
pub async fn now_playing_devices(
    Query(query): Query<NowPlayingDevicesQuery>,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<crate::models::NowPlayingDevicesResponse>, AppError> {
    let api_key = extract_api_key(&headers)?;

    if !state.db_pool.verify_api_key(&api_key).await? {
        return Err(AppError::unauthorized("Invalid API key"));
    }
    if !check_user_access(&state, &api_key, query.user_id).await? {
        return Err(AppError::forbidden("You can only view your own devices!"));
    }

    let devices = state
        .redis_client
        .get_now_playing_devices(query.user_id)
        .await?;

    Ok(Json(crate::models::NowPlayingDevicesResponse { devices }))
}

// ============================ Connection registry ============================

struct DeviceConn {
    /// Unique per socket, so a stale socket's teardown can't evict the newer
    /// connection that replaced it under the same `device_id`.
    conn_id: u64,
    tx: mpsc::UnboundedSender<Message>,
}

type DeviceSenders = HashMap<String, DeviceConn>;

/// In-process registry of live now-playing sockets, keyed by `(user_id, device_id)`
/// so the server can address or exclude an individual device (unlike the task/refresh
/// sockets, which are keyed by user only). Cross-instance fan-out goes over Valkey
/// pub/sub; within one instance this is the source of routing.
#[derive(Default)]
pub struct NowPlayingManager {
    connections: RwLock<HashMap<i32, DeviceSenders>>,
    next_conn_id: AtomicU64,
}

impl NowPlayingManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a socket for a device and return its connection id. If the device
    /// already had a live socket (reconnect racing the old teardown, or two
    /// players sharing an id), the old one is told to close with [`CLOSE_REPLACED`].
    async fn add(&self, user_id: i32, device_id: String, tx: mpsc::UnboundedSender<Message>) -> u64 {
        let conn_id = self.next_conn_id.fetch_add(1, Ordering::Relaxed);
        let previous = self
            .connections
            .write()
            .await
            .entry(user_id)
            .or_default()
            .insert(device_id, DeviceConn { conn_id, tx });
        if let Some(previous) = previous {
            let _ = previous.tx.send(Message::Close(Some(CloseFrame {
                code: CLOSE_REPLACED,
                reason: "replaced by a newer connection for this device".into(),
            })));
        }
        conn_id
    }

    /// Unregister a socket. Only removes the entry if it still belongs to
    /// `conn_id`; returns whether it did (i.e. whether this socket was still the
    /// device's live connection and the device is now actually gone).
    async fn remove(&self, user_id: i32, device_id: &str, conn_id: u64) -> bool {
        let mut conns = self.connections.write().await;
        let Some(devs) = conns.get_mut(&user_id) else {
            return false;
        };
        let owned = devs.get(device_id).is_some_and(|c| c.conn_id == conn_id);
        if owned {
            devs.remove(device_id);
            if devs.is_empty() {
                conns.remove(&user_id);
            }
        }
        owned
    }

    /// Relay a message to one specific device. Returns true if a live connection
    /// accepted it (the receiver still applies it locally — delivery is not
    /// execution). Non-blocking: an unbounded send never awaits.
    async fn send_to_device(&self, user_id: i32, device_id: &str, msg: Message) -> bool {
        let conns = self.connections.read().await;
        conns
            .get(&user_id)
            .and_then(|devs| devs.get(device_id))
            .map(|c| c.tx.send(msg).is_ok())
            .unwrap_or(false)
    }

    /// Send a message to all of a user's connected devices except the origin.
    async fn broadcast_user_except(&self, user_id: i32, except_device: &str, msg: Message) {
        let conns = self.connections.read().await;
        if let Some(devs) = conns.get(&user_id) {
            for (device_id, c) in devs.iter() {
                if device_id != except_device {
                    let _ = c.tx.send(msg.clone());
                }
            }
        }
    }
}

// ============================ Wire protocol ============================

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMsg {
    /// This device's current playback state (sent on the client's playback tick).
    Report {
        episode_id: i32,
        #[serde(default)]
        is_youtube: bool,
        #[serde(default)]
        title: String,
        #[serde(default)]
        artwork_url: String,
        #[serde(default)]
        position_sec: f64,
        #[serde(default)]
        duration_sec: f64,
        #[serde(default)]
        playing: bool,
        #[serde(default)]
        speed: f64,
    },
    /// Keep the snapshot TTL alive without a state change (e.g. while paused).
    Heartbeat,
    /// Advisory remote-control command aimed at another of the user's devices.
    Command {
        target_device_id: String,
        action: String,
        #[serde(default)]
        args: Value,
        /// Optional client correlation id, echoed in the ack.
        #[serde(default)]
        cmd_id: Option<String>,
    },
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMsg {
    /// The user's current set of active devices and their now-playing state.
    Devices { devices: Vec<NowPlayingSnapshot> },
    /// A command relayed from another device, to be applied on the local player.
    Command {
        from_device_id: String,
        action: String,
        args: Value,
    },
    /// Result of a command the client sent (relayed vs. target offline).
    Ack {
        ok: bool,
        detail: String,
        cmd_id: Option<String>,
        target_device_id: String,
    },
}

fn to_ws(msg: &ServerMsg) -> Message {
    Message::Text(
        serde_json::to_string(msg)
            .unwrap_or_else(|_| "{}".to_string())
            .into(),
    )
}

fn forbidden(detail: &'static str) -> Response {
    axum::response::Response::builder()
        .status(403)
        .body(detail.into())
        .unwrap()
}

// ============================ WebSocket: now playing ============================

#[derive(Deserialize)]
pub struct NowPlayingWsQuery {
    api_key: String,
    device_id: String,
    #[serde(default)]
    device_name: String,
    #[serde(default)]
    device_type: String,
    #[serde(default)]
    capabilities: String,
}

/// Most a device can claim, and the longest action name kept, so a client can't
/// bloat every device list it's part of.
const MAX_CAPABILITIES: usize = 32;
const MAX_CAPABILITY_LEN: usize = 32;

/// `"play, pause,,seek"` → `["play", "pause", "seek"]`.
fn parse_capabilities(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|c| !c.is_empty() && c.len() <= MAX_CAPABILITY_LEN)
        .take(MAX_CAPABILITIES)
        .map(str::to_string)
        .collect()
}

/// Full-duplex now-playing socket: `/ws/api/nowplaying/{user_id}`. Carries device
/// reports (client→server) and device-list updates + relayed commands (server→client).
/// Authenticated by `?api_key=` (browsers can't set WS headers), owner-or-system-user.
pub async fn now_playing_websocket(
    ws: WebSocketUpgrade,
    Path(user_id): Path<i32>,
    Query(query): Query<NowPlayingWsQuery>,
    State(state): State<AppState>,
) -> Response {
    match state.db_pool.verify_api_key(&query.api_key).await {
        Ok(true) => match state.db_pool.get_user_id_from_api_key(&query.api_key).await {
            Ok(key_user_id) if key_user_id == user_id || key_user_id == 1 => {
                ws.on_upgrade(move |socket| handle_now_playing_socket(socket, user_id, query, state))
            }
            Ok(_) => forbidden("API key does not belong to requested user"),
            Err(_) => forbidden("Invalid API key"),
        },
        Ok(false) | Err(_) => forbidden("Invalid API key"),
    }
}

async fn handle_now_playing_socket(
    socket: WebSocket,
    user_id: i32,
    query: NowPlayingWsQuery,
    state: AppState,
) {
    let device_id = query.device_id.clone();
    let device_name = query.device_name.clone();
    let device_type = query.device_type.clone();
    let capabilities = parse_capabilities(&query.capabilities);

    let (mut sink, mut stream) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let conn_id = state
        .now_playing_manager
        .add(user_id, device_id.clone(), tx.clone())
        .await;

    // List the device right away (as idle unless a fresher snapshot already
    // exists) so it can be targeted before it has played anything, then let the
    // user's other devices know it arrived.
    let _ = state
        .redis_client
        .ensure_now_playing_present(
            user_id,
            idle_snapshot(&device_id, &device_name, &device_type, &capabilities),
        )
        .await;
    notify_devices_changed(&state, user_id, &device_id).await;

    // Send the current device list immediately on connect.
    let devices = state
        .redis_client
        .get_now_playing_devices(user_id)
        .await
        .unwrap_or_default();
    let _ = tx.send(to_ws(&ServerMsg::Devices { devices }));

    // Liveness: any inbound frame (incl. pongs) bumps this; the pinger tears the
    // socket down if it goes quiet for too long.
    let started = Instant::now();
    let last_seen_ms = Arc::new(AtomicU64::new(0));

    // Writer: drain queued messages to the socket. A queued Close (sent when a
    // newer connection replaced this one) is flushed and ends the socket.
    let mut writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let closing = matches!(msg, Message::Close(_));
            if sink.send(msg).await.is_err() || closing {
                break;
            }
        }
    });

    // Pinger / watchdog.
    let ping_tx = tx.clone();
    let ping_seen = last_seen_ms.clone();
    let mut pinger = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(PING_INTERVAL);
        ticker.tick().await; // first tick fires immediately
        loop {
            ticker.tick().await;
            let now_ms = started.elapsed().as_millis() as u64;
            let silent_ms = now_ms.saturating_sub(ping_seen.load(Ordering::Relaxed));
            if silent_ms > IDLE_TIMEOUT.as_millis() as u64 {
                tracing::debug!("now-playing socket silent for {silent_ms}ms; dropping");
                break;
            }
            if ping_tx.send(Message::Ping(Vec::new().into())).is_err() {
                break;
            }
        }
    });

    // Reader: handle inbound client messages.
    let reader_state = state.clone();
    let reader_device = device_id.clone();
    let reader_name = device_name.clone();
    let reader_type = device_type.clone();
    let reader_caps = capabilities.clone();
    let reader_tx = tx.clone();
    let reader_seen = last_seen_ms.clone();
    let mut reader = tokio::spawn(async move {
        while let Some(Ok(msg)) = stream.next().await {
            reader_seen.store(started.elapsed().as_millis() as u64, Ordering::Relaxed);
            match msg {
                Message::Text(text) => {
                    let parsed: Result<ClientMsg, _> = serde_json::from_str(text.as_str());
                    match parsed {
                        Ok(ClientMsg::Report {
                            episode_id,
                            is_youtube,
                            title,
                            artwork_url,
                            position_sec,
                            duration_sec,
                            playing,
                            speed,
                        }) => {
                            let snapshot = NowPlayingSnapshot {
                                device_id: reader_device.clone(),
                                device_name: reader_name.clone(),
                                device_type: reader_type.clone(),
                                episode_id,
                                is_youtube,
                                title,
                                artwork_url,
                                position_sec,
                                duration_sec,
                                playing,
                                speed,
                                updated_at: 0, // stamped by upsert
                                capabilities: reader_caps.clone(),
                            };
                            let _ = reader_state
                                .redis_client
                                .upsert_now_playing(user_id, snapshot)
                                .await;
                            notify_devices_changed(&reader_state, user_id, &reader_device).await;
                        }
                        Ok(ClientMsg::Heartbeat) => {
                            let alive = reader_state
                                .redis_client
                                .touch_now_playing(user_id, &reader_device)
                                .await
                                .unwrap_or(true);
                            // The snapshot expired (e.g. a long quiet spell): re-list
                            // the device as idle rather than letting it vanish while
                            // its socket is still open.
                            if !alive {
                                let _ = reader_state
                                    .redis_client
                                    .ensure_now_playing_present(
                                        user_id,
                                        idle_snapshot(
                                            &reader_device,
                                            &reader_name,
                                            &reader_type,
                                            &reader_caps,
                                        ),
                                    )
                                    .await;
                                notify_devices_changed(&reader_state, user_id, &reader_device).await;
                            }
                        }
                        Ok(ClientMsg::Command {
                            target_device_id,
                            action,
                            args,
                            cmd_id,
                        }) => {
                            let relay = to_ws(&ServerMsg::Command {
                                from_device_id: reader_device.clone(),
                                action: action.clone(),
                                args: args.clone(),
                            });
                            let delivered = reader_state
                                .now_playing_manager
                                .send_to_device(user_id, &target_device_id, relay)
                                .await;
                            let (ok, detail) = if delivered {
                                (true, "relayed")
                            } else {
                                // Target isn't on this instance — fan the command out
                                // over pub/sub so whichever replica holds it delivers.
                                let other_instance = publish_command(
                                    &reader_state,
                                    user_id,
                                    &target_device_id,
                                    &reader_device,
                                    &action,
                                    &args,
                                )
                                .await;
                                // Another replica merely *listening* doesn't mean it
                                // holds the target: also require the target to still be
                                // present (every live socket keeps a snapshot).
                                let present = other_instance
                                    && reader_state
                                        .redis_client
                                        .now_playing_exists(user_id, &target_device_id)
                                        .await
                                        .unwrap_or(false);
                                if present {
                                    (true, "relayed to another instance")
                                } else {
                                    (false, "device offline")
                                }
                            };
                            let _ = reader_tx.send(to_ws(&ServerMsg::Ack {
                                ok,
                                detail: detail.to_string(),
                                cmd_id,
                                target_device_id,
                            }));
                        }
                        Err(_) => { /* ignore malformed frames */ }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // When any task ends, stop the others and tear down.
    tokio::select! {
        _ = &mut writer => {},
        _ = &mut reader => {},
        _ = &mut pinger => {},
    }
    writer.abort();
    reader.abort();
    pinger.abort();

    // Only clean up if this socket still owns the device. If a newer connection
    // replaced it, that connection owns the registry entry and the snapshot.
    if state
        .now_playing_manager
        .remove(user_id, &device_id, conn_id)
        .await
    {
        let _ = state
            .redis_client
            .remove_now_playing(user_id, &device_id)
            .await;
        notify_devices_changed(&state, user_id, &device_id).await;
    }
}

/// The snapshot a connected device has before it reports any playback.
fn idle_snapshot(
    device_id: &str,
    device_name: &str,
    device_type: &str,
    capabilities: &[String],
) -> NowPlayingSnapshot {
    NowPlayingSnapshot {
        device_id: device_id.to_string(),
        device_name: device_name.to_string(),
        device_type: device_type.to_string(),
        episode_id: 0,
        is_youtube: false,
        title: String::new(),
        artwork_url: String::new(),
        position_sec: 0.0,
        duration_sec: 0.0,
        playing: false,
        speed: 1.0,
        updated_at: 0,
        capabilities: capabilities.to_vec(),
    }
}

/// Push the freshly-computed device list to all of the user's *other* connected
/// devices on THIS instance.
async fn broadcast_devices(state: &AppState, user_id: i32, origin_device: &str) {
    let devices = state
        .redis_client
        .get_now_playing_devices(user_id)
        .await
        .unwrap_or_default();
    state
        .now_playing_manager
        .broadcast_user_except(user_id, origin_device, to_ws(&ServerMsg::Devices { devices }))
        .await;
}

// ======================= Multi-replica pub/sub bridge =======================
//
// The connection registry is in-process, so with more than one API replica a
// report/command handled on replica A must reach the sockets on replica B. We
// bridge those two events over a single Valkey channel; snapshots already live in
// shared Valkey, so device lists are correct cross-instance without any sync.
// Everything degrades cleanly to in-process fan-out when pub/sub is unavailable
// (the default single-container deploy never needs the bridge).

const NOW_PLAYING_CHANNEL: &str = "nowplaying:events";

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum NpBridgeMsg {
    /// A user's device list changed (report/disconnect) — other replicas should
    /// re-broadcast the fresh list to their local sockets for that user.
    DevicesChanged {
        user_id: i32,
        origin_device: String,
        instance_id: String,
    },
    /// A command whose target device wasn't connected to the origin instance —
    /// whichever replica holds the target delivers it.
    Command {
        user_id: i32,
        target_device_id: String,
        from_device_id: String,
        action: String,
        args: Value,
        instance_id: String,
    },
}

impl NpBridgeMsg {
    fn instance_id(&self) -> &str {
        match self {
            NpBridgeMsg::DevicesChanged { instance_id, .. } => instance_id,
            NpBridgeMsg::Command { instance_id, .. } => instance_id,
        }
    }
}

/// Publish a bridge message; returns the number of subscribers that received it
/// (0 on serialize/publish failure). Our own bridge subscription counts as one.
async fn publish_bridge(state: &AppState, msg: &NpBridgeMsg) -> i64 {
    match serde_json::to_string(msg) {
        Ok(payload) => state
            .redis_client
            .publish(NOW_PLAYING_CHANNEL, &payload)
            .await
            .unwrap_or(0),
        Err(_) => 0,
    }
}

/// Broadcast the fresh device list to this instance's other sockets AND fan the
/// change out to the other replicas so their sockets update too.
async fn notify_devices_changed(state: &AppState, user_id: i32, origin_device: &str) {
    broadcast_devices(state, user_id, origin_device).await;
    let msg = NpBridgeMsg::DevicesChanged {
        user_id,
        origin_device: origin_device.to_string(),
        instance_id: state.instance_id.to_string(),
    };
    let _ = publish_bridge(state, &msg).await;
}

/// Fan a command out to the other replicas (used when the target device isn't on
/// this instance). Returns true if at least one *other* instance is subscribed
/// (so the caller can ack "relayed" rather than "device offline").
async fn publish_command(
    state: &AppState,
    user_id: i32,
    target_device_id: &str,
    from_device_id: &str,
    action: &str,
    args: &Value,
) -> bool {
    let msg = NpBridgeMsg::Command {
        user_id,
        target_device_id: target_device_id.to_string(),
        from_device_id: from_device_id.to_string(),
        action: action.to_string(),
        args: args.clone(),
        instance_id: state.instance_id.to_string(),
    };
    // Subscriber count includes our own bridge subscription, so >1 means another
    // replica is listening and could hold the target.
    publish_bridge(state, &msg).await > 1
}

/// Start the Valkey pub/sub subscriber that relays now-playing events from other
/// API replicas onto this instance's local sockets. Best-effort: on any failure
/// it logs and retries; the in-process fan-out keeps working meanwhile.
pub fn spawn_nowplaying_bridge(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = run_nowplaying_bridge(&state).await {
                tracing::warn!("now-playing pub/sub bridge stopped ({e}); retrying in 5s");
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

async fn run_nowplaying_bridge(state: &AppState) -> crate::error::AppResult<()> {
    let mut pubsub = state.redis_client.get_pubsub().await?;
    pubsub.subscribe(NOW_PLAYING_CHANNEL).await?;
    tracing::info!("now-playing pub/sub bridge subscribed to {NOW_PLAYING_CHANNEL}");

    let mut messages = pubsub.on_message();
    while let Some(msg) = messages.next().await {
        let payload: String = match msg.get_payload() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let bridge: NpBridgeMsg = match serde_json::from_str(&payload) {
            Ok(b) => b,
            Err(_) => continue,
        };
        // Ignore what we published ourselves — already handled locally.
        if bridge.instance_id() == state.instance_id.as_ref() {
            continue;
        }
        match bridge {
            NpBridgeMsg::DevicesChanged {
                user_id,
                origin_device,
                ..
            } => {
                broadcast_devices(state, user_id, &origin_device).await;
            }
            NpBridgeMsg::Command {
                user_id,
                target_device_id,
                from_device_id,
                action,
                args,
                ..
            } => {
                let relay = to_ws(&ServerMsg::Command {
                    from_device_id,
                    action,
                    args,
                });
                let _ = state
                    .now_playing_manager
                    .send_to_device(user_id, &target_device_id, relay)
                    .await;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn txt(s: &str) -> Message {
        Message::Text(s.to_string().into())
    }

    #[tokio::test]
    async fn registry_targets_and_excludes_devices() {
        let mgr = NowPlayingManager::new();
        let (tx_a, mut rx_a) = mpsc::unbounded_channel::<Message>();
        let (tx_b, mut rx_b) = mpsc::unbounded_channel::<Message>();
        mgr.add(1, "A".to_string(), tx_a).await;
        let b_conn = mgr.add(1, "B".to_string(), tx_b).await;

        // Command aimed at B reaches only B.
        assert!(mgr.send_to_device(1, "B", txt("cmd")).await);
        assert!(rx_b.try_recv().is_ok());
        assert!(rx_a.try_recv().is_err());

        // Unknown target is reported as undelivered.
        assert!(!mgr.send_to_device(1, "ghost", txt("cmd")).await);

        // Broadcast excludes the origin device.
        mgr.broadcast_user_except(1, "A", txt("devices")).await;
        assert!(rx_b.try_recv().is_ok());
        assert!(rx_a.try_recv().is_err());

        // A different user's device never receives another user's traffic.
        let (tx_c, mut rx_c) = mpsc::unbounded_channel::<Message>();
        mgr.add(2, "C".to_string(), tx_c).await;
        mgr.broadcast_user_except(1, "A", txt("devices")).await;
        assert!(rx_c.try_recv().is_err());

        // After removal the device is no longer addressable.
        assert!(mgr.remove(1, "B", b_conn).await);
        assert!(!mgr.send_to_device(1, "B", txt("cmd")).await);
    }

    #[test]
    fn capabilities_are_trimmed_and_bounded() {
        assert_eq!(
            parse_capabilities(" play, pause,,seek "),
            vec!["play", "pause", "seek"]
        );
        assert!(parse_capabilities("").is_empty());
        let long = "x".repeat(MAX_CAPABILITY_LEN + 1);
        assert_eq!(parse_capabilities(&format!("play,{long}")), vec!["play"]);
        let many = vec!["a"; MAX_CAPABILITIES + 5].join(",");
        assert_eq!(parse_capabilities(&many).len(), MAX_CAPABILITIES);
    }

    #[tokio::test]
    async fn reconnect_replaces_and_stale_teardown_is_a_noop() {
        let mgr = NowPlayingManager::new();
        let (tx_old, mut rx_old) = mpsc::unbounded_channel::<Message>();
        let (tx_new, mut rx_new) = mpsc::unbounded_channel::<Message>();
        let old_conn = mgr.add(1, "D".to_string(), tx_old).await;
        let new_conn = mgr.add(1, "D".to_string(), tx_new).await;
        assert_ne!(old_conn, new_conn);

        // The replaced socket is told to close with the "replaced" code.
        match rx_old.try_recv() {
            Ok(Message::Close(Some(frame))) => assert_eq!(frame.code, CLOSE_REPLACED),
            other => panic!("expected replaced close frame, got {other:?}"),
        }

        // The old socket's teardown must not evict the new connection.
        assert!(!mgr.remove(1, "D", old_conn).await);
        assert!(mgr.send_to_device(1, "D", txt("cmd")).await);
        assert!(rx_new.try_recv().is_ok());

        // The owning connection's teardown does remove it.
        assert!(mgr.remove(1, "D", new_conn).await);
        assert!(!mgr.send_to_device(1, "D", txt("cmd")).await);
    }
}
