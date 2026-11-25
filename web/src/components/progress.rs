//! Listening progress: where the user is in what they play, and how long they played it.
//!
//! One recorder watches the player for the whole page. It reports while something is playing
//! (every [`REPORT_EVERY_MS`]) and whenever the position stops following the clock: a pause, a
//! seek, the end, another episode, the tab going away. Nothing is sent while paused.
//!
//! Every report is first written to `localStorage`, one entry per episode, and removed only
//! when the server has it, so a closed tab or a dropped connection loses nothing. Each entry
//! carries the time it was observed; the server lets the newest observation of an episode win,
//! whichever device made it.

use std::cell::{Cell, RefCell};

use gloo_net::http::Request;
use js_sys::{Function, Object, Reflect};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use yewdux::prelude::*;

use crate::components::context::{AppState, UIState};
use crate::requests::pod_req::{
    call_get_episode_metadata, call_increment_listen_time, call_mark_episode_completed, call_record_listen_duration, EpisodeRequest,
    MarkEpisodeCompletedRequest, RecordListenDurationRequest,
};

/// How often a playing episode's position is reported.
pub const REPORT_EVERY_MS: f64 = 30_000.0;
/// The position moving this far from where the clock puts it is a seek.
const SEEK_SECONDS: f64 = 3.0;
/// A longer gap between two looks at the player means the page was asleep, not playing.
const MAX_TICK_MS: f64 = 120_000.0;
/// A saved position this close to the end starts the episode over.
const AT_END_SECONDS: f64 = 2.0;
const STORAGE_KEY: &str = "pinepods_progress_pending";

// ---------------------------------------------------------------------------------------------
// Logic (no browser)
// ---------------------------------------------------------------------------------------------

/// One episode's unreported progress.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Update {
    pub episode_id: i32,
    #[serde(default)]
    pub is_youtube: bool,
    /// Seconds into the episode.
    pub position: f64,
    /// When the position was observed, in unix milliseconds on this device's clock.
    pub at: f64,
    /// Seconds played since the last report.
    #[serde(default)]
    pub listened: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default)]
    pub finished: bool,
    /// Episode time gained by playing faster than 1x during `listened` (negative when slower).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub speed_saved: f64,
    /// Seconds of silence skipped.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub silence_skipped: f64,
    /// Seconds of ads skipped.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub ads_skipped: f64,
    /// Played from a file on this device.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub downloaded: bool,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl Update {
    /// The played and saved time, taken out of a later report of the same episode.
    fn take_counts(&mut self, sent: &Update) {
        self.listened = (self.listened - sent.listened).max(0.0);
        self.speed_saved -= sent.speed_saved;
        self.silence_skipped = (self.silence_skipped - sent.silence_skipped).max(0.0);
        self.ads_skipped = (self.ads_skipped - sent.ads_skipped).max(0.0);
    }

    fn clear_counts(&mut self) {
        self.listened = 0.0;
        self.speed_saved = 0.0;
        self.silence_skipped = 0.0;
        self.ads_skipped = 0.0;
    }
}

/// Unreported progress, one entry per episode.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Pending(pub Vec<Update>);

impl Pending {
    pub fn get(&self, episode_id: i32, is_youtube: bool) -> Option<&Update> {
        self.0.iter().find(|u| u.episode_id == episode_id && u.is_youtube == is_youtube)
    }

    /// Fold `update` into the episode's entry: the newer position, the played time added up.
    pub fn merge(&mut self, update: Update) {
        match self.0.iter_mut().find(|u| u.episode_id == update.episode_id && u.is_youtube == update.is_youtube) {
            Some(entry) => {
                entry.listened += update.listened;
                entry.speed_saved += update.speed_saved;
                entry.silence_skipped += update.silence_skipped;
                entry.ads_skipped += update.ads_skipped;
                if update.at >= entry.at {
                    entry.position = update.position;
                    entry.at = update.at;
                    entry.finished = update.finished;
                    entry.duration = update.duration.or(entry.duration);
                    entry.downloaded = update.downloaded;
                }
            }
            None => self.0.push(update),
        }
    }

