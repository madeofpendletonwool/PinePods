use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::env;
use dotenvy::dotenv;
use std::time::{SystemTime, UNIX_EPOCH};
use sha1::{Digest, Sha1};
use log::error;
use actix_cors::Cors;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::collections::{HashMap, HashSet};
use tokio::process::Command;
use chrono;

#[derive(Deserialize)]
struct SearchQuery {
    query: Option<String>,
    index: Option<String>,
    search_type: Option<String>,
}

#[derive(Deserialize)]
struct PodcastQuery {
    id: String,
}

#[derive(Deserialize)]
struct TrendingQuery {
    cat: Option<String>,
    notcat: Option<String>,
    lang: Option<String>,
    max: Option<u32>,
    since: Option<i64>,
}

#[derive(Deserialize)]
struct YouTubeChannelQuery {
    id: String,
}

// Hit counter for API usage tracking
#[derive(Clone)]
struct HitCounters {
    itunes_hits: Arc<AtomicU64>,
    podcast_index_hits: Arc<AtomicU64>,
    youtube_hits: Arc<AtomicU64>,
}

impl HitCounters {
    fn new() -> Self {
        Self {
            itunes_hits: Arc::new(AtomicU64::new(0)),
            podcast_index_hits: Arc::new(AtomicU64::new(0)),
            youtube_hits: Arc::new(AtomicU64::new(0)),
        }
    }

