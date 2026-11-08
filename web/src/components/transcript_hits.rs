//! "Said in" results on the search page: episodes whose transcripts contain the search, each
//! with the moments it was said and a button to play from there.

use crate::components::audio::on_play_click;
use crate::components::context::{AppState, UIState};
use crate::components::gen_components::FallbackImage;
use crate::components::gen_funcs::{format_date, format_time_rm_hour};
use crate::requests::episode::Episode;
use crate::requests::search_pods::{call_search_transcripts, TranscriptHit, TranscriptSearchPage};
use i18nrs::yew::use_translation;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

const PAGE: i64 = 20;
/// Moments shown before "more moments".
const FOLDED_MOMENTS: usize = 2;

/// `text` with every occurrence of `terms` (case-insensitive) wrapped in `<mark>`.
pub fn highlight_terms(text: &str, terms: &[String]) -> Html {
    let chars: Vec<char> = text.chars().collect();
    let folded: Vec<char> = chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let needles: Vec<Vec<char>> = terms
        .iter()
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().map(|c| c.to_lowercase().next().unwrap_or(c)).collect())
        .collect();
    let mut marks = vec![false; chars.len()];
    for n in &needles {
        let mut i = 0;
        while i + n.len() <= folded.len() {
            if folded[i..i + n.len()] == n[..] {
                marks[i..i + n.len()].iter_mut().for_each(|m| *m = true);
                i += n.len();
            } else {
                i += 1;
            }
        }
    }
    let mut parts: Vec<Html> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let on = marks[i];
        let start = i;
        while i < chars.len() && marks[i] == on {
            i += 1;
        }
        let piece: String = chars[start..i].iter().collect();
        parts.push(if on { html! { <mark>{ piece }</mark> } } else { html! { { piece } } });
    }
    html! { <>{ for parts }</> }
}

/// Play `episode` from `seconds`: seek when it's already loaded, otherwise start it there.
pub fn play_from(episode: Episode, seconds: f64, e: MouseEvent) {
    let ui = Dispatch::<UIState>::global();
    let loaded = ui.get().currently_playing.as_ref().is_some_and(|c| c.episode_id == episode.episodeid);
    if loaded {
        ui.reduce_mut(|state| {
            if let Some(media) = state.media_element.as_ref() {
                media.set_current_time(seconds);
                let _ = media.play();
            } else if let Some(audio) = state.audio_element.as_ref() {
                audio.set_current_time(seconds);
                let _ = audio.play();
            }
            state.current_time_seconds = seconds;
            state.audio_playing = Some(true);
        });
        return;
    }
    let app = Dispatch::<AppState>::global().get();
    let (Some(auth), Some(user)) = (app.auth_details.as_ref(), app.user_details.as_ref()) else { return };
    let Some(api_key) = auth.api_key.clone() else { return };
    let mut episode = episode;
    episode.listenduration = seconds.max(0.0) as i32;
    on_play_click(episode, api_key, user.UserID, auth.server_name.clone(), ui.clone(), ui.get(), false, false, None).emit(e);
}

/// The episode page, opened at `seconds` in its transcript with `query` found.
pub fn episode_link(episode_id: i32, seconds: Option<f64>, query: &str, source: &str) -> String {
    let mut url = format!("/episode?episode_id={}&q={}&source={}", episode_id, urlencoding::encode(query), source);
    if let Some(t) = seconds {
        url.push_str(&format!("&t={}", t.floor() as i64));
    }
    url
}

#[derive(Properties, PartialEq)]
pub struct TranscriptHitsProps {
    /// The committed search.
    pub term: String,
    /// Matching episodes once loaded, None while loading.
    #[prop_or_default]
    pub on_total: Callback<Option<i64>>,
}

