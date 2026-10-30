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

/// Fingerprint of an audio file's content: SHA-256 of the bytes between any leading ID3v2 tags and
/// a trailing ID3v1 tag. Tagging a download doesn't change it, so a tagged server download and the
/// untagged copy it was fetched as compare equal; a copy with different ads does not.
pub async fn audio_fingerprint(path: &str) -> Result<String, String> {
    let path = path.to_string();
    tokio::task::spawn_blocking(move || fingerprint_file(&path))
        .await
        .map_err(|e| e.to_string())?
}

fn fingerprint_file(path: &str) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).map_err(|e| format!("open {}: {}", path, e))?;
    let len = file.metadata().map_err(|e| e.to_string())?.len();

    // Skip ID3v2 tags (there can be more than one). Header: "ID3", version (2), flags (1),
    // syncsafe size (4); a footer (flag 0x10) adds another 10 bytes.
    let mut start = 0u64;
    loop {
        let mut header = [0u8; 10];
        file.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
        if start + 10 > len || file.read_exact(&mut header).is_err() || &header[..3] != b"ID3" {
            break;
        }
        let size = header[6..10].iter().fold(0u64, |acc, b| (acc << 7) | (*b as u64 & 0x7f));
        let footer = if header[5] & 0x10 != 0 { 10 } else { 0 };
        start += 10 + size + footer;
    }

    let mut end = len;
    if end >= start + 128 {
        let mut tail = [0u8; 3];
        file.seek(SeekFrom::Start(end - 128)).map_err(|e| e.to_string())?;
        if file.read_exact(&mut tail).is_ok() && &tail == b"TAG" {
            end -= 128;
        }
    }
    if start >= end {
        start = 0;
        end = len;
    }

    file.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut remaining = end - start;
    let mut buf = vec![0u8; 1 << 20];
    while remaining > 0 {
        let want = remaining.min(buf.len() as u64) as usize;
        let n = file.read(&mut buf[..want]).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        remaining -= n as u64;
    }
    Ok(hex::encode(hasher.finalize()))
}

/// One user's server download of an episode.
#[derive(Debug, Clone)]
pub struct EpisodeDownload {
    pub user_id: i32,
    pub path: String,
    pub fingerprint: Option<String>,
}

/// Every server download of an episode whose file is still on disk.
pub async fn episode_downloads(db_pool: &DatabasePool, episode_id: i32) -> Result<Vec<EpisodeDownload>, String> {
    let rows: Vec<EpisodeDownload> = match db_pool {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"SELECT userid, downloadedlocation, audiofingerprint FROM "DownloadedEpisodes" WHERE episodeid = $1"#,
        )
        .bind(episode_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|r| Some(EpisodeDownload {
            user_id: r.try_get("userid").ok()?,
            path: r.try_get("downloadedlocation").ok()?,
            fingerprint: r.try_get("audiofingerprint").ok().flatten(),
        }))
        .collect(),
        DatabasePool::MySQL(pool) => sqlx::query(
            "SELECT UserID, DownloadedLocation, AudioFingerprint FROM DownloadedEpisodes WHERE EpisodeID = ?",
        )
        .bind(episode_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|r| Some(EpisodeDownload {
            user_id: r.try_get("UserID").ok()?,
            path: r.try_get("DownloadedLocation").ok()?,
            fingerprint: r.try_get("AudioFingerprint").ok().flatten(),
        }))
        .collect(),
    };
    Ok(rows.into_iter().filter(|d| std::path::Path::new(&d.path).exists()).collect())
}

