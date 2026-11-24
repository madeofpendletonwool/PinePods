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

                    // Counted from the library and history rather than the running tallies in
                    // UserStats, which miss podcasts and plays that arrived through sync.
                    let counted = q::query(r#"
                        SELECT
                            (SELECT COUNT(*) FROM "Podcasts" WHERE userid = $1 AND subscribed = TRUE)::bigint AS podcasts,
                            (SELECT COUNT(DISTINCT pid) FROM (
                                SELECT p.podcastid AS pid FROM "UserEpisodeHistory" h
                                  JOIN "Episodes" e ON h.episodeid = e.episodeid
                                  JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = h.userid
                                 WHERE h.userid = $1
                                UNION
                                SELECT v.podcastid AS pid FROM "UserVideoHistory" h
                                  JOIN "YouTubeVideos" v ON h.videoid = v.videoid
                                 WHERE h.userid = $1
                            ) played)::bigint AS played,
                            ((SELECT COALESCE(SUM(listenedseconds), 0) FROM "UserEpisodeHistory" WHERE userid = $1)
                             + (SELECT COALESCE(SUM(listenedseconds), 0) FROM "UserVideoHistory" WHERE userid = $1))::bigint AS seconds
                    "#).bind(user_id).fetch_one(pool).await?;
                    let (podcasts_added, podcasts_played, time_listened) = counted_stats(
                        counted.try_get("podcasts")?,
                        counted.try_get("played")?,
                        counted.try_get("seconds")?,
                        row.try_get::<i32, _>("timelistened")?,
                    );

                    let stats = serde_json::json!({
                        "UserCreated": row.try_get::<chrono::NaiveDateTime, _>("usercreated")?.format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
                        "PodcastsPlayed": podcasts_played,
                        "TimeListened": time_listened,
                        "PodcastsAdded": podcasts_added,
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

                    // Counted from the library and history; see the PostgreSQL branch.
                    let counted = sqlx::query(
                        "SELECT
                            CAST((SELECT COUNT(*) FROM Podcasts WHERE UserID = ? AND Subscribed = 1) AS SIGNED) AS podcasts,
                            CAST((SELECT COUNT(DISTINCT pid) FROM (
                                SELECT p.PodcastID AS pid FROM UserEpisodeHistory h
                                  JOIN Episodes e ON h.EpisodeID = e.EpisodeID
                                  JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = h.UserID
                                 WHERE h.UserID = ?
                                UNION
                                SELECT v.PodcastID AS pid FROM UserVideoHistory h
                                  JOIN YouTubeVideos v ON h.VideoID = v.VideoID
                                 WHERE h.UserID = ?
                            ) played) AS SIGNED) AS played,
                            CAST((SELECT COALESCE(SUM(ListenedSeconds), 0) FROM UserEpisodeHistory WHERE UserID = ?)
                                 + (SELECT COALESCE(SUM(ListenedSeconds), 0) FROM UserVideoHistory WHERE UserID = ?) AS SIGNED) AS seconds"
                    )
                    .bind(user_id).bind(user_id).bind(user_id).bind(user_id).bind(user_id)
                    .fetch_one(pool).await?;
                    let (podcasts_added, podcasts_played, time_listened) = counted_stats(
                        counted.try_get("podcasts")?,
                        counted.try_get("played")?,
                        counted.try_get("seconds")?,
                        row.try_get::<i32, _>("TimeListened")?,
                    );

                    let stats = serde_json::json!({
                        "UserCreated": row.try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>("UserCreated")?.format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
                        "PodcastsPlayed": podcasts_played,
                        "TimeListened": time_listened,
                        "PodcastsAdded": podcasts_added,
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

    /// Extended stats: the base insights plus listening activity and library numbers.
    pub async fn get_extended_stats(&self, user_id: i32) -> AppResult<serde_json::Value> {
        let mut stats = self.extended_stats_base(user_id).await?;
        let extra = self.listening_activity(user_id).await?;
        if let (Some(obj), serde_json::Value::Object(extra)) = (stats.as_object_mut(), extra) {
            obj.extend(extra);
        }
        Ok(stats)
    }

    /// Listening by local day and hour from `ListeningLog`, plus the per-episode history for the
    /// time before the log began, summarised by [`activity_summary`]; library counts; time saved.
    async fn listening_activity(&self, user_id: i32) -> AppResult<serde_json::Value> {
        let (tz, rows, logged, saved, library) = dialect_match!(self, {
            pg(pool) => {
                let tz: Option<String> = q::query(r#"SELECT timezone FROM "Users" WHERE userid = $1"#)
                    .bind(user_id).fetch_optional(pool).await?
                    .and_then(|r| r.try_get::<Option<String>, _>("timezone").ok().flatten());
                let rows: Vec<(chrono::NaiveDateTime, i64)> = q::query(r#"
                    SELECT listendate, CAST(COALESCE(listenedseconds, 0) AS BIGINT) AS secs
                    FROM "UserEpisodeHistory"
                    WHERE userid = $1 AND listendate IS NOT NULL
                    UNION ALL
                    SELECT listendate, CAST(COALESCE(listenedseconds, 0) AS BIGINT) AS secs
                    FROM "UserVideoHistory"
                    WHERE userid = $1 AND listendate IS NOT NULL
                "#).bind(user_id).fetch_all(pool).await?
                    .iter()
                    .filter_map(|r| Some((r.try_get::<chrono::NaiveDateTime, _>("listendate").ok()?, r.try_get::<i64, _>("secs").unwrap_or(0))))
                    .collect();
                let logged: Vec<(String, i64, i64)> = q::query(r#"
                    SELECT CAST(listenday AS TEXT) AS day, CAST(listenhour AS BIGINT) AS hour, CAST(SUM(seconds) AS BIGINT) AS secs
                    FROM "ListeningLog" WHERE userid = $1
                    GROUP BY listenday, listenhour
                    HAVING SUM(seconds) > 0
                "#).bind(user_id).fetch_all(pool).await?
                    .iter()
                    .filter_map(|r| Some((r.try_get("day").ok()?, r.try_get("hour").ok()?, r.try_get("secs").ok()?)))
                    .collect();
                let saved = q::query(r#"
                    SELECT CAST(COALESCE(SUM(speedsaved), 0) AS BIGINT) AS speed, CAST(COALESCE(SUM(silenceskipped), 0) AS BIGINT) AS silence,
                           CAST(COALESCE(SUM(adsskipped), 0) AS BIGINT) AS ads
                    FROM "ListeningLog" WHERE userid = $1
                "#).bind(user_id).fetch_one(pool).await?;
                let saved = (saved.try_get("speed").unwrap_or(0i64), saved.try_get("silence").unwrap_or(0i64), saved.try_get("ads").unwrap_or(0i64));
                let lib = q::query(r#"
                    SELECT
                        (SELECT COUNT(DISTINCT p.podcastid)
                           FROM "UserEpisodeHistory" h
                           JOIN "Episodes" e ON h.episodeid = e.episodeid
                           JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = h.userid
                          WHERE h.userid = $1)::bigint AS podcasts_listened,
                        (SELECT COUNT(*)
                           FROM "Episodes" e
                           JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = $1 AND p.subscribed = TRUE
                           LEFT JOIN "UserEpisodeHistory" h ON h.episodeid = e.episodeid AND h.userid = $1
                          WHERE h.episodeid IS NULL)::bigint AS backlog_episodes,
                        (SELECT COALESCE(SUM(e.episodeduration), 0)
                           FROM "Episodes" e
                           JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = $1 AND p.subscribed = TRUE
                           LEFT JOIN "UserEpisodeHistory" h ON h.episodeid = e.episodeid AND h.userid = $1
                          WHERE h.episodeid IS NULL)::bigint AS backlog_seconds
                "#).bind(user_id).fetch_one(pool).await?;
                let library = (
                    lib.try_get::<i64, _>("podcasts_listened").unwrap_or(0),
                    lib.try_get::<i64, _>("backlog_episodes").unwrap_or(0),
                    lib.try_get::<i64, _>("backlog_seconds").unwrap_or(0),
                );
                (tz, rows, logged, saved, library)
            },
            mysql(pool) => {
                let tz: Option<String> = sqlx::query("SELECT TimeZone FROM Users WHERE UserID = ?")
                    .bind(user_id).fetch_optional(pool).await?
                    .and_then(|r| r.try_get::<Option<String>, _>("TimeZone").ok().flatten());
                let rows: Vec<(chrono::NaiveDateTime, i64)> = sqlx::query(
                    "SELECT ListenDate AS listendate, CAST(COALESCE(ListenedSeconds, 0) AS SIGNED) AS secs
                     FROM UserEpisodeHistory
                     WHERE UserID = ? AND ListenDate IS NOT NULL
                     UNION ALL
                     SELECT ListenDate AS listendate, CAST(COALESCE(ListenedSeconds, 0) AS SIGNED) AS secs
                     FROM UserVideoHistory
                     WHERE UserID = ? AND ListenDate IS NOT NULL"
                ).bind(user_id).bind(user_id).fetch_all(pool).await?
                    .iter()
                    .filter_map(|r| {
                        let when = r.try_get::<chrono::NaiveDateTime, _>("listendate").ok().or_else(|| {
                            r.try_get::<chrono::DateTime<chrono::Utc>, _>("listendate").ok().map(|d| d.naive_utc())
                        })?;
                        Some((when, r.try_get::<i64, _>("secs").unwrap_or(0)))
                    })
                    .collect();
                let lib = sqlx::query(
                    "SELECT
                        CAST((SELECT COUNT(DISTINCT p.PodcastID)
                           FROM UserEpisodeHistory h
                           JOIN Episodes e ON h.EpisodeID = e.EpisodeID
                           JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = h.UserID
                          WHERE h.UserID = ?) AS SIGNED) AS podcasts_listened,
                        CAST((SELECT COUNT(*)
                           FROM Episodes e
                           JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = ? AND p.Subscribed = 1
                           LEFT JOIN UserEpisodeHistory h ON h.EpisodeID = e.EpisodeID AND h.UserID = ?
                          WHERE h.EpisodeID IS NULL) AS SIGNED) AS backlog_episodes,
                        CAST((SELECT COALESCE(SUM(e.EpisodeDuration), 0)
                           FROM Episodes e
                           JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = ? AND p.Subscribed = 1
                           LEFT JOIN UserEpisodeHistory h ON h.EpisodeID = e.EpisodeID AND h.UserID = ?
                          WHERE h.EpisodeID IS NULL) AS SIGNED) AS backlog_seconds"
                ).bind(user_id).bind(user_id).bind(user_id).bind(user_id).bind(user_id).fetch_one(pool).await?;
                let library = (
                    lib.try_get::<i64, _>("podcasts_listened").unwrap_or(0),
                    lib.try_get::<i64, _>("backlog_episodes").unwrap_or(0),
                    lib.try_get::<i64, _>("backlog_seconds").unwrap_or(0),
                );
                let logged: Vec<(String, i64, i64)> = sqlx::query(
                    "SELECT CAST(ListenDay AS CHAR) AS day, CAST(ListenHour AS SIGNED) AS hour, CAST(SUM(Seconds) AS SIGNED) AS secs
                     FROM ListeningLog WHERE UserID = ?
                     GROUP BY ListenDay, ListenHour
                     HAVING SUM(Seconds) > 0"
                ).bind(user_id).fetch_all(pool).await?
                    .iter()
                    .filter_map(|r| Some((r.try_get("day").ok()?, r.try_get("hour").ok()?, r.try_get("secs").ok()?)))
                    .collect();
                let saved = sqlx::query(
                    "SELECT CAST(COALESCE(SUM(SpeedSaved), 0) AS SIGNED) AS speed, CAST(COALESCE(SUM(SilenceSkipped), 0) AS SIGNED) AS silence,
                            CAST(COALESCE(SUM(AdsSkipped), 0) AS SIGNED) AS ads
                     FROM ListeningLog WHERE UserID = ?"
                ).bind(user_id).fetch_one(pool).await?;
                let saved = (saved.try_get("speed").unwrap_or(0i64), saved.try_get("silence").unwrap_or(0i64), saved.try_get("ads").unwrap_or(0i64));
                (tz, rows, logged, saved, library)
            },
        });
        let tz = tz.and_then(|t| t.parse::<Tz>().ok()).unwrap_or(Tz::UTC);
        let today = chrono::Utc::now().with_timezone(&tz).date_naive();
        let logged: Vec<Listened> = logged
            .into_iter()
            .filter_map(|(day, hour, secs)| Some((chrono::NaiveDate::parse_from_str(day.get(..10)?, "%Y-%m-%d").ok()?, hour.clamp(0, 23) as u32, secs)))
            .collect();
        let entries = combine_listening(&logged, &rows, tz);
        let mut summary = activity_summary(&entries, tz, today);
        if let Some(obj) = summary.as_object_mut() {
            obj.insert("time_saved".into(), serde_json::json!({ "speed": saved.0, "silence": saved.1, "ads": saved.2 }));
            obj.insert("podcasts_listened".into(), library.0.into());
            obj.insert("backlog_episodes".into(), library.1.into());
            obj.insert("backlog_seconds".into(), library.2.into());
        }
        Ok(summary)
    }

    async fn extended_stats_base(&self, user_id: i32) -> AppResult<serde_json::Value> {
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

        dialect_match!(self, {
            pg(pool) => {
                // Top 5 podcasts by listen time
                let podcast_rows = q::query(r#"
                    SELECT p.podcastid, p.podcastname, COALESCE(p.artworkurl, '') as artworkurl,
                           SUM(ueh.listenedseconds) AS total_seconds
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
                    SELECT p.categories, SUM(ueh.listenedseconds) AS total_seconds
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
                           SUM(listenedseconds) AS total_seconds
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
                    "episodes_completed": completed,
                    "episodes_listened": total,
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
                            SUM(ueh.ListenedSeconds) AS total_seconds
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
                    "SELECT p.Categories as categories, SUM(ueh.ListenedSeconds) AS total_seconds
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
                            SUM(ListenedSeconds) AS total_seconds
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
                    "episodes_completed": completed,
                    "episodes_listened": total,
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

/// (PodcastsAdded, PodcastsPlayed, TimeListened in minutes). Time listened keeps the stored
/// total when it is larger: it also holds time from episodes that have since been removed.
fn counted_stats(podcasts: i64, played: i64, history_seconds: i64, stored_minutes: i32) -> (i32, i32, i32) {
    let from_history = (history_seconds.max(0) / 60).min(i64::from(i32::MAX)) as i32;
    (podcasts.clamp(0, i64::from(i32::MAX)) as i32, played.clamp(0, i64::from(i32::MAX)) as i32, from_history.max(stored_minutes))
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

/// Days of history the heatmap covers.
const HEATMAP_DAYS: i64 = 182;

/// Seconds played on a local day, in an hour of it.
type Listened = (chrono::NaiveDate, u32, i64);

/// The log's entries, plus history rows (last listen time in UTC, seconds played) from before
/// the log's first day. History keeps one row per episode, dated its last play, so after the
/// log starts it would count an episode again, on one day, at one hour.
fn combine_listening(logged: &[Listened], history: &[(chrono::NaiveDateTime, i64)], tz: Tz) -> Vec<Listened> {
    use chrono::{TimeZone, Timelike};
    let log_start = logged.iter().map(|(d, _, _)| *d).min();
    let mut out = logged.to_vec();
    for (utc, secs) in history {
        let local = tz.from_utc_datetime(utc);
        if log_start.is_none_or(|start| local.date_naive() < start) {
            out.push((local.date_naive(), local.hour(), (*secs).max(0)));
        }
    }
    out
}

/// Listening patterns from (local day, hour, seconds played) entries.
fn activity_summary(entries: &[Listened], tz: Tz, today: chrono::NaiveDate) -> serde_json::Value {
    use chrono::{Datelike, Duration};
    let mut by_hour = [0i64; 24];
    let mut by_day: std::collections::BTreeMap<chrono::NaiveDate, i64> = std::collections::BTreeMap::new();
    // Average hour, weighted by time played (an entry with none still counts a little).
    let (mut hour_sum, mut weight) = (0f64, 0f64);
    for (day, hour, secs) in entries {
        let secs = (*secs).max(0);
        let w = secs.max(1) as f64;
        hour_sum += f64::from(*hour) * w;
        weight += w;
        by_hour[(*hour).min(23) as usize] += secs;
        *by_day.entry(*day).or_insert(0) += secs;
    }

    // Longest run of consecutive days with any listening.
    let (mut longest, mut run, mut prev) = (0i32, 0i32, None::<chrono::NaiveDate>);
    for day in by_day.keys() {
        run = if prev.and_then(|p| p.succ_opt()) == Some(*day) { run + 1 } else { 1 };
        longest = longest.max(run);
        prev = Some(*day);
    }

    // The run of days ending today, or yesterday when nothing was played yet today.
    let mut current = 0i32;
    let mut day = if by_day.contains_key(&today) { Some(today) } else { today.pred_opt() };
    while let Some(d) = day.filter(|d| by_day.contains_key(d)) {
        current += 1;
        day = d.pred_opt();
    }

    // Sunday first.
    let mut by_dow = [0i64; 7];
    for (day, secs) in &by_day {
        by_dow[day.weekday().num_days_from_sunday() as usize] += secs;
    }

    let heat_start = today - Duration::days(HEATMAP_DAYS - 1);
    let daily: Vec<serde_json::Value> = by_day
        .range(heat_start..=today)
        .filter(|(_, secs)| **secs > 0)
        .map(|(d, secs)| serde_json::json!({ "date": d.format("%Y-%m-%d").to_string(), "seconds": secs }))
        .collect();

    // The last 12 calendar months, oldest first.
    let month_index = |d: chrono::NaiveDate| d.year() * 12 + d.month0() as i32;
    let this_month = month_index(today);
    let mut months = [0i64; 12];
    for (day, secs) in &by_day {
        let back = this_month - month_index(*day);
        if (0..12).contains(&back) {
            months[11 - back as usize] += secs;
        }
    }
    let by_month: Vec<serde_json::Value> = (0..12)
        .map(|i| {
            let idx = this_month - 11 + i as i32;
            serde_json::json!({
                "month": format!("{:04}-{:02}", idx.div_euclid(12), idx.rem_euclid(12) + 1),
                "seconds": months[i],
            })
        })
        .collect();

    let year_seconds: i64 = by_day.iter().filter(|(d, _)| d.year() == today.year()).map(|(_, s)| s).sum();
    let peak_hour = by_hour.iter().enumerate().filter(|(_, s)| **s > 0).max_by_key(|(_, s)| **s).map(|(h, _)| h as i32);

    serde_json::json!({
        "listening_by_hour": by_hour,
        "listening_by_month": by_month,
        "daily_listening": daily,
        "active_days": by_day.values().filter(|s| **s > 0).count(),
        "longest_streak": longest,
        // Replace the base values, which used UTC days.
        "current_streak": current,
        "listening_by_dow": by_dow,
        "year_seconds": year_seconds,
        "peak_hour": peak_hour,
        // Replaces the base value, which averaged UTC hours.
        "listening_badge": badge_from_avg_hour((weight > 0.0).then(|| hour_sum / weight)),
        "timezone": tz.name(),
    })
}

#[cfg(test)]
mod activity_tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(y: i32, m: u32, d: u32, h: u32) -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }

    #[test]
    fn buckets_in_local_time() {
        // 02:00 UTC on the 2nd is 21:00 on the 1st in New York.
        let rows = vec![(at(2026, 11, 2, 2), 600)];
        let tz: Tz = "America/New_York".parse().unwrap();
        let v = activity_summary(&combine_listening(&[], &rows, tz), tz, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        assert_eq!(v["listening_by_hour"][21], 600);
        assert_eq!(v["daily_listening"][0]["date"], "2026-11-01");
        assert_eq!(v["peak_hour"], 21);
        assert_eq!(v["listening_badge"], "evening_listener");
    }

    #[test]
    fn streaks_months_and_window() {
        let rows = vec![
            (at(2026, 11, 1, 12), 60),
            (at(2026, 11, 2, 12), 60),
            (at(2026, 11, 3, 12), 60),
            (at(2026, 11, 10, 12), 60),
            (at(2025, 12, 5, 12), 120),
            (at(2024, 1, 1, 12), 999),
        ];
        let v = activity_summary(&combine_listening(&[], &rows, Tz::UTC), Tz::UTC, NaiveDate::from_ymd_opt(2026, 11, 20).unwrap());
        assert_eq!(v["longest_streak"], 3);
        assert_eq!(v["current_streak"], 0, "nothing since the 10th");
        assert_eq!(v["listening_by_dow"][2], 120, "Tuesdays: 3 and 10 November");
        assert_eq!(v["active_days"], 6);
        assert_eq!(v["year_seconds"], 240);
        let months = v["listening_by_month"].as_array().unwrap();
        assert_eq!(months.len(), 12);
        assert_eq!(months[0]["month"], "2025-12");
        assert_eq!(months[0]["seconds"], 120);
        assert_eq!(months[11]["month"], "2026-11");
        assert_eq!(months[11]["seconds"], 240);
        // Only the last 182 days make the heatmap.
        assert_eq!(v["daily_listening"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn the_log_replaces_history_from_its_first_day() {
        let day = |d| NaiveDate::from_ymd_opt(2026, 11, d).unwrap();
        let logged = vec![(day(10), 8, 300), (day(11), 20, 200)];
        // Before the log: kept. On or after its first day: the log has it already.
        let history = vec![(at(2026, 11, 5, 12), 900), (at(2026, 11, 11, 20), 5000)];
        let entries = combine_listening(&logged, &history, Tz::UTC);
        assert_eq!(entries.len(), 3);
        let v = activity_summary(&entries, Tz::UTC, day(12));
        assert_eq!(v["current_streak"], 2, "the 10th and 11th, nothing yet on the 12th");
        assert_eq!(v["listening_by_hour"][20], 200);
        assert_eq!(v["listening_by_hour"][12], 900);
        assert_eq!(v["active_days"], 3);
        assert_eq!(v["year_seconds"], 1400);
        // History in another zone is compared on its local day.
        let ny: Tz = "America/New_York".parse().unwrap();
        let late = vec![(at(2026, 11, 10, 3), 60)]; // 22:00 on the 9th in New York
        assert_eq!(combine_listening(&logged, &late, ny).len(), 3);
    }

    #[test]
    fn counted_stats_prefer_the_larger_time() {
        assert_eq!(counted_stats(6, 3, 600, 4), (6, 3, 10));
        assert_eq!(counted_stats(6, 3, 60, 90), (6, 3, 90));
    }

    #[test]
    fn empty_history() {
        let v = activity_summary(&[], Tz::UTC, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(v["longest_streak"], 0);
        assert!(v["peak_hour"].is_null());
    }
}
