// Feeds: one row per feed address and credentials, shared by every subscription to it. A
// subscription (Podcasts row) points at its feed through Podcasts.FeedID; the refresh is
// scheduled per feed and its state (validators, schedule, failures) lives on the feed.

use sqlx::Row;
use tracing::{debug, warn};

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};

/// Which subscriptions share a feed besides its address: '' for public feeds, a hash of the
/// credentials for private ones, and the owner for anything that isn't http(s) (uploads are
/// never shared). Must match _feed_credential_key in database_functions/migration_definitions.py.
pub fn credential_key(feed_url: &str, username: Option<&str>, password: Option<&str>, user_id: i32) -> String {
    let url = feed_url.trim().to_ascii_lowercase();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return crate::services::http::content_hash(&format!("local\0{user_id}"));
    }
    let (username, password) = (username.unwrap_or(""), password.unwrap_or(""));
    if username.is_empty() && password.is_empty() {
        return String::new();
    }
    crate::services::http::content_hash(&format!("{username}\0{password}"))
}

/// Feeds.FeedURLHash: the address in the unique key (TEXT can't be indexed on MySQL).
pub fn url_hash(feed_url: &str) -> String {
    crate::services::http::content_hash(feed_url)
}

// The refresh state a new feed starts from: the subscription's own.
const STATE_COLUMNS: &str = "FeedETag, FeedLastModified, FeedContentHash, RefreshNotBefore, NextRefreshAt, \
     RefreshIntervalMinutes, LastRefreshAttempt, LastRefreshSuccess, ConsecutiveFailures, LastErrorMessage, \
     FailingSince, LastErrorKind";
const STATE_VALUES: &str = "FeedETag, FeedLastModified, FeedContentHash, RefreshNotBefore, NextRefreshAt, \
     RefreshIntervalMinutes, LastRefreshAttempt, LastRefreshSuccess, COALESCE(ConsecutiveFailures, 0), LastErrorMessage, \
     FailingSince, LastErrorKind";

