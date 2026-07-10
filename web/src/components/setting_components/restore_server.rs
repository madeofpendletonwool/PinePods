use crate::components::context::{AppState, NotificationState};
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
    let i18n_warning = i18n.t("restore_server.warning").to_string();
    let i18n_restore_method = i18n.t("restore_server.restore_method").to_string();
    let i18n_upload_backup_file = i18n.t("restore_server.upload_backup_file").to_string();
    let i18n_select_from_server_backups = i18n.t("restore_server.select_from_server_backups").to_string();
    let i18n_backup_file_sql = i18n.t("restore_server.backup_file_sql").to_string();
    let i18n_upload_backup_hint = i18n.t("restore_server.upload_backup_hint").to_string();
    let i18n_choose_file = i18n.t("restore_server.choose_file").to_string();
    let i18n_file_selected = i18n.t("restore_server.file_selected").to_string();
    let i18n_database_password = i18n.t("restore_server.database_password").to_string();
    let i18n_restore_server = i18n.t("restore_server.restore_server").to_string();
    let i18n_server_backup_files = i18n.t("restore_server.server_backup_files").to_string();
    let i18n_server_backups_hint = i18n.t("restore_server.server_backups_hint").to_string();
    let i18n_loading_backup_files = i18n.t("restore_server.loading_backup_files").to_string();
    let i18n_no_backup_files = i18n.t("restore_server.no_backup_files").to_string();
    let i18n_selected_file_warning = i18n.t("restore_server.selected_file_warning").to_string();
    let i18n_restore_from_selected = i18n.t("restore_server.restore_from_selected").to_string();
    let i18n_restoring_database = i18n.t("restore_server.restoring_database").to_string();
    let i18n_dont_close_window = i18n.t("restore_server.dont_close_window").to_string();
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
        let _dispatch = dispatch.clone();
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
                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
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
        let _dispatch = dispatch.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = input.files() {
                if let Some(file) = files.get(0) {
                    // Uploads are streamed server-side, so there is no client size cap.
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
            let _dispatch = dispatch.clone();

            // Validate inputs
            if selected_file.is_none() {
                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                    state.error_message = Some("Please select a backup file".to_string());
                });
                return;
            }

            if database_password.trim().is_empty() {
                Dispatch::<NotificationState>::global().reduce_mut(|state| {
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
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message =
                                Some(format!("Server restore started successfully: {}", message));
                        });
                        // Persist a flag so RestoreOverlay knows to poll/show. The restore
                        // truncates the Sessions table and we redirect to /sign_out, so
                        // in-memory state is wiped -- localStorage survives both.
                        if let Some(window) = web_sys::window() {
                            if let Ok(Some(storage)) = window.local_storage() {
                                let _ = storage.set_item("pinepods_restore_active", "1");
                                let _ = storage.set_item("pinepods_restore_server", &server_name);
                            }
                        }
                        history.push("/sign_out");
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
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
                let _dispatch = dispatch.clone();
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
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Some(
                                    "Restore from backup file started successfully".to_string(),
                                );
                            });
                            // Persist a flag so RestoreOverlay knows to poll/show. The restore
                            // truncates the Sessions table and we redirect to /sign_out, so
                            // in-memory state is wiped -- localStorage survives both.
                            if let Some(window) = web_sys::window() {
                                if let Ok(Some(storage)) = window.local_storage() {
                                    let _ = storage.set_item("pinepods_restore_active", "1");
                                    let _ =
                                        storage.set_item("pinepods_restore_server", &server_name);
                                }
                            }
                            history.push("/sign_out");
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
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
                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                    state.error_message = Some("Please select a backup file".to_string());
                });
            }
        })
    };

    html! {
        <>
            <div class="settings-row">
                <div class="settings-row-label">
                    <div style="color:var(--error-color,#ef4444);font-weight:600;font-size:13px;">
                        { &i18n_warning }
                    </div>
                </div>
            </div>

            <div class="settings-row" style="flex-wrap: wrap;">
                <div class="settings-row-label">
                    <div>{ &i18n_restore_method }</div>
                </div>
                <div class="settings-row-control" style="flex: 1 1 200px; min-width: 0;">
                    <label style="display:flex;align-items:center;gap:6px;cursor:pointer;font-size:13px;color:var(--text-color);">
                        <input
                            type="radio"
                            name="restore_mode"
                            value="upload"
                            checked={*restore_mode == "upload"}
                            onchange={Callback::from({
                                let restore_mode = restore_mode.clone();
                                move |_| restore_mode.set("upload".to_string())
                            })}
                        />
                        { &i18n_upload_backup_file }
                    </label>
                    <label style="display:flex;align-items:center;gap:6px;cursor:pointer;font-size:13px;color:var(--text-color);">
                        <input
                            type="radio"
                            name="restore_mode"
                            value="select"
                            checked={*restore_mode == "select"}
                            onchange={Callback::from({
                                let restore_mode = restore_mode.clone();
                                move |_| restore_mode.set("select".to_string())
                            })}
                        />
                        { &i18n_select_from_server_backups }
                    </label>
                </div>
            </div>

            {
                if *restore_mode == "upload" {
                    html! {
                        <>
                            <div class="settings-row">
                                <div class="settings-row-label">
                                    <div>{ &i18n_backup_file_sql }</div>
                                    <div class="settings-row-desc">{ &i18n_upload_backup_hint }</div>
                                </div>
                                <div class="settings-row-control">
                                    <label class="btn btn-secondary" style="padding:6px 12px;cursor:pointer;">
                                        <i class="ph ph-upload-simple"></i>
                                        <span>{ &i18n_choose_file }</span>
                                        <input
                                            type="file"
                                            id="backup_file"
                                            accept=".sql"
                                            disabled={*is_loading}
                                            onchange={on_file_change}
                                            style="display:none;"
                                        />
                                    </label>
                                    if selected_file.is_some() {
                                        <span class="settings-row-desc">{ &i18n_file_selected }</span>
                                    }
                                </div>
                            </div>

                            <div class="settings-row" style="flex-wrap: wrap;">
                                <div class="settings-row-label">
                                    <div>{ &i18n_database_password }</div>
                                </div>
                                <div class="settings-row-control" style="flex: 1 1 200px; min-width: 0;">
                                    <input
                                        type="password"
                                        id="db_pw"
                                        disabled={*is_loading}
                                        oninput={Callback::from(move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            database_password.set(input.value());
                                        })}
                                        class="input"
                                        placeholder="Database password"
                                        style="flex: 1; min-width: 0;"
                                    />
                                    <button
                                        onclick={onclick_restore}
                                        disabled={*is_loading}
                                        class="btn btn-danger"
                                        style="flex-shrink: 0;"
                                    >
                                        if *is_loading {
                                            <i class="ph ph-spinner animate-spin"></i>
                                            <span>{"Restoring..."}</span>
                                        } else {
                                            <i class="ph ph-arrow-counter-clockwise"></i>
                                            <span>{ &i18n_restore_server }</span>
                                        }
                                    </button>
                                </div>
                            </div>
                        </>
                    }
                } else {
                    html! {
                        <>
                            <div class="settings-row">
                                <div class="settings-row-label">
                                    <div>{ &i18n_server_backup_files }</div>
                                    <div class="settings-row-desc">{ &i18n_server_backups_hint }</div>
                                </div>
                            </div>

                            {
                                if *files_loading {
                                    html! {
                                        <div class="settings-row">
                                            <div class="settings-row-label">
                                                <div class="settings-row-desc" style="display:flex;align-items:center;gap:8px;">
                                                    <i class="ph ph-spinner animate-spin"></i>
                                                    <span>{ &i18n_loading_backup_files }</span>
                                                </div>
                                            </div>
                                        </div>
                                    }
                                } else if backup_files.is_empty() {
                                    html! {
                                        <div class="settings-row">
                                            <div class="settings-row-label">
                                                <div class="settings-row-desc">
                                                    <i class="ph ph-folder-open" style="margin-right:4px;"></i>
                                                    { &i18n_no_backup_files }
                                                </div>
                                            </div>
                                        </div>
                                    }
                                } else {
                                    html! {
                                        <div style="display:flex;flex-direction:column;gap:4px;max-height:240px;overflow-y:auto;padding:0 16px 8px;">
                                            {for backup_files.iter().map(|file| {
                                                let filename = file.get("filename").and_then(|f| f.as_str()).unwrap_or("Unknown");
                                                let size = file.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                                let modified = file.get("modified").and_then(|m| m.as_u64()).unwrap_or(0);
                                                let is_selected = selected_backup_file.as_ref() == Some(&filename.to_string());

                                                let filename_clone = filename.to_string();
                                                let selected_backup_file_clone = selected_backup_file.clone();

                                                let item_style = if is_selected {
                                                    "display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-radius:6px;cursor:pointer;border:1px solid var(--accent-color);background:rgba(128,128,128,0.08);"
                                                } else {
                                                    "display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-radius:6px;cursor:pointer;border:1px solid rgba(128,128,128,0.2);"
                                                };

                                                html! {
                                                    <div
                                                        style={item_style}
                                                        onclick={Callback::from(move |_: MouseEvent| {
                                                            selected_backup_file_clone.set(Some(filename_clone.clone()));
                                                        })}
                                                    >
                                                        <div style="display:flex;align-items:center;gap:10px;">
                                                            <i class="ph ph-file-sql" style={if is_selected { "color:var(--accent-color);font-size:18px;" } else { "color:var(--text-secondary-color);font-size:18px;" }}></i>
                                                            <div>
                                                                <div style="font-size:13px;color:var(--text-color);font-weight:500;">{filename}</div>
                                                                <div style="font-size:11px;color:var(--text-secondary-color);">
                                                                    {format!("{:.1} MB", size as f64 / 1024.0 / 1024.0)}
                                                                    {" • "}
                                                                    {if modified > 0 {
                                                                        let timestamp_ms = (modified as f64) * 1000.0;
                                                                        let date = js_sys::Date::new(&JsValue::from(timestamp_ms));
                                                                        date.to_locale_string("en-US", &JsValue::UNDEFINED).as_string().unwrap_or_default()
                                                                    } else {
                                                                        "Unknown date".to_string()
                                                                    }}
                                                                </div>
                                                            </div>
                                                        </div>
                                                        if is_selected {
                                                            <i class="ph ph-check-circle" style="color:var(--accent-color);"></i>
                                                        }
                                                    </div>
                                                }
                                            })}
                                        </div>
                                    }
                                }
                            }

                            if selected_backup_file.is_some() {
                                <div class="settings-row">
                                    <div class="settings-row-label">
                                        <div class="settings-row-desc">{ &i18n_selected_file_warning }</div>
                                    </div>
                                    <div class="settings-row-control">
                                        <button
                                            onclick={onclick_restore_from_file}
                                            disabled={*is_loading}
                                            class="btn btn-danger"
                                            style="padding:6px 12px;"
                                        >
                                            if *is_loading {
                                                <i class="ph ph-spinner animate-spin"></i>
                                                <span>{"Restoring..."}</span>
                                            } else {
                                                <i class="ph ph-arrow-counter-clockwise"></i>
                                                <span>{ &i18n_restore_from_selected }</span>
                                            }
                                        </button>
                                    </div>
                                </div>
                            }
                        </>
                    }
                }
            }

            if *is_loading {
                <div class="settings-row">
                    <div class="settings-row-label">
                        <div class="settings-row-desc" style="display:flex;align-items:center;gap:8px;">
                            <i class="ph ph-spinner animate-spin"></i>
                            <span>{ &i18n_restoring_database }</span>
                        </div>
                        <div class="settings-row-desc">{ &i18n_dont_close_window }</div>
                    </div>
                </div>
            }
        </>
    }
}
