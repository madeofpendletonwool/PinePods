//! Listening progress: the one place a playback position, listened time or completion is
//! written, whoever reports it (the progress endpoint, the endpoints older apps call, sync).
//!
//! `ListenDuration` is the position and moves either way. `ListenDate` is when that position was
//! observed, on the database clock, and decides between writers: the newest observation wins.
//! `ListenedSeconds` is time actually played and only grows.

use chrono::NaiveDateTime;

use super::*;
use crate::services::sync::events::{self, EpisodeEvent};
use crate::services::sync::SyncOrigin;

/// When a position was observed.
#[derive(Debug, Clone, Copy)]
pub enum Observed {
    /// This many seconds before now (0 = now). Measured by the reporter's own clock, so its
    /// offset from ours doesn't matter.
    Ago(i64),
    /// At this time (UTC). For sync, which carries the remote action's time.
    At(NaiveDateTime),
}

/// Which stored positions a write may replace.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteRule {
    /// One observed at or before this one.
    Newest,
    /// A lower one. For reports that carry no time.
    Forward,
    /// A lower one, or exactly this position: the reporter's previous report, which means nobody
    /// else has written since and it is moving its own position back.
    ForwardOrOwn(i32),
}

#[derive(Debug, Clone, Copy)]
pub struct ProgressWrite {
    pub episode_id: i32,
    pub is_youtube: bool,
    /// Seconds into the episode.
    pub position: i32,
    pub observed: Observed,
    pub rule: WriteRule,
    /// Seconds played since the reporter's last report; credited even when the position loses.
    pub listened: i32,
    /// Played to the end.
    pub finished: bool,
    /// The player's length for the episode, stored when the feed gave none.
    pub duration: Option<i32>,
}

impl ProgressWrite {
    pub fn now(episode_id: i32, is_youtube: bool, position: i32, rule: WriteRule) -> Self {
        Self { episode_id, is_youtube, position, observed: Observed::Ago(0), rule, listened: 0, finished: false, duration: None }
    }
}

/// The stored state after a write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressOutcome {
    /// Whether this write's position became the stored one.
    pub applied: bool,
    pub position: i32,
    pub completed: bool,
    /// When the stored position was observed (unix seconds).
    pub updated_at: Option<i64>,
}

struct Stored {
    duration: Option<i32>,
    has_row: bool,
    position: Option<i32>,
    listen_date: Option<NaiveDateTime>,
    completed: bool,
    auto_complete: i32,
    now: NaiveDateTime,
}

impl Stored {
    fn observed(&self, observed: Observed) -> NaiveDateTime {
        match observed {
            Observed::Ago(secs) => self.now - chrono::Duration::seconds(secs.max(0)),
            // Never in the future: it would beat every later write until the clock caught up.
            Observed::At(at) => at.min(self.now),
        }
    }

    fn accepts(&self, write: &ProgressWrite, at: NaiveDateTime) -> bool {
        if !self.has_row {
            return true;
        }
        let stored = self.position.unwrap_or(0);
        match write.rule {
            WriteRule::Newest => self.listen_date.is_none_or(|d| d <= at),
            WriteRule::Forward => write.position > stored,
            WriteRule::ForwardOrOwn(own) => write.position > stored || stored == own,
        }
    }
}

fn age(observed: Observed) -> i64 {
    match observed {
        Observed::Ago(secs) => secs.max(0),
        Observed::At(_) => 0,
    }
}

