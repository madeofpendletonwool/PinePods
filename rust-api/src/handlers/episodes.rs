use axum::{extract::State, http::HeaderMap, response::Json};
use axum::response::{Response, IntoResponse};
use axum::http::{StatusCode, header};
use sqlx::Row;
use crate::{
    error::{AppError, AppResult},
    handlers::{extract_api_key, validate_api_key},
    models::{BulkEpisodeActionRequest, BulkEpisodeActionResponse},
    AppState,
};

// Bulk episode action handlers for efficient mass operations

// Bulk mark episodes as completed
pub async fn bulk_mark_episodes_completed(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only mark episodes as completed for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let (processed_count, failed_count) = state.db_pool
        .bulk_mark_episodes_completed(request.episode_ids, request.user_id, is_youtube)
        .await?;

    let message = if failed_count > 0 {
        format!("Marked {} episodes as completed, {} failed", processed_count, failed_count)
    } else {
        format!("Successfully marked {} episodes as completed", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Bulk save episodes
pub async fn bulk_save_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only save episodes for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let (processed_count, failed_count) = state.db_pool
        .bulk_save_episodes(request.episode_ids, request.user_id, is_youtube)
        .await?;

    let message = if failed_count > 0 {
        format!("Saved {} episodes, {} failed or already saved", processed_count, failed_count)
    } else {
        format!("Successfully saved {} episodes", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Bulk queue episodes
pub async fn bulk_queue_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only queue episodes for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let (processed_count, failed_count) = state.db_pool
        .bulk_queue_episodes(request.episode_ids, request.user_id, is_youtube)
        .await?;

    let message = if failed_count > 0 {
        format!("Queued {} episodes, {} failed or already queued", processed_count, failed_count)
    } else {
        format!("Successfully queued {} episodes", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Bulk download episodes - triggers download tasks
pub async fn bulk_download_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only download episodes for yourself!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let mut processed_count = 0;
    let mut failed_count = 0;

    // Check if episodes are already downloaded and queue download tasks
    for episode_id in request.episode_ids {
        let is_downloaded = state.db_pool
            .check_downloaded(request.user_id, episode_id, is_youtube)
            .await?;

        if !is_downloaded {
            let result = if is_youtube {
                state.task_spawner.spawn_download_youtube_video(episode_id, request.user_id).await
            } else {
                state.task_spawner.spawn_download_podcast_episode(episode_id, request.user_id).await
            };

            match result {
                Ok(_) => processed_count += 1,
                Err(_) => failed_count += 1,
            }
        }
    }

    let message = if failed_count > 0 {
        format!("Queued {} episodes for download, {} failed or already downloaded", processed_count, failed_count)
    } else {
        format!("Successfully queued {} episodes for download", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Bulk delete downloaded episodes - removes multiple downloaded episodes at once
pub async fn bulk_delete_downloaded_episodes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<BulkEpisodeActionRequest>,
) -> AppResult<Json<BulkEpisodeActionResponse>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    let calling_user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    if calling_user_id != request.user_id {
        return Err(AppError::forbidden("You can only delete your own downloaded episodes!"));
    }

    let is_youtube = request.is_youtube.unwrap_or(false);
    let (processed_count, failed_count) = state.db_pool
        .bulk_delete_downloaded_episodes(request.episode_ids, request.user_id, is_youtube)
        .await?;

    let message = if failed_count > 0 {
        format!("Deleted {} downloaded episodes, {} failed or were not found", processed_count, failed_count)
    } else {
        format!("Successfully deleted {} downloaded episodes", processed_count)
    };

    Ok(Json(BulkEpisodeActionResponse {
        message,
        processed_count,
        failed_count: if failed_count > 0 { Some(failed_count) } else { None },
    }))
}

// Share episode - creates a shareable URL that expires in 60 days
pub async fn share_episode(
    State(state): State<AppState>,
    axum::extract::Path(episode_id): axum::extract::Path<i32>,
    headers: HeaderMap,
) -> AppResult<Json<serde_json::Value>> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;
    
    // Get the user ID from the API key
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Generate unique share code and expiration date
    let share_code = uuid::Uuid::new_v4().to_string();
    let expiration_date = chrono::Utc::now() + chrono::Duration::days(60);
    
    // Insert the shared episode entry
    let result = state.db_pool
        .add_shared_episode(episode_id, user_id, &share_code, expiration_date)
        .await?;
    
    if result {
        Ok(Json(serde_json::json!({ "url_key": share_code })))
    } else {
        Err(AppError::internal("Failed to share episode"))
    }
}

// Get episode by URL key - for accessing shared episodes
pub async fn get_episode_by_url_key(
    State(state): State<AppState>,
    axum::extract::Path(url_key): axum::extract::Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    // Find the episode ID associated with the URL key
    let episode_id = match state.db_pool.get_episode_id_by_share_code(&url_key).await? {
        Some(id) => id,
        None => return Err(AppError::not_found("Invalid or expired URL key")),
    };
    
    // Now retrieve the episode metadata using the special shared episode method
    // This bypasses user restrictions for public shared access
    let episode_data = state.db_pool
        .get_shared_episode_metadata(episode_id)
        .await?;
    
    Ok(Json(serde_json::json!({ "episode": episode_data })))
}

// Download episode file with metadata
pub async fn download_episode_file(
    State(state): State<AppState>,
    axum::extract::Path(episode_id): axum::extract::Path<i32>,
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<impl IntoResponse> {
    // Try to get API key from header first, then from query parameter
    let api_key = if let Ok(key) = extract_api_key(&headers) {
        key
    } else if let Some(key) = params.get("api_key") {
        key.clone()
    } else {
        return Err(AppError::unauthorized("API key is required"));
    };
    
    validate_api_key(&state, &api_key).await?;
    
    let user_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    
    // Get episode metadata
    let episode_info = match &state.db_pool {
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
    
    let (episode_url, episode_title, podcast_name, pub_date, author, episode_artwork, artwork_url, _description) = episode_info;
    
    // Download the episode file
    let client = reqwest::Client::new();
    let response = client.get(&episode_url)
        .send()
        .await
        .map_err(|e| AppError::internal(&format!("Failed to download episode: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(AppError::internal(&format!("Server returned error: {}", response.status())));
    }
    
    let audio_bytes = response.bytes()
        .await
        .map_err(|e| AppError::internal(&format!("Failed to download audio content: {}", e)))?;
    
    // Create a temporary file for metadata processing
    let temp_dir = std::env::temp_dir();
    let temp_filename = format!("episode_{}_{}_{}.mp3", episode_id, user_id, chrono::Utc::now().timestamp());
    let temp_path = temp_dir.join(&temp_filename);
    
    // Write audio content to temp file
    std::fs::write(&temp_path, &audio_bytes)
        .map_err(|e| AppError::internal(&format!("Failed to write temp file: {}", e)))?;
    
    // Add metadata using the same function as server downloads
    if let Err(e) = add_podcast_metadata(
        &temp_path,
        &episode_title,
        author.as_deref().unwrap_or("Unknown"),
        &podcast_name,
        pub_date.as_ref(),
        episode_artwork.as_deref().or(artwork_url.as_deref())
    ).await {
        tracing::warn!("Failed to add metadata to downloaded episode: {}", e);
    }
    
    // Read the file with metadata back
    let final_bytes = std::fs::read(&temp_path)
        .map_err(|e| AppError::internal(&format!("Failed to read processed file: {}", e)))?;
    
    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);
    
    // Create safe filename for download
    let safe_episode_title = episode_title.chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    
    let safe_podcast_name = podcast_name.chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_string();
    
    let pub_date_str = if let Some(date) = pub_date {
        date.format("%Y-%m-%d").to_string()
    } else {
        chrono::Utc::now().format("%Y-%m-%d").to_string()
    };
    
    let filename = format!("{}_{}_-_{}.mp3", pub_date_str, safe_podcast_name, safe_episode_title);
    
    // Return the file with appropriate headers
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/mpeg")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
        .header(header::CONTENT_LENGTH, final_bytes.len())
        .body(axum::body::Body::from(final_bytes))
        .map_err(|e| AppError::internal(&format!("Failed to create response: {}", e)))?;
    
    Ok(response)
}

// Function to add metadata to downloaded MP3 files (copied from tasks.rs)
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

// Helper function to download artwork (copied from tasks.rs)
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