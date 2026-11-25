//! Desktop app work that runs while the app is open: automatic downloads, removing played
//! downloads, new-episode notifications, podcast links and dropped OPML files, and the
//! update prompt.

use crate::components::context::{AppState, EpisodeStatusState, NotificationState};
use crate::components::i18n::use_translation;
use crate::desktop::{self, DesktopState, Settings};
use crate::requests::episode::Episode;
use crate::requests::pod_req::call_get_recent_eps_paged;
use crate::requests::search_pods::{call_get_podcast_details_dynamic, call_get_podcast_episodes};
use gloo_timers::callback::{Interval, Timeout};
use std::collections::{BTreeMap, HashSet};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

/// How often the background pass runs.
const PASS_MINUTES: u32 = 15;
/// Newest episode id already announced, so only later ones are.
const SEEN_KEY: &str = "pinepods_desktop_seen_episode";
/// How many of a podcast's latest episodes are looked at.
const LOOK_BACK: i64 = 200;

#[derive(Clone, PartialEq)]
struct Account {
    server: String,
    api_key: String,
    user_id: i32,
}

fn account(state: &AppState) -> Option<Account> {
    let auth = state.auth_details.as_ref()?;
    Some(Account {
        server: auth.server_name.clone(),
        api_key: auth.api_key.clone()?,
        user_id: state.user_details.as_ref()?.UserID,
    })
}

#[derive(Clone)]
struct Texts {
    new_episodes: String,
    and_more: String,
}

/// What to download and delete for one podcast with automatic downloads: the newest `keep`
/// unplayed episodes stay; other automatic downloads of it go. `episodes` is newest first.
pub fn plan_podcast(
    episodes: &[Episode],
    keep: u32,
    local: &HashSet<i32>,
    automatic: &HashSet<i32>,
) -> (Vec<Episode>, Vec<i32>) {
    let wanted: Vec<&Episode> = episodes.iter().filter(|e| !e.completed).take(keep as usize).collect();
    let wanted_ids: HashSet<i32> = wanted.iter().map(|e| e.episodeid).collect();
    let download = wanted.into_iter().filter(|e| !local.contains(&e.episodeid)).cloned().collect();
    let remove = episodes
        .iter()
        .map(|e| e.episodeid)
        .filter(|id| automatic.contains(id) && local.contains(id) && !wanted_ids.contains(id))
        .collect();
    (download, remove)
}

/// Downloads on this computer that have been played.
pub fn played(episodes: &[Episode], local: &HashSet<i32>) -> Vec<i32> {
    episodes.iter().filter(|e| e.completed && local.contains(&e.episodeid)).map(|e| e.episodeid).collect()
}

async fn podcast_episodes(account: &Account, podcast_id: i32) -> Vec<Episode> {
    call_get_podcast_episodes(
        &account.server,
        &Some(account.api_key.clone()),
        &account.user_id,
        &podcast_id,
        Some(LOOK_BACK),
        Some(0),
        Some("date"),
        Some("desc"),
        None,
        None,
    )
    .await
    .map(|r| r.episodes)
    .unwrap_or_else(|e| {
        desktop::log_error(format!("automatic downloads: podcast {podcast_id}: {e}"));
        Vec::new()
    })
}

