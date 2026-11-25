//! Settings of the desktop app itself, kept on this computer.

use crate::components::context::NotificationState;
use crate::components::i18n::use_translation;
use crate::desktop::{self, DesktopState, Settings};
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Properties, PartialEq)]
pub struct DesktopSettingsProps {
    /// Which card: "desktop_window", "desktop_downloads", "desktop_notifications" or
    /// "desktop_updates".
    pub section: AttrValue,
}

fn save_failed(message: String) {
    desktop::log_error(message.clone());
    Dispatch::<NotificationState>::global().reduce_mut(move |s| s.error_message = Some(message));
}

#[derive(Properties, PartialEq)]
struct ToggleRowProps {
    label: AttrValue,
    description: AttrValue,
    checked: bool,
    #[prop_or_default]
    disabled: bool,
    onchange: Callback<bool>,
}

#[function_component(ToggleRow)]
fn toggle_row(props: &ToggleRowProps) -> Html {
    let checked = props.checked;
    let onchange = props.onchange.reform(move |_: MouseEvent| !checked);
    html! {
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{ &props.label }</div>
                <div class="settings-row-desc">{ &props.description }</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input type="checkbox" checked={props.checked} disabled={props.disabled} onclick={onchange} />
                    <span class="toggle-track"><span class="toggle-thumb"></span></span>
                </label>
            </div>
        </div>
    }
}

#[function_component(DesktopSettings)]
pub fn desktop_settings(props: &DesktopSettingsProps) -> Html {
    let (i18n, _) = use_translation();
    let state = use_store_value::<DesktopState>();
    let checking = use_state(|| false);
    let checked_once = use_state(|| false);
    let Some(settings) = state.settings.clone() else { return html! {} };
    let failed = i18n.t("desktop.settings_save_failed").to_string();

    // A toggle that changes one setting.
    let toggle = |change: fn(&mut Settings, bool)| {
        let failed = failed.clone();
        Callback::from(move |on: bool| {
            let failed = failed.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = desktop::update_settings(|s| change(s, on)).await {
                    save_failed(format!("{failed}: {e}"));
                }
            });
        })
    };

    match props.section.as_str() {
        "desktop_window" => {
            let on_login = {
                let failed = failed.clone();
                Callback::from(move |on: bool| {
                    let failed = failed.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Err(e) = desktop::set_launch_at_login(on).await {
                            save_failed(format!("{failed}: {e}"));
                        }
                    });
                })
            };
            let on_mini = Callback::from(|_: MouseEvent| {
                wasm_bindgen_futures::spawn_local(async {
                    if let Err(e) = desktop::open_mini_player().await {
                        desktop::log_error(format!("could not open the mini player: {e}"));
                    }
                });
            });
            html! {
                <>
                    <ToggleRow
                        label={i18n.t("desktop.close_to_tray")}
                        description={i18n.t("desktop.close_to_tray_desc")}
                        checked={settings.close_to_tray}
                        onchange={toggle(|s, on| s.close_to_tray = on)}
                    />
                    <ToggleRow
                        label={i18n.t("desktop.launch_at_login")}
                        description={i18n.t("desktop.launch_at_login_desc")}
                        checked={state.launch_at_login}
                        onchange={on_login}
                    />
                    <div class="settings-row">
                        <div>
                            <div class="settings-row-label">{ i18n.t("desktop.mini_player") }</div>
                            <div class="settings-row-desc">{ i18n.t("desktop.mini_player_desc") }</div>
                        </div>
                        <div class="settings-row-control">
                            <button type="button" class="btn btn-secondary" onclick={on_mini}>
                                { i18n.t("desktop.open_mini_player") }
                            </button>
                        </div>
                    </div>
                </>
            }
        }
        "desktop_downloads" => html! {
            <>
                <ToggleRow
                    label={i18n.t("desktop.delete_finished")}
                    description={i18n.t("desktop.delete_finished_desc")}
                    checked={settings.delete_finished_downloads}
                    onchange={toggle(|s, on| s.delete_finished_downloads = on)}
                />
                <div class="settings-row">
                    <div>
                        <div class="settings-row-label">{ i18n.t("desktop.auto_download") }</div>
                        <div class="settings-row-desc">
                            { i18n.t("desktop.auto_download_settings_desc").replace("{count}", &settings.auto_download.len().to_string()) }
                        </div>
                    </div>
                </div>
            </>
        },
        "desktop_notifications" => html! {
            <>
                <ToggleRow
                    label={i18n.t("desktop.notify_new_episodes")}
                    description={i18n.t("desktop.notify_new_episodes_desc")}
                    checked={settings.notify_new_episodes}
                    onchange={toggle(|s, on| s.notify_new_episodes = on)}
                />
                <ToggleRow
                    label={i18n.t("desktop.notify_downloads")}
                    description={i18n.t("desktop.notify_downloads_desc")}
                    checked={settings.notify_downloads}
                    onchange={toggle(|s, on| s.notify_downloads = on)}
                />
            </>
        },
        "desktop_updates" => {
            if !state.updates_supported {
                return html! {
                    <div class="settings-row">
                        <div class="settings-row-desc">{ i18n.t("desktop.updates_from_package") }</div>
                    </div>
                };
            }
            let on_check = {
                let (checking, checked_once) = (checking.clone(), checked_once.clone());
                let failed = i18n.t("desktop.update_check_failed").to_string();
                Callback::from(move |_: MouseEvent| {
                    checking.set(true);
                    let (checking, checked_once, failed) = (checking.clone(), checked_once.clone(), failed.clone());
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Err(e) = desktop::check_for_update().await {
                            save_failed(format!("{failed}: {e}"));
                        }
                        checking.set(false);
                        checked_once.set(true);
                    });
                })
            };
            let status = match (&state.update, *checked_once) {
                (Some(update), _) => i18n.t("desktop.update_available").replace("{version}", &update.version),
                (None, true) => i18n.t("desktop.up_to_date").to_string(),
                (None, false) => String::new(),
            };
            html! {
                <>
                    <ToggleRow
                        label={i18n.t("desktop.check_for_updates")}
                        description={i18n.t("desktop.check_for_updates_desc")}
                        checked={settings.check_for_updates}
                        onchange={toggle(|s, on| s.check_for_updates = on)}
                    />
                    <div class="settings-row">
                        <div>
                            <div class="settings-row-label">{ i18n.t("desktop.check_now") }</div>
                            <div class="settings-row-desc">{ status }</div>
                        </div>
                        <div class="settings-row-control">
                            <button type="button" class="btn btn-secondary" onclick={on_check} disabled={*checking}>
                                { i18n.t("desktop.check_now") }
                            </button>
                        </div>
                    </div>
                </>
            }
        }
        _ => html! {},
    }
}

