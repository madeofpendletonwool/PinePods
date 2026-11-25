mod credentials;
mod downloads;
mod library;
mod links;
#[cfg(target_os = "linux")]
mod media_controls;
mod media_server;
mod settings;
mod updates;
#[cfg(test)]
mod pipeline_tests;

use downloads::{DownloadRequest, Downloads, Progress};
use library::Library;
use media_server::MediaServer;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Builder, Emitter, Listener, Manager, Runtime, State, WebviewUrl, WebviewWindowBuilder};

pub struct Desktop {
    data_dir: PathBuf,
    library: Arc<Mutex<Library>>,
    downloads: Arc<Downloads>,
    media: MediaServer,
    settings: Mutex<settings::Settings>,
    /// The tray's and notices' words, in the app's language once the page sends them.
    texts: Mutex<Texts>,
    /// A podcast link the app was started with, until the page asks for it.
    launch_feed: Mutex<Option<String>>,
    update: updates::Pending,
}

#[derive(Serialize)]
struct DesktopInfo {
    version: &'static str,
    media_base_url: String,
    downloads_dir: String,
}

#[derive(Serialize)]
struct DownloadList {
    episodes: Vec<Value>,
    podcasts: Vec<Value>,
    active: Vec<Progress>,
    /// Episodes downloaded by automatic downloads (which may also delete them).
    automatic: Vec<i32>,
}

#[tauri::command]
fn desktop_info(desktop: State<'_, Desktop>) -> DesktopInfo {
    let library = desktop.library.lock().unwrap();
    DesktopInfo {
        version: env!("CARGO_PKG_VERSION"),
        media_base_url: desktop.media.base_url().to_string(),
        downloads_dir: library.downloads_dir().display().to_string(),
    }
}

/// Downloaded episodes as the app's `Episode` JSON, pointing at the local media server.
#[tauri::command]
fn list_downloads(desktop: State<'_, Desktop>) -> DownloadList {
    let base = desktop.media.base_url();
    let library = desktop.library.lock().unwrap();
    let episodes = library
        .episodes()
        .iter()
        .map(|record| {
            let mut episode = record.episode.clone();
            if let Some(obj) = episode.as_object_mut() {
                obj.insert("downloadedlocation".into(), format!("{base}/{}", record.file).into());
                if let Some(art) = &record.artwork_file {
                    obj.insert("episodeartwork".into(), format!("{base}/{art}").into());
                }
            }
            episode
        })
        .collect();
    DownloadList {
        episodes,
        podcasts: library.podcasts().to_vec(),
        active: desktop.downloads.active(),
        automatic: library.episodes().iter().filter(|r| r.automatic).map(|r| r.episode_id).collect(),
    }
}

#[tauri::command]
fn download_episode<R: Runtime>(
    app: AppHandle<R>,
    desktop: State<'_, Desktop>,
    request: DownloadRequest,
) -> Result<bool, String> {
    desktop.downloads.start(app, desktop.library.clone(), request)
}

#[tauri::command]
fn cancel_download(desktop: State<'_, Desktop>, episode_id: i32) {
    desktop.downloads.cancel(episode_id);
}

#[tauri::command]
fn delete_downloads(desktop: State<'_, Desktop>, episode_ids: Vec<i32>) -> Vec<i32> {
    for id in &episode_ids {
        desktop.downloads.cancel(*id);
    }
    desktop.library.lock().unwrap().remove(&episode_ids)
}

