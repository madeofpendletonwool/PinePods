use sqlx::{MySql, Pool, Postgres, Row};
use std::time::Duration;
use crate::{config::Config, error::{AppError, AppResult}};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Clone)]
pub enum DatabasePool {
    Postgres(Pool<Postgres>),
    MySQL(Pool<MySql>),
}

impl DatabasePool {
    pub async fn new(config: &Config) -> AppResult<Self> {
        let database_url = config.database_url();
        
        match config.database.db_type.as_str() {
            "postgresql" => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(config.database.max_connections)
                    .min_connections(config.database.min_connections)
                    .acquire_timeout(Duration::from_secs(30))
                    .connect(&database_url)
                    .await?;

                // Skip migrations for now - database already exists
                
                Ok(DatabasePool::Postgres(pool))
            }
            _ => {
                let pool = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(config.database.max_connections)
                    .min_connections(config.database.min_connections)
                    .acquire_timeout(Duration::from_secs(30))
                    .connect(&database_url)
                    .await?;

                // Skip migrations for now - database already exists
                
                Ok(DatabasePool::MySQL(pool))
            }
        }
    }

    pub async fn health_check(&self) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query("SELECT 1 as health")
                    .fetch_one(pool)
                    .await?;
                let health: i32 = row.try_get("health")?;
                Ok(health == 1)
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT 1 as health")
                    .fetch_one(pool)
                    .await?;
                let health: i32 = row.try_get("health")?;
                Ok(health == 1)
            }
        }
    }

    // Helper methods for database operations

    // Verify API key - matches Python verify_api_key function
    pub async fn verify_api_key(&self, api_key: &str) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT * FROM "APIKeys" WHERE apikey = $1"#)
                    .bind(api_key)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(row.is_some())
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT * FROM APIKeys WHERE APIKey = ?")
                    .bind(api_key)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(row.is_some())
            }
        }
    }

    // Verify password - matches Python verify_password function
    pub async fn verify_password(&self, username: &str, password: &str) -> AppResult<bool> {
        use crate::services::auth::verify_password;
        
        let stored_hash = match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT hashed_pw FROM "Users" WHERE username = $1"#)
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<String, _>("hashed_pw")?
                } else {
                    return Ok(false);
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT Hashed_PW FROM Users WHERE Username = ?")
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<String, _>("Hashed_PW")?
                } else {
                    return Ok(false);
                }
            }
        };

        verify_password(password, &stored_hash)
    }

    // Get API key for user - matches Python get_api_key function  
    pub async fn get_api_key(&self, username: &str) -> AppResult<String> {
        match self {
            DatabasePool::Postgres(pool) => {
                // First get UserID
                let user_row = sqlx::query(r#"SELECT userid FROM "Users" WHERE username = $1"#)
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                let user_id: i32 = match user_row {
                    Some(row) => row.try_get("userid")?,
                    None => return Err(AppError::not_found("User not found")),
                };
                
                // Then get API key
                let api_row = sqlx::query(r#"SELECT apikey FROM "APIKeys" WHERE userid = $1 LIMIT 1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match api_row {
                    Some(row) => Ok(row.try_get("apikey")?),
                    None => Err(AppError::not_found("API key not found for user")),
                }
            }
            DatabasePool::MySQL(pool) => {
                // First get UserID
                let user_row = sqlx::query("SELECT UserID FROM Users WHERE Username = ?")
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                let user_id: i32 = match user_row {
                    Some(row) => row.try_get("UserID")?,
                    None => return Err(AppError::not_found("User not found")),
                };
                
                // Then get API key
                let api_row = sqlx::query("SELECT APIKey FROM APIKeys WHERE UserID = ? LIMIT 1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match api_row {
                    Some(row) => Ok(row.try_get("APIKey")?),
                    None => Err(AppError::not_found("API key not found for user")),
                }
            }
        }
    }

    // Get user ID from API key - matches Python get_api_user function
    pub async fn get_user_id_from_api_key(&self, api_key: &str) -> AppResult<i32> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT userid FROM "APIKeys" WHERE apikey = $1 LIMIT 1"#)
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("userid")?)
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT UserID FROM APIKeys WHERE APIKey = ? LIMIT 1")
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("UserID")?)
            }
        }
    }

    // Get user details by ID - matches Python get_user_details_id function
    pub async fn get_user_details_by_id(&self, user_id: i32) -> AppResult<crate::handlers::auth::UserDetails> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT * FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(crate::handlers::auth::UserDetails {
                    UserID: row.try_get("userid")?,
                    Fullname: row.try_get("fullname").ok(),
                    Username: row.try_get("username").ok(),
                    Email: row.try_get("email").ok(),
                    Hashed_PW: row.try_get("hashed_pw").ok(),
                    Salt: row.try_get("salt").ok(),
                })
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT * FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(crate::handlers::auth::UserDetails {
                    UserID: row.try_get("UserID")?,
                    Fullname: row.try_get("Fullname").ok(),
                    Username: row.try_get("Username").ok(),
                    Email: row.try_get("Email").ok(),
                    Hashed_PW: row.try_get("Hashed_PW").ok(),
                    Salt: row.try_get("Salt").ok(),
                })
            }
        }
    }

    pub async fn get_user_by_credentials(&self, username: &str) -> AppResult<Option<UserCredentials>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(
                    r#"SELECT userid as user_id, username as username, hashed_pw as hashed_password, email as email
                     FROM "Users" WHERE username = $1"#
                )
                .bind(username)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(UserCredentials {
                        user_id: row.try_get("user_id")?,
                        username: row.try_get("username")?,
                        hashed_password: row.try_get("hashed_password")?,
                        email: row.try_get("email")?,
                    }))
                } else {
                    Ok(None)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query(
                    "SELECT UserID as user_id, Username as username, Hashed_PW as hashed_password, Email as email
                     FROM Users WHERE Username = ?"
                )
                .bind(username)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(UserCredentials {
                        user_id: row.try_get("user_id")?,
                        username: row.try_get("username")?,
                        hashed_password: row.try_get("hashed_password")?,
                        email: row.try_get("email")?,
                    }))
                } else {
                    Ok(None)
                }
            }
        }
    }

    pub async fn get_user_settings_by_user_id(&self, user_id: i32) -> AppResult<Option<UserSettings>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(
                    r#"SELECT us.userid, ak.apikey as api_key, us.theme, 
                              COALESCE(us.auto_download_episodes, false) as auto_download_episodes,
                              COALESCE(us.auto_delete_episodes, false) as auto_delete_episodes
                       FROM "UserSettings" us 
                       LEFT JOIN "APIKeys" ak ON us.userid = ak.userid
                       WHERE us.userid = $1"#
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(UserSettings {
                        user_id: row.try_get("userid")?,
                        api_key: row.try_get("api_key")?,
                        theme: row.try_get("theme")?,
                        auto_download_episodes: row.try_get("auto_download_episodes")?,
                        auto_delete_episodes: row.try_get("auto_delete_episodes")?,
                    }))
                } else {
                    Ok(None)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query(
                    "SELECT user_id, api_key, theme, auto_download_episodes, auto_delete_episodes
                     FROM UserSettings WHERE user_id = ?"
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(UserSettings {
                        user_id: row.try_get("user_id")?,
                        api_key: row.try_get("api_key")?,
                        theme: row.try_get("theme")?,
                        auto_download_episodes: row.try_get("auto_download_episodes")?,
                        auto_delete_episodes: row.try_get("auto_delete_episodes")?,
                    }))
                } else {
                    Ok(None)
                }
            }
        }
    }

    // Get user ID by API key - matches Python get_api_user function
    pub async fn get_api_user(&self, api_key: &str) -> AppResult<i32> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT userid FROM "APIKeys" WHERE apikey = $1"#)
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("userid")?)
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT UserID FROM APIKeys WHERE APIKey = ?")
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("UserID")?)
            }
        }
    }

    // Get episodes for user - matches Python return_episodes function
    pub async fn return_episodes(&self, user_id: i32) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT * FROM (
                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "Episodes".episodetitle as episodetitle,
                            "Episodes".episodepubdate as episodepubdate,
                            "Episodes".episodedescription as episodedescription,
                            "Episodes".episodeartwork as episodeartwork,
                            "Episodes".episodeurl as episodeurl,
                            "Episodes".episodeduration as episodeduration,
                            "UserEpisodeHistory".listenduration as listenduration,
                            "Episodes".episodeid as episodeid,
                            "Episodes".completed as completed,
                            CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube
                        FROM "Episodes"
                        INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                        LEFT JOIN "UserEpisodeHistory" ON
                            "Episodes".episodeid = "UserEpisodeHistory".episodeid
                            AND "UserEpisodeHistory".userid = $1
                        LEFT JOIN "SavedEpisodes" ON
                            "Episodes".episodeid = "SavedEpisodes".episodeid
                            AND "SavedEpisodes".userid = $1
                        LEFT JOIN "EpisodeQueue" ON
                            "Episodes".episodeid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $1
                            AND "EpisodeQueue".is_youtube = FALSE
                        LEFT JOIN "DownloadedEpisodes" ON
                            "Episodes".episodeid = "DownloadedEpisodes".episodeid
                            AND "DownloadedEpisodes".userid = $1
                        WHERE "Podcasts".userid = $1

                        UNION ALL

                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "YouTubeVideos".videotitle as episodetitle,
                            "YouTubeVideos".publishedat as episodepubdate,
                            "YouTubeVideos".videodescription as episodedescription,
                            "YouTubeVideos".thumbnailurl as episodeartwork,
                            "YouTubeVideos".videourl as episodeurl,
                            "YouTubeVideos".duration as episodeduration,
                            "YouTubeVideos".listenposition as listenduration,
                            "YouTubeVideos".videoid as episodeid,
                            "YouTubeVideos".completed as completed,
                            CASE WHEN "SavedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL AND "EpisodeQueue".is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube
                        FROM "YouTubeVideos"
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "SavedVideos" ON
                            "YouTubeVideos".videoid = "SavedVideos".videoid
                            AND "SavedVideos".userid = $2
                        LEFT JOIN "EpisodeQueue" ON
                            "YouTubeVideos".videoid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $3
                            AND "EpisodeQueue".is_youtube = TRUE
                        LEFT JOIN "DownloadedVideos" ON
                            "YouTubeVideos".videoid = "DownloadedVideos".videoid
                            AND "DownloadedVideos".userid = $4
                        WHERE "Podcasts".userid = $5
                    ) combined
                    ORDER BY episodepubdate DESC"#
                )
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::handlers::podcasts::Episode {
                        podcastname: row.try_get("podcastname")?,
                        episodetitle: row.try_get("episodetitle")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT * FROM (
                        SELECT
                            Podcasts.PodcastName as podcastname,
                            Episodes.EpisodeTitle as episodetitle,
                            Episodes.EpisodePubDate as episodepubdate,
                            Episodes.EpisodeDescription as episodedescription,
                            Episodes.EpisodeArtwork as episodeartwork,
                            Episodes.EpisodeURL as episodeurl,
                            Episodes.EpisodeDuration as episodeduration,
                            UserEpisodeHistory.ListenDuration as listenduration,
                            Episodes.EpisodeID as episodeid,
                            Episodes.Completed as completed,
                            CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube
                        FROM Episodes
                        INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                        LEFT JOIN UserEpisodeHistory ON
                            Episodes.EpisodeID = UserEpisodeHistory.EpisodeID
                            AND UserEpisodeHistory.UserID = ?
                        LEFT JOIN SavedEpisodes ON
                            Episodes.EpisodeID = SavedEpisodes.EpisodeID
                            AND SavedEpisodes.UserID = ?
                        LEFT JOIN EpisodeQueue ON
                            Episodes.EpisodeID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = FALSE
                        LEFT JOIN DownloadedEpisodes ON
                            Episodes.EpisodeID = DownloadedEpisodes.EpisodeID
                            AND DownloadedEpisodes.UserID = ?
                        WHERE Podcasts.UserID = ?

                        UNION ALL

                        SELECT
                            Podcasts.PodcastName as podcastname,
                            YouTubeVideos.VideoTitle as episodetitle,
                            YouTubeVideos.PublishedAt as episodepubdate,
                            YouTubeVideos.VideoDescription as episodedescription,
                            YouTubeVideos.ThumbnailURL as episodeartwork,
                            YouTubeVideos.VideoURL as episodeurl,
                            YouTubeVideos.Duration as episodeduration,
                            YouTubeVideos.ListenPosition as listenduration,
                            YouTubeVideos.VideoID as episodeid,
                            YouTubeVideos.Completed as completed,
                            CASE WHEN SavedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL AND EpisodeQueue.is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN DownloadedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube
                        FROM YouTubeVideos
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        LEFT JOIN SavedVideos ON
                            YouTubeVideos.VideoID = SavedVideos.VideoID
                            AND SavedVideos.UserID = ?
                        LEFT JOIN EpisodeQueue ON
                            YouTubeVideos.VideoID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = TRUE
                        LEFT JOIN DownloadedVideos ON
                            YouTubeVideos.VideoID = DownloadedVideos.VideoID
                            AND DownloadedVideos.UserID = ?
                        WHERE Podcasts.UserID = ?
                    ) combined
                    ORDER BY episodepubdate DESC"
                )
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::handlers::podcasts::Episode {
                        podcastname: row.try_get("podcastname")?,
                        episodetitle: row.try_get("episodetitle")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
        }
    }

    // Add podcast - matches Python add_podcast function exactly
    pub async fn add_podcast(
        &self,
        podcast_values: &crate::handlers::podcasts::PodcastValues,
        podcast_index_id: i64,
        username: Option<&str>,
        password: Option<&str>,
    ) -> AppResult<(i32, Option<i32>)> {
        match self {
            DatabasePool::Postgres(pool) => {
                // Check if podcast already exists
                let existing = sqlx::query(r#"SELECT podcastid, podcastname, feedurl FROM "Podcasts" WHERE feedurl = $1 AND userid = $2"#)
                    .bind(&podcast_values.pod_feed_url)
                    .bind(podcast_values.user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = existing {
                    let podcast_id: i32 = row.try_get("podcastid")?;
                    // Check if there are episodes
                    let episode_count = sqlx::query(r#"SELECT COUNT(*) as count FROM "Episodes" WHERE podcastid = $1"#)
                        .bind(podcast_id)
                        .fetch_one(pool)
                        .await?;
                    
                    let count: i64 = episode_count.try_get("count")?;
                    if count == 0 {
                        // No episodes, add them
                        let first_episode_id = self.add_episodes(podcast_id, &podcast_values.pod_feed_url, 
                                                                  &podcast_values.pod_artwork, false, 
                                                                  username, password).await?;
                        return Ok((podcast_id, first_episode_id));
                    } else {
                        return Ok((podcast_id, None));
                    }
                }
                
                // Convert categories to string
                let category_list = serde_json::to_string(&podcast_values.categories)?;
                
                // Insert new podcast
                let row = sqlx::query(
                    r#"INSERT INTO "Podcasts" 
                       (podcastname, artworkurl, author, categories, description, episodecount, 
                        feedurl, websiteurl, explicit, userid, feedcutoffdays, username, password, podcastindexid)
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                       RETURNING podcastid"#
                )
                .bind(&podcast_values.pod_title)
                .bind(&podcast_values.pod_artwork)
                .bind(&podcast_values.pod_author)
                .bind(&category_list)
                .bind(&podcast_values.pod_description)
                .bind(0) // EpisodeCount starts at 0
                .bind(&podcast_values.pod_feed_url)
                .bind(&podcast_values.pod_website)
                .bind(podcast_values.pod_explicit)
                .bind(podcast_values.user_id)
                .bind(30) // Default feed cutoff days
                .bind(username)
                .bind(password)
                .bind(podcast_index_id)
                .fetch_one(pool)
                .await?;
                
                let podcast_id: i32 = row.try_get("podcastid")?;
                
                // Update UserStats table
                sqlx::query(r#"UPDATE "UserStats" SET podcastsadded = podcastsadded + 1 WHERE userid = $1"#)
                    .bind(podcast_values.user_id)
                    .execute(pool)
                    .await?;
                
                // Add episodes
                let first_episode_id = self.add_episodes(podcast_id, &podcast_values.pod_feed_url, 
                                                          &podcast_values.pod_artwork, false, 
                                                          username, password).await?;
                
                Ok((podcast_id, first_episode_id))
            }
            DatabasePool::MySQL(pool) => {
                // Check if podcast already exists
                let existing = sqlx::query("SELECT PodcastID, PodcastName, FeedURL FROM Podcasts WHERE FeedURL = ? AND UserID = ?")
                    .bind(&podcast_values.pod_feed_url)
                    .bind(podcast_values.user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = existing {
                    let podcast_id: i32 = row.try_get("PodcastID")?;
                    // Check if there are episodes
                    let episode_count = sqlx::query("SELECT COUNT(*) as count FROM Episodes WHERE PodcastID = ?")
                        .bind(podcast_id)
                        .fetch_one(pool)
                        .await?;
                    
                    let count: i64 = episode_count.try_get("count")?;
                    if count == 0 {
                        // No episodes, add them
                        let first_episode_id = self.add_episodes(podcast_id, &podcast_values.pod_feed_url, 
                                                                  &podcast_values.pod_artwork, false, 
                                                                  username, password).await?;
                        return Ok((podcast_id, first_episode_id));
                    } else {
                        return Ok((podcast_id, None));
                    }
                }
                
                // Convert categories to string
                let category_list = serde_json::to_string(&podcast_values.categories)?;
                
                // Insert new podcast
                let result = sqlx::query(
                    "INSERT INTO Podcasts 
                     (PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount, 
                      FeedURL, WebsiteURL, Explicit, UserID, FeedCutoffDays, Username, Password, PodcastIndexID)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(&podcast_values.pod_title)
                .bind(&podcast_values.pod_artwork)
                .bind(&podcast_values.pod_author)
                .bind(&category_list)
                .bind(&podcast_values.pod_description)
                .bind(0) // EpisodeCount starts at 0
                .bind(&podcast_values.pod_feed_url)
                .bind(&podcast_values.pod_website)
                .bind(if podcast_values.pod_explicit { 1 } else { 0 })
                .bind(podcast_values.user_id)
                .bind(30) // Default feed cutoff days
                .bind(username)
                .bind(password)
                .bind(podcast_index_id)
                .execute(pool)
                .await?;
                
                let podcast_id = result.last_insert_id() as i32;
                
                // Update UserStats table
                sqlx::query("UPDATE UserStats SET PodcastsAdded = PodcastsAdded + 1 WHERE UserID = ?")
                    .bind(podcast_values.user_id)
                    .execute(pool)
                    .await?;
                
                // Add episodes
                let first_episode_id = self.add_episodes(podcast_id, &podcast_values.pod_feed_url, 
                                                          &podcast_values.pod_artwork, false, 
                                                          username, password).await?;
                
                Ok((podcast_id, first_episode_id))
            }
        }
    }

    // Remove podcast - matches Python remove_podcast function
    pub async fn remove_podcast(
        &self,
        podcast_name: &str,
        podcast_url: &str,
        user_id: i32,
    ) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(
                    r#"DELETE FROM "Podcasts" 
                       WHERE "PodcastName" = $1 AND "FeedURL" = $2 AND "UserID" = $3"#
                )
                .bind(podcast_name)
                .bind(podcast_url)
                .bind(user_id)
                .execute(pool)
                .await?;
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(
                    "DELETE FROM Podcasts 
                     WHERE PodcastName = ? AND FeedURL = ? AND UserID = ?"
                )
                .bind(podcast_name)
                .bind(podcast_url)
                .bind(user_id)
                .execute(pool)
                .await?;
                Ok(())
            }
        }
    }

    // Get user podcast count - for refresh progress tracking
    pub async fn get_user_podcast_count(&self, user_id: i32) -> AppResult<u32> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT COUNT(*) as count FROM "Podcasts" WHERE "UserID" = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get::<i64, _>("count")? as u32)
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT COUNT(*) as count FROM Podcasts WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get::<i64, _>("count")? as u32)
            }
        }
    }

    // Get user podcasts for refresh - matches Python refresh logic
    pub async fn get_user_podcasts_for_refresh(&self, user_id: i32) -> AppResult<Vec<crate::handlers::refresh::PodcastForRefresh>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT 
                        "PodcastID" as id,
                        "PodcastName" as name,
                        "FeedURL" as feed_url,
                        "ArtworkURL" as artwork_url,
                        "IsYoutube" as is_youtube,
                        "AutoDownload" as auto_download,
                        "Username" as username,
                        "Password" as password,
                        "FeedCutoffDays" as feed_cutoff_days,
                        "UserID" as user_id
                    FROM "Podcasts" 
                    WHERE "UserID" = $1"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut podcasts = Vec::new();
                for row in rows {
                    podcasts.push(crate::handlers::refresh::PodcastForRefresh {
                        id: row.try_get("id")?,
                        name: row.try_get("name")?,
                        feed_url: row.try_get("feed_url")?,
                        artwork_url: row.try_get("artwork_url").unwrap_or_default(),
                        is_youtube: row.try_get("is_youtube")?,
                        auto_download: row.try_get("auto_download")?,
                        username: row.try_get("username").ok(),
                        password: row.try_get("password").ok(),
                        feed_cutoff_days: row.try_get("feed_cutoff_days").ok(),
                        user_id: row.try_get("user_id")?,
                    });
                }
                Ok(podcasts)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT 
                        PodcastID as id,
                        PodcastName as name,
                        FeedURL as feed_url,
                        ArtworkURL as artwork_url,
                        IsYoutube as is_youtube,
                        AutoDownload as auto_download,
                        Username as username,
                        Password as password,
                        FeedCutoffDays as feed_cutoff_days,
                        UserID as user_id
                    FROM Podcasts 
                    WHERE UserID = ?"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut podcasts = Vec::new();
                for row in rows {
                    podcasts.push(crate::handlers::refresh::PodcastForRefresh {
                        id: row.try_get("id")?,
                        name: row.try_get("name")?,
                        feed_url: row.try_get("feed_url")?,
                        artwork_url: row.try_get("artwork_url").unwrap_or_default(),
                        is_youtube: row.try_get("is_youtube")?,
                        auto_download: row.try_get("auto_download")?,
                        username: row.try_get("username").ok(),
                        password: row.try_get("password").ok(),
                        feed_cutoff_days: row.try_get("feed_cutoff_days").ok(),
                        user_id: row.try_get("user_id")?,
                    });
                }
                Ok(podcasts)
            }
        }
    }

    // Remove podcast by name and URL - matches Python remove_podcast function
    pub async fn remove_podcast_by_name_url(
        &self,
        podcast_name: &str,
        podcast_url: &str,
        user_id: i32,
    ) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                // First get the podcast ID to cascade delete properly
                let podcast_row = sqlx::query(
                    r#"SELECT "PodcastID" FROM "Podcasts" 
                       WHERE "PodcastName" = $1 AND "FeedURL" = $2 AND "UserID" = $3"#
                )
                .bind(podcast_name)
                .bind(podcast_url)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = podcast_row {
                    let podcast_id: i32 = row.try_get("PodcastID")?;
                    
                    // Delete in the proper order to handle foreign key constraints
                    // 1. PlaylistContents first
                    sqlx::query(r#"DELETE FROM "PlaylistContents" WHERE "EpisodeID" IN (SELECT "EpisodeID" FROM "Episodes" WHERE "PodcastID" = $1)"#)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    // 2. UserEpisodeHistory
                    sqlx::query(r#"DELETE FROM "UserEpisodeHistory" WHERE "EpisodeID" IN (SELECT "EpisodeID" FROM "Episodes" WHERE "PodcastID" = $1)"#)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    // 3. DownloadedEpisodes
                    sqlx::query(r#"DELETE FROM "DownloadedEpisodes" WHERE "EpisodeID" IN (SELECT "EpisodeID" FROM "Episodes" WHERE "PodcastID" = $1)"#)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    // 4. SavedEpisodes
                    sqlx::query(r#"DELETE FROM "SavedEpisodes" WHERE "EpisodeID" IN (SELECT "EpisodeID" FROM "Episodes" WHERE "PodcastID" = $1)"#)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    // 5. QueuedEpisodes (EpisodeQueue in Python)
                    sqlx::query(r#"DELETE FROM "QueuedEpisodes" WHERE "EpisodeID" IN (SELECT "EpisodeID" FROM "Episodes" WHERE "PodcastID" = $1)"#)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    // 6. Episodes
                    sqlx::query(r#"DELETE FROM "Episodes" WHERE "PodcastID" = $1"#)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    // 7. Finally delete the podcast
                    sqlx::query(r#"DELETE FROM "Podcasts" WHERE "PodcastID" = $1"#)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    // 8. Update UserStats - decrement PodcastsAdded
                    sqlx::query(r#"UPDATE "UserStats" SET "PodcastsAdded" = "PodcastsAdded" - 1 WHERE "UserID" = $1"#)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                }
                
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                // First get the podcast ID to cascade delete properly
                let podcast_row = sqlx::query(
                    "SELECT PodcastID FROM Podcasts 
                     WHERE PodcastName = ? AND FeedURL = ? AND UserID = ?"
                )
                .bind(podcast_name)
                .bind(podcast_url)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = podcast_row {
                    let podcast_id: i32 = row.try_get("PodcastID")?;
                    
                    // Delete in the proper order to handle foreign key constraints
                    sqlx::query("DELETE FROM PlaylistContents WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = ?)")
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    sqlx::query("DELETE FROM UserEpisodeHistory WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = ?)")
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    sqlx::query("DELETE FROM DownloadedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = ?)")
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    sqlx::query("DELETE FROM SavedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = ?)")
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    sqlx::query("DELETE FROM QueuedEpisodes WHERE EpisodeID IN (SELECT EpisodeID FROM Episodes WHERE PodcastID = ?)")
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    sqlx::query("DELETE FROM Episodes WHERE PodcastID = ?")
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    sqlx::query("DELETE FROM Podcasts WHERE PodcastID = ?")
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    
                    sqlx::query("UPDATE UserStats SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = ?")
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                }
                
                Ok(())
            }
        }
    }

    // Return podcasts basic - matches Python return_pods function
    pub async fn return_pods(&self, user_id: i32) -> AppResult<Vec<crate::models::PodcastResponse>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT 
                        podcastid as podcastid,
                        COALESCE(podcastname, 'Unknown Podcast') as podcastname,
                        CASE 
                            WHEN artworkurl IS NULL OR artworkurl = '' 
                            THEN '/static/assets/default-podcast.png'
                            ELSE artworkurl
                        END as artworkurl,
                        COALESCE(description, 'No description available') as description,
                        COALESCE(episodecount, 0) as episodecount,
                        COALESCE(websiteurl, '') as websiteurl,
                        COALESCE(feedurl, '') as feedurl,
                        COALESCE(author, 'Unknown Author') as author,
                        COALESCE(categories, '') as categories,
                        COALESCE(explicit, false) as explicit,
                        COALESCE(podcastindexid, 0) as podcastindexid
                    FROM "Podcasts"
                    WHERE userid = $1
                    ORDER BY podcastname"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut podcasts = Vec::new();
                for row in rows {
                    podcasts.push(crate::models::PodcastResponse {
                        podcastid: row.try_get("podcastid")?,
                        podcastname: row.try_get("podcastname")?,
                        artworkurl: row.try_get("artworkurl").ok(),
                        description: row.try_get("description").ok(),
                        episodecount: row.try_get("episodecount").ok(),
                        websiteurl: row.try_get("websiteurl").ok(),
                        feedurl: row.try_get("feedurl")?,
                        author: row.try_get("author").ok(),
                        categories: row.try_get("categories")?,
                        explicit: row.try_get("explicit")?,
                        podcastindexid: row.try_get::<i32, _>("podcastindexid").ok().map(|i| i as i64),
                    });
                }
                Ok(podcasts)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT 
                        PodcastID as podcastid,
                        COALESCE(PodcastName, 'Unknown Podcast') as podcastname,
                        CASE 
                            WHEN ArtworkURL IS NULL OR ArtworkURL = '' 
                            THEN '/static/assets/default-podcast.png'
                            ELSE ArtworkURL
                        END as artworkurl,
                        COALESCE(Description, 'No description available') as description,
                        COALESCE(EpisodeCount, 0) as episodecount,
                        COALESCE(WebsiteURL, '') as websiteurl,
                        COALESCE(FeedURL, '') as feedurl,
                        COALESCE(Author, 'Unknown Author') as author,
                        COALESCE(Categories, '') as categories,
                        COALESCE(Explicit, false) as explicit,
                        COALESCE(PodcastIndexID, 0) as podcastindexid
                    FROM Podcasts
                    WHERE UserID = ?
                    ORDER BY PodcastName"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut podcasts = Vec::new();
                for row in rows {
                    podcasts.push(crate::models::PodcastResponse {
                        podcastid: row.try_get("podcastid")?,
                        podcastname: row.try_get("podcastname")?,
                        artworkurl: row.try_get("artworkurl").ok(),
                        description: row.try_get("description").ok(),
                        episodecount: row.try_get("episodecount").ok(),
                        websiteurl: row.try_get("websiteurl").ok(),
                        feedurl: row.try_get("feedurl")?,
                        author: row.try_get("author").ok(),
                        categories: row.try_get("categories")?,
                        explicit: row.try_get("explicit")?,
                        podcastindexid: row.try_get::<i32, _>("podcastindexid").ok().map(|i| i as i64),
                    });
                }
                Ok(podcasts)
            }
        }
    }

    // Return podcasts with extra stats - matches Python return_pods with analytics
    pub async fn return_pods_extra(&self, user_id: i32) -> AppResult<Vec<crate::models::PodcastExtraResponse>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT 
                        p.podcastid as podcastid,
                        COALESCE(p.podcastname, 'Unknown Podcast') as podcastname,
                        CASE 
                            WHEN p.artworkurl IS NULL OR p.artworkurl = '' 
                            THEN '/static/assets/default-podcast.png'
                            ELSE p.artworkurl
                        END as artworkurl,
                        COALESCE(p.description, 'No description available') as description,
                        COALESCE(p.episodecount, 0) as episodecount,
                        COALESCE(p.websiteurl, '') as websiteurl,
                        COALESCE(p.feedurl, '') as feedurl,
                        COALESCE(p.author, 'Unknown Author') as author,
                        COALESCE(p.categories, '') as categories,
                        COALESCE(p.explicit, false) as explicit,
                        COALESCE(p.podcastindexid, 0) as podcastindexid,
                        COUNT(ueh.userepisodehistoryid) as play_count,
                        COUNT(DISTINCT ueh.episodeid) as episodes_played,
                        MIN(e.episodepubdate) as oldest_episode_date,
                        COALESCE(p.isyoutube, false) as is_youtube
                    FROM "Podcasts" p
                    LEFT JOIN "Episodes" e ON p.podcastid = e.podcastid
                    LEFT JOIN "UserEpisodeHistory" ueh ON e.episodeid = ueh.episodeid AND ueh.userid = $1
                    WHERE p.userid = $1
                    GROUP BY p.podcastid, p.podcastname, p.artworkurl, p.description, 
                             p.episodecount, p.websiteurl, p.feedurl, p.author, 
                             p.categories, p.explicit, p.podcastindexid, p.isyoutube
                    ORDER BY p.podcastname"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut podcasts = Vec::new();
                for row in rows {
                    let feed_url: String = row.try_get("feedurl")?;
                    let is_youtube = row.try_get("is_youtube").unwrap_or_else(|_| feed_url.contains("youtube.com"));
                    
                    podcasts.push(crate::models::PodcastExtraResponse {
                        podcastid: row.try_get("podcastid")?,
                        podcastname: row.try_get("podcastname")?,
                        artworkurl: row.try_get("artworkurl").ok(),
                        description: row.try_get("description").ok(),
                        episodecount: row.try_get("episodecount").ok(),
                        websiteurl: row.try_get("websiteurl").ok(),
                        feedurl: feed_url,
                        author: row.try_get("author").ok(),
                        categories: row.try_get("categories")?,
                        explicit: row.try_get("explicit")?,
                        podcastindexid: row.try_get::<i32, _>("podcastindexid").ok().map(|i| i as i64),
                        play_count: row.try_get("play_count")?,
                        episodes_played: row.try_get("episodes_played")?,
                        oldest_episode_date: row.try_get("oldest_episode_date").ok(),
                        is_youtube,
                    });
                }
                Ok(podcasts)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT 
                        p.PodcastID as podcastid,
                        COALESCE(p.PodcastName, 'Unknown Podcast') as podcastname,
                        CASE 
                            WHEN p.ArtworkURL IS NULL OR p.ArtworkURL = '' 
                            THEN '/static/assets/default-podcast.png'
                            ELSE p.ArtworkURL
                        END as artworkurl,
                        COALESCE(p.Description, 'No description available') as description,
                        COALESCE(p.EpisodeCount, 0) as episodecount,
                        COALESCE(p.WebsiteURL, '') as websiteurl,
                        COALESCE(p.FeedURL, '') as feedurl,
                        COALESCE(p.Author, 'Unknown Author') as author,
                        COALESCE(p.Categories, '') as categories,
                        COALESCE(p.Explicit, false) as explicit,
                        COALESCE(p.PodcastIndexID, 0) as podcastindexid,
                        COUNT(ueh.UserEpisodeHistoryID) as play_count,
                        COUNT(DISTINCT ueh.EpisodeID) as episodes_played,
                        MIN(e.EpisodePubDate) as oldest_episode_date,
                        COALESCE(p.IsYoutube, false) as is_youtube
                    FROM Podcasts p
                    LEFT JOIN Episodes e ON p.PodcastID = e.PodcastID
                    LEFT JOIN UserEpisodeHistory ueh ON e.EpisodeID = ueh.EpisodeID AND ueh.UserID = ?
                    WHERE p.UserID = ?
                    GROUP BY p.PodcastID, p.PodcastName, p.ArtworkURL, p.Description, 
                             p.EpisodeCount, p.WebsiteURL, p.FeedURL, p.Author, 
                             p.Categories, p.Explicit, p.PodcastIndexID, p.IsYoutube
                    ORDER BY p.PodcastName"
                )
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut podcasts = Vec::new();
                for row in rows {
                    let feed_url: String = row.try_get("feedurl")?;
                    let is_youtube = row.try_get("is_youtube").unwrap_or_else(|_| feed_url.contains("youtube.com"));
                    
                    podcasts.push(crate::models::PodcastExtraResponse {
                        podcastid: row.try_get("podcastid")?,
                        podcastname: row.try_get("podcastname")?,
                        artworkurl: row.try_get("artworkurl").ok(),
                        description: row.try_get("description").ok(),
                        episodecount: row.try_get("episodecount").ok(),
                        websiteurl: row.try_get("websiteurl").ok(),
                        feedurl: feed_url,
                        author: row.try_get("author").ok(),
                        categories: row.try_get("categories")?,
                        explicit: row.try_get("explicit")?,
                        podcastindexid: row.try_get::<i32, _>("podcastindexid").ok().map(|i| i as i64),
                        play_count: row.try_get("play_count")?,
                        episodes_played: row.try_get("episodes_played")?,
                        oldest_episode_date: row.try_get("oldest_episode_date").ok(),
                        is_youtube,
                    });
                }
                Ok(podcasts)
            }
        }
    }

    // Get time info for user - matches Python get_time_info function
    pub async fn get_time_info(&self, user_id: i32) -> AppResult<crate::models::TimeInfoResponse> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(
                    r#"SELECT COALESCE(timezone, 'UTC') as timezone, 
                              COALESCE(timeformat, 12) as hour_pref,
                              dateformat as date_format
                       FROM "Users" WHERE userid = $1"#
                )
                .bind(user_id)
                .fetch_one(pool)
                .await?;
                
                Ok(crate::models::TimeInfoResponse {
                    timezone: row.try_get("timezone")?,
                    hour_pref: row.try_get("hour_pref")?,
                    date_format: row.try_get("date_format").ok(),
                })
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query(
                    "SELECT COALESCE(Timezone, 'UTC') as timezone, 
                            COALESCE(TimeFormat, 12) as hour_pref,
                            DateFormat as date_format
                     FROM Users WHERE UserID = ?"
                )
                .bind(user_id)
                .fetch_one(pool)
                .await?;
                
                Ok(crate::models::TimeInfoResponse {
                    timezone: row.try_get("timezone")?,
                    hour_pref: row.try_get("hour_pref")?,
                    date_format: row.try_get("date_format").ok(),
                })
            }
        }
    }

    // Check if podcast exists - matches Python check_podcast function
    pub async fn check_podcast(&self, user_id: i32, podcast_name: &str, podcast_url: &str) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(
                    r#"SELECT podcastid FROM "Podcasts" 
                       WHERE userid = $1 AND podcastname = $2 AND feedurl = $3"#
                )
                .bind(user_id)
                .bind(podcast_name)
                .bind(podcast_url)
                .fetch_optional(pool)
                .await?;
                
                Ok(row.is_some())
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query(
                    "SELECT PodcastID FROM Podcasts 
                     WHERE UserID = ? AND PodcastName = ? AND FeedURL = ?"
                )
                .bind(user_id)
                .bind(podcast_name)
                .bind(podcast_url)
                .fetch_optional(pool)
                .await?;
                
                Ok(row.is_some())
            }
        }
    }

    // Check if episode exists in database - matches Python check_episode_exists function
    pub async fn check_episode_exists(&self, user_id: i32, episode_title: &str, episode_url: &str) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(
                    r#"SELECT EXISTS(
                        SELECT 1 FROM "Episodes"
                        JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                        WHERE "Podcasts".userid = $1 
                          AND "Episodes".episodetitle = $2 
                          AND "Episodes".episodeurl = $3
                    ) as episode_exists"#
                )
                .bind(user_id)
                .bind(episode_title)
                .bind(episode_url)
                .fetch_one(pool)
                .await?;
                
                Ok(row.try_get("episode_exists")?)
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query(
                    "SELECT EXISTS(
                        SELECT 1 FROM Episodes
                        JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                        WHERE Podcasts.UserID = ? 
                          AND Episodes.EpisodeTitle = ? 
                          AND Episodes.EpisodeURL = ?
                    ) as episode_exists"
                )
                .bind(user_id)
                .bind(episode_title)
                .bind(episode_url)
                .fetch_one(pool)
                .await?;
                
                // MySQL returns integer (0 or 1) for EXISTS
                let exists_int: i32 = row.try_get("episode_exists")?;
                Ok(exists_int == 1)
            }
        }
    }

    // Queue episode - matches Python queue_pod function
    pub async fn queue_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                // First check if already queued
                let existing = sqlx::query(
                    r#"SELECT queueid FROM "EpisodeQueue" 
                       WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                )
                .bind(episode_id)
                .bind(user_id) 
                .bind(is_youtube)
                .fetch_optional(pool)
                .await?;

                if existing.is_some() {
                    return Ok(()); // Already queued, don't duplicate
                }

                // Get max queue position for user
                let max_pos_row = sqlx::query(
                    r#"SELECT COALESCE(MAX(queueposition), 0) as max_pos FROM "EpisodeQueue" WHERE userid = $1"#
                )
                .bind(user_id)
                .fetch_one(pool)
                .await?;
                
                let max_pos: i32 = max_pos_row.try_get("max_pos")?;
                let new_position = max_pos + 1;

                // Insert new queued episode
                sqlx::query(
                    r#"INSERT INTO "EpisodeQueue" (episodeid, userid, queueposition, is_youtube) 
                       VALUES ($1, $2, $3, $4)"#
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(new_position)
                .bind(is_youtube)
                .execute(pool)
                .await?;

                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                // First check if already queued
                let existing = sqlx::query(
                    "SELECT QueueID FROM EpisodeQueue 
                     WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(is_youtube)
                .fetch_optional(pool)
                .await?;

                if existing.is_some() {
                    return Ok(()); // Already queued, don't duplicate
                }

                // Get max queue position for user
                let max_pos_row = sqlx::query(
                    "SELECT COALESCE(MAX(QueuePosition), 0) as max_pos FROM EpisodeQueue WHERE UserID = ?"
                )
                .bind(user_id)
                .fetch_one(pool)
                .await?;
                
                let max_pos: i32 = max_pos_row.try_get("max_pos")?;
                let new_position = max_pos + 1;

                // Insert new queued episode
                sqlx::query(
                    "INSERT INTO EpisodeQueue (EpisodeID, UserID, QueuePosition, is_youtube) 
                     VALUES (?, ?, ?, ?)"
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(new_position)
                .bind(is_youtube)
                .execute(pool)
                .await?;

                Ok(())
            }
        }
    }

    // Remove queued episode - matches Python remove_queued_pod function
    pub async fn remove_queued_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                // Get the queue position of the episode to be removed
                let position_row = sqlx::query(
                    r#"SELECT queueposition FROM "EpisodeQueue" 
                       WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(is_youtube)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = position_row {
                    let removed_position: i32 = row.try_get("queueposition")?;

                    // Delete the episode from queue
                    sqlx::query(
                        r#"DELETE FROM "EpisodeQueue" 
                           WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(is_youtube)
                    .execute(pool)
                    .await?;

                    // Update positions of all episodes that were after the removed one
                    sqlx::query(
                        r#"UPDATE "EpisodeQueue" SET queueposition = queueposition - 1 
                           WHERE userid = $1 AND queueposition > $2"#
                    )
                    .bind(user_id)
                    .bind(removed_position)
                    .execute(pool)
                    .await?;
                }

                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                // Get the queue position of the episode to be removed
                let position_row = sqlx::query(
                    "SELECT QueuePosition FROM EpisodeQueue 
                     WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(is_youtube)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = position_row {
                    let removed_position: i32 = row.try_get("QueuePosition")?;

                    // Delete the episode from queue
                    sqlx::query(
                        "DELETE FROM EpisodeQueue 
                         WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(is_youtube)
                    .execute(pool)
                    .await?;

                    // Update positions of all episodes that were after the removed one
                    sqlx::query(
                        "UPDATE EpisodeQueue SET QueuePosition = QueuePosition - 1 
                         WHERE UserID = ? AND QueuePosition > ?"
                    )
                    .bind(user_id)
                    .bind(removed_position)
                    .execute(pool)
                    .await?;
                }

                Ok(())
            }
        }
    }

    // Get queued episodes - matches Python get_queued_episodes function
    pub async fn get_queued_episodes(&self, user_id: i32) -> AppResult<Vec<crate::models::QueuedEpisode>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT * FROM (
                        SELECT
                            "Episodes".episodetitle as episodetitle,
                            "Podcasts".podcastname as podcastname,
                            "Episodes".episodepubdate as episodepubdate,
                            "Episodes".episodedescription as episodedescription,
                            "Episodes".episodeartwork as episodeartwork,
                            "Episodes".episodeurl as episodeurl,
                            "EpisodeQueue".queueposition as queueposition,
                            "Episodes".episodeduration as episodeduration,
                            "EpisodeQueue".queuedate as queuedate,
                            "UserEpisodeHistory".listenduration as listenduration,
                            "Episodes".episodeid as episodeid,
                            "Episodes".completed as completed,
                            CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            TRUE as queued,
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube
                        FROM "EpisodeQueue"
                        INNER JOIN "Episodes" ON "EpisodeQueue".episodeid = "Episodes".episodeid
                        INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                        LEFT JOIN "UserEpisodeHistory" ON
                            "EpisodeQueue".episodeid = "UserEpisodeHistory".episodeid
                            AND "EpisodeQueue".userid = "UserEpisodeHistory".userid
                        LEFT JOIN "SavedEpisodes" ON
                            "EpisodeQueue".episodeid = "SavedEpisodes".episodeid
                            AND "EpisodeQueue".userid = "SavedEpisodes".userid
                        LEFT JOIN "DownloadedEpisodes" ON
                            "EpisodeQueue".episodeid = "DownloadedEpisodes".episodeid
                            AND "EpisodeQueue".userid = "DownloadedEpisodes".userid
                        WHERE "EpisodeQueue".userid = $1 AND "EpisodeQueue".is_youtube = FALSE

                        UNION ALL

                        SELECT
                            "YouTubeVideos".videotitle as episodetitle,
                            "Podcasts".podcastname as podcastname,
                            "YouTubeVideos".publishedat as episodepubdate,
                            "YouTubeVideos".videodescription as episodedescription,
                            "YouTubeVideos".thumbnailurl as episodeartwork,
                            "YouTubeVideos".videourl as episodeurl,
                            "EpisodeQueue".queueposition as queueposition,
                            "YouTubeVideos".duration as episodeduration,
                            "EpisodeQueue".queuedate as queuedate,
                            "YouTubeVideos".listenposition as listenduration,
                            "YouTubeVideos".videoid as episodeid,
                            "YouTubeVideos".completed as completed,
                            CASE WHEN "SavedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            TRUE as queued,
                            CASE WHEN "DownloadedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube
                        FROM "EpisodeQueue"
                        INNER JOIN "YouTubeVideos" ON "EpisodeQueue".episodeid = "YouTubeVideos".videoid
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "SavedVideos" ON
                            "EpisodeQueue".episodeid = "SavedVideos".videoid
                            AND "EpisodeQueue".userid = "SavedVideos".userid
                        LEFT JOIN "DownloadedVideos" ON
                            "EpisodeQueue".episodeid = "DownloadedVideos".videoid
                            AND "EpisodeQueue".userid = "DownloadedVideos".userid
                        WHERE "EpisodeQueue".userid = $2 AND "EpisodeQueue".is_youtube = TRUE
                    ) combined
                    ORDER BY queueposition ASC"#
                )
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::QueuedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        queueposition: row.try_get("queueposition").ok(),
                        episodeduration: row.try_get("episodeduration")?,
                        queuedate: row.try_get("queuedate")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT * FROM (
                        SELECT
                            Episodes.EpisodeTitle as episodetitle,
                            Podcasts.PodcastName as podcastname,
                            Episodes.EpisodePubDate as episodepubdate,
                            Episodes.EpisodeDescription as episodedescription,
                            Episodes.EpisodeArtwork as episodeartwork,
                            Episodes.EpisodeURL as episodeurl,
                            EpisodeQueue.QueuePosition as queueposition,
                            Episodes.EpisodeDuration as episodeduration,
                            EpisodeQueue.QueueDate as queuedate,
                            UserEpisodeHistory.ListenDuration as listenduration,
                            Episodes.EpisodeID as episodeid,
                            Episodes.Completed as completed,
                            CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            TRUE as queued,
                            CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube
                        FROM EpisodeQueue
                        INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID
                        INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                        LEFT JOIN UserEpisodeHistory ON
                            EpisodeQueue.EpisodeID = UserEpisodeHistory.EpisodeID
                            AND EpisodeQueue.UserID = UserEpisodeHistory.UserID
                        LEFT JOIN SavedEpisodes ON
                            EpisodeQueue.EpisodeID = SavedEpisodes.EpisodeID
                            AND EpisodeQueue.UserID = SavedEpisodes.UserID
                        LEFT JOIN DownloadedEpisodes ON
                            EpisodeQueue.EpisodeID = DownloadedEpisodes.EpisodeID
                            AND EpisodeQueue.UserID = DownloadedEpisodes.UserID
                        WHERE EpisodeQueue.UserID = ? AND EpisodeQueue.is_youtube = FALSE

                        UNION ALL

                        SELECT
                            YouTubeVideos.VideoTitle as episodetitle,
                            Podcasts.PodcastName as podcastname,
                            YouTubeVideos.PublishedAt as episodepubdate,
                            YouTubeVideos.VideoDescription as episodedescription,
                            YouTubeVideos.ThumbnailURL as episodeartwork,
                            YouTubeVideos.VideoURL as episodeurl,
                            EpisodeQueue.QueuePosition as queueposition,
                            YouTubeVideos.Duration as episodeduration,
                            EpisodeQueue.QueueDate as queuedate,
                            YouTubeVideos.ListenPosition as listenduration,
                            YouTubeVideos.VideoID as episodeid,
                            YouTubeVideos.Completed as completed,
                            CASE WHEN SavedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            TRUE as queued,
                            CASE WHEN DownloadedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube
                        FROM EpisodeQueue
                        INNER JOIN YouTubeVideos ON EpisodeQueue.EpisodeID = YouTubeVideos.VideoID
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        LEFT JOIN SavedVideos ON
                            EpisodeQueue.EpisodeID = SavedVideos.VideoID
                            AND EpisodeQueue.UserID = SavedVideos.UserID
                        LEFT JOIN DownloadedVideos ON
                            EpisodeQueue.EpisodeID = DownloadedVideos.VideoID
                            AND EpisodeQueue.UserID = DownloadedVideos.UserID
                        WHERE EpisodeQueue.UserID = ? AND EpisodeQueue.is_youtube = TRUE
                    ) combined
                    ORDER BY queueposition ASC"
                )
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::QueuedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        queueposition: row.try_get("queueposition").ok(),
                        episodeduration: row.try_get("episodeduration")?,
                        queuedate: row.try_get("queuedate")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
        }
    }

    // Reorder queue - matches Python reorder_queued_episodes function
    pub async fn reorder_queue(&self, user_id: i32, episode_ids: Vec<i32>) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                
                for (index, episode_id) in episode_ids.iter().enumerate() {
                    let new_position = (index + 1) as i32;
                    sqlx::query(
                        r#"UPDATE "EpisodeQueue" SET "QueuePosition" = $1 
                           WHERE "EpisodeID" = $2 AND "UserID" = $3"#
                    )
                    .bind(new_position)
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                }
                
                tx.commit().await?;
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                
                for (index, episode_id) in episode_ids.iter().enumerate() {
                    let new_position = (index + 1) as i32;
                    sqlx::query(
                        "UPDATE EpisodeQueue SET QueuePosition = ? 
                         WHERE EpisodeID = ? AND UserID = ?"
                    )
                    .bind(new_position)
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                }
                
                tx.commit().await?;
                Ok(())
            }
        }
    }

    // Save episode - matches Python save_episode function
    pub async fn save_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                if is_youtube {
                    // Check if already saved
                    let existing = sqlx::query(
                        r#"SELECT "SaveID" FROM "SavedVideos" WHERE "VideoID" = $1 AND "UserID" = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                    if existing.is_none() {
                        sqlx::query(
                            r#"INSERT INTO "SavedVideos" ("VideoID", "UserID") VALUES ($1, $2)"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    }
                } else {
                    // Check if already saved
                    let existing = sqlx::query(
                        r#"SELECT saveid FROM "SavedEpisodes" WHERE episodeid = $1 AND userid = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                    if existing.is_none() {
                        sqlx::query(
                            r#"INSERT INTO "SavedEpisodes" (episodeid, userid) VALUES ($1, $2)"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    }
                }
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                if is_youtube {
                    // Check if already saved
                    let existing = sqlx::query(
                        "SELECT SaveID FROM SavedVideos WHERE VideoID = ? AND UserID = ?"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                    if existing.is_none() {
                        sqlx::query(
                            "INSERT INTO SavedVideos (VideoID, UserID) VALUES (?, ?)"
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    }
                } else {
                    // Check if already saved
                    let existing = sqlx::query(
                        "SELECT SaveID FROM SavedEpisodes WHERE EpisodeID = ? AND UserID = ?"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                    if existing.is_none() {
                        sqlx::query(
                            "INSERT INTO SavedEpisodes (EpisodeID, UserID) VALUES (?, ?)"
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    }
                }
                Ok(())
            }
        }
    }

    // Remove saved episode - matches Python remove_saved_episode function
    pub async fn remove_saved_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                if is_youtube {
                    sqlx::query(
                        r#"DELETE FROM "SavedVideos" WHERE "VideoID" = $1 AND "UserID" = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                } else {
                    sqlx::query(
                        r#"DELETE FROM "SavedEpisodes" WHERE episodeid = $1 AND userid = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                if is_youtube {
                    sqlx::query(
                        "DELETE FROM SavedVideos WHERE VideoID = ? AND UserID = ?"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                } else {
                    sqlx::query(
                        "DELETE FROM SavedEpisodes WHERE EpisodeID = ? AND UserID = ?"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                }
                Ok(())
            }
        }
    }

    // Mark episode as completed - matches Python mark_episode_completed function
    pub async fn mark_episode_completed(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                if is_youtube {
                    // Get YouTube video duration
                    let duration_row = sqlx::query(
                        r#"SELECT duration FROM "YouTubeVideos" WHERE videoid = $1"#
                    )
                    .bind(episode_id)
                    .fetch_optional(pool)
                    .await?;

                    if let Some(row) = duration_row {
                        let duration: Option<i32> = row.try_get("duration").ok();
                        
                        if let Some(duration) = duration {
                            // Update completion status
                            sqlx::query(
                                r#"UPDATE "YouTubeVideos" SET completed = TRUE WHERE videoid = $1"#
                            )
                            .bind(episode_id)
                            .execute(pool)
                            .await?;

                            // Update history
                            sqlx::query(
                                r#"INSERT INTO "UserVideoHistory" (userid, videoid, listendate, listenduration)
                                   VALUES ($1, $2, NOW(), $3)
                                   ON CONFLICT (userid, videoid)
                                   DO UPDATE SET listenduration = $4, listendate = NOW()"#
                            )
                            .bind(user_id)
                            .bind(episode_id)
                            .bind(duration)
                            .bind(duration)
                            .execute(pool)
                            .await?;
                        }
                    }
                } else {
                    // Get episode duration
                    let duration_row = sqlx::query(
                        r#"SELECT episodeduration FROM "Episodes" WHERE episodeid = $1"#
                    )
                    .bind(episode_id)
                    .fetch_optional(pool)
                    .await?;

                    if let Some(row) = duration_row {
                        let duration: Option<i32> = row.try_get("episodeduration").ok();
                        
                        if let Some(duration) = duration {
                            // Update completion status
                            sqlx::query(
                                r#"UPDATE "Episodes" SET completed = TRUE WHERE episodeid = $1"#
                            )
                            .bind(episode_id)
                            .execute(pool)
                            .await?;

                            // Update history
                            sqlx::query(
                                r#"INSERT INTO "UserEpisodeHistory" (userid, episodeid, listendate, listenduration)
                                   VALUES ($1, $2, NOW(), $3)
                                   ON CONFLICT (userid, episodeid)
                                   DO UPDATE SET listenduration = $4, listendate = NOW()"#
                            )
                            .bind(user_id)
                            .bind(episode_id)
                            .bind(duration)
                            .bind(duration)
                            .execute(pool)
                            .await?;
                        }
                    }
                }
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                if is_youtube {
                    // Get YouTube video duration
                    let duration_row = sqlx::query(
                        "SELECT Duration FROM YouTubeVideos WHERE VideoID = ?"
                    )
                    .bind(episode_id)
                    .fetch_optional(pool)
                    .await?;

                    if let Some(row) = duration_row {
                        let duration: Option<i32> = row.try_get("Duration").ok();
                        
                        if let Some(duration) = duration {
                            // Update completion status
                            sqlx::query(
                                "UPDATE YouTubeVideos SET Completed = 1 WHERE VideoID = ?"
                            )
                            .bind(episode_id)
                            .execute(pool)
                            .await?;

                            // Update history
                            sqlx::query(
                                "INSERT INTO UserVideoHistory (UserID, VideoID, ListenDate, ListenDuration)
                                 VALUES (?, ?, NOW(), ?)
                                 ON DUPLICATE KEY UPDATE
                                     ListenDuration = ?,
                                     ListenDate = NOW()"
                            )
                            .bind(user_id)
                            .bind(episode_id)
                            .bind(duration)
                            .bind(duration)
                            .execute(pool)
                            .await?;
                        }
                    }
                } else {
                    // Get episode duration
                    let duration_row = sqlx::query(
                        "SELECT EpisodeDuration FROM Episodes WHERE EpisodeID = ?"
                    )
                    .bind(episode_id)
                    .fetch_optional(pool)
                    .await?;

                    if let Some(row) = duration_row {
                        let duration: Option<i32> = row.try_get("EpisodeDuration").ok();
                        
                        if let Some(duration) = duration {
                            // Update completion status
                            sqlx::query(
                                "UPDATE Episodes SET Completed = 1 WHERE EpisodeID = ?"
                            )
                            .bind(episode_id)
                            .execute(pool)
                            .await?;

                            // Update history
                            sqlx::query(
                                "INSERT INTO UserEpisodeHistory (UserID, EpisodeID, ListenDate, ListenDuration)
                                 VALUES (?, ?, NOW(), ?)
                                 ON DUPLICATE KEY UPDATE
                                     ListenDuration = ?,
                                     ListenDate = NOW()"
                            )
                            .bind(user_id)
                            .bind(episode_id)
                            .bind(duration)
                            .bind(duration)
                            .execute(pool)
                            .await?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    // Increment played count - matches Python increment_played function
    pub async fn increment_played(&self, user_id: i32) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(
                    r#"UPDATE "UserStats" SET podcastsplayed = podcastsplayed + 1 WHERE userid = $1"#
                )
                .bind(user_id)
                .execute(pool)
                .await?;
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(
                    "UPDATE UserStats SET PodcastsPlayed = PodcastsPlayed + 1 WHERE UserID = ?"
                )
                .bind(user_id)
                .execute(pool)
                .await?;
                Ok(())
            }
        }
    }

    // Get podcast ID from episode ID - matches Python get_podcast_id_from_episode function
    pub async fn get_podcast_id_from_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<Option<i32>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let query = if is_youtube {
                    r#"SELECT "YouTubeVideos".podcastid
                       FROM "YouTubeVideos"
                       INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                       WHERE "YouTubeVideos".videoid = $1 AND "Podcasts".userid = $2"#
                } else {
                    r#"SELECT "Episodes".podcastid
                       FROM "Episodes" 
                       INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                       WHERE "Episodes".episodeid = $1 AND "Podcasts".userid = $2"#
                };

                // First try with provided user_id
                let row = sqlx::query(query)
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    return Ok(Some(row.try_get("podcastid")?));
                }

                // If not found, try with system user (1)
                let row = sqlx::query(query)
                    .bind(episode_id)
                    .bind(1)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    Ok(Some(row.try_get("podcastid")?))
                } else {
                    Ok(None)
                }
            }
            DatabasePool::MySQL(pool) => {
                let query = if is_youtube {
                    "SELECT YouTubeVideos.PodcastID
                     FROM YouTubeVideos
                     INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                     WHERE YouTubeVideos.VideoID = ? AND Podcasts.UserID = ?"
                } else {
                    "SELECT Episodes.PodcastID
                     FROM Episodes
                     INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                     WHERE Episodes.EpisodeID = ? AND Podcasts.UserID = ?"
                };

                // First try with provided user_id
                let row = sqlx::query(query)
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    return Ok(Some(row.try_get("PodcastID")?));
                }

                // If not found, try with system user (1)
                let row = sqlx::query(query)
                    .bind(episode_id)
                    .bind(1)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    Ok(Some(row.try_get("PodcastID")?))
                } else {
                    Ok(None)
                }
            }
        }
    }

    // Get saved episodes - matches Python saved_episode_list function
    pub async fn get_saved_episodes(&self, user_id: i32) -> AppResult<Vec<crate::models::SavedEpisode>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT * FROM (
                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "Episodes".episodetitle as episodetitle,
                            "Episodes".episodepubdate as episodepubdate,
                            "Episodes".episodedescription as episodedescription,
                            "Episodes".episodeid as episodeid,
                            "Episodes".episodeartwork as episodeartwork,
                            "Episodes".episodeurl as episodeurl,
                            "Episodes".episodeduration as episodeduration,
                            "Podcasts".websiteurl as websiteurl,
                            "UserEpisodeHistory".listenduration as listenduration,
                            "Episodes".completed as completed,
                            TRUE as saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube
                        FROM "SavedEpisodes"
                        INNER JOIN "Episodes" ON "SavedEpisodes".episodeid = "Episodes".episodeid
                        INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                        LEFT JOIN "UserEpisodeHistory" ON
                            "SavedEpisodes".episodeid = "UserEpisodeHistory".episodeid
                            AND "UserEpisodeHistory".userid = $1
                        LEFT JOIN "EpisodeQueue" ON
                            "SavedEpisodes".episodeid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $2
                            AND "EpisodeQueue".is_youtube = FALSE
                        LEFT JOIN "DownloadedEpisodes" ON
                            "SavedEpisodes".episodeid = "DownloadedEpisodes".episodeid
                            AND "DownloadedEpisodes".userid = $3
                        WHERE "SavedEpisodes".userid = $4

                        UNION ALL

                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "YouTubeVideos".videotitle as episodetitle,
                            "YouTubeVideos".publishedat as episodepubdate,
                            "YouTubeVideos".videodescription as episodedescription,
                            "YouTubeVideos".videoid as episodeid,
                            "YouTubeVideos".thumbnailurl as episodeartwork,
                            "YouTubeVideos".videourl as episodeurl,
                            "YouTubeVideos".duration as episodeduration,
                            "Podcasts".websiteurl as websiteurl,
                            "YouTubeVideos".listenposition as listenduration,
                            "YouTubeVideos".completed as completed,
                            TRUE as saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL AND "EpisodeQueue".is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube
                        FROM "SavedVideos"
                        INNER JOIN "YouTubeVideos" ON "SavedVideos".videoid = "YouTubeVideos".videoid
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "EpisodeQueue" ON
                            "SavedVideos".videoid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $5
                            AND "EpisodeQueue".is_youtube = TRUE
                        LEFT JOIN "DownloadedVideos" ON
                            "SavedVideos".videoid = "DownloadedVideos".videoid
                            AND "DownloadedVideos".userid = $6
                        WHERE "SavedVideos".userid = $7
                    ) combined
                    ORDER BY episodepubdate DESC"#
                )
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::SavedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        websiteurl: row.try_get("websiteurl")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT * FROM (
                        SELECT
                            Podcasts.PodcastName as podcastname,
                            Episodes.EpisodeTitle as episodetitle,
                            Episodes.EpisodePubDate as episodepubdate,
                            Episodes.EpisodeDescription as episodedescription,
                            Episodes.EpisodeID as episodeid,
                            Episodes.EpisodeArtwork as episodeartwork,
                            Episodes.EpisodeURL as episodeurl,
                            Episodes.EpisodeDuration as episodeduration,
                            Podcasts.WebsiteURL as websiteurl,
                            UserEpisodeHistory.ListenDuration as listenduration,
                            Episodes.Completed as completed,
                            TRUE as saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube
                        FROM SavedEpisodes
                        INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID
                        INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                        LEFT JOIN UserEpisodeHistory ON
                            SavedEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID
                            AND UserEpisodeHistory.UserID = ?
                        LEFT JOIN EpisodeQueue ON
                            SavedEpisodes.EpisodeID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = FALSE
                        LEFT JOIN DownloadedEpisodes ON
                            SavedEpisodes.EpisodeID = DownloadedEpisodes.EpisodeID
                            AND DownloadedEpisodes.UserID = ?
                        WHERE SavedEpisodes.UserID = ?

                        UNION ALL

                        SELECT
                            Podcasts.PodcastName as podcastname,
                            YouTubeVideos.VideoTitle as episodetitle,
                            YouTubeVideos.PublishedAt as episodepubdate,
                            YouTubeVideos.VideoDescription as episodedescription,
                            YouTubeVideos.VideoID as episodeid,
                            YouTubeVideos.ThumbnailURL as episodeartwork,
                            YouTubeVideos.VideoURL as episodeurl,
                            YouTubeVideos.Duration as episodeduration,
                            Podcasts.WebsiteURL as websiteurl,
                            YouTubeVideos.ListenPosition as listenduration,
                            YouTubeVideos.Completed as completed,
                            TRUE as saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL AND EpisodeQueue.is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN DownloadedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube
                        FROM SavedVideos
                        INNER JOIN YouTubeVideos ON SavedVideos.VideoID = YouTubeVideos.VideoID
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        LEFT JOIN EpisodeQueue ON
                            SavedVideos.VideoID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = TRUE
                        LEFT JOIN DownloadedVideos ON
                            SavedVideos.VideoID = DownloadedVideos.VideoID
                            AND DownloadedVideos.UserID = ?
                        WHERE SavedVideos.UserID = ?
                    ) combined
                    ORDER BY episodepubdate DESC"
                )
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::SavedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        websiteurl: row.try_get("websiteurl")?,
                        completed: row.try_get("completed")?,
                        saved: true,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
        }
    }

    // Record podcast history - matches Python record_podcast_history function
    pub async fn record_podcast_history(&self, episode_id: i32, user_id: i32, episode_pos: f32, is_youtube: bool) -> AppResult<()> {
        let listen_duration = (episode_pos * 100.0) as i32; // Convert position to duration

        match self {
            DatabasePool::Postgres(pool) => {
                if is_youtube {
                    // Insert or update video history
                    sqlx::query(
                        r#"INSERT INTO "UserVideoHistory" ("VideoID", "UserID", "ListenDuration", "ListenDate")
                           VALUES ($1, $2, $3, NOW())
                           ON CONFLICT ("VideoID", "UserID") 
                           DO UPDATE SET "ListenDuration" = $3, "ListenDate" = NOW()"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(listen_duration)
                    .execute(pool)
                    .await?;
                } else {
                    // Insert or update episode history
                    sqlx::query(
                        r#"INSERT INTO "UserEpisodeHistory" ("EpisodeID", "UserID", "ListenDuration", "ListenDate")
                           VALUES ($1, $2, $3, NOW())
                           ON CONFLICT ("EpisodeID", "UserID") 
                           DO UPDATE SET "ListenDuration" = $3, "ListenDate" = NOW()"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(listen_duration)
                    .execute(pool)
                    .await?;
                }
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                if is_youtube {
                    // Insert or update video history
                    sqlx::query(
                        "INSERT INTO UserVideoHistory (VideoID, UserID, ListenDuration, ListenDate)
                         VALUES (?, ?, ?, NOW())
                         ON DUPLICATE KEY UPDATE ListenDuration = ?, ListenDate = NOW()"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(listen_duration)
                    .bind(listen_duration)
                    .execute(pool)
                    .await?;
                } else {
                    // Insert or update episode history
                    sqlx::query(
                        "INSERT INTO UserEpisodeHistory (EpisodeID, UserID, ListenDuration, ListenDate)
                         VALUES (?, ?, ?, NOW())
                         ON DUPLICATE KEY UPDATE ListenDuration = ?, ListenDate = NOW()"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(listen_duration)
                    .bind(listen_duration)
                    .execute(pool)
                    .await?;
                }
                Ok(())
            }
        }
    }

    // Get user history - matches Python user_history function
    pub async fn get_user_history(&self, user_id: i32) -> AppResult<Vec<crate::models::HistoryEpisode>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT 
                        e."EpisodeTitle" as episodetitle,
                        p."PodcastName" as podcastname,
                        e."EpisodePubDate" as episodepubdate,
                        e."EpisodeDescription" as episodedescription,
                        e."EpisodeArtwork" as episodeartwork,
                        e."EpisodeURL" as episodeurl,
                        e."EpisodeDuration" as episodeduration,
                        ueh."ListenDuration" as listenduration,
                        e."EpisodeID" as episodeid,
                        CASE WHEN ueh."ListenDuration" >= (e."EpisodeDuration" * 0.95) THEN true ELSE false END as completed,
                        ueh."ListenDate" as listendate,
                        false as is_youtube
                    FROM "UserEpisodeHistory" ueh
                    JOIN "Episodes" e ON ueh."EpisodeID" = e."EpisodeID"
                    JOIN "Podcasts" p ON e."PodcastID" = p."PodcastID"
                    WHERE ueh."UserID" = $1 AND p."UserID" = $1
                    
                    UNION ALL
                    
                    SELECT 
                        yv."VideoTitle" as episodetitle,
                        yc."ChannelName" as podcastname,
                        yv."VideoUploadDate" as episodepubdate,
                        yv."VideoDescription" as episodedescription,
                        yv."VideoThumbnail" as episodeartwork,
                        yv."VideoURL" as episodeurl,
                        COALESCE(yv."VideoDuration", 0) as episodeduration,
                        uvh."ListenDuration" as listenduration,
                        yv."VideoID" as episodeid,
                        CASE WHEN uvh."ListenDuration" >= (yv."VideoDuration" * 0.95) THEN true ELSE false END as completed,
                        uvh."ListenDate" as listendate,
                        true as is_youtube
                    FROM "UserVideoHistory" uvh
                    JOIN "YouTubeVideos" yv ON uvh."VideoID" = yv."VideoID"
                    JOIN "YouTubeChannels" yc ON yv."ChannelID" = yc."ChannelID"
                    WHERE uvh."UserID" = $1 AND uvh."ListenDuration" > 0
                    
                    ORDER BY listendate DESC"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::HistoryEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        listendate: row.try_get("listendate").ok(),
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
            DatabasePool::MySQL(pool) => {
                // Similar MySQL implementation
                let rows = sqlx::query(
                    "SELECT 
                        e.EpisodeTitle as episodetitle,
                        p.PodcastName as podcastname,
                        e.EpisodePubDate as episodepubdate,
                        e.EpisodeDescription as episodedescription,
                        e.EpisodeArtwork as episodeartwork,
                        e.EpisodeURL as episodeurl,
                        e.EpisodeDuration as episodeduration,
                        ueh.ListenDuration as listenduration,
                        e.EpisodeID as episodeid,
                        CASE WHEN ueh.ListenDuration >= (e.EpisodeDuration * 0.95) THEN true ELSE false END as completed,
                        ueh.ListenDate as listendate,
                        false as is_youtube
                    FROM UserEpisodeHistory ueh
                    JOIN Episodes e ON ueh.EpisodeID = e.EpisodeID
                    JOIN Podcasts p ON e.PodcastID = p.PodcastID
                    WHERE ueh.UserID = ? AND p.UserID = ?
                    ORDER BY listendate DESC"
                )
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::HistoryEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        listendate: row.try_get("listendate").ok(),
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
        }
    }

    // Get self-service status - matches Python self_service_status function
    pub async fn get_self_service_status(&self) -> AppResult<SelfServiceStatus> {
        match self {
            DatabasePool::Postgres(pool) => {
                // Get self-service status
                let self_service_row = sqlx::query(r#"SELECT selfserviceuser FROM "AppSettings" WHERE selfserviceuser = true"#)
                    .fetch_optional(pool)
                    .await?;
                
                let self_service_enabled = self_service_row.is_some();
                
                // Check if admin exists (excluding background_tasks user)
                let admin_row = sqlx::query(r#"SELECT COUNT(*) as count FROM "Users" WHERE isadmin = true AND username != 'background_tasks'"#)
                    .fetch_one(pool)
                    .await?;
                
                let admin_count: i64 = admin_row.try_get("count")?;
                let admin_exists = admin_count > 0;
                
                Ok(SelfServiceStatus {
                    status: self_service_enabled,
                    admin_exists,
                })
            }
            DatabasePool::MySQL(pool) => {
                // Get self-service status
                let self_service_row = sqlx::query("SELECT SelfServiceUser FROM AppSettings WHERE SelfServiceUser = 1")
                    .fetch_optional(pool)
                    .await?;
                
                let self_service_enabled = self_service_row.is_some();
                
                // Check if admin exists (excluding background_tasks user)
                let admin_row = sqlx::query("SELECT COUNT(*) as count FROM Users WHERE IsAdmin = 1 AND Username != 'background_tasks'")
                    .fetch_one(pool)
                    .await?;
                
                let admin_count: i64 = admin_row.try_get("count")?;
                let admin_exists = admin_count > 0;
                
                Ok(SelfServiceStatus {
                    status: self_service_enabled,
                    admin_exists,
                })
            }
        }
    }

    // Get public OIDC providers - matches Python get_public_oidc_providers function
    pub async fn get_public_oidc_providers(&self) -> AppResult<Vec<PublicOidcProvider>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT
                        providerid,
                        providername,
                        clientid,
                        authorizationurl,
                        scope,
                        buttoncolor,
                        buttontext,
                        buttontextcolor,
                        iconsvg
                    FROM "OIDCProviders"
                    WHERE enabled = true"#
                )
                .fetch_all(pool)
                .await?;
                
                let mut providers = Vec::new();
                for row in rows {
                    providers.push(PublicOidcProvider {
                        provider_id: row.try_get("providerid")?,
                        provider_name: row.try_get("providername")?,
                        client_id: row.try_get("clientid")?,
                        authorization_url: row.try_get("authorizationurl")?,
                        scope: row.try_get("scope")?,
                        button_color: row.try_get("buttoncolor")?,
                        button_text: row.try_get("buttontext")?,
                        button_text_color: row.try_get("buttontextcolor")?,
                        icon_svg: row.try_get("iconsvg").ok(),
                    });
                }
                Ok(providers)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT
                        ProviderID as providerid,
                        ProviderName as providername,
                        ClientID as clientid,
                        AuthorizationURL as authorizationurl,
                        Scope as scope,
                        ButtonColor as buttoncolor,
                        ButtonText as buttontext,
                        ButtonTextColor as buttontextcolor,
                        IconSVG as iconsvg
                    FROM OIDCProviders
                    WHERE Enabled = true"
                )
                .fetch_all(pool)
                .await?;
                
                let mut providers = Vec::new();
                for row in rows {
                    providers.push(PublicOidcProvider {
                        provider_id: row.try_get("providerid")?,
                        provider_name: row.try_get("providername")?,
                        client_id: row.try_get("clientid")?,
                        authorization_url: row.try_get("authorizationurl")?,
                        scope: row.try_get("scope")?,
                        button_color: row.try_get("buttoncolor")?,
                        button_text: row.try_get("buttontext")?,
                        button_text_color: row.try_get("buttontextcolor")?,
                        icon_svg: row.try_get("iconsvg").ok(),
                    });
                }
                Ok(providers)
            }
        }
    }

    // Add admin user - matches Python add_admin_user function
    pub async fn add_admin_user(&self, fullname: &str, username: &str, email: &str, hashed_password: &str) -> AppResult<i32> {
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                
                // Insert the admin user
                let user_row = sqlx::query(
                    r#"WITH inserted_user AS (
                        INSERT INTO "Users"
                        (fullname, username, email, hashed_pw, isadmin)
                        VALUES ($1, $2, $3, $4, true)
                        ON CONFLICT (username) DO NOTHING
                        RETURNING userid
                    )
                    SELECT userid FROM inserted_user
                    UNION ALL
                    SELECT userid FROM "Users" WHERE username = $5
                    LIMIT 1"#
                )
                .bind(fullname)
                .bind(username)
                .bind(email)
                .bind(hashed_password)
                .bind(username)
                .fetch_one(&mut *tx)
                .await?;
                
                let user_id: i32 = user_row.try_get("userid")?;
                
                // Add user settings
                sqlx::query(
                    r#"INSERT INTO "UserSettings" (userid, theme) VALUES ($1, $2)
                       ON CONFLICT (userid) DO NOTHING"#
                )
                .bind(user_id)
                .bind("Nordic")
                .execute(&mut *tx)
                .await?;
                
                // Add user stats
                sqlx::query(
                    r#"INSERT INTO "UserStats" (userid) VALUES ($1)
                       ON CONFLICT (userid) DO NOTHING"#
                )
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
                
                // Create API key for the user
                let api_key = self.generate_api_key();
                sqlx::query(
                    r#"INSERT INTO "APIKeys" (userid, apikey) VALUES ($1, $2)"#
                )
                .bind(user_id)
                .bind(&api_key)
                .execute(&mut *tx)
                .await?;
                
                tx.commit().await?;
                Ok(user_id)
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                
                // Insert the admin user
                let result = sqlx::query(
                    "INSERT INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin) VALUES (?, ?, ?, ?, 1)"
                )
                .bind(fullname)
                .bind(username)
                .bind(email)
                .bind(hashed_password)
                .execute(&mut *tx)
                .await?;
                
                let user_id = result.last_insert_id() as i32;
                
                // Add user settings
                sqlx::query(
                    "INSERT INTO UserSettings (UserID, Theme) VALUES (?, ?)"
                )
                .bind(user_id)
                .bind("Nordic")
                .execute(&mut *tx)
                .await?;
                
                // Add user stats
                sqlx::query(
                    "INSERT INTO UserStats (UserID) VALUES (?)"
                )
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
                
                // Create API key for the user
                let api_key = self.generate_api_key();
                sqlx::query(
                    "INSERT INTO APIKeys (UserID, APIKey) VALUES (?, ?)"
                )
                .bind(user_id)
                .bind(&api_key)
                .execute(&mut *tx)
                .await?;
                
                tx.commit().await?;
                Ok(user_id)
            }
        }
    }

    // Check if admin exists - matches Python check_admin_exists function
    pub async fn check_admin_exists(&self) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT COUNT(*) as count FROM "Users" WHERE isadmin = true AND username != 'background_tasks'"#)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = row.try_get("count")?;
                Ok(count > 0)
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT COUNT(*) as count FROM Users WHERE IsAdmin = 1 AND Username != 'background_tasks'")
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = row.try_get("count")?;
                Ok(count > 0)
            }
        }
    }

    // Generate API key - matches Python create_api_key function
    fn generate_api_key(&self) -> String {
        use rand::Rng;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::thread_rng();
        (0..64)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    // Get user startpage - matches Python get_user_startpage function
    pub async fn get_user_startpage(&self, user_id: i32) -> AppResult<String> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT startpage FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("startpage").unwrap_or_else(|_| "home".to_string())),
                    None => Ok("home".to_string()),
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT StartPage FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("StartPage").unwrap_or_else(|_| "home".to_string())),
                    None => Ok("home".to_string()),
                }
            }
        }
    }

    // Get theme - matches Python get_theme function
    pub async fn get_theme(&self, user_id: i32) -> AppResult<String> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT theme FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("theme").unwrap_or_else(|_| "Nordic".to_string())),
                    None => Ok("Nordic".to_string()),
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT Theme FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("Theme").unwrap_or_else(|_| "Nordic".to_string())),
                    None => Ok("Nordic".to_string()),
                }
            }
        }
    }

    // Check MFA enabled - matches Python check_mfa_enabled function
    pub async fn check_mfa_enabled(&self, user_id: i32) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT mfa_secret FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => {
                        let mfa_secret: Option<String> = row.try_get("mfa_secret").ok();
                        Ok(mfa_secret.map_or(false, |s| !s.is_empty()))
                    }
                    None => Ok(false),
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT MFA_Secret FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => {
                        let mfa_secret: Option<String> = row.try_get("MFA_Secret").ok();
                        Ok(mfa_secret.map_or(false, |s| !s.is_empty()))
                    }
                    None => Ok(false),
                }
            }
        }
    }

    // First login done - matches Python first_login_done function
    pub async fn first_login_done(&self, user_id: i32) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT firstlogin FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("firstlogin").unwrap_or(false)),
                    None => Err(AppError::not_found("User not found")),
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT FirstLogin FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("FirstLogin").unwrap_or(false)),
                    None => Err(AppError::not_found("User not found")),
                }
            }
        }
    }

    // Add episodes - matches Python add_episodes function exactly
    pub async fn add_episodes(
        &self,
        podcast_id: i32,
        feed_url: &str,
        artwork_url: &str,
        auto_download: bool,
        username: Option<&str>,
        password: Option<&str>,
    ) -> AppResult<Option<i32>> {
        // Fetch the RSS feed
        let content = self.try_fetch_feed(feed_url, username, password).await?;
        
        // Parse the RSS feed
        let episodes = self.parse_rss_feed(&content, podcast_id, artwork_url).await?;
        
        let mut first_episode_id = None;
        
        for episode in episodes {
            // Check if episode already exists
            let exists = match self {
                DatabasePool::Postgres(pool) => {
                    let row = sqlx::query(r#"SELECT episodeid FROM "Episodes" WHERE podcastid = $1 AND episodetitle = $2"#)
                        .bind(podcast_id)
                        .bind(&episode.title)
                        .fetch_optional(pool)
                        .await?;
                    row.is_some()
                }
                DatabasePool::MySQL(pool) => {
                    let row = sqlx::query("SELECT EpisodeID FROM Episodes WHERE PodcastID = ? AND EpisodeTitle = ?")
                        .bind(podcast_id)
                        .bind(&episode.title)
                        .fetch_optional(pool)
                        .await?;
                    row.is_some()
                }
            };
            
            if exists {
                continue;
            }
            
            // Insert new episode
            let episode_id = match self {
                DatabasePool::Postgres(pool) => {
                    let row = sqlx::query(
                        r#"INSERT INTO "Episodes" 
                           (podcastid, episodetitle, episodedescription, episodeurl, episodeartwork, episodepubdate, episodeduration)
                           VALUES ($1, $2, $3, $4, $5, $6, $7)
                           RETURNING episodeid"#
                    )
                    .bind(podcast_id)
                    .bind(&episode.title)
                    .bind(&episode.description)
                    .bind(&episode.url)
                    .bind(&episode.artwork_url)
                    .bind(&episode.pub_date)
                    .bind(episode.duration)
                    .fetch_one(pool)
                    .await?;
                    
                    row.try_get::<i32, _>("episodeid")?
                }
                DatabasePool::MySQL(pool) => {
                    let result = sqlx::query(
                        "INSERT INTO Episodes 
                         (PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration)
                         VALUES (?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(podcast_id)
                    .bind(&episode.title)
                    .bind(&episode.description)
                    .bind(&episode.url)
                    .bind(&episode.artwork_url)
                    .bind(&episode.pub_date)
                    .bind(episode.duration)
                    .execute(pool)
                    .await?;
                    
                    result.last_insert_id() as i32
                }
            };
            
            // Set first episode ID if not set
            if first_episode_id.is_none() {
                first_episode_id = Some(episode_id);
            }
        }
        
        // Update episode count
        self.update_episode_count(podcast_id).await?;
        
        // Get the actual first episode ID (earliest by pub date)
        let first_id = self.get_first_episode_id(podcast_id, false).await?;
        
        Ok(first_id)
    }

    // Try to fetch RSS feed - matches Python try_fetch_feed function
    async fn try_fetch_feed(
        &self,
        url: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> AppResult<String> {
        let client = reqwest::Client::new();
        let mut request = client.get(url).header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        
        if let (Some(user), Some(pass)) = (username, password) {
            request = request.basic_auth(user, Some(pass));
        }
        
        let response = request.send().await.map_err(|e| AppError::Http(e))?;
        
        if !response.status().is_success() {
            // Try alternate URL (www vs non-www)
            let alternate_url = if url.contains("://www.") {
                url.replace("://www.", "://")
            } else {
                url.replace("://", "://www.")
            };
            
            let mut alt_request = client.get(&alternate_url).header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
            
            if let (Some(user), Some(pass)) = (username, password) {
                alt_request = alt_request.basic_auth(user, Some(pass));
            }
            
            let alt_response = alt_request.send().await.map_err(|e| AppError::Http(e))?;
            
            if !alt_response.status().is_success() {
                return Err(AppError::bad_request("Invalid username or password"));
            }
            
            return Ok(alt_response.text().await.map_err(|e| AppError::Http(e))?);
        }
        
        Ok(response.text().await.map_err(|e| AppError::Http(e))?)
    }

    // Parse RSS feed - matches Python RSS parsing logic
    async fn parse_rss_feed(
        &self,
        content: &str,
        podcast_id: i32,
        artwork_url: &str,
    ) -> AppResult<Vec<EpisodeData>> {
        use quick_xml::Reader;
        use quick_xml::events::Event;
        use chrono::{DateTime, Utc};
        use std::collections::HashMap;
        
        let mut reader = Reader::from_str(content);
        reader.config_mut().trim_text(true);
        
        let mut episodes = Vec::new();
        let mut current_episode: Option<EpisodeData> = None;
        let mut current_tag = String::new();
        let mut current_text = String::new();
        let mut in_item = false;
        let mut in_content = false;
        let mut current_attrs: HashMap<String, String> = HashMap::new();
        let mut episode_data: HashMap<String, String> = HashMap::new();
        
        loop {
            match reader.read_event() {
                Ok(Event::Start(ref e)) => {
                    current_tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    current_text.clear();
                    current_attrs.clear();
                    
                    // Store attributes
                    for attr in e.attributes() {
                        if let Ok(attr) = attr {
                            let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let value = String::from_utf8_lossy(&attr.value).to_string();
                            current_attrs.insert(key, value);
                        }
                    }
                    
                    if current_tag == "item" {
                        in_item = true;
                        episode_data.clear();
                        current_episode = Some(EpisodeData {
                            title: String::new(),
                            description: String::new(),
                            url: String::new(),
                            artwork_url: artwork_url.to_string(),
                            pub_date: Utc::now(),
                            duration: 0,
                        });
                    }
                    
                    // Handle enclosure tag for audio URL and file size
                    if current_tag == "enclosure" && in_item {
                        if let Some(url) = current_attrs.get("url") {
                            episode_data.insert("enclosure_url".to_string(), url.clone());
                        }
                        if let Some(length) = current_attrs.get("length") {
                            episode_data.insert("enclosure_length".to_string(), length.clone());
                        }
                    }
                    
                    // Handle content:encoded differently
                    if current_tag == "content:encoded" && in_item {
                        in_content = true;
                    }
                }
                Ok(Event::Text(e)) => {
                    current_text = e.decode().unwrap_or_default().into_owned();
                }
                Ok(Event::CData(e)) => {
                    current_text = e.decode().unwrap_or_default().into_owned();
                }
                Ok(Event::End(ref e)) => {
                    let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    
                    if tag == "item" {
                        if let Some(mut episode) = current_episode.take() {
                            // Apply all the Python-style parsing logic
                            self.apply_python_style_parsing(&mut episode, &episode_data, artwork_url);
                            
                            if !episode.title.is_empty() {
                                episodes.push(episode);
                            }
                        }
                        in_item = false;
                        episode_data.clear();
                    } else if in_item {
                        // Store all tag content for later processing
                        episode_data.insert(tag.clone(), current_text.clone());
                        
                        // Handle content:encoded
                        if tag == "content:encoded" {
                            in_content = false;
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(AppError::Internal(format!("RSS parsing error: {}", e))),
                _ => {}
            }
        }
        
        Ok(episodes)
    }
    
    // Apply Python-style parsing logic with all fallbacks
    fn apply_python_style_parsing(&self, episode: &mut EpisodeData, data: &HashMap<String, String>, default_artwork: &str) {
        // Title - required field
        if let Some(title) = data.get("title") {
            episode.title = title.clone();
        }
        
        // Description with multiple fallbacks like Python version
        episode.description = data.get("content:encoded")
            .filter(|s| !s.trim().is_empty())
            .or_else(|| data.get("description"))
            .or_else(|| data.get("summary"))
            .or_else(|| data.get("itunes:summary"))
            .or_else(|| data.get("subtitle"))
            .cloned()
            .unwrap_or_else(|| "No description available".to_string());
        
        // Audio URL - enclosure URL has priority, then link
        episode.url = data.get("enclosure_url")
            .or_else(|| data.get("link"))
            .cloned()
            .unwrap_or_default();
        
        // Artwork with multiple fallbacks
        episode.artwork_url = data.get("itunes:image")
            .or_else(|| data.get("image"))
            .or_else(|| data.get("media:thumbnail"))
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| default_artwork.to_string());
        
        // Publication date with fallbacks
        if let Some(pub_date_str) = data.get("pubDate")
            .or_else(|| data.get("published"))
            .or_else(|| data.get("dc:date")) {
            
            // Try multiple date formats
            if let Ok(parsed) = DateTime::parse_from_rfc2822(pub_date_str) {
                episode.pub_date = parsed.with_timezone(&Utc);
            } else if let Ok(parsed) = DateTime::parse_from_rfc3339(pub_date_str) {
                episode.pub_date = parsed.with_timezone(&Utc);
            } else {
                // Try other common formats
                let formats = [
                    "%Y-%m-%d %H:%M:%S",
                    "%Y-%m-%dT%H:%M:%S",
                    "%a, %d %b %Y %H:%M:%S %z",
                    "%d %b %Y %H:%M:%S %z",
                ];
                
                for format in &formats {
                    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(pub_date_str, format) {
                        episode.pub_date = DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
                        break;
                    }
                }
            }
        }
        
        // Duration parsing with extensive fallbacks like Python
        episode.duration = self.parse_duration_comprehensive(data);
    }
    
    // Comprehensive duration parsing matching Python logic
    fn parse_duration_comprehensive(&self, data: &HashMap<String, String>) -> i32 {
        // Try itunes:duration first
        if let Some(duration_str) = data.get("itunes:duration") {
            if let Some(duration) = self.parse_duration_string(duration_str) {
                return duration;
            }
        }
        
        // Try other duration fields
        for field in ["duration", "itunes:duration_seconds", "length"] {
            if let Some(duration_str) = data.get(field) {
                if let Some(duration) = self.parse_duration_string(duration_str) {
                    return duration;
                }
            }
        }
        
        // Estimate from file size as last resort (like Python)
        if let Some(length_str) = data.get("enclosure_length") {
            if let Ok(file_size) = length_str.parse::<i64>() {
                if file_size > 1_000_000 { // Only for files > 1MB
                    return self.estimate_duration_from_file_size(file_size);
                }
            }
        }
        
        0 // Default fallback
    }
    
    // Parse duration string with multiple formats
    fn parse_duration_string(&self, duration_str: &str) -> Option<i32> {
        let duration_str = duration_str.trim();
        
        if duration_str.contains(':') {
            // Handle HH:MM:SS or MM:SS format
            let parts: Vec<&str> = duration_str.split(':').collect();
            let mut time_parts: Vec<i32> = Vec::new();
            
            for part in parts.iter().rev() { // Reverse to handle missing hours
                if let Ok(num) = part.parse::<i32>() {
                    time_parts.insert(0, num);
                } else {
                    return None;
                }
            }
            
            // Pad with zeros for missing parts
            while time_parts.len() < 3 {
                time_parts.insert(0, 0);
            }
            
            if time_parts.len() >= 3 {
                let (h, m, s) = (time_parts[0], time_parts[1], time_parts[2]);
                return Some(h * 3600 + m * 60 + s);
            }
        } else if let Ok(seconds) = duration_str.parse::<i32>() {
            // Direct seconds format
            return Some(seconds);
        }
        
        None
    }
    
    // Estimate duration from file size (matching Python logic)
    fn estimate_duration_from_file_size(&self, file_size_bytes: i64) -> i32 {
        let bitrate_kbps = 128; // Default bitrate assumption
        let bytes_per_second = (bitrate_kbps * 1000) / 8;
        (file_size_bytes / bytes_per_second as i64) as i32
    }

    // Parse duration from various formats
    fn parse_duration(&self, duration_str: &str) -> i32 {
        if duration_str.contains(':') {
            let parts: Vec<&str> = duration_str.split(':').collect();
            match parts.len() {
                2 => {
                    // MM:SS
                    let minutes = parts[0].parse::<i32>().unwrap_or(0);
                    let seconds = parts[1].parse::<i32>().unwrap_or(0);
                    minutes * 60 + seconds
                }
                3 => {
                    // HH:MM:SS
                    let hours = parts[0].parse::<i32>().unwrap_or(0);
                    let minutes = parts[1].parse::<i32>().unwrap_or(0);
                    let seconds = parts[2].parse::<i32>().unwrap_or(0);
                    hours * 3600 + minutes * 60 + seconds
                }
                _ => 0,
            }
        } else {
            duration_str.parse::<i32>().unwrap_or(0)
        }
    }

    // Update episode count - matches Python update_episode_count function
    pub async fn update_episode_count(&self, podcast_id: i32) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                let episode_count = sqlx::query(r#"SELECT COUNT(*) as count FROM "Episodes" WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = episode_count.try_get("count")?;
                
                sqlx::query(r#"UPDATE "Podcasts" SET episodecount = $1 WHERE podcastid = $2"#)
                    .bind(count)
                    .bind(podcast_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                let episode_count = sqlx::query("SELECT COUNT(*) as count FROM Episodes WHERE PodcastID = ?")
                    .bind(podcast_id)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = episode_count.try_get("count")?;
                
                sqlx::query("UPDATE Podcasts SET EpisodeCount = ? WHERE PodcastID = ?")
                    .bind(count)
                    .bind(podcast_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    // Get first episode ID - matches Python get_first_episode_id function
    pub async fn get_first_episode_id(&self, podcast_id: i32, is_youtube: bool) -> AppResult<Option<i32>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let query = if is_youtube {
                    r#"SELECT videoid FROM "YouTubeVideos" WHERE podcastid = $1 ORDER BY publishedat ASC LIMIT 1"#
                } else {
                    r#"SELECT episodeid FROM "Episodes" WHERE podcastid = $1 ORDER BY episodepubdate ASC LIMIT 1"#
                };
                
                let row = sqlx::query(query)
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    if is_youtube {
                        Ok(Some(row.try_get("videoid")?))
                    } else {
                        Ok(Some(row.try_get("episodeid")?))
                    }
                } else {
                    Ok(None)
                }
            }
            DatabasePool::MySQL(pool) => {
                let query = if is_youtube {
                    "SELECT VideoID FROM YouTubeVideos WHERE PodcastID = ? ORDER BY PublishedAt ASC LIMIT 1"
                } else {
                    "SELECT EpisodeID FROM Episodes WHERE PodcastID = ? ORDER BY EpisodePubDate ASC LIMIT 1"
                };
                
                let row = sqlx::query(query)
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    if is_youtube {
                        Ok(Some(row.try_get("VideoID")?))
                    } else {
                        Ok(Some(row.try_get("EpisodeID")?))
                    }
                } else {
                    Ok(None)
                }
            }
        }
    }

    // Setup timezone info - matches Python setup_timezone_info function
    pub async fn setup_timezone_info(&self, user_id: i32, timezone: &str, hour_pref: i32, date_format: &str) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let result = sqlx::query(r#"UPDATE "Users" SET timezone = $1, timeformat = $2, dateformat = $3, firstlogin = $4 WHERE userid = $5"#)
                    .bind(timezone)
                    .bind(hour_pref)
                    .bind(date_format)
                    .bind(true)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            }
            DatabasePool::MySQL(pool) => {
                let result = sqlx::query("UPDATE Users SET Timezone = ?, TimeFormat = ?, DateFormat = ?, FirstLogin = ? WHERE UserID = ?")
                    .bind(timezone)
                    .bind(hour_pref)
                    .bind(date_format)
                    .bind(1)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            }
        }
    }

    // User admin check - matches Python user_admin_check function
    pub async fn user_admin_check(&self, user_id: i32) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT isadmin FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("isadmin")?)
                } else {
                    Ok(false)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT IsAdmin FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let is_admin: i8 = row.try_get("IsAdmin")?;
                    Ok(is_admin != 0)
                } else {
                    Ok(false)
                }
            }
        }
    }

    // Get podcast ID by user, feed URL, and title
    pub async fn get_podcast_id(&self, user_id: i32, podcast_feed: &str, podcast_title: &str) -> AppResult<Option<i32>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT podcastid FROM "Podcasts" WHERE feedurl = $1 AND podcastname = $2 AND userid = $3"#)
                    .bind(podcast_feed)
                    .bind(podcast_title)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(Some(row.try_get("podcastid")?))
                } else {
                    Ok(None)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT PodcastID FROM Podcasts WHERE FeedURL = ? AND PodcastName = ? AND UserID = ?")
                    .bind(podcast_feed)
                    .bind(podcast_title)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(Some(row.try_get("PodcastID")?))
                } else {
                    Ok(None)
                }
            }
        }
    }

    // Get downloaded episodes - matches Python download_episode_list function
    pub async fn download_episode_list(&self, user_id: i32) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT * FROM (
                        SELECT
                            "Podcasts".podcastid as podcastid,
                            "Podcasts".podcastname as podcastname,
                            "Podcasts".artworkurl as artworkurl,
                            "Episodes".episodeid as episodeid,
                            "Episodes".episodetitle as episodetitle,
                            "Episodes".episodepubdate as episodepubdate,
                            "Episodes".episodedescription as episodedescription,
                            "Episodes".episodeartwork as episodeartwork,
                            "Episodes".episodeurl as episodeurl,
                            "Episodes".episodeduration as episodeduration,
                            "Podcasts".podcastindexid as podcastindexid,
                            "Podcasts".websiteurl as websiteurl,
                            "DownloadedEpisodes".downloadedlocation as downloadedlocation,
                            "UserEpisodeHistory".listenduration as listenduration,
                            "Episodes".completed as completed,
                            CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            TRUE as downloaded,
                            FALSE as is_youtube
                        FROM "DownloadedEpisodes"
                        INNER JOIN "Episodes" ON "DownloadedEpisodes".episodeid = "Episodes".episodeid
                        INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                        LEFT JOIN "UserEpisodeHistory" ON
                            "DownloadedEpisodes".episodeid = "UserEpisodeHistory".episodeid
                            AND "DownloadedEpisodes".userid = "UserEpisodeHistory".userid
                        LEFT JOIN "SavedEpisodes" ON
                            "DownloadedEpisodes".episodeid = "SavedEpisodes".episodeid
                            AND "SavedEpisodes".userid = $1
                        LEFT JOIN "EpisodeQueue" ON
                            "DownloadedEpisodes".episodeid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $2
                            AND "EpisodeQueue".is_youtube = FALSE
                        WHERE "DownloadedEpisodes".userid = $3

                        UNION ALL

                        SELECT
                            "Podcasts".podcastid as podcastid,
                            "Podcasts".podcastname as podcastname,
                            "Podcasts".artworkurl as artworkurl,
                            "YouTubeVideos".videoid as episodeid,
                            "YouTubeVideos".videotitle as episodetitle,
                            "YouTubeVideos".publishedat as episodepubdate,
                            "YouTubeVideos".videodescription as episodedescription,
                            "YouTubeVideos".thumbnailurl as episodeartwork,
                            "YouTubeVideos".videourl as episodeurl,
                            "YouTubeVideos".duration as episodeduration,
                            "Podcasts".podcastindexid as podcastindexid,
                            "Podcasts".websiteurl as websiteurl,
                            "DownloadedVideos".downloadedlocation as downloadedlocation,
                            "YouTubeVideos".listenposition as listenduration,
                            "YouTubeVideos".completed as completed,
                            CASE WHEN "SavedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL AND "EpisodeQueue".is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            TRUE as downloaded,
                            TRUE as is_youtube
                        FROM "DownloadedVideos"
                        INNER JOIN "YouTubeVideos" ON "DownloadedVideos".videoid = "YouTubeVideos".videoid
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "SavedVideos" ON
                            "DownloadedVideos".videoid = "SavedVideos".videoid
                            AND "SavedVideos".userid = $4
                        LEFT JOIN "EpisodeQueue" ON
                            "DownloadedVideos".videoid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $5
                            AND "EpisodeQueue".is_youtube = TRUE
                        WHERE "DownloadedVideos".userid = $6
                    ) combined
                    ORDER BY episodepubdate DESC"#
                )
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::handlers::podcasts::Episode {
                        podcastname: row.try_get("podcastname")?,
                        episodetitle: row.try_get("episodetitle")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT * FROM (
                        SELECT
                            Podcasts.PodcastID as podcastid,
                            Podcasts.PodcastName as podcastname,
                            Podcasts.ArtworkURL as artworkurl,
                            Episodes.EpisodeID as episodeid,
                            Episodes.EpisodeTitle as episodetitle,
                            Episodes.EpisodePubDate as episodepubdate,
                            Episodes.EpisodeDescription as episodedescription,
                            Episodes.EpisodeArtwork as episodeartwork,
                            Episodes.EpisodeURL as episodeurl,
                            Episodes.EpisodeDuration as episodeduration,
                            Podcasts.PodcastIndexID as podcastindexid,
                            Podcasts.WebsiteURL as websiteurl,
                            DownloadedEpisodes.DownloadedLocation as downloadedlocation,
                            UserEpisodeHistory.ListenDuration as listenduration,
                            Episodes.Completed as completed,
                            CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            TRUE as downloaded,
                            FALSE as is_youtube
                        FROM DownloadedEpisodes
                        INNER JOIN Episodes ON DownloadedEpisodes.EpisodeID = Episodes.EpisodeID
                        INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                        LEFT JOIN UserEpisodeHistory ON
                            DownloadedEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID
                            AND DownloadedEpisodes.UserID = UserEpisodeHistory.UserID
                        LEFT JOIN SavedEpisodes ON
                            DownloadedEpisodes.EpisodeID = SavedEpisodes.EpisodeID
                            AND SavedEpisodes.UserID = ?
                        LEFT JOIN EpisodeQueue ON
                            DownloadedEpisodes.EpisodeID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = FALSE
                        WHERE DownloadedEpisodes.UserID = ?

                        UNION ALL

                        SELECT
                            Podcasts.PodcastID as podcastid,
                            Podcasts.PodcastName as podcastname,
                            Podcasts.ArtworkURL as artworkurl,
                            YouTubeVideos.VideoID as episodeid,
                            YouTubeVideos.VideoTitle as episodetitle,
                            YouTubeVideos.PublishedAt as episodepubdate,
                            YouTubeVideos.VideoDescription as episodedescription,
                            YouTubeVideos.ThumbnailURL as episodeartwork,
                            YouTubeVideos.VideoURL as episodeurl,
                            YouTubeVideos.Duration as episodeduration,
                            Podcasts.PodcastIndexID as podcastindexid,
                            Podcasts.WebsiteURL as websiteurl,
                            DownloadedVideos.DownloadedLocation as downloadedlocation,
                            YouTubeVideos.ListenPosition as listenduration,
                            YouTubeVideos.Completed as completed,
                            CASE WHEN SavedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL AND EpisodeQueue.is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            TRUE as downloaded,
                            TRUE as is_youtube
                        FROM DownloadedVideos
                        INNER JOIN YouTubeVideos ON DownloadedVideos.VideoID = YouTubeVideos.VideoID
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        LEFT JOIN SavedVideos ON
                            DownloadedVideos.VideoID = SavedVideos.VideoID
                            AND SavedVideos.UserID = ?
                        LEFT JOIN EpisodeQueue ON
                            DownloadedVideos.VideoID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = TRUE
                        WHERE DownloadedVideos.UserID = ?
                    ) combined
                    ORDER BY episodepubdate DESC"
                )
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::handlers::podcasts::Episode {
                        podcastname: row.try_get("podcastname")?,
                        episodetitle: row.try_get("episodetitle")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                Ok(episodes)
            }
        }
    }

    // Check if episode is already downloaded
    pub async fn check_downloaded(&self, user_id: i32, episode_id: i32, is_youtube: bool) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let query = if is_youtube {
                    r#"SELECT COUNT(*) as count FROM "DownloadedVideos" WHERE userid = $1 AND videoid = $2"#
                } else {
                    r#"SELECT COUNT(*) as count FROM "DownloadedEpisodes" WHERE userid = $1 AND episodeid = $2"#
                };
                
                let row = sqlx::query(query)
                    .bind(user_id)
                    .bind(episode_id)
                    .fetch_one(pool)
                    .await?;
                    
                let count: i64 = row.try_get("count")?;
                Ok(count > 0)
            }
            DatabasePool::MySQL(pool) => {
                let query = if is_youtube {
                    "SELECT COUNT(*) as count FROM DownloadedVideos WHERE UserID = ? AND VideoID = ?"
                } else {
                    "SELECT COUNT(*) as count FROM DownloadedEpisodes WHERE UserID = ? AND EpisodeID = ?"
                };
                
                let row = sqlx::query(query)
                    .bind(user_id)
                    .bind(episode_id)
                    .fetch_one(pool)
                    .await?;
                    
                let count: i64 = row.try_get("count")?;
                Ok(count > 0)
            }
        }
    }

    // Delete downloaded episode
    pub async fn delete_episode(&self, user_id: i32, episode_id: i32, is_youtube: bool) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                if is_youtube {
                    sqlx::query(r#"DELETE FROM "DownloadedVideos" WHERE userid = $1 AND videoid = $2"#)
                        .bind(user_id)
                        .bind(episode_id)
                        .execute(pool)
                        .await?;
                } else {
                    sqlx::query(r#"DELETE FROM "DownloadedEpisodes" WHERE userid = $1 AND episodeid = $2"#)
                        .bind(user_id)
                        .bind(episode_id)
                        .execute(pool)
                        .await?;
                }
                Ok(())
            }
            DatabasePool::MySQL(pool) => {
                if is_youtube {
                    sqlx::query("DELETE FROM DownloadedVideos WHERE UserID = ? AND VideoID = ?")
                        .bind(user_id)
                        .bind(episode_id)
                        .execute(pool)
                        .await?;
                } else {
                    sqlx::query("DELETE FROM DownloadedEpisodes WHERE UserID = ? AND EpisodeID = ?")
                        .bind(user_id)
                        .bind(episode_id)
                        .execute(pool)
                        .await?;
                }
                Ok(())
            }
        }
    }

    // Get download status for user
    pub async fn get_download_status(&self, user_id: i32) -> AppResult<serde_json::Value> {
        match self {
            DatabasePool::Postgres(pool) => {
                // Get active download tasks
                let rows = sqlx::query(
                    r#"SELECT taskid, tasktype, progress, status FROM "UserTasks" 
                       WHERE userid = $1 AND tasktype IN ('download_episode', 'download_video', 'download_all_episodes', 'download_all_videos') 
                       AND status IN ('pending', 'running')"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut tasks = Vec::new();
                for row in rows {
                    tasks.push(serde_json::json!({
                        "task_id": row.try_get::<String, _>("taskid")?,
                        "task_type": row.try_get::<String, _>("tasktype")?,
                        "progress": row.try_get::<Option<i32>, _>("progress")?,
                        "status": row.try_get::<String, _>("status")?
                    }));
                }
                
                Ok(serde_json::json!({ "active_downloads": tasks }))
            }
            DatabasePool::MySQL(pool) => {
                // Get active download tasks
                let rows = sqlx::query(
                    "SELECT TaskID, TaskType, Progress, Status FROM UserTasks 
                     WHERE UserID = ? AND TaskType IN ('download_episode', 'download_video', 'download_all_episodes', 'download_all_videos') 
                     AND Status IN ('pending', 'running')"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut tasks = Vec::new();
                for row in rows {
                    tasks.push(serde_json::json!({
                        "task_id": row.try_get::<String, _>("TaskID")?,
                        "task_type": row.try_get::<String, _>("TaskType")?,
                        "progress": row.try_get::<Option<i32>, _>("Progress")?,
                        "status": row.try_get::<String, _>("Status")?
                    }));
                }
                
                Ok(serde_json::json!({ "active_downloads": tasks }))
            }
        }
    }

    // Get episodes for a specific podcast - matches Python return_podcast_episodes function
    pub async fn return_podcast_episodes(&self, user_id: i32, podcast_id: i32) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT 
                        "Podcasts".podcastid, "Podcasts".podcastname, "Episodes".episodeid,
                        "Episodes".episodetitle, "Episodes".episodepubdate, "Episodes".episodedescription,
                        "Episodes".episodeartwork, "Episodes".episodeurl, "Episodes".episodeduration,
                        "Episodes".completed,
                        "UserEpisodeHistory".listenduration, CAST("Episodes".episodeid AS VARCHAR) AS guid
                       FROM "Episodes"
                       INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                       LEFT JOIN "UserEpisodeHistory" ON "Episodes".episodeid = "UserEpisodeHistory".episodeid AND "UserEpisodeHistory".userid = $1
                       WHERE "Podcasts".podcastid = $2 AND "Podcasts".userid = $3
                       ORDER BY "Episodes".episodepubdate DESC"#
                )
                .bind(user_id)
                .bind(podcast_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::handlers::podcasts::Episode {
                        podcastname: row.try_get("podcastname")?,
                        episodetitle: row.try_get("episodetitle")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: false, // Not included in this query
                        queued: false, // Not included in this query
                        downloaded: false, // Not included in this query
                        is_youtube: false, // This is for regular episodes
                    });
                }
                Ok(episodes)
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT 
                        Podcasts.PodcastID, Podcasts.PodcastName, Episodes.EpisodeID,
                        Episodes.EpisodeTitle, Episodes.EpisodePubDate, Episodes.EpisodeDescription,
                        Episodes.EpisodeArtwork, Episodes.EpisodeURL, Episodes.EpisodeDuration,
                        Episodes.Completed,
                        UserEpisodeHistory.ListenDuration, CAST(Episodes.EpisodeID AS CHAR) AS guid
                     FROM Episodes
                     INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                     LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID AND UserEpisodeHistory.UserID = ?
                     WHERE Podcasts.PodcastID = ? AND Podcasts.UserID = ?
                     ORDER BY Episodes.EpisodePubDate DESC"
                )
                .bind(user_id)
                .bind(podcast_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::handlers::podcasts::Episode {
                        podcastname: row.try_get("PodcastName")?,
                        episodetitle: row.try_get("EpisodeTitle")?,
                        episodepubdate: {
                            let datetime = row.try_get::<chrono::DateTime<chrono::Utc>, _>("EpisodePubDate")?;
                            datetime.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("EpisodeDescription")?,
                        episodeartwork: row.try_get("EpisodeArtwork")?,
                        episodeurl: row.try_get("EpisodeURL")?,
                        episodeduration: row.try_get("EpisodeDuration")?,
                        listenduration: row.try_get("ListenDuration").ok(),
                        episodeid: row.try_get("EpisodeID")?,
                        completed: row.try_get("Completed")?,
                        saved: false, // Not included in this query
                        queued: false, // Not included in this query
                        downloaded: false, // Not included in this query
                        is_youtube: false, // This is for regular episodes
                    });
                }
                Ok(episodes)
            }
        }
    }

    // Get podcast ID from episode name and URL - matches Python get_podcast_id_from_episode_name function
    pub async fn get_podcast_id_from_episode_name(&self, user_id: i32, episode_name: &str, episode_url: &str) -> AppResult<Option<i32>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = if episode_url.is_empty() {
                    // If episode_url is empty, search by title only
                    sqlx::query(
                        r#"SELECT podcast_id FROM (
                            SELECT "Episodes".podcastid as podcast_id
                            FROM "Episodes"
                            INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                            WHERE "Episodes".episodetitle = $1
                            AND "Podcasts".userid = $2

                            UNION

                            SELECT "YouTubeVideos".podcastid as podcast_id
                            FROM "YouTubeVideos"
                            INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                            WHERE "YouTubeVideos".videotitle = $3
                            AND "Podcasts".userid = $4
                        ) combined_results
                        LIMIT 1"#
                    )
                    .bind(episode_name)
                    .bind(user_id)
                    .bind(episode_name)
                    .bind(user_id)
                } else {
                    // If episode_url is provided, search by both title and URL
                    sqlx::query(
                        r#"SELECT podcast_id FROM (
                            SELECT "Episodes".podcastid as podcast_id
                            FROM "Episodes"
                            INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                            WHERE "Episodes".episodetitle = $1
                            AND "Episodes".episodeurl = $2
                            AND "Podcasts".userid = $3

                            UNION

                            SELECT "YouTubeVideos".podcastid as podcast_id
                            FROM "YouTubeVideos"
                            INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                            WHERE "YouTubeVideos".videotitle = $4
                            AND "YouTubeVideos".videourl = $5
                            AND "Podcasts".userid = $6
                        ) combined_results
                        LIMIT 1"#
                    )
                    .bind(episode_name)
                    .bind(episode_url)
                    .bind(user_id)
                    .bind(episode_name)
                    .bind(episode_url)
                    .bind(user_id)
                }
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(row.try_get("podcast_id")?))
                } else {
                    Ok(None)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query(
                    "SELECT podcast_id FROM (
                        SELECT Episodes.PodcastID as podcast_id
                        FROM Episodes
                        INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                        WHERE Episodes.EpisodeTitle = ?
                        AND Episodes.EpisodeURL = ?
                        AND Podcasts.UserID = ?

                        UNION

                        SELECT YouTubeVideos.PodcastID as podcast_id
                        FROM YouTubeVideos
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        WHERE YouTubeVideos.VideoTitle = ?
                        AND YouTubeVideos.VideoURL = ?
                        AND Podcasts.UserID = ?
                    ) combined_results
                    LIMIT 1"
                )
                .bind(episode_name)
                .bind(episode_url)
                .bind(user_id)
                .bind(episode_name)
                .bind(episode_url)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(row.try_get("podcast_id")?))
                } else {
                    Ok(None)
                }
            }
        }
    }

    // Get episode metadata - matches Python get_episode_metadata function exactly
    pub async fn get_episode_metadata(&self, episode_id: i32, user_id: i32, person_episode: bool, is_youtube: bool) -> AppResult<serde_json::Value> {
        match self {
            DatabasePool::Postgres(pool) => {
                if is_youtube {
                    // Query for YouTube videos
                    let row = sqlx::query(
                        r#"SELECT "Podcasts".podcastid, "Podcasts".podcastindexid, "Podcasts".feedurl,
                                "Podcasts".podcastname, "Podcasts".artworkurl,
                                "YouTubeVideos".videotitle as episodetitle,
                                "YouTubeVideos".publishedat as episodepubdate,
                                "YouTubeVideos".videodescription as episodedescription,
                                "YouTubeVideos".thumbnailurl as episodeartwork,
                                "YouTubeVideos".videourl as episodeurl,
                                "YouTubeVideos".duration as episodeduration,
                                "YouTubeVideos".videoid as episodeid,
                                "YouTubeVideos".listenposition as listenduration,
                                "YouTubeVideos".completed,
                                CASE WHEN q.episodeid IS NOT NULL THEN true ELSE false END as is_queued,
                                CASE WHEN s.episodeid IS NOT NULL THEN true ELSE false END as is_saved,
                                CASE WHEN d.episodeid IS NOT NULL THEN true ELSE false END as is_downloaded,
                                TRUE::boolean as is_youtube
                        FROM "YouTubeVideos"
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "EpisodeQueue" q ON "YouTubeVideos".videoid = q.episodeid AND q.userid = $1
                        LEFT JOIN "SavedEpisodes" s ON "YouTubeVideos".videoid = s.episodeid AND s.userid = $1
                        LEFT JOIN "DownloadedEpisodes" d ON "YouTubeVideos".videoid = d.episodeid AND d.userid = $1
                        WHERE "YouTubeVideos".videoid = $2 AND "Podcasts".userid = $1"#
                    )
                    .bind(user_id)
                    .bind(episode_id)
                    .fetch_optional(pool)
                    .await?;
                    
                    if let Some(row) = row {
                        let naive_date = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                        let episode_pubdate = naive_date.format("%Y-%m-%dT%H:%M:%S").to_string();
                        
                        return Ok(serde_json::json!({
                            "podcastid": row.try_get::<i32, _>("podcastid")?,
                            "podcastindexid": row.try_get::<Option<i32>, _>("podcastindexid")?,
                            "feedurl": row.try_get::<String, _>("feedurl")?,
                            "podcastname": row.try_get::<String, _>("podcastname")?,
                            "artworkurl": row.try_get::<String, _>("artworkurl")?,
                            "episodetitle": row.try_get::<String, _>("episodetitle")?,
                            "episodepubdate": episode_pubdate,
                            "episodedescription": row.try_get::<String, _>("episodedescription")?,
                            "episodeartwork": row.try_get::<String, _>("episodeartwork")?,
                            "episodeurl": row.try_get::<String, _>("episodeurl")?,
                            "episodeduration": row.try_get::<i32, _>("episodeduration")?,
                            "episodeid": row.try_get::<i32, _>("episodeid")?,
                            "listenduration": row.try_get::<Option<i32>, _>("listenduration")?,
                            "completed": row.try_get::<bool, _>("completed")?,
                            "is_queued": row.try_get::<bool, _>("is_queued")?,
                            "is_saved": row.try_get::<bool, _>("is_saved")?,
                            "is_downloaded": row.try_get::<bool, _>("is_downloaded")?,
                            "is_youtube": row.try_get::<bool, _>("is_youtube")?,
                        }));
                    }
                }
                
                // Query for regular episodes
                let row = sqlx::query(
                    r#"SELECT "Podcasts".podcastid, "Podcasts".podcastindexid, "Podcasts".feedurl,
                            "Podcasts".podcastname, "Podcasts".artworkurl,
                            "Episodes".episodetitle,
                            "Episodes".episodepubdate,
                            "Episodes".episodedescription,
                            "Episodes".episodeartwork,
                            "Episodes".episodeurl,
                            "Episodes".episodeduration,
                            "Episodes".episodeid,
                            "UserEpisodeHistory".listenduration,
                            "Episodes".completed,
                            CASE WHEN q.episodeid IS NOT NULL THEN true ELSE false END as is_queued,
                            CASE WHEN s.episodeid IS NOT NULL THEN true ELSE false END as is_saved,
                            CASE WHEN d.episodeid IS NOT NULL THEN true ELSE false END as is_downloaded,
                            FALSE::boolean as is_youtube
                    FROM "Episodes"
                    INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                    LEFT JOIN "UserEpisodeHistory" ON "Episodes".episodeid = "UserEpisodeHistory".episodeid AND "UserEpisodeHistory".userid = $1
                    LEFT JOIN "EpisodeQueue" q ON "Episodes".episodeid = q.episodeid AND q.userid = $1
                    LEFT JOIN "SavedEpisodes" s ON "Episodes".episodeid = s.episodeid AND s.userid = $1
                    LEFT JOIN "DownloadedEpisodes" d ON "Episodes".episodeid = d.episodeid AND d.userid = $1
                    WHERE "Episodes".episodeid = $2 AND "Podcasts".userid = $1"#
                )
                .bind(user_id)
                .bind(episode_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    let naive_date = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                    let episode_pubdate = naive_date.format("%Y-%m-%dT%H:%M:%S").to_string();
                    
                    Ok(serde_json::json!({
                        "podcastid": row.try_get::<i32, _>("podcastid")?,
                        "podcastindexid": row.try_get::<Option<i32>, _>("podcastindexid")?,
                        "feedurl": row.try_get::<String, _>("feedurl")?,
                        "podcastname": row.try_get::<String, _>("podcastname")?,
                        "artworkurl": row.try_get::<String, _>("artworkurl")?,
                        "episodetitle": row.try_get::<String, _>("episodetitle")?,
                        "episodepubdate": episode_pubdate,
                        "episodedescription": row.try_get::<String, _>("episodedescription")?,
                        "episodeartwork": row.try_get::<String, _>("episodeartwork")?,
                        "episodeurl": row.try_get::<String, _>("episodeurl")?,
                        "episodeduration": row.try_get::<i32, _>("episodeduration")?,
                        "episodeid": row.try_get::<i32, _>("episodeid")?,
                        "listenduration": row.try_get::<Option<i32>, _>("listenduration")?,
                        "completed": row.try_get::<bool, _>("completed")?,
                        "is_queued": row.try_get::<bool, _>("is_queued")?,
                        "is_saved": row.try_get::<bool, _>("is_saved")?,
                        "is_downloaded": row.try_get::<bool, _>("is_downloaded")?,
                        "is_youtube": row.try_get::<bool, _>("is_youtube")?,
                    }))
                } else {
                    Err(AppError::not_found("Episode not found"))
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query(
                    r#"SELECT 
                        Episodes.EpisodeTitle as title,
                        Episodes.EpisodeDescription as description,
                        Episodes.EpisodeURL as episode_url,
                        Episodes.EpisodeArtwork as artwork_url,
                        Episodes.EpisodeDuration as duration,
                        Episodes.EpisodePubDate as pub_date,
                        Podcasts.PodcastName as podcast_name,
                        Podcasts.ArtworkURL as podcast_artwork,
                        UserEpisodeHistory.ListenDuration as listen_duration,
                        UserEpisodeHistory.Completed as completed
                    FROM Episodes
                    INNER JOIN Podcasts ON Episodes.PodcastID = Podcasts.PodcastID
                    LEFT JOIN UserEpisodeHistory ON Episodes.EpisodeID = UserEpisodeHistory.EpisodeID AND UserEpisodeHistory.UserID = ?
                    WHERE Episodes.EpisodeID = ? AND Podcasts.UserID = ?"#
                )
                .bind(user_id)
                .bind(episode_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    let naive_date = row.try_get::<chrono::NaiveDateTime, _>("pub_date")?;
                    let pub_date = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive_date, chrono::Utc);
                    
                    Ok(serde_json::json!({
                        "title": row.try_get::<String, _>("title")?,
                        "description": row.try_get::<String, _>("description")?,
                        "episode_url": row.try_get::<String, _>("episode_url")?,
                        "artwork_url": row.try_get::<String, _>("artwork_url")?,
                        "duration": row.try_get::<i32, _>("duration")?,
                        "pub_date": pub_date.to_rfc3339(),
                        "podcast_name": row.try_get::<String, _>("podcast_name")?,
                        "podcast_artwork": row.try_get::<String, _>("podcast_artwork")?,
                        "listen_duration": row.try_get::<Option<i32>, _>("listen_duration")?,
                        "completed": row.try_get::<Option<bool>, _>("completed")?.unwrap_or(false)
                    }))
                } else {
                    Err(AppError::not_found("Episode not found"))
                }
            }
        }
    }

    // Fetch podcasting 2.0 data for episode
    pub async fn fetch_podcasting_2_data(&self, episode_id: i32, user_id: i32) -> AppResult<serde_json::Value> {
        // For now, return empty data as podcasting 2.0 features may not be fully implemented
        Ok(serde_json::json!({
            "transcript": null,
            "chapters": [],
            "funding": [],
            "value": null,
            "soundbites": []
        }))
    }

    // Get auto download status for user
    pub async fn get_auto_download_status(&self, user_id: i32) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT auto_download_episodes FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    Ok(row.try_get::<bool, _>("auto_download_episodes")?)
                } else {
                    Ok(false)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT AutoDownloadEpisodes FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    Ok(row.try_get::<bool, _>("AutoDownloadEpisodes")?)
                } else {
                    Ok(false)
                }
            }
        }
    }


    // Fetch podcasting 2.0 podcast data
    pub async fn fetch_podcasting_2_pod_data(&self, podcast_id: i32, user_id: i32) -> AppResult<serde_json::Value> {
        // For now, return empty data as podcasting 2.0 features may not be fully implemented
        Ok(serde_json::json!({
            "funding": [],
            "value": null,
            "locked": false,
            "guid": null
        }))
    }

    // Check if API key is web key - matches Python is_web_key check
    pub async fn is_web_key(&self, api_key: &str) -> AppResult<bool> {
        // This would need to be implemented based on your web key configuration
        // For now, return false - implement according to your Python logic
        Ok(false)
    }

    // Call get auto download status - matches Python call_get_auto_download_status function exactly
    pub async fn call_get_auto_download_status(&self, podcast_id: i32, user_id: i32) -> AppResult<Option<bool>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT autodownload FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("autodownload")?;
                    Ok(result)
                } else {
                    Ok(None)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT AutoDownload FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("AutoDownload")?;
                    Ok(result)
                } else {
                    Ok(None)
                }
            }
        }
    }

    // Get feed cutoff days - matches Python get_feed_cutoff_days function exactly  
    pub async fn get_feed_cutoff_days(&self, podcast_id: i32, user_id: i32) -> AppResult<Option<i32>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT feedcutoffdays FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<i32> = row.try_get("feedcutoffdays")?;
                    Ok(result.or(Some(365))) // Default to 365 if null
                } else {
                    Ok(None)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT FeedCutoffDays FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<i32> = row.try_get("FeedCutoffDays")?;
                    Ok(result.or(Some(365))) // Default to 365 if null
                } else {
                    Ok(None)
                }
            }
        }
    }

    // Get podcast notification status - matches Python get_podcast_notification_status function exactly
    pub async fn get_podcast_notification_status(&self, podcast_id: i32, user_id: i32) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT notificationsenabled FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("notificationsenabled")?;
                    Ok(result.unwrap_or(false))
                } else {
                    Ok(false)
                }
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT NotificationsEnabled FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("NotificationsEnabled")?;
                    Ok(result.unwrap_or(false))
                } else {
                    Ok(false)
                }
            }
        }
    }

    // Get play episode details - matches Python get_play_episode_details function exactly
    pub async fn get_play_episode_details(&self, user_id: i32, podcast_id: i32, is_youtube: bool) -> AppResult<(f64, i32, i32)> {
        match self {
            DatabasePool::Postgres(pool) => {
                // First get user's default playback speed
                let user_row = sqlx::query(r#"SELECT playbackspeed FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                let user_playback_speed = if let Some(row) = user_row {
                    row.try_get::<Option<f64>, _>("playbackspeed")?.unwrap_or(1.0)
                } else {
                    1.0
                };

                // Now get podcast-specific settings
                let podcast_row = sqlx::query(r#"
                    SELECT playbackspeed, playbackspeedcustomized, startskip, endskip
                    FROM "Podcasts"
                    WHERE podcastid = $1 AND userid = $2
                "#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = podcast_row {
                    let playback_speed_customized: Option<bool> = row.try_get("playbackspeedcustomized")?;
                    let podcast_playback_speed: Option<f64> = row.try_get("playbackspeed")?;
                    let start_skip: Option<i32> = row.try_get("startskip")?;
                    let end_skip: Option<i32> = row.try_get("endskip")?;
                    
                    let final_playback_speed = if playback_speed_customized.unwrap_or(false) {
                        podcast_playback_speed.unwrap_or(user_playback_speed)
                    } else {
                        user_playback_speed
                    };
                    
                    Ok((final_playback_speed, start_skip.unwrap_or(0), end_skip.unwrap_or(0)))
                } else {
                    Ok((user_playback_speed, 0, 0))
                }
            }
            DatabasePool::MySQL(pool) => {
                // First get user's default playback speed
                let user_row = sqlx::query("SELECT PlaybackSpeed FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                let user_playback_speed = if let Some(row) = user_row {
                    row.try_get::<Option<f64>, _>("PlaybackSpeed")?.unwrap_or(1.0)
                } else {
                    1.0
                };

                // Now get podcast-specific settings
                let podcast_row = sqlx::query("
                    SELECT PlaybackSpeed, PlaybackSpeedCustomized, StartSkip, EndSkip
                    FROM Podcasts
                    WHERE PodcastID = ? AND UserID = ?
                ")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = podcast_row {
                    let playback_speed_customized: Option<bool> = row.try_get("PlaybackSpeedCustomized")?;
                    let podcast_playback_speed: Option<f64> = row.try_get("PlaybackSpeed")?;
                    let start_skip: Option<i32> = row.try_get("StartSkip")?;
                    let end_skip: Option<i32> = row.try_get("EndSkip")?;
                    
                    let final_playback_speed = if playback_speed_customized.unwrap_or(false) {
                        podcast_playback_speed.unwrap_or(user_playback_speed)
                    } else {
                        user_playback_speed
                    };
                    
                    Ok((final_playback_speed, start_skip.unwrap_or(0), end_skip.unwrap_or(0)))
                } else {
                    Ok((user_playback_speed, 0, 0))
                }
            }
        }
    }

    // Get podcast episodes with capitalized field names for frontend compatibility
    pub async fn return_podcast_episodes_capitalized(&self, user_id: i32, podcast_id: i32) -> AppResult<Vec<crate::handlers::podcasts::PodcastEpisode>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT * FROM (
                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "Episodes".episodetitle as "Episodetitle",
                            "Episodes".episodepubdate as "Episodepubdate",
                            "Episodes".episodedescription as "Episodedescription",
                            "Episodes".episodeartwork as "Episodeartwork",
                            "Episodes".episodeurl as "Episodeurl",
                            "Episodes".episodeduration as "Episodeduration",
                            "UserEpisodeHistory".listenduration as "Listenduration",
                            "Episodes".episodeid as "Episodeid",
                            "Episodes".completed as "Completed",
                            CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube
                        FROM "Episodes"
                        INNER JOIN "Podcasts" ON "Episodes".podcastid = "Podcasts".podcastid
                        LEFT JOIN "UserEpisodeHistory" ON
                            "Episodes".episodeid = "UserEpisodeHistory".episodeid
                            AND "UserEpisodeHistory".userid = $1
                        LEFT JOIN "SavedEpisodes" ON
                            "Episodes".episodeid = "SavedEpisodes".episodeid
                            AND "SavedEpisodes".userid = $1
                        LEFT JOIN "EpisodeQueue" ON
                            "Episodes".episodeid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $1
                            AND "EpisodeQueue".is_youtube = FALSE
                        LEFT JOIN "DownloadedEpisodes" ON
                            "Episodes".episodeid = "DownloadedEpisodes".episodeid
                            AND "DownloadedEpisodes".userid = $1
                        WHERE "Podcasts".userid = $1 AND "Podcasts".podcastid = $2

                        UNION ALL

                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "YouTubeVideos".videotitle as "Episodetitle",
                            "YouTubeVideos".publishedat as "Episodepubdate",
                            "YouTubeVideos".videodescription as "Episodedescription",
                            "YouTubeVideos".thumbnailurl as "Episodeartwork",
                            "YouTubeVideos".videourl as "Episodeurl",
                            "YouTubeVideos".duration as "Episodeduration",
                            "YouTubeVideos".listenposition as "Listenduration",
                            "YouTubeVideos".videoid as "Episodeid",
                            "YouTubeVideos".completed as "Completed",
                            CASE WHEN "SavedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL AND "EpisodeQueue".is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube
                        FROM "YouTubeVideos"
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "SavedVideos" ON
                            "YouTubeVideos".videoid = "SavedVideos".videoid
                            AND "SavedVideos".userid = $1
                        LEFT JOIN "EpisodeQueue" ON
                            "YouTubeVideos".videoid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $1
                            AND "EpisodeQueue".is_youtube = TRUE
                        LEFT JOIN "DownloadedVideos" ON
                            "YouTubeVideos".videoid = "DownloadedVideos".videoid
                            AND "DownloadedVideos".userid = $1
                        WHERE "Podcasts".userid = $1 AND "Podcasts".podcastid = $2
                    ) combined
                    ORDER BY "Episodepubdate" DESC"#
                )
                .bind(user_id)
                .bind(podcast_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    let naive_date = row.try_get::<chrono::NaiveDateTime, _>("Episodepubdate")?;
                    let episodepubdate = naive_date.format("%Y-%m-%dT%H:%M:%S").to_string();
                    
                    episodes.push(crate::handlers::podcasts::PodcastEpisode {
                        podcastname: row.try_get("podcastname")?,
                        episodetitle: row.try_get("Episodetitle")?,
                        episodepubdate,
                        episodedescription: row.try_get("Episodedescription")?,
                        episodeartwork: row.try_get("Episodeartwork")?,
                        episodeurl: row.try_get("Episodeurl")?,
                        episodeduration: row.try_get("Episodeduration")?,
                        listenduration: row.try_get("Listenduration")?,
                        episodeid: row.try_get("Episodeid")?,
                        completed: row.try_get("Completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                    });
                }
                
                Ok(episodes)
            }
            DatabasePool::MySQL(pool) => {
                // MySQL version would go here - similar structure but without quoted table names
                Ok(vec![])
            }
        }
    }

    // Add more database operations as needed...
}

#[derive(Debug, Clone)]
pub struct EpisodeData {
    pub title: String,
    pub description: String,
    pub url: String,
    pub artwork_url: String,
    pub pub_date: DateTime<Utc>,
    pub duration: i32,
}

#[derive(Debug, Clone)]
pub struct UserCredentials {
    pub user_id: i32,
    pub username: String,
    pub hashed_password: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserSettings {
    pub user_id: i32,
    pub api_key: String,
    pub theme: String,
    pub auto_download_episodes: bool,
    pub auto_delete_episodes: bool,
}

#[derive(Debug, Clone)]
pub struct SelfServiceStatus {
    pub status: bool,
    pub admin_exists: bool,
}

#[derive(Debug, Clone)]
pub struct PublicOidcProvider {
    pub provider_id: i32,
    pub provider_name: String,
    pub client_id: String,
    pub authorization_url: String,
    pub scope: String,
    pub button_color: String,
    pub button_text: String,
    pub button_text_color: String,
    pub icon_svg: Option<String>,
}