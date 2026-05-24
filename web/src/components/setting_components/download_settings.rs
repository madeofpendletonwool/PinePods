use crate::components::context::{AppState, NotificationState};
use crate::requests::setting_reqs::{call_download_status, call_enable_disable_downloads};
use std::borrow::Borrow;
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
    let dispatch_effect = _dispatch.clone();

    // Capture i18n strings before they get moved
    let i18n_enable_server_downloads = i18n.t("download_settings.enable_server_downloads").to_string();
    let i18n_error_getting_download_status = i18n.t("download_settings.error_getting_download_status").to_string();
    let i18n_error_enabling_disabling_downloads = i18n.t("download_settings.error_enabling_disabling_downloads").to_string();

    {
        let download_status = download_status.clone();
        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let download_status = download_status.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let error_prefix = i18n_error_getting_download_status.clone();
                let future = async move {
                    if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                        let response = call_download_status(server_name, api_key.unwrap()).await;
                        match response {
                            Ok(download_status_response) => {
                                download_status.set(download_status_response);
                            }
                            Err(e) => {
                                let error_msg = format!("{}{}", error_prefix, e);
                                Dispatch::<NotificationState>::global().reduce_mut(|audio_state| {
                                    audio_state.error_message = Option::from(error_msg)
                                });
                            }
                        }
                    }
                };
                spawn_local(future);
                // Return cleanup function
                || {}
            },
        );
    }
    let html_download = download_status.clone();
    let loading = use_state(|| false);

    html! {
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_enable_server_downloads}</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input type="checkbox" disabled={**loading.borrow()} checked={**download_status.borrow()} onclick={{
                        let error_prefix = i18n_error_enabling_disabling_downloads.clone();
                        Callback::from(move |_| {
                            let error_prefix = error_prefix.clone();
                            let api_key = api_key.clone();
                            let server_name = server_name.clone();
                            let download_status = html_download.clone();
                            let _dispatch = _dispatch.clone();
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
    }
}
