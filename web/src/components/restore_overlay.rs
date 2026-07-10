use crate::requests::setting_reqs::call_restore_status;
use i18nrs::yew::use_translation;
use yew::prelude::*;

/// Global overlay that shows a full-page "restore in progress" screen while a server
/// restore started from *this* browser is running, and reloads the page automatically
/// when it finishes.
///
/// The restore locks/truncates the database, so normal requests block and the app would
/// otherwise appear to hang. This component polls the dedicated DB-free `/restore_status`
/// probe (see `call_restore_status`), which stays responsive throughout.
///
/// Polling is event-driven: it only runs when the restore trigger
/// (`setting_components::restore_server`) has set the `pinepods_restore_active` flag in
/// localStorage. When idle (the normal case) this component does nothing at all -- no
/// interval, no requests -- so `/restore_status` is never hit outside an actual restore.
/// The flag survives the sign-out that a restore forces (see `LogOut`, which preserves it
/// across its storage clear + full page reload), so this component remounts fresh with the
/// flag set and begins polling.
#[function_component(RestoreOverlay)]
pub fn restore_overlay() -> Html {
    let (i18n, _) = use_translation();

    // Read the persisted flag + server URL once at mount. `call_restore_status` needs no
    // auth, so the latched URL keeps working even though the session was wiped by the
    // restore. `None` => not tracking a restore => render nothing and start no interval.
    let server_url = use_state(|| -> Option<String> {
        let storage = web_sys::window()?.local_storage().ok().flatten()?;
        if storage
            .get_item("pinepods_restore_active")
            .ok()
            .flatten()
            .as_deref()
            == Some("1")
        {
            storage.get_item("pinepods_restore_server").ok().flatten()
        } else {
            None
        }
    });

    let is_tracking = server_url.is_some();

    // Persist across interval ticks (independent of renders): whether we've observed the
    // restore actually in progress, and a tick counter for the safety fallback.
    let seen_active = use_mut_ref(|| false);
    let ticks = use_mut_ref(|| 0u32);

    {
        let seen_active = seen_active.clone();
        let ticks = ticks.clone();
        use_effect_with((*server_url).clone(), move |server_url| {
            // Only builds an interval when a restore is being tracked. Poll every 4s for
            // snappy completion detection; the restore locks the DB for minutes so this
            // is short-lived.
            let interval = server_url.clone().map(|server_name| {
                let seen_active = seen_active.clone();
                let ticks = ticks.clone();
                gloo_timers::callback::Interval::new(4_000, move || {
                    let seen_active = seen_active.clone();
                    let ticks = ticks.clone();
                    let server_name = server_name.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        *ticks.borrow_mut() += 1;
                        let elapsed = *ticks.borrow();
                        if let Ok(in_progress) = call_restore_status(&server_name).await {
                            if in_progress {
                                *seen_active.borrow_mut() = true;
                                return;
                            }
                            // in_progress == false: finish once we've seen it running and it's
                            // now done, or as a safety net if it never appeared active within
                            // ~2 min (finished before our first poll, or never really started).
                            if *seen_active.borrow() || elapsed >= 30 {
                                finish_restore();
                            }
                        }
                    });
                })
            });

            move || drop(interval)
        });
    }

    if !is_tracking {
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

/// Clears the restore flags and reloads once. After the reload this component remounts
/// with no flag set, so it renders nothing and never polls again.
fn finish_restore() {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.remove_item("pinepods_restore_active");
            let _ = storage.remove_item("pinepods_restore_server");
        }
        let _ = window.location().reload();
    }
}
