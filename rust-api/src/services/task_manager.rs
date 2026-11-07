use crate::{error::AppResult, redis_client::RedisClient};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub enum TaskStatus {
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "DOWNLOADING")]
    Running,
    #[serde(rename = "SUCCESS")]
    Completed,
    #[serde(rename = "FAILED")]
    Failed,
}

/// Durable in-app notification severity. Mirrored on the frontend
/// (`web/src/components/context.rs`). `warning`/`error` count toward the bell
/// badge; `info`/`success` do not.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MsgSeverity {
    Info,
    Success,
    Warning,
    Error,
}

impl Default for MsgSeverity {
    fn default() -> Self {
        MsgSeverity::Info
    }
}

/// A durable, no-progress notification ("message"/"alert") shown in the Activity
/// center's Messages / "Needs attention" sections. Distinct from a `TaskInfo`
/// (which has progress and a lifecycle). Persisted per-user in Redis and streamed
/// over the same task WebSocket. See the notification redesign plan.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct InAppNotification {
    pub id: String,
    pub user_id: i32,
    /// Free-form category tag (e.g. "feed_refresh", "ai", "new_episode").
    pub category: String,
    pub severity: MsgSeverity,
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub episode_id: Option<i32>,
    #[serde(default)]
    pub podcast_id: Option<i32>,
    #[serde(default)]
    pub art_url: Option<String>,
    /// Optional in-app link the message can deep-link to.
    #[serde(default)]
    pub link: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub read: bool,
    /// Server-authoritative expiry. List reads drop anything past this instant.
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct TaskInfo {
    pub id: String,
    pub task_type: String,
    pub user_id: i32,
    pub status: TaskStatus,
    pub progress: f64,
    pub message: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub episode_title: Option<String>,
    #[serde(default)]
    pub podcast_name: Option<String>,
    // ---- Direction-2 activity-center enrichment ----
    /// When the task reached SUCCESS/FAILED — drives "Done" ordering + relative time.
    #[serde(default)]
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Cover art for the card thumbnail.
    #[serde(default)]
    pub art_url: Option<String>,
    /// Sub-line detail (e.g. "72 of 128 feeds"; on failure, the reason).
    #[serde(default)]
    pub detail: Option<String>,
    /// Shared key folding sibling tasks into one collapsible group.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub group_label: Option<String>,
    /// Total item count a grouped run is working through (for "x of N").
    #[serde(default)]
    pub total: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskUpdate {
    pub task_id: String,
    pub user_id: i32,
    #[serde(rename = "type")]
    pub task_type: String,
    #[serde(default)]
    pub item_id: Option<i32>,
    pub progress: f64,
    pub status: TaskStatus,
    pub details: serde_json::Value,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    // ---- Direction-2 activity-center enrichment (top-level for the frontend) ----
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub art_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<i32>,
}

// WebSocket message format. `task`/`tasks` carry progress items; `notification`/
// `notifications` carry durable messages. `initial`/`refresh` populate the list
// forms; `update`/`notification` events carry a single item.
#[derive(Debug, Clone, Serialize)]
pub struct WebSocketMessage {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<TaskUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<TaskInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<InAppNotification>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<Vec<InAppNotification>>,
}

impl WebSocketMessage {
    pub fn tasks(event: &str, tasks: Vec<TaskInfo>, notifications: Vec<InAppNotification>) -> Self {
        Self {
            event: event.to_string(),
            task: None,
            tasks: Some(tasks),
            notification: None,
            notifications: Some(notifications),
        }
    }

    pub fn update(task: TaskUpdate) -> Self {
        Self {
            event: "update".to_string(),
            task: Some(task),
            tasks: None,
            notification: None,
            notifications: None,
        }
    }

    pub fn notification(notification: InAppNotification) -> Self {
        Self {
            event: "notification".to_string(),
            task: None,
            tasks: None,
            notification: Some(notification),
            notifications: None,
        }
    }
}

/// Cross-replica bridge packet. Every task update / notification is published to
/// `TASK_EVENTS_CHANNEL` stamped with the origin `instance_id`; each replica
/// re-injects packets from *other* instances into its local broadcast so sockets
/// it holds receive events that originated elsewhere. Mirrors the now-playing
/// bridge pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgePacket {
    pub instance_id: String,
    #[serde(flatten)]
    pub event: BridgeEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "bridge_kind", rename_all = "snake_case")]
