use yew::prelude::*;
use web_sys::{console, HtmlInputElement, window};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use yew_router::history::{BrowserHistory, History};
use crate::requests::login_requests;
use crate::components::context::{AppState};
use yewdux::prelude::*;

use yewdux::prelude::*;
use crate::requests::pod_req::call_verify_pinepods;

#[function_component(Login)]
pub fn login() -> Html {
    let history = BrowserHistory::new();
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());
    let (app_state, dispatch) = use_store::<AppState>();
    let mut error_message = use_state(|| None::<String>);
    let error_message_clone = error_message.clone();

    {
        let error_message = error_message.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let error_message_clone = error_message.clone();
            let closure = Closure::wrap(Box::new(move |_event: Event| {
                error_message_clone.set(None);
            }) as Box<dyn Fn(_)>);

            if error_message.is_some() {
                document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
            }

            // Return cleanup function
            move || {
                if error_message.is_some() {
                    document.remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
                }
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }

    // User Auto Login with saved state
    use_effect_with((), {
        let error_clone_use = error_message_clone.clone();
        let history = history.clone();
        move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(local_storage) = window.local_storage() {
                    if let Some(storage) = local_storage {
                        if let Ok(Some(user_state)) = storage.get_item("userState") {
                            let app_state_result = AppState::deserialize(&user_state);

                            if let Ok(Some(auth_state)) = storage.get_item("userAuthState") {
                                match AppState::deserialize(&auth_state) {
                                    Ok(auth_details) => { // Successful deserialization of auth state
                                        if let Ok(Some(server_state)) = storage.get_item("serverState") {
                                            let server_details_result = AppState::deserialize(&server_state);

                                            if let Ok(app_state) = app_state_result { // Successful deserialization of user state
                                                if let Ok(server_details) = server_details_result { // Successful deserialization of server state
                                                    // Check if the deserialized state contains valid data
                                                    if app_state.user_details.is_some() && auth_details.auth_details.is_some() && server_details.server_details.is_some() {

                                                        let app_state_clone = app_state.clone();
                                                        let auth_state_clone = auth_details.clone();
                                                        // Auto login logic here
                                                        dispatch.reduce_mut(move |state| {
                                                            state.user_details = app_state.user_details;
                                                            state.auth_details = auth_details.auth_details;
                                                            state.server_details = server_details.server_details;

                                                        });
                                                        console::log_1(&format!("user_id: {:?}", &app_state_clone).into());
                                                        console::log_1(&format!("auth_details: {:?}", &auth_state_clone).into());
                                                        // let mut error_message = app_state.error_message;
                                                        // Retrieve the originally requested route, if any
                                                        let session_storage = window.session_storage().unwrap().unwrap();
                                                        session_storage.set_item("isAuthenticated", "true").unwrap();
                                                        let requested_route = storage.get_item("requested_route").unwrap_or(None);

                                                        let redirect_route = requested_route.unwrap_or_else(|| "/home".to_string());
                                                        history.push(&redirect_route); // Redirect to the requested or home page
//                                                         let server_name_local = auth_state_clone.auth_details.as_ref().map(|ad| ad.server_name.clone());
//                                                         let api_key_local = auth_state_clone.auth_details.as_ref().and_then(|ad| ad.api_key.clone());
//
//                                                         // let server_name_local = auth_details.auth_details.as_ref().and_then(|ad| ad.server_name.clone());
//                                                         console::log_1(&format!("server_name_pre_wasm: {:?}", &server_name_local).into());
//
// // Use the local variables instead of `app_state`
//                                                         wasm_bindgen_futures::spawn_local(async move {
//                                                             match call_verify_pinepods(server_name_local.unwrap(), api_key_local).await {
//                                                                 Ok(_) => {
//                                                                     history.push("/home"); // Redirect to the home page
//                                                                 }
//                                                                 Err(e) => {
//                                                                     error_clone_use.set(Some(e.to_string())); // Set the error message
//                                                                 }
//                                                             }
//                                                         });
                                                    }
                                                }
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        web_sys::console::log_1(&format!("Error deserializing auth state: {:?}", e).into());
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




    let on_username_change = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            username.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_password_change = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            password.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    let history_clone = history.clone();
    let on_submit = {
        Callback::from(move |_| {
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match login_requests::login(username.to_string(), password.to_string()).await {
                    Ok(_) => {
                        history.push("/home"); // Use the route path

//                         // Function to calculate the MD5 hash of the user's email
//                         fn calculate_gravatar_hash(email: &str) -> String {
//                             // Implement the MD5 hash calculation here
//                         }
//
//                         // Function to generate the Gravatar URL
//                         fn generate_gravatar_url(email: &str, size: usize) -> String {
//                             let hash = calculate_gravatar_hash(email);
//                             format!("https://gravatar.com/avatar/{}?s={}", hash, size)
//                         }
//
//                         // After user login, update the image URL
//                         let user_email = "user@example.com"; // Replace with the actual user email
//                         let gravatar_url = generate_gravatar_url(user_email, 80); // 80 is the image size

                    },
                    Err(_) => {
                        // Handle error
                    }
                }
            });
        })
    };
    let history_clone = history.clone();
    let on_different_server = {
        Callback::from(move |_| {
            let history = history_clone.clone();
            history.push("/change_server"); // Use the route path
        })
    };

    html! {
        <div class="flex justify-center items-center h-screen">
            <div class="flex flex-col space-y-4 w-full max-w-xs p-8 border border-gray-300 rounded-lg shadow-lg">
                <div class="flex justify-center items-center">
                    <img class="object-scale-down h-20 w-66" src="static/assets/favicon.png" alt="Pinepods Logo" />
                </div>
                <h1 class="text-xl font-bold mb-2 text-center">{"Pinepods"}</h1>
                <p class="text-center">{"A Forest of Podcasts, Rooted in the Spirit of Self-Hosting"}</p>
                <input
                    type="text"
                    placeholder="Username"
                    class="p-2 border border-gray-300 rounded"
                    oninput={on_username_change}
                />
                <input
                    type="password"
                    placeholder="Password"
                    class="p-2 border border-gray-300 rounded"
                    oninput={on_password_change}
                />
                // Forgot Password and Create New User buttons
                <div class="flex justify-between">
                    <button
                        class="text-sm text-blue-500 hover:text-blue-700"
                    >
                        {"Forgot Password?"}
                    </button>
                    <button
                        class="text-sm text-blue-500 hover:text-blue-700"
                    >
                        {"Create New User"}
                    </button>
                </div>
                <button
                    onclick={on_submit}
                    class="p-2 bg-blue-500 text-white rounded hover:bg-blue-600"
                >
                    {"Submit"}
                </button>
            </div>
            // Conditional rendering for the error banner
            if let Some(error) = (*error_message).as_ref() {
                <div class="error-snackbar">{ error }</div>
            }
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
    }

}

#[function_component(ChangeServer)]
pub fn login() -> Html {
    let (app_state, dispatch) = use_store::<AppState>();
    let history = BrowserHistory::new();
    let server_name = use_state(|| "".to_string());
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());
    let error_message = use_state(|| None::<String>);
    let (app_state, dispatch) = use_store::<AppState>();


    {
        let error_message = error_message.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let error_message_clone = error_message.clone();
            let closure = Closure::wrap(Box::new(move |_event: Event| {
                error_message_clone.set(None);
            }) as Box<dyn Fn(_)>);

            if error_message.is_some() {
                document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
            }

            // Return cleanup function
            move || {
                if error_message.is_some() {
                    document.remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
                }
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }

    let on_server_name_change = {
        let server_name = server_name.clone();
        Callback::from(move |e: InputEvent| {
            server_name.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_username_change = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            username.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_password_change = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            password.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let history_clone = history.clone();
    let error_message_clone = error_message.clone();
    // let app_state_clone = app_state.clone();
    let dispatch_clone = dispatch.clone();
    let on_submit = {
        Callback::from(move |_| {
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            let server_name = server_name.clone();
            let error_message = error_message_clone.clone();
            let dispatch = dispatch.clone(); // No need to clone app_state here

            wasm_bindgen_futures::spawn_local(async move {
                match login_requests::login_new_server(server_name.to_string(), username.to_string(), password.to_string()).await {
                    Ok((user_details, login_request, server_details)) => {
                        // Use reduce_mut to modify the state directly
                        dispatch.reduce_mut(move |state| {
                            state.user_details = Some(user_details);
                            state.auth_details = Some(login_request);
                            state.server_details = Some(server_details);

                            state.store_app_state();

                        });

                        // console::log_1(&format!("Set User Context: {:?}", user_details).into());
                        // console::log_1(&format!("Set Auth Context: {:?}", login_request).into());
                        // state.store_app_state
                        history.push("/home"); // Use the route path
                    },
                    Err(e) => {
                        error_message.set(Some(e.to_string())); // Set the error message
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

    html! {
        <div class="flex justify-center items-center h-screen">
            <div class="flex flex-col space-y-4 w-full max-w-xs p-8 border border-gray-300 rounded-lg shadow-lg">
                <div class="flex justify-center items-center">
                    <img class="object-scale-down h-20 w-66" src="static/assets/favicon.png" alt="Pinepods Logo" />
                </div>
                <h1 class="text-xl font-bold mb-2 text-center">{"Pinepods"}</h1>
                <p class="text-center">{"A Forest of Podcasts, Rooted in the Spirit of Self-Hosting"}</p>
                <input
                    type="text"
                    placeholder="Server Name"
                    class="p-2 border border-gray-300 rounded"
                    oninput={on_server_name_change}
                    onkeypress={handle_key_press.clone()}
                />
                <input
                    type="text"
                    placeholder="Username"
                    class="p-2 border border-gray-300 rounded"
                    oninput={on_username_change}
                    onkeypress={handle_key_press.clone()}
                />
                <input
                    type="password"
                    placeholder="Password"
                    class="p-2 border border-gray-300 rounded"
                    oninput={on_password_change}
                    onkeypress={handle_key_press.clone()}
                />
                <button onclick={on_submit_click} class="p-2 bg-blue-500 text-white rounded hover:bg-blue-600">
                    {"Submit"}
                </button>
            </div>
            // Conditional rendering for the error banner
            if let Some(error) = (*error_message).as_ref() {
                <div class="error-snackbar">{ error }</div>
            }

            // Connect to Different Server button at bottom right
            <div class="fixed bottom-4 right-4">
                <button onclick={on_different_server} class="p-2 bg-gray-500 text-white rounded hover:bg-gray-600">
                    {"Connect to Local Server"}
                </button>
            </div>
        </div>
    }

}

#[function_component(LogOut)]
pub fn logout() -> Html {
    let history = BrowserHistory::new();
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());

    let on_username_change = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            username.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_password_change = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            password.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    let history_clone = history.clone();
    let on_submit = {
        Callback::from(move |_| {
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match login_requests::login(username.to_string(), password.to_string()).await {
                    Ok(_) => {
                        history.push("/home"); // Use the route path

//                         // Function to calculate the MD5 hash of the user's email
//                         fn calculate_gravatar_hash(email: &str) -> String {
//                             // Implement the MD5 hash calculation here
//                         }
//
//                         // Function to generate the Gravatar URL
//                         fn generate_gravatar_url(email: &str, size: usize) -> String {
//                             let hash = calculate_gravatar_hash(email);
//                             format!("https://gravatar.com/avatar/{}?s={}", hash, size)
//                         }
//
//                         // After user login, update the image URL
//                         let user_email = "user@example.com"; // Replace with the actual user email
//                         let gravatar_url = generate_gravatar_url(user_email, 80); // 80 is the image size

                    },
                    Err(_) => {
                        // Handle error
                    }
                }
            });
        })
    };
    let history_clone = history.clone();
    let on_different_server = {
        Callback::from(move |_| {
            let history = history_clone.clone();
            history.push("/change_server"); // Use the route path
        })
    };

    html! {
        <div class="flex justify-center items-center h-screen">
            <div class="flex flex-col space-y-4 w-full max-w-xs p-8 border border-gray-300 rounded-lg shadow-lg">
                <div class="flex justify-center items-center">
                    <img class="object-scale-down h-20 w-66" src="static/assets/favicon.png" alt="Pinepods Logo" />
                </div>
                <h1 class="text-xl font-bold mb-2 text-center">{"Pinepods"}</h1>
                <p class="text-center">{"A Forest of Podcasts, Rooted in the Spirit of Self-Hosting"}</p>
                <input
                    type="text"
                    placeholder="Username"
                    class="p-2 border border-gray-300 rounded"
                    oninput={on_username_change}
                />
                <input
                    type="password"
                    placeholder="Password"
                    class="p-2 border border-gray-300 rounded"
                    oninput={on_password_change}
                />
                // Forgot Password and Create New User buttons
                <div class="flex justify-between">
                    <button
                        class="text-sm text-blue-500 hover:text-blue-700"
                    >
                        {"Forgot Password?"}
                    </button>
                    <button
                        class="text-sm text-blue-500 hover:text-blue-700"
                    >
                        {"Create New User"}
                    </button>
                </div>
                <button
                    onclick={on_submit}
                    class="p-2 bg-blue-500 text-white rounded hover:bg-blue-600"
                >
                    {"Submit"}
                </button>
            </div>
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
    }

}