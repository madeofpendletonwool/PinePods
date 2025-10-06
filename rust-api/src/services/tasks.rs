use crate::{
    error::AppResult,
    services::task_manager::TaskManager,
    database::DatabasePool,
};
use futures::Future;
use serde_json::Value;
use std::sync::Arc;
use sqlx::Row;

// New function that actually downloads an episode and waits for completion
async fn download_episode_and_wait(
    db_pool: &crate::database::DatabasePool,
    episode_id: i32,
    user_id: i32,
) -> Result<String, crate::error::AppError> {
    tracing::info!("Starting actual download for episode {} for user {}", episode_id, user_id);
    
    // Get episode metadata from database
    let episode_info = match db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            let row = sqlx::query(r#"
                SELECT e.episodeurl, e.episodetitle, p.podcastname, 
                       e.episodepubdate
                FROM "Episodes" e
                JOIN "Podcasts" p ON e.podcastid = p.podcastid
                WHERE e.episodeid = $1
            "#)
            .bind(episode_id)
            .fetch_one(pool)
            .await?;
            
            (
                row.try_get::<String, _>("episodeurl")?,
                row.try_get::<String, _>("episodetitle")?,
                row.try_get::<String, _>("podcastname")?,
                row.try_get::<Option<chrono::NaiveDateTime>, _>("episodepubdate")?,
            )
        }
        crate::database::DatabasePool::MySQL(pool) => {
            let row = sqlx::query("
                SELECT e.EpisodeURL, e.EpisodeTitle, p.PodcastName, e.EpisodePubDate
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
            )
        }
    };
    
    let (episode_url, episode_title, podcast_name, pub_date) = episode_info;
    
    // Create download directory structure
    let safe_podcast_name = podcast_name.chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    
    let safe_episode_title = episode_title.chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    
    let download_dir = std::path::Path::new("/opt/pinepods/downloads").join(&safe_podcast_name);
    if !download_dir.exists() {
        std::fs::create_dir_all(&download_dir)
            .map_err(|e| crate::error::AppError::Internal(format!("Failed to create download directory: {}", e)))?;
        
        // Set ownership using PUID/PGID environment variables
        let puid: u32 = std::env::var("PUID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
        let pgid: u32 = std::env::var("PGID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
        
        // Set directory ownership (ignore errors for NFS mounts)
        let _ = std::process::Command::new("chown")
            .args(&[format!("{}:{}", puid, pgid), download_dir.to_string_lossy().to_string()])
            .output();
    }
    
    let pub_date_str = if let Some(date) = pub_date {
        date.format("%Y-%m-%d").to_string()
    } else {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    };
    
    let filename = format!("{}_{}_{}_{}.mp3", pub_date_str, safe_episode_title, user_id, episode_id);
    let file_path = download_dir.join(&filename);
    
    // Download the file
    let client = reqwest::Client::new();
    let mut response = client.get(&episode_url)
        .send()
        .await
        .map_err(|e| crate::error::AppError::Internal(format!("Failed to start download: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(crate::error::AppError::Internal(format!("Server returned error: {}", response.status())));
    }
    
    let mut file = std::fs::File::create(&file_path)
        .map_err(|e| crate::error::AppError::Internal(format!("Failed to create file: {}", e)))?;
    
    // Download the content
    while let Some(chunk) = response.chunk().await.map_err(|e| crate::error::AppError::Internal(format!("Download failed: {}", e)))? {
        std::io::Write::write_all(&mut file, &chunk)
            .map_err(|e| crate::error::AppError::Internal(format!("Failed to write file: {}", e)))?;
    }
    
    // Close the file before setting ownership
    drop(file);
    
    // Set file ownership using PUID/PGID environment variables
    let puid: u32 = std::env::var("PUID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
    let pgid: u32 = std::env::var("PGID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
    
    // Set file ownership (ignore errors for NFS mounts)
    let _ = std::process::Command::new("chown")
        .args(&[format!("{}:{}", puid, pgid), file_path.to_string_lossy().to_string()])
        .output();
    
    // Record download in database  
    let file_size = tokio::fs::metadata(&file_path).await
        .map(|m| m.len() as i64)
        .unwrap_or(0);
        
    match db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            sqlx::query(r#"INSERT INTO "DownloadedEpisodes" (userid, episodeid, downloadedsize, downloadedlocation) VALUES ($1, $2, $3, $4)"#)
                .bind(user_id)
                .bind(episode_id)
                .bind(file_size)
                .bind(file_path.to_string_lossy().to_string())
                .execute(pool)
                .await?;
        }
        crate::database::DatabasePool::MySQL(pool) => {
            sqlx::query("INSERT INTO DownloadedEpisodes (UserID, EpisodeID, DownloadedSize, DownloadedLocation) VALUES (?, ?, ?, ?)")
                .bind(user_id)
                .bind(episode_id)
                .bind(file_size)
                .bind(file_path.to_string_lossy().to_string())
                .execute(pool)
                .await?;
        }
    }
    
    tracing::info!("Successfully downloaded episode {} - {}", episode_id, episode_title);
    Ok(episode_title)
}

#[derive(Clone)]
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
                
                // Create download directory structure like Python version
                let safe_podcast_name = podcast_name.chars()
                    .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
                    .collect::<String>()
                    .trim()
                    .to_string();
                
                let safe_episode_title = episode_title.chars()
                    .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
                    .collect::<String>()
                    .trim()
                    .to_string();
                
                // Create podcast-specific directory (like Python version)
                let download_dir = std::path::Path::new("/opt/pinepods/downloads").join(&safe_podcast_name);
                if !download_dir.exists() {
                    std::fs::create_dir_all(&download_dir)
                        .map_err(|e| crate::error::AppError::internal(&format!("Failed to create download directory: {}", e)))?;
                    
                    // Set ownership using PUID/PGID environment variables
                    let puid: u32 = std::env::var("PUID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
                    let pgid: u32 = std::env::var("PGID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
                    
                    // Set directory ownership (ignore errors for NFS mounts)
                    let _ = std::process::Command::new("chown")
                        .args(&[format!("{}:{}", puid, pgid), download_dir.to_string_lossy().to_string()])
                        .output();
                }
                
                // Format date for filename (like Python version)
                let pub_date_str = if let Some(date) = pub_date {
                    date.format("%Y-%m-%d").to_string()
                } else {
                    chrono::Utc::now().format("%Y-%m-%d").to_string()
                };
                
                // Create filename with date, title, and IDs (like Python version)
                let filename = format!("{}_{}_{}_{}.mp3", pub_date_str, safe_episode_title, user_id, episode_id);
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
                
                // Set file ownership using PUID/PGID environment variables
                let puid: u32 = std::env::var("PUID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
                let pgid: u32 = std::env::var("PGID").unwrap_or_else(|_| "1000".to_string()).parse().unwrap_or(1000);
                
                // Set file ownership (ignore errors for NFS mounts)
                let _ = std::process::Command::new("chown")
                    .args(&[format!("{}:{}", puid, pgid), file_path.to_string_lossy().to_string()])
                    .output();
                
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
                            INSERT INTO "DownloadedEpisodes" (userid, episodeid, downloadedsize, downloadedlocation)
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
                            UPDATE "UserStats" SET episodesdownloaded = episodesdownloaded + 1 WHERE userid = $1
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
        // Create the task first
        let task_id = self.task_manager.create_task("download_all_episodes".to_string(), user_id).await?;
        let task_manager = self.task_manager.clone();
        let task_spawner = self.clone();
        let db_pool = self.db_pool.clone();
        let task_id_clone = task_id.clone();
        let task_manager_for_completion = task_manager.clone();
        let task_id_for_completion = task_id_clone.clone();

        tokio::spawn(async move {
            let result: Result<serde_json::Value, crate::error::AppError> = (async move {
                tracing::info!("Downloading all episodes for podcast {} for user {}", podcast_id, user_id);
                
                // Update progress to starting
                task_manager.update_task_progress_with_details(&task_id_clone, 0.0, Some("Getting episode list...".to_string()), None, Some("bulk_download".to_string()), None).await?;
                
                // Get episode IDs that are NOT already downloaded (replicating check_downloaded logic)
                let episode_ids = match &db_pool {
                    crate::database::DatabasePool::Postgres(pool) => {
                        let rows = sqlx::query(r#"
                            SELECT e.episodeid 
                            FROM "Episodes" e
                            LEFT JOIN "DownloadedEpisodes" de ON e.episodeid = de.episodeid AND de.userid = $2
                            WHERE e.podcastid = $1 AND de.episodeid IS NULL
                            ORDER BY e.episodepubdate DESC
                        "#)
                            .bind(podcast_id)
                            .bind(user_id)
                            .fetch_all(pool)
                            .await?;
                        
                        rows.into_iter()
                            .map(|row| row.try_get::<i32, _>("episodeid"))
                            .collect::<Result<Vec<i32>, _>>()?
                    }
                    crate::database::DatabasePool::MySQL(pool) => {
                        let rows = sqlx::query("
                            SELECT e.EpisodeID 
                            FROM Episodes e
                            LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID AND de.UserID = ?
                            WHERE e.PodcastID = ? AND de.EpisodeID IS NULL
                            ORDER BY e.EpisodePubDate DESC
                        ")
                            .bind(user_id)
                            .bind(podcast_id)
                            .fetch_all(pool)
                            .await?;
                        
                        rows.into_iter()
                            .map(|row| row.try_get::<i32, _>("EpisodeID"))
                            .collect::<Result<Vec<i32>, _>>()?
                    }
                };
                
                let total_episodes = episode_ids.len();
                tracing::info!("Found {} episodes for podcast {} to download", total_episodes, podcast_id);
                
                if total_episodes == 0 {
                    task_manager.update_task_progress_with_details(&task_id_clone, 100.0, Some("No episodes found to download".to_string()), None, Some("bulk_download".to_string()), None).await?;
                    return Ok(serde_json::json!({
                        "podcast_id": podcast_id,
                        "user_id": user_id,
                        "status": "no_episodes_found",
                        "total_episodes": 0
                    }));
                }
                
                // Download episodes ONE at a time sequentially
                let mut successful_downloads = 0;
                
                for (index, episode_id) in episode_ids.iter().enumerate() {
                    tracing::info!("Starting download {}/{}: episode {}", index + 1, total_episodes, episode_id);
                    
                    // Update progress before starting download
                    let progress = (index as f64 / total_episodes as f64) * 100.0;
                    task_manager.update_task_progress_with_details(
                        &task_id_clone, 
                        progress, 
                        Some(format!("Starting download {}/{} episodes...", index + 1, total_episodes)), 
                        None, 
                        Some("bulk_download".to_string()), 
                        None
                    ).await?;
                    
                    // Actually download the episode and wait for it to complete
                    match download_episode_and_wait(&db_pool, *episode_id, user_id).await {
                        Ok(episode_title) => {
                            successful_downloads += 1;
                            tracing::info!("Successfully downloaded episode {} - {}", episode_id, episode_title);
                            
                            // Update progress after actual completion
                            let completed_progress = ((index + 1) as f64 / total_episodes as f64) * 100.0;
                            task_manager.update_task_progress_with_details(
                                &task_id_clone, 
                                completed_progress, 
                                Some(format!("Downloaded {}/{} episodes: {}", index + 1, total_episodes, episode_title)), 
                                None, 
                                Some("bulk_download".to_string()), 
                                None
                            ).await?;
                        }
                        Err(e) => {
                            tracing::warn!("Failed to download episode {}: {}", episode_id, e);
                        }
                    }
                }
                
                tracing::info!("Successfully started {} out of {} episode downloads", successful_downloads, total_episodes);
                
                task_manager.update_task_progress_with_details(
                    &task_id_clone, 
                    100.0, 
                    Some(format!("Successfully started {}/{} episode downloads", successful_downloads, total_episodes)), 
                    None, 
                    Some("bulk_download".to_string()), 
                    None
                ).await?;
                
                tracing::info!("Successfully started {} out of {} episode downloads for podcast {} for user {}", successful_downloads, total_episodes, podcast_id, user_id);
                
                Ok(serde_json::json!({
                    "podcast_id": podcast_id,
                    "user_id": user_id,
                    "status": "episodes_queued_sequentially",
                    "total_episodes": total_episodes,
                    "queued_episodes": successful_downloads
                }))
            }).await;

            match result {
                Ok(response) => {
                    if let Err(e) = task_manager_for_completion.complete_task(&task_id_for_completion, Some(response), Some("All episodes queued for download".to_string())).await {
                        tracing::error!("Failed to complete download all episodes task: {}", e);
                    }
                }
                Err(e) => {
                    if let Err(err) = task_manager_for_completion.fail_task(&task_id_for_completion, format!("Download all episodes failed: {}", e)).await {
                        tracing::error!("Failed to mark download all episodes task as failed: {}", err);
                    }
                }
            }
        });

        Ok(task_id)
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

impl TaskSpawner {
    pub async fn spawn_add_podcast_episodes_task(
        &self,
        podcast_id: i32,
        feed_url: String,
        artwork_url: String,
        user_id: i32,
        username: Option<String>,
        password: Option<String>,
    ) -> AppResult<String> {
        let task_type = "add_podcast_episodes".to_string();
        
        self.spawn_task(
            task_type,
            user_id,
            move |task_id, task_manager, db_pool| {
                Box::pin(async move {
                    println!("Starting episode processing for podcast {} (user {})", podcast_id, user_id);
                    
                    // Update progress - starting
                    task_manager.update_task_progress(&task_id, 10.0, Some("Fetching podcast feed...".to_string())).await?;
                    
                    // Add episodes to the existing podcast
                    match db_pool.add_episodes(
                        podcast_id,
                        &feed_url,
                        &artwork_url,
                        false, // auto_download
                        username.as_deref(),
                        password.as_deref(),
                    ).await {
                        Ok(first_episode_id) => {
                            // Update progress - fetching count
                            task_manager.update_task_progress(&task_id, 80.0, Some("Counting episodes...".to_string())).await?;
                            
                            // Count episodes for logging and notification
                            let episode_count: i64 = match &db_pool {
                                crate::database::DatabasePool::Postgres(pool) => {
                                    sqlx::query_scalar(r#"SELECT COUNT(*) FROM "Episodes" WHERE podcastid = $1"#)
                                        .bind(podcast_id)
                                        .fetch_one(pool)
                                        .await?
                                }
                                crate::database::DatabasePool::MySQL(pool) => {
                                    sqlx::query_scalar("SELECT COUNT(*) FROM Episodes WHERE PodcastID = ?")
                                        .bind(podcast_id)
                                        .fetch_one(pool)
                                        .await?
                                }
                            };
                            
                            // Final progress update
                            task_manager.update_task_progress(&task_id, 100.0, Some(format!("Added {} episodes", episode_count))).await?;
                            
                            println!("✅ Added {} episodes for podcast {} (user {})", episode_count, podcast_id, user_id);
                            
                            Ok(serde_json::json!({
                                "podcast_id": podcast_id,
                                "user_id": user_id,
                                "episode_count": episode_count,
                                "first_episode_id": first_episode_id,
                                "status": "completed"
                            }))
                        }
                        Err(e) => {
                            println!("Failed to add episodes for podcast {}: {}", podcast_id, e);
                            task_manager.update_task_progress(&task_id, 0.0, Some(format!("Failed to add episodes: {}", e))).await?;
                            Err(e)
                        }
                    }
                })
            },
        ).await
    }
}