pub enum BridgeEvent {
    Task(TaskUpdate),
    Notification(InAppNotification),
}

pub const TASK_EVENTS_CHANNEL: &str = "pinepods:events";

/// Durable message TTL backstops (a user can always clear sooner). Non-critical
/// info/success fade on their own; errors/warnings linger until acknowledged.
const NOTIF_TTL_TRANSIENT_SECS: i64 = 24 * 60 * 60; // info / success
const NOTIF_TTL_STICKY_SECS: i64 = 30 * 24 * 60 * 60; // warning / error

pub type TaskProgressSender = broadcast::Sender<TaskUpdate>;
pub type TaskProgressReceiver = broadcast::Receiver<TaskUpdate>;

#[derive(Clone)]
pub struct TaskManager {
    redis: RedisClient,
    /// Where in-app notifications are kept.
    db: crate::database::DatabasePool,
    progress_sender: TaskProgressSender,
    notification_sender: broadcast::Sender<InAppNotification>,
    /// This process's id, stamped on bridge packets so we ignore our own.
    instance_id: Arc<str>,
}

impl TaskManager {
    pub fn new(redis: RedisClient, db: crate::database::DatabasePool, instance_id: Arc<str>) -> Self {
        let (progress_sender, _) = broadcast::channel(1000);
        let (notification_sender, _) = broadcast::channel(1000);

        Self {
            redis,
            db,
            progress_sender,
            notification_sender,
            instance_id,
        }
    }

    pub fn subscribe_to_progress(&self) -> TaskProgressReceiver {
        self.progress_sender.subscribe()
    }

    pub fn subscribe_to_notifications(&self) -> broadcast::Receiver<InAppNotification> {
        self.notification_sender.subscribe()
    }

    /// Send a task update to local sockets AND fan it out to other replicas.
    /// All internal call sites go through this so bridging is automatic.
    async fn emit_update(&self, update: TaskUpdate) {
        let _ = self.progress_sender.send(update.clone());
        self.publish_bridge(BridgeEvent::Task(update)).await;
    }

    /// Re-inject a task update received from another replica (no re-publish).
    pub fn reinject_update(&self, update: TaskUpdate) {
        let _ = self.progress_sender.send(update);
    }

    /// Re-inject a notification received from another replica (no re-publish).
    pub fn reinject_notification(&self, notification: InAppNotification) {
        let _ = self.notification_sender.send(notification);
    }

    async fn publish_bridge(&self, event: BridgeEvent) {
        let packet = BridgePacket {
            instance_id: self.instance_id.to_string(),
            event,
        };
        if let Ok(payload) = serde_json::to_string(&packet) {
            let _ = self.redis.publish(TASK_EVENTS_CHANNEL, &payload).await;
        }
    }

    pub async fn create_task(
        &self,
        task_type: String,
        user_id: i32,
    ) -> AppResult<String> {
        self.create_task_with_item_id(task_type, user_id, None).await
    }

    pub async fn create_task_with_item_id(
        &self,
        task_type: String,
        user_id: i32,
        item_id: Option<i32>,
    ) -> AppResult<String> {
        let task_id = Uuid::new_v4().to_string();
        let task = TaskInfo {
            id: task_id.clone(),
            task_type: task_type.clone(),
            user_id,
            status: TaskStatus::Pending,
            progress: 0.0,
            message: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            result: None,
            episode_title: None,
            podcast_name: None,
            completed_at: None,
            art_url: None,
            detail: None,
            group: None,
            group_label: None,
            total: None,
        };

        self.save_task(&task).await?;

        // Send initial task update with item_id for frontend compatibility
        let update = self.build_update(&task, item_id, serde_json::json!({}));
        self.emit_update(update).await;

        Ok(task_id)
    }

