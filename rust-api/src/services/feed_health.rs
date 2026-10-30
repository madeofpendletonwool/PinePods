// Feed health: what kind of failure a refresh or import hit, and telling the user about a feed
// that keeps failing (once per run of failures) and about it working again.

use std::time::Duration;

use serde::Serialize;
use tracing::warn;

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::task_manager::{MsgSeverity, TaskManager};

/// How long a feed has to keep failing before the user is told.
pub const PROBLEM_AFTER: Duration = Duration::from_secs(24 * 3600);
/// A feed that's gone (404/410) is reported after this many failures instead.
const GONE_AFTER_FAILURES: i32 = 3;
/// Activity message categories. Their title is the podcast's name and their body the error
/// kind; the web app writes the text around them.
pub const CATEGORY_PROBLEM: &str = "feed_problem";
pub const CATEGORY_RECOVERED: &str = "feed_recovered";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum FeedErrorKind {
    /// 404 / 410: the feed isn't there any more.
    Gone,
    /// 401 / 403: the host wants credentials or refuses Pinepods.
    Auth,
    /// The response isn't a feed Pinepods can read.
    NotFeed,
    /// No answer: DNS, connection, TLS or timeout.
    Unreachable,
    /// 5xx from the host.
    Server,
    /// 429 / 503 with Retry-After: the host asked Pinepods to wait.
    Throttled,
    Other,
}

impl FeedErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FeedErrorKind::Gone => "gone",
            FeedErrorKind::Auth => "auth",
            FeedErrorKind::NotFeed => "not_feed",
            FeedErrorKind::Unreachable => "unreachable",
            FeedErrorKind::Server => "server",
            FeedErrorKind::Throttled => "throttled",
            FeedErrorKind::Other => "other",
        }
    }

    pub fn parse(s: &str) -> FeedErrorKind {
        match s {
            "gone" => FeedErrorKind::Gone,
            "auth" => FeedErrorKind::Auth,
            "not_feed" => FeedErrorKind::NotFeed,
            "unreachable" => FeedErrorKind::Unreachable,
            "server" => FeedErrorKind::Server,
            "throttled" => FeedErrorKind::Throttled,
            _ => FeedErrorKind::Other,
        }
    }

    pub fn from_status(status: u16) -> FeedErrorKind {
        match status {
            404 | 410 => FeedErrorKind::Gone,
            401 | 403 | 407 => FeedErrorKind::Auth,
            429 => FeedErrorKind::Throttled,
            500..=599 => FeedErrorKind::Server,
            _ => FeedErrorKind::Other,
        }
    }

    /// The kind of a fetch or import error: the HTTP status when there was one, otherwise
    /// whether the host answered at all.
    pub fn of(e: &AppError) -> FeedErrorKind {
        if let AppError::Http(e) = e {
            return match e.status() {
                Some(status) => FeedErrorKind::from_status(status.as_u16()),
                None if e.is_decode() || e.is_body() => FeedErrorKind::NotFeed,
                None => FeedErrorKind::Unreachable,
            };
        }
        let msg = e.to_string();
        if let Some(status) = http_status_in(&msg) {
            return FeedErrorKind::from_status(status);
        }
        let lower = msg.to_ascii_lowercase();
        if ["parse", "xml", "rss", "not a valid feed", "no channel"].iter().any(|w| lower.contains(w)) {
            FeedErrorKind::NotFeed
        } else if ["timed out", "timeout", "dns", "connect", "resolve"].iter().any(|w| lower.contains(w)) {
            FeedErrorKind::Unreachable
        } else {
            FeedErrorKind::Other
        }
    }
}

/// The status in our own fetch errors ("... HTTP 404", "HTTP 403 Forbidden").
fn http_status_in(msg: &str) -> Option<u16> {
    let rest = &msg[msg.find("HTTP ")? + 5..];
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    (digits.len() == 3).then(|| digits.parse().ok()).flatten()
}

/// A subscription whose feed has a problem, as the user is told about it.
#[derive(Debug, Clone)]
pub struct ProblemFeed {
    pub podcast_id: i32,
    pub user_id: i32,
    pub name: String,
    pub artwork: Option<String>,
    pub kind: FeedErrorKind,
}

/// Link the Activity messages open.
pub const PROBLEMS_LINK: &str = "/settings?section=feed_problems";

/// After failed refreshes of `podcast_ids`: tell each user about feeds that just crossed the
/// problem threshold. Every run of failures is reported once.
pub async fn after_failure(db: &DatabasePool, tasks: &TaskManager, podcast_ids: &[i32]) {
    match db.feed_problems_to_report(podcast_ids).await {
        Ok(feeds) => {
            for f in feeds {
                let push = tasks
                    .push_notification(
                        f.user_id,
                        CATEGORY_PROBLEM,
                        MsgSeverity::Warning,
                        f.name.clone(),
                        Some(f.kind.as_str().to_string()),
                        None,
                        Some(f.podcast_id),
                        f.artwork.clone(),
                        Some(PROBLEMS_LINK.to_string()),
                    )
                    .await;
                if let Err(e) = push {
                    warn!("Telling user {} about feed problem of podcast {} failed: {}", f.user_id, f.podcast_id, e);
                }
            }
        }
        Err(e) => warn!("Checking feeds for problems to report failed: {}", e),
    }
}

