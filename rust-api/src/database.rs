use sqlx::{MySql, Pool, Postgres, Row};
use std::time::Duration;
use crate::{config::Config, error::{AppError, AppResult}};

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
                let row = sqlx::query(r#"SELECT * FROM "APIKeys" WHERE APIKey = $1"#)
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
                let row = sqlx::query(r#"SELECT "Hashed_PW" FROM "Users" WHERE "Username" = $1"#)
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<String, _>("Hashed_PW")?
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
                let user_row = sqlx::query(r#"SELECT "UserID" FROM "Users" WHERE "Username" = $1"#)
                    .bind(username)
                    .fetch_one(pool)
                    .await?;
                
                let user_id: i32 = user_row.try_get("UserID")?;
                
                // Then get API key
                let api_row = sqlx::query(r#"SELECT "APIKey" FROM "APIKeys" WHERE "UserID" = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(api_row.try_get("APIKey")?)
            }
            DatabasePool::MySQL(pool) => {
                // First get UserID
                let user_row = sqlx::query("SELECT UserID FROM Users WHERE Username = ?")
                    .bind(username)
                    .fetch_one(pool)
                    .await?;
                
                let user_id: i32 = user_row.try_get("UserID")?;
                
                // Then get API key
                let api_row = sqlx::query("SELECT APIKey FROM APIKeys WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(api_row.try_get("APIKey")?)
            }
        }
    }

    // Get user ID from API key - matches Python get_api_user function
    pub async fn get_user_id_from_api_key(&self, api_key: &str) -> AppResult<i32> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT "UserID" FROM "APIKeys" WHERE "APIKey" = $1 LIMIT 1"#)
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("UserID")?)
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
                let row = sqlx::query(r#"SELECT * FROM "Users" WHERE "UserID" = $1"#)
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
                    r#"SELECT "UserID" as user_id, "Username" as username, "Hashed_PW" as hashed_password, "Email" as email
                     FROM "Users" WHERE "Username" = $1"#
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
                    r#"SELECT user_id, api_key, theme, auto_download_episodes, auto_delete_episodes
                       FROM "UserSettings" WHERE user_id = $1"#
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
                let row = sqlx::query(r#"SELECT user_id FROM "UserSettings" WHERE api_key = $1"#)
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("user_id")?)
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT user_id FROM UserSettings WHERE api_key = ?")
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("user_id")?)
            }
        }
    }

    // Get episodes for user - matches Python return_episodes function
    pub async fn return_episodes(&self, user_id: i32) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT 
                        p."PodcastName" as podcastname,
                        e."EpisodeTitle" as episodetitle, 
                        e."EpisodePubDate" as episodepubdate,
                        e."EpisodeDescription" as episodedescription,
                        e."EpisodeArtwork" as episodeartwork,
                        e."EpisodeURL" as episodeurl,
                        e."EpisodeDuration" as episodeduration,
                        ls."ListenDuration" as listenduration,
                        e."EpisodeID" as episodeid,
                        COALESCE(ls."Completed", false) as completed,
                        COALESCE(se."Saved", false) as saved,
                        COALESCE(qe."Queued", false) as queued,
                        COALESCE(de."Downloaded", false) as downloaded,
                        p."IsYoutube" as is_youtube
                    FROM "Episodes" e
                    JOIN "Podcasts" p ON e."PodcastID" = p."PodcastID"
                    LEFT JOIN "ListenStats" ls ON e."EpisodeID" = ls."EpisodeID" AND ls."UserID" = $1
                    LEFT JOIN "SavedEpisodes" se ON e."EpisodeID" = se."EpisodeID" AND se."UserID" = $1
                    LEFT JOIN "QueuedEpisodes" qe ON e."EpisodeID" = qe."EpisodeID" AND qe."UserID" = $1
                    LEFT JOIN "DownloadedEpisodes" de ON e."EpisodeID" = de."EpisodeID" AND de."UserID" = $1
                    WHERE p."UserID" = $1
                    ORDER BY e."EpisodePubDate" DESC"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::handlers::podcasts::Episode {
                        podcastname: row.try_get("podcastname")?,
                        episodetitle: row.try_get("episodetitle")?,
                        episodepubdate: row.try_get("episodepubdate")?,
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
                    "SELECT 
                        p.PodcastName as podcastname,
                        e.EpisodeTitle as episodetitle, 
                        e.EpisodePubDate as episodepubdate,
                        e.EpisodeDescription as episodedescription,
                        e.EpisodeArtwork as episodeartwork,
                        e.EpisodeURL as episodeurl,
                        e.EpisodeDuration as episodeduration,
                        ls.ListenDuration as listenduration,
                        e.EpisodeID as episodeid,
                        COALESCE(ls.Completed, false) as completed,
                        COALESCE(se.Saved, false) as saved,
                        COALESCE(qe.Queued, false) as queued,
                        COALESCE(de.Downloaded, false) as downloaded,
                        p.IsYoutube as is_youtube
                    FROM Episodes e
                    JOIN Podcasts p ON e.PodcastID = p.PodcastID
                    LEFT JOIN ListenStats ls ON e.EpisodeID = ls.EpisodeID AND ls.UserID = ?
                    LEFT JOIN SavedEpisodes se ON e.EpisodeID = se.EpisodeID AND se.UserID = ?
                    LEFT JOIN QueuedEpisodes qe ON e.EpisodeID = qe.EpisodeID AND qe.UserID = ?
                    LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID AND de.UserID = ?
                    WHERE p.UserID = ?
                    ORDER BY e.EpisodePubDate DESC"
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
                        episodepubdate: row.try_get("episodepubdate")?,
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

    // Add podcast - simplified version of Python add_podcast function
    pub async fn add_podcast(
        &self,
        podcast_values: &crate::handlers::podcasts::PodcastValues,
        podcast_index_id: i64,
    ) -> AppResult<(i32, i32)> {
        match self {
            DatabasePool::Postgres(pool) => {
                // This is a simplified version - the full Python function is very complex
                // For now, just insert basic podcast data
                let row = sqlx::query(
                    r#"INSERT INTO "Podcasts" 
                       ("PodcastName", "ArtworkURL", "Author", "Categories", "Description", 
                        "EpisodeCount", "FeedURL", "WebsiteURL", "Explicit", "UserID")
                       VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                       RETURNING "PodcastID""#
                )
                .bind(&podcast_values.pod_title)
                .bind(&podcast_values.pod_artwork)
                .bind(&podcast_values.pod_author)
                .bind(serde_json::to_string(&podcast_values.categories)?)
                .bind(&podcast_values.pod_description)
                .bind(podcast_values.pod_episode_count)
                .bind(&podcast_values.pod_feed_url)
                .bind(&podcast_values.pod_website)
                .bind(podcast_values.pod_explicit)
                .bind(podcast_values.user_id)
                .fetch_one(pool)
                .await?;
                
                let podcast_id: i32 = row.try_get("PodcastID")?;
                
                // Return dummy first episode ID for now
                // TODO: Implement proper episode parsing and insertion
                Ok((podcast_id, 0))
            }
            DatabasePool::MySQL(pool) => {
                let result = sqlx::query(
                    "INSERT INTO Podcasts 
                     (PodcastName, ArtworkURL, Author, Categories, Description, 
                      EpisodeCount, FeedURL, WebsiteURL, Explicit, UserID)
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                )
                .bind(&podcast_values.pod_title)
                .bind(&podcast_values.pod_artwork)
                .bind(&podcast_values.pod_author)
                .bind(serde_json::to_string(&podcast_values.categories)?)
                .bind(&podcast_values.pod_description)
                .bind(podcast_values.pod_episode_count)
                .bind(&podcast_values.pod_feed_url)
                .bind(&podcast_values.pod_website)
                .bind(podcast_values.pod_explicit)
                .bind(podcast_values.user_id)
                .execute(pool)
                .await?;
                
                let podcast_id = result.last_insert_id() as i32;
                
                // Return dummy first episode ID for now
                // TODO: Implement proper episode parsing and insertion
                Ok((podcast_id, 0))
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
                        "IsYoutube" as is_youtube,
                        "AutoDownload" as auto_download,
                        "Username" as username,
                        "Password" as password,
                        "FeedCutoffDays" as feed_cutoff_days
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
                        is_youtube: row.try_get("is_youtube")?,
                        auto_download: row.try_get("auto_download")?,
                        username: row.try_get("username").ok(),
                        password: row.try_get("password").ok(),
                        feed_cutoff_days: row.try_get("feed_cutoff_days").ok(),
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
                        IsYoutube as is_youtube,
                        AutoDownload as auto_download,
                        Username as username,
                        Password as password,
                        FeedCutoffDays as feed_cutoff_days
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
                        is_youtube: row.try_get("is_youtube")?,
                        auto_download: row.try_get("auto_download")?,
                        username: row.try_get("username").ok(),
                        password: row.try_get("password").ok(),
                        feed_cutoff_days: row.try_get("feed_cutoff_days").ok(),
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
                        "PodcastID" as podcastid,
                        COALESCE("PodcastName", 'Unknown Podcast') as podcastname,
                        CASE 
                            WHEN "ArtworkURL" IS NULL OR "ArtworkURL" = '' 
                            THEN '/static/assets/default-podcast.png'
                            ELSE "ArtworkURL"
                        END as artworkurl,
                        COALESCE("Description", 'No description available') as description,
                        COALESCE("EpisodeCount", 0) as episodecount,
                        COALESCE("WebsiteURL", '') as websiteurl,
                        COALESCE("FeedURL", '') as feedurl,
                        COALESCE("Author", 'Unknown Author') as author,
                        COALESCE("Categories", '') as categories,
                        COALESCE("Explicit", false) as explicit,
                        COALESCE("PodcastIndexID", 0) as podcastindexid
                    FROM "Podcasts"
                    WHERE "UserID" = $1
                    ORDER BY "PodcastName""#
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
                        p."PodcastID" as podcastid,
                        COALESCE(p."PodcastName", 'Unknown Podcast') as podcastname,
                        CASE 
                            WHEN p."ArtworkURL" IS NULL OR p."ArtworkURL" = '' 
                            THEN '/static/assets/default-podcast.png'
                            ELSE p."ArtworkURL"
                        END as artworkurl,
                        COALESCE(p."Description", 'No description available') as description,
                        COALESCE(p."EpisodeCount", 0) as episodecount,
                        COALESCE(p."WebsiteURL", '') as websiteurl,
                        COALESCE(p."FeedURL", '') as feedurl,
                        COALESCE(p."Author", 'Unknown Author') as author,
                        COALESCE(p."Categories", '') as categories,
                        COALESCE(p."Explicit", false) as explicit,
                        COALESCE(p."PodcastIndexID", 0) as podcastindexid,
                        COUNT(ueh."UserEpisodeHistoryID") as play_count,
                        COUNT(DISTINCT ueh."EpisodeID") as episodes_played,
                        MIN(e."EpisodePubDate") as oldest_episode_date,
                        COALESCE(p."IsYoutube", false) as is_youtube
                    FROM "Podcasts" p
                    LEFT JOIN "Episodes" e ON p."PodcastID" = e."PodcastID"
                    LEFT JOIN "UserEpisodeHistory" ueh ON e."EpisodeID" = ueh."EpisodeID" AND ueh."UserID" = $1
                    WHERE p."UserID" = $1
                    GROUP BY p."PodcastID", p."PodcastName", p."ArtworkURL", p."Description", 
                             p."EpisodeCount", p."WebsiteURL", p."FeedURL", p."Author", 
                             p."Categories", p."Explicit", p."PodcastIndexID", p."IsYoutube"
                    ORDER BY p."PodcastName""#
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
                    r#"SELECT COALESCE("Timezone", 'UTC') as timezone, 
                              COALESCE("TimeFormat", 12) as hour_pref,
                              "DateFormat" as date_format
                       FROM "Users" WHERE "UserID" = $1"#
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
                    r#"SELECT "PodcastID" FROM "Podcasts" 
                       WHERE "UserID" = $1 AND "PodcastName" = $2 AND "FeedURL" = $3"#
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
                        JOIN "Podcasts" ON "Episodes"."PodcastID" = "Podcasts"."PodcastID"
                        WHERE "Podcasts"."UserID" = $1 
                          AND "Episodes"."EpisodeTitle" = $2 
                          AND "Episodes"."EpisodeURL" = $3
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
                    r#"SELECT "QueueID" FROM "EpisodeQueue" 
                       WHERE "EpisodeID" = $1 AND "UserID" = $2 AND "is_youtube" = $3"#
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
                    r#"SELECT COALESCE(MAX("QueuePosition"), 0) as max_pos FROM "EpisodeQueue" WHERE "UserID" = $1"#
                )
                .bind(user_id)
                .fetch_one(pool)
                .await?;
                
                let max_pos: i32 = max_pos_row.try_get("max_pos")?;
                let new_position = max_pos + 1;

                // Insert new queued episode
                sqlx::query(
                    r#"INSERT INTO "EpisodeQueue" ("EpisodeID", "UserID", "QueuePosition", "is_youtube") 
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
                    r#"SELECT "QueuePosition" FROM "EpisodeQueue" 
                       WHERE "EpisodeID" = $1 AND "UserID" = $2 AND "is_youtube" = $3"#
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
                        r#"DELETE FROM "EpisodeQueue" 
                           WHERE "EpisodeID" = $1 AND "UserID" = $2 AND "is_youtube" = $3"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(is_youtube)
                    .execute(pool)
                    .await?;

                    // Update positions of all episodes that were after the removed one
                    sqlx::query(
                        r#"UPDATE "EpisodeQueue" SET "QueuePosition" = "QueuePosition" - 1 
                           WHERE "UserID" = $1 AND "QueuePosition" > $2"#
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
                    r#"SELECT 
                        e."EpisodeTitle" as episodetitle,
                        p."PodcastName" as podcastname,
                        e."EpisodePubDate" as episodepubdate,
                        e."EpisodeDescription" as episodedescription,
                        e."EpisodeArtwork" as episodeartwork,
                        e."EpisodeURL" as episodeurl,
                        eq."QueuePosition" as queueposition,
                        e."EpisodeDuration" as episodeduration,
                        eq."QueueDate" as queuedate,
                        ls."ListenDuration" as listenduration,
                        e."EpisodeID" as episodeid,
                        COALESCE(ls."Completed", false) as completed,
                        COALESCE(se."Saved", false) as saved,
                        true as queued,
                        COALESCE(de."Downloaded", false) as downloaded,
                        eq."is_youtube" as is_youtube
                    FROM "EpisodeQueue" eq
                    JOIN "Episodes" e ON eq."EpisodeID" = e."EpisodeID"
                    JOIN "Podcasts" p ON e."PodcastID" = p."PodcastID"
                    LEFT JOIN "ListenStats" ls ON e."EpisodeID" = ls."EpisodeID" AND ls."UserID" = $1
                    LEFT JOIN "SavedEpisodes" se ON e."EpisodeID" = se."EpisodeID" AND se."UserID" = $1
                    LEFT JOIN "DownloadedEpisodes" de ON e."EpisodeID" = de."EpisodeID" AND de."UserID" = $1
                    WHERE eq."UserID" = $1
                    ORDER BY eq."QueuePosition" ASC"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::QueuedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: row.try_get("episodepubdate")?,
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
                    "SELECT 
                        e.EpisodeTitle as episodetitle,
                        p.PodcastName as podcastname,
                        e.EpisodePubDate as episodepubdate,
                        e.EpisodeDescription as episodedescription,
                        e.EpisodeArtwork as episodeartwork,
                        e.EpisodeURL as episodeurl,
                        eq.QueuePosition as queueposition,
                        e.EpisodeDuration as episodeduration,
                        eq.QueueDate as queuedate,
                        ls.ListenDuration as listenduration,
                        e.EpisodeID as episodeid,
                        COALESCE(ls.Completed, false) as completed,
                        COALESCE(se.Saved, false) as saved,
                        true as queued,
                        COALESCE(de.Downloaded, false) as downloaded,
                        eq.is_youtube as is_youtube
                    FROM EpisodeQueue eq
                    JOIN Episodes e ON eq.EpisodeID = e.EpisodeID
                    JOIN Podcasts p ON e.PodcastID = p.PodcastID
                    LEFT JOIN ListenStats ls ON e.EpisodeID = ls.EpisodeID AND ls.UserID = ?
                    LEFT JOIN SavedEpisodes se ON e.EpisodeID = se.EpisodeID AND se.UserID = ?
                    LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID AND de.UserID = ?
                    WHERE eq.UserID = ?
                    ORDER BY eq.QueuePosition ASC"
                )
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::QueuedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: row.try_get("episodepubdate")?,
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
                        r#"SELECT "SaveID" FROM "SavedEpisodes" WHERE "EpisodeID" = $1 AND "UserID" = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                    if existing.is_none() {
                        sqlx::query(
                            r#"INSERT INTO "SavedEpisodes" ("EpisodeID", "UserID") VALUES ($1, $2)"#
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
                        r#"DELETE FROM "SavedEpisodes" WHERE "EpisodeID" = $1 AND "UserID" = $2"#
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

    // Get saved episodes - matches Python saved_episode_list function
    pub async fn get_saved_episodes(&self, user_id: i32) -> AppResult<Vec<crate::models::SavedEpisode>> {
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
                        ls."ListenDuration" as listenduration,
                        e."EpisodeID" as episodeid,
                        COALESCE(p."WebsiteURL", '') as websiteurl,
                        COALESCE(ls."Completed", false) as completed,
                        true as saved,
                        COALESCE(qe."Queued", false) as queued,
                        COALESCE(de."Downloaded", false) as downloaded,
                        false as is_youtube
                    FROM "SavedEpisodes" se
                    JOIN "Episodes" e ON se."EpisodeID" = e."EpisodeID"
                    JOIN "Podcasts" p ON e."PodcastID" = p."PodcastID"
                    LEFT JOIN "ListenStats" ls ON e."EpisodeID" = ls."EpisodeID" AND ls."UserID" = $1
                    LEFT JOIN "QueuedEpisodes" qe ON e."EpisodeID" = qe."EpisodeID" AND qe."UserID" = $1
                    LEFT JOIN "DownloadedEpisodes" de ON e."EpisodeID" = de."EpisodeID" AND de."UserID" = $1
                    WHERE se."UserID" = $1
                    
                    UNION ALL
                    
                    SELECT 
                        yv."VideoTitle" as episodetitle,
                        yc."ChannelName" as podcastname,
                        yv."VideoUploadDate" as episodepubdate,
                        yv."VideoDescription" as episodedescription,
                        yv."VideoThumbnail" as episodeartwork,
                        yv."VideoURL" as episodeurl,
                        COALESCE(yv."VideoDuration", 0) as episodeduration,
                        yvh."ListenDuration" as listenduration,
                        yv."VideoID" as episodeid,
                        COALESCE(yc."ChannelURL", '') as websiteurl,
                        CASE WHEN yvh."ListenPosition" >= 0.95 THEN true ELSE false END as completed,
                        true as saved,
                        COALESCE(qv."Queued", false) as queued,
                        false as downloaded,
                        true as is_youtube
                    FROM "SavedVideos" sv
                    JOIN "YouTubeVideos" yv ON sv."VideoID" = yv."VideoID"
                    JOIN "YouTubeChannels" yc ON yv."ChannelID" = yc."ChannelID"
                    LEFT JOIN "UserVideoHistory" yvh ON yv."VideoID" = yvh."VideoID" AND yvh."UserID" = $1
                    LEFT JOIN "QueuedVideos" qv ON yv."VideoID" = qv."VideoID" AND qv."UserID" = $1
                    WHERE sv."UserID" = $1
                    
                    ORDER BY episodepubdate DESC"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::SavedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: row.try_get("episodepubdate")?,
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
                // Similar MySQL implementation with unquoted identifiers
                let rows = sqlx::query(
                    "SELECT 
                        e.EpisodeTitle as episodetitle,
                        p.PodcastName as podcastname,
                        e.EpisodePubDate as episodepubdate,
                        e.EpisodeDescription as episodedescription,
                        e.EpisodeArtwork as episodeartwork,
                        e.EpisodeURL as episodeurl,
                        e.EpisodeDuration as episodeduration,
                        ls.ListenDuration as listenduration,
                        e.EpisodeID as episodeid,
                        COALESCE(p.WebsiteURL, '') as websiteurl,
                        COALESCE(ls.Completed, false) as completed,
                        true as saved,
                        COALESCE(qe.Queued, false) as queued,
                        COALESCE(de.Downloaded, false) as downloaded,
                        false as is_youtube
                    FROM SavedEpisodes se
                    JOIN Episodes e ON se.EpisodeID = e.EpisodeID
                    JOIN Podcasts p ON e.PodcastID = p.PodcastID
                    LEFT JOIN ListenStats ls ON e.EpisodeID = ls.EpisodeID AND ls.UserID = ?
                    LEFT JOIN QueuedEpisodes qe ON e.EpisodeID = qe.EpisodeID AND qe.UserID = ?
                    LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID AND de.UserID = ?
                    WHERE se.UserID = ?
                    ORDER BY episodepubdate DESC"
                )
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
                        episodepubdate: row.try_get("episodepubdate")?,
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
                        episodepubdate: row.try_get("episodepubdate")?,
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
                        episodepubdate: row.try_get("episodepubdate")?,
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

    // Add more database operations as needed...
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