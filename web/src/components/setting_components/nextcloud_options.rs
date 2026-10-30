// Podcast sync settings: the PinePods gpodder server (on/off, the URL and login for apps, app
// passwords, devices) and the external sync server (gpodder or Nextcloud) with its status.

use crate::components::context::{AppState, NotificationState};
use crate::pages::routes::Route;
use crate::requests::setting_reqs::{
    call_add_nextcloud_server, call_create_app_password, call_delete_sync_target,
    call_dismiss_sync_notice, call_get_sync_server, call_get_sync_stats, call_get_sync_target,
    call_list_app_passwords, call_list_sync_devices, call_remove_sync_device,
    call_rename_sync_device, call_revoke_app_password, call_run_sync, call_save_sync_target,
    call_set_sync_server, call_test_sync_target, initiate_nextcloud_login, AppPasswordEntry,
    CreatedAppPassword, NextcloudAuthRequest, SyncDeviceEntry, SyncServerInfo, SyncStats,
    SyncTargetCredentials, SyncTargetInfo, SyncTestResponse,
};
use i18nrs::yew::use_translation;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;
use yew_router::prelude::Link;
use yewdux::prelude::*;

/// How long the Nextcloud login flow is waited for (the server polls Nextcloud as long).
const NEXTCLOUD_LOGIN_WAIT_SECS: u32 = 20 * 60;

fn notify_error(message: String) {
    Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(message));
}

fn notify_info(message: String) {
    Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(message));
}

fn fmt_time(secs: Option<i64>, never: &str) -> String {
    match secs {
        Some(s) => js_sys::Date::new(&JsValue::from_f64(s as f64 * 1000.0))
            .to_locale_string("default", &JsValue::UNDEFINED)
            .as_string()
            .unwrap_or_default(),
        None => never.to_string(),
    }
}

fn confirm(message: &str) -> bool {
    web_sys::window().and_then(|w| w.confirm_with_message(message).ok()).unwrap_or(false)
}

fn copy_text(text: &str) {
    if let Some(w) = web_sys::window() {
        let _ = w.navigator().clipboard().write_text(text);
    }
}

fn input_setter(handle: UseStateHandle<String>) -> Callback<InputEvent> {
    Callback::from(move |e: InputEvent| {
        if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
            handle.set(input.value());
        }
    })
}

/// Server name and API key of the signed-in user.
#[hook]
fn use_auth() -> Option<(String, String)> {
    let (state, _) = use_store::<AppState>();
    state
        .auth_details
        .as_ref()
        .and_then(|a| a.api_key.clone().map(|k| (a.server_name.clone(), k)))
}

#[function_component(SyncOptions)]
pub fn sync_options() -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let server = use_state(|| None::<SyncServerInfo>);
    let target = use_state(|| None::<Option<SyncTargetInfo>>);
    let reload = use_state(|| 0u32);

    {
        let (auth, server, target) = (auth.clone(), server.clone(), target.clone());
        use_effect_with((*reload, auth.clone()), move |_| {
            if let Some((server_name, api_key)) = auth {
                spawn_local(async move {
                    match call_get_sync_server(&server_name, &api_key).await {
                        Ok(s) => server.set(Some(s)),
                        Err(e) => notify_error(e.to_string()),
                    }
                    match call_get_sync_target(&server_name, &api_key).await {
                        Ok(t) => target.set(Some(t.target)),
                        Err(e) => notify_error(e.to_string()),
                    }
                });
            }
            || ()
        });
    }

    let on_change = {
        let reload = reload.clone();
        Callback::from(move |_| reload.set(*reload + 1))
    };

    let dismiss_notice = {
        let (auth, on_change) = (auth.clone(), on_change.clone());
        Callback::from(move |_: MouseEvent| {
            if let Some((server_name, api_key)) = auth.clone() {
                let on_change = on_change.clone();
                spawn_local(async move {
                    if let Err(e) = call_dismiss_sync_notice(&server_name, &api_key).await {
                        notify_error(e.to_string());
                    }
                    on_change.emit(());
                });
            }
        })
    };

    let (Some(server_info), Some(target_info)) = ((*server).clone(), (*target).clone()) else {
        return html! { <div class="settings-row"><div class="settings-row-desc"><i class="ph ph-spinner animate-spin"></i></div></div> };
    };

    html! {
        <>
            if server_info.notice.as_deref() == Some("reenter_external") {
                <div class="sync-notice">
                    <i class="ph ph-warning"></i>
                    <div class="sync-notice-text">{ i18n.t("sync_settings.reenter_notice") }</div>
                    <button class="btn btn-ghost" onclick={dismiss_notice}>{ i18n.t("sync_settings.dismiss") }</button>
                </div>
            }
            <ServerSection server={server_info.clone()} on_change={on_change.clone()} />
            <ExternalSection target={target_info.clone()} on_change={on_change.clone()} />
            if server_info.enabled || target_info.is_some() {
                <StatsPanel reload_key={*reload} />
            }
        </>
    }
}

