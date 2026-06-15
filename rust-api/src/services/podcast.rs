use crate::{error::AppResult, AppState, database::DatabasePool};
use crate::handlers::refresh::PodcastForRefresh;
use crate::handlers::youtube::process_youtube_channel;
use tracing::{debug, error, info, warn};
use serde_json::Value;
use sqlx::Row;

/// Podcast refresh service - matches Python's refresh_pods_for_user function exactly
pub async fn refresh_podcast(state: &AppState, podcast_id: i32) -> AppResult<Vec<Value>> {
    // Check if already refreshing
    if state.redis_client.is_podcast_refreshing(podcast_id).await? {
        return Ok(vec![]);
    }
    
    // Mark as refreshing
    state.redis_client.set_podcast_refreshing(podcast_id).await?;
    
    let result = refresh_podcast_internal(state, podcast_id).await;
    
    // Clear refreshing flag
    state.redis_client.clear_podcast_refreshing(podcast_id).await?;
    
    result
}

/// Internal refresh logic - matches Python refresh_pods_for_user function
async fn refresh_podcast_internal(state: &AppState, podcast_id: i32) -> AppResult<Vec<Value>> {
    info!("Refresh begin for podcast {}", podcast_id);

    // Get podcast details from database
    let podcast_info = get_podcast_for_refresh(&state.db_pool, podcast_id).await?;

    if let Some(podcast) = podcast_info {
        info!("Processing podcast: {}", podcast_id);

        if podcast.is_youtube {
            // Handle YouTube channel refresh
            refresh_youtube_channel(state, podcast_id, &podcast.feed_url, podcast.feed_cutoff_days.unwrap_or(0)).await?;
            Ok(vec![])
        } else {
            // Handle regular RSS podcast refresh
            let episodes = state.db_pool.add_episodes(
                podcast_id,
                &podcast.feed_url,
                podcast.artwork_url.as_deref().unwrap_or(""),
                podcast.auto_download,
                podcast.username.as_deref(),
                podcast.password.as_deref(),
            ).await?;
            
            // Convert episodes to JSON format for WebSocket response
            let episode_json = episodes.map(|_| vec![]).unwrap_or_default();
            Ok(episode_json)
        }
    } else {
        warn!("Podcast {} not found", podcast_id);
        Ok(vec![])
    }
}

/// Refresh all podcasts - matches Python refresh_pods function exactly
pub async fn refresh_all_podcasts(state: &AppState) -> AppResult<()> {
    info!("🚀 Starting refresh process for all podcasts");
    
    // Get all podcasts from database
    let podcasts = get_all_podcasts_for_refresh(&state.db_pool).await?;
    debug!("📊 Found {} podcasts to refresh", podcasts.len());
    
    let mut successful_refreshes = 0;
    let mut failed_refreshes = 0;
    
    for podcast in podcasts {
        match refresh_single_podcast(state, &podcast).await {
            Ok(_) => {
                successful_refreshes += 1;
            }
            Err(e) => {
                failed_refreshes += 1;
                error!("❌ Error refreshing podcast '{}' (ID: {}): {}", podcast.name, podcast.id, e);
            }
        }
    }
    
    warn!("🎯 Refresh process completed: {} successful, {} failed", successful_refreshes, failed_refreshes);
    Ok(())
}

/// Refresh a single podcast - matches Python refresh logic
async fn refresh_single_podcast(state: &AppState, podcast: &PodcastForRefresh) -> AppResult<()> {
    info!("🔄 Starting refresh for podcast '{}' (ID: {})", podcast.name, podcast.id);

    // Count episodes before refresh
    let episodes_before = match &state.db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "Episodes" WHERE podcastid = $1"#)
                .bind(podcast.id)
                .fetch_one(pool)
                .await.unwrap_or(0)
        }
        crate::database::DatabasePool::MySQL(pool) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM Episodes WHERE PodcastID = ?")
                .bind(podcast.id)
                .fetch_one(pool)
                .await.unwrap_or(0)
        }
    };

    if podcast.is_youtube {
        // Handle YouTube channel
        refresh_youtube_channel(state, podcast.id, &podcast.feed_url, podcast.feed_cutoff_days.unwrap_or(0)).await?;
    } else {
        // Handle regular RSS podcast
        state.db_pool.add_episodes(
            podcast.id,
            &podcast.feed_url,
            podcast.artwork_url.as_deref().unwrap_or(""),
            podcast.auto_download,
            podcast.username.as_deref(),
            podcast.password.as_deref(),
        ).await?;
    }
    
    // Count episodes after refresh
    let episodes_after: i64 = match &state.db_pool {
        crate::database::DatabasePool::Postgres(pool) => {
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "Episodes" WHERE podcastid = $1"#)
                .bind(podcast.id)
                .fetch_one(pool)
                .await.unwrap_or(0)
        }
        crate::database::DatabasePool::MySQL(pool) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM Episodes WHERE PodcastID = ?")
                .bind(podcast.id)
                .fetch_one(pool)
                .await.unwrap_or(0)
        }
    };
    
    let new_episodes = episodes_after - episodes_before;
    if new_episodes > 0 {
        debug!("✅ Completed refresh for podcast '{}' - added {} new episodes", podcast.name, new_episodes);
    } else {
        debug!("✅ Completed refresh for podcast '{}' - no new episodes found", podcast.name);
    }
    
    Ok(())
}

