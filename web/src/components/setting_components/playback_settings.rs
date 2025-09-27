// src/components/setting_components/playback_settings.rs

use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{call_get_auto_complete_seconds, call_update_auto_complete_seconds};
use anyhow::Error;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[derive(Serialize, Deserialize, Debug)]
pub struct SetPlaybackSpeedRequest {
    pub user_id: i32,
    pub playback_speed: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetPlaybackSpeedRequest {
    pub user_id: i32,
    pub podcast_id: Option<i32>,
}

#[derive(Deserialize, Debug)]
struct PlaybackSpeedResponse {
    detail: String,
}

#[derive(Deserialize, Debug)]
struct PlaybackSpeedGetResponse {
    playback_speed: f64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SetPlaybackSpeedUserRequest {
    pub user_id: i32,
    pub playback_speed: f64,
}

async fn call_set_user_playback_speed(
    server_name: &String,
    api_key: &Option<String>,
    user_id: i32,
    playback_speed: f64,
) -> Result<String, Error> {
    let url = format!("{}/api/data/user/set_playback_speed", server_name);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_data = SetPlaybackSpeedUserRequest {
        user_id,
        playback_speed,
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
        let response_body: PlaybackSpeedResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.detail)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to set playback speed: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

async fn call_get_user_playback_speed(
    server_name: &String,
    api_key: &Option<String>,
    user_id: i32,
) -> Result<f64, Error> {
    let url = format!("{}/api/data/get_playback_speed", server_name);
    let api_key_ref = api_key
        .as_deref()
        .ok_or_else(|| anyhow::Error::msg("API key is missing"))?;

    let request_data = GetPlaybackSpeedRequest {
        user_id,
        podcast_id: None, // This is None for user-wide settings
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
        let response_body: PlaybackSpeedGetResponse =
            response.json().await.map_err(|e| anyhow::Error::new(e))?;
        Ok(response_body.playback_speed)
    } else {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| String::from("Failed to read error message"));
        Err(anyhow::Error::msg(format!(
            "Failed to get playback speed: {} - {}",
            response.status_text(),
            error_text
        )))
    }
}

#[function_component(PlaybackSettings)]
pub fn playback_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID);

    // Capture i18n strings before they get moved
    let i18n_failed_to_fetch_playback_speed = i18n.t("playback_settings.failed_to_fetch_playback_speed").to_string();
    let i18n_failed_to_fetch_auto_complete_seconds = i18n.t("playback_settings.failed_to_fetch_auto_complete_seconds").to_string();
    let i18n_default_playback_speed_updated_successfully = i18n.t("playback_settings.default_playback_speed_updated_successfully").to_string();
    let i18n_failed_to_update_playback_speed = i18n.t("playback_settings.failed_to_update_playback_speed").to_string();
    let i18n_auto_complete_seconds_updated_successfully = i18n.t("playback_settings.auto_complete_seconds_updated_successfully").to_string();
    let i18n_failed_to_update_auto_complete_seconds = i18n.t("playback_settings.failed_to_update_auto_complete_seconds").to_string();
    let i18n_playback_preferences_description = i18n.t("playback_settings.playback_preferences_description").to_string();
    let i18n_default_playback_speed = i18n.t("playback_settings.default_playback_speed").to_string();
    let i18n_save = i18n.t("common.save").to_string();
    let i18n_range = i18n.t("playback_settings.range").to_string();
    let i18n_auto_complete_episode_threshold = i18n.t("playback_settings.auto_complete_episode_threshold").to_string();
    let i18n_seconds = i18n.t("playback_settings.seconds").to_string();
    let i18n_auto_complete_description = i18n.t("playback_settings.auto_complete_description").to_string();

    // State for playback speed
    let default_playback_speed = use_state(|| 1.0);
    let is_loading = use_state(|| true);
    let show_success = use_state(|| false);
    let success_message = use_state(|| "".to_string());
    
    // State for auto complete seconds
    let auto_complete_seconds = use_state(|| 0);
    let auto_complete_loading = use_state(|| true);

