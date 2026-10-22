// SQL for the gpodder server, per backend.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, NaiveDateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{MySqlConnection, PgConnection, Row};

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::sync::{shareable_feed, ActionKind, EpisodeAction};

const PG_NOW: &str = "SELECT FLOOR(EXTRACT(EPOCH FROM CLOCK_TIMESTAMP()))::BIGINT";
const MYSQL_NOW: &str = "SELECT UNIX_TIMESTAMP()";

// Most new episodes returned by one `updates` request.
const UPDATES_LIMIT: i64 = 1000;

fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

#[derive(Debug, Clone)]
pub(crate) struct GpodderUser {
    pub user_id: i32,
    pub username: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct SessionUser {
    pub session_id: i32,
    pub user_id: i32,
    pub username: String,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Device {
    pub id: i32,
    pub name: String,
    pub caption: Option<String>,
    pub kind: Option<String>,
    pub subs_cursor: Option<i64>,
}

/// A logged action as served: the action (with its device) and the episode's known duration.
#[derive(Debug, Clone)]
pub(crate) struct StoredAction {
    pub id: i32,
    pub action: EpisodeAction,
    pub known_duration: Option<i32>,
}

#[derive(Debug, Clone)]
pub(crate) struct PodcastInfo {
    pub title: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub logo_url: Option<String>,
    pub website: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct NewEpisode {
    pub episode_id: i32,
    pub title: Option<String>,
    pub url: String,
    pub description: Option<String>,
    pub released: Option<NaiveDateTime>,
    pub podcast_title: Option<String>,
    pub podcast_url: String,
    pub website: Option<String>,
}

/// Which settings a request addresses (`GpodderSyncSettings` rows).
#[derive(Debug, Clone)]
pub(crate) struct SettingsKey<'a> {
    pub scope: &'a str,
    pub device_id: Option<i32>,
    pub podcast: Option<&'a str>,
    pub episode: Option<&'a str>,
}

/// The internal server as the old statistics view shows it.
#[derive(Debug, Clone, Default)]
pub struct GpodderStats {
    pub devices: Vec<GpodderStatsDevice>,
    pub library: Vec<(String, Option<String>)>,
    pub recent_actions: Vec<GpodderStatsAction>,
    pub total_actions: i64,
    pub last_change: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct GpodderStatsDevice {
    pub name: String,
    pub caption: String,
    pub kind: String,
}

#[derive(Debug, Clone)]
pub struct GpodderStatsAction {
    pub podcast: String,
    pub episode: String,
    pub action: String,
    pub timestamp: i64,
    pub position: Option<i32>,
    pub device: Option<String>,
    /// `client`, `pinepods` or `target:<id>`.
    pub origin: String,
}

fn reserved(name: &str) -> AppError {
    AppError::bad_request(format!("device id {name:?} is reserved"))
}

// ---- devices (inside a transaction) -------------------------------------------------------

/// The device `name`, created if missing and reactivated if inactive. Pinepods' own device
/// can't be used by a client.
async fn pg_ensure_device(conn: &mut PgConnection, user_id: i32, name: &str) -> AppResult<Device> {
    let row = sqlx::query(
        r#"INSERT INTO "GpodderDevices" (userid, devicename, devicetype, isdefault, isactive, lastsync)
           VALUES ($1, $2, 'other', FALSE, TRUE, CURRENT_TIMESTAMP)
           ON CONFLICT (userid, devicename) DO UPDATE SET isactive = TRUE, lastsync = CURRENT_TIMESTAMP
             WHERE "GpodderDevices".systemrole IS NULL
           RETURNING deviceid, devicename, COALESCE(usercaption, devicecaption) AS devicecaption, devicetype, subsdownloadcursor"#,
    )
    .bind(user_id)
    .bind(name)
    .fetch_optional(&mut *conn)
    .await?;
    let Some(r) = row else { return Err(reserved(name)) };
    Ok(Device {
        id: r.try_get("deviceid")?,
        name: r.try_get("devicename")?,
        caption: r.try_get("devicecaption")?,
        kind: r.try_get("devicetype")?,
        subs_cursor: r.try_get("subsdownloadcursor")?,
    })
}

async fn mysql_ensure_device(conn: &mut MySqlConnection, user_id: i32, name: &str) -> AppResult<Device> {
    sqlx::query(
        "INSERT INTO GpodderDevices (UserID, DeviceName, DeviceType, IsDefault, IsActive, LastSync)
         VALUES (?, ?, 'other', FALSE, TRUE, CURRENT_TIMESTAMP)
         ON DUPLICATE KEY UPDATE
           IsActive = IF(SystemRole IS NULL, TRUE, IsActive),
           LastSync = IF(SystemRole IS NULL, CURRENT_TIMESTAMP, LastSync)",
    )
    .bind(user_id)
    .bind(name)
    .execute(&mut *conn)
    .await?;
    let r = sqlx::query(
        "SELECT DeviceID, DeviceName, COALESCE(UserCaption, DeviceCaption) AS DeviceCaption, DeviceType, SubsDownloadCursor, SystemRole
         FROM GpodderDevices WHERE UserID = ? AND DeviceName = ?",
    )
    .bind(user_id)
    .bind(name)
    .fetch_one(&mut *conn)
    .await?;
    if r.try_get::<Option<String>, _>("SystemRole")?.is_some() {
        return Err(reserved(name));
    }
    Ok(Device {
        id: r.try_get("DeviceID")?,
        name: r.try_get("DeviceName")?,
        caption: r.try_get("DeviceCaption")?,
        kind: r.try_get("DeviceType")?,
        subs_cursor: r.try_get("SubsDownloadCursor")?,
    })
}

fn fold_changes(rows: Vec<(String, String)>) -> Vec<(String, bool)> {
    let mut last: HashMap<String, (usize, bool)> = HashMap::new();
    for (i, (url, action)) in rows.into_iter().enumerate() {
        last.insert(url, (i, action.eq_ignore_ascii_case("add")));
    }
    let mut out: Vec<(usize, String, bool)> = last.into_iter().map(|(u, (i, a))| (i, u, a)).collect();
    out.sort_by_key(|(i, _, _)| *i);
    out.into_iter().map(|(_, u, a)| (u, a)).collect()
}

impl DatabasePool {
    /// Unix seconds from the database clock.
    pub(crate) async fn gpodder_now(&self) -> AppResult<i64> {
        Ok(match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(PG_NOW).fetch_one(pool).await?,
            DatabasePool::MySQL(pool) => sqlx::query_scalar(MYSQL_NOW).fetch_one(pool).await?,
        })
    }

    // ---- users, passwords, sessions -------------------------------------------------------

    /// The user named `name`, case-insensitively (an exact match wins if names differ only in case).
    pub(crate) async fn gpodder_user(&self, name: &str) -> AppResult<Option<GpodderUser>> {
        let rows: Vec<GpodderUser> = match self {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"SELECT userid, username, gpodderserverenabled FROM "Users" WHERE LOWER(username) = LOWER($1)"#,
            )
            .bind(name)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| {
                Ok(GpodderUser {
                    user_id: r.try_get("userid")?,
                    username: r.try_get("username")?,
                    enabled: r.try_get("gpodderserverenabled")?,
                })
            })
            .collect::<AppResult<_>>()?,
            DatabasePool::MySQL(pool) => sqlx::query(
                "SELECT UserID, Username, GpodderServerEnabled FROM Users WHERE LOWER(Username) = LOWER(?)",
            )
            .bind(name)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| {
                Ok(GpodderUser {
                    user_id: r.try_get("UserID")?,
                    username: r.try_get("Username")?,
                    enabled: r.try_get("GpodderServerEnabled")?,
                })
            })
            .collect::<AppResult<_>>()?,
        };
        let exact = rows.iter().position(|u| u.username == name);
        Ok(match exact {
            Some(i) => rows.into_iter().nth(i),
            None if rows.len() == 1 => rows.into_iter().next(),
            None => None,
        })
    }

    /// `(AppPasswordID, PasswordHash)` for each of the user's app passwords.
    pub(crate) async fn gpodder_app_passwords(&self, user_id: i32) -> AppResult<Vec<(i32, String)>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_as(r#"SELECT apppasswordid, passwordhash FROM "GpodderAppPasswords" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_as("SELECT AppPasswordID, PasswordHash FROM GpodderAppPasswords WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?
            }
        })
    }

    pub(crate) async fn gpodder_touch_app_password(&self, id: i32) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "GpodderAppPasswords" SET lastusedat = CURRENT_TIMESTAMP WHERE apppasswordid = $1"#)
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE GpodderAppPasswords SET LastUsedAt = CURRENT_TIMESTAMP WHERE AppPasswordID = ?")
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    /// A new 30-day session. Opened with an app password, it ends when that password is revoked.
    pub(crate) async fn gpodder_create_session(
        &self,
        user_id: i32,
        token: &str,
        user_agent: Option<&str>,
        client_ip: Option<&str>,
        app_password: Option<i32>,
    ) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(
                    r#"INSERT INTO "GpodderSessions" (userid, sessiontoken, expiresat, useragent, clientip, apppasswordid)
                       VALUES ($1, $2, NOW() + INTERVAL '30 days', $3, $4, $5)"#,
                )
                .bind(user_id)
                .bind(token)
                .bind(user_agent)
                .bind(client_ip)
                .bind(app_password)
                .execute(pool)
                .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(
                    "INSERT INTO GpodderSessions (UserID, SessionToken, ExpiresAt, UserAgent, ClientIP, AppPasswordID)
                     VALUES (?, ?, NOW() + INTERVAL 30 DAY, ?, ?, ?)",
                )
                .bind(user_id)
                .bind(token)
                .bind(user_agent)
                .bind(client_ip)
                .bind(app_password)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    /// The unexpired session with this token. Only sessions opened with an app password count.
    pub(crate) async fn gpodder_session(&self, token: &str) -> AppResult<Option<SessionUser>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"SELECT s.sessionid, s.userid, u.username, u.gpodderserverenabled
                   FROM "GpodderSessions" s JOIN "Users" u ON u.userid = s.userid
                   WHERE s.sessiontoken = $1 AND s.expiresat > NOW() AND s.apppasswordid IS NOT NULL"#,
            )
            .bind(token)
            .fetch_optional(pool)
            .await?
            .map(|r| -> AppResult<_> {
                Ok(SessionUser {
                    session_id: r.try_get("sessionid")?,
                    user_id: r.try_get("userid")?,
                    username: r.try_get("username")?,
                    enabled: r.try_get("gpodderserverenabled")?,
                })
            })
            .transpose()?,
            DatabasePool::MySQL(pool) => sqlx::query(
                "SELECT s.SessionID, s.UserID, u.Username, u.GpodderServerEnabled
                 FROM GpodderSessions s JOIN Users u ON u.UserID = s.UserID
                 WHERE s.SessionToken = ? AND s.ExpiresAt > NOW() AND s.AppPasswordID IS NOT NULL",
            )
            .bind(token)
            .fetch_optional(pool)
            .await?
            .map(|r| -> AppResult<_> {
                Ok(SessionUser {
                    session_id: r.try_get("SessionID")?,
                    user_id: r.try_get("UserID")?,
                    username: r.try_get("Username")?,
                    enabled: r.try_get("GpodderServerEnabled")?,
                })
            })
            .transpose()?,
        })
    }

    pub(crate) async fn gpodder_delete_session(&self, token: &str) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"DELETE FROM "GpodderSessions" WHERE sessiontoken = $1"#).bind(token).execute(pool).await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("DELETE FROM GpodderSessions WHERE SessionToken = ?").bind(token).execute(pool).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn gpodder_episodes_cursor(&self, session_id: i32) -> AppResult<Option<i64>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar(r#"SELECT episodesdownloadcursor FROM "GpodderSessions" WHERE sessionid = $1"#)
                    .bind(session_id)
                    .fetch_optional(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_scalar("SELECT EpisodesDownloadCursor FROM GpodderSessions WHERE SessionID = ?")
                    .bind(session_id)
                    .fetch_optional(pool)
                    .await?
            }
        }
        .flatten())
    }

    pub(crate) async fn gpodder_set_episodes_cursor(&self, session_id: i32, cursor: i64) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "GpodderSessions" SET episodesdownloadcursor = $1, lastactive = CURRENT_TIMESTAMP WHERE sessionid = $2"#)
                    .bind(cursor)
                    .bind(session_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE GpodderSessions SET EpisodesDownloadCursor = ?, LastActive = CURRENT_TIMESTAMP WHERE SessionID = ?")
                    .bind(cursor)
                    .bind(session_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    // ---- devices ----------------------------------------------------------------------------

    pub(crate) async fn gpodder_ensure_device(&self, user_id: i32, name: &str) -> AppResult<Device> {
        match self {
            DatabasePool::Postgres(pool) => pg_ensure_device(&mut *pool.acquire().await?, user_id, name).await,
            DatabasePool::MySQL(pool) => mysql_ensure_device(&mut *pool.acquire().await?, user_id, name).await,
        }
    }

    /// Set a device's caption and/or type, creating it if needed.
    pub(crate) async fn gpodder_update_device(&self, user_id: i32, name: &str, caption: Option<&str>, kind: Option<&str>) -> AppResult<()> {
        let device = self.gpodder_ensure_device(user_id, name).await?;
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(
                    r#"UPDATE "GpodderDevices" SET devicecaption = COALESCE($1, devicecaption), devicetype = COALESCE($2, devicetype)
                       WHERE deviceid = $3"#,
                )
                .bind(caption)
                .bind(kind)
                .bind(device.id)
                .execute(pool)
                .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(
                    "UPDATE GpodderDevices SET DeviceCaption = COALESCE(?, DeviceCaption), DeviceType = COALESCE(?, DeviceType)
                     WHERE DeviceID = ?",
                )
                .bind(caption)
                .bind(kind)
                .bind(device.id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    /// The user's active devices, Pinepods' own included.
    pub(crate) async fn gpodder_devices(&self, user_id: i32) -> AppResult<Vec<Device>> {
        let rows = match self {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"SELECT deviceid AS id, devicename AS name, COALESCE(usercaption, devicecaption) AS caption, devicetype AS kind, subsdownloadcursor AS subs_cursor
                   FROM "GpodderDevices" WHERE userid = $1 AND COALESCE(isactive, TRUE) ORDER BY deviceid"#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| device_row(r))
            .collect::<AppResult<Vec<_>>>()?,
            DatabasePool::MySQL(pool) => sqlx::query(
                "SELECT DeviceID AS id, DeviceName AS name, COALESCE(UserCaption, DeviceCaption) AS caption, DeviceType AS kind, SubsDownloadCursor AS subs_cursor
                 FROM GpodderDevices WHERE UserID = ? AND COALESCE(IsActive, TRUE) ORDER BY DeviceID",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| device_row(r))
            .collect::<AppResult<Vec<_>>>()?,
        };
        Ok(rows)
    }

    /// A device's id by name, without creating it.
    pub(crate) async fn gpodder_device_id(&self, user_id: i32, name: &str) -> AppResult<Option<i32>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar(r#"SELECT deviceid FROM "GpodderDevices" WHERE userid = $1 AND devicename = $2"#)
                    .bind(user_id)
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_scalar("SELECT DeviceID FROM GpodderDevices WHERE UserID = ? AND DeviceName = ?")
                    .bind(user_id)
                    .bind(name)
                    .fetch_optional(pool)
                    .await?
            }
        })
    }

    pub(crate) async fn gpodder_set_subs_cursor(&self, device_id: i32, cursor: i64) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "GpodderDevices" SET subsdownloadcursor = $1 WHERE deviceid = $2"#)
                    .bind(cursor)
                    .bind(device_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE GpodderDevices SET SubsDownloadCursor = ? WHERE DeviceID = ?")
                    .bind(cursor)
                    .bind(device_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }

    // ---- subscriptions ------------------------------------------------------------------------

    /// The library as clients see it: the user's shareable podcasts, then feeds a device or
    /// target subscribed to that aren't imported yet (latest log action is `add`). Log rows
    /// older than the old internal sync's last run (`Users.LastSyncTime`) were already handled
    /// by it, so they don't count.
    pub(crate) async fn gpodder_library(&self, user_id: i32) -> AppResult<Vec<String>> {
        let (podcasts, log): (Vec<(Option<String>, Option<String>, Option<String>, bool)>, Vec<(String, String)>) = match self {
            DatabasePool::Postgres(pool) => (
                sqlx::query_as(
                    r#"SELECT feedurl, username, password, COALESCE(isyoutubechannel, FALSE)
                       FROM "Podcasts" WHERE userid = $1 ORDER BY podcastid"#,
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?,
                sqlx::query_as(
                    r#"SELECT podcasturl, action FROM "GpodderSyncSubscriptions"
                       WHERE userid = $1
                         AND serverts >= COALESCE((SELECT FLOOR(EXTRACT(EPOCH FROM lastsynctime))::BIGINT FROM "Users" WHERE userid = $1), 0)
                       ORDER BY subscriptionid"#,
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?,
            ),
            DatabasePool::MySQL(pool) => (
                sqlx::query_as::<_, (Option<String>, Option<String>, Option<String>, Option<bool>)>(
                    "SELECT FeedURL, Username, Password, IsYouTubeChannel FROM Podcasts WHERE UserID = ? ORDER BY PodcastID",
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|(a, b, c, d)| (a, b, c, d.unwrap_or(false)))
                .collect(),
                // LastSyncTime is a UTC DATETIME; TIMESTAMPDIFF avoids UNIX_TIMESTAMP's session time zone.
                sqlx::query_as(
                    "SELECT PodcastURL, Action FROM GpodderSyncSubscriptions
                     WHERE UserID = ?
                       AND ServerTs >= COALESCE((SELECT TIMESTAMPDIFF(SECOND, '1970-01-01 00:00:00', LastSyncTime) FROM Users WHERE UserID = ?), 0)
                     ORDER BY SubscriptionID",
                )
                .bind(user_id)
                .bind(user_id)
                .fetch_all(pool)
                .await?,
            ),
        };
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        let mut all_feeds = HashSet::new();
        for (url, user, pass, yt) in podcasts {
            let Some(url) = url else { continue };
            all_feeds.insert(url.clone());
            if shareable_feed(&url, user.as_deref(), pass.as_deref(), yt) && seen.insert(url.clone()) {
                out.push(url);
            }
        }
        let pending = fold_changes(log).into_iter().filter(|(_, add)| *add).map(|(u, _)| u);
        let in_flight = crate::services::sync::apply::in_flight_adds(user_id);
        for url in pending.chain(in_flight) {
            if !all_feeds.contains(&url) && super::is_http(&url) && seen.insert(url.clone()) {
                out.push(url);
            }
        }
        Ok(out)
    }

    /// Changes logged since `since` by anyone but `device_id`, folded to the last action per URL.
    pub(crate) async fn gpodder_subscription_changes(&self, user_id: i32, since: i64, device_id: i32) -> AppResult<Vec<(String, bool)>> {
        let rows: Vec<(String, String)> = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_as(
                    r#"SELECT podcasturl, action FROM "GpodderSyncSubscriptions"
                       WHERE userid = $1 AND serverts >= $2 AND (deviceid IS NULL OR deviceid <> $3)
                       ORDER BY subscriptionid"#,
                )
                .bind(user_id)
                .bind(since)
                .bind(device_id)
                .fetch_all(pool)
                .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_as(
                    "SELECT PodcastURL, Action FROM GpodderSyncSubscriptions
                     WHERE UserID = ? AND ServerTs >= ? AND (DeviceID IS NULL OR DeviceID <> ?)
                     ORDER BY SubscriptionID",
                )
                .bind(user_id)
                .bind(since)
                .bind(device_id)
                .fetch_all(pool)
                .await?
            }
        };
        Ok(fold_changes(rows))
    }

    /// The feeds this device has reported (adds minus removes, POST and PUT).
    pub(crate) async fn gpodder_device_subscriptions(&self, device_id: i32) -> AppResult<HashSet<String>> {
        let urls: Vec<String> = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar(r#"SELECT podcasturl FROM "GpodderDeviceSubscriptions" WHERE deviceid = $1"#)
                    .bind(device_id)
                    .fetch_all(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_scalar("SELECT PodcastURL FROM GpodderDeviceSubscriptions WHERE DeviceID = ?")
                    .bind(device_id)
                    .fetch_all(pool)
                    .await?
            }
        };
        Ok(urls.into_iter().collect())
    }

    /// Log a device's subscription changes and update its list, in one transaction.
    pub(crate) async fn gpodder_record_subscriptions(&self, user_id: i32, device_id: i32, add: &[String], remove: &[String]) -> AppResult<()> {
        if add.is_empty() && remove.is_empty() {
            return Ok(());
        }
        let mut urls: Vec<&str> = add.iter().map(String::as_str).collect();
        urls.extend(remove.iter().map(String::as_str));
        let actions: Vec<&str> = add.iter().map(|_| "add").chain(remove.iter().map(|_| "remove")).collect();
        let push = match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let targets = crate::services::sync::db::pg_lock_targets_shared(&mut tx, user_id).await?;
                let now: i64 = sqlx::query_scalar(PG_NOW).fetch_one(&mut *tx).await?;
                sqlx::query(
                    r#"INSERT INTO "GpodderSyncSubscriptions" (userid, deviceid, podcasturl, action, timestamp, serverts, origin)
                       SELECT $1, $2, u.url, u.action, $3, $3, 'client' FROM UNNEST($4::text[], $5::text[]) AS u(url, action)"#,
                )
                .bind(user_id)
                .bind(device_id)
                .bind(now)
                .bind(&urls)
                .bind(&actions)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    r#"INSERT INTO "GpodderDeviceSubscriptions" (deviceid, podcasturl)
                       SELECT $1, UNNEST($2::text[]) ON CONFLICT DO NOTHING"#,
                )
                .bind(device_id)
                .bind(add)
                .execute(&mut *tx)
                .await?;
                sqlx::query(r#"DELETE FROM "GpodderDeviceSubscriptions" WHERE deviceid = $1 AND podcasturl = ANY($2)"#)
                    .bind(device_id)
                    .bind(remove)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                targets.any_enabled
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                let targets = crate::services::sync::db::mysql_lock_targets_shared(&mut tx, user_id).await?;
                let now: i64 = sqlx::query_scalar(MYSQL_NOW).fetch_one(&mut *tx).await?;
                for chunk in urls.iter().zip(&actions).collect::<Vec<_>>().chunks(500) {
                    let values = vec!["(?, ?, ?, ?, ?, ?, 'client')"; chunk.len()].join(", ");
                    let sql = format!(
                        "INSERT INTO GpodderSyncSubscriptions (UserID, DeviceID, PodcastURL, Action, Timestamp, ServerTs, Origin) VALUES {values}"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
                    for (url, action) in chunk {
                        q = q.bind(user_id).bind(device_id).bind(**url).bind(**action).bind(now).bind(now);
                    }
                    q.execute(&mut *tx).await?;
                }
                for chunk in add.chunks(500) {
                    let values = vec!["(?, ?)"; chunk.len()].join(", ");
                    let sql = format!("INSERT IGNORE INTO GpodderDeviceSubscriptions (DeviceID, PodcastURL) VALUES {values}");
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
                    for url in chunk {
                        q = q.bind(device_id).bind(url);
                    }
                    q.execute(&mut *tx).await?;
                }
                for chunk in remove.chunks(500) {
                    let marks = vec!["?"; chunk.len()].join(", ");
                    let sql = format!("DELETE FROM GpodderDeviceSubscriptions WHERE DeviceID = ? AND PodcastURLHash IN ({marks})");
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql)).bind(device_id);
                    for url in chunk {
                        q = q.bind(sha256_hex(url));
                    }
                    q.execute(&mut *tx).await?;
                }
                tx.commit().await?;
                targets.any_enabled
            }
        };
        // Client rows go to the external target too.
        if push {
            crate::services::sync::push::schedule(self, user_id);
        }
        Ok(())
    }

    /// Title etc. of the user's podcasts, by feed URL.
    pub(crate) async fn gpodder_podcast_info(&self, user_id: i32) -> AppResult<HashMap<String, PodcastInfo>> {
        type Row6 = (Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>);
        let rows: Vec<Row6> = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_as(
                    r#"SELECT feedurl, podcastname, description, author, artworkurl, websiteurl FROM "Podcasts" WHERE userid = $1"#,
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_as("SELECT FeedURL, PodcastName, Description, Author, ArtworkURL, WebsiteURL FROM Podcasts WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_all(pool)
                    .await?
            }
        };
        Ok(rows
            .into_iter()
            .filter_map(|(url, title, description, author, logo_url, website)| {
                Some((url?, PodcastInfo { title, description, author, logo_url, website }))
            })
            .collect())
    }

    // ---- episode actions --------------------------------------------------------------------

    /// Logged actions received since `since`, oldest first, optionally for one feed or device.
    pub(crate) async fn gpodder_episode_actions(&self, user_id: i32, since: i64, podcast: Option<&str>, device_id: Option<i32>) -> AppResult<Vec<StoredAction>> {
        let rows = match self {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"SELECT a.actionid AS id, a.podcasturl AS purl, a.episodeurl AS eurl, a.guid, a.action, a.timestamp AS ts,
                          a.started, a.position, a.total, d.devicename AS device, e.episodeduration AS duration
                   FROM "GpodderSyncEpisodeActions" a
                   LEFT JOIN "GpodderDevices" d ON d.deviceid = a.deviceid
                   LEFT JOIN "Episodes" e ON e.episodeid = a.episodeid
                   WHERE a.userid = $1 AND a.serverts >= $2
                     AND ($3::text IS NULL OR a.podcasturl = $3) AND ($4::int IS NULL OR a.deviceid = $4)
                   ORDER BY a.actionid"#,
            )
            .bind(user_id)
            .bind(since)
            .bind(podcast)
            .bind(device_id)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| stored_action(r))
            .collect::<AppResult<Vec<_>>>()?,
            DatabasePool::MySQL(pool) => sqlx::query(
                "SELECT a.ActionID AS id, a.PodcastURL AS purl, a.EpisodeURL AS eurl, a.Guid AS guid, a.Action AS action,
                        a.Timestamp AS ts, a.Started AS started, a.Position AS position, a.Total AS total,
                        d.DeviceName AS device, e.EpisodeDuration AS duration
                 FROM GpodderSyncEpisodeActions a
                 LEFT JOIN GpodderDevices d ON d.DeviceID = a.DeviceID
                 LEFT JOIN Episodes e ON e.EpisodeID = a.EpisodeID
                 WHERE a.UserID = ? AND a.ServerTs >= ?
                   AND (? IS NULL OR a.PodcastURL = ?) AND (? IS NULL OR a.DeviceID = ?)
                 ORDER BY a.ActionID",
            )
            .bind(user_id)
            .bind(since)
            .bind(podcast)
            .bind(podcast)
            .bind(device_id)
            .bind(device_id)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| stored_action(r))
            .collect::<AppResult<Vec<_>>>()?,
        };
        Ok(rows.into_iter().flatten().collect())
    }

    /// Log uploaded actions in one transaction: one receive time for all, the client's action
    /// time kept, devices created as needed, `resolved[i]` stored as the action's EpisodeID.
    pub(crate) async fn gpodder_record_episode_actions(&self, user_id: i32, actions: &[EpisodeAction], resolved: &[Option<i32>]) -> AppResult<()> {
        if actions.is_empty() {
            return Ok(());
        }
        let names: Vec<&str> = {
            let mut seen = HashSet::new();
            actions.iter().filter_map(|a| a.device.as_deref()).filter(|d| seen.insert(*d)).collect()
        };
        let push = match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                let targets = crate::services::sync::db::pg_lock_targets_shared(&mut tx, user_id).await?;
                let mut ids = HashMap::new();
                for name in names {
                    ids.insert(name, pg_ensure_device(&mut tx, user_id, name).await?.id);
                }
                let now: i64 = sqlx::query_scalar(PG_NOW).fetch_one(&mut *tx).await?;
                let col = |f: &dyn Fn(&EpisodeAction) -> Option<i32>| actions.iter().map(f).collect::<Vec<_>>();
                sqlx::query(
                    r#"INSERT INTO "GpodderSyncEpisodeActions"
                         (userid, deviceid, podcasturl, episodeurl, action, timestamp, serverts, started, position, total, guid, episodeid, origin)
                       SELECT $1, u.deviceid, u.purl, u.eurl, u.action, u.ts, $2, u.started, u.position, u.total, u.guid, u.episodeid, 'client'
                       FROM UNNEST($3::int[], $4::text[], $5::text[], $6::text[], $7::bigint[], $8::int[], $9::int[], $10::int[], $11::text[], $12::int[])
                         AS u(deviceid, purl, eurl, action, ts, started, position, total, guid, episodeid)"#,
                )
                .bind(user_id)
                .bind(now)
                .bind(actions.iter().map(|a| a.device.as_deref().and_then(|d| ids.get(d).copied())).collect::<Vec<_>>())
                .bind(actions.iter().map(|a| a.podcast_url.as_str()).collect::<Vec<_>>())
                .bind(actions.iter().map(|a| a.episode_url.as_str()).collect::<Vec<_>>())
                .bind(actions.iter().map(|a| a.action.as_str()).collect::<Vec<_>>())
                .bind(actions.iter().map(|a| a.timestamp.timestamp()).collect::<Vec<_>>())
                .bind(col(&|a| a.started))
                .bind(col(&|a| a.position))
                .bind(col(&|a| a.total))
                .bind(actions.iter().map(|a| a.guid.clone()).collect::<Vec<_>>())
                .bind(resolved)
                .execute(&mut *tx)
                .await?;
                tx.commit().await?;
                targets.any_enabled
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                let targets = crate::services::sync::db::mysql_lock_targets_shared(&mut tx, user_id).await?;
                let mut ids = HashMap::new();
                for name in names {
                    ids.insert(name, mysql_ensure_device(&mut tx, user_id, name).await?.id);
                }
                let now: i64 = sqlx::query_scalar(MYSQL_NOW).fetch_one(&mut *tx).await?;
                for (chunk_no, chunk) in actions.chunks(500).enumerate() {
                    let values = vec!["(?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'client')"; chunk.len()].join(", ");
                    let sql = format!(
                        "INSERT INTO GpodderSyncEpisodeActions
                           (UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Started, Position, Total, Guid, EpisodeID, Origin)
                         VALUES {values}"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql));
                    for (i, a) in chunk.iter().enumerate() {
                        q = q
                            .bind(user_id)
                            .bind(a.device.as_deref().and_then(|d| ids.get(d).copied()))
                            .bind(&a.podcast_url)
                            .bind(&a.episode_url)
                            .bind(a.action.as_str())
                            .bind(a.timestamp.timestamp())
                            .bind(now)
                            .bind(a.started)
                            .bind(a.position)
                            .bind(a.total)
                            .bind(&a.guid)
                            .bind(resolved.get(chunk_no * 500 + i).copied().flatten());
                    }
                    q.execute(&mut *tx).await?;
                }
                tx.commit().await?;
                targets.any_enabled
            }
        };
        // Client rows go to the external target too.
        if push {
            crate::services::sync::push::schedule(self, user_id);
        }
        Ok(())
    }

    /// Episodes of the user's podcasts published since `since`, newest first.
    pub(crate) async fn gpodder_new_episodes(&self, user_id: i32, since: i64) -> AppResult<Vec<NewEpisode>> {
        let since = DateTime::from_timestamp(since, 0).unwrap_or_default().naive_utc();
        let rows = match self {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"SELECT e.episodeid AS id, e.episodetitle AS title, e.episodeurl AS url, e.episodedescription AS description,
                          e.episodepubdate AS released, p.podcastname AS ptitle, p.feedurl AS purl, p.websiteurl AS website,
                          p.username, p.password, COALESCE(p.isyoutubechannel, FALSE) AS yt
                   FROM "Episodes" e JOIN "Podcasts" p ON p.podcastid = e.podcastid
                   WHERE p.userid = $1 AND e.episodepubdate >= $2
                   ORDER BY e.episodepubdate DESC LIMIT $3"#,
            )
            .bind(user_id)
            .bind(since)
            .bind(UPDATES_LIMIT)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| new_episode(r, r.try_get("yt")?))
            .collect::<AppResult<Vec<_>>>()?,
            DatabasePool::MySQL(pool) => sqlx::query(
                "SELECT e.EpisodeID AS id, e.EpisodeTitle AS title, e.EpisodeURL AS url, e.EpisodeDescription AS description,
                        e.EpisodePubDate AS released, p.PodcastName AS ptitle, p.FeedURL AS purl, p.WebsiteURL AS website,
                        p.Username AS username, p.Password AS password, p.IsYouTubeChannel AS yt
                 FROM Episodes e JOIN Podcasts p ON p.PodcastID = e.PodcastID
                 WHERE p.UserID = ? AND e.EpisodePubDate >= ?
                 ORDER BY e.EpisodePubDate DESC LIMIT ?",
            )
            .bind(user_id)
            .bind(since)
            .bind(UPDATES_LIMIT)
            .fetch_all(pool)
            .await?
            .iter()
            .map(|r| new_episode(r, r.try_get::<Option<bool>, _>("yt")?.unwrap_or(false)))
            .collect::<AppResult<Vec<_>>>()?,
        };
        Ok(rows.into_iter().flatten().collect())
    }

    /// The latest logged action (by action time) for each of `episode_ids`.
    pub(crate) async fn gpodder_latest_actions(&self, user_id: i32, episode_ids: &[i32]) -> AppResult<HashMap<i32, String>> {
        let mut out = HashMap::new();
        if episode_ids.is_empty() {
            return Ok(out);
        }
        let rows: Vec<(i32, String)> = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_as(
                    r#"SELECT episodeid, action FROM "GpodderSyncEpisodeActions"
                       WHERE userid = $1 AND episodeid = ANY($2) ORDER BY timestamp, actionid"#,
                )
                .bind(user_id)
                .bind(episode_ids)
                .fetch_all(pool)
                .await?
            }
            DatabasePool::MySQL(pool) => {
                let marks = vec!["?"; episode_ids.len()].join(", ");
                let sql = format!(
                    "SELECT EpisodeID, Action FROM GpodderSyncEpisodeActions
                     WHERE UserID = ? AND EpisodeID IN ({marks}) ORDER BY Timestamp, ActionID"
                );
                let mut q = sqlx::query_as(sqlx::AssertSqlSafe(sql)).bind(user_id);
                for id in episode_ids {
                    q = q.bind(id);
                }
                q.fetch_all(pool).await?
            }
        };
        for (id, action) in rows {
            out.insert(id, action.to_ascii_lowercase());
        }
        Ok(out)
    }

    // ---- settings ---------------------------------------------------------------------------

    /// `(key, JSON-encoded value)` pairs stored for `key`.
    pub(crate) async fn gpodder_settings(&self, user_id: i32, key: &SettingsKey<'_>) -> AppResult<Vec<(String, Option<String>)>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_as(
                    r#"SELECT settingkey, settingvalue FROM "GpodderSyncSettings"
                       WHERE userid = $1 AND scope = $2 AND deviceid IS NOT DISTINCT FROM $3
                         AND podcasturl IS NOT DISTINCT FROM $4 AND episodeurl IS NOT DISTINCT FROM $5
                       ORDER BY settingid"#,
                )
                .bind(user_id)
                .bind(key.scope)
                .bind(key.device_id)
                .bind(key.podcast)
                .bind(key.episode)
                .fetch_all(pool)
                .await?
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_as(
                    "SELECT SettingKey, SettingValue FROM GpodderSyncSettings
                     WHERE UserID = ? AND Scope = ? AND DeviceID <=> ? AND PodcastURL <=> ? AND EpisodeURL <=> ?
                     ORDER BY SettingID",
                )
                .bind(user_id)
                .bind(key.scope)
                .bind(key.device_id)
                .bind(key.podcast)
                .bind(key.episode)
                .fetch_all(pool)
                .await?
            }
        })
    }

    /// Replace the values of `set` and delete `remove`, in one transaction.
    pub(crate) async fn gpodder_save_settings(&self, user_id: i32, key: &SettingsKey<'_>, set: &[(String, String)], remove: &[String]) -> AppResult<()> {
        let keys: Vec<&str> = set.iter().map(|(k, _)| k.as_str()).chain(remove.iter().map(String::as_str)).collect();
        match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                sqlx::query(
                    r#"DELETE FROM "GpodderSyncSettings"
                       WHERE userid = $1 AND scope = $2 AND deviceid IS NOT DISTINCT FROM $3
                         AND podcasturl IS NOT DISTINCT FROM $4 AND episodeurl IS NOT DISTINCT FROM $5
                         AND settingkey = ANY($6)"#,
                )
                .bind(user_id)
                .bind(key.scope)
                .bind(key.device_id)
                .bind(key.podcast)
                .bind(key.episode)
                .bind(&keys)
                .execute(&mut *tx)
                .await?;
                for (k, v) in set {
                    sqlx::query(
                        r#"INSERT INTO "GpodderSyncSettings" (userid, scope, deviceid, podcasturl, episodeurl, settingkey, settingvalue)
                           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
                    )
                    .bind(user_id)
                    .bind(key.scope)
                    .bind(key.device_id)
                    .bind(key.podcast)
                    .bind(key.episode)
                    .bind(k)
                    .bind(v)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                for k in &keys {
                    sqlx::query(
                        "DELETE FROM GpodderSyncSettings
                         WHERE UserID = ? AND Scope = ? AND DeviceID <=> ? AND PodcastURL <=> ? AND EpisodeURL <=> ? AND SettingKey = ?",
                    )
                    .bind(user_id)
                    .bind(key.scope)
                    .bind(key.device_id)
                    .bind(key.podcast)
                    .bind(key.episode)
                    .bind(k)
                    .execute(&mut *tx)
                    .await?;
                }
                for (k, v) in set {
                    sqlx::query(
                        "INSERT INTO GpodderSyncSettings (UserID, Scope, DeviceID, PodcastURL, EpisodeURL, SettingKey, SettingValue)
                         VALUES (?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(user_id)
                    .bind(key.scope)
                    .bind(key.device_id)
                    .bind(key.podcast)
                    .bind(key.episode)
                    .bind(k)
                    .bind(v)
                    .execute(&mut *tx)
                    .await?;
                }
                tx.commit().await?;
            }
        }
        Ok(())
    }

    // ---- Pinepods side ------------------------------------------------------------------------

    /// Write the user's current listening state to the log as Pinepods' own `play` actions, so
    /// clients connecting to the server get it on their first sync. Skips episodes whose state is
    /// already logged (a Pinepods action at or after the history row's time). Returns the count.
    pub async fn gpodder_publish_episode_state(&self, user_id: i32) -> AppResult<u64> {
        // Position: the episode's duration if completed, else the saved position.
        let inserted = match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                crate::services::sync::db::pg_lock_targets_shared(&mut tx, user_id).await?;
                let device = crate::services::sync::db::pg_pinepods_device(&mut tx, user_id).await?;
                let n = sqlx::query(
                    r#"INSERT INTO "GpodderSyncEpisodeActions"
                         (userid, deviceid, podcasturl, episodeurl, action, timestamp, serverts, started, position, total, guid, episodeid, origin)
                       SELECT $1, $2, s.feedurl, s.episodeurl, 'play', s.ts, n.t, 0, s.pos, GREATEST(COALESCE(NULLIF(s.duration, 0), s.pos), s.pos),
                              s.guid, s.episodeid, 'pinepods'
                       FROM (
                           SELECT p.feedurl, e.episodeurl, NULLIF(e.episodeguid, '') AS guid, e.episodeid, e.episodeduration AS duration,
                                  CASE WHEN COALESCE(e.completed, FALSE) AND COALESCE(e.episodeduration, 0) > 0
                                       THEN e.episodeduration ELSE COALESCE(h.listenduration, 0) END AS pos,
                                  COALESCE(FLOOR(EXTRACT(EPOCH FROM h.listendate))::BIGINT, 0) AS ts
                           FROM "Episodes" e
                           JOIN "Podcasts" p ON p.podcastid = e.podcastid
                           LEFT JOIN "UserEpisodeHistory" h ON h.episodeid = e.episodeid AND h.userid = $1
                           WHERE p.userid = $1
                             AND (LOWER(p.feedurl) LIKE 'http://%' OR LOWER(p.feedurl) LIKE 'https://%')
                             AND COALESCE(p.username, '') = '' AND COALESCE(p.password, '') = ''
                             AND NOT COALESCE(p.isyoutubechannel, FALSE)
                       ) s, (SELECT FLOOR(EXTRACT(EPOCH FROM CLOCK_TIMESTAMP()))::BIGINT AS t) n
                       WHERE s.pos > 0
                         AND NOT EXISTS (SELECT 1 FROM "GpodderSyncEpisodeActions" a
                                         WHERE a.userid = $1 AND a.episodeid = s.episodeid AND a.origin = 'pinepods' AND a.timestamp >= s.ts)"#,
                )
                .bind(user_id)
                .bind(device)
                .execute(&mut *tx)
                .await?
                .rows_affected();
                tx.commit().await?;
                n
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = pool.begin().await?;
                crate::services::sync::db::mysql_lock_targets_shared(&mut tx, user_id).await?;
                let device = crate::services::sync::db::mysql_pinepods_device(&mut tx, user_id).await?;
                let n = sqlx::query(
                    "INSERT INTO GpodderSyncEpisodeActions
                       (UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Started, Position, Total, Guid, EpisodeID, Origin)
                     SELECT ?, ?, s.FeedURL, s.EpisodeURL, 'play', s.ts, UNIX_TIMESTAMP(), 0, s.pos,
                            GREATEST(COALESCE(NULLIF(s.duration, 0), s.pos), s.pos), s.guid, s.EpisodeID, 'pinepods'
                     FROM (
                         SELECT p.FeedURL, e.EpisodeURL, NULLIF(e.EpisodeGUID, '') AS guid, e.EpisodeID, e.EpisodeDuration AS duration,
                                CASE WHEN COALESCE(e.Completed, FALSE) AND COALESCE(e.EpisodeDuration, 0) > 0
                                     THEN e.EpisodeDuration ELSE COALESCE(h.ListenDuration, 0) END AS pos,
                                COALESCE(TIMESTAMPDIFF(SECOND, '1970-01-01 00:00:00', h.ListenDate), 0) AS ts
                         FROM Episodes e
                         JOIN Podcasts p ON p.PodcastID = e.PodcastID
                         LEFT JOIN UserEpisodeHistory h ON h.EpisodeID = e.EpisodeID AND h.UserID = ?
                         WHERE p.UserID = ?
                           AND (LOWER(p.FeedURL) LIKE 'http://%' OR LOWER(p.FeedURL) LIKE 'https://%')
                           AND COALESCE(p.Username, '') = '' AND COALESCE(p.Password, '') = ''
                           AND NOT COALESCE(p.IsYouTubeChannel, FALSE)
                     ) s
                     WHERE s.pos > 0
                       AND NOT EXISTS (SELECT 1 FROM GpodderSyncEpisodeActions a
                                       WHERE a.UserID = ? AND a.EpisodeID = s.EpisodeID AND a.Origin = 'pinepods' AND a.Timestamp >= s.ts)",
                )
                .bind(user_id)
                .bind(device)
                .bind(user_id)
                .bind(user_id)
                .bind(user_id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
                tx.commit().await?;
                n
            }
        };
        Ok(inserted)
    }

    /// Devices, library and recent actions of the user's internal server, for the statistics view.
    pub async fn gpodder_server_stats(&self, user_id: i32) -> AppResult<GpodderStats> {
        let devices = self
            .gpodder_devices(user_id)
            .await?
            .into_iter()
            .map(|d| GpodderStatsDevice {
                caption: d.caption.filter(|c| !c.is_empty()).unwrap_or_else(|| d.name.clone()),
                kind: d.kind.unwrap_or_else(|| "other".into()),
                name: d.name,
            })
            .collect();
        let info = self.gpodder_podcast_info(user_id).await?;
        let library = self
            .gpodder_library(user_id)
            .await?
            .into_iter()
            .map(|url| {
                let title = info.get(&url).and_then(|i| i.title.clone());
                (url, title)
            })
            .collect();
        type ActionRow = (String, String, String, i64, Option<i32>, Option<String>, String);
        let (recent, total, last): (Vec<ActionRow>, i64, Option<i64>) = match self {
            DatabasePool::Postgres(pool) => (
                sqlx::query_as(
                    r#"SELECT a.podcasturl, a.episodeurl, a.action, a.timestamp, a.position, d.devicename, a.origin
                       FROM "GpodderSyncEpisodeActions" a LEFT JOIN "GpodderDevices" d ON d.deviceid = a.deviceid
                       WHERE a.userid = $1 ORDER BY a.serverts DESC, a.actionid DESC LIMIT 10"#,
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?,
                sqlx::query_scalar(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?,
                sqlx::query_scalar(r#"SELECT MAX(serverts) FROM "GpodderSyncEpisodeActions" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?,
            ),
            DatabasePool::MySQL(pool) => (
                sqlx::query_as(
                    "SELECT a.PodcastURL, a.EpisodeURL, a.Action, a.Timestamp, a.Position, d.DeviceName, a.Origin
                     FROM GpodderSyncEpisodeActions a LEFT JOIN GpodderDevices d ON d.DeviceID = a.DeviceID
                     WHERE a.UserID = ? ORDER BY a.ServerTs DESC, a.ActionID DESC LIMIT 10",
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?,
                sqlx::query_scalar("SELECT COUNT(*) FROM GpodderSyncEpisodeActions WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?,
                sqlx::query_scalar("SELECT MAX(ServerTs) FROM GpodderSyncEpisodeActions WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_one(pool)
                    .await?,
            ),
        };
        Ok(GpodderStats {
            devices,
            library,
            recent_actions: recent
                .into_iter()
                .map(|(podcast, episode, action, timestamp, position, device, origin)| GpodderStatsAction {
                    podcast,
                    episode,
                    action,
                    timestamp,
                    position,
                    device,
                    origin,
                })
                .collect(),
            total_actions: total,
            last_change: last,
        })
    }
}

/// A device as the sync settings show it. `system` = Pinepods' own device (can't be renamed
/// or removed). Times are unix seconds.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub id: i32,
    pub name: String,
    pub caption: Option<String>,
    pub kind: Option<String>,
    pub last_seen: Option<i64>,
    pub system: bool,
}

/// An app password without its hash. Times are unix seconds.
#[derive(Debug, Clone)]
pub struct AppPasswordInfo {
    pub id: i32,
    pub label: String,
    pub created_at: Option<i64>,
    pub last_used_at: Option<i64>,
}

type DeviceInfoRow = (i32, String, Option<String>, Option<String>, Option<i64>, Option<String>);

fn device_info((id, name, caption, kind, last_seen, role): DeviceInfoRow) -> DeviceInfo {
    DeviceInfo { id, name, caption: caption.filter(|c| !c.is_empty()), kind: kind.filter(|k| !k.is_empty()), last_seen, system: role.is_some() }
}

impl DatabasePool {
    // ---- sync settings: devices and app passwords ---------------------------------------------

    /// The username clients log in with.
    pub async fn gpodder_username(&self, user_id: i32) -> AppResult<Option<String>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(r#"SELECT username FROM "Users" WHERE userid = $1"#)
                .bind(user_id)
                .fetch_optional(pool)
                .await?,
            DatabasePool::MySQL(pool) => sqlx::query_scalar("SELECT Username FROM Users WHERE UserID = ?")
                .bind(user_id)
                .fetch_optional(pool)
                .await?,
        })
    }

    /// The user's active devices, Pinepods' own included, with when each last synced.
    pub async fn gpodder_device_list(&self, user_id: i32) -> AppResult<Vec<DeviceInfo>> {
        let rows: Vec<DeviceInfoRow> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT deviceid, devicename, COALESCE(usercaption, devicecaption), devicetype,
                          EXTRACT(EPOCH FROM lastsync::timestamptz)::BIGINT, systemrole
                   FROM "GpodderDevices" WHERE userid = $1 AND COALESCE(isactive, TRUE) ORDER BY deviceid"#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as(
                "SELECT DeviceID, DeviceName, COALESCE(UserCaption, DeviceCaption), DeviceType, CAST(UNIX_TIMESTAMP(LastSync) AS SIGNED), SystemRole
                 FROM GpodderDevices WHERE UserID = ? AND COALESCE(IsActive, TRUE) ORDER BY DeviceID",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
        };
        Ok(rows.into_iter().map(device_info).collect())
    }

    /// One of the user's devices by id.
    pub async fn gpodder_device_by_id(&self, user_id: i32, device_id: i32) -> AppResult<Option<DeviceInfo>> {
        Ok(self.gpodder_device_list(user_id).await?.into_iter().find(|d| d.id == device_id))
    }

    /// Name a client device in Pinepods. Its own caption stays underneath, so the app
    /// configuring the device again doesn't undo it. Pinepods' own device is left alone (false).
    pub async fn gpodder_rename_device(&self, user_id: i32, device_id: i32, caption: &str) -> AppResult<bool> {
        let n = match self {
            DatabasePool::Postgres(pool) => sqlx::query(
                r#"UPDATE "GpodderDevices" SET usercaption = $1 WHERE deviceid = $2 AND userid = $3 AND systemrole IS NULL"#,
            )
            .bind(caption)
            .bind(device_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected(),
            DatabasePool::MySQL(pool) => sqlx::query(
                "UPDATE GpodderDevices SET UserCaption = ? WHERE DeviceID = ? AND UserID = ? AND SystemRole IS NULL",
            )
            .bind(caption)
            .bind(device_id)
            .bind(user_id)
            .execute(pool)
            .await?
            .rows_affected(),
        };
        Ok(n > 0)
    }

    /// Remove a client device. Its log rows stay (device set NULL), its subscription list goes.
    /// The client gets a new device the next time it syncs. Pinepods' own device is never removed.
    pub async fn gpodder_remove_device(&self, user_id: i32, device_id: i32) -> AppResult<bool> {
        // Clearing the device on log rows writes to the log, so lock the targets first.
        let n = match self {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await?;
                crate::services::sync::db::pg_lock_targets_shared(&mut tx, user_id).await?;
                let n = sqlx::query(r#"DELETE FROM "GpodderDevices" WHERE deviceid = $1 AND userid = $2 AND systemrole IS NULL"#)
                    .bind(device_id)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();
                tx.commit().await?;
                n
            }
            DatabasePool::MySQL(pool) => {
                let mut tx = crate::services::sync::events::begin_mysql(pool, user_id, true).await?;
                let n = sqlx::query("DELETE FROM GpodderDevices WHERE DeviceID = ? AND UserID = ? AND SystemRole IS NULL")
                    .bind(device_id)
                    .bind(user_id)
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();
                tx.commit().await?;
                n
            }
        };
        Ok(n > 0)
    }

    pub async fn gpodder_app_password_list(&self, user_id: i32) -> AppResult<Vec<AppPasswordInfo>> {
        let rows: Vec<(i32, String, Option<i64>, Option<i64>)> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT apppasswordid, label, EXTRACT(EPOCH FROM createdat::timestamptz)::BIGINT,
                          EXTRACT(EPOCH FROM lastusedat::timestamptz)::BIGINT
                   FROM "GpodderAppPasswords" WHERE userid = $1 ORDER BY apppasswordid"#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as(
                "SELECT AppPasswordID, Label, CAST(UNIX_TIMESTAMP(CreatedAt) AS SIGNED), CAST(UNIX_TIMESTAMP(LastUsedAt) AS SIGNED)
                 FROM GpodderAppPasswords WHERE UserID = ? ORDER BY AppPasswordID",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
        };
        Ok(rows.into_iter().map(|(id, label, created_at, last_used_at)| AppPasswordInfo { id, label, created_at, last_used_at }).collect())
    }

    /// Store a new app password (its argon2 hash) and return its id.
    pub async fn gpodder_create_app_password(&self, user_id: i32, label: &str, hash: &str) -> AppResult<i32> {
        Ok(match self {
            DatabasePool::Postgres(pool) => sqlx::query_scalar(
                r#"INSERT INTO "GpodderAppPasswords" (userid, label, passwordhash) VALUES ($1, $2, $3) RETURNING apppasswordid"#,
            )
            .bind(user_id)
            .bind(label)
            .bind(hash)
            .fetch_one(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query("INSERT INTO GpodderAppPasswords (UserID, Label, PasswordHash) VALUES (?, ?, ?)")
                .bind(user_id)
                .bind(label)
                .bind(hash)
                .execute(pool)
                .await?
                .last_insert_id() as i32,
        })
    }

    pub async fn gpodder_revoke_app_password(&self, user_id: i32, id: i32) -> AppResult<bool> {
        let n = match self {
            DatabasePool::Postgres(pool) => sqlx::query(r#"DELETE FROM "GpodderAppPasswords" WHERE apppasswordid = $1 AND userid = $2"#)
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?
                .rows_affected(),
            DatabasePool::MySQL(pool) => sqlx::query("DELETE FROM GpodderAppPasswords WHERE AppPasswordID = ? AND UserID = ?")
                .bind(id)
                .bind(user_id)
                .execute(pool)
                .await?
                .rows_affected(),
        };
        Ok(n > 0)
    }
}

fn device_row<R: Row>(r: &R) -> AppResult<Device>
where
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    i32: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    Ok(Device {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        caption: r.try_get("caption")?,
        kind: r.try_get("kind")?,
        subs_cursor: r.try_get("subs_cursor")?,
    })
}

fn stored_action<R: Row>(r: &R) -> AppResult<Option<StoredAction>>
where
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    i32: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    i64: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let Some(action) = ActionKind::parse(&r.try_get::<String, _>("action")?) else { return Ok(None) };
    let ts: i64 = r.try_get("ts")?;
    Ok(Some(StoredAction {
        id: r.try_get("id")?,
        action: EpisodeAction {
            podcast_url: r.try_get("purl")?,
            episode_url: r.try_get("eurl")?,
            guid: r.try_get::<Option<String>, _>("guid")?.filter(|g| !g.is_empty()),
            action,
            timestamp: DateTime::<Utc>::from_timestamp(ts, 0).unwrap_or_default(),
            started: r.try_get("started")?,
            position: r.try_get("position")?,
            total: r.try_get("total")?,
            device: r.try_get("device")?,
        },
        known_duration: r.try_get("duration")?,
    }))
}

fn new_episode<R: Row>(r: &R, yt: bool) -> AppResult<Option<NewEpisode>>
where
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    i32: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    String: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    NaiveDateTime: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
{
    let podcast_url: String = r.try_get::<Option<String>, _>("purl")?.unwrap_or_default();
    let user: Option<String> = r.try_get("username")?;
    let pass: Option<String> = r.try_get("password")?;
    if !shareable_feed(&podcast_url, user.as_deref(), pass.as_deref(), yt) {
        return Ok(None);
    }
    Ok(Some(NewEpisode {
        episode_id: r.try_get("id")?,
        title: r.try_get("title")?,
        url: r.try_get::<Option<String>, _>("url")?.unwrap_or_default(),
        description: r.try_get("description")?,
        released: r.try_get("released")?,
        podcast_title: r.try_get("ptitle")?,
        podcast_url,
        website: r.try_get("website")?,
    }))
}

#[cfg(test)]
mod tests {
    use super::fold_changes;

    #[test]
    fn changes_fold_to_the_last_action_per_url() {
        let rows = vec![
            ("a".to_string(), "add".to_string()),
            ("b".to_string(), "add".to_string()),
            ("a".to_string(), "remove".to_string()),
            ("c".to_string(), "remove".to_string()),
            ("a".to_string(), "add".to_string()),
        ];
        assert_eq!(
            fold_changes(rows),
            vec![("b".to_string(), true), ("c".to_string(), false), ("a".to_string(), true)]
        );
    }
}
