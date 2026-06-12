use axum::{
    extract::{Multipart, Query, State},
    http::HeaderMap,
    response::Json,
};
use chrono::{NaiveDateTime, Utc};
use id3::TagLike;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::{
    error::AppError,
    handlers::{extract_api_key, validate_api_key},
    AppState,
};

const LOCAL_MEDIA_ROOT: &str = "/opt/pinepods/local-media";
const ARTWORK_DIR: &str = "_artwork";
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "m4a", "ogg", "flac", "wav", "aac", "opus"];

#[derive(Deserialize)]
pub struct AddLocalPodcastRequest {
    pub user_id: i32,
    pub directory_path: String,
    pub podcast_name: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub explicit: Option<bool>,
}

#[derive(Deserialize)]
pub struct RefreshLocalPodcastRequest {
    pub user_id: i32,
    pub podcast_id: i32,
}

#[derive(Deserialize)]
pub struct ListLocalDirectoriesQuery {
    #[serde(default)]
    pub path: String,
}

#[derive(Serialize)]
pub struct LocalDirectoryEntry {
    pub name: String,
    pub path: String,
    pub audio_count: usize,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LocalEpisodeCandidate {
    pub file_path: String,
    pub title: String,
    pub description: String,
    pub pub_date: NaiveDateTime,
    pub duration: i32,
    pub track_num: Option<u32>,
    pub artwork_url: Option<String>,
}

fn validate_local_media_path(input: &str) -> Result<PathBuf, AppError> {
    let root = Path::new(LOCAL_MEDIA_ROOT);
    // Canonicalize resolves symlinks and `..` segments, blocking traversal
    let candidate = root.join(input);
    let canonical = candidate
        .canonicalize()
        .map_err(|_| AppError::bad_request("Directory does not exist or is not accessible"))?;
    if !canonical.starts_with(root) {
        return Err(AppError::bad_request("Path traversal detected"));
    }
    Ok(canonical)
}

fn ensure_artwork_dir() -> Result<PathBuf, AppError> {
    let artwork_dir = PathBuf::from(LOCAL_MEDIA_ROOT).join(ARTWORK_DIR);
    std::fs::create_dir_all(&artwork_dir)
        .map_err(|e| AppError::internal(format!("Failed to create artwork directory: {}", e)))?;
    Ok(artwork_dir)
}

fn save_artwork_bytes(data: &[u8], artwork_dir: &Path) -> Option<String> {
    if data.is_empty() {
        return None;
    }
    // Use a UUID filename to avoid collisions
    let filename = format!("{}.jpg", Uuid::new_v4());
    let path = artwork_dir.join(&filename);
    if std::fs::write(&path, data).is_ok() {
        Some(format!("/{}/{}/{}", "api/local-media", ARTWORK_DIR, filename))
    } else {
        None
    }
}

fn extract_id3_artwork(tag: &id3::Tag, artwork_dir: &Path) -> Option<String> {
    for pic in tag.pictures() {
        if !pic.data.is_empty() {
            return save_artwork_bytes(&pic.data, artwork_dir);
        }
    }
    None
}

fn find_cover_art_in_dir(dir: &Path, artwork_dir: &Path) -> Option<String> {
    for name in &["cover.jpg", "cover.jpeg", "cover.png", "folder.jpg", "folder.jpeg", "artwork.jpg", "artwork.png"] {
        let candidate = dir.join(name);
        if candidate.exists() {
            if let Ok(data) = std::fs::read(&candidate) {
                return save_artwork_bytes(&data, artwork_dir);
            }
        }
    }
    None
}

pub fn scan_local_directory(dir: &Path) -> Result<Vec<LocalEpisodeCandidate>, AppError> {
    let artwork_dir = ensure_artwork_dir()?;

    // Look for a cover art file in the directory once
    let dir_artwork = find_cover_art_in_dir(dir, &artwork_dir);

    let mut candidates: Vec<LocalEpisodeCandidate> = Vec::new();

    let entries = std::fs::read_dir(dir).map_err(|e| {
        AppError::bad_request(format!("Cannot read directory: {}", e))
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Filter by audio extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        if !AUDIO_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        let path_str = path.to_string_lossy().to_string();

        // Try to read ID3 tags (works for MP3/M4A/most formats)
        let (title, description, pub_date, duration, track_num, episode_artwork) =
            if let Ok(tag) = id3::Tag::read_from_path(&path) {
                let title = tag
                    .title()
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| {
                        path.file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("Unknown")
                            .to_string()
                    });

                let description = tag
                    .comments()
                    .next()
                    .map(|c| c.text.clone())
                    .unwrap_or_default();

                let pub_date = tag
                    .date_recorded()
                    .and_then(|d| {
                        NaiveDateTime::parse_from_str(
                            &format!(
                                "{:04}-{:02}-{:02} 00:00:00",
                                d.year,
                                d.month.unwrap_or(1),
                                d.day.unwrap_or(1)
                            ),
                            "%Y-%m-%d %H:%M:%S",
                        )
                        .ok()
                    })
                    .or_else(|| {
                        // Fallback: file modification time
                        std::fs::metadata(&path)
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| {
                                let secs = t
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .ok()?
                                    .as_secs() as i64;
                                NaiveDateTime::from_timestamp_opt(secs, 0)
                            })
                    })
                    .unwrap_or_else(|| Utc::now().naive_utc());

                let track_num = tag.track();

                let duration = crate::handlers::youtube::get_mp3_duration(&path_str)
                    .unwrap_or(0);

                let episode_artwork = extract_id3_artwork(&tag, &artwork_dir);

                (title, description, pub_date, duration, track_num, episode_artwork)
            } else {
                // No ID3 tags — use filename and file metadata
                let title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let pub_date = std::fs::metadata(&path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| {
                        let secs = t
                            .duration_since(std::time::UNIX_EPOCH)
                            .ok()?
                            .as_secs() as i64;
                        NaiveDateTime::from_timestamp_opt(secs, 0)
                    })
                    .unwrap_or_else(|| Utc::now().naive_utc());

                let duration = crate::handlers::youtube::get_mp3_duration(&path_str)
                    .unwrap_or(0);

                (title, String::new(), pub_date, duration, None, None)
            };

        let artwork_url = episode_artwork.or_else(|| dir_artwork.clone());

        candidates.push(LocalEpisodeCandidate {
            file_path: path_str,
            title,
            description,
            pub_date,
            duration,
            track_num,
            artwork_url,
        });
    }

