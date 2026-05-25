use crate::components::gen_components::Search_nav;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState, UserStatsStore};
use crate::components::gen_funcs::{format_date, format_time, format_time_mins};
use crate::requests::pod_req::call_get_pinepods_version;
use crate::requests::stat_reqs::{self, TopPodcastStat, CategoryStat};
use crate::components::app_drawer::App_drawer;
use i18nrs::yew::use_translation;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

#[function_component(UserStats)]
pub fn user_stats() -> Html {
    fn render_dow_chart(data: &[i64]) -> Html {
        if data.len() < 7 {
            return html! {};
        }
        let days = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let max_val = *data.iter().max().unwrap_or(&1);
        let max_val = if max_val == 0 { 1 } else { max_val };
        let chart_height: i32 = 100;
        let bar_width: i32 = 30;
        let gap: i32 = 8;
        let svg_width: i32 = (bar_width + gap) * 7 - gap;
        let svg_height: i32 = chart_height + 22;

        let bars: Html = data.iter().enumerate().map(|(i, &val)| {
            let bar_h = ((val as f64 / max_val as f64) * chart_height as f64) as i32;
            let bar_h = if bar_h < 2 && val > 0 { 2 } else { bar_h };
            let x = i as i32 * (bar_width + gap);
            let y = chart_height - bar_h;
            let cx = x + bar_width / 2;
            html! {
                <>
                <rect x={x.to_string()} y={y.to_string()}
                      width={bar_width.to_string()} height={bar_h.to_string()}
                      rx="4" class="chart-bar" />
                <text x={cx.to_string()} y={(chart_height + 14).to_string()}
                      class="chart-label" text-anchor="middle">
                    {days[i]}
                </text>
                </>
            }
        }).collect();

        html! {
            <svg width={svg_width.to_string()} height={svg_height.to_string()}
                 viewBox={format!("0 0 {} {}", svg_width, svg_height)}
                 style="display:block">
                {bars}
            </svg>
        }
    }

    fn render_top_podcasts(podcasts: &[TopPodcastStat]) -> Html {
        if podcasts.is_empty() {
            return html! {};
        }
        let max_val = podcasts.iter().map(|p| p.total_seconds).max().unwrap_or(1);
        let max_val = if max_val == 0 { 1 } else { max_val };

        podcasts.iter().map(|p| {
            let pct = (p.total_seconds as f64 / max_val as f64 * 100.0) as i32;
            let h = (p.total_seconds / 3600) as i32;
            let m = ((p.total_seconds % 3600) / 60) as i32;
            let time_str = if h > 0 { format!("{}h {}m", h, m) } else { format!("{}m", m) };
            html! {
                <div class="top-podcast-row">
                    <img src={p.artworkurl.clone()} class="top-podcast-artwork"
                         alt={p.podcastname.clone()} />
                    <div class="top-podcast-bar-container">
                        <span class="top-podcast-name">{&p.podcastname}</span>
                        <div class="top-podcast-bar-track">
                            <div class="top-podcast-bar-fill"
                                 style={format!("width: {}%", pct)} />
                        </div>
                    </div>
                    <span class="top-podcast-time">{time_str}</span>
                </div>
            }
        }).collect()
    }

    fn render_categories(cats: &[CategoryStat]) -> Html {
        if cats.is_empty() {
            return html! {};
        }
        cats.iter().map(|c| {
            html! {
                <span class="category-chip">
                    <span class="category-chip-dot" />
                    {&c.name}
                </span>
            }
        }).collect()
    }

    let (i18n, _) = use_translation();
    let (stat_state, stat_dispatch) = use_store::<UserStatsStore>();
    let user_stats = stat_state.stats.as_ref();
    let extended_stats = stat_state.extended_stats.as_ref();
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

                    let ext_server_name = server_name.clone();
                    let ext_api_key = api_key.clone();
                    let ext_dispatch = stat_dispatch.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(fetched) = stat_reqs::call_get_extended_stats(
                            ext_server_name.clone(),
                            ext_api_key.clone(),
                            &user_id,
                        )
                        .await
                        {
                            ext_dispatch.reduce_mut(move |state| {
                                state.extended_stats = Some(fetched);
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

            // Extended stats section
            {
                if let Some(ext) = extended_stats {
                    let badge_label = match ext.listening_badge.as_str() {
                        "night_owl"          => (i18n.t("user_stats.badge_night_owl"), "ph ph-moon"),
                        "evening_listener"   => (i18n.t("user_stats.badge_evening_listener"), "ph ph-sunset"),
                        "afternoon_listener" => (i18n.t("user_stats.badge_afternoon_listener"), "ph ph-sun"),
                        "morning_listener"   => (i18n.t("user_stats.badge_morning_listener"), "ph ph-sunrise"),
                        _                    => (i18n.t("user_stats.no_listening_data"), "ph ph-headphones"),
                    };

                    let longest_ep_html = if let Some(ep) = &ext.longest_episode {
                        let dur_str = format_time(ep.episodeduration);
                        html! {
                            <div class="stats-card" style="margin-bottom: 1rem;">
                                <i class="ph ph-trophy stats-icon"></i>
                                <p class="stats-label">{i18n.t("user_stats.longest_episode")}</p>
                                <span class="longest-ep-title">{&ep.episodetitle}</span>
                                <p class="longest-ep-podcast">{format!("{} · {}", &ep.podcastname, dur_str)}</p>
                            </div>
                        }
                    } else {
                        html! {}
                    };

                    let dow_all_zero = ext.listening_by_dow.iter().all(|&v| v == 0);
                    let dow_chart_html = if !dow_all_zero && ext.listening_by_dow.len() == 7 {
                        let chart = render_dow_chart(&ext.listening_by_dow);
                        html! {
                            <div class="chart-card">
                                <p class="chart-card-title">{i18n.t("user_stats.listening_by_day")}</p>
                                {chart}
                            </div>
                        }
                    } else {
                        html! {}
                    };

                    let top_podcasts_html = if !ext.top_podcasts.is_empty() {
                        let bars = render_top_podcasts(&ext.top_podcasts);
                        html! {
                            <div class="chart-card">
                                <p class="chart-card-title">{i18n.t("user_stats.top_podcasts_section")}</p>
                                {bars}
                            </div>
                        }
                    } else {
                        html! {}
                    };

                    let categories_html = if !ext.favorite_categories.is_empty() {
                        let chips = render_categories(&ext.favorite_categories);
                        html! {
                            <div class="chart-card">
                                <p class="chart-card-title">{i18n.t("user_stats.favorite_categories_section")}</p>
                                <div class="category-chips">{chips}</div>
                            </div>
                        }
                    } else {
                        html! {}
                    };

                    html! {
                        <div class="extended-stats-section">
                            <p class="extended-stats-section-title">{i18n.t("user_stats.listening_insights")}</p>

                            <div class="insights-row">
                                <div class="stats-card">
                                    <i class={badge_label.1} aria-hidden="true" class="badge-icon stats-icon"></i>
                                    <p class="stats-value badge-name">{badge_label.0}</p>
                                    <p class="stats-label">{i18n.t("user_stats.listening_badge")}</p>
                                </div>

                                <div class="stats-card">
                                    <i class="ph ph-flame stats-icon"></i>
                                    <p class="stats-value streak-number">{ ext.current_streak }</p>
                                    <p class="stats-label">{i18n.t("user_stats.current_streak")}</p>
                                </div>

                                <div class="stats-card">
                                    <i class="ph ph-check-circle stats-icon"></i>
                                    <p class="stats-value completion-rate-value">{ format!("{}%", ext.completion_rate) }</p>
                                    <p class="stats-label">{i18n.t("user_stats.completion_rate")}</p>
                                </div>
                            </div>

                            <div class="insights-row" style="grid-template-columns: 1fr 1fr;">
                                <div class="stats-card">
                                    <i class="ph ph-hard-drive stats-icon"></i>
                                    <p class="stats-value">{ &ext.total_downloaded_formatted }</p>
                                    <p class="stats-label">{i18n.t("user_stats.total_downloaded")}</p>
                                </div>
                                {longest_ep_html}
                            </div>

                            {dow_chart_html}
                            {top_podcasts_html}
                            {categories_html}
                        </div>
                    }
                } else {
                    html! {}
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