    /// The server has `sent`. Entries unchanged since are dropped; one that moved on while the
    /// request was in flight stays, less the played time already delivered.
    pub fn settle(&mut self, sent: &[Update]) {
        for s in sent {
            if let Some(i) = self.0.iter().position(|u| u.episode_id == s.episode_id && u.is_youtube == s.is_youtube) {
                if self.0[i].at <= s.at {
                    self.0.remove(i);
                } else {
                    self.0[i].take_counts(s);
                }
            }
        }
    }
}

/// What the player is doing at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub episode_id: i32,
    pub is_youtube: bool,
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    pub rate: f64,
    /// Playing a file on this device.
    pub downloaded: bool,
}

/// Turns looks at the player into reports.
#[derive(Debug, Default)]
pub struct Tracker {
    last: Option<Sample>,
    last_tick: f64,
    last_report: f64,
    reported_position: f64,
    listened: f64,
    speed_saved: f64,
    silence_skipped: f64,
    ads_skipped: f64,
}

/// What an automatic skip jumped over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skipped {
    Silence,
    Ad,
}

impl Tracker {
    fn report(&mut self, sample: &Sample, now: f64, finished: bool) -> Update {
        self.last_report = now;
        self.reported_position = sample.position;
        Update {
            episode_id: sample.episode_id,
            is_youtube: sample.is_youtube,
            position: sample.position,
            at: now,
            listened: std::mem::take(&mut self.listened),
            duration: (sample.duration.is_finite() && sample.duration >= 1.0).then_some(sample.duration),
            finished,
            speed_saved: std::mem::take(&mut self.speed_saved),
            silence_skipped: std::mem::take(&mut self.silence_skipped),
            ads_skipped: std::mem::take(&mut self.ads_skipped),
            downloaded: sample.downloaded,
        }
    }

    /// Whether the tracked episode has anything the server hasn't been told.
    fn unreported(&self, sample: &Sample) -> bool {
        self.listened >= 1.0 || self.silence_skipped + self.ads_skipped >= 1.0 || (sample.position - self.reported_position).abs() >= 1.0
    }

    /// The player skipped `seconds` of silence or ads in the tracked episode.
    pub fn skipped(&mut self, kind: Skipped, seconds: f64) {
        if self.last.is_none() || !seconds.is_finite() || seconds <= 0.0 {
            return;
        }
        match kind {
            Skipped::Silence => self.silence_skipped += seconds,
            Skipped::Ad => self.ads_skipped += seconds,
        }
    }

    /// Look at the player (`None`: nothing loaded) at `now` (unix ms). Returns what to report.
    pub fn observe(&mut self, now: f64, sample: Option<Sample>) -> Vec<Update> {
        let mut out = Vec::new();
        let previous = self.last;
        let same = match (&previous, &sample) {
            (Some(p), Some(s)) => p.episode_id == s.episode_id && p.is_youtube == s.is_youtube,
            _ => false,
        };
        if !same {
            // The episode changed or was closed: where the old one was left.
            if let Some(p) = previous.filter(|p| self.unreported(p)) {
                out.push(self.report(&p, now, false));
            }
            self.listened = 0.0;
            self.speed_saved = 0.0;
            self.silence_skipped = 0.0;
            self.ads_skipped = 0.0;
            self.last = sample;
            self.last_tick = now;
            // Nothing is reported for the start itself: the starting position is what some
            // device last saved (or a guess, offline), not something observed here.
            self.last_report = now;
            self.reported_position = sample.map_or(0.0, |s| s.position);
            return out;
        }
        let (previous, sample) = (previous.unwrap(), sample.unwrap());
        let elapsed = (now - self.last_tick).max(0.0);
        let awake = elapsed <= MAX_TICK_MS;
        let mut expected = previous.position;
        if previous.playing && awake {
            let rate = if previous.rate > 0.0 { previous.rate } else { 1.0 };
            self.listened += elapsed / 1000.0;
            self.speed_saved += elapsed / 1000.0 * (rate - 1.0);
            expected += elapsed / 1000.0 * rate;
        }
        let seeked = awake && (sample.position - expected).abs() > SEEK_SECONDS;
        let paused = previous.playing && !sample.playing;
        let due = sample.playing && now - self.last_report >= REPORT_EVERY_MS;
        self.last = Some(sample);
        self.last_tick = now;
        if seeked || paused || due {
            out.push(self.report(&sample, now, false));
        }
        out
    }

