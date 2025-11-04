use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};
use serde::{Deserialize, Serialize};
use std::env;
use dotenvy::dotenv;
use std::time::{SystemTime, UNIX_EPOCH};
use sha1::{Digest, Sha1};
use log::{info, error};
use actix_cors::Cors;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
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
struct YouTubeChannelQuery {
    id: String,
}

#[derive(Deserialize)]
struct TrendingQuery {
    max: Option<u32>,
    since: Option<i64>,
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

// YouTube API response structures for search
#[derive(Deserialize, Serialize)]
struct YouTubeSearchResponse {
    items: Vec<YouTubeChannelResult>,
}

#[derive(Deserialize, Serialize)]
struct YouTubeChannelResult {
    id: YouTubeChannelId,
    snippet: YouTubeChannelSnippet,
}

#[derive(Deserialize, Serialize)]
struct YouTubeChannelId {
    #[serde(rename = "channelId")]
    channel_id: String,
}

#[derive(Deserialize, Serialize)]
struct YouTubeChannelSnippet {
    title: String,
    description: String,
    thumbnails: YouTubeThumbnails,
    #[serde(rename = "channelTitle")]
    channel_title: Option<String>,
}

#[derive(Deserialize, Serialize)]
struct YouTubeThumbnails {
    default: Option<YouTubeThumbnail>,
    medium: Option<YouTubeThumbnail>,
    high: Option<YouTubeThumbnail>,
}

#[derive(Deserialize, Serialize)]
struct YouTubeThumbnail {
    url: String,
}

// YouTube API response structures for channel details
#[derive(Deserialize)]
struct YouTubeChannelDetailsResponse {
    items: Vec<YouTubeChannelDetailsItem>,
}

#[derive(Deserialize)]
struct YouTubeChannelDetailsItem {
    snippet: YouTubeChannelDetailsSnippet,
    statistics: Option<YouTubeChannelStatistics>,
}

#[derive(Deserialize)]
struct YouTubeChannelDetailsSnippet {
    title: String,
    description: String,
    thumbnails: YouTubeThumbnails,
}

#[derive(Deserialize)]
struct YouTubeChannelStatistics {
    #[serde(rename = "subscriberCount")]
    subscriber_count: Option<String>,
    #[serde(rename = "videoCount")]
    video_count: Option<String>,
}

// YouTube API response structures for channel videos
#[derive(Deserialize)]
struct YouTubeVideosResponse {
    items: Vec<YouTubeVideoItem>,
}

#[derive(Deserialize)]
struct YouTubeVideoItem {
    id: YouTubeVideoId,
    snippet: YouTubeVideoSnippet,
    #[serde(rename = "contentDetails")]
    content_details: Option<YouTubeVideoContentDetails>,
}

#[derive(Deserialize)]
struct YouTubeVideoId {
    #[serde(rename = "videoId")]
    video_id: String,
}

#[derive(Deserialize)]
struct YouTubeVideoSnippet {
    title: String,
    description: String,
    thumbnails: YouTubeThumbnails,
    #[serde(rename = "publishedAt")]
    published_at: String,
}

#[derive(Deserialize)]
struct YouTubeVideoContentDetails {
    duration: Option<String>,
}

// Simplified response format to match other APIs
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

// YouTube channel details response (when user clicks a channel)
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
        // iTunes Search
        hit_counters.increment_itunes();
        let itunes_search_url = format!("https://itunes.apple.com/search?term={}&media=podcast", search_term);
        println!("Using iTunes search URL: {}", itunes_search_url);

