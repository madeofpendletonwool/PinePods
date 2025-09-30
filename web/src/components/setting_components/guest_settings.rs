use crate::components::context::AppState;
use crate::requests::setting_reqs::{call_enable_disable_guest, call_guest_status};
use std::borrow::Borrow;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[function_component(GuestSettings)]
pub fn guest_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let _user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let _error_message = state.error_message.clone();
    let guest_status = use_state(|| false);
    let dispatch_effect = _dispatch.clone();

    {
        let guest_status = guest_status.clone();
        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let guest_status = guest_status.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let future = async move {
                    if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                        let response = call_guest_status(server_name, api_key.unwrap()).await;
                        match response {
                            Ok(guest_status_response) => {
                                guest_status.set(guest_status_response);
                            }
                            Err(e) => {
                                dispatch_effect.reduce_mut(|audio_state| {
                                    audio_state.error_message =
                                        Option::from(format!("Error getting guest status: {}", e))
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
    let html_guest = guest_status.clone();
    let loading = use_state(|| false);

    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{i18n.t("settings.guest_user_settings")}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{i18n.t("settings.guest_user_description")}</p> // Styled paragraph

            <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" disabled={**loading.borrow()} checked={**guest_status.borrow()} class="sr-only peer" onclick={Callback::from(move |_| {
                let _dispatch = _dispatch.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let guest_status = html_guest.clone();
                let loading = loading.clone();
                let future = async move {
                    loading.set(true);
                    if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                        let response = call_enable_disable_guest(server_name, api_key.unwrap()).await;
                        match response {
                            Ok(_) => {
                                let current_status = guest_status.borrow().clone();
                                guest_status.set(!*current_status);
                            },

                            Err(e) => {
                                _dispatch.reduce_mut(|audio_state| audio_state.error_message = Option::from(format!("Error enabling/disabling guest access: {}", e)));
                            },
                        }
                    }
                    loading.set(false);
                };
                spawn_local(future);
            })} />
            <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
            <span class="ms-3 text-sm font-medium item_container-text">{
                if **guest_status.borrow() {
                    i18n.t("settings.disable_guest_user")
                } else {
                    i18n.t("settings.enable_guest_user")
                }
            }</span>
        </label>
        </div>
    }
}
