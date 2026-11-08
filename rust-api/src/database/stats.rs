//! User statistics.

use super::*;

impl DatabasePool {
    // Get user stats - matches Python get_stats function
    pub async fn get_stats(&self, user_id: i32) -> AppResult<Option<serde_json::Value>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT usercreated, podcastsplayed, timelistened, podcastsadded 
                       FROM "UserStats" WHERE userid = $1"#
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    // Count saved episodes directly from SavedEpisodes table
                    let saved_count_row = q::query(
                        r#"SELECT COUNT(*) as saved_count FROM "SavedEpisodes" WHERE userid = $1"#
                    )
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;

                    // Count downloaded episodes directly from DownloadedEpisodes table  
                    let downloaded_count_row = q::query(
                        r#"SELECT COUNT(*) as downloaded_count FROM "DownloadedEpisodes" WHERE userid = $1"#
                    )
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;

                    let saved_count: i64 = saved_count_row.try_get("saved_count")?;
                    let downloaded_count: i64 = downloaded_count_row.try_get("downloaded_count")?;

                    let stats = serde_json::json!({
                        "UserCreated": row.try_get::<chrono::NaiveDateTime, _>("usercreated")?.format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
                        "PodcastsPlayed": row.try_get::<i32, _>("podcastsplayed")?,
                        "TimeListened": row.try_get::<i32, _>("timelistened")?,
                        "PodcastsAdded": row.try_get::<i32, _>("podcastsadded")?,
                        "EpisodesSaved": saved_count as i32,
                        "EpisodesDownloaded": downloaded_count as i32
                    });

