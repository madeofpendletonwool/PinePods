//! The desktop app's player outside the main window: the main window publishes what's
//! playing, and the tray, the mini player and (on Linux) the system media controls send
//! commands back. Everything goes through Tauri events, so the audio stays in one place.

use crate::components::context::{AppState, EpisodeStatusState, UIState};
use crate::components::i18n::use_translation;
use crate::components::player_ui;
use crate::desktop::{self, emit, listen, PLAYER_COMMAND, PLAYER_HELLO, PLAYER_STATE};
use crate::requests::episode::Episode;
use crate::requests::pod_req::call_get_queued_episodes;
use gloo_timers::callback::Interval;
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use wasm_bindgen::JsValue;
use yew::prelude::*;
use yewdux::prelude::*;

/// What's playing, as the other windows and the tray see it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct PlayerState {
    pub episode_id: i32,
    pub title: String,
    pub podcast: String,
    pub artwork: String,
    pub playing: bool,
    pub position: f64,
    pub duration: f64,
    pub rate: f64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Command {
    pub action: String,
    #[serde(default)]
    pub value: Option<f64>,
}

thread_local! {
    static FOLLOWING: Cell<bool> = const { Cell::new(false) };
}

fn current() -> Option<PlayerState> {
    let ui = Dispatch::<UIState>::global().get();
    let playing = ui.currently_playing.as_ref()?;
    let (position, duration, rate) = player_ui::position_and_rate().unwrap_or((
        ui.current_time_seconds,
        playing.duration_sec,
        1.0,
    ));
    Some(PlayerState {
        episode_id: playing.episode_id,
        title: playing.title.clone(),
        podcast: playing.episode.podcastname.clone(),
        artwork: playing.artwork_url.clone(),
        playing: ui.audio_playing.unwrap_or(false),
        position: if position.is_finite() { position } else { 0.0 },
        duration: if duration.is_finite() { duration } else { playing.duration_sec },
        rate: if rate.is_finite() && rate > 0.0 { rate } else { 1.0 },
    })
}

fn publish() {
    emit(PLAYER_STATE, &current());
}

fn on_command(payload: JsValue) {
    let Ok(command) = serde_wasm_bindgen::from_value::<Command>(payload) else { return };
    match (command.action.as_str(), command.value) {
        ("toggle", _) => player_ui::toggle_play(),
        ("play", _) => player_ui::set_playing(true),
        ("pause", _) => player_ui::set_playing(false),
        ("seek_by", Some(delta)) => player_ui::seek_by(delta),
        ("seek_to", Some(seconds)) => player_ui::seek_to(seconds),
        ("play_episode", Some(id)) => wasm_bindgen_futures::spawn_local(play_queued(id as i32)),
        _ => {}
    }
    publish();
}

/// Play an episode from the queue (the mini player's up-next list).
async fn play_queued(episode_id: i32) {
    let app = Dispatch::<AppState>::global().get();
    let (Some(auth), Some(user)) = (app.auth_details.clone(), app.user_details.clone()) else { return };
    let Some(api_key) = auth.api_key.clone() else { return };
    let queue = call_get_queued_episodes(&auth.server_name, &Some(api_key.clone()), &user.UserID)
        .await
        .unwrap_or_default();
    let Some(episode) = queue.into_iter().find(|e| e.episodeid == episode_id) else { return };
    let is_local = Dispatch::<EpisodeStatusState>::global()
        .get()
        .downloaded_episodes
        .is_local_download(episode_id);
    let Ok(click) = web_sys::MouseEvent::new("click") else { return };
    crate::components::audio::on_play_click(
        episode,
        api_key,
        user.UserID,
        auth.server_name,
        Dispatch::<UIState>::global(),
        Dispatch::<UIState>::global().get(),
        is_local,
        false,
        None,
    )
    .emit(click);
}

/// In the main window: publish the player's state and take commands.
#[function_component(PlayerBridge)]
pub fn player_bridge() -> Html {
    let now = use_selector(|s: &UIState| {
        s.currently_playing.as_ref().map(|c| c.episode_id).zip(Some(s.audio_playing.unwrap_or(false)))
    });
    let playing = now.is_some_and(|(_, p)| p);

    use_effect_with((), |_| {
        if desktop::is_desktop() && !FOLLOWING.with(|f| f.replace(true)) {
            wasm_bindgen_futures::spawn_local(async {
                listen(PLAYER_COMMAND, on_command).await;
                listen(PLAYER_HELLO, |_| publish()).await;
            });
        }
        || ()
    });
    use_effect_with(*now, |_| {
        publish();
        || ()
    });
    // While playing, keep the position fresh for the mini player and media controls.
    use_effect_with(playing, |playing| {
        let interval = (*playing && desktop::is_desktop()).then(|| Interval::new(2_000, publish));
        move || drop(interval)
    });
    html! {}
}

fn fmt(seconds: f64) -> String {
    player_ui::fmt_time(seconds.max(0.0), seconds >= 3600.0)
}

