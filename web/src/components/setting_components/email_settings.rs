use crate::components::context::AppState;
use crate::requests::setting_reqs::{
    call_get_email_settings, call_save_email_settings, call_send_test_email,
    EmailSettingsResponse, TestEmailSettings,
};
use std::ops::Deref;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(EmailSettings)]
pub fn email_settings() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let user_email = state.user_details.as_ref().map(|ud| ud.Email.clone());

    // Current settings from database
    let current_settings: UseStateHandle<EmailSettingsResponse> = use_state(EmailSettingsResponse::default);
    
    // Form inputs
    let form_server_name = use_state(|| "".to_string());
    let form_server_port = use_state(|| "587".to_string());
    let form_from_email = use_state(|| "".to_string());
    let form_send_mode = use_state(|| "SMTP".to_string());
    let form_encryption = use_state(|| "StartTLS".to_string());
    let form_username = use_state(|| "".to_string());
    let form_password = use_state(|| "".to_string());
    let form_auth_required = use_state(|| true);
    
    // UI state
    let is_testing = use_state(|| false);
    let test_success = use_state(|| false);
    let is_saving = use_state(|| false);

    // Load current settings on component mount
    {
        let current_settings = current_settings.clone();
        let form_server_name = form_server_name.clone();
        let form_server_port = form_server_port.clone();
        let form_from_email = form_from_email.clone();
        let form_encryption = form_encryption.clone();
        let form_username = form_username.clone();
        let form_auth_required = form_auth_required.clone();
        let dispatch = dispatch.clone();
        
        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let current_settings = current_settings.clone();
                let form_server_name = form_server_name.clone();
                let form_server_port = form_server_port.clone();
                let form_from_email = form_from_email.clone();
                let form_encryption = form_encryption.clone();
                let form_username = form_username.clone();
                let form_auth_required = form_auth_required.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let dispatch = dispatch.clone();
                
                spawn_local(async move {
                    if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                        match call_get_email_settings(server_name, api_key.unwrap()).await {
                            Ok(settings) => {
                                // Populate form with current settings
                                form_server_name.set(settings.ServerName.clone());
                                form_server_port.set(settings.ServerPort.to_string());
                                form_from_email.set(settings.FromEmail.clone());
                                form_encryption.set(settings.Encryption.clone());
                                form_username.set(settings.Username.clone());
                                form_auth_required.set(settings.AuthRequired == 1);
                                current_settings.set(settings);
                            }
                            Err(e) => {
                                dispatch.reduce_mut(|state| {
                                    state.error_message = Some(format!("Error loading email settings: {}", e));
                                });
                            }
                        }
                    }
                });
                || {}
            },
        );
    }

    // Input change handlers
    let on_server_name_change = {
        let form_server_name = form_server_name.clone();
        Callback::from(move |e: InputEvent| {
            form_server_name.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_server_port_change = {
        let form_server_port = form_server_port.clone();
        Callback::from(move |e: InputEvent| {
            form_server_port.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_from_email_change = {
        let form_from_email = form_from_email.clone();
        Callback::from(move |e: InputEvent| {
            form_from_email.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_encryption_change = {
        let form_encryption = form_encryption.clone();
        Callback::from(move |e: Event| {
            let target = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            form_encryption.set(target.value());
        })
    };

    let on_username_change = {
        let form_username = form_username.clone();
        Callback::from(move |e: InputEvent| {
            form_username.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_password_change = {
        let form_password = form_password.clone();
        Callback::from(move |e: InputEvent| {
            form_password.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_auth_required_change = {
        let form_auth_required = form_auth_required.clone();
        Callback::from(move |_| {
            form_auth_required.set(!*form_auth_required);
        })
    };

    // Test email functionality
    let on_test_email = {
        let form_server_name = form_server_name.clone();
        let form_server_port = form_server_port.clone();
        let form_from_email = form_from_email.clone();
        let form_encryption = form_encryption.clone();
        let form_username = form_username.clone();
        let form_password = form_password.clone();
        let form_auth_required = form_auth_required.clone();
        let is_testing = is_testing.clone();
        let test_success = test_success.clone();
        let dispatch = dispatch.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_email = user_email.clone();

        Callback::from(move |_: MouseEvent| {
            let form_server_name = form_server_name.clone();
            let form_server_port = form_server_port.clone();
            let form_from_email = form_from_email.clone();
            let form_encryption = form_encryption.clone();
            let form_username = form_username.clone();
            let form_password = form_password.clone();
            let form_auth_required = form_auth_required.clone();
            let is_testing = is_testing.clone();
            let test_success = test_success.clone();
            let dispatch = dispatch.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let user_email = user_email.clone();

            spawn_local(async move {
                is_testing.set(true);
                test_success.set(false);

                if let (Some(api_key), Some(server_name), Some(user_email)) = 
                    (api_key, server_name, user_email) {
                    
                    let test_settings = TestEmailSettings {
                        server_name: form_server_name.deref().clone(),
                        server_port: form_server_port.deref().clone(),
                        from_email: form_from_email.deref().clone(),
                        send_mode: "SMTP".to_string(),
                        encryption: form_encryption.deref().clone(),
                        auth_required: *form_auth_required,
                        email_username: form_username.deref().clone(),
                        email_password: form_password.deref().clone(),
                        to_email: user_email.unwrap(),
                        message: "This is a test email from PinePods! If you received this, your email settings are working correctly.".to_string(),
                    };

                    match call_send_test_email(server_name, api_key.unwrap(), test_settings).await {
                        Ok(_) => {
                            test_success.set(true);
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some("Test email sent successfully! Check your inbox.".to_string());
                            });
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Failed to send test email: {}", e));
                            });
                        }
                    }
                }
                is_testing.set(false);
            });
        })
    };

    // Save settings functionality
    let on_save_settings = {
        let form_server_name = form_server_name.clone();
        let form_server_port = form_server_port.clone();
        let form_from_email = form_from_email.clone();
        let form_encryption = form_encryption.clone();
        let form_username = form_username.clone();
        let form_password = form_password.clone();
        let form_auth_required = form_auth_required.clone();
        let is_saving = is_saving.clone();
        let dispatch = dispatch.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();

        Callback::from(move |_: MouseEvent| {
            let form_server_name = form_server_name.clone();
            let form_server_port = form_server_port.clone();
            let form_from_email = form_from_email.clone();
            let form_encryption = form_encryption.clone();
            let form_username = form_username.clone();
            let form_password = form_password.clone();
            let form_auth_required = form_auth_required.clone();
            let is_saving = is_saving.clone();
            let dispatch = dispatch.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();

            spawn_local(async move {
                is_saving.set(true);

                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let email_settings = crate::requests::setting_reqs::EmailSettings {
                        server_name: form_server_name.deref().clone(),
                        server_port: form_server_port.deref().clone(),
                        from_email: form_from_email.deref().clone(),
                        send_mode: "SMTP".to_string(),
                        encryption: form_encryption.deref().clone(),
                        auth_required: *form_auth_required,
                        email_username: form_username.deref().clone(),
                        email_password: form_password.deref().clone(),
                    };

                    match call_save_email_settings(server_name, api_key.unwrap(), email_settings).await {
                        Ok(_) => {
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some("Email settings saved successfully!".to_string());
                            });
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("Failed to save email settings: {}", e));
                            });
                        }
                    }
                }
                is_saving.set(false);
            });
        })
    };

    html! {
        <div class="max-w-4xl mx-auto p-6 bg-white dark:bg-gray-800 rounded-lg shadow-lg">
            <div class="mb-8">
                <h2 class="text-2xl font-bold text-gray-900 dark:text-white mb-2">{"Email Settings"}</h2>
                <p class="text-gray-600 dark:text-gray-400">
                    {"Configure your SMTP email settings for password resets and notifications. Test your settings before saving to ensure they work correctly."}
                </p>
            </div>

            // Current Settings Display
            <div class="mb-8 p-4 bg-gray-50 dark:bg-gray-700 rounded-lg">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white mb-4">{"Current Settings"}</h3>
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 text-sm">
                    <div>
                        <span class="font-medium text-gray-700 dark:text-gray-300">{"Server:"}</span>
                        <span class="ml-2 text-gray-600 dark:text-gray-400">
                            {format!("{}:{}", current_settings.ServerName, current_settings.ServerPort)}
                        </span>
                    </div>
                    <div>
                        <span class="font-medium text-gray-700 dark:text-gray-300">{"From Email:"}</span>
                        <span class="ml-2 text-gray-600 dark:text-gray-400">{&current_settings.FromEmail}</span>
                    </div>
                    <div>
                        <span class="font-medium text-gray-700 dark:text-gray-300">{"Encryption:"}</span>
                        <span class="ml-2 text-gray-600 dark:text-gray-400">{&current_settings.Encryption}</span>
                    </div>
                    <div>
                        <span class="font-medium text-gray-700 dark:text-gray-300">{"Auth Required:"}</span>
                        <span class="ml-2 text-gray-600 dark:text-gray-400">
                            {if current_settings.AuthRequired == 1 { "Yes" } else { "No" }}
                        </span>
                    </div>
                    <div>
                        <span class="font-medium text-gray-700 dark:text-gray-300">{"Username:"}</span>
                        <span class="ml-2 text-gray-600 dark:text-gray-400">{&current_settings.Username}</span>
                    </div>
                </div>
            </div>

            // Settings Form
            <div class="space-y-6">
                <h3 class="text-lg font-semibold text-gray-900 dark:text-white">{"Update Settings"}</h3>
                
                // Server Configuration
                <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                    <div>
                        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            {"SMTP Server"}
                        </label>
                        <input
                            type="text"
                            placeholder="smtp.gmail.com"
                            value={form_server_name.deref().clone()}
                            oninput={on_server_name_change}
                            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                        />
                    </div>
                    <div>
                        <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                            {"Port"}
                        </label>
                        <input
                            type="number"
                            placeholder="587"
                            value={form_server_port.deref().clone()}
                            oninput={on_server_port_change}
                            class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                        />
                    </div>
                </div>

                // From Email
                <div>
                    <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                        {"From Email Address"}
                    </label>
                    <input
                        type="email"
                        placeholder="noreply@yourdomain.com"
                        value={form_from_email.deref().clone()}
                        oninput={on_from_email_change}
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                    />
                </div>

                // Encryption
                <div>
                    <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                        {"Encryption"}
                    </label>
                    <select
                        value={form_encryption.deref().clone()}
                        onchange={on_encryption_change}
                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                    >
                        <option value="none">{"None"}</option>
                        <option value="SSL/TLS">{"SSL/TLS"}</option>
                        <option value="StartTLS">{"StartTLS"}</option>
                    </select>
                </div>

                // Authentication
                <div class="space-y-4">
                    <div class="flex items-center">
                        <input
                            type="checkbox"
                            id="auth_required"
                            checked={*form_auth_required}
                            onclick={on_auth_required_change}
                            class="h-4 w-4 text-blue-600 focus:ring-blue-500 border-gray-300 rounded"
                        />
                        <label for="auth_required" class="ml-2 block text-sm text-gray-700 dark:text-gray-300">
                            {"Authentication Required"}
                        </label>
                    </div>

                    {if *form_auth_required {
                        html! {
                            <div class="grid grid-cols-1 md:grid-cols-2 gap-4 pl-6">
                                <div>
                                    <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                        {"Username"}
                                    </label>
                                    <input
                                        type="text"
                                        placeholder="your-email@domain.com"
                                        value={form_username.deref().clone()}
                                        oninput={on_username_change}
                                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                                    />
                                </div>
                                <div>
                                    <label class="block text-sm font-medium text-gray-700 dark:text-gray-300 mb-2">
                                        {"Password"}
                                    </label>
                                    <input
                                        type="password"
                                        placeholder="Your password or app password"
                                        value={form_password.deref().clone()}
                                        oninput={on_password_change}
                                        class="w-full px-3 py-2 border border-gray-300 dark:border-gray-600 rounded-md shadow-sm focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent dark:bg-gray-700 dark:text-white"
                                    />
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>

                // Action Buttons
                <div class="flex flex-col sm:flex-row gap-4 pt-6 border-t border-gray-200 dark:border-gray-600">
                    <button
                        onclick={on_test_email}
                        disabled={*is_testing}
                        class="inline-flex items-center justify-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-blue-600 hover:bg-blue-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-blue-500 disabled:opacity-50 disabled:cursor-not-allowed"
                    >
                        {if *is_testing {
                            html! {
                                <>
                                    <svg class="animate-spin -ml-1 mr-3 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    {"Testing..."}
                                </>
                            }
                        } else {
                            html! { "Test Email Settings" }
                        }}
                    </button>

                    {if *test_success {
                        html! {
                            <button
                                onclick={on_save_settings}
                                disabled={*is_saving}
                                class="inline-flex items-center justify-center px-4 py-2 border border-transparent text-sm font-medium rounded-md text-white bg-green-600 hover:bg-green-700 focus:outline-none focus:ring-2 focus:ring-offset-2 focus:ring-green-500 disabled:opacity-50 disabled:cursor-not-allowed"
                            >
                                {if *is_saving {
                                    html! {
                                        <>
                                            <svg class="animate-spin -ml-1 mr-3 h-4 w-4 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                            </svg>
                                            {"Saving..."}
                                        </>
                                    }
                                } else {
                                    html! { "Save Settings" }
                                }}
                            </button>
                        }
                    } else {
                        html! {}
                    }}
                </div>

                // Help Text
                <div class="mt-6 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
                    <h4 class="text-sm font-medium text-blue-900 dark:text-blue-100 mb-2">{"Common SMTP Settings:"}</h4>
                    <div class="text-xs text-blue-800 dark:text-blue-200 space-y-1">
                        <div>{"Gmail: smtp.gmail.com:587 (StartTLS) or smtp.gmail.com:465 (SSL/TLS)"}</div>
                        <div>{"Outlook: smtp-mail.outlook.com:587 (StartTLS)"}</div>
                        <div>{"Yahoo: smtp.mail.yahoo.com:587 (StartTLS)"}</div>
                        <div class="mt-2 font-medium">{"Note: Gmail requires an App Password instead of your regular password."}</div>
                    </div>
                </div>
            </div>
        </div>
    }
}