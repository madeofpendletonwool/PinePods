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
/// redundant work (and to avoid two triggers racing to transcribe the same episode). A job still
/// `running` after 12 hours was cut off by a restart and no longer counts.
async fn has_complete_transcript(db_pool: &DatabasePool, episode_id: i32) -> bool {
    match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT 1 FROM "EpisodeTranscripts" WHERE episodeid = $1 AND source = $2
               AND (status = 'complete' OR (status IN ('running','pending') AND createdat > NOW() - INTERVAL '12 hours')) LIMIT 1"#,
        )
        .bind(episode_id)
        .bind(SOURCE_GENERATED)
        .fetch_optional(pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT 1 FROM EpisodeTranscripts WHERE EpisodeID = ? AND Source = ?
             AND (Status = 'complete' OR (Status IN ('running','pending') AND CreatedAt > NOW() - INTERVAL 12 HOUR)) LIMIT 1",
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
    audio: &AudioCopy,
) -> Result<(), String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"
                UPDATE "EpisodeTranscripts"
                SET language = $1, model = $2, transcripttext = $3, segments = $4::jsonb, status = 'complete',
                    audiofingerprint = $5, audioduration = $6, audiosource = $7
                WHERE transcriptid = $8
            "#)
            .bind(language)
            .bind(model)
            .bind(full_text)
            .bind(segments_json)
            .bind(&audio.fingerprint)
            .bind(audio.duration)
            .bind(&audio.source)
            .bind(transcript_id as i32)
            .execute(pool)
            .await
            .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query(r#"
                UPDATE EpisodeTranscripts
                SET Language = ?, Model = ?, TranscriptText = ?, Segments = ?, Status = 'complete',
                    AudioFingerprint = ?, AudioDuration = ?, AudioSource = ?
                WHERE TranscriptID = ?
            "#)
            .bind(language)
            .bind(model)
            .bind(full_text)
            .bind(segments_json)
            .bind(&audio.fingerprint)
            .bind(audio.duration)
            .bind(&audio.source)
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
                FROM "Episodes" e JOIN "Podcasts" p ON e.feedid = p.feedid
                WHERE e.episodeid = $1 LIMIT 1
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
                FROM Episodes e JOIN Podcasts p ON e.FeedID = p.FeedID
                WHERE e.EpisodeID = ? LIMIT 1
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

/// The copy of an episode's audio a transcript (and the ad times found in it) was made from.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct AudioCopy {
    /// [`audio_processing::audio_fingerprint`] of the file, when known.
    pub fingerprint: Option<String>,
    /// Decoded length in seconds.
    pub duration: Option<f64>,
    /// `download` (a server download) or `stream` (fetched from the feed just for this).
    pub source: Option<String>,
}

pub const AUDIO_SOURCE_DOWNLOAD: &str = "download";
pub const AUDIO_SOURCE_STREAM: &str = "stream";

/// The audio copy behind the episode's completed generated transcript, if there is one.
pub async fn analyzed_audio(db_pool: &DatabasePool, episode_id: i32) -> Option<AudioCopy> {
    match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(r#"
            SELECT audiofingerprint, audioduration, audiosource FROM "EpisodeTranscripts"
            WHERE episodeid = $1 AND source = $2 AND status = 'complete'
            ORDER BY createdat DESC LIMIT 1
        "#)
        .bind(episode_id)
        .bind(SOURCE_GENERATED)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|r| AudioCopy {
            fingerprint: r.try_get("audiofingerprint").ok().flatten(),
            duration: r.try_get("audioduration").ok().flatten(),
            source: r.try_get("audiosource").ok().flatten(),
        }),
        DatabasePool::MySQL(pool) => sqlx::query(r#"
            SELECT AudioFingerprint, AudioDuration, AudioSource FROM EpisodeTranscripts
            WHERE EpisodeID = ? AND Source = ? AND Status = 'complete'
            ORDER BY CreatedAt DESC LIMIT 1
        "#)
        .bind(episode_id)
        .bind(SOURCE_GENERATED)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|r| AudioCopy {
            fingerprint: r.try_get("AudioFingerprint").ok().flatten(),
            duration: r.try_get("AudioDuration").ok().flatten(),
            source: r.try_get("AudioSource").ok().flatten(),
        }),
    }
}

