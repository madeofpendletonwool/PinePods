//! The inbox (episodes the user hasn't dealt with yet) and episode labels.
//!
//! Nothing is written when an episode arrives: the inbox is every episode of a subscribed
//! podcast with its inbox on, released in the last `UserSettings.InboxDays` days, that the user
//! hasn't played, finished, queued, saved, downloaded or dismissed. Doing any of those anywhere
//! takes it out.

use super::*;

/// Days the inbox reaches back when the user hasn't chosen.
pub const INBOX_DEFAULT_DAYS: i32 = 30;
/// The longest the inbox can reach back.
pub const INBOX_MAX_DAYS: i32 = 365;
/// Colours a label can have; clients map them onto their theme.
pub const LABEL_COLORS: &[&str] = &["blue", "green", "yellow", "orange", "red", "purple", "pink", "teal", "gray"];

/// Which inbox episodes a page, count or clear covers.
#[derive(Debug, Clone, Default)]
pub struct InboxFilter {
    /// Only episodes with this label.
    pub label_id: Option<i32>,
    /// Only episodes without any label (ignored with `label_id`).
    pub unlabeled: bool,
    pub podcast_id: Option<i32>,
    /// Oldest first instead of newest first.
    pub oldest_first: bool,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct EpisodeLabel {
    pub label_id: i32,
    pub name: String,
    pub color: String,
    /// Episodes of the user's podcasts that have the label.
    pub episode_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct LabelAssignment {
    pub label_id: i32,
    pub episode_id: i32,
    pub is_youtube: bool,
}

/// Inbox conditions on episodes (`e`) and videos (`v`) of podcast `p`, for one dialect.
struct InboxSql {
    mysql: bool,
}

impl InboxSql {
    fn t(&self, name: &str) -> String {
        if self.mysql { name.to_string() } else { format!("\"{name}\"") }
    }

    fn bool(&self, v: bool) -> &'static str {
        match (self.mysql, v) {
            (true, true) => "1",
            (true, false) => "0",
            (false, true) => "TRUE",
            (false, false) => "FALSE",
        }
    }

    /// The conditions for one side of the union; `id` is the episode or video id column.
    fn conditions(&self, user_id: i32, youtube: bool, filter: &InboxFilter) -> String {
        let yt = self.bool(youtube);
        let id = if youtube { "v.videoid" } else { "e.episodeid" };
        let mut parts = vec![
            format!("p.userid = {user_id}"),
            format!("p.subscribed = {}", self.bool(true)),
            format!("p.inboxenabled = {}", self.bool(true)),
        ];
        if youtube {
            parts.push("v.publishedat >= $1".into());
            parts.push(format!("COALESCE(v.completed, {}) = {}", self.bool(false), self.bool(false)));
            parts.push("COALESCE(v.listenposition, 0) = 0".into());
            parts.push(format!("NOT EXISTS (SELECT 1 FROM {} x WHERE x.userid = p.userid AND x.videoid = v.videoid)", self.t("SavedVideos")));
            parts.push(format!("NOT EXISTS (SELECT 1 FROM {} x WHERE x.userid = p.userid AND x.videoid = v.videoid)", self.t("DownloadedVideos")));
        } else {
            parts.push("e.episodepubdate >= $1".into());
            parts.push(format!(
                "NOT EXISTS (SELECT 1 FROM {} x WHERE x.userid = p.userid AND x.episodeid = e.episodeid AND x.completed = {})",
                self.t("UserEpisodeState"),
                self.bool(true)
            ));
            parts.push(format!(
                "NOT EXISTS (SELECT 1 FROM {} x WHERE x.userid = p.userid AND x.episodeid = e.episodeid AND x.listenduration > 0)",
                self.t("UserEpisodeHistory")
            ));
            parts.push(format!("NOT EXISTS (SELECT 1 FROM {} x WHERE x.userid = p.userid AND x.episodeid = e.episodeid)", self.t("SavedEpisodes")));
            parts.push(format!("NOT EXISTS (SELECT 1 FROM {} x WHERE x.userid = p.userid AND x.episodeid = e.episodeid)", self.t("DownloadedEpisodes")));
        }
        parts.push(format!(
            "NOT EXISTS (SELECT 1 FROM {} x WHERE x.userid = p.userid AND x.episodeid = {id} AND x.is_youtube = {yt})",
            self.t("EpisodeQueue")
        ));
        parts.push(format!(
            "NOT EXISTS (SELECT 1 FROM {} x WHERE x.userid = p.userid AND x.episodeid = {id} AND x.isyoutube = {yt})",
            self.t("InboxDismissed")
        ));
        if let Some(label_id) = filter.label_id {
            parts.push(format!(
                "EXISTS (SELECT 1 FROM {} la WHERE la.userid = p.userid AND la.episodeid = {id} AND la.isyoutube = {yt} AND la.labelid = {label_id})",
                self.t("EpisodeLabelAssignments")
            ));
        } else if filter.unlabeled {
            parts.push(format!(
                "NOT EXISTS (SELECT 1 FROM {} la WHERE la.userid = p.userid AND la.episodeid = {id} AND la.isyoutube = {yt})",
                self.t("EpisodeLabelAssignments")
            ));
        }
        if let Some(podcast_id) = filter.podcast_id {
            parts.push(format!("p.podcastid = {podcast_id}"));
        }
        let sql = parts.join(" AND ");
        // MySQL takes `?` placeholders; the cutoff is the only bound value.
        if self.mysql { sql.replace("$1", "?") } else { sql }
    }

    /// Ids of every inbox item: `episodeid`, `is_youtube`.
    fn ids(&self, user_id: i32, filter: &InboxFilter) -> String {
        format!(
            "SELECT e.episodeid AS episodeid, {f} AS is_youtube FROM {episodes} e JOIN {podcasts} p ON p.feedid = e.feedid WHERE {ep}
             UNION ALL
             SELECT v.videoid AS episodeid, {t} AS is_youtube FROM {videos} v JOIN {podcasts} p ON p.podcastid = v.podcastid WHERE {yt}",
            f = self.bool(false),
            t = self.bool(true),
            episodes = self.t("Episodes"),
            videos = self.t("YouTubeVideos"),
            podcasts = self.t("Podcasts"),
            ep = self.conditions(user_id, false, filter),
            yt = self.conditions(user_id, true, filter),
        )
    }

    /// A page of inbox items in the shape of the feed's episodes.
    fn page(&self, user_id: i32, filter: &InboxFilter) -> String {
        let covers = |own: &str| {
            format!(
                "CASE WHEN p.usepodcastcoverscustomized = {t} AND p.usepodcastcovers = {t} THEN p.artworkurl
                      WHEN u.usepodcastcovers = {t} THEN p.artworkurl
                      ELSE COALESCE({own}, p.artworkurl) END",
                t = self.bool(true)
            )
        };
        let is_video = if self.mysql { "e.IsVideo" } else { "e.is_video" };
        let flag = |v: bool| if self.mysql { format!("CAST({} AS SIGNED)", self.bool(v)) } else { self.bool(v).to_string() };
        let video_flag = if self.mysql { format!("CAST(COALESCE({is_video}, 0) AS SIGNED)") } else { format!("COALESCE({is_video}, FALSE)") };
        let order = if filter.oldest_first { "ASC" } else { "DESC" };
        format!(
            "SELECT * FROM (
                SELECT p.podcastid AS podcastid, p.podcastname AS podcastname, e.episodetitle AS episodetitle,
                       e.episodepubdate AS episodepubdate, COALESCE(e.episodedescription, '') AS episodedescription,
                       {ep_art} AS episodeartwork, e.episodeurl AS episodeurl, COALESCE(e.episodeduration, 0) AS episodeduration,
                       e.episodeid AS episodeid, {no} AS is_youtube, {video_flag} AS is_video
                FROM {episodes} e JOIN {podcasts} p ON p.feedid = e.feedid LEFT JOIN {users} u ON u.userid = p.userid
                WHERE {ep}
                UNION ALL
                SELECT p.podcastid AS podcastid, p.podcastname AS podcastname, v.videotitle AS episodetitle,
                       v.publishedat AS episodepubdate, COALESCE(v.videodescription, '') AS episodedescription,
                       {yt_art} AS episodeartwork, v.videourl AS episodeurl, COALESCE(v.duration, 0) AS episodeduration,
                       v.videoid AS episodeid, {yes} AS is_youtube, {yes} AS is_video
                FROM {videos} v JOIN {podcasts} p ON p.podcastid = v.podcastid LEFT JOIN {users} u ON u.userid = p.userid
                WHERE {yt}
             ) inbox
             ORDER BY episodepubdate {order}, episodeid {order}
             LIMIT {limit_marker} OFFSET {offset_marker}",
            ep_art = covers("e.episodeartwork"),
            yt_art = covers("v.thumbnailurl"),
            no = flag(false),
            yes = flag(true),
            episodes = self.t("Episodes"),
            videos = self.t("YouTubeVideos"),
            podcasts = self.t("Podcasts"),
            users = self.t("Users"),
            ep = self.conditions(user_id, false, filter),
            yt = self.conditions(user_id, true, filter),
            limit_marker = if self.mysql { "?" } else { "$2" },
            offset_marker = if self.mysql { "?" } else { "$3" },
        )
    }
}

