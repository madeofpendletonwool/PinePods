//! Searching the stored transcripts of a user's episodes.

use super::DatabasePool;
use crate::database::dialect::dialect_match;
use crate::error::AppResult;
use crate::services::transcript_search::{boolean_mode_query, websearch_query, Term};
use serde::Serialize;
#[allow(unused_imports)]
use sqlx::Row;

/// An episode whose transcript matched, in the shape the episode lists use.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct TranscriptSearchEpisode {
    pub podcastid: i32,
    pub podcastname: String,
    pub artworkurl: String,
    pub author: String,
    pub feedurl: String,
    pub userid: i32,
    pub episodeid: i32,
    pub episodetitle: String,
    pub episodedescription: String,
    pub episodeurl: String,
    pub episodeartwork: String,
    /// `YYYY-MM-DDTHH:MM:SS`, empty when unknown.
    pub episodepubdate: String,
    pub episodeduration: i32,
    pub listenduration: i32,
    pub completed: bool,
    pub saved: bool,
    pub queued: bool,
    pub downloaded: bool,
    pub is_youtube: bool,
}

/// A matching transcript: its episode and the text to find the moments in.
pub struct TranscriptMatch {
    pub episode: TranscriptSearchEpisode,
    pub source: String,
    pub text: Option<String>,
    pub segments: Option<String>,
}

/// Matching rows ranked per episode: an episode with both a generated and a feed transcript
/// uses the generated one, whose timings follow the audio the server plays.
const PICK_ONE: &str = "ROW_NUMBER() OVER (PARTITION BY t.episodeid ORDER BY CASE WHEN t.source = 'generated' THEN 0 ELSE 1 END, t.transcriptid DESC)";

fn pub_date(d: Option<chrono::NaiveDateTime>) -> String {
    d.map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string()).unwrap_or_default()
}

macro_rules! read_match {
    ($r:expr, $flag:expr) => {{
        let r = $r;
        TranscriptMatch {
            episode: TranscriptSearchEpisode {
                podcastid: r.try_get("podcastid")?,
                podcastname: r.try_get::<Option<String>, _>("podcastname")?.unwrap_or_default(),
                artworkurl: r.try_get::<Option<String>, _>("artworkurl")?.unwrap_or_default(),
                author: r.try_get::<Option<String>, _>("author")?.unwrap_or_default(),
                feedurl: r.try_get::<Option<String>, _>("feedurl")?.unwrap_or_default(),
                userid: r.try_get("userid")?,
                episodeid: r.try_get("episodeid")?,
                episodetitle: r.try_get::<Option<String>, _>("episodetitle")?.unwrap_or_default(),
                episodedescription: r.try_get::<Option<String>, _>("episodedescription")?.unwrap_or_default(),
                episodeurl: r.try_get::<Option<String>, _>("episodeurl")?.unwrap_or_default(),
                episodeartwork: r.try_get::<Option<String>, _>("episodeartwork")?.unwrap_or_default(),
                episodepubdate: pub_date(r.try_get("episodepubdate").ok().flatten()),
                episodeduration: r.try_get::<Option<i32>, _>("episodeduration")?.unwrap_or(0),
                listenduration: r.try_get::<Option<i32>, _>("listenduration").ok().flatten().unwrap_or(0),
                completed: $flag(&r, "completed"),
                saved: $flag(&r, "saved"),
                queued: $flag(&r, "queued"),
                downloaded: $flag(&r, "downloaded"),
                is_youtube: false,
            },
            source: r.try_get("source")?,
            text: r.try_get("transcripttext")?,
            segments: r.try_get("segments_text")?,
        }
    }};
}

