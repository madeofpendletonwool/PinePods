//! Rewind: a month's or a year's listening played as a story of full-screen slides.

use std::rc::Rc;

use crate::components::art_accent::{accent_style, use_art_accent};
use crate::components::context::AppState;
use crate::components::gen_components::FallbackImage;
use crate::components::i18n::use_translation;
use crate::pages::user_stats::{fmt_day, fmt_secs, full_month, group, hour_label, peak};
use crate::requests::stat_reqs::{self, ListeningReport, ReportEpisode, ReportPodcast, Rewind};
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yewdux::prelude::*;

/// How long each slide stays up, in seconds.
const SLIDE_SECONDS: u32 = 7;

/// `YYYY` or `YYYY-MM` as "2026" or "November 2026".
pub fn period_label(period: &str) -> String {
    match period.split_once('-') {
        Some((y, m)) => format!("{} {y}", full_month(m.parse().unwrap_or(0))),
        None => period.to_string(),
    }
}

/// The period before, as the comparison names it: "October" or "2025".
fn previous_label(period: &str) -> String {
    match period.split_once('-') {
        Some((_, m)) => {
            let m: u32 = m.parse().unwrap_or(1);
            full_month(if m == 1 { 12 } else { m - 1 })
        }
        None => period.parse::<i32>().map(|y| (y - 1).to_string()).unwrap_or_default(),
    }
}

/// A duration as a big number and its unit: (42, "hours") or (35, "minutes").
fn big_time(secs: i64, t: &dyn Fn(&str) -> String) -> (String, String) {
    if secs >= 3600 {
        let hours = secs as f64 / 3600.0;
        let value = if hours < 10.0 { format!("{:.1}", hours) } else { group(hours.round() as i64) };
        (value, t("hours"))
    } else {
        ((secs / 60).to_string(), t("minutes"))
    }
}

/// The listener a peak hour makes you.
fn listener_kind(hour: usize) -> (&'static str, &'static str) {
    match hour {
        5..=8 => ("early_riser", "ph-sun-horizon"),
        9..=11 => ("morning", "ph-sun-dim"),
        12..=16 => ("afternoon", "ph-sun"),
        17..=20 => ("evening", "ph-cloud-moon"),
        _ => ("night_owl", "ph-moon-stars"),
    }
}

/// Change from the period before, as a whole percentage.
fn change_pct(now: i64, before: i64) -> Option<i64> {
    (before > 0 && now > 0).then(|| ((now - before) as f64 / before as f64 * 100.0).round() as i64)
}

enum Step {
    Next,
    Prev,
    Restart,
    TogglePause,
    Hold(bool),
    Count(usize),
}

#[derive(Clone, PartialEq, Default)]
struct Story {
    index: usize,
    count: usize,
    paused: bool,
    held: bool,
}

impl Reducible for Story {
    type Action = Step;
    fn reduce(self: Rc<Self>, step: Step) -> Rc<Self> {
        let mut s = (*self).clone();
        match step {
            Step::Next => s.index = (s.index + 1).min(s.count.saturating_sub(1)),
            Step::Prev => s.index = s.index.saturating_sub(1),
            Step::Restart => {
                s.index = 0;
                s.paused = false;
            }
            Step::TogglePause => s.paused = !s.paused,
            Step::Hold(h) => s.held = h,
            Step::Count(n) => {
                s.count = n;
                s.index = s.index.min(n.saturating_sub(1));
            }
        }
        Rc::new(s)
    }
}

struct Slide {
    theme: &'static str,
    body: Html,
}

fn art(src: &str, class: &'static str) -> Html {
    html! { <FallbackImage size={Some(480)} src={src.to_string()} alt="" class={class.to_string()} loading="eager" /> }
}

fn podcast_list(podcasts: &[ReportPodcast]) -> Html {
    html! {
        <ol class="rw-list">
            { for podcasts.iter().take(5).enumerate().map(|(i, p)| html! {
                <li class="rw-list-row rw-rise" style={format!("animation-delay: {}ms;", 250 + i * 140)}>
                    <span class="rw-list-rank">{ i + 1 }</span>
                    { art(&p.artwork, "rw-list-art") }
                    <span class="rw-list-name">{ &p.name }</span>
                    <span class="rw-list-time">{ fmt_secs(p.seconds) }</span>
                </li>
            })}
        </ol>
    }
}

