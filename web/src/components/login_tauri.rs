use crate::components::context::{AppState, UIState};
use crate::components::gen_funcs::format_error_message;
use crate::components::notification_center::ToastNotification;
use crate::requests::login_requests::{self, call_check_mfa_enabled};
use crate::requests::login_requests::{
    call_first_login_done, call_get_time_info, call_self_service_login_status,
    call_setup_timezone_info, call_verify_key, call_verify_mfa, TimeZoneInfo,
};
use crate::requests::setting_reqs::call_get_theme;
use chrono_tz::{Tz, TZ_VARIANTS};
use md5;
use rand::Rng;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{console, window};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

// Gravatar URL generation functions (outside of use_effect_with)
fn calculate_gravatar_hash(email: &String) -> String {
    format!("{:x}", md5::compute(email.to_lowercase()))
}

fn generate_gravatar_url(email: &Option<String>, size: usize) -> String {
    let hash = calculate_gravatar_hash(&email.clone().unwrap());
    format!("https://gravatar.com/avatar/{}?s={}", hash, size)
}

#[function_component(Login)]
pub fn login() -> Html {
    let history = BrowserHistory::new();
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());
    let (app_state, dispatch) = use_store::<AppState>();
    let (_state, _dispatch) = use_store::<UIState>();
    let _error_message = app_state.error_message.clone();
    let error_message = app_state.error_message.clone();
    let time_zone = use_state(|| "".to_string());
    let date_format = use_state(|| "".to_string());
    let time_pref = use_state(|| 12);
    let mfa_code = use_state(|| "".to_string());
    let server_name = use_state(|| "".to_string());
    let temp_api_key = use_state(|| "".to_string());
    let temp_user_id = use_state(|| 0);
    let temp_server_name = use_state(|| "".to_string());
    let info_message = app_state.info_message.clone();
    // Define the initial state
    // Define states for both self-service and first admin
    let page_state = use_state(|| PageState::Default);
    let self_service_enabled = use_state(|| false);
    let first_admin_created = use_state(|| false);

    // Clone states for the effect
    let effect_self_service = self_service_enabled.clone();
    let effect_first_admin = first_admin_created.clone();

    use_effect_with((), move |_| {
        let self_service_enabled = effect_self_service.clone();
        let first_admin_created = effect_first_admin.clone();

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
                Err(_e) => {
                    // Handle error if needed
                }
            }
        });

        || ()
    });

    let effect_displatch = dispatch.clone();
    // User Auto Login with saved state
    use_effect_with((), {
        // let error_clone_use = error_message_clone.clone();
        let history = history.clone();
        move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(local_storage) = window.local_storage() {
                    if let Some(storage) = local_storage {
                        if let Ok(Some(stored_theme)) = storage.get_item("selected_theme") {
                            // Convert 'nordic' to 'Nordic' if needed
                            let theme_to_use = if stored_theme == "nordic" {
                                String::from("Nordic")
                            } else {
                                stored_theme
                            };

                            // Set the theme using your existing theme change function
                            crate::components::setting_components::theme_options::changeTheme(
                                &theme_to_use,
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
                                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                            match call_get_time_info(server_name, api_key, &wasm_user_id).await{
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
                                                                            let redirect_route = requested_route.unwrap_or_else(|| "/home".to_string());
                                                                            history.push(
                                                                                &redirect_route,
                                                                            ); // Redirect to the requested or home page
                                                                        }
                                                                        Err(_) => {
                                                                            // API key is not valid, redirect to login
                                                                            history.push("/");
                                                                        }
                                                                    }
                                                                },
                                                            );
                                                        } else {
                                                            console::log_1(
                                                                &"Auth details are None".into(),
                                                            );
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
                                    }
                                }
                            }
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
    let submit_post_state = dispatch.clone();
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
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    match call_get_time_info(
                                                        server_name,
                                                        api_key.unwrap(),
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
                                                history.push("/home"); // Use the route path
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

    let offline_hist = history.clone();
    let offline_post_state = _dispatch.clone();
    let on_offline = {
        Callback::from(move |_| {
            let history = offline_hist.clone();
            offline_post_state.reduce_mut(|state| state.app_offline_mode = Option::from(true));
            history.push("/local_downloads");
        })
    };

    let on_offline_mode = {
        let on_offline = on_offline.clone(); // Clone the existing on_submit logic
        Callback::from(move |_: MouseEvent| {
            on_offline.emit(()); // Invoke the existing on_submit logic
        })
    };

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Default,
        TimeZone,
        MFAPrompt,
    }

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
    let time_state_error = dispatch.clone();
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
    let dispatch_time = dispatch.clone();
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
                                        history.push("/home"); // Use the route path
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
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow">
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold">
                            {"Time Zone Setup"}
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
                            {"Welcome to Pinepods! This appears to be your first time logging in. To start, let's get some basic information about your time and time zone preferences. This will determine how times appear throughout the app."}
                            </p>
                            <div>
                                <label for="hour_format" style="margin-right: 10px;">{"Hour Format:"}</label>
                                <select id="hour_format" name="hour_format" class="email-select border p-2 rounded" oninput={on_time_pref_change}>
                                    <option value="12">{"12 Hour"}</option>
                                    <option value="24">{"24 Hour"}</option>
                                </select>
                            </div>
                            <div>
                                <label for="time_zone" style="margin-right: 10px;">{"Time Zone:"}</label>
                                <select id="time_zone" name="time_zone" class="email-select border p-2 rounded" oninput={on_tz_change}>
                                    { for TZ_VARIANTS.iter().map(|tz| render_time_zone_option(*tz)) }
                                </select>
                            </div>
                            <div>
                            <label for="date_format" style="margin-right: 10px;">{"Date Format:"}</label>
                            <select id="date_format" name="date_format" class="email-select border p-2 rounded" oninput={on_df_change}>
                                <option value="MDY">{"MDY (MM-DD-YYYY)"}</option>
                                <option value="DMY">{"DMY (DD-MM-YYYY)"}</option>
                                <option value="YMD">{"YMD (YYYY-MM-DD)"}</option>
                                <option value="JUL">{"JUL (YY/DDD)"}</option>
                                <option value="ISO">{"ISO (YYYY-MM-DD)"}</option>
                                <option value="USA">{"USA (MM/DD/YYYY)"}</option>
                                <option value="EUR">{"EUR (DD.MM.YYYY)"}</option>
                                <option value="JIS">{"JIS (YYYY-MM-DD)"}</option>
                            </select>
                        </div>
                            <button type="submit" onclick={on_time_zone_submit} class="download-button w-full focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-5 py-2.5 text-center">{"Submit"}</button>
                        </form>
                    </div>
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
    let post_state = dispatch.clone();
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
                            wasm_bindgen_futures::spawn_local(async move {
                                match call_get_time_info(
                                    server_name.unwrap(),
                                    api_key.unwrap().unwrap(),
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
                            history.push("/home"); // Use the route path
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
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700">
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
                <button onclick={on_offline_mode} class="p-2 download-button rounded">
                    {"Offline Mode"}
                </button>
            </div>
            <ToastNotification />
        </div>
        </div>
        </>
    }
}

#[function_component(ChangeServer)]
pub fn login() -> Html {
    html! {
        <p>{"This route isn't used on the client version"}</p>
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