/// Transcribe one episode via the AI sidecar and persist the result.
///
/// `force` re-runs even if a complete transcript already exists. Audio comes from a server
/// download when there is one (`prefer_user`'s first), since that is the copy users play; with no
/// download it is fetched from the feed to a temp file and removed afterward. Ad times found on a
/// different copy are cleared, as dynamically inserted ads move between copies.
pub async fn transcribe_episode(
    db_pool: &DatabasePool,
    episode_id: i32,
    force: bool,
    prefer_user: Option<i32>,
    on_progress: impl FnMut(f64),
) -> Result<(), String> {
    if ai_client::ai_base_url().is_none() {
        return Err("AI service not configured".to_string());
    }
    if !force && has_complete_transcript(db_pool, episode_id).await {
        debug!("Episode {} already transcribed; skipping", episode_id);
        return Ok(());
    }

    let downloads = audio_processing::episode_downloads(db_pool, episode_id).await?;
    let download = prefer_user
        .and_then(|u| downloads.iter().find(|d| d.user_id == u))
        .or_else(|| downloads.first());
    let (file_path, is_temp, fingerprint, source) = match download {
        Some(d) => (
            d.path.clone(),
            false,
            audio_processing::download_fingerprint(db_pool, d, episode_id).await,
            AUDIO_SOURCE_DOWNLOAD,
        ),
        None => {
            let tmp = fetch_episode_audio_temp(db_pool, episode_id).await?;
            let fp = audio_processing::audio_fingerprint(&tmp).await.ok();
            (tmp, true, fp, AUDIO_SOURCE_STREAM)
        }
    };

    let previous = analyzed_audio(db_pool, episode_id).await;
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
            let audio = AudioCopy {
                fingerprint,
                duration: (result.duration > 0.0).then_some(result.duration),
                source: Some(source.to_string()),
            };
            complete_row(db_pool, transcript_id, &result.language, &result.model, &result.text, &segments_json, &audio).await?;
            debug!("Stored transcript for episode {} ({} segments, {} copy)", episode_id, result.segments.len(), source);

            // Ads found on another copy don't line up with this one: drop them and detect again.
            let same_copy = matches!((&previous, &audio.fingerprint), (Some(p), Some(fp)) if p.fingerprint.as_deref() == Some(fp.as_str()));
            let mut redetect = false;
            if !same_copy {
                redetect = crate::services::ad_detection::clear_ad_segments(db_pool, episode_id).await?;
            }
            // Chain ad detection if a subscriber opted in, or ads had been found before (safe
            // against the ad path's own transcription trigger via an in-flight guard).
            crate::services::ad_detection::maybe_detect_ads_after_transcript(db_pool.clone(), episode_id, redetect);
            Ok(())
        }
        Err(e) => {
            fail_row(db_pool, transcript_id).await;
            Err(e)
        }
    }
}