/// Shown on the home page while the external sync server has to be entered again; leads to the
/// sync settings.
#[function_component(SyncNoticeBanner)]
pub fn sync_notice_banner() -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let notice = use_state(|| None::<String>);

    {
        let (auth, notice) = (auth.clone(), notice.clone());
        use_effect_with(auth.clone(), move |_| {
            if let Some((server_name, api_key)) = auth {
                spawn_local(async move {
                    if let Ok(s) = call_get_sync_server(&server_name, &api_key).await {
                        notice.set(s.notice);
                    }
                });
            }
            || ()
        });
    }

    let dismiss = {
        let (auth, notice) = (auth.clone(), notice.clone());
        Callback::from(move |_: MouseEvent| {
            notice.set(None);
            if let Some((server_name, api_key)) = auth.clone() {
                spawn_local(async move {
                    let _ = call_dismiss_sync_notice(&server_name, &api_key).await;
                });
            }
        })
    };

    if notice.as_deref() != Some("reenter_external") {
        return html! {};
    }
    html! {
        <div class="sync-notice">
            <i class="ph ph-warning"></i>
            <div class="sync-notice-text">{ i18n.t("sync_settings.reenter_notice") }</div>
            <Link<Route> to={Route::Settings} classes="btn btn-secondary">{ i18n.t("sync_settings.open_settings") }</Link<Route>>
            <button class="btn btn-ghost" onclick={dismiss}>{ i18n.t("sync_settings.dismiss") }</button>
        </div>
    }
}

// ---- the PinePods gpodder server -----------------------------------------------------------------

#[derive(Properties, PartialEq)]
struct ServerProps {
    server: SyncServerInfo,
    on_change: Callback<()>,
}

#[function_component(ServerSection)]
fn server_section(props: &ServerProps) -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let toggling = use_state(|| false);
    let server = &props.server;

    let on_toggle = {
        let (auth, toggling, on_change, enabled) = (auth.clone(), toggling.clone(), props.on_change.clone(), server.enabled);
        Callback::from(move |_: MouseEvent| {
            let Some((server_name, api_key)) = auth.clone() else { return };
            let (toggling, on_change) = (toggling.clone(), on_change.clone());
            toggling.set(true);
            spawn_local(async move {
                if let Err(e) = call_set_sync_server(&server_name, &api_key, !enabled).await {
                    notify_error(e.to_string());
                }
                toggling.set(false);
                on_change.emit(());
            });
        })
    };

    // Without a configured public URL, the address this browser reached Pinepods at is the best guess.
    let url = if server.server_url_configured {
        server.server_url.clone()
    } else {
        auth.as_ref().map(|(s, _)| s.trim_end_matches('/').to_string()).unwrap_or_else(|| server.server_url.clone())
    };
    let copy_url = {
        let url = url.clone();
        let copied = i18n.t("sync_settings.copied").to_string();
        Callback::from(move |_: MouseEvent| {
            copy_text(&url);
            notify_info(copied.clone());
        })
    };

    html! {
        <>
            <div class="settings-subsection-title">{ i18n.t("sync_settings.server_title") }</div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("sync_settings.server_toggle") }</div>
                    <div class="settings-row-desc">{ i18n.t("sync_settings.server_desc") }</div>
                </div>
                <div class="settings-row-control">
                    <label class="toggle">
                        <input type="checkbox" checked={server.enabled} disabled={*toggling} onclick={on_toggle} />
                        <span class="toggle-track"><span class="toggle-thumb"></span></span>
                    </label>
                    if *toggling {
                        <i class="ph ph-spinner animate-spin"></i>
                    }
                </div>
            </div>
            if server.enabled {
                <div class="settings-row">
                    <div>
                        <div class="settings-row-label">{ i18n.t("sync_settings.server_url") }</div>
                        <div class="settings-row-desc">{ i18n.t("sync_settings.server_url_hint") }</div>
                    </div>
                    <div class="settings-row-control">
                        <code class="sync-code">{ url.clone() }</code>
                        <button class="btn btn-ghost" onclick={copy_url} title={i18n.t("sync_settings.copy").to_string()}>
                            <i class="ph ph-copy"></i>
                        </button>
                    </div>
                </div>
                <div class="settings-row">
                    <div>
                        <div class="settings-row-label">{ i18n.t("sync_settings.username") }</div>
                        <div class="settings-row-desc">{ i18n.t("sync_settings.password_hint") }</div>
                    </div>
                    <div class="settings-row-control"><code class="sync-code">{ server.username.clone() }</code></div>
                </div>
                <AppPasswords />
                <ServerDevices />
            }
        </>
    }
}

