use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::Response,
};
use futures::{sink::SinkExt, stream::StreamExt};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, RwLock};
use crate::{
    error::AppResult,
    services::task_manager::{TaskManager, TaskUpdate},
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

pub async fn task_progress_websocket(
    ws: WebSocketUpgrade,
    Path(user_id): Path<i32>,
    State(state): State<AppState>,
) -> Response {
    ws.on_upgrade(move |socket| handle_task_progress_socket(socket, user_id, state))
}

async fn handle_task_progress_socket(socket: WebSocket, user_id: i32, state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = broadcast::channel::<TaskUpdate>(100);

    // Add connection to manager
    state.websocket_manager.add_connection(user_id, tx.clone()).await;

    // Subscribe to task manager updates
    let mut task_receiver = state.task_manager.subscribe_to_progress();

    // Spawn task to forward task manager updates to user
    let tx_clone = tx.clone();
    let forward_task = tokio::spawn(async move {
        while let Ok(update) = task_receiver.recv().await {
            if update.user_id == user_id {
                let _ = tx_clone.send(update);
            }
        }
    });

    // Spawn task to send WebSocket messages
    let websocket_task = tokio::spawn(async move {
        while let Ok(update) = rx.recv().await {
            let message = match serde_json::to_string(&update) {
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

pub async fn get_user_tasks(
    Path(user_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<axum::Json<Vec<crate::services::task_manager::TaskInfo>>, crate::error::AppError> {
    let tasks = state.task_manager.get_user_tasks(user_id).await?;
    Ok(axum::Json(tasks))
}

pub async fn get_task_status(
    Path(task_id): Path<String>,
    State(state): State<AppState>,
) -> Result<axum::Json<crate::services::task_manager::TaskInfo>, crate::error::AppError> {
    let task = state.task_manager.get_task(&task_id).await?;
    Ok(axum::Json(task))
}

pub async fn get_active_tasks(
    Query(params): Query<std::collections::HashMap<String, String>>,
    State(state): State<AppState>,
) -> Result<axum::Json<Vec<crate::services::task_manager::TaskInfo>>, crate::error::AppError> {
    // Get user_id from query parameter
    let user_id: Option<i32> = params.get("user_id")
        .and_then(|id| id.parse().ok());
    
    if let Some(user_id) = user_id {
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