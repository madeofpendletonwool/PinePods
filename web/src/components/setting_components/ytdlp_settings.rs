// src/components/setting_components/ytdlp_settings.rs
//
// yt-dlp management (#793): admin-only. YouTube breaks whenever Google changes its internals and
// the fix is always a newer yt-dlp. Rather than rebuild/release the whole image for every yt-dlp
// bump, the locally-run binary self-updates — daily, at startup, and on demand here. This panel
// shows the current version, lets the admin pick the update channel (stable/nightly — nightly is
// the escape hatch, landing YouTube fixes the same day), toggle auto-update, and force an update.

use crate::components::context::{AppState, NotificationState};
use crate::requests::pod_req::{
    call_get_ytdlp_settings, call_trigger_ytdlp_update, call_update_ytdlp_settings,
    YtDlpSettingsUpdate,
};
use crate::components::i18n::use_translation;
use web_sys::HtmlSelectElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(YtDlpSettings)]
pub fn ytdlp_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().and_then(|ud| ud.api_key.clone());

    let is_admin = use_state(|| false);
    let version = use_state(|| Option::<String>::None);
    let auto_update = use_state(|| true);
    let channel = use_state(|| "stable".to_string());
    let last_updated = use_state(|| Option::<String>::None);
    let last_result = use_state(|| Option::<String>::None);
    let saving = use_state(|| false);
    let updating = use_state(|| false);
    let refresh = use_state(|| 0u32);

    {
        let is_admin = is_admin.clone();
        let version = version.clone();
        let auto_update = auto_update.clone();
        let channel = channel.clone();
        let last_updated = last_updated.clone();
        let last_result = last_result.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        use_effect_with(*refresh, move |_| {
            if let Some(server_name) = server_name {
                wasm_bindgen_futures::spawn_local(async move {
                    // Admin-only endpoint: success also tells us the user is an admin.
                    if let Ok(resp) = call_get_ytdlp_settings(&server_name, &api_key).await {
                        auto_update.set(resp.settings.auto_update);
                        channel.set(if resp.settings.channel.is_empty() {
                            "stable".to_string()
                        } else {
                            resp.settings.channel.clone()
                        });
                        last_updated.set(resp.settings.last_updated.clone());
                        last_result.set(resp.settings.last_result.clone());
                        version.set(resp.version.clone());
                        is_admin.set(true);
                    }
                });
            }
            || ()
        });
    }

    let on_toggle_auto = {
        let auto_update = auto_update.clone();
        Callback::from(move |_: Event| auto_update.set(!*auto_update))
    };

    let on_channel = {
        let channel = channel.clone();
        Callback::from(move |e: Event| {
            if let Some(sel) = e.target_dyn_into::<HtmlSelectElement>() {
                channel.set(sel.value());
            }
        })
    };

    let on_save = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let auto_update = auto_update.clone();
        let channel = channel.clone();
        let saving = saving.clone();
        let refresh = refresh.clone();
        let saved_msg = i18n.t("ytdlp_settings.saved").to_string();
        let err_msg = i18n.t("ytdlp_settings.save_error").to_string();
        Callback::from(move |_: MouseEvent| {
            let (server_name, api_key) = (server_name.clone(), api_key.clone());
            let update = YtDlpSettingsUpdate {
                auto_update: *auto_update,
                channel: (*channel).clone(),
            };
            let (saving, refresh) = (saving.clone(), refresh.clone());
            let (saved_msg, err_msg) = (saved_msg.clone(), err_msg.clone());
            saving.set(true);
            if let Some(server_name) = server_name {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_update_ytdlp_settings(&server_name, &api_key, &update).await {
                        Ok(_) => {
                            Dispatch::<NotificationState>::global()
                                .reduce_mut(|s| s.info_message = Some(saved_msg));
                            refresh.set(*refresh + 1);
                        }
                        Err(e) => Dispatch::<NotificationState>::global()
                            .reduce_mut(|s| s.error_message = Some(format!("{}: {}", err_msg, e))),
                    }
                    saving.set(false);
                });
            }
        })
    };

    let on_update_now = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let updating = updating.clone();
        let refresh = refresh.clone();
        let started = i18n.t("ytdlp_settings.update_started").to_string();
        let err_msg = i18n.t("ytdlp_settings.update_error").to_string();
        Callback::from(move |_: MouseEvent| {
            let (server_name, api_key) = (server_name.clone(), api_key.clone());
            let (updating, refresh) = (updating.clone(), refresh.clone());
            let (started, err_msg) = (started.clone(), err_msg.clone());
            updating.set(true);
            Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(started.clone()));
            if let Some(server_name) = server_name {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_trigger_ytdlp_update(&server_name, &api_key).await {
                        Ok(result) => {
                            if result.success {
                                Dispatch::<NotificationState>::global()
                                    .reduce_mut(|s| s.info_message = Some(result.detail));
                            } else {
                                Dispatch::<NotificationState>::global()
                                    .reduce_mut(|s| s.error_message = Some(result.detail));
                            }
                            refresh.set(*refresh + 1);
                        }
                        Err(e) => Dispatch::<NotificationState>::global()
                            .reduce_mut(|s| s.error_message = Some(format!("{}: {}", err_msg, e))),
                    }
                    updating.set(false);
                });
            }
        })
    };

    // Non-admins get nothing (the panel is admin-only, like AI settings).
    if !*is_admin {
        return html! {};
    }

    let version_label = match &*version {
        Some(v) => v.clone(),
        None => i18n.t("ytdlp_settings.version_unknown").to_string(),
    };
    let chan = (*channel).clone();

    html! {
        <>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("ytdlp_settings.current_version") }</div>
                    <div class="settings-row-desc">{ i18n.t("ytdlp_settings.intro") }</div>
                </div>
                <div class="settings-row-control">
                    <span style="font-weight:500; color: var(--text-color);">{ version_label }</span>
                </div>
            </div>

            {
                if let Some(lu) = (*last_updated).clone() {
                    let detail = (*last_result).clone().unwrap_or_default();
                    html! {
                        <div class="settings-row">
                            <div>
                                <div class="settings-row-label">{ i18n.t("ytdlp_settings.last_update") }</div>
                                <div class="settings-row-desc">{ detail }</div>
                            </div>
                            <div class="settings-row-control">
                                <span class="settings-row-desc">{ lu }</span>
                            </div>
                        </div>
                    }
                } else { html! {} }
            }

            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("ytdlp_settings.channel") }</div>
                    <div class="settings-row-desc">{ i18n.t("ytdlp_settings.channel_desc") }</div>
                </div>
                <div class="settings-row-control">
                    <select class="select" style="width:200px;max-width:60vw;" onchange={on_channel} value={chan.clone()}>
                        <option value="stable" selected={chan == "stable"}>{ i18n.t("ytdlp_settings.channel_stable") }</option>
                        <option value="nightly" selected={chan == "nightly"}>{ i18n.t("ytdlp_settings.channel_nightly") }</option>
                    </select>
                </div>
            </div>

            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("ytdlp_settings.auto_update") }</div>
                    <div class="settings-row-desc">{ i18n.t("ytdlp_settings.auto_update_desc") }</div>
                </div>
                <div class="settings-row-control">
                    <label class="toggle">
                        <input type="checkbox" checked={*auto_update} onchange={on_toggle_auto} />
                        <span class="toggle-track"><span class="toggle-thumb"></span></span>
                    </label>
                </div>
            </div>

            <div class="settings-row">
                <div></div>
                <div class="settings-row-control" style="gap:8px;">
                    <button class="btn btn-secondary" onclick={on_update_now} disabled={*updating}>
                        <i class="ph ph-arrows-clockwise"></i>
                        <span>{ if *updating { i18n.t("ytdlp_settings.updating") } else { i18n.t("ytdlp_settings.update_now") } }</span>
                    </button>
                    <button class="btn btn-primary" onclick={on_save} disabled={*saving}>
                        <i class="ph ph-floppy-disk"></i>
                        <span>{ if *saving { i18n.t("ytdlp_settings.saving") } else { i18n.t("ytdlp_settings.save") } }</span>
                    </button>
                </div>
            </div>
        </>
    }
}