fn inbox_cutoff(days: i32) -> chrono::NaiveDateTime {
    let days = days.clamp(1, INBOX_MAX_DAYS);
    (chrono::Utc::now() - chrono::Duration::days(days as i64)).naive_utc()
}

/// `[1,2]` text from a playlist's LabelIDs column; empty when unset or unreadable.
pub fn parse_label_ids(raw: Option<&str>) -> Vec<i32> {
    raw.and_then(|s| serde_json::from_str::<Vec<i32>>(s).ok()).unwrap_or_default()
}

/// A smart playlist condition on episode `e` of podcast `p`: it has any of these labels.
pub fn playlist_label_condition(label_ids: &[i32], mysql: bool) -> Option<String> {
    if label_ids.is_empty() {
        return None;
    }
    let ids = label_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    Some(if mysql {
        format!("EXISTS (SELECT 1 FROM EpisodeLabelAssignments la WHERE la.UserID = p.UserID AND la.EpisodeID = e.EpisodeID AND la.IsYoutube = 0 AND la.LabelID IN ({ids}))")
    } else {
        format!("EXISTS (SELECT 1 FROM \"EpisodeLabelAssignments\" la WHERE la.userid = p.userid AND la.episodeid = e.episodeid AND la.isyoutube = FALSE AND la.labelid IN ({ids}))")
    })
}

