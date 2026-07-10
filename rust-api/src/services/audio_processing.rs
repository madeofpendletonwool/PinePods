//! Server-side audio processing for the skip-segment pipeline.
//!
//! Currently this implements silence detection (#727 "trim silence"). It shells out to the
//! `ffmpeg` binary that already ships in the image (used by yt-dlp `--extract-audio`) and runs
//! the `silencedetect` filter over a locally downloaded episode file, then persists the detected
//! silent ranges into `EpisodeSkipSegments` as `Kind='silence'` rows.
//!
//! Segments are content-level (per episode, not per user): the audio is identical for every
//! subscriber, so detection runs once and `Episodes.SilenceDetected` guards re-analysis. This is
//! the same skip-segment substrate a future ad-detector (#790) would write into.

use crate::database::DatabasePool;
use sqlx::Row;
use tracing::{debug, warn};

/// Source tag written to `EpisodeSkipSegments.Source` for auto-detected silence.
pub const SOURCE_SILENCE: &str = "auto-silence";
/// `Kind` value for silence ranges.
pub const KIND_SILENCE: &str = "silence";

/// Map the per-podcast `SilenceThreshold` preset to `silencedetect` parameters.
///
/// Returns `(noise_floor_db, min_silence_seconds)`. A higher level is more aggressive: it treats
/// shorter and slightly louder gaps as skippable silence.
fn threshold_params(level: i32) -> (f64, f64) {
    match level {
        1 => (-30.0, 1.0), // low: only long, very quiet gaps
        3 => (-45.0, 0.3), // high: shorter/louder gaps count as silence
        _ => (-40.0, 0.5), // medium (default)
    }
}

/// Run `ffmpeg silencedetect` over a local audio file, returning silent `(start, end)` ranges
/// in seconds. Never errors on "no silence" — that is a valid empty result.
pub async fn detect_silence(file_path: &str, level: i32) -> Result<Vec<(f64, f64)>, String> {
    let (noise_db, min_dur) = threshold_params(level);
    let filter = format!("silencedetect=noise={}dB:d={}", noise_db, min_dur);

    debug!("Running ffmpeg silencedetect on {} ({})", file_path, filter);
    let output = tokio::process::Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-nostats")
        .arg("-i")
        .arg(file_path)
        .arg("-af")
        .arg(&filter)
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()
        .await
        .map_err(|e| format!("failed to spawn ffmpeg: {}", e))?;

    // silencedetect emits its markers on stderr regardless of exit status; ffmpeg returns 0 on a
    // successful analysis of a valid file.
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "ffmpeg exited with {}: {}",
            output.status,
            stderr.lines().last().unwrap_or("")
        ));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(parse_silencedetect(&stderr))
}

/// Parse `silence_start` / `silence_end` markers out of ffmpeg's stderr.
///
/// Lines look like:
///   `[silencedetect @ 0x..] silence_start: 12.345`
///   `[silencedetect @ 0x..] silence_end: 18.9 | silence_duration: 6.55`
fn parse_silencedetect(stderr: &str) -> Vec<(f64, f64)> {
    let mut segments = Vec::new();
    let mut current_start: Option<f64> = None;

    for line in stderr.lines() {
        if let Some(idx) = line.find("silence_start:") {
            let rest = &line[idx + "silence_start:".len()..];
            if let Some(v) = rest.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                current_start = Some(v);
            }
        } else if let Some(idx) = line.find("silence_end:") {
            let rest = &line[idx + "silence_end:".len()..];
            if let Some(end) = rest.split_whitespace().next().and_then(|s| s.parse::<f64>().ok()) {
                if let Some(start) = current_start.take() {
                    if end > start {
                        segments.push((start, end));
                    }
                }
            }
        }
    }
    segments
}

