//! The desktop app (Tauri): detection, and the bridge to its native side.
//!
//! The same build runs in a browser and in the desktop app, so everything here checks
//! `is_desktop()` at runtime and does nothing in a browser.

use crate::components::context::{EpisodeStatusState, NotificationState};
use crate::components::i18n::use_translation;
use crate::requests::episode::Episode;
use crate::requests::pod_req::{call_get_episode_metadata, EpisodeRequest};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yewdux::prelude::*;

const DOWNLOAD_EVENT: &str = "desktop://download";
const SIGN_IN_EVENT: &str = "desktop://sign-in";
const SUBSCRIBE_EVENT: &str = "desktop://subscribe";
const OPML_EVENT: &str = "desktop://opml";
/// The player in the main window publishes its state as this event...
pub const PLAYER_STATE: &str = "desktop://player-state";
/// ...and takes commands (tray, mini player, media keys) as this one.
pub const PLAYER_COMMAND: &str = "desktop://player-command";
/// The mini player asks for the current state when it opens.
pub const PLAYER_HELLO: &str = "desktop://player-hello";

/// Where the server sends the browser after an OIDC sign-in started in the desktop app.
pub const SIGN_IN_LINK: &str = "pinepods://auth/callback";

thread_local! {
    static IS_DESKTOP: Cell<Option<bool>> = const { Cell::new(None) };
    static MEDIA_BASE: RefCell<Option<String>> = const { RefCell::new(None) };
    static LISTENING: Cell<bool> = const { Cell::new(false) };
}

/// True when running inside the desktop app.
pub fn is_desktop() -> bool {
    IS_DESKTOP.with(|cell| {
        if let Some(v) = cell.get() {
            return v;
        }
        let v = web_sys::window()
            .and_then(|w| js_sys::Reflect::get(&w, &JsValue::from_str("__TAURI__")).ok())
            .is_some_and(|t| !t.is_undefined() && !t.is_null());
        cell.set(Some(v));
        v
    })
}

/// The client name recorded with listening time.
pub fn client_name() -> &'static str {
    if is_desktop() { "desktop" } else { "web" }
}

/// True if `src` is a file this computer serves from its downloads.
pub fn is_local_media(src: &str) -> bool {
    MEDIA_BASE.with(|b| b.borrow().as_deref().is_some_and(|base| src.starts_with(base)))
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "core"], js_name = invoke)]
    async fn tauri_invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "event"], js_name = listen)]
    async fn tauri_listen(event: &str, handler: &js_sys::Function) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "event"], js_name = emit)]
    async fn tauri_emit(event: &str, payload: JsValue) -> Result<JsValue, JsValue>;
}

/// Send an event to the app's other windows and its native side.
pub fn emit(event: &'static str, payload: &impl Serialize) {
    if !is_desktop() {
        return;
    }
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    let Ok(payload) = payload.serialize(&serializer) else { return };
    wasm_bindgen_futures::spawn_local(async move {
        let _ = tauri_emit(event, payload).await;
    });
}

/// Follow an event for the rest of the visit; `on_payload` gets each payload.
pub async fn listen(event: &str, on_payload: impl Fn(JsValue) + 'static) {
    if !is_desktop() {
        return;
    }
    let handler = Closure::<dyn Fn(JsValue)>::new(move |e: JsValue| {
        if let Ok(payload) = js_sys::Reflect::get(&e, &JsValue::from_str("payload")) {
            on_payload(payload);
        }
    });
    if let Err(e) = tauri_listen(event, handler.as_ref().unchecked_ref()).await {
        log_error(format!("could not follow {event}: {}", error_text(e)));
    }
    handler.forget();
}

/// True in the mini player window (opened with `?window=mini`).
pub fn is_mini_window() -> bool {
    is_desktop()
        && web_sys::window()
            .and_then(|w| w.location().search().ok())
            .is_some_and(|q| q.contains("window=mini"))
}

