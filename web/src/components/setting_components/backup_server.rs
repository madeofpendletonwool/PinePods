use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{call_backup_server, call_schedule_backup, call_get_scheduled_backup, call_manual_backup_to_directory};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{window, Blob, BlobPropertyBag, Url, HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[function_component(BackupServer)]
pub fn backup_server() -> Html {
    let (i18n, _) = use_translation();
    let database_password = use_state(|| "".to_string());
    let is_loading = use_state(|| false);
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID);
    let blob_property_bag = BlobPropertyBag::new();

    // Scheduled backup states
    let schedule_enabled = use_state(|| false);
    let cron_schedule = use_state(|| "0 0 2 * * *".to_string()); // Default: daily at 2 AM
    let schedule_loading = use_state(|| false);
    let current_schedule = use_state(|| None::<serde_json::Value>);

    // Load current schedule on mount
    {
        let current_schedule = current_schedule.clone();
        let schedule_enabled = schedule_enabled.clone();
        let cron_schedule = cron_schedule.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let _dispatch = _dispatch.clone();
        // Capture error message before move
        let error_prefix = i18n.t("backup_server.failed_to_load_backup_schedule").to_string();

        use_effect_with((), move |_| {
                if let (Some(api_key), Some(server_name), Some(user_id)) = (api_key.clone(), server_name.clone(), user_id) {
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_get_scheduled_backup(&server_name, &api_key.unwrap(), user_id).await {
                            Ok(schedule_info) => {
                                current_schedule.set(Some(schedule_info.clone()));
                                if let Some(enabled) = schedule_info.get("enabled").and_then(|v| v.as_bool()) {
                                    schedule_enabled.set(enabled);
                                }
                                if let Some(schedule) = schedule_info.get("schedule").and_then(|v| v.as_str()) {
                                    cron_schedule.set(schedule.to_string());
                                }
                            }
                            Err(e) => {
                                _dispatch.reduce_mut(|state| {
                                    state.error_message = Some(format!("{}{}", error_prefix, format_error_message(&e.to_string())));
                                });
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    let on_download_click = {
        let database_password = database_password.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let blob_property_bag = blob_property_bag.clone();
        let dispatch_call = _dispatch.clone();
        let is_loading = is_loading.clone();
        // Capture error messages before move
        let empty_password_msg = i18n.t("backup_server.database_password_empty").to_string();
        let backup_error_prefix = i18n.t("backup_server.backup_error").to_string();

        Callback::from(move |_| {
            let empty_password_msg = empty_password_msg.clone();
            let backup_error_prefix = backup_error_prefix.clone();
            let dispatch_call = dispatch_call.clone();
            let db_pass = (*database_password).trim().to_string();
            if db_pass.is_empty() {
                dispatch_call.reduce_mut(|audio_state| {
                    audio_state.error_message = Option::from(empty_password_msg.clone())
                });
                return;
            }

            is_loading.set(true);
            let api_key = api_key.clone().unwrap_or_default();
            let server_name = server_name.clone().unwrap_or_default();
            let bloberty_bag = blob_property_bag.clone();
            let is_loading_clone = is_loading.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match call_backup_server(&server_name, &db_pass, &api_key.unwrap()).await {
                    Ok(backup_data) => {
                        let array = js_sys::Array::new();
                        array.push(&JsValue::from_str(&backup_data));

                        let blob =
                            Blob::new_with_str_sequence_and_options(&array, &bloberty_bag).unwrap();
                        let url = Url::create_object_url_with_blob(&blob).unwrap();

                        if let Some(window) = window() {
                            let document = window.document().unwrap();
                            let a = document
                                .create_element("a")
                                .unwrap()
                                .dyn_into::<web_sys::HtmlAnchorElement>()
                                .unwrap();
                            a.set_href(&url);
                            a.set_download("server_backup.sql");
                            a.click();

                            Url::revoke_object_url(&url).unwrap();
                        }
                        is_loading_clone.set(false);
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        dispatch_call.reduce_mut(|audio_state| {
                            audio_state.error_message = Option::from(
                                format!("{}{}", backup_error_prefix.clone(), formatted_error)
                            )
                        });
                        is_loading_clone.set(false);
                    }
                }
            });
        })
    };

    // Schedule handlers
    let on_schedule_time_change = {
        let cron_schedule = cron_schedule.clone();
        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            cron_schedule.set(select.value());
        })
    };

    let on_schedule_toggle = {
        let schedule_enabled = schedule_enabled.clone();
        let cron_schedule = cron_schedule.clone();
        let schedule_loading = schedule_loading.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let _dispatch = _dispatch.clone();
        // Capture translated messages before move
        let enabled_msg = i18n.t("backup_server.scheduled_backup_enabled").to_string();
        let disabled_msg = i18n.t("backup_server.scheduled_backup_disabled").to_string();
        let error_prefix = i18n.t("backup_server.failed_to_update_backup_schedule").to_string();
        
        Callback::from(move |_| {
            let enabled_msg = enabled_msg.clone();
            let disabled_msg = disabled_msg.clone();
            let error_prefix = error_prefix.clone();
            if let (Some(api_key), Some(server_name), Some(user_id)) = (api_key.clone(), server_name.clone(), user_id) {
                schedule_loading.set(true);
                let new_enabled = !*schedule_enabled;
                schedule_enabled.set(new_enabled);
                
                let schedule = (*cron_schedule).clone();
                let schedule_loading = schedule_loading.clone();
                let schedule_enabled_for_async = schedule_enabled.clone();
                let _dispatch = _dispatch.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_schedule_backup(&server_name, &api_key.unwrap(), user_id, &schedule, new_enabled).await {
                        Ok(_) => {
                            _dispatch.reduce_mut(|state| {
                                state.info_message = Some(
                                    if new_enabled { enabled_msg.clone() } else { disabled_msg.clone() }
                                );
                            });
                        }
                        Err(e) => {
                            schedule_enabled_for_async.set(!new_enabled); // Revert on error
                            _dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("{}{}", error_prefix.clone(), format_error_message(&e.to_string())));
                            });
                        }
                    }
                    schedule_loading.set(false);
                });
            }
        })
    };

    let on_manual_backup_to_directory = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let _dispatch = _dispatch.clone();
        let is_loading = is_loading.clone();
        // Capture translated messages before move
        let success_template = i18n.t("backup_server.manual_backup_started").to_string();
        let error_prefix = i18n.t("backup_server.failed_to_start_manual_backup").to_string();
        
        Callback::from(move |_: MouseEvent| {
            let success_template = success_template.clone();
            let error_prefix = error_prefix.clone();
            if let (Some(api_key), Some(server_name), Some(user_id)) = (api_key.clone(), server_name.clone(), user_id) {
                is_loading.set(true);
                let _dispatch = _dispatch.clone();
                let is_loading = is_loading.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_manual_backup_to_directory(&server_name, &api_key.unwrap(), user_id).await {
                        Ok(response) => {
                            let filename = response.get("filename").and_then(|f| f.as_str()).unwrap_or("backup file");
                            _dispatch.reduce_mut(|state| {
                                state.info_message = Some(success_template.clone().replace("{}", filename));
                            });
                        }
                        Err(e) => {
                            _dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("{}{}", error_prefix.clone(), format_error_message(&e.to_string())));
                            });
                        }
                    }
                    is_loading.set(false);
                });
            }
        })
    };

    html! {
        <div class="p-4 space-y-8">
            <div class="backup-section">
                <div class="flex items-center gap-3 mb-4">
                    <i class="ph ph-clock-clockwise text-2xl text-blue-600"></i>
                    <h2 class="item_container-text text-lg font-bold">{i18n.t("backup_server.scheduled_backups")}</h2>
                </div>
                <p class="item_container-text text-md mb-4">
                    {i18n.t("backup_server.scheduled_backups_description")}
                </p>
                
                <div class="bg-gray-50 dark:bg-gray-800 rounded-lg p-4 space-y-4">
                    <div class="grid grid-cols-1 md:grid-cols-2 gap-4">
                        <div>
                            <label class="block text-sm font-medium item_container-text mb-2">
                                {i18n.t("backup_server.schedule_time")}
                            </label>
                            <select
                                value={(*cron_schedule).clone()}
                                onchange={on_schedule_time_change}
                                disabled={*schedule_loading}
                                class="w-full p-2 border rounded-lg search-bar-input"
                                key={(*cron_schedule).clone()}
                            >
                                <option value="0 0 2 * * *" selected={*cron_schedule == "0 0 2 * * *"}>{i18n.t("backup_server.daily_2am")}</option>
                                <option value="0 0 3 * * 0" selected={*cron_schedule == "0 0 3 * * 0"}>{i18n.t("backup_server.weekly_sunday_3am")}</option>
                                <option value="0 0 1 1 * *" selected={*cron_schedule == "0 0 1 1 * *"}>{i18n.t("backup_server.monthly_1st_1am")}</option>
                                <option value="0 0 */6 * * *" selected={*cron_schedule == "0 0 */6 * * *"}>{i18n.t("backup_server.every_6_hours")}</option>
                                <option value="0 0 */12 * * *" selected={*cron_schedule == "0 0 */12 * * *"}>{i18n.t("backup_server.every_12_hours")}</option>
                            </select>
                        </div>
                        
                        <div>
                            <label class="block text-sm font-medium item_container-text mb-2">
                                {i18n.t("backup_server.status")}
                            </label>
                            <label class="relative inline-flex items-center cursor-pointer">
                                <input 
                                    type="checkbox" 
                                    checked={*schedule_enabled} 
                                    disabled={*schedule_loading}
                                    onclick={on_schedule_toggle}
                                    class="sr-only peer" 
                                />
                                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                <span class="ms-3 text-sm font-medium item_container-text">
                                    {if *schedule_enabled { i18n.t("backup_server.enabled") } else { i18n.t("backup_server.disabled") }}
                                </span>
                            </label>
                        </div>
                    </div>
                    
                    if let Some(schedule_info) = &*current_schedule {
                        if let Some(updated_at) = schedule_info.get("updated_at").and_then(|v| v.as_str()) {
                            <div class="mt-4 text-sm text-gray-600 dark:text-gray-400">
                                <i class="ph ph-info mr-1"></i>
                                {format!("{}{}", i18n.t("backup_server.last_updated"), updated_at)}
                            </div>
                        }
                    }
                </div>
            </div>

            <div class="backup-section">
                <div class="flex items-center gap-3 mb-4">
                    <i class="ph ph-download-simple text-2xl text-green-600"></i>
                    <h2 class="item_container-text text-lg font-bold">{i18n.t("backup_server.manual_backup")}</h2>
                </div>
                <p class="item_container-text text-md mb-4">{i18n.t("backup_server.manual_backup_description")}</p>
            
            <div class="space-y-4">
                // Download backup section
                <div class="backup-option">
                    <h4 class="item_container-text font-semibold mb-2">{i18n.t("backup_server.download_backup_file")}</h4>
                    <p class="item_container-text text-sm mb-3">{i18n.t("backup_server.download_backup_description")}</p>
                    <div class="flex flex-col space-y-4 sm:flex-row sm:space-y-0 sm:space-x-4">
                <input
                    type="password"
                    id="db-pw"
                    disabled={*is_loading}
                    oninput={Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        database_password.set(input.value());
                    })}
                    class={classes!(
                        "search-bar-input",
                        "border",
                        "text-sm",
                        "rounded-lg",
                        "block",
                        "w-full",
                        "p-2.5",
                        if *is_loading { "opacity-50 cursor-not-allowed" } else { "" }
                    )}
                    placeholder="mYDBp@ss!"
                />
                <button
                    onclick={on_download_click}
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
                        "min-w-[120px]",
                        if *is_loading { "opacity-75 cursor-not-allowed" } else { "" }
                    )}
                >
                    if *is_loading {
                        <div class="inline-flex items-center">
                            <svg class="animate-spin -ml-1 mr-3 h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                            {i18n.t("backup_server.exporting")}
                        </div>
                    } else {
                        {i18n.t("backup_server.authenticate")}
                    }
                </button>
                    </div>
                </div>
                
                // Backup to directory section
                <div class="backup-option">
                    <h4 class="item_container-text font-semibold mb-2">{i18n.t("backup_server.save_to_backup_directory")}</h4>
                    <p class="item_container-text text-sm mb-3">{i18n.t("backup_server.save_to_backup_description")}</p>
                    <button
                        onclick={on_manual_backup_to_directory}
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
                            "min-w-[120px]",
                            if *is_loading { "opacity-75 cursor-not-allowed" } else { "" }
                        )}
                    >
                        if *is_loading {
                            <div class="inline-flex items-center">
                                <svg class="animate-spin -ml-1 mr-3 h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                    <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                    <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 818-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 714 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                                </svg>
                                {i18n.t("backup_server.creating")}
                            </div>
                        } else {
                            {i18n.t("backup_server.create_backup")}
                        }
                    </button>
                </div>
            </div>

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
                                {i18n.t("backup_server.backing_up_database")}
                            </p>
                            <p class="text-sm text-blue-700 dark:text-blue-300">
                                {i18n.t("backup_server.backup_loading_message")}
                            </p>
                        </div>
                    </div>
                </div>
            }
            </div>
        </div>
    }
}
