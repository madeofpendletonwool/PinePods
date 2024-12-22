use crate::components::context::AppState;
use crate::requests::setting_reqs::call_restore_server;
use js_sys::Uint8Array;
use wasm_bindgen::JsCast;
use web_sys::{Event, File, FormData, HtmlInputElement};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[function_component(RestoreServer)]
pub fn restore_server() -> Html {
    let database_password = use_state(|| "".to_string());
    let selected_file = use_state(|| None::<File>);
    let is_loading = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let info_message = use_state(|| None::<String>);

    let (state, _) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    let on_file_change = {
        let selected_file = selected_file.clone();
        let error_message = error_message.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    // Check file size (e.g., limit to 100MB)
                    if file.size() > 100.0 * 1024.0 * 1024.0 {
                        error_message
                            .set(Some("File size too large. Maximum size is 100MB.".into()));
                        return;
                    }
                    selected_file.set(Some(file));
                }
            }
        })
    };

    let onclick_restore = {
        let history = BrowserHistory::new();
        let api_key = api_key.unwrap_or_default();
        let server_name = server_name.unwrap_or_default();
        let database_password = (*database_password).clone();
        let selected_file = selected_file.clone();
        let error_message = error_message.clone();
        let info_message = info_message.clone();
        let is_loading = is_loading.clone();

        Callback::from(move |_| {
            let selected_file = (*selected_file).clone();
            let info_call = info_message.clone();
            let error_call = error_message.clone();
            if selected_file.is_none() {
                error_message.set(Some("Please select a backup file".into()));
                return;
            }

            is_loading.set(true);
            let form_data = FormData::new().unwrap();
            form_data
                .append_with_str("database_pass", &database_password)
                .unwrap();
            form_data
                .append_with_blob("backup_file", &selected_file.unwrap())
                .unwrap();

            // Clone values for async block
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let history = history.clone();
            let is_loading = is_loading.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match call_restore_server(&server_name, form_data, &api_key.unwrap()).await {
                    Ok(message) => {
                        info_call.set(Some(message));
                        history.push("/sign_out");
                    }
                    Err(e) => {
                        error_call.set(Some(e.to_string()));
                        is_loading.set(false);
                    }
                }
            });
        })
    };

    html! {
        <div class="p-4">
            <p class="item_container-text text-lg font-bold mb-4">{"Restore Server:"}</p>
            <p class="item_container-text text-md mb-4 text-red-600 font-bold">
                {"WARNING: This will delete everything on your server and restore to the point that the backup contains."}
            </p>
            <p class="item_container-text text-md mb-4">
                {"Upload a backup file (.sql) and provide your database password to restore your server, including all settings, users, and data."}
            </p>

            <div class="space-y-4">
                <div class="flex flex-col space-y-2">
                    <label for="backup_file" class="item_container-text">{"Backup File (.sql)"}</label>
                    <input
                        type="file"
                        id="backup_file"
                        accept=".sql"
                        disabled={*is_loading}
                        onchange={on_file_change}
                        class="block w-full text-sm file:mr-4 file:py-2 file:px-4 file:rounded-md file:border-0 file:text-sm file:font-semibold file:settings-button hover:file:bg-blue-600"
                    />
                </div>

                <div class="flex flex-col space-y-2">
                    <label for="db_pw" class="item_container-text">{"Database Password"}</label>
                    <div class="flex space-x-4">
                        <input
                            type="password"
                            id="db_pw"
                            disabled={*is_loading}
                            oninput={Callback::from(move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                database_password.set(input.value());
                            })}
                            class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                            placeholder="Database password"
                        />
                        <button
                            onclick={onclick_restore}
                            disabled={*is_loading}
                            class={classes!(
                                "settings-button",
                                "font-bold",
                                "py-2",
                                "px-6",
                                "rounded",
                                "focus:outline-none",
                                "focus:shadow-outline",
                                "inline-flex",
                                "items-center",
                                if *is_loading { "opacity-50 cursor-not-allowed" } else { "" }
                            )}
                        >
                            if *is_loading {
                                <div class="inline-flex items-center">
                                    <svg class="animate-spin -ml-1 mr-3 h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                    </svg>
                                    {"Restoring..."}
                                </div>
                            } else {
                                {"Restore Server"}
                            }
                        </button>
                    </div>
                </div>
            </div>

            if let Some(error) = &*error_message {
                <div class="mt-4 p-4 bg-red-50 dark:bg-red-900/20 rounded-lg text-red-900 dark:text-red-200">
                    {error}
                </div>
            }

            if let Some(info) = &*info_message {
                <div class="mt-4 p-4 bg-green-50 dark:bg-green-900/20 rounded-lg text-green-900 dark:text-green-200">
                    {info}
                </div>
            }

            if *is_loading {
                <div class="mt-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
                    <div class="flex items-center space-x-3">
                        <div class="flex-shrink-0">
                            <svg class="animate-spin h-5 w-5 text-blue-600 dark:text-blue-400" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                        </div>
                        <div class="flex-1 min-w-0">
                            <p class="text-sm font-medium text-blue-900 dark:text-blue-100">
                                {"Restoring database..."}
                            </p>
                            <p class="text-sm text-blue-700 dark:text-blue-300">
                                {"This may take several minutes. Please don't close this window."}
                            </p>
                        </div>
                    </div>
                </div>
            }
        </div>
    }
}