#[derive(Serialize)]
struct LogArgs<'a> {
    level: u8,
    message: &'a str,
}

/// Write to the desktop app's log file (and the console), so problems can be reported.
pub fn log_error(message: String) {
    web_sys::console::error_1(&message.clone().into());
    log(5, message);
}

/// Log right away, without waiting for the async executor: used from the panic hook, after
/// which no further tasks run.
pub fn log_now(message: &str) {
    if !is_desktop() {
        return;
    }
    let Some(window) = web_sys::window() else { return };
    let invoke = js_sys::Reflect::get(&window, &"__TAURI__".into())
        .and_then(|t| js_sys::Reflect::get(&t, &"core".into()))
        .and_then(|c| js_sys::Reflect::get(&c, &"invoke".into()));
    if let Ok(invoke) = invoke.and_then(|f| f.dyn_into::<js_sys::Function>().map_err(JsValue::from)) {
        let args = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&args, &"level".into(), &JsValue::from(5));
        let _ = js_sys::Reflect::set(&args, &"message".into(), &JsValue::from_str(message));
        let _ = invoke.call2(&JsValue::NULL, &"plugin:log|log".into(), &args);
    }
}

fn log(level: u8, message: String) {
    if is_desktop() {
        wasm_bindgen_futures::spawn_local(async move {
            let _ = invoke::<()>("plugin:log|log", LogArgs { level, message: &message }).await;
        });
    }
}

fn error_text(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

async fn invoke<T: DeserializeOwned>(cmd: &str, args: impl Serialize) -> Result<T, String> {
    if !is_desktop() {
        return Err("not running in the desktop app".into());
    }
    let serializer = serde_wasm_bindgen::Serializer::json_compatible();
    let mut args = args.serialize(&serializer).map_err(|e| e.to_string())?;
    if args.is_null() || args.is_undefined() {
        args = js_sys::Object::new().into();
    }
    let value = tauri_invoke(cmd, args).await.map_err(error_text)?;
    serde_wasm_bindgen::from_value(value).map_err(|e| e.to_string())
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Queued,
    Downloading,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct Progress {
    pub episode_id: i32,
    #[serde(default)]
    pub title: String,
    pub phase: Phase,
    pub received: u64,
    pub total: Option<u64>,
    #[serde(default)]
    pub error: Option<String>,
}

impl Progress {
    pub fn percent(&self) -> Option<u32> {
        self.total.filter(|t| *t > 0).map(|t| ((self.received * 100) / t).min(100) as u32)
    }
}

/// Downloads to this computer that haven't finished yet.
#[derive(Default, Clone, PartialEq, Store)]
pub struct DeviceDownloads {
    pub active: HashMap<i32, Progress>,
    pub downloads_dir: Option<String>,
}

#[derive(Deserialize)]
struct DesktopInfo {
    media_base_url: String,
    downloads_dir: String,
}

#[derive(Deserialize)]
struct DownloadList {
    episodes: Vec<serde_json::Value>,
    active: Vec<Progress>,
    #[serde(default)]
    automatic: Vec<i32>,
}

/// The desktop app's own settings, kept on this computer.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Settings {
    pub close_to_tray: bool,
    pub tray_notice_shown: bool,
    pub notify_new_episodes: bool,
    pub notify_downloads: bool,
    pub check_for_updates: bool,
    pub delete_finished_downloads: bool,
    /// Podcast id → how many of its newest episodes to keep downloaded.
    pub auto_download: std::collections::BTreeMap<i32, u32>,
}

#[derive(Deserialize)]
struct SettingsView {
    #[serde(flatten)]
    settings: Settings,
    launch_at_login: bool,
    updates_supported: bool,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// Desktop state the pages follow.
#[derive(Default, Clone, PartialEq, Store)]
pub struct DesktopState {
    /// None until loaded from the native side.
    pub settings: Option<Settings>,
    pub launch_at_login: bool,
    pub updates_supported: bool,
    /// A newer version, found by the update check.
    pub update: Option<UpdateInfo>,
    /// A podcast link to open once signed in.
    pub feed_link: Option<String>,
    /// A dropped OPML file, for the import section to pick up.
    pub opml: Option<String>,
    /// Downloads made by automatic downloads.
    pub automatic: std::collections::HashSet<i32>,
}

/// Save the desktop settings.
pub async fn save_settings(settings: Settings) -> Result<(), String> {
    #[derive(Serialize)]
    struct Args<'a> {
        settings: &'a Settings,
    }
    invoke::<()>("set_desktop_settings", Args { settings: &settings }).await?;
    Dispatch::<DesktopState>::global().reduce_mut(move |s| s.settings = Some(settings));
    Ok(())
}

/// Change one setting from its current value.
pub async fn update_settings(change: impl FnOnce(&mut Settings)) -> Result<(), String> {
    let Some(mut settings) = Dispatch::<DesktopState>::global().get().settings.clone() else {
        return Err("settings not loaded".into());
    };
    change(&mut settings);
    save_settings(settings).await
}

pub async fn set_launch_at_login(enabled: bool) -> Result<bool, String> {
    #[derive(Serialize)]
    struct Args {
        enabled: bool,
    }
    let now = invoke::<bool>("set_launch_at_login", Args { enabled }).await?;
    Dispatch::<DesktopState>::global().reduce_mut(move |s| s.launch_at_login = now);
    Ok(now)
}

/// Show a system notification.
pub fn notify(title: String, body: String) {
    #[derive(Serialize)]
    struct Args {
        title: String,
        body: String,
    }
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = invoke::<()>("notify", Args { title, body }).await {
            log_error(format!("notification failed: {e}"));
        }
    });
}

