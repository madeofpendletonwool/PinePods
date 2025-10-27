use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use std::collections::{HashMap, HashSet};

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

// Query struct for YouTube channel search
#[derive(Deserialize)]
pub struct YouTubeSearchQuery {
    pub query: String,
    pub max_results: Option<i32>,
    pub user_id: i32,
}

// YouTube channel struct for search results - matches Python response exactly
#[derive(Serialize, Debug)]
pub struct YouTubeChannel {
    pub channel_id: String,
    pub name: String,
    pub description: String,
    pub subscriber_count: Option<i64>,
    pub url: String,
    pub video_count: Option<i64>,
    pub thumbnail_url: String,
    pub recent_videos: Vec<YouTubeVideo>,
}

// YouTube video struct for recent videos in channel - matches Python response exactly
#[derive(Serialize, Debug, Clone)]
pub struct YouTubeVideo {
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,  // Note: Python uses float, not i64
    pub url: String,
}

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

// Query struct for check YouTube channel endpoint
#[derive(Deserialize)]
pub struct CheckYouTubeChannelQuery {
    pub user_id: i32,
    pub channel_name: String,
    pub channel_url: String,
}

// Search YouTube channels - matches Python search_youtube_channels function exactly
pub async fn search_youtube_channels(
    State(state): State<AppState>,
    Query(query): Query<YouTubeSearchQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only search for themselves  
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only search with your own account."));
    }

    let max_results = query.max_results.unwrap_or(5);
    
    // First get channel ID using a search - matches Python exactly  
    let search_url = format!("ytsearch{}:{}", max_results * 4, query.query);
    
    println!("Searching YouTube with query: {}", query.query);
    
    // Use yt-dlp binary to search
    let output = Command::new("yt-dlp")
        .args(&[
            "--quiet",
            "--no-warnings",
            "--flat-playlist", 
            "--skip-download",
            "--dump-json",
            &search_url
        ])
        .output()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to execute yt-dlp: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::external_error(&format!("yt-dlp search failed: {}", stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    
    // Parse each line as a separate JSON object (yt-dlp outputs one JSON per line for search results)
    let mut entries = Vec::new();
    for line in stdout.lines() {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            entries.push(entry);
        }
    }

    if entries.is_empty() {
        return Ok(Json(serde_json::json!({"results": []})));
    }

    let mut processed_results = Vec::new();
    let mut seen_channels = HashSet::new();
    let mut channel_videos: HashMap<String, Vec<YouTubeVideo>> = HashMap::new();

    // Process entries to collect videos by channel - matches Python logic exactly
    for entry in &entries {
        if let Some(channel_id) = entry.get("channel_id").and_then(|v| v.as_str())
            .or_else(|| entry.get("uploader_id").and_then(|v| v.as_str())) {
            
            // First collect the video regardless of whether we've seen the channel
            if !channel_videos.contains_key(channel_id) {
                channel_videos.insert(channel_id.to_string(), Vec::new());
            }
            
            if let Some(videos) = channel_videos.get_mut(channel_id) {
                if videos.len() < 3 {  // Limit to 3 videos like Python
                    if let Some(video_id) = entry.get("id").and_then(|v| v.as_str()) {
                        let video = YouTubeVideo {
                            id: video_id.to_string(),
                            title: entry.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            duration: entry.get("duration").and_then(|v| v.as_f64()),
                            url: format!("https://www.youtube.com/watch?v={}", video_id),
                        };
                        videos.push(video);
                        println!("Added video to channel {}, now has {} videos", channel_id, videos.len());
                    }
                }
            }
        }
    }

    // Now process channels - matches Python logic exactly
    for entry in &entries {
        if let Some(channel_id) = entry.get("channel_id").and_then(|v| v.as_str())
            .or_else(|| entry.get("uploader_id").and_then(|v| v.as_str())) {
            
            // Check if we've already processed this channel
            if seen_channels.contains(channel_id) {
                continue;
            }
            seen_channels.insert(channel_id.to_string());

            // Get minimal channel info
            let channel_url = format!("https://www.youtube.com/channel/{}", channel_id);
            
            // Get thumbnail from search result - much faster than individual channel lookups
            let thumbnail_url = entry.get("channel_thumbnail").and_then(|v| v.as_str())
                .or_else(|| entry.get("thumbnail").and_then(|v| v.as_str()))
                .unwrap_or("").to_string();

            let channel_name = entry.get("channel").and_then(|v| v.as_str())
                .or_else(|| entry.get("uploader").and_then(|v| v.as_str()))
                .unwrap_or("").to_string();

            println!("Creating channel {} with {} videos", channel_id, 
                channel_videos.get(channel_id).map(|v| v.len()).unwrap_or(0));

            let channel = YouTubeChannel {
                channel_id: channel_id.to_string(),
                name: channel_name,
                description: entry.get("description").and_then(|v| v.as_str())
                    .unwrap_or("").chars().take(500).collect::<String>(),
                subscriber_count: None,  // Always null like Python
                url: channel_url,
                video_count: None,       // Always null like Python
                thumbnail_url,
                recent_videos: channel_videos.get(channel_id).cloned().unwrap_or_default(),
            };

            if processed_results.len() < max_results as usize {
                processed_results.push(channel);
            } else {
                break;
            }
        }
    }

    println!("Found {} channels", processed_results.len());
    Ok(Json(serde_json::json!({"results": processed_results})))
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

// Helper function to get YouTube channel info using Backend service
pub async fn get_youtube_channel_info(channel_id: &str) -> Result<HashMap<String, String>, AppError> {
    println!("Getting channel info for {} from Backend service", channel_id);
    
    // Get Backend URL from environment variable
    let search_api_url = std::env::var("SEARCH_API_URL")
        .map_err(|_| AppError::external_error("SEARCH_API_URL environment variable not set"))?;
    
    // Replace /api/search with /api/youtube/channel for the channel details endpoint
    let backend_url = search_api_url.replace("/api/search", &format!("/api/youtube/channel?id={}", channel_id));
    
    let client = reqwest::Client::new();
    let response = client.get(&backend_url)
        .send()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to call Backend service: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::external_error(&format!("Backend service error: {}", response.status())));
    }

    let channel_data: serde_json::Value = response.json()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to parse Backend response: {}", e)))?;

    // Extract channel info from Backend service response
    let mut channel_info = HashMap::new();
    
    channel_info.insert("channel_id".to_string(), channel_id.to_string());
    channel_info.insert("name".to_string(), 
        channel_data.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string());
    
    let description = channel_data.get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .chars()
        .take(500)
        .collect::<String>();
    channel_info.insert("description".to_string(), description);

    channel_info.insert("thumbnail_url".to_string(), 
        channel_data.get("thumbnailUrl").and_then(|v| v.as_str()).unwrap_or("").to_string());

    println!("Successfully extracted channel info for: {}", channel_info.get("name").unwrap_or(&"Unknown".to_string()));
    Ok(channel_info)
}

