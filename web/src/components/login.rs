use crate::components::context::{AppState, UIState};
use crate::components::gen_components::{AdminSetupData, FirstAdminModal};
use crate::components::gen_funcs::format_error_message;
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
use crate::requests::setting_reqs::{call_get_startpage, call_get_theme};
use chrono_tz::{Tz, TZ_VARIANTS};
use md5;
use rand::Rng;
use wasm_bindgen::JsCast;
use web_sys::{console, window};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

// Gravatar URL generation functions (outside of use_effect_with)
fn calculate_gravatar_hash(email: &String) -> String {
    format!("{:x}", md5::compute(email.to_lowercase()))
}

pub fn generate_gravatar_url(email: &Option<String>, size: usize) -> String {
    let hash = calculate_gravatar_hash(&email.clone().unwrap());
    format!("https://gravatar.com/avatar/{}?s={}", hash, size)
}

#[function_component(Login)]
pub fn login() -> Html {
    let history = BrowserHistory::new();
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());
    let new_username = use_state(|| "".to_string());
    let forgot_email = use_state(|| "".to_string());
    let forgot_username = use_state(|| "".to_string());
    let reset_password = use_state(|| "".to_string());
    let reset_code = use_state(|| "".to_string());
    let new_password = use_state(|| "".to_string());
    let email = use_state(|| "".to_string());
    let fullname = use_state(|| "".to_string());
    let (_app_state, dispatch) = use_store::<AppState>();
    let (_state, _dispatch) = use_store::<UIState>();
    let time_zone = use_state(|| "".to_string());
    let date_format = use_state(|| "".to_string());
    let time_pref = use_state(|| 12);
    let mfa_code = use_state(|| "".to_string());
    let temp_api_key = use_state(|| "".to_string());
    let temp_user_id = use_state(|| 0);
    let temp_server_name = use_state(|| "".to_string());
    let loading = use_state(|| true);
    // Define the initial state
    let page_state = use_state(|| PageState::Default);
    let self_service_enabled = use_state(|| false); // State to store self-service status
    let effect_self_service = self_service_enabled.clone();
    let first_admin_created = use_state(|| true);
    let oidc_providers = use_state(|| Vec::new());

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
                .href()
                .expect("should have a href")
                .trim_end_matches('/')
                .to_string();

            match call_self_service_login_status(server_name).await {
                Ok((status, admin_created)) => {
                    self_service_enabled.set(status);
                    first_admin_created.set(admin_created);
                }
                Err(e) => {
                    web_sys::console::log_1(&format!("Error checking status: {:?}", e).into());
                }
            }
        });

        || ()
    });

    let effect_providers = oidc_providers.clone();
    use_effect_with((), move |_| {
        let providers = effect_providers.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let window = web_sys::window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location
                .href()
                .expect("should have a href")
                .trim_end_matches('/')
                .to_string();

            match call_get_public_oidc_providers(server_name).await {
                Ok(response) => {
                    providers.set(response.providers);
                }
                Err(e) => {
                    web_sys::console::log_1(
                        &format!("Error fetching OIDC providers: {:?}", e).into(),
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
                                                                            let gravatar_url = generate_gravatar_url(&Some(wasm_email.clone().unwrap()), 80);
                                                                            // Auto login logic here
                                                                            final_dispatch.reduce_mut(move |state| {
                                                                            state.user_details = wasm_app_state.user_details;
                                                                            state.auth_details = Some(wasm_auth_details.clone());
                                                                            state.server_details = server_details.server_details;
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
                                                                                    effect_displatch.reduce_mut(move |state| {
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
                                                                        Err(_) => {
                                                                            effect_loading
                                                                                .set(false);
                                                                            history.push("/");
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

    // This effect runs only once when the component mounts
    let background_image_url = use_state(|| String::new());
    let effect_background_image = background_image_url.clone();
    // This effect runs only once when the component mounts
    use_effect_with(
        (), // Dependencies, an empty tuple here signifies no dependencies.
        move |_| {
            let background_number = rand::thread_rng().gen_range(1..=9); // Assuming you have images named 1.jpg through 9.jpg.
            effect_background_image.set(format!(
                "static/assets/backgrounds/{}.jpg",
                background_number
            ));

            // Return the cleanup function, which is required but can be empty if no cleanup is needed.
            || {}
        },
    );

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
        Callback::from(move |_| {
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            let dispatch = submit_dispatch.clone();
            let post_state = submit_post_state.clone();
            let page_state = submit_state.clone();
            let temp_server_name = call_server_name.clone();
            let temp_api_key = call_api_key.clone();
            let temp_user_id = call_user_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let window = window().expect("no global `window` exists");
                let location = window.location();
                let server_name = location.href().expect("should have a href");
                let server_name = server_name.trim_end_matches('/').to_string();
                let page_state = page_state.clone();
                match login_requests::login_new_server(
                    server_name.clone(),
                    username.to_string(),
                    password.to_string(),
                )
                .await
                {
                    Ok((user_details, login_request, server_details)) => {
                        // After user login, update the image URL with user's email from user_details
                        let gravatar_url = generate_gravatar_url(&user_details.Email, 80); // 80 is the image size
                        let key_copy = login_request.clone();
                        let user_copy = user_details.clone();
                        dispatch.reduce_mut(move |state| {
                            state.user_details = Some(user_details);
                            state.auth_details = Some(login_request);
                            state.server_details = Some(server_details);
                            state.gravatar_url = Some(gravatar_url); // Store the Gravatar URL

                            state.store_app_state();
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
                                                            dispatch.reduce_mut(move |state| {
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
                                            post_state.reduce_mut(|state| {
                                                state.error_message = Option::from(
                                                    "Error Checking MFA Status".to_string(),
                                                )
                                            });
                                        }
                                    }
                                } else {
                                    page_state.set(PageState::TimeZone);
                                }
                            }
                            Err(_) => {
                                post_state.reduce_mut(|state| {
                                    state.error_message = Option::from(
                                        "Error checking first login status".to_string(),
                                    )
                                });
                            }
                        }
                    }
                    Err(_) => {
                        post_state.reduce_mut(|state| {
                            state.error_message =
                                Option::from("Your credentials appear to be incorrect".to_string())
                        });
                        // Handle error
                    }
                }
            });
        })
    };

    let on_key_press = {
        let on_submit = on_submit.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                on_submit.emit(()); // Invoke the existing on_submit logic
            }
        })
    };

    let on_submit_click = {
        let on_submit = on_submit.clone(); // Clone the existing on_submit logic
        Callback::from(move |_: MouseEvent| {
            on_submit.emit(()); // Invoke the existing on_submit logic
        })
    };

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Default,
        CreateUser,
        ForgotPassword,
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

    let on_background_click = {
        let on_close_modal = on_close_modal.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            if element.tag_name() == "DIV" {
                on_close_modal.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

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
        Callback::from(move |e: MouseEvent| {
            let create_state = create_state.clone();
            let window = window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location.href().expect("should have a href");
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
                                        create_state.reduce_mut(|state| {
                                            state.info_message = Some(
                                                "Account created successfully! You can now login."
                                                    .to_string(),
                                            )
                                        });
                                    }
                                }
                                Err(e) => {
                                    // The error message is now user-friendly from our updated call_add_login_user
                                    create_state.reduce_mut(|state| {
                                        state.error_message = Some(e.to_string())
                                    });
                                }
                            }
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        create_state.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Error creating account: {}", formatted_error))
                        });
                    }
                }
            }
        })
    };
    // Define the modal components
    let create_user_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Create New User"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="username" class="block mb-2 text-sm font-medium">{"Username"}</label>
                                <input oninput={on_username_change.clone()} type="text" id="username" name="username" class="search-bar-input border text-sm rounded-lg block w-full p-2.5" required=true />
                                {
                                    match *username_error {
                                        username_error_notice::Hidden => html! {},
                                        username_error_notice::Shown => html! {<p class="text-red-500 text-xs italic">{"Username must be at least 4 characters long"}</p>},
                                    }
                                }
                            </div>
                            <div>
                                <label for="fullname" class="block mb-2 text-sm font-medium">{"Full Name"}</label>
                                <input oninput={on_fullname_change} type="text" id="fullname" name="fullname" class="search-bar-input border text-sm rounded-lg block w-full p-2.5" required=true />
                            </div>
                            <div>
                                <label for="email" class="block mb-2 text-sm font-medium">{"Email"}</label>
                                <input oninput={on_email_change} type="email" id="email" name="email" class="search-bar-input border text-sm rounded-lg block w-full p-2.5" required=true />
                                {
                                    match *email_error {
                                        email_error_notice::Hidden => html! {},
                                        email_error_notice::Shown => html! {<p class="text-red-500 text-xs italic">{"Invalid email address"}</p>},
                                    }
                                }
                            </div>
                            <div>
                                <label for="password" class="block mb-2 text-sm font-medium">{"Password"}</label>
                                <input oninput={on_password_change.clone()} type="password" id="password" name="password" class="search-bar-input border text-sm rounded-lg block w-full p-2.5" required=true />
                                {
                                    match *password_error {
                                        password_error_notice::Hidden => html! {},
                                        password_error_notice::Shown => html! {<p class="text-red-500 text-xs italic">{"Password must be at least 6 characters long"}</p>},
                                    }
                                }
                            </div>
                            <button type="submit" onclick={on_create_submit} class="download-button w-full focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-5 py-2.5 text-center">{"Submit"}</button>
                        </form>
                    </div>
                </div>
            </div>
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
        let dispatch_wasm = dispatch.clone();
        Callback::from(move |e: yew::events::MouseEvent| {
            e.prevent_default();
            let window = window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location.href().expect("should have a href");
            let server_name = server_name.trim_end_matches('/').to_string();
            let dispatch = dispatch_wasm.clone();
            let page_state = page_state.clone();
            page_state.set(PageState::Default);
            let reset_code_request = Some(ResetCodePayload {
                username: forgot_username.clone(),
                email: forgot_email.clone(),
            });

            wasm_bindgen_futures::spawn_local(async move {
                match call_reset_password_create_code(server_name, &reset_code_request.unwrap())
                    .await
                {
                    Ok(success) => {
                        if success {
                            page_state.set(PageState::EnterCode);
                        } else {
                            page_state.set(PageState::Default);
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Option::from(format!("Error Sending Reset Email"))
                            });
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        let formatted_error = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|state| {
                            state.error_message =
                                Option::from(format!("Error sending reset: {:?}", formatted_error))
                        });
                    }
                }
            });
        })
    };

    let forgot_password_modal = html! {
        <div id="forgot-password-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Forgot Password"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <p class="text-m font-semibold">
                            {"Please enter your username and email to reset your password."}
                            </p>
                            <div>
                                <label for="username" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Username"}</label>
                                <input oninput={on_forgot_username_change} type="text" id="username" name="username" class="search-bar-input border text-sm rounded-lg block w-full p-2.5" required=true />
                            </div>
                            <div>
                                <label for="email" class="block mb-2 text-sm font-medium">{"Email"}</label>
                                <input oninput={on_forgot_email_change} type="email" id="email" name="email" class="search-bar-input border text-sm rounded-lg block w-full p-2.5" required=true />
                            </div>
                            <button onclick={on_reset_submit} type="submit" class="download-button w-full focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-5 py-2.5 text-center">{"Submit"}</button>
                        </form>
                    </div>
                </div>
            </div>
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

    let on_reset_code_submit = {
        let page_state = page_state.clone();
        let reset_password = reset_password.clone().to_string();
        let forgot_email = forgot_email.clone().to_string();
        let reset_code = reset_code.clone().to_string();
        let dispatch_wasm = dispatch.clone();
        Callback::from(move |_e: yew::events::MouseEvent| {
            let window = window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location.href().expect("should have a href");
            let server_name = server_name.trim_end_matches('/').to_string();
            let dispatch = dispatch_wasm.clone();
            let page_state = page_state.clone();
            page_state.set(PageState::Default);
            // let forgot__deref = (*forgot_username.clone();
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
                                if success.message == "Password Reset Successfully" {
                                    page_state.set(PageState::Default);
                                } else {
                                    page_state.set(PageState::Default);
                                    dispatch.reduce_mut(|state| {
                                        state.error_message =
                                            Option::from(format!("Error Sending Reset Email"))
                                    });
                                }
                            }
                            Err(e) => {
                                page_state.set(PageState::Default);
                                let formatted_error = format_error_message(&e.to_string());
                                dispatch.reduce_mut(|state| {
                                    state.error_message = Option::from(format!(
                                        "Error Resetting Password: {:?}",
                                        formatted_error
                                    ))
                                });
                            }
                        }
                    });
                }
                Err(e) => {
                    let formatted_error = format_error_message(&e.to_string());
                    dispatch.reduce_mut(|state| {
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
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Password Reset"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <p class="text-m font-semibold">
                            {"An email has been sent to your email address. Please enter a new password and the code contained within the email to reset your password."}
                            </p>
                            <input oninput={on_reset_code_change} type="text" id="reset_code" name="reset_code" class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder="Enter Password Reset Code" />
                            <input oninput={on_reset_password_change} type="text" id="reset_password" name="reset_password" class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder="Enter your new password" />
                            <button type="submit" onclick={on_reset_code_submit} class="download-button w-full focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-5 py-2.5 text-center">{"Submit"}</button>
                        </form>
                    </div>
                </div>
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

    let on_time_zone_submit = {
        // let (state, dispatch) = use_store::<AppState>();
        let page_state = page_state.clone();
        let time_pref = time_pref.clone();
        let time_zone = time_zone.clone();
        let date_format = date_format.clone();
        // let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        // let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        // let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let temp_server_name = temp_server_name.clone();
        let temp_api_key = temp_api_key.clone();
        let temp_user_id = temp_user_id.clone();
        let history = history.clone();
        // let error_message_create = error_message.clone();
        let dispatch_wasm = dispatch.clone();
        Callback::from(move |e: MouseEvent| {
            let dispatch = dispatch_wasm.clone();
            e.prevent_default();
            let server_name = (*temp_server_name).clone();
            let api_key = (*temp_api_key).clone();
            let user_id = *temp_user_id;
            let page_state = page_state.clone();
            let history = history.clone();
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
                                    dispatch.reduce_mut(|state| {
                                        state.error_message =
                                            Option::from("Error Checking MFA Status".to_string())
                                    });
                                }
                            }
                        } else {
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Option::from("Error Setting up Time Zone".to_string())
                            });
                            page_state.set(PageState::Default);
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Option::from(format!("Error setting up time zone"))
                            });
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        let formatted_error = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|state| {
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
                    <h3 class="text-lg">{"Time Zone Setup"}</h3>
                </div>

                // Content
                <div class="modal-body">
                    <form>
                        <div class="modal-welcome">
                            <i class="ph ph-hand-waving text-xl"></i>
                            <p>
                                {"Welcome to Pinepods! This appears to be your first time logging in. To start, let's get some basic information about your time and time zone preferences. This will determine how times appear throughout the app."}
                            </p>
                        </div>

                        <div class="modal-form-group">
                            <label class="modal-label">
                                <i class="ph ph-clock-clockwise"></i>
                                <span>{"Hour Format"}</span>
                            </label>
                            <select
                                id="hour_format"
                                name="hour_format"
                                class="modal-select"
                                oninput={on_time_pref_change}
                            >
                                <option value="12">{"12 Hour"}</option>
                                <option value="24">{"24 Hour"}</option>
                            </select>
                        </div>

                        <div class="modal-form-group">
                            <label class="modal-label">
                                <i class="ph ph-globe"></i>
                                <span>{"Time Zone"}</span>
                            </label>
                            <select
                                id="time_zone"
                                name="time_zone"
                                class="modal-select"
                                oninput={on_tz_change}
                            >
                                { for TZ_VARIANTS.iter().map(|tz| render_time_zone_option(*tz)) }
                            </select>
                        </div>

                        <div class="modal-form-group">
                            <label class="modal-label">
                                <i class="ph ph-calendar"></i>
                                <span>{"Date Format"}</span>
                            </label>
                            <select
                                id="date_format"
                                name="date_format"
                                class="modal-select"
                                oninput={on_df_change}
                            >
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

                        <button
                            type="submit"
                            onclick={on_time_zone_submit}
                            class="modal-button"
                        >
                            <i class="ph ph-check"></i>
                            <span>{"Save Preferences"}</span>
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
        let (state, dispatch) = use_store::<AppState>();
        let page_state = page_state.clone();
        let mfa_code = mfa_code.clone();
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let history = history.clone();
        // let error_message_create = error_message.clone();
        let dispatch_wasm = dispatch.clone();
        Callback::from(move |e: MouseEvent| {
            let dispatch = dispatch_wasm.clone();
            let mfa_code = mfa_code.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let page_state = page_state.clone();
            let history = history.clone();
            // let error_message_clone = error_message_create.clone();
            e.prevent_default();

            wasm_bindgen_futures::spawn_local(async move {
                // let verify_mfa_request = VerifyMFABody {
                //     user_id: user_id,
                //     mfa_code: mfa_code,
                // };
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
                                    }
                                    Err(_e) => {
                                        // console::log_1(&format!("Error getting theme: {:?}", e).into());
                                    }
                                }
                            });
                            let time_server = server_name.clone();
                            let api_server = api_key.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match call_get_time_info(
                                    time_server.unwrap(),
                                    api_server.unwrap().unwrap(),
                                    &user_id.unwrap(),
                                )
                                .await
                                {
                                    Ok(tz_response) => {
                                        dispatch.reduce_mut(move |state| {
                                            state.user_tz = Some(tz_response.timezone);
                                            state.hour_preference = Some(tz_response.hour_pref);
                                            state.date_format = Some(tz_response.date_format);
                                        });
                                    }
                                    Err(e) => {
                                        console::log_1(
                                            &format!("Error getting theme: {:?}", e).into(),
                                        );
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
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Option::from(format!("Error setting up time zone"))
                            });
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        let formatted_error = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|state| {
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
    let first_admin_create_clone = first_admin_created.clone();
    let on_admin_setup = {
        let history = history.clone();
        let first_admin_created = first_admin_create_clone.clone();
        let dispatch_wasm = dispatch.clone();

        Callback::from(move |data: AdminSetupData| {
            let history = history.clone();
            let first_admin_created = first_admin_created.clone();
            let audio_dispatch = dispatch_wasm.clone();

            // Get server name from window location
            let window = web_sys::window().expect("no global `window` exists");
            let location = window.location();
            let server_name = location
                .href()
                .expect("should have a href")
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
                        audio_dispatch.reduce_mut(|state| {
                            state.info_message =
                                Some("Admin account created successfully".to_string());
                        });
                        history.push("/"); // Redirect to login
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        audio_dispatch.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Failed to create admin: {}", formatted_error));
                        });
                    }
                }
            });
        })
    };

    let mfa_code_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700" onclick={stop_propagation.clone()}>
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
                            {"MFA Login"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <p class="text-m font-semibold text-gray-900 dark:text-white">
                            {"Welcome to Pinepods! Please enter your MFA Code Below."}
                            </p>
                            <input oninput={on_mfa_change} type="text" id="mfa_code" name="mfa_code" class="w-full px-3 py-2 text-gray-700 border rounded-lg focus:outline-none" placeholder="Enter MFA Code" />
                            <button type="submit" onclick={on_mfa_submit} class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Submit"}</button>
                        </form>
                    </div>
                </div>
            </div>
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
        <div id="login-page" style={format!("background-image: url('{}'); background-repeat: no-repeat; background-attachment: fixed; background-size: cover;", *background_image_url)}>
        {
            match *page_state {
            PageState::CreateUser => create_user_modal,
            PageState::ForgotPassword => forgot_password_modal,
            PageState::TimeZone => time_zone_setup_modal,
            PageState::MFAPrompt => mfa_code_modal,
            PageState::EnterCode => enter_code_modal,
            _ => html! {},
            }
        }
        {modal_content}

            <div class="flex justify-center items-start pt-[10vh] h-screen">
                <div class="modal-container flex flex-col space-y-4 w-full max-w-xs p-8 border rounded-lg shadow-lg">
                    <div class="flex justify-center items-center">
                        <img class="object-scale-down h-20 w-66" src="static/assets/favicon.png" alt="Pinepods Logo" />
                    </div>
                    <h1 class="item_container-text text-xl font-bold mb-2 text-center">{"Pinepods"}</h1>
                    <p class="item_container-text text-center">{"A Forest of Podcasts, Rooted in the Spirit of Self-Hosting"}</p>
                    <input
                        type="text"
                        placeholder="Username"
                        class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                        oninput={on_login_username_change}
                        onkeypress={on_key_press.clone()}
                    />
                    <input
                        type="password"
                        placeholder="Password"
                        class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                        oninput={on_login_password_change}
                        onkeypress={on_key_press}
                    />
                    // Forgot Password and Create New User buttons
                    <div class="flex justify-between">
                        <button
                            onclick={on_forgot_password}
                            class="login-link text-sm"
                        >
                            {"Forgot Password?"}
                        </button>
                        // <button
                        //     onclick={on_create_new_user}
                        //     class="text-sm text-blue-500 hover:text-blue-700"
                        // >
                        //     {"Create New User"}
                        // </button>
                        {
                            if *self_service_enabled {
                                html! {
                                    <button
                                        onclick={on_create_new_user.clone()}
                                        class="text-sm login-link"
                                    >
                                        {"Create New User"}
                                    </button>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <button
                        onclick={on_submit_click}
                        class="p-2 download-button rounded"
                    >
                        {"Login"}
                    </button>


                    // In your HTML template:
                    {
                        if !oidc_providers.is_empty() {
                            html! {
                                <>
                                    <div class="oidc-divider">
                                        <div class="oidc-divider-line"></div>
                                        <div class="relative flex justify-center">
                                            <span class="oidc-divider-text item_container-text">{"Or continue with"}</span>
                                        </div>
                                    </div>

                                    <div class="flex flex-col space-y-2">
                                        {
                                            oidc_providers.iter().map(|provider| {
                                                let button_style = format!(
                                                    "background-color: {}",
                                                    provider.button_color
                                                );
                                                let button_text_style = format!(
                                                    "color: {}",
                                                    provider.button_text_color
                                                );

                                                html! {
                                                    <button
                                                        class="oidc-button"
                                                        style={button_style}
                                                        onclick={
                                                            let auth_url = provider.authorization_url.clone();
                                                            let client_id = provider.client_id.clone();
                                                            let scope = provider.scope.clone();
                                                            // let server_name = server_name.clone();
                                                            let window = web_sys::window().expect("no global `window` exists");
                                                            let location = window.location();
                                                            let server_name = location
                                                                .href()
                                                                .expect("should have a href")
                                                                .trim_end_matches('/')
                                                                .to_string();

                                                            Callback::from(move |_| {
                                                                let window = web_sys::window().unwrap();
                                                                let crypto = window.crypto().unwrap();
                                                                let mut random_bytes = [0u8; 16];
                                                                crypto.get_random_values_with_u8_array(&mut random_bytes).unwrap();
                                                                let state = random_bytes.iter()
                                                                    .map(|b| format!("{:02x}", b))
                                                                    .collect::<String>();

                                                                let auth_url_clone = auth_url.clone();
                                                                let client_id_clone = client_id.clone();
                                                                let scope_clone = scope.clone();
                                                                let state_clone = state.clone();
                                                                let server_name_clone = server_name.clone();

                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                    match call_store_oidc_state(
                                                                        server_name_clone,
                                                                        state_clone.clone(),
                                                                        client_id_clone.clone(),
                                                                    ).await {
                                                                        Ok(_) => {
                                                                            let origin = window.location().origin().unwrap();
                                                                            let redirect_uri = format!("{}/api/auth/callback", origin);
                                                                            let auth_url = format!(
                                                                                "{}?client_id={}&redirect_uri={}&scope={}&response_type=code&state={}",
                                                                                auth_url_clone, client_id_clone, redirect_uri, scope_clone, state_clone
                                                                            );
                                                                            window.location().set_href(&auth_url).unwrap();
                                                                        },
                                                                        Err(e) => {
                                                                            web_sys::console::log_1(&format!("Failed to store state: {:?}", e).into());
                                                                        }
                                                                    }
                                                                });
                                                            })
                                                        }
                                                    >
                                                        if let Some(icon) = &provider.icon_svg {
                                                            <div class="oidc-icon">
                                                                {Html::from_html_unchecked(AttrValue::from(icon.clone()))}
                                                            </div>
                                                        }
                                                        <span style={button_text_style}>{&provider.button_text}</span>
                                                    </button>
                                                }
                                            }).collect::<Html>()
                                        }
                                    </div>
                                </>
                            }
                        } else {
                            html! {}
                        }
                    }


                </div>
                <ToastNotification />
                // Connect to Different Server button at bottom right
                <div class="fixed bottom-4 right-4">
                    <button
                        onclick={on_different_server}
                        class="p-2 bg-gray-500 text-white rounded hover:bg-gray-600"
                    >
                        {"Connect to Different Server"}
                    </button>
                </div>
            </div>
            </div>
        }
        </>

    }
}