#[derive(Properties, PartialEq)]
pub struct PodcastAutoDownloadProps {
    pub podcast_id: i32,
}

const KEEP_CHOICES: [u32; 6] = [0, 1, 3, 5, 10, 20];

/// Desktop app, in a podcast's options: keep its newest episodes downloaded on this computer.
#[function_component(PodcastAutoDownload)]
pub fn podcast_auto_download(props: &PodcastAutoDownloadProps) -> Html {
    let (i18n, _) = use_translation();
    let podcast_id = props.podcast_id;
    let keep = use_selector(move |s: &DesktopState| {
        s.settings.as_ref().map(|s| s.auto_download.get(&podcast_id).copied().unwrap_or(0))
    });
    let Some(keep) = *keep else { return html! {} };
    if !desktop::is_desktop() || podcast_id <= 0 {
        return html! {};
    }
    let failed = i18n.t("desktop.settings_save_failed").to_string();
    let on_change = Callback::from(move |e: Event| {
        let value = e.target_unchecked_into::<web_sys::HtmlSelectElement>().value().parse::<u32>().unwrap_or(0);
        let failed = failed.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = desktop::update_settings(|s| {
                if value == 0 {
                    s.auto_download.remove(&podcast_id);
                } else {
                    s.auto_download.insert(podcast_id, value);
                }
            })
            .await;
            if let Err(e) = result {
                save_failed(format!("{failed}: {e}"));
            }
        });
    });
    let mut choices = KEEP_CHOICES.to_vec();
    if !choices.contains(&keep) {
        choices.push(keep);
        choices.sort();
    }
    let off = i18n.t("desktop.auto_download_off").to_string();
    let newest = i18n.t("desktop.auto_download_newest").to_string();
    html! {
        <div>
            <label class="block mb-2 text-sm font-medium">{ i18n.t("desktop.auto_download") }</label>
            <select class="select" style="width:220px;" onchange={on_change}>
                { for choices.iter().map(|n| html! {
                    <option value={n.to_string()} selected={*n == keep}>
                        { if *n == 0 { off.clone() } else { newest.replace("{count}", &n.to_string()) } }
                    </option>
                }) }
            </select>
            <p class="text-xs mt-1 item_container-text">{ i18n.t("desktop.auto_download_desc") }</p>
        </div>
    }
}
