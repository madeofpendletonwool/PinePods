// Which of a user's podcasts a feed address belongs to. Sync apps can hold a feed under another
// address than Pinepods (they follow feed moves, or write the address differently), so addresses
// are compared loosely, a podcast's other addresses are remembered in PodcastFeedAliases, and a
// feed that turns out to be a show the user already has is never added a second time.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use tracing::info;

use crate::database::DatabasePool;
use crate::error::AppResult;

/// How feed addresses are compared: no scheme, no "www.", no trailing slash, any case. Must
/// match _feed_url_key in database_functions/migration_definitions.py.
pub fn feed_url_key(url: &str) -> String {
    let u = url.trim().to_lowercase();
    let u = u.strip_prefix("https://").or_else(|| u.strip_prefix("http://")).unwrap_or(&u);
    let u = u.strip_prefix("www.").unwrap_or(u);
    u.trim_end_matches('/').to_string()
}

fn title_key(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

static PODCAST_GUID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<podcast:guid>\s*(.*?)\s*</podcast:guid>").unwrap());
static NEW_FEED_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<itunes:new-feed-url>\s*(.*?)\s*</itunes:new-feed-url>").unwrap());
static ITEM_GUID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<guid\b[^>]*>\s*(?:<!\[CDATA\[)?\s*(.*?)\s*(?:\]\]>)?\s*</guid>").unwrap());
static ENCLOSURE_URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"<enclosure\b[^>]*\burl\s*=\s*["']([^"']+)["']"#).unwrap());

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
}

/// What a fetched feed says about which show it is.
#[derive(Debug, Default)]
pub struct FeedIdentity {
    pub podcast_guid: Option<String>,
    pub new_feed_url: Option<String>,
    pub episode_guids: Vec<String>,
    pub episode_urls: Vec<String>,
}

impl FeedIdentity {
    pub fn of(body: &str) -> Self {
        let first = |re: &Regex| re.captures(body).map(|c| unescape(&c[1])).filter(|s| !s.is_empty());
        let all = |re: &Regex| {
            let mut seen = HashSet::new();
            re.captures_iter(body).map(|c| unescape(&c[1])).filter(|s| !s.is_empty() && seen.insert(s.clone())).collect()
        };
        FeedIdentity {
            podcast_guid: first(&PODCAST_GUID),
            new_feed_url: first(&NEW_FEED_URL),
            episode_guids: all(&ITEM_GUID),
            episode_urls: all(&ENCLOSURE_URL),
        }
    }

    fn episode_count(&self) -> usize {
        self.episode_guids.len().max(self.episode_urls.len())
    }
}

