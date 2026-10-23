use axum::{
    extract::{Query, State},
    http::HeaderMap,
    response::Json,
};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use std::collections::{HashMap, HashSet};
use tracing::{debug, error, info, warn};

use crate::{
    error::AppError,
    handlers::{extract_api_key, require_valid_api_key},
    services::ytdlp::ytdlp_binary,
    AppState,
};

// Query struct for YouTube channel search
#[derive(Deserialize, utoipa::IntoParams)]
pub struct YouTubeSearchQuery {
    pub query: String,
    pub max_results: Option<i32>,
    pub user_id: i32,
}

// YouTube channel struct for search results - matches Python response exactly
#[derive(Serialize, Debug, utoipa::ToSchema)]
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
#[derive(Serialize, Debug, Clone, utoipa::ToSchema)]
pub struct YouTubeVideo {
    pub id: String,
    pub title: String,
    pub duration: Option<f64>,  // Note: Python uses float, not i64
    pub url: String,
}

// Query struct for YouTube subscription endpoint
#[derive(Deserialize, utoipa::IntoParams)]
pub struct YouTubeSubscribeQuery {
    pub channel_id: String,
    pub user_id: i32,
    pub feed_cutoff: Option<i32>,
}

// Query struct for check YouTube channel endpoint
#[derive(Deserialize, utoipa::IntoParams)]
pub struct CheckYouTubeChannelQuery {
    pub user_id: i32,
    pub channel_name: String,
    pub channel_url: String,
}

