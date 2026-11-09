//! In-app notifications (the activity center's messages), each stored as its JSON.

use super::*;
use chrono::NaiveDateTime;

/// One stored in-app notification.
pub struct StoredInAppNotification<'a> {
    pub id: &'a str,
    pub user_id: i32,
    pub payload: &'a str,
    pub read: bool,
    pub created_at: NaiveDateTime,
    pub expires_at: NaiveDateTime,
}

fn now() -> NaiveDateTime {
    Utc::now().naive_utc()
}

impl DatabasePool {
    /// Insert a notification, or replace the stored one with the same id.
    pub async fn save_in_app_notification(&self, n: &StoredInAppNotification<'_>) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(
                    r#"INSERT INTO "InAppNotifications" (notificationid, userid, payload, isread, createdat, expiresat)
                       VALUES ($1, $2, $3, $4, $5, $6)
                       ON CONFLICT (notificationid) DO UPDATE
                       SET payload = EXCLUDED.payload, isread = EXCLUDED.isread, expiresat = EXCLUDED.expiresat"#,
                )
                .bind(n.id)
                .bind(n.user_id)
                .bind(n.payload)
                .bind(n.read)
                .bind(n.created_at)
                .bind(n.expires_at)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                sqlx::query(
                    "INSERT INTO InAppNotifications (NotificationID, UserID, Payload, IsRead, CreatedAt, ExpiresAt)
                     VALUES (?, ?, ?, ?, ?, ?)
                     ON DUPLICATE KEY UPDATE Payload = VALUES(Payload), IsRead = VALUES(IsRead), ExpiresAt = VALUES(ExpiresAt)",
                )
                .bind(n.id)
                .bind(n.user_id)
                .bind(n.payload)
                .bind(n.read)
                .bind(n.created_at)
                .bind(n.expires_at)
                .execute(pool)
                .await?;
            },
        });
        Ok(())
    }

    /// The user's live notifications as JSON, after deleting their expired ones.
    pub async fn list_in_app_notifications(&self, user_id: i32) -> AppResult<Vec<String>> {
        let now = now();
        let payloads = dialect_match!(self, {
            pg(pool) => {
                q::query(r#"DELETE FROM "InAppNotifications" WHERE userid = $1 AND expiresat <= $2"#)
                    .bind(user_id)
                    .bind(now)
                    .execute(pool)
                    .await?;
                q::query_scalar::<String>(r#"SELECT payload FROM "InAppNotifications" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?
            },
            mysql(pool) => {
                sqlx::query("DELETE FROM InAppNotifications WHERE UserID = ? AND ExpiresAt <= ?")
                    .bind(user_id)
                    .bind(now)
                    .execute(pool)
                    .await?;
                sqlx::query_scalar::<_, String>("SELECT Payload FROM InAppNotifications WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?
            },
        });
        Ok(payloads)
    }

    /// One of the user's live notifications as JSON.
    pub async fn get_in_app_notification(&self, user_id: i32, id: &str) -> AppResult<Option<String>> {
        let now = now();
        let payload = dialect_match!(self, {
            pg(pool) => {
                q::query_scalar::<String>(
                    r#"SELECT payload FROM "InAppNotifications" WHERE userid = $1 AND notificationid = $2 AND expiresat > $3"#,
                )
                .bind(user_id)
                .bind(id)
                .bind(now)
                .fetch_optional(pool)
                .await?
            },
            mysql(pool) => {
                sqlx::query_scalar::<_, String>(
                    "SELECT Payload FROM InAppNotifications WHERE UserID = ? AND NotificationID = ? AND ExpiresAt > ?",
                )
                .bind(user_id)
                .bind(id)
                .bind(now)
                .fetch_optional(pool)
                .await?
            },
        });
        Ok(payload)
    }

    pub async fn delete_in_app_notification(&self, user_id: i32, id: &str) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"DELETE FROM "InAppNotifications" WHERE userid = $1 AND notificationid = $2"#)
                    .bind(user_id)
                    .bind(id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("DELETE FROM InAppNotifications WHERE UserID = ? AND NotificationID = ?")
                    .bind(user_id)
                    .bind(id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    /// Delete the user's notifications: the read ones, or all of them.
    pub async fn clear_in_app_notifications(&self, user_id: i32, read_only: bool) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                let sql = if read_only {
                    r#"DELETE FROM "InAppNotifications" WHERE userid = $1 AND isread = TRUE"#
                } else {
                    r#"DELETE FROM "InAppNotifications" WHERE userid = $1"#
                };
                q::query(sql).bind(user_id).execute(pool).await?;
            },
            mysql(pool) => {
                let sql = if read_only {
                    "DELETE FROM InAppNotifications WHERE UserID = ? AND IsRead = 1"
                } else {
                    "DELETE FROM InAppNotifications WHERE UserID = ?"
                };
                sqlx::query(sql).bind(user_id).execute(pool).await?;
            },
        });
        Ok(())
    }
}
