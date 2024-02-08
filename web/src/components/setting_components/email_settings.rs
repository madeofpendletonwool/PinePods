use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::AppState;
use yew::platform::spawn_local;
use crate::requests::setting_reqs::call_get_user_info;
use web_sys::console;
use std::borrow::Borrow;
use crate::requests::setting_reqs::{call_get_email_settings, EmailSettingsResponse, EmailSettingsRequest, call_save_email_settings};
use crate::components::gen_funcs::encode_password;
use crate::components::gen_funcs::validate_user_input;
// use crate::gen_components::_ErrorMessageProps::error_message;

#[function_component(EmailSettings)]
pub fn email_settings() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let effect_dispatch = dispatch.clone();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let error_message = state.error_message.clone();
    let auth_required = use_state(|| false);

    let toggle_auth_required = {
        let auth_required = auth_required.clone();
        Callback::from(move |_| auth_required.set(!*auth_required))
    };
        // Define the type of user in the Vec
        let email_values: UseStateHandle<EmailSettingsResponse> = use_state(EmailSettingsResponse::default);

    {
        let email_values = email_values.clone();
        use_effect_with((api_key.clone(), server_name.clone()), move |(api_key, server_name)| {
            let email_values = email_values.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let future = async move {
                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let response = call_get_email_settings(server_name, api_key.unwrap()).await;
                    match response {
                        Ok(email_info) => {
                            email_values.set(email_info);
                        },
                        Err(e) => console::log_1(&format!("Error getting user info: {}", e).into()),
                    }
                }
            };
            spawn_local(future);
            // Return cleanup function
            || {}
        });
    }

    html! {
        <div class="p-4">
            <h2 class="text-lg font-bold mb-4">{"Email Setup:"}</h2>
            <p class="text-md mb-4">{"You can setup server Email settings here. Email is mostly used for self service password resets. The server will require that you verify your email settings setup before it will allow you to submit the settings you've entered."}</p>
            <p class="text-lg font-bold mb-4">{"Current Settings:"}</p>
            <div class="relative overflow-x-auto">
                <table class="w-full text-sm text-left rtl:text-right text-gray-500 dark:text-gray-400">
                    <thead class="text-xs uppercase table-header">
                        <tr>
                            <th scope="col" class="px-6 py-3">{"Server"}</th>
                            <th scope="col" class="px-6 py-3">{"From Email"}</th>
                            <th scope="col" class="px-6 py-3">{"Send Mode"}</th>
                            <th scope="col" class="px-6 py-3">{"Encryption"}</th>
                            <th scope="col" class="px-6 py-3">{"Auth Required"}</th>
                            <th scope="col" class="px-6 py-3">{"Username"}</th>
                        </tr>
                    </thead>
                    <tbody>
                    {
                        // Access the state directly without let binding inside html!
                        html! {
                            <tr class="table-row border-b cursor-pointer">
                                <td class="px-6 py-4">{ format!("{}:{}", &email_values.Server_Name, &email_values.Server_Port) }</td>
                                <td class="px-6 py-4">{ &email_values.From_Email }</td>
                                <td class="px-6 py-4">{ &email_values.Send_Mode }</td>
                                <td class="px-6 py-4">{ &email_values.Encryption }</td>
                                <td class="px-6 py-4">{ if email_values.Auth_Required == 1 { "Yes" } else { "No" } }</td>
                                <td class="px-6 py-4">{ &email_values.Username }</td>
                            </tr>
                        }
                    }
                    </tbody>
                </table>
            </div>
            <div class="flex mt-4">
            <button class="bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline mr-2" type="button">
                {"Test Current Settings"}
            </button>
        
        </div>
        <p class="text-lg font-bold mb-4">{"Update Settings:"}</p>

        <div class="flex mt-4">
            <input type="text" placeholder="Server Name" class="border p-2 mr-2 rounded"/>
            <span>{":"}</span>
            <input type="text" placeholder="Port" class="border p-2 ml-2 rounded"/>
        </div>

        <div class="mt-4">
            <p class="font-medium">{"Send Mode:"}</p>
            <select class="border p-2 rounded mr-2">
                <option>{"SMTP"}</option>
            </select>
        </div>
        <div class="mt-4">
            <p class="font-medium">{"Encryption:"}</p>
            <select class="border p-2 rounded">
                <option value="none" selected=true>{"None"}</option>
                <option>{"SSL/TLS"}</option>
                <option>{"StartTLS"}</option>
            </select>
        </div>

        <input type="text" placeholder="From Address" class="border p-2 mt-4 rounded"/>

        <div class="flex items-center mt-4">
            <input type="checkbox" id="auth_required" checked={*auth_required} onclick={toggle_auth_required}/>
            <label for="auth_required" class="ml-2">{"Authentication Required"}</label>
        </div>
        {
            if *auth_required {
                html! {
                                <>
                                    <input type="text" placeholder="Username" class="border p-2 mt-4 rounded"/>
                                    <input type="password" placeholder="Password" class="border p-2 mt-4 rounded"/>
                                </>
                            }
            } else {
                html! {}
            }
        }
        <div class="flex mt-4">
            <button class="bg-green-500 hover:bg-green-700 text-white font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline mr-2" type="button">
                {"Test & Submit"}
            </button>
        </div>
        </div>
    }
}
