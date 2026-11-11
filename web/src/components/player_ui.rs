//! Pointer-driven player widgets: the seek bar, a general slider, the speed and
//! volume controls, and the drag gesture that expands and dismisses the player
//! sheet. Mouse, touch and pen all go through pointer events with pointer
//! capture, and anything that moves every frame is written straight to the DOM
//! so the big player component never re-renders mid-gesture.

use crate::components::context::UIState;
use gloo::events::{EventListener, EventListenerOptions, EventListenerPhase};
use i18nrs::yew::use_translation;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, HtmlMediaElement, KeyboardEvent, PointerEvent};
use yew::prelude::*;
use yewdux::prelude::*;

// ---------------------------------------------------------------------------
// Media helpers
// ---------------------------------------------------------------------------

fn with_media<R>(f: impl FnOnce(&HtmlMediaElement) -> R) -> Option<R> {
    let state = Dispatch::<UIState>::global().get();
    if let Some(m) = state.media_element.as_ref() {
        Some(f(m.as_media_element()))
    } else {
        state.audio_element.as_ref().map(|a| f(a))
    }
}

/// Live (position, duration, end of the buffered range around the playhead).
fn playhead() -> Option<(f64, f64, f64)> {
    with_media(|m| {
        let pos = m.current_time();
        let buffered = m.buffered();
        let mut buf_end = pos;
        for i in 0..buffered.length() {
            if let (Ok(s), Ok(e)) = (buffered.start(i), buffered.end(i)) {
                if s <= pos + 0.5 && e >= pos {
                    buf_end = e;
                }
            }
        }
        (pos, m.duration(), buf_end)
    })
}

/// Seek whatever is playing and keep the shared state in step.
pub fn seek_to(seconds: f64) {
    let seconds = seconds.max(0.0);
    if with_media(|m| m.set_current_time(seconds)).is_some() {
        Dispatch::<UIState>::global().reduce_mut(|s| s.update_current_time(seconds));
    }
}

pub fn seek_by(delta: f64) {
    if let Some((pos, dur, _)) = playhead() {
        let target = pos + delta;
        seek_to(if dur.is_finite() && dur > 0.0 { target.min(dur - 0.25) } else { target });
    }
}

pub fn toggle_play() {
    let state = Dispatch::<UIState>::global().get();
    let playing = state.audio_playing.unwrap_or(false);
    if let Some(m) = state.media_element.as_ref() {
        if playing {
            let _ = m.pause();
        } else {
            let _ = m.play();
        }
    } else if let Some(a) = state.audio_element.as_ref() {
        if playing {
            let _ = a.pause();
        } else {
            let _ = a.play();
        }
    }
}

/// `M:SS`, or `H:MM:SS` once the episode runs past an hour.
pub fn fmt_time(seconds: f64, long: bool) -> String {
    let s = if seconds.is_finite() { seconds.max(0.0) as i64 } else { 0 };
    let (h, m, sec) = (s / 3600, (s % 3600) / 60, s % 60);
    if long || h > 0 {
        format!("{}:{:02}:{:02}", h, m, sec)
    } else {
        format!("{}:{:02}", m, sec)
    }
}

fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

fn set_var(el: &HtmlElement, name: &str, value: f64) {
    let _ = el.style().set_property(name, &format!("{:.5}", value));
}

