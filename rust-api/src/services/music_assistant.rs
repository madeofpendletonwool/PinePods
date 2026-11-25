//! Music Assistant as a PinePods audio output.
//!
//! A per-user, opt-in bridge dials Music Assistant's WebSocket
//! (`ws://host:8095/ws`, first command `auth`), registers each available player
//! or group as a now-playing virtual device, and turns relay commands into MA
//! `players/cmd/*` and `player_queues/*` calls. A downloaded file is served from
//! the PinePods stream URL (podcast-scoped RSS key). An episode that was never
//! downloaded is the remote enclosure, not that stream URL, which 404s until a
//! file exists. Queue events write listen progress back.
//!
//! Nothing here runs unless the user saved an enabled configuration. The
//! WebSocket client is the only extra crate, and it is unused until then.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::ws::Message as RelayMessage;
use futures::{Sink, SinkExt, Stream, StreamExt};
use serde_json::{json, Value};
use sqlx::Row;
use tokio::sync::{mpsc, oneshot, watch, Notify, RwLock};
use tokio_tungstenite::tungstenite::Message;

use crate::database::dialect::dialect_match;
use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::handlers::nowplaying::notify_devices_changed;
use crate::models::{NowPlayingSnapshot, SleepTimer};
use crate::AppState;

/// Actions a Music Assistant virtual device announces on the now-playing relay.
pub const MA_CAPABILITIES: &[&str] = &[
    "play",
    "pause",
    "stop",
    "seek",
    "skip_forward",
    "skip_back",
    "play_episode",
    "next",
    "sleep_timer",
    "volume_set",
    "volume_mute",
    "group",
    "group_many",
    "ungroup",
    "set_members",
    "group_volume",
];

const DEVICE_TYPE: &str = "music_assistant";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const PLAYER_REFRESH: Duration = Duration::from_secs(20);
const PROGRESS_WRITE_EVERY: Duration = Duration::from_secs(10);

// ============================ URLs and versions ============================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaEndpoints {
    /// `http://host:port` (or https), stored and shown in settings.
    pub origin: String,
    pub ws: String,
    pub api: String,
}

/// Normalize a Music Assistant base URL to its WebSocket and JSON-RPC endpoints.
///
/// Accepts `http`, `https`, `ws`, and `wss`. Loopback is allowed (PinePods and
/// Music Assistant on the same machine). Link-local and cloud-metadata addresses
/// are rejected. A missing port becomes 8095 for plain http, 443 for https.
pub fn parse_server_url(raw: &str) -> Result<MaEndpoints, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("Music Assistant URL is empty".into());
    }
    let (secure, rest) = if let Some(rest) = trimmed.strip_prefix("wss://") {
        (true, format!("https://{rest}"))
    } else if let Some(rest) = trimmed.strip_prefix("ws://") {
        (false, format!("http://{rest}"))
    } else if trimmed.starts_with("https://") {
        (true, trimmed.to_string())
    } else if trimmed.starts_with("http://") {
        (false, trimmed.to_string())
    } else {
        return Err("Music Assistant URL must be http, https, ws, or wss".into());
    };
    let url = url::Url::parse(&rest).map_err(|e| format!("invalid Music Assistant URL: {e}"))?;
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Music Assistant URL must not include credentials".into());
    }
    let path = url.path().trim_end_matches('/');
    if !path.is_empty() && path != "/ws" && path != "/api" {
        return Err(
            "Music Assistant URL should be the server origin, for example http://192.168.1.20:8095"
                .into(),
        );
    }
    let host = url
        .host_str()
        .ok_or_else(|| "Music Assistant URL has no host".to_string())?;
    if is_link_local_or_metadata(host) {
        return Err(
            "Music Assistant URL must not point at a link-local or metadata address".into(),
        );
    }
    let port = url.port().unwrap_or(if secure { 443 } else { 8095 });
    let hostport = format!("{host}:{port}");
    let http_scheme = if secure { "https" } else { "http" };
    let ws_scheme = if secure { "wss" } else { "ws" };
    Ok(MaEndpoints {
        origin: format!("{http_scheme}://{hostport}"),
        ws: format!("{ws_scheme}://{hostport}/ws"),
        api: format!("{http_scheme}://{hostport}/api"),
    })
}

/// Origin Music Assistant will use to fetch `/api/data/stream/...`.
///
/// Must be http(s) with no path. Loopback is rejected: MA is another machine
/// (or another container) and cannot use PinePods' localhost. Private LAN
/// addresses are allowed.
pub fn normalize_external_base(raw: &str) -> Result<String, String> {
    let url = url::Url::parse(raw.trim()).map_err(|e| format!("invalid external base URL: {e}"))?;
    if url.scheme() != "http" && url.scheme() != "https" {
        return Err("external base URL must be http or https".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("external base URL must not include credentials".into());
    }
    if url.path() != "/" && !url.path().is_empty() {
        return Err(
            "external base URL must be an origin without a path (for example http://192.168.1.20:8040)"
                .into(),
        );
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("external base URL must not include a query or fragment".into());
    }
    let host = url
        .host_str()
        .ok_or_else(|| "external base URL has no host".to_string())?;
    if is_unreachable_pinepods_host(host) {
        return Err(
            "external base URL must be a LAN or public host Music Assistant can reach, not localhost"
                .into(),
        );
    }
    let origin = match url.port() {
        Some(p) => format!("{}://{host}:{p}", url.scheme()),
        None => format!("{}://{host}", url.scheme()),
    };
    Ok(origin)
}

fn host_bare(host: &str) -> &str {
    host.trim_matches(|c| c == '[' || c == ']')
}

fn is_link_local_or_metadata(host: &str) -> bool {
    let bare = host_bare(host);
    let lower = bare.to_ascii_lowercase();
    if lower == "metadata.google.internal" || lower.ends_with(".metadata.google.internal") {
        return true;
    }
    match bare.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_link_local(),
        Ok(IpAddr::V6(v6)) => (v6.segments()[0] & 0xffc0) == 0xfe80,
        Err(_) => false,
    }
}

fn is_unreachable_pinepods_host(host: &str) -> bool {
    let bare = host_bare(host);
    let lower = bare.to_ascii_lowercase();
    if lower == "localhost" || lower == "localhost.localdomain" || lower.ends_with(".localhost") {
        return true;
    }
    if is_link_local_or_metadata(host) {
        return true;
    }
    match bare.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => v4.is_loopback() || v4.is_unspecified(),
        Ok(IpAddr::V6(v6)) => v6.is_loopback() || v6.is_unspecified(),
        Err(_) => false,
    }
}

/// `2.10.1`, `v2.10.0-beta` → `(2, 10, 1)`.
pub fn parse_version(raw: &str) -> Option<(u64, u64, u64)> {
    let s = raw.trim().trim_start_matches(['v', 'V']);
    let s = s.split(['-', '+', ' ']).next().unwrap_or(s);
    let mut parts = s.split('.');
    let major: u64 = parts.next()?.parse().ok()?;
    let minor: u64 = parts.next().unwrap_or("0").parse().ok()?;
    let patch_src = parts.next().unwrap_or("0");
    let patch_digits: String = patch_src
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let patch = if patch_digits.is_empty() {
        0
    } else {
        patch_digits.parse().ok()?
    };
    Some((major, minor, patch))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionAssessment {
    pub compatible: bool,
    /// Set when we connected but the server is outside the tested 2.10 line.
    pub warning: Option<String>,
}

/// 2.10.x is the tested line. Older than 2.6 is refused. Newer majors and
/// missing version fields warn and continue, so a schema bump does not brick playback.
pub fn assess_version(server_version: &str, schema_version: Option<i64>) -> VersionAssessment {
    let version = server_version.trim();
    if version.is_empty() {
        return VersionAssessment {
            compatible: true,
            warning: Some("Music Assistant did not report a version; continuing".into()),
        };
    }
    let Some((major, minor, _)) = parse_version(version) else {
        return VersionAssessment {
            compatible: true,
            warning: Some(format!(
                "could not parse Music Assistant version '{version}'; continuing"
            )),
        };
    };
    if major < 2 || (major == 2 && minor < 6) {
        return VersionAssessment {
            compatible: false,
            warning: Some(format!(
                "Music Assistant {version} is too old; 2.10.x is required"
            )),
        };
    }
    let mut warning = None;
    if major >= 3 {
        warning = Some(format!(
            "Music Assistant {version} is newer than the tested 2.10.x line; commands may differ"
        ));
    } else if minor < 10 {
        warning = Some(format!(
            "Music Assistant {version} is older than the tested 2.10.x line; playback should work, newer commands might not"
        ));
    }
    if let Some(schema) = schema_version {
        if schema < 20 {
            let extra = format!("schema_version {schema} is older than expected for 2.10");
            warning = Some(match warning {
                Some(w) => format!("{w}; {extra}"),
                None => extra,
            });
        }
    }
    VersionAssessment {
        compatible: true,
        warning,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub server_version: String,
    pub schema_version: Option<i64>,
    pub server_id: Option<String>,
    pub assessment: VersionAssessment,
}

pub fn server_info_from_result(v: &Value) -> ServerInfo {
    let server_version = v
        .get("server_version")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
    let schema_version = v.get("schema_version").and_then(json_i64);
    let server_id = v
        .get("server_id")
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    let assessment = assess_version(&server_version, schema_version);
    ServerInfo {
        server_version,
        schema_version,
        server_id,
        assessment,
    }
}

/// Where Music Assistant should load an episode from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaPlayback {
    /// PinePods can serve a file (a download, or `local://` media).
    Stream,
    /// Remote http(s) enclosure. Never a PinePods stream URL.
    Direct(String),
}

fn is_http_url(uri: &str) -> bool {
    let lower = uri.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// RFC 3986 scheme at the start of `raw` (`alpha *(alpha / digit / "+" / "-" / ".") ":"`).
fn has_uri_scheme(raw: &str) -> bool {
    let mut chars = raw.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return false;
    }
    let Some(colon) = raw.find(':') else {
        return false;
    };
    raw[..colon]
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}

/// `served_by_pinepods` is a file the stream endpoint can actually return.
/// Anything else must be a playable http(s) enclosure. The stream URL is not a
/// fallback: it 404s when the episode was never saved.
pub fn ma_playback(
    served_by_pinepods: bool,
    direct_uri: Option<String>,
) -> Result<MaPlayback, &'static str> {
    if served_by_pinepods {
        return Ok(MaPlayback::Stream);
    }
    match direct_uri.filter(|uri| is_http_url(uri)) {
        Some(uri) => Ok(MaPlayback::Direct(uri)),
        None => {
            Err("This episode has no URL Music Assistant can play. Download it in PinePods first.")
        }
    }
}

pub fn build_stream_url(
    external_base: &str,
    episode_id: i32,
    rss_key: &str,
    user_id: i32,
    is_youtube: bool,
) -> String {
    let base = external_base.trim_end_matches('/');
    let key = urlencoding::encode(rss_key);
    let mut url = format!("{base}/api/data/stream/{episode_id}?api_key={key}&user_id={user_id}");
    if is_youtube {
        url.push_str("&type=youtube");
    }
    url
}

/// `ma:{user_id}:{player_id}`. Player ids may themselves contain colons.
pub fn device_id_for(user_id: i32, player_id: &str) -> String {
    format!("ma:{user_id}:{player_id}")
}

pub fn player_id_from_device(user_id: i32, device_id: &str) -> Option<String> {
    let prefix = format!("ma:{user_id}:");
    let rest = device_id.strip_prefix(&prefix)?;
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

pub fn canonical_player_id(user_id: i32, raw: &str) -> String {
    player_id_from_device(user_id, raw).unwrap_or_else(|| raw.to_string())
}

// ============================ Players and progress ============================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaPlayerKind {
    Player,
    Group,
}

impl MaPlayerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Player => "player",
            Self::Group => "group",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaPlayer {
    pub player_id: String,
    pub name: String,
    pub available: bool,
    pub state: String,
    pub volume_level: Option<f64>,
    pub volume_muted: bool,
    pub kind: MaPlayerKind,
    pub group_members: Vec<String>,
    pub synced_to: Option<String>,
    pub can_group_with: Vec<String>,
    pub active_source: Option<String>,
    pub elapsed_time: Option<f64>,
    pub current_uri: Option<String>,
    pub current_title: Option<String>,
}

pub fn players_from_result(v: &Value) -> Vec<MaPlayer> {
    let list = if let Some(arr) = v.as_array() {
        arr
    } else if let Some(arr) = v.get("players").and_then(|p| p.as_array()) {
        arr
    } else {
        return Vec::new();
    };
    list.iter().filter_map(player_from_value).collect()
}

