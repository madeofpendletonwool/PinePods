// notification_center.rs
use crate::components::context::AppState;
use crate::requests::task_reqs::init_task_monitoring;
use gloo_timers::callback::Interval;
use gloo_timers::callback::Timeout;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsCast;
use web_sys::{window, Event, MouseEvent};
use yew::prelude::*;
use yewdux::prelude::*;

// Task progress state that will be stored in AppState
// In notification_center.rs, update the TaskProgress struct:

// Task progress state that will be stored in AppState
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
    pub completion_time: Option<f64>, // JS timestamp for auto-cleanup
}

// Custom serde module to handle both string and integer values for item_id
mod item_id_string_or_int {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::fmt;

    // A custom deserialize implementation that accepts either a string or an integer
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringOrIntVisitor;

        impl<'de> serde::de::Visitor<'de> for StringOrIntVisitor {
            type Value = Option<String>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("string or integer")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Some(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Some(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Some(value.to_string()))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(Some(value.to_string()))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(None)
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(None)
            }
        }

        deserializer.deserialize_any(StringOrIntVisitor)
    }

    // A simple serialization implementation that just converts to a string
    pub fn serialize<S>(value: &Option<String>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(v) => serializer.serialize_str(v),
            None => serializer.serialize_none(),
        }
    }
}

