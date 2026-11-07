//! Smart playlists.

use super::*;

// When each user's playlist counts were last computed, per database (in-process).
pub(super) static PLAYLIST_COUNTS_AT: std::sync::LazyLock<std::sync::Mutex<HashMap<(String, i32), std::time::Instant>>> =
    std::sync::LazyLock::new(Default::default);

pub(super) const PLAYLIST_COUNTS_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 60);

// Playlist configuration struct - matches Python playlist data structure exactly
#[derive(Debug, Clone)]
pub struct PlaylistConfig {
    pub podcast_ids: Option<Vec<i32>>,
    pub include_unplayed: bool,
    pub include_partially_played: bool,
    pub include_played: bool,
    pub play_progress_min: Option<f64>,
    pub play_progress_max: Option<f64>,
    pub time_filter_hours: Option<i32>,
    pub min_duration: Option<i32>,
    pub max_duration: Option<i32>,
    pub sort_order: String,
    pub group_by_podcast: bool,
    pub max_episodes: Option<i32>,
}

impl PlaylistConfig {
    /// From a PostgreSQL or SQLite row; `podcast_ids` is its decoded `podcastids` array.
    pub fn from_row<R: sqlx::Row>(row: &R, podcast_ids: Result<Option<Vec<i32>>, sqlx::Error>) -> AppResult<Self>
    where
        for<'c> &'c str: sqlx::ColumnIndex<R>,
        for<'r> bool: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
        for<'r> i32: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
        for<'r> f64: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
        for<'r> String: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    {
        // Parse podcast IDs from the int4 array - can be NULL or {29,57} format
        let podcast_ids = match podcast_ids {
            Ok(Some(ids)) => {
                info!("PostgreSQL got podcastids array: {:?}", ids);
                if ids.is_empty() {
                    None
                } else {
                    Some(ids)
                }
            }
            Ok(None) => {
                info!("PostgreSQL podcastids is NULL");
                None
            }
            Err(_) => {
                warn!("PostgreSQL failed to get podcastids as array");
                None
            }
        };
        
        // Decoded as Option: SQLite's driver reads NULL as 0 / "" / false otherwise.
        fn opt<'r, R: sqlx::Row, T>(row: &'r R, col: &str) -> Option<T>
        where
            for<'c> &'c str: sqlx::ColumnIndex<R>,
            T: sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
        {
            row.try_get::<Option<T>, _>(col).ok().flatten()
        }
        Ok(PlaylistConfig {
            podcast_ids,
            include_unplayed: opt(row, "includeunplayed").unwrap_or(true),
            include_partially_played: opt(row, "includepartiallyplayed").unwrap_or(true),
            include_played: opt(row, "includeplayed").unwrap_or(false),
            play_progress_min: opt(row, "playprogressmin"),
            play_progress_max: opt(row, "playprogressmax"),
            time_filter_hours: opt(row, "timefilterhours"),
            min_duration: opt(row, "minduration"),
            max_duration: opt(row, "maxduration"),
            sort_order: opt(row, "sortorder").unwrap_or_else(|| "date_desc".to_string()),
            group_by_podcast: opt(row, "groupbypodcast").unwrap_or(false),
            max_episodes: opt(row, "maxepisodes"),
        })
    }
    
    // Create from MySQL row - matches Python playlist dictionary extraction
    pub fn from_mysql_row(row: &sqlx::mysql::MySqlRow) -> AppResult<Self> {
        use sqlx::Row;
        
        // Parse podcast IDs from MySQL JSON (stored as BLOB)
        let podcast_ids = match row.try_get::<Option<Vec<u8>>, _>("PodcastIDs") {
            Ok(Some(ids_bytes)) => {
                let ids_str = String::from_utf8_lossy(&ids_bytes);
                info!("Got PodcastIDs from BLOB: '{}'", ids_str);
                if ids_str.is_empty() || ids_str == "null" || ids_str == "[]" {
                    None
                } else {
                    let parsed: Vec<i32> = serde_json::from_str(&ids_str).unwrap_or_default();
                    if parsed.is_empty() {
                        None
                    } else {
                        Some(parsed)
                    }
                }
            }
            Ok(None) => {
                info!("PodcastIDs is NULL");
                None
            }
            Err(_) => {
                // Fallback to try as String for older records
                match row.try_get::<Option<String>, _>("PodcastIDs") {
                    Ok(Some(ids_str)) => {
                        info!("Got PodcastIDs as String: '{}'", ids_str);
                        if ids_str.is_empty() || ids_str == "null" || ids_str == "[]" {
                            None
                        } else {
                            let parsed: Vec<i32> = serde_json::from_str(&ids_str).unwrap_or_default();
                            if parsed.is_empty() { None } else { Some(parsed) }
                        }
                    }
                    _ => None
                }
            }
        };
        
        Ok(PlaylistConfig {
            podcast_ids,
            include_unplayed: row.try_get("IncludeUnplayed").unwrap_or(true),
            include_partially_played: row.try_get("IncludePartiallyPlayed").unwrap_or(true),
            include_played: row.try_get("IncludePlayed").unwrap_or(false),
            play_progress_min: row.try_get("PlayProgressMin").ok(),
            play_progress_max: row.try_get("PlayProgressMax").ok(),
            time_filter_hours: row.try_get("TimeFilterHours").ok(),
            min_duration: row.try_get("MinDuration").ok(),
            max_duration: row.try_get("MaxDuration").ok(),
            sort_order: row.try_get("SortOrder").unwrap_or_else(|_| "date_desc".to_string()),
            group_by_podcast: row.try_get("GroupByPodcast").unwrap_or(false),
            max_episodes: row.try_get("MaxEpisodes").ok(),
        })
    }
    
    // Get PostgreSQL sort order - matches Python sort_mapping exactly (for inner query)
    pub fn get_postgres_sort_order(&self) -> String {
        match self.sort_order.as_str() {
            "date_asc" => "ORDER BY e.episodepubdate ASC".to_string(),
            "date_desc" => "ORDER BY e.episodepubdate DESC".to_string(),
            "duration_asc" => "ORDER BY e.episodeduration ASC".to_string(),
            "duration_desc" => "ORDER BY e.episodeduration DESC".to_string(),
            "listen_progress" => "ORDER BY (COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) DESC".to_string(),
            "completion" => "ORDER BY COALESCE(h.listenduration::float / NULLIF(e.episodeduration, 0), 0) DESC".to_string(),
            _ => "ORDER BY e.episodepubdate DESC".to_string(),
        }
    }
    
    // Get PostgreSQL sort order for outer query (ROW_NUMBER OVER) - fixed alias scoping
    pub fn get_postgres_outer_sort_order(&self) -> String {
        match self.sort_order.as_str() {
            "date_asc" => "ORDER BY episodes.episodepubdate ASC".to_string(),
            "date_desc" => "ORDER BY episodes.episodepubdate DESC".to_string(),
            "duration_asc" => "ORDER BY episodes.episodeduration ASC".to_string(),
            "duration_desc" => "ORDER BY episodes.episodeduration DESC".to_string(),
            "listen_progress" => "ORDER BY (episodes.listenduration::float / NULLIF(episodes.episodeduration, 0)) DESC".to_string(),
            "completion" => "ORDER BY (episodes.listenduration::float / NULLIF(episodes.episodeduration, 0)) DESC".to_string(),
            _ => "ORDER BY episodes.episodepubdate DESC".to_string(),
        }
    }
    
    // Get MySQL sort order - matches Python sort_mapping exactly (for inner query)
    pub fn get_mysql_sort_order(&self) -> String {
        match self.sort_order.as_str() {
            "date_asc" => "ORDER BY e.EpisodePubDate ASC".to_string(),
            "date_desc" => "ORDER BY e.EpisodePubDate DESC".to_string(),
            "duration_asc" => "ORDER BY e.EpisodeDuration ASC".to_string(),
            "duration_desc" => "ORDER BY e.EpisodeDuration DESC".to_string(),
            "listen_progress" => "ORDER BY (COALESCE(h.ListenDuration, 0) / NULLIF(e.EpisodeDuration, 0)) DESC".to_string(),
            "completion" => "ORDER BY COALESCE(h.ListenDuration / NULLIF(e.EpisodeDuration, 0), 0) DESC".to_string(),
            _ => "ORDER BY e.EpisodePubDate DESC".to_string(),
        }
    }
    
    // Get MySQL sort order for outer query (ROW_NUMBER OVER) - fixed alias scoping
    pub fn get_mysql_outer_sort_order(&self) -> String {
        match self.sort_order.as_str() {
            "date_asc" => "ORDER BY episodes.EpisodePubDate ASC".to_string(),
            "date_desc" => "ORDER BY episodes.EpisodePubDate DESC".to_string(),
            "duration_asc" => "ORDER BY episodes.EpisodeDuration ASC".to_string(),
            "duration_desc" => "ORDER BY episodes.EpisodeDuration DESC".to_string(),
            "listen_progress" => "ORDER BY (episodes.ListenDuration / NULLIF(episodes.EpisodeDuration, 0)) DESC".to_string(),
            "completion" => "ORDER BY (episodes.ListenDuration / NULLIF(episodes.EpisodeDuration, 0)) DESC".to_string(),
            _ => "ORDER BY episodes.EpisodePubDate DESC".to_string(),
        }
    }
    
}

// Standalone create_playlist function that matches Python API
pub async fn create_playlist(pool: &DatabasePool, config: &Config, playlist_data: &crate::models::CreatePlaylistRequest) -> AppResult<i32> {
    pool.create_playlist(config, playlist_data).await
}

// Standalone delete_playlist function that matches Python API
pub async fn delete_playlist(pool: &DatabasePool, _config: &Config, playlist_data: &crate::models::DeletePlaylistRequest) -> AppResult<()> {
    pool.delete_playlist(playlist_data.user_id, playlist_data.playlist_id).await
}

pub async fn update_playlist(pool: &DatabasePool, config: &Config, playlist_data: &crate::models::UpdatePlaylistRequest) -> AppResult<()> {
    pool.update_playlist(config, playlist_data).await
}

