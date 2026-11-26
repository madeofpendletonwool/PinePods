use crate::components::context::{AppState, NotificationState, UIState, UserPreferencesState};
use crate::components::gen_components::{AdminSetupData, FirstAdminModal};
use crate::components::gen_funcs::format_error_message;
use crate::components::login_shell::{CardHead, LoginField, LoginShell, PlayButton};
use crate::components::gen_funcs::{encode_password, validate_user_input, ValidationError};
use crate::components::notification_center::ToastNotification;
use crate::components::setting_components::theme_options::initialize_default_theme;
use crate::requests::login_requests::{self, call_check_mfa_enabled, call_create_first_admin};
use crate::requests::login_requests::{call_add_login_user, AddUserRequest};
use crate::requests::login_requests::{
    call_first_login_done, call_get_public_oidc_providers, call_get_time_info,
    call_reset_password_create_code, call_self_service_login_status, call_setup_timezone_info,
    call_store_oidc_state, call_verify_and_reset_password, call_verify_key, call_verify_mfa,
    ResetCodePayload, ResetForgotPasswordPayload, TimeZoneInfo,
};
use crate::requests::setting_reqs::{
    call_get_available_languages, call_get_startpage, call_get_theme, call_update_user_language,
    AvailableLanguage,
};
use base64::Engine;
use chrono_tz::{Tz, TZ_VARIANTS};
use crate::components::i18n::use_translation;
use md5;
use web_sys::{console, window};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

// Gravatar URL generation functions (outside of use_effect_with)
#[allow(dead_code)]
fn calculate_gravatar_hash(email: &String) -> String {
    format!("{:x}", md5::compute(email.to_lowercase()))
}

#[allow(dead_code)]
pub fn generate_gravatar_url(email: &Option<String>, size: usize) -> String {
    let hash = calculate_gravatar_hash(&email.clone().unwrap_or_default());
    format!("https://gravatar.com/avatar/{}?s={}", hash, size)
}

