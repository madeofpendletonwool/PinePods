// Speaker picker, grouping, and the now-playing bar for Music Assistant.
// Rendered only while the per-user bridge is enabled.

use std::collections::HashSet;
use std::rc::Rc;

use gloo_events::EventListener;
use gloo_timers::callback::Interval;
use serde_json::{json, Value};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlElement, HtmlInputElement, KeyboardEvent};
use yew::prelude::*;
use yewdux::prelude::*;

use crate::components::chapter_list::ChapterList;
use crate::components::gen_components::EpisodeModal;
use crate::components::people::{PeopleLayout, PeopleStrip};
use crate::components::context::{
    current_playing_anchor, AppState, EpisodeDetailState, EpisodeNavigationState, NotificationState,
    UIState,
};
use crate::requests::music_assistant::{
    choose_playback, explain_test, format_clock, is_music_assistant, is_nothing_next,
    note_relay_transport, play_episode_args,
    plan_group, playback_state_for, player_name, refresh_music_assistant, send_player_command,
    for_episode, member_of, progress_percent, send_to_speaker_allowed, sleep_args, sleep_remaining,
    SleepChoice, SLEEP_CHOICES, speaker_key_command, speaker_playlist, speaker_start, transport_is_recent,
    transport_shown, unix_now,
    GroupPlanError, MaPlayer,
    MusicAssistantState, PlaybackCandidate, PlaybackFocus, playing_speakers, started_speaker,
};
use crate::requests::notify::{toast_error, toast_info};
use crate::requests::now_playing::{send_command, NowPlayingDevice, NowPlayingState, SleepTimer};
use crate::requests::pod_req::{
    call_fetch_podcasting_2_data, call_get_episode_metadata, call_get_play_episode_details,
    Chapter, EpisodeRequest, FetchPodcasting2DataRequest, Transcript,
};
use crate::pages::episode::TranscriptInline;
use crate::requests::episode::Episode;
use crate::requests::people_req::CreditedPerson;
use crate::components::i18n::use_translation;
use yew_router::history::{BrowserHistory, History};

fn session() -> Option<(String, String, i32)> {
    let app = Dispatch::<AppState>::global().get();
    let auth = app.auth_details.as_ref()?;
    let api_key = auth.api_key.clone()?;
    let user_id = app.user_details.as_ref()?.UserID;
    Some((auth.server_name.clone(), api_key, user_id))
}

fn toast_failure(prefix: &str, err: &anyhow::Error) {
    let explained = explain_test("", Err(&err.to_string()));
    let detail = if explained.detail.is_empty() {
        err.to_string()
    } else {
        explained.detail
    };
    toast_error(
        &Dispatch::<NotificationState>::global(),
        format!("{prefix}: {detail}"),
    );
}

fn run_http(
    player_id: String,
    action: &'static str,
    args: Value,
    failure: String,
    pause_local: bool,
) {
    let Some((server, api_key, user_id)) = session() else {
        return;
    };
    spawn_local(async move {
        match send_player_command(&server, &api_key, user_id, &player_id, action, args).await {
            Ok(resp) => {
                if pause_local {
                    let ui = Dispatch::<UIState>::global();
                    if let Some(media) = ui.get().media_element.as_ref() {
                        let _ = media.pause();
                    }
                    ui.reduce_mut(|s| s.audio_playing = Some(false));
                }
                if !resp.warnings.is_empty() {
                    toast_info(
                        &Dispatch::<NotificationState>::global(),
                        resp.warnings.join(" "),
                    );
                }
                refresh_music_assistant();
            }
            Err(err) => toast_failure(&failure, &err),
        }
    });
}

pub(crate) fn now_unix() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

fn set_player_class(active: bool) {
    let Some(body) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.body())
    else {
        return;
    };
    if active {
        let _ = body.class_list().add_1("audio-player-active");
    } else {
        let _ = body.class_list().remove_1("audio-player-active");
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct VolumeProps {
    pub(crate) device_id: String,
    pub(crate) level: f64,
    pub(crate) muted: bool,
    /// Group volume applies to every member. Speaker volume is just this player.
    pub(crate) group: bool,
    pub(crate) label: String,
}

#[function_component(VolumeControl)]
pub(crate) fn volume_control(props: &VolumeProps) -> Html {
    let (i18n, _) = use_translation();
    let action = if props.group {
        "group_volume"
    } else {
        "volume_set"
    };
    let on_vol = {
        let id = props.device_id.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(level) = input.value().parse::<f64>() {
                    send_command(&id, action, json!({ "volume_level": level }));
                }
            }
        })
    };
    let on_mute = {
        let id = props.device_id.clone();
        let muted = props.muted;
        Callback::from(move |_: MouseEvent| {
            send_command(&id, "volume_mute", json!({ "muted": !muted }));
        })
    };
    let mute_label = if props.muted {
        i18n.t("music_assistant.unmute").to_string()
    } else {
        i18n.t("music_assistant.mute").to_string()
    };
    html! {
        <div class="ma-volume">
            <span class="ma-volume-label">{ props.label.clone() }</span>
            <input
                type="range"
                min="0"
                max="100"
                step="1"
                value={props.level.to_string()}
                onchange={on_vol}
                aria-label={props.label.clone()}
            />
            <span class="ma-volume-level">{ format!("{:.0}", props.level) }</span>
            <button type="button" class="iconbtn" title={mute_label} onclick={on_mute}>
                <i class={ if props.muted { "ph ph-speaker-slash" } else { "ph ph-speaker-high" } }></i>
            </button>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub(crate) struct SeekSliderProps {
    pub(crate) device_id: String,
    pub(crate) shown: f64,
    pub(crate) duration: f64,
    pub(crate) label: String,
    #[prop_or(classes!("ma-seek"))]
    pub(crate) class: Classes,
}

/// Range that keeps the thumb where the drag left it, then seeks on release.
/// The parent keeps passing the interpolated position, which the thumb follows
/// only while it is not being dragged.
#[function_component(SeekSlider)]
pub(crate) fn seek_slider(props: &SeekSliderProps) -> Html {
    let held = use_state(|| None::<f64>);
    let oninput = {
        let held = held.clone();
        Callback::from(move |e: InputEvent| {
            let Some(input) = e.target_dyn_into::<HtmlInputElement>() else {
                return;
            };
            if let Ok(pos) = input.value().parse::<f64>() {
                held.set(Some(pos.max(0.0)));
            }
        })
    };
    let onchange = {
        let held = held.clone();
        let id = props.device_id.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                if let Ok(pos) = input.value().parse::<f64>() {
                    send_command(
                        &id,
                        "seek",
                        json!({ "position_sec": integer_position(pos) }),
                    );
                }
            }
            held.set(None);
        })
    };
    let shown = if props.shown.is_finite() {
        props.shown.clamp(0.0, props.duration.max(0.0))
    } else {
        0.0
    };
    let value = match *held {
        Some(pos) => pos.clamp(0.0, props.duration.max(0.0)),
        None => shown,
    };
    html! {
        <input
            class={props.class.clone()}
            type="range"
            min="0"
            max={props.duration.to_string()}
            step="1"
            value={value.to_string()}
            oninput={oninput}
            onchange={onchange}
            aria-label={props.label.clone()}
        />
    }
}