// Search YouTube channels - matches Python search_youtube_channels function exactly
#[utoipa::path(
    get,
    path = "/search_youtube_channels",
    tag = "youtube",
    summary = "Search youtube channels",
    params(YouTubeSearchQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn search_youtube_channels(
    State(state): State<AppState>,
    Query(query): Query<YouTubeSearchQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    require_valid_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only search for themselves  
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only search with your own account."));
    }

    let max_results = query.max_results.unwrap_or(5);
    
    // First get channel ID using a search - matches Python exactly  
    let search_url = format!("ytsearch{}:{}", max_results * 4, query.query);
    
    info!("Searching YouTube with query: {}", query.query);
    
    // Use the local yt-dlp binary to search
    let bin = ytdlp_binary();
    let output = Command::new(&bin)
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
        .map_err(|e| {
            error!("yt-dlp search: failed to execute {}: {}", bin, e);
            AppError::external_error(&format!("Failed to execute yt-dlp: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("yt-dlp search failed for '{}': {}", query.query, stderr.trim());
        return Err(AppError::external_error(&format!("yt-dlp search failed: {}", stderr.trim())));
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
                        debug!("Added video to channel {}, now has {} videos", channel_id, videos.len());
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

            info!("Creating channel {} with {} videos", channel_id, 
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

    debug!("Found {} channels", processed_results.len());
    Ok(Json(serde_json::json!({"results": processed_results})))
}


// Subscribe to YouTube channel - matches Python subscribe_to_youtube_channel function exactly
#[utoipa::path(
    post,
    path = "/youtube/subscribe",
    tag = "youtube",
    summary = "Subscribe to youtube channel",
    params(YouTubeSubscribeQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn subscribe_to_youtube_channel(
    State(state): State<AppState>,
    Query(query): Query<YouTubeSubscribeQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    require_valid_api_key(&state, &api_key).await?;

    // Check authorization - web key or user can only subscribe for themselves
    let key_id = state.db_pool.get_user_id_from_api_key(&api_key).await?;
    let is_web_key = state.db_pool.is_web_key(&api_key).await?;

    if key_id != query.user_id && !is_web_key {
        return Err(AppError::forbidden("You can only subscribe for yourself!"));
    }

    let feed_cutoff = query.feed_cutoff.unwrap_or(30);

    debug!("Starting subscription for channel {}", query.channel_id);

    // Check if channel already exists
    let existing_id = state.db_pool.check_existing_channel_subscription(
        &query.channel_id,
        query.user_id,
    ).await?;

    if let Some(podcast_id) = existing_id {
        debug!("Channel {} already subscribed", query.channel_id);
        return Ok(Json(serde_json::json!({
            "success": true,
            "podcast_id": podcast_id,
            "message": "Already subscribed to this channel"
        })));
    }

    debug!("Getting channel info");
    let channel_info = get_youtube_channel_info(&query.channel_id).await?;

    debug!("Adding channel to database");
    let podcast_id = state.db_pool.add_youtube_channel(
        &channel_info,
        query.user_id,
        feed_cutoff,
    ).await?;

    // Spawn background task to process YouTube videos
    let state_clone = state.clone();
    let channel_id_clone = query.channel_id.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::handlers::refresh::import_added_channel(&state_clone, podcast_id, &channel_id_clone, feed_cutoff).await {
            warn!("Error processing YouTube channel {}: {}", channel_id_clone, e);
        }
    });

    Ok(Json(serde_json::json!({
        "success": true,
        "podcast_id": podcast_id,
        "message": "Successfully subscribed to YouTube channel"
    })))
}

// Run the local yt-dlp binary (#793).
async fn run_ytdlp(args: &[&str]) -> Result<std::process::Output, AppError> {
    let bin = ytdlp_binary();
    Command::new(&bin).args(args).output().await.map_err(|e| {
        error!("Failed to execute yt-dlp {}: {}", bin, e);
        AppError::external_error(&format!("Failed to execute yt-dlp: {}", e))
    })
}

// yt-dlp prints one JSON object per line for --dump-json.
fn json_lines(stdout: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn video_thumbnail(entry: &serde_json::Value, video_id: &str) -> String {
    entry.get("thumbnail").and_then(|v| v.as_str()).map(str::to_string)
        .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video_id))
}

// Channel name and artwork for a new subscription, from yt-dlp's data on its newest video.
pub async fn get_youtube_channel_info(channel_id: &str) -> Result<HashMap<String, String>, AppError> {
    debug!("Getting channel info for {} via local yt-dlp", channel_id);
    let channel_url = format!("https://www.youtube.com/channel/{}/videos", channel_id);
    let output = run_ytdlp(&[
        "--quiet",
        "--no-warnings",
        "--skip-download",
        "--dump-json",
        "--playlist-end", "1",
        &channel_url,
    ]).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("yt-dlp channel fetch failed for {}: {}", channel_id, stderr.trim());
        return Err(AppError::external_error(&format!("yt-dlp channel fetch failed: {}", stderr.trim())));
    }
    let entries = json_lines(&output.stdout);
    let first = entries.first().ok_or_else(|| AppError::not_found("Channel not found or has no videos"))?;

    let name = first.get("channel").and_then(|v| v.as_str())
        .or_else(|| first.get("uploader").and_then(|v| v.as_str()))
        .unwrap_or("");
    let first_video_id = first.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let thumbnail = first.get("channel_thumbnail").and_then(|v| v.as_str()).map(str::to_string)
        .unwrap_or_else(|| video_thumbnail(first, first_video_id));

    let mut channel_info = HashMap::new();
    channel_info.insert("channel_id".to_string(), channel_id.to_string());
    channel_info.insert("name".to_string(), name.to_string());
    channel_info.insert("description".to_string(), String::new());
    channel_info.insert("thumbnail_url".to_string(), thumbnail);
    debug!("Extracted channel info for: {}", name);
    Ok(channel_info)
}


// Helper function to get MP3 duration from file
pub fn get_mp3_duration(file_path: &str) -> Option<i32> {
    match mp3_metadata::read_from_file(file_path) {
        Ok(metadata) => Some(metadata.duration.as_secs() as i32),
        Err(e) => {
            warn!("Failed to read MP3 metadata from {}: {}", file_path, e);
            None
        }
    }
}

/// The channel ID in a YouTube channel podcast's feed URL (`https://www.youtube.com/channel/<id>`).
pub fn channel_id_from_url(feed_url: &str) -> &str {
    match feed_url.split_once("channel/") {
        Some((_, rest)) => rest.split(['/', '?']).next().unwrap_or(rest),
        None => feed_url,
    }
}

/// One of a channel's newest videos.
#[derive(Debug, Clone)]
pub struct ListedVideo {
    pub id: String,
    pub title: Option<String>,
    /// Unknown when the list came from yt-dlp's flat listing.
    pub published: Option<chrono::DateTime<chrono::Utc>>,
}

/// What a look at a channel found (see `list_channel_videos`).
pub enum ChannelListing {
    /// The feed hasn't changed since its subscribers last applied it.
    Unchanged { etag: Option<String>, last_modified: Option<String> },
    /// The newest videos, with the validators and body hash to store once they're applied (no
    /// hash when the list came from yt-dlp).
    Listed {
        videos: Vec<ListedVideo>,
        etag: Option<String>,
        last_modified: Option<String>,
        content_hash: Option<String>,
    },
    /// 429/503 from YouTube.
    Throttled { status: u16, retry_after: Option<std::time::Duration> },
}

/// A channel's newest videos from its RSS feed (conditional GET, body hash), or from yt-dlp's flat
/// listing when the feed can't be read. Err when neither works, with the feed's error.
pub async fn list_channel_videos(
    state: &AppState,
    channel_id: &str,
    prev: crate::database::FeedValidators<'_>,
) -> Result<ChannelListing, AppError> {
    use crate::database::{FeedFetch, FeedValidators};
    let url = state.config.api.youtube_feed_url.replace("{channel_id}", channel_id);
    // The body hash is our own (see channel_feed_hash), so it's compared here.
    let validators = FeedValidators { content_hash: None, ..prev };
    let error = match state.db_pool.fetch_feed_conditional(&url, None, None, validators).await {
        Ok(FeedFetch::NotModified { etag, last_modified, .. }) => {
            return Ok(ChannelListing::Unchanged { etag, last_modified });
        }
        Ok(FeedFetch::Fetched { body, etag, last_modified, .. }) => {
            let hash = channel_feed_hash(&body);
            if prev.content_hash == Some(hash.as_str()) {
                debug!("Channel feed unchanged (hash match): {}", channel_id);
                return Ok(ChannelListing::Unchanged { etag, last_modified });
            }
            match parse_channel_feed(&body) {
                Ok(videos) => {
                    return Ok(ChannelListing::Listed { videos, etag, last_modified, content_hash: Some(hash) });
                }
                Err(e) => e,
            }
        }
        Ok(FeedFetch::Throttled { status, retry_after }) => {
            return Ok(ChannelListing::Throttled { status, retry_after });
        }
        Err(e) => e,
    };
    warn!("Feed of YouTube channel {} failed ({}), listing it with yt-dlp", channel_id, error);
    match list_channel_videos_flat(channel_id).await {
        Ok(videos) => Ok(ChannelListing::Listed { videos, etag: None, last_modified: None, content_hash: None }),
        Err(e) => {
            warn!("yt-dlp listing of YouTube channel {} failed too: {}", channel_id, e);
            Err(error)
        }
    }
}

/// Hash of a channel feed without its view counts and ratings, which change on every fetch.
fn channel_feed_hash(body: &str) -> String {
    const OPEN: &str = "<media:community>";
    const CLOSE: &str = "</media:community>";
    let mut stable = String::with_capacity(body.len());
    let mut rest = body;
    while let Some(start) = rest.find(OPEN) {
        stable.push_str(&rest[..start]);
        match rest[start..].find(CLOSE) {
            Some(end) => rest = &rest[start + end + CLOSE.len()..],
            None => {
                rest = "";
                break;
            }
        }
    }
    stable.push_str(rest);
    crate::services::http::content_hash(&stable)
}

/// The videos in a channel's Atom feed, newest first.
fn parse_channel_feed(body: &str) -> Result<Vec<ListedVideo>, AppError> {
    let feed = feed_rs::parser::parse(body.as_bytes())
        .map_err(|e| AppError::external_error(&format!("Not a valid feed: {}", e)))?;
    Ok(feed
        .entries
        .iter()
        .filter_map(|entry| {
            let id = entry.id.strip_prefix("yt:video:")?;
            Some(ListedVideo {
                id: id.to_string(),
                title: entry.title.as_ref().map(|t| t.content.clone()),
                published: entry.published.or(entry.updated),
            })
        })
        .collect())
}

/// A channel's 15 newest videos from yt-dlp, without fetching each one.
async fn list_channel_videos_flat(channel_id: &str) -> Result<Vec<ListedVideo>, AppError> {
    let channel_url = format!("https://www.youtube.com/channel/{}/videos", channel_id);
    let _permit = crate::services::ytdlp::refresh_permit().await;
    let output = run_ytdlp(&[
        "--quiet",
        "--no-warnings",
        "--flat-playlist",
        "--dump-json",
        "--playlist-end", "15",
        &channel_url,
    ]).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AppError::external_error(&format!("yt-dlp channel listing failed: {}", stderr.trim())));
    }
    Ok(json_lines(&output.stdout)
        .iter()
        .filter_map(|entry| {
            let id = entry.get("id").and_then(|v| v.as_str())?;
            let title = entry.get("title").and_then(|v| v.as_str()).map(str::to_string);
            Some(ListedVideo { id: id.to_string(), title, published: None })
        })
        .collect())
}