#[function_component(Login)]
pub fn login() -> Html {
    let (i18n, _) = use_translation();

    // Pre-capture translation strings for use in closures and async blocks
    let i18n_error_checking_status = i18n.t("login.error_checking_status").to_string();
    let i18n_error_fetching_oidc = i18n.t("login.error_fetching_oidc_providers").to_string();
    let i18n_oidc_login_failed = i18n.t("login.oidc_login_failed").to_string();
    let _i18n_error_fetching_languages = i18n.t("login.error_fetching_languages").to_string();
    let i18n_account_created_success = i18n.t("login.account_created_successfully").to_string();
    let i18n_error_creating_account = i18n.t("login.error_creating_account").to_string();
    let i18n_error_checking_mfa = i18n.t("login.error_checking_mfa_status").to_string();
    let i18n_error_checking_first_login = i18n
        .t("login.error_checking_first_login_status")
        .to_string();
    let i18n_credentials_incorrect = i18n.t("login.credentials_incorrect").to_string();
    let i18n_error_setting_up_timezone = i18n.t("login.error_setting_up_timezone").to_string();
    let _i18n_first_time_welcome = i18n.t("login.first_time_welcome").to_string();
    let _i18n_mfa_welcome = i18n.t("login.mfa_welcome").to_string();
    let i18n_forgot_password = i18n.t("login.forgot_password").to_string();
    let i18n_reset_instructions = i18n.t("login.reset_password_instructions").to_string();
    let i18n_fill_password_fields = i18n.t("login.fill_password_fields").to_string();
    let i18n_passwords_no_match = i18n.t("login.passwords_no_match").to_string();
    let i18n_enter_reset_code = i18n.t("login.enter_reset_code").to_string();
    let i18n_password_reset_success = i18n.t("login.password_reset_success").to_string();
    let i18n_invalid_reset_code = i18n.t("login.invalid_reset_code").to_string();
    let i18n_password_reset_title = i18n.t("login.password_reset_title").to_string();
    let i18n_reset_code_sent = i18n.t("login.reset_code_sent").to_string();
    let _i18n_reset_code_label = i18n.t("login.reset_code_label").to_string();
    let _i18n_new_password_label = i18n.t("login.new_password_label").to_string();
    let _i18n_confirm_password_label = i18n.t("login.confirm_password_label").to_string();
    let i18n_initial_setup = i18n.t("login.initial_setup").to_string();
    let i18n_hour_format = i18n.t("login.hour_format").to_string();
    let i18n_time_zone = i18n.t("login.time_zone").to_string();
    let i18n_date_format = i18n.t("login.date_format").to_string();
    let i18n_save_preferences = i18n.t("login.save_preferences").to_string();
    let i18n_mfa_login = i18n.t("login.mfa_login").to_string();
    let i18n_connect_different_server = i18n.t("login.connect_different_server").to_string();
    let _i18n_connect_local_server = i18n.t("login.connect_local_server").to_string();
    let i18n_admin_created_success = i18n.t("login.admin_created_success").to_string();
    let _i18n_error_parsing_time = i18n.t("login.error_parsing_time").to_string();
    let i18n_language_updated = i18n.t("login.language_updated").to_string();
    let i18n_error_setup_timezone = i18n.t("login.error_setup_timezone").to_string();
    let _i18n_error_validating_mfa = i18n.t("login.error_validating_mfa").to_string();
    let i18n_language = i18n.t("common.language").to_string();
    let i18n_login = i18n.t("auth.login").to_string();
    let i18n_password_reset_successfully = i18n.t("login.password_reset_successfully").to_string();
    let i18n_sending_reset_code = i18n.t("login.sending_reset_code").to_string();
    let i18n_please_wait_processing = i18n.t("login.please_wait_processing").to_string();
    let i18n_date_format_julian = i18n.t("login.date_format_julian").to_string();
    let i18n_date_format_iso8601 = i18n.t("login.date_format_iso8601").to_string();
    let i18n_forgot_password_link = i18n.t("login.forgot_password_link").to_string();
    let _i18n_login_button = i18n.t("login.login_button").to_string();

    let history = BrowserHistory::new();
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());
    let new_username = use_state(|| "".to_string());
    let forgot_email = use_state(|| "".to_string());
    let forgot_username = use_state(|| "".to_string());
    let reset_password = use_state(|| "".to_string());
    let reset_password_confirm = use_state(|| "".to_string());
    let reset_code = use_state(|| "".to_string());
    let new_password = use_state(|| "".to_string());
    let email = use_state(|| "".to_string());
    let fullname = use_state(|| "".to_string());
    let (_app_state, dispatch) = use_store::<AppState>();
    let (_state, _dispatch) = use_store::<UIState>();
    let time_zone = use_state(|| "America/New_York".to_string());
    let date_format = use_state(|| "MDY".to_string());
    let time_pref = use_state(|| 12);
    let user_language = use_state(|| "en".to_string());
    let available_languages: UseStateHandle<Vec<AvailableLanguage>> = use_state(Vec::new);
    let mfa_code = use_state(|| "".to_string());
    let mfa_session_token = use_state(|| "".to_string());
    let temp_api_key = use_state(|| "".to_string());
    let temp_user_id = use_state(|| 0);
    let temp_server_name = use_state(|| "".to_string());
    let loading = use_state(|| true);
    // Define the initial state
    let page_state = use_state(|| PageState::Default);
    let submitting = use_state(|| false);
    let self_service_enabled = use_state(|| false); // State to store self-service status
    let effect_self_service = self_service_enabled.clone();
    let first_admin_created = use_state(|| true);
    let oidc_providers = use_state(|| Vec::new());
    let disable_standard_login = use_state(|| false);

    use_effect_with((), move |_| {
        initialize_default_theme();
        || ()
    });
    let first_admin_create_effect = first_admin_created.clone();
    use_effect_with((), move |_| {
        let self_service_enabled = effect_self_service.clone();
        let first_admin_created = first_admin_create_effect.clone();

        wasm_bindgen_futures::spawn_local(async move {
            let window = web_sys::window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location
                .origin()
                .expect("should have an origin")
                .trim_end_matches('/')
                .to_string();

            // The desktop app isn't served by a server; it signs in through ChangeServer.
            if crate::desktop::is_desktop() {
                return;
            }
            match call_self_service_login_status(server_name).await {
                Ok((status, admin_created)) => {
                    self_service_enabled.set(status);
                    first_admin_created.set(admin_created);
                }
                Err(e) => {
                    web_sys::console::log_1(
                        &format!("{}: {:?}", i18n_error_checking_status, e).into(),
                    );
                }
            }
        });

        || ()
    });

    let effect_providers = oidc_providers.clone();
    let effect_disable_standard_login = disable_standard_login.clone();

    use_effect_with((), move |_| {
        let providers = effect_providers.clone();
        let disable_std_login = effect_disable_standard_login.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let window = web_sys::window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location
                .origin()
                .expect("should have an origin")
                .trim_end_matches('/')
                .to_string();

            // The desktop app isn't served by a server; it signs in through ChangeServer.
            if crate::desktop::is_desktop() {
                return;
            }
            match call_get_public_oidc_providers(server_name).await {
                Ok(response) => {
                    disable_std_login.set(response.disable_standard_login);
                    providers.set(response.providers);
                }
                Err(e) => {
                    web_sys::console::log_1(
                        &format!("{}: {:?}", i18n_error_fetching_oidc, e).into(),
                    );
                }
            }
        });
        || ()
    });

    // Load available languages for first-time setup
    let effect_available_languages = available_languages.clone();
    use_effect_with((), move |_| {
        let languages = effect_available_languages.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let window = web_sys::window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location
                .origin()
                .expect("should have an origin")
                .trim_end_matches('/')
                .to_string();
            // The desktop app isn't served by a server; it signs in through ChangeServer.
            if crate::desktop::is_desktop() {
                return;
            }
            match call_get_available_languages(server_name).await {
                Ok(response) => {
                    languages.set(response);
                }
                Err(e) => {
                    web_sys::console::log_1(
                        &format!("Error fetching available languages: {:?}", e).into(),
                    );
                }
            }
        });
        || ()
    });

    let effect_displatch = dispatch.clone();
    let effect_loading = loading.clone();
    // User Auto Login with saved state
    use_effect_with((), {
        let history = history.clone();
        move |_| {
            effect_loading.set(true);
            if let Some(window) = web_sys::window() {
                if let Ok(local_storage) = window.local_storage() {
                    if let Some(storage) = local_storage {
                        if let Ok(Some(stored_theme)) = storage.get_item("selected_theme") {
                            // Set the theme using your existing theme change function
                            crate::components::setting_components::theme_options::changeTheme(
                                &stored_theme,
                            );
                        }
                        if let Ok(Some(user_state)) = storage.get_item("userState") {
                            let app_state_result = AppState::deserialize(&user_state);

                            if let Ok(Some(auth_state)) = storage.get_item("userAuthState") {
                                match AppState::deserialize(&auth_state) {
                                    Ok(auth_details) => {
                                        // Successful deserialization of auth state
                                        if let Ok(Some(server_state)) =
                                            storage.get_item("serverState")
                                        {
                                            let server_details_result =
                                                AppState::deserialize(&server_state);

                                            if let Ok(app_state) = app_state_result {
                                                // Successful deserialization of user state
                                                if let Ok(server_details) = server_details_result {
                                                    // Successful deserialization of server state
                                                    // Check if the deserialized state contains valid data
                                                    if app_state.user_details.is_some()
                                                        && auth_details.auth_details.is_some()
                                                        && server_details.server_details.is_some()
                                                    {
                                                        let auth_state_clone = auth_details.clone();
                                                        let email = &app_state
                                                            .user_details
                                                            .as_ref()
                                                            .unwrap()
                                                            .Email;
                                                        let user_id = app_state
                                                            .user_details
                                                            .as_ref()
                                                            .unwrap()
                                                            .UserID
                                                            .clone();
                                                        // Safely access server_name and api_key
                                                        let auth_details_clone =
                                                            auth_state_clone.auth_details.clone();
                                                        if let Some(auth_details) =
                                                            auth_details_clone.as_ref()
                                                        {
                                                            let server_name =
                                                                auth_details.server_name.clone();
                                                            let api_key = auth_details
                                                                .api_key
                                                                .clone()
                                                                .unwrap_or_default();

                                                            // Now verify the API key
                                                            // let wasm_user_id = user_id.clone();
                                                            let wasm_app_state = app_state.clone();
                                                            let wasm_auth_details: login_requests::LoginServerRequest = auth_details.clone();
                                                            let wasm_email = email.clone();
                                                            let wasm_user_id = user_id.clone();
                                                            wasm_bindgen_futures::spawn_local(
                                                                async move {
                                                                    match call_verify_key(
                                                                        &server_name.clone(),
                                                                        &api_key.clone(),
                                                                    )
                                                                    .await
                                                                    {
                                                                        Ok(_) => {
                                                                            // API key is valid, user can stay logged in
                                                                            let final_dispatch =
                                                                                effect_displatch
                                                                                    .clone();
                                                                            let gravatar_url = generate_gravatar_url(&wasm_email, 80);
                                                                            // Auto login logic here
                                                                            final_dispatch.reduce_mut(move |state| {
                                                                            state.user_details = wasm_app_state.user_details;
                                                                            state.auth_details = Some(wasm_auth_details.clone());
                                                                            state.server_details = server_details.server_details;
                                                                        });
                                                                            Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                                                                                state.gravatar_url = Some(gravatar_url);
                                                                            });
                                                                            // let mut error_message = app_state.error_message;
                                                                            // Retrieve the originally requested route, if any
                                                                            let session_storage = window.session_storage().unwrap().unwrap();
                                                                            session_storage.set_item("isAuthenticated", "true").unwrap();
                                                                            let requested_route = session_storage.get_item("requested_route").unwrap_or(None);
                                                                            // Get Theme
                                                                            let theme_api =
                                                                                api_key.clone();
                                                                            let theme_server =
                                                                                server_name.clone();
                                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                            match call_get_theme(theme_server, theme_api, &wasm_user_id).await{
                                                                                Ok(theme) => {
                                                                                    crate::components::setting_components::theme_options::changeTheme(&theme);
                                                                                    if let Some(window) = web_sys::window() {
                                                                                        if let Ok(Some(local_storage)) = window.local_storage() {
                                                                                            match local_storage.set_item("selected_theme", &theme) {
                                                                                                Ok(_) => console::log_1(&"Updated theme in local storage".into()),
                                                                                                Err(e) => console::log_1(&format!("Error updating theme in local storage: {:?}", e).into()),
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                }
                                                                                Err(_e) => {
                                                                                    // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                                                                }
                                                                            }
                                                                        });
                                                                            let time_server =
                                                                                server_name.clone();
                                                                            let time_api =
                                                                                api_key.clone();
                                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                            match call_get_time_info(time_server, time_api, &wasm_user_id).await{
                                                                                Ok(tz_response) => {
                                                                                    Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                                                                                        state.user_tz = Some(tz_response.timezone);
                                                                                        state.hour_preference = Some(tz_response.hour_pref);
                                                                                        state.date_format = Some(tz_response.date_format);
                                                                                    });
                                                                                }
                                                                                Err(_e) => {
                                                                                    // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                                                                }
                                                                            }
                                                                        });
                                                                            // let redirect_route = requested_route.unwrap_or_else(|| "/home".to_string());
                                                                            // effect_loading
                                                                            //     .set(false);
                                                                            // history.push(
                                                                            //     &redirect_route,
                                                                            // ); // Redirect to the requested or home page
                                                                            //
                                                                            // Add start page retrieval
                                                                            let startpage_api =
                                                                                api_key.clone();
                                                                            let startpage_server =
                                                                                server_name.clone();
                                                                            let startpage_user_id =
                                                                                wasm_user_id.clone();
                                                                            let startpage_history =
                                                                                history.clone();
                                                                            let startpage_loading =
                                                                                effect_loading
                                                                                    .clone();
                                                                            let startpage_requested_route =
                                                                                requested_route
                                                                                    .clone();

                                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                                // First try to use the requested route if it exists
                                                                                if let Some(route) = startpage_requested_route {
                                                                                    startpage_loading.set(false);
                                                                                    startpage_history.push(&route);
                                                                                    return; // Early return if we have a requested route
                                                                                }

                                                                                // Otherwise try to get the user's configured start page
                                                                                match call_get_startpage(&startpage_server, &startpage_api, &startpage_user_id).await {
                                                                                    Ok(start_page) => {
                                                                                        if !start_page.is_empty() {
                                                                                            // Use user's configured start page
                                                                                            startpage_loading.set(false);
                                                                                            startpage_history.push(&start_page);
                                                                                        } else {
                                                                                            // Empty start page, use default
                                                                                            startpage_loading.set(false);
                                                                                            startpage_history.push("/home");
                                                                                        }
                                                                                    }
                                                                                    Err(_) => {
                                                                                        // Failed to get start page, use default
                                                                                        startpage_loading.set(false);
                                                                                        startpage_history.push("/home");
                                                                                    }
                                                                                }
                                                                            });
                                                                        }
                                                                        Err(e) => {
                                                                            effect_loading
                                                                                .set(false);
                                                                            // Desktop app with the server unreachable:
                                                                            // play the downloads instead.
                                                                            if !(crate::desktop::is_unreachable(&e)
                                                                                && crate::desktop::offline_fallback().await)
                                                                            {
                                                                                history.push("/");
                                                                            }
                                                                        }
                                                                    }
                                                                },
                                                            );
                                                        } else {
                                                            // API key is not valid, redirect to login
                                                            effect_loading.set(false);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(
                                            &format!("Error deserializing auth state: {:?}", e)
                                                .into(),
                                        );
                                        effect_loading.set(false);
                                    }
                                }
                            }
                        } else {
                            effect_loading.set(false);
                        }
                    }
                }
            }

            || () // Return an empty closure to satisfy use_effect_with
        }
    });


    let on_login_username_change = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            username.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_login_password_change = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            password.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };
    let history_clone = history.clone();
    let submit_state = page_state.clone();
    let call_server_name = temp_server_name.clone();
    let call_api_key = temp_api_key.clone();
    let call_user_id = temp_user_id.clone();
    let submit_post_state = dispatch.clone();
    let on_submit = {
        let submit_dispatch = dispatch.clone();
        let mfa_session_token = mfa_session_token.clone();
        let i18n_error_checking_mfa = (&i18n_error_checking_mfa).clone();
        let i18n_error_checking_first_login = (&i18n_error_checking_first_login).clone();
        let i18n_credentials_incorrect = (&i18n_credentials_incorrect).clone();
        let submitting = submitting.clone();
        Callback::from(move |_| {
            let submitting = submitting.clone();
            submitting.set(true);
            let i18n_error_checking_mfa = (&i18n_error_checking_mfa).clone();
            let i18n_error_checking_first_login = (&i18n_error_checking_first_login).clone();
            let i18n_credentials_incorrect = (&i18n_credentials_incorrect).clone();
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            let dispatch = submit_dispatch.clone();
            let _post_state = submit_post_state.clone();
            let page_state = submit_state.clone();
            let temp_server_name = call_server_name.clone();
            let temp_api_key = call_api_key.clone();
            let temp_user_id = call_user_id.clone();
            let mfa_session_token = mfa_session_token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let window = window().expect("no global `window` exists");
                let location = window.location();
                let server_name = location.origin().expect("should have an origin");
                let server_name = server_name.trim_end_matches('/').to_string();
                let page_state = page_state.clone();
                match login_requests::login_new_server_secure(
                    server_name.clone(),
                    username.to_string(),
                    password.to_string(),
                )
                .await
                {
                    Ok(login_requests::LoginResult::Success(
                        user_details,
                        login_request,
                        server_details,
                    )) => {
                        // After user login, update the image URL with user's email from user_details
                        let gravatar_url = generate_gravatar_url(&user_details.Email, 80); // 80 is the image size
                        let key_copy = login_request.clone();
                        let user_copy = user_details.clone();
                        dispatch.reduce_mut(move |state| {
                            state.user_details = Some(user_details);
                            state.auth_details = Some(login_request);
                            state.server_details = Some(server_details);
                            state.store_app_state();
                        });
                        Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                            state.gravatar_url = Some(gravatar_url);
                        });

                        // Extract server_name, api_key, and user_id
                        let server_name = key_copy.server_name;
                        let api_key = key_copy.api_key;
                        let user_id = user_copy.UserID;

                        temp_server_name.set(server_name.clone());
                        temp_api_key.set(api_key.clone().unwrap());
                        temp_user_id.set(user_id.clone());

                        match call_first_login_done(
                            server_name.clone(),
                            api_key.clone().unwrap(),
                            &user_id,
                        )
                        .await
                        {
                            Ok(first_login_done) => {
                                if first_login_done {
                                    match call_check_mfa_enabled(
                                        server_name.clone(),
                                        api_key.clone().unwrap(),
                                        &user_id,
                                    )
                                    .await
                                    {
                                        Ok(response) => {
                                            if response.mfa_enabled {
                                                page_state.set(PageState::MFAPrompt);
                                            } else {
                                                let theme_api = api_key.clone();
                                                let theme_server = server_name.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    match call_get_theme(
                                                        theme_server,
                                                        theme_api.unwrap(),
                                                        &user_id,
                                                    )
                                                    .await
                                                    {
                                                        Ok(theme) => {
                                                            crate::components::setting_components::theme_options::changeTheme(&theme);
                                                            // Update the local storage with the new theme
                                                            if let Some(window) = web_sys::window()
                                                            {
                                                                if let Ok(Some(local_storage)) =
                                                                    window.local_storage()
                                                                {
                                                                    match local_storage.set_item("selected_theme", &theme) {
                                                                        Ok(_) => console::log_1(&"Updated theme in local storage".into()),
                                                                        Err(e) => console::log_1(&format!("Error updating theme in local storage: {:?}", e).into()),
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Err(_e) => {
                                                            // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                                        }
                                                    }
                                                });
                                                let time_server = server_name.clone();
                                                let time_api = api_key.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    match call_get_time_info(
                                                        time_server,
                                                        time_api.unwrap(),
                                                        &user_id,
                                                    )
                                                    .await
                                                    {
                                                        Ok(tz_response) => {
                                                            Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                                                                state.user_tz =
                                                                    Some(tz_response.timezone);
                                                                state.hour_preference =
                                                                    Some(tz_response.hour_pref);
                                                                state.date_format =
                                                                    Some(tz_response.date_format);
                                                            });
                                                        }
                                                        Err(_e) => {
                                                            // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                                        }
                                                    }
                                                });
                                                // Add start page retrieval before redirecting
                                                let startpage_api = api_key.clone().unwrap();
                                                let startpage_server = server_name.clone();
                                                let startpage_user_id = user_id.clone();
                                                let startpage_history = history.clone();

                                                wasm_bindgen_futures::spawn_local(async move {
                                                    // Try to get the user's configured start page
                                                    match call_get_startpage(
                                                        &startpage_server,
                                                        &startpage_api,
                                                        &startpage_user_id,
                                                    )
                                                    .await
                                                    {
                                                        Ok(start_page) => {
                                                            if !start_page.is_empty() {
                                                                // Use user's configured start page
                                                                startpage_history.push(&start_page);
                                                            } else {
                                                                // Empty start page, use default
                                                                startpage_history.push("/home");
                                                            }
                                                        }
                                                        Err(_) => {
                                                            // Failed to get start page, use default
                                                            startpage_history.push("/home");
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        Err(_) => {
                                            submitting.set(false);
                                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                state.error_message =
                                                    Option::from((&i18n_error_checking_mfa).clone())
                                            });
                                        }
                                    }
                                } else {
                                    page_state.set(PageState::TimeZone);
                                }
                            }
                            Err(_) => {
                                submitting.set(false);
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message =
                                        Option::from((&i18n_error_checking_first_login).clone())
                                });
                            }
                        }
                    }
                    Ok(login_requests::LoginResult::MfaRequired {
                        server_name: mfa_server,
                        username: _mfa_username,
                        user_id: mfa_user_id,
                        mfa_session_token: session_token,
                    }) => {
                        // Store MFA session data for the MFA prompt
                        temp_server_name.set(mfa_server.clone());
                        temp_user_id.set(mfa_user_id);
                        mfa_session_token.set(session_token);

                        console::log_1(&"MFA required - transitioning to MFA prompt".into());

                        // Set page state to show MFA prompt
                        page_state.set(PageState::MFAPrompt);
                    }
                    Err(_) => {
                        submitting.set(false);
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message =
                                Option::from((&i18n_credentials_incorrect).clone())
                        });
                        // Handle error
                    }
                }
            });
        })
    };

    let on_form_submit = {
        let on_submit = on_submit.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            on_submit.emit(());
        })
    };

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Default,
        CreateUser,
        ForgotPassword,
        ForgotPasswordLoading,
        TimeZone,
        MFAPrompt,
        EnterCode,
    }
    // Define the callback functions
    let create_new_state = page_state.clone();
    let on_create_new_user = {
        let page_state = create_new_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::CreateUser);
        })
    };

    // Define the callback function for closing the modal
    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Default);
        })
    };



    let on_fullname_change = {
        let fullname = fullname.clone();
        Callback::from(move |e: InputEvent| {
            fullname.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_username_change = {
        let new_username = new_username.clone();
        Callback::from(move |e: InputEvent| {
            new_username.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_email_change = {
        let email = email.clone();
        Callback::from(move |e: InputEvent| {
            email.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_password_change = {
        let new_password = new_password.clone();
        Callback::from(move |e: InputEvent| {
            new_password.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };
    #[allow(non_camel_case_types)]
    enum email_error_notice {
        Hidden,
        Shown,
    }
    #[allow(non_camel_case_types)]
    enum password_error_notice {
        Hidden,
        Shown,
    }
    #[allow(non_camel_case_types)]
    enum username_error_notice {
        Hidden,
        Shown,
    }

    //Define States for error message
    let email_error = use_state(|| email_error_notice::Hidden);
    let password_error = use_state(|| password_error_notice::Hidden);
    let username_error = use_state(|| username_error_notice::Hidden);

    let create_state = dispatch.clone();
    let on_create_submit = {
        let page_state = page_state.clone();
        let fullname = fullname.clone().to_string();
        let new_username = new_username.clone().to_string();
        let email = email.clone().to_string();
        let new_password = new_password.clone();
        let username_error = username_error.clone();
        let password_error = password_error.clone();
        let email_error = email_error.clone();
        let i18n_account_created_success = (&i18n_account_created_success).clone();
        let i18n_error_creating_account = (&i18n_error_creating_account).clone();
        Callback::from(move |e: MouseEvent| {
            let i18n_account_created_success = (&i18n_account_created_success).clone();
            let i18n_error_creating_account = (&i18n_error_creating_account).clone();
            let _create_state = create_state.clone();
            let window = window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location.origin().expect("should have an origin");
            let server_name = server_name.trim_end_matches('/').to_string();
            let new_username = new_username.clone();
            let new_password = new_password.clone();
            let fullname = fullname.clone();
            let email = email.clone();
            let page_state = page_state.clone();

            e.prevent_default();

            // Validate input fields
            let errors = validate_user_input(&new_username, &new_password, &email);

            if errors.contains(&ValidationError::UsernameTooShort) {
                username_error.set(username_error_notice::Shown);
            } else {
                username_error.set(username_error_notice::Hidden);
            }

            if errors.contains(&ValidationError::PasswordTooShort) {
                password_error.set(password_error_notice::Shown);
            } else {
                password_error.set(password_error_notice::Hidden);
            }

            if errors.contains(&ValidationError::InvalidEmail) {
                email_error.set(email_error_notice::Shown);
            } else {
                email_error.set(email_error_notice::Hidden);
            }

            if errors.is_empty() {
                match encode_password(&new_password) {
                    Ok(hash_pw) => {
                        let user_settings = AddUserRequest {
                            fullname: fullname.clone(),
                            username: new_username.clone(),
                            email: email.clone(),
                            hash_pw: hash_pw.clone(),
                        };
                        let add_user_request = Some(user_settings);

                        wasm_bindgen_futures::spawn_local(async move {
                            match call_add_login_user(server_name, &add_user_request).await {
                                Ok(success) => {
                                    if success {
                                        page_state.set(PageState::Default);
                                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                            state.info_message =
                                                Some((&i18n_account_created_success).clone())
                                        });
                                    }
                                }
                                Err(e) => {
                                    // The error message is now user-friendly from our updated call_add_login_user
                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                        state.error_message = Some(e.to_string())
                                    });
                                }
                            }
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!(
                                "{}: {}",
                                i18n_error_creating_account, formatted_error
                            ))
                        });
                    }
                }
            }
        })
    };
    // Define the modal components
    let create_user_modal = html! {
        <div class="lp-panel" key="create">
            <CardHead
                title={i18n.t("login.create_account_title")}
                subtitle={Some(AttrValue::from(i18n.t("login.create_account_subtitle")))}
                on_back={Some(on_close_modal.clone())}
            />
            <form action="#">
                <LoginField
                    id="fullname"
                    label={i18n.t("common.full_name")}
                    icon="ph-identification-card"
                    autocomplete={Some(AttrValue::Static("name"))}
                    autofocus=true
                    oninput={on_fullname_change}
                />
                <LoginField
                    id="new_username"
                    label={i18n.t("common.username")}
                    icon="ph-user"
                    autocomplete={Some(AttrValue::Static("username"))}
                    oninput={on_username_change.clone()}
                    error={match *username_error {
                        username_error_notice::Shown => Some(AttrValue::from(i18n.t("validation.username_min_length"))),
                        username_error_notice::Hidden => None,
                    }}
                />
                <LoginField
                    id="new_email"
                    kind="email"
                    label={i18n.t("common.email")}
                    icon="ph-envelope-simple"
                    autocomplete={Some(AttrValue::Static("email"))}
                    oninput={on_email_change}
                    error={match *email_error {
                        email_error_notice::Shown => Some(AttrValue::from(i18n.t("validation.invalid_email"))),
                        email_error_notice::Hidden => None,
                    }}
                />
                <LoginField
                    id="new_password"
                    kind="password"
                    label={i18n.t("common.password")}
                    icon="ph-lock-key"
                    autocomplete={Some(AttrValue::Static("new-password"))}
                    oninput={on_password_change.clone()}
                    error={match *password_error {
                        password_error_notice::Shown => Some(AttrValue::from(i18n.t("validation.password_min_length"))),
                        password_error_notice::Hidden => None,
                    }}
                />
                <PlayButton label={i18n.t("login.create_account")} icon="ph-user-plus" onclick={Some(on_create_submit)} />
            </form>
        </div>
    };

    let on_forgot_password = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::ForgotPassword);
        })
    };

    let on_forgot_username_change = {
        let forgot_username = forgot_username.clone();
        Callback::from(move |e: InputEvent| {
            forgot_username.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_forgot_email_change = {
        let forgot_email = forgot_email.clone();
        Callback::from(move |e: InputEvent| {
            forgot_email.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_reset_submit = {
        let page_state = page_state.clone();
        let forgot_username = forgot_username.clone().to_string();
        let forgot_email = forgot_email.clone().to_string();
        Callback::from(move |e: yew::events::MouseEvent| {
            e.prevent_default();
            let window = window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location.origin().expect("should have an origin");
            let server_name = server_name.trim_end_matches('/').to_string();
            let page_state = page_state.clone();

            // Show loading state immediately
            page_state.set(PageState::ForgotPasswordLoading);

            let reset_code_request = Some(ResetCodePayload {
                username: forgot_username.clone(),
                email: forgot_email.clone(),
            });

            wasm_bindgen_futures::spawn_local(async move {
                match call_reset_password_create_code(server_name, &reset_code_request.unwrap())
                    .await
                {
                    Ok(_success) => {
                        // Always show success - don't reveal if user exists or not
                        page_state.set(PageState::EnterCode);
                    }
                    Err(_e) => {
                        // Still show success for security - prevents user enumeration
                        page_state.set(PageState::EnterCode);
                    }
                }
            });
        })
    };

    let forgot_password_modal = html! {
        <div class="lp-panel" key="forgot">
            <CardHead
                title={i18n_forgot_password.clone()}
                subtitle={Some(AttrValue::from(i18n_reset_instructions.clone()))}
                on_back={Some(on_close_modal.clone())}
            />
            <form action="#">
                <LoginField
                    id="forgot_username"
                    label={i18n.t("common.username")}
                    icon="ph-user"
                    autocomplete={Some(AttrValue::Static("username"))}
                    autofocus=true
                    oninput={on_forgot_username_change}
                />
                <LoginField
                    id="forgot_email"
                    kind="email"
                    label={i18n.t("common.email")}
                    icon="ph-envelope-simple"
                    autocomplete={Some(AttrValue::Static("email"))}
                    oninput={on_forgot_email_change}
                />
                <PlayButton label={i18n.t("login.send_reset_code")} icon="ph-envelope-simple" onclick={Some(on_reset_submit)} />
            </form>
        </div>
    };

    let on_reset_code_change = {
        let reset_code = reset_code.clone();
        Callback::from(move |e: InputEvent| {
            reset_code.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_reset_password_change = {
        let reset_password = reset_password.clone();
        Callback::from(move |e: InputEvent| {
            reset_password.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_reset_password_confirm_change = {
        let reset_password_confirm = reset_password_confirm.clone();
        Callback::from(move |e: InputEvent| {
            reset_password_confirm.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_reset_code_submit = {
        let page_state = page_state.clone();
        let reset_password = reset_password.clone().to_string();
        let reset_password_confirm = reset_password_confirm.clone().to_string();
        let forgot_email = forgot_email.clone().to_string();
        let reset_code = reset_code.clone().to_string();
        let dispatch_wasm = dispatch.clone();
        let i18n_fill_password_fields = i18n_fill_password_fields.clone();
        let i18n_passwords_no_match = i18n_passwords_no_match.clone();
        let i18n_enter_reset_code = i18n_enter_reset_code.clone();
        let i18n_password_reset_success = i18n_password_reset_success.clone();
        let i18n_invalid_reset_code = i18n_invalid_reset_code.clone();
        let i18n_password_reset_successfully = i18n_password_reset_successfully.clone();
        Callback::from(move |_e: yew::events::MouseEvent| {
            let i18n_fill_password_fields = i18n_fill_password_fields.clone();
            let i18n_passwords_no_match = i18n_passwords_no_match.clone();
            let i18n_enter_reset_code = i18n_enter_reset_code.clone();
            let i18n_password_reset_success = i18n_password_reset_success.clone();
            let i18n_invalid_reset_code = i18n_invalid_reset_code.clone();
            let i18n_password_reset_successfully = i18n_password_reset_successfully.clone();
            let _dispatch = dispatch_wasm.clone();
            let page_state = page_state.clone();

            // Validate password confirmation
            if reset_password.is_empty() || reset_password_confirm.is_empty() {
                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                    state.error_message = Option::from(i18n_fill_password_fields.clone());
                });
                return;
            }

            if reset_password != reset_password_confirm {
                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                    state.error_message = Option::from(i18n_passwords_no_match.clone());
                });
                return;
            }

            if reset_code.is_empty() {
                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                    state.error_message = Option::from(i18n_enter_reset_code.clone());
                });
                return;
            }

            let window = window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location.origin().expect("should have an origin");
            let server_name = server_name.trim_end_matches('/').to_string();

            match encode_password(&reset_password) {
                Ok(hash_pw) => {
                    let reset_password_request = Some(ResetForgotPasswordPayload {
                        reset_code: reset_code.clone(),
                        email: forgot_email.clone(),
                        new_password: hash_pw.clone(),
                    });
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_verify_and_reset_password(
                            server_name,
                            &reset_password_request.unwrap(),
                        )
                        .await
                        {
                            Ok(success) => {
                                if success.message == i18n_password_reset_successfully {
                                    page_state.set(PageState::Default);
                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                        state.info_message =
                                            Option::from(i18n_password_reset_success.clone());
                                    });
                                } else {
                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                        state.error_message =
                                            Option::from(i18n_invalid_reset_code.clone());
                                    });
                                }
                            }
                            Err(_e) => {
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message = Option::from("Invalid reset code or code has expired. Please request a new reset code.".to_string());
                                });
                            }
                        }
                    });
                }
                Err(e) => {
                    let formatted_error = format_error_message(&e.to_string());
                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                        state.error_message = Option::from(format!(
                            "Unable to hash new password: {:?}",
                            formatted_error
                        ))
                    });
                    page_state.set(PageState::Default);
                }
            }
        })
    };

    let enter_code_modal = html! {
        <div class="lp-panel" key="enter-code">
            <CardHead
                title={i18n_password_reset_title.clone()}
                subtitle={Some(AttrValue::from(i18n_reset_code_sent.clone()))}
                on_back={Some(on_close_modal.clone())}
            />
            <form action="#">
                <LoginField
                    id="reset_code"
                    label={i18n.t("auth.enter_password_reset_code")}
                    icon="ph-key"
                    autocomplete={Some(AttrValue::Static("one-time-code"))}
                    autofocus=true
                    oninput={on_reset_code_change}
                />
                <LoginField
                    id="reset_password"
                    kind="password"
                    label={i18n.t("auth.enter_new_password")}
                    icon="ph-lock-key"
                    autocomplete={Some(AttrValue::Static("new-password"))}
                    oninput={on_reset_password_change}
                />
                <LoginField
                    id="reset_password_confirm"
                    kind="password"
                    label={i18n.t("auth.confirm_new_password")}
                    icon="ph-lock-key"
                    autocomplete={Some(AttrValue::Static("new-password"))}
                    oninput={on_reset_password_confirm_change}
                />
                <PlayButton label={i18n.t("login.reset_password_button")} icon="ph-key" onclick={Some(on_reset_code_submit)} />
            </form>
        </div>
    };

    let forgot_password_loading_modal = html! {
        <div class="lp-panel" key="sending">
            <div class="lp-busy">
                <span class="lp-eq" aria-hidden="true"><i></i><i></i><i></i><i></i></span>
                <h3>{ &i18n_sending_reset_code }</h3>
                <p>{ &i18n_please_wait_processing }</p>
            </div>
        </div>
    };

    let history_clone = history.clone();
    let on_different_server = {
        Callback::from(move |_| {
            let history = history_clone.clone();
            history.push("/change_server"); // Use the route path
        })
    };

    let on_tz_change = {
        let tz = time_zone.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            tz.set(select_element.value());
        })
    };
    let on_df_change = {
        let df = date_format.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            df.set(select_element.value());
        })
    };

    let on_time_pref_change = {
        let time_pref = time_pref.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            let value_str = select_element.value();
            if let Ok(value_int) = value_str.parse::<i32>() {
                time_pref.set(value_int);
            } else {
                // console::log_1(&"Error parsing time preference".into());
            }
        })
    };

    let on_language_change = {
        let user_language = user_language.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            user_language.set(select_element.value());
        })
    };

    let on_time_zone_submit = {
        // let (state, dispatch) = use_store::<AppState>();
        let page_state = page_state.clone();
        let time_pref = time_pref.clone();
        let time_zone = time_zone.clone();
        let date_format = date_format.clone();
        let user_language = user_language.clone();
        // let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        // let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        // let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let temp_server_name = temp_server_name.clone();
        let temp_api_key = temp_api_key.clone();
        let temp_user_id = temp_user_id.clone();
        let history = history.clone();
        // let error_message_create = error_message.clone();
        let dispatch_wasm = dispatch.clone();
        let i18n_error_checking_mfa = (&i18n_error_checking_mfa).clone();
        let i18n_error_setting_up_timezone = (&i18n_error_setting_up_timezone).clone();
        let i18n_language_updated = i18n_language_updated.clone();
        let i18n_error_setup_timezone = i18n_error_setup_timezone.clone();
        Callback::from(move |e: MouseEvent| {
            let i18n_error_checking_mfa = (&i18n_error_checking_mfa).clone();
            let i18n_error_setting_up_timezone = (&i18n_error_setting_up_timezone).clone();
            let i18n_language_updated = i18n_language_updated.clone();
            let i18n_error_setup_timezone = i18n_error_setup_timezone.clone();
            let _dispatch = dispatch_wasm.clone();
            e.prevent_default();
            let server_name = (*temp_server_name).clone();
            let api_key = (*temp_api_key).clone();
            let user_id = *temp_user_id;
            let page_state = page_state.clone();
            let history = history.clone();
            let user_language = (*user_language).clone();
            // let error_message_clone = error_message_create.clone();
            e.prevent_default();
            // page_state.set(PageState::Default);

            let timezone_info = TimeZoneInfo {
                user_id: *temp_user_id, // assuming temp_user_id is a use_state of i32
                timezone: (*time_zone).clone(),
                hour_pref: *time_pref,
                date_format: (*date_format).clone(),
            };

            wasm_bindgen_futures::spawn_local(async move {
                // Directly use timezone_info without checking it against time_zone_setup
                match call_setup_timezone_info(server_name.clone(), api_key.clone(), timezone_info)
                    .await
                {
                    Ok(success) => {
                        if success.success {
                            // Update language preference if not English
                            let user_lang = (user_language).to_string();
                            if !user_lang.is_empty() && user_lang != "en" {
                                match call_update_user_language(
                                    server_name.clone(),
                                    api_key.clone(),
                                    user_id,
                                    user_lang,
                                )
                                .await
                                {
                                    Ok(_) => {
                                        web_sys::console::log_1(
                                            &i18n_language_updated.clone().into(),
                                        );
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(
                                            &format!("Failed to update language preference: {}", e)
                                                .into(),
                                        );
                                        // Don't fail the entire setup if language update fails
                                    }
                                }
                            }
                            page_state.set(PageState::Default);
                            match call_check_mfa_enabled(
                                server_name.clone(),
                                api_key.clone(),
                                &user_id,
                            )
                            .await
                            {
                                Ok(response) => {
                                    if response.mfa_enabled {
                                        page_state.set(PageState::MFAPrompt);
                                    } else {
                                        // Add start page retrieval before redirecting
                                        let startpage_api = api_key.clone();
                                        let startpage_server = server_name.clone();
                                        let startpage_user_id = user_id.clone();
                                        let startpage_history = history.clone();

                                        wasm_bindgen_futures::spawn_local(async move {
                                            // Try to get the user's configured start page
                                            match call_get_startpage(
                                                &startpage_server,
                                                &startpage_api,
                                                &startpage_user_id,
                                            )
                                            .await
                                            {
                                                Ok(start_page) => {
                                                    if !start_page.is_empty() {
                                                        // Use user's configured start page
                                                        startpage_history.push(&start_page);
                                                    } else {
                                                        // Empty start page, use default
                                                        startpage_history.push("/home");
                                                    }
                                                }
                                                Err(_) => {
                                                    // Failed to get start page, use default
                                                    startpage_history.push("/home");
                                                }
                                            }
                                        });
                                    }
                                }
                                Err(_) => {
                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                        state.error_message =
                                            Option::from((&i18n_error_checking_mfa).clone())
                                    });
                                }
                            }
                        } else {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Option::from((&i18n_error_setting_up_timezone).clone())
                            });
                            page_state.set(PageState::Default);
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Option::from(i18n_error_setup_timezone.clone())
                            });
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!(
                                "Error setting up time zone: {:?}",
                                formatted_error
                            ))
                        });
                    }
                }
            });
        })
    };

    fn render_time_zone_option(tz: Tz) -> Html {
        html! {
            <option value={tz.name()}>{tz.name()}</option>
        }
    }

    let time_zone_setup_modal = html! {
            <div class="modal-overlay">
                <div class="item_container-text modal-content">
                    // Header
                    <div class="modal-header">
                        <i class="ph ph-clock text-xl"></i>
                        <h3 class="text-lg">{&i18n_initial_setup}</h3>
                    </div>

                    // Content
                    <div class="modal-body">
                        <form>
                            <div class="modal-welcome">
                                <i class="ph ph-hand-waving text-xl"></i>
                                <p>
    {&i18n.t("login.first_time_welcome")}
                                </p>
                            </div>

                            <div class="modal-form-group">
                                <label class="modal-label">
                                    <i class="ph ph-translate"></i>
                                    <span>{&i18n_language}</span>
                                </label>
                                <select
                                    id="language"
                                    name="language"
                                    class="modal-select"
                                    oninput={on_language_change.clone()}
                                    value={(*user_language).clone()}
                                >
                                    { for available_languages.iter().map(|lang| {
                                        let selected = lang.code == *user_language;
                                        html! {
                                            <option value={lang.code.clone()} selected={selected}>{&lang.name}</option>
                                        }
                                    })}
                                </select>
                            </div>

                            <div class="modal-form-group">
                                <label class="modal-label">
                                    <i class="ph ph-clock-clockwise"></i>
                                    <span>{&i18n_hour_format}</span>
                                </label>
                                <select
                                    id="hour_format"
                                    name="hour_format"
                                    class="modal-select"
                                    oninput={on_time_pref_change}
                                    value={(*time_pref).to_string()}
                                >
                                    <option value="12" selected={*time_pref == 12}>{"12 Hour"}</option>
                                    <option value="24" selected={*time_pref == 24}>{"24 Hour"}</option>
                                </select>
                            </div>

                            <div class="modal-form-group">
                                <label class="modal-label">
                                    <i class="ph ph-globe"></i>
                                    <span>{&i18n_time_zone}</span>
                                </label>
                                <select
                                    id="time_zone"
                                    name="time_zone"
                                    class="modal-select"
                                    oninput={on_tz_change}
                                    value={(*time_zone).clone()}
                                >
                                    { for TZ_VARIANTS.iter().map(|tz| render_time_zone_option(*tz)) }
                                </select>
                            </div>

                            <div class="modal-form-group">
                                <label class="modal-label">
                                    <i class="ph ph-calendar"></i>
                                    <span>{&i18n_date_format}</span>
                                </label>
                                <select
                                    id="date_format"
                                    name="date_format"
                                    class="modal-select"
                                    oninput={on_df_change}
                                    value={(*date_format).clone()}
                                >
                                    <option value="MDY" selected={*date_format == "MDY"}>{"MM-DD-YYYY"}</option>
                                    <option value="DMY" selected={*date_format == "DMY"}>{"DD-MM-YYYY"}</option>
                                    <option value="YMD" selected={*date_format == "YMD"}>{"YYYY-MM-DD"}</option>
                                    <option value="JUL" selected={*date_format == "JUL"}>{ &i18n_date_format_julian }</option>
                                    <option value="ISO" selected={*date_format == "ISO"}>{ &i18n_date_format_iso8601 }</option>
                                    <option value="USA" selected={*date_format == "USA"}>{"MM/DD/YYYY"}</option>
                                    <option value="EUR" selected={*date_format == "EUR"}>{"DD.MM.YYYY"}</option>
                                    <option value="JIS" selected={*date_format == "JIS"}>{"YYYY-MM-DD"}</option>
                                </select>
                            </div>

                            <button
                                type="submit"
                                onclick={on_time_zone_submit}
                                class="modal-button"
                            >
                                <i class="ph ph-check"></i>
                                <span>{&i18n_save_preferences}</span>
                            </button>
                        </form>
                    </div>
                </div>
            </div>
        };

    let on_mfa_change = {
        let mfa_code = mfa_code.clone();
        Callback::from(move |e: InputEvent| {
            mfa_code.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_mfa_submit = {
        let dispatch = dispatch.clone();
        let page_state = page_state.clone();
        let mfa_code = mfa_code.clone();
        let mfa_session_token = mfa_session_token.clone();
        let temp_server_name = temp_server_name.clone();
        let history = history.clone();

        Callback::from(move |e: MouseEvent| {
            let dispatch = dispatch.clone();
            let mfa_code = mfa_code.clone();
            let mfa_session_token = mfa_session_token.clone();
            let temp_server_name = temp_server_name.clone();
            let page_state = page_state.clone();
            let history = history.clone();

            e.prevent_default();

            wasm_bindgen_futures::spawn_local(async move {
                // Use the new secure MFA verification endpoint
                match login_requests::complete_mfa_login(
                    (*temp_server_name).clone(),
                    "".to_string(), // Username not needed for completion
                    (*mfa_session_token).clone(),
                    (*mfa_code).clone(),
                )
                .await
                {
                    Ok((user_details, login_request, server_details)) => {
                        // MFA verified successfully - complete the login process
                        let gravatar_url = generate_gravatar_url(&user_details.Email, 80);

                        // Extract values for theme/time/startpage operations before moving into closure
                        let server_name = login_request.server_name.clone();
                        let api_key = login_request.api_key.clone().unwrap();
                        let user_id = user_details.UserID;

                        dispatch.reduce_mut(move |state| {
                            state.user_details = Some(user_details);
                            state.auth_details = Some(login_request);
                            state.server_details = Some(server_details);
                            state.store_app_state();
                        });
                        Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                            state.gravatar_url = Some(gravatar_url);
                        });

                        page_state.set(PageState::Default);

                        // Set session authentication
                        let window = web_sys::window().expect("no global `window` exists");
                        let session_storage = window.session_storage().unwrap().unwrap();
                        session_storage.set_item("isAuthenticated", "true").unwrap();

                        // Get theme
                        let theme_api = api_key.clone();
                        let theme_server = server_name.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match call_get_theme(theme_server, theme_api, &user_id).await {
                                Ok(theme) => {
                                    crate::components::setting_components::theme_options::changeTheme(&theme);
                                    // Update local storage with the new theme
                                    if let Some(window) = web_sys::window() {
                                        if let Ok(Some(local_storage)) = window.local_storage() {
                                            match local_storage.set_item("selected_theme", &theme) {
                                                Ok(_) => console::log_1(&"Updated theme in local storage".into()),
                                                Err(e) => console::log_1(&format!("Error updating theme in local storage: {:?}", e).into()),
                                            }
                                        }
                                    }
                                }
                                Err(_e) => {
                                    // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                }
                            }
                        });

                        // Get time info
                        let time_server = server_name.clone();
                        let time_api = api_key.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match call_get_time_info(time_server, time_api, &user_id).await {
                                Ok(tz_response) => {
                                    Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                                        state.user_tz = Some(tz_response.timezone);
                                        state.hour_preference = Some(tz_response.hour_pref);
                                        state.date_format = Some(tz_response.date_format);
                                    });
                                }
                                Err(e) => {
                                    console::log_1(
                                        &format!("Error getting time info: {:?}", e).into(),
                                    );
                                }
                            }
                        });

                        // Get start page and redirect
                        let startpage_api = api_key.clone();
                        let startpage_server = server_name.clone();
                        let startpage_user_id = user_id.clone();
                        let startpage_history = history.clone();

                        wasm_bindgen_futures::spawn_local(async move {
                            // First check for requested route
                            let window = web_sys::window().expect("no global `window` exists");
                            let session_storage = window.session_storage().unwrap().unwrap();
                            let requested_route =
                                session_storage.get_item("requested_route").unwrap_or(None);

                            if let Some(route) = requested_route {
                                startpage_history.push(&route);
                                return;
                            }

                            // Try to get the user's configured start page
                            match call_get_startpage(
                                &startpage_server,
                                &startpage_api,
                                &startpage_user_id,
                            )
                            .await
                            {
                                Ok(start_page) => {
                                    if !start_page.is_empty() {
                                        // Use user's configured start page
                                        startpage_history.push(&start_page);
                                    } else {
                                        // Empty start page, use default
                                        startpage_history.push("/home");
                                    }
                                }
                                Err(_) => {
                                    // Failed to get start page, use default
                                    startpage_history.push("/home");
                                }
                            }
                        });
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!(
                                "MFA verification failed: {}",
                                formatted_error
                            ))
                        });
                    }
                }
            });
        })
    };
    let first_admin_create_clone = first_admin_created.clone();
    let on_admin_setup = {
        let history = history.clone();
        let first_admin_created = first_admin_create_clone.clone();
        let dispatch_wasm = dispatch.clone();
        let i18n_admin_created_success = i18n_admin_created_success.clone();

        Callback::from(move |data: AdminSetupData| {
            let i18n_admin_created_success = i18n_admin_created_success.clone();
            let history = history.clone();
            let first_admin_created = first_admin_created.clone();
            let _audio_dispatch = dispatch_wasm.clone();

            // Get server name from window location
            let window = web_sys::window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location
                .origin()
                .expect("should have an origin")
                .trim_end_matches('/')
                .to_string();

            let request = AdminSetupData {
                username: data.username,
                password: data.password,
                email: data.email,
                fullname: data.fullname,
            };

            wasm_bindgen_futures::spawn_local(async move {
                match call_create_first_admin(&server_name, request).await {
                    Ok(_) => {
                        first_admin_created.set(true);
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(i18n_admin_created_success.clone());
                        });
                        history.push("/"); // Redirect to login
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Failed to create admin: {}", formatted_error));
                        });
                    }
                }
            });
        })
    };

    let mfa_code_modal = html! {
        <div class="lp-panel" key="mfa">
            <CardHead
                title={i18n_mfa_login.clone()}
                subtitle={Some(AttrValue::from(i18n.t("login.mfa_subtitle")))}
                on_back={Some(on_close_modal.clone())}
            />
            <form action="#">
                <LoginField
                    id="mfa_code"
                    label={i18n.t("auth.enter_mfa_code")}
                    icon="ph-shield-check"
                    inputmode={Some(AttrValue::Static("numeric"))}
                    autocomplete={Some(AttrValue::Static("one-time-code"))}
                    autofocus=true
                    oninput={on_mfa_change}
                />
                <PlayButton label={i18n.t("login.verify")} icon="ph-shield-check" onclick={Some(on_mfa_submit)} />
            </form>
        </div>
    };

    let modal_content = {
        if !*first_admin_created {
            html! {
                <FirstAdminModal
                    on_submit={on_admin_setup}
                />
            }
        } else {
            html! {}
        }
    };

    let busy = *submitting && *page_state == PageState::Default;
    let server_host = window().and_then(|w| w.location().host().ok());

    let on_oidc = {
        let i18n_oidc_login_failed = i18n_oidc_login_failed.clone();
        Callback::from(move |provider: login_requests::OIDCProvider| {
            let window = web_sys::window().expect("no global `window` exists");
            let origin = window.location().origin().expect("should have an origin");
            let server_name = origin.trim_end_matches('/').to_string();
            let (state, code_verifier, code_challenge) = oidc_secrets();
            // Remembered here so the callback only finishes a login this browser started.
            if let Ok(Some(storage)) = window.session_storage() {
                let _ = storage.set_item(crate::components::oauth_callback::OIDC_STATE_KEY, &state);
            }
            let i18n_oidc_login_failed = i18n_oidc_login_failed.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let redirect_uri = format!("{}/api/auth/callback", origin);
                match call_store_oidc_state(
                    server_name,
                    state.clone(),
                    provider.client_id.clone(),
                    Some(origin.clone()),
                    Some(code_verifier),
                    Some(redirect_uri.clone()),
                ).await {
                    Ok(_) => {
                        let auth_url = oidc_authorization_url(&provider, &redirect_uri, &state, &code_challenge);
                        window.location().set_href(&auth_url).unwrap();
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Failed to store state: {:?}", e).into());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!("{}: {}", i18n_oidc_login_failed, e));
                        });
                    }
                }
            });
        })
    };
    // The divider separates the OIDC buttons from the standard login form, so skip it when
    // that form is hidden.
    let oidc_block = html! {
        <OidcButtons
            providers={(*oidc_providers).clone()}
            divider={!*disable_standard_login}
            onclick={on_oidc}
        />
    };

    let main_panel = html! {
        <div class="lp-panel" key="main">
            <CardHead
                title={i18n.t("login.welcome_back")}
                subtitle={Some(AttrValue::from(i18n.t("login.sign_in_subtitle")))}
            />
            if !*disable_standard_login {
                <form onsubmit={on_form_submit}>
                    <LoginField
                        id="username"
                        label={i18n.t("auth.username")}
                        icon="ph-user"
                        autocomplete={Some(AttrValue::Static("username"))}
                        oninput={on_login_username_change}
                    />
                    <LoginField
                        id="current-password"
                        kind="password"
                        label={i18n.t("auth.password")}
                        icon="ph-lock-key"
                        autocomplete={Some(AttrValue::Static("current-password"))}
                        oninput={on_login_password_change}
                    />
                    <div class="lp-row">
                        <button type="button" class="lp-link" onclick={on_forgot_password}>
                            { &i18n_forgot_password_link }
                        </button>
                    </div>
                    <PlayButton label={i18n_login.clone()} busy={busy} />
                </form>
            }
            { oidc_block }
            if *self_service_enabled && !*disable_standard_login {
                <p class="lp-note">
                    { i18n.t("login.new_here") }
                    <button type="button" class="lp-link" onclick={on_create_new_user.clone()}>
                        { i18n.t("login.create_account") }
                    </button>
                </p>
            }
        </div>
    };

    let card_body = match *page_state {
        PageState::CreateUser => create_user_modal,
        PageState::ForgotPassword => forgot_password_modal,
        PageState::ForgotPasswordLoading => forgot_password_loading_modal,
        PageState::MFAPrompt => mfa_code_modal,
        PageState::EnterCode => enter_code_modal,
        _ => main_panel,
    };

    let corner = html! {
        <button type="button" class="lp-ghost" onclick={on_different_server}>
            <i class="ph ph-arrows-left-right" aria-hidden="true"></i>
            <span>{ &i18n_connect_different_server }</span>
        </button>
    };

    html! {
        <>
        if *loading {
            <div class="loading-animation fixed inset-0 flex justify-center items-center bg-opacity-80 z-50">
                <div class="frame1"></div>
                <div class="frame2"></div>
                <div class="frame3"></div>
                <div class="frame4"></div>
                <div class="frame5"></div>
                <div class="frame6"></div>
            </div>
        } else {
            if crate::desktop::is_desktop() {
                // No server behind this page in the desktop app: ask for one.
                <ChangeServer />
            } else {
                <LoginShell tuning={busy} server={server_host} corner={corner}>
                    <div class="lp-card">{ card_body }</div>
                </LoginShell>
            }
            if *page_state == PageState::TimeZone {
                { time_zone_setup_modal }
            }
            { modal_content }
            <ToastNotification />
        }
        </>
    }
}