/// Look for a newer version; remembered in `DesktopState::update`.
pub async fn check_for_update() -> Result<Option<UpdateInfo>, String> {
    let update = invoke::<Option<UpdateInfo>>("check_for_update", ()).await?;
    let found = update.clone();
    Dispatch::<DesktopState>::global().reduce_mut(move |s| s.update = found);
    Ok(update)
}

/// Install the update found by `check_for_update` and restart into it.
pub async fn install_update() -> Result<(), String> {
    invoke::<()>("install_update", ()).await
}

/// Words the native side shows (tray menu, notices), in the app's language.
#[derive(Serialize, Clone, PartialEq)]
pub struct NativeTexts {
    pub show: String,
    pub play: String,
    pub pause: String,
    pub back: String,
    pub forward: String,
    pub mini_player: String,
    pub quit: String,
    pub still_running: String,
    pub still_running_body: String,
}

pub fn set_native_texts(texts: NativeTexts) {
    #[derive(Serialize)]
    struct Args {
        texts: NativeTexts,
    }
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = invoke::<()>("set_texts", Args { texts }).await {
            log_error(format!("could not translate the tray: {e}"));
        }
    });
}

pub async fn open_mini_player() -> Result<(), String> {
    invoke::<()>("open_mini_player", ()).await
}

async fn load_settings() {
    match invoke::<SettingsView>("get_desktop_settings", ()).await {
        Ok(view) => Dispatch::<DesktopState>::global().reduce_mut(move |s| {
            s.settings = Some(view.settings);
            s.launch_at_login = view.launch_at_login;
            s.updates_supported = view.updates_supported;
        }),
        Err(e) => log_error(format!("could not load the desktop settings: {e}")),
    }
}

/// Parse each record on its own, so one bad record doesn't hide the rest.
fn parse_episodes(values: Vec<serde_json::Value>) -> Vec<Episode> {
    values
        .into_iter()
        .filter_map(|v| match serde_json::from_value::<Episode>(v) {
            Ok(ep) => Some(ep),
            Err(e) => {
                log_error(format!("skipped a downloaded episode that couldn't be read: {e}"));
                None
            }
        })
        .collect()
}