    fn increment_itunes(&self) {
        self.itunes_hits.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_podcast_index(&self) {
        self.podcast_index_hits.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_youtube(&self) {
        self.youtube_hits.fetch_add(1, Ordering::Relaxed);
    }

    fn get_stats(&self) -> (u64, u64, u64) {
        (
            self.itunes_hits.load(Ordering::Relaxed),
            self.podcast_index_hits.load(Ordering::Relaxed),
            self.youtube_hits.load(Ordering::Relaxed),
        )
    }
}

// Output format for YouTube channel search results
#[derive(Serialize)]
struct YouTubeSearchResult {
    results: Vec<YouTubeChannel>,
}

#[derive(Serialize)]
struct YouTubeChannel {
    #[serde(rename = "channelId")]
    channel_id: String,
    name: String,
    description: String,
    #[serde(rename = "thumbnailUrl")]
    thumbnail_url: String,
    url: String,
}

// Output format for YouTube channel details (used by rust-api's get_youtube_channel_info
// and process_youtube_channel — field names must stay stable)
#[derive(Serialize)]
struct YouTubeChannelDetails {
    #[serde(rename = "channelId")]
    channel_id: String,
    name: String,
    description: String,
    #[serde(rename = "thumbnailUrl")]
    thumbnail_url: String,
    url: String,
    #[serde(rename = "subscriberCount")]
    subscriber_count: Option<i64>,
    #[serde(rename = "videoCount")]
    video_count: Option<i64>,
    #[serde(rename = "recentVideos")]
    recent_videos: Vec<YouTubeVideo>,
}

#[derive(Serialize)]
struct YouTubeVideo {
    id: String,
    title: String,
    description: String,
    url: String,
    thumbnail: String,
    #[serde(rename = "publishedAt")]
    published_at: String,
    duration: Option<String>,
}

// Converts yt-dlp float seconds to ISO 8601 duration string (e.g. 253.0 → "PT4M13S").
// rust-api's parse_youtube_duration() expects this format.
fn seconds_to_pt_duration(seconds: f64) -> String {
    let total = seconds as u64;
    let h = total / 3600;
    let m = (total % 3600) / 60;
    let s = total % 60;
    if h > 0 {
        format!("PT{}H{}M{}S", h, m, s)
    } else if m > 0 {
        format!("PT{}M{}S", m, s)
    } else {
        format!("PT{}S", s)
    }
}

// Converts yt-dlp upload_date "YYYYMMDD" to RFC3339 "YYYY-MM-DDT00:00:00Z".
// rust-api's process_youtube_channel parses publishedAt with DateTime::parse_from_rfc3339.
fn upload_date_to_rfc3339(upload_date: &str) -> String {
    if upload_date.len() == 8 {
        format!(
            "{}-{}-{}T00:00:00Z",
            &upload_date[..4],
            &upload_date[4..6],
            &upload_date[6..8]
        )
    } else {
        chrono::Utc::now().to_rfc3339()
    }
}

async fn search_handler(
    query: web::Query<SearchQuery>,
    hit_counters: web::Data<HitCounters>,
) -> impl Responder {
    println!("search_handler called");

    if query.query.is_none() && query.index.is_none() {
        println!("Empty query and index - returning 200 OK");
        return HttpResponse::Ok().body("Test connection successful");
    }

    let search_term = query.query.clone().unwrap_or_default();
    let index = query.index.clone().unwrap_or_default().to_lowercase();
    let search_type = query.search_type.clone().unwrap_or_else(|| "term".to_string());

    println!("Received search request - Query: {}, Index: {}, Type: {}", search_term, index, search_type);
    println!("Searching for: {}", search_term);
    let client = reqwest::Client::new();
    println!("Client created");

    let response = if index == "itunes" {
        hit_counters.increment_itunes();
        let itunes_search_url = format!("https://itunes.apple.com/search?term={}&media=podcast", search_term);
        println!("Using iTunes search URL: {}", itunes_search_url);
        client.get(&itunes_search_url).send().await
    } else if index == "youtube" {
        hit_counters.increment_youtube();
        return search_youtube_channels(&search_term).await;
    } else {
        // Podcast Index API search
        hit_counters.increment_podcast_index();
        let (api_key, api_secret) = match get_api_credentials() {
            Ok(creds) => creds,
            Err(response) => return response,
        };

        let encoded_search_term = urlencoding::encode(&search_term);
        println!("Encoded search term: {}", encoded_search_term);
        println!("Search type: {}", search_type);
        let podcast_search_url = match search_type.as_str() {
            "person" => {
                println!("Using /search/byperson endpoint");
                format!("https://api.podcastindex.org/api/1.0/search/byperson?q={}", encoded_search_term)
            },
            _ => {
                println!("Using /search/byterm endpoint");
                format!("https://api.podcastindex.org/api/1.0/search/byterm?q={}", encoded_search_term)
            },
        };

        println!("Using Podcast Index search URL: {}", podcast_search_url);

        let headers = match create_auth_headers(&api_key, &api_secret) {
            Ok(h) => h,
            Err(response) => return response,
        };

        println!("Final Podcast Index URL: {}", podcast_search_url);
        client.get(&podcast_search_url).headers(headers).send().await
    };

    handle_response(response).await
}

async fn search_youtube_channels(search_term: &str) -> HttpResponse {
    println!("Searching YouTube with yt-dlp for: {}", search_term);

    let search_url = format!("ytsearch25:{}", search_term);
    let output = Command::new("yt-dlp")
        .args(&[
            "--quiet",
            "--no-warnings",
            "--flat-playlist",
            "--skip-download",
            "--dump-json",
            &search_url,
        ])
        .output()
        .await;

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to execute yt-dlp: {}", e);
            return HttpResponse::InternalServerError().body("yt-dlp not available");
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("yt-dlp search failed: {}", stderr);
        return HttpResponse::InternalServerError().body("yt-dlp search failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries: Vec<serde_json::Value> = Vec::new();
    for line in stdout.lines() {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            entries.push(entry);
        }
    }

    // First pass: collect up to 3 videos per channel
    let mut channel_videos: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
    for entry in &entries {
        let channel_id = entry.get("channel_id").and_then(|v| v.as_str())
            .or_else(|| entry.get("uploader_id").and_then(|v| v.as_str()))
            .unwrap_or("").to_string();
        if channel_id.is_empty() { continue; }
        let videos = channel_videos.entry(channel_id).or_default();
        if videos.len() < 3 {
            videos.push(entry.clone());
        }
    }

    // Second pass: build deduplicated channel list
    let mut seen_channels: HashSet<String> = HashSet::new();
    let mut channels: Vec<YouTubeChannel> = Vec::new();
    for entry in &entries {
        let channel_id = entry.get("channel_id").and_then(|v| v.as_str())
            .or_else(|| entry.get("uploader_id").and_then(|v| v.as_str()))
            .unwrap_or("").to_string();
        if channel_id.is_empty() || seen_channels.contains(&channel_id) { continue; }
        seen_channels.insert(channel_id.clone());

        let video_id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let thumbnail_url = entry.get("channel_thumbnail").and_then(|v| v.as_str())
            .or_else(|| entry.get("thumbnail").and_then(|v| v.as_str()))
            .map(|s| s.to_string())
            .or_else(|| {
                entry.get("thumbnails")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.last())
                    .and_then(|t| t.get("url"))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string())
            })
            .or_else(|| {
                if !video_id.is_empty() {
                    Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video_id))
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let name = entry.get("channel").and_then(|v| v.as_str())
            .or_else(|| entry.get("uploader").and_then(|v| v.as_str()))
            .unwrap_or("").to_string();

        channels.push(YouTubeChannel {
            channel_id: channel_id.clone(),
            name,
            description: entry.get("description").and_then(|v| v.as_str())
                .unwrap_or("").chars().take(500).collect(),
            thumbnail_url,
            url: format!("https://www.youtube.com/channel/{}", channel_id),
        });

        if channels.len() >= 25 { break; }
    }

    let result = YouTubeSearchResult { results: channels };
    match serde_json::to_string(&result) {
        Ok(json) => {
            println!("YouTube search found {} channels", result.results.len());
            HttpResponse::Ok().content_type("application/json").body(json)
        }
        Err(e) => {
            error!("Serialization error: {}", e);
            HttpResponse::InternalServerError().body("Failed to serialize response")
        }
    }
}

async fn podcast_handler(
    query: web::Query<PodcastQuery>,
    hit_counters: web::Data<HitCounters>,
) -> impl Responder {
    println!("podcast_handler called");
    hit_counters.increment_podcast_index();

    let podcast_id = &query.id;
    let client = reqwest::Client::new();

    let (api_key, api_secret) = match get_api_credentials() {
        Ok(creds) => creds,
        Err(response) => return response,
    };

    let podcast_url = format!("https://api.podcastindex.org/api/1.0/podcasts/byfeedid?id={}", podcast_id);
    println!("Using Podcast Index URL: {}", podcast_url);

    let headers = match create_auth_headers(&api_key, &api_secret) {
        Ok(h) => h,
        Err(response) => return response,
    };

    let response = client.get(&podcast_url).headers(headers).send().await;
    handle_response(response).await
}

// Proxy PodcastIndex /podcasts/trending. Powers the Discover page's category-filtered
// "trending" rows and the recommendation engine's candidate generation.
async fn trending_handler(
    query: web::Query<TrendingQuery>,
    hit_counters: web::Data<HitCounters>,
) -> impl Responder {
    println!("trending_handler called");
    hit_counters.increment_podcast_index();

    let (api_key, api_secret) = match get_api_credentials() {
        Ok(creds) => creds,
        Err(response) => return response,
    };

    let headers = match create_auth_headers(&api_key, &api_secret) {
        Ok(h) => h,
        Err(response) => return response,
    };

    // Forward only the params the caller actually supplied; reqwest handles encoding.
    let mut params: Vec<(&str, String)> = Vec::new();
    if let Some(cat) = &query.cat {
        if !cat.is_empty() {
            params.push(("cat", cat.clone()));
        }
    }
    if let Some(notcat) = &query.notcat {
        if !notcat.is_empty() {
            params.push(("notcat", notcat.clone()));
        }
    }
    if let Some(lang) = &query.lang {
        if !lang.is_empty() {
            params.push(("lang", lang.clone()));
        }
    }
    if let Some(max) = query.max {
        params.push(("max", max.to_string()));
    }
    if let Some(since) = query.since {
        params.push(("since", since.to_string()));
    }

    println!("Using Podcast Index trending URL with params: {:?}", params);

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.podcastindex.org/api/1.0/podcasts/trending")
        .query(&params)
        .headers(headers)
        .send()
        .await;
    handle_response(response).await
}

// Proxy PodcastIndex /categories/list (the canonical category taxonomy). Changes rarely;
// callers cache it. Powers the Discover page's "Browse by category" chips.
async fn categories_handler(hit_counters: web::Data<HitCounters>) -> impl Responder {
    println!("categories_handler called");
    hit_counters.increment_podcast_index();

    let (api_key, api_secret) = match get_api_credentials() {
        Ok(creds) => creds,
        Err(response) => return response,
    };

    let headers = match create_auth_headers(&api_key, &api_secret) {
        Ok(h) => h,
        Err(response) => return response,
    };

    let client = reqwest::Client::new();
    let response = client
        .get("https://api.podcastindex.org/api/1.0/categories/list")
        .headers(headers)
        .send()
        .await;
    handle_response(response).await
}

async fn youtube_channel_handler(
    query: web::Query<YouTubeChannelQuery>,
    hit_counters: web::Data<HitCounters>,
) -> impl Responder {
    println!("youtube_channel_handler called for channel: {}", query.id);
    hit_counters.increment_youtube();

    let channel_url = format!("https://www.youtube.com/channel/{}/videos", query.id);
    let output = Command::new("yt-dlp")
        .args(&[
            "--quiet",
            "--no-warnings",
            "--skip-download",
            "--dump-json",
            "--playlist-end", "15",
            &channel_url,
        ])
        .output()
        .await;

    let output = match output {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to execute yt-dlp: {}", e);
            return HttpResponse::InternalServerError().body("yt-dlp not available");
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        error!("yt-dlp channel fetch failed: {}", stderr);
        return HttpResponse::InternalServerError().body("yt-dlp channel fetch failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries: Vec<serde_json::Value> = Vec::new();
    for line in stdout.lines() {
        if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
            entries.push(entry);
        }
    }

    if entries.is_empty() {
        return HttpResponse::NotFound().body("Channel not found or has no videos");
    }

    let first = &entries[0];
    let channel_name = first.get("channel").and_then(|v| v.as_str())
        .or_else(|| first.get("uploader").and_then(|v| v.as_str()))
        .unwrap_or("").to_string();
    // channel_thumbnail is a URL string in full-metadata mode; fall back to first video thumbnail
    let first_video_id = first.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let thumbnail_url = first.get("channel_thumbnail").and_then(|v| v.as_str())
        .or_else(|| first.get("thumbnail").and_then(|v| v.as_str()))
        .map(|s| s.to_string())
        .or_else(|| {
            if !first_video_id.is_empty() {
                Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", first_video_id))
            } else {
                None
            }
        })
        .unwrap_or_default();

    let recent_videos: Vec<YouTubeVideo> = entries.iter().map(|entry| {
        let video_id = entry.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let upload_date = entry.get("upload_date").and_then(|v| v.as_str()).unwrap_or("");
        let duration_secs = entry.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let thumb = entry.get("thumbnail").and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                if !video_id.is_empty() {
                    Some(format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video_id))
                } else {
                    None
                }
            })
            .unwrap_or_default();

        YouTubeVideo {
            id: video_id.clone(),
            title: entry.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            description: entry.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            url: format!("https://www.youtube.com/watch?v={}", video_id),
            thumbnail: thumb,
            published_at: upload_date_to_rfc3339(upload_date),
            duration: Some(seconds_to_pt_duration(duration_secs)),
        }
    }).collect();

    let result = YouTubeChannelDetails {
        channel_id: query.id.clone(),
        name: channel_name.clone(),
        description: String::new(),
        thumbnail_url,
        url: format!("https://www.youtube.com/channel/{}", query.id),
        subscriber_count: None,
        video_count: None,
        recent_videos,
    };

    match serde_json::to_string(&result) {
        Ok(json) => {
            println!("Channel details for '{}': {} videos", channel_name, result.recent_videos.len());
            HttpResponse::Ok().content_type("application/json").body(json)
        }
        Err(e) => {
            error!("Serialization error: {}", e);
            HttpResponse::InternalServerError().body("Failed to serialize response")
        }
    }
}

