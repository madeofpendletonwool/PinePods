use crate::error::AppError;
use crate::redis_client::RedisClient;
use serde::{Deserialize, Serialize};
use std::env;

const TRENDING_CACHE_KEY: &str = "homepage:trending";
const TRENDING_CACHE_TTL: u64 = 6 * 60 * 60; // 6 hours in seconds

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrendingPodcast {
    pub id: i64,
    pub title: String,
    pub url: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub image: Option<String>,
    pub artwork: Option<String>,
    #[serde(rename = "trendScore")]
    pub trend_score: Option<f64>,
    pub categories: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct TrendingResponse {
    status: String,
    feeds: Vec<TrendingFeed>,
    count: i32,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrendingFeed {
    id: i64,
    title: String,
    url: String,
    #[serde(rename = "originalUrl")]
    original_url: Option<String>,
    link: Option<String>,
    description: Option<String>,
    author: Option<String>,
    #[serde(rename = "ownerName")]
    owner_name: Option<String>,
    image: Option<String>,
    artwork: Option<String>,
    #[serde(rename = "newestItemPubdate")]
    newest_item_pubdate: Option<i64>,
    #[serde(rename = "itunesId")]
    itunes_id: Option<i64>,
    #[serde(rename = "trendScore")]
    trend_score: Option<f64>,
    language: Option<String>,
    categories: Option<serde_json::Value>,
}

/// Fetches trending podcasts from the search proxy backend with caching
pub async fn get_trending_podcasts(
    redis_client: &RedisClient,
    max_results: Option<u32>,
) -> Result<Vec<TrendingPodcast>, AppError> {
    // Try to get from cache first
    if let Ok(Some(cached_data)) = redis_client.get::<String>(TRENDING_CACHE_KEY).await {
        if let Ok(podcasts) = serde_json::from_str::<Vec<TrendingPodcast>>(&cached_data) {
            tracing::info!("Returning trending podcasts from cache");

            // Apply max_results limit if specified
            if let Some(max) = max_results {
                return Ok(podcasts.into_iter().take(max as usize).collect());
            }
            return Ok(podcasts);
        }
    }

    // Cache miss or expired - fetch from search proxy
    tracing::info!("Cache miss for trending podcasts - fetching from search proxy");

    let search_api_url = env::var("SEARCH_API_URL")
        .map_err(|_| AppError::external_error("SEARCH_API_URL environment variable not set"))?;

    // Replace /api/search with /api/trending
    let trending_url = search_api_url.replace("/api/search", "/api/trending");

    // Add max parameter if specified
    let url_with_params = if let Some(max) = max_results {
        format!("{}?max={}", trending_url, max)
    } else {
        trending_url
    };

    tracing::info!("Fetching trending from: {}", url_with_params);

    let client = reqwest::Client::new();
    let response = client
        .get(&url_with_params)
        .send()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to call search proxy: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::external_error(&format!(
            "Search proxy error: {}",
            response.status()
        )));
    }

    let trending_response: TrendingResponse = response
        .json()
        .await
        .map_err(|e| AppError::external_error(&format!("Failed to parse trending response: {}", e)))?;

    // Convert to our simplified format
    let podcasts: Vec<TrendingPodcast> = trending_response
        .feeds
        .into_iter()
        .map(|feed| TrendingPodcast {
            id: feed.id,
            title: feed.title,
            url: feed.url,
            description: feed.description,
            author: feed.author.or(feed.owner_name),
            image: feed.image.clone(),
            artwork: feed.artwork.or(feed.image),
            trend_score: feed.trend_score,
            categories: feed.categories,
        })
        .collect();

    // Cache the results
    if let Ok(json_data) = serde_json::to_string(&podcasts) {
        if let Err(e) = redis_client
            .set_ex(TRENDING_CACHE_KEY, &json_data, TRENDING_CACHE_TTL)
            .await
        {
            tracing::warn!("Failed to cache trending podcasts: {}", e);
            // Don't fail the request if caching fails
        } else {
            tracing::info!("Cached {} trending podcasts for {} hours", podcasts.len(), TRENDING_CACHE_TTL / 3600);
        }
    }

    Ok(podcasts)
}