/// Messages shown when a download ends, translated by the caller.
#[derive(Clone)]
pub struct DownloadMessages {
    pub complete: String,
    pub failed: String,
}

/// Connect to the native side once: learn the media server address, load the downloads
/// and follow download progress.
pub async fn init(messages: DownloadMessages) {
    if !is_desktop() || LISTENING.with(|l| l.replace(true)) {
        return;
    }
    match invoke::<DesktopInfo>("desktop_info", ()).await {
        Ok(info) => {
            MEDIA_BASE.with(|b| *b.borrow_mut() = Some(info.media_base_url));
            Dispatch::<DeviceDownloads>::global()
                .reduce_mut(|s| s.downloads_dir = Some(info.downloads_dir));
        }
        Err(e) => log_error(format!("desktop_info failed: {e}")),
    }
    refresh().await;

    let handler = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
        let Ok(payload) = js_sys::Reflect::get(&event, &JsValue::from_str("payload")) else { return };
        let Ok(progress) = serde_wasm_bindgen::from_value::<Progress>(payload) else { return };
        on_progress(progress, &messages);
    });
    if let Err(e) = tauri_listen(DOWNLOAD_EVENT, handler.as_ref().unchecked_ref()).await {
        log_error(format!("could not follow downloads: {}", error_text(e)));
    } else {
        let count = Dispatch::<EpisodeStatusState>::global().get().downloaded_episodes.local_ids().count();
        log(3, format!("desktop bridge ready: {count} download(s) on this computer"));
    }
    handler.forget();

    load_settings().await;

    // Podcast links (also one the app was started with) and dropped OPML files.
    if let Ok(Some(feed)) = invoke::<Option<String>>("take_launch_feed", ()).await {
        Dispatch::<DesktopState>::global().reduce_mut(move |s| s.feed_link = Some(feed));
    }
    listen(SUBSCRIBE_EVENT, |payload| {
        if let Some(feed) = payload.as_string() {
            Dispatch::<DesktopState>::global().reduce_mut(move |s| s.feed_link = Some(feed));
        }
    })
    .await;
    listen(OPML_EVENT, |payload| {
        if let Some(opml) = payload.as_string() {
            Dispatch::<DesktopState>::global().reduce_mut(move |s| s.opml = Some(opml));
        }
    })
    .await;

    // Webviews without a Media Session (some Linux builds) get the system controls natively.
    let has_media_session = web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w.navigator(), &JsValue::from_str("mediaSession")).ok())
        .is_some_and(|v| !v.is_undefined() && !v.is_null());
    if !has_media_session {
        let _ = invoke::<()>("native_media_controls", ()).await;
    }

    let state = Dispatch::<DesktopState>::global().get();
    if state.updates_supported && state.settings.as_ref().is_some_and(|s| s.check_for_updates) {
        if let Err(e) = check_for_update().await {
            log(3, format!("update check failed: {e}"));
        }
    }

    // The browser coming back from an OIDC sign-in.
    let sign_in = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
        let link = js_sys::Reflect::get(&event, &JsValue::from_str("payload")).ok().and_then(|p| p.as_string());
        if let Some(link) = link {
            Dispatch::<BrowserSignIn>::global().reduce_mut(move |s| s.returned = Some(link));
        }
    });
    if let Err(e) = tauri_listen(SIGN_IN_EVENT, sign_in.as_ref().unchecked_ref()).await {
        log_error(format!("could not follow sign-in links: {}", error_text(e)));
    }
    sign_in.forget();
}

/// An OIDC sign-in waiting in the system browser.
#[derive(Clone, PartialEq, Debug)]
pub struct PendingSignIn {
    pub server: String,
    pub state: String,
}

