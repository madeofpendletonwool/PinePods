//! Higher-level steps most feature tests need: subscribe to a fixture feed, list episodes,
//! run the scheduler's refresh, wait for background work.

use std::future::Future;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::app::{TestApp, TestUser};

/// Poll `f` until it yields `Some`, failing after 20s with `what` in the message.
pub async fn eventually<T, F, Fut>(what: &str, mut f: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(v) = f().await {
            return v;
        }
        if Instant::now() > deadline {
            panic!("timed out waiting for: {what}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

impl TestApp {
    /// Subscribe `user` to the feed at `feed_url` through `/api/data/add_podcast`, then wait
    /// for the background import to land `expected_episodes` episodes. Returns the podcast id.
    pub async fn subscribe(&self, user: &TestUser, feed_url: &str, expected_episodes: usize) -> i32 {
        let resp = self
            .post("/api/data/add_podcast")
            .key(user)
            .json(&json!({
                "podcast_values": {
                    "pod_title": "client-supplied title",
                    "pod_artwork": "",
                    "pod_author": "",
                    "categories": {},
                    "pod_description": "",
                    "pod_episode_count": 0,
                    "pod_feed_url": feed_url,
                    "pod_website": "",
                    "pod_explicit": false,
                    "user_id": user.id,
                },
                "podcast_index_id": 0,
            }))
            .send()
            .await;
        let body = resp.ok_json();
        assert_eq!(body["success"], true, "add_podcast: {body}");
        let podcast_id = body["podcast_id"].as_i64().unwrap() as i32;
        eventually(&format!("{expected_episodes} episodes imported for podcast {podcast_id}"), || async {
            let n = self.episodes(user, podcast_id).await.len();
            (n == expected_episodes).then_some(())
        })
        .await;
        // Podcasting 2.0 tags are stored right after the episodes.
        eventually(&format!("feed tags stored for podcast {podcast_id}"), || async {
            self.feed_tags_stored(podcast_id).await.then_some(())
        })
        .await;
        // Scheduling the next refresh is the import's last write; a test moving the schedule
        // before it lands would be overwritten.
        let scheduled = format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE PodcastID = {podcast_id} AND NextRefreshAt IS NOT NULL"#);
        eventually(&format!("next refresh scheduled for podcast {podcast_id}"), || async {
            (super::sql::int(self, &scheduled).await == Some(1)).then_some(())
        })
        .await;
        podcast_id
    }

    /// True once refresh has stored the podcast's Podcasting 2.0 tags (even if it has none).
    pub async fn feed_tags_stored(&self, podcast_id: i32) -> bool {
        use pinepods_api::database::DatabasePool;
        let hash: Option<Option<String>> = match self.db() {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(r#"SELECT podcasting2hash FROM "Podcasts" WHERE podcastid = $1"#)
                .bind(podcast_id)
                .fetch_optional(pool)
                .await
                .unwrap(),
            DatabasePool::MySQL(pool) => sqlx::query_scalar("SELECT Podcasting2Hash FROM Podcasts WHERE PodcastID = ?")
                .bind(podcast_id)
                .fetch_optional(pool)
                .await
                .unwrap(),
        };
        hash.flatten().is_some()
    }

    /// All episodes of `podcast_id` as `user` sees them (`/api/data/podcast_episodes`).
    pub async fn episodes(&self, user: &TestUser, podcast_id: i32) -> Vec<Value> {
        let body = self
            .get("/api/data/podcast_episodes")
            .key(user)
            .query("user_id", user.id)
            .query("podcast_id", podcast_id)
            .query("limit", 1000)
            .send()
            .await
            .ok_json();
        body["episodes"].as_array().cloned().unwrap_or_default()
    }

    /// Episode id for the episode titled `title` in `podcast_id`.
    pub async fn episode_id(&self, user: &TestUser, podcast_id: i32, title: &str) -> i32 {
        let eps = self.episodes(user, podcast_id).await;
        eps.iter()
            .find(|e| e["Episodetitle"] == title)
            .unwrap_or_else(|| panic!("no episode {title:?} in {eps:#?}"))["Episodeid"]
            .as_i64()
            .unwrap() as i32
    }

    /// The user's subscriptions (`/api/data/return_pods/{id}`).
    pub async fn podcasts(&self, user: &TestUser) -> Vec<Value> {
        let body = self.get(&format!("/api/data/return_pods/{}", user.id)).key(user).send().await.ok_json();
        body["pods"].as_array().cloned().unwrap_or_default()
    }

    /// Run the scheduled feed refresh to completion (what the cron job does).
    pub async fn refresh(&self) {
        pinepods_api::handlers::refresh::refresh_pods_admin_internal(&self.state)
            .await
            .expect("refresh");
    }

    /// Serve a feed under this test's fixture namespace and return its URL.
    pub fn feed(&self, path: &str, feed: &super::rss::Feed) -> String {
        self.fixtures().serve_feed(path, feed.to_xml())
    }
}