/// A video's details from `yt-dlp --dump-json`.
#[derive(Debug, Clone)]
pub struct VideoDetails {
    pub id: String,
    pub title: String,
    pub description: String,
    pub thumbnail: String,
    pub published: chrono::DateTime<chrono::Utc>,
    /// Seconds.
    pub duration: i32,
    pub channel: String,
}

impl VideoDetails {
    fn from_json(entry: &serde_json::Value) -> Option<VideoDetails> {
        let id = entry.get("id").and_then(|v| v.as_str())?.to_string();
        let text = |key: &str| entry.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let published = entry
            .get("timestamp")
            .and_then(|v| v.as_i64())
            .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0))
            .or_else(|| {
                let date = chrono::NaiveDate::parse_from_str(entry.get("upload_date")?.as_str()?, "%Y%m%d").ok()?;
                Some(date.and_hms_opt(0, 0, 0)?.and_utc())
            })
            .unwrap_or_else(chrono::Utc::now);
        Some(VideoDetails {
            thumbnail: video_thumbnail(entry, &id),
            title: text("title"),
            description: text("description"),
            published,
            duration: entry.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32,
            channel: entry.get("channel").or_else(|| entry.get("uploader")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
            id,
        })
    }
}

// Details of recently fetched videos, so a video a subscriber's filter turned down isn't fetched
// again each time its channel's feed changes.
const DETAILS_CACHE_SIZE: usize = 10_000;
static DETAILS_CACHE: std::sync::LazyLock<std::sync::Mutex<HashMap<String, VideoDetails>>> =
    std::sync::LazyLock::new(Default::default);