// Helper function to get MP3 duration from file
pub fn get_mp3_duration(file_path: &str) -> Option<i32> {
    match mp3_metadata::read_from_file(file_path) {
        Ok(metadata) => Some(metadata.duration.as_secs() as i32),
        Err(e) => {
            println!("Failed to read MP3 metadata from {}: {}", file_path, e);
            None
        }
    }
}

// Helper function to parse YouTube duration format (PT4M13S) to seconds
pub fn parse_youtube_duration(duration_str: &str) -> Option<i64> {
    if !duration_str.starts_with("PT") {
        return None;
    }
    
    let duration_part = &duration_str[2..]; // Remove "PT"
    let mut total_seconds = 0i64;
    let mut current_number = String::new();
    
    for ch in duration_part.chars() {
        if ch.is_ascii_digit() {
            current_number.push(ch);
        } else {
            if let Ok(num) = current_number.parse::<i64>() {
                match ch {
                    'H' => total_seconds += num * 3600,
                    'M' => total_seconds += num * 60,
                    'S' => total_seconds += num,
                    _ => {}
                }
            }
            current_number.clear();
        }
    }
    
    Some(total_seconds)
}

// Process YouTube channel videos using Backend service
pub async fn process_youtube_channel(
    podcast_id: i32,
    channel_id: &str,
    feed_cutoff: i32,
    state: &AppState,
) -> Result<(), AppError> {
    println!("{}", "=".repeat(50));
    println!("Starting YouTube channel processing with Backend service");
    println!("Podcast ID: {}", podcast_id);
    println!("Channel ID: {}", channel_id);
    println!("{}", "=".repeat(50));

    let cutoff_date = chrono::Utc::now() - chrono::Duration::days(feed_cutoff as i64);
    println!("Cutoff date set to: {}", cutoff_date);

    // Clean up old videos
    println!("Cleaning up videos older than cutoff date...");
    state.db_pool.remove_old_youtube_videos(podcast_id, cutoff_date).await?;

    // Get Backend URL from environment variable
    let search_api_url = std::env::var("SEARCH_API_URL")
        .map_err(|_| AppError::external_error("SEARCH_API_URL environment variable not set"))?;
    
    // Replace /api/search with /api/youtube/channel for the channel details endpoint
    let backend_url = search_api_url.replace("/api/search", &format!("/api/youtube/channel?id={}", channel_id));
    println!("Fetching channel data from Backend service: {}", backend_url);

    // Get video list using Backend service
    let client = reqwest::Client::new();
    let response = client.get(&backend_url)
        .send()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to call Backend service: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::external_error(&format!("Backend service error: {}", response.status())));
    }

    let channel_data: serde_json::Value = response.json()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to parse Backend response: {}", e)))?;

    let empty_vec = vec![];
    let recent_videos_data = channel_data.get("recentVideos")
        .and_then(|v| v.as_array())
        .unwrap_or(&empty_vec);
    
    println!("Found {} total videos from Backend service", recent_videos_data.len());

    let mut recent_videos = Vec::new();

    // Process each video from Backend service response
    for video_entry in recent_videos_data {
        let video_id = video_entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if video_id.is_empty() {
            println!("Skipping video with missing ID");
            continue;
        }

        println!("Processing video ID: {}", video_id);

        // Parse the publishedAt date from Backend service
        let published_str = video_entry.get("publishedAt").and_then(|v| v.as_str()).unwrap_or("");
        let published = chrono::DateTime::parse_from_rfc3339(published_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| {
                println!("Failed to parse date {}, using current time", published_str);
                chrono::Utc::now()
            });

        println!("Video publish date: {}", published);

        if published <= cutoff_date {
            println!("Video {} from {} is too old, stopping processing", video_id, published);
            break;
        }

        // Debug: print what we got from Backend for this video
        println!("Backend video data for {}: {:?}", video_id, video_entry);
        let duration_str = video_entry.get("duration").and_then(|v| v.as_str()).unwrap_or("");
        println!("Duration string from Backend: '{}'", duration_str);
        let parsed_duration = if !duration_str.is_empty() {
            parse_youtube_duration(duration_str).unwrap_or(0)
        } else {
            0
        };
        println!("Parsed duration: {}", parsed_duration);

        let video_data = serde_json::json!({
            "id": video_id,
            "title": video_entry.get("title").and_then(|v| v.as_str()).unwrap_or(""),
            "description": video_entry.get("description").and_then(|v| v.as_str()).unwrap_or(""),
            "url": format!("https://www.youtube.com/watch?v={}", video_id),
            "thumbnail": video_entry.get("thumbnail").and_then(|v| v.as_str()).unwrap_or(""),
            "publish_date": published.to_rfc3339(),
            "duration": duration_str  // Store as string for proper parsing in database
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
                    
                    // Get duration from the downloaded MP3 file and update database
                    if let Some(duration) = get_mp3_duration(&output_path) {
                        if let Err(e) = state.db_pool.update_youtube_video_duration(video_id, duration).await {
                            println!("Failed to update duration for video {}: {}", video_id, e);
                        } else {
                            println!("Updated duration for video {} to {} seconds", video_id, duration);
                        }
                    } else {
                        println!("Could not read duration from MP3 file: {}", output_path);
                    }
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


// Download YouTube audio using yt-dlp binary
pub async fn download_youtube_audio(video_id: &str, output_path: &str) -> Result<(), AppError> {
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

// Check if YouTube channel exists - matches Python api_check_youtube_channel function exactly
pub async fn check_youtube_channel(
    State(state): State<AppState>,
    Query(query): Query<CheckYouTubeChannelQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only check for themselves
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only check channels for yourself!"));
    }

    let exists = state.db_pool.check_youtube_channel(
        query.user_id,
        &query.channel_name,
        &query.channel_url,
    ).await?;

    Ok(Json(serde_json::json!({ "exists": exists })))
}