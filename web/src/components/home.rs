use super::app_drawer::App_drawer;
use super::gen_components::{empty_message, Search_nav, UseScrollToTop};
use super::routes::Route;
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, UIState};
use crate::components::gen_components::on_shownotes_click;
use crate::components::gen_funcs::{format_datetime, match_date_format, parse_date};
use crate::requests::pod_req;
use crate::requests::pod_req::{HomeEpisode, Playlist};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yew_router::prelude::use_navigator;
use yew_router::prelude::Link;
use yewdux::prelude::*;

#[derive(Properties, PartialEq, Clone)]
struct QuickLinkProps {
    route: Route,
    icon: &'static str,
    label: &'static str,
}

#[function_component(QuickLink)]
fn quick_link(props: &QuickLinkProps) -> Html {
    html! {
        <Link<Route> to={props.route.clone()} classes="quick-link-card rounded-lg">
            <i class={classes!("ph", props.icon)}></i>
            <span>{ props.label }</span>
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
                        <span class="playlist-count">{format!("{} episodes", props.playlist.episode_count)}</span>
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
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let loading = use_state(|| true);
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();
    let history = BrowserHistory::new();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

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
                                effect_dispatch.reduce_mut(move |state| {
                                    state.home_overview = Some(home_data);
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
                            <h2 class="text-2xl font-bold mb-4 item_container-text">{"Quick Links"}</h2>
                            <div class="grid grid-cols-3 gap-2 md:gap-4">
                                <QuickLink route={Route::Saved} icon="ph-star" label="Saved" />
                                <QuickLink route={Route::Downloads} icon="ph-download-simple" label="Downloads" />
                                <QuickLink route={Route::Queue} icon="ph-queue" label="Queue" />
                                <QuickLink route={Route::PodHistory} icon="ph-clock-counter-clockwise" label="History" />
                                <QuickLink route={Route::Feed} icon="ph-bell-ringing" label="Feed" />
                                <QuickLink route={Route::Podcasts} icon="ph-microphone-stage" label="Podcasts" />
                            </div>
                        </div>

                        // Continue Listening Section
                        <div class="section-container">
                            <h2 class="text-2xl font-bold mb-4 item_container-text">{"Continue Listening"}</h2>
                            if home_data.in_progress_episodes.is_empty() {
                                <p class="item_container-text">{"No episodes in progress"}</p>
                            } else {
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
                            }
                        </div>

                        // Top Podcasts Section
                        // In your top podcasts section, replace the existing podcast grid items with:
                        <h2 class="text-2xl font-bold mb-4 item_container-text">{"Top Podcasts"}</h2>
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
                                    podcast.podcastindexid,
                                    podcast.podcastname.clone(),
                                    podcast.feedurl.clone().unwrap_or_default(),
                                    podcast.description.clone().unwrap_or_else(|| String::from("No Description Provided")),
                                    podcast.author.clone().unwrap_or_else(|| String::from("Unknown Author")),
                                    podcast.artworkurl.clone().unwrap_or_default(),
                                    podcast.explicit,
                                    podcast.episodecount.unwrap_or(0),
                                    Some(podcast.categories.clone()),
                                    podcast.websiteurl.clone().unwrap_or_else(|| String::from("No Website Provided")),
                                    user_id.unwrap(),
                                    podcast.is_youtube,
                                );

                                html! {
                                    <div class="podcast-grid-item" onclick={on_title_click}>
                                        <div class="podcast-image-container">
                                            <img
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
                            <h2 class="text-2xl font-bold mb-4 item_container-text">{"Smart Playlists"}</h2>
                            if let Some(playlists) = &state.playlists {
                                <div class="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
                                    {
                                        playlists.iter().map(|playlist| {
                                            let history_clone = history.clone();
                                            let playlist_id = playlist.playlist_id;
                                            let onclick = Callback::from(move |_| {
                                                let route = Route::PlaylistDetail { id: playlist_id };
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
                                <p class="item_container-text">{"No playlists available"}</p>
                            }
                        </div>

                        // Recent Episodes Section
                        <div class="section-container">
                            <h2 class="text-2xl font-bold mb-4 item_container-text">{"Recent Episodes"}</h2>
                            if home_data.recent_episodes.is_empty() {
                                <p class="item_container-text">{"No recent episodes"}</p>
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
                    </div>
                } else {
                    { empty_message(
                        "Welcome to Pinepods",
                        "Start by adding some podcasts using the search bar above."
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

            // Error/Info Messages
            if let Some(error) = error_message {
                <div class="error-snackbar">{ error }</div>
            }
            if let Some(info) = info_message {
                <div class="info-snackbar">{ info }</div>
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

    // Check if this episode is currently playing
    let is_current_episode = audio_state
        .currently_playing
        .as_ref()
        .map_or(false, |current| {
            current.episode_id == props.episode.episodeid
        });

    let is_playing = audio_state.audio_playing.unwrap_or(false);

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
                <img
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
                </div>
                <p class="item_container-text text-sm">{ &props.episode.podcastname }</p>
                <span class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2">
                    <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                        <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                    </svg>
                    { formatted_date }
                </span>
            </div>
            <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
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
            </div>
        </div>
    }
}
