//! Ad/sponsor detection pipeline (#790): feeds an episode's stored transcript segments to the
//! optional `pinepods-ai` sidecar's LLM, which labels ad spans, and stores those as
//! `EpisodeSkipSegments` rows with `Kind='ad'`/`Source='auto-ad'`.
//!
//! Ad segments are content-level (one detection per episode, shared across subscribers). Because
//! this is a multi-user app, the *review/skip* decision is per-user: `EpisodeAdSkipReview` holds
//! each user's per-segment override, falling back to the podcast's `AdSkipAutoActivate` default.

use crate::database::DatabasePool;
use crate::services::ai_client::AiSegment;
use crate::services::{ai_client, ai_settings, transcription};
use serde::Serialize;
use sqlx::Row;
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};
use tracing::{debug, warn};

/// Episodes currently being ad-scanned, so two triggers (e.g. the post-transcription chain and a
/// manual request) don't run the LLM twice for the same episode.
fn in_flight() -> &'static Mutex<HashSet<i32>> {
    static S: OnceLock<Mutex<HashSet<i32>>> = OnceLock::new();
    S.get_or_init(|| Mutex::new(HashSet::new()))
}

/// RAII marker removing the episode from the in-flight set on drop.
struct InFlightGuard(i32);
impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = in_flight().lock() {
            set.remove(&self.0);
        }
    }
}

/// Source tag written to `EpisodeSkipSegments.Source` for AI-detected ads.
pub const SOURCE_AD: &str = "auto-ad";
/// `Kind` value for ad ranges.
pub const KIND_AD: &str = "ad";

/// A skip range enriched with its DB id and (for ads) the requesting user's effective status.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct SkipSegmentView {
    pub segment_id: i32,
    pub kind: String,
    pub start_time: f64,
    pub end_time: f64,
    pub source: String,
    /// For `kind='ad'`: `"active"`/`"confirmed"` (skip), `"pending"`/`"rejected"` (don't skip).
    /// `None` for non-ad kinds (e.g. silence).
    pub status: Option<String>,
}

