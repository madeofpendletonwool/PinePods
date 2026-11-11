//! API keys, login checks and password reset codes.

use super::*;

impl DatabasePool {
    // Helper methods for database operations

    // Verify API key - matches Python verify_api_key function
    pub async fn verify_api_key(&self, api_key: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT * FROM "APIKeys" WHERE apikey = $1"#)
                    .bind(api_key)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(row.is_some())
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT * FROM APIKeys WHERE APIKey = ?")
                    .bind(api_key)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(row.is_some())
            },
        })
    }

    // Verify password - matches Python verify_password function
    pub async fn verify_password(&self, username: &str, password: &str) -> AppResult<bool> {
        use crate::services::auth::verify_password;
        
        let stored_hash = dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT hashed_pw FROM "Users" WHERE username = $1"#)
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<String, _>("hashed_pw")?
                } else {
                    return Ok(false);
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT Hashed_PW FROM Users WHERE Username = ?")
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    row.try_get::<String, _>("Hashed_PW")?
                } else {
                    return Ok(false);
                }
            },
        });

        verify_password(password, &stored_hash)
    }

    // Get API key for user - matches Python get_api_key function  
    pub async fn get_api_key(&self, username: &str) -> AppResult<String> {
        dialect_match!(self, {
            pg(pool) => {
                // First get UserID
                let user_row = q::query(r#"SELECT userid FROM "Users" WHERE username = $1"#)
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                let user_id: i32 = match user_row {
                    Some(row) => row.try_get("userid")?,
                    None => return Err(AppError::not_found("User not found")),
                };
                
                // Then get API key
                let api_row = q::query(r#"SELECT apikey FROM "APIKeys" WHERE userid = $1 LIMIT 1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match api_row {
                    Some(row) => Ok(row.try_get("apikey")?),
                    None => Err(AppError::not_found("API key not found for user")),
                }
            },
            mysql(pool) => {
                // First get UserID
                let user_row = sqlx::query("SELECT UserID FROM Users WHERE Username = ?")
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                let user_id: i32 = match user_row {
                    Some(row) => row.try_get("UserID")?,
                    None => return Err(AppError::not_found("User not found")),
                };
                
                // Then get API key
                let api_row = sqlx::query("SELECT APIKey FROM APIKeys WHERE UserID = ? LIMIT 1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                match api_row {
                    Some(row) => Ok(row.try_get("APIKey")?),
                    None => Err(AppError::not_found("API key not found for user")),
                }
            },
        })
    }

    // Get user ID from API key - matches Python get_api_user function
    /// Owner of `api_key`. An unknown key is an auth failure (401), not a database error.
    pub async fn get_user_id_from_api_key(&self, api_key: &str) -> AppResult<i32> {
        let user_id = dialect_match!(self, {
            pg(pool) => {
                q::query(r#"SELECT userid FROM "APIKeys" WHERE apikey = $1 LIMIT 1"#)
                    .bind(api_key)
                    .fetch_optional(pool)
                    .await?
                    .map(|row| row.try_get("userid"))
                    .transpose()?
            },
            mysql(pool) => {
                sqlx::query("SELECT UserID FROM APIKeys WHERE APIKey = ? LIMIT 1")
                    .bind(api_key)
                    .fetch_optional(pool)
                    .await?
                    .map(|row| row.try_get("UserID"))
                    .transpose()?
            },
        });
        user_id.ok_or_else(|| AppError::unauthorized("Invalid API key"))
    }

    // Get user ID from username - for login and key creation
    pub async fn get_user_id_from_username(&self, username: &str) -> AppResult<i32> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT userid FROM "Users" WHERE username = $1"#)
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("userid")?),
                    None => Err(AppError::not_found("User not found")),
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT UserID FROM Users WHERE Username = ?")
                    .bind(username)
                    .fetch_optional(pool)
                    .await?;
                
                match row {
                    Some(row) => Ok(row.try_get("UserID")?),
                    None => Err(AppError::not_found("User not found")),
                }
            },
        })
    }

    // Get user ID by API key - matches Python get_api_user function
    pub async fn get_api_user(&self, api_key: &str) -> AppResult<i32> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT userid FROM "APIKeys" WHERE apikey = $1"#)
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("userid")?)
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT UserID FROM APIKeys WHERE APIKey = ?")
                    .bind(api_key)
                    .fetch_one(pool)
                    .await?;
                
                Ok(row.try_get("UserID")?)
            },
        })
    }

    // Generate API key - matches Python create_api_key function
    pub(super) fn generate_api_key(&self) -> String {
        use rand::RngExt;
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::rng();
        (0..64)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    // User admin check - matches Python user_admin_check function
    pub async fn user_admin_check(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT isadmin FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(row.try_get("isadmin")?)
                } else {
                    Ok(false)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT IsAdmin FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let is_admin: i8 = row.try_get("IsAdmin")?;
                    Ok(is_admin != 0)
                } else {
                    Ok(false)
                }
            },
        })
    }

    // Legacy web key check - no longer used but kept for compatibility
    // All authentication now uses User ID 1 (background_tasks) for system operations
    pub async fn is_web_key(&self, _api_key: &str) -> AppResult<bool> {
        // Always return false - web keys are deprecated in favor of background_tasks user (ID 1)
        Ok(false)
    }

    // Get API info - matches Python get_api_info function exactly
    pub async fn get_api_info(&self, user_id: i32) -> AppResult<Option<Vec<crate::handlers::settings::ApiInfo>>> {
        dialect_match!(self, {
            pg(pool) => {
                // Check if user is admin
                let is_admin: Option<bool> = q::query_scalar(r#"SELECT isadmin FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                let is_admin = is_admin.unwrap_or(false);

                let query = if is_admin {
                    // Admin sees all API keys
                    r#"SELECT a.apikeyid, a.userid, u.username, RIGHT(a.apikey, 4) as lastfourdigits, 
                       a.created::text as created
                       FROM "APIKeys" a
                       JOIN "Users" u ON a.userid = u.userid"#
                } else {
                    // Non-admin sees only their own API keys
                    r#"SELECT a.apikeyid, a.userid, u.username, RIGHT(a.apikey, 4) as lastfourdigits,
                       a.created::text as created
                       FROM "APIKeys" a
                       JOIN "Users" u ON a.userid = u.userid
                       WHERE a.userid = $1"#
                };

                let rows = if is_admin {
                    q::query(query).fetch_all(pool).await?
                } else {
                    q::query(query).bind(user_id).fetch_all(pool).await?
                };

                let mut api_infos = Vec::new();
                for row in rows {
                    api_infos.push(crate::handlers::settings::ApiInfo {
                        apikeyid: row.try_get("apikeyid")?,
                        userid: row.try_get("userid")?,
                        username: row.try_get("username")?,
                        lastfourdigits: row.try_get("lastfourdigits")?,
                        created: row.try_get("created")?,
                        podcastids: vec![], // Empty array as in Python
                    });
                }

                if api_infos.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(api_infos))
                }
            },
            mysql(pool) => {
                // Check if user is admin
                let is_admin: Option<i32> = sqlx::query_scalar("SELECT IsAdmin FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                let is_admin = is_admin.unwrap_or(0) == 1;

                let query = if is_admin {
                    // Admin sees all API keys
                    "SELECT a.APIKeyID as apikeyid, a.UserID as userid, u.Username as username, RIGHT(a.APIKey, 4) as lastfourdigits,
                     a.Created as created
                     FROM APIKeys a
                     JOIN Users u ON a.UserID = u.UserID"
                } else {
                    // Non-admin sees only their own API keys  
                    "SELECT a.APIKeyID as apikeyid, a.UserID as userid, u.Username as username, RIGHT(a.APIKey, 4) as lastfourdigits,
                     a.Created as created
                     FROM APIKeys a
                     JOIN Users u ON a.UserID = u.UserID
                     WHERE a.UserID = ?"
                };

                let rows = if is_admin {
                    sqlx::query(query).fetch_all(pool).await?
                } else {
                    sqlx::query(query).bind(user_id).fetch_all(pool).await?
                };

                let mut api_infos = Vec::new();
                for row in rows {
                    api_infos.push(crate::handlers::settings::ApiInfo {
                        apikeyid: row.try_get("apikeyid")?,
                        userid: row.try_get("userid")?,
                        username: row.try_get("username")?,
                        lastfourdigits: row.try_get("lastfourdigits")?,
                        created: row.try_get::<chrono::DateTime<chrono::Utc>, _>("created")?.to_string(),
                        podcastids: vec![], // Empty array as in Python
                    });
                }

                if api_infos.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(api_infos))
                }
            },
        })
    }

    // Create API key - matches Python create_api_key function exactly
    pub async fn create_api_key(&self, user_id: i32) -> AppResult<String> {
        use rand::RngExt;
        
        // Generate 64-character API key
        let charset: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                abcdefghijklmnopqrstuvwxyz\
                                0123456789";
        let api_key: String = {
            let mut rng = rand::rng();
            (0..64)
                .map(|_| {
                    let idx = rng.random_range(0..charset.len());
                    charset[idx] as char
                })
                .collect()
        };

        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"INSERT INTO "APIKeys" (userid, apikey) VALUES ($1, $2)"#)
                    .bind(user_id)
                    .bind(&api_key)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("INSERT INTO APIKeys (UserID, APIKey) VALUES (?, ?)")
                    .bind(user_id)
                    .bind(&api_key)
                    .execute(pool)
                    .await?;
            },
        });

        Ok(api_key)
    }

    // Count user API keys excluding a specific one - safety check for final API key
    pub async fn count_user_api_keys_excluding(&self, user_id: i32, exclude_api_id: i32) -> AppResult<i32> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT COUNT(*) as count FROM "APIKeys" WHERE userid = $1 AND apikeyid != $2"#)
                    .bind(user_id)
                    .bind(exclude_api_id)
                    .fetch_one(pool)
                    .await?;
                Ok(row.try_get::<i64, _>("count")? as i32)
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT COUNT(*) as count FROM APIKeys WHERE UserID = ? AND APIKeyID != ?")
                    .bind(user_id)
                    .bind(exclude_api_id)
                    .fetch_one(pool)
                    .await?;
                Ok(row.try_get::<i64, _>("count")? as i32)
            },
        })
    }

    // Delete API key - matches Python delete_api function exactly
    pub async fn delete_api_key(&self, api_id: i32) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"DELETE FROM "APIKeys" WHERE apikeyid = $1"#)
                    .bind(api_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("DELETE FROM APIKeys WHERE APIKeyID = ?")
                    .bind(api_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Check if API key is the same as the one being deleted - matches Python is_same_api_key function exactly
    pub async fn is_same_api_key(&self, api_id: i32, api_key: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result: Option<String> = q::query_scalar(r#"SELECT apikey FROM "APIKeys" WHERE apikeyid = $1"#)
                    .bind(api_id)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(result.map(|key| key == api_key).unwrap_or(false))
            },
            mysql(pool) => {
                let result: Option<String> = sqlx::query_scalar("SELECT APIKey FROM APIKeys WHERE APIKeyID = ?")
                    .bind(api_id)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(result.map(|key| key == api_key).unwrap_or(false))
            },
        })
    }

    // Get the owner user ID of an API key by API key ID - for authorization checks
    pub async fn get_api_key_owner(&self, api_id: i32) -> AppResult<Option<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let result: Option<i32> = q::query_scalar(r#"SELECT userid FROM "APIKeys" WHERE apikeyid = $1"#)
                    .bind(api_id)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(result)
            },
            mysql(pool) => {
                let result: Option<i32> = sqlx::query_scalar("SELECT UserID FROM APIKeys WHERE APIKeyID = ?")
                    .bind(api_id)
                    .fetch_optional(pool)
                    .await?;
                
                Ok(result)
            },
        })
    }

    // Get user API key - matches Python get_user_api_key function EXACTLY
    pub async fn get_user_api_key(&self, user_id: i32) -> AppResult<Option<String>> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"
                    SELECT apikey FROM "APIKeys" 
                    WHERE userid = $1 
                    ORDER BY created DESC 
                    LIMIT 1
                "#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = result {
                    Ok(Some(row.try_get("apikey")?))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let result = sqlx::query(r#"
                    SELECT APIKey FROM APIKeys 
                    WHERE UserID = ? 
                    ORDER BY Created DESC 
                    LIMIT 1
                "#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = result {
                    Ok(Some(row.try_get("APIKey")?))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Create or get API key for user - for OIDC login completion
    pub async fn create_or_get_api_key(&self, user_id: i32) -> AppResult<String> {
        dialect_match!(self, {
            pg(pool) => {
                // Check for existing API key
                let existing_key = q::query(r#"SELECT apikey FROM "APIKeys" WHERE userid = $1 LIMIT 1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = existing_key {
                    return Ok(row.try_get("apikey")?);
                }

                // Generate new API key
                let api_key = format!("pk_{}", uuid::Uuid::new_v4().simple());
                
                q::query(r#"INSERT INTO "APIKeys" (userid, apikey) VALUES ($1, $2)"#)
                    .bind(user_id)
                    .bind(&api_key)
                    .execute(pool)
                    .await?;

                Ok(api_key)
            },
            mysql(pool) => {
                let existing_key = sqlx::query("SELECT APIKey FROM APIKeys WHERE UserID = ? LIMIT 1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;

                if let Some(row) = existing_key {
                    return Ok(row.try_get("APIKey")?);
                }

                let api_key = format!("pk_{}", uuid::Uuid::new_v4().simple());
                
                sqlx::query("INSERT INTO APIKeys (UserID, APIKey) VALUES (?, ?)")
                    .bind(user_id)
                    .bind(&api_key)
                    .execute(pool)
                    .await?;

                Ok(api_key)
            },
        })
    }

    // Check if a user exists with the given username and email
    pub async fn check_reset_user(&self, username: &str, email: &str) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"SELECT userid FROM "Users" WHERE username = $1 AND email = $2"#)
                    .bind(username)
                    .bind(email)
                    .fetch_optional(pool)
                    .await?;
                Ok(result.is_some())
            },
            mysql(pool) => {
                let result = sqlx::query("SELECT UserID FROM Users WHERE Username = ? AND Email = ?")
                    .bind(username)
                    .bind(email)
                    .fetch_optional(pool)
                    .await?;
                Ok(result.is_some())
            },
        })
    }

    // Create a password reset code for the user
    pub async fn reset_password_create_code(&self, user_email: &str) -> AppResult<Option<String>> {
        use rand::RngExt;
        use chrono::{Utc, Duration};
        
        // Generate 6-character reset code with uppercase letters and digits
        let reset_code: String = (0..6)
            .map(|_| {
                let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
                chars[rand::rng().random_range(0..chars.len())] as char
            })
            .collect();

        let reset_expiry: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc> = (Utc::now() + Duration::hours(1)).into();

        dialect_match!(self, {
            pg(pool) => {
                // Check if user exists first
                let user_exists = q::query(r#"SELECT userid FROM "Users" WHERE email = $1"#)
                    .bind(user_email)
                    .fetch_optional(pool)
                    .await?;
                
                if user_exists.is_none() {
                    return Ok(None);
                }

                // Update reset code and expiry
                let result = q::query(r#"
                    UPDATE "Users"
                    SET reset_code = $1, reset_expiry = $2
                    WHERE email = $3
                "#)
                    .bind(&reset_code)
                    .bind(reset_expiry)
                    .bind(user_email)
                    .execute(pool)
                    .await?;

                if result.rows_affected() > 0 {
                    Ok(Some(reset_code))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                // Check if user exists first
                let user_exists = sqlx::query("SELECT UserID FROM Users WHERE Email = ?")
                    .bind(user_email)
                    .fetch_optional(pool)
                    .await?;
                
                if user_exists.is_none() {
                    return Ok(None);
                }

                // Update reset code and expiry
                let result = sqlx::query(r#"
                    UPDATE Users
                    SET Reset_Code = ?, Reset_Expiry = ?
                    WHERE Email = ?
                "#)
                    .bind(&reset_code)
                    .bind(reset_expiry)
                    .bind(user_email)
                    .execute(pool)
                    .await?;

                if result.rows_affected() > 0 {
                    Ok(Some(reset_code))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Remove reset code from user (used when email sending fails)
    pub async fn reset_password_remove_code(&self, email: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET reset_code = NULL, reset_expiry = NULL WHERE email = $1"#)
                    .bind(email)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET Reset_Code = NULL, Reset_Expiry = NULL WHERE Email = ?")
                    .bind(email)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Verify reset code is valid and not expired
    pub async fn verify_reset_code(&self, user_email: &str, reset_code: &str) -> AppResult<Option<bool>> {
        use chrono::Utc;
        
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"SELECT reset_code, reset_expiry FROM "Users" WHERE email = $1"#)
                    .bind(user_email)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = result {
                    let stored_code: Option<String> = row.try_get("reset_code").ok();
                    let expiry: Option<chrono::NaiveDateTime> = row.try_get("reset_expiry").ok();
                    
                    if let (Some(stored_code), Some(expiry)) = (stored_code.clone(), expiry.clone()) {
                        // Convert NaiveDateTime to UTC for comparison
                        let expiry_utc = expiry.and_utc();
                        let is_valid = stored_code == reset_code && Utc::now() < expiry_utc;
                        Ok(Some(is_valid))
                    } else {
                        Ok(Some(false)) // No reset code set
                    }
                } else {
                    Ok(None) // User not found
                }
            },
            mysql(pool) => {
                let result = sqlx::query("SELECT Reset_Code, Reset_Expiry FROM Users WHERE Email = ?")
                    .bind(user_email)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = result {
                    let stored_code: Option<String> = row.try_get("Reset_Code").ok();
                    let expiry: Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>> = row.try_get("Reset_Expiry").ok();
                    
                    if let (Some(stored_code), Some(expiry)) = (stored_code, expiry) {
                        Ok(Some(stored_code == reset_code && Utc::now() < expiry))
                    } else {
                        Ok(Some(false)) // No reset code set
                    }
                } else {
                    Ok(None) // User not found
                }
            },
        })
    }

    // Reset password and clear reset code/expiry
    pub async fn reset_password_prompt(&self, user_email: &str, hashed_password: &str) -> AppResult<Option<String>> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"
                    UPDATE "Users"
                    SET hashed_pw = $1, reset_code = NULL, reset_expiry = NULL
                    WHERE email = $2
                "#)
                    .bind(hashed_password)
                    .bind(user_email)
                    .execute(pool)
                    .await?;
                
                if result.rows_affected() > 0 {
                    Ok(Some("Password Reset Successfully".to_string()))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let result = sqlx::query(r#"
                    UPDATE Users
                    SET Hashed_PW = ?, Reset_Code = NULL, Reset_Expiry = NULL
                    WHERE Email = ?
                "#)
                    .bind(hashed_password)
                    .bind(user_email)
                    .execute(pool)
                    .await?;
                
                if result.rows_affected() > 0 {
                    Ok(Some("Password Reset Successfully".to_string()))
                } else {
                    Ok(None)
                }
            },
        })
    }
}
