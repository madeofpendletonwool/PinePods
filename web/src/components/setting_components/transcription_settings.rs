// src/components/setting_components/transcription_settings.rs
//
// Manage AI transcription (#726): shows whether the optional pinepods-ai sidecar is connected
// and renders a live queue of transcription jobs (filtered from the global active-task stream
// that already powers downloads).

use crate::components::context::{AppState, NotificationState};
use crate::requests::pod_req::call_get_ai_status;
use i18nrs::yew::use_translation;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(TranscriptionSettings)]
pub fn transcription_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let (notif_state, _) = use_store::<NotificationState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().and_then(|ud| ud.api_key.clone());

    let ai_available = use_state(|| false);
    let checked = use_state(|| false);

    {
        let ai_available = ai_available.clone();
        let checked = checked.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        use_effect_with((), move |_| {
            if let Some(server_name) = server_name {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(up) = call_get_ai_status(&server_name, &api_key).await {
                        ai_available.set(up);
                    }
                    checked.set(true);
                });
            }
            || ()
        });
    }

    // Live queue: filter the global active-task list for transcription jobs.
    let transcription_tasks: Vec<_> = notif_state
        .active_tasks
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|t| t.r#type == "transcribe_episode")
        .collect();

    let status_badge = if *ai_available {
        html! { <span style="color:var(--success-color);font-weight:500;">{ i18n.t("transcription.connected") }</span> }
    } else if *checked {
        html! { <span style="color:var(--error-color);font-weight:500;">{ i18n.t("transcription.not_connected") }</span> }
    } else {
        html! { <span style="color:var(--text-secondary-color);">{ i18n.t("transcription.checking") }</span> }
    };

    html! {
        <div class="transcription-settings">
            <div class="mb-3">
                <span class="text-sm font-medium" style="color:var(--text-color);">{ i18n.t("transcription.ai_service") }{" "}</span>
                { status_badge }
            </div>
            <p class="item_container-text text-sm mb-3">
                { i18n.t("transcription.intro") }
            </p>
            {
                if !*ai_available && *checked {
                    html! {
                        <p class="item_container-text text-sm mb-3">
                            { i18n.t("transcription.enable_hint") }
                        </p>
                    }
                } else { html! {} }
            }

            <div class="settings-section-title mt-4 mb-2">{ i18n.t("transcription.queue") }</div>
            {
                if transcription_tasks.is_empty() {
                    html! { <p class="item_container-text text-sm" style="color:var(--text-secondary-color);">{ i18n.t("transcription.no_active_jobs") }</p> }
                } else {
                    html! {
                        <ul class="transcription-queue">
                            { for transcription_tasks.iter().map(|t| {
                                let label = t.item_id.clone().unwrap_or_else(|| t.task_id.clone());
                                let pct = (t.progress * 100.0).round() as i32;
                                html! {
                                    <li class="flex items-center justify-between py-1 text-sm">
                                        <span class="item_container-text">{ format!("{} {}", i18n.t("transcription.episode"), label) }</span>
                                        <span style="color:var(--text-secondary-color);">{ format!("{} — {}%", t.status, pct) }</span>
                                    </li>
                                }
                            }) }
                        </ul>
                    }
                }
            }
        </div>
    }
}