fn episode_card(e: &ReportEpisode, delay: u32) -> Html {
    html! {
        <div class="rw-episode rw-rise" style={format!("animation-delay: {delay}ms;")}>
            { art(&e.artwork, "rw-episode-art") }
            <span class="rw-episode-title">{ &e.title }</span>
            <span class="rw-episode-show">{ &e.podcast }</span>
        </div>
    }
}

fn big(value: String, unit: String, delay: u32) -> Html {
    html! {
        <div class="rw-big rw-pop" style={format!("animation-delay: {delay}ms;")}>
            <span class="rw-big-value">{ value }</span>
            <span class="rw-big-unit">{ unit }</span>
        </div>
    }
}

fn line(text: String, class: &'static str, delay: u32) -> Html {
    html! { <p class={classes!(class, "rw-rise")} style={format!("animation-delay: {delay}ms;")}>{ text }</p> }
}

/// Days and times only cover the log: say where they start when part of the period doesn't.
fn times_note(rewind: &Rewind, t: &dyn Fn(&str) -> String) -> Html {
    match rewind.history_until.as_deref() {
        Some(day) => html! { <p class="rw-note">{ t("times_from").replace("{date}", &fmt_day(day)) }</p> },
        None => html! {},
    }
}

fn slides(rewind: &Rewind, t: &dyn Fn(&str) -> String, actions: Html) -> Vec<Slide> {
    let r: &ListeningReport = &rewind.report;
    let label = period_label(&rewind.period);
    let is_year = !rewind.period.contains('-');
    let mut out = vec![Slide {
        theme: "rw-t-intro",
        body: html! {
            <>
                <img class="rw-logo rw-pop" src="static/assets/favicon.png" alt="" />
                { line(t("intro_kicker"), "rw-kicker", 200) }
                <h2 class="rw-title rw-rise" style="animation-delay: 400ms;">{ t("intro_title").replace("{period}", &label) }</h2>
                { line(t("intro_sub"), "rw-sub", 800) }
            </>
        },
    }];

    if r.seconds <= 0 {
        out.push(Slide {
            theme: "rw-t-calm",
            body: html! {
                <>
                    <h2 class="rw-title rw-rise">{ t("empty_title") }</h2>
                    { line(t("empty_sub").replace("{period}", &label), "rw-sub", 300) }
                    { actions }
                </>
            },
        });
        return out;
    }

    // How much.
    let (value, unit) = big_time(r.seconds, t);
    let change = change_pct(r.seconds, rewind.previous_seconds).map(|pct| {
        let key = if pct >= 0 { "more_than" } else { "less_than" };
        t(key).replace("{pct}", &pct.abs().to_string()).replace("{period}", &previous_label(&rewind.period))
    });
    let days = r.seconds / 86_400;
    out.push(Slide {
        theme: "rw-t-time",
        body: html! {
            <>
                { line(t("time_kicker"), "rw-kicker", 100) }
                { big(value, unit, 250) }
                if days >= 1 {
                    { line(t("time_days").replace("{days}", &days.to_string()), "rw-sub", 700) }
                }
                <div class="rw-chips rw-rise" style="animation-delay: 900ms;">
                    <span class="rw-chip"><i class="ph ph-microphone-stage"></i>{ t("n_podcasts").replace("{n}", &group(r.podcasts)) }</span>
                    <span class="rw-chip"><i class="ph ph-play"></i>{ t("n_episodes").replace("{n}", &group(r.episodes)) }</span>
                    if r.finished > 0 {
                        <span class="rw-chip"><i class="ph ph-check-circle"></i>{ t("n_finished").replace("{n}", &group(r.finished)) }</span>
                    }
                </div>
                if let Some(change) = change {
                    { line(change, "rw-change", 1200) }
                }
            </>
        },
    });

    if let Some(first) = rewind.first_play.as_ref() {
        out.push(Slide {
            theme: "rw-t-dawn",
            body: html! {
                <>
                    { line(t("first_kicker").replace("{period}", &label), "rw-kicker", 100) }
                    { episode_card(first, 350) }
                </>
            },
        });
    }

    // What.
    if let Some(top) = r.top_podcasts.first() {
        let share = (top.seconds as f64 / r.seconds.max(1) as f64 * 100.0).round() as i64;
        out.push(Slide {
            theme: "rw-t-art",
            body: html! {
                <>
                    { line(t("top_show_kicker"), "rw-kicker", 100) }
                    <div class="rw-hero-art rw-pop" style="animation-delay: 300ms;">{ art(&top.artwork, "rw-hero-img") }</div>
                    <h2 class={classes!("rw-title", "rw-rise", (top.name.chars().count() > 28).then_some("is-long"))} style="animation-delay: 700ms;">{ &top.name }</h2>
                    { line(t("top_show_sub").replace("{time}", &fmt_secs(top.seconds)).replace("{n}", &top.episodes.to_string()), "rw-sub", 950) }
                    if share >= 5 && r.top_podcasts.len() > 1 {
                        { line(t("top_show_share").replace("{pct}", &share.to_string()), "rw-change", 1200) }
                    }
                </>
            },
        });
    }
    if r.top_podcasts.len() >= 3 {
        out.push(Slide {
            theme: "rw-t-list",
            body: html! {
                <>
                    { line(t("top_shows_kicker"), "rw-kicker", 100) }
                    { podcast_list(&r.top_podcasts) }
                </>
            },
        });
    }
    if let Some(ep) = r.top_episodes.first() {
        out.push(Slide {
            theme: "rw-t-ember",
            body: html! {
                <>
                    { line(t("top_episode_kicker"), "rw-kicker", 100) }
                    { episode_card(ep, 300) }
                    { line(t("top_episode_sub").replace("{time}", &fmt_secs(ep.seconds)), "rw-sub", 900) }
                </>
            },
        });
    }
    if rewind.new_podcast_count > 0 {
        out.push(Slide {
            theme: "rw-t-fresh",
            body: html! {
                <>
                    { line(t("new_kicker"), "rw-kicker", 100) }
                    { big(rewind.new_podcast_count.to_string(), if rewind.new_podcast_count == 1 { t("new_show") } else { t("new_shows") }, 250) }
                    <div class="rw-art-row">
                        { for rewind.new_podcasts.iter().enumerate().map(|(i, p)| html! {
                            <div class="rw-art-tile rw-pop" style={format!("animation-delay: {}ms;", 600 + i * 120)} title={p.name.clone()}>
                                { art(&p.artwork, "rw-art-img") }
                            </div>
                        })}
                    </div>
                    if let Some(p) = rewind.new_podcasts.first() {
                        { line(t("new_sub").replace("{name}", &p.name), "rw-sub", 1200) }
                    }
                </>
            },
        });
    }
    if !r.top_categories.is_empty() {
        let total: i64 = r.top_categories.iter().map(|c| c.seconds).sum::<i64>().max(1);
        out.push(Slide {
            theme: "rw-t-genre",
            body: html! {
                <>
                    { line(t("genres_kicker"), "rw-kicker", 100) }
                    <h2 class="rw-title rw-pop" style="animation-delay: 300ms;">{ &r.top_categories[0].name }</h2>
                    <div class="rw-bars">
                        { for r.top_categories.iter().take(5).enumerate().map(|(i, c)| {
                            let pct = (c.seconds as f64 / total as f64 * 100.0).round() as i64;
                            html! {
                                <div class="rw-bar-row rw-rise" style={format!("animation-delay: {}ms;", 600 + i * 120)}>
                                    <span class="rw-bar-name">{ &c.name }</span>
                                    <span class="rw-bar-track"><span class="rw-bar-fill" style={format!("width: {}%;", pct.max(3))}></span></span>
                                    <span class="rw-bar-pct">{ format!("{pct}%") }</span>
                                </div>
                            }
                        })}
                    </div>
                </>
            },
        });
    }
    if !r.top_people.is_empty() {
        out.push(Slide {
            theme: "rw-t-voices",
            body: html! {
                <>
                    { line(t("people_kicker"), "rw-kicker", 100) }
                    <div class="rw-people">
                        { for r.top_people.iter().take(3).enumerate().map(|(i, p)| html! {
                            <div class="rw-person rw-pop" style={format!("animation-delay: {}ms;", 300 + i * 200)}>
                                if let Some(img) = p.image.as_ref() {
                                    <img class="rw-person-img" src={img.clone()} alt="" />
                                } else {
                                    <span class="rw-person-img rw-initial">{ p.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default() }</span>
                                }
                                <span class="rw-person-name">{ &p.name }</span>
                                <span class="rw-person-time">{ fmt_secs(p.seconds) }</span>
                            </div>
                        })}
                    </div>
                </>
            },
        });
    }

    // When.
    if let Some(hour) = peak(&r.by_hour) {
        let (kind, icon) = listener_kind(hour);
        let max = r.by_hour.iter().copied().max().unwrap_or(1).max(1);
        let dow = peak(&r.by_dow).map(|d| t(&format!("day_{d}")));
        out.push(Slide {
            theme: "rw-t-night",
            body: html! {
                <>
                    { line(t("when_kicker"), "rw-kicker", 100) }
                    <i class={classes!("ph", icon, "rw-icon", "rw-pop")} style="animation-delay: 250ms;"></i>
                    <h2 class="rw-title rw-rise" style="animation-delay: 450ms;">{ t(&format!("kind_{kind}")) }</h2>
                    { line(t("when_sub").replace("{hour}", &hour_label(hour as i32)), "rw-sub", 700) }
                    <div class="rw-hours rw-rise" style="animation-delay: 900ms;" aria-hidden="true">
                        { for r.by_hour.iter().enumerate().map(|(h, v)| html! {
                            <span class={classes!("rw-hour", (h == hour).then_some("is-peak"))} style={format!("height: {}%;", (*v as f64 / max as f64 * 100.0).max(3.0))}></span>
                        })}
                    </div>
                    if let Some(dow) = dow {
                        { line(t("when_day").replace("{day}", &dow), "rw-sub", 1100) }
                    }
                    { times_note(rewind, t) }
                </>
            },
        });
    }
    if r.biggest_day.is_some() || r.longest_streak > 1 {
        out.push(Slide {
            theme: "rw-t-streak",
            body: html! {
                <>
                    if r.longest_streak > 1 {
                        { line(t("streak_kicker"), "rw-kicker", 100) }
                        { big(r.longest_streak.to_string(), t("days_in_a_row"), 250) }
                    }
                    if let Some(day) = r.biggest_day.as_ref() {
                        { line(t("biggest_day").replace("{date}", &fmt_day(&day.date)).replace("{time}", &fmt_secs(day.seconds)), "rw-sub", 700) }
                    }
                    { line(t("active_days").replace("{n}", &r.active_days.to_string()), "rw-sub", 900) }
                    { times_note(rewind, t) }
                </>
            },
        });
    }
    if is_year && r.by_month.iter().filter(|m| m.seconds > 0).count() >= 2 {
        let max = r.by_month.iter().map(|m| m.seconds).max().unwrap_or(1).max(1);
        let best = r.by_month.iter().max_by_key(|m| m.seconds);
        out.push(Slide {
            theme: "rw-t-time",
            body: html! {
                <>
                    { line(t("months_kicker"), "rw-kicker", 100) }
                    if let Some(best) = best {
                        <h2 class="rw-title rw-pop" style="animation-delay: 250ms;">{ full_month(best.month.get(5..).and_then(|m| m.parse().ok()).unwrap_or(0)) }</h2>
                    }
                    <div class="rw-months rw-rise" style="animation-delay: 600ms;">
                        { for r.by_month.iter().map(|m| html! {
                            <span class="rw-month">
                                <span class={classes!("rw-month-bar", best.is_some_and(|b| b.month == m.month).then_some("is-peak"))} style={format!("height: {}%;", (m.seconds as f64 / max as f64 * 100.0).max(2.0))}></span>
                                <span class="rw-month-label">{ full_month(m.month.get(5..).and_then(|x| x.parse().ok()).unwrap_or(0)).chars().next().unwrap_or(' ') }</span>
                            </span>
                        })}
                    </div>
                    { times_note(rewind, t) }
                </>
            },
        });
    }

    // How.
    let saved = r.time_saved.speed.max(0) + r.time_saved.silence + r.time_saved.ads;
    let speed = r.average_speed.filter(|s| (*s - 1.0).abs() >= 0.05);
    let day_one = r.release_lag.as_ref().filter(|l| l.episodes >= 3 && l.within_day > 0.0);
    if saved >= 60 || speed.is_some() || day_one.is_some() {
        out.push(Slide {
            theme: "rw-t-fresh",
            body: html! {
                <>
                    { line(t("habits_kicker"), "rw-kicker", 100) }
                    <div class="rw-facts">
                        if let Some(s) = speed {
                            <div class="rw-fact rw-rise" style="animation-delay: 300ms;">
                                <i class="ph ph-gauge"></i>
                                <strong>{ format!("{s:.2}×").replace(".00", "") }</strong>
                                <span>{ t("habit_speed") }</span>
                            </div>
                        }
                        if saved >= 60 {
                            <div class="rw-fact rw-rise" style="animation-delay: 500ms;">
                                <i class="ph ph-lightning"></i>
                                <strong>{ fmt_secs(saved) }</strong>
                                <span>{ t("habit_saved") }</span>
                            </div>
                        }
                        if let Some(lag) = day_one {
                            <div class="rw-fact rw-rise" style="animation-delay: 700ms;">
                                <i class="ph ph-rocket-launch"></i>
                                <strong>{ format!("{}%", (lag.within_day * 100.0).round() as i64) }</strong>
                                <span>{ t("habit_day_one") }</span>
                            </div>
                        }
                    </div>
                </>
            },
        });
    }

    // The summary card.
    let (value, unit) = big_time(r.seconds, t);
    out.push(Slide {
        theme: "rw-t-summary",
        body: html! {
            <>
                <div class="rw-card rw-pop">
                    <div class="rw-card-head">
                        <img src="static/assets/favicon.png" alt="" />
                        <span>{ t("card_title").replace("{period}", &label) }</span>
                    </div>
                    if let Some(top) = r.top_podcasts.first() {
                        <div class="rw-card-top">
                            { art(&top.artwork, "rw-card-art") }
                            <div>
                                <span class="rw-card-label">{ t("card_top_show") }</span>
                                <strong>{ &top.name }</strong>
                            </div>
                        </div>
                    }
                    <div class="rw-card-grid">
                        <div><span class="rw-card-label">{ t("card_listened") }</span><strong>{ format!("{value} {unit}") }</strong></div>
                        <div><span class="rw-card-label">{ t("card_episodes") }</span><strong>{ group(r.episodes) }</strong></div>
                        <div><span class="rw-card-label">{ t("card_shows") }</span><strong>{ group(r.podcasts) }</strong></div>
                        if let Some(c) = r.top_categories.first() {
                            <div><span class="rw-card-label">{ t("card_genre") }</span><strong>{ &c.name }</strong></div>
                        } else {
                            <div><span class="rw-card-label">{ t("card_days") }</span><strong>{ r.active_days }</strong></div>
                        }
                    </div>
                    if r.top_podcasts.len() > 1 {
                        <ol class="rw-card-list">
                            { for r.top_podcasts.iter().skip(1).take(4).enumerate().map(|(i, p)| html! { <li><span>{ i + 2 }</span>{ &p.name }</li> }) }
                        </ol>
                    }
                </div>
                { actions }
            </>
        },
    });
    out
}

