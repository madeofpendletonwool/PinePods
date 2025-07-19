use crate::{
    error::{AppError, AppResult},
    services::task_manager::{TaskManager, TaskInfo},
};
use futures::Future;
use serde_json::Value;
use std::sync::Arc;
use tokio::task::JoinHandle;

pub struct TaskSpawner {
    task_manager: Arc<TaskManager>,
}

impl TaskSpawner {
    pub fn new(task_manager: Arc<TaskManager>) -> Self {
        Self { task_manager }
    }

    pub async fn spawn_task<F, Fut>(
        &self,
        task_type: String,
        user_id: i32,
        task_fn: F,
    ) -> AppResult<String>
    where
        F: FnOnce(String, Arc<TaskManager>) -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<Value>> + Send + 'static,
    {
        let task_id = self.task_manager.create_task(task_type, user_id).await?;
        let task_manager = self.task_manager.clone();
        let task_id_clone = task_id.clone();

        tokio::spawn(async move {
            match task_fn(task_id_clone.clone(), task_manager.clone()).await {
                Ok(result) => {
                    if let Err(e) = task_manager
                        .complete_task(&task_id_clone, Some(result), None)
                        .await
                    {
                        tracing::error!("Failed to mark task {} as completed: {}", task_id_clone, e);
                    }
                }
                Err(e) => {
                    if let Err(err) = task_manager
                        .fail_task(&task_id_clone, e.to_string())
                        .await
                    {
                        tracing::error!("Failed to mark task {} as failed: {}", task_id_clone, err);
                    }
                }
            }
        });

        Ok(task_id)
    }

    pub async fn spawn_simple_task<F, Fut>(
        &self,
        task_type: String,
        user_id: i32,
        task_fn: F,
    ) -> AppResult<String>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<Value>> + Send + 'static,
    {
        self.spawn_task(task_type, user_id, move |_task_id, _task_manager| {
            task_fn()
        })
        .await
    }

    pub async fn spawn_progress_task<F, Fut>(
        &self,
        task_type: String,
        user_id: i32,
        task_fn: F,
    ) -> AppResult<String>
    where
        F: FnOnce(Arc<dyn ProgressReporter>) -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<Value>> + Send + 'static,
    {
        self.spawn_task(task_type, user_id, move |task_id, task_manager| {
            let reporter = Arc::new(TaskProgressReporter {
                task_id,
                task_manager,
            });
            task_fn(reporter)
        })
        .await
    }
}

#[async_trait::async_trait]
pub trait ProgressReporter: Send + Sync {
    async fn update_progress(&self, progress: f64, message: Option<String>) -> AppResult<()>;
}

pub struct TaskProgressReporter {
    task_id: String,
    task_manager: Arc<TaskManager>,
}

#[async_trait::async_trait]
impl ProgressReporter for TaskProgressReporter {
    async fn update_progress(&self, progress: f64, message: Option<String>) -> AppResult<()> {
        self.task_manager
            .update_task_progress(&self.task_id, progress, message)
            .await
    }
}

// Example task implementations

pub async fn download_episode_task(
    episode_id: String,
    url: String,
    reporter: Arc<dyn ProgressReporter>,
) -> AppResult<Value> {
    reporter
        .update_progress(10.0, Some("Starting download...".to_string()))
        .await?;

    // Simulate download progress
    for i in 1..=9 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        let progress = 10.0 + (i as f64 * 10.0);
        reporter
            .update_progress(
                progress,
                Some(format!("Downloading... {}%", progress as u32)),
            )
            .await?;
    }

    reporter
        .update_progress(100.0, Some("Download completed".to_string()))
        .await?;

    Ok(serde_json::json!({
        "episode_id": episode_id,
        "url": url,
        "file_path": "/downloads/episode.mp3"
    }))
}

