//! Subscribing, removing, merging and looking up podcasts.

use super::*;

// Unsubscribing: the episodes of the podcast's feed (or of the podcast itself, before it was
// linked to a feed), as a subquery. Ids are integers, so formatting them in is safe.
pub(super) fn feed_episodes_sql(feed_id: Option<i32>, podcast_id: i32, pg: bool) -> String {
    match (feed_id, pg) {
        (Some(f), true) => format!(r#"SELECT episodeid FROM "Episodes" WHERE feedid = {f}"#),
        (Some(f), false) => format!("SELECT EpisodeID FROM Episodes WHERE FeedID = {f}"),
        (None, true) => format!(r#"SELECT episodeid FROM "Episodes" WHERE podcastid = {podcast_id}"#),
        (None, false) => format!("SELECT EpisodeID FROM Episodes WHERE PodcastID = {podcast_id}"),
    }
}

// The unsubscribing user's own rows for those episodes; each statement binds the user once.
pub(super) fn user_episode_rows_pg(episodes: &str) -> Vec<String> {
    vec![
        format!(r#"DELETE FROM "PlaylistContents" WHERE episodeid IN ({episodes}) AND playlistid IN (SELECT playlistid FROM "Playlists" WHERE userid = $1)"#),
        format!(r#"DELETE FROM "CollectionEpisodes" WHERE episodeid IN ({episodes}) AND collectionid IN (SELECT collectionid FROM "Collections" WHERE userid = $1)"#),
        format!(r#"DELETE FROM "EpisodeAdSkipReview" WHERE userid = $1 AND segmentid IN (SELECT segmentid FROM "EpisodeSkipSegments" WHERE episodeid IN ({episodes}))"#),
        format!(r#"DELETE FROM "SharedEpisodes" WHERE sharedby = $1 AND episodeid IN ({episodes})"#),
        format!(r#"DELETE FROM "UserEpisodeHistory" WHERE userid = $1 AND episodeid IN ({episodes})"#),
        format!(r#"DELETE FROM "UserEpisodeState" WHERE userid = $1 AND episodeid IN ({episodes})"#),
        format!(r#"DELETE FROM "DownloadedEpisodes" WHERE userid = $1 AND episodeid IN ({episodes})"#),
        format!(r#"DELETE FROM "SavedEpisodes" WHERE userid = $1 AND episodeid IN ({episodes})"#),
        format!(r#"DELETE FROM "EpisodeQueue" WHERE userid = $1 AND is_youtube = FALSE AND episodeid IN ({episodes})"#),
    ]
}

pub(super) fn user_episode_rows_mysql(episodes: &str) -> Vec<String> {
    vec![
        format!("DELETE FROM PlaylistContents WHERE EpisodeID IN ({episodes}) AND PlaylistID IN (SELECT PlaylistID FROM Playlists WHERE UserID = ?)"),
        format!("DELETE FROM CollectionEpisodes WHERE EpisodeID IN ({episodes}) AND CollectionID IN (SELECT CollectionID FROM Collections WHERE UserID = ?)"),
        format!("DELETE FROM EpisodeAdSkipReview WHERE UserID = ? AND SegmentID IN (SELECT SegmentID FROM EpisodeSkipSegments WHERE EpisodeID IN ({episodes}))"),
        format!("DELETE FROM SharedEpisodes WHERE SharedBy = ? AND EpisodeID IN ({episodes})"),
        format!("DELETE FROM UserEpisodeHistory WHERE UserID = ? AND EpisodeID IN ({episodes})"),
        format!("DELETE FROM UserEpisodeState WHERE UserID = ? AND EpisodeID IN ({episodes})"),
        format!("DELETE FROM DownloadedEpisodes WHERE UserID = ? AND EpisodeID IN ({episodes})"),
        format!("DELETE FROM SavedEpisodes WHERE UserID = ? AND EpisodeID IN ({episodes})"),
        format!("DELETE FROM EpisodeQueue WHERE UserID = ? AND is_youtube = 0 AND EpisodeID IN ({episodes})"),
    ]
}

// After the subscription is gone: with nobody else subscribed, the episodes (anything still
// pointing at them first) and the feed go too.
dialect_fn!([remove_unsubscribed_episodes_pg, remove_unsubscribed_episodes_lite]
    pub(super) async fn(conn: &mut q::Conn, feed_id: Option<i32>, episodes: &str) -> AppResult<()> {
    if let Some(f) = feed_id {
        q::query(r#"SELECT feedid FROM "Feeds" WHERE feedid = $1 FOR UPDATE"#).bind(f).fetch_optional(&mut *conn).await?;
        let left: i64 = q::query_scalar(r#"SELECT COUNT(*) FROM "Podcasts" WHERE feedid = $1"#).bind(f).fetch_one(&mut *conn).await?;
        if left > 0 {
            return Ok(());
        }
    }
    for table in ["PlaylistContents", "UserEpisodeHistory", "DownloadedEpisodes", "SavedEpisodes"] {
        q::query(sqlx::AssertSqlSafe(format!(r#"DELETE FROM "{table}" WHERE episodeid IN ({episodes})"#))).execute(&mut *conn).await?;
    }
    q::query(sqlx::AssertSqlSafe(format!(r#"DELETE FROM "EpisodeQueue" WHERE is_youtube = FALSE AND episodeid IN ({episodes})"#))).execute(&mut *conn).await?;
    q::query(sqlx::AssertSqlSafe(format!(r#"DELETE FROM "Episodes" WHERE episodeid IN ({episodes})"#))).execute(&mut *conn).await?;
    if let Some(f) = feed_id {
        q::query(r#"DELETE FROM "Feeds" WHERE feedid = $1"#).bind(f).execute(&mut *conn).await?;
    }
    Ok(())
});

pub(super) async fn remove_unsubscribed_episodes_mysql(conn: &mut sqlx::MySqlConnection, feed_id: Option<i32>, episodes: &str) -> AppResult<()> {
    if let Some(f) = feed_id {
        sqlx::query("SELECT FeedID FROM Feeds WHERE FeedID = ? FOR UPDATE").bind(f).fetch_optional(&mut *conn).await?;
        let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Podcasts WHERE FeedID = ?").bind(f).fetch_one(&mut *conn).await?;
        if left > 0 {
            return Ok(());
        }
    }
    // MySQL can't delete from a table its subquery reads, so the ids are fetched first.
    let ids: Vec<i32> = sqlx::query_scalar(sqlx::AssertSqlSafe(episodes.to_string())).fetch_all(&mut *conn).await?;
    for chunk in ids.chunks(1000) {
        let list = chunk.iter().map(i32::to_string).collect::<Vec<_>>().join(", ");
        for table in ["PlaylistContents", "UserEpisodeHistory", "DownloadedEpisodes", "SavedEpisodes"] {
            sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {table} WHERE EpisodeID IN ({list})"))).execute(&mut *conn).await?;
        }
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM EpisodeQueue WHERE is_youtube = 0 AND EpisodeID IN ({list})"))).execute(&mut *conn).await?;
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM Episodes WHERE EpisodeID IN ({list})"))).execute(&mut *conn).await?;
    }
    if let Some(f) = feed_id {
        sqlx::query("DELETE FROM Feeds WHERE FeedID = ?").bind(f).execute(&mut *conn).await?;
    }
    Ok(())
}

/// Drop blank categories and repeats of the same name (ignoring case and surrounding space),
/// keeping the first by key, so a podcast never shows an empty or duplicate category.
pub(crate) fn clean_categories(
    categories: std::collections::HashMap<String, String>,
) -> std::collections::HashMap<String, String> {
    let mut entries: Vec<(String, String)> = categories.into_iter().collect();
    // Numeric keys (Podcast Index ids, comma-split positions) in numeric order, then the rest.
    entries.sort_by(|(a, _), (b, _)| match (a.parse::<i64>(), b.parse::<i64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => std::cmp::Ordering::Less,
        (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
        _ => a.cmp(b),
    });
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter_map(|(key, name)| {
            let name = name.trim().to_string();
            (!name.is_empty() && seen.insert(name.to_lowercase())).then_some((key, name))
        })
        .collect()
}

// A new subscription starts with auto-queue on when its user asked for that by default (#648).
// Runs in the subscription's transaction, right after the podcast row is inserted.
dialect_fn!([apply_auto_queue_default_pg, apply_auto_queue_default_lite]
    async fn(conn: &mut q::Conn, podcast_id: i32, user_id: i32) -> AppResult<()> {
    q::query(
        r#"UPDATE "Podcasts" SET autoqueue = TRUE
           WHERE podcastid = $1
             AND EXISTS (SELECT 1 FROM "UserSettings" WHERE userid = $2 AND autoqueuenewpodcasts = TRUE)"#,
    )
    .bind(podcast_id)
    .bind(user_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
});

async fn apply_auto_queue_default_mysql(conn: &mut sqlx::MySqlConnection, podcast_id: i32, user_id: i32) -> AppResult<()> {
    sqlx::query(
        "UPDATE Podcasts SET AutoQueue = TRUE
         WHERE PodcastID = ?
           AND EXISTS (SELECT 1 FROM UserSettings WHERE UserID = ? AND AutoQueueNewPodcasts = TRUE)",
    )
    .bind(podcast_id)
    .bind(user_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

impl DatabasePool {
    // Add podcast - matches Python add_podcast function exactly
    pub async fn add_podcast(
        &self,
        podcast_values: &crate::handlers::podcasts::PodcastValues,
        podcast_index_id: i64,
        username: Option<&str>,
        password: Option<&str>,
    ) -> AppResult<(i32, Option<i32>)> {
        self.add_podcast_with_feed(podcast_values, podcast_index_id, username, password, None, crate::services::sync::SyncOrigin::Pinepods).await
    }

    /// `add_podcast`, importing episodes from `feed_body` when the caller already fetched the
    /// feed (e.g. for `get_podcast_values_with_body`), so it isn't downloaded twice.
    pub async fn add_podcast_with_feed(
        &self,
        podcast_values: &crate::handlers::podcasts::PodcastValues,
        podcast_index_id: i64,
        username: Option<&str>,
        password: Option<&str>,
        feed_body: Option<&str>,
        origin: crate::services::sync::SyncOrigin,
    ) -> AppResult<(i32, Option<i32>)> {
        let log = crate::services::sync::events::should_log(self, podcast_values.user_id, origin).await
            && crate::services::sync::shareable_feed(&podcast_values.pod_feed_url, username, password, false);
        dialect_match!(self, {
            pg(pool) => {
                // Check if podcast already exists
                let existing = q::query(r#"SELECT podcastid, podcastname, feedurl FROM "Podcasts" WHERE feedurl = $1 AND userid = $2"#)
                    .bind(&podcast_values.pod_feed_url)
                    .bind(podcast_values.user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = existing {
                    let podcast_id: i32 = row.try_get("podcastid")?;
                    // Check if there are episodes
                    let episode_count = q::query(r#"SELECT COUNT(*) as count FROM "Episodes" WHERE feedid = (SELECT feedid FROM "Podcasts" WHERE podcastid = $1)"#)
                        .bind(podcast_id)
                        .fetch_one(pool)
                        .await?;
                    
                    let count: i64 = episode_count.try_get("count")?;
                    if count == 0 {
                        // No episodes, add them
                        let first_episode_id = self.add_episodes(podcast_id, &podcast_values.pod_feed_url, 
                                                                  &podcast_values.pod_artwork, false, 
                                                                  username, password, feed_body).await?;
                        return Ok((podcast_id, first_episode_id));
                    } else {
                        return Ok((podcast_id, None));
                    }
                }
                
                // Convert categories to string
                let category_list = serde_json::to_string(&clean_categories(podcast_values.categories.clone()))?;
                
                // Insert new podcast
                // The subscription and its sync event commit together.
                let mut tx = q::begin(pool).await?;
                let row = q::query(
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
                .fetch_one(&mut *tx)
                .await?;
                
                let podcast_id: i32 = row.try_get("podcastid")?;
                q::pick(apply_auto_queue_default_pg, apply_auto_queue_default_lite)(&mut tx, podcast_id, podcast_values.user_id).await?;
                
                // Update UserStats table
                q::query(r#"UPDATE "UserStats" SET podcastsadded = podcastsadded + 1 WHERE userid = $1"#)
                    .bind(podcast_values.user_id)
                    .execute(&mut *tx)
                    .await?;
                if log {
                    q::pick(crate::services::sync::events::subscription_pg, crate::services::sync::events::subscription_lite)(self, &mut tx, podcast_values.user_id, &podcast_values.pod_feed_url, "add").await;
                }
                tx.commit().await?;
                self.link_feed_or_warn(podcast_id).await;
                
                // Add episodes
                let first_episode_id = self.add_episodes(podcast_id, &podcast_values.pod_feed_url, 
                                                          &podcast_values.pod_artwork, false, 
                                                          username, password, feed_body).await?;
                
                // Count episodes for logging
                let episode_count: i64 = q::query_scalar(r#"SELECT COUNT(*) FROM "Episodes" WHERE feedid = (SELECT feedid FROM "Podcasts" WHERE podcastid = $1)"#)
                    .bind(podcast_id)
                    .fetch_one(pool)
                    .await?;
                
                debug!("✅ Added podcast '{}' for user {} with {} episodes", 
                    podcast_values.pod_title, podcast_values.user_id, episode_count);
                
                Ok((podcast_id, first_episode_id))
            },
            mysql(pool) => {
                // Check if podcast already exists
                let existing = sqlx::query("SELECT PodcastID, PodcastName, FeedURL FROM Podcasts WHERE FeedURL = ? AND UserID = ?")
                    .bind(&podcast_values.pod_feed_url)
                    .bind(podcast_values.user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = existing {
                    let podcast_id: i32 = row.try_get("PodcastID")?;
                    // Check if there are episodes
                    let episode_count = sqlx::query("SELECT COUNT(*) as count FROM Episodes WHERE FeedID = (SELECT FeedID FROM Podcasts WHERE PodcastID = ?)")
                        .bind(podcast_id)
                        .fetch_one(pool)
                        .await?;
                    
                    let count: i64 = episode_count.try_get("count")?;
                    if count == 0 {
                        // No episodes, add them
                        let first_episode_id = self.add_episodes(podcast_id, &podcast_values.pod_feed_url, 
                                                                  &podcast_values.pod_artwork, false, 
                                                                  username, password, feed_body).await?;
                        return Ok((podcast_id, first_episode_id));
                    } else {
                        return Ok((podcast_id, None));
                    }
                }
                
                // Convert categories to string
                let category_list = serde_json::to_string(&clean_categories(podcast_values.categories.clone()))?;
                
                // Insert new podcast
                // The subscription and its sync event commit together.
                let mut tx = crate::services::sync::events::begin_mysql(pool, podcast_values.user_id, log).await?;
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
                .execute(&mut *tx)
                .await?;
                
                let podcast_id = result.last_insert_id() as i32;
                apply_auto_queue_default_mysql(&mut tx, podcast_id, podcast_values.user_id).await?;
                
                // Update UserStats table
                sqlx::query("UPDATE UserStats SET PodcastsAdded = PodcastsAdded + 1 WHERE UserID = ?")
                    .bind(podcast_values.user_id)
                    .execute(&mut *tx)
                    .await?;
                if log {
                    crate::services::sync::events::subscription_mysql(self, &mut tx, podcast_values.user_id, &podcast_values.pod_feed_url, "add").await;
                }
                tx.commit().await?;
                self.link_feed_or_warn(podcast_id).await;
                
                // Add episodes
                let first_episode_id = self.add_episodes(podcast_id, &podcast_values.pod_feed_url, 
                                                          &podcast_values.pod_artwork, false, 
                                                          username, password, feed_body).await?;
                
                // Count episodes for logging
                let episode_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Episodes WHERE FeedID = (SELECT FeedID FROM Podcasts WHERE PodcastID = ?)")
                    .bind(podcast_id)
                    .fetch_one(pool)
                    .await?;
                
                debug!("✅ Added podcast '{}' for user {} with {} episodes", 
                    podcast_values.pod_title, podcast_values.user_id, episode_count);
                
                Ok((podcast_id, first_episode_id))
            },
        })
    }

    // Add podcast without episodes - for background episode processing
    pub async fn add_podcast_without_episodes(
        &self,
        podcast_values: &crate::handlers::podcasts::PodcastValues,
        podcast_index_id: i64,
        username: Option<&str>,
        password: Option<&str>,
        origin: crate::services::sync::SyncOrigin,
    ) -> AppResult<i32> {
        let log = crate::services::sync::events::should_log(self, podcast_values.user_id, origin).await
            && crate::services::sync::shareable_feed(&podcast_values.pod_feed_url, username, password, false);
        dialect_match!(self, {
            pg(pool) => {
                // Check if podcast already exists
                let existing = q::query(r#"SELECT podcastid, podcastname, feedurl FROM "Podcasts" WHERE feedurl = $1 AND userid = $2"#)
                    .bind(&podcast_values.pod_feed_url)
                    .bind(podcast_values.user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = existing {
                    let podcast_id: i32 = row.try_get("podcastid")?;
                    return Ok(podcast_id);
                }
                
                // Convert categories to string
                let category_list = serde_json::to_string(&clean_categories(podcast_values.categories.clone()))?;
                
                // Insert new podcast without episodes
                // The subscription and its sync event commit together.
                let mut tx = q::begin(pool).await?;
                let row = q::query(
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
                .fetch_one(&mut *tx)
                .await?;
                
                let podcast_id: i32 = row.try_get("podcastid")?;
                q::pick(apply_auto_queue_default_pg, apply_auto_queue_default_lite)(&mut tx, podcast_id, podcast_values.user_id).await?;
                
                // Update UserStats table
                q::query(r#"UPDATE "UserStats" SET podcastsadded = podcastsadded + 1 WHERE userid = $1"#)
                    .bind(podcast_values.user_id)
                    .execute(&mut *tx)
                    .await?;
                if log {
                    q::pick(crate::services::sync::events::subscription_pg, crate::services::sync::events::subscription_lite)(self, &mut tx, podcast_values.user_id, &podcast_values.pod_feed_url, "add").await;
                }
                tx.commit().await?;
                self.link_feed_or_warn(podcast_id).await;
                
                debug!("✅ Added podcast '{}' for user {} (episodes will be processed in background)", 
                    podcast_values.pod_title, podcast_values.user_id);
                
                Ok(podcast_id)
            },
            mysql(pool) => {
                // Check if podcast already exists
                let existing = sqlx::query("SELECT PodcastID, PodcastName, FeedURL FROM Podcasts WHERE FeedURL = ? AND UserID = ?")
                    .bind(&podcast_values.pod_feed_url)
                    .bind(podcast_values.user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = existing {
                    let podcast_id: i32 = row.try_get("PodcastID")?;
                    return Ok(podcast_id);
                }
                
                // Convert categories to string
                let category_list = serde_json::to_string(&clean_categories(podcast_values.categories.clone()))?;
                
                // Insert new podcast without episodes
                // The subscription and its sync event commit together.
                let mut tx = crate::services::sync::events::begin_mysql(pool, podcast_values.user_id, log).await?;
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
                .bind(podcast_values.pod_explicit)
                .bind(podcast_values.user_id)
                .bind(30) // Default feed cutoff days
                .bind(username)
                .bind(password)
                .bind(podcast_index_id)
                .execute(&mut *tx)
                .await?;
                
                let podcast_id = result.last_insert_id() as i32;
                apply_auto_queue_default_mysql(&mut tx, podcast_id, podcast_values.user_id).await?;
                
                // Update UserStats table
                sqlx::query("UPDATE UserStats SET PodcastsAdded = PodcastsAdded + 1 WHERE UserID = ?")
                    .bind(podcast_values.user_id)
                    .execute(&mut *tx)
                    .await?;
                if log {
                    crate::services::sync::events::subscription_mysql(self, &mut tx, podcast_values.user_id, &podcast_values.pod_feed_url, "add").await;
                }
                tx.commit().await?;
                self.link_feed_or_warn(podcast_id).await;
                
                debug!("✅ Added podcast '{}' for user {} (episodes will be processed in background)", 
                    podcast_values.pod_title, podcast_values.user_id);
                
                Ok(podcast_id)
            },
        })
    }

    // Remove podcast - matches Python remove_podcast function
    pub async fn remove_podcast(
        &self,
        podcast_name: &str,
        podcast_url: &str,
        user_id: i32,
        origin: crate::services::sync::SyncOrigin,
    ) -> AppResult<()> {
        let podcast_id: Option<i32> = dialect_match!(self, {
            pg(pool) => q::query_scalar(
                r#"SELECT podcastid FROM "Podcasts" WHERE podcastname = $1 AND feedurl = $2 AND userid = $3"#
            )
            .bind(podcast_name)
            .bind(podcast_url)
            .bind(user_id)
            .fetch_optional(pool)
            .await?,
            mysql(pool) => sqlx::query_scalar(
                "SELECT PodcastID FROM Podcasts WHERE PodcastName = ? AND FeedURL = ? AND UserID = ?"
            )
            .bind(podcast_name)
            .bind(podcast_url)
            .bind(user_id)
            .fetch_optional(pool)
            .await?,
        });
        if let Some(podcast_id) = podcast_id {
            self.remove_podcast_cascade(podcast_id, user_id, origin).await?;
        }
        Ok(())
    }

    // Remove podcast by ID - matches Python remove_podcast_id function
    pub async fn remove_podcast_id(&self, podcast_id: i32, user_id: i32, origin: crate::services::sync::SyncOrigin) -> AppResult<()> {
        if podcast_id == 0 {
            return Err(AppError::bad_request("Invalid podcast ID"));
        }
        self.remove_podcast_cascade(podcast_id, user_id, origin).await?;
        Ok(())
    }

    /// Unsubscribe: delete a podcast and everything that depends on it in one transaction, and
    /// log the `remove` for sync. Every unsubscribe path (by id, by name, by URL, sync apply)
    /// goes through here. Returns false if the podcast doesn't exist. Retried when it collides
    /// with a concurrent write to the same rows (e.g. a queued import of that feed).
    pub(super) async fn remove_podcast_cascade(&self, podcast_id: i32, user_id: i32, origin: crate::services::sync::SyncOrigin) -> AppResult<bool> {
        let mut attempt = 0;
        let files = loop {
            match self.remove_podcast_cascade_once(podcast_id, user_id, origin).await {
                Err(e) if attempt < 3 && is_write_conflict(&e) => {
                    attempt += 1;
                    debug!("Removing podcast {} hit a write conflict, retrying ({}/3)", podcast_id, attempt);
                    tokio::time::sleep(Duration::from_millis(20 * attempt as u64)).await;
                }
                other => break other?,
            }
        };
        let Some(files) = files else { return Ok(false) };
        // The download rows are gone with the podcast, so their files go too.
        for path in files {
            if tokio::fs::metadata(&path).await.is_ok() {
                if let Err(e) = tokio::fs::remove_file(&path).await {
                    warn!("Could not remove downloaded file {} of removed podcast {}: {}", path, podcast_id, e);
                }
            }
        }
        Ok(true)
    }

    // Returns the user's downloaded files of the podcast, or None if it doesn't exist.
    pub(super) async fn remove_podcast_cascade_once(&self, podcast_id: i32, user_id: i32, origin: crate::services::sync::SyncOrigin) -> AppResult<Option<Vec<String>>> {
        use crate::services::sync::{events, shareable_feed};
        let log = events::should_log(self, user_id, origin).await;
        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                // Locking the row first makes a concurrent import of this feed wait (its episode
                // inserts check the row), instead of adding episodes between the deletes below.
                let podcast = q::query(
                    r#"SELECT feedurl, username, password, COALESCE(isyoutubechannel, FALSE) AS yt, feedid FROM "Podcasts" WHERE podcastid = $1 FOR UPDATE"#
                )
                .bind(podcast_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some(podcast) = podcast else { return Ok(None) };
                let feed_url: String = podcast.try_get::<Option<String>, _>("feedurl")?.unwrap_or_default();
                let shareable = shareable_feed(
                    &feed_url,
                    podcast.try_get::<Option<String>, _>("username")?.as_deref(),
                    podcast.try_get::<Option<String>, _>("password")?.as_deref(),
                    podcast.try_get("yt")?,
                );

                let feed_id: Option<i32> = podcast.try_get("feedid")?;
                // The feed's episodes are shared: only this user's rows for them go, and the
                // episodes themselves only with the last subscription.
                let episodes = feed_episodes_sql(feed_id, podcast_id, true);
                // Local podcasts point at the user's own media, which stays.
                let files: Vec<Option<String>> = if feed_url.starts_with("local://") {
                    Vec::new()
                } else {
                    q::query_scalar(sqlx::AssertSqlSafe(format!(
                        r#"SELECT downloadedlocation FROM "DownloadedEpisodes" WHERE userid = $1 AND episodeid IN ({episodes})
                           UNION ALL
                           SELECT downloadedlocation FROM "DownloadedVideos" WHERE userid = $1 AND videoid IN (SELECT videoid FROM "YouTubeVideos" WHERE podcastid = {podcast_id})"#
                    )))
                    .bind(user_id)
                    .fetch_all(&mut *tx)
                    .await?
                };
                for sql in user_episode_rows_pg(&episodes) {
                    q::query(sqlx::AssertSqlSafe(sql)).bind(user_id).execute(&mut *tx).await?;
                }

                // 6a. YouTubeVideos dependents (NO ACTION FKs) before the videos themselves
                q::query(r#"DELETE FROM "UserVideoHistory" WHERE videoid IN (SELECT videoid FROM "YouTubeVideos" WHERE podcastid = $1)"#)
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;
                q::query(r#"DELETE FROM "SavedVideos" WHERE videoid IN (SELECT videoid FROM "YouTubeVideos" WHERE podcastid = $1)"#)
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;
                q::query(r#"DELETE FROM "DownloadedVideos" WHERE videoid IN (SELECT videoid FROM "YouTubeVideos" WHERE podcastid = $1)"#)
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;

                // 6a2. YouTubeVideos — references Podcasts (NO ACTION), so must go before the podcast
                q::query(r#"DELETE FROM "YouTubeVideos" WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;


                // 7. Finally delete the podcast itself
                q::query(r#"DELETE FROM "Podcasts" WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;
                q::pick(remove_unsubscribed_episodes_pg, remove_unsubscribed_episodes_lite)(&mut tx, feed_id, &episodes).await?;

                // Update user stats
                q::query(r#"UPDATE "UserStats" SET podcastsadded = podcastsadded - 1 WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;

                if log && shareable {
                    q::pick(events::subscription_pg, events::subscription_lite)(self, &mut tx, user_id, &feed_url, "remove").await;
                }
                tx.commit().await?;
                Ok(Some(files.into_iter().flatten().collect()))
            },
            mysql(pool) => {
                let mut tx = crate::services::sync::events::begin_mysql(pool, user_id, log).await?;
                let podcast = sqlx::query(
                    "SELECT FeedURL, Username, Password, IsYouTubeChannel AS yt, FeedID FROM Podcasts WHERE PodcastID = ? FOR UPDATE"
                )
                .bind(podcast_id)
                .fetch_optional(&mut *tx)
                .await?;
                let Some(podcast) = podcast else { return Ok(None) };
                let feed_url: String = podcast.try_get::<Option<String>, _>("FeedURL")?.unwrap_or_default();
                let shareable = shareable_feed(
                    &feed_url,
                    podcast.try_get::<Option<String>, _>("Username")?.as_deref(),
                    podcast.try_get::<Option<String>, _>("Password")?.as_deref(),
                    podcast.try_get::<Option<bool>, _>("yt")?.unwrap_or(false),
                );

                let feed_id: Option<i32> = podcast.try_get("FeedID")?;
                let episodes = feed_episodes_sql(feed_id, podcast_id, false);
                let files: Vec<Option<String>> = if feed_url.starts_with("local://") {
                    Vec::new()
                } else {
                    sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                        "SELECT DownloadedLocation FROM DownloadedEpisodes WHERE UserID = ? AND EpisodeID IN ({episodes})
                         UNION ALL
                         SELECT DownloadedLocation FROM DownloadedVideos WHERE UserID = ? AND VideoID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = {podcast_id})"
                    )))
                    .bind(user_id)
                    .bind(user_id)
                    .fetch_all(&mut *tx)
                    .await?
                };
                for sql in user_episode_rows_mysql(&episodes) {
                    sqlx::query(sqlx::AssertSqlSafe(sql)).bind(user_id).execute(&mut *tx).await?;
                }

                // 6a. YouTubeVideos dependents (NO ACTION FKs) before the videos themselves
                sqlx::query("DELETE FROM UserVideoHistory WHERE VideoID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = ?)")
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM SavedVideos WHERE VideoID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = ?)")
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM DownloadedVideos WHERE VideoID IN (SELECT VideoID FROM YouTubeVideos WHERE PodcastID = ?)")
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;

                // 6a2. YouTubeVideos — references Podcasts (NO ACTION), so must go before the podcast
                sqlx::query("DELETE FROM YouTubeVideos WHERE PodcastID = ?")
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;


                // 7. Finally delete the podcast itself
                sqlx::query("DELETE FROM Podcasts WHERE PodcastID = ?")
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;
                remove_unsubscribed_episodes_mysql(&mut tx, feed_id, &episodes).await?;

                // Update user stats
                sqlx::query("UPDATE UserStats SET PodcastsAdded = PodcastsAdded - 1 WHERE UserID = ?")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;

                if log && shareable {
                    events::subscription_mysql(self, &mut tx, user_id, &feed_url, "remove").await;
                }
                tx.commit().await?;
                Ok(Some(files.into_iter().flatten().collect()))
            },
        })
    }

    // Get user podcast count - for refresh progress tracking
    pub async fn get_user_podcast_count(&self, user_id: i32) -> AppResult<u32> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT COUNT(*) as count FROM "Podcasts" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get::<i64, _>("count")? as u32)
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT COUNT(*) as count FROM Podcasts WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get::<i64, _>("count")? as u32)
            },
        })
    }

    // Return podcasts basic - matches Python return_pods function
    pub async fn return_pods(&self, user_id: i32) -> AppResult<Vec<crate::models::PodcastResponse>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
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
                        COALESCE(podcastindexid, 0) as podcastindexid,
                        COALESCE(isfavorite, false) as isfavorite,
                        EXISTS(SELECT 1 FROM "Episodes" e WHERE e.feedid = "Podcasts".feedid AND e.is_video = true) as is_video,
                        TO_CHAR(COALESCE((SELECT MIN(e.episodepubdate) FROM "Episodes" e WHERE e.feedid = "Podcasts".feedid),
                                         (SELECT MIN(v.publishedat) FROM "YouTubeVideos" v WHERE v.podcastid = "Podcasts".podcastid)),
                                'YYYY-MM-DD"T"HH24:MI:SS') as oldest_episode_date,
                        TO_CHAR(COALESCE((SELECT MAX(e.episodepubdate) FROM "Episodes" e WHERE e.feedid = "Podcasts".feedid),
                                         (SELECT MAX(v.publishedat) FROM "YouTubeVideos" v WHERE v.podcastid = "Podcasts".podcastid)),
                                'YYYY-MM-DD"T"HH24:MI:SS') as newest_episode_date,
                        (SELECT COUNT(*) FROM "UserEpisodeHistory" h JOIN "Episodes" e ON e.episodeid = h.episodeid
                         WHERE h.userid = $1 AND e.feedid = "Podcasts".feedid) as play_count
                    FROM "Podcasts"
                    WHERE userid = $1 AND COALESCE(displaypodcast, TRUE) = TRUE
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
                        categories: {
                            let categories_str: String = row.try_get("categories")?;
                            self.parse_categories_json(&categories_str)
                        },
                        explicit: row.try_get("explicit")?,
                        podcastindexid: row.try_get::<i32, _>("podcastindexid").ok().map(|i| i as i64),
                        is_favorite: row.try_get("isfavorite").unwrap_or(false),
                        is_video: row.try_get("is_video").unwrap_or(false),
                        oldest_episode_date: row.try_get("oldest_episode_date")?,
                        newest_episode_date: row.try_get("newest_episode_date")?,
                        play_count: row.try_get("play_count")?,
                    });
                }
                Ok(podcasts)
            },
            mysql(pool) => {
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
                        COALESCE(PodcastIndexID, 0) as podcastindexid,
                        COALESCE(IsFavorite, false) as isfavorite,
                        EXISTS(SELECT 1 FROM Episodes e WHERE e.FeedID = Podcasts.FeedID AND e.IsVideo = 1) as is_video,
                        DATE_FORMAT(COALESCE((SELECT MIN(e.EpisodePubDate) FROM Episodes e WHERE e.FeedID = Podcasts.FeedID),
                                             (SELECT MIN(v.PublishedAt) FROM YouTubeVideos v WHERE v.PodcastID = Podcasts.PodcastID)),
                                    '%Y-%m-%dT%H:%i:%s') as oldest_episode_date,
                        DATE_FORMAT(COALESCE((SELECT MAX(e.EpisodePubDate) FROM Episodes e WHERE e.FeedID = Podcasts.FeedID),
                                             (SELECT MAX(v.PublishedAt) FROM YouTubeVideos v WHERE v.PodcastID = Podcasts.PodcastID)),
                                    '%Y-%m-%dT%H:%i:%s') as newest_episode_date,
                        (SELECT COUNT(*) FROM UserEpisodeHistory h JOIN Episodes e ON e.EpisodeID = h.EpisodeID
                         WHERE h.UserID = ? AND e.FeedID = Podcasts.FeedID) as play_count
                    FROM Podcasts
                    WHERE UserID = ? AND COALESCE(DisplayPodcast, 1) = 1
                    ORDER BY PodcastName"
                )
                .bind(user_id)
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
                        categories: {
                            let categories_str: String = row.try_get("categories")?;
                            self.parse_categories_json(&categories_str)
                        },
                        explicit: row.try_get("explicit")?,
                        podcastindexid: row.try_get::<i32, _>("podcastindexid").ok().map(|i| i as i64),
                        is_favorite: row.try_get("isfavorite").unwrap_or(false),
                        is_video: row.try_get("is_video").unwrap_or(false),
                        oldest_episode_date: row.try_get("oldest_episode_date")?,
                        newest_episode_date: row.try_get("newest_episode_date")?,
                        play_count: row.try_get("play_count")?,
                    });
                }
                Ok(podcasts)
            },
        })
    }

    // Return podcasts with extra stats - matches Python return_pods with analytics
    pub async fn return_pods_extra(&self, user_id: i32) -> AppResult<Vec<crate::models::PodcastExtraResponse>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
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
                        COUNT(DISTINCT ueh.episodeid)::INT as episodes_played,
                        TO_CHAR(MIN(e.episodepubdate), 'YYYY-MM-DD"T"HH24:MI:SS') as oldest_episode_date,
                        COALESCE(p.isyoutubechannel, false) as is_youtube,
                        COALESCE(BOOL_OR(e.is_video), false) as is_video,
                        COALESCE(p.isfavorite, false) as isfavorite
                    FROM "Podcasts" p
                    LEFT JOIN "Episodes" e ON p.feedid = e.feedid
                    LEFT JOIN "UserEpisodeHistory" ueh ON e.episodeid = ueh.episodeid AND ueh.userid = $1
                    WHERE p.userid = $1 AND COALESCE(p.displaypodcast, TRUE) = TRUE
                    GROUP BY p.podcastid, p.podcastname, p.artworkurl, p.description,
                             p.episodecount, p.websiteurl, p.feedurl, p.author,
                             p.categories, p.explicit, p.podcastindexid, p.isyoutubechannel, p.isfavorite
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
                        categories: {
                            let categories_str: String = row.try_get("categories")?;
                            self.parse_categories_json(&categories_str)
                        },
                        explicit: row.try_get("explicit")?,
                        podcastindexid: row.try_get::<i32, _>("podcastindexid").ok().map(|i| i as i64),
                        play_count: row.try_get("play_count")?,
                        episodes_played: row.try_get("episodes_played")?,
                        oldest_episode_date: row.try_get("oldest_episode_date").ok(),
                        is_youtube,
                        is_video: row.try_get("is_video").unwrap_or(false),
                        is_favorite: row.try_get("isfavorite").unwrap_or(false),
                    });
                }
                Ok(podcasts)
            },
            mysql(pool) => {
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
                        DATE_FORMAT(MIN(e.EpisodePubDate), '%Y-%m-%dT%H:%i:%s') as oldest_episode_date,
                        COALESCE(p.IsYouTubeChannel, false) as is_youtube,
                        COALESCE(MAX(e.IsVideo), 0) as is_video,
                        COALESCE(p.IsFavorite, false) as isfavorite
                    FROM Podcasts p
                    LEFT JOIN Episodes e ON p.FeedID = e.FeedID
                    LEFT JOIN UserEpisodeHistory ueh ON e.EpisodeID = ueh.EpisodeID AND ueh.UserID = ?
                    WHERE p.UserID = ? AND COALESCE(p.DisplayPodcast, 1) = 1
                    GROUP BY p.PodcastID, p.PodcastName, p.ArtworkURL, p.Description,
                             p.EpisodeCount, p.WebsiteURL, p.FeedURL, p.Author,
                             p.Categories, p.Explicit, p.PodcastIndexID, p.IsYouTubeChannel, p.IsFavorite
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
                        categories: {
                            let categories_str: String = row.try_get("categories")?;
                            self.parse_categories_json(&categories_str)
                        },
                        explicit: row.try_get("explicit")?,
                        podcastindexid: row.try_get::<i32, _>("podcastindexid").ok().map(|i| i as i64),
                        play_count: row.try_get("play_count")?,
                        episodes_played: row.try_get("episodes_played")?,
                        oldest_episode_date: row.try_get("oldest_episode_date").ok(),
                        is_youtube,
                        is_video: row.try_get("is_video").unwrap_or(false),
                        is_favorite: row.try_get("isfavorite").unwrap_or(false),
                    });
                }
                Ok(podcasts)
            },
        })
    }

    // Merge podcasts - set DisplayPodcast=FALSE for secondary podcasts and update primary with merged IDs
    pub async fn merge_podcasts(&self, primary_podcast_id: i32, secondary_podcast_ids: &[i32], user_id: i32) -> AppResult<()> {
        // Validate that all podcasts belong to the user
        for &podcast_id in std::iter::once(&primary_podcast_id).chain(secondary_podcast_ids.iter()) {
            let exists = self.verify_podcast_belongs_to_user(podcast_id, user_id).await?;
            if !exists {
                return Err(crate::error::AppError::forbidden("One or more podcasts do not belong to this user"));
            }
        }

        // Prevent circular merges - check if any secondary podcasts are already primary podcasts with merges
        for &secondary_id in secondary_podcast_ids {
            let existing_merges = self.get_merged_podcast_ids(secondary_id).await?;
            if !existing_merges.is_empty() {
                return Err(crate::error::AppError::bad_request("Cannot merge a podcast that is already a primary podcast with merged podcasts"));
            }
        }

        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                
                // Set DisplayPodcast=FALSE for secondary podcasts
                for &secondary_id in secondary_podcast_ids {
                    q::query(r#"UPDATE "Podcasts" SET displaypodcast = FALSE WHERE podcastid = $1"#)
                        .bind(secondary_id)
                        .execute(&mut *tx)
                        .await?;
                }
                
                // Get current merged IDs for primary podcast
                let current_merged_ids = self.get_merged_podcast_ids(primary_podcast_id).await?;
                let mut all_merged_ids = current_merged_ids;
                all_merged_ids.extend_from_slice(secondary_podcast_ids);
                
                // Update primary podcast with merged IDs
                let merged_json = serde_json::to_string(&all_merged_ids)?;
                q::query(r#"UPDATE "Podcasts" SET mergedpodcastids = $1 WHERE podcastid = $2"#)
                    .bind(merged_json)
                    .bind(primary_podcast_id)
                    .execute(&mut *tx)
                    .await?;
                
                tx.commit().await?;
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;
                
                // Set DisplayPodcast=0 for secondary podcasts
                for &secondary_id in secondary_podcast_ids {
                    sqlx::query("UPDATE Podcasts SET DisplayPodcast = 0 WHERE PodcastID = ?")
                        .bind(secondary_id)
                        .execute(&mut *tx)
                        .await?;
                }
                
                // Get current merged IDs for primary podcast
                let current_merged_ids = self.get_merged_podcast_ids(primary_podcast_id).await?;
                let mut all_merged_ids = current_merged_ids;
                all_merged_ids.extend_from_slice(secondary_podcast_ids);
                
                // Update primary podcast with merged IDs
                let merged_json = serde_json::to_string(&all_merged_ids)?;
                sqlx::query("UPDATE Podcasts SET MergedPodcastIDs = ? WHERE PodcastID = ?")
                    .bind(merged_json)
                    .bind(primary_podcast_id)
                    .execute(&mut *tx)
                    .await?;
                
                tx.commit().await?;
            },
        });
        Ok(())
    }

    // Unmerge a specific podcast from a primary podcast
    pub async fn unmerge_podcast(&self, primary_podcast_id: i32, target_podcast_id: i32, user_id: i32) -> AppResult<()> {
        // Validate ownership
        let primary_exists = self.verify_podcast_belongs_to_user(primary_podcast_id, user_id).await?;
        let target_exists = self.verify_podcast_belongs_to_user(target_podcast_id, user_id).await?;
        
        if !primary_exists || !target_exists {
            return Err(crate::error::AppError::forbidden("One or more podcasts do not belong to this user"));
        }

        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                
                // Get current merged IDs and remove the target
                let mut merged_ids = self.get_merged_podcast_ids(primary_podcast_id).await?;
                merged_ids.retain(|&id| id != target_podcast_id);
                
                // Update primary podcast
                let merged_json = if merged_ids.is_empty() {
                    None
                } else {
                    Some(serde_json::to_string(&merged_ids)?)
                };
                
                q::query(r#"UPDATE "Podcasts" SET mergedpodcastids = $1 WHERE podcastid = $2"#)
                    .bind(merged_json)
                    .bind(primary_podcast_id)
                    .execute(&mut *tx)
                    .await?;
                
                // Set DisplayPodcast=TRUE for the unmerged podcast
                q::query(r#"UPDATE "Podcasts" SET displaypodcast = TRUE WHERE podcastid = $1"#)
                    .bind(target_podcast_id)
                    .execute(&mut *tx)
                    .await?;
                
                tx.commit().await?;
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;
                
                // Get current merged IDs and remove the target
                let mut merged_ids = self.get_merged_podcast_ids(primary_podcast_id).await?;
                merged_ids.retain(|&id| id != target_podcast_id);
                
                // Update primary podcast
                let merged_json = if merged_ids.is_empty() {
                    None
                } else {
                    Some(serde_json::to_string(&merged_ids)?)
                };
                
                sqlx::query("UPDATE Podcasts SET MergedPodcastIDs = ? WHERE PodcastID = ?")
                    .bind(merged_json)
                    .bind(primary_podcast_id)
                    .execute(&mut *tx)
                    .await?;
                
                // Set DisplayPodcast=1 for the unmerged podcast
                sqlx::query("UPDATE Podcasts SET DisplayPodcast = 1 WHERE PodcastID = ?")
                    .bind(target_podcast_id)
                    .execute(&mut *tx)
                    .await?;
                
                tx.commit().await?;
            },
        });
        Ok(())
    }

    // Get merged podcast IDs for a primary podcast
    /// The user a podcast row belongs to, or `None` if there's no such podcast.
    pub async fn podcast_owner(&self, podcast_id: i32) -> AppResult<Option<i32>> {
        Ok(dialect_match!(self, {
            pg(pool) => q::query_scalar(r#"SELECT userid FROM "Podcasts" WHERE podcastid = $1"#)
                .bind(podcast_id)
                .fetch_optional(pool)
                .await?,
            mysql(pool) => sqlx::query_scalar("SELECT UserID FROM Podcasts WHERE PodcastID = ?")
                .bind(podcast_id)
                .fetch_optional(pool)
                .await?,
        }))
    }

    pub async fn get_merged_podcast_ids(&self, podcast_id: i32) -> AppResult<Vec<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT mergedpodcastids FROM "Podcasts" WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    if let Some(merged_json) = row.try_get::<Option<String>, _>("mergedpodcastids")? {
                        let merged_ids: Vec<i32> = serde_json::from_str(&merged_json).unwrap_or_default();
                        Ok(merged_ids)
                    } else {
                        Ok(Vec::new())
                    }
                } else {
                    Ok(Vec::new())
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT MergedPodcastIDs FROM Podcasts WHERE PodcastID = ?")
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    if let Some(merged_json) = row.try_get::<Option<String>, _>("MergedPodcastIDs")? {
                        let merged_ids: Vec<i32> = serde_json::from_str(&merged_json).unwrap_or_default();
                        Ok(merged_ids)
                    } else {
                        Ok(Vec::new())
                    }
                } else {
                    Ok(Vec::new())
                }
            },
        })
    }

    // Helper function to verify a podcast belongs to a user
    pub async fn verify_podcast_belongs_to_user(&self, podcast_id: i32, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let count = q::query(r#"SELECT COUNT(*) as count FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(count.try_get::<i64, _>("count")? > 0)
            },
            mysql(pool) => {
                let count = sqlx::query("SELECT COUNT(*) as count FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                Ok(count.try_get::<i64, _>("count")? > 0)
            },
        })
    }

    // Check if podcast exists - matches Python check_podcast function
    pub async fn check_podcast(&self, user_id: i32, podcast_name: &str, podcast_url: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT podcastid FROM "Podcasts" 
                       WHERE userid = $1 AND podcastname = $2 AND feedurl = $3"#
                )
                .bind(user_id)
                .bind(podcast_name)
                .bind(podcast_url)
                .fetch_optional(pool)
                .await?;
                
                Ok(row.is_some())
            },
            mysql(pool) => {
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
            },
        })
    }

    // Get podcast ID by user, feed URL, and title
    pub async fn get_podcast_id(&self, user_id: i32, podcast_feed: &str, podcast_title: &str) -> AppResult<Option<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT podcastid FROM "Podcasts" WHERE feedurl = $1 AND podcastname = $2 AND userid = $3"#)
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
            },
            mysql(pool) => {
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
            },
        })
    }

    // Get podcast index ID - matches Python get_podcast_index_id function
    pub async fn get_podcast_index_id(&self, podcast_id: i32) -> AppResult<Option<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT podcastindexid FROM "Podcasts" WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("podcastindexid").ok())
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT PodcastIndexID FROM Podcasts WHERE PodcastID = ?")
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("PodcastIndexID").ok())
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get podcast values from RSS feed - matches Python get_podcast_values function exactly
    pub async fn get_podcast_values(&self, feed_url: &str, user_id: i32, username: Option<&str>, password: Option<&str>) -> AppResult<std::collections::HashMap<String, String>> {
        Ok(self.get_podcast_values_with_body(feed_url, user_id, username, password).await?.0)
    }

    /// `get_podcast_values`, plus the feed body it fetched, so adding the podcast can import its
    /// episodes from the same download instead of fetching the feed a second time.
    pub async fn get_podcast_values_with_body(
        &self,
        feed_url: &str,
        user_id: i32,
        username: Option<&str>,
        password: Option<&str>,
    ) -> AppResult<(std::collections::HashMap<String, String>, String)> {
        debug!("Fetching podcast values from feed URL: {}", feed_url);

        // SSRF guard: feed_url is attacker-controlled (add_podcast request body).
        // Reject loopback/private/link-local/reserved destinations before any
        // server-side request; redirects are re-validated by the shared clients.
        if let Err(reason) = crate::services::url_guard::ensure_safe_public_url_async(feed_url).await {
            warn!("Refusing to fetch feed from unsafe URL {}: {}", feed_url, reason);
            return Err(AppError::bad_request(format!("Refused to fetch feed URL: {}", reason)));
        }

        // PinePods UA first — many hosts serve full feeds to podcast apps but truncated
        // versions to browser UAs.
        let response = Self::feed_get(crate::services::http::feed_client(), feed_url, username, password)
            .send()
            .await
            .map_err(|e| {
                warn!("HTTP request failed for {}: {}", feed_url, e);
                AppError::Http(e)
            })?;
        debug!("Received response with status: {}", response.status());

        let content = if response.status().is_success() {
            response.text().await?
        } else if matches!(response.status().as_u16(), 403 | 406) {
            // The host may be blocking podcast client User-Agents; retry with a browser one.
            debug!("Got {}, retrying with browser User-Agent", response.status());
            let retry = Self::feed_get(crate::services::http::browser_feed_client(), feed_url, username, password)
                .send()
                .await
                .map_err(|e| {
                    warn!("Browser UA request failed for {}: {}", feed_url, e);
                    AppError::Http(e)
                })?;
            if !retry.status().is_success() {
                warn!("Browser UA request also failed with status: {}", retry.status());
                return Err(AppError::bad_request(&format!("Feed request failed: HTTP {}", response.status())));
            }
            retry.text().await?
        } else {
            return Err(AppError::bad_request(&format!("Feed request failed: HTTP {}", response.status())));
        };

        let values = self.parse_feed_content_to_values(&content, feed_url, user_id).await?;
        Ok((values, content))
    }

    // Add podcast from RSS values - wrapper function for custom podcast addition
    /// `feed_body`: the feed as fetched for `podcast_values` (`get_podcast_values_with_body`), so
    /// episodes are imported without downloading it again.
    pub async fn add_podcast_from_values(&self, podcast_values: &std::collections::HashMap<String, String>, user_id: i32, _feed_cutoff: i32, username: Option<&str>, password: Option<&str>, feed_body: Option<&str>, origin: crate::services::sync::SyncOrigin) -> AppResult<(i32, Option<i32>)> {
        // Convert HashMap values to PodcastValues struct
        let podcast_data = crate::handlers::podcasts::PodcastValues {
            user_id,
            pod_title: podcast_values.get("podcastname").unwrap_or(&"".to_string()).clone(),
            pod_artwork: podcast_values.get("artworkurl").unwrap_or(&"".to_string()).clone(),
            pod_author: podcast_values.get("author").unwrap_or(&"".to_string()).clone(),
            categories: std::collections::HashMap::new(), // Parse from string if needed
            pod_description: podcast_values.get("description").unwrap_or(&"".to_string()).clone(),
            pod_episode_count: podcast_values.get("episodecount").unwrap_or(&"0".to_string()).parse().unwrap_or(0),
            pod_feed_url: podcast_values.get("feedurl").unwrap_or(&"".to_string()).clone(),
            pod_website: podcast_values.get("websiteurl").unwrap_or(&"".to_string()).clone(),
            pod_explicit: podcast_values.get("explicit").unwrap_or(&"False".to_string()) == "True",
        };

        let podcast_index_id = podcast_values.get("podcastindexid")
            .unwrap_or(&"0".to_string())
            .parse::<i64>()
            .unwrap_or(0);

        self.add_podcast_with_feed(&podcast_data, podcast_index_id, username, password, feed_body, origin).await
    }

    // Update episode count for podcast - matches Python update_episode_count function exactly
    pub async fn update_episode_count(&self, podcast_id: i32) -> AppResult<()> {
        debug!("Updating episode count for podcast {}", podcast_id);
        
        // Count episodes and YouTube videos
        let (episode_count, youtube_count) = dialect_match!(self, {
            pg(pool) => {
                let episode_row = q::query(r#"SELECT COUNT(*) as count FROM "Episodes" WHERE feedid = (SELECT feedid FROM "Podcasts" WHERE podcastid = $1)"#)
                    .bind(podcast_id)
                    .fetch_one(pool)
                    .await?;
                let episode_count: i64 = episode_row.try_get("count")?;
                
                let youtube_row = q::query(r#"SELECT COUNT(*) as count FROM "YouTubeVideos" WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .fetch_one(pool)
                    .await?;
                let youtube_count: i64 = youtube_row.try_get("count")?;
                
                (episode_count, youtube_count)
            },
            mysql(pool) => {
                let episode_row = sqlx::query("SELECT COUNT(*) as count FROM Episodes WHERE FeedID = (SELECT FeedID FROM Podcasts WHERE PodcastID = ?)")
                    .bind(podcast_id)
                    .fetch_one(pool)
                    .await?;
                let episode_count: i64 = episode_row.try_get("count")?;
                
                let youtube_row = sqlx::query("SELECT COUNT(*) as count FROM YouTubeVideos WHERE PodcastID = ?")
                    .bind(podcast_id)
                    .fetch_one(pool)
                    .await?;
                let youtube_count: i64 = youtube_row.try_get("count")?;
                
                (episode_count, youtube_count)
            },
        });
        
        let total_count = episode_count + youtube_count;
        
        // Update podcast episode count
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET episodecount = $2 WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .bind(total_count as i32)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET EpisodeCount = ? WHERE PodcastID = ?")
                    .bind(total_count as i32)
                    .bind(podcast_id)
                    .execute(pool)
                    .await?;
            },
        });
        
        debug!("Updated episode count to {} ({} episodes + {} videos)", total_count, episode_count, youtube_count);
        Ok(())
    }

    // Add person podcast from values map - matches Python add_person_podcast function exactly  
    pub async fn add_person_podcast_from_values(&self, podcast_values: &std::collections::HashMap<String, String>, user_id: i32) -> AppResult<bool> {
        // Use the same key mapping as add_podcast_from_values
        let pod_title = podcast_values.get("podcastname").cloned().unwrap_or_default();
        let pod_feed_url = podcast_values.get("feedurl").cloned().unwrap_or_default();
        let pod_artwork = podcast_values.get("artworkurl").cloned().unwrap_or_default();
        let pod_description = podcast_values.get("description").cloned().unwrap_or_default();
        // First check if podcast already exists for user with a valid feed URL
        dialect_match!(self, {
            pg(pool) => {
                let existing = q::query(r#"SELECT podcastid FROM "Podcasts" WHERE feedurl = $1 AND userid = $2 AND feedurl != ''"#)
                    .bind(&pod_feed_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if existing.is_some() {
                    return Ok(true); // Already exists
                }
                
                // Insert new podcast
                q::query(r#"
                    INSERT INTO "Podcasts" (podcastname, feedurl, artworkurl, description, 
                                         userid, autodownload, isyoutubechannel, podcastindexid)
                    VALUES ($1, $2, $3, $4, $5, FALSE, FALSE, $6)
                "#)
                .bind(&pod_title)
                .bind(&pod_feed_url)
                .bind(&pod_artwork)
                .bind(&pod_description)
                .bind(user_id)
                .bind(0) // podcast_index_id placeholder
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                let existing = sqlx::query("SELECT PodcastID FROM Podcasts WHERE FeedURL = ? AND UserID = ? AND FeedURL != ''")
                    .bind(&pod_feed_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if existing.is_some() {
                    return Ok(true); // Already exists
                }
                
                // Insert new podcast
                sqlx::query("
                    INSERT INTO Podcasts (PodcastName, FeedURL, ArtworkURL, Description, 
                                        UserID, AutoDownload, IsYouTubeChannel, PodcastIndexID)
                    VALUES (?, ?, ?, ?, ?, 0, 0, ?)
                ")
                .bind(&pod_title)
                .bind(&pod_feed_url)
                .bind(&pod_artwork)
                .bind(&pod_description)
                .bind(user_id)
                .bind(0) // podcast_index_id placeholder
                .execute(pool)
                .await?;
            },
        });
        
        Ok(true)
    }

    // Get podcast ID by feed URL
    pub async fn get_podcast_id_by_feed_url(&self, user_id: i32, feed_url: &str) -> AppResult<Option<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"SELECT podcastid FROM "Podcasts" WHERE feedurl = $1 AND userid = $2"#)
                    .bind(feed_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(result.map(|row| row.try_get("podcastid")).transpose()?)
            },
            mysql(pool) => {
                let result = sqlx::query("SELECT PodcastID FROM Podcasts WHERE FeedURL = ? AND UserID = ?")
                    .bind(feed_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(result.map(|row| row.try_get("PodcastID")).transpose()?)
            },
        })
    }

    // Get podcast values for person podcasts - uses existing get_podcast_values function
    pub async fn get_podcast_values_for_person(&self, feed_url: &str) -> AppResult<std::collections::HashMap<String, String>> {
        self.get_podcast_values(feed_url, 1, None, None).await
    }

    // Get podcast details - matches Python get_podcast_details function exactly
    pub async fn get_podcast_details(&self, user_id: i32, podcast_id: i32) -> AppResult<serde_json::Value> {
        dialect_match!(self, {
            pg(pool) => {
                // First try to get podcast for specific user
                let mut podcast_row = q::query(r#"
                    SELECT podcastid, podcastname, feedurl, description, author, artworkurl, 
                           explicit, episodecount, categories, websiteurl, podcastindexid, isyoutubechannel,
                           userid, autodownload, startskip, endskip, username, password, notificationsenabled, feedcutoffdays,
                           playbackspeed, playbackspeedcustomized
                    FROM "Podcasts" 
                    WHERE podcastid = $1 AND userid = $2
                "#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                // Fallback to admin/public podcasts (userid = 1) if not found for user
                if podcast_row.is_none() {
                    podcast_row = q::query(r#"
                        SELECT podcastid, podcastname, feedurl, description, author, artworkurl, 
                               explicit, episodecount, categories, websiteurl, podcastindexid, isyoutubechannel,
                               userid, autodownload, startskip, endskip, username, password, notificationsenabled, feedcutoffdays,
                               playbackspeed, playbackspeedcustomized
                        FROM "Podcasts" 
                        WHERE podcastid = $1 AND userid = 1
                    "#)
                        .bind(podcast_id)
                        .fetch_optional(pool)
                        .await?;
                }

                let row = podcast_row.ok_or_else(|| AppError::not_found("Podcast not found"))?;

                // Get episode count (special handling for YouTube channels)
                let is_youtube: bool = row.try_get("isyoutubechannel").unwrap_or(false);
                let episode_count = if is_youtube {
                    // Get count from YouTubeVideos table for YouTube channels
                    let count_row = q::query(r#"SELECT COUNT(*) as count FROM "YouTubeVideos" WHERE podcastid = $1"#)
                        .bind(podcast_id)
                        .fetch_one(pool)
                        .await?;
                    count_row.try_get::<i64, _>("count")? as i32
                } else {
                    row.try_get("episodecount").unwrap_or(0)
                };

                // Get categories and parse to HashMap - matches Python version exactly
                let categories_str = row.try_get::<String, _>("categories").unwrap_or_else(|_| String::new());
                let categories = self.parse_categories_json(&categories_str);

                Ok(serde_json::json!({
                    "podcastid": row.try_get::<i32, _>("podcastid")?,
                    "podcastindexid": row.try_get::<i32, _>("podcastindexid").unwrap_or(0),
                    "podcastname": row.try_get::<String, _>("podcastname").unwrap_or_else(|_| "Unknown Podcast".to_string()),
                    "artworkurl": row.try_get::<Option<String>, _>("artworkurl").unwrap_or_default().unwrap_or_else(|| String::new()),
                    "author": row.try_get::<String, _>("author").unwrap_or_else(|_| "Unknown Author".to_string()),
                    "categories": categories,
                    "description": row.try_get::<String, _>("description").unwrap_or_else(|_| String::new()),
                    "episodecount": episode_count,
                    "feedurl": row.try_get::<String, _>("feedurl").unwrap_or_else(|_| String::new()),
                    "websiteurl": row.try_get::<String, _>("websiteurl").unwrap_or_else(|_| String::new()),
                    "explicit": row.try_get::<bool, _>("explicit").unwrap_or(false),
                    "userid": row.try_get::<i32, _>("userid")?,
                    "autodownload": row.try_get::<bool, _>("autodownload").unwrap_or(false),
                    "startskip": row.try_get::<i32, _>("startskip").unwrap_or(0),
                    "endskip": row.try_get::<i32, _>("endskip").unwrap_or(0),
                    "username": row.try_get::<Option<String>, _>("username")?,
                    "password": row.try_get::<Option<String>, _>("password")?,
                    "isyoutubechannel": is_youtube,
                    "notificationsenabled": row.try_get::<bool, _>("notificationsenabled").unwrap_or(false),
                    "feedcutoffdays": row.try_get::<i32, _>("feedcutoffdays").unwrap_or(0),
                    "playbackspeedcustomized": row.try_get::<bool, _>("playbackspeedcustomized").unwrap_or(false),
                    "playbackspeed": row.try_get::<f64, _>("playbackspeed").unwrap_or(1.0)
                }))
            },
            mysql(pool) => {
                // First try to get podcast for specific user
                let mut podcast_row = sqlx::query(r#"
                    SELECT PodcastID, PodcastName, FeedURL, Description, Author, ArtworkURL, 
                           Explicit, EpisodeCount, Categories, WebsiteURL, PodcastIndexID, IsYouTubeChannel,
                           UserID, AutoDownload, StartSkip, EndSkip, Username, Password, NotificationsEnabled, FeedCutoffDays,
                           PlaybackSpeed, PlaybackSpeedCustomized
                    FROM Podcasts 
                    WHERE PodcastID = ? AND UserID = ?
                "#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                // Fallback to admin/public podcasts (UserID = 1) if not found for user
                if podcast_row.is_none() {
                    podcast_row = sqlx::query(r#"
                        SELECT PodcastID, PodcastName, FeedURL, Description, Author, ArtworkURL, 
                               Explicit, EpisodeCount, Categories, WebsiteURL, PodcastIndexID, IsYouTubeChannel,
                               UserID, AutoDownload, StartSkip, EndSkip, Username, Password, NotificationsEnabled, FeedCutoffDays,
                               PlaybackSpeed, PlaybackSpeedCustomized
                        FROM Podcasts 
                        WHERE PodcastID = ? AND UserID = 1
                    "#)
                        .bind(podcast_id)
                        .fetch_optional(pool)
                        .await?;
                }

                let row = podcast_row.ok_or_else(|| AppError::not_found("Podcast not found"))?;

                // Get episode count (special handling for YouTube channels)
                let is_youtube: bool = row.try_get("IsYouTubeChannel").unwrap_or(false);
                let episode_count = if is_youtube {
                    // Get count from YouTubeVideos table for YouTube channels
                    let count_row = sqlx::query("SELECT COUNT(*) as count FROM YouTubeVideos WHERE PodcastID = ?")
                        .bind(podcast_id)
                        .fetch_one(pool)
                        .await?;
                    count_row.try_get::<i64, _>("count")? as i32
                } else {
                    row.try_get("EpisodeCount").unwrap_or(0)
                };

                // Get categories and parse to HashMap - matches Python version exactly
                let categories_str = row.try_get::<String, _>("Categories").unwrap_or_else(|_| String::new());
                let categories = self.parse_categories_json(&categories_str);

                Ok(serde_json::json!({
                    "podcastid": row.try_get::<i32, _>("PodcastID")?,
                    "podcastindexid": row.try_get::<i32, _>("PodcastIndexID").unwrap_or(0),
                    "podcastname": row.try_get::<String, _>("PodcastName").unwrap_or_else(|_| "Unknown Podcast".to_string()),
                    "artworkurl": row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default().unwrap_or_else(|| String::new()),
                    "author": row.try_get::<String, _>("Author").unwrap_or_else(|_| "Unknown Author".to_string()),
                    "categories": categories,
                    "description": row.try_get::<String, _>("Description").unwrap_or_else(|_| String::new()),
                    "episodecount": episode_count,
                    "feedurl": row.try_get::<String, _>("FeedURL").unwrap_or_else(|_| String::new()),
                    "websiteurl": row.try_get::<String, _>("WebsiteURL").unwrap_or_else(|_| String::new()),
                    "explicit": row.try_get::<i8, _>("Explicit").unwrap_or(0) != 0,
                    "userid": row.try_get::<i32, _>("UserID")?,
                    "autodownload": row.try_get::<i8, _>("AutoDownload").unwrap_or(0) != 0,
                    "startskip": row.try_get::<i32, _>("StartSkip").unwrap_or(0),
                    "endskip": row.try_get::<i32, _>("EndSkip").unwrap_or(0),
                    "username": row.try_get::<Option<String>, _>("Username")?,
                    "password": row.try_get::<Option<String>, _>("Password")?,
                    "isyoutubechannel": is_youtube,
                    "notificationsenabled": row.try_get::<i8, _>("NotificationsEnabled").unwrap_or(0) != 0,
                    "feedcutoffdays": row.try_get::<i32, _>("FeedCutoffDays").unwrap_or(0),
                    "playbackspeedcustomized": row.try_get::<i8, _>("PlaybackSpeedCustomized").unwrap_or(0) != 0,
                    "playbackspeed": if let Ok(speed) = row.try_get::<bigdecimal::BigDecimal, _>("PlaybackSpeed") {
                        speed.to_f64().unwrap_or(1.0)
                    } else {
                        1.0
                    }
                }))
            },
        })
    }

    // Check if user already has a specific podcast feed - matches Python logic
    pub async fn user_has_podcast_feed(&self, user_id: i32, feed_url: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT podcastid FROM "Podcasts" WHERE userid = $1 AND feedurl = $2"#)
                    .bind(user_id)
                    .bind(feed_url)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(row.is_some())
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT PodcastID FROM Podcasts WHERE UserID = ? AND FeedURL = ?")
                    .bind(user_id)
                    .bind(feed_url)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(row.is_some())
            },
        })
    }

    // Add PinePods news feed to admin users - matches Python add_news_feed_if_not_added function
    pub async fn add_news_feed_if_not_added(&self) -> AppResult<()> {
        let admin_user_ids = self.get_all_admin_user_ids().await?;
        let feed_url = "https://news.pinepods.online/feed.xml";

        for user_id in admin_user_ids {
            // Check if this user already has the news feed
            if !self.user_has_podcast_feed(user_id, feed_url).await? {
                // Add the PinePods news feed using existing functions - matches Python add_custom_podcast
                match self.get_podcast_values_with_body(feed_url, user_id, None, None).await {
                    Ok((podcast_values, body)) => {
                        let feed_cutoff = 30; // Default cutoff like Python
                        if let Err(e) = self.add_podcast_from_values(&podcast_values, user_id, feed_cutoff, None, None, Some(&body), crate::services::sync::SyncOrigin::Pinepods).await {
                            error!("Failed to add PinePods news feed for user {}: {}", user_id, e);
                            // Continue with other users even if one fails
                        }
                    },
                    Err(e) => {
                        error!("Failed to get podcast values for PinePods news feed for user {}: {}", user_id, e);
                        // Continue with other users even if one fails
                    }
                }
            }
        }

        Ok(())
    }

    // Get podcast ID by feed URL and title - for get_podcast_details_dynamic
    pub async fn get_podcast_id_by_feed(&self, user_id: i32, feed_url: &str, _podcast_title: &str) -> AppResult<i32> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT podcastid FROM "Podcasts" WHERE feedurl = $1 AND userid = $2"#
                )
                .bind(feed_url)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    Ok(row.try_get("podcastid")?)
                } else {
                    Err(AppError::not_found("Podcast not found"))
                }
            },
            mysql(pool) => {
                let row = sqlx::query(
                    "SELECT PodcastID FROM Podcasts WHERE FeedURL = ? AND UserID = ?"
                )
                .bind(feed_url)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    Ok(row.try_get("PodcastID")?)
                } else {
                    Err(AppError::not_found("Podcast not found"))
                }
            },
        })
    }

    // Get raw podcast details - returns all fields as JSON for get_podcast_details_dynamic
    pub async fn get_podcast_details_raw(&self, user_id: i32, podcast_id: i32) -> AppResult<Option<serde_json::Value>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT * FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#
                )
                .bind(podcast_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    let mut details = serde_json::Map::new();
                    
                    details.insert("podcastname".to_string(), serde_json::Value::String(row.try_get::<String, _>("podcastname").unwrap_or_default()));
                    details.insert("feedurl".to_string(), serde_json::Value::String(row.try_get::<String, _>("feedurl").unwrap_or_default()));
                    details.insert("description".to_string(), serde_json::Value::String(row.try_get::<String, _>("description").unwrap_or_default()));
                    details.insert("author".to_string(), serde_json::Value::String(row.try_get::<String, _>("author").unwrap_or_default()));
                    details.insert("artworkurl".to_string(), serde_json::Value::String(row.try_get::<Option<String>, _>("artworkurl").unwrap_or_default().unwrap_or_default()));
                    details.insert("explicit".to_string(), serde_json::Value::Bool(row.try_get::<bool, _>("explicit").unwrap_or(false)));
                    details.insert("episodecount".to_string(), serde_json::Value::Number(serde_json::Number::from(row.try_get::<i32, _>("episodecount").unwrap_or(0))));
                    let categories_str = row.try_get::<String, _>("categories").unwrap_or_default();
                    let categories_parsed = self.parse_categories_json(&categories_str).unwrap_or_default();
                    details.insert("categories".to_string(), serde_json::to_value(categories_parsed).unwrap_or(serde_json::Value::Object(serde_json::Map::new())));
                    details.insert("websiteurl".to_string(), serde_json::Value::String(row.try_get::<String, _>("websiteurl").unwrap_or_default()));
                    details.insert("podcastindexid".to_string(), serde_json::Value::Number(serde_json::Number::from(row.try_get::<i32, _>("podcastindexid").unwrap_or(0))));
                    details.insert("isyoutubechannel".to_string(), serde_json::Value::Bool(row.try_get::<bool, _>("isyoutubechannel").unwrap_or(false)));

                    Ok(Some(serde_json::Value::Object(details)))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query(
                    "SELECT * FROM Podcasts WHERE PodcastID = ? AND UserID = ?"
                )
                .bind(podcast_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    let mut details = serde_json::Map::new();
                    
                    details.insert("podcastname".to_string(), serde_json::Value::String(row.try_get::<String, _>("PodcastName").unwrap_or_default()));
                    details.insert("feedurl".to_string(), serde_json::Value::String(row.try_get::<String, _>("FeedURL").unwrap_or_default()));
                    details.insert("description".to_string(), serde_json::Value::String(row.try_get::<String, _>("Description").unwrap_or_default()));
                    details.insert("author".to_string(), serde_json::Value::String(row.try_get::<String, _>("Author").unwrap_or_default()));
                    details.insert("artworkurl".to_string(), serde_json::Value::String(row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default().unwrap_or_default()));
                    details.insert("explicit".to_string(), serde_json::Value::Bool(row.try_get::<bool, _>("Explicit").unwrap_or(false)));
                    details.insert("episodecount".to_string(), serde_json::Value::Number(serde_json::Number::from(row.try_get::<i32, _>("EpisodeCount").unwrap_or(0))));
                    let categories_str = row.try_get::<String, _>("Categories").unwrap_or_default();
                    let categories_parsed = self.parse_categories_json(&categories_str).unwrap_or_default();
                    details.insert("categories".to_string(), serde_json::to_value(categories_parsed).unwrap_or(serde_json::Value::Object(serde_json::Map::new())));
                    details.insert("websiteurl".to_string(), serde_json::Value::String(row.try_get::<String, _>("WebsiteURL").unwrap_or_default()));
                    details.insert("podcastindexid".to_string(), serde_json::Value::Number(serde_json::Number::from(row.try_get::<i32, _>("PodcastIndexID").unwrap_or(0))));
                    details.insert("isyoutubechannel".to_string(), serde_json::Value::Bool(row.try_get::<bool, _>("IsYouTubeChannel").unwrap_or(false)));

                    Ok(Some(serde_json::Value::Object(details)))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get podcast values from feed - for get_podcast_details_dynamic when podcast is not added
    pub async fn get_podcast_values_from_feed(&self, feed_url: &str, user_id: i32, _display_only: bool) -> AppResult<serde_json::Value> {
        // Use the real get_podcast_values function that exists in the codebase
        let podcast_values = self.get_podcast_values(feed_url, user_id, None, None).await?;
        
        // Convert HashMap to the expected JSON format for get_podcast_details_dynamic
        let response = serde_json::json!({
            "pod_title": podcast_values.get("podcastname").unwrap_or(&"Unknown Podcast".to_string()),
            "pod_feed_url": feed_url,
            "pod_description": podcast_values.get("description").unwrap_or(&"".to_string()),
            "pod_author": podcast_values.get("author").unwrap_or(&"Unknown Author".to_string()),
            "pod_artwork": podcast_values.get("artworkurl").unwrap_or(&"/static/assets/default-podcast.png".to_string()),
            "pod_explicit": podcast_values.get("explicit").unwrap_or(&"False".to_string()) == "True",
            "pod_episode_count": podcast_values.get("episodecount").unwrap_or(&"0".to_string()).parse::<i32>().unwrap_or(0),
            "categories": podcast_values.get("categories").unwrap_or(&"{}".to_string()),
            "pod_website": podcast_values.get("websiteurl").unwrap_or(&"".to_string()),
        });

        Ok(response)
    }

    // Update podcast basic info - for edit podcast functionality
    pub async fn update_podcast_info(&self, podcast_id: i32, user_id: i32, feed_url: Option<String>, username: Option<String>, password: Option<String>, podcast_name: Option<String>, description: Option<String>, author: Option<String>, artwork_url: Option<String>, website_url: Option<String>, podcast_index_id: Option<i64>) -> AppResult<bool> {
        // A new address or new credentials can make it another feed.
        let rekey = feed_url.is_some() || username.is_some() || password.is_some();
        dialect_match!(self, {
            pg(pool) => {
                // First verify podcast exists and belongs to user
                let existing = q::query(r#"SELECT podcastid FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                if existing.is_none() {
                    return Ok(false);
                }

                // Build dynamic update query based on provided fields
                let mut update_parts = Vec::new();
                let mut bind_count = 1;
                
                if feed_url.is_some() {
                    update_parts.push(format!("feedurl = ${}", bind_count));
                    bind_count += 1;
                }
                if username.is_some() {
                    update_parts.push(format!("username = ${}", bind_count));
                    bind_count += 1;
                }
                if password.is_some() {
                    update_parts.push(format!("password = ${}", bind_count));
                    bind_count += 1;
                }
                if podcast_name.is_some() {
                    update_parts.push(format!("podcastname = ${}", bind_count));
                    bind_count += 1;
                }
                if description.is_some() {
                    update_parts.push(format!("description = ${}", bind_count));
                    bind_count += 1;
                }
                if author.is_some() {
                    update_parts.push(format!("author = ${}", bind_count));
                    bind_count += 1;
                }
                if artwork_url.is_some() {
                    update_parts.push(format!("artworkurl = ${}", bind_count));
                    bind_count += 1;
                }
                if website_url.is_some() {
                    update_parts.push(format!("websiteurl = ${}", bind_count));
                    bind_count += 1;
                }
                if podcast_index_id.is_some() {
                    update_parts.push(format!("podcastindexid = ${}", bind_count));
                    bind_count += 1;
                }

                if update_parts.is_empty() {
                    return Ok(false);
                }

                let query_str = format!(
                    r#"UPDATE "Podcasts" SET {} WHERE podcastid = ${} AND userid = ${}"#,
                    update_parts.join(", "),
                    bind_count,
                    bind_count + 1
                );

                let mut query = q::query(sqlx::AssertSqlSafe(query_str.as_str()));
                
                if let Some(url) = feed_url {
                    query = query.bind(url);
                }
                if let Some(uname) = username {
                    query = query.bind(uname);
                }
                if let Some(pwd) = password {
                    query = query.bind(pwd);
                }
                if let Some(name) = podcast_name {
                    query = query.bind(name);
                }
                if let Some(desc) = description {
                    query = query.bind(desc);
                }
                if let Some(auth) = author {
                    query = query.bind(auth);
                }
                if let Some(artwork) = artwork_url {
                    query = query.bind(artwork);
                }
                if let Some(website) = website_url {
                    query = query.bind(website);
                }
                if let Some(idx_id) = podcast_index_id {
                    query = query.bind(idx_id);
                }
                
                query = query.bind(podcast_id).bind(user_id);
                
                let result = query.execute(pool).await?;
                let changed = result.rows_affected() > 0;
                if changed && rekey {
                    self.link_feed_or_warn(podcast_id).await;
                }
                Ok(changed)
            },
            mysql(pool) => {
                // First verify podcast exists and belongs to user
                let existing = sqlx::query("SELECT PodcastID FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                if existing.is_none() {
                    return Ok(false);
                }

                // Build dynamic update query based on provided fields
                let mut update_parts = Vec::new();
                
                if feed_url.is_some() {
                    update_parts.push("FeedURL = ?");
                }
                if username.is_some() {
                    update_parts.push("Username = ?");
                }
                if password.is_some() {
                    update_parts.push("Password = ?");
                }
                if podcast_name.is_some() {
                    update_parts.push("PodcastName = ?");
                }
                if description.is_some() {
                    update_parts.push("Description = ?");
                }
                if author.is_some() {
                    update_parts.push("Author = ?");
                }
                if artwork_url.is_some() {
                    update_parts.push("ArtworkURL = ?");
                }
                if website_url.is_some() {
                    update_parts.push("WebsiteURL = ?");
                }
                if podcast_index_id.is_some() {
                    update_parts.push("PodcastIndexID = ?");
                }

                if update_parts.is_empty() {
                    return Ok(false);
                }

                let query_str = format!(
                    "UPDATE Podcasts SET {} WHERE PodcastID = ? AND UserID = ?",
                    update_parts.join(", ")
                );

                let mut query = sqlx::query(sqlx::AssertSqlSafe(query_str.as_str()));
                
                if let Some(url) = feed_url {
                    query = query.bind(url);
                }
                if let Some(uname) = username {
                    query = query.bind(uname);
                }
                if let Some(pwd) = password {
                    query = query.bind(pwd);
                }
                if let Some(name) = podcast_name {
                    query = query.bind(name);
                }
                if let Some(desc) = description {
                    query = query.bind(desc);
                }
                if let Some(auth) = author {
                    query = query.bind(auth);
                }
                if let Some(artwork) = artwork_url {
                    query = query.bind(artwork);
                }
                if let Some(website) = website_url {
                    query = query.bind(website);
                }
                if let Some(idx_id) = podcast_index_id {
                    query = query.bind(idx_id);
                }
                
                query = query.bind(podcast_id).bind(user_id);
                
                let result = query.execute(pool).await?;
                let changed = result.rows_affected() > 0;
                if changed && rekey {
                    self.link_feed_or_warn(podcast_id).await;
                }
                Ok(changed)
            },
        })
    }

    // Add podcast from URL - used by Nextcloud sync
    pub async fn add_podcast_from_url(&self, user_id: i32, feed_url: &str, _feed_cutoff: Option<i32>, origin: crate::services::sync::SyncOrigin) -> AppResult<()> {
        if let Some(podcast_id) = self.subscription_for_url(user_id, feed_url).await? {
            tracing::info!("Podcast {} already exists for user {}", feed_url, user_id);
            return self.add_feed_alias(user_id, podcast_id, feed_url).await;
        }

        // Get podcast metadata from feed URL using existing function
        let (podcast_values, body) = self.get_podcast_values_with_body(feed_url, user_id, None, None).await?;

        // A sync app may know a show the user has under its new address: remember the address
        // instead of subscribing twice.
        let title = podcast_values.get("podcastname").map(String::as_str).unwrap_or_default();
        if let Some(podcast_id) = self.same_show(user_id, title, &body).await? {
            tracing::info!("{} is podcast {} under another address for user {}; not adding it again", feed_url, podcast_id, user_id);
            if !self.adopt_feed_url_if_failing(user_id, podcast_id, feed_url).await? {
                self.add_feed_alias(user_id, podcast_id, feed_url).await?;
            }
            return Ok(());
        }
        
        // Add podcast using existing function
        let _result = self.add_podcast_from_values(&podcast_values, user_id, 30, None, None, Some(&body), origin).await?;
        
        tracing::info!("Successfully added podcast {} for user {}", feed_url, user_id);
        Ok(())
    }

    // Remove podcast by URL - used by sync (subscription removes)
    pub async fn remove_podcast_by_url(&self, user_id: i32, feed_url: &str, origin: crate::services::sync::SyncOrigin) -> AppResult<()> {
        match self.subscription_for_url(user_id, feed_url).await? {
            Some(podcast_id) if self.remove_podcast_cascade(podcast_id, user_id, origin).await? => {
                tracing::info!("Successfully removed podcast {} for user {}", feed_url, user_id);
            }
            _ => tracing::info!("Podcast {} not found for user {}", feed_url, user_id),
        }
        Ok(())
    }

    // Get podcasts that have podcast_index_id = 0 (imported without podcast index match)
    pub async fn get_unmatched_podcasts(&self, user_id: i32) -> AppResult<Vec<serde_json::Value>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT podcastid, podcastname, artworkurl, author, description, feedurl
                       FROM "Podcasts" 
                       WHERE userid = $1 AND (podcastindexid = 0 OR podcastindexid IS NULL) 
                       AND (ignorepodcastindex = FALSE OR ignorepodcastindex IS NULL)
                       ORDER BY podcastname"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut podcasts = Vec::new();
                for row in rows {
                    let podcast = serde_json::json!({
                        "podcast_id": row.try_get::<i32, _>("podcastid")?,
                        "podcast_name": row.try_get::<String, _>("podcastname")?,
                        "artwork_url": row.try_get::<Option<String>, _>("artworkurl")?,
                        "author": row.try_get::<Option<String>, _>("author")?,
                        "description": row.try_get::<Option<String>, _>("description")?,
                        "feed_url": row.try_get::<String, _>("feedurl")?
                    });
                    podcasts.push(podcast);
                }

                Ok(podcasts)
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT PodcastID, PodcastName, ArtworkURL, Author, Description, FeedURL
                     FROM Podcasts 
                     WHERE UserID = ? AND (PodcastIndexID = 0 OR PodcastIndexID IS NULL) 
                     AND (IgnorePodcastIndex = 0 OR IgnorePodcastIndex IS NULL)
                     ORDER BY PodcastName"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut podcasts = Vec::new();
                for row in rows {
                    let podcast = serde_json::json!({
                        "podcast_id": row.try_get::<i32, _>("PodcastID")?,
                        "podcast_name": row.try_get::<String, _>("PodcastName")?,
                        "artwork_url": row.try_get::<Option<String>, _>("ArtworkURL")?,
                        "author": row.try_get::<Option<String>, _>("Author")?,
                        "description": row.try_get::<Option<String>, _>("Description")?,
                        "feed_url": row.try_get::<String, _>("FeedURL")?
                    });
                    podcasts.push(podcast);
                }

                Ok(podcasts)
            },
        })
    }

    // Update a podcast's podcast_index_id
    pub async fn update_podcast_index_id(&self, user_id: i32, podcast_id: i32, podcast_index_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(
                    r#"UPDATE "Podcasts" 
                       SET podcastindexid = $1 
                       WHERE podcastid = $2 AND userid = $3"#
                )
                .bind(podcast_index_id)
                .bind(podcast_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                sqlx::query(
                    "UPDATE Podcasts 
                     SET PodcastIndexID = ? 
                     WHERE PodcastID = ? AND UserID = ?"
                )
                .bind(podcast_index_id)
                .bind(podcast_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
        });
        Ok(())
    }

    // Ignore/unignore a podcast's index ID requirement
    pub async fn ignore_podcast_index_id(&self, user_id: i32, podcast_id: i32, ignore: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(
                    r#"UPDATE "Podcasts" 
                       SET ignorepodcastindex = $1 
                       WHERE podcastid = $2 AND userid = $3"#
                )
                .bind(ignore)
                .bind(podcast_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                sqlx::query(
                    "UPDATE Podcasts 
                     SET IgnorePodcastIndex = ? 
                     WHERE PodcastID = ? AND UserID = ?"
                )
                .bind(ignore)
                .bind(podcast_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
        });
        Ok(())
    }

    // Get ignored podcasts for a user  
    pub async fn get_ignored_podcasts(&self, user_id: i32) -> AppResult<Vec<serde_json::Value>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT podcastid, podcastname, artworkurl, author, description, feedurl
                       FROM "Podcasts" 
                       WHERE userid = $1 AND ignorepodcastindex = TRUE
                       ORDER BY podcastname"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut podcasts = Vec::new();
                for row in rows {
                    let podcast = serde_json::json!({
                        "podcast_id": row.try_get::<i32, _>("podcastid")?,
                        "podcast_name": row.try_get::<String, _>("podcastname")?,
                        "artwork_url": row.try_get::<Option<String>, _>("artworkurl")?,
                        "author": row.try_get::<Option<String>, _>("author")?,
                        "description": row.try_get::<Option<String>, _>("description")?,
                        "feed_url": row.try_get::<String, _>("feedurl")?
                    });
                    podcasts.push(podcast);
                }

                Ok(podcasts)
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT PodcastID, PodcastName, ArtworkURL, Author, Description, FeedURL
                     FROM Podcasts 
                     WHERE UserID = ? AND IgnorePodcastIndex = 1
                     ORDER BY PodcastName"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut podcasts = Vec::new();
                for row in rows {
                    let podcast = serde_json::json!({
                        "podcast_id": row.try_get::<i32, _>("PodcastID")?,
                        "podcast_name": row.try_get::<String, _>("PodcastName")?,
                        "artwork_url": row.try_get::<Option<String>, _>("ArtworkURL")?,
                        "author": row.try_get::<Option<String>, _>("Author")?,
                        "description": row.try_get::<Option<String>, _>("Description")?,
                        "feed_url": row.try_get::<String, _>("FeedURL")?
                    });
                    podcasts.push(podcast);
                }

                Ok(podcasts)
            },
        })
    }
}

#[cfg(test)]
mod clean_categories_tests {
    use super::clean_categories;
    use std::collections::HashMap;

    fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn drops_blank_and_whitespace_categories() {
        let cleaned = clean_categories(map(&[("1", "News"), ("2", ""), ("3", "   ")]));
        assert_eq!(cleaned, map(&[("1", "News")]));
    }

    #[test]
    fn keeps_one_of_each_name_ignoring_case_and_space() {
        let cleaned = clean_categories(map(&[("10", "Technology"), ("2", "technology "), ("3", "News")]));
        assert_eq!(cleaned, map(&[("2", "technology"), ("3", "News")]));
    }
}