impl DatabasePool {
    /// The user's (podcast id, address) pairs: feed URLs, then remembered aliases.
    async fn subscription_addresses(&self, user_id: i32) -> AppResult<Vec<(i32, String)>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT podcastid, feedurl FROM "Podcasts" WHERE userid = $1 AND feedurl IS NOT NULL
                   UNION ALL SELECT podcastid, feedurl FROM "PodcastFeedAliases" WHERE userid = $1"#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as(
                "SELECT PodcastID, FeedURL FROM Podcasts WHERE UserID = ? AND FeedURL IS NOT NULL
                 UNION ALL SELECT PodcastID, FeedURL FROM PodcastFeedAliases WHERE UserID = ?",
            )
            .bind(user_id)
            .bind(user_id)
            .fetch_all(pool)
            .await?,
        })
    }

    /// Every address the user's podcasts are known under, as `feed_url_key`s.
    pub async fn subscription_url_keys(&self, user_id: i32) -> AppResult<HashSet<String>> {
        Ok(self.subscription_addresses(user_id).await?.into_iter().map(|(_, url)| feed_url_key(&url)).collect())
    }

    /// The user's podcasts (by their own feed URL) that `addresses` already name under any of
    /// their addresses.
    pub async fn feed_urls_known_under(&self, user_id: i32, addresses: &[String]) -> AppResult<HashSet<String>> {
        let named: HashSet<String> = addresses.iter().map(|u| feed_url_key(u)).collect();
        let all = self.subscription_addresses(user_id).await?;
        let ids: HashSet<i32> = all.iter().filter(|(_, u)| named.contains(&feed_url_key(u))).map(|(id, _)| *id).collect();
        let own: Vec<(i32, String)> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(r#"SELECT podcastid, feedurl FROM "Podcasts" WHERE userid = $1 AND feedurl IS NOT NULL"#)
                .bind(user_id)
                .fetch_all(pool)
                .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as("SELECT PodcastID, FeedURL FROM Podcasts WHERE UserID = ? AND FeedURL IS NOT NULL")
                .bind(user_id)
                .fetch_all(pool)
                .await?,
        };
        Ok(own.into_iter().filter(|(id, _)| ids.contains(id)).map(|(_, url)| url).collect())
    }

    /// The user's podcast with this address: the exact one, an equivalent one or an alias.
    pub async fn subscription_for_url(&self, user_id: i32, url: &str) -> AppResult<Option<i32>> {
        let addresses = self.subscription_addresses(user_id).await?;
        if let Some((id, _)) = addresses.iter().find(|(_, u)| u == url) {
            return Ok(Some(*id));
        }
        let key = feed_url_key(url);
        Ok(addresses.into_iter().find(|(_, u)| feed_url_key(u) == key).map(|(id, _)| id))
    }

    /// Remember `url` as another address of the user's podcast.
    pub async fn add_feed_alias(&self, user_id: i32, podcast_id: i32, url: &str) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(
                    r#"INSERT INTO "PodcastFeedAliases" (podcastid, userid, feedurl)
                       SELECT $1, $2, $3 WHERE NOT EXISTS (SELECT 1 FROM "Podcasts" WHERE podcastid = $1 AND feedurl = $3)
                       ON CONFLICT (userid, feedurl) DO NOTHING"#,
                )
                .bind(podcast_id)
                .bind(user_id)
                .bind(url)
                .execute(pool)
                .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(
                    "INSERT IGNORE INTO PodcastFeedAliases (PodcastID, UserID, FeedURL)
                     SELECT ?, ?, ? FROM DUAL WHERE NOT EXISTS (SELECT 1 FROM Podcasts WHERE PodcastID = ? AND FeedURL = ?)",
                )
                .bind(podcast_id)
                .bind(user_id)
                .bind(url)
                .bind(podcast_id)
                .bind(url)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    /// The user's podcast that `body` (fetched from a new address) is the same show as: same
    /// Podcasting 2.0 GUID, a declared new address the user has, or the same title with episodes
    /// in common.
    pub async fn same_show(&self, user_id: i32, title: &str, body: &str) -> AppResult<Option<i32>> {
        let feed = FeedIdentity::of(body);
        if let Some(new_url) = &feed.new_feed_url {
            if let Some(id) = self.subscription_for_url(user_id, new_url).await? {
                return Ok(Some(id));
            }
        }
        let podcasts: Vec<(i32, Option<String>, Option<String>, i64)> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT p.podcastid, p.podcastname, p.podcastguid, (SELECT COUNT(*) FROM "Episodes" e WHERE e.podcastid = p.podcastid)
                   FROM "Podcasts" p WHERE p.userid = $1 AND NOT COALESCE(p.isyoutubechannel, FALSE)"#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as(
                "SELECT p.PodcastID, p.PodcastName, p.PodcastGUID, CAST((SELECT COUNT(*) FROM Episodes e WHERE e.PodcastID = p.PodcastID) AS SIGNED)
                 FROM Podcasts p WHERE p.UserID = ? AND COALESCE(p.IsYouTubeChannel, 0) = 0",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
        };
        if let Some(guid) = &feed.podcast_guid {
            if let Some((id, ..)) = podcasts.iter().find(|p| p.2.as_deref().is_some_and(|g| g.trim().eq_ignore_ascii_case(guid))) {
                return Ok(Some(*id));
            }
        }
        let title = title_key(title);
        if title.is_empty() {
            return Ok(None);
        }
        for (id, name, _, count) in podcasts.iter().filter(|p| p.1.as_deref().map(title_key).as_deref() == Some(title.as_str())) {
            let needed = 3.min(feed.episode_count()).min(*count as usize);
            if needed == 0 {
                continue;
            }
            let shared = self.shared_episode_count(*id, &feed).await?;
            if shared >= needed {
                info!("Feed for {:?} is the same show as podcast {} ({} episodes in common)", name.as_deref().unwrap_or_default(), id, shared);
                return Ok(Some(*id));
            }
        }
        Ok(None)
    }

    async fn shared_episode_count(&self, podcast_id: i32, feed: &FeedIdentity) -> AppResult<usize> {
        let count: i64 = match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM "Episodes" WHERE podcastid = $1 AND (episodeguid = ANY($2) OR episodeurl = ANY($3))"#,
            )
            .bind(podcast_id)
            .bind(&feed.episode_guids)
            .bind(&feed.episode_urls)
            .fetch_one(pool)
            .await?,
            DatabasePool::MySQL(pool) => {
                // Only the newest episodes are compared, to keep the statement small.
                let guids: Vec<&String> = feed.episode_guids.iter().take(200).collect();
                let urls: Vec<&String> = feed.episode_urls.iter().take(200).collect();
                let list = |n: usize| if n == 0 { "NULL".to_string() } else { vec!["?"; n].join(", ") };
                let sql = format!(
                    "SELECT COUNT(*) FROM Episodes WHERE PodcastID = ? AND (EpisodeGUID IN ({}) OR EpisodeURL IN ({}))",
                    list(guids.len()),
                    list(urls.len())
                );
                let mut q = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str())).bind(podcast_id);
                for v in guids.into_iter().chain(urls) {
                    q = q.bind(v);
                }
                q.fetch_one(pool).await?
            }
        };
        Ok(count as usize)
    }

    /// A podcast whose own address keeps failing moves to `url`, where the same show was found;
    /// the old address stays known as an alias.
    pub async fn adopt_feed_url_if_failing(&self, user_id: i32, podcast_id: i32, url: &str) -> AppResult<bool> {
        let old: Option<String> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(
                r#"SELECT feedurl FROM "Podcasts" WHERE podcastid = $1 AND userid = $2 AND COALESCE(consecutivefailures, 0) > 0"#,
            )
            .bind(podcast_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_scalar(
                "SELECT FeedURL FROM Podcasts WHERE PodcastID = ? AND UserID = ? AND COALESCE(ConsecutiveFailures, 0) > 0",
            )
            .bind(podcast_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?,
        };
        let Some(old) = old else { return Ok(false) };
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(r#"DELETE FROM "PodcastFeedAliases" WHERE userid = $1 AND feedurl = $2"#)
                    .bind(user_id)
                    .bind(url)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    r#"UPDATE "Podcasts" SET feedurl = $1, feedetag = NULL, feedlastmodified = NULL, feedcontenthash = NULL
                       WHERE podcastid = $2"#,
                )
                .bind(url)
                .bind(podcast_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(r#"INSERT INTO "PodcastFeedAliases" (podcastid, userid, feedurl) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .bind(&old)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query("DELETE FROM PodcastFeedAliases WHERE UserID = ? AND FeedURLHash = SHA2(?, 256)")
                    .bind(user_id)
                    .bind(url)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "UPDATE Podcasts SET FeedURL = ?, FeedETag = NULL, FeedLastModified = NULL, FeedContentHash = NULL WHERE PodcastID = ?",
                )
                .bind(url)
                .bind(podcast_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query("INSERT IGNORE INTO PodcastFeedAliases (PodcastID, UserID, FeedURL) VALUES (?, ?, ?)")
                    .bind(podcast_id)
                    .bind(user_id)
                    .bind(&old)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
            }
        }
        self.link_feed_or_warn(podcast_id).await;
        info!("Podcast {} moved from its failing address {} to {}", podcast_id, old, url);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_compare_without_scheme_www_slash_or_case() {
        let a = feed_url_key("https://omnycontent.com/d/playlist/E062C374/podcast.rss");
        assert_eq!(a, feed_url_key("http://www.omnycontent.com/d/playlist/e062c374/podcast.rss/"));
        assert_ne!(a, feed_url_key("https://omnycontent.com/d/playlist/other/podcast.rss"));
    }

    #[test]
    fn reads_what_identifies_a_feed() {
        let body = r#"<rss><channel><podcast:guid> abc-123 </podcast:guid>
            <itunes:new-feed-url>https://new.example/feed?a=1&amp;b=2</itunes:new-feed-url>
            <item><guid isPermaLink="false"><![CDATA[ep-1]]></guid><enclosure url="https://cdn.example/1.mp3" type="audio/mpeg"/></item>
            <item><guid>ep-2</guid><enclosure type="audio/mpeg" url='https://cdn.example/2.mp3'/></item>
        </channel></rss>"#;
        let f = FeedIdentity::of(body);
        assert_eq!(f.podcast_guid.as_deref(), Some("abc-123"));
        assert_eq!(f.new_feed_url.as_deref(), Some("https://new.example/feed?a=1&b=2"));
        assert_eq!(f.episode_guids, ["ep-1", "ep-2"]);
        assert_eq!(f.episode_urls, ["https://cdn.example/1.mp3", "https://cdn.example/2.mp3"]);
    }
}