/// Handle YouTube channel refresh — fetches new videos and retries any missing audio downloads
async fn refresh_youtube_channel(state: &AppState, podcast_id: i32, feed_url: &str, feed_cutoff_days: i32) -> AppResult<()> {
    // Extract channel ID from feed URL
    let channel_id = if feed_url.contains("channel/") {
        feed_url.split("channel/").nth(1).unwrap_or(feed_url)
    } else {
        feed_url
    };
    let channel_id = channel_id.split('/').next().unwrap_or(channel_id);
    let channel_id = channel_id.split('?').next().unwrap_or(channel_id);

    info!("Refreshing YouTube channel: {} for podcast {}", channel_id, podcast_id);
    process_youtube_channel(podcast_id, channel_id, feed_cutoff_days, state).await
}

/// Get podcast details for refresh - matches Python select_podcast query
async fn get_podcast_for_refresh(db_pool: &DatabasePool, podcast_id: i32) -> AppResult<Option<PodcastForRefresh>> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            let row = sqlx::query(
                r#"SELECT 
                    PodcastID, FeedURL, ArtworkURL, AutoDownload, Username, Password,
                    IsYouTubeChannel, UserID, COALESCE(FeedURL, '') as channel_id, FeedCutoffDays
                FROM "Podcasts"
                WHERE PodcastID = $1"#
            )
            .bind(podcast_id)
            .fetch_optional(pool)
            .await?;
            
            if let Some(row) = row {
                Ok(Some(PodcastForRefresh {
                    id: row.try_get("PodcastID")?,
                    name: "".to_string(), // Not needed for refresh
                    feed_url: row.try_get("FeedURL")?,
                    artwork_url: row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default(),
                    auto_download: row.try_get("AutoDownload")?,
                    username: row.try_get("Username").ok(),
                    password: row.try_get("Password").ok(),
                    is_youtube: row.try_get("IsYouTubeChannel")?,
                    user_id: row.try_get("UserID")?,
                    feed_cutoff_days: row.try_get("FeedCutoffDays").ok(),
                }))
            } else {
                Ok(None)
            }
        }
        DatabasePool::MySQL(pool) => {
            let row = sqlx::query(
                "SELECT 
                    PodcastID, FeedURL, ArtworkURL, AutoDownload, Username, Password,
                    IsYouTubeChannel, UserID, COALESCE(FeedURL, '') as channel_id, FeedCutoffDays
                FROM Podcasts
                WHERE PodcastID = ?"
            )
            .bind(podcast_id)
            .fetch_optional(pool)
            .await?;
            
            if let Some(row) = row {
                Ok(Some(PodcastForRefresh {
                    id: row.try_get("PodcastID")?,
                    name: "".to_string(), // Not needed for refresh
                    feed_url: row.try_get("FeedURL")?,
                    artwork_url: row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default(),
                    auto_download: row.try_get("AutoDownload")?,
                    username: row.try_get("Username").ok(),
                    password: row.try_get("Password").ok(),
                    is_youtube: row.try_get("IsYouTubeChannel")?,
                    user_id: row.try_get("UserID")?,
                    feed_cutoff_days: row.try_get("FeedCutoffDays").ok(),
                }))
            } else {
                Ok(None)
            }
        }
    }
}

