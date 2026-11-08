//! Recommendation cache.

use super::*;

impl DatabasePool {
    // Return the cached recommendations JSON for a user if present and newer than
    // max_age_hours; otherwise None (signals the caller to regenerate). See migration 058.
    pub async fn get_recommendation_cache(&self, user_id: i32, max_age_hours: i32) -> AppResult<Option<String>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"
                    SELECT resultsjson FROM "RecommendationCache"
                    WHERE userid = $1 AND generatedat > CURRENT_TIMESTAMP - make_interval(hours => $2)
                "#)
                .bind(user_id)
                .bind(max_age_hours)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(|r| r.try_get::<String, _>("resultsjson").unwrap_or_default()))
            },
            mysql(pool) => {
                let row = sqlx::query(r#"
                    SELECT ResultsJSON FROM RecommendationCache
                    WHERE UserID = ? AND GeneratedAt > (NOW() - INTERVAL ? HOUR)
                "#)
                .bind(user_id)
                .bind(max_age_hours)
                .fetch_optional(pool)
                .await?;
                Ok(row.map(|r| r.try_get::<String, _>("ResultsJSON").unwrap_or_default()))
            },
        })
    }

    // Upsert a user's recommendations JSON, stamping GeneratedAt to now. One row per user.
    pub async fn upsert_recommendation_cache(&self, user_id: i32, results_json: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"
                    INSERT INTO "RecommendationCache" (userid, generatedat, resultsjson)
                    VALUES ($1, CURRENT_TIMESTAMP, $2)
                    ON CONFLICT (userid) DO UPDATE SET generatedat = CURRENT_TIMESTAMP, resultsjson = EXCLUDED.resultsjson
                "#)
                .bind(user_id)
                .bind(results_json)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                sqlx::query(r#"
                    INSERT INTO RecommendationCache (UserID, GeneratedAt, ResultsJSON)
                    VALUES (?, NOW(), ?)
                    ON DUPLICATE KEY UPDATE GeneratedAt = NOW(), ResultsJSON = VALUES(ResultsJSON)
                "#)
                .bind(user_id)
                .bind(results_json)
                .execute(pool)
                .await?;
            },
        });
        Ok(())
    }

    // User IDs that already have a cached recommendation set (i.e. have used the Discover
    // page). The scheduler refreshes these nightly so return visits are instant; brand-new
    // users are handled on demand by the endpoint.
    pub async fn get_recommendation_cache_user_ids(&self) -> AppResult<Vec<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(r#"SELECT userid FROM "RecommendationCache""#)
                    .fetch_all(pool)
                    .await?;
                Ok(rows.iter().filter_map(|r| r.try_get::<i32, _>("userid").ok()).collect())
            },
            mysql(pool) => {
                let rows = sqlx::query("SELECT UserID FROM RecommendationCache")
                    .fetch_all(pool)
                    .await?;
                Ok(rows.iter().filter_map(|r| r.try_get::<i32, _>("UserID").ok()).collect())
            },
        })
    }

    // Lean per-subscription inputs for the recommendation taste profile (#103). Only touches
    // columns guaranteed to exist on Podcasts, unlike return_pods_extra (which references a
    // non-existent p.isyoutube column). play_count = number of listen-history rows for the sub.
    pub async fn get_recommendation_taste_inputs(
        &self,
        user_id: i32,
    ) -> AppResult<Vec<crate::models::RecommendationTasteInput>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(r#"
                    SELECT p.podcastname, p.author, p.description, p.categories,
                           p.podcastindexid, p.feedurl,
                           COALESCE(p.isfavorite, false) AS isfavorite,
                           COUNT(ueh.userepisodehistoryid) AS play_count
                    FROM "Podcasts" p
                    LEFT JOIN "Episodes" e ON p.feedid = e.feedid
                    LEFT JOIN "UserEpisodeHistory" ueh ON e.episodeid = ueh.episodeid AND ueh.userid = $1
                    WHERE p.userid = $1 AND COALESCE(p.displaypodcast, TRUE) = TRUE
                    GROUP BY p.podcastid, p.podcastname, p.author, p.description, p.categories,
                             p.podcastindexid, p.feedurl, p.isfavorite
                "#)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                Ok(rows.iter().map(|row| {
                    let cats: Option<String> = row.try_get("categories").ok();
                    crate::models::RecommendationTasteInput {
                        podcastname: row.try_get::<Option<String>, _>("podcastname").ok().flatten().unwrap_or_default(),
                        author: row.try_get::<Option<String>, _>("author").ok().flatten(),
                        description: row.try_get::<Option<String>, _>("description").ok().flatten(),
                        categories: cats.and_then(|c| self.parse_categories_json(&c)),
                        podcastindexid: row.try_get::<Option<i32>, _>("podcastindexid").ok().flatten().map(|v| v as i64),
                        feedurl: row.try_get::<Option<String>, _>("feedurl").ok().flatten().unwrap_or_default(),
                        is_favorite: row.try_get::<bool, _>("isfavorite").unwrap_or(false),
                        play_count: row.try_get::<i64, _>("play_count").unwrap_or(0),
                    }
                }).collect())
            },
            mysql(pool) => {
                let rows = sqlx::query(r#"
                    SELECT p.PodcastName, p.Author, p.Description, p.Categories,
                           p.PodcastIndexID, p.FeedURL,
                           COALESCE(p.IsFavorite, false) AS isfavorite,
                           COUNT(ueh.UserEpisodeHistoryID) AS play_count
                    FROM Podcasts p
                    LEFT JOIN Episodes e ON p.FeedID = e.FeedID
                    LEFT JOIN UserEpisodeHistory ueh ON e.EpisodeID = ueh.EpisodeID AND ueh.UserID = ?
                    WHERE p.UserID = ? AND COALESCE(p.DisplayPodcast, TRUE) = TRUE
                    GROUP BY p.PodcastID, p.PodcastName, p.Author, p.Description, p.Categories,
                             p.PodcastIndexID, p.FeedURL, p.IsFavorite
                "#)
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                Ok(rows.iter().map(|row| {
                    let cats: Option<String> = row.try_get("Categories").ok();
                    crate::models::RecommendationTasteInput {
                        podcastname: row.try_get::<Option<String>, _>("PodcastName").ok().flatten().unwrap_or_default(),
                        author: row.try_get::<Option<String>, _>("Author").ok().flatten(),
                        description: row.try_get::<Option<String>, _>("Description").ok().flatten(),
                        categories: cats.and_then(|c| self.parse_categories_json(&c)),
                        podcastindexid: row.try_get::<Option<i32>, _>("PodcastIndexID").ok().flatten().map(|v| v as i64),
                        feedurl: row.try_get::<Option<String>, _>("FeedURL").ok().flatten().unwrap_or_default(),
                        is_favorite: row.try_get::<bool, _>("isfavorite").unwrap_or(false),
                        play_count: row.try_get::<i64, _>("play_count").unwrap_or(0),
                    }
                }).collect())
            },
        })
    }
}
