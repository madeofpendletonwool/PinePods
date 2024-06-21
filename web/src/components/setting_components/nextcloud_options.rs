use crate::components::context::{AppState, UIState};
use crate::requests::setting_reqs::{
    call_add_gpodder_server, call_add_nextcloud_server, call_check_nextcloud_server,
    call_get_nextcloud_server, initiate_nextcloud_login, GpodderAuthRequest, NextcloudAuthRequest,
    NextcloudInitiateResponse,
};
use serde::Deserialize;
use serde::Serialize;
use serde_wasm_bindgen;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{console, HtmlInputElement, Request, RequestInit, RequestMode, Response};
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

// async fn initiate_nextcloud_login(server_url: &str, server_name: &str) -> Result<NextcloudLoginResponse, anyhow::Error> {
//     let login_endpoint = format!("{}/index.php/login/v2", server_url);
//     let window = web_sys::window().expect("no global `window` exists");
//     let request = Request::new_with_str_and_init(&login_endpoint, RequestInit::new().method("POST").mode(RequestMode::Cors))
//         .expect("Failed to build request.");

//     match JsFuture::from(window.fetch_with_request(&request)).await {
//         Ok(js_value) => {
//             console::log_1(&"Received response from server...".into());
//             let response: Response = js_value.dyn_into().unwrap();
//             if response.status() == 200 {
//                 console::log_1(&"Response status is 200...".into());
//                 match JsFuture::from(response.json().unwrap()).await {
//                     Ok(json_result) => {
//                         console::log_1(&"Before login response".into());
//                         match serde_wasm_bindgen::from_value::<NextcloudLoginResponse>(json_result) {
//                             Ok(login_data) => {
//                                 console::log_1(&format!("Login URL: {}", &login_data.login.clone()).into());
//                                 window.open_with_url(&login_data.login).expect("Failed to open login URL");
//                                 Ok(login_data)
//                             },
//                             Err(_) => {
//                                 console::log_1(&"Failed to deserialize JSON response...".into());
//                                 Err(anyhow::Error::msg("Failed to deserialize JSON response"))
//                             },
//                         }
//                     },
//                     Err(_) => {
//                         console::log_1(&"Failed to parse JSON response...".into());
//                         Err(anyhow::Error::msg("Failed to parse JSON response"))
//                     },
//                 }
//             } else {
//                 console::log_1(&format!("Failed to initiate Nextcloud login, status: {}", response.status()).into());
//                 Err(anyhow::Error::msg(format!("Failed to initiate Nextcloud login, status: {}", response.status())))
//             }
//         },
//         Err(_) => {
//             console::log_1(&"Failed to send authentication request...".into());
//             Err(anyhow::Error::msg("Failed to send authentication request."))
//         },
//     }
// }

async fn open_nextcloud_login(url: &str) -> Result<(), JsValue> {
    let window = web_sys::window().expect("no global `window` exists");
    window.open_with_url_and_target(url, "_blank")?;
    Ok(())
}

#[function_component(NextcloudOptions)]
pub fn nextcloud_options() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let server_url = use_state(|| String::new());
    let server_user = use_state(|| String::new());
    let server_pass = use_state(|| String::new());
    let auth_status = use_state(|| String::new());
    let nextcloud_url = use_state(|| String::new()); // State to hold the Nextcloud server URL
    let _error_message = audio_state.error_message.clone();
    let _info_message = audio_state.info_message.clone();

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
                        nextcloud_url.set(server);
                    }
                    Err(_) => {
                        nextcloud_url
                            .set(String::from("Not currently syncing with Nextcloud server"));
                    }
                }
            });

            || () // Return empty cleanup function
        });
    }

    // Handler for initiating authentication
    let on_authenticate_click = {
        let server_url = server_url.clone();
        let server_url_initiate = server_url.clone();
        // let audio_dispatch = audio_dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let auth_status = auth_status.clone();
        let audio_dispatch_call = audio_dispatch.clone();
        Callback::from(move |_| {
            let audio_dispatch = audio_dispatch_call.clone();
            let auth_status = auth_status.clone();
            let server = (*server_url_initiate).clone().trim().to_string();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();

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
                                                    audio_dispatch.reduce_mut(|audio_state| audio_state.info_message = Option::from("Nextcloud server has been authenticated successfully".to_string()));
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
                                    audio_dispatch.reduce_mut(|audio_state| {
                                        audio_state.error_message = Option::from(
                                            format!("Error calling add_nextcloud_server: {}", e)
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
                            audio_dispatch.reduce_mut(|audio_state| audio_state.error_message = Option::from("Failed to initiate Nextcloud login. Please check the server URL.".to_string()));
                            auth_status.set(
                                "Failed to initiate Nextcloud login. Please check the server URL."
                                    .to_string(),
                            );
                        }
                    }
                });
            } else {
                auth_status.set("Please enter a Nextcloud server URL.".to_string());
                audio_dispatch.reduce_mut(|audio_state| {
                    audio_state.error_message =
                        Option::from("Please enter a Nextcloud Server URL".to_string())
                });
            }
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
            let audio_dispatch = audio_dispatch.clone();
            let auth_status = auth_status.clone();
            let server = (*server_url_initiate).clone().trim().to_string();
            let server_user = server_user.clone();
            let server_pass = server_pass.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let server_user_deref = (*server_user).clone();
            let server_pass_deref = (*server_pass).clone();

            if !server.trim().is_empty() {
                wasm_bindgen_futures::spawn_local(async move {
                    let auth_request = GpodderAuthRequest {
                        user_id: user_id.clone().unwrap(),
                        username: server_user_deref,
                        password: server_pass_deref,
                        nextcloud_url: server.clone(),
                    };
                    match call_add_gpodder_server(
                        &server_name.clone().unwrap(),
                        &api_key.clone().unwrap().unwrap(),
                        auth_request,
                    )
                    .await
                    {
                        Ok(_) => {
                            log::info!("Gpodder server now added and podcasts syncing!");
                            // Start polling the check_gpodder_settings endpoint
                        }
                        Err(e) => {
                            web_sys::console::log_1(&JsValue::from_str(&format!(
                                "Failed to initiate Nextcloud login: {:?}",
                                e
                            )));
                            audio_dispatch.reduce_mut(|audio_state| audio_state.error_message = Option::from("Failed to initiate Gpodder login. Please check the server URL.".to_string()));
                            auth_status.set(
                                "Failed to initiate Gpodder login. Please check the server URL."
                                    .to_string(),
                            );
                        }
                    }
                });
            } else {
                auth_status.set("Please enter a Gpodder server URL.".to_string());
                audio_dispatch.reduce_mut(|audio_state| {
                    audio_state.error_message =
                        Option::from("Please enter a Gpodder Server URL".to_string())
                });
            }
        })
    };

    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{"Nextcloud Podcast Sync:"}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{"With this option you can authenticate with a Nextcloud or Gpodder server to use as a podcast sync client. This option works great with AntennaPod on Android so you can have the same exact feed there while on mobile. In addition, if you're already using AntennaPod with Nextcloud Podcast sync you can connect your existing sync feed to quickly import everything right into Pinepods! You'll only enter information for one of the below options. Nextcloud requires that you have the gpodder sync add-on in nextcloud and the gpodder option requires you to have an external gpodder podcast sync server. Such as this: https://github.com/kd2org/opodsync."}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{"Current Nextcloud Server: "}<span class="item_container-text font-bold">{(*nextcloud_url).clone()}</span></p> // Styled paragraph
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