        client.get(&itunes_search_url).send().await
    } else if index == "youtube" {
        // YouTube Data API v3 Search
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
    println!("Searching YouTube for: {}", search_term);
    
    let youtube_api_key = match env::var("YOUTUBE_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            error!("YOUTUBE_API_KEY not set in the environment");
            return HttpResponse::InternalServerError().body("YouTube API key not configured");
        }
    };

    let client = reqwest::Client::new();
    let encoded_search_term = urlencoding::encode(search_term);
    
    // YouTube Data API v3 search for channels
    let youtube_search_url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&type=channel&q={}&maxResults=25&key={}",
        encoded_search_term, youtube_api_key
    );
    
    println!("Using YouTube search URL: {}", youtube_search_url);

    match client.get(&youtube_search_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<YouTubeSearchResponse>().await {
                    Ok(youtube_response) => {
                        // Convert YouTube response to our format
                        let channels: Vec<YouTubeChannel> = youtube_response.items.into_iter().map(|item| {
                            let thumbnail_url = item.snippet.thumbnails.high
                                .or(item.snippet.thumbnails.medium)
                                .or(item.snippet.thumbnails.default)
                                .map(|thumb| thumb.url)
                                .unwrap_or_default();

                            YouTubeChannel {
                                channel_id: item.id.channel_id.clone(),
                                name: item.snippet.title,
                                description: item.snippet.description,
                                thumbnail_url,
                                url: format!("https://www.youtube.com/channel/{}", item.id.channel_id),
                            }
                        }).collect();

                        let result = YouTubeSearchResult { results: channels };
                        
                        match serde_json::to_string(&result) {
                            Ok(json_response) => {
                                println!("YouTube search successful, found {} channels", result.results.len());
                                HttpResponse::Ok().content_type("application/json").body(json_response)
                            }
                            Err(e) => {
                                error!("Failed to serialize YouTube response: {}", e);
                                HttpResponse::InternalServerError().body("Failed to process YouTube response")
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse YouTube API response: {}", e);
                        HttpResponse::InternalServerError().body("Failed to parse YouTube response")
                    }
                }
            } else {
                error!("YouTube API request failed with status: {}", resp.status());
                HttpResponse::InternalServerError().body(format!("YouTube API error: {}", resp.status()))
            }
        }
        Err(e) => {
            error!("YouTube API request error: {}", e);
            HttpResponse::InternalServerError().body("YouTube API request failed")
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

async fn trending_handler(
    query: web::Query<TrendingQuery>,
    hit_counters: web::Data<HitCounters>,
) -> impl Responder {
    println!("trending_handler called");
    hit_counters.increment_podcast_index();

    let client = reqwest::Client::new();

    let (api_key, api_secret) = match get_api_credentials() {
        Ok(creds) => creds,
        Err(response) => return response,
    };

    // Build trending URL with optional parameters
    let mut trending_url = String::from("https://api.podcastindex.org/api/1.0/podcasts/trending");
    let mut params = Vec::new();

    if let Some(max) = query.max {
        params.push(format!("max={}", max));
    }
    if let Some(since) = query.since {
        params.push(format!("since={}", since));
    }

    if !params.is_empty() {
        trending_url.push('?');
        trending_url.push_str(&params.join("&"));
    }

    println!("Using Podcast Index trending URL: {}", trending_url);

    let headers = match create_auth_headers(&api_key, &api_secret) {
        Ok(h) => h,
        Err(response) => return response,
    };

    let response = client.get(&trending_url).headers(headers).send().await;
    handle_response(response).await
}

async fn youtube_channel_handler(
    query: web::Query<YouTubeChannelQuery>,
    hit_counters: web::Data<HitCounters>,
) -> impl Responder {
    println!("youtube_channel_handler called for channel: {}", query.id);
    hit_counters.increment_youtube();
    
    let youtube_api_key = match env::var("YOUTUBE_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            error!("YOUTUBE_API_KEY not set in the environment");
            return HttpResponse::InternalServerError().body("YouTube API key not configured");
        }
    };

    let client = reqwest::Client::new();
    let channel_id = &query.id;
    
    // Step 1: Get channel details and statistics
    let channel_details_url = format!(
        "https://www.googleapis.com/youtube/v3/channels?part=snippet,statistics&id={}&key={}",
        channel_id, youtube_api_key
    );
    
    println!("Fetching channel details: {}", channel_details_url);

    let channel_details = match client.get(&channel_details_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<YouTubeChannelDetailsResponse>().await {
                    Ok(details) => {
                        if details.items.is_empty() {
                            return HttpResponse::NotFound().body("Channel not found");
                        }
                        details.items.into_iter().next().unwrap()
                    }
                    Err(e) => {
                        error!("Failed to parse channel details: {}", e);
                        return HttpResponse::InternalServerError().body("Failed to parse channel details");
                    }
                }
            } else {
                error!("Channel details request failed with status: {}", resp.status());
                return HttpResponse::InternalServerError().body(format!("YouTube API error: {}", resp.status()));
            }
        }
        Err(e) => {
            error!("Channel details request error: {}", e);
            return HttpResponse::InternalServerError().body("YouTube API request failed");
        }
    };
    
    // Step 2: Get recent videos from the channel
    let videos_url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&channelId={}&type=video&order=date&maxResults=10&key={}",
        channel_id, youtube_api_key
    );
    
    println!("Fetching recent videos: {}", videos_url);

    let videos = match client.get(&videos_url).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                match resp.json::<YouTubeVideosResponse>().await {
                    Ok(videos_response) => {
                        videos_response.items.into_iter().map(|item| {
                            let thumbnail_url = item.snippet.thumbnails.medium
                                .or(item.snippet.thumbnails.high)
                                .or(item.snippet.thumbnails.default)
                                .map(|thumb| thumb.url)
                                .unwrap_or_default();

                            YouTubeVideo {
                                id: item.id.video_id.clone(),
                                title: item.snippet.title,
                                description: item.snippet.description,
                                url: format!("https://www.youtube.com/watch?v={}", item.id.video_id),
                                thumbnail: thumbnail_url,
                                published_at: item.snippet.published_at,
                                duration: item.content_details.and_then(|cd| cd.duration),
                            }
                        }).collect()
                    }
                    Err(e) => {
                        error!("Failed to parse videos response: {}", e);
                        return HttpResponse::InternalServerError().body("Failed to parse videos");
                    }
                }
            } else {
                error!("Videos request failed with status: {}", resp.status());
                // Continue without videos rather than failing completely
                Vec::new()
            }
        }
        Err(e) => {
            error!("Videos request error: {}", e);
            // Continue without videos rather than failing completely
            Vec::new()
        }
    };

    // Extract thumbnail URL from channel details
    let thumbnail_url = channel_details.snippet.thumbnails.high
        .or(channel_details.snippet.thumbnails.medium)
        .or(channel_details.snippet.thumbnails.default)
        .map(|thumb| thumb.url)
        .unwrap_or_default();

    // Parse subscriber and video counts
    let subscriber_count = channel_details.statistics.as_ref()
        .and_then(|stats| stats.subscriber_count.as_ref())
        .and_then(|count| count.parse::<i64>().ok());
    
    let video_count = channel_details.statistics.as_ref()
        .and_then(|stats| stats.video_count.as_ref())
        .and_then(|count| count.parse::<i64>().ok());

    let result = YouTubeChannelDetails {
        channel_id: channel_id.to_string(),
        name: channel_details.snippet.title,
        description: channel_details.snippet.description,
        thumbnail_url,
        url: format!("https://www.youtube.com/channel/{}", channel_id),
        subscriber_count,
        video_count,
        recent_videos: videos,
    };

    match serde_json::to_string(&result) {
        Ok(json_response) => {
            println!("YouTube channel details successful for {}, found {} videos", result.name, result.recent_videos.len());
            HttpResponse::Ok().content_type("application/json").body(json_response)
        }
        Err(e) => {
            error!("Failed to serialize channel details response: {}", e);
            HttpResponse::InternalServerError().body("Failed to process channel details")
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

    println!("Starting the Actix Web server with yt search");
    
    // Initialize hit counters
    let hit_counters = web::Data::new(HitCounters::new());

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()  // Allow all origins since this is self-hostable
            .allow_any_method()  // Allow all HTTP methods
            .allow_any_header()  // Allow all headers
            .supports_credentials()
            .max_age(3600);      // Cache preflight requests for 1 hour

        App::new()
            .app_data(hit_counters.clone())
            .wrap(cors)
            .route("/api/search", web::get().to(search_handler))
            .route("/api/podcast", web::get().to(podcast_handler))
            .route("/api/trending", web::get().to(trending_handler))
            .route("/api/youtube/channel", web::get().to(youtube_channel_handler))
            .route("/api/stats", web::get().to(stats_handler))
    })
    .bind("0.0.0.0:5000")?
    .run()
    .await
}
