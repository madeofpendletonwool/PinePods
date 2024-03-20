use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use yew::platform::spawn_local;
use web_sys::console;
use crate::requests::setting_reqs::{call_guest_status, call_enable_disable_guest};
use std::borrow::Borrow;


#[function_component(GuestSettings)]
pub fn guest_settings() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let _user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let _error_message = state.error_message.clone();
    let guest_status = use_state(|| false);

    {
        let guest_status = guest_status.clone();
        use_effect_with((api_key.clone(), server_name.clone()), move |(api_key, server_name)| {
            let guest_status = guest_status.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let future = async move {
                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let response = call_guest_status(server_name, api_key.unwrap()).await;
                    match response {
                        Ok(guest_status_response) => {
                            guest_status.set(guest_status_response);
                        },
                        Err(e) => console::log_1(&format!("Error getting guest status: {}", e).into()),
                    }
                }
            };
            spawn_local(future);
            // Return cleanup function
            || {}
        });
    }
    let html_guest = guest_status.clone();
    let loading = use_state(|| false);


    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{"Guest User Settings:"}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{"You can choose to enable or disable the Guest user here. It's always disabled by default. Basically, enabling the guest user enables a button on the login page to login as guest. This guest user essentially has access to add podcasts and listen to them in an ephemeral sense. Once logged out, the session is deleted along with any podcasts the Guest saved. If your Pinepods server is exposed to the internet you probably want to disable this option. It's meant more for demos or if you want to allow people to quickly listen to a podcast using your server."}</p> // Styled paragraph

            <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" disabled={**loading.borrow()} checked={**guest_status.borrow()} class="sr-only peer" onclick={Callback::from(move |_| {
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
                            Err(e) => console::log_1(&format!("Error enabling/disabling guest access: {}", e).into()),
                        }
                    }
                    loading.set(false);
                };
                spawn_local(future);
            })} />
            <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
            <span class="ms-3 text-sm font-medium item_container-text">{
                if **guest_status.borrow() {
                    "Disable Guest User"
                } else {
                    "Enable Guest User"
                }
            }</span>
        </label>
        </div>
    }
}

