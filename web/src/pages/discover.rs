//! The Discover page: a rotating spotlight, a category and language filter, fresh episodes
//! you can play without following, personal picks, the charts, category tiles and hosts.

use crate::components::app_drawer::App_drawer;
use crate::components::art_accent::{accent_style, css_url_escape, use_art_accent};
use crate::components::audio::on_play_click;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, PodcastState, UIState};
use crate::components::gen_components::{on_shownotes_click, proxied_image_url, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::i18n::use_translation;
use crate::components::page_cache;
use crate::pages::podcast_layout::use_podcast_follow;
use crate::requests::discover_req::{
    call_get_categories, call_get_feed_episodes, call_get_recommendations, call_get_trending,
    DiscoverEpisodeItem, PodcastCategory, RecommendedPodcast,
};
use crate::requests::episode::Episode;
use crate::requests::people_req::{self, DiscoverHost};
use crate::requests::search_pods::UnifiedPodcast;
use gloo_timers::callback::Interval;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

const SPOTLIGHT_LEN: usize = 5;
const CHART_LEN: usize = 20;
const FRESH_LEN: usize = 18;
const LANGUAGE_KEY: &str = "discover_language";

/// PodcastIndex category names shown as quick filters, with an icon each.
const FEATURED_CATEGORIES: &[(&str, &str)] = &[
    ("News", "ph-newspaper"),
    ("Comedy", "ph-smiley"),
    ("True Crime", "ph-fingerprint"),
    ("Technology", "ph-cpu"),
    ("Business", "ph-briefcase"),
    ("Society", "ph-users-three"),
    ("Sports", "ph-soccer-ball"),
    ("History", "ph-scroll"),
    ("Science", "ph-atom"),
    ("Health", "ph-heartbeat"),
    ("Education", "ph-graduation-cap"),
    ("Arts", "ph-palette"),
    ("Fiction", "ph-book-open"),
    ("Music", "ph-music-notes"),
    ("Religion", "ph-hands-praying"),
    ("Kids", "ph-baby"),
    ("TV", "ph-television"),
    ("Leisure", "ph-game-controller"),
    ("Government", "ph-bank"),
];

/// Languages offered for the charts: (PodcastIndex code, name in that language).
const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("es", "Español"),
    ("de", "Deutsch"),
    ("fr", "Français"),
    ("pt", "Português"),
    ("it", "Italiano"),
    ("nl", "Nederlands"),
    ("sv", "Svenska"),
    ("pl", "Polski"),
    ("ru", "Русский"),
    ("tr", "Türkçe"),
    ("ja", "日本語"),
    ("ko", "한국어"),
    ("zh", "中文"),
];

fn category_icon(name: &str) -> &'static str {
    FEATURED_CATEGORIES
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, icon)| *icon)
        .unwrap_or("ph-hash")
}

/// Text from a feed description that may carry HTML. Block tags become spaces; inline
/// ones vanish so "<b>word</b>," stays "word,".
fn plain_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut tag = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let name: String = tag
                    .trim_start_matches('/')
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_lowercase();
                if matches!(name.as_str(), "p" | "br" | "div" | "li" | "ul" | "ol" | "h1" | "h2" | "h3" | "h4" | "blockquote") {
                    out.push(' ');
                }
            }
            _ if in_tag => tag.push(c),
            _ => out.push(c),
        }
    }
    out.replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn art_of(p: &UnifiedPodcast) -> String {
    if p.artwork.is_empty() {
        p.image.clone()
    } else {
        p.artwork.clone()
    }
}