pub fn player_from_value(v: &Value) -> Option<MaPlayer> {
    let player_id = v.get("player_id").and_then(|s| s.as_str())?.to_string();
    if player_id.is_empty() {
        return None;
    }
    let name = v
        .get("name")
        .or_else(|| v.get("display_name"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&player_id)
        .to_string();
    let group_members = group_member_ids(v);
    let type_name = v.get("type").and_then(|s| s.as_str()).unwrap_or("");
    let kind = if type_name.to_ascii_lowercase().contains("group") {
        MaPlayerKind::Group
    } else {
        MaPlayerKind::Player
    };
    let state = playback_state_str(v).unwrap_or_else(|| "idle".to_string());
    let synced_to = v
        .get("synced_to")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let active_source = v
        .get("active_source")
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Some(MaPlayer {
        player_id,
        name,
        available: v.get("available").and_then(|b| b.as_bool()).unwrap_or(true),
        state,
        volume_level: v.get("volume_level").and_then(json_f64),
        volume_muted: v
            .get("volume_muted")
            .and_then(|b| b.as_bool())
            .unwrap_or(false),
        kind,
        group_members,
        synced_to,
        can_group_with: id_list(v.get("can_group_with").unwrap_or(&Value::Null)),
        active_source,
        elapsed_time: v.get("elapsed_time").and_then(json_f64),
        current_uri: first_uri(v),
        current_title: first_title(v),
    })
}

/// `playback_state` is the 2.10 field. `state` is the alias older payloads and
/// the queue events still use. Empty and JSON null count as missing.
pub fn playback_state_str(v: &Value) -> Option<String> {
    for key in ["playback_state", "state"] {
        if let Some(raw) = v.get(key).and_then(|s| s.as_str()) {
            let trimmed = raw.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// `group_childs` is the pre-2.6 name. 2.10 sends `group_members` and, when the
/// serializer adds the alias, both. Prefer whichever list is non-empty.
pub fn group_member_ids(v: &Value) -> Vec<String> {
    let childs = id_list(v.get("group_childs").unwrap_or(&Value::Null));
    if !childs.is_empty() {
        return childs;
    }
    id_list(v.get("group_members").unwrap_or(&Value::Null))
}

/// Fold one player event onto the player we already stored.
///
/// A `player_updated` frame that omits playback state, availability, or the
/// active queue must not replace a playing Sonos speaker with a default idle
/// player. Music Assistant sends `null` for `state` while a sync member's
/// playback lives on the coordinator; that is not "nothing is loaded".
pub fn merge_player_update(previous: Option<&MaPlayer>, raw: &Value) -> Option<MaPlayer> {
    let mut player = player_from_value(raw)?;
    let Some(prev) = previous else {
        return Some(player);
    };
    let state_field = raw.get("playback_state").or_else(|| raw.get("state"));
    let keep_state = match state_field {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) if s.trim().is_empty() => true,
        _ => false,
    };
    if keep_state {
        player.state = prev.state.clone();
    }
    if raw.get("available").is_none() {
        player.available = prev.available;
    }
    if raw.get("active_source").is_none() {
        player.active_source = prev.active_source.clone();
    }
    if raw.get("synced_to").is_none() {
        player.synced_to = prev.synced_to.clone();
    }
    if raw.get("group_childs").is_none() && raw.get("group_members").is_none() {
        player.group_members = prev.group_members.clone();
        player.kind = prev.kind;
    }
    if raw.get("volume_level").is_none() {
        player.volume_level = prev.volume_level;
    }
    if raw.get("volume_muted").is_none() {
        player.volume_muted = prev.volume_muted;
    }
    if raw.get("name").is_none() && raw.get("display_name").is_none() {
        player.name = prev.name.clone();
    }
    if raw.get("current_media").is_none() && raw.get("current_item").is_none() {
        player.current_uri = prev.current_uri.clone();
        player.current_title = prev.current_title.clone();
        if player.elapsed_time.is_none() {
            player.elapsed_time = prev.elapsed_time;
        }
    }
    if raw.get("type").is_none() {
        player.kind = prev.kind;
    }
    Some(player)
}

fn id_list(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    item.as_str().map(|s| s.to_string()).or_else(|| {
                        item.get("player_id")
                            .and_then(|p| p.as_str())
                            .map(|s| s.to_string())
                    })
                })
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn first_uri(v: &Value) -> Option<String> {
    for key in ["current_media", "current_item"] {
        if let Some(node) = v.get(key) {
            if let Some(uri) = node.get("uri").and_then(|s| s.as_str()) {
                return Some(uri.to_string());
            }
            if let Some(uri) = node
                .get("media_item")
                .and_then(|m| m.get("uri"))
                .and_then(|s| s.as_str())
            {
                return Some(uri.to_string());
            }
        }
    }
    None
}

fn first_title(v: &Value) -> Option<String> {
    for key in ["current_media", "current_item"] {
        if let Some(node) = v.get(key) {
            if let Some(name) = node
                .get("name")
                .or_else(|| node.get("title"))
                .and_then(|s| s.as_str())
            {
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamRef {
    pub episode_id: i32,
    pub user_id: Option<i32>,
    pub is_youtube: bool,
}

/// Pull a PinePods stream URL (`/api/data/stream/{id}?api_key=…&user_id=…`) out of any string.
pub fn parse_stream_url(raw: &str) -> Option<StreamRef> {
    let marker = "/api/data/stream/";
    let idx = raw.find(marker)?;
    let rest = &raw[idx + marker.len()..];
    let id_str: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if id_str.is_empty() {
        return None;
    }
    let episode_id: i32 = id_str.parse().ok()?;
    let query = raw.split_once('?').map(|(_, q)| q).unwrap_or("");
    let mut user_id = None;
    let mut is_youtube = false;
    for pair in query.split(['&', '#']) {
        let mut it = pair.splitn(2, '=');
        let key = it.next().unwrap_or("").trim_start_matches("amp;");
        let val = it.next().unwrap_or("");
        if key == "user_id" {
            user_id = val.parse().ok();
        } else if key == "type" && val == "youtube" {
            is_youtube = true;
        }
    }
    Some(StreamRef {
        episode_id,
        user_id,
        is_youtube,
    })
}

pub fn find_stream_ref(v: &Value) -> Option<StreamRef> {
    match v {
        Value::String(s) => parse_stream_url(s),
        Value::Array(items) => items.iter().find_map(find_stream_ref),
        Value::Object(map) => map.values().find_map(find_stream_ref),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressUpdate {
    pub queue_id: Option<String>,
    pub stream: Option<StreamRef>,
    pub position_sec: Option<f64>,
    pub duration_sec: Option<f64>,
    pub playing: Option<bool>,
    pub title: Option<String>,
    pub speed: Option<f64>,
    /// True when the payload names `current_item` or `current_media` and sets it
    /// to null. An omitted item is not a clear: queue time ticks only carry
    /// `elapsed_time`.
    pub media_cleared: bool,
    /// True when the payload's playback state is present and is neither playing,
    /// buffering, nor paused. Sonos reports that for a Music Assistant pause and
    /// for a sync group whose leader is still playing.
    pub reported_idle: bool,
    /// Unix seconds the position was measured. A full queue object uses
    /// `elapsed_time_last_updated`. A numeric tick leaves this empty so
    /// [`progress_from_event`] can stamp the time the bridge received it.
    pub position_updated_at: Option<i64>,
    /// Every string inside `current_item` / `current_media`, when the payload
    /// has one. Music Assistant echoes the media URL PinePods sent there.
    pub item_strings: Option<Vec<String>>,
    /// `resume_pos` from a full queue object. See [`paused_position`].
    pub resume_pos: Option<f64>,
}

fn collect_strings(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.clone()),
        Value::Array(items) => items.iter().for_each(|i| collect_strings(i, out)),
        Value::Object(map) => map.values().for_each(|i| collect_strings(i, out)),
        _ => {}
    }
}

/// Read whatever a queue or player event tells us about PinePods playback.
///
/// Music Assistant 2.10.7 sends `queue_time_updated` with `data` as a bare
/// number of media-time seconds. Older 2.10 builds send an object with
/// `elapsed_time`. Both are accepted. A number has no timestamp of its own.
pub fn progress_from_payload(data: &Value) -> ProgressUpdate {
    if data.is_number() {
        return ProgressUpdate {
            queue_id: None,
            stream: None,
            position_sec: json_f64(data).map(|n| n.max(0.0)),
            duration_sec: None,
            playing: None,
            title: None,
            speed: None,
            media_cleared: false,
            reported_idle: false,
            position_updated_at: None,
            item_strings: None,
            resume_pos: None,
        };
    }
    let queue_id = data
        .get("queue_id")
        .or_else(|| data.get("player_id"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string());
    let position_sec = data.get("elapsed_time").and_then(json_f64);
    let state = playback_state_str(data);
    let reported_idle = state.as_deref().is_some_and(|s| {
        let lower = s.to_ascii_lowercase();
        lower != "playing" && lower != "buffering" && lower != "paused"
    });
    let playing = state.as_deref().map(|s| {
        let lower = s.to_ascii_lowercase();
        lower == "playing" || lower == "buffering"
    });
    let media_cleared = matches!(data.get("current_item"), Some(Value::Null))
        || matches!(data.get("current_media"), Some(Value::Null));
    let item = data
        .get("current_item")
        .or_else(|| data.get("current_media"));
    let duration_sec = item
        .and_then(|i| i.get("duration"))
        .and_then(json_f64)
        .or_else(|| {
            item.and_then(|i| i.get("media_item"))
                .and_then(|m| m.get("duration"))
                .and_then(json_f64)
        });
    let title = item.and_then(|i| {
        i.get("name")
            .or_else(|| i.get("title"))
            .or_else(|| i.get("media_item").and_then(|m| m.get("name")))
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    });
    let speed = data.get("playback_speed").and_then(json_f64).or_else(|| {
        item.and_then(|i| i.get("playback_speed"))
            .and_then(json_f64)
    });
    // Only a full queue object carries this. It is media-time's clock, so the
    // snapshot must not add the network delay a second time.
    let position_updated_at = position_sec.and_then(|_| {
        data.get("elapsed_time_last_updated")
            .and_then(json_f64)
            .filter(|t| t.is_finite() && *t > 0.0)
            .map(|t| t.trunc() as i64)
    });
    ProgressUpdate {
        queue_id,
        stream: find_stream_ref(data),
        position_sec,
        duration_sec,
        playing,
        title,
        speed,
        media_cleared,
        reported_idle,
        position_updated_at,
        item_strings: item.filter(|i| !i.is_null()).map(|i| {
            let mut out = Vec::new();
            collect_strings(i, &mut out);
            out
        }),
        resume_pos: data.get("resume_pos").and_then(json_f64),
    }
}

/// Queue event as the bridge should store it.
///
/// `object_id` is the queue id when the payload does not name one. Numeric
/// ticks are stamped with `received_at`. A full object keeps
/// `elapsed_time_last_updated` when it sent one, and otherwise uses
/// `received_at` as well.
pub fn progress_from_event(
    object_id: Option<&str>,
    data: &Value,
    received_at: i64,
) -> ProgressUpdate {
    let mut update = progress_from_payload(data);
    if update.queue_id.is_none() {
        update.queue_id = object_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| id.to_string());
    }
    if update.position_sec.is_some() && update.position_updated_at.is_none() && received_at > 0 {
        update.position_updated_at = Some(received_at);
    }
    update
}

/// Next playing flag and whether the loaded episode should be dropped.
///
/// Sonos pause is stop-plus-resume. Music Assistant reports that as idle while
/// the episode is still loaded. The bar stays paused and keeps the episode.
/// An explicit null `current_item` / `current_media` is a real stop.
/// The loaded-episode and idle flags stay in the signature so a sync member
/// and a Sonos pause share one call. Idle does not clear the episode; the
/// playing flag from the payload (false for idle) is what the bar shows.
pub fn next_playback(
    currently_playing: bool,
    _has_stream: bool,
    update_playing: Option<bool>,
    _reported_idle: bool,
    media_cleared: bool,
) -> (bool, bool) {
    if media_cleared {
        return (false, true);
    }
    let playing = match update_playing {
        Some(is_playing) => is_playing,
        None => currently_playing,
    };
    (playing, false)
}

/// Queue id whose snapshot should stand for `player`.
///
/// A queue that still has a PinePods stream beats an empty one. Sonos
/// coordinator changes and Music Assistant clearing `active_source` (it does
/// that on purpose while its own queue is the source) otherwise publish
/// `episode_id: 0` and the clients hide the controls. `retained` is the last
/// queue id that had a stream for this player.
pub fn snapshot_queue_id(
    player: &MaPlayer,
    players: &[MaPlayer],
    loaded_queues: &HashSet<String>,
    retained: Option<&str>,
) -> String {
    let mut candidates: Vec<String> = Vec::new();
    let mut push = |id: &str| {
        let id = id.trim();
        if id.is_empty() || candidates.iter().any(|existing| existing == id) {
            return;
        }
        candidates.push(id.to_string());
    };
    if let Some(src) = player.active_source.as_deref() {
        push(src);
    }
    push(&player.player_id);
    if let Some(leader) = player.synced_to.as_deref() {
        push(leader);
        if let Some(leader_player) = players.iter().find(|p| p.player_id == leader) {
            if let Some(src) = leader_player.active_source.as_deref() {
                push(src);
            }
        }
    }
    for member in &player.group_members {
        push(member);
        if let Some(member_player) = players.iter().find(|p| p.player_id == member.as_str()) {
            if let Some(src) = member_player.active_source.as_deref() {
                push(src);
            }
        }
    }
    for other in players {
        if other.player_id == player.player_id {
            continue;
        }
        let points_here = other.synced_to.as_deref() == Some(player.player_id.as_str())
            || other.group_members.iter().any(|m| m == &player.player_id);
        if points_here {
            push(&other.player_id);
            if let Some(src) = other.active_source.as_deref() {
                push(src);
            }
        }
    }
    if let Some(retained) = retained {
        push(retained);
    }
    if let Some(loaded) = candidates.iter().find(|id| loaded_queues.contains(*id)) {
        return loaded.clone();
    }
    candidates
        .into_iter()
        .next()
        .unwrap_or_else(|| player.player_id.clone())
}

/// Unavailable players stay registered while they still have a loaded episode,
/// so a Sonos availability blip does not delete the now-playing row. A player
/// that is gone (`player_removed`) is not in the list and is dropped.
pub fn keep_virtual_device(available: bool, has_loaded_episode: bool) -> bool {
    available || has_loaded_episode
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressSample {
    pub episode_id: i32,
    pub position_sec: f64,
    pub playing: bool,
}

/// Throttle history writes: every 10s, on pause, on episode change, or on a seek-sized jump.
///
/// Nothing is written while the episode's resume seek is pending: its first
/// samples are the start of the file and would overwrite the saved position
/// the seek is about to restore.
pub fn should_write_progress(
    prev: Option<&ProgressSample>,
    next: &ProgressSample,
    since_write: Duration,
    resume_pending: bool,
) -> bool {
    if resume_pending {
        return false;
    }
    let Some(prev) = prev else {
        return true;
    };
    if prev.episode_id != next.episode_id {
        return true;
    }
    if prev.playing && !next.playing {
        return true;
    }
    if (next.position_sec - prev.position_sec).abs() >= 8.0 {
        return true;
    }
    since_write >= PROGRESS_WRITE_EVERY
}

/// Queue that `play_media`, `seek`, and `skip` must target.
///
/// A Sonos sync member is not the queue owner. Music Assistant redirects
/// `players/cmd/play|pause|stop` onto `synced_to` itself, but
/// `player_queues/play_media` and `player_queues/seek` do not. Sending those
/// to the member hits "queue is not active", and `play_media` on a member
/// unsyncs the group. Resolution matches the leader half of
/// [`snapshot_queue_id`]: `synced_to`, then that leader's `active_source`,
/// then the leader's player id. A speaker that is not synced uses its own
/// `active_source` or its player id.
pub fn queue_id_for(players: &[MaPlayer], player_id: &str) -> String {
    let Some(player) = players.iter().find(|p| p.player_id == player_id) else {
        return player_id.to_string();
    };
    let leader_id = player
        .synced_to
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .unwrap_or(player.player_id.as_str());
    let Some(leader) = players.iter().find(|p| p.player_id == leader_id) else {
        return leader_id.to_string();
    };
    leader
        .active_source
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .unwrap_or(leader.player_id.as_str())
        .to_string()
}

/// Whole seconds for Music Assistant. 2.10.7 types `position` and `seconds` as
/// `int`. Truncate toward zero; a negative or non-finite value is 0.
pub fn integer_seconds(position: f64) -> i64 {
    if !position.is_finite() || position <= 0.0 {
        return 0;
    }
    let truncated = position.trunc();
    if truncated >= i64::MAX as f64 {
        return i64::MAX;
    }
    truncated as i64
}

fn integer_seconds_signed(seconds: f64) -> i64 {
    if !seconds.is_finite() {
        return 0;
    }
    let truncated = seconds.trunc();
    if truncated >= i64::MAX as f64 {
        return i64::MAX;
    }
    if truncated <= i64::MIN as f64 {
        return i64::MIN;
    }
    truncated as i64
}

// ============================ Commands ============================

#[derive(Debug, Clone, PartialEq)]
pub struct MaCommand {
    pub command: String,
    pub args: Value,
}

/// Map a relay/REST action onto one or more Music Assistant commands.
/// `play_episode` is expanded by [`prepare_play_episode`] because it needs the database.
pub fn translate_action(
    player_id: &str,
    queue_id: &str,
    action: &str,
    args: &Value,
) -> Result<Vec<MaCommand>, String> {
    // Episodes go to Music Assistant as tracks. It only tempo-shifts podcast
    // episodes and audiobooks, so a speed command errors. Ignore it: no MA
    // call, and no error for the client to show.
    if action == "set_speed" {
        return Ok(Vec::new());
    }
    let player_cmd = |name: &str, extra: Value| MaCommand {
        command: format!("players/cmd/{name}"),
        args: {
            let mut base = json!({ "player_id": player_id });
            if let (Some(obj), Some(extra_obj)) = (base.as_object_mut(), extra.as_object()) {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
            base
        },
    };
    let queue_cmd = |name: &str, extra: Value| MaCommand {
        command: format!("player_queues/{name}"),
        args: {
            let mut base = json!({ "queue_id": queue_id });
            if let (Some(obj), Some(extra_obj)) = (base.as_object_mut(), extra.as_object()) {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
            base
        },
    };

    let one = match action {
        "play" => player_cmd("play", json!({})),
        "pause" => player_cmd("pause", json!({})),
        "stop" => player_cmd("stop", json!({})),
        "seek" => {
            let position = args
                .get("position_sec")
                .or_else(|| args.get("position"))
                .and_then(json_f64)
                .ok_or_else(|| "seek requires position_sec".to_string())?;
            if position < 0.0 {
                return Err("seek position must be >= 0".into());
            }
            queue_cmd("seek", json!({ "position": integer_seconds(position) }))
        }
        "skip_forward" => {
            // Same 15s step as the local web player. Callers may still pass seconds.
            let seconds = args.get("seconds").and_then(json_f64).unwrap_or(15.0).abs();
            queue_cmd("skip", json!({ "seconds": integer_seconds(seconds) }))
        }
        "skip_back" => {
            let seconds = args.get("seconds").and_then(json_f64).unwrap_or(15.0).abs();
            queue_cmd(
                "skip",
                json!({ "seconds": integer_seconds_signed(-seconds) }),
            )
        }
        "volume_set" => {
            let level = args
                .get("volume_level")
                .or_else(|| args.get("volume"))
                .and_then(json_f64)
                .ok_or_else(|| "volume_set requires volume_level".to_string())?;
            let level = level.clamp(0.0, 100.0);
            player_cmd("volume_set", json!({ "volume_level": level }))
        }
        "volume_mute" => {
            let muted = args
                .get("muted")
                .or_else(|| args.get("mute"))
                .and_then(|v| v.as_bool())
                .ok_or_else(|| "volume_mute requires muted".to_string())?;
            player_cmd("volume_mute", json!({ "muted": muted }))
        }
        "group" => {
            let target = args
                .get("target_player")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "group requires target_player".to_string())?;
            player_cmd("group", json!({ "target_player": target }))
        }
        "group_many" => {
            let ids = string_list(args.get("player_ids").or_else(|| args.get("members")));
            if ids.is_empty() {
                return Err("group_many requires player_ids".into());
            }
            let target = args
                .get("target_player")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(player_id);
            player_cmd(
                "group_many",
                json!({ "target_player": target, "player_ids": ids }),
            )
        }
        "ungroup" => player_cmd("ungroup", json!({})),
        "set_members" => {
            let add = string_list(args.get("player_ids_to_add"));
            let remove = string_list(args.get("player_ids_to_remove"));
            if add.is_empty() && remove.is_empty() {
                return Err(
                    "set_members requires player_ids_to_add or player_ids_to_remove".into(),
                );
            }
            let target = args
                .get("target_player")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .unwrap_or(player_id);
            player_cmd(
                "set_members",
                json!({
                    "target_player": target,
                    "player_ids_to_add": add,
                    "player_ids_to_remove": remove,
                }),
            )
        }
        "group_volume" => {
            let level = args
                .get("volume_level")
                .or_else(|| args.get("volume"))
                .and_then(json_f64)
                .ok_or_else(|| "group_volume requires volume_level".to_string())?;
            player_cmd(
                "group_volume",
                json!({ "volume_level": level.clamp(0.0, 100.0) }),
            )
        }
        other => return Err(format!("unsupported action '{other}'")),
    };
    Ok(vec![one])
}

/// Title, podcast, length, and artwork for a `play_media` Track.
///
/// Music Assistant 2.10.4 (`music-assistant-models` 1.1.205) parses a dict with
/// `media_from_dict`. A dict that includes `provider_mappings` and
/// `media_type: "track"` becomes a Track; without `provider_mappings` it becomes
/// an ItemMapping and the image is dropped. A bare URL is resolved as a builtin
/// Track with no image, so that form is only used when there is no artwork.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaTrackDisplay {
    pub title: String,
    pub podcast_name: String,
    pub duration_sec: i64,
    pub artwork_url: Option<String>,
}

/// Absolute http(s) artwork Music Assistant can fetch.
///
/// A relative path or a PinePods proxy path (`/api/proxy/image?...`) is joined
/// onto `external_base`. Anything that is not http(s) and cannot be joined is
/// dropped.
pub fn artwork_url_for_ma(raw: &str, external_base: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if is_http_url(raw) {
        return Some(raw.to_string());
    }
    // `javascript:` and `data:` are schemes without `://`. A proxy path may
    // still carry `https://` inside its query and is joined below.
    if has_uri_scheme(raw) {
        return None;
    }
    let base = external_base.trim().trim_end_matches('/');
    if !is_http_url(base) {
        return None;
    }
    let path = raw.trim_start_matches('/');
    if path.is_empty() {
        return None;
    }
    Some(format!("{base}/{path}"))
}

fn display_from_meta(meta: &Value, external_base: &str) -> MaTrackDisplay {
    let title = meta
        .get("episodetitle")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let podcast_name = meta
        .get("podcastname")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    let duration_sec = meta
        .get("episodeduration")
        .and_then(json_i64)
        .unwrap_or(0)
        .max(0);
    let episode_art = meta
        .get("episodeartwork")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let podcast_art = meta
        .get("artworkurl")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let artwork_url = artwork_url_for_ma(episode_art, external_base)
        .or_else(|| artwork_url_for_ma(podcast_art, external_base));
    MaTrackDisplay {
        title,
        podcast_name,
        duration_sec,
        artwork_url,
    }
}

/// Track dict accepted by `Track.from_dict` on models 1.1.205. `None` when there
/// is no fetchable artwork: the caller then sends the URL string.
fn track_media(media_url: &str, display: &MaTrackDisplay) -> Option<Value> {
    let artwork = display
        .artwork_url
        .as_deref()
        .map(str::trim)
        .filter(|url| is_http_url(url))?;
    let title = display.title.trim();
    let name = if title.is_empty() { "Episode" } else { title };
    let mut item = json!({
        "media_type": "track",
        "item_id": media_url,
        "provider": "builtin",
        "name": name,
        "duration": display.duration_sec.max(0),
        "provider_mappings": [{
            "item_id": media_url,
            "provider_domain": "builtin",
            "provider_instance": "builtin",
            "available": true,
        }],
        "metadata": {
            "images": [{
                "type": "thumb",
                "path": artwork,
                "provider": "builtin",
                "remotely_accessible": true,
            }]
        }
    });
    let podcast = display.podcast_name.trim();
    if !podcast.is_empty() {
        item["artists"] = json!([{
            "media_type": "artist",
            "item_id": podcast,
            "provider": "builtin",
            "name": podcast,
            "available": true,
        }]);
    }
    Some(item)
}

/// `play_media` only. Local playback speed is not included.
///
/// Episodes are sent as `media_type: track`. Music Assistant only tempo-shifts
/// podcast episodes and audiobooks, so a speed other than 1x must not be
/// carried onto the speaker. The resume offset is not in this list either:
/// an episode with a resume point is loaded by [`staged_start_commands`].
/// Music Assistant seeks by rebuilding the stream, and a seek sent before the
/// queue has a duration fails or plays the start and then jumps.
/// [`resume_seek_command`] is the fallback seek, sent after a queue event
/// reports a duration, when Music Assistant refuses the staged start.
pub fn play_media_commands(
    queue_id: &str,
    media_url: &str,
    display: Option<&MaTrackDisplay>,
) -> Vec<MaCommand> {
    let media = display
        .and_then(|display| track_media(media_url, display))
        .unwrap_or_else(|| json!(media_url));
    vec![MaCommand {
        command: "player_queues/play_media".into(),
        args: json!({
            "queue_id": queue_id,
            "media": media,
            "option": "replace",
        }),
    }]
}

/// How an episode with a resume point is started: staged on an emptied queue, then started at
/// the offset, so the speaker starts once.
///
/// `play_media` with `replace` starts at 0 and has no offset of its own. Following it with a
/// seek starts the stream twice within a moment, and Music Assistant 2.10.13 then leaves some
/// players stalled at the seek target while it reports `playing` (about one start in eight on a
/// squeezelite player). An `add` only stages the item on an idle queue, and
/// `play_index` takes the offset.
#[derive(Debug, Clone, PartialEq)]
pub struct StagedStart {
    /// `clear`, then `play_media` with `add`. Each has to succeed.
    pub stage: Vec<MaCommand>,
    /// `play_index` at the offset.
    pub start: MaCommand,
    /// `play_index` from the beginning, for a server that refuses the offset. The seek that
    /// waits for the queue's duration takes over from there.
    pub from_start: MaCommand,
}

/// `None` when the offset is under a second: [`play_media_commands`] plays from the start.
pub fn staged_start_commands(
    queue_id: &str,
    media_url: &str,
    display: Option<&MaTrackDisplay>,
    position_sec: f64,
) -> Option<StagedStart> {
    let position = integer_seconds(position_sec);
    if position_sec < 1.0 || position < 1 {
        return None;
    }
    let media = display
        .and_then(|display| track_media(media_url, display))
        .unwrap_or_else(|| json!(media_url));
    let play_index = |args: Value| MaCommand {
        command: "player_queues/play_index".into(),
        args,
    };
    Some(StagedStart {
        stage: vec![
            MaCommand {
                command: "player_queues/clear".into(),
                args: json!({ "queue_id": queue_id }),
            },
            MaCommand {
                command: "player_queues/play_media".into(),
                args: json!({ "queue_id": queue_id, "media": media, "option": "add" }),
            },
        ],
        start: play_index(json!({ "queue_id": queue_id, "index": 0, "seek_position": position })),
        from_start: play_index(json!({ "queue_id": queue_id, "index": 0 })),
    })
}

/// One integer `player_queues/seek` once the queue has reported a duration.
/// `None` when the offset is under a second or the duration is still unknown,
/// so the bridge does not send a seek Music Assistant will reject.
pub fn resume_seek_command(
    queue_id: &str,
    position_sec: f64,
    duration_sec: f64,
) -> Option<MaCommand> {
    if duration_sec <= 0.0 || position_sec < 1.0 {
        return None;
    }
    let position = integer_seconds(position_sec);
    if position < 1 {
        return None;
    }
    Some(MaCommand {
        command: "player_queues/seek".into(),
        args: json!({ "queue_id": queue_id, "position": position }),
    })
}

/// Inline wait in the command response, then the same wait continues in the background.
pub const RESUME_SEEK_INLINE: Duration = Duration::from_secs(1);
/// How long the background wait for the queue's duration lasts before the seek is abandoned.
pub const RESUME_SEEK_PATIENCE: Duration = Duration::from_secs(30);
const RESUME_SEEK_POLL: Duration = Duration::from_millis(350);
/// How long after `play_media` a queue may still report that it holds nothing.
pub const LOAD_GRACE: Duration = Duration::from_secs(30);

/// Whether a null `current_item` is the empty queue from before the episode loaded.
///
/// A queue that held nothing (a new speaker, a cleared queue) answers `play_media` with
/// `queue_items_updated` and `queue_updated` events that still say `current_item: null`, and
/// only then makes the new item current. That is not a stop. Taking it for one forgot the
/// episode and its resume seek: the speaker started at 0, and a remote enclosure was never
/// matched to its episode again, so nothing was shown or saved. `loading_for` is the time since
/// `play_episode` sent the media, while no queue event has shown that media yet.
pub fn stale_empty_queue(media_cleared: bool, loading_for: Option<Duration>) -> bool {
    media_cleared && loading_for.is_some_and(|waited| waited < LOAD_GRACE)
}

/// Whether a queue event's item is the media PinePods sent.
pub fn item_is_sent_media(item_strings: Option<&[String]>, media_url: &str) -> bool {
    !media_url.is_empty()
        && item_strings.is_some_and(|strings| strings.iter().any(|s| s.contains(media_url)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeekPlan {
    /// Offset is under a second. Do not seek and do not use history.
    Skip,
    Send(i64),
    Wait,
    GiveUp(&'static str),
}

/// Decide the next step of the single post-load seek.
///
/// `waited` is how long the seek has waited for the queue to report a
/// duration. Once it reaches `patience` the seek gives up.
pub fn plan_resume_seek(
    position_sec: f64,
    duration_sec: f64,
    waited: Duration,
    patience: Duration,
) -> SeekPlan {
    if position_sec < 1.0 {
        return SeekPlan::Skip;
    }
    if let Some(cmd) = resume_seek_command("queue", position_sec, duration_sec) {
        return SeekPlan::Send(cmd.args["position"].as_i64().unwrap_or(0));
    }
    if waited >= patience {
        return SeekPlan::GiveUp("queue without duration");
    }
    SeekPlan::Wait
}

/// Music Assistant answers "Queue is not active" or "without duration" when the
/// seek races the queue. Those are worth another try. Anything else is final.
pub fn seek_is_retryable(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("not active") || lower.contains("without duration")
}

/// Episode `prepare_play_episode` associated with a queue so a remote enclosure
/// URL, which is not a PinePods `/api/data/stream/` URL, still writes history.
#[derive(Debug, Clone, PartialEq)]
pub struct RememberedEpisode {
    pub user_id: i32,
    pub episode_id: i32,
    pub is_youtube: bool,
    pub duration_sec: f64,
    /// Playlist the episode was started from, so the end of it plays the next playlist item.
    pub playlist_id: Option<i32>,
    /// URL sent to Music Assistant. Empty when the episode was learned from a stream URL.
    pub media_url: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HistoryEpisode {
    /// Real stop. Drop the remembered episode.
    Clear,
    /// The event is about an item other than the episode PinePods last
    /// started: the previous item Music Assistant still reports right after
    /// `play_media`, or media started from Music Assistant itself. Drop the
    /// loaded episode without writing, and keep the remembered one.
    OtherItem,
    /// Another user's stream URL. Do not write and do not replace memory.
    Foreign,
    Write(StreamRef),
    /// No URL and nothing remembered. Leave the track alone.
    None,
}

/// Which episode a queue event should write history for.
///
/// `bare_tick_on_loaded` is a clock tick for a queue that already has an
/// episode. A tick says nothing about which item plays, so it keeps that
/// episode: after `play_episode` remembers the next one, a late tick from the
/// previous item must not be written as the new episode's position.
/// `item_strings` is what the event says about its item. When it does not
/// mention the URL PinePods last sent, the event is about another item.
pub fn history_stream(
    remembered: Option<&RememberedEpisode>,
    parsed: Option<&StreamRef>,
    media_cleared: bool,
    bridge_user: i32,
    bare_tick_on_loaded: bool,
    item_strings: Option<&[String]>,
) -> HistoryEpisode {
    if media_cleared {
        return HistoryEpisode::Clear;
    }
    if bare_tick_on_loaded {
        return HistoryEpisode::None;
    }
    if let (Some(row), Some(strings)) = (
        remembered.filter(|row| row.user_id == bridge_user && !row.media_url.is_empty()),
        item_strings,
    ) {
        if !strings.iter().any(|s| s.contains(row.media_url.as_str())) {
            return HistoryEpisode::OtherItem;
        }
    }
    if let Some(parsed) = parsed {
        if parsed.user_id.is_some_and(|id| id != bridge_user) {
            return HistoryEpisode::Foreign;
        }
        return HistoryEpisode::Write(parsed.clone());
    }
    if let Some(remembered) = remembered.filter(|row| row.user_id == bridge_user) {
        return HistoryEpisode::Write(StreamRef {
            episode_id: remembered.episode_id,
            user_id: Some(remembered.user_id),
            is_youtube: remembered.is_youtube,
        });
    }
    HistoryEpisode::None
}

/// Memory after one event. A real stop clears it. A different PinePods episode
/// replaces it. A stream URL for another user leaves it unchanged.
pub fn next_remembered(
    current: Option<RememberedEpisode>,
    parsed: Option<&StreamRef>,
    media_cleared: bool,
    bridge_user: i32,
) -> Option<RememberedEpisode> {
    if media_cleared {
        return None;
    }
    let Some(parsed) = parsed else {
        return current;
    };
    if parsed.user_id.is_some_and(|id| id != bridge_user) {
        return current;
    }
    let same = current
        .as_ref()
        .filter(|row| row.episode_id == parsed.episode_id && row.is_youtube == parsed.is_youtube);
    Some(RememberedEpisode {
        user_id: bridge_user,
        episode_id: parsed.episode_id,
        is_youtube: parsed.is_youtube,
        duration_sec: same.map(|row| row.duration_sec).unwrap_or(0.0),
        playlist_id: same.and_then(|row| row.playlist_id),
        media_url: same.map(|row| row.media_url.clone()).unwrap_or_default(),
    })
}

/// Where playback starts when the client omits `position_sec`.
///
/// An explicit value, including 0, is used as sent. An omitted value resumes
/// at `max(saved, start_skip)` so a low history row still skips the intro.
pub fn resume_position(explicit: Option<f64>, saved: f64, start_skip: i32) -> f64 {
    match explicit {
        Some(position) => position.max(0.0),
        None => saved.max(0.0).max(start_skip.max(0) as f64),
    }
}

/// Mark the episode complete at the podcast outro or the user's auto-complete
/// window, and at the natural end when neither is set.
pub fn should_mark_complete(
    position: f64,
    duration: f64,
    auto_complete_seconds: i32,
    end_skip: i32,
) -> bool {
    if !position.is_finite() || !duration.is_finite() || duration <= 0.0 {
        return false;
    }
    let end_skip = end_skip.max(0) as f64;
    if position >= duration - end_skip {
        return true;
    }
    auto_complete_seconds > 0 && (duration - position) <= auto_complete_seconds as f64
}

/// Seconds before the end that still count as finishing when the queue stops.
const NATURAL_END_GRACE_SEC: f64 = 5.0;

/// Media-time now, from the last measured position.
///
/// Music Assistant 2.10.4 sends `queue_time_updated` only around a seek, so a
/// playing queue can go minutes without an event. Its own frontend does the
/// same projection from `elapsed_time` and `elapsed_time_last_updated`.
pub fn projected_position(position: f64, updated_at: i64, playing: bool, speed: f64, now: i64) -> f64 {
    if !playing || updated_at <= 0 {
        return position.max(0.0);
    }
    let speed = if speed.is_finite() && speed > 0.0 { speed } else { 1.0 };
    (position + (now - updated_at).max(0) as f64 * speed).max(0.0)
}

/// The position to publish in a snapshot at `now`.
///
/// The relay stamps every snapshot with the time it is stored, and clients add the time since
/// then. So the position has to be the one at `now`, not the last anchor: a snapshot published
/// between anchors (a player refresh, a volume change) would otherwise jump back by up to
/// `CLOCK_ANCHOR_EVERY_SECS` and then catch up. It never passes a known duration.
pub fn snapshot_position(position: f64, updated_at: i64, playing: bool, speed: f64, duration: f64, now: i64) -> f64 {
    let projected = projected_position(position, updated_at, playing, speed, now);
    if duration > 0.0 {
        projected.min(duration)
    } else {
        projected
    }
}

/// Whether the loaded episode is finished and PinePods should play the next one.
///
/// While playing, the podcast outro (`end_skip`) finishes it, as on local web.
/// Otherwise playing → stopped near the end does. `position` is the reported
/// position, or the projected one when Music Assistant reports 0 at the end.
pub fn advance_due(
    position: f64,
    was_playing: bool,
    playing: bool,
    duration: f64,
    end_skip: i32,
) -> bool {
    if !duration.is_finite() || duration <= 0.0 || !position.is_finite() {
        return false;
    }
    let end_skip = end_skip.max(0) as f64;
    if playing {
        return end_skip > 0.0 && position >= duration - end_skip;
    }
    was_playing && position >= duration - end_skip.max(NATURAL_END_GRACE_SEC)
}

/// A finished episode is playing again from earlier on, so history writes
/// resume. The bare 0 a Chromecast sends at the end of a track while it still
/// reports playing is not a replay; a real one passes 1 second a moment later.
pub fn replayed_after_finish(playing: bool, position: f64, duration: f64, end_skip: i32) -> bool {
    let finish_line = duration - (end_skip.max(0) as f64).max(NATURAL_END_GRACE_SEC) - 1.0;
    playing && position >= 1.0 && position < finish_line
}

/// Whether a queue event ends the loaded episode.
///
/// `reported` is the event's position and `projected` the bridge's position
/// just before it. At a natural end Music Assistant 2.10.4 on a Chromecast
/// sends `queue_time_updated` 0 while still playing, then idle with 0, so a
/// jump to the start from the finish line counts as stopping there.
pub fn episode_finished(
    reported: Option<f64>,
    projected: f64,
    was_playing: bool,
    playing: bool,
    duration: f64,
    end_skip: i32,
) -> bool {
    let restarted = was_playing && reported.is_some_and(|p| p < 1.0);
    let position = reported.filter(|p| *p >= 1.0).unwrap_or(projected);
    let playing = playing && !restarted;
    advance_due(position, was_playing, playing, duration, end_skip)
}

/// Position a queue that is not playing should show and save, if Music
/// Assistant knows better than its elapsed time.
///
/// Music Assistant sets `resume_pos` when a playing queue pauses or stops and
/// resets it to 0 when playback starts. A Sonos pause is a stop, and its
/// elapsed time falls back to where the stream started, so `resume_pos` wins
/// and is held until the queue plays again.
pub fn paused_position(held: Option<f64>, resume_pos: Option<f64>, playing: bool) -> Option<f64> {
    if playing {
        return None;
    }
    resume_pos.filter(|p| p.is_finite() && *p > 0.0).or(held)
}

/// Longest sleep timer, in minutes, and most episodes, as on mobile.
pub const SLEEP_MAX_MINUTES: i64 = 12 * 60;
pub const SLEEP_MAX_EPISODES: i64 = 10;

/// Sleep timer a `sleep_timer` command asks for. `Ok(None)` turns it off.
///
/// `mode` is `off`, `minutes` (with `minutes`), `end_of_episode`, or
/// `episodes` (with `episodes`). One episode is the end of this one.
pub fn sleep_timer_from_args(args: &Value, now: i64) -> Result<Option<SleepTimer>, String> {
    let count = |key: &str, max: i64| {
        args.get(key)
            .and_then(json_i64)
            .filter(|n| (1..=max).contains(n))
            .ok_or_else(|| format!("sleep_timer needs {key} from 1 to {max}"))
    };
    match args.get("mode").and_then(Value::as_str).unwrap_or("") {
        "off" => Ok(None),
        "minutes" => Ok(Some(SleepTimer::Minutes {
            ends_at: now + count("minutes", SLEEP_MAX_MINUTES)? * 60,
        })),
        "end_of_episode" => Ok(Some(SleepTimer::EndOfEpisode)),
        "episodes" => Ok(Some(match count("episodes", SLEEP_MAX_EPISODES)? {
            1 => SleepTimer::EndOfEpisode,
            n => SleepTimer::Episodes { left: n as u32 },
        })),
        other => Err(format!("unknown sleep_timer mode {other:?}")),
    }
}

/// An episode finished: whether the speaker stops here instead of playing the
/// next one, and the timer left afterwards.
pub fn sleep_after_episode(timer: Option<SleepTimer>) -> (bool, Option<SleepTimer>) {
    match timer {
        Some(SleepTimer::EndOfEpisode) | Some(SleepTimer::Episodes { left: 0..=1 }) => (true, None),
        Some(SleepTimer::Episodes { left }) => {
            (false, Some(SleepTimer::Episodes { left: left - 1 }))
        }
        other => (false, other),
    }
}

/// A minutes timer whose time has come.
pub fn sleep_due(timer: Option<SleepTimer>, now: i64) -> bool {
    matches!(timer, Some(SleepTimer::Minutes { ends_at }) if ends_at <= now)
}

/// One PinePods queue row, as far as picking the next episode cares.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueEntry {
    pub episode_id: i32,
    pub is_youtube: bool,
    pub position: Option<i32>,
    pub completed: bool,
}

/// Next queued episode after `current`.
///
/// When an episode finishes it is the first other unfinished row, as on local
/// web. A manual skip takes the row after the current one when it is queued,
/// because a skipped episode stays in the queue and would otherwise come back.
pub fn pick_queued(
    entries: &[QueueEntry],
    current: (i32, bool),
    skip: bool,
) -> Option<(i32, bool)> {
    let order = |e: &QueueEntry| e.position.unwrap_or(i32::MAX);
    let after = entries
        .iter()
        .find(|e| skip && (e.episode_id, e.is_youtube) == current)
        .map(order);
    entries
        .iter()
        .filter(|e| (e.episode_id, e.is_youtube) != current && !e.completed)
        .filter(|e| after.is_none_or(|p| order(e) > p))
        .min_by_key(|e| order(e))
        .map(|e| (e.episode_id, e.is_youtube))
}

/// Episode PinePods plays after the current one on a speaker.
#[derive(Debug, Clone, PartialEq)]
pub struct NextEpisode {
    pub episode_id: i32,
    pub is_youtube: bool,
    pub playlist_id: Option<i32>,
}

/// Playlist next, else the podcast's auto-play-next, else the queue, in that
/// order, the same as local web. Read this before `mark_episode_completed`
/// takes the finished episode out of the queue.
pub async fn next_pinepods_episode(
    db: &DatabasePool,
    user_id: i32,
    current: (i32, bool),
    playlist_id: Option<i32>,
    skip: bool,
) -> Option<NextEpisode> {
    let (episode_id, is_youtube) = current;
    if let Some(pid) = playlist_id {
        if let Ok(Some(next)) = db.get_next_playlist_episode(episode_id, pid, user_id).await {
            return Some(NextEpisode {
                episode_id: next.episodeid,
                is_youtube: next.is_youtube,
                playlist_id: Some(pid),
            });
        }
    }
    // Auto-play-next looks the podcast up in Episodes, so a video id would hit
    // an unrelated episode with the same number.
    if !is_youtube {
        if let Ok(Some(podcast_id)) = db
            .get_podcast_id_from_episode(episode_id, user_id, false)
            .await
        {
            if let Ok(Some(true)) = db.get_auto_play_next_status(podcast_id, user_id).await {
                if let Ok(Some(next)) = db.get_next_podcast_episode(episode_id, user_id).await {
                    return Some(NextEpisode {
                        episode_id: next.episodeid,
                        is_youtube: next.is_youtube,
                        playlist_id: None,
                    });
                }
            }
        }
    }
    let queue: Vec<QueueEntry> = db
        .get_queued_episodes(user_id)
        .await
        .ok()?
        .into_iter()
        .map(|e| QueueEntry {
            episode_id: e.episodeid,
            is_youtube: e.is_youtube,
            position: e.queueposition,
            completed: e.completed,
        })
        .collect();
    pick_queued(&queue, current, skip).map(|(episode_id, is_youtube)| NextEpisode {
        episode_id,
        is_youtube,
        playlist_id: None,
    })
}

fn string_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|i| i.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}

fn canonicalize_args(user_id: i32, args: &Value) -> Value {
    let mut args = args.clone();
    let Some(obj) = args.as_object_mut() else {
        return args;
    };
    for key in ["target_player"] {
        if let Some(s) = obj.get(key).and_then(|v| v.as_str()) {
            let canon = canonical_player_id(user_id, s);
            obj.insert(key.to_string(), json!(canon));
        }
    }
    for key in [
        "player_ids",
        "members",
        "player_ids_to_add",
        "player_ids_to_remove",
    ] {
        if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
            let ids: Vec<String> = arr
                .iter()
                .filter_map(|v| v.as_str())
                .map(|s| canonical_player_id(user_id, s))
                .collect();
            obj.insert(key.to_string(), json!(ids));
        }
    }
    args
}

/// Relayed now-playing frame: `{"type":"command","action","args"}`.
pub fn parse_relay_command(text: &str) -> Option<(String, Value)> {
    let v: Value = serde_json::from_str(text).ok()?;
    if v.get("type").and_then(|t| t.as_str()) != Some("command") {
        return None;
    }
    let action = v.get("action")?.as_str()?.to_string();
    let args = v.get("args").cloned().unwrap_or_else(|| json!({}));
    Some((action, args))
}

// ============================ Wire protocol ============================

#[derive(Debug)]
pub enum Inbound {
    Result {
        message_id: String,
        result: Value,
    },
    Error {
        message_id: String,
        error_code: i64,
        details: String,
    },
    Event {
        event: String,
        object_id: Option<String>,
        data: Value,
    },
    Ignore,
}

pub fn parse_inbound(text: &str) -> Inbound {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return Inbound::Ignore;
    };
    if let Some(event) = v.get("event").and_then(|e| e.as_str()) {
        if v.get("message_id").is_none() || v.get("result").is_none() {
            return Inbound::Event {
                event: event.to_string(),
                object_id: v
                    .get("object_id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string()),
                data: v.get("data").cloned().unwrap_or(Value::Null),
            };
        }
    }
    let Some(message_id) = v.get("message_id").and_then(|s| s.as_str()) else {
        return Inbound::Ignore;
    };
    if v.get("error_code").is_some() || (v.get("result").is_none() && v.get("error").is_some()) {
        let details = v
            .get("details")
            .or_else(|| v.get("error"))
            .or_else(|| v.get("message"))
            .and_then(|s| s.as_str())
            .unwrap_or("Music Assistant command failed")
            .to_string();
        return Inbound::Error {
            message_id: message_id.to_string(),
            error_code: v.get("error_code").and_then(json_i64).unwrap_or(0),
            details,
        };
    }
    if let Some(result) = v.get("result") {
        return Inbound::Result {
            message_id: message_id.to_string(),
            result: result.clone(),
        };
    }
    Inbound::Ignore
}

#[derive(Debug)]
enum SessionNote {
    Server(ServerInfo),
    Players(Vec<MaPlayer>),
    Event {
        event: String,
        object_id: Option<String>,
        data: Value,
    },
}

struct Outbound {
    command: String,
    args: Value,
    reply: Option<oneshot::Sender<Result<Value, String>>>,
}

struct Inflight {
    reply: Option<oneshot::Sender<Result<Value, String>>>,
}

/// Speak the Music Assistant WebSocket: auth, `info`, `players/all`, then commands and events.
async fn drive_socket<S>(
    mut socket: S,
    token: &str,
    mut outbound: mpsc::UnboundedReceiver<Outbound>,
    events: mpsc::UnboundedSender<SessionNote>,
) -> Result<(), String>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    let mut next_id: u64 = 1;
    let mut inflight: HashMap<String, Inflight> = HashMap::new();

    let auth_id = send_cmd(
        &mut socket,
        &mut next_id,
        &mut inflight,
        "auth",
        json!({ "token": token }),
        None,
    )
    .await?;
    let auth_result = wait_result(
        &mut socket,
        &mut inflight,
        &events,
        &auth_id,
        HANDSHAKE_TIMEOUT,
    )
    .await?;
    if auth_result.get("authenticated").and_then(|v| v.as_bool()) == Some(false) {
        return Err("Music Assistant rejected the token".into());
    }

    let info_id = send_cmd(
        &mut socket,
        &mut next_id,
        &mut inflight,
        "info",
        json!({}),
        None,
    )
    .await?;
    match wait_result(
        &mut socket,
        &mut inflight,
        &events,
        &info_id,
        HANDSHAKE_TIMEOUT,
    )
    .await
    {
        Ok(result) => {
            let info = server_info_from_result(&result);
            if !info.assessment.compatible {
                let why = info
                    .assessment
                    .warning
                    .clone()
                    .unwrap_or_else(|| "incompatible Music Assistant version".into());
                let _ = events.send(SessionNote::Server(info));
                return Err(why);
            }
            let _ = events.send(SessionNote::Server(info));
        }
        Err(e) => {
            tracing::warn!("music assistant info command failed: {e}");
            let _ = events.send(SessionNote::Server(ServerInfo {
                server_version: String::new(),
                schema_version: None,
                server_id: None,
                assessment: VersionAssessment {
                    compatible: true,
                    warning: Some(format!("could not read server info: {e}")),
                },
            }));
        }
    }

    let players_id = send_cmd(
        &mut socket,
        &mut next_id,
        &mut inflight,
        "players/all",
        json!({}),
        None,
    )
    .await?;
    let players_result = wait_result(
        &mut socket,
        &mut inflight,
        &events,
        &players_id,
        HANDSHAKE_TIMEOUT,
    )
    .await?;
    let _ = events.send(SessionNote::Players(players_from_result(&players_result)));

    let mut refresh = tokio::time::interval(PLAYER_REFRESH);
    refresh.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    refresh.tick().await;
    let mut commands_open = true;

    loop {
        tokio::select! {
            msg = socket.next() => {
                let Some(msg) = msg else {
                    return Err("Music Assistant closed the connection".into());
                };
                let msg = msg.map_err(|e| e.to_string())?;
                handle_socket_message(msg, &mut inflight, &events).await?;
            }
            cmd = outbound.recv(), if commands_open => {
                match cmd {
                    Some(cmd) => {
                        send_cmd(&mut socket, &mut next_id, &mut inflight, &cmd.command, cmd.args, cmd.reply).await?;
                    }
                    None => commands_open = false,
                }
            }
            _ = refresh.tick() => {
                send_cmd(&mut socket, &mut next_id, &mut inflight, "players/all", json!({}), None).await?;
            }
        }
    }
}

async fn handle_socket_message(
    msg: Message,
    inflight: &mut HashMap<String, Inflight>,
    events: &mpsc::UnboundedSender<SessionNote>,
) -> Result<(), String> {
    let text = match msg {
        Message::Text(t) => t.to_string(),
        Message::Close(_) => return Err("Music Assistant closed the connection".into()),
        Message::Ping(_) | Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {
            return Ok(())
        }
    };
    match parse_inbound(&text) {
        Inbound::Result { message_id, result } => {
            if let Some(pending) = inflight.remove(&message_id) {
                if let Some(reply) = pending.reply {
                    let _ = reply.send(Ok(result.clone()));
                }
            }
            // Unsolicited-looking players/all refresh uses a message id we sent
            // with reply None. Also surface a players list when the result is an
            // array of player objects so the periodic refresh updates registration.
            if result.as_array().is_some_and(|arr| {
                arr.first()
                    .and_then(|p| p.get("player_id"))
                    .and_then(|s| s.as_str())
                    .is_some()
            }) {
                let _ = events.send(SessionNote::Players(players_from_result(&result)));
            }
        }
        Inbound::Error {
            message_id,
            details,
            ..
        } => {
            if let Some(pending) = inflight.remove(&message_id) {
                if let Some(reply) = pending.reply {
                    let _ = reply.send(Err(details));
                }
            }
        }
        Inbound::Event {
            event,
            object_id,
            data,
        } => {
            let _ = events.send(SessionNote::Event {
                event,
                object_id,
                data,
            });
        }
        Inbound::Ignore => {}
    }
    Ok(())
}

async fn send_cmd<S>(
    socket: &mut S,
    next_id: &mut u64,
    inflight: &mut HashMap<String, Inflight>,
    command: &str,
    args: Value,
    reply: Option<oneshot::Sender<Result<Value, String>>>,
) -> Result<String, String>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let id = next_id.to_string();
    *next_id += 1;
    let body = json!({
        "message_id": id,
        "command": command,
        "args": args,
    });
    socket
        .send(Message::Text(body.to_string().into()))
        .await
        .map_err(|e| format!("failed to send {command}: {e}"))?;
    inflight.insert(id.clone(), Inflight { reply });
    Ok(id)
}

async fn wait_result<S>(
    socket: &mut S,
    inflight: &mut HashMap<String, Inflight>,
    events: &mpsc::UnboundedSender<SessionNote>,
    message_id: &str,
    timeout: Duration,
) -> Result<Value, String>
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin,
{
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(format!(
                "timed out waiting for Music Assistant ({message_id})"
            ));
        }
        let msg = tokio::time::timeout(remaining, socket.next())
            .await
            .map_err(|_| "timed out waiting for Music Assistant".to_string())?
            .ok_or_else(|| "Music Assistant closed the connection".to_string())?
            .map_err(|e| e.to_string())?;
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => return Err("Music Assistant closed the connection".into()),
            _ => continue,
        };
        match parse_inbound(&text) {
            Inbound::Result {
                message_id: id,
                result,
            } if id == message_id => {
                inflight.remove(&id);
                return Ok(result);
            }
            Inbound::Error {
                message_id: id,
                details,
                ..
            } if id == message_id => {
                inflight.remove(&id);
                return Err(details);
            }
            other => {
                // Reuse the live handler by reserializing would drop the match.
                // Dispatch directly so events during handshake are not lost.
                match other {
                    Inbound::Result {
                        message_id: id,
                        result,
                    } => {
                        if let Some(pending) = inflight.remove(&id) {
                            if let Some(reply) = pending.reply {
                                let _ = reply.send(Ok(result));
                            }
                        }
                    }
                    Inbound::Error {
                        message_id: id,
                        details,
                        ..
                    } => {
                        if let Some(pending) = inflight.remove(&id) {
                            if let Some(reply) = pending.reply {
                                let _ = reply.send(Err(details));
                            }
                        }
                    }
                    Inbound::Event {
                        event,
                        object_id,
                        data,
                    } => {
                        let _ = events.send(SessionNote::Event {
                            event,
                            object_id,
                            data,
                        });
                    }
                    Inbound::Ignore => {}
                }
            }
        }
    }
}

fn redact(err: String, token: &str) -> String {
    if token.is_empty() {
        err
    } else {
        err.replace(token, "[redacted]")
    }
}

#[derive(Debug, Clone)]
pub struct ProbeReport {
    pub ok: bool,
    pub detail: String,
    pub server_version: Option<String>,
    pub schema_version: Option<i64>,
    pub player_count: i64,
    pub warning: Option<String>,
}

pub async fn probe(ws_url: &str, token: &str) -> ProbeReport {
    let connect = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async(ws_url),
    )
    .await;
    let socket = match connect {
        Ok(Ok((socket, _))) => socket,
        Ok(Err(e)) => {
            return ProbeReport::fail(redact(format!("could not connect: {e}"), token));
        }
        Err(_) => return ProbeReport::fail("timed out connecting to Music Assistant"),
    };
    probe_socket(socket, token).await
}

/// The probe on an open socket: authenticate, read the server info, list the players.
async fn probe_socket<S>(socket: S, token: &str) -> ProbeReport
where
    S: Stream<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Sink<Message, Error = tokio_tungstenite::tungstenite::Error>
        + Unpin
        + Send
        + 'static,
{
    let token_owned = token.to_string();
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (note_tx, mut note_rx) = mpsc::unbounded_channel();
    let token_for_drive = token_owned.clone();
    let mut driver =
        tokio::spawn(async move { drive_socket(socket, &token_for_drive, cmd_rx, note_tx).await });
    let mut info: Option<ServerInfo> = None;
    let mut player_count: Option<i64> = None;
    let mut drive_err: Option<String> = None;
    let mut timed_out = false;
    let driver_end = |res: Result<Result<(), String>, tokio::task::JoinError>| match res {
        Ok(Ok(())) => "connection closed".to_string(),
        Ok(Err(e)) => e,
        Err(e) => format!("connection task stopped: {e}"),
    };
    let deadline = tokio::time::sleep(Duration::from_secs(15));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => {
                timed_out = true;
                break;
            }
            note = note_rx.recv() => {
                match note {
                    Some(SessionNote::Server(s)) => info = Some(s),
                    Some(SessionNote::Players(p)) => {
                        player_count = Some(p.len() as i64);
                        break;
                    }
                    Some(_) => {}
                    None => {
                        // The driver ended and dropped its sender. Its result says why: a
                        // rejected token ends it, and `select!` may see the closed channel
                        // before the finished task. Breaking here without the result
                        // reported a rejected token as "connected, 0 player(s)".
                        drive_err = Some(driver_end((&mut driver).await));
                        break;
                    }
                }
            }
            res = &mut driver => {
                drive_err = Some(driver_end(res));
                break;
            }
        }
    }
    drop(cmd_tx);
    driver.abort();
    if timed_out && player_count.is_none() {
        // Nothing listed the players, so nothing shows the token was accepted.
        drive_err = Some("timed out waiting for Music Assistant".into());
    }

    if let Some(err) = drive_err {
        if player_count.is_none() {
            let warning = info.as_ref().and_then(|i| i.assessment.warning.clone());
            return ProbeReport {
                ok: false,
                detail: redact(err, &token_owned),
                server_version: info
                    .as_ref()
                    .map(|i| i.server_version.clone())
                    .filter(|s| !s.is_empty()),
                schema_version: info.as_ref().and_then(|i| i.schema_version),
                player_count: 0,
                warning,
            };
        }
    }
    let warning = info.as_ref().and_then(|i| i.assessment.warning.clone());
    let compatible = info
        .as_ref()
        .map(|i| i.assessment.compatible)
        .unwrap_or(true);
    let count = player_count.unwrap_or(0);
    if !compatible {
        return ProbeReport {
            ok: false,
            detail: warning
                .clone()
                .unwrap_or_else(|| "incompatible Music Assistant".into()),
            server_version: info.as_ref().map(|i| i.server_version.clone()),
            schema_version: info.as_ref().and_then(|i| i.schema_version),
            player_count: count,
            warning,
        };
    }
    ProbeReport {
        ok: true,
        detail: format!("connected, {count} player(s)"),
        server_version: info
            .as_ref()
            .map(|i| i.server_version.clone())
            .filter(|s| !s.is_empty()),
        schema_version: info.as_ref().and_then(|i| i.schema_version),
        player_count: count,
        warning,
    }
}

impl ProbeReport {
    fn fail(detail: impl Into<String>) -> Self {
        Self {
            ok: false,
            detail: detail.into(),
            server_version: None,
            schema_version: None,
            player_count: 0,
            warning: None,
        }
    }
}

// ============================ Settings =================================

#[derive(Debug, Clone)]
struct StoredSettings {
    enabled: bool,
    server_url: Option<String>,
    token_encrypted: Option<String>,
    external_base_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MaSettingsView {
    pub enabled: bool,
    pub server_url: Option<String>,
    pub external_base_url: Option<String>,
    pub has_token: bool,
    pub connected: bool,
    pub server_version: Option<String>,
    pub schema_version: Option<i64>,
    pub warning: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MaSettingsUpdate {
    pub user_id: i32,
    pub enabled: bool,
    pub server_url: Option<String>,
    pub external_base_url: Option<String>,
    pub token: Option<String>,
    pub clear_token: bool,
}

pub async fn load_settings_view(state: &AppState, user_id: i32) -> AppResult<MaSettingsView> {
    let stored = read_settings(&state.db_pool, user_id).await?;
    let live = state.music_assistant.status(user_id).await;
    Ok(view_from(&stored, &live))
}

fn view_from(stored: &StoredSettings, live: &SessionStatus) -> MaSettingsView {
    MaSettingsView {
        enabled: stored.enabled,
        server_url: stored.server_url.clone(),
        external_base_url: stored.external_base_url.clone(),
        has_token: stored
            .token_encrypted
            .as_deref()
            .map(|t| !t.is_empty())
            .unwrap_or(false),
        connected: live.connected,
        server_version: live.server_version.clone(),
        schema_version: live.schema_version,
        warning: live.warning.clone(),
        last_error: live.last_error.clone(),
    }
}

pub async fn save_settings(
    state: &AppState,
    update: MaSettingsUpdate,
) -> AppResult<MaSettingsView> {
    let existing = read_settings(&state.db_pool, update.user_id).await?;
    let mut server_url = match update
        .server_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => Some(parse_server_url(raw).map_err(AppError::bad_request)?.origin),
        None => existing.server_url.clone(),
    };
    let mut external = match update
        .external_base_url
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(raw) => Some(normalize_external_base(raw).map_err(AppError::bad_request)?),
        None => existing.external_base_url.clone(),
    };
    // Touch the JSON-RPC URL so a typo'd path fails at save time, and so the
    // field is not flagged unused when only the WebSocket is dialed.
    if let Some(raw) = server_url.as_deref() {
        let endpoints = parse_server_url(raw).map_err(AppError::bad_request)?;
        tracing::debug!("music assistant json-rpc endpoint {}", endpoints.api);
        server_url = Some(endpoints.origin);
    }
    let mut token_encrypted = existing.token_encrypted.clone();
    if update.clear_token {
        token_encrypted = None;
    }
    if let Some(token) = update
        .token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        token_encrypted = Some(state.db_pool.encrypt_password(token).await?);
    }
    if update.enabled {
        if server_url.as_deref().unwrap_or("").is_empty() {
            return Err(AppError::bad_request(
                "server_url is required to enable Music Assistant",
            ));
        }
        if external.as_deref().unwrap_or("").is_empty() {
            return Err(AppError::bad_request(
                "external_base_url is required so Music Assistant can reach PinePods",
            ));
        }
        if token_encrypted.as_deref().unwrap_or("").is_empty() {
            return Err(AppError::bad_request(
                "a Music Assistant token is required to enable the connection",
            ));
        }
    }
    if !update.enabled {
        // Keep previously validated values when the client omits them.
        let _ = (&mut server_url, &mut external);
    }
    write_settings(
        &state.db_pool,
        update.user_id,
        StoredSettings {
            enabled: update.enabled,
            server_url,
            token_encrypted,
            external_base_url: external,
        },
    )
    .await?;
    state.music_assistant.request_reload();
    load_settings_view(state, update.user_id).await
}

