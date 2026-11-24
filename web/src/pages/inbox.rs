//! The inbox: new episodes not dealt with yet. Queueing, saving, playing, finishing or dismissing
//! one takes it out, here or anywhere else.

use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, EpisodeStatusState, NotificationState, PodcastFeedState};
use crate::components::context_menu_button::PageType;
use crate::components::episode_list_view::EpisodeListView;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::i18n::use_translation;
use crate::components::labels::{self, color_class, creds, LabelState};
use crate::components::loading::Loading;
use crate::requests::episode::Episode;
use crate::requests::inbox_req::{self, EpisodeRef, InboxQuery};
use crate::requests::notify::{toast_error, toast_info};
use crate::requests::pod_req;
use gloo_timers::future::TimeoutFuture;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::history::History;
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

/// Episode ids that the shared state marks queued, saved or finished.
fn handled_ids(s: &EpisodeStatusState) -> HashSet<i32> {
    let mut ids: HashSet<i32> = s.completed_episodes.iter().copied().collect();
    ids.extend(s.saved_episode_ids());
    if let Some(q) = s.queued_episode_ids.as_ref() {
        ids.extend(q.iter().copied());
    }
    ids
}

#[function_component(Inbox)]
pub fn inbox() -> Html {
    let (i18n, _) = use_translation();
    let label_state = use_store_value::<LabelState>();
    // Signed-in state can arrive after the first render on a fresh page load.
    let signed_in = use_selector(|s: &AppState| {
        s.auth_details.as_ref().and_then(|a| a.api_key.clone().map(|k| (a.server_name.clone(), k)))
            .zip(s.user_details.as_ref().map(|u| u.UserID))
    });
    let podcasts = use_selector(|s: &PodcastFeedState| s.podcast_feed_return_extra.clone());
    let handled = use_selector(|s: &EpisodeStatusState| handled_ids(s));

    let query = use_state(InboxQuery::default);
    let episodes = use_state(|| Rc::new(Vec::<Episode>::new()));
    let total = use_state(|| 0i64);
    let loading = use_state(|| true);
    let loading_more = use_state(|| false);
    // Taken out on this page since the last load.
    let removed = use_state(HashSet::<i32>::new);
    // What the shared state already marked handled when the page loaded; only later marks hide.
    let baseline = use_state(HashSet::<i32>::new);
    // The last dismissal, for Undo.
    let last_dismissed = use_state(|| None::<Vec<EpisodeRef>>);
    let reload_nonce = use_state(|| 0u32);

    {
        let episodes = episodes.clone();
        let total = total.clone();
        let loading = loading.clone();
        let removed = removed.clone();
        let baseline = baseline.clone();
        use_effect_with(((*query).clone(), *reload_nonce, (*signed_in).clone()), move |(query, _, _)| {
            labels::ensure_loaded();
            if let Some((server, api_key, user_id)) = creds() {
                if podcasts_missing() {
                    let server = server.clone();
                    let api_key = Some(api_key.clone());
                    spawn_local(async move {
                        if let Ok(pods) = pod_req::call_get_podcasts_extra(&server, &api_key, &user_id).await {
                            Dispatch::<PodcastFeedState>::global().reduce_mut(move |s| {
                                s.podcast_feed_return_extra = Some(pod_req::PodcastResponseExtra { pods: Some(pods) });
                            });
                        }
                    });
                }
                loading.set(true);
                let query = query.clone();
                spawn_local(async move {
                    match inbox_req::call_get_inbox(&server, &api_key, &query, PAGE_SIZE, 0).await {
                        Ok(page) => {
                            baseline.set(handled_ids(&Dispatch::<EpisodeStatusState>::global().get()));
                            removed.set(HashSet::new());
                            total.set(page.total);
                            if query == InboxQuery::default() {
                                let count = page.total;
                                Dispatch::<LabelState>::global().reduce_mut(move |s| s.inbox_count = Some(count));
                            }
                            episodes.set(Rc::new(page.episodes));
                        }
                        Err(e) => toast_error(&Dispatch::<NotificationState>::global(), e.to_string()),
                    }
                    loading.set(false);
                });
            }
            || ()
        });
    }

    let on_load_more = {
        let episodes = episodes.clone();
        let total = total.clone();
        let loading_more = loading_more.clone();
        let query = query.clone();
        let removed = removed.clone();
        use_callback(((*query).clone(), removed.len()), move |_: (), (q, _)| {
            if *loading_more {
                return;
            }
            // Removed items no longer count towards the server's offset.
            let offset = episodes.len() as i64 - removed.len() as i64;
            if offset >= *total {
                return;
            }
            let Some((server, api_key, _)) = creds() else { return };
            loading_more.set(true);
            let episodes = episodes.clone();
            let total = total.clone();
            let loading_more = loading_more.clone();
            let q = q.clone();
            spawn_local(async move {
                if let Ok(page) = inbox_req::call_get_inbox(&server, &api_key, &q, PAGE_SIZE, offset.max(0)).await {
                    TimeoutFuture::new(0).await;
                    let mut all = (**episodes).clone();
                    let seen: HashSet<(i32, bool)> = all.iter().map(|e| (e.episodeid, e.is_youtube)).collect();
                    all.extend(page.episodes.into_iter().filter(|e| !seen.contains(&(e.episodeid, e.is_youtube))));
                    total.set(page.total);
                    episodes.set(Rc::new(all));
                    TimeoutFuture::new(0).await;
                }
                loading_more.set(false);
            });
        })
    };

    // Dismissing from a card: remember it for Undo.
    let on_dismissed = {
        let removed = removed.clone();
        let last_dismissed = last_dismissed.clone();
        let episodes = episodes.clone();
        use_callback((), move |id: i32, _| {
            let mut next = (*removed).clone();
            next.insert(id);
            removed.set(next);
            let refs: Vec<EpisodeRef> = episodes.iter().filter(|e| e.episodeid == id).map(EpisodeRef::from).collect();
            last_dismissed.set(Some(refs));
        })
    };

    let undo = {
        let last_dismissed = last_dismissed.clone();
        let reload_nonce = reload_nonce.clone();
        Callback::from(move |_: MouseEvent| {
            let Some(refs) = (*last_dismissed).clone() else { return };
            let Some((server, api_key, _)) = creds() else { return };
            let last_dismissed = last_dismissed.clone();
            let reload_nonce = reload_nonce.clone();
            spawn_local(async move {
                match inbox_req::call_restore_inbox(&server, &api_key, &refs).await {
                    Ok(_) => {
                        last_dismissed.set(None);
                        labels::refresh_inbox_count();
                        reload_nonce.set(reload_nonce.wrapping_add(1));
                    }
                    Err(e) => toast_error(&Dispatch::<NotificationState>::global(), e.to_string()),
                }
            });
        })
    };

    let visible: Rc<Vec<Episode>> = {
        let hide = |e: &Episode| {
            removed.contains(&e.episodeid) || (handled.contains(&e.episodeid) && !baseline.contains(&e.episodeid))
        };
        if episodes.iter().any(hide) {
            Rc::new(episodes.iter().filter(|e| !hide(e)).cloned().collect())
        } else {
            (*episodes).clone()
        }
    };
    let hidden = episodes.len() - visible.len();
    let remaining = (*total - hidden as i64).max(0);

    let i18n_clear_confirm = i18n.t("inbox.clear_confirm").to_string();
    let i18n_cleared = i18n.t("inbox.cleared").to_string();
    let clear = {
        let query = query.clone();
        let reload_nonce = reload_nonce.clone();
        let last_dismissed = last_dismissed.clone();
        let visible = visible.clone();
        Callback::from(move |_: MouseEvent| {
            let msg = i18n_clear_confirm.replace("{count}", &remaining.to_string());
            if !web_sys::window().and_then(|w| w.confirm_with_message(&msg).ok()).unwrap_or(false) {
                return;
            }
            let Some((server, api_key, _)) = creds() else { return };
            let q = (*query).clone();
            let reload_nonce = reload_nonce.clone();
            let last_dismissed = last_dismissed.clone();
            let cleared_tpl = i18n_cleared.clone();
            // Undo can bring back what was loaded; the rest stays dismissed.
            let loaded: Vec<EpisodeRef> = visible.iter().map(EpisodeRef::from).collect();
            spawn_local(async move {
                match inbox_req::call_clear_inbox(&server, &api_key, &q).await {
                    Ok(n) => {
                        toast_info(&Dispatch::<NotificationState>::global(), cleared_tpl.replace("{count}", &n.to_string()));
                        last_dismissed.set(Some(loaded));
                        labels::refresh_inbox_count();
                        reload_nonce.set(reload_nonce.wrapping_add(1));
                    }
                    Err(e) => toast_error(&Dispatch::<NotificationState>::global(), e.to_string()),
                }
            });
        })
    };

    let set_filter = |label_id: Option<i32>, unlabeled: bool| {
        let query = query.clone();
        Callback::from(move |_: MouseEvent| {
            let mut q = (*query).clone();
            q.label_id = label_id;
            q.unlabeled = unlabeled;
            query.set(q);
        })
    };
    let toggle_sort = {
        let query = query.clone();
        Callback::from(move |_: MouseEvent| {
            let mut q = (*query).clone();
            q.oldest_first = !q.oldest_first;
            query.set(q);
        })
    };
    let on_podcast = {
        let query = query.clone();
        Callback::from(move |e: Event| {
            let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let mut q = (*query).clone();
            q.podcast_id = select.value().parse::<i32>().ok().filter(|id| *id > 0);
            query.set(q);
        })
    };
    let label_all_visible = {
        let visible = visible.clone();
        Callback::from(move |_: MouseEvent| {
            labels::open_picker(visible.iter().map(EpisodeRef::from).collect());
        })
    };

    let q = (*query).clone();
    let chip = |active: bool, label: Html, onclick: Callback<MouseEvent>| {
        html! {
            <button class={classes!("inbox-chip", active.then_some("is-active"))} {onclick}>{ label }</button>
        }
    };
    let mut pods: Vec<(i32, String)> = (*podcasts)
        .as_ref()
        .and_then(|p| p.pods.as_ref())
        .map(|p| p.iter().map(|p| (p.podcastid, p.podcastname.clone())).collect())
        .unwrap_or_default();
    pods.sort_by_key(|(_, n)| n.to_lowercase());

    let list_key = format!("{:?}-{:?}-{}-{}-{}", q.label_id, q.podcast_id, q.unlabeled, q.oldest_first, *reload_nonce);
    let backend_can_load_more = (episodes.len() - removed.len()) < (*total as usize);

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            <div class="inbox-header">
                <div class="inbox-title-row">
                    <h1 class="inbox-title">
                        { i18n.t("inbox.title") }
                        if !*loading {
                            <span class="inbox-count">{ remaining }</span>
                        }
                    </h1>
                    <div class="inbox-actions">
                        <button class="inbox-tool" onclick={toggle_sort} title={i18n.t("inbox.sort")}>
                            <i class={classes!("ph", if q.oldest_first { "ph-sort-ascending" } else { "ph-sort-descending" })}></i>
                            <span>{ if q.oldest_first { i18n.t("inbox.oldest_first") } else { i18n.t("inbox.newest_first") } }</span>
                        </button>
                        <button class="inbox-tool" onclick={label_all_visible} disabled={visible.is_empty()} title={i18n.t("inbox.label_all")}>
                            <i class="ph ph-tag"></i>
                            <span>{ i18n.t("inbox.label_all") }</span>
                        </button>
                        <button class="inbox-tool" onclick={clear} disabled={remaining == 0}>
                            <i class="ph ph-broom"></i>
                            <span>{ if q == InboxQuery::default() { i18n.t("inbox.clear_all") } else { i18n.t("inbox.clear_shown") } }</span>
                        </button>
                        <button class="inbox-tool" title={i18n.t("inbox.settings")} onclick={Callback::from(|_: MouseEvent| {
                            yew_router::history::BrowserHistory::new().push("/settings?page=inbox");
                        })}>
                            <i class="ph ph-gear"></i>
                        </button>
                    </div>
                </div>
                <div class="inbox-filters">
                    { chip(q.label_id.is_none() && !q.unlabeled, html!{ i18n.t("inbox.all") }, set_filter(None, false)) }
                    if !label_state.labels.is_empty() {
                        { chip(q.unlabeled && q.label_id.is_none(), html!{ i18n.t("inbox.unlabeled") }, set_filter(None, true)) }
                    }
                    { for label_state.labels.iter().map(|l| {
                        let active = q.label_id == Some(l.label_id);
                        html! {
                            <button class={classes!("inbox-chip", active.then_some("is-active"))} onclick={set_filter(Some(l.label_id), false)}>
                                <span class={classes!("ep-label-dot", color_class(&l.color))}></span>
                                { l.name.clone() }
                            </button>
                        }
                    }) }
                    if !pods.is_empty() {
                        <select class="inbox-podcast-select" onchange={on_podcast}>
                            <option value="0" selected={q.podcast_id.is_none()}>{ i18n.t("inbox.all_podcasts") }</option>
                            { for pods.iter().map(|(id, name)| html! {
                                <option value={id.to_string()} selected={q.podcast_id == Some(*id)}>{ name.clone() }</option>
                            }) }
                        </select>
                    }
                </div>
                if let Some(refs) = (*last_dismissed).clone() {
                    if !refs.is_empty() {
                        <div class="inbox-undo">
                            <span>{ i18n.t("inbox.dismissed_count").replace("{count}", &refs.len().to_string()) }</span>
                            <button class="inbox-undo-btn" onclick={undo}>{ i18n.t("inbox.undo") }</button>
                        </div>
                    }
                }
            </div>
            {
                if *loading {
                    html! { <Loading/> }
                } else if visible.is_empty() {
                    empty_message(&i18n.t("inbox.empty_title"), &i18n.t("inbox.empty_description"))
                } else {
                    html! {
                        <div class="flex-grow overflow-y-auto" key={list_key.clone()}>
                            <EpisodeListView
                                episodes={visible.clone()}
                                page_type={PageType::Inbox}
                                backend_can_load_more={backend_can_load_more}
                                loading_more={*loading_more}
                                on_load_more={on_load_more.clone()}
                                on_episode_removed={on_dismissed.clone()}
                            />
                        </div>
                    }
                }
            }
            <AudioPlayerBar />
        </div>
        <App_drawer />
        </>
    }
}

fn podcasts_missing() -> bool {
    Dispatch::<PodcastFeedState>::global().get().podcast_feed_return_extra.is_none()
}
