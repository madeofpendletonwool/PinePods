use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState, UserPreferencesState, UserStatsStore};
use crate::components::gen_components::{FallbackImage, Search_nav};
use crate::components::gen_funcs::{format_date, format_time};
use crate::components::i18n::use_translation;
use crate::requests::pod_req::call_get_pinepods_version;
use crate::requests::stat_reqs::{self, DayStat, ExtendedUserStats, ListeningReport, MonthStat, NamedSeconds};
use chrono::{Datelike, Duration, NaiveDate};
use yew::prelude::*;
use yewdux::prelude::*;

/// Days the activity heatmap covers (matches the server).
const HEATMAP_DAYS: i64 = 182;

/// "3h 12m", "12m", or "0m".
fn fmt_secs(secs: i64) -> String {
    let (h, m) = (secs / 3600, (secs % 3600) / 60);
    if h > 0 { format!("{h}h {m}m") } else { format!("{m}m") }
}

/// Large totals as whole hours ("1,234h"), smaller ones as "3h 12m".
fn fmt_total(secs: i64) -> String {
    if secs >= 100 * 3600 { format!("{}h", group(secs / 3600)) } else { fmt_secs(secs) }
}

/// 12345 -> "12,345"
fn group(n: i64) -> String {
    let digits = n.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 { format!("-{out}") } else { out }
}

fn hour_label(h: i32) -> String {
    match h {
        0 => "12 AM".into(),
        12 => "12 PM".into(),
        h if h < 12 => format!("{h} AM"),
        h => format!("{} PM", h - 12),
    }
}