pub async fn test_connection(
    state: &AppState,
    user_id: i32,
    server_url: Option<&str>,
    token: Option<&str>,
) -> AppResult<ProbeReport> {
    let stored = read_settings(&state.db_pool, user_id).await?;
    let url_raw = server_url
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or(stored.server_url.clone());
    let Some(url_raw) = url_raw else {
        return Ok(ProbeReport::fail(
            "Music Assistant is not configured. Provide a server URL and token.",
        ));
    };
    let endpoints = parse_server_url(&url_raw).map_err(AppError::bad_request)?;
    let token_owned = if let Some(t) = token.map(str::trim).filter(|t| !t.is_empty()) {
        t.to_string()
    } else if let Some(enc) = stored.token_encrypted.as_deref().filter(|t| !t.is_empty()) {
        state.db_pool.decrypt_password(enc).await?
    } else {
        return Ok(ProbeReport::fail(
            "Music Assistant is not configured. Provide a server URL and token.",
        ));
    };
    Ok(probe(&endpoints.ws, &token_owned).await)
}

async fn read_settings(db: &DatabasePool, user_id: i32) -> AppResult<StoredSettings> {
    dialect_match!(db, {
        pg(pool) => {
            let row = q::query(
                r#"SELECT CASE WHEN enabled THEN 1 ELSE 0 END AS enabled,
                          serverurl AS server_url,
                          tokenencrypted AS token_encrypted,
                          externalbaseurl AS external_base_url
                   FROM "MusicAssistant" WHERE userid = $1"#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(match row {
                Some(row) => map_stored(
                    row.try_get::<i32, _>("enabled")? as i64,
                    row.try_get("server_url")?,
                    row.try_get("token_encrypted")?,
                    row.try_get("external_base_url")?,
                ),
                None => StoredSettings::disabled(),
            })
        },
        mysql(pool) => {
            let row = sqlx::query(
                r#"SELECT IF(Enabled, 1, 0) AS enabled,
                          ServerUrl AS server_url,
                          TokenEncrypted AS token_encrypted,
                          ExternalBaseUrl AS external_base_url
                   FROM MusicAssistant WHERE UserID = ?"#,
            )
            .bind(user_id)
            .fetch_optional(pool)
            .await?;
            Ok(match row {
                Some(row) => {
                    let enabled: i64 = row
                        .try_get::<i64, _>("enabled")
                        .or_else(|_| row.try_get::<i32, _>("enabled").map(|v| v as i64))?;
                    map_stored(
                        enabled,
                        row.try_get("server_url")?,
                        row.try_get("token_encrypted")?,
                        row.try_get("external_base_url")?,
                    )
                }
                None => StoredSettings::disabled(),
            })
        },
    })
}