    // Fetch initial playback speed
    {
        let default_playback_speed = default_playback_speed.clone();
        let is_loading = is_loading.clone();
        let dispatch = dispatch.clone();

        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                if let (Some(api_key), Some(server_name), Some(user_id)) =
                    (api_key.clone(), server_name.clone(), user_id)
                {
                    let server_name = server_name.clone();
                    let api_key = api_key.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match call_get_user_playback_speed(&server_name, &api_key, user_id).await {
                            Ok(speed) => {
                                default_playback_speed.set(speed);
                                is_loading.set(false);
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                dispatch.reduce_mut(|state| {
                                    state.error_message = Some(format!(
                                        "{}{}",
                                        i18n_failed_to_fetch_playback_speed.clone(),
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

    // Fetch initial auto complete seconds
    {
        let auto_complete_seconds = auto_complete_seconds.clone();
        let auto_complete_loading = auto_complete_loading.clone();
        let dispatch = dispatch.clone();

        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                if let (Some(api_key), Some(server_name), Some(user_id)) =
                    (api_key.clone(), server_name.clone(), user_id)
                {
                    let server_name = server_name.clone();
                    let api_key = api_key.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_get_auto_complete_seconds(server_name, api_key.unwrap(), user_id).await {
                            Ok(seconds) => {
                                auto_complete_seconds.set(seconds);
                                auto_complete_loading.set(false);
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                dispatch.reduce_mut(|state| {
                                    state.error_message = Some(format!(
                                        "{}{}",
                                        i18n_failed_to_fetch_auto_complete_seconds.clone(),
                                        formatted_error
                                    ));
                                });
                                auto_complete_loading.set(false);
                            }
                        }
                    });
                } else {
                    auto_complete_loading.set(false);
                }
                || ()
            },
        );
    }

    // Input handler for playback speed
    let on_playback_speed_change = {
        let default_playback_speed = default_playback_speed.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            let value = target.value().parse::<f64>().unwrap_or(1.0);
            // Constrain to reasonable values (0.5 to 3.0)
            let value = value.max(0.5).min(3.0);
            default_playback_speed.set(value);
        })
    };

    // Save playback speed
    let on_save_playback_speed = {
        let default_playback_speed = default_playback_speed.clone();
        let show_success = show_success.clone();
        let success_message = success_message.clone();
        let dispatch = dispatch.clone();
        let api_key_call = api_key.clone();
        let server_name_call = server_name.clone();
        let i18n_default_playback_speed_updated_successfully = i18n_default_playback_speed_updated_successfully.clone();
        let i18n_failed_to_update_playback_speed = i18n_failed_to_update_playback_speed.clone();

        Callback::from(move |e: MouseEvent| {
            let i18n_default_playback_speed_updated_successfully = i18n_default_playback_speed_updated_successfully.clone();
            let i18n_failed_to_update_playback_speed = i18n_failed_to_update_playback_speed.clone();
            e.prevent_default();

            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key_call.clone(), server_name_call.clone(), user_id)
            {
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let speed = *default_playback_speed;
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let dispatch = dispatch.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_set_user_playback_speed(&server_name, &api_key, user_id, speed).await
                    {
                        Ok(_) => {
                            show_success.set(true);
                            success_message
                                .set(i18n_default_playback_speed_updated_successfully.clone());

                            // Auto-hide success message after 3 seconds
                            let show_success_clone = show_success.clone();
                            gloo_timers::callback::Timeout::new(3000, move || {
                                show_success_clone.set(false);
                            })
                            .forget();
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    i18n_failed_to_update_playback_speed,
                                    formatted_error
                                ));
                            });
                        }
                    }
                });
            }
        })
    };

    // Input handler for auto complete seconds
    let on_auto_complete_change = {
        let auto_complete_seconds = auto_complete_seconds.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_unchecked_into::<HtmlInputElement>();
            let value = target.value().parse::<i32>().unwrap_or(0);
            // Constrain to reasonable values (0 to 3600 seconds = 1 hour)
            let value = value.max(0).min(3600);
            auto_complete_seconds.set(value);
        })
    };

    // Save auto complete seconds
    let on_save_auto_complete = {
        let auto_complete_seconds = auto_complete_seconds.clone();
        let show_success = show_success.clone();
        let success_message = success_message.clone();
        let dispatch = dispatch.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let i18n_auto_complete_seconds_updated_successfully = i18n_auto_complete_seconds_updated_successfully.clone();
        let i18n_failed_to_update_auto_complete_seconds = i18n_failed_to_update_auto_complete_seconds.clone();

        Callback::from(move |e: MouseEvent| {
            let i18n_auto_complete_seconds_updated_successfully = i18n_auto_complete_seconds_updated_successfully.clone();
            let i18n_failed_to_update_auto_complete_seconds = i18n_failed_to_update_auto_complete_seconds.clone();
            e.prevent_default();

            if let (Some(api_key), Some(server_name), Some(user_id)) =
                (api_key.clone(), server_name.clone(), user_id)
            {
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let seconds = *auto_complete_seconds;
                let show_success = show_success.clone();
                let success_message = success_message.clone();
                let dispatch = dispatch.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_update_auto_complete_seconds(server_name, api_key.unwrap(), user_id, seconds).await
                    {
                        Ok(_) => {
                            show_success.set(true);
                            success_message
                                .set(i18n_auto_complete_seconds_updated_successfully.clone());

                            // Auto-hide success message after 3 seconds
                            let show_success_clone = show_success.clone();
                            gloo_timers::callback::Timeout::new(3000, move || {
                                show_success_clone.set(false);
                            })
                            .forget();
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!(
                                    "{}{}",
                                    i18n_failed_to_update_auto_complete_seconds,
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
        <div class="playback-settings-container">
            <div class="settings-description mb-4">
                <p>{&i18n_playback_preferences_description}</p>
            </div>

            <div class="playback-speed-control">
                <div class="mt-4">
                    <label for="default-playback-speed" class="block mb-2 text-sm font-medium">{&i18n_default_playback_speed}</label>
                    <div class="flex items-center space-x-2">
                        <input
                            type="number"
                            id="default-playback-speed"
                            value={default_playback_speed.to_string()}
                            oninput={on_playback_speed_change}
                            class="form-input w-20"
                            min="0.5"
                            max="3.0"
                            step="0.1"
                            disabled={*is_loading}
                        />
                        <span class="text-sm">{"x"}</span>
                        <button
                            class="playback-submit-button ml-2"
                            onclick={on_save_playback_speed}
                            disabled={*is_loading}
                        >
                            <i class="ph ph-floppy-disk mr-1"></i>
{&i18n_save}
                        </button>
                    </div>
                    <p class="text-xs text-gray-500 mt-1">{&i18n_range}</p>
                </div>
            </div>

            <div class="auto-complete-control mt-6">
                <div class="mt-4">
                    <label for="auto-complete-seconds" class="block mb-2 text-sm font-medium">{&i18n_auto_complete_episode_threshold}</label>
                    <div class="flex items-center space-x-2">
                        <input
                            type="number"
                            id="auto-complete-seconds"
                            value={auto_complete_seconds.to_string()}
                            oninput={on_auto_complete_change}
                            class="form-input w-20"
                            min="0"
                            max="3600"
                            step="1"
                            disabled={*auto_complete_loading}
                        />
                        <span class="text-sm">{&i18n_seconds}</span>
                        <button
                            class="playback-submit-button ml-2"
                            onclick={on_save_auto_complete}
                            disabled={*auto_complete_loading}
                        >
                            <i class="ph ph-floppy-disk mr-1"></i>
{&i18n_save}
                        </button>
                    </div>
                    <p class="text-xs text-gray-500 mt-1">{&i18n_auto_complete_description}</p>
                </div>
            </div>

            if *show_success {
                <div class="success-message mt-4">
                    {(*success_message).clone()}
                </div>
            }
        </div>
    }
}
