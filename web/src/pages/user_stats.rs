use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState, UserPreferencesState, UserStatsStore};
use crate::components::gen_components::{FallbackImage, Search_nav};
use crate::components::gen_funcs::{format_date, format_time};
use crate::components::i18n::use_translation;
use crate::requests::pod_req::call_get_pinepods_version;
use crate::requests::stat_reqs::{self, CategoryStat, DayStat, ExtendedUserStats, MonthStat, TopPodcastStat};
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

/// GitHub-style grid: one column per week, Sunday at the top.
fn heatmap(days: &[DayStat], today: NaiveDate, less: String, more: String) -> Html {
    let by_date: std::collections::HashMap<NaiveDate, i64> = days
        .iter()
        .filter_map(|d| NaiveDate::parse_from_str(&d.date, "%Y-%m-%d").ok().map(|n| (n, d.seconds)))
        .collect();
    let first = today - Duration::days(HEATMAP_DAYS - 1);
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

fn top_podcasts(podcasts: &[TopPodcastStat]) -> Html {
    let max = podcasts.iter().map(|p| p.total_seconds).max().unwrap_or(0).max(1);
    html! {
        <ol class="us-top">
            { for podcasts.iter().enumerate().map(|(i, p)| {
                let pct = (p.total_seconds as f64 / max as f64 * 100.0).max(2.0);
                html! {
                    <li class="us-top-row">
                        <span class="us-top-rank">{ i + 1 }</span>
                        <FallbackImage size={Some(128)} src={p.artworkurl.clone()} alt={p.podcastname.clone()} class="us-top-art" />
                        <div class="us-top-body">
                            <div class="us-top-line">
                                <span class="us-top-name">{ &p.podcastname }</span>
                                <span class="us-top-time">{ fmt_secs(p.total_seconds) }</span>
                            </div>
                            <div class="us-meter"><div style={format!("width: {pct:.1}%;")}></div></div>
                        </div>
                    </li>
                }
            })}
        </ol>
    }
}

fn categories(cats: &[CategoryStat]) -> Html {
    let total: i64 = cats.iter().map(|c| c.total_seconds).sum::<i64>().max(1);
    html! {
        <div class="us-cats">
            { for cats.iter().map(|c| {
                let pct = (c.total_seconds as f64 / total as f64 * 100.0).round() as i64;
                html! {
                    <div class="us-cat">
                        <div class="us-top-line">
                            <span class="us-top-name">{ &c.name }</span>
                            <span class="us-top-time">{ format!("{pct}%") }</span>
                        </div>
                        <div class="us-meter"><div style={format!("width: {pct}%;")}></div></div>
                    </div>
                }
            })}
        </div>
    }
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

fn insights(ext: &ExtendedUserStats, i18n: &crate::components::i18n::Translations) -> Html {
    let t = |k: &str| i18n.t(&format!("user_stats.{k}")).to_string();
    let today = today_in(ext.timezone.as_deref());
    let has_activity = ext.listening_by_hour.iter().any(|v| *v > 0);

    let finished = ext.episodes_completed.map(|n| {
        html! {
            <Tile icon="ph-check-circle" value={group(n)} label={t("episodes_finished")}
                  sub={Some(t("completion_of").replace("{rate}", &ext.completion_rate.to_string()))} />
        }
    });

    let hour_labels: Vec<String> = (0..24)
        .map(|h| if h % 6 == 0 { hour_label(h) } else { String::new() })
        .collect();
    let peak_note = ext.peak_hour.map(|h| t("peak_hour").replace("{hour}", &hour_label(h)));
    let dow_labels: Vec<String> = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"]
        .iter()
        .map(|d| t(&format!("dow_{d}")))
        .collect();
    let dow_peak = ext
        .listening_by_dow
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 0)
        .max_by_key(|(_, v)| **v)
        .map(|(i, _)| i);
    let months: &[MonthStat] = &ext.listening_by_month;
    let month_values: Vec<i64> = months.iter().map(|m| m.seconds).collect();
    let month_labels: Vec<String> = months.iter().map(|m| month_label(&m.month)).collect();
    let month_peak = month_values
        .iter()
        .enumerate()
        .filter(|(_, v)| **v > 0)
        .max_by_key(|(_, v)| **v)
        .map(|(i, _)| i);

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
            </div>

            if has_activity {
                <Panel title={t("activity")} note={Some(t("activity_note"))} class="us-span-2">
                    { heatmap(&ext.daily_listening, today, t("less"), t("more")) }
                </Panel>
                <div class="us-two">
                    <Panel title={t("by_hour")} note={peak_note}>
                        { bar_chart(&ext.listening_by_hour, &hour_labels, ext.peak_hour.map(|h| h as usize), "us-bars-hours") }
                    </Panel>
                    <Panel title={t("listening_by_day")}>
                        { bar_chart(&ext.listening_by_dow, &dow_labels, dow_peak, "") }
                    </Panel>
                </div>
                if !months.is_empty() {
                    <Panel title={t("by_month")}>
                        { bar_chart(&month_values, &month_labels, month_peak, "") }
                    </Panel>
                }
            }

            <div class="us-two">
                if !ext.top_podcasts.is_empty() {
                    <Panel title={t("top_podcasts_section")}>{ top_podcasts(&ext.top_podcasts) }</Panel>
                }
                if !ext.favorite_categories.is_empty() {
                    <Panel title={t("favorite_categories_section")}>{ categories(&ext.favorite_categories) }</Panel>
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

#[function_component(UserStats)]
pub fn user_stats() -> Html {
    let (i18n, _) = use_translation();
    let (stat_state, stat_dispatch) = use_store::<UserStatsStore>();
    let (post_state, _) = use_store::<AppState>();
    let (audio_state, _) = use_store::<UIState>();
    let gravatar = use_selector(|s: &UserPreferencesState| s.gravatar_url.clone());

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
                if let Some(ext) = stat_state.extended_stats.as_ref() {
                    { insights(ext, &i18n) }
                }
                { library.unwrap_or_default() }

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
}
