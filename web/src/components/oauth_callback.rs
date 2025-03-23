use crate::components::context::AppState;
use crate::requests::login_requests::{
    call_first_login_done, call_get_api_config, call_get_time_info, call_get_user_details,
    call_get_user_id, call_setup_timezone_info, call_verify_key, LoginServerRequest, TimeZoneInfo,
};
use crate::requests::setting_reqs::call_get_theme;
use chrono_tz::{Tz, TZ_VARIANTS};
use gloo::utils::window;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlSelectElement;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[derive(PartialEq, Clone)]
enum PageState {
    Loading,
    Error(String),
    TimeZone,
    Success,
}

// Gravatar URL generation functions (outside of use_effect_with)
fn calculate_gravatar_hash(email: &String) -> String {
    format!("{:x}", md5::compute(email.to_lowercase()))
}

pub fn generate_gravatar_url(email: &Option<String>, size: usize) -> String {
    let hash = calculate_gravatar_hash(&email.clone().unwrap());
    format!("https://gravatar.com/avatar/{}?s={}", hash, size)
}

#[function_component(OAuthCallback)]
pub fn oauth_callback() -> Html {
    let history = BrowserHistory::new();
    let (_, dispatch) = use_store::<AppState>();
    let page_state = use_state(|| PageState::Loading);

    let time_zone = use_state(|| "UTC".to_string());
    let date_format = use_state(|| "ISO".to_string());
    // Store as i32 to match TimeZoneInfo's requirements
    let time_pref = use_state(|| 24_i32);
    web_sys::console::log_1(&"part of oauth".into());

    // Timezone change handler
    let on_tz_change = {
        let tz = time_zone.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<HtmlSelectElement>();
            tz.set(select_element.value());
        })
    };

    // Date format change handler
    let on_df_change = {
        let df = date_format.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<HtmlSelectElement>();
            df.set(select_element.value());
        })
    };

    // Time preference change handler
    let on_time_pref_change = {
        let time_pref = time_pref.clone();
        Callback::from(move |e: InputEvent| {
            let select_element = e.target_unchecked_into::<HtmlSelectElement>();
            if let Ok(value) = select_element.value().parse::<i32>() {
                time_pref.set(value);
            }
        })
    };

    // Timezone submit handler
    let on_time_zone_submit = {
        let page_state = page_state.clone();
        let time_pref = time_pref.clone();
        let time_zone = time_zone.clone();
        let date_format = date_format.clone();
        let history = history.clone();
        let dispatch = dispatch.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            web_sys::console::log_1(&"Time zone submit clicked".into());
            let call_dispatch = dispatch.clone();
            let call_history = history.clone();
            let call_page_state = page_state.clone();
            let window = window();
            let location = window.location();
            let server_name = location.origin().expect("should have origin");
            let search = location.search().unwrap_or_default();
            let params = web_sys::UrlSearchParams::new_with_str(&search).unwrap();

            if let Some(api_key) = params.get("api_key") {
                web_sys::console::log_1(&"Got API key".into());

                let timezone = time_zone.clone();
                let time_p = time_pref.clone();
                let date_f = date_format.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_get_user_id(&server_name, &api_key).await {
                        Ok(user_id_response) => {
                            if let Some(user_id) = user_id_response.retrieved_id {
                                let timezone_info = TimeZoneInfo {
                                    user_id,
                                    timezone: (*timezone).clone(),
                                    hour_pref: *time_p, // Already i16
                                    date_format: (*date_f).clone(),
                                };

                                web_sys::console::log_1(
                                    &format!("Submitting timezone info: {:?}", timezone_info)
                                        .into(),
                                );

                                match call_setup_timezone_info(
                                    server_name.clone(),
                                    api_key.clone(),
                                    timezone_info.clone(), // Clone for use in dispatch
                                )
                                .await
                                {
                                    Ok(success) => {
                                        if success.success {
                                            call_dispatch.reduce_mut(move |state| {
                                                state.user_tz = Some(timezone_info.timezone);
                                                state.hour_preference = Some(
                                                    timezone_info.hour_pref.try_into().unwrap(),
                                                );
                                                state.date_format = Some(timezone_info.date_format);
                                            });

                                            call_history.push("/home");
                                        } else {
                                            call_page_state.set(PageState::Error(
                                                "Failed to set timezone".into(),
                                            ));
                                        }
                                    }
                                    Err(_) => {
                                        call_page_state
                                            .set(PageState::Error("Error setting timezone".into()));
                                    }
                                }
                            } else {
                                call_page_state
                                    .set(PageState::Error("Could not get user ID".into()));
                            }
                        }
                        Err(_) => {
                            call_page_state.set(PageState::Error("Failed to get user ID".into()));
                        }
                    }
                });
            } else {
                call_page_state.set(PageState::Error("API key not found".into()));
            }
        })
    };

    // Initial API key and login setup
    {
        let page_state = page_state.clone();
        let dispatch = dispatch.clone();
        let history = history.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                let window = window();
                let search = window.location().search().unwrap_or_default();
                let params = web_sys::UrlSearchParams::new_with_str(&search).unwrap();

                if let Some(api_key) = params.get("api_key") {
                    let location = window.location();
                    let server_name = location.origin().expect("should have origin");
                    web_sys::console::log_1(
                        &format!("Starting verification process with API key: {}", api_key).into(),
                    );

                    // Standard login flow...
                    match call_verify_key(&server_name, &api_key).await {
                        Ok(_) => {
                            web_sys::console::log_1(&format!("Verified key successfully").into());
                            match call_get_user_id(&server_name, &api_key).await {
                                Ok(user_id_response) => {
                                    web_sys::console::log_1(&"got user".into());
                                    if let Some(user_id) = user_id_response.retrieved_id {
                                        match call_get_user_details(
                                            &server_name,
                                            &api_key,
                                            &user_id,
                                        )
                                        .await
                                        {
                                            Ok(user_details) => {
                                                let gravatar_url =
                                                    generate_gravatar_url(&user_details.Email, 80);

                                                // Get server details
                                                match call_get_api_config(&server_name, &api_key)
                                                    .await
                                                {
                                                    Ok(server_details) => {
                                                        let auth_details = LoginServerRequest {
                                                            server_name: server_name.clone(),
                                                            username: None,
                                                            password: None,
                                                            api_key: Some(api_key.to_string()),
                                                        };

                                                        dispatch.reduce_mut(move |state| {
                                                            state.user_details =
                                                                Some(user_details.clone());
                                                            state.auth_details = Some(auth_details);
                                                            state.server_details =
                                                                Some(server_details); // Store server details
                                                            state.gravatar_url = Some(gravatar_url);
                                                            state.store_app_state();
                                                        });

                                                        // Rest of the flow...
                                                        //
                                                        match call_first_login_done(
                                                            server_name.clone(),
                                                            api_key.clone(),
                                                            &user_id,
                                                        )
                                                        .await
                                                        {
                                                            Ok(first_login_done) => {
                                                                if !first_login_done {
                                                                    page_state
                                                                        .set(PageState::TimeZone);
                                                                    return;
                                                                }

                                                                // Regular login flow - get preferences and redirect
                                                                spawn_local(async move {
                                                                    if let Ok(theme) =
                                                                        call_get_theme(
                                                                            server_name.clone(),
                                                                            api_key.clone(),
                                                                            &user_id,
                                                                        )
                                                                        .await
                                                                    {
                                                                        crate::components::setting_components::theme_options::changeTheme(&theme);
                                                                        if let Some(window) =
                                                                            web_sys::window()
                                                                        {
                                                                            if let Ok(Some(
                                                                                local_storage,
                                                                            )) = window
                                                                                .local_storage()
                                                                            {
                                                                                let _ = local_storage
                                                                                    .set_item(
                                                                                        "selected_theme",
                                                                                        &theme,
                                                                                    );
                                                                            }
                                                                        }
                                                                    }

                                                                    if let Ok(tz_response) =
                                                                        call_get_time_info(
                                                                            server_name,
                                                                            api_key,
                                                                            &user_id,
                                                                        )
                                                                        .await
                                                                    {
                                                                        dispatch.reduce_mut(move |state| {
                                                                            state.user_tz =
                                                                                Some(tz_response.timezone);
                                                                            state.hour_preference =
                                                                                Some(tz_response.hour_pref);
                                                                            state.date_format = Some(
                                                                                tz_response.date_format,
                                                                            );
                                                                        });
                                                                    }

                                                                    history.push("/home");
                                                                });
                                                            }
                                                            Err(_) => {
                                                                page_state.set(PageState::Error(
                                                                    "Error checking first login status"
                                                                        .into(),
                                                                ));
                                                            }
                                                        }
                                                    }
                                                    Err(_) => {
                                                        page_state.set(PageState::Error(
                                                            "Failed to get server configuration"
                                                                .into(),
                                                        ));
                                                        return;
                                                    }
                                                }
                                            }
                                            Err(_) => {
                                                page_state.set(PageState::Error(
                                                    "Failed to get user details".into(),
                                                ));
                                            }
                                        }
                                    }
                                }
                                Err(_) => {
                                    page_state
                                        .set(PageState::Error("Failed to get user ID".into()));
                                }
                            }
                        }
                        Err(_) => {
                            page_state.set(PageState::Error("Invalid API key".into()));
                        }
                    }
                } else if let Some(err) = params.get("error") {
                    let error_message = match err.as_str() {
                        "username_conflict" => "Unable to create account - username already exists",
                        "authentication_failed" => "Authentication failed. Please try again.",
                        "invalid_provider" => "Invalid authentication provider.",
                        _ => "An unexpected error occurred during login.",
                    };
                    page_state.set(PageState::Error(error_message.into()));
                } else {
                    page_state.set(PageState::Error(
                        "No authentication information received.".into(),
                    ));
                }
            });
            || ()
        });
    }

    fn render_time_zone_option(tz: Tz) -> Html {
        html! {
            <option value={tz.name()}>{tz.name()}</option>
        }
    }

    match (*page_state).clone() {
        PageState::Loading => html! {
            <div class="loading-container">
                <div class="loading-spinner"></div>
                <p>{"Processing login..."}</p>
            </div>
        },
        PageState::Error(msg) => html! {
            <ErrorDisplay message={msg} />
        },
        PageState::TimeZone => html! {
            <div class="modal-overlay">
                <div class="item_container-text modal-content">
                    <div class="item_container-text modal-header">
                        <i class="ph ph-clock text-xl"></i>
                        <h3 class="text-lg">{"Time Zone Setup"}</h3>
                    </div>

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
        },
        PageState::Success => html! {
            <div class="loading-container">
                <div class="loading-spinner"></div>
                <p>{"Redirecting..."}</p>
            </div>
        },
    }
}

#[derive(Properties, PartialEq)]
struct ErrorDisplayProps {
    message: String,
}

#[function_component(ErrorDisplay)]
fn error_display(props: &ErrorDisplayProps) -> Html {
    html! {
        <div class="auth-error-container">
            <div class="auth-error-message">
                <span class="material-icons">{"error"}</span>
                <p>{&props.message}</p>
            </div>
            <a href="/" class="auth-error-button">
                {"Back to Login"}
            </a>
        </div>
    }
}