fn integer_position(pos: f64) -> i64 {
    if !pos.is_finite() || pos <= 0.0 {
        0
    } else {
        pos.trunc() as i64
    }
}

/// Playback follows the sync parent. A group row is its own leader, which is
/// the same player the group volume control targets.
fn transport_player_id(player: &MaPlayer) -> String {
    player
        .synced_to
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .unwrap_or(player.player_id.as_str())
        .to_string()
}

fn player_id_from_device(device_id: &str) -> Option<String> {
    let mut parts = device_id.splitn(3, ':');
    let prefix = parts.next()?;
    if !prefix.eq_ignore_ascii_case("ma") {
        return None;
    }
    let _user = parts.next()?;
    let player = parts.next()?.trim();
    if player.is_empty() {
        None
    } else {
        Some(player.to_string())
    }
}

fn state_i18n_key(state: &str) -> Option<&'static str> {
    if state.eq_ignore_ascii_case("playing") || state.eq_ignore_ascii_case("buffering") {
        Some("music_assistant.state_playing")
    } else if state.eq_ignore_ascii_case("paused") {
        Some("music_assistant.state_paused")
    } else if state.is_empty()
        || state.eq_ignore_ascii_case("idle")
        || state.eq_ignore_ascii_case("off")
        || state.eq_ignore_ascii_case("standby")
    {
        Some("music_assistant.state_idle")
    } else {
        None
    }
}

