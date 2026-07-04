// src/components/setting_components/auto_delete_settings.rs
//
// Global (per-user) default for auto-deleting server-side downloads after N days (#655).
// A value of 0 disables the feature. Per-podcast overrides live on the podcast page.
// Because this permanently deletes downloaded files from the server, the UI makes the
// destructive nature explicit with a warning.

use crate::components::context::{AppState, NotificationState};
use crate::components::gen_funcs::format_error_message;
use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[derive(Serialize, Deserialize, Debug)]
pub struct SetAutoDeleteDaysUserRequest {
    pub user_id: i32,
    pub days: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetAutoDeleteDaysRequest {
    pub user_id: i32,
    pub podcast_id: Option<i32>,
}

#[derive(Deserialize, Debug)]
struct AutoDeleteDaysSetResponse {
    detail: String,
}

#[derive(Deserialize, Debug)]
struct AutoDeleteDaysGetResponse {
    days: i32,
}

async fn call_set_user_auto_delete_days(
    server_name: &String,
    api_key: &Option<String>,
    user_id: i32,
    days: i32,
) -> Result<String, Error> {
    let url = format!("{}/api/data/user/set_auto_download_delete_days", server_name);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_data = SetAutoDeleteDaysUserRequest { user_id, days };

    let request_body = serde_json::to_string(&request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body: AutoDeleteDaysSetResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.detail)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to set auto-delete days: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

async fn call_get_user_auto_delete_days(
    server_name: &String,
    api_key: &Option<String>,
    user_id: i32,
) -> Result<i32, Error> {
    let url = format!("{}/api/data/get_auto_download_delete_days", server_name);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_data = GetAutoDeleteDaysRequest {
        user_id,
        podcast_id: None, // None = user-wide default
    };

    let request_body = serde_json::to_string(&request_data)
        .map_err(|e| anyhow::Error::msg(format!("Serialization Error: {}", e)))?;

    let response = Request::post(&url)
        .header("Api-Key", api_key_ref)
        .header("Content-Type", "application/json")
        .body(request_body)?
        .send()
        .await?;

    if response.ok() {
        let response_body: AutoDeleteDaysGetResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.days)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to get auto-delete days: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

#[function_component(AutoDeleteSettings)]
pub fn auto_delete_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID);

    // Capture i18n strings before they get moved
    let i18n_failed_to_fetch = i18n.t("auto_delete_settings.failed_to_fetch").to_string();
    let i18n_updated_successfully = i18n.t("auto_delete_settings.updated_successfully").to_string();
    let i18n_failed_to_update = i18n.t("auto_delete_settings.failed_to_update").to_string();
    let i18n_label = i18n.t("auto_delete_settings.label").to_string();
    let i18n_description = i18n.t("auto_delete_settings.description").to_string();
    let i18n_warning = i18n.t("auto_delete_settings.warning").to_string();
    let i18n_days = i18n.t("auto_delete_settings.days").to_string();
    let i18n_save = i18n.t("common.save").to_string();

    let auto_delete_days = use_state(|| 0);
    let is_loading = use_state(|| true);
    let show_success = use_state(|| false);
    let success_message = use_state(|| "".to_string());

    // Fetch initial value
    {
        let auto_delete_days = auto_delete_days.clone();
        let is_loading = is_loading.clone();
        let i18n_failed_to_fetch = i18n_failed_to_fetch.clone();

        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                if let (Some(api_key), Some(server_name), Some(user_id)) =
                    (api_key.clone(), server_name.clone(), user_id)
                {
                    let server_name = server_name.clone();
                    let api_key = api_key.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match call_get_user_auto_delete_days(&server_name, &api_key, user_id).await {
                            Ok(days) => {
                                auto_delete_days.set(days);
                                is_loading.set(false);
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message = Some(format!(
                                        "{}{}",
                                        i18n_failed_to_fetch.clone(),
                                        formatted_error
                                    ));
                                });
                                is_loading.set(false);
                            }
                        }
                    });
                } else {
                    is_loading.set(false);
                }
                || ()
            },
        );
    }

    // Input handler
    let on_days_change = {
        let auto_delete_days = auto_delete_days.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            let value = target.value().parse::<i32>().unwrap_or(0);
            let value = value.max(0).min(3650); // cap at ~10 years
            auto_delete_days.set(value);
        })
    };

    // Save handler
    let on_save = {
        let auto_delete_days = auto_delete_days.clone();
        let show_success = show_success.clone();
        let success_message = success_message.clone();
        let dispatch = dispatch.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let i18n_updated_successfully = i18n_updated_successfully.clone();
        let i18n_failed_to_update = i18n_failed_to_update.clone();

        Callback::from(move |e: MouseEvent| {
            let i18n_updated_successfully = i18n_updated_successfully.clone();
            let i18n_failed_to_update = i18n_failed_to_update.clone();
            e.prevent_default();

            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let days = *auto_delete_days;
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let _dispatch = dispatch.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_set_user_auto_delete_days(&server_name, &api_key, user_id, days).await {
                        Ok(_) => {
                            show_success.set(true);
                            success_message.set(i18n_updated_successfully.clone());

                            let show_success_clone = show_success.clone();
                            gloo_timers::callback::Timeout::new(3000, move || {
                                show_success_clone.set(false);
                            })
                            .forget();
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    i18n_failed_to_update,
                                    formatted_error
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
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{&i18n_label}</div>
                <div class="settings-row-desc">{&i18n_description}</div>
            </div>
            <div class="settings-row-control" style="display:flex;align-items:center;gap:6px;">
                <input
                    type="number"
                    value={auto_delete_days.to_string()}
                    oninput={on_days_change}
                    class="input"
                    style="width:72px;"
                    min="0"
                    max="3650"
                    step="1"
                    disabled={*is_loading}
                />
                <span style="font-size:12px;color:var(--text-secondary-color);">{&i18n_days}</span>
                <button
                    class="btn btn-secondary"
                    style="padding:6px 12px;"
                    onclick={on_save}
                    disabled={*is_loading}
                >
                    <i class="ph ph-floppy-disk"></i>
                    <span>{&i18n_save}</span>
                </button>
            </div>
        </div>
        if *auto_delete_days > 0 {
            <div class="settings-row">
                <div class="settings-row-desc" style="display:flex;align-items:center;gap:6px;color:var(--error-color, #d9534f);">
                    <i class="ph ph-warning-circle"></i>
                    <span>{&i18n_warning}</span>
                </div>
            </div>
        }
        if *show_success {
            <div class="settings-row">
                <div class="success-message" style="font-size:12px;color:var(--hover-color);">
                    {(*success_message).clone()}
                </div>
            </div>
        }
        </>
    }
}