fn frac_at(track: &NodeRef, client_x: f64) -> f64 {
    track
        .cast::<Element>()
        .map(|el| {
            let r = el.get_bounding_client_rect();
            if r.width() > 0.0 {
                ((client_x - r.left()) / r.width()).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .unwrap_or(0.0)
}

/// Captures on the element under the pointer, so later events still bubble
/// through our handlers. (Yew listens at the root, so `current_target` is the
/// root, and capturing there would starve the component of the rest.)
fn capture(e: &PointerEvent) {
    if let Some(el) = e.target().and_then(|t| t.dyn_into::<Element>().ok()) {
        let _ = el.set_pointer_capture(e.pointer_id());
    }
}

// ---------------------------------------------------------------------------
// Frame loop
// ---------------------------------------------------------------------------

/// Calls `tick` once per animation frame until dropped.
pub struct FrameLoop {
    id: Rc<Cell<i32>>,
    cb: Rc<RefCell<Option<Closure<dyn FnMut()>>>>,
}

fn raf(cb: &Closure<dyn FnMut()>) -> i32 {
    web_sys::window()
        .and_then(|w| w.request_animation_frame(cb.as_ref().unchecked_ref()).ok())
        .unwrap_or(0)
}

impl FrameLoop {
    pub fn start(mut tick: impl FnMut() + 'static) -> Self {
        let cb: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
        let id = Rc::new(Cell::new(0));
        let (next_cb, next_id) = (cb.clone(), id.clone());
        *cb.borrow_mut() = Some(Closure::new(move || {
            tick();
            if let Some(c) = next_cb.borrow().as_ref() {
                next_id.set(raf(c));
            }
        }));
        if let Some(c) = cb.borrow().as_ref() {
            id.set(raf(c));
        }
        Self { id, cb }
    }
}

impl Drop for FrameLoop {
    fn drop(&mut self) {
        if let Some(w) = web_sys::window() {
            let _ = w.cancel_animation_frame(self.id.get());
        }
        // Breaks the closure's reference to itself.
        self.cb.borrow_mut().take();
    }
}

// ---------------------------------------------------------------------------
// Seek bar
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq, Clone)]
pub struct SeekBarProps {
    /// Duration from the episode record; the media's own length wins once known.
    pub duration: f64,
    /// Chapter starts (seconds) and titles, drawn as notches on the track.
    #[prop_or_default]
    pub chapters: Rc<Vec<(f64, String)>>,
    /// Times under the track instead of either side of it.
    #[prop_or_default]
    pub stacked: bool,
    #[prop_or_default]
    pub class: Classes,
}

/// Follows the playhead every frame. Drag (or click) anywhere on it to scrub:
/// the thumb follows the pointer with a time bubble, and the seek happens once
/// on release so streams aren't asked for a new range on every pixel.
#[function_component(SeekBar)]
pub fn seek_bar(props: &SeekBarProps) -> Html {
    let root = use_node_ref();
    let track = use_node_ref();
    let scrub = use_state(|| None::<f64>);
    let hover = use_state(|| None::<f64>);
    let shown_sec = use_state(|| 0i64);
    let media_dur = use_state(|| 0.0f64);
    let remaining = use_state(|| false);
    let dragging = use_mut_ref(|| false);

    let duration = if *media_dur > 0.0 { *media_dur } else { props.duration.max(0.0) };
    let long = duration >= 3600.0;

    {
        let root = root.clone();
        let dragging = dragging.clone();
        let shown_sec = shown_sec.clone();
        let media_dur = media_dur.clone();
        let fallback = use_mut_ref(|| 0.0f64);
        *fallback.borrow_mut() = props.duration;
        use_effect_with((), move |_| {
            let last_sec = Cell::new(-1i64);
            let last_dur = Cell::new(-1.0f64);
            let last = Cell::new((-1.0f64, -1.0f64));
            let frames = FrameLoop::start(move || {
                let Some((pos, mdur, buf)) = playhead() else { return };
                let real = mdur.is_finite() && mdur > 0.0;
                if real && (mdur - last_dur.get()).abs() > 0.5 {
                    last_dur.set(mdur);
                    media_dur.set(mdur);
                } else if !real && last_dur.get() > 0.0 {
                    // A new episode is loading: fall back to its recorded length.
                    last_dur.set(-1.0);
                    media_dur.set(0.0);
                }
                let dur = if real { mdur } else { *fallback.borrow() };
                let sec = pos.floor() as i64;
                if sec != last_sec.get() {
                    last_sec.set(sec);
                    shown_sec.set(sec);
                }
                if *dragging.borrow() || dur <= 0.0 {
                    return;
                }
                let p = (pos / dur).clamp(0.0, 1.0);
                let b = (buf / dur).clamp(0.0, 1.0);
                let (lp, lb) = last.get();
                if (p - lp).abs() > 0.00005 || (b - lb).abs() > 0.001 {
                    last.set((p, b));
                    if let Some(el) = root.cast::<HtmlElement>() {
                        set_var(&el, "--p", p);
                        set_var(&el, "--b", b);
                    }
                }
            });
            move || drop(frames)
        });
    }

    let begin = {
        let (root, track, scrub, dragging) = (root.clone(), track.clone(), scrub.clone(), dragging.clone());
        Callback::from(move |e: PointerEvent| {
            if e.button() != 0 || duration <= 0.0 {
                return;
            }
            e.prevent_default();
            e.stop_propagation();
            capture(&e);
            if let Some(el) = track.cast::<HtmlElement>() {
                let _ = el.focus();
            }
            let f = frac_at(&track, e.client_x());
            *dragging.borrow_mut() = true;
            if let Some(el) = root.cast::<HtmlElement>() {
                set_var(&el, "--p", f);
            }
            scrub.set(Some(f * duration));
        })
    };
    let move_ = {
        let (root, track, scrub, hover, dragging) =
            (root.clone(), track.clone(), scrub.clone(), hover.clone(), dragging.clone());
        Callback::from(move |e: PointerEvent| {
            let f = frac_at(&track, e.client_x());
            if *dragging.borrow() {
                e.prevent_default();
                if let Some(el) = root.cast::<HtmlElement>() {
                    set_var(&el, "--p", f);
                }
                scrub.set(Some(f * duration));
            } else if e.pointer_type() == "mouse" {
                hover.set(Some(f));
            }
        })
    };
    let end = {
        let (track, scrub, hover, dragging) = (track.clone(), scrub.clone(), hover.clone(), dragging.clone());
        Callback::from(move |e: PointerEvent| {
            if *dragging.borrow() {
                *dragging.borrow_mut() = false;
                seek_to(frac_at(&track, e.client_x()) * duration);
                scrub.set(None);
                if e.pointer_type() != "mouse" {
                    hover.set(None);
                }
            }
        })
    };
    let cancel = {
        let (scrub, dragging) = (scrub.clone(), dragging.clone());
        Callback::from(move |_: PointerEvent| {
            *dragging.borrow_mut() = false;
            scrub.set(None);
        })
    };
    let leave = {
        let hover = hover.clone();
        Callback::from(move |_: PointerEvent| hover.set(None))
    };
    let onkeydown = Callback::from(move |e: KeyboardEvent| {
        let step = match e.key().as_str() {
            "ArrowLeft" | "ArrowDown" => Some(-5.0),
            "ArrowRight" | "ArrowUp" => Some(5.0),
            "PageDown" => Some(-30.0),
            "PageUp" => Some(30.0),
            _ => None,
        };
        match (step, e.key().as_str()) {
            (Some(d), _) => seek_by(d),
            (None, "Home") => seek_to(0.0),
            (None, "End") if duration > 0.0 => seek_to(duration - 1.0),
            _ => return,
        }
        e.prevent_default();
        e.stop_propagation();
    });
    let toggle_remaining = {
        let remaining = remaining.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            remaining.set(!*remaining);
        })
    };

    let current = scrub.unwrap_or(*shown_sec as f64);
    let cur_label = fmt_time(current, long);
    let end_label = if *remaining {
        format!("-{}", fmt_time((duration - current).max(0.0), long))
    } else {
        fmt_time(duration, long)
    };
    let chapter_at = |t: f64| -> Option<&str> {
        props
            .chapters
            .iter()
            .rev()
            .find(|(start, _)| *start <= t)
            .map(|(_, title)| title.as_str())
            .filter(|t| !t.is_empty())
    };
    let bubble = scrub
        .map(|t| if duration > 0.0 { t / duration } else { 0.0 })
        .or(*hover);
    let bubble_html = bubble.map(|f| {
        let t = f * duration;
        html! {
            <div class="pp-seek-bubble" style={format!("left: clamp(28px, {:.3}%, calc(100% - 28px))", f * 100.0)}>
                <span class="pp-seek-bubble-time">{ fmt_time(t, long) }</span>
                { chapter_at(t).map(|c| html! { <span class="pp-seek-bubble-chapter">{ c.to_string() }</span> }).unwrap_or_default() }
            </div>
        }
    });
    let ticks = if duration > 0.0 {
        props
            .chapters
            .iter()
            .filter(|(s, _)| *s > 0.5 && *s < duration)
            .map(|(s, _)| html! { <span class="pp-seek-tick" style={format!("left: {:.3}%", s / duration * 100.0)} /> })
            .collect::<Html>()
    } else {
        html! {}
    };

    let end_button = html! {
        <button type="button" class="pp-seek-time pp-seek-time--end" onclick={toggle_remaining}
            title="Show remaining time">{ end_label }</button>
    };

    html! {
        <div ref={root} class={classes!(
            "pp-seek",
            props.stacked.then_some("pp-seek--stacked"),
            scrub.is_some().then_some("is-scrubbing"),
            props.class.clone(),
        )}>
            if !props.stacked { <span class="pp-seek-time">{ cur_label.clone() }</span> }
            <div ref={track} class="pp-seek-hit"
                role="slider" tabindex="0" aria-label="Seek"
                aria-valuemin="0"
                aria-valuemax={format!("{:.0}", duration)}
                aria-valuenow={format!("{:.0}", current)}
                aria-valuetext={cur_label.clone()}
                onpointerdown={begin}
                onpointermove={move_}
                onpointerup={end}
                onpointercancel={cancel}
                onpointerleave={leave}
                onkeydown={onkeydown}
            >
                <div class="pp-seek-track">
                    <div class="pp-seek-buffer" />
                    if let (Some(f), None) = (*hover, *scrub) {
                        <div class="pp-seek-ghost" style={format!("transform: scaleX({:.4})", f)} />
                    }
                    <div class="pp-seek-fill" />
                    { ticks }
                </div>
                <div class="pp-seek-thumb-rail"><div class="pp-seek-thumb" /></div>
                { bubble_html.unwrap_or_default() }
            </div>
            if props.stacked {
                <div class="pp-seek-times">
                    <span class="pp-seek-time">{ cur_label }</span>
                    { end_button }
                </div>
            } else {
                { end_button }
            }
        </div>
    }
}

