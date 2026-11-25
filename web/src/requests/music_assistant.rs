// Music Assistant settings, discovery, and control for the web client.
//
// The HTTP routes are in the server's OpenAPI document under the `music_assistant` tag. Playback state
// for players that are online also arrives on the now-playing relay
// (`device_type` `music_assistant`). This module owns the request types, the
// small pieces of UI logic that need tests (error copy, grouping, which device
// the now-playing bar follows), and the yewdux store the settings page and the
// speaker picker share.

use std::collections::HashMap;

use anyhow::Error;
use gloo_net::http::Request;
use serde::Deserialize;
use serde_json::{json, Value};
use yewdux::prelude::*;

/// How long a playing, buffering, paused, or loaded-queue observation keeps
/// the speaker-row buttons up. Longer than the 12 second player refresh.
pub const RECENT_TRANSPORT_SECS: i64 = 60;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaSettings {
    pub enabled: bool,
    #[serde(default)]
    pub server_url: Option<String>,
    #[serde(default)]
    pub external_base_url: Option<String>,
    #[serde(default)]
    pub has_token: bool,
    #[serde(default)]
    pub connected: bool,
    #[serde(default)]
    pub server_version: Option<String>,
    #[serde(default)]
    pub schema_version: Option<i64>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaTestReport {
    pub ok: bool,
    pub detail: String,
    #[serde(default)]
    pub server_version: Option<String>,
    #[serde(default)]
    pub schema_version: Option<i64>,
    #[serde(default)]
    pub player_count: i64,
    #[serde(default)]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct MaPlayer {
    pub player_id: String,
    pub device_id: String,
    pub name: String,
    /// `player` or `group`.
    pub kind: String,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub volume_level: Option<f64>,
    #[serde(default)]
    pub volume_muted: bool,
    #[serde(default)]
    pub group_members: Vec<String>,
    #[serde(default)]
    pub synced_to: Option<String>,
    #[serde(default)]
    pub can_group_with: Vec<String>,
    #[serde(default)]
    pub queue_id: String,
}

impl MaPlayer {
    pub fn is_group(&self) -> bool {
        self.kind.eq_ignore_ascii_case("group") || !self.group_members.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaPlayersResponse {
    pub enabled: bool,
    pub connected: bool,
    pub players: Vec<MaPlayer>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct MaCommandResponse {
    pub ok: bool,
    pub detail: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PendingEpisode {
    pub episode_id: i32,
    pub is_youtube: bool,
}

/// Who the bottom bar follows while this browser is not playing. The last
/// start wins: this browser starting playback, the user sending audio to a
/// speaker, or a speaker starting to play.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum PlaybackFocus {
    /// Nothing has started this session.
    #[default]
    Unset,
    Local,
    /// Now-playing `device_id` of the speaker.
    Speaker(String),
}

impl PlaybackFocus {
    pub fn speaker_id(&self) -> Option<&str> {
        match self {
            PlaybackFocus::Speaker(id) => Some(id.as_str()),
            _ => None,
        }
    }
}

/// Shared by the settings page, the speaker picker, and the now-playing bar.
/// Speaker controls read `enabled` and stay hidden until it is true.
#[derive(Default, Clone, PartialEq, Store)]
pub struct MusicAssistantState {
    pub loaded: bool,
    pub enabled: bool,
    pub connected: bool,
    pub server_version: Option<String>,
    pub warning: Option<String>,
    pub last_error: Option<String>,
    pub players: Vec<MaPlayer>,
    pub picker_open: bool,
    /// Set when an episode menu asks the picker to start that episode.
    pub pending_episode: Option<PendingEpisode>,
    /// Which player the bottom bar follows while this browser is not playing.
    pub focus: PlaybackFocus,
    /// Unix seconds when a player last had transport, a loaded episode, or
    /// relay playback. A later idle or unavailable poll still shows controls.
    pub recent_transport: HashMap<String, i64>,
}

pub fn apply_settings(view: &MaSettings) {
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        s.loaded = true;
        s.enabled = view.enabled;
        s.connected = view.connected;
        s.server_version = view.server_version.clone();
        s.warning = view.warning.clone();
        s.last_error = view.last_error.clone();
        if !view.enabled {
            s.players.clear();
            s.picker_open = false;
            s.recent_transport.clear();
        }
    });
}

pub fn apply_players(list: MaPlayersResponse) {
    let now = unix_now();
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        // Remember the state this refresh is about to replace. A Sonos poll
        // that comes back idle or unavailable must not drop the buttons.
        for player in &s.players {
            if transport_state_live(&player.state) {
                s.recent_transport.insert(player.player_id.clone(), now);
            }
        }
        s.loaded = true;
        s.enabled = list.enabled;
        s.connected = list.connected;
        s.players = if list.enabled {
            list.players
        } else {
            Vec::new()
        };
        if !list.enabled {
            s.picker_open = false;
            s.recent_transport.clear();
        } else {
            for player in &s.players {
                if transport_state_live(&player.state) {
                    s.recent_transport.insert(player.player_id.clone(), now);
                }
            }
        }
    });
}

/// Record players whose relay snapshot still has an episode or is playing.
pub fn note_relay_transport(player_ids: &[String]) {
    if player_ids.is_empty() {
        return;
    }
    let now = unix_now();
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        for id in player_ids {
            if !id.is_empty() {
                s.recent_transport.insert(id.clone(), now);
            }
        }
    });
}

pub fn unix_now() -> i64 {
    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now() / 1000.0) as i64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// True when `seen_at` is within [`RECENT_TRANSPORT_SECS`] of `now`.
pub fn transport_is_recent(seen_at: Option<i64>, now: i64) -> bool {
    match seen_at {
        Some(seen) if now >= seen && now.saturating_sub(seen) <= RECENT_TRANSPORT_SECS => true,
        _ => false,
    }
}

pub fn transport_state_live(player_state: &str) -> bool {
    let state = player_state.trim();
    state.eq_ignore_ascii_case("playing")
        || state.eq_ignore_ascii_case("buffering")
        || state.eq_ignore_ascii_case("paused")
}

