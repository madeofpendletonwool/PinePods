// task_req.rs
use anyhow::{Error, Result};
use futures::{SinkExt, StreamExt};
use gloo::net::http::Request;
use gloo::net::websocket::{futures::WebSocket, Message};
use serde::Deserialize;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use web_sys::console;
use yewdux::prelude::*;

use crate::components::context::AppState;
use crate::components::notification_center::TaskProgress;

// Response structs
#[derive(Deserialize, Debug)]
struct TaskListResponse {
    tasks: Vec<TaskProgress>,
}

// Struct for parsing the backend TaskInfo format (used in "initial" events)
#[derive(Deserialize, Debug, Clone)]
struct BackendTaskInfo {
    pub id: String,
    pub task_type: String,
    pub user_id: i32,
    pub status: String,
    pub progress: f64,
    pub message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub result: Option<serde_json::Value>,
}

#[derive(Deserialize, Debug)]
struct TaskUpdateMessage {
    event: String,
    #[serde(default)]
    tasks: Option<Vec<BackendTaskInfo>>,
    #[serde(default)]
    task: Option<RawTaskProgress>,
}

// New struct specifically for parsing the raw data coming from the server
#[derive(Deserialize, Debug, Clone)]
struct RawTaskProgress {
    pub task_id: String,
    pub user_id: i32,
    pub item_id: Value, // Use serde_json::Value to accept any JSON type
    pub r#type: String,
    pub progress: f64,
    pub status: String,
    pub started_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub details: Option<HashMap<String, Value>>, // Also using Value for details
}

// Convert BackendTaskInfo to TaskProgress
impl From<BackendTaskInfo> for TaskProgress {
    fn from(backend_task: BackendTaskInfo) -> Self {
        // Extract details from result field
        let mut details: HashMap<String, String> = HashMap::new();

        if let Some(result) = &backend_task.result {
            // Try to extract useful information from result
            if let Some(obj) = result.as_object() {
                for (key, value) in obj {
                    match value {
                        Value::String(s) => {
                            details.insert(key.clone(), s.clone());
                        }
                        Value::Number(n) => {
                            details.insert(key.clone(), n.to_string());
                        }
                        _ => {
                            details.insert(key.clone(), value.to_string().trim_matches('"').to_string());
                        }
                    }
                }
            }
        }

        // Add status message if available
        if let Some(message) = &backend_task.message {
            details.insert("status_text".to_string(), message.clone());
        }

        // Check if task is completed before moving values
        let is_completed = backend_task.status == "SUCCESS" || backend_task.status == "FAILED";

        TaskProgress {
            task_id: backend_task.id,
            user_id: backend_task.user_id,
            item_id: None, // BackendTaskInfo doesn't have item_id
            r#type: backend_task.task_type,
            progress: backend_task.progress,
            status: backend_task.status,
            started_at: backend_task.created_at,
            completed_at: if is_completed {
                Some(backend_task.updated_at)
            } else {
                None
            },
            details: Some(details),
            completion_time: if is_completed {
                Some(js_sys::Date::now())
            } else {
                None
            },
        }
    }
}