/// Whether ad detection has already run for this episode (its own guard column).
async fn already_detected(db_pool: &DatabasePool, episode_id: i32) -> bool {
    match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT COALESCE(adsdetected, FALSE) AS d FROM "Episodes" WHERE episodeid = $1"#,
        )
        .bind(episode_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .and_then(|r| r.try_get::<bool, _>("d").ok())
        .unwrap_or(false),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT COALESCE(AdsDetected, 0) AS d FROM Episodes WHERE EpisodeID = ?",
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

/// Replace this episode's ad segments (scoped by Source so silence rows are untouched) and mark
/// it analyzed. Returns the number of ad ranges written.
async fn store_ad_segments(
    db_pool: &DatabasePool,
    episode_id: i32,
    segments: &[(f64, f64)],
) -> Result<usize, String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"DELETE FROM "EpisodeSkipSegments" WHERE episodeid = $1 AND source = $2"#)
                .bind(episode_id)
                .bind(SOURCE_AD)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            for (start, end) in segments {
                sqlx::query(r#"
                    INSERT INTO "EpisodeSkipSegments" (episodeid, kind, starttime, endtime, source)
                    VALUES ($1, $2, $3, $4, $5)
                "#)
                .bind(episode_id)
                .bind(KIND_AD)
                .bind(*start)
                .bind(*end)
                .bind(SOURCE_AD)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            }
            sqlx::query(r#"UPDATE "Episodes" SET adsdetected = TRUE WHERE episodeid = $1"#)
                .bind(episode_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("DELETE FROM EpisodeSkipSegments WHERE EpisodeID = ? AND Source = ?")
                .bind(episode_id)
                .bind(SOURCE_AD)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            for (start, end) in segments {
                sqlx::query("
                    INSERT INTO EpisodeSkipSegments (EpisodeID, Kind, StartTime, EndTime, Source)
                    VALUES (?, ?, ?, ?, ?)
                ")
                .bind(episode_id)
                .bind(KIND_AD)
                .bind(*start)
                .bind(*end)
                .bind(SOURCE_AD)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            }
            sqlx::query("UPDATE Episodes SET AdsDetected = TRUE WHERE EpisodeID = ?")
                .bind(episode_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(segments.len())
}

/// Fetch the episode's stored generated transcript segments, transcribing first if none exist.
async fn ensure_transcript_segments(
    db_pool: &DatabasePool,
    episode_id: i32,
) -> Result<Vec<AiSegment>, String> {
    // Try the stored transcript first.
    if let Some(t) = transcription::get_episode_transcript(db_pool, episode_id).await? {
        if t.status == "complete" {
            if let Some(seg_json) = t.segments {
                let segs: Vec<AiSegment> =
                    serde_json::from_str(&seg_json).map_err(|e| format!("bad transcript segments: {}", e))?;
                if !segs.is_empty() {
                    return Ok(segs);
                }
            }
        }
    }
    // No usable transcript — generate one, then re-read.
    debug!("Ad detection: episode {} has no transcript; transcribing first", episode_id);
    transcription::transcribe_episode(db_pool, episode_id, false, |_| {}).await?;
    let t = transcription::get_episode_transcript(db_pool, episode_id)
        .await?
        .ok_or_else(|| "transcript unavailable after transcription".to_string())?;
    let seg_json = t.segments.ok_or_else(|| "transcript has no segments".to_string())?;
    serde_json::from_str(&seg_json).map_err(|e| format!("bad transcript segments: {}", e))
}

/// Detect ads for an episode: ensure a transcript exists, run the LLM, store the ad ranges.
/// `on_progress` is called with 0.0–1.0 fractions as detection windows complete.
pub async fn detect_episode_ads(
    db_pool: &DatabasePool,
    episode_id: i32,
    force: bool,
    on_progress: impl FnMut(f64),
) -> Result<usize, String> {
    if ai_client::ai_base_url().is_none() {
        return Err("AI service not configured".to_string());
    }
    if !force && already_detected(db_pool, episode_id).await {
        debug!("Episode {} already ad-scanned; skipping", episode_id);
        return Ok(0);
    }

    // Skip if another detection for this episode is already running.
    {
        let mut set = in_flight().lock().map_err(|_| "in-flight lock poisoned".to_string())?;
        if set.contains(&episode_id) {
            debug!("Episode {} ad detection already in progress; skipping", episode_id);
            return Ok(0);
        }
        set.insert(episode_id);
    }
    let _guard = InFlightGuard(episode_id);

    let segments = ensure_transcript_segments(db_pool, episode_id).await?;
    let llm = ai_settings::resolve_llm_spec(db_pool).await?;
    let result = ai_client::detect_ads(&segments, None, &llm, on_progress).await?;
    let spans: Vec<(f64, f64)> = result.segments.iter().map(|s| (s.start, s.end)).collect();
    let n = store_ad_segments(db_pool, episode_id, &spans).await?;
    debug!("Ad detection stored {} ad span(s) for episode {}", n, episode_id);
    Ok(n)
}

/// Chain hook run at the tail of a successful transcription: if any subscriber to the episode's
/// feed opted into auto ad-detection, detect ads once (content-level). Detached; never blocks.
pub fn maybe_detect_ads_after_transcript(db_pool: DatabasePool, episode_id: i32) {
    if ai_client::ai_base_url().is_none() {
        return;
    }
    tokio::spawn(async move {
        let any_opted_in = match db_pool {
            DatabasePool::Postgres(ref pool) => sqlx::query(r#"
                SELECT EXISTS(
                    SELECT 1 FROM "Episodes" e JOIN "Podcasts" p ON e.podcastid = p.podcastid
                    WHERE e.episodeid = $1 AND COALESCE(p.autoaddetect, FALSE) = TRUE
                ) AS any_on
            "#)
            .bind(episode_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .and_then(|r| r.try_get::<bool, _>("any_on").ok())
            .unwrap_or(false),
            DatabasePool::MySQL(ref pool) => sqlx::query("
                SELECT EXISTS(
                    SELECT 1 FROM Episodes e JOIN Podcasts p ON e.PodcastID = p.PodcastID
                    WHERE e.EpisodeID = ? AND COALESCE(p.AutoAdDetect, 0) = 1
                ) AS any_on
            ")
            .bind(episode_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .and_then(|r| r.try_get::<i64, _>("any_on").ok())
            .map(|v| v != 0)
            .unwrap_or(false),
        };

        if any_opted_in {
            if let Err(e) = detect_episode_ads(&db_pool, episode_id, false, |_| {}).await {
                warn!("Auto ad-detection failed for episode {}: {}", episode_id, e);
            }
        }
    });
}

/// Update a podcast's auto-ad-detect opt-in (owner-scoped).
pub async fn set_auto_ad_detect(
    db_pool: &DatabasePool,
    podcast_id: i32,
    user_id: i32,
    enabled: bool,
) -> Result<(), String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"UPDATE "Podcasts" SET autoaddetect = $1 WHERE podcastid = $2 AND userid = $3"#)
                .bind(enabled).bind(podcast_id).bind(user_id)
                .execute(pool).await.map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("UPDATE Podcasts SET AutoAdDetect = ? WHERE PodcastID = ? AND UserID = ?")
                .bind(enabled).bind(podcast_id).bind(user_id)
                .execute(pool).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Read a podcast's auto-ad-detect setting.
pub async fn get_auto_ad_detect(db_pool: &DatabasePool, podcast_id: i32) -> Result<bool, String> {
    let enabled = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT COALESCE(autoaddetect, FALSE) AS a FROM "Podcasts" WHERE podcastid = $1"#,
        )
        .bind(podcast_id).fetch_optional(pool).await.map_err(|e| e.to_string())?
        .and_then(|r| r.try_get::<bool, _>("a").ok()).unwrap_or(false),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT COALESCE(AutoAdDetect, 0) AS a FROM Podcasts WHERE PodcastID = ?",
        )
        .bind(podcast_id).fetch_optional(pool).await.map_err(|e| e.to_string())?
        .and_then(|r| r.try_get::<i8, _>("a").ok()).map(|a| a != 0).unwrap_or(false),
    };
    Ok(enabled)
}

/// Update a podcast's ad-skip auto-activate setting (owner-scoped).
pub async fn set_ad_skip_auto_activate(
    db_pool: &DatabasePool,
    podcast_id: i32,
    user_id: i32,
    enabled: bool,
) -> Result<(), String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"UPDATE "Podcasts" SET adskipautoactivate = $1 WHERE podcastid = $2 AND userid = $3"#)
                .bind(enabled).bind(podcast_id).bind(user_id)
                .execute(pool).await.map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("UPDATE Podcasts SET AdSkipAutoActivate = ? WHERE PodcastID = ? AND UserID = ?")
                .bind(enabled).bind(podcast_id).bind(user_id)
                .execute(pool).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Read a podcast's ad-skip auto-activate setting (default TRUE).
pub async fn get_ad_skip_auto_activate(db_pool: &DatabasePool, podcast_id: i32) -> Result<bool, String> {
    let enabled = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT COALESCE(adskipautoactivate, TRUE) AS a FROM "Podcasts" WHERE podcastid = $1"#,
        )
        .bind(podcast_id).fetch_optional(pool).await.map_err(|e| e.to_string())?
        .and_then(|r| r.try_get::<bool, _>("a").ok()).unwrap_or(true),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT COALESCE(AdSkipAutoActivate, 1) AS a FROM Podcasts WHERE PodcastID = ?",
        )
        .bind(podcast_id).fetch_optional(pool).await.map_err(|e| e.to_string())?
        .and_then(|r| r.try_get::<i8, _>("a").ok()).map(|a| a != 0).unwrap_or(true),
    };
    Ok(enabled)
}

/// Record a user's confirm/deny of an ad segment. `status` must be `confirmed` or `rejected`.
pub async fn set_ad_segment_review(
    db_pool: &DatabasePool,
    user_id: i32,
    segment_id: i32,
    status: &str,
) -> Result<(), String> {
    if status != "confirmed" && status != "rejected" {
        return Err("status must be 'confirmed' or 'rejected'".to_string());
    }
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"
                INSERT INTO "EpisodeAdSkipReview" (userid, segmentid, status)
                VALUES ($1, $2, $3)
                ON CONFLICT (userid, segmentid) DO UPDATE SET status = EXCLUDED.status, createdat = CURRENT_TIMESTAMP
            "#)
            .bind(user_id).bind(segment_id).bind(status)
            .execute(pool).await.map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("
                INSERT INTO EpisodeAdSkipReview (UserID, SegmentID, Status)
                VALUES (?, ?, ?)
                ON DUPLICATE KEY UPDATE Status = VALUES(Status), CreatedAt = CURRENT_TIMESTAMP
            ")
            .bind(user_id).bind(segment_id).bind(status)
            .execute(pool).await.map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Read an episode's skip segments for a specific user, resolving each ad segment's effective
/// status (per-user override, else the podcast's auto-activate default). Silence segments carry
/// `status = None`. Replaces the plain `audio_processing::get_episode_skip_segments` on the
/// client-facing read path so the player and the transcript review UI share one shape.
pub async fn get_episode_skip_segments_for_user(
    db_pool: &DatabasePool,
    user_id: i32,
    episode_id: i32,
) -> Result<Vec<SkipSegmentView>, String> {
    let rows: Vec<(i32, String, f64, f64, String, Option<String>, bool)> = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(r#"
            SELECT s.segmentid, s.kind, s.starttime, s.endtime, s.source,
                   r.status AS review_status,
                   COALESCE(p.adskipautoactivate, TRUE) AS auto_activate
            FROM "EpisodeSkipSegments" s
            LEFT JOIN "EpisodeAdSkipReview" r ON r.segmentid = s.segmentid AND r.userid = $2
            LEFT JOIN "Episodes" e ON e.episodeid = s.episodeid
            LEFT JOIN "Podcasts" p ON p.podcastid = e.podcastid AND p.userid = $2
            WHERE s.episodeid = $1
            ORDER BY s.starttime
        "#)
        .bind(episode_id).bind(user_id)
        .fetch_all(pool).await.map_err(|e| e.to_string())?
        .into_iter()
        .map(|r| (
            r.try_get::<i32, _>("segmentid").unwrap_or(0),
            r.try_get::<String, _>("kind").unwrap_or_default(),
            r.try_get::<f64, _>("starttime").unwrap_or(0.0),
            r.try_get::<f64, _>("endtime").unwrap_or(0.0),
            r.try_get::<String, _>("source").unwrap_or_default(),
            r.try_get::<Option<String>, _>("review_status").ok().flatten(),
            r.try_get::<bool, _>("auto_activate").unwrap_or(true),
        ))
        .collect(),
        DatabasePool::MySQL(pool) => sqlx::query(r#"
            SELECT s.SegmentID AS segmentid, s.Kind AS kind, s.StartTime AS starttime,
                   s.EndTime AS endtime, s.Source AS source,
                   r.Status AS review_status,
                   COALESCE(p.AdSkipAutoActivate, 1) AS auto_activate
            FROM EpisodeSkipSegments s
            LEFT JOIN EpisodeAdSkipReview r ON r.SegmentID = s.SegmentID AND r.UserID = ?
            LEFT JOIN Episodes e ON e.EpisodeID = s.EpisodeID
            LEFT JOIN Podcasts p ON p.PodcastID = e.PodcastID AND p.UserID = ?
            WHERE s.EpisodeID = ?
            ORDER BY s.StartTime
        "#)
        .bind(user_id).bind(user_id).bind(episode_id)
        .fetch_all(pool).await.map_err(|e| e.to_string())?
        .into_iter()
        .map(|r| (
            r.try_get::<i32, _>("segmentid").unwrap_or(0),
            r.try_get::<String, _>("kind").unwrap_or_default(),
            r.try_get::<f64, _>("starttime").unwrap_or(0.0),
            r.try_get::<f64, _>("endtime").unwrap_or(0.0),
            r.try_get::<String, _>("source").unwrap_or_default(),
            r.try_get::<Option<String>, _>("review_status").ok().flatten(),
            r.try_get::<i8, _>("auto_activate").map(|v| v != 0).unwrap_or(true),
        ))
        .collect(),
    };

    Ok(rows
        .into_iter()
        .map(|(segment_id, kind, start_time, end_time, source, review_status, auto_activate)| {
            let status = if kind == KIND_AD {
                Some(match review_status {
                    Some(s) => s, // 'confirmed' | 'rejected'
                    None => if auto_activate { "active".to_string() } else { "pending".to_string() },
                })
            } else {
                None
            };
            SkipSegmentView { segment_id, kind, start_time, end_time, source, status }
        })
        .collect())
}