/// A thin progress line for the collapsed phone bar, smooth like the seek bar.
#[function_component(ProgressLine)]
pub fn progress_line() -> Html {
    let el = use_node_ref();
    {
        let el = el.clone();
        use_effect_with((), move |_| {
            let last = Cell::new(-1.0f64);
            let frames = FrameLoop::start(move || {
                let Some((pos, dur, _)) = playhead() else { return };
                if !(dur.is_finite() && dur > 0.0) {
                    return;
                }
                let p = (pos / dur).clamp(0.0, 1.0);
                if (p - last.get()).abs() > 0.0001 {
                    last.set(p);
                    if let Some(el) = el.cast::<HtmlElement>() {
                        set_var(&el, "--p", p);
                    }
                }
            });
            move || drop(frames)
        });
    }
    html! { <div ref={el} class="pp-line" aria-hidden="true"><div class="pp-line-fill" /></div> }
}

// ---------------------------------------------------------------------------
// Slider
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq, Clone)]
pub struct SliderProps {
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
    pub on_input: Callback<f64>,
    pub label: AttrValue,
    #[prop_or_default]
    pub value_text: Option<AttrValue>,
    #[prop_or_default]
    pub class: Classes,
}

/// A horizontal slider that drags with mouse, finger or pen and thickens while held.
#[function_component(Slider)]
pub fn slider(props: &SliderProps) -> Html {
    let track = use_node_ref();
    let held = use_state(|| false);
    let (min, max, step) = (props.min, props.max, props.step);
    let snap = move |raw: f64| -> f64 {
        let v = min + ((raw - min) / step).round() * step;
        (v.clamp(min, max) * 1000.0).round() / 1000.0
    };
    let emit_at = {
        let (track, on_input) = (track.clone(), props.on_input.clone());
        move |x: f64| on_input.emit(snap(min + frac_at(&track, x) * (max - min)))
    };

    let down = {
        let (held, emit_at) = (held.clone(), emit_at.clone());
        Callback::from(move |e: PointerEvent| {
            if e.button() != 0 {
                return;
            }
            e.prevent_default();
            e.stop_propagation();
            capture(&e);
            held.set(true);
            emit_at(e.client_x());
        })
    };
    let move_ = {
        let (held, emit_at) = (held.clone(), emit_at.clone());
        Callback::from(move |e: PointerEvent| {
            if *held {
                e.prevent_default();
                emit_at(e.client_x());
            }
        })
    };
    let up = {
        let held = held.clone();
        Callback::from(move |_: PointerEvent| held.set(false))
    };
    let onkeydown = {
        let (value, on_input) = (props.value, props.on_input.clone());
        Callback::from(move |e: KeyboardEvent| {
            let next = match e.key().as_str() {
                "ArrowLeft" | "ArrowDown" => value - step,
                "ArrowRight" | "ArrowUp" => value + step,
                "Home" => min,
                "End" => max,
                _ => return,
            };
            e.prevent_default();
            e.stop_propagation();
            on_input.emit(snap(next));
        })
    };

    let p = if max > min { ((props.value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 };
    html! {
        <div ref={track}
            class={classes!("pp-slider", held.then_some("is-held"), props.class.clone())}
            style={format!("--p: {:.4}", p)}
            role="slider" tabindex="0"
            aria-label={props.label.clone()}
            aria-valuemin={min.to_string()}
            aria-valuemax={max.to_string()}
            aria-valuenow={props.value.to_string()}
            aria-valuetext={props.value_text.clone()}
            onpointerdown={down}
            onpointermove={move_}
            onpointerup={up.clone()}
            onpointercancel={up}
            onkeydown={onkeydown}
        >
            <div class="pp-slider-track"><div class="pp-slider-fill" /></div>
            <div class="pp-slider-thumb-rail"><div class="pp-slider-thumb" /></div>
        </div>
    }
}

// ---------------------------------------------------------------------------
// Volume and speed
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
pub struct VolumeProps {
    /// 0–100.
    pub volume: f64,
    pub on_change: Callback<f64>,
    #[prop_or_default]
    pub class: Classes,
}

/// Mute toggle plus an inline slider. Muting remembers the level to come back to.
#[function_component(VolumeControl)]
pub fn volume_control(props: &VolumeProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_mute = i18n.t("audio.mute").to_string();
    let i18n_unmute = i18n.t("audio.unmute").to_string();
    let i18n_volume = i18n.t("audio.volume").to_string();
    let restore = use_mut_ref(|| 100.0f64);
    if props.volume > 0.0 {
        *restore.borrow_mut() = props.volume;
    }
    let icon = match props.volume {
        v if v <= 0.0 => "ph-speaker-x",
        v if v < 34.0 => "ph-speaker-none",
        v if v < 67.0 => "ph-speaker-low",
        _ => "ph-speaker-high",
    };
    let toggle = {
        let (volume, on_change, restore) = (props.volume, props.on_change.clone(), restore.clone());
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            on_change.emit(if volume > 0.0 { 0.0 } else { (*restore.borrow()).max(10.0) });
        })
    };
    html! {
        <div class={classes!("pp-volume", props.class.clone())}>
            <button type="button" class="player-btn" onclick={toggle}
                title={if props.volume > 0.0 { i18n_mute } else { i18n_unmute }}>
                <i class={classes!("ph", icon)}></i>
            </button>
            <Slider value={props.volume} min={0.0} max={100.0} step={1.0}
                on_input={props.on_change.clone()} label={i18n_volume}
                value_text={Some(AttrValue::from(format!("{}%", props.volume as i32)))}
                class={classes!("pp-volume-slider")} />
        </div>
    }
}