/// Details of these videos from a single yt-dlp run. Videos yt-dlp can't read (members-only,
/// private, upcoming premieres) are left out; Err only when yt-dlp itself failed.
async fn fetch_video_details(ids: &[String]) -> Result<HashMap<String, VideoDetails>, AppError> {
    let mut details: HashMap<String, VideoDetails> = {
        let cache = DETAILS_CACHE.lock().unwrap();
        ids.iter().filter_map(|id| cache.get(id).map(|d| (id.clone(), d.clone()))).collect()
    };
    let ids: Vec<&String> = ids.iter().filter(|id| !details.contains_key(*id)).collect();
    if ids.is_empty() {
        return Ok(details);
    }
    let urls: Vec<String> = ids.iter().map(|id| format!("https://www.youtube.com/watch?v={}", id)).collect();
    let mut args = vec!["--quiet", "--no-warnings", "--skip-download", "--ignore-errors", "--socket-timeout", "30", "--dump-json"];
    args.extend(urls.iter().map(String::as_str));
    let _permit = crate::services::ytdlp::refresh_permit().await;
    let output = run_ytdlp(&args).await?;
    let fetched: Vec<VideoDetails> = json_lines(&output.stdout).iter().filter_map(VideoDetails::from_json).collect();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // A failing video makes yt-dlp exit non-zero; only a run with no per-video errors to show for
    // it is a broken yt-dlp.
    if !output.status.success() && fetched.is_empty() && !stderr.contains("ERROR: [youtube]") {
        return Err(AppError::external_error(&format!("yt-dlp video fetch failed: {}", stderr.trim())));
    }
    if fetched.len() < ids.len() {
        debug!("yt-dlp skipped {} of {} videos: {}", ids.len() - fetched.len(), ids.len(), stderr.trim());
    }
    {
        let mut cache = DETAILS_CACHE.lock().unwrap();
        if cache.len() + fetched.len() > DETAILS_CACHE_SIZE {
            cache.clear();
        }
        cache.extend(fetched.iter().map(|d| (d.id.clone(), d.clone())));
    }
    details.extend(fetched.into_iter().map(|d| (d.id.clone(), d)));
    Ok(details)
}

