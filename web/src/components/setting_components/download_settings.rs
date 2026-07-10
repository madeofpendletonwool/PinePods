use crate::components::context::{AppState, NotificationState};
use crate::requests::setting_reqs::{
    call_download_status, call_enable_disable_downloads, call_get_download_metadata_settings,
    call_set_download_metadata_settings, DownloadMetadataSettings,
};
use std::borrow::Borrow;
use wasm_bindgen::JsCast;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[function_component(DownloadSettings)]
pub fn download_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let _user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let download_status = use_state(|| false);
    // The admin download-metadata options (#451/#533/#658). None until loaded.
    let meta_settings = use_state(|| None::<DownloadMetadataSettings>);
    let _dispatch_effect = _dispatch.clone();

    // Capture i18n strings before they get moved
    let i18n_enable_server_downloads = i18n.t("download_settings.enable_server_downloads").to_string();
    let i18n_error_getting_download_status = i18n.t("download_settings.error_getting_download_status").to_string();
    let i18n_error_enabling_disabling_downloads = i18n.t("download_settings.error_enabling_disabling_downloads").to_string();
    let i18n_folder_cover_label = i18n.t("download_settings.folder_cover_label").to_string();
    let i18n_folder_cover_description = i18n.t("download_settings.folder_cover_description").to_string();
    let i18n_episode_cover_label = i18n.t("download_settings.episode_cover_label").to_string();
    let i18n_episode_cover_description = i18n.t("download_settings.episode_cover_description").to_string();
    let i18n_metadata_sidecar_label = i18n.t("download_settings.metadata_sidecar_label").to_string();
    let i18n_metadata_sidecar_description = i18n.t("download_settings.metadata_sidecar_description").to_string();
    let i18n_metadata_format_label = i18n.t("download_settings.metadata_format_label").to_string();
    let i18n_metadata_format_json = i18n.t("download_settings.metadata_format_json").to_string();
    let i18n_metadata_format_xml = i18n.t("download_settings.metadata_format_xml").to_string();
    let i18n_metadata_format_ffmetadata = i18n.t("download_settings.metadata_format_ffmetadata").to_string();
    let i18n_metadata_format_both = i18n.t("download_settings.metadata_format_both").to_string();
    let i18n_metadata_subfolder_label = i18n.t("download_settings.metadata_subfolder_label").to_string();
    let i18n_error_getting_metadata_settings = i18n.t("download_settings.error_getting_metadata_settings").to_string();
    let i18n_error_saving_metadata_settings = i18n.t("download_settings.error_saving_metadata_settings").to_string();

    // Load both the download master toggle and the metadata settings on mount.
    {
        let download_status = download_status.clone();
        let meta_settings = meta_settings.clone();
        let status_error_prefix = i18n_error_getting_download_status.clone();
        let meta_error_prefix = i18n_error_getting_metadata_settings.clone();
        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let download_status = download_status.clone();
                let meta_settings = meta_settings.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let future = async move {
                    if let (Some(Some(api_key)), Some(server_name)) = (api_key, server_name) {
                        match call_download_status(server_name.clone(), api_key.clone()).await {
                            Ok(status) => download_status.set(status),
                            Err(e) => {
                                let error_msg = format!("{}{}", status_error_prefix, e);
                                Dispatch::<NotificationState>::global().reduce_mut(|s| {
                                    s.error_message = Option::from(error_msg)
                                });
                            }
                        }
                        match call_get_download_metadata_settings(server_name, api_key).await {
                            Ok(settings) => meta_settings.set(Some(settings)),
                            Err(e) => {
                                let error_msg = format!("{}{}", meta_error_prefix, e);
                                Dispatch::<NotificationState>::global().reduce_mut(|s| {
                                    s.error_message = Option::from(error_msg)
                                });
                            }
                        }
                    }
                };
                spawn_local(future);
                || {}
            },
        );
    }

    let html_download = download_status.clone();
    let loading = use_state(|| false);

    // Persist a new metadata-settings value: update local state, then POST it.
    let persist_meta = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let meta_settings = meta_settings.clone();
        let save_error_prefix = i18n_error_saving_metadata_settings.clone();
        Callback::from(move |new_settings: DownloadMetadataSettings| {
            meta_settings.set(Some(new_settings.clone()));
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let save_error_prefix = save_error_prefix.clone();
            spawn_local(async move {
                if let (Some(Some(api_key)), Some(server_name)) = (api_key, server_name) {
                    if let Err(e) =
                        call_set_download_metadata_settings(server_name, api_key, &new_settings).await
                    {
                        Dispatch::<NotificationState>::global().reduce_mut(|s| {
                            s.error_message = Option::from(format!("{}{}", save_error_prefix, e))
                        });
                    }
                }
            });
        })
    };

    // Checkbox handler factory: flips one boolean field of the current settings.
    let make_bool_toggle = {
        let meta_settings = meta_settings.clone();
        let persist_meta = persist_meta.clone();
        move |field: &'static str| {
            let meta_settings = meta_settings.clone();
            let persist_meta = persist_meta.clone();
            Callback::from(move |e: Event| {
                let checked = e
                    .target()
                    .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                    .map(|i| i.checked())
                    .unwrap_or(false);
                if let Some(current) = (*meta_settings).clone() {
                    let mut next = current;
                    match field {
                        "folder_cover" => next.folder_cover = checked,
                        "episode_cover" => next.episode_cover = checked,
                        "metadata_sidecar" => next.metadata_sidecar = checked,
                        "metadata_subfolder" => next.metadata_subfolder = checked,
                        _ => {}
                    }
                    persist_meta.emit(next);
                }
            })
        }
    };

    let on_format_change = {
        let meta_settings = meta_settings.clone();
        let persist_meta = persist_meta.clone();
        Callback::from(move |e: Event| {
            let value = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok())
                .map(|s| s.value())
                .unwrap_or_else(|| "both".to_string());
            if let Some(current) = (*meta_settings).clone() {
                let mut next = current;
                next.metadata_format = value;
                persist_meta.emit(next);
            }
        })
    };

    // Snapshot of current metadata settings for rendering.
    let ms = (*meta_settings).clone();
    let folder_cover = ms.as_ref().map(|s| s.folder_cover).unwrap_or(false);
    let episode_cover = ms.as_ref().map(|s| s.episode_cover).unwrap_or(false);
    let metadata_sidecar = ms.as_ref().map(|s| s.metadata_sidecar).unwrap_or(false);
    let metadata_subfolder = ms.as_ref().map(|s| s.metadata_subfolder).unwrap_or(true);
    let metadata_format = ms
        .as_ref()
        .map(|s| s.metadata_format.clone())
        .unwrap_or_else(|| "both".to_string());
    let settings_loaded = ms.is_some();

    html! {
        <>
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_enable_server_downloads}</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input type="checkbox" disabled={**loading.borrow()} checked={**download_status.borrow()} onclick={{
                        let error_prefix = i18n_error_enabling_disabling_downloads.clone();
                        let api_key = api_key.clone();
                        let server_name = server_name.clone();
                        Callback::from(move |_| {
                            let error_prefix = error_prefix.clone();
                            let api_key = api_key.clone();
                            let server_name = server_name.clone();
                            let download_status = html_download.clone();
                            let loading = loading.clone();
                            let future = async move {
                                loading.set(true);
                                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                                    let response = call_enable_disable_downloads(server_name, api_key.unwrap()).await;
                                    match response {
                                        Ok(_) => {
                                            let current_status = download_status.borrow().clone();
                                            download_status.set(!*current_status);
                                        },
                                        Err(e) => {
                                            Dispatch::<NotificationState>::global().reduce_mut(|audio_state| audio_state.error_message = Option::from(format!("{}{}", error_prefix.clone(), e)));
                                        },
                                    }
                                }
                                loading.set(false);
                            };
                            spawn_local(future);
                        })
                    }} />
                    <span class="toggle-track"><span class="toggle-thumb"></span></span>
                </label>
            </div>
        </div>

        // Podcast cover as folder.jpg (#658)
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_folder_cover_label}</div>
                <div class="settings-row-description">{&i18n_folder_cover_description}</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input type="checkbox" disabled={!settings_loaded} checked={folder_cover}
                        onchange={make_bool_toggle("folder_cover")} />
                    <span class="toggle-track"><span class="toggle-thumb"></span></span>
                </label>
            </div>
        </div>

        // Episode cover sidecar image (#451)
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_episode_cover_label}</div>
                <div class="settings-row-description">{&i18n_episode_cover_description}</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input type="checkbox" disabled={!settings_loaded} checked={episode_cover}
                        onchange={make_bool_toggle("episode_cover")} />
                    <span class="toggle-track"><span class="toggle-thumb"></span></span>
                </label>
            </div>
        </div>

        // Episode metadata sidecar (#451)
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_metadata_sidecar_label}</div>
                <div class="settings-row-description">{&i18n_metadata_sidecar_description}</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input type="checkbox" disabled={!settings_loaded} checked={metadata_sidecar}
                        onchange={make_bool_toggle("metadata_sidecar")} />
                    <span class="toggle-track"><span class="toggle-thumb"></span></span>
                </label>
            </div>
        </div>

        // Metadata format + subfolder options only apply when the sidecar is enabled.
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_metadata_format_label}</div>
            </div>
            <div class="settings-row-control">
                <select class="select" disabled={!settings_loaded || !metadata_sidecar}
                    onchange={on_format_change}>
                    <option value="json" selected={metadata_format == "json"}>{&i18n_metadata_format_json}</option>
                    <option value="xml" selected={metadata_format == "xml"}>{&i18n_metadata_format_xml}</option>
                    <option value="ffmetadata" selected={metadata_format == "ffmetadata"}>{&i18n_metadata_format_ffmetadata}</option>
                    <option value="both" selected={metadata_format == "both"}>{&i18n_metadata_format_both}</option>
                </select>
            </div>
        </div>

        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_metadata_subfolder_label}</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input type="checkbox" disabled={!settings_loaded || !metadata_sidecar} checked={metadata_subfolder}
                        onchange={make_bool_toggle("metadata_subfolder")} />
                    <span class="toggle-track"><span class="toggle-thumb"></span></span>
                </label>
            </div>
        </div>
        </>
    }
}