impl StoredSettings {
    fn disabled() -> Self {
        Self {
            enabled: false,
            server_url: None,
            token_encrypted: None,
            external_base_url: None,
        }
    }
}

fn map_stored(
    enabled: i64,
    server_url: Option<String>,
    token: Option<String>,
    base: Option<String>,
) -> StoredSettings {
    let clean = |v: Option<String>| v.filter(|s| !s.is_empty());
    StoredSettings {
        enabled: enabled != 0,
        server_url: clean(server_url),
        token_encrypted: clean(token),
        external_base_url: clean(base),
    }
}

async fn write_settings(db: &DatabasePool, user_id: i32, stored: StoredSettings) -> AppResult<()> {
    dialect_match!(db, {
        pg(pool) => {
            q::query(
                r#"INSERT INTO "MusicAssistant" (userid, enabled, serverurl, tokenencrypted, externalbaseurl, updatedat)
                   VALUES ($1, $2, $3, $4, $5, NOW())
                   ON CONFLICT (userid) DO UPDATE SET
                     enabled = EXCLUDED.enabled,
                     serverurl = EXCLUDED.serverurl,
                     tokenencrypted = EXCLUDED.tokenencrypted,
                     externalbaseurl = EXCLUDED.externalbaseurl,
                     updatedat = NOW()"#,
            )
            .bind(user_id)
            .bind(stored.enabled)
            .bind(&stored.server_url)
            .bind(&stored.token_encrypted)
            .bind(&stored.external_base_url)
            .execute(pool)
            .await?;
        },
        mysql(pool) => {
            sqlx::query(
                r#"INSERT INTO MusicAssistant (UserID, Enabled, ServerUrl, TokenEncrypted, ExternalBaseUrl, UpdatedAt)
                   VALUES (?, ?, ?, ?, ?, NOW())
                   ON DUPLICATE KEY UPDATE
                     Enabled = VALUES(Enabled),
                     ServerUrl = VALUES(ServerUrl),
                     TokenEncrypted = VALUES(TokenEncrypted),
                     ExternalBaseUrl = VALUES(ExternalBaseUrl),
                     UpdatedAt = NOW()"#,
            )
            .bind(user_id)
            .bind(stored.enabled)
            .bind(&stored.server_url)
            .bind(&stored.token_encrypted)
            .bind(&stored.external_base_url)
            .execute(pool)
            .await?;
        },
    });
    Ok(())
}

async fn list_enabled(db: &DatabasePool) -> AppResult<Vec<(i32, StoredSettings)>> {
    dialect_match!(db, {
        pg(pool) => {
            let rows = q::query(
                r#"SELECT userid AS user_id,
                          CASE WHEN enabled THEN 1 ELSE 0 END AS enabled,
                          serverurl AS server_url,
                          tokenencrypted AS token_encrypted,
                          externalbaseurl AS external_base_url
                   FROM "MusicAssistant"
                   WHERE enabled = TRUE
                     AND serverurl IS NOT NULL AND serverurl <> ''
                     AND tokenencrypted IS NOT NULL AND tokenencrypted <> ''
                     AND externalbaseurl IS NOT NULL AND externalbaseurl <> ''"#,
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::new();
            for row in rows {
                let user_id: i32 = row.try_get("user_id")?;
                let enabled: i32 = row.try_get("enabled")?;
                out.push((
                    user_id,
                    map_stored(
                        enabled as i64,
                        row.try_get("server_url")?,
                        row.try_get("token_encrypted")?,
                        row.try_get("external_base_url")?,
                    ),
                ));
            }
            Ok(out)
        },
        mysql(pool) => {
            let rows = sqlx::query(
                r#"SELECT UserID AS user_id,
                          IF(Enabled, 1, 0) AS enabled,
                          ServerUrl AS server_url,
                          TokenEncrypted AS token_encrypted,
                          ExternalBaseUrl AS external_base_url
                   FROM MusicAssistant
                   WHERE Enabled = 1
                     AND ServerUrl IS NOT NULL AND ServerUrl <> ''
                     AND TokenEncrypted IS NOT NULL AND TokenEncrypted <> ''
                     AND ExternalBaseUrl IS NOT NULL AND ExternalBaseUrl <> ''"#,
            )
            .fetch_all(pool)
            .await?;
            let mut out = Vec::new();
            for row in rows {
                let user_id: i32 = match row.try_get("user_id") {
                    Ok(v) => v,
                    Err(_) => row.try_get::<i64, _>("user_id")? as i32,
                };
                let enabled: i64 = row
                    .try_get::<i64, _>("enabled")
                    .or_else(|_| row.try_get::<i32, _>("enabled").map(|v| v as i64))?;
                out.push((
                    user_id,
                    map_stored(
                        enabled,
                        row.try_get("server_url")?,
                        row.try_get("token_encrypted")?,
                        row.try_get("external_base_url")?,
                    ),
                ));
            }
            Ok(out)
        },
    })
}

// ============================ Hub and session ============================

#[derive(Debug, Clone)]
pub struct ActionOutcome {
    pub detail: String,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SessionStatus {
    pub connected: bool,
    pub server_version: Option<String>,
    pub schema_version: Option<i64>,
    pub warning: Option<String>,
    pub last_error: Option<String>,
}

/// A resume seek that has not landed yet. `arm` tells a newer `play_episode` from an older one.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PendingResume {
    arm: u64,
    position_sec: f64,
}

struct SessionLive {
    connected: AtomicBool,
    players: RwLock<Vec<MaPlayer>>,
    server_version: RwLock<Option<String>>,
    schema_version: RwLock<Option<i64>>,
    warning: RwLock<Option<String>>,
    last_error: RwLock<Option<String>>,
    /// queue id → episode started by `prepare_play_episode`, including remote URLs.
    episodes: RwLock<HashMap<String, RememberedEpisode>>,
    /// Duration last reported by a queue event. Metadata duration does not count.
    queue_duration: RwLock<HashMap<String, f64>>,
    /// Offset to seek once that queue reports a duration, until the seek lands
    /// or a newer episode replaces it. No history is written for the queue meanwhile.
    pending_resume: RwLock<HashMap<String, PendingResume>>,
    /// Source of [`PendingResume::arm`].
    resume_arms: AtomicU64,
    /// Sleep timer per queue. In memory: a restart drops it.
    sleep: RwLock<HashMap<String, SleepTimer>>,
    /// queue id → when `play_episode` sent its media, until a queue event shows that media.
    /// See [`stale_empty_queue`].
    loading: RwLock<HashMap<String, Instant>>,
}

impl SessionLive {
    fn new() -> Self {
        Self {
            connected: AtomicBool::new(false),
            players: RwLock::new(Vec::new()),
            server_version: RwLock::new(None),
            schema_version: RwLock::new(None),
            warning: RwLock::new(None),
            last_error: RwLock::new(None),
            episodes: RwLock::new(HashMap::new()),
            queue_duration: RwLock::new(HashMap::new()),
            pending_resume: RwLock::new(HashMap::new()),
            resume_arms: AtomicU64::new(0),
            sleep: RwLock::new(HashMap::new()),
            loading: RwLock::new(HashMap::new()),
        }
    }

    async fn begin_load(&self, queue_id: &str) {
        self.loading.write().await.insert(queue_id.to_string(), Instant::now());
    }

    async fn end_load(&self, queue_id: &str) {
        self.loading.write().await.remove(queue_id);
    }

    async fn loading_for(&self, queue_id: &str) -> Option<Duration> {
        self.loading.read().await.get(queue_id).map(Instant::elapsed)
    }

    async fn sleep_timer(&self, queue_id: &str) -> Option<SleepTimer> {
        self.sleep.read().await.get(queue_id).copied()
    }

    async fn set_sleep_timer(&self, queue_id: &str, timer: Option<SleepTimer>) {
        let mut sleep = self.sleep.write().await;
        match timer {
            Some(timer) => sleep.insert(queue_id.to_string(), timer),
            None => sleep.remove(queue_id),
        };
    }

    /// Minutes timers that are due, removed so each fires once.
    async fn take_due_sleep(&self, now: i64) -> Vec<String> {
        let mut sleep = self.sleep.write().await;
        let due: Vec<String> = sleep
            .iter()
            .filter(|(_, timer)| sleep_due(Some(**timer), now))
            .map(|(queue_id, _)| queue_id.clone())
            .collect();
        for queue_id in &due {
            sleep.remove(queue_id);
        }
        due
    }

    async fn remember(&self, queue_id: &str, episode: RememberedEpisode) {
        self.episodes
            .write()
            .await
            .insert(queue_id.to_string(), episode);
    }

    async fn remembered(&self, queue_id: &str) -> Option<RememberedEpisode> {
        self.episodes.read().await.get(queue_id).cloned()
    }

    async fn forget(&self, queue_id: &str) {
        self.episodes.write().await.remove(queue_id);
        self.queue_duration.write().await.remove(queue_id);
        self.pending_resume.write().await.remove(queue_id);
        self.sleep.write().await.remove(queue_id);
        self.loading.write().await.remove(queue_id);
    }

    async fn note_queue_duration(&self, queue_id: &str, duration: f64) {
        if duration > 0.0 {
            self.queue_duration
                .write()
                .await
                .insert(queue_id.to_string(), duration);
        }
    }

    async fn queue_duration(&self, queue_id: &str) -> f64 {
        self.queue_duration
            .read()
            .await
            .get(queue_id)
            .copied()
            .unwrap_or(0.0)
    }

    /// A new episode is loading on `queue_id`. The previous episode's duration
    /// must not release its seek, so it is dropped here, and a newer arm
    /// replaces any seek still waiting. Returns the arm of the seek to land,
    /// or `None` when the offset is under a second and there is nothing to seek.
    async fn arm_resume(&self, queue_id: &str, position_sec: f64) -> Option<u64> {
        self.queue_duration.write().await.remove(queue_id);
        let mut pending = self.pending_resume.write().await;
        if position_sec >= 1.0 {
            let arm = self.resume_arms.fetch_add(1, Ordering::Relaxed) + 1;
            pending.insert(queue_id.to_string(), PendingResume { arm, position_sec });
            Some(arm)
        } else {
            pending.remove(queue_id);
            None
        }
    }

    async fn pending_resume(&self, queue_id: &str) -> Option<PendingResume> {
        self.pending_resume.read().await.get(queue_id).copied()
    }

    /// The seek armed as `arm` is over. A newer `play_episode` keeps its own.
    async fn finish_resume(&self, queue_id: &str, arm: u64) {
        let mut pending = self.pending_resume.write().await;
        if pending.get(queue_id).is_some_and(|p| p.arm == arm) {
            pending.remove(queue_id);
        }
    }
}

struct SessionSlot {
    outbound: mpsc::UnboundedSender<Outbound>,
    live: Arc<SessionLive>,
    fingerprint: String,
    cancel: watch::Sender<bool>,
}

struct HubInner {
    sessions: RwLock<HashMap<i32, SessionSlot>>,
    reload: Notify,
}

#[derive(Clone)]
pub struct MusicAssistantHub {
    inner: Arc<HubInner>,
}

impl MusicAssistantHub {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(HubInner {
                sessions: RwLock::new(HashMap::new()),
                reload: Notify::new(),
            }),
        }
    }

    pub fn request_reload(&self) {
        self.inner.reload.notify_waiters();
    }

    pub async fn status(&self, user_id: i32) -> SessionStatus {
        let sessions = self.inner.sessions.read().await;
        let Some(slot) = sessions.get(&user_id) else {
            return SessionStatus::default();
        };
        let connected = slot.live.connected.load(Ordering::Relaxed);
        let server_version = slot.live.server_version.read().await.clone();
        let schema_version = *slot.live.schema_version.read().await;
        let warning = slot.live.warning.read().await.clone();
        let last_error = slot.live.last_error.read().await.clone();
        drop(sessions);
        SessionStatus {
            connected,
            server_version,
            schema_version,
            warning,
            last_error,
        }
    }

    async fn session_live(&self, user_id: i32) -> Option<Arc<SessionLive>> {
        self.inner
            .sessions
            .read()
            .await
            .get(&user_id)
            .map(|slot| slot.live.clone())
    }

    pub async fn players(&self, user_id: i32) -> Vec<MaPlayer> {
        let sessions = self.inner.sessions.read().await;
        let Some(slot) = sessions.get(&user_id) else {
            return Vec::new();
        };
        let players = slot.live.players.read().await.clone();
        drop(sessions);
        players
    }

    async fn send_commands(
        &self,
        user_id: i32,
        commands: Vec<MaCommand>,
    ) -> AppResult<ActionOutcome> {
        let tx = {
            let sessions = self.inner.sessions.read().await;
            sessions
                .get(&user_id)
                .map(|s| s.outbound.clone())
                .ok_or_else(|| AppError::service_unavailable("Music Assistant is not connected"))?
        };
        let mut warnings = Vec::new();
        for (i, cmd) in commands.into_iter().enumerate() {
            let (reply_tx, reply_rx) = oneshot::channel();
            let name = cmd.command.clone();
            tx.send(Outbound {
                command: cmd.command,
                args: cmd.args,
                reply: Some(reply_tx),
            })
            .map_err(|_| AppError::service_unavailable("Music Assistant is not connected"))?;
            let result = tokio::time::timeout(COMMAND_TIMEOUT, reply_rx)
                .await
                .map_err(|_| AppError::service_unavailable("Music Assistant did not respond"))?
                .map_err(|_| AppError::service_unavailable("Music Assistant connection lost"))?;
            match result {
                Ok(_) => {}
                Err(e) if i == 0 => return Err(AppError::bad_request(e)),
                Err(e) => warnings.push(format!("{name}: {e}")),
            }
        }
        Ok(ActionOutcome {
            detail: "ok".into(),
            warnings,
        })
    }
}

impl Default for MusicAssistantHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply a relay or REST action for one of the user's Music Assistant players.
pub async fn dispatch(
    state: &AppState,
    user_id: i32,
    player_or_device: &str,
    action: &str,
    args: &Value,
) -> AppResult<ActionOutcome> {
    let player_id = canonical_player_id(user_id, player_or_device);
    if player_id.is_empty() {
        return Err(AppError::bad_request("player_id is required"));
    }
    let args = canonicalize_args(user_id, args);
    let players = state.music_assistant.players(user_id).await;
    let queue_id = queue_id_for(&players, &player_id);
    match action {
        "play_episode" => play_on_queue(state, user_id, &queue_id, &args).await,
        "next" => play_next(state, user_id, &queue_id).await,
        "sleep_timer" => set_sleep_timer(state, user_id, &queue_id, &args).await,
        _ => {
            let commands = translate_action(&player_id, &queue_id, action, &args)
                .map_err(AppError::bad_request)?;
            state.music_assistant.send_commands(user_id, commands).await
        }
    }
}

async fn play_on_queue(
    state: &AppState,
    user_id: i32,
    queue_id: &str,
    args: &Value,
) -> AppResult<ActionOutcome> {
    let (load, arm) = prepare_play_episode(state, user_id, queue_id, args).await?;
    let hub = state.music_assistant.clone();
    let queue_id = queue_id.to_string();
    // Its own task: a client that hangs up mid-command must neither strand the
    // resume seek nor leave its history hold armed.
    tokio::spawn(async move { load_and_seek(&hub, user_id, &queue_id, load, arm).await })
        .await
        .map_err(|e| AppError::internal(format!("play_episode: {e}")))?
}

/// How `play_episode` loads the episode on the queue.
#[derive(Debug, Clone, PartialEq)]
enum EpisodeLoad {
    /// Nothing to resume: `play_media` replaces the queue and plays from the start.
    FromStart(Vec<MaCommand>),
    /// A resume point: see [`StagedStart`].
    AtOffset(StagedStart),
}

/// Load the episode and get it to its resume point. History waits for `arm` until then.
///
/// A resume point starts the speaker at the offset. Only when Music Assistant refuses that
/// does the episode start from the beginning, with the seek that waits for the queue's
/// duration; a seek still waiting after the inline second carries on in the background.
async fn load_and_seek(
    hub: &MusicAssistantHub,
    user_id: i32,
    queue_id: &str,
    load: EpisodeLoad,
    arm: Option<u64>,
) -> AppResult<ActionOutcome> {
    let loaded = send_load(hub, user_id, load).await;
    let (mut outcome, at_offset) = match loaded {
        Ok(loaded) => loaded,
        Err(e) => {
            // Nothing loaded, so no seek follows and history must not wait for one.
            if let Some(arm) = arm {
                release_resume(hub, user_id, queue_id, arm).await;
            }
            if let Some(live) = hub.session_live(user_id).await {
                live.end_load(queue_id).await;
            }
            return Err(e);
        }
    };
    let Some(arm) = arm else {
        return Ok(outcome);
    };
    if at_offset {
        release_resume(hub, user_id, queue_id, arm).await;
        return Ok(outcome);
    }
    match land_resume_seek(hub, user_id, queue_id, arm, RESUME_SEEK_INLINE).await {
        ResumeSeek::Landed | ResumeSeek::Superseded => {}
        ResumeSeek::Failed(warning) => outcome.warnings.push(warning),
        ResumeSeek::TimedOut => resume_in_background(hub, user_id, queue_id, arm),
    }
    Ok(outcome)
}

/// Send the load. The flag is true when the speaker was started at its resume point, so no
/// seek is owed.
async fn send_load(
    hub: &MusicAssistantHub,
    user_id: i32,
    load: EpisodeLoad,
) -> AppResult<(ActionOutcome, bool)> {
    let staged = match load {
        EpisodeLoad::FromStart(commands) => {
            return Ok((hub.send_commands(user_id, commands).await?, false));
        }
        EpisodeLoad::AtOffset(staged) => staged,
    };
    // One call each: `send_commands` only requires its first command to succeed.
    for command in staged.stage {
        hub.send_commands(user_id, vec![command]).await?;
    }
    match hub.send_commands(user_id, vec![staged.start]).await {
        Ok(outcome) => Ok((outcome, true)),
        Err(refused) => {
            let mut outcome = hub.send_commands(user_id, vec![staged.from_start]).await?;
            outcome.warnings.push(format!("player_queues/play_index at the resume point: {refused}"));
            Ok((outcome, false))
        }
    }
}

/// Set or clear the sleep timer on the leader queue, then show it on the
/// speaker rows right away instead of at the next Music Assistant event.
async fn set_sleep_timer(
    state: &AppState,
    user_id: i32,
    queue_id: &str,
    args: &Value,
) -> AppResult<ActionOutcome> {
    let timer = sleep_timer_from_args(args, unix_secs()).map_err(AppError::bad_request)?;
    let live = state
        .music_assistant
        .session_live(user_id)
        .await
        .ok_or_else(|| AppError::service_unavailable("Music Assistant is not connected"))?;
    live.set_sleep_timer(queue_id, timer).await;
    let players = live.players.read().await.clone();
    let rows = state.redis_client.get_now_playing_devices(user_id).await?;
    let mut wrote = false;
    for mut row in rows {
        let related = row.ma_player_id.as_deref().is_some_and(|id| {
            id == queue_id
                || players
                    .iter()
                    .any(|p| p.player_id == id && player_related(p, queue_id))
        });
        if related {
            row.sleep_timer = timer;
            if state
                .redis_client
                .upsert_now_playing(user_id, row)
                .await
                .is_ok()
            {
                wrote = true;
            }
        }
    }
    if wrote {
        notify_devices_changed(state, user_id, "").await;
    }
    Ok(ActionOutcome {
        detail: "ok".into(),
        warnings: Vec::new(),
    })
}

fn next_args(next: &NextEpisode) -> Value {
    let mut args = json!({ "episode_id": next.episode_id, "is_youtube": next.is_youtube });
    if let Some(pid) = next.playlist_id {
        args["playlist_id"] = json!(pid);
    }
    args
}

/// `detail` of a `next` that found nothing to play. The speaker keeps playing,
/// so it is not an error.
pub const NOTHING_NEXT: &str = "nothing_next";

/// Manual skip to the next PinePods item. The current episode is not marked
/// complete, the same as the local web skip.
async fn play_next(state: &AppState, user_id: i32, queue_id: &str) -> AppResult<ActionOutcome> {
    let live = state
        .music_assistant
        .session_live(user_id)
        .await
        .ok_or_else(|| AppError::service_unavailable("Music Assistant is not connected"))?;
    let current = live
        .remembered(queue_id)
        .await
        .filter(|row| row.user_id == user_id)
        .ok_or_else(|| AppError::bad_request("No PinePods episode is playing on this speaker"))?;
    let Some(next) = next_pinepods_episode(
        &state.db_pool,
        user_id,
        (current.episode_id, current.is_youtube),
        current.playlist_id,
        true,
    )
    .await
    else {
        return Ok(ActionOutcome {
            detail: NOTHING_NEXT.into(),
            warnings: Vec::new(),
        });
    };
    play_on_queue(state, user_id, queue_id, &next_args(&next)).await
}

/// The episode on `queue_id` finished. Pick the next item first, because
/// completion removes the finished one from the PinePods queue, then mark it
/// complete and play the next one. With nothing next, an outro pauses like
/// local web does. Runs off the session loop, which must keep forwarding the
/// commands this sends.
async fn advance_queue(
    state: AppState,
    user_id: i32,
    queue_id: String,
    finished: StreamRef,
    playlist_id: Option<i32>,
    still_playing: bool,
) {
    let current = (finished.episode_id, finished.is_youtube);
    let live = state.music_assistant.session_live(user_id).await;
    let timer = match &live {
        Some(live) => live.sleep_timer(&queue_id).await,
        None => None,
    };
    let (sleep_here, timer_left) = sleep_after_episode(timer);
    if let (Some(live), Some(_)) = (&live, timer) {
        live.set_sleep_timer(&queue_id, timer_left).await;
    }
    let next = if sleep_here {
        None
    } else {
        next_pinepods_episode(&state.db_pool, user_id, current, playlist_id, false).await
    };
    if let Err(e) = state
        .db_pool
        .mark_episode_completed(
            finished.episode_id,
            user_id,
            finished.is_youtube,
            crate::services::sync::SyncOrigin::Pinepods,
        )
        .await
    {
        tracing::warn!(
            "music assistant completion for episode {}: {e}",
            finished.episode_id
        );
    }
    let result = match next {
        Some(next) => play_on_queue(&state, user_id, &queue_id, &next_args(&next))
            .await
            .map(|_| ()),
        None if still_playing => {
            let pause = MaCommand {
                command: "player_queues/pause".into(),
                args: json!({ "queue_id": queue_id }),
            };
            state
                .music_assistant
                .send_commands(user_id, vec![pause])
                .await
                .map(|_| ())
        }
        None => Ok(()),
    };
    if let Err(e) = result {
        tracing::warn!("music assistant advance on {queue_id}: {e}");
    }
}

