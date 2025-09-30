use super::app_drawer::App_drawer;
use super::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use super::routes::Route;
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, UIState};
use crate::components::gen_components::on_shownotes_click;
use crate::components::gen_components::ContextButton;
use crate::components::gen_components::EpisodeTrait;
use crate::components::gen_funcs::{format_datetime, format_time, match_date_format, parse_date};
use crate::requests::pod_req;
use crate::requests::pod_req::{HomeEpisode, Playlist};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yew_router::prelude::Link;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[derive(Properties, PartialEq, Clone)]
struct QuickLinkProps {
    route: Route,
    icon: &'static str,
    label: String,
}

#[function_component(QuickLink)]
fn quick_link(props: &QuickLinkProps) -> Html {
    html! {
        <Link<Route> to={props.route.clone()} classes="quick-link-card rounded-lg">
            <i class={classes!("ph", props.icon)}></i>
            <span>{ props.label.clone() }</span>
        </Link<Route>>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct PlaylistCardProps {
    pub playlist: Playlist,
    pub onclick: Callback<MouseEvent>,
}

#[function_component(PlaylistCard)]
fn playlist_card(props: &PlaylistCardProps) -> Html {
    html! {
        <div class="playlist-card" onclick={props.onclick.clone()}>
            <div class="playlist-card-stack">
                <div class="playlist-card-content">
                    <i class={classes!("ph", props.playlist.icon_name.clone(), "playlist-icon")}></i>
                    <div class="playlist-info">
                        <h3 class="playlist-title">{&props.playlist.name}</h3>
                        <span class="playlist-count">
                            {format!("{} episodes", props.playlist.episode_count.unwrap_or(0))}
                        </span>
                        if let Some(description) = &props.playlist.description {
                            <p class="playlist-description">{description}</p>
                        }
                    </div>
                </div>
            </div>
        </div>
    }
}

#[function_component(Home)]
pub fn home() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let loading = use_state(|| true);
    let history = BrowserHistory::new();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    
    // Capture i18n strings before they get moved
    let i18n_quick_links = i18n.t("home.quick_links").to_string();
    let i18n_saved = i18n.t("app_drawer.saved").to_string();
    let i18n_downloads = i18n.t("app_drawer.downloads").to_string();
    let i18n_queue = i18n.t("app_drawer.queue").to_string();
    let i18n_history = i18n.t("app_drawer.history").to_string();
    let i18n_feed = i18n.t("app_drawer.feed").to_string();
    let i18n_podcasts = i18n.t("app_drawer.podcasts").to_string();
    let i18n_continue_listening = i18n.t("home.continue_listening").to_string();
    let i18n_top_podcasts = i18n.t("home.top_podcasts").to_string();
    let i18n_smart_playlists = i18n.t("home.smart_playlists").to_string();
    let i18n_no_playlists_available = i18n.t("home.no_playlists_available").to_string();
    let i18n_recent_episodes = i18n.t("home.recent_episodes").to_string();
    let i18n_no_recent_episodes = i18n.t("home.no_recent_episodes").to_string();
    let i18n_welcome_to_pinepods = i18n.t("home.welcome_to_pinepods").to_string();
    let i18n_welcome_description = i18n.t("home.welcome_description").to_string();
    let i18n_no_description_provided = i18n.t("home.no_description_provided").to_string();
    let i18n_unknown_author = i18n.t("home.unknown_author").to_string();
    let i18n_no_categories_found = i18n.t("home.no_categories_found").to_string();
    let i18n_no_website_provided = i18n.t("home.no_website_provided").to_string();
    let i18n_completed = i18n.t("downloads.completed").to_string();
    let i18n_episodes = i18n.t("home.episodes").to_string();

    // Fetch home overview data
    let effect_dispatch = dispatch.clone();
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
                                    .chain(home_data.in_progress_episodes.iter());

                                // Extract episode state information
                                let completed_episode_ids: Vec<i32> = all_episodes
                                    .clone()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();

                                let saved_episode_ids: Vec<i32> = all_episodes
                                    .clone()
                                    .filter(|ep| ep.saved)
                                    .map(|ep| ep.episodeid)
                                    .collect();

                                let queued_episode_ids: Vec<i32> = all_episodes
                                    .clone()
                                    .filter(|ep| ep.queued)
                                    .map(|ep| ep.episodeid)
                                    .collect();

                                let downloaded_episode_ids: Vec<i32> = all_episodes
                                    .clone()
                                    .filter(|ep| ep.downloaded)
                                    .map(|ep| ep.episodeid)
                                    .collect();

                                effect_dispatch.reduce_mut(move |state| {
                                    state.home_overview = Some(home_data);

                                    // Update state collections with merged data
                                    state.completed_episodes = Some(completed_episode_ids);
                                    state.saved_episode_ids = Some(saved_episode_ids);
                                    state.queued_episode_ids = Some(queued_episode_ids);
                                    state.downloaded_episode_ids = Some(downloaded_episode_ids);
                                });
                                loading.set(false);
                            }
                            Err(e) => {
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
        let dispatch = dispatch.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_playlists(&server_name, &api_key.unwrap(), user_id)
                            .await
                        {
                            Ok(playlist_response) => {
                                dispatch.reduce_mut(move |state| {
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

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />

            if *loading {
                <div class="loading-animation">
                    <div class="frame1"></div>
                    <div class="frame2"></div>
                    <div class="frame3"></div>
                    <div class="frame4"></div>
                    <div class="frame5"></div>
                    <div class="frame6"></div>
                </div>
            } else {
                if let Some(home_data) = &state.home_overview {
                    <div class="space-y-8">
                        // Quick Links Section
                        <div class="section-container">
                            <h2 class="text-2xl font-bold mb-4 item_container-text">{&i18n_quick_links}</h2>
                            <div class="grid grid-cols-3 gap-2 md:gap-4">
                                <QuickLink route={Route::Saved} icon="ph-star" label={i18n_saved.clone()} />
                                <QuickLink route={Route::Downloads} icon="ph-download-simple" label={i18n_downloads.clone()} />
                                <QuickLink route={Route::Queue} icon="ph-queue" label={i18n_queue.clone()} />
                                <QuickLink route={Route::PodHistory} icon="ph-clock-counter-clockwise" label={i18n_history.clone()} />
                                <QuickLink route={Route::Feed} icon="ph-bell-ringing" label={i18n_feed.clone()} />
                                <QuickLink route={Route::Podcasts} icon="ph-microphone-stage" label={i18n_podcasts.clone()} />
                            </div>
                        </div>

                        {
                            if !home_data.in_progress_episodes.is_empty() {
                                html! {
                                    <div class="section-container">
                                        <h2 class="text-2xl font-bold mb-4 item_container-text">{&i18n_continue_listening}</h2>
                                        <div class="space-y-4">
                                            { for home_data.in_progress_episodes.iter().take(3).map(|episode| {
                                                html! {
                                                    <HomeEpisodeItem
                                                        episode={episode.clone()}
                                                        page_type="home"
                                                    />
                                                }
                                            })}
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }

                        // Top Podcasts Section
                        // In your top podcasts section, replace the existing podcast grid items with:
                        <h2 class="text-2xl font-bold mb-4 item_container-text">{&i18n_top_podcasts}</h2>
                        <div class="podcast-grid">
                            { for home_data.top_podcasts.iter().map(|podcast| {
                                let api_key_clone = api_key.clone();
                                let server_name_clone = server_name.clone();
                                let history_clone = history.clone();
                                let dispatch_clone = dispatch.clone();

                                let on_title_click = create_on_title_click(
                                    dispatch_clone,
                                    server_name_clone.unwrap_or_default(),
                                    api_key_clone,
                                    &history_clone,
                                    podcast.podcastindexid.unwrap_or_default(),
                                    podcast.podcastname.clone(),
                                    podcast.feedurl.clone().unwrap_or_default(),
                                    podcast.description.clone().unwrap_or_else(|| i18n_no_description_provided.clone()),
                                    podcast.author.clone().unwrap_or_else(|| i18n_unknown_author.clone()),
                                    podcast.artworkurl.clone().unwrap_or_default(),
                                    podcast.explicit.unwrap_or(false),
                                    podcast.episodecount.unwrap_or(0),
                                    podcast.categories.as_ref().map(|cats| cats.values().cloned().collect::<Vec<_>>().join(", ")).or_else(|| Some(i18n_no_categories_found.clone())),
                                    podcast.websiteurl.clone().unwrap_or_else(|| i18n_no_website_provided.clone()),
                                    user_id.unwrap(),
                                    podcast.is_youtube,
                                );

                                html! {
                                    <div class="podcast-grid-item" onclick={on_title_click}>
                                        <div class="podcast-image-container">
                                            <FallbackImage
                                                src={podcast.artworkurl.clone().unwrap_or_default()}
                                                alt={format!("Cover for {}", podcast.podcastname)}
                                                class="podcast-image"
                                            />
                                        </div>
                                        <div class="podcast-info">
                                            <h3 class="podcast-title-grid">{&podcast.podcastname}</h3>
                                        </div>
                                    </div>
                                }
                            })}
                        </div>

                        <div class="section-container">
                            <h2 class="text-2xl font-bold mb-4 item_container-text">{&i18n_smart_playlists}</h2>
                            if let Some(playlists) = &state.playlists {
                                <div class="grid grid-cols-1 xs:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
                                    {
                                        playlists.iter().map(|playlist| {
                                            let history_clone = history.clone();
                                            let playlist_id = playlist.playlist_id;
                                            let onclick = Callback::from(move |_| {
                                                let url = format!("/playlist/{}", playlist_id);
                                                history_clone.push(url);
                                            });
                                            html! {
                                                <PlaylistCard
                                                    playlist={playlist.clone()}
                                                    {onclick}
                                                />
                                            }
                                        }).collect::<Html>()
                                    }
                                </div>
                            } else {
                                <p class="item_container-text">{&i18n_no_playlists_available}</p>
                            }
                        </div>

                        // Recent Episodes Section
                        <div class="section-container">
                            <h2 class="text-2xl font-bold mb-4 item_container-text">{&i18n_recent_episodes}</h2>
                            if home_data.recent_episodes.is_empty() {
                                <p class="item_container-text">{&i18n_no_recent_episodes}</p>
                            } else {
                                <div class="space-y-4">
                                    { for home_data.recent_episodes.iter().take(5).map(|episode| {
                                        html! {
                                            <HomeEpisodeItem
                                                episode={episode.clone()}
                                                page_type="home"
                                            />
                                        }
                                    })}
                                </div>
                            }
                        </div>
                        <div class="h-30"></div>
                    </div>
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
                />
            }
        </div>
        <App_drawer />
        </>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct HomeEpisodeItemProps {
    pub episode: HomeEpisode,
    pub page_type: String,
}

#[function_component(HomeEpisodeItem)]
pub fn home_episode_item(props: &HomeEpisodeItemProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let history = BrowserHistory::new();
    let should_show_buttons = !props.episode.episodeurl.is_empty();
    let episode: Box<dyn EpisodeTrait> = Box::new(props.episode.clone());
    let listen_duration = props.episode.listenduration.unwrap_or(0);
    let total_duration = props.episode.episodeduration;

    let completed = props.episode.completed
        || state
            .completed_episodes
            .as_ref()
            .unwrap_or(&vec![])
            .contains(&props.episode.episodeid);

    let progress_percentage = if total_duration > 0 {
        ((listen_duration as f64 / total_duration as f64) * 100.0).min(100.0)
    } else {
        0.0
    };

    // Format durations for display
    let formatted_duration = format_time(total_duration as f64);
    let duration_clone = formatted_duration.clone();
    let duration_again = formatted_duration.clone();

    // Format listen duration if it exists
    let formatted_listen_duration = if listen_duration > 0 {
        Some(format_time(listen_duration as f64))
    } else {
        None
    };

    let date_format = match_date_format(state.date_format.as_deref());
    let datetime = parse_date(&props.episode.episodepubdate, &state.user_tz);
    let formatted_date = format!(
        "{}",
        format_datetime(&datetime, &state.hour_preference, date_format)
    );

    let on_play_pause = on_play_pause(
        props.episode.episodeurl.clone(),
        props.episode.episodetitle.clone(),
        props.episode.episodedescription.clone(),
        formatted_date.clone(),
        props.episode.episodeartwork.clone(),
        props.episode.episodeduration,
        props.episode.episodeid,
        props.episode.listenduration,
        api_key.unwrap().unwrap(),
        user_id.unwrap(),
        server_name.unwrap(),
        audio_dispatch.clone(),
        audio_state.clone(),
        None,
        Some(props.episode.is_youtube.clone()),
    );

    let on_shownotes_click = {
        on_shownotes_click(
            history.clone(),
            dispatch.clone(),
            Some(props.episode.episodeid),
            Some(props.page_type.clone()),
            Some(props.page_type.clone()),
            Some(props.page_type.clone()),
            true,
            None,
            Some(props.episode.is_youtube.clone()),
        )
    };

    let is_current_episode = audio_state
        .currently_playing
        .as_ref()
        .map_or(false, |current| {
            current.episode_id == props.episode.episodeid
        });

    let is_playing = audio_state.audio_playing.unwrap_or(false);

    html! {
        <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg">
            <div class="flex flex-col w-auto object-cover pl-4">
                <FallbackImage
                    src={props.episode.episodeartwork.clone()}
                    alt={format!("Cover for {}", props.episode.episodetitle)}
                    class="episode-image"
                />
            </div>
            <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                <div class="flex items-center space-x-2 cursor-pointer" onclick={on_shownotes_click.clone()}>
                    <p class="item_container-text episode-title font-semibold line-clamp-2">
                        { &props.episode.episodetitle }
                    </p>
                    {
                        if completed.clone() {
                            html! {
                                <i class="ph ph-check-circle text-2xl text-green-500"></i>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
                <p class="item_container-text text-sm">{ &props.episode.podcastname }</p>
                <div class="episode-time-badge-container" style="max-width: 100%; overflow: hidden;">
                    <span
                        class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2"
                        style="flex-grow: 0; flex-shrink: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
                    >
                        <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                            <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                        </svg>
                        { formatted_date }
                    </span>
                </div>
                {
                    if completed {
                        html! {
                            <div class="flex items-center space-x-2">
                                <span class="item_container-text">{ duration_clone }</span>
                                <span class="item_container-text">{ "-  Completed" }</span>
                            </div>
                        }
                    } else {
                        if formatted_listen_duration.is_some() {
                            html! {
                                <div class="flex items-center space-x-2">
                                    <span class="item_container-text">{ formatted_listen_duration.clone() }</span>
                                    <div class="progress-bar-container">
                                        <div class="progress-bar" style={ format!("width: {}%;", progress_percentage) }></div>
                                    </div>
                                    <span class="item_container-text">{ duration_again }</span>
                                </div>
                            }
                        } else {
                            html! {
                                <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                            }
                        }
                    }
                }
            </div>
            <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                if should_show_buttons {
                    <button
                        class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                        onclick={on_play_pause}
                    >
                        {
                            if is_current_episode && is_playing {
                                html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                            } else {
                                html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                            }
                        }
                    </button>
                    <div class="hidden sm:block"> // This will hide the context button below 640px
                        <ContextButton episode={episode.clone()} page_type={"home".to_string()} />
                    </div>
                }
            </div>
        </div>
    }
}