/// Which new videos a channel subscription adds. `None` means no limit.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq, utoipa::ToSchema)]
pub struct YouTubeVideoFilter {
    /// Shortest video, in minutes.
    pub min_minutes: Option<i32>,
    /// Longest video, in minutes.
    pub max_minutes: Option<i32>,
    /// Text the title must contain (any case).
    pub title_contains: Option<String>,
}

impl YouTubeVideoFilter {
    /// Zeros and blank text mean no limit; Err for values that can't be right.
    pub fn normalized(self) -> Result<YouTubeVideoFilter, AppError> {
        let minutes = |m: Option<i32>| match m {
            Some(m) if m < 0 => Err(AppError::bad_request("Video lengths can't be negative")),
            Some(0) | None => Ok(None),
            Some(m) => Ok(Some(m)),
        };
        let (min_minutes, max_minutes) = (minutes(self.min_minutes)?, minutes(self.max_minutes)?);
        if let (Some(min), Some(max)) = (min_minutes, max_minutes) {
            if min > max {
                return Err(AppError::bad_request("The shortest length is longer than the longest"));
            }
        }
        let title_contains = self.title_contains.map(|t| t.trim().to_string()).filter(|t| !t.is_empty());
        if title_contains.as_ref().is_some_and(|t| t.chars().count() > 255) {
            return Err(AppError::bad_request("The title filter is too long (255 characters at most)"));
        }
        Ok(YouTubeVideoFilter { min_minutes, max_minutes, title_contains })
    }

    pub fn title_ok(&self, title: &str) -> bool {
        self.title_contains.as_ref().map_or(true, |t| title.to_lowercase().contains(&t.to_lowercase()))
    }

    /// A video with no known length only passes when no length limit is set.
    pub fn length_ok(&self, seconds: i32) -> bool {
        if self.min_minutes.is_none() && self.max_minutes.is_none() {
            return true;
        }
        seconds > 0
            && self.min_minutes.map_or(true, |m| seconds >= m * 60)
            && self.max_minutes.map_or(true, |m| seconds <= m * 60)
    }
}

/// A channel subscription brought up to date by `process_youtube_channel`.
pub struct ChannelSubscriber {
    pub podcast_id: i32,
    /// Days of videos to keep; 0 or less keeps everything the channel lists.
    pub feed_cutoff: i32,
    pub filter: YouTubeVideoFilter,
}

/// Bring a channel's subscriptions up to date with its newest videos: drop videos past each
/// subscriber's cutoff, fetch the details of the listed videos a subscriber is missing and would
/// take (one yt-dlp run, never past the longest cutoff), insert the ones its filter lets through,
/// and download their audio. Returns the inserted rows.
pub async fn process_youtube_channel(
    state: &AppState,
    subscribers: &[ChannelSubscriber],
    listed: &[ListedVideo],
) -> Result<Vec<crate::handlers::podcasts::Episode>, AppError> {
    let now = chrono::Utc::now();
    let cutoff = |sub: &ChannelSubscriber| (sub.feed_cutoff > 0).then(|| now - chrono::Duration::days(sub.feed_cutoff as i64));
    let in_cutoff = |sub: &ChannelSubscriber, published| cutoff(sub).map_or(true, |c| published > c);

    let mut have: Vec<HashSet<String>> = Vec::with_capacity(subscribers.len());
    for sub in subscribers {
        if let Some(cutoff) = cutoff(sub) {
            state.db_pool.remove_old_youtube_videos(sub.podcast_id, cutoff).await?;
        }
        have.push(state.db_pool.youtube_video_ids(sub.podcast_id).await?);
    }
    let missing = |video: &ListedVideo, sub: &ChannelSubscriber, have: &HashSet<String>| {
        !have.contains(&video.id)
            && video.published.map_or(true, |p| in_cutoff(sub, p))
            && video.title.as_deref().map_or(true, |t| sub.filter.title_ok(t))
    };
    let wanted: Vec<String> = listed
        .iter()
        .filter(|v| subscribers.iter().zip(&have).any(|(sub, have)| missing(v, sub, have)))
        .map(|v| v.id.clone())
        .collect();
    let details = if wanted.is_empty() { HashMap::new() } else { fetch_video_details(&wanted).await? };

    let mut new_episodes = Vec::new();
    let mut stored: HashSet<String> = have.iter().flatten().cloned().collect();
    for (sub, have) in subscribers.iter().zip(&have) {
        let videos: Vec<&VideoDetails> = wanted
            .iter()
            .filter(|id| !have.contains(*id))
            .filter_map(|id| details.get(id))
            .filter(|d| in_cutoff(sub, d.published) && sub.filter.title_ok(&d.title) && sub.filter.length_ok(d.duration))
            .collect();
        if let Some(first) = videos.first() {
            heal_channel_metadata(state, sub.podcast_id, first).await;
            let inserted = state.db_pool.add_youtube_videos(sub.podcast_id, &videos).await?;
            debug!("Podcast {}: {} new videos", sub.podcast_id, inserted.len());
            stored.extend(videos.iter().map(|d| d.id.clone()));
            new_episodes.extend(inserted);
        }
        state.db_pool.update_episode_count(sub.podcast_id).await?;
    }

    // Audio of the listed videos a subscriber keeps (so never past the longest cutoff).
    let keep: Vec<&str> = listed.iter().map(|v| v.id.as_str()).filter(|id| stored.contains(*id)).collect();
    download_missing_audio(state, &keep).await;

    Ok(new_episodes)
}

