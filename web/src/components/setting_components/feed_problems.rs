// Feed problems: the user's podcasts whose refresh keeps failing, and podcasts a sync app
// subscribed to that couldn't be added, with what can be done about each.

use crate::components::context::{AppState, NotificationState};
use crate::requests::pod_req::{call_remove_podcasts, RemovePodcastValues};
use crate::requests::setting_reqs::{
    call_dismiss_import, call_get_feed_problems, call_refresh_failing_feed, call_remove_import,
    call_retry_import, FailingFeed, FeedProblems, ImportFailure,
};
use i18nrs::yew::use_translation;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

fn notify_error(message: String) {
    Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(message));
}

fn notify_info(message: String) {
    Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(message));
}

fn fmt_time(secs: i64) -> String {
    js_sys::Date::new(&JsValue::from_f64(secs as f64 * 1000.0))
        .to_locale_string("default", &JsValue::UNDEFINED)
        .as_string()
        .unwrap_or_default()
}

fn confirm(message: &str) -> bool {
    web_sys::window().and_then(|w| w.confirm_with_message(message).ok()).unwrap_or(false)
}

/// The translation key describing an error kind from the API.
pub fn kind_key(kind: &str) -> &'static str {
    match kind {
        "gone" => "feed_problems.kind_gone",
        "auth" => "feed_problems.kind_auth",
        "not_feed" => "feed_problems.kind_not_feed",
        "unreachable" => "feed_problems.kind_unreachable",
        "server" => "feed_problems.kind_server",
        "throttled" => "feed_problems.kind_throttled",
        _ => "feed_problems.kind_other",
    }
}

#[derive(Clone, Copy)]
enum Action {
    Refresh(i32),
    Unsubscribe(i32, i32),
    Retry(i32),
    Remove(i32),
    Dismiss(i32),
}

impl Action {
    /// The row it's for, to show it as busy.
    fn row(self) -> String {
        match self {
            Action::Refresh(id) | Action::Unsubscribe(id, _) => format!("feed-{id}"),
            Action::Retry(id) | Action::Remove(id) | Action::Dismiss(id) => format!("import-{id}"),
        }
    }
}

