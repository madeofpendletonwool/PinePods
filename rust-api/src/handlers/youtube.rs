use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use std::collections::HashMap;

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

// Request struct for YouTube channel subscription
#[derive(Deserialize)]
pub struct YouTubeSubscribeRequest {
    pub channel_id: String,
    pub user_id: i32,
    pub feed_cutoff: Option<i32>,
}

// Query struct for YouTube subscription endpoint
#[derive(Deserialize)]
pub struct YouTubeSubscribeQuery {
    pub channel_id: String,
    pub user_id: i32,
    pub feed_cutoff: Option<i32>,
}

// Subscribe to YouTube channel - matches Python subscribe_to_youtube_channel function exactly
pub async fn subscribe_to_youtube_channel(
    State(state): State<AppState>,
    Query(query): Query<YouTubeSubscribeQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only subscribe for themselves
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only subscribe for yourself!"));
    }

    let feed_cutoff = query.feed_cutoff.unwrap_or(30);

    println!("Starting subscription for channel {}", query.channel_id);

    // Check if channel already exists
    let existing_id = state.db_pool.check_existing_channel_subscription(
        &query.channel_id,
        query.user_id,
    ).await?;

    if let Some(podcast_id) = existing_id {
        println!("Channel {} already subscribed", query.channel_id);
        return Ok(Json(serde_json::json!({
            "success": true,
            "podcast_id": podcast_id,
            "message": "Already subscribed to this channel"
        })));
    }

    println!("Getting channel info");
    let channel_info = get_youtube_channel_info(&query.channel_id).await?;

    println!("Adding channel to database");
    let podcast_id = state.db_pool.add_youtube_channel(
        &channel_info,
        query.user_id,
        feed_cutoff,
    ).await?;

    // Spawn background task to process YouTube videos
    let state_clone = state.clone();
    let channel_id_clone = query.channel_id.clone();
    tokio::spawn(async move {
        if let Err(e) = process_youtube_channel(podcast_id, &channel_id_clone, feed_cutoff, &state_clone).await {
            println!("Error processing YouTube channel {}: {}", channel_id_clone, e);
        }
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "podcast_id": podcast_id,
        "message": "Successfully subscribed to YouTube channel"
    })))
}

