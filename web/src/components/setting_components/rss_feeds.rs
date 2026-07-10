use crate::components::context::{AppState, NotificationState};
use crate::components::gen_funcs::format_error_message;
use crate::requests::pod_req::{call_get_collections, call_get_playlists, Collection, Playlist};
use crate::requests::setting_reqs::{
    call_get_rss_key, call_rss_feed_status, call_toggle_rss_feeds,
};
use i18nrs::yew::use_translation;
use std::borrow::Borrow;
use web_sys::console;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

/// How many feeds to show per category before collapsing behind "Show more".
const FEED_CATEGORY_LIMIT: usize = 2;

#[derive(Properties, PartialEq)]
struct FeedCategoryProps {
    title: String,
    /// (label, url) pairs.
    feeds: Vec<(String, String)>,
}

/// A titled group of feed rows that collapses to `FEED_CATEGORY_LIMIT` rows
/// with a "Show more"/"Show less" toggle when it has more than that.
#[function_component(FeedCategory)]
fn feed_category(props: &FeedCategoryProps) -> Html {
    let (i18n, _) = use_translation();
    let expanded = use_state(|| false);

    let total = props.feeds.len();
    let visible = if *expanded { total } else { FEED_CATEGORY_LIMIT.min(total) };
    let hidden = total.saturating_sub(FEED_CATEGORY_LIMIT);

    let toggle = {
        let expanded = expanded.clone();
        Callback::from(move |_| expanded.set(!*expanded))
    };

    let toggle_label = if *expanded {
        i18n.t("rss_feeds.show_less").to_string()
    } else {
        format!("{} ({})", i18n.t("rss_feeds.show_more"), hidden)
    };
    let toggle_icon = if *expanded { "ph ph-caret-up" } else { "ph ph-caret-down" };

    html! {
        <>
        <div class="settings-section-subhead">{ &props.title }</div>
        { for props.feeds.iter().take(visible).map(|(label, url)| feed_row(label.clone(), url.clone())) }
        if total > FEED_CATEGORY_LIMIT {
            <div class="settings-row">
                <button
                    onclick={toggle}
                    class="btn btn-ghost"
                    style="display:flex;align-items:center;gap:6px;padding:6px 10px;font-size:12px;"
                >
                    { toggle_label }
                    <i class={toggle_icon}></i>
                </button>
            </div>
        }
        </>
    }
}

/// Render a single labeled, read-only RSS feed URL with a copy-to-clipboard button.
fn feed_row(label: String, url: String) -> Html {
    html! {
        <div class="settings-row" key={url.clone()}>
            <div>
                <div class="settings-row-label">{ label }</div>
            </div>
            <div class="settings-row-control" style="display:flex;align-items:center;gap:8px;max-width:320px;width:100%;">
                <input
                    type="text"
                    value={url.clone()}
                    readonly=true
                    class="input"
                    style="font-size:11px;"
                />
                <button
                    onclick={{
                        let url = url.clone();
                        Callback::from(move |_| {
                            if let Some(window) = web_sys::window() {
                                let clipboard = window.navigator().clipboard();
                                let _ = clipboard.write_text(&url);
                            }
                        })
                    }}
                    class="btn btn-ghost"
                    style="padding:6px 10px;white-space:nowrap;"
                >
                    <i class="ph ph-copy"></i>
                </button>
            </div>
        </div>
    }
}

