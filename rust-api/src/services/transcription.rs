//! Transcription pipeline (#726): turns a downloaded episode's audio into a stored transcript
//! by calling the optional `pinepods-ai` sidecar, then persisting the result into
//! `EpisodeTranscripts`.
//!
//! Transcripts are content-level (per episode, deduped across users). The row's `Status` tracks
//! the async lifecycle (`running` → `complete`/`failed`) so a queue view can surface progress.

use crate::database::DatabasePool;
use crate::services::{ai_client, audio_processing};
use serde::Serialize;
use sqlx::Row;
use tracing::{debug, warn};

pub const SOURCE_GENERATED: &str = "generated";

/// A stored transcript as served to clients.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct StoredTranscript {
    pub source: String,
    pub language: Option<String>,
    pub model: Option<String>,
    pub status: String,
    pub full_text: Option<String>,
    /// Raw JSON string of `[{start,end,text}]` segments, or null.
    pub segments: Option<String>,
}

/// Whether a generated transcript already exists OR is in progress for the episode. Used to skip
/// redundant work (and to avoid two triggers racing to transcribe the same episode).
async fn has_complete_transcript(db_pool: &DatabasePool, episode_id: i32) -> bool {
    match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT 1 FROM "EpisodeTranscripts" WHERE episodeid = $1 AND source = $2 AND status IN ('complete','running','pending') LIMIT 1"#,
        )
        .bind(episode_id)
        .bind(SOURCE_GENERATED)
        .fetch_optional(pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT 1 FROM EpisodeTranscripts WHERE EpisodeID = ? AND Source = ? AND Status IN ('complete','running','pending') LIMIT 1",
        )
        .bind(episode_id)
        .bind(SOURCE_GENERATED)
        .fetch_optional(pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false),
    }
}

/// Delete any prior generated transcript rows for an episode (so re-runs don't accumulate).
async fn clear_generated(db_pool: &DatabasePool, episode_id: i32) -> Result<(), String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"DELETE FROM "EpisodeTranscripts" WHERE episodeid = $1 AND source = $2"#)
                .bind(episode_id)
                .bind(SOURCE_GENERATED)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("DELETE FROM EpisodeTranscripts WHERE EpisodeID = ? AND Source = ?")
                .bind(episode_id)
                .bind(SOURCE_GENERATED)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Insert a `running` placeholder row and return its id, so a queue view can show in-progress work.