fn short_duration(secs: i32) -> String {
    let mins = (secs + 30) / 60;
    match (mins / 60, mins % 60) {
        (0, 0) => String::new(),
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

fn ago(i18n: &i18nrs::I18n, published: i64) -> String {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let secs = (now - published).max(0);
    let (key, n) = if secs < 3600 {
        ("discover.minutes_ago", (secs / 60).max(1))
    } else if secs < 86_400 {
        ("discover.hours_ago", secs / 3600)
    } else {
        ("discover.days_ago", secs / 86_400)
    };
    i18n.t(key).replace("{count}", &n.to_string())
}

/// What the filtered part of the page shows, cached per filter for instant revisits.
#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
struct DiscoverFeed {
    trending: Vec<UnifiedPodcast>,
    episodes: Vec<DiscoverEpisodeItem>,
}

/// Newest first, at most two per show.
fn pick_fresh(items: Vec<DiscoverEpisodeItem>) -> Vec<DiscoverEpisodeItem> {
    let mut items = items;
    items.sort_by(|a, b| b.datePublished.cmp(&a.datePublished));
    let mut per_feed: HashMap<i64, usize> = HashMap::new();
    items
        .into_iter()
        .filter(|e| !e.enclosureUrl.is_empty())
        .filter(|e| {
            let n = per_feed.entry(e.feedId).or_default();
            *n += 1;
            *n <= 2
        })
        .take(FRESH_LEN)
        .collect()
}

fn group_by_reason(recs: &[RecommendedPodcast]) -> Vec<(String, Vec<UnifiedPodcast>)> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: HashMap<String, Vec<UnifiedPodcast>> = HashMap::new();
    for r in recs {
        if !groups.contains_key(&r.reason) {
            order.push(r.reason.clone());
        }
        groups.entry(r.reason.clone()).or_default().push(r.to_unified());
    }
    order
        .into_iter()
        .map(|reason| {
            let items = groups.remove(&reason).unwrap_or_default();
            (reason, items)
        })
        .collect()
}

fn stored_language() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(LANGUAGE_KEY).ok().flatten())
}

fn default_language(ui_language: &str) -> String {
    if let Some(stored) = stored_language() {
        return stored;
    }
    let code = ui_language.split(['-', '_']).next().unwrap_or("").to_lowercase();
    if LANGUAGES.iter().any(|(c, _)| *c == code) {
        code
    } else {
        String::new()
    }
}