// Fill in a podcast's name/artwork if they were empty from a broken initial subscription.
async fn heal_channel_metadata(state: &AppState, podcast_id: i32, video: &VideoDetails) {
    if video.channel.is_empty() && video.thumbnail.is_empty() {
        return;
    }
    let result = match &state.db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            sqlx::query(r#"
                UPDATE "Podcasts"
                SET podcastname = CASE WHEN podcastname = '' OR podcastname IS NULL THEN $2 ELSE podcastname END,
                    artworkurl   = CASE WHEN artworkurl  = '' OR artworkurl  IS NULL THEN $3 ELSE artworkurl  END,
                    author       = CASE WHEN author      = '' OR author      IS NULL THEN $2 ELSE author      END
                WHERE podcastid = $1
            "#)
            .bind(podcast_id)
            .bind(&video.channel)
            .bind(&video.thumbnail)
            .execute(pool)
            .await
            .map(|r| r.rows_affected())
        }
        crate::database::DatabasePool::MySQL(pool) => {
            sqlx::query(r#"
                UPDATE Podcasts
                SET PodcastName = CASE WHEN PodcastName = '' OR PodcastName IS NULL THEN ? ELSE PodcastName END,
                    ArtworkURL  = CASE WHEN ArtworkURL  = '' OR ArtworkURL  IS NULL THEN ? ELSE ArtworkURL  END,
                    Author      = CASE WHEN Author      = '' OR Author      IS NULL THEN ? ELSE Author      END
                WHERE PodcastID = ?
            "#)
            .bind(&video.channel)
            .bind(&video.thumbnail)
            .bind(&video.channel)
            .bind(podcast_id)
            .execute(pool)
            .await
            .map(|r| r.rows_affected())
        }
    };
    if let Err(e) = result {
        warn!("Updating metadata of YouTube podcast {} failed: {}", podcast_id, e);
    }
}

