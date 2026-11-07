//! Public episode share links.

use super::*;

impl DatabasePool {
    // Cleanup expired shared episodes - matches Python cleanup_expired_shared_episodes function exactly
    pub async fn cleanup_expired_shared_episodes(&self) -> AppResult<()> {
        let now = chrono::Utc::now();
        
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"DELETE FROM "SharedEpisodes" WHERE expirationdate < $1"#)
                    .bind(now)
                    .execute(pool)
                    .await?;
                
                tracing::info!("Cleaned up {} expired SharedEpisodes records", result.rows_affected());
            },
            mysql(pool) => {
                let result = sqlx::query("DELETE FROM SharedEpisodes WHERE ExpirationDate < ?")
                    .bind(now)
                    .execute(pool)
                    .await?;
                
                tracing::info!("Cleaned up {} expired SharedEpisodes records", result.rows_affected());
            },
        });
        Ok(())
    }

    // Add shared episode - uses current database schema with ShareCode
    pub async fn add_shared_episode(&self, episode_id: i32, shared_by: i32, share_code: &str, expiration_date: chrono::DateTime<chrono::Utc>) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"
                    INSERT INTO "SharedEpisodes" (episodeid, sharedby, sharecode, expirationdate)
                    VALUES ($1, $2, $3, $4)
                "#)
                .bind(episode_id)
                .bind(shared_by)
                .bind(share_code)
                .bind(expiration_date)
                .execute(pool)
                .await;

                match result {
                    Ok(_) => Ok(true),
                    Err(e) => {
                        tracing::error!("Error sharing episode: {}", e);
                        Ok(false)
                    }
                }
            },
            mysql(pool) => {
                let result = sqlx::query(
                    "INSERT INTO SharedEpisodes (EpisodeID, SharedBy, ShareCode, ExpirationDate) VALUES (?, ?, ?, ?)"
                )
                .bind(episode_id)
                .bind(shared_by)
                .bind(share_code)
                .bind(expiration_date)
                .execute(pool)
                .await;

                match result {
                    Ok(_) => Ok(true),
                    Err(e) => {
                        tracing::error!("Error sharing episode: {}", e);
                        Ok(false)
                    }
                }
            },
        })
    }

    // Get episode ID by share code (for shared episode access)
    pub async fn get_episode_id_by_share_code(&self, share_code: &str) -> AppResult<Option<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query_as::<(i32,)>(r#"
                    SELECT episodeid FROM "SharedEpisodes" 
                    WHERE sharecode = $1 AND expirationdate > NOW()
                "#)
                .bind(share_code)
                .fetch_optional(pool)
                .await?;
                
                Ok(result.map(|row| row.0))
            },
            mysql(pool) => {
                let result = sqlx::query_as::<_, (i32,)>(
                    "SELECT EpisodeID FROM SharedEpisodes 
                     WHERE ShareCode = ? AND ExpirationDate > NOW()"
                )
                .bind(share_code)
                .fetch_optional(pool)
                .await?;
                
                Ok(result.map(|row| row.0))
            },
        })
    }

    // Get shared episode metadata - bypasses user restrictions for public access
    pub async fn get_shared_episode_metadata(&self, episode_id: i32) -> AppResult<serde_json::Value> {
        dialect_match!(self, {
            pg(pool) => {
                // First try regular episodes
                let row = q::query(
                    r#"SELECT 
                        "Podcasts".podcastid, 
                        "Podcasts".feedurl,
                        "Podcasts".podcastname, 
                        "Podcasts".artworkurl,
                        "Episodes".episodetitle,
                        "Episodes".episodepubdate,
                        "Episodes".episodedescription,
                        "Episodes".episodeartwork,
                        "Episodes".episodeurl,
                        "Episodes".episodeduration,
                        "Episodes".episodeid,
                        "Podcasts".websiteurl,
                        COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) AS completed,
                        FALSE::boolean as is_youtube
                    FROM "Episodes"
                    INNER JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid
                    WHERE "Episodes".episodeid = $1 ORDER BY "Podcasts".podcastid LIMIT 1"#
                )
                .bind(episode_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    let episodepubdate = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?
                        .format("%Y-%m-%dT%H:%M:%S").to_string();
                    
                    Ok(serde_json::json!({
                        "podcastid": row.try_get::<i32, _>("podcastid")?,
                        "feedurl": row.try_get::<Option<String>, _>("feedurl")?,
                        "podcastname": row.try_get::<String, _>("podcastname")?,
                        "artworkurl": row.try_get::<Option<String>, _>("artworkurl")?,
                        "episodetitle": row.try_get::<String, _>("episodetitle")?,
                        "episodepubdate": episodepubdate,
                        "episodedescription": row.try_get::<Option<String>, _>("episodedescription")?,
                        "episodeartwork": row.try_get::<Option<String>, _>("episodeartwork")?,
                        "episodeurl": row.try_get::<String, _>("episodeurl")?,
                        "episodeduration": row.try_get::<Option<i32>, _>("episodeduration")?,
                        "episodeid": row.try_get::<i32, _>("episodeid")?,
                        "websiteurl": row.try_get::<Option<String>, _>("websiteurl")?,
                        "listenduration": None::<i32>, // No user-specific data for shared episodes
                        "completed": row.try_get::<Option<bool>, _>("completed")?,
                        "is_youtube": false
                    }))
                } else {
                    // Try YouTube videos
                    let row = q::query(
                        r#"SELECT 
                            "Podcasts".podcastid, 
                            "Podcasts".feedurl,
                            "Podcasts".podcastname, 
                            "Podcasts".artworkurl,
                            "YouTubeVideos".videotitle as episodetitle,
                            "YouTubeVideos".publishedat as episodepubdate,
                            "YouTubeVideos".videodescription as episodedescription,
                            "YouTubeVideos".thumbnailurl as episodeartwork,
                            "YouTubeVideos".videourl as episodeurl,
                            "YouTubeVideos".duration as episodeduration,
                            "YouTubeVideos".videoid as episodeid,
                            "Podcasts".websiteurl,
                            "YouTubeVideos".completed,
                            TRUE::boolean as is_youtube
                        FROM "YouTubeVideos"
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        WHERE "YouTubeVideos".videoid = $1"#
                    )
                    .bind(episode_id)
                    .fetch_optional(pool)
                    .await?;

                    if let Some(row) = row {
                        let episodepubdate = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?
                            .format("%Y-%m-%dT%H:%M:%S").to_string();
                        
                        Ok(serde_json::json!({
                            "podcastid": row.try_get::<i32, _>("podcastid")?,
                            "feedurl": row.try_get::<Option<String>, _>("feedurl")?,
                            "podcastname": row.try_get::<String, _>("podcastname")?,
                            "artworkurl": row.try_get::<Option<String>, _>("artworkurl")?,
                            "episodetitle": row.try_get::<String, _>("episodetitle")?,
                            "episodepubdate": episodepubdate,
                            "episodedescription": row.try_get::<Option<String>, _>("episodedescription")?,
                            "episodeartwork": row.try_get::<Option<String>, _>("episodeartwork")?,
                            "episodeurl": row.try_get::<String, _>("episodeurl")?,
                            "episodeduration": row.try_get::<Option<i32>, _>("episodeduration")?,
                            "episodeid": row.try_get::<i32, _>("episodeid")?,
                            "websiteurl": row.try_get::<Option<String>, _>("websiteurl")?,
                            "listenposition": None::<i32>, // No user-specific data for shared episodes  
                            "completed": row.try_get::<Option<bool>, _>("completed")?,
                            "is_youtube": true
                        }))
                    } else {
                        Err(AppError::not_found("Episode not found"))
                    }
                }
            },
            mysql(pool) => {
                // First try regular episodes
                let row = sqlx::query(
                    "SELECT 
                        Podcasts.PodcastID, 
                        Podcasts.FeedURL,
                        Podcasts.PodcastName, 
                        Podcasts.ArtworkURL,
                        Episodes.EpisodeTitle,
                        Episodes.EpisodePubDate,
                        Episodes.EpisodeDescription,
                        Episodes.EpisodeArtwork,
                        Episodes.EpisodeURL,
                        Episodes.EpisodeDuration,
                        Episodes.EpisodeID,
                        Podcasts.WebsiteURL,
                        COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = Podcasts.UserID), 0) AS Completed,
                        FALSE as is_youtube
                    FROM Episodes
                    INNER JOIN Podcasts ON Episodes.FeedID = Podcasts.FeedID
                    WHERE Episodes.EpisodeID = ? ORDER BY Podcasts.PodcastID LIMIT 1"
                )
                .bind(episode_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    let episodepubdate = row.try_get::<chrono::NaiveDateTime, _>("EpisodePubDate")?
                        .format("%Y-%m-%dT%H:%M:%S").to_string();
                    
                    Ok(serde_json::json!({
                        "podcastid": row.try_get::<i32, _>("PodcastID")?,
                        "feedurl": row.try_get::<Option<String>, _>("FeedURL")?,
                        "podcastname": row.try_get::<String, _>("PodcastName")?,
                        "artworkurl": row.try_get::<Option<String>, _>("ArtworkURL")?,
                        "episodetitle": row.try_get::<String, _>("EpisodeTitle")?,
                        "episodepubdate": episodepubdate,
                        "episodedescription": row.try_get::<Option<String>, _>("EpisodeDescription")?,
                        "episodeartwork": row.try_get::<Option<String>, _>("EpisodeArtwork")?,
                        "episodeurl": row.try_get::<String, _>("EpisodeURL")?,
                        "episodeduration": row.try_get::<Option<i32>, _>("EpisodeDuration")?,
                        "episodeid": row.try_get::<i32, _>("EpisodeID")?,
                        "websiteurl": row.try_get::<Option<String>, _>("WebsiteURL")?,
                        "listenduration": None::<i32>, // No user-specific data for shared episodes
                        "completed": row.try_get::<Option<bool>, _>("Completed")?,
                        "is_youtube": false
                    }))
                } else {
                    // Try YouTube videos
                    let row = sqlx::query(
                        "SELECT 
                            Podcasts.PodcastID, 
                            Podcasts.FeedURL,
                            Podcasts.PodcastName, 
                            Podcasts.ArtworkURL,
                            YouTubeVideos.VideoTitle as EpisodeTitle,
                            YouTubeVideos.PublishedAt as EpisodePubDate,
                            YouTubeVideos.VideoDescription as EpisodeDescription,
                            YouTubeVideos.ThumbnailURL as EpisodeArtwork,
                            YouTubeVideos.VideoURL as EpisodeURL,
                            YouTubeVideos.Duration as EpisodeDuration,
                            YouTubeVideos.VideoID as EpisodeID,
                            Podcasts.WebsiteURL,
                            YouTubeVideos.Completed,
                            TRUE as is_youtube
                        FROM YouTubeVideos
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        WHERE YouTubeVideos.VideoID = ?"
                    )
                    .bind(episode_id)
                    .fetch_optional(pool)
                    .await?;

                    if let Some(row) = row {
                        let episodepubdate = row.try_get::<chrono::NaiveDateTime, _>("EpisodePubDate")?
                            .format("%Y-%m-%dT%H:%M:%S").to_string();
                        
                        Ok(serde_json::json!({
                            "podcastid": row.try_get::<i32, _>("PodcastID")?,
                            "feedurl": row.try_get::<Option<String>, _>("FeedURL")?,
                            "podcastname": row.try_get::<String, _>("PodcastName")?,
                            "artworkurl": row.try_get::<Option<String>, _>("ArtworkURL")?,
                            "episodetitle": row.try_get::<String, _>("EpisodeTitle")?,
                            "episodepubdate": episodepubdate,
                            "episodedescription": row.try_get::<Option<String>, _>("EpisodeDescription")?,
                            "episodeartwork": row.try_get::<Option<String>, _>("EpisodeArtwork")?,
                            "episodeurl": row.try_get::<String, _>("EpisodeURL")?,
                            "episodeduration": row.try_get::<Option<i32>, _>("EpisodeDuration")?,
                            "episodeid": row.try_get::<i32, _>("EpisodeID")?,
                            "websiteurl": row.try_get::<Option<String>, _>("WebsiteURL")?,
                            "listenposition": None::<i32>, // No user-specific data for shared episodes  
                            "completed": row.try_get::<Option<bool>, _>("Completed")?,
                            "is_youtube": true
                        }))
                    } else {
                        Err(AppError::not_found("Episode not found"))
                    }
                }
            },
        })
    }

    // Get all shared episodes for a user
    pub async fn get_user_shared_episodes(&self, user_id: i32) -> AppResult<Vec<serde_json::Value>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(r#"
                    SELECT
                        se.sharecode,
                        se.expirationdate,
                        COALESCE(e.episodetitle, ye.videotitle) AS episodetitle,
                        COALESCE(p.podcastname, yp.podcastname) AS podcastname
                    FROM "SharedEpisodes" se
                    LEFT JOIN "Episodes" e ON se.episodeid = e.episodeid
                    LEFT JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = se.sharedby
                    LEFT JOIN "YouTubeVideos" ye ON se.episodeid = ye.videoid
                    LEFT JOIN "Podcasts" yp ON ye.podcastid = yp.podcastid
                    WHERE se.sharedby = $1
                    ORDER BY se.expirationdate DESC
                "#)
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let result = rows.iter().map(|row| {
                    let sharecode: String = row.try_get("sharecode").unwrap_or_default();
                    let expiration: chrono::NaiveDateTime = row.try_get("expirationdate").unwrap_or_else(|_| chrono::Utc::now().naive_utc());
                    let episode_title: String = row.try_get("episodetitle").unwrap_or_else(|_| "Unknown".to_string());
                    let podcast_name: String = row.try_get("podcastname").unwrap_or_else(|_| "Unknown".to_string());
                    serde_json::json!({
                        "share_code": sharecode,
                        "expiration_date": expiration.format("%Y-%m-%d %H:%M UTC").to_string(),
                        "episode_title": episode_title,
                        "podcast_name": podcast_name,
                    })
                }).collect();
                Ok(result)
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT
                        se.ShareCode AS sharecode,
                        se.ExpirationDate AS expirationdate,
                        COALESCE(e.EpisodeTitle, ye.VideoTitle) AS episodetitle,
                        COALESCE(p.PodcastName, yp.PodcastName) AS podcastname
                    FROM SharedEpisodes se
                    LEFT JOIN Episodes e ON se.EpisodeID = e.EpisodeID
                    LEFT JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = se.SharedBy
                    LEFT JOIN YouTubeVideos ye ON se.EpisodeID = ye.VideoID
                    LEFT JOIN Podcasts yp ON ye.PodcastID = yp.PodcastID
                    WHERE se.SharedBy = ?
                    ORDER BY se.ExpirationDate DESC"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let result = rows.iter().map(|row| {
                    let sharecode: String = row.try_get("sharecode").unwrap_or_default();
                    let expiration: chrono::NaiveDateTime = row.try_get("expirationdate").unwrap_or_else(|_| chrono::Utc::now().naive_utc());
                    let episode_title: String = row.try_get("episodetitle").unwrap_or_else(|_| "Unknown".to_string());
                    let podcast_name: String = row.try_get("podcastname").unwrap_or_else(|_| "Unknown".to_string());
                    serde_json::json!({
                        "share_code": sharecode,
                        "expiration_date": expiration.format("%Y-%m-%d %H:%M UTC").to_string(),
                        "episode_title": episode_title,
                        "podcast_name": podcast_name,
                    })
                }).collect();
                Ok(result)
            },
        })
    }

    // Delete a shared episode by share code (user must own it)
    pub async fn delete_user_shared_episode(&self, share_code: &str, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"DELETE FROM "SharedEpisodes" WHERE sharecode = $1 AND sharedby = $2"#)
                    .bind(share_code)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let result = sqlx::query("DELETE FROM SharedEpisodes WHERE ShareCode = ? AND SharedBy = ?")
                    .bind(share_code)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Extend expiration of a shared episode by N days (user must own it)
    pub async fn extend_user_shared_episode(&self, share_code: &str, user_id: i32, days: i64) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let current: Option<chrono::NaiveDateTime> = q::query_scalar(r#"
                    SELECT expirationdate FROM "SharedEpisodes" WHERE sharecode = $1 AND sharedby = $2
                "#)
                .bind(share_code)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                let Some(current) = current else { return Ok(false); };
                let new_expiry = current + chrono::Duration::days(days);
                tracing::info!("Extending {} from {} to {} (+{} days)", share_code, current, new_expiry, days);

                q::query(r#"
                    UPDATE "SharedEpisodes"
                    SET expirationdate = $1
                    WHERE sharecode = $2 AND sharedby = $3
                "#)
                .bind(new_expiry)
                .bind(share_code)
                .bind(user_id)
                .execute(pool)
                .await?;
                Ok(true)
            },
            mysql(pool) => {
                let result = sqlx::query(
                    "UPDATE SharedEpisodes SET ExpirationDate = DATE_ADD(ExpirationDate, INTERVAL ? DAY) WHERE ShareCode = ? AND SharedBy = ?"
                )
                .bind(days)
                .bind(share_code)
                .bind(user_id)
                .execute(pool)
                .await?;
                Ok(result.rows_affected() > 0)
            },
        })
    }
}
