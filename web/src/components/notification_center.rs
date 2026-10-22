// notification_center.rs
//
// The Activity center: a right-edge drawer (Direction 2 "Status cards", compact
// density) that shows ongoing background TASKS and durable server MESSAGES under
// All / Active / Done tabs. Completed tasks move to "Done" and stay until the
// user clears them (the old 30s auto-yank is gone). Transient toasts for
// client-initiated actions live in `ToastNotification` below.
use crate::components::context::{AppState, NotificationMessage, NotificationState};
use crate::pages::routes::Route;
use crate::requests::pod_req::RefreshProgress;
use crate::requests::task_reqs::{init_task_monitoring, parse_rfc3339_ms};
use gloo_timers::callback::Interval;
use gloo_timers::callback::Timeout;
use i18nrs::yew::use_translation;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

// Task progress state stored in NotificationState.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct TaskProgress {
    pub task_id: String,
    pub user_id: i32,
    pub item_id: Option<String>,
    pub r#type: String,
    pub progress: f64,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub details: Option<HashMap<String, String>>,
    #[serde(default)]
    pub completion_time: Option<f64>, // JS timestamp used for relative-time labels
    // ---- Direction-2 activity-center enrichment (mirrors backend TaskInfo/TaskUpdate) ----
    #[serde(default)]
    pub art_url: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub group_label: Option<String>,
    #[serde(default)]
    pub total: Option<i32>,
}

impl TaskProgress {
    // Create a TaskProgress object from RefreshProgress data
    #[allow(dead_code)]
    pub fn from_refresh_progress(progress: &RefreshProgress) -> Self {
        let progress_percentage = if progress.total > 0 {
            (progress.current as f64 / progress.total as f64) * 100.0
        } else {
            0.0
        };

        Self {
            task_id: format!("feed_refresh_{}", js_sys::Date::now()),
            user_id: 0,
            item_id: None,
            r#type: "feed_refresh".to_string(),
            progress: progress_percentage,
            status: "PROGRESS".to_string(),
            started_at: format!("{}", js_sys::Date::now()),
            completed_at: None,
            details: Some({
                let mut details = HashMap::new();
                details.insert(
                    "current_podcast".to_string(),
                    progress.current_podcast.clone(),
                );
                details.insert("current".to_string(), progress.current.to_string());
                details.insert("total".to_string(), progress.total.to_string());
                details
            }),
            completion_time: None,
            art_url: None,
            detail: None,
            group: None,
            group_label: None,
            total: None,
        }
    }

    /// The card sub-line: explicit `detail`, else `details.status_text`.
    fn sub_line(&self) -> Option<String> {
        if let Some(d) = &self.detail {
            if !d.is_empty() {
                return Some(d.clone());
            }
        }
        self.details
            .as_ref()
            .and_then(|d| d.get("status_text"))
            .filter(|s| !s.is_empty())
            .cloned()
    }
}

// ===================== classification helpers =====================

fn is_progress(status: &str) -> bool {
    matches!(
        status,
        "PROGRESS" | "STARTED" | "DOWNLOADING" | "PROCESSING" | "FINALIZING"
    )
}
fn is_active(status: &str) -> bool {
    status == "PENDING" || is_progress(status)
}
fn is_done(status: &str) -> bool {
    status == "SUCCESS" || status == "FAILED"
}
/// Semantic kind driving color (`k-*` CSS classes).
fn kind_class(status: &str) -> &'static str {
    match status {
        "PENDING" => "queued",
        "SUCCESS" => "success",
        "FAILED" => "failed",
        _ => "active",
    }
}
/// Sort rank: active first, then queued, failed, success.
fn rank(status: &str) -> u8 {
    if is_progress(status) {
        0
    } else if status == "PENDING" {
        1
    } else if status == "FAILED" {
        2
    } else {
        3
    }
}
fn task_icon(task_type: &str) -> &'static str {
    match task_type {
        "download_episode" | "podcast_download" | "bulk_download" | "download_all_episodes" => {
            "ph-download-simple"
        }
        "feed_refresh" => "ph-arrows-clockwise",
        "youtube_download" | "download_video" | "download_all_videos" => "ph-youtube-logo",
        "playlist_generation" | "update_playlists" => "ph-list-checks",
        "opml_import" | "add_podcast_episodes" => "ph-upload-simple",
        "refresh_gpodder_subscriptions" | "gpodder_subscription_refresh" | "sync_setup" => "ph-arrows-left-right",
        "refresh_nextcloud_subscriptions" | "nextcloud_auth" => "ph-cloud",
        "manual_backup_to_directory" => "ph-floppy-disk",
        "restore_from_backup_file" => "ph-clock-counter-clockwise",
        "cleanup_tasks" => "ph-broom",
        "refresh_hosts" => "ph-users",
        _ => "ph-circle",
    }
}
/// Message severity → (kind class, icon).
fn msg_kind(severity: &str) -> (&'static str, &'static str) {
    match severity {
        "error" => ("error", "ph-warning-circle"),
        "warning" => ("error", "ph-warning"),
        "success" => ("success", "ph-check-circle"),
        _ => ("info", "ph-info"),
    }
}
fn msg_counts_badge(severity: &str) -> bool {
    severity == "error" || severity == "warning"
}
/// Relative time label from a JS epoch-ms timestamp.
fn rel_time(ts: f64) -> String {
    let secs = (js_sys::Date::now() - ts) / 1000.0;
    if secs < 45.0 {
        return "just now".to_string();
    }
    let mins = (secs / 60.0).round();
    if mins < 60.0 {
        return format!("{}m ago", mins as i64);
    }
    let hours = (mins / 60.0).round();
    if hours < 24.0 {
        return format!("{}h ago", hours as i64);
    }
    format!("{}d ago", (hours / 24.0).round() as i64)
}