/// Open the speaker picker for one episode (episode context menu).
pub fn open_speaker_picker(episode_id: i32, is_youtube: bool) {
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        s.pending_episode = Some(PendingEpisode {
            episode_id,
            is_youtube,
        });
        s.picker_open = true;
    });
}

// ============================ Errors =====================================

/// Why a settings or test call failed, mapped to a short explanation in the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaProblem {
    /// External base URL is localhost, loopback, or link-local.
    LocalhostRejected,
    /// The Music Assistant WebSocket could not be opened.
    Unreachable,
    /// Playback can start and then stop because MA cannot fetch the stream.
    StreamHint,
    NotConfigured,
    AuthFailed,
    Other,
}

/// Result of a Test click: the probe outcome plus which explanations to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestExplanation {
    pub ok: bool,
    pub problems: Vec<MaProblem>,
    /// True when the form should repeat that MA must reach the stream URL.
    pub show_stream_hint: bool,
    pub detail: String,
}

/// Pull `message` out of an `{error, message, status_code}` body. Other text
/// is returned unchanged.
pub fn message_from_error_body(body: &str) -> String {
    let trimmed = body.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if let Some(message) = value.get("message").and_then(Value::as_str) {
            let message = message.trim();
            if !message.is_empty() {
                return message.to_string();
            }
        }
    }
    trimmed.to_string()
}

/// Host of an http(s) URL, without port or brackets. Empty when it cannot be split.
pub fn external_host(raw: &str) -> String {
    let trimmed = raw.trim();
    let after_scheme = trimmed
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(trimmed);
    let hostport = after_scheme
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(after_scheme);
    let host = if let Some(rest) = hostport.strip_prefix('[') {
        rest.split(']').next().unwrap_or(rest)
    } else {
        hostport.split(':').next().unwrap_or(hostport)
    };
    host.trim().to_ascii_lowercase()
}

/// Client-side mirror of the server's external-base rejection, so the form can
/// explain the failure before the round trip. The server stays authoritative.
pub fn external_base_is_rejected(raw: &str) -> bool {
    let host = external_host(raw);
    if host.is_empty() {
        return false;
    }
    if host == "localhost"
        || host == "localhost.localdomain"
        || host.ends_with(".localhost")
        || host == "metadata.google.internal"
        || host.ends_with(".metadata.google.internal")
    {
        return true;
    }
    if host == "127.0.0.1" || host == "::1" || host == "0.0.0.0" || host == "::" {
        return true;
    }
    if host.starts_with("169.254.") {
        return true;
    }
    // fe80::/10, written without brackets after `external_host` strips them.
    let v6 = host.split('%').next().unwrap_or(&host);
    v6.starts_with("fe80:")
}

pub fn classify_ma_text(text: &str) -> MaProblem {
    let lower = text.to_ascii_lowercase();
    if lower.contains("localhost")
        || lower.contains("loopback")
        || lower.contains("link-local")
        || lower.contains("must be a lan")
        || lower.contains("music assistant can reach")
    {
        return MaProblem::LocalhostRejected;
    }
    if lower.contains("could not connect")
        || lower.contains("timed out")
        || lower.contains("connection refused")
        || lower.contains("unreachable")
        || lower.contains("os error")
        || lower.contains("dns error")
        || lower.contains("network is unreachable")
    {
        return MaProblem::Unreachable;
    }
    if lower.contains("stream")
        && (lower.contains("reach") || lower.contains("fetch") || lower.contains("external"))
    {
        return MaProblem::StreamHint;
    }
    if lower.contains("not configured") || lower.contains("is required") {
        return MaProblem::NotConfigured;
    }
    if lower.contains("invalid or expired token")
        || lower.contains("authentication")
        || lower.contains("unauthorized")
        || (lower.contains("token") && (lower.contains("invalid") || lower.contains("expired")))
    {
        return MaProblem::AuthFailed;
    }
    MaProblem::Other
}

fn push_unique(problems: &mut Vec<MaProblem>, problem: MaProblem) {
    if !problems.contains(&problem) {
        problems.push(problem);
    }
}

/// Combine the typed external base URL with the probe (or the HTTP error text).
pub fn explain_test(external_base: &str, probe: Result<&MaTestReport, &str>) -> TestExplanation {
    let mut problems = Vec::new();
    let mut details: Vec<String> = Vec::new();
    if external_base_is_rejected(external_base) {
        push_unique(&mut problems, MaProblem::LocalhostRejected);
        push_unique(&mut problems, MaProblem::StreamHint);
    }
    match probe {
        Ok(report) => {
            if let Some(warning) = report.warning.as_deref().filter(|w| !w.is_empty()) {
                details.push(warning.to_string());
            }
            if !report.detail.is_empty() {
                details.push(report.detail.clone());
            }
            if !report.ok {
                push_unique(&mut problems, classify_ma_text(&report.detail));
            }
            let blocking = problems.iter().any(|p| *p != MaProblem::StreamHint);
            TestExplanation {
                ok: report.ok && !blocking,
                problems,
                show_stream_hint: true,
                detail: details.join(" — "),
            }
        }
        Err(err) => {
            let message = message_from_error_body(err);
            let problem = classify_ma_text(&message);
            push_unique(&mut problems, problem);
            if problem == MaProblem::LocalhostRejected || problem == MaProblem::StreamHint {
                push_unique(&mut problems, MaProblem::StreamHint);
            }
            if !message.is_empty() {
                details.push(message);
            }
            TestExplanation {
                ok: false,
                show_stream_hint: problems.contains(&MaProblem::StreamHint)
                    || problems.contains(&MaProblem::LocalhostRejected),
                problems,
                detail: details.join(" — "),
            }
        }
    }
}