/// The mini player window: a remote for the player in the main window.
#[function_component(MiniPlayer)]
pub fn mini_player() -> Html {
    let (i18n, _) = use_translation();
    let state = use_state(|| None::<PlayerState>);
    // When the last state arrived, to move the position along between updates.
    let received = use_mut_ref(|| 0.0_f64);
    let tick = use_state(|| 0_u32);
    let queue = use_state(Vec::<Episode>::new);
    let auth = use_selector(|s: &AppState| {
        let a = s.auth_details.as_ref();
        (
            a.map(|a| a.server_name.clone()),
            a.and_then(|a| a.api_key.clone()),
            s.user_details.as_ref().map(|u| u.UserID),
        )
    });

    {
        let state = state.clone();
        let received = received.clone();
        use_effect_with((), move |_| {
            if let Some(theme) = web_sys::window()
                .and_then(|w| w.local_storage().ok().flatten())
                .and_then(|s| s.get_item("selected_theme").ok().flatten())
            {
                crate::components::setting_components::theme_options::changeTheme(&theme);
            }
            wasm_bindgen_futures::spawn_local(async move {
                listen(PLAYER_STATE, move |payload| {
                    let next = serde_wasm_bindgen::from_value::<Option<PlayerState>>(payload).ok().flatten();
                    *received.borrow_mut() = js_sys::Date::now();
                    state.set(next);
                })
                .await;
                emit(PLAYER_HELLO, &());
            });
            || ()
        });
    }
    let playing = state.as_ref().is_some_and(|s| s.playing);
    {
        let tick = tick.clone();
        use_effect_with(playing, move |playing| {
            let interval = playing.then(|| Interval::new(500, move || tick.set(*tick + 1)));
            move || drop(interval)
        });
    }
    // Up next: the queue after the current episode, refreshed when the episode changes.
    {
        let queue = queue.clone();
        let episode_id = state.as_ref().map(|s| s.episode_id);
        use_effect_with((episode_id, (*auth).clone()), move |(episode_id, (server, key, user))| {
            if let (Some(server), Some(key), Some(user)) = (server.clone(), key.clone(), *user) {
                let current = *episode_id;
                wasm_bindgen_futures::spawn_local(async move {
                    let list = call_get_queued_episodes(&server, &Some(key), &user).await.unwrap_or_default();
                    queue.set(list.into_iter().filter(|e| Some(e.episodeid) != current).take(3).collect());
                });
            }
            || ()
        });
    }

    let command = |action: &'static str, value: Option<f64>| {
        Callback::from(move |_: MouseEvent| emit(PLAYER_COMMAND, &Command { action: action.into(), value }))
    };

    let Some(now) = (*state).clone() else {
        return html! {
            <div class="mini-player mini-player-empty">
                <i class="ph ph-headphones" aria-hidden="true"></i>
                <span>{ i18n.t("desktop.mini_nothing_playing") }</span>
            </div>
        };
    };
    let elapsed = if now.playing { (js_sys::Date::now() - *received.borrow()) / 1000.0 * now.rate } else { 0.0 };
    let position = if now.duration > 0.0 { (now.position + elapsed).min(now.duration) } else { now.position + elapsed };
    let percent = if now.duration > 0.0 { position / now.duration * 100.0 } else { 0.0 };
    let duration = now.duration;
    let on_seek = Callback::from(move |e: InputEvent| {
        let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
        if let Ok(p) = input.value().parse::<f64>() {
            emit(PLAYER_COMMAND, &Command { action: "seek_to".into(), value: Some(p / 100.0 * duration) });
        }
    });

    html! {
        <div class="mini-player">
            <div class="mini-player-main">
                if !now.artwork.is_empty() {
                    <img class="mini-player-art" src={now.artwork.clone()} alt="" />
                }
                <div class="mini-player-text">
                    <div class="mini-player-title" title={now.title.clone()}>{ &now.title }</div>
                    <div class="mini-player-podcast">{ &now.podcast }</div>
                </div>
                <div class="mini-player-controls">
                    <button type="button" class="mini-player-btn" title={i18n.t("desktop.back_15")}
                        onclick={command("seek_by", Some(-15.0))}>
                        <i class="ph ph-arrow-counter-clockwise"></i>
                    </button>
                    <button type="button" class="mini-player-btn mini-player-play"
                        title={if now.playing { i18n.t("desktop.pause") } else { i18n.t("desktop.play") }}
                        onclick={command("toggle", None)}>
                        <i class={if now.playing { "ph-fill ph-pause" } else { "ph-fill ph-play" }}></i>
                    </button>
                    <button type="button" class="mini-player-btn" title={i18n.t("desktop.forward_15")}
                        onclick={command("seek_by", Some(15.0))}>
                        <i class="ph ph-arrow-clockwise"></i>
                    </button>
                </div>
            </div>
            <div class="mini-player-seek">
                <span>{ fmt(position) }</span>
                <input type="range" min="0" max="100" step="0.1" value={format!("{percent:.1}")} oninput={on_seek}
                    aria-label={i18n.t("desktop.position")} />
                <span>{ fmt(now.duration) }</span>
            </div>
            if !queue.is_empty() {
                <div class="mini-player-queue">
                    <div class="mini-player-queue-head">{ i18n.t("desktop.up_next") }</div>
                    { for queue.iter().map(|ep| {
                        let id = ep.episodeid;
                        html! {
                            <button type="button" class="mini-player-queue-item" title={ep.episodetitle.clone()}
                                onclick={Callback::from(move |_: MouseEvent| emit(PLAYER_COMMAND, &Command { action: "play_episode".into(), value: Some(id as f64) }))}>
                                <i class="ph ph-play" aria-hidden="true"></i>
                                <span>{ &ep.episodetitle }</span>
                            </button>
                        }
                    }) }
                </div>
            }
        </div>
    }
}

/// Root of the mini player window: the player, in the app's language.
#[function_component(MiniPlayerRoot)]
pub fn mini_player_root() -> Html {
    html! {
        <crate::components::i18n::TranslationProvider translations={crate::components::i18n::all_translations()} default_language="en">
            <MiniPlayer />
        </crate::components::i18n::TranslationProvider>
    }
}