#[function_component(Discover)]
pub fn discover() -> Html {
    let (i18n, _) = use_translation();
    let (post_state, _) = use_store::<AppState>();
    let api_key = post_state.auth_details.as_ref().and_then(|a| a.api_key.clone());
    let server_name = post_state.auth_details.as_ref().map(|a| a.server_name.clone());
    let user_id = post_state.user_details.as_ref().map(|u| u.UserID);

    let category = use_state(|| None::<String>);
    let language = {
        let ui = i18n.get_current_language().to_string();
        use_state(move || default_language(&ui))
    };
    let feed = use_state(|| None::<DiscoverFeed>);
    let episodes_failed = use_state(|| false);
    let categories = use_state(Vec::<PodcastCategory>::new);
    let show_all_categories = use_state(|| false);
    let recommendations = use_state(|| None::<Vec<RecommendedPodcast>>);
    let hosts = use_state(Vec::<DiscoverHost>::new);

    // Things that don't depend on the filter: categories, picks, hosts, and which shows
    // the user follows (so the tiles don't each ask).
    {
        let (api_key, server_name) = (api_key.clone(), server_name.clone());
        let (categories, recommendations, hosts) = (categories.clone(), recommendations.clone(), hosts.clone());
        use_effect_with((server_name.clone(), user_id), move |_| {
            if let (Some(server), Some(key), Some(uid)) = (server_name, api_key, user_id) {
                spawn_local(async move {
                    let key_opt = Some(key.clone());
                    let (cats, recs, top, followed) = futures::join!(
                        call_get_categories(&server, &key),
                        call_get_recommendations(&server, &key, false),
                        people_req::call_discover_top_hosts(&server, &key, 18),
                        crate::requests::pod_req::call_get_podcasts(&server, &key_opt, &uid),
                    );
                    if let Ok(mut v) = cats {
                        v.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                        categories.set(v);
                    }
                    recommendations.set(Some(recs.unwrap_or_default()));
                    if let Ok(v) = top {
                        hosts.set(v);
                    }
                    if let Ok(list) = followed {
                        Dispatch::<PodcastState>::global().reduce_mut(move |s| {
                            for p in list {
                                s.added_podcast_urls.insert(p.feedurl);
                            }
                        });
                    }
                });
            }
            || ()
        });
    }

    // The charts and fresh episodes for the chosen category and language.
    {
        let (api_key, server_name) = (api_key.clone(), server_name.clone());
        let (feed, episodes_failed) = (feed.clone(), episodes_failed.clone());
        use_effect_with(
            ((*category).clone(), (*language).clone(), server_name.clone()),
            move |(cat, lang, _)| {
                if let (Some(server), Some(key), Some(uid)) = (server_name, api_key, user_id) {
                    let cache_key = page_cache::key(
                        &format!("discover:{}:{}", cat.clone().unwrap_or_default(), lang),
                        &server,
                        uid,
                    );
                    feed.set(page_cache::memory::<DiscoverFeed>(&cache_key));
                    let (cat, lang) = (cat.clone(), lang.clone());
                    spawn_local(async move {
                        let lang = (!lang.is_empty()).then_some(lang);
                        let trending = call_get_trending(&server, &key, cat.as_deref(), lang.as_deref(), 40)
                            .await
                            .unwrap_or_default();
                        let ids: Vec<i32> = trending.iter().take(30).map(|p| p.index_id).collect();
                        let episodes = match call_get_feed_episodes(&server, &key, &ids, 80).await {
                            Ok(items) => {
                                episodes_failed.set(false);
                                pick_fresh(items)
                            }
                            Err(_) => {
                                episodes_failed.set(true);
                                Vec::new()
                            }
                        };
                        let fresh = DiscoverFeed { trending, episodes };
                        page_cache::put(&cache_key, &fresh);
                        feed.set(Some(fresh));
                    });
                }
                || ()
            },
        );
    }

    let on_category = {
        let category = category.clone();
        Callback::from(move |cat: Option<String>| {
            category.set(cat);
            if let Some(w) = web_sys::window() {
                w.scroll_to_with_x_and_y(0.0, 0.0);
            }
        })
    };
    let on_language = {
        let language = language.clone();
        Callback::from(move |e: Event| {
            let value = e.target_unchecked_into::<web_sys::HtmlSelectElement>().value();
            if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
                let _ = storage.set_item(LANGUAGE_KEY, &value);
            }
            language.set(value);
        })
    };

    let trending: Vec<UnifiedPodcast> = feed.as_ref().map(|f| f.trending.clone()).unwrap_or_default();
    let by_feed: HashMap<i64, UnifiedPodcast> = trending.iter().map(|p| (p.index_id as i64, p.clone())).collect();
    let fresh: Vec<(Episode, i64)> = feed
        .as_ref()
        .map(|f| {
            f.episodes
                .iter()
                .filter_map(|e| by_feed.get(&e.feedId).map(|p| (e.to_episode(p), e.datePublished)))
                .collect()
        })
        .unwrap_or_default();
    // Newest playable episode per show, for the spotlight's play button.
    let mut latest: HashMap<String, Episode> = HashMap::new();
    for (ep, _) in &fresh {
        latest.entry(ep.feedurl.clone()).or_insert_with(|| ep.clone());
    }
    let loading = feed.is_none();
    let spotlight: Vec<UnifiedPodcast> = trending.iter().take(SPOTLIGHT_LEN).cloned().collect();
    let chart: Vec<UnifiedPodcast> = trending.iter().skip(SPOTLIGHT_LEN).take(CHART_LEN).cloned().collect();
    let scope = match (*category).as_ref() {
        Some(c) => i18n.t("discover.trending_in").replace("{category}", c),
        None => i18n.t("discover.trending_everywhere"),
    };

    let featured: Vec<(&str, &str)> = FEATURED_CATEGORIES
        .iter()
        .filter(|(name, _)| categories.is_empty() || categories.iter().any(|c| c.name.eq_ignore_ascii_case(name)))
        .copied()
        .collect();
    let extra: Vec<String> = categories
        .iter()
        .map(|c| c.name.clone())
        .filter(|n| !FEATURED_CATEGORIES.iter().any(|(f, _)| f.eq_ignore_ascii_case(n)))
        .collect();

    let chip = |name: Option<String>, icon: &str, label: String| -> Html {
        let active = *category == name;
        let onclick = {
            let on_category = on_category.clone();
            Callback::from(move |_: MouseEvent| on_category.emit(name.clone()))
        };
        html! {
            <button class={classes!("dsc-chip", active.then_some("is-active"))} {onclick}>
                <i class={classes!("ph", icon.to_string())}></i>{ label }
            </button>
        }
    };

    html! {
        <>
        <div class="main-container dsc-page">
            <Search_nav />
            <UseScrollToTop />

            <header class="dsc-head">
                <div>
                    <h1 class="dsc-title">{ i18n.t("discover.title") }</h1>
                    <p class="dsc-sub">{ i18n.t("discover.subtitle") }</p>
                </div>
                <label class="dsc-lang">
                    <i class="ph ph-globe-simple"></i>
                    <select onchange={on_language} aria-label={i18n.t("discover.language")}>
                        <option value="" selected={language.is_empty()}>{ i18n.t("discover.all_languages") }</option>
                        { for LANGUAGES.iter().map(|(code, name)| html! {
                            <option value={*code} selected={*language == *code}>{ *name }</option>
                        }) }
                    </select>
                </label>
            </header>

            <nav class={classes!("dsc-chips", show_all_categories.then_some("is-open"))}>
                { chip(None, "ph-compass", i18n.t("discover.all_categories")) }
                { for featured.iter().map(|(name, icon)| chip(Some(name.to_string()), icon, name.to_string())) }
                if *show_all_categories {
                    { for extra.iter().map(|name| chip(Some(name.clone()), "ph-hash", name.clone())) }
                }
                if !extra.is_empty() {
                    <button class="dsc-chip is-more" onclick={{
                        let s = show_all_categories.clone();
                        Callback::from(move |_: MouseEvent| s.set(!*s))
                    }}>
                        { if *show_all_categories { i18n.t("discover.fewer") } else { i18n.t("discover.more_categories") } }
                    </button>
                }
            </nav>

            if loading {
                <div class="dsc-spotlight is-skeleton"></div>
            } else if spotlight.is_empty() {
                <div class="dsc-empty">
                    <i class="ph ph-binoculars"></i>
                    <h3>{ i18n.t("discover.empty_title") }</h3>
                    <p>{ i18n.t("discover.empty_message") }</p>
                </div>
            } else {
                <Spotlight podcasts={spotlight} latest={latest.clone()} scope={scope.clone()} />
            }

            if loading || !fresh.is_empty() {
                <section class="dsc-section">
                    <div class="dsc-section-head">
                        <h2><i class="ph ph-sparkle"></i>{ i18n.t("discover.fresh_episodes") }</h2>
                        <p>{ i18n.t("discover.fresh_episodes_hint") }</p>
                    </div>
                    <div class="dsc-rail">
                        if loading {
                            { for (0..6).map(|_| html! { <div class="dsc-ep is-skeleton"><div class="dsc-ep-art"></div><span></span><span></span></div> }) }
                        } else {
                            { for fresh.iter().map(|(ep, published)| html! {
                                <FreshEpisode key={ep.episodeurl.clone()} episode={ep.clone()} age={ago(&i18n, *published)} />
                            }) }
                        }
                    </div>
                </section>
            } else if *episodes_failed && !trending.is_empty() {
                <p class="dsc-note"><i class="ph ph-info"></i>{ i18n.t("discover.episodes_unavailable") }</p>
            }

            if category.is_none() {
                {
                    match &*recommendations {
                        Some(recs) if !recs.is_empty() => html! {
                            { for group_by_reason(recs).into_iter().take(4).map(|(reason, items)| html! {
                                <section class="dsc-section">
                                    <div class="dsc-section-head">
                                        <h2><i class="ph ph-heart"></i>{ reason }</h2>
                                    </div>
                                    <div class="dsc-rail">
                                        { for items.iter().map(|p| html! { <PodcastTile key={p.url.clone()} podcast={p.clone()} /> }) }
                                    </div>
                                </section>
                            }) }
                        },
                        Some(_) => html! {
                            <section class="dsc-section">
                                <div class="dsc-callout">
                                    <i class="ph ph-heart"></i>
                                    <div>
                                        <h3>{ i18n.t("discover.for_you") }</h3>
                                        <p>{ i18n.t("discover.for_you_empty") }</p>
                                    </div>
                                </div>
                            </section>
                        },
                        None => html! {},
                    }
                }
            }

            if loading || !chart.is_empty() {
                <section class="dsc-section">
                    <div class="dsc-section-head">
                        <h2><i class="ph ph-trend-up"></i>{ i18n.t("discover.top_charts") }</h2>
                        <p>{ scope.clone() }</p>
                    </div>
                    <ol class="dsc-chart">
                        if loading {
                            { for (0..8).map(|_| html! { <li class="dsc-chart-row is-skeleton"><span></span><span></span></li> }) }
                        } else {
                            { for chart.iter().enumerate().map(|(i, p)| html! {
                                <ChartRow key={p.url.clone()} rank={i + SPOTLIGHT_LEN + 1} podcast={p.clone()} />
                            }) }
                        }
                    </ol>
                </section>
            }

            if category.is_none() {
                <section class="dsc-section">
                    <div class="dsc-section-head">
                        <h2><i class="ph ph-squares-four"></i>{ i18n.t("discover.browse_categories") }</h2>
                    </div>
                    <div class="dsc-cat-grid">
                        { for featured.iter().take(12).enumerate().map(|(i, (name, icon))| {
                            let onclick = {
                                let on_category = on_category.clone();
                                let name = name.to_string();
                                Callback::from(move |_: MouseEvent| on_category.emit(Some(name.clone())))
                            };
                            html! {
                                <button class={classes!("dsc-cat", format!("hue-{}", i % 12))} {onclick}>
                                    <i class={classes!("ph", icon.to_string())}></i>
                                    <span>{ *name }</span>
                                </button>
                            }
                        }) }
                    </div>
                </section>
            }

            if !hosts.is_empty() && category.is_none() {
                <section class="dsc-section">
                    <div class="dsc-section-head">
                        <h2><i class="ph ph-microphone-stage"></i>{ i18n.t("discover.hosts_section") }</h2>
                        <p>{ i18n.t("discover.hosts_hint") }</p>
                    </div>
                    <div class="dsc-rail dsc-hosts">
                        { for hosts.iter().map(|h| {
                            let id = h.id;
                            let onclick = Callback::from(move |_: MouseEvent| {
                                BrowserHistory::new().push(format!("/person/pp-{}", id));
                            });
                            html! {
                                <button class="dsc-host" {onclick}>
                                    <FallbackImage size={Some(192)} src={h.img.clone()} alt={h.name.clone()} class="dsc-host-img" />
                                    <span class="dsc-host-name">{ &h.name }</span>
                                    if h.podcast_count > 0 {
                                        <span class="dsc-host-sub">{ i18n.t("discover.shows_count").replace("{count}", &h.podcast_count.to_string()) }</span>
                                    }
                                </button>
                            }
                        }) }
                    </div>
                </section>
            }

            <AudioPlayerBar />
        </div>
        <App_drawer />
        </>
    }
}

