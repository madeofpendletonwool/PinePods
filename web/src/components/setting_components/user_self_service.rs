use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use yew::platform::spawn_local;
use web_sys::console;
use crate::requests::setting_reqs::{call_self_service_status, call_enable_disable_self_service};
use std::borrow::Borrow;

#[function_component(SelfServiceSettings)]
pub fn self_service_settings() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let _user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let _error_message = state.error_message.clone();
    let self_service_status = use_state(|| false);

    {
        let self_service_status = self_service_status.clone();
        use_effect_with((api_key.clone(), server_name.clone()), move |(api_key, server_name)| {
            let self_service_status = self_service_status.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let future = async move {
                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let response = call_self_service_status(server_name, api_key.unwrap()).await;
                    match response {
                        Ok(self_service_status_response) => {
                            self_service_status.set(self_service_status_response);
                        },
                        Err(e) => console::log_1(&format!("Error getting self service status: {}", e).into()),
                    }
                }
            };
            spawn_local(future);
            // Return cleanup function
            || {}
        });
    }
    let html_self_service = self_service_status.clone();
    let loading = use_state(|| false);

    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{"User Self Service Settings:"}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{"You can enable or disable user self service setup here. That is as it sounds. Once enabled there's a button on the login screen that allows users to set themselves up. It's highly recommended that if you enable this option you disable server downloads and setup the email settings so users can do self service password resets. If you'd rather not enable this you can just set new users up manually using User Settings above."}</p> // Styled paragraph

            <label class="relative inline-flex items-center cursor-pointer">
                <input type="checkbox" disabled={**loading.borrow()} checked={**self_service_status.borrow()} class="sr-only peer" onclick={Callback::from(move |_| {
                    let api_key = api_key.clone();
                    let server_name = server_name.clone();
                    let self_service_status = html_self_service.clone();
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
                                Err(e) => console::log_1(&format!("Error enabling/disabling self service: {}", e).into()),
                            }
                        }
                        loading.set(false);
                    };
                    spawn_local(future);
                })} />
                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                <span class="ms-3 text-sm font-medium item_container-text">{"Enable User Self Service"}</span>
            </label>
        </div>
    }
}