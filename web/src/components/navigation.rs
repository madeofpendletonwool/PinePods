// navigation.rs
use crate::components::context::AppState;
use crate::components::gen_funcs::generate_gravatar_url;
use crate::requests::login_requests::{
    call_get_time_info, call_verify_key, use_check_authentication,
};
use crate::requests::setting_reqs::call_get_theme;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::window;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[derive(Properties, PartialEq)]
pub struct NavigationHandlerProps {
    pub children: Children,
}

#[function_component(NavigationHandler)]
pub fn navigation_handler(props: &NavigationHandlerProps) -> Html {
    let navigation_state = use_state(|| 0);
    let (state, dispatch) = use_store::<AppState>();
    let loading = use_state(|| true);

    // Handle initial auth check and auto-login
    {
        let dispatch = dispatch.clone();
        let state = state.clone();
        let effect_loading = loading.clone();

        use_effect_with((), move |_| {
            if let Some(window) = web_sys::window() {
                let current_route = window.location().href().unwrap_or_default();

                // First check local storage for saved state
                if let Ok(Some(local_storage)) = window.local_storage() {
                    if let Ok(Some(user_state)) = local_storage.get_item("userState") {
                        let app_state_result = AppState::deserialize(&user_state);

                        if let Ok(Some(auth_state)) = local_storage.get_item("userAuthState") {
                            if let Ok(auth_details) = AppState::deserialize(&auth_state) {
                                if let Ok(Some(server_state)) =
                                    local_storage.get_item("serverState")
                                {
                                    let server_details_result =
                                        AppState::deserialize(&server_state);

                                    if let (Ok(app_state), Ok(server_details)) =
                                        (app_state_result, server_details_result)
                                    {
                                        if app_state.user_details.is_some()
                                            && auth_details.auth_details.is_some()
                                            && server_details.server_details.is_some()
                                        {
                                            let auth_state_clone = auth_details.clone();
                                            if let Some(user_details) =
                                                app_state.user_details.clone()
                                            {
                                                let email = user_details.Email.clone();
                                                let user_id = user_details.UserID;

                                                if let Some(auth_details) =
                                                    auth_state_clone.auth_details
                                                {
                                                    let server_name =
                                                        auth_details.server_name.clone();
                                                    let api_key = auth_details
                                                        .api_key
                                                        .clone()
                                                        .unwrap_or_default();

                                                    let dispatch_clone = dispatch.clone();
                                                    let app_state = app_state.clone();
                                                    let server_details = server_details.clone();
                                                    let auth_details = auth_details.clone();

                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        match call_verify_key(
                                                            &server_name,
                                                            &api_key,
                                                        )
                                                        .await
                                                        {
                                                            Ok(_) => {
                                                                let gravatar_url =
                                                                    generate_gravatar_url(
                                                                        &email.clone(),
                                                                        80,
                                                                    );

                                                                // Update app state
                                                                dispatch_clone.reduce_mut(
                                                                    move |state| {
                                                                        state.user_details =
                                                                            app_state.user_details;
                                                                        state.auth_details = Some(
                                                                            auth_details.clone(),
                                                                        );
                                                                        state.server_details =
                                                                            server_details
                                                                                .server_details;
                                                                        state.gravatar_url =
                                                                            Some(gravatar_url);
                                                                    },
                                                                );

                                                                if let Some(window) =
                                                                    web_sys::window()
                                                                {
                                                                    // Set session storage
                                                                    if let Ok(Some(
                                                                        session_storage,
                                                                    )) = window.session_storage()
                                                                    {
                                                                        session_storage
                                                                            .set_item(
                                                                                "isAuthenticated",
                                                                                "true",
                                                                            )
                                                                            .ok();
                                                                    }

                                                                    // Get theme
                                                                    if let Ok(Some(local_storage)) =
                                                                        window.local_storage()
                                                                    {
                                                                        if let Ok(theme) =
                                                                            call_get_theme(
                                                                                server_name.clone(),
                                                                                api_key.clone(),
                                                                                &user_id,
                                                                            )
                                                                            .await
                                                                        {
                                                                            crate::components::setting_components::theme_options::changeTheme(&theme);
                                                                            local_storage
                                                                                .set_item(
                                                                                    "selected_theme",
                                                                                    &theme,
                                                                                )
                                                                                .ok();
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
                                                                    dispatch_clone.reduce_mut(move |state| {
                                                                        state.user_tz = Some(tz_response.timezone);
                                                                        state.hour_preference = Some(tz_response.hour_pref);
                                                                        state.date_format = Some(tz_response.date_format);
                                                                    });
                                                                }
                                                            }
                                                            Err(_) => {
                                                                // Failed to verify key, redirect to login
                                                                let history = BrowserHistory::new();
                                                                history.push("/");
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Check for page reload
                if !state.reload_occured.unwrap_or(false) {
                    let performance = window.performance().expect("should have performance");
                    let navigation_type = performance.navigation().type_();

                    if navigation_type == 1 {
                        if let Ok(Some(session_storage)) = window.session_storage() {
                            session_storage.set_item("isAuthenticated", "false").ok();
                        }
                    }

                    use_check_authentication(dispatch.clone(), &current_route);

                    dispatch.reduce_mut(|state| {
                        state.reload_occured = Some(true);
                        state.clone()
                    });
                }
            }
            effect_loading.set(false);
            || ()
        });
    }

    // Handle browser navigation events
    {
        let navigation_state = navigation_state.clone();
        use_effect_with((), move |_| {
            let window = window().unwrap();
            let window_clone = window.clone();

            let onpopstate = Closure::wrap(Box::new(move |_: web_sys::Event| {
                let path = window_clone.location().pathname().unwrap_or_default();
                let search = window_clone.location().search().unwrap_or_default();

                if let Ok(Some(session_storage)) = window_clone.session_storage() {
                    // Check if we're already handling a double-back
                    let is_double_back = session_storage
                        .get_item("handling_double_back")
                        .unwrap_or(None);

                    if is_double_back == Some("true".to_string()) {
                        // Clear the flag and don't do another back
                        session_storage.remove_item("handling_double_back").ok();
                    } else {
                    }
                }
                navigation_state.set(*navigation_state + 1);
            }) as Box<dyn FnMut(_)>);

            window
                .add_event_listener_with_callback("popstate", onpopstate.as_ref().unchecked_ref())
                .unwrap();

            move || {
                window
                    .remove_event_listener_with_callback(
                        "popstate",
                        onpopstate.as_ref().unchecked_ref(),
                    )
                    .unwrap();
                onpopstate.forget();
            }
        });
    }

    html! {
        if *loading {
            <div class="loading-animation">
                <div class="frame1"></div>
                <div class="frame2"></div>
                <div class="frame3"></div>
                <div class="frame4"></div>
                <div class="frame5"></div>
                <div class="frame6"></div>
            </div>
        } else {
            <>
                { props.children.clone() }
            </>
        }
    }
}

#[hook]
pub fn use_back_button() -> Callback<MouseEvent> {
    Callback::from(move |e: MouseEvent| {
        e.prevent_default();
        if let Some(window) = web_sys::window() {
            // First go back in history
            let history = BrowserHistory::new();
            history.back();

            // Then create and dispatch a PopStateEvent to ensure our handler runs
            let event = web_sys::PopStateEvent::new("popstate").unwrap();
            window.dispatch_event(&event.into()).unwrap();
        }
    })
}
