//! The play queue.

use super::*;

/// Where a newly-queued episode should land in a user's queue.
///
/// The queue is a "now playing + up next" list ordered by `queueposition` ASC
/// (position 1 = top = plays next). User-initiated adds go to the top or directly
/// under the currently-playing episode ("play next"); automated adds (feed refresh)
/// still append to the bottom to preserve chronological build-up.
#[derive(Debug, Clone, Copy)]
pub enum QueuePlacement {
    /// Insert at position 1 (top). Used when nothing is playing.
    Top,
    /// Append at MAX(position)+1 (bottom). Used by auto-queue-on-refresh.
    Bottom,
    /// Insert directly under the given currently-playing anchor. Falls back to
    /// `Top` when the anchor isn't in the queue. The anchor's `is_youtube` is
    /// required for an unambiguous lookup (podcast/YouTube ids can collide).
    Under {
        playing_episode_id: i32,
        playing_is_youtube: bool,
    },
}

impl DatabasePool {
    // Queue episode - matches Python queue_pod function
    /// Add an episode to a user's queue at the requested placement.
    ///
    /// `placement` controls where the new row lands: `Top` (position 1), `Bottom`
    /// (MAX+1, used by auto-queue-on-refresh), or `Under` the currently-playing
    /// anchor ("play next"). Existing rows at/after the insert position are shifted
    /// down by one to make room. Already-queued episodes are a no-op (not moved).
    pub async fn queue_episode(
        &self,
        episode_id: i32,
        user_id: i32,
        is_youtube: bool,
        placement: QueuePlacement,
    ) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                // First check if already queued
                let existing = q::query(
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

                let mut tx = q::begin(pool).await?;

                // Resolve the target position for this placement.
                let insert_pos: i32 = match placement {
                    QueuePlacement::Top => 1,
                    QueuePlacement::Bottom => {
                        let max_pos: i32 = q::query(
                            r#"SELECT COALESCE(MAX(queueposition), 0) as max_pos FROM "EpisodeQueue" WHERE userid = $1"#
                        )
                        .bind(user_id)
                        .fetch_one(&mut *tx)
                        .await?
                        .try_get("max_pos")?;
                        max_pos + 1
                    }
                    QueuePlacement::Under { playing_episode_id, playing_is_youtube } => {
                        let anchor: Option<i32> = q::query(
                            r#"SELECT queueposition FROM "EpisodeQueue"
                               WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                        )
                        .bind(playing_episode_id)
                        .bind(user_id)
                        .bind(playing_is_youtube)
                        .fetch_optional(&mut *tx)
                        .await?
                        .map(|row| row.try_get("queueposition"))
                        .transpose()?;
                        // Under the anchor, or top if the anchor isn't queued.
                        anchor.map(|p| p + 1).unwrap_or(1)
                    }
                };

                // Make room: shift everything at/after the insert position down by one.
                q::query(
                    r#"UPDATE "EpisodeQueue" SET queueposition = queueposition + 1
                       WHERE userid = $1 AND queueposition >= $2"#
                )
                .bind(user_id)
                .bind(insert_pos)
                .execute(&mut *tx)
                .await?;

                // Insert new queued episode
                q::query(
                    r#"INSERT INTO "EpisodeQueue" (episodeid, userid, queueposition, is_youtube)
                       VALUES ($1, $2, $3, $4)"#
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(insert_pos)
                .bind(is_youtube)
                .execute(&mut *tx)
                .await?;

                tx.commit().await?;
                Ok(())
            },
            mysql(pool) => {
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

                let mut tx = pool.begin().await?;

                // Resolve the target position for this placement.
                let insert_pos: i32 = match placement {
                    QueuePlacement::Top => 1,
                    QueuePlacement::Bottom => {
                        let max_pos: i32 = sqlx::query(
                            "SELECT COALESCE(MAX(QueuePosition), 0) as max_pos FROM EpisodeQueue WHERE UserID = ?"
                        )
                        .bind(user_id)
                        .fetch_one(&mut *tx)
                        .await?
                        .try_get("max_pos")?;
                        max_pos + 1
                    }
                    QueuePlacement::Under { playing_episode_id, playing_is_youtube } => {
                        let anchor: Option<i32> = sqlx::query(
                            "SELECT QueuePosition FROM EpisodeQueue
                             WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                        )
                        .bind(playing_episode_id)
                        .bind(user_id)
                        .bind(playing_is_youtube)
                        .fetch_optional(&mut *tx)
                        .await?
                        .map(|row| row.try_get("QueuePosition"))
                        .transpose()?;
                        anchor.map(|p| p + 1).unwrap_or(1)
                    }
                };

                // Make room: shift everything at/after the insert position down by one.
                sqlx::query(
                    "UPDATE EpisodeQueue SET QueuePosition = QueuePosition + 1
                     WHERE UserID = ? AND QueuePosition >= ?"
                )
                .bind(user_id)
                .bind(insert_pos)
                .execute(&mut *tx)
                .await?;

                // Insert new queued episode
                sqlx::query(
                    "INSERT INTO EpisodeQueue (EpisodeID, UserID, QueuePosition, is_youtube)
                     VALUES (?, ?, ?, ?)"
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(insert_pos)
                .bind(is_youtube)
                .execute(&mut *tx)
                .await?;

                tx.commit().await?;
                Ok(())
            },
        })
    }

    /// Move an episode to the top of a user's queue (position 1), or insert it there
    /// if it isn't queued yet. Used when a device starts playing an episode so the
    /// currently-playing item is always the queue anchor. Rows above the episode's
    /// old position shift down by one; rows below are untouched (no gap).
    pub async fn move_queue_episode_to_top(
        &self,
        episode_id: i32,
        user_id: i32,
        is_youtube: bool,
    ) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;

                let current_pos: Option<i32> = q::query(
                    r#"SELECT queueposition FROM "EpisodeQueue"
                       WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(is_youtube)
                .fetch_optional(&mut *tx)
                .await?
                .map(|row| row.try_get("queueposition"))
                .transpose()?;

                match current_pos {
                    Some(pos) if pos > 1 => {
                        // Shift the block above it down, then move it to the top.
                        q::query(
                            r#"UPDATE "EpisodeQueue" SET queueposition = queueposition + 1
                               WHERE userid = $1 AND queueposition < $2"#
                        )
                        .bind(user_id)
                        .bind(pos)
                        .execute(&mut *tx)
                        .await?;
                        q::query(
                            r#"UPDATE "EpisodeQueue" SET queueposition = 1
                               WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .bind(is_youtube)
                        .execute(&mut *tx)
                        .await?;
                    }
                    Some(_) => { /* already at position 1 — nothing to do */ }
                    None => {
                        // Not queued yet: make room at the top and insert.
                        q::query(
                            r#"UPDATE "EpisodeQueue" SET queueposition = queueposition + 1
                               WHERE userid = $1"#
                        )
                        .bind(user_id)
                        .execute(&mut *tx)
                        .await?;
                        q::query(
                            r#"INSERT INTO "EpisodeQueue" (episodeid, userid, queueposition, is_youtube)
                               VALUES ($1, $2, 1, $3)"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .bind(is_youtube)
                        .execute(&mut *tx)
                        .await?;
                    }
                }

                tx.commit().await?;
                Ok(())
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;

                let current_pos: Option<i32> = sqlx::query(
                    "SELECT QueuePosition FROM EpisodeQueue
                     WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                )
                .bind(episode_id)
                .bind(user_id)
                .bind(is_youtube)
                .fetch_optional(&mut *tx)
                .await?
                .map(|row| row.try_get("QueuePosition"))
                .transpose()?;

                match current_pos {
                    Some(pos) if pos > 1 => {
                        sqlx::query(
                            "UPDATE EpisodeQueue SET QueuePosition = QueuePosition + 1
                             WHERE UserID = ? AND QueuePosition < ?"
                        )
                        .bind(user_id)
                        .bind(pos)
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query(
                            "UPDATE EpisodeQueue SET QueuePosition = 1
                             WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .bind(is_youtube)
                        .execute(&mut *tx)
                        .await?;
                    }
                    Some(_) => { /* already at position 1 */ }
                    None => {
                        sqlx::query(
                            "UPDATE EpisodeQueue SET QueuePosition = QueuePosition + 1
                             WHERE UserID = ?"
                        )
                        .bind(user_id)
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query(
                            "INSERT INTO EpisodeQueue (EpisodeID, UserID, QueuePosition, is_youtube)
                             VALUES (?, ?, 1, ?)"
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .bind(is_youtube)
                        .execute(&mut *tx)
                        .await?;
                    }
                }

                tx.commit().await?;
                Ok(())
            },
        })
    }

    // Remove queued episode - matches Python remove_queued_pod function
    pub async fn remove_queued_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                // Get the queue position of the episode to be removed
                let position_row = q::query(
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
                    q::query(
                        r#"DELETE FROM "EpisodeQueue" 
                           WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(is_youtube)
                    .execute(pool)
                    .await?;

                    // Update positions of all episodes that were after the removed one
                    q::query(
                        r#"UPDATE "EpisodeQueue" SET queueposition = queueposition - 1 
                           WHERE userid = $1 AND queueposition > $2"#
                    )
                    .bind(user_id)
                    .bind(removed_position)
                    .execute(pool)
                    .await?;
                }

                Ok(())
            },
            mysql(pool) => {
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
            },
        })
    }

    pub async fn clear_queue(&self, user_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"DELETE FROM "EpisodeQueue" WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                Ok(())
            },
            mysql(pool) => {
                sqlx::query("DELETE FROM EpisodeQueue WHERE UserID = ?")
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                Ok(())
            },
        })
    }

    // Get queued episodes - matches Python get_queued_episodes function
    pub async fn get_queued_episodes(&self, user_id: i32) -> AppResult<Vec<crate::models::QueuedEpisode>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT * FROM (
                        SELECT
                            "Episodes".episodetitle as episodetitle,
                            "Podcasts".podcastname as podcastname,
                            "Podcasts".podcastid as podcastid,
                            "Episodes".episodepubdate as episodepubdate,
                            "Episodes".episodedescription as episodedescription,
                            CASE
                                WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                ELSE "Episodes".episodeartwork
                            END as episodeartwork,
                            "Episodes".episodeurl as episodeurl,
                            "EpisodeQueue".queueposition as queueposition,
                            "Episodes".episodeduration as episodeduration,
                            "EpisodeQueue".queuedate as queuedate,
                            "UserEpisodeHistory".listenduration as listenduration,
                            "Episodes".episodeid as episodeid,
                            COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) as completed,
                            CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            TRUE as queued,
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube,
                            "Episodes".is_video as is_video
                        FROM "EpisodeQueue"
                        INNER JOIN "Episodes" ON "EpisodeQueue".episodeid = "Episodes".episodeid
                        INNER JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid AND "Podcasts".userid = "EpisodeQueue".userid
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
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
                            "YouTubeVideos".podcastid as podcastid,
                            "YouTubeVideos".publishedat as episodepubdate,
                            "YouTubeVideos".videodescription as episodedescription,
                            CASE 
                                WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                ELSE "YouTubeVideos".thumbnailurl
                            END as episodeartwork,
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
                            TRUE as is_youtube,
                            TRUE as is_video
                        FROM "EpisodeQueue"
                        INNER JOIN "YouTubeVideos" ON "EpisodeQueue".episodeid = "YouTubeVideos".videoid
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
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
                        podcastid: row.try_get("podcastid")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        queueposition: row.try_get("queueposition").ok(),
                        episodeduration: row.try_get("episodeduration")?,
                        queuedate: {
                        let naive = row.try_get::<chrono::NaiveDateTime, _>("queuedate")?;
                        naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                    },
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                        is_video: row.try_get("is_video")?,
                    });
                }
                Ok(episodes)
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT * FROM (
                        SELECT
                            Episodes.EpisodeTitle as episodetitle,
                            Podcasts.PodcastName as podcastname,
                            Podcasts.PodcastID as podcastid,
                            Episodes.EpisodePubDate as episodepubdate,
                            Episodes.EpisodeDescription as episodedescription,
                            CASE
                                WHEN Podcasts.UsePodcastCoversCustomized = TRUE AND Podcasts.UsePodcastCovers = TRUE THEN Podcasts.ArtworkURL
                                WHEN Users.UsePodcastCovers = TRUE THEN Podcasts.ArtworkURL
                                ELSE Episodes.EpisodeArtwork
                            END as episodeartwork,
                            Episodes.EpisodeURL as episodeurl,
                            EpisodeQueue.QueuePosition as queueposition,
                            Episodes.EpisodeDuration as episodeduration,
                            EpisodeQueue.QueueDate as queuedate,
                            UserEpisodeHistory.ListenDuration as listenduration,
                            Episodes.EpisodeID as episodeid,
                            COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = Podcasts.UserID), 0) as completed,
                            CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            TRUE as queued,
                            CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube,
                            Episodes.IsVideo as is_video
                        FROM EpisodeQueue
                        INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID
                        INNER JOIN Podcasts ON Episodes.FeedID = Podcasts.FeedID AND Podcasts.UserID = EpisodeQueue.UserID
                        LEFT JOIN Users ON Podcasts.UserID = Users.UserID
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
                            YouTubeVideos.PodcastID as podcastid,
                            YouTubeVideos.PublishedAt as episodepubdate,
                            YouTubeVideos.VideoDescription as episodedescription,
                            CASE 
                                WHEN Podcasts.UsePodcastCoversCustomized = TRUE AND Podcasts.UsePodcastCovers = TRUE THEN Podcasts.ArtworkURL
                                WHEN Users.UsePodcastCovers = TRUE THEN Podcasts.ArtworkURL
                                ELSE YouTubeVideos.ThumbnailURL
                            END as episodeartwork,
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
                            TRUE as is_youtube,
                            TRUE as is_video
                        FROM EpisodeQueue
                        INNER JOIN YouTubeVideos ON EpisodeQueue.EpisodeID = YouTubeVideos.VideoID
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        LEFT JOIN Users ON Podcasts.UserID = Users.UserID
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
                        podcastid: row.try_get("podcastid")?,
                        episodepubdate: {
                            let dt = row.try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>("episodepubdate")?;
                            dt.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        queueposition: row.try_get("queueposition").ok(),
                        episodeduration: row.try_get("episodeduration")?,
                        queuedate: {
                        let dt = row.try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>("queuedate")?;
                        dt.format("%Y-%m-%dT%H:%M:%S").to_string()
                    },
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                        is_video: row.try_get("is_video")?,
                    });
                }
                Ok(episodes)
            },
        })
    }

    // Reorder queue - matches Python reorder_queued_episodes function
    /// Rewrite a user's queue positions to `1..N` in the given order.
    ///
    /// Each item is `(episode_id, is_youtube)`. `is_youtube` disambiguates a podcast
    /// `EpisodeID` from a YouTube `VideoID` that share an integer, so only the
    /// intended row is repositioned. `None` preserves the legacy id-only match for
    /// older clients that don't send a type discriminator.
    pub async fn reorder_queue(
        &self,
        user_id: i32,
        items: Vec<(i32, Option<bool>)>,
    ) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;

                for (index, (episode_id, is_youtube)) in items.iter().enumerate() {
                    let new_position = (index + 1) as i32;
                    match is_youtube {
                        Some(flag) => {
                            q::query(
                                r#"UPDATE "EpisodeQueue" SET queueposition = $1
                                   WHERE episodeid = $2 AND userid = $3 AND is_youtube = $4"#
                            )
                            .bind(new_position)
                            .bind(episode_id)
                            .bind(user_id)
                            .bind(flag)
                            .execute(&mut *tx)
                            .await?;
                        }
                        None => {
                            q::query(
                                r#"UPDATE "EpisodeQueue" SET queueposition = $1
                                   WHERE episodeid = $2 AND userid = $3"#
                            )
                            .bind(new_position)
                            .bind(episode_id)
                            .bind(user_id)
                            .execute(&mut *tx)
                            .await?;
                        }
                    }
                }

                tx.commit().await?;
                Ok(())
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;

                for (index, (episode_id, is_youtube)) in items.iter().enumerate() {
                    let new_position = (index + 1) as i32;
                    match is_youtube {
                        Some(flag) => {
                            sqlx::query(
                                "UPDATE EpisodeQueue SET QueuePosition = ?
                                 WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                            )
                            .bind(new_position)
                            .bind(episode_id)
                            .bind(user_id)
                            .bind(flag)
                            .execute(&mut *tx)
                            .await?;
                        }
                        None => {
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
                    }
                }

                tx.commit().await?;
                Ok(())
            },
        })
    }

    /// Bulk-add episodes to a user's queue at the requested placement, preserving
    /// the given order. YouTube videos and podcast episodes both live in
    /// `EpisodeQueue` (keyed by `is_youtube`) — there is no separate table. Each new
    /// item is inserted with a shift-to-make-room so the batch stays contiguous.
    pub async fn bulk_queue_episodes(
        &self,
        episode_ids: Vec<i32>,
        user_id: i32,
        is_youtube: bool,
        placement: QueuePlacement,
    ) -> AppResult<(i32, i32)> {
        if episode_ids.is_empty() {
            return Ok((0, 0));
        }

        let mut processed = 0;
        let mut failed = 0;

        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;

                // Resolve the position the first new item should occupy.
                let mut next_pos: i32 = match placement {
                    QueuePlacement::Top => 1,
                    QueuePlacement::Bottom => {
                        let max_pos: i32 = q::query(
                            r#"SELECT COALESCE(MAX(queueposition), 0) as max_pos FROM "EpisodeQueue" WHERE userid = $1"#
                        )
                        .bind(user_id)
                        .fetch_one(&mut *tx)
                        .await?
                        .try_get("max_pos")?;
                        max_pos + 1
                    }
                    QueuePlacement::Under { playing_episode_id, playing_is_youtube } => {
                        let anchor: Option<i32> = q::query(
                            r#"SELECT queueposition FROM "EpisodeQueue"
                               WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                        )
                        .bind(playing_episode_id)
                        .bind(user_id)
                        .bind(playing_is_youtube)
                        .fetch_optional(&mut *tx)
                        .await?
                        .map(|row| row.try_get("queueposition"))
                        .transpose()?;
                        anchor.map(|p| p + 1).unwrap_or(1)
                    }
                };

                for episode_id in episode_ids {
                    // Check if already queued to avoid duplicates
                    let existing = q::query(
                        r#"SELECT queueid FROM "EpisodeQueue" WHERE episodeid = $1 AND userid = $2 AND is_youtube = $3"#
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(is_youtube)
                    .fetch_optional(&mut *tx)
                    .await?;

                    if existing.is_none() {
                        // Make room at next_pos, then insert there.
                        q::query(
                            r#"UPDATE "EpisodeQueue" SET queueposition = queueposition + 1
                               WHERE userid = $1 AND queueposition >= $2"#
                        )
                        .bind(user_id)
                        .bind(next_pos)
                        .execute(&mut *tx)
                        .await?;
                        match q::query(
                            r#"INSERT INTO "EpisodeQueue" (episodeid, userid, queueposition, is_youtube) VALUES ($1, $2, $3, $4)"#
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .bind(next_pos)
                        .bind(is_youtube)
                        .execute(&mut *tx)
                        .await {
                            Ok(_) => { processed += 1; next_pos += 1; }
                            Err(_) => failed += 1,
                        }
                    }
                }

                tx.commit().await?;
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;

                let mut next_pos: i32 = match placement {
                    QueuePlacement::Top => 1,
                    QueuePlacement::Bottom => {
                        let max_pos: i32 = sqlx::query(
                            "SELECT COALESCE(MAX(QueuePosition), 0) as max_pos FROM EpisodeQueue WHERE UserID = ?"
                        )
                        .bind(user_id)
                        .fetch_one(&mut *tx)
                        .await?
                        .try_get("max_pos")?;
                        max_pos + 1
                    }
                    QueuePlacement::Under { playing_episode_id, playing_is_youtube } => {
                        let anchor: Option<i32> = sqlx::query(
                            "SELECT QueuePosition FROM EpisodeQueue
                             WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                        )
                        .bind(playing_episode_id)
                        .bind(user_id)
                        .bind(playing_is_youtube)
                        .fetch_optional(&mut *tx)
                        .await?
                        .map(|row| row.try_get("QueuePosition"))
                        .transpose()?;
                        anchor.map(|p| p + 1).unwrap_or(1)
                    }
                };

                for episode_id in episode_ids {
                    let existing = sqlx::query(
                        "SELECT QueueID FROM EpisodeQueue WHERE EpisodeID = ? AND UserID = ? AND is_youtube = ?"
                    )
                    .bind(episode_id)
                    .bind(user_id)
                    .bind(is_youtube)
                    .fetch_optional(&mut *tx)
                    .await?;

                    if existing.is_none() {
                        sqlx::query(
                            "UPDATE EpisodeQueue SET QueuePosition = QueuePosition + 1
                             WHERE UserID = ? AND QueuePosition >= ?"
                        )
                        .bind(user_id)
                        .bind(next_pos)
                        .execute(&mut *tx)
                        .await?;
                        match sqlx::query(
                            "INSERT INTO EpisodeQueue (EpisodeID, UserID, QueuePosition, is_youtube) VALUES (?, ?, ?, ?)"
                        )
                        .bind(episode_id)
                        .bind(user_id)
                        .bind(next_pos)
                        .bind(is_youtube)
                        .execute(&mut *tx)
                        .await {
                            Ok(_) => { processed += 1; next_pos += 1; }
                            Err(_) => failed += 1,
                        }
                    }
                }

                tx.commit().await?;
            },
        });

        Ok((processed, failed))
    }
}