async fn stats_handler(hit_counters: web::Data<HitCounters>) -> impl Responder {
    let (itunes, podcast_index, youtube) = hit_counters.get_stats();

    let stats = serde_json::json!({
        "api_usage": {
            "itunes_hits": itunes,
            "podcast_index_hits": podcast_index,
            "youtube_hits": youtube,
            "total_hits": itunes + podcast_index + youtube
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    });

    HttpResponse::Ok().content_type("application/json").json(stats)
}

fn get_api_credentials() -> Result<(String, String), HttpResponse> {
    let api_key = match env::var("API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("API_KEY not set in the environment");
            return Err(HttpResponse::InternalServerError().body("API_KEY not set"));
        }
    };
    let api_secret = match env::var("API_SECRET") {
        Ok(secret) => secret,
        Err(_) => {
            println!("API_SECRET not set in the environment");
            return Err(HttpResponse::InternalServerError().body("API_SECRET not set"));
        }
    };
    Ok((api_key, api_secret))
}

fn create_auth_headers(api_key: &str, api_secret: &str) -> Result<HeaderMap, HttpResponse> {
    let epoch_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();
    let data_to_hash = format!("{}{}{}", api_key, api_secret, epoch_time);

    let mut hasher = Sha1::new();
    hasher.update(data_to_hash.as_bytes());
    let sha_1 = format!("{:x}", hasher.finalize());

    let mut headers = HeaderMap::new();
    headers.insert("X-Auth-Date", HeaderValue::from_str(&epoch_time).unwrap_or_else(|e| {
        error!("Failed to insert X-Auth-Date header: {:?}", e);
        std::process::exit(1);
    }));
    headers.insert("X-Auth-Key", HeaderValue::from_str(api_key).unwrap_or_else(|e| {
        error!("Failed to insert X-Auth-Key header: {:?}", e);
        std::process::exit(1);
    }));
    headers.insert("Authorization", HeaderValue::from_str(&sha_1).unwrap_or_else(|e| {
        error!("Failed to insert Authorization header: {:?}", e);
        std::process::exit(1);
    }));
    headers.insert(USER_AGENT, HeaderValue::from_static("PodPeopleDB/1.0"));

    Ok(headers)
}