// Connect to the task websocket
pub async fn connect_to_task_websocket(
    server_name: String,
    user_id: i32,
    api_key: String,
    dispatch: Dispatch<AppState>,
) -> Result<(), Error> {
    // Normalize server name for WebSocket connection
    let clean_server_name = server_name
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    let ws_protocol = if server_name.starts_with("https://") {
        "wss://"
    } else {
        "ws://"
    };

    // Create WebSocket URL - changed to use the working pattern
    let url = format!(
        "{}{}/ws/api/tasks/{}?api_key={}",
        ws_protocol, clean_server_name, user_id, api_key
    );

    console::log_1(&format!("Connecting to task WebSocket at: {}", url).into());

    // Try to open WebSocket
    let ws_result = WebSocket::open(&url);
    if let Err(e) = ws_result {
        console::error_1(&format!("Failed to open task WebSocket: {:?}", e).into());
        console::warn_1(&"Falling back to REST API for task updates".into());

        // If WebSocket fails, fall back to REST API
        spawn_local({
            let server_name_clone = server_name.clone();
            let api_key_clone = api_key.clone();
            let dispatch_clone = dispatch.clone();
            async move {
                let _ =
                    fetch_active_tasks(server_name_clone, user_id, api_key_clone, dispatch_clone)
                        .await;
            }
        });

        return Err(Error::msg(format!("Failed to open WebSocket: {:?}", e)));
    }

    let mut websocket = ws_result.unwrap();

    // Track if this task should keep running
    let active = Rc::new(RefCell::new(true));
    let active_clone = active.clone();

    // Process incoming messages in a separate task
    let dispatch_clone = dispatch.clone();
    let server_name_clone = server_name.clone();
    let api_key_clone = api_key.clone();

    // Ping with a separate WebSocket connection periodically
    let ping_url = url.clone();
    let _ping_handle = gloo_timers::callback::Interval::new(30_000, move || {
        // Only ping if the connection is still active
        if !*active_clone.borrow() {
            return;
        }
        // Create a new WebSocket just for the ping
        let ping_url_handle = ping_url.clone();
        spawn_local(async move {
            // Try to connect with a new socket for ping
            match WebSocket::open(&ping_url_handle) {
                Ok(mut ping_socket) => {
                    // Send ping
                    let _ = ping_socket
                        .send(Message::Text(r#"{"action":"ping"}"#.to_string()))
                        .await;
                    // Wait for a response or timeout
                    let _ = gloo_timers::future::TimeoutFuture::new(5_000).await;
                    // Close the ping socket - add the required arguments
                    let _ = ping_socket.close(None, None); // Remove .await and add parameters
                }
                Err(_) => {
                    // Just log the error, don't retry the main connection here
                    console::warn_1(&"Failed to open ping WebSocket".into());
                }
            }
        });
    });

    // Main websocket message handling loop
    spawn_local(async move {
        while let Some(msg) = websocket.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Parse the incoming message
                    match serde_json::from_str::<TaskUpdateMessage>(&text) {
                        Ok(update) => {
                            match update.event.as_str() {
                                "initial" | "refresh" => {
                                    if let Some(backend_tasks) = update.tasks {
                                        // Convert BackendTaskInfo to TaskProgress
                                        let tasks: Vec<TaskProgress> = backend_tasks
                                            .into_iter()
                                            .map(|t| t.into())
                                            .collect();

                                        dispatch_clone.reduce_mut(|state| {
                                            state.active_tasks = Some(tasks);
                                        });
                                    }
                                }
                                // WebSocket message handling section in task_reqs.rs
                                // Replace the "update" event handler with this code:
                                "update" => {
                                    if let Some(raw_task) = update.task {
                                        // Convert RawTaskProgress to TaskProgress
                                        let item_id = match raw_task.item_id {
                                            Value::Number(num) => Some(num.to_string()),
                                            Value::String(s) => Some(s),
                                            _ => None,
                                        };

                                        // Extract episode names and other details from the raw details
                                        let mut details: HashMap<String, String> = HashMap::new();

                                        // Fix for the borrow issue in task_reqs.rs

                                        // Process the details field - fixed version that doesn't move raw_details
                                        if let Some(ref raw_details) = raw_task.details {
                                            // First, create a copy of the keys we need to check later
                                            let has_episode_title =
                                                raw_details.contains_key("episode_title");
                                            let has_item_title =
                                                raw_details.contains_key("item_title");

                                            // Then process each key-value pair
                                            for (key, value) in raw_details {
                                                match (key.as_str(), value) {
                                                    // Special handling for episode_id to fetch episode name
                                                    ("episode_id", Value::Number(num)) => {
                                                        details.insert(
                                                            "episode_id".to_string(),
                                                            num.to_string(),
                                                        );

                                                        // Try to extract a more user-friendly name
                                                        let episode_id = num.as_i64().unwrap_or(0);

                                                        // Use the flags we saved earlier instead of re-checking raw_details
                                                        if !has_episode_title && !has_item_title {
                                                            details.insert(
                                                                "episode_title".to_string(),
                                                                format!("Episode #{}", episode_id),
                                                            );
                                                        }
                                                    }
                                                    // Handle normal string cases
                                                    (_, Value::String(s)) => {
                                                        details.insert(key.clone(), s.clone());
                                                    }
                                                    // Convert other types to strings
                                                    (_, value) => {
                                                        details.insert(
                                                            key.clone(),
                                                            value
                                                                .to_string()
                                                                .trim_matches('"')
                                                                .to_string(),
                                                        );
                                                    }
                                                }
                                            }
                                        }

                                        // Default status text based on task type and status
                                        if !details.contains_key("status_text") {
                                            let default_status = match (
                                                raw_task.r#type.as_str(),
                                                raw_task.status.as_str(),
                                            ) {
                                                ("podcast_download", "DOWNLOADING") => format!(
                                                    "Downloading {}",
                                                    details
                                                        .get("episode_title")
                                                        .unwrap_or(&"episode".to_string())
                                                ),
                                                ("podcast_download", "PROCESSING") => format!(
                                                    "Processing {}",
                                                    details
                                                        .get("episode_title")
                                                        .unwrap_or(&"episode".to_string())
                                                ),
                                                ("podcast_download", "SUCCESS") => format!(
                                                    "Downloaded {}",
                                                    details
                                                        .get("episode_title")
                                                        .unwrap_or(&"episode".to_string())
                                                ),
                                                ("podcast_download", "FAILED") => format!(
                                                    "Failed to download {}",
                                                    details
                                                        .get("episode_title")
                                                        .unwrap_or(&"episode".to_string())
                                                ),
                                                ("feed_refresh", _) => {
                                                    "Refreshing podcast feeds".to_string()
                                                }
                                                ("youtube_download", _) => format!(
                                                    "YouTube download: {}",
                                                    details
                                                        .get("item_title")
                                                        .unwrap_or(&"video".to_string())
                                                ),
                                                _ => format!(
                                                    "{} task: {}",
                                                    raw_task.r#type, raw_task.status
                                                ),
                                            };
                                            details
                                                .insert("status_text".to_string(), default_status);
                                        }

                                        let task = TaskProgress {
                                            task_id: raw_task.task_id,
                                            user_id: raw_task.user_id,
                                            item_id,
                                            r#type: raw_task.r#type,
                                            progress: raw_task.progress,
                                            status: raw_task.status.clone(),
                                            started_at: raw_task.started_at,
                                            completed_at: raw_task.completed_at,
                                            details: Some(details),
                                            // Add a timestamp for auto-removal of completed tasks
                                            completion_time: if raw_task.status == "SUCCESS"
                                                || raw_task.status == "FAILED"
                                            {
                                                Some(js_sys::Date::now())
                                            } else {
                                                None
                                            },
                                        };

                                        dispatch_clone.reduce_mut(|state| {
                                            let mut tasks =
                                                state.active_tasks.clone().unwrap_or_default();

                                            // Find and update existing task or add new one
                                            let mut found = false;
                                            for existing_task in tasks.iter_mut() {
                                                if existing_task.task_id == task.task_id {
                                                    *existing_task = task.clone();
                                                    found = true;
                                                    break;
                                                }
                                            }

                                            if !found {
                                                console::log_1(
                                                    &format!(
                                                        "Adding new task: {} - status: {}",
                                                        task.task_id, task.status
                                                    )
                                                    .into(),
                                                );
                                                tasks.push(task.clone());
                                            }

                                            // Auto-cleanup completed tasks after a delay
                                            tasks.retain(|t| {
                                                if let Some(completion_time) = t.completion_time {
                                                    // Remove completed tasks after 30 seconds (30000 ms)
                                                    const TASK_DISPLAY_DURATION: f64 = 30000.0;
                                                    let current_time = js_sys::Date::now();
                                                    return (current_time - completion_time)
                                                        < TASK_DISPLAY_DURATION;
                                                }
                                                true
                                            });

                                            state.active_tasks = Some(tasks);
                                        });
                                    }
                                }
                                "pong" => {
                                    // Server responded to our ping
                                    console::log_1(&"Received pong from server".into());
                                }
                                _ => console::log_1(
                                    &format!("Unknown event type: {}", update.event).into(),
                                ),
                            }
                        }
                        Err(e) => {
                            console::error_1(
                                &format!(
                                    "Failed to parse WebSocket message: {}. Text: {}",
                                    e, text
                                )
                                .into(),
                            );
                        }
                    }
                }
                Ok(Message::Bytes(_)) => {
                    console::log_1(&"Binary message received, ignoring".into());
                }
                Err(e) => {
                    console::error_1(&format!("WebSocket error: {:?}", e).into());

                    // Mark connection as inactive
                    *active.borrow_mut() = false;

                    // Try to reconnect with REST API
                    let dispatch_inner = dispatch_clone.clone();
                    spawn_local(async move {
                        let _ = fetch_active_tasks(
                            server_name_clone,
                            user_id,
                            api_key_clone,
                            dispatch_inner,
                        )
                        .await;
                    });

                    break;
                }
            }
        }

        // Mark connection as inactive when the loop exits
        *active.borrow_mut() = false;
        console::log_1(&"WebSocket connection closed".into());
    });

    Ok(())
}

