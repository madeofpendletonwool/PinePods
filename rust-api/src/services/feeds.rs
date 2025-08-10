use feed_rs::parser;
use reqwest::Client;
use crate::error::{AppError, AppResult};

/// RSS feed fetching and parsing - will replace Python's feedparser
pub async fn fetch_and_parse_feed(url: &str) -> AppResult<feed_rs::model::Feed> {
    let client = Client::new();
    
    let response = client
        .get(url)
        .header("User-Agent", "PinePods/1.0")
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(AppError::FeedParsing(format!("HTTP error: {}", response.status())));
    }
    
    let content = response.bytes().await?;
    
    parser::parse(&content[..])
        .map_err(|e| AppError::FeedParsing(format!("Feed parsing error: {}", e)))
}