    pub async fn create_download_task(
        &self,
        task_type: String,
        user_id: i32,
        item_id: Option<i32>,
        episode_title: Option<String>,
        podcast_name: Option<String>,
    ) -> AppResult<String> {
        let task_id = Uuid::new_v4().to_string();
        let task = TaskInfo {
            id: task_id.clone(),
            task_type: task_type.clone(),
            user_id,
            status: TaskStatus::Pending,
            progress: 0.0,
            message: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            result: None,
            episode_title,
            podcast_name,
            completed_at: None,
            art_url: None,
            detail: None,
            group: None,
            group_label: None,
            total: None,
        };

        self.save_task(&task).await?;

        let update = self.build_update(&task, item_id, serde_json::json!({}));
        self.emit_update(update).await;

        Ok(task_id)
    }

    /// Build a wire `TaskUpdate` from the stored `TaskInfo`, carrying the
    /// activity-center enrichment fields so the frontend always has art/group/
    /// detail without a separate fetch. `details` holds the transient JSON blob.
    fn build_update(
        &self,
        task: &TaskInfo,
        item_id: Option<i32>,
        details: serde_json::Value,
    ) -> TaskUpdate {
        TaskUpdate {
            task_id: task.id.clone(),
            user_id: task.user_id,
            task_type: task.task_type.clone(),
            item_id,
            progress: task.progress,
            status: task.status.clone(),
            details,
            started_at: task.created_at.to_rfc3339(),
            completed_at: task.completed_at.map(|t| t.to_rfc3339()),
            art_url: task.art_url.clone(),
            detail: task.detail.clone(),
            group: task.group.clone(),
            group_label: task.group_label.clone(),
            total: task.total,
        }
    }

    /// Set the activity-center enrichment on a task (art, sub-line, grouping).
    /// Called by spawners so cards render with cover art and bulk runs fold into
    /// one group. Emits a refreshed update.
    pub async fn set_task_display(
        &self,
        task_id: &str,
        art_url: Option<String>,
        detail: Option<String>,
        group: Option<String>,
        group_label: Option<String>,
        total: Option<i32>,
    ) -> AppResult<()> {
        let mut task = self.get_task(task_id).await?;
        if art_url.is_some() {
            task.art_url = art_url;
        }
        if detail.is_some() {
            task.detail = detail;
        }
        if group.is_some() {
            task.group = group;
        }
        if group_label.is_some() {
            task.group_label = group_label;
        }
        if total.is_some() {
            task.total = total;
        }
        task.updated_at = chrono::Utc::now();
        self.save_task(&task).await?;
        let update = self.build_update(&task, None, serde_json::json!({}));
        self.emit_update(update).await;
        Ok(())
    }

    pub async fn set_task_metadata(
        &self,
        task_id: &str,
        episode_title: Option<String>,
        podcast_name: Option<String>,
    ) -> AppResult<()> {
        let mut task = self.get_task(task_id).await?;
        task.episode_title = episode_title;
        task.podcast_name = podcast_name;
        task.updated_at = chrono::Utc::now();
        self.save_task(&task).await
    }

    pub async fn update_task_progress(
        &self,
        task_id: &str,
        progress: f64,
        message: Option<String>,
    ) -> AppResult<()> {
        self.update_task_progress_with_item_id(task_id, progress, message, None, None).await
    }

    pub async fn update_task_progress_with_item_id(
        &self,
        task_id: &str,
        progress: f64,
        message: Option<String>,
        item_id: Option<i32>,
        task_type: Option<String>,
    ) -> AppResult<()> {
        self.update_task_progress_with_details(task_id, progress, message, item_id, task_type, None).await
    }

