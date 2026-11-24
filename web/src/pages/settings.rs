use crate::components::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, NotificationState, UIState};
use crate::components::gen_components::{Search_nav, UseScrollToTop};
use crate::components::gen_funcs::format_error_message;
use crate::components::i18n::use_translation;
use crate::components::setting_components;
use crate::requests::pod_req::call_get_ai_status;
use crate::requests::setting_reqs::call_user_admin_check;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yew_router::hooks::use_location;
use yewdux::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Group {
    Personal,
    Server,
}

/// One card on a settings page. `keywords` are extra English words search should match.
struct SectionDef {
    id: &'static str,
    icon: &'static str,
    title: &'static str,
    info: Option<&'static str>,
    keywords: &'static str,
}

struct PageDef {
    id: &'static str,
    icon: &'static str,
    title: &'static str,
    group: Group,
    sections: &'static [SectionDef],
}

const fn sec(
    id: &'static str,
    icon: &'static str,
    title: &'static str,
    info: Option<&'static str>,
    keywords: &'static str,
) -> SectionDef {
    SectionDef { id, icon, title, info, keywords }
}

const PAGES: &[PageDef] = &[
    PageDef {
        id: "account",
        icon: "ph-user-circle",
        title: "settings.page_account",
        group: Group::Personal,
        sections: &[
            sec("account", "ph-user-circle", "settings.account_settings", None, "profile name email username password language timezone time format date avatar gravatar"),
            sec("mfa", "ph-shield-check", "settings.mfa_settings", Some("mfa_settings.mfa_description"), "two factor 2fa totp authenticator security login"),
            sec("api_keys", "ph-key", "settings.api_keys", Some("api_keys.api_keys_description"), "token integration security"),
        ],
    },
    PageDef {
        id: "appearance",
        icon: "ph-paint-brush",
        title: "settings.page_appearance",
        group: Group::Personal,
        sections: &[
            sec("theme", "ph-paint-roller", "settings.change_theme", Some("settings.choose_preferred_theme"), "theme dark light colour color accent artwork"),
            sec("custom_themes", "ph-palette", "settings.custom_themes", Some("settings.custom_themes_description"), "theme colour color create"),
            sec("display", "ph-monitor", "settings.display_settings", Some("start_page_options.start_page_description"), "start page home landing count layout"),
        ],
    },
    PageDef {
        id: "playback",
        icon: "ph-play-circle",
        title: "settings.page_playback",
        group: Group::Personal,
        sections: &[
            sec("playback", "ph-play-circle", "settings.playback_settings", Some("playback_settings.playback_preferences_description"), "speed volume skip forward back seek queue autoplay silence trim"),
            sec("auto_delete", "ph-trash", "settings.auto_delete_settings", Some("auto_delete_settings.description"), "downloads cleanup storage finished remove"),
        ],
    },
    PageDef {
        id: "inbox",
        icon: "ph-tray",
        title: "settings.page_inbox",
        group: Group::Personal,
        sections: &[
            sec("inbox", "ph-tray", "settings.inbox_settings", Some("inbox_settings.description"), "inbox new episodes triage days window dismiss"),
            sec("labels", "ph-tag", "settings.episode_labels", Some("labels.description"), "labels tags triage later listen maybe filter smart playlist"),
        ],
    },
    PageDef {
        id: "notifications",
        icon: "ph-bell-ringing",
        title: "settings.page_notifications",
        group: Group::Personal,
        sections: &[
            sec("notifications", "ph-bell-ringing", "settings.notification_settings", None, "ntfy gotify apprise push alerts new episodes"),
        ],
    },
    PageDef {
        id: "podcasts",
        icon: "ph-rss-simple",
        title: "settings.page_podcasts",
        group: Group::Personal,
        sections: &[
            sec("custom_feed", "ph-rss", "settings.add_custom_feed", Some("custom_feed.add_feed_description"), "add feed url private patreon password"),
            sec("local_podcast", "ph-hard-drives", "settings.add_local_podcast", Some("local_podcast.description"), "files folder audio local"),
            sec("feed_problems", "ph-warning-circle", "feed_problems.title", Some("feed_problems.description"), "broken failing errors refresh"),
            sec("podcast_index", "ph-magnifying-glass", "settings.match_podcasts_podcast_index", None, "podcastindex match link id"),
        ],
    },
    PageDef {
        id: "sync",
        icon: "ph-arrows-clockwise",
        title: "settings.page_sync",
        group: Group::Personal,
        sections: &[
            sec("podcast_sync", "ph-arrows-clockwise", "settings.podcast_sync", Some("nextcloud_options.gpodder_sync_description"), "gpodder nextcloud antennapod devices app password"),
            sec("import", "ph-upload-simple", "settings.import_podcasts", Some("import_options.import_description"), "opml import upload migrate"),
            sec("export", "ph-download-simple", "settings.export_backup_podcasts", Some("export_settings.export_description"), "opml export backup download"),
        ],
    },
    PageDef {
        id: "sharing",
        icon: "ph-share-network",
        title: "settings.page_sharing",
        group: Group::Personal,
        sections: &[
            sec("rss_feeds", "ph-wifi", "settings.enable_disable_rss_feeds", Some("rss_feeds.rss_feed_description"), "rss feed subscribe external app"),
            sec("shared_links", "ph-share-network", "settings.shared_links", Some("shared_links.description"), "share link public episode"),
        ],
    },
    PageDef {
        id: "ai",
        icon: "ph-sparkle",
        title: "settings.ai",
        group: Group::Personal,
        sections: &[
            sec("ai", "ph-sparkle", "settings.ai", Some("ai_settings.intro"), "ai transcription transcript whisper ads ad detection llm model ollama"),
        ],
    },
    PageDef {
        id: "users",
        icon: "ph-users",
        title: "settings.page_users",
        group: Group::Server,
        sections: &[
            sec("user_management", "ph-users", "settings.user_management", Some("settings.user_management_description"), "create delete edit admin accounts"),
            sec("self_service", "ph-user-plus", "settings.user_self_service_settings", Some("user_self_service.self_service_description"), "sign up registration self service"),
            sec("oidc", "ph-lock-key", "settings.oidc_sso_settings", None, "oidc sso oauth single sign on authentik keycloak login provider"),
        ],
    },
    PageDef {
        id: "downloads",
        icon: "ph-download-simple",
        title: "settings.page_downloads",
        group: Group::Server,
        sections: &[
            sec("downloads", "ph-download-simple", "settings.download_settings", Some("download_settings.server_download_description"), "server downloads folder metadata cover id3"),
            sec("ytdlp", "ph-youtube-logo", "settings.ytdlp_management", None, "youtube yt-dlp update channel"),
        ],
    },
    PageDef {
        id: "services",
        icon: "ph-broadcast",
        title: "update_services.title",
        group: Group::Server,
        sections: &[
            sec("update_services", "ph-broadcast", "update_services.title", Some("update_services.description"), "podping refresh schedule updates"),
        ],
    },
    PageDef {
        id: "email",
        icon: "ph-envelope",
        title: "settings.email_settings",
        group: Group::Server,
        sections: &[
            sec("email", "ph-envelope", "settings.email_settings", Some("email_settings.email_settings_description"), "smtp mail password reset"),
        ],
    },
    PageDef {
        id: "backup",
        icon: "ph-database",
        title: "settings.page_backup",
        group: Group::Server,
        sections: &[
            sec("backup_server", "ph-database", "settings.backup_server", None, "backup database dump export"),
            sec("restore_server", "ph-clock-counter-clockwise", "settings.restore_server", None, "restore database import"),
        ],
    },
];

