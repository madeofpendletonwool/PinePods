use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use yew::platform::spawn_local;
use crate::requests::setting_reqs::call_get_user_info;
use web_sys::console;
use std::borrow::Borrow;
use crate::requests::setting_reqs::{APIInfo, APIInfoResponse, call_get_api_info, call_create_api_key};
use crate::components::gen_funcs::encode_password;
use crate::components::gen_funcs::validate_user_input;
// use crate::gen_components::_ErrorMessageProps::error_message;

#[function_component(APIKeys)]
pub fn api_keys() -> Html {

    let (state, dispatch) = use_store::<AppState>();
    let effect_dispatch = dispatch.clone();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let error_message = state.error_message.clone();
    let api_infos = use_state(|| Vec::new());
    let api_key_modal_visible = use_state(|| false);
    let new_api_key = use_state(|| String::new());
    // Define the type of user in the Vec
    // let users: UseStateHandle<Vec<SettingsUser>> = use_state(|| Vec::new());

    {
        let api_infos = api_infos.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();

        use_effect_with((api_key, server_name), move |(api_key, server_name)| {
            let api_infos = api_infos.clone();
            let api_key_cloned = api_key.clone();
            let server_name_cloned = server_name.clone();
    
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(api_key) = api_key_cloned {
                    if let Some(server_name) = server_name_cloned {
                        match call_get_api_info(server_name, user_id.unwrap(), api_key.unwrap()).await {
                            Ok(response) => {
                                api_infos.set(response.api_info);
                            },
                            Err(e) => {
                                console::log_1(&format!("Error getting API info: {}", e).into());
                            }
                        }
                    }
                }
            });
    
            || ()
        });
    }

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
    }

    // Define the initial state
    let page_state = use_state(|| PageState::Hidden);



    // Define the function to close the modal
    let close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
        })
    };

    // Define the function to open the modal and request a new API key
    let request_api_key = {
        let page_state = page_state.clone();
        let new_api_key = new_api_key.clone();
        let api_key = api_key.clone();
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = server_name.clone();
        // Assume you have user_id and api_key from context or props
        let user_id = 1; // Example user_id
        Callback::from(move |_| {
            let api_key = api_key.clone();
            let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
            let server_name = server_name.clone();
            let page_state = page_state.clone();
            let new_api_key = new_api_key.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_create_api_key(&server_name.unwrap(), user_id.unwrap(), &api_key.unwrap().unwrap()).await {
                    Ok(response) => {
                        new_api_key.set(response.api_key);
                        page_state.set(PageState::Shown); // Move to the edit page state
                    },
                    Err(e) => console::log_1(&e.to_string().into()),
                }
            });
        })
    };
    let api_key_display = (*new_api_key).clone();


    let create_api_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700">
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex flex-col items-start justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <button onclick={close_modal.clone()} class="self-end text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
                            {"New Api Key Created"}
                        </h3>
                        <p class="text-m font-semibold text-gray-900 dark:text-white">
                        {"Copy the API Key Listed Below. Be sure to save it in a safe place. You will only ever be able to view it once. You can always just create a new one if you lose it."}
                        </p>
                        <div class="mt-4 bg-gray-100 p-4 rounded-md overflow-x-auto whitespace-nowrap max-w-full">
                            {api_key_display}
                        </div>
                        <button onclick={close_modal} class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                            {"OK"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    };

    


    html! {
        <>
        {
            match *page_state {
            PageState::Shown => create_api_modal,
            _ => html! {},
            }
        }
            <div class="p-4">
                <p class="text-lg font-bold mb-4">{"API Keys:"}</p>
                <p class="text-md mb-4">{"You can request a Pinepods API Key here. These keys can then be used in conjunction with other Pinepods apps (like Pinepods Firewood) to connect them to the Pinepods server. In addition, you can also use an API Key to authenticate to this server from any other Pinepods server. Sort of like using a different server as a client for this one."}</p>
                <button onclick={request_api_key} class="mt-4 bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" type="button">
                    {"Request API Key"}
                </button>
            </div>
            <div class="relative overflow-x-auto">
                <table class="w-full text-sm text-left rtl:text-right text-gray-500 dark:text-gray-400">
                    <thead class="text-xs uppercase table-header">
                        <tr>
                            <th scope="col" class="px-6 py-3">{"API ID"}</th>
                            <th scope="col" class="px-6 py-3">{"Last 4 Digits"}</th>
                            <th scope="col" class="px-6 py-3">{"Date Created"}</th>
                            <th scope="col" class="px-6 py-3">{"User"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {
                            for (*api_infos).iter().map(|api_info| html! {
                                <tr class="table-row border-b cursor-pointer">
                                    <td class="px-6 py-4">{ api_info.APIKeyID }</td>
                                    <td class="px-6 py-4">{ api_info.LastFourDigits.clone() }</td>
                                    <td class="px-6 py-4">{ api_info.Created.clone() }</td>
                                    <td class="px-6 py-4">{ api_info.Username.clone() }</td>
                                </tr>
                            })
                        }
                    </tbody>
                </table>
            </div>
        </>
    }
}