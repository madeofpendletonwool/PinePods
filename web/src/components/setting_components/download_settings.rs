use crate::components::context::AppState;
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
    let _error_message = state.error_message.clone();
    let download_status = use_state(|| false);
    let dispatch_effect = _dispatch.clone();

    // Capture i18n strings before they get moved
    let i18n_server_download_settings = i18n.t("download_settings.server_download_settings").to_string();
    let i18n_server_download_description = i18n.t("download_settings.server_download_description").to_string();
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
                                dispatch_effect.reduce_mut(|audio_state| {
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
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{&i18n_server_download_settings}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{&i18n_server_download_description}</p> // Styled paragraph
            <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" disabled={**loading.borrow()} checked={**download_status.borrow()} class="sr-only peer" onclick={{
                // Use pre-captured translated message
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
                                _dispatch.reduce_mut(|audio_state| audio_state.error_message = Option::from(format!("{}{}", error_prefix.clone(), e)));

                            },
                        }
                    }
                    loading.set(false);
                };
                spawn_local(future);
            })
            }} />
                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                <span class="ms-3 text-sm font-medium item_container-text">{&i18n_enable_server_downloads}</span>
            </label>
        </div>
    }
}
