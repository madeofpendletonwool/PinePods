use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::HeaderMap,
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use crate::{
    handlers::{check_user_access, extract_api_key, validate_api_key},
    services::task_manager::{BridgeEvent, BridgePacket, TaskUpdate, WebSocketMessage, TASK_EVENTS_CHANNEL},
    AppState,
};

type UserConnections = Arc<RwLock<HashMap<i32, Vec<broadcast::Sender<TaskUpdate>>>>>;

pub struct WebSocketManager {
    connections: UserConnections,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_connection(&self, user_id: i32, sender: broadcast::Sender<TaskUpdate>) {
        let mut connections = self.connections.write().await;
        connections.entry(user_id).or_insert_with(Vec::new).push(sender);
    }

    pub async fn remove_connection(&self, user_id: i32, sender: &broadcast::Sender<TaskUpdate>) {
        let mut connections = self.connections.write().await;
        if let Some(user_connections) = connections.get_mut(&user_id) {
            user_connections.retain(|s| !s.same_channel(sender));
            if user_connections.is_empty() {
                connections.remove(&user_id);
            }
        }
    }

    pub async fn broadcast_to_user(&self, user_id: i32, update: TaskUpdate) {
        let connections = self.connections.read().await;
        if let Some(user_connections) = connections.get(&user_id) {
            for sender in user_connections {
                let _ = sender.send(update.clone());
            }
        }
    }
}

use serde::Deserialize;

#[derive(Deserialize)]
pub struct WebSocketQuery {
    api_key: String,
}

pub async fn task_progress_websocket(
    ws: WebSocketUpgrade,
    Path(user_id): Path<i32>,
    Query(query): Query<WebSocketQuery>,
    State(state): State<AppState>,
) -> Response {
    // Validate API key before upgrading websocket
    match state.db_pool.verify_api_key(&query.api_key).await {
        Ok(true) => {
            // Verify the API key belongs to this user (or system user for background tasks)
            match state.db_pool.get_user_id_from_api_key(&query.api_key).await {
                Ok(key_user_id) => {
                    // Allow access if API key matches the user or if it's the system user (ID 1)
                    if key_user_id == user_id || key_user_id == 1 {
                        ws.on_upgrade(move |socket| handle_task_progress_socket(socket, user_id, state))
                    } else {
                        tracing::warn!("WebSocket auth failed: API key user {} tried to access user {} tasks", key_user_id, user_id);
                        axum::response::Response::builder()
                            .status(403)
                            .body("Unauthorized - API key does not belong to requested user".into())
                            .unwrap()
                    }
                }
                Err(e) => {
                    tracing::error!("WebSocket auth error getting user ID from API key: {}", e);
                    axum::response::Response::builder()
                        .status(403)
                        .body("Invalid API key".into())
                        .unwrap()
                }
            }
        }
        Ok(false) | Err(_) => {
            tracing::warn!("WebSocket auth failed: Invalid API key");
            axum::response::Response::builder()
                .status(403)
                .body("Invalid API key".into())
                .unwrap()
        }
    }
}