/// Fixed bar shown when local playback is idle and an MA player has an episode.
#[function_component(MaNowPlayingBar)]
pub fn ma_now_playing_bar() -> Html {
    let (i18n, _) = use_translation();
    let (ma, ma_dispatch) = use_store::<MusicAssistantState>();
    let (np, _) = use_store::<NowPlayingState>();
    let tick = use_state(|| 0u32);
    // Collapsed again whenever the bar goes away, so a later local player or
    // speaker never opens fullscreen on its own.
    let expanded = use_state(|| false);
    // Chapters and people of the speaker's episode, tagged with that episode so
    // a late response for the previous one is never shown.
    let chapters = use_state(|| (0, Rc::new(Vec::<Chapter>::new())));
    let people = use_state(|| (0, Rc::new(Vec::<CreditedPerson>::new())));
    let transcripts = use_state(|| (0, Rc::new(Vec::<Transcript>::new())));
    let chapters_open = use_state(|| false);
    let transcript_open = use_state(|| false);
    let sleep_open = use_state(|| false);
    // Show notes are fetched when asked for, for the episode then playing.
    let notes = use_state(|| None::<Episode>);

    let candidates: Vec<PlaybackCandidate> =
        np.ma_devices.iter().map(PlaybackCandidate::from).collect();
    let device: Option<NowPlayingDevice> = if ma.enabled {
        choose_playback(&candidates, ma.focus.speaker_id()).and_then(|c| {
            np.ma_devices
                .iter()
                .find(|d| d.device_id == c.device_id)
                .cloned()
        })
    } else {
        None
    };

    {
        // Space and the arrow keys control the speaker while this bar is shown,
        // as they do the local player. The listener goes when the bar does.
        let keys = device.as_ref().map(|d| {
            let supported: Vec<&'static str> = ["play", "pause", "skip_forward", "skip_back"]
                .into_iter()
                .filter(|a| d.supports(a))
                .collect();
            (d.device_id.clone(), d.playing, supported)
        });
        use_effect_with(keys, move |keys| {
            let listener = keys.clone().and_then(|(id, playing, supported)| {
                let win = web_sys::window()?;
                Some(EventListener::new(&win, "keydown", move |event| {
                    let Some(event) = event.dyn_ref::<KeyboardEvent>() else {
                        return;
                    };
                    let typing = event
                        .target()
                        .and_then(|t| t.dyn_into::<HtmlElement>().ok())
                        .is_some_and(|el| {
                            el.is_content_editable()
                                || matches!(
                                    el.tag_name().to_ascii_lowercase().as_str(),
                                    "input" | "textarea" | "select"
                                )
                        });
                    if typing {
                        return;
                    }
                    let modified = event.alt_key() || event.ctrl_key() || event.meta_key();
                    let Some((action, args)) =
                        speaker_key_command(&event.key(), playing, modified)
                    else {
                        return;
                    };
                    if supported.contains(&action) {
                        event.prevent_default();
                        send_command(&id, action, args);
                    }
                }))
            });
            move || drop(listener)
        });
    }

    {
        let chapters = chapters.clone();
        let people = people.clone();
        let transcripts = transcripts.clone();
        let episode = device
            .as_ref()
            .filter(|d| d.episode_id > 0 && !d.is_youtube)
            .map(|d| d.episode_id);
        use_effect_with(episode, move |episode| {
            if let (Some(episode_id), Some((server, api_key, user_id))) = (*episode, session()) {
                spawn_local(async move {
                    let request = FetchPodcasting2DataRequest { episode_id, user_id };
                    if let Ok(data) =
                        call_fetch_podcasting_2_data(&server, &Some(api_key), &request).await
                    {
                        chapters.set((episode_id, Rc::new(data.chapters)));
                        people.set((episode_id, Rc::new(data.people)));
                        transcripts.set((episode_id, Rc::new(data.transcripts)));
                    }
                });
            }
            || ()
        });
    }

    let playing = device.as_ref().map(|d| d.playing).unwrap_or(false);
    {
        let tick = tick.clone();
        use_effect_with(playing, move |playing| {
            let iv = if *playing {
                let tick = tick.clone();
                Some(Interval::new(1_000, move || {
                    let next = *tick + 1;
                    tick.set(next);
                }))
            } else {
                None
            };
            move || drop(iv)
        });
    }
    {
        let active = device.is_some();
        use_effect_with(active, move |active| {
            set_player_class(*active);
            move || set_player_class(false)
        });
    }
    let _ = *tick;

    let Some(device) = device else {
        return html! {};
    };

    // Speakers play at 1x. Music Assistant only tempo-shifts podcast episodes
    // and audiobooks, and these episodes are sent as tracks.
    let shown = crate::requests::music_assistant::displayed_position(
        device.position_sec,
        device.playing,
        1.0,
        device.updated_at,
        now_unix(),
    );
    let duration = if device.duration_sec.is_finite() {
        device.duration_sec.max(0.0)
    } else {
        0.0
    };
    let elapsed = format_clock(if duration > 0.0 { shown.min(duration) } else { shown });
    let total = format_clock(duration);
    let percent = progress_percent(shown, duration);
    let toggle_expanded = {
        let expanded = expanded.clone();
        Callback::from(move |_: MouseEvent| expanded.set(!*expanded))
    };
    let episode_chapters = for_episode(&chapters, device.episode_id);
    let cover = device.artwork_url.clone();
    let chapter_list = (*chapters_open && !episode_chapters.is_empty()).then(|| {
        let seek_id = device.device_id.clone();
        let toggle_id = device.device_id.clone();
        let is_playing = device.playing;
        let chapters_open = chapters_open.clone();
        html! {
            <ChapterList
                chapters={episode_chapters.clone()}
                position={shown}
                duration={duration}
                playing={is_playing}
                on_seek={Callback::from(move |start: f64| {
                    send_command(&seek_id, "seek", json!({ "position_sec": integer_position(start) }));
                })}
                on_toggle={Callback::from(move |_: ()| {
                    send_command(&toggle_id, if is_playing { "pause" } else { "play" }, json!({}));
                })}
                on_close={Callback::from(move |_: ()| chapters_open.set(false))}
            />
        }
    });
    let episode_people = for_episode(&people, device.episode_id);
    let episode_transcripts = for_episode(&transcripts, device.episode_id);
    let transcript_panel = (*transcript_open && !episode_transcripts.is_empty())
        .then(session)
        .flatten()
        .map(|(server, api_key, _)| {
            let close = {
                let transcript_open = transcript_open.clone();
                Callback::from(move |_: MouseEvent| transcript_open.set(false))
            };
            let seek_id = device.device_id.clone();
            let can_seek = device.supports("seek");
            html! {
                <div class="chapter-select-modal fixed top-0 right-0 left-0 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25"
                    onclick={close.clone()}>
                    <div class="modal-container relative p-4 w-full max-w-2xl max-h-full rounded-lg shadow"
                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <div class="flex items-center justify-between p-4 border-b rounded-t">
                            <h3 class="text-xl font-semibold">{ i18n.t("episode.transcript") }</h3>
                            <button type="button" class="player-btn" title={i18n.t("common.close_modal").to_string()} onclick={close}>
                                <i class="ph ph-x"></i>
                            </button>
                        </div>
                        <div class="p-4 max-h-[70vh] overflow-y-auto">
                            <TranscriptInline
                                transcripts={(*episode_transcripts).clone()}
                                server_name={server}
                                api_key={api_key}
                                on_seek={can_seek.then(|| Callback::from(move |(t, _): (f64, MouseEvent)| {
                                    send_command(&seek_id, "seek", json!({ "position_sec": integer_position(t) }));
                                }))}
                            />
                        </div>
                    </div>
                </div>
            }
        });
    let open_transcript = {
        let transcript_open = transcript_open.clone();
        Callback::from(move |_: MouseEvent| transcript_open.set(true))
    };
    let open_notes = {
        let notes = notes.clone();
        let (episode_id, is_youtube) = (device.episode_id, device.is_youtube);
        let failure = i18n.t("music_assistant.command_failed").to_string();
        Callback::from(move |_: MouseEvent| {
            let Some((server, api_key, user_id)) = session() else {
                return;
            };
            let (notes, failure) = (notes.clone(), failure.clone());
            spawn_local(async move {
                let request = EpisodeRequest {
                    episode_id,
                    user_id,
                    person_episode: false,
                    is_youtube,
                };
                match call_get_episode_metadata(&server, Some(api_key), &request).await {
                    Ok(episode) => notes.set(Some(episode)),
                    Err(err) => toast_failure(&failure, &err),
                }
            });
        })
    };
    let notes_modal = (*notes)
        .clone()
        .filter(|episode| episode.episodeid == device.episode_id)
        .map(|episode| {
            let close = {
                let notes = notes.clone();
                Callback::from(move |_: MouseEvent| notes.set(None))
            };
            let to_episode_page = {
                let notes = notes.clone();
                let expanded = expanded.clone();
                let (episode_id, is_youtube) = (episode.episodeid, episode.is_youtube);
                Callback::from(move |_: MouseEvent| {
                    notes.set(None);
                    expanded.set(false);
                    Dispatch::<EpisodeNavigationState>::global()
                        .reduce_mut(|s| s.selected_episode_id = Some(episode_id));
                    Dispatch::<EpisodeDetailState>::global().reduce_mut(|s| {
                        s.person_episode = Some(false);
                        s.fetched_episode = None;
                    });
                    let youtube = if is_youtube { "&youtube=true" } else { "" };
                    BrowserHistory::new().push(format!("/episode?episode_id={episode_id}{youtube}"));
                })
            };
            html! {
                <EpisodeModal
                    episode_id={episode.episodeid}
                    episode_url={episode.episodeurl.clone()}
                    episode_artwork={episode.episodeartwork.clone()}
                    episode_title={episode.episodetitle.clone()}
                    description={episode.episodedescription.clone()}
                    format_release={episode.episodepubdate.clone()}
                    duration={duration as i32}
                    on_close={close}
                    on_show_notes={to_episode_page}
                    listen_duration_percentage={progress_percent(shown, duration) as i32}
                    is_youtube={episode.is_youtube}
                    is_video={false}
                />
            }
        });
    let sleep_label = match device.sleep_timer {
        Some(SleepTimer::Minutes { .. }) => {
            sleep_remaining(device.sleep_timer, now_unix()).map(|left| format_clock(left as f64))
        }
        Some(SleepTimer::EndOfEpisode) => {
            Some(i18n.t("music_assistant.sleep_end_of_episode").to_string())
        }
        Some(SleepTimer::Episodes { left }) => Some(
            i18n.t("music_assistant.sleep_episodes_left")
                .to_string()
                .replace("{n}", &left.to_string()),
        ),
        None => None,
    };
    let sleep_menu = (*sleep_open).then(|| {
        let close = {
            let sleep_open = sleep_open.clone();
            Callback::from(move |_: MouseEvent| sleep_open.set(false))
        };
        let current = match device.sleep_timer {
            None => Some(SleepChoice::Off),
            Some(SleepTimer::EndOfEpisode) => Some(SleepChoice::EndOfEpisode),
            Some(SleepTimer::Episodes { left }) => Some(SleepChoice::Episodes(left)),
            Some(SleepTimer::Minutes { .. }) => None,
        };
        html! {
            <div class="chapter-select-modal fixed top-0 right-0 left-0 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25"
                onclick={close.clone()}>
                <div class="modal-container relative p-4 w-full max-w-sm max-h-full rounded-lg shadow"
                    onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                    <div class="flex items-center justify-between p-4 border-b rounded-t">
                        <h3 class="text-xl font-semibold">{ i18n.t("music_assistant.sleep_timer") }</h3>
                        <button type="button" class="player-btn" title={i18n.t("common.close_modal").to_string()} onclick={close}>
                            <i class="ph ph-x"></i>
                        </button>
                    </div>
                    <div class="p-2">
                        { for SLEEP_CHOICES.iter().map(|choice| {
                            let label = match *choice {
                                SleepChoice::Off => i18n.t("music_assistant.sleep_off").to_string(),
                                SleepChoice::Minutes(n) => i18n.t("music_assistant.sleep_minutes").to_string().replace("{n}", &n.to_string()),
                                SleepChoice::EndOfEpisode => i18n.t("music_assistant.sleep_end_of_episode").to_string(),
                                SleepChoice::Episodes(n) => i18n.t("music_assistant.sleep_episodes").to_string().replace("{n}", &n.to_string()),
                            };
                            let pick = {
                                let id = device.device_id.clone();
                                let sleep_open = sleep_open.clone();
                                let choice = *choice;
                                Callback::from(move |_: MouseEvent| {
                                    send_command(&id, "sleep_timer", sleep_args(choice));
                                    sleep_open.set(false);
                                })
                            };
                            html! {
                                <button type="button"
                                    class={classes!("ma-sleep-choice", (current == Some(*choice)).then_some("is-current"))}
                                    onclick={pick}>
                                    { label }
                                </button>
                            }
                        }) }
                    </div>
                </div>
            </div>
        }
    });
    let open_chapters = {
        let chapters_open = chapters_open.clone();
        Callback::from(move |_: MouseEvent| chapters_open.set(true))
    };
    let kicker = i18n
        .t("music_assistant.playing_on_ma")
        .to_string()
        .replace("{name}", &device.device_name);
    let title = if device.title.is_empty() {
        i18n.t("music_assistant.now_playing").to_string()
    } else {
        device.title.clone()
    };
    let id = device.device_id.clone();
    let is_playing = device.playing;
    let on_play_pause = {
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            send_command(&id, if is_playing { "pause" } else { "play" }, json!({}));
        })
    };
    let on_stop = {
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            send_command(&id, "stop", json!({}));
        })
    };
    let on_back = {
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            send_command(&id, "skip_back", json!({ "seconds": 15 }));
        })
    };
    let on_fwd = {
        let id = id.clone();
        Callback::from(move |_: MouseEvent| {
            send_command(&id, "skip_forward", json!({ "seconds": 15 }));
        })
    };
    let on_next = {
        let id = id.clone();
        let failure = i18n.t("music_assistant.command_failed").to_string();
        let nothing_next = i18n.t("music_assistant.nothing_next").to_string();
        Callback::from(move |_: MouseEvent| {
            let Some((server, api_key, user_id)) = session() else {
                return;
            };
            let (id, failure, nothing_next) = (id.clone(), failure.clone(), nothing_next.clone());
            spawn_local(async move {
                match send_player_command(&server, &api_key, user_id, &id, "next", json!({})).await {
                    Ok(resp) if is_nothing_next(&resp) => {
                        toast_info(&Dispatch::<NotificationState>::global(), nothing_next);
                    }
                    Ok(_) => refresh_music_assistant(),
                    Err(err) => toast_failure(&failure, &err),
                }
            });
        })
    };
    let open_picker = {
        let ma_dispatch = ma_dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            ma_dispatch.reduce_mut(|s| {
                s.pending_episode = None;
                s.picker_open = true;
            });
        })
    };
    let members = device.group_members.clone().unwrap_or_default();
    let volume = device.volume_level.unwrap_or(0.0);
    let muted = device.volume_muted.unwrap_or(false);
    let volume_label = if members.is_empty() {
        i18n.t("music_assistant.volume").to_string()
    } else {
        i18n.t("music_assistant.group_volume").to_string()
    };

    let can_seek = duration > 0.0 && device.supports("seek");
    let play_title = if is_playing {
        i18n.t("music_assistant.pause").to_string()
    } else {
        i18n.t("music_assistant.play").to_string()
    };
    let scrub = |class: &'static str| {
        html! {
            <>
                <span class="player-time">{ elapsed.clone() }</span>
                if can_seek {
                    <SeekSlider
                        device_id={id.clone()}
                        shown={shown}
                        duration={duration}
                        label={i18n.t("music_assistant.seek").to_string()}
                        class={classes!(class)}
                    />
                } else {
                    <span class="player-range" />
                }
                <span class="player-time">{ total.clone() }</span>
            </>
        }
    };
    let volume_controls = html! {
        <>
            if device.supports(if members.is_empty() { "volume_set" } else { "group_volume" }) {
                <VolumeControl
                    device_id={id.clone()}
                    level={volume}
                    muted={muted}
                    group={!members.is_empty()}
                    label={volume_label}
                />
            }
            if !members.is_empty() {
                <div class="ma-members">
                    <div class="ma-volume-label">{ i18n.t("music_assistant.members") }</div>
                    { for members.iter().filter_map(|member| {
                        let player = ma.players.iter().find(|p| p.player_id == *member)?;
                        if !player.available {
                            return None;
                        }
                        let label = if player.name.is_empty() { member.clone() } else { player.name.clone() };
                        Some(html! {
                            <VolumeControl
                                key={player.player_id.clone()}
                                device_id={player.device_id.clone()}
                                level={player.volume_level.unwrap_or(0.0)}
                                muted={player.volume_muted}
                                group={false}
                                label={label}
                            />
                        })
                    }) }
                </div>
            }
        </>
    };

    // Same bar family as the local player: the collapsed `.player-bar` and the
    // fullscreen `.player-fs`, switched by `.expanded`.
    html! {
        <>
        { for chapter_list }
        { for transcript_panel }
        { for sleep_menu }
        <div class={classes!("audio-player", "ma-now", (*expanded).then_some("expanded"))}>
            <div class="player-fs-backdrop"
                style={format!("background-image: url('{}');", device.artwork_url)}
            />
            <div class="player-fs-overlay" />
            <div class="player-fs">
                <div class="player-fs-topbar">
                    <button type="button" class="player-btn" title={i18n.t("music_assistant.collapse").to_string()} onclick={toggle_expanded.clone()}>
                        <i class="ph ph-caret-down" style="font-size:24px;"></i>
                    </button>
                    <span class="player-fs-eyebrow">{ kicker.clone() }</span>
                    <div class="player-fs-topbar-right">
                        if device.supports("sleep_timer") {
                            <button type="button" class="player-btn ma-sleep-btn" title={i18n.t("music_assistant.sleep_timer").to_string()}
                                onclick={{
                                    let sleep_open = sleep_open.clone();
                                    Callback::from(move |_: MouseEvent| sleep_open.set(true))
                                }}>
                                <i class={ if sleep_label.is_some() { "ph ph-moon-stars" } else { "ph ph-moon" } } style="font-size:22px;"></i>
                                if let Some(label) = sleep_label.clone() {
                                    <span class="ma-sleep-label">{ label }</span>
                                }
                            </button>
                        }
                        <button type="button" class="player-btn" title={i18n.t("music_assistant.speakers").to_string()} onclick={open_picker.clone()}>
                            <i class="ph ph-speaker-high" style="font-size:22px;"></i>
                        </button>
                        <button type="button" class="player-btn" title={i18n.t("queue_panel.up_next").to_string()} onclick={Callback::from(|_: MouseEvent| {
                            Dispatch::<UIState>::global().reduce_mut(|s| s.queue_panel_open = !s.queue_panel_open);
                        })}>
                            <i class="ph ph-queue" style="font-size:22px;"></i>
                        </button>
                    </div>
                </div>
                <div class="player-fs-main">
                    <div class="player-fs-cover-slot">
                        if !cover.is_empty() {
                            <img class="player-fs-art" src={cover} alt="" />
                        }
                    </div>
                    <div class="player-fs-meta">
                        <div class="player-fs-title">{ title.clone() }</div>
                        if !episode_people.is_empty() {
                            <PeopleStrip
                                people={episode_people.clone()}
                                layout={PeopleLayout::Chips}
                                class={classes!("player-fs-people")}
                                on_open={{
                                    let expanded = expanded.clone();
                                    Callback::from(move |_: ()| expanded.set(false))
                                }}
                            />
                        }
                    </div>
                    <div class="player-fs-scrub-area">{ scrub("player-range") }</div>
                    <div class="player-fs-controls">
                        <div class="player-fs-side-btns player-fs-side-btns--left">
                            if device.supports("stop") {
                                <button type="button" class="player-fs-ctrl" title={i18n.t("music_assistant.stop").to_string()} onclick={on_stop.clone()}>
                                    <i class="ph ph-stop"></i>
                                </button>
                            }
                            if device.supports("skip_back") {
                                <button type="button" class="player-fs-ctrl" title={i18n.t("music_assistant.skip_back").to_string()} onclick={on_back.clone()}>
                                    <i class="ph ph-rewind"></i>
                                </button>
                            }
                        </div>
                        if device.supports(if is_playing { "pause" } else { "play" }) {
                            <button type="button" class="player-fs-ctrl player-fs-ctrl-big" title={play_title.clone()} onclick={on_play_pause.clone()}>
                                <i class={ if is_playing { "ph ph-pause-circle" } else { "ph ph-play-circle" } }></i>
                            </button>
                        }
                        <div class="player-fs-side-btns">
                            if device.supports("skip_forward") {
                                <button type="button" class="player-fs-ctrl" title={i18n.t("music_assistant.skip_forward").to_string()} onclick={on_fwd.clone()}>
                                    <i class="ph ph-fast-forward"></i>
                                </button>
                            }
                            if device.supports("next") {
                                <button type="button" class="player-fs-ctrl" title={i18n.t("music_assistant.next").to_string()} onclick={on_next.clone()}>
                                    <i class="ph ph-skip-forward"></i>
                                </button>
                            }
                        </div>
                    </div>
                    if device.episode_id > 0 {
                        <div class="player-fs-footer">
                            <button type="button" class="player-btn" title={i18n.t("audio.shownotes").to_string()} onclick={open_notes}>
                                <i class="ph ph-article"></i>
                            </button>
                            if !episode_chapters.is_empty() && device.supports("seek") {
                                <button type="button" class="player-btn" title={i18n.t("audio.chapters").to_string()} onclick={open_chapters}>
                                    <i class="ph ph-list-numbers"></i>
                                </button>
                            }
                            if !episode_transcripts.is_empty() {
                                <button type="button" class="player-btn" title={i18n.t("episode.transcript").to_string()} onclick={open_transcript}>
                                    <i class="ph ph-closed-captioning"></i>
                                </button>
                            }
                        </div>
                    }
                    <div class="player-fs-footer ma-fs-footer">{ volume_controls }</div>
                </div>
            </div>
            <div class="player-bar">
                <div class="mobile-progress-container">
                    <div class="mobile-progress-bar" style={format!("width: {}%", percent)} />
                </div>
                <div class="player-left" onclick={toggle_expanded.clone()}>
                    <div class="artwork-container">
                        if !device.artwork_url.is_empty() {
                            <img class={classes!("artwork", is_playing.then_some("playing"))} src={device.artwork_url.clone()} alt="" />
                        }
                    </div>
                    <div class="player-info">
                        <div class="ma-now-kicker">{ kicker }</div>
                        <div class="player-title">{ title }</div>
                    </div>
                </div>
                <div class="player-center">
                    <div class="player-buttons">
                        if device.supports("skip_back") {
                            <button type="button" class="player-btn" title={i18n.t("music_assistant.skip_back").to_string()} onclick={on_back}>
                                <i class="ph ph-rewind"></i>
                            </button>
                        }
                        if device.supports(if is_playing { "pause" } else { "play" }) {
                            <button type="button" class="player-btn play" title={play_title} onclick={on_play_pause}>
                                <i class={ if is_playing { "ph ph-pause" } else { "ph ph-play" } }></i>
                            </button>
                        }
                        if device.supports("skip_forward") {
                            <button type="button" class="player-btn" title={i18n.t("music_assistant.skip_forward").to_string()} onclick={on_fwd}>
                                <i class="ph ph-fast-forward"></i>
                            </button>
                        }
                        if device.supports("next") {
                            <button type="button" class="player-btn" title={i18n.t("music_assistant.next").to_string()} onclick={on_next}>
                                <i class="ph ph-skip-forward"></i>
                            </button>
                        }
                        if device.supports("stop") {
                            <button type="button" class="player-btn" title={i18n.t("music_assistant.stop").to_string()} onclick={on_stop}>
                                <i class="ph ph-stop"></i>
                            </button>
                        }
                    </div>
                    <div class="player-scrub">{ scrub("player-range") }</div>
                </div>
                <div class="player-right">
                    <button type="button" class="player-btn" title={i18n.t("music_assistant.speakers").to_string()} onclick={open_picker}>
                        <i class="ph ph-speaker-high"></i>
                    </button>
                    <button type="button" class="player-btn" title={i18n.t("music_assistant.expand").to_string()} onclick={toggle_expanded}>
                        <i class="ph ph-arrows-out"></i>
                    </button>
                </div>
            </div>
        </div>
        // After the bar, as on the local player: both sit at z-index 50.
        { for notes_modal }
        </>
    }
}

