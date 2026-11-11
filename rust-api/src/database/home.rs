//! Home overview and search.

use super::*;

impl DatabasePool {
    // Search data - matches Python search_data function (simplified version)
    pub async fn search_data(&self, search_term: &str, user_id: i32, categories: &[String], limit: i64, offset: i64, filter: &str) -> AppResult<(Vec<serde_json::Value>, i64)> {
        dialect_match!(self, {
            pg(pool) => {
                let cat_clause = if categories.is_empty() {
                    String::new()
                } else {
                    let conds: Vec<String> = (0..categories.len())
                        .map(|i| format!("LOWER(p.categories) LIKE LOWER(${})", i + 5))
                        .collect();
                    format!("AND ({})", conds.join(" OR "))
                };
                let filter_clause = match filter {
                    "unplayed"    => " WHERE listenduration = 0 AND completed = false",
                    "in_progress" => " WHERE listenduration > 0 AND completed = false",
                    "saved"       => " WHERE saved = true",
                    "downloaded"  => " WHERE downloaded = true",
                    _             => "",
                };

                let sql = format!(
                    r#"SELECT *, COUNT(*) OVER() AS total_count FROM (
                        SELECT
                            p.podcastid,
                            p.podcastname,
                            p.artworkurl,
                            p.author,
                            p.categories,
                            p.description,
                            p.episodecount,
                            p.feedurl,
                            p.websiteurl,
                            p.explicit,
                            p.userid,
                            COALESCE(p.isyoutubechannel, false) as is_youtube,
                            e.episodeid,
                            e.episodetitle,
                            e.episodedescription,
                            e.episodeurl,
                            CASE
                                WHEN p.usepodcastcoverscustomized = TRUE AND p.usepodcastcovers = TRUE THEN p.artworkurl
                                WHEN u.usepodcastcovers = TRUE THEN p.artworkurl
                                ELSE e.episodeartwork
                            END as episodeartwork,
                            e.episodepubdate,
                            e.episodeduration,
                            COALESCE(h.listenduration, 0) as listenduration,
                            COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) as completed,
                            CASE WHEN se.episodeid IS NOT NULL THEN true ELSE false END as saved,
                            CASE WHEN eq.episodeid IS NOT NULL THEN true ELSE false END as queued,
                            CASE WHEN de.episodeid IS NOT NULL THEN true ELSE false END as downloaded
                        FROM "Podcasts" p
                        LEFT JOIN "Users" u ON p.userid = u.userid
                        LEFT JOIN "Episodes" e ON p.feedid = e.feedid
                        LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = $2
                        LEFT JOIN "SavedEpisodes" se ON e.episodeid = se.episodeid AND se.userid = $2
                        LEFT JOIN "EpisodeQueue" eq ON e.episodeid = eq.episodeid AND eq.userid = $2 AND eq.is_youtube = false
                        LEFT JOIN "DownloadedEpisodes" de ON e.episodeid = de.episodeid AND de.userid = $2
                        WHERE p.userid = $2
                          AND e.episodeid IS NOT NULL
                          AND (LOWER(p.podcastname) LIKE LOWER($1)
                               OR LOWER(e.episodetitle) LIKE LOWER($1)
                               OR LOWER(e.episodedescription) LIKE LOWER($1))
                          {}
                        ORDER BY p.podcastname, e.episodepubdate DESC
                    ) inner_q
                    {}
                    LIMIT $3 OFFSET $4"#,
                    cat_clause, filter_clause
                );

                let mut q = q::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(format!("%{}%", search_term))
                    .bind(user_id)
                    .bind(limit)
                    .bind(offset);

                for cat in categories {
                    q = q.bind(format!("%{}%", cat));
                }

                let rows = q.fetch_all(pool).await?;

                let total: i64 = rows.first()
                    .and_then(|r| r.try_get::<i64, _>("total_count").ok())
                    .unwrap_or(0);

                let mut results = Vec::new();
                for row in rows {
                    let pub_date = if let Ok(date) = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate") {
                        date.format("%Y-%m-%dT%H:%M:%S").to_string()
                    } else {
                        "".to_string()
                    };

                    let categories_str = row.try_get::<String, _>("categories").unwrap_or_default();
                    let categories_value = serde_json::to_value(self.parse_categories_json(&categories_str).unwrap_or_default()).unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                    let result = serde_json::json!({
                        "podcastid": row.try_get::<i32, _>("podcastid").unwrap_or(0),
                        "podcastname": row.try_get::<String, _>("podcastname").unwrap_or_default(),
                        "artworkurl": row.try_get::<Option<String>, _>("artworkurl").unwrap_or_default().unwrap_or_default(),
                        "author": row.try_get::<String, _>("author").unwrap_or_default(),
                        "categories": categories_value,
                        "podcastdescription": row.try_get::<String, _>("description").unwrap_or_default(),
                        "episodecount": row.try_get::<Option<i32>, _>("episodecount").ok().flatten(),
                        "feedurl": row.try_get::<String, _>("feedurl").unwrap_or_default(),
                        "websiteurl": row.try_get::<String, _>("websiteurl").unwrap_or_default(),
                        "explicit": if row.try_get::<bool, _>("explicit").unwrap_or(false) { 1 } else { 0 },
                        "userid": row.try_get::<i32, _>("userid").unwrap_or(0),
                        "episodeid": row.try_get::<i32, _>("episodeid").unwrap_or(0),
                        "episodetitle": row.try_get::<String, _>("episodetitle").unwrap_or_default(),
                        "episodedescription": row.try_get::<String, _>("episodedescription").unwrap_or_default(),
                        "episodeurl": row.try_get::<String, _>("episodeurl").unwrap_or_default(),
                        "episodeartwork": row.try_get::<String, _>("episodeartwork").unwrap_or_default(),
                        "episodepubdate": if pub_date.is_empty() { None } else { Some(pub_date) },
                        "episodeduration": row.try_get::<i32, _>("episodeduration").unwrap_or(0),
                        "listenduration": row.try_get::<i32, _>("listenduration").unwrap_or(0),
                        "completed": row.try_get::<bool, _>("completed").unwrap_or(false),
                        "saved": row.try_get::<bool, _>("saved").unwrap_or(false),
                        "queued": row.try_get::<bool, _>("queued").unwrap_or(false),
                        "downloaded": row.try_get::<bool, _>("downloaded").unwrap_or(false),
                        "is_youtube": row.try_get::<bool, _>("is_youtube").unwrap_or(false)
                    });
                    results.push(result);
                }
                Ok((results, total))
            },
            mysql(pool) => {
                let cat_clause = if categories.is_empty() {
                    String::new()
                } else {
                    let conds = vec!["LOWER(p.Categories) LIKE LOWER(?)"; categories.len()];
                    format!("AND ({})", conds.join(" OR "))
                };
                let filter_clause = match filter {
                    "unplayed"    => " WHERE listenduration = 0 AND completed = 0",
                    "in_progress" => " WHERE listenduration > 0 AND completed = 0",
                    "saved"       => " WHERE saved = 1",
                    "downloaded"  => " WHERE downloaded = 1",
                    _             => "",
                };

                let sql = format!(
                    "SELECT *, COUNT(*) OVER() AS total_count FROM (
                        SELECT
                            p.PodcastID as podcastid,
                            p.PodcastName as podcastname,
                            p.ArtworkURL as artworkurl,
                            p.Author as author,
                            p.Categories as categories,
                            p.Description as description,
                            p.EpisodeCount as episodecount,
                            p.FeedURL as feedurl,
                            p.WebsiteURL as websiteurl,
                            p.Explicit as explicit,
                            p.UserID as userid,
                            COALESCE(p.IsYouTubeChannel, false) as is_youtube,
                            e.EpisodeID as episodeid,
                            e.EpisodeTitle as episodetitle,
                            e.EpisodeDescription as episodedescription,
                            e.EpisodeURL as episodeurl,
                            CASE
                                WHEN p.UsePodcastCoversCustomized = TRUE AND p.UsePodcastCovers = TRUE THEN p.ArtworkURL
                                WHEN u.UsePodcastCovers = TRUE THEN p.ArtworkURL
                                ELSE e.EpisodeArtwork
                            END as episodeartwork,
                            e.EpisodePubDate as episodepubdate,
                            e.EpisodeDuration as episodeduration,
                            COALESCE(h.ListenDuration, 0) as listenduration,
                            COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) as completed,
                            CASE WHEN se.EpisodeID IS NOT NULL THEN true ELSE false END as saved,
                            CASE WHEN eq.EpisodeID IS NOT NULL THEN true ELSE false END as queued,
                            CASE WHEN de.EpisodeID IS NOT NULL THEN true ELSE false END as downloaded
                        FROM Podcasts p
                        LEFT JOIN Users u ON p.UserID = u.UserID
                        LEFT JOIN Episodes e ON p.FeedID = e.FeedID
                        LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = ?
                        LEFT JOIN SavedEpisodes se ON e.EpisodeID = se.EpisodeID AND se.UserID = ?
                        LEFT JOIN EpisodeQueue eq ON e.EpisodeID = eq.EpisodeID AND eq.UserID = ? AND eq.is_youtube = false
                        LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID AND de.UserID = ?
                        WHERE p.UserID = ?
                          AND e.EpisodeID IS NOT NULL
                          AND (LOWER(p.PodcastName) LIKE LOWER(?)
                               OR LOWER(e.EpisodeTitle) LIKE LOWER(?)
                               OR LOWER(e.EpisodeDescription) LIKE LOWER(?))
                          {}
                        ORDER BY p.PodcastName, e.EpisodePubDate DESC
                    ) inner_q
                    {}
                    LIMIT ? OFFSET ?",
                    cat_clause, filter_clause
                );

                let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(format!("%{}%", search_term))
                    .bind(format!("%{}%", search_term))
                    .bind(format!("%{}%", search_term))
                    .bind(limit)
                    .bind(offset);

                for cat in categories {
                    q = q.bind(format!("%{}%", cat));
                }

                let rows = q.fetch_all(pool).await?;

                let total: i64 = rows.first()
                    .and_then(|r| r.try_get::<i64, _>("total_count").ok())
                    .unwrap_or(0);

                let mut results = Vec::new();
                for row in rows {
                    let pub_date = if let Ok(date) = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate") {
                        date.format("%Y-%m-%dT%H:%M:%S").to_string()
                    } else {
                        "".to_string()
                    };

                    let categories_str = row.try_get::<String, _>("categories").unwrap_or_default();
                    let categories_value = serde_json::to_value(self.parse_categories_json(&categories_str).unwrap_or_default()).unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

                    let result = serde_json::json!({
                        "podcastid": row.try_get::<i32, _>("podcastid").unwrap_or(0),
                        "podcastname": row.try_get::<String, _>("podcastname").unwrap_or_default(),
                        "artworkurl": row.try_get::<Option<String>, _>("artworkurl").unwrap_or_default().unwrap_or_default(),
                        "author": row.try_get::<String, _>("author").unwrap_or_default(),
                        "categories": categories_value,
                        "podcastdescription": row.try_get::<String, _>("description").unwrap_or_default(),
                        "episodecount": row.try_get::<Option<i32>, _>("episodecount").ok().flatten(),
                        "feedurl": row.try_get::<String, _>("feedurl").unwrap_or_default(),
                        "websiteurl": row.try_get::<String, _>("websiteurl").unwrap_or_default(),
                        "explicit": if row.try_get::<bool, _>("explicit").unwrap_or(false) { 1 } else { 0 },
                        "userid": row.try_get::<i32, _>("userid").unwrap_or(0),
                        "episodeid": row.try_get::<i32, _>("episodeid").unwrap_or(0),
                        "episodetitle": row.try_get::<String, _>("episodetitle").unwrap_or_default(),
                        "episodedescription": row.try_get::<String, _>("episodedescription").unwrap_or_default(),
                        "episodeurl": row.try_get::<String, _>("episodeurl").unwrap_or_default(),
                        "episodeartwork": row.try_get::<String, _>("episodeartwork").unwrap_or_default(),
                        "episodepubdate": if pub_date.is_empty() { None } else { Some(pub_date) },
                        "episodeduration": row.try_get::<i32, _>("episodeduration").unwrap_or(0),
                        "listenduration": row.try_get::<i32, _>("listenduration").unwrap_or(0),
                        "completed": row.try_get::<bool, _>("completed").unwrap_or(false),
                        "saved": row.try_get::<bool, _>("saved").unwrap_or(false),
                        "queued": row.try_get::<bool, _>("queued").unwrap_or(false),
                        "downloaded": row.try_get::<bool, _>("downloaded").unwrap_or(false),
                        "is_youtube": row.try_get::<bool, _>("is_youtube").unwrap_or(false)
                    });
                    results.push(result);
                }
                Ok((results, total))
            },
        })
    }

    // Get home overview data - matches Python get_home_overview function
    pub async fn get_home_overview(&self, user_id: i32) -> AppResult<serde_json::Value> {
        dialect_match!(self, {
            pg(pool) => {
                // Shared row -> JSON mapping for the episode-list sections (recent,
                // in-progress, queue preview). All three queries alias to these columns.
                let map_episode_row = |row: &q::Row| -> AppResult<serde_json::Value> {
                    let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                    Ok(serde_json::json!({
                        "episodeid": row.try_get::<i32, _>("episodeid")?,
                        "episodetitle": row.try_get::<String, _>("episodetitle")?,
                        "episodepubdate": naive.format("%Y-%m-%dT%H:%M:%S").to_string(),
                        "episodedescription": row.try_get::<String, _>("episodedescription")?,
                        "episodeartwork": row.try_get::<String, _>("episodeartwork")?,
                        "episodeurl": row.try_get::<String, _>("episodeurl")?,
                        "episodeduration": row.try_get::<i32, _>("episodeduration")?,
                        "completed": row.try_get::<bool, _>("completed")?,
                        "podcastname": row.try_get::<String, _>("podcastname")?,
                        "podcastid": row.try_get::<i32, _>("podcastid")?,
                        "is_youtube": row.try_get::<bool, _>("is_youtube")?,
                        "listenduration": row.try_get::<Option<i32>, _>("listenduration")?,
                        "saved": row.try_get::<bool, _>("saved")?,
                        "queued": row.try_get::<bool, _>("queued")?,
                        "downloaded": row.try_get::<bool, _>("downloaded")?
                    }))
                };

                // Recent Episodes query
                let recent_query = r#"
                    SELECT
                        "Episodes".episodeid,
                        "Episodes".episodetitle,
                        "Episodes".episodepubdate,
                        "Episodes".episodedescription,
                        CASE 
                            WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                            WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                            ELSE "Episodes".episodeartwork
                        END as episodeartwork,
                        "Episodes".episodeurl,
                        "Episodes".episodeduration,
                        COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) AS completed,
                        "Podcasts".podcastname,
                        "Podcasts".podcastid,
                        "Podcasts".isyoutubechannel as is_youtube,
                        "UserEpisodeHistory".listenduration,
                        CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                        CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                        CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
                    FROM "Episodes"
                    INNER JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid
                    LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                    LEFT JOIN "UserEpisodeHistory" ON
                        "Episodes".episodeid = "UserEpisodeHistory".episodeid
                        AND "UserEpisodeHistory".userid = $1
                    LEFT JOIN "SavedEpisodes" ON
                        "Episodes".episodeid = "SavedEpisodes".episodeid
                        AND "SavedEpisodes".userid = $2
                    LEFT JOIN "EpisodeQueue" ON
                        "Episodes".episodeid = "EpisodeQueue".episodeid
                        AND "EpisodeQueue".userid = $3
                    LEFT JOIN "DownloadedEpisodes" ON
                        "Episodes".episodeid = "DownloadedEpisodes".episodeid
                        AND "DownloadedEpisodes".userid = $4
                    WHERE "Podcasts".userid = $5
                        AND "Episodes".episodepubdate >= NOW() - INTERVAL '7 days'
                    ORDER BY "Episodes".episodepubdate DESC
                    LIMIT 10
                "#;

                let recent_fut = async {
                    let rows = q::query(recent_query)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;
                    let mut episodes = Vec::with_capacity(rows.len());
                    for row in &rows {
                        episodes.push(map_episode_row(row)?);
                    }
                    Ok::<serde_json::Value, AppError>(serde_json::Value::Array(episodes))
                };

                // In Progress Episodes query
                let in_progress_query = r#"
                    SELECT
                        "Episodes".episodeid,
                        "Episodes".episodetitle,
                        "Episodes".episodepubdate,
                        "Episodes".episodedescription,
                        CASE 
                            WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                            WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                            ELSE "Episodes".episodeartwork
                        END as episodeartwork,
                        "Episodes".episodeurl,
                        "Episodes".episodeduration,
                        COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) AS completed,
                        "Podcasts".podcastname,
                        "Podcasts".podcastid,
                        "Podcasts".isyoutubechannel as is_youtube,
                        "UserEpisodeHistory".listenduration,
                        CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                        CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                        CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
                    FROM "UserEpisodeHistory"
                    JOIN "Episodes" ON "UserEpisodeHistory".episodeid = "Episodes".episodeid
                    JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid AND "Podcasts".userid = "UserEpisodeHistory".userid
                    LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                    LEFT JOIN "SavedEpisodes" ON
                        "Episodes".episodeid = "SavedEpisodes".episodeid
                        AND "SavedEpisodes".userid = $1
                    LEFT JOIN "EpisodeQueue" ON
                        "Episodes".episodeid = "EpisodeQueue".episodeid
                        AND "EpisodeQueue".userid = $2
                    LEFT JOIN "DownloadedEpisodes" ON
                        "Episodes".episodeid = "DownloadedEpisodes".episodeid
                        AND "DownloadedEpisodes".userid = $3
                    WHERE "UserEpisodeHistory".userid = $4
                    AND "UserEpisodeHistory".listenduration > 0
                    AND COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) = FALSE
                    ORDER BY "UserEpisodeHistory".listendate DESC
                    LIMIT 10
                "#;

                let in_progress_fut = async {
                    let rows = q::query(in_progress_query)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;
                    let mut episodes = Vec::with_capacity(rows.len());
                    for row in &rows {
                        episodes.push(map_episode_row(row)?);
                    }
                    Ok::<serde_json::Value, AppError>(serde_json::Value::Array(episodes))
                };

                // Queue preview query (next few queued episodes, same shape as the lists above)
                let queue_preview_query = r#"
                    SELECT
                        "Episodes".episodeid,
                        "Episodes".episodetitle,
                        "Episodes".episodepubdate,
                        "Episodes".episodedescription,
                        CASE
                            WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                            WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                            ELSE "Episodes".episodeartwork
                        END as episodeartwork,
                        "Episodes".episodeurl,
                        "Episodes".episodeduration,
                        COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) AS completed,
                        "Podcasts".podcastname,
                        "Podcasts".podcastid,
                        "Podcasts".isyoutubechannel as is_youtube,
                        "UserEpisodeHistory".listenduration,
                        CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                        TRUE as queued,
                        CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
                    FROM "EpisodeQueue"
                    INNER JOIN "Episodes" ON "EpisodeQueue".episodeid = "Episodes".episodeid
                    INNER JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid AND "Podcasts".userid = "EpisodeQueue".userid
                    LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                    LEFT JOIN "UserEpisodeHistory" ON
                        "EpisodeQueue".episodeid = "UserEpisodeHistory".episodeid
                        AND "UserEpisodeHistory".userid = $1
                    LEFT JOIN "SavedEpisodes" ON
                        "Episodes".episodeid = "SavedEpisodes".episodeid
                        AND "SavedEpisodes".userid = $2
                    LEFT JOIN "DownloadedEpisodes" ON
                        "Episodes".episodeid = "DownloadedEpisodes".episodeid
                        AND "DownloadedEpisodes".userid = $3
                    WHERE "EpisodeQueue".userid = $4 AND "EpisodeQueue".is_youtube = FALSE
                    ORDER BY "EpisodeQueue".queueposition ASC
                    LIMIT 3
                "#;

                let queue_preview_fut = async {
                    let rows = q::query(queue_preview_query)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;
                    let mut episodes = Vec::with_capacity(rows.len());
                    for row in &rows {
                        episodes.push(map_episode_row(row)?);
                    }
                    Ok::<serde_json::Value, AppError>(serde_json::Value::Array(episodes))
                };

                // Top Podcasts query. Aggregate the user's listen history first (only rows they
                // actually listened to) and then join podcast metadata, instead of scanning every
                // episode of every podcast and grouping afterwards.
                let top_podcasts_query = r#"
                    SELECT
                        "Podcasts".podcastid,
                        "Podcasts".podcastname,
                        "Podcasts".podcastindexid,
                        "Podcasts".artworkurl,
                        "Podcasts".author,
                        "Podcasts".categories,
                        "Podcasts".description,
                        "Podcasts".episodecount,
                        "Podcasts".feedurl,
                        "Podcasts".websiteurl,
                        "Podcasts".explicit,
                        "Podcasts".isyoutubechannel as is_youtube,
                        COALESCE(stats.play_count, 0) as play_count,
                        stats.total_listen_time as total_listen_time
                    FROM "Podcasts"
                    LEFT JOIN (
                        SELECT "Episodes".feedid as feedid,
                               COUNT(DISTINCT "UserEpisodeHistory".episodeid) as play_count,
                               SUM("UserEpisodeHistory".listenduration) as total_listen_time
                        FROM "UserEpisodeHistory"
                        JOIN "Episodes" ON "UserEpisodeHistory".episodeid = "Episodes".episodeid
                        WHERE "UserEpisodeHistory".userid = $1
                        GROUP BY "Episodes".feedid
                    ) stats ON "Podcasts".feedid = stats.feedid
                    WHERE "Podcasts".userid = $2
                    ORDER BY stats.total_listen_time DESC NULLS LAST
                    LIMIT 6
                "#;

                let top_podcasts_fut = async {
                    let rows = q::query(top_podcasts_query)
                        .bind(user_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;
                    let mut top_podcasts = Vec::with_capacity(rows.len());
                    for row in &rows {
                        let podcastid: i32 = row.try_get("podcastid").unwrap_or(0);
                        let podcastname: String = row.try_get("podcastname").unwrap_or_default();
                        let podcastindexid: Option<i32> = row.try_get("podcastindexid").ok();
                        let artworkurl: String = row.try_get("artworkurl").unwrap_or_default();
                        let author: String = row.try_get("author").unwrap_or_default();
                        let categories_str: String = row.try_get("categories").unwrap_or_default();
                        let categories = self.parse_categories_json(&categories_str).unwrap_or_default();
                        let description: String = row.try_get("description").unwrap_or_default();
                        let episodecount: i32 = row.try_get("episodecount").unwrap_or(0);
                        let feedurl: String = row.try_get("feedurl").unwrap_or_default();
                        let websiteurl: String = row.try_get("websiteurl").unwrap_or_default();
                        let explicit: bool = row.try_get("explicit").unwrap_or(false);
                        let is_youtube: bool = row.try_get("is_youtube").unwrap_or(false);
                        let play_count: i64 = row.try_get("play_count").unwrap_or(0);
                        let total_listen_time: Option<i64> = row.try_get("total_listen_time").ok();

                        top_podcasts.push(serde_json::json!({
                            "podcastid": podcastid,
                            "podcastname": podcastname,
                            "podcastindexid": podcastindexid,
                            "artworkurl": artworkurl,
                            "author": author,
                            "categories": serde_json::to_value(categories).unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                            "description": description,
                            "episodecount": episodecount,
                            "feedurl": feedurl,
                            "websiteurl": websiteurl,
                            "explicit": explicit,
                            "is_youtube": is_youtube,
                            "play_count": play_count,
                            "total_listen_time": total_listen_time
                        }));
                    }
                    Ok::<serde_json::Value, AppError>(serde_json::Value::Array(top_podcasts))
                };

                // Counts (folded into a single round-trip) and weekly listening stats.
                let counts_fut = async {
                    let row = q::query(
                        r#"SELECT
                            (SELECT COUNT(*) FROM "SavedEpisodes" WHERE userid = $1) AS saved_count,
                            (SELECT COUNT(*) FROM "DownloadedEpisodes" WHERE userid = $1) AS downloaded_count,
                            (SELECT COUNT(*) FROM "EpisodeQueue" WHERE userid = $1) AS queue_count"#,
                    )
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                    let saved_count: i64 = row.try_get("saved_count")?;
                    let downloaded_count: i64 = row.try_get("downloaded_count")?;
                    let queue_count: i64 = row.try_get("queue_count")?;
                    Ok::<(i64, i64, i64), AppError>((saved_count, downloaded_count, queue_count))
                };

                let weekly_fut = async {
                    let row = q::query(
                        r#"SELECT
                            COALESCE(SUM("UserEpisodeHistory".listenduration), 0)::bigint AS seconds_listened,
                            COUNT(*) FILTER (WHERE EXISTS (SELECT 1 FROM "UserEpisodeState" ues
                                WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "UserEpisodeHistory".userid AND ues.completed)) AS episodes_completed
                        FROM "UserEpisodeHistory"
                        JOIN "Episodes" ON "UserEpisodeHistory".episodeid = "Episodes".episodeid
                        WHERE "UserEpisodeHistory".userid = $1
                          AND "UserEpisodeHistory".listendate >= NOW() - INTERVAL '7 days'"#,
                    )
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                    let seconds_listened: i64 = row.try_get("seconds_listened")?;
                    let episodes_completed: i64 = row.try_get("episodes_completed")?;
                    Ok::<serde_json::Value, AppError>(serde_json::json!({
                        "seconds_listened": seconds_listened,
                        "episodes_completed": episodes_completed
                    }))
                };

                // Run all independent reads concurrently instead of serially.
                let (
                    recent_episodes,
                    in_progress_episodes,
                    queue_preview,
                    top_podcasts,
                    counts,
                    weekly_stats,
                ) = tokio::try_join!(
                    recent_fut,
                    in_progress_fut,
                    queue_preview_fut,
                    top_podcasts_fut,
                    counts_fut,
                    weekly_fut
                )?;
                let (saved_count, downloaded_count, queue_count) = counts;

                Ok(serde_json::json!({
                    "recent_episodes": recent_episodes,
                    "in_progress_episodes": in_progress_episodes,
                    "queue_preview": queue_preview,
                    "top_podcasts": top_podcasts,
                    "saved_count": saved_count,
                    "downloaded_count": downloaded_count,
                    "queue_count": queue_count,
                    "weekly_stats": weekly_stats
                }))
            },
            mysql(pool) => {
                // Shared row -> JSON mapping for the episode-list sections (recent,
                // in-progress, queue preview). All three queries alias to these columns.
                let map_episode_row = |row: &sqlx::mysql::MySqlRow| -> AppResult<serde_json::Value> {
                    let naive = row.try_get::<chrono::NaiveDateTime, _>("EpisodePubDate")?;
                    Ok(serde_json::json!({
                        "episodeid": row.try_get::<i32, _>("EpisodeID")?,
                        "episodetitle": row.try_get::<String, _>("EpisodeTitle")?,
                        "episodepubdate": naive.format("%Y-%m-%dT%H:%M:%S").to_string(),
                        "episodedescription": row.try_get::<String, _>("EpisodeDescription")?,
                        "episodeartwork": row.try_get::<String, _>("EpisodeArtwork")?,
                        "episodeurl": row.try_get::<String, _>("EpisodeURL")?,
                        "episodeduration": row.try_get::<i32, _>("EpisodeDuration")?,
                        "completed": row.try_get::<i8, _>("Completed")? != 0,
                        "podcastname": row.try_get::<String, _>("PodcastName")?,
                        "podcastid": row.try_get::<i32, _>("PodcastID")?,
                        "is_youtube": row.try_get::<i8, _>("is_youtube")? != 0,
                        "listenduration": row.try_get::<Option<i32>, _>("ListenDuration")?,
                        "saved": row.try_get::<i8, _>("saved")? != 0,
                        "queued": row.try_get::<i8, _>("queued")? != 0,
                        "downloaded": row.try_get::<i8, _>("downloaded")? != 0
                    }))
                };

                // Recent Episodes query for MySQL
                let recent_query = r#"
                    SELECT
                        Episodes.EpisodeID,
                        Episodes.EpisodeTitle,
                        Episodes.EpisodePubDate,
                        Episodes.EpisodeDescription,
                        CASE 
                            WHEN Podcasts.UsePodcastCoversCustomized = 1 AND Podcasts.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                            WHEN Users.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                            ELSE Episodes.EpisodeArtwork
                        END as EpisodeArtwork,
                        Episodes.EpisodeURL,
                        Episodes.EpisodeDuration,
                        COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = Podcasts.UserID), 0) AS Completed,
                        Podcasts.PodcastName,
                        Podcasts.PodcastID,
                        Podcasts.IsYouTubeChannel as is_youtube,
                        UserEpisodeHistory.ListenDuration,
                        CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                        CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                        CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
                    FROM Episodes
                    INNER JOIN Podcasts ON Episodes.FeedID = Podcasts.FeedID
                    LEFT JOIN Users ON Podcasts.UserID = Users.UserID
                    LEFT JOIN UserEpisodeHistory ON
                        Episodes.EpisodeID = UserEpisodeHistory.EpisodeID
                        AND UserEpisodeHistory.UserID = ?
                    LEFT JOIN SavedEpisodes ON
                        Episodes.EpisodeID = SavedEpisodes.EpisodeID
                        AND SavedEpisodes.UserID = ?
                    LEFT JOIN EpisodeQueue ON
                        Episodes.EpisodeID = EpisodeQueue.EpisodeID
                        AND EpisodeQueue.UserID = ?
                    LEFT JOIN DownloadedEpisodes ON
                        Episodes.EpisodeID = DownloadedEpisodes.EpisodeID
                        AND DownloadedEpisodes.UserID = ?
                    WHERE Podcasts.UserID = ?
                        AND Episodes.EpisodePubDate >= DATE_SUB(NOW(), INTERVAL 7 DAY)
                    ORDER BY Episodes.EpisodePubDate DESC
                    LIMIT 10
                "#;

                let recent_fut = async {
                    let rows = sqlx::query(recent_query)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;
                    let mut episodes = Vec::with_capacity(rows.len());
                    for row in &rows {
                        episodes.push(map_episode_row(row)?);
                    }
                    Ok::<serde_json::Value, AppError>(serde_json::Value::Array(episodes))
                };

                // In Progress Episodes query for MySQL
                let in_progress_query = r#"
                    SELECT
                        Episodes.EpisodeID,
                        Episodes.EpisodeTitle,
                        Episodes.EpisodePubDate,
                        Episodes.EpisodeDescription,
                        CASE 
                            WHEN Podcasts.UsePodcastCoversCustomized = 1 AND Podcasts.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                            WHEN Users.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                            ELSE Episodes.EpisodeArtwork
                        END as EpisodeArtwork,
                        Episodes.EpisodeURL,
                        Episodes.EpisodeDuration,
                        COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = Podcasts.UserID), 0) AS Completed,
                        Podcasts.PodcastName,
                        Podcasts.PodcastID,
                        Podcasts.IsYouTubeChannel as is_youtube,
                        UserEpisodeHistory.ListenDuration,
                        CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                        CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                        CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
                    FROM UserEpisodeHistory
                    JOIN Episodes ON UserEpisodeHistory.EpisodeID = Episodes.EpisodeID
                    JOIN Podcasts ON Episodes.FeedID = Podcasts.FeedID AND Podcasts.UserID = UserEpisodeHistory.UserID
                    LEFT JOIN Users ON Podcasts.UserID = Users.UserID
                    LEFT JOIN SavedEpisodes ON
                        Episodes.EpisodeID = SavedEpisodes.EpisodeID
                        AND SavedEpisodes.UserID = ?
                    LEFT JOIN EpisodeQueue ON
                        Episodes.EpisodeID = EpisodeQueue.EpisodeID
                        AND EpisodeQueue.UserID = ?
                    LEFT JOIN DownloadedEpisodes ON
                        Episodes.EpisodeID = DownloadedEpisodes.EpisodeID
                        AND DownloadedEpisodes.UserID = ?
                    WHERE UserEpisodeHistory.UserID = ?
                    AND UserEpisodeHistory.ListenDuration > 0
                    AND COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = Podcasts.UserID), 0) = 0
                    ORDER BY UserEpisodeHistory.ListenDate DESC
                    LIMIT 10
                "#;

                let in_progress_fut = async {
                    let rows = sqlx::query(in_progress_query)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;
                    let mut episodes = Vec::with_capacity(rows.len());
                    for row in &rows {
                        episodes.push(map_episode_row(row)?);
                    }
                    Ok::<serde_json::Value, AppError>(serde_json::Value::Array(episodes))
                };

                // Queue preview query (next few queued episodes, same shape as the lists above)
                let queue_preview_query = r#"
                    SELECT
                        Episodes.EpisodeID,
                        Episodes.EpisodeTitle,
                        Episodes.EpisodePubDate,
                        Episodes.EpisodeDescription,
                        CASE
                            WHEN Podcasts.UsePodcastCoversCustomized = 1 AND Podcasts.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                            WHEN Users.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                            ELSE Episodes.EpisodeArtwork
                        END as EpisodeArtwork,
                        Episodes.EpisodeURL,
                        Episodes.EpisodeDuration,
                        COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = Podcasts.UserID), 0) AS Completed,
                        Podcasts.PodcastName,
                        Podcasts.PodcastID,
                        Podcasts.IsYouTubeChannel as is_youtube,
                        UserEpisodeHistory.ListenDuration,
                        CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                        TRUE as queued,
                        CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded
                    FROM EpisodeQueue
                    INNER JOIN Episodes ON EpisodeQueue.EpisodeID = Episodes.EpisodeID
                    INNER JOIN Podcasts ON Episodes.FeedID = Podcasts.FeedID AND Podcasts.UserID = EpisodeQueue.UserID
                    LEFT JOIN Users ON Podcasts.UserID = Users.UserID
                    LEFT JOIN UserEpisodeHistory ON
                        EpisodeQueue.EpisodeID = UserEpisodeHistory.EpisodeID
                        AND UserEpisodeHistory.UserID = ?
                    LEFT JOIN SavedEpisodes ON
                        Episodes.EpisodeID = SavedEpisodes.EpisodeID
                        AND SavedEpisodes.UserID = ?
                    LEFT JOIN DownloadedEpisodes ON
                        Episodes.EpisodeID = DownloadedEpisodes.EpisodeID
                        AND DownloadedEpisodes.UserID = ?
                    WHERE EpisodeQueue.UserID = ? AND EpisodeQueue.is_youtube = FALSE
                    ORDER BY EpisodeQueue.QueuePosition ASC
                    LIMIT 3
                "#;

                let queue_preview_fut = async {
                    let rows = sqlx::query(queue_preview_query)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;
                    let mut episodes = Vec::with_capacity(rows.len());
                    for row in &rows {
                        episodes.push(map_episode_row(row)?);
                    }
                    Ok::<serde_json::Value, AppError>(serde_json::Value::Array(episodes))
                };

                // Top Podcasts query for MySQL. Aggregate listen history first (only rows actually
                // listened to), then join podcast metadata, instead of scanning every episode.
                let top_podcasts_query = r#"
                    SELECT
                        Podcasts.PodcastID,
                        Podcasts.PodcastName,
                        Podcasts.PodcastIndexID,
                        Podcasts.ArtworkURL,
                        Podcasts.Author,
                        Podcasts.Categories,
                        Podcasts.Description,
                        Podcasts.EpisodeCount,
                        Podcasts.FeedURL,
                        Podcasts.WebsiteURL,
                        Podcasts.Explicit,
                        Podcasts.IsYouTubeChannel as is_youtube,
                        COALESCE(stats.play_count, 0) as play_count,
                        stats.total_listen_time as total_listen_time
                    FROM Podcasts
                    LEFT JOIN (
                        SELECT Episodes.FeedID as FeedID,
                               COUNT(DISTINCT UserEpisodeHistory.EpisodeID) as play_count,
                               SUM(UserEpisodeHistory.ListenDuration) as total_listen_time
                        FROM UserEpisodeHistory
                        JOIN Episodes ON UserEpisodeHistory.EpisodeID = Episodes.EpisodeID
                        WHERE UserEpisodeHistory.UserID = ?
                        GROUP BY Episodes.FeedID
                    ) stats ON Podcasts.FeedID = stats.FeedID
                    WHERE Podcasts.UserID = ?
                    ORDER BY stats.total_listen_time DESC
                    LIMIT 5
                "#;

                let top_podcasts_fut = async {
                    let rows = sqlx::query(top_podcasts_query)
                        .bind(user_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;
                    let mut top_podcasts = Vec::with_capacity(rows.len());
                    for row in &rows {
                        let podcastid: i32 = row.try_get("PodcastID").unwrap_or(0);
                        let podcastname: String = row.try_get("PodcastName").unwrap_or_default();
                        let podcastindexid: Option<i32> = row.try_get("PodcastIndexID").ok();
                        let artworkurl: String = row.try_get("ArtworkURL").unwrap_or_default();
                        let author: String = row.try_get("Author").unwrap_or_default();
                        let categories_str: String = row.try_get("Categories").unwrap_or_default();
                        let categories = self.parse_categories_json(&categories_str).unwrap_or_default();
                        let description: String = row.try_get("Description").unwrap_or_default();
                        let episodecount: i32 = row.try_get("EpisodeCount").unwrap_or(0);
                        let feedurl: String = row.try_get("FeedURL").unwrap_or_default();
                        let websiteurl: String = row.try_get("WebsiteURL").unwrap_or_default();
                        let explicit: bool = row.try_get::<i8, _>("Explicit").unwrap_or(0) != 0;
                        let is_youtube: bool = row.try_get::<i8, _>("is_youtube").unwrap_or(0) != 0;
                        let play_count: i64 = row.try_get("play_count").unwrap_or(0);
                        let total_listen_time: Option<i64> = row.try_get("total_listen_time").ok();

                        top_podcasts.push(serde_json::json!({
                            "podcastid": podcastid,
                            "podcastname": podcastname,
                            "podcastindexid": podcastindexid,
                            "artworkurl": artworkurl,
                            "author": author,
                            "categories": serde_json::to_value(categories).unwrap_or(serde_json::Value::Object(serde_json::Map::new())),
                            "description": description,
                            "episodecount": episodecount,
                            "feedurl": feedurl,
                            "websiteurl": websiteurl,
                            "explicit": explicit,
                            "is_youtube": is_youtube,
                            "play_count": play_count,
                            "total_listen_time": total_listen_time
                        }));
                    }
                    Ok::<serde_json::Value, AppError>(serde_json::Value::Array(top_podcasts))
                };

                // Counts (folded into a single round-trip) and weekly listening stats.
                let counts_fut = async {
                    let row = sqlx::query(
                        "SELECT \
                            (SELECT COUNT(*) FROM SavedEpisodes WHERE UserID = ?) AS saved_count, \
                            (SELECT COUNT(*) FROM DownloadedEpisodes WHERE UserID = ?) AS downloaded_count, \
                            (SELECT COUNT(*) FROM EpisodeQueue WHERE UserID = ?) AS queue_count",
                    )
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                    let saved_count: i64 = row.try_get("saved_count")?;
                    let downloaded_count: i64 = row.try_get("downloaded_count")?;
                    let queue_count: i64 = row.try_get("queue_count")?;
                    Ok::<(i64, i64, i64), AppError>((saved_count, downloaded_count, queue_count))
                };

                let weekly_fut = async {
                    let row = sqlx::query(
                        "SELECT \
                            CAST(COALESCE(SUM(UserEpisodeHistory.ListenDuration), 0) AS SIGNED) AS seconds_listened, \
                            CAST(COALESCE(SUM(CASE WHEN EXISTS (SELECT 1 FROM UserEpisodeState ues \
                                WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = UserEpisodeHistory.UserID AND ues.Completed = 1) \
                                THEN 1 ELSE 0 END), 0) AS SIGNED) AS episodes_completed \
                        FROM UserEpisodeHistory \
                        JOIN Episodes ON UserEpisodeHistory.EpisodeID = Episodes.EpisodeID \
                        WHERE UserEpisodeHistory.UserID = ? \
                          AND UserEpisodeHistory.ListenDate >= DATE_SUB(NOW(), INTERVAL 7 DAY)",
                    )
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                    let seconds_listened: i64 = row.try_get("seconds_listened")?;
                    let episodes_completed: i64 = row.try_get("episodes_completed")?;
                    Ok::<serde_json::Value, AppError>(serde_json::json!({
                        "seconds_listened": seconds_listened,
                        "episodes_completed": episodes_completed
                    }))
                };

                // Run all independent reads concurrently instead of serially.
                let (
                    recent_episodes,
                    in_progress_episodes,
                    queue_preview,
                    top_podcasts,
                    counts,
                    weekly_stats,
                ) = tokio::try_join!(
                    recent_fut,
                    in_progress_fut,
                    queue_preview_fut,
                    top_podcasts_fut,
                    counts_fut,
                    weekly_fut
                )?;
                let (saved_count, downloaded_count, queue_count) = counts;

                Ok(serde_json::json!({
                    "recent_episodes": recent_episodes,
                    "in_progress_episodes": in_progress_episodes,
                    "queue_preview": queue_preview,
                    "top_podcasts": top_podcasts,
                    "saved_count": saved_count,
                    "downloaded_count": downloaded_count,
                    "queue_count": queue_count,
                    "weekly_stats": weekly_stats
                }))
            },
        })
    }
}