/// A fresh OIDC `state`, and a PKCE verifier with its S256 challenge (always sent, so
/// PKCE-enforcing providers accept the flow).
fn oidc_secrets() -> (String, String, String) {
    let crypto = web_sys::window().unwrap().crypto().unwrap();
    let mut state_bytes = [0u8; 16];
    crypto.get_random_values_with_u8_array(&mut state_bytes).unwrap();
    let state = state_bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    let mut verifier_bytes = [0u8; 32];
    crypto.get_random_values_with_u8_array(&mut verifier_bytes).unwrap();
    let code_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(verifier_bytes);
    let code_challenge = login_requests::pkce_code_challenge_s256(&code_verifier);
    (state, code_verifier, code_challenge)
}

/// The provider's sign-in page for this login.
fn oidc_authorization_url(
    provider: &login_requests::OIDCProvider,
    redirect_uri: &str,
    state: &str,
    code_challenge: &str,
) -> String {
    let separator = if provider.authorization_url.contains('?') { '&' } else { '?' };
    format!(
        "{}{}client_id={}&redirect_uri={}&scope={}&response_type=code&state={}&code_challenge={}&code_challenge_method=S256",
        provider.authorization_url,
        separator,
        urlencoding::encode(&provider.client_id),
        urlencoding::encode(redirect_uri),
        urlencoding::encode(&provider.scope),
        state,
        code_challenge
    )
}

