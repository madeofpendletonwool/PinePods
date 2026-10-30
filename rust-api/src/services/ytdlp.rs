//! Local, self-updating yt-dlp management (#793).
//!
//! YouTube breaks whenever Google changes its internals, and the fix is always a newer
//! yt-dlp. Instead of rebuilding/releasing the whole image for every yt-dlp bump, the
//! binary now updates itself — daily, at startup, and on demand from the admin settings
//! page — via `yt-dlp --update-to <channel>`. The `nightly` channel is the escape hatch:
//! YouTube fixes usually land there the same day, before a stable release.
//!
//! All yt-dlp invocations across the app resolve the binary through [`ytdlp_binary`], so the
//! updater and the callers (YouTube search / channel / download) always agree on which file
//! to run — the persistent, user-writable managed copy seeded by `startup.sh`.

use crate::database::DatabasePool;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use tokio::process::Command;
use tracing::{info, warn};

/// Path to the yt-dlp binary. `YTDLP_PATH` points at the persistent, uid-911-writable managed
/// copy (seeded under the downloads volume by `startup.sh`) so `--update-to` can replace it in
/// place and survive container recreation; falls back to `yt-dlp` on `PATH` (the baked-in floor).
pub fn ytdlp_binary() -> String {
    std::env::var("YTDLP_PATH").unwrap_or_else(|_| "yt-dlp".to_string())
}

/// yt-dlp processes the feed refresh may run at once. Each one is heavy and talks to YouTube, so
/// this is far below the refresh's feed concurrency.
const REFRESH_PROCESSES: usize = 3;

static REFRESH_PERMITS: std::sync::LazyLock<tokio::sync::Semaphore> =
    std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(REFRESH_PROCESSES));

/// Wait for a turn to run yt-dlp from the refresh.
pub async fn refresh_permit() -> tokio::sync::SemaphorePermit<'static> {
    REFRESH_PERMITS.acquire().await.expect("yt-dlp permits are never closed")
}

/// Normalize an arbitrary channel string to one yt-dlp accepts. Anything other than `nightly`
/// falls back to `stable` so the UI can't persist a value that bricks updates.
pub fn normalize_channel(channel: &str) -> &'static str {
    if channel == "nightly" {
        "nightly"
    } else {
        "stable"
    }
}

/// yt-dlp management config as shown to / updated from the admin settings page.
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct YtDlpSettings {
    pub auto_update: bool,
    pub channel: String, // "stable" | "nightly"
    pub last_updated: Option<String>,
    pub last_result: Option<String>,
}

impl Default for YtDlpSettings {
    fn default() -> Self {
        Self {
            auto_update: true,
            channel: "stable".to_string(),
            last_updated: None,
            last_result: None,
        }
    }
}

/// Update payload from the settings page (only the two user-editable fields).
#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
pub struct YtDlpSettingsUpdate {
    pub auto_update: bool,
    pub channel: String,
}

/// Read the yt-dlp management settings from the `AppSettings` singleton.
pub async fn get_settings(db_pool: &DatabasePool) -> Result<YtDlpSettings, String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            let row = sqlx::query(r#"
                SELECT ytdlpautoupdate AS auto_update, ytdlpchannel AS channel,
                       ytdlplastupdated::text AS last_updated, ytdlplastresult AS last_result
                FROM "AppSettings" LIMIT 1
            "#)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
            let Some(r) = row else { return Ok(YtDlpSettings::default()) };
            Ok(YtDlpSettings {
                auto_update: r.try_get::<Option<bool>, _>("auto_update").ok().flatten().unwrap_or(true),
                channel: r.try_get::<Option<String>, _>("channel").ok().flatten().unwrap_or_else(|| "stable".into()),
                last_updated: r.try_get::<Option<String>, _>("last_updated").ok().flatten(),
                last_result: r.try_get::<Option<String>, _>("last_result").ok().flatten(),
            })
        }
        DatabasePool::MySQL(pool) => {
            let row = sqlx::query(r#"
                SELECT YtDlpAutoUpdate AS auto_update, YtDlpChannel AS channel,
                       CAST(YtDlpLastUpdated AS CHAR) AS last_updated, YtDlpLastResult AS last_result
                FROM AppSettings LIMIT 1
            "#)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
            let Some(r) = row else { return Ok(YtDlpSettings::default()) };
            Ok(YtDlpSettings {
                auto_update: r.try_get::<Option<i32>, _>("auto_update").ok().flatten().unwrap_or(1) == 1,
                channel: r.try_get::<Option<String>, _>("channel").ok().flatten().unwrap_or_else(|| "stable".into()),
                last_updated: r.try_get::<Option<String>, _>("last_updated").ok().flatten(),
                last_result: r.try_get::<Option<String>, _>("last_result").ok().flatten(),
            })
        }
    }
}