    // Sort: by track number first, then filename alphanumerically
    candidates.sort_by(|a, b| {
        match (a.track_num, b.track_num) {
            (Some(ta), Some(tb)) => ta.cmp(&tb),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => {
                let fa = Path::new(&a.file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                let fb = Path::new(&b.file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
                fa.cmp(fb)
            }
        }
    });

    Ok(candidates)
}

pub async fn add_local_podcast(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AddLocalPodcastRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id_from_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if request.user_id != user_id_from_key && !is_web_key {
        return Err(AppError::forbidden("You can only add podcasts for yourself"));
    }

    let canonical_path = validate_local_media_path(&request.directory_path)?;
    let feed_url = format!("local://{}", canonical_path.display());

    // Check for duplicate
    if state.db_pool.local_podcast_exists(&feed_url, request.user_id).await? {
        return Err(AppError::bad_request("A local podcast for this directory already exists"));
    }

    let candidates = scan_local_directory(&canonical_path)?;
    if candidates.is_empty() {
        return Err(AppError::bad_request(
            "No audio files found in the specified directory. Supported formats: mp3, m4a, ogg, flac, wav, aac, opus",
        ));
    }

    // Use cover art from the first episode that has artwork, or directory cover art
    let podcast_artwork = candidates
        .iter()
        .find_map(|c| c.artwork_url.clone());

    let podcast_id = state
        .db_pool
        .add_local_podcast(
            &request.podcast_name,
            &feed_url,
            request.description.as_deref().unwrap_or(""),
            request.author.as_deref().unwrap_or(""),
            podcast_artwork.as_deref().unwrap_or(""),
            request.explicit.unwrap_or(false),
            request.user_id,
        )
        .await?;

    state
        .db_pool
        .add_local_episodes(podcast_id, request.user_id, &candidates)
        .await?;

    let podcast_details = state
        .db_pool
        .get_podcast_details(request.user_id, podcast_id)
        .await?;

    println!(
        "✅ Local podcast '{}' added with {} episodes",
        request.podcast_name,
        candidates.len()
    );

    Ok(Json(serde_json::json!({ "data": podcast_details })))
}

pub async fn refresh_local_podcast(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RefreshLocalPodcastRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let user_id_from_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if request.user_id != user_id_from_key && !is_web_key {
        return Err(AppError::forbidden("You can only refresh your own podcasts"));
    }

    // Look up the podcast and verify it is a local podcast
    let feed_url = state
        .db_pool
        .get_feed_url_for_podcast(request.podcast_id, request.user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Podcast not found"))?;

    if !feed_url.starts_with("local://") {
        return Err(AppError::bad_request("This is not a local podcast"));
    }

    let dir_path = feed_url.strip_prefix("local://").unwrap_or(&feed_url);
    let canonical_path = Path::new(dir_path);
    if !canonical_path.exists() {
        return Err(AppError::bad_request("Local media directory no longer exists"));
    }

    let candidates = scan_local_directory(canonical_path)?;

    // Get existing episode file paths to avoid duplicates
    let existing_paths = state
        .db_pool
        .get_local_episode_paths(request.podcast_id)
        .await?;

    let new_candidates: Vec<LocalEpisodeCandidate> = candidates
        .into_iter()
        .filter(|c| !existing_paths.contains(&c.file_path))
        .collect();

    let new_count = new_candidates.len();
    if !new_candidates.is_empty() {
        state
            .db_pool
            .add_local_episodes(request.podcast_id, request.user_id, &new_candidates)
            .await?;
    }

    println!(
        "🔄 Refreshed local podcast {}: {} new episodes",
        request.podcast_id, new_count
    );

    Ok(Json(serde_json::json!({
        "detail": format!("Refresh complete: {} new episode(s) added", new_count),
        "new_episodes": new_count
    })))
}

pub async fn add_local_podcast_artwork(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    let mut podcast_id: Option<i32> = None;
    let mut user_id: Option<i32> = None;
    let mut artwork_data: Option<Vec<u8>> = None;
    let mut artwork_ext = "jpg".to_string();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "podcast_id" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::bad_request(format!("Failed to read podcast_id: {}", e)))?;
                podcast_id = text
                    .parse::<i32>()
                    .ok();
            }
            "user_id" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| AppError::bad_request(format!("Failed to read user_id: {}", e)))?;
                user_id = text.parse::<i32>().ok();
            }
            "artwork" => {
                // Detect extension from content type or filename
                if let Some(ct) = field.content_type() {
                    if ct.contains("png") {
                        artwork_ext = "png".to_string();
                    }
                }
                if let Some(filename) = field.file_name() {
                    if let Some(ext) = Path::new(filename).extension().and_then(|e| e.to_str()) {
                        artwork_ext = ext.to_lowercase();
                    }
                }
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad_request(format!("Failed to read artwork: {}", e)))?;
                artwork_data = Some(data.to_vec());
            }
            _ => {
                // Consume unknown fields
                let _ = field.bytes().await;
            }
        }
    }

    let podcast_id = podcast_id.ok_or_else(|| AppError::bad_request("Missing podcast_id"))?;
    let user_id = user_id.ok_or_else(|| AppError::bad_request("Missing user_id"))?;
    let data = artwork_data.ok_or_else(|| AppError::bad_request("Missing artwork file"))?;

    // Verify user owns this podcast
    let user_id_from_key = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;
    if user_id != user_id_from_key && !is_web_key {
        return Err(AppError::forbidden("You can only update your own podcasts"));
    }

    // Validate it's actually an image (check magic bytes)
    if data.len() < 4 {
        return Err(AppError::bad_request("Invalid image file"));
    }
    let is_jpeg = data.starts_with(&[0xFF, 0xD8, 0xFF]);
    let is_png = data.starts_with(&[0x89, 0x50, 0x4E, 0x47]);
    let is_gif = data.starts_with(b"GIF");
    let is_webp = data.len() >= 12 && &data[8..12] == b"WEBP";
    if !is_jpeg && !is_png && !is_gif && !is_webp {
        return Err(AppError::bad_request("File does not appear to be a valid image"));
    }

    let artwork_dir = ensure_artwork_dir()?;
    let filename = format!("podcast_{}.{}", podcast_id, artwork_ext);
    let file_path = artwork_dir.join(&filename);
    std::fs::write(&file_path, &data)
        .map_err(|e| AppError::internal(format!("Failed to save artwork: {}", e)))?;

    let artwork_url = format!("/{}/{}/{}", "api/local-media", ARTWORK_DIR, filename);

    state
        .db_pool
        .update_podcast_artwork(podcast_id, user_id, &artwork_url)
        .await?;

    Ok(Json(serde_json::json!({
        "detail": "Artwork updated successfully",
        "artwork_url": artwork_url
    })))
}

