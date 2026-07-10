use crate::components::context::{EpisodeStatusState, NotificationState};
use crate::components::gen_components::FallbackImage;
use crate::components::gen_funcs::format_time;
use crate::requests::episode::Episode;
use crate::requests::pod_req::{
    call_get_queued_episodes, call_remove_queued_episode, call_reorder_queue, QueuePodcastRequest,
    QueuedEpisodesResponse,
};
use i18nrs::yew::use_translation;
use std::collections::{HashMap, HashSet};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, MouseEvent};
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum SortMode {
    None,
    DateAsc,
    DateDesc,
}

/// Which destructive action the inline confirmation step is guarding.
#[derive(Clone, Copy, PartialEq)]
enum ConfirmAction {
    Reorder,
    Remove,
}

/// Parse a queue `episodepubdate` into a comparable key. The backend emits
/// `"%Y-%m-%dT%H:%M:%S"` (optionally with fractional seconds); malformed/empty dates fall back
/// to the Unix epoch so sorting never panics. Mirrors `gen_funcs::format_date`.
fn pubdate_key(s: &str) -> chrono::NaiveDateTime {
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f"))
        .unwrap_or_else(|_| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0)
                .map(|dt| dt.naive_utc())
                .expect("epoch is a valid timestamp")
        })
}

#[derive(Properties, PartialEq)]
pub struct QueueManageModalProps {
    /// Snapshot of the current queue order, taken when the modal opens.
    pub episodes: Vec<Episode>,
    pub server: String,
    pub api_key: Option<String>,
    pub user_id: i32,
    pub on_close: Callback<()>,
}

