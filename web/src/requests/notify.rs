//! Notification classification helpers — the single, consistent entry point for
//! surfacing feedback going forward.
//!
//! Three tiers (see the notification redesign):
//! - **Toast** — transient, client-only feedback for a user-initiated action
//!   ("Queued!", "Saved", "Copied"). Use [`toast_info`] / [`toast_success`] /
//!   [`toast_error`]. Flashes ~5s, never persisted, never in the Activity drawer.
//! - **Task** — ongoing, progress-bearing background op. Created server-side and
//!   streamed over the task WebSocket; not raised from the client.
//! - **Message (alert)** — durable server-side notice (feed errors, AI failures,
//!   new episodes). Created server-side and delivered over the same socket; the
//!   client only lists / reads / dismisses them via the helpers below.
//!
//! Rule of thumb: client-initiated confirmation → `toast_*`; server-side durable
//! event → a backend notification. The frontend never fabricates durable messages.

use anyhow::Error;
use gloo::net::http::Request;
use yewdux::prelude::*;

use crate::components::context::NotificationState;

/// Flash a transient info/success toast.
pub fn toast_info(dispatch: &Dispatch<NotificationState>, message: impl Into<String>) {
    let message = message.into();
    dispatch.reduce_mut(move |state| state.info_message = Some(message));
}

/// Alias for [`toast_info`] — reads better at success call sites.
pub fn toast_success(dispatch: &Dispatch<NotificationState>, message: impl Into<String>) {
    toast_info(dispatch, message);
}

/// Flash a transient error toast.
pub fn toast_error(dispatch: &Dispatch<NotificationState>, message: impl Into<String>) {
    let message = message.into();
    dispatch.reduce_mut(move |state| state.error_message = Some(message));
}

fn clean_base(server_name: &str) -> String {
    server_name.trim_end_matches('/').to_string()
}

/// Dismiss (delete) a durable notification on the server.
pub async fn dismiss_notification(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    id: &str,
) -> Result<(), Error> {
    let url = format!(
        "{}/api/data/notifications/{}/dismiss?user_id={}",
        clean_base(server_name),
        id,
        user_id
    );
    let resp = Request::post(&url)
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("dismiss notification: {e}")))?;
    if !resp.ok() {
        return Err(Error::msg(format!(
            "dismiss notification failed: {}",
            resp.status()
        )));
    }
    Ok(())
}

/// Mark a durable notification read on the server.
pub async fn mark_notification_read(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    id: &str,
) -> Result<(), Error> {
    let url = format!(
        "{}/api/data/notifications/{}/read?user_id={}",
        clean_base(server_name),
        id,
        user_id
    );
    let resp = Request::post(&url)
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("mark notification read: {e}")))?;
    if !resp.ok() {
        return Err(Error::msg(format!(
            "mark notification read failed: {}",
            resp.status()
        )));
    }
    Ok(())
}

/// Clear all (or all read) durable notifications for the user on the server.
pub async fn clear_notifications(
    server_name: &str,
    api_key: &str,
    user_id: i32,
    read_only: bool,
) -> Result<(), Error> {
    let url = format!(
        "{}/api/data/notifications/clear?user_id={}&read_only={}",
        clean_base(server_name),
        user_id,
        read_only
    );
    let resp = Request::post(&url)
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("clear notifications: {e}")))?;
    if !resp.ok() {
        return Err(Error::msg(format!(
            "clear notifications failed: {}",
            resp.status()
        )));
    }
    Ok(())
}

async fn post_task_action(server_name: &str, api_key: &str, path: &str) -> Result<(), Error> {
    let url = format!("{}/api/tasks/{}", clean_base(server_name), path);
    let resp = Request::post(&url)
        .header("Api-Key", api_key)
        .send()
        .await
        .map_err(|e| Error::msg(format!("task {path}: {e}")))?;
    if !resp.ok() {
        return Err(Error::msg(format!("task {path} failed: {}", resp.status())));
    }
    Ok(())
}

/// Remove a finished task from the user's activity list on the server.
pub async fn dismiss_task(server_name: &str, api_key: &str, task_id: &str) -> Result<(), Error> {
    post_task_action(server_name, api_key, &format!("{task_id}/dismiss")).await
}

/// Cancel a task that is still queued.
pub async fn cancel_task(server_name: &str, api_key: &str, task_id: &str) -> Result<(), Error> {
    post_task_action(server_name, api_key, &format!("{task_id}/cancel")).await
}

/// Remove every finished task from the user's activity list on the server.
pub async fn clear_finished_tasks(server_name: &str, api_key: &str) -> Result<(), Error> {
    post_task_action(server_name, api_key, "clear_finished").await
}
