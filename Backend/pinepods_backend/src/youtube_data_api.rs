// Optional YouTube Data API v3 backend, used instead of yt-dlp scraping when
// YOUTUBE_API_KEY is configured. Kept separate from the yt-dlp path in main.rs
// since the two share no implementation, only their output shape
// (YouTubeChannel / YouTubeChannelDetails / YouTubeVideo).

use actix_web::HttpResponse;
use log::error;
use std::collections::HashMap;
use std::env;

use super::{YouTubeChannel, YouTubeChannelDetails, YouTubeSearchResult, YouTubeVideo};

/// Returns the configured YouTube Data API v3 key, if any. When set, it's
/// preferred over the yt-dlp scraping path: it's the officially supported
/// way to query YouTube and doesn't depend on yt-dlp staying in sync with
/// YouTube's frontend. Optional -- callers fall back to yt-dlp when unset,
/// so this is fully backwards compatible with existing deployments.
pub fn api_key() -> Option<String> {
    non_empty(env::var("YOUTUBE_API_KEY").ok())
}

// Split out from api_key() so the "blank counts as unset" rule is testable
// without mutating process-wide env vars (which races across parallel tests).
fn non_empty(key: Option<String>) -> Option<String> {
    key.filter(|k| !k.trim().is_empty())
}