#[derive(Properties, PartialEq)]
struct SpotlightProps {
    podcasts: Vec<UnifiedPodcast>,
    latest: HashMap<String, Episode>,
    scope: String,
}

/// The big rotating banner for the top shows.
#[function_component(Spotlight)]
fn spotlight(props: &SpotlightProps) -> Html {
    let index = use_state(|| 0usize);
    let paused = use_state(|| false);
    let len = props.podcasts.len();

    // Start over when the list changes (a new filter).
    {
        let index = index.clone();
        use_effect_with(props.podcasts.first().map(|p| p.url.clone()), move |_| {
            index.set(0);
            || ()
        });
    }
    {
        let index = index.clone();
        use_effect_with((*paused, len), move |(paused, len)| {
            let timer = (!*paused && *len > 1).then(|| {
                let len = *len;
                Interval::new(9_000, move || index.set((*index + 1) % len))
            });
            move || drop(timer)
        });
    }

    let current = (*index).min(len.saturating_sub(1));
    let go = |to: usize| {
        let index = index.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            index.set(to);
        })
    };
    let prev = go((current + len - 1) % len.max(1));
    let next = go((current + 1) % len.max(1));

    html! {
        <section
            class="dsc-spotlight"
            onmouseenter={{ let p = paused.clone(); Callback::from(move |_| p.set(true)) }}
            onmouseleave={{ let p = paused.clone(); Callback::from(move |_| p.set(false)) }}
        >
            { for props.podcasts.iter().enumerate().map(|(i, p)| html! {
                <SpotlightSlide
                    key={p.url.clone()}
                    podcast={p.clone()}
                    rank={i + 1}
                    active={i == current}
                    scope={props.scope.clone()}
                    latest={props.latest.get(&p.url).cloned()}
                />
            }) }
            if len > 1 {
                <button class="dsc-spot-nav is-prev" onclick={prev} aria-label="Previous"><i class="ph ph-caret-left"></i></button>
                <button class="dsc-spot-nav is-next" onclick={next} aria-label="Next"><i class="ph ph-caret-right"></i></button>
                <div class="dsc-spot-dots">
                    { for (0..len).map(|i| html! {
                        <button class={classes!("dsc-dot", (i == current).then_some("is-active"), (*paused).then_some("is-paused"))} onclick={go(i)} aria-label={format!("{}", i + 1)}>
                            <span></span>
                        </button>
                    }) }
                </div>
            }
        </section>
    }
}