                    Ok(Some(stats))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query(
                    "SELECT UserCreated, PodcastsPlayed, TimeListened, PodcastsAdded 
                     FROM UserStats WHERE UserID = ?"
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {

                    // Count saved episodes directly from SavedEpisodes table
                    let saved_count_row = sqlx::query(
                        "SELECT COUNT(*) as saved_count FROM SavedEpisodes WHERE UserID = ?"
                    )
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;

                    // Count downloaded episodes directly from DownloadedEpisodes table
                    let downloaded_count_row = sqlx::query(
                        "SELECT COUNT(*) as downloaded_count FROM DownloadedEpisodes WHERE UserID = ?"
                    )
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;

                    let saved_count: i64 = saved_count_row.try_get("saved_count")?;
                    let downloaded_count: i64 = downloaded_count_row.try_get("downloaded_count")?;

                    let stats = serde_json::json!({
                        "UserCreated": row.try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>("UserCreated")?.format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
                        "PodcastsPlayed": row.try_get::<i32, _>("PodcastsPlayed")?,
                        "TimeListened": row.try_get::<i32, _>("TimeListened")?,
                        "PodcastsAdded": row.try_get::<i32, _>("PodcastsAdded")?,
                        "EpisodesSaved": saved_count as i32,
                        "EpisodesDownloaded": downloaded_count as i32
                    });

                    Ok(Some(stats))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get extended user stats with rich listening insights
    pub async fn get_extended_stats(&self, user_id: i32) -> AppResult<serde_json::Value> {
        fn compute_streak(days: &[chrono::NaiveDate]) -> i32 {
            if days.is_empty() {
                return 0;
            }
            let today = chrono::Utc::now().date_naive();
            let yesterday = today.pred_opt().unwrap_or(today);
            let start = days[0];
            if start != today && start != yesterday {
                return 0;
            }
            let mut streak = 0i32;
            let mut expected = start;
            for day in days {
                if *day == expected {
                    streak += 1;
                    match expected.pred_opt() {
                        Some(prev) => expected = prev,
                        None => break,
                    }
                } else if *day < expected {
                    break;
                }
            }
            streak
        }

        fn format_bytes(bytes: i64) -> String {
            if bytes >= 1_073_741_824 {
                format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
            } else if bytes >= 1_048_576 {
                format!("{:.1} MB", bytes as f64 / 1_048_576.0)
            } else if bytes >= 1024 {
                format!("{} KB", bytes / 1024)
            } else {
                format!("{} B", bytes)
            }
        }

        fn badge_from_avg_hour(avg_hour: Option<f64>) -> &'static str {
            match avg_hour {
                None => "no_data",
                Some(h) if h < 6.0 || h >= 22.0 => "night_owl",
                Some(h) if h >= 18.0 => "evening_listener",
                Some(h) if h >= 12.0 => "afternoon_listener",
                _ => "morning_listener",
            }
        }

        dialect_match!(self, {
            pg(pool) => {
                // Top 5 podcasts by listen time
                let podcast_rows = q::query(r#"
                    SELECT p.podcastid, p.podcastname, COALESCE(p.artworkurl, '') as artworkurl,
                           SUM(ueh.listenduration) AS total_seconds
                    FROM "UserEpisodeHistory" ueh
                    INNER JOIN "Episodes" e ON ueh.episodeid = e.episodeid
                    INNER JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = ueh.userid
                    WHERE ueh.userid = $1
                    GROUP BY p.podcastid, p.podcastname, p.artworkurl
                    ORDER BY total_seconds DESC LIMIT 5
                "#).bind(user_id).fetch_all(pool).await?;

                let top_podcasts: Vec<serde_json::Value> = podcast_rows.iter().map(|row| {
                    serde_json::json!({
                        "podcastid": row.try_get::<i32, _>("podcastid").unwrap_or(0),
                        "podcastname": row.try_get::<String, _>("podcastname").unwrap_or_default(),
                        "artworkurl": row.try_get::<String, _>("artworkurl").unwrap_or_default(),
                        "total_seconds": row.try_get::<i64, _>("total_seconds").unwrap_or(0)
                    })
                }).collect();

                // Categories with listen time (parse in Rust since format varies)
                let cat_rows = q::query(r#"
                    SELECT p.categories, SUM(ueh.listenduration) AS total_seconds
                    FROM "UserEpisodeHistory" ueh
                    INNER JOIN "Episodes" e ON ueh.episodeid = e.episodeid
                    INNER JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = ueh.userid
                    WHERE ueh.userid = $1 AND p.categories IS NOT NULL AND p.categories != ''
                    GROUP BY p.podcastid, p.categories
                "#).bind(user_id).fetch_all(pool).await?;

                let mut category_map: HashMap<String, i64> = HashMap::new();
                for row in &cat_rows {
                    let cats_str: String = row.try_get("categories").unwrap_or_default();
                    let secs: i64 = row.try_get("total_seconds").unwrap_or(0);
                    if let Some(parsed) = self.parse_categories_json(&cats_str) {
                        for (_, cat_name) in parsed {
                            if !cat_name.is_empty() {
                                *category_map.entry(cat_name).or_insert(0) += secs;
                            }
                        }
                    }
                }
                let mut cats_sorted: Vec<(String, i64)> = category_map.into_iter().collect();
                cats_sorted.sort_by(|a, b| b.1.cmp(&a.1));
                cats_sorted.truncate(5);
                let favorite_categories: Vec<serde_json::Value> = cats_sorted.into_iter()
                    .map(|(name, secs)| serde_json::json!({"name": name, "total_seconds": secs}))
                    .collect();

                // Episode completion rate
                let comp_row = q::query(r#"
                    SELECT COUNT(*) AS total,
                           COUNT(CASE WHEN ueh.listenduration >= e.episodeduration * 0.9
                                       AND e.episodeduration > 0 THEN 1 END) AS completed
                    FROM "UserEpisodeHistory" ueh
                    INNER JOIN "Episodes" e ON ueh.episodeid = e.episodeid
                    WHERE ueh.userid = $1
                "#).bind(user_id).fetch_one(pool).await?;

                let total: i64 = comp_row.try_get("total").unwrap_or(0);
                let completed: i64 = comp_row.try_get("completed").unwrap_or(0);
                let completion_rate = if total > 0 {
                    ((completed as f64 / total as f64) * 100.0).round() as i32
                } else { 0 };

                // Longest episode listened
                let longest_row = q::query(r#"
                    SELECT e.episodeid, e.episodetitle, e.episodeduration, p.podcastname
                    FROM "UserEpisodeHistory" ueh
                    INNER JOIN "Episodes" e ON ueh.episodeid = e.episodeid
                    INNER JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = ueh.userid
                    WHERE ueh.userid = $1 AND e.episodeduration IS NOT NULL AND e.episodeduration > 0
                    ORDER BY e.episodeduration DESC LIMIT 1
                "#).bind(user_id).fetch_optional(pool).await?;

                let longest_episode = longest_row.as_ref().map(|row| serde_json::json!({
                    "episodeid": row.try_get::<i32, _>("episodeid").unwrap_or(0),
                    "episodetitle": row.try_get::<String, _>("episodetitle").unwrap_or_default(),
                    "episodeduration": row.try_get::<i32, _>("episodeduration").unwrap_or(0),
                    "podcastname": row.try_get::<String, _>("podcastname").unwrap_or_default()
                }));

                // Listening personality badge (avg hour of day)
                let personality_row = q::query(r#"
                    SELECT AVG(EXTRACT(HOUR FROM listendate))::float8 AS avg_hour
                    FROM "UserEpisodeHistory"
                    WHERE userid = $1 AND listendate IS NOT NULL
                "#).bind(user_id).fetch_one(pool).await?;

                let avg_hour: Option<f64> = personality_row.try_get("avg_hour").unwrap_or(None);
                let listening_badge = badge_from_avg_hour(avg_hour);

                // Total data downloaded
                let dl_row = q::query(r#"
                    SELECT COALESCE(SUM(downloadedsize), 0)::BIGINT AS total_bytes
                    FROM "DownloadedEpisodes" WHERE userid = $1
                "#).bind(user_id).fetch_one(pool).await?;

                let total_bytes: i64 = dl_row.try_get("total_bytes").unwrap_or(0);

                // Current streak (distinct calendar days, sorted DESC)
                let day_rows = q::query(r#"
                    SELECT DISTINCT DATE(listendate) AS listen_day
                    FROM "UserEpisodeHistory"
                    WHERE userid = $1 AND listendate IS NOT NULL
                    ORDER BY listen_day DESC
                "#).bind(user_id).fetch_all(pool).await?;

                let days: Vec<chrono::NaiveDate> = day_rows.iter()
                    .filter_map(|row| row.try_get::<chrono::NaiveDate, _>("listen_day").ok())
                    .collect();
                let current_streak = compute_streak(&days);

                // Listening by day of week
                // Cast to int because PostgreSQL 14+ changed EXTRACT() return type from
                // float8 to numeric, which would silently fail try_get::<i64> and map
                // all rows to Sunday (index 0).
                let dow_rows = q::query(r#"
                    SELECT EXTRACT(DOW FROM listendate)::int AS dow,
                           SUM(listenduration) AS total_seconds
                    FROM "UserEpisodeHistory"
                    WHERE userid = $1 AND listendate IS NOT NULL
                    GROUP BY EXTRACT(DOW FROM listendate) ORDER BY dow
                "#).bind(user_id).fetch_all(pool).await?;

                let mut dow_data = [0i64; 7];
                for row in &dow_rows {
                    let dow: i32 = row.try_get("dow").unwrap_or(0);
                    let secs: i64 = row.try_get("total_seconds").unwrap_or(0);
                    let idx = dow as usize;
                    if idx < 7 { dow_data[idx] = secs; }
                }

                Ok(serde_json::json!({
                    "top_podcasts": top_podcasts,
                    "favorite_categories": favorite_categories,
                    "completion_rate": completion_rate,
                    "longest_episode": longest_episode,
                    "listening_badge": listening_badge,
                    "total_downloaded_bytes": total_bytes,
                    "total_downloaded_formatted": format_bytes(total_bytes),
                    "current_streak": current_streak,
                    "listening_by_dow": dow_data
                }))
            },
            mysql(pool) => {
                // Top 5 podcasts by listen time
                let podcast_rows = sqlx::query(
                    "SELECT p.PodcastID as podcastid, p.PodcastName as podcastname,
                            COALESCE(p.ArtworkURL, '') as artworkurl,
                            SUM(ueh.ListenDuration) AS total_seconds
                     FROM UserEpisodeHistory ueh
                     INNER JOIN Episodes e ON ueh.EpisodeID = e.EpisodeID
                     INNER JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = ueh.UserID
                     WHERE ueh.UserID = ?
                     GROUP BY p.PodcastID, p.PodcastName, p.ArtworkURL
                     ORDER BY total_seconds DESC LIMIT 5"
                ).bind(user_id).fetch_all(pool).await?;

                let top_podcasts: Vec<serde_json::Value> = podcast_rows.iter().map(|row| {
                    serde_json::json!({
                        "podcastid": row.try_get::<i32, _>("podcastid").unwrap_or(0),
                        "podcastname": row.try_get::<String, _>("podcastname").unwrap_or_default(),
                        "artworkurl": row.try_get::<String, _>("artworkurl").unwrap_or_default(),
                        "total_seconds": row.try_get::<i64, _>("total_seconds").unwrap_or(0)
                    })
                }).collect();

                // Categories
                let cat_rows = sqlx::query(
                    "SELECT p.Categories as categories, SUM(ueh.ListenDuration) AS total_seconds
                     FROM UserEpisodeHistory ueh
                     INNER JOIN Episodes e ON ueh.EpisodeID = e.EpisodeID
                     INNER JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = ueh.UserID
                     WHERE ueh.UserID = ? AND p.Categories IS NOT NULL AND p.Categories != ''
                     GROUP BY p.PodcastID, p.Categories"
                ).bind(user_id).fetch_all(pool).await?;

                let mut category_map: HashMap<String, i64> = HashMap::new();
                for row in &cat_rows {
                    let cats_str: String = row.try_get("categories").unwrap_or_default();
                    let secs: i64 = row.try_get("total_seconds").unwrap_or(0);
                    if let Some(parsed) = self.parse_categories_json(&cats_str) {
                        for (_, cat_name) in parsed {
                            if !cat_name.is_empty() {
                                *category_map.entry(cat_name).or_insert(0) += secs;
                            }
                        }
                    }
                }
                let mut cats_sorted: Vec<(String, i64)> = category_map.into_iter().collect();
                cats_sorted.sort_by(|a, b| b.1.cmp(&a.1));
                cats_sorted.truncate(5);
                let favorite_categories: Vec<serde_json::Value> = cats_sorted.into_iter()
                    .map(|(name, secs)| serde_json::json!({"name": name, "total_seconds": secs}))
                    .collect();

                // Completion rate
                let comp_row = sqlx::query(
                    "SELECT COUNT(*) AS total,
                            COUNT(CASE WHEN ueh.ListenDuration >= e.EpisodeDuration * 0.9
                                        AND e.EpisodeDuration > 0 THEN 1 END) AS completed
                     FROM UserEpisodeHistory ueh
                     INNER JOIN Episodes e ON ueh.EpisodeID = e.EpisodeID
                     WHERE ueh.UserID = ?"
                ).bind(user_id).fetch_one(pool).await?;

                let total: i64 = comp_row.try_get("total").unwrap_or(0);
                let completed: i64 = comp_row.try_get("completed").unwrap_or(0);
                let completion_rate = if total > 0 {
                    ((completed as f64 / total as f64) * 100.0).round() as i32
                } else { 0 };

                // Longest episode
                let longest_row = sqlx::query(
                    "SELECT e.EpisodeID as episodeid, e.EpisodeTitle as episodetitle,
                            e.EpisodeDuration as episodeduration, p.PodcastName as podcastname
                     FROM UserEpisodeHistory ueh
                     INNER JOIN Episodes e ON ueh.EpisodeID = e.EpisodeID
                     INNER JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = ueh.UserID
                     WHERE ueh.UserID = ? AND e.EpisodeDuration IS NOT NULL AND e.EpisodeDuration > 0
                     ORDER BY e.EpisodeDuration DESC LIMIT 1"
                ).bind(user_id).fetch_optional(pool).await?;

                let longest_episode = longest_row.as_ref().map(|row| serde_json::json!({
                    "episodeid": row.try_get::<i32, _>("episodeid").unwrap_or(0),
                    "episodetitle": row.try_get::<String, _>("episodetitle").unwrap_or_default(),
                    "episodeduration": row.try_get::<i32, _>("episodeduration").unwrap_or(0),
                    "podcastname": row.try_get::<String, _>("podcastname").unwrap_or_default()
                }));

                // Listening personality badge via SUM/COUNT to avoid DECIMAL type
                let personality_row = sqlx::query(
                    "SELECT COALESCE(SUM(HOUR(ListenDate)), 0) AS hour_sum, COUNT(*) AS row_count
                     FROM UserEpisodeHistory
                     WHERE UserID = ? AND ListenDate IS NOT NULL"
                ).bind(user_id).fetch_one(pool).await?;

                let hour_sum: i64 = personality_row.try_get("hour_sum").unwrap_or(0);
                let row_count: i64 = personality_row.try_get("row_count").unwrap_or(0);
                let avg_hour_f64: Option<f64> = if row_count > 0 {
                    Some(hour_sum as f64 / row_count as f64)
                } else { None };
                let listening_badge = badge_from_avg_hour(avg_hour_f64);

                // Total downloaded
                let dl_row = sqlx::query(
                    "SELECT CAST(COALESCE(SUM(DownloadedSize), 0) AS SIGNED) AS total_bytes
                     FROM DownloadedEpisodes WHERE UserID = ?"
                ).bind(user_id).fetch_one(pool).await?;

                let total_bytes: i64 = dl_row.try_get("total_bytes").unwrap_or(0);

                // Streak
                let day_rows = sqlx::query(
                    "SELECT DISTINCT DATE(ListenDate) AS listen_day
                     FROM UserEpisodeHistory
                     WHERE UserID = ? AND ListenDate IS NOT NULL
                     ORDER BY listen_day DESC"
                ).bind(user_id).fetch_all(pool).await?;

                let days: Vec<chrono::NaiveDate> = day_rows.iter()
                    .filter_map(|row| row.try_get::<chrono::NaiveDate, _>("listen_day").ok())
                    .collect();
                let current_streak = compute_streak(&days);

                // DOW: MySQL DAYOFWEEK returns 1=Sun..7=Sat, subtract 1 for 0-indexed
                let dow_rows = sqlx::query(
                    "SELECT DAYOFWEEK(ListenDate) - 1 AS dow,
                            SUM(ListenDuration) AS total_seconds
                     FROM UserEpisodeHistory
                     WHERE UserID = ? AND ListenDate IS NOT NULL
                     GROUP BY DAYOFWEEK(ListenDate) ORDER BY dow"
                ).bind(user_id).fetch_all(pool).await?;

                let mut dow_data = [0i64; 7];
                for row in &dow_rows {
                    let dow: i32 = row.try_get("dow").unwrap_or(0);
                    let secs: i64 = row.try_get("total_seconds").unwrap_or(0);
                    let idx = dow as usize;
                    if idx < 7 { dow_data[idx] = secs; }
                }

                Ok(serde_json::json!({
                    "top_podcasts": top_podcasts,
                    "favorite_categories": favorite_categories,
                    "completion_rate": completion_rate,
                    "longest_episode": longest_episode,
                    "listening_badge": listening_badge,
                    "total_downloaded_bytes": total_bytes,
                    "total_downloaded_formatted": format_bytes(total_bytes),
                    "current_streak": current_streak,
                    "listening_by_dow": dow_data
                }))
            },
        })
    }
}
