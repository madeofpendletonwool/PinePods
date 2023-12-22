use yew::prelude::*;
// use yew::{function_component, html, use_state, Callback, Html, InputEvent};
use web_sys::{HtmlInputElement, window};
use log::{info, warn};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::console;
use web_sys::console::error;
// use yew_router::router::_RouterProps::history;
use yew_router::prelude::*;
use yew_router::history::{BrowserHistory, History};
use crate::requests::login_requests;


#[function_component(Login)]
pub fn login() -> Html {
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

#[function_component(ChangeServer)]
pub fn login() -> Html {
    let history = BrowserHistory::new();
    let server_name = use_state(|| "".to_string());
    let username = use_state(|| "".to_string());
    let password = use_state(|| "".to_string());
    let error_message = use_state(|| None::<String>);
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
    let on_submit = {
        Callback::from(move |_| {
            let history = history_clone.clone();
            let username = username.clone();
            let password = password.clone();
            let server_name = server_name.clone();
            let error_message = error_message_clone.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match login_requests::login_new_server(server_name.to_string(), username.to_string(), password.to_string()).await {
                    Ok(_) => {
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