    /// The episode played to its end.
    pub fn finish(&mut self, now: f64) -> Option<Update> {
        let mut sample = self.last?;
        if sample.duration.is_finite() && sample.duration > 0.0 {
            sample.position = sample.duration;
        }
        sample.playing = false;
        self.last = Some(sample);
        self.last_tick = now;
        Some(self.report(&sample, now, true))
    }

    /// The page is going away: whatever hasn't been reported.
    pub fn leave(&mut self, now: f64) -> Option<Update> {
        let sample = self.last?;
        self.unreported(&sample).then(|| self.report(&sample, now, false))
    }
}

/// Where to start an episode saved at `position`: there, or from the top if that is the end.
pub fn resume_from(position: f64, duration: f64) -> f64 {
    if !position.is_finite() || position <= 0.0 {
        return 0.0;
    }
    if duration.is_finite() && duration > 0.0 && position >= duration - AT_END_SECONDS {
        return 0.0;
    }
    position
}

/// True when playing a file downloaded to this computer (desktop app).
pub fn is_local_file(src: &str) -> bool {
    crate::desktop::is_local_media(src)
}

/// The request body for `updates`, sent at `now` (unix ms).
pub fn request_body(updates: &[Update], now: f64) -> String {
    serde_json::json!({ "sent_at": now as i64, "client": crate::desktop::client_name(), "updates": updates.iter().map(|u| {
        let mut v = serde_json::to_value(u).unwrap_or_default();
        v["at"] = serde_json::json!(u.at as i64);
        v
    }).collect::<Vec<_>>() })
    .to_string()
}

// ---------------------------------------------------------------------------------------------
// Browser
// ---------------------------------------------------------------------------------------------

