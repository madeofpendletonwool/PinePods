//! Updating the app in place from GitHub releases (signed; see the updater key in
//! tauri.conf.json). Packages that update through a package manager (deb, rpm, Flatpak,
//! AUR) don't use this.

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

/// The update found by the last check, until it's installed.
#[derive(Default)]
pub struct Pending(Mutex<Option<Update>>);

#[derive(Serialize, Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub notes: Option<String>,
}

/// Whether this install can update itself: macOS, Windows and AppImage, and not dev builds
/// (they'd be replaced by a release).
pub fn supported() -> bool {
    if cfg!(debug_assertions) && std::env::var_os("PINEPODS_UPDATE_TEST").is_none() {
        return false;
    }
    if cfg!(target_os = "linux") {
        return std::env::var_os("APPIMAGE").is_some() && std::env::var_os("FLATPAK_ID").is_none();
    }
    true
}

pub async fn check<R: Runtime>(app: &AppHandle<R>, pending: &Pending) -> Result<Option<UpdateInfo>, String> {
    if !supported() {
        return Ok(None);
    }
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|u| UpdateInfo { version: u.version.clone(), notes: u.body.clone() });
    if let Some(info) = &info {
        log::info!("update available: {}", info.version);
    }
    *pending.0.lock().unwrap() = update;
    Ok(info)
}

/// Download and install the update found by `check`, then restart into it.
pub async fn install<R: Runtime>(app: &AppHandle<R>, pending: &Pending) -> Result<(), String> {
    let update = pending.0.lock().unwrap().take().ok_or("no update to install")?;
    log::info!("installing update {}", update.version);
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| {
            log::warn!("update failed: {e}");
            e.to_string()
        })?;
    app.restart()
}
