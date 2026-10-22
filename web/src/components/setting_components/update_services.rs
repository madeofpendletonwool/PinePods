// Admin: the search service's status and Podping (on/off, whether the relay keeps up, what this
// server did with its announcements).

use crate::components::context::{AppState, NotificationState};
use crate::requests::setting_reqs::{call_get_update_services, call_set_podping, UpdateServices};
use i18nrs::yew::use_translation;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

fn notify_error(message: String) {
    Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(message));
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

#[function_component(UpdateServicesSection)]
pub fn update_services_section() -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let auth = state
        .auth_details
        .as_ref()
        .and_then(|a| a.api_key.clone().map(|k| (a.server_name.clone(), k)));
    let info = use_state(|| None::<UpdateServices>);
    let toggling = use_state(|| false);
    let reload = use_state(|| 0u32);

    {
        let (info, auth) = (info.clone(), auth.clone());
        use_effect_with((auth.clone(), *reload), move |_| {
            if let Some((server, key)) = auth {
                spawn_local(async move {
                    match call_get_update_services(&server, &key).await {
                        Ok(i) => info.set(Some(i)),
                        Err(e) => notify_error(e.to_string()),
                    }
                });
            }
            || ()
        });
    }

    let Some(i) = (*info).clone() else {
        return html! { <div class="settings-row"><div class="settings-row-desc"><i class="ph ph-spinner animate-spin"></i></div></div> };
    };
    let never = i18n.t("update_services.never").to_string();

    let on_toggle = {
        let (auth, info, toggling) = (auth.clone(), info.clone(), toggling.clone());
        let enable = !i.podping.enabled;
        Callback::from(move |_: MouseEvent| {
            let Some((server, key)) = auth.clone() else { return };
            let (info, toggling) = (info.clone(), toggling.clone());
            toggling.set(true);
            spawn_local(async move {
                match call_set_podping(&server, &key, enable).await {
                    Ok(i) => info.set(Some(i)),
                    Err(e) => notify_error(e.to_string()),
                }
                toggling.set(false);
            });
        })
    };
    let refresh = {
        let reload = reload.clone();
        Callback::from(move |_: MouseEvent| reload.set(*reload + 1))
    };

    let search = &i.search;
    let search_state = if search.reachable {
        i18n.t("update_services.reachable").replace("{ms}", &search.latency_ms.unwrap_or(0).to_string())
    } else {
        format!("{} {}", i18n.t("update_services.unreachable"), search.error.clone().unwrap_or_default())
    };
    let p = &i.podping;
    let following = p.relay.as_ref().and_then(|r| r.get("following")).and_then(|f| f.as_bool()).unwrap_or(false);
    let relay_state = match (&p.relay, &p.relay_error) {
        (Some(_), _) if following => i18n.t("update_services.relay_following").to_string(),
        (Some(_), _) => i18n.t("update_services.relay_behind").to_string(),
        (None, Some(e)) => format!("{} {}", i18n.t("update_services.relay_unavailable"), e),
        (None, None) => i18n.t("update_services.relay_unavailable").to_string(),
    };

    html! {
        <>
            <div class="settings-row">
                <div style="min-width:0">
                    <div class="settings-row-label">{ i18n.t("update_services.search_title") }</div>
                    <div class="settings-row-desc" style="overflow-wrap:anywhere">{ &search.url }</div>
                    <div class="settings-row-desc">
                        <i class={classes!("ph", if search.reachable { "ph-check-circle" } else { "ph-warning-circle" })}></i>
                        { format!(" {search_state}") }
                    </div>
                </div>
                <div class="settings-row-control">
                    <button class="btn btn-ghost" onclick={refresh}><i class="ph ph-arrows-clockwise"></i>{ i18n.t("update_services.check_again") }</button>
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ i18n.t("update_services.podping_toggle") }</div>
                    <div class="settings-row-desc">{ i18n.t("update_services.podping_desc") }</div>
                </div>
                <div class="settings-row-control">
                    <label class="toggle">
                        <input type="checkbox" checked={p.enabled} disabled={*toggling} onclick={on_toggle} />
                        <span class="toggle-track"><span class="toggle-thumb"></span></span>
                    </label>
                    if *toggling {
                        <i class="ph ph-spinner animate-spin"></i>
                    }
                </div>
            </div>
            if p.enabled {
                <div class="settings-row">
                    <div style="min-width:0">
                        <div class="settings-row-desc" style="overflow-wrap:anywhere">{ format!("{} {}", i18n.t("update_services.relay"), p.relay_url) }</div>
                        <div class="settings-row-desc">{ relay_state }</div>
                        <div class="settings-row-desc">
                            { i18n.t("update_services.client_stats")
                                .replace("{pings}", &p.client.pings_seen.to_string())
                                .replace("{feeds}", &p.client.feeds_refreshed.to_string())
                                .replace("{time}", &fmt_time(p.client.last_success_at, &never)) }
                        </div>
                        if let Some(e) = &p.client.last_error {
                            <div class="settings-row-desc">{ format!("{} {}", i18n.t("update_services.last_error"), e) }</div>
                        }
                    </div>
                </div>
            }
        </>
    }
}