// Helper function to get YouTube channel info using yt-dlp binary
async fn get_youtube_channel_info(channel_id: &str) -> Result<HashMap<String, String>, AppError> {
    println!("Getting channel info for {}", channel_id);
    
    let channel_url = format!("https://www.youtube.com/channel/{}", channel_id);
    
    // Use yt-dlp binary to get channel info
    let output = Command::new("yt-dlp")
        .args(&[
            "--quiet",
            "--no-warnings", 
            "--extract-flat",
            "--playlist-items", "0", // Just get channel info, not videos
            "--socket-timeout", "30",
            "--timeout", "60",
            "--dump-json",
            &channel_url
        ])
        .output()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to execute yt-dlp: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::external_error(&format!("yt-dlp failed: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let channel_data: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| AppError::external_error(&format!("Failed to parse yt-dlp output: {}", e)))?;

    // Extract channel info exactly like Python implementation
    let mut channel_info = HashMap::new();
    
    channel_info.insert("channel_id".to_string(), channel_id.to_string());
    channel_info.insert("name".to_string(), 
        channel_data.get("channel").and_then(|v| v.as_str())
            .or_else(|| channel_data.get("title").and_then(|v| v.as_str()))
            .unwrap_or("").to_string());
    
    let description = channel_data.get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(500)
        .collect::<String>();
    channel_info.insert("description".to_string(), description);

    // Extract avatar/thumbnail URL
    let thumbnail_url = if let Some(thumbnails) = channel_data.get("thumbnails").and_then(|v| v.as_array()) {
        // Look for avatar thumbnails first
        let avatar_thumbs: Vec<_> = thumbnails.iter()
            .filter(|t| t.get("id").and_then(|id| id.as_str()).unwrap_or("").starts_with("avatar"))
            .collect();
        
        if !avatar_thumbs.is_empty() {
            avatar_thumbs.last()
                .and_then(|t| t.get("url"))
                .and_then(|u| u.as_str())
                .unwrap_or("")
        } else {
            // Look for thumbnails with "avatar" in URL
            let avatar_url_thumbs: Vec<_> = thumbnails.iter()
                .filter(|t| t.get("url").and_then(|url| url.as_str()).unwrap_or("").to_lowercase().contains("avatar"))
                .collect();
            
            if !avatar_url_thumbs.is_empty() {
                avatar_url_thumbs.last()
                    .and_then(|t| t.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
            } else {
                // Fall back to first thumbnail
                thumbnails.first()
                    .and_then(|t| t.get("url"))
                    .and_then(|u| u.as_str())
                    .unwrap_or("")
            }
        }
    } else {
        ""
    };
    
    channel_info.insert("thumbnail_url".to_string(), thumbnail_url.to_string());

    println!("Successfully extracted channel info for: {}", channel_info.get("name").unwrap_or(&"Unknown".to_string()));
    Ok(channel_info)
}

// Process YouTube channel videos - matches Python process_youtube_videos function exactly
async fn process_youtube_channel(
    podcast_id: i32,
    channel_id: &str,
    feed_cutoff: i32,
    state: &AppState,
) -> Result<(), AppError> {
    println!("{}", "=".repeat(50));
    println!("Starting YouTube channel processing");
    println!("Podcast ID: {}", podcast_id);
    println!("Channel ID: {}", channel_id);
    println!("{}", "=".repeat(50));

    let cutoff_date = chrono::Utc::now() - chrono::Duration::days(feed_cutoff as i64);
    println!("Cutoff date set to: {}", cutoff_date);

    // Clean up old videos
    println!("Cleaning up videos older than cutoff date...");
    state.db_pool.remove_old_youtube_videos(podcast_id, cutoff_date).await?;

    let channel_url = format!("https://www.youtube.com/channel/{}/videos", channel_id);
    println!("Fetching channel data from: {}", channel_url);

    // Get video list using yt-dlp
    let output = Command::new("yt-dlp")
        .args(&[
            "--quiet",
            "--no-warnings",
            "--extract-flat",
            "--ignore-errors",
            "--socket-timeout", "30",
            "--timeout", "60",
            "--dump-json",
            &channel_url
        ])
        .output()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to execute yt-dlp: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::external_error(&format!("yt-dlp failed: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let results: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| AppError::external_error(&format!("Failed to parse yt-dlp output: {}", e)))?;

    if !results.is_object() || !results.get("entries").is_some() {
        println!("No video list found in results");
        return Ok(());
    }

    let empty_vec = vec![];
    let entries = results.get("entries").and_then(|e| e.as_array()).unwrap_or(&empty_vec);
    println!("Found {} total videos", entries.len());

    let mut recent_videos = Vec::new();

    // Process each video
    for entry in entries {
        if !entry.is_object() || !entry.get("id").is_some() {
            println!("Skipping invalid entry");
            continue;
        }

        let video_id = entry.get("id").and_then(|v| v.as_str()).unwrap();
        println!("Processing video ID: {}", video_id);

        // Get video date using the same method as Python
        let published = get_video_date(video_id).await?;
        println!("Video publish date: {}", published);

        if published <= cutoff_date {
            println!("Video {} from {} is too old, stopping processing", video_id, published);
            break;
        }

        let video_data = serde_json::json!({
            "id": video_id,
            "title": entry.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            "description": entry.get("description").and_then(|v| v.as_str()).unwrap_or(""),
            "url": format!("https://www.youtube.com/watch?v={}", video_id),
            "thumbnail": entry.get("thumbnails")
                .and_then(|t| t.as_array())
                .and_then(|arr| arr.first())
                .and_then(|thumb| thumb.get("url"))
                .and_then(|url| url.as_str())
                .unwrap_or(""),
            "publish_date": published.to_rfc3339(),
            "duration": entry.get("duration").and_then(|v| v.as_i64()).unwrap_or(0)
        });

        println!("Successfully added video {} to processing queue", video_id);
        recent_videos.push(video_data);
    }

    println!("Processing complete - Found {} recent videos", recent_videos.len());

    if !recent_videos.is_empty() {
        println!("Starting database updates");
        
        // Get existing videos
        let existing_videos = state.db_pool.get_existing_youtube_videos(podcast_id).await?;

        // Filter out videos that already exist
        let mut new_videos = Vec::new();
        for video in &recent_videos {
            let video_url = format!("https://www.youtube.com/watch?v={}", 
                video.get("id").and_then(|v| v.as_str()).unwrap_or(""));
            if !existing_videos.contains(&video_url) {
                new_videos.push(video.clone());
            } else {
                println!("Video already exists, skipping: {}", 
                    video.get("title").and_then(|v| v.as_str()).unwrap_or(""));
            }
        }

        if !new_videos.is_empty() {
            state.db_pool.add_youtube_videos(podcast_id, &new_videos).await?;
            println!("Successfully added {} new videos", new_videos.len());
        } else {
            println!("No new videos to add");
        }

        // Download audio for recent videos
        println!("Starting audio downloads");
        let mut successful_downloads = 0;
        let mut failed_downloads = 0;

        for video in &recent_videos {
            let video_id = video.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let title = video.get("title").and_then(|v| v.as_str()).unwrap_or("");
            
            let output_path = format!("/opt/pinepods/downloads/youtube/{}.mp3", video_id);
            let output_path_double = format!("{}.mp3", output_path);

            println!("Processing download for video: {}", video_id);
            println!("Title: {}", title);
            println!("Target path: {}", output_path);

            // Check if file already exists
            if tokio::fs::metadata(&output_path).await.is_ok() || 
               tokio::fs::metadata(&output_path_double).await.is_ok() {
                println!("Audio file already exists, skipping download");
                continue;
            }

            println!("Starting download...");
            match download_youtube_audio(video_id, &output_path).await {
                Ok(_) => {
                    println!("Download completed successfully");
                    successful_downloads += 1;
                }
                Err(e) => {
                    failed_downloads += 1;
                    let error_msg = e.to_string();
                    if error_msg.to_lowercase().contains("members-only") {
                        println!("Skipping video {} - Members-only content: {}", video_id, title);
                    } else if error_msg.to_lowercase().contains("private") {
                        println!("Skipping video {} - Private video: {}", video_id, title);
                    } else if error_msg.to_lowercase().contains("unavailable") {
                        println!("Skipping video {} - Unavailable video: {}", video_id, title);
                    } else {
                        println!("Failed to download video {}: {}", video_id, title);
                        println!("Error: {}", error_msg);
                    }
                }
            }
        }

        println!("Download summary: {} successful, {} failed", successful_downloads, failed_downloads);
    } else {
        println!("No new videos to process");
    }

    // Update episode count
    state.db_pool.update_episode_count(podcast_id).await?;

    println!("{}", "=".repeat(50));
    println!("Channel processing complete");
    println!("{}", "=".repeat(50));

    Ok(())
}

// Get video date using web scraping (matches Python get_video_date function)
async fn get_video_date(video_id: &str) -> Result<chrono::DateTime<chrono::Utc>, AppError> {
    use chrono::TimeZone;
    
    let client = reqwest::Client::new();
    let url = format!("https://www.youtube.com/watch?v={}", video_id);
    
    let response = client.get(&url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to fetch video page: {}", e)))?;

    let html = response.text().await
        .map_err(|e| AppError::external_error(&format!("Failed to read response: {}", e)))?;

    // Parse HTML to find upload date (simplified version of Python's BeautifulSoup approach)
    if let Some(start) = html.find("\"uploadDate\":\"") {
        let date_start = start + "\"uploadDate\":\"".len();
        if let Some(end) = html[date_start..].find("\"") {
            let date_str = &html[date_start..date_start + end];
            if let Ok(parsed_date) = chrono::DateTime::parse_from_rfc3339(date_str) {
                return Ok(parsed_date.with_timezone(&chrono::Utc));
            }
        }
    }

    // Fallback to current time minus some hours if date not found
    Ok(chrono::Utc::now() - chrono::Duration::hours(1))
}

// Download YouTube audio using yt-dlp binary
async fn download_youtube_audio(video_id: &str, output_path: &str) -> Result<(), AppError> {
    // Remove .mp3 extension if present to prevent double extension
    let base_path = if output_path.ends_with(".mp3") {
        &output_path[..output_path.len() - 4]
    } else {
        output_path
    };

    let video_url = format!("https://www.youtube.com/watch?v={}", video_id);

    let output = Command::new("yt-dlp")
        .args(&[
            "--format", "bestaudio/best",
            "--extract-audio",
            "--audio-format", "mp3",
            "--output", base_path,
            "--ignore-errors",
            "--socket-timeout", "30",
            "--timeout", "60",
            &video_url
        ])
        .output()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to execute yt-dlp: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::external_error(&format!("yt-dlp download failed: {}", stderr)));
    }

    Ok(())
}