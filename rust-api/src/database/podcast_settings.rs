//! Per-podcast user settings: speed, auto-download, categories, favorites.

use super::*;

impl DatabasePool {
    // Get auto download status for user
    pub async fn get_auto_download_status(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT auto_download_episodes FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    Ok(row.try_get::<bool, _>("auto_download_episodes")?)
                } else {
                    Ok(false)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT AutoDownloadEpisodes FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    Ok(row.try_get::<bool, _>("AutoDownloadEpisodes")?)
                } else {
                    Ok(false)
                }
            },
        })
    }

    // Set podcast playback speed - matches Python set_podcast_playback_speed function
    pub async fn set_podcast_playback_speed(&self, user_id: i32, podcast_id: i32, playback_speed: f64) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET playbackspeed = $1, playbackspeedcustomized = TRUE WHERE podcastid = $2 AND userid = $3"#)
                    .bind(playback_speed)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET PlaybackSpeed = ?, PlaybackSpeedCustomized = TRUE WHERE PodcastID = ? AND UserID = ?")
                    .bind(playback_speed)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Clear podcast playback speed - resets the podcast back to the global default
    pub async fn clear_podcast_playback_speed(&self, user_id: i32, podcast_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET playbackspeed = 1.0, playbackspeedcustomized = FALSE WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET PlaybackSpeed = 1.0, PlaybackSpeedCustomized = FALSE WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set per-podcast auto-delete-downloads days override (#655) - mirrors set_podcast_playback_speed
    pub async fn set_podcast_auto_download_delete_days(&self, user_id: i32, podcast_id: i32, days: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET autodownloaddeletedays = $1, autodownloaddeletecustomized = TRUE WHERE podcastid = $2 AND userid = $3"#)
                    .bind(days)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET AutoDownloadDeleteDays = ?, AutoDownloadDeleteCustomized = TRUE WHERE PodcastID = ? AND UserID = ?")
                    .bind(days)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Clear per-podcast auto-delete override - resets the podcast back to the global default (#655)
    pub async fn clear_podcast_auto_download_delete_days(&self, user_id: i32, podcast_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET autodownloaddeletedays = 0, autodownloaddeletecustomized = FALSE WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET AutoDownloadDeleteDays = 0, AutoDownloadDeleteCustomized = FALSE WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Enable/disable auto download for podcast - matches Python enable_auto_download function
    pub async fn enable_auto_download(&self, podcast_id: i32, auto_download: bool, user_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET autodownload = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(auto_download)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET AutoDownload = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(auto_download)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Enable/disable auto play next for podcast
    pub async fn enable_auto_play_next(&self, podcast_id: i32, auto_play_next: bool, user_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET autoplaynext = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(auto_play_next)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET AutoPlayNext = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(auto_play_next)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Enable/disable auto-queue new episodes for podcast (#648)
    pub async fn enable_auto_queue(&self, podcast_id: i32, auto_queue: bool, user_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET autoqueue = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(auto_queue)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET AutoQueue = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(auto_queue)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    /// Whether the scheduled refresh and Refresh all check this podcast. `None` when the user has
    /// no such podcast.
    pub async fn get_auto_refresh(&self, podcast_id: i32, user_id: i32) -> AppResult<Option<bool>> {
        let enabled = dialect_match!(self, {
            pg(pool) => {
                q::query_scalar::<Option<bool>>(r#"SELECT refreshpodcast FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
            },
            mysql(pool) => {
                sqlx::query_scalar::<_, Option<bool>>("SELECT RefreshPodcast FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
            },
        });
        Ok(enabled.map(|e| e.unwrap_or(true)))
    }

    /// Turn a podcast's automatic refreshes on or off. `false` when the user has no such podcast.
    pub async fn set_auto_refresh(&self, podcast_id: i32, user_id: i32, enabled: bool) -> AppResult<bool> {
        let updated = dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET refreshpodcast = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(enabled)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            },
            mysql(pool) => {
                // MySQL counts only changed rows: check the row exists instead.
                sqlx::query("UPDATE Podcasts SET RefreshPodcast = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(enabled)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                u64::from(self.get_auto_refresh(podcast_id, user_id).await?.is_some())
            },
        });
        Ok(updated > 0)
    }

    // Toggle podcast notifications - matches Python toggle_podcast_notifications function
    pub async fn toggle_podcast_notifications(&self, user_id: i32, podcast_id: i32, enabled: bool) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                // First verify user owns the podcast
                let row = q::query(r#"SELECT 1 FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if row.is_none() {
                    return Ok(false);
                }
                
                // Update notifications setting
                let result = q::query(r#"UPDATE "Podcasts" SET notificationsenabled = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(enabled)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                // First verify user owns the podcast
                let row = sqlx::query("SELECT 1 FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if row.is_none() {
                    return Ok(false);
                }
                
                // Update notifications setting
                let result = sqlx::query("UPDATE Podcasts SET NotificationsEnabled = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(enabled)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Toggle podcast favorite status
    pub async fn toggle_podcast_favorite(&self, user_id: i32, podcast_id: i32, is_favorite: bool) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT 1 FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                if row.is_none() {
                    return Ok(false);
                }
                let result = q::query(r#"UPDATE "Podcasts" SET isfavorite = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(is_favorite)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT 1 FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                if row.is_none() {
                    return Ok(false);
                }
                let result = sqlx::query("UPDATE Podcasts SET IsFavorite = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(is_favorite)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                Ok(result.rows_affected() > 0)
            },
        })
    }

    // Get podcast favorite status
    pub async fn get_podcast_favorite_status(&self, user_id: i32, podcast_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT COALESCE(isfavorite, false) as isfavorite FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(row.map(|r| r.try_get::<bool, _>("isfavorite").unwrap_or(false)).unwrap_or(false))
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT COALESCE(IsFavorite, false) as isfavorite FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(row.map(|r| r.try_get::<bool, _>("isfavorite").unwrap_or(false)).unwrap_or(false))
            },
        })
    }

    // Adjust skip times for podcast - matches Python adjust_skip_times function
    pub async fn adjust_skip_times(&self, podcast_id: i32, start_skip: i32, end_skip: i32, user_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET startskip = $1, endskip = $2 WHERE podcastid = $3 AND userid = $4"#)
                    .bind(start_skip)
                    .bind(end_skip)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET StartSkip = ?, EndSkip = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(start_skip)
                    .bind(end_skip)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Remove category from podcast - matches Python remove_category function
    pub async fn remove_category(&self, podcast_id: i32, user_id: i32, category: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                // Fetch current categories
                let row = q::query(r#"SELECT categories FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let current_categories_str: String = row.try_get("categories").unwrap_or_default();
                    let mut categories = self.parse_categories_json(&current_categories_str).unwrap_or_default();
                    
                    // Remove the category by value (find and remove the key with this value)
                    categories.retain(|_key, value| value != category);
                    
                    // Convert back to JSON string
                    let new_categories_json = serde_json::to_string(&categories)
                        .unwrap_or_else(|_| "{}".to_string());
                    
                    // Update with new categories
                    q::query(r#"UPDATE "Podcasts" SET categories = $1 WHERE podcastid = $2 AND userid = $3"#)
                        .bind(&new_categories_json)
                        .bind(podcast_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                }
            },
            mysql(pool) => {
                // Fetch current categories
                let row = sqlx::query("SELECT Categories FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let current_categories_str: String = row.try_get("Categories").unwrap_or_default();
                    let mut categories = self.parse_categories_json(&current_categories_str).unwrap_or_default();
                    
                    // Remove the category by value (find and remove the key with this value)
                    categories.retain(|_key, value| value != category);
                    
                    // Convert back to JSON string
                    let new_categories_json = serde_json::to_string(&categories)
                        .unwrap_or_else(|_| "{}".to_string());
                    
                    // Update with new categories
                    sqlx::query("UPDATE Podcasts SET Categories = ? WHERE PodcastID = ? AND UserID = ?")
                        .bind(&new_categories_json)
                        .bind(podcast_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                }
            },
        });
        Ok(())
    }

    // One-time cleanup: strip blank/whitespace category values that older adds
    // could have stored (e.g. {"0": ""}). Idempotent - only rewrites rows that
    // actually changed. Safe to run on every startup.
    pub async fn cleanup_blank_categories(&self) -> AppResult<()> {
        let mut cleaned = 0u32;
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(r#"SELECT podcastid, categories FROM "Podcasts""#)
                    .fetch_all(pool)
                    .await?;

                for row in rows {
                    let podcast_id: i32 = row.try_get("podcastid")?;
                    let current_categories_str: String = row.try_get("categories").unwrap_or_default();
                    let mut categories = self.parse_categories_json(&current_categories_str).unwrap_or_default();

                    let before = categories.len();
                    categories.retain(|_key, value| !value.trim().is_empty());
                    if categories.len() == before {
                        continue;
                    }

                    let new_categories_json = serde_json::to_string(&categories)
                        .unwrap_or_else(|_| "{}".to_string());
                    q::query(r#"UPDATE "Podcasts" SET categories = $1 WHERE podcastid = $2"#)
                        .bind(&new_categories_json)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    cleaned += 1;
                }
            },
            mysql(pool) => {
                let rows = sqlx::query(r#"SELECT PodcastID, Categories FROM Podcasts"#)
                    .fetch_all(pool)
                    .await?;

                for row in rows {
                    let podcast_id: i32 = row.try_get("PodcastID")?;
                    let current_categories_str: String = row.try_get("Categories").unwrap_or_default();
                    let mut categories = self.parse_categories_json(&current_categories_str).unwrap_or_default();

                    let before = categories.len();
                    categories.retain(|_key, value| !value.trim().is_empty());
                    if categories.len() == before {
                        continue;
                    }

                    let new_categories_json = serde_json::to_string(&categories)
                        .unwrap_or_else(|_| "{}".to_string());
                    sqlx::query("UPDATE Podcasts SET Categories = ? WHERE PodcastID = ?")
                        .bind(&new_categories_json)
                        .bind(podcast_id)
                        .execute(pool)
                        .await?;
                    cleaned += 1;
                }
            },
        });
        if cleaned > 0 {
            info!("Cleaned blank categories from {} podcast(s)", cleaned);
        }
        Ok(())
    }

    // Add category to podcast - matches Python add_category function
    pub async fn add_category(&self, podcast_id: i32, user_id: i32, category: &str) -> AppResult<String> {
        dialect_match!(self, {
            pg(pool) => {
                // Fetch current categories
                let row = q::query(r#"SELECT categories FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let current_categories_str: String = row.try_get("categories").unwrap_or_default();
                    let mut categories = self.parse_categories_json(&current_categories_str).unwrap_or_default();
                    
                    // Check if category already exists
                    if categories.values().any(|v| v == category) {
                        return Ok("Category already exists.".to_string());
                    }
                    
                    // Add new category with a generated key (use the next available number)
                    let next_key = if categories.is_empty() {
                        "1".to_string()
                    } else {
                        let max_key = categories.keys()
                            .filter_map(|k| k.parse::<i32>().ok())
                            .max()
                            .unwrap_or(0);
                        (max_key + 1).to_string()
                    };
                    
                    categories.insert(next_key, category.to_string());
                    
                    // Convert back to JSON string
                    let new_categories_json = serde_json::to_string(&categories)
                        .unwrap_or_else(|_| "{}".to_string());
                    
                    // Update with new categories
                    q::query(r#"UPDATE "Podcasts" SET categories = $1 WHERE podcastid = $2 AND userid = $3"#)
                        .bind(&new_categories_json)
                        .bind(podcast_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    
                    Ok("Category added!".to_string())
                } else {
                    Err(AppError::not_found("Podcast not found"))
                }
            },
            mysql(pool) => {
                // Fetch current categories
                let row = sqlx::query("SELECT Categories FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let current_categories_str: String = row.try_get("Categories").unwrap_or_default();
                    let mut categories = self.parse_categories_json(&current_categories_str).unwrap_or_default();
                    
                    // Check if category already exists
                    if categories.values().any(|v| v == category) {
                        return Ok("Category already exists.".to_string());
                    }
                    
                    // Add new category with a generated key (use the next available number)
                    let next_key = if categories.is_empty() {
                        "1".to_string()
                    } else {
                        let max_key = categories.keys()
                            .filter_map(|k| k.parse::<i32>().ok())
                            .max()
                            .unwrap_or(0);
                        (max_key + 1).to_string()
                    };
                    
                    categories.insert(next_key, category.to_string());
                    
                    // Convert back to JSON string
                    let new_categories_json = serde_json::to_string(&categories)
                        .unwrap_or_else(|_| "{}".to_string());
                    
                    // Update with new categories
                    sqlx::query("UPDATE Podcasts SET Categories = ? WHERE PodcastID = ? AND UserID = ?")
                        .bind(&new_categories_json)
                        .bind(podcast_id)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    
                    Ok("Category added!".to_string())
                } else {
                    Err(AppError::not_found("Podcast not found"))
                }
            },
        })
    }

    // Call get auto download status - matches Python call_get_auto_download_status function exactly
    pub async fn call_get_auto_download_status(&self, podcast_id: i32, user_id: i32) -> AppResult<Option<bool>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT autodownload FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("autodownload")?;
                    Ok(result)
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT AutoDownload FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("AutoDownload")?;
                    Ok(result)
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get auto-queue status for a podcast (#648)
    pub async fn call_get_auto_queue_status(&self, podcast_id: i32, user_id: i32) -> AppResult<Option<bool>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT autoqueue FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("autoqueue")?;
                    Ok(result)
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT AutoQueue FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("AutoQueue")?;
                    Ok(result)
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get auto play next status for a podcast
    pub async fn get_auto_play_next_status(&self, podcast_id: i32, user_id: i32) -> AppResult<Option<bool>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT autoplaynext FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("autoplaynext")?;
                    Ok(result)
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT AutoPlayNext FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = row {
                    let result: Option<bool> = row.try_get("AutoPlayNext")?;
                    Ok(result)
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get feed cutoff days - matches Python get_feed_cutoff_days function exactly
    pub async fn get_feed_cutoff_days(&self, podcast_id: i32, user_id: i32) -> AppResult<Option<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT feedcutoffdays FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<i32> = row.try_get("feedcutoffdays")?;
                    Ok(result.or(Some(365))) // Default to 365 if null
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT FeedCutoffDays FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                    
                if let Some(row) = row {
                    let result: Option<i32> = row.try_get("FeedCutoffDays")?;
                    Ok(result.or(Some(365))) // Default to 365 if null
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Per-podcast override; None goes back to the user's default.
    pub async fn set_podcast_auto_delete_on_complete(&self, user_id: i32, podcast_id: i32, enabled: Option<bool>) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET autodeleteoncomplete = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(enabled)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET AutoDeleteOnComplete = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(enabled)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set podcast cover preference - per-podcast setting
    pub async fn set_podcast_cover_preference(&self, user_id: i32, podcast_id: i32, use_podcast_covers: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET usepodcastcovers = $1, usepodcastcoverscustomized = TRUE WHERE podcastid = $2 AND userid = $3"#)
                    .bind(use_podcast_covers)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET UsePodcastCovers = ?, UsePodcastCoversCustomized = 1 WHERE PodcastID = ? AND UserID = ?")
                    .bind(use_podcast_covers as i32)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Clear podcast cover preference - reset to use global setting
    pub async fn clear_podcast_cover_preference(&self, user_id: i32, podcast_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET usepodcastcovers = FALSE, usepodcastcoverscustomized = FALSE WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Podcasts SET UsePodcastCovers = 0, UsePodcastCoversCustomized = 0 WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Get per-podcast cover preference - for episode layout page
    pub async fn get_podcast_cover_preference(&self, user_id: i32, podcast_id: i32) -> AppResult<Option<bool>> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query_scalar::<Option<bool>>(r#"SELECT usepodcastcovers FROM "Podcasts" WHERE podcastid = $1 AND userid = $2 AND usepodcastcoverscustomized = TRUE"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(result.flatten())
            },
            mysql(pool) => {
                let result = sqlx::query_scalar::<_, Option<i32>>("SELECT UsePodcastCovers FROM Podcasts WHERE PodcastID = ? AND UserID = ? AND UsePodcastCoversCustomized = 1")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                Ok(result.flatten().map(|val| val != 0))
            },
        })
    }

    // Get podcast notification status - matches Python get_podcast_notification_status function
    pub async fn get_podcast_notification_status(&self, podcast_id: i32, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"
                    SELECT notificationsenabled
                    FROM "Podcasts"
                    WHERE podcastid = $1 AND userid = $2
                "#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("notificationsenabled").unwrap_or(false))
                } else {
                    Ok(false)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("
                    SELECT NotificationsEnabled
                    FROM Podcasts
                    WHERE PodcastID = ? AND UserID = ?
                ")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let enabled: i8 = row.try_get("NotificationsEnabled").unwrap_or(0);
                    Ok(enabled != 0)
                } else {
                    Ok(false)
                }
            },
        })
    }

    // Update feed cutoff days - for update_feed_cutoff_days endpoint
    pub async fn update_feed_cutoff_days(&self, podcast_id: i32, user_id: i32, feed_cutoff_days: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                // First verify podcast exists and belongs to user
                let existing = q::query(r#"SELECT podcastid FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if existing.is_none() {
                    return Ok(false);
                }

                // Update the feed cutoff days
                let result = q::query(r#"UPDATE "Podcasts" SET feedcutoffdays = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(feed_cutoff_days)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                Ok(result.rows_affected() > 0)
            },
            mysql(pool) => {
                // First verify podcast exists and belongs to user
                let existing = sqlx::query("SELECT PodcastID FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if existing.is_none() {
                    return Ok(false);
                }

                // Update the feed cutoff days
                let result = sqlx::query("UPDATE Podcasts SET FeedCutoffDays = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(feed_cutoff_days)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                Ok(result.rows_affected() > 0)
            },
        })
    }
}