/// OIDC sign-in through the system browser: the one started here, and the link that came back.
#[derive(Default, Clone, PartialEq, Store)]
pub struct BrowserSignIn {
    pub pending: Option<PendingSignIn>,
    pub returned: Option<String>,
}

/// What a returned sign-in link means.
#[derive(Debug, PartialEq)]
pub enum SignInResult {
    SignedIn { server: String, api_key: String },
    /// The server's error code, e.g. `no_access`.
    Failed(String),
    /// The link doesn't belong to the sign-in started here.
    Mismatch,
    /// No sign-in was started; the link is ignored.
    NotStarted,
}

/// Check a returned link against the sign-in started here. The server's error links carry no
/// state, so they are accepted while a sign-in is waiting.
pub fn read_sign_in_link(link: &str, pending: Option<&PendingSignIn>) -> SignInResult {
    let Some(pending) = pending else { return SignInResult::NotStarted };
    let query = link.split_once('?').map(|(_, q)| q).unwrap_or_default();
    let param = |name: &str| {
        url::form_urlencoded::parse(query.as_bytes())
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
            .filter(|v| !v.is_empty())
    };
    if let Some(code) = param("error") {
        return SignInResult::Failed(code);
    }
    match (param("api_key"), param("state")) {
        (Some(api_key), Some(state)) if state == pending.state => {
            SignInResult::SignedIn { server: pending.server.clone(), api_key }
        }
        _ => SignInResult::Mismatch,
    }
}

#[derive(Serialize)]
struct UrlArgs {
    url: String,
}

/// Open a provider's sign-in page in the system browser.
pub async fn open_in_browser(url: String) -> Result<(), String> {
    invoke::<()>("open_sign_in", UrlArgs { url }).await
}

fn on_progress(progress: Progress, messages: &DownloadMessages) {
    let id = progress.episode_id;
    match progress.phase {
        Phase::Queued | Phase::Downloading => {
            Dispatch::<DeviceDownloads>::global().reduce_mut(move |s| {
                s.active.insert(id, progress);
            });
        }
        Phase::Complete | Phase::Failed | Phase::Cancelled => {
            Dispatch::<DeviceDownloads>::global().reduce_mut(|s| {
                s.active.remove(&id);
            });
            let message = match progress.phase {
                Phase::Complete => Some((true, messages.complete.clone())),
                Phase::Failed => Some((
                    false,
                    format!("{}: {}", messages.failed, progress.error.unwrap_or_default()),
                )),
                _ => None,
            };
            let settings = Dispatch::<DesktopState>::global().get().settings.clone();
            if progress.phase == Phase::Complete && settings.is_some_and(|s| s.notify_downloads) {
                notify(messages.complete.clone(), progress.title.clone());
            }
            if let Some((ok, text)) = message {
                Dispatch::<NotificationState>::global().reduce_mut(move |s| {
                    if ok {
                        s.info_message = Some(text);
                    } else {
                        s.error_message = Some(text);
                    }
                });
            }
            wasm_bindgen_futures::spawn_local(refresh());
        }
    }
}

/// Reload the list of episodes downloaded to this computer.
pub async fn refresh() {
    if !is_desktop() {
        return;
    }
    match invoke::<DownloadList>("list_downloads", ()).await {
        Ok(list) => {
            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                s.downloaded_episodes.clear_local();
                for ep in parse_episodes(list.episodes) {
                    s.downloaded_episodes.push_local(ep);
                }
            });
            Dispatch::<DeviceDownloads>::global().reduce_mut(move |s| {
                s.active = list.active.into_iter().map(|p| (p.episode_id, p)).collect();
            });
            Dispatch::<DesktopState>::global()
                .reduce_mut(move |s| s.automatic = list.automatic.into_iter().collect());
        }
        Err(e) => log_error(format!("list_downloads failed: {e}")),
    }
}