/// Which group a page is listed under. AI is configured by admins, so it moves to Server for them.
fn page_group(page: &PageDef, is_admin: bool) -> Group {
    if page.id == "ai" && is_admin {
        Group::Server
    } else {
        page.group
    }
}

fn page_visible(page: &PageDef, is_admin: bool, ai_available: bool) -> bool {
    match (page.group, page.id) {
        (Group::Server, _) => is_admin,
        // Users only see AI when an admin has it on and the AI container is connected.
        (_, "ai") => is_admin || ai_available,
        _ => true,
    }
}

fn page_of_section(section: &str) -> Option<&'static PageDef> {
    PAGES.iter().find(|p| p.sections.iter().any(|s| s.id == section))
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.trim_start_matches('?').split('&').find_map(|kv| {
        kv.strip_prefix(key)
            .and_then(|rest| rest.strip_prefix('='))
            .filter(|v| !v.is_empty())
            .map(str::to_string)
    })
}

fn section_body(id: &str, themes_trigger: &UseStateHandle<u32>) -> Html {
    use setting_components::*;
    match id {
        "account" => html! { <user_self_settings::UserSelfSettings /> },
        "mfa" => html! { <mfa_settings::MFAOptions /> },
        "api_keys" => html! { <api_keys::APIKeys /> },
        "theme" => html! {
            <>
                <theme_options::ThemeOptions refresh_trigger={**themes_trigger} />
                <div class="px-6 pb-4"><theme_options::ArtAccentOption /></div>
            </>
        },
        "custom_themes" => {
            let trigger = themes_trigger.clone();
            html! {
                <custom_theme_creator::CustomThemeCreator
                    on_created={Callback::from(move |_| trigger.set(*trigger + 1))}
                />
            }
        }
        "display" => html! { <start_page_options::StartPageOptions /> },
        "playback" => html! { <playback_settings::PlaybackSettings /> },
        "auto_delete" => html! { <auto_delete_settings::AutoDeleteSettings /> },
        "inbox" => html! { <inbox_settings::InboxSettings /> },
        "labels" => html! { <inbox_settings::LabelSettings /> },
        "notifications" => html! { <notifications::NotificationOptions /> },
        "custom_feed" => html! { <custom_feed::CustomFeed /> },
        "local_podcast" => html! { <local_podcast::LocalPodcast /> },
        "feed_problems" => html! { <feed_problems::FeedProblemsSection /> },
        "podcast_index" => html! { <podcast_index_matching::PodcastIndexMatching /> },
        "podcast_sync" => html! { <nextcloud_options::SyncOptions /> },
        "import" => html! { <import_options::ImportOptions /> },
        "export" => html! { <export_settings::ExportOptions /> },
        "rss_feeds" => html! { <rss_feeds::RSSFeedSettings /> },
        "shared_links" => html! { <shared_links::SharedLinks /> },
        "ai" => html! { <ai_settings::AiSettings /> },
        "user_management" => html! { <user_settings::UserSettings /> },
        "self_service" => html! { <user_self_service::SelfServiceSettings /> },
        "oidc" => html! { <oidc::OIDCSettings /> },
        "downloads" => html! { <download_settings::DownloadSettings /> },
        "ytdlp" => html! { <ytdlp_settings::YtDlpSettings /> },
        "update_services" => html! { <update_services::UpdateServicesSection /> },
        "email" => html! { <email_settings::EmailSettings /> },
        "backup_server" => html! { <backup_server::BackupServer /> },
        "restore_server" => html! { <restore_server::RestoreServer /> },
        _ => html! {},
    }
}