#[function_component(RSSFeedSettings)]
pub fn rss_feed_settings() -> Html {
    let (i18n, _) = use_translation();
    let i18n_enable_rss_feeds = i18n.t("rss_feeds.enable_rss_feeds").to_string();
    let i18n_url_includes_api_key = i18n.t("rss_feeds.url_includes_api_key").to_string();
    let i18n_all_subscriptions = i18n.t("rss_feeds.all_subscriptions").to_string();
    let i18n_saved = i18n.t("rss_feeds.saved").to_string();
    let i18n_queue = i18n.t("rss_feeds.queue").to_string();
    let i18n_downloads = i18n.t("rss_feeds.downloads").to_string();
    let i18n_history = i18n.t("rss_feeds.history").to_string();
    let i18n_general_feeds = i18n.t("rss_feeds.general_feeds").to_string();
    let i18n_playlist_feeds = i18n.t("rss_feeds.playlist_feeds").to_string();
    let i18n_collection_feeds = i18n.t("rss_feeds.collection_feeds").to_string();
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    let rss_feed_status = use_state(|| false);
    let loading = use_state(|| false);
    // The user's RSS key; feed URLs are all derived from it.
    let rss_key = use_state(|| String::new());
    let playlists = use_state(|| Vec::<Playlist>::new());
    let collections = use_state(|| Vec::<Collection>::new());

    // Effect to get initial RSS feed status
    {
        let rss_feed_status = rss_feed_status.clone();
        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let rss_feed_status = rss_feed_status.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let future = async move {
                    if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                        let response = call_rss_feed_status(server_name, api_key.unwrap()).await;
                        match response {
                            Ok(status) => {
                                rss_feed_status.set(status);
                            }
                            Err(e) => console::log_1(
                                &format!("Error getting RSS feed status: {}", e).into(),
                            ),
                        }
                    }
                };
                spawn_local(future);
                || {}
            },
        );
    }

    let html_rss_status = rss_feed_status.clone();

    // Effect to fetch RSS key, playlists and collections when RSS feeds are enabled.
    {
        let rss_key = rss_key.clone();
        let playlists = playlists.clone();
        let collections = collections.clone();
        use_effect_with(
            (
                api_key.clone(),
                server_name.clone(),
                user_id.clone(),
                *rss_feed_status,
            ),
            move |(api_key, server_name, user_id, rss_enabled)| {
                let rss_key = rss_key.clone();
                let playlists = playlists.clone();
                let collections = collections.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let user_id = user_id.clone();
                let rss_enabled = *rss_enabled;
                spawn_local(async move {
                    if rss_enabled {
                        if let (Some(api_key), Some(server_name), Some(user_id)) =
                            (api_key, server_name, user_id)
                        {
                            let api_key = api_key.unwrap();
                            match call_get_rss_key(server_name.clone(), api_key.clone(), user_id)
                                .await
                            {
                                Ok(key) => rss_key.set(key),
                                Err(e) => {
                                    console::log_1(&format!("Error getting RSS key: {}", e).into());
                                    rss_key.set(String::new());
                                }
                            }
                            match call_get_playlists(&server_name, &api_key, user_id).await {
                                Ok(resp) => playlists.set(resp.playlists),
                                Err(e) => console::log_1(
                                    &format!("Error getting playlists: {}", e).into(),
                                ),
                            }
                            match call_get_collections(&server_name, &api_key, user_id).await {
                                Ok(cols) => collections.set(cols),
                                Err(e) => console::log_1(
                                    &format!("Error getting collections: {}", e).into(),
                                ),
                            }
                        }
                    } else {
                        rss_key.set(String::new());
                        playlists.set(Vec::new());
                        collections.set(Vec::new());
                    }
                });
                || {}
            },
        );
    }

    // Build the feed URLs from the RSS key. All variants share the base `/rss/{user_id}` path.
    let base_url = {
        let key = (*rss_key).clone();
        match (server_name.clone(), user_id, key.is_empty()) {
            (Some(server), Some(uid), false) => {
                Some(format!("{}/rss/{}?api_key={}", server, uid, key))
            }
            _ => None,
        }
    };

    // Build the collapsible feed categories (computed outside `html!` since it needs `let`s).
    let feeds_section = match (*rss_feed_status, base_url) {
        (true, Some(base_url)) => {
            let general_feeds = vec![
                (i18n_all_subscriptions.clone(), base_url.clone()),
                (i18n_saved.clone(), format!("{}&source=saved", base_url)),
                (i18n_queue.clone(), format!("{}&source=queue", base_url)),
                (i18n_downloads.clone(), format!("{}&source=downloads", base_url)),
                (i18n_history.clone(), format!("{}&source=history", base_url)),
            ];
            let playlist_feeds: Vec<(String, String)> = playlists.iter().map(|p| (
                p.name.clone(),
                format!("{}&source=playlist&id={}", base_url, p.playlist_id),
            )).collect();
            let collection_feeds: Vec<(String, String)> = collections.iter().map(|c| (
                c.name.clone(),
                format!("{}&source=collection&id={}", base_url, c.collection_id),
            )).collect();

            html! {
                <>
                <FeedCategory title={i18n_general_feeds.clone()} feeds={general_feeds} />
                if !playlist_feeds.is_empty() {
                    <FeedCategory title={i18n_playlist_feeds.clone()} feeds={playlist_feeds} />
                }
                if !collection_feeds.is_empty() {
                    <FeedCategory title={i18n_collection_feeds.clone()} feeds={collection_feeds} />
                }
                </>
            }
        }
        _ => html! {},
    };

    html! {
        <>
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{ &i18n_enable_rss_feeds }</div>
                <div class="settings-row-desc">{ &i18n_url_includes_api_key }</div>
            </div>
            <div class="settings-row-control">
                <label class="toggle">
                    <input
                        type="checkbox"
                        disabled={**loading.borrow()}
                        checked={**rss_feed_status.borrow()}
                        onclick={Callback::from(move |_| {
                            let api_key = api_key.clone();
                            let server_name = server_name.clone();
                            let rss_feed_status = html_rss_status.clone();
                            let _dispatch = _dispatch.clone();
                            let loading = loading.clone();
                            let future = async move {
                                loading.set(true);
                                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                                    let response = call_toggle_rss_feeds(server_name, api_key.unwrap()).await;
                                    match response {
                                        Ok(toggle_response) => {
                                            rss_feed_status.set(toggle_response.enabled);
                                        },
                                        Err(e) => {
                                            let formatted_error = format_error_message(&e.to_string());
                                            Dispatch::<NotificationState>::global().reduce_mut(|audio_state|
                                                audio_state.error_message = Some(format!("Error toggling RSS feeds: {}", formatted_error))
                                            );
                                        },
                                    }
                                }
                                loading.set(false);
                            };
                            spawn_local(future);
                        })}
                    />
                    <span class="toggle-track"><span class="toggle-thumb"></span></span>
                </label>
            </div>
        </div>
        { feeds_section }
        </>
    }
}