/// Invalidates the trending cache (useful for admin operations)
pub async fn invalidate_trending_cache(redis_client: &RedisClient) -> Result<(), AppError> {
    redis_client.delete(TRENDING_CACHE_KEY).await?;
    tracing::info!("Trending cache invalidated");
    Ok(())
}

/// Generate podcast recommendations for a single user based on their listening history and preferences
pub async fn generate_user_recommendations(
    db_pool: &crate::database::DatabasePool,
    redis_client: &RedisClient,
    user_id: i32,
) -> Result<(), AppError> {
    use std::collections::HashMap;

    tracing::info!("Generating recommendations for user {}", user_id);

    // Get user's podcast categories
    let user_categories = db_pool.get_user_podcast_categories(user_id).await?;

    if user_categories.is_empty() {
        tracing::info!("User {} has no podcasts yet, skipping recommendations", user_id);
        return Ok(());
    }

    tracing::debug!("User {} categories: {:?}", user_id, user_categories);

    // Get trending podcasts as a pool of potential recommendations
    let trending_podcasts = get_trending_podcasts(redis_client, Some(100)).await?;

    if trending_podcasts.is_empty() {
        tracing::warn!("No trending podcasts available for recommendations");
        return Ok(());
    }

    // Score each trending podcast based on category overlap
    let mut recommendations: Vec<(i64, f64, Option<String>)> = Vec::new();

    for podcast in trending_podcasts {
        let mut score = 0.0;
        let mut matched_categories = Vec::new();

        // Parse podcast categories
        if let Some(ref categories) = podcast.categories {
            if let Some(cats_map) = categories.as_object() {
                for (_, cat_value) in cats_map {
                    if let Some(cat_name) = cat_value.as_str() {
                        let cat_clean = cat_name.trim();

                        // Check if this category matches any of the user's categories
                        for user_cat in &user_categories {
                            if user_cat.eq_ignore_ascii_case(cat_clean) {
                                score += 10.0; // Base score for category match
                                matched_categories.push(cat_clean.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Add trending score bonus (if available)
        if let Some(trend_score) = podcast.trend_score {
            score += trend_score * 0.1; // Small bonus for trending
        }

        // Only recommend if there's some score
        if score > 0.0 {
            let reason = if !matched_categories.is_empty() {
                Some(format!("Matches your interests: {}", matched_categories.join(", ")))
            } else {
                Some("Popular and trending".to_string())
            };

            recommendations.push((podcast.id, score, reason));
        }
    }

    // Sort by score descending and take top 20
    recommendations.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    recommendations.truncate(20);

    tracing::info!(
        "Generated {} recommendations for user {} (from {} trending podcasts)",
        recommendations.len(),
        user_id,
        trending_podcasts.len()
    );

    // Store recommendations in database
    if !recommendations.is_empty() {
        db_pool.store_recommendations(user_id, recommendations).await?;
        tracing::info!("Stored {} recommendations for user {}", recommendations.len(), user_id);
    }

    Ok(())
}

/// Generate recommendations for all users who have podcasts
pub async fn generate_all_recommendations(
    db_pool: &crate::database::DatabasePool,
    redis_client: &RedisClient,
) -> Result<(), AppError> {
    tracing::info!("Starting recommendation generation for all users");

    let users = db_pool.get_users_with_podcasts().await?;
    tracing::info!("Found {} users with podcasts", users.len());

    let mut success_count = 0;
    let mut error_count = 0;

    for user_id in users {
        match generate_user_recommendations(db_pool, redis_client, user_id).await {
            Ok(_) => {
                success_count += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to generate recommendations for user {}: {}", user_id, e);
                error_count += 1;
            }
        }
    }

    tracing::info!(
        "Recommendation generation complete: {} successful, {} errors",
        success_count,
        error_count
    );

    Ok(())
}