/// How a resume seek ended.
#[derive(Debug, PartialEq)]
enum ResumeSeek {
    /// The seek was sent, or the offset needed none.
    Landed,
    /// A newer `play_episode` or a real stop owns the queue now.
    Superseded,
    /// The queue still reported no duration when the patience ran out. The seek
    /// stays armed; the caller decides whether to keep waiting.
    TimedOut,
    Failed(String),
}

/// One seek to the leader queue once it reports a duration.
///
/// A speaker can take many seconds to report its duration, so this polls until
/// `patience` is spent. The seek belongs to the `play_episode` that armed it:
/// when a newer one or a real stop took the queue over, nothing is sent.
/// `Landed` and `Failed` end the seek; `TimedOut` leaves it armed.
async fn land_resume_seek(
    hub: &MusicAssistantHub,
    user_id: i32,
    queue_id: &str,
    arm: u64,
    patience: Duration,
) -> ResumeSeek {
    let started = Instant::now();
    loop {
        let Some(live) = hub.session_live(user_id).await else {
            return ResumeSeek::Superseded;
        };
        let pending = live.pending_resume(queue_id).await;
        let Some(pending) = pending.filter(|p| p.arm == arm) else {
            return ResumeSeek::Superseded;
        };
        let duration = live.queue_duration(queue_id).await;
        match plan_resume_seek(pending.position_sec, duration, started.elapsed(), patience) {
            SeekPlan::Skip => {
                live.finish_resume(queue_id, arm).await;
                return ResumeSeek::Landed;
            }
            SeekPlan::Wait => tokio::time::sleep(RESUME_SEEK_POLL).await,
            SeekPlan::GiveUp(_) => return ResumeSeek::TimedOut,
            SeekPlan::Send(seconds) => {
                let seek = MaCommand {
                    command: "player_queues/seek".into(),
                    args: json!({ "queue_id": queue_id, "position": seconds }),
                };
                match hub.send_commands(user_id, vec![seek]).await {
                    Ok(_) => {
                        live.finish_resume(queue_id, arm).await;
                        return ResumeSeek::Landed;
                    }
                    Err(e) => {
                        if seek_is_retryable(&e.to_string()) && started.elapsed() < patience {
                            tokio::time::sleep(RESUME_SEEK_POLL).await;
                            continue;
                        }
                        live.finish_resume(queue_id, arm).await;
                        return ResumeSeek::Failed(format!("player_queues/seek: {e}"));
                    }
                }
            }
        }
    }
}

/// Keep waiting for the queue's duration after the command response went out.
/// Nobody is left to hear a failure, so it goes to the log.
fn resume_in_background(hub: &MusicAssistantHub, user_id: i32, queue_id: &str, arm: u64) {
    let hub = hub.clone();
    let queue_id = queue_id.to_string();
    tokio::spawn(async move {
        let ended = land_resume_seek(&hub, user_id, &queue_id, arm, RESUME_SEEK_PATIENCE).await;
        let reason = match ended {
            ResumeSeek::Landed | ResumeSeek::Superseded => return,
            ResumeSeek::TimedOut => {
                release_resume(&hub, user_id, &queue_id, arm).await;
                "queue without duration".to_string()
            }
            ResumeSeek::Failed(warning) => warning,
        };
        tracing::warn!("music assistant resume seek on {queue_id}: {reason}");
    });
}

/// The seek armed as `arm` will not run, so history stops waiting for it.
async fn release_resume(hub: &MusicAssistantHub, user_id: i32, queue_id: &str, arm: u64) {
    if let Some(live) = hub.session_live(user_id).await {
        live.finish_resume(queue_id, arm).await;
    }
}

/// How to load the episode, and the arm that holds history back until the episode is at its
/// resume point. The arm is `None` when the offset is under a second or the user has no
/// session.
async fn prepare_play_episode(
    state: &AppState,
    user_id: i32,
    queue_id: &str,
    args: &Value,
) -> AppResult<(EpisodeLoad, Option<u64>)> {
    let episode_id = args
        .get("episode_id")
        .and_then(json_i64)
        .ok_or_else(|| AppError::bad_request("play_episode requires episode_id"))?;
    if episode_id <= 0 || episode_id > i32::MAX as i64 {
        return Err(AppError::bad_request("play_episode requires episode_id"));
    }
    let episode_id = episode_id as i32;
    let is_youtube = args
        .get("is_youtube")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let meta = state
        .db_pool
        .get_episode_metadata(episode_id, user_id, false, is_youtube)
        .await?;
    let podcast_id = meta
        .get("podcastid")
        .and_then(json_i64)
        .ok_or_else(|| AppError::not_found("Episode not found"))? as i32;
    let stored = read_settings(&state.db_pool, user_id).await?;
    let base = stored
        .external_base_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::bad_request("Set an external base URL so Music Assistant can reach PinePods")
        })?;
    let rss_key = state
        .db_pool
        .get_or_create_podcast_rss_key(user_id, podcast_id)
        .await?;
    let downloaded = if is_youtube {
        state
            .db_pool
            .get_youtube_video_location(episode_id, user_id)
            .await
            .ok()
            .flatten()
    } else {
        state
            .db_pool
            .get_download_location(episode_id, user_id)
            .await
            .ok()
            .flatten()
    };
    // A saved file is served by PinePods, and so is local media (`local://`). Anything else is
    // the feed's own enclosure, which Music Assistant fetches itself.
    let enclosure = if is_youtube || downloaded.is_some() {
        None
    } else {
        meta.get("episodeurl")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let served_by_pinepods = downloaded.is_some()
        || enclosure
            .as_deref()
            .is_some_and(|uri| uri.to_ascii_lowercase().starts_with("local://"));
    let direct = enclosure;
    // Feed username/password is not applied to a direct URL; download the episode
    // first when the host requires those credentials.
    let media = match ma_playback(served_by_pinepods, direct) {
        Ok(MaPlayback::Direct(uri)) => uri,
        Ok(MaPlayback::Stream) => build_stream_url(base, episode_id, &rss_key, user_id, is_youtube),
        Err(msg) => {
            tracing::warn!("music assistant episode {episode_id} user {user_id}: {msg}");
            return Err(AppError::bad_request(msg));
        }
    };
    let explicit = args.get("position_sec").and_then(json_f64);
    let saved = meta.get("listenduration").and_then(json_f64).unwrap_or(0.0);
    let start_skip = if explicit.is_some() {
        0
    } else {
        state
            .db_pool
            .get_play_episode_details(user_id, podcast_id, is_youtube)
            .await
            .map(|(_, start, _, _)| start)
            .unwrap_or(0)
    };
    let position = resume_position(explicit, saved, start_skip);
    let playlist_id = args
        .get("playlist_id")
        .and_then(json_i64)
        .filter(|id| *id > 0 && *id <= i32::MAX as i64)
        .map(|id| id as i32);
    // Do not read or forward the local playback speed. A 1.5x web session
    // stays on this browser; the speaker starts at Music Assistant's own rate.
    let display = display_from_meta(&meta, base);
    let mut arm = None;
    // Remember the episode before play_media returns so a remote enclosure,
    // which never matches `/api/data/stream/`, still gets a history row. Arming
    // the resume seek here also drops the previous episode's duration.
    if let Some(live) = state.music_assistant.session_live(user_id).await {
        live.remember(
            queue_id,
            RememberedEpisode {
                user_id,
                episode_id,
                is_youtube,
                duration_sec: display.duration_sec.max(0) as f64,
                playlist_id,
                media_url: media.clone(),
            },
        )
        .await;
        live.begin_load(queue_id).await;
        arm = live.arm_resume(queue_id, position).await;
    }
    let load = match staged_start_commands(queue_id, &media, Some(&display), position) {
        Some(staged) if arm.is_some() => EpisodeLoad::AtOffset(staged),
        _ => EpisodeLoad::FromStart(play_media_commands(queue_id, &media, Some(&display))),
    };
    Ok((load, arm))
}

pub fn spawn_supervisor(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(e) = reconcile(&state).await {
                tracing::warn!("music assistant reconcile failed: {e}");
            }
            tokio::select! {
                _ = state.music_assistant.inner.reload.notified() => {}
                _ = tokio::time::sleep(Duration::from_secs(30)) => {}
            }
        }
    });
}

async fn reconcile(state: &AppState) -> AppResult<()> {
    let enabled = list_enabled(&state.db_pool).await?;
    let mut wanted: HashMap<i32, (String, StoredSettings)> = HashMap::new();
    for (user_id, stored) in enabled {
        let fp = fingerprint(&stored);
        wanted.insert(user_id, (fp, stored));
    }
    let mut stop = Vec::new();
    {
        let sessions = state.music_assistant.inner.sessions.read().await;
        for (user_id, slot) in sessions.iter() {
            match wanted.get(user_id) {
                Some((fp, _)) if fp == &slot.fingerprint => {}
                _ => stop.push(*user_id),
            }
        }
    }
    for user_id in stop {
        if let Some(slot) = state
            .music_assistant
            .inner
            .sessions
            .write()
            .await
            .remove(&user_id)
        {
            let _ = slot.cancel.send(true);
        }
    }
    for (user_id, (fp, stored)) in wanted {
        if state
            .music_assistant
            .inner
            .sessions
            .read()
            .await
            .contains_key(&user_id)
        {
            continue;
        }
        let token = match state
            .db_pool
            .decrypt_password(stored.token_encrypted.as_deref().unwrap_or(""))
            .await
        {
            Ok(t) if !t.is_empty() => t,
            Ok(_) => {
                tracing::warn!("music assistant token for user {user_id} decrypted empty");
                continue;
            }
            Err(e) => {
                tracing::warn!(
                    "music assistant token for user {user_id} could not be decrypted: {e}"
                );
                continue;
            }
        };
        let endpoints = match parse_server_url(stored.server_url.as_deref().unwrap_or("")) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("music assistant URL for user {user_id} is invalid: {e}");
                continue;
            }
        };
        let external = stored.external_base_url.clone().unwrap_or_default();
        let (tx, rx) = mpsc::unbounded_channel();
        let (cancel_tx, cancel_rx) = watch::channel(false);
        let live = Arc::new(SessionLive::new());
        state.music_assistant.inner.sessions.write().await.insert(
            user_id,
            SessionSlot {
                outbound: tx,
                live: live.clone(),
                fingerprint: fp,
                cancel: cancel_tx,
            },
        );
        let state = state.clone();
        tokio::spawn(async move {
            maintain(
                state, user_id, endpoints, external, token, rx, live, cancel_rx,
            )
            .await;
        });
    }
    Ok(())
}

fn fingerprint(stored: &StoredSettings) -> String {
    format!(
        "{}|{}|{}",
        stored.server_url.as_deref().unwrap_or(""),
        stored.external_base_url.as_deref().unwrap_or(""),
        stored.token_encrypted.as_deref().unwrap_or("")
    )
}

struct QueueTrack {
    stream: Option<StreamRef>,
    title: String,
    artwork: String,
    duration: f64,
    position: f64,
    /// Unix seconds the position was measured. 0 until a queue clock arrives.
    updated_at: i64,
    playing: bool,
    speed: f64,
    sample: Option<ProgressSample>,
    last_write: Option<Instant>,
    loaded_episode: Option<(i32, bool)>,
    /// Podcast outro, in seconds. Completion uses this and auto-complete.
    end_skip: i32,
    played_counted: bool,
    listen_marked_at: Option<Instant>,
    /// The episode finished and the advance ran. No more history writes for it.
    advanced: bool,
    /// Where Music Assistant will resume this paused queue. See [`paused_position`].
    paused_at: Option<f64>,
}

impl QueueTrack {
    fn clear_episode(&mut self) {
        self.paused_at = None;
        self.stream = None;
        self.title.clear();
        self.artwork.clear();
        self.duration = 0.0;
        self.position = 0.0;
        self.updated_at = 0;
        self.playing = false;
        self.loaded_episode = None;
        self.end_skip = 0;
        self.played_counted = false;
        self.listen_marked_at = None;
        self.advanced = false;
    }
}

/// How often the bridge re-anchors a playing queue's projected position,
/// writes it, and republishes the snapshot when Music Assistant is quiet.
const CLOCK_ANCHOR_EVERY_SECS: i64 = 10;

async fn maintain(
    state: AppState,
    user_id: i32,
    endpoints: MaEndpoints,
    external_base: String,
    token: String,
    mut outbound: mpsc::UnboundedReceiver<Outbound>,
    live: Arc<SessionLive>,
    mut cancel: watch::Receiver<bool>,
) {
    let mut virtuals: HashMap<String, u64> = HashMap::new();
    let mut tracks: HashMap<String, QueueTrack> = HashMap::new();
    // player_id → queue id that last had a PinePods stream. Survives a socket
    // reconnect so the next players/all refresh can put the episode back.
    let mut retained: HashMap<String, String> = HashMap::new();
    let mut backoff = 2u64;
    loop {
        if *cancel.borrow() {
            break;
        }
        match run_once(
            &state,
            user_id,
            &endpoints,
            &external_base,
            &token,
            &mut outbound,
            &live,
            &mut cancel,
            &mut virtuals,
            &mut tracks,
            &mut retained,
        )
        .await
        {
            Ok(()) => backoff = 2,
            Err(e) => {
                let redacted = redact(e, &token);
                tracing::warn!("music assistant user {user_id}: {redacted}");
                *live.last_error.write().await = Some(redacted);
                live.connected.store(false, Ordering::Relaxed);
            }
        }
        if *cancel.borrow() || outbound.is_closed() {
            break;
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(backoff)) => {}
            _ = cancel.changed() => {}
        }
        if *cancel.borrow() {
            break;
        }
        backoff = (backoff * 2).min(60);
    }
    live.connected.store(false, Ordering::Relaxed);
    clear_virtuals(&state, user_id, &mut virtuals).await;
}

async fn run_once(
    state: &AppState,
    user_id: i32,
    endpoints: &MaEndpoints,
    _external_base: &str,
    token: &str,
    outbound: &mut mpsc::UnboundedReceiver<Outbound>,
    live: &Arc<SessionLive>,
    cancel: &mut watch::Receiver<bool>,
    virtuals: &mut HashMap<String, u64>,
    tracks: &mut HashMap<String, QueueTrack>,
    retained: &mut HashMap<String, String>,
) -> Result<(), String> {
    let socket = tokio::time::timeout(
        Duration::from_secs(10),
        tokio_tungstenite::connect_async(&endpoints.ws),
    )
    .await
    .map_err(|_| "timed out connecting to Music Assistant".to_string())?
    .map_err(|e| redact(e.to_string(), token))?
    .0;
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (note_tx, mut note_rx) = mpsc::unbounded_channel();
    let token_for_drive = token.to_string();
    let mut driver =
        tokio::spawn(async move { drive_socket(socket, &token_for_drive, cmd_rx, note_tx).await });
    live.connected.store(true, Ordering::Relaxed);
    let mut clock = tokio::time::interval(Duration::from_secs(1));
    clock.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let result = loop {
        tokio::select! {
            _ = clock.tick() => {
                tick_tracks(state, user_id, live, tracks, retained).await;
            }
            res = &mut driver => {
                live.connected.store(false, Ordering::Relaxed);
                break match res {
                    Ok(r) => r,
                    Err(e) => Err(format!("connection task stopped: {e}")),
                };
            }
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    break Ok(());
                }
            }
            note = note_rx.recv() => {
                match note {
                    Some(note) => {
                        apply_note(state, user_id, live, virtuals, tracks, retained, note).await
                    }
                    None => break Err("Music Assistant connection ended".into()),
                }
            }
            cmd = outbound.recv() => {
                match cmd {
                    Some(cmd) => {
                        if cmd_tx.send(cmd).is_err() {
                            break Err("Music Assistant connection ended".into());
                        }
                    }
                    None => break Ok(()),
                }
            }
        }
    };
    live.connected.store(false, Ordering::Relaxed);
    driver.abort();
    result
}

async fn apply_note(
    state: &AppState,
    user_id: i32,
    live: &Arc<SessionLive>,
    virtuals: &mut HashMap<String, u64>,
    tracks: &mut HashMap<String, QueueTrack>,
    retained: &mut HashMap<String, String>,
    note: SessionNote,
) {
    match note {
        SessionNote::Server(info) => {
            *live.server_version.write().await =
                Some(info.server_version).filter(|s| !s.is_empty());
            *live.schema_version.write().await = info.schema_version;
            *live.warning.write().await = info.assessment.warning;
        }
        SessionNote::Players(players) => {
            *live.players.write().await = players.clone();
            if let Err(e) =
                sync_virtuals(state, user_id, &players, virtuals, tracks, retained).await
            {
                tracing::debug!("music assistant device sync: {e}");
            }
        }
        SessionNote::Event {
            event,
            object_id,
            data,
        } => {
            if event == "player_updated" || event == "player_added" {
                let player_id = data
                    .get("player_id")
                    .and_then(|s| s.as_str())
                    .map(|s| s.to_string());
                let mut players = live.players.write().await;
                let previous = player_id
                    .as_deref()
                    .and_then(|id| players.iter().find(|p| p.player_id == id).cloned());
                if let Some(player) = merge_player_update(previous.as_ref(), &data) {
                    if let Some(slot) = players.iter_mut().find(|p| p.player_id == player.player_id)
                    {
                        *slot = player;
                    } else if event == "player_added" {
                        players.push(player);
                    }
                    let snapshot_players = players.clone();
                    drop(players);
                    let _ = sync_virtuals(
                        state,
                        user_id,
                        &snapshot_players,
                        virtuals,
                        tracks,
                        retained,
                    )
                    .await;
                }
            } else if event == "player_removed" {
                if let Some(id) = object_id.or_else(|| {
                    data.get("player_id")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string())
                }) {
                    live.players.write().await.retain(|p| p.player_id != id);
                    retained.remove(&id);
                    let players = live.players.read().await.clone();
                    let _ =
                        sync_virtuals(state, user_id, &players, virtuals, tracks, retained).await;
                }
            } else if event.starts_with("queue_") {
                let update = progress_from_event(object_id.as_deref(), &data, unix_secs());
                apply_progress(state, user_id, live, tracks, retained, update).await;
            }
        }
    }
}

async fn apply_progress(
    state: &AppState,
    user_id: i32,
    live: &Arc<SessionLive>,
    tracks: &mut HashMap<String, QueueTrack>,
    retained: &mut HashMap<String, String>,
    mut update: ProgressUpdate,
) {
    let Some(queue_id) = update.queue_id.clone() else {
        return;
    };
    // The queue held nothing when `play_episode` loaded it and still says so.
    if stale_empty_queue(update.media_cleared, live.loading_for(&queue_id).await) {
        return;
    }
    let players = live.players.read().await.clone();
    let remembered = live.remembered(&queue_id).await;
    if remembered
        .as_ref()
        .is_some_and(|row| item_is_sent_media(update.item_strings.as_deref(), &row.media_url))
    {
        live.end_load(&queue_id).await;
    }
    let remembered_playlist = remembered
        .as_ref()
        .map(|row| ((row.episode_id, row.is_youtube), row.playlist_id));
    let bare_tick = update.stream.is_none()
        && update.duration_sec.is_none()
        && update.title.is_none()
        && update.playing.is_none()
        && !update.media_cleared
        && tracks.get(&queue_id).is_some_and(|t| t.stream.is_some());
    let history = history_stream(
        remembered.as_ref(),
        update.stream.as_ref(),
        update.media_cleared,
        user_id,
        bare_tick,
        update.item_strings.as_deref(),
    );
    match &history {
        HistoryEpisode::Clear => {
            live.forget(&queue_id).await;
        }
        HistoryEpisode::OtherItem => {
            let had_episode = tracks
                .get_mut(&queue_id)
                .filter(|t| t.stream.is_some())
                .map(|t| t.clear_episode())
                .is_some();
            if had_episode {
                retained.retain(|_, queue| queue != &queue_id);
                publish_track_snapshot(state, user_id, live, tracks, retained, &queue_id, true)
                    .await;
            }
            return;
        }
        HistoryEpisode::Foreign => {}
        HistoryEpisode::Write(stream) => {
            if update.stream.is_none() {
                update.stream = Some(stream.clone());
            }
            if let Some(next) = next_remembered(remembered, update.stream.as_ref(), false, user_id)
            {
                live.remember(&queue_id, next).await;
            }
        }
        HistoryEpisode::None => {}
    }
    let donor = if update.stream.is_none() && !update.media_cleared {
        donor_track(tracks, &players, &queue_id, retained)
    } else {
        None
    };
    let track = tracks
        .entry(queue_id.clone())
        .or_insert_with(|| QueueTrack {
            stream: None,
            title: String::new(),
            artwork: String::new(),
            duration: 0.0,
            position: 0.0,
            updated_at: 0,
            playing: false,
            speed: 1.0,
            sample: None,
            last_write: None,
            loaded_episode: None,
            end_skip: 0,
            played_counted: false,
            listen_marked_at: None,
            advanced: false,
            paused_at: None,
        });
    let (playing, clear_stream) = next_playback(
        track.playing,
        track.stream.is_some() || donor.is_some() || update.stream.is_some(),
        update.playing,
        update.reported_idle,
        update.media_cleared,
    );
    let episode_changed = update.stream.as_ref().map(|s| (s.episode_id, s.is_youtube))
        != track.stream.as_ref().map(|s| (s.episode_id, s.is_youtube))
        && update.stream.is_some();
    let finished = track
        .stream
        .clone()
        .filter(|s| !track.advanced && !episode_changed && s.user_id.is_none_or(|id| id == user_id))
        .filter(|_| {
            let projected = projected_position(
                track.position,
                track.updated_at,
                track.playing,
                track.speed,
                unix_secs(),
            );
            let duration = update
                .duration_sec
                .filter(|d| *d > 0.0)
                .unwrap_or(track.duration);
            episode_finished(
                update.position_sec,
                projected,
                track.playing,
                playing,
                duration,
                track.end_skip,
            )
        });
    if let Some(stream) = finished {
        track.advanced = true;
        let playlist_id = remembered_playlist
            .filter(|(id, _)| *id == (stream.episode_id, stream.is_youtube))
            .and_then(|(_, pid)| pid);
        tokio::spawn(advance_queue(
            state.clone(),
            user_id,
            queue_id.clone(),
            stream,
            playlist_id,
            playing,
        ));
    }
    if clear_stream {
        track.clear_episode();
        retained.retain(|_, queue| queue != &queue_id);
    } else {
        if episode_changed {
            // The previous episode's length must not finish this one.
            track.duration = 0.0;
            track.advanced = false;
        }
        if let Some(stream) = update.stream.clone() {
            track.stream = Some(stream);
        } else if track.stream.is_none() {
            if let Some(donor) = donor {
                track.stream = Some(donor.stream);
                if track.title.is_empty() {
                    track.title = donor.title;
                }
                if track.artwork.is_empty() {
                    track.artwork = donor.artwork;
                }
                if track.duration <= 0.0 {
                    track.duration = donor.duration;
                }
                if donor.speed > 0.0 {
                    track.speed = donor.speed;
                }
            }
        }
        track.playing = playing;
        if let Some(p) = update.position_sec {
            track.position = p.max(0.0);
            if let Some(at) = update.position_updated_at {
                track.updated_at = at;
            }
        }
        let held = track.paused_at.filter(|_| !episode_changed);
        track.paused_at = paused_position(held, update.resume_pos, playing);
        if let Some(p) = track.paused_at {
            track.position = p;
        }
        if let Some(d) = update.duration_sec {
            if d > 0.0 {
                track.duration = d;
                // Only a queue-reported duration may release the resume seek.
                live.note_queue_duration(&queue_id, d).await;
            }
        }
        if let Some(title) = update.title.clone() {
            if !title.is_empty() && track.title.is_empty() {
                track.title = title;
            }
        }
        if let Some(speed) = update.speed {
            if speed > 0.0 {
                track.speed = speed;
            }
        }
    }
    let Some(stream) = track.stream.clone() else {
        publish_track_snapshot(
            state,
            user_id,
            live,
            tracks,
            retained,
            &queue_id,
            clear_stream,
        )
        .await;
        return;
    };
    if stream.user_id.is_some_and(|id| id != user_id) {
        return;
    }
    if episode_changed {
        track.played_counted = false;
        track.listen_marked_at = None;
        track.end_skip = 0;
    }
    if episode_changed || track.loaded_episode != Some((stream.episode_id, stream.is_youtube)) {
        if let Ok(meta) = state
            .db_pool
            .get_episode_metadata(stream.episode_id, user_id, false, stream.is_youtube)
            .await
        {
            if let Some(title) = meta.get("episodetitle").and_then(|v| v.as_str()) {
                track.title = title.to_string();
            }
            if let Some(art) = meta.get("episodeartwork").and_then(|v| v.as_str()) {
                track.artwork = art.to_string();
            }
            if let Some(dur) = meta.get("episodeduration").and_then(json_f64) {
                if dur > 0.0 && track.duration <= 0.0 {
                    track.duration = dur;
                }
            }
            if let Some(podcast_id) = meta.get("podcastid").and_then(json_i64) {
                if let Ok((_, _, end_skip, _)) = state
                    .db_pool
                    .get_play_episode_details(user_id, podcast_id as i32, stream.is_youtube)
                    .await
                {
                    track.end_skip = end_skip.max(0);
                }
            }
            track.loaded_episode = Some((stream.episode_id, stream.is_youtube));
        }
    }
    let resume_pending = live.pending_resume(&queue_id).await.is_some();
    write_track_progress(state, user_id, track, &stream, resume_pending).await;
    publish_track_snapshot(state, user_id, live, tracks, retained, &queue_id, false).await;
}