    pub async fn update_task_progress_with_details(
        &self,
        task_id: &str,
        progress: f64,
        message: Option<String>,
        item_id: Option<i32>,
        task_type: Option<String>,
        episode_title: Option<String>,
    ) -> AppResult<()> {
        let mut task = self.get_task(task_id).await?;
        task.progress = progress.clamp(0.0, 100.0);
        task.message = message.clone();
        task.updated_at = chrono::Utc::now();

        if progress > 0.0 && matches!(task.status, TaskStatus::Pending) {
            task.status = TaskStatus::Running;
        }

        self.save_task(&task).await?;

        let mut details = serde_json::json!({
            "status_text": message.as_deref().unwrap_or("Processing...")
        });

        // Add episode details if provided
        if let Some(episode_id) = item_id {
            details["episode_id"] = serde_json::json!(episode_id);
        }
        if let Some(title) = episode_title {
            details["episode_title"] = serde_json::json!(title);
        }

        let mut update = self.build_update(&task, item_id, details);
        if let Some(tt) = task_type {
            update.task_type = tt;
        }
        self.emit_update(update).await;
        Ok(())
    }

    pub async fn complete_task(
        &self,
        task_id: &str,
        result: Option<serde_json::Value>,
        message: Option<String>,
    ) -> AppResult<()> {
        let mut task = self.get_task(task_id).await?;
        task.status = TaskStatus::Completed;
        task.progress = 100.0;
        task.message = message.clone();
        task.result = result.clone();
        task.completed_at = Some(chrono::Utc::now());
        task.updated_at = chrono::Utc::now();

        self.save_task(&task).await?;

        let details = serde_json::json!({
            "status_text": message.as_deref().unwrap_or("Completed"),
            "result": result
        });
        let update = self.build_update(&task, None, details);
        self.emit_update(update).await;
        Ok(())
    }

    pub async fn fail_task(
        &self,
        task_id: &str,
        error_message: String,
    ) -> AppResult<()> {
        let mut task = self.get_task(task_id).await?;
        task.status = TaskStatus::Failed;
        task.message = Some(error_message.clone());
        // Surface the reason as the card sub-line ("Server returned 404").
        task.detail = Some(error_message.clone());
        task.completed_at = Some(chrono::Utc::now());
        task.updated_at = chrono::Utc::now();

        self.save_task(&task).await?;

        let details = serde_json::json!({
            "status_text": error_message,
            "error": error_message
        });
        let update = self.build_update(&task, None, details);
        self.emit_update(update).await;
        Ok(())
    }

    pub async fn get_task(&self, task_id: &str) -> AppResult<TaskInfo> {
        let key = format!("task:{}", task_id);
        let task_json: String = self
            .redis
            .get(&key)
            .await?
            .ok_or_else(|| crate::error::AppError::not_found("Task not found"))?;
        let task: TaskInfo = serde_json::from_str(&task_json)?;
        Ok(task)
    }

    fn task_key(task_id: &str) -> String {
        format!("task:{}", task_id)
    }
    fn task_index_key(user_id: i32) -> String {
        format!("task:index:{}", user_id)
    }

    /// List a user's tasks via their per-user index SET (no global `KEYS` scan),
    /// pruning any index members whose task key has expired. Most-recent first.
    pub async fn get_user_tasks(&self, user_id: i32) -> AppResult<Vec<TaskInfo>> {
        let index = Self::task_index_key(user_id);
        let ids = self.redis.smembers(&index).await?;

        let mut user_tasks = Vec::with_capacity(ids.len());
        for id in ids {
            let key = Self::task_key(&id);
            match self.redis.get::<String>(&key).await {
                Ok(Some(json)) => match serde_json::from_str::<TaskInfo>(&json) {
                    Ok(task) => user_tasks.push(task),
                    Err(_) => self.redis.srem(&index, &id).await?,
                },
                // Task key expired: drop the stale index member.
                Ok(None) => self.redis.srem(&index, &id).await?,
                Err(_) => {}
            }
        }

        user_tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(user_tasks)
    }

