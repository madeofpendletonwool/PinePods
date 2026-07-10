// src/components/setting_components/ai_settings.rs
//
// AI settings (#726 transcription + #790 ad removal): shows whether the optional pinepods-ai
// sidecar is connected (per capability), lets an admin pick the transcription + LLM models
// (local bundled GGUF or a remote OpenAI-compatible endpoint), pull new models, and shows a live
// queue of AI jobs (transcription, ad detection, model pulls).

use crate::components::context::{AppState, NotificationState};
use crate::requests::pod_req::{
    call_ai_pull_model, call_get_ai_models, call_get_ai_settings, call_get_ai_status_full,
    call_update_ai_settings, AiModels, AiPullModelRequest, AiSettingsUpdate, AiStatus,
};
use i18nrs::yew::use_translation;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(AiSettings)]
pub fn ai_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let (notif_state, _) = use_store::<NotificationState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().and_then(|ud| ud.api_key.clone());

    let ai_status = use_state(AiStatus::default);
    let checked = use_state(|| false);
    let is_admin = use_state(|| false); // could we load admin-only settings?
    let models = use_state(AiModels::default);

    // Editable form fields (populated from the loaded settings).
    let transcription_model = use_state(String::new);
    let llm_backend = use_state(|| "local".to_string());
    let llm_model = use_state(String::new);
    let llm_url = use_state(String::new);
    let llm_api_key = use_state(String::new);
    let has_api_key = use_state(|| false);
    let saving = use_state(|| false);
    let refresh = use_state(|| 0u32);

    // Pull-model form.
    let pull_kind = use_state(|| "gguf".to_string());
    let pull_model = use_state(String::new);
    let pull_repo = use_state(String::new);
    let pull_filename = use_state(String::new);

    {
        let ai_status = ai_status.clone();
        let checked = checked.clone();
        let is_admin = is_admin.clone();
        let models = models.clone();
        let transcription_model = transcription_model.clone();
        let llm_backend = llm_backend.clone();
        let llm_model = llm_model.clone();
        let llm_url = llm_url.clone();
        let has_api_key = has_api_key.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        use_effect_with(*refresh, move |_| {
            if let Some(server_name) = server_name {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(st) = call_get_ai_status_full(&server_name, &api_key).await {
                        ai_status.set(st);
                    }
                    checked.set(true);
                    // Admin-only: settings + installed models.
                    if let Ok(s) = call_get_ai_settings(&server_name, &api_key).await {
                        transcription_model.set(s.transcription_model.clone());
                        llm_backend.set(s.llm_backend.clone());
                        llm_model.set(s.llm_model.clone().unwrap_or_default());
                        llm_url.set(s.llm_url.clone().unwrap_or_default());
                        has_api_key.set(s.has_api_key);
                        is_admin.set(true);
                    }
                    let remote = None; // local + whisper listing; remote enumerated on demand
                    if let Ok(m) = call_get_ai_models(&server_name, &api_key, remote).await {
                        models.set(m);
                    }
                });
            }
            || ()
        });
    }

    let capability_row = |label: String, ready: bool| -> Html {
        let (color, text) = if ready {
            ("var(--success-color)", i18n.t("ai_settings.ready"))
        } else {
            ("var(--text-secondary-color)", i18n.t("ai_settings.not_configured"))
        };
        html! {
            <div class="flex items-center justify-between py-1 text-sm">
                <span class="item_container-text">{ label }</span>
                <span style={format!("color:{};font-weight:500;", color)}>{ text }</span>
            </div>
        }
    };

    let connection_badge = if ai_status.available {
        html! { <span style="color:var(--success-color);font-weight:500;">{ i18n.t("ai_settings.connected") }</span> }
    } else if *checked {
        html! { <span style="color:var(--error-color);font-weight:500;">{ i18n.t("ai_settings.not_connected") }</span> }
    } else {
        html! { <span style="color:var(--text-secondary-color);">{ i18n.t("ai_settings.checking") }</span> }
    };

    // ---- input handlers ----
    let set_from_select = |handle: UseStateHandle<String>| {
        Callback::from(move |e: Event| {
            if let Some(sel) = e.target_dyn_into::<HtmlSelectElement>() {
                handle.set(sel.value());
            }
        })
    };
    let set_from_input = |handle: UseStateHandle<String>| {
        Callback::from(move |e: InputEvent| {
            if let Some(inp) = e.target_dyn_into::<HtmlInputElement>() {
                handle.set(inp.value());
            }
        })
    };

    let on_save = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let transcription_model = transcription_model.clone();
        let llm_backend = llm_backend.clone();
        let llm_model = llm_model.clone();
        let llm_url = llm_url.clone();
        let llm_api_key = llm_api_key.clone();
        let saving = saving.clone();
        let refresh = refresh.clone();
        let saved_msg = i18n.t("ai_settings.saved").to_string();
        let err_msg = i18n.t("ai_settings.save_error").to_string();
        Callback::from(move |_: MouseEvent| {
            let (server_name, api_key) = (server_name.clone(), api_key.clone());
            let update = AiSettingsUpdate {
                transcription_model: (*transcription_model).clone(),
                llm_backend: (*llm_backend).clone(),
                llm_model: Some((*llm_model).clone()).filter(|s| !s.is_empty()),
                llm_url: Some((*llm_url).clone()).filter(|s| !s.is_empty()),
                llm_api_key: Some((*llm_api_key).clone()).filter(|s| !s.is_empty()),
                clear_api_key: false,
                whisper_device: None,
                whisper_compute_type: None,
            };
            let (saving, refresh) = (saving.clone(), refresh.clone());
            let (saved_msg, err_msg) = (saved_msg.clone(), err_msg.clone());
            let llm_api_key = llm_api_key.clone();
            saving.set(true);
            if let Some(server_name) = server_name {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_update_ai_settings(&server_name, &api_key, &update).await {
                        Ok(_) => {
                            llm_api_key.set(String::new());
                            Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(saved_msg));
                            refresh.set(*refresh + 1);
                        }
                        Err(e) => Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(format!("{}: {}", err_msg, e))),
                    }
                    saving.set(false);
                });
            }
        })
    };

    let on_pull = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let pull_kind = pull_kind.clone();
        let pull_model = pull_model.clone();
        let pull_repo = pull_repo.clone();
        let pull_filename = pull_filename.clone();
        let llm_url = llm_url.clone();
        let started = i18n.t("ai_settings.pull_started").to_string();
        let err = i18n.t("ai_settings.pull_error").to_string();
        Callback::from(move |_: MouseEvent| {
            let (server_name, api_key) = (server_name.clone(), api_key.clone());
            let kind = (*pull_kind).clone();
            let request = AiPullModelRequest {
                kind: kind.clone(),
                model: (*pull_model).clone(),
                repo: Some((*pull_repo).clone()).filter(|s| !s.is_empty()),
                filename: Some((*pull_filename).clone()).filter(|s| !s.is_empty()),
                url: if kind == "ollama" { Some((*llm_url).clone()).filter(|s| !s.is_empty()) } else { None },
            };
            let (started, err) = (started.clone(), err.clone());
            if let Some(server_name) = server_name {
                wasm_bindgen_futures::spawn_local(async move {
                    match call_ai_pull_model(&server_name, &api_key, &request).await {
                        Ok(_) => Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(started)),
                        Err(e) => Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(format!("{}: {}", err, e))),
                    }
                });
            }
        })
    };

    // Live AI job queue (transcription, ad detection, model pulls).
    let ai_tasks: Vec<_> = notif_state
        .active_tasks
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|t| matches!(t.r#type.as_str(), "transcribe_episode" | "detect_ads" | "pull_model"))
        .collect();

    let backend = (*llm_backend).clone();
    let whisper_opts = models.whisper.clone();
    let local_opts = models.llm_local.clone();

    html! {
        <div class="transcription-settings">
            <div class="mb-3">
                <span class="text-sm font-medium" style="color:var(--text-color);">{ i18n.t("ai_settings.ai_service") }{" "}</span>
                { connection_badge }
            </div>
            <p class="item_container-text text-sm mb-3">{ i18n.t("ai_settings.intro") }</p>

            <div class="settings-section-title mt-3 mb-1">{ i18n.t("ai_settings.capabilities") }</div>
            { capability_row(i18n.t("ai_settings.transcription").to_string(), ai_status.transcription_ready) }
            { capability_row(i18n.t("ai_settings.ad_removal").to_string(), ai_status.ad_removal_ready) }

            {
                // Ad removal has no default model — tell the admin how to enable it.
                if *is_admin && ai_status.available && !ai_status.ad_removal_ready {
                    html! {
                        <p class="item_container-text text-sm mt-2 mb-1" style="color:var(--text-secondary-color);">
                            { i18n.t("ai_settings.ad_removal_hint") }
                        </p>
                    }
                } else { html! {} }
            }

            {
                if *is_admin {
                    html! {
                        <>
                            <div class="settings-section-title mt-4 mb-2">{ i18n.t("ai_settings.models") }</div>

                            // Transcription (whisper) model
                            <label class="block mb-1 text-sm font-medium" style="color:var(--text-color);">{ i18n.t("ai_settings.transcription_model") }</label>
                            <select class="email-select mb-3" onchange={set_from_select(transcription_model.clone())} value={(*transcription_model).clone()}>
                                { for whisper_opts.iter().map(|m| html!{ <option value={m.clone()} selected={*transcription_model == *m}>{ m.clone() }</option> }) }
                            </select>

                            // LLM backend (ad detection)
                            <label class="block mb-1 text-sm font-medium" style="color:var(--text-color);">{ i18n.t("ai_settings.llm_backend") }</label>
                            <select class="email-select mb-3" onchange={set_from_select(llm_backend.clone())} value={backend.clone()}>
                                <option value="local" selected={backend == "local"}>{ i18n.t("ai_settings.backend_local") }</option>
                                <option value="remote" selected={backend == "remote"}>{ i18n.t("ai_settings.backend_remote") }</option>
                                <option value="anthropic" selected={backend == "anthropic"}>{ i18n.t("ai_settings.backend_anthropic") }</option>
                            </select>

                            {
                                if backend == "remote" || backend == "anthropic" {
                                    let (url_ph, model_ph) = if backend == "anthropic" {
                                        ("https://api.z.ai/api/anthropic", "glm-4.6")
                                    } else {
                                        ("http://ollama:11434/v1", "qwen2.5:3b")
                                    };
                                    html! {
                                        <>
                                            <label class="block mb-1 text-sm font-medium" style="color:var(--text-color);">{ i18n.t("ai_settings.llm_url") }</label>
                                            <input type="text" class="email-input mb-3" placeholder={url_ph}
                                                value={(*llm_url).clone()} oninput={set_from_input(llm_url.clone())} />
                                            <label class="block mb-1 text-sm font-medium" style="color:var(--text-color);">{ i18n.t("ai_settings.llm_model_name") }</label>
                                            <input type="text" class="email-input mb-3" placeholder={model_ph}
                                                value={(*llm_model).clone()} oninput={set_from_input(llm_model.clone())} />
                                            <label class="block mb-1 text-sm font-medium" style="color:var(--text-color);">{ i18n.t("ai_settings.llm_api_key") }</label>
                                            <input type="password" class="email-input mb-3"
                                                placeholder={ if *has_api_key { i18n.t("ai_settings.api_key_set").to_string() } else { String::new() } }
                                                value={(*llm_api_key).clone()} oninput={set_from_input(llm_api_key.clone())} />
                                        </>
                                    }
                                } else {
                                    html! {
                                        <>
                                            <label class="block mb-1 text-sm font-medium" style="color:var(--text-color);">{ i18n.t("ai_settings.llm_local_model") }</label>
                                            <select class="email-select mb-3" onchange={set_from_select(llm_model.clone())} value={(*llm_model).clone()}>
                                                <option value="" selected={(*llm_model).is_empty()}>{ i18n.t("ai_settings.select_model") }</option>
                                                { for local_opts.iter().map(|m| html!{ <option value={m.clone()} selected={*llm_model == *m}>{ m.clone() }</option> }) }
                                            </select>
                                        </>
                                    }
                                }
                            }

                            <button class="download-button mb-4" onclick={on_save} disabled={*saving}>
                                { if *saving { i18n.t("ai_settings.saving") } else { i18n.t("ai_settings.save") } }
                            </button>

                            // Pull a new model
                            <div class="settings-section-title mt-2 mb-2">{ i18n.t("ai_settings.pull_model") }</div>
                            <div class="flex flex-wrap gap-2 items-end mb-3">
                                <select class="email-select" onchange={set_from_select(pull_kind.clone())} value={(*pull_kind).clone()}>
                                    <option value="gguf" selected={*pull_kind == "gguf"}>{ "GGUF (Hugging Face)" }</option>
                                    <option value="whisper" selected={*pull_kind == "whisper"}>{ "Whisper" }</option>
                                    <option value="ollama" selected={*pull_kind == "ollama"}>{ "Ollama" }</option>
                                </select>
                                {
                                    if *pull_kind == "gguf" {
                                        html! {
                                            <>
                                                <input type="text" class="email-input" placeholder={i18n.t("ai_settings.hf_repo").to_string()} value={(*pull_repo).clone()} oninput={set_from_input(pull_repo.clone())} />
                                                <input type="text" class="email-input" placeholder={i18n.t("ai_settings.hf_filename").to_string()} value={(*pull_filename).clone()} oninput={set_from_input(pull_filename.clone())} />
                                            </>
                                        }
                                    } else {
                                        html! {
                                            <input type="text" class="email-input" placeholder={i18n.t("ai_settings.model_name").to_string()} value={(*pull_model).clone()} oninput={set_from_input(pull_model.clone())} />
                                        }
                                    }
                                }
                                <button class="download-button" onclick={on_pull}>{ i18n.t("ai_settings.pull") }</button>
                            </div>
                        </>
                    }
                } else { html! {} }
            }

            <div class="settings-section-title mt-4 mb-2">{ i18n.t("ai_settings.queue") }</div>
            {
                if ai_tasks.is_empty() {
                    html! { <p class="item_container-text text-sm" style="color:var(--text-secondary-color);">{ i18n.t("ai_settings.no_active_jobs") }</p> }
                } else {
                    html! {
                        <ul class="transcription-queue">
                            { for ai_tasks.iter().map(|t| {
                                let label = t.item_id.clone().unwrap_or_else(|| t.task_id.clone());
                                // task.progress is already a 0–100 percentage.
                                let pct = t.progress.round().clamp(0.0, 100.0) as i32;
                                let kind = match t.r#type.as_str() {
                                    "detect_ads" => i18n.t("ai_settings.job_ads"),
                                    "pull_model" => i18n.t("ai_settings.job_pull"),
                                    _ => i18n.t("ai_settings.job_transcribe"),
                                };
                                // Prefer the operation message (e.g. "Transcribing…") from details;
                                // the raw `status` enum is download-centric ("DOWNLOADING").
                                let status_label = t.details.as_ref()
                                    .and_then(|d| d.get("status_text").cloned())
                                    .unwrap_or_else(|| match t.status.as_str() {
                                        "SUCCESS" => i18n.t("ai_settings.done").to_string(),
                                        "FAILED" => i18n.t("ai_settings.failed").to_string(),
                                        _ => i18n.t("ai_settings.running").to_string(),
                                    });
                                html! {
                                    <li class="flex items-center justify-between py-1 text-sm">
                                        <span class="item_container-text">{ format!("{} {}", kind, label) }</span>
                                        <span style="color:var(--text-secondary-color);">{ format!("{} — {}%", status_label, pct) }</span>
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