#[function_component(AppPasswords)]
fn app_passwords() -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let list = use_state(Vec::<AppPasswordEntry>::new);
    let label = use_state(String::new);
    let created = use_state(|| None::<CreatedAppPassword>);
    let reload = use_state(|| 0u32);

    {
        let (auth, list) = (auth.clone(), list.clone());
        use_effect_with(*reload, move |_| {
            if let Some((server_name, api_key)) = auth {
                spawn_local(async move {
                    match call_list_app_passwords(&server_name, &api_key).await {
                        Ok(l) => list.set(l),
                        Err(e) => notify_error(e.to_string()),
                    }
                });
            }
            || ()
        });
    }

    let generate = {
        let (auth, label, created, reload) = (auth.clone(), label.clone(), created.clone(), reload.clone());
        Callback::from(move |_: MouseEvent| {
            let Some((server_name, api_key)) = auth.clone() else { return };
            let (label, created, reload) = (label.clone(), created.clone(), reload.clone());
            spawn_local(async move {
                match call_create_app_password(&server_name, &api_key, label.trim()).await {
                    Ok(p) => {
                        created.set(Some(p));
                        label.set(String::new());
                        reload.set(*reload + 1);
                    }
                    Err(e) => notify_error(e.to_string()),
                }
            });
        })
    };

    let revoke = {
        let (auth, reload) = (auth.clone(), reload.clone());
        let question = i18n.t("sync_settings.revoke_confirm").to_string();
        Callback::from(move |id: i32| {
            if !confirm(&question) {
                return;
            }
            let Some((server_name, api_key)) = auth.clone() else { return };
            let reload = reload.clone();
            spawn_local(async move {
                if let Err(e) = call_revoke_app_password(&server_name, &api_key, id).await {
                    notify_error(e.to_string());
                }
                reload.set(*reload + 1);
            });
        })
    };

    let close = {
        let created = created.clone();
        Callback::from(move |_: MouseEvent| created.set(None))
    };
    let never = i18n.t("sync_settings.never").to_string();

    html! {
        <>
            if let Some(p) = (*created).clone() {
                <div class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-full bg-black bg-opacity-25">
                    <div class="modal-container relative p-4 w-full max-w-md rounded-lg shadow">
                        <div class="flex flex-col items-start p-4 md:p-5">
                            <h3 style="font-size:16px;font-weight:600;color:var(--text-color);margin-bottom:8px;">
                                { i18n.t("sync_settings.app_password_created") }
                            </h3>
                            <p style="font-size:13px;color:var(--text-secondary-color);">{ i18n.t("sync_settings.app_password_shown_once") }</p>
                            <p style="font-size:13px;color:var(--text-secondary-color);margin-top:4px;">{ p.label.clone() }</p>
                            <div class="mfa-code-box mt-4 p-4 rounded-md overflow-x-auto whitespace-nowrap max-w-full">{ p.password.clone() }</div>
                            <div class="flex" style="gap:8px;margin-top:16px;">
                                <button class="btn btn-secondary" onclick={{
                                    let password = p.password.clone();
                                    let copied = i18n.t("sync_settings.copied").to_string();
                                    Callback::from(move |_: MouseEvent| { copy_text(&password); notify_info(copied.clone()); })
                                }}>
                                    <i class="ph ph-copy"></i>{ i18n.t("sync_settings.copy") }
                                </button>
                                <button class="btn btn-primary" onclick={close}>{ i18n.t("sync_settings.done") }</button>
                            </div>
                        </div>
                    </div>
                </div>
            }
            <div class="settings-section-subhead">{ i18n.t("sync_settings.app_passwords") }</div>
            <div class="settings-row">
                <div><div class="settings-row-desc">{ i18n.t("sync_settings.app_passwords_desc") }</div></div>
                <div class="settings-row-control">
                    <input type="text" class="input" style="width:220px;max-width:60vw;" value={(*label).clone()}
                        placeholder={i18n.t("sync_settings.app_password_label_placeholder").to_string()} oninput={input_setter(label.clone())} />
                    <button class="btn btn-secondary" onclick={generate}>
                        <i class="ph ph-key"></i>{ i18n.t("sync_settings.generate_app_password") }
                    </button>
                </div>
            </div>
            if list.is_empty() {
                <div class="settings-row"><div class="settings-row-desc">{ i18n.t("sync_settings.no_app_passwords") }</div></div>
            }
            { for list.iter().map(|p| {
                let id = p.id;
                let revoke = revoke.clone();
                html! {
                    <div class="settings-row">
                        <div>
                            <div class="settings-row-label">{ p.label.clone() }</div>
                            <div class="settings-row-desc">
                                { format!("{} {} · {} {}",
                                    i18n.t("sync_settings.created"), fmt_time(p.created_at, &never),
                                    i18n.t("sync_settings.last_used"), fmt_time(p.last_used_at, &never)) }
                            </div>
                        </div>
                        <div class="settings-row-control">
                            <button class="btn btn-ghost" onclick={Callback::from(move |_: MouseEvent| revoke.emit(id))}>
                                <i class="ph ph-trash"></i>{ i18n.t("sync_settings.revoke") }
                            </button>
                        </div>
                    </div>
                }
            }) }
        </>
    }
}