#[function_component(FeedProblemsSection)]
pub fn feed_problems_section() -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let auth = state
        .auth_details
        .as_ref()
        .and_then(|a| a.api_key.clone().map(|k| (a.server_name.clone(), k)));
    let user_id = state.user_details.as_ref().map(|u| u.UserID);
    let problems = use_state(|| None::<FeedProblems>);
    let busy = use_state(|| None::<String>);
    let reload = use_state(|| 0u32);

    {
        let (problems, auth) = (problems.clone(), auth.clone());
        use_effect_with((auth.clone(), *reload), move |_| {
            if let Some((server, key)) = auth {
                spawn_local(async move {
                    match call_get_feed_problems(&server, &key).await {
                        Ok(p) => problems.set(Some(p)),
                        Err(e) => notify_error(e.to_string()),
                    }
                });
            }
            || ()
        });
    }

    // Runs an action for one row, then reloads the list.
    let run = {
        let (auth, busy, reload) = (auth.clone(), busy.clone(), reload.clone());
        Callback::from(move |(action, done): (Action, Option<String>)| {
            let Some((server, key)) = auth.clone() else { return };
            let (busy, reload) = (busy.clone(), reload.clone());
            busy.set(Some(action.row()));
            spawn_local(async move {
                let result = match action {
                    Action::Refresh(id) => call_refresh_failing_feed(&server, &key, id).await.map(|_| ()),
                    Action::Unsubscribe(podcast_id, user_id) => {
                        let values = RemovePodcastValues { podcast_id, user_id, is_youtube: false };
                        call_remove_podcasts(&server, &Some(key), &values).await.map(|_| ())
                    }
                    Action::Retry(id) => call_retry_import(&server, &key, id).await.map(|_| ()),
                    Action::Remove(id) => call_remove_import(&server, &key, id).await.map(|_| ()),
                    Action::Dismiss(id) => call_dismiss_import(&server, &key, id).await.map(|_| ()),
                };
                match result {
                    Ok(()) => {
                        if let Some(done) = done {
                            notify_info(done);
                        }
                    }
                    Err(e) => notify_error(e.to_string()),
                }
                busy.set(None);
                reload.set(*reload + 1);
            });
        })
    };

    let Some(p) = (*problems).clone() else {
        return html! { <div class="settings-row"><div class="settings-row-desc"><i class="ph ph-spinner animate-spin"></i></div></div> };
    };
    if p.feeds.is_empty() && p.imports.is_empty() {
        return html! { <div class="settings-row"><div class="settings-row-desc">{ i18n.t("feed_problems.none") }</div></div> };
    }

    let feed_row = |f: &FailingFeed| -> Html {
        let working = busy.as_deref() == Some(Action::Refresh(f.podcast_id).row().as_str());
        let (id, name) = (f.podcast_id, f.name.clone());
        let refresh = run.reform(move |_: MouseEvent| (Action::Refresh(id), None));
        let unsubscribe = {
            let run = run.clone();
            let question = i18n.t("feed_problems.unsubscribe_confirm").replace("{name}", &name);
            Callback::from(move |_: MouseEvent| {
                if let Some(user_id) = user_id.filter(|_| confirm(&question)) {
                    run.emit((Action::Unsubscribe(id, user_id), None));
                }
            })
        };
        let since = f.failing_since.map(fmt_time).unwrap_or_default();
        html! {
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{ &f.name }</div>
                    <div class="settings-row-desc">{ i18n.t(kind_key(&f.kind)) }</div>
                    <div class="settings-row-desc">
                        { i18n.t("feed_problems.failing_since").replace("{time}", &since).replace("{count}", &f.failures.to_string()) }
                    </div>
                    if let Some(error) = &f.error {
                        <div class="settings-row-desc"><code class="sync-code">{ error }</code></div>
                    }
                </div>
                <div class="settings-row-control">
                    <button class="btn btn-ghost" disabled={working} onclick={refresh}>
                        <i class={classes!("ph", if working { "ph-spinner animate-spin" } else { "ph-arrows-clockwise" })}></i>{ i18n.t("feed_problems.refresh_now") }
                    </button>
                    <button class="btn btn-ghost" disabled={working} onclick={unsubscribe}>
                        <i class="ph ph-trash"></i>{ i18n.t("feed_problems.unsubscribe") }
                    </button>
                </div>
            </div>
        }
    };

    let import_row = |f: &ImportFailure| -> Html {
        let working = busy.as_deref() == Some(Action::Retry(f.id).row().as_str());
        let id = f.id;
        let done = i18n.t("feed_problems.retry_started").to_string();
        let retry = run.reform(move |_: MouseEvent| (Action::Retry(id), Some(done.clone())));
        let remove = {
            let run = run.clone();
            let question = i18n.t("feed_problems.remove_import_confirm").to_string();
            Callback::from(move |_: MouseEvent| {
                if confirm(&question) {
                    run.emit((Action::Remove(id), None));
                }
            })
        };
        let dismiss = run.reform(move |_: MouseEvent| (Action::Dismiss(id), None));
        let source = if f.source == "target" { i18n.t("feed_problems.from_external") } else { i18n.t("feed_problems.from_app") };
        let next = match f.next_attempt_at {
            Some(t) => i18n.t("feed_problems.next_retry").replace("{time}", &fmt_time(t)),
            None => i18n.t("feed_problems.gave_up").to_string(),
        };
        html! {
            <div class="settings-row">
                <div style="min-width:0">
                    <div class="settings-row-label" style="overflow-wrap:anywhere">{ &f.feed_url }</div>
                    <div class="settings-row-desc">{ format!("{} · {}", source, i18n.t(kind_key(&f.kind))) }</div>
                    <div class="settings-row-desc">{ next }</div>
                </div>
                <div class="settings-row-control">
                    <button class="btn btn-ghost" disabled={working} onclick={retry}>
                        <i class={classes!("ph", if working { "ph-spinner animate-spin" } else { "ph-arrows-clockwise" })}></i>{ i18n.t("feed_problems.retry") }
                    </button>
                    <button class="btn btn-ghost" disabled={working} onclick={remove}><i class="ph ph-trash"></i>{ i18n.t("feed_problems.unsubscribe") }</button>
                    <button class="btn btn-ghost" disabled={working} onclick={dismiss}><i class="ph ph-x"></i>{ i18n.t("feed_problems.dismiss") }</button>
                </div>
            </div>
        }
    };

    html! {
        <>
            if !p.feeds.is_empty() {
                <div class="settings-subsection-title">{ i18n.t("feed_problems.feeds_title") }</div>
                { for p.feeds.iter().map(feed_row) }
            }
            if !p.imports.is_empty() {
                <div class="settings-subsection-title">{ i18n.t("feed_problems.imports_title") }</div>
                <div class="settings-row"><div class="settings-row-desc">{ i18n.t("feed_problems.imports_desc") }</div></div>
                { for p.imports.iter().map(import_row) }
            }
        </>
    }
}

#[derive(Properties, PartialEq)]
pub struct FeedProblemBannerProps {
    pub podcast_id: i32,
}

/// Shown on a podcast's page while its feed keeps failing, linking to the feed problems list.
#[function_component(FeedProblemBanner)]
pub fn feed_problem_banner(props: &FeedProblemBannerProps) -> Html {
    let (i18n, _) = use_translation();
    let (state, _) = use_store::<AppState>();
    let auth = state
        .auth_details
        .as_ref()
        .and_then(|a| a.api_key.clone().map(|k| (a.server_name.clone(), k)));
    let failing = use_state(|| None::<FailingFeed>);
    {
        let failing = failing.clone();
        let podcast_id = props.podcast_id;
        use_effect_with((auth, podcast_id), move |(auth, podcast_id)| {
            failing.set(None);
            if let (Some((server, key)), id) = (auth.clone(), *podcast_id) {
                if id > 0 {
                    spawn_local(async move {
                        if let Ok(p) = call_get_feed_problems(&server, &key).await {
                            failing.set(p.feeds.into_iter().find(|f| f.podcast_id == id && f.reported));
                        }
                    });
                }
            }
            || ()
        });
    }
    let Some(f) = (*failing).clone() else { return html! {} };
    let open = Callback::from(|_: MouseEvent| {
        yew_router::history::History::push(&yew_router::history::BrowserHistory::new(), "/settings?section=feed_problems");
    });
    html! {
        <div class="sync-notice" role="status">
            <i class="ph ph-warning-circle"></i>
            <div class="sync-notice-text">
                <div>{ i18n.t("feed_problems.banner") }</div>
                <div style="opacity:.8">{ i18n.t(kind_key(&f.kind)) }</div>
            </div>
            <button class="btn btn-secondary" onclick={open}>{ i18n.t("feed_problems.open") }</button>
        </div>
    }
}
