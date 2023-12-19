use yew::prelude::*;
// use yew::{function_component, html, use_state, Callback, Html, InputEvent};
use web_sys::HtmlInputElement;
use log::{info, warn};
use web_sys::console;
// use yew_router::router::_RouterProps::history;
use yew_router::prelude::*;
use yew_router::history::History;
use crate::requests::login_requests;
use crate::Route;


#[function_component(Login)]
pub fn login() -> Html {
    // let history =  History;
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

    let on_submit = {
        Callback::from(move |_| {
            let username = username.clone();
            let password = password.clone();
            // Handle the login logic here
            // For example, send the username and password to a server
            wasm_bindgen_futures::spawn_local(async move {
                match login_requests::login(username.to_string(), password.to_string()).await {
                    Ok(response) => {
                        // Handle successful response
                        History::push(Route::Home);
                    },
                    Err(error) => {
                        // Handle error
                    }
                }
            })
        })
    };

    let on_different_server = {
        Callback::from(move |_| {
            println!('t')
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