#[function_component(ServerDevices)]
fn server_devices() -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let devices = use_state(Vec::<SyncDeviceEntry>::new);
    let editing = use_state(|| None::<i32>);
    let caption = use_state(String::new);
    let reload = use_state(|| 0u32);

    {
        let (auth, devices) = (auth.clone(), devices.clone());
        use_effect_with(*reload, move |_| {
            if let Some((server_name, api_key)) = auth {
                spawn_local(async move {
                    match call_list_sync_devices(&server_name, &api_key).await {
                        Ok(d) => devices.set(d),
                        Err(e) => notify_error(e.to_string()),
                    }
                });
            }
            || ()
        });
    }

    let save = {
        let (auth, editing, caption, reload) = (auth.clone(), editing.clone(), caption.clone(), reload.clone());
        Callback::from(move |_: MouseEvent| {
            let (Some((server_name, api_key)), Some(id)) = (auth.clone(), *editing) else { return };
            let (editing, caption, reload) = (editing.clone(), caption.clone(), reload.clone());
            spawn_local(async move {
                match call_rename_sync_device(&server_name, &api_key, id, caption.trim()).await {
                    Ok(_) => editing.set(None),
                    Err(e) => notify_error(e.to_string()),
                }
                reload.set(*reload + 1);
            });
        })
    };
    let cancel = {
        let editing = editing.clone();
        Callback::from(move |_: MouseEvent| editing.set(None))
    };
    let remove = {
        let (auth, reload) = (auth.clone(), reload.clone());
        let question = i18n.t("sync_settings.remove_device_confirm").to_string();
        Callback::from(move |id: i32| {
            if !confirm(&question) {
                return;
            }
            let Some((server_name, api_key)) = auth.clone() else { return };
            let reload = reload.clone();
            spawn_local(async move {
                if let Err(e) = call_remove_sync_device(&server_name, &api_key, id).await {
                    notify_error(e.to_string());
                }
                reload.set(*reload + 1);
            });
        })
    };
    let never = i18n.t("sync_settings.never").to_string();
    let apps = devices.iter().filter(|d| !d.protected).count();

    html! {
        <>
            <div class="settings-section-subhead">{ i18n.t("sync_settings.devices") }</div>
            <div class="settings-row"><div><div class="settings-row-desc">
                { if apps == 0 { i18n.t("sync_settings.no_devices") } else { i18n.t("sync_settings.devices_desc") } }
            </div></div></div>
            { for devices.iter().map(|d| {
                let id = d.id;
                let title = if d.protected { i18n.t("sync_settings.this_server").to_string() } else { d.caption.clone() };
                let start_edit = {
                    let (editing, caption, current) = (editing.clone(), caption.clone(), d.caption.clone());
                    Callback::from(move |_: MouseEvent| { caption.set(current.clone()); editing.set(Some(id)); })
                };
                let remove = remove.clone();
                html! {
                    <div class="settings-row">
                        <div>
                            if *editing == Some(id) {
                                <input type="text" class="input" style="width:220px;max-width:60vw;" value={(*caption).clone()} oninput={input_setter(caption.clone())} />
                            } else {
                                <div class="settings-row-label">{ title }</div>
                            }
                            <div class="settings-row-desc">
                                { format!("{} · {} · {} {}", d.name, d.kind, i18n.t("sync_settings.last_seen"), fmt_time(d.last_seen, &never)) }
                            </div>
                        </div>
                        if !d.protected {
                            <div class="settings-row-control">
                                if *editing == Some(id) {
                                    <button class="btn btn-primary" onclick={save.clone()}>{ i18n.t("sync_settings.save") }</button>
                                    <button class="btn btn-ghost" onclick={cancel.clone()}>{ i18n.t("sync_settings.cancel") }</button>
                                } else {
                                    <button class="btn btn-ghost" onclick={start_edit}><i class="ph ph-pencil-simple"></i>{ i18n.t("sync_settings.rename") }</button>
                                    <button class="btn btn-ghost" onclick={Callback::from(move |_: MouseEvent| remove.emit(id))}>
                                        <i class="ph ph-trash"></i>{ i18n.t("sync_settings.remove") }
                                    </button>
                                }
                            </div>
                        }
                    </div>
                }
            }) }
        </>
    }
}

