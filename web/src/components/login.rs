use yew::prelude::*;
// use yew::{function_component, html, use_state, Callback, Html, InputEvent};
use web_sys::HtmlInputElement;
use log::{info, warn};
use web_sys::console;


#[function_component(Login)]
pub fn login() -> Html {
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
        let username = username.clone();
        let password = password.clone();
        Callback::from(move |_| {
            // Handle the login logic here
            // For example, send the username and password to a server
            console::log_1(&"Logging in...".into());
            let message = format!("Logging in with username: {}, password: {}", *username, *password);
            console::log_1(&message.into());
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
            <button
                onclick={on_submit}
                class="p-2 bg-blue-500 text-white rounded hover:bg-blue-600"
            >
                {"Submit"}
            </button>
        </div>
    </div>
}

}