/// Look up a locally downloaded file path for an episode (any user — the file content is the same).
pub async fn downloaded_location(db_pool: &DatabasePool, episode_id: i32) -> Result<Option<String>, String> {
    let loc = match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"SELECT downloadedlocation FROM "DownloadedEpisodes" WHERE episodeid = $1 LIMIT 1"#)
                .bind(episode_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?
                .and_then(|row| row.try_get::<String, _>("downloadedlocation").ok())
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("SELECT DownloadedLocation FROM DownloadedEpisodes WHERE EpisodeID = ? LIMIT 1")
                .bind(episode_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?
                .and_then(|row| row.try_get::<String, _>("DownloadedLocation").ok())
        }
    };
    Ok(loc)
}

/// Read a podcast's silence-trim settings for the podcast owning `episode_id`.
/// Returns `(trim_enabled, threshold_level)`.
async fn podcast_silence_settings(db_pool: &DatabasePool, episode_id: i32) -> Result<(bool, i32), String> {
    let row = match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"
                SELECT COALESCE(p.trimsilence, FALSE) AS trim, COALESCE(p.silencethreshold, 2) AS thr
                FROM "Episodes" e JOIN "Podcasts" p ON e.podcastid = p.podcastid
                WHERE e.episodeid = $1
            "#)
            .bind(episode_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .map(|r| (
                r.try_get::<bool, _>("trim").unwrap_or(false),
                r.try_get::<i32, _>("thr").unwrap_or(2),
            ))
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("
                SELECT COALESCE(p.TrimSilence, 0) AS trim, COALESCE(p.SilenceThreshold, 2) AS thr
                FROM Episodes e JOIN Podcasts p ON e.PodcastID = p.PodcastID
                WHERE e.EpisodeID = ?
            ")
            .bind(episode_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .map(|r| (
                r.try_get::<i8, _>("trim").unwrap_or(0) != 0,
                r.try_get::<i32, _>("thr").unwrap_or(2),
            ))
        }
    };
    Ok(row.unwrap_or((false, 2)))
}

/// Persist detected silence segments for an episode: clear prior auto-silence rows, insert the new
/// ranges, and mark the episode analyzed. Returns the number of segments written.
async fn store_silence_segments(
    db_pool: &DatabasePool,
    episode_id: i32,
    segments: &[(f64, f64)],
) -> Result<usize, String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"DELETE FROM "EpisodeSkipSegments" WHERE episodeid = $1 AND source = $2"#)
                .bind(episode_id)
                .bind(SOURCE_SILENCE)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            for (start, end) in segments {
                sqlx::query(r#"
                    INSERT INTO "EpisodeSkipSegments" (episodeid, kind, starttime, endtime, source)
                    VALUES ($1, $2, $3, $4, $5)
                "#)
                .bind(episode_id)
                .bind(KIND_SILENCE)
                .bind(*start)
                .bind(*end)
                .bind(SOURCE_SILENCE)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            }
            sqlx::query(r#"UPDATE "Episodes" SET silencedetected = TRUE WHERE episodeid = $1"#)
                .bind(episode_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("DELETE FROM EpisodeSkipSegments WHERE EpisodeID = ? AND Source = ?")
                .bind(episode_id)
                .bind(SOURCE_SILENCE)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            for (start, end) in segments {
                sqlx::query("
                    INSERT INTO EpisodeSkipSegments (EpisodeID, Kind, StartTime, EndTime, Source)
                    VALUES (?, ?, ?, ?, ?)
                ")
                .bind(episode_id)
                .bind(KIND_SILENCE)
                .bind(*start)
                .bind(*end)
                .bind(SOURCE_SILENCE)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            }
            sqlx::query("UPDATE Episodes SET SilenceDetected = TRUE WHERE EpisodeID = ?")
                .bind(episode_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(segments.len())
}

/// Whether an episode has already been analyzed for silence.
async fn already_detected(db_pool: &DatabasePool, episode_id: i32) -> bool {
    match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT COALESCE(silencedetected, FALSE) AS d FROM "Episodes" WHERE episodeid = $1"#,
        )
        .bind(episode_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<bool, _>("d").ok())
        .unwrap_or(false),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT COALESCE(SilenceDetected, 0) AS d FROM Episodes WHERE EpisodeID = ?",
        )
        .bind(episode_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<i8, _>("d").ok())
        .map(|d| d != 0)
        .unwrap_or(false),
    }
}

/// Analyze one episode's downloaded file for silence and persist the result.
///
/// `force` re-runs even if the episode was already analyzed (used by the manual endpoint). When
/// `threshold_override` is `None`, the owning podcast's `SilenceThreshold` is used.
pub async fn analyze_episode_silence(
    db_pool: &DatabasePool,
    episode_id: i32,
    force: bool,
    threshold_override: Option<i32>,
) -> Result<usize, String> {
    if !force && already_detected(db_pool, episode_id).await {
        debug!("Episode {} already analyzed for silence; skipping", episode_id);
        return Ok(0);
    }

    let file_path = match downloaded_location(db_pool, episode_id).await? {
        Some(p) => p,
        None => return Err(format!("episode {} has no downloaded file to analyze", episode_id)),
    };
    if !std::path::Path::new(&file_path).exists() {
        return Err(format!("downloaded file for episode {} is missing: {}", episode_id, file_path));
    }

    let level = match threshold_override {
        Some(l) => l,
        None => podcast_silence_settings(db_pool, episode_id).await?.1,
    };

    let segments = detect_silence(&file_path, level).await?;
    let count = store_silence_segments(db_pool, episode_id, &segments).await?;
    debug!("Stored {} silence segment(s) for episode {}", count, episode_id);
    Ok(count)
}

/// Read a podcast's silence-trim settings by podcast id. Returns `(enabled, threshold)`.
pub async fn get_trim_silence(
    db_pool: &DatabasePool,
    podcast_id: i32,
) -> Result<(bool, i32), String> {
    let row = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT COALESCE(trimsilence, FALSE) AS trim, COALESCE(silencethreshold, 2) AS thr FROM "Podcasts" WHERE podcastid = $1"#,
        )
        .bind(podcast_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .map(|r| (
            r.try_get::<bool, _>("trim").unwrap_or(false),
            r.try_get::<i32, _>("thr").unwrap_or(2),
        )),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT COALESCE(TrimSilence, 0) AS trim, COALESCE(SilenceThreshold, 2) AS thr FROM Podcasts WHERE PodcastID = ?",
        )
        .bind(podcast_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .map(|r| (
            r.try_get::<i8, _>("trim").unwrap_or(0) != 0,
            r.try_get::<i32, _>("thr").unwrap_or(2),
        )),
    };
    Ok(row.unwrap_or((false, 2)))
}

/// Update a podcast's silence-trim settings (owner-scoped by user_id, like `adjust_skip_times`).
pub async fn set_trim_silence(
    db_pool: &DatabasePool,
    podcast_id: i32,
    user_id: i32,
    enabled: bool,
    threshold: i32,
) -> Result<(), String> {
    let threshold = threshold.clamp(1, 3);
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"UPDATE "Podcasts" SET trimsilence = $1, silencethreshold = $2 WHERE podcastid = $3 AND userid = $4"#)
                .bind(enabled)
                .bind(threshold)
                .bind(podcast_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("UPDATE Podcasts SET TrimSilence = ?, SilenceThreshold = ? WHERE PodcastID = ? AND UserID = ?")
                .bind(enabled)
                .bind(threshold)
                .bind(podcast_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Fire-and-forget hook called after an episode finishes downloading: if the owning podcast has
/// silence-trim enabled and the episode hasn't been analyzed yet, kick off detection in the
/// background so it never blocks the download's completion.
pub fn maybe_detect_silence_after_download(db_pool: DatabasePool, episode_id: i32) {
    tokio::spawn(async move {
        match podcast_silence_settings(&db_pool, episode_id).await {
            Ok((true, _)) => {
                if let Err(e) = analyze_episode_silence(&db_pool, episode_id, false, None).await {
                    warn!("Silence detection failed for episode {}: {}", episode_id, e);
                }
            }
            Ok((false, _)) => {} // trim-silence not enabled for this podcast
            Err(e) => warn!("Could not read silence settings for episode {}: {}", episode_id, e),
        }
    });
}
