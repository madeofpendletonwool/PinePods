use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::pod_req::connect_to_episode_websocket;
use crate::requests::setting_reqs::{
    call_add_gpodder_server, call_add_nextcloud_server, call_check_nextcloud_server,
    call_get_nextcloud_server, call_remove_podcast_sync, call_verify_gpodder_auth,
    initiate_nextcloud_login, GpodderAuthRequest, GpodderCheckRequest, NextcloudAuthRequest,
};
use serde::Deserialize;
use serde::Serialize;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::use_store;
// use wasm_timer;

// Assume this struct is for handling the response of the initial login request
#[derive(Serialize, Deserialize)]
pub struct NextcloudLoginResponse {
    pub poll: Poll,
    pub login: String,
}

#[derive(Serialize, Deserialize)]
pub struct Poll {
    pub token: String,
    pub endpoint: String,
}

async fn open_nextcloud_login(url: &str) -> Result<(), JsValue> {
    let window = web_sys::window().expect("no global `window` exists");
    window.open_with_url_and_target(url, "_blank")?;
    Ok(())
}

#[function_component(NextcloudOptions)]
pub fn nextcloud_options() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let server_url = use_state(|| String::new());
    let server_user = use_state(|| String::new());
    let server_pass = use_state(|| String::new());
    let auth_status = use_state(|| String::new());
    let nextcloud_url = use_state(|| String::new()); // State to hold the Nextcloud server URL
    let _error_message = state.error_message.clone();
    let _info_message = state.info_message.clone();

    let is_sync_configured = use_state(|| false); // Track if sync is configured
    let is_loading = use_state(|| false); // Track loading state for remove button

    // Handler for server URL input change
    let on_server_url_change = {
        let server_url = server_url.clone();
        Callback::from(move |e: InputEvent| {
            // Cast the event target to HtmlInputElement to access the value
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                server_url.set(input.value());
            }
        })
    };
    let on_username_change = {
        let server_user = server_user.clone();
        Callback::from(move |e: InputEvent| {
            // Cast the event target to HtmlInputElement to access the value
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                server_user.set(input.value());
            }
        })
    };
    let on_password_change = {
        let server_pass = server_pass.clone();
        Callback::from(move |e: InputEvent| {
            // Cast the event target to HtmlInputElement to access the value
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                server_pass.set(input.value());
            }
        })
    };

    {
        let nextcloud_url = nextcloud_url.clone();
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        let configured_effect = is_sync_configured.clone();

        use_effect_with(&(), move |_| {
            let nextcloud_url = nextcloud_url.clone();
            let user_id = user_id.clone().unwrap_or_default(); // Make sure user_id is available

            wasm_bindgen_futures::spawn_local(async move {
                match call_get_nextcloud_server(
                    &server_name.clone().unwrap(),
                    &api_key.clone().unwrap().unwrap(),
                    user_id,
                )
                .await
                {
                    Ok(server) => {
                        if !server.is_empty()
                            && server != "Not currently syncing with Nextcloud server"
                            && server != "Not currently syncing with any server"
                        {
                            nextcloud_url.set(server);
                            configured_effect.set(true);
                        } else {
                            nextcloud_url
                                .set(String::from("Not currently syncing with any server"));
                            configured_effect.set(false);
                        }
                    }
                    Err(_) => {
                        nextcloud_url.set(String::from("Not currently syncing with any server"));
                        configured_effect.set(false);
                    }
                }
            });

            || () // Return empty cleanup function
        });
    }

    // Handler for initiating authentication
    let on_authenticate_click = {
        let app_dispatch = _dispatch.clone();
        let server_url = server_url.clone();
        let server_url_initiate = server_url.clone();
        // let audio_dispatch = audio_dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let auth_status = auth_status.clone();
        Callback::from(move |_| {
            let app_dispatch = app_dispatch.clone();
            let auth_status = auth_status.clone();
            let server = (*server_url_initiate).clone().trim().to_string();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let dispatch_clone = app_dispatch.clone();

            if !server.trim().is_empty() {
                wasm_bindgen_futures::spawn_local(async move {
                    match initiate_nextcloud_login(
                        &server,
                        &server_name.clone().unwrap(),
                        &api_key.clone().unwrap().unwrap(),
                        user_id.clone().unwrap(),
                    )
                    .await
                    {
                        Ok(login_data) => {
                            match open_nextcloud_login(&login_data.login).await {
                                Ok(_) => println!("Opened login URL in new tab"),
                                Err(e) => println!("Failed to open login URL in new tab: {:?}", e),
                            }
                            // Use login_data.poll_endpoint and login_data.token for the next steps
                            let auth_request = NextcloudAuthRequest {
                                user_id: user_id.clone().unwrap(),
                                token: login_data.poll.token,
                                poll_endpoint: login_data.poll.endpoint,
                                nextcloud_url: server.clone(),
                            };
                            match call_add_nextcloud_server(
                                &server_name.clone().unwrap(),
                                &api_key.clone().unwrap().unwrap(),
                                auth_request,
                            )
                            .await
                            {
                                Ok(_) => {
                                    log::info!("pinepods server now polling nextcloud");
                                    // Start polling the check_gpodder_settings endpoint
                                    loop {
                                        match call_check_nextcloud_server(
                                            &server_name.clone().unwrap(),
                                            &api_key.clone().unwrap().unwrap(),
                                            user_id.clone().unwrap(),
                                        )
                                        .await
                                        {
                                            Ok(response) => {
                                                if response.data {
                                                    log::info!("gPodder settings have been set up");
                                                    app_dispatch.reduce_mut(|audio_state| audio_state.info_message = Option::from("Nextcloud server has been authenticated successfully".to_string()));

                                                    // Set `is_refreshing` to true and start the WebSocket refresh
                                                    let server_name_call = server_name.clone();
                                                    let user_id_call = user_id.clone();
                                                    let api_key_call = api_key.clone();
                                                    dispatch_clone.reduce_mut(|state| {
                                                        state.is_refreshing = Some(true);
                                                        state.clone() // Return the modified state
                                                    });

                                                    spawn_local(async move {
                                                        if let Err(e) =
                                                            connect_to_episode_websocket(
                                                                &server_name_call.unwrap(),
                                                                &user_id_call.unwrap(),
                                                                &api_key_call.unwrap().unwrap(),
                                                                true,
                                                                dispatch_clone.clone(),
                                                            )
                                                            .await
                                                        {
                                                            web_sys::console::log_1(
                                                                &format!("Failed to connect to WebSocket: {:?}", e).into(),
                                                            );
                                                        } else {
                                                            web_sys::console::log_1(
                                                                &"WebSocket connection established and refresh initiated.".into(),
                                                            );
                                                        }

                                                        // Stop the loading animation after the WebSocket operation is complete
                                                        dispatch_clone.reduce_mut(|state| {
                                                            state.is_refreshing = Some(false);
                                                            state.clone() // Return the modified state
                                                        });
                                                    });

                                                    break;
                                                } else {
                                                    log::info!("gPodder settings are not yet set up, continuing to poll...");
                                                }
                                            }
                                            Err(e) => log::error!(
                                                "Error calling check_gpodder_settings: {:?}",
                                                e
                                            ),
                                        }

                                        // // Wait for a short period before polling again
                                        let delay = std::time::Duration::from_secs(5);
                                        async_std::task::sleep(delay).await;
                                        // let _ = wasm_timer::Delay::new(delay).await;
                                    }
                                }
                                Err(e) => {
                                    log::error!("Error calling add_nextcloud_server: {:?}", e);
                                    let formatted_error = format_error_message(&e.to_string());
                                    app_dispatch.reduce_mut(|audio_state| {
                                        audio_state.error_message = Option::from(
                                            format!(
                                                "Error calling add_nextcloud_server: {}",
                                                formatted_error
                                            )
                                            .to_string(),
                                        )
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(&JsValue::from_str(&format!(
                                "Failed to initiate Nextcloud login: {:?}",
                                e
                            )));
                            app_dispatch.reduce_mut(|audio_state| audio_state.error_message = Option::from("Failed to initiate Nextcloud login. Please check the server URL.".to_string()));
                            auth_status.set(
                                "Failed to initiate Nextcloud login. Please check the server URL."
                                    .to_string(),
                            );
                        }
                    }
                });
            } else {
                auth_status.set("Please enter a Nextcloud server URL.".to_string());
                app_dispatch.reduce_mut(|audio_state| {
                    audio_state.error_message =
                        Option::from("Please enter a Nextcloud Server URL".to_string())
                });
            }
        })
    };

    let on_remove_sync_click = {
        let dispatch = _dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let nextcloud_url = nextcloud_url.clone();
        let is_sync_configured = is_sync_configured.clone();
        let is_loading = is_loading.clone();

        Callback::from(move |_| {
            let dispatch = dispatch.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let nextcloud_url = nextcloud_url.clone();
            let is_sync_configured = is_sync_configured.clone();
            let is_loading = is_loading.clone();

            is_loading.set(true);

            wasm_bindgen_futures::spawn_local(async move {
                match call_remove_podcast_sync(
                    &server_name.clone().unwrap(),
                    &api_key.clone().unwrap().unwrap(),
                    user_id.clone().unwrap(),
                )
                .await
                {
                    Ok(success) => {
                        if success {
                            nextcloud_url
                                .set(String::from("Not currently syncing with any server"));
                            is_sync_configured.set(false);
                            dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Podcast sync settings removed successfully".to_string());
                            });
                        } else {
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some("Failed to remove sync settings".to_string());
                            });
                        }
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Error removing sync settings: {}", formatted_error));
                        });
                    }
                }
                is_loading.set(false);
            });
        })
    };

    // Handler for initiating authentication
    let on_authenticate_server_click = {
        let server_url = server_url.clone();
        let server_user = server_user.clone();
        let server_pass = server_pass.clone();
        let server_url_initiate = server_url.clone();
        // let audio_dispatch = audio_dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let auth_status = auth_status.clone();
        Callback::from(move |_| {
            let auth_status = auth_status.clone();
            let server = (*server_url_initiate).clone().trim().to_string();
            let server_user = server_user.clone();
            let server_pass = server_pass.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let dispatch_clone = _dispatch.clone();
            let user_id = user_id.clone();
            let server_user_check_deref = (*server_user).clone();
            let server_user_deref = (*server_user).clone();
            let server_pass_check_deref = (*server_pass).clone();
            let server_pass_deref = (*server_pass).clone();

            if !server.trim().is_empty() {
                wasm_bindgen_futures::spawn_local(async move {
                    let auth_request = GpodderAuthRequest {
                        user_id: user_id.clone().unwrap(),
                        gpodder_url: server.clone(),
                        gpodder_username: server_user_deref,
                        gpodder_password: server_pass_deref,
                    };
                    let check_request = GpodderCheckRequest {
                        gpodder_url: server.clone(),
                        gpodder_username: server_user_check_deref,
                        gpodder_password: server_pass_check_deref,
                    };
                    match call_verify_gpodder_auth(&server_name.clone().unwrap(), check_request)
                        .await
                    {
                        Ok(auth_response) => {
                            if auth_response.status == "success" {
                                match call_add_gpodder_server(
                                    &server_name.clone().unwrap(),
                                    &api_key.clone().unwrap().unwrap(),
                                    auth_request,
                                )
                                .await
                                {
                                    Ok(_) => {
                                        log::info!(
                                            "Gpodder server now added and podcasts syncing!"
                                        );
                                        dispatch_clone.reduce_mut(|audio_state| {
                                            audio_state.info_message = Option::from(
                                                "Gpodder server now added and podcasts syncing!"
                                                    .to_string(),
                                            )
                                        });
                                        // Set `is_refreshing` to true and start the WebSocket refresh
                                        let server_name_call = server_name.clone();
                                        let user_id_call = user_id.clone();
                                        let api_key_call = api_key.clone();
                                        dispatch_clone.reduce_mut(|state| {
                                            state.is_refreshing = Some(true);
                                            state.clone() // Return the modified state
                                        });

                                        spawn_local(async move {
                                            if let Err(e) = connect_to_episode_websocket(
                                                &server_name_call.unwrap(),
                                                &user_id_call.unwrap(),
                                                &api_key_call.unwrap().unwrap(),
                                                true,
                                                dispatch_clone.clone(),
                                            )
                                            .await
                                            {
                                                web_sys::console::log_1(
                                                    &format!(
                                                        "Failed to connect to WebSocket: {:?}",
                                                        e
                                                    )
                                                    .into(),
                                                );
                                            } else {
                                                web_sys::console::log_1(
                                                    &"WebSocket connection established and refresh initiated.".into(),
                                                );
                                            }

                                            // Stop the loading animation after the WebSocket operation is complete
                                            dispatch_clone.reduce_mut(|state| {
                                                state.is_refreshing = Some(false);
                                                state.clone() // Return the modified state
                                            });
                                        });
                                        // Start polling the check_gpodder_settings endpoint
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(&JsValue::from_str(&format!(
                                            "Failed to add Gpodder server: {:?}",
                                            e
                                        )));
                                        let formatted_error = format_error_message(&e.to_string());
                                        dispatch_clone.reduce_mut(|audio_state| audio_state.error_message = Option::from("Failed to add Gpodder server. Please check the server URL.".to_string()));
                                        auth_status.set(
                                            format!("Failed to add Gpodder server. Please check the server URL and credentials. {:?}", formatted_error)
                                                .to_string(),
                                        );
                                    }
                                }
                            } else {
                                web_sys::console::log_1(&JsValue::from_str(
                                    "Authentication failed.",
                                ));
                                dispatch_clone.reduce_mut(|audio_state| {
                                    audio_state.error_message = Option::from(
                                        "Authentication failed. Please check your credentials."
                                            .to_string(),
                                    )
                                });
                                auth_status.set(
                                    "Authentication failed. Please check your credentials."
                                        .to_string(),
                                );
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(&JsValue::from_str(&format!(
                                "Failed to verify Gpodder auth: {:?}",
                                e
                            )));
                            dispatch_clone.reduce_mut(|audio_state| {
                                audio_state.error_message = Option::from(
                                    "Failed to verify Gpodder auth. Please check the server URL."
                                        .to_string(),
                                )
                            });
                            auth_status.set(
                                "Failed to verify Gpodder auth. Please check the server URL."
                                    .to_string(),
                            );
                        }
                    }
                });
            } else {
                auth_status.set("Please enter a Gpodder server URL.".to_string());
                dispatch_clone.reduce_mut(|audio_state| {
                    audio_state.error_message =
                        Option::from("Please enter a Gpodder Server URL".to_string())
                });
            }
        })
    };

    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{"Nextcloud Podcast Sync:"}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{"With this option you can authenticate with a Nextcloud or Gpodder server to use as a podcast sync client. This option works great with AntennaPod on Android so you can have the same exact feed there while on mobile. In addition, if you're already using AntennaPod with Nextcloud Podcast sync you can connect your existing sync feed to quickly import everything right into Pinepods! You'll only enter information for one of the below options. Nextcloud requires that you have the gpodder sync add-on in nextcloud and the gpodder option requires you to have an external gpodder podcast sync server that authenticates via user and pass. Such as this: https://github.com/kd2org/opodsync."}</p> // Styled paragraph
            <div class="flex items-center mb-4">
                <p class="item_container-text text-md mr-4">{"Current Podcast Sync Server: "}<span class="item_container-text font-bold">{(*nextcloud_url).clone()}</span></p>
                {
                    if *is_sync_configured {
                        html! {
                            <button
                                onclick={on_remove_sync_click}
                                disabled={*is_loading}
                                class="ml-4 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                            >
                            {
                                if *is_loading {
                                    html! { <span class="flex items-center"><i class="ph ph-spinner animate-spin mr-2"></i>{"Removing..."}</span> }
                                } else {
                                    html! { "Remove Sync" }
                                }
                            }
                            </button>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
            <br/>
            <label for="server_url" class="item_container-text block mb-2 text-sm font-medium">{ "New Nextcloud Server" }</label>
            <div class="flex items-center">
                <input type="text" id="first_name" oninput={on_server_url_change.clone()} class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500" placeholder="https://nextcloud.com" />
                <button onclick={on_authenticate_click} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                {"Authenticate"}
                </button>
            </div>

            <label for="server_url" class="item_container-text block mb-2 text-sm font-medium">{ "GPodder-compatible Server" }</label>
            <div class="flex items-center">
                <input type="text" id="url" oninput={on_server_url_change} class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500" placeholder="https://mypodcastsync.mydomain.com" />
                <input type="text" id="username" oninput={on_username_change} class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500" placeholder="myusername" />
                <input type="password" id="password" oninput={on_password_change} class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500" placeholder="mypassword" />
                <button onclick={on_authenticate_server_click} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                {"Authenticate"}
                </button>
            </div>
            // <input type="text" class="input" placeholder="Enter Nextcloud server URL" value={(*server_url).clone()} oninput={on_server_url_change} />

        </div>
    }
}
