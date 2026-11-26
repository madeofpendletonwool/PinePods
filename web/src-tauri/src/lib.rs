mod credentials;
mod downloads;
mod library;
mod media_server;
#[cfg(test)]
mod pipeline_tests;

use downloads::{DownloadRequest, Downloads, Progress};
use library::Library;
use media_server::MediaServer;
use serde::Serialize;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Builder, Emitter, Manager, Runtime, State};

pub struct Desktop {
    data_dir: PathBuf,
    library: Arc<Mutex<Library>>,
    downloads: Arc<Downloads>,
    media: MediaServer,
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

/// True for the sign-in link the server sends: `pinepods://auth/callback?...`.
fn is_sign_in_link(url: &url::Url) -> bool {
    url.scheme() == "pinepods" && url.host_str() == Some("auth") && url.path() == "/callback"
}

/// Hand sign-in links to the page and bring the window forward. A second launch (Windows,
/// Linux) reaches here through the single-instance plugin.
#[cfg_attr(test, allow(dead_code))]
fn follow_sign_in_links<R: Runtime>(app: &tauri::App<R>) {
    use tauri_plugin_deep_link::DeepLinkExt;
    // Installers register the scheme; an AppImage or a dev build registers itself here.
    #[cfg(any(windows, target_os = "linux"))]
    if cfg!(windows) || app.env().appimage.is_some() {
        if let Err(e) = app.deep_link().register_all() {
            log::warn!("could not register the pinepods:// link handler: {e}");
        }
    }
    let handle = app.handle().clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls().into_iter().filter(is_sign_in_link) {
            log::info!("received the sign-in link");
            let _ = handle.emit(SIGN_IN_EVENT, url.to_string());
            show_main(&handle);
        }
    });
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
            // The tray needs the main thread, which unit tests don't run on.
            #[cfg(not(test))]
            {
                build_tray(app)?;
                follow_sign_in_links(app);
            }
            app.manage(Desktop {
                data_dir,
                library: Arc::new(Mutex::new(library)),
                downloads: Arc::new(Downloads::new()),
                media,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_info,
            list_downloads,
            download_episode,
            cancel_download,
            delete_downloads,
            open_sign_in,
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

/// The app sends these to the page as `desktop://tray` so it can control playback.
const TRAY_EVENT: &str = "desktop://tray";

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg_attr(test, allow(dead_code))]
fn build_tray<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
    use tauri::tray::TrayIconBuilder;

    let show = MenuItem::with_id(app, "show", "Show Pinepods", true, None::<&str>)?;
    let play = MenuItem::with_id(app, "play_pause", "Play / Pause", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Pinepods", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &play, &separator, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Pinepods")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "play_pause" => {
                let _ = app.emit(TRAY_EVENT, "play_pause");
            }
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        // First, so a second launch hands over to the running app before anything starts.
        // With its deep-link feature it also forwards a sign-in link the second launch was given.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .build(),
        );
    build_app(builder)
        .run(context())
        .expect("error while running tauri application");
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
    }

    #[test]
    fn only_the_sign_in_link_is_followed() {
        let link = |s: &str| super::is_sign_in_link(&s.parse().unwrap());
        assert!(link("pinepods://auth/callback?api_key=k&state=s"));
        assert!(link("pinepods://auth/callback?error=no_access"));
        assert!(!link("pinepods://auth/callbackevil?api_key=k"));
        assert!(!link("pinepods://other/callback?api_key=k"));
        assert!(!link("https://auth/callback?api_key=k"));
    }
}