#[derive(Properties, PartialEq)]
struct OidcButtonsProps {
    providers: Vec<login_requests::OIDCProvider>,
    divider: bool,
    onclick: Callback<login_requests::OIDCProvider>,
}

/// A sign-in button for each OIDC provider the server offers.
#[function_component(OidcButtons)]
fn oidc_buttons(props: &OidcButtonsProps) -> Html {
    let (i18n, _) = use_translation();
    if props.providers.is_empty() {
        return html! {};
    }
    html! {
        <>
            if props.divider {
                <div class="lp-divider">{ i18n.t("login.or_continue_with") }</div>
            }
            <div class="lp-oidc">
                { for props.providers.iter().map(|provider| {
                    let button_style = format!(
                        "background-color: {}; color: {}",
                        provider.button_color, provider.button_text_color
                    );
                    let button_text_style = format!("color: {}", provider.button_text_color);
                    let onclick = {
                        let provider = provider.clone();
                        props.onclick.reform(move |_: MouseEvent| provider.clone())
                    };
                    html! {
                        <button class="lp-oidc-button" type="button" style={button_style} {onclick}>
                            if let Some(icon) = &provider.icon_svg {
                                <div class="lp-oidc-icon">
                                    {Html::from_html_unchecked(AttrValue::from(icon.clone()))}
                                </div>
                            }
                            <span style={button_text_style}>{&provider.button_text}</span>
                        </button>
                    }
                }) }
            </div>
        </>
    }
}