/// Download, keep and delete according to the settings.
async fn sync_downloads(account: &Account, settings: &Settings) {
    let status = Dispatch::<EpisodeStatusState>::global().get();
    let local: HashSet<i32> = status.downloaded_episodes.local_ids().collect();
    let local_podcasts: HashSet<i32> = status
        .downloaded_episodes
        .episodes()
        .filter(|e| local.contains(&e.episodeid))
        .map(|e| e.podcastid)
        .collect();
    let automatic = Dispatch::<DesktopState>::global().get().automatic.clone();

    let mut podcasts: BTreeMap<i32, u32> = settings.auto_download.clone();
    if settings.delete_finished_downloads {
        for id in local_podcasts {
            podcasts.entry(id).or_insert(0);
        }
    }
    let mut remove = Vec::new();
    for (podcast_id, keep) in podcasts {
        let episodes = podcast_episodes(account, podcast_id).await;
        if keep > 0 {
            let (download, drop) = plan_podcast(&episodes, keep, &local, &automatic);
            remove.extend(drop);
            for episode in download {
                let id = episode.episodeid;
                if let Err(e) = desktop::download_episode(
                    episode,
                    account.server.clone(),
                    account.api_key.clone(),
                    account.user_id,
                    true,
                )
                .await
                {
                    desktop::log_error(format!("automatic download of episode {id} failed: {e}"));
                }
            }
        }
        if settings.delete_finished_downloads {
            remove.extend(played(&episodes, &local));
        }
    }
    remove.sort();
    remove.dedup();
    if !remove.is_empty() {
        if let Err(e) = desktop::delete(remove).await {
            desktop::log_error(format!("could not remove played downloads: {e}"));
        }
    }
}

/// Notify about episodes newer than the last one seen.
async fn announce_new_episodes(account: &Account, texts: &Texts) {
    let Ok(page) = call_get_recent_eps_paged(&account.server, &Some(account.api_key.clone()), &account.user_id, 20, 0).await
    else {
        return;
    };
    let Some(newest) = page.episodes.iter().map(|e| e.episodeid).max() else { return };
    let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
    let seen = storage
        .as_ref()
        .and_then(|s| s.get_item(SEEN_KEY).ok().flatten())
        .and_then(|v| v.parse::<i32>().ok());
    if let Some(storage) = &storage {
        let _ = storage.set_item(SEEN_KEY, &newest.to_string());
    }
    // The first run only records where things stand.
    let Some(seen) = seen else { return };
    let fresh: Vec<&Episode> = page.episodes.iter().filter(|e| e.episodeid > seen).collect();
    if fresh.is_empty() {
        return;
    }
    let mut lines: Vec<String> = fresh.iter().take(3).map(|e| format!("{}: {}", e.podcastname, e.episodetitle)).collect();
    if fresh.len() > 3 {
        lines.push(texts.and_more.replace("{count}", &(fresh.len() - 3).to_string()));
    }
    desktop::notify(texts.new_episodes.clone(), lines.join("\n"));
}

async fn background_pass(account: Account, texts: Texts) {
    let Some(settings) = Dispatch::<DesktopState>::global().get().settings.clone() else { return };
    if !settings.auto_download.is_empty() || settings.delete_finished_downloads {
        sync_downloads(&account, &settings).await;
    }
    if settings.notify_new_episodes {
        announce_new_episodes(&account, &texts).await;
    }
}

/// Open the page of a podcast from a subscribe link, where it can be subscribed to.
async fn open_feed(account: Account, feed: String) -> Result<(), String> {
    let details = call_get_podcast_details_dynamic(
        &account.server,
        &account.api_key,
        account.user_id,
        "",
        &urlencoding::encode(&feed),
        0,
        false,
        Some(true),
    )
    .await
    .map_err(|e| e.to_string())?
    .details;
    let categories = details
        .categories
        .as_ref()
        .map(|c| c.values().cloned().collect::<Vec<_>>().join(", "));
    let click = web_sys::MouseEvent::new("click").map_err(|_| "no click event".to_string())?;
    crate::components::click_events::create_on_title_click(
        account.server,
        Some(Some(account.api_key)),
        &BrowserHistory::new(),
        details.podcastid,
        details.podcastindexid,
        details.podcastname,
        feed,
        details.description,
        details.author,
        details.artworkurl,
        details.explicit,
        details.episodecount,
        categories,
        details.websiteurl,
        account.user_id,
        details.is_youtube.unwrap_or(false),
    )
    .emit(click);
    Ok(())
}

