use crate::components::gen_components::Search_nav;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState, UserStatsStore};
use crate::components::gen_funcs::{format_date, format_time_mins};
use crate::requests::pod_req::call_get_pinepods_version;
use crate::requests::stat_reqs;
use crate::components::app_drawer::App_drawer;
use i18nrs::yew::use_translation;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

#[function_component(UserStats)]
pub fn user_stats() -> Html {
    let (i18n, _) = use_translation();
    let (stat_state, stat_dispatch) = use_store::<UserStatsStore>();
    let user_stats = stat_state.stats.as_ref();
    let pinepods_version = stat_state.pinepods_version.as_ref();

    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();

    {
        let api_key = post_state
            .auth_details
            .as_ref()
            .map(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = post_state
            .auth_details
            .as_ref()
            .map(|ud| ud.server_name.clone());
        let server_name_effect = server_name.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let get_server_name = server_name.clone();
                    let get_api_key = api_key.clone();
                    let get_stat_dispatch = stat_dispatch.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(fetched_stats) = stat_reqs::call_get_stats(
                            get_server_name.clone(),
                            get_api_key.clone(),
                            &user_id,
                        )
                        .await
                        {
                            get_stat_dispatch.reduce_mut(move |state| {
                                state.stats = Some(fetched_stats);
                            });
                        }
                    });

                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(fetched_stats) = call_get_pinepods_version(
                            server_name_effect.unwrap().clone(),
                            &api_key.clone(),
                        )
                        .await
                        {
                            stat_dispatch.reduce_mut(move |state| {
                                state.pinepods_version = Some(fetched_stats);
                            });
                        }
                    });
                }
                || ()
            },
        );
    }

    let display_version_value = "test_mode".to_string();
    let display_version = pinepods_version
        .as_deref()
        .unwrap_or(&display_version_value);

    html! {
        <>
        <div class="main-container">
            <Search_nav />

            <div class="user-stats-header">
                <h1 class="user-stats-title">{i18n.t("user_stats.user_statistics")}</h1>
            </div>

            {
                if let Some(stats) = user_stats {
                    let formatted_date = format_date(&stats.UserCreated);
                    let time_formatted = format_time_mins(stats.TimeListened);

                    let sync_content = match stats.Pod_Sync_Type.as_str() {
                        "None" => html! { <p class="stats-value">{i18n.t("user_stats.not_syncing")}</p> },
                        _ => {
                            let sync_type = if stats.Pod_Sync_Type == "gpodder" {
                                if stats.GpodderUrl == "http://localhost:8042" {
                                    i18n.t("user_stats.internal_gpodder")
                                } else {
                                    i18n.t("user_stats.external_gpodder")
                                }
                            } else if stats.Pod_Sync_Type == "nextcloud" {
                                i18n.t("user_stats.nextcloud")
                            } else {
                                i18n.t("user_stats.unknown_sync_type")
                            };
                            html! {
                                <>
                                    <p class="stats-value">{sync_type}</p>
                                    <p class="stats-detail">{&stats.GpodderUrl}</p>
                                </>
                            }
                        }
                    };

                    html! {
                        <>
                        <div class="user-stats-grid">
                            <div class="stats-card">
                                <i class="ph ph-calendar stats-icon"></i>
                                <p class="stats-value">{&formatted_date}</p>
                                <p class="stats-label">{i18n.t("user_stats.user_created")}</p>
                            </div>

                            <div class="stats-card">
                                <i class="ph ph-play-circle stats-icon"></i>
                                <p class="stats-value">{ stats.PodcastsPlayed }</p>
                                <p class="stats-label">{i18n.t("user_stats.podcasts_played")}</p>
                            </div>

                            <div class="stats-card">
                                <i class="ph ph-clock stats-icon"></i>
                                <p class="stats-value">{ &time_formatted }</p>
                                <p class="stats-label">{i18n.t("user_stats.time_listened")}</p>
                            </div>

                            <div class="stats-card">
                                <i class="ph ph-microphone-stage stats-icon"></i>
                                <p class="stats-value">{ stats.PodcastsAdded }</p>
                                <p class="stats-label">{i18n.t("user_stats.podcasts_added")}</p>
                            </div>

                            <div class="stats-card">
                                <i class="ph ph-bookmark-simple stats-icon"></i>
                                <p class="stats-value">{ stats.EpisodesSaved }</p>
                                <p class="stats-label">{i18n.t("user_stats.episodes_saved")}</p>
                            </div>

                            <div class="stats-card">
                                <i class="ph ph-download-simple stats-icon"></i>
                                <p class="stats-value">{ stats.EpisodesDownloaded }</p>
                                <p class="stats-label">{i18n.t("user_stats.episodes_downloaded")}</p>
                            </div>
                        </div>

                        <div class="stats-card stats-sync-card">
                            <i class="ph ph-arrows-clockwise stats-icon"></i>
                            <p class="stats-label">{i18n.t("user_stats.podcast_sync_status")}</p>
                            {sync_content}
                        </div>
                        </>
                    }
                } else {
                    html! { <p class="item_container-text">{i18n.t("user_stats.loading_user_stats")}</p> }
                }
            }

            <div class="large-card">
                <img src="static/assets/favicon.png" alt="Pinepods Logo" class="large-card-image"/>
                <div class="about-info">
                    <p class="large-card-paragraph item_container-text">{ format!("{}{}", i18n.t("user_stats.current_version"), display_version) }</p>
                    <p class="large-card-paragraph item_container-text">{i18n.t("user_stats.about_text")}</p>
                    <div class="about-links">
                        <a href="https://pinepods.online" target="_blank" class="large-card-button focus:ring-4 font-medium rounded-lg text-sm px-5 py-2.5 focus:outline-none">{i18n.t("user_stats.pinepods_documentation")}</a>
                        <a href="https://github.com/madeofpendletonwool/pinepods" target="_blank" class="large-card-button focus:ring-4 font-medium rounded-lg text-sm px-5 py-2.5 focus:outline-none">{i18n.t("user_stats.pinepods_github_repo")}</a>
                        <a href="https://www.buymeacoffee.com/collinscoffee" target="_blank" class="large-card-button focus:ring-4 font-medium rounded-lg text-sm px-5 py-2.5 focus:outline-none">{i18n.t("user_stats.buy_me_coffee")}</a>
                    </div>
                </div>
            </div>

            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! {
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
                } else {
                    html! {}
                }
            }
        </div>
        <App_drawer />
        </>
    }
}