fn month_label(month: &str) -> String {
    const NAMES: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    month
        .split('-')
        .nth(1)
        .and_then(|m| m.parse::<usize>().ok())
        .and_then(|m| NAMES.get(m.wrapping_sub(1)))
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn today_in(tz: Option<&str>) -> NaiveDate {
    let now = chrono::Utc::now();
    tz.and_then(|t| t.parse::<chrono_tz::Tz>().ok())
        .map(|tz| now.with_timezone(&tz).date_naive())
        .unwrap_or_else(|| now.date_naive())
}

/// Vertical bars with labels under some of them.
fn bar_chart(values: &[i64], labels: &[String], highlight: Option<usize>, class: &str) -> Html {
    let max = values.iter().copied().max().unwrap_or(0).max(1);
    html! {
        <div class={classes!("us-bars", class.to_string())}>
            { for values.iter().enumerate().map(|(i, v)| {
                let pct = if *v > 0 { ((*v as f64 / max as f64) * 100.0).max(3.0) } else { 0.0 };
                let label = labels.get(i).cloned().unwrap_or_default();
                html! {
                    <div class={classes!("us-bar", (highlight == Some(i)).then_some("is-peak"))} title={format!("{label}: {}", fmt_secs(*v))}>
                        <div class="us-bar-track"><div class="us-bar-fill" style={format!("height: {pct:.1}%;")}></div></div>
                        <span class="us-bar-label">{ label }</span>
                    </div>
                }
            })}
        </div>
    }
}

/// GitHub-style grid of `first..=today`: one column per week, Sunday at the top.
fn heatmap(days: &[DayStat], first: NaiveDate, today: NaiveDate, less: String, more: String) -> Html {
    let by_date: std::collections::HashMap<NaiveDate, i64> = days
        .iter()
        .filter_map(|d| parse_day(&d.date).map(|n| (n, d.seconds)))
        .collect();
    // Start on the Sunday on or before the first day.
    let start = first - Duration::days(first.weekday().num_days_from_sunday() as i64);
    let max = by_date.values().copied().max().unwrap_or(0).max(1);
    let level = |secs: i64| -> u8 {
        if secs <= 0 { 0 } else {
            let r = secs as f64 / max as f64;
            if r > 0.75 { 4 } else if r > 0.45 { 3 } else if r > 0.2 { 2 } else { 1 }
        }
    };
    let weeks = ((today - start).num_days() / 7 + 1) as usize;

    html! {
        <div class="us-heat-wrap">
            <div class="us-heat" style={format!("grid-template-columns: repeat({weeks}, 1fr);")}>
                { for (0..weeks).flat_map(|w| (0..7).map(move |d| (w, d))).map(|(w, d)| {
                    let date = start + Duration::days((w * 7 + d) as i64);
                    if date < first || date > today {
                        return html! { <span class="us-heat-cell is-out" style={format!("grid-column: {}; grid-row: {};", w + 1, d + 1)}></span> };
                    }
                    let secs = by_date.get(&date).copied().unwrap_or(0);
                    html! {
                        <span
                            class={format!("us-heat-cell l{}", level(secs))}
                            style={format!("grid-column: {}; grid-row: {};", w + 1, d + 1)}
                            title={format!("{}: {}", date.format("%b %-d, %Y"), fmt_secs(secs))}
                        ></span>
                    }
                })}
            </div>
            <div class="us-heat-legend">
                <span>{ less }</span>
                { for (0..5).map(|l| html! { <span class={format!("us-heat-cell l{l}")}></span> }) }
                <span>{ more }</span>
            </div>
        </div>
    }
}

/// One row of a ranked list.
struct Ranked {
    name: String,
    sub: Option<String>,
    image: Option<String>,
    /// A person: round picture, or their initial.
    person: bool,
    seconds: i64,
}

fn ranked(rows: Vec<Ranked>) -> Html {
    let max = rows.iter().map(|r| r.seconds).max().unwrap_or(0).max(1);
    html! {
        <ol class="us-top">
            { for rows.into_iter().enumerate().map(|(i, r)| {
                let pct = (r.seconds as f64 / max as f64 * 100.0).max(2.0);
                let picture = match (&r.image, r.person) {
                    (Some(src), false) => html! { <FallbackImage size={Some(128)} src={src.clone()} alt={r.name.clone()} class="us-top-art" /> },
                    (Some(src), true) => html! { <img class="us-top-art is-person" src={src.clone()} alt="" loading="lazy" /> },
                    (None, true) => html! { <span class="us-top-art is-person us-initial">{ r.name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default() }</span> },
                    (None, false) => html! {},
                };
                html! {
                    <li class="us-top-row">
                        <span class="us-top-rank">{ i + 1 }</span>
                        { picture }
                        <div class="us-top-body">
                            <div class="us-top-line">
                                <span class="us-top-name" title={r.name.clone()}>{ &r.name }</span>
                                <span class="us-top-time">{ fmt_secs(r.seconds) }</span>
                            </div>
                            if let Some(sub) = &r.sub {
                                <span class="us-top-sub">{ sub }</span>
                            }
                            <div class="us-meter"><div style={format!("width: {pct:.1}%;")}></div></div>
                        </div>
                    </li>
                }
            })}
        </ol>
    }
}

/// Shares of the total, as a list of meters.
fn shares(rows: &[(String, i64)]) -> Html {
    let total: i64 = rows.iter().map(|(_, s)| *s).sum::<i64>().max(1);
    html! {
        <div class="us-cats">
            { for rows.iter().map(|(name, secs)| {
                let pct = (*secs as f64 / total as f64 * 100.0).round() as i64;
                html! {
                    <div class="us-cat">
                        <div class="us-top-line">
                            <span class="us-top-name">{ name }</span>
                            <span class="us-top-time">{ format!("{pct}% · {}", fmt_secs(*secs)) }</span>
                        </div>
                        <div class="us-meter"><div style={format!("width: {pct}%;")}></div></div>
                    </div>
                }
            })}
        </div>
    }
}

fn parse_day(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()
}

fn fmt_day(s: &str) -> String {
    parse_day(s).map(|d| d.format("%b %-d, %Y").to_string()).unwrap_or_default()
}

fn peak(values: &[i64]) -> Option<usize> {
    values.iter().enumerate().filter(|(_, v)| **v > 0).max_by_key(|(_, v)| **v).map(|(i, _)| i)
}

#[derive(Properties, PartialEq)]
struct TileProps {
    icon: &'static str,
    value: String,
    label: String,
    #[prop_or_default]
    sub: Option<String>,
}

#[function_component(Tile)]
fn tile(props: &TileProps) -> Html {
    html! {
        <div class="us-tile">
            <i class={classes!("ph", props.icon, "us-tile-icon")}></i>
            <span class="us-tile-value">{ &props.value }</span>
            <span class="us-tile-label">{ &props.label }</span>
            if let Some(sub) = &props.sub {
                <span class="us-tile-sub">{ sub }</span>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct PanelProps {
    title: String,
    #[prop_or_default]
    note: Option<String>,
    #[prop_or_default]
    class: Classes,
    children: Html,
}

#[function_component(Panel)]
fn panel(props: &PanelProps) -> Html {
    html! {
        <section class={classes!("us-panel", props.class.clone())}>
            <header class="us-panel-head">
                <h2>{ &props.title }</h2>
                if let Some(note) = &props.note {
                    <span class="us-panel-note">{ note }</span>
                }
            </header>
            { props.children.clone() }
        </section>
    }
}

/// Time saved by speed and skipping, once there is a minute of it.
fn saved_tile(saved: Option<&stat_reqs::TimeSaved>, t: &dyn Fn(&str) -> String) -> Html {
    let Some(saved) = saved else { return Html::default() };
    let skipped = saved.silence.max(0) + saved.ads.max(0);
    let total = saved.speed.max(0) + skipped;
    if total < 60 {
        return Html::default();
    }
    let sub = t("time_saved_detail").replace("{speed}", &fmt_secs(saved.speed.max(0))).replace("{skipped}", &fmt_secs(skipped));
    html! { <Tile icon="ph-lightning" value={fmt_total(total)} label={t("time_saved")} sub={Some(sub)} /> }
}

/// The period the page shows.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Period {
    All,
    Year(i32),
    Month(i32, u32),
}

impl Period {
    /// First and last day.
    fn range(self) -> Option<(NaiveDate, NaiveDate)> {
        let (first, last) = match self {
            Period::All => return None,
            Period::Year(y) => (NaiveDate::from_ymd_opt(y, 1, 1)?, NaiveDate::from_ymd_opt(y, 12, 31)?),
            Period::Month(y, m) => {
                let first = NaiveDate::from_ymd_opt(y, m, 1)?;
                (first, first.checked_add_months(chrono::Months::new(1))?.pred_opt()?)
            }
        };
        Some((first, last))
    }

    fn title(self, t: &dyn Fn(&str) -> String) -> String {
        match self {
            Period::All => t("period_all"),
            Period::Year(y) => y.to_string(),
            Period::Month(y, m) => format!("{} {y}", full_month(m)),
        }
    }
}

fn full_month(m: u32) -> String {
    const NAMES: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    NAMES.get(m.wrapping_sub(1) as usize).map(|s| s.to_string()).unwrap_or_default()
}

fn dow_labels(t: &dyn Fn(&str) -> String) -> Vec<String> {
    ["sun", "mon", "tue", "wed", "thu", "fri", "sat"].iter().map(|d| t(&format!("dow_{d}"))).collect()
}

fn hour_labels() -> Vec<String> {
    (0..24).map(|h| if h % 6 == 0 { hour_label(h) } else { String::new() }).collect()
}

/// One month as a calendar, Sunday first, each day shaded by how much was played.
fn month_calendar(days: &[DayStat], first: NaiveDate, last: NaiveDate, dows: &[String]) -> Html {
    let by_date: std::collections::HashMap<NaiveDate, i64> = days.iter().filter_map(|d| parse_day(&d.date).map(|n| (n, d.seconds))).collect();
    let max = by_date.values().copied().max().unwrap_or(0).max(1);
    let level = |secs: i64| -> u8 {
        if secs <= 0 { 0 } else {
            let r = secs as f64 / max as f64;
            if r > 0.75 { 4 } else if r > 0.45 { 3 } else if r > 0.2 { 2 } else { 1 }
        }
    };
    let lead = first.weekday().num_days_from_sunday() as usize;
    let month_end = first.checked_add_months(chrono::Months::new(1)).and_then(|d| d.pred_opt()).unwrap_or(last);
    let count = (month_end - first).num_days() as usize + 1;
    html! {
        <div class="us-cal">
            { for dows.iter().map(|d| html! { <span class="us-cal-dow">{ d }</span> }) }
            { for (0..lead).map(|_| html! { <span class="us-cal-day is-out"></span> }) }
            { for (0..count).map(|i| {
                let date = first + Duration::days(i as i64);
                let secs = by_date.get(&date).copied().unwrap_or(0);
                let future = date > last;
                html! {
                    <span class={classes!("us-cal-day", format!("l{}", level(secs)), future.then_some("is-future"))}
                          title={format!("{}: {}", date.format("%b %-d"), fmt_secs(secs))}>{ date.day() }</span>
                }
            })}
        </div>
    }
}

fn episodes_label(n: i64, t: &dyn Fn(&str) -> String) -> String {
    if n == 1 { t("episodes_count_one") } else { t("episodes_count").replace("{count}", &group(n)) }
}

fn top_podcast_rows(report: &ListeningReport, t: &dyn Fn(&str) -> String) -> Vec<Ranked> {
    report
        .top_podcasts
        .iter()
        .map(|p| Ranked {
            name: p.name.clone(),
            sub: Some(episodes_label(p.episodes, t)),
            image: Some(p.artwork.clone()),
            person: false,
            seconds: p.seconds,
        })
        .collect()
}

fn category_rows(cats: &[NamedSeconds]) -> Vec<(String, i64)> {
    cats.iter().take(6).map(|c| (c.name.clone(), c.seconds)).collect()
}

/// All time: the calendar figures from the extended stats (which reach back before the
/// listening record), the rest from the report when the server has one.
fn all_time(ext: &ExtendedUserStats, report: Option<&ListeningReport>, t: &dyn Fn(&str) -> String) -> Html {
    let today = today_in(ext.timezone.as_deref());
    let has_activity = ext.listening_by_hour.iter().any(|v| *v > 0);

    let finished = ext.episodes_completed.map(|n| {
        html! {
            <Tile icon="ph-check-circle" value={group(n)} label={t("episodes_finished")}
                  sub={Some(t("completion_of").replace("{rate}", &ext.completion_rate.to_string()))} />
        }
    });
    let peak_note = ext.peak_hour.map(|h| t("peak_hour").replace("{hour}", &hour_label(h)));
    let months: &[MonthStat] = &ext.listening_by_month;
    let month_values: Vec<i64> = months.iter().map(|m| m.seconds).collect();
    let month_labels: Vec<String> = months.iter().map(|m| month_label(&m.month)).collect();

    let podcasts: Vec<Ranked> = match report.filter(|r| !r.top_podcasts.is_empty()) {
        Some(r) => top_podcast_rows(r, t),
        None => ext
            .top_podcasts
            .iter()
            .map(|p| Ranked { name: p.podcastname.clone(), sub: None, image: Some(p.artworkurl.clone()), person: false, seconds: p.total_seconds })
            .collect(),
    };
    let cats: Vec<(String, i64)> = match report.filter(|r| !r.top_categories.is_empty()) {
        Some(r) => category_rows(&r.top_categories),
        None => ext.favorite_categories.iter().map(|c| (c.name.clone(), c.total_seconds)).collect(),
    };

    html! {
        <>
            <div class="us-tiles">
                <Tile icon="ph-calendar-check" value={fmt_total(ext.year_seconds)}
                      label={t("this_year").replace("{year}", &today.year().to_string())} />
                { finished.unwrap_or_default() }
                <Tile icon="ph-flame" value={ext.current_streak.to_string()} label={t("current_streak")}
                      sub={(ext.longest_streak > 0).then(|| t("longest_streak").replace("{days}", &ext.longest_streak.to_string()))} />
                <Tile icon="ph-calendar-dots" value={group(ext.active_days)} label={t("active_days")} />
                if let Some(played) = ext.episodes_listened {
                    <Tile icon="ph-headphones" value={group(played)} label={t("episodes_played")} />
                }
                <Tile icon="ph-hourglass-medium" value={fmt_total(ext.backlog_seconds)} label={t("backlog")}
                      sub={Some(t("backlog_episodes").replace("{count}", &group(ext.backlog_episodes)))} />
                { saved_tile(report.map(|r| &r.time_saved).or(ext.time_saved.as_ref()), t) }
            </div>

            if has_activity {
                <Panel title={t("activity")} note={Some(t("activity_note"))} class="us-span-2">
                    { heatmap(&ext.daily_listening, today - Duration::days(HEATMAP_DAYS - 1), today, t("less"), t("more")) }
                </Panel>
                <div class="us-two">
                    <Panel title={t("by_hour")} note={peak_note}>
                        { bar_chart(&ext.listening_by_hour, &hour_labels(), ext.peak_hour.map(|h| h as usize), "us-bars-hours") }
                    </Panel>
                    <Panel title={t("listening_by_day")}>
                        { bar_chart(&ext.listening_by_dow, &dow_labels(t), peak(&ext.listening_by_dow), "") }
                    </Panel>
                </div>
                if !months.is_empty() {
                    <Panel title={t("by_month")}>
                        { bar_chart(&month_values, &month_labels, peak(&month_values), "") }
                    </Panel>
                }
            }

            <div class="us-two">
                if !podcasts.is_empty() {
                    <Panel title={t("top_podcasts_section")}>{ ranked(podcasts) }</Panel>
                }
                if !cats.is_empty() {
                    <Panel title={t("favorite_categories_section")}>{ shares(&cats) }</Panel>
                }
            </div>

            if let Some(ep) = &ext.longest_episode {
                <div class="us-longest">
                    <i class="ph ph-trophy"></i>
                    <div>
                        <span class="us-longest-label">{ t("longest_episode") }</span>
                        <span class="us-longest-title">{ &ep.episodetitle }</span>
                        <span class="us-longest-sub">{ format!("{} · {}", ep.podcastname, format_time(ep.episodeduration)) }</span>
                    </div>
                </div>
            }
        </>
    }
}

/// A year or a month, all from the report.
fn period_view(r: &ListeningReport, period: Period, t: &dyn Fn(&str) -> String) -> Html {
    let today = today_in(Some(&r.timezone));
    let Some((first, last)) = period.range() else { return Html::default() };
    let last = last.min(today);
    let before_record = r.recorded_since.as_deref().and_then(parse_day).is_some_and(|since| since > first);
    let month_values: Vec<i64> = r.by_month.iter().map(|m| m.seconds).collect();
    let month_labels: Vec<String> = r.by_month.iter().map(|m| month_label(&m.month)).collect();
    let hour_peak = peak(&r.by_hour);
    let cats = category_rows(&r.top_categories);
    let podcasts = top_podcast_rows(r, t);

    if r.seconds <= 0 {
        return html! { <p class="us-empty">{ t("period_empty") }</p> };
    }
    html! {
        <>
            if before_record {
                if let Some(since) = r.recorded_since.as_deref() {
                    <p class="us-note"><i class="ph ph-info"></i>{ t("recorded_since").replace("{date}", &fmt_day(since)) }</p>
                }
            }
            <div class="us-tiles">
                <Tile icon="ph-headphones" value={fmt_total(r.seconds)} label={t("listened")} />
                <Tile icon="ph-calendar-dots" value={group(r.active_days)} label={t("active_days")}
                      sub={(r.longest_streak > 1).then(|| t("longest_streak").replace("{days}", &r.longest_streak.to_string()))} />
                <Tile icon="ph-play-circle" value={group(r.episodes)} label={t("episodes_played")}
                      sub={Some(t("podcasts_count").replace("{count}", &group(r.podcasts)))} />
                <Tile icon="ph-check-circle" value={group(r.finished)} label={t("episodes_finished")} />
                { saved_tile(Some(&r.time_saved), t) }
            </div>
            if last >= first {
                <Panel title={t("activity")} class="us-span-2">
                    if matches!(period, Period::Month(..)) {
                        { month_calendar(&r.daily, first, last, &dow_labels(t)) }
                    } else {
                        { heatmap(&r.daily, first, last, t("less"), t("more")) }
                    }
                </Panel>
            }
            <div class="us-two">
                <Panel title={t("by_hour")} note={hour_peak.map(|h| t("peak_hour").replace("{hour}", &hour_label(h as i32)))}>
                    { bar_chart(&r.by_hour, &hour_labels(), hour_peak, "us-bars-hours") }
                </Panel>
                <Panel title={t("listening_by_day")}>
                    { bar_chart(&r.by_dow, &dow_labels(t), peak(&r.by_dow), "") }
                </Panel>
            </div>
            if !r.by_month.is_empty() {
                <Panel title={t("by_month_of").replace("{year}", &first.year().to_string())}>
                    { bar_chart(&month_values, &month_labels, peak(&month_values), "") }
                </Panel>
            }
            <div class="us-two">
                if !podcasts.is_empty() {
                    <Panel title={t("top_podcasts_section")}>{ ranked(podcasts) }</Panel>
                }
                if !cats.is_empty() {
                    <Panel title={t("favorite_categories_section")}>{ shares(&cats) }</Panel>
                }
            </div>
        </>
    }
}

fn fact(icon: &'static str, label: String, value: String, sub: Option<String>) -> Html {
    html! {
        <div class="us-fact">
            <i class={classes!("ph", icon)}></i>
            <div>
                <span class="us-fact-label">{ label }</span>
                <span class="us-fact-value">{ value }</span>
                if let Some(sub) = sub {
                    <span class="us-fact-sub">{ sub }</span>
                }
            </div>
        </div>
    }
}

fn client_name(name: &str, t: &dyn Fn(&str) -> String) -> String {
    match name {
        "web" => t("client_web"),
        "desktop" => t("client_desktop"),
        "android" => "Android".into(),
        "ios" => "iOS".into(),
        "sync" => t("client_sync"),
        _ => t("client_other"),
    }
}

/// Sections only the report has, for any period: highlights, episodes, people, apps, ads.
fn report_sections(r: &ListeningReport, t: &dyn Fn(&str) -> String) -> Html {
    if r.seconds <= 0 {
        return Html::default();
    }
    let mut facts = Vec::new();
    if let Some(d) = &r.biggest_day {
        facts.push(fact("ph-trophy", t("biggest_day"), fmt_secs(d.seconds), Some(fmt_day(&d.date))));
    }
    if let Some(w) = &r.busiest_week {
        facts.push(fact("ph-calendar-star", t("busiest_week"), fmt_secs(w.seconds), Some(t("week_of").replace("{date}", &fmt_day(&w.date)))));
    }
    if r.sessions.count > 0 {
        let when = r
            .sessions
            .longest_started_at
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .map(|d| d.with_timezone(&r.timezone.parse::<chrono_tz::Tz>().unwrap_or(chrono_tz::UTC)).format("%b %-d, %Y").to_string());
        facts.push(fact("ph-timer", t("longest_session"), fmt_secs(r.sessions.longest_seconds), when));
        facts.push(fact(
            "ph-repeat",
            t("sessions"),
            group(r.sessions.count),
            Some(t("average_session").replace("{time}", &fmt_secs(r.sessions.average_seconds))),
        ));
    }
    if let Some(speed) = r.average_speed {
        facts.push(fact("ph-gauge", t("average_speed"), format!("{speed:.2}×"), None));
    }
    if r.downloaded_seconds > 0 {
        let pct = (r.downloaded_seconds as f64 / r.seconds.max(1) as f64 * 100.0).round();
        facts.push(fact("ph-download-simple", t("played_offline"), fmt_secs(r.downloaded_seconds), Some(t("of_listening").replace("{pct}", &format!("{pct}")))));
    }
    if let Some(lag) = &r.release_lag {
        let value = if lag.median_days < 1.0 { t("same_day") } else { t("days_after").replace("{days}", &format!("{:.0}", lag.median_days.round())) };
        let sub = t("within_a_day").replace("{pct}", &format!("{}", (lag.within_day * 100.0).round()));
        facts.push(fact("ph-rocket-launch", t("release_lag"), value, Some(sub)));
    }

    let episodes: Vec<Ranked> = r
        .top_episodes
        .iter()
        .map(|e| Ranked { name: e.title.clone(), sub: Some(e.podcast.clone()), image: Some(e.artwork.clone()), person: false, seconds: e.seconds })
        .collect();
    let people: Vec<Ranked> = r
        .top_people
        .iter()
        .map(|p| Ranked {
            name: p.name.clone(),
            sub: Some(episodes_label(p.episodes, t)),
            image: p.image.clone(),
            person: true,
            seconds: p.seconds,
        })
        .collect();
    let clients: Vec<(String, i64)> = r.by_client.iter().map(|c| (client_name(&c.name, t), c.seconds)).collect();
    let ads: Vec<Ranked> = r
        .ad_heavy
        .iter()
        .map(|p| Ranked { name: p.name.clone(), sub: None, image: Some(p.artwork.clone()), person: false, seconds: p.seconds })
        .collect();

    html! {
        <>
            if !facts.is_empty() {
                <Panel title={t("highlights")}>
                    <div class="us-facts">{ for facts }</div>
                </Panel>
            }
            <div class="us-two">
                if !episodes.is_empty() {
                    <Panel title={t("top_episodes")}>{ ranked(episodes) }</Panel>
                }
                if !people.is_empty() {
                    <Panel title={t("top_people")} note={Some(t("top_people_note"))}>{ ranked(people) }</Panel>
                }
            </div>
            <div class="us-two">
                if clients.len() > 1 || clients.first().is_some_and(|(n, _)| n != &t("client_other")) {
                    <Panel title={t("by_app")}>{ shares(&clients) }</Panel>
                }
                if !ads.is_empty() {
                    <Panel title={t("most_ads")} note={Some(t("most_ads_note"))}>{ ranked(ads) }</Panel>
                }
            </div>
        </>
    }
}

/// All time, then each year with listening, and that year's months.
fn period_picker(months: &[String], period: Period, set: Callback<Period>, t: &dyn Fn(&str) -> String) -> Html {
    let parsed: Vec<(i32, u32)> = months
        .iter()
        .filter_map(|m| {
            let (y, mo) = m.split_once('-')?;
            Some((y.parse().ok()?, mo.parse().ok()?))
        })
        .collect();
    let mut years: Vec<i32> = parsed.iter().map(|(y, _)| *y).collect();
    years.sort_unstable_by(|a, b| b.cmp(a));
    years.dedup();
    let year = match period {
        Period::Year(y) | Period::Month(y, _) => Some(y),
        Period::All => None,
    };
    let chip = |label: String, value: Period, active: bool| {
        let set = set.clone();
        html! {
            <button type="button" class={classes!("us-chip", active.then_some("is-active"))} aria-pressed={active.to_string()}
                    onclick={Callback::from(move |_| set.emit(value))}>{ label }</button>
        }
    };
    let mut year_months: Vec<u32> = year.map(|y| parsed.iter().filter(|(py, _)| *py == y).map(|(_, m)| *m).collect()).unwrap_or_default();
    year_months.sort_unstable();
    html! {
        <nav class="us-periods" aria-label={t("period")}>
            <div class="us-chips">
                { chip(t("period_all"), Period::All, period == Period::All) }
                { for years.iter().map(|y| chip(y.to_string(), Period::Year(*y), year == Some(*y))) }
            </div>
            if let Some(y) = year {
                <div class="us-chips us-chips-sub">
                    { chip(t("period_whole_year"), Period::Year(y), period == Period::Year(y)) }
                    { for year_months.iter().map(|m| chip(month_label(&format!("{y}-{m:02}")), Period::Month(y, *m), period == Period::Month(y, *m))) }
                </div>
            }
        </nav>
    }
}

#[function_component(UserStats)]
pub fn user_stats() -> Html {
    let (i18n, _) = use_translation();
    let (stat_state, stat_dispatch) = use_store::<UserStatsStore>();
    let (post_state, _) = use_store::<AppState>();
    let (audio_state, _) = use_store::<UIState>();
    let gravatar = use_selector(|s: &UserPreferencesState| s.gravatar_url.clone());
    let period = use_state(|| Period::All);
    // The report for `period`, and the months with listening (kept while another loads).
    let report = use_state(|| None::<(Period, ListeningReport)>);
    let months = use_state(Vec::<String>::new);

    {
        let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID);
        let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        use_effect_with((api_key, user_id, server_name), move |(api_key, user_id, server_name)| {
            if let (Some(api_key), Some(user_id), Some(server_name)) = (api_key.clone(), *user_id, server_name.clone()) {
                {
                    let (server, key, dispatch) = (server_name.clone(), api_key.clone(), stat_dispatch.clone());
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(stats) = stat_reqs::call_get_stats(server, key, &user_id).await {
                            dispatch.reduce_mut(move |s| s.stats = Some(stats));
                        }
                    });
                }
                {
                    let (server, key, dispatch) = (server_name.clone(), api_key.clone(), stat_dispatch.clone());
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(ext) = stat_reqs::call_get_extended_stats(server, key, &user_id).await {
                            dispatch.reduce_mut(move |s| s.extended_stats = Some(ext));
                        }
                    });
                }
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(version) = call_get_pinepods_version(server_name, &api_key).await {
                        stat_dispatch.reduce_mut(move |s| s.pinepods_version = Some(version));
                    }
                });
            }
            || ()
        });
    }
    {
        let api_key = post_state.auth_details.as_ref().and_then(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID);
        let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        let (report, months) = (report.clone(), months.clone());
        use_effect_with((api_key, user_id, server_name, *period), move |(api_key, user_id, server_name, period)| {
            if let (Some(api_key), Some(user_id), Some(server_name)) = (api_key.clone(), *user_id, server_name.clone()) {
                let period = *period;
                let range = period.range().map(|(s, e)| (s.format("%Y-%m-%d").to_string(), e.format("%Y-%m-%d").to_string()));
                wasm_bindgen_futures::spawn_local(async move {
                    // An older server has no report: the page shows what the extended stats have.
                    if let Ok(r) = stat_reqs::call_get_listening_report(&server_name, &api_key, user_id, range).await {
                        months.set(r.months.clone());
                        report.set(Some((period, r)));
                    }
                });
            }
            || ()
        });
    }
    let set_period = {
        let period = period.clone();
        Callback::from(move |p: Period| period.set(p))
    };
    let shown = report.as_ref().filter(|(p, _)| *p == *period).map(|(_, r)| r);

    let t = |k: &str| i18n.t(&format!("user_stats.{k}")).to_string();
    let username = post_state
        .user_details
        .as_ref()
        .and_then(|u| u.Username.clone())
        .unwrap_or_default();
    let use_gravatar = post_state.user_details.as_ref().map_or(false, |u| u.UseGravatar);
    let avatar = (*gravatar)
        .clone()
        .filter(|_| use_gravatar)
        .unwrap_or_else(|| "/static/assets/favicon.png".to_string());

    let hero = match stat_state.stats.as_ref() {
        Some(stats) => {
            let badge = stat_state.extended_stats.as_ref().and_then(|ext| match ext.listening_badge.as_str() {
                "night_owl" => Some((t("badge_night_owl"), "ph-moon")),
                "evening_listener" => Some((t("badge_evening_listener"), "ph-sun-horizon")),
                "afternoon_listener" => Some((t("badge_afternoon_listener"), "ph-sun")),
                "morning_listener" => Some((t("badge_morning_listener"), "ph-sun-dim")),
                _ => None,
            });
            html! {
                <section class="us-hero">
                    <img class="us-hero-avatar" src={avatar} alt="" />
                    <div class="us-hero-body">
                        <h2 class="us-hero-name">{ &username }</h2>
                        <span class="us-hero-since">{ t("listening_since").replace("{date}", &format_date(&stats.UserCreated)) }</span>
                        if let Some((label, icon)) = badge {
                            <span class="us-badge"><i class={classes!("ph", icon)}></i>{ label }</span>
                        }
                    </div>
                    <div class="us-hero-total">
                        <span class="us-hero-total-value">{ fmt_total(stats.TimeListened as i64 * 60) }</span>
                        <span class="us-hero-total-label">{ t("time_listened") }</span>
                    </div>
                </section>
            }
        }
        None => html! { <p class="item_container-text">{ t("loading_user_stats") }</p> },
    };

    let library = stat_state.stats.as_ref().map(|stats| {
        let downloaded_size = stat_state
            .extended_stats
            .as_ref()
            .map(|e| e.total_downloaded_formatted.clone())
            .filter(|s| s != "0 B");
        // The PinePods server and an external one can both be on; older servers only send
        // Pod_Sync_Type and GpodderUrl.
        let sync_lines: Vec<(String, String)> = match stats.SyncServerEnabled {
            Some(server_on) => {
                let mut lines = Vec::new();
                if server_on {
                    let url = stats
                        .SyncServerUrl
                        .clone()
                        .or_else(|| post_state.auth_details.as_ref().map(|a| a.server_name.clone()))
                        .unwrap_or_default();
                    lines.push((t("internal_gpodder"), url));
                }
                match stats.SyncTargetKind.as_deref() {
                    Some("nextcloud") => lines.push((t("nextcloud"), stats.SyncTargetUrl.clone().unwrap_or_default())),
                    Some(_) => lines.push((t("external_gpodder"), stats.SyncTargetUrl.clone().unwrap_or_default())),
                    None => {}
                }
                lines
            }
            None => match stats.Pod_Sync_Type.as_str() {
                "None" => Vec::new(),
                "gpodder" if stats.GpodderUrl == "http://localhost:8042" => vec![(t("internal_gpodder"), String::new())],
                "gpodder" => vec![(t("external_gpodder"), stats.GpodderUrl.clone())],
                "nextcloud" => vec![(t("nextcloud"), stats.GpodderUrl.clone())],
                _ => vec![(t("unknown_sync_type"), stats.GpodderUrl.clone())],
            },
        };
        let sync_value = if sync_lines.is_empty() {
            t("not_syncing")
        } else {
            sync_lines.iter().map(|(l, _)| l.clone()).collect::<Vec<_>>().join(" + ")
        };
        let sync_detail = sync_lines
            .iter()
            .filter(|(_, u)| !u.is_empty())
            .map(|(_, u)| u.clone())
            .collect::<Vec<_>>()
            .join(" · ");

        html! {
            <Panel title={t("library")}>
                <div class="us-list">
                    <div class="us-list-row"><i class="ph ph-microphone-stage"></i><span>{ t("podcasts_added") }</span><strong>{ group(stats.PodcastsAdded as i64) }</strong></div>
                    <div class="us-list-row"><i class="ph ph-play-circle"></i><span>{ t("podcasts_played") }</span><strong>{ group(stats.PodcastsPlayed as i64) }</strong></div>
                    <div class="us-list-row"><i class="ph ph-bookmark-simple"></i><span>{ t("episodes_saved") }</span><strong>{ group(stats.EpisodesSaved as i64) }</strong></div>
                    <div class="us-list-row">
                        <i class="ph ph-download-simple"></i><span>{ t("episodes_downloaded") }</span>
                        <strong>{ group(stats.EpisodesDownloaded as i64) }{ downloaded_size.map(|s| format!(" · {s}")).unwrap_or_default() }</strong>
                    </div>
                    <div class="us-list-row">
                        <i class="ph ph-arrows-clockwise"></i><span>{ t("podcast_sync_status") }</span>
                        <strong title={sync_detail}>{ sync_value }</strong>
                    </div>
                </div>
            </Panel>
        }
    });

    let version = stat_state.pinepods_version.clone().unwrap_or_else(|| "test_mode".to_string());

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <div class="us-page">
                <h1 class="us-page-title">{ t("user_statistics") }</h1>
                { hero }
                if !months.is_empty() {
                    { period_picker(&months, *period, set_period, &t) }
                }
                if *period == Period::All {
                    if let Some(ext) = stat_state.extended_stats.as_ref() {
                        { all_time(ext, shown, &t) }
                    }
                } else {
                    <h2 class="us-period-title">{ period.title(&t) }</h2>
                    if let Some(r) = shown {
                        { period_view(r, *period, &t) }
                    } else {
                        <p class="us-empty">{ t("loading_user_stats") }</p>
                    }
                }
                if let Some(r) = shown {
                    { report_sections(r, &t) }
                }
                if *period == Period::All {
                    { library.unwrap_or_default() }
                }

                <section class="us-about">
                    <img src="static/assets/favicon.png" alt="Pinepods" class="us-about-logo" />
                    <div class="us-about-body">
                        <span class="us-about-version">{ format!("{}{}", t("current_version"), version) }</span>
                        <p>{ t("about_text") }</p>
                        <div class="us-about-links">
                            <a href="https://pinepods.online" target="_blank" rel="noopener noreferrer"><i class="ph ph-book-open"></i>{ t("pinepods_documentation") }</a>
                            <a href="https://github.com/madeofpendletonwool/pinepods" target="_blank" rel="noopener noreferrer"><i class="ph ph-github-logo"></i>{ t("pinepods_github_repo") }</a>
                            <a href="https://www.buymeacoffee.com/collinscoffee" target="_blank" rel="noopener noreferrer"><i class="ph ph-coffee"></i>{ t("buy_me_coffee") }</a>
                        </div>
                    </div>
                </section>
                <div class="h-30"></div>
            </div>

            if let Some(audio_props) = &audio_state.currently_playing {
                <AudioPlayer
                    episode={audio_props.episode.clone()}
                    src={audio_props.src.clone()}
                    title={audio_props.title.clone()}
                    description={audio_props.description.clone()}
                    release_date={audio_props.release_date.clone()}
                    artwork_url={audio_props.artwork_url.clone()}
                    duration={audio_props.duration.clone()}
                    episode_id={audio_props.episode_id.clone()}
                    duration_sec={audio_props.duration_sec.clone()}
                    start_pos_sec={audio_props.start_pos_sec.clone()}
                    end_pos_sec={audio_props.end_pos_sec.clone()}
                    offline={audio_props.offline.clone()}
                    is_youtube={audio_props.is_youtube.clone()}
                    is_video={audio_props.is_video.clone()}
                />
            }
        </div>
        <App_drawer />
        </>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn formats() {
        assert_eq!(group(1234567), "1,234,567");
        assert_eq!(group(999), "999");
        assert_eq!(fmt_secs(3 * 3600 + 12 * 60), "3h 12m");
        assert_eq!(fmt_total(250 * 3600), "250h");
        assert_eq!(hour_label(0), "12 AM");
        assert_eq!(hour_label(17), "5 PM");
        assert_eq!(month_label("2026-11"), "Nov");
    }

    #[wasm_bindgen_test]
    fn period_ranges() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        assert_eq!(Period::All.range(), None);
        assert_eq!(Period::Year(2026).range(), Some((d(2026, 1, 1), d(2026, 12, 31))));
        assert_eq!(Period::Month(2028, 2).range(), Some((d(2028, 2, 1), d(2028, 2, 29))));
        assert_eq!(Period::Month(2026, 12).range(), Some((d(2026, 12, 1), d(2026, 12, 31))));
    }
}
