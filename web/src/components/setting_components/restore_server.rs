use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_list_backup_files, call_restore_backup_file, call_restore_server,
};
use i18nrs::yew::use_translation;
use wasm_bindgen::JsValue;
use web_sys::{Event, File, FormData, HtmlInputElement};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[function_component(RestoreServer)]
pub fn restore_server() -> Html {
    let (i18n, _) = use_translation();

    // Capture i18n strings before they get moved
    let i18n_failed_to_load_backup_files = i18n
        .t("restore_server.failed_to_load_backup_files")
        .to_string();
    let i18n_file_size_too_large = i18n.t("restore_server.file_size_too_large").to_string();
    let database_password = use_state(|| "".to_string());
    let selected_file = use_state(|| None::<File>);
    let is_loading = use_state(|| false);

    // New state for backup file selection
    let restore_mode = use_state(|| "upload".to_string()); // "upload" or "select"
    let backup_files = use_state(|| Vec::<serde_json::Value>::new());
    let selected_backup_file = use_state(|| None::<String>);
    let files_loading = use_state(|| false);

    let (state, dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID);

    // Load backup files when switching to select mode
    {
        let backup_files = backup_files.clone();
        let files_loading = files_loading.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let dispatch = dispatch.clone();
        let restore_mode = restore_mode.clone();

        use_effect_with(
            (
                api_key.clone(),
                server_name.clone(),
                (*restore_mode).clone(),
            ),
            move |(api_key, server_name, mode)| {
                if mode == "select" {
                    if let (Some(api_key), Some(server_name), Some(user_id)) =
                        (api_key.clone(), server_name.clone(), user_id)
                    {
                        files_loading.set(true);
                        wasm_bindgen_futures::spawn_local(async move {
                            match call_list_backup_files(&server_name, &api_key.unwrap(), user_id)
                                .await
                            {
                                Ok(response) => {
                                    if let Some(files) =
                                        response.get("backup_files").and_then(|f| f.as_array())
                                    {
                                        let file_list: Vec<serde_json::Value> = files.clone();
                                        backup_files.set(file_list);
                                    }
                                }
                                Err(e) => {
                                    dispatch.reduce_mut(|state| {
                                        state.error_message = Some(format!(
                                            "{}{}",
                                            i18n_failed_to_load_backup_files.clone(),
                                            format_error_message(&e.to_string())
                                        ));
                                    });
                                }
                            }
                            files_loading.set(false);
                        });
                    }
                }
                || ()
            },
        );
    }

    let on_file_change = {
        let selected_file = selected_file.clone();
        let dispatch = dispatch.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    // Check file size (e.g., limit to 100MB)
                    if file.size() > 100.0 * 1024.0 * 1024.0 {
                        dispatch.reduce_mut(|state| {
                            state.error_message = Some(i18n_file_size_too_large.clone());
                        });
                        return;
                    }
                    selected_file.set(Some(file));
                }
            }
        })
    };

    let onclick_restore = {
        let history = BrowserHistory::new();
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let database_password = (*database_password).clone();
        let selected_file = selected_file.clone();
        let dispatch = dispatch.clone();
        let is_loading = is_loading.clone();

        Callback::from(move |_| {
            let selected_file = (*selected_file).clone();
            let dispatch = dispatch.clone();

            // Validate inputs
            if selected_file.is_none() {
                dispatch.reduce_mut(|state| {
                    state.error_message = Some("Please select a backup file".to_string());
                });
                return;
            }

            if database_password.trim().is_empty() {
                dispatch.reduce_mut(|state| {
                    state.error_message = Some("Database password is required".to_string());
                });
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
                        dispatch.reduce_mut(|state| {
                            state.info_message =
                                Some(format!("Server restore started successfully: {}", message));
                        });
                        history.push("/sign_out");
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        dispatch.reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Failed to restore server: {}", formatted_error));
                        });
                        is_loading.set(false);
                    }
                }
            });
        })
    };

    let onclick_restore_from_file = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let selected_backup_file = selected_backup_file.clone();
        let dispatch = dispatch.clone();
        let is_loading = is_loading.clone();
        let history = BrowserHistory::new();

        Callback::from(move |_: MouseEvent| {
            if let (Some(api_key), Some(server_name), Some(user_id), Some(filename)) = (
                api_key.clone(),
                server_name.clone(),
                user_id,
                (*selected_backup_file).clone(),
            ) {
                is_loading.set(true);
                let history = history.clone();
                let dispatch = dispatch.clone();
                let is_loading = is_loading.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_restore_backup_file(
                        &server_name,
                        &api_key.unwrap(),
                        user_id,
                        &filename,
                    )
                    .await
                    {
                        Ok(_) => {
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some(
                                    "Restore from backup file started successfully".to_string(),
                                );
                            });
                            history.push("/sign_out");
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "Failed to restore from backup file: {}",
                                    format_error_message(&e.to_string())
                                ));
                            });
                            is_loading.set(false);
                        }
                    }
                });
            } else {
                dispatch.reduce_mut(|state| {
                    state.error_message = Some("Please select a backup file".to_string());
                });
            }
        })
    };

    html! {
        <div class="p-4">
            <p class="item_container-text text-lg font-bold mb-4">{"Restore Server:"}</p>
            <p class="item_container-text text-md mb-4 text-red-600 font-bold">
                {"WARNING: This will delete everything on your server and restore to the point that the backup contains."}
            </p>

            // Mode Selection
            <div class="mb-6">
                <label class="block text-sm font-medium item_container-text mb-3">{"Restore Method"}</label>
                <div class="flex space-x-4">
                    <label class="flex items-center">
                        <input
                            type="radio"
                            name="restore_mode"
                            value="upload"
                            checked={*restore_mode == "upload"}
                            onchange={Callback::from({
                                let restore_mode = restore_mode.clone();
                                move |_| restore_mode.set("upload".to_string())
                            })}
                            class="mr-2"
                        />
                        <span class="item_container-text">{"Upload Backup File"}</span>
                    </label>
                    <label class="flex items-center">
                        <input
                            type="radio"
                            name="restore_mode"
                            value="select"
                            checked={*restore_mode == "select"}
                            onchange={Callback::from({
                                let restore_mode = restore_mode.clone();
                                move |_| restore_mode.set("select".to_string())
                            })}
                            class="mr-2"
                        />
                        <span class="item_container-text">{"Select from Server Backups"}</span>
                    </label>
                </div>
            </div>

            {
                if *restore_mode == "upload" {
                    html! {
                        <div class="space-y-4">
                            <p class="item_container-text text-md mb-4">
                                {"Upload a backup file (.sql) and provide your database password to restore your server."}
                            </p>
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
                    <div class="flex flex-col space-y-4 sm:flex-row sm:space-y-0 sm:space-x-4">
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
                                "justify-center",
                                "min-w-[140px]",
                                "whitespace-nowrap",
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
                    }
                } else {
                    html! {
                        <div class="space-y-4">
                            <p class="item_container-text text-md mb-4">
                                {"Choose from existing backup files in the mounted backup directory."}
                            </p>

                            {
                                if *files_loading {
                                    html! {
                                        <div class="loading-state p-4 text-center">
                                            <i class="ph ph-spinner animate-spin text-2xl mb-2"></i>
                                            <p class="text-sm item_container-text">{"Loading backup files..."}</p>
                                        </div>
                                    }
                                } else if backup_files.is_empty() {
                                    html! {
                                        <div class="empty-state p-4 text-center bg-yellow-50 dark:bg-yellow-900/20 rounded-lg">
                                            <i class="ph ph-folder-open text-3xl text-yellow-600 mb-2"></i>
                                            <p class="text-sm text-yellow-800 dark:text-yellow-200">
                                                {"No backup files found in the backup directory."}
                                            </p>
                                        </div>
                                    }
                                } else {
                                    html! {
                                        <div class="backup-files-list space-y-2 max-h-60 overflow-y-auto">
                                            {for backup_files.iter().map(|file| {
                                                let filename = file.get("filename").and_then(|f| f.as_str()).unwrap_or("Unknown");
                                                let size = file.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                                let modified = file.get("modified").and_then(|m| m.as_u64()).unwrap_or(0);
                                                let is_selected = selected_backup_file.as_ref() == Some(&filename.to_string());

                                                let filename_clone = filename.to_string();
                                                let selected_backup_file_clone = selected_backup_file.clone();

                                                html! {
                                                    <div
                                                        class={if is_selected {
                                                            "backup-file-item p-3 border rounded-lg cursor-pointer transition-colors border-blue-500 bg-blue-50 dark:bg-blue-900/20"
                                                        } else {
                                                            "backup-file-item p-3 border rounded-lg cursor-pointer transition-colors border-gray-200 hover:bg-gray-50 dark:border-gray-700 dark:hover:bg-gray-800"
                                                        }}
                                                        onclick={Callback::from(move |_: MouseEvent| {
                                                            selected_backup_file_clone.set(Some(filename_clone.clone()));
                                                        })}
                                                    >
                                                        <div class="flex items-center justify-between">
                                                            <div class="flex items-center gap-3">
                                                                <i class={if is_selected {
                                                                    "ph ph-file-sql text-lg text-blue-600"
                                                                } else {
                                                                    "ph ph-file-sql text-lg text-gray-500"
                                                                }}></i>
                                                                <div>
                                                                    <p class={if is_selected {
                                                                        "text-sm font-medium text-blue-900 dark:text-blue-100"
                                                                    } else {
                                                                        "text-sm font-medium item_container-text"
                                                                    }}>
                                                                        {filename}
                                                                    </p>
                                                                    <p class="text-xs text-gray-500">
                                                                        {format!("{:.1} MB", size as f64 / 1024.0 / 1024.0)}
                                                                        {" • "}
                                                                        {if modified > 0 {
                                                                            let timestamp_ms = (modified as f64) * 1000.0;
                                                                            let date = js_sys::Date::new(&JsValue::from(timestamp_ms));
                                                                            date.to_locale_string("en-US", &JsValue::UNDEFINED).as_string().unwrap_or_default()
                                                                        } else {
                                                                            "Unknown date".to_string()
                                                                        }}
                                                                    </p>
                                                                </div>
                                                            </div>
                                                            {
                                                                if is_selected {
                                                                    html! { <i class="ph ph-check-circle text-blue-600"></i> }
                                                                } else {
                                                                    html! { <></> }
                                                                }
                                                            }
                                                        </div>
                                                    </div>
                                                }
                                            })}
                                        </div>
                                    }
                                }
                            }

                            {
                                if selected_backup_file.is_some() {
                                    html! {
                                        <div class="mt-4">
                                            <button
                                                onclick={onclick_restore_from_file}
                                                disabled={*is_loading}
                                                class={if *is_loading {
                                                    "settings-button font-bold py-3 px-6 rounded focus:outline-none focus:shadow-outline inline-flex items-center justify-center min-w-[160px] bg-red-600 hover:bg-red-700 text-white opacity-50 cursor-not-allowed"
                                                } else {
                                                    "settings-button font-bold py-3 px-6 rounded focus:outline-none focus:shadow-outline inline-flex items-center justify-center min-w-[160px] bg-red-600 hover:bg-red-700 text-white"
                                                }}
                                            >
                                                {
                                                    if *is_loading {
                                                        html! {
                                                            <div class="inline-flex items-center">
                                                                <svg class="animate-spin -ml-1 mr-3 h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 818-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 714 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                                                </svg>
                                                                {"Restoring..."}
                                                            </div>
                                                        }
                                                    } else {
                                                        html! { {"Restore from Selected File"} }
                                                    }
                                                }
                                            </button>
                                        </div>
                                    }
                                } else {
                                    html! { <></> }
                                }
                            }
                        </div>
                    }
                }
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
