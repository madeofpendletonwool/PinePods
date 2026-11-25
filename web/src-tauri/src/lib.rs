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
use tauri::{AppHandle, Builder, Manager, Runtime, State};

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(
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
        let mut context = super::context();
        // The tray icon needs the main thread, which a test doesn't run on.
        context.config_mut().app.tray_icon = None;
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
    }
}
