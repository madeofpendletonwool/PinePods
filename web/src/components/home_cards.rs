//! Cards for the home page: the Continue Listening hero, episode tiles for shelves, and
//! the shelf wrapper itself.

use crate::components::art_accent::{accent_style, css_url_escape, use_art_accent};
use crate::components::audio::on_play_click;
use crate::components::click_events::open_podcast_by_id;
use crate::components::context::{AppState, EpisodeStatusState, PodcastFeedState, UIState};
use crate::components::gen_components::{on_shownotes_click, proxied_image_url, FallbackImage};
use crate::components::gen_funcs::format_time;
use crate::components::i18n::use_translation;
use crate::pages::routes::Route;
use crate::requests::episode::Episode;
use yew::prelude::*;
use yew_router::history::BrowserHistory;
use yew_router::prelude::Link;
use yewdux::prelude::*;

/// What a home card needs to play, open and link an episode.
struct EpisodeControls {
    on_play_pause: Callback<MouseEvent>,
    on_open: Callback<MouseEvent>,
    on_podcast: Callback<MouseEvent>,
    href: Option<String>,
    is_playing: bool,
    is_loading: bool,
    is_completed: bool,
    art_src: String,
    accent: Option<String>,
    server_name: String,
}

#[hook]
fn use_episode_controls(episode: &Episode) -> EpisodeControls {
    let episode_id = episode.episodeid;
    let status = use_selector(move |s: &EpisodeStatusState| {
        (
            s.completed_episodes.contains(&episode_id),
            s.downloaded_episodes.is_server_download(episode_id),
            s.downloaded_episodes.is_local_download(episode_id),
        )
    });
    let (is_completed, downloaded_server, downloaded_local) = *status;
    let play_state = use_selector(move |s: &UIState| {
        let current = s.currently_playing.as_ref().map_or(false, |cp| cp.episode_id == episode_id);
        (current && s.audio_playing.unwrap_or(false), s.loading_episode_id == Some(episode_id))
    });
    let (is_playing, is_loading) = *play_state;
    let podcast_added = *use_selector(|s: &PodcastFeedState| s.podcast_added.unwrap_or(false));
    let auth = use_selector(|s: &AppState| {
        (
            s.auth_details.as_ref().map(|a| a.server_name.clone()).unwrap_or_default(),
            s.auth_details.as_ref().and_then(|a| a.api_key.clone()).unwrap_or_default(),
            s.user_details.as_ref().map(|u| u.UserID).unwrap_or_default(),
        )
    });
    let (server_name, api_key, user_id) = (*auth).clone();

    let is_local = podcast_added && episode_id != 0 && (downloaded_server || downloaded_local);

    let on_play_pause = {
        let episode = episode.clone();
        let (server_name, api_key) = (server_name.clone(), api_key.clone());
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
                    is_local,
                    false,
                    None,
                )
                .emit(e);
            }
        })
    };

    let open = {
        let src = if episode.episodeurl.contains("youtube.com") {
            format!(
                "{}/api/data/stream/{}?api_key={}&user_id={}&type=youtube",
                server_name, episode.episodeid, api_key, user_id
            )
        } else if downloaded_server || episode.downloaded || episode.episodeurl.starts_with("local://") {
            format!(
                "{}/api/data/stream/{}?api_key={}&user_id={}",
                server_name, episode.episodeid, api_key, user_id
            )
        } else {
            episode.episodeurl.clone()
        };
        on_shownotes_click(
            BrowserHistory::new(),
            Dispatch::<AppState>::global(),
            episode.episodeid,
            episode.feedurl.clone(),
            src,
            episode.episodetitle.clone(),
            true,
            false,
            episode.is_youtube,
        )
    };
    // A real link, so modifier/middle clicks open a new tab; plain clicks stay in the app.
    let on_open = Callback::from(move |e: MouseEvent| {
        if e.ctrl_key() || e.meta_key() || e.shift_key() || e.alt_key() || e.button() != 0 {
            return;
        }
        e.prevent_default();
        open.emit(e);
    });
    let href = (episode.episodeid > 0).then(|| {
        if episode.is_youtube {
            format!("/episode?episode_id={}&youtube=true", episode.episodeid)
        } else {
            format!("/episode?episode_id={}", episode.episodeid)
        }
    });

    let on_podcast = {
        let (server_name, api_key) = (server_name.clone(), api_key.clone());
        let podcast_id = episode.podcastid;
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            e.prevent_default();
            open_podcast_by_id(
                server_name.clone(),
                api_key.clone(),
                user_id,
                podcast_id,
                BrowserHistory::new(),
                e,
            );
        })
    };

    let art_src = if episode.episodeartwork.is_empty() {
        episode.artworkurl.clone()
    } else {
        episode.episodeartwork.clone()
    };
    let accent = use_art_accent(&art_src);

    EpisodeControls {
        on_play_pause,
        on_open,
        on_podcast,
        href,
        is_playing,
        is_loading,
        is_completed,
        art_src,
        accent,
        server_name,
    }
}

