//! New-episode notifications and their settings.

use super::*;

/// One user notification channel for new episodes of a podcast.
pub(super) struct NotificationTarget {
    podcast_name: String,
    platform: String,
    ntfy_topic: Option<String>,
    ntfy_server_url: Option<String>,
    ntfy_username: Option<String>,
    ntfy_password: Option<String>,
    ntfy_access_token: Option<String>,
    gotify_url: Option<String>,
    gotify_token: Option<String>,
    http_url: Option<String>,
    http_token: Option<String>,
    http_method: Option<String>,
}

/// The chat id from a Telegram notification token written as `<bot token>:<chat id>`. Bot tokens
/// contain a colon themselves (`123456:ABC…`), so the chat id is what follows the last one.
pub(crate) fn telegram_chat_id(token: &str) -> Option<&str> {
    match token.rsplit_once(':') {
        Some((bot, chat)) if bot.contains(':') && !chat.is_empty() => Some(chat),
        _ => None,
    }
}

impl DatabasePool {
    /// Where to announce new episodes of `podcast_id`: the owner's enabled notification settings,
    /// if notifications are on for this podcast. Loaded once per refresh of the podcast.
    pub(super) async fn load_notification_targets(&self, podcast_id: i32) -> AppResult<Vec<NotificationTarget>> {
        let mut targets = Vec::new();
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT p.podcastname, uns.platform, uns.ntfytopic, uns.ntfyserverurl,
                           uns.ntfyusername, uns.ntfypassword, uns.ntfyaccesstoken,
                           uns.gotifyurl, uns.gotifytoken, uns.httpurl, uns.httptoken, uns.httpmethod
                    FROM "Podcasts" p
                    JOIN "UserNotificationSettings" uns ON p.userid = uns.userid
                    WHERE p.podcastid = $1 AND p.notificationsenabled = true AND uns.enabled = true"#
                )
                .bind(podcast_id)
                .fetch_all(pool)
                .await?;
                for row in rows {
                    targets.push(NotificationTarget {
                        podcast_name: row.try_get("podcastname")?,
                        platform: row.try_get("platform")?,
                        ntfy_topic: row.try_get("ntfytopic").ok(),
                        ntfy_server_url: row.try_get("ntfyserverurl").ok(),
                        ntfy_username: row.try_get("ntfyusername").ok(),
                        ntfy_password: row.try_get("ntfypassword").ok(),
                        ntfy_access_token: row.try_get("ntfyaccesstoken").ok(),
                        gotify_url: row.try_get("gotifyurl").ok(),
                        gotify_token: row.try_get("gotifytoken").ok(),
                        http_url: row.try_get("httpurl").ok(),
                        http_token: row.try_get("httptoken").ok(),
                        http_method: row.try_get("httpmethod").ok(),
                    });
                }
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT p.PodcastName, uns.Platform, uns.NtfyTopic, uns.NtfyServerUrl,
                            uns.NtfyUsername, uns.NtfyPassword, uns.NtfyAccessToken,
                            uns.GotifyUrl, uns.GotifyToken, uns.HttpUrl, uns.HttpToken, uns.HttpMethod
                     FROM Podcasts p
                     JOIN UserNotificationSettings uns ON p.UserID = uns.UserID
                     WHERE p.PodcastID = ? AND p.NotificationsEnabled = true AND uns.Enabled = true"
                )
                .bind(podcast_id)
                .fetch_all(pool)
                .await?;
                for row in rows {
                    targets.push(NotificationTarget {
                        podcast_name: row.try_get("PodcastName")?,
                        platform: row.try_get("Platform")?,
                        ntfy_topic: row.try_get("NtfyTopic").ok(),
                        ntfy_server_url: row.try_get("NtfyServerUrl").ok(),
                        ntfy_username: row.try_get("NtfyUsername").ok(),
                        ntfy_password: row.try_get("NtfyPassword").ok(),
                        ntfy_access_token: row.try_get("NtfyAccessToken").ok(),
                        gotify_url: row.try_get("GotifyUrl").ok(),
                        gotify_token: row.try_get("GotifyToken").ok(),
                        http_url: row.try_get("HttpUrl").ok(),
                        http_token: row.try_get("HttpToken").ok(),
                        http_method: row.try_get("HttpMethod").ok(),
                    });
                }
            },
        });
        Ok(targets)
    }

    /// Announce one new episode on every target. Returns whether any send succeeded; failures
    /// are logged by the senders.
    pub(super) async fn send_new_episode_notifications(targets: &[NotificationTarget], episode_title: &str) -> bool {
        let client = crate::services::http::service_client();
        let mut success = false;
        for t in targets {
            let sent = match t.platform.as_str() {
                "ntfy" => match (&t.ntfy_topic, &t.ntfy_server_url) {
                    (Some(topic), Some(server_url)) => Self::send_ntfy_notification(
                        client,
                        topic,
                        server_url,
                        t.ntfy_username.as_deref(),
                        t.ntfy_password.as_deref(),
                        t.ntfy_access_token.as_deref(),
                        &t.podcast_name,
                        episode_title,
                    )
                    .await
                    .unwrap_or(false),
                    _ => false,
                },
                "gotify" => match (&t.gotify_url, &t.gotify_token) {
                    (Some(url), Some(token)) => {
                        Self::send_gotify_notification(client, url, token, &t.podcast_name, episode_title)
                            .await
                            .unwrap_or(false)
                    }
                    _ => false,
                },
                "http" => match &t.http_url {
                    Some(url) if !url.is_empty() => Self::send_http_notification(
                        client,
                        url,
                        t.http_token.as_deref(),
                        t.http_method.as_deref(),
                        &t.podcast_name,
                        episode_title,
                    )
                    .await
                    .unwrap_or(false),
                    _ => false,
                },
                other => {
                    tracing::warn!("Unknown notification platform: {}", other);
                    false
                }
            };
            success |= sent;
        }
        success
    }

    // Helper function to send NTFY notification - matches Python send_ntfy_notification function
    pub(super) async fn send_ntfy_notification(
        client: &reqwest::Client,
        topic: &str,
        server_url: &str,
        username: Option<&str>,
        password: Option<&str>,
        access_token: Option<&str>,
        podcast_name: &str,
        episode_title: &str,
    ) -> AppResult<bool> {
        let url = format!("{}/{}", server_url.trim_end_matches('/'), topic);
        let message = format!("New episode available for {}: {}", podcast_name, episode_title);
        
        let mut request = client
            .post(&url)
            .timeout(std::time::Duration::from_secs(2))
            .header("Content-Type", "text/plain")
            .body(message);
        
        // Add authentication if provided
        if let Some(token) = access_token.filter(|t| !t.is_empty()) {
            // Use access token (preferred method)
            request = request.header("Authorization", format!("Bearer {}", token));
        } else if let (Some(user), Some(pass)) = (username.filter(|u| !u.is_empty()), password.filter(|p| !p.is_empty())) {
            // Use username/password basic auth
            request = request.basic_auth(user, Some(pass));
        }
        
        match request.send().await
        {
            Ok(response) => {
                if response.status().is_success() {
                    tracing::debug!("Sent NTFY notification to {}", url);
                    Ok(true)
                } else {
                    tracing::warn!("NTFY notification failed with status: {}", response.status());
                    Ok(false)
                }
            }
            Err(e) => {
                tracing::warn!("Failed to send NTFY notification: {}", e);
                Ok(false)
            }
        }
    }

    // Helper function to send Gotify notification - matches Python send_gotify_notification function
    pub(super) async fn send_gotify_notification(
        client: &reqwest::Client,
        server_url: &str,
        token: &str,
        podcast_name: &str,
        episode_title: &str,
    ) -> AppResult<bool> {
        let url = format!("{}/message?token={}", server_url.trim_end_matches('/'), token);
        let message = format!("New episode available for {}: {}", podcast_name, episode_title);
        
        match client
            .post(&url)
            .timeout(std::time::Duration::from_secs(2))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!({
                "message": message,
                "title": "New Podcast Episode"
            }))
            .send()
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    // The URL carries the app token; don't log it.
                    tracing::debug!("Sent Gotify notification to {}", server_url);
                    Ok(true)
                } else {
                    tracing::warn!("Gotify notification failed with status: {}", response.status());
                    Ok(false)
                }
            }
            Err(e) => {
                tracing::warn!("Failed to send Gotify notification: {}", e);
                Ok(false)
            }
        }
    }

    /// Send a new-episode notification to a generic HTTP endpoint, in the same shape as the test
    /// notification: GET adds `?message=`, POST sends JSON (Telegram's `chat_id`/`text` when the
    /// URL is the Telegram Bot API). A token is sent as a Bearer header except for Telegram.
    pub(super) async fn send_http_notification(
        client: &reqwest::Client,
        url: &str,
        token: Option<&str>,
        method: Option<&str>,
        podcast_name: &str,
        episode_title: &str,
    ) -> AppResult<bool> {
        let message = format!("New episode available for {}: {}", podcast_name, episode_title);
        let token = token.filter(|t| !t.is_empty());
        let is_telegram = url.contains("api.telegram.org");

        let mut request = if method.unwrap_or("POST").eq_ignore_ascii_case("GET") {
            let sep = if url.contains('?') { '&' } else { '?' };
            client.get(format!("{}{}message={}", url, sep, urlencoding::encode(&message)))
        } else if is_telegram {
            let chat_id = token.and_then(telegram_chat_id).unwrap_or("");
            client.post(url).json(&serde_json::json!({ "chat_id": chat_id, "text": message }))
        } else {
            client.post(url).json(&serde_json::json!({
                "title": "New Podcast Episode",
                "message": message,
                "text": message
            }))
        };
        request = request.timeout(std::time::Duration::from_secs(2));
        if let Some(token) = token.filter(|_| !is_telegram) {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        match request.send().await {
            Ok(response) if response.status().is_success() => {
                // The URL may carry a secret; don't log it.
                tracing::debug!("Sent HTTP notification");
                Ok(true)
            }
            Ok(response) => {
                tracing::warn!("HTTP notification failed with status: {}", response.status());
                Ok(false)
            }
            Err(e) => {
                tracing::warn!("Failed to send HTTP notification: {}", e);
                Ok(false)
            }
        }
    }

    // // Get podcast details - matches Python get_podcast_details function exactly
    // pub async fn get_podcast_details(&self, user_id: i32, podcast_id: i32) -> AppResult<Option<serde_json::Value>> {
    //     info!("Getting podcast details for podcast {} and user {}", podcast_id, user_id);
        
    //     let details = match self {
    //         DatabasePool::Postgres(pool) => {
    //             // Try to get podcast for user first, then fall back to UserID=1
    //             let mut row = sqlx::query(r#"SELECT * FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
    //                 .bind(podcast_id)
    //                 .bind(user_id)
    //                 .fetch_optional(pool)
    //                 .await?;
                
    //             if row.is_none() {
    //                 row = sqlx::query(r#"SELECT * FROM "Podcasts" WHERE podcastid = $1 AND userid = 1"#)
    //                     .bind(podcast_id)
    //                     .fetch_optional(pool)
    //                     .await?;
    //             }
                
    //             if let Some(row) = row {
    //                 // Get episode count from YouTubeVideos table if this is a YouTube channel
    //                 let mut episode_count = row.try_get::<i32, _>("episodecount")?;
    //                 let is_youtube_channel = row.try_get::<bool, _>("isyoutubechannel").unwrap_or(false);
                    
    //                 if is_youtube_channel {
    //                     let count_result = sqlx::query(r#"SELECT COUNT(*) as count FROM "YouTubeVideos" WHERE podcastid = $1"#)
    //                         .bind(podcast_id)
    //                         .fetch_one(pool)
    //                         .await?;
    //                     episode_count = count_result.try_get::<i64, _>("count")? as i32;
    //                 }

    //                 Some(serde_json::json!({
    //                     "podcastid": row.try_get::<i32, _>("podcastid")?,
    //                     "podcastindexid": row.try_get::<Option<i32>, _>("podcastindexid")?,
    //                     "podcastname": row.try_get::<String, _>("podcastname")?,
    //                     "artworkurl": row.try_get::<Option<String>, _>("artworkurl").unwrap_or_default().unwrap_or_default(),
    //                     "author": row.try_get::<String, _>("author")?,
    //                     "categories": row.try_get::<Option<String>, _>("categories")?,
    //                     "description": row.try_get::<Option<String>, _>("description")?.unwrap_or_default(),
    //                     "episodecount": episode_count,
    //                     "feedurl": row.try_get::<String, _>("feedurl")?,
    //                     "websiteurl": row.try_get::<String, _>("websiteurl")?,
    //                     "explicit": row.try_get::<bool, _>("explicit")?,
    //                     "userid": row.try_get::<i32, _>("userid")?,
    //                     "autodownload": row.try_get::<bool, _>("autodownload").unwrap_or(false),
    //                     "startskip": row.try_get::<i32, _>("startskip").unwrap_or(0),
    //                     "endskip": row.try_get::<i32, _>("endskip").unwrap_or(0),
    //                     "username": row.try_get::<Option<String>, _>("username")?,
    //                     "password": row.try_get::<Option<String>, _>("password")?,
    //                     "isyoutubechannel": is_youtube_channel,
    //                     "notificationsenabled": row.try_get::<bool, _>("notificationsenabled").unwrap_or(false),
    //                     "feedcutoffdays": row.try_get::<i32, _>("feedcutoffdays").unwrap_or(0),
    //                 }))
    //             } else {
    //                 None
    //             }
    //         }
    //         DatabasePool::MySQL(pool) => {
    //             // Try to get podcast for user first, then fall back to UserID=1
    //             let mut row = sqlx::query("SELECT * FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
    //                 .bind(podcast_id)
    //                 .bind(user_id)
    //                 .fetch_optional(pool)
    //                 .await?;
                
    //             if row.is_none() {
    //                 row = sqlx::query("SELECT * FROM Podcasts WHERE PodcastID = ? AND UserID = 1")
    //                     .bind(podcast_id)
    //                     .fetch_optional(pool)
    //                     .await?;
    //             }
                
    //             if let Some(row) = row {
    //                 // Get episode count from YouTubeVideos table if this is a YouTube channel
    //                 let mut episode_count = row.try_get::<i32, _>("EpisodeCount")?;
    //                 let is_youtube_channel = row.try_get::<i8, _>("IsYouTubeChannel").unwrap_or(0) != 0;
                    
    //                 if is_youtube_channel {
    //                     let count_result = sqlx::query("SELECT COUNT(*) as count FROM YouTubeVideos WHERE PodcastID = ?")
    //                         .bind(podcast_id)
    //                         .fetch_one(pool)
    //                         .await?;
    //                     episode_count = count_result.try_get::<i64, _>("count")? as i32;
    //                 }

    //                 Some(serde_json::json!({
    //                     "podcastid": row.try_get::<i32, _>("PodcastID")?,
    //                     "podcastindexid": row.try_get::<Option<i32>, _>("PodcastIndexID")?,
    //                     "podcastname": row.try_get::<String, _>("PodcastName")?,
    //                     "artworkurl": row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default().unwrap_or_default(),
    //                     "author": row.try_get::<String, _>("Author")?,
    //                     "categories": row.try_get::<Option<String>, _>("Categories")?,
    //                     "description": row.try_get::<String, _>("Description")?,
    //                     "episodecount": episode_count,
    //                     "feedurl": row.try_get::<String, _>("FeedURL")?,
    //                     "websiteurl": row.try_get::<String, _>("WebsiteURL")?,
    //                     "explicit": row.try_get::<i8, _>("Explicit").unwrap_or(0) != 0,
    //                     "userid": row.try_get::<i32, _>("UserID")?,
    //                     "autodownload": row.try_get::<i8, _>("AutoDownload").unwrap_or(0) != 0,
    //                     "startskip": row.try_get::<i32, _>("StartSkip").unwrap_or(0),
    //                     "endskip": row.try_get::<i32, _>("EndSkip").unwrap_or(0),
    //                     "username": row.try_get::<Option<String>, _>("Username")?,
    //                     "password": row.try_get::<Option<String>, _>("Password")?,
    //                     "isyoutubechannel": is_youtube_channel,
    //                     "notificationsenabled": row.try_get::<i8, _>("NotificationsEnabled").unwrap_or(0) != 0,
    //                     "feedcutoffdays": row.try_get::<i32, _>("FeedCutoffDays").unwrap_or(0),
    //                 }))
    //             } else {
    //                 None
    //             }
    //         }
    //     };
        
    //     if let Some(ref result) = details {
    //         debug!("Found podcast details for: {}", result["podcast_name"]);
    //     } else {
    //         debug!("No podcast found with ID {} for user {}", podcast_id, user_id);
    //     }
        
    //     Ok(details)
    // }
    
    // Get notification settings - matches Python get_notification_settings function exactly
    pub async fn get_notification_settings(&self, user_id: i32) -> AppResult<Vec<serde_json::Value>> {
        info!("Getting notification settings for user {}", user_id);
        
        let settings = dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(r#"
                    SELECT platform, enabled, ntfytopic, ntfyserverurl, ntfyusername, ntfypassword, ntfyaccesstoken, gotifyurl, gotifytoken, httpurl, httptoken, httpmethod
                    FROM "UserNotificationSettings" 
                    WHERE userid = $1
                "#)
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?;
                
                let mut settings = Vec::new();
                for row in rows {
                    let setting = serde_json::json!({
                        "platform": row.try_get::<String, _>("platform")?,
                        "enabled": row.try_get::<bool, _>("enabled")?,
                        "ntfy_topic": row.try_get::<Option<String>, _>("ntfytopic")?,
                        "ntfy_server_url": row.try_get::<Option<String>, _>("ntfyserverurl")?,
                        "ntfy_username": row.try_get::<Option<String>, _>("ntfyusername")?,
                        "ntfy_password": row.try_get::<Option<String>, _>("ntfypassword")?,
                        "ntfy_access_token": row.try_get::<Option<String>, _>("ntfyaccesstoken")?,
                        "gotify_url": row.try_get::<Option<String>, _>("gotifyurl")?,
                        "gotify_token": row.try_get::<Option<String>, _>("gotifytoken")?,
                        "http_url": row.try_get::<Option<String>, _>("httpurl")?,
                        "http_token": row.try_get::<Option<String>, _>("httptoken")?,
                        "http_method": row.try_get::<Option<String>, _>("httpmethod")?
                    });
                    settings.push(setting);
                }
                settings
            },
            mysql(pool) => {
                let rows = sqlx::query("
                    SELECT Platform, Enabled, NtfyTopic, NtfyServerURL, NtfyUsername, NtfyPassword, NtfyAccessToken, GotifyURL, GotifyToken, HttpUrl, HttpToken, HttpMethod
                    FROM UserNotificationSettings 
                    WHERE UserID = ?
                    ORDER BY Platform
                ")
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?;
                
                let mut settings = Vec::new();
                for row in rows {
                    let setting = serde_json::json!({
                        "platform": row.try_get::<String, _>("Platform")?,
                        "enabled": row.try_get::<bool, _>("Enabled")?,
                        "ntfy_topic": row.try_get::<Option<String>, _>("NtfyTopic")?,
                        "ntfy_server_url": row.try_get::<Option<String>, _>("NtfyServerURL")?,
                        "ntfy_username": row.try_get::<Option<String>, _>("NtfyUsername")?,
                        "ntfy_password": row.try_get::<Option<String>, _>("NtfyPassword")?,
                        "ntfy_access_token": row.try_get::<Option<String>, _>("NtfyAccessToken")?,
                        "gotify_url": row.try_get::<Option<String>, _>("GotifyURL")?,
                        "gotify_token": row.try_get::<Option<String>, _>("GotifyToken")?,
                        "http_url": row.try_get::<Option<String>, _>("HttpUrl")?,
                        "http_token": row.try_get::<Option<String>, _>("HttpToken")?,
                        "http_method": row.try_get::<Option<String>, _>("HttpMethod")?
                    });
                    settings.push(setting);
                }
                settings
            },
        });
        
        debug!("Found {} notification settings for user {}", settings.len(), user_id);
        Ok(settings)
    }

    // Update notification settings - matches Python update_notification_settings function exactly
    pub async fn update_notification_settings(&self, user_id: i32, platform: &str, enabled: bool, ntfy_topic: Option<&str>, ntfy_server_url: Option<&str>, ntfy_username: Option<&str>, ntfy_password: Option<&str>, ntfy_access_token: Option<&str>, gotify_url: Option<&str>, gotify_token: Option<&str>, http_url: Option<&str>, http_token: Option<&str>, http_method: Option<&str>) -> AppResult<bool> {
        info!("Updating notification settings for user {} platform {}", user_id, platform);
        
        // Check if settings exist for this user/platform combination and perform update/insert
        let success = dialect_match!(self, {
            pg(pool) => {
                let existing = q::query(r#"SELECT COUNT(*) as count FROM "UserNotificationSettings" WHERE userid = $1 AND platform = $2"#)
                    .bind(user_id)
                    .bind(platform)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = existing.try_get("count")?;
                
                if count > 0 {
                    // Update existing record
                    let result = q::query(r#"
                        UPDATE "UserNotificationSettings" 
                        SET enabled = $3, ntfytopic = $4, ntfyserverurl = $5, ntfyusername = $6, ntfypassword = $7, ntfyaccesstoken = $8, gotifyurl = $9, gotifytoken = $10, httpurl = $11, httptoken = $12, httpmethod = $13
                        WHERE userid = $1 AND platform = $2
                    "#)
                        .bind(user_id)
                        .bind(platform)
                        .bind(enabled)
                        .bind(ntfy_topic)
                        .bind(ntfy_server_url)
                        .bind(ntfy_username)
                        .bind(ntfy_password)
                        .bind(ntfy_access_token)
                        .bind(gotify_url)
                        .bind(gotify_token)
                        .bind(http_url)
                        .bind(http_token)
                        .bind(http_method)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                } else {
                    // Insert new record
                    let result = q::query(r#"
                        INSERT INTO "UserNotificationSettings" 
                        (userid, platform, enabled, ntfytopic, ntfyserverurl, ntfyusername, ntfypassword, ntfyaccesstoken, gotifyurl, gotifytoken, httpurl, httptoken, httpmethod)
                        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                    "#)
                        .bind(user_id)
                        .bind(platform)
                        .bind(enabled)
                        .bind(ntfy_topic)
                        .bind(ntfy_server_url)
                        .bind(ntfy_username)
                        .bind(ntfy_password)
                        .bind(ntfy_access_token)
                        .bind(gotify_url)
                        .bind(gotify_token)
                        .bind(http_url)
                        .bind(http_token)
                        .bind(http_method)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                }
            },
            mysql(pool) => {
                let existing = sqlx::query("SELECT COUNT(*) as count FROM UserNotificationSettings WHERE UserID = ? AND Platform = ?")
                    .bind(user_id)
                    .bind(platform)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = existing.try_get("count")?;
                
                if count > 0 {
                    // Update existing record
                    let result = sqlx::query("
                        UPDATE UserNotificationSettings 
                        SET Enabled = ?, NtfyTopic = ?, NtfyServerURL = ?, NtfyUsername = ?, NtfyPassword = ?, NtfyAccessToken = ?, GotifyURL = ?, GotifyToken = ?, HttpUrl = ?, HttpToken = ?, HttpMethod = ?
                        WHERE UserID = ? AND Platform = ?
                    ")
                        .bind(enabled)
                        .bind(ntfy_topic)
                        .bind(ntfy_server_url)
                        .bind(ntfy_username)
                        .bind(ntfy_password)
                        .bind(ntfy_access_token)
                        .bind(gotify_url)
                        .bind(gotify_token)
                        .bind(http_url)
                        .bind(http_token)
                        .bind(http_method)
                        .bind(user_id)
                        .bind(platform)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                } else {
                    // Insert new record
                    let result = sqlx::query("
                        INSERT INTO UserNotificationSettings 
                        (UserID, Platform, Enabled, NtfyTopic, NtfyServerURL, NtfyUsername, NtfyPassword, NtfyAccessToken, GotifyURL, GotifyToken, HttpUrl, HttpToken, HttpMethod)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ")
                        .bind(user_id)
                        .bind(platform)
                        .bind(enabled)
                        .bind(ntfy_topic)
                        .bind(ntfy_server_url)
                        .bind(ntfy_username)
                        .bind(ntfy_password)
                        .bind(ntfy_access_token)
                        .bind(gotify_url)
                        .bind(gotify_token)
                        .bind(http_url)
                        .bind(http_token)
                        .bind(http_method)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                }
            },
        });
        
        debug!("Successfully updated notification settings for user {} platform {}: {}", user_id, platform, success);
        Ok(success)
    }
}

#[cfg(test)]
mod telegram_token_tests {
    use super::telegram_chat_id;

    #[test]
    fn the_chat_id_follows_the_bot_tokens_own_colon() {
        assert_eq!(telegram_chat_id("123456:ABC-def:987654"), Some("987654"));
        assert_eq!(telegram_chat_id("123456:ABC-def:-100123"), Some("-100123"));
        assert_eq!(telegram_chat_id("123456:ABC-def"), None, "a bare bot token has no chat id");
        assert_eq!(telegram_chat_id("123456:ABC-def:"), None);
    }
}