#[function_component(NotificationCenter)]
pub fn notification_center() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let dropdown_open = use_state(|| false);
    let notification_count = use_state(|| 0);
    let ws_initialized = use_state(|| false);
    let show_completed = use_state(|| true); // State to toggle showing completed tasks

    // Initialize WebSocket connection on component mount
    {
        let state = state.clone();
        let dispatch = dispatch.clone();
        let ws_initialized = ws_initialized.clone();

        use_effect_with((), move |_| {
            if !*ws_initialized {
                init_task_monitoring(&state, dispatch);
                ws_initialized.set(true);
            }
            || ()
        });
    }

    // Auto-hide completed tasks after delay
    {
        let dispatch = dispatch.clone();
        let show_completed = show_completed.clone();

        use_effect_with((), move |_| {
            let interval = gloo_timers::callback::Interval::new(5000, move || {
                dispatch.reduce_mut(|state| {
                    if let Some(tasks) = &mut state.active_tasks {
                        // Auto-cleanup completed tasks after a delay
                        tasks.retain(|t| {
                            if let Some(completion_time) = t.completion_time {
                                // Check if task should be auto-removed
                                const TASK_DISPLAY_DURATION: f64 = 30000.0; // 30 seconds
                                let current_time = js_sys::Date::now();
                                return (current_time - completion_time) < TASK_DISPLAY_DURATION;
                            }
                            true
                        });
                    }
                });
            });

            // Cleanup function to cancel the interval when component unmounts
            move || drop(interval)
        });
    }

    // Get active tasks from state
    let active_tasks = state.active_tasks.clone().unwrap_or_default();

    // Get error and info messages
    let error_message = state.error_message.clone();
    let info_message = state.info_message.clone();

    {
        let dispatch = dispatch.clone();
        let info_message = info_message.clone();

        use_effect_with(info_message, move |info| {
            let timeout = if info.is_some() {
                // Clear info messages after 5 seconds
                let dispatch_clone = dispatch.clone();
                let handle = gloo_timers::callback::Timeout::new(5000, move || {
                    dispatch_clone.reduce_mut(|state| {
                        state.info_message = None;
                    });
                });
                Some(handle)
            } else {
                None
            };

            // Return cleanup function to cancel timeout if the component unmounts
            move || {
                if let Some(timeout) = timeout {
                    // Timeout is automatically dropped/cancelled here
                    drop(timeout);
                }
            }
        });
    }

    // Filter tasks based on show_completed setting
    let filtered_tasks = if *show_completed {
        active_tasks.clone()
    } else {
        active_tasks
            .iter()
            .filter(|task| !(task.status == "SUCCESS" || task.status == "FAILED"))
            .cloned()
            .collect::<Vec<_>>()
    };

    // Count active (non-completed) tasks for badge
    let active_count = active_tasks
        .iter()
        .filter(|task| !(task.status == "SUCCESS" || task.status == "FAILED"))
        .count();

    // Count notifications - active tasks plus any error or info messages
    {
        let notification_count = notification_count.clone();
        let active_count = active_count;
        let has_error = error_message.is_some();
        // Info messages no longer count toward the notification badge

        use_effect_with((active_count, has_error), move |(tasks_len, has_error)| {
            // Info messages are not included in the count
            let count = *tasks_len + (*has_error as usize);
            notification_count.set(count);
            || ()
        });
    }

    // Handle toggle dropdown
    let toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            dropdown_open.set(!*dropdown_open);
        })
    };

    // Handle toggle show completed
    let toggle_show_completed = {
        let show_completed = show_completed.clone();
        Callback::from(move |_| {
            show_completed.set(!*show_completed);
        })
    };

    // Handle dismiss all completed tasks
    let dismiss_completed = {
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|state| {
                if let Some(ref mut tasks) = state.active_tasks {
                    tasks.retain(|task| !(task.status == "SUCCESS" || task.status == "FAILED"));
                }
            });
        })
    };

    // Handle dismiss single task
    let dismiss_task = {
        let dispatch = dispatch.clone();
        Callback::from(move |task_id: String| {
            dispatch.reduce_mut(|state| {
                if let Some(ref mut tasks) = state.active_tasks {
                    tasks.retain(|task| task.task_id != task_id);
                }
            });
        })
    };

    // Clear all notifications
    let clear_all = {
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            dispatch.reduce_mut(|state| {
                state.active_tasks = Some(Vec::new());
                state.error_message = None;
                state.info_message = None;
            });
        })
    };

    // Handle click outside to close dropdown
    {
        let dropdown_open = dropdown_open.clone();
        use_effect_with(*dropdown_open, move |is_open| {
            if *is_open {
                // Document click event handling code (unchanged from original)
                // ...
                let document = window().unwrap().document().unwrap();
                let document_clone = document.clone();
                let dropdown_open = dropdown_open.clone();

                let closure = Closure::wrap(Box::new(move |event: Event| {
                    let target = event.target().unwrap();

                    // Try to cast as Element first
                    if let Some(element) = target.dyn_ref::<web_sys::Element>() {
                        // Check if the click is outside the notification center
                        let is_notification_click = element.closest(".notification-center").is_ok();
                        if !is_notification_click {
                            dropdown_open.set(false);
                        }
                    } else if let Some(node) = target.dyn_ref::<web_sys::Node>() {
                        // If not an element, try to get parent element
                        if let Some(parent) = node.parent_element() {
                            let is_notification_click =
                                parent.closest(".notification-center").is_ok();
                            if !is_notification_click {
                                dropdown_open.set(false);
                            }
                        } else {
                            // No parent element, assume outside
                            dropdown_open.set(false);
                        }
                    }
                }) as Box<dyn FnMut(_)>);

                // Use the original document for adding the listener
                document
                    .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                    .unwrap();

                // Return cleanup function
                Box::new(move || {
                    // Use the cloned document for cleanup
                    let _ = document_clone.remove_event_listener_with_callback(
                        "click",
                        closure.as_ref().unchecked_ref(),
                    );
                    closure.forget(); // Prevent the closure from being dropped
                }) as Box<dyn FnOnce()>
            } else {
                Box::new(|| ()) as Box<dyn FnOnce()>
            }
        });
    }

    // Render the notification bell and dropdown
    html! {
        <div class="notification-center relative">
            <button
                type="button"
                class="notification-bell flex items-center justify-center relative p-2 rounded-full hover:bg-opacity-20"
                onclick={toggle_dropdown}
            >
                <i class="ph ph-bell text-2xl"></i>
                {
                    if *notification_count > 0 {
                        html! {
                            <span class="notification-badge absolute top-0 right-0 inline-flex items-center justify-center px-2 py-1 text-xs font-bold leading-none transform translate-x-1/2 -translate-y-1/2 rounded-full">
                                {*notification_count}
                            </span>
                        }
                    } else {
                        html! {}
                    }
                }
            </button>

            {
                if *dropdown_open {
                    html! {
                        <div class="notification-dropdown absolute right-0 mt-2 w-80 max-h-96 overflow-y-auto z-50" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                            <div class="notification-header flex justify-between items-center p-3 border-b border-color">
                                <h3 class="text-lg font-semibold">{"Notifications"}</h3>
                                <div class="flex space-x-2">
                                    <button
                                        class="text-sm px-2 py-1 rounded hover:bg-opacity-20"
                                        onclick={toggle_show_completed}
                                        title={if *show_completed { "Hide completed" } else { "Show completed" }}
                                    >
                                        <i class={if *show_completed { "ph ph-eye-slash" } else { "ph ph-eye" }}></i>
                                    </button>
                                    <button
                                        class="text-sm px-2 py-1 rounded hover:bg-opacity-20"
                                        onclick={clear_all}
                                        title="Clear all notifications"
                                    >
                                        <i class="ph ph-trash"></i>
                                    </button>
                                </div>
                            </div>

                            <div class="item_container-text notification-body p-2">
                                {
                                    // Render tasks
                                    if !filtered_tasks.is_empty() {
                                        html! {
                                            <div class="mb-2">
                                                <div class="flex justify-between items-center">
                                                    <h4 class="text-sm font-medium px-2 py-1">{if *show_completed { "All Tasks" } else { "Active Tasks" }}</h4>
                                                    {
                                                        if filtered_tasks.iter().any(|t| t.status == "SUCCESS" || t.status == "FAILED") {
                                                            html! {
                                                                <button
                                                                    class="text-xs px-2 py-1 rounded hover:bg-opacity-20"
                                                                    onclick={dismiss_completed}
                                                                    title="Dismiss all completed tasks"
                                                                >
                                                                    {"Dismiss Completed"}
                                                                </button>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>
                                                {
                                                    filtered_tasks.iter().map(|task| {
                                                        // Task item HTML - similar to the original but with dismiss button
                                                        let task_id = task.task_id.clone();
                                                        let task_dismiss = dismiss_task.clone();
                                                        let on_dismiss = Callback::from(move |_| task_dismiss.emit(task_id.clone()));

                                                        // Determine status styling
                                                        let status_str = task.status.as_str();
                                                        let (status_class, status_text) = match status_str {
                                                            "PENDING" => ("status-pending", "Queued"),
                                                            "STARTED" => ("status-started", "In Progress"),
                                                            "PROGRESS" => ("status-started", "In Progress"),
                                                            "DOWNLOADING" => ("status-started", "Downloading"),
                                                            "PROCESSING" => ("status-started", "Processing"),
                                                            "FINALIZING" => ("status-started", "Finalizing"),
                                                            "SUCCESS" => ("status-success", "Completed"),
                                                            "FAILED" => ("status-failed", "Failed"),
                                                            _ => ("status-started", status_str),
                                                        };

                                                        // Get task type display name
                                                        let task_type_display = match task.r#type.as_str() {
                                                            "podcast_download" => "Download",
                                                            "feed_refresh" => "Feed Refresh",
                                                            "playlist_generation" => "Playlist",
                                                            "youtube_download" => "YouTube Download",
                                                            _ => &task.r#type
                                                        };

                                                        // Get status detail text if available
                                                        let status_detail = task.details.as_ref()
                                                            .and_then(|details| details.get("status_text"))
                                                            .map(|s| s.as_str())
                                                            .unwrap_or("");

                                                        // Construct episode title or fall back to generic description
                                                        let item_description = task.details.as_ref()
                                                            .and_then(|details| {
                                                                // Try different possible key names for the title
                                                                details.get("episode_title")
                                                                    .or_else(|| details.get("item_title"))      // For YouTube videos
                                                            })
                                                            .map(|s| s.as_str())
                                                            .unwrap_or(match task.r#type.as_str() {
                                                                "podcast_download" => "Episode",
                                                                "youtube_download" => "YouTube Video",
                                                                _ => "Item"
                                                            });

                                                        // Calculate if we should show progress (any active download/processing status)
                                                        let show_progress = matches!(status_str,
                                                            "STARTED" | "PROGRESS" | "DOWNLOADING" | "PROCESSING" | "FINALIZING");

                                                        html! {
                                                            <div class="notification-item p-3 mb-2 rounded">
                                                                <div class="flex justify-between items-center mb-1">
                                                                    <div class="flex items-center">
                                                                        <span class="font-medium">{task_type_display}</span>
                                                                        <span class={format!("notification-status ml-2 px-2 py-1 rounded-full text-xs {}", status_class)}>
                                                                            {status_text}
                                                                        </span>
                                                                    </div>
                                                                    <button
                                                                        class="dismiss-button text-xs hover:opacity-70"
                                                                        onclick={on_dismiss}
                                                                        title="Dismiss notification"
                                                                    >
                                                                        <i class="ph ph-x"></i>
                                                                    </button>
                                                                </div>
                                                                {
                                                                    if !status_detail.is_empty() {
                                                                        html! { <p class="text-xs mb-2">{status_detail}</p> }
                                                                    } else if task.item_id.is_some() {
                                                                        html! { <p class="text-xs mb-2">{item_description}</p> }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }
                                                                {
                                                                    if show_progress {
                                                                        html! {
                                                                            <div class="flex items-center">
                                                                                <div class="notification-progress-bar-container flex-grow h-2 rounded overflow-hidden">
                                                                                    <div
                                                                                        class="progress-bar-fill h-full"
                                                                                        style={format!("width: {}%", task.progress)}
                                                                                    ></div>
                                                                                </div>
                                                                                <span class="progress-text ml-2 text-xs">{format!("{:.0}%", task.progress)}</span>
                                                                            </div>
                                                                        }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }
                                                            </div>
                                                        }
                                                    }).collect::<Html>()
                                                }
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }

                                {
                                    // Render error messages with dismiss button
                                    if let Some(error) = &error_message {
                                        let dispatch_clone = dispatch.clone();
                                        let dismiss_error = Callback::from(move |_| {
                                            dispatch_clone.reduce_mut(|state| {
                                                state.error_message = None;
                                            });
                                        });

                                        html! {
                                            <div class="notification-item notification-error p-3 mb-2 rounded">
                                                <div class="flex justify-between items-start">
                                                    <div class="flex items-start">
                                                        <i class="ph ph-warning-circle text-xl mr-2"></i>
                                                        <p class="text-sm">{error}</p>
                                                    </div>
                                                    <button
                                                        class="dismiss-button text-xs hover:opacity-70 ml-2"
                                                        onclick={dismiss_error}
                                                        title="Dismiss error"
                                                    >
                                                        <i class="ph ph-x"></i>
                                                    </button>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }

                                {
                                    // Render info messages with dismiss button
                                    if let Some(info) = &info_message {
                                        let dispatch_clone = dispatch.clone();
                                        let dismiss_info = Callback::from(move |_| {
                                            dispatch_clone.reduce_mut(|state| {
                                                state.info_message = None;
                                            });
                                        });

                                        html! {
                                            <div class="notification-item notification-info p-3 mb-2 rounded">
                                                <div class="flex justify-between items-start">
                                                    <div class="flex items-start">
                                                        <i class="ph ph-info text-xl mr-2"></i>
                                                        <p class="text-sm">{info}</p>
                                                    </div>
                                                    <button
                                                        class="dismiss-button text-xs hover:opacity-70 ml-2"
                                                        onclick={dismiss_info}
                                                        title="Dismiss message"
                                                    >
                                                        <i class="ph ph-x"></i>
                                                    </button>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }

                                {
                                    // If no notifications at all
                                    if filtered_tasks.is_empty() && error_message.is_none() && info_message.is_none() {
                                        html! {
                                            <div class="p-3 text-center notification-empty">
                                                <p class="text-sm">{"No notifications"}</p>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>
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
    let (state, dispatch) = use_store::<AppState>();
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

                    // First check for toasts that need to be hidden
                    for toast in new_queue.iter_mut() {
                        if toast.visible && now >= toast.expiry_time {
                            toast.visible = false;
                            changed = true;
                            log(&format!(
                                "Auto-hiding toast #{}: '{}'",
                                toast.id, toast.content
                            ));
                        }
                    }

                    // Then remove toasts that have been hidden for at least 500ms (animation time)
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
                // Check if this exact message is already in the queue
                let existing_message = (*toast_queue).iter().any(|toast: &ToastItem| {
                    toast.content == *error_msg && toast.toast_type == "error" && toast.visible
                });

                if !existing_message {
                    log(&format!("Adding new error toast: {}", error_msg));
                    let new_id = *counter;
                    counter.set(new_id + 1);

                    // Set expiry 5 seconds from now
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

                    // Clear the error message after a delay
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
                // Check if this exact message is already in the queue
                let existing_message = (*toast_queue).iter().any(|toast: &ToastItem| {
                    toast.content == *info_msg && toast.toast_type == "info" && toast.visible
                });

                if !existing_message {
                    log(&format!("Adding new info toast: {}", info_msg));
                    let new_id = *counter;
                    counter.set(new_id + 1);

                    // Set expiry 5 seconds from now
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

                    // Clear the info message after a delay
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
                                        <i class={classes!(icon_class, "text-xl", "mr-2")}></i>
                                        <p class="toast-message">
                                            {toast.content.clone()}
                                        </p>
                                    </div>
                                    // Add manual close button
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
