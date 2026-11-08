//! Collections.

use super::*;

/// Serialize a collection's auto-add category list for storage.
/// Returns `None` (SQL NULL) when the list is absent or empty, otherwise a JSON array string.
pub(super) fn serialize_auto_add_categories(categories: Option<&[String]>) -> Option<String> {
    match categories {
        Some(cats) if !cats.is_empty() => serde_json::to_string(cats).ok(),
        _ => None,
    }
}

/// Parse a stored `AutoAddCategories` value (JSON array string) back into a list.
/// Returns `None` for NULL/empty/unparseable values.
pub(super) fn parse_auto_add_categories(stored: Option<String>) -> Option<Vec<String>> {
    let s = stored?;
    if s.trim().is_empty() {
        return None;
    }
    let cats = serde_json::from_str::<Vec<String>>(&s).ok()?;
    if cats.is_empty() { None } else { Some(cats) }
}

impl DatabasePool {
    // ---- Collections ------------------------------------------------------

    /// Look up a collection's owner and default flag. Returns (user_id, is_default).
    pub async fn get_collection_meta(&self, collection_id: i32) -> AppResult<(i32, bool)> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT userid, isdefault FROM "Collections" WHERE collectionid = $1"#,
                )
                .bind(collection_id)
                .fetch_optional(pool)
                .await?
                .ok_or_else(|| AppError::not_found("Collection not found"))?;
                Ok((row.try_get("userid")?, row.try_get("isdefault")?))
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT UserID, IsDefault FROM Collections WHERE CollectionID = ?")
                    .bind(collection_id)
                    .fetch_optional(pool)
                    .await?
                    .ok_or_else(|| AppError::not_found("Collection not found"))?;
                let is_default: i8 = row.try_get("IsDefault")?;
                Ok((row.try_get("UserID")?, is_default != 0))
            },
        })
    }

    pub async fn create_collection(
        &self,
        req: &crate::models::CreateCollectionRequest,
    ) -> AppResult<i32> {
        let icon = req.icon.clone().unwrap_or_else(|| "ph-bookmark-simple".to_string());
        let categories = serialize_auto_add_categories(req.auto_add_categories.as_deref());
        dialect_match!(self, {
            pg(pool) => {
                // Surface a clean conflict instead of a 500 on duplicate name
                let exists = q::query(
                    r#"SELECT 1 FROM "Collections" WHERE userid = $1 AND name = $2"#,
                )
                .bind(req.user_id)
                .bind(&req.name)
                .fetch_optional(pool)
                .await?;
                if exists.is_some() {
                    return Err(AppError::conflict("A collection with that name already exists"));
                }

                let row = q::query(
                    r#"INSERT INTO "Collections" (userid, name, description, icon, isdefault, autoaddcategories)
                       VALUES ($1, $2, $3, $4, FALSE, $5) RETURNING collectionid"#,
                )
                .bind(req.user_id)
                .bind(&req.name)
                .bind(&req.description)
                .bind(&icon)
                .bind(&categories)
                .fetch_one(pool)
                .await?;
                Ok(row.try_get("collectionid")?)
            },
            mysql(pool) => {
                let exists = sqlx::query("SELECT 1 FROM Collections WHERE UserID = ? AND Name = ?")
                    .bind(req.user_id)
                    .bind(&req.name)
                    .fetch_optional(pool)
                    .await?;
                if exists.is_some() {
                    return Err(AppError::conflict("A collection with that name already exists"));
                }

                let result = sqlx::query(
                    "INSERT INTO Collections (UserID, Name, Description, Icon, IsDefault, AutoAddCategories) VALUES (?, ?, ?, ?, FALSE, ?)",
                )
                .bind(req.user_id)
                .bind(&req.name)
                .bind(&req.description)
                .bind(&icon)
                .bind(&categories)
                .execute(pool)
                .await?;
                Ok(result.last_insert_id() as i32)
            },
        })
    }

    /// Guarantee the user has a default "Saved" collection (users created after the
    /// Collections migration, or any edge case, get one created lazily here).
    pub async fn ensure_default_collection(&self, user_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(
                    r#"INSERT INTO "Collections" (userid, name, isdefault, icon)
                       SELECT $1, 'Saved', TRUE, 'ph-bookmark-simple'
                       WHERE NOT EXISTS (
                           SELECT 1 FROM "Collections" WHERE userid = $1 AND isdefault = TRUE
                       )"#,
                )
                .bind(user_id)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                sqlx::query(
                    "INSERT INTO Collections (UserID, Name, IsDefault, Icon)
                     SELECT ?, 'Saved', TRUE, 'ph-bookmark-simple' FROM DUAL
                     WHERE NOT EXISTS (
                         SELECT 1 FROM Collections WHERE UserID = ? AND IsDefault = TRUE
                     )",
                )
                .bind(user_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
        });
        Ok(())
    }

    pub async fn get_collections(&self, user_id: i32) -> AppResult<Vec<crate::models::Collection>> {
        self.ensure_default_collection(user_id).await?;
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT
                        c.collectionid,
                        c.userid,
                        c.name,
                        c.description,
                        c.isdefault,
                        c.icon,
                        c.autoaddcategories,
                        c.createdat,
                        c.lastupdated,
                        CASE
                            WHEN c.isdefault THEN (
                                (SELECT COUNT(*) FROM "SavedEpisodes" se WHERE se.userid = c.userid)
                                + (SELECT COUNT(*) FROM "SavedVideos" sv WHERE sv.userid = c.userid)
                            )
                            ELSE (SELECT COUNT(*) FROM "CollectionEpisodes" ce WHERE ce.collectionid = c.collectionid)
                        END AS episode_count
                    FROM "Collections" c
                    WHERE c.userid = $1
                    ORDER BY c.isdefault DESC, c.createdat ASC"#,
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut collections = Vec::new();
                for row in rows {
                    collections.push(crate::models::Collection {
                        collection_id: row.try_get("collectionid")?,
                        user_id: row.try_get("userid")?,
                        name: row.try_get("name")?,
                        description: row.try_get("description").ok(),
                        is_default: row.try_get("isdefault")?,
                        icon: row.try_get("icon")?,
                        created_at: row
                            .try_get::<chrono::NaiveDateTime, _>("createdat")?
                            .format("%Y-%m-%dT%H:%M:%S")
                            .to_string(),
                        last_updated: row
                            .try_get::<chrono::NaiveDateTime, _>("lastupdated")?
                            .format("%Y-%m-%dT%H:%M:%S")
                            .to_string(),
                        episode_count: row.try_get("episode_count")?,
                        auto_add_categories: parse_auto_add_categories(row.try_get("autoaddcategories").ok()),
                    });
                }
                Ok(collections)
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT
                        c.CollectionID as collectionid,
                        c.UserID as userid,
                        c.Name as name,
                        c.Description as description,
                        c.IsDefault as isdefault,
                        c.Icon as icon,
                        c.AutoAddCategories as autoaddcategories,
                        c.CreatedAt as createdat,
                        c.LastUpdated as lastupdated,
                        CASE
                            WHEN c.IsDefault THEN (
                                (SELECT COUNT(*) FROM SavedEpisodes se WHERE se.UserID = c.UserID)
                                + (SELECT COUNT(*) FROM SavedVideos sv WHERE sv.UserID = c.UserID)
                            )
                            ELSE (SELECT COUNT(*) FROM CollectionEpisodes ce WHERE ce.CollectionID = c.CollectionID)
                        END AS episode_count
                    FROM Collections c
                    WHERE c.UserID = ?
                    ORDER BY c.IsDefault DESC, c.CreatedAt ASC",
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut collections = Vec::new();
                for row in rows {
                    let is_default: i8 = row.try_get("isdefault")?;
                    collections.push(crate::models::Collection {
                        collection_id: row.try_get("collectionid")?,
                        user_id: row.try_get("userid")?,
                        name: row.try_get("name")?,
                        description: row.try_get("description").ok(),
                        is_default: is_default != 0,
                        icon: row.try_get("icon")?,
                        // TIMESTAMP columns decode as DateTime<Utc> on MySQL (NaiveDateTime is DATETIME).
                        created_at: row
                            .try_get::<chrono::DateTime<chrono::Utc>, _>("createdat")?
                            .format("%Y-%m-%dT%H:%M:%S")
                            .to_string(),
                        last_updated: row
                            .try_get::<chrono::DateTime<chrono::Utc>, _>("lastupdated")?
                            .format("%Y-%m-%dT%H:%M:%S")
                            .to_string(),
                        episode_count: row.try_get("episode_count")?,
                        auto_add_categories: parse_auto_add_categories(row.try_get("autoaddcategories").ok()),
                    });
                }
                Ok(collections)
            },
        })
    }

    pub async fn update_collection(
        &self,
        user_id: i32,
        collection_id: i32,
        req: &crate::models::UpdateCollectionRequest,
    ) -> AppResult<()> {
        let (owner_id, is_default) = self.get_collection_meta(collection_id).await?;
        if owner_id != user_id {
            return Err(AppError::forbidden("You can only edit your own collections!"));
        }
        if is_default {
            return Err(AppError::bad_request("The default Saved collection cannot be edited"));
        }

        // Guard against renaming onto an existing name
        if let Some(ref new_name) = req.name {
            dialect_match!(self, {
                pg(pool) => {
                    let clash = q::query(
                        r#"SELECT 1 FROM "Collections" WHERE userid = $1 AND name = $2 AND collectionid <> $3"#,
                    )
                    .bind(user_id)
                    .bind(new_name)
                    .bind(collection_id)
                    .fetch_optional(pool)
                    .await?;
                    if clash.is_some() {
                        return Err(AppError::conflict("A collection with that name already exists"));
                    }
                },
                mysql(pool) => {
                    let clash = sqlx::query(
                        "SELECT 1 FROM Collections WHERE UserID = ? AND Name = ? AND CollectionID <> ?",
                    )
                    .bind(user_id)
                    .bind(new_name)
                    .bind(collection_id)
                    .fetch_optional(pool)
                    .await?;
                    if clash.is_some() {
                        return Err(AppError::conflict("A collection with that name already exists"));
                    }
                },
            })
        }

        dialect_match!(self, {
            pg(pool) => {
                q::query(
                    r#"UPDATE "Collections" SET
                        name = COALESCE($1, name),
                        description = COALESCE($2, description),
                        icon = COALESCE($3, icon),
                        lastupdated = CURRENT_TIMESTAMP
                       WHERE collectionid = $4 AND userid = $5 AND isdefault = FALSE"#,
                )
                .bind(&req.name)
                .bind(&req.description)
                .bind(&req.icon)
                .bind(collection_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                sqlx::query(
                    "UPDATE Collections SET
                        Name = COALESCE(?, Name),
                        Description = COALESCE(?, Description),
                        Icon = COALESCE(?, Icon),
                        LastUpdated = CURRENT_TIMESTAMP
                     WHERE CollectionID = ? AND UserID = ? AND IsDefault = FALSE",
                )
                .bind(&req.name)
                .bind(&req.description)
                .bind(&req.icon)
                .bind(collection_id)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
        });

        // Auto-add categories are set explicitly (an empty list clears the rule), so they can't
        // use the COALESCE-ignore-NULL pattern above — only touch the column when provided.
        if req.auto_add_categories.is_some() {
            let categories = serialize_auto_add_categories(req.auto_add_categories.as_deref());
            dialect_match!(self, {
                pg(pool) => {
                    q::query(
                        r#"UPDATE "Collections" SET autoaddcategories = $1, lastupdated = CURRENT_TIMESTAMP
                           WHERE collectionid = $2 AND userid = $3 AND isdefault = FALSE"#,
                    )
                    .bind(&categories)
                    .bind(collection_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                },
                mysql(pool) => {
                    sqlx::query(
                        "UPDATE Collections SET AutoAddCategories = ?, LastUpdated = CURRENT_TIMESTAMP
                         WHERE CollectionID = ? AND UserID = ? AND IsDefault = FALSE",
                    )
                    .bind(&categories)
                    .bind(collection_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
                },
            })
        }
        Ok(())
    }

    /// Distinct podcast category names across the user's subscriptions, sorted alphabetically.
    /// Feeds the auto-add category picker in the collection edit modal.
    pub async fn get_user_categories(&self, user_id: i32) -> AppResult<Vec<String>> {
        let raw_rows: Vec<String> = dialect_match!(self, {
            pg(pool) => {
                q::query_scalar(
                    r#"SELECT categories FROM "Podcasts"
                       WHERE userid = $1 AND categories IS NOT NULL AND categories <> ''"#,
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?
            },
            mysql(pool) => {
                sqlx::query_scalar(
                    "SELECT Categories FROM Podcasts
                     WHERE UserID = ? AND Categories IS NOT NULL AND Categories <> ''",
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?
            },
        });

        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for raw in &raw_rows {
            if let Some(map) = self.parse_categories_json(raw) {
                for name in map.values() {
                    let trimmed = name.trim();
                    if !trimmed.is_empty() {
                        seen.insert(trimmed.to_string());
                    }
                }
            }
        }
        Ok(seen.into_iter().collect())
    }

    /// Auto-add every episode from the owner's podcasts in the collection's category rule that
    /// isn't already a member. Idempotent (skips existing rows); no-op for the default collection
    /// or when the collection has no category rule. Categories are OR-matched as case-insensitive
    /// substrings.
    pub async fn auto_add_category_episodes(&self, collection_id: i32) -> AppResult<()> {
        // Load the collection's owner, default flag, and category rule in one shot.
        let (is_default, categories, owner) = dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT isdefault, autoaddcategories, userid FROM "Collections" WHERE collectionid = $1"#,
                )
                .bind(collection_id)
                .fetch_optional(pool)
                .await?;
                match row {
                    Some(r) => (
                        r.try_get::<bool, _>("isdefault")?,
                        parse_auto_add_categories(r.try_get("autoaddcategories").ok()),
                        r.try_get::<i32, _>("userid")?,
                    ),
                    None => return Ok(()),
                }
            },
            mysql(pool) => {
                let row = sqlx::query(
                    "SELECT IsDefault, AutoAddCategories as autoaddcategories, UserID FROM Collections WHERE CollectionID = ?",
                )
                .bind(collection_id)
                .fetch_optional(pool)
                .await?;
                match row {
                    Some(r) => (
                        r.try_get::<i8, _>("IsDefault")? != 0,
                        parse_auto_add_categories(r.try_get("autoaddcategories").ok()),
                        r.try_get::<i32, _>("UserID")?,
                    ),
                    None => return Ok(()),
                }
            },
        });

        if is_default {
            return Ok(());
        }
        let categories = match categories {
            Some(c) if !c.is_empty() => c,
            _ => return Ok(()),
        };

        dialect_match!(self, {
            pg(pool) => {
                // Category placeholders start at $2 ($1 is collection_id).
                let cat_clause: String = (0..categories.len())
                    .map(|i| format!("LOWER(p.categories) LIKE LOWER(${})", i + 2))
                    .collect::<Vec<_>>()
                    .join(" OR ");
                let owner_param = categories.len() + 2;
                let sql = format!(
                    r#"INSERT INTO "CollectionEpisodes" (collectionid, episodeid)
                       SELECT $1, e.episodeid
                       FROM "Episodes" e
                       JOIN "Podcasts" p ON e.feedid = p.feedid
                       WHERE ({cat_clause})
                         AND p.userid = ${owner_param}
                         AND NOT EXISTS (
                             SELECT 1 FROM "CollectionEpisodes" ce
                             WHERE ce.collectionid = $1 AND ce.episodeid = e.episodeid
                         )"#,
                );
                let mut q = q::query(sqlx::AssertSqlSafe(sql.as_str())).bind(collection_id);
                for cat in &categories {
                    q = q.bind(format!("%{}%", cat));
                }
                q.bind(owner).execute(pool).await?;

                q::query(r#"UPDATE "Collections" SET lastupdated = CURRENT_TIMESTAMP WHERE collectionid = $1"#)
                    .bind(collection_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                let cat_clause: String = std::iter::repeat("LOWER(p.Categories) LIKE LOWER(?)")
                    .take(categories.len())
                    .collect::<Vec<_>>()
                    .join(" OR ");
                let sql = format!(
                    "INSERT INTO CollectionEpisodes (CollectionID, EpisodeID)
                     SELECT ?, e.EpisodeID
                     FROM Episodes e
                     JOIN Podcasts p ON e.FeedID = p.FeedID
                     WHERE ({cat_clause})
                       AND p.UserID = ?
                       AND NOT EXISTS (
                           SELECT 1 FROM CollectionEpisodes ce
                           WHERE ce.CollectionID = ? AND ce.EpisodeID = e.EpisodeID
                       )",
                );
                let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(collection_id);
                for cat in &categories {
                    q = q.bind(format!("%{}%", cat));
                }
                q = q.bind(owner).bind(collection_id);
                q.execute(pool).await?;

                sqlx::query("UPDATE Collections SET LastUpdated = CURRENT_TIMESTAMP WHERE CollectionID = ?")
                    .bind(collection_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    /// Add new episodes of one of the user's podcasts to the user's collections whose category
    /// rule matches the podcast (same matching as `auto_add_category_episodes`). Called by the
    /// refresh with the episodes it just inserted.
    pub async fn auto_add_category_episodes_for(&self, user_id: i32, podcast_id: i32, episode_ids: &[i32]) -> AppResult<()> {
        if episode_ids.is_empty() {
            return Ok(());
        }
        let (rules, categories): (Vec<(i32, Option<String>)>, Option<String>) = dialect_match!(self, {
            pg(pool) => (
                q::query_as(
                    r#"SELECT collectionid, autoaddcategories FROM "Collections"
                       WHERE userid = $1 AND isdefault = FALSE AND autoaddcategories IS NOT NULL AND autoaddcategories <> ''"#,
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?,
                q::query_scalar(r#"SELECT categories FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .flatten(),
            ),
            mysql(pool) => (
                sqlx::query_as(
                    "SELECT CollectionID, AutoAddCategories FROM Collections
                     WHERE UserID = ? AND IsDefault = FALSE AND AutoAddCategories IS NOT NULL AND AutoAddCategories <> ''",
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?,
                sqlx::query_scalar("SELECT Categories FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .flatten(),
            ),
        });
        let podcast_categories = categories.unwrap_or_default().to_lowercase();
        let matching = rules.into_iter().filter_map(|(id, rule)| {
            parse_auto_add_categories(rule)?.iter().any(|c| podcast_categories.contains(&c.to_lowercase())).then_some(id)
        });
        for collection_id in matching {
            dialect_match!(self, {
                pg(pool) => {
                    q::query(
                        r#"INSERT INTO "CollectionEpisodes" (collectionid, episodeid)
                           SELECT $1, e.episodeid FROM "Episodes" e
                           WHERE e.episodeid = ANY($2) AND e.feedid = (SELECT feedid FROM "Podcasts" WHERE podcastid = $3)
                             AND NOT EXISTS (SELECT 1 FROM "CollectionEpisodes" ce WHERE ce.collectionid = $1 AND ce.episodeid = e.episodeid)"#,
                    )
                    .bind(collection_id)
                    .bind(episode_ids)
                    .bind(podcast_id)
                    .execute(pool)
                    .await?;
                    q::query(r#"UPDATE "Collections" SET lastupdated = CURRENT_TIMESTAMP WHERE collectionid = $1"#)
                        .bind(collection_id)
                        .execute(pool)
                        .await?;
                },
                mysql(pool) => {
                    let ids = vec!["?"; episode_ids.len()].join(", ");
                    let sql = format!(
                        "INSERT INTO CollectionEpisodes (CollectionID, EpisodeID)
                         SELECT ?, e.EpisodeID FROM Episodes e
                         WHERE e.EpisodeID IN ({ids}) AND e.FeedID = (SELECT FeedID FROM Podcasts WHERE PodcastID = ?)
                           AND NOT EXISTS (SELECT 1 FROM CollectionEpisodes ce WHERE ce.CollectionID = ? AND ce.EpisodeID = e.EpisodeID)"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(collection_id);
                    for id in episode_ids {
                        q = q.bind(id);
                    }
                    q.bind(podcast_id).bind(collection_id).execute(pool).await?;
                    sqlx::query("UPDATE Collections SET LastUpdated = CURRENT_TIMESTAMP WHERE CollectionID = ?")
                        .bind(collection_id)
                        .execute(pool)
                        .await?;
                },
            })
        }
        Ok(())
    }

    pub async fn delete_collection(&self, user_id: i32, collection_id: i32) -> AppResult<()> {
        let (owner_id, is_default) = self.get_collection_meta(collection_id).await?;
        if owner_id != user_id {
            return Err(AppError::forbidden("You can only delete your own collections!"));
        }
        if is_default {
            return Err(AppError::bad_request("The default Saved collection cannot be deleted"));
        }
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"DELETE FROM "Collections" WHERE collectionid = $1 AND userid = $2 AND isdefault = FALSE"#)
                    .bind(collection_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("DELETE FROM Collections WHERE CollectionID = ? AND UserID = ? AND IsDefault = FALSE")
                    .bind(collection_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    pub async fn add_episode_to_collection(
        &self,
        user_id: i32,
        collection_id: i32,
        episode_id: i32,
        is_youtube: bool,
    ) -> AppResult<()> {
        let (owner_id, is_default) = self.get_collection_meta(collection_id).await?;
        if owner_id != user_id {
            return Err(AppError::forbidden("You can only modify your own collections!"));
        }
        // The default collection is backed by SavedEpisodes/SavedVideos (keeps UserStats correct).
        if is_default {
            return self.save_episode(episode_id, user_id, is_youtube).await;
        }

        let (ep_col, vid_col) = if is_youtube { (None, Some(episode_id)) } else { (Some(episode_id), None) };
        dialect_match!(self, {
            pg(pool) => {
                let existing = if is_youtube {
                    q::query(r#"SELECT 1 FROM "CollectionEpisodes" WHERE collectionid = $1 AND videoid = $2"#)
                        .bind(collection_id).bind(episode_id).fetch_optional(pool).await?
                } else {
                    q::query(r#"SELECT 1 FROM "CollectionEpisodes" WHERE collectionid = $1 AND episodeid = $2"#)
                        .bind(collection_id).bind(episode_id).fetch_optional(pool).await?
                };
                if existing.is_none() {
                    q::query(r#"INSERT INTO "CollectionEpisodes" (collectionid, episodeid, videoid) VALUES ($1, $2, $3)"#)
                        .bind(collection_id).bind(ep_col).bind(vid_col).execute(pool).await?;
                    q::query(r#"UPDATE "Collections" SET lastupdated = CURRENT_TIMESTAMP WHERE collectionid = $1"#)
                        .bind(collection_id).execute(pool).await?;
                }
            },
            mysql(pool) => {
                let existing = if is_youtube {
                    sqlx::query("SELECT 1 FROM CollectionEpisodes WHERE CollectionID = ? AND VideoID = ?")
                        .bind(collection_id).bind(episode_id).fetch_optional(pool).await?
                } else {
                    sqlx::query("SELECT 1 FROM CollectionEpisodes WHERE CollectionID = ? AND EpisodeID = ?")
                        .bind(collection_id).bind(episode_id).fetch_optional(pool).await?
                };
                if existing.is_none() {
                    sqlx::query("INSERT INTO CollectionEpisodes (CollectionID, EpisodeID, VideoID) VALUES (?, ?, ?)")
                        .bind(collection_id).bind(ep_col).bind(vid_col).execute(pool).await?;
                    sqlx::query("UPDATE Collections SET LastUpdated = CURRENT_TIMESTAMP WHERE CollectionID = ?")
                        .bind(collection_id).execute(pool).await?;
                }
            },
        });
        Ok(())
    }

    pub async fn remove_episode_from_collection(
        &self,
        user_id: i32,
        collection_id: i32,
        episode_id: i32,
        is_youtube: bool,
    ) -> AppResult<()> {
        let (owner_id, is_default) = self.get_collection_meta(collection_id).await?;
        if owner_id != user_id {
            return Err(AppError::forbidden("You can only modify your own collections!"));
        }
        if is_default {
            return self.remove_saved_episode(episode_id, user_id, is_youtube).await;
        }
        dialect_match!(self, {
            pg(pool) => {
                if is_youtube {
                    q::query(r#"DELETE FROM "CollectionEpisodes" WHERE collectionid = $1 AND videoid = $2"#)
                        .bind(collection_id).bind(episode_id).execute(pool).await?;
                } else {
                    q::query(r#"DELETE FROM "CollectionEpisodes" WHERE collectionid = $1 AND episodeid = $2"#)
                        .bind(collection_id).bind(episode_id).execute(pool).await?;
                }
            },
            mysql(pool) => {
                if is_youtube {
                    sqlx::query("DELETE FROM CollectionEpisodes WHERE CollectionID = ? AND VideoID = ?")
                        .bind(collection_id).bind(episode_id).execute(pool).await?;
                } else {
                    sqlx::query("DELETE FROM CollectionEpisodes WHERE CollectionID = ? AND EpisodeID = ?")
                        .bind(collection_id).bind(episode_id).execute(pool).await?;
                }
            },
        });
        Ok(())
    }

    pub async fn bulk_add_to_collection(
        &self,
        user_id: i32,
        collection_id: i32,
        episodes: &[(i32, bool)],
    ) -> AppResult<()> {
        // Ownership is checked once per episode inside add_episode_to_collection (cheap
        // metadata lookup); acceptable for the bulk sizes this endpoint handles.
        for (episode_id, is_youtube) in episodes {
            self.add_episode_to_collection(user_id, collection_id, *episode_id, *is_youtube).await?;
        }
        Ok(())
    }

    /// Collection IDs (including the default Saved collection) that contain this episode.
    pub async fn get_episode_collections(
        &self,
        user_id: i32,
        episode_id: i32,
        is_youtube: bool,
    ) -> AppResult<Vec<i32>> {
        let mut ids: Vec<i32> = Vec::new();
        dialect_match!(self, {
            pg(pool) => {
                let rows = if is_youtube {
                    q::query(
                        r#"SELECT ce.collectionid FROM "CollectionEpisodes" ce
                           JOIN "Collections" c ON ce.collectionid = c.collectionid
                           WHERE c.userid = $1 AND ce.videoid = $2"#,
                    )
                    .bind(user_id).bind(episode_id).fetch_all(pool).await?
                } else {
                    q::query(
                        r#"SELECT ce.collectionid FROM "CollectionEpisodes" ce
                           JOIN "Collections" c ON ce.collectionid = c.collectionid
                           WHERE c.userid = $1 AND ce.episodeid = $2"#,
                    )
                    .bind(user_id).bind(episode_id).fetch_all(pool).await?
                };
                for row in rows {
                    ids.push(row.try_get("collectionid")?);
                }

                // Include the default collection if the episode is saved
                let saved = if is_youtube {
                    q::query(r#"SELECT 1 FROM "SavedVideos" WHERE userid = $1 AND videoid = $2"#)
                        .bind(user_id).bind(episode_id).fetch_optional(pool).await?
                } else {
                    q::query(r#"SELECT 1 FROM "SavedEpisodes" WHERE userid = $1 AND episodeid = $2"#)
                        .bind(user_id).bind(episode_id).fetch_optional(pool).await?
                };
                if saved.is_some() {
                    if let Some(row) = q::query(r#"SELECT collectionid FROM "Collections" WHERE userid = $1 AND isdefault = TRUE"#)
                        .bind(user_id).fetch_optional(pool).await?
                    {
                        ids.push(row.try_get("collectionid")?);
                    }
                }
            },
            mysql(pool) => {
                let rows = if is_youtube {
                    sqlx::query(
                        "SELECT ce.CollectionID as collectionid FROM CollectionEpisodes ce
                         JOIN Collections c ON ce.CollectionID = c.CollectionID
                         WHERE c.UserID = ? AND ce.VideoID = ?",
                    )
                    .bind(user_id).bind(episode_id).fetch_all(pool).await?
                } else {
                    sqlx::query(
                        "SELECT ce.CollectionID as collectionid FROM CollectionEpisodes ce
                         JOIN Collections c ON ce.CollectionID = c.CollectionID
                         WHERE c.UserID = ? AND ce.EpisodeID = ?",
                    )
                    .bind(user_id).bind(episode_id).fetch_all(pool).await?
                };
                for row in rows {
                    ids.push(row.try_get("collectionid")?);
                }

                let saved = if is_youtube {
                    sqlx::query("SELECT 1 FROM SavedVideos WHERE UserID = ? AND VideoID = ?")
                        .bind(user_id).bind(episode_id).fetch_optional(pool).await?
                } else {
                    sqlx::query("SELECT 1 FROM SavedEpisodes WHERE UserID = ? AND EpisodeID = ?")
                        .bind(user_id).bind(episode_id).fetch_optional(pool).await?
                };
                if saved.is_some() {
                    if let Some(row) = sqlx::query("SELECT CollectionID as collectionid FROM Collections WHERE UserID = ? AND IsDefault = TRUE")
                        .bind(user_id).fetch_optional(pool).await?
                    {
                        ids.push(row.try_get("collectionid")?);
                    }
                }
            },
        });
        Ok(ids)
    }

    /// Paginated episode list for a non-default collection. Mirrors get_saved_episodes but
    /// sources rows from CollectionEpisodes and computes `saved` against the user's SavedEpisodes.
    pub async fn get_collection_episodes(
        &self,
        user_id: i32,
        collection_id: i32,
        limit: i64,
        offset: i64,
        sort_by: &str,
        sort_order: &str,
        filter: &str,
    ) -> AppResult<(Vec<crate::models::SavedEpisode>, i64)> {
        // `savedate` is the AddedAt alias here, matching get_saved_episodes' sort contract
        let order_col = match sort_by {
            "duration" => "episodeduration",
            "title"    => "episodetitle",
            _          => "savedate",
        };
        let order_dir = if sort_order == "asc" { "ASC" } else { "DESC" };

        dialect_match!(self, {
            pg(pool) => {
                let filter_clause = match filter {
                    "completed"   => " WHERE completed = true",
                    "in_progress" => " WHERE completed = false AND listenduration IS NOT NULL AND listenduration > 0",
                    _             => "",
                };
                let sql = format!(
                    r#"SELECT *, COUNT(*) OVER() AS total_count FROM (
                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "Episodes".episodetitle as episodetitle,
                            "Episodes".episodepubdate as episodepubdate,
                            "Episodes".episodedescription as episodedescription,
                            "Episodes".episodeid as episodeid,
                            CASE
                                WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                ELSE "Episodes".episodeartwork
                            END as episodeartwork,
                            "Episodes".episodeurl as episodeurl,
                            "Episodes".episodeduration as episodeduration,
                            "Podcasts".websiteurl as websiteurl,
                            "UserEpisodeHistory".listenduration as listenduration,
                            COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = "Episodes".episodeid AND ues.userid = "Podcasts".userid), FALSE) as completed,
                            CASE WHEN "SavedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedEpisodes".episodeid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube,
                            "Podcasts".podcastid as podcastid,
                            "CollectionEpisodes".addedat as savedate
                        FROM "CollectionEpisodes"
                        INNER JOIN "Episodes" ON "CollectionEpisodes".episodeid = "Episodes".episodeid
                        INNER JOIN "Podcasts" ON "Episodes".feedid = "Podcasts".feedid
                            AND "Podcasts".userid = (SELECT userid FROM "Collections" WHERE collectionid = "CollectionEpisodes".collectionid)
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                        LEFT JOIN "UserEpisodeHistory" ON
                            "CollectionEpisodes".episodeid = "UserEpisodeHistory".episodeid
                            AND "UserEpisodeHistory".userid = $1
                        LEFT JOIN "EpisodeQueue" ON
                            "CollectionEpisodes".episodeid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $2
                            AND "EpisodeQueue".is_youtube = FALSE
                        LEFT JOIN "DownloadedEpisodes" ON
                            "CollectionEpisodes".episodeid = "DownloadedEpisodes".episodeid
                            AND "DownloadedEpisodes".userid = $3
                        LEFT JOIN "SavedEpisodes" ON
                            "CollectionEpisodes".episodeid = "SavedEpisodes".episodeid
                            AND "SavedEpisodes".userid = $4
                        WHERE "CollectionEpisodes".collectionid = $5 AND "CollectionEpisodes".episodeid IS NOT NULL

                        UNION ALL

                        SELECT
                            "Podcasts".podcastname as podcastname,
                            "YouTubeVideos".videotitle as episodetitle,
                            "YouTubeVideos".publishedat as episodepubdate,
                            "YouTubeVideos".videodescription as episodedescription,
                            "YouTubeVideos".videoid as episodeid,
                            CASE
                                WHEN "Podcasts".usepodcastcoverscustomized = TRUE AND "Podcasts".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                WHEN "Users".usepodcastcovers = TRUE THEN "Podcasts".artworkurl
                                ELSE "YouTubeVideos".thumbnailurl
                            END as episodeartwork,
                            "YouTubeVideos".videourl as episodeurl,
                            "YouTubeVideos".duration as episodeduration,
                            "Podcasts".websiteurl as websiteurl,
                            "YouTubeVideos".listenposition as listenduration,
                            "YouTubeVideos".completed as completed,
                            CASE WHEN "SavedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN "EpisodeQueue".episodeid IS NOT NULL AND "EpisodeQueue".is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN "DownloadedVideos".videoid IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube,
                            "Podcasts".podcastid as podcastid,
                            "CollectionEpisodes".addedat as savedate
                        FROM "CollectionEpisodes"
                        INNER JOIN "YouTubeVideos" ON "CollectionEpisodes".videoid = "YouTubeVideos".videoid
                        INNER JOIN "Podcasts" ON "YouTubeVideos".podcastid = "Podcasts".podcastid
                        LEFT JOIN "Users" ON "Podcasts".userid = "Users".userid
                        LEFT JOIN "EpisodeQueue" ON
                            "CollectionEpisodes".videoid = "EpisodeQueue".episodeid
                            AND "EpisodeQueue".userid = $6
                            AND "EpisodeQueue".is_youtube = TRUE
                        LEFT JOIN "DownloadedVideos" ON
                            "CollectionEpisodes".videoid = "DownloadedVideos".videoid
                            AND "DownloadedVideos".userid = $7
                        LEFT JOIN "SavedVideos" ON
                            "CollectionEpisodes".videoid = "SavedVideos".videoid
                            AND "SavedVideos".userid = $8
                        WHERE "CollectionEpisodes".collectionid = $9 AND "CollectionEpisodes".videoid IS NOT NULL
                    ) combined{filter_clause}
                    ORDER BY {order_col} {order_dir} NULLS LAST
                    LIMIT $10 OFFSET $11"#
                );

                let rows = q::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(collection_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(collection_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?;

                let total: i64 = rows.first()
                    .and_then(|r| r.try_get::<i64, _>("total_count").ok())
                    .unwrap_or(0);

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::SavedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
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
                        savedate: row.try_get::<chrono::NaiveDateTime, _>("savedate").ok()
                            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
                    });
                }
                Ok((episodes, total))
            },
            mysql(pool) => {
                let mysql_filter = match filter {
                    "completed"   => " WHERE completed = 1",
                    "in_progress" => " WHERE completed = 0 AND listenduration IS NOT NULL AND listenduration > 0",
                    _             => "",
                };
                let sql = format!(
                    "SELECT *, COUNT(*) OVER() AS total_count FROM (
                        SELECT
                            Podcasts.PodcastName as podcastname,
                            Episodes.EpisodeTitle as episodetitle,
                            Episodes.EpisodePubDate as episodepubdate,
                            Episodes.EpisodeDescription as episodedescription,
                            Episodes.EpisodeID as episodeid,
                            CASE
                                WHEN Podcasts.UsePodcastCoversCustomized = 1 AND Podcasts.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                                WHEN Users.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                                ELSE Episodes.EpisodeArtwork
                            END as episodeartwork,
                            Episodes.EpisodeURL as episodeurl,
                            Episodes.EpisodeDuration as episodeduration,
                            Podcasts.WebsiteURL as websiteurl,
                            UserEpisodeHistory.ListenDuration as listenduration,
                            COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = Episodes.EpisodeID AND ues.UserID = Podcasts.UserID), 0) as completed,
                            CASE WHEN SavedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN DownloadedEpisodes.EpisodeID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            FALSE as is_youtube,
                            Podcasts.PodcastID as podcastid,
                            CollectionEpisodes.AddedAt as savedate
                        FROM CollectionEpisodes
                        INNER JOIN Episodes ON CollectionEpisodes.EpisodeID = Episodes.EpisodeID
                        INNER JOIN Podcasts ON Episodes.FeedID = Podcasts.FeedID
                            AND Podcasts.UserID = (SELECT UserID FROM Collections WHERE CollectionID = CollectionEpisodes.CollectionID)
                        LEFT JOIN Users ON Podcasts.UserID = Users.UserID
                        LEFT JOIN UserEpisodeHistory ON
                            CollectionEpisodes.EpisodeID = UserEpisodeHistory.EpisodeID
                            AND UserEpisodeHistory.UserID = ?
                        LEFT JOIN EpisodeQueue ON
                            CollectionEpisodes.EpisodeID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = FALSE
                        LEFT JOIN DownloadedEpisodes ON
                            CollectionEpisodes.EpisodeID = DownloadedEpisodes.EpisodeID
                            AND DownloadedEpisodes.UserID = ?
                        LEFT JOIN SavedEpisodes ON
                            CollectionEpisodes.EpisodeID = SavedEpisodes.EpisodeID
                            AND SavedEpisodes.UserID = ?
                        WHERE CollectionEpisodes.CollectionID = ? AND CollectionEpisodes.EpisodeID IS NOT NULL

                        UNION ALL

                        SELECT
                            Podcasts.PodcastName as podcastname,
                            YouTubeVideos.VideoTitle as episodetitle,
                            YouTubeVideos.PublishedAt as episodepubdate,
                            YouTubeVideos.VideoDescription as episodedescription,
                            YouTubeVideos.VideoID as episodeid,
                            CASE
                                WHEN Podcasts.UsePodcastCoversCustomized = 1 AND Podcasts.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                                WHEN Users.UsePodcastCovers = 1 THEN Podcasts.ArtworkURL
                                ELSE YouTubeVideos.ThumbnailURL
                            END as episodeartwork,
                            YouTubeVideos.VideoURL as episodeurl,
                            YouTubeVideos.Duration as episodeduration,
                            Podcasts.WebsiteURL as websiteurl,
                            YouTubeVideos.ListenPosition as listenduration,
                            YouTubeVideos.Completed as completed,
                            CASE WHEN SavedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS saved,
                            CASE WHEN EpisodeQueue.EpisodeID IS NOT NULL AND EpisodeQueue.is_youtube = TRUE THEN TRUE ELSE FALSE END AS queued,
                            CASE WHEN DownloadedVideos.VideoID IS NOT NULL THEN TRUE ELSE FALSE END AS downloaded,
                            TRUE as is_youtube,
                            Podcasts.PodcastID as podcastid,
                            CollectionEpisodes.AddedAt as savedate
                        FROM CollectionEpisodes
                        INNER JOIN YouTubeVideos ON CollectionEpisodes.VideoID = YouTubeVideos.VideoID
                        INNER JOIN Podcasts ON YouTubeVideos.PodcastID = Podcasts.PodcastID
                        LEFT JOIN Users ON Podcasts.UserID = Users.UserID
                        LEFT JOIN EpisodeQueue ON
                            CollectionEpisodes.VideoID = EpisodeQueue.EpisodeID
                            AND EpisodeQueue.UserID = ?
                            AND EpisodeQueue.is_youtube = TRUE
                        LEFT JOIN DownloadedVideos ON
                            CollectionEpisodes.VideoID = DownloadedVideos.VideoID
                            AND DownloadedVideos.UserID = ?
                        LEFT JOIN SavedVideos ON
                            CollectionEpisodes.VideoID = SavedVideos.VideoID
                            AND SavedVideos.UserID = ?
                        WHERE CollectionEpisodes.CollectionID = ? AND CollectionEpisodes.VideoID IS NOT NULL
                    ) combined{mysql_filter}
                    ORDER BY {order_col} {order_dir}
                    LIMIT ? OFFSET ?"
                );

                let rows = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(collection_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(collection_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?;

                let total: i64 = rows.first()
                    .and_then(|r| r.try_get::<i64, _>("total_count").ok())
                    .unwrap_or(0);

                let mut episodes = Vec::new();
                for row in rows {
                    episodes.push(crate::models::SavedEpisode {
                        episodetitle: row.try_get("episodetitle")?,
                        podcastname: row.try_get("podcastname")?,
                        episodepubdate: {
                            let naive = row.try_get::<chrono::NaiveDateTime, _>("episodepubdate")?;
                            naive.format("%Y-%m-%dT%H:%M:%S").to_string()
                        },
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
                        savedate: row.try_get::<chrono::NaiveDateTime, _>("savedate").ok()
                            .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string()),
                    });
                }
                Ok((episodes, total))
            },
        })
    }

    // Get the user's collection_add_ui preference ('modal' or 'submenu')
    pub async fn get_collection_add_ui(&self, user_id: i32) -> AppResult<String> {
        let mode = dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT collection_add_ui FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                row.and_then(|r| r.try_get::<Option<String>, _>("collection_add_ui").ok().flatten())
                    .unwrap_or_else(|| "modal".to_string())
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT CollectionAddUI FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                row.and_then(|r| r.try_get::<Option<String>, _>("CollectionAddUI").ok().flatten())
                    .unwrap_or_else(|| "modal".to_string())
            },
        });
        Ok(mode)
    }

    // Set the user's collection_add_ui preference
    pub async fn set_collection_add_ui(&self, user_id: i32, mode: &str) -> AppResult<bool> {
        let mode = if mode == "submenu" { "submenu" } else { "modal" };
        let success = dialect_match!(self, {
            pg(pool) => {
                let existing = q::query(r#"SELECT COUNT(*) as count FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                let count: i64 = existing.try_get("count")?;
                if count > 0 {
                    let result = q::query(r#"UPDATE "UserSettings" SET collection_add_ui = $2 WHERE userid = $1"#)
                        .bind(user_id)
                        .bind(mode)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                } else {
                    let result = q::query(r#"INSERT INTO "UserSettings" (userid, collection_add_ui, theme) VALUES ($1, $2, 'Nordic')"#)
                        .bind(user_id)
                        .bind(mode)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                }
            },
            mysql(pool) => {
                let existing = sqlx::query("SELECT COUNT(*) as count FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;
                let count: i64 = existing.try_get("count")?;
                if count > 0 {
                    let result = sqlx::query("UPDATE UserSettings SET CollectionAddUI = ? WHERE UserID = ?")
                        .bind(mode)
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                } else {
                    let result = sqlx::query("INSERT INTO UserSettings (UserID, CollectionAddUI, Theme) VALUES (?, ?, 'Nordic')")
                        .bind(user_id)
                        .bind(mode)
                        .execute(pool)
                        .await?;
                    result.rows_affected() > 0
                }
            },
        });
        Ok(success)
    }
}
