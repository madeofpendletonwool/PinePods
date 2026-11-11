//! Listening history.

use super::*;

impl DatabasePool {
    // Record podcast history - matches Python record_podcast_history function
    pub async fn record_podcast_history(&self, episode_id: i32, user_id: i32, episode_pos: f32, is_youtube: bool) -> AppResult<()> {
        // Position in seconds, stored as-is (same unit record_listen_duration stores).
        let listen_duration = episode_pos.max(0.0) as i32;

        dialect_match!(self, {
            pg(pool) => {
                if is_youtube {
                    // Insert or update video history
                    q::query(
                        r#"INSERT INTO "UserVideoHistory" ("videoid", "userid", "listenduration", "listendate")
                           VALUES ($1, $2, $3, NOW())
                           ON CONFLICT ("videoid", "userid") 
                           DO UPDATE SET "listenduration" = $3, "listendate" = NOW()"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(listen_duration)
                    .execute(pool)
                    .await?;
                } else {
                    // Insert or update episode history
                    q::query(
                        r#"INSERT INTO "UserEpisodeHistory" ("episodeid", "userid", "listenduration", "listendate")
                           VALUES ($1, $2, $3, NOW())
                           ON CONFLICT ("episodeid", "userid") 
                           DO UPDATE SET "listenduration" = $3, "listendate" = NOW()"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(listen_duration)
                    .execute(pool)
                    .await?;
                }
                Ok(())
            },
            mysql(pool) => {
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
            },
        })
    }

    // Get user history - matches Python user_history function
    pub async fn get_user_history(&self, user_id: i32) -> AppResult<Vec<crate::models::HistoryEpisode>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
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
                    JOIN "Podcasts" p ON e."FeedID" = p."FeedID"
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
            },
            mysql(pool) => {
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
                    JOIN Podcasts p ON e.FeedID = p.FeedID
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
            },
        })
    }

    // Get user history - matches Python user_history function exactly with YouTube UNION
    pub async fn user_history(
        &self,
        user_id: i32,
        limit: i64,
        offset: i64,
        sort_by: &str,
        sort_order: &str,
        filter: &str,
    ) -> AppResult<(Vec<serde_json::Value>, i64)> {
        // Validate and map sort column/direction to prevent SQL injection
        let order_col = match sort_by {
            "duration" => "episodeduration",
            "title"    => "episodetitle",
            _          => "listendate",  // "date" or any unknown value
        };
        let order_dir = if sort_order == "asc" { "ASC" } else { "DESC" };
        let filter_clause = match filter {
            "completed"   => " WHERE completed = true",
            "in_progress" => " WHERE completed = false AND listenduration IS NOT NULL AND listenduration > 0",
            _             => "",
        };

        dialect_match!(self, {
            pg(pool) => {
                let null_sort_last = if order_col == "listendate" { " NULLS LAST" } else { "" };
                let sql = format!(
                    r#"SELECT *, COUNT(*) OVER() AS total_count FROM (
                        SELECT
                            "Episodes".episodeid as episodeid,
                            "UserEpisodeHistory".listendate as listendate,
                            "UserEpisodeHistory".listenduration as listenduration,
                            "Episodes".episodetitle as episodetitle,
                            "Episodes".episodedescription as episodedescription,
                            CASE
                                WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                ELSE "Episodes".episodeartwork
                            END as episodeartwork,
                            "Episodes".episodeurl as episodeurl,
                            "Episodes".episodeduration as episodeduration,
                            "Podcasts".podcastname as podcastname,
                            "Episodes".episodepubdate as episodepubdate,
                            COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) as completed,
                            CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube
                        FROM "UserEpisodeHistory"
                        JOIN "Episodes" ON "UserEpisodeHistory".episodeid = "Episodes".episodeid
                        JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                        LEFT JOIN "SavedEpisodes" ON "Episodes".episodeid = "SavedEpisodes".episodeid AND "SavedEpisodes".userid = $1
                        LEFT JOIN "EpisodeQueue" ON "Episodes".episodeid = "EpisodeQueue".episodeid AND "EpisodeQueue".userid = $1
                        LEFT JOIN "DownloadedEpisodes" ON "Episodes".episodeid = "DownloadedEpisodes".episodeid AND "DownloadedEpisodes".userid = $1
                        WHERE "UserEpisodeHistory".userid = $1

                        UNION ALL

                        SELECT
                            "YouTubeVideos".videoid as episodeid,
                            NULL as listendate,
                            "YouTubeVideos".listenposition as listenduration,
                            "YouTubeVideos".videotitle as episodetitle,
                            "YouTubeVideos".videodescription as episodedescription,
                            CASE
                                WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                ELSE "YouTubeVideos".thumbnailurl
                            END as episodeartwork,
                            "YouTubeVideos".videourl as episodeurl,
                            "YouTubeVideos".duration as episodeduration,
                            "Podcasts".podcastname as podcastname,
                            "YouTubeVideos".publishedat as episodepubdate,
                            "YouTubeVideos".completed as completed,
                            CASE WHEN "SavedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube
                        FROM "YouTubeVideos"
                        JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                        LEFT JOIN "SavedVideos" ON "YouTubeVideos".videoid = "SavedVideos".videoid AND "SavedVideos".userid = $1
                        LEFT JOIN "EpisodeQueue" ON "YouTubeVideos".videoid = "EpisodeQueue".episodeid AND "EpisodeQueue".userid = $1
                        LEFT JOIN "DownloadedVideos" ON "YouTubeVideos".videoid = "DownloadedVideos".videoid AND "DownloadedVideos".userid = $1
                        WHERE "YouTubeVideos".listenposition > 0
                          AND "Podcasts".userid = $1
                    ) combined_results{filter_clause}
                    ORDER BY {order_col} {order_dir}{null_sort_last}
                    LIMIT $2 OFFSET $3"#
                );

                let rows = q::query(sqlx::AssertSqlSafe(sql.as_str()))
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
                    let listendate = row.try_get::<Option<chrono::NaiveDateTime>, _>("listendate")?
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string());
                    let episodepubdate = row.try_get::<Option<chrono::NaiveDateTime>, _>("episodepubdate")?
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string());

                    episodes.push(serde_json::json!({
                        "episodeid": row.get::<Option<i32>, _>("episodeid"),
                        "listendate": listendate,
                        "listenduration": row.get::<Option<i32>, _>("listenduration"),
                        "episodetitle": row.get::<Option<String>, _>("episodetitle"),
                        "episodedescription": row.get::<Option<String>, _>("episodedescription"),
                        "episodeartwork": row.get::<Option<String>, _>("episodeartwork"),
                        "episodeurl": row.get::<Option<String>, _>("episodeurl"),
                        "episodeduration": row.get::<Option<i32>, _>("episodeduration"),
                        "podcastname": row.get::<Option<String>, _>("podcastname"),
                        "episodepubdate": episodepubdate,
                        "completed": row.get::<Option<bool>, _>("completed"),
                        "saved": row.get::<Option<bool>, _>("saved"),
                        "queued": row.get::<Option<bool>, _>("queued"),
                        "downloaded": row.get::<Option<bool>, _>("downloaded"),
                        "is_youtube": row.get::<Option<bool>, _>("is_youtube")
                    }));
                }
                Ok((episodes, total))
            },
            mysql(pool) => {
                let mysql_filter = match filter {
                    "completed"   => " WHERE completed = 1",
                    "in_progress" => " WHERE completed = 0 AND listenduration IS NOT NULL AND listenduration > 0",
                    _             => "",
                };
                let sql = format!(
                    "SELECT *, COUNT(*) OVER() AS total_count FROM (
                        SELECT
                            e.EpisodeID as episodeid,
                            ueh.ListenDate as listendate,
                            ueh.ListenDuration as listenduration,
                            e.EpisodeTitle as episodetitle,
                            e.EpisodeDescription as episodedescription,
                            CASE
                                WHEN p.UsePodcastCoversCustomized = 1 AND p.UsePodcastCovers = 1 THEN p.ArtworkURL
                                WHEN u.UsePodcastCovers = 1 THEN p.ArtworkURL
                                ELSE e.EpisodeArtwork
                            END as episodeartwork,
                            e.EpisodeURL as episodeurl,
                            e.EpisodeDuration as episodeduration,
                            p.PodcastName as podcastname,
                            e.EpisodePubDate as episodepubdate,
                            COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) as completed,
                            CASE WHEN se.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN eq.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN de.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            0 as is_youtube
                        FROM UserEpisodeHistory ueh
                        JOIN Episodes e ON ueh.EpisodeID = e.EpisodeID
                        JOIN Podcasts p ON e.FeedID = p.FeedID
                        LEFT JOIN Users u ON p.UserID = u.UserID
                        LEFT JOIN SavedEpisodes se ON e.EpisodeID = se.EpisodeID AND se.UserID = ?
                        LEFT JOIN EpisodeQueue eq ON e.EpisodeID = eq.EpisodeID AND eq.UserID = ?
                        LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID AND de.UserID = ?
                        WHERE ueh.UserID = ?

                        UNION ALL

                        SELECT
                            yv.VideoID as episodeid,
                            NULL as listendate,
                            yv.ListenPosition as listenduration,
                            yv.VideoTitle as episodetitle,
                            yv.VideoDescription as episodedescription,
                            CASE
                                WHEN p.UsePodcastCoversCustomized = 1 AND p.UsePodcastCovers = 1 THEN p.ArtworkURL
                                WHEN u.UsePodcastCovers = 1 THEN p.ArtworkURL
                                ELSE yv.ThumbnailURL
                            END as episodeartwork,
                            yv.VideoURL as episodeurl,
                            yv.Duration as episodeduration,
                            p.PodcastName as podcastname,
                            yv.PublishedAt as episodepubdate,
                            yv.Completed as completed,
                            CASE WHEN sv.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN eq.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN dv.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            1 as is_youtube
                        FROM YouTubeVideos yv
                        JOIN Podcasts p ON yv.PodcastID = p.PodcastID
                        LEFT JOIN Users u ON p.UserID = u.UserID
                        LEFT JOIN SavedVideos sv ON yv.VideoID = sv.VideoID AND sv.UserID = ?
                        LEFT JOIN EpisodeQueue eq ON yv.VideoID = eq.EpisodeID AND eq.UserID = ?
                        LEFT JOIN DownloadedVideos dv ON yv.VideoID = dv.VideoID AND dv.UserID = ?
                        WHERE yv.ListenPosition > 0
                          AND p.UserID = ?
                    ) combined_results{mysql_filter}
                    ORDER BY {order_col} {order_dir}
                    LIMIT ? OFFSET ?"
                );

                let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(user_id)  // SavedEpisodes join
                    .bind(user_id)  // EpisodeQueue join
                    .bind(user_id)  // DownloadedEpisodes join
                    .bind(user_id)  // WHERE clause
                    .bind(user_id)  // SavedVideos join
                    .bind(user_id)  // EpisodeQueue join (YouTube)
                    .bind(user_id)  // DownloadedVideos join
                    .bind(user_id)  // WHERE clause (YouTube)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?;

                let total: i64 = rows.first()
                    .and_then(|r| r.try_get::<i64, _>("total_count").ok())
                    .unwrap_or(0);

                let mut episodes = Vec::new();
                for row in rows {
                    let listendate = row.try_get::<Option<chrono::NaiveDateTime>, _>("listendate")?
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string());
                    let episodepubdate = row.try_get::<Option<chrono::NaiveDateTime>, _>("episodepubdate")?
                        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string());

                    episodes.push(serde_json::json!({
                        "episodeid": row.get::<Option<i32>, _>("episodeid"),
                        "listendate": listendate,
                        "listenduration": row.get::<Option<i32>, _>("listenduration"),
                        "episodetitle": row.get::<Option<String>, _>("episodetitle"),
                        "episodedescription": row.get::<Option<String>, _>("episodedescription"),
                        "episodeartwork": row.get::<Option<String>, _>("episodeartwork"),
                        "episodeurl": row.get::<Option<String>, _>("episodeurl"),
                        "episodeduration": row.get::<Option<i32>, _>("episodeduration"),
                        "podcastname": row.get::<Option<String>, _>("podcastname"),
                        "episodepubdate": episodepubdate,
                        "completed": row.get::<Option<bool>, _>("completed"),
                        "saved": row.get::<Option<bool>, _>("saved"),
                        "queued": row.get::<Option<bool>, _>("queued"),
                        "downloaded": row.get::<Option<bool>, _>("downloaded"),
                        "is_youtube": row.get::<Option<bool>, _>("is_youtube")
                    }));
                }
                Ok((episodes, total))
            },
        })
    }

    // Remove episode from history - used by Nextcloud episode actions
    pub async fn remove_episode_from_history(&self, user_id: i32, episode_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"DELETE FROM "UserEpisodeHistory" WHERE userid = $1 AND episodeid = $2"#)
                    .bind(user_id)
                    .bind(episode_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("DELETE FROM UserEpisodeHistory WHERE UserID = ? AND EpisodeID = ?")
                    .bind(user_id)
                    .bind(episode_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }
}
