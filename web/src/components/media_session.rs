//! The browser's Media Session: what the OS media overlay, lock screen,
//! keyboard media keys and headsets see and control.
//!
//! `navigator.mediaSession` is missing in some webviews (WebKitGTK builds of the
//! Linux desktop app), and browsers throw for actions they don't support, so
//! every call is guarded: without the API the player just has no OS controls.

use crate::components::player_ui;
use js_sys::{Array, Function, Object, Reflect};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{MediaMetadata, MediaPositionState, MediaSession, MediaSessionPlaybackState};
use yew::prelude::*;

/// Same distance as the on-screen skip buttons and the arrow keys.
pub const SKIP_SECONDS: f64 = 15.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    Play,
    Pause,
    SeekBackward,
    SeekForward,
    PreviousTrack,
    NextTrack,
    SeekTo,
}

impl Action {
    pub const ALL: [Action; 7] = [
        Action::Play,
        Action::Pause,
        Action::SeekBackward,
        Action::SeekForward,
        Action::PreviousTrack,
        Action::NextTrack,
        Action::SeekTo,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Action::Play => "play",
            Action::Pause => "pause",
            Action::SeekBackward => "seekbackward",
            Action::SeekForward => "seekforward",
            Action::PreviousTrack => "previoustrack",
            Action::NextTrack => "nexttrack",
            Action::SeekTo => "seekto",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    Play,
    Pause,
    SeekBy(f64),
    SeekTo(f64),
}

/// What an action does to the player. `seek_offset` and `seek_time` come from
/// the browser's action details.
///
/// Previous/next track skip within the episode, as they do in the phone apps:
/// keyboards and headsets only have those keys, and listeners use them to skip
/// ads and replay a moment far more than to change episode.
pub fn effect(action: Action, seek_offset: Option<f64>, seek_time: Option<f64>) -> Option<Effect> {
    let by = seek_offset
        .filter(|s| s.is_finite() && *s > 0.0)
        .unwrap_or(SKIP_SECONDS);
    match action {
        Action::Play => Some(Effect::Play),
        Action::Pause => Some(Effect::Pause),
        Action::SeekBackward => Some(Effect::SeekBy(-by)),
        Action::SeekForward => Some(Effect::SeekBy(by)),
        Action::PreviousTrack => Some(Effect::SeekBy(-SKIP_SECONDS)),
        Action::NextTrack => Some(Effect::SeekBy(SKIP_SECONDS)),
        Action::SeekTo => seek_time
            .filter(|t| t.is_finite() && *t >= 0.0)
            .map(Effect::SeekTo),
    }
}

fn session() -> Option<MediaSession> {
    let navigator = web_sys::window()?.navigator();
    let value = Reflect::get(&navigator, &JsValue::from_str("mediaSession")).ok()?;
    if value.is_undefined() || value.is_null() {
        return None;
    }
    Some(value.unchecked_into())
}

/// `setActionHandler` through a catching call: the generated binding doesn't
/// catch, and an unsupported action throws.
fn set_handler(session: &MediaSession, action: Action, handler: Option<&Function>) {
    let Ok(set) = Reflect::get(session, &JsValue::from_str("setActionHandler")) else {
        return;
    };
    let Ok(set) = set.dyn_into::<Function>() else {
        return;
    };
    let handler = handler.map(JsValue::from).unwrap_or(JsValue::NULL);
    let _ = set.call2(session, &JsValue::from_str(action.name()), &handler);
}

fn apply(effect: Effect) {
    match effect {
        Effect::Play => player_ui::set_playing(true),
        Effect::Pause => player_ui::set_playing(false),
        Effect::SeekBy(delta) => player_ui::seek_by(delta),
        Effect::SeekTo(seconds) => player_ui::seek_to(seconds),
    }
}

/// Tell the OS where playback is, so its scrubber and remaining time are right.
/// The OS extrapolates from the rate between updates, so this is needed only on
/// seeks and play/pause/speed changes.
pub fn update_position() {
    let Some(session) = session() else { return };
    let Some((position, duration, rate)) = player_ui::position_and_rate() else {
        return;
    };
    // Throws unless the duration is known and the position is within it.
    if !duration.is_finite() || duration <= 0.0 || rate <= 0.0 {
        return;
    }
    let state = MediaPositionState::new();
    state.set_duration(duration);
    state.set_playback_rate(rate);
    state.set_position(position.clamp(0.0, duration));
    let _ = session.set_position_state_with_state(&state);
}

fn set_metadata(session: &MediaSession, title: &str, podcast: &str, artwork_url: &str) {
    let Ok(metadata) = MediaMetadata::new() else { return };
    metadata.set_title(title);
    metadata.set_artist(podcast);
    metadata.set_album(podcast);
    if !artwork_url.is_empty() {
        let image = Object::new();
        let _ = Reflect::set(&image, &JsValue::from_str("src"), &JsValue::from_str(artwork_url));
        // web-sys has no artwork setter without extra features; the property
        // setter accepts plain {src} objects.
        let _ = Reflect::set(&metadata, &JsValue::from_str("artwork"), &Array::of1(&image));
    }
    session.set_metadata(Some(&metadata));
}

/// Wire the Media Session to the player while it is mounted.
#[hook]
pub fn use_media_session(
    episode_id: i32,
    title: String,
    podcast: String,
    artwork_url: String,
    playing: bool,
    speed: f64,
) {
    use_effect_with((), |_| {
        let mut handlers: Vec<Closure<dyn FnMut(JsValue)>> = Vec::new();
        if let Some(session) = session() {
            for action in Action::ALL {
                let handler = Closure::<dyn FnMut(JsValue)>::new(move |details: JsValue| {
                    let number = |key: &str| {
                        Reflect::get(&details, &JsValue::from_str(key))
                            .ok()
                            .and_then(|v| v.as_f64())
                    };
                    if let Some(effect) = effect(action, number("seekOffset"), number("seekTime")) {
                        apply(effect);
                    }
                });
                set_handler(&session, action, Some(handler.as_ref().unchecked_ref()));
                handlers.push(handler);
            }
        }
        move || {
            if let Some(session) = session() {
                for action in Action::ALL {
                    set_handler(&session, action, None);
                }
                session.set_metadata(None);
                session.set_playback_state(MediaSessionPlaybackState::None);
            }
            drop(handlers);
        }
    });

    use_effect_with((episode_id, title, podcast, artwork_url), |(_, title, podcast, artwork)| {
        if let Some(session) = session() {
            set_metadata(&session, title, podcast, artwork);
        }
        || ()
    });

    use_effect_with((episode_id, playing, speed.to_bits()), move |_| {
        if let Some(session) = session() {
            session.set_playback_state(if playing {
                MediaSessionPlaybackState::Playing
            } else {
                MediaSessionPlaybackState::Paused
            });
        }
        update_position();
        || ()
    });
}