// Download the audio of these videos where it isn't on disk yet.
async fn download_missing_audio(state: &AppState, video_ids: &[&str]) {
    let (mut downloaded, mut failed) = (0, 0);
    for video_id in video_ids {
        let output_path = format!("/opt/pinepods/downloads/youtube/{}.mp3", video_id);
        let output_path_double = format!("{}.mp3", output_path);
        if tokio::fs::metadata(&output_path).await.is_ok() || tokio::fs::metadata(&output_path_double).await.is_ok() {
            continue;
        }
        debug!("Downloading audio of video {} to {}", video_id, output_path);
        let result = {
            let _permit = crate::services::ytdlp::refresh_permit().await;
            download_youtube_audio(video_id, &output_path).await
        };
        match result {
            Ok(_) => {
                downloaded += 1;
                match get_mp3_duration(&output_path) {
                    Some(duration) => {
                        if let Err(e) = state.db_pool.update_youtube_video_duration(video_id, duration).await {
                            warn!("Failed to update duration for video {}: {}", video_id, e);
                        }
                    }
                    None => debug!("Could not read duration from MP3 file: {}", output_path),
                }
            }
            Err(e) => {
                failed += 1;
                let error_msg = e.to_string().to_lowercase();
                if ["members-only", "private", "unavailable"].iter().any(|w| error_msg.contains(w)) {
                    debug!("Skipping video {}: {}", video_id, e);
                } else {
                    warn!("Failed to download video {}: {}", video_id, e);
                }
            }
        }
    }
    if downloaded + failed > 0 {
        debug!("YouTube audio downloads: {} successful, {} failed", downloaded, failed);
    }
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

    let bin = ytdlp_binary();
    let output = Command::new(&bin)
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
        .map_err(|e| {
            error!("yt-dlp download: failed to execute {} for {}: {}", bin, video_id, e);
            AppError::external_error(&format!("Failed to execute yt-dlp: {}", e))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("yt-dlp download failed for {}: {}", video_id, stderr.trim());
        return Err(AppError::external_error(&format!("yt-dlp download failed: {}", stderr.trim())));
    }

    Ok(())
}

#[derive(Deserialize, utoipa::IntoParams)]
pub struct YouTubeVideoFilterQuery {
    pub podcast_id: i32,
    pub user_id: i32,
}

// The video filter of a YouTube channel subscription.
#[utoipa::path(
    get,
    path = "/youtube_video_filter",
    tag = "youtube",
    summary = "Get a channel's video filter",
    params(YouTubeVideoFilterQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "The filter; unset fields mean no limit", body = YouTubeVideoFilter),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not your podcast"),
        (status = 404, description = "No such YouTube channel"),
    ),
)]
pub async fn get_youtube_video_filter(
    State(state): State<AppState>,
    Query(query): Query<YouTubeVideoFilterQuery>,
    headers: HeaderMap,
) -> Result<Json<YouTubeVideoFilter>, AppError> {
    let api_key = extract_api_key(&headers)?;
    require_valid_api_key(&state, &api_key).await?;
    if !crate::handlers::check_user_access(&state, &api_key, query.user_id).await? {
        return Err(AppError::forbidden("You can only see settings of your own podcasts!"));
    }
    if !state.db_pool.owns_youtube_channel(query.podcast_id, query.user_id).await? {
        return Err(AppError::not_found("YouTube channel not found"));
    }
    let mut filters = state.db_pool.youtube_video_filters(&[query.podcast_id]).await?;
    Ok(Json(filters.remove(&query.podcast_id).unwrap_or_default()))
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct UpdateYouTubeVideoFilterRequest {
    pub podcast_id: i32,
    pub user_id: i32,
    /// Shortest video to add, in minutes; empty or 0 for no limit.
    pub min_minutes: Option<i32>,
    /// Longest video to add, in minutes; empty or 0 for no limit.
    pub max_minutes: Option<i32>,
    /// Only add videos whose title contains this (any case); empty for all.
    pub title_contains: Option<String>,
}

// Set which new videos a YouTube channel subscription adds. Videos already added stay.
#[utoipa::path(
    post,
    path = "/update_youtube_video_filter",
    tag = "youtube",
    summary = "Set a channel's video filter",
    request_body = UpdateYouTubeVideoFilterRequest,
    security(("api_key" = [])),
    responses(
        (status = 200, description = "The filter as stored", body = YouTubeVideoFilter),
        (status = 400, description = "Negative lengths, shortest longer than longest, or title filter too long"),
        (status = 401, description = "Invalid or missing API key"),
        (status = 403, description = "Not your podcast"),
        (status = 404, description = "No such YouTube channel"),
    ),
)]
pub async fn update_youtube_video_filter(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdateYouTubeVideoFilterRequest>,
) -> Result<Json<YouTubeVideoFilter>, AppError> {
    let api_key = extract_api_key(&headers)?;
    require_valid_api_key(&state, &api_key).await?;
    if !crate::handlers::check_user_access(&state, &api_key, request.user_id).await? {
        return Err(AppError::forbidden("You can only modify settings of your own podcasts!"));
    }
    let filter = YouTubeVideoFilter {
        min_minutes: request.min_minutes,
        max_minutes: request.max_minutes,
        title_contains: request.title_contains,
    }
    .normalized()?;
    if !state.db_pool.set_youtube_video_filter(request.podcast_id, request.user_id, &filter).await? {
        return Err(AppError::not_found("YouTube channel not found"));
    }
    Ok(Json(filter))
}

