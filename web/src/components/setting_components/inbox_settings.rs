//! Settings: how far back the inbox reaches, and the user's episode labels.

use crate::components::context::NotificationState;
use crate::components::i18n::use_translation;
use crate::components::labels::{creds, refresh_inbox_count, use_signed_in, LabelManager};
use crate::requests::inbox_req;
use crate::requests::notify::{toast_error, toast_info};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

const DAY_CHOICES: &[i32] = &[7, 14, 30, 60, 90, 180, 365];

#[function_component(InboxSettings)]
pub fn inbox_settings() -> Html {
    let (i18n, _) = use_translation();
    let days = use_state(|| None::<i32>);

    let signed_in = use_signed_in();
    {
        let days = days.clone();
        use_effect_with(signed_in, move |_| {
            if let Some((server, api_key, _)) = creds() {
                spawn_local(async move {
                    if let Ok(d) = inbox_req::call_get_inbox_days(&server, &api_key).await {
                        days.set(Some(d));
                    }
                });
            }
            || ()
        });
    }

    let saved_msg = i18n.t("inbox_settings.saved").to_string();
    let on_change = {
        let days = days.clone();
        Callback::from(move |e: Event| {
            let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let Ok(value) = select.value().parse::<i32>() else { return };
            let Some((server, api_key, _)) = creds() else { return };
            let days = days.clone();
            let saved_msg = saved_msg.clone();
            spawn_local(async move {
                match inbox_req::call_set_inbox_days(&server, &api_key, value).await {
                    Ok(d) => {
                        days.set(Some(d));
                        refresh_inbox_count();
                        toast_info(&Dispatch::<NotificationState>::global(), saved_msg);
                    }
                    Err(e) => toast_error(&Dispatch::<NotificationState>::global(), e.to_string()),
                }
            });
        })
    };

    let current = (*days).unwrap_or(30);
    let mut choices = DAY_CHOICES.to_vec();
    if !choices.contains(&current) {
        choices.push(current);
        choices.sort();
    }
    let day_label = i18n.t("inbox_settings.days_option").to_string();

    html! {
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{ i18n.t("inbox_settings.window") }</div>
                <div class="settings-row-desc">{ i18n.t("inbox_settings.window_description") }</div>
            </div>
            <div class="settings-row-control">
                <select class="select" style="width:200px;" onchange={on_change} disabled={days.is_none()}>
                    { for choices.iter().map(|d| html! {
                        <option value={d.to_string()} selected={*d == current}>{ day_label.replace("{days}", &d.to_string()) }</option>
                    }) }
                </select>
            </div>
        </div>
    }
}

#[function_component(LabelSettings)]
pub fn label_settings() -> Html {
    html! { <LabelManager /> }
}
