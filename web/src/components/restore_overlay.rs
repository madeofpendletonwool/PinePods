use crate::components::context::AppState;
use crate::requests::setting_reqs::call_restore_status;
use i18nrs::yew::use_translation;
use yew::prelude::*;
use yewdux::prelude::*;

/// Global overlay that shows a full-page "restore in progress" screen while a server
/// restore is running, and reloads the page automatically when it finishes.
///
/// During a restore the database is locked, so every normal request blocks and the app
/// would otherwise just appear to hang. This component polls the dedicated DB-free
/// `/restore_status` probe (see `call_restore_status`), which stays responsive throughout.
#[function_component(RestoreOverlay)]
pub fn restore_overlay() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();

    let server_name = state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    // Latch the first non-empty server URL and keep polling with it, independent of later
    // auth changes. A restore truncates the Sessions table, which wipes the admin's own
    // session mid-restore and clears `auth_details` (hence `server_name`). Re-keying the
    // poller on `server_name` would tear it down at exactly the moment we need it to keep
    // polling, observe completion, and reload. `call_restore_status` requires no auth, so the
    // latched URL keeps working even with no valid session. (Logged-out clients never latch,
    // so they don't poll at all.)
    let base_url = use_state(|| Option::<String>::None);
    {
        let base_url = base_url.clone();
        use_effect_with(server_name.clone(), move |server_name| {
            if base_url.is_none() {
                if let Some(sn) = server_name {
                    if !sn.is_empty() {
                        base_url.set(Some(sn.clone()));
                    }
                }
            }
            || ()
        });
    }

    let active = use_state(|| false);
    // Tracks the previous poll result across interval ticks (independent of renders) so we
    // can detect the in-progress -> finished transition and reload exactly once.
    let last_known = use_mut_ref(|| false);

    {
        let active = active.clone();
        let last_known = last_known.clone();
        let is_active = *active;
        // Adaptive cadence so an idle client doesn't hammer /restore_status: poll every 4s
        // while a restore is active (snappy completion detection + reload), and back off to
        // 30s when idle. A restore locks the database for minutes, so a 30s idle poll still
        // notices one started elsewhere well within that window. Keyed on the latched URL plus
        // the active flag, so the interval is (re)built only when the URL first appears or the
        // active state flips -- the `last_known` ref persists across those rebuilds.
        use_effect_with(((*base_url).clone(), is_active), move |(base_url, is_active)| {
            let period = if *is_active { 4_000 } else { 30_000 };
            let interval = base_url.clone().map(|server_name| {
                let active = active.clone();
                let last_known = last_known.clone();
                gloo_timers::callback::Interval::new(period, move || {
                    let active = active.clone();
                    let last_known = last_known.clone();
                    let server_name = server_name.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(in_progress) = call_restore_status(&server_name).await {
                            let was_in_progress = *last_known.borrow();
                            *last_known.borrow_mut() = in_progress;
                            if *active != in_progress {
                                active.set(in_progress);
                            }
                            // Restore just finished: reload to fetch fresh data.
                            if was_in_progress && !in_progress {
                                if let Some(window) = web_sys::window() {
                                    let _ = window.location().reload();
                                }
                            }
                        }
                    });
                })
            });

            move || drop(interval)
        });
    }

    if !*active {
        return Html::default();
    }

    let title = i18n.t("restore_overlay.title");
    let message = i18n.t("restore_overlay.message");
    let dont_close = i18n.t("restore_overlay.dont_close");

    html! {
        <div class="fixed inset-0 bg-black bg-opacity-70 z-[9999] flex items-center justify-center p-4">
            <div class="modal-container relative w-full max-w-md rounded-lg shadow p-8 flex flex-col items-center text-center">
                <div class="loading-spinner mb-6"></div>
                <h2 style="font-size:1.5rem;font-weight:700;color:var(--text-color);margin-bottom:0.75rem;">
                    { title }
                </h2>
                <p style="font-size:1rem;color:var(--text-color);margin-bottom:1.5rem;line-height:1.6;">
                    { message }
                </p>
                <div class="w-full rounded-md px-4 py-3" style="border:1px solid var(--border-color);background-color:rgba(0,0,0,0.12);">
                    <p style="font-size:0.85rem;color:var(--text-secondary-color);line-height:1.5;opacity:0.85;">
                        { dont_close }
                    </p>
                </div>
            </div>
        </div>
    }
}
