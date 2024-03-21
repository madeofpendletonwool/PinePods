use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::{AppState, UIState};
use yew::platform::spawn_local;
use web_sys::console;
use crate::requests::setting_reqs::{call_get_email_settings, EmailSettingsResponse, SendEmailSettings, call_save_email_settings, call_send_test_email, call_send_email, TestEmailSettings};
use std::ops::Deref;
// use crate::gen_components::_ErrorMessageProps::error_message;

#[function_component(EmailSettings)]
pub fn email_settings() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let _user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let user_email = state.user_details.as_ref().map(|ud| ud.Email.clone());
    let _error_message = audio_state.error_message.clone();
    let _info_message = audio_state.info_message.clone();
    let auth_required = use_state(|| false);
    web_sys::console::log_1(&"testlog".into());

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

    let server_name_ref = use_state(|| "".to_string());
    let server_port_ref = use_state(|| "".to_string());
    let from_email_ref = use_state(|| "".to_string());
    let send_mode_ref = use_state(|| "SMTP".to_string());
    let encryption_ref = use_state(|| "none".to_string());    
    let username_ref = use_state(|| "".to_string());
    let password_ref = use_state(|| "".to_string());

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
    }

    // Define the initial state
    let page_state = use_state(|| PageState::Hidden);
    let page_state_edit = page_state.clone();


    let on_server_name_change = {
        let server_name_ref = server_name_ref.clone();
        Callback::from(move |e: InputEvent| {
            server_name_ref.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_server_port_change = {
        let server_port_ref = server_port_ref.clone();
        Callback::from(move |e: InputEvent| {
            server_port_ref.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_from_email_change = {
        let from_email_ref = from_email_ref.clone();
        Callback::from(move |e: InputEvent| {
            from_email_ref.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_send_mode_change = {
        let send_mode_ref = send_mode_ref.clone();
        Callback::from(move |e: InputEvent| {
            let value = e.target_unchecked_into::<web_sys::HtmlInputElement>().value();
            console::log_1(&format!("Send mode changed to: {}", value).into());
            send_mode_ref.set(value);
        })
    };
    
    
    let on_encryption_change = {
        let encryption_ref = encryption_ref.clone();
        Callback::from(move |e: InputEvent| {
            encryption_ref.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_username_change = {
        let username_ref = username_ref.clone();
        Callback::from(move |e: InputEvent| {
            username_ref.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_password_change = {
        let password_ref = password_ref.clone();
        Callback::from(move |e: InputEvent| {
            password_ref.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    // Define the callback function for closing the modal
    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
        })
    };
    let edit_api_key = api_key.clone();
    let edit_server_name = server_name.clone();

    let on_edit_submit = {
        let server_name_ref = server_name_ref.clone();
        let server_port_ref = server_port_ref.clone();
        let from_email_ref = from_email_ref.clone();
        let send_mode_ref = send_mode_ref.clone();
        let encryption_ref = encryption_ref.clone();
        let username_ref = username_ref.clone();
        let password_ref = password_ref.clone();
        let auth_required = auth_required.clone();
        let page_state = page_state.clone();
        let audio_dispatch_call = audio_dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            let server_name = edit_server_name.clone();
            let server_name_ref = server_name_ref.clone().deref().to_string();
            // let server_name = server_name_ref.clone().deref().to_string();
            let server_port = server_port_ref.clone().deref().to_string();
            let from_email = from_email_ref.clone().deref().clone();
            let send_mode = send_mode_ref.clone().deref().clone();
            let encryption = encryption_ref.clone().deref().clone();
            let auth_required = *auth_required.clone();
            let email_username = username_ref.clone().deref().clone();
            let email_password = password_ref.clone().deref().clone();
            let server_name_future = server_name_ref.clone(); // Clone here
            let email_settings = crate::requests::setting_reqs::EmailSettings {
                server_name: server_name_future.clone(),
                server_port: server_port.to_string(),
                from_email: from_email.clone(),
                send_mode: send_mode.clone(),
                encryption: encryption.clone(),
                auth_required: auth_required,
                email_username: email_username.clone(),
                email_password: email_password.clone(),
            };
            // let server_name = server_name_ref.deref().clone();
            let api_key = edit_api_key.clone().unwrap_or_default();
            let future = async move {
                let _ = call_save_email_settings(server_name.unwrap(), api_key.unwrap().clone(), email_settings).await;
            };
            spawn_local(future);
            page_state.set(PageState::Hidden);
            audio_dispatch_call.reduce_mut(|audio_state| audio_state.info_message = Option::from("Email Settings Saved!".to_string()));
        })
    };

    // Define the modal components
    let verify_email_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700">
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
                            {"Email sent!"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <p class="text-m font-semibold text-gray-900 dark:text-white">
                    {"Once you verify you recieved it click Save below to save the email settings to the server."}
                    </p>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <button type="submit" onclick={on_edit_submit} class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Verify and Save Email Settings"}</button>
                            <button type="submit" onclick={on_close_modal} class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Cancel"}</button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };
    let audio_send_test = audio_dispatch.clone();
    let api_test = api_key.clone();
    let submit_email = user_email.clone();
    let on_submit = {
        let server_name = server_name.clone();
        let server_port_ref = server_port_ref.clone();
        let from_email_ref = from_email_ref.clone();
        let send_mode_ref = send_mode_ref.clone();
        let encryption_ref = encryption_ref.clone();
        let username_ref = username_ref.clone();
        let password_ref = password_ref.clone();
        let auth_required = auth_required.clone();
        let page_state = page_state_edit.clone();
        Callback::from(move |_: MouseEvent| {
            console::log_1(&"Button clicked".into());
            let audio_dispatch_call = audio_send_test.clone();
            let server_name = server_name.clone();
            let server_name_ref = server_name_ref.clone().deref().to_string();
            let server_port = server_port_ref.clone().deref().to_string();
            let from_email = from_email_ref.clone().deref().clone();
            let send_mode = send_mode_ref.clone().deref().clone();
            let encryption = encryption_ref.clone().deref().clone();
            let auth_required = *auth_required.clone();
            let email_username = username_ref.clone().deref().clone();
            let email_password = password_ref.clone().deref().clone();
            let page_state = page_state.clone();
            let test_email_settings = TestEmailSettings {
                server_name: server_name_ref.clone(),
                server_port: server_port.to_string(),
                from_email: from_email.clone(),
                send_mode: send_mode.clone(),
                encryption: encryption.clone(),
                auth_required: auth_required,
                email_username: email_username.clone(),
                email_password: email_password.clone(),
                to_email: submit_email.clone().unwrap().unwrap(),
                message: "If you got this email Pinepods emailing works! Be sure to verify your settings to confirm!".to_string(),
            };
            let server_name = server_name.clone();
            let api_key = api_test.clone().unwrap_or_default();
            let future = async move {
                let send_email_result = call_send_test_email(server_name.clone().unwrap(), api_key.clone().unwrap(), test_email_settings.clone()).await;
                match send_email_result {
                    Ok(_) => {
                        page_state.set(PageState::Shown);
                    },
                    Err(e) => {
                        console::log_1(&format!("Error: {}", e).into());
                        audio_dispatch_call.reduce_mut(|audio_state| audio_state.error_message = Option::from(format!("Error: {}", e)));
                        // Handle the error, e.g., by showing an error message to the user
                    }
                }
            };
            spawn_local(future);
        })
    };

    let on_test_email_send = {
        let server_name = server_name.clone(); // Assuming you have these values in your component's state
        let api_key = api_key.clone(); // Assuming you have API key in your component's state
        let audio_dispatch_call = audio_dispatch.clone();
        
        Callback::from(move |_: MouseEvent| {
            let api_key = api_key.clone();
            let server_name = server_name.clone().unwrap_or_default(); // Ensure server_name has a default value if it's an Option
            let audio_dispatch_call = audio_dispatch_call.clone();
            // Setting up the email settings. Adjust these values as necessary.
            let email_settings = SendEmailSettings {
                to_email: user_email.clone().unwrap().unwrap(), // This should be dynamically set based on your application's needs
                subject: "Test Pinepods Email".to_string(),
                message: "This is an email from Pinepods. If you got this your email setup works!".to_string(),
            };
    
            let future = async move {
                match call_send_email(server_name, api_key.unwrap_or_default().unwrap(), email_settings).await {
                    Ok(_) => {
                        audio_dispatch_call.reduce_mut(|audio_state| audio_state.info_message = Option::from("Email sent successfully!".to_string()));
                        console::log_1(&"Email sent successfully!".into());
                        // Optionally, use dispatch_callback to update a global state or trigger other app-wide effects
                    },
                    Err(e) => {
                        console::log_1(&format!("Error: {}", e).into());
                        audio_dispatch_call.reduce_mut(|audio_state| audio_state.error_message = Option::from(format!("Error: {}", e)));
                        // Handle the error, e.g., by updating a state with the error message
                    }
                }
            };
    
            spawn_local(future);
        })
    };

    html! {
        <>
        {
            match *page_state {
            PageState::Shown => verify_email_modal,
            _ => html! {},
            }
        }
        <div class="p-4">
            <h2 class="item_container-text text-xl font-bold mb-4">{"Email Setup:"}</h2>
            <p class="item_container-text text-md mb-4">{"You can setup server Email settings here. Email is mostly used for self service password resets. The server will require that you verify your email settings setup before it will allow you to submit the settings you've entered."}</p>
            <p class="item_container-text text-lg font-bold mb-4">{"Current Settings:"}</p>
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
                            <tr class="table-row border-b">
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
            <button onclick={on_test_email_send} class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline mr-2">
                {"Test Current Settings"}
            </button>
        
        </div>
        <p class="item_container-text text-lg font-bold mb-4">{"Update Settings:"}</p>

        <div class="flex mt-4">
            <input oninput={on_server_name_change.clone()} type="text" placeholder="Server Name" class="email-input border p-2 mr-2 rounded"/>
            <span class="item_container-text">{":"}</span>
            <input oninput={on_server_port_change.clone()} type="text" placeholder="Port" class="email-input border p-2 ml-2 rounded"/>
        </div>

        <div class="mt-4">
            <p class="item_container-text font-medium">{"Send Mode:"}</p>
            <select oninput={on_send_mode_change.clone()} class="email-select border p-2 rounded mr-2">
                <option value="SMTP" selected=true>{"SMTP"}</option>
            </select>
        </div>
        <div class="mt-4">
            <p class="item_container-text font-medium">{"Encryption:"}</p>
            <select oninput={on_encryption_change.clone()} class="email-select border p-2 rounded">
                <option value="none" selected=true>{"None"}</option>
                <option>{"SSL/TLS"}</option>
                <option>{"StartTLS"}</option>
            </select>
        </div>

        <input oninput={on_from_email_change.clone()} type="text" placeholder="From Address" class="email-input border p-2 mt-4 rounded"/>

        <div class="flex items-center mt-4">
            <input type="checkbox" id="auth_required" checked={*auth_required} onclick={toggle_auth_required}/>
            <label for="auth_required" class="item_container-text ml-2">{"Authentication Required"}</label>
        </div>
        {
            if *auth_required {
                html! {
                                <>
                                    <input oninput={on_username_change.clone()} type="text" placeholder="Username" class="email-input border p-2 mt-4 rounded"/>
                                    <input oninput={on_password_change.clone()} type="password" placeholder="Password" class="email-input border p-2 mt-4 rounded"/>
                                </>
                            }
            } else {
                html! {}
            }
        }
        <div class="flex mt-4">
            <button onclick={on_submit} class="settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline mr-2">
                {"Test & Submit"}
            </button>
        </div>
        </div>
        </>
    }
}