#[function_component(MusicAssistantHost)]
pub fn music_assistant_host() -> Html {
    let server =
        use_selector(|s: &AppState| s.auth_details.as_ref().map(|a| a.server_name.clone()));
    let api_key =
        use_selector(|s: &AppState| s.auth_details.as_ref().and_then(|a| a.api_key.clone()));
    let user_id = use_selector(|s: &AppState| s.user_details.as_ref().map(|u| u.UserID));
    {
        let deps = ((*server).clone(), (*api_key).clone(), *user_id);
        use_effect_with(deps, move |_| {
            refresh_music_assistant();
            let iv = Interval::new(12_000, refresh_music_assistant);
            move || drop(iv)
        });
    }
    let relay_active = use_selector(|s: &NowPlayingState| {
        s.ma_devices
            .iter()
            .filter(|d| d.episode_id != 0 || d.playing)
            .filter_map(|d| {
                d.ma_player_id
                    .clone()
                    .filter(|id| !id.is_empty())
                    .or_else(|| player_id_from_device(&d.device_id))
            })
            .collect::<Vec<_>>()
    });
    {
        let ids = (*relay_active).clone();
        use_effect_with(ids, move |ids| {
            note_relay_transport(ids);
            || ()
        });
    }
    let (ma, _) = use_store::<MusicAssistantState>();
    html! {
        if ma.enabled {
            <SpeakerPicker />
        }
    }
}