async fn insert_running(db_pool: &DatabasePool, episode_id: i32) -> Result<i64, String> {
    let id = match db_pool {
        DatabasePool::Postgres(pool) => {
            let row = sqlx::query(r#"
                INSERT INTO "EpisodeTranscripts" (episodeid, source, status)
                VALUES ($1, $2, 'running') RETURNING transcriptid
            "#)
            .bind(episode_id)
            .bind(SOURCE_GENERATED)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
            row.try_get::<i32, _>("transcriptid").map_err(|e| e.to_string())? as i64
        }
        DatabasePool::MySQL(pool) => {
            let res = sqlx::query("INSERT INTO EpisodeTranscripts (EpisodeID, Source, Status) VALUES (?, ?, 'running')")
                .bind(episode_id)
                .bind(SOURCE_GENERATED)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
            res.last_insert_id() as i64
        }
    };
    Ok(id)
}

/// Fill in a completed transcript for a previously-inserted running row.
async fn complete_row(
    db_pool: &DatabasePool,
    transcript_id: i64,
    language: &str,
    model: &str,
    full_text: &str,
    segments_json: &str,
) -> Result<(), String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"
                UPDATE "EpisodeTranscripts"
                SET language = $1, model = $2, transcripttext = $3, segments = $4::jsonb, status = 'complete'
                WHERE transcriptid = $5
            "#)
            .bind(language)
            .bind(model)
            .bind(full_text)
            .bind(segments_json)
            .bind(transcript_id as i32)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query(r#"
                UPDATE EpisodeTranscripts
                SET Language = ?, Model = ?, TranscriptText = ?, Segments = ?, Status = 'complete'
                WHERE TranscriptID = ?
            "#)
            .bind(language)
            .bind(model)
            .bind(full_text)
            .bind(segments_json)
            .bind(transcript_id)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

async fn fail_row(db_pool: &DatabasePool, transcript_id: i64) {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            let _ = sqlx::query(r#"UPDATE "EpisodeTranscripts" SET status = 'failed' WHERE transcriptid = $1"#)
                .bind(transcript_id as i32)
                .execute(pool)
                .await;
        }
        DatabasePool::MySQL(pool) => {
            let _ = sqlx::query("UPDATE EpisodeTranscripts SET Status = 'failed' WHERE TranscriptID = ?")
                .bind(transcript_id)
                .execute(pool)
                .await;
        }
    }
}

/// Temp directory (under the shared downloads mount so the AI sidecar can read it) for audio we
/// fetch only to transcribe and then delete.
const TRANSCRIBE_TMP_DIR: &str = "/opt/pinepods/downloads/.transcribe-tmp";

/// Download an episode's audio to a temp file under the shared downloads mount, so transcription
/// works even when the user hasn't downloaded the episode. Returns the temp path; caller deletes it.
async fn fetch_episode_audio_temp(db_pool: &DatabasePool, episode_id: i32) -> Result<String, String> {
    // Episode URL + optional feed auth.
    let (url, username, password) = match db_pool {
        DatabasePool::Postgres(pool) => {
            let row = sqlx::query(r#"
                SELECT e.episodeurl, p.username, p.password
                FROM "Episodes" e JOIN "Podcasts" p ON e.podcastid = p.podcastid
                WHERE e.episodeid = $1
            "#)
            .bind(episode_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
            (
                row.try_get::<String, _>("episodeurl").map_err(|e| e.to_string())?,
                row.try_get::<Option<String>, _>("username").ok().flatten(),
                row.try_get::<Option<String>, _>("password").ok().flatten(),
            )
        }
        DatabasePool::MySQL(pool) => {
            let row = sqlx::query("
                SELECT e.EpisodeURL, p.Username, p.Password
                FROM Episodes e JOIN Podcasts p ON e.PodcastID = p.PodcastID
                WHERE e.EpisodeID = ?
            ")
            .bind(episode_id)
            .fetch_one(pool)
            .await
            .map_err(|e| e.to_string())?;
            (
                row.try_get::<String, _>("EpisodeURL").map_err(|e| e.to_string())?,
                row.try_get::<Option<String>, _>("Username").ok().flatten(),
                row.try_get::<Option<String>, _>("Password").ok().flatten(),
            )
        }
    };

    // SSRF guard: enclosure URLs come from attacker-controlled feeds.
    crate::services::url_guard::ensure_safe_public_url_async(&url)
        .await
        .map_err(|reason| format!("refusing to fetch episode URL: {}", reason))?;

    std::fs::create_dir_all(TRANSCRIBE_TMP_DIR)
        .map_err(|e| format!("failed to create temp dir: {}", e))?;
    let tmp_path = format!("{}/{}.mp3", TRANSCRIBE_TMP_DIR, episode_id);

    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            match crate::services::url_guard::ensure_safe_public_url(attempt.url().as_str()) {
                Ok(()) => attempt.follow(),
                Err(_) => attempt.stop(),
            }
        }))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client
        .get(&url)
        .header("User-Agent", "PinePods/1.0")
        .header("Accept", "*/*");
    if let (Some(u), Some(p)) = (&username, &password) {
        if !u.is_empty() {
            req = req.basic_auth(u, Some(p));
        }
    }
    let mut resp = req.send().await.map_err(|e| format!("download failed: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("download returned {}", resp.status()));
    }
    let mut file = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        std::io::Write::write_all(&mut file, &chunk).map_err(|e| e.to_string())?;
    }
    Ok(tmp_path)
}

/// Transcribe one episode via the AI sidecar and persist the result.
///
/// `force` re-runs even if a complete transcript already exists. The episode does NOT need to be
/// downloaded — if there's no local file, the audio is fetched to a temp file just for
/// transcription and removed afterward.
pub async fn transcribe_episode(
    db_pool: &DatabasePool,
    episode_id: i32,
    force: bool,
    on_progress: impl FnMut(f64),
) -> Result<(), String> {
    if ai_client::ai_base_url().is_none() {
        return Err("AI service not configured".to_string());
    }
    if !force && has_complete_transcript(db_pool, episode_id).await {
        debug!("Episode {} already transcribed; skipping", episode_id);
        return Ok(());
    }

    // Prefer an existing download; otherwise fetch a temp copy we clean up afterward.
    let (file_path, is_temp) = match audio_processing::downloaded_location(db_pool, episode_id).await? {
        Some(p) if std::path::Path::new(&p).exists() => (p, false),
        _ => (fetch_episode_audio_temp(db_pool, episode_id).await?, true),
    };

    clear_generated(db_pool, episode_id).await?;
    let transcript_id = insert_running(db_pool, episode_id).await?;

    // Use the admin-configured whisper model (AISettings), falling back to the sidecar default.
    let model = crate::services::ai_settings::transcription_model(db_pool).await;
    let result = ai_client::transcribe(&file_path, None, Some(&model), on_progress).await;
    if is_temp {
        let _ = std::fs::remove_file(&file_path); // best-effort cleanup
    }
    match result {
        Ok(result) => {
            let segments_json = serde_json::to_string(
                &result
                    .segments
                    .iter()
                    .map(|s| serde_json::json!({ "start": s.start, "end": s.end, "text": s.text }))
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_else(|_| "[]".to_string());
            complete_row(db_pool, transcript_id, &result.language, &result.model, &result.text, &segments_json).await?;
            debug!("Stored transcript for episode {} ({} segments)", episode_id, result.segments.len());
            // Chain ad detection if any subscriber to this feed opted in (safe against the
            // ad-path's own transcription trigger via an in-flight guard).
            crate::services::ad_detection::maybe_detect_ads_after_transcript(db_pool.clone(), episode_id);
            Ok(())
        }
        Err(e) => {
            fail_row(db_pool, transcript_id).await;
            Err(e)
        }
    }
}

/// Read the stored generated transcript for an episode, if any.
pub async fn get_episode_transcript(
    db_pool: &DatabasePool,
    episode_id: i32,
) -> Result<Option<StoredTranscript>, String> {
    let transcript = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(r#"
            SELECT source, language, model, status, transcripttext, segments::text AS segments_text
            FROM "EpisodeTranscripts"
            WHERE episodeid = $1 AND source = $2
            ORDER BY createdat DESC LIMIT 1
        "#)
        .bind(episode_id)
        .bind(SOURCE_GENERATED)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .map(|r| StoredTranscript {
            source: r.try_get("source").unwrap_or_default(),
            language: r.try_get("language").ok(),
            model: r.try_get("model").ok(),
            status: r.try_get("status").unwrap_or_default(),
            full_text: r.try_get("transcripttext").ok(),
            segments: r.try_get("segments_text").ok(),
        }),
        DatabasePool::MySQL(pool) => sqlx::query(r#"
            SELECT Source, Language, Model, Status, TranscriptText, Segments
            FROM EpisodeTranscripts
            WHERE EpisodeID = ? AND Source = ?
            ORDER BY CreatedAt DESC LIMIT 1
        "#)
        .bind(episode_id)
        .bind(SOURCE_GENERATED)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .map(|r| StoredTranscript {
            source: r.try_get("Source").unwrap_or_default(),
            language: r.try_get("Language").ok(),
            model: r.try_get("Model").ok(),
            status: r.try_get("Status").unwrap_or_default(),
            full_text: r.try_get("TranscriptText").ok(),
            segments: r.try_get("Segments").ok(),
        }),
    };
    Ok(transcript)
}

/// Format a seconds value as an SRT timestamp `HH:MM:SS,mmm`.
fn srt_timestamp(seconds: f64) -> String {
    let ms_total = (seconds * 1000.0).round() as i64;
    let ms = ms_total % 1000;
    let s = (ms_total / 1000) % 60;
    let m = (ms_total / 60_000) % 60;
    let h = ms_total / 3_600_000;
    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

/// Render the stored generated transcript for an episode as SRT, so it can flow through the same
/// transcript UI as feed transcripts. Returns None if there's no completed transcript.
pub async fn get_episode_transcript_srt(
    db_pool: &DatabasePool,
    episode_id: i32,
) -> Result<Option<String>, String> {
    let stored = match get_episode_transcript(db_pool, episode_id).await? {
        Some(t) if t.status == "complete" => t,
        _ => return Ok(None),
    };
    let segments_json = match stored.segments {
        Some(s) => s,
        None => return Ok(None),
    };
    let segments: Vec<serde_json::Value> =
        serde_json::from_str(&segments_json).map_err(|e| e.to_string())?;

    let mut srt = String::new();
    for (i, seg) in segments.iter().enumerate() {
        let start = seg.get("start").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let end = seg.get("end").and_then(|v| v.as_f64()).unwrap_or(start);
        let text = seg.get("text").and_then(|v| v.as_str()).unwrap_or("").trim();
        srt.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            srt_timestamp(start),
            srt_timestamp(end),
            text,
        ));
    }
    Ok(Some(srt))
}

/// Update a podcast's auto-transcribe opt-in (owner-scoped, like the silence-trim setter).
pub async fn set_auto_transcribe(
    db_pool: &DatabasePool,
    podcast_id: i32,
    user_id: i32,
    enabled: bool,
) -> Result<(), String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"UPDATE "Podcasts" SET autotranscribe = $1 WHERE podcastid = $2 AND userid = $3"#)
                .bind(enabled)
                .bind(podcast_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("UPDATE Podcasts SET AutoTranscribe = ? WHERE PodcastID = ? AND UserID = ?")
                .bind(enabled)
                .bind(podcast_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Read a podcast's auto-transcribe setting.
pub async fn get_auto_transcribe(db_pool: &DatabasePool, podcast_id: i32) -> Result<bool, String> {
    let enabled = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT COALESCE(autotranscribe, FALSE) AS a FROM "Podcasts" WHERE podcastid = $1"#,
        )
        .bind(podcast_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .and_then(|r| r.try_get::<bool, _>("a").ok())
        .unwrap_or(false),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT COALESCE(AutoTranscribe, 0) AS a FROM Podcasts WHERE PodcastID = ?",
        )
        .bind(podcast_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?
        .and_then(|r| r.try_get::<i8, _>("a").ok())
        .map(|a| a != 0)
        .unwrap_or(false),
    };
    Ok(enabled)
}

/// Auto-transcribe hook, fired when a new episode arrives (on refresh) or finishes downloading.
/// Runs only if the AI sidecar is configured and the owning podcast opted in. Detached so it
/// never blocks the caller; transcription fetches the audio on demand if it isn't downloaded.
pub fn maybe_transcribe_episode(db_pool: DatabasePool, episode_id: i32) {
    if ai_client::ai_base_url().is_none() {
        return; // AI features disabled — nothing to do
    }
    tokio::spawn(async move {
        // Read the podcast's AutoTranscribe via the episode.
        let podcast_enabled = match db_pool {
            DatabasePool::Postgres(ref pool) => sqlx::query(r#"
                SELECT COALESCE(p.autotranscribe, FALSE) AS a
                FROM "Episodes" e JOIN "Podcasts" p ON e.podcastid = p.podcastid
                WHERE e.episodeid = $1
            "#)
            .bind(episode_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .and_then(|r| r.try_get::<bool, _>("a").ok())
            .unwrap_or(false),
            DatabasePool::MySQL(ref pool) => sqlx::query("
                SELECT COALESCE(p.AutoTranscribe, 0) AS a
                FROM Episodes e JOIN Podcasts p ON e.PodcastID = p.PodcastID
                WHERE e.EpisodeID = ?
            ")
            .bind(episode_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .and_then(|r| r.try_get::<i8, _>("a").ok())
            .map(|a| a != 0)
            .unwrap_or(false),
        };

        if podcast_enabled {
            if let Err(e) = transcribe_episode(&db_pool, episode_id, false, |_| {}).await {
                warn!("Auto-transcription failed for episode {}: {}", episode_id, e);
            }
        }
    });
}