/// After a successful refresh: tell users whose feed had been reported that it works again.
pub async fn after_success(tasks: &TaskManager, recovered: Vec<ProblemFeed>) {
    for f in recovered {
        let push = tasks
            .push_notification(
                f.user_id,
                CATEGORY_RECOVERED,
                MsgSeverity::Success,
                f.name.clone(),
                None,
                None,
                Some(f.podcast_id),
                f.artwork.clone(),
                None,
            )
            .await;
        if let Err(e) = push {
            warn!("Telling user {} that podcast {} recovered failed: {}", f.user_id, f.podcast_id, e);
        }
    }
}

/// One of the user's podcasts that's failing right now.
#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct FailingFeed {
    pub podcast_id: i32,
    pub name: String,
    pub feed_url: String,
    pub artwork: Option<String>,
    pub kind: FeedErrorKind,
    /// The last error, as the host or the parser reported it.
    pub error: Option<String>,
    pub failures: i32,
    /// Unix seconds.
    pub failing_since: Option<i64>,
    pub last_attempt: Option<i64>,
    /// Over the reporting threshold (and so reported).
    pub reported: bool,
}

type FailingRow = (i32, String, String, Option<String>, Option<String>, Option<String>, i32, Option<i64>, Option<i64>, bool);

fn failing_feed(r: FailingRow) -> FailingFeed {
    FailingFeed {
        podcast_id: r.0,
        name: r.1,
        feed_url: r.2,
        artwork: r.3.filter(|a| !a.is_empty()),
        kind: FeedErrorKind::parse(r.4.as_deref().unwrap_or("other")),
        error: r.5,
        failures: r.6,
        failing_since: r.7,
        last_attempt: r.8,
        reported: r.9,
    }
}

type ProblemRow = (i32, i32, String, Option<String>, Option<String>);

fn problem_feed(r: ProblemRow) -> ProblemFeed {
    ProblemFeed {
        podcast_id: r.0,
        user_id: r.1,
        name: r.2,
        artwork: r.3.filter(|a| !a.is_empty()),
        kind: FeedErrorKind::parse(r.4.as_deref().unwrap_or("other")),
    }
}

impl DatabasePool {
    /// Feeds among `podcast_ids` that just crossed the threshold, marked as reported. Throttling
    /// alone isn't reported: the host is answering, just asking for time.
    pub async fn feed_problems_to_report(&self, podcast_ids: &[i32]) -> AppResult<Vec<ProblemFeed>> {
        if podcast_ids.is_empty() {
            return Ok(Vec::new());
        }
        let hours = (PROBLEM_AFTER.as_secs() / 3600) as i64;
        match self {
            DatabasePool::Postgres(pool) => {
                let rows: Vec<ProblemRow> = sqlx::query_as(
                    r#"UPDATE "Podcasts" SET problemnotified = TRUE
                       WHERE podcastid = ANY($1) AND NOT problemnotified AND failingsince IS NOT NULL
                         AND COALESCE(lasterrorkind, 'other') <> 'throttled'
                         AND (failingsince <= NOW() - make_interval(hours => $2::INT)
                              OR (lasterrorkind = 'gone' AND consecutivefailures >= $3))
                       RETURNING podcastid, userid, podcastname, artworkurl, lasterrorkind"#,
                )
                .bind(podcast_ids)
                .bind(hours as i32)
                .bind(GONE_AFTER_FAILURES)
                .fetch_all(pool)
                .await?;
                Ok(rows.into_iter().map(problem_feed).collect())
            }
            DatabasePool::MySQL(pool) => {
                let placeholders = vec!["?"; podcast_ids.len()].join(", ");
                let sql = format!(
                    "SELECT PodcastID, UserID, PodcastName, ArtworkURL, LastErrorKind FROM Podcasts
                     WHERE PodcastID IN ({placeholders}) AND NOT ProblemNotified AND FailingSince IS NOT NULL
                       AND COALESCE(LastErrorKind, 'other') <> 'throttled'
                       AND (FailingSince <= NOW() - INTERVAL ? HOUR
                            OR (LastErrorKind = 'gone' AND ConsecutiveFailures >= ?))"
                );
                let mut q = sqlx::query_as::<_, ProblemRow>(sqlx::AssertSqlSafe(sql.as_str()));
                for id in podcast_ids {
                    q = q.bind(id);
                }
                let candidates = q.bind(hours).bind(GONE_AFTER_FAILURES).fetch_all(pool).await?;
                let mut reported = Vec::new();
                for row in candidates {
                    // Only the update that flips the flag reports, so two refreshes can't both.
                    let n = sqlx::query("UPDATE Podcasts SET ProblemNotified = TRUE WHERE PodcastID = ? AND NOT ProblemNotified")
                        .bind(row.0)
                        .execute(pool)
                        .await?
                        .rows_affected();
                    if n == 1 {
                        reported.push(problem_feed(row));
                    }
                }
                Ok(reported)
            }
        }
    }