/// Persist the two user-editable settings (auto-update toggle + channel). The channel is
/// normalized so only `stable`/`nightly` can be stored.
pub async fn set_settings(db_pool: &DatabasePool, update: &YtDlpSettingsUpdate) -> Result<(), String> {
    let channel = normalize_channel(&update.channel);
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"UPDATE "AppSettings" SET ytdlpautoupdate = $1, ytdlpchannel = $2"#)
                .bind(update.auto_update)
                .bind(channel)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("UPDATE AppSettings SET YtDlpAutoUpdate = ?, YtDlpChannel = ?")
                .bind(update.auto_update as i32)
                .bind(channel)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Record the outcome of an update attempt (timestamp + human-readable result) for the UI.
async fn record_result(db_pool: &DatabasePool, result: &str) -> Result<(), String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"UPDATE "AppSettings" SET ytdlplastupdated = CURRENT_TIMESTAMP, ytdlplastresult = $1"#)
                .bind(result)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("UPDATE AppSettings SET YtDlpLastUpdated = CURRENT_TIMESTAMP, YtDlpLastResult = ?")
                .bind(result)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Current yt-dlp version (`yt-dlp --version`), trimmed. Doubles as a health check.
pub async fn current_version() -> Result<String, String> {
    let bin = ytdlp_binary();
    let output = Command::new(&bin)
        .arg("--version")
        .output()
        .await
        .map_err(|e| format!("failed to execute yt-dlp ({}): {}", bin, e))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp --version failed: {}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Self-update yt-dlp to `channel`, then verify with `--version`. `--update-to` is atomic, so a
/// failed download leaves the working binary in place; we only report the new version on success.
pub async fn run_update(channel: &str) -> Result<String, String> {
    let bin = ytdlp_binary();
    let channel = normalize_channel(channel);
    info!("yt-dlp: updating {} to '{}' channel", bin, channel);

    let output = Command::new(&bin)
        .args(["--update-to", channel])
        .output()
        .await
        .map_err(|e| format!("failed to execute yt-dlp ({}): {}", bin, e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("yt-dlp update failed: {}", stderr.trim()));
    }

    let version = current_version()
        .await
        .map_err(|e| format!("yt-dlp updated but --version failed: {}", e))?;
    info!("yt-dlp updated successfully; version now {}", version);
    Ok(version)
}

/// Update yt-dlp to the configured channel and record the result into `AppSettings`. Shared by
/// the scheduler (daily/startup) and the manual admin trigger. Never returns an error to callers
/// that only care about "did it run" — failures are logged (`warn!`) and persisted for the UI.
pub async fn update_and_record(db_pool: &DatabasePool) -> Result<String, String> {
    let settings = get_settings(db_pool).await.unwrap_or_default();
    let result = run_update(&settings.channel).await;
    let record = match &result {
        Ok(v) => format!("Updated to {} ({} channel)", v, normalize_channel(&settings.channel)),
        Err(e) => {
            warn!("yt-dlp update failed (keeping existing binary): {}", e);
            format!("Update failed: {}", e)
        }
    };
    if let Err(e) = record_result(db_pool, &record).await {
        warn!("yt-dlp: failed to persist last-update result: {}", e);
    }
    result
}

/// Log the current yt-dlp version at startup (visibility for debugging YouTube breakage).
pub async fn log_version_at_startup() {
    match current_version().await {
        Ok(v) => info!("yt-dlp binary in use: {} (version {})", ytdlp_binary(), v),
        Err(e) => warn!("yt-dlp binary not usable ({}): {}", ytdlp_binary(), e),
    }
}