// Count immediate audio files in a directory (non-recursive)
fn count_audio_files(dir: &Path) -> usize {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return 0,
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return false;
            }
            path.extension()
                .and_then(|e| e.to_str())
                .map(|e| AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
                .unwrap_or(false)
        })
        .count()
}

// Detect the cover art a directory would use (cover.jpg/embedded ID3 art), so the
// frontend can preview it before adding. Mirrors the artwork the add flow auto-selects.
pub async fn detect_local_cover(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListLocalDirectoriesQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    if query.path.trim().is_empty() {
        return Ok(Json(serde_json::json!({ "artwork_url": serde_json::Value::Null })));
    }

    let canonical_path = validate_local_media_path(&query.path)?;
    if !canonical_path.is_dir() {
        return Ok(Json(serde_json::json!({ "artwork_url": serde_json::Value::Null })));
    }

    // Reuse the same scan the add flow uses, then pick the first available artwork —
    // exactly how add_local_podcast chooses the podcast artwork.
    let artwork_url = scan_local_directory(&canonical_path)
        .ok()
        .and_then(|candidates| candidates.iter().find_map(|c| c.artwork_url.clone()));

    Ok(Json(serde_json::json!({ "artwork_url": artwork_url })))
}

// List immediate subdirectories under the local-media root (optionally within a
// relative subpath), with an audio-file count for each, so the frontend can browse.
pub async fn list_local_directories(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListLocalDirectoriesQuery>,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    validate_api_key(&state, &api_key).await?;

    // Resolve the directory to list. Empty path = the local-media root (create it if it
    // does not exist yet so first-time users get an empty list rather than an error).
    let target = if query.path.trim().is_empty() {
        let root = PathBuf::from(LOCAL_MEDIA_ROOT);
        std::fs::create_dir_all(&root)
            .map_err(|e| AppError::internal(format!("Failed to access local-media root: {}", e)))?;
        root.canonicalize()
            .map_err(|_| AppError::internal("local-media root is not accessible"))?
    } else {
        validate_local_media_path(&query.path)?
    };

    if !target.is_dir() {
        return Err(AppError::bad_request("Path is not a directory"));
    }

    // Canonical root, used to compute paths relative to the local-media mount.
    let root_canonical = PathBuf::from(LOCAL_MEDIA_ROOT)
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from(LOCAL_MEDIA_ROOT));

    let mut directories: Vec<LocalDirectoryEntry> = Vec::new();

    let entries = std::fs::read_dir(&target)
        .map_err(|e| AppError::bad_request(format!("Cannot read directory: {}", e)))?;

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip the internal artwork directory
        if path.file_name().and_then(|n| n.to_str()) == Some(ARTWORK_DIR) {
            continue;
        }

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }

        // Relative path the frontend can pass straight back as directory_path
        let relative = path
            .strip_prefix(&root_canonical)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| name.clone());

        directories.push(LocalDirectoryEntry {
            name,
            path: relative,
            audio_count: count_audio_files(&path),
        });
    }

    // Alphabetical, case-insensitive
    directories.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    // Current path relative to root (empty string at the root)
    let current_path = target
        .strip_prefix(&root_canonical)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    Ok(Json(serde_json::json!({
        "current_path": current_path,
        "directories": directories,
    })))
}