/// Open an OIDC provider's sign-in page in the system browser: providers refuse embedded
/// webviews. Only web addresses are opened.
#[tauri::command]
fn open_sign_in<R: Runtime>(app: AppHandle<R>, url: String) -> Result<(), String> {
    let parsed = url::Url::parse(&url).map_err(|e| e.to_string())?;
    if !matches!(parsed.scheme(), "https" | "http") {
        return Err(format!("not a web address: {}", parsed.scheme()));
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener().open_url(parsed.as_str(), None::<&str>).map_err(|e| e.to_string())
}

/// The server sends the browser back to the app with this link after an OIDC sign-in.
const SIGN_IN_EVENT: &str = "desktop://sign-in";
/// A podcast subscribe link (`podcast://`, `itpc://`, `pinepods-subscribe://`): the feed URL.
const SUBSCRIBE_EVENT: &str = "desktop://subscribe";
/// An OPML file dropped on the window: its contents.
const OPML_EVENT: &str = "desktop://opml";
/// The player in the main window publishes its state as this event...
const PLAYER_STATE: &str = "desktop://player-state";
/// ...and takes commands (from the tray, the mini player, media keys) as this one.
const PLAYER_COMMAND: &str = "desktop://player-command";
const MAX_OPML: u64 = 5 * 1024 * 1024;

fn handle_link<R: Runtime>(app: &AppHandle<R>, url: &url::Url) {
    if links::is_sign_in_link(url) {
        log::info!("received the sign-in link");
        let _ = app.emit_to("main", SIGN_IN_EVENT, url.to_string());
    } else if let Some(feed) = links::feed_from_link(url) {
        log::info!("received a podcast link: {feed}");
        let _ = app.emit_to("main", SUBSCRIBE_EVENT, feed);
    } else {
        return;
    }
    show_main(app);
}

/// Follow sign-in and podcast links. A second launch (Windows, Linux) reaches here through
/// the single-instance plugin.
#[cfg_attr(test, allow(dead_code))]
fn follow_links<R: Runtime>(app: &tauri::App<R>) {
    use tauri_plugin_deep_link::DeepLinkExt;
    // Installers register all the schemes. An AppImage or a dev build registers its own ones
    // here; podcast:// and itpc:// are left alone so another podcast app the listener chose
    // keeps them.
    #[cfg(any(windows, target_os = "linux"))]
    if cfg!(windows) || app.env().appimage.is_some() {
        for scheme in links::OWN_SCHEMES {
            if let Err(e) = app.deep_link().register(scheme) {
                log::warn!("could not register {scheme}:// links: {e}");
            }
        }
    }
    // Started by clicking a podcast link: hold the feed until the page is ready for it.
    if let Ok(Some(urls)) = app.deep_link().get_current() {
        let feed = urls.iter().find_map(links::feed_from_link);
        if let Some(desktop) = app.try_state::<Desktop>() {
            *desktop.launch_feed.lock().unwrap() = feed;
        }
    }
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            handle_link(&handle, &url);
        }
    });
}

/// The podcast link the app was started with, once.
#[tauri::command]
fn take_launch_feed(desktop: State<'_, Desktop>) -> Option<String> {
    desktop.launch_feed.lock().unwrap().take()
}

#[derive(Serialize)]
struct SettingsView {
    #[serde(flatten)]
    settings: settings::Settings,
    launch_at_login: bool,
    updates_supported: bool,
}

fn launch_at_login<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<tauri_plugin_autostart::AutoLaunchManager>()
        .and_then(|m| m.is_enabled().ok())
        .unwrap_or(false)
}

#[tauri::command]
fn get_desktop_settings<R: Runtime>(app: AppHandle<R>, desktop: State<'_, Desktop>) -> SettingsView {
    SettingsView {
        settings: desktop.settings.lock().unwrap().clone(),
        launch_at_login: launch_at_login(&app),
        updates_supported: updates::supported(),
    }
}

#[tauri::command]
fn set_desktop_settings(desktop: State<'_, Desktop>, settings: settings::Settings) -> Result<(), String> {
    settings::save(&desktop.data_dir, &settings)?;
    *desktop.settings.lock().unwrap() = settings;
    Ok(())
}

/// Start with the computer (in the tray, without a window).
#[tauri::command]
fn set_launch_at_login<R: Runtime>(app: AppHandle<R>, enabled: bool) -> Result<bool, String> {
    let manager = app
        .try_state::<tauri_plugin_autostart::AutoLaunchManager>()
        .ok_or("not available")?;
    let result = if enabled { manager.enable() } else { manager.disable() };
    result.map_err(|e| e.to_string())?;
    Ok(launch_at_login(&app))
}

/// A system notification.
#[tauri::command]
fn notify<R: Runtime>(app: AppHandle<R>, title: String, body: String) {
    send_notification(&app, &title, &body);
}

fn send_notification<R: Runtime>(app: &AppHandle<R>, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("could not show a notification: {e}");
    }
}

#[tauri::command]
async fn check_for_update<R: Runtime>(
    app: AppHandle<R>,
    desktop: State<'_, Desktop>,
) -> Result<Option<updates::UpdateInfo>, String> {
    updates::check(&app, &desktop.update).await
}

#[tauri::command]
async fn install_update<R: Runtime>(app: AppHandle<R>, desktop: State<'_, Desktop>) -> Result<(), String> {
    updates::install(&app, &desktop.update).await
}

/// A small always-on-top window with the player.
#[tauri::command]
fn open_mini_player<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    show_mini(&app).map_err(|e| e.to_string())
}

