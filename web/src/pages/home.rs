use crate::components::app_drawer::App_drawer;
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, EpisodeStatusState, UIState};
use crate::components::context_menu_button::ContextMenuButton;
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_components::on_shownotes_click;
use crate::components::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::gen_funcs::{format_datetime, format_time, match_date_format, parse_date};
use crate::components::loading::Loading;
use crate::pages::routes::Route;
use crate::requests::episode::Episode;
use crate::requests::pod_req::Playlist;
use crate::requests::pod_req::{self};

use i18nrs::yew::use_translation;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yew_router::prelude::Link;
use yewdux::prelude::*;

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

                                effect_dispatch.reduce_mut(move |state| {
                                    state.home_overview = Some(home_data);
                                });
                                Dispatch::<EpisodeStatusState>::global().reduce_mut(move |state| {
                                    state.completed_episodes = Some(completed_episode_ids);
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
                {
                    html! { <Loading/> }
                }
            } else {
                if let Some(home_data) = &state.home_overview {
                    <div class="space-y-8">
                        // Quick Links Section
                        <div class="section-container">
                            <h2 class="text-2xl font-bold mb-4 item_container-text">{&i18n_quick_links}</h2>
                            <div class="grid grid-cols-3 gap-2 md:gap-4">
                                <QuickLink route={Route::Saved} icon="ph-star" label={i18n_saved.clone()} />
                                <QuickLink route={Route::Downloads} icon="ph-download-simple" label={i18n_downloads.clone()} />
                                <QuickLink route={Route::Playlists} icon="ph-playlist" label={i18n_smart_playlists.clone()} />
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
                                                    <EpisodeListItem
                                                        episode={ episode.clone() }
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
                                    podcast.podcastid,
                                    podcast.podcastindexid,
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
                                            <EpisodeListItem
                                                episode={episode.clone()}
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