#[derive(Properties, PartialEq)]
struct SpotlightSlideProps {
    podcast: UnifiedPodcast,
    rank: usize,
    active: bool,
    scope: String,
    latest: Option<Episode>,
}

#[function_component(SpotlightSlide)]
fn spotlight_slide(props: &SpotlightSlideProps) -> Html {
    let (i18n, _) = use_translation();
    let p = &props.podcast;
    let follow = use_podcast_follow(p, false);
    let art = art_of(p);
    let accent = use_art_accent(&art);
    let server = use_selector(|s: &AppState| s.auth_details.as_ref().map(|a| a.server_name.clone()).unwrap_or_default());
    let backdrop = proxied_image_url(&server, &art, Some(256));
    let categories: Vec<String> = p
        .categories
        .as_ref()
        .map(|c| c.values().take(3).cloned().collect())
        .unwrap_or_default();

    html! {
        <article
            class={classes!("dsc-slide", props.active.then_some("is-active"))}
            style={accent_style(&accent)}
            aria-hidden={(!props.active).to_string()}
        >
            <div class="dsc-slide-backdrop" style={format!("background-image:url('{}')", css_url_escape(&backdrop))}></div>
            <div class="dsc-slide-scrim"></div>
            <button class="dsc-slide-art" onclick={follow.open.clone()} tabindex={if props.active { "0" } else { "-1" }}>
                <FallbackImage size={Some(512)} src={art.clone()} alt={p.title.clone()} class="dsc-slide-img" />
            </button>
            <div class="dsc-slide-body">
                <p class="dsc-eyebrow"><span class="dsc-rank">{ format!("#{}", props.rank) }</span>{ &props.scope }</p>
                <h2 class="dsc-slide-title" onclick={follow.open.clone()}>{ &p.title }</h2>
                if !p.author.is_empty() {
                    <p class="dsc-slide-author">{ &p.author }</p>
                }
                <p class="dsc-slide-desc">{ plain_text(&p.description) }</p>
                if !categories.is_empty() {
                    <div class="dsc-slide-tags">
                        { for categories.iter().map(|c| html! { <span><i class={classes!("ph", category_icon(c))}></i>{ c }</span> }) }
                    </div>
                }
                <div class="dsc-slide-actions">
                    if let Some(ep) = props.latest.clone() {
                        <PlayLatest episode={ep} />
                    }
                    <button class={classes!("dsc-btn", follow.is_added.then_some("is-following"))} onclick={follow.toggle.clone()} disabled={follow.loading}>
                        <i class={classes!("ph", if follow.loading { "ph-spinner-gap" } else if follow.is_added { "ph-check" } else { "ph-plus" })}></i>
                        { if follow.is_added { i18n.t("discover.following") } else { i18n.t("discover.follow") } }
                    </button>
                    <button class="dsc-btn is-ghost" onclick={follow.open.clone()}>
                        <i class="ph ph-arrow-up-right"></i>{ i18n.t("discover.open_show") }
                    </button>
                </div>
            </div>
        </article>
    }
}