/// Keeps `MusicAssistantState::focus` on the player that started last.
/// Mounted once at the app root: `MusicAssistantHost` remounts with every
/// page, which would re-report speakers that were already playing as new
/// starts.
#[function_component(PlaybackFocusTracker)]
pub fn playback_focus_tracker() -> Html {
    let local_playing = use_selector(|s: &UIState| s.audio_playing == Some(true));
    use_effect_with(*local_playing, |playing| {
        if *playing {
            Dispatch::<MusicAssistantState>::global()
                .reduce_mut(|s| s.focus = PlaybackFocus::Local);
        }
        || ()
    });
    let speakers_playing = use_selector(|s: &NowPlayingState| playing_speakers(&s.ma_devices));
    let seen = use_mut_ref(Vec::<String>::new);
    use_effect_with((*speakers_playing).clone(), move |now| {
        let started = started_speaker(&seen.borrow(), now).map(str::to_string);
        if let Some(id) = started {
            Dispatch::<MusicAssistantState>::global()
                .reduce_mut(|s| s.focus = PlaybackFocus::Speaker(id));
        }
        *seen.borrow_mut() = now.clone();
        || ()
    });
    html! {}
}

#[derive(Clone, PartialEq)]
struct ResumePrompt {
    player_id: String,
    device_id: String,
    episode_id: i32,
    is_youtube: bool,
    resume_sec: f64,
    start_over_sec: f64,
}