impl DatabasePool {
    // Get playlists - matches Python get_playlists function
    pub async fn get_playlists(&self, user_id: i32) -> AppResult<Vec<serde_json::Value>> {
        if self.playlist_counts_stale(user_id) {
            if let Err(e) = self.update_user_playlist_counts(user_id).await {
                warn!("Updating playlist counts for user {} failed: {}", user_id, e);
            }
        }
        dialect_match!(self, {
            pg(pool) => {
                let query = r#"
                    SELECT
                        p.playlistid,
                        p.userid,
                        p.name,
                        p.description,
                        p.issystemplaylist,
                        p.podcastids,
                        p.includeunplayed,
                        p.includepartiallyplayed,
                        p.includeplayed,
                        p.minduration,
                        p.maxduration,
                        p.sortorder,
                        p.groupbypodcast,
                        p.maxepisodes,
                        p.lastupdated,
                        p.created,
                        p.iconname,
                        COALESCE(p.episodecount, 0) as episode_count
                    FROM "Playlists" p
                    WHERE p.userid = $1
                    ORDER BY p.issystemplaylist DESC, p.name ASC
                "#;

                let rows = q::query(query)
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?;

                let mut playlists = Vec::new();
                for row in rows {
                    let playlist_id: i32 = row.try_get("playlistid")?;
                    
                    // Get preview episodes
                    let preview_query = r#"
                        SELECT e.episodetitle, e.episodeartwork
                        FROM "PlaylistContents" pc
                        JOIN "Episodes" e ON pc.episodeid = e.episodeid
                        JOIN "Podcasts" p ON e.feedid = p.feedid
                        WHERE pc.playlistid = $1
                        AND p.userid = $2
                        ORDER BY pc.position
                        LIMIT 3
                    "#;

                    let preview_rows = q::query(preview_query)
                        .bind(playlist_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;

                    let mut preview_episodes = Vec::new();
                    for preview_row in preview_rows {
                        preview_episodes.push(serde_json::json!({
                            "title": preview_row.try_get::<Option<String>, _>("episodetitle")?.unwrap_or_default(),
                            "artwork": preview_row.try_get::<Option<String>, _>("episodeartwork")?.unwrap_or_default()
                        }));
                    }

                    // Process podcast_ids - in PostgreSQL it's stored as an array
                    let podcast_ids: Option<Vec<i32>> = q::int_array(&row, "podcastids").ok().flatten();

                    let playlist = serde_json::json!({
                        "playlist_id": playlist_id,
                        "user_id": row.try_get::<i32, _>("userid")?,
                        "name": row.try_get::<String, _>("name")?,
                        "description": row.try_get::<Option<String>, _>("description")?.unwrap_or_default(),
                        "is_system_playlist": row.try_get::<bool, _>("issystemplaylist")?,
                        "podcast_ids": podcast_ids,
                        "include_unplayed": row.try_get::<bool, _>("includeunplayed")?,
                        "include_partially_played": row.try_get::<bool, _>("includepartiallyplayed")?,
                        "include_played": row.try_get::<bool, _>("includeplayed")?,
                        "min_duration": row.try_get::<Option<i32>, _>("minduration")?,
                        "max_duration": row.try_get::<Option<i32>, _>("maxduration")?,
                        "sort_order": row.try_get::<Option<String>, _>("sortorder")?,
                        "group_by_podcast": row.try_get::<bool, _>("groupbypodcast")?,
                        "max_episodes": row.try_get::<Option<i32>, _>("maxepisodes")?,
                        "last_updated": row.try_get::<Option<chrono::NaiveDateTime>, _>("lastupdated")?.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()).unwrap_or_default(),
                        "created": row.try_get::<Option<chrono::NaiveDateTime>, _>("created")?.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()).unwrap_or_default(),
                        "icon_name": row.try_get::<Option<String>, _>("iconname")?.unwrap_or_default(),
                        "episode_count": row.try_get::<i32, _>("episode_count")?,
                        "preview_episodes": preview_episodes
                    });

                    playlists.push(playlist);
                }

                Ok(playlists)
            },
            mysql(pool) => {
                let query = r#"
                    SELECT
                        p.PlaylistID,
                        p.UserID,
                        p.Name,
                        p.Description,
                        p.IsSystemPlaylist,
                        p.PodcastIDs,
                        p.IncludeUnplayed,
                        p.IncludePartiallyPlayed,
                        p.IncludePlayed,
                        p.MinDuration,
                        p.MaxDuration,
                        p.SortOrder,
                        p.GroupByPodcast,
                        p.MaxEpisodes,
                        p.LastUpdated,
                        p.Created,
                        p.IconName,
                        COALESCE(p.EpisodeCount, 0) as episode_count
                    FROM Playlists p
                    WHERE p.UserID = ?
                    ORDER BY p.IsSystemPlaylist DESC, p.Name ASC
                "#;

                let rows = sqlx::query(query)
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?;

                let mut playlists = Vec::new();
                for row in rows {
                    let playlist_id: i32 = row.try_get("PlaylistID")?;
                    
                    // Get preview episodes
                    let preview_query = r#"
                        SELECT e.EpisodeTitle, e.EpisodeArtwork
                        FROM PlaylistContents pc
                        JOIN Episodes e ON pc.EpisodeID = e.EpisodeID
                        JOIN Podcasts p ON e.FeedID = p.FeedID
                        WHERE pc.PlaylistID = ?
                        AND p.UserID = ?
                        ORDER BY pc.Position
                        LIMIT 3
                    "#;

                    let preview_rows = sqlx::query(preview_query)
                        .bind(playlist_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?;

                    let mut preview_episodes = Vec::new();
                    for preview_row in preview_rows {
                        preview_episodes.push(serde_json::json!({
                            "title": preview_row.try_get::<Option<String>, _>("EpisodeTitle")?.unwrap_or_default(),
                            "artwork": preview_row.try_get::<Option<String>, _>("EpisodeArtwork")?.unwrap_or_default()
                        }));
                    }

                    // Process podcast_ids - in MySQL it might be stored as JSON string.
                    // MariaDB reports its JSON column type over the wire with a binary
                    // charset, so sqlx sees it as SQL type BLOB rather than VARCHAR/TEXT.
                    // Decoding directly as Option<String> then fails, and `.ok()` silently
                    // swallows that error so podcast_ids always reports as None here on
                    // installs where this column comes back as BLOB (same issue fixed for the
                    // smart-playlist code paths in #773/#913). Decode as raw bytes first, and
                    // only fall back to a direct String decode where the driver reports a text
                    // charset for this column.
                    let raw_podcast_ids: Option<String> = match row.try_get::<Option<Vec<u8>>, _>("PodcastIDs") {
                        Ok(bytes_opt) => bytes_opt.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
                        Err(_) => row.try_get("PodcastIDs").ok(),
                    };
                    let mut podcast_ids: Option<Vec<i32>> = None;
                    
                    if let Some(raw_ids) = raw_podcast_ids {
                        if !raw_ids.is_empty() {
                            // Try to parse as JSON first
                            if let Ok(parsed) = serde_json::from_str::<Vec<i32>>(&raw_ids) {
                                podcast_ids = Some(parsed);
                            } else {
                                // Handle other formats like comma-separated strings
                                let parsed: Result<Vec<i32>, _> = raw_ids
                                    .trim_matches(|c| c == '[' || c == ']' || c == '"' || c == '\'')
                                    .split(',')
                                    .filter(|s| !s.trim().is_empty())
                                    .map(|s| s.trim().parse::<i32>())
                                    .collect();
                                
                                if let Ok(ids) = parsed {
                                    podcast_ids = Some(ids);
                                }
                            }
                        }
                    }

                    let playlist = serde_json::json!({
                        "playlist_id": playlist_id,
                        "user_id": row.try_get::<i32, _>("UserID")?,
                        "name": row.try_get::<String, _>("Name")?,
                        "description": row.try_get::<Option<String>, _>("Description")?.unwrap_or_default(),
                        "is_system_playlist": row.try_get::<i8, _>("IsSystemPlaylist")? != 0,
                        "podcast_ids": podcast_ids,
                        "include_unplayed": row.try_get::<i8, _>("IncludeUnplayed")? != 0,
                        "include_partially_played": row.try_get::<i8, _>("IncludePartiallyPlayed")? != 0,
                        "include_played": row.try_get::<i8, _>("IncludePlayed")? != 0,
                        "min_duration": row.try_get::<Option<i32>, _>("MinDuration")?,
                        "max_duration": row.try_get::<Option<i32>, _>("MaxDuration")?,
                        "sort_order": row.try_get::<Option<String>, _>("SortOrder")?,
                        "group_by_podcast": row.try_get::<i8, _>("GroupByPodcast")? != 0,
                        "max_episodes": row.try_get::<Option<i32>, _>("MaxEpisodes")?,
                        "last_updated": row.try_get::<Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>, _>("LastUpdated")?.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()).unwrap_or_default(),
                        "created": row.try_get::<Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>, _>("Created")?.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()).unwrap_or_default(),
                        "icon_name": row.try_get::<Option<String>, _>("IconName")?.unwrap_or_default(),
                        "episode_count": row.try_get::<i64, _>("episode_count")?,
                        "preview_episodes": preview_episodes
                    });

                    playlists.push(playlist);
                }

                Ok(playlists)
            },
        })
    }

    // Create playlist - matches Python create_playlist function exactly
    pub async fn create_playlist(&self, _config: &Config, playlist_data: &crate::models::CreatePlaylistRequest) -> AppResult<i32> {
        let min_duration = playlist_data.min_duration.map(|d| d * 60);
        let max_duration = playlist_data.max_duration.map(|d| d * 60);
        
        dialect_match!(self, {
            pg(pool) => {
                let podcast_ids_array = if let Some(ref ids) = playlist_data.podcast_ids {
                    ids.clone()
                } else {
                    vec![]
                };

                let result = q::query(r#"
                    INSERT INTO "Playlists" (
                        userid,
                        name,
                        description,
                        issystemplaylist,
                        podcastids,
                        includeunplayed,
                        includepartiallyplayed,
                        includeplayed,
                        minduration,
                        maxduration,
                        sortorder,
                        groupbypodcast,
                        maxepisodes,
                        iconname,
                        playprogressmin,
                        playprogressmax,
                        timefilterhours
                    ) VALUES (
                        $1, $2, $3, FALSE, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16
                    ) RETURNING playlistid
                "#)
                .bind(playlist_data.user_id)
                .bind(&playlist_data.name)
                .bind(&playlist_data.description)
                .bind(&podcast_ids_array)
                .bind(playlist_data.include_unplayed)
                .bind(playlist_data.include_partially_played)
                .bind(playlist_data.include_played)
                .bind(min_duration)
                .bind(max_duration)
                .bind(&playlist_data.sort_order)
                .bind(playlist_data.group_by_podcast)
                .bind(playlist_data.max_episodes)
                .bind(&playlist_data.icon_name)
                .bind(playlist_data.play_progress_min)
                .bind(playlist_data.play_progress_max)
                .bind(playlist_data.time_filter_hours)
                .fetch_one(pool)
                .await?;

                let playlist_id = result.get::<i32, _>("playlistid");
                
                // Update playlist contents immediately like Python does
                self.update_playlist_contents(playlist_id).await?;
                
                Ok(playlist_id)
            },
            mysql(pool) => {
                let podcast_ids_json = if let Some(ref ids) = playlist_data.podcast_ids {
                    serde_json::to_string(ids)?
                } else {
                    "[]".to_string()
                };

                let result = sqlx::query(r#"
                    INSERT INTO Playlists (
                        UserID,
                        Name,
                        Description,
                        IsSystemPlaylist,
                        PodcastIDs,
                        IncludeUnplayed,
                        IncludePartiallyPlayed,
                        IncludePlayed,
                        MinDuration,
                        MaxDuration,
                        SortOrder,
                        GroupByPodcast,
                        MaxEpisodes,
                        IconName,
                        PlayProgressMin,
                        PlayProgressMax,
                        TimeFilterHours
                    ) VALUES (
                        ?, ?, ?, FALSE, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
                    )
                "#)
                .bind(playlist_data.user_id)
                .bind(&playlist_data.name)
                .bind(&playlist_data.description)
                .bind(&podcast_ids_json)
                .bind(playlist_data.include_unplayed)
                .bind(playlist_data.include_partially_played)
                .bind(playlist_data.include_played)
                .bind(min_duration)
                .bind(max_duration)
                .bind(&playlist_data.sort_order)
                .bind(playlist_data.group_by_podcast)
                .bind(playlist_data.max_episodes)
                .bind(&playlist_data.icon_name)
                .bind(playlist_data.play_progress_min)
                .bind(playlist_data.play_progress_max)
                .bind(playlist_data.time_filter_hours)
                .execute(pool)
                .await?;

                let playlist_id = result.last_insert_id() as i32;
                
                // Update playlist contents immediately like Python does
                self.update_playlist_contents(playlist_id).await?;
                
                Ok(playlist_id)
            },
        })
    }

    pub async fn get_next_playlist_episode(&self, episode_id: i32, playlist_id: i32, user_id: i32) -> AppResult<Option<crate::models::QueuedEpisode>> {
        // Use the same dynamic query as the playlist display so the playback order exactly
        // matches what the user sees in the UI. PlaylistContents was previously used, but it
        // can be stale (e.g. only refreshed on creation/podcast-refresh), causing the currently-
        // playing episode to appear at the last position or not at all, which surfaced as
        // "Playlist exhausted" immediately after the first episode finished.
        //
        // Strategy:
        //   1. Fetch up to 500 episodes via the dynamic query (same order as UI).
        //   2. Find the just-played episode_id in the list.
        //   3. If found: return the next one after it.
        //   4. If not found (episode was filtered out because it just completed):
        //      return the first episode that isn't the current one — i.e. the next
        //      uncompleted episode the user would see at the top of the playlist.
        let page = self.get_playlist_episodes_dynamic(playlist_id, user_id, 500, 0).await?;
        let episodes = page.episodes;

        let to_queued = |ep: &crate::models::SavedEpisode| crate::models::QueuedEpisode {
            episodeid: ep.episodeid,
            episodetitle: ep.episodetitle.clone(),
            podcastname: ep.podcastname.clone(),
            podcastid: ep.podcastid.unwrap_or(0),
            episodepubdate: ep.episodepubdate.clone(),
            episodedescription: ep.episodedescription.clone(),
            episodeartwork: ep.episodeartwork.clone(),
            episodeurl: ep.episodeurl.clone(),
            episodeduration: ep.episodeduration,
            listenduration: ep.listenduration,
            queueposition: None,
            queuedate: String::new(),
            completed: ep.completed,
            saved: ep.saved,
            queued: ep.queued,
            downloaded: ep.downloaded,
            is_youtube: ep.is_youtube,
            is_video: false,
        };

        // Try to find the current episode in the list.
        if let Some(pos) = episodes.iter().position(|ep| ep.episodeid == episode_id) {
            // Episode still in the list (not yet filtered as completed): return the next one.
            if let Some(next_ep) = episodes.get(pos + 1) {
                return Ok(Some(to_queued(next_ep)));
            }
            // Current episode was the last one — playlist exhausted.
            return Ok(None);
        }

        // Current episode not in list (filtered out after completion): play the first
        // remaining episode that isn't the current one.
        if let Some(next_ep) = episodes.iter().find(|ep| ep.episodeid != episode_id) {
            return Ok(Some(to_queued(next_ep)));
        }

        Ok(None)
    }

    // Update all playlists - matches Python update_all_playlists function exactly
    pub async fn update_all_playlists(&self) -> AppResult<()> {
        tracing::debug!("=================== PLAYLIST UPDATE STARTING ===================");
        tracing::info!("Starting to fetch all playlists");
        
        dialect_match!(self, {
            pg(pool) => {
                let playlists = q::query(r#"
                    SELECT playlistid, name, userid, podcastids, includeunplayed, 
                           includepartiallyplayed, includeplayed, playprogressmin, playprogressmax, 
                           timefilterhours, minduration, maxduration, sortorder, 
                           groupbypodcast, maxepisodes
                    FROM "Playlists"
                "#)
                    .fetch_all(pool)
                    .await?;
                
                tracing::info!("Found {} playlists to update", playlists.len());
                
                for playlist in playlists {
                    let playlist_id: i32 = playlist.try_get("playlistid")?;
                    let playlist_name: String = playlist.try_get("name")?;
                    let user_id: i32 = playlist.try_get("userid")?;
                    
                    tracing::info!("Updating playlist: {} (ID: {}, User: {})", playlist_name, playlist_id, user_id);
                    
                    match self.update_playlist_contents(playlist_id).await {
                        Ok(episode_count) => {
                            tracing::info!("Successfully updated playlist '{}': {} episodes", playlist_name, episode_count);
                        }
                        Err(e) => {
                            tracing::error!("Failed to update playlist '{}' (ID: {}): {}", playlist_name, playlist_id, e);
                            // Continue with other playlists
                        }
                    }
                }
            },
            mysql(pool) => {
                let playlists = sqlx::query("
                    SELECT PlaylistID, Name, UserID, PodcastIds, IncludeUnplayed, 
                           IncludePartiallyPlayed, IncludePlayed, PlayProgressMin, PlayProgressMax, 
                           TimeFilterHours, MinDuration, MaxDuration, SortOrder, 
                           GroupByPodcast, MaxEpisodes
                    FROM Playlists
                ")
                    .fetch_all(pool)
                    .await?;
                
                tracing::info!("Found {} playlists to update", playlists.len());
                
                for playlist in playlists {
                    let playlist_id: i32 = playlist.try_get("PlaylistID")?;
                    let playlist_name: String = playlist.try_get("Name")?;
                    let user_id: i32 = playlist.try_get("UserID")?;
                    
                    tracing::info!("Updating playlist: {} (ID: {}, User: {})", playlist_name, playlist_id, user_id);
                    
                    match self.update_playlist_contents(playlist_id).await {
                        Ok(episode_count) => {
                            tracing::info!("Successfully updated playlist '{}': {} episodes", playlist_name, episode_count);
                        }
                        Err(e) => {
                            tracing::error!("Failed to update playlist '{}' (ID: {}): {}", playlist_name, playlist_id, e);
                            // Continue with other playlists
                        }
                    }
                }
            },
        });
        
        tracing::debug!("=================== PLAYLIST UPDATE COMPLETED ===================");
        Ok(())
    }

    // Update playlist contents - matches Python update_playlist_contents function exactly
    pub async fn update_playlist_contents(&self, playlist_id: i32) -> AppResult<i32> {
        tracing::debug!("======= UPDATE PLAYLIST ID: {} =======", playlist_id);
        
        dialect_match!(self, {
            pg(pool) => {
                // Get playlist configuration first  
                let playlist = q::query(r#"
                    SELECT playlistid, name, userid, podcastids, includeunplayed, 
                           includepartiallyplayed, includeplayed, playprogressmin, playprogressmax, 
                           timefilterhours, minduration, maxduration, sortorder, 
                           groupbypodcast, maxepisodes, issystemplaylist
                    FROM "Playlists" WHERE playlistid = $1
                "#)
                    .bind(playlist_id)
                    .fetch_one(pool)
                    .await?;
                
                // Clear existing contents
                q::query(r#"DELETE FROM "PlaylistContents" WHERE playlistid = $1"#)
                    .bind(playlist_id)
                    .execute(pool)
                    .await?;
                
                // Handle special playlists
                let playlist_name: String = playlist.try_get("name")?;
                let is_system_playlist: bool = playlist.try_get("issystemplaylist").unwrap_or(false);
                
                let episode_count = if playlist_name == "Fresh Releases" && is_system_playlist {
                    // Special handling for Fresh Releases
                    q::pick(Self::update_fresh_releases_playlist_postgres, Self::update_fresh_releases_playlist_lite)(self, pool, playlist_id).await?
                } else if playlist_name == "Currently Listening" && is_system_playlist {
                    // Special handling for Currently Listening
                    q::pick(Self::update_currently_listening_playlist_postgres, Self::update_currently_listening_playlist_lite)(self, pool, playlist_id).await?
                } else if playlist_name == "Almost Done" && is_system_playlist {
                    // Special handling for Almost Done  
                    q::pick(Self::update_almost_done_playlist_postgres, Self::update_almost_done_playlist_lite)(self, pool, playlist_id).await?
                } else if playlist_name == "Quick Listens" && is_system_playlist {
                    // Special handling for Quick Listens
                    q::pick(Self::update_quick_listens_playlist_postgres, Self::update_quick_listens_playlist_lite)(self, pool, playlist_id).await?
                } else {
                    // Standard playlist query building
                    q::pick(Self::build_and_execute_playlist_query_postgres, Self::build_and_execute_playlist_query_lite)(self, pool, &playlist).await?
                };
                
                // Update timestamp
                q::query(r#"UPDATE "Playlists" SET lastupdated = CURRENT_TIMESTAMP WHERE playlistid = $1"#)
                    .bind(playlist_id)
                    .execute(pool)
                    .await?;
                
                Ok(episode_count)
            },
            mysql(pool) => {
                // Get playlist configuration first
                let playlist = sqlx::query("
                    SELECT PlaylistID, Name, UserID, PodcastIDs, IncludeUnplayed, 
                           IncludePartiallyPlayed, IncludePlayed, PlayProgressMin, PlayProgressMax, 
                           TimeFilterHours, MinDuration, MaxDuration, SortOrder, 
                           GroupByPodcast, MaxEpisodes, IsSystemPlaylist
                    FROM Playlists WHERE PlaylistID = ?
                ")
                    .bind(playlist_id)
                    .fetch_one(pool)
                    .await?;
                
                // Clear existing contents  
                sqlx::query("DELETE FROM PlaylistContents WHERE PlaylistID = ?")
                    .bind(playlist_id)
                    .execute(pool)
                    .await?;
                
                // Handle special playlists
                let playlist_name: String = playlist.try_get("Name")?;
                let is_system_playlist: bool = playlist.try_get("IsSystemPlaylist").unwrap_or(false);
                
                let episode_count = if playlist_name == "Fresh Releases" && is_system_playlist {
                    // Special handling for Fresh Releases
                    self.update_fresh_releases_playlist_mysql(pool, playlist_id).await?
                } else if playlist_name == "Currently Listening" && is_system_playlist {
                    // Special handling for Currently Listening
                    self.update_currently_listening_playlist_mysql(pool, playlist_id).await?
                } else if playlist_name == "Almost Done" && is_system_playlist {
                    // Special handling for Almost Done
                    self.update_almost_done_playlist_mysql(pool, playlist_id).await?
                } else if playlist_name == "Quick Listens" && is_system_playlist {
                    // Special handling for Quick Listens
                    self.update_quick_listens_playlist_mysql(pool, playlist_id).await?
                } else {
                    // Standard playlist query building
                    self.build_and_execute_playlist_query_mysql(pool, &playlist).await?
                };
                
                // Update timestamp
                sqlx::query("UPDATE Playlists SET LastUpdated = CURRENT_TIMESTAMP WHERE PlaylistID = ?")
                    .bind(playlist_id)
                    .execute(pool)
                    .await?;
                
                Ok(episode_count)
            },
        })
    }

    // Helper function to normalize timezone names for database compatibility
    pub(super) fn normalize_timezone(tz_str: &str) -> String {
        // Try to parse the timezone string with chrono-tz
        if let Ok(tz) = tz_str.parse::<Tz>() {
            // Return the canonical name for this timezone
            tz.name().to_string()
        } else {
            // If parsing fails, return UTC as fallback
            tracing::warn!("Unable to parse timezone '{}', falling back to UTC", tz_str);
            "UTC".to_string()
        }
    }

    // COMPLETE PLAYLIST SYSTEM IMPLEMENTATION - matches Python functionality exactly

    // Update Fresh Releases playlist with timezone-aware logic - matches Python update_fresh_releases_playlist
    // Update Currently Listening playlist - episodes that users have started but not finished
    // Update Almost Done playlist - episodes that users are 75%+ through
    // Update Quick Listens playlist - episodes under 15 minutes from ALL users' podcasts
    pub(super) async fn update_quick_listens_playlist_mysql(&self, pool: &Pool<MySql>, playlist_id: i32) -> AppResult<i32> {
        tracing::info!("Updating Quick Listens playlist");
        
        // Get shortest 1000 episodes under 15 minutes (900 seconds) and over 1 second from ALL podcasts
        let episodes = sqlx::query("
            SELECT e.EpisodeID
            FROM Episodes e
            JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = (SELECT UserID FROM Playlists WHERE PlaylistID = ?)
            WHERE e.EpisodeDuration >= 1
            AND e.EpisodeDuration <= 900
            AND COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) = FALSE
            ORDER BY e.EpisodeDuration ASC
            LIMIT 1000
        ")
            .bind(playlist_id)
            .fetch_all(pool)
            .await?;
        
        let mut position = 1;
        
        // Add episodes to playlist
        for episode in episodes {
            let episode_id: i32 = episode.try_get("EpisodeID")?;
            sqlx::query("INSERT INTO PlaylistContents (PlaylistID, EpisodeID, Position) VALUES (?, ?, ?)")
                .bind(playlist_id)
                .bind(episode_id)
                .bind(position)
                .execute(pool)
                .await?;
            
            position += 1;
        }
        
        let episode_count = position - 1;
        tracing::info!("Quick Listens playlist updated with {} episodes", episode_count);
        Ok(episode_count)
    }

    pub(super) async fn update_fresh_releases_playlist_mysql(&self, pool: &Pool<MySql>, playlist_id: i32) -> AppResult<i32> {
        tracing::info!("Updating Fresh Releases playlist with timezone logic");
        
        // Get all users with their timezones
        let users = sqlx::query("SELECT UserID, TimeZone FROM Users WHERE UserID = (SELECT UserID FROM Playlists WHERE PlaylistID = ?)")
            .bind(playlist_id)
            .fetch_all(pool)
            .await?;
        
        let mut added_episodes: std::collections::HashSet<i32> = std::collections::HashSet::new();
        let mut position = 1;
        
        for user in users {
            let user_id: i32 = user.try_get("UserID")?;
            let timezone: Option<String> = user.try_get("TimeZone").ok();
            let raw_tz = timezone.as_deref()
                .filter(|s| !s.is_empty()) // Filter out empty strings
                .unwrap_or("UTC");
            
            let normalized_tz = Self::normalize_timezone(raw_tz);
            
            tracing::info!("Processing Fresh Releases for user {} with timezone {} (normalized: {})", user_id, raw_tz, normalized_tz);
            
            // Get episodes from last 24 hours in user's timezone
            let episodes = sqlx::query("
                SELECT e.EpisodeID
                FROM Episodes e
                JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = ?
                WHERE CONVERT_TZ(e.EpisodePubDate, 'UTC', ?) >
                      DATE_SUB(CONVERT_TZ(NOW(), 'UTC', ?), INTERVAL 24 HOUR)
                ORDER BY e.EpisodePubDate DESC
            ")
                .bind(user_id)
                .bind(&normalized_tz)
                .bind(&normalized_tz)
                .fetch_all(pool)
                .await?;
            
            // Add unique episodes to playlist
            for episode in episodes {
                let episode_id: i32 = episode.try_get("EpisodeID")?;
                if !added_episodes.contains(&episode_id) {
                    sqlx::query("
                        INSERT INTO PlaylistContents (PlaylistID, EpisodeID, Position)
                        VALUES (?, ?, ?)
                    ")
                        .bind(playlist_id)
                        .bind(episode_id)
                        .bind(position)
                        .execute(pool)
                        .await?;
                    
                    added_episodes.insert(episode_id);
                    position += 1;
                }
            }
        }
        
        tracing::info!("Fresh Releases playlist updated with {} episodes", added_episodes.len());
        Ok(added_episodes.len() as i32)
    }

    pub(super) async fn update_currently_listening_playlist_mysql(&self, pool: &Pool<MySql>, playlist_id: i32) -> AppResult<i32> {
        tracing::info!("Updating Currently Listening playlist");
        
        // Clear existing playlist contents
        sqlx::query("DELETE FROM PlaylistContents WHERE PlaylistID = ?")
            .bind(playlist_id)
            .execute(pool)
            .await?;
        
        // Get all users
        let users = sqlx::query("SELECT UserID FROM Users WHERE UserID = (SELECT UserID FROM Playlists WHERE PlaylistID = ?)")
            .bind(playlist_id)
            .fetch_all(pool)
            .await?;
        
        let mut added_episodes: std::collections::HashSet<i32> = std::collections::HashSet::new();
        let mut position = 1;
        
        for user in users {
            let user_id: i32 = user.try_get("UserID")?;
            
            tracing::info!("Processing Currently Listening for user {}", user_id);
            
            // Get episodes that are currently being listened to (started but not completed)
            let episodes = sqlx::query("
                SELECT e.EpisodeID
                FROM Episodes e
                JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = ?
                JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = h.UserID
                WHERE h.ListenDuration > 0 AND h.ListenDuration < e.EpisodeDuration
                ORDER BY h.ListenDate DESC
            ")
                .bind(user_id)
                .fetch_all(pool)
                .await?;
            
            // Add unique episodes to playlist
            for episode in episodes {
                let episode_id: i32 = episode.try_get("EpisodeID")?;
                if !added_episodes.contains(&episode_id) {
                    sqlx::query("
                        INSERT INTO PlaylistContents (PlaylistID, EpisodeID, Position)
                        VALUES (?, ?, ?)
                    ")
                        .bind(playlist_id)
                        .bind(episode_id)
                        .bind(position)
                        .execute(pool)
                        .await?;
                    
                    added_episodes.insert(episode_id);
                    position += 1;
                }
            }
        }
        
        tracing::info!("Currently Listening playlist updated with {} episodes", added_episodes.len());
        Ok(added_episodes.len() as i32)
    }

    pub(super) async fn update_almost_done_playlist_mysql(&self, pool: &Pool<MySql>, playlist_id: i32) -> AppResult<i32> {
        tracing::info!("Updating Almost Done playlist");
        
        // Clear existing playlist contents
        sqlx::query("DELETE FROM PlaylistContents WHERE PlaylistID = ?")
            .bind(playlist_id)
            .execute(pool)
            .await?;
        
        // Get all users
        let users = sqlx::query("SELECT UserID FROM Users WHERE UserID = (SELECT UserID FROM Playlists WHERE PlaylistID = ?)")
            .bind(playlist_id)
            .fetch_all(pool)
            .await?;
        
        let mut added_episodes: std::collections::HashSet<i32> = std::collections::HashSet::new();
        let mut position = 1;
        
        for user in users {
            let user_id: i32 = user.try_get("UserID")?;
            
            tracing::info!("Processing Almost Done for user {}", user_id);
            
            // Get episodes that are almost done (75%+ listened and not completed)
            let episodes = sqlx::query("
                SELECT e.EpisodeID
                FROM Episodes e
                JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = ?
                JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = h.UserID
                WHERE h.ListenDuration > 0 
                  AND h.ListenDuration < e.EpisodeDuration 
                  AND (h.ListenDuration / NULLIF(e.EpisodeDuration, 0)) >= 0.75
                ORDER BY h.ListenDate DESC
            ")
                .bind(user_id)
                .fetch_all(pool)
                .await?;
            
            // Add unique episodes to playlist
            for episode in episodes {
                let episode_id: i32 = episode.try_get("EpisodeID")?;
                if !added_episodes.contains(&episode_id) {
                    sqlx::query("
                        INSERT INTO PlaylistContents (PlaylistID, EpisodeID, Position)
                        VALUES (?, ?, ?)
                    ")
                        .bind(playlist_id)
                        .bind(episode_id)
                        .bind(position)
                        .execute(pool)
                        .await?;
                    
                    added_episodes.insert(episode_id);
                    position += 1;
                }
            }
        }
        
        tracing::info!("Almost Done playlist updated with {} episodes", added_episodes.len());
        Ok(added_episodes.len() as i32)
    }

    // Build and execute playlist query for PostgreSQL - matches Python build_playlist_query exactly
    // Build and execute playlist query for MySQL - matches Python build_playlist_query exactly  
    pub(super) async fn build_and_execute_playlist_query_mysql(&self, pool: &Pool<MySql>, playlist: &sqlx::mysql::MySqlRow) -> AppResult<i32> {
        let playlist_id: i32 = playlist.try_get("PlaylistID")?;
        let user_id: i32 = playlist.try_get("UserID")?;
        let playlist_name: String = playlist.try_get("Name")?;
        let is_system_playlist: bool = playlist.try_get("IsSystemPlaylist").unwrap_or(false);
        
        // Parse playlist configuration
        let config = PlaylistConfig::from_mysql_row(playlist)?;
        
        // Determine if this playlist needs user history filtering
        let needs_user_history = playlist_name == "Currently Listening" || 
                                  playlist_name == "Almost Done" || 
                                  !is_system_playlist;
        
        // Check for special optimized queries for partially played
        if config.include_partially_played && !config.include_unplayed && !config.include_played {
            return self.execute_partially_played_query_mysql(pool, playlist_id, user_id, &config).await;
        }
        
        // Build the appropriate base query
        let (base_query, params) = if is_system_playlist {
            if needs_user_history {
                // System playlist with user history filtering
                ("
                    SELECT e.EpisodeID
                    FROM Episodes e
                    JOIN Podcasts p ON e.FeedID = p.FeedID
                    LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = ?
                    JOIN Users u ON u.UserID = ?
                    WHERE p.UserID = u.UserID
                ".to_string(), vec![user_id, user_id])
            } else {
                // System playlist without user history filtering (Fresh Releases, etc.)
                // But still need user context for history when needed
                ("
                    SELECT e.EpisodeID
                    FROM Episodes e
                    JOIN Podcasts p ON e.FeedID = p.FeedID
                    LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = ?
                    JOIN Users u ON u.UserID = ?
                    WHERE p.UserID = u.UserID
                ".to_string(), vec![user_id, user_id])
            }
        } else {
            // User-specific playlist
            ("
                SELECT e.EpisodeID
                FROM Episodes e
                JOIN Podcasts p ON e.FeedID = p.FeedID
                LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = ?
                JOIN Users u ON u.UserID = ?
                WHERE p.UserID = ?
            ".to_string(), vec![user_id, user_id, user_id])
        };
        
        // Build the complete query with all filters
        let (complete_query, all_params) = self.build_complete_mysql_query(
            base_query, params, &config, playlist_id
        )?;
        
        // Execute the query and insert episodes
        self.execute_playlist_query_mysql(pool, &complete_query, &all_params, playlist_id).await
    }

    // Execute optimized partially played query for PostgreSQL - FIXED VERSION
    // Execute optimized partially played query for MySQL
    pub(super) async fn execute_partially_played_query_mysql(&self, pool: &Pool<MySql>, playlist_id: i32, user_id: i32, config: &PlaylistConfig) -> AppResult<i32> {
        // Use direct INSERT without subquery for this optimized case - no alias scoping issues
        let sort_order = config.get_mysql_sort_order().replace("ORDER BY ", "");
        
        let query = format!("
            INSERT INTO PlaylistContents (PlaylistID, EpisodeID, Position)
            SELECT ?, e.EpisodeID, ROW_NUMBER() OVER (ORDER BY {}) as position
            FROM Episodes e
            JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID
            JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = h.UserID
            WHERE h.ListenDuration > 0
            AND h.ListenDuration < e.EpisodeDuration
            AND COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) = FALSE
            AND e.EpisodeDuration > 0
            AND h.UserID = ?
        ", sort_order);
        
        // For simplicity, execute basic version - full implementation would add all filters
        let result = sqlx::query(sqlx::AssertSqlSafe(query.as_str()))
            .bind(playlist_id)
            .bind(user_id)
            .execute(pool)
            .await?;
        
        Ok(result.rows_affected() as i32)
    }

    // Build complete PostgreSQL query with all filters - EXACT PYTHON MATCH
    pub(super) fn build_complete_postgres_query(&self, base_query: String, params: Vec<i32>, config: &PlaylistConfig, playlist_id: i32) -> AppResult<(String, Vec<i32>)> {
        // Build proper SELECT query first - need to include columns for ordering in subquery
        let select_columns = vec![
            "e.episodeid".to_string(),
            "p.podcastid".to_string(),
            "e.episodepubdate".to_string(),
            "e.episodeduration".to_string(),
            "COALESCE(h.listenduration, 0) as listenduration".to_string()
        ];
        
        let mut select_query = base_query.replace(
            "SELECT e.episodeid, p.podcastid, u.timezone",
            &format!("SELECT {}", select_columns.join(", "))
        );
        
        let mut all_params = params;
        let mut param_index = all_params.len() + 2; // +2 because playlist_id will be inserted as $1
        
        // Add podcast filter (PostgreSQL IN clause support)
        if let Some(ref podcast_ids) = config.podcast_ids {
            info!("Playlist {}: Applying podcast filter with IDs: {:?}", playlist_id, podcast_ids);
            if !podcast_ids.is_empty() {
                if podcast_ids.len() == 1 {
                    info!("PostgreSQL single podcast filter: p.podcastid = {}", podcast_ids[0]);
                    select_query.push_str(&format!(" AND p.podcastid = ${}", param_index));
                    all_params.push(podcast_ids[0]);
                    param_index += 1;
                } else {
                    let placeholders: String = (0..podcast_ids.len())
                        .map(|i| format!("${}", param_index + i))
                        .collect::<Vec<_>>()
                        .join(",");
                    info!("PostgreSQL multiple podcast filter: p.podcastid IN ({})", placeholders);
                    select_query.push_str(&format!(" AND p.podcastid IN ({})", placeholders));
                    all_params.extend(podcast_ids);
                    param_index += podcast_ids.len();
                }
            } else {
                // If podcast_ids is Some but empty, user selected specific podcasts but list is empty
                // This should return no results (exclude all podcasts)
                info!("PostgreSQL podcast filter is empty - excluding all podcasts");
                select_query.push_str(" AND FALSE");
            }
        } else {
            info!("PostgreSQL no podcast filter applied - config.podcast_ids is None");
        }
        
        // Add duration filters
        if let Some(min_duration) = config.min_duration {
            select_query.push_str(&format!(" AND e.episodeduration >= ${}", param_index));
            all_params.push(min_duration);
            param_index += 1;
        }
        
        if let Some(max_duration) = config.max_duration {
            select_query.push_str(&format!(" AND e.episodeduration <= ${}", param_index));
            all_params.push(max_duration);
        }

        // Add time filter with timezone awareness
        if let Some(time_filter_hours) = config.time_filter_hours {
            select_query.push_str(&format!(
                " AND e.episodepubdate AT TIME ZONE 'UTC' AT TIME ZONE COALESCE(u.timezone, 'UTC') > \
                 (CURRENT_TIMESTAMP AT TIME ZONE 'UTC' AT TIME ZONE COALESCE(u.timezone, 'UTC') - INTERVAL '{}' HOUR)",
                time_filter_hours
            ));
        }
        
        // Add play state filters - EXACT PYTHON LOGIC
        info!("Playlist {}: Applying play state filters - unplayed: {}, partially_played: {}, played: {}", 
                playlist_id, config.include_unplayed, config.include_partially_played, config.include_played);
        let mut play_state_conditions = Vec::new();
        
        if config.include_unplayed {
            play_state_conditions.push("(h.listenduration IS NULL AND COALESCE((SELECT ues.completed FROM \"UserEpisodeState\" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) IS NOT TRUE)".to_string());
            debug!("Playlist {}: Added unplayed episode filter", playlist_id);
        }
        
        if config.include_partially_played {
            let mut partial_condition = "(h.listenduration > 0 AND h.listenduration < e.episodeduration AND COALESCE((SELECT ues.completed FROM \"UserEpisodeState\" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) = FALSE)".to_string();
            
            if let Some(min_progress) = config.play_progress_min {
                partial_condition.push_str(&format!(" AND (h.listenduration::float / NULLIF(e.episodeduration, 0)) >= {}", min_progress / 100.0));
            }
            
            if let Some(max_progress) = config.play_progress_max {
                partial_condition.push_str(&format!(" AND (h.listenduration::float / NULLIF(e.episodeduration, 0)) <= {}", max_progress / 100.0));
            }
            
            play_state_conditions.push(partial_condition);
        }
        
        if config.include_played {
            play_state_conditions.push("(COALESCE((SELECT ues.completed FROM \"UserEpisodeState\" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) = TRUE OR h.listenduration >= e.episodeduration)".to_string());
            tracing::debug!("Playlist {}: Added played/completed episode filter", playlist_id);
        }

        if !play_state_conditions.is_empty() {
            select_query.push_str(&format!(" AND ({})", play_state_conditions.join(" OR ")));
            tracing::debug!("Playlist {}: Applied play state filter: ({})", playlist_id, play_state_conditions.join(" OR "));
        } else {
            // If no play states are selected, exclude all episodes (return no results)
            select_query.push_str(" AND FALSE");
            tracing::debug!("Playlist {}: No play states selected - excluding all episodes", playlist_id);
        }

        // Explicitly exclude completed episodes when include_played is false
        if !config.include_played {
            select_query.push_str(" AND COALESCE((SELECT ues.completed FROM \"UserEpisodeState\" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) = FALSE");
            tracing::debug!("Playlist {}: Excluding all completed episodes (include_played=false)", playlist_id);
        }
        
        // Note: No ORDER BY in inner query - final sorting is handled by ROW_NUMBER() OVER clause
        
        // Add limit
        if let Some(max_episodes) = config.max_episodes {
            select_query.push_str(&format!(" LIMIT {}", max_episodes));
        }
        
        // Now wrap the SELECT query in INSERT with ROW_NUMBER() - FIXED ALIAS SCOPING
        info!("Playlist {}: group_by_podcast setting: {}", playlist_id, config.group_by_podcast);
        let sort_for_insert = if config.group_by_podcast {
            let sort_with_grouping = format!("ORDER BY episodes.podcastid, {}", config.get_postgres_outer_sort_order().replace("ORDER BY ", ""));
            info!("Playlist {}: Using grouped sort: {}", playlist_id, sort_with_grouping);
            sort_with_grouping
        } else {
            let sort_without_grouping = config.get_postgres_outer_sort_order();
            info!("Playlist {}: Using non-grouped sort: {}", playlist_id, sort_without_grouping);
            debug!("Playlist {}: Debug - raw sort_order: '{}'", playlist_id, config.sort_order);
            sort_without_grouping
        };
        
        let insert_query = format!(r#"
            INSERT INTO "PlaylistContents" (playlistid, episodeid, position)
            SELECT $1, episodes.episodeid, ROW_NUMBER() OVER ({}) as position
            FROM ({}) episodes
        "#, sort_for_insert, select_query);
        
        info!("Playlist {}: Final insert query: {}", playlist_id, insert_query);
        
        // Final params: playlist_id first, then all query params
        let mut final_params = vec![playlist_id];
        final_params.extend(all_params);
        
        Ok((insert_query, final_params))
    }

    // Build complete MySQL query with all filters - EXACT PYTHON MATCH
    pub(super) fn build_complete_mysql_query(&self, base_query: String, params: Vec<i32>, config: &PlaylistConfig, playlist_id: i32) -> AppResult<(String, Vec<i32>)> {
        // Build proper SELECT query first - need to include columns for ordering in subquery
        let select_columns = vec![
            "e.EpisodeID".to_string(),
            "p.PodcastID".to_string(),
            "e.EpisodePubDate".to_string(),
            "e.EpisodeDuration".to_string(),
            "COALESCE(h.ListenDuration, 0) as ListenDuration".to_string()
        ];
        
        let mut select_query = base_query.replace(
            "SELECT e.EpisodeID",
            &format!("SELECT {}", select_columns.join(", "))
        );
        
        let mut all_params = params;
        
        // Add podcast filter (MySQL JSON/IN support)
        if let Some(ref podcast_ids) = config.podcast_ids {
            if !podcast_ids.is_empty() {
                info!("MySQL applying podcast filter with IDs: {:?}", podcast_ids);
                if podcast_ids.len() == 1 {
                    select_query.push_str(" AND p.PodcastID = ?");
                    all_params.push(podcast_ids[0]);
                    info!("MySQL single podcast filter: p.PodcastID = {}", podcast_ids[0]);
                } else {
                    let placeholders: String = podcast_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
                    select_query.push_str(&format!(" AND p.PodcastID IN ({})", placeholders));
                    all_params.extend(podcast_ids);
                    info!("MySQL multiple podcast filter: p.PodcastID IN ({})", placeholders);
                }
            } else {
                // If podcast_ids is Some but empty, user selected specific podcasts but list is empty
                // This should return no results (exclude all podcasts)
                info!("MySQL podcast filter is empty - excluding all podcasts");
                select_query.push_str(" AND FALSE");
            }
        } else {
            info!("MySQL no podcast_ids specified, no filter applied");
        }
        
        // Add duration filters
        if let Some(min_duration) = config.min_duration {
            select_query.push_str(" AND e.EpisodeDuration >= ?");
            all_params.push(min_duration);
        }
        
        if let Some(max_duration) = config.max_duration {
            select_query.push_str(" AND e.EpisodeDuration <= ?");
            all_params.push(max_duration);
        }
        
        // Add time filter with timezone awareness
        if let Some(time_filter_hours) = config.time_filter_hours {
            select_query.push_str(&format!(
                " AND CONVERT_TZ(e.EpisodePubDate, 'UTC', COALESCE(u.TimeZone, 'UTC')) > \
                 DATE_SUB(CONVERT_TZ(NOW(), 'UTC', COALESCE(u.TimeZone, 'UTC')), INTERVAL {} HOUR)",
                time_filter_hours
            ));
        }
        
        // Add play state filters - EXACT PYTHON LOGIC
        let mut play_state_conditions = Vec::new();
        
        if config.include_unplayed {
            play_state_conditions.push("(h.ListenDuration IS NULL AND COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) IS NOT TRUE)".to_string());
        }
        
        if config.include_partially_played {
            let mut partial_condition = "(h.ListenDuration > 0 AND h.ListenDuration < e.EpisodeDuration AND COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) = FALSE)".to_string();
            
            if let Some(min_progress) = config.play_progress_min {
                partial_condition.push_str(&format!(" AND (h.ListenDuration / NULLIF(e.EpisodeDuration, 0)) >= {}", min_progress / 100.0));
            }
            
            if let Some(max_progress) = config.play_progress_max {
                partial_condition.push_str(&format!(" AND (h.ListenDuration / NULLIF(e.EpisodeDuration, 0)) <= {}", max_progress / 100.0));
            }
            
            play_state_conditions.push(partial_condition);
        }
        
        if config.include_played {
            play_state_conditions.push("(COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) = TRUE OR h.ListenDuration >= e.EpisodeDuration)".to_string());
        }

        if !play_state_conditions.is_empty() {
            select_query.push_str(&format!(" AND ({})", play_state_conditions.join(" OR ")));
        } else {
            // If no play states are selected, exclude all episodes (return no results)
            select_query.push_str(" AND FALSE");
        }

        // Explicitly exclude completed episodes when include_played is false
        if !config.include_played {
            select_query.push_str(" AND COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) = FALSE");
        }
        
        // Note: No ORDER BY in inner query - final sorting is handled by ROW_NUMBER() OVER clause
        
        // Add limit
        if let Some(max_episodes) = config.max_episodes {
            select_query.push_str(&format!(" LIMIT {}", max_episodes));
        }
        
        // Now wrap the SELECT query in INSERT with ROW_NUMBER() - FIXED ALIAS SCOPING
        info!("Playlist {}: group_by_podcast setting: {}", playlist_id, config.group_by_podcast);
        let sort_for_insert = if config.group_by_podcast {
            let sort_with_grouping = format!("ORDER BY episodes.PodcastID, {}", config.get_mysql_outer_sort_order().replace("ORDER BY ", ""));
            info!("Playlist {}: Using grouped sort: {}", playlist_id, sort_with_grouping);
            sort_with_grouping
        } else {
            let sort_without_grouping = config.get_mysql_outer_sort_order();
            info!("Playlist {}: Using non-grouped sort: {}", playlist_id, sort_without_grouping);
            sort_without_grouping
        };
        
        let insert_query = format!(r#"
            INSERT INTO PlaylistContents (PlaylistID, EpisodeID, Position)
            SELECT ?, episodes.EpisodeID, ROW_NUMBER() OVER ({}) as position
            FROM ({}) episodes
        "#, sort_for_insert, select_query);
        
        // Final params: playlist_id first, then all query params
        let mut final_params = vec![playlist_id];
        final_params.extend(all_params);
        
        Ok((insert_query, final_params))
    }

    // Execute the final playlist query for PostgreSQL - FIXED VERSION
    // Execute the final playlist query for MySQL
    pub(super) async fn execute_playlist_query_mysql(&self, pool: &Pool<MySql>, query: &str, params: &[i32], _playlist_id: i32) -> AppResult<i32> {
        tracing::info!("Executing MySQL playlist query with {} parameters", params.len());
        tracing::debug!("Query: {}", query);
        tracing::debug!("Params: {:?}", params);
        
        // Build query with proper parameter binding
        let query_owned = query.to_owned();
        let mut sqlx_query = sqlx::query(sqlx::AssertSqlSafe(query_owned.as_str()));
        for param in params {
            sqlx_query = sqlx_query.bind(*param);
        }

        let result = sqlx_query.execute(pool)
            .await?;
        
        Ok(result.rows_affected() as i32)
    }

    // Get playlist episodes - matches Python get_playlist_episodes function exactly
    pub async fn get_playlist_episodes(&self, user_id: i32, playlist_id: i32) -> AppResult<serde_json::Value> {
        dialect_match!(self, {
            pg(pool) => {
                // Get playlist info with episode count - matches Python exactly
                let playlist_row = q::query(r#"
                    SELECT
                        p.name,
                        p.description,
                        (SELECT COUNT(*)
                            FROM "PlaylistContents" pc
                            JOIN "Episodes" e ON pc.episodeid = e.episodeid
                            JOIN "Podcasts" pod ON e.feedid = pod.feedid
                            LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = $1
                            WHERE pc.playlistid = p.playlistid
                            AND (p.issystemplaylist = FALSE OR
                                (p.issystemplaylist = TRUE AND
                                (h.episodeid IS NOT NULL OR pod.userid = $2)))
                        ) as episode_count,
                        p.iconname,
                        p.issystemplaylist
                    FROM "Playlists" p
                    WHERE p.playlistid = $3 AND (p.userid = $4 OR p.issystemplaylist = TRUE)
                "#)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(playlist_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if playlist_row.is_none() {
                    return Err(AppError::not_found("Playlist not found"));
                }

                let row = playlist_row.unwrap();
                let playlist_name: String = row.try_get("name")?;
                let playlist_description: String = row.try_get("description").unwrap_or_default();
                let episode_count: i64 = row.try_get("episodecount")?;
                let icon_name: String = row.try_get("iconname").unwrap_or_default();
                let is_system_playlist: bool = row.try_get("issystemplaylist")?;

                let episodes_rows = if is_system_playlist {
                    // For system playlists, update playlist first to get current episodes, then filter to user's podcasts
                    self.update_playlist_contents(playlist_id).await?;
                    
                    // Same query as user playlists but with additional filter for user's podcasts
                    q::query(r#"
                        SELECT DISTINCT
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
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            "PlaylistContents".dateadded
                        FROM "PlaylistContents"
                        JOIN "Episodes" ON "PlaylistContents".episodeid = "Episodes".episodeid
                        JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                        LEFT JOIN "UserEpisodeHistory" ON "Episodes".episodeid = "UserEpisodeHistory".episodeid
                            AND "UserEpisodeHistory".userid = $1
                        LEFT JOIN "SavedEpisodes" ON "Episodes".episodeid = "SavedEpisodes".episodeid
                            AND "SavedEpisodes".userid = $1
                        LEFT JOIN "EpisodeQueue" ON "Episodes".episodeid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $1
                            AND "EpisodeQueue".is_youtube = FALSE
                        LEFT JOIN "DownloadedEpisodes" ON "Episodes".episodeid = "DownloadedEpisodes".episodeid
                            AND "DownloadedEpisodes".userid = $1
                        WHERE "PlaylistContents".playlistid = $2 
                            AND "Podcasts".userid = $1
                        ORDER BY "PlaylistContents".dateadded DESC
                    "#)
                        .bind(user_id)
                        .bind(playlist_id)
                        .fetch_all(pool)
                        .await?
                } else {
                    // For user playlists, use existing PlaylistContents logic
                    q::query(r#"
                        SELECT DISTINCT
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
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            "PlaylistContents".dateadded
                        FROM "PlaylistContents"
                        JOIN "Episodes" ON "PlaylistContents".episodeid = "Episodes".episodeid
                        JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid AND "Podcasts".userid = $1
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                        LEFT JOIN "UserEpisodeHistory" ON "Episodes".episodeid = "UserEpisodeHistory".episodeid
                            AND "UserEpisodeHistory".userid = $1
                        LEFT JOIN "SavedEpisodes" ON "Episodes".episodeid = "SavedEpisodes".episodeid
                            AND "SavedEpisodes".userid = $1
                        LEFT JOIN "EpisodeQueue" ON "Episodes".episodeid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $1
                            AND "EpisodeQueue".is_youtube = FALSE
                        LEFT JOIN "DownloadedEpisodes" ON "Episodes".episodeid = "DownloadedEpisodes".episodeid
                            AND "DownloadedEpisodes".userid = $1
                        WHERE "PlaylistContents".playlistid = $2
                        ORDER BY "PlaylistContents".dateadded DESC
                    "#)
                        .bind(user_id)
                        .bind(playlist_id)
                        .fetch_all(pool)
                        .await?
                };

                let mut episodes = Vec::new();
                for row in episodes_rows {
                    let episodeid: i32 = row.try_get("episodeid")?;
                    let episodetitle: String = row.try_get("episodetitle")?;
                    let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                    let episodepubdate = naive.format("%Y-%m-%dT%H:%M:%S").to_string();
                    let episodedescription: String = row.try_get("episodedescription")?;
                    let episodeartwork: String = row.try_get("episodeartwork")?;
                    let episodeurl: String = row.try_get("episodeurl")?;
                    let episodeduration: i32 = row.try_get("episodeduration")?;
                    let completed: bool = row.try_get("completed")?;
                    let podcastname: String = row.try_get("podcastname")?;
                    let podcastid: i32 = row.try_get("podcastid")?;
                    let is_youtube: bool = row.try_get("is_youtube")?;
                    let listenduration: Option<i32> = row.try_get("listenduration")?;
                    let saved: bool = row.try_get("saved")?;
                    let queued: bool = row.try_get("queued")?;
                    let downloaded: bool = row.try_get("downloaded")?;
                    let dateadded_naive = row.try_get::<chrono::NaiveDateTime, _>("dateadded")?;
                    let dateadded = dateadded_naive.format("%Y-%m-%dT%H:%M:%S").to_string();

                    episodes.push(serde_json::json!({
                        "episodeid": episodeid,
                        "episodetitle": episodetitle,
                        "episodepubdate": episodepubdate,
                        "episodedescription": episodedescription,
                        "episodeartwork": episodeartwork,
                        "episodeurl": episodeurl,
                        "episodeduration": episodeduration,
                        "completed": completed,
                        "podcastname": podcastname,
                        "podcastid": podcastid,
                        "is_youtube": is_youtube,
                        "listenduration": listenduration,
                        "saved": saved,
                        "queued": queued,
                        "downloaded": downloaded,
                        "dateadded": dateadded
                    }));
                }

                // Build playlist_info structure matching Python exactly
                let playlist_info = serde_json::json!({
                    "name": playlist_name,
                    "description": playlist_description,
                    "episode_count": episode_count,
                    "icon_name": icon_name
                });

                Ok(serde_json::json!({
                    "playlist_info": playlist_info,
                    "episodes": episodes
                }))
            },
            mysql(pool) => {
                // Get playlist info with episode count - matches Python exactly
                let playlist_row = sqlx::query(
                    "SELECT
                        p.Name,
                        p.Description,
                        (SELECT COUNT(*)
                            FROM PlaylistContents pc
                            JOIN Episodes e ON pc.EpisodeID = e.EpisodeID
                            JOIN Podcasts pod ON e.FeedID = pod.FeedID
                            LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = ?
                            WHERE pc.PlaylistID = p.PlaylistID
                            AND (p.IsSystemPlaylist = 0 OR
                                (p.IsSystemPlaylist = 1 AND
                                (h.EpisodeID IS NOT NULL OR pod.UserID = ?)))
                        ) as episode_count,
                        p.IconName,
                        p.IsSystemPlaylist
                    FROM Playlists p
                    WHERE p.PlaylistID = ? AND (p.UserID = ? OR p.IsSystemPlaylist = 1)"
                )
                    .bind(user_id)
                    .bind(user_id)
                    .bind(playlist_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if playlist_row.is_none() {
                    return Err(AppError::not_found("Playlist not found"));
                }

                let row = playlist_row.unwrap();
                let playlist_name: String = row.try_get("Name")?;
                let playlist_description: String = row.try_get("Description").unwrap_or_default();
                let episode_count: i64 = row.try_get("episode_count")?;
                let icon_name: String = row.try_get("IconName").unwrap_or_default();
                let is_system_playlist: bool = row.try_get::<i8, _>("IsSystemPlaylist")? != 0;

                let episodes_rows = if is_system_playlist {
                    // For system playlists, update playlist first to get current episodes, then filter to user's podcasts
                    self.update_playlist_contents(playlist_id).await?;
                    
                    // Same query as user playlists but with additional filter for user's podcasts
                    sqlx::query(
                        "SELECT DISTINCT
                            e.EpisodeID as episodeid,
                            e.EpisodeTitle as episodetitle,
                            e.EpisodePubDate as episodepubdate,
                            e.EpisodeDescription as episodedescription,
                            CASE 
                                WHEN p.UsePodcastCoversCustomized = TRUE AND p.UsePodcastCovers = TRUE THEN p.ArtworkURL
                                WHEN u.UsePodcastCovers = TRUE THEN p.ArtworkURL
                                ELSE e.EpisodeArtwork
                            END as episodeartwork,
                            e.EpisodeURL as episodeurl,
                            e.EpisodeDuration as episodeduration,
                            COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) as completed,
                            p.PodcastName as podcastname,
                            p.PodcastID as podcastid,
                            p.IsYouTubeChannel as is_youtube,
                            ueh.ListenDuration as listenduration,
                            CASE WHEN se.EpisodeID IS NOT NULL THEN 1 ELSE 0 END AS saved,
                            CASE WHEN eq.EpisodeID IS NOT NULL THEN 1 ELSE 0 END AS queued,
                            CASE WHEN de.EpisodeID IS NOT NULL THEN 1 ELSE 0 END AS downloaded,
                            pc.DateAdded as addeddate
                         FROM PlaylistContents pc
                         JOIN Episodes e ON pc.EpisodeID = e.EpisodeID
                         JOIN Podcasts p ON e.FeedID = p.FeedID
                         LEFT JOIN Users u ON p.UserID = u.UserID
                         LEFT JOIN UserEpisodeHistory ueh ON e.EpisodeID = ueh.EpisodeID AND ueh.UserID = ?
                         LEFT JOIN SavedEpisodes se ON e.EpisodeID = se.EpisodeID AND se.UserID = ?
                         LEFT JOIN EpisodeQueue eq ON e.EpisodeID = eq.EpisodeID AND eq.UserID = ? AND eq.is_youtube = 0
                         LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID AND de.UserID = ?
                         WHERE pc.PlaylistID = ? AND p.UserID = ?
                         ORDER BY pc.DateAdded DESC"
                    )
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(playlist_id)
                        .bind(user_id)
                        .fetch_all(pool)
                        .await?
                } else {
                    // For user playlists, use existing PlaylistContents logic
                    sqlx::query(
                        "SELECT DISTINCT
                            e.EpisodeID as episodeid,
                            e.EpisodeTitle as episodetitle,
                            e.EpisodePubDate as episodepubdate,
                            e.EpisodeDescription as episodedescription,
                            CASE 
                                WHEN p.UsePodcastCoversCustomized = TRUE AND p.UsePodcastCovers = TRUE THEN p.ArtworkURL
                                WHEN u.UsePodcastCovers = TRUE THEN p.ArtworkURL
                                ELSE e.EpisodeArtwork
                            END as episodeartwork,
                            e.EpisodeURL as episodeurl,
                            e.EpisodeDuration as episodeduration,
                            COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) as completed,
                            p.PodcastName as podcastname,
                            p.PodcastID as podcastid,
                            p.IsYouTubeChannel as is_youtube,
                            ueh.ListenDuration as listenduration,
                            CASE WHEN se.EpisodeID IS NOT NULL THEN 1 ELSE 0 END AS saved,
                            CASE WHEN eq.EpisodeID IS NOT NULL THEN 1 ELSE 0 END AS queued,
                            CASE WHEN de.EpisodeID IS NOT NULL THEN 1 ELSE 0 END AS downloaded,
                            pc.DateAdded as addeddate
                         FROM PlaylistContents pc
                         JOIN Episodes e ON pc.EpisodeID = e.EpisodeID
                         JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = ?
                         LEFT JOIN Users u ON p.UserID = u.UserID
                         LEFT JOIN UserEpisodeHistory ueh ON e.EpisodeID = ueh.EpisodeID AND ueh.UserID = ?
                         LEFT JOIN SavedEpisodes se ON e.EpisodeID = se.EpisodeID AND se.UserID = ?
                         LEFT JOIN EpisodeQueue eq ON e.EpisodeID = eq.EpisodeID AND eq.UserID = ? AND eq.is_youtube = 0
                         LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID AND de.UserID = ?
                         WHERE pc.PlaylistID = ?
                         ORDER BY pc.DateAdded DESC"
                    )
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(user_id)
                        .bind(playlist_id)
                        .fetch_all(pool)
                        .await?
                };

                let mut episodes = Vec::new();
                for row in episodes_rows {
                    let episodeid: i32 = row.try_get("episodeid")?;
                    let episodetitle: String = row.try_get("episodetitle")?;
                    let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                    let episodepubdate = naive.format("%Y-%m-%dT%H:%M:%S").to_string();
                    let episodedescription: String = row.try_get("episodedescription")?;
                    let episodeartwork: String = row.try_get("episodeartwork")?;
                    let episodeurl: String = row.try_get("episodeurl")?;
                    let episodeduration: i32 = row.try_get("episodeduration")?;
                    let completed: bool = row.try_get::<i8, _>("completed")? != 0;
                    let podcastname: String = row.try_get("podcastname")?;
                    let podcastid: i32 = row.try_get("podcastid")?;
                    let is_youtube: bool = row.try_get::<i8, _>("is_youtube")? != 0;
                    let listenduration: Option<i32> = row.try_get("listenduration")?;
                    let saved: bool = row.try_get::<i8, _>("saved")? != 0;
                    let queued: bool = row.try_get::<i8, _>("queued")? != 0;
                    let downloaded: bool = row.try_get::<i8, _>("downloaded")? != 0;
                    let addeddate_dt = row.try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>("addeddate")?;
                    let addeddate = addeddate_dt.format("%Y-%m-%dT%H:%M:%S").to_string();

                    episodes.push(serde_json::json!({
                        "episodeid": episodeid,
                        "episodetitle": episodetitle,
                        "episodepubdate": episodepubdate,
                        "episodedescription": episodedescription,
                        "episodeartwork": episodeartwork,
                        "episodeurl": episodeurl,
                        "episodeduration": episodeduration,
                        "completed": completed,
                        "podcastname": podcastname,
                        "podcastid": podcastid,
                        "is_youtube": is_youtube,
                        "listenduration": listenduration,
                        "saved": saved,
                        "queued": queued,
                        "downloaded": downloaded,
                        "dateadded": addeddate
                    }));
                }

                // Build playlist_info structure matching Python exactly
                let playlist_info = serde_json::json!({
                    "name": playlist_name,
                    "description": playlist_description,
                    "episode_count": episode_count,
                    "icon_name": icon_name
                });

                Ok(serde_json::json!({
                    "playlist_info": playlist_info,
                    "episodes": episodes
                }))
            },
        })
    }

    pub async fn delete_playlist(&self, user_id: i32, playlist_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                // Check if playlist exists and belongs to user
                let playlist = q::query(
                    r#"SELECT issystemplaylist, userid FROM "Playlists" WHERE playlistid = $1"#
                )
                .bind(playlist_id)
                .fetch_optional(pool)
                .await?;

                let playlist = playlist.ok_or_else(|| AppError::not_found("Playlist not found"))?;

                let is_system: bool = playlist.try_get("issystemplaylist")?;
                let owner_id: i32 = playlist.try_get("userid")?;

                if is_system {
                    return Err(AppError::bad_request("Cannot delete system playlists"));
                }

                if owner_id != user_id {
                    return Err(AppError::forbidden("Unauthorized to delete this playlist"));
                }

                // Delete the playlist
                q::query(r#"DELETE FROM "Playlists" WHERE playlistid = $1"#)
                    .bind(playlist_id)
                    .execute(pool)
                    .await?;

                Ok(())
            },
            mysql(pool) => {
                // Check if playlist exists and belongs to user
                let playlist = sqlx::query("SELECT IsSystemPlaylist, UserID FROM Playlists WHERE PlaylistID = ?")
                    .bind(playlist_id)
                    .fetch_optional(pool)
                    .await?;

                let playlist = playlist.ok_or_else(|| AppError::not_found("Playlist not found"))?;

                let is_system: i8 = playlist.try_get("IsSystemPlaylist")?;
                let owner_id: i32 = playlist.try_get("UserID")?;

                if is_system != 0 {
                    return Err(AppError::bad_request("Cannot delete system playlists"));
                }

                if owner_id != user_id {
                    return Err(AppError::forbidden("Unauthorized to delete this playlist"));
                }

                // Delete the playlist
                sqlx::query("DELETE FROM Playlists WHERE PlaylistID = ?")
                    .bind(playlist_id)
                    .execute(pool)
                    .await?;

                Ok(())
            },
        })
    }

    pub async fn update_playlist(&self, _config: &crate::config::Config, playlist_data: &crate::models::UpdatePlaylistRequest) -> AppResult<()> {
        let min_duration = playlist_data.min_duration.map(|d| d * 60);
        let max_duration = playlist_data.max_duration.map(|d| d * 60);

        dialect_match!(self, {
            pg(pool) => {
                // Verify ownership and not a system playlist
                let row = q::query(
                    r#"SELECT issystemplaylist, userid FROM "Playlists" WHERE playlistid = $1"#
                )
                .bind(playlist_data.playlist_id)
                .fetch_optional(pool)
                .await?;

                let row = row.ok_or_else(|| AppError::not_found("Playlist not found"))?;
                if row.try_get::<bool, _>("issystemplaylist")? {
                    return Err(AppError::bad_request("Cannot edit system playlists"));
                }
                if row.try_get::<i32, _>("userid")? != playlist_data.user_id {
                    return Err(AppError::forbidden("Unauthorized to edit this playlist"));
                }

                let podcast_ids_array = playlist_data.podcast_ids.clone().unwrap_or_default();

                q::query(r#"
                    UPDATE "Playlists" SET
                        name = $1,
                        description = $2,
                        podcastids = $3,
                        includeunplayed = $4,
                        includepartiallyplayed = $5,
                        includeplayed = $6,
                        minduration = $7,
                        maxduration = $8,
                        sortorder = $9,
                        groupbypodcast = $10,
                        maxepisodes = $11,
                        iconname = $12,
                        playprogressmin = $13,
                        playprogressmax = $14,
                        timefilterhours = $15
                    WHERE playlistid = $16
                "#)
                .bind(&playlist_data.name)
                .bind(&playlist_data.description)
                .bind(&podcast_ids_array)
                .bind(playlist_data.include_unplayed)
                .bind(playlist_data.include_partially_played)
                .bind(playlist_data.include_played)
                .bind(min_duration)
                .bind(max_duration)
                .bind(&playlist_data.sort_order)
                .bind(playlist_data.group_by_podcast)
                .bind(playlist_data.max_episodes)
                .bind(&playlist_data.icon_name)
                .bind(playlist_data.play_progress_min)
                .bind(playlist_data.play_progress_max)
                .bind(playlist_data.time_filter_hours)
                .bind(playlist_data.playlist_id)
                .execute(pool)
                .await?;

                self.update_playlist_contents(playlist_data.playlist_id).await?;
                Ok(())
            },
            mysql(pool) => {
                let row = sqlx::query(
                    "SELECT IsSystemPlaylist, UserID FROM Playlists WHERE PlaylistID = ?"
                )
                .bind(playlist_data.playlist_id)
                .fetch_optional(pool)
                .await?;

                let row = row.ok_or_else(|| AppError::not_found("Playlist not found"))?;
                let is_system: i8 = row.try_get("IsSystemPlaylist")?;
                if is_system != 0 {
                    return Err(AppError::bad_request("Cannot edit system playlists"));
                }
                if row.try_get::<i32, _>("UserID")? != playlist_data.user_id {
                    return Err(AppError::forbidden("Unauthorized to edit this playlist"));
                }

                let podcast_ids_json = serde_json::to_string(
                    &playlist_data.podcast_ids.clone().unwrap_or_default()
                )?;

                sqlx::query(r#"
                    UPDATE Playlists SET
                        Name = ?,
                        Description = ?,
                        PodcastIDs = ?,
                        IncludeUnplayed = ?,
                        IncludePartiallyPlayed = ?,
                        IncludePlayed = ?,
                        MinDuration = ?,
                        MaxDuration = ?,
                        SortOrder = ?,
                        GroupByPodcast = ?,
                        MaxEpisodes = ?,
                        IconName = ?,
                        PlayProgressMin = ?,
                        PlayProgressMax = ?,
                        TimeFilterHours = ?
                    WHERE PlaylistID = ?
                "#)
                .bind(&playlist_data.name)
                .bind(&playlist_data.description)
                .bind(&podcast_ids_json)
                .bind(playlist_data.include_unplayed)
                .bind(playlist_data.include_partially_played)
                .bind(playlist_data.include_played)
                .bind(min_duration)
                .bind(max_duration)
                .bind(&playlist_data.sort_order)
                .bind(playlist_data.group_by_podcast)
                .bind(playlist_data.max_episodes)
                .bind(&playlist_data.icon_name)
                .bind(playlist_data.play_progress_min)
                .bind(playlist_data.play_progress_max)
                .bind(playlist_data.time_filter_hours)
                .bind(playlist_data.playlist_id)
                .execute(pool)
                .await?;

                self.update_playlist_contents(playlist_data.playlist_id).await?;
                Ok(())
            },
        })
    }

    // Create missing default playlists for existing users
    pub async fn create_missing_default_playlists(&self) -> AppResult<()> {
        
        info!("🎵 Checking for missing default playlists for existing users...");
        
        // Define default playlists (same as migration 032)
        let default_playlists = vec![
            ("Quick Listens", "Short episodes under 15 minutes, perfect for quick breaks", Some(1), Some(900), "duration_asc", false, true, true, None, false, Some(1000), "ph-fast-forward"),
            ("Longform", "Extended episodes over 1 hour, ideal for long drives or deep dives", Some(3600), None, "duration_desc", true, true, true, None, false, Some(1000), "ph-car"),
            ("Currently Listening", "Episodes you've started but haven't finished", None, None, "date_desc", false, true, false, None, false, None, "ph-play"),
            ("Fresh Releases", "Latest episodes from the last 24 hours", None, None, "date_desc", true, false, false, Some(24), false, None, "ph-sparkle"),
            ("Weekend Marathon", "Longer episodes (30+ minutes) perfect for weekend listening", Some(1800), None, "duration_desc", true, true, true, None, true, Some(1000), "ph-couch"),
            ("Commuter Mix", "Perfect-length episodes (15-45 minutes) for your daily commute", Some(900), Some(2700), "date_desc", true, true, true, None, false, Some(1000), "ph-car-simple"),
        ];
        
        dialect_match!(self, {
            pg(pool) => {
                // Get all existing users (excluding background user if present)
                let user_rows = q::query(r#"SELECT userid FROM "Users" WHERE userid > 1"#)
                    .fetch_all(pool)
                    .await?;
                
                info!("Found {} users to check for missing default playlists", user_rows.len());
                
                for user_row in user_rows {
                    let user_id: i32 = user_row.try_get("userid")?;
                    
                    for (name, description, min_duration, max_duration, sort_order, include_unplayed, include_partially_played, include_played, time_filter_hours, group_by_podcast, max_episodes, icon_name) in &default_playlists {
                        // Check if this playlist already exists for this user
                        let exists: bool = q::query_scalar(
                            r#"SELECT EXISTS(SELECT 1 FROM "Playlists" WHERE userid = $1 AND name = $2)"#
                        ).bind(user_id).bind(name).fetch_one(pool).await?;
                        
                        if !exists {
                            // Create the playlist for this user
                            match q::query(r#"
                                INSERT INTO "Playlists" (
                                    userid, name, description, issystemplaylist, minduration, maxduration, sortorder,
                                    includeunplayed, includepartiallyplayed, includeplayed, timefilterhours,
                                    groupbypodcast, maxepisodes, playprogressmin, playprogressmax, podcastids,
                                    iconname, episodecount
                                ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                                "#)
                                .bind(user_id).bind(name).bind(description).bind(false)
                                .bind(min_duration).bind(max_duration).bind(sort_order)
                                .bind(include_unplayed).bind(include_partially_played).bind(include_played)
                                .bind(time_filter_hours).bind(group_by_podcast).bind(max_episodes)
                                .bind(0.0).bind(100.0).bind(&[] as &[i32]).bind(icon_name).bind(0)
                                .execute(pool).await {
                                Ok(_) => info!("Created playlist '{}' for user {}", name, user_id),
                                Err(e) => warn!("Failed to create playlist '{}' for user {}: {}", name, user_id, e),
                            }
                        }
                    }
                }
            },
            mysql(pool) => {
                // Get all existing users (excluding background user if present)
                let user_rows = sqlx::query("SELECT UserID FROM Users WHERE UserID > 1")
                    .fetch_all(pool)
                    .await?;
                
                info!("Found {} users to check for missing default playlists", user_rows.len());
                
                for user_row in user_rows {
                    let user_id: i32 = user_row.try_get("UserID")?;
                    
                    for (name, description, min_duration, max_duration, sort_order, include_unplayed, include_partially_played, include_played, time_filter_hours, group_by_podcast, max_episodes, icon_name) in &default_playlists {
                        // Check if this playlist already exists for this user
                        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Playlists WHERE UserID = ? AND Name = ?")
                            .bind(user_id)
                            .bind(name)
                            .fetch_one(pool)
                            .await?;
                        
                        if count == 0 {
                            // Create the playlist for this user
                            match sqlx::query("
                                INSERT INTO Playlists (
                                    UserID, Name, Description, IsSystemPlaylist, MinDuration, MaxDuration, SortOrder,
                                    IncludeUnplayed, IncludePartiallyPlayed, IncludePlayed, TimeFilterHours,
                                    GroupByPodcast, MaxEpisodes, PlayProgressMin, PlayProgressMax, PodcastIDs,
                                    IconName, EpisodeCount
                                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                                ")
                                .bind(user_id).bind(name).bind(description).bind(false)
                                .bind(min_duration).bind(max_duration).bind(sort_order)
                                .bind(include_unplayed).bind(include_partially_played).bind(include_played)
                                .bind(time_filter_hours).bind(group_by_podcast).bind(max_episodes)
                                .bind(0.0).bind(100.0).bind("[]").bind(icon_name).bind(0)
                                .execute(pool).await {
                                Ok(_) => info!("Created playlist '{}' for user {}", name, user_id),
                                Err(e) => warn!("Failed to create playlist '{}' for user {}: {}", name, user_id, e),
                            }
                        }
                    }
                }
            },
        });
        
        info!("✅ Finished checking for missing default playlists");
        Ok(())
    }

    // Create default playlists for a single user - shared by user creation and startup check
    pub(super) async fn create_default_playlists_for_user(&self, user_id: i32) -> AppResult<()> {
        
        // Define default playlists (same as migration 032)
        let default_playlists = vec![
            ("Quick Listens", "Short episodes under 15 minutes, perfect for quick breaks", Some(1), Some(900), "duration_asc", true, true, true, None, false, Some(1000), "ph-fast-forward"),
            ("Longform", "Extended episodes over 1 hour, ideal for long drives or deep dives", Some(3600), None, "duration_desc", true, true, true, None, false, Some(1000), "ph-car"),
            ("Currently Listening", "Episodes you've started but haven't finished", None, None, "date_desc", false, true, false, None, false, None, "ph-play"),
            ("Fresh Releases", "Latest episodes from the last 24 hours", None, None, "date_desc", true, false, false, Some(24), false, None, "ph-sparkle"),
            ("Weekend Marathon", "Longer episodes (30+ minutes) perfect for weekend listening", Some(1800), None, "duration_desc", true, true, true, None, true, Some(1000), "ph-couch"),
            ("Commuter Mix", "Perfect-length episodes (15-45 minutes) for your daily commute", Some(900), Some(2700), "date_desc", true, true, true, None, false, Some(1000), "ph-car-simple"),
        ];
        
        dialect_match!(self, {
            pg(pool) => {
                for (name, description, min_duration, max_duration, sort_order, include_unplayed, include_partially_played, include_played, time_filter_hours, group_by_podcast, max_episodes, icon_name) in &default_playlists {
                    // Check if this playlist already exists for this user
                    let exists: bool = q::query_scalar(
                        r#"SELECT EXISTS(SELECT 1 FROM "Playlists" WHERE userid = $1 AND name = $2)"#
                    ).bind(user_id).bind(name).fetch_one(pool).await?;
                    
                    if !exists {
                        // Create the playlist for this user
                        match q::query(r#"
                            INSERT INTO "Playlists" (
                                userid, name, description, issystemplaylist, minduration, maxduration, sortorder,
                                includeunplayed, includepartiallyplayed, includeplayed, timefilterhours,
                                groupbypodcast, maxepisodes, playprogressmin, playprogressmax, podcastids,
                                iconname, episodecount
                            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
                            "#)
                            .bind(user_id).bind(name).bind(description).bind(false)
                            .bind(min_duration).bind(max_duration).bind(sort_order)
                            .bind(include_unplayed).bind(include_partially_played).bind(include_played)
                            .bind(time_filter_hours).bind(group_by_podcast).bind(max_episodes)
                            .bind(0.0).bind(100.0).bind(&[] as &[i32]).bind(icon_name).bind(0)
                            .execute(pool).await {
                            Ok(_) => info!("Created playlist '{}' for user {}", name, user_id),
                            Err(e) => warn!("Failed to create playlist '{}' for user {}: {}", name, user_id, e),
                        }
                    }
                }
            },
            mysql(pool) => {
                for (name, description, min_duration, max_duration, sort_order, include_unplayed, include_partially_played, include_played, time_filter_hours, group_by_podcast, max_episodes, icon_name) in &default_playlists {
                    // Check if this playlist already exists for this user
                    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Playlists WHERE UserID = ? AND Name = ?")
                        .bind(user_id)
                        .bind(name)
                        .fetch_one(pool)
                        .await?;
                    
                    if count == 0 {
                        // Create the playlist for this user
                        match sqlx::query("
                            INSERT INTO Playlists (
                                UserID, Name, Description, IsSystemPlaylist, MinDuration, MaxDuration, SortOrder,
                                IncludeUnplayed, IncludePartiallyPlayed, IncludePlayed, TimeFilterHours,
                                GroupByPodcast, MaxEpisodes, PlayProgressMin, PlayProgressMax, PodcastIDs,
                                IconName, EpisodeCount
                            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                            ")
                            .bind(user_id).bind(name).bind(description).bind(false)
                            .bind(min_duration).bind(max_duration).bind(sort_order)
                            .bind(include_unplayed).bind(include_partially_played).bind(include_played)
                            .bind(time_filter_hours).bind(group_by_podcast).bind(max_episodes)
                            .bind(0.0).bind(100.0).bind("[]").bind(icon_name).bind(0)
                            .execute(pool).await {
                            Ok(_) => info!("Created playlist '{}' for user {}", name, user_id),
                            Err(e) => warn!("Failed to create playlist '{}' for user {}: {}", name, user_id, e),
                        }
                    }
                }
            },
        });
        
        Ok(())
    }

    /// Recompute the stored episode counts of the user's playlists. The refresh calls it for users
    /// whose podcasts got new episodes; `get_playlists` when the counts are older than
    /// `PLAYLIST_COUNTS_MAX_AGE` (listening and time filters change them too).
    pub async fn update_user_playlist_counts(&self, user_id: i32) -> AppResult<()> {
        PLAYLIST_COUNTS_AT.lock().unwrap().insert((self.instance_key(), user_id), std::time::Instant::now());
        let playlists: Vec<i32> = dialect_match!(self, {
            pg(pool) => q::query_scalar(r#"SELECT playlistid FROM "Playlists" WHERE userid = $1"#)
                .bind(user_id)
                .fetch_all(pool)
                .await?,
            mysql(pool) => sqlx::query_scalar("SELECT PlaylistID FROM Playlists WHERE UserID = ?")
                .bind(user_id)
                .fetch_all(pool)
                .await?,
        });
        for playlist_id in playlists {
            let count = match self.count_playlist_episodes_dynamic(playlist_id, user_id).await {
                Ok(count) => count,
                Err(e) => {
                    warn!("Failed to count episodes for playlist {}: {}", playlist_id, e);
                    continue;
                }
            };
            let updated = dialect_match!(self, {
                pg(pool) => q::query(r#"UPDATE "Playlists" SET episodecount = $1 WHERE playlistid = $2"#)
                    .bind(count)
                    .bind(playlist_id)
                    .execute(pool)
                    .await
                    .map(|_| ()),
                mysql(pool) => sqlx::query("UPDATE Playlists SET EpisodeCount = ? WHERE PlaylistID = ?")
                    .bind(count)
                    .bind(playlist_id)
                    .execute(pool)
                    .await
                    .map(|_| ()),
            });
            if let Err(e) = updated {
                warn!("Failed to update the count of playlist {}: {}", playlist_id, e);
            }
        }
        Ok(())
    }

    pub(super) fn playlist_counts_stale(&self, user_id: i32) -> bool {
        PLAYLIST_COUNTS_AT
            .lock()
            .unwrap()
            .get(&(self.instance_key(), user_id))
            .is_none_or(|at| at.elapsed() > PLAYLIST_COUNTS_MAX_AGE)
    }

    // Count episodes for a playlist using the same dynamic logic (without pagination)
    pub(super) async fn count_playlist_episodes_dynamic(&self, playlist_id: i32, user_id: i32) -> AppResult<i32> {
        
        dialect_match!(self, {
            pg(pool) => {
                // Get user timezone for proper date calculations
                let raw_user_timezone: String = q::query_scalar(
                    r#"SELECT timezone FROM "Users" WHERE userid = $1"#
                ).bind(user_id).fetch_optional(pool).await?.unwrap_or_else(|| "UTC".to_string());
                let raw_timezone = if raw_user_timezone.is_empty() { "UTC".to_string() } else { raw_user_timezone };
                let user_timezone = Self::map_timezone_for_postgres(&raw_timezone);
                
                // Get playlist configuration
                let playlist_row = q::query(
                    r#"SELECT userid, name, minduration, maxduration, sortorder, 
                       includeunplayed, includepartiallyplayed, includeplayed, timefilterhours,
                       groupbypodcast, maxepisodes, playprogressmin, playprogressmax, podcastids
                       FROM "Playlists" WHERE playlistid = $1"#
                ).bind(playlist_id).fetch_optional(pool).await?;
                
                let playlist = playlist_row.ok_or_else(|| crate::error::AppError::not_found("Playlist not found"))?;
                
                // Build count query using same logic as dynamic function
                let mut query_parts = Vec::new();
                let mut where_conditions = Vec::new();
                
                query_parts.push(format!(r#"
                    SELECT COUNT(DISTINCT e.episodeid)
                    FROM "Episodes" e
                    JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = {}
                    LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = {}"#, 
                    user_id, user_id
                ));
                
                where_conditions.push("p.userid = $1".to_string());
                
                // Apply all the same filters as dynamic function
                if let Some(min_dur) = playlist.try_get::<Option<i32>, _>("minduration")? {
                    where_conditions.push(format!("e.episodeduration >= {}", min_dur));
                }
                if let Some(max_dur) = playlist.try_get::<Option<i32>, _>("maxduration")? {
                    where_conditions.push(format!("e.episodeduration <= {}", max_dur));
                }
                
                if let Some(hours) = playlist.try_get::<Option<i32>, _>("timefilterhours")? {
                    where_conditions.push(format!(
                        "e.episodepubdate >= (NOW() AT TIME ZONE '{}' - INTERVAL '{} hours') AT TIME ZONE '{}' AT TIME ZONE 'UTC'", 
                        user_timezone, hours, user_timezone
                    ));
                }
                
                // Handle PostgreSQL array type for podcast IDs
                if let Some(podcast_ids) = q::int_array(&playlist, "podcastids")? {
                    if !podcast_ids.is_empty() && !podcast_ids.contains(&-1) {
                        let podcast_ids_str = podcast_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                        where_conditions.push(format!("p.podcastid IN ({})", podcast_ids_str));
                    }
                }
                
                // Play state filters
                let mut play_state_conditions = Vec::new();
                if playlist.try_get::<bool, _>("includeunplayed")? {
                    play_state_conditions.push("(COALESCE((SELECT ues.completed FROM \"UserEpisodeState\" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) IS NOT TRUE AND (h.listenduration IS NULL OR h.listenduration = 0))".to_string());
                }
                if playlist.try_get::<bool, _>("includepartiallyplayed")? {
                    play_state_conditions.push(
                        "(h.listenduration > 0 AND h.listenduration < e.episodeduration * 0.9 AND (e.episodeduration - h.listenduration) > 30)".to_string()
                    );
                }
                if playlist.try_get::<bool, _>("includeplayed")? {
                    play_state_conditions.push(
                        "(COALESCE((SELECT ues.completed FROM \"UserEpisodeState\" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) IS TRUE OR (h.listenduration IS NOT NULL AND (h.listenduration >= e.episodeduration * 0.9 OR (e.episodeduration - h.listenduration) <= 30)))".to_string()
                    );
                }
                
                if !play_state_conditions.is_empty() {
                    where_conditions.push(format!("({})", play_state_conditions.join(" OR ")));
                } else {
                    where_conditions.push("FALSE".to_string());
                }
                
                // Progress filters
                if let Some(min_progress) = playlist.try_get::<Option<f64>, _>("playprogressmin")? {
                    where_conditions.push(format!(
                        "(COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) >= {}", 
                        min_progress / 100.0
                    ));
                }
                if let Some(max_progress) = playlist.try_get::<Option<f64>, _>("playprogressmax")? {
                    where_conditions.push(format!(
                        "(COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) <= {}", 
                        max_progress / 100.0
                    ));
                }
                
                let where_clause = if where_conditions.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", where_conditions.join(" AND "))
                };
                
                let final_query = format!("{}{}", query_parts.join(" "), where_clause);
                
                let count: i64 = q::query_scalar(sqlx::AssertSqlSafe(final_query.as_str()))
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                
                // Apply MaxEpisodes limit if specified
                let final_count = if let Some(max_eps) = playlist.try_get::<Option<i32>, _>("maxepisodes")? {
                    if max_eps > 0 {
                        std::cmp::min(count as i32, max_eps)
                    } else {
                        count as i32
                    }
                } else {
                    count as i32
                };
                
                Ok(final_count)
            },
            mysql(pool) => {
                // Similar implementation for MySQL with adjusted syntax
                let raw_user_timezone: String = sqlx::query_scalar("SELECT TimeZone FROM Users WHERE UserID = ?")
                    .bind(user_id).fetch_optional(pool).await?.unwrap_or_else(|| "UTC".to_string());
                let raw_timezone = if raw_user_timezone.is_empty() { "UTC".to_string() } else { raw_user_timezone };
                let user_timezone = Self::map_timezone_for_postgres(&raw_timezone);
                
                let playlist_row = sqlx::query(
                    r#"SELECT UserID, Name, MinDuration, MaxDuration, SortOrder, 
                       IncludeUnplayed, IncludePartiallyPlayed, IncludePlayed, TimeFilterHours,
                       GroupByPodcast, MaxEpisodes, PlayProgressMin, PlayProgressMax, PodcastIDs
                       FROM Playlists WHERE PlaylistID = ?"#
                ).bind(playlist_id).fetch_optional(pool).await?;
                
                let playlist = playlist_row.ok_or_else(|| crate::error::AppError::not_found("Playlist not found"))?;
                
                // MySQL count query with similar logic
                let mut query_parts = Vec::new();
                let mut where_conditions = Vec::new();
                
                query_parts.push(format!(r#"
                    SELECT COUNT(DISTINCT e.EpisodeID)
                    FROM Episodes e
                    JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = {}
                    LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = {}"#, 
                    user_id, user_id
                ));
                
                where_conditions.push("p.UserID = ?".to_string());
                
                // Apply all MySQL filters
                if let Some(min_dur) = playlist.try_get::<Option<i32>, _>("MinDuration")? {
                    where_conditions.push(format!("e.EpisodeDuration >= {}", min_dur));
                }
                if let Some(max_dur) = playlist.try_get::<Option<i32>, _>("MaxDuration")? {
                    where_conditions.push(format!("e.EpisodeDuration <= {}", max_dur));
                }
                
                if let Some(hours) = playlist.try_get::<Option<i32>, _>("TimeFilterHours")? {
                    where_conditions.push(format!(
                        "e.EpisodePubDate >= DATE_SUB(CONVERT_TZ(NOW(), 'UTC', '{}'), INTERVAL {} HOUR)", 
                        user_timezone, hours
                    ));
                }
                
                // Podcast filter - handle JSON array of podcast IDs (MySQL/MariaDB).
                // MariaDB reports the JSON column as a binary BLOB charset, so decode as
                // raw bytes first and only fall back to a direct String decode (see #773).
                let podcast_ids_json_owned: Option<String> = match playlist.try_get::<Option<Vec<u8>>, _>("PodcastIDs") {
                    Ok(bytes_opt) => bytes_opt.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
                    Err(_) => playlist.try_get::<Option<String>, _>("PodcastIDs")?,
                };
                if let Some(podcast_ids_json) = podcast_ids_json_owned.as_ref() {
                    if !podcast_ids_json.is_empty() && podcast_ids_json != "[]" && podcast_ids_json != "null" {
                        match serde_json::from_str::<Vec<i32>>(podcast_ids_json) {
                            Ok(podcast_ids) if !podcast_ids.is_empty() && !podcast_ids.contains(&-1) => {
                                let podcast_ids_str = podcast_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                                where_conditions.push(format!("p.PodcastID IN ({})", podcast_ids_str));
                            }
                            Ok(_) => {}
                            Err(e) => warn!("⚠️ Failed to parse MySQL podcast IDs JSON '{}': {}", podcast_ids_json, e),
                        }
                    }
                }

                // Play state filters - must mirror get_playlist_episodes_dynamic exactly
                // so the cached count matches what the detail view returns
                let mut play_state_conditions = Vec::new();
                if playlist.try_get::<bool, _>("IncludeUnplayed")? {
                    play_state_conditions.push("(COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) IS NOT TRUE AND (h.ListenDuration IS NULL OR h.ListenDuration = 0))".to_string());
                }
                if playlist.try_get::<bool, _>("IncludePartiallyPlayed")? {
                    play_state_conditions.push(
                        "(h.ListenDuration > 0 AND h.ListenDuration < e.EpisodeDuration * 0.9 AND (e.EpisodeDuration - h.ListenDuration) > 30)".to_string()
                    );
                }
                if playlist.try_get::<bool, _>("IncludePlayed")? {
                    play_state_conditions.push(
                        "(COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) IS TRUE OR (h.ListenDuration IS NOT NULL AND (h.ListenDuration >= e.EpisodeDuration * 0.9 OR (e.EpisodeDuration - h.ListenDuration) <= 30)))".to_string()
                    );
                }

                if !play_state_conditions.is_empty() {
                    where_conditions.push(format!("({})", play_state_conditions.join(" OR ")));
                } else {
                    where_conditions.push("FALSE".to_string());
                }

                // Progress percentage filters
                if let Some(min_progress) = playlist.try_get::<Option<f64>, _>("PlayProgressMin")? {
                    where_conditions.push(format!(
                        "(COALESCE(h.ListenDuration, 0) / NULLIF(e.EpisodeDuration, 0)) >= {}",
                        min_progress / 100.0
                    ));
                }
                if let Some(max_progress) = playlist.try_get::<Option<f64>, _>("PlayProgressMax")? {
                    where_conditions.push(format!(
                        "(COALESCE(h.ListenDuration, 0) / NULLIF(e.EpisodeDuration, 0)) <= {}",
                        max_progress / 100.0
                    ));
                }

                let where_clause = if where_conditions.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", where_conditions.join(" AND "))
                };

                let final_query = format!("{}{}", query_parts.join(" "), where_clause);

                let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(final_query.as_str()))
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;

                let final_count = if let Some(max_eps) = playlist.try_get::<Option<i32>, _>("MaxEpisodes")? {
                    if max_eps > 0 {
                        std::cmp::min(count as i32, max_eps)
                    } else {
                        count as i32
                    }
                } else {
                    count as i32
                };
                
                Ok(final_count)
            },
        })
    }

    /// Convert chrono-tz timezone names to PostgreSQL-compatible timezone names
    /// This handles the mapping between frontend timezone selections and database queries
    pub(super) fn map_timezone_for_postgres(user_timezone: &str) -> String {
        match user_timezone {
            // US timezone mappings
            "US/Alaska" => "America/Anchorage".to_string(),
            "US/Aleutian" => "America/Adak".to_string(),
            "US/Arizona" => "America/Phoenix".to_string(),
            "US/Central" => "America/Chicago".to_string(),
            "US/East-Indiana" => "America/Indiana/Indianapolis".to_string(),
            "US/Eastern" => "America/New_York".to_string(),
            "US/Hawaii" => "Pacific/Honolulu".to_string(),
            "US/Indiana-Starke" => "America/Indiana/Knox".to_string(),
            "US/Michigan" => "America/Detroit".to_string(),
            "US/Mountain" => "America/Denver".to_string(),
            "US/Pacific" => "America/Los_Angeles".to_string(),
            "US/Samoa" => "Pacific/Pago_Pago".to_string(),

            // Canada timezone mappings
            "Canada/Atlantic" => "America/Halifax".to_string(),
            "Canada/Central" => "America/Winnipeg".to_string(),
            "Canada/Eastern" => "America/Toronto".to_string(),
            "Canada/Mountain" => "America/Edmonton".to_string(),
            "Canada/Newfoundland" => "America/St_Johns".to_string(),
            "Canada/Pacific" => "America/Vancouver".to_string(),
            "Canada/Saskatchewan" => "America/Regina".to_string(),
            "Canada/Yukon" => "America/Whitehorse".to_string(),

            // Brazil timezone mappings
            "Brazil/Acre" => "America/Rio_Branco".to_string(),
            "Brazil/DeNoronha" => "America/Noronha".to_string(),
            "Brazil/East" => "America/Sao_Paulo".to_string(),
            "Brazil/West" => "America/Manaus".to_string(),

            // Chile timezone mappings
            "Chile/Continental" => "America/Santiago".to_string(),
            "Chile/EasterIsland" => "Pacific/Easter".to_string(),

            // Mexico timezone mappings
            "Mexico/BajaNorte" => "America/Tijuana".to_string(),
            "Mexico/BajaSur" => "America/Mazatlan".to_string(),
            "Mexico/General" => "America/Mexico_City".to_string(),

            // Common US legacy timezone abbreviations
            "EST" => "America/New_York".to_string(),
            "CST" => "America/Chicago".to_string(),
            "MST" => "America/Denver".to_string(),
            "PST" => "America/Los_Angeles".to_string(),
            "HST" => "Pacific/Honolulu".to_string(),
            "EST5EDT" => "America/New_York".to_string(),
            "CST6CDT" => "America/Chicago".to_string(),
            "MST7MDT" => "America/Denver".to_string(),
            "PST8PDT" => "America/Los_Angeles".to_string(),

            // European legacy mappings
            "CET" => "Europe/Paris".to_string(),
            "EET" => "Europe/Helsinki".to_string(),
            "WET" => "Europe/Lisbon".to_string(),
            "MET" => "Europe/Paris".to_string(),

            // Common international legacy mappings
            "GMT" => "UTC".to_string(),
            "GMT+0" => "UTC".to_string(),
            "GMT-0" => "UTC".to_string(),
            "GMT0" => "UTC".to_string(),
            "Greenwich" => "UTC".to_string(),
            "UCT" => "UTC".to_string(),
            "Universal" => "UTC".to_string(),
            "Zulu" => "UTC".to_string(),

            // Country/region legacy mappings
            "Cuba" => "America/Havana".to_string(),
            "Egypt" => "Africa/Cairo".to_string(),
            "Eire" => "Europe/Dublin".to_string(),
            "GB" => "Europe/London".to_string(),
            "GB-Eire" => "Europe/London".to_string(),
            "Hongkong" => "Asia/Hong_Kong".to_string(),
            "Iceland" => "Atlantic/Reykjavik".to_string(),
            "Iran" => "Asia/Tehran".to_string(),
            "Israel" => "Asia/Jerusalem".to_string(),
            "Jamaica" => "America/Jamaica".to_string(),
            "Japan" => "Asia/Tokyo".to_string(),
            "Kwajalein" => "Pacific/Kwajalein".to_string(),
            "Libya" => "Africa/Tripoli".to_string(),
            "NZ" => "Pacific/Auckland".to_string(),
            "NZ-CHAT" => "Pacific/Chatham".to_string(),
            "Navajo" => "America/Denver".to_string(),
            "PRC" => "Asia/Shanghai".to_string(),
            "Poland" => "Europe/Warsaw".to_string(),
            "Portugal" => "Europe/Lisbon".to_string(),
            "ROC" => "Asia/Taipei".to_string(),
            "ROK" => "Asia/Seoul".to_string(),
            "Singapore" => "Asia/Singapore".to_string(),
            "Turkey" => "Europe/Istanbul".to_string(),
            "W-SU" => "Europe/Moscow".to_string(),

            // The result is formatted into SQL, so only known tz names pass through
            _ => match user_timezone.parse::<Tz>() {
                Ok(tz) => tz.name().to_string(),
                Err(_) => "UTC".to_string(),
            },
        }
    }

    // Get playlist episodes dynamically without using PlaylistContents table
    // ULTRA-PRECISE implementation covering ALL playlist options with timezone awareness
    pub async fn get_playlist_episodes_dynamic(
        &self,
        playlist_id: i32,
        user_id: i32,
        limit: i64,
        offset: i64,
    ) -> AppResult<crate::models::PlaylistEpisodesResponse> {
        
        debug!("🎵 Getting dynamic playlist episodes for playlist {} user {}", playlist_id, user_id);
        
        dialect_match!(self, {
            pg(pool) => {
                // Get user timezone for proper date calculations
                let raw_user_timezone: String = q::query_scalar(
                    r#"SELECT timezone FROM "Users" WHERE userid = $1"#
                ).bind(user_id).fetch_optional(pool).await?.unwrap_or_else(|| "UTC".to_string());
                let raw_timezone = if raw_user_timezone.is_empty() { "UTC".to_string() } else { raw_user_timezone };
                let user_timezone = Self::map_timezone_for_postgres(&raw_timezone);
                
                debug!("User {} timezone: {} -> {}", user_id, raw_timezone, user_timezone);
                
                // Get playlist configuration with ALL fields
                let playlist_row = q::query(
                    r#"SELECT userid, name, description, minduration, maxduration, sortorder, 
                       includeunplayed, includepartiallyplayed, includeplayed, timefilterhours,
                       groupbypodcast, maxepisodes, playprogressmin, playprogressmax, podcastids,
                       issystemplaylist, created, iconname, episodecount
                       FROM "Playlists" WHERE playlistid = $1"#
                ).bind(playlist_id).fetch_optional(pool).await?;
                
                let playlist = playlist_row.ok_or_else(|| crate::error::AppError::not_found("Playlist not found"))?;
                
                // Check user permissions - users can only access their own playlists
                if playlist.try_get::<i32, _>("userid")? != user_id {
                    return Err(crate::error::AppError::forbidden("You can only access your own playlists"));
                }
                
                debug!("📋 Playlist '{}' config: min_dur={:?}, max_dur={:?}, sort={}, time_filter={:?}, progress_min={:?}, progress_max={:?}", 
                    playlist.try_get::<String, _>("name")?, playlist.try_get::<Option<i32>, _>("minduration")?, playlist.try_get::<Option<i32>, _>("maxduration")?, playlist.try_get::<String, _>("sortorder")?, 
                    playlist.try_get::<Option<i32>, _>("timefilterhours")?, playlist.try_get::<Option<f64>, _>("playprogressmin")?, playlist.try_get::<Option<f64>, _>("playprogressmax")?);
                
                // Build the comprehensive dynamic query
                let mut query_parts = Vec::new();
                let mut where_conditions = Vec::new();
                let _bind_values: Vec<Box<dyn std::fmt::Debug + Send + Sync>> = Vec::new();
                
                // Base SELECT with all episode data needed for SavedEpisode model including podcastid
                query_parts.push(format!(r#"
                    SELECT DISTINCT
                        e.episodetitle,
                        p.podcastname,
                        TO_CHAR(e.episodepubdate AT TIME ZONE '{}', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') as episodepubdate,
                        e.episodedescription,
                        CASE
                            WHEN p.usepodcastcoverscustomized = TRUE AND p.usepodcastcovers = TRUE THEN p.artworkurl
                            WHEN u.usepodcastcovers = TRUE THEN p.artworkurl
                            ELSE COALESCE(e.episodeartwork, p.artworkurl)
                        END as episodeartwork,
                        e.episodeurl,
                        e.episodeduration,
                        COALESCE(h.listenduration, 0) as listenduration,
                        e.episodeid,
                        COALESCE(p.websiteurl, '') as websiteurl,
                        -- ULTRA-PRECISE completion logic: 90% threshold OR within 30 seconds of end
                        CASE WHEN 
                            h.listenduration IS NOT NULL AND (
                                h.listenduration >= e.episodeduration * 0.9 OR 
                                (e.episodeduration - h.listenduration) <= 30
                            ) 
                        THEN true ELSE false END as completed,
                        EXISTS(SELECT 1 FROM "SavedEpisodes" se WHERE se.episodeid = e.episodeid AND se.userid = {}) as saved,
                        EXISTS(SELECT 1 FROM "EpisodeQueue" eq WHERE eq.episodeid = e.episodeid AND eq.userid = {}) as queued,
                        EXISTS(SELECT 1 FROM "DownloadedEpisodes" de WHERE de.episodeid = e.episodeid AND de.userid = {}) as downloaded,
                        false as is_youtube,
                        p.podcastid,
                        -- Progress percentage for debugging
                        ROUND(((COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) * 100)::numeric, 2) as progress_percent
                    FROM "Episodes" e
                    JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = {}
                    LEFT JOIN "Users" u ON p.userid = u.userid
                    LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = {}"#,
                    user_timezone, user_id, user_id, user_id, user_id, user_id
                ));
                
                // Base condition - always filter by user's podcasts
                where_conditions.push("p.userid = $1".to_string());
                
                // 1. DURATION FILTERS - exact duration matching
                if let Some(min_dur) = playlist.try_get::<Option<i32>, _>("minduration")? {
                    where_conditions.push(format!("e.episodeduration >= {}", min_dur));
                    debug!("🕒 Added min duration filter: {} seconds", min_dur);
                }
                if let Some(max_dur) = playlist.try_get::<Option<i32>, _>("maxduration")? {
                    where_conditions.push(format!("e.episodeduration <= {}", max_dur));
                    debug!("🕒 Added max duration filter: {} seconds", max_dur);
                }
                
                // 2. TIMEZONE-AWARE TIME FILTER - ULTRA-PRECISE datetime handling
                if let Some(hours) = playlist.try_get::<Option<i32>, _>("timefilterhours")? {
                    // Convert hours to user's timezone for precise "last X hours" calculation
                    where_conditions.push(format!(
                        "e.episodepubdate >= (NOW() AT TIME ZONE '{}' - INTERVAL '{} hours') AT TIME ZONE '{}' AT TIME ZONE 'UTC'", 
                        user_timezone, hours, user_timezone
                    ));
                    debug!("📅 Added timezone-aware time filter: last {} hours in timezone {}", hours, user_timezone);
                }
                
                // 3. PODCAST FILTER - handle PostgreSQL array of podcast IDs
                if let Some(podcast_ids) = q::int_array(&playlist, "podcastids")? {
                    if !podcast_ids.is_empty() && !podcast_ids.contains(&-1) {
                        let podcast_ids_str = podcast_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                        where_conditions.push(format!("p.podcastid IN ({})", podcast_ids_str));
                        debug!("🎙️ Added PostgreSQL podcast filter: {:?}", podcast_ids);
                    } else {
                        debug!("🎙️ PostgreSQL podcast filter contains -1 or is empty, including all podcasts");
                    }
                }
                
                // 4. ULTRA-PRECISE PLAY STATE FILTERS
                let mut play_state_conditions = Vec::new();
                
                if playlist.try_get::<bool, _>("includeunplayed")? {
                    // UNPLAYED: No history record OR listen duration is 0 or NULL
                    play_state_conditions.push("(COALESCE((SELECT ues.completed FROM \"UserEpisodeState\" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) IS NOT TRUE AND (h.listenduration IS NULL OR h.listenduration = 0))".to_string());
                    debug!("▶️ Including UNPLAYED episodes");
                }
                
                if playlist.try_get::<bool, _>("includepartiallyplayed")? {
                    // PARTIALLY PLAYED: Has listen time > 0 but < 90% AND not within 30 seconds of end
                    play_state_conditions.push(
                        "(h.listenduration > 0 AND h.listenduration < e.episodeduration * 0.9 AND (e.episodeduration - h.listenduration) > 30)".to_string()
                    );
                    debug!("⏸️ Including PARTIALLY PLAYED episodes (>0% and <90%, not within 30s of end)");
                }
                
                if playlist.try_get::<bool, _>("includeplayed")? {
                    // PLAYED: Listen duration >= 90% OR within 30 seconds of end
                    play_state_conditions.push(
                        "(COALESCE((SELECT ues.completed FROM \"UserEpisodeState\" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) IS TRUE OR (h.listenduration IS NOT NULL AND (h.listenduration >= e.episodeduration * 0.9 OR (e.episodeduration - h.listenduration) <= 30)))".to_string()
                    );
                    debug!("✅ Including PLAYED episodes (>=90% or within 30s of end)");
                }
                
                if !play_state_conditions.is_empty() {
                    where_conditions.push(format!("({})", play_state_conditions.join(" OR ")));
                } else {
                    // No play states selected - return no results
                    where_conditions.push("FALSE".to_string());
                    warn!("⚠️ No play states selected for playlist '{}' - will return empty results", playlist.try_get::<String, _>("name")?);
                }
                
                // 5. ULTRA-PRECISE PROGRESS PERCENTAGE FILTERS
                if let Some(min_progress) = playlist.try_get::<Option<f64>, _>("playprogressmin")? {
                    let min_decimal = min_progress / 100.0;
                    where_conditions.push(format!(
                        "(COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) >= {}", 
                        min_decimal
                    ));
                    debug!("📊 Added min progress filter: {}% ({})", min_progress, min_decimal);
                }
                if let Some(max_progress) = playlist.try_get::<Option<f64>, _>("playprogressmax")? {
                    let max_decimal = max_progress / 100.0;
                    where_conditions.push(format!(
                        "(COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) <= {}", 
                        max_decimal
                    ));
                    debug!("📊 Added max progress filter: {}% ({})", max_progress, max_decimal);
                }
                
                // 6. ULTRA-PRECISE ORDERING with podcast grouping support
                let mut order_parts = Vec::new();
                
                if playlist.try_get::<bool, _>("groupbypodcast")? {
                    order_parts.push("p.podcastid".to_string());
                    debug!("📚 Grouping by podcast enabled");
                }
                
                let sort_clause = match playlist.try_get::<String, _>("sortorder")?.as_str() {
                    "date_asc" => "episodepubdate ASC",
                    "date_desc" => "episodepubdate DESC", 
                    "duration_asc" => "episodeduration ASC",
                    "duration_desc" => "episodeduration DESC",
                    "listen_progress" => "(COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) DESC",
                    "completion" => "(COALESCE(h.listenduration, 0)::float / NULLIF(e.episodeduration, 0)) DESC",
                    "random" => "RANDOM()",
                    "title_asc" => "episodetitle ASC",
                    "title_desc" => "episodetitle DESC",
                    "podcast_asc" => "podcastname ASC",
                    "podcast_desc" => "podcastname DESC",
                    _ => {
                        warn!("⚠️ Unknown sort order '{}', defaulting to date_desc", playlist.try_get::<String, _>("sortorder")?);
                        "episodepubdate DESC"
                    }
                };
                order_parts.push(sort_clause.to_string());
                debug!("🔄 Sort order: {}", sort_clause);
                
                // 7. BUILD FINAL QUERY
                let where_clause = if where_conditions.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", where_conditions.join(" AND "))
                };

                let order_clause = format!(" ORDER BY {}", order_parts.join(", "));

                // Count total matching episodes (respecting maxepisodes if set)
                let max_eps_opt = playlist.try_get::<Option<i32>, _>("maxepisodes")?;
                let count_query = format!(
                    r#"SELECT COUNT(DISTINCT e.episodeid) FROM "Episodes" e
                    JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = {}
                    LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = {}{}"#,
                    user_id, user_id, where_clause
                );
                let raw_total: i64 = q::query_scalar(sqlx::AssertSqlSafe(count_query.as_str()))
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                let total_count = if let Some(max_eps) = max_eps_opt {
                    if max_eps > 0 { raw_total.min(max_eps as i64) } else { raw_total }
                } else {
                    raw_total
                };

                // Compute effective page limit respecting maxepisodes
                let page_limit = if let Some(max_eps) = max_eps_opt {
                    if max_eps > 0 {
                        let remaining = (max_eps as i64).saturating_sub(offset);
                        limit.min(remaining).max(0)
                    } else {
                        limit
                    }
                } else {
                    limit
                };
                let pagination_clause = format!(" LIMIT {} OFFSET {}", page_limit, offset);

                let final_query = format!("{}{}{}{}",
                    query_parts.join(" "), where_clause, order_clause, pagination_clause);

                debug!("🔍 Final dynamic playlist query: {}", final_query);

                // Execute the main query
                let rows = q::query(sqlx::AssertSqlSafe(final_query.as_str()))
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::SavedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: row.try_get("episodepubdate")?,
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        websiteurl: row.try_get("websiteurl")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                        podcastid: row.try_get("podcastid").ok(),
                        savedate: None,
                    });
                }

                debug!("📝 Retrieved {} episodes from dynamic query (total: {})", episodes.len(), total_count);

                // Create playlist info from the playlist row we already have
                let min_dur_secs: Option<i32> = playlist.try_get("minduration")?;
                let max_dur_secs: Option<i32> = playlist.try_get("maxduration")?;
                let podcast_ids: Option<Vec<i32>> = q::int_array(&playlist, "podcastids").ok().flatten();
                let playlist_info = crate::models::PlaylistInfo {
                    name: playlist.try_get::<String, _>("name")?,
                    description: playlist.try_get::<Option<String>, _>("description")?.unwrap_or_default(),
                    episode_count: total_count as i32,
                    icon_name: playlist.try_get::<String, _>("iconname")?,
                    is_system_playlist: playlist.try_get::<bool, _>("issystemplaylist")?,
                    podcast_ids,
                    include_unplayed: playlist.try_get::<bool, _>("includeunplayed")?,
                    include_partially_played: playlist.try_get::<bool, _>("includepartiallyplayed")?,
                    include_played: playlist.try_get::<bool, _>("includeplayed")?,
                    min_duration: min_dur_secs.map(|s| s / 60),
                    max_duration: max_dur_secs.map(|s| s / 60),
                    sort_order: playlist.try_get::<String, _>("sortorder")?,
                    group_by_podcast: playlist.try_get::<bool, _>("groupbypodcast")?,
                    max_episodes: playlist.try_get::<Option<i32>, _>("maxepisodes")?,
                    play_progress_min: playlist.try_get::<Option<f64>, _>("playprogressmin")?,
                    play_progress_max: playlist.try_get::<Option<f64>, _>("playprogressmax")?,
                    time_filter_hours: playlist.try_get::<Option<i32>, _>("timefilterhours")?,
                };

                Ok(crate::models::PlaylistEpisodesResponse {
                    episodes,
                    playlist_info,
                    total: total_count,
                })
            },
            mysql(pool) => {
                // FULL MySQL implementation with exact same logic but MySQL syntax
                let raw_user_timezone: String = sqlx::query_scalar("SELECT TimeZone FROM Users WHERE UserID = ?").bind(user_id)
                    .fetch_optional(pool).await?.unwrap_or_else(|| "UTC".to_string());
                let raw_timezone = if raw_user_timezone.is_empty() { "UTC".to_string() } else { raw_user_timezone };
                let user_timezone = Self::map_timezone_for_postgres(&raw_timezone);
                
                debug!("User {} timezone: {} -> {}", user_id, raw_timezone, user_timezone);
                
                let playlist_row = sqlx::query(
                    r#"SELECT UserID, Name, Description, MinDuration, MaxDuration, SortOrder,
                       IncludeUnplayed, IncludePartiallyPlayed, IncludePlayed, TimeFilterHours,
                       GroupByPodcast, MaxEpisodes, PlayProgressMin, PlayProgressMax, PodcastIDs,
                       IsSystemPlaylist, Created, IconName, EpisodeCount
                       FROM Playlists WHERE PlaylistID = ?"#
                ).bind(playlist_id).fetch_optional(pool).await?;
                
                let playlist = playlist_row.ok_or_else(|| crate::error::AppError::not_found("Playlist not found"))?;
                
                if playlist.try_get::<i32, _>("UserID")? != user_id {
                    return Err(crate::error::AppError::forbidden("You can only access your own playlists"));
                }
                
                debug!("📋 MySQL Playlist '{}' config loaded", playlist.try_get::<String, _>("Name")?);
                
                // MySQL version with adjusted syntax for timezone conversion and date handling
                let mut query_parts = Vec::new();
                let mut where_conditions = Vec::new();
                
                query_parts.push(format!(r#"
                    SELECT DISTINCT
                        e.EpisodeTitle as episodetitle,
                        p.PodcastName as podcastname,
                        DATE_FORMAT(CONVERT_TZ(e.EpisodePubDate, 'UTC', '{}'), '%Y-%m-%dT%H:%i:%sZ') as episodepubdate,
                        e.EpisodeDescription as episodedescription,
                        CASE
                            WHEN p.UsePodcastCoversCustomized = TRUE AND p.UsePodcastCovers = TRUE THEN p.ArtworkURL
                            WHEN u.UsePodcastCovers = TRUE THEN p.ArtworkURL
                            ELSE COALESCE(e.EpisodeArtwork, p.ArtworkURL)
                        END as episodeartwork,
                        e.EpisodeURL as episodeurl,
                        e.EpisodeDuration as episodeduration,
                        COALESCE(h.ListenDuration, 0) as listenduration,
                        e.EpisodeID as episodeid,
                        COALESCE(p.WebsiteURL, '') as websiteurl,
                        CASE WHEN 
                            h.ListenDuration IS NOT NULL AND (
                                h.ListenDuration >= e.EpisodeDuration * 0.9 OR 
                                (e.EpisodeDuration - h.ListenDuration) <= 30
                            ) 
                        THEN 1 ELSE 0 END as completed,
                        EXISTS(SELECT 1 FROM SavedEpisodes se WHERE se.EpisodeID = e.EpisodeID AND se.UserID = {}) as saved,
                        EXISTS(SELECT 1 FROM EpisodeQueue eq WHERE eq.EpisodeID = e.EpisodeID AND eq.UserID = {}) as queued,
                        EXISTS(SELECT 1 FROM DownloadedEpisodes de WHERE de.EpisodeID = e.EpisodeID AND de.UserID = {}) as downloaded,
                        0 as is_youtube,
                        p.PodcastID as podcastid,
                        ROUND((COALESCE(h.ListenDuration, 0) / NULLIF(e.EpisodeDuration, 0)) * 100, 2) as progress_percent
                    FROM Episodes e
                    JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = {}
                    LEFT JOIN Users u ON p.UserID = u.UserID
                    LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = {}"#,
                    user_timezone, user_id, user_id, user_id, user_id, user_id
                ));
                
                where_conditions.push("p.UserID = ?".to_string());
                
                // Apply all the same filters with MySQL syntax
                if let Some(min_dur) = playlist.try_get::<Option<i32>, _>("MinDuration")? {
                    where_conditions.push(format!("e.EpisodeDuration >= {}", min_dur));
                }
                if let Some(max_dur) = playlist.try_get::<Option<i32>, _>("MaxDuration")? {
                    where_conditions.push(format!("e.EpisodeDuration <= {}", max_dur));
                }
                
                if let Some(hours) = playlist.try_get::<Option<i32>, _>("TimeFilterHours")? {
                    where_conditions.push(format!(
                        "e.EpisodePubDate >= DATE_SUB(CONVERT_TZ(NOW(), 'UTC', '{}'), INTERVAL {} HOUR)", 
                        user_timezone, hours
                    ));
                    debug!("📅 Added MySQL timezone-aware time filter: last {} hours in timezone {}", hours, user_timezone);
                }
                
                // 3. PODCAST FILTER - handle JSON array of podcast IDs (MySQL)
                // MariaDB's JSON column type is reported over the wire with a binary
                // charset, so sqlx sees it as SQL type BLOB rather than VARCHAR/TEXT.
                // Decoding it directly as Option<String> then fails with a "mismatched
                // types" error on every request (see #773). Decode as raw bytes first,
                // matching the fallback already used in PlaylistConfig::from_mysql_row,
                // and only fall back to a direct String decode for installs where the
                // driver does report a text charset for this column.
                let podcast_ids_json_owned: Option<String> = match playlist.try_get::<Option<Vec<u8>>, _>("PodcastIDs") {
                    Ok(bytes_opt) => bytes_opt.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
                    Err(_) => playlist.try_get::<Option<String>, _>("PodcastIDs")?,
                };
                if let Some(podcast_ids_json) = podcast_ids_json_owned.as_ref() {
                    if !podcast_ids_json.is_empty() && podcast_ids_json != "[]" && podcast_ids_json != "null" {
                        match serde_json::from_str::<Vec<i32>>(podcast_ids_json) {
                            Ok(podcast_ids) if !podcast_ids.is_empty() && !podcast_ids.contains(&-1) => {
                                let podcast_ids_str = podcast_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                                where_conditions.push(format!("p.PodcastID IN ({})", podcast_ids_str));
                                debug!("🎙️ Added MySQL podcast filter: {:?}", podcast_ids);
                            }
                            Ok(_) => debug!("🎙️ MySQL podcast filter contains -1 or is empty, including all podcasts"),
                            Err(e) => warn!("⚠️ Failed to parse MySQL podcast IDs JSON '{}': {}", podcast_ids_json, e),
                        }
                    }
                }
                
                // 4. ULTRA-PRECISE PLAY STATE FILTERS (MySQL)
                let mut play_state_conditions = Vec::new();
                
                if playlist.try_get::<bool, _>("IncludeUnplayed")? {
                    play_state_conditions.push("(COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) IS NOT TRUE AND (h.ListenDuration IS NULL OR h.ListenDuration = 0))".to_string());
                    debug!("▶️ Including UNPLAYED episodes (MySQL)");
                }
                
                if playlist.try_get::<bool, _>("IncludePartiallyPlayed")? {
                    play_state_conditions.push(
                        "(h.ListenDuration > 0 AND h.ListenDuration < e.EpisodeDuration * 0.9 AND (e.EpisodeDuration - h.ListenDuration) > 30)".to_string()
                    );
                    debug!("⏸️ Including PARTIALLY PLAYED episodes (MySQL)");
                }
                
                if playlist.try_get::<bool, _>("IncludePlayed")? {
                    play_state_conditions.push(
                        "(COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = p.UserID), 0) IS TRUE OR (h.ListenDuration IS NOT NULL AND (h.ListenDuration >= e.EpisodeDuration * 0.9 OR (e.EpisodeDuration - h.ListenDuration) <= 30)))".to_string()
                    );
                    debug!("✅ Including PLAYED episodes (MySQL)");
                }
                
                if !play_state_conditions.is_empty() {
                    where_conditions.push(format!("({})", play_state_conditions.join(" OR ")));
                } else {
                    where_conditions.push("FALSE".to_string());
                    warn!("⚠️ No play states selected for MySQL playlist '{}' - will return empty results", playlist.try_get::<String, _>("Name")?);
                }
                
                // 5. ULTRA-PRECISE PROGRESS PERCENTAGE FILTERS (MySQL)
                if let Some(min_progress) = playlist.try_get::<Option<f64>, _>("PlayProgressMin")? {
                    let min_decimal = min_progress / 100.0;
                    where_conditions.push(format!(
                        "(COALESCE(h.ListenDuration, 0) / NULLIF(e.EpisodeDuration, 0)) >= {}", 
                        min_decimal
                    ));
                    debug!("📊 Added MySQL min progress filter: {}% ({})", min_progress, min_decimal);
                }
                if let Some(max_progress) = playlist.try_get::<Option<f64>, _>("PlayProgressMax")? {
                    let max_decimal = max_progress / 100.0;
                    where_conditions.push(format!(
                        "(COALESCE(h.ListenDuration, 0) / NULLIF(e.EpisodeDuration, 0)) <= {}", 
                        max_decimal
                    ));
                    debug!("📊 Added MySQL max progress filter: {}% ({})", max_progress, max_decimal);
                }
                
                // 6. ULTRA-PRECISE ORDERING with podcast grouping support (MySQL)
                let mut order_parts = Vec::new();
                
                if playlist.try_get::<bool, _>("GroupByPodcast")? {
                    order_parts.push("podcastid".to_string());
                    debug!("📚 MySQL grouping by podcast enabled");
                }
                
                let sort_clause = match playlist.try_get::<String, _>("SortOrder")?.as_str() {
                    "date_asc" => "episodepubdate ASC",
                    "date_desc" => "episodepubdate DESC", 
                    "duration_asc" => "episodeduration ASC",
                    "duration_desc" => "episodeduration DESC",
                    "listen_progress" => "(COALESCE(h.ListenDuration, 0) / NULLIF(e.EpisodeDuration, 0)) DESC",
                    "completion" => "(COALESCE(h.ListenDuration, 0) / NULLIF(e.EpisodeDuration, 0)) DESC",
                    "random" => "RAND()",
                    "title_asc" => "episodetitle ASC",
                    "title_desc" => "episodetitle DESC",
                    "podcast_asc" => "podcastname ASC",
                    "podcast_desc" => "podcastname DESC",
                    _ => {
                        warn!("⚠️ Unknown MySQL sort order '{}', defaulting to date_desc", playlist.try_get::<String, _>("SortOrder")?);
                        "episodepubdate DESC"
                    }
                };
                order_parts.push(sort_clause.to_string());
                debug!("🔄 MySQL Sort order: {}", sort_clause);
                
                // 7. BUILD FINAL MYSQL QUERY
                let where_clause = if where_conditions.is_empty() {
                    String::new()
                } else {
                    format!(" WHERE {}", where_conditions.join(" AND "))
                };

                let order_clause = format!(" ORDER BY {}", order_parts.join(", "));

                // Count total matching episodes (MySQL)
                let max_eps_opt = playlist.try_get::<Option<i32>, _>("MaxEpisodes")?;
                let count_query = format!(
                    "SELECT COUNT(DISTINCT e.EpisodeID) FROM Episodes e
                    JOIN Podcasts p ON e.FeedID = p.FeedID AND p.UserID = {}
                    LEFT JOIN UserEpisodeHistory h ON e.EpisodeID = h.EpisodeID AND h.UserID = {}{}",
                    user_id, user_id, where_clause
                );
                let raw_total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_query.as_str()))
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                let total_count = if let Some(max_eps) = max_eps_opt {
                    if max_eps > 0 { raw_total.min(max_eps as i64) } else { raw_total }
                } else {
                    raw_total
                };

                // Compute effective page limit respecting MaxEpisodes (MySQL)
                let page_limit = if let Some(max_eps) = max_eps_opt {
                    if max_eps > 0 {
                        let remaining = (max_eps as i64).saturating_sub(offset);
                        limit.min(remaining).max(0)
                    } else {
                        limit
                    }
                } else {
                    limit
                };
                let pagination_clause = format!(" LIMIT {} OFFSET {}", page_limit, offset);

                let final_query = format!("{}{}{}{}",
                    query_parts.join(" "), where_clause, order_clause, pagination_clause);

                debug!("🔍 Final MySQL dynamic playlist query: {}", final_query);

                // Execute the main MySQL query
                let rows = sqlx::query(sqlx::AssertSqlSafe(final_query.as_str()))
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?;

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::SavedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: row.try_get("episodepubdate")?,
                        episodedescription: row.try_get("episodedescription")?,
                        episodeartwork: row.try_get("episodeartwork")?,
                        episodeurl: row.try_get("episodeurl")?,
                        episodeduration: row.try_get("episodeduration")?,
                        listenduration: row.try_get("listenduration").ok(),
                        episodeid: row.try_get("episodeid")?,
                        websiteurl: row.try_get("websiteurl")?,
                        completed: row.try_get("completed")?,
                        saved: row.try_get("saved")?,
                        queued: row.try_get("queued")?,
                        downloaded: row.try_get("downloaded")?,
                        is_youtube: row.try_get("is_youtube")?,
                        podcastid: row.try_get("podcastid").ok(),
                        savedate: None,
                    });
                }

                debug!("📝 Retrieved {} episodes from MySQL dynamic query (total: {})", episodes.len(), total_count);

                // Create playlist info from the MySQL playlist row we already have
                let min_dur_secs: Option<i32> = playlist.try_get("MinDuration")?;
                let max_dur_secs: Option<i32> = playlist.try_get("MaxDuration")?;
                let is_system_raw: i8 = playlist.try_get("IsSystemPlaylist")?;
                // Same BLOB-vs-VARCHAR decode as the podcast filter above (#773) - without
                // this, `.ok()` silently swallows the decode error and podcast_ids always
                // reports as None here on installs where MariaDB reports this column as BLOB.
                let podcast_ids_raw: Option<String> = match playlist.try_get::<Option<Vec<u8>>, _>("PodcastIDs") {
                    Ok(bytes_opt) => bytes_opt.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()),
                    Err(_) => playlist.try_get("PodcastIDs").ok().flatten(),
                };
                let podcast_ids: Option<Vec<i32>> = podcast_ids_raw
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok());
                let include_unplayed_raw: i8 = playlist.try_get("IncludeUnplayed")?;
                let include_partially_raw: i8 = playlist.try_get("IncludePartiallyPlayed")?;
                let include_played_raw: i8 = playlist.try_get("IncludePlayed")?;
                let group_by_raw: i8 = playlist.try_get("GroupByPodcast")?;
                let playlist_info = crate::models::PlaylistInfo {
                    name: playlist.try_get::<String, _>("Name")?,
                    description: playlist.try_get::<Option<String>, _>("Description")?.unwrap_or_default(),
                    episode_count: total_count as i32,
                    icon_name: playlist.try_get::<String, _>("IconName")?,
                    is_system_playlist: is_system_raw != 0,
                    podcast_ids,
                    include_unplayed: include_unplayed_raw != 0,
                    include_partially_played: include_partially_raw != 0,
                    include_played: include_played_raw != 0,
                    min_duration: min_dur_secs.map(|s| s / 60),
                    max_duration: max_dur_secs.map(|s| s / 60),
                    sort_order: playlist.try_get::<String, _>("SortOrder")?,
                    group_by_podcast: group_by_raw != 0,
                    max_episodes: playlist.try_get::<Option<i32>, _>("MaxEpisodes")?,
                    play_progress_min: playlist.try_get::<Option<f64>, _>("PlayProgressMin")?,
                    play_progress_max: playlist.try_get::<Option<f64>, _>("PlayProgressMax")?,
                    time_filter_hours: playlist.try_get::<Option<i32>, _>("TimeFilterHours")?,
                };

                Ok(crate::models::PlaylistEpisodesResponse {
                    episodes,
                    playlist_info,
                    total: total_count,
                })
            },
        })
    }
}

#[cfg(test)]
mod timezone_mapping_tests {
    use super::DatabasePool;

    #[test]
    fn known_and_iana_names_pass_through() {
        assert_eq!(DatabasePool::map_timezone_for_postgres("US/Eastern"), "America/New_York");
        assert_eq!(DatabasePool::map_timezone_for_postgres("Europe/Berlin"), "Europe/Berlin");
    }

    #[test]
    fn unknown_or_hostile_values_become_utc() {
        for tz in ["", "UTC'", "Z','D')||(SELECT current_user)||('", "Not/AZone"] {
            assert_eq!(DatabasePool::map_timezone_for_postgres(tz), "UTC");
        }
    }
}

dialect_method!([update_fresh_releases_playlist_postgres, update_fresh_releases_playlist_lite]
    pub(super) async fn(&self, pool: &q::Pool, playlist_id: i32) -> AppResult<i32> {
    tracing::info!("Updating Fresh Releases playlist with timezone logic");
    
    // Get all users with their timezones
    let users = q::query(r#"SELECT userid, timezone FROM "Users" WHERE userid = (SELECT userid FROM "Playlists" WHERE playlistid = $1)"#)
        .bind(playlist_id)
        .fetch_all(pool)
        .await?;
    
    let mut added_episodes: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let mut position = 1;
    
    for user in users {
        let user_id: i32 = user.try_get("userid")?;
        let timezone: Option<String> = user.try_get("timezone").ok();
        let raw_tz = timezone.as_deref()
            .filter(|s| !s.is_empty()) // Filter out empty strings
            .unwrap_or("UTC");
        
        let normalized_tz = Self::normalize_timezone(raw_tz);
        
        tracing::info!("Processing Fresh Releases for user {} with timezone {} (normalized: {})", user_id, raw_tz, normalized_tz);
        
        // Get episodes from last 24 hours in user's timezone
        let episodes = q::query(r#"
            SELECT e.episodeid
            FROM "Episodes" e
            JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = $2
            WHERE e.episodepubdate AT TIME ZONE 'UTC' AT TIME ZONE $1 >
                  (CURRENT_TIMESTAMP AT TIME ZONE 'UTC' AT TIME ZONE $1 - INTERVAL '24 hours')
            ORDER BY e.episodepubdate DESC
        "#)
            .bind(&normalized_tz)
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        
        // Add unique episodes to playlist
        for episode in episodes {
            let episode_id: i32 = episode.try_get("episodeid")?;
            if !added_episodes.contains(&episode_id) {
                q::query(r#"
                    INSERT INTO "PlaylistContents" (playlistid, episodeid, position)
                    VALUES ($1, $2, $3)
                "#)
                    .bind(playlist_id)
                    .bind(episode_id)
                    .bind(position)
                    .execute(pool)
                    .await?;
                
                added_episodes.insert(episode_id);
                position += 1;
            }
        }
    }
    
    tracing::info!("Fresh Releases playlist updated with {} episodes", added_episodes.len());
    Ok(added_episodes.len() as i32)
});

dialect_method!([update_currently_listening_playlist_postgres, update_currently_listening_playlist_lite]
    pub(super) async fn(&self, pool: &q::Pool, playlist_id: i32) -> AppResult<i32> {
    tracing::info!("Updating Currently Listening playlist");
    
    // Get all users and their currently listening episodes
    let users = q::query(r#"SELECT userid FROM "Users" WHERE userid = (SELECT userid FROM "Playlists" WHERE playlistid = $1)"#)
        .bind(playlist_id)
        .fetch_all(pool)
        .await?;
    
    let mut added_episodes: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let mut position = 1;
    
    for user in users {
        let user_id: i32 = user.try_get("userid")?;
        
        tracing::info!("Processing Currently Listening for user {}", user_id);
        
        // Get episodes user has started but not finished
        let episodes = q::query(r#"
            SELECT e.episodeid
            FROM "Episodes" e
            JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid
            WHERE h.userid = $1
            AND h.listenduration > 0
            AND h.listenduration < e.episodeduration
            ORDER BY h.listendate DESC
        "#)
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        
        // Add unique episodes to playlist
        for episode in episodes {
            let episode_id: i32 = episode.try_get("episodeid")?;
            if !added_episodes.contains(&episode_id) {
                q::query(r#"INSERT INTO "PlaylistContents" (playlistid, episodeid, position) VALUES ($1, $2, $3)"#)
                    .bind(playlist_id)
                    .bind(episode_id)
                    .bind(position)
                    .execute(pool)
                    .await?;
                
                added_episodes.insert(episode_id);
                position += 1;
            }
        }
    }
    
    tracing::info!("Currently Listening playlist updated with {} episodes", added_episodes.len());
    Ok(added_episodes.len() as i32)
});

dialect_method!([update_almost_done_playlist_postgres, update_almost_done_playlist_lite]
    pub(super) async fn(&self, pool: &q::Pool, playlist_id: i32) -> AppResult<i32> {
    tracing::info!("Updating Almost Done playlist");
    
    // Get all users and their almost done episodes
    let users = q::query(r#"SELECT userid FROM "Users" WHERE userid = (SELECT userid FROM "Playlists" WHERE playlistid = $1)"#)
        .bind(playlist_id)
        .fetch_all(pool)
        .await?;
    
    let mut added_episodes: std::collections::HashSet<i32> = std::collections::HashSet::new();
    let mut position = 1;
    
    for user in users {
        let user_id: i32 = user.try_get("userid")?;
        
        tracing::info!("Processing Almost Done for user {}", user_id);
        
        // Get episodes user is 75%+ through but not completed
        let episodes = q::query(r#"
            SELECT e.episodeid
            FROM "Episodes" e
            JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid
            WHERE h.userid = $1
            AND h.listenduration > 0
            AND h.listenduration < e.episodeduration
            AND (h.listenduration::float / NULLIF(e.episodeduration, 0)) >= 0.75
            ORDER BY h.listendate DESC
        "#)
            .bind(user_id)
            .fetch_all(pool)
            .await?;
        
        // Add unique episodes to playlist
        for episode in episodes {
            let episode_id: i32 = episode.try_get("episodeid")?;
            if !added_episodes.contains(&episode_id) {
                q::query(r#"INSERT INTO "PlaylistContents" (playlistid, episodeid, position) VALUES ($1, $2, $3)"#)
                    .bind(playlist_id)
                    .bind(episode_id)
                    .bind(position)
                    .execute(pool)
                    .await?;
                
                added_episodes.insert(episode_id);
                position += 1;
            }
        }
    }
    
    tracing::info!("Almost Done playlist updated with {} episodes", added_episodes.len());
    Ok(added_episodes.len() as i32)
});

dialect_method!([update_quick_listens_playlist_postgres, update_quick_listens_playlist_lite]
    pub(super) async fn(&self, pool: &q::Pool, playlist_id: i32) -> AppResult<i32> {
    tracing::info!("Updating Quick Listens playlist");
    
    // Get shortest 1000 episodes under 15 minutes (900 seconds) and over 1 second from ALL podcasts
    let episodes = q::query(r#"
        SELECT e.episodeid
        FROM "Episodes" e
        JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = (SELECT userid FROM "Playlists" WHERE playlistid = $1)
        WHERE e.episodeduration >= 1
        AND e.episodeduration <= 900
        AND COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) = FALSE
        ORDER BY e.episodeduration ASC
        LIMIT 1000
    "#)
        .bind(playlist_id)
        .fetch_all(pool)
        .await?;
    
    let mut position = 1;
    
    // Add episodes to playlist
    for episode in episodes {
        let episode_id: i32 = episode.try_get("episodeid")?;
        q::query(r#"INSERT INTO "PlaylistContents" (playlistid, episodeid, position) VALUES ($1, $2, $3)"#)
            .bind(playlist_id)
            .bind(episode_id)
            .bind(position)
            .execute(pool)
            .await?;
        
        position += 1;
    }
    
    let episode_count = position - 1;
    tracing::info!("Quick Listens playlist updated with {} episodes", episode_count);
    Ok(episode_count)
});

dialect_method!([build_and_execute_playlist_query_postgres, build_and_execute_playlist_query_lite]
    pub(super) async fn(&self, pool: &q::Pool, playlist: &q::Row) -> AppResult<i32> {
    let playlist_id: i32 = playlist.try_get("playlistid")?;
    let user_id: i32 = playlist.try_get("userid")?;
    let playlist_name: String = playlist.try_get("name")?;
    let is_system_playlist: bool = playlist.try_get("issystemplaylist").unwrap_or(false);
    
    // Parse playlist configuration
    let config = PlaylistConfig::from_row(q::raw(playlist), q::int_array(playlist, "podcastids"))?;
    
    // Determine if this playlist needs user history filtering
    let needs_user_history = playlist_name == "Currently Listening" || 
                              playlist_name == "Almost Done" || 
                              !is_system_playlist;
    
    // Check for special optimized queries for partially played
    if config.include_partially_played && !config.include_unplayed && !config.include_played {
        return q::pick(Self::execute_partially_played_query_postgres, Self::execute_partially_played_query_lite)(self, pool, playlist_id, user_id, &config).await;
    }
    
    // Build the appropriate base query - FIXED PARAMETER INDEXING
    let (base_query, params) = if is_system_playlist {
        if needs_user_history {
            // System playlist with user history filtering
            (r#"
                SELECT e.episodeid, p.podcastid, u.timezone
                FROM "Episodes" e
                JOIN "Podcasts" p ON e.feedid = p.feedid
                LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = $2
                JOIN "Users" u ON u.userid = $3
                WHERE p.userid = u.userid
            "#.to_string(), vec![user_id, user_id])
        } else {
            // System playlist without user history filtering (Fresh Releases, etc.)  
            // But still need user context for history when needed
            (r#"
                SELECT e.episodeid, p.podcastid, u.timezone
                FROM "Episodes" e
                JOIN "Podcasts" p ON e.feedid = p.feedid
                LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = $2
                JOIN "Users" u ON u.userid = $3
                WHERE p.userid = u.userid
            "#.to_string(), vec![user_id, user_id])
        }
    } else {
        // User-specific playlist
        (r#"
            SELECT e.episodeid, p.podcastid, u.timezone
            FROM "Episodes" e
            JOIN "Podcasts" p ON e.feedid = p.feedid
            LEFT JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid AND h.userid = $2
            JOIN "Users" u ON u.userid = $3
            WHERE p.userid = $4
        "#.to_string(), vec![user_id, user_id, user_id])
    };
    
    // Build the complete query with all filters
    let (complete_query, all_params) = self.build_complete_postgres_query(
        base_query, params, &config, playlist_id
    )?;
    
    // Execute the query and insert episodes
    q::pick(Self::execute_playlist_query_postgres, Self::execute_playlist_query_lite)(self, pool, &complete_query, &all_params, playlist_id).await
});

dialect_method!([execute_partially_played_query_postgres, execute_partially_played_query_lite]
    pub(super) async fn(&self, pool: &q::Pool, playlist_id: i32, user_id: i32, config: &PlaylistConfig) -> AppResult<i32> {
    // Use direct INSERT without subquery for this optimized case - no alias scoping issues
    let sort_order = config.get_postgres_sort_order().replace("ORDER BY ", "");
    
    // Build base query
    let mut query = format!(r#"
        INSERT INTO "PlaylistContents" (playlistid, episodeid, position)
        SELECT $1, e.episodeid, ROW_NUMBER() OVER (ORDER BY {}) as position
        FROM "Episodes" e
        JOIN "UserEpisodeHistory" h ON e.episodeid = h.episodeid
        JOIN "Podcasts" p ON e.feedid = p.feedid AND p.userid = h.userid
        WHERE h.listenduration > 0
        AND h.listenduration < e.episodeduration
        AND COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = e.episodeid AND ues.userid = p.userid), FALSE) = FALSE
        AND e.episodeduration > 0
        AND h.userid = $2
    "#, sort_order);
    
    let params = vec![playlist_id, user_id];
    
    // Add progress filters using hardcoded values instead of parameters to avoid type issues
    if let Some(min_progress) = config.play_progress_min {
        query.push_str(&format!(" AND (h.listenduration::float / NULLIF(e.episodeduration, 0)) >= {}", min_progress / 100.0));
    }
    
    if let Some(max_progress) = config.play_progress_max {
        query.push_str(&format!(" AND (h.listenduration::float / NULLIF(e.episodeduration, 0)) <= {}", max_progress / 100.0));
    }
    
    // Add limit
    if let Some(max_episodes) = config.max_episodes {
        query.push_str(&format!(" LIMIT {}", max_episodes));
    }
    
    tracing::info!("Executing partially played query with {} parameters", params.len());
    tracing::debug!("Query: {}", query);
    
    // Execute with proper parameter binding
    let mut sqlx_query = q::query(sqlx::AssertSqlSafe(query.as_str()));
    for param in &params {
        sqlx_query = sqlx_query.bind(*param);
    }
    
    let result = sqlx_query.execute(pool).await?;
    Ok(result.rows_affected() as i32)
});

dialect_method!([execute_playlist_query_postgres, execute_playlist_query_lite]
    pub(super) async fn(&self, pool: &q::Pool, query: &str, params: &[i32], _playlist_id: i32) -> AppResult<i32> {
    info!("PostgreSQL executing playlist query with {} parameters", params.len());
    info!("PostgreSQL Query: {}", query);
    info!("PostgreSQL Params: {:?}", params);
    
    // Build query with proper parameter binding
    let query_owned = query.to_owned();
    let mut sqlx_query = q::query(sqlx::AssertSqlSafe(query_owned.as_str()));
    for param in params {
        sqlx_query = sqlx_query.bind(*param);
    }

    let result = sqlx_query.execute(pool).await?;
    info!("PostgreSQL playlist query affected {} rows", result.rows_affected());
    Ok(result.rows_affected() as i32)
});