pub async fn import_opml_task(
    opml_content: String,
    reporter: Arc<dyn ProgressReporter>,
) -> AppResult<Value> {
    reporter
        .update_progress(5.0, Some("Parsing OPML...".to_string()))
        .await?;

    // Simulate OPML parsing and processing
    let feed_count = 10; // Would parse OPML to get actual count

    for i in 1..=feed_count {
        let progress = 5.0 + ((i as f64 / feed_count as f64) * 90.0);
        reporter
            .update_progress(
                progress,
                Some(format!("Processing feed {} of {}", i, feed_count)),
            )
            .await?;

        // Simulate processing each feed
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    reporter
        .update_progress(100.0, Some("OPML import completed".to_string()))
        .await?;

    Ok(serde_json::json!({
        "imported_feeds": feed_count,
        "success": true
    }))
}

pub async fn refresh_all_feeds_task(
    user_id: i32,
    reporter: Arc<dyn ProgressReporter>,
) -> AppResult<Value> {
    reporter
        .update_progress(5.0, Some("Fetching user podcasts...".to_string()))
        .await?;

    // Simulate fetching podcasts from database
    let podcast_count = 25; // Would fetch from DB

    for i in 1..=podcast_count {
        let progress = 5.0 + ((i as f64 / podcast_count as f64) * 90.0);
        reporter
            .update_progress(
                progress,
                Some(format!("Refreshing podcast {} of {}", i, podcast_count)),
            )
            .await?;

        // Simulate refreshing each podcast
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    reporter
        .update_progress(100.0, Some("All feeds refreshed".to_string()))
        .await?;

    Ok(serde_json::json!({
        "user_id": user_id,
        "refreshed_count": podcast_count,
        "success": true
    }))
}

impl TaskSpawner {
    // Download task spawners for podcast episodes and YouTube videos
    pub async fn spawn_download_podcast_episode(&self, episode_id: i32, user_id: i32) -> AppResult<String> {
        self.spawn_task(
            "download_episode".to_string(),
            user_id,
            move |task_id, task_manager| async move {
                // TODO: Implement actual download logic
                tracing::info!("Downloading podcast episode {} for user {}", episode_id, user_id);
                
                // Placeholder - in real implementation this would:
                // 1. Get episode metadata from database
                // 2. Download the audio file
                // 3. Save to filesystem
                // 4. Update database with download location
                // 5. Update progress via task_manager.update_progress()
                
                Ok(serde_json::json!({
                    "episode_id": episode_id,
                    "user_id": user_id,
                    "status": "downloaded"
                }))
            },
        ).await
    }

    pub async fn spawn_download_youtube_video(&self, video_id: i32, user_id: i32) -> AppResult<String> {
        self.spawn_task(
            "download_video".to_string(),
            user_id,
            move |task_id, task_manager| async move {
                // TODO: Implement actual YouTube download logic
                tracing::info!("Downloading YouTube video {} for user {}", video_id, user_id);
                
                // Placeholder - in real implementation this would:
                // 1. Get video metadata from database
                // 2. Download the video file using youtube-dl or similar
                // 3. Save to filesystem
                // 4. Update database with download location
                // 5. Update progress via task_manager.update_progress()
                
                Ok(serde_json::json!({
                    "video_id": video_id,
                    "user_id": user_id,
                    "status": "downloaded"
                }))
            },
        ).await
    }

    pub async fn spawn_download_all_podcast_episodes(&self, podcast_id: i32, user_id: i32) -> AppResult<String> {
        self.spawn_task(
            "download_all_episodes".to_string(),
            user_id,
            move |task_id, task_manager| async move {
                // TODO: Implement actual bulk download logic
                tracing::info!("Downloading all episodes for podcast {} for user {}", podcast_id, user_id);
                
                // Placeholder - in real implementation this would:
                // 1. Get all episodes for the podcast from database
                // 2. Queue individual download tasks for each episode
                // 3. Monitor progress of all downloads
                // 4. Update overall progress via task_manager.update_progress()
                
                Ok(serde_json::json!({
                    "podcast_id": podcast_id,
                    "user_id": user_id,
                    "status": "all_episodes_queued"
                }))
            },
        ).await
    }

    pub async fn spawn_download_all_youtube_videos(&self, channel_id: i32, user_id: i32) -> AppResult<String> {
        self.spawn_task(
            "download_all_videos".to_string(),
            user_id,
            move |task_id, task_manager| async move {
                // TODO: Implement actual bulk YouTube download logic
                tracing::info!("Downloading all videos for channel {} for user {}", channel_id, user_id);
                
                // Placeholder - in real implementation this would:
                // 1. Get all videos for the channel from database
                // 2. Queue individual download tasks for each video
                // 3. Monitor progress of all downloads
                // 4. Update overall progress via task_manager.update_progress()
                
                Ok(serde_json::json!({
                    "channel_id": channel_id,
                    "user_id": user_id,
                    "status": "all_videos_queued"
                }))
            },
        ).await
    }
}