// ============================ Grouping ===================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupPlan {
    /// `players/cmd/group_many` via the relay or the command route.
    GroupMany {
        target_player: String,
        player_ids: Vec<String>,
    },
    /// Adjust one existing group or sync leader.
    SetMembers {
        player_id: String,
        add: Vec<String>,
        remove: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupPlanError {
    NeedTwo,
    MultipleGroups,
    Unchanged,
}

impl GroupPlan {
    pub fn action(&self) -> &'static str {
        match self {
            GroupPlan::GroupMany { .. } => "group_many",
            GroupPlan::SetMembers { .. } => "set_members",
        }
    }

    /// Player id the command is addressed to.
    pub fn player_id(&self) -> &str {
        match self {
            GroupPlan::GroupMany { target_player, .. } => target_player,
            GroupPlan::SetMembers { player_id, .. } => player_id,
        }
    }

    pub fn args(&self) -> Value {
        match self {
            GroupPlan::GroupMany {
                target_player,
                player_ids,
            } => json!({
                "player_ids": player_ids,
                "target_player": target_player,
            }),
            GroupPlan::SetMembers { add, remove, .. } => json!({
                "player_ids_to_add": add,
                "player_ids_to_remove": remove,
            }),
        }
    }
}

/// Turn a checkbox selection into one grouping command.
///
/// One selected group (or sync leader that already has members) is edited with
/// `set_members`. Several plain players become `group_many`, led by the first
/// selected id.
pub fn plan_group(players: &[MaPlayer], selected: &[String]) -> Result<GroupPlan, GroupPlanError> {
    if selected.len() < 2 {
        return Err(GroupPlanError::NeedTwo);
    }
    let groups: Vec<&MaPlayer> = players
        .iter()
        .filter(|p| selected.iter().any(|id| id == &p.player_id) && p.is_group())
        .collect();
    if groups.len() > 1 {
        return Err(GroupPlanError::MultipleGroups);
    }
    if groups.len() == 1 {
        let group = groups[0];
        let desired: Vec<&String> = selected
            .iter()
            .filter(|id| *id != &group.player_id)
            .collect();
        let add: Vec<String> = desired
            .iter()
            .filter(|id| !group.group_members.iter().any(|m| m == **id))
            .map(|id| (*id).clone())
            .collect();
        let remove: Vec<String> = group
            .group_members
            .iter()
            .filter(|id| !desired.iter().any(|d| *d == *id))
            .cloned()
            .collect();
        if add.is_empty() && remove.is_empty() {
            return Err(GroupPlanError::Unchanged);
        }
        return Ok(GroupPlan::SetMembers {
            player_id: group.player_id.clone(),
            add,
            remove,
        });
    }
    Ok(GroupPlan::GroupMany {
        target_player: selected[0].clone(),
        player_ids: selected.to_vec(),
    })
}

/// A now-playing row the bar can follow. Built from relay snapshots.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackCandidate {
    pub device_id: String,
    pub device_type: String,
    pub episode_id: i32,
    pub playing: bool,
    pub updated_at: i64,
}

impl From<&crate::requests::now_playing::NowPlayingDevice> for PlaybackCandidate {
    fn from(d: &crate::requests::now_playing::NowPlayingDevice) -> Self {
        PlaybackCandidate {
            device_id: d.device_id.clone(),
            device_type: d.device_type.clone(),
            episode_id: d.episode_id,
            playing: d.playing,
            updated_at: d.updated_at,
        }
    }
}

pub fn is_music_assistant(device_type: &str) -> bool {
    device_type.eq_ignore_ascii_case("music_assistant")
}

/// Music Assistant players belong on the speaker panel, not in the generic
/// "other devices" list. Every other now-playing device stays there.
pub fn include_in_other_devices(device_type: &str) -> bool {
    !is_music_assistant(device_type)
}

/// `Some(true)` shows pause, `Some(false)` shows play. `None` hides transport.
///
/// Playing and buffering show pause. Paused shows play. Idle, unknown, off,
/// unavailable, and a missing state still show play while an episode is loaded,
/// the relay says the speaker is playing, or this player was playing or paused
/// recently (`recent`). A 12 second poll that reports some other state must not
/// hide the buttons. Sonos reports idle for a Music Assistant pause and for a
/// sync member whose coordinator is playing.
pub fn transport_shown(
    player_state: &str,
    episode_loaded: bool,
    relay_playing: bool,
    recent: bool,
) -> Option<bool> {
    let state = player_state.trim();
    let playing = relay_playing
        || state.eq_ignore_ascii_case("playing")
        || state.eq_ignore_ascii_case("buffering");
    if playing {
        return Some(true);
    }
    if state.eq_ignore_ascii_case("paused") || episode_loaded || recent {
        return Some(false);
    }
    None
}

/// Playback state the transport buttons should follow. A synced speaker's own
/// state is often idle while the leader is playing.
pub fn playback_state_for<'a>(player: &'a MaPlayer, players: &'a [MaPlayer]) -> &'a str {
    if let Some(id) = player
        .synced_to
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        if let Some(leader) = players.iter().find(|p| p.player_id == id) {
            if !leader.state.trim().is_empty() {
                return leader.state.as_str();
            }
        }
    }
    player.state.as_str()
}

/// The bar follows a Music Assistant speaker when the bridge is on, this
/// browser is not playing, a speaker has an episode loaded, and this
/// browser did not start the episode it still has loaded more recently.
pub fn show_ma_now_playing(
    enabled: bool,
    local_playing: bool,
    local_loaded: bool,
    devices: &[PlaybackCandidate],
    focus: &PlaybackFocus,
) -> bool {
    enabled
        && !local_playing
        && !(local_loaded && *focus == PlaybackFocus::Local)
        && choose_playback(devices, focus.speaker_id()).is_some()
}

/// Device ids of speakers the relay reports playing an episode, sorted.
pub fn playing_speakers(devices: &[crate::requests::now_playing::NowPlayingDevice]) -> Vec<String> {
    let mut ids: Vec<String> = devices
        .iter()
        .filter(|d| is_music_assistant(&d.device_type) && d.episode_id != 0 && d.playing)
        .map(|d| d.device_id.clone())
        .collect();
    ids.sort();
    ids
}

/// A speaker playing in `now` that was not playing in `before`: it just started.
pub fn started_speaker<'a>(before: &[String], now: &'a [String]) -> Option<&'a str> {
    now.iter().find(|id| !before.contains(id)).map(String::as_str)
}