/// History, played count, and listen time for one queue's loaded episode.
/// `resume_pending` holds the history write, not the counters.
async fn write_track_progress(
    state: &AppState,
    user_id: i32,
    track: &mut QueueTrack,
    stream: &StreamRef,
    resume_pending: bool,
) {
    let sample = ProgressSample {
        episode_id: stream.episode_id,
        position_sec: track.position,
        playing: track.playing,
    };
    let since = track
        .last_write
        .map(|t| t.elapsed())
        .unwrap_or(PROGRESS_WRITE_EVERY);
    if track.advanced
        && replayed_after_finish(
            track.playing,
            track.position,
            track.duration,
            track.end_skip,
        )
    {
        track.advanced = false;
    }
    if !track.advanced
        && should_write_progress(track.sample.as_ref(), &sample, since, resume_pending)
    {
        write_listen_progress(
            state,
            user_id,
            stream.episode_id,
            stream.is_youtube,
            track.position,
            track.duration,
            track.end_skip,
        )
        .await;
        track.last_write = Some(Instant::now());
        track.sample = Some(sample);
    }
    if track.playing {
        if !track.played_counted && state.db_pool.increment_played(user_id).await.is_ok() {
            track.played_counted = true;
        }
        match track.listen_marked_at {
            None => track.listen_marked_at = Some(Instant::now()),
            Some(started) if started.elapsed() >= Duration::from_secs(60) => {
                if state.db_pool.add_listen_seconds(user_id, 60).await.is_ok() {
                    track.listen_marked_at = Some(Instant::now());
                }
            }
            Some(_) => {}
        }
    } else {
        track.listen_marked_at = None;
    }
}

/// Once a second: Music Assistant 2.10.4 is silent while a queue plays, so
/// the bridge projects the position itself. It fires the outro advance on
/// time, and every few seconds re-anchors the position, writes history, and
/// republishes the snapshot so clients do not freeze after 30 seconds.
async fn tick_tracks(
    state: &AppState,
    user_id: i32,
    live: &Arc<SessionLive>,
    tracks: &mut HashMap<String, QueueTrack>,
    retained: &mut HashMap<String, String>,
) {
    let now = unix_secs();
    for queue_id in live.take_due_sleep(now).await {
        let Some(track) = tracks.get(&queue_id) else {
            continue;
        };
        // A paused speaker has nothing to stop; the timer just ends.
        if track.playing {
            if let Some(stream) = track.stream.clone() {
                let position =
                    projected_position(track.position, track.updated_at, true, track.speed, now);
                write_listen_progress(
                    state,
                    user_id,
                    stream.episode_id,
                    stream.is_youtube,
                    position,
                    track.duration,
                    track.end_skip,
                )
                .await;
            }
            let pause = MaCommand {
                command: "player_queues/pause".into(),
                args: json!({ "queue_id": queue_id }),
            };
            let state = state.clone();
            // The session loop has to keep forwarding while this is sent.
            tokio::spawn(async move {
                if let Err(e) = state
                    .music_assistant
                    .send_commands(user_id, vec![pause])
                    .await
                {
                    tracing::warn!("music assistant sleep timer pause: {e}");
                }
            });
        }
        publish_track_snapshot(state, user_id, live, tracks, retained, &queue_id, false).await;
    }
    let playing: Vec<String> = tracks
        .iter()
        .filter(|(_, t)| t.playing && t.stream.is_some() && t.updated_at > 0)
        .map(|(queue_id, _)| queue_id.clone())
        .collect();
    for queue_id in playing {
        let Some(track) = tracks.get_mut(&queue_id) else {
            continue;
        };
        let Some(stream) = track.stream.clone() else {
            continue;
        };
        if stream.user_id.is_some_and(|id| id != user_id) {
            continue;
        }
        let projected =
            projected_position(track.position, track.updated_at, true, track.speed, now);
        if !track.advanced && advance_due(projected, true, true, track.duration, track.end_skip) {
            track.advanced = true;
            let playlist_id = live
                .remembered(&queue_id)
                .await
                .filter(|row| {
                    (row.episode_id, row.is_youtube) == (stream.episode_id, stream.is_youtube)
                })
                .and_then(|row| row.playlist_id);
            tokio::spawn(advance_queue(
                state.clone(),
                user_id,
                queue_id.clone(),
                stream.clone(),
                playlist_id,
                true,
            ));
        }
        if now - track.updated_at < CLOCK_ANCHOR_EVERY_SECS {
            continue;
        }
        track.position = projected;
        track.updated_at = now;
        let resume_pending = live.pending_resume(&queue_id).await.is_some();
        write_track_progress(state, user_id, track, &stream, resume_pending).await;
        publish_track_snapshot(state, user_id, live, tracks, retained, &queue_id, false).await;
    }
}

async fn write_listen_progress(
    state: &AppState,
    user_id: i32,
    episode_id: i32,
    is_youtube: bool,
    position: f64,
    duration_hint: f64,
    end_skip: i32,
) {
    // The bridge reads the speaker's position now, so it is the newest observation and may
    // move the stored position either way (a seek back on the speaker is kept).
    let write = crate::database::ProgressWrite::now(
        episode_id,
        is_youtube,
        position.max(0.0) as i32,
        crate::database::WriteRule::Newest,
    );
    match state
        .db_pool
        .write_progress(user_id, write, crate::services::sync::SyncOrigin::Pinepods)
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            tracing::debug!("music assistant progress write skipped: episode {episode_id} is not the user's");
            return;
        }
        Err(e) => {
            tracing::debug!("music assistant progress write failed: {e}");
            return;
        }
    }
    let auto = state
        .db_pool
        .get_user_auto_complete_seconds(user_id)
        .await
        .unwrap_or(0);
    let duration = if duration_hint > 0.0 {
        duration_hint
    } else if is_youtube {
        state
            .db_pool
            .get_youtube_episode_duration(episode_id)
            .await
            .unwrap_or(0) as f64
    } else {
        state
            .db_pool
            .get_episode_duration(episode_id)
            .await
            .unwrap_or(0) as f64
    };
    if should_mark_complete(position, duration, auto, end_skip) {
        let _ = state
            .db_pool
            .mark_episode_completed(
                episode_id,
                user_id,
                is_youtube,
                crate::services::sync::SyncOrigin::Pinepods,
            )
            .await;
    }
}

async fn publish_track_snapshot(
    state: &AppState,
    user_id: i32,
    live: &Arc<SessionLive>,
    tracks: &HashMap<String, QueueTrack>,
    retained: &mut HashMap<String, String>,
    queue_id: &str,
    cleared: bool,
) {
    let players = live.players.read().await.clone();
    let loaded = loaded_queue_ids(tracks);
    let mut wrote = false;
    for player in players.iter().filter(|p| player_related(p, queue_id)) {
        let key = snapshot_queue_id(
            player,
            &players,
            &loaded,
            retained.get(&player.player_id).map(String::as_str),
        );
        let chosen = tracks.get(&key);
        let episode = chosen
            .and_then(|t| t.stream.as_ref())
            .map(|s| s.episode_id)
            .unwrap_or(0);
        // An empty tick for a new coordinator queue must not replace the
        // episode the clients are showing. A real stop (`cleared`) does.
        if episode == 0 && !cleared && retained.contains_key(&player.player_id) {
            continue;
        }
        let mut snap = snapshot_for(user_id, player, chosen);
        snap.sleep_timer = live.sleep_timer(&key).await;
        if state
            .redis_client
            .upsert_now_playing(user_id, snap)
            .await
            .is_err()
        {
            continue;
        }
        wrote = true;
        if episode != 0 {
            retained.insert(player.player_id.clone(), key);
        } else if cleared {
            retained.remove(&player.player_id);
        }
    }
    if wrote {
        notify_devices_changed(state, user_id, "").await;
    }
}

fn loaded_queue_ids(tracks: &HashMap<String, QueueTrack>) -> HashSet<String> {
    tracks
        .iter()
        .filter(|(_, track)| track.stream.is_some())
        .map(|(id, _)| id.clone())
        .collect()
}

fn player_related(player: &MaPlayer, queue_id: &str) -> bool {
    player.player_id == queue_id
        || player.active_source.as_deref() == Some(queue_id)
        || player.synced_to.as_deref() == Some(queue_id)
        || player.group_members.iter().any(|member| member == queue_id)
}

struct DonorTrack {
    stream: StreamRef,
    title: String,
    artwork: String,
    duration: f64,
    speed: f64,
}

/// Stream already loaded on a player this queue belongs to. Used when a Sonos
/// coordinator change opens a new queue id that has elapsed time but no URL.
fn donor_track(
    tracks: &HashMap<String, QueueTrack>,
    players: &[MaPlayer],
    queue_id: &str,
    retained: &HashMap<String, String>,
) -> Option<DonorTrack> {
    let loaded = loaded_queue_ids(tracks);
    for player in players.iter().filter(|p| player_related(p, queue_id)) {
        let key = snapshot_queue_id(
            player,
            players,
            &loaded,
            retained.get(&player.player_id).map(String::as_str),
        );
        if key == queue_id {
            continue;
        }
        if let Some(track) = tracks.get(&key).filter(|t| t.stream.is_some()) {
            return Some(DonorTrack {
                stream: track.stream.clone().unwrap(),
                title: track.title.clone(),
                artwork: track.artwork.clone(),
                duration: track.duration,
                speed: track.speed,
            });
        }
    }
    None
}

async fn sync_virtuals(
    state: &AppState,
    user_id: i32,
    players: &[MaPlayer],
    virtuals: &mut HashMap<String, u64>,
    tracks: &HashMap<String, QueueTrack>,
    retained: &mut HashMap<String, String>,
) -> AppResult<()> {
    let loaded = loaded_queue_ids(tracks);
    let live = state.music_assistant.session_live(user_id).await;
    let mut keep = HashSet::new();
    for player in players {
        let queue_key = snapshot_queue_id(
            player,
            players,
            &loaded,
            retained.get(&player.player_id).map(String::as_str),
        );
        let track = tracks.get(&queue_key);
        let has_episode = track.and_then(|t| t.stream.as_ref()).is_some();
        // `retained` means this player already published an episode. An
        // availability blip or a coordinator queue that does not yet hold the
        // stream must not unregister it.
        let remembered = retained.contains_key(&player.player_id);
        if !keep_virtual_device(player.available, has_episode || remembered) {
            continue;
        }
        let device_id = device_id_for(user_id, &player.player_id);
        keep.insert(device_id.clone());
        if !virtuals.contains_key(&device_id) {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let conn = state
                .now_playing_manager
                .register_virtual(user_id, device_id.clone(), tx)
                .await;
            virtuals.insert(device_id.clone(), conn);
            let state = state.clone();
            let player_id = player.player_id.clone();
            tokio::spawn(async move {
                while let Some(msg) = rx.recv().await {
                    let text = match msg {
                        RelayMessage::Text(t) => t.to_string(),
                        RelayMessage::Close(_) => break,
                        _ => continue,
                    };
                    let Some((action, args)) = parse_relay_command(&text) else {
                        continue;
                    };
                    if let Err(e) = dispatch(&state, user_id, &player_id, &action, &args).await {
                        tracing::warn!("music assistant {player_id} {action} failed: {e}");
                    }
                }
            });
        }
        // Don't replace a loaded episode with an empty snapshot. The retained
        // queue is still the one the speaker is playing.
        if !has_episode && retained.contains_key(&player.player_id) {
            continue;
        }
        let mut snap = snapshot_for(user_id, player, track);
        if let Some(live) = &live {
            snap.sleep_timer = live.sleep_timer(&queue_key).await;
        }
        state.redis_client.upsert_now_playing(user_id, snap).await?;
        if has_episode {
            retained.insert(player.player_id.clone(), queue_key);
        }
    }
    let stale: Vec<String> = virtuals
        .keys()
        .filter(|id| !keep.contains(*id))
        .cloned()
        .collect();
    for device_id in stale {
        if let Some(conn) = virtuals.remove(&device_id) {
            state
                .now_playing_manager
                .unregister_virtual(user_id, &device_id, conn)
                .await;
        }
        state
            .redis_client
            .remove_now_playing(user_id, &device_id)
            .await?;
    }
    notify_devices_changed(state, user_id, "").await;
    Ok(())
}

async fn clear_virtuals(state: &AppState, user_id: i32, virtuals: &mut HashMap<String, u64>) {
    for (device_id, conn) in virtuals.drain() {
        state
            .now_playing_manager
            .unregister_virtual(user_id, &device_id, conn)
            .await;
        let _ = state
            .redis_client
            .remove_now_playing(user_id, &device_id)
            .await;
    }
    notify_devices_changed(state, user_id, "").await;
}

fn snapshot_for(user_id: i32, player: &MaPlayer, track: Option<&QueueTrack>) -> NowPlayingSnapshot {
    let (episode_id, is_youtube, title, artwork, position, duration, playing, speed) = match track {
        Some(t) => {
            let (episode_id, is_youtube) = t
                .stream
                .as_ref()
                .map(|s| (s.episode_id, s.is_youtube))
                .unwrap_or((0, false));
            let title = if t.title.is_empty() {
                player.current_title.clone().unwrap_or_default()
            } else {
                t.title.clone()
            };
            (
                episode_id,
                is_youtube,
                title,
                t.artwork.clone(),
                snapshot_position(t.position, t.updated_at, t.playing, t.speed, t.duration, unix_secs()),
                if t.duration > 0.0 { t.duration } else { 0.0 },
                t.playing,
                t.speed,
            )
        }
        None => (
            0,
            false,
            player.current_title.clone().unwrap_or_default(),
            String::new(),
            player.elapsed_time.unwrap_or(0.0),
            0.0,
            player.state == "playing" || player.state == "buffering",
            1.0,
        ),
    };
    NowPlayingSnapshot {
        device_id: device_id_for(user_id, &player.player_id),
        device_name: player.name.clone(),
        device_type: DEVICE_TYPE.to_string(),
        episode_id,
        is_youtube,
        title,
        artwork_url: artwork,
        position_sec: position,
        duration_sec: duration,
        playing,
        speed,
        updated_at: track.map(|t| t.updated_at).unwrap_or(0),
        capabilities: MA_CAPABILITIES.iter().map(|s| (*s).to_string()).collect(),
        volume_level: player.volume_level,
        volume_muted: Some(player.volume_muted),
        ma_player_id: Some(player.player_id.clone()),
        group_members: Some(player.group_members.clone()),
        sleep_timer: None,
    }
}

fn unix_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn json_i64(v: &Value) -> Option<i64> {
    v.as_i64()
        .or_else(|| v.as_u64().map(|n| n as i64))
        .or_else(|| v.as_f64().map(|n| n as i64))
}

// ============================ Tests ======================================

#[cfg(test)]
mod tests {
    use super::*;
    use futures::{SinkExt, StreamExt};
    use std::collections::HashSet;
    use tokio::io::DuplexStream;
    use tokio_tungstenite::tungstenite::protocol::Role;
    use tokio_tungstenite::WebSocketStream;

    #[test]
    fn external_base_rejects_loopback_and_accepts_lan() {
        assert!(normalize_external_base("http://127.0.0.1:8040").is_err());
        assert!(normalize_external_base("http://localhost:8040").is_err());
        assert!(normalize_external_base("http://[::1]:8040").is_err());
        assert!(normalize_external_base("http://169.254.169.254/").is_err());
        assert_eq!(
            normalize_external_base("http://192.168.1.20:8040/").unwrap(),
            "http://192.168.1.20:8040"
        );
        assert_eq!(
            normalize_external_base("https://pods.example.com").unwrap(),
            "https://pods.example.com"
        );
        assert!(normalize_external_base("http://192.168.1.20:8040/pinepods").is_err());
    }

    #[test]
    fn server_url_normalizes_ws_and_blocks_metadata() {
        let ep = parse_server_url("http://192.168.1.5:8095").unwrap();
        assert_eq!(ep.ws, "ws://192.168.1.5:8095/ws");
        assert_eq!(ep.api, "http://192.168.1.5:8095/api");
        assert_eq!(ep.origin, "http://192.168.1.5:8095");
        let via_ws = parse_server_url("ws://192.168.1.5:8095/ws").unwrap();
        assert_eq!(via_ws.ws, ep.ws);
        assert!(parse_server_url("http://127.0.0.1:8095").is_ok());
        assert!(parse_server_url("http://169.254.169.254").is_err());
        assert!(parse_server_url("file:///etc/passwd").is_err());
        let def = parse_server_url("http://ma.home").unwrap();
        assert_eq!(def.ws, "ws://ma.home:8095/ws");
    }

    #[test]
    fn undownloaded_episode_uses_the_enclosure_and_never_a_missing_stream() {
        assert_eq!(
            ma_playback(false, Some("https://cdn.example/ep.mp3".into())).unwrap(),
            MaPlayback::Direct("https://cdn.example/ep.mp3".into())
        );
        assert_eq!(ma_playback(true, None).unwrap(), MaPlayback::Stream);
        assert!(ma_playback(false, None).is_err());
        assert!(ma_playback(false, Some("ipfs://bafy".into())).is_err());
        assert!(ma_playback(false, Some("magnet:?xt=urn:btih:abc".into())).is_err());
    }

    #[test]
    fn stream_url_uses_scoped_key_and_youtube_flag() {
        let url = build_stream_url("http://192.168.1.20:8040", 42, "rss key", 7, false);
        assert_eq!(
            url,
            "http://192.168.1.20:8040/api/data/stream/42?api_key=rss%20key&user_id=7"
        );
        let yt = build_stream_url("http://192.168.1.20:8040/", 3, "k", 1, true);
        assert!(yt.ends_with("&type=youtube"));
        let parsed = parse_stream_url(&yt).unwrap();
        assert_eq!(parsed.episode_id, 3);
        assert_eq!(parsed.user_id, Some(1));
        assert!(parsed.is_youtube);
    }

    #[test]
    fn device_id_roundtrips_player_ids_that_contain_colons() {
        let id = device_id_for(4, "b8:27:eb:8a:b8:8e");
        assert_eq!(id, "ma:4:b8:27:eb:8a:b8:8e");
        assert_eq!(
            player_id_from_device(4, &id).as_deref(),
            Some("b8:27:eb:8a:b8:8e")
        );
        assert!(player_id_from_device(5, &id).is_none());
        assert_eq!(canonical_player_id(4, &id), "b8:27:eb:8a:b8:8e");
    }

    #[test]
    fn version_check_accepts_2_10_and_refuses_ancient() {
        let ok = assess_version("2.10.3", Some(80));
        assert!(ok.compatible);
        assert!(ok.warning.is_none());
        assert!(!assess_version("1.9.0", None).compatible);
        assert!(!assess_version("2.5.1", Some(10)).compatible);
        let older = assess_version("2.8.0", None);
        assert!(older.compatible && older.warning.is_some());
        let future = assess_version("3.0.0", Some(200));
        assert!(future.compatible && future.warning.is_some());
        let unparsed = assess_version("not-a-version", None);
        assert!(unparsed.compatible && unparsed.warning.is_some());
        let old_schema = assess_version("2.10.0", Some(5));
        assert!(old_schema.compatible && old_schema.warning.is_some());
    }

    #[test]
    fn translate_transport_volume_and_grouping() {
        let pause = translate_action("kitchen", "kitchen", "pause", &json!({})).unwrap();
        assert_eq!(pause[0].command, "players/cmd/pause");
        assert_eq!(pause[0].args["player_id"], "kitchen");

        let seek =
            translate_action("kitchen", "q1", "seek", &json!({"position_sec": 12.5})).unwrap();
        assert_eq!(seek[0].command, "player_queues/seek");
        assert_eq!(seek[0].args["queue_id"], "q1");
        assert_eq!(seek[0].args["position"], json!(12));

        let back = translate_action("kitchen", "q1", "skip_back", &json!({})).unwrap();
        assert_eq!(back[0].args["seconds"], json!(-15));
        let fwd = translate_action("kitchen", "q1", "skip_forward", &json!({})).unwrap();
        assert_eq!(fwd[0].args["seconds"], json!(15));

        let vol =
            translate_action("kitchen", "q1", "volume_set", &json!({"volume_level": 140})).unwrap();
        assert_eq!(vol[0].command, "players/cmd/volume_set");
        assert_eq!(vol[0].args["volume_level"], 100.0);

        let mute =
            translate_action("kitchen", "q1", "volume_mute", &json!({"muted": true})).unwrap();
        assert_eq!(mute[0].args["muted"], true);

        let group =
            translate_action("bed", "bed", "group", &json!({"target_player": "kitchen"})).unwrap();
        assert_eq!(group[0].command, "players/cmd/group");
        assert_eq!(group[0].args["target_player"], "kitchen");

        let many = translate_action(
            "kitchen",
            "kitchen",
            "group_many",
            &json!({"player_ids": ["bed", "patio"]}),
        )
        .unwrap();
        assert_eq!(many[0].command, "players/cmd/group_many");
        assert_eq!(many[0].args["target_player"], "kitchen");
        assert_eq!(many[0].args["player_ids"][1], "patio");

        let members = translate_action(
            "grp",
            "grp",
            "set_members",
            &json!({"player_ids_to_add": ["a"], "player_ids_to_remove": []}),
        )
        .unwrap();
        assert_eq!(members[0].command, "players/cmd/set_members");

        let ignored =
            translate_action("kitchen", "q1", "set_speed", &json!({"speed": 1.5})).unwrap();
        assert!(ignored.is_empty());
        let missing = translate_action("kitchen", "q1", "set_speed", &json!({})).unwrap();
        assert!(missing.is_empty());
        assert!(translate_action("kitchen", "q1", "nope", &json!({})).is_err());
    }

    #[test]
    fn capabilities_do_not_advertise_speed() {
        assert!(!MA_CAPABILITIES.contains(&"set_speed"));
        assert!(MA_CAPABILITIES
            .iter()
            .all(|name| !name.to_ascii_lowercase().contains("speed")));
        assert!(MA_CAPABILITIES.contains(&"play"));
        assert!(MA_CAPABILITIES.contains(&"seek"));
        assert!(MA_CAPABILITIES.contains(&"play_episode"));
    }