#[function_component(TranscriptHits)]
pub fn transcript_hits(props: &TranscriptHitsProps) -> Html {
    let (i18n, _) = use_translation();
    let (app, _) = use_store::<AppState>();
    let server = app.auth_details.as_ref().map(|a| a.server_name.clone());
    let api_key = app.auth_details.as_ref().and_then(|a| a.api_key.clone());

    let page = use_state(|| Rc::new(TranscriptSearchPage::default()));
    let loading = use_state(|| false);
    let failed = use_state(|| false);
    let expanded = use_state(HashSet::<i32>::new);
    let latest = use_mut_ref(String::new);

    // Search again whenever the committed term changes.
    {
        let (page, loading, failed, expanded, latest) = (page.clone(), loading.clone(), failed.clone(), expanded.clone(), latest.clone());
        let on_total = props.on_total.clone();
        let (server, api_key) = (server.clone(), api_key.clone());
        use_effect_with(props.term.clone(), move |term| {
            *latest.borrow_mut() = term.clone();
            page.set(Rc::new(TranscriptSearchPage::default()));
            expanded.set(HashSet::new());
            failed.set(false);
            if let (Some(server), Some(key), false) = (server, api_key, term.trim().is_empty()) {
                loading.set(true);
                on_total.emit(None);
                let term = term.clone();
                spawn_local(async move {
                    let res = call_search_transcripts(&server, &key, &term, PAGE, 0).await;
                    if *latest.borrow() != term {
                        return;
                    }
                    match res {
                        Ok(p) => {
                            on_total.emit(Some(p.total));
                            page.set(Rc::new(p));
                        }
                        Err(e) => {
                            web_sys::console::log_1(&format!("Transcript search failed: {e}").into());
                            failed.set(true);
                            on_total.emit(Some(0));
                        }
                    }
                    loading.set(false);
                });
            }
            || ()
        });
    }

    let load_more = {
        let (page, loading, latest) = (page.clone(), loading.clone(), latest.clone());
        let (server, api_key) = (server.clone(), api_key.clone());
        let term = props.term.clone();
        Callback::from(move |_: MouseEvent| {
            let (Some(server), Some(key)) = (server.clone(), api_key.clone()) else { return };
            if *loading {
                return;
            }
            loading.set(true);
            let (page, loading, latest, term) = (page.clone(), loading.clone(), latest.clone(), term.clone());
            let offset = page.results.len() as i64;
            spawn_local(async move {
                if let Ok(more) = call_search_transcripts(&server, &key, &term, PAGE, offset).await {
                    if *latest.borrow() == term {
                        let mut next = (**page).clone();
                        next.results.extend(more.results);
                        next.total = more.total;
                        page.set(Rc::new(next));
                    }
                }
                loading.set(false);
            });
        })
    };

    let history = BrowserHistory::new();
    let terms = page.terms.clone();
    let count_label = if page.total == 1 {
        i18n.t("search.transcripts_one_episode")
    } else {
        i18n.t("search.transcripts_episodes").replace("{count}", &page.total.to_string())
    };

    let body = if *loading && page.results.is_empty() {
        html! {
            <div class="ts-skeleton">
                { for (0..3).map(|_| html! { <div class="ts-skeleton-row"></div> }) }
            </div>
        }
    } else if *failed {
        html! { <div class="ts-empty"><p>{ i18n.t("search.transcripts_failed") }</p></div> }
    } else if page.results.is_empty() {
        if page.searchable_episodes == 0 {
            html! {
                <div class="ts-empty">
                    <i class="ph ph-scroll"></i>
                    <div>
                        <h4>{ i18n.t("search.transcripts_none_yet") }</h4>
                        <p>{ i18n.t("search.transcripts_none_yet_body") }</p>
                    </div>
                </div>
            }
        } else {
            html! {
                <div class="ts-empty">
                    <i class="ph ph-scroll"></i>
                    <div>
                        <h4>{ i18n.t("search.transcripts_no_match") }</h4>
                        <p>{ i18n.t("search.transcripts_searched").replace("{count}", &page.searchable_episodes.to_string()) }</p>
                    </div>
                </div>
            }
        }
    } else {
        html! {
            <>
                <div class="ts-hits">
                    { for page.results.iter().map(|hit| render_hit(hit, &terms, &props.term, &expanded, &history, &i18n)) }
                </div>
                if (page.results.len() as i64) < page.total {
                    <button class="ts-load-more" onclick={load_more} disabled={*loading}>
                        if *loading { <i class="ph ph-circle-notch ts-spin"></i> }
                        { i18n.t("search.transcripts_more_episodes") }
                    </button>
                }
            </>
        }
    };

    html! {
        <section class="ts-section">
            <div class="sp-sec-head ts-head">
                <h3><i class="ph ph-scroll"></i>{ i18n.t("search.transcripts_heading") }</h3>
                if !page.results.is_empty() {
                    <span class="ts-head-count">{ count_label }</span>
                }
            </div>
            { body }
        </section>
    }
}