fn show_mini<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("mini") {
        window.unminimize()?;
        window.show()?;
        return window.set_focus();
    }
    WebviewWindowBuilder::new(app, "mini", WebviewUrl::App("index.html?window=mini".into()))
        .title("Pinepods")
        .inner_size(400.0, 250.0)
        .min_inner_size(320.0, 150.0)
        .always_on_top(true)
        .build()?;
    Ok(())
}

/// Turn on the native media controls (Linux, when the webview has no Media Session).
#[tauri::command]
fn native_media_controls<R: Runtime>(app: AppHandle<R>) {
    #[cfg(target_os = "linux")]
    media_controls::start(&app);
    #[cfg(not(target_os = "linux"))]
    let _ = app;
}

#[tauri::command]
fn store_credentials(desktop: State<'_, Desktop>, key: String, value: String) -> Result<(), String> {
    credentials::store(&desktop.data_dir, &key, &value)
}

#[tauri::command]
fn get_credentials(desktop: State<'_, Desktop>, key: String) -> Result<String, String> {
    credentials::get(&desktop.data_dir, &key)
}

#[tauri::command]
fn remove_credentials(desktop: State<'_, Desktop>, key: String) -> Result<(), String> {
    credentials::remove(&desktop.data_dir, &key)
}

#[tauri::command]
fn clear_all_credentials(desktop: State<'_, Desktop>) -> Result<(), String> {
    credentials::clear(&desktop.data_dir)
}

/// The data folder earlier versions used, kept so existing downloads and logins carry over.
/// `PINEPODS_DATA_DIR` overrides it (portable installs, tests).
fn data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("PINEPODS_DATA_DIR") {
        return PathBuf::from(dir);
    }
    directories::ProjectDirs::from("com", "gooseberrydevelopment", "pinepods")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("pinepods"))
}

/// Commands and state, shared by the app and the tests.
fn build_app<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder
        .setup(|app| {
            let data_dir = data_dir();
            std::fs::create_dir_all(&data_dir)?;
            let library = Library::open(&data_dir);
            let media = MediaServer::start(library.downloads_dir().to_path_buf())?;
            log::info!("Pinepods {} started; data in {}", env!("CARGO_PKG_VERSION"), data_dir.display());
            let settings = settings::load(&data_dir);
            app.manage(Desktop {
                data_dir,
                library: Arc::new(Mutex::new(library)),
                downloads: Arc::new(Downloads::new()),
                media,
                settings: Mutex::new(settings),
                texts: Mutex::new(Texts::default()),
                launch_feed: Mutex::new(None),
                update: updates::Pending::default(),
            });
            // The tray needs the main thread, which unit tests don't run on.
            #[cfg(not(test))]
            {
                build_tray(app)?;
                follow_links(app);
                // Started with the computer: stay in the tray.
                if !std::env::args().any(|a| a == "--hidden") {
                    show_main(app.handle());
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_info,
            list_downloads,
            download_episode,
            cancel_download,
            delete_downloads,
            open_sign_in,
            take_launch_feed,
            get_desktop_settings,
            set_desktop_settings,
            set_launch_at_login,
            notify,
            check_for_update,
            install_update,
            open_mini_player,
            native_media_controls,
            set_texts,
            store_credentials,
            get_credentials,
            remove_credentials,
            clear_all_credentials
        ])
}

/// The app config, expanded once (the macro can only appear once per crate on macOS).
fn context<R: Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Tell the player in the main window to do something.
fn player_command<R: Runtime>(app: &AppHandle<R>, action: &str, value: Option<f64>) {
    let _ = app.emit_to("main", PLAYER_COMMAND, serde_json::json!({ "action": action, "value": value }));
}

/// Words the native side shows, sent by the page in the app's language.
#[derive(serde::Deserialize, Clone)]
#[serde(default)]
struct Texts {
    show: String,
    play: String,
    pause: String,
    back: String,
    forward: String,
    mini_player: String,
    quit: String,
    still_running: String,
    still_running_body: String,
}

impl Default for Texts {
    fn default() -> Self {
        Texts {
            show: "Show Pinepods".into(),
            play: "Play".into(),
            pause: "Pause".into(),
            back: "Back 15 seconds".into(),
            forward: "Forward 15 seconds".into(),
            mini_player: "Mini player".into(),
            quit: "Quit Pinepods".into(),
            still_running: "Pinepods is still running".into(),
            still_running_body: "Playback continues. Open Pinepods from the tray, or quit it there.".into(),
        }
    }
}

/// The tray menu's items, to retitle them.
struct TrayItems<R: Runtime> {
    show: tauri::menu::MenuItem<R>,
    play: tauri::menu::MenuItem<R>,
    back: tauri::menu::MenuItem<R>,
    forward: tauri::menu::MenuItem<R>,
    mini: tauri::menu::MenuItem<R>,
    quit: tauri::menu::MenuItem<R>,
    playing: std::sync::atomic::AtomicBool,
}

impl<R: Runtime> TrayItems<R> {
    fn retitle(&self, texts: &Texts) {
        let playing = self.playing.load(std::sync::atomic::Ordering::Relaxed);
        let _ = self.show.set_text(&texts.show);
        let _ = self.play.set_text(if playing { &texts.pause } else { &texts.play });
        let _ = self.back.set_text(&texts.back);
        let _ = self.forward.set_text(&texts.forward);
        let _ = self.mini.set_text(&texts.mini_player);
        let _ = self.quit.set_text(&texts.quit);
    }
}

/// The page's translations for the tray and notices.
#[tauri::command]
fn set_texts<R: Runtime>(app: AppHandle<R>, desktop: State<'_, Desktop>, texts: Texts) {
    if let Some(items) = app.try_state::<TrayItems<R>>() {
        items.retitle(&texts);
    }
    *desktop.texts.lock().unwrap() = texts;
}

/// What the tray shows about the player, from `desktop://player-state`.
#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct PlayerState {
    title: String,
    playing: bool,
}