/// Which MA device the now-playing bar should show.
/// Prefers the speaker the user last targeted, then one that is playing,
/// then the freshest snapshot that has an episode loaded.
pub fn choose_playback<'a>(
    devices: &'a [PlaybackCandidate],
    focus_device_id: Option<&str>,
) -> Option<&'a PlaybackCandidate> {
    let mut matched: Vec<&PlaybackCandidate> = devices
        .iter()
        .filter(|d| is_music_assistant(&d.device_type) && d.episode_id != 0)
        .collect();
    if matched.is_empty() {
        return None;
    }
    if let Some(id) = focus_device_id {
        if let Some(found) = matched.iter().copied().find(|d| d.device_id == id) {
            return Some(found);
        }
    }
    if let Some(playing) = matched.iter().copied().find(|d| d.playing) {
        return Some(playing);
    }
    matched.sort_by_key(|d| d.updated_at);
    matched.pop()
}

/// A `next` the bridge accepted with nothing up next. The speaker keeps playing.
pub fn is_nothing_next(resp: &MaCommandResponse) -> bool {
    resp.detail == "nothing_next"
}

/// Items fetched for `episode_id`, or none when what is stored belongs to an
/// earlier episode. A late response must not label the next episode.
pub fn for_episode<T>(tagged: &(i32, std::rc::Rc<Vec<T>>), episode_id: i32) -> std::rc::Rc<Vec<T>> {
    if episode_id > 0 && tagged.0 == episode_id {
        tagged.1.clone()
    } else {
        std::rc::Rc::new(Vec::new())
    }
}

/// One entry of the speaker's sleep timer menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SleepChoice {
    Off,
    Minutes(u32),
    EndOfEpisode,
    Episodes(u32),
}

/// The sleep timer menu, the same choices as mobile.
pub const SLEEP_CHOICES: &[SleepChoice] = &[
    SleepChoice::Off,
    SleepChoice::Minutes(15),
    SleepChoice::Minutes(30),
    SleepChoice::Minutes(45),
    SleepChoice::Minutes(60),
    SleepChoice::Minutes(90),
    SleepChoice::EndOfEpisode,
    SleepChoice::Episodes(2),
    SleepChoice::Episodes(3),
];

/// Arguments of the `sleep_timer` command for a menu choice.
pub fn sleep_args(choice: SleepChoice) -> Value {
    match choice {
        SleepChoice::Off => json!({ "mode": "off" }),
        SleepChoice::Minutes(minutes) => json!({ "mode": "minutes", "minutes": minutes }),
        SleepChoice::EndOfEpisode => json!({ "mode": "end_of_episode" }),
        SleepChoice::Episodes(episodes) => json!({ "mode": "episodes", "episodes": episodes }),
    }
}

/// Seconds left on a minutes timer, never below 0. Episode timers have none.
pub fn sleep_remaining(
    timer: Option<crate::requests::now_playing::SleepTimer>,
    now: i64,
) -> Option<i64> {
    match timer {
        Some(crate::requests::now_playing::SleepTimer::Minutes { ends_at }) => {
            Some((ends_at - now).max(0))
        }
        _ => None,
    }
}

/// The sync leader a speaker plays under as a member: the one it is
/// `synced_to`, or a sync leader (a player, not a group player) that lists it
/// in `group_members`. Playing on the member would split it off, so the picker
/// offers play on the leader only.
pub fn member_of<'a>(player: &MaPlayer, players: &'a [MaPlayer]) -> Option<&'a MaPlayer> {
    if let Some(leader) = player
        .synced_to
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    {
        return players.iter().find(|p| p.player_id == leader);
    }
    players.iter().find(|p| {
        p.player_id != player.player_id
            && !p.kind.eq_ignore_ascii_case("group")
            && p.group_members.iter().any(|m| *m == player.player_id)
    })
}

/// Whether a speaker may take this browser's playback. Offline playback is a
/// file only this device can reach, so it stays here.
pub fn send_to_speaker_allowed(local_playing: bool, offline: bool) -> bool {
    local_playing && !offline
}

/// Width of the bar's thin progress line, in percent. An unknown duration
/// shows no progress rather than a full bar.
pub fn progress_percent(shown: f64, duration: f64) -> f64 {
    if !(duration.is_finite() && duration > 0.0 && shown.is_finite()) {
        return 0.0;
    }
    (shown / duration * 100.0).clamp(0.0, 100.0)
}

/// Keyboard shortcut for the speaker in the now-playing bar, the same keys as
/// local web: Space plays or pauses, the arrow keys skip 15 seconds. A key held
/// with Alt, Ctrl or Meta is left to the browser.
pub fn speaker_key_command(
    key: &str,
    playing: bool,
    modified: bool,
) -> Option<(&'static str, Value)> {
    if modified {
        return None;
    }
    match key {
        " " => Some((if playing { "pause" } else { "play" }, json!({}))),
        "ArrowRight" => Some(("skip_forward", json!({ "seconds": 15 }))),
        "ArrowLeft" => Some(("skip_back", json!({ "seconds": 15 }))),
        _ => None,
    }
}

/// Playlist a speaker should continue with: only when the episode sent is the
/// one loaded locally, because that is the episode the playlist context belongs to.
pub fn speaker_playlist(
    local: (Option<i32>, Option<bool>),
    sent: (i32, bool),
    playlist_id: Option<i32>,
) -> Option<i32> {
    (local == (Some(sent.0), Some(sent.1))).then_some(playlist_id).flatten()
}

/// Advance a relay position between snapshots. Stops extrapolating once the
/// snapshot is older than 30 seconds so a dropped socket does not run away.
pub fn displayed_position(
    position_sec: f64,
    playing: bool,
    speed: f64,
    updated_at: i64,
    now: i64,
) -> f64 {
    let base = if position_sec.is_finite() {
        position_sec.max(0.0)
    } else {
        0.0
    };
    if !playing || updated_at <= 0 {
        return base;
    }
    let elapsed = (now - updated_at) as f64;
    if elapsed < 0.0 || elapsed > 30.0 {
        return base;
    }
    let rate = if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    };
    base + elapsed * rate
}