fn thumbnail_url(thumbnails: Option<&serde_json::Value>) -> Option<String> {
    thumbnails
        .and_then(|t| t.get("high").or_else(|| t.get("medium")).or_else(|| t.get("default")))
        .and_then(|t| t.get("url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

pub async fn search_channels(search_term: &str, api_key: &str) -> HttpResponse {
    println!("Searching YouTube via Data API for: {}", search_term);
    let client = reqwest::Client::new();
    let url = format!(
        "https://www.googleapis.com/youtube/v3/search?part=snippet&type=channel&maxResults=25&q={}&key={}",
        urlencoding::encode(search_term),
        api_key
    );

    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("YouTube Data API search request failed: {}", e);
            return HttpResponse::InternalServerError().body("YouTube Data API request failed");
        }
    };

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        error!("YouTube Data API search failed ({}): {}", status, body);
        return HttpResponse::InternalServerError().body("YouTube Data API search failed");
    }

    let body: serde_json::Value = match response.json().await {
        Ok(b) => b,
        Err(e) => {
            error!("Failed to parse YouTube Data API search response: {}", e);
            return HttpResponse::InternalServerError().body("Failed to parse YouTube Data API response");
        }
    };

    let items = body.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    let channels: Vec<YouTubeChannel> = items.iter().filter_map(|item| {
        let channel_id = item.get("id")?.get("channelId")?.as_str()?.to_string();
        let snippet = item.get("snippet");
        let name = snippet.and_then(|s| s.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let description = snippet.and_then(|s| s.get("description")).and_then(|v| v.as_str())
            .unwrap_or("").chars().take(500).collect();
        let thumbnail = thumbnail_url(snippet.and_then(|s| s.get("thumbnails"))).unwrap_or_default();

        Some(YouTubeChannel {
            channel_id: channel_id.clone(),
            name,
            description,
            thumbnail_url: thumbnail,
            url: format!("https://www.youtube.com/channel/{}", channel_id),
        })
    }).collect();

    let result = YouTubeSearchResult { results: channels };
    match serde_json::to_string(&result) {
        Ok(json) => {
            println!("YouTube Data API search found {} channels", result.results.len());
            HttpResponse::Ok().content_type("application/json").body(json)
        }
        Err(e) => {
            error!("Serialization error: {}", e);
            HttpResponse::InternalServerError().body("Failed to serialize response")
        }
    }
}

pub async fn channel_details(channel_id: &str, api_key: &str) -> HttpResponse {
    println!("Fetching YouTube channel via Data API: {}", channel_id);
    let client = reqwest::Client::new();

    let channel_url = format!(
        "https://www.googleapis.com/youtube/v3/channels?part=snippet,statistics,contentDetails&id={}&key={}",
        channel_id, api_key
    );
    let channel_resp = match client.get(&channel_url).send().await {
        Ok(r) => r,
        Err(e) => {
            error!("YouTube Data API channel request failed: {}", e);
            return HttpResponse::InternalServerError().body("YouTube Data API request failed");
        }
    };

    if !channel_resp.status().is_success() {
        let status = channel_resp.status();
        let body = channel_resp.text().await.unwrap_or_default();
        error!("YouTube Data API channel lookup failed ({}): {}", status, body);
        return HttpResponse::InternalServerError().body("YouTube Data API channel lookup failed");
    }

    let channel_body: serde_json::Value = match channel_resp.json().await {
        Ok(b) => b,
        Err(e) => {
            error!("Failed to parse YouTube Data API channel response: {}", e);
            return HttpResponse::InternalServerError().body("Failed to parse YouTube Data API response");
        }
    };

    let item = match channel_body.get("items").and_then(|v| v.as_array()).and_then(|a| a.first()) {
        Some(i) => i,
        None => return HttpResponse::NotFound().body("Channel not found or has no videos"),
    };

    let snippet = item.get("snippet");
    let name = snippet.and_then(|s| s.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let description = snippet.and_then(|s| s.get("description")).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let thumbnail = thumbnail_url(snippet.and_then(|s| s.get("thumbnails"))).unwrap_or_default();
    let subscriber_count = item.get("statistics")
        .and_then(|s| s.get("subscriberCount")).and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok());
    let video_count = item.get("statistics")
        .and_then(|s| s.get("videoCount")).and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok());
    let uploads_playlist_id = item.get("contentDetails")
        .and_then(|c| c.get("relatedPlaylists"))
        .and_then(|r| r.get("uploads"))
        .and_then(|v| v.as_str())
        .unwrap_or("").to_string();

    let mut recent_videos: Vec<YouTubeVideo> = Vec::new();
    if !uploads_playlist_id.is_empty() {
        // Paginate the full uploads playlist instead of stopping at one page.
        // playlistItems.list costs 1 quota unit per call regardless of page
        // size, so walking a channel's entire upload history (even thousands
        // of videos) is cheap. Hard-capped at 40 pages (~2000 videos) so a
        // pathological channel can't loop forever or blow the daily quota.
        let mut playlist_items: Vec<serde_json::Value> = Vec::new();
        let mut page_token: Option<String> = None;
        let mut pages_fetched = 0;
        loop {
            let mut playlist_url = format!(
                "https://www.googleapis.com/youtube/v3/playlistItems?part=snippet,contentDetails&maxResults=50&playlistId={}&key={}",
                uploads_playlist_id, api_key
            );
            if let Some(token) = &page_token {
                playlist_url.push_str(&format!("&pageToken={}", token));
            }

            let playlist_resp = match client.get(&playlist_url).send().await {
                Ok(r) if r.status().is_success() => r,
                _ => break,
            };
            let playlist_body: serde_json::Value = match playlist_resp.json().await {
                Ok(b) => b,
                Err(_) => break,
            };

            if let Some(items) = playlist_body.get("items").and_then(|v| v.as_array()) {
                playlist_items.extend(items.iter().cloned());
            }
            pages_fetched += 1;

            page_token = playlist_body.get("nextPageToken").and_then(|v| v.as_str()).map(|s| s.to_string());
            if page_token.is_none() || pages_fetched >= 40 {
                break;
            }
        }

        let video_ids: Vec<String> = playlist_items.iter()
            .filter_map(|i| i.get("contentDetails")?.get("videoId")?.as_str().map(|s| s.to_string()))
            .collect();

        // Batched calls to get ISO 8601 durations (playlistItems doesn't include them).
        // videos.list accepts at most 50 ids per call, so chunk the full id list.
        let mut durations: HashMap<String, String> = HashMap::new();
        for chunk in video_ids.chunks(50) {
            let videos_url = format!(
                "https://www.googleapis.com/youtube/v3/videos?part=contentDetails&id={}&key={}",
                chunk.join(","), api_key
            );
            if let Ok(videos_resp) = client.get(&videos_url).send().await {
                if let Ok(videos_body) = videos_resp.json::<serde_json::Value>().await {
                    if let Some(varr) = videos_body.get("items").and_then(|v| v.as_array()) {
                        for v in varr {
                            if let (Some(id), Some(dur)) = (
                                v.get("id").and_then(|x| x.as_str()),
                                v.get("contentDetails").and_then(|c| c.get("duration")).and_then(|x| x.as_str()),
                            ) {
                                durations.insert(id.to_string(), dur.to_string());
                            }
                        }
                    }
                }
            }
        }

        for i in &playlist_items {
            let cd = i.get("contentDetails");
            let video_id = match cd.and_then(|c| c.get("videoId")).and_then(|v| v.as_str()) {
                Some(id) if !id.is_empty() => id.to_string(),
                _ => continue,
            };
            let snip = i.get("snippet");
            let title = snip.and_then(|s| s.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let description = snip.and_then(|s| s.get("description")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let published_at = cd.and_then(|c| c.get("videoPublishedAt")).and_then(|v| v.as_str())
                .unwrap_or("").to_string();
            let thumbnail = thumbnail_url(snip.and_then(|s| s.get("thumbnails")))
                .unwrap_or_else(|| format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", video_id));

            recent_videos.push(YouTubeVideo {
                id: video_id.clone(),
                title,
                description,
                url: format!("https://www.youtube.com/watch?v={}", video_id),
                thumbnail,
                published_at,
                duration: durations.get(&video_id).cloned(),
            });
        }
    }

    let result = YouTubeChannelDetails {
        channel_id: channel_id.to_string(),
        name,
        description,
        thumbnail_url: thumbnail,
        url: format!("https://www.youtube.com/channel/{}", channel_id),
        subscriber_count,
        video_count,
        recent_videos,
    };

    match serde_json::to_string(&result) {
        Ok(json) => HttpResponse::Ok().content_type("application/json").body(json),
        Err(e) => {
            error!("Serialization error: {}", e);
            HttpResponse::InternalServerError().body("Failed to serialize response")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn non_empty_rejects_none_and_blank() {
        assert_eq!(non_empty(None), None);
        assert_eq!(non_empty(Some("".to_string())), None);
        assert_eq!(non_empty(Some("   ".to_string())), None);
    }

    #[test]
    fn non_empty_keeps_real_key() {
        assert_eq!(non_empty(Some("AIzaSyExampleKey".to_string())), Some("AIzaSyExampleKey".to_string()));
    }

    #[test]
    fn thumbnail_url_prefers_high_over_medium_and_default() {
        let thumbs = json!({
            "default": {"url": "default.jpg"},
            "medium": {"url": "medium.jpg"},
            "high": {"url": "high.jpg"},
        });
        assert_eq!(thumbnail_url(Some(&thumbs)), Some("high.jpg".to_string()));
    }

    #[test]
    fn thumbnail_url_falls_back_to_medium_then_default() {
        let medium_only = json!({"default": {"url": "default.jpg"}, "medium": {"url": "medium.jpg"}});
        assert_eq!(thumbnail_url(Some(&medium_only)), Some("medium.jpg".to_string()));

        let default_only = json!({"default": {"url": "default.jpg"}});
        assert_eq!(thumbnail_url(Some(&default_only)), Some("default.jpg".to_string()));
    }

    #[test]
    fn thumbnail_url_none_when_missing_or_absent() {
        assert_eq!(thumbnail_url(None), None);
        assert_eq!(thumbnail_url(Some(&json!({}))), None);
    }
}