async fn handle_response(response: Result<reqwest::Response, reqwest::Error>) -> HttpResponse {
    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                println!("Request succeeded");
                match resp.text().await {
                    Ok(body) => {
                        println!("Response body: {:?}", body);
                        HttpResponse::Ok().content_type("application/json").body(body)
                    },
                    Err(_) => {
                        error!("Failed to parse response body");
                        HttpResponse::InternalServerError().body("Failed to parse response body")
                    }
                }
            } else {
                error!("Request failed with status code: {}", resp.status());
                println!("Request Headers: {:?}", resp.headers());
                HttpResponse::InternalServerError().body(format!("Request failed with status code: {}", resp.status()))
            }
        }
        Err(err) => {
            error!("Request error: {:?}", err);
            HttpResponse::InternalServerError().body("Request error occurred")
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();

    println!("Starting the Actix Web server with yt-dlp YouTube search");

    let hit_counters = web::Data::new(HitCounters::new());

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .supports_credentials()
            .max_age(3600);

        App::new()
            .app_data(hit_counters.clone())
            .wrap(cors)
            .route("/api/search", web::get().to(search_handler))
            .route("/api/podcast", web::get().to(podcast_handler))
            .route("/api/trending", web::get().to(trending_handler))
            .route("/api/categories", web::get().to(categories_handler))
            .route("/api/youtube/channel", web::get().to(youtube_channel_handler))
            .route("/api/stats", web::get().to(stats_handler))
    })
    .bind("0.0.0.0:5000")?
    .run()
    .await
}