/// In the main window: runs the background pass and handles links and dropped files.
#[function_component(Background)]
pub fn background() -> Html {
    let (i18n, _) = use_translation();
    let account = use_selector(|s: &AppState| account(s));
    let loaded = use_selector(|s: &DesktopState| s.settings.is_some());
    let feed_link = use_selector(|s: &DesktopState| s.feed_link.clone());
    let opml = use_selector(|s: &DesktopState| s.opml.is_some());
    let texts = Texts {
        new_episodes: i18n.t("desktop.new_episodes").to_string(),
        and_more: i18n.t("desktop.and_more").to_string(),
    };
    let open_failed = i18n.t("desktop.open_link_failed").to_string();

    // Shortly after signing in (or once the settings load), then every few minutes.
    use_effect_with(((*account).clone(), *loaded), move |(account, loaded)| {
        let timers = account.clone().filter(|_| *loaded && desktop::is_desktop()).map(|account| {
            let (first_account, first_texts) = (account.clone(), texts.clone());
            let first = Timeout::new(20_000, move || {
                wasm_bindgen_futures::spawn_local(background_pass(first_account, first_texts));
            });
            let every = Interval::new(PASS_MINUTES * 60_000, move || {
                wasm_bindgen_futures::spawn_local(background_pass(account.clone(), texts.clone()));
            });
            (first, every)
        });
        move || drop(timers)
    });

    // A podcast link, once signed in.
    use_effect_with(((*feed_link).clone(), (*account).clone()), move |(feed, account)| {
        if let (Some(feed), Some(account)) = (feed.clone(), account.clone()) {
            Dispatch::<DesktopState>::global().reduce_mut(|s| s.feed_link = None);
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = open_feed(account, feed.clone()).await {
                    desktop::log_error(format!("could not open the podcast link {feed}: {e}"));
                    Dispatch::<NotificationState>::global().reduce_mut(move |s| s.error_message = Some(open_failed));
                }
            });
        }
        || ()
    });

    // A dropped OPML file: the import section picks it up.
    use_effect_with((*opml, account.is_some()), |(opml, signed_in)| {
        if *opml && *signed_in {
            BrowserHistory::new().push("/settings?section=import");
        }
        || ()
    });

    html! { <UpdatePrompt /> }
}

/// Offers a newer version when the update check found one.
#[function_component(UpdatePrompt)]
fn update_prompt() -> Html {
    let (i18n, _) = use_translation();
    let update = use_selector(|s: &DesktopState| s.update.clone());
    let dismissed = use_state(|| false);
    let installing = use_state(|| false);
    let Some(update) = (*update).clone() else { return html! {} };
    if *dismissed {
        return html! {};
    }
    let failed = i18n.t("desktop.update_failed").to_string();
    let on_install = {
        let installing = installing.clone();
        Callback::from(move |_: MouseEvent| {
            installing.set(true);
            let (installing, failed) = (installing.clone(), failed.clone());
            wasm_bindgen_futures::spawn_local(async move {
                // Restarts into the new version on success.
                if let Err(e) = desktop::install_update().await {
                    installing.set(false);
                    desktop::log_error(format!("update failed: {e}"));
                    Dispatch::<NotificationState>::global()
                        .reduce_mut(move |s| s.error_message = Some(format!("{failed}: {e}")));
                }
            });
        })
    };
    let on_later = {
        let dismissed = dismissed.clone();
        Callback::from(move |_: MouseEvent| dismissed.set(true))
    };
    html! {
        <div class="desktop-update" role="status">
            <i class="ph ph-arrow-circle-up" aria-hidden="true"></i>
            <span>{ i18n.t("desktop.update_available").replace("{version}", &update.version) }</span>
            <button type="button" class="desktop-update-install" onclick={on_install} disabled={*installing}>
                { if *installing { i18n.t("desktop.update_installing") } else { i18n.t("desktop.update_install") } }
            </button>
            if !*installing {
                <button type="button" class="desktop-update-later" onclick={on_later}>{ i18n.t("desktop.update_later") }</button>
            }
        </div>
    }
}