// REST API fallback to fetch active tasks
pub async fn fetch_active_tasks(
    server_name: String,
    user_id: i32,
    api_key: String,
    dispatch: Dispatch<AppState>,
) -> Result<(), Error> {
    console::log_1(&"Fetching active tasks via REST API".into());

    let url = format!("{}/api/tasks/active?user_id={}", server_name, user_id);

    match Request::get(&url)
        .header("Content-Type", "application/json")
        .header("X-Api-Key", &api_key)
        .send()
        .await
    {
        Ok(response) => {
            if response.ok() {
                let task_list: TaskListResponse = response.json().await?;

                dispatch.reduce_mut(|state| {
                    state.active_tasks = Some(task_list.tasks);
                });

                Ok(())
            } else if response.status() == 404 {
                // Handle 404 gracefully
                console::warn_1(&"Task API endpoint not found (404), using empty task list".into());

                // Initialize empty task list to avoid UI errors
                dispatch.reduce_mut(|state| {
                    state.active_tasks = Some(Vec::new());
                });

                Ok(())
            } else {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Failed to read error message".to_string());

                console::error_1(&format!("Error fetching tasks: {}", error_text).into());
                Err(Error::msg(format!("Failed to fetch tasks: {}", error_text)))
            }
        }
        Err(e) => {
            console::error_1(&format!("Request error: {:?}", e).into());
            Err(Error::msg(format!("Request error: {:?}", e)))
        }
    }
}