thread_local! {
    static TRACKER: RefCell<Tracker> = RefCell::new(Tracker::default());
    static STARTED: Cell<bool> = const { Cell::new(false) };
    static TICKS: Cell<u32> = const { Cell::new(0) };
    static SENDING: Cell<bool> = const { Cell::new(false) };
    static SEND_AGAIN: Cell<bool> = const { Cell::new(false) };
    /// The server predates `/progress`; use the endpoints it has.
    static OLD_SERVER: Cell<bool> = const { Cell::new(false) };
    /// Played seconds not yet a whole minute, for a server that counts minutes.
    static OLD_SERVER_SECONDS: Cell<f64> = const { Cell::new(0.0) };
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn load() -> Pending {
    storage()
        .and_then(|s| s.get_item(STORAGE_KEY).ok()?)
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn save(pending: &Pending) {
    let Some(storage) = storage() else { return };
    if pending.0.is_empty() {
        let _ = storage.remove_item(STORAGE_KEY);
    } else if let Ok(text) = serde_json::to_string(pending) {
        let _ = storage.set_item(STORAGE_KEY, &text);
    }
}

fn record(updates: Vec<Update>) {
    if updates.is_empty() {
        return;
    }
    let mut pending = load();
    for update in updates {
        pending.merge(update);
    }
    save(&pending);
}

fn sample() -> Option<Sample> {
    let state = Dispatch::<UIState>::global().get();
    let playing = state.currently_playing.as_ref().filter(|p| p.episode_id != 0)?;
    let (position, playing_now, rate, src) = if let Some(m) = state.media_element.as_ref() {
        let e = m.as_media_element();
        (e.current_time(), !e.paused() && !e.ended(), e.playback_rate(), e.current_src())
    } else if let Some(a) = state.audio_element.as_ref() {
        (a.current_time(), !a.paused() && !a.ended(), a.playback_rate(), a.current_src())
    } else {
        return None;
    };
    Some(Sample {
        episode_id: playing.episode_id,
        is_youtube: playing.is_youtube,
        position,
        duration: playing.duration_sec,
        playing: playing_now,
        rate,
        downloaded: is_local_file(&src),
    })
}

struct Credentials {
    server: String,
    api_key: String,
    user_id: i32,
}

fn credentials() -> Option<Credentials> {
    let state = Dispatch::<AppState>::global().get();
    let auth = state.auth_details.as_ref()?;
    Some(Credentials { server: auth.server_name.clone(), api_key: auth.api_key.clone()?, user_id: state.user_details.as_ref()?.UserID })
}

fn tick() {
    let updates = TRACKER.with(|t| t.borrow_mut().observe(now_ms(), sample()));
    // Once a minute, try again for anything a failed request (or a page that loaded before the
    // user was signed in) left behind.
    let retry = TICKS.with(|t| {
        t.set(t.get().wrapping_add(1));
        t.get() % 60 == 0
    });
    if !updates.is_empty() {
        record(updates);
        send();
    } else if retry {
        send();
    }
}

/// Start watching the player and send anything left over from an earlier visit. Safe to call
/// from every component that might be first on the page.
pub fn start() {
    if STARTED.with(|s| s.replace(true)) {
        return;
    }
    gloo_timers::callback::Interval::new(1000, tick).forget();
    if let Some(window) = web_sys::window() {
        gloo_events::EventListener::new(&window, "pagehide", |_| leave()).forget();
        gloo_events::EventListener::new(&window, "online", |_| send()).forget();
        if let Some(document) = window.document() {
            gloo_events::EventListener::new(&document.clone(), "visibilitychange", move |_| {
                if Reflect::get(&document, &"hidden".into()).is_ok_and(|h| h.is_truthy()) {
                    leave();
                }
            })
            .forget();
        }
    }
    send();
}

/// The player skipped `seconds` of silence or an ad.
pub fn skipped(kind: Skipped, seconds: f64) {
    TRACKER.with(|t| t.borrow_mut().skipped(kind, seconds));
}

/// The episode in the player ran to its end.
pub fn finished() {
    tick();
    if let Some(update) = TRACKER.with(|t| t.borrow_mut().finish(now_ms())) {
        record(vec![update]);
        send();
    }
}

/// The page may not come back: save and send without waiting for an answer.
fn leave() {
    if let Some(update) = TRACKER.with(|t| t.borrow_mut().leave(now_ms())) {
        record(vec![update]);
    }
    let mut pending = load();
    let (Some(creds), false) = (credentials(), pending.0.is_empty() || OLD_SERVER.with(Cell::get)) else { return };
    let init = Object::new();
    let headers = Object::new();
    let _ = Reflect::set(&headers, &"Content-Type".into(), &"application/json".into());
    let _ = Reflect::set(&headers, &"Api-Key".into(), &creds.api_key.into());
    let _ = Reflect::set(&init, &"method".into(), &"POST".into());
    let _ = Reflect::set(&init, &"headers".into(), &headers);
    let _ = Reflect::set(&init, &"body".into(), &request_body(&pending.0, now_ms()).into());
    // Lets the request outlive the page.
    let _ = Reflect::set(&init, &"keepalive".into(), &JsValue::TRUE);
    let Some(window) = web_sys::window() else { return };
    let Ok(fetch) = Reflect::get(&window, &"fetch".into()).and_then(|f| f.dyn_into::<Function>()) else { return };
    let _ = fetch.call2(&window, &format!("{}/api/data/progress", creds.server).into(), &init);
    // The positions stay until a later request confirms them (sending one twice is harmless);
    // the played and saved time is taken as delivered so it can't be counted twice.
    for update in &mut pending.0 {
        update.clear_counts();
    }
    save(&pending);
}

/// Send everything pending. One request at a time; a call during one runs another after it.
pub fn send() {
    if SENDING.with(|s| s.replace(true)) {
        SEND_AGAIN.with(|s| s.set(true));
        return;
    }
    spawn_local(async {
        loop {
            SEND_AGAIN.with(|s| s.set(false));
            send_once().await;
            if !SEND_AGAIN.with(Cell::get) {
                break;
            }
        }
        SENDING.with(|s| s.set(false));
    });
}

async fn send_once() {
    let sent = load().0;
    let (Some(creds), false) = (credentials(), sent.is_empty()) else { return };
    let delivered = if OLD_SERVER.with(Cell::get) {
        send_to_old_server(&creds, &sent).await
    } else {
        let request = Request::post(&format!("{}/api/data/progress", creds.server))
            .header("Content-Type", "application/json")
            .header("Api-Key", &creds.api_key)
            .body(request_body(&sent, now_ms()));
        let Ok(request) = request else { return };
        match request.send().await {
            Ok(response) if response.ok() => true,
            Ok(response) if matches!(response.status(), 404 | 405) => {
                OLD_SERVER.with(|o| o.set(true));
                send_to_old_server(&creds, &sent).await
            }
            _ => false,
        }
    };
    if delivered {
        let mut pending = load();
        pending.settle(&sent);
        save(&pending);
    }
}

/// A server from before `/progress`: a position call per episode, a completion call for a
/// finished one, and a call per minute played.
async fn send_to_old_server(creds: &Credentials, updates: &[Update]) -> bool {
    for update in updates {
        let request = RecordListenDurationRequest {
            episode_id: update.episode_id,
            user_id: creds.user_id,
            listen_duration: update.position,
            is_youtube: Some(update.is_youtube),
        };
        if call_record_listen_duration(&creds.server, &creds.api_key, request).await.is_err() {
            return false;
        }
        if update.finished {
            let request = MarkEpisodeCompletedRequest { episode_id: update.episode_id, user_id: creds.user_id, is_youtube: update.is_youtube };
            if call_mark_episode_completed(&creds.server, &Some(creds.api_key.clone()), &request).await.is_err() {
                return false;
            }
        }
        let mut seconds = OLD_SERVER_SECONDS.with(Cell::get) + update.listened;
        while seconds >= 60.0 {
            seconds -= 60.0;
            let _ = call_increment_listen_time(&creds.server, &creds.api_key, creds.user_id).await;
        }
        OLD_SERVER_SECONDS.with(|s| s.set(seconds));
    }
    true
}

/// Where to start an episode that plays from this device alone (no server to ask).
pub fn local_resume(episode_id: i32, is_youtube: bool, listed_position: f64, duration: f64) -> f64 {
    let position = load().get(episode_id, is_youtube).map_or(listed_position, |u| if u.finished { 0.0 } else { u.position });
    resume_from(position, duration)
}

/// Where to start an episode: progress made here that the server doesn't have yet, else what
/// the server has now, else what the list the user clicked in said.
pub async fn resume_position(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    episode_id: i32,
    is_youtube: bool,
    listed_position: f64,
    duration: f64,
) -> f64 {
    if episode_id == 0 {
        return 0.0;
    }
    if let Some(pending) = load().get(episode_id, is_youtube) {
        return if pending.finished { 0.0 } else { resume_from(pending.position, duration) };
    }
    let request = EpisodeRequest { episode_id, user_id, person_episode: false, is_youtube };
    let (position, duration) = match call_get_episode_metadata(server_name, Some(api_key.to_string()), &request).await {
        Ok(episode) => (
            episode.listenduration as f64,
            if episode.episodeduration > 0 { episode.episodeduration as f64 } else { duration },
        ),
        Err(_) => (listed_position, duration),
    };
    resume_from(position, duration)
}