#[function_component(ChangeServer)]
pub fn login() -> Html {
    let (_app_state, _app_dispatch) = use_store::<AppState>();
    let (_state, _dispatch) = use_store::<UIState>();
    let history = BrowserHistory::new();
    let server_name = use_state(|| "".to_string());
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());
    let (_app_state, dispatch) = use_store::<AppState>();
    let time_zone = use_state(|| "".to_string());
    let date_format = use_state(|| "".to_string());
    let time_pref = use_state(|| 12);
    let mfa_code = use_state(|| "".to_string());
    let temp_api_key = use_state(|| "".to_string());
    let temp_user_id = use_state(|| 0);
    let temp_server_name = use_state(|| "".to_string());
    let page_state = use_state(|| PageState::Default);

    // This effect runs only once when the component mounts
    let background_image_url = use_state(|| String::new());
    let effect_background_image = background_image_url.clone();
    // This effect runs only once when the component mounts
    use_effect_with(
        (), // Dependencies, an empty tuple here signifies no dependencies.
        move |_| {
            let background_number = rand::thread_rng().gen_range(1..=9); // Assuming you have images named 1.jpg through 9.jpg.
            effect_background_image.set(format!(
                "static/assets/backgrounds/{}.jpg",
                background_number
            ));

            // Return the cleanup function, which is required but can be empty if no cleanup is needed.
            || {}
        },
    );

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
    let on_submit = {
        let submit_dispatch = dispatch.clone();
        Callback::from(move |_| {
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            let dispatch = submit_dispatch.clone();
            let post_state = submit_post_state.clone();
            let server_name = server_name.clone();
            let page_state = submit_state.clone();
            let temp_server_name = call_server_name.clone();
            let temp_api_key = call_api_key.clone();
            let temp_user_id = call_user_id.clone();
            wasm_bindgen_futures::spawn_local(async move {
                // let server_name = location.href().expect("should have a href");
                let server_name = server_name.clone();
                let page_state = page_state.clone();
                match login_requests::login_new_server(
                    server_name.to_string(),
                    username.to_string(),
                    password.to_string(),
                )
                .await
                {
                    Ok((user_details, login_request, server_details)) => {
                        // After user login, update the image URL with user's email from user_details
                        let gravatar_url = generate_gravatar_url(&user_details.Email, 80); // 80 is the image size
                        let key_copy = login_request.clone();
                        let user_copy = user_details.clone();
                        dispatch.reduce_mut(move |state| {
                            state.user_details = Some(user_details);
                            state.auth_details = Some(login_request);
                            state.server_details = Some(server_details);
                            state.gravatar_url = Some(gravatar_url); // Store the Gravatar URL

                            state.store_app_state();
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
                                                            dispatch.reduce_mut(move |state| {
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
                                            post_state.reduce_mut(|state| {
                                                state.error_message = Option::from(
                                                    "Error Checking MFA Status".to_string(),
                                                )
                                            });
                                        }
                                    }
                                } else {
                                    page_state.set(PageState::TimeZone);
                                }
                            }
                            Err(_) => {
                                post_state.reduce_mut(|state| {
                                    state.error_message = Option::from(
                                        "Error checking first login status".to_string(),
                                    )
                                });
                            }
                        }
                    }
                    Err(_) => {
                        // console::log_1(&format!("Error logging into server: {}", server_name).into());
                        post_state.reduce_mut(|state| {
                            state.error_message =
                                Option::from("Your credentials appear to be incorrect".to_string())
                        });
                        // Handle error
                    }
                }
            });
        })
    };
    let on_submit_click = {
        let on_submit = on_submit.clone(); // Clone the existing on_submit logic
        Callback::from(move |_: MouseEvent| {
            on_submit.emit(()); // Invoke the existing on_submit logic
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
    let handle_key_press = {
        let on_submit = on_submit.clone(); // Clone the on_submit callback
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                on_submit.emit(());
            }
        })
    };
    // Define the callback function for closing the modal
    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Default);
        })
    };

    let on_background_click = {
        let on_close_modal = on_close_modal.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            if element.tag_name() == "DIV" {
                on_close_modal.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

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
    let time_state_error = _app_dispatch.clone();
    let on_time_pref_change = {
        let time_pref = time_pref.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
            let value_str = select_element.value();
            if let Ok(value_int) = value_str.parse::<i32>() {
                time_pref.set(value_int);
            } else {
                time_state_error.reduce_mut(|state| {
                    state.error_message = Option::from("Error parsing time preference".to_string())
                });
            }
        })
    };
    let dispatch_time = _app_dispatch.clone();
    let on_time_zone_submit = {
        // let (state, dispatch) = use_store::<AppState>();
        let page_state = page_state.clone();
        let time_pref = time_pref.clone();
        let time_zone = time_zone.clone();
        let date_format = date_format.clone();
        // let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        // let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        // let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let temp_server_name = temp_server_name.clone();
        let temp_api_key = temp_api_key.clone();
        let temp_user_id = temp_user_id.clone();
        let history = history.clone();
        // let error_message_create = error_message.clone();
        Callback::from(move |e: MouseEvent| {
            let post_state = dispatch_time.clone();
            e.prevent_default();
            let server_name = (*temp_server_name).clone();
            let api_key = (*temp_api_key).clone();
            let user_id = *temp_user_id;
            let page_state = page_state.clone();
            let history = history.clone();
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
                                    post_state.reduce_mut(|state| {
                                        state.error_message =
                                            Option::from("Error Checking MFA Status".to_string())
                                    });
                                }
                            }
                        } else {
                            post_state.reduce_mut(|state| {
                                state.error_message =
                                    Option::from("Error Setting up Time Zone".to_string())
                            });
                            page_state.set(PageState::Default);
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        // dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error setting up time zone: {:?}", e)));
                        let formatted_error = format_error_message(&e.to_string());
                        post_state.reduce_mut(|state| {
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
                    <h3 class="text-lg">{"Time Zone Setup"}</h3>
                </div>

                // Content
                <div class="modal-body">
                    <form>
                        <div class="modal-welcome">
                            <i class="ph ph-hand-waving text-xl"></i>
                            <p>
                                {"Welcome to Pinepods! This appears to be your first time logging in. To start, let's get some basic information about your time and time zone preferences. This will determine how times appear throughout the app."}
                            </p>
                        </div>

                        <div class="modal-form-group">
                            <label class="modal-label">
                                <i class="ph ph-clock-clockwise"></i>
                                <span>{"Hour Format"}</span>
                            </label>
                            <select
                                id="hour_format"
                                name="hour_format"
                                class="modal-select"
                                oninput={on_time_pref_change}
                            >
                                <option value="12">{"12 Hour"}</option>
                                <option value="24">{"24 Hour"}</option>
                            </select>
                        </div>

                        <div class="modal-form-group">
                            <label class="modal-label">
                                <i class="ph ph-globe"></i>
                                <span>{"Time Zone"}</span>
                            </label>
                            <select
                                id="time_zone"
                                name="time_zone"
                                class="modal-select"
                                oninput={on_tz_change}
                            >
                                { for TZ_VARIANTS.iter().map(|tz| render_time_zone_option(*tz)) }
                            </select>
                        </div>

                        <div class="modal-form-group">
                            <label class="modal-label">
                                <i class="ph ph-calendar"></i>
                                <span>{"Date Format"}</span>
                            </label>
                            <select
                                id="date_format"
                                name="date_format"
                                class="modal-select"
                                oninput={on_df_change}
                            >
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

                        <button
                            type="submit"
                            onclick={on_time_zone_submit}
                            class="modal-button"
                        >
                            <i class="ph ph-check"></i>
                            <span>{"Save Preferences"}</span>
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
        Callback::from(move |e: MouseEvent| {
            let dispatch = dispatch_wasm.clone();
            let mfa_code = mfa_code.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let page_state = page_state.clone();
            let history = history.clone();
            let post_state = post_state.clone();
            // let error_message_clone = error_message_create.clone();
            e.prevent_default();

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
                                        dispatch.reduce_mut(move |state| {
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
                            post_state.reduce_mut(|state| {
                                state.error_message =
                                    Option::from(format!("Error validating MFA Code"))
                            });
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Default);
                        let formatted_error = format_error_message(&e.to_string());
                        post_state.reduce_mut(|state| {
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

    let mfa_code_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700" onclick={stop_propagation.clone()}>
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
                            {"MFA Login"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <p class="text-m font-semibold text-gray-900 dark:text-white">
                            {"Welcome to Pinepods! Please enter your MFA Code Below."}
                            </p>
                            <input oninput={on_mfa_change} type="text" id="mfa_code" name="mfa_code" class="w-full px-3 py-2 text-gray-700 border rounded-lg focus:outline-none" placeholder="Enter MFA Code" />
                            <button type="submit" onclick={on_mfa_submit} class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Submit"}</button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    html! {
        <>
        <div id="login-page" style={format!("background-image: url('{}'); background-repeat: no-repeat; background-attachment: fixed; background-size: cover;", *background_image_url)}>
        {
            match *page_state {
            PageState::TimeZone => time_zone_setup_modal,
            PageState::MFAPrompt => mfa_code_modal,
            _ => html! {},
            }
        }
        <div class="flex justify-center items-center h-screen">
            <div class="modal-container flex flex-col space-y-4 w-full max-w-xs p-8 border rounded-lg shadow-lg">
                <div class="flex justify-center items-center">
                    <img class="object-scale-down h-20 w-66" src="static/assets/favicon.png" alt="Pinepods Logo" />
                </div>
                <h1 class="item_container-text text-xl font-bold mb-2 text-center">{"Pinepods"}</h1>
                <p class="item_container-text text-center">{"A Forest of Podcasts, Rooted in the Spirit of Self-Hosting"}</p>
                <input
                    type="text"
                    placeholder="Server Name"
                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                    oninput={on_server_name_change}
                    onkeypress={handle_key_press.clone()}
                />
                <input
                    type="text"
                    placeholder="Username"
                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                    oninput={on_username_change}
                    onkeypress={handle_key_press.clone()}
                />
                <input
                    type="password"
                    placeholder="Password"
                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                    oninput={on_password_change}
                    onkeypress={handle_key_press.clone()}
                />
                <button onclick={on_submit_click} class="p-2 download-button rounded">
                    {"Login"}
                </button>
            </div>
            <ToastNotification />

            // Connect to Different Server button at bottom right
            <div class="fixed bottom-4 right-4">
                <button onclick={on_different_server} class="p-2 bg-gray-500 text-white rounded hover:bg-gray-600">
                    {"Connect to Local Server"}
                </button>
            </div>
        </div>
        </div>
        </>
    }
}

#[function_component(LogOut)]
pub fn logout() -> Html {
    let history = BrowserHistory::new();

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

    // Redirect to root path
    history.push("/");

    html! {}
}