/// Saved position has to be more than a few seconds in, and the episode must
/// still be unfinished, before the speaker picker asks resume or start over.
pub const RESUME_MIN_SEC: f64 = 5.0;

#[derive(Debug, Clone, PartialEq)]
pub struct ResumeOffer {
    pub resume_sec: f64,
    pub start_over_sec: f64,
}

/// Where a picked episode should start on a speaker.
///
/// `offer` is set only when the saved position is real and the episode is not
/// complete. Resume is `max(saved, start_skip)`. Start over is `start_skip`.
/// Otherwise `position_sec` is `start_skip` (or 0) and nothing is asked.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerStart {
    pub offer: Option<ResumeOffer>,
    pub position_sec: f64,
}

pub fn speaker_start(saved_sec: f64, completed: bool, start_skip: i64) -> SpeakerStart {
    let start = start_skip.max(0) as f64;
    let saved = if saved_sec.is_finite() {
        saved_sec.max(0.0)
    } else {
        0.0
    };
    if completed || saved < RESUME_MIN_SEC {
        return SpeakerStart {
            offer: None,
            position_sec: start,
        };
    }
    let resume = saved.max(start);
    SpeakerStart {
        offer: Some(ResumeOffer {
            resume_sec: resume,
            start_over_sec: start,
        }),
        position_sec: resume,
    }
}

/// Arguments of the `play_episode` command. `position_sec` is left out when the
/// client does not know the position, so the server resumes from the saved one.
/// An explicit `0` is honoured and starts the episode from the beginning.
pub fn play_episode_args(episode_id: i32, is_youtube: bool, position_sec: Option<f64>) -> Value {
    let mut args = json!({ "episode_id": episode_id, "is_youtube": is_youtube });
    if let Some(position_sec) = position_sec {
        args["position_sec"] = json!(position_sec);
    }
    args
}

pub fn format_clock(secs: f64) -> String {
    if !secs.is_finite() || secs < 0.0 {
        return "00:00".to_string();
    }
    let total = secs as i64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn player_name<'a>(players: &'a [MaPlayer], player_id: &'a str) -> &'a str {
    players
        .iter()
        .find(|p| p.player_id == player_id)
        .map(|p| p.name.as_str())
        .filter(|n| !n.is_empty())
        .unwrap_or(player_id)
}

// ============================ HTTP =======================================

fn nonempty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

async fn error_from_response(response: gloo_net::http::Response) -> Error {
    let status = response.status();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| String::from("Failed to read error message"));
    let message = message_from_error_body(&body);
    Error::msg(format!("{status}: {message}"))
}