/// Store a download's fingerprint.
pub async fn set_download_fingerprint(
    db_pool: &DatabasePool,
    episode_id: i32,
    user_id: i32,
    fingerprint: &str,
) -> Result<(), String> {
    match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"UPDATE "DownloadedEpisodes" SET audiofingerprint = $1 WHERE episodeid = $2 AND userid = $3"#)
                .bind(fingerprint)
                .bind(episode_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
        DatabasePool::MySQL(pool) => {
            sqlx::query("UPDATE DownloadedEpisodes SET AudioFingerprint = ? WHERE EpisodeID = ? AND UserID = ?")
                .bind(fingerprint)
                .bind(episode_id)
                .bind(user_id)
                .execute(pool)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// A download's fingerprint, computing and storing it first for downloads made before
/// fingerprints were recorded.
pub async fn download_fingerprint(db_pool: &DatabasePool, download: &EpisodeDownload, episode_id: i32) -> Option<String> {
    if let Some(fp) = &download.fingerprint {
        return Some(fp.clone());
    }
    match audio_fingerprint(&download.path).await {
        Ok(fp) => {
            if let Err(e) = set_download_fingerprint(db_pool, episode_id, download.user_id, &fp).await {
                warn!("Could not store fingerprint for episode {} download: {}", episode_id, e);
            }
            Some(fp)
        }
        Err(e) => {
            warn!("Could not fingerprint {}: {}", download.path, e);
            None
        }
    }
}

/// Instead of fetching an episode again, link `dest` to another user's server download of it,
/// preferring the copy ads were detected on. Dynamically inserted ads differ between fetches, so
/// this keeps every user on the same copy. Returns the size and fingerprint when it reused one.
pub async fn reuse_download(
    db_pool: &DatabasePool,
    episode_id: i32,
    user_id: i32,
    dest: &std::path::Path,
) -> Option<(u64, Option<String>)> {
    let downloads: Vec<EpisodeDownload> = episode_downloads(db_pool, episode_id)
        .await
        .ok()?
        .into_iter()
        .filter(|d| d.user_id != user_id && std::path::Path::new(&d.path) != dest)
        .collect();
    if downloads.is_empty() {
        return None;
    }
    let analyzed = crate::services::transcription::analyzed_audio(db_pool, episode_id)
        .await
        .and_then(|a| a.fingerprint);
    let mut source = None;
    if analyzed.is_some() {
        for d in &downloads {
            if download_fingerprint(db_pool, d, episode_id).await == analyzed {
                source = Some(d.clone());
                break;
            }
        }
    }
    let source = source.unwrap_or_else(|| downloads[0].clone());

    let _ = std::fs::remove_file(dest);
    if let Err(e) = std::fs::hard_link(&source.path, dest) {
        debug!("Hard link to {} failed ({}); copying", source.path, e);
        if let Err(e) = tokio::fs::copy(&source.path, dest).await {
            warn!("Could not reuse {} for episode {}: {}", source.path, episode_id, e);
            return None;
        }
    }
    let size = std::fs::metadata(dest).ok()?.len();
    let fingerprint = match source.fingerprint {
        Some(fp) => Some(fp),
        None => download_fingerprint(db_pool, &source, episode_id).await,
    };
    debug!("Episode {} download for user {} reuses {}", episode_id, user_id, source.path);
    Some((size, fingerprint))
}

/// Read a podcast's silence-trim settings for the podcast owning `episode_id`.
/// Returns `(trim_enabled, threshold_level)`.
async fn podcast_silence_settings(db_pool: &DatabasePool, episode_id: i32) -> Result<(bool, i32), String> {
    let row = match db_pool {
        DatabasePool::Postgres(pool) => {
            sqlx::query(r#"
                SELECT COALESCE(p.trimsilence, FALSE) AS trim, COALESCE(p.silencethreshold, 2) AS thr
                FROM "Episodes" e JOIN "Podcasts" p ON e.feedid = p.feedid
                WHERE e.episodeid = $1
                ORDER BY COALESCE(p.trimsilence, FALSE) DESC LIMIT 1
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
                FROM Episodes e JOIN Podcasts p ON e.FeedID = p.FeedID
                WHERE e.EpisodeID = ?
                ORDER BY COALESCE(p.TrimSilence, 0) DESC LIMIT 1
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

#[cfg(test)]
mod tests {
    use super::fingerprint_file;

    fn id3v2(body_len: u32) -> Vec<u8> {
        // Syncsafe size: 7 bits per byte.
        let size = [(body_len >> 21) as u8 & 0x7f, (body_len >> 14) as u8 & 0x7f, (body_len >> 7) as u8 & 0x7f, body_len as u8 & 0x7f];
        let mut tag = b"ID3\x04\x00\x00".to_vec();
        tag.extend_from_slice(&size);
        tag.extend(std::iter::repeat(b'x').take(body_len as usize));
        tag
    }

    fn fingerprint(bytes: &[u8]) -> String {
        let dir = std::env::temp_dir().join(format!("pinepods-fp-{}-{}", std::process::id(), rand_suffix()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.mp3");
        std::fs::write(&path, bytes).unwrap();
        let fp = fingerprint_file(path.to_str().unwrap()).unwrap();
        let _ = std::fs::remove_dir_all(dir);
        fp
    }

    fn rand_suffix() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    }

    #[test]
    fn tags_dont_change_the_fingerprint_but_audio_does() {
        let audio = b"\xff\xfbframes of audio".to_vec();
        let plain = fingerprint(&audio);

        let mut tagged = id3v2(300);
        tagged.extend(id3v2(20)); // a second tag
        tagged.extend(&audio);
        let mut v1 = b"TAG".to_vec();
        v1.extend(std::iter::repeat(0u8).take(125));
        tagged.extend(v1);
        assert_eq!(fingerprint(&tagged), plain);

        let mut other = audio.clone();
        other.extend(b" plus an inserted ad");
        assert_ne!(fingerprint(&other), plain);
    }
}