// Check if YouTube channel exists - matches Python api_check_youtube_channel function exactly
#[utoipa::path(
    get,
    path = "/check_youtube_channel",
    tag = "youtube",
    summary = "Check youtube channel",
    params(CheckYouTubeChannelQuery),
    security(("api_key" = [])),
    responses(
        (status = 200, description = "Success", body = serde_json::Value),
        (status = 401, description = "Invalid or missing API key"),
    ),
)]
pub async fn check_youtube_channel(
    State(state): State<AppState>,
    Query(query): Query<CheckYouTubeChannelQuery>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AppError> {
    let api_key = extract_api_key(&headers)?;
    require_valid_api_key(&state, &api_key).await?;

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
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_ids_from_feed_urls() {
        assert_eq!(channel_id_from_url("https://www.youtube.com/channel/UCabc"), "UCabc");
        assert_eq!(channel_id_from_url("https://www.youtube.com/channel/UCabc/videos?x=1"), "UCabc");
        assert_eq!(channel_id_from_url("UCabc"), "UCabc");
    }

    #[test]
    fn view_counts_dont_change_the_feed_hash() {
        let feed = |views: u32, title: &str| {
            format!("<entry><title>{title}</title><media:community><media:statistics views=\"{views}\"/></media:community></entry>")
        };
        assert_eq!(channel_feed_hash(&feed(1, "A")), channel_feed_hash(&feed(999, "A")));
        assert_ne!(channel_feed_hash(&feed(1, "A")), channel_feed_hash(&feed(1, "B")));
    }

    #[test]
    fn video_filters() {
        let any = YouTubeVideoFilter::default();
        assert!(any.length_ok(0) && any.length_ok(30) && any.title_ok("anything"));

        let long = YouTubeVideoFilter { min_minutes: Some(45), ..Default::default() };
        assert!(long.length_ok(45 * 60) && !long.length_ok(45 * 60 - 1));
        assert!(!long.length_ok(0), "unknown length doesn't pass a length limit");
        let short = YouTubeVideoFilter { max_minutes: Some(120), ..Default::default() };
        assert!(short.length_ok(120 * 60) && !short.length_ok(120 * 60 + 1));

        let titled = YouTubeVideoFilter { title_contains: Some("The Standup".into()), ..Default::default() };
        assert!(titled.title_ok("Monday: THE STANDUP #12") && !titled.title_ok("Shorts"));
    }

    #[test]
    fn video_filter_normalization() {
        let f = |min, max, title: &str| YouTubeVideoFilter { min_minutes: min, max_minutes: max, title_contains: Some(title.into()) };
        assert_eq!(f(Some(0), Some(0), "  ").normalized().unwrap(), YouTubeVideoFilter::default());
        assert_eq!(f(Some(5), None, " Pod ").normalized().unwrap().title_contains.as_deref(), Some("Pod"));
        assert!(f(Some(-1), None, "").normalized().is_err());
        assert!(f(Some(60), Some(30), "").normalized().is_err());
        assert!(f(None, None, &"x".repeat(256)).normalized().is_err());
    }

    #[test]
    fn video_ids_from_a_channel_feed() {
        let body = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns="http://www.w3.org/2005/Atom">
<id>yt:channel:UCabc</id><title>Chan</title>
<entry><id>yt:video:vid1</id><yt:videoId>vid1</yt:videoId><title>One</title><published>2026-10-20T10:00:00+00:00</published></entry>
</feed>"#;
        let videos = parse_channel_feed(body).unwrap();
        assert_eq!(videos.len(), 1);
        assert_eq!(videos[0].id, "vid1");
        assert_eq!(videos[0].title.as_deref(), Some("One"));
        assert_eq!(videos[0].published.unwrap().to_rfc3339(), "2026-10-20T10:00:00+00:00");
        assert!(parse_channel_feed("<html>nope</html>").is_err());
    }
}