fn play_icon(is_playing: bool, is_loading: bool) -> Html {
    if is_loading {
        html! { <i class="ph ph-spinner home-spin"></i> }
    } else if is_playing {
        html! { <i class="ph ph-pause"></i> }
    } else {
        html! { <i class="ph ph-play"></i> }
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct EpisodeCardProps {
    pub episode: Episode,
}

/// The big Continue Listening card: blurred artwork, progress and a Resume button, tinted
/// with the artwork colour.
#[function_component(ContinueHero)]
pub fn continue_hero(props: &EpisodeCardProps) -> Html {
    let (i18n, _) = use_translation();
    let ep = &props.episode;
    let c = use_episode_controls(ep);

    let pct = if c.is_completed {
        100
    } else if ep.episodeduration > 0 {
        ((ep.listenduration as i64 * 100) / ep.episodeduration as i64).clamp(0, 100) as i32
    } else {
        0
    };
    let remaining = (ep.episodeduration > ep.listenduration && ep.listenduration > 0)
        .then(|| {
            i18n.t("episode_list_item.time_left")
                .replace("{time}", &format_time(ep.episodeduration - ep.listenduration))
        })
        .unwrap_or_else(|| format_time(ep.episodeduration));
    let action = if c.is_playing {
        i18n.t("episode_list_item.pause")
    } else {
        i18n.t("episode_list_item.resume")
    };
    let backdrop = (!c.art_src.is_empty()).then(|| {
        format!(
            "background-image: url('{}');",
            css_url_escape(&proxied_image_url(&c.server_name, &c.art_src, Some(256)))
        )
    });

    html! {
        <div class={classes!("home-hero", c.accent.is_some().then_some("art-scope"))} style={accent_style(&c.accent)}>
            <div class="home-hero-backdrop" style={backdrop}></div>
            <div class="home-hero-scrim"></div>
            <button class="home-hero-art" onclick={c.on_play_pause.clone()} aria-label={action.to_string()}>
                <FallbackImage size={Some(256)} src={c.art_src.clone()} alt={ep.episodetitle.clone()} class="home-hero-img" />
            </button>
            <div class="home-hero-body">
                <span class="home-eyebrow">{ i18n.t("home.continue_listening") }</span>
                <a class="home-hero-title" href={c.href.clone()} onclick={c.on_open.clone()}>{ &ep.episodetitle }</a>
                if !ep.podcastname.is_empty() {
                    <a class="home-hero-podcast" href="#" onclick={c.on_podcast.clone()}>{ &ep.podcastname }</a>
                }
                <div class="home-hero-progress" role="progressbar" aria-valuenow={pct.to_string()} aria-valuemin="0" aria-valuemax="100">
                    <div style={format!("width: {pct}%;")}></div>
                </div>
                <div class="home-hero-actions">
                    <button class="home-hero-resume" onclick={c.on_play_pause.clone()}>
                        { play_icon(c.is_playing, c.is_loading) }
                        <span>{ action }</span>
                    </button>
                    <span class="home-hero-left">{ remaining }</span>
                </div>
            </div>
        </div>
    }
}

/// A square-artwork tile for a horizontal shelf, with a play button over the art.
#[function_component(EpisodeTile)]
pub fn episode_tile(props: &EpisodeCardProps) -> Html {
    let (i18n, _) = use_translation();
    let ep = &props.episode;
    let c = use_episode_controls(ep);
    let pct = if ep.episodeduration > 0 && ep.listenduration > 0 && !c.is_completed {
        ((ep.listenduration as i64 * 100) / ep.episodeduration as i64).clamp(0, 100) as i32
    } else {
        0
    };
    let label = if c.is_playing {
        i18n.t("episode_list_item.pause")
    } else {
        i18n.t("episode_list_item.play")
    };

    html! {
        <div class={classes!("home-tile", c.is_completed.then_some("is-done"))} style={accent_style(&c.accent)}>
            <a class="home-tile-art" href={c.href.clone()} onclick={c.on_open.clone()}>
                <FallbackImage size={Some(256)} src={c.art_src.clone()} alt={ep.episodetitle.clone()} class="home-tile-img" />
                if pct > 0 {
                    <div class="home-tile-progress"><div style={format!("width: {pct}%;")}></div></div>
                }
                <button
                    class={classes!("home-tile-play", (c.is_playing || c.is_loading).then_some("is-on"))}
                    onclick={c.on_play_pause.clone()}
                    aria-label={label.to_string()}
                    title={label.to_string()}
                >
                    { play_icon(c.is_playing, c.is_loading) }
                </button>
            </a>
            <a class="home-tile-title" href={c.href.clone()} onclick={c.on_open.clone()} title={ep.episodetitle.clone()}>
                { &ep.episodetitle }
            </a>
            if !ep.podcastname.is_empty() {
                <a class="home-tile-sub" href="#" onclick={c.on_podcast.clone()}>{ &ep.podcastname }</a>
            }
            <span class="home-tile-meta">
                if c.is_completed {
                    <i class="ph ph-check-circle"></i>
                }
                { format_time(ep.episodeduration) }
            </span>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct ShelfProps {
    pub title: String,
    #[prop_or_default]
    pub see_all: Option<Route>,
    #[prop_or_default]
    pub class: Classes,
    pub children: Html,
}

/// A titled, horizontally scrolling row.
#[function_component(Shelf)]
pub fn shelf(props: &ShelfProps) -> Html {
    let (i18n, _) = use_translation();
    html! {
        <section class="home-shelf">
            <div class="home-shelf-head">
                <h2>{ &props.title }</h2>
                if let Some(route) = props.see_all.clone() {
                    <Link<Route> to={route} classes="home-see-all">
                        { i18n.t("home.see_all") }
                        <i class="ph ph-caret-right"></i>
                    </Link<Route>>
                }
            </div>
            <div class={classes!("home-shelf-row", props.class.clone())}>
                { props.children.clone() }
            </div>
        </section>
    }
}