impl DatabasePool {
    /// Record a position (and listened time) for one of the user's episodes. `None` when the
    /// episode isn't theirs.
    pub async fn write_progress(&self, user_id: i32, write: ProgressWrite, origin: SyncOrigin) -> AppResult<Option<ProgressOutcome>> {
        let write = ProgressWrite { position: write.position.max(0), listened: write.listened.clamp(0, 86_400), ..write };
        let yt = write.is_youtube;
        let log = !yt && events::should_log(self, user_id, origin).await;

        let stored: Stored;
        let at: NaiveDateTime;
        let applied: bool;
        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                let sql = if yt {
                    r#"SELECT v.duration AS duration, h.uservideohistoryid AS hid, h.listenduration AS pos, h.listendate AS listendate,
                              COALESCE(v.completed, FALSE) AS completed,
                              COALESCE((SELECT us.autocompleteseconds FROM "UserSettings" us WHERE us.userid = $1), 0) AS auto,
                              NOW()::timestamp AS now
                       FROM "YouTubeVideos" v
                       LEFT JOIN "UserVideoHistory" h ON h.videoid = v.videoid AND h.userid = $1
                       WHERE v.videoid = $2 AND EXISTS (SELECT 1 FROM "Podcasts" p WHERE p.podcastid = v.podcastid AND p.userid = $1)"#
                } else {
                    r#"SELECT e.episodeduration AS duration, h.userepisodehistoryid AS hid, h.listenduration AS pos, h.listendate AS listendate,
                              COALESCE((SELECT ues.completed FROM "UserEpisodeState" ues WHERE ues.episodeid = e.episodeid AND ues.userid = $1), FALSE) AS completed,
                              COALESCE((SELECT us.autocompleteseconds FROM "UserSettings" us WHERE us.userid = $1), 0) AS auto,
                              NOW()::timestamp AS now
                       FROM "Episodes" e
                       LEFT JOIN "UserEpisodeHistory" h ON h.episodeid = e.episodeid AND h.userid = $1
                       WHERE e.episodeid = $2 AND EXISTS (SELECT 1 FROM "Podcasts" p WHERE p.feedid = e.feedid AND p.userid = $1)"#
                };
                let Some(row) = q::query(sql).bind(user_id).bind(write.episode_id).fetch_optional(&mut *tx).await? else {
                    return Ok(None);
                };
                stored = Stored {
                    duration: row.try_get("duration")?,
                    has_row: row.try_get::<Option<i32>, _>("hid")?.is_some(),
                    position: row.try_get("pos")?,
                    listen_date: row.try_get("listendate")?,
                    completed: row.try_get::<Option<bool>, _>("completed")?.unwrap_or(false),
                    auto_complete: row.try_get::<Option<i32>, _>("auto")?.unwrap_or(0),
                    now: row.try_get("now")?,
                };
                at = stored.observed(write.observed);
                applied = stored.accepts(&write, at);

                let (table, id) = if yt { (r#""UserVideoHistory""#, "videoid") } else { (r#""UserEpisodeHistory""#, "episodeid") };
                if applied {
                    q::query(sqlx::AssertSqlSafe(format!(
                        r#"INSERT INTO {table} (userid, {id}, listendate, listenduration, listenedseconds)
                           VALUES ($1, $2, $3, $4, $5)
                           ON CONFLICT (userid, {id}) DO UPDATE SET
                               listenduration = EXCLUDED.listenduration, listendate = EXCLUDED.listendate,
                               listenedseconds = COALESCE({table}.listenedseconds, 0) + EXCLUDED.listenedseconds"#
                    )))
                    .bind(user_id)
                    .bind(write.episode_id)
                    .bind(at)
                    .bind(write.position)
                    .bind(write.listened)
                    .execute(&mut *tx)
                    .await?;
                    if yt {
                        q::query(r#"UPDATE "YouTubeVideos" SET listenposition = $1 WHERE videoid = $2"#)
                            .bind(write.position)
                            .bind(write.episode_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                } else if write.listened > 0 {
                    q::query(sqlx::AssertSqlSafe(format!(
                        "UPDATE {table} SET listenedseconds = COALESCE(listenedseconds, 0) + $1 WHERE userid = $2 AND {id} = $3"
                    )))
                    .bind(write.listened)
                    .bind(user_id)
                    .bind(write.episode_id)
                    .execute(&mut *tx)
                    .await?;
                }
                if log && applied && stored.position != Some(write.position) {
                    let event = EpisodeEvent::Progress { position: write.position, age: age(write.observed) };
                    q::pick(events::episode_pg, events::episode_lite)(self, &mut tx, user_id, write.episode_id, event).await;
                }
                tx.commit().await?;
            },
            mysql(pool) => {
                let mut tx = events::begin_mysql(pool, user_id, log).await?;
                let sql = if yt {
                    "SELECT v.Duration AS duration, h.UserVideoHistoryID AS hid, h.ListenDuration AS pos, CAST(h.ListenDate AS DATETIME) AS listendate,
                            COALESCE(v.Completed, 0) AS completed,
                            COALESCE((SELECT us.AutoCompleteSeconds FROM UserSettings us WHERE us.UserID = ?), 0) AS auto,
                            CAST(NOW() AS DATETIME) AS now
                     FROM YouTubeVideos v
                     LEFT JOIN UserVideoHistory h ON h.VideoID = v.VideoID AND h.UserID = ?
                     WHERE v.VideoID = ? AND EXISTS (SELECT 1 FROM Podcasts p WHERE p.PodcastID = v.PodcastID AND p.UserID = ?)"
                } else {
                    "SELECT e.EpisodeDuration AS duration, h.UserEpisodeHistoryID AS hid, h.ListenDuration AS pos, CAST(h.ListenDate AS DATETIME) AS listendate,
                            COALESCE((SELECT ues.Completed FROM UserEpisodeState ues WHERE ues.EpisodeID = e.EpisodeID AND ues.UserID = ?), 0) AS completed,
                            COALESCE((SELECT us.AutoCompleteSeconds FROM UserSettings us WHERE us.UserID = ?), 0) AS auto,
                            CAST(NOW() AS DATETIME) AS now
                     FROM Episodes e
                     LEFT JOIN UserEpisodeHistory h ON h.EpisodeID = e.EpisodeID AND h.UserID = ?
                     WHERE e.EpisodeID = ? AND EXISTS (SELECT 1 FROM Podcasts p WHERE p.FeedID = e.FeedID AND p.UserID = ?)"
                };
                let mut query = sqlx::query(sql);
                if !yt {
                    query = query.bind(user_id);
                }
                let Some(row) = query.bind(user_id).bind(user_id).bind(write.episode_id).bind(user_id).fetch_optional(&mut *tx).await? else {
                    return Ok(None);
                };
                stored = Stored {
                    duration: row.try_get("duration")?,
                    has_row: row.try_get::<Option<i32>, _>("hid")?.is_some(),
                    position: row.try_get("pos")?,
                    listen_date: row.try_get("listendate")?,
                    completed: row.try_get::<Option<i64>, _>("completed")?.unwrap_or(0) != 0,
                    auto_complete: row.try_get::<Option<i64>, _>("auto")?.unwrap_or(0) as i32,
                    now: row.try_get("now")?,
                };
                at = stored.observed(write.observed);
                applied = stored.accepts(&write, at);

                let (table, id) = if yt { ("UserVideoHistory", "VideoID") } else { ("UserEpisodeHistory", "EpisodeID") };
                if applied {
                    sqlx::query(sqlx::AssertSqlSafe(format!(
                        "INSERT INTO {table} (UserID, {id}, ListenDate, ListenDuration, ListenedSeconds)
                         VALUES (?, ?, ?, ?, ?)
                         ON DUPLICATE KEY UPDATE
                             ListenDuration = VALUES(ListenDuration), ListenDate = VALUES(ListenDate),
                             ListenedSeconds = COALESCE(ListenedSeconds, 0) + VALUES(ListenedSeconds)"
                    )))
                    .bind(user_id)
                    .bind(write.episode_id)
                    .bind(at)
                    .bind(write.position)
                    .bind(write.listened)
                    .execute(&mut *tx)
                    .await?;
                    if yt {
                        sqlx::query("UPDATE YouTubeVideos SET ListenPosition = ? WHERE VideoID = ?")
                            .bind(write.position)
                            .bind(write.episode_id)
                            .execute(&mut *tx)
                            .await?;
                    }
                } else if write.listened > 0 {
                    sqlx::query(sqlx::AssertSqlSafe(format!(
                        "UPDATE {table} SET ListenedSeconds = COALESCE(ListenedSeconds, 0) + ? WHERE UserID = ? AND {id} = ?"
                    )))
                    .bind(write.listened)
                    .bind(user_id)
                    .bind(write.episode_id)
                    .execute(&mut *tx)
                    .await?;
                }
                if log && applied && stored.position != Some(write.position) {
                    let event = EpisodeEvent::Progress { position: write.position, age: age(write.observed) };
                    events::episode_mysql(self, &mut tx, user_id, write.episode_id, event).await;
                }
                tx.commit().await?;
            },
        });

        let mut duration = stored.duration.filter(|d| *d > 0);
        if let (None, false, Some(known)) = (duration, yt, write.duration.filter(|d| *d > 0)) {
            self.set_episode_duration_if_missing(write.episode_id, known).await?;
            duration = Some(known);
        }

        let mut completed = stored.completed;
        let near_end = duration.is_some_and(|d| stored.auto_complete > 0 && write.position >= d - stored.auto_complete);
        if write.finished || (applied && near_end && !stored.completed) {
            self.complete_episode(write.episode_id, user_id, yt, origin, write.observed).await?;
            // The position went to the end too, unless a newer one was stored.
            if let (true, Some(d)) = (applied, stored.duration) {
                return Ok(Some(ProgressOutcome { applied, position: d, completed: true, updated_at: Some(at.and_utc().timestamp()) }));
            }
            completed = stored.duration.is_some() || stored.completed;
        }
        Ok(Some(if applied {
            ProgressOutcome { applied, position: write.position, completed, updated_at: Some(at.and_utc().timestamp()) }
        } else {
            ProgressOutcome {
                applied,
                position: stored.position.unwrap_or(0),
                completed,
                updated_at: stored.listen_date.map(|d| d.and_utc().timestamp()),
            }
        }))
    }

    /// Mark an episode completed now (the UI, bulk actions, the nightly sweep, sync).
    pub async fn mark_episode_completed(&self, episode_id: i32, user_id: i32, is_youtube: bool, origin: SyncOrigin) -> AppResult<()> {
        self.complete_episode(episode_id, user_id, is_youtube, origin, Observed::Ago(0)).await
    }

    /// Mark an episode completed as of `observed`. The position moves to the end unless a newer
    /// one is stored (a replay started since). Also takes the episode out of the queue. An
    /// episode whose length is NULL is left alone.
    pub async fn complete_episode(&self, episode_id: i32, user_id: i32, is_youtube: bool, origin: SyncOrigin, observed: Observed) -> AppResult<()> {
        self.remove_queued_episode(episode_id, user_id, is_youtube).await?;
        let log = !is_youtube && events::should_log(self, user_id, origin).await;
        let ago = age(observed);
        let at = |now: NaiveDateTime| match observed {
            Observed::Ago(secs) => now - chrono::Duration::seconds(secs.max(0)),
            Observed::At(at) => at.min(now),
        };

        dialect_match!(self, {
            pg(pool) => {
                let mut tx = q::begin(pool).await?;
                let sql = if is_youtube {
                    r#"SELECT duration AS duration, NOW()::timestamp AS now FROM "YouTubeVideos" WHERE videoid = $1"#
                } else {
                    r#"SELECT episodeduration AS duration, NOW()::timestamp AS now FROM "Episodes" WHERE episodeid = $1"#
                };
                let Some(row) = q::query(sql).bind(episode_id).fetch_optional(&mut *tx).await? else { return Ok(()) };
                let Some(duration) = row.try_get::<Option<i32>, _>("duration")? else { return Ok(()) };
                let at = at(row.try_get("now")?);
                let (table, id) = if is_youtube {
                    q::query(r#"UPDATE "YouTubeVideos" SET completed = TRUE WHERE videoid = $1"#).bind(episode_id).execute(&mut *tx).await?;
                    (r#""UserVideoHistory""#, "videoid")
                } else {
                    q::pick(super::episodes::set_completed_pg, super::episodes::set_completed_lite)(&mut tx, user_id, &[episode_id], true).await?;
                    (r#""UserEpisodeHistory""#, "episodeid")
                };
                let moved = q::query(sqlx::AssertSqlSafe(format!(
                    r#"INSERT INTO {table} (userid, {id}, listendate, listenduration)
                       VALUES ($1, $2, $3, $4)
                       ON CONFLICT (userid, {id}) DO UPDATE SET
                           listenduration = EXCLUDED.listenduration, listendate = EXCLUDED.listendate
                       WHERE {table}.listendate IS NULL OR {table}.listendate <= EXCLUDED.listendate"#
                )))
                .bind(user_id)
                .bind(episode_id)
                .bind(at)
                .bind(duration)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                    > 0;
                if is_youtube && moved {
                    q::query(r#"UPDATE "YouTubeVideos" SET listenposition = $1 WHERE videoid = $2"#)
                        .bind(duration)
                        .bind(episode_id)
                        .execute(&mut *tx)
                        .await?;
                }
                if log {
                    q::pick(events::episode_pg, events::episode_lite)(self, &mut tx, user_id, episode_id, EpisodeEvent::Completed { age: ago }).await;
                }
                tx.commit().await?;
            },
            mysql(pool) => {
                let mut tx = events::begin_mysql(pool, user_id, log).await?;
                let sql = if is_youtube {
                    "SELECT Duration AS duration, CAST(NOW() AS DATETIME) AS now FROM YouTubeVideos WHERE VideoID = ?"
                } else {
                    "SELECT EpisodeDuration AS duration, CAST(NOW() AS DATETIME) AS now FROM Episodes WHERE EpisodeID = ?"
                };
                let Some(row) = sqlx::query(sql).bind(episode_id).fetch_optional(&mut *tx).await? else { return Ok(()) };
                let Some(duration) = row.try_get::<Option<i32>, _>("duration")? else { return Ok(()) };
                let at = at(row.try_get("now")?);
                let (table, id) = if is_youtube {
                    sqlx::query("UPDATE YouTubeVideos SET Completed = 1 WHERE VideoID = ?").bind(episode_id).execute(&mut *tx).await?;
                    ("UserVideoHistory", "VideoID")
                } else {
                    super::episodes::set_completed_mysql(&mut tx, user_id, &[episode_id], true).await?;
                    ("UserEpisodeHistory", "EpisodeID")
                };
                // Assignments run left to right, so the position is decided before the date changes.
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "INSERT INTO {table} (UserID, {id}, ListenDate, ListenDuration)
                     VALUES (?, ?, ?, ?)
                     ON DUPLICATE KEY UPDATE
                         ListenDuration = IF(ListenDate IS NULL OR ListenDate <= VALUES(ListenDate), VALUES(ListenDuration), ListenDuration),
                         ListenDate = IF(ListenDate IS NULL OR ListenDate <= VALUES(ListenDate), VALUES(ListenDate), ListenDate)"
                )))
                .bind(user_id)
                .bind(episode_id)
                .bind(at)
                .bind(duration)
                .execute(&mut *tx)
                .await?;
                if is_youtube {
                    sqlx::query(
                        "UPDATE YouTubeVideos v JOIN UserVideoHistory h ON h.VideoID = v.VideoID AND h.UserID = ?
                         SET v.ListenPosition = h.ListenDuration WHERE v.VideoID = ?",
                    )
                    .bind(user_id)
                    .bind(episode_id)
                    .execute(&mut *tx)
                    .await?;
                }
                if log {
                    events::episode_mysql(self, &mut tx, user_id, episode_id, EpisodeEvent::Completed { age: ago }).await;
                }
                tx.commit().await?;
            },
        });
        Ok(())
    }

    /// Add time actually played to the user's total. `TimeListened` is in minutes; the seconds
    /// that don't make a minute yet wait in `ListenSecondsCarry`.
    pub async fn add_listen_seconds(&self, user_id: i32, seconds: i32) -> AppResult<()> {
        if seconds <= 0 {
            return Ok(());
        }
        dialect_match!(self, {
            pg(pool) => {
                q::query(
                    r#"UPDATE "UserStats"
                       SET timelistened = COALESCE(timelistened, 0) + (listensecondscarry + $1) / 60,
                           listensecondscarry = (listensecondscarry + $1) % 60
                       WHERE userid = $2"#,
                )
                .bind(seconds)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                // Assignments run left to right: the minutes are taken before the carry is reduced.
                sqlx::query(
                    "UPDATE UserStats
                     SET TimeListened = COALESCE(TimeListened, 0) + (ListenSecondsCarry + ?) DIV 60,
                         ListenSecondsCarry = (ListenSecondsCarry + ?) MOD 60
                     WHERE UserID = ?",
                )
                .bind(seconds)
                .bind(seconds)
                .bind(user_id)
                .execute(pool)
                .await?;
            },
        });
        Ok(())
    }
}