// Store a login that passed its second factor, load the user's theme and time settings, then
// go to their start page.
async fn finish_verified_login(
    dispatch: &Dispatch<AppState>,
    user_details: login_requests::GetUserDetails,
    login_request: login_requests::LoginServerRequest,
    server_details: login_requests::GetApiDetails,
) {
    let server_name = login_request.server_name.clone();
    let api_key = login_request.api_key.clone().unwrap_or_default();
    let user_id = user_details.UserID;
    let gravatar_url = generate_gravatar_url(&user_details.Email, 80);
    dispatch.reduce_mut(move |state| {
        state.user_details = Some(user_details);
        state.auth_details = Some(login_request);
        state.server_details = Some(server_details);
        state.store_app_state();
    });
    Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
        state.gravatar_url = Some(gravatar_url);
    });
    let window = web_sys::window();
    if let Some(storage) = window.as_ref().and_then(|w| w.session_storage().ok().flatten()) {
        let _ = storage.set_item("isAuthenticated", "true");
    }
    if let Ok(theme) = call_get_theme(server_name.clone(), api_key.clone(), &user_id).await {
        crate::components::setting_components::theme_options::changeTheme(&theme);
        if let Some(storage) = window.as_ref().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item("selected_theme", &theme);
        }
    }
    if let Ok(tz) = call_get_time_info(server_name.clone(), api_key.clone(), &user_id).await {
        Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
            state.user_tz = Some(tz.timezone);
            state.hour_preference = Some(tz.hour_pref);
            state.date_format = Some(tz.date_format);
        });
    }
    let start_page = call_get_startpage(&server_name, &api_key, &user_id).await.unwrap_or_default();
    BrowserHistory::new().push(if start_page.is_empty() { "/home" } else { start_page.as_str() });
}

