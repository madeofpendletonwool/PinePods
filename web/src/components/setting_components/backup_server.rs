use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_backup_server, call_get_scheduled_backup, call_manual_backup_to_directory,
    call_schedule_backup,
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

    // Load current schedule on mount
    {
        let current_schedule = current_schedule.clone();
        let schedule_enabled = schedule_enabled.clone();
        let cron_schedule = cron_schedule.clone();
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
                        }
                        Err(e) => {
                            _dispatch.reduce_mut(|state| {
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
        let disabled_msg = i18n
            .t("backup_server.scheduled_backup_disabled")
            .to_string();
        let error_prefix = i18n
            .t("backup_server.failed_to_update_backup_schedule")
            .to_string();

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
                    )
                    .await
                    {
                        Ok(_) => {
                            _dispatch.reduce_mut(|state| {
                                state.info_message = Some(if new_enabled {
                                    enabled_msg.clone()
                                } else {
                                    disabled_msg.clone()
                                });
                            });
                        }
                        Err(e) => {
                            schedule_enabled_for_async.set(!new_enabled); // Revert on error
                            _dispatch.reduce_mut(|state| {
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
                is_loading.set(true);
                let _dispatch = _dispatch.clone();
                let is_loading = is_loading.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match call_manual_backup_to_directory(&server_name, &api_key.unwrap(), user_id)
                        .await
                    {
                        Ok(response) => {
                            let filename = response
                                .get("filename")
                                .and_then(|f| f.as_str())
                                .unwrap_or("backup file");
                            _dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some(success_template.clone().replace("{}", filename));
                            });
                        }
                        Err(e) => {
                            _dispatch.reduce_mut(|state| {
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

            <div class="settings-row">
                <div class="settings-row-label">
                    <div>{i18n.t("backup_server.download_backup_file")}</div>
                    <div class="settings-row-desc">{i18n.t("backup_server.download_backup_description")}</div>
                </div>
                <div class="settings-row-control">
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
                        style="width:180px;"
                    />
                    <button
                        onclick={on_download_click}
                        disabled={*is_loading}
                        class="btn btn-secondary"
                        style="padding:6px 12px;"
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

            <div class="settings-row">
                <div class="settings-row-label">
                    <div>{i18n.t("backup_server.save_to_backup_directory")}</div>
                    <div class="settings-row-desc">{i18n.t("backup_server.save_to_backup_description")}</div>
                </div>
                <div class="settings-row-control">
                    <button
                        onclick={on_manual_backup_to_directory}
                        disabled={*is_loading}
                        class="btn btn-secondary"
                        style="padding:6px 12px;"
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
        </>
    }
}