// A group of sibling tasks folded into one collapsible summary row.
struct GroupNode {
    key: String,
    label: String,
    ttype: String,
    art: Option<String>,
    children: Vec<TaskProgress>,
    done_count: usize,
    active_count: usize,
    progress: f64,
    all_done: bool,
    any_failed: bool,
    count: i32,
}

enum RenderNode {
    Task(TaskProgress),
    Group(GroupNode),
}

/// Fold tasks sharing a `group` key into synthetic group nodes; ungrouped tasks
/// pass through. Input should already be sorted.
fn group_tasks(tasks: Vec<TaskProgress>) -> Vec<RenderNode> {
    let mut out: Vec<RenderNode> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    for task in tasks {
        match task.group.clone() {
            None => out.push(RenderNode::Task(task)),
            Some(key) => {
                if let Some(&i) = index.get(&key) {
                    if let RenderNode::Group(node) = &mut out[i] {
                        node.children.push(task);
                    }
                } else {
                    index.insert(key.clone(), out.len());
                    out.push(RenderNode::Group(GroupNode {
                        label: task.group_label.clone().unwrap_or_else(|| "Tasks".to_string()),
                        ttype: task.r#type.clone(),
                        art: task.art_url.clone(),
                        count: task.total.unwrap_or(0),
                        key,
                        children: vec![task],
                        done_count: 0,
                        active_count: 0,
                        progress: 0.0,
                        all_done: false,
                        any_failed: false,
                    }));
                }
            }
        }
    }

    for node in out.iter_mut() {
        if let RenderNode::Group(g) = node {
            let ch = &g.children;
            g.done_count = ch.iter().filter(|t| is_done(&t.status)).count();
            g.active_count = ch.iter().filter(|t| is_active(&t.status)).count();
            g.progress = if ch.is_empty() {
                0.0
            } else {
                ch.iter().map(|t| t.progress).sum::<f64>() / ch.len() as f64
            };
            g.all_done = ch.iter().all(|t| is_done(&t.status));
            g.any_failed = ch.iter().any(|t| t.status == "FAILED");
            if g.count <= 0 {
                g.count = ch.len() as i32;
            }
        }
    }
    out
}

