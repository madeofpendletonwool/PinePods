use yew::prelude::*;
use web_sys::{console, window};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use yew_router::history::{BrowserHistory, History};
use crate::requests::login_requests;
use crate::components::context::{AppState, UIState};
// use yewdux::prelude::*;
use md5;
use yewdux::prelude::*;
use crate::requests::login_requests::{AddUserRequest, call_add_login_user};
use crate::requests::setting_reqs::call_get_theme;
use crate::components::gen_funcs::{encode_password, validate_user_input};
use crate::components::episodes_layout::UIStateMsg;

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
    let new_username = use_state(|| "".to_string());
    let new_password = use_state(|| "".to_string());
    let email = use_state(|| "".to_string());
    let fullname = use_state(|| "".to_string());
    let (app_state, dispatch) = use_store::<AppState>();
    let (state, _dispatch) = use_store::<UIState>();
    let error_message = app_state.error_message.clone();

    {
        let ui_dispatch = _dispatch.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let closure = Closure::wrap(Box::new(move |_event: Event| {
                ui_dispatch.apply(UIStateMsg::ClearErrorMessage);
                ui_dispatch.apply(UIStateMsg::ClearInfoMessage);
            }) as Box<dyn Fn(_)>);

            document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();

            // Return cleanup function
            move || {
                document.remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }
    let effect_displatch = dispatch.clone();
    // User Auto Login with saved state
    use_effect_with((), {
        // let error_clone_use = error_message_clone.clone();
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
                                                        console::log_1(&format!("auth deets: {:?}", &auth_state_clone).into());
                                                        let email = &app_state.user_details.as_ref().unwrap().Email;
                                                        let user_id = app_state.user_details.as_ref().unwrap().UserID.clone();
                                                        // Safely access server_name and api_key
                                                        let auth_details_clone = auth_state_clone.auth_details.clone();
                                                        if let Some(auth_details) = auth_details_clone.as_ref() {
                                                            let server_name = auth_details.server_name.clone();
                                                            let api_key = auth_details.api_key.clone().unwrap_or_default();
                                                            
                                                            console::log_1(&format!("user email: {:?}", &email).into());
                                                            let gravatar_url = generate_gravatar_url(&Some(email.clone().unwrap()), 80);
                                                            console::log_1(&format!("gravatar_url: {:?}", &gravatar_url).into());
                                                            // Auto login logic here
                                                            effect_displatch.reduce_mut(move |state| {
                                                                state.user_details = app_state.user_details;
                                                                state.auth_details = Some(auth_details.clone());
                                                                state.server_details = server_details.server_details;
                                                                state.gravatar_url = Some(gravatar_url);
    
                                                            });
                                                            // let mut error_message = app_state.error_message;
                                                            // Retrieve the originally requested route, if any
                                                            let session_storage = window.session_storage().unwrap().unwrap();
                                                            session_storage.set_item("isAuthenticated", "true").unwrap();
                                                            let requested_route = storage.get_item("requested_route").unwrap_or(None);
    
                                                            // Get Theme
                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                console::log_1(&format!("theme test server: {:?}", server_name.clone()).into());
                                                                console::log_1(&format!("theme test api: {:?}", api_key.clone()).into());
                                                                match call_get_theme(server_name, api_key, &user_id).await{
                                                                    Ok(theme) => {
                                                                        console::log_1(&format!("theme test: {:?}", &theme).into());
                                                                        crate::components::setting_components::theme_options::changeTheme(&theme);
                                                                    }
                                                                    Err(e) => {
                                                                        console::log_1(&format!("Error getting theme: {:?}", e).into());
                                                                    }
                                                                }
                                                            });
                                                            let redirect_route = requested_route.unwrap_or_else(|| "/home".to_string());
                                                            history.push(&redirect_route); // Redirect to the requested or home page
                                                            // console::log_1(&format!("Server: {:?}", server_name).into());
                                                            // console::log_1(&format!("API Key: {:?}", api_key).into());
                                                        } else {
                                                            console::log_1(&"Auth details are None".into());
                                                        }
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
    let dispatch_clone = dispatch.clone();
    let on_submit = {
        let submit_dispatch = dispatch.clone();
        Callback::from(move |_| {
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            let dispatch = submit_dispatch.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match login_requests::login_new_server("http://localhost:8040".to_string(), username.to_string(), password.to_string()).await {
                    Ok((user_details, login_request, server_details)) => {
                        // After user login, update the image URL with user's email from user_details
                        let gravatar_url = generate_gravatar_url(&user_details.Email, 80); // 80 is the image size
    
                        dispatch.reduce_mut(move |state| {
                            state.user_details = Some(user_details);
                            state.auth_details = Some(login_request);
                            state.server_details = Some(server_details);
                            state.gravatar_url = Some(gravatar_url); // Store the Gravatar URL
    
                            state.store_app_state();
                        });
    
                        history.push("/home"); // Use the route path
                    },
                    Err(_) => {
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
        CreateUser,
        ForgotPassword,
    }

    // Define the initial state
    let page_state = use_state(|| PageState::Default);

    // Define the callback functions
    let on_create_new_user = {
        let page_state = page_state.clone();
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
            fullname.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_username_change = {
        let new_username = new_username.clone();
        Callback::from(move |e: InputEvent| {
            new_username.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_email_change = {
        let email = email.clone();
        Callback::from(move |e: InputEvent| {
            email.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_password_change = {
        let new_password = new_password.clone();
        Callback::from(move |e: InputEvent| {
            new_password.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };    

    let on_create_submit = {
        let page_state = page_state.clone();
        let fullname = fullname.clone().to_string();
        let new_username = new_username.clone().to_string();
        let email = email.clone().to_string();
        let new_password = new_password.clone();
        let add_user_request = app_state.add_user_request.clone();
        let error_message_create = error_message.clone();
        let dispatch_wasm = dispatch.clone();
        Callback::from(move |e: MouseEvent| {
            let dispatch = dispatch_wasm.clone();
            let new_username = new_username.clone();
            let new_password = new_password.clone();
            let fullname = fullname.clone();
            let hash_pw = String::new();
            let salt = String::new();
            let email = email.clone();
            let page_state = page_state.clone();
            let error_message_clone = error_message_create.clone();
            e.prevent_default();
            page_state.set(PageState::Default);
            // Hash the password and generate a salt
            match validate_user_input(&new_username, &new_password, &email) {
                Ok(_) => {
                    match encode_password(&new_password) {
                        Ok((hash_pw)) => {
                                        // Set the state
                            dispatch.reduce_mut(move |state| {
                                state.add_user_request = Some(AddUserRequest {
                                    fullname,
                                    new_username,
                                    email,
                                    hash_pw,
                                });
                            });

                            let add_user_request = add_user_request.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match call_add_login_user("http://localhost:8040".to_string(), &add_user_request).await {
                                    Ok(success) => {
                                        if success {
                                            console::log_1(&"User added successfully".into());
                                            page_state.set(PageState::Default);
                                        } else {
                                            console::log_1(&"Error adding user".into());
                                            page_state.set(PageState::Default);
                                            dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error adding user")));

                                        }
                                    }
                                    Err(e) => {
                                        console::log_1(&format!("Error: {}", e).into());
                                        page_state.set(PageState::Default);
                                        dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error adding user: {:?}", e)));
                                    }
                                }
                            });
                        },
                        Err(e) => {
                            // Handle the error here
                            dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Password Encoding Failed {:?}", e)));
                        }
                    }
                },
                Err(e) => {
                    dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Invalid User Input {:?}", e)));
                    return;
                }
            }


        })
    };
    // Define the modal components
    let create_user_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700">
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
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
                                <label for="username" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Username"}</label>
                                <input oninput={on_username_change.clone()} type="text" id="username" name="username" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="password" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Password"}</label>
                                <input oninput={on_password_change.clone()} type="password" id="password" name="password" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="fullname" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Full Name"}</label>
                                <input oninput={on_fullname_change} type="text" id="fullname" name="fullname" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="email" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Email"}</label>
                                <input oninput={on_email_change} type="email" id="email" name="email" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <button type="submit" onclick={on_create_submit} class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Submit"}</button>
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


    let forgot_password_modal = html! {
        <div id="forgot-password-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700">
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
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
                            <div>
                                <label for="username" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Username"}</label>
                                <input type="text" id="username" name="username" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="email" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Email"}</label>
                                <input type="email" id="email" name="email" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <button type="submit" class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Submit"}</button>
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


    html! {
        <>
        {
            match *page_state {
            PageState::CreateUser => create_user_modal,
            PageState::ForgotPassword => forgot_password_modal,
            _ => html! {},
            }
        }
        
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
                            onclick={on_forgot_password}
                            class="text-sm text-blue-500 hover:text-blue-700"
                        >
                            {"Forgot Password?"}
                        </button>
                        <button
                            onclick={on_create_new_user}
                            class="text-sm text-blue-500 hover:text-blue-700"
                        >
                            {"Create New User"}
                        </button>
                    </div>
                    <button
                        onclick={on_submit}
                        class="p-2 bg-blue-500 text-white rounded hover:bg-blue-600"
                    >
                        {"Login"}
                    </button>
                </div>
                {
                    if app_state.error_message.as_ref().map_or(false, |msg| !msg.is_empty()) {
                        html! { <div class="error-snackbar">{ &app_state.error_message }</div> }
                    } else {
                        html! {}
                    }
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
        
        </>
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
                        let gravatar_url = generate_gravatar_url(&user_details.Email, 80); // 80 is the image size

                        dispatch.reduce_mut(move |state| {
                            state.user_details = Some(user_details);
                            state.auth_details = Some(login_request);
                            state.server_details = Some(server_details);
                            state.gravatar_url = Some(gravatar_url); // Store the Gravatar URL

                            state.store_app_state();
                        });

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
                    {"Login"}
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

    // Clear local and session storage
    let window = web_sys::window().expect("no global `window` exists");
    let local_storage = window.local_storage().expect("localStorage not enabled").expect("localStorage is null");
    let session_storage = window.session_storage().expect("sessionStorage not enabled").expect("sessionStorage is null");
    local_storage.clear().expect("failed to clear localStorage");
    session_storage.clear().expect("failed to clear sessionStorage");

    // Redirect to root path
    history.push("/");

    html! {}
}