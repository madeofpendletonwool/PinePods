//! gpodder / Nextcloud sync settings.

use super::*;

// Nextcloud login data structure
#[derive(Debug, Clone)]
pub struct NextcloudLoginData {
    pub raw_response: serde_json::Value,
}

// gPodder status structure - matches Python get_user_gpodder_status response
#[derive(Debug, Clone)]
pub struct GpodderStatus {
    pub sync_type: String,
    pub gpodder_url: Option<String>,
}

impl DatabasePool {
    // Initiate Nextcloud login - matches Python initiate_nextcloud_login function exactly
    pub async fn initiate_nextcloud_login(&self, _user_id: i32, nextcloud_url: &str) -> AppResult<NextcloudLoginData> {
        let client = reqwest::Client::new();
        
        // Call Nextcloud login flow v2 API
        let login_url = format!("{}/index.php/login/v2", nextcloud_url.trim_end_matches('/'));
        
        let response = client
            .post(&login_url)
            .send()
            .await
            .map_err(|e| AppError::internal(&format!("Failed to initiate Nextcloud login: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(AppError::internal("Nextcloud login initiation failed"));
        }
        
        let json: serde_json::Value = response.json().await
            .map_err(|e| AppError::internal(&format!("Failed to parse Nextcloud response: {}", e)))?;
        
        // Return the raw JSON response from Nextcloud to match Python behavior
        Ok(NextcloudLoginData {
            raw_response: json,
        })
    }

    // Save Nextcloud credentials (from the login flow) as the user's sync target. The target save
    // keeps the legacy Users columns in step.
    pub async fn save_nextcloud_credentials(&self, user_id: i32, nextcloud_url: &str, app_password: &str, login_name: &str) -> AppResult<()> {
        self.sync_target_save(user_id, crate::services::sync::engine_db::TargetKind::Nextcloud, nextcloud_url, login_name, app_password).await?;
        Ok(())
    }

    // Save an external gpodder server as the user's sync target (replacing any other). The caller
    // starts the first (full) sync. Pinepods' device on that server is chosen by the sync engine,
    // never another client's.
    pub async fn add_gpodder_server(&self, user_id: i32, gpodder_url: &str, username: &str, password: &str) -> AppResult<bool> {
        self.sync_target_save(user_id, crate::services::sync::engine_db::TargetKind::Gpodder, gpodder_url, username, password).await?;
        Ok(true)
    }

    // Encrypt password using Fernet - matches Python encryption
    pub async fn encrypt_password(&self, password: &str) -> AppResult<String> {
        use fernet::Fernet;
        
        // Get encryption key from app settings (base64 string)
        let encryption_key = self.get_encryption_key().await?;
        let fernet = Fernet::new(&encryption_key)
            .ok_or_else(|| AppError::internal("Failed to create Fernet cipher"))?;
        
        let encrypted = fernet.encrypt(password.as_bytes());
        Ok(encrypted)
    }

    // Get encryption key from app settings
    pub(crate) async fn get_encryption_key(&self) -> AppResult<String> {
        dialect_match!(self, {
            pg(pool) => {
                // Try as string first (new format), fallback to bytes (old format)
                let row = q::query(r#"SELECT encryptionkey FROM "AppSettings" LIMIT 1"#)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    // Try string first
                    if let Ok(key_string) = row.try_get::<String, _>("encryptionkey") {
                        return Ok(key_string);
                    }
                    // Fallback to bytes and convert to string
                    let key_bytes: Option<Vec<u8>> = row.try_get("encryptionkey")?;
                    if let Some(bytes) = key_bytes {
                        String::from_utf8(bytes)
                            .map_err(|e| AppError::internal(&format!("Invalid UTF-8 in encryption key: {}", e)))
                    } else {
                        Err(AppError::internal("Encryption key not found"))
                    }
                } else {
                    Err(AppError::internal("App settings not found"))
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT EncryptionKey FROM AppSettings LIMIT 1")
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let key: Option<String> = row.try_get("EncryptionKey")?;
                    key.ok_or_else(|| AppError::internal("Encryption key not found"))
                } else {
                    Err(AppError::internal("App settings not found"))
                }
            },
        })
    }

    // Get gPodder settings - matches Python get_gpodder_settings function exactly
    pub async fn get_gpodder_settings(&self, user_id: i32) -> AppResult<Option<serde_json::Value>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT gpodderurl, gpodderloginname, pod_sync_type FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let url: Option<String> = row.try_get("gpodderurl")?;
                    let username: Option<String> = row.try_get("gpodderloginname")?;
                    let sync_type: Option<String> = row.try_get("pod_sync_type")?;
                    
                    if url.is_some() && username.is_some() {
                        Ok(Some(serde_json::json!({
                            "gpodderurl": url.unwrap_or_default(),
                            "gpoddertoken": "", // Frontend expects this field but it's not used for display
                            "sync_type": sync_type.unwrap_or_default()
                        })))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT GpodderUrl, GpodderLoginName, Pod_Sync_Type FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let url: Option<String> = row.try_get("GpodderUrl")?;
                    let username: Option<String> = row.try_get("GpodderLoginName")?;
                    let sync_type: Option<String> = row.try_get("Pod_Sync_Type")?;
                    
                    if url.is_some() && username.is_some() {
                        Ok(Some(serde_json::json!({
                            "gpodderurl": url.unwrap_or_default(),
                            "gpoddertoken": "", // Frontend expects this field but it's not used for display
                            "sync_type": sync_type.unwrap_or_default()
                        })))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Check gPodder settings - matches Python check_gpodder_settings function exactly
    pub async fn check_gpodder_settings(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT gpodderurl, gpodderloginname FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let url: Option<String> = row.try_get("gpodderurl")?;
                    let username: Option<String> = row.try_get("gpodderloginname")?;
                    Ok(url.is_some() && username.is_some())
                } else {
                    Ok(false)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT GpodderUrl, GpodderLoginName FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let url: Option<String> = row.try_get("GpodderUrl")?;
                    let username: Option<String> = row.try_get("GpodderLoginName")?;
                    Ok(url.is_some() && username.is_some())
                } else {
                    Ok(false)
                }
            },
        })
    }

