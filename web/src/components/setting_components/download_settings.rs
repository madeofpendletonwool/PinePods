use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use yew::platform::spawn_local;
use web_sys::console;
use crate::requests::setting_reqs::{call_download_status, call_enable_disable_downloads};
use std::borrow::Borrow;

#[function_component(DownloadSettings)]
pub fn download_settings() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let _user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let _error_message = state.error_message.clone();
    let download_status = use_state(|| false);

    {
        let download_status = download_status.clone();
        use_effect_with((api_key.clone(), server_name.clone()), move |(api_key, server_name)| {
            let download_status = download_status.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let future = async move {
                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let response = call_download_status(server_name, api_key.unwrap()).await;
                    match response {
                        Ok(download_status_response) => {
                            download_status.set(download_status_response);
                        },
                        Err(e) => console::log_1(&format!("Error getting download status: {}", e).into()),
                    }
                }
            };
            spawn_local(future);
            // Return cleanup function
            || {}
        });
    }
    let html_download = download_status.clone();
    let loading = use_state(|| false);

    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{"Server Download Settings:"}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{"You can choose to enable or disable server downloads here. This does not effect local downloads. There's two types of downloads in Pinepods. Local and Server. Local downloads would be where a user clicks download and it downloads the podcast to their local machine. A server download is when a user downloads the podcast to the server specifically. This is meant as an archival option. If you're concerned the podcast may not be always available you may want to archive it using this option. See the Pinepods documentation for mapping a specific location (like a NAS) as the location server downloads download to. You might want to turn this option off if you have self service enabled or your Pinepods server accessible to the internet. You wouldn't want any random user filling up your server."}</p> // Styled paragraph
            <label class="relative inline-flex items-center cursor-pointer">
            <input type="checkbox" disabled={**loading.borrow()} checked={**download_status.borrow()} class="sr-only peer" onclick={Callback::from(move |_| {
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
                            Err(e) => console::log_1(&format!("Error enabling/disabling downloads: {}", e).into()),
                        }
                    }
                    loading.set(false);
                };
                spawn_local(future);
            })} />
                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                <span class="ms-3 text-sm font-medium item_container-text">{"Enable Server Downloads"}</span>
            </label>
        </div>
    }
}


