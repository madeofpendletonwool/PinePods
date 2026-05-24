use crate::components::context::{AppState, NotificationState};
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{call_enable_disable_self_service, call_self_service_status};
use std::borrow::Borrow;
use web_sys::console;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[function_component(SelfServiceSettings)]
pub fn self_service_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let _user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    // Capture i18n strings before they get moved
    let i18n_enable_user_self_service = i18n.t("user_self_service.enable_user_self_service").to_string();

    let self_service_status = use_state(|| false);

    {
        let self_service_status = self_service_status.clone();
        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let self_service_status = self_service_status.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let future = async move {
                    if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                        let response =
                            call_self_service_status(server_name, api_key.unwrap()).await;
                        match response {
                            Ok(self_service_status_response) => {
                                self_service_status.set(self_service_status_response);
                            }
                            Err(e) => console::log_1(
                                &format!("Error getting self service status: {}", e).into(),
                            ),
                        }
                    }
                };
                spawn_local(future);
                // Return cleanup function
                || {}
            },
        );
    }
    let html_self_service = self_service_status.clone();
    let loading = use_state(|| false);

    html! {
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_enable_user_self_service}</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input type="checkbox" disabled={**loading.borrow()} checked={**self_service_status.borrow()} onclick={Callback::from(move |_| {
                        let api_key = api_key.clone();
                        let server_name = server_name.clone();
                        let self_service_status = html_self_service.clone();
                        let _dispatch = _dispatch.clone();
                        let loading = loading.clone();
                        let future = async move {
                            loading.set(true);
                            if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                                let response = call_enable_disable_self_service(server_name, api_key.unwrap()).await;
                                match response {
                                    Ok(_) => {
                                        let current_status = self_service_status.borrow().clone();
                                        self_service_status.set(!*current_status);
                                    },
                                    Err(e) => {
                                        let formatted_error = format_error_message(&e.to_string());
                                        Dispatch::<NotificationState>::global().reduce_mut(|audio_state| audio_state.error_message = Option::from(format!("Error enabling/disabling self service: {}", formatted_error)));
                                    },
                                }
                            }
                            loading.set(false);
                        };
                        spawn_local(future);
                    })} />
                    <span class="toggle-track"><span class="toggle-thumb"></span></span>
                </label>
            </div>
        </div>
    }
}