/// The downloaded copy of an episode on this computer, if there is one.
pub fn local_copy(episode_id: i32) -> Option<Episode> {
    let status = Dispatch::<EpisodeStatusState>::global().get();
    if !status.downloaded_episodes.is_local_download(episode_id) {
        return None;
    }
    let found = status
        .downloaded_episodes
        .episodes()
        .find(|ep| ep.episodeid == episode_id && ep.downloadedlocation.is_some())
        .cloned();
    found
}

/// Where to fetch an episode's media: through the server when it holds the file
/// (YouTube, local media, server downloads), otherwise from the feed.
pub fn media_url(episode: &Episode, server: &str, api_key: &str, user_id: i32, on_server: bool) -> String {
    if episode.is_youtube || episode.episodeurl.contains("youtube.com") {
        format!("{server}/api/data/stream/{}?api_key={api_key}&user_id={user_id}&type=youtube", episode.episodeid)
    } else if on_server || episode.downloaded || episode.episodeurl.starts_with("local://") {
        format!("{server}/api/data/stream/{}?api_key={api_key}&user_id={user_id}", episode.episodeid)
    } else {
        episode.episodeurl.clone()
    }
}

#[derive(Serialize)]
struct DownloadRequest<'a> {
    episode: &'a Episode,
    media_url: String,
    artwork_url: Option<String>,
    automatic: bool,
}

#[derive(Serialize)]
struct DownloadArgs<'a> {
    request: DownloadRequest<'a>,
}

/// Download an episode to this computer. Progress arrives through `DeviceDownloads`.
pub async fn download(episode: Episode, server: String, api_key: String, user_id: i32) -> Result<(), String> {
    download_episode(episode, server, api_key, user_id, false).await
}

/// `download`, marked as made by automatic downloads when `automatic`.
pub async fn download_episode(
    episode: Episode,
    server: String,
    api_key: String,
    user_id: i32,
    automatic: bool,
) -> Result<(), String> {
    let on_server = Dispatch::<EpisodeStatusState>::global()
        .get()
        .downloaded_episodes
        .is_server_download(episode.episodeid);
    // The full record (complete description, podcast details) when the server has it.
    let request = EpisodeRequest {
        episode_id: episode.episodeid,
        user_id,
        person_episode: false,
        is_youtube: episode.is_youtube,
    };
    let mut full = call_get_episode_metadata(&server, Some(api_key.clone()), &request)
        .await
        .unwrap_or_else(|_| episode.clone());
    if full.episodeid == 0 {
        full = episode;
    }
    let artwork = [&full.episodeartwork, &full.artworkurl]
        .into_iter()
        .find(|a| !a.is_empty())
        .cloned();
    let media_url = media_url(&full, &server, &api_key, user_id, on_server);
    invoke::<bool>(
        "download_episode",
        DownloadArgs { request: DownloadRequest { episode: &full, media_url, artwork_url: artwork, automatic } },
    )
    .await
    .map(|_| ())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeIdArgs {
    episode_id: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EpisodeIdsArgs {
    episode_ids: Vec<i32>,
}

pub async fn cancel(episode_id: i32) {
    let _ = invoke::<()>("cancel_download", EpisodeIdArgs { episode_id }).await;
}

/// Delete downloads from this computer. Returns the ids that were deleted.
pub async fn delete(episode_ids: Vec<i32>) -> Result<Vec<i32>, String> {
    let removed = invoke::<Vec<i32>>("delete_downloads", EpisodeIdsArgs { episode_ids }).await?;
    let gone = removed.clone();
    Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
        for id in gone {
            s.downloaded_episodes.remove_local(id);
        }
    });
    Ok(removed)
}

#[derive(Properties, PartialEq)]
pub struct DeviceDownloadItemProps {
    pub episode: Episode,
    /// Closes the menu the item sits in.
    pub on_done: Callback<()>,
}

