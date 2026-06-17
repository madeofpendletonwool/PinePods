use crate::components::context::{AppState, NotificationState};
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_backup_server, call_delete_backup_file, call_get_scheduled_backup,
    call_list_backup_files, call_manual_backup_to_directory, call_schedule_backup,
};
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{window, Blob, BlobPropertyBag, HtmlSelectElement, Url};
use yew::prelude::*;
use yewdux::prelude::*;

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
    let retention_count = use_state(|| 0i32); // 0 = keep all

    // Existing backups management states
    let backup_files = use_state(|| Vec::<serde_json::Value>::new());
    let files_loading = use_state(|| false);
    let files_reload = use_state(|| 0u32); // bump to refetch the list

    // Load current schedule on mount
    {
        let current_schedule = current_schedule.clone();
        let schedule_enabled = schedule_enabled.clone();
        let cron_schedule = cron_schedule.clone();
        let retention_count = retention_count.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let _dispatch = _dispatch.clone();
        // Capture error message before move
        let error_prefix = i18n
            .t("backup_server.failed_to_load_backup_schedule")
            .to_string();

        use_effect_with((), move |_| {
            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_get_scheduled_backup(&server_name, &api_key.unwrap(), user_id).await
                    {
                        Ok(schedule_info) => {
                            current_schedule.set(Some(schedule_info.clone()));
                            if let Some(enabled) =
                                schedule_info.get("enabled").and_then(|v| v.as_bool())
                            {
                                schedule_enabled.set(enabled);
                            }
                            if let Some(schedule) =
                                schedule_info.get("schedule").and_then(|v| v.as_str())
                            {
                                cron_schedule.set(schedule.to_string());
                            }
                            if let Some(retention) =
                                schedule_info.get("retention_count").and_then(|v| v.as_i64())
                            {
                                retention_count.set(retention as i32);
                            }
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    error_prefix,
                                    format_error_message(&e.to_string())
                                ));
                            });
                        }
                    }
                });
            }
            || ()
        });
    }

    // Load the list of existing backup files (re-runs whenever files_reload bumps)
    {
        let backup_files = backup_files.clone();
        let files_loading = files_loading.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let error_prefix = i18n.t("backup_server.failed_to_load_backups").to_string();

        use_effect_with((*files_reload, api_key.clone(), server_name.clone()), move |_| {
            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                files_loading.set(true);
                wasm_bindgen_futures::spawn_local(async move {
                    match call_list_backup_files(&server_name, &api_key.unwrap(), user_id).await {
                        Ok(response) => {
                            if let Some(files) =
                                response.get("backup_files").and_then(|f| f.as_array())
                            {
                                backup_files.set(files.clone());
                            }
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    error_prefix,
                                    format_error_message(&e.to_string())
                                ));
                            });
                        }
                    }
                    files_loading.set(false);
                });
            }
            || ()
        });
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
            let _dispatch_call = dispatch_call.clone();
            let db_pass = (*database_password).trim().to_string();
            if db_pass.is_empty() {
                Dispatch::<NotificationState>::global().reduce_mut(|audio_state| {
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
                        Dispatch::<NotificationState>::global().reduce_mut(|audio_state| {
                            audio_state.error_message = Option::from(format!(
                                "{}{}",
                                backup_error_prefix.clone(),
                                formatted_error
                            ))
                        });
                        is_loading_clone.set(false);
                    }
                }
            });
        })
    };

    // Schedule handlers. Changing the time persists immediately (using current enabled +
    // retention) so the schedule actually takes effect without needing to toggle.
    let on_schedule_time_change = {
        let cron_schedule = cron_schedule.clone();
        let schedule_enabled = schedule_enabled.clone();
        let retention_count = retention_count.clone();
        let schedule_loading = schedule_loading.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let error_prefix = i18n
            .t("backup_server.failed_to_update_backup_schedule")
            .to_string();

        Callback::from(move |e: Event| {
            let select: HtmlSelectElement = e.target_unchecked_into();
            let new_schedule = select.value();
            cron_schedule.set(new_schedule.clone());

            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                let enabled = *schedule_enabled;
                let retention = *retention_count;
                let schedule_loading = schedule_loading.clone();
                let error_prefix = error_prefix.clone();
                schedule_loading.set(true);
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = call_schedule_backup(
                        &server_name,
                        &api_key.unwrap(),
                        user_id,
                        &new_schedule,
                        enabled,
                        if retention > 0 { Some(retention) } else { None },
                    )
                    .await
                    {
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!(
                                "{}{}",
                                error_prefix,
                                format_error_message(&e.to_string())
                            ));
                        });
                    }
                    schedule_loading.set(false);
                });
            }
        })
    };

    // Persist retention when the input loses focus / changes.
    let on_retention_change = {
        let cron_schedule = cron_schedule.clone();
        let schedule_enabled = schedule_enabled.clone();
        let retention_count = retention_count.clone();
        let schedule_loading = schedule_loading.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let error_prefix = i18n
            .t("backup_server.failed_to_update_backup_schedule")
            .to_string();

        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let new_retention = input.value().trim().parse::<i32>().unwrap_or(0).max(0);
            retention_count.set(new_retention);

            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                let enabled = *schedule_enabled;
                let schedule = (*cron_schedule).clone();
                let schedule_loading = schedule_loading.clone();
                let error_prefix = error_prefix.clone();
                schedule_loading.set(true);
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = call_schedule_backup(
                        &server_name,
                        &api_key.unwrap(),
                        user_id,
                        &schedule,
                        enabled,
                        if new_retention > 0 { Some(new_retention) } else { None },
                    )
                    .await
                    {
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!(
                                "{}{}",
                                error_prefix,
                                format_error_message(&e.to_string())
                            ));
                        });
                    }
                    schedule_loading.set(false);
                });
            }
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
        let disabled_msg = i18n
            .t("backup_server.scheduled_backup_disabled")
            .to_string();
        let error_prefix = i18n
            .t("backup_server.failed_to_update_backup_schedule")
            .to_string();
        let retention_count = retention_count.clone();

        Callback::from(move |_| {
            let enabled_msg = enabled_msg.clone();
            let disabled_msg = disabled_msg.clone();
            let error_prefix = error_prefix.clone();
            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                schedule_loading.set(true);
                let new_enabled = !*schedule_enabled;
                schedule_enabled.set(new_enabled);

                let schedule = (*cron_schedule).clone();
                let retention = *retention_count;
                let schedule_loading = schedule_loading.clone();
                let schedule_enabled_for_async = schedule_enabled.clone();
                let _dispatch = _dispatch.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_schedule_backup(
                        &server_name,
                        &api_key.unwrap(),
                        user_id,
                        &schedule,
                        new_enabled,
                        if retention > 0 { Some(retention) } else { None },
                    )
                    .await
                    {
                        Ok(_) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Some(if new_enabled {
                                    enabled_msg.clone()
                                } else {
                                    disabled_msg.clone()
                                });
                            });
                        }
                        Err(e) => {
                            schedule_enabled_for_async.set(!new_enabled); // Revert on error
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    error_prefix.clone(),
                                    format_error_message(&e.to_string())
                                ));
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
        let backup_files = backup_files.clone();
        // Capture translated messages before move
        let success_template = i18n.t("backup_server.manual_backup_started").to_string();
        let error_prefix = i18n
            .t("backup_server.failed_to_start_manual_backup")
            .to_string();

        Callback::from(move |_: MouseEvent| {
            let success_template = success_template.clone();
            let error_prefix = error_prefix.clone();
            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                let api_key = match api_key {
                    Some(key) => key,
                    None => return,
                };
                is_loading.set(true);
                let _dispatch = _dispatch.clone();
                let is_loading = is_loading.clone();
                let backup_files = backup_files.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_manual_backup_to_directory(&server_name, &api_key, user_id).await {
                        Ok(response) => {
                            let filename = response
                                .get("filename")
                                .and_then(|f| f.as_str())
                                .unwrap_or("backup file")
                                .to_string();
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message =
                                    Some(success_template.clone().replace("{}", &filename));
                            });

                            // The backup runs as a background task, so the .sql file isn't ready
                            // when the request returns. Poll the list until the new file appears,
                            // refreshing the displayed list as it does, then settle its final size.
                            for _ in 0..60 {
                                gloo_timers::future::TimeoutFuture::new(3_000).await;
                                if let Ok(resp) =
                                    call_list_backup_files(&server_name, &api_key, user_id).await
                                {
                                    if let Some(files) =
                                        resp.get("backup_files").and_then(|f| f.as_array())
                                    {
                                        let appeared = files.iter().any(|f| {
                                            f.get("filename").and_then(|n| n.as_str())
                                                == Some(filename.as_str())
                                        });
                                        backup_files.set(files.clone());
                                        if appeared {
                                            // One more refresh to capture the final file size.
                                            gloo_timers::future::TimeoutFuture::new(3_000).await;
                                            if let Ok(resp2) = call_list_backup_files(
                                                &server_name,
                                                &api_key,
                                                user_id,
                                            )
                                            .await
                                            {
                                                if let Some(files2) = resp2
                                                    .get("backup_files")
                                                    .and_then(|f| f.as_array())
                                                {
                                                    backup_files.set(files2.clone());
                                                }
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    error_prefix.clone(),
                                    format_error_message(&e.to_string())
                                ));
                            });
                        }
                    }
                    is_loading.set(false);
                });
            }
        })
    };

    // Manually refresh the existing-backups list.
    let on_refresh_backups = {
        let files_reload = files_reload.clone();
        Callback::from(move |_: MouseEvent| {
            files_reload.set(*files_reload + 1);
        })
    };

    // Delete an existing backup file (with confirmation), then refresh the list.
    let on_delete_backup = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let files_reload = files_reload.clone();
        let confirm_msg = i18n.t("backup_server.confirm_delete_backup").to_string();
        let deleted_msg = i18n.t("backup_server.backup_deleted").to_string();
        let error_prefix = i18n.t("backup_server.failed_to_delete_backup").to_string();

        Callback::from(move |filename: String| {
            let confirm_text = format!("{} {}", confirm_msg, filename);
            let confirmed = window()
                .and_then(|w| w.confirm_with_message(&confirm_text).ok())
                .unwrap_or(false);
            if !confirmed {
                return;
            }

            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                let files_reload = files_reload.clone();
                let deleted_msg = deleted_msg.clone();
                let error_prefix = error_prefix.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_delete_backup_file(&server_name, &api_key.unwrap(), user_id, &filename)
                        .await
                    {
                        Ok(_) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Some(deleted_msg.clone());
                            });
                            files_reload.set(*files_reload + 1);
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    error_prefix,
                                    format_error_message(&e.to_string())
                                ));
                            });
                        }
                    }
                });
            }
        })
    };

    html! {
        <>
            <div class="settings-subsection-title">
                <i class="ph ph-clock-clockwise" style="margin-right:6px;color:var(--accent-color);"></i>
                {i18n.t("backup_server.scheduled_backups")}
            </div>

            <div class="settings-row">
                <div class="settings-row-label">
                    <div>{i18n.t("backup_server.schedule_time")}</div>
                </div>
                <div class="settings-row-control">
                    <select
                        value={(*cron_schedule).clone()}
                        onchange={on_schedule_time_change}
                        disabled={*schedule_loading}
                        class="select"
                        key={(*cron_schedule).clone()}
                    >
                        <option value="0 0 2 * * *" selected={*cron_schedule == "0 0 2 * * *"}>{i18n.t("backup_server.daily_2am")}</option>
                        <option value="0 0 3 * * 0" selected={*cron_schedule == "0 0 3 * * 0"}>{i18n.t("backup_server.weekly_sunday_3am")}</option>
                        <option value="0 0 1 1 * *" selected={*cron_schedule == "0 0 1 1 * *"}>{i18n.t("backup_server.monthly_1st_1am")}</option>
                        <option value="0 0 */6 * * *" selected={*cron_schedule == "0 0 */6 * * *"}>{i18n.t("backup_server.every_6_hours")}</option>
                        <option value="0 0 */12 * * *" selected={*cron_schedule == "0 0 */12 * * *"}>{i18n.t("backup_server.every_12_hours")}</option>
                    </select>
                </div>
            </div>

            <div class="settings-row">
                <div class="settings-row-label">
                    <div>{i18n.t("backup_server.status")}</div>
                </div>
                <div class="settings-row-control">
                    <label class="toggle">
                        <input
                            type="checkbox"
                            checked={*schedule_enabled}
                            disabled={*schedule_loading}
                            onclick={on_schedule_toggle}
                        />
                        <span class="toggle-track"><span class="toggle-thumb"></span></span>
                        <span style="font-size:13px;color:var(--text-color);">
                            {if *schedule_enabled { i18n.t("backup_server.enabled") } else { i18n.t("backup_server.disabled") }}
                        </span>
                    </label>
                </div>
            </div>

            <div class="settings-row">
                <div class="settings-row-label">
                    <div>{i18n.t("backup_server.retention_label")}</div>
                    <div class="settings-row-desc">{i18n.t("backup_server.retention_hint")}</div>
                </div>
                <div class="settings-row-control">
                    <input
                        type="number"
                        min="0"
                        value={(*retention_count).to_string()}
                        onchange={on_retention_change}
                        disabled={*schedule_loading}
                        class="input"
                        style="width:90px;"
                    />
                </div>
            </div>

            if let Some(schedule_info) = &*current_schedule {
                if let Some(updated_at) = schedule_info.get("updated_at").and_then(|v| v.as_str()) {
                    <div class="settings-row">
                        <div class="settings-row-label">
                            <div class="settings-row-desc">
                                <i class="ph ph-info" style="margin-right:4px;"></i>
                                {format!("{}{}", i18n.t("backup_server.last_updated"), updated_at)}
                            </div>
                        </div>
                    </div>
                }
            }

            <div class="settings-subsection-title">
                <i class="ph ph-download-simple" style="margin-right:6px;color:var(--accent-color);"></i>
                {i18n.t("backup_server.manual_backup")}
            </div>

            <div class="settings-row" style="flex-wrap: wrap;">
                <div class="settings-row-label">
                    <div>{i18n.t("backup_server.download_backup_file")}</div>
                    <div class="settings-row-desc">{i18n.t("backup_server.download_backup_description")}</div>
                </div>
                <div class="settings-row-control" style="flex: 1 1 200px; min-width: 0;">
                    <input
                        type="password"
                        id="db-pw"
                        disabled={*is_loading}
                        oninput={Callback::from(move |e: InputEvent| {
                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                            database_password.set(input.value());
                        })}
                        class="input"
                        placeholder="mYDBp@ss!"
                        style="flex: 1; min-width: 0;"
                    />
                    <button
                        onclick={on_download_click}
                        disabled={*is_loading}
                        class="btn btn-secondary"
                        style="flex-shrink: 0;"
                    >
                        if *is_loading {
                            <i class="ph ph-spinner animate-spin"></i>
                            <span>{i18n.t("backup_server.exporting")}</span>
                        } else {
                            <i class="ph ph-download-simple"></i>
                            <span>{i18n.t("backup_server.authenticate")}</span>
                        }
                    </button>
                </div>
            </div>

            <div class="settings-row" style="flex-wrap: wrap;">
                <div class="settings-row-label">
                    <div>{i18n.t("backup_server.save_to_backup_directory")}</div>
                    <div class="settings-row-desc">{i18n.t("backup_server.save_to_backup_description")}</div>
                </div>
                <div class="settings-row-control" style="flex: 1 1 auto;">
                    <button
                        onclick={on_manual_backup_to_directory}
                        disabled={*is_loading}
                        class="btn btn-secondary"
                    >
                        if *is_loading {
                            <i class="ph ph-spinner animate-spin"></i>
                            <span>{i18n.t("backup_server.creating")}</span>
                        } else {
                            <i class="ph ph-folder-plus"></i>
                            <span>{i18n.t("backup_server.create_backup")}</span>
                        }
                    </button>
                </div>
            </div>

            if *is_loading {
                <div class="settings-row">
                    <div class="settings-row-label">
                        <div class="settings-row-desc" style="display:flex;align-items:center;gap:8px;">
                            <i class="ph ph-spinner animate-spin"></i>
                            <span>{i18n.t("backup_server.backing_up_database")}</span>
                        </div>
                        <div class="settings-row-desc">{i18n.t("backup_server.backup_loading_message")}</div>
                    </div>
                </div>
            }

            <div class="settings-subsection-title" style="display:flex;align-items:center;justify-content:space-between;">
                <span>
                    <i class="ph ph-archive" style="margin-right:6px;color:var(--accent-color);"></i>
                    {i18n.t("backup_server.existing_backups")}
                </span>
                <button
                    onclick={on_refresh_backups}
                    disabled={*files_loading}
                    class="btn btn-secondary"
                    title={i18n.t("backup_server.refresh_backups")}
                    style="padding:4px 10px;"
                >
                    if *files_loading {
                        <i class="ph ph-spinner animate-spin"></i>
                    } else {
                        <i class="ph ph-arrows-clockwise"></i>
                    }
                    <span>{i18n.t("backup_server.refresh")}</span>
                </button>
            </div>

            <div class="settings-row">
                <div class="settings-row-label">
                    <div class="settings-row-desc">{i18n.t("backup_server.existing_backups_hint")}</div>
                </div>
            </div>

            {
                if *files_loading {
                    html! {
                        <div class="settings-row">
                            <div class="settings-row-label">
                                <div class="settings-row-desc" style="display:flex;align-items:center;gap:8px;">
                                    <i class="ph ph-spinner animate-spin"></i>
                                    <span>{i18n.t("backup_server.loading_backups")}</span>
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
                                    {i18n.t("backup_server.no_backups")}
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    html! {
                        <div style="display:flex;flex-direction:column;gap:4px;max-height:300px;overflow-y:auto;padding:0 16px 8px;">
                            {for backup_files.iter().map(|file| {
                                let filename = file.get("filename").and_then(|f| f.as_str()).unwrap_or("Unknown").to_string();
                                let size = file.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                                let modified = file.get("modified").and_then(|m| m.as_u64()).unwrap_or(0);

                                let on_delete_backup = on_delete_backup.clone();
                                let filename_for_click = filename.clone();
                                let delete_title = i18n.t("backup_server.delete_backup").to_string();

                                html! {
                                    <div style="display:flex;align-items:center;justify-content:space-between;padding:10px 12px;border-radius:6px;border:1px solid rgba(128,128,128,0.2);">
                                        <div style="display:flex;align-items:center;gap:10px;min-width:0;">
                                            <i class="ph ph-file-sql" style="color:var(--text-secondary-color);font-size:18px;"></i>
                                            <div style="min-width:0;">
                                                <div style="font-size:13px;color:var(--text-color);font-weight:500;overflow:hidden;text-overflow:ellipsis;">{&filename}</div>
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
                                        <button
                                            class="btn btn-danger"
                                            title={delete_title}
                                            style="flex-shrink:0;padding:4px 8px;"
                                            onclick={Callback::from(move |_: MouseEvent| {
                                                on_delete_backup.emit(filename_for_click.clone());
                                            })}
                                        >
                                            <i class="ph ph-trash"></i>
                                        </button>
                                    </div>
                                }
                            })}
                        </div>
                    }
                }
            }
        </>
    }
}