/// Plays or pauses an episode straight from its feed; the server keeps progress once it
/// has resolved the episode.
#[hook]
fn use_feed_play(episode: &Episode) -> (Callback<MouseEvent>, bool) {
    let url = episode.episodeurl.clone();
    let state = use_selector(move |s: &UIState| {
        let current = s
            .currently_playing
            .as_ref()
            .map_or(false, |cp| cp.src == url || cp.episode.episodeurl == url);
        (current, current && s.audio_playing.unwrap_or(false))
    });
    let (is_current, playing) = *state;
    let auth = use_selector(|s: &AppState| {
        (
            s.auth_details.as_ref().map(|a| a.server_name.clone()).unwrap_or_default(),
            s.auth_details.as_ref().and_then(|a| a.api_key.clone()).unwrap_or_default(),
            s.user_details.as_ref().map(|u| u.UserID).unwrap_or_default(),
        )
    });
    let on_click = {
        let episode = episode.clone();
        let (server, key, uid) = (*auth).clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            let dispatch = Dispatch::<UIState>::global();
            if is_current {
                dispatch.reduce_mut(|s| s.toggle_playback());
                return;
            }
            on_play_click(
                episode.clone(),
                key.clone(),
                uid,
                server.clone(),
                dispatch.clone(),
                dispatch.get(),
                false,
                false,
                None,
            )
            .emit(e);
        })
    };
    (on_click, playing)
}