/// The context-menu entry for downloading an episode to this computer, following it while it
/// downloads (click to cancel) and deleting it afterwards. Renders nothing in a browser.
#[function_component(DeviceDownloadItem)]
pub fn device_download_item(props: &DeviceDownloadItemProps) -> Html {
    let (i18n, _) = use_translation();
    let id = props.episode.episodeid;
    let downloaded = use_selector(move |s: &EpisodeStatusState| s.downloaded_episodes.is_local_download(id));
    let active = use_selector(move |s: &DeviceDownloads| s.active.get(&id).cloned());
    let auth = use_selector(|s: &crate::components::context::AppState| {
        let server = s.auth_details.as_ref().map(|a| a.server_name.clone());
        let key = s.auth_details.as_ref().and_then(|a| a.api_key.clone());
        let user = s.user_details.as_ref().map(|u| u.UserID);
        (server, key, user)
    });
    if !is_desktop() {
        return html! {};
    }

    let failed_text = i18n.t("desktop.download_failed").to_string();
    let deleted_text = i18n.t("desktop.download_deleted").to_string();
    let onclick = {
        let episode = props.episode.clone();
        let on_done = props.on_done.clone();
        let downloaded = *downloaded;
        let is_active = active.is_some();
        let auth = (*auth).clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_done.emit(());
            let episode = episode.clone();
            let auth = auth.clone();
            let (failed_text, deleted_text) = (failed_text.clone(), deleted_text.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let result = if is_active {
                    cancel(episode.episodeid).await;
                    Ok(None)
                } else if downloaded {
                    delete(vec![episode.episodeid]).await.map(|_| Some(deleted_text))
                } else if let (Some(server), Some(key), Some(user)) = auth {
                    download(episode, server, key, user).await.map(|_| None)
                } else {
                    Err("not signed in".to_string())
                };
                Dispatch::<NotificationState>::global().reduce_mut(move |s| match result {
                    Ok(Some(text)) => s.info_message = Some(text),
                    Ok(None) => {}
                    Err(e) => s.error_message = Some(format!("{failed_text}: {e}")),
                });
            });
        })
    };

    let label = match (&*active, *downloaded) {
        (Some(p), _) => match (p.phase.clone(), p.percent()) {
            (Phase::Queued, _) => i18n.t("desktop.download_waiting").to_string(),
            (_, Some(pct)) => format!("{} {pct}%", i18n.t("desktop.downloading")),
            _ => i18n.t("desktop.downloading").to_string(),
        },
        (None, true) => i18n.t("desktop.delete_from_computer").to_string(),
        (None, false) => i18n.t("desktop.download_to_computer").to_string(),
    };
    let hint = active.is_some().then(|| i18n.t("desktop.click_to_cancel").to_string());
    html! {
        <li class="dropdown-option" onclick={onclick} title={hint}>{ label }</li>
    }
}

/// Size in bytes as "12.3 MB".
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 { format!("{bytes} B") } else { format!("{value:.1} {}", UNITS[unit]) }
}

/// True when this computer has episodes downloaded.
pub fn has_downloads() -> bool {
    Dispatch::<EpisodeStatusState>::global().get().downloaded_episodes.local_ids().next().is_some()
}

/// Switch to offline listening: just the downloads, no server.
pub fn go_offline() {
    Dispatch::<crate::components::context::UIState>::global().reduce_mut(|s| s.app_offline_mode = Some(true));
    yew_router::history::History::push(&yew_router::history::BrowserHistory::new(), "/local_downloads");
}

/// When the server can't be reached at launch, open the downloads instead of the sign-in
/// page. Returns false (do nothing) outside the desktop app or with nothing downloaded.
pub async fn offline_fallback() -> bool {
    if !is_desktop() {
        return false;
    }
    refresh().await;
    if !has_downloads() {
        return false;
    }
    go_offline();
    true
}

/// True if the error means the server couldn't be reached (rather than it refusing).
pub fn is_unreachable(e: &anyhow::Error) -> bool {
    e.downcast_ref::<gloo_net::Error>().is_some()
}
