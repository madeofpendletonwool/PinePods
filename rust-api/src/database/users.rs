//! User accounts and admin management.

use super::*;

#[derive(Debug, Clone)]
pub struct UserCredentials {
    pub user_id: i32,
    pub username: String,
    pub hashed_password: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserSettings {
    pub user_id: i32,
    pub api_key: String,
    pub theme: String,
    pub auto_download_episodes: bool,
    pub auto_delete_episodes: bool,
}

#[derive(Debug, Clone)]
pub struct SelfServiceStatus {
    pub status: bool,
    pub admin_exists: bool,
}

impl DatabasePool {
    // Get user details by ID - matches Python get_user_details_id function
    pub async fn get_user_details_by_id(&self, user_id: i32) -> AppResult<crate::handlers::auth::UserDetails> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT userid, fullname, username, email, usegravatar FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;

                // The password hash never leaves the server; the fields stay for response compatibility.
                Ok(crate::handlers::auth::UserDetails {
                    UserID: row.try_get("userid")?,
                    Fullname: row.try_get("fullname").ok(),
                    Username: row.try_get("username").ok(),
                    Email: row.try_get("email").ok(),
                    Hashed_PW: None,
                    Salt: None,
                    UseGravatar: row.try_get("usegravatar")?,
                })
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT UserID, Fullname, Username, Email, UseGravatar FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?;