impl DatabasePool {
    /// Point a subscription at the feed its address and credentials belong to, creating the feed
    /// if it's new. Returns the feed, or None for a subscription without an address.
    pub async fn link_feed(&self, podcast_id: i32) -> AppResult<Option<i32>> {
        type Sub = (Option<String>, Option<String>, Option<String>, i32, Option<i32>);
        let sub: Option<Sub> = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_as(r#"SELECT feedurl, username, password, userid, feedid FROM "Podcasts" WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_as("SELECT FeedURL, Username, Password, UserID, FeedID FROM Podcasts WHERE PodcastID = ?")
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?
            }
        };
        let Some((Some(url), username, password, user_id, current)) = sub else { return Ok(None) };
        if url.is_empty() {
            return Ok(None);
        }
        let key = credential_key(&url, username.as_deref(), password.as_deref(), user_id);
        let hash = url_hash(&url);
        let feed_id: i32 = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    r#"INSERT INTO "Feeds" (FeedURL, FeedURLHash, CredentialKey, IsYouTubeChannel, {STATE_COLUMNS})
                       SELECT FeedURL, $2, $3, COALESCE(IsYouTubeChannel, FALSE), {STATE_VALUES}
                       FROM "Podcasts" WHERE PodcastID = $1
                       ON CONFLICT (FeedURLHash, CredentialKey) DO NOTHING"#
                )))
                .bind(podcast_id)
                .bind(&hash)
                .bind(&key)
                .execute(pool)
                .await?;
                sqlx::query_scalar(r#"SELECT feedid FROM "Feeds" WHERE feedurlhash = $1 AND credentialkey = $2"#)
                    .bind(&hash)
                    .bind(&key)
                    .fetch_one(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT INTO Feeds (FeedURL, FeedURLHash, CredentialKey, IsYouTubeChannel, {STATE_COLUMNS})
                     SELECT FeedURL, ?, ?, COALESCE(IsYouTubeChannel, 0), {STATE_VALUES}
                     FROM Podcasts WHERE PodcastID = ?
                     ON DUPLICATE KEY UPDATE Feeds.FeedID = Feeds.FeedID"
                )))
                .bind(&hash)
                .bind(&key)
                .bind(podcast_id)
                .execute(pool)
                .await?;
                sqlx::query_scalar("SELECT FeedID FROM Feeds WHERE FeedURLHash = ? AND CredentialKey = ?")
                    .bind(&hash)
                    .bind(&key)
                    .fetch_one(pool)
                    .await?
            }
        };
        if current == Some(feed_id) {
            return Ok(Some(feed_id));
        }
        // A subscription joining a feed that waits longer than it would have brings the refresh in.
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(r#"UPDATE "Podcasts" SET feedid = $1 WHERE podcastid = $2"#)
                    .bind(feed_id)
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    r#"UPDATE "Feeds" f SET nextrefreshat = p.nextrefreshat FROM "Podcasts" p
                       WHERE f.feedid = $1 AND p.podcastid = $2 AND p.nextrefreshat < f.nextrefreshat"#,
                )
                .bind(feed_id)
                .bind(podcast_id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query("UPDATE Podcasts SET FeedID = ? WHERE PodcastID = ?")
                    .bind(feed_id)
                    .bind(podcast_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "UPDATE Feeds f JOIN Podcasts p ON p.PodcastID = ? SET f.NextRefreshAt = p.NextRefreshAt
                     WHERE f.FeedID = ? AND p.NextRefreshAt < f.NextRefreshAt",
                )
                .bind(podcast_id)
                .bind(feed_id)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
            }
        }
        Ok(Some(feed_id))
    }

    /// The feed a subscription's episodes belong to, linking it first if it isn't yet.
    pub async fn feed_of_podcast(&self, podcast_id: i32) -> AppResult<i32> {
        let current: Option<Option<i32>> = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar(r#"SELECT feedid FROM "Podcasts" WHERE podcastid = $1"#)
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_scalar("SELECT FeedID FROM Podcasts WHERE PodcastID = ?")
                    .bind(podcast_id)
                    .fetch_optional(pool)
                    .await?
            }
        };
        match current {
            Some(Some(feed_id)) => Ok(feed_id),
            Some(None) => self
                .link_feed(podcast_id)
                .await?
                .ok_or_else(|| AppError::bad_request("Podcast has no feed address")),
            None => Err(AppError::not_found("Podcast not found")),
        }
    }

    /// The subscriptions (podcast ids) of a feed.
    pub async fn feed_subscriptions(&self, feed_id: i32) -> AppResult<Vec<i32>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar(r#"SELECT podcastid FROM "Podcasts" WHERE feedid = $1 ORDER BY podcastid"#)
                    .bind(feed_id)
                    .fetch_all(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_scalar("SELECT PodcastID FROM Podcasts WHERE FeedID = ? ORDER BY PodcastID")
                    .bind(feed_id)
                    .fetch_all(pool)
                    .await?
            }
        })
    }

    /// `link_feed` for callers that carry on regardless: a failure is logged and the next
    /// refresh tick links the subscription instead.
    pub async fn link_feed_or_warn(&self, podcast_id: i32) {
        if let Err(e) = self.link_feed(podcast_id).await {
            warn!("Linking podcast {} to its feed failed: {}", podcast_id, e);
        }
    }

    /// Link every subscription without a feed, or whose address no longer matches its feed's.
    /// Returns how many were linked.
    pub async fn link_unlinked_feeds(&self) -> AppResult<usize> {
        let ids: Vec<i32> = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar(
                    r#"SELECT p.podcastid FROM "Podcasts" p LEFT JOIN "Feeds" f ON f.feedid = p.feedid
                       WHERE p.feedurl IS NOT NULL AND p.feedurl <> '' AND (f.feedid IS NULL OR f.feedurl <> p.feedurl)"#,
                )
                .fetch_all(pool)
                .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_scalar(
                    "SELECT p.PodcastID FROM Podcasts p LEFT JOIN Feeds f ON f.FeedID = p.FeedID
                     WHERE p.FeedURL IS NOT NULL AND p.FeedURL <> '' AND (f.FeedID IS NULL OR f.FeedURL <> p.FeedURL)",
                )
                .fetch_all(pool)
                .await?
            }
        };
        for &id in &ids {
            self.link_feed(id).await?;
        }
        if !ids.is_empty() {
            debug!("Linked {} podcasts to their feeds", ids.len());
        }
        Ok(ids.len())
    }

    /// Delete feeds nobody subscribes to any more. Feeds younger than an hour are left alone, so
    /// one that is being linked right now isn't caught between its insert and its subscription.
    pub async fn delete_orphan_feeds(&self) -> AppResult<u64> {
        let deleted = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(
                    r#"DELETE FROM "Feeds" f WHERE f.createdat < NOW() - INTERVAL '1 hour'
                       AND NOT EXISTS (SELECT 1 FROM "Podcasts" p WHERE p.feedid = f.feedid)"#,
                )
                .execute(pool)
                .await?
                .rows_affected()
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(
                    "DELETE FROM Feeds WHERE CreatedAt < NOW() - INTERVAL 1 HOUR
                     AND NOT EXISTS (SELECT 1 FROM Podcasts p WHERE p.FeedID = Feeds.FeedID)",
                )
                .execute(pool)
                .await?
                .rows_affected()
            }
        };
        if deleted > 0 {
            debug!("Deleted {} feeds without subscribers", deleted);
        }
        Ok(deleted)
    }

    /// Feeds due for a scheduled refresh, the longest overdue first. A feed needs a subscriber
    /// that still wants it refreshed; feeds in failure backoff or under a host's Retry-After hold
    /// wait.
    pub async fn due_refresh_feeds(&self, limit: i64) -> AppResult<Vec<i32>> {
        use crate::services::scheduler::{BACKOFF_MAX_STEPS, BACKOFF_STEP_MINUTES};
        let ids = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    r#"SELECT f.feedid FROM "Feeds" f
                       WHERE f.feedurl NOT LIKE 'local://%'
                         AND EXISTS (SELECT 1 FROM "Podcasts" p WHERE p.feedid = f.feedid AND COALESCE(p.refreshpodcast, TRUE))
                         AND (f.nextrefreshat IS NULL OR f.nextrefreshat <= NOW())
                         AND (f.refreshnotbefore IS NULL OR f.refreshnotbefore <= NOW())
                         AND (f.consecutivefailures = 0 OR f.lastrefreshattempt IS NULL
                              OR f.lastrefreshattempt < NOW() - (INTERVAL '1 minute' * ({BACKOFF_STEP_MINUTES} * LEAST(f.consecutivefailures, {BACKOFF_MAX_STEPS}))))
                       ORDER BY f.nextrefreshat ASC NULLS FIRST
                       LIMIT $1"#
                )))
                .bind(limit)
                .fetch_all(pool)
                .await?
                .iter()
                .map(|r| r.try_get("feedid"))
                .collect::<Result<Vec<i32>, _>>()?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "SELECT f.FeedID FROM Feeds f
                     WHERE f.FeedURL NOT LIKE 'local://%'
                       AND EXISTS (SELECT 1 FROM Podcasts p WHERE p.FeedID = f.FeedID AND COALESCE(p.RefreshPodcast, 1) = 1)
                       AND (f.NextRefreshAt IS NULL OR f.NextRefreshAt <= NOW())
                       AND (f.RefreshNotBefore IS NULL OR f.RefreshNotBefore <= NOW())
                       AND (f.ConsecutiveFailures = 0 OR f.LastRefreshAttempt IS NULL
                            OR f.LastRefreshAttempt < NOW() - INTERVAL ({BACKOFF_STEP_MINUTES} * LEAST(f.ConsecutiveFailures, {BACKOFF_MAX_STEPS})) MINUTE)
                     ORDER BY f.NextRefreshAt IS NOT NULL, f.NextRefreshAt ASC
                     LIMIT ?"
                )))
                .bind(limit)
                .fetch_all(pool)
                .await?
                .iter()
                .map(|r| r.try_get("FeedID"))
                .collect::<Result<Vec<i32>, _>>()?
            }
        };
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::credential_key;

    #[test]
    fn credential_keys() {
        assert_eq!(credential_key("https://example.com/feed", None, Some(""), 1), "");
        assert_eq!(credential_key("https://example.com/feed", None, None, 2), "");
        // Same credentials share a feed whoever subscribes; other credentials don't.
        let bob = credential_key("https://example.com/feed", Some("bob"), Some("pw"), 1);
        assert_eq!(bob, credential_key("https://example.com/feed", Some("bob"), Some("pw"), 2));
        assert_ne!(bob, credential_key("https://example.com/feed", Some("bob"), Some("pw2"), 1));
        // Uploads belong to their owner.
        assert_ne!(credential_key("local://1", None, None, 1), credential_key("local://1", None, None, 2));
        // The migration computes the same keys.
        assert_eq!(bob, "91e3d51d23ba15185dc7a7464f764c8d73c9b3e6be9cb9553c053f42e2aa4905");
        assert_eq!(credential_key("local://x", None, None, 1), "c3d6533821de63bde3ba24db9e838433f7299d4fd406145f25557b6e9e6a72dd");
    }
}