async fn handle_task_progress_socket(socket: WebSocket, user_id: i32, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = broadcast::channel::<TaskUpdate>(100);

    // Add connection to manager
    state.websocket_manager.add_connection(user_id, tx.clone()).await;

    // Subscribe to task-progress + durable-notification streams.
    let mut task_receiver = state.task_manager.subscribe_to_progress();
    let mut notif_receiver = state.task_manager.subscribe_to_notifications();

    // Spawn task to forward task manager updates to user
    let tx_clone = tx.clone();
    let forward_task = tokio::spawn(async move {
        while let Ok(update) = task_receiver.recv().await {
            if update.user_id == user_id {
                let _ = tx_clone.send(update);
            }
        }
    });

    // Send initial task list + durable notifications to the newly connected client.
    let initial_tasks = state.task_manager.get_user_tasks(user_id).await.unwrap_or_default();
    let initial_notifs = state.task_manager.list_notifications(user_id).await.unwrap_or_default();
    let initial_message = WebSocketMessage::tasks("initial", initial_tasks, initial_notifs);
    let initial_json = match serde_json::to_string(&initial_message) {
        Ok(json) => json,
        Err(_) => "{}".to_string(),
    };
    let _ = sender.send(Message::Text(initial_json.into())).await;

    // Spawn task to send WebSocket messages: task updates (per-connection channel)
    // and durable notifications (global stream, filtered by user) share the sink.
    let websocket_task = tokio::spawn(async move {
        loop {
            let ws_message = tokio::select! {
                r = rx.recv() => match r {
                    Ok(update) => WebSocketMessage::update(update),
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                },
                n = notif_receiver.recv() => match n {
                    Ok(notif) if notif.user_id == user_id => WebSocketMessage::notification(notif),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Closed) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                },
            };

            let message = match serde_json::to_string(&ws_message) {
                Ok(json) => Message::Text(json.into()),
                Err(_) => continue,
            };

            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    // Handle incoming WebSocket messages (if any)
    let ping_task = tokio::spawn(async move {
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    // Handle ping/pong or other control messages
                    if text == "ping" {
                        // Connection is alive, no action needed
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }
    });

    // Wait for any task to complete
    tokio::select! {
        _ = forward_task => {},
        _ = websocket_task => {},
        _ = ping_task => {},
    }

    // Clean up connection
    state.websocket_manager.remove_connection(user_id, &tx).await;
}

// ======================= Multi-replica pub/sub bridge =======================
//
// Task progress + durable notifications are held in shared Valkey, but the live
// broadcast that feeds each socket is in-process. With more than one API replica,
// an event raised on replica A must reach the sockets on replica B. Every emit
// publishes a `BridgePacket` (stamped with the origin instance id) to
// `TASK_EVENTS_CHANNEL`; here we subscribe and re-inject packets from *other*
// instances into the local broadcast. Degrades cleanly to in-process fan-out
// when pub/sub is unavailable (the default single-container deploy needs none).

/// Start the Valkey subscriber that relays task/notification events from other
/// API replicas onto this instance's local streams. Best-effort: logs and retries.
pub fn spawn_task_bridge(state: AppState) {
    // Only replicas sharing a Valkey server have anything to bridge.
    if !state.redis_client.is_shared() {
        return;
    }
    tokio::spawn(async move {
        loop {
            if let Err(e) = run_task_bridge(&state).await {
                tracing::warn!("task pub/sub bridge stopped ({e}); retrying in 5s");
            }
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });
}

async fn run_task_bridge(state: &AppState) -> crate::error::AppResult<()> {
    let mut pubsub = state.redis_client.get_pubsub().await?;
    pubsub.subscribe(TASK_EVENTS_CHANNEL).await?;
    tracing::info!("task pub/sub bridge subscribed to {TASK_EVENTS_CHANNEL}");

    let mut messages = pubsub.on_message();
    while let Some(msg) = messages.next().await {
        let payload: String = match msg.get_payload() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let packet: BridgePacket = match serde_json::from_str(&payload) {
            Ok(p) => p,
            Err(_) => continue,
        };
        // Ignore what we published ourselves — already delivered locally.
        if packet.instance_id == state.instance_id.as_ref() {
            continue;
        }
        match packet.event {
            BridgeEvent::Task(update) => state.task_manager.reinject_update(update),
            BridgeEvent::Notification(notif) => state.task_manager.reinject_notification(notif),
        }
    }
    Ok(())
}

#[utoipa::path(
    get,
    path = "/user/{user_id}",
    tag = "tasks",
    summary = "Get user tasks",
    params(("user_id" = i32, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = Vec<crate::services::task_manager::TaskInfo>),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "API key does not belong to the requested user"),
    ),
)]
pub async fn get_user_tasks(
    headers: HeaderMap,
    Path(user_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<axum::Json<Vec<crate::services::task_manager::TaskInfo>>, crate::error::AppError> {
    let api_key = extract_api_key(&headers)?;
    if !validate_api_key(&state, &api_key).await? {
        return Err(crate::error::AppError::unauthorized("Invalid API key"));
    }
    if !check_user_access(&state, &api_key, user_id).await? {
        return Err(crate::error::AppError::forbidden("You can only view your own tasks!"));
    }

    let tasks = state.task_manager.get_user_tasks(user_id).await?;
    Ok(axum::Json(tasks))
}

#[utoipa::path(
    get,
    path = "/{task_id}",
    tag = "tasks",
    summary = "Get task status",
    params(("task_id" = String, Path)),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = crate::services::task_manager::TaskInfo),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Task does not belong to the requesting user"),
    ),
)]
pub async fn get_task_status(
    headers: HeaderMap,
    Path(task_id): Path<String>,
    State(state): State<AppState>,
) -> Result<axum::Json<crate::services::task_manager::TaskInfo>, crate::error::AppError> {
    let api_key = extract_api_key(&headers)?;
    if !validate_api_key(&state, &api_key).await? {
        return Err(crate::error::AppError::unauthorized("Invalid API key"));
    }

    let task = state.task_manager.get_task(&task_id).await?;
    if !check_user_access(&state, &api_key, task.user_id).await? {
        return Err(crate::error::AppError::forbidden("You can only view your own tasks!"));
    }

    Ok(axum::Json(task))
}

#[utoipa::path(
    get,
    path = "/active",
    tag = "tasks",
    summary = "Get active tasks",
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = Vec<crate::services::task_manager::TaskInfo>),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "API key does not belong to the requested user"),
    ),
)]
pub async fn get_active_tasks(
    headers: HeaderMap,
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<axum::Json<Vec<crate::services::task_manager::TaskInfo>>, crate::error::AppError> {
    let api_key = extract_api_key(&headers)?;
    if !validate_api_key(&state, &api_key).await? {
        return Err(crate::error::AppError::unauthorized("Invalid API key"));
    }

    // Get user_id from query parameter
    let user_id: Option<i32> = params.get("user_id")
        .and_then(|id| id.parse().ok());

    if let Some(user_id) = user_id {
        if !check_user_access(&state, &api_key, user_id).await? {
            return Err(crate::error::AppError::forbidden("You can only view your own tasks!"));
        }
        // Get active tasks for specific user
        let tasks = state.task_manager.get_user_tasks(user_id).await?;
        // Filter only active tasks (status = Running or Pending)
        let active_tasks: Vec<_> = tasks.into_iter()
            .filter(|task| matches!(task.status, crate::services::task_manager::TaskStatus::Pending | crate::services::task_manager::TaskStatus::Running))
            .collect();
        Ok(axum::Json(active_tasks))
    } else {
        // Return empty if no user_id provided
        Ok(axum::Json(vec![]))
    }
}