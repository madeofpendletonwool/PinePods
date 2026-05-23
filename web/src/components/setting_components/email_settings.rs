use crate::components::context::AppState;
use crate::requests::setting_reqs::{
    call_get_email_settings, call_save_email_settings, call_send_test_email, EmailSettingsResponse,
    TestEmailSettings,
};
use i18nrs::yew::use_translation;
use std::ops::Deref;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(EmailSettings)]
pub fn email_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let user_email = state.user_details.as_ref().map(|ud| ud.Email.clone());

    // Current settings from database
    let current_settings: UseStateHandle<EmailSettingsResponse> =
        use_state(EmailSettingsResponse::default);

    // Form inputs
    let form_server_name = use_state(|| "".to_string());
    let form_server_port = use_state(|| "587".to_string());
    let form_from_email = use_state(|| "".to_string());
    let form_encryption = use_state(|| "StartTLS".to_string());
    let form_username = use_state(|| "".to_string());
    let form_password = use_state(|| "".to_string());
    let form_auth_required = use_state(|| true);

    // UI state
    let is_testing = use_state(|| false);
    let test_success = use_state(|| false);
    let is_saving = use_state(|| false);

    // Capture all i18n strings before any closures
    let i18n_error_loading_email_settings = i18n
        .t("email_settings.error_loading_email_settings")
        .to_string();
    let i18n_test_email_success = i18n.t("email_settings.test_email_success").to_string();
    let i18n_test_email_failed = i18n.t("email_settings.test_email_failed").to_string();
    let i18n_email_settings_saved = i18n.t("email_settings.email_settings_saved").to_string();
    let i18n_save_settings_failed = i18n.t("email_settings.save_settings_failed").to_string();
    let i18n_current_settings = i18n.t("email_settings.current_settings").to_string();
    let i18n_server = i18n.t("email_settings.server").to_string();
    let i18n_from_email = i18n.t("email_settings.from_email").to_string();
    let i18n_encryption = i18n.t("email_settings.encryption").to_string();
    let i18n_auth_required = i18n.t("email_settings.auth_required").to_string();
    let i18n_username = i18n.t("email_settings.username").to_string();
    let i18n_yes = i18n.t("email_settings.yes").to_string();
    let i18n_no = i18n.t("email_settings.no").to_string();
    let i18n_update_settings = i18n.t("email_settings.update_settings").to_string();
    let i18n_smtp_server = i18n.t("email_settings.smtp_server").to_string();
    let i18n_port = i18n.t("email_settings.port").to_string();
    let i18n_from_email_address = i18n.t("email_settings.from_email_address").to_string();
    let i18n_encryption_method = i18n.t("email_settings.encryption_method").to_string();
    let i18n_none = i18n.t("email_settings.none").to_string();
    let i18n_auth_username = i18n.t("email_settings.auth_username").to_string();
    let i18n_password = i18n.t("email_settings.password").to_string();
    let i18n_require_authentication = i18n.t("email_settings.require_authentication").to_string();
    let i18n_send_test_email = i18n.t("email_settings.send_test_email").to_string();
    let i18n_testing = i18n.t("email_settings.testing").to_string();
    let i18n_save_settings = i18n.t("email_settings.save_settings").to_string();
    let i18n_saving = i18n.t("email_settings.saving").to_string();

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
                                    state.error_message = Some(format!(
                                        "{}{}",
                                        i18n_error_loading_email_settings.clone(),
                                        e
                                    ));
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
            form_server_name.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_server_port_change = {
        let form_server_port = form_server_port.clone();
        Callback::from(move |e: InputEvent| {
            form_server_port.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_from_email_change = {
        let form_from_email = form_from_email.clone();
        Callback::from(move |e: InputEvent| {
            form_from_email.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
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
            form_username.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_password_change = {
        let form_password = form_password.clone();
        Callback::from(move |e: InputEvent| {
            form_password.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
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
            let i18n_test_email_success = i18n_test_email_success.clone();
            let i18n_test_email_failed = i18n_test_email_failed.clone();
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
                    (api_key, server_name, user_email)
                {
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
                                state.info_message = Some(i18n_test_email_success.clone());
                            });
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("{}{}", i18n_test_email_failed.clone(), e));
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
            let i18n_email_settings_saved = i18n_email_settings_saved.clone();
            let i18n_save_settings_failed = i18n_save_settings_failed.clone();
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

                    match call_save_email_settings(server_name, api_key.unwrap(), email_settings)
                        .await
                    {
                        Ok(_) => {
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some(i18n_email_settings_saved.clone());
                            });
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("{}{}", i18n_save_settings_failed.clone(), e));
                            });
                        }
                    }
                }
                is_saving.set(false);
            });
        })
    };

    html! {
        <>
            <div class="settings-subsection-title">{&i18n_current_settings}</div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_server}</div></div>
                <div class="settings-row-control">
                    <span style="font-size:13px;color:var(--text-color);">
                        {format!("{}:{}", current_settings.ServerName, current_settings.ServerPort)}
                    </span>
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_from_email}</div></div>
                <div class="settings-row-control">
                    <span style="font-size:13px;color:var(--text-color);">{&current_settings.FromEmail}</span>
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_encryption}</div></div>
                <div class="settings-row-control">
                    <span style="font-size:13px;color:var(--text-color);">{&current_settings.Encryption}</span>
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_auth_required}</div></div>
                <div class="settings-row-control">
                    <span style="font-size:13px;color:var(--text-color);">
                        {if current_settings.AuthRequired == 1 { &i18n_yes } else { &i18n_no }}
                    </span>
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_username}</div></div>
                <div class="settings-row-control">
                    <span style="font-size:13px;color:var(--text-color);">{&current_settings.Username}</span>
                </div>
            </div>

            <div class="settings-subsection-title">{&i18n_update_settings}</div>

            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_smtp_server}</div></div>
                <div class="settings-row-control">
                    <input
                        type="text"
                        placeholder="smtp.gmail.com"
                        value={form_server_name.deref().clone()}
                        oninput={on_server_name_change}
                        class="input"
                    />
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_port}</div></div>
                <div class="settings-row-control">
                    <input
                        type="number"
                        placeholder="587"
                        value={form_server_port.deref().clone()}
                        oninput={on_server_port_change}
                        class="input"
                        style="width:100px;"
                    />
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_from_email_address}</div></div>
                <div class="settings-row-control">
                    <input
                        type="email"
                        placeholder="noreply@yourdomain.com"
                        value={form_from_email.deref().clone()}
                        oninput={on_from_email_change}
                        class="input"
                    />
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_encryption_method}</div></div>
                <div class="settings-row-control">
                    <select
                        value={form_encryption.deref().clone()}
                        onchange={on_encryption_change}
                        class="select"
                    >
                        <option value="none">{&i18n_none}</option>
                        <option value="SSL/TLS">{"SSL/TLS"}</option>
                        <option value="StartTLS">{"StartTLS"}</option>
                    </select>
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_require_authentication}</div></div>
                <div class="settings-row-control">
                    <label class="toggle">
                        <input
                            type="checkbox"
                            id="auth_required"
                            checked={*form_auth_required}
                            onclick={on_auth_required_change}
                        />
                        <span class="toggle-track"><span class="toggle-thumb"></span></span>
                    </label>
                </div>
            </div>

            if *form_auth_required {
                <>
                    <div class="settings-row">
                        <div><div class="settings-row-label">{&i18n_auth_username}</div></div>
                        <div class="settings-row-control">
                            <input
                                type="text"
                                placeholder="your-email@domain.com"
                                value={form_username.deref().clone()}
                                oninput={on_username_change}
                                class="input"
                            />
                        </div>
                    </div>
                    <div class="settings-row">
                        <div><div class="settings-row-label">{&i18n_password}</div></div>
                        <div class="settings-row-control">
                            <input
                                type="password"
                                placeholder="Your password or app password"
                                value={form_password.deref().clone()}
                                oninput={on_password_change}
                                class="input"
                            />
                        </div>
                    </div>
                </>
            }

            <div class="settings-row">
                <div></div>
                <div class="settings-row-control">
                    <button
                        onclick={on_test_email}
                        disabled={*is_testing}
                        class="btn btn-secondary"
                        style="padding:6px 12px;"
                    >
                        if *is_testing {
                            <i class="ph ph-spinner animate-spin"></i>
                            {&i18n_testing}
                        } else {
                            <i class="ph ph-envelope"></i>
                            {&i18n_send_test_email}
                        }
                    </button>
                    if *test_success {
                        <button
                            onclick={on_save_settings}
                            disabled={*is_saving}
                            class="btn btn-primary"
                            style="padding:6px 12px;"
                        >
                            if *is_saving {
                                <i class="ph ph-spinner animate-spin"></i>
                                {&i18n_saving}
                            } else {
                                <i class="ph ph-floppy-disk"></i>
                                {&i18n_save_settings}
                            }
                        </button>
                    }
                </div>
            </div>

            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{"Common SMTP Settings"}</div>
                    <div class="settings-row-desc">{"Gmail: smtp.gmail.com:587 (StartTLS) or :465 (SSL/TLS)"}</div>
                    <div class="settings-row-desc">{"Outlook: smtp-mail.outlook.com:587 (StartTLS)"}</div>
                    <div class="settings-row-desc">{"Yahoo: smtp.mail.yahoo.com:587 (StartTLS)"}</div>
                    <div class="settings-row-desc">{"Note: Gmail requires an App Password instead of your regular password."}</div>
                </div>
            </div>
        </>
    }
}