/// Get all podcasts for refresh - matches Python select_podcasts query
async fn get_all_podcasts_for_refresh(db_pool: &DatabasePool) -> AppResult<Vec<PodcastForRefresh>> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            let rows = sqlx::query(
                r#"SELECT 
                    PodcastID, FeedURL, ArtworkURL, AutoDownload, Username, Password,
                    IsYouTubeChannel, UserID, COALESCE(FeedURL, '') as channel_id, FeedCutoffDays
                FROM "Podcasts""#
            )
            .fetch_all(pool)
            .await?;
            
            let mut podcasts = Vec::new();
            for row in rows {
                podcasts.push(PodcastForRefresh {
                    id: row.try_get("PodcastID")?,
                    name: "".to_string(), // Not needed for refresh
                    feed_url: row.try_get("FeedURL")?,
                    artwork_url: row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default(),
                    auto_download: row.try_get("AutoDownload")?,
                    username: row.try_get("Username").ok(),
                    password: row.try_get("Password").ok(),
                    is_youtube: row.try_get("IsYouTubeChannel")?,
                    user_id: row.try_get("UserID")?,
                    feed_cutoff_days: row.try_get("FeedCutoffDays").ok(),
                });
            }
            Ok(podcasts)
        }
        DatabasePool::MySQL(pool) => {
            let rows = sqlx::query(
                "SELECT 
                    PodcastID, FeedURL, ArtworkURL, AutoDownload, Username, Password,
                    IsYouTubeChannel, UserID, COALESCE(FeedURL, '') as channel_id, FeedCutoffDays
                FROM Podcasts"
            )
            .fetch_all(pool)
            .await?;
            
            let mut podcasts = Vec::new();
            for row in rows {
                podcasts.push(PodcastForRefresh {
                    id: row.try_get("PodcastID")?,
                    name: "".to_string(), // Not needed for refresh
                    feed_url: row.try_get("FeedURL")?,
                    artwork_url: row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default(),
                    auto_download: row.try_get("AutoDownload")?,
                    username: row.try_get("Username").ok(),
                    password: row.try_get("Password").ok(),
                    is_youtube: row.try_get("IsYouTubeChannel")?,
                    user_id: row.try_get("UserID")?,
                    feed_cutoff_days: row.try_get("FeedCutoffDays").ok(),
                });
            }
            Ok(podcasts)
        }
    }
}


/// Remove unavailable episodes - matches Python remove_unavailable_episodes function
pub async fn remove_unavailable_episodes(db_pool: &DatabasePool) -> AppResult<()> {
    info!("Starting removal of unavailable episodes");
    
    // Get all episodes from database
    let episodes = get_all_episodes_for_check(db_pool).await?;
    
    let client = reqwest::Client::new();
    
    for episode in episodes {
        // Check if episode URL is still valid
        match client.head(&episode.url).send().await {
            Ok(response) => {
                if response.status().as_u16() == 404 {
                    // Remove episode from database
                    info!("Removing unavailable episode: {}", episode.id);
                    remove_episode_from_database(db_pool, episode.id).await?;
                }
            }
            Err(e) => {
                error!("Error checking episode {}: {}", episode.id, e);
            }
        }
    }
    
    Ok(())
}

/// Get all episodes for availability check
async fn get_all_episodes_for_check(db_pool: &DatabasePool) -> AppResult<Vec<EpisodeForCheck>> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            let rows = sqlx::query(
                r#"SELECT EpisodeID, PodcastID, EpisodeTitle, EpisodeURL, EpisodePubDate FROM "Episodes""#
            )
            .fetch_all(pool)
            .await?;
            
            let mut episodes = Vec::new();
            for row in rows {
                episodes.push(EpisodeForCheck {
                    id: row.try_get("EpisodeID")?,
                    podcast_id: row.try_get("PodcastID")?,
                    title: row.try_get("EpisodeTitle")?,
                    url: row.try_get("EpisodeURL")?,
                    pub_date: row.try_get("EpisodePubDate")?,
                });
            }
            Ok(episodes)
        }
        DatabasePool::MySQL(pool) => {
            let rows = sqlx::query(
                "SELECT EpisodeID, PodcastID, EpisodeTitle, EpisodeURL, EpisodePubDate FROM Episodes"
            )
            .fetch_all(pool)
            .await?;
            
            let mut episodes = Vec::new();
            for row in rows {
                episodes.push(EpisodeForCheck {
                    id: row.try_get("EpisodeID")?,
                    podcast_id: row.try_get("PodcastID")?,
                    title: row.try_get("EpisodeTitle")?,
                    url: row.try_get("EpisodeURL")?,
                    pub_date: row.try_get("EpisodePubDate")?,
                });
            }
            Ok(episodes)
        }
    }
}

/// Remove episode from database
async fn remove_episode_from_database(db_pool: &DatabasePool, episode_id: i32) -> AppResult<()> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"DELETE FROM "Episodes" WHERE EpisodeID = $1"#)
                .bind(episode_id)
                .execute(pool)
                .await?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("DELETE FROM Episodes WHERE EpisodeID = ?")
                .bind(episode_id)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

/// Episode data structure for availability check
#[derive(Debug, Clone)]
pub struct EpisodeForCheck {
    pub id: i32,
    pub podcast_id: i32,
    pub title: String,
    pub url: String,
    pub pub_date: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
}