enum SpeakerResolve {
    Offer(f64, f64),
    Send(f64),
    Failed(anyhow::Error),
}

fn send_episode(
    player_id: String,
    device_id: String,
    episode_id: i32,
    is_youtube: bool,
    position_sec: Option<f64>,
    failure: String,
) {
    let mut args = play_episode_args(episode_id, is_youtube, position_sec);
    let playlist_id = Dispatch::<UIState>::global().get().current_playlist_id;
    if let Some(pid) = speaker_playlist(current_playing_anchor(), (episode_id, is_youtube), playlist_id) {
        args["playlist_id"] = json!(pid);
    }
    Dispatch::<MusicAssistantState>::global().reduce_mut(|s| {
        s.focus = PlaybackFocus::Speaker(device_id);
        s.picker_open = false;
        s.pending_episode = None;
    });
    run_http(player_id, "play_episode", args, failure, true);
}

async fn resolve_speaker_start(episode_id: i32, is_youtube: bool) -> SpeakerResolve {
    let Some((server, api_key, user_id)) = session() else {
        return SpeakerResolve::Failed(anyhow::Error::msg("not signed in"));
    };
    let req = EpisodeRequest {
        episode_id,
        user_id,
        person_episode: false,
        is_youtube,
    };
    let episode = match call_get_episode_metadata(&server, Some(api_key.clone()), &req).await {
        Ok(episode) => episode,
        Err(err) => return SpeakerResolve::Failed(err),
    };
    let start_skip = if episode.podcastid > 0 {
        call_get_play_episode_details(
            &server,
            &Some(api_key),
            user_id,
            episode.podcastid,
            is_youtube,
        )
        .await
        .map(|(_, start, _, _)| start)
        .unwrap_or(0)
    } else {
        0
    };
    let start = speaker_start(
        episode.listenduration as f64,
        episode.completed,
        start_skip as i64,
    );
    match start.offer {
        Some(offer) => SpeakerResolve::Offer(offer.resume_sec, offer.start_over_sec),
        None => SpeakerResolve::Send(start.position_sec),
    }
}

