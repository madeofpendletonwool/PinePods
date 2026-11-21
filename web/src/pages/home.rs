use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{
    AppState, EpisodeStatusState, HomePageState, PlaylistDataState, UIState,
};
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::home_cards::{ContinueHero, EpisodeTile, Shelf};
use crate::components::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::components::page_cache;
use crate::components::setting_components::nextcloud_options::SyncNoticeBanner;
use crate::pages::routes::Route;
use crate::requests::episode::Episode;
use crate::requests::pod_req::Playlist;
use crate::requests::pod_req::{self};

use gloo_timers::future::TimeoutFuture;
use crate::components::i18n::use_translation;
use std::collections::HashSet;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yew_router::prelude::Link;
use yewdux::prelude::*;

#[derive(Properties, PartialEq, Clone)]
struct HomeStripProps {
    seconds_listened: i64,
    episodes_completed: i32,
    downloaded_count: i32,
}

/// One line under the hero: this week's listening, plus a few shortcuts.
#[function_component(HomeStrip)]
fn home_strip(props: &HomeStripProps) -> Html {
    let (i18n, _) = use_translation();
    let secs = props.seconds_listened;
    let (hours, mins) = (secs / 3600, (secs % 3600) / 60);
    let time_str = if hours > 0 { format!("{hours}h {mins}m") } else { format!("{mins}m") };

    html! {
        <div class="home-strip">
            <Link<Route> to={Route::UserStats} classes="home-week">
                <span class="home-week-label">{ i18n.t("home.this_week") }</span>
                <span class="home-week-stat"><i class="ph ph-clock"></i>{ time_str }<span>{ i18n.t("home.listened") }</span></span>
                <span class="home-week-stat"><i class="ph ph-check-circle"></i>{ props.episodes_completed }<span>{ i18n.t("home.completed") }</span></span>
            </Link<Route>>
            <div class="home-chips">
                <Link<Route> to={Route::Feed} classes="home-chip">
                    <i class="ph ph-bell-ringing"></i>{ i18n.t("app_drawer.feed") }
                </Link<Route>>
                <Link<Route> to={Route::Downloads} classes="home-chip">
                    <i class="ph ph-download-simple"></i>{ i18n.t("app_drawer.downloads") }
                    if props.downloaded_count > 0 {
                        <span class="home-chip-count">{ props.downloaded_count }</span>
                    }
                </Link<Route>>
                <Link<Route> to={Route::PodHistory} classes="home-chip">
                    <i class="ph ph-clock-counter-clockwise"></i>{ i18n.t("app_drawer.history") }
                </Link<Route>>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
struct ContinueListeningProps {
    episodes: Vec<Episode>,
}

/// Renders the home "Continue Listening" section. Shows up to 3 in-progress episodes and
/// reactively removes any that get marked complete (via the shared
/// `EpisodeStatusState.completed_episodes` set), animating the card out and sliding the next
/// queued episode into its place. Hides itself when nothing is left to show.
#[function_component(ContinueListening)]
fn continue_listening(props: &ContinueListeningProps) -> Html {

    let completed = use_selector(|state: &EpisodeStatusState| state.completed_episodes.clone());
    // Ids mid-exit-animation (still mounted) and ids fully removed for this session.
    let leaving = use_state(HashSet::<i32>::new);
    let removed = use_state(HashSet::<i32>::new);

    // Reset local state whenever the underlying list changes (e.g. a fresh home fetch),
    // so a refreshed list shows in full again.
    {
        let leaving = leaving.clone();
        let removed = removed.clone();
        use_effect_with(props.episodes.clone(), move |_| {
            leaving.set(HashSet::new());
            removed.set(HashSet::new());
            || ()
        });
    }

    // The currently-visible window: first 3 episodes that haven't been fully removed yet.
    // A "leaving" episode is not yet in `removed`, so it keeps its slot during the exit
    // animation and the 4th card only surfaces once the animation completes.
    let visible: Vec<Episode> = props
        .episodes
        .iter()
        .filter(|ep| !removed.contains(&ep.episodeid))
        .take(3)
        .cloned()
        .collect();

    // When a visible episode becomes completed, animate it out then remove it.
    {
        let leaving = leaving.clone();
        let removed_state = removed.clone();
        let visible_ids: Vec<i32> = visible.iter().map(|ep| ep.episodeid).collect();
        use_effect_with(
            (completed.clone(), (*removed).clone()),
            move |(completed, _removed)| {
                // Collect every newly-completed visible id first, then apply a single
                // state update, so simultaneous completions don't clobber each other.
                let newly_leaving: Vec<i32> = visible_ids
                    .into_iter()
                    .filter(|id| {
                        completed.contains(id)
                            && !leaving.contains(id)
                            && !removed_state.contains(id)
                    })
                    .collect();

                if !newly_leaving.is_empty() {
                    let mut next_leaving = (*leaving).clone();
                    for id in &newly_leaving {
                        next_leaving.insert(*id);
                    }
                    leaving.set(next_leaving);

                    for id in newly_leaving {
                        let leaving = leaving.clone();
                        let removed_state = removed_state.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(400).await;
                            let mut next_removed = (*removed_state).clone();
                            next_removed.insert(id);
                            removed_state.set(next_removed);
                            let mut next_leaving = (*leaving).clone();
                            next_leaving.remove(&id);
                            leaving.set(next_leaving);
                        });
                    }
                }
                || ()
            },
        );
    }

    let Some((first, rest)) = visible.split_first() else {
        return html! {};
    };
    let leaving_class = |ep: &Episode| leaving.contains(&ep.episodeid).then_some("leaving");

    html! {
        <div class="home-continue">
            <div key={first.episodeid} class={classes!("continue-item", leaving_class(first))}>
                <ContinueHero episode={first.clone()} />
            </div>
            if !rest.is_empty() {
                <div class="continue-list">
                    { for rest.iter().map(|episode| html! {
                        <div key={episode.episodeid} class={classes!("continue-item", leaving_class(episode))}>
                            <EpisodeListItem episode={ episode.clone() } />
                        </div>
                    })}
                </div>
            }
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
struct WelcomeTipProps {
    icon: &'static str,
    title: String,
    description: String,
}

#[function_component(WelcomeTip)]
fn welcome_tip(props: &WelcomeTipProps) -> Html {
    html! {
        <div class="welcome-tip-card">
            <div class="welcome-tip-icon">
                <i class={classes!("ph", props.icon)}></i>
            </div>
            <div class="welcome-tip-text">
                <h3 class="welcome-tip-title">{ props.title.clone() }</h3>
                <p class="welcome-tip-desc">{ props.description.clone() }</p>
            </div>
        </div>
    }
}

#[function_component(WelcomeHome)]
fn welcome_home() -> Html {
    let (i18n, _) = use_translation();

    let hero_subtitle = i18n.t("home.welcome_hero_subtitle").to_string();
    let intro = i18n.t("home.welcome_intro").to_string();
    let get_started = i18n.t("home.welcome_get_started").to_string();
    let docs_button = i18n.t("home.welcome_docs_button").to_string();
    let tips_heading = i18n.t("home.welcome_tips_heading").to_string();
    let tip_search_title = i18n.t("home.welcome_tip_search_title").to_string();
    let tip_search_desc = i18n.t("home.welcome_tip_search_desc").to_string();
    let tip_feed_title = i18n.t("home.welcome_tip_feed_title").to_string();
    let tip_feed_desc = i18n.t("home.welcome_tip_feed_desc").to_string();
    let tip_docs_title = i18n.t("home.welcome_tip_docs_title").to_string();
    let tip_docs_desc = i18n.t("home.welcome_tip_docs_desc").to_string();
    let welcome_title = i18n.t("home.welcome_to_pinepods").to_string();
    let open_podcasts = i18n.t("home.welcome_open_podcasts").to_string();

    html! {
        <div class="welcome-container">
            <div class="welcome-hero">
                <img src="static/assets/favicon.png" alt="Pinepods" class="welcome-logo" />
                <h1 class="welcome-title">{ welcome_title }</h1>
                <p class="welcome-subtitle">{ hero_subtitle }</p>
                <p class="welcome-intro">{ intro }</p>
                <div class="welcome-actions">
                    <a
                        href="https://pinepods.online/docs/intro"
                        target="_blank"
                        rel="noopener noreferrer"
                        class="welcome-btn welcome-btn-secondary"
                    >
                        <i class="ph ph-book"></i>
                        <span>{ docs_button }</span>
                    </a>
                    <Link<Route> to={Route::Podcasts} classes="welcome-btn welcome-btn-secondary">
                        <i class="ph ph-microphone-stage"></i>
                        <span>{ open_podcasts }</span>
                    </Link<Route>>
                </div>
            </div>

            <div class="welcome-tips">
                <h2 class="welcome-tips-heading">{ format!("{} \u{2014} {}", get_started, tips_heading) }</h2>
                <div class="welcome-tips-grid">
                    <WelcomeTip
                        icon="ph-magnifying-glass"
                        title={tip_search_title}
                        description={tip_search_desc}
                    />
                    <WelcomeTip
                        icon="ph-rss"
                        title={tip_feed_title}
                        description={tip_feed_desc}
                    />
                    <WelcomeTip
                        icon="ph-binoculars"
                        title={tip_docs_title}
                        description={tip_docs_desc}
                    />
                </div>
            </div>
        </div>
    }
}

#[function_component(Home)]
pub fn home() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let (home_state, home_dispatch) = use_store::<HomePageState>();
    let (playlist_state, playlist_dispatch) = use_store::<PlaylistDataState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    // Data from an earlier visit renders straight away; the fetch below refreshes it.
    let loading = use_state(|| home_state.home_overview.is_none());
    let history = BrowserHistory::new();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    // Capture i18n strings before they get moved
    let i18n_collections = i18n.t("navigation.collections").to_string();
    let i18n_top_podcasts = i18n.t("home.top_podcasts").to_string();
    let i18n_smart_playlists = i18n.t("home.smart_playlists").to_string();
    let i18n_recent_episodes = i18n.t("home.recent_episodes").to_string();
    let i18n_episode_count = i18n.t("home.episode_count").to_string();
    let i18n_welcome_to_pinepods = i18n.t("home.welcome_to_pinepods").to_string();
    let i18n_welcome_description = i18n.t("home.welcome_description").to_string();
    let i18n_no_description_provided = i18n.t("home.no_description_provided").to_string();
    let i18n_unknown_author = i18n.t("home.unknown_author").to_string();
    let i18n_no_categories_found = i18n.t("home.no_categories_found").to_string();
    let i18n_no_website_provided = i18n.t("home.no_website_provided").to_string();

    // Fetch home overview data
    let effect_dispatch = home_dispatch.clone();
    {
        let loading = loading.clone();
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let cache_key = page_cache::key("home", &server_name, user_id);
                    let network_done = std::rc::Rc::new(std::cell::Cell::new(false));
                    if effect_dispatch.get().home_overview.is_none() {
                        let cache_key = cache_key.clone();
                        let network_done = network_done.clone();
                        let loading = loading.clone();
                        let dispatch = effect_dispatch.clone();
                        spawn_local(async move {
                            if let Some(cached) =
                                page_cache::stored::<pod_req::HomeOverview>(&cache_key).await
                            {
                                if !network_done.get() && dispatch.get().home_overview.is_none() {
                                    let completed: std::collections::HashSet<i32> = cached
                                        .recent_episodes
                                        .iter()
                                        .chain(cached.in_progress_episodes.iter())
                                        .chain(cached.queue_preview.iter())
                                        .filter(|ep| ep.completed)
                                        .map(|ep| ep.episodeid)
                                        .collect();
                                    Dispatch::<EpisodeStatusState>::global()
                                        .reduce_mut(move |state| state.completed_episodes = completed);
                                    dispatch.reduce_mut(move |state| {
                                        state.home_overview = Some(cached);
                                    });
                                    loading.set(false);
                                }
                            }
                        });
                    }
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_home_overview(
                            &server_name,
                            &api_key.unwrap(),
                            user_id,
                        )
                        .await
                        {
                            Ok(home_data) => {
                                // Collect all episodes from the various sections
                                let all_episodes = home_data
                                    .recent_episodes
                                    .iter()
                                    .chain(home_data.in_progress_episodes.iter())
                                    .chain(home_data.queue_preview.iter());

                                // Extract episode state information
                                let completed_episode_ids: std::collections::HashSet<i32> =
                                    all_episodes
                                        .clone()
                                        .filter(|ep| ep.completed)
                                        .map(|ep| ep.episodeid)
                                        .collect();

                                let saved_episodes: Vec<Episode> = all_episodes
                                    .clone()
                                    .filter(|ep| ep.saved)
                                    .map(|e| e.to_owned())
                                    .collect();

                                let queued_episode_ids: Vec<i32> = all_episodes
                                    .clone()
                                    .filter(|ep| ep.queued)
                                    .map(|ep| ep.episodeid)
                                    .collect();

                                let downloaded_episodes: Vec<Episode> = all_episodes
                                    .filter(|ep| ep.downloaded)
                                    .map(|ep| ep.clone())
                                    .collect();

                                network_done.set(true);
                                page_cache::put(&cache_key, &home_data);
                                effect_dispatch.reduce_mut(move |state| {
                                    state.home_overview = Some(home_data);
                                });
                                Dispatch::<EpisodeStatusState>::global().reduce_mut(move |state| {
                                    state.completed_episodes = completed_episode_ids;
                                    state.saved_episodes = saved_episodes;
                                    state.queued_episode_ids = Some(queued_episode_ids);
                                    state.downloaded_episodes.clear_server();
                                    for ep in downloaded_episodes {
                                        state.downloaded_episodes.push_server(ep);
                                    }
                                });
                                loading.set(false);
                            }
                            Err(e) => {
                                network_done.set(true);
                                web_sys::console::log_1(
                                    &format!("Error fetching home data: {:?}", e).into(),
                                );
                                loading.set(false);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    {
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        let playlist_dispatch = playlist_dispatch.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let cache_key = page_cache::key("home_playlists", &server_name, user_id);
                    if playlist_dispatch.get().playlists.is_none() {
                        let cache_key = cache_key.clone();
                        let dispatch = playlist_dispatch.clone();
                        spawn_local(async move {
                            if let Some(cached) =
                                page_cache::stored::<Vec<Playlist>>(&cache_key).await
                            {
                                if dispatch.get().playlists.is_none() {
                                    dispatch.reduce_mut(move |state| state.playlists = Some(cached));
                                }
                            }
                        });
                    }
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_playlists(&server_name, &api_key.unwrap(), user_id)
                            .await
                        {
                            Ok(playlist_response) => {
                                page_cache::put(&cache_key, &playlist_response.playlists);
                                playlist_dispatch.reduce_mut(move |state| {
                                    state.playlists = Some(playlist_response.playlists);
                                });
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error fetching playlists: {:?}", e).into(),
                                );
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    // Collections row: cached copy first, then the server's.
    let collections = {
        let initial = match (&server_name, user_id) {
            (Some(server), Some(uid)) => {
                page_cache::memory::<Vec<pod_req::Collection>>(&page_cache::key("home_collections", server, uid))
            }
            _ => None,
        };
        use_state(move || initial.unwrap_or_default())
    };
    {
        let collections = collections.clone();
        use_effect_with(
            (api_key.clone(), user_id, server_name.clone()),
            move |(api_key, user_id, server_name)| {
                if let (Some(Some(api_key)), Some(user_id), Some(server_name)) =
                    (api_key.clone(), *user_id, server_name.clone())
                {
                    let cache_key = page_cache::key("home_collections", &server_name, user_id);
                    spawn_local(async move {
                        if collections.is_empty() {
                            if let Some(cached) = page_cache::stored::<Vec<pod_req::Collection>>(&cache_key).await {
                                collections.set(cached);
                            }
                        }
                        if let Ok(cols) = pod_req::call_get_collections(&server_name, &api_key, user_id).await {
                            page_cache::put(&cache_key, &cols);
                            collections.set(cols);
                        }
                    });
                }
                || ()
            },
        );
    }

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            <SyncNoticeBanner />

            if *loading {
                {
                    html! { <Loading/> }
                }
            } else {
                if let Some(home_data) = &home_state.home_overview {
                    if home_data.top_podcasts.is_empty() {
                        // Zero podcasts added — greet the user with the welcome page
                        <WelcomeHome />
                    } else {
                    <div class="home-page">
                        <ContinueListening episodes={home_data.in_progress_episodes.clone()} />

                        <HomeStrip
                            seconds_listened={home_data.weekly_stats.seconds_listened}
                            episodes_completed={home_data.weekly_stats.episodes_completed}
                            downloaded_count={home_data.downloaded_count}
                        />

                        // Newest episodes from subscriptions, minus what's already above
                        {{
                            let in_progress: HashSet<i32> = home_data
                                .in_progress_episodes
                                .iter()
                                .take(3)
                                .map(|e| e.episodeid)
                                .collect();
                            let recent: Vec<&Episode> = home_data
                                .recent_episodes
                                .iter()
                                .filter(|e| !in_progress.contains(&e.episodeid))
                                .take(12)
                                .collect();
                            if recent.is_empty() {
                                html! {}
                            } else {
                                html! {
                                    <Shelf title={i18n_recent_episodes.clone()} see_all={Some(Route::Feed)}>
                                        { for recent.into_iter().map(|episode| html! {
                                            <EpisodeTile key={episode.episodeid} episode={episode.clone()} />
                                        })}
                                    </Shelf>
                                }
                            }
                        }}

                        <Shelf title={i18n_top_podcasts.clone()} see_all={Some(Route::Podcasts)} class="home-podcast-row">
                            { for home_data.top_podcasts.iter().map(|podcast| {
                                let on_title_click = create_on_title_click(
                                    server_name.clone().unwrap_or_default(),
                                    api_key.clone(),
                                    &history,
                                    podcast.podcastid,
                                    podcast.podcastindexid.unwrap_or(0),
                                    podcast.podcastname.clone(),
                                    podcast.feedurl.clone().unwrap_or_default(),
                                    podcast.description.clone().unwrap_or_else(|| i18n_no_description_provided.clone()),
                                    podcast.author.clone().unwrap_or_else(|| i18n_unknown_author.clone()),
                                    podcast.artworkurl.clone().unwrap_or_default(),
                                    podcast.explicit.unwrap_or(false),
                                    podcast.episodecount.unwrap_or(0),
                                    podcast.categories.as_ref().map(|cats| cats.values().cloned().collect::<Vec<_>>().join(", ")).or_else(|| Some(i18n_no_categories_found.clone())),
                                    podcast.websiteurl.clone().unwrap_or_else(|| i18n_no_website_provided.clone()),
                                    user_id.unwrap_or_default(),
                                    podcast.is_youtube,
                                );
                                html! {
                                    <button key={podcast.podcastid} class="home-podcast" onclick={on_title_click} title={podcast.podcastname.clone()}>
                                        <FallbackImage size={Some(256)}
                                            src={podcast.artworkurl.clone().unwrap_or_default()}
                                            alt={format!("Cover for {}", podcast.podcastname)}
                                            class="home-podcast-img"
                                        />
                                        <span class="home-podcast-name">{ &podcast.podcastname }</span>
                                    </button>
                                }
                            })}
                        </Shelf>

                        if !collections.is_empty() {
                            <Shelf title={i18n_collections.clone()} see_all={Some(Route::Collections)} class="home-card-row">
                                { for collections.iter().map(|c| {
                                    let history = history.clone();
                                    let id = c.collection_id;
                                    let onclick = Callback::from(move |_| history.push(format!("/collections?id={id}")));
                                    html! {
                                        <button key={c.collection_id} class="home-card" {onclick}>
                                            <i class={classes!("ph", c.icon.clone(), "home-card-icon")}></i>
                                            <span class="home-card-title">{ &c.name }</span>
                                            <span class="home-card-sub">{ i18n_episode_count.replace("{count}", &c.episode_count.to_string()) }</span>
                                        </button>
                                    }
                                })}
                            </Shelf>
                        }

                        if let Some(playlists) = playlist_state.playlists.as_ref().filter(|p| !p.is_empty()) {
                            <Shelf title={i18n_smart_playlists.clone()} see_all={Some(Route::Playlists)} class="home-card-row">
                                { for playlists.iter().map(|playlist| {
                                    let history = history.clone();
                                    let id = playlist.playlist_id;
                                    let onclick = Callback::from(move |_| history.push(format!("/playlist/{id}")));
                                    html! {
                                        <button key={playlist.playlist_id} class="home-card" {onclick}>
                                            <i class={classes!("ph", playlist.icon_name.clone(), "home-card-icon")}></i>
                                            <span class="home-card-title">{ &playlist.name }</span>
                                            <span class="home-card-sub">{ i18n_episode_count.replace("{count}", &playlist.episode_count.unwrap_or(0).to_string()) }</span>
                                        </button>
                                    }
                                })}
                            </Shelf>
                        }
                        <div class="h-30"></div>
                    </div>
                    }
                } else {
                    { empty_message(
                        &i18n_welcome_to_pinepods,
                        &i18n_welcome_description
                    )}
                }
            }

            // Audio Player
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