impl DatabasePool {
    pub async fn get_inbox_days(&self, user_id: i32) -> AppResult<i32> {
        let days: Option<i32> = dialect_match!(self, {
            pg(pool) => {
                q::query_scalar::<Option<i32>>(r#"SELECT inboxdays FROM "UserSettings" WHERE userid = $1"#)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .flatten()
            },
            mysql(pool) => {
                sqlx::query_scalar::<_, Option<i32>>("SELECT InboxDays FROM UserSettings WHERE UserID = ?")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .flatten()
            },
        });
        Ok(days.filter(|d| *d > 0).unwrap_or(INBOX_DEFAULT_DAYS).min(INBOX_MAX_DAYS))
    }

    pub async fn set_inbox_days(&self, user_id: i32, days: i32) -> AppResult<i32> {
        let days = days.clamp(1, INBOX_MAX_DAYS);
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"INSERT INTO "UserSettings" (userid, inboxdays, theme) VALUES ($1, $2, 'Nordic')
                            ON CONFLICT (userid) DO UPDATE SET inboxdays = EXCLUDED.inboxdays"#)
                    .bind(user_id)
                    .bind(days)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query("INSERT INTO UserSettings (UserID, InboxDays, Theme) VALUES (?, ?, 'Nordic')
                             ON DUPLICATE KEY UPDATE InboxDays = VALUES(InboxDays)")
                    .bind(user_id)
                    .bind(days)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(days)
    }

    /// Whether a podcast's new episodes land in the inbox; `None` when it isn't the user's.
    pub async fn get_podcast_inbox(&self, podcast_id: i32, user_id: i32) -> AppResult<Option<bool>> {
        Ok(dialect_match!(self, {
            pg(pool) => {
                q::query_scalar::<bool>(r#"SELECT inboxenabled FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#)
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
            },
            mysql(pool) => {
                sqlx::query_scalar::<_, i64>("SELECT CAST(InboxEnabled AS SIGNED) FROM Podcasts WHERE PodcastID = ? AND UserID = ?")
                    .bind(podcast_id)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?
                    .map(|v| v != 0)
            },
        }))
    }

    /// False when the podcast isn't the user's.
    pub async fn set_podcast_inbox(&self, podcast_id: i32, user_id: i32, enabled: bool) -> AppResult<bool> {
        let changed = dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "Podcasts" SET inboxenabled = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(enabled)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            },
            mysql(pool) => {
                // MySQL counts only changed rows, so an unchanged value is checked for separately.
                let n = sqlx::query("UPDATE Podcasts SET InboxEnabled = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(enabled)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?
                    .rows_affected();
                if n == 0 && self.get_podcast_inbox(podcast_id, user_id).await?.is_some() { 1 } else { n }
            },
        });
        Ok(changed > 0)
    }

    /// One page of the inbox, and how many items it holds in all.
    pub async fn get_inbox(
        &self,
        user_id: i32,
        filter: &InboxFilter,
        limit: i64,
        offset: i64,
    ) -> AppResult<(Vec<crate::handlers::podcasts::Episode>, i64)> {
        let cutoff = inbox_cutoff(self.get_inbox_days(user_id).await?);
        let episode = |podcastid, podcastname, episodetitle, pubdate: chrono::NaiveDateTime, episodedescription, episodeartwork, episodeurl, episodeduration, episodeid, is_youtube, is_video| {
            crate::handlers::podcasts::Episode {
                podcastid,
                podcastname,
                episodetitle,
                episodepubdate: pubdate.format("%Y-%m-%dT%H:%M:%S").to_string(),
                episodedescription,
                episodeartwork,
                episodeurl,
                episodeduration,
                listenduration: None,
                episodeid,
                completed: false,
                saved: false,
                queued: false,
                downloaded: false,
                is_youtube,
                is_video,
            }
        };
        dialect_match!(self, {
            pg(pool) => {
                let sql = InboxSql { mysql: false };
                let count_sql = format!("SELECT COUNT(*) FROM ({}) ids", sql.ids(user_id, filter));
                let total: i64 = q::query_scalar(sqlx::AssertSqlSafe(count_sql)).bind(cutoff).fetch_one(pool).await?;
                let rows = q::query(sqlx::AssertSqlSafe(sql.page(user_id, filter)))
                    .bind(cutoff)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?;
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    out.push(episode(
                        row.try_get("podcastid")?,
                        row.try_get("podcastname")?,
                        row.try_get("episodetitle")?,
                        row.try_get("episodepubdate")?,
                        row.try_get("episodedescription")?,
                        row.try_get::<Option<String>, _>("episodeartwork")?.unwrap_or_default(),
                        row.try_get::<Option<String>, _>("episodeurl")?.unwrap_or_default(),
                        row.try_get("episodeduration")?,
                        row.try_get("episodeid")?,
                        row.try_get("is_youtube")?,
                        row.try_get("is_video")?,
                    ));
                }
                Ok((out, total))
            },
            mysql(pool) => {
                let sql = InboxSql { mysql: true };
                let count_sql = format!("SELECT COUNT(*) FROM ({}) ids", sql.ids(user_id, filter));
                let total: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_sql)).bind(cutoff).bind(cutoff).fetch_one(pool).await?;
                let rows = sqlx::query(sqlx::AssertSqlSafe(sql.page(user_id, filter)))
                    .bind(cutoff)
                    .bind(cutoff)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?;
                let mut out = Vec::with_capacity(rows.len());
                for row in rows {
                    out.push(episode(
                        row.try_get("podcastid")?,
                        row.try_get("podcastname")?,
                        row.try_get("episodetitle")?,
                        row.try_get("episodepubdate")?,
                        row.try_get("episodedescription")?,
                        row.try_get::<Option<String>, _>("episodeartwork")?.unwrap_or_default(),
                        row.try_get::<Option<String>, _>("episodeurl")?.unwrap_or_default(),
                        row.try_get("episodeduration")?,
                        row.try_get("episodeid")?,
                        row.try_get::<i64, _>("is_youtube")? != 0,
                        row.try_get::<i64, _>("is_video")? != 0,
                    ));
                }
                Ok((out, total))
            },
        })
    }

    pub async fn inbox_count(&self, user_id: i32) -> AppResult<i64> {
        let cutoff = inbox_cutoff(self.get_inbox_days(user_id).await?);
        let filter = InboxFilter::default();
        Ok(dialect_match!(self, {
            pg(pool) => {
                let sql = format!("SELECT COUNT(*) FROM ({}) ids", InboxSql { mysql: false }.ids(user_id, &filter));
                q::query_scalar::<i64>(sqlx::AssertSqlSafe(sql)).bind(cutoff).fetch_one(pool).await?
            },
            mysql(pool) => {
                let sql = format!("SELECT COUNT(*) FROM ({}) ids", InboxSql { mysql: true }.ids(user_id, &filter));
                sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(sql)).bind(cutoff).bind(cutoff).fetch_one(pool).await?
            },
        }))
    }

    /// Take episodes out of the inbox. Returns how many weren't dismissed already.
    pub async fn dismiss_inbox_episodes(&self, user_id: i32, episodes: &[(i32, bool)]) -> AppResult<u64> {
        if episodes.is_empty() {
            return Ok(0);
        }
        let mut added = 0;
        for chunk in episodes.chunks(500) {
            added += dialect_match!(self, {
                pg(pool) => {
                    let values = chunk
                        .iter()
                        .map(|(id, yt)| format!("({user_id}, {id}, {})", if *yt { "TRUE" } else { "FALSE" }))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let sql = format!(
                        r#"INSERT INTO "InboxDismissed" (userid, episodeid, isyoutube) VALUES {values}
                           ON CONFLICT (userid, episodeid, isyoutube) DO NOTHING"#
                    );
                    q::query(sqlx::AssertSqlSafe(sql)).execute(pool).await?.rows_affected()
                },
                mysql(pool) => {
                    let values = chunk
                        .iter()
                        .map(|(id, yt)| format!("({user_id}, {id}, {})", if *yt { 1 } else { 0 }))
                        .collect::<Vec<_>>()
                        .join(", ");
                    let sql = format!("INSERT IGNORE INTO InboxDismissed (UserID, EpisodeID, IsYoutube) VALUES {values}");
                    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await?.rows_affected()
                },
            });
        }
        Ok(added)
    }

    /// Put dismissed episodes back. They show again only if nothing else took them out.
    pub async fn restore_inbox_episodes(&self, user_id: i32, episodes: &[(i32, bool)]) -> AppResult<u64> {
        let mut removed = 0;
        for (id, yt) in episodes {
            removed += dialect_match!(self, {
                pg(pool) => {
                    q::query(r#"DELETE FROM "InboxDismissed" WHERE userid = $1 AND episodeid = $2 AND isyoutube = $3"#)
                        .bind(user_id).bind(*id).bind(*yt)
                        .execute(pool).await?.rows_affected()
                },
                mysql(pool) => {
                    sqlx::query("DELETE FROM InboxDismissed WHERE UserID = ? AND EpisodeID = ? AND IsYoutube = ?")
                        .bind(user_id).bind(*id).bind(*yt)
                        .execute(pool).await?.rows_affected()
                },
            });
        }
        Ok(removed)
    }

    /// Dismiss everything the filter covers (all of the inbox by default).
    pub async fn clear_inbox(&self, user_id: i32, filter: &InboxFilter) -> AppResult<u64> {
        let cutoff = inbox_cutoff(self.get_inbox_days(user_id).await?);
        Ok(dialect_match!(self, {
            pg(pool) => {
                let sql = format!(
                    r#"INSERT INTO "InboxDismissed" (userid, episodeid, isyoutube)
                       SELECT {user_id}, episodeid, is_youtube FROM ({}) ids WHERE TRUE
                       ON CONFLICT (userid, episodeid, isyoutube) DO NOTHING"#,
                    InboxSql { mysql: false }.ids(user_id, filter)
                );
                q::query(sqlx::AssertSqlSafe(sql)).bind(cutoff).execute(pool).await?.rows_affected()
            },
            mysql(pool) => {
                let sql = format!(
                    "INSERT IGNORE INTO InboxDismissed (UserID, EpisodeID, IsYoutube)
                     SELECT {user_id}, episodeid, is_youtube FROM ({}) ids",
                    InboxSql { mysql: true }.ids(user_id, filter)
                );
                sqlx::query(sqlx::AssertSqlSafe(sql)).bind(cutoff).bind(cutoff).execute(pool).await?.rows_affected()
            },
        }))
    }

    /// Drop dismissals too old for any inbox to reach, and label assignments on episodes the
    /// user no longer has.
    pub async fn prune_inbox_and_labels(&self) -> AppResult<u64> {
        let cutoff = inbox_cutoff(INBOX_MAX_DAYS + 1);
        Ok(dialect_match!(self, {
            pg(pool) => {
                let dismissed = q::query(r#"DELETE FROM "InboxDismissed" WHERE dismissedat < $1"#)
                    .bind(cutoff).execute(pool).await?.rows_affected();
                let labels = q::query(
                    r#"DELETE FROM "EpisodeLabelAssignments" WHERE
                         (isyoutube = FALSE AND NOT EXISTS (SELECT 1 FROM "Episodes" e JOIN "Podcasts" p ON p.feedid = e.feedid
                             WHERE e.episodeid = "EpisodeLabelAssignments".episodeid AND p.userid = "EpisodeLabelAssignments".userid))
                      OR (isyoutube = TRUE AND NOT EXISTS (SELECT 1 FROM "YouTubeVideos" v JOIN "Podcasts" p ON p.podcastid = v.podcastid
                             WHERE v.videoid = "EpisodeLabelAssignments".episodeid AND p.userid = "EpisodeLabelAssignments".userid))"#,
                ).execute(pool).await?.rows_affected();
                dismissed + labels
            },
            mysql(pool) => {
                let dismissed = sqlx::query("DELETE FROM InboxDismissed WHERE DismissedAt < ?")
                    .bind(cutoff).execute(pool).await?.rows_affected();
                let labels = sqlx::query(
                    "DELETE la FROM EpisodeLabelAssignments la WHERE
                         (la.IsYoutube = 0 AND NOT EXISTS (SELECT 1 FROM Episodes e JOIN Podcasts p ON p.FeedID = e.FeedID
                             WHERE e.EpisodeID = la.EpisodeID AND p.UserID = la.UserID))
                      OR (la.IsYoutube = 1 AND NOT EXISTS (SELECT 1 FROM YouTubeVideos v JOIN Podcasts p ON p.PodcastID = v.PodcastID
                             WHERE v.VideoID = la.EpisodeID AND p.UserID = la.UserID))",
                ).execute(pool).await?.rows_affected();
                dismissed + labels
            },
        }))
    }

    // ---- Labels ----

    pub async fn get_episode_labels(&self, user_id: i32) -> AppResult<Vec<EpisodeLabel>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT l.labelid, l.name, l.color,
                              (SELECT COUNT(*) FROM "EpisodeLabelAssignments" a WHERE a.labelid = l.labelid) AS episode_count
                       FROM "EpisodeLabels" l WHERE l.userid = $1 ORDER BY LOWER(l.name)"#,
                ).bind(user_id).fetch_all(pool).await?;
                rows.iter()
                    .map(|r| Ok(EpisodeLabel {
                        label_id: r.try_get("labelid")?,
                        name: r.try_get("name")?,
                        color: r.try_get("color")?,
                        episode_count: r.try_get("episode_count")?,
                    }))
                    .collect()
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT l.LabelID, l.Name, l.Color,
                            (SELECT COUNT(*) FROM EpisodeLabelAssignments a WHERE a.LabelID = l.LabelID) AS episode_count
                     FROM EpisodeLabels l WHERE l.UserID = ? ORDER BY LOWER(l.Name)",
                ).bind(user_id).fetch_all(pool).await?;
                rows.iter()
                    .map(|r| Ok(EpisodeLabel {
                        label_id: r.try_get("LabelID")?,
                        name: r.try_get("Name")?,
                        color: r.try_get("Color")?,
                        episode_count: r.try_get("episode_count")?,
                    }))
                    .collect()
            },
        })
    }

    /// The user's label with this name, ignoring case.
    async fn label_named(&self, user_id: i32, name: &str) -> AppResult<Option<i32>> {
        Ok(dialect_match!(self, {
            pg(pool) => {
                q::query_scalar::<i32>(r#"SELECT labelid FROM "EpisodeLabels" WHERE userid = $1 AND LOWER(name) = LOWER($2)"#)
                    .bind(user_id).bind(name).fetch_optional(pool).await?
            },
            mysql(pool) => {
                sqlx::query_scalar::<_, i32>("SELECT LabelID FROM EpisodeLabels WHERE UserID = ? AND LOWER(Name) = LOWER(?)")
                    .bind(user_id).bind(name).fetch_optional(pool).await?
            },
        }))
    }

    /// Err(conflict) when the user already has a label with that name.
    pub async fn create_episode_label(&self, user_id: i32, name: &str, color: &str) -> AppResult<EpisodeLabel> {
        if self.label_named(user_id, name).await?.is_some() {
            return Err(AppError::conflict("A label with that name already exists"));
        }
        let label_id: i32 = dialect_match!(self, {
            pg(pool) => {
                q::query_scalar(r#"INSERT INTO "EpisodeLabels" (userid, name, color) VALUES ($1, $2, $3) RETURNING labelid"#)
                    .bind(user_id).bind(name).bind(color).fetch_one(pool).await?
            },
            mysql(pool) => {
                let res = sqlx::query("INSERT INTO EpisodeLabels (UserID, Name, Color) VALUES (?, ?, ?)")
                    .bind(user_id).bind(name).bind(color).execute(pool).await?;
                res.last_insert_id() as i32
            },
        });
        Ok(EpisodeLabel { label_id, name: name.to_string(), color: color.to_string(), episode_count: 0 })
    }

    /// False when the label isn't the user's.
    pub async fn update_episode_label(&self, user_id: i32, label_id: i32, name: &str, color: &str) -> AppResult<bool> {
        if !self.label_owned(user_id, label_id).await? {
            return Ok(false);
        }
        if let Some(other) = self.label_named(user_id, name).await? {
            if other != label_id {
                return Err(AppError::conflict("A label with that name already exists"));
            }
        }
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "EpisodeLabels" SET name = $1, color = $2 WHERE labelid = $3 AND userid = $4"#)
                    .bind(name).bind(color).bind(label_id).bind(user_id).execute(pool).await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE EpisodeLabels SET Name = ?, Color = ? WHERE LabelID = ? AND UserID = ?")
                    .bind(name).bind(color).bind(label_id).bind(user_id).execute(pool).await?;
            },
        });
        Ok(true)
    }

    /// False when the label isn't the user's. Its assignments go with it.
    pub async fn delete_episode_label(&self, user_id: i32, label_id: i32) -> AppResult<bool> {
        if !self.label_owned(user_id, label_id).await? {
            return Ok(false);
        }
        dialect_match!(self, {
            pg(pool) => {
                // Explicit: SQLite only follows ON DELETE CASCADE with foreign keys on.
                q::query(r#"DELETE FROM "EpisodeLabelAssignments" WHERE labelid = $1"#).bind(label_id).execute(pool).await?;
                q::query(r#"DELETE FROM "EpisodeLabels" WHERE labelid = $1 AND userid = $2"#)
                    .bind(label_id).bind(user_id).execute(pool).await?;
            },
            mysql(pool) => {
                sqlx::query("DELETE FROM EpisodeLabels WHERE LabelID = ? AND UserID = ?")
                    .bind(label_id).bind(user_id).execute(pool).await?;
            },
        });
        Ok(true)
    }

    pub async fn label_owned(&self, user_id: i32, label_id: i32) -> AppResult<bool> {
        Ok(dialect_match!(self, {
            pg(pool) => {
                q::query(r#"SELECT 1 FROM "EpisodeLabels" WHERE labelid = $1 AND userid = $2"#)
                    .bind(label_id).bind(user_id).fetch_optional(pool).await?.is_some()
            },
            mysql(pool) => {
                sqlx::query("SELECT 1 FROM EpisodeLabels WHERE LabelID = ? AND UserID = ?")
                    .bind(label_id).bind(user_id).fetch_optional(pool).await?.is_some()
            },
        }))
    }

    /// Every label on every episode of the user's.
    pub async fn get_label_assignments(&self, user_id: i32) -> AppResult<Vec<LabelAssignment>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT labelid, episodeid, isyoutube FROM "EpisodeLabelAssignments" WHERE userid = $1 ORDER BY assignedat"#,
                ).bind(user_id).fetch_all(pool).await?;
                rows.iter()
                    .map(|r| Ok(LabelAssignment {
                        label_id: r.try_get("labelid")?,
                        episode_id: r.try_get("episodeid")?,
                        is_youtube: r.try_get("isyoutube")?,
                    }))
                    .collect()
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT LabelID, EpisodeID, CAST(IsYoutube AS SIGNED) AS IsYoutube FROM EpisodeLabelAssignments WHERE UserID = ? ORDER BY AssignedAt",
                ).bind(user_id).fetch_all(pool).await?;
                rows.iter()
                    .map(|r| Ok(LabelAssignment {
                        label_id: r.try_get("LabelID")?,
                        episode_id: r.try_get("EpisodeID")?,
                        is_youtube: r.try_get::<i64, _>("IsYoutube")? != 0,
                    }))
                    .collect()
            },
        })
    }

    /// Put a label on episodes, or take it off. The caller checks the label and episodes are
    /// the user's. Returns how many changed.
    pub async fn set_label_on_episodes(&self, user_id: i32, label_id: i32, episodes: &[(i32, bool)], assigned: bool) -> AppResult<u64> {
        let mut changed = 0;
        for (id, yt) in episodes {
            changed += dialect_match!(self, {
                pg(pool) => {
                    let sql = if assigned {
                        r#"INSERT INTO "EpisodeLabelAssignments" (labelid, userid, episodeid, isyoutube) VALUES ($1, $2, $3, $4)
                           ON CONFLICT (labelid, episodeid, isyoutube) DO NOTHING"#
                    } else {
                        r#"DELETE FROM "EpisodeLabelAssignments" WHERE labelid = $1 AND userid = $2 AND episodeid = $3 AND isyoutube = $4"#
                    };
                    q::query(sql).bind(label_id).bind(user_id).bind(*id).bind(*yt).execute(pool).await?.rows_affected()
                },
                mysql(pool) => {
                    let sql = if assigned {
                        "INSERT IGNORE INTO EpisodeLabelAssignments (LabelID, UserID, EpisodeID, IsYoutube) VALUES (?, ?, ?, ?)"
                    } else {
                        "DELETE FROM EpisodeLabelAssignments WHERE LabelID = ? AND UserID = ? AND EpisodeID = ? AND IsYoutube = ?"
                    };
                    sqlx::query(sql).bind(label_id).bind(user_id).bind(*id).bind(*yt).execute(pool).await?.rows_affected()
                },
            });
        }
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_ids_parse_leniently() {
        assert_eq!(parse_label_ids(Some("[3, 4]")), vec![3, 4]);
        assert!(parse_label_ids(Some("null")).is_empty());
        assert!(parse_label_ids(Some("garbage")).is_empty());
        assert!(parse_label_ids(None).is_empty());
    }

    #[test]
    fn mysql_inbox_sql_has_no_pg_markers() {
        let sql = InboxSql { mysql: true }.page(7, &InboxFilter { label_id: Some(2), ..Default::default() });
        assert!(!sql.contains("$1") && !sql.contains('"'));
        assert_eq!(sql.matches('?').count(), 4);
    }
}
