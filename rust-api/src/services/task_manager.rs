use crate::{error::AppResult, redis_client::RedisClient};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskUpdate {
    pub task_id: String,
    pub user_id: i32,
    pub status: TaskStatus,
    pub progress: f64,
    pub message: Option<String>,
    pub result: Option<serde_json::Value>,
}

pub type TaskProgressSender = broadcast::Sender<TaskUpdate>;
pub type TaskProgressReceiver = broadcast::Receiver<TaskUpdate>;

#[derive(Clone)]
pub struct TaskManager {
    redis: RedisClient,
    progress_sender: TaskProgressSender,
}

impl TaskManager {
    pub fn new(redis: RedisClient) -> Self {
        let (progress_sender, _) = broadcast::channel(1000);
        
        Self {
            redis,
            progress_sender,
        }
    }

    pub fn subscribe_to_progress(&self) -> TaskProgressReceiver {
        self.progress_sender.subscribe()
    }

    pub async fn create_task(
        &self,
        task_type: String,
        user_id: i32,
    ) -> AppResult<String> {
        let task_id = Uuid::new_v4().to_string();
        let task = TaskInfo {
            id: task_id.clone(),
            task_type,
            user_id,
            status: TaskStatus::Pending,
            progress: 0.0,
            message: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            result: None,
        };

        self.save_task(&task).await?;
        Ok(task_id)
    }

    pub async fn update_task_progress(
        &self,
        task_id: &str,
        progress: f64,
        message: Option<String>,
    ) -> AppResult<()> {
        let mut task = self.get_task(task_id).await?;
        task.progress = progress.clamp(0.0, 100.0);
        task.message = message.clone();
        task.updated_at = chrono::Utc::now();

        if progress > 0.0 && matches!(task.status, TaskStatus::Pending) {
            task.status = TaskStatus::Running;
        }

        self.save_task(&task).await?;

        let update = TaskUpdate {
            task_id: task_id.to_string(),
            user_id: task.user_id,
            status: task.status.clone(),
            progress,
            message,
            result: None,
        };

        let _ = self.progress_sender.send(update);
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
        task.updated_at = chrono::Utc::now();

        self.save_task(&task).await?;

        let update = TaskUpdate {
            task_id: task_id.to_string(),
            user_id: task.user_id,
            status: TaskStatus::Completed,
            progress: 100.0,
            message,
            result,
        };

        let _ = self.progress_sender.send(update);
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
        task.updated_at = chrono::Utc::now();

        self.save_task(&task).await?;

        let update = TaskUpdate {
            task_id: task_id.to_string(),
            user_id: task.user_id,
            status: TaskStatus::Failed,
            progress: task.progress,
            message: Some(error_message),
            result: None,
        };

        let _ = self.progress_sender.send(update);
        Ok(())
    }

    pub async fn get_task(&self, task_id: &str) -> AppResult<TaskInfo> {
        let key = format!("task:{}", task_id);
        let mut conn = self.redis.get_connection().await?;
        let task_json: String = conn.get(&key).await?;
        let task: TaskInfo = serde_json::from_str(&task_json)?;
        Ok(task)
    }

    pub async fn get_user_tasks(&self, user_id: i32) -> AppResult<Vec<TaskInfo>> {
        let pattern = format!("task:*");
        let mut conn = self.redis.get_connection().await?;
        let keys: Vec<String> = conn.keys(&pattern).await?;
        
        let mut user_tasks = Vec::new();
        for key in keys {
            if let Ok(task_json) = conn.get::<_, String>(&key).await {
                if let Ok(task) = serde_json::from_str::<TaskInfo>(&task_json) {
                    if task.user_id == user_id {
                        user_tasks.push(task);
                    }
                }
            }
        }

        user_tasks.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(user_tasks)
    }

    async fn save_task(&self, task: &TaskInfo) -> AppResult<()> {
        let key = format!("task:{}", task.id);
        let task_json = serde_json::to_string(task)?;
        let mut conn = self.redis.get_connection().await?;
        
        conn.set_ex::<_, _, ()>(&key, &task_json, 86400 * 7).await?; // 7 days TTL
        Ok(())
    }

    pub async fn cleanup_old_tasks(&self) -> AppResult<()> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(7);
        let pattern = "task:*";
        let mut conn = self.redis.get_connection().await?;
        let keys: Vec<String> = conn.keys(&pattern).await?;
        
        for key in keys {
            if let Ok(task_json) = conn.get::<_, String>(&key).await {
                if let Ok(task) = serde_json::from_str::<TaskInfo>(&task_json) {
                    if task.created_at < cutoff {
                        let _: () = conn.del(&key).await?;
                    }
                }
            }
        }
        
        Ok(())
    }
}