impl DatabasePool {
    /// One page of the user's episodes whose transcripts match `terms`, newest first, and how
    /// many match in all. PostgreSQL and MariaDB/MySQL use their full-text index; SQLite reads the
    /// text (case-insensitive for ASCII).
    pub async fn search_transcripts(&self, user_id: i32, terms: &[Term], limit: i64, offset: i64) -> AppResult<(Vec<TranscriptMatch>, i64)> {
        if !terms.iter().any(|t| !t.negate) {
            return Ok((Vec::new(), 0));
        }
        let (rows, total) = dialect_match!(self, {
            pg(pool) => {
                let lite_terms: Vec<String> = terms.iter().map(|t| t.text.clone()).collect();
                let matches = q::pick(
                    "to_tsvector('simple', COALESCE(t.transcripttext, '')) @@ websearch_to_tsquery('simple', $4)".to_string(),
                    terms
                        .iter()
                        .enumerate()
                        .map(|(i, t)| format!("instr(lower(t.transcripttext), ${}) {}", i + 4, if t.negate { "= 0" } else { "> 0" }))
                        .collect::<Vec<_>>()
                        .join(" AND "),
                );
                let binds: Vec<String> = q::pick(vec![websearch_query(terms)], lite_terms);
                let sql = format!(r#"
                    SELECT p.podcastid, p.podcastname, p.artworkurl, p.author, p.feedurl, p.userid,
                           e.episodeid, e.episodetitle, e.episodedescription, e.episodeurl,
                           CASE
                               WHEN p.usepodcastcoverscustomized = TRUE AND p.usepodcastcovers = TRUE THEN p.artworkurl
                               WHEN u.usepodcastcovers = TRUE THEN p.artworkurl
                               ELSE e.episodeartwork
                           END AS episodeartwork,
                           e.episodepubdate, e.episodeduration,
                           COALESCE(h.listenduration, 0) AS listenduration,
                           COALESCE(ues.completed, FALSE) AS completed,
                           (se.episodeid IS NOT NULL) AS saved,
                           (eq.episodeid IS NOT NULL) AS queued,
                           (de.episodeid IS NOT NULL) AS downloaded,
                           t.source, t.transcripttext, t.segments::text AS segments_text,
                           COUNT(*) OVER() AS total_count
                    FROM (
                        SELECT t.transcriptid, t.episodeid, {PICK_ONE} AS rn
                        FROM "EpisodeTranscripts" t
                        WHERE t.status = 'complete' AND t.episodeid IN (
                            SELECT e2.episodeid FROM "Episodes" e2 JOIN "Podcasts" p2 ON p2.feedid = e2.feedid WHERE p2.userid = $1)
                          AND {matches}
                    ) m
                    JOIN "EpisodeTranscripts" t ON t.transcriptid = m.transcriptid
                    JOIN "Episodes" e ON e.episodeid = m.episodeid
                    JOIN "Podcasts" p ON p.feedid = e.feedid AND p.userid = $1
                    LEFT JOIN "Users" u ON u.userid = p.userid
                    LEFT JOIN "UserEpisodeHistory" h ON h.episodeid = e.episodeid AND h.userid = $1
                    LEFT JOIN "UserEpisodeState" ues ON ues.episodeid = e.episodeid AND ues.userid = $1
                    LEFT JOIN "SavedEpisodes" se ON se.episodeid = e.episodeid AND se.userid = $1
                    LEFT JOIN "EpisodeQueue" eq ON eq.episodeid = e.episodeid AND eq.userid = $1 AND eq.is_youtube = FALSE
                    LEFT JOIN "DownloadedEpisodes" de ON de.episodeid = e.episodeid AND de.userid = $1
                    WHERE m.rn = 1
                    ORDER BY e.episodepubdate DESC, e.episodeid DESC
                    LIMIT $2 OFFSET $3
                "#);
                let mut query = q::query(sqlx::AssertSqlSafe(sql)).bind(user_id).bind(limit).bind(offset);
                for b in binds {
                    query = query.bind(b);
                }
                let rows = query.fetch_all(pool).await?;
                let total = rows.first().map(|r| r.try_get::<i64, _>("total_count")).transpose()?.unwrap_or(0);
                let flag = |r: &q::Row, c: &str| r.try_get::<Option<bool>, _>(c).ok().flatten().unwrap_or(false);
                let mut out = Vec::with_capacity(rows.len());
                for r in rows {
                    out.push(read_match!(r, flag));
                }
                (out, total)
            },
            mysql(pool) => {
                let sql = format!(r#"
                    SELECT p.PodcastID AS podcastid, p.PodcastName AS podcastname, p.ArtworkURL AS artworkurl,
                           p.Author AS author, p.FeedURL AS feedurl, p.UserID AS userid,
                           e.EpisodeID AS episodeid, e.EpisodeTitle AS episodetitle,
                           e.EpisodeDescription AS episodedescription, e.EpisodeURL AS episodeurl,
                           CASE
                               WHEN p.UsePodcastCoversCustomized = TRUE AND p.UsePodcastCovers = TRUE THEN p.ArtworkURL
                               WHEN u.UsePodcastCovers = TRUE THEN p.ArtworkURL
                               ELSE e.EpisodeArtwork
                           END AS episodeartwork,
                           e.EpisodePubDate AS episodepubdate, e.EpisodeDuration AS episodeduration,
                           COALESCE(h.ListenDuration, 0) AS listenduration,
                           CAST(COALESCE(ues.Completed, 0) AS SIGNED) AS completed,
                           CAST(se.EpisodeID IS NOT NULL AS SIGNED) AS saved,
                           CAST(eq.EpisodeID IS NOT NULL AS SIGNED) AS queued,
                           CAST(de.EpisodeID IS NOT NULL AS SIGNED) AS downloaded,
                           t.Source AS source, t.TranscriptText AS transcripttext, CAST(t.Segments AS CHAR) AS segments_text,
                           COUNT(*) OVER() AS total_count
                    FROM (
                        SELECT t.TranscriptID AS transcriptid, t.EpisodeID AS episodeid,
                               {pick} AS rn
                        FROM EpisodeTranscripts t
                        WHERE t.Status = 'complete' AND t.EpisodeID IN (
                            SELECT e2.EpisodeID FROM Episodes e2 JOIN Podcasts p2 ON p2.FeedID = e2.FeedID WHERE p2.UserID = ?)
                          AND MATCH(t.TranscriptText) AGAINST (? IN BOOLEAN MODE)
                    ) m
                    JOIN EpisodeTranscripts t ON t.TranscriptID = m.transcriptid
                    JOIN Episodes e ON e.EpisodeID = m.episodeid
                    JOIN Podcasts p ON p.FeedID = e.FeedID AND p.UserID = ?
                    LEFT JOIN Users u ON u.UserID = p.UserID
                    LEFT JOIN UserEpisodeHistory h ON h.EpisodeID = e.EpisodeID AND h.UserID = ?
                    LEFT JOIN UserEpisodeState ues ON ues.EpisodeID = e.EpisodeID AND ues.UserID = ?
                    LEFT JOIN SavedEpisodes se ON se.EpisodeID = e.EpisodeID AND se.UserID = ?
                    LEFT JOIN EpisodeQueue eq ON eq.EpisodeID = e.EpisodeID AND eq.UserID = ? AND eq.is_youtube = FALSE
                    LEFT JOIN DownloadedEpisodes de ON de.EpisodeID = e.EpisodeID AND de.UserID = ?
                    WHERE m.rn = 1
                    ORDER BY e.EpisodePubDate DESC, e.EpisodeID DESC
                    LIMIT ? OFFSET ?
                "#, pick = PICK_ONE.replace("t.episodeid", "t.EpisodeID").replace("t.source", "t.Source").replace("t.transcriptid", "t.TranscriptID"));
                let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
                    .bind(user_id)
                    .bind(boolean_mode_query(terms))
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(user_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?;
                let total = rows.first().map(|r| r.try_get::<i64, _>("total_count")).transpose()?.unwrap_or(0);
                let flag = |r: &sqlx::mysql::MySqlRow, c: &str| r.try_get::<Option<i64>, _>(c).ok().flatten().unwrap_or(0) != 0;
                let mut out = Vec::with_capacity(rows.len());
                for r in rows {
                    out.push(read_match!(r, flag));
                }
                (out, total)
            },
        });
        Ok((rows, total))
    }

    /// How many of the user's episodes have a transcript that can be searched.
    pub async fn searchable_transcript_count(&self, user_id: i32) -> AppResult<i64> {
        Ok(dialect_match!(self, {
            pg(pool) => q::query_scalar::<i64>(r#"
                SELECT COUNT(DISTINCT t.episodeid) FROM "EpisodeTranscripts" t
                JOIN "Episodes" e ON e.episodeid = t.episodeid
                JOIN "Podcasts" p ON p.feedid = e.feedid
                WHERE p.userid = $1 AND t.status = 'complete' AND t.transcripttext IS NOT NULL
            "#)
            .bind(user_id)
            .fetch_one(pool)
            .await?,
            mysql(pool) => sqlx::query_scalar::<_, i64>("
                SELECT COUNT(DISTINCT t.EpisodeID) FROM EpisodeTranscripts t
                JOIN Episodes e ON e.EpisodeID = t.EpisodeID
                JOIN Podcasts p ON p.FeedID = e.FeedID
                WHERE p.UserID = ? AND t.Status = 'complete' AND t.TranscriptText IS NOT NULL
            ")
            .bind(user_id)
            .fetch_one(pool)
            .await?,
        }))
    }
}
