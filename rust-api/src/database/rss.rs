//! Pinepods-generated RSS feeds and RSS keys.

use super::*;

#[derive(Debug)]
pub(super) struct RssEpisode {
    title: String,
    description: String,
    url: String,
    pub_date: String,
    duration: Option<i32>,
    author: Option<String>,
    artwork_url: Option<String>,
}

impl DatabasePool {
    // Toggle RSS feeds - matches Python toggle_rss_feeds function exactly
    pub async fn toggle_rss_feeds(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                // Get current status
                let current_status: Option<bool> = q::query_scalar(r#"SELECT enablerssfeeds FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                let new_status = !current_status.unwrap_or(false);

                // Update status
                q::query(r#"UPDATE "Users" SET enablerssfeeds = $1 WHERE userid = $2"#)
                    .bind(new_status)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                Ok(new_status)
            },
            mysql(pool) => {
                // Get current status
                let current_status: Option<i32> = sqlx::query_scalar("SELECT EnableRSSFeeds FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                let new_status = current_status.unwrap_or(0) == 0;

                // Update status
                sqlx::query("UPDATE Users SET EnableRSSFeeds = ? WHERE UserID = ?")
                    .bind(new_status as i32)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                Ok(new_status)
            },
        })
    }

    // Create RSS key - matches Python create_rss_key function exactly
    pub async fn create_rss_key(&self, user_id: i32, podcast_ids: Option<Vec<i32>>) -> AppResult<String> {
        use rand::RngExt;
        use rand::distr::Alphanumeric;
        
        // Generate 64-character RSS key
        let rss_key: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();

        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                let key_id: i32 = q::query_scalar(r#"INSERT INTO "RssKeys" (userid, rsskey) VALUES ($1, $2) RETURNING rsskeyid"#)
                    .bind(user_id)
                    .bind(&rss_key)
                    .fetch_one(&mut *tx)
                    .await?;

                // Handle podcast IDs if provided
                if let Some(podcast_ids) = podcast_ids {
                    for podcast_id in podcast_ids {
                        q::query(r#"INSERT INTO "RssKeyMap" (rsskeyid, podcastid) VALUES ($1, $2)"#)
                            .bind(key_id)
                            .bind(podcast_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                }
                tx.commit().await?;
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;
                let result = sqlx::query("INSERT INTO RssKeys (UserID, RssKey) VALUES (?, ?)")
                    .bind(user_id)
                    .bind(&rss_key)
                    .execute(&mut *tx)
                    .await?;

                let key_id = result.last_insert_id() as i32;

                // Handle podcast IDs if provided
                if let Some(podcast_ids) = podcast_ids {
                    for podcast_id in podcast_ids {
                        sqlx::query("INSERT INTO RssKeyMap (RssKeyID, PodcastID) VALUES (?, ?)")
                            .bind(key_id)
                            .bind(podcast_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                }
                tx.commit().await?;
            },
        });

        Ok(rss_key)
    }

    /// The user's RSS key covering only `podcast_id`, created on first use. Single-podcast feed
    /// links use it, so sharing one podcast's feed doesn't expose the user's other feeds.
    pub async fn get_or_create_podcast_rss_key(&self, user_id: i32, podcast_id: i32) -> AppResult<String> {
        let existing: Option<String> = dialect_match!(self, {
            pg(pool) => q::query_scalar(
                r#"SELECT k.rsskey FROM "RssKeys" k
                   WHERE k.userid = $1
                     AND EXISTS (SELECT 1 FROM "RssKeyMap" m WHERE m.rsskeyid = k.rsskeyid AND m.podcastid = $2)
                     AND NOT EXISTS (SELECT 1 FROM "RssKeyMap" m WHERE m.rsskeyid = k.rsskeyid AND m.podcastid <> $2)
                   ORDER BY k.rsskeyid LIMIT 1"#,
            )
            .bind(user_id)
            .bind(podcast_id)
            .fetch_optional(pool)
            .await?,
            mysql(pool) => sqlx::query_scalar(
                "SELECT k.RssKey FROM RssKeys k
                 WHERE k.UserID = ?
                   AND EXISTS (SELECT 1 FROM RssKeyMap m WHERE m.RssKeyID = k.RssKeyID AND m.PodcastID = ?)
                   AND NOT EXISTS (SELECT 1 FROM RssKeyMap m WHERE m.RssKeyID = k.RssKeyID AND m.PodcastID <> ?)
                 ORDER BY k.RssKeyID LIMIT 1",
            )
            .bind(user_id)
            .bind(podcast_id)
            .bind(podcast_id)
            .fetch_optional(pool)
            .await?,
        });
        match existing {
            Some(key) => Ok(key),
            None => self.create_rss_key(user_id, Some(vec![podcast_id])).await,
        }
    }

    // RSS key validation - matches Python get_rss_key_if_valid function
    pub async fn get_rss_key_if_valid(&self, api_key: &str, podcast_ids: Option<&Vec<i32>>) -> AppResult<Option<crate::handlers::feed::RssKeyInfo>> {
        use crate::handlers::feed::RssKeyInfo;
        
        let filter_podcast_ids = podcast_ids.map_or(false, |ids| !ids.is_empty() && !ids.contains(&-1));
        
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"
                    SELECT fk.userid, STRING_AGG(CAST(fkm.podcastid AS TEXT), ',') as podcastids
                    FROM "RssKeys" fk
                    LEFT JOIN "RssKeyMap" fkm ON fk.rsskeyid = fkm.rsskeyid
                    WHERE fk.rsskey = $1
                    GROUP BY fk.userid
                "#)
                    .bind(api_key)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let user_id: i32 = row.try_get("userid")?;
                    let podcast_ids_str: Option<String> = row.try_get("podcastids").ok();
                    
                    let key_podcast_ids = if let Some(ids_str) = podcast_ids_str {
                        ids_str.split(',')
                            .filter_map(|s| s.parse::<i32>().ok())
                            .collect::<Vec<i32>>()
                    } else {
                        vec![-1] // Universal access if no specific podcasts
                    };
                    
                    // Check if access is allowed
                    if filter_podcast_ids {
                        if let Some(requested_ids) = podcast_ids {
                            let has_universal = key_podcast_ids.contains(&-1);
                            let has_specific_access = requested_ids.iter()
                                .all(|id| key_podcast_ids.contains(id));
                            
                            if !has_universal && !has_specific_access {
                                return Ok(None);
                            }
                        }
                    }
                    
                    Ok(Some(RssKeyInfo {
                        podcast_ids: key_podcast_ids,
                        user_id,
                        key: api_key.to_string(),
                    }))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query(r#"
                    SELECT fk.UserID, GROUP_CONCAT(fkm.PodcastID) as podcastids
                    FROM RssKeys fk
                    LEFT JOIN RssKeyMap fkm ON fk.RssKeyID = fkm.RssKeyID
                    WHERE fk.RssKey = ?
                    GROUP BY fk.UserID
                "#)
                    .bind(api_key)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let user_id: i32 = row.try_get("UserID")?;
                    let podcast_ids_str: Option<String> = row.try_get("podcastids").ok();
                    
                    let key_podcast_ids = if let Some(ids_str) = podcast_ids_str {
                        ids_str.split(',')
                            .filter_map(|s| s.parse::<i32>().ok())
                            .collect::<Vec<i32>>()
                    } else {
                        vec![-1] // Universal access if no specific podcasts
                    };
                    
                    // Check if access is allowed
                    if filter_podcast_ids {
                        if let Some(requested_ids) = podcast_ids {
                            let has_universal = key_podcast_ids.contains(&-1);
                            let has_specific_access = requested_ids.iter()
                                .all(|id| key_podcast_ids.contains(id));
                            
                            if !has_universal && !has_specific_access {
                                return Ok(None);
                            }
                        }
                    }
                    
                    Ok(Some(RssKeyInfo {
                        podcast_ids: key_podcast_ids,
                        user_id,
                        key: api_key.to_string(),
                    }))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get RSS feed status - matches Python get_rss_feed_status function
    pub async fn get_rss_feed_status(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT enablerssfeeds FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let enabled: Option<bool> = row.try_get("enablerssfeeds").ok();
                    Ok(enabled.unwrap_or(false))
                } else {
                    Ok(false)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT EnableRSSFeeds FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let enabled: Option<bool> = row.try_get("EnableRSSFeeds").ok();
                    Ok(enabled.unwrap_or(false))
                } else {
                    Ok(false)
                }
            },
        })
    }

    // Generate podcast RSS feed - matches Python generate_podcast_rss function
    pub async fn generate_podcast_rss(
        &self,
        rss_key: crate::handlers::feed::RssKeyInfo,
        limit: i32,
        source_type: Option<&str>,
        domain: &str,
        podcast_ids: Option<&Vec<i32>>,
    ) -> AppResult<String> {
        
        let user_id = rss_key.user_id;
        let mut effective_podcast_ids = rss_key.podcast_ids.clone();
        let key_is_scoped = !rss_key.podcast_ids.contains(&-1);

        // If podcast_id parameter is provided, use it (get_rss_key_if_valid already checked a
        // scoped key covers it); otherwise a scoped key still only sees its own podcasts.
        let explicit_podcast_filter = podcast_ids.is_some();
        if let Some(ids) = podcast_ids {
            if !ids.is_empty() {
                effective_podcast_ids = ids.clone();
            }
        }
        let podcast_filter = explicit_podcast_filter || key_is_scoped;
        
        // Check if RSS feeds are enabled for user
        if !self.get_rss_feed_status(user_id).await? {
            return Err(AppError::forbidden("RSS feeds not enabled for this user"));
        }
        
        // Get user info for feed metadata
        let username = dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT username FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<String, _>("username").unwrap_or_else(|_| "Unknown User".to_string())
                } else {
                    return Err(AppError::not_found("User not found"));
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT Username FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<String, _>("Username").unwrap_or_else(|_| "Unknown User".to_string())
                } else {
                    return Err(AppError::not_found("User not found"));
                }
            },
        });
        
        // Get podcast details for feed metadata - exact Python logic
        let (podcast_name, feed_image, feed_description) = if podcast_filter {
            dialect_match!(self, {
                pg(pool) => {
                    let row = q::query(r#"SELECT podcastname, artworkurl, description FROM "Podcasts" WHERE podcastid = ANY($1) AND userid = $2"#)
                        .bind(&effective_podcast_ids)
                        .bind(user_id)
                        .fetch_optional(pool)
                        .await?;
                    
                    if let Some(row) = row {
                        (
                            row.try_get::<String, _>("podcastname").unwrap_or_else(|_| "Unknown Podcast".to_string()),
                            row.try_get::<Option<String>, _>("artworkurl").unwrap_or_default().unwrap_or_else(|| format!("{}/static/assets/favicon.png", domain)),
                            row.try_get::<String, _>("description").unwrap_or_else(|_| "No description available".to_string()),
                        )
                    } else {
                        ("Unknown Podcast".to_string(), format!("{}/static/assets/favicon.png", domain), "No description available".to_string())
                    }
                },
                mysql(pool) => {
                    if effective_podcast_ids.len() == 1 {
                        let row = sqlx::query("SELECT PodcastName, ArtworkURL, Description FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                            .bind(effective_podcast_ids[0])
                            .bind(user_id)
                            .fetch_optional(pool)
                            .await?;
                        
                        if let Some(row) = row {
                            (
                                row.try_get::<String, _>("PodcastName").unwrap_or_else(|_| "Unknown Podcast".to_string()),
                                row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default().unwrap_or_else(|| format!("{}/static/assets/favicon.png", domain)),
                                row.try_get::<String, _>("Description").unwrap_or_else(|_| "No description available".to_string()),
                            )
                        } else {
                            ("Unknown Podcast".to_string(), format!("{}/static/assets/favicon.png", domain), "No description available".to_string())
                        }
                    } else {
                        let placeholders = vec!["?"; effective_podcast_ids.len()].join(",");
                        let query_str = format!("SELECT PodcastName, ArtworkURL, Description FROM Podcasts WHERE PodcastID IN ({}) AND UserID = ?", placeholders);
                        let mut query = sqlx::query(sqlx::AssertSqlSafe(query_str.as_str()));
                        for &id in &effective_podcast_ids {
                            query = query.bind(id);
                        }
                        query = query.bind(user_id);
                        let row = query.fetch_optional(pool).await?;
                        
                        if let Some(row) = row {
                            (
                                row.try_get::<String, _>("PodcastName").unwrap_or_else(|_| "Unknown Podcast".to_string()),
                                row.try_get::<Option<String>, _>("ArtworkURL").unwrap_or_default().unwrap_or_else(|| format!("{}/static/assets/favicon.png", domain)),
                                row.try_get::<String, _>("Description").unwrap_or_else(|_| "No description available".to_string()),
                            )
                        } else {
                            ("Unknown Podcast".to_string(), format!("{}/static/assets/favicon.png", domain), "No description available".to_string())
                        }
                    }
                },
            })
        } else {
            ("All Podcasts".to_string(), format!("{}/static/assets/favicon.png", domain), "RSS feed for all podcasts from Pinepods".to_string())
        };

        // Stream URLs carry the user's RSS key, except for a scoped key: handing out the
        // user's universal key there would undo the scope.
        let user_rss_key = if key_is_scoped {
            rss_key.key.clone()
        } else {
            self.get_or_create_user_rss_key(user_id).await?
        };

        // Get episodes
        let episodes = self.get_rss_episodes(user_id, limit, source_type, &effective_podcast_ids, podcast_filter, domain, &user_rss_key).await?;

        Self::write_rss_document(
            &format!("Pinepods - {}", podcast_name),
            &feed_description,
            &username,
            &feed_image,
            &episodes,
        )
    }

    /// Serialize a channel + its episodes into an RSS 2.0 (iTunes) XML document.
    /// Shared by the subscriptions feed and the per-collection feeds.
    pub(super) fn write_rss_document(
        title: &str,
        description: &str,
        author: &str,
        image: &str,
        episodes: &[RssEpisode],
    ) -> AppResult<String> {
        use quick_xml::events::{Event, BytesStart, BytesEnd, BytesText, BytesCData};
        use quick_xml::Writer;
        use std::io::Cursor;

        let mut writer = Writer::new(Cursor::new(Vec::new()));

        // XML declaration
        writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None)))?;

        // RSS root element with namespace
        let mut rss_elem = BytesStart::new("rss");
        rss_elem.push_attribute(("version", "2.0"));
        rss_elem.push_attribute(("xmlns:itunes", "http://www.itunes.com/dtds/podcast-1.0.dtd"));
        writer.write_event(Event::Start(rss_elem))?;

        // Channel
        writer.write_event(Event::Start(BytesStart::new("channel")))?;

        // Channel metadata
        writer.write_event(Event::Start(BytesStart::new("title")))?;
        writer.write_event(Event::Text(BytesText::new(title)))?;
        writer.write_event(Event::End(BytesEnd::new("title")))?;

        writer.write_event(Event::Start(BytesStart::new("link")))?;
        writer.write_event(Event::Text(BytesText::new("https://github.com/madeofpendletonwool/pinepods")))?;
        writer.write_event(Event::End(BytesEnd::new("link")))?;

        writer.write_event(Event::Start(BytesStart::new("description")))?;
        writer.write_event(Event::Text(BytesText::new(description)))?;
        writer.write_event(Event::End(BytesEnd::new("description")))?;

        writer.write_event(Event::Start(BytesStart::new("language")))?;
        writer.write_event(Event::Text(BytesText::new("en")))?;
        writer.write_event(Event::End(BytesEnd::new("language")))?;

        writer.write_event(Event::Start(BytesStart::new("itunes:author")))?;
        writer.write_event(Event::Text(BytesText::new(author)))?;
        writer.write_event(Event::End(BytesEnd::new("itunes:author")))?;

        // iTunes image
        let mut itunes_image = BytesStart::new("itunes:image");
        itunes_image.push_attribute(("href", image));
        writer.write_event(Event::Empty(itunes_image))?;

        // RSS image block
        writer.write_event(Event::Start(BytesStart::new("image")))?;

        writer.write_event(Event::Start(BytesStart::new("url")))?;
        writer.write_event(Event::Text(BytesText::new(image)))?;
        writer.write_event(Event::End(BytesEnd::new("url")))?;

        writer.write_event(Event::Start(BytesStart::new("title")))?;
        writer.write_event(Event::Text(BytesText::new(title)))?;
        writer.write_event(Event::End(BytesEnd::new("title")))?;

        writer.write_event(Event::Start(BytesStart::new("link")))?;
        writer.write_event(Event::Text(BytesText::new("https://github.com/madeofpendletonwool/pinepods")))?;
        writer.write_event(Event::End(BytesEnd::new("link")))?;

        writer.write_event(Event::End(BytesEnd::new("image")))?;

        // TTL
        writer.write_event(Event::Start(BytesStart::new("ttl")))?;
        writer.write_event(Event::Text(BytesText::new("60")))?;
        writer.write_event(Event::End(BytesEnd::new("ttl")))?;

        // Write episodes
        for episode in episodes {
            writer.write_event(Event::Start(BytesStart::new("item")))?;

            // Title (using CDATA for safety)
            writer.write_event(Event::Start(BytesStart::new("title")))?;
            writer.write_event(Event::CData(BytesCData::new(&episode.title)))?;
            writer.write_event(Event::End(BytesEnd::new("title")))?;

            // Link (URL will be properly escaped)
            writer.write_event(Event::Start(BytesStart::new("link")))?;
            writer.write_event(Event::Text(BytesText::new(&episode.url)))?;
            writer.write_event(Event::End(BytesEnd::new("link")))?;

            // Description (using CDATA for safety)
            writer.write_event(Event::Start(BytesStart::new("description")))?;
            writer.write_event(Event::CData(BytesCData::new(&episode.description)))?;
            writer.write_event(Event::End(BytesEnd::new("description")))?;

            // GUID
            writer.write_event(Event::Start(BytesStart::new("guid")))?;
            writer.write_event(Event::Text(BytesText::new(&episode.url)))?;
            writer.write_event(Event::End(BytesEnd::new("guid")))?;

            // Pub date
            writer.write_event(Event::Start(BytesStart::new("pubDate")))?;
            writer.write_event(Event::Text(BytesText::new(&episode.pub_date)))?;
            writer.write_event(Event::End(BytesEnd::new("pubDate")))?;

            // Author (if present)
            if let Some(ref author) = episode.author {
                writer.write_event(Event::Start(BytesStart::new("itunes:author")))?;
                writer.write_event(Event::Text(BytesText::new(author)))?;
                writer.write_event(Event::End(BytesEnd::new("itunes:author")))?;
            }

            // Artwork (if present)
            if let Some(ref artwork_url) = episode.artwork_url {
                let mut itunes_img = BytesStart::new("itunes:image");
                itunes_img.push_attribute(("href", artwork_url.as_str()));
                writer.write_event(Event::Empty(itunes_img))?;
            }

            // Duration (iTunes format: HH:MM:SS or MM:SS)
            if let Some(duration_seconds) = episode.duration {
                let hours = duration_seconds / 3600;
                let minutes = (duration_seconds % 3600) / 60;
                let seconds = duration_seconds % 60;

                let duration_str = if hours > 0 {
                    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
                } else {
                    format!("{:02}:{:02}", minutes, seconds)
                };

                writer.write_event(Event::Start(BytesStart::new("itunes:duration")))?;
                writer.write_event(Event::Text(BytesText::new(&duration_str)))?;
                writer.write_event(Event::End(BytesEnd::new("itunes:duration")))?;
            }

            // Enclosure (length should be file size in bytes, using duration as placeholder)
            let mut enclosure = BytesStart::new("enclosure");
            enclosure.push_attribute(("url", episode.url.as_str()));
            enclosure.push_attribute(("length", episode.duration.unwrap_or(0).to_string().as_str()));
            enclosure.push_attribute(("type", "audio/mpeg"));
            writer.write_event(Event::Empty(enclosure))?;

            writer.write_event(Event::End(BytesEnd::new("item")))?;
        }

        // Close channel and RSS
        writer.write_event(Event::End(BytesEnd::new("channel")))?;
        writer.write_event(Event::End(BytesEnd::new("rss")))?;

        // Convert to string
        let result = writer.into_inner().into_inner();
        let rss_content = String::from_utf8(result)
            .map_err(|e| AppError::internal(&format!("Failed to convert RSS to UTF-8: {}", e)))?;

        Ok(rss_content)
    }

    /// Format a stored ISO timestamp (`%Y-%m-%dT%H:%M:%S`) as an RFC-822 pubDate.
    pub(super) fn rss_pub_date(iso: &str) -> String {
        use chrono::{DateTime, Utc, NaiveDateTime};
        match NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%S") {
            Ok(naive) => DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
                .format("%a, %d %b %Y %H:%M:%S %z")
                .to_string(),
            Err(_) => Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string(),
        }
    }

    /// Build a single RssEpisode from normalized collection-episode fields,
    /// applying the same downloaded->stream-URL rewrite the subscriptions feed uses.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn build_rss_episode(
        domain: &str,
        user_rss_key: &str,
        user_id: i32,
        episodeid: i32,
        title: String,
        description: String,
        episodeurl: String,
        episodepubdate: &str,
        episodeduration: i32,
        episodeartwork: Option<String>,
        author: Option<String>,
        downloaded: bool,
        is_youtube: bool,
    ) -> RssEpisode {
        let url = if downloaded {
            let yt = if is_youtube { "&type=youtube" } else { "" };
            format!(
                "{}/api/data/stream/{}?api_key={}{}&user_id={}",
                domain, episodeid, user_rss_key, yt, user_id
            )
        } else {
            episodeurl
        };

        RssEpisode {
            title,
            description,
            url,
            pub_date: Self::rss_pub_date(episodepubdate),
            duration: Some(episodeduration),
            author,
            artwork_url: episodeartwork.filter(|s| !s.is_empty()),
        }
    }

    /// Map a JSON collection-episode row (playlist / history shape) into an RssEpisode.
    pub(super) fn build_rss_episode_from_json(
        domain: &str,
        user_rss_key: &str,
        user_id: i32,
        v: &serde_json::Value,
    ) -> RssEpisode {
        let episodeid = v.get("episodeid").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
        let title = v.get("episodetitle").and_then(|x| x.as_str()).unwrap_or("Untitled Episode").to_string();
        let description = v.get("episodedescription").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let episodeurl = v.get("episodeurl").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let pubdate = v.get("episodepubdate").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let duration = v.get("episodeduration").and_then(|x| x.as_i64()).unwrap_or(0) as i32;
        let artwork = v.get("episodeartwork").and_then(|x| x.as_str()).map(|s| s.to_string());
        let podcastname = v.get("podcastname").and_then(|x| x.as_str()).map(|s| s.to_string());
        let downloaded = v.get("downloaded").and_then(|x| x.as_bool()).unwrap_or(false);
        let is_youtube = v.get("is_youtube").and_then(|x| x.as_bool()).unwrap_or(false);

        Self::build_rss_episode(
            domain, user_rss_key, user_id, episodeid, title, description, episodeurl,
            &pubdate, duration, artwork, podcastname, downloaded, is_youtube,
        )
    }

    /// Generate an RSS feed for one of a user's episode collections
    /// (saved, queue, playlist, collection, downloads, history).
    pub async fn generate_collection_rss(
        &self,
        user_id: i32,
        source: crate::handlers::feed::FeedSource,
        limit: i32,
        domain: &str,
    ) -> AppResult<String> {
        use crate::handlers::feed::FeedSource;

        // Check if RSS feeds are enabled for user
        if !self.get_rss_feed_status(user_id).await? {
            return Err(AppError::forbidden("RSS feeds not enabled for this user"));
        }

        // Cap for the collection fetchers that take an explicit limit; a non-positive
        // limit means "no limit" (mirrors the subscriptions feed behavior).
        let lim: i64 = if limit > 0 { limit as i64 } else { 100_000 };
        let take_n: usize = if limit > 0 { limit as usize } else { usize::MAX };

        // Username for channel <itunes:author>
        let username = dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT username FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                match row {
                    Some(row) => row.try_get::<String, _>("username").unwrap_or_else(|_| "Unknown User".to_string()),
                    None => return Err(AppError::not_found("User not found")),
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT Username FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                match row {
                    Some(row) => row.try_get::<String, _>("Username").unwrap_or_else(|_| "Unknown User".to_string()),
                    None => return Err(AppError::not_found("User not found")),
                }
            },
        });

        // User RSS key for authenticated stream URLs on downloaded episodes
        let user_rss_key = self.get_or_create_user_rss_key(user_id).await?;
        let default_image = format!("{}/static/assets/favicon.png", domain);

        // Resolve the feed title/description and the episode list per source.
        let (feed_name, feed_description, episodes): (String, String, Vec<RssEpisode>) = match source {
            FeedSource::Subscriptions => {
                // Handled by generate_podcast_rss; guard just in case.
                return Err(AppError::internal("Subscriptions feed must use generate_podcast_rss"));
            }
            FeedSource::Saved => {
                let (eps, _) = self.get_saved_episodes(user_id, lim, 0, "date", "desc", "").await?;
                let items = eps.into_iter().map(|e| Self::build_rss_episode(
                    domain, &user_rss_key, user_id, e.episodeid, e.episodetitle, e.episodedescription,
                    e.episodeurl, &e.episodepubdate, e.episodeduration, Some(e.episodeartwork),
                    Some(e.podcastname), e.downloaded, e.is_youtube,
                )).collect();
                ("Saved Episodes".to_string(), "Your saved episodes from Pinepods".to_string(), items)
            }
            FeedSource::Queue => {
                let eps = self.get_queued_episodes(user_id).await?;
                let items = eps.into_iter().take(take_n).map(|e| Self::build_rss_episode(
                    domain, &user_rss_key, user_id, e.episodeid, e.episodetitle, e.episodedescription,
                    e.episodeurl, &e.episodepubdate, e.episodeduration, Some(e.episodeartwork),
                    Some(e.podcastname), e.downloaded, e.is_youtube,
                )).collect();
                ("Queue".to_string(), "Your episode queue from Pinepods".to_string(), items)
            }
            FeedSource::Downloads => {
                let eps = self.download_episode_list(user_id).await?;
                let items = eps.into_iter().take(take_n).map(|e| Self::build_rss_episode(
                    domain, &user_rss_key, user_id, e.episodeid, e.episodetitle, e.episodedescription,
                    e.episodeurl, &e.episodepubdate, e.episodeduration, e.episodeartwork,
                    Some(e.podcastname), e.downloaded, e.is_youtube,
                )).collect();
                ("Downloads".to_string(), "Your downloaded episodes from Pinepods".to_string(), items)
            }
            FeedSource::History => {
                let (rows, _) = self.user_history(user_id, lim, 0, "date", "desc", "").await?;
                let items = rows.iter()
                    .map(|v| Self::build_rss_episode_from_json(domain, &user_rss_key, user_id, v))
                    .collect();
                ("History".to_string(), "Your recently played episodes from Pinepods".to_string(), items)
            }
            FeedSource::Playlist(playlist_id) => {
                let value = self.get_playlist_episodes(user_id, playlist_id).await?;
                let name = value
                    .get("playlist_info")
                    .and_then(|p| p.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("Playlist")
                    .to_string();
                let items = value
                    .get("episodes")
                    .and_then(|e| e.as_array())
                    .map(|arr| arr.iter().take(take_n)
                        .map(|v| Self::build_rss_episode_from_json(domain, &user_rss_key, user_id, v))
                        .collect())
                    .unwrap_or_default();
                let desc = format!("Episodes from your \"{}\" playlist in Pinepods", name);
                (name, desc, items)
            }
            FeedSource::Collection(collection_id) => {
                // Ownership check
                let (owner_id, is_default) = self.get_collection_meta(collection_id).await?;
                if owner_id != user_id {
                    return Err(AppError::forbidden("You can only view your own collections!"));
                }
                let (eps, _) = if is_default {
                    self.get_saved_episodes(user_id, lim, 0, "date", "desc", "").await?
                } else {
                    self.get_collection_episodes(user_id, collection_id, lim, 0, "date", "desc", "").await?
                };
                let items = eps.into_iter().map(|e| Self::build_rss_episode(
                    domain, &user_rss_key, user_id, e.episodeid, e.episodetitle, e.episodedescription,
                    e.episodeurl, &e.episodepubdate, e.episodeduration, Some(e.episodeartwork),
                    Some(e.podcastname), e.downloaded, e.is_youtube,
                )).collect();
                // Resolve the collection name for the channel title.
                let name = self.get_collections(user_id).await?
                    .into_iter()
                    .find(|c| c.collection_id == collection_id)
                    .map(|c| c.name)
                    .unwrap_or_else(|| "Collection".to_string());
                let desc = format!("Episodes from your \"{}\" collection in Pinepods", name);
                (name, desc, items)
            }
        };

        Self::write_rss_document(
            &format!("Pinepods - {}", feed_name),
            &feed_description,
            &username,
            &default_image,
            &episodes,
        )
    }

    // Get user RSS key - matches Python get_user_rss_key function
    pub async fn get_user_rss_key(&self, user_id: i32) -> AppResult<Option<String>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT k.rsskey FROM "RssKeys" k
                    WHERE k.userid = $1 AND NOT EXISTS (SELECT 1 FROM "RssKeyMap" m WHERE m.rsskeyid = k.rsskeyid)
                    ORDER BY k.rsskeyid LIMIT 1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("rsskey").ok())
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT k.RssKey FROM RssKeys k
                    WHERE k.UserID = ? AND NOT EXISTS (SELECT 1 FROM RssKeyMap m WHERE m.RssKeyID = k.RssKeyID)
                    ORDER BY k.RssKeyID LIMIT 1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("RssKey").ok())
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get or create RSS key for user - ensures user always has an RSS key for stream URLs
    pub async fn get_or_create_user_rss_key(&self, user_id: i32) -> AppResult<String> {
        dialect_match!(self, {
            pg(pool) => {
                // Try to get existing RSS key
                // Only a universal key (no podcast scope) can stand in as "the user's RSS key".
                let existing_key = q::query(r#"SELECT k.rsskey FROM "RssKeys" k
                    WHERE k.userid = $1 AND NOT EXISTS (SELECT 1 FROM "RssKeyMap" m WHERE m.rsskeyid = k.rsskeyid)
                    ORDER BY k.rsskeyid LIMIT 1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = existing_key {
                    let key: String = row.try_get("rsskey")?;
                    Ok(key)
                } else {
                    // Create new RSS key
                    let new_key = uuid::Uuid::new_v4().to_string();
                    q::query(r#"INSERT INTO "RssKeys" (userid, rsskey) VALUES ($1, $2)"#)
                        .bind(user_id)
                        .bind(&new_key)
                        .execute(pool)
                        .await?;
                    Ok(new_key)
                }
            },
            mysql(pool) => {
                // Try to get existing RSS key
                let existing_key = sqlx::query("SELECT k.RssKey FROM RssKeys k
                    WHERE k.UserID = ? AND NOT EXISTS (SELECT 1 FROM RssKeyMap m WHERE m.RssKeyID = k.RssKeyID)
                    ORDER BY k.RssKeyID LIMIT 1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = existing_key {
                    let key: String = row.try_get("RssKey")?;
                    Ok(key)
                } else {
                    // Create new RSS key
                    let new_key = uuid::Uuid::new_v4().to_string();
                    sqlx::query("INSERT INTO RssKeys (UserID, RssKey) VALUES (?, ?)")
                        .bind(user_id)
                        .bind(&new_key)
                        .execute(pool)
                        .await?;
                    Ok(new_key)
                }
            },
        })
    }

    // Helper function to get RSS episodes
    pub(super) async fn get_rss_episodes(
        &self,
        user_id: i32,
        limit: i32,
        source_type: Option<&str>,
        podcast_ids: &[i32],
        podcast_filter: bool,
        domain: &str,
        api_key: &str,
    ) -> AppResult<Vec<RssEpisode>> {
        use chrono::{DateTime, Utc};
        
        dialect_match!(self, {
            pg(pool) => {
                let mut base_query = r#"
                    SELECT
                        e.episodeid,
                        pp.podcastid,
                        e.episodetitle,
                        e.episodedescription,
                        CASE WHEN de.episodeid IS NULL
                                THEN e.episodeurl
                                ELSE CONCAT(CAST($1 AS TEXT), '/api/data/stream/', e.episodeid, '?api_key=', CAST($2 AS TEXT), '&user_id=', pp.userid)
                        END as episodeurl,
                        e.episodeartwork,
                        e.episodepubdate,
                        e.episodeduration,
                        pp.podcastname,
                        pp.author,
                        pp.artworkurl,
                        pp.description as podcastdescription
                    FROM "Episodes" e
                    JOIN "Podcasts" pp ON e.feedid = pp.feedid
                    LEFT JOIN "DownloadedEpisodes" de ON e.episodeid = de.episodeid
                    WHERE pp.userid = $3
                "#.to_string();

                let mut param_count = 3;
                if podcast_filter && !podcast_ids.is_empty() {
                    param_count += 1;
                    base_query.push_str(&format!(" AND pp.podcastid = ANY(${})", param_count));
                }

                // Add YouTube union if needed (exact Python logic)
                let add_youtube_union = source_type.is_none() || source_type == Some("youtube");
                if add_youtube_union {
                    base_query.push_str(r#"
                        UNION ALL
                        SELECT
                            y.videoid as episodeid,
                            y.podcastid,
                            y.videotitle as episodetitle,
                            y.videodescription as episodedescription,
                            CONCAT(CAST($1 AS TEXT), '/api/data/stream/', CAST(y.videoid AS TEXT), '?api_key=', CAST($2 AS TEXT), '&type=youtube&user_id=', pv.userid) as episodeurl,
                            y.thumbnailurl as episodeartwork,
                            y.publishedat as episodepubdate,
                            y.duration as episodeduration,
                            pv.podcastname,
                            pv.author,
                            pv.artworkurl,
                            pv.description as podcastdescription
                        FROM "YouTubeVideos" y
                        JOIN "Podcasts" pv on y.podcastid = pv.podcastid
                        WHERE pv.userid = $3
                    "#);

                    if podcast_filter && !podcast_ids.is_empty() {
                        base_query.push_str(&format!(" AND pv.podcastid = ANY(${})", param_count));
                    }
                }

                base_query.push_str(" ORDER BY episodepubdate DESC");
                if limit > 0 {
                    base_query.push_str(&format!(" LIMIT {}", limit));
                }

                // Execute query
                let mut query = q::query(sqlx::AssertSqlSafe(base_query.as_str()))
                    .bind(domain)
                    .bind(api_key)
                    .bind(user_id);

                if podcast_filter && !podcast_ids.is_empty() {
                    query = query.bind(podcast_ids);
                }

                let rows = query.fetch_all(pool).await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    let title: String = row.try_get("episodetitle").unwrap_or_else(|_| "Untitled Episode".to_string());
                    let description: String = row.try_get("episodedescription").unwrap_or_else(|_| String::new());
                    let url: String = row.try_get("episodeurl").unwrap_or_else(|_| String::new());
                    let duration: Option<i32> = row.try_get("episodeduration").ok();
                    let author: Option<String> = row.try_get("author").ok();
                    // Use episode-specific artwork if available, otherwise fall back to podcast artwork
                    let episode_artwork: Option<String> = row.try_get("episodeartwork").ok();
                    let podcast_artwork: Option<String> = row.try_get("artworkurl").ok();
                    let artwork_url = episode_artwork.filter(|url| !url.is_empty()).or(podcast_artwork);
                    
                    let pub_date = match row.try_get::<chrono::NaiveDateTime, _>("episodepubdate") {
                        Ok(naive) => DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
                            .format("%a, %d %b %Y %H:%M:%S %z")
                            .to_string(),
                        Err(_) => Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string(),
                    };

                    episodes.push(RssEpisode {
                        title,
                        description,
                        url,
                        pub_date,
                        duration,
                        author,
                        artwork_url,
                    });
                }
                
                Ok(episodes)
            },
            mysql(pool) => {
                let mut base_query = r#"
                    SELECT
                        e.EpisodeID,
                        pp.PodcastID,
                        e.EpisodeTitle COLLATE utf8mb4_unicode_ci as EpisodeTitle,
                        e.EpisodeDescription COLLATE utf8mb4_unicode_ci as EpisodeDescription,
                        CASE WHEN de.EpisodeID IS NULL
                                THEN e.EpisodeURL COLLATE utf8mb4_unicode_ci
                                ELSE CONCAT(CAST(? AS CHAR), '/api/data/stream/', CAST(e.EpisodeID AS CHAR), '?api_key=', CAST(? AS CHAR), '&user_id=', pp.UserID)
                        END COLLATE utf8mb4_unicode_ci as EpisodeURL,
                        e.EpisodeArtwork COLLATE utf8mb4_unicode_ci as EpisodeArtwork,
                        e.EpisodePubDate,
                        e.EpisodeDuration,
                        pp.PodcastName COLLATE utf8mb4_unicode_ci as PodcastName,
                        pp.Author COLLATE utf8mb4_unicode_ci as Author,
                        pp.ArtworkURL COLLATE utf8mb4_unicode_ci as ArtworkURL,
                        pp.Description COLLATE utf8mb4_unicode_ci as PodcastDescription
                    FROM Episodes e
                    JOIN Podcasts pp ON e.FeedID = pp.FeedID
                    LEFT JOIN DownloadedEpisodes de ON e.EpisodeID = de.EpisodeID
                    WHERE pp.UserID = ?
                "#.to_string();

                if podcast_filter && !podcast_ids.is_empty() {
                    let placeholders = vec!["?"; podcast_ids.len()].join(",");
                    base_query.push_str(&format!(" AND pp.PodcastID IN ({})", placeholders));
                }

                // Add YouTube union if needed
                let add_youtube_union = source_type.is_none() || source_type == Some("youtube");
                if add_youtube_union {
                    base_query.push_str(r#"
                        UNION ALL
                        SELECT
                            y.VideoID as EpisodeID,
                            y.PodcastID as PodcastID,
                            y.VideoTitle COLLATE utf8mb4_unicode_ci as EpisodeTitle,
                            y.VideoDescription COLLATE utf8mb4_unicode_ci as EpisodeDescription,
                            CONCAT(CAST(? AS CHAR), '/api/data/stream/', CAST(y.VideoID AS CHAR), '?api_key=', CAST(? AS CHAR), '&type=youtube&user_id=', pv.UserID) COLLATE utf8mb4_unicode_ci as EpisodeURL,
                            y.ThumbnailURL COLLATE utf8mb4_unicode_ci as EpisodeArtwork,
                            y.PublishedAt as EpisodePubDate,
                            y.Duration as EpisodeDuration,
                            pv.PodcastName COLLATE utf8mb4_unicode_ci as PodcastName,
                            pv.Author COLLATE utf8mb4_unicode_ci as Author,
                            pv.ArtworkURL COLLATE utf8mb4_unicode_ci as ArtworkURL,
                            pv.Description COLLATE utf8mb4_unicode_ci as PodcastDescription
                        FROM YouTubeVideos y
                        JOIN Podcasts pv on y.PodcastID = pv.PodcastID
                        WHERE pv.UserID = ?
                    "#);

                    if podcast_filter && !podcast_ids.is_empty() {
                        let placeholders = vec!["?"; podcast_ids.len()].join(",");
                        base_query.push_str(&format!(" AND pv.PodcastID IN ({})", placeholders));
                    }
                }

                base_query.push_str(" ORDER BY EpisodePubDate DESC");
                if limit > 0 {
                    base_query.push_str(&format!(" LIMIT {}", limit));
                }

                // Build query with parameters
                let mut query = sqlx::query(sqlx::AssertSqlSafe(base_query.as_str()))
                    .bind(domain)
                    .bind(api_key)
                    .bind(user_id);

                if podcast_filter && !podcast_ids.is_empty() {
                    for &id in podcast_ids {
                        query = query.bind(id);
                    }
                }

                if add_youtube_union {
                    query = query.bind(domain).bind(api_key).bind(user_id);
                    if podcast_filter && !podcast_ids.is_empty() {
                        for &id in podcast_ids {
                            query = query.bind(id);
                        }
                    }
                }

                let rows = query.fetch_all(pool).await?;
                
                let mut episodes = Vec::new();
                for row in rows {
                    let title: String = row.try_get("EpisodeTitle").unwrap_or_else(|_| "Untitled Episode".to_string());
                    let description: String = row.try_get("EpisodeDescription").unwrap_or_else(|_| String::new());
                    let url: String = row.try_get("EpisodeURL").unwrap_or_else(|_| String::new());
                    let duration: Option<i32> = row.try_get("EpisodeDuration").ok();
                    let author: Option<String> = row.try_get("Author").ok();
                    // Use episode-specific artwork if available, otherwise fall back to podcast artwork
                    let episode_artwork: Option<String> = row.try_get("EpisodeArtwork").ok();
                    let podcast_artwork: Option<String> = row.try_get("ArtworkURL").ok();
                    let artwork_url = episode_artwork.filter(|url| !url.is_empty()).or(podcast_artwork);
                    
                    let pub_date = match row.try_get::<chrono::NaiveDateTime, _>("EpisodePubDate") {
                        Ok(naive) => DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc)
                            .format("%a, %d %b %Y %H:%M:%S %z")
                            .to_string(),
                        Err(_) => Utc::now().format("%a, %d %b %Y %H:%M:%S %z").to_string(),
                    };

                    episodes.push(RssEpisode {
                        title,
                        description,
                        url,
                        pub_date,
                        duration,
                        author,
                        artwork_url,
                    });
                }
                
                Ok(episodes)
            },
        })
    }
}
