//! Cards for the Smart Playlists page: an artwork mosaic, the playlist's rules in plain
//! words, its totals and the next few episodes, each playable in place.

use crate::components::art_accent::{accent_style, use_art_accent};
use crate::components::audio::on_play_click;
use crate::components::context::{AppState, UIState};
use crate::components::gen_components::FallbackImage;
use crate::components::i18n::use_translation;
use crate::requests::episode::Episode;
use crate::requests::pod_req::{Playlist, PlaylistFeedPage};
use yew::prelude::*;
use yewdux::prelude::*;

/// How many upcoming episodes a card lists.
pub const PREVIEW_LEN: i64 = 3;

/// "6h 12m", "42m" or "<1m".
pub fn human_duration(secs: i64) -> String {
    let mins = (secs + 30) / 60;
    match (mins / 60, mins % 60) {
        (0, 0) => "<1m".to_string(),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

fn minutes_label(mins: i32) -> String {
    if mins >= 60 && mins % 60 == 0 {
        format!("{} h", mins / 60)
    } else {
        format!("{mins} min")
    }
}

/// The playlist's rules as short chips: (icon, text).
fn rule_chips(playlist: &Playlist, page: Option<&PlaylistFeedPage>, i18n: &i18nrs::I18n) -> Vec<(&'static str, String)> {
    let info = page.map(|p| &p.playlist_info);
    let mut chips = Vec::new();

    let states = [
        (playlist.include_unplayed, "playlists.rule_unplayed"),
        (playlist.include_partially_played, "playlists.rule_in_progress"),
        (playlist.include_played, "playlists.rule_played"),
    ];
    let picked: Vec<String> = states.iter().filter(|(on, _)| *on).map(|(_, k)| i18n.t(k)).collect();
    if picked.len() < 3 {
        chips.push(("ph-circle-half", picked.join(" + ")));
    }

    // The overview list sends durations in seconds, a playlist page in minutes.
    let (min, max) = match info {
        Some(i) => (i.min_duration, i.max_duration),
        None => (playlist.min_duration.map(|s| s / 60), playlist.max_duration.map(|s| s / 60)),
    };
    match (min.filter(|m| *m > 0), max.filter(|m| *m > 0)) {
        (Some(a), Some(b)) => chips.push(("ph-timer", format!("{}–{}", minutes_label(a), minutes_label(b)))),
        (Some(a), None) => chips.push(("ph-timer", format!("{} {}", i18n.t("playlists.rule_over"), minutes_label(a)))),
        (None, Some(b)) => chips.push(("ph-timer", format!("{} {}", i18n.t("playlists.rule_under"), minutes_label(b)))),
        _ => {}
    }

    if let Some(hours) = info.and_then(|i| i.time_filter_hours).filter(|h| *h > 0) {
        let text = if hours >= 48 && hours % 24 == 0 {
            format!("{} {} {}", i18n.t("playlists.rule_last"), hours / 24, i18n.t("playlists.rule_days"))
        } else {
            format!("{} {} {}", i18n.t("playlists.rule_last"), hours, i18n.t("playlists.rule_hours"))
        };
        chips.push(("ph-calendar-blank", text));
    }

    if let Some(i) = info {
        match (i.play_progress_min.filter(|p| *p > 0.0), i.play_progress_max.filter(|p| *p < 100.0)) {
            (Some(a), Some(b)) => chips.push(("ph-chart-pie-slice", format!("{a:.0}–{b:.0}% {}", i18n.t("playlists.rule_played_pct")))),
            (Some(a), None) => chips.push(("ph-chart-pie-slice", format!("{a:.0}%+ {}", i18n.t("playlists.rule_played_pct")))),
            (None, Some(b)) => chips.push(("ph-chart-pie-slice", format!("≤{b:.0}% {}", i18n.t("playlists.rule_played_pct")))),
            _ => {}
        }
    }

    if let Some(ids) = playlist.podcast_ids.as_ref().filter(|ids| !ids.is_empty()) {
        let key = if ids.len() == 1 { "playlists.rule_chosen_podcast" } else { "playlists.rule_chosen_podcasts" };
        chips.push(("ph-microphone", format!("{} {}", ids.len(), i18n.t(key))));
    }
    if !playlist.label_ids.is_empty() {
        let key = if playlist.label_ids.len() == 1 { "playlists.rule_label" } else { "playlists.rule_labels" };
        chips.push(("ph-tag", format!("{} {}", playlist.label_ids.len(), i18n.t(key))));
    }

    let sort = match playlist.sort_order.as_str() {
        "date_desc" => Some("playlists.newest_first"),
        "date_asc" => Some("playlists.oldest_first"),
        "duration_asc" => Some("playlists.shortest_first"),
        "duration_desc" => Some("playlists.longest_first"),
        "listen_progress" | "completion" => Some("playlists.rule_most_progress"),
        "random" => Some("playlists.rule_shuffled"),
        _ => None,
    };
    if let Some(key) = sort {
        chips.push(("ph-sort-descending", i18n.t(key)));
    }
    if playlist.group_by_podcast {
        chips.push(("ph-stack", i18n.t("playlists.group_by_podcast")));
    }
    if let Some(max) = playlist.max_episodes.filter(|m| *m > 0) {
        chips.push(("ph-hash", format!("{} {}", i18n.t("playlists.rule_up_to"), max)));
    }
    chips
}

fn art_of(ep: &Episode) -> String {
    if ep.episodeartwork.is_empty() {
        ep.artworkurl.clone()
    } else {
        ep.episodeartwork.clone()
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct PlaylistOverviewCardProps {
    pub playlist: Playlist,
    /// The first page of the playlist, once loaded.
    pub page: Option<PlaylistFeedPage>,
    pub on_open: Callback<MouseEvent>,
    pub on_delete: Callback<MouseEvent>,
    pub selectable: bool,
    pub selected: bool,
}

#[function_component(PlaylistOverviewCard)]
pub fn playlist_overview_card(props: &PlaylistOverviewCardProps) -> Html {
    let (i18n, _) = use_translation();
    let playlist = &props.playlist;
    let episodes: Vec<Episode> = props.page.as_ref().map(|p| p.episodes.clone()).unwrap_or_default();
    let first_art = episodes.first().map(art_of).unwrap_or_default();
    let accent = use_art_accent(&first_art);

    // Up to four different covers for the mosaic.
    let mut covers: Vec<String> = Vec::new();
    for ep in &episodes {
        let art = art_of(ep);
        if !art.is_empty() && !covers.contains(&art) {
            covers.push(art);
        }
    }
    let count = props
        .page
        .as_ref()
        .map(|p| p.total)
        .unwrap_or(playlist.episode_count.unwrap_or(0) as i64);
    let stats = props.page.as_ref().and_then(|p| p.stats.clone());
    let chips = rule_chips(playlist, props.page.as_ref(), &i18n);
    let icon = playlist.icon_name.clone().filter(|i| !i.is_empty()).unwrap_or_else(|| "ph-playlist".to_string());
    let loading = props.page.is_none() && count > 0;

    let play_target = episodes.iter().find(|e| !e.completed).or(episodes.first()).cloned();

    html! {
        <article
            class={classes!(
                "spl-card",
                props.selected.then_some("is-selected"),
                props.selectable.then_some("is-selectable"),
                (count == 0).then_some("is-empty"),
            )}
            style={accent_style(&accent)}
            onclick={props.on_open.clone()}
        >
            <div class="spl-head">
                <div class={classes!("spl-mosaic", format!("n{}", covers.len().min(4)))}>
                    { for covers.iter().take(4).map(|src| html! {
                        <FallbackImage size={Some(128)} src={src.clone()} alt="" class="spl-mosaic-img" />
                    }) }
                    <span class="spl-icon"><i class={classes!("ph", icon)}></i></span>
                </div>
                <div class="spl-title-block">
                    <h3 class="spl-name">
                        { &playlist.name }
                        if playlist.is_system_playlist {
                            <span class="spl-badge">{ i18n.t("playlists.built_in") }</span>
                        }
                    </h3>
                    if let Some(desc) = playlist.description.as_ref().filter(|d| !d.is_empty()) {
                        <p class="spl-desc">{ desc }</p>
                    }
                    <div class="spl-stats">
                        <span class="spl-stat-strong">
                            { format!("{} {}", count, if count == 1 { i18n.t("playlists.episode_one") } else { i18n.t("playlists.episodes") }) }
                        </span>
                        if let Some(s) = stats.as_ref().filter(|_| count > 0) {
                            <span><i class="ph ph-hourglass-medium"></i>{ format!("{} {}", human_duration(s.remaining_seconds), i18n.t("playlists.left")) }</span>
                            <span><i class="ph ph-microphone"></i>{ format!("{} {}", s.podcast_count, if s.podcast_count == 1 { i18n.t("playlists.podcast_one") } else { i18n.t("playlists.podcasts_lower") }) }</span>
                            if s.in_progress > 0 {
                                <span><i class="ph ph-play-circle"></i>{ format!("{} {}", s.in_progress, i18n.t("playlists.started")) }</span>
                            }
                        }
                    </div>
                </div>
                if props.selectable {
                    <span class={classes!("spl-check", props.selected.then_some("is-on"))} aria-hidden="true">
                        <i class="ph ph-check"></i>
                    </span>
                } else {
                    <div class="spl-actions">
                        if let Some(ep) = play_target {
                            <PlayPlaylistButton episode={ep} playlist_id={playlist.playlist_id} />
                        }
                        <button
                            class="spl-icon-btn"
                            onclick={props.on_delete.clone()}
                            title={i18n.t("playlists.delete_playlist")}
                            aria-label={i18n.t("playlists.delete_playlist")}
                        >
                            <i class="ph ph-trash"></i>
                        </button>
                    </div>
                }
            </div>

            if !chips.is_empty() {
                <ul class="spl-rules">
                    { for chips.into_iter().map(|(icon, text)| html! {
                        <li><i class={classes!("ph", icon)}></i>{ text }</li>
                    }) }
                </ul>
            }

            <div class="spl-next">
                if loading {
                    { for (0..PREVIEW_LEN).map(|_| html! { <div class="spl-row is-skeleton"><span></span><span></span></div> }) }
                } else if episodes.is_empty() {
                    <p class="spl-none"><i class="ph ph-leaf"></i>{ i18n.t("playlists.nothing_matches") }</p>
                } else {
                    <p class="spl-next-label">{ i18n.t("playlists.up_next") }</p>
                    { for episodes.iter().map(|ep| html! {
                        <UpNextRow episode={ep.clone()} playlist_id={playlist.playlist_id} />
                    }) }
                    if count > episodes.len() as i64 {
                        <p class="spl-more">{ format!("+{} {}", count - episodes.len() as i64, i18n.t("playlists.more")) }</p>
                    }
                }
            </div>
        </article>
    }
}

/// Plays an episode as part of a playlist, or toggles it if it is already playing.
#[hook]
fn use_playlist_play(episode: &Episode, playlist_id: i32) -> (Callback<MouseEvent>, bool, bool) {
    let episode_id = episode.episodeid;
    let state = use_selector(move |s: &UIState| {
        let current = s.currently_playing.as_ref().map_or(false, |cp| cp.episode_id == episode_id);
        (current && s.audio_playing.unwrap_or(false), s.loading_episode_id == Some(episode_id))
    });
    let auth = use_selector(|s: &AppState| {
        (
            s.auth_details.as_ref().map(|a| a.server_name.clone()).unwrap_or_default(),
            s.auth_details.as_ref().and_then(|a| a.api_key.clone()).unwrap_or_default(),
            s.user_details.as_ref().map(|u| u.UserID).unwrap_or_default(),
        )
    });
    let on_click = {
        let episode = episode.clone();
        let (server_name, api_key, user_id) = (*auth).clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            let dispatch = Dispatch::<UIState>::global();
            let current = dispatch.get();
            let is_current = current
                .currently_playing
                .as_ref()
                .map_or(false, |cp| cp.episode_id == episode.episodeid);
            if is_current {
                dispatch.reduce_mut(|s| s.toggle_playback());
            } else {
                let id = episode.episodeid;
                dispatch.reduce_mut(move |s| s.loading_episode_id = Some(id));
                on_play_click(
                    episode.clone(),
                    api_key.clone(),
                    user_id,
                    server_name.clone(),
                    dispatch.clone(),
                    current.clone(),
                    false,
                    false,
                    Some(playlist_id),
                )
                .emit(e);
            }
        })
    };
    let (playing, loading) = *state;
    (on_click, playing, loading)
}

fn play_glyph(playing: bool, loading: bool) -> Html {
    if loading {
        html! { <i class="ph ph-spinner spl-spin"></i> }
    } else if playing {
        html! { <i class="ph ph-pause"></i> }
    } else {
        html! { <i class="ph ph-play"></i> }
    }
}

#[derive(Properties, PartialEq, Clone)]
struct PlayEpisodeProps {
    episode: Episode,
    playlist_id: i32,
}

#[function_component(PlayPlaylistButton)]
fn play_playlist_button(props: &PlayEpisodeProps) -> Html {
    let (i18n, _) = use_translation();
    let (on_click, playing, loading) = use_playlist_play(&props.episode, props.playlist_id);
    let label = if playing { i18n.t("episode_list_item.pause") } else { i18n.t("playlists.play_playlist") };
    html! {
        <button class={classes!("spl-play", (playing || loading).then_some("is-on"))} onclick={on_click} title={label.clone()} aria-label={label}>
            { play_glyph(playing, loading) }
        </button>
    }
}

#[function_component(UpNextRow)]
fn up_next_row(props: &PlayEpisodeProps) -> Html {
    let ep = &props.episode;
    let (on_click, playing, loading) = use_playlist_play(ep, props.playlist_id);
    let pct = if ep.episodeduration > 0 && ep.listenduration > 0 && !ep.completed {
        ((ep.listenduration as i64 * 100) / ep.episodeduration as i64).clamp(2, 100)
    } else {
        0
    };
    let left = if pct > 0 {
        (ep.episodeduration - ep.listenduration).max(0) as i64
    } else {
        ep.episodeduration as i64
    };
    html! {
        <button class={classes!("spl-row", (playing || loading).then_some("is-on"))} onclick={on_click} title={ep.episodetitle.clone()}>
            <span class="spl-row-art">
                <FallbackImage size={Some(96)} src={art_of(ep)} alt="" class="spl-row-img" />
                <span class="spl-row-glyph">{ play_glyph(playing, loading) }</span>
            </span>
            <span class="spl-row-text">
                <span class="spl-row-title">{ &ep.episodetitle }</span>
                <span class="spl-row-sub">{ &ep.podcastname }</span>
            </span>
            <span class="spl-row-time">
                { human_duration(left) }
                if pct > 0 {
                    <span class="spl-row-bar"><span style={format!("width:{pct}%")}></span></span>
                }
            </span>
        </button>
    }
}
