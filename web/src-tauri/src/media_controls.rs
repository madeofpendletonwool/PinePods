//! System media controls (MPRIS) for Linux webviews without a Media Session.
//!
//! The player in the main window publishes its state as `desktop://player-state`; media
//! keys and the desktop's media widget answer with `desktop://player-command`.

use crate::{player_command, PLAYER_STATE};
use souvlaki::{MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig, SeekDirection};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tauri::{AppHandle, Listener, Runtime};

static CONTROLS: OnceLock<Mutex<MediaControls>> = OnceLock::new();

#[derive(serde::Deserialize, Default)]
#[serde(default)]
struct State {
    title: String,
    podcast: String,
    artwork: String,
    playing: bool,
    position: f64,
    duration: f64,
}

pub fn start<R: Runtime>(app: &AppHandle<R>) {
    if CONTROLS.get().is_some() {
        return;
    }
    let config = PlatformConfig { dbus_name: "pinepods", display_name: "Pinepods", hwnd: None };
    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("no system media controls: {e:?}");
            return;
        }
    };
    let handle = app.clone();
    let attached = controls.attach(move |event| match event {
        MediaControlEvent::Play => player_command(&handle, "play", None),
        MediaControlEvent::Pause => player_command(&handle, "pause", None),
        MediaControlEvent::Toggle => player_command(&handle, "toggle", None),
        MediaControlEvent::Next | MediaControlEvent::Seek(SeekDirection::Forward) => {
            player_command(&handle, "seek_by", Some(15.0))
        }
        MediaControlEvent::Previous | MediaControlEvent::Seek(SeekDirection::Backward) => {
            player_command(&handle, "seek_by", Some(-15.0))
        }
        MediaControlEvent::SeekBy(SeekDirection::Forward, d) => player_command(&handle, "seek_by", Some(d.as_secs_f64())),
        MediaControlEvent::SeekBy(SeekDirection::Backward, d) => {
            player_command(&handle, "seek_by", Some(-d.as_secs_f64()))
        }
        MediaControlEvent::SetPosition(MediaPosition(p)) => player_command(&handle, "seek_to", Some(p.as_secs_f64())),
        _ => {}
    });
    if let Err(e) = attached {
        log::warn!("no system media controls: {e:?}");
        return;
    }
    if CONTROLS.set(Mutex::new(controls)).is_err() {
        return;
    }
    app.listen_any(PLAYER_STATE, |event| {
        let Some(controls) = CONTROLS.get() else { return };
        let mut controls = controls.lock().unwrap();
        let state: Option<State> = serde_json::from_str(event.payload()).unwrap_or_default();
        let Some(state) = state else {
            let _ = controls.set_playback(MediaPlayback::Stopped);
            return;
        };
        let _ = controls.set_metadata(MediaMetadata {
            title: Some(&state.title),
            artist: Some(&state.podcast),
            album: Some(&state.podcast),
            cover_url: (!state.artwork.is_empty()).then_some(state.artwork.as_str()),
            duration: (state.duration > 0.0).then(|| Duration::from_secs_f64(state.duration)),
        });
        let progress = Some(MediaPosition(Duration::from_secs_f64(state.position.max(0.0))));
        let _ = controls.set_playback(if state.playing {
            MediaPlayback::Playing { progress }
        } else {
            MediaPlayback::Paused { progress }
        });
    });
    log::info!("system media controls on");
}