// ---- the external sync server --------------------------------------------------------------------

#[derive(Properties, PartialEq)]
struct ExternalProps {
    target: Option<SyncTargetInfo>,
    on_change: Callback<()>,
}

#[function_component(ExternalSection)]
fn external_section(props: &ExternalProps) -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let (app_state, _) = use_store::<AppState>();
    let user_id = app_state.user_details.as_ref().map(|u| u.UserID);
    let current = props.target.clone();

    let kind = use_state(|| current.as_ref().map(|t| t.kind.clone()).unwrap_or_else(|| "none".into()));
    let url = use_state(|| current.as_ref().map(|t| t.url.clone()).unwrap_or_default());
    let username = use_state(|| current.as_ref().map(|t| t.username.clone()).unwrap_or_default());
    let password = use_state(String::new);
    let device = use_state(|| current.as_ref().and_then(|t| t.device_name.clone()).unwrap_or_default());
    let busy = use_state(|| None::<&'static str>);
    let tested = use_state(|| None::<SyncTestResponse>);

    // Follow the saved target when it changes (saved, removed, Nextcloud login finished).
    {
        let (kind, url, username, device, password) = (kind.clone(), url.clone(), username.clone(), device.clone(), password.clone());
        use_effect_with(current.as_ref().map(|t| (t.kind.clone(), t.url.clone(), t.username.clone(), t.device_name.clone())), move |saved| {
            if let Some((k, u, n, d)) = saved.clone() {
                kind.set(k);
                url.set(u);
                username.set(n);
                device.set(d.unwrap_or_default());
                password.set(String::new());
            }
            || ()
        });
    }

    let creds = {
        let (kind, url, username, password, device) = (kind.clone(), url.clone(), username.clone(), password.clone(), device.clone());
        move || SyncTargetCredentials {
            kind: (*kind).clone(),
            url: url.trim().to_string(),
            username: username.trim().to_string(),
            password: Some((*password).clone()).filter(|p| !p.is_empty()),
            device_name: Some(device.trim().to_string()).filter(|d| !d.is_empty() && *kind == "gpodder"),
        }
    };

    let on_kind = {
        let (kind, tested) = (kind.clone(), tested.clone());
        Callback::from(move |e: Event| {
            if let Some(sel) = e.target_dyn_into::<HtmlSelectElement>() {
                kind.set(sel.value());
                tested.set(None);
            }
        })
    };

    let on_test = {
        let (auth, busy, tested, creds) = (auth.clone(), busy.clone(), tested.clone(), creds.clone());
        Callback::from(move |_: MouseEvent| {
            let Some((server_name, api_key)) = auth.clone() else { return };
            let (busy, tested, c) = (busy.clone(), tested.clone(), creds());
            busy.set(Some("test"));
            spawn_local(async move {
                match call_test_sync_target(&server_name, &api_key, &c).await {
                    Ok(r) => tested.set(Some(r)),
                    Err(e) => notify_error(e.to_string()),
                }
                busy.set(None);
            });
        })
    };

    let on_save = {
        let (auth, busy, creds, on_change) = (auth.clone(), busy.clone(), creds.clone(), props.on_change.clone());
        let saved_msg = i18n.t("sync_settings.saved").to_string();
        Callback::from(move |_: MouseEvent| {
            let Some((server_name, api_key)) = auth.clone() else { return };
            let (busy, c, on_change, saved_msg) = (busy.clone(), creds(), on_change.clone(), saved_msg.clone());
            busy.set(Some("save"));
            spawn_local(async move {
                match call_save_sync_target(&server_name, &api_key, &c).await {
                    Ok(_) => notify_info(saved_msg),
                    Err(e) => notify_error(e.to_string()),
                }
                busy.set(None);
                on_change.emit(());
            });
        })
    };

    let on_remove = {
        let (auth, on_change) = (auth.clone(), props.on_change.clone());
        let question = i18n.t("sync_settings.remove_external_confirm").to_string();
        Callback::from(move |_: MouseEvent| {
            if !confirm(&question) {
                return;
            }
            let Some((server_name, api_key)) = auth.clone() else { return };
            let on_change = on_change.clone();
            spawn_local(async move {
                if let Err(e) = call_delete_sync_target(&server_name, &api_key).await {
                    notify_error(e.to_string());
                }
                on_change.emit(());
            });
        })
    };

    // Nextcloud login flow: approve Pinepods in a new tab; the server polls Nextcloud and saves
    // the target, which this page waits for.
    let on_nextcloud_login = {
        let (auth, busy, url, on_change) = (auth.clone(), busy.clone(), url.clone(), props.on_change.clone());
        let connected = i18n.t("sync_settings.nextcloud_connected").to_string();
        Callback::from(move |_: MouseEvent| {
            let (Some((server_name, api_key)), Some(user_id)) = (auth.clone(), user_id) else { return };
            let nextcloud_url = url.trim().trim_end_matches('/').to_string();
            if nextcloud_url.is_empty() {
                return;
            }
            let (busy, on_change, connected) = (busy.clone(), on_change.clone(), connected.clone());
            busy.set(Some("nextcloud"));
            spawn_local(async move {
                let started = async {
                    let login = initiate_nextcloud_login(&nextcloud_url, &server_name, &api_key, user_id).await?;
                    if let Some(w) = web_sys::window() {
                        let _ = w.open_with_url_and_target(&login.login, "_blank");
                    }
                    let request = NextcloudAuthRequest {
                        user_id,
                        token: login.poll.token,
                        poll_endpoint: login.poll.endpoint,
                        nextcloud_url: nextcloud_url.clone(),
                    };
                    call_add_nextcloud_server(&server_name, &api_key, request).await?;
                    Ok::<(), anyhow::Error>(())
                }
                .await;
                if let Err(e) = started {
                    notify_error(e.to_string());
                    busy.set(None);
                    return;
                }
                for _ in 0..(NEXTCLOUD_LOGIN_WAIT_SECS / 3) {
                    gloo_timers::future::TimeoutFuture::new(3_000).await;
                    if let Ok(r) = call_get_sync_target(&server_name, &api_key).await {
                        if r.target.is_some_and(|t| t.kind == "nextcloud" && t.url.trim_end_matches('/') == nextcloud_url) {
                            notify_info(connected);
                            break;
                        }
                    }
                }
                busy.set(None);
                on_change.emit(());
            });
        })
    };

    let run = {
        let (auth, busy, on_change) = (auth.clone(), busy.clone(), props.on_change.clone());
        let (done, failed) = (i18n.t("sync_settings.sync_done").to_string(), i18n.t("sync_settings.sync_failed").to_string());
        Callback::from(move |full: bool| {
            let Some((server_name, api_key)) = auth.clone() else { return };
            let (busy, on_change, done, failed) = (busy.clone(), on_change.clone(), done.clone(), failed.clone());
            busy.set(Some(if full { "full" } else { "run" }));
            spawn_local(async move {
                match call_run_sync(&server_name, &api_key, full).await {
                    Ok(r) if r.success => notify_info(done),
                    Ok(r) => notify_error(format!("{}{}", failed, r.message.unwrap_or_default())),
                    Err(e) => notify_error(format!("{}{}", failed, e)),
                }
                busy.set(None);
                on_change.emit(());
            });
        })
    };

    let never = i18n.t("sync_settings.never").to_string();
    let spinner = |what: &str| -> Html {
        if *busy == Some(what) { html! { <i class="ph ph-spinner animate-spin"></i> } } else { html! {} }
    };
    let is_busy = busy.is_some();
    let kind_now = (*kind).clone();
    let saved_same_kind = current.as_ref().is_some_and(|t| t.kind == kind_now);

    html! {
        <>
            <div class="settings-subsection-title">{ i18n.t("sync_settings.external_title") }</div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("sync_settings.external_kind") }</div>
                    <div class="settings-row-desc">{ i18n.t("sync_settings.external_desc") }</div>
                </div>
                <div class="settings-row-control">
                    <select class="select" style="width:220px;max-width:60vw;" onchange={on_kind}>
                        <option value="none" selected={kind_now == "none"}>{ i18n.t("sync_settings.external_none") }</option>
                        <option value="gpodder" selected={kind_now == "gpodder"}>{ i18n.t("sync_settings.external_gpodder") }</option>
                        <option value="nextcloud" selected={kind_now == "nextcloud"}>{ i18n.t("sync_settings.external_nextcloud") }</option>
                    </select>
                </div>
            </div>

            if kind_now == "none" {
                if current.is_some() {
                    <div class="settings-row">
                        <div><div class="settings-row-desc">{ i18n.t("sync_settings.remove_external_hint") }</div></div>
                        <div class="settings-row-control">
                            <button class="btn btn-danger" onclick={on_remove.clone()}><i class="ph ph-trash"></i>{ i18n.t("sync_settings.remove") }</button>
                        </div>
                    </div>
                }
            } else {
                <div class="settings-row">
                    <div><div class="settings-row-label">{ i18n.t("sync_settings.server_url") }</div></div>
                    <div class="settings-row-control">
                        <input type="text" class="input" style="width:280px;max-width:60vw;" value={(*url).clone()} oninput={input_setter(url.clone())}
                            placeholder={ if kind_now == "nextcloud" { "https://cloud.example.com" } else { "https://gpodder.example.com" } } />
                    </div>
                </div>
                if kind_now == "nextcloud" {
                    <div class="settings-row">
                        <div><div class="settings-row-desc">{ i18n.t("sync_settings.nextcloud_login_hint") }</div></div>
                        <div class="settings-row-control">
                            <button class="btn btn-secondary" disabled={is_busy || url.trim().is_empty()} onclick={on_nextcloud_login}>
                                { spinner("nextcloud") }<i class="ph ph-cloud"></i>
                                { if *busy == Some("nextcloud") { i18n.t("sync_settings.waiting_for_nextcloud") } else { i18n.t("sync_settings.nextcloud_login") } }
                            </button>
                        </div>
                    </div>
                }
                <div class="settings-row">
                    <div><div class="settings-row-label">{ i18n.t("sync_settings.username") }</div></div>
                    <div class="settings-row-control">
                        <input type="text" class="input" style="width:280px;max-width:60vw;" value={(*username).clone()} oninput={input_setter(username.clone())} />
                    </div>
                </div>
                <div class="settings-row">
                    <div><div class="settings-row-label">{ if kind_now == "nextcloud" { i18n.t("sync_settings.app_password") } else { i18n.t("sync_settings.password") } }</div></div>
                    <div class="settings-row-control">
                        <input type="password" class="input" style="width:280px;max-width:60vw;" value={(*password).clone()} oninput={input_setter(password.clone())}
                            placeholder={ if saved_same_kind { i18n.t("sync_settings.password_keep").to_string() } else { String::new() } } />
                    </div>
                </div>
                if kind_now == "gpodder" {
                    <div class="settings-row">
                        <div>
                            <div class="settings-row-label">{ i18n.t("sync_settings.device_name") }</div>
                            <div class="settings-row-desc">{ i18n.t("sync_settings.device_name_hint") }</div>
                        </div>
                        <div class="settings-row-control">
                            <input type="text" class="input" style="width:280px;max-width:60vw;" value={(*device).clone()} oninput={input_setter(device.clone())} placeholder="pinepods" />
                        </div>
                    </div>
                }
                if let Some(t) = (*tested).clone() {
                    <div class="settings-row">
                        <div>
                            if t.success {
                                <div class="settings-row-label" style="color:var(--success-color);">{ i18n.t("sync_settings.connection_ok") }</div>
                                if !t.devices.is_empty() {
                                    <div class="settings-row-desc">
                                        { format!("{}{}", i18n.t("sync_settings.remote_devices"), t.devices.iter().map(|d| d.caption.clone()).collect::<Vec<_>>().join(", ")) }
                                    </div>
                                }
                                if let Some(name) = t.device_name.clone() {
                                    <div class="settings-row-desc">{ format!("{}{}", i18n.t("sync_settings.syncs_as"), name) }</div>
                                }
                            } else {
                                <div class="settings-row-label" style="color:var(--error-color);">{ format!("{}{}", i18n.t("sync_settings.connection_failed"), t.message) }</div>
                            }
                        </div>
                    </div>
                }
                <div class="settings-row">
                    <div></div>
                    <div class="settings-row-control">
                        <button class="btn btn-secondary" disabled={is_busy} onclick={on_test}>
                            { spinner("test") }<i class="ph ph-check-circle"></i>{ i18n.t("sync_settings.test_connection") }
                        </button>
                        <button class="btn btn-primary" disabled={is_busy} onclick={on_save}>
                            { spinner("save") }<i class="ph ph-floppy-disk"></i>{ i18n.t("sync_settings.save_and_sync") }
                        </button>
                    </div>
                </div>
            }

            if let Some(t) = current.clone() {
                <div class="settings-section-subhead">{ i18n.t("sync_settings.status") }</div>
                <div class="settings-row">
                    <div>
                        <div class="settings-row-label">{ format!("{} · {}", t.url, t.username) }</div>
                        <div class="settings-row-desc">{ format!("{} {}", i18n.t("sync_settings.last_success"), fmt_time(t.last_success_at, &never)) }</div>
                        if let Some(err) = t.last_error.clone() {
                            <div class="settings-row-desc" style="color:var(--error-color);">{ format!("{}{}", i18n.t("sync_settings.last_error"), err) }</div>
                        }
                        if t.in_backoff {
                            <div class="settings-row-desc">
                                { i18n.t("sync_settings.backoff")
                                    .replace("{count}", &t.consecutive_failures.to_string())
                                    .replace("{time}", &fmt_time(t.retry_at, &never)) }
                            </div>
                        }
                        if t.needs_full_sync {
                            <div class="settings-row-desc">{ i18n.t("sync_settings.full_sync_pending") }</div>
                        }
                    </div>
                    <div class="settings-row-control">
                        <button class="btn btn-secondary" disabled={is_busy} onclick={{ let run = run.clone(); Callback::from(move |_: MouseEvent| run.emit(false)) }}>
                            { spinner("run") }<i class="ph ph-arrows-clockwise"></i>{ i18n.t("sync_settings.sync_now") }
                        </button>
                        <button class="btn btn-ghost" disabled={is_busy} onclick={{ let run = run.clone(); Callback::from(move |_: MouseEvent| run.emit(true)) }}>
                            { spinner("full") }{ i18n.t("sync_settings.force_full_sync") }
                        </button>
                    </div>
                </div>
            }
        </>
    }
}

