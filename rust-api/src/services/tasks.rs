use crate::{
    error::{AppError, AppResult},
    services::task_manager::{TaskManager, TaskInfo},
    database::DatabasePool,
};
use futures::Future;
use serde_json::Value;
use std::sync::Arc;
use tokio::task::JoinHandle;
use sqlx::Row;

pub struct TaskSpawner {
    task_manager: Arc<TaskManager>,
    db_pool: DatabasePool,
}

impl TaskSpawner {
    pub fn new(task_manager: Arc<TaskManager>, db_pool: DatabasePool) -> Self {
        Self { task_manager, db_pool }
    }

    pub async fn spawn_task<F, Fut>(
        &self,
        task_type: String,
        user_id: i32,
        task_fn: F,
    ) -> AppResult<String>
    where
        F: FnOnce(String, Arc<TaskManager>, DatabasePool) -> Fut + Send + 'static,
        Fut: Future<Output = AppResult<Value>> + Send + 'static,
    {
        let task_id = self.task_manager.create_task(task_type, user_id).await?;
        let task_manager = self.task_manager.clone();
        let db_pool = self.db_pool.clone();
        let task_id_clone = task_id.clone();

        tokio::spawn(async move {
            match task_fn(task_id_clone.clone(), task_manager.clone(), db_pool).await {
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
        self.spawn_task(task_type, user_id, move |_task_id, _task_manager, _db_pool| {
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
        self.spawn_task(task_type, user_id, move |task_id, task_manager, _db_pool| {
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
        let db_pool = self.db_pool.clone();
        
        // Create task with episode_id as item_id for frontend compatibility
        let task_id = self.task_manager.create_task_with_item_id(
            "download_episode".to_string(),
            user_id,
            Some(episode_id),
        ).await?;
        
        let task_manager = self.task_manager.clone();
        let task_id_clone = task_id.clone();
        let task_manager_for_completion = task_manager.clone();
        let task_id_for_completion = task_id_clone.clone();

        tokio::spawn(async move {
            let result = async move {
                tracing::info!("Downloading podcast episode {} for user {}", episode_id, user_id);
                
                // Update progress to starting with item_id
                task_manager.update_task_progress_with_details(&task_id_clone, 0.0, Some("Starting download...".to_string()), Some(episode_id), Some("podcast_download".to_string()), None).await?;
                
                // Get complete episode metadata from database
                let episode_info = match &db_pool {
                    crate::database::DatabasePool::Postgres(pool) => {
                        let row = sqlx::query(r#"
                            SELECT e."episodeurl", e."episodetitle", p."podcastname", 
                                   e."episodepubdate", p."author", e."episodeartwork", p."artworkurl",
                                   e."episodedescription"
                            FROM "Episodes" e
                            JOIN "Podcasts" p ON e."podcastid" = p."podcastid"
                            WHERE e."episodeid" = $1
                        "#)
                        .bind(episode_id)
                        .fetch_one(pool)
                        .await?;
                        
                        (
                            row.try_get::<String, _>("episodeurl")?,
                            row.try_get::<String, _>("episodetitle")?,
                            row.try_get::<String, _>("podcastname")?,
                            row.try_get::<Option<chrono::NaiveDateTime>, _>("episodepubdate")?,
                            row.try_get::<Option<String>, _>("author")?,
                            row.try_get::<Option<String>, _>("episodeartwork")?,
                            row.try_get::<Option<String>, _>("artworkurl")?,
                            row.try_get::<Option<String>, _>("episodedescription")?
                        )
                    }
                    crate::database::DatabasePool::MySQL(pool) => {
                        let row = sqlx::query("
                            SELECT e.EpisodeURL, e.EpisodeTitle, p.PodcastName,
                                   e.EpisodePubDate, p.Author, e.EpisodeArtwork, p.ArtworkURL,
                                   e.EpisodeDescription
                            FROM Episodes e
                            JOIN Podcasts p ON e.PodcastID = p.PodcastID
                            WHERE e.EpisodeID = ?
                        ")
                        .bind(episode_id)
                        .fetch_one(pool)
                        .await?;
                        
                        (
                            row.try_get::<String, _>("EpisodeURL")?,
                            row.try_get::<String, _>("EpisodeTitle")?,
                            row.try_get::<String, _>("PodcastName")?,
                            row.try_get::<Option<chrono::NaiveDateTime>, _>("EpisodePubDate")?,
                            row.try_get::<Option<String>, _>("Author")?,
                            row.try_get::<Option<String>, _>("EpisodeArtwork")?,
                            row.try_get::<Option<String>, _>("ArtworkURL")?,
                            row.try_get::<Option<String>, _>("EpisodeDescription")?
                        )
                    }
                };
                
                let (episode_url, episode_title, podcast_name, pub_date, author, episode_artwork, artwork_url, description) = episode_info;
                
                let status_message = format!("Preparing {}", episode_title);
                task_manager.update_task_progress_with_details(&task_id_clone, 10.0, Some(status_message.clone()), Some(episode_id), Some("podcast_download".to_string()), Some(episode_title.clone())).await?;
                
                // Create download directory
                let download_dir = std::path::Path::new("/pinepods/downloads");
                if !download_dir.exists() {
                    std::fs::create_dir_all(download_dir)
                        .map_err(|e| crate::error::AppError::internal(&format!("Failed to create download directory: {}", e)))?;
                }
                
                // Sanitize filename
                let safe_podcast_name = podcast_name.chars()
                    .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
                    .collect::<String>();
                let safe_episode_title = episode_title.chars()
                    .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
                    .collect::<String>();
                
                let file_extension = if episode_url.contains(".mp3") { "mp3" } else { "m4a" };
                let filename = format!("{}_{}.{}", safe_podcast_name, safe_episode_title, file_extension);
                let file_path = download_dir.join(&filename);
                
                let status_message = format!("Connecting to {}", episode_title);
                task_manager.update_task_progress_with_details(&task_id_clone, 20.0, Some(status_message), Some(episode_id), Some("podcast_download".to_string()), Some(episode_title.clone())).await?;
                
                // Download the file
                let client = reqwest::Client::new();
                let mut response = client.get(&episode_url)
                    .send()
                    .await
                    .map_err(|e| crate::error::AppError::internal(&format!("Failed to start download: {}", e)))?;
                
                if !response.status().is_success() {
                    return Err(crate::error::AppError::internal(&format!("Server returned error: {}", response.status())));
                }
                
                let total_size = response.content_length().unwrap_or(0);
                let mut downloaded = 0;
                let mut file = std::fs::File::create(&file_path)
                    .map_err(|e| crate::error::AppError::internal(&format!("Failed to create file: {}", e)))?;
                
                let status_message = format!("Starting download {}", episode_title);
                task_manager.update_task_progress_with_details(&task_id_clone, 25.0, Some(status_message), Some(episode_id), Some("podcast_download".to_string()), Some(episode_title.clone())).await?;
                
                // Download in chunks with progress updates (throttled)
                use std::io::Write;
                let mut last_reported_progress = 0.0;
                
                while let Some(chunk) = response.chunk().await
                    .map_err(|e| crate::error::AppError::internal(&format!("Download failed: {}", e)))?
                {
                    file.write_all(&chunk)
                        .map_err(|e| crate::error::AppError::internal(&format!("Failed to write file: {}", e)))?;
                    
                    downloaded += chunk.len() as u64;
                    
                    if total_size > 0 {
                        let progress = 25.0 + (downloaded as f64 / total_size as f64) * 65.0; // 25% to 90%
                        
                        // Only send WebSocket updates every 5% to avoid overwhelming the browser
                        if progress - last_reported_progress >= 5.0 || downloaded == total_size {
                            let status_message = format!("Downloading {}", episode_title);
                            task_manager.update_task_progress_with_details(
                                &task_id_clone, 
                                progress, 
                                Some(status_message), 
                                Some(episode_id), 
                                Some("podcast_download".to_string()),
                                Some(episode_title.clone())
                            ).await?;
                            last_reported_progress = progress;
                        }
                    }
                }
                
                file.flush()
                    .map_err(|e| crate::error::AppError::internal(&format!("Failed to flush file: {}", e)))?;
                
                drop(file); // Close the file handle before metadata operations
                
                let status_message = format!("Processing {}", episode_title);
                task_manager.update_task_progress_with_details(&task_id_clone, 85.0, Some(status_message), Some(episode_id), Some("podcast_download".to_string()), Some(episode_title.clone())).await?;
                
                // Add metadata to the downloaded file
                if let Err(e) = add_podcast_metadata(
                    &file_path,
                    &episode_title,
                    author.as_deref().unwrap_or("Unknown"),
                    &podcast_name,
                    pub_date.as_ref(),
                    episode_artwork.as_deref().or(artwork_url.as_deref())
                ).await {
                    tracing::warn!("Failed to add metadata to {}: {}", file_path.display(), e);
                }
                
                let status_message = format!("Finalizing {}", episode_title);
                task_manager.update_task_progress_with_details(&task_id_clone, 90.0, Some(status_message), Some(episode_id), Some("podcast_download".to_string()), Some(episode_title.clone())).await?;
                
                // Update database with download info
                match &db_pool {
                    crate::database::DatabasePool::Postgres(pool) => {
                        sqlx::query(r#"
                            INSERT INTO "DownloadedEpisodes" ("userid", "episodeid", "downloadedsize", "downloadedlocation")
                            VALUES ($1, $2, $3, $4)
                        "#)
                        .bind(user_id)
                        .bind(episode_id)
                        .bind(downloaded as i64)
                        .bind(file_path.to_string_lossy().as_ref())
                        .execute(pool)
                        .await?;

                        // Update UserStats table to increment EpisodesDownloaded count
                        sqlx::query(r#"
                            UPDATE "UserStats" SET "EpisodesDownloaded" = "EpisodesDownloaded" + 1 WHERE "UserID" = $1
                        "#)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    }
                    crate::database::DatabasePool::MySQL(pool) => {
                        sqlx::query("
                            INSERT INTO DownloadedEpisodes (UserID, EpisodeID, DownloadedSize, DownloadedLocation)
                            VALUES (?, ?, ?, ?)
                        ")
                        .bind(user_id)
                        .bind(episode_id)
                        .bind(downloaded as i64)
                        .bind(file_path.to_string_lossy().as_ref())
                        .execute(pool)
                        .await?;

                        // Update UserStats table to increment EpisodesDownloaded count
                        sqlx::query("
                            UPDATE UserStats SET EpisodesDownloaded = EpisodesDownloaded + 1 WHERE UserID = ?
                        ")
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    }
                }
                
                let status_message = format!("Downloaded {}", episode_title);
                task_manager.update_task_progress_with_details(&task_id_clone, 100.0, Some(status_message), Some(episode_id), Some("podcast_download".to_string()), Some(episode_title.clone())).await?;
                
                Ok(serde_json::json!({
                    "episode_id": episode_id,
                    "user_id": user_id,
                    "status": "downloaded",
                    "file_path": file_path.to_string_lossy(),
                    "file_size": downloaded
                }))
            };

            match result.await {
                Ok(result) => {
                    if let Err(e) = task_manager_for_completion
                        .complete_task(&task_id_for_completion, Some(result), None)
                        .await
                    {
                        tracing::error!("Failed to mark task {} as completed: {}", task_id_for_completion, e);
                    }
                }
                Err(e) => {
                    if let Err(err) = task_manager_for_completion
                        .fail_task(&task_id_for_completion, e.to_string())
                        .await
                    {
                        tracing::error!("Failed to mark task {} as failed: {}", task_id_for_completion, err);
                    }
                }
            }
        });

        Ok(task_id)
    }

    pub async fn spawn_download_youtube_video(&self, video_id: i32, user_id: i32) -> AppResult<String> {
        self.spawn_task(
            "download_video".to_string(),
            user_id,
            move |task_id, task_manager, db_pool| async move {
                tracing::info!("Downloading YouTube video {} for user {}", video_id, user_id);
                
                // Get the video from database using the video ID
                let (youtube_video_id, video_title) = match &db_pool {
                    crate::database::DatabasePool::Postgres(pool) => {
                        let row = sqlx::query(r#"SELECT youtubevideoid, videotitle FROM "YouTubeVideos" WHERE videoid = $1"#)
                            .bind(video_id)
                            .fetch_one(pool)
                            .await
                            .map_err(|e| crate::error::AppError::internal(&format!("Failed to get video: {}", e)))?;
                        
                        let youtube_video_id: String = row.try_get("youtubevideoid")
                            .map_err(|e| crate::error::AppError::internal(&format!("Failed to get YouTube video ID: {}", e)))?;
                        let video_title: String = row.try_get("videotitle")
                            .map_err(|e| crate::error::AppError::internal(&format!("Failed to get video title: {}", e)))?;
                        
                        (youtube_video_id, video_title)
                    }
                    crate::database::DatabasePool::MySQL(pool) => {
                        let row = sqlx::query("SELECT YouTubeVideoID, VideoTitle FROM YouTubeVideos WHERE VideoID = ?")
                            .bind(video_id)
                            .fetch_one(pool)
                            .await
                            .map_err(|e| crate::error::AppError::internal(&format!("Failed to get video: {}", e)))?;
                        
                        let youtube_video_id: String = row.try_get("YouTubeVideoID")
                            .map_err(|e| crate::error::AppError::internal(&format!("Failed to get YouTube video ID: {}", e)))?;
                        let video_title: String = row.try_get("VideoTitle")
                            .map_err(|e| crate::error::AppError::internal(&format!("Failed to get video title: {}", e)))?;
                        
                        (youtube_video_id, video_title)
                    }
                };
                
                let output_path = format!("/opt/pinepods/downloads/youtube/{}.mp3", youtube_video_id);
                
                // Check if file already exists
                if tokio::fs::metadata(&output_path).await.is_ok() {
                    tracing::info!("Video {} already downloaded", video_title);
                    return Ok(serde_json::json!({
                        "video_id": video_id,
                        "status": "already_downloaded",
                        "path": output_path
                    }));
                }
                
                // Download the video using the YouTube handler function
                match crate::handlers::youtube::download_youtube_audio(&youtube_video_id, &output_path).await {
                    Ok(_) => {
                        tracing::info!("Successfully downloaded YouTube video: {}", video_title);
                        
                        // Get duration from the downloaded MP3 file and update database
                        if let Some(duration) = crate::handlers::youtube::get_mp3_duration(&output_path) {
                            if let Err(e) = db_pool.update_youtube_video_duration(&youtube_video_id, duration).await {
                                tracing::error!("Failed to update duration for video {}: {}", youtube_video_id, e);
                            } else {
                                tracing::info!("Updated duration for video {} to {} seconds", youtube_video_id, duration);
                            }
                        } else {
                            tracing::warn!("Could not read duration from MP3 file: {}", output_path);
                        }
                        
                        Ok(serde_json::json!({
                            "video_id": video_id,
                            "user_id": user_id,
                            "status": "downloaded",
                            "path": output_path,
                            "title": video_title
                        }))
                    }
                    Err(e) => {
                        tracing::error!("Failed to download YouTube video {}: {}", video_title, e);
                        Err(e)
                    }
                }
            },
        ).await
    }

    pub async fn spawn_download_all_podcast_episodes(&self, podcast_id: i32, user_id: i32) -> AppResult<String> {
        self.spawn_task(
            "download_all_episodes".to_string(),
            user_id,
            move |task_id, task_manager, db_pool| async move {
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
            move |task_id, task_manager, db_pool| async move {
                tracing::info!("Downloading all videos for channel {} for user {}", channel_id, user_id);
                
                // Get all videos for the channel from database
                let videos_data = match &db_pool {
                    crate::database::DatabasePool::Postgres(pool) => {
                        let rows = sqlx::query(r#"SELECT videoid, youtubevideoid, videotitle FROM "YouTubeVideos" WHERE podcastid = $1"#)
                            .bind(channel_id)
                            .fetch_all(pool)
                            .await
                            .map_err(|e| crate::error::AppError::internal(&format!("Failed to get videos: {}", e)))?;
                        
                        rows.into_iter().map(|row| {
                            let youtube_video_id: String = row.try_get("youtubevideoid")
                                .map_err(|e| crate::error::AppError::internal(&format!("Failed to get YouTube video ID: {}", e)))?;
                            let video_title: String = row.try_get("videotitle")
                                .map_err(|e| crate::error::AppError::internal(&format!("Failed to get video title: {}", e)))?;
                            Ok((youtube_video_id, video_title))
                        }).collect::<Result<Vec<(String, String)>, crate::error::AppError>>()?
                    }
                    crate::database::DatabasePool::MySQL(pool) => {
                        let rows = sqlx::query("SELECT VideoID, YouTubeVideoID, VideoTitle FROM YouTubeVideos WHERE PodcastID = ?")
                            .bind(channel_id)
                            .fetch_all(pool)
                            .await
                            .map_err(|e| crate::error::AppError::internal(&format!("Failed to get videos: {}", e)))?;
                        
                        rows.into_iter().map(|row| {
                            let youtube_video_id: String = row.try_get("YouTubeVideoID")
                                .map_err(|e| crate::error::AppError::internal(&format!("Failed to get YouTube video ID: {}", e)))?;
                            let video_title: String = row.try_get("VideoTitle")
                                .map_err(|e| crate::error::AppError::internal(&format!("Failed to get video title: {}", e)))?;
                            Ok((youtube_video_id, video_title))
                        }).collect::<Result<Vec<(String, String)>, crate::error::AppError>>()?
                    }
                };
                
                let total_videos = videos_data.len();
                let mut downloaded = 0;
                let mut already_downloaded = 0;
                let mut failed = 0;
                
                for (index, (youtube_video_id, video_title)) in videos_data.iter().enumerate() {
                    
                    let output_path = format!("/opt/pinepods/downloads/youtube/{}.mp3", youtube_video_id);
                    
                    // Update progress
                    let progress = (index as f64 / total_videos as f64) * 100.0;
                    task_manager.update_task_progress(&task_id, progress, Some(format!("Downloading: {}", video_title))).await?;
                    
                    // Check if file already exists
                    if tokio::fs::metadata(&output_path).await.is_ok() {
                        tracing::info!("Video {} already downloaded", video_title);
                        already_downloaded += 1;
                        continue;
                    }
                    
                    // Download the video
                    match crate::handlers::youtube::download_youtube_audio(youtube_video_id, &output_path).await {
                        Ok(_) => {
                            tracing::info!("Successfully downloaded: {}", video_title);
                            downloaded += 1;
                            
                            // Get duration from the downloaded MP3 file and update database
                            if let Some(duration) = crate::handlers::youtube::get_mp3_duration(&output_path) {
                                if let Err(e) = db_pool.update_youtube_video_duration(youtube_video_id, duration).await {
                                    tracing::error!("Failed to update duration for video {}: {}", youtube_video_id, e);
                                } else {
                                    tracing::info!("Updated duration for video {} to {} seconds", youtube_video_id, duration);
                                }
                            } else {
                                tracing::warn!("Could not read duration from MP3 file: {}", output_path);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to download {}: {}", video_title, e);
                            failed += 1;
                            // Continue with next video instead of failing entire batch
                        }
                    }
                }
                
                // Final progress update
                task_manager.update_task_progress(&task_id, 100.0, Some("Download batch completed".to_string())).await?;
                
                Ok(serde_json::json!({
                    "channel_id": channel_id,
                    "user_id": user_id,
                    "status": "completed",
                    "total_videos": total_videos,
                    "downloaded": downloaded,
                    "already_downloaded": already_downloaded,
                    "failed": failed
                }))
            },
        ).await
    }
}

// Function to add metadata to downloaded MP3 files
async fn add_podcast_metadata(
    file_path: &std::path::Path,
    title: &str,
    artist: &str,
    album: &str,
    date: Option<&chrono::NaiveDateTime>,
    artwork_url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use id3::TagLike;  // Import the trait to use methods
    use chrono::Datelike;  // For year(), month(), day() methods
    
    // Create ID3 tag and add basic metadata
    let mut tag = id3::Tag::new();
    tag.set_title(title);
    tag.set_artist(artist);
    tag.set_album(album);
    
    // Set date if available
    if let Some(date) = date {
        tag.set_date_recorded(id3::Timestamp {
            year: date.year(),
            month: Some(date.month() as u8),
            day: Some(date.day() as u8),
            hour: None,
            minute: None,
            second: None,
        });
    }
    
    // Add genre for podcasts
    tag.set_genre("Podcast");
    
    // Download and add artwork if available
    if let Some(artwork_url) = artwork_url {
        if let Ok(artwork_data) = download_artwork(artwork_url).await {
            // Determine MIME type based on the data
            let mime_type = if artwork_data.starts_with(&[0xFF, 0xD8, 0xFF]) {
                "image/jpeg"
            } else if artwork_data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
                "image/png"
            } else {
                "image/jpeg" // Default fallback
            };
            
            tag.add_frame(id3::frame::Picture {
                mime_type: mime_type.to_string(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: "Cover".to_string(),
                data: artwork_data,
            });
        }
    }
    
    // Write the tag to the file
    tag.write_to_path(file_path, id3::Version::Id3v24)?;
    
    Ok(())
}

// Helper function to download artwork
async fn download_artwork(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "PinePods/1.0")
        .send()
        .await?;
    
    if response.status().is_success() {
        let bytes = response.bytes().await?;
        // Limit artwork size to reasonable bounds (e.g., 5MB)
        if bytes.len() > 5 * 1024 * 1024 {
            return Err("Artwork too large".into());
        }
        Ok(bytes.to_vec())
    } else {
        Err(format!("Failed to download artwork: HTTP {}", response.status()).into())
    }
}