/// The account behind the API key an OIDC sign-in returned, and whether its first-login
/// setup is done.
async fn load_oidc_login(
    server: &str,
    api_key: &str,
) -> anyhow::Result<(
    login_requests::GetUserDetails,
    login_requests::LoginServerRequest,
    login_requests::GetApiDetails,
    bool,
)> {
    call_verify_key(server, api_key).await?;
    let user_id = login_requests::call_get_user_id(server, api_key)
        .await?
        .retrieved_id
        .ok_or_else(|| anyhow::anyhow!("the server found no user for this key"))?;
    let user_details = login_requests::call_get_user_details(server, api_key, &user_id).await?;
    let server_details = login_requests::call_get_api_config(server, api_key).await?;
    let setup_done = call_first_login_done(server.to_string(), api_key.to_string(), &user_id).await?;
    let login_request = login_requests::LoginServerRequest {
        server_name: server.to_string(),
        username: None,
        password: None,
        api_key: Some(api_key.to_string()),
    };
    Ok((user_details, login_request, server_details, setup_done))
}

#[function_component(ChangeServer)]
pub fn login() -> Html {
    let (i18n, _) = use_translation();
    // Desktop app: offer the downloads when there are any (they play without the server).
    let has_local_downloads = use_selector(|s: &crate::components::context::EpisodeStatusState| {
        s.downloaded_episodes.local_ids().next().is_some()
    });

    // Pre-capture translation strings for use in closures and async blocks
    let i18n_error_checking_mfa = i18n.t("login.error_checking_mfa_status").to_string();
    let i18n_error_checking_first_login = i18n
        .t("login.error_checking_first_login_status")
        .to_string();
    let i18n_credentials_incorrect = i18n.t("login.credentials_incorrect").to_string();
    let i18n_error_setting_up_timezone = i18n.t("login.error_setting_up_timezone").to_string();
    let i18n_initial_setup = i18n.t("login.initial_setup").to_string();
    let i18n_hour_format = i18n.t("login.hour_format").to_string();
    let i18n_time_zone = i18n.t("login.time_zone").to_string();
    let i18n_date_format = i18n.t("login.date_format").to_string();
    let i18n_save_preferences = i18n.t("login.save_preferences").to_string();
    let i18n_mfa_login = i18n.t("login.mfa_login").to_string();
    let i18n_connect_local_server = i18n.t("login.connect_local_server").to_string();
    let i18n_error_parsing_time = i18n.t("login.error_parsing_time").to_string();
    let i18n_language_updated = i18n.t("login.language_updated").to_string();
    let i18n_error_validating_mfa = i18n.t("login.error_validating_mfa").to_string();
    let i18n_language = i18n.t("common.language").to_string();
    let i18n_login_button = i18n.t("login.login_button").to_string();
    let i18n_date_format_julian = i18n.t("login.date_format_julian").to_string();
    let i18n_date_format_iso8601 = i18n.t("login.date_format_iso8601").to_string();

    let (_app_state, _app_dispatch) = use_store::<AppState>();
    let (_state, _dispatch) = use_store::<UIState>();
    let history = BrowserHistory::new();
    let server_name = use_state(|| "".to_string());
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());
    let (_app_state, dispatch) = use_store::<AppState>();
    let time_zone = use_state(|| "America/New_York".to_string());
    let date_format = use_state(|| "MDY".to_string());
    let time_pref = use_state(|| 12);
    let mfa_code = use_state(|| "".to_string());
    let temp_api_key = use_state(|| "".to_string());
    let temp_user_id = use_state(|| 0);
    let temp_server_name = use_state(|| "".to_string());
    // Set when the server asks for a second factor before handing out an API key.
    let mfa_session_token = use_state(|| "".to_string());
    let page_state = use_state(|| PageState::Default);
    let submitting = use_state(|| false);
    let user_language = use_state(|| "en".to_string());
    let available_languages: UseStateHandle<Vec<AvailableLanguage>> = use_state(Vec::new);

    // OIDC in the desktop app: the providers of the server typed in, and sign-ins that finish
    // in the system browser and come back as a pinepods:// link.
    let oidc_server = server_name.trim().trim_end_matches('/').to_string();
    let oidc_providers: UseStateHandle<(String, Vec<login_requests::OIDCProvider>)> =
        use_state(Default::default);
    {
        let oidc_providers = oidc_providers.clone();
        use_effect_with(oidc_server.clone(), move |server| {
            let server = server.clone();
            let looks_like_url = server.starts_with("http://") || server.starts_with("https://");
            // Wait for typing to pause before asking the server.
            let timeout = (crate::desktop::is_desktop() && looks_like_url).then(|| {
                gloo_timers::callback::Timeout::new(500, move || {
                    wasm_bindgen_futures::spawn_local(async move {
                        let providers = call_get_public_oidc_providers(server.clone())
                            .await
                            .map(|r| r.providers)
                            .unwrap_or_default();
                        oidc_providers.set((server, providers));
                    });
                })
            });
            move || drop(timeout)
        });
    }
    let shown_providers = if oidc_providers.0 == oidc_server { oidc_providers.1.clone() } else { Vec::new() };
    let waiting_for_browser =
        use_selector(|s: &crate::desktop::BrowserSignIn| s.pending.is_some());
    let returned_link = use_selector(|s: &crate::desktop::BrowserSignIn| s.returned.clone());

    let on_desktop_oidc = {
        let i18n_oidc_login_failed = i18n.t("login.oidc_login_failed").to_string();
        let i18n_browser_failed = i18n.t("desktop.sign_in_browser_failed").to_string();
        let server = oidc_server.clone();
        Callback::from(move |provider: login_requests::OIDCProvider| {
            let server = server.clone();
            let (state, code_verifier, code_challenge) = oidc_secrets();
            let (i18n_oidc_login_failed, i18n_browser_failed) =
                (i18n_oidc_login_failed.clone(), i18n_browser_failed.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let redirect_uri = format!("{server}/api/auth/callback");
                let sign_in = Dispatch::<crate::desktop::BrowserSignIn>::global();
                let stored = call_store_oidc_state(
                    server.clone(),
                    state.clone(),
                    provider.client_id.clone(),
                    Some(crate::desktop::SIGN_IN_LINK.to_string()),
                    Some(code_verifier),
                    Some(redirect_uri.clone()),
                )
                .await;
                let result = match stored {
                    Err(e) => Err(format!("{i18n_oidc_login_failed}: {e}")),
                    Ok(()) => {
                        let pending = crate::desktop::PendingSignIn { server, state: state.clone() };
                        sign_in.reduce_mut(move |s| s.pending = Some(pending));
                        let url = oidc_authorization_url(&provider, &redirect_uri, &state, &code_challenge);
                        crate::desktop::open_in_browser(url)
                            .await
                            .map_err(|e| format!("{i18n_browser_failed}: {e}"))
                    }
                };
                if let Err(message) = result {
                    sign_in.reduce_mut(|s| s.pending = None);
                    crate::desktop::log_error(format!("OIDC sign-in: {message}"));
                    Dispatch::<NotificationState>::global()
                        .reduce_mut(move |s| s.error_message = Some(message));
                }
            });
        })
    };
    let on_cancel_sign_in = Callback::from(|_: MouseEvent| {
        Dispatch::<crate::desktop::BrowserSignIn>::global().reduce_mut(|s| s.pending = None);
    });


    // Load available languages
    let effect_available_languages = available_languages.clone();
    use_effect_with((), move |_| {
        let languages = effect_available_languages.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let window = web_sys::window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location
                .origin()
                .expect("should have an origin")
                .trim_end_matches('/')
                .to_string();
            match call_get_available_languages(server_name).await {
                Ok(response) => {
                    languages.set(response);
                }
                Err(e) => {
                    web_sys::console::log_1(
                        &format!("Error fetching available languages: {:?}", e).into(),
                    );
                }
            }
        });
        || ()
    });

    let server_display = server_name.clone();
    let on_server_name_change = {
        let server_name = server_name.clone();
        Callback::from(move |e: InputEvent| {
            server_name.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_username_change = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            username.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let on_password_change = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            password.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };

    let history_clone = history.clone();
    // let app_state_clone = app_state.clone();
    let submit_state = page_state.clone();
    let call_server_name = temp_server_name.clone();
    let call_api_key = temp_api_key.clone();
    let call_user_id = temp_user_id.clone();
    let submit_post_state = _app_dispatch.clone();
    let submit_mfa_session_token = mfa_session_token.clone();
    let on_submit = {
        let submit_dispatch = dispatch.clone();
        let i18n_error_checking_mfa = (&i18n_error_checking_mfa).clone();
        let i18n_error_checking_first_login = (&i18n_error_checking_first_login).clone();
        let i18n_credentials_incorrect = (&i18n_credentials_incorrect).clone();
        let submitting = submitting.clone();
        Callback::from(move |_| {
            let submitting = submitting.clone();
            submitting.set(true);
            let i18n_error_checking_mfa = (&i18n_error_checking_mfa).clone();
            let i18n_error_checking_first_login = (&i18n_error_checking_first_login).clone();
            let i18n_credentials_incorrect = (&i18n_credentials_incorrect).clone();
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            let dispatch = submit_dispatch.clone();
            let _post_state = submit_post_state.clone();
            let server_name = server_name.clone();
            let page_state = submit_state.clone();
            let temp_server_name = call_server_name.clone();
            let temp_api_key = call_api_key.clone();
            let temp_user_id = call_user_id.clone();
            let mfa_session_token = submit_mfa_session_token.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // let server_name = location.origin().expect("should have an origin");
                let server_name = server_name.clone();
                let page_state = page_state.clone();
                match login_requests::login_new_server_secure(
                    server_name.to_string(),
                    username.to_string(),
                    password.to_string(),
                )
                .await
                {
                    Ok(login_requests::LoginResult::MfaRequired {
                        server_name: mfa_server,
                        user_id: mfa_user_id,
                        mfa_session_token: token,
                        ..
                    }) => {
                        temp_server_name.set(mfa_server);
                        temp_user_id.set(mfa_user_id);
                        mfa_session_token.set(token);
                        page_state.set(PageState::MFAPrompt);
                    }
                    Ok(login_requests::LoginResult::Success(user_details, login_request, server_details)) => {
                        // After user login, update the image URL with user's email from user_details
                        let gravatar_url = generate_gravatar_url(&user_details.Email, 80); // 80 is the image size
                        let key_copy = login_request.clone();
                        let user_copy = user_details.clone();
                        dispatch.reduce_mut(move |state| {
                            state.user_details = Some(user_details);
                            state.auth_details = Some(login_request);
                            state.server_details = Some(server_details);
                            state.store_app_state();
                        });
                        Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                            state.gravatar_url = Some(gravatar_url);
                        });

                        // Extract server_name, api_key, and user_id
                        let server_name = key_copy.server_name;
                        let api_key = key_copy.api_key;
                        let user_id = user_copy.UserID;

                        temp_server_name.set(server_name.clone());
                        temp_api_key.set(api_key.clone().unwrap());
                        temp_user_id.set(user_id.clone());

                        match call_first_login_done(
                            server_name.clone(),
                            api_key.clone().unwrap(),
                            &user_id,
                        )
                        .await
                        {
                            Ok(first_login_done) => {
                                if first_login_done {
                                    match call_check_mfa_enabled(
                                        server_name.clone(),
                                        api_key.clone().unwrap(),
                                        &user_id,
                                    )
                                    .await
                                    {
                                        Ok(response) => {
                                            if response.mfa_enabled {
                                                page_state.set(PageState::MFAPrompt);
                                            } else {
                                                let theme_api = api_key.clone();
                                                let theme_server = server_name.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    match call_get_theme(
                                                        theme_server,
                                                        theme_api.unwrap(),
                                                        &user_id,
                                                    )
                                                    .await
                                                    {
                                                        Ok(theme) => {
                                                            crate::components::setting_components::theme_options::changeTheme(&theme);
                                                            if let Some(window) = web_sys::window()
                                                            {
                                                                if let Ok(Some(local_storage)) =
                                                                    window.local_storage()
                                                                {
                                                                    match local_storage.set_item("selected_theme", &theme) {
                                                                        Ok(_) => console::log_1(&"Updated theme in local storage".into()),
                                                                        Err(e) => console::log_1(&format!("Error updating theme in local storage: {:?}", e).into()),
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        Err(_e) => {
                                                            // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                                        }
                                                    }
                                                });
                                                let time_server = server_name.clone();
                                                let time_api = api_key.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    match call_get_time_info(
                                                        time_server,
                                                        time_api.unwrap(),
                                                        &user_id,
                                                    )
                                                    .await
                                                    {
                                                        Ok(tz_response) => {
                                                            Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                                                                state.user_tz =
                                                                    Some(tz_response.timezone);
                                                                state.hour_preference =
                                                                    Some(tz_response.hour_pref);
                                                                state.date_format =
                                                                    Some(tz_response.date_format);
                                                            });
                                                        }
                                                        Err(_e) => {
                                                            // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                                        }
                                                    }
                                                });
                                                // Add start page retrieval before redirecting
                                                let startpage_api = api_key.clone();
                                                let startpage_server = server_name.clone();
                                                let startpage_user_id = user_id.clone();
                                                let startpage_history = history.clone();

                                                wasm_bindgen_futures::spawn_local(async move {
                                                    // Try to get the user's configured start page
                                                    match call_get_startpage(
                                                        &startpage_server,
                                                        &startpage_api.unwrap(),
                                                        &startpage_user_id,
                                                    )
                                                    .await
                                                    {
                                                        Ok(start_page) => {
                                                            if !start_page.is_empty() {
                                                                // Use user's configured start page
                                                                startpage_history.push(&start_page);
                                                            } else {
                                                                // Empty start page, use default
                                                                startpage_history.push("/home");
                                                            }
                                                        }
                                                        Err(_) => {
                                                            // Failed to get start page, use default
                                                            startpage_history.push("/home");
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        Err(_) => {
                                            submitting.set(false);
                                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                state.error_message =
                                                    Option::from((&i18n_error_checking_mfa).clone())
                                            });
                                        }
                                    }
                                } else {
                                    page_state.set(PageState::TimeZone);
                                }
                            }
                            Err(_) => {
                                submitting.set(false);
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message =
                                        Option::from((&i18n_error_checking_first_login).clone())
                                });
                            }
                        }
                    }
                    Err(_) => {
                        // console::log_1(&format!("Error logging into server: {}", server_name).into());
                        submitting.set(false);
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message =
                                Option::from((&i18n_credentials_incorrect).clone())
                        });
                        // Handle error
                    }
                }
            });
        })
    };

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Default,
        TimeZone,
        MFAPrompt,
    }

    let history_clone = history.clone();
    let on_different_server = {
        Callback::from(move |_| {
            let history = history_clone.clone();
            history.push("/"); // Use the route path
        })
    };
    // Define the callback function for closing the modal
    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Default);
        })
    };



    let on_tz_change = {
        let tz = time_zone.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            tz.set(select_element.value());
        })
    };
    let on_df_change = {
        let df = date_format.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            df.set(select_element.value());
        })
    };
    let _time_state_error = _app_dispatch.clone();
    let on_time_pref_change = {
        let time_pref = time_pref.clone();
        let i18n_error_parsing_time = i18n_error_parsing_time.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            let value_str = select_element.value();
            if let Ok(value_int) = value_str.parse::<i32>() {
                time_pref.set(value_int);
            } else {
                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                    state.error_message = Option::from(i18n_error_parsing_time.clone())
                });
            }
        })
    };

    let on_language_change = {
        let user_language = user_language.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            user_language.set(select_element.value());
        })
    };

    let dispatch_time = _app_dispatch.clone();
    let on_time_zone_submit = {
        // let (state, dispatch) = use_store::<AppState>();
        let page_state = page_state.clone();
        let time_pref = time_pref.clone();
        let time_zone = time_zone.clone();
        let date_format = date_format.clone();
        let user_language = user_language.clone();
        // let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        // let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        // let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let temp_server_name = temp_server_name.clone();
        let temp_api_key = temp_api_key.clone();
        let temp_user_id = temp_user_id.clone();
        let history = history.clone();
        // let error_message_create = error_message.clone();
        let i18n_error_checking_mfa = (&i18n_error_checking_mfa).clone();
        let i18n_error_setting_up_timezone = (&i18n_error_setting_up_timezone).clone();
        let i18n_language_updated = i18n_language_updated.clone();
        Callback::from(move |e: MouseEvent| {
            let i18n_error_checking_mfa = (&i18n_error_checking_mfa).clone();
            let i18n_error_setting_up_timezone = (&i18n_error_setting_up_timezone).clone();
            let i18n_language_updated = i18n_language_updated.clone();
            let _post_state = dispatch_time.clone();
            e.prevent_default();
            let server_name = (*temp_server_name).clone();
            let api_key = (*temp_api_key).clone();
            let user_id = *temp_user_id;
            let page_state = page_state.clone();
            let history = history.clone();
            let user_language_clone = user_language.clone();
            // let error_message_clone = error_message_create.clone();
            e.prevent_default();
            // page_state.set(PageState::Default);

            let timezone_info = TimeZoneInfo {
                user_id: *temp_user_id, // assuming temp_user_id is a use_state of i32
                timezone: (*time_zone).clone(),
                hour_pref: *time_pref,
                date_format: (*date_format).clone(),
            };

            wasm_bindgen_futures::spawn_local(async move {
                // Directly use timezone_info without checking it against time_zone_setup
                match call_setup_timezone_info(server_name.clone(), api_key.clone(), timezone_info)
                    .await
                {
                    Ok(success) => {
                        if success.success {
                            // Update language preference if not English
                            let user_lang = (*user_language_clone).to_string();
                            if !user_lang.is_empty() && user_lang != "en" {
                                match call_update_user_language(
                                    server_name.clone(),
                                    api_key.clone(),
                                    user_id,
                                    user_lang,
                                )
                                .await
                                {
                                    Ok(_) => {
                                        web_sys::console::log_1(
                                            &i18n_language_updated.clone().into(),
                                        );
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(
                                            &format!("Failed to update language preference: {}", e)
                                                .into(),
                                        );
                                        // Don't fail the entire setup if language update fails
                                    }
                                }
                            }
                            page_state.set(PageState::Default);
                            match call_check_mfa_enabled(
                                server_name.clone(),
                                api_key.clone(),
                                &user_id,
                            )
                            .await
                            {
                                Ok(response) => {
                                    if response.mfa_enabled {
                                        page_state.set(PageState::MFAPrompt);
                                    } else {
                                        // Add start page retrieval before redirecting
                                        let startpage_api = api_key.clone();
                                        let startpage_server = server_name.clone();
                                        let startpage_user_id = user_id.clone();
                                        let startpage_history = history.clone();

                                        wasm_bindgen_futures::spawn_local(async move {
                                            // Try to get the user's configured start page
                                            match call_get_startpage(
                                                &startpage_server,
                                                &startpage_api,
                                                &startpage_user_id,
                                            )
                                            .await
                                            {
                                                Ok(start_page) => {
                                                    if !start_page.is_empty() {
                                                        // Use user's configured start page
                                                        startpage_history.push(&start_page);
                                                    } else {
                                                        // Empty start page, use default
                                                        startpage_history.push("/home");
                                                    }
                                                }
                                                Err(_) => {
                                                    // Failed to get start page, use default
                                                    startpage_history.push("/home");
                                                }
                                            }
                                        });
                                    }
                                }
                                Err(_) => {
                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                        state.error_message =
                                            Option::from((&i18n_error_checking_mfa).clone())
                                    });
                                }
                            }
                        } else {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Option::from((&i18n_error_setting_up_timezone).clone())
                            });
                            page_state.set(PageState::Default);
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        // dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error setting up time zone: {:?}", e)));
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!(
                                "Error setting up time zone: {:?}",
                                formatted_error
                            ))
                        });
                    }
                }
            });
        })
    };

    fn render_time_zone_option(tz: Tz) -> Html {
        html! {
            <option value={tz.name()}>{tz.name()}</option>
        }
    }

    let time_zone_setup_modal = html! {
            <div class="modal-overlay">
                <div class="item_container-text modal-content">
                    // Header
                    <div class="item_container-text modal-header">
                        <i class="ph ph-clock text-xl"></i>
                        <h3 class="text-lg">{&i18n_initial_setup}</h3>
                    </div>

                    // Content
                    <div class="modal-body">
                        <form>
                            <div class="modal-welcome">
                                <i class="ph ph-hand-waving text-xl"></i>
                                <p>
    {&i18n.t("login.first_time_welcome")}
                                </p>
                            </div>

                            <div class="modal-form-group">
                                <label class="modal-label">
                                    <i class="ph ph-translate"></i>
                                    <span>{&i18n_language}</span>
                                </label>
                                <select
                                    id="language"
                                    name="language"
                                    class="modal-select"
                                    oninput={on_language_change.clone()}
                                    value={(*user_language).clone()}
                                >
                                    { for available_languages.iter().map(|lang| {
                                        let selected = lang.code == *user_language;
                                        html! {
                                            <option value={lang.code.clone()} selected={selected}>{&lang.name}</option>
                                        }
                                    })}
                                </select>
                            </div>

                            <div class="modal-form-group">
                                <label class="modal-label">
                                    <i class="ph ph-clock-clockwise"></i>
                                    <span>{&i18n_hour_format}</span>
                                </label>
                                <select
                                    id="hour_format"
                                    name="hour_format"
                                    class="modal-select"
                                    oninput={on_time_pref_change}
                                    value={(*time_pref).to_string()}
                                >
                                    <option value="12" selected={*time_pref == 12}>{"12 Hour"}</option>
                                    <option value="24" selected={*time_pref == 24}>{"24 Hour"}</option>
                                </select>
                            </div>

                            <div class="modal-form-group">
                                <label class="modal-label">
                                    <i class="ph ph-globe"></i>
                                    <span>{&i18n_time_zone}</span>
                                </label>
                                <select
                                    id="time_zone"
                                    name="time_zone"
                                    class="modal-select"
                                    oninput={on_tz_change}
                                    value={(*time_zone).clone()}
                                >
                                    { for TZ_VARIANTS.iter().map(|tz| render_time_zone_option(*tz)) }
                                </select>
                            </div>

                            <div class="modal-form-group">
                                <label class="modal-label">
                                    <i class="ph ph-calendar"></i>
                                    <span>{&i18n_date_format}</span>
                                </label>
                                <select
                                    id="date_format"
                                    name="date_format"
                                    class="modal-select"
                                    oninput={on_df_change}
                                    value={(*date_format).clone()}
                                >
                                    <option value="MDY" selected={*date_format == "MDY"}>{"MM-DD-YYYY"}</option>
                                    <option value="DMY" selected={*date_format == "DMY"}>{"DD-MM-YYYY"}</option>
                                    <option value="YMD" selected={*date_format == "YMD"}>{"YYYY-MM-DD"}</option>
                                    <option value="JUL" selected={*date_format == "JUL"}>{ &i18n_date_format_julian }</option>
                                    <option value="ISO" selected={*date_format == "ISO"}>{ &i18n_date_format_iso8601 }</option>
                                    <option value="USA" selected={*date_format == "USA"}>{"MM/DD/YYYY"}</option>
                                    <option value="EUR" selected={*date_format == "EUR"}>{"DD.MM.YYYY"}</option>
                                    <option value="JIS" selected={*date_format == "JIS"}>{"YYYY-MM-DD"}</option>
                                </select>
                            </div>

                            <button
                                type="submit"
                                onclick={on_time_zone_submit}
                                class="modal-button"
                            >
                                <i class="ph ph-check"></i>
                                <span>{&i18n_save_preferences}</span>
                            </button>
                        </form>
                    </div>
                </div>
            </div>
        };

    let on_mfa_change = {
        let mfa_code = mfa_code.clone();
        Callback::from(move |e: InputEvent| {
            mfa_code.set(
                e.target_unchecked_into::<web_sys::HtmlInputElement>()
                    .value(),
            );
        })
    };
    let post_state = _app_dispatch.clone();
    let on_mfa_submit = {
        let (state, dispatch) = use_store::<AppState>();
        let page_state = page_state.clone();
        let mfa_code = mfa_code.clone();
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let history = history.clone();
        // let error_message_create = error_message.clone();
        let dispatch_wasm = dispatch.clone();
        let i18n_error_validating_mfa = i18n_error_validating_mfa.clone();
        let mfa_session_token = mfa_session_token.clone();
        let mfa_server_name = temp_server_name.clone();
        Callback::from(move |e: MouseEvent| {
            let i18n_error_validating_mfa = i18n_error_validating_mfa.clone();
            let _dispatch = dispatch_wasm.clone();
            let mfa_code = mfa_code.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let page_state = page_state.clone();
            let history = history.clone();
            let _post_state = post_state.clone();
            // let error_message_clone = error_message_create.clone();
            e.prevent_default();

            if !mfa_session_token.is_empty() {
                let dispatch = _dispatch.clone();
                let server = (*mfa_server_name).clone();
                let token = (*mfa_session_token).clone();
                let code = (*mfa_code).clone();
                let i18n_error_validating_mfa = i18n_error_validating_mfa.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match login_requests::complete_mfa_login(server, "".to_string(), token, code).await {
                        Ok((user_details, login_request, server_details)) => {
                            page_state.set(PageState::Default);
                            finish_verified_login(&dispatch, user_details, login_request, server_details).await;
                        }
                        Err(_) => {
                            page_state.set(PageState::Default);
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Option::from(i18n_error_validating_mfa)
                            });
                        }
                    }
                });
                return;
            }

            wasm_bindgen_futures::spawn_local(async move {
                match call_verify_mfa(
                    &server_name.clone().unwrap(),
                    &api_key.clone().unwrap().unwrap(),
                    user_id.clone().unwrap(),
                    (*mfa_code).clone(),
                )
                .await
                {
                    Ok(response) => {
                        if response.verified {
                            page_state.set(PageState::Default);
                            let theme_api = api_key.clone();
                            let theme_server = server_name.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match call_get_theme(
                                    theme_server.unwrap(),
                                    theme_api.unwrap().unwrap(),
                                    &user_id.unwrap(),
                                )
                                .await
                                {
                                    Ok(theme) => {
                                        crate::components::setting_components::theme_options::changeTheme(&theme);
                                        if let Some(window) = web_sys::window() {
                                            if let Ok(Some(local_storage)) = window.local_storage()
                                            {
                                                match local_storage.set_item("selected_theme", &theme) {
                                                    Ok(_) => console::log_1(&"Updated theme in local storage".into()),
                                                    Err(e) => console::log_1(&format!("Error updating theme in local storage: {:?}", e).into()),
                                                }
                                            }
                                        }
                                    }
                                    Err(_e) => {
                                        // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                    }
                                }
                            });
                            let time_server = server_name.clone();
                            let time_api = api_key.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match call_get_time_info(
                                    time_server.unwrap(),
                                    time_api.unwrap().unwrap(),
                                    &user_id.unwrap(),
                                )
                                .await
                                {
                                    Ok(tz_response) => {
                                        Dispatch::<UserPreferencesState>::global().reduce_mut(move |state| {
                                            state.user_tz = Some(tz_response.timezone);
                                            state.hour_preference = Some(tz_response.hour_pref);
                                            state.date_format = Some(tz_response.date_format);
                                        });
                                    }
                                    Err(_e) => {
                                        // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                    }
                                }
                            });
                            // Add start page retrieval before redirecting
                            let startpage_api = api_key.clone();
                            let startpage_server = server_name.clone();
                            let startpage_user_id = user_id.clone();
                            let startpage_history = history.clone();

                            wasm_bindgen_futures::spawn_local(async move {
                                // Try to get the user's configured start page
                                match call_get_startpage(
                                    &startpage_server.unwrap(),
                                    &startpage_api.unwrap().unwrap(),
                                    &startpage_user_id.unwrap(),
                                )
                                .await
                                {
                                    Ok(start_page) => {
                                        if !start_page.is_empty() {
                                            // Use user's configured start page
                                            startpage_history.push(&start_page);
                                        } else {
                                            // Empty start page, use default
                                            startpage_history.push("/home");
                                        }
                                    }
                                    Err(_) => {
                                        // Failed to get start page, use default
                                        startpage_history.push("/home");
                                    }
                                }
                            });
                        } else {
                            page_state.set(PageState::Default);
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Option::from(i18n_error_validating_mfa.clone())
                            });
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!(
                                "Error setting up time zone: {:?}",
                                formatted_error
                            ))
                        });
                    }
                }
            });
        })
    };

    // A sign-in link came back from the browser: check it belongs to the sign-in started here,
    // then finish signing in like the web app's OIDC callback.
    {
        let dispatch = dispatch.clone();
        let page_state = page_state.clone();
        let submitting = submitting.clone();
        let (temp_server_name, temp_api_key, temp_user_id) =
            (temp_server_name.clone(), temp_api_key.clone(), temp_user_id.clone());
        let messages = [
            "desktop.sign_in_mismatch",
            "oauth_callback.username_conflict",
            "oauth_callback.no_access",
            "oauth_callback.invalid_provider",
            "oauth_callback.authentication_failed",
        ]
        .map(|key| i18n.t(key).to_string());
        use_effect_with((*returned_link).clone(), move |link| {
            if let Some(link) = link.clone() {
                use crate::desktop::{read_sign_in_link, BrowserSignIn, SignInResult};
                let sign_in = Dispatch::<BrowserSignIn>::global();
                let pending = sign_in.get().pending.clone();
                sign_in.reduce_mut(|s| {
                    s.returned = None;
                    s.pending = None;
                });
                let [mismatch, conflict, no_access, invalid_provider, failed] = messages;
                let error = match read_sign_in_link(&link, pending.as_ref()) {
                    SignInResult::NotStarted => {
                        crate::desktop::log_error("ignored a sign-in link: no sign-in was started here".into());
                        None
                    }
                    SignInResult::Mismatch => {
                        crate::desktop::log_error("ignored a sign-in link that doesn't match the sign-in started here".into());
                        Some(mismatch)
                    }
                    SignInResult::Failed(code) => {
                        crate::desktop::log_error(format!("OIDC sign-in failed: {code}"));
                        Some(match code.as_str() {
                            "username_conflict" => conflict,
                            "no_access" => no_access,
                            "invalid_provider" => invalid_provider,
                            _ => failed,
                        })
                    }
                    SignInResult::SignedIn { server, api_key } => {
                        submitting.set(true);
                        wasm_bindgen_futures::spawn_local(async move {
                            match load_oidc_login(&server, &api_key).await {
                                Ok((user_details, login_request, server_details, true)) => {
                                    finish_verified_login(&dispatch, user_details, login_request, server_details).await;
                                }
                                Ok((user_details, login_request, server_details, false)) => {
                                    // First sign-in: ask for the time zone, as a password login does.
                                    temp_server_name.set(server);
                                    temp_api_key.set(api_key);
                                    temp_user_id.set(user_details.UserID);
                                    dispatch.reduce_mut(move |state| {
                                        state.user_details = Some(user_details);
                                        state.auth_details = Some(login_request);
                                        state.server_details = Some(server_details);
                                        state.store_app_state();
                                    });
                                    page_state.set(PageState::TimeZone);
                                }
                                Err(e) => {
                                    submitting.set(false);
                                    crate::desktop::log_error(format!("OIDC sign-in: could not load the account: {e}"));
                                    Dispatch::<NotificationState>::global()
                                        .reduce_mut(move |s| s.error_message = Some(failed));
                                }
                            }
                        });
                        None
                    }
                };
                if let Some(message) = error {
                    Dispatch::<NotificationState>::global()
                        .reduce_mut(move |s| s.error_message = Some(message));
                }
            }
            || ()
        });
    }

    let waiting_panel = html! {
        <div class="lp-panel" key="oidc-wait">
            <CardHead
                title={i18n.t("desktop.sign_in_waiting_title")}
                subtitle={Some(AttrValue::from(i18n.t("desktop.sign_in_waiting_subtitle")))}
                on_back={Some(on_cancel_sign_in.clone())}
            />
            <p class="lp-note">
                <button type="button" class="lp-link" onclick={on_cancel_sign_in}>
                    { i18n.t("desktop.sign_in_cancel") }
                </button>
            </p>
        </div>
    };

    let mfa_code_modal = html! {
        <div class="lp-panel" key="mfa">
            <CardHead
                title={i18n_mfa_login.clone()}
                subtitle={Some(AttrValue::from(i18n.t("login.mfa_subtitle")))}
                on_back={Some(on_close_modal.clone())}
            />
            <form action="#">
                <LoginField
                    id="mfa_code"
                    label={i18n.t("auth.enter_mfa_code")}
                    icon="ph-shield-check"
                    inputmode={Some(AttrValue::Static("numeric"))}
                    autocomplete={Some(AttrValue::Static("one-time-code"))}
                    autofocus=true
                    oninput={on_mfa_change}
                />
                <PlayButton label={i18n.t("login.verify")} icon="ph-shield-check" onclick={Some(on_mfa_submit)} />
            </form>
        </div>
    };

    let busy = *submitting && *page_state == PageState::Default;
    let on_form_submit = {
        let on_submit = on_submit.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            on_submit.emit(());
        })
    };

    let main_panel = html! {
        <div class="lp-panel" key="main">
            <CardHead
                title={i18n.t("login.change_server_title")}
                subtitle={Some(AttrValue::from(i18n.t("login.change_server_subtitle")))}
            />
            <form onsubmit={on_form_submit}>
                <LoginField
                    id="server"
                    kind="url"
                    label={i18n.t("auth.server_name")}
                    icon="ph-hard-drives"
                    autocomplete={Some(AttrValue::Static("url"))}
                    oninput={on_server_name_change}
                />
                <LoginField
                    id="username"
                    label={i18n.t("auth.username")}
                    icon="ph-user"
                    autocomplete={Some(AttrValue::Static("username"))}
                    oninput={on_username_change}
                />
                <LoginField
                    id="current-password"
                    kind="password"
                    label={i18n.t("auth.password")}
                    icon="ph-lock-key"
                    autocomplete={Some(AttrValue::Static("current-password"))}
                    oninput={on_password_change}
                />
                <PlayButton label={i18n_login_button.clone()} busy={busy} />
            </form>
            <OidcButtons providers={shown_providers} divider=true onclick={on_desktop_oidc} />
        </div>
    };

    let card_body = match *page_state {
        PageState::MFAPrompt => mfa_code_modal,
        _ if *waiting_for_browser => waiting_panel,
        _ => main_panel,
    };

    // The desktop app has no "this server" to go back to; offer the downloads instead.
    let corner = if crate::desktop::is_desktop() {
        html! {
            if *has_local_downloads {
                <button type="button" class="lp-ghost" onclick={Callback::from(|_: MouseEvent| crate::desktop::go_offline())}>
                    <i class="ph ph-wifi-slash" aria-hidden="true"></i>
                    <span>{ i18n.t("desktop.listen_offline") }</span>
                </button>
            }
        }
    } else {
        html! {
            <button type="button" class="lp-ghost" onclick={on_different_server}>
                <i class="ph ph-arrow-left" aria-hidden="true"></i>
                <span>{ &i18n_connect_local_server }</span>
            </button>
        }
    };

    html! {
        <>
            <LoginShell tuning={busy} server={(!server_display.is_empty()).then(|| (*server_display).clone())} corner={corner}>
                <div class="lp-card">{ card_body }</div>
            </LoginShell>
            if *page_state == PageState::TimeZone {
                { time_zone_setup_modal }
            }
            <ToastNotification />
        </>
    }
}