    // Find episode ID by URL for user
    pub(super) async fn find_episode_by_url(&self, user_id: i32, episode_url: &str) -> AppResult<Option<i32>> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"
                    SELECT e.episodeid 
                    FROM "Episodes" e 
                    JOIN "Podcasts" p ON e.feedid = p.feedid 
                    WHERE e.episodeurl = $1 AND p.userid = $2
                "#)
                    .bind(episode_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(Some(row.try_get("episodeid")?))
                } else {
                    Ok(None)
                }
            },
            mysql(pool) => {
                let row = sqlx::query("
                    SELECT e.EpisodeID 
                    FROM Episodes e 
                    JOIN Podcasts p ON e.FeedID = p.FeedID 
                    WHERE e.EpisodeURL = ? AND p.UserID = ?
                ")
                    .bind(episode_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    Ok(Some(row.try_get("EpisodeID")?))
                } else {
                    Ok(None)
                }
            },
        })
    }

    // Get gPodder status - matches Python get_user_gpodder_status function exactly
    pub async fn gpodder_get_status(&self, user_id: i32) -> AppResult<GpodderStatus> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"SELECT pod_sync_type, gpodderurl FROM "Users" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let sync_type: Option<String> = row.try_get("pod_sync_type")?;
                    let gpodder_url: Option<String> = row.try_get("gpodderurl")?;
                    
                    let sync_type = sync_type.unwrap_or_else(|| "None".to_string());
                    
                    Ok(GpodderStatus {
                        sync_type: sync_type.clone(),
                        gpodder_url,
                    })
                } else {
                    Ok(GpodderStatus {
                        sync_type: "None".to_string(),
                        gpodder_url: None,
                    })
                }
            },
            mysql(pool) => {
                let row = sqlx::query("SELECT Pod_Sync_Type, GpodderUrl FROM Users WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = row {
                    let sync_type: Option<String> = row.try_get("Pod_Sync_Type")?;
                    let gpodder_url: Option<String> = row.try_get("GpodderUrl")?;
                    
                    let sync_type = sync_type.unwrap_or_else(|| "None".to_string());
                    
                    Ok(GpodderStatus {
                        sync_type: sync_type.clone(),
                        gpodder_url,
                    })
                } else {
                    Ok(GpodderStatus {
                        sync_type: "None".to_string(),
                        gpodder_url: None,
                    })
                }
            },
        })
    }

    // Decrypt password using Fernet - matches Python encryption
    pub async fn decrypt_password(&self, encrypted_password: &str) -> AppResult<String> {
        use fernet::Fernet;
        
        // Get encryption key from app settings (base64 string)
        let encryption_key = self.get_encryption_key().await?;
        let fernet = Fernet::new(&encryption_key)
            .ok_or_else(|| AppError::internal("Failed to create Fernet cipher"))?;
        
        let decrypted = fernet.decrypt(encrypted_password)
            .map_err(|_e| AppError::internal(&format!("Failed to decrypt password: Fernet decryption error")))?;
        
        String::from_utf8(decrypted)
            .map_err(|e| AppError::internal(&format!("Invalid UTF-8 in decrypted password: {}", e)))
    }

    /// Delete gpodder API sessions that expired more than a day ago. Expired sessions are
    /// already refused at login; this only stops the table growing (the day of slack covers
    /// clock/timezone skew between the writer and the database). Returns the rows deleted.
    pub async fn cleanup_expired_gpodder_sessions(&self) -> AppResult<u64> {
        let deleted = dialect_match!(self, {
            pg(pool) => q::query(r#"DELETE FROM "GpodderSessions" WHERE ExpiresAt < NOW() - INTERVAL '1 day'"#)
                .execute(pool)
                .await?
                .rows_affected(),
            mysql(pool) => sqlx::query("DELETE FROM GpodderSessions WHERE ExpiresAt < NOW() - INTERVAL 1 DAY")
                .execute(pool)
                .await?
                .rows_affected(),
        });
        tracing::debug!("Cleaned up {} expired gpodder sessions", deleted);
        Ok(deleted)
    }

    // Turn on the PinePods gpodder server for a user. Clients log in with the user's password or
    // an app password, so no token is generated. An external server the user already syncs with
    // keeps its settings ("both"). The user's current listening state is published to the sync
    // log so clients get it on their first sync.
    pub async fn set_gpodder_internal_sync(&self, user_id: i32) -> AppResult<serde_json::Value> {
        let device_name = self.sync_ensure_pinepods_device(user_id).await?;
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET gpodderserverenabled = TRUE, defaultgpodderdevice = $1 WHERE userid = $2"#)
                    .bind(&device_name)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET GpodderServerEnabled = TRUE, DefaultGpodderDevice = ? WHERE UserID = ?")
                    .bind(&device_name)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        self.sync_mirror_sync_type(user_id).await?;
        crate::services::sync::invalidate_sync_enabled(user_id);

        match self.gpodder_publish_episode_state(user_id).await {
            Ok(n) => tracing::debug!("Published {} episode states to the sync log for user {}", n, user_id),
            Err(e) => tracing::warn!("Publishing episode state for user {} failed: {}", user_id, e),
        }

        Ok(serde_json::json!({
            "device_name": device_name,
            "device_id": device_name,
            "success": true
        }))
    }

    // Turn off the PinePods gpodder server. An external target keeps syncing.
    pub async fn disable_gpodder_internal_sync(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Users" SET gpodderserverenabled = FALSE WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Users SET GpodderServerEnabled = FALSE WHERE UserID = ?")
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        self.sync_mirror_sync_type(user_id).await?;
        crate::services::sync::invalidate_sync_enabled(user_id);
        Ok(true)
    }

    // Remove GPodder sync settings for a user - matches Python remove_gpodder_settings function exactly
    pub async fn remove_gpodder_settings(&self, user_id: i32) -> AppResult<bool> {
        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                
                // First delete any device records
                q::query(r#"DELETE FROM "GpodderDevices" WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                
                q::query(r#"DELETE FROM "GpodderSyncState" WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;

                q::query(r#"DELETE FROM "SyncTargets" WHERE userid = $1"#)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                
                // Then clear GPodder settings from user record
                q::query(r#"
                    UPDATE "Users"
                    SET gpodderurl = '', gpodderloginname = '', gpoddertoken = '', pod_sync_type = 'None', gpodderserverenabled = FALSE
                    WHERE userid = $1
                "#)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                
                tx.commit().await?;
                crate::services::sync::invalidate_sync_enabled(user_id); // GpodderServerEnabled changed
                Ok(true)
            },
            mysql(pool) => {
                let mut tx = pool.begin().await?;
                
                // First delete any device records
                sqlx::query("DELETE FROM GpodderDevices WHERE UserID = ?")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                
                sqlx::query("DELETE FROM GpodderSyncState WHERE UserID = ?")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;

                sqlx::query("DELETE FROM SyncTargets WHERE UserID = ?")
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                
                // Then clear GPodder settings from user record
                sqlx::query(r#"
                    UPDATE Users
                    SET GpodderUrl = '', GpodderLoginName = '', GpodderToken = '', Pod_Sync_Type = 'None', GpodderServerEnabled = FALSE
                    WHERE UserID = ?
                "#)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?;
                
                tx.commit().await?;
                crate::services::sync::invalidate_sync_enabled(user_id); // GpodderServerEnabled changed
                Ok(true)
            },
        })
    }
}