pub const SPEED_PRESETS: [f64; 7] = [0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5];

pub fn fmt_speed(speed: f64) -> String {
    let s = format!("{:.2}", speed);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    let s = if s.contains('.') { s.to_string() } else { format!("{}.0", s) };
    format!("{}×", s)
}

#[derive(Properties, PartialEq)]
pub struct SpeedProps {
    pub speed: f64,
    pub on_change: Callback<f64>,
}

/// A speed pill that opens presets and a fine slider above it.
#[function_component(SpeedControl)]
pub fn speed_control(props: &SpeedProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_speed = i18n.t("audio.speed").to_string();
    let i18n_playback_speed = i18n.t("audio.playback_speed").to_string();
    let open = use_state(|| false);
    let toggle = {
        let open = open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            open.set(!*open);
        })
    };
    let close = {
        let open = open.clone();
        Callback::from(move |_: MouseEvent| open.set(false))
    };
    let speed = props.speed;
    let presets = SPEED_PRESETS
        .iter()
        .map(|&p| {
            let on_change = props.on_change.clone();
            let active = (p - speed).abs() < 0.001;
            html! {
                <button type="button" class={classes!("pp-chip", active.then_some("is-active"))}
                    onclick={Callback::from(move |_: MouseEvent| on_change.emit(p))}>
                    { fmt_speed(p) }
                </button>
            }
        })
        .collect::<Html>();
    html! {
        <div class="pp-speed">
            <button type="button" class={classes!("pp-pill", (*open).then_some("is-open"))}
                onclick={toggle} title={i18n_playback_speed.clone()} aria-expanded={open.to_string()}>
                <i class="ph ph-gauge"></i>
                <span>{ fmt_speed(speed) }</span>
            </button>
            if *open {
                <div class="pp-popover-scrim" onclick={close} />
                <div class="pp-popover" role="dialog" aria-label={i18n_playback_speed.clone()}>
                    <div class="pp-popover-head">
                        <span>{ i18n_speed }</span>
                        <span class="pp-popover-value">{ fmt_speed(speed) }</span>
                    </div>
                    <div class="pp-chips">{ presets }</div>
                    <Slider value={speed} min={0.5} max={3.0} step={0.05}
                        on_input={props.on_change.clone()} label={i18n_playback_speed}
                        value_text={Some(AttrValue::from(fmt_speed(speed)))} />
                </div>
            }
        </div>
    }
}

