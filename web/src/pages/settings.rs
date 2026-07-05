use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, NotificationState, UIState};
use crate::components::gen_funcs::format_error_message;
use crate::components::setting_components;
use crate::requests::setting_reqs::call_user_admin_check;
use crate::components::app_drawer::App_drawer;
use crate::components::gen_components::{Search_nav, UseScrollToTop};
use i18nrs::yew::use_translation;
use yew::prelude::*;
use yewdux::prelude::*;
// use crate::components::gen_funcs::check_auth;

#[derive(Properties, PartialEq, Clone)]
pub struct TabProps {
    pub is_active: bool,
    pub label: String,
    pub onclick: Callback<MouseEvent>,
    pub class: String,
}

#[function_component(Tab)]
fn tab(props: &TabProps) -> Html {
    let TabProps {
        is_active,
        label,
        onclick,
        class,
    } = props.clone();

    let tab_class = if is_active {
        format!(
            "{} tab-hightlight-colors px-6 py-2 rounded-md transition-all duration-200",
            class
        )
    } else {
        format!("{} tab-unselect-colors px-6 py-2 rounded-md hover:bg-opacity-10 hover:bg-white transition-all duration-200", class)
    };

    html! {
        <button class={tab_class} onclick={onclick}>{ label }</button>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct AccordionItemProps {
    pub title: String,
    pub content: Html,
    pub position: AccordionItemPosition, // Add this line
}

// Enum to represent the position of the accordion item
#[derive(PartialEq, Clone)]
#[allow(dead_code)]
pub enum AccordionItemPosition {
    First,
    Middle,
}

#[function_component(AccordionItem)]
pub fn accordion_item(
    AccordionItemProps {
        title,
        content,
        position,
    }: &AccordionItemProps,
) -> Html {
    let is_open = use_state(|| false);
    let toggle = {
        let is_open = is_open.clone();
        Callback::from(move |_| is_open.set(!*is_open))
    };

    let (border_class, button_class) = match position {
        AccordionItemPosition::First => ("rounded-t-xl", "border-b-0"),
        AccordionItemPosition::Middle => ("", "border-b-0"),
    };

    let arrow_rotation_class = if *is_open { "rotate-180" } else { "rotate-0" };

    html! {
        <div class={format!("accordion-container {}", border_class)}>
            <h2>
                <button
                    class={format!("accordion-button flex items-center justify-between w-full p-5 font-medium {} focus:ring-4 gap-3 relative", button_class)}
                    onclick={toggle}
                >
                    <span>{ title }</span>
                    <svg
                        class={format!("w-3 h-3 shrink-0 transition-transform duration-300 accordion-arrow {}", arrow_rotation_class)}
                        xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6"
                    >
                        <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5 5 1 1 5"/>
                    </svg>
                </button>
            </h2>
            if *is_open {
                <div class="p-5 accordion-content">
                    { content.clone() }
                </div>
            }
        </div>
    }
}

#[function_component(Settings)]
pub fn settings() -> Html {
    let (i18n, _) = use_translation();
    let (_post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let active_tab = use_state(|| "user");

    let api_key = _post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = _post_state
        .user_details
        .as_ref()
        .map(|ud| ud.UserID.clone());
    let server_name = _post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    let is_admin = use_state(|| false);
    let custom_themes_trigger = use_state(|| 0u32);
    let _audio_admin = _post_dispatch.clone();

    // Pre-capture translation string for async block
    let admin_check_error_msg = i18n.t("settings.admin_check_error");

    {
        let is_admin = is_admin.clone();
        let admin_check_error_msg = admin_check_error_msg.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_user_admin_check(&server_name, &api_key.unwrap(), user_id).await
                        {
                            Ok(response) => {
                                is_admin.set(response.is_admin);
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message = Some(format!(
                                        "{}: {:?}",
                                        admin_check_error_msg, formatted_error
                                    ))
                                });
                                // console::log_1(&format!("Failed to check admin status: {:?}", e).into());
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    let on_user_tab_click = {
        let active_tab = active_tab.clone();
        Callback::from(move |_| active_tab.set("user"))
    };

    let on_admin_tab_click = {
        let active_tab = active_tab.clone();
        Callback::from(move |_| active_tab.set("admin"))
    };

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            <div class="my-4 settings-page">
                <div class="screen-title">{ &i18n.t("settings.settings") }</div>

                // Tab switcher: User / Admin
                <div class="settings-tabs">
                    <button
                        class={classes!("settings-tab", if *active_tab == "user" { "is-active" } else { "" })}
                        onclick={on_user_tab_click.clone()}
                    >
                        { &i18n.t("settings.user_settings") }
                    </button>
                    {
                        if *is_admin {
                            html! {
                                <button
                                    class={classes!("settings-tab", if *active_tab == "admin" { "is-active" } else { "" })}
                                    onclick={on_admin_tab_click.clone()}
                                >
                                    { &i18n.t("settings.admin_settings") }
                                </button>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                {
                    if *active_tab == "user" {
                        html! {
                            <>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-paint-roller"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.change_theme") }</div>
                                            <button class="info-btn" title={i18n.t("settings.choose_preferred_theme").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::theme_options::ThemeOptions refresh_trigger={*custom_themes_trigger} />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-palette"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.custom_themes") }</div>
                                            <button class="info-btn" title="Create your own themes with custom colors">{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::custom_theme_creator::CustomThemeCreator
                                        on_created={{
                                            let trigger = custom_themes_trigger.clone();
                                            Callback::from(move |_| trigger.set(*trigger + 1))
                                        }}
                                    />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-user-circle"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.account_settings") }</div>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::user_self_settings::UserSelfSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-play-circle"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.playback_settings") }</div>
                                            <button class="info-btn" title={i18n.t("playback_settings.playback_preferences_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::playback_settings::PlaybackSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-trash"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.auto_delete_settings") }</div>
                                            <button class="info-btn" title={i18n.t("auto_delete_settings.description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::auto_delete_settings::AutoDeleteSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-text-aa"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.ai") }</div>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::ai_settings::AiSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-shield-check"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.mfa_settings") }</div>
                                            <button class="info-btn" title={i18n.t("mfa_settings.mfa_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::mfa_settings::MFAOptions />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-download-simple"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.export_backup_podcasts") }</div>
                                            <button class="info-btn" title={i18n.t("export_settings.export_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::export_settings::ExportOptions />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-upload-simple"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.import_podcasts") }</div>
                                            <button class="info-btn" title={i18n.t("import_options.import_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::import_options::ImportOptions />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-monitor"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.display_settings") }</div>
                                            <button class="info-btn" title={i18n.t("start_page_options.start_page_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::start_page_options::StartPageOptions />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-bell-ringing"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.notification_settings") }</div>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::notifications::NotificationOptions />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-rss"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.add_custom_feed") }</div>
                                            <button class="info-btn" title={i18n.t("custom_feed.add_feed_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::custom_feed::CustomFeed />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-hard-drives"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.add_local_podcast") }</div>
                                            <button class="info-btn" title={i18n.t("local_podcast.description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::local_podcast::LocalPodcast />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-arrows-clockwise"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.podcast_sync") }</div>
                                            <button class="info-btn" title={i18n.t("nextcloud_options.gpodder_sync_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::nextcloud_options::SyncOptions />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-wifi"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.enable_disable_rss_feeds") }</div>
                                            <button class="info-btn" title={i18n.t("rss_feeds.rss_feed_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::rss_feeds::RSSFeedSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-magnifying-glass"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.match_podcasts_podcast_index") }</div>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::podcast_index_matching::PodcastIndexMatching />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-key"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.api_keys") }</div>
                                            <button class="info-btn" title={i18n.t("api_keys.api_keys_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::api_keys::APIKeys />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-share-network"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.shared_links") }</div>
                                            <button class="info-btn" title={i18n.t("shared_links.description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::shared_links::SharedLinks />
                                </div>
                            </div>
                            </>
                        }
                    } else if *active_tab == "admin" {
                        html! {
                            <>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-users"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.user_management") }</div>
                                            <button class="info-btn" title={i18n.t("settings.user_management_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::user_settings::UserSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-lock-key"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.oidc_sso_settings") }</div>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::oidc::OIDCSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-download-simple"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.download_settings") }</div>
                                            <button class="info-btn" title={i18n.t("download_settings.server_download_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::download_settings::DownloadSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-gear"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.user_self_service_settings") }</div>
                                            <button class="info-btn" title={i18n.t("user_self_service.self_service_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::user_self_service::SelfServiceSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-envelope"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.email_settings") }</div>
                                            <button class="info-btn" title={i18n.t("email_settings.email_settings_description").to_string()}>{"?"}</button>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::email_settings::EmailSettings />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-database"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.backup_server") }</div>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::backup_server::BackupServer />
                                </div>
                            </div>
                            <div class="settings-section">
                                <div class="settings-section-head">
                                    <i class="ph ph-database-backup"></i>
                                    <div>
                                        <div class="settings-section-title-row">
                                            <div class="settings-section-title">{ &i18n.t("settings.restore_server") }</div>
                                        </div>
                                    </div>
                                </div>
                                <div class="settings-section-body">
                                    <setting_components::restore_server::RestoreServer />
                                </div>
                            </div>
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
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
