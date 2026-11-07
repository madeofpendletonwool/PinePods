//! Local (file-based) podcasts.

use super::*;

impl DatabasePool {
    // =========================================================
    // Local Podcast Functions
    // =========================================================

    pub async fn local_podcast_exists(&self, feed_url: &str, user_id: i32) -> AppResult<bool> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT 1 FROM "Podcasts" WHERE feedurl = $1 AND userid = $2"#)
                    .bind(feed_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(row.is_some())
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT 1 FROM Podcasts WHERE FeedURL = ? AND UserID = ?")
                    .bind(feed_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(row.is_some())
            }
        }
    }

    pub async fn add_local_podcast(
        &self,
        podcast_name: &str,
        feed_url: &str,
        description: &str,
        author: &str,
        artwork_url: &str,
        explicit: bool,
        user_id: i32,
    ) -> AppResult<i32> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(
                    r#"INSERT INTO "Podcasts"
                       (podcastname, artworkurl, author, categories, description, episodecount,
                        feedurl, websiteurl, explicit, userid, feedcutoffdays, podcastindexid)
                       VALUES ($1, $2, $3, $4, $5, 0, $6, '', $7, $8, 0, 0)
                       RETURNING podcastid"#,
                )
                .bind(podcast_name)
                .bind(artwork_url)
                .bind(author)
                .bind("{}") // empty categories JSON
                .bind(description)
                .bind(feed_url)
                .bind(explicit)
                .bind(user_id)
                .fetch_one(pool)
                .await?;

                let podcast_id: i32 = row.try_get("podcastid")?;

                sqlx::query(r#"UPDATE "UserStats" SET podcastsadded = podcastsadded + 1 WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                self.link_feed_or_warn(podcast_id).await;
                Ok(podcast_id)
            }
            DatabasePool::MySQL(pool) => {
                let result = sqlx::query(
                    "INSERT INTO Podcasts
                     (PodcastName, ArtworkURL, Author, Categories, Description, EpisodeCount,
                      FeedURL, WebsiteURL, Explicit, UserID, FeedCutoffDays, PodcastIndexID)
                     VALUES (?, ?, ?, ?, ?, 0, ?, '', ?, ?, 0, 0)",
                )
                .bind(podcast_name)
                .bind(artwork_url)
                .bind(author)
                .bind("{}") // empty categories JSON
                .bind(description)
                .bind(feed_url)
                .bind(explicit)
                .bind(user_id)
                .execute(pool)
                .await?;

                let podcast_id = result.last_insert_id() as i32;

                sqlx::query("UPDATE UserStats SET PodcastsAdded = PodcastsAdded + 1 WHERE UserID = ?")
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                self.link_feed_or_warn(podcast_id).await;
                Ok(podcast_id)
            }
        }
    }

    pub async fn add_local_episodes(
        &self,
        podcast_id: i32,
        user_id: i32,
        candidates: &[crate::handlers::local_podcast::LocalEpisodeCandidate],
    ) -> AppResult<()> {
        let feed_id = self.feed_of_podcast(podcast_id).await?;
        for candidate in candidates {
            let episode_url = format!("local://{}", candidate.file_path);
            let artwork = candidate.artwork_url.as_deref().unwrap_or("");

            let episode_id = match self {
                DatabasePool::Postgres(pool) => {
                    // Skip if already exists
                    let existing = sqlx::query(r#"SELECT episodeid FROM "Episodes" WHERE feedid = $1 AND episodeurl = $2"#)
                        .bind(feed_id)
                        .bind(&episode_url)
                        .fetch_optional(pool)
                        .await?;
                    if let Some(row) = existing {
                        row.try_get::<i32, _>("episodeid")?
                    } else {
                        let row = sqlx::query(
                            r#"INSERT INTO "Episodes"
                               (podcastid, feedid, episodetitle, episodedescription, episodeurl, episodeartwork, episodepubdate, episodeduration)
                               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                               RETURNING episodeid"#,
                        )
                        .bind(podcast_id)
                        .bind(feed_id)
                        .bind(&candidate.title)
                        .bind(&candidate.description)
                        .bind(&episode_url)
                        .bind(artwork)
                        .bind(candidate.pub_date)
                        .bind(candidate.duration)
                        .fetch_one(pool)
                        .await?;
                        row.try_get::<i32, _>("episodeid")?
                    }
                }
                DatabasePool::MySQL(pool) => {
                    // Skip if already exists
                    let existing = sqlx::query("SELECT EpisodeID FROM Episodes WHERE FeedID = ? AND EpisodeURL = ?")
                        .bind(feed_id)
                        .bind(&episode_url)
                        .fetch_optional(pool)
                        .await?;
                    if let Some(row) = existing {
                        row.try_get::<i32, _>("EpisodeID")?
                    } else {
                        let result = sqlx::query(
                            "INSERT INTO Episodes
                             (PodcastID, FeedID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                        )
                        .bind(podcast_id)
                        .bind(feed_id)
                        .bind(&candidate.title)
                        .bind(&candidate.description)
                        .bind(&episode_url)
                        .bind(artwork)
                        .bind(candidate.pub_date)
                        .bind(candidate.duration)
                        .execute(pool)
                        .await?;
                        result.last_insert_id() as i32
                    }
                }
            };

            // Get file size for DownloadedEpisodes
            let file_size = std::fs::metadata(&candidate.file_path)
                .map(|m| m.len() as i64)
                .unwrap_or(0);

            // Insert into DownloadedEpisodes so stream_episode() can serve the file
            match self {
                DatabasePool::Postgres(pool) => {
                    // Only insert if not already there
                    let existing = sqlx::query(r#"SELECT 1 FROM "DownloadedEpisodes" WHERE userid = $1 AND episodeid = $2"#)
                        .bind(user_id)
                        .bind(episode_id)
                        .fetch_optional(pool)
                        .await?;
                    if existing.is_none() {
                        sqlx::query(r#"INSERT INTO "DownloadedEpisodes" (userid, episodeid, downloadedsize, downloadedlocation) VALUES ($1, $2, $3, $4)"#)
                            .bind(user_id)
                            .bind(episode_id)
                            .bind(file_size)
                            .bind(&candidate.file_path)
                            .execute(pool)
                            .await?;
                    }
                }
                DatabasePool::MySQL(pool) => {
                    let existing = sqlx::query("SELECT 1 FROM DownloadedEpisodes WHERE UserID = ? AND EpisodeID = ?")
                        .bind(user_id)
                        .bind(episode_id)
                        .fetch_optional(pool)
                        .await?;
                    if existing.is_none() {
                        sqlx::query("INSERT INTO DownloadedEpisodes (UserID, EpisodeID, DownloadedSize, DownloadedLocation) VALUES (?, ?, ?, ?)")
                            .bind(user_id)
                            .bind(episode_id)
                            .bind(file_size)
                            .bind(&candidate.file_path)
                            .execute(pool)
                            .await?;
                    }
                }
            }
        }

        self.update_episode_count(podcast_id).await?;
        Ok(())
    }

    pub async fn get_feed_url_for_podcast(
        &self,
        podcast_id: i32,
        user_id: i32,
    ) -> AppResult<Option<String>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(r#"SELECT feedurl FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(row.and_then(|r| r.try_get::<String, _>("feedurl").ok()))
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query("SELECT FeedURL FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(row.and_then(|r| r.try_get::<String, _>("FeedURL").ok()))
            }
        }
    }

    pub async fn get_feed_url_for_episode(
        &self,
        episode_id: i32,
    ) -> AppResult<Option<String>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let row = sqlx::query(
                    r#"SELECT p.feedurl FROM "Episodes" e
                       JOIN "Podcasts" p ON e.feedid = p.feedid
                       WHERE e.episodeid = $1 LIMIT 1"#,
                )
                .bind(episode_id)
                .fetch_optional(pool)
                .await?;
                Ok(row.and_then(|r| r.try_get::<String, _>("feedurl").ok()))
            }
            DatabasePool::MySQL(pool) => {
                let row = sqlx::query(
                    "SELECT p.FeedURL FROM Episodes e
                     JOIN Podcasts p ON e.FeedID = p.FeedID
                     WHERE e.EpisodeID = ? LIMIT 1",
                )
                .bind(episode_id)
                .fetch_optional(pool)
                .await?;
                Ok(row.and_then(|r| r.try_get::<String, _>("FeedURL").ok()))
            }
        }
    }

    pub async fn get_local_episode_paths(
        &self,
        podcast_id: i32,
    ) -> AppResult<Vec<String>> {
        match self {
            DatabasePool::Postgres(pool) => {
                let rows = sqlx::query(
                    r#"SELECT de.downloadedlocation FROM "DownloadedEpisodes" de
                       JOIN "Episodes" e ON de.episodeid = e.episodeid
                       WHERE e.feedid = (SELECT feedid FROM "Podcasts" WHERE podcastid = $1)"#,
                )
                .bind(podcast_id)
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .iter()
                    .filter_map(|r| r.try_get::<String, _>("downloadedlocation").ok())
                    .collect())
            }
            DatabasePool::MySQL(pool) => {
                let rows = sqlx::query(
                    "SELECT de.DownloadedLocation FROM DownloadedEpisodes de
                     JOIN Episodes e ON de.EpisodeID = e.EpisodeID
                     WHERE e.FeedID = (SELECT FeedID FROM Podcasts WHERE PodcastID = ?)",
                )
                .bind(podcast_id)
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .iter()
                    .filter_map(|r| r.try_get::<String, _>("DownloadedLocation").ok())
                    .collect())
            }
        }
    }

    pub async fn update_podcast_artwork(
        &self,
        podcast_id: i32,
        user_id: i32,
        artwork_url: &str,
    ) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "Podcasts" SET artworkurl = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(artwork_url)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE Podcasts SET ArtworkURL = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(artwork_url)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }
}