#[function_component(SpeakerPicker)]
fn speaker_picker() -> Html {
    let (i18n, _) = use_translation();
    let (ma, ma_dispatch) = use_store::<MusicAssistantState>();
    let (ui, _) = use_store::<UIState>();
    let selected = use_state(HashSet::<String>::new);
    let prompt = use_state(|| None::<ResumePrompt>);
    let fetching = use_state(|| false);
    let open = ma.picker_open;

    let close = {
        let ma_dispatch = ma_dispatch.clone();
        let prompt = prompt.clone();
        Callback::from(move |_: MouseEvent| {
            prompt.set(None);
            ma_dispatch.reduce_mut(|s| {
                s.picker_open = false;
                s.pending_episode = None;
            });
        })
    };
    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    let failure = i18n.t("music_assistant.command_failed").to_string();
    let nothing = i18n.t("music_assistant.nothing_to_play").to_string();
    let offline_here = i18n.t("music_assistant.offline_here").to_string();
    let need_two = i18n.t("music_assistant.group_need_two").to_string();
    let multi = i18n.t("music_assistant.group_one_existing").to_string();
    let unchanged = i18n.t("music_assistant.group_unchanged").to_string();

    let play_target = |pending: &Option<crate::requests::music_assistant::PendingEpisode>,
                       ui: &UIState|
     -> Option<(i32, bool, Option<f64>)> {
        if let Some(pending) = pending {
            return Some((pending.episode_id, pending.is_youtube, None));
        }
        let (episode_id, is_youtube) = current_playing_anchor();
        let episode_id = episode_id?;
        let position = ui
            .media_element
            .as_ref()
            .map(|m| m.current_time())
            .unwrap_or(ui.current_time_seconds);
        Some((episode_id, is_youtube.unwrap_or(false), Some(position)))
    };

    let on_play = {
        let failure = failure.clone();
        let nothing = nothing.clone();
        let pending = ma.pending_episode.clone();
        let ui = ui.clone();
        let prompt = prompt.clone();
        let fetching = fetching.clone();
        let offline_here = offline_here.clone();
        Callback::from(move |player: MaPlayer| {
            if pending.is_none() && !send_to_speaker_allowed(true, ui.offline == Some(true)) {
                toast_info(&Dispatch::<NotificationState>::global(), offline_here.clone());
                return;
            }
            let Some((episode_id, is_youtube, position)) = play_target(&pending, &ui) else {
                toast_info(&Dispatch::<NotificationState>::global(), nothing.clone());
                return;
            };
            if let Some(position) = position {
                prompt.set(None);
                send_episode(
                    player.player_id,
                    player.device_id,
                    episode_id,
                    is_youtube,
                    Some(position.max(0.0)),
                    failure.clone(),
                );
                return;
            }
            if *fetching {
                return;
            }
            fetching.set(true);
            let failure = failure.clone();
            let prompt = prompt.clone();
            let fetching = fetching.clone();
            let player_id = player.player_id;
            let device_id = player.device_id;
            spawn_local(async move {
                let resolved = resolve_speaker_start(episode_id, is_youtube).await;
                fetching.set(false);
                match resolved {
                    SpeakerResolve::Offer(resume_sec, start_over_sec) => {
                        prompt.set(Some(ResumePrompt {
                            player_id,
                            device_id,
                            episode_id,
                            is_youtube,
                            resume_sec,
                            start_over_sec,
                        }));
                    }
                    SpeakerResolve::Send(position_sec) => {
                        prompt.set(None);
                        send_episode(
                            player_id,
                            device_id,
                            episode_id,
                            is_youtube,
                            Some(position_sec),
                            failure,
                        );
                    }
                    SpeakerResolve::Failed(err) => {
                        toast_failure(&failure, &err);
                        prompt.set(None);
                        send_episode(player_id, device_id, episode_id, is_youtube, None, failure);
                    }
                }
            });
        })
    };

    let on_group = {
        let selected = selected.clone();
        let players = ma.players.clone();
        let failure = failure.clone();
        let need_two = need_two.clone();
        let multi = multi.clone();
        let unchanged = unchanged.clone();
        Callback::from(move |_: MouseEvent| {
            let ids: Vec<String> = players
                .iter()
                .filter(|p| selected.contains(&p.player_id))
                .map(|p| p.player_id.clone())
                .collect();
            match plan_group(&players, &ids) {
                Ok(plan) => {
                    let player_id = plan.player_id().to_string();
                    let action = plan.action();
                    let args = plan.args();
                    selected.set(HashSet::new());
                    run_http(player_id, action, args, failure.clone(), false);
                }
                Err(GroupPlanError::NeedTwo) => {
                    toast_info(&Dispatch::<NotificationState>::global(), need_two.clone())
                }
                Err(GroupPlanError::MultipleGroups) => {
                    toast_info(&Dispatch::<NotificationState>::global(), multi.clone())
                }
                Err(GroupPlanError::Unchanged) => {
                    toast_info(&Dispatch::<NotificationState>::global(), unchanged.clone())
                }
            }
        })
    };

    // Episode id and playing flag only. Position ticks must not rebuild the picker.
    let relay = use_selector(|s: &NowPlayingState| {
        s.ma_devices
            .iter()
            .map(|d| (d.device_id.clone(), d.episode_id, d.playing))
            .collect::<Vec<_>>()
    });
    let checked_count = ma
        .players
        .iter()
        .filter(|p| selected.contains(&p.player_id))
        .count();
    let play_label = if ma.pending_episode.is_some() {
        i18n.t("music_assistant.play_episode").to_string()
    } else {
        i18n.t("music_assistant.play_current").to_string()
    };

    let resume_prompt = (*prompt).clone().map(|choice| {
        let resume_label = i18n
            .t("music_assistant.resume_at")
            .to_string()
            .replace("{time}", &format_clock(choice.resume_sec));
        let start_label = i18n.t("music_assistant.start_over").to_string();
        let on_resume = {
            let failure = failure.clone();
            let prompt = prompt.clone();
            let choice = choice.clone();
            Callback::from(move |_: MouseEvent| {
                prompt.set(None);
                send_episode(
                    choice.player_id.clone(),
                    choice.device_id.clone(),
                    choice.episode_id,
                    choice.is_youtube,
                    Some(choice.resume_sec),
                    failure.clone(),
                );
            })
        };
        let on_start = {
            let failure = failure.clone();
            let prompt = prompt.clone();
            Callback::from(move |_: MouseEvent| {
                prompt.set(None);
                send_episode(
                    choice.player_id.clone(),
                    choice.device_id.clone(),
                    choice.episode_id,
                    choice.is_youtube,
                    Some(choice.start_over_sec),
                    failure.clone(),
                );
            })
        };
        html! {
            <div class="ma-note">
                <button type="button" class="btn btn-primary" onclick={on_resume}>{ resume_label }</button>
                <button type="button" class="btn" onclick={on_start}>{ start_label }</button>
            </div>
        }
    });

    html! {
        <>
            <div class={classes!("nc-scrim", open.then_some("is-open"))} onclick={close.clone()}></div>
            <div class={classes!("nc-panel", "nc-form-drawer", "ma-picker", open.then_some("is-open"))}
                 data-density="compact" onclick={stop}>
                <div class="nc-header">
                    <h3 class="nc-h-title">{ i18n.t("music_assistant.speaker_picker") }</h3>
                    <div class="nc-h-spacer"></div>
                    <button type="button" class="nc-h-btn" title={i18n.t("music_assistant.close").to_string()} onclick={close}>
                        <i class="ph ph-arrow-line-right"></i>
                    </button>
                </div>
                <div class="nc-body">
                    { resume_prompt }
                    if !ma.connected {
                        <div class="ma-note">{ i18n.t("music_assistant.not_connected") }</div>
                    }
                    if ma.players.is_empty() {
                        <div class="nc-empty">
                            <span class="nc-empty-ico"><i class="ph ph-speaker-high"></i></span>
                            <div class="nc-empty-t">{ i18n.t("music_assistant.no_players") }</div>
                        </div>
                    }
                    <div class="ma-note">{ i18n.t("music_assistant.select_speakers") }</div>
                    { for ma.players.iter().map(|player| {
                        let id = player.player_id.clone();
                        let checked = selected.contains(&id);
                        let on_check = {
                            let selected = selected.clone();
                            let id = id.clone();
                            Callback::from(move |_: Event| {
                                let mut next = (*selected).clone();
                                if !next.insert(id.clone()) {
                                    next.remove(&id);
                                }
                                selected.set(next);
                            })
                        };
                        let play = {
                            let on_play = on_play.clone();
                            let player = player.clone();
                            Callback::from(move |_: MouseEvent| on_play.emit(player.clone()))
                        };
                        let kind = if player.kind.eq_ignore_ascii_case("group") || !player.group_members.is_empty() {
                            i18n.t("music_assistant.group_kind").to_string()
                        } else {
                            i18n.t("music_assistant.player").to_string()
                        };
                        let playback_state = playback_state_for(player, &ma.players);
                        // A sync member plays with its leader. Playing on it alone
                        // would split the group, so its row offers no Play.
                        let member_of_group = member_of(player, &ma.players).map(|leader| {
                            i18n.t("music_assistant.in_group").to_string().replace(
                                "{group}",
                                if leader.name.is_empty() { &leader.player_id } else { &leader.name },
                            )
                        });
                        let ungroup = {
                            let failure = failure.clone();
                            let id = id.clone();
                            Callback::from(move |_: MouseEvent| {
                                run_http(id.clone(), "ungroup", json!({}), failure.clone(), false);
                            })
                        };
                        let remove_member = {
                            let failure = failure.clone();
                            let group_id = id.clone();
                            Callback::from(move |member: String| {
                                run_http(
                                    group_id.clone(),
                                    "set_members",
                                    json!({ "player_ids_to_remove": [member] }),
                                    failure.clone(),
                                    false,
                                );
                            })
                        };
                        let add_member = {
                            let failure = failure.clone();
                            let group_id = id.clone();
                            Callback::from(move |e: Event| {
                                let Some(input) = e.target_dyn_into::<web_sys::HtmlSelectElement>() else {
                                    return;
                                };
                                let member = input.value();
                                if member.is_empty() {
                                    return;
                                }
                                run_http(
                                    group_id.clone(),
                                    "set_members",
                                    json!({ "player_ids_to_add": [member] }),
                                    failure.clone(),
                                    false,
                                );
                            })
                        };
                        let show_group_tools = player.is_group();
                        let transport_id = transport_player_id(player);
                        let episode_of = |device_id: &str| {
                            relay.iter().find(|(id, _, _)| id == device_id).map(|(_, episode, playing)| {
                                (*episode, *playing)
                            })
                        };
                        let leader_device = ma
                            .players
                            .iter()
                            .find(|p| p.player_id == transport_id)
                            .map(|p| p.device_id.as_str());
                        let loaded_here = episode_of(&player.device_id);
                        let loaded_leader = leader_device.and_then(episode_of);
                        let episode_loaded = loaded_here.map(|(episode, _)| episode != 0).unwrap_or(false)
                            || loaded_leader.map(|(episode, _)| episode != 0).unwrap_or(false);
                        let relay_playing = loaded_here
                            .map(|(episode, playing)| episode != 0 && playing)
                            .unwrap_or(false)
                            || loaded_leader
                                .map(|(episode, playing)| episode != 0 && playing)
                                .unwrap_or(false);
                        // A 12s refresh that reports idle, some other state, or
                        // unavailable keeps play/pause/stop while a queue is
                        // loaded or this speaker was playing recently.
                        let now = unix_now();
                        let recent = transport_is_recent(
                            ma.recent_transport.get(&player.player_id).copied(),
                            now,
                        ) || transport_is_recent(ma.recent_transport.get(&transport_id).copied(), now);
                        let playing_now =
                            transport_shown(playback_state, episode_loaded, relay_playing, recent);
                        let state = if !player.available && playing_now.is_some() {
                            i18n.t("music_assistant.reconnecting").to_string()
                        } else if state_i18n_key(playback_state).is_none() && playing_now.is_some()
                        {
                            i18n.t("music_assistant.state_unknown").to_string()
                        } else {
                            match state_i18n_key(playback_state) {
                                Some(key) => i18n.t(key).to_string(),
                                None => playback_state.to_string(),
                            }
                        };
                        let unavailable = if player.available || playing_now.is_some() {
                            String::new()
                        } else {
                            i18n.t("music_assistant.unavailable").to_string()
                        };
                        let on_toggle = {
                            let failure = failure.clone();
                            let transport_id = transport_id.clone();
                            let playing_now = playing_now.unwrap_or(false);
                            Callback::from(move |_: MouseEvent| {
                                run_http(
                                    transport_id.clone(),
                                    if playing_now { "pause" } else { "play" },
                                    json!({}),
                                    failure.clone(),
                                    false,
                                );
                            })
                        };
                        let on_stop = {
                            let failure = failure.clone();
                            let transport_id = transport_id.clone();
                            Callback::from(move |_: MouseEvent| {
                                run_http(transport_id.clone(), "stop", json!({}), failure.clone(), false);
                            })
                        };
                        html! {
                            <div class="rd-row" key={player.player_id.clone()}>
                                <div class="rd-row-top">
                                    <label class="ma-check ma-speaker-check">
                                        <input type="checkbox" checked={checked} onchange={on_check} />
                                        <span class="ma-speaker-text">
                                            <span class="rd-name">
                                                <i class={ if show_group_tools { "ph ph-speakers" } else { "ph ph-speaker-high" } }></i>
                                                { player.name.clone() }
                                            </span>
                                            <span class={classes!("rd-state", (playback_state.eq_ignore_ascii_case("playing") || playback_state.eq_ignore_ascii_case("buffering")).then_some("is-playing"))}>
                                                { kind }{ " · " }{ state }
                                                if !unavailable.is_empty() {
                                                    { format!(" · {unavailable}") }
                                                }
                                            </span>
                                        </span>
                                    </label>
                                </div>
                                if let Some(label) = member_of_group.clone() {
                                    <span class="rd-track">{ label }</span>
                                }
                                if show_group_tools {
                                    <div class="ma-member-list">
                                        { for player.group_members.iter().map(|member| {
                                            let label = player_name(&ma.players, member).to_string();
                                            let remove = {
                                                let remove_member = remove_member.clone();
                                                let member = member.clone();
                                                Callback::from(move |_: MouseEvent| remove_member.emit(member.clone()))
                                            };
                                            html! {
                                                <span class="ma-chip" key={member.clone()}>
                                                    { label }
                                                    <button type="button" class="iconbtn" title={i18n.t("music_assistant.remove_member").to_string()} onclick={remove}>
                                                        <i class="ph ph-x"></i>
                                                    </button>
                                                </span>
                                            }
                                        }) }
                                    </div>
                                    if !player.can_group_with.is_empty() {
                                        <select class="select ma-add-member" onchange={add_member}>
                                            <option value="">{ i18n.t("music_assistant.add_member") }</option>
                                            { for player.can_group_with.iter().filter(|id| !player.group_members.iter().any(|m| m == *id)).map(|id| {
                                                let label = player_name(&ma.players, id).to_string();
                                                html! { <option value={id.clone()}>{ label }</option> }
                                            }) }
                                        </select>
                                    }
                                }
                                if player.available {
                                    if let Some(level) = player.volume_level {
                                        if show_group_tools {
                                            <VolumeControl
                                                device_id={player.device_id.clone()}
                                                level={level}
                                                muted={player.volume_muted}
                                                group={true}
                                                label={i18n.t("music_assistant.group_volume").to_string()}
                                            />
                                        } else {
                                            <VolumeControl
                                                device_id={player.device_id.clone()}
                                                level={level}
                                                muted={player.volume_muted}
                                                group={false}
                                                label={i18n.t("music_assistant.volume").to_string()}
                                            />
                                        }
                                    }
                                }
                                <div class="rd-controls">
                                    if let Some(is_playing) = playing_now {
                                        <button type="button" class="iconbtn" title={ if is_playing { i18n.t("music_assistant.pause").to_string() } else { i18n.t("music_assistant.play").to_string() } } onclick={on_toggle}>
                                            <i class={ if is_playing { "ph ph-pause" } else { "ph ph-play" } }></i>
                                        </button>
                                        <button type="button" class="iconbtn" title={i18n.t("music_assistant.stop").to_string()} onclick={on_stop}>
                                            <i class="ph ph-stop"></i>
                                        </button>
                                    }
                                    if member_of_group.is_none() {
                                        <button type="button" class="rd-play-here" disabled={!player.available} onclick={play}>
                                            <i class="ph ph-play mr-1"></i>{ play_label.clone() }
                                        </button>
                                    }
                                    if show_group_tools {
                                        <button type="button" class="rd-play-here" onclick={ungroup}>
                                            { i18n.t("music_assistant.ungroup") }
                                        </button>
                                    }
                                </div>
                            </div>
                        }
                    }) }
                    <div class="ma-group-bar">
                        <button type="button" class="btn btn-primary" onclick={on_group} disabled={checked_count < 2}>
                            { i18n.t("music_assistant.group") }
                        </button>
                    </div>
                </div>
            </div>
        </>
    }
}

/// True when a relay device is a Music Assistant player. Used by the remote panel.
pub fn device_is_ma(device_type: &str) -> bool {
    is_music_assistant(device_type)
}