fn render_hit(
    hit: &TranscriptHit,
    terms: &[String],
    query: &str,
    expanded: &UseStateHandle<HashSet<i32>>,
    history: &BrowserHistory,
    i18n: &i18nrs::I18n,
) -> Html {
    let ep = &hit.episode;
    let id = ep.episodeid;
    let is_open = expanded.contains(&id);
    let shown = if is_open { hit.moments.len() } else { hit.moments.len().min(FOLDED_MOMENTS) };
    let hidden = hit.moments.len() - shown;

    let open_episode = {
        let history = history.clone();
        let link = episode_link(id, hit.moments.first().and_then(|m| m.start), query, &hit.source);
        Callback::from(move |_: MouseEvent| history.push(link.clone()))
    };
    let toggle = {
        let expanded = expanded.clone();
        Callback::from(move |_: MouseEvent| {
            let mut set = (*expanded).clone();
            if !set.remove(&id) {
                set.insert(id);
            }
            expanded.set(set);
        })
    };
    let source_label = if hit.source == "generated" {
        i18n.t("search.transcripts_source_ai")
    } else {
        i18n.t("search.transcripts_source_feed")
    };
    let mentions = if hit.mentions == 1 {
        i18n.t("search.transcripts_one_mention")
    } else {
        i18n.t("search.transcripts_mentions").replace("{count}", &hit.mentions.to_string())
    };
    let art = if ep.episodeartwork.is_empty() { ep.artworkurl.clone() } else { ep.episodeartwork.clone() };
    let date = if ep.episodepubdate.is_empty() { String::new() } else { format_date(&ep.episodepubdate) };

    html! {
        <article class="ts-hit" key={id}>
            <button class="ts-hit-art" onclick={open_episode.clone()} aria-label={ep.episodetitle.clone()}>
                <FallbackImage src={art} alt={ep.episodetitle.clone()} class="ts-hit-art-img" />
            </button>
            <div class="ts-hit-body">
                <button class="ts-hit-title" onclick={open_episode}>{ &ep.episodetitle }</button>
                <div class="ts-hit-meta">
                    <span>{ &ep.podcastname }</span>
                    if !date.is_empty() { <span>{ date }</span> }
                    <span>{ mentions }</span>
                    <span class={classes!("ts-source", (hit.source == "generated").then_some("is-ai"))}>{ source_label }</span>
                </div>
                <ul class="ts-moments">
                    { for hit.moments.iter().take(shown).map(|m| {
                        let time = m.start.map(|t| {
                            let ep = ep.clone();
                            let onclick = Callback::from(move |e: MouseEvent| play_from(ep.clone(), t, e));
                            html! {
                                <button class="ts-moment-time" {onclick} title={i18n.t("search.transcripts_play_from")}>
                                    <i class="ph-fill ph-play"></i>{ format_time_rm_hour(t as i32) }
                                </button>
                            }
                        });
                        html! {
                            <li class="ts-moment">
                                { time.unwrap_or_else(|| html! { <span class="ts-moment-quote"><i class="ph ph-quotes"></i></span> }) }
                                <span class="ts-moment-text">{ highlight_terms(&m.text, terms) }</span>
                            </li>
                        }
                    }) }
                </ul>
                if hidden > 0 || is_open && hit.moments.len() > FOLDED_MOMENTS {
                    <button class="ts-more" onclick={toggle}>
                        if is_open {
                            <i class="ph ph-caret-up"></i>{ i18n.t("search.transcripts_fewer_moments") }
                        } else {
                            <i class="ph ph-caret-down"></i>{ i18n.t("search.transcripts_more_moments").replace("{count}", &hidden.to_string()) }
                        }
                    </button>
                }
            </div>
        </article>
    }
}