    #[test]
    fn play_media_does_not_send_speed() {
        let url = "http://h/api/data/stream/1?api_key=k&user_id=2";
        let cmds = play_media_commands("q", url, None);
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].command, "player_queues/play_media");
        assert_eq!(cmds[0].args["media"], url);
        assert_eq!(cmds[0].args["option"], "replace");
        assert!(cmds[0].args.get("speed").is_none());
        assert!(cmds
            .iter()
            .all(|cmd| cmd.command != "player_queues/set_playback_speed"));
        let plain = play_media_commands("q", "http://h/a", None);
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].args["media"], "http://h/a");
        let seek = resume_seek_command("q", 30.5, 3600.0).unwrap();
        assert_eq!(seek.command, "player_queues/seek");
        assert_eq!(seek.args["position"], json!(30));
        assert!(resume_seek_command("q", 30.0, 0.0).is_none());
        assert!(resume_seek_command("q", 0.4, 3600.0).is_none());
    }

    #[test]
    fn play_media_sends_a_track_dict_when_artwork_is_fetchable() {
        let url = "https://pods.example/api/data/stream/9?api_key=k&user_id=2";
        let with_art = MaTrackDisplay {
            title: "Episode 12".into(),
            podcast_name: "No Agenda".into(),
            duration_sec: 3600,
            artwork_url: Some("https://cdn.example/ep.jpg".into()),
        };
        let cmds = play_media_commands("kitchen", url, Some(&with_art));
        assert_eq!(cmds.len(), 1);
        let media = &cmds[0].args["media"];
        assert_eq!(media["media_type"], "track");
        assert_eq!(media["item_id"], url);
        assert_eq!(media["provider"], "builtin");
        assert_eq!(media["name"], "Episode 12");
        assert_eq!(media["duration"], 3600);
        assert_eq!(media["artists"][0]["media_type"], "artist");
        assert_eq!(media["artists"][0]["name"], "No Agenda");
        assert_eq!(media["artists"][0]["item_id"], "No Agenda");
        assert_eq!(media["artists"][0]["provider"], "builtin");
        assert_eq!(media["artists"][0]["available"], true);
        assert_eq!(media["provider_mappings"][0]["item_id"], url);
        assert_eq!(media["provider_mappings"][0]["provider_domain"], "builtin");
        assert_eq!(
            media["provider_mappings"][0]["provider_instance"],
            "builtin"
        );
        assert_eq!(media["provider_mappings"][0]["available"], true);
        assert_eq!(media["metadata"]["images"][0]["type"], "thumb");
        assert_eq!(
            media["metadata"]["images"][0]["path"],
            "https://cdn.example/ep.jpg"
        );
        assert_eq!(media["metadata"]["images"][0]["provider"], "builtin");
        assert_eq!(media["metadata"]["images"][0]["remotely_accessible"], true);

        let no_art = MaTrackDisplay {
            title: "Episode 12".into(),
            podcast_name: "No Agenda".into(),
            duration_sec: 3600,
            artwork_url: None,
        };
        let plain = play_media_commands("kitchen", url, Some(&no_art));
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].args["media"], url);
        let seek = resume_seek_command("kitchen", 12.5, 3600.0).unwrap();
        assert_eq!(seek.args["queue_id"], "kitchen");
        assert_eq!(seek.args["position"], json!(12));
        assert!(resume_seek_command("kitchen", 12.0, 0.0).is_none());
    }

    #[test]
    fn artwork_uses_the_episode_image_or_the_podcast_image_on_the_external_base() {
        let meta = json!({
            "episodetitle": "Live",
            "podcastname": "Show",
            "episodeduration": 90,
            "episodeartwork": "/api/proxy/image?url=https%3A%2F%2Fcdn.example%2Fep.jpg",
            "artworkurl": "https://cdn.example/show.jpg",
        });
        let display = display_from_meta(&meta, "https://pods.example/");
        assert_eq!(display.title, "Live");
        assert_eq!(display.podcast_name, "Show");
        assert_eq!(display.duration_sec, 90);
        assert_eq!(
            display.artwork_url.as_deref(),
            Some("https://pods.example/api/proxy/image?url=https%3A%2F%2Fcdn.example%2Fep.jpg")
        );

        let fallback = json!({
            "episodetitle": "Live",
            "podcastname": "Show",
            "episodeartwork": "",
            "artworkurl": "https://cdn.example/show.jpg",
        });
        assert_eq!(
            display_from_meta(&fallback, "https://pods.example")
                .artwork_url
                .as_deref(),
            Some("https://cdn.example/show.jpg")
        );

        let none = json!({
            "episodetitle": "Live",
            "episodeartwork": "javascript:alert(1)",
            "artworkurl": "",
        });
        assert!(display_from_meta(&none, "https://pods.example")
            .artwork_url
            .is_none());
        assert!(artwork_url_for_ma("https://cdn.example/a.jpg", "https://pods.example").is_some());
        assert_eq!(
            artwork_url_for_ma(
                "/api/proxy/image?url=https://cdn.example/ep.jpg",
                "https://pods.example"
            )
            .as_deref(),
            Some("https://pods.example/api/proxy/image?url=https://cdn.example/ep.jpg")
        );
    }

    #[test]
    fn parses_results_errors_and_events() {
        match parse_inbound(r#"{"message_id":"7","result":{"authenticated":true}}"#) {
            Inbound::Result { message_id, result } => {
                assert_eq!(message_id, "7");
                assert_eq!(result["authenticated"], true);
            }
            other => panic!("expected result, got {other:?}"),
        }
        match parse_inbound(r#"{"message_id":"7","error_code":4,"details":"nope"}"#) {
            Inbound::Error {
                details,
                error_code,
                ..
            } => {
                assert_eq!(details, "nope");
                assert_eq!(error_code, 4);
            }
            other => panic!("expected error, got {other:?}"),
        }
        match parse_inbound(
            r#"{"event":"queue_time_updated","object_id":"k","data":{"elapsed_time":3}}"#,
        ) {
            Inbound::Event {
                event,
                object_id,
                data,
            } => {
                assert_eq!(event, "queue_time_updated");
                assert_eq!(object_id.as_deref(), Some("k"));
                assert_eq!(data["elapsed_time"], 3);
            }
            other => panic!("expected event, got {other:?}"),
        }
    }

    #[test]
    fn parses_players_groups_and_queue_progress() {
        let players = players_from_result(&json!([
            {
                "player_id": "kitchen",
                "name": "Kitchen",
                "available": true,
                "state": "playing",
                "volume_level": 35,
                "volume_muted": false,
                "type": "player",
                "group_childs": [],
                "active_source": "kitchen"
            },
            {
                "player_id": "downstairs",
                "name": "Downstairs",
                "type": "group",
                "group_childs": ["kitchen", {"player_id": "patio"}],
                "can_group_with": ["bed"]
            }
        ]));
        assert_eq!(players.len(), 2);
        assert_eq!(players[0].kind, MaPlayerKind::Player);
        assert_eq!(players[0].volume_level, Some(35.0));
        assert_eq!(players[1].kind, MaPlayerKind::Group);
        assert_eq!(
            players[1].group_members,
            vec!["kitchen".to_string(), "patio".to_string()]
        );

        let update = progress_from_payload(&json!({
            "queue_id": "kitchen",
            "state": "playing",
            "elapsed_time": 42.5,
            "current_item": {
                "name": "Show notes",
                "duration": 1800,
                "media_item": {
                    "uri": "http://192.168.1.20:8040/api/data/stream/15?api_key=abc&user_id=3"
                }
            }
        }));
        assert_eq!(update.queue_id.as_deref(), Some("kitchen"));
        assert_eq!(update.position_sec, Some(42.5));
        assert_eq!(update.duration_sec, Some(1800.0));
        assert_eq!(update.playing, Some(true));
        let stream = update.stream.unwrap();
        assert_eq!(stream.episode_id, 15);
        assert_eq!(stream.user_id, Some(3));
        assert!(!stream.is_youtube);

        let tick = progress_from_payload(&json!({"queue_id": "kitchen", "elapsed_time": 43.0}));
        assert!(tick.stream.is_none());
        assert_eq!(tick.position_sec, Some(43.0));
        assert!(!tick.media_cleared);
        assert!(!tick.reported_idle);

        let idle = progress_from_payload(&json!({
            "queue_id": "kitchen",
            "playback_state": "idle"
        }));
        assert_eq!(idle.playing, Some(false));
        assert!(idle.reported_idle);
        assert!(!idle.media_cleared);

        let stopped = progress_from_payload(&json!({
            "queue_id": "kitchen",
            "state": "idle",
            "current_item": null
        }));
        assert!(stopped.media_cleared);
    }

    fn sample_player(id: &str, state: &str) -> MaPlayer {
        MaPlayer {
            player_id: id.into(),
            name: id.into(),
            available: true,
            state: state.into(),
            volume_level: Some(20.0),
            volume_muted: false,
            kind: MaPlayerKind::Player,
            group_members: Vec::new(),
            synced_to: None,
            can_group_with: Vec::new(),
            active_source: None,
            elapsed_time: None,
            current_uri: None,
            current_title: None,
        }
    }

    #[test]
    fn playback_state_and_group_members_follow_the_2_10_names() {
        let player = player_from_value(&json!({
            "player_id": "RINCON_1",
            "name": "Living Room",
            "playback_state": "playing",
            "group_members": ["RINCON_1", "RINCON_2"],
            "type": "player"
        }))
        .unwrap();
        assert_eq!(player.state, "playing");
        assert_eq!(
            player.group_members,
            vec!["RINCON_1".to_string(), "RINCON_2".to_string()]
        );
        assert_eq!(player.kind, MaPlayerKind::Player);
    }

    #[test]
    fn partial_update_and_null_state_keep_a_playing_speaker() {
        let previous = player_from_value(&json!({
            "player_id": "RINCON_1",
            "name": "Living Room",
            "state": "playing",
            "available": true,
            "active_source": "RINCON_1",
            "volume_level": 20,
            "current_media": {"uri": "http://h/api/data/stream/4?user_id=1", "name": "Ep"}
        }))
        .unwrap();
        let volume = merge_player_update(
            Some(&previous),
            &json!({"player_id": "RINCON_1", "volume_level": 22}),
        )
        .unwrap();
        assert_eq!(volume.state, "playing");
        assert_eq!(volume.active_source.as_deref(), Some("RINCON_1"));
        assert!(volume.available);
        assert_eq!(volume.volume_level, Some(22.0));
        assert_eq!(volume.name, "Living Room");
        assert!(volume.current_uri.unwrap().contains("/stream/4"));

        let nulled = merge_player_update(
            Some(&previous),
            &json!({"player_id": "RINCON_1", "state": null, "playback_state": null}),
        )
        .unwrap();
        assert_eq!(nulled.state, "playing");
    }

    #[test]
    fn idle_with_a_loaded_episode_does_not_clear_it() {
        let (playing, clear) = next_playback(true, true, Some(false), true, false);
        assert!(!playing);
        assert!(!clear);
        let (playing, clear) = next_playback(true, true, Some(false), false, false);
        assert!(!playing);
        assert!(!clear);
        let (playing, clear) = next_playback(true, true, Some(false), true, true);
        assert!(!playing);
        assert!(clear);
    }

    #[test]
    fn coordinator_change_keeps_the_queue_that_has_the_episode() {
        let mut sonos = sample_player("RINCON_1", "idle");
        sonos.active_source = Some("group-2".into());
        let loaded = HashSet::from(["RINCON_1".to_string()]);
        let key = snapshot_queue_id(&sonos, &[sonos.clone()], &loaded, Some("RINCON_1"));
        assert_eq!(key, "RINCON_1");

        let mut member = sample_player("bed", "idle");
        member.synced_to = Some("living".into());
        let mut leader = sample_player("living", "playing");
        leader.active_source = Some("living".into());
        let loaded = HashSet::from(["living".to_string()]);
        assert_eq!(
            snapshot_queue_id(&member, &[member.clone(), leader], &loaded, None),
            "living"
        );
    }

    #[test]
    fn unavailable_player_with_an_episode_stays_registered() {
        assert!(keep_virtual_device(false, true));
        assert!(!keep_virtual_device(false, false));
        assert!(keep_virtual_device(true, false));
    }

    #[test]
    fn progress_writes_are_throttled_until_pause_seek_or_interval() {
        let first = ProgressSample {
            episode_id: 1,
            position_sec: 10.0,
            playing: true,
        };
        assert!(should_write_progress(None, &first, Duration::from_secs(0), false));
        // The first sample of a new episode waits for its resume seek.
        assert!(!should_write_progress(None, &first, PROGRESS_WRITE_EVERY, true));
        let later = ProgressSample {
            episode_id: 1,
            position_sec: 12.0,
            playing: true,
        };
        assert!(!should_write_progress(
            Some(&first),
            &later,
            Duration::from_secs(3),
            false
        ));
        assert!(should_write_progress(
            Some(&first),
            &later,
            Duration::from_secs(10),
            false
        ));
        let paused = ProgressSample {
            episode_id: 1,
            position_sec: 12.0,
            playing: false,
        };
        assert!(should_write_progress(
            Some(&later),
            &paused,
            Duration::from_secs(1),
            false
        ));
        let seek = ProgressSample {
            episode_id: 1,
            position_sec: 80.0,
            playing: true,
        };
        assert!(should_write_progress(
            Some(&later),
            &seek,
            Duration::from_secs(1),
            false
        ));
    }

    #[test]
    fn numeric_and_object_queue_times_stamp_the_right_clock() {
        let numeric = progress_from_event(Some("kitchen"), &json!(43.5), 1_700_000_100);
        assert_eq!(numeric.queue_id.as_deref(), Some("kitchen"));
        assert_eq!(numeric.position_sec, Some(43.5));
        assert_eq!(numeric.position_updated_at, Some(1_700_000_100));
        assert!(numeric.stream.is_none());
        assert!(numeric.duration_sec.is_none());

        let object = progress_from_event(
            Some("kitchen"),
            &json!({
                "elapsed_time": 42.5,
                "elapsed_time_last_updated": 1_700_000_000.4
            }),
            1_700_000_050,
        );
        assert_eq!(object.position_sec, Some(42.5));
        assert_eq!(object.position_updated_at, Some(1_700_000_000));

        let unstamped = progress_from_event(
            Some("kitchen"),
            &json!({ "elapsed_time": 10.0 }),
            1_700_000_050,
        );
        assert_eq!(unstamped.position_updated_at, Some(1_700_000_050));

        let empty = progress_from_event(Some("kitchen"), &json!({}), 1_700_000_050);
        assert!(empty.position_sec.is_none());
        assert!(empty.position_updated_at.is_none());
    }

    #[test]
    fn grouped_member_commands_use_the_leader_queue() {
        let mut bed = sample_player("bed", "idle");
        bed.synced_to = Some("living".into());
        bed.active_source = Some("bed".into());
        let mut living = sample_player("living", "playing");
        living.active_source = Some("living-queue".into());
        let players = vec![bed, living];
        assert_eq!(queue_id_for(&players, "bed"), "living-queue");
        assert_eq!(queue_id_for(&players, "living"), "living-queue");

        let mut leader_only = sample_player("bed", "idle");
        leader_only.synced_to = Some("living".into());
        assert_eq!(queue_id_for(&[leader_only], "bed"), "living");

        let play = play_media_commands("living-queue", "https://cdn.example/ep.mp3", None);
        assert_eq!(play[0].args["queue_id"], "living-queue");
        let seek = translate_action(
            "bed",
            "living-queue",
            "seek",
            &json!({"position_sec": 90.9}),
        )
        .unwrap();
        assert_eq!(seek[0].command, "player_queues/seek");
        assert_eq!(seek[0].args["queue_id"], "living-queue");
        assert_eq!(seek[0].args["position"], json!(90));
        let skip = translate_action(
            "bed",
            "living-queue",
            "skip_forward",
            &json!({"seconds": 15.8}),
        )
        .unwrap();
        assert_eq!(skip[0].args["queue_id"], "living-queue");
        assert_eq!(skip[0].args["seconds"], json!(15));
        let pause = translate_action("bed", "living-queue", "pause", &json!({})).unwrap();
        assert_eq!(pause[0].command, "players/cmd/pause");
        assert_eq!(pause[0].args["player_id"], "bed");
    }

    #[test]
    fn resume_seek_retries_only_while_the_queue_is_not_ready() {
        assert_eq!(
            plan_resume_seek(0.0, 0.0, Duration::ZERO, RESUME_SEEK_PATIENCE),
            SeekPlan::Skip
        );
        // A speaker can take many seconds to report its duration. The old
        // planner gave up after 0.7s.
        assert_eq!(
            plan_resume_seek(12.5, 0.0, Duration::from_secs(5), RESUME_SEEK_PATIENCE),
            SeekPlan::Wait
        );
        assert_eq!(
            plan_resume_seek(12.5, 0.0, RESUME_SEEK_PATIENCE, RESUME_SEEK_PATIENCE),
            SeekPlan::GiveUp("queue without duration")
        );
        assert_eq!(
            plan_resume_seek(12.5, 3600.0, Duration::ZERO, RESUME_SEEK_PATIENCE),
            SeekPlan::Send(12)
        );
        assert!(seek_is_retryable("Queue is not active"));
        assert!(seek_is_retryable("cannot seek without duration"));
        assert!(!seek_is_retryable("player offline"));
        assert_eq!(integer_seconds(12.5), 12);
        assert_eq!(integer_seconds(-3.0), 0);
        assert_eq!(integer_seconds(f64::NAN), 0);
    }

    #[tokio::test]
    async fn a_new_episode_clears_the_old_duration_and_supersedes_the_old_seek() {
        let live = SessionLive::new();
        live.note_queue_duration("q", 1800.0).await;
        let a = live.arm_resume("q", 90.0).await;
        // The previous episode's length must not release the new episode's seek.
        assert_eq!(live.queue_duration("q").await, 0.0);
        let b = live.arm_resume("q", 40.0).await;
        assert!(a.is_some() && b.is_some() && a != b);
        // The older seek ending leaves the newer one armed.
        live.finish_resume("q", a.unwrap()).await;
        assert_eq!(live.pending_resume("q").await.unwrap().position_sec, 40.0);
        // An episode with nothing to seek clears whatever was waiting.
        assert_eq!(live.arm_resume("q", 0.5).await, None);
        assert_eq!(live.pending_resume("q").await, None);
    }

    #[tokio::test]
    async fn an_empty_queue_right_after_play_media_is_not_a_stop() {
        // Music Assistant 2.10.13, first play on a queue that held nothing. After accepting
        // play_media it sends this twice before the item becomes current.
        let empty = json!({
            "queue_id": "kitchen", "active": true, "state": "idle", "items": 1,
            "current_index": null, "current_item": null, "elapsed_time": 0,
        });
        let loaded = json!({
            "queue_id": "kitchen", "state": "playing", "items": 1, "elapsed_time": 0,
            "current_item": {
                "duration": 360,
                "media_item": {"item_id": "http://feeds.example/ep1.mp3", "uri": "builtin://track/http://feeds.example/ep1.mp3"},
            },
        });
        let live = SessionLive::new();
        // Nothing loading: a null item is a real stop.
        let update = progress_from_event(None, &empty, 1_800_000_000);
        assert!(update.media_cleared);
        assert!(!stale_empty_queue(update.media_cleared, live.loading_for("kitchen").await));

        // play_episode remembers the episode, arms its seek and marks the queue loading.
        live.begin_load("kitchen").await;
        let arm = live.arm_resume("kitchen", 100.0).await;
        assert!(stale_empty_queue(update.media_cleared, live.loading_for("kitchen").await));
        // So the event is dropped and the seek stays armed.
        assert_eq!(live.pending_resume("kitchen").await.map(|p| p.arm), arm);
        // Another queue is not covered, and neither is an event that has an item.
        assert!(!stale_empty_queue(true, live.loading_for("bedroom").await));
        let update = progress_from_event(None, &loaded, 1_800_000_001);
        assert!(!stale_empty_queue(update.media_cleared, live.loading_for("kitchen").await));

        // The item showing up ends the load. A null item after that is a stop again.
        assert!(item_is_sent_media(update.item_strings.as_deref(), "http://feeds.example/ep1.mp3"));
        assert!(!item_is_sent_media(update.item_strings.as_deref(), "http://feeds.example/ep2.mp3"));
        assert!(!item_is_sent_media(update.item_strings.as_deref(), ""));
        assert!(!item_is_sent_media(None, "http://feeds.example/ep1.mp3"));
        live.end_load("kitchen").await;
        assert!(!stale_empty_queue(true, live.loading_for("kitchen").await));

        // A load that never shows its item stops covering for the queue.
        assert!(stale_empty_queue(true, Some(LOAD_GRACE - Duration::from_secs(1))));
        assert!(!stale_empty_queue(true, Some(LOAD_GRACE)));
        // A real stop forgets the load with everything else.
        live.begin_load("kitchen").await;
        live.forget("kitchen").await;
        assert_eq!(live.loading_for("kitchen").await, None);
    }

    /// A hub with one session whose outbound commands arrive on the receiver.
    async fn hub_with_session(
        user_id: i32,
    ) -> (MusicAssistantHub, Arc<SessionLive>, mpsc::UnboundedReceiver<Outbound>) {
        let hub = MusicAssistantHub::new();
        let (outbound, sent) = mpsc::unbounded_channel();
        let live = Arc::new(SessionLive::new());
        hub.inner.sessions.write().await.insert(
            user_id,
            SessionSlot {
                outbound,
                live: live.clone(),
                fingerprint: String::new(),
                cancel: watch::channel(false).0,
            },
        );
        (hub, live, sent)
    }

    #[tokio::test]
    async fn resume_seek_lands_when_the_duration_arrives_late() {
        let (hub, live, mut sent) = hub_with_session(1).await;
        let arm = live.arm_resume("q", 90.0).await.unwrap();
        let seek = {
            let hub = hub.clone();
            tokio::spawn(async move {
                land_resume_seek(&hub, 1, "q", arm, Duration::from_secs(5)).await
            })
        };
        // The speaker has not reported a duration, so nothing may be sent yet.
        tokio::time::sleep(Duration::from_millis(1200)).await;
        assert!(sent.try_recv().is_err());
        live.note_queue_duration("q", 3600.0).await;
        let outbound = tokio::time::timeout(Duration::from_secs(2), sent.recv())
            .await
            .expect("the seek is sent once the duration is known")
            .expect("the session is still open");
        assert_eq!(outbound.command, "player_queues/seek");
        assert_eq!(outbound.args, json!({ "queue_id": "q", "position": 90 }));
        outbound
            .reply
            .expect("a reply channel")
            .send(Ok(Value::Null))
            .unwrap();
        assert_eq!(seek.await.unwrap(), ResumeSeek::Landed);
        assert!(live.pending_resume("q").await.is_none());
    }

    #[tokio::test]
    async fn resume_seek_yields_to_a_newer_episode_and_times_out_still_armed() {
        let (hub, live, mut sent) = hub_with_session(1).await;
        let old = live.arm_resume("q", 90.0).await.unwrap();
        live.arm_resume("q", 40.0).await.unwrap();
        live.note_queue_duration("q", 3600.0).await;
        // The older seek must not act on the newer episode, even with a duration.
        let ended = land_resume_seek(&hub, 1, "q", old, Duration::from_secs(5)).await;
        assert_eq!(ended, ResumeSeek::Superseded);
        assert!(sent.try_recv().is_err());
        // No session for the user: nothing to seek on.
        let ended = land_resume_seek(&hub, 2, "q", old, Duration::from_secs(5)).await;
        assert_eq!(ended, ResumeSeek::Superseded);
        // No duration and no patience left: the seek times out and stays armed
        // until the caller releases it.
        let arm = live.arm_resume("other", 40.0).await.unwrap();
        let ended = land_resume_seek(&hub, 1, "other", arm, Duration::ZERO).await;
        assert_eq!(ended, ResumeSeek::TimedOut);
        assert_eq!(live.pending_resume("other").await.unwrap().arm, arm);
        release_resume(&hub, 1, "other", arm).await;
        assert!(live.pending_resume("other").await.is_none());
    }

    #[test]
    fn a_resume_point_is_staged_and_started_at_the_offset() {
        let staged = staged_start_commands("kitchen", "http://feeds.example/ep1.mp3", None, 120.7).unwrap();
        let sent: Vec<(&str, &Value)> = staged
            .stage
            .iter()
            .chain([&staged.start])
            .map(|c| (c.command.as_str(), &c.args))
            .collect();
        assert_eq!(
            sent,
            vec![
                ("player_queues/clear", &json!({ "queue_id": "kitchen" })),
                (
                    "player_queues/play_media",
                    &json!({ "queue_id": "kitchen", "media": "http://feeds.example/ep1.mp3", "option": "add" })
                ),
                (
                    "player_queues/play_index",
                    &json!({ "queue_id": "kitchen", "index": 0, "seek_position": 120 })
                ),
            ]
        );
        // Nothing here plays from 0 first: `replace` would.
        assert!(staged.stage.iter().all(|c| c.args.get("option") != Some(&json!("replace"))));
        assert_eq!(staged.from_start.args, json!({ "queue_id": "kitchen", "index": 0 }));
        // Under a second there is nothing to resume.
        assert_eq!(staged_start_commands("kitchen", "http://feeds.example/ep1.mp3", None, 0.9), None);
        assert_eq!(staged_start_commands("kitchen", "http://feeds.example/ep1.mp3", None, 0.0), None);
    }

    /// Answer the next command the hub sends, and return its name.
    async fn answer(sent: &mut mpsc::UnboundedReceiver<Outbound>, reply: Result<Value, String>) -> String {
        let outbound = tokio::time::timeout(Duration::from_secs(2), sent.recv())
            .await
            .expect("a command is sent")
            .expect("the session is still open");
        outbound.reply.expect("a reply channel").send(reply).unwrap();
        match outbound.args.get("seek_position") {
            Some(at) => format!("{} at {at}", outbound.command),
            None => outbound.command,
        }
    }

    #[tokio::test]
    async fn a_resume_starts_the_speaker_once_at_the_offset() {
        let (hub, live, mut sent) = hub_with_session(1).await;
        let staged = staged_start_commands("q", "http://feeds.example/ep1.mp3", None, 90.0).unwrap();
        live.begin_load("q").await;
        let arm = live.arm_resume("q", 90.0).await;
        let load = {
            let hub = hub.clone();
            tokio::spawn(async move { load_and_seek(&hub, 1, "q", EpisodeLoad::AtOffset(staged), arm).await })
        };
        let mut commands = Vec::new();
        for _ in 0..3 {
            commands.push(answer(&mut sent, Ok(Value::Null)).await);
        }
        assert_eq!(
            commands,
            ["player_queues/clear", "player_queues/play_media", "player_queues/play_index at 90"]
        );
        let outcome = load.await.unwrap().unwrap();
        assert!(outcome.warnings.is_empty());
        // No seek follows, and history no longer waits.
        assert!(sent.try_recv().is_err());
        assert!(live.pending_resume("q").await.is_none());
    }

    #[tokio::test]
    async fn a_refused_offset_falls_back_to_the_seek_and_a_failed_load_releases_history() {
        let (hub, live, mut sent) = hub_with_session(1).await;
        let staged = staged_start_commands("q", "http://feeds.example/ep1.mp3", None, 90.0).unwrap();
        live.begin_load("q").await;
        let arm = live.arm_resume("q", 90.0).await;
        live.note_queue_duration("q", 3600.0).await;
        let load = {
            let hub = hub.clone();
            let staged = staged.clone();
            tokio::spawn(async move { load_and_seek(&hub, 1, "q", EpisodeLoad::AtOffset(staged), arm).await })
        };
        assert_eq!(answer(&mut sent, Ok(Value::Null)).await, "player_queues/clear");
        assert_eq!(answer(&mut sent, Ok(Value::Null)).await, "player_queues/play_media");
        assert_eq!(answer(&mut sent, Err("unknown argument".into())).await, "player_queues/play_index at 90");
        // Started from the beginning instead, then the seek that waits for a duration.
        assert_eq!(answer(&mut sent, Ok(Value::Null)).await, "player_queues/play_index");
        assert_eq!(answer(&mut sent, Ok(Value::Null)).await, "player_queues/seek");
        let outcome = load.await.unwrap().unwrap();
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.warnings[0].contains("unknown argument"));
        assert!(live.pending_resume("q").await.is_none());

        // The item could not even be staged: an error, and neither the history hold nor the
        // load stays behind.
        live.begin_load("q").await;
        let arm = live.arm_resume("q", 90.0).await;
        let load = {
            let hub = hub.clone();
            tokio::spawn(async move { load_and_seek(&hub, 1, "q", EpisodeLoad::AtOffset(staged), arm).await })
        };
        assert_eq!(answer(&mut sent, Ok(Value::Null)).await, "player_queues/clear");
        assert_eq!(answer(&mut sent, Err("no playable items".into())).await, "player_queues/play_media");
        assert!(load.await.unwrap().is_err());
        assert!(sent.try_recv().is_err());
        assert!(live.pending_resume("q").await.is_none());
        assert_eq!(live.loading_for("q").await, None);
    }

    #[test]
    fn remembered_remote_url_writes_history_until_a_real_stop() {
        let remembered = RememberedEpisode {
            user_id: 3,
            episode_id: 15,
            is_youtube: false,
            duration_sec: 1800.0,
            playlist_id: Some(7),
            media_url: String::new(),
        };
        match history_stream(Some(&remembered), None, false, 3, false, None) {
            HistoryEpisode::Write(stream) => {
                assert_eq!(stream.episode_id, 15);
                assert_eq!(stream.user_id, Some(3));
                assert!(!stream.is_youtube);
            }
            other => panic!("expected remembered episode, got {other:?}"),
        }
        assert_eq!(
            history_stream(Some(&remembered), None, true, 3, false, None),
            HistoryEpisode::Clear
        );
        assert_eq!(
            next_remembered(Some(remembered.clone()), None, true, 3),
            None
        );

        let foreign = StreamRef {
            episode_id: 99,
            user_id: Some(8),
            is_youtube: false,
        };
        assert_eq!(
            history_stream(Some(&remembered), Some(&foreign), false, 3, false, None),
            HistoryEpisode::Foreign
        );
        assert_eq!(
            next_remembered(Some(remembered.clone()), Some(&foreign), false, 3),
            Some(remembered.clone())
        );

        let other = StreamRef {
            episode_id: 44,
            user_id: Some(3),
            is_youtube: true,
        };
        assert_eq!(
            next_remembered(Some(remembered.clone()), Some(&other), false, 3),
            Some(RememberedEpisode {
                user_id: 3,
                episode_id: 44,
                is_youtube: true,
                duration_sec: 0.0,
                playlist_id: None,
                media_url: String::new(),
            })
        );
        match history_stream(None, Some(&other), false, 3, false, None) {
            HistoryEpisode::Write(stream) => assert!(stream.is_youtube),
            other => panic!("expected youtube stream, got {other:?}"),
        }
        assert_eq!(
            next_remembered(Some(remembered.clone()), None, false, 3)
                .unwrap()
                .duration_sec,
            1800.0
        );
    }

    #[test]
    fn a_late_tick_keeps_the_loaded_episode_after_the_next_is_remembered() {
        let next = RememberedEpisode {
            user_id: 3,
            episode_id: 16,
            is_youtube: false,
            duration_sec: 0.0,
            playlist_id: None,
            media_url: "https://cdn.example/16.mp3".into(),
        };
        // The tick from the previous item must not be credited to episode 16.
        assert_eq!(
            history_stream(Some(&next), None, false, 3, true, None),
            HistoryEpisode::None
        );
        // With nothing loaded, the remembered episode still names a remote URL.
        match history_stream(Some(&next), None, false, 3, false, None) {
            HistoryEpisode::Write(stream) => assert_eq!(stream.episode_id, 16),
            other => panic!("expected remembered episode, got {other:?}"),
        }
        // A real stop still clears.
        assert_eq!(
            history_stream(Some(&next), None, true, 3, true, None),
            HistoryEpisode::Clear
        );
    }

    #[test]
    fn same_episode_keeps_its_playlist_and_another_drops_it() {
        let remembered = RememberedEpisode {
            user_id: 3,
            episode_id: 15,
            is_youtube: false,
            duration_sec: 1800.0,
            playlist_id: Some(7),
            media_url: String::new(),
        };
        let same = StreamRef {
            episode_id: 15,
            user_id: Some(3),
            is_youtube: false,
        };
        let kept = next_remembered(Some(remembered.clone()), Some(&same), false, 3).unwrap();
        assert_eq!(kept.playlist_id, Some(7));
        let other = StreamRef {
            episode_id: 16,
            ..same
        };
        let replaced = next_remembered(Some(remembered), Some(&other), false, 3).unwrap();
        assert_eq!(replaced.playlist_id, None);
    }

    #[test]
    fn an_event_about_the_previous_item_is_not_credited_to_the_new_episode() {
        // Music Assistant 2.10.4, Chromecast: right after play_media for
        // episode 94 it re-sent the previous item (episode 33 at 308s) before
        // the new one, and 308s was written to episode 94's history.
        let next = RememberedEpisode {
            user_id: 2,
            episode_id: 94,
            is_youtube: false,
            duration_sec: 5186.0,
            playlist_id: None,
            media_url: "https://op3.dev/e/mp3s.example/NA-1841.mp3".into(),
        };
        let old_item = progress_from_payload(&json!({
            "queue_id": "rr",
            "elapsed_time": 308.8,
            "state": "idle",
            "current_item": {
                "queue_item_id": "1d4b",
                "name": "No Agenda Show - 1902",
                "duration": 6565,
                "media_item": { "item_id": "https://op3.dev/e/mp3s.example/NA-1902.mp3" }
            }
        }));
        assert_eq!(
            history_stream(Some(&next), None, false, 2, false, old_item.item_strings.as_deref()),
            HistoryEpisode::OtherItem
        );
        let new_item = progress_from_payload(&json!({
            "queue_id": "rr",
            "elapsed_time": 0.0,
            "state": "playing",
            "current_item": {
                "queue_item_id": "7869",
                "media_item": { "uri": "builtin://track/https://op3.dev/e/mp3s.example/NA-1841.mp3" }
            }
        }));
        match history_stream(Some(&next), None, false, 2, false, new_item.item_strings.as_deref()) {
            HistoryEpisode::Write(stream) => assert_eq!(stream.episode_id, 94),
            other => panic!("expected episode 94, got {other:?}"),
        }
        // A numeric tick carries no item at all.
        assert_eq!(progress_from_payload(&json!(5131.0)).item_strings, None);
        // A remembered episode learned from a stream URL has no media URL to check.
        let learned = RememberedEpisode { media_url: String::new(), ..next };
        assert!(matches!(
            history_stream(Some(&learned), None, false, 2, false, old_item.item_strings.as_deref()),
            HistoryEpisode::Write(_)
        ));
    }

    #[test]
    fn a_snapshot_between_anchors_does_not_jump_back() {
        // Anchored at 100s six seconds ago. A player refresh publishes now, and the relay
        // stamps it with now: sending 100 would show 100 again where the client was at 106.
        let anchored_at = 1_800_000_000;
        let now = anchored_at + 6;
        assert_eq!(snapshot_position(100.0, anchored_at, true, 1.0, 360.0, now), 106.0);
        // A paused queue stays where it is, and the end of the file is the limit.
        assert_eq!(snapshot_position(100.0, anchored_at, false, 1.0, 360.0, now), 100.0);
        assert_eq!(snapshot_position(358.0, anchored_at, true, 1.0, 360.0, now), 360.0);
        assert_eq!(snapshot_position(358.0, anchored_at, true, 1.0, 0.0, now), 364.0);
    }

    #[test]
    fn a_quiet_queue_projects_past_thirty_seconds() {
        // Music Assistant 2.10.4 on a Chromecast: last event at 5131s, then
        // nothing for 56 seconds until the queue went idle at 5186s.
        let measured_at = 1_792_804_314;
        let now = measured_at + 56;
        assert_eq!(projected_position(5131.0, measured_at, true, 1.0, now), 5187.0);
        // That reaches the end; the old 30 second cap stopped at 5161 and missed it.
        assert!(advance_due(5187.0, true, false, 5186.0, 0));
        assert!(!advance_due(5161.0, true, false, 5186.0, 0));
        // Paused, unstamped, or a clock behind the measurement do not move it.
        assert_eq!(projected_position(5131.0, measured_at, false, 1.0, now), 5131.0);
        assert_eq!(projected_position(5131.0, 0, true, 1.0, now), 5131.0);
        assert_eq!(projected_position(5131.0, now, true, 1.0, measured_at), 5131.0);
        assert_eq!(projected_position(100.0, measured_at, true, f64::NAN, measured_at + 10), 110.0);
    }

    #[test]
    fn advance_fires_at_the_outro_or_the_natural_end_but_not_on_pause() {
        // Outro while playing, as local web does.
        assert!(advance_due(1771.0, true, true, 1800.0, 30));
        assert!(!advance_due(1769.0, true, true, 1800.0, 30));
        // No outro: playing never advances; the queue stopping at the end does.
        assert!(!advance_due(1800.0, true, true, 1800.0, 0));
        assert!(advance_due(1797.0, true, false, 1800.0, 0));
        // A pause, including a Sonos pause reported as idle, mid-episode.
        assert!(!advance_due(900.0, true, false, 1800.0, 0));
        assert!(!advance_due(1790.0, true, false, 1800.0, 0));
        // Already stopped, or no duration yet.
        assert!(!advance_due(1799.0, false, false, 1800.0, 0));
        assert!(!advance_due(10.0, true, false, 0.0, 0));
    }

    #[test]
    fn sleep_timer_modes_parse_like_mobile() {
        let now = 1_000;
        let parse = |args: Value| sleep_timer_from_args(&args, now);
        assert_eq!(parse(json!({"mode": "off"})), Ok(None));
        assert_eq!(
            parse(json!({"mode": "minutes", "minutes": 30})),
            Ok(Some(SleepTimer::Minutes { ends_at: 2_800 }))
        );
        assert_eq!(
            parse(json!({"mode": "end_of_episode"})),
            Ok(Some(SleepTimer::EndOfEpisode))
        );
        assert_eq!(
            parse(json!({"mode": "episodes", "episodes": 3})),
            Ok(Some(SleepTimer::Episodes { left: 3 }))
        );
        // One episode is the end of this one.
        assert_eq!(
            parse(json!({"mode": "episodes", "episodes": 1})),
            Ok(Some(SleepTimer::EndOfEpisode))
        );
        for bad in [
            json!({"mode": "minutes"}),
            json!({"mode": "minutes", "minutes": 0}),
            json!({"mode": "minutes", "minutes": 721}),
            json!({"mode": "episodes", "episodes": 11}),
            json!({"mode": "later"}),
            json!({}),
        ] {
            assert!(parse(bad.clone()).is_err(), "{bad}");
        }
    }

    #[test]
    fn sleep_timer_stops_after_the_last_counted_episode_or_on_time() {
        assert_eq!(sleep_after_episode(None), (false, None));
        assert_eq!(
            sleep_after_episode(Some(SleepTimer::EndOfEpisode)),
            (true, None)
        );
        assert_eq!(
            sleep_after_episode(Some(SleepTimer::Episodes { left: 3 })),
            (false, Some(SleepTimer::Episodes { left: 2 }))
        );
        assert_eq!(
            sleep_after_episode(Some(SleepTimer::Episodes { left: 1 })),
            (true, None)
        );
        // A minutes timer does not care about episode ends.
        let minutes = Some(SleepTimer::Minutes { ends_at: 50 });
        assert_eq!(sleep_after_episode(minutes), (false, minutes));
        assert!(!sleep_due(minutes, 49));
        assert!(sleep_due(minutes, 50));
        assert!(!sleep_due(Some(SleepTimer::EndOfEpisode), i64::MAX));
        assert!(!sleep_due(None, 100));
    }

    #[test]
    fn a_sonos_pause_holds_the_resume_point_until_it_plays_again() {
        // Sonos Move, Music Assistant 2.10.4: played from 5592, paused about
        // 23s later. Elapsed fell back to 5592; resume_pos was the pause point.
        let paused = progress_from_payload(&json!({
            "queue_id": "RINCON_1",
            "state": "idle",
            "elapsed_time": 5592.0,
            "resume_pos": 5615
        }));
        assert_eq!(paused.playing, Some(false));
        assert_eq!(paused.position_sec, Some(5592.0));
        assert_eq!(paused.resume_pos, Some(5615.0));
        assert!(progress_from_payload(&json!(5592.0)).resume_pos.is_none());

        let held = paused_position(None, paused.resume_pos, false);
        assert_eq!(held, Some(5615.0));
        // A later report without resume_pos, or with the 0 play_index sets
        // before the queue reports playing, keeps the pause point.
        assert_eq!(paused_position(held, None, false), Some(5615.0));
        assert_eq!(paused_position(held, Some(0.0), false), Some(5615.0));
        // Playing again releases it, and a queue that never paused has none.
        assert_eq!(paused_position(held, Some(5615.0), true), None);
        assert_eq!(paused_position(None, Some(0.0), false), None);
        assert_eq!(paused_position(None, None, false), None);
    }

    #[test]
    fn the_zero_after_the_end_does_not_resume_history_writes() {
        // Chromecast, Music Assistant 2.10.4: after an episode finished and
        // the speaker stopped there, a bare queue_time_updated 0 arrived while
        // it still reported playing, and history was overwritten with 0.
        // (playing, position, duration, end_skip, replayed)
        let cases = [
            (true, 0.0, 6565.0, 0, false),
            (true, 0.4, 6565.0, 0, false),
            (true, 30.0, 6565.0, 0, true),
            (true, 6000.0, 6565.0, 60, true),
            (true, 6563.0, 6565.0, 0, false),
            (false, 30.0, 6565.0, 0, false),
        ];
        for (playing, position, duration, end_skip, want) in cases {
            assert_eq!(
                replayed_after_finish(playing, position, duration, end_skip),
                want,
                "playing {playing} at {position} of {duration}, end_skip {end_skip}"
            );
        }
    }

    #[test]
    fn a_jump_to_zero_at_the_end_finishes_but_a_seek_to_zero_does_not() {
        // (reported, projected, was playing, playing, duration, end_skip, finished)
        let cases = [
            // Music Assistant 2.10.4, Chromecast, episode 22: the bridge had
            // projected 10647 when queue_time_updated 0 arrived while the queue
            // still said playing. Idle with 0 followed a second later.
            (Some(0.0), 10647.0, true, true, 10647.0, 0, true),
            (Some(0.0), 10646.0, true, false, 10646.0, 0, true),
            // Idle with the final position, as an earlier run reported.
            (Some(5186.1), 5180.0, true, false, 5186.0, 0, true),
            // A seek to the start mid-episode, or a stop after one.
            (Some(0.0), 900.0, true, true, 1800.0, 0, false),
            (Some(0.0), 900.0, true, false, 1800.0, 0, false),
            // Before the episode was playing, 0 is just the start.
            (Some(0.0), 10647.0, false, true, 10647.0, 0, false),
            // A plain playing event keeps the outro rule.
            (Some(1790.0), 1790.0, true, true, 1800.0, 0, false),
            (Some(1771.0), 1771.0, true, true, 1800.0, 30, true),
        ];
        for (reported, projected, was, now, duration, end_skip, want) in cases {
            assert_eq!(
                episode_finished(reported, projected, was, now, duration, end_skip),
                want,
                "{reported:?} after {projected}, playing {was} -> {now}"
            );
        }
    }

    fn entry(episode_id: i32, position: i32, completed: bool) -> QueueEntry {
        QueueEntry {
            episode_id,
            is_youtube: false,
            position: Some(position),
            completed,
        }
    }

    #[test]
    fn queue_next_skips_the_current_and_finished_rows() {
        let queue = vec![
            entry(5, 3, false),
            entry(9, 1, false),
            entry(4, 2, true),
            entry(6, 4, false),
        ];
        // Finished: first other unfinished row, wherever the current one sat.
        assert_eq!(pick_queued(&queue, (9, false), false), Some((5, false)));
        assert_eq!(pick_queued(&queue, (5, false), false), Some((9, false)));
        assert_eq!(pick_queued(&queue, (77, false), false), Some((9, false)));
        // Skip: the row after the current one, so a skipped row does not come back.
        assert_eq!(pick_queued(&queue, (5, false), true), Some((6, false)));
        assert_eq!(pick_queued(&queue, (6, false), true), None);
        assert_eq!(pick_queued(&queue, (77, false), true), Some((9, false)));
        // A video with the same number is a different row.
        let mixed = vec![
            QueueEntry {
                is_youtube: true,
                ..entry(9, 1, false)
            },
            entry(9, 2, false),
        ];
        assert_eq!(pick_queued(&mixed, (9, false), false), Some((9, true)));
        assert_eq!(pick_queued(&[], (9, false), false), None);
    }

    #[test]
    fn next_is_advertised_and_carries_the_playlist() {
        assert!(MA_CAPABILITIES.contains(&"next"));
        let args = next_args(&NextEpisode {
            episode_id: 12,
            is_youtube: false,
            playlist_id: Some(3),
        });
        assert_eq!(
            args,
            json!({ "episode_id": 12, "is_youtube": false, "playlist_id": 3 })
        );
        let plain = next_args(&NextEpisode {
            episode_id: 12,
            is_youtube: true,
            playlist_id: None,
        });
        assert!(plain.get("playlist_id").is_none());
        // The web app matches this to show an info toast instead of an error.
        assert_eq!(NOTHING_NEXT, "nothing_next");
    }

    #[test]
    fn resume_position_honors_an_explicit_zero_and_the_intro_skip() {
        assert_eq!(resume_position(None, 90.0, 30), 90.0);
        assert_eq!(resume_position(None, 10.0, 30), 30.0);
        assert_eq!(resume_position(Some(0.0), 90.0, 30), 0.0);
        assert_eq!(resume_position(Some(90.0), 10.0, 30), 90.0);
        assert_eq!(resume_position(None, 0.0, 0), 0.0);
    }

    #[test]
    fn completion_uses_end_skip_and_auto_complete() {
        assert!(should_mark_complete(70.0, 100.0, 0, 30));
        assert!(!should_mark_complete(69.0, 100.0, 0, 30));
        assert!(should_mark_complete(91.0, 100.0, 10, 0));
        assert!(!should_mark_complete(89.0, 100.0, 10, 0));
        assert!(should_mark_complete(100.0, 100.0, 0, 0));
        assert!(!should_mark_complete(99.0, 100.0, 0, 0));
        assert!(!should_mark_complete(100.0, 0.0, 10, 30));
    }

    #[test]
    fn a_one_second_tick_does_not_rewrite_history() {
        let prev = ProgressSample {
            episode_id: 15,
            position_sec: 40.0,
            playing: true,
        };
        let tick = ProgressSample {
            episode_id: 15,
            position_sec: 41.0,
            playing: true,
        };
        assert!(!should_write_progress(
            Some(&prev),
            &tick,
            Duration::from_secs(1),
            false
        ));
        let backward = ProgressSample {
            episode_id: 15,
            position_sec: 20.0,
            playing: true,
        };
        assert!(should_write_progress(
            Some(&prev),
            &backward,
            Duration::from_secs(1),
            false
        ));
    }

    #[test]
    fn relay_command_frame_parses() {
        let (action, args) = parse_relay_command(
            r#"{"type":"command","from_device_id":"web","action":"pause","args":{}}"#,
        )
        .unwrap();
        assert_eq!(action, "pause");
        assert!(args.as_object().unwrap().is_empty());
        assert!(parse_relay_command(r#"{"type":"devices","devices":[]}"#).is_none());
    }

    async fn recv_json(socket: &mut WebSocketStream<DuplexStream>) -> Value {
        loop {
            match socket.next().await.unwrap().unwrap() {
                Message::Text(t) => return serde_json::from_str(t.as_ref()).unwrap(),
                Message::Ping(p) => {
                    socket.send(Message::Pong(p)).await.unwrap();
                }
                other => panic!("unexpected frame {other:?}"),
            }
        }
    }

    async fn send_json(socket: &mut WebSocketStream<DuplexStream>, v: Value) {
        socket
            .send(Message::Text(v.to_string().into()))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn session_auths_lists_players_controls_and_reports_progress() {
        let (client_io, server_io) = tokio::io::duplex(64 * 1024);
        let client = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
        let mut server = WebSocketStream::from_raw_socket(server_io, Role::Server, None).await;

        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (ev_tx, mut ev_rx) = mpsc::unbounded_channel();
        let drive =
            tokio::spawn(async move { drive_socket(client, "secret-token", cmd_rx, ev_tx).await });

        let auth = recv_json(&mut server).await;
        assert_eq!(auth["command"], "auth");
        assert_eq!(auth["args"]["token"], "secret-token");
        let auth_id = auth["message_id"].as_str().unwrap().to_string();
        send_json(
            &mut server,
            json!({"message_id": auth_id, "result": {"authenticated": true}}),
        )
        .await;

        let info = recv_json(&mut server).await;
        assert_eq!(info["command"], "info");
        let info_id = info["message_id"].as_str().unwrap().to_string();
        send_json(
            &mut server,
            json!({
                "message_id": info_id,
                "result": {"server_version": "2.10.3", "schema_version": 80, "server_id": "srv"}
            }),
        )
        .await;

        let players = recv_json(&mut server).await;
        assert_eq!(players["command"], "players/all");
        let players_id = players["message_id"].as_str().unwrap().to_string();
        send_json(
            &mut server,
            json!({
                "message_id": players_id,
                "result": [{
                    "player_id": "kitchen",
                    "name": "Kitchen",
                    "available": true,
                    "state": "idle",
                    "type": "player",
                    "volume_level": 20
                }]
            }),
        )
        .await;

        let mut saw_server = false;
        let mut saw_players = false;
        while let Some(note) = ev_rx.recv().await {
            match note {
                SessionNote::Server(info) => {
                    assert_eq!(info.server_version, "2.10.3");
                    assert!(info.assessment.compatible);
                    saw_server = true;
                }
                SessionNote::Players(list) => {
                    assert_eq!(list.len(), 1);
                    assert_eq!(list[0].player_id, "kitchen");
                    saw_players = true;
                    break;
                }
                SessionNote::Event { .. } => {}
            }
        }
        assert!(saw_server && saw_players);

        cmd_tx
            .send(Outbound {
                command: "player_queues/play_media".into(),
                args: json!({
                    "queue_id": "kitchen",
                    "media": "http://192.168.1.20:8040/api/data/stream/15?api_key=abc&user_id=3",
                    "option": "replace"
                }),
                reply: None,
            })
            .unwrap();

        let play = recv_json(&mut server).await;
        assert_eq!(play["command"], "player_queues/play_media");
        assert_eq!(
            play["args"]["media"],
            "http://192.168.1.20:8040/api/data/stream/15?api_key=abc&user_id=3"
        );
        let play_id = play["message_id"].as_str().unwrap().to_string();
        send_json(&mut server, json!({"message_id": play_id, "result": true})).await;
        send_json(
            &mut server,
            json!({
                "event": "queue_updated",
                "object_id": "kitchen",
                "data": {
                    "queue_id": "kitchen",
                    "state": "playing",
                    "elapsed_time": 1.5,
                    "current_item": {
                        "duration": 100,
                        "media_item": {
                            "uri": "http://192.168.1.20:8040/api/data/stream/15?api_key=abc&user_id=3"
                        }
                    }
                }
            }),
        )
        .await;

        let mut progress = None;
        while let Some(note) = ev_rx.recv().await {
            if let SessionNote::Event { event, data, .. } = note {
                assert_eq!(event, "queue_updated");
                progress = Some(progress_from_payload(&data));
                break;
            }
        }
        let progress = progress.expect("queue event");
        assert_eq!(progress.stream.unwrap().episode_id, 15);
        assert_eq!(progress.position_sec, Some(1.5));
        assert_eq!(progress.playing, Some(true));

        drive.abort();
    }

    #[tokio::test]
    async fn the_probe_reports_a_rejected_token_every_time() {
        // The driver's error and its closed channel become ready together. Which one the
        // probe sees first is random, so one run proves nothing: half of them used to say
        // "connected, 0 player(s)".
        for round in 0..40 {
            let (client_io, server_io) = tokio::io::duplex(8 * 1024);
            let client = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
            let mut server = WebSocketStream::from_raw_socket(server_io, Role::Server, None).await;
            let probe = tokio::spawn(async move { probe_socket(client, "nope").await });
            let auth = recv_json(&mut server).await;
            let id = auth["message_id"].as_str().unwrap().to_string();
            send_json(
                &mut server,
                json!({"message_id": id, "error_code": 1, "details": "Invalid or expired token"}),
            )
            .await;
            let report = probe.await.unwrap();
            assert!(!report.ok, "round {round}: {}", report.detail);
            assert!(report.detail.contains("Invalid or expired token"), "round {round}: {}", report.detail);
        }
    }

    #[tokio::test]
    async fn auth_failure_ends_the_session() {
        let (client_io, server_io) = tokio::io::duplex(8 * 1024);
        let client = WebSocketStream::from_raw_socket(client_io, Role::Client, None).await;
        let mut server = WebSocketStream::from_raw_socket(server_io, Role::Server, None).await;
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<Outbound>();
        let (ev_tx, _ev_rx) = mpsc::unbounded_channel();
        let drive = tokio::spawn(async move { drive_socket(client, "nope", cmd_rx, ev_tx).await });
        let auth = recv_json(&mut server).await;
        let id = auth["message_id"].as_str().unwrap().to_string();
        send_json(
            &mut server,
            json!({"message_id": id, "error_code": 1, "details": "Invalid or expired token"}),
        )
        .await;
        let err = drive.await.unwrap().unwrap_err();
        assert!(err.contains("Invalid or expired token"), "{err}");
        drop(cmd_tx);
    }
}
