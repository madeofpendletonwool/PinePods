//! The desktop app's own settings, kept on this computer (not on the server).

use crate::library::write_atomic;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SETTINGS_FILE: &str = "desktop_settings.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Closing the window keeps the app (and playback) running in the tray.
    pub close_to_tray: bool,
    /// Set once the "still running in the tray" notice has been shown.
    pub tray_notice_shown: bool,
    pub notify_new_episodes: bool,
    pub notify_downloads: bool,
    pub check_for_updates: bool,
    /// Delete downloads on this computer once they're played.
    pub delete_finished_downloads: bool,
    /// Podcast id → how many of its newest episodes to keep downloaded on this computer.
    pub auto_download: BTreeMap<i32, u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            close_to_tray: true,
            tray_notice_shown: false,
            notify_new_episodes: false,
            notify_downloads: false,
            check_for_updates: true,
            delete_finished_downloads: false,
            auto_download: BTreeMap::new(),
        }
    }
}

fn path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_FILE)
}

/// The saved settings, or the defaults if there are none or the file can't be read.
pub fn load(data_dir: &Path) -> Settings {
    let path = path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            log::warn!("could not read {}: {e}; using defaults", path.display());
            Settings::default()
        }),
        Err(_) => Settings::default(),
    }
}

pub fn save(data_dir: &Path, settings: &Settings) -> Result<(), String> {
    let mut clean = settings.clone();
    clean.auto_download.retain(|_, keep| *keep > 0);
    write_atomic(&path(data_dir), &clean).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_defaults() {
        let dir = std::env::temp_dir().join(format!("pinepods-settings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(load(&dir), Settings::default());

        let mut s = Settings { notify_downloads: true, ..Settings::default() };
        s.auto_download.insert(7, 3);
        s.auto_download.insert(8, 0);
        save(&dir, &s).unwrap();
        let loaded = load(&dir);
        assert!(loaded.notify_downloads);
        assert_eq!(loaded.auto_download.into_iter().collect::<Vec<_>>(), vec![(7, 3)]);

        // Unknown or missing fields don't lose the rest.
        std::fs::write(dir.join(SETTINGS_FILE), r#"{"close_to_tray":false,"future":1}"#).unwrap();
        let loaded = load(&dir);
        assert!(!loaded.close_to_tray);
        assert!(loaded.check_for_updates);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
