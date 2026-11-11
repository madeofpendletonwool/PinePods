//! Transcripts that feeds publish (`podcast:transcript`), stored so they can be searched like
//! generated ones. A scheduled job fetches the transcripts of episodes that don't have one stored
//! yet, newest first, a batch at a time.

use std::sync::LazyLock;

use futures::StreamExt;
use regex::Regex;
use serde::Serialize;
use sqlx::Row;
use tracing::{debug, info};

use crate::database::dialect::dialect_match;
use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::services::podcasting2::Transcript;

pub const SOURCE_FEED: &str = "feed";
/// Episodes fetched per run.
const BATCH: i64 = 60;
const CONCURRENCY: usize = 4;
const MAX_BYTES: usize = 8 * 1024 * 1024;
/// A transcript that couldn't be fetched or read is tried again after this many days.
const RETRY_DAYS: i64 = 7;
/// Podcasting 2.0 JSON transcripts are often one segment per word; words are joined into
/// segments of about this length.
const JOIN_SECONDS: f64 = 10.0;

/// A timed piece of a transcript, stored in `EpisodeTranscripts.Segments` (same shape as
/// generated transcripts).
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Default, PartialEq)]
pub struct Parsed {
    pub text: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Format {
    Cues,
    Json,
    Html,
    Plain,
}

fn format_of(mime: Option<&str>, body: &str) -> Format {
    let mime = mime.unwrap_or("").to_ascii_lowercase();
    if mime.contains("vtt") || mime.contains("srt") || mime.contains("subrip") {
        return Format::Cues;
    }
    if mime.contains("json") {
        return Format::Json;
    }
    if mime.contains("html") {
        return Format::Html;
    }
    let head = body.trim_start_matches('\u{feff}').trim_start();
    if head.starts_with("WEBVTT") || body.contains("-->") {
        Format::Cues
    } else if head.starts_with('{') {
        Format::Json
    } else if mime.contains("text/plain") {
        Format::Plain
    } else if head.starts_with('<') {
        Format::Html
    } else {
        Format::Plain
    }
}

/// Which of an episode's transcripts to store: timed formats first.
pub fn pick(transcripts: &[Transcript]) -> Option<&Transcript> {
    let rank = |t: &Transcript| {
        let m = t.mime_type.as_deref().unwrap_or("").to_ascii_lowercase();
        if m.contains("vtt") {
            0
        } else if m.contains("srt") || m.contains("subrip") {
            1
        } else if m.contains("json") {
            2
        } else if m.contains("html") {
            3
        } else {
            4
        }
    };
    transcripts.iter().filter(|t| t.url.starts_with("http")).min_by_key(|t| rank(t))
}

/// Read a transcript file (SRT, WebVTT, Podcasting 2.0 JSON, HTML or plain text).
pub fn parse(mime: Option<&str>, body: &str) -> Option<Parsed> {
    let parsed = match format_of(mime, body) {
        Format::Cues => parse_cues(body),
        Format::Json => parse_json(body).unwrap_or_else(|| Parsed { text: clean_text(body), segments: vec![] }),
        Format::Html => Parsed { text: html_text(body), segments: vec![] },
        Format::Plain => Parsed { text: clean_text(body), segments: vec![] },
    };
    (!parsed.text.trim().is_empty()).then_some(parsed)
}

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());
static BLOCK_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)</?(p|div|br|li|h[1-6]|tr|section|article)\b[^>]*>").unwrap());
static SCRIPT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<(script|style)\b.*?</(script|style)>").unwrap());

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn clean_text(s: &str) -> String {
    s.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

fn html_text(body: &str) -> String {
    let no_script = SCRIPT.replace_all(body, " ");
    let breaks = BLOCK_TAG.replace_all(&no_script, "\n");
    clean_text(&decode_entities(&TAG.replace_all(&breaks, " ")))
}

/// `HH:MM:SS,mmm`, `HH:MM:SS.mmm` or `MM:SS.mmm` -> seconds.
fn timestamp(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (h.parse::<f64>().ok()?, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + sec)
}

/// SRT and WebVTT: blocks separated by blank lines, each with a `start --> end` line.
fn parse_cues(body: &str) -> Parsed {
    let body = body.trim_start_matches('\u{feff}').replace("\r\n", "\n").replace('\r', "\n");
    let mut segments = Vec::new();
    for block in body.split("\n\n") {
        let mut lines = block.lines().skip_while(|l| !l.contains("-->"));
        let Some(timing) = lines.next() else { continue };
        let mut sides = timing.split("-->");
        let Some(start) = sides.next().and_then(timestamp) else { continue };
        // WebVTT cue settings follow the end time.
        let end = sides
            .next()
            .and_then(|s| s.split_whitespace().next())
            .and_then(timestamp)
            .unwrap_or(start);
        let text = lines.map(|l| decode_entities(&TAG.replace_all(l, "")).trim().to_string()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ");
        if !text.is_empty() {
            segments.push(Segment { start, end, text });
        }
    }
    let text = segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join("\n");
    Parsed { text, segments }
}

/// Podcasting 2.0 JSON: `{"segments": [{"startTime", "endTime", "body", "speaker"}]}`.
fn parse_json(body: &str) -> Option<Parsed> {
    let v: serde_json::Value = serde_json::from_str(body.trim_start_matches('\u{feff}')).ok()?;
    let raw = v.get("segments")?.as_array()?;
    let mut segments: Vec<Segment> = Vec::new();
    let mut current: Option<(Segment, Option<String>)> = None;
    for s in raw {
        let text = s.get("body").and_then(|b| b.as_str()).unwrap_or("").trim().to_string();
        if text.is_empty() {
            continue;
        }
        let start = s.get("startTime").and_then(|t| t.as_f64()).unwrap_or(0.0);
        let end = s.get("endTime").and_then(|t| t.as_f64()).unwrap_or(start);
        let speaker = s.get("speaker").and_then(|t| t.as_str()).map(str::to_string);
        match current.as_mut() {
            Some((seg, spk)) if *spk == speaker && start - seg.start < JOIN_SECONDS && !ends_sentence(&seg.text) => {
                seg.text.push(' ');
                seg.text.push_str(&text);
                seg.end = end;
            }
            _ => {
                if let Some((seg, _)) = current.take() {
                    segments.push(seg);
                }
                current = Some((Segment { start, end, text }, speaker));
            }
        }
    }
    if let Some((seg, _)) = current {
        segments.push(seg);
    }
    let text = segments.iter().map(|s| s.text.as_str()).collect::<Vec<_>>().join("\n");
    Some(Parsed { text, segments })
}

fn ends_sentence(s: &str) -> bool {
    s.ends_with(['.', '?', '!'])
}

// ---------------------------------------------------------------------------------------------
// Storing
// ---------------------------------------------------------------------------------------------

/// Episodes with feed transcript links and nothing stored for them, newest first. A failed
/// fetch counts as stored until it's [`RETRY_DAYS`] old.
async fn due(db: &DatabasePool) -> AppResult<Vec<(i32, String)>> {
    let retry_before = chrono::Utc::now().naive_utc() - chrono::Duration::days(RETRY_DAYS);
    Ok(dialect_match!(db, {
        pg(pool) => {
            let rows = q::query(r#"
                SELECT e.episodeid, e.transcriptsjson FROM "Episodes" e
                WHERE e.transcriptsjson IS NOT NULL AND e.feedid IS NOT NULL
                  AND EXISTS (SELECT 1 FROM "Podcasts" p WHERE p.feedid = e.feedid)
                  AND NOT EXISTS (
                      SELECT 1 FROM "EpisodeTranscripts" t
                      WHERE t.episodeid = e.episodeid AND t.source = 'feed'
                        AND (t.status = 'complete' OR t.createdat > $1))
                ORDER BY e.episodepubdate DESC LIMIT $2
            "#)
            .bind(retry_before)
            .bind(BATCH)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for r in rows {
                out.push((r.try_get::<i32, _>("episodeid")?, r.try_get::<String, _>("transcriptsjson")?));
            }
            out
        },
        mysql(pool) => {
            let rows = sqlx::query("
                SELECT e.EpisodeID, e.TranscriptsJSON FROM Episodes e
                WHERE e.TranscriptsJSON IS NOT NULL AND e.FeedID IS NOT NULL
                  AND EXISTS (SELECT 1 FROM Podcasts p WHERE p.FeedID = e.FeedID)
                  AND NOT EXISTS (
                      SELECT 1 FROM EpisodeTranscripts t
                      WHERE t.EpisodeID = e.EpisodeID AND t.Source = 'feed'
                        AND (t.Status = 'complete' OR t.CreatedAt > ?))
                ORDER BY e.EpisodePubDate DESC LIMIT ?
            ")
            .bind(retry_before)
            .bind(BATCH)
            .fetch_all(pool)
            .await?;
            let mut out = Vec::with_capacity(rows.len());
            for r in rows {
                out.push((r.try_get::<i32, _>("EpisodeID")?, r.try_get::<String, _>("TranscriptsJSON")?));
            }
            out
        },
    }))
}

/// Replace the episode's feed transcript with `parsed`, or with a failed row when it's None.
pub async fn store(db: &DatabasePool, episode_id: i32, language: Option<&str>, parsed: Option<&Parsed>) -> AppResult<()> {
    let status = if parsed.is_some() { "complete" } else { "failed" };
    let text = parsed.map(|p| p.text.as_str());
    let segments = parsed
        .filter(|p| !p.segments.is_empty())
        .map(|p| serde_json::to_string(&p.segments).unwrap_or_else(|_| "[]".into()));
    let language = language.map(|l| l.chars().take(20).collect::<String>());
    dialect_match!(db, {
        pg(pool) => {
            let mut tx = q::begin(pool).await?;
            q::query(r#"DELETE FROM "EpisodeTranscripts" WHERE episodeid = $1 AND source = 'feed'"#)
                .bind(episode_id)
                .execute(&mut *tx)
                .await?;
            q::query(r#"
                INSERT INTO "EpisodeTranscripts" (episodeid, source, language, transcripttext, segments, status)
                VALUES ($1, 'feed', $2, $3, $4::jsonb, $5)
            "#)
            .bind(episode_id)
            .bind(language)
            .bind(text)
            .bind(segments)
            .bind(status)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        },
        mysql(pool) => {
            let mut tx = pool.begin().await?;
            sqlx::query("DELETE FROM EpisodeTranscripts WHERE EpisodeID = ? AND Source = 'feed'")
                .bind(episode_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("
                INSERT INTO EpisodeTranscripts (EpisodeID, Source, Language, TranscriptText, Segments, Status)
                VALUES (?, 'feed', ?, ?, ?, ?)
            ")
            .bind(episode_id)
            .bind(language)
            .bind(text)
            .bind(segments)
            .bind(status)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        },
    });
    Ok(())
}

async fn fetch(url: &str) -> Result<String, String> {
    crate::services::url_guard::ensure_safe_public_url_async(url).await?;
    let mut resp = crate::services::http::feed_client()
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(|e| e.to_string())?;
    let mut body = Vec::new();
    while let Some(chunk) = resp.chunk().await.map_err(|e| e.to_string())? {
        body.extend_from_slice(&chunk);
        if body.len() > MAX_BYTES {
            return Err("transcript is too large".into());
        }
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// Fetch and read one episode's feed transcript: its language and text, None when there's no
/// usable transcript.
async fn fetch_transcript(episode_id: i32, transcripts_json: &str) -> (Option<String>, Option<Parsed>) {
    let transcripts: Vec<Transcript> = serde_json::from_str(transcripts_json).unwrap_or_default();
    let Some(t) = pick(&transcripts) else { return (None, None) };
    match fetch(&t.url).await {
        Ok(body) => (t.language.clone(), parse(t.mime_type.as_deref(), &body)),
        Err(e) => {
            debug!("Feed transcript for episode {} not fetched from {}: {}", episode_id, t.url, e);
            (None, None)
        }
    }
}

/// The scheduled run: store the next batch of feed transcripts. Fetches run in parallel; the
/// writes one at a time, so concurrent replace-by-episode deletes don't deadlock on MariaDB.
pub async fn run_due(db: &DatabasePool) -> AppResult<()> {
    let due = due(db).await?;
    if due.is_empty() {
        return Ok(());
    }
    let mut fetched = futures::stream::iter(due)
        .map(|(id, json)| async move { (id, fetch_transcript(id, &json).await) })
        .buffer_unordered(CONCURRENCY);
    let (mut seen, mut stored) = (0, 0);
    while let Some((id, (language, parsed))) = fetched.next().await {
        store(db, id, language.as_deref(), parsed.as_ref()).await?;
        seen += 1;
        stored += usize::from(parsed.is_some());
    }
    if stored > 0 {
        info!("Stored {} of {} feed transcripts", stored, seen);
    } else {
        debug!("No feed transcripts stored ({} tried)", seen);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_srt() {
        let srt = "1\r\n00:00:01,000 --> 00:00:04,500\r\nHello <i>there</i>\r\nfriend\r\n\r\n2\r\n01:02:03,250 --> 01:02:05,000\r\nSecond &amp; last\r\n";
        let p = parse(Some("application/srt"), srt).unwrap();
        assert_eq!(p.segments, vec![
            Segment { start: 1.0, end: 4.5, text: "Hello there friend".into() },
            Segment { start: 3723.25, end: 3725.0, text: "Second & last".into() },
        ]);
        assert_eq!(p.text, "Hello there friend\nSecond & last");
    }

    #[test]
    fn reads_webvtt_with_speakers_settings_and_notes() {
        let vtt = "\u{feff}WEBVTT\n\nNOTE made by hand\n\nintro\n00:05.000 --> 00:07.000 align:start\n<v Ann>Hi all</v>\n\n00:01:00.000 --> 00:01:02.000\nBye\n";
        let p = parse(Some("text/vtt"), vtt).unwrap();
        assert_eq!(p.segments.len(), 2);
        assert_eq!(p.segments[0], Segment { start: 5.0, end: 7.0, text: "Hi all".into() });
        assert_eq!(p.segments[1].start, 60.0);
    }

    #[test]
    fn joins_word_level_json_into_sentences() {
        let json = r#"{"version":"1.0.0","segments":[
            {"speaker":"A","startTime":0.0,"endTime":0.4,"body":"Hello"},
            {"speaker":"A","startTime":0.4,"endTime":0.9,"body":"world."},
            {"speaker":"A","startTime":1.0,"endTime":1.3,"body":"Next"},
            {"speaker":"B","startTime":1.3,"endTime":1.6,"body":"Other"}
        ]}"#;
        let p = parse(Some("application/json"), json).unwrap();
        let texts: Vec<&str> = p.segments.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["Hello world.", "Next", "Other"]);
        assert_eq!(p.segments[0].end, 0.9);
    }

    #[test]
    fn reads_html_and_plain_text_without_timings() {
        let html = "<html><style>p{}</style><p>First&nbsp;line</p><p>Second</p></html>";
        let p = parse(Some("text/html"), html).unwrap();
        assert_eq!(p.text, "First line\nSecond");
        assert!(p.segments.is_empty());
        assert_eq!(parse(Some("text/plain"), "  just words \n\n more ").unwrap().text, "just words\nmore");
        assert!(parse(Some("text/plain"), "   \n ").is_none());
    }

    #[test]
    fn sniffs_unlabelled_files() {
        assert_eq!(format_of(None, "WEBVTT\n"), Format::Cues);
        assert_eq!(format_of(Some("application/octet-stream"), "1\n00:00:01,000 --> 00:00:02,000\nx"), Format::Cues);
        assert_eq!(format_of(None, "{\"segments\":[]}"), Format::Json);
        assert_eq!(format_of(None, "<p>x</p>"), Format::Html);
    }

    #[test]
    fn picks_timed_formats_first() {
        let t = |m: &str| Transcript { url: format!("https://x/{m}"), mime_type: Some(m.into()), language: None, rel: None };
        let all = vec![t("text/html"), t("application/json"), t("application/x-subrip"), t("text/vtt")];
        assert_eq!(pick(&all).unwrap().mime_type.as_deref(), Some("text/vtt"));
        assert_eq!(pick(&all[..2]).unwrap().mime_type.as_deref(), Some("application/json"));
        assert!(pick(&[Transcript { url: "ftp://x".into(), mime_type: None, language: None, rel: None }]).is_none());
    }
}
