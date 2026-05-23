use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_get_rss_key, call_rss_feed_status, call_toggle_rss_feeds,
};
use std::borrow::Borrow;
use web_sys::console;
use yew::platform::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(RSSFeedSettings)]
pub fn rss_feed_settings() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    let rss_feed_status = use_state(|| false);
    let loading = use_state(|| false);
    let rss_feed_url = use_state(|| String::new());

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

    // Effect to fetch RSS key and generate URL when RSS feeds are enabled
    {
        let rss_feed_url = rss_feed_url.clone();
        use_effect_with(
            (
                api_key.clone(),
                server_name.clone(),
                user_id.clone(),
                *rss_feed_status,
            ),
            move |(api_key, server_name, user_id, rss_enabled)| {
                let rss_feed_url = rss_feed_url.clone();
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let user_id = user_id.clone();
                let rss_enabled = *rss_enabled;
                spawn_local(async move {
                    if rss_enabled {
                        if let (Some(api_key), Some(server_name), Some(user_id)) =
                            (api_key, server_name, user_id)
                        {
                            match call_get_rss_key(server_name.clone(), api_key.unwrap(), user_id)
                                .await
                            {
                                Ok(rss_key) => {
                                    let url = format!(
                                        "{}/rss/{}?api_key={}",
                                        server_name, user_id, rss_key
                                    );
                                    rss_feed_url.set(url);
                                }
                                Err(e) => {
                                    console::log_1(&format!("Error getting RSS key: {}", e).into());
                                    rss_feed_url.set(String::new());
                                }
                            }
                        }
                    } else {
                        rss_feed_url.set(String::new());
                    }
                });
                || {}
            },
        );
    }

    html! {
        <>
        <div class="settings-row">
            <div>
                <div class="settings-row-label">{"Enable RSS feeds"}</div>
                <div class="settings-row-desc">{"The URL includes your API key — keep it private."}</div>
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
                                            _dispatch.reduce_mut(|audio_state|
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
        if *rss_feed_status {
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{"Your RSS feed URL"}</div>
                </div>
                <div class="settings-row-control" style="display:flex;align-items:center;gap:8px;max-width:320px;width:100%;">
                    <input
                        type="text"
                        value={(*rss_feed_url).clone()}
                        readonly=true
                        class="input"
                        style="font-size:11px;"
                    />
                    <button
                        onclick={{
                            let rss_feed_url = rss_feed_url.clone();
                            Callback::from(move |_| {
                                if let Some(window) = web_sys::window() {
                                    let clipboard = window.navigator().clipboard();
                                    let _ = clipboard.write_text(&(*rss_feed_url));
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
        </>
    }
}