/// Read the stored generated transcript for an episode, if any. A job still `running` after 12
/// hours was cut off by a restart and reads as `failed`, so it can be started again.
pub async fn get_episode_transcript(
    db_pool: &DatabasePool,
    episode_id: i32,
) -> Result<Option<StoredTranscript>, String> {
    let transcript = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(r#"
            SELECT source, language, model, transcripttext, segments::text AS segments_text,
                   CASE WHEN status IN ('running','pending') AND createdat < NOW() - INTERVAL '12 hours'
                        THEN 'failed' ELSE status END AS status
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
            SELECT Source, Language, Model, TranscriptText, Segments,
                   CASE WHEN Status IN ('running','pending') AND CreatedAt < NOW() - INTERVAL 12 HOUR
                        THEN 'failed' ELSE Status END AS Status
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

/// Whether any subscription to the episode's feed opted into auto-transcription, and whether one
/// of those also auto-downloads to the server.
async fn auto_transcribe_opt_in(db_pool: &DatabasePool, episode_id: i32) -> (bool, bool) {
    let row = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(r#"
            SELECT COALESCE(BOOL_OR(COALESCE(p.autotranscribe, FALSE)), FALSE) AS t,
                   COALESCE(BOOL_OR(COALESCE(p.autotranscribe, FALSE) AND COALESCE(p.autodownload, FALSE)), FALSE) AS d
            FROM "Episodes" e JOIN "Podcasts" p ON e.feedid = p.feedid
            WHERE e.episodeid = $1
        "#)
        .bind(episode_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|r| (r.try_get::<bool, _>("t").unwrap_or(false), r.try_get::<bool, _>("d").unwrap_or(false))),
        DatabasePool::MySQL(pool) => sqlx::query("
            SELECT CAST(COALESCE(MAX(COALESCE(p.AutoTranscribe, 0)), 0) AS SIGNED) AS t,
                   CAST(COALESCE(MAX(COALESCE(p.AutoTranscribe, 0) AND COALESCE(p.AutoDownload, 0)), 0) AS SIGNED) AS d
            FROM Episodes e JOIN Podcasts p ON e.FeedID = p.FeedID
            WHERE e.EpisodeID = ?
        ")
        .bind(episode_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .map(|r| (
            r.try_get::<i64, _>("t").map(|v| v != 0).unwrap_or(false),
            r.try_get::<i64, _>("d").map(|v| v != 0).unwrap_or(false),
        )),
    };
    row.unwrap_or((false, false))
}

/// Auto-transcribe hook for a newly arrived episode. When an opted-in subscription also
/// auto-downloads, this waits for the download instead ([`after_download`]), so the transcript
/// and ad times come from the copy that gets played.
pub fn maybe_transcribe_new_episode(db_pool: DatabasePool, episode_id: i32) {
    if ai_client::ai_base_url().is_none() {
        return; // AI features disabled — nothing to do
    }
    tokio::spawn(async move {
        let (opted_in, downloads) = auto_transcribe_opt_in(&db_pool, episode_id).await;
        if opted_in && !downloads {
            if let Err(e) = transcribe_episode(&db_pool, episode_id, false, None, |_| {}).await {
                warn!("Auto-transcription failed for episode {}: {}", episode_id, e);
            }
        }
    });
}

/// Hook run when a user's server download finishes. Transcribes it if the feed opted in and has no
/// transcript yet, and redoes the transcript (and ad times) on this download when the existing one
/// came from a copy that no longer exists on the server, e.g. a one-off stream with other ads.
pub fn after_download(db_pool: DatabasePool, episode_id: i32, user_id: i32) {
    if ai_client::ai_base_url().is_none() {
        return;
    }
    tokio::spawn(async move {
        let (opted_in, _) = auto_transcribe_opt_in(&db_pool, episode_id).await;
        let analyzed = analyzed_audio(&db_pool, episode_id).await;
        let had_ads = crate::services::ad_detection::has_ad_segments(&db_pool, episode_id).await;

        let force = match &analyzed {
            None => {
                if !opted_in {
                    return;
                }
                false
            }
            Some(copy) => {
                if !opted_in && !had_ads {
                    return;
                }
                let downloads = audio_processing::episode_downloads(&db_pool, episode_id).await.unwrap_or_default();
                let mut analyzed_copy_on_server = false;
                for d in &downloads {
                    let fp = audio_processing::download_fingerprint(&db_pool, d, episode_id).await;
                    if fp.is_some() && fp == copy.fingerprint {
                        analyzed_copy_on_server = true;
                        break;
                    }
                }
                if analyzed_copy_on_server {
                    return; // this download is that copy, or another user's download still is
                }
                true
            }
        };
        if let Err(e) = transcribe_episode(&db_pool, episode_id, force, Some(user_id), |_| {}).await {
            warn!("Transcription after download failed for episode {}: {}", episode_id, e);
        }
    });
}