#[derive(Properties, PartialEq)]
struct EpisodeProps {
    episode: Episode,
}

#[function_component(PlayLatest)]
fn play_latest(props: &EpisodeProps) -> Html {
    let (i18n, _) = use_translation();
    let (on_click, playing) = use_feed_play(&props.episode);
    html! {
        <button class="dsc-btn is-primary" onclick={on_click} title={props.episode.episodetitle.clone()}>
            <i class={classes!("ph", if playing { "ph-pause" } else { "ph-play" })}></i>
            { if playing { i18n.t("discover.pause") } else { i18n.t("discover.play_latest") } }
        </button>
    }
}

#[derive(Properties, PartialEq)]
struct FreshEpisodeProps {
    episode: Episode,
    age: String,
}

#[function_component(FreshEpisode)]
fn fresh_episode(props: &FreshEpisodeProps) -> Html {
    let (i18n, _) = use_translation();
    let ep = &props.episode;
    let (on_play, playing) = use_feed_play(ep);
    let accent = use_art_accent(&ep.episodeartwork);
    let opening = use_state(|| false);
    let auth = use_selector(|s: &AppState| {
        (
            s.auth_details.as_ref().map(|a| a.server_name.clone()).unwrap_or_default(),
            s.auth_details.as_ref().and_then(|a| a.api_key.clone()).unwrap_or_default(),
            s.user_details.as_ref().map(|u| u.UserID).unwrap_or_default(),
        )
    });
    // The episode page needs a server id, so resolve the feed episode first.
    let on_open = {
        let episode = ep.clone();
        let opening = opening.clone();
        let (server, key, uid) = (*auth).clone();
        Callback::from(move |e: MouseEvent| {
            if *opening {
                return;
            }
            opening.set(true);
            let (episode, server, key, opening) = (episode.clone(), server.clone(), key.clone(), opening.clone());
            spawn_local(async move {
                match crate::requests::pod_req::resolve_feed_episode(&server, &key, uid, &episode).await {
                    Ok(resolved) => on_shownotes_click(
                        BrowserHistory::new(),
                        Dispatch::<AppState>::global(),
                        resolved.episodeid,
                        resolved.feedurl.clone(),
                        resolved.episodeurl.clone(),
                        resolved.episodetitle.clone(),
                        true,
                        false,
                        false,
                    )
                    .emit(e),
                    Err(err) => web_sys::console::log_1(&format!("Could not open the episode: {err}").into()),
                }
                opening.set(false);
            });
        })
    };
    let label = if playing { i18n.t("discover.pause") } else { i18n.t("discover.play") };
    html! {
        <article class={classes!("dsc-ep", (*opening).then_some("is-opening"))} style={accent_style(&accent)} onclick={on_open}>
            <div class="dsc-ep-art">
                <FallbackImage size={Some(320)} src={ep.episodeartwork.clone()} alt="" class="dsc-ep-img" />
                <button class={classes!("dsc-ep-play", playing.then_some("is-on"))} onclick={on_play} title={label.clone()} aria-label={label}>
                    <i class={classes!("ph", if playing { "ph-pause" } else { "ph-play" })}></i>
                </button>
                if ep.episodeduration > 0 {
                    <span class="dsc-ep-len">{ short_duration(ep.episodeduration) }</span>
                }
            </div>
            <span class="dsc-ep-show">{ &ep.podcastname }</span>
            <h3 class="dsc-ep-title" title={ep.episodetitle.clone()}>{ &ep.episodetitle }</h3>
            <span class="dsc-ep-age">{ &props.age }</span>
        </article>
    }
}

