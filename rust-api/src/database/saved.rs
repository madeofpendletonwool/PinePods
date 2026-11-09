//! Saved episodes.

use super::*;

impl DatabasePool {
    // Save episode - matches Python save_episode function
    pub async fn save_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                if is_youtube {
                    // Check if already saved
                    let existing = q::query(
                        r#"SELECT "SaveID" FROM "SavedVideos" WHERE "VideoID" = $1 AND "UserID" = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                    if existing.is_none() {
                        q::query(
                            r#"INSERT INTO "SavedVideos" ("VideoID", "UserID") VALUES ($1, $2)"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;

                        // Update UserStats table - increment EpisodesSaved count
                        q::query(r#"UPDATE "UserStats" SET episodessaved = episodessaved + 1 WHERE userid = $1"#)
                            .bind(user_id)
                            .execute(pool)
                            .await?;
                    }
                } else {
                    // Check if already saved
                    let existing = q::query(
                        r#"SELECT saveid FROM "SavedEpisodes" WHERE episodeid = $1 AND userid = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                    if existing.is_none() {
                        q::query(
                            r#"INSERT INTO "SavedEpisodes" (episodeid, userid) VALUES ($1, $2)"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;

                        // Update UserStats table - increment EpisodesSaved count
                        q::query(r#"UPDATE "UserStats" SET episodessaved = episodessaved + 1 WHERE userid = $1"#)
                            .bind(user_id)
                            .execute(pool)
                            .await?;
                    }
                }
                Ok(())
            },
            mysql(pool) => {
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

                        // Update UserStats table - increment EpisodesSaved count
                        sqlx::query("UPDATE UserStats SET EpisodesSaved = EpisodesSaved + 1 WHERE UserID = ?")
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

                        // Update UserStats table - increment EpisodesSaved count
                        sqlx::query("UPDATE UserStats SET EpisodesSaved = EpisodesSaved + 1 WHERE UserID = ?")
                            .bind(user_id)
                            .execute(pool)
                            .await?;
                    }
                }
                Ok(())
            },
        })
    }

    // Remove saved episode - matches Python remove_saved_episode function
    pub async fn remove_saved_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                if is_youtube {
                    let result = q::query(
                        r#"DELETE FROM "SavedVideos" WHERE "VideoID" = $1 AND "UserID" = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                    // Only update UserStats if a row was actually deleted
                    if result.rows_affected() > 0 {
                        q::query(r#"UPDATE "UserStats" SET episodessaved = episodessaved - 1 WHERE userid = $1"#)
                            .bind(user_id)
                            .execute(pool)
                            .await?;
                    }
                } else {
                    let result = q::query(
                        r#"DELETE FROM "SavedEpisodes" WHERE episodeid = $1 AND userid = $2"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                    // Only update UserStats if a row was actually deleted
                    if result.rows_affected() > 0 {
                        q::query(r#"UPDATE "UserStats" SET episodessaved = episodessaved - 1 WHERE userid = $1"#)
                            .bind(user_id)
                            .execute(pool)
                            .await?;
                    }
                }
                Ok(())
            },
            mysql(pool) => {
                if is_youtube {
                    let result = sqlx::query(
                        "DELETE FROM SavedVideos WHERE VideoID = ? AND UserID = ?"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                    // Only update UserStats if a row was actually deleted
                    if result.rows_affected() > 0 {
                        sqlx::query("UPDATE UserStats SET EpisodesSaved = EpisodesSaved - 1 WHERE UserID = ?")
                            .bind(user_id)
                            .execute(pool)
                            .await?;
                    }
                } else {
                    let result = sqlx::query(
                        "DELETE FROM SavedEpisodes WHERE EpisodeID = ? AND UserID = ?"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                    // Only update UserStats if a row was actually deleted
                    if result.rows_affected() > 0 {
                        sqlx::query("UPDATE UserStats SET EpisodesSaved = EpisodesSaved - 1 WHERE UserID = ?")
                            .bind(user_id)
                            .execute(pool)
                            .await?;
                    }
                }
                Ok(())
            },
        })
    }

    // Get saved episodes - matches Python saved_episode_list function
    pub async fn get_saved_episodes(
        &self,
        user_id: i32,
        limit: i64,
        offset: i64,
        sort_by: &str,
        sort_order: &str,
        filter: &str,
    ) -> AppResult<(Vec<crate::models::SavedEpisode>, i64)> {
        // Validate and map sort column/direction to prevent SQL injection
        let order_col = match sort_by {
            "duration" => "episodeduration",
            "title"    => "episodetitle",
            _          => "savedate",  // "date" or any unknown value
        };
        let order_dir = if sort_order == "asc" { "ASC" } else { "DESC" };
        let filter_clause = match filter {
            "completed"   => " WHERE completed = true",
            "in_progress" => " WHERE completed = false AND listenduration IS NOT NULL AND listenduration > 0",
            _             => "",
        };

        dialect_match!(self, {
            pg(pool) => {
                let sql = format!(
                    r#"SELECT *, COUNT(*) OVER() AS total_count FROM (
                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "Episodes".episodetitle as episodetitle,
                            "Episodes".episodepubdate as episodepubdate,
                            "Episodes".episodedescription as episodedescription,
                            "Episodes".episodeid as episodeid,
                            CASE
                                WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                ELSE "Episodes".episodeartwork
                            END as episodeartwork,
                            "Episodes".episodeurl as episodeurl,
                            "Episodes".episodeduration as episodeduration,
                            "Podcasts".websiteurl as websiteurl,
                            "UserEpisodeHistory".listenduration as listenduration,
                            COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) as completed,
                            TRUE as saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube,
                            "Podcasts".podcastid as podcastid,
                            "SavedEpisodes".savedate as savedate
                        FROM "SavedEpisodes"
                        INNER JOIN "Episodes" ON "SavedEpisodes".episodeid = "Episodes".episodeid
                        INNER JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid AND "Podcasts".userid = "SavedEpisodes".userid
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
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
                            CASE
                                WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                ELSE "YouTubeVideos".thumbnailurl
                            END as episodeartwork,
                            "YouTubeVideos".videourl as episodeurl,
                            "YouTubeVideos".duration as episodeduration,
                            "Podcasts".websiteurl as websiteurl,
                            "YouTubeVideos".listenposition as listenduration,
                            "YouTubeVideos".completed as completed,
                            TRUE as saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL AND "EpisodeQueue".is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube,
                            "Podcasts".podcastid as podcastid,
                            "SavedVideos".savedate as savedate
                        FROM "SavedVideos"
                        INNER JOIN "YouTubeVideos" ON "SavedVideos".videoid = "YouTubeVideos".videoid
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                        LEFT JOIN "EpisodeQueue" ON
                            "SavedVideos".videoid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $5
                            AND "EpisodeQueue".is_youtube = TRUE
                        LEFT JOIN "DownloadedVideos" ON
                            "SavedVideos".videoid = "DownloadedVideos".videoid
                            AND "DownloadedVideos".userid = $6
                        WHERE "SavedVideos".userid = $7
                    ) combined{filter_clause}
                    ORDER BY {order_col} {order_dir} NULLS LAST
                    LIMIT $8 OFFSET $9"#
                );

                let rows = q::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?;

                let total: i64 = rows.first()
                    .and_then(|r| r.try_get::<i64, _>("total_count").ok())
                    .unwrap_or(0);

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
                        podcastid: row.try_get("podcastid").ok(),
                        savedate: row.try_get::<chrono::NaiveDateTime, _>("savedate").ok()
                            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
                    });
                }
                Ok((episodes, total))
            },
            mysql(pool) => {
                // MySQL uses ? placeholders; COUNT(*) OVER() requires MySQL 8+
                let mysql_filter = match filter {
                    "completed"   => " WHERE completed = 1",
                    "in_progress" => " WHERE completed = 0 AND listenduration IS NOT NULL AND listenduration > 0",
                    _             => "",
                };
                let sql = format!(
                    "SELECT *, COUNT(*) OVER() AS total_count FROM (
                        SELECT
                            Podcasts.PodcastName as podcastname,
                            Episodes.EpisodeTitle as episodetitle,
                            Episodes.EpisodePubDate as episodepubdate,
                            Episodes.EpisodeDescription as episodedescription,
                            Episodes.EpisodeID as episodeid,
                            CASE
                                WHEN Podcasts.UsePodcastCoversCustomized = 1 AND Podcasts.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                                WHEN Users.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                                ELSE Episodes.EpisodeArtwork
                            END as episodeartwork,
                            Episodes.EpisodeURL as episodeurl,
                            Episodes.EpisodeDuration as episodeduration,
                            Podcasts.WebsiteURL as websiteurl,
                            UserEpisodeHistory.ListenDuration as listenduration,
                            COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = Podcasts.UserID), 0) as completed,
                            TRUE as saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube,
                            Podcasts.PodcastID as podcastid,
                            SavedEpisodes.SaveDate as savedate
                        FROM SavedEpisodes
                        INNER JOIN Episodes ON SavedEpisodes.EpisodeID = Episodes.EpisodeID
                        INNER JOIN Podcasts ON Episodes.FeedID = Podcasts.FeedID AND Podcasts.UserID = SavedEpisodes.UserID
                        LEFT JOIN Users ON Podcasts.UserID = Users.UserID
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
                            CASE
                                WHEN Podcasts.UsePodcastCoversCustomized = 1 AND Podcasts.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                                WHEN Users.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                                ELSE YouTubeVideos.ThumbnailURL
                            END as episodeartwork,
                            YouTubeVideos.VideoURL as episodeurl,
                            YouTubeVideos.Duration as episodeduration,
                            Podcasts.WebsiteURL as websiteurl,
                            YouTubeVideos.ListenPosition as listenduration,
                            YouTubeVideos.Completed as completed,
                            TRUE as saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL AND EpisodeQueue.is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN DownloadedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube,
                            Podcasts.PodcastID as podcastid,
                            SavedVideos.SaveDate as savedate
                        FROM SavedVideos
                        INNER JOIN YouTubeVideos ON SavedVideos.VideoID = YouTubeVideos.VideoID
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        LEFT JOIN Users ON Podcasts.UserID = Users.UserID
                        LEFT JOIN EpisodeQueue ON
                            SavedVideos.VideoID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = TRUE
                        LEFT JOIN DownloadedVideos ON
                            SavedVideos.VideoID = DownloadedVideos.VideoID
                            AND DownloadedVideos.UserID = ?
                        WHERE SavedVideos.UserID = ?
                    ) combined{mysql_filter}
                    ORDER BY {order_col} {order_dir}
                    LIMIT ? OFFSET ?"
                );

                let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?;

                let total: i64 = rows.first()
                    .and_then(|r| r.try_get::<i64, _>("total_count").ok())
                    .unwrap_or(0);

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
                        podcastid: row.try_get("podcastid").ok(),
                        savedate: row.try_get::<chrono::NaiveDateTime, _>("savedate").ok()
                            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
                    });
                }
                Ok((episodes, total))
            },
        })
    }

    pub async fn bulk_save_episodes(&self, episode_ids: Vec<i32>, user_id: i32, is_youtube: bool) -> AppResult<(i32, i32)> {
        if episode_ids.is_empty() {
            return Ok((0, 0));
        }

        let mut processed = 0;
        let mut failed = 0;

        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                
                if is_youtube {
                    for episode_id in episode_ids {
                        // Check if already saved to avoid duplicates
                        let existing = q::query(
                            r#"SELECT "SaveID" FROM "SavedVideos" WHERE "VideoID" = $1 AND "UserID" = $2"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                        
                        if existing.is_none() {
                            match q::query(
                                r#"INSERT INTO "SavedVideos" ("VideoID", "UserID") VALUES ($1, $2)"#
                            )
                            .bind(episode_id)
                            .bind(user_id)
                            .execute(&mut *tx)
                            .await {
                                Ok(_) => processed += 1,
                                Err(_) => failed += 1,
                            }
                        }
                    }
                } else {
                    for episode_id in episode_ids {
                        // Check if already saved to avoid duplicates  
                        let existing = q::query(
                            r#"SELECT saveid FROM "SavedEpisodes" WHERE episodeid = $1 AND userid = $2"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                        
                        if existing.is_none() {
                            match q::query(
                                r#"INSERT INTO "SavedEpisodes" (episodeid, userid) VALUES ($1, $2)"#
                            )
                            .bind(episode_id)
                            .bind(user_id)
                            .execute(&mut *tx)
                            .await {
                                Ok(_) => processed += 1,
                                Err(_) => failed += 1,
                            }
                        }
                    }
                }
                
                tx.commit().await?;
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;
                
                if is_youtube {
                    for episode_id in episode_ids {
                        let existing = sqlx::query(
                            "SELECT SaveID FROM SavedVideos WHERE VideoID = ? AND UserID = ?"
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                        
                        if existing.is_none() {
                            match sqlx::query(
                                "INSERT INTO SavedVideos (VideoID, UserID) VALUES (?, ?)"
                            )
                            .bind(episode_id)
                            .bind(user_id)
                            .execute(&mut *tx)
                            .await {
                                Ok(_) => processed += 1,
                                Err(_) => failed += 1,
                            }
                        }
                    }
                } else {
                    for episode_id in episode_ids {
                        let existing = sqlx::query(
                            "SELECT SaveID FROM SavedEpisodes WHERE EpisodeID = ? AND UserID = ?"
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .fetch_optional(&mut *tx)
                        .await?;
                        
                        if existing.is_none() {
                            match sqlx::query(
                                "INSERT INTO SavedEpisodes (EpisodeID, UserID) VALUES (?, ?)"
                            )
                            .bind(episode_id)
                            .bind(user_id)
                            .execute(&mut *tx)
                            .await {
                                Ok(_) => processed += 1,
                                Err(_) => failed += 1,
                            }
                        }
                    }
                }
                
                tx.commit().await?;
            },
        });

        Ok((processed, failed))
    }
}