// Initialize WebSocket connection or fall back to REST API
pub fn init_task_monitoring(state: &AppState, dispatch: Dispatch<AppState>) {
    if let (Some(user_id), Some(Some(api_key)), Some(server_name)) = (
        state.user_details.as_ref().map(|ud| ud.UserID.clone()),
        state.auth_details.as_ref().map(|ud| ud.api_key.clone()),
        state.auth_details.as_ref().map(|ud| ud.server_name.clone()),
    ) {
        // Copy owned values to avoid lifetime issues
        let server_name_owned = server_name.clone();
        let api_key_owned = api_key.clone();
        let user_id_owned = user_id;
        let dispatch_clone = dispatch.clone();

        spawn_local(async move {
            match connect_to_task_websocket(
                server_name_owned.clone(),
                user_id_owned,
                api_key_owned.clone(),
                dispatch_clone.clone(),
            )
            .await
            {
                Ok(_) => console::log_1(&"Task WebSocket connected successfully".into()),
                Err(e) => {
                    console::error_1(
                        &format!("Failed to connect to task WebSocket: {:?}", e).into(),
                    );
                    // Fall back to REST API if WebSocket fails
                    if let Err(e) = fetch_active_tasks(
                        server_name_owned,
                        user_id_owned,
                        api_key_owned,
                        dispatch_clone,
                    )
                    .await
                    {
                        console::error_1(
                            &format!("Failed to fetch tasks via REST API: {:?}", e).into(),
                        );
                    }
                }
            }
        });
    } else {
        console::warn_1(&"Missing user credentials, cannot initialize task monitoring".into());
    }
}