pub async fn get_settings(
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<MaSettings, Error> {
    let url = format!("{server_name}/api/data/music_assistant/settings?user_id={user_id}");
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

pub async fn save_settings(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    enabled: bool,
    server_url: &str,
    external_base_url: &str,
    token: &str,
    clear_token: bool,
) -> Result<MaSettings, Error> {
    let url = format!("{server_name}/api/data/music_assistant/settings");
    let body = json!({
        "user_id": user_id,
        "enabled": enabled,
        "server_url": nonempty(server_url),
        "external_base_url": nonempty(external_base_url),
        "token": nonempty(token),
        "clear_token": clear_token,
    });
    let response = Request::put(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())?
        .send()
        .await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

pub async fn test_connection(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    server_url: &str,
    external_base_url: &str,
    token: &str,
) -> Result<MaTestReport, Error> {
    let url = format!("{server_name}/api/data/music_assistant/test");
    let body = json!({
        "user_id": user_id,
        "server_url": nonempty(server_url),
        "external_base_url": nonempty(external_base_url),
        "token": nonempty(token),
    });
    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())?
        .send()
        .await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

pub async fn list_players(
    server_name: &str,
    api_key: &str,
    user_id: i32,
) -> Result<MaPlayersResponse, Error> {
    let url = format!("{server_name}/api/data/music_assistant/players?user_id={user_id}");
    let response = Request::get(&url).header("Api-Key", api_key).send().await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

pub async fn send_player_command(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    player_id: &str,
    action: &str,
    args: Value,
) -> Result<MaCommandResponse, Error> {
    let url = format!("{server_name}/api/data/music_assistant/command");
    let body = json!({
        "user_id": user_id,
        "player_id": player_id,
        "action": action,
        "args": args,
    });
    let response = Request::post(&url)
        .header("Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(body.to_string())?
        .send()
        .await?;
    if response.ok() {
        Ok(response.json().await?)
    } else {
        Err(error_from_response(response).await)
    }
}

/// Reload settings, and players when the bridge is on. Safe to call with no
/// session: it does nothing.
pub fn refresh_music_assistant() {
    let app = Dispatch::<crate::components::context::AppState>::global().get();
    let Some(auth) = app.auth_details.as_ref() else {
        return;
    };
    let Some(api_key) = auth.api_key.clone() else {
        return;
    };
    let Some(user) = app.user_details.as_ref() else {
        return;
    };
    let server = auth.server_name.clone();
    let user_id = user.UserID;
    wasm_bindgen_futures::spawn_local(async move {
        match get_settings(&server, &api_key, user_id).await {
            Ok(view) => {
                let enabled = view.enabled;
                apply_settings(&view);
                if enabled {
                    if let Ok(list) = list_players(&server, &api_key, user_id).await {
                        apply_players(list);
                    }
                }
            }
            Err(err) => {
                web_sys::console::warn_1(&format!("music assistant settings: {err}").into());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    fn sleep_menu_sends_the_server_modes_and_reads_its_snapshot() {
        use crate::requests::now_playing::{NowPlayingDevice, SleepTimer};
        assert_eq!(sleep_args(SleepChoice::Off), json!({ "mode": "off" }));
        assert_eq!(
            sleep_args(SleepChoice::Minutes(30)),
            json!({ "mode": "minutes", "minutes": 30 })
        );
        assert_eq!(
            sleep_args(SleepChoice::EndOfEpisode),
            json!({ "mode": "end_of_episode" })
        );
        assert_eq!(
            sleep_args(SleepChoice::Episodes(2)),
            json!({ "mode": "episodes", "episodes": 2 })
        );
        assert_eq!(SLEEP_CHOICES.first(), Some(&SleepChoice::Off));

        let device: NowPlayingDevice = serde_json::from_value(json!({
            "device_id": "ma:2:x",
            "sleep_timer": { "mode": "minutes", "ends_at": 1_000 }
        }))
        .unwrap();
        assert_eq!(device.sleep_timer, Some(SleepTimer::Minutes { ends_at: 1_000 }));
        assert_eq!(sleep_remaining(device.sleep_timer, 900), Some(100));
        assert_eq!(sleep_remaining(device.sleep_timer, 1_200), Some(0));
        let episodes: NowPlayingDevice =
            serde_json::from_value(json!({ "sleep_timer": { "mode": "episodes", "left": 2 } })).unwrap();
        assert_eq!(episodes.sleep_timer, Some(SleepTimer::Episodes { left: 2 }));
        assert_eq!(sleep_remaining(episodes.sleep_timer, 0), None);
        let none: NowPlayingDevice = serde_json::from_value(json!({})).unwrap();
        assert_eq!(none.sleep_timer, None);
    }

    #[wasm_bindgen_test]
    fn speaker_extras_show_only_for_the_episode_they_were_fetched_for() {
        let tagged = (22, std::rc::Rc::new(vec!["chapter"]));
        assert_eq!(*for_episode(&tagged, 22), vec!["chapter"]);
        assert!(for_episode(&tagged, 33).is_empty());
        assert!(for_episode(&(0, std::rc::Rc::new(vec!["x"])), 0).is_empty());
    }

    #[wasm_bindgen_test]
    fn sync_members_point_at_their_leader_and_groups_do_not() {
        let player = |id: &str, kind: &str, synced: Option<&str>, members: &[&str]| MaPlayer {
            player_id: id.into(),
            kind: kind.into(),
            synced_to: synced.map(Into::into),
            group_members: members.iter().map(|m| m.to_string()).collect(),
            ..Default::default()
        };
        let players = vec![
            player("living", "player", None, &["bed", "kitchen"]),
            player("bed", "player", Some("living"), &[]),
            player("kitchen", "player", None, &[]),
            player("party", "group", None, &["garage"]),
            player("garage", "player", None, &[]),
            player("office", "player", None, &[]),
        ];
        let leader = |id: &str| {
            let p = players.iter().find(|p| p.player_id == id).unwrap();
            member_of(p, &players).map(|l| l.player_id.clone())
        };
        assert_eq!(leader("bed").as_deref(), Some("living"));
        // Listed by a sync leader without synced_to yet.
        assert_eq!(leader("kitchen").as_deref(), Some("living"));
        // A group player's member can still play on its own.
        assert_eq!(leader("garage"), None);
        assert_eq!(leader("living"), None);
        assert_eq!(leader("office"), None);

        assert!(send_to_speaker_allowed(true, false));
        assert!(!send_to_speaker_allowed(true, true));
        assert!(!send_to_speaker_allowed(false, false));
    }

    #[wasm_bindgen_test]
    fn progress_line_is_clamped_and_empty_without_a_duration() {
        assert_eq!(progress_percent(30.0, 120.0), 25.0);
        assert_eq!(progress_percent(200.0, 120.0), 100.0);
        assert_eq!(progress_percent(-5.0, 120.0), 0.0);
        assert_eq!(progress_percent(30.0, 0.0), 0.0);
        assert_eq!(progress_percent(f64::NAN, 120.0), 0.0);
        assert_eq!(progress_percent(30.0, f64::INFINITY), 0.0);
    }

    #[wasm_bindgen_test]
    fn speaker_keys_match_local_web() {
        assert_eq!(speaker_key_command(" ", true, false), Some(("pause", json!({}))));
        assert_eq!(speaker_key_command(" ", false, false), Some(("play", json!({}))));
        assert_eq!(
            speaker_key_command("ArrowRight", true, false),
            Some(("skip_forward", json!({ "seconds": 15 })))
        );
        assert_eq!(
            speaker_key_command("ArrowLeft", false, false),
            Some(("skip_back", json!({ "seconds": 15 })))
        );
        assert_eq!(speaker_key_command("ArrowLeft", true, true), None);
        assert_eq!(speaker_key_command(" ", true, true), None);
        assert_eq!(speaker_key_command("ArrowUp", true, false), None);
        assert_eq!(speaker_key_command(">", true, false), None);
    }

    #[wasm_bindgen_test]
    fn nothing_next_is_recognized_only_from_its_detail() {
        let resp = |detail: &str| MaCommandResponse {
            ok: true,
            detail: detail.into(),
            warnings: Vec::new(),
        };
        assert!(is_nothing_next(&resp("nothing_next")));
        assert!(!is_nothing_next(&resp("ok")));
    }

    #[wasm_bindgen_test]
    fn playlist_follows_only_the_locally_loaded_episode() {
        assert_eq!(speaker_playlist((Some(4), Some(false)), (4, false), Some(9)), Some(9));
        assert_eq!(speaker_playlist((Some(4), Some(false)), (5, false), Some(9)), None);
        assert_eq!(speaker_playlist((Some(4), Some(true)), (4, false), Some(9)), None);
        assert_eq!(speaker_playlist((None, None), (4, false), Some(9)), None);
        assert_eq!(speaker_playlist((Some(4), Some(false)), (4, false), None), None);
    }

    fn player(id: &str, kind: &str, members: &[&str]) -> MaPlayer {
        MaPlayer {
            player_id: id.into(),
            device_id: format!("ma:1:{id}"),
            name: id.into(),
            kind: kind.into(),
            available: true,
            state: "idle".into(),
            volume_level: Some(20.0),
            volume_muted: false,
            group_members: members.iter().map(|m| (*m).to_string()).collect(),
            synced_to: None,
            can_group_with: vec!["living".into()],
            queue_id: id.into(),
        }
    }

    fn candidate(id: &str, episode: i32, playing: bool, updated: i64) -> PlaybackCandidate {
        PlaybackCandidate {
            device_id: id.into(),
            device_type: "music_assistant".into(),
            episode_id: episode,
            playing,
            updated_at: updated,
        }
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn error_body_uses_message_and_leaves_plain_text() {
        let body = r#"{"error":"Bad request","message":"Bad request: external base URL must be a LAN or public host Music Assistant can reach, not localhost","status_code":400}"#;
        assert!(message_from_error_body(body).contains("not localhost"));
        assert_eq!(
            message_from_error_body("timed out connecting"),
            "timed out connecting"
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn localhost_external_urls_are_rejected() {
        assert!(external_base_is_rejected("http://localhost:8040"));
        assert!(external_base_is_rejected("http://127.0.0.1:8040"));
        assert!(external_base_is_rejected("http://[::1]:8040"));
        assert!(external_base_is_rejected("http://app.localhost"));
        assert!(external_base_is_rejected("http://169.254.1.1"));
        assert!(external_base_is_rejected("http://[fe80::1]/"));
        assert!(!external_base_is_rejected("http://192.168.1.10:8040"));
        assert!(!external_base_is_rejected("https://pods.example.com"));
        assert!(!external_base_is_rejected(""));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn classify_known_failures() {
        assert_eq!(
            classify_ma_text("external base URL must be a LAN or public host Music Assistant can reach, not localhost"),
            MaProblem::LocalhostRejected
        );
        assert_eq!(
            classify_ma_text("could not connect: connection refused"),
            MaProblem::Unreachable
        );
        assert_eq!(
            classify_ma_text("timed out connecting to Music Assistant"),
            MaProblem::Unreachable
        );
        assert_eq!(
            classify_ma_text("Music Assistant cannot fetch the stream URL"),
            MaProblem::StreamHint
        );
        assert_eq!(
            classify_ma_text("a Music Assistant token is required to enable the connection"),
            MaProblem::NotConfigured
        );
        assert_eq!(
            classify_ma_text("Invalid or expired token"),
            MaProblem::AuthFailed
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn explain_test_surfaces_localhost_and_unreachable() {
        let probe = MaTestReport {
            ok: false,
            detail: "could not connect: connection refused".into(),
            server_version: None,
            schema_version: None,
            player_count: 0,
            warning: None,
        };
        let explained = explain_test("http://localhost:8040", Ok(&probe));
        assert!(!explained.ok);
        assert!(explained.problems.contains(&MaProblem::LocalhostRejected));
        assert!(explained.problems.contains(&MaProblem::Unreachable));
        assert!(explained.show_stream_hint);

        let server_err = r#"{"message":"Bad request: external base URL must be a LAN or public host Music Assistant can reach, not localhost"}"#;
        let explained = explain_test("http://192.168.1.10:8040", Err(server_err));
        assert!(explained.problems.contains(&MaProblem::LocalhostRejected));
        assert!(explained.show_stream_hint);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn explain_test_success_keeps_the_stream_hint() {
        let probe = MaTestReport {
            ok: true,
            detail: "connected, 2 player(s)".into(),
            server_version: Some("2.10.3".into()),
            schema_version: Some(80),
            player_count: 2,
            warning: None,
        };
        let explained = explain_test("http://192.168.1.10:8040", Ok(&probe));
        assert!(explained.ok);
        assert!(explained.problems.is_empty());
        assert!(explained.show_stream_hint);
        assert!(explained.detail.contains("2 player"));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn plan_group_creates_and_edits() {
        let players = vec![
            player("kitchen", "player", &[]),
            player("living", "player", &[]),
            player("den", "player", &[]),
        ];
        let plan = plan_group(&players, &["kitchen".into(), "living".into()]).unwrap();
        assert_eq!(
            plan,
            GroupPlan::GroupMany {
                target_player: "kitchen".into(),
                player_ids: vec!["kitchen".into(), "living".into()],
            }
        );
        assert_eq!(plan.action(), "group_many");
        assert_eq!(plan.player_id(), "kitchen");

        let grouped = vec![
            player("kitchen", "player", &["living"]),
            player("living", "player", &[]),
            player("den", "player", &[]),
        ];
        let plan =
            plan_group(&grouped, &["kitchen".into(), "living".into(), "den".into()]).unwrap();
        assert_eq!(
            plan,
            GroupPlan::SetMembers {
                player_id: "kitchen".into(),
                add: vec!["den".into()],
                remove: vec![],
            }
        );
        assert!(plan_group(&grouped, &["kitchen".into(), "living".into()]).is_err());
        assert_eq!(
            plan_group(&players, &["kitchen".into()]),
            Err(GroupPlanError::NeedTwo)
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn choose_playback_prefers_focus_then_playing() {
        let devices = vec![
            candidate("ma:1:kitchen", 4, false, 10),
            candidate("ma:1:living", 9, true, 5),
            candidate("web", 3, true, 99),
            candidate("ma:1:idle", 0, false, 100),
        ];
        let chosen = choose_playback(&devices, Some("ma:1:kitchen")).unwrap();
        assert_eq!(chosen.device_id, "ma:1:kitchen");
        let chosen = choose_playback(&devices, None).unwrap();
        assert_eq!(chosen.device_id, "ma:1:living");
        let paused = vec![
            candidate("ma:1:kitchen", 4, false, 10),
            candidate("ma:1:den", 8, false, 20),
        ];
        assert_eq!(
            choose_playback(&paused, None).unwrap().device_id,
            "ma:1:den"
        );
        assert!(choose_playback(&[], None).is_none());
        assert!(show_ma_now_playing(true, false, false, &devices, &PlaybackFocus::Unset));
        assert!(!show_ma_now_playing(true, true, false, &devices, &PlaybackFocus::Unset));
        assert!(!show_ma_now_playing(false, false, false, &devices, &PlaybackFocus::Unset));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn bar_follows_the_last_started_player() {
        // A stopped speaker keeps its episode loaded.
        let stopped = vec![candidate("ma:1:kitchen", 4, false, 10)];
        let kitchen = PlaybackFocus::Speaker("ma:1:kitchen".into());
        // Local started after the speaker and is now paused: stay local.
        assert!(!show_ma_now_playing(true, false, true, &stopped, &PlaybackFocus::Local));
        assert!(show_ma_now_playing(true, false, false, &stopped, &PlaybackFocus::Local));
        assert!(show_ma_now_playing(true, false, true, &stopped, &kitchen));
        assert!(show_ma_now_playing(true, false, true, &stopped, &PlaybackFocus::Unset));
        assert!(!show_ma_now_playing(true, true, true, &stopped, &kitchen));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn speaker_start_is_a_new_playing_id() {
        use crate::requests::now_playing::NowPlayingDevice;
        let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(started_speaker(&[], &ids(&["ma:1:kitchen"])), Some("ma:1:kitchen"));
        assert_eq!(
            started_speaker(&ids(&["ma:1:kitchen"]), &ids(&["ma:1:kitchen"])),
            None
        );
        assert_eq!(
            started_speaker(&ids(&["ma:1:kitchen"]), &ids(&["ma:1:den", "ma:1:kitchen"])),
            Some("ma:1:den")
        );
        assert_eq!(
            started_speaker(&ids(&["ma:1:den", "ma:1:kitchen"]), &ids(&["ma:1:kitchen"])),
            None
        );
        assert_eq!(started_speaker(&ids(&["ma:1:kitchen"]), &[]), None);

        let device = |id: &str, kind: &str, episode: i32, playing: bool| NowPlayingDevice {
            device_id: id.into(),
            device_type: kind.into(),
            episode_id: episode,
            playing,
            ..Default::default()
        };
        let mut devices = vec![
            device("ma:1:den", "music_assistant", 3, true),
            device("ma:1:bed", "music_assistant", 0, true),
            device("ma:1:attic", "music_assistant", 5, false),
            device("web-1", "Web", 2, true),
        ];
        assert_eq!(playing_speakers(&devices), ids(&["ma:1:den"]));
        devices.push(device("ma:1:bath", "music_assistant", 7, true));
        assert_eq!(playing_speakers(&devices), ids(&["ma:1:bath", "ma:1:den"]));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn transport_stays_when_sonos_reports_idle() {
        assert_eq!(transport_shown("playing", true, false, false), Some(true));
        assert_eq!(transport_shown("paused", true, false, false), Some(false));
        assert_eq!(transport_shown("idle", true, false, false), Some(false));
        assert_eq!(transport_shown("idle", true, true, false), Some(true));
        assert_eq!(transport_shown("", true, false, false), Some(false));
        assert_eq!(transport_shown("idle", false, false, false), None);
        assert_eq!(transport_shown("unknown", false, true, false), Some(true));
        assert_eq!(transport_shown("idle", false, false, true), Some(false));
        assert_eq!(transport_shown("mystery", false, false, true), Some(false));
        assert!(transport_is_recent(Some(1_000), 1_012));
        assert!(transport_is_recent(Some(1_000), 1_060));
        assert!(!transport_is_recent(Some(1_000), 1_061));
        assert!(!transport_is_recent(None, 1_000));
        assert!(transport_state_live("Playing"));
        assert!(transport_state_live("paused"));
        assert!(!transport_state_live("idle"));
        assert!(!include_in_other_devices("music_assistant"));
        assert!(!include_in_other_devices("Music_Assistant"));
        assert!(include_in_other_devices("Web"));
        assert!(include_in_other_devices("iOS"));
        assert!(include_in_other_devices("Android"));
        assert!(include_in_other_devices("Firewood"));
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn synced_speaker_follows_the_leader_state() {
        let mut member = player("bed", "player", &[]);
        member.state = "idle".into();
        member.synced_to = Some("living".into());
        let mut leader = player("living", "player", &["bed"]);
        leader.state = "playing".into();
        assert_eq!(
            playback_state_for(&member, &[member.clone(), leader.clone()]),
            "playing"
        );
        assert_eq!(
            playback_state_for(&leader, &[member, leader.clone()]),
            "playing"
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn displayed_position_follows_speed_and_stops_when_stale() {
        assert_eq!(displayed_position(10.0, true, 2.0, 100, 105), 20.0);
        assert_eq!(displayed_position(10.0, false, 2.0, 100, 105), 10.0);
        assert_eq!(displayed_position(10.0, true, 1.0, 100, 140), 10.0);
        assert_eq!(displayed_position(10.0, true, 1.0, 0, 140), 10.0);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn speaker_start_asks_only_for_a_real_unfinished_position() {
        let fresh = speaker_start(0.0, false, 15);
        assert!(fresh.offer.is_none());
        assert_eq!(fresh.position_sec, 15.0);

        let finished = speaker_start(3400.0, true, 15);
        assert!(finished.offer.is_none());
        assert_eq!(finished.position_sec, 15.0);

        let early = speaker_start(4.0, false, 0);
        assert!(early.offer.is_none());
        assert_eq!(early.position_sec, 0.0);

        let resume = speaker_start(90.0, false, 30).offer.unwrap();
        assert_eq!(resume.resume_sec, 90.0);
        assert_eq!(resume.start_over_sec, 30.0);

        let intro = speaker_start(10.0, false, 45).offer.unwrap();
        assert_eq!(intro.resume_sec, 45.0);
        assert_eq!(intro.start_over_sec, 45.0);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn players_json_parses_groups() {
        let raw = r#"{
            "enabled": true,
            "connected": true,
            "players": [{
                "player_id": "kitchen",
                "device_id": "ma:1:kitchen",
                "name": "Kitchen",
                "kind": "group",
                "available": true,
                "state": "playing",
                "volume_level": 20,
                "volume_muted": false,
                "group_members": ["living"],
                "synced_to": null,
                "can_group_with": ["den"],
                "queue_id": "kitchen"
            }]
        }"#;
        let parsed: MaPlayersResponse = serde_json::from_str(raw).unwrap();
        assert!(parsed.players[0].is_group());
        assert_eq!(parsed.players[0].group_members, vec!["living".to_string()]);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn play_episode_args_omit_an_unknown_position() {
        assert!(play_episode_args(4, false, None).get("position_sec").is_none());
        assert_eq!(play_episode_args(4, true, Some(0.0))["position_sec"], 0.0);
    }
}