#[function_component(QueueManageModal)]
pub fn queue_manage_modal(props: &QueueManageModalProps) -> Html {
    let (i18n, _) = use_translation();

    let sort_mode = use_state(|| SortMode::None);
    let group_by_podcast = use_state(|| false);
    let search = use_state(String::new);
    let selected = use_state(HashSet::<i32>::new);
    let confirm = use_state(|| Option::<ConfirmAction>::None);
    let applying = use_state(|| false);

    // ── Derive the transformed ordering (this is what gets persisted). ────────────────────────
    // Search only affects which rows are *displayed*, never the order we send to the server —
    // filtering the persisted list would silently drop episodes from the queue.
    let ordered: Vec<Episode> = {
        let mut v = props.episodes.clone();
        match *sort_mode {
            SortMode::None => {}
            SortMode::DateAsc => v.sort_by_key(|e| pubdate_key(&e.episodepubdate)),
            SortMode::DateDesc => {
                v.sort_by_key(|e| pubdate_key(&e.episodepubdate));
                v.reverse();
            }
        }
        if *group_by_podcast {
            // Stable pass: group by podcast in first-seen order, preserving within-group order
            // (which is the date order above when a sort is also active).
            let mut order: Vec<i32> = Vec::new();
            let mut groups: HashMap<i32, Vec<Episode>> = HashMap::new();
            for ep in v.into_iter() {
                if !groups.contains_key(&ep.podcastid) {
                    order.push(ep.podcastid);
                }
                groups.entry(ep.podcastid).or_default().push(ep);
            }
            order
                .into_iter()
                .flat_map(|pid| groups.remove(&pid).unwrap_or_default())
                .collect()
        } else {
            v
        }
    };

    let has_order_change = *sort_mode != SortMode::None || *group_by_podcast;

    let search_term = (*search).to_lowercase();
    let displayed: Vec<Episode> = ordered
        .iter()
        .filter(|e| {
            search_term.is_empty()
                || e.episodetitle.to_lowercase().contains(&search_term)
                || e.podcastname.to_lowercase().contains(&search_term)
        })
        .cloned()
        .collect();

    // ── i18n strings ─────────────────────────────────────────────────────────────────────────
    let i18n_title = i18n.t("queue_panel.manage_queue_title").to_string();
    let i18n_sort = i18n.t("queue_panel.sort").to_string();
    let i18n_sort_none = i18n.t("queue_panel.sort_none").to_string();
    let i18n_newest = i18n.t("queue_panel.date_newest_first").to_string();
    let i18n_oldest = i18n.t("queue_panel.date_oldest_first").to_string();
    let i18n_group = i18n.t("queue_panel.group_by_podcast").to_string();
    let i18n_search = i18n.t("queue_panel.search_episodes").to_string();
    let i18n_preview = i18n.t("queue_panel.preview_new_order").to_string();
    let i18n_no_matches = i18n.t("queue_panel.no_matches").to_string();
    let i18n_apply = i18n.t("queue_panel.apply_new_order").to_string();
    let i18n_warning = i18n.t("queue_panel.irreversible_warning").to_string();
    let i18n_confirm_apply = i18n.t("queue_panel.confirm_apply").to_string();
    let i18n_confirm_apply_msg = i18n.t("queue_panel.confirm_apply_order").to_string();
    let i18n_cancel = i18n.t("queue_panel.cancel").to_string();
    let i18n_close = i18n.t("queue_panel.close").to_string();
    let i18n_confirm_remove = i18n.t("queue_panel.confirm_remove").to_string();
    let i18n_applying = i18n.t("queue_panel.applying").to_string();
    let selected_count = selected.len();
    let i18n_remove_selected = i18n
        .t("queue_panel.remove_selected_count")
        .replace("{count}", &selected_count.to_string());
    let i18n_confirm_remove_msg = i18n
        .t("queue_panel.confirm_remove_selected")
        .replace("{count}", &selected_count.to_string());

    // ── Callbacks ────────────────────────────────────────────────────────────────────────────
    let close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| on_close.emit(()))
    };
    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    let on_search = {
        let search = search.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            search.set(input.value());
        })
    };

    let make_sort = |mode: SortMode| {
        let sort_mode = sort_mode.clone();
        Callback::from(move |_: MouseEvent| sort_mode.set(mode))
    };
    let sort_none = make_sort(SortMode::None);
    let sort_newest = make_sort(SortMode::DateDesc);
    let sort_oldest = make_sort(SortMode::DateAsc);

    let toggle_group = {
        let group_by_podcast = group_by_podcast.clone();
        Callback::from(move |_: MouseEvent| {
            group_by_podcast.set(!*group_by_podcast);
        })
    };

    let toggle_select = {
        let selected = selected.clone();
        move |ep_id: i32| {
            let selected = selected.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                let mut set = (*selected).clone();
                if !set.remove(&ep_id) {
                    set.insert(ep_id);
                }
                selected.set(set);
            })
        }
    };

    let ask_reorder = {
        let confirm = confirm.clone();
        Callback::from(move |_: MouseEvent| confirm.set(Some(ConfirmAction::Reorder)))
    };
    let ask_remove = {
        let confirm = confirm.clone();
        Callback::from(move |_: MouseEvent| confirm.set(Some(ConfirmAction::Remove)))
    };
    let cancel_confirm = {
        let confirm = confirm.clone();
        Callback::from(move |_: MouseEvent| confirm.set(None))
    };

    // Commit the reordered queue permanently, then close.
    let do_reorder = {
        let server = props.server.clone();
        let api_key = props.api_key.clone();
        let user_id = props.user_id;
        let on_close = props.on_close.clone();
        let applying = applying.clone();
        let confirm = confirm.clone();
        let ordered = ordered.clone();
        Callback::from(move |_: MouseEvent| {
            let episode_ids: Vec<i32> = ordered.iter().map(|e| e.episodeid).collect();
            let ordered_snapshot = ordered.clone();
            let server = server.clone();
            let api_key = api_key.clone();
            let on_close = on_close.clone();
            let applying = applying.clone();
            let confirm = confirm.clone();
            applying.set(true);
            spawn_local(async move {
                match call_reorder_queue(&server, &api_key, &user_id, &episode_ids).await {
                    Ok(_) => {
                        Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                            s.queued_episodes = Some(QueuedEpisodesResponse {
                                episodes: ordered_snapshot,
                            });
                            s.queued_episode_ids = Some(episode_ids);
                        });
                        on_close.emit(());
                    }
                    Err(e) => {
                        Dispatch::<NotificationState>::global()
                            .reduce_mut(|s| s.error_message = Some(format!("{}", e)));
                        applying.set(false);
                        confirm.set(None);
                    }
                }
            });
        })
    };

    // Remove all selected episodes, refetch the queue, keep the modal open (parent re-feeds props).
    let do_remove = {
        let server = props.server.clone();
        let api_key = props.api_key.clone();
        let user_id = props.user_id;
        let episodes = props.episodes.clone();
        let selected = selected.clone();
        let applying = applying.clone();
        let confirm = confirm.clone();
        Callback::from(move |_: MouseEvent| {
            let targets: Vec<(i32, bool)> = episodes
                .iter()
                .filter(|e| selected.contains(&e.episodeid))
                .map(|e| (e.episodeid, e.is_youtube))
                .collect();
            if targets.is_empty() {
                confirm.set(None);
                return;
            }
            let server = server.clone();
            let api_key = api_key.clone();
            let selected = selected.clone();
            let applying = applying.clone();
            let confirm = confirm.clone();
            applying.set(true);
            spawn_local(async move {
                for (episode_id, is_youtube) in targets {
                    let req = QueuePodcastRequest {
                        episode_id,
                        user_id,
                        is_youtube,
                    };
                    let _ = call_remove_queued_episode(&server, &api_key, &req).await;
                }
                if let Ok(mut eps) = call_get_queued_episodes(&server, &api_key, &user_id).await {
                    eps.sort_by_key(|e| e.queueposition.unwrap_or(i32::MAX));
                    let ids: Vec<i32> = eps.iter().map(|e| e.episodeid).collect();
                    Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                        s.queued_episodes = Some(QueuedEpisodesResponse { episodes: eps });
                        s.queued_episode_ids = Some(ids);
                    });
                }
                selected.set(HashSet::new());
                applying.set(false);
                confirm.set(None);
            });
        })
    };

    let is_applying = *applying;

    html! {
        <div class="queue-manage-overlay" onclick={close.clone()}>
            <div class="queue-manage-modal" onclick={stop}>
                <div class="queue-manage-head">
                    <div class="queue-manage-title">{ i18n_title }</div>
                    <button class="player-btn" onclick={close.clone()} title={i18n_close}>
                        <i class="ph ph-x"></i>
                    </button>
                </div>

                <div class="queue-manage-toolbar">
                    <div class="queue-manage-sort">
                        <span class="queue-manage-label">{ i18n_sort }</span>
                        <div class="queue-manage-segmented">
                            <button
                                class={classes!("queue-manage-seg", (*sort_mode == SortMode::None).then_some("is-active"))}
                                onclick={sort_none}
                            >{ i18n_sort_none }</button>
                            <button
                                class={classes!("queue-manage-seg", (*sort_mode == SortMode::DateDesc).then_some("is-active"))}
                                onclick={sort_newest}
                            >{ i18n_newest }</button>
                            <button
                                class={classes!("queue-manage-seg", (*sort_mode == SortMode::DateAsc).then_some("is-active"))}
                                onclick={sort_oldest}
                            >{ i18n_oldest }</button>
                        </div>
                    </div>

                    <button
                        class={classes!("queue-manage-toggle", (*group_by_podcast).then_some("is-active"))}
                        onclick={toggle_group}
                    >
                        <i class={classes!("ph", if *group_by_podcast { "ph-check-square" } else { "ph-square" })}></i>
                        { i18n_group }
                    </button>

                    <div class="queue-manage-search">
                        <i class="ph ph-magnifying-glass"></i>
                        <input
                            type="text"
                            value={(*search).clone()}
                            oninput={on_search}
                            placeholder={i18n_search}
                        />
                    </div>
                </div>

                if has_order_change {
                    <div class="queue-manage-warning">
                        <i class="ph ph-warning"></i>
                        <span>{ i18n_warning.clone() }</span>
                    </div>
                }

                <div class="queue-manage-preview-label">{ i18n_preview }</div>
                <div class="queue-manage-preview">
                    if displayed.is_empty() {
                        <div class="queue-manage-empty">{ i18n_no_matches }</div>
                    } else {
                        { for displayed.iter().enumerate().map(|(idx, ep)| {
                            let ep_id = ep.episodeid;
                            let is_sel = selected.contains(&ep_id);
                            let on_row_select = toggle_select(ep_id);
                            let date = crate::components::gen_funcs::format_date(&ep.episodepubdate);
                            let dur = format_time(ep.episodeduration);
                            html! {
                                <div
                                    class={classes!("queue-manage-row", is_sel.then_some("is-selected"))}
                                    key={ep_id}
                                    onclick={on_row_select.clone()}
                                >
                                    <button class="queue-manage-check" onclick={on_row_select}>
                                        <i class={classes!("ph", if is_sel { "ph-check-square" } else { "ph-square" })}></i>
                                    </button>
                                    <span class="queue-manage-index">{ idx + 1 }</span>
                                    <FallbackImage
                                        src={ep.episodeartwork.clone()}
                                        alt="Episode art"
                                        class="queue-manage-art"
                                    />
                                    <div class="queue-manage-row-body">
                                        <div class="queue-manage-row-title">{ ep.episodetitle.clone() }</div>
                                        <div class="queue-manage-row-sub">
                                            { format!("{} \u{00B7} {} \u{00B7} {}", ep.podcastname.clone(), date, dur) }
                                        </div>
                                    </div>
                                </div>
                            }
                        }) }
                    }
                </div>

                <div class="queue-manage-actions">
                    <button
                        class="queue-manage-btn queue-manage-btn-danger"
                        onclick={ask_remove}
                        disabled={selected_count == 0 || is_applying}
                    >
                        <i class="ph ph-trash"></i>
                        { i18n_remove_selected }
                    </button>
                    <button
                        class="queue-manage-btn queue-manage-btn-primary"
                        onclick={ask_reorder}
                        disabled={!has_order_change || is_applying}
                    >
                        <i class="ph ph-check"></i>
                        { if is_applying { i18n_applying.clone() } else { i18n_apply.clone() } }
                    </button>
                </div>

                if let Some(action) = *confirm {
                    <div class="queue-manage-confirm-scrim">
                        <div class="queue-manage-confirm">
                            <i class="ph ph-warning-circle queue-manage-confirm-icon"></i>
                            <div class="queue-manage-confirm-msg">
                                { match action {
                                    ConfirmAction::Reorder => i18n_confirm_apply_msg.clone(),
                                    ConfirmAction::Remove => i18n_confirm_remove_msg.clone(),
                                } }
                            </div>
                            <div class="queue-manage-confirm-actions">
                                <button
                                    class="queue-manage-btn"
                                    onclick={cancel_confirm}
                                    disabled={is_applying}
                                >{ i18n_cancel }</button>
                                <button
                                    class="queue-manage-btn queue-manage-btn-danger"
                                    onclick={match action { ConfirmAction::Reorder => do_reorder, ConfirmAction::Remove => do_remove }}
                                    disabled={is_applying}
                                >
                                    { match action {
                                        ConfirmAction::Reorder => i18n_confirm_apply.clone(),
                                        ConfirmAction::Remove => i18n_confirm_remove.clone(),
                                    } }
                                </button>
                            </div>
                        </div>
                    </div>
                }
            </div>
        </div>
    }
}
