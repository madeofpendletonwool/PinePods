use wasm_bindgen::JsCast;
use yew::prelude::*;
use yewdux::prelude::*;
use yew_router::history::{BrowserHistory, History};
use crate::components::context::AppState;
use crate::requests::setting_reqs::call_restore_server;
use web_sys::{HtmlInputElement, Event};
use web_sys::{Blob, FileReader};
use wasm_bindgen::closure::Closure;

#[function_component(RestoreServer)]
pub fn restore_server() -> Html {
    let database_password = use_state(|| "".to_string());
    let file_content = use_state(|| "".to_string());
    let error_message = use_state(|| None::<String>);
    let info_message = use_state(|| None::<String>);

    // API key, server name, and other data can be fetched from AppState if required
    let (state, _) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    // Correct setup for `on_password_change`
    let on_password_change = {
        let database_password = database_password.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            database_password.set(input.value());
        })
    };

    // Correct setup for `on_file_change`
    let on_file_change = {
        let file_content = file_content.clone();
        let error_message = error_message.clone();
        Callback::from(move |e: Event| {
            let file_content = file_content.clone();
            let error_message = error_message.clone();
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) { // Directly get the File, no conversion needed
                    let blob: Blob = file.into(); // Convert File into Blob correctly
                    let reader = FileReader::new().expect("Failed to create FileReader");
                    let reader_clone = reader.clone();
                    let onloadend = Closure::wrap(Box::new(move |_event: Event| {
                        if let Ok(result) = reader_clone.result() {
                            let text = result.as_string().unwrap_or_default();
                            file_content.set(text);
                        } else {
                            error_message.set(Some("Failed to read file".into()));
                        }
                    }) as Box<dyn FnMut(_)>);
    
                    reader.set_onloadend(Some(onloadend.as_ref().unchecked_ref())); // Set the onloadend event listener
                    reader.read_as_text(&blob).expect("Failed to start reading file"); // Start reading the file as text
                    onloadend.forget(); // Prevent the closure from being cleaned up
                }
            }
        })
    };
    
    
    

    // Ensure `onclick_restore` is correctly used
    let onclick_restore = {
        let history = BrowserHistory::new();  // Get the browser history for navigation
        let api_key = api_key.unwrap_or_default();
        let server_name = server_name.unwrap_or_default();
        let database_password = (*database_password).clone();
        let file_content = (*file_content).clone();
        let error_message = error_message.clone();
        let info_message = info_message.clone();
        Callback::from(move |_| {
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let database_password = database_password.clone();
            let file_content = file_content.clone();
            let error_message = error_message.clone();
            let info_message = info_message.clone();
            let history = history.clone();  // Clone history for use in the async block
            wasm_bindgen_futures::spawn_local(async move {
                match call_restore_server(&server_name, &database_password, &file_content, &api_key.unwrap()).await {
                    Ok(message) => {
                        info_message.set(Some(message));
                        // Navigate to the logout route after initiating the restore process
                        history.push("/sign_out");
                    },
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                    }
                }
            });
        })
    };

    html! {
        <div class="p-4">
            <p class="item_container-text text-lg font-bold mb-4">{"Restore Server:"}</p>
            <p class="item_container-text text-md mb-4">{"With this option you can restore your entire server with all its previous settings, users, and data from a backup. Take a backup above to restore here. WARNING: This will delete everything on your server now and restore to the point that the backup contains."}</p>
            
            <br/>
            <input onchange={on_file_change} type="file" accept=".sql"/>
            <div class="flex items-center">
                <input type="password" id="db_pw" oninput={on_password_change.clone()} class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-700 dark:border-gray-600 dark:placeholder-gray-400 dark:text-white dark:focus:ring-blue-500 dark:focus:border-blue-500" placeholder="mYDBp@ss!" />
                <button onclick={onclick_restore} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                {"Restore Server"}
                </button>
            </div>
            // Conditional rendering for the error banner
            // if let Some(error) = error_message {
            //     <div class="error-snackbar">{ error }</div>
            // }
            // if let Some(info) = info_message {
            //     <div class="info-snackbar">{ info }</div>
            // }
        </div>
    }
}