#[derive(Properties, PartialEq)]
struct PodcastTileProps {
    podcast: UnifiedPodcast,
}

#[function_component(PodcastTile)]
fn podcast_tile(props: &PodcastTileProps) -> Html {
    let (i18n, _) = use_translation();
    let p = &props.podcast;
    let follow = use_podcast_follow(p, false);
    let label = if follow.is_added { i18n.t("discover.following") } else { i18n.t("discover.follow") };
    html! {
        <article class="dsc-tile" onclick={follow.open.clone()}>
            <div class="dsc-tile-art">
                <FallbackImage size={Some(320)} src={art_of(p)} alt={p.title.clone()} class="dsc-tile-img" />
                <button class={classes!("dsc-follow", follow.is_added.then_some("is-on"))} onclick={follow.toggle.clone()} disabled={follow.loading} title={label.clone()} aria-label={label}>
                    <i class={classes!("ph", if follow.loading { "ph-spinner-gap" } else if follow.is_added { "ph-check" } else { "ph-plus" })}></i>
                </button>
            </div>
            <h3 class="dsc-tile-title" title={p.title.clone()}>{ &p.title }</h3>
            if !p.author.is_empty() {
                <span class="dsc-tile-sub">{ &p.author }</span>
            }
        </article>
    }
}

#[derive(Properties, PartialEq)]
struct ChartRowProps {
    rank: usize,
    podcast: UnifiedPodcast,
}

#[function_component(ChartRow)]
fn chart_row(props: &ChartRowProps) -> Html {
    let (i18n, _) = use_translation();
    let p = &props.podcast;
    let follow = use_podcast_follow(p, false);
    let category = p.categories.as_ref().and_then(|c| c.values().next().cloned());
    let label = if follow.is_added { i18n.t("discover.following") } else { i18n.t("discover.follow") };
    html! {
        <li class="dsc-chart-row" onclick={follow.open.clone()}>
            <span class="dsc-chart-rank">{ props.rank }</span>
            <FallbackImage size={Some(128)} src={art_of(p)} alt="" class="dsc-chart-img" />
            <span class="dsc-chart-text">
                <span class="dsc-chart-title">{ &p.title }</span>
                <span class="dsc-chart-sub">
                    { &p.author }
                    if let Some(c) = category {
                        <span class="dsc-chart-cat"><i class={classes!("ph", category_icon(&c))}></i>{ c }</span>
                    }
                </span>
            </span>
            <button class={classes!("dsc-follow", "is-inline", follow.is_added.then_some("is-on"))} onclick={follow.toggle.clone()} disabled={follow.loading} title={label.clone()} aria-label={label}>
                <i class={classes!("ph", if follow.loading { "ph-spinner-gap" } else if follow.is_added { "ph-check" } else { "ph-plus" })}></i>
            </button>
        </li>
    }
}
