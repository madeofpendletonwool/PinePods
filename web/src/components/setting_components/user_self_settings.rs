use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::components::gen_funcs::{encode_password, validate_email, validate_username};
use crate::requests::setting_reqs::{call_get_my_user_info, MyUserInfo};
use crate::requests::setting_reqs::{
    call_set_email, call_set_fullname, call_set_password, call_set_username,
    call_update_timezone, call_update_date_format, call_update_time_format,
};
use chrono_tz::{Tz, TZ_VARIANTS};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(UserSelfSettings)]
pub fn user_self_settings() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID);

    // In the component
    let updated_fields = use_state(Vec::new);

    // UI States for messages and status
    let update_trigger = use_state(|| false);

    // Form and main states
    let username = use_state(|| "".to_string());
    let new_password = use_state(|| "".to_string());
    let confirm_password = use_state(String::new);
    let email = use_state(|| "".to_string());
    let fullname = use_state(|| "".to_string());
    
    // Time zone and format states
    let timezone = use_state(|| "".to_string());
    let date_format = use_state(|| "".to_string());
    let time_format = use_state(|| "".to_string()); // Default to empty string

    // Main user info state
    let user_info: UseStateHandle<Option<MyUserInfo>> = use_state(|| None);

    // Error states for validation
    let show_username_error = use_state(|| false);
    let show_email_error = use_state(|| false);
    let show_password_error = use_state(|| false);
    let show_password_match_error = use_state(|| false);

    // Success states match *error_container { match *error_container {
    let show_success = use_state(|| false);
    let success_message = use_state(|| "".to_string());

    // Single effect to fetch user info
    {
        let user_info = user_info.clone();
        let update_trigger = update_trigger.clone();
        let _dispatch = _dispatch.clone();

        use_effect_with(
            (api_key.clone(), server_name.clone(), *update_trigger), // Include update_trigger in dependencies
            move |(api_key, server_name, _)| {
                if let (Some(api_key), Some(server_name), Some(user_id)) =
                    (api_key.clone(), server_name.clone(), user_id)
                {
                    let server_name = server_name.clone();
                    let api_key = api_key.unwrap().clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match call_get_my_user_info(&server_name, api_key, user_id).await {
                            Ok(info) => {
                                user_info.set(Some(info));
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                _dispatch.reduce_mut(|state| {
                                    state.error_message = Some(format!(
                                        "Failed to fetch user info: {}",
                                        formatted_error
                                    ));
                                });
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    // Input handlers
    let on_username_change = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            username.set(target.value());
        })
    };

    let on_fullname_change = {
        let fullname = fullname.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            fullname.set(target.value());
        })
    };

    let on_email_change = {
        let email = email.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            email.set(target.value());
        })
    };

    let on_password_change = {
        let new_password = new_password.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            new_password.set(target.value());
        })
    };

    let on_confirm_password_change = {
        let confirm_password = confirm_password.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            confirm_password.set(target.value());
        })
    };

    let on_timezone_change = {
        let timezone = timezone.clone();
        Callback::from(move |e: Event| {
            let target = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            timezone.set(target.value());
        })
    };

    let on_date_format_change = {
        let date_format = date_format.clone();
        Callback::from(move |e: Event| {
            let target = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            date_format.set(target.value());
        })
    };

    let on_time_format_change = {
        let time_format = time_format.clone();
        Callback::from(move |e: Event| {
            let target = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            time_format.set(target.value());
        })
    };

    // Helper function to render timezone options
    fn render_time_zone_option(tz: Tz) -> Html {
        html! {
            <option value={tz.name()}>{tz.name()}</option>
        }
    }

    // Submit handler
    let on_submit = {
        let username = username.clone();
        let fullname = fullname.clone();
        let email = email.clone();
        let new_password = new_password.clone();
        let confirm_password = confirm_password.clone();
        let timezone = timezone.clone();
        let date_format = date_format.clone();
        let time_format = time_format.clone();
        let show_username_error = show_username_error.clone();
        let show_email_error = show_email_error.clone();
        let show_password_error = show_password_error.clone();
        let show_password_match_error = show_password_match_error.clone();
        let show_success = show_success.clone();
        let success_message = success_message.clone();
        let _dispatch = _dispatch.clone();
        let updated_fields_call = updated_fields.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            // let audio_dispatch_clone = audio_dispatch.clone();
            // let show_success_clone = show_success.clone();
            // let success_message_clone = success_message.clone();

            let server_name = state
                .auth_details
                .as_ref()
                .map(|ud| ud.server_name.clone())
                .unwrap_or_default();
            let api_key = state
                .auth_details
                .as_ref()
                .and_then(|ud| ud.api_key.clone())
                .unwrap_or_default();
            let user_id = state
                .user_details
                .as_ref()
                .map(|ud| ud.UserID)
                .unwrap_or_default();

            // Reset error states
            show_username_error.set(false);
            show_email_error.set(false);
            show_password_error.set(false);
            show_password_match_error.set(false);
            show_success.set(false);
            updated_fields_call.set(Vec::new());

            if !username.is_empty() {
                let errors = validate_username(&username);
                if !errors.is_empty() {
                    web_sys::console::log_1(&"Username validation failed".into());
                    show_username_error.set(true);
                    return;
                }

                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let _dispatch = _dispatch.clone();
                let updated_user = updated_fields_call.clone();
                let updated_trigger_call = update_trigger.clone();

                // Update username
                let username_clone = (*username).clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_set_username(server_name, api_key, user_id, username_clone).await {
                        Ok(_) => {
                            let mut fields = (*updated_user).clone();
                            fields.push("username");
                            updated_user.set(fields.clone());
                            show_success.set(true);
                            updated_trigger_call.set(!*updated_trigger_call);

                            success_message.set("Successfully updated user values".to_string());
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Username update failed: {}", e).into(),
                            );
                            let formatted_error = format_error_message(&e.to_string());
                            _dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to update username: {}", formatted_error));
                            });
                        }
                    }
                });
            }

            // Validate email only if something was entered in the field
            if !email.is_empty() {
                let errors = validate_email(&email);
                if !errors.is_empty() {
                    show_email_error.set(true);
                    return;
                }

                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let _dispatch = _dispatch.clone();
                let updated_user = updated_fields_call.clone();
                let updated_trigger_call = update_trigger.clone();

                let email_clone = (*email).clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_set_email(server_name, api_key, user_id, email_clone).await {
                        Ok(_) => {
                            let mut fields = (*updated_user).clone();
                            fields.push("email");
                            updated_user.set(fields.clone());
                            show_success.set(true);
                            updated_trigger_call.set(!*updated_trigger_call);

                            success_message.set("Successfully updated user values".to_string());
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            _dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to update email: {}", formatted_error));
                            });
                        }
                    }
                });
            }

            // Update fullname only if something was entered in the field
            if !fullname.is_empty() {
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let _dispatch = _dispatch.clone();
                let updated_user = updated_fields_call.clone();
                let updated_trigger_call = update_trigger.clone();

                let fullname_clone = (*fullname).clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_set_fullname(server_name, api_key, user_id, fullname_clone).await {
                        Ok(_) => {
                            let mut fields = (*updated_user).clone();
                            fields.push("fullname");
                            updated_user.set(fields.clone());
                            show_success.set(true);
                            updated_trigger_call.set(!*updated_trigger_call);

                            success_message.set("Successfully updated user values".to_string());
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            _dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to update full name: {}",
                                    formatted_error
                                ));
                            });
                        }
                    }
                });
            }

            // Handle password update if either password field has input
            if !new_password.is_empty() || !confirm_password.is_empty() {
                // Check if both fields are filled
                if new_password.is_empty() || confirm_password.is_empty() {
                    show_password_match_error.set(true);
                    return;
                }

                // Check if passwords match
                if *new_password != *confirm_password {
                    show_password_match_error.set(true);
                    return;
                }

                // Validate password length
                if new_password.len() < 6 {
                    show_password_error.set(true);
                    return;
                }

                // Only proceed if validation passed
                match encode_password(&new_password) {
                    Ok(hashed_password) => {
                        let server_name = server_name.clone();
                        let api_key = api_key.clone();
                        let show_success = show_success.clone();
                        let success_message = success_message.clone();
                        let _dispatch = _dispatch.clone();
                        let updated_trigger_call = update_trigger.clone();

                        wasm_bindgen_futures::spawn_local(async move {
                            match call_set_password(server_name, api_key, user_id, hashed_password)
                                .await
                            {
                                Ok(_) => {
                                    show_success.set(true);
                                    updated_trigger_call.set(!*updated_trigger_call);
                                    success_message
                                        .set("Successfully updated user values".to_string());
                                }
                                Err(e) => {
                                    let formatted_error = format_error_message(&e.to_string());
                                    _dispatch.reduce_mut(|state| {
                                        state.error_message = Some(format!(
                                            "Failed to update password: {}",
                                            formatted_error
                                        ));
                                    });
                                }
                            }
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        _dispatch.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Error encoding password: {}", formatted_error));
                        });
                    }
                }
            }

            // Update timezone if entered
            if !timezone.is_empty() {
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let _dispatch = _dispatch.clone();
                let updated_user = updated_fields_call.clone();
                let updated_trigger_call = update_trigger.clone();

                let timezone_clone = (*timezone).clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_update_timezone(server_name, api_key, user_id, timezone_clone).await {
                        Ok(_) => {
                            let mut fields = (*updated_user).clone();
                            fields.push("timezone");
                            updated_user.set(fields.clone());
                            show_success.set(true);
                            updated_trigger_call.set(!*updated_trigger_call);
                            success_message.set("Successfully updated user values".to_string());
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            _dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to update timezone: {}",
                                    formatted_error
                                ));
                            });
                        }
                    }
                });
            }

            // Update date format if entered
            if !date_format.is_empty() {
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let _dispatch = _dispatch.clone();
                let updated_user = updated_fields_call.clone();
                let updated_trigger_call = update_trigger.clone();

                let date_format_clone = (*date_format).clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_update_date_format(server_name, api_key, user_id, date_format_clone).await {
                        Ok(_) => {
                            let mut fields = (*updated_user).clone();
                            fields.push("date_format");
                            updated_user.set(fields.clone());
                            show_success.set(true);
                            updated_trigger_call.set(!*updated_trigger_call);
                            success_message.set("Successfully updated user values".to_string());
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            _dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to update date format: {}",
                                    formatted_error
                                ));
                            });
                        }
                    }
                });
            }

            // Update time format if entered
            if !time_format.is_empty() {
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let _dispatch = _dispatch.clone();
                let updated_user = updated_fields_call.clone();
                let updated_trigger_call = update_trigger.clone();

                // Parse string to integer for API call
                if let Ok(time_format_int) = time_format.parse::<i32>() {
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_update_time_format(server_name, api_key, user_id, time_format_int).await {
                            Ok(_) => {
                                let mut fields = (*updated_user).clone();
                                fields.push("time_format");
                                updated_user.set(fields.clone());
                                show_success.set(true);
                                updated_trigger_call.set(!*updated_trigger_call);
                                success_message.set("Successfully updated user values".to_string());
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                _dispatch.reduce_mut(|state| {
                                    state.error_message = Some(format!(
                                        "Failed to update time format: {}",
                                        formatted_error
                                    ));
                                });
                            }
                        }
                    });
                }
            }
        })
    };

    html! {
        <div class="user-settings-container">
            <div class="settings-header">
                <div class="flex items-center gap-4">
                    <i class="ph ph-user-circle text-2xl"></i>
                    <h2 class="text-xl font-semibold">{"Account Settings"}</h2>
                </div>
                if let Some(info) = &*user_info {
                    <div class="user-info-container mt-4 p-4 border border-solid border-opacity-10 rounded-lg overflow-hidden">
                        <div class="flex flex-col gap-4 lg:grid lg:grid-cols-1 xl:grid-cols-3 xl:gap-6">
                            <div class="min-w-0">
                                <span class="text-sm opacity-80">{"Current Username:"}</span>
                                <p class="font-medium mt-1 break-words truncate" title={info.username.clone()}>{&info.username}</p>
                            </div>
                            <div class="min-w-0">
                                <span class="text-sm opacity-80">{"Current Full Name:"}</span>
                                <p class="font-medium mt-1 break-words truncate" title={info.fullname.clone()}>{&info.fullname}</p>
                            </div>
                            <div class="min-w-0">
                                <span class="text-sm opacity-80">{"Current Email:"}</span>
                                <p class="font-medium mt-1 break-words truncate" title={info.email.clone()}>{&info.email}</p>
                            </div>
                        </div>
                    </div>
                }
            </div>

            <form onsubmit={on_submit} class="space-y-4">
                <div class="form-group">
                    <label for="username" class="form-label">{"Username"}</label>
                    <input
                        type="text"
                        id="username"
                        value={(*username).clone()}
                        oninput={on_username_change}
                        class="form-input"
                        placeholder="Enter username"
                    />
                    if *show_username_error {
                        <p class="error-text">{"Username must be at least 4 characters long"}</p>
                    }
                </div>

                <div class="form-group">
                    <label for="fullname" class="form-label">{"Full Name"}</label>
                    <input
                        type="text"
                        id="fullname"
                        value={(*fullname).clone()}
                        oninput={on_fullname_change}
                        class="form-input"
                        placeholder="Enter full name"
                    />
                </div>

                <div class="form-group">
                    <label for="email" class="form-label">{"Email"}</label>
                    <input
                        type="email"
                        id="email"
                        value={(*email).clone()}
                        oninput={on_email_change}
                        class="form-input"
                        placeholder="Enter email address"
                    />
                    if *show_email_error {
                        <p class="error-text">{"Please enter a valid email address"}</p>
                    }
                </div>

                <div class="password-section">
                    <h3 class="text-lg font-medium">{"Change Password"}</h3>
                    <div class="form-group">
                        <label for="new-password" class="form-label">{"New Password"}</label>
                        <input
                            type="password"
                            id="new-password"
                            value={(*new_password).clone()}
                            oninput={on_password_change}
                            class="form-input"
                            placeholder="Enter new password"
                        />
                        if *show_password_error {
                            <p class="error-text">{"Password must be at least 6 characters long"}</p>
                        }
                    </div>

                    <div class="form-group">
                        <label for="confirm-password" class="form-label">{"Confirm Password"}</label>
                        <input
                            type="password"
                            id="confirm-password"
                            value={(*confirm_password).clone()}
                            oninput={on_confirm_password_change}
                            class="form-input"
                            placeholder="Confirm new password"
                        />
                        if *show_password_match_error {
                            <p class="error-text">{"Passwords do not match"}</p>
                        }
                    </div>
                </div>

                <div class="timezone-section">
                    <h3 class="text-lg font-medium">{"Time & Date Settings"}</h3>
                    
                    <div class="form-group">
                        <label for="timezone" class="form-label">
                            <i class="ph ph-globe"></i>
                            {"Time Zone"}
                        </label>
                        <select
                            id="timezone"
                            class="form-input"
                            onchange={on_timezone_change}
                            value={(*timezone).clone()}
                        >
                            <option value="">{"Select timezone..."}</option>
                            { for TZ_VARIANTS.iter().map(|tz| render_time_zone_option(*tz)) }
                        </select>
                    </div>

                    <div class="form-group">
                        <label for="date_format" class="form-label">
                            <i class="ph ph-calendar"></i>
                            {"Date Format"}
                        </label>
                        <select
                            id="date_format"
                            class="form-input"
                            onchange={on_date_format_change}
                            value={(*date_format).clone()}
                        >
                            <option value="">{"Select date format..."}</option>
                            <option value="MDY">{"MM-DD-YYYY"}</option>
                            <option value="DMY">{"DD-MM-YYYY"}</option>
                            <option value="YMD">{"YYYY-MM-DD"}</option>
                            <option value="JUL">{"YY/DDD (Julian)"}</option>
                            <option value="ISO">{"ISO 8601"}</option>
                            <option value="USA">{"MM/DD/YYYY"}</option>
                            <option value="EUR">{"DD.MM.YYYY"}</option>
                            <option value="JIS">{"YYYY-MM-DD"}</option>
                        </select>
                    </div>

                    <div class="form-group">
                        <label for="time_format" class="form-label">
                            <i class="ph ph-clock-clockwise"></i>
                            {"Time Format"}
                        </label>
                        <select
                            id="time_format"
                            class="form-input"
                            onchange={on_time_format_change}
                            value={time_format.to_string()}
                        >
                            <option value="12">{"12 Hour"}</option>
                            <option value="24">{"24 Hour"}</option>
                        </select>
                    </div>
                </div>

                if *show_success {
                    <div class="success-message">
                        {(*success_message).clone()}
                    </div>
                }

                <button type="submit" class="submit-button">
                    <i class="ph ph-floppy-disk"></i>
                    {"Save Changes"}
                </button>
            </form>
        </div>
    }
}