#[cfg_attr(test, allow(dead_code))]
fn build_tray<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::TrayIconBuilder;

    let texts = app.state::<Desktop>().texts.lock().unwrap().clone();
    let show = MenuItem::with_id(app, "show", &texts.show, true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play_pause", &texts.play, false, None::<&str>)?;
    let back = MenuItem::with_id(app, "back", &texts.back, false, None::<&str>)?;
    let forward = MenuItem::with_id(app, "forward", &texts.forward, false, None::<&str>)?;
    let mini = MenuItem::with_id(app, "mini", &texts.mini_player, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", &texts.quit, true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &show,
            &PredefinedMenuItem::separator(app)?,
            &play,
            &back,
            &forward,
            &mini,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Pinepods")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "play_pause" => player_command(app, "toggle", None),
            "back" => player_command(app, "seek_by", Some(-15.0)),
            "forward" => player_command(app, "seek_by", Some(15.0)),
            "mini" => {
                if let Err(e) = show_mini(app) {
                    log::warn!("could not open the mini player: {e}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    let tray = tray.build(app)?;

    app.manage(TrayItems {
        show,
        play,
        back,
        forward,
        mini,
        quit,
        playing: Default::default(),
    });

    // Follow the player: what's playing in the tooltip, Play or Pause in the menu.
    let handle = app.handle().clone();
    app.listen_any(PLAYER_STATE, move |event| {
        let state: Option<PlayerState> = serde_json::from_str(event.payload()).unwrap_or_default();
        let loaded = state.is_some();
        let state = state.unwrap_or_default();
        let tooltip = if loaded { format!("Pinepods: {}", state.title) } else { "Pinepods".into() };
        let _ = tray.set_tooltip(Some(tooltip));
        let (Some(items), Some(desktop)) = (handle.try_state::<TrayItems<R>>(), handle.try_state::<Desktop>()) else {
            return;
        };
        items.playing.store(state.playing, std::sync::atomic::Ordering::Relaxed);
        items.retitle(&desktop.texts.lock().unwrap());
        for item in [&items.play, &items.back, &items.forward] {
            let _ = item.set_enabled(loaded);
        }
    });
    Ok(())
}

/// Closing the main window hides it to the tray, unless that's turned off. Files dropped on
/// it are imported when they're OPML.
fn on_window_event<R: Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    use tauri::{DragDropEvent, WindowEvent};
    if window.label() != "main" {
        return;
    }
    let app = window.app_handle();
    match event {
        WindowEvent::CloseRequested { api, .. } => {
            let Some(desktop) = app.try_state::<Desktop>() else { return };
            let mut settings = desktop.settings.lock().unwrap();
            if !settings.close_to_tray {
                return;
            }
            api.prevent_close();
            let _ = window.hide();
            if !settings.tray_notice_shown {
                settings.tray_notice_shown = true;
                let _ = settings::save(&desktop.data_dir, &settings);
                let texts = desktop.texts.lock().unwrap().clone();
                send_notification(app, &texts.still_running, &texts.still_running_body);
            }
        }
        // The audio lives in the main window: once it's gone, so is the app.
        WindowEvent::Destroyed => app.exit(0),
        WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) => {
            for path in paths {
                if let Some(opml) = read_opml(path) {
                    let _ = app.emit_to("main", OPML_EVENT, opml);
                }
            }
        }
        _ => {}
    }
}

/// The contents of a dropped file, if it looks like an OPML subscription list.
fn read_opml(path: &std::path::Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(ext.as_str(), "opml" | "xml") || std::fs::metadata(path).ok()?.len() > MAX_OPML {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    text.contains("<opml").then_some(text)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use tauri_plugin_window_state::StateFlags;
    let builder = tauri::Builder::default()
        // First, so a second launch hands over to the running app before anything starts.
        // With its deep-link feature it also forwards a link the second launch was given.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        // Not visibility: the window starts hidden and is shown unless started with the computer.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::all() - StateFlags::VISIBLE)
                .build(),
        )
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .build(),
        )
        .on_window_event(on_window_event);
    build_app(builder)
        .build(context())
        .expect("error while building tauri application")
        .run(|app, event| {
            // macOS: clicking the Dock icon while the window is hidden.
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen { has_visible_windows: false, .. } = event {
                show_main(app);
            }
            let _ = (app, event);
        });
}

#[cfg(test)]
mod ipc_tests {
    use serde_json::{json, Value};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::webview::InvokeRequest;

    fn call(webview: &tauri::WebviewWindow<tauri::test::MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
        tauri::test::get_ipc_response(
            webview,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(args),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        )
        .map(|b| b.deserialize::<Value>().unwrap())
    }

    /// The argument names the frontend sends (camelCase), with the app's real config and
    /// capabilities.
    #[test]
    fn frontend_calls_reach_the_commands() {
        let dir = std::env::temp_dir().join(format!("pinepods-ipc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("PINEPODS_DATA_DIR", &dir);
        let context = super::context();
        let mut app = super::build_app(tauri::test::mock_builder()).build(context).unwrap();
        // One loop iteration runs the setup hook, which creates the app's state.
        #[allow(deprecated)]
        app.run_iteration(|_, _| {});
        let webview = tauri::Manager::get_webview_window(&app, "main").expect("the configured window");

        let info = call(&webview, "desktop_info", json!({})).unwrap();
        assert!(info["media_base_url"].as_str().unwrap().starts_with("http://127.0.0.1:"));
        let list = call(&webview, "list_downloads", json!({})).unwrap();
        assert_eq!(list["episodes"], json!([]));
        call(&webview, "cancel_download", json!({ "episodeId": 4 })).unwrap();
        assert_eq!(call(&webview, "delete_downloads", json!({ "episodeIds": [4] })).unwrap(), json!([]));
        // Unreachable URL: accepted, then fails in the background.
        let req = json!({ "request": { "episode": { "episodeid": 4 }, "media_url": "http://127.0.0.1:9/x", "artwork_url": null } });
        assert_eq!(call(&webview, "download_episode", req).unwrap(), json!(true));

        call(&webview, "store_credentials", json!({ "key": "userState", "value": "{}" })).unwrap();
        assert_eq!(call(&webview, "get_credentials", json!({ "key": "userState" })).unwrap(), json!("{}"));
        assert!(call(&webview, "get_credentials", json!({ "key": "../../etc/passwd" })).is_err());
        call(&webview, "clear_all_credentials", json!({})).unwrap();
        assert!(call(&webview, "get_credentials", json!({ "key": "userState" })).is_err());

        // Only web pages open in the browser.
        for url in ["file:///etc/passwd", "javascript:alert(1)", "pinepods://auth/callback", "not a url"] {
            assert!(call(&webview, "open_sign_in", json!({ "url": url })).is_err(), "{url}");
        }

        let settings = call(&webview, "get_desktop_settings", json!({})).unwrap();
        assert_eq!(settings["close_to_tray"], json!(true));
        assert_eq!(settings["launch_at_login"], json!(false));
        let mut changed = settings.clone();
        changed["notify_downloads"] = json!(true);
        changed["auto_download"] = json!({ "12": 3 });
        call(&webview, "set_desktop_settings", json!({ "settings": changed })).unwrap();
        let saved = call(&webview, "get_desktop_settings", json!({})).unwrap();
        assert_eq!(saved["notify_downloads"], json!(true));
        assert_eq!(saved["auto_download"], json!({ "12": 3 }));
        assert!(dir.join("desktop_settings.json").exists());
        assert_eq!(call(&webview, "take_launch_feed", json!({})).unwrap(), json!(null));
    }
}