// ---- statistics -----------------------------------------------------------------------------------

#[derive(Properties, PartialEq)]
struct StatsProps {
    reload_key: u32,
}

#[function_component(StatsPanel)]
fn stats_panel(props: &StatsProps) -> Html {
    let (i18n, _) = use_translation();
    let auth = use_auth();
    let open = use_state(|| false);
    let stats = use_state(|| None::<SyncStats>);
    let refresh = use_state(|| 0u32);

    {
        let (auth, stats, open) = (auth.clone(), stats.clone(), open.clone());
        use_effect_with((*open, *refresh, props.reload_key), move |_| {
            if let (true, Some((server_name, api_key))) = (*open, auth) {
                spawn_local(async move {
                    match call_get_sync_stats(&server_name, &api_key).await {
                        Ok(s) => stats.set(Some(s)),
                        Err(e) => notify_error(e.to_string()),
                    }
                });
            }
            || ()
        });
    }

    let toggle = {
        let open = open.clone();
        Callback::from(move |_: MouseEvent| open.set(!*open))
    };
    let on_refresh = {
        let refresh = refresh.clone();
        Callback::from(move |_: MouseEvent| refresh.set(*refresh + 1))
    };
    let never = i18n.t("sync_settings.never").to_string();
    let origin_label = |origin: &str, device: Option<&String>| -> String {
        match origin {
            "pinepods" => "PinePods".to_string(),
            "target" => i18n.t("sync_settings.origin_external").to_string(),
            _ => device.cloned().unwrap_or_else(|| i18n.t("sync_settings.origin_app").to_string()),
        }
    };

    html! {
        <>
            <div class="settings-row">
                <div><div class="settings-row-label">{ i18n.t("sync_settings.stats_title") }</div></div>
                <div class="settings-row-control">
                    if *open {
                        <button class="btn btn-ghost" onclick={on_refresh} title={i18n.t("sync_settings.refresh").to_string()}><i class="ph ph-arrows-clockwise"></i></button>
                    }
                    <button class="btn btn-ghost" onclick={toggle}>
                        <i class={if *open { "ph ph-caret-up" } else { "ph ph-caret-down" }}></i>
                    </button>
                </div>
            </div>
            if *open {
                if let Some(s) = (*stats).clone() {
                    <div class="sync-stats-grid">
                        <div class="sync-stat"><div class="sync-stat-value">{ s.subscriptions }</div><div class="sync-stat-label">{ i18n.t("sync_settings.stats_subscriptions") }</div></div>
                        <div class="sync-stat"><div class="sync-stat-value">{ s.total_episode_actions }</div><div class="sync-stat-label">{ i18n.t("sync_settings.stats_actions") }</div></div>
                        if s.server_enabled {
                            <div class="sync-stat"><div class="sync-stat-value">{ s.devices }</div><div class="sync-stat-label">{ i18n.t("sync_settings.devices") }</div></div>
                        }
                        if let Some(t) = s.target.clone() {
                            <div class="sync-stat">
                                <div class="sync-stat-value">
                                    { match (t.pending_subscriptions, t.pending_actions) {
                                        (Some(a), Some(b)) => (a + b).to_string(),
                                        _ => "–".to_string(),
                                    } }
                                </div>
                                <div class="sync-stat-label">{ i18n.t("sync_settings.stats_pending") }</div>
                            </div>
                        }
                    </div>
                    <div class="settings-section-subhead">{ i18n.t("sync_settings.recent_actions") }</div>
                    if s.recent_episode_actions.is_empty() {
                        <div class="settings-row"><div class="settings-row-desc">{ i18n.t("sync_settings.no_actions") }</div></div>
                    }
                    { for s.recent_episode_actions.iter().map(|a| html! {
                        <div class="settings-row">
                            <div>
                                <div class="settings-row-label" style="overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">{ a.episode.clone() }</div>
                                <div class="settings-row-desc">
                                    { format!("{} · {} · {}", a.action, origin_label(&a.origin, a.device.as_ref()), fmt_time(Some(a.timestamp), &never)) }
                                </div>
                            </div>
                        </div>
                    }) }
                } else {
                    <div class="settings-row"><div class="settings-row-desc"><i class="ph ph-spinner animate-spin"></i></div></div>
                }
            }
        </>
    }
}