    async fn save_task(&self, task: &TaskInfo) -> AppResult<()> {
        let key = Self::task_key(&task.id);
        let index = Self::task_index_key(task.user_id);
        let task_json = serde_json::to_string(task)?;
        self.redis.set_ex(&key, task_json, 86400 * 7).await?; // 7 days TTL
        self.redis.sadd(&index, &task.id).await?;
        // Keep the index alive a bit past a single task so a quiet user's set isn't
        // dropped mid-run; stale members are pruned lazily on read.
        self.redis.expire(&index, 86400 * 8).await?;
        Ok(())
    }

    /// Best-effort sweep of tasks older than 7 days. Retained mainly as a safety
    /// net; the per-key TTL already reaps them and the index self-prunes on read.
    pub async fn cleanup_old_tasks(&self) -> AppResult<()> {
        // Per-key TTLs handle expiry; nothing to scan globally anymore.
        Ok(())
    }

    // ===================== Durable notifications (messages) =====================

    fn notif_ttl_secs(severity: MsgSeverity) -> i64 {
        match severity {
            MsgSeverity::Info | MsgSeverity::Success => NOTIF_TTL_TRANSIENT_SECS,
            MsgSeverity::Warning | MsgSeverity::Error => NOTIF_TTL_STICKY_SECS,
        }
    }

    /// Create, persist, and broadcast a durable notification. This is the single
    /// entry point for server-side alerts (feed errors, AI failures, new episodes).
    #[allow(clippy::too_many_arguments)]
    pub async fn push_notification(
        &self,
        user_id: i32,
        category: impl Into<String>,
        severity: MsgSeverity,
        title: impl Into<String>,
        body: Option<String>,
        episode_id: Option<i32>,
        podcast_id: Option<i32>,
        art_url: Option<String>,
        link: Option<String>,
    ) -> AppResult<InAppNotification> {
        let ttl = Self::notif_ttl_secs(severity);
        let now = chrono::Utc::now();
        let notif = InAppNotification {
            id: Uuid::new_v4().to_string(),
            user_id,
            category: category.into(),
            severity,
            title: title.into(),
            body,
            episode_id,
            podcast_id,
            art_url,
            link,
            created_at: now,
            read: false,
            expires_at: now + chrono::Duration::seconds(ttl),
        };

        self.save_notification(&notif).await?;
        let _ = self.notification_sender.send(notif.clone());
        self.publish_bridge(BridgeEvent::Notification(notif.clone())).await;
        Ok(notif)
    }

    async fn save_notification(&self, notif: &InAppNotification) -> AppResult<()> {
        let payload = serde_json::to_string(notif)?;
        self.db
            .save_in_app_notification(&crate::database::StoredInAppNotification {
                id: &notif.id,
                user_id: notif.user_id,
                payload: &payload,
                read: notif.read,
                created_at: notif.created_at.naive_utc(),
                expires_at: notif.expires_at.naive_utc(),
            })
            .await
    }

    /// List a user's live notifications, most recent first.
    pub async fn list_notifications(&self, user_id: i32) -> AppResult<Vec<InAppNotification>> {
        let mut out: Vec<InAppNotification> = self
            .db
            .list_in_app_notifications(user_id)
            .await?
            .iter()
            .filter_map(|json| serde_json::from_str(json).ok())
            .collect();
        out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(out)
    }

    pub async fn mark_notification_read(&self, user_id: i32, id: &str) -> AppResult<()> {
        if let Some(json) = self.db.get_in_app_notification(user_id, id).await? {
            if let Ok(mut n) = serde_json::from_str::<InAppNotification>(&json) {
                n.read = true;
                self.save_notification(&n).await?;
            }
        }
        Ok(())
    }

    pub async fn dismiss_notification(&self, user_id: i32, id: &str) -> AppResult<()> {
        self.db.delete_in_app_notification(user_id, id).await
    }

    /// Clear notifications for a user. `read_only` clears just already-read ones;
    /// otherwise all of them.
    pub async fn clear_notifications(&self, user_id: i32, read_only: bool) -> AppResult<()> {
        self.db.clear_in_app_notifications(user_id, read_only).await
    }
}