fn sort_tasks(mut tasks: Vec<TaskProgress>) -> Vec<TaskProgress> {
    tasks.sort_by(|a, b| {
        rank(&a.status).cmp(&rank(&b.status)).then_with(|| {
            b.completion_time
                .unwrap_or(0.0)
                .partial_cmp(&a.completion_time.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    tasks
}

#[function_component(NotificationCenter)]
pub fn notification_center() -> Html {
    let (i18n, _) = use_translation();

    // Task type + status labels (reuse existing i18n keys).
    let l_download = i18n.t("notification_center.task_download").to_string();
    let l_feed_refresh = i18n.t("notification_center.task_feed_refresh").to_string();
    let l_playlist = i18n.t("notification_center.task_playlist").to_string();
    let l_youtube = i18n.t("notification_center.task_youtube_download").to_string();
    let l_bulk = i18n.t("notification_center.task_bulk_download").to_string();
    let l_youtube_bulk = i18n
        .t("notification_center.task_youtube_bulk_download")
        .to_string();
    let l_opml = i18n.t("notification_center.task_opml_import").to_string();
    let l_add_podcast = i18n.t("notification_center.task_add_podcast").to_string();
    let l_backup = i18n.t("notification_center.task_backup").to_string();
    let l_restore = i18n.t("notification_center.task_restore").to_string();
    let l_gpodder = i18n.t("notification_center.task_gpodder_sync").to_string();
    let l_sync_setup = i18n.t("sync_settings.task_first_sync").to_string();
    let l_nextcloud = i18n.t("notification_center.task_nextcloud_sync").to_string();
    let l_nextcloud_auth = i18n.t("notification_center.task_nextcloud_auth").to_string();
    let l_playlist_update = i18n.t("notification_center.task_playlist_update").to_string();
    let l_cleanup = i18n.t("notification_center.task_cleanup").to_string();
    let l_refresh_hosts = i18n.t("notification_center.task_refresh_hosts").to_string();

    let s_queued = i18n.t("notification_center.status_queued").to_string();
    let s_in_progress = i18n.t("notification_center.status_in_progress").to_string();
    let s_completed = i18n.t("notification_center.status_completed").to_string();
    let s_failed = i18n.t("notification_center.status_failed").to_string();

    let l_activity = i18n.t("notification_center.activity").to_string();
    let l_clear = i18n.t("notification_center.clear").to_string();
    let l_clear_completed = i18n.t("notification_center.clear_completed").to_string();
    let l_settings = i18n.t("notification_center.settings").to_string();

    let type_label = {
        move |t: &str| -> String {
            match t {
                "download_episode" | "podcast_download" => l_download.clone(),
                "bulk_download" | "download_all_episodes" => l_bulk.clone(),
                "feed_refresh" => l_feed_refresh.clone(),
                "youtube_download" | "download_video" => l_youtube.clone(),
                "download_all_videos" => l_youtube_bulk.clone(),
                "playlist_generation" => l_playlist.clone(),
                "update_playlists" => l_playlist_update.clone(),
                "opml_import" => l_opml.clone(),
                "add_podcast_episodes" => l_add_podcast.clone(),
                "manual_backup_to_directory" => l_backup.clone(),
                "restore_from_backup_file" => l_restore.clone(),
                "refresh_gpodder_subscriptions" | "gpodder_subscription_refresh" => l_gpodder.clone(),
                "sync_setup" => l_sync_setup.clone(),
                "refresh_nextcloud_subscriptions" => l_nextcloud.clone(),
                "nextcloud_auth" => l_nextcloud_auth.clone(),
                "cleanup_tasks" => l_cleanup.clone(),
                "refresh_hosts" => l_refresh_hosts.clone(),
                other => other.to_string(),
            }
        }
    };
    let status_label = {
        move |status: &str| -> String {
            match kind_class(status) {
                "queued" => s_queued.clone(),
                "success" => s_completed.clone(),
                "failed" => s_failed.clone(),
                _ => s_in_progress.clone(),
            }
        }
    };

    let (state, dispatch) = use_store::<NotificationState>();
    let (app_state_store, _) = use_store::<AppState>();
    let navigator = use_navigator();

    let drawer_open = use_state(|| false);
    let tab = use_state(|| "all".to_string());
    let expanded = use_state(HashSet::<String>::new);
    let ws_initialized = use_state(|| false);

    // Credentials for server-side notification management.
    let creds: Option<(i32, String, String)> =
        match (&app_state_store.user_details, &app_state_store.auth_details) {
            (Some(ud), Some(ad)) => ad
                .api_key
                .clone()
                .map(|key| (ud.UserID, key, ad.server_name.clone())),
            _ => None,
        };

    // Initialize the task/notification WebSocket on mount.
    {
        let dispatch = dispatch.clone();
        let ws_initialized = ws_initialized.clone();
        use_effect_with((), move |_| {
            if !*ws_initialized {
                let app_state = Dispatch::<AppState>::global().get();
                init_task_monitoring(&app_state, dispatch);
                ws_initialized.set(true);
            }
            || ()
        });
    }

    // Open the persistent now-playing socket as soon as credentials are available.
    {
        use_effect_with(creds.clone(), move |creds| {
            if let Some((user_id, api_key, server_name)) = creds.clone() {
                crate::requests::now_playing::connect_now_playing_ws(server_name, user_id, api_key);
            }
            || ()
        });
    }

    // ---- derived data ----
    let tasks = state.active_tasks.clone().unwrap_or_default();
    let messages = state.messages.clone();

    let active_tasks: Vec<TaskProgress> =
        tasks.iter().filter(|t| is_active(&t.status)).cloned().collect();
    let done_tasks: Vec<TaskProgress> =
        tasks.iter().filter(|t| is_done(&t.status)).cloned().collect();
    let error_msgs: Vec<NotificationMessage> = messages
        .iter()
        .filter(|m| msg_counts_badge(&m.severity))
        .cloned()
        .collect();

    let running: Vec<&TaskProgress> = tasks.iter().filter(|t| is_progress(&t.status)).collect();
    let running_count = running.len();
    let progress_avg = if running_count > 0 {
        running.iter().map(|t| t.progress).sum::<f64>() / running_count as f64
    } else {
        0.0
    };

    let badge = active_tasks.len() + error_msgs.len();

    // ---- callbacks ----
    let toggle_drawer = {
        let drawer_open = drawer_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            drawer_open.set(!*drawer_open);
        })
    };
    let close_drawer = {
        let drawer_open = drawer_open.clone();
        Callback::from(move |_| drawer_open.set(false))
    };
    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    let set_tab = {
        let tab = tab.clone();
        move |name: &'static str| {
            let tab = tab.clone();
            Callback::from(move |_| tab.set(name.to_string()))
        }
    };

    let toggle_group = {
        let expanded = expanded.clone();
        Callback::from(move |g: String| {
            let mut next = (*expanded).clone();
            if next.contains(&g) {
                next.remove(&g);
            } else {
                next.insert(g);
            }
            expanded.set(next);
        })
    };

    let dismiss_task = {
        let dispatch = dispatch.clone();
        Callback::from(move |id: String| {
            dispatch.reduce_mut(move |state| {
                if let Some(tasks) = &mut state.active_tasks {
                    tasks.retain(|t| t.task_id != id);
                }
            });
        })
    };

    let clear_done = {
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|state| {
                if let Some(tasks) = &mut state.active_tasks {
                    tasks.retain(|t| !is_done(&t.status));
                }
            });
        })
    };

    let dismiss_message = {
        let dispatch = dispatch.clone();
        let creds = creds.clone();
        Callback::from(move |id: String| {
            {
                let id = id.clone();
                dispatch.reduce_mut(move |state| state.messages.retain(|m| m.id != id));
            }
            if let Some((user_id, api_key, server_name)) = creds.clone() {
                let id = id.clone();
                spawn_local(async move {
                    let _ = crate::requests::notify::dismiss_notification(
                        &server_name,
                        &api_key,
                        user_id,
                        &id,
                    )
                    .await;
                });
            }
        })
    };

    let go_settings = {
        let navigator = navigator.clone();
        let drawer_open = drawer_open.clone();
        Callback::from(move |_| {
            drawer_open.set(false);
            if let Some(nav) = &navigator {
                nav.push(&Route::Settings);
            }
        })
    };

    // ---- build sections for the active tab ----
    // Each section: (label, task render nodes, standalone messages).
    let mut sections: Vec<(&'static str, Vec<RenderNode>, Vec<NotificationMessage>)> = Vec::new();
    match tab.as_str() {
        "active" => {
            if !active_tasks.is_empty() {
                sections.push(("In progress", group_tasks(sort_tasks(active_tasks.clone())), vec![]));
            }
            if !error_msgs.is_empty() {
                sections.push(("Needs attention", vec![], error_msgs.clone()));
            }
        }
        "done" => {
            if !done_tasks.is_empty() {
                sections.push(("Completed", group_tasks(sort_tasks(done_tasks.clone())), vec![]));
            }
        }
        _ => {
            if !tasks.is_empty() {
                sections.push(("Tasks", group_tasks(sort_tasks(tasks.clone())), vec![]));
            }
            if !messages.is_empty() {
                sections.push(("Messages", vec![], messages.clone()));
            }
        }
    }
    let show_labels = sections.len() > 1;
    let is_empty = sections.iter().all(|(_, n, m)| n.is_empty() && m.is_empty());

    let empty_copy: (&str, &str) = match tab.as_str() {
        "active" => (
            "Nothing running",
            "Downloads and feed refreshes will show up here.",
        ),
        "done" => (
            "Nothing completed yet",
            "Finished tasks land here so you can review them.",
        ),
        _ => (
            "You're all caught up",
            "No downloads, syncs, or messages right now.",
        ),
    };

    let show_summary = (tab.as_str() == "all" || tab.as_str() == "active") && running_count > 0;
    let footer_visible = !done_tasks.is_empty() || !tasks.is_empty() || !messages.is_empty();

    // ---- render one node ----
    let render_task = |task: &TaskProgress| -> Html {
        let kind = kind_class(&task.status);
        let show_prog = is_progress(&task.status);
        let title = task
            .details
            .as_ref()
            .and_then(|d| d.get("episode_title").or_else(|| d.get("item_title")))
            .cloned()
            .unwrap_or_else(|| type_label(&task.r#type));
        let sub = task.sub_line();
        let id = task.task_id.clone();
        let on_dismiss = dismiss_task.clone();
        html! {
            <div class={classes!("nc-card", format!("k-{kind}"))}>
                <span class={classes!("nc-thumb", format!("k-{kind}"))}>
                    {
                        if let Some(art) = &task.art_url {
                            html! { <img src={art.clone()} alt="" /> }
                        } else {
                            html! { <i class={classes!("ph", task_icon(&task.r#type))}></i> }
                        }
                    }
                </span>
                <div class="nc-main-col">
                    <div class="nc-line1">{ title }</div>
                    <div class="nc-line2">
                        <span>{ type_label(&task.r#type) }</span>
                        {
                            if let Some(sub) = &sub {
                                html! { <><span class="nc-dot-sep">{"·"}</span><span>{ sub.clone() }</span></> }
                            } else { html! {} }
                        }
                    </div>
                    { if show_prog { render_progress(task.progress, true) } else { html! {} } }
                </div>
                <div class="nc-trail">
                    <span class={classes!("nc-pill", format!("k-{kind}"))}>{ status_label(&task.status) }</span>
                    {
                        if is_done(&task.status) {
                            let on_dismiss = on_dismiss.clone();
                            html! {
                                <button class="nc-x" title="Dismiss"
                                    onclick={Callback::from(move |_| on_dismiss.emit(id.clone()))}>
                                    <i class="ph ph-x"></i>
                                </button>
                            }
                        } else if show_prog {
                            html! { <span class="nc-time">{ format!("{}%", task.progress.round() as i64) }</span> }
                        } else { html! {} }
                    }
                </div>
            </div>
        }
    };

    let render_group = |g: &GroupNode| -> Html {
        let kind = if g.all_done {
            if g.any_failed { "failed" } else { "success" }
        } else {
            "active"
        };
        let is_open = expanded.contains(&g.key);
        let key = g.key.clone();
        let on_toggle = toggle_group.clone();
        html! {
            <>
                <div class={classes!("nc-card", format!("k-{kind}"), "nc-group-row", is_open.then_some("is-open"))}
                     style="cursor:pointer"
                     onclick={Callback::from(move |_| on_toggle.emit(key.clone()))}>
                    <span class={classes!("nc-thumb", format!("k-{kind}"))}>
                        {
                            if let Some(art) = &g.art {
                                html! { <img src={art.clone()} alt="" /> }
                            } else {
                                html! { <i class={classes!("ph", task_icon(&g.ttype))}></i> }
                            }
                        }
                    </span>
                    <div class="nc-main-col">
                        <div class="nc-line1">{ g.label.clone() }</div>
                        <div class="nc-line2">
                            <span class="nc-count-chip">{ format!("{}/{} done", g.done_count, g.count) }</span>
                            {
                                if !g.all_done {
                                    html! { <><span class="nc-dot-sep">{"·"}</span>
                                        <span class="nc-status-word k-active">{ format!("{} running", g.active_count) }</span></> }
                                } else { html! {} }
                            }
                        </div>
                        { if !g.all_done { render_progress(g.progress, true) } else { html! {} } }
                    </div>
                    <div class="nc-trail">
                        <i class="ph ph-caret-right nc-group-caret"></i>
                    </div>
                </div>
                {
                    if is_open {
                        html! {
                            <div class="nc-children">
                                { for g.children.iter().map(|c| {
                                    let ck = kind_class(&c.status);
                                    let ctitle = c.details.as_ref()
                                        .and_then(|d| d.get("episode_title").or_else(|| d.get("item_title")))
                                        .cloned()
                                        .unwrap_or_else(|| c.r#type.clone());
                                    let cp = if is_progress(&c.status) {
                                        format!("{}%", c.progress.round() as i64)
                                    } else { status_label(&c.status) };
                                    html! {
                                        <div class="nc-child">
                                            <span class={classes!("nc-child-dot", format!("k-{ck}"))}></span>
                                            <span class="nc-child-t">{ ctitle }</span>
                                            <span class="nc-child-p">{ cp }</span>
                                        </div>
                                    }
                                }) }
                            </div>
                        }
                    } else { html! {} }
                }
            </>
        }
    };

    let render_message = |m: &NotificationMessage| -> Html {
        let (kind, icon) = msg_kind(&m.severity);
        let id = m.id.clone();
        let on_dismiss = dismiss_message.clone();
        let rel = parse_rfc3339_ms(&m.created_at).map(rel_time).unwrap_or_default();
        html! {
            <div class={classes!("nc-card", format!("k-{kind}"))}>
                <span class={classes!("nc-thumb", format!("k-{kind}"))}>
                    {
                        if let Some(art) = &m.art_url {
                            html! { <img src={art.clone()} alt="" /> }
                        } else {
                            html! { <i class={classes!("ph", icon)}></i> }
                        }
                    }
                </span>
                <div class="nc-main-col">
                    <div class="nc-line1" style="white-space:normal;line-height:1.4">{ m.title.clone() }</div>
                    {
                        if let Some(body) = &m.body {
                            html! { <div class="nc-line2" style="white-space:normal">{ body.clone() }</div> }
                        } else { html! {} }
                    }
                    <div class="nc-line2"><span class="nc-time" style="opacity:.7">{ rel }</span></div>
                </div>
                <div class="nc-trail">
                    <button class="nc-x" title="Dismiss"
                        onclick={Callback::from(move |_| on_dismiss.emit(id.clone()))}>
                        <i class="ph ph-x"></i>
                    </button>
                </div>
            </div>
        }
    };

    let bell_icon = if badge > 0 { "ph-bell-ringing" } else { "ph-bell" };
    let badge_text = if badge > 9 { "9+".to_string() } else { badge.to_string() };

    html! {
        <div class="notification-center">
            <button type="button"
                class={classes!("nc-bell", (*drawer_open).then_some("is-open"))}
                onclick={toggle_drawer} title="Activity" aria-label="Activity">
                <i class={classes!("ph", bell_icon)}></i>
                { if badge > 0 { html! { <span class="nc-bell-badge">{ badge_text }</span> } } else { html! {} } }
            </button>

            // Scrim + panel stay mounted; the `.is-open` class drives the CSS
            // transition both ways so closing animates (not just opening).
            <div class={classes!("nc-scrim", (*drawer_open).then_some("is-open"))} onclick={close_drawer.clone()}></div>
            <div class={classes!("nc-panel", "nc-dir-2", "nc-form-drawer", (*drawer_open).then_some("is-open"))}
                 data-density="compact" onclick={stop}>
                                <div class="nc-header">
                                    <h3 class="nc-h-title">{ &l_activity }</h3>
                                    <div class="nc-h-spacer"></div>
                                    <button class="nc-h-btn" title="Close" onclick={close_drawer.clone()}>
                                        <i class="ph ph-arrow-line-right"></i>
                                    </button>
                                </div>

                                <div class="nc-tabs">
                                    { render_tab("all", "All", tasks.len() + messages.len(), &tab, set_tab("all")) }
                                    { render_tab("active", "Active", active_tasks.len() + error_msgs.len(), &tab, set_tab("active")) }
                                    { render_tab("done", "Done", done_tasks.len(), &tab, set_tab("done")) }
                                </div>

                                {
                                    if show_summary {
                                        html! {
                                            <div class="nc-summary">
                                                <div class="nc-summary-top">
                                                    <span class="nc-spinner"><i class="ph ph-circle-notch"></i></span>
                                                    <span>{ format!("{} {} running", running_count, if running_count == 1 { "task" } else { "tasks" }) }</span>
                                                    <span class="nc-summary-pct">{ format!("{}%", progress_avg.round() as i64) }</span>
                                                </div>
                                                { render_progress(progress_avg, false) }
                                            </div>
                                        }
                                    } else { html! {} }
                                }

                                <div class="nc-body">
                                    {
                                        if is_empty {
                                            html! {
                                                <div class="nc-empty">
                                                    <span class="nc-empty-ico"><i class="ph ph-check-circle"></i></span>
                                                    <div class="nc-empty-t">{ empty_copy.0 }</div>
                                                    <div class="nc-empty-s">{ empty_copy.1 }</div>
                                                </div>
                                            }
                                        } else {
                                            html! {
                                                { for sections.iter().map(|(label, nodes, msgs)| {
                                                    let count = nodes.len() + msgs.len();
                                                    let is_done_section = *label == "Completed";
                                                    let clear_done = clear_done.clone();
                                                    html! {
                                                        <div>
                                                            {
                                                                if show_labels {
                                                                    html! {
                                                                        <div class="nc-section-label">
                                                                            <span>{ *label }</span>
                                                                            <span class="n">{ count }</span>
                                                                            {
                                                                                if is_done_section {
                                                                                    html! { <button class="nc-section-clear"
                                                                                        onclick={Callback::from(move |_| clear_done.emit(()))}>{ &l_clear }</button> }
                                                                                } else { html! {} }
                                                                            }
                                                                        </div>
                                                                    }
                                                                } else { html! {} }
                                                            }
                                                            { for nodes.iter().map(|n| match n {
                                                                RenderNode::Task(t) => render_task(t),
                                                                RenderNode::Group(g) => render_group(g),
                                                            }) }
                                                            { for msgs.iter().map(|m| render_message(m)) }
                                                        </div>
                                                    }
                                                }) }
                                            }
                                        }
                                    }
                                </div>

                                {
                                    if footer_visible {
                                        let clear_done_footer = clear_done.clone();
                                        html! {
                                            <div class="nc-footer">
                                                {
                                                    if !done_tasks.is_empty() {
                                                        html! { <button class="nc-foot-btn"
                                                            onclick={Callback::from(move |_| clear_done_footer.emit(()))}>
                                                            <i class="ph ph-check"></i>{ &l_clear_completed }</button> }
                                                    } else {
                                                        html! { <span style="font-size:12px;opacity:.55;padding:6px 8px">{ format!("{} active", running_count) }</span> }
                                                    }
                                                }
                                                <div class="nc-foot-spacer"></div>
                                                <button class="nc-foot-btn" title="Notification settings" onclick={go_settings}>
                                                    <i class="ph ph-gear"></i>{ &l_settings }
                                                </button>
                                            </div>
                                        }
                                    } else { html! {} }
                                }
            </div>
        </div>
    }
}

fn render_progress(progress: f64, show_pct: bool) -> Html {
    let width = progress.clamp(2.0, 100.0);
    html! {
        <div class="nc-prog">
            <div class="nc-prog-track">
                <div class="nc-prog-fill" style={format!("width:{}%", width)}></div>
            </div>
            { if show_pct { html! { <span class="nc-prog-pct">{ format!("{}%", progress.round() as i64) }</span> } } else { html! {} } }
        </div>
    }
}

fn render_tab(
    id: &str,
    label: &str,
    n: usize,
    active: &UseStateHandle<String>,
    onclick: Callback<MouseEvent>,
) -> Html {
    let is_active = active.as_str() == id;
    html! {
        <button class={classes!("nc-tab", is_active.then_some("is-active"))} {onclick}>
            { label }
            { if n > 0 { html! { <span class="nc-tab-pip">{ n }</span> } } else { html! {} } }
        </button>
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[derive(Clone, Debug, PartialEq)]
struct ToastItem {
    id: usize,
    content: String,
    toast_type: String,
    visible: bool,
    expiry_time: f64, // When this toast should expire (timestamp)
}

#[function_component(ToastNotification)]
pub fn toast_notification() -> Html {
    let (state, dispatch) = use_store::<NotificationState>();
    let toast_queue = use_state(|| vec![]);
    let counter = use_state(|| 0);

    // Single cleanup timer for all toasts - runs every 100ms
    {
        let toast_queue = toast_queue.clone();

        use_effect(move || {
            let interval_handle = Interval::new(100, move || {
                let now = js_sys::Date::now();

                toast_queue.set({
                    let mut new_queue: Vec<ToastItem> = (*toast_queue).clone();
                    let mut changed = false;

                    for toast in new_queue.iter_mut() {
                        if toast.visible && now >= toast.expiry_time {
                            toast.visible = false;
                            changed = true;
                        }
                    }

                    let before_len = new_queue.len();
                    new_queue.retain(|toast| toast.visible || now < toast.expiry_time + 500.0);
                    if new_queue.len() != before_len {
                        changed = true;
                    }

                    if changed {
                        new_queue
                    } else {
                        (*toast_queue).clone()
                    }
                });
            });

            move || {
                interval_handle.cancel();
            }
        });
    }

    // Process error messages
    {
        let toast_queue = toast_queue.clone();
        let counter = counter.clone();
        let dispatch = dispatch.clone();
        let error_message = state.error_message.clone();

        use_effect_with(error_message.clone(), move |error_message| {
            if let Some(error_msg) = error_message {
                let existing_message = (*toast_queue).iter().any(|toast: &ToastItem| {
                    toast.content == *error_msg && toast.toast_type == "error" && toast.visible
                });

                if !existing_message {
                    let new_id = *counter;
                    counter.set(new_id + 1);

                    let now = js_sys::Date::now();
                    let expiry_time = now + 5000.0;

                    let new_toast = ToastItem {
                        id: new_id,
                        content: error_msg.clone(),
                        toast_type: "error".to_string(),
                        visible: true,
                        expiry_time,
                    };

                    toast_queue.set({
                        let mut new_queue = (*toast_queue).clone();
                        new_queue.push(new_toast);
                        new_queue
                    });

                    let dispatch_clone = dispatch.clone();
                    let error_msg_clone = error_msg.clone();
                    let handle = Timeout::new(5500, move || {
                        dispatch_clone.reduce_mut(|state| {
                            if state.error_message.as_ref() == Some(&error_msg_clone) {
                                state.error_message = None;
                            }
                        });
                    });

                    handle.forget();
                }
            }
            || ()
        });
    }

    // Process info messages
    {
        let toast_queue = toast_queue.clone();
        let counter = counter.clone();
        let dispatch = dispatch.clone();
        let info_message = state.info_message.clone();

        use_effect_with(info_message.clone(), move |info_message| {
            if let Some(info_msg) = info_message {
                let existing_message = (*toast_queue).iter().any(|toast: &ToastItem| {
                    toast.content == *info_msg && toast.toast_type == "info" && toast.visible
                });

                if !existing_message {
                    let new_id = *counter;
                    counter.set(new_id + 1);

                    let now = js_sys::Date::now();
                    let expiry_time = now + 5000.0;

                    let new_toast = ToastItem {
                        id: new_id,
                        content: info_msg.clone(),
                        toast_type: "info".to_string(),
                        visible: true,
                        expiry_time,
                    };

                    toast_queue.set({
                        let mut new_queue = (*toast_queue).clone();
                        new_queue.push(new_toast);
                        new_queue
                    });

                    let dispatch_clone = dispatch.clone();
                    let info_msg_clone = info_msg.clone();
                    let handle = Timeout::new(5500, move || {
                        dispatch_clone.reduce_mut(|state| {
                            if state.info_message.as_ref() == Some(&info_msg_clone) {
                                state.info_message = None;
                            }
                        });
                    });

                    handle.forget();
                }
            }
            || ()
        });
    }

    html! {
        <div class="toast-container">
            {
                (*toast_queue).iter().map(|toast| {
                    let toast_class = if toast.toast_type == "error" {
                        "toast-error"
                    } else {
                        "toast-info"
                    };

                    let icon_class = if toast.toast_type == "error" {
                        "ph ph-warning-circle"
                    } else {
                        "ph ph-info"
                    };

                    html! {
                        <div
                            key={toast.id}
                            class={classes!(
                                "toast-item",
                                if toast.visible { "toast-visible" } else { "toast-hidden" }
                            )}
                        >
                            <div class={classes!("toast", toast_class)}>
                                <div class="flex items-center justify-between">
                                    <div class="item_conatiner-text flex items-center">
                                        <i class={classes!(icon_class, "item_container-text", "text-xl", "mr-2")}></i>
                                        <p class="toast-message">
                                            {toast.content.clone()}
                                        </p>
                                    </div>
                                    <button
                                        class="toast-dismiss text-lg ml-2"
                                        onclick={
                                            let toast_queue = toast_queue.clone();
                                            let toast_id = toast.id;
                                            Callback::from(move |_| {
                                                toast_queue.set({
                                                    let mut new_queue = (*toast_queue).clone();
                                                    if let Some(t) = new_queue.iter_mut().find(|t| t.id == toast_id) {
                                                        t.visible = false;
                                                    }
                                                    new_queue
                                                });
                                            })
                                        }
                                    >
                                        <i class="ph ph-x"></i>
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                }).collect::<Html>()
            }
        </div>
    }
}