// ---------------------------------------------------------------------------
// Sheet drag
// ---------------------------------------------------------------------------

const BAR_HEIGHT: f64 = 72.0;
const DRAG_SLOP: f64 = 8.0;

#[derive(Default)]
struct Drag {
    pointer: Option<i32>,
    start: (f64, f64),
    active: bool,
    expanded: bool,
    vh: f64,
    offset: f64,
    samples: Vec<(f64, f64)>,
    suppress_until: f64,
}

#[derive(Clone)]
pub struct SheetDrag {
    pub onpointerdown: Callback<PointerEvent>,
    pub onpointermove: Callback<PointerEvent>,
    pub onpointerup: Callback<PointerEvent>,
    pub onpointercancel: Callback<PointerEvent>,
}

fn clear_drag_styles(sheet: &HtmlElement) {
    let style = sheet.style();
    let _ = style.remove_property("height");
    let _ = style.remove_property("transform");
    let _ = style.remove_property("--sheet-p");
    let _ = sheet.class_list().remove_1("is-dragging");
}

/// Drag the collapsed bar up to open the player, or the open player down to
/// close it. The sheet follows the pointer; release past a distance or with
/// enough speed and it finishes the move, otherwise it springs back. A drag
/// never turns into a click on whatever it started on.
#[hook]
pub fn use_sheet_drag(sheet: NodeRef, on_expanded: Callback<bool>) -> SheetDrag {
    let drag = use_mut_ref(Drag::default);

    // Clicks that end a drag are swallowed before Yew (which listens at the
    // root in the capture phase) sees them.
    {
        let drag = drag.clone();
        use_effect_with((), move |_| {
            let listener = web_sys::window().map(|w| {
                EventListener::new_with_options(
                    &w,
                    "click",
                    EventListenerOptions { phase: EventListenerPhase::Capture, passive: false },
                    move |e| {
                        if now_ms() < drag.borrow().suppress_until {
                            e.stop_immediate_propagation();
                            e.prevent_default();
                        }
                    },
                )
            });
            move || drop(listener)
        });
    }

    let onpointerdown = {
        let drag = drag.clone();
        Callback::from(move |e: PointerEvent| {
            if e.button() != 0 || !e.is_primary() {
                return;
            }
            let mut d = drag.borrow_mut();
            d.pointer = Some(e.pointer_id());
            d.start = (e.client_x() as f64, e.client_y() as f64);
            d.active = false;
            d.offset = 0.0;
            d.expanded = Dispatch::<UIState>::global().get().is_expanded;
            d.samples.clear();
            // Capture right away: a quick flick leaves the 72px bar before the
            // first move. Captured on the element pressed, so a plain tap still
            // clicks it.
            capture(&e);
        })
    };

    let onpointermove = {
        let (drag, sheet) = (drag.clone(), sheet.clone());
        Callback::from(move |e: PointerEvent| {
            let mut d = drag.borrow_mut();
            if d.pointer != Some(e.pointer_id()) {
                return;
            }
            let (x, y) = (e.client_x() as f64, e.client_y() as f64);
            let (dx, dy) = (x - d.start.0, y - d.start.1);
            let Some(el) = sheet.cast::<HtmlElement>() else { return };
            if !d.active {
                // Only a drag in the direction the sheet can go counts.
                let toward = if d.expanded { dy } else { -dy };
                if toward > DRAG_SLOP && dy.abs() > dx.abs() {
                    d.active = true;
                    d.vh = web_sys::window()
                        .and_then(|w| w.inner_height().ok())
                        .and_then(|h| h.as_f64())
                        .unwrap_or(800.0);
                    let _ = el.class_list().add_1("is-dragging");
                } else if dx.abs() > DRAG_SLOP || dy.abs() > DRAG_SLOP {
                    d.pointer = None;
                    return;
                } else {
                    return;
                }
            }
            e.prevent_default();
            let t = now_ms();
            d.samples.push((t, y));
            d.samples.retain(|(st, _)| t - st < 120.0);
            let style = el.style();
            if d.expanded {
                let down = dy.max(0.0) + dy.min(0.0) * 0.15;
                d.offset = down;
                let _ = style.set_property("transform", &format!("translateY({:.1}px)", down.max(0.0)));
                set_var(&el, "--sheet-p", (1.0 - down.max(0.0) / d.vh).clamp(0.0, 1.0));
            } else {
                let up = (-dy).max(0.0);
                d.offset = up;
                let h = (BAR_HEIGHT + up).min(d.vh);
                let _ = style.set_property_with_priority("height", &format!("{:.1}px", h), "important");
                set_var(&el, "--sheet-p", (up / (d.vh - BAR_HEIGHT)).clamp(0.0, 1.0));
            }
        })
    };

    let finish = {
        let (drag, sheet, on_expanded) = (drag.clone(), sheet.clone(), on_expanded.clone());
        move |e: PointerEvent, cancelled: bool| {
            let mut d = drag.borrow_mut();
            if d.pointer != Some(e.pointer_id()) {
                return;
            }
            d.pointer = None;
            if !d.active {
                return;
            }
            d.active = false;
            d.suppress_until = now_ms() + 350.0;
            // px per ms, positive downward.
            let velocity = match (d.samples.first(), d.samples.last()) {
                (Some(a), Some(b)) if b.0 - a.0 > 8.0 => (b.1 - a.1) / (b.0 - a.0),
                _ => 0.0,
            };
            let flip = !cancelled
                && if d.expanded {
                    d.offset > 120.0 || velocity > 0.45
                } else {
                    d.offset > 90.0 || velocity < -0.45
                };
            if let Some(el) = sheet.cast::<HtmlElement>() {
                clear_drag_styles(&el);
                if flip {
                    // Set the class now so the transition starts from where the
                    // finger let go; Yew's render agrees with it a moment later.
                    let list = el.class_list();
                    let _ = if d.expanded { list.remove_1("expanded") } else { list.add_1("expanded") };
                }
            }
            if flip {
                on_expanded.emit(!d.expanded);
            }
        }
    };

    let onpointerup = {
        let finish = finish.clone();
        Callback::from(move |e: PointerEvent| finish(e, false))
    };
    let onpointercancel = Callback::from(move |e: PointerEvent| finish(e, true));

    SheetDrag { onpointerdown, onpointermove, onpointerup, onpointercancel }
}