fn go_to(query: &str) {
    BrowserHistory::new().push(format!("/settings{query}"));
}

#[function_component(Settings)]
pub fn settings() -> Html {
    let (i18n, _) = use_translation();
    let (post_state, _) = use_store::<AppState>();
    let (audio_state, _) = use_store::<UIState>();

    let api_key = post_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID);
    let server_name = post_state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    let is_admin = use_state(|| false);
    let ai_available = use_state(|| false);
    let custom_themes_trigger = use_state(|| 0u32);
    let search = use_state(String::new);

    let admin_check_error_msg = i18n.t("settings.admin_check_error");
    {
        let is_admin = is_admin.clone();
        let ai_available = ai_available.clone();
        use_effect_with(
            (api_key.clone(), user_id, server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id, server_name.clone())
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(up) = call_get_ai_status(&server_name, &api_key).await {
                            ai_available.set(up);
                        }
                        match call_user_admin_check(&server_name, &api_key.unwrap(), user_id).await {
                            Ok(response) => is_admin.set(response.is_admin),
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message =
                                        Some(format!("{}: {:?}", admin_check_error_msg, formatted_error))
                                });
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    // `?page=<id>` picks a page; `?section=<id>` (links from Activity messages) opens the page
    // holding that section and scrolls to it.
    let query = use_location().map(|loc| loc.query_str().to_string()).unwrap_or_default();
    let section_param = query_param(&query, "section");
    let page_param = query_param(&query, "page");
    {
        use_effect_with(section_param.clone(), move |section| {
            if let Some(section) = section.clone() {
                gloo_timers::callback::Timeout::new(150, move || {
                    if let Some(el) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id(&format!("settings-{section}")))
                    {
                        el.scroll_into_view();
                    }
                })
                .forget();
            }
            || ()
        });
    }

    let visible: Vec<&'static PageDef> = PAGES
        .iter()
        .filter(|p| page_visible(p, *is_admin, *ai_available))
        .collect();
    let requested = page_param
        .as_deref()
        .and_then(|id| visible.iter().copied().find(|p| p.id == id))
        .or_else(|| section_param.as_deref().and_then(page_of_section));
    // On narrow screens the page list is its own screen until a page is picked.
    let chosen = requested.is_some();
    let active = requested.or_else(|| visible.first().copied());

    let search_text = search.trim().to_lowercase();
    let searching = !search_text.is_empty();

    let on_search = {
        let search = search.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                search.set(input.value());
            }
        })
    };
    let clear_search = {
        let search = search.clone();
        Callback::from(move |_: MouseEvent| search.set(String::new()))
    };

    // `head: false` for a page's only section, whose title the page heading already shows.
    let section_card = |s: &SectionDef, crumb: Option<String>, head: bool| -> Html {
        let info = s.info.map(|k| i18n.t(k).to_string());
        html! {
            <div class="settings-section" id={format!("settings-{}", s.id)} key={s.id}>
                if head {
                <div class="settings-section-head">
                    <i class={format!("ph {}", s.icon)}></i>
                    <div>
                        if let Some(crumb) = crumb {
                            <div class="settings-section-crumb">{ crumb }</div>
                        }
                        <div class="settings-section-title-row">
                            <div class="settings-section-title">{ i18n.t(s.title) }</div>
                            if let Some(info) = info {
                                <button class="info-btn" title={info}>{"?"}</button>
                            }
                        </div>
                    </div>
                </div>
                }
                <div class="settings-section-body">
                    { section_body(s.id, &custom_themes_trigger) }
                </div>
            </div>
        }
    };

    let content = if searching {
        let words: Vec<&str> = search_text.split_whitespace().collect();
        let hits: Vec<Html> = visible
            .iter()
            .flat_map(|p| p.sections.iter().map(move |s| (*p, s)))
            .filter(|(p, s)| {
                let hay = format!(
                    "{} {} {} {}",
                    i18n.t(p.title),
                    i18n.t(s.title),
                    s.info.map(|k| i18n.t(k).to_string()).unwrap_or_default(),
                    s.keywords
                )
                .to_lowercase();
                words.iter().all(|w| hay.contains(w))
            })
            .map(|(p, s)| section_card(s, Some(i18n.t(p.title).to_string()), true))
            .collect();
        if hits.is_empty() {
            html! {
                <div class="settings-empty">
                    <i class="ph ph-magnifying-glass"></i>
                    <div>{ i18n.t("settings.search_no_results") }</div>
                </div>
            }
        } else {
            html! { <>{ for hits }</> }
        }
    } else if let Some(page) = active {
        let single = page.sections.len() == 1;
        let page_info = page.sections.first().filter(|_| single).and_then(|s| s.info).map(|k| i18n.t(k).to_string());
        html! {
            <>
                <div class="settings-content-head">
                    <button class="settings-back" onclick={Callback::from(|_| go_to(""))}
                        aria-label={i18n.t("settings.settings").to_string()}>
                        <i class="ph ph-caret-left"></i>
                    </button>
                    <h2 class="settings-content-title">{ i18n.t(page.title) }</h2>
                    if let Some(info) = page_info {
                        <button class="info-btn" title={info}>{"?"}</button>
                    }
                </div>
                { for page.sections.iter().map(|s| section_card(s, None, !single)) }
            </>
        }
    } else {
        html! {}
    };

    let nav_group = |group: Group, label: String| -> Html {
        let pages: Vec<&&PageDef> = visible.iter().filter(|p| page_group(p, *is_admin) == group).collect();
        if pages.is_empty() {
            return html! {};
        }
        html! {
            <div class="settings-nav-group">
                <div class="settings-nav-label">{ label }</div>
                { for pages.into_iter().map(|p| {
                    let id = p.id;
                    let is_active = !searching && active.is_some_and(|a| a.id == id);
                    html! {
                        <button
                            class={classes!("settings-nav-item", is_active.then_some("is-active"))}
                            onclick={Callback::from(move |_| go_to(&format!("?page={id}")))}
                        >
                            <i class={format!("ph {}", p.icon)}></i>
                            <span>{ i18n.t(p.title) }</span>
                            <i class="ph ph-caret-right settings-nav-chevron"></i>
                        </button>
                    }
                }) }
            </div>
        }
    };

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            <div class={classes!("my-4", "settings-page", (chosen || searching).then_some("has-page"))}>
                <div class="settings-header">
                    <div class="screen-title">{ i18n.t("settings.settings") }</div>
                    <div class="settings-search">
                        <i class="ph ph-magnifying-glass"></i>
                        <input
                            type="search"
                            placeholder={i18n.t("settings.search_placeholder").to_string()}
                            value={(*search).clone()}
                            oninput={on_search}
                        />
                        if searching {
                            <button class="settings-search-clear" onclick={clear_search}
                                aria-label={i18n.t("common.close_modal").to_string()}>
                                <i class="ph ph-x"></i>
                            </button>
                        }
                    </div>
                </div>
                <div class="settings-layout">
                    <nav class="settings-nav">
                        { nav_group(Group::Personal, i18n.t("settings.group_personal").to_string()) }
                        { nav_group(Group::Server, i18n.t("settings.group_server").to_string()) }
                    </nav>
                    <div class="settings-content">
                        { content }
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