#[derive(Properties, PartialEq)]
pub struct RewindPlayerProps {
    /// `YYYY` or `YYYY-MM`.
    pub period: String,
    pub on_close: Callback<()>,
}

/// The Rewind story, full screen. Tap the right side (or press →) for the next slide, the left
/// for the previous; hold or press space to pause; Escape closes.
#[function_component(RewindPlayer)]
pub fn rewind_player(props: &RewindPlayerProps) -> Html {
    let (i18n, _) = use_translation();
    let t = |k: &str| i18n.t(&format!("rewind.{k}"));
    let (state, _) = use_store::<AppState>();
    let data = use_state(|| None::<Result<Rewind, String>>);
    let story = use_reducer(Story::default);
    let pressed_at = use_mut_ref(|| 0.0f64);

    {
        let data = data.clone();
        let api_key = state.auth_details.as_ref().and_then(|a| a.api_key.clone());
        let server = state.auth_details.as_ref().map(|a| a.server_name.clone());
        let user_id = state.user_details.as_ref().map(|u| u.UserID);
        use_effect_with(props.period.clone(), move |period| {
            if let (Some(key), Some(server), Some(user_id)) = (api_key, server, user_id) {
                let period = period.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    data.set(Some(stat_reqs::call_get_rewind(&server, &key, user_id, &period).await.map_err(|e| e.to_string())));
                });
            }
            || ()
        });
    }

    // Keys, and no page scrolling behind the story.
    {
        let (story, on_close) = (story.dispatcher(), props.on_close.clone());
        use_effect_with((), move |_| {
            let document = web_sys::window().and_then(|w| w.document());
            let listener = document.as_ref().map(|doc| {
                // Not passive, so space can be kept from scrolling or pressing a button.
                gloo_events::EventListener::new_with_options(doc, "keydown", gloo_events::EventListenerOptions::enable_prevent_default(), move |event| {
                    let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() else { return };
                    match key.key().as_str() {
                        "Escape" => on_close.emit(()),
                        "ArrowRight" => story.dispatch(Step::Next),
                        "ArrowLeft" => story.dispatch(Step::Prev),
                        " " => {
                            key.prevent_default();
                            story.dispatch(Step::TogglePause);
                        }
                        _ => {}
                    }
                })
            });
            let body = document.and_then(|d| d.body());
            if let Some(body) = &body {
                let _ = body.class_list().add_1("ep-modal-open");
            }
            move || {
                drop(listener);
                if let Some(body) = body {
                    let _ = body.class_list().remove_1("ep-modal-open");
                }
            }
        });
    }

    let close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| on_close.emit(()))
    };
    let actions = {
        let restart = {
            let story = story.dispatcher();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                story.dispatch(Step::Restart);
            })
        };
        html! {
            <div class="rw-actions rw-rise" style="animation-delay: 500ms;">
                <button type="button" class="rw-btn" onclick={restart}><i class="ph ph-arrow-counter-clockwise"></i>{ t("replay") }</button>
                <button type="button" class="rw-btn is-primary" onclick={close.clone()}>{ t("done") }</button>
            </div>
        }
    };

    let deck = match data.as_ref() {
        Some(Ok(rewind)) => slides(rewind, &t, actions),
        _ => Vec::new(),
    };
    {
        let (story, n) = (story.dispatcher(), deck.len());
        use_effect_with(n, move |n| {
            story.dispatch(Step::Count(*n));
            || ()
        });
    }
    let accent = use_art_accent(
        data.as_ref()
            .and_then(|d| d.as_ref().ok())
            .and_then(|r| r.report.top_podcasts.first())
            .map(|p| p.artwork.as_str())
            .unwrap_or(""),
    );

    let index = story.index.min(deck.len().saturating_sub(1));
    let paused = story.paused || story.held;
    let last = index + 1 >= deck.len();

    // A tap moves; a hold (over a third of a second) only pauses.
    let now = || web_sys::window().and_then(|w| w.performance()).map(|p| p.now()).unwrap_or(0.0);
    let on_down = {
        let (story, pressed_at) = (story.dispatcher(), pressed_at.clone());
        Callback::from(move |_: PointerEvent| {
            *pressed_at.borrow_mut() = now();
            story.dispatch(Step::Hold(true));
        })
    };
    let on_up = {
        let story = story.dispatcher();
        Callback::from(move |_: PointerEvent| story.dispatch(Step::Hold(false)))
    };
    let tap = |step: fn() -> Step| {
        let (story, pressed_at) = (story.dispatcher(), pressed_at.clone());
        Callback::from(move |_: MouseEvent| {
            if now() - *pressed_at.borrow() < 350.0 {
                story.dispatch(step());
            }
        })
    };
    let on_ended = {
        let story = story.dispatcher();
        Callback::from(move |_: AnimationEvent| story.dispatch(Step::Next))
    };
    let toggle = {
        let story = story.dispatcher();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            story.dispatch(Step::TogglePause);
        })
    };
    let label = period_label(&props.period);
    let theme = deck.get(index).map(|s| s.theme).unwrap_or("rw-t-intro");

    html! {
        <div class="rw-overlay" role="dialog" aria-modal="true" aria-label={t("title").replace("{period}", &label)}>
            <div class={classes!("rw-stage", theme, paused.then_some("is-paused"))} style={accent_style(&accent)}
                 onpointerdown={on_down} onpointerup={on_up.clone()} onpointerleave={on_up}>
                <div class="rw-progress">
                    { for (0..deck.len().max(1)).map(|i| {
                        let fill = if i < index {
                            html! { <span class="rw-seg-fill is-done"></span> }
                        } else if i == index && !deck.is_empty() {
                            html! {
                                <span key={format!("seg-{index}")} class={classes!("rw-seg-fill", "is-active", last.then_some("is-last"))}
                                      style={format!("animation-duration: {SLIDE_SECONDS}s;")}
                                      onanimationend={(!last).then(|| on_ended.clone())}></span>
                            }
                        } else {
                            html! {}
                        };
                        html! { <span class="rw-seg">{ fill }</span> }
                    })}
                </div>
                <div class="rw-top">
                    <span class="rw-brand">{ t("title").replace("{period}", &label) }</span>
                    <button type="button" class="rw-icon-btn" onclick={toggle} aria-label={if story.paused { t("play") } else { t("pause") }}>
                        <i class={classes!("ph", if story.paused { "ph-play" } else { "ph-pause" })}></i>
                    </button>
                    <button type="button" class="rw-icon-btn" onclick={close.clone()} aria-label={t("close")}>
                        <i class="ph ph-x"></i>
                    </button>
                </div>
                <button type="button" class="rw-tap rw-tap-prev" aria-label={t("previous")} onclick={tap(|| Step::Prev)}></button>
                <button type="button" class="rw-tap rw-tap-next" aria-label={t("next")} onclick={tap(|| Step::Next)}></button>
                <div class="rw-slide" key={format!("slide-{index}")} aria-live="polite">
                    { match data.as_ref() {
                        None => html! { <div class="rw-loading"><i class="ph ph-circle-notch"></i></div> },
                        Some(Err(_)) => html! {
                            <>
                                <h2 class="rw-title">{ t("error") }</h2>
                                <div class="rw-actions"><button type="button" class="rw-btn is-primary" onclick={close}>{ t("close") }</button></div>
                            </>
                        },
                        Some(Ok(_)) => deck.into_iter().nth(index).map(|s| s.body).unwrap_or_default(),
                    }}
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn labels_and_numbers() {
        assert_eq!(period_label("2026-11"), "November 2026");
        assert_eq!(period_label("2026"), "2026");
        assert_eq!(previous_label("2026-01"), "December");
        assert_eq!(previous_label("2026"), "2025");
        assert_eq!(change_pct(150, 100), Some(50));
        assert_eq!(change_pct(50, 100), Some(-50));
        assert_eq!(change_pct(50, 0), None);
        assert_eq!(listener_kind(23).0, "night_owl");
        assert_eq!(listener_kind(7).0, "early_riser");
    }
}