    /// Reported feeds among `podcast_ids` (read before a successful refresh clears the flag).
    pub async fn feed_problems_reported(&self, podcast_ids: &[i32]) -> AppResult<Vec<ProblemFeed>> {
        if podcast_ids.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<ProblemRow> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT podcastid, userid, podcastname, artworkurl, lasterrorkind FROM "Podcasts"
                   WHERE podcastid = ANY($1) AND problemnotified"#,
            )
            .bind(podcast_ids)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => {
                let sql = format!(
                    "SELECT PodcastID, UserID, PodcastName, ArtworkURL, LastErrorKind FROM Podcasts
                     WHERE PodcastID IN ({}) AND ProblemNotified",
                    vec!["?"; podcast_ids.len()].join(", ")
                );
                let mut q = sqlx::query_as::<_, ProblemRow>(sqlx::AssertSqlSafe(sql.as_str()));
                for id in podcast_ids {
                    q = q.bind(id);
                }
                q.fetch_all(pool).await?
            }
        };
        Ok(rows.into_iter().map(problem_feed).collect())
    }

    /// The user's podcasts whose last refresh failed, longest-failing first.
    pub async fn failing_feeds(&self, user_id: i32) -> AppResult<Vec<FailingFeed>> {
        let rows: Vec<FailingRow> = match self {
            DatabasePool::Postgres(pool) => sqlx::query_as(
                r#"SELECT podcastid, podcastname, feedurl, artworkurl, lasterrorkind, lasterrormessage,
                          COALESCE(consecutivefailures, 0),
                          EXTRACT(EPOCH FROM failingsince::timestamptz)::BIGINT,
                          EXTRACT(EPOCH FROM lastrefreshattempt::timestamptz)::BIGINT,
                          problemnotified
                   FROM "Podcasts"
                   WHERE userid = $1 AND COALESCE(consecutivefailures, 0) > 0
                   ORDER BY failingsince NULLS LAST, podcastname"#,
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?,
            DatabasePool::MySQL(pool) => sqlx::query_as::<_, (i32, String, String, Option<String>, Option<String>, Option<String>, i32, Option<i64>, Option<i64>, i8)>(
                "SELECT PodcastID, PodcastName, FeedURL, ArtworkURL, LastErrorKind, LastErrorMessage,
                        COALESCE(ConsecutiveFailures, 0),
                        CAST(TIMESTAMPDIFF(SECOND, '1970-01-01 00:00:00', FailingSince) AS SIGNED),
                        CAST(TIMESTAMPDIFF(SECOND, '1970-01-01 00:00:00', LastRefreshAttempt) AS SIGNED),
                        ProblemNotified
                 FROM Podcasts
                 WHERE UserID = ? AND COALESCE(ConsecutiveFailures, 0) > 0
                 ORDER BY FailingSince IS NULL, FailingSince, PodcastName",
            )
            .bind(user_id)
            .fetch_all(pool)
            .await?
            .into_iter()
            .map(|r| (r.0, r.1, r.2, r.3, r.4, r.5, r.6, r.7, r.8, r.9 != 0))
            .collect(),
        };
        Ok(rows.into_iter().map(failing_feed).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_from_our_fetch_errors() {
        let kind = |m: &str| FeedErrorKind::of(&AppError::external_error(m));
        assert_eq!(kind("Feed gone: HTTP 404"), FeedErrorKind::Gone);
        assert_eq!(kind("Feed gone: HTTP 410"), FeedErrorKind::Gone);
        assert_eq!(kind("Feed request failed: HTTP 403 Forbidden"), FeedErrorKind::Auth);
        assert_eq!(kind("Feed request failed: HTTP 502"), FeedErrorKind::Server);
        assert_eq!(kind("Feed host throttled refresh (HTTP 429)"), FeedErrorKind::Throttled);
        assert_eq!(kind("Failed to parse feed: unexpected end of XML"), FeedErrorKind::NotFeed);
        assert_eq!(kind("operation timed out"), FeedErrorKind::Unreachable);
        assert_eq!(kind("something else"), FeedErrorKind::Other);
    }

    #[test]
    fn kinds_round_trip() {
        for k in [FeedErrorKind::Gone, FeedErrorKind::Auth, FeedErrorKind::NotFeed, FeedErrorKind::Unreachable, FeedErrorKind::Server, FeedErrorKind::Throttled, FeedErrorKind::Other] {
            assert_eq!(FeedErrorKind::parse(k.as_str()), k);
        }
    }
}