#[function_component(LogOut)]
pub fn logout() -> Html {
    // Clear local and session storage except for 'user_theme'
    let window = web_sys::window().expect("no global `window` exists");
    let local_storage = window
        .local_storage()
        .expect("localStorage not enabled")
        .expect("localStorage is null");
    let session_storage = window
        .session_storage()
        .expect("sessionStorage not enabled")
        .expect("sessionStorage is null");

    // Save 'user_theme' value
    let selected_theme = local_storage
        .get_item("selected_theme")
        .expect("failed to get 'selected_theme'");
    let art_accents = local_storage
        .get_item(crate::components::art_accent::CardAppearance::STORAGE_KEY)
        .ok()
        .flatten();
    // Preserve the restore-in-progress flags. A server restore signs the admin out
    // (it truncates the Sessions table), but RestoreOverlay needs these to survive the
    // sign-out so it can show the overlay and poll after the reload below.
    let restore_active = local_storage.get_item("pinepods_restore_active").ok().flatten();
    let restore_server = local_storage.get_item("pinepods_restore_server").ok().flatten();

    // Clear storages
    local_storage.clear().expect("failed to clear localStorage");
    session_storage
        .clear()
        .expect("failed to clear sessionStorage");

    // Restore 'user_theme' value
    if let Some(theme) = selected_theme {
        local_storage
            .set_item("selected_theme", &theme)
            .expect("failed to set 'selected_theme'");
    }
    if let Some(value) = art_accents {
        let _ = local_storage
            .set_item(crate::components::art_accent::CardAppearance::STORAGE_KEY, &value);
    }
    // Cached page data belongs to the user signing out.
    wasm_bindgen_futures::spawn_local(crate::components::page_cache::clear());
    // Restore the restore-in-progress flags.
    if let (Some(active), Some(server)) = (restore_active, restore_server) {
        let _ = local_storage.set_item("pinepods_restore_active", &active);
        let _ = local_storage.set_item("pinepods_restore_server", &server);
    }

    // Force a full page reload (not a client-side route push) so that all
    // in-memory yewdux stores (AppState, PodcastFeedState, etc.) are wiped.
    // A client-side push leaves the previous user's data in WASM memory, which
    // leaks across user switches (e.g. a stale podcast list on /podcasts).
    // The desktop app's saved login goes first, or the next launch signs back in.
    wasm_bindgen_futures::spawn_local(async move {
        crate::components::context::AppState::clear_tauri_credentials().await;
        let _ = window.location().set_href("/");
    });

    html! {}
}