                Ok(crate::handlers::auth::UserDetails {
                    UserID: row.try_get("UserID")?,
                    Fullname: row.try_get("Fullname").ok(),
                    Username: row.try_get("Username").ok(),
                    Email: row.try_get("Email").ok(),
                    Hashed_PW: None,
                    Salt: None,
                    UseGravatar: row.try_get::<bool, _>("UseGravatar")?,
                })
            },
        })
    }

    pub async fn get_user_by_credentials(&self, username: &str) -> AppResult<Option<UserCredentials>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT userid as user_id, username as username, hashed_pw as hashed_password, email as email
                     FROM "Users" WHERE username = $1"#
                )
                .bind(username)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(UserCredentials {
                        user_id: row.try_get("user_id")?,
                        username: row.try_get("username")?,
                        hashed_password: row.try_get("hashed_password")?,
                        email: row.try_get("email")?,
                    }))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query(
                    "SELECT UserID as user_id, Username as username, Hashed_PW as hashed_password, Email as email
                     FROM Users WHERE Username = ?"
                )
                .bind(username)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(UserCredentials {
                        user_id: row.try_get("user_id")?,
                        username: row.try_get("username")?,
                        hashed_password: row.try_get("hashed_password")?,
                        email: row.try_get("email")?,
                    }))
                } else {
                    Ok(None)
                }
            },
        })
    }

    pub async fn get_user_settings_by_user_id(&self, user_id: i32) -> AppResult<Option<UserSettings>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT us.userid, ak.apikey as api_key, us.theme, 
                              COALESCE(us.auto_download_episodes, false) as auto_download_episodes,
                              COALESCE(us.auto_delete_episodes, false) as auto_delete_episodes
                       FROM "UserSettings" us 
                       LEFT JOIN "APIKeys" ak ON us.userid = ak.userid
                       WHERE us.userid = $1"#
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(UserSettings {
                        user_id: row.try_get("userid")?,
                        api_key: row.try_get("api_key")?,
                        theme: row.try_get("theme")?,
                        auto_download_episodes: row.try_get("auto_download_episodes")?,
                        auto_delete_episodes: row.try_get("auto_delete_episodes")?,
                    }))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query(
                    "SELECT user_id, api_key, theme, auto_download_episodes, auto_delete_episodes
                     FROM UserSettings WHERE user_id = ?"
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?;
                
                if let Some(row) = row {
                    Ok(Some(UserSettings {
                        user_id: row.try_get("user_id")?,
                        api_key: row.try_get("api_key")?,
                        theme: row.try_get("theme")?,
                        auto_download_episodes: row.try_get("auto_download_episodes")?,
                        auto_delete_episodes: row.try_get("auto_delete_episodes")?,
                    }))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get self-service status - matches Python self_service_status function
    pub async fn get_self_service_status(&self) -> AppResult<SelfServiceStatus> {
        dialect_match!(self, {
            pg(pool) => {
                // Get self-service status
                let self_service_row = q::query(r#"SELECT selfserviceuser FROM "AppSettings" WHERE selfserviceuser = true"#)
                    .fetch_optional(pool)
                    .await?;
                
                let self_service_enabled = self_service_row.is_some();
                
                // Check if admin exists (excluding background_tasks user)
                let admin_row = q::query(r#"SELECT COUNT(*) as count FROM "Users" WHERE isadmin = true AND username != 'background_tasks'"#)
                    .fetch_one(pool)
                    .await?;
                
                let admin_count: i64 = admin_row.try_get("count")?;
                let admin_exists = admin_count > 0;
                
                Ok(SelfServiceStatus {
                    status: self_service_enabled,
                    admin_exists,
                })
            },
            mysql(pool) => {
                // Get self-service status
                let self_service_row = sqlx::query("SELECT SelfServiceUser FROM AppSettings WHERE SelfServiceUser = 1")
                    .fetch_optional(pool)
                    .await?;
                
                let self_service_enabled = self_service_row.is_some();
                
                // Check if admin exists (excluding background_tasks user)
                let admin_row = sqlx::query("SELECT COUNT(*) as count FROM Users WHERE IsAdmin = 1 AND Username != 'background_tasks'")
                    .fetch_one(pool)
                    .await?;
                
                let admin_count: i64 = admin_row.try_get("count")?;
                let admin_exists = admin_count > 0;
                
                Ok(SelfServiceStatus {
                    status: self_service_enabled,
                    admin_exists,
                })
            },
        })
    }

    // Add admin user - matches Python add_admin_user function
    pub async fn add_admin_user(&self, fullname: &str, username: &str, email: &str, hashed_password: &str) -> AppResult<i32> {
        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                
                // Insert the admin user
                q::query(
                    r#"INSERT INTO "Users" (fullname, username, email, hashed_pw, isadmin)
                       VALUES ($1, $2, $3, $4, true)
                       ON CONFLICT (username) DO NOTHING"#
                )
                .bind(fullname)
                .bind(username)
                .bind(email)
                .bind(hashed_password)
                .execute(&mut *tx)
                .await?;
                let user_row = q::query(r#"SELECT userid FROM "Users" WHERE username = $1"#)
                    .bind(username)
                    .fetch_one(&mut *tx)
                    .await?;
                
                let user_id: i32 = user_row.try_get("userid")?;
                
                // Add user settings
                q::query(
                    r#"INSERT INTO "UserSettings" (userid, theme) VALUES ($1, $2)
                       ON CONFLICT (userid) DO NOTHING"#
                )
                .bind(user_id)
                .bind("Nordic")
                .execute(&mut *tx)
                .await?;
                
                // Add user stats
                q::query(
                    r#"INSERT INTO "UserStats" (userid) VALUES ($1)
                       ON CONFLICT (userid) DO NOTHING"#
                )
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
                
                // Create API key for the user
                let api_key = self.generate_api_key();
                q::query(
                    r#"INSERT INTO "APIKeys" (userid, apikey) VALUES ($1, $2)"#
                )
                .bind(user_id)
                .bind(&api_key)
                .execute(&mut *tx)
                .await?;
                
                tx.commit().await?;
                Ok(user_id)
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;
                
                // Insert the admin user
                let result = sqlx::query(
                    "INSERT INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin) VALUES (?, ?, ?, ?, 1)"
                )
                .bind(fullname)
                .bind(username)
                .bind(email)
                .bind(hashed_password)
                .execute(&mut *tx)
                .await?;
                
                let user_id = result.last_insert_id() as i32;
                
                // Add user settings
                sqlx::query(
                    "INSERT INTO UserSettings (UserID, Theme) VALUES (?, ?)"
                )
                .bind(user_id)
                .bind("Nordic")
                .execute(&mut *tx)
                .await?;
                
                // Add user stats
                sqlx::query(
                    "INSERT INTO UserStats (UserID) VALUES (?)"
                )
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
                
                // Create API key for the user
                let api_key = self.generate_api_key();
                sqlx::query(
                    "INSERT INTO APIKeys (UserID, APIKey) VALUES (?, ?)"
                )
                .bind(user_id)
                .bind(&api_key)
                .execute(&mut *tx)
                .await?;
                
                tx.commit().await?;
                Ok(user_id)
            },
        })
    }

    // Check if admin exists - matches Python check_admin_exists function
    pub async fn check_admin_exists(&self) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT COUNT(*) as count FROM "Users" WHERE isadmin = true AND username != 'background_tasks'"#)
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = row.try_get("count")?;
                Ok(count > 0)
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT COUNT(*) as count FROM Users WHERE IsAdmin = 1 AND Username != 'background_tasks'")
                    .fetch_one(pool)
                    .await?;
                
                let count: i64 = row.try_get("count")?;
                Ok(count > 0)
            },
        })
    }

    // First login done - matches Python first_login_done function
    pub async fn first_login_done(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT firstlogin FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("firstlogin").unwrap_or(false)),
                    None => Err(AppError::not_found("User not found")),
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT FirstLogin FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("FirstLogin").unwrap_or(false)),
                    None => Err(AppError::not_found("User not found")),
                }
            },
        })
    }

    // Get all users info - matches Python get_user_info function exactly
    pub async fn get_user_info(&self) -> AppResult<Vec<crate::handlers::settings::UserInfo>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT userid, fullname, username, email, CASE WHEN isadmin THEN true ELSE false END AS isadmin FROM "Users""#
                )
                .fetch_all(pool)
                .await?;

                let mut users = Vec::new();
                for row in rows {
                    users.push(crate::handlers::settings::UserInfo {
                        userid: row.try_get("userid")?,
                        fullname: row.try_get("fullname")?,
                        username: row.try_get("username")?,
                        email: row.try_get("email")?,
                        isadmin: row.try_get("isadmin")?,
                    });
                }
                Ok(users)
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT UserID as userid, Fullname as fullname, Username as username, Email as email, IsAdmin as isadmin FROM Users"
                )
                .fetch_all(pool)
                .await?;

                let mut users = Vec::new();
                for row in rows {
                    users.push(crate::handlers::settings::UserInfo {
                        userid: row.try_get("userid")?,
                        fullname: row.try_get("fullname")?,
                        username: row.try_get("username")?,
                        email: row.try_get("email")?,
                        isadmin: row.try_get("isadmin")?,
                    });
                }
                Ok(users)
            },
        })
    }

    // Get specific user info - matches Python get_my_user_info function exactly
    pub async fn get_my_user_info(&self, user_id: i32) -> AppResult<Option<serde_json::Value>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"SELECT userid, fullname, username, email, CASE WHEN isadmin THEN 1 ELSE 0 END AS isadmin, timezone, timeformat, dateformat, language FROM "Users" WHERE userid = $1"#
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    Ok(Some(serde_json::json!({
                        "userid": row.try_get::<i32, _>("userid")?,
                        "fullname": row.try_get::<String, _>("fullname")?,
                        "username": row.try_get::<String, _>("username")?,
                        "email": row.try_get::<String, _>("email")?,
                        "isadmin": row.try_get::<i32, _>("isadmin")?,
                        "timezone": row.try_get::<String, _>("timezone")?,
                        "timeformat": row.try_get::<i32, _>("timeformat")?,
                        "dateformat": row.try_get::<String, _>("dateformat")?,
                        "language": row.try_get::<String, _>("language")?,
                    })))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query(
                    "SELECT UserID as userid, Fullname as fullname, Username as username, Email as email, IsAdmin as isadmin, TimeZone as timezone, TimeFormat as timeformat, DateFormat as dateformat, Language as language FROM Users WHERE UserID = ?"
                )
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    Ok(Some(serde_json::json!({
                        "userid": row.try_get::<i32, _>("userid")?,
                        "fullname": row.try_get::<String, _>("fullname")?,
                        "username": row.try_get::<String, _>("username")?,
                        "email": row.try_get::<String, _>("email")?,
                        "isadmin": row.try_get::<i32, _>("isadmin")?,
                        "timezone": row.try_get::<String, _>("timezone")?,
                        "timeformat": row.try_get::<i32, _>("timeformat")?,
                        "dateformat": row.try_get::<String, _>("dateformat")?,
                        "language": row.try_get::<String, _>("language")?,
                    })))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Add user - matches Python add_user function exactly
    pub async fn add_user(&self, fullname: &str, username: &str, email: &str, hashed_pw: &str) -> AppResult<i32> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(
                    r#"INSERT INTO "Users" (fullname, username, email, hashed_pw, isadmin) VALUES ($1, $2, $3, $4, false) RETURNING userid"#
                )
                .bind(fullname)
                .bind(username)
                .bind(email)
                .bind(hashed_pw)
                .fetch_one(pool)
                .await?;

                let user_id: i32 = row.try_get("userid")?;

                // Add user settings like Python version
                q::query(r#"INSERT INTO "UserSettings" (userid, theme) VALUES ($1, $2)"#)
                    .bind(user_id)
                    .bind("Nordic")
                    .execute(pool)
                    .await?;

                // Stats row (the stats page and every stats counter depend on it).
                q::query(r#"INSERT INTO "UserStats" (userid) VALUES ($1) ON CONFLICT (userid) DO NOTHING"#)
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                // Create default playlists for the new user
                if let Err(e) = self.create_default_playlists_for_user(user_id).await {
                    tracing::warn!("⚠️ Failed to create default playlists for new user {}: {}", user_id, e);
                    // Don't fail user creation if playlist creation fails
                }

                Ok(user_id)
            },
            mysql(pool) => {
                let result = sqlx::query(
                    "INSERT INTO Users (Fullname, Username, Email, Hashed_PW, IsAdmin) VALUES (?, ?, ?, ?, 0)"
                )
                .bind(fullname)
                .bind(username)
                .bind(email)
                .bind(hashed_pw)
                .execute(pool)
                .await?;

                let user_id = result.last_insert_id() as i32;

                // Add user settings like Python version
                sqlx::query("INSERT INTO UserSettings (UserID, Theme) VALUES (?, ?)")
                    .bind(user_id)
                    .bind("Nordic")
                    .execute(pool)
                    .await?;

                // Stats row (the stats page and every stats counter depend on it).
                sqlx::query("INSERT IGNORE INTO UserStats (UserID) VALUES (?)")
                    .bind(user_id)
                    .execute(pool)
                    .await?;

                // Create default playlists for the new user
                if let Err(e) = self.create_default_playlists_for_user(user_id).await {
                    tracing::warn!("⚠️ Failed to create default playlists for new user {}: {}", user_id, e);
                    // Don't fail user creation if playlist creation fails
                }

                Ok(user_id)
            },
        })
    }

    // Set fullname - matches Python set_fullname function exactly
    pub async fn set_fullname(&self, user_id: i32, new_name: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET fullname = $1 WHERE userid = $2"#)
                    .bind(new_name)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET Fullname = ? WHERE UserID = ?")
                    .bind(new_name)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set password - matches Python set_password function exactly
    pub async fn set_password(&self, user_id: i32, hash_pw: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET hashed_pw = $1 WHERE userid = $2"#)
                    .bind(hash_pw)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET Hashed_PW = ? WHERE UserID = ?")
                    .bind(hash_pw)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Delete user - matches Python delete_user function exactly
    pub async fn delete_user(&self, user_id: i32) -> AppResult<()> {
        // Podcasts own episodes and videos through NO ACTION foreign keys; reuse the
        // unsubscribe path, which already deletes their dependents in a valid order.
        let podcast_ids: Vec<i32> = dialect_match!(self, {
            pg(pool) => {
                q::query_scalar(r#"SELECT podcastid FROM "Podcasts" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?
            },
            mysql(pool) => {
                sqlx::query_scalar("SELECT PodcastID FROM Podcasts WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?
            },
        });
        for podcast_id in podcast_ids {
            self.remove_podcast_id(podcast_id, user_id, crate::services::sync::SyncOrigin::Unlogged).await?;
        }

        // Tables that reference Users without ON DELETE CASCADE.
        // Everything else cascades from the final DELETE.
        dialect_match!(self, {
            pg(pool) => {
                for table in [
                    "UserEpisodeHistory", "UserVideoHistory", "SavedEpisodes", "SavedVideos",
                    "DownloadedEpisodes", "DownloadedVideos", "EpisodeQueue", "UserSettings", "UserStats",
                ] {
                    q::query(sqlx::AssertSqlSafe(format!(r#"DELETE FROM "{table}" WHERE userid = $1"#)))
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                }
                q::query(r#"DELETE FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                for table in [
                    "UserEpisodeHistory", "UserVideoHistory", "SavedEpisodes", "SavedVideos",
                    "DownloadedEpisodes", "DownloadedVideos", "EpisodeQueue", "UserSettings", "UserStats",
                ] {
                    sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM {table} WHERE UserID = ?")))
                        .bind(user_id)
                        .execute(pool)
                        .await?;
                }
                sqlx::query("DELETE FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set email - matches Python set_email function exactly
    pub async fn set_email(&self, user_id: i32, new_email: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET email = $1 WHERE userid = $2"#)
                    .bind(new_email)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET Email = ? WHERE UserID = ?")
                    .bind(new_email)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set username - matches Python set_username function exactly
    pub async fn set_username(&self, user_id: i32, new_username: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET username = $1 WHERE userid = $2"#)
                    .bind(new_username)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET Username = ? WHERE UserID = ?")
                    .bind(new_username)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Set isadmin - matches Python set_isadmin function exactly
    pub async fn set_isadmin(&self, user_id: i32, isadmin: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET isadmin = $1 WHERE userid = $2"#)
                    .bind(isadmin)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET IsAdmin = ? WHERE UserID = ?")
                    .bind(isadmin as i32)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Final admin check - matches Python final_admin function exactly
    pub async fn final_admin(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                // Count total admins
                let admin_count: i64 = q::query_scalar(r#"SELECT COUNT(*) FROM "Users" WHERE isadmin = TRUE"#)
                    .fetch_one(pool)
                    .await?;

                if admin_count == 1 {
                    // Check if this user is admin
                    let is_admin: Option<bool> = q::query_scalar(r#"SELECT isadmin FROM "Users" WHERE userid = $1"#)
                        .bind(user_id)
                        .fetch_optional(pool)
                        .await?;

                    Ok(is_admin.unwrap_or(false))
                } else {
                    Ok(false)
                }
            },
            mysql(pool) => {
                // Count total admins
                let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Users WHERE IsAdmin = 1")
                    .fetch_one(pool)
                    .await?;

                if admin_count == 1 {
                    // Check if this user is admin
                    let is_admin: Option<i32> = sqlx::query_scalar("SELECT IsAdmin FROM Users WHERE UserID = ?")
                        .bind(user_id)
                        .fetch_optional(pool)
                        .await?;

                    Ok(is_admin.unwrap_or(0) == 1)
                } else {
                    Ok(false)
                }
            },
        })
    }

    // Get self service status - matches Python self_service_status function exactly
    pub async fn self_service_status(&self) -> AppResult<SelfServiceStatus> {
        dialect_match!(self, {
            pg(pool) => {
                // Get self-service status
                let self_service_result: Option<bool> = q::query_scalar(r#"SELECT selfserviceuser FROM "AppSettings" WHERE selfserviceuser = TRUE"#)
                    .fetch_optional(pool)
                    .await?;

                // Get admin count
                let admin_count: i64 = q::query_scalar(r#"SELECT COUNT(*) FROM "Users" WHERE isadmin = TRUE"#)
                    .fetch_one(pool)
                    .await?;

                Ok(SelfServiceStatus {
                    status: self_service_result.unwrap_or(false),
                    admin_exists: admin_count > 0,
                })
            },
            mysql(pool) => {
                // Get self-service status
                let self_service_result: Option<i32> = sqlx::query_scalar("SELECT SelfServiceUser FROM AppSettings WHERE SelfServiceUser = 1")
                    .fetch_optional(pool)
                    .await?;

                // Get admin count
                let admin_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM Users WHERE IsAdmin = 1")
                    .fetch_one(pool)
                    .await?;

                Ok(SelfServiceStatus {
                    status: self_service_result.unwrap_or(0) == 1,
                    admin_exists: admin_count > 0,
                })
            },
        })
    }

    // Get all users with podcasts - for admin refresh
    pub async fn get_all_users_with_podcasts(&self) -> AppResult<Vec<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(r#"SELECT DISTINCT userid FROM "Podcasts" ORDER BY userid"#)
                    .fetch_all(pool)
                    .await?;
                
                let mut users = Vec::new();
                for row in rows {
                    users.push(row.try_get("userid")?);
                }
                Ok(users)
            },
            mysql(pool) => {
                let rows = sqlx::query("SELECT DISTINCT UserID FROM Podcasts ORDER BY UserID")
                    .fetch_all(pool)
                    .await?;
                
                let mut users = Vec::new();
                for row in rows {
                    users.push(row.try_get("UserID")?);
                }
                Ok(users)
            },
        })
    }

    // Get user by email - matches Python get_user_by_email function EXACTLY  
    pub async fn get_user_by_email(&self, email: &str) -> AppResult<Option<(i32, String, Option<String>, Option<String>, bool)>> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"
                    SELECT userid, email, username, fullname, isadmin
                    FROM "Users"
                    WHERE email = $1
                "#)
                    .bind(email)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = result {
                    Ok(Some((
                        row.try_get("userid")?,
                        row.try_get("email")?,
                        row.try_get("username")?,
                        row.try_get("fullname")?,
                        row.try_get("isadmin")?,
                    )))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let result = sqlx::query(r#"
                    SELECT UserID, Email, Username, Fullname, IsAdmin
                    FROM Users
                    WHERE Email = ?
                "#)
                    .bind(email)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = result {
                    let is_admin: i32 = row.try_get("IsAdmin")?;
                    Ok(Some((
                        row.try_get("UserID")?,
                        row.try_get("Email")?,
                        row.try_get("Username")?,
                        row.try_get("Fullname")?,
                        is_admin != 0,
                    )))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Check if username exists - matches Python check_usernames function EXACTLY
    pub async fn check_usernames(&self, username: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"SELECT COUNT(*) as count FROM "Users" WHERE username = $1"#)
                    .bind(username)
                    .fetch_one(pool)
                    .await?;
                let count: i64 = result.try_get("count")?;
                Ok(count > 0)
            },
            mysql(pool) => {
                let result = sqlx::query(r#"SELECT COUNT(*) as count FROM Users WHERE Username = ?"#)
                    .bind(username)
                    .fetch_one(pool)
                    .await?;
                let count: i64 = result.try_get("count")?;
                Ok(count > 0)
            },
        })
    }

    // Check if username exists - helper for OIDC user creation
    pub async fn username_exists(&self, username: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"SELECT 1 FROM "Users" WHERE username = $1 LIMIT 1"#)
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                Ok(result.is_some())
            },
            mysql(pool) => {
                let result = sqlx::query("SELECT 1 FROM Users WHERE Username = ? LIMIT 1")
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                Ok(result.is_some())
            },
        })
    }

    // Get all admin user IDs - matches Python add_news_feed_if_not_added logic
    pub async fn get_all_admin_user_ids(&self) -> AppResult<Vec<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(r#"SELECT userid FROM "Users" WHERE isadmin = TRUE"#)
                    .fetch_all(pool)
                    .await?;
                
                let user_ids: Vec<i32> = rows.into_iter()
                    .map(|row| row.try_get("userid"))
                    .collect::<Result<Vec<_>, _>>()?;
                
                Ok(user_ids)
            },
            mysql(pool) => {
                let rows = sqlx::query("SELECT UserID FROM Users WHERE IsAdmin = 1")
                    .fetch_all(pool)
                    .await?;
                
                let user_ids: Vec<i32> = rows.into_iter()
                    .map(|row| row.try_get("UserID"))
                    .collect::<Result<Vec<_>, _>>()?;
                
                Ok(user_ids)
            },
        })
    }
}
