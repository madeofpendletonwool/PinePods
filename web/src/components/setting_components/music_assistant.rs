// Music Assistant connection settings. Always visible: this is how the feature
// is turned on. Speaker controls elsewhere stay hidden until `enabled` is true.

use crate::components::context::{AppState, NotificationState};
use crate::requests::music_assistant::{
    apply_settings, explain_test, get_settings, refresh_music_assistant, save_settings,
    test_connection, MaProblem, TestExplanation,
};
use crate::components::i18n::use_translation;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

fn problem_key(problem: MaProblem) -> &'static str {
    match problem {
        MaProblem::LocalhostRejected => "music_assistant.localhost_rejected",
        MaProblem::Unreachable => "music_assistant.unreachable",
        MaProblem::StreamHint => "music_assistant.stream_hint",
        MaProblem::NotConfigured => "music_assistant.not_configured",
        MaProblem::AuthFailed => "music_assistant.auth_failed",
        MaProblem::Other => "music_assistant.command_failed",
    }
}

fn input_value(e: InputEvent) -> String {
    e.target_dyn_into::<HtmlInputElement>()
        .map(|input| input.value())
        .unwrap_or_default()
}

#[function_component(MusicAssistantSettings)]
pub fn music_assistant_settings() -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().and_then(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID);

    let enabled = use_state(|| false);
    let server_url = use_state(String::new);
    let external_url = use_state(String::new);
    let token = use_state(String::new);
    let has_token = use_state(|| false);
    let clear_token = use_state(|| false);
    let connected = use_state(|| false);
    let version = use_state(|| Option::<String>::None);
    let warning = use_state(|| Option::<String>::None);
    let last_error = use_state(|| Option::<String>::None);
    let saving = use_state(|| false);
    let testing = use_state(|| false);
    let test_result = use_state(|| Option::<TestExplanation>::None);
    let loaded = use_state(|| false);

    {
        let enabled = enabled.clone();
        let server_url = server_url.clone();
        let external_url = external_url.clone();
        let has_token = has_token.clone();
        let connected = connected.clone();
        let version = version.clone();
        let warning = warning.clone();
        let last_error = last_error.clone();
        let loaded = loaded.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id;
        use_effect_with((), move |_| {
            if let (Some(server_name), Some(api_key), Some(user_id)) = (server_name, api_key, user_id)
            {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(view) = get_settings(&server_name, &api_key, user_id).await {
                        apply_settings(&view);
                        enabled.set(view.enabled);
                        server_url.set(view.server_url.unwrap_or_default());
                        external_url.set(view.external_base_url.unwrap_or_default());
                        has_token.set(view.has_token);
                        connected.set(view.connected);
                        version.set(view.server_version);
                        warning.set(view.warning);
                        last_error.set(view.last_error);
                        loaded.set(true);
                    }
                });
            }
            || ()
        });
    }

    let on_enabled = {
        let enabled = enabled.clone();
        Callback::from(move |_: Event| enabled.set(!*enabled))
    };
    let on_server = {
        let server_url = server_url.clone();
        Callback::from(move |e: InputEvent| server_url.set(input_value(e)))
    };
    let on_external = {
        let external_url = external_url.clone();
        Callback::from(move |e: InputEvent| external_url.set(input_value(e)))
    };
    let on_token = {
        let token = token.clone();
        Callback::from(move |e: InputEvent| token.set(input_value(e)))
    };
    let on_clear = {
        let clear_token = clear_token.clone();
        Callback::from(move |_: Event| clear_token.set(!*clear_token))
    };

    let on_save = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id;
        let enabled = enabled.clone();
        let server_url = server_url.clone();
        let external_url = external_url.clone();
        let token = token.clone();
        let clear_token = clear_token.clone();
        let has_token = has_token.clone();
        let connected = connected.clone();
        let version = version.clone();
        let warning = warning.clone();
        let last_error = last_error.clone();
        let saving = saving.clone();
        let test_result = test_result.clone();
        let saved_msg = i18n.t("music_assistant.saved").to_string();
        let failed = i18n.t("music_assistant.command_failed").to_string();
        Callback::from(move |_: MouseEvent| {
            let (Some(server_name), Some(api_key), Some(user_id)) =
                (server_name.clone(), api_key.clone(), user_id)
            else {
                return;
            };
            let enabled_val = *enabled;
            let server_url = (*server_url).clone();
            let external_url = (*external_url).clone();
            let token_val = (*token).clone();
            let clear = *clear_token;
            let saving = saving.clone();
            let token_state = token.clone();
            let clear_state = clear_token.clone();
            let has_token = has_token.clone();
            let connected = connected.clone();
            let version = version.clone();
            let warning = warning.clone();
            let last_error = last_error.clone();
            let test_result = test_result.clone();
            let saved_msg = saved_msg.clone();
            let failed = failed.clone();
            saving.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                match save_settings(
                    &server_name,
                    &api_key,
                    user_id,
                    enabled_val,
                    &server_url,
                    &external_url,
                    &token_val,
                    clear,
                )
                .await
                {
                    Ok(view) => {
                        apply_settings(&view);
                        refresh_music_assistant();
                        has_token.set(view.has_token);
                        connected.set(view.connected);
                        version.set(view.server_version.clone());
                        warning.set(view.warning.clone());
                        last_error.set(view.last_error.clone());
                        token_state.set(String::new());
                        clear_state.set(false);
                        test_result.set(None);
                        Dispatch::<NotificationState>::global()
                            .reduce_mut(|s| s.info_message = Some(saved_msg));
                    }
                    Err(err) => {
                        let explained = explain_test(&external_url, Err(&err.to_string()));
                        test_result.set(Some(explained));
                        Dispatch::<NotificationState>::global()
                            .reduce_mut(|s| s.error_message = Some(failed));
                    }
                }
                saving.set(false);
            });
        })
    };

    let on_test = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id;
        let server_url = server_url.clone();
        let external_url = external_url.clone();
        let token = token.clone();
        let testing = testing.clone();
        let test_result = test_result.clone();
        Callback::from(move |_: MouseEvent| {
            let (Some(server_name), Some(api_key), Some(user_id)) =
                (server_name.clone(), api_key.clone(), user_id)
            else {
                return;
            };
            let server_url = (*server_url).clone();
            let external_url = (*external_url).clone();
            let token_val = (*token).clone();
            let testing = testing.clone();
            let test_result = test_result.clone();
            testing.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                // A rejected external URL is explained locally and left off the
                // probe, so the probe can still report whether MA itself is up.
                let probe_external = if crate::requests::music_assistant::external_base_is_rejected(
                    &external_url,
                ) {
                    ""
                } else {
                    external_url.as_str()
                };
                let explained = match test_connection(
                    &server_name,
                    &api_key,
                    user_id,
                    &server_url,
                    probe_external,
                    &token_val,
                )
                .await
                {
                    Ok(report) => explain_test(&external_url, Ok(&report)),
                    Err(err) => explain_test(&external_url, Err(&err.to_string())),
                };
                test_result.set(Some(explained));
                testing.set(false);
            });
        })
    };

    let token_placeholder = if *has_token {
        i18n.t("music_assistant.token_saved").to_string()
    } else {
        String::new()
    };
    let status = if *connected {
        i18n.t("music_assistant.connected").to_string()
    } else {
        i18n.t("music_assistant.not_connected").to_string()
    };

    html! {
        <>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("music_assistant.enabled") }</div>
                    <div class="settings-row-desc">{ i18n.t("music_assistant.enabled_desc") }</div>
                </div>
                <div class="settings-row-control">
                    <label class="toggle">
                        <input type="checkbox" checked={*enabled} onchange={on_enabled} />
                        <span class="toggle-track"><span class="toggle-thumb"></span></span>
                    </label>
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("music_assistant.server_url") }</div>
                    <div class="settings-row-desc">{ i18n.t("music_assistant.server_url_desc") }</div>
                    <input
                        type="url"
                        class="form-input w-full ma-field"
                        placeholder="http://192.168.1.20:8095"
                        value={(*server_url).clone()}
                        oninput={on_server}
                    />
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("music_assistant.token") }</div>
                    <div class="settings-row-desc">{ i18n.t("music_assistant.token_desc") }</div>
                    <input
                        type="password"
                        class="form-input w-full ma-field"
                        autocomplete="off"
                        placeholder={token_placeholder}
                        value={(*token).clone()}
                        oninput={on_token}
                    />
                    if *has_token {
                        <label class="ma-check">
                            <input type="checkbox" checked={*clear_token} onchange={on_clear} />
                            <span>{ i18n.t("music_assistant.clear_token") }</span>
                        </label>
                    }
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("music_assistant.external_base_url") }</div>
                    <div class="settings-row-desc">{ i18n.t("music_assistant.external_base_url_desc") }</div>
                    <input
                        type="url"
                        class="form-input w-full ma-field"
                        placeholder="http://192.168.1.10:8040"
                        value={(*external_url).clone()}
                        oninput={on_external}
                    />
                    <div class="ma-note">{ i18n.t("music_assistant.stream_hint") }</div>
                </div>
            </div>
            if *loaded {
                <div class="settings-row">
                    <div>
                        <div class="settings-row-label">{ status }</div>
                        <div class="settings-row-desc">
                            if let Some(v) = &*version {
                                { format!("{} {v}", i18n.t("music_assistant.version")) }
                            }
                            if let Some(w) = &*warning {
                                <div class="ma-note">{ format!("{} {w}", i18n.t("music_assistant.warning")) }</div>
                            }
                            if let Some(err) = &*last_error {
                                <div class="ma-note is-error">{ err.clone() }</div>
                            }
                        </div>
                    </div>
                </div>
            }
            if let Some(result) = &*test_result {
                <div class="settings-row">
                    <div>
                        <div class={classes!("ma-note", if result.ok { "is-ok" } else { "is-error" })}>
                            if result.ok {
                                { i18n.t("music_assistant.test_ok") }
                            }
                        </div>
                        { for result.problems.iter().copied().filter(|problem| *problem != MaProblem::StreamHint).map(|problem| html! {
                            <div class="ma-note is-error">{ i18n.t(problem_key(problem)) }</div>
                        }) }
                        if !result.detail.is_empty() {
                            <div class="settings-row-desc">{ result.detail.clone() }</div>
                        }
                        if result.show_stream_hint {
                            <div class="ma-note">{ i18n.t("music_assistant.stream_hint") }</div>
                        }
                    </div>
                </div>
            }
            <div class="settings-row">
                <div></div>
                <div class="settings-row-control" style="gap:8px;">
                    <button type="button" class="btn btn-secondary" onclick={on_test} disabled={*testing}>
                        <i class="ph ph-plugs-connected"></i>
                        <span>{ if *testing { i18n.t("music_assistant.testing") } else { i18n.t("music_assistant.test") } }</span>
                    </button>
                    <button type="button" class="btn btn-primary" onclick={on_save} disabled={*saving}>
                        <i class="ph ph-floppy-disk"></i>
                        <span>{ if *saving { i18n.t("music_assistant.saving") } else { i18n.t("music_assistant.save") } }</span>
                    </button>
                </div>
            </div>
        </>
    }
}
