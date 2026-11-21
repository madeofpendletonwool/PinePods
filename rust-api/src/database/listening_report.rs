//! The listening report: what a user played over a period (or all time), from `ListeningLog`,
//! its item snapshots and listening sessions. All time also counts the per-episode history for
//! listening from before the log began.

use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use serde::Serialize;

use super::*;

/// How many entries each top list has.
const TOP: usize = 10;

/// One `ListeningLog` row.
#[derive(Debug, Clone, Default)]
pub(crate) struct LogRow {
    pub day: NaiveDate,
    pub hour: u32,
    pub kind: i64,
    pub item: i64,
    pub client: String,
    pub seconds: i64,
    pub speed: i64,
    pub silence: i64,
    pub ads: i64,
    pub downloaded: i64,
}

/// What an item was when played.
#[derive(Debug, Clone, Default)]
pub(crate) struct ItemInfo {
    pub podcast_id: Option<i64>,
    pub podcast: String,
    pub feed_url: String,
    pub title: String,
    pub artwork: String,
    pub categories: Vec<String>,
    pub pub_date: Option<NaiveDateTime>,
    pub first_listened: Option<NaiveDateTime>,
}

/// Everything the report is built from.
#[derive(Debug, Default)]
pub(crate) struct ReportInput {
    pub log: Vec<LogRow>,
    pub items: HashMap<(i64, i64), ItemInfo>,
    /// All time only: seconds per item from the per-episode history, with what it is.
    pub history: Vec<((i64, i64), i64, ItemInfo)>,
    /// Credits per episode id, and per podcast id for episodes without their own.
    pub episode_people: HashMap<i64, Vec<Person>>,
    pub podcast_people: HashMap<i64, Vec<Person>>,
    /// (started, seconds) of the period's sessions, UTC.
    pub sessions: Vec<(NaiveDateTime, i64)>,
    pub finished: i64,
    /// The user's current podcasts: a link to one that is gone would lead nowhere.
    pub podcasts: HashSet<i64>,
    /// Every day with listening in the log, for the period picker.
    pub logged_days: Vec<NaiveDate>,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
pub struct Person {
    pub person_id: i64,
    pub name: String,
    pub image: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReportPodcast {
    /// Null when it is no longer in the library.
    pub podcast_id: Option<i64>,
    pub name: String,
    pub artwork: String,
    pub seconds: i64,
    pub episodes: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReportEpisode {
    /// Null when its podcast is no longer in the library.
    pub episode_id: Option<i64>,
    pub is_youtube: bool,
    pub title: String,
    pub podcast: String,
    pub artwork: String,
    pub seconds: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReportPerson {
    pub person_id: i64,
    pub name: String,
    pub image: Option<String>,
    pub seconds: i64,
    pub episodes: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct NamedSeconds {
    pub name: String,
    pub seconds: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DaySeconds {
    /// `YYYY-MM-DD`, local.
    pub date: String,
    pub seconds: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct MonthSeconds {
    /// `YYYY-MM`.
    pub month: String,
    pub seconds: i64,
}

#[derive(Debug, Default, Serialize, utoipa::ToSchema)]
pub struct SessionStats {
    pub count: i64,
    pub average_seconds: i64,
    pub longest_seconds: i64,
    /// When the longest began, unix seconds.
    pub longest_started_at: Option<i64>,
}

#[derive(Debug, Default, Serialize, utoipa::ToSchema)]
pub struct SavedStats {
    pub speed: i64,
    pub silence: i64,
    pub ads: i64,
}

/// How soon after release episodes were first played.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReleaseLag {
    pub median_days: f64,
    /// Share (0–1) first played within a day of release, and within a week.
    pub within_day: f64,
    pub within_week: f64,
    pub episodes: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ListeningReport {
    /// The period, inclusive (`YYYY-MM-DD`); null for all time.
    pub start: Option<String>,
    pub end: Option<String>,
    pub timezone: String,
    /// The first day in the listening record. Days and hours before it aren't known.
    pub recorded_since: Option<String>,
    /// Months with listening in the record, newest first (`YYYY-MM`).
    pub months: Vec<String>,
    pub seconds: i64,
    pub active_days: i64,
    pub episodes: i64,
    pub podcasts: i64,
    pub finished: i64,
    pub top_podcasts: Vec<ReportPodcast>,
    pub top_episodes: Vec<ReportEpisode>,
    pub top_categories: Vec<NamedSeconds>,
    pub top_people: Vec<ReportPerson>,
    pub by_hour: Vec<i64>,
    /// Sunday first.
    pub by_dow: Vec<i64>,
    pub daily: Vec<DaySeconds>,
    /// Each month of the period, when it spans more than one.
    pub by_month: Vec<MonthSeconds>,
    pub biggest_day: Option<DaySeconds>,
    /// The Monday-to-Sunday week with the most listening; `date` is its Monday.
    pub busiest_week: Option<DaySeconds>,
    pub longest_streak: i64,
    pub sessions: SessionStats,
    /// Across the apps that report speed; null without any.
    pub average_speed: Option<f64>,
    pub time_saved: SavedStats,
    /// Podcasts by ads skipped.
    pub ad_heavy: Vec<ReportPodcast>,
    /// `web`, `desktop`, `android`, `ios`, `sync` or `unknown`.
    pub by_client: Vec<NamedSeconds>,
    /// Played from a file on the device.
    pub downloaded_seconds: i64,
    pub release_lag: Option<ReleaseLag>,
}

/// Apps that report playback speed (older apps and sync don't).
const SPEED_CLIENTS: [&str; 4] = ["web", "desktop", "android", "ios"];

fn day_string(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn top<T>(mut v: Vec<T>, key: impl Fn(&T) -> i64) -> Vec<T> {
    v.retain(|x| key(x) > 0);
    v.sort_by_key(|x| std::cmp::Reverse(key(x)));
    v.truncate(TOP);
    v
}

/// The report for `start..=end` (local days; `None` for all time).
pub(crate) fn build_report(input: &ReportInput, range: Option<(NaiveDate, NaiveDate)>, tz: Tz) -> ListeningReport {
    let log = &input.log;

    // Seconds per item: the log's, or for all time the larger of that and the history's
    // (history counts each episode's whole listening, from before the log too).
    let mut item_seconds: HashMap<(i64, i64), i64> = HashMap::new();
    for r in log {
        *item_seconds.entry((r.kind, r.item)).or_insert(0) += r.seconds.max(0);
    }
    let mut info: HashMap<(i64, i64), ItemInfo> = input.items.clone();
    if range.is_none() {
        for (key, secs, item) in &input.history {
            let e = item_seconds.entry(*key).or_insert(0);
            *e = (*e).max(*secs);
            info.entry(*key).or_insert_with(|| item.clone());
        }
    }
    let empty = ItemInfo::default();
    let info_of = |key: &(i64, i64)| info.get(key).unwrap_or(&empty);
    let podcast_key = |i: &ItemInfo| if i.feed_url.is_empty() { i.podcast.clone() } else { i.feed_url.clone() };

    // Podcasts and episodes.
    struct Show {
        id: Option<i64>,
        name: String,
        artwork: String,
        seconds: i64,
        episodes: i64,
        ads: i64,
    }
    let mut shows: HashMap<String, Show> = HashMap::new();
    let mut episodes = Vec::new();
    let mut categories: HashMap<String, i64> = HashMap::new();
    for (key, secs) in &item_seconds {
        if *secs <= 0 {
            continue;
        }
        let i = info_of(key);
        let live = i.podcast_id.filter(|id| input.podcasts.contains(id));
        let show = shows.entry(podcast_key(i)).or_insert_with(|| Show {
            id: live,
            name: i.podcast.clone(),
            artwork: i.artwork.clone(),
            seconds: 0,
            episodes: 0,
            ads: 0,
        });
        show.seconds += secs;
        show.episodes += 1;
        show.id = show.id.or(live);
        for c in &i.categories {
            *categories.entry(c.clone()).or_insert(0) += secs;
        }
        episodes.push(ReportEpisode {
            episode_id: live.map(|_| key.1),
            is_youtube: key.0 == 1,
            title: i.title.clone(),
            podcast: i.podcast.clone(),
            artwork: i.artwork.clone(),
            seconds: *secs,
        });
    }
    for r in log.iter().filter(|r| r.ads > 0) {
        if let Some(show) = shows.get_mut(&podcast_key(info_of(&(r.kind, r.item)))) {
            show.ads += r.ads;
        }
    }
    let as_podcast = |s: &Show, seconds: i64| ReportPodcast { podcast_id: s.id, name: s.name.clone(), artwork: s.artwork.clone(), seconds, episodes: s.episodes };
    let top_podcasts = top(shows.values().map(|s| as_podcast(s, s.seconds)).collect(), |p| p.seconds);
    let mut ad_heavy = top(shows.values().map(|s| as_podcast(s, s.ads)).collect(), |p| p.seconds);
    ad_heavy.truncate(5);
    let top_episodes = top(episodes, |e| e.seconds);
    let top_categories = top(categories.into_iter().map(|(name, seconds)| NamedSeconds { name, seconds }).collect(), |c| c.seconds);

    // People: an episode's own credits, else its podcast's.
    let mut people: HashMap<i64, ReportPerson> = HashMap::new();
    for (key, secs) in &item_seconds {
        if key.0 != 0 || *secs <= 0 {
            continue;
        }
        let credits = input
            .episode_people
            .get(&key.1)
            .or_else(|| info_of(key).podcast_id.and_then(|p| input.podcast_people.get(&p)));
        let mut seen = HashSet::new();
        for p in credits.into_iter().flatten() {
            if !seen.insert(p.person_id) {
                continue;
            }
            let e = people.entry(p.person_id).or_insert_with(|| ReportPerson {
                person_id: p.person_id,
                name: p.name.clone(),
                image: p.image.clone(),
                seconds: 0,
                episodes: 0,
            });
            e.seconds += secs;
            e.episodes += 1;
        }
    }
    let top_people = top(people.into_values().collect(), |p| p.seconds);

    // When.
    let mut by_hour = vec![0i64; 24];
    let mut by_day: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    let mut clients: HashMap<String, i64> = HashMap::new();
    let (mut saved, mut downloaded) = (SavedStats::default(), 0i64);
    let (mut speed_secs, mut speed_gain) = (0i64, 0i64);
    for r in log {
        by_hour[r.hour.min(23) as usize] += r.seconds;
        *by_day.entry(r.day).or_insert(0) += r.seconds;
        let client = if r.client.is_empty() { "unknown" } else { r.client.as_str() };
        *clients.entry(client.to_string()).or_insert(0) += r.seconds;
        saved.speed += r.speed;
        saved.silence += r.silence;
        saved.ads += r.ads;
        downloaded += r.downloaded;
        if SPEED_CLIENTS.contains(&r.client.as_str()) {
            speed_secs += r.seconds;
            speed_gain += r.speed;
        }
    }
    by_day.retain(|_, s| *s > 0);
    let mut by_dow = vec![0i64; 7];
    let mut weeks: BTreeMap<NaiveDate, i64> = BTreeMap::new();
    for (d, s) in &by_day {
        by_dow[d.weekday().num_days_from_sunday() as usize] += s;
        let monday = *d - Duration::days(i64::from(d.weekday().num_days_from_monday()));
        *weeks.entry(monday).or_insert(0) += s;
    }
    let best = |m: &BTreeMap<NaiveDate, i64>| m.iter().max_by_key(|(d, s)| (**s, std::cmp::Reverse(**d))).map(|(d, s)| DaySeconds { date: day_string(*d), seconds: *s });
    let (mut longest_streak, mut run, mut prev) = (0i64, 0i64, None::<NaiveDate>);
    for d in by_day.keys() {
        run = if prev.and_then(|p| p.succ_opt()) == Some(*d) { run + 1 } else { 1 };
        longest_streak = longest_streak.max(run);
        prev = Some(*d);
    }
    let by_month = match range {
        Some((start, end)) if (start.year(), start.month()) != (end.year(), end.month()) => {
            let mut out = Vec::new();
            let mut m = NaiveDate::from_ymd_opt(start.year(), start.month(), 1).unwrap_or(start);
            while m <= end {
                let seconds = by_day.iter().filter(|(d, _)| d.year() == m.year() && d.month() == m.month()).map(|(_, s)| s).sum();
                out.push(MonthSeconds { month: m.format("%Y-%m").to_string(), seconds });
                m = m.checked_add_months(chrono::Months::new(1)).unwrap_or(end + Duration::days(1));
            }
            out
        }
        _ => Vec::new(),
    };

    // Sessions.
    let mut sessions = SessionStats { count: input.sessions.len() as i64, ..Default::default() };
    if let Some((started, secs)) = input.sessions.iter().max_by_key(|(_, s)| *s) {
        sessions.longest_seconds = *secs;
        sessions.longest_started_at = Some(started.and_utc().timestamp());
        sessions.average_seconds = input.sessions.iter().map(|(_, s)| s).sum::<i64>() / sessions.count.max(1);
    }

    // How soon after release: items first played in the period.
    let in_range = |at: NaiveDateTime| {
        let local = tz.from_utc_datetime(&at).date_naive();
        range.is_none_or(|(s, e)| local >= s && local <= e)
    };
    let mut lags: Vec<f64> = info
        .values()
        .filter_map(|i| {
            let (first, published) = (i.first_listened?, i.pub_date?);
            in_range(first).then(|| ((first - published).num_minutes().max(0) as f64) / 1440.0)
        })
        .collect();
    lags.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let release_lag = (!lags.is_empty()).then(|| {
        let n = lags.len();
        let median = if n % 2 == 1 { lags[n / 2] } else { (lags[n / 2 - 1] + lags[n / 2]) / 2.0 };
        let share = |days: f64| lags.iter().filter(|l| **l <= days).count() as f64 / n as f64;
        ReleaseLag { median_days: (median * 10.0).round() / 10.0, within_day: share(1.0), within_week: share(7.0), episodes: n as i64 }
    });

    let mut months: Vec<String> = input.logged_days.iter().map(|d| d.format("%Y-%m").to_string()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    months.reverse();

    ListeningReport {
        start: range.map(|(s, _)| day_string(s)),
        end: range.map(|(_, e)| day_string(e)),
        timezone: tz.name().to_string(),
        recorded_since: input.logged_days.iter().min().map(|d| day_string(*d)),
        months,
        seconds: item_seconds.values().sum(),
        active_days: by_day.len() as i64,
        episodes: item_seconds.values().filter(|s| **s > 0).count() as i64,
        podcasts: shows.len() as i64,
        finished: input.finished,
        top_podcasts,
        top_episodes,
        top_categories,
        top_people,
        by_hour,
        by_dow,
        daily: by_day.iter().map(|(d, s)| DaySeconds { date: day_string(*d), seconds: *s }).collect(),
        by_month,
        biggest_day: best(&by_day),
        busiest_week: best(&weeks),
        longest_streak,
        sessions,
        average_speed: (speed_secs > 0).then(|| ((speed_secs + speed_gain) as f64 / speed_secs as f64 * 100.0).round() / 100.0),
        time_saved: saved,
        ad_heavy,
        by_client: top(clients.into_iter().map(|(name, seconds)| NamedSeconds { name, seconds }).collect(), |c| c.seconds),
        downloaded_seconds: downloaded,
        release_lag,
    }
}

/// UTC bounds of local days `start..=end`.
fn utc_bounds(range: Option<(NaiveDate, NaiveDate)>, tz: Tz) -> (NaiveDateTime, NaiveDateTime) {
    let midnight = |d: NaiveDate| {
        let local = d.and_hms_opt(0, 0, 0).unwrap();
        tz.from_local_datetime(&local).earliest().map(|t| t.naive_utc()).unwrap_or(local)
    };
    match range {
        Some((s, e)) => (midnight(s), midnight(e + Duration::days(1))),
        None => (NaiveDate::from_ymd_opt(1900, 1, 1).unwrap().into(), NaiveDate::from_ymd_opt(9999, 1, 1).unwrap().into()),
    }
}

fn parse_day(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d").ok()
}

impl DatabasePool {
    /// What `user_id` listened to over local days `start..=end` (both or neither).
    pub async fn listening_report(&self, user_id: i32, range: Option<(NaiveDate, NaiveDate)>) -> AppResult<ListeningReport> {
        let (from, to) = match range {
            Some(r) => r,
            None => (NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(), NaiveDate::from_ymd_opt(9999, 1, 1).unwrap()),
        };
        let mut input = ReportInput::default();
        let tz: Option<String>;
        // Item rows: (kind, item, podcast id, podcast, feed, title, artwork, categories, pub date, first listened).
        type ItemRow = (i64, i64, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<NaiveDateTime>, Option<NaiveDateTime>);
        let mut item_rows: Vec<ItemRow> = Vec::new();
        // History rows: (kind, item, seconds, podcast id, podcast, feed, title, artwork, categories).
        type HistoryRow = (i64, i64, i64, Option<i64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>);
        let mut history_rows: Vec<HistoryRow> = Vec::new();
        let mut episode_credits: Vec<(i64, i64, String, Option<String>)> = Vec::new();
        let mut podcast_credits: Vec<(i64, i64, String, Option<String>)> = Vec::new();
        let log_days: Vec<String>;
        let (start_utc, end_utc);

        dialect_match!(self, {
            pg(pool) => {
                tz = q::query(r#"SELECT timezone FROM "Users" WHERE userid = $1"#)
                    .bind(user_id).fetch_optional(pool).await?
                    .and_then(|r| r.try_get::<Option<String>, _>("timezone").ok().flatten());
                let zone = tz.as_deref().and_then(|t| t.parse::<Tz>().ok()).unwrap_or(Tz::UTC);
                (start_utc, end_utc) = utc_bounds(range, zone);
                for r in q::query(r#"
                    SELECT CAST(listenday AS TEXT) AS day, CAST(listenhour AS BIGINT) AS hour, CAST(itemkind AS BIGINT) AS kind,
                           CAST(itemid AS BIGINT) AS item, client, CAST(seconds AS BIGINT) AS secs, CAST(speedsaved AS BIGINT) AS speed,
                           CAST(silenceskipped AS BIGINT) AS silence, CAST(adsskipped AS BIGINT) AS ads, CAST(downloadedseconds AS BIGINT) AS dl
                    FROM "ListeningLog" WHERE userid = $1 AND listenday >= $2 AND listenday <= $3
                "#).bind(user_id).bind(from).bind(to).fetch_all(pool).await? {
                    let Some(day) = r.try_get::<String, _>("day").ok().as_deref().and_then(parse_day) else { continue };
                    input.log.push(LogRow {
                        day,
                        hour: r.try_get::<i64, _>("hour")?.clamp(0, 23) as u32,
                        kind: r.try_get("kind")?,
                        item: r.try_get("item")?,
                        client: r.try_get::<Option<String>, _>("client")?.unwrap_or_default(),
                        seconds: r.try_get("secs")?,
                        speed: r.try_get("speed")?,
                        silence: r.try_get("silence")?,
                        ads: r.try_get("ads")?,
                        downloaded: r.try_get("dl")?,
                    });
                }
                log_days = q::query_scalar(r#"SELECT DISTINCT CAST(listenday AS TEXT) FROM "ListeningLog" WHERE userid = $1"#)
                    .bind(user_id).fetch_all(pool).await?;
                for r in q::query(r#"
                    SELECT CAST(itemkind AS BIGINT) AS kind, CAST(itemid AS BIGINT) AS item, CAST(podcastid AS BIGINT) AS pid, podcastname, feedurl,
                           itemtitle, artwork, categories, pubdate, firstlistened
                    FROM "ListeningLogItems" WHERE userid = $1
                "#).bind(user_id).fetch_all(pool).await? {
                    item_rows.push((r.try_get("kind")?, r.try_get("item")?, r.try_get("pid")?, r.try_get("podcastname")?, r.try_get("feedurl")?,
                        r.try_get("itemtitle")?, r.try_get("artwork")?, r.try_get("categories")?, r.try_get("pubdate").ok().flatten(), r.try_get("firstlistened").ok().flatten()));
                }
                if range.is_none() {
                    for r in q::query(r#"
                        SELECT 0 AS kind, CAST(h.episodeid AS BIGINT) AS item, CAST(h.listenedseconds AS BIGINT) AS secs, CAST(p.podcastid AS BIGINT) AS pid,
                               p.podcastname, p.feedurl, e.episodetitle AS title, COALESCE(NULLIF(e.episodeartwork, ''), p.artworkurl) AS art, p.categories
                        FROM "UserEpisodeHistory" h
                        JOIN "Episodes" e ON e.episodeid = h.episodeid
                        JOIN "Podcasts" p ON p.feedid = e.feedid AND p.userid = h.userid
                        WHERE h.userid = $1 AND h.listenedseconds > 0
                        UNION ALL
                        SELECT 1 AS kind, CAST(h.videoid AS BIGINT) AS item, CAST(h.listenedseconds AS BIGINT) AS secs, CAST(p.podcastid AS BIGINT) AS pid,
                               p.podcastname, p.feedurl, v.videotitle AS title, COALESCE(NULLIF(v.thumbnailurl, ''), p.artworkurl) AS art, p.categories
                        FROM "UserVideoHistory" h
                        JOIN "YouTubeVideos" v ON v.videoid = h.videoid
                        JOIN "Podcasts" p ON p.podcastid = v.podcastid AND p.userid = h.userid
                        WHERE h.userid = $1 AND h.listenedseconds > 0
                    "#).bind(user_id).fetch_all(pool).await? {
                        history_rows.push((r.try_get::<i32, _>("kind").map(i64::from).or_else(|_| r.try_get::<i64, _>("kind"))?, r.try_get("item")?, r.try_get("secs")?,
                            r.try_get("pid")?, r.try_get("podcastname")?, r.try_get("feedurl")?, r.try_get("title")?, r.try_get("art")?, r.try_get("categories")?));
                    }
                }
                for r in q::query(r#"
                    SELECT CAST(ep.episodeid AS BIGINT) AS owner, CAST(pe.personid AS BIGINT) AS pid, pe.name, pe.img
                    FROM "EpisodePeople" ep JOIN "Persons" pe ON pe.personid = ep.personid
                    WHERE ep.episodeid IN (SELECT itemid FROM "ListeningLog" WHERE userid = $1 AND itemkind = 0
                                           UNION SELECT episodeid FROM "UserEpisodeHistory" WHERE userid = $1)
                "#).bind(user_id).fetch_all(pool).await? {
                    episode_credits.push((r.try_get("owner")?, r.try_get("pid")?, r.try_get("name")?, r.try_get("img")?));
                }
                for r in q::query(r#"
                    SELECT CAST(pp.podcastid AS BIGINT) AS owner, CAST(pe.personid AS BIGINT) AS pid, pe.name, pe.img
                    FROM "PodcastPeople" pp JOIN "Persons" pe ON pe.personid = pp.personid
                    WHERE pp.podcastid IN (SELECT podcastid FROM "Podcasts" WHERE userid = $1)
                "#).bind(user_id).fetch_all(pool).await? {
                    podcast_credits.push((r.try_get("owner")?, r.try_get("pid")?, r.try_get("name")?, r.try_get("img")?));
                }
                for r in q::query(r#"
                    SELECT startedat, CAST(seconds AS BIGINT) AS secs FROM "ListeningSessions"
                    WHERE userid = $1 AND startedat >= $2 AND startedat < $3
                "#).bind(user_id).bind(start_utc).bind(end_utc).fetch_all(pool).await? {
                    input.sessions.push((r.try_get("startedat")?, r.try_get("secs")?));
                }
                let finished = if range.is_some() {
                    q::query_scalar::<i64>(r#"SELECT COUNT(*) FROM "UserEpisodeState" WHERE userid = $1 AND completed = TRUE AND completedat >= $2 AND completedat < $3"#)
                        .bind(user_id).bind(start_utc).bind(end_utc).fetch_one(pool).await?
                } else {
                    q::query_scalar::<i64>(r#"SELECT COUNT(*) FROM "UserEpisodeState" WHERE userid = $1 AND completed = TRUE"#)
                        .bind(user_id).fetch_one(pool).await?
                };
                input.finished = finished;
                input.podcasts = q::query_scalar::<i64>(r#"SELECT CAST(podcastid AS BIGINT) FROM "Podcasts" WHERE userid = $1"#)
                    .bind(user_id).fetch_all(pool).await?.into_iter().collect();
            },
            mysql(pool) => {
                tz = sqlx::query("SELECT TimeZone FROM Users WHERE UserID = ?")
                    .bind(user_id).fetch_optional(pool).await?
                    .and_then(|r| r.try_get::<Option<String>, _>("TimeZone").ok().flatten());
                let zone = tz.as_deref().and_then(|t| t.parse::<Tz>().ok()).unwrap_or(Tz::UTC);
                (start_utc, end_utc) = utc_bounds(range, zone);
                for r in sqlx::query(
                    "SELECT CAST(ListenDay AS CHAR) AS day, CAST(ListenHour AS SIGNED) AS hour, CAST(ItemKind AS SIGNED) AS kind,
                            CAST(ItemID AS SIGNED) AS item, Client AS client, CAST(Seconds AS SIGNED) AS secs, CAST(SpeedSaved AS SIGNED) AS speed,
                            CAST(SilenceSkipped AS SIGNED) AS silence, CAST(AdsSkipped AS SIGNED) AS ads, CAST(DownloadedSeconds AS SIGNED) AS dl
                     FROM ListeningLog WHERE UserID = ? AND ListenDay >= ? AND ListenDay <= ?"
                ).bind(user_id).bind(from).bind(to).fetch_all(pool).await? {
                    let Some(day) = r.try_get::<String, _>("day").ok().as_deref().and_then(parse_day) else { continue };
                    input.log.push(LogRow {
                        day,
                        hour: r.try_get::<i64, _>("hour")?.clamp(0, 23) as u32,
                        kind: r.try_get("kind")?,
                        item: r.try_get("item")?,
                        client: r.try_get::<Option<String>, _>("client")?.unwrap_or_default(),
                        seconds: r.try_get("secs")?,
                        speed: r.try_get("speed")?,
                        silence: r.try_get("silence")?,
                        ads: r.try_get("ads")?,
                        downloaded: r.try_get("dl")?,
                    });
                }
                log_days = sqlx::query_scalar("SELECT DISTINCT CAST(ListenDay AS CHAR) FROM ListeningLog WHERE UserID = ?")
                    .bind(user_id).fetch_all(pool).await?;
                for r in sqlx::query(
                    "SELECT CAST(ItemKind AS SIGNED) AS kind, CAST(ItemID AS SIGNED) AS item, CAST(PodcastID AS SIGNED) AS pid, PodcastName AS podcastname,
                            FeedURL AS feedurl, ItemTitle AS itemtitle, Artwork AS artwork, Categories AS categories,
                            CAST(PubDate AS DATETIME) AS pubdate, CAST(FirstListened AS DATETIME) AS firstlistened
                     FROM ListeningLogItems WHERE UserID = ?"
                ).bind(user_id).fetch_all(pool).await? {
                    item_rows.push((r.try_get("kind")?, r.try_get("item")?, r.try_get("pid")?, r.try_get("podcastname")?, r.try_get("feedurl")?,
                        r.try_get("itemtitle")?, r.try_get("artwork")?, r.try_get("categories")?, r.try_get("pubdate").ok().flatten(), r.try_get("firstlistened").ok().flatten()));
                }
                if range.is_none() {
                    for r in sqlx::query(
                        "SELECT CAST(0 AS SIGNED) AS kind, CAST(h.EpisodeID AS SIGNED) AS item, CAST(h.ListenedSeconds AS SIGNED) AS secs, CAST(p.PodcastID AS SIGNED) AS pid,
                                p.PodcastName AS podcastname, p.FeedURL AS feedurl, e.EpisodeTitle AS title,
                                COALESCE(NULLIF(e.EpisodeArtwork, ''), p.ArtworkURL) AS art, p.Categories AS categories
                         FROM UserEpisodeHistory h
                         JOIN Episodes e ON e.EpisodeID = h.EpisodeID
                         JOIN Podcasts p ON p.FeedID = e.FeedID AND p.UserID = h.UserID
                         WHERE h.UserID = ? AND h.ListenedSeconds > 0
                         UNION ALL
                         SELECT CAST(1 AS SIGNED) AS kind, CAST(h.VideoID AS SIGNED) AS item, CAST(h.ListenedSeconds AS SIGNED) AS secs, CAST(p.PodcastID AS SIGNED) AS pid,
                                p.PodcastName AS podcastname, p.FeedURL AS feedurl, v.VideoTitle AS title,
                                COALESCE(NULLIF(v.ThumbnailURL, ''), p.ArtworkURL) AS art, p.Categories AS categories
                         FROM UserVideoHistory h
                         JOIN YouTubeVideos v ON v.VideoID = h.VideoID
                         JOIN Podcasts p ON p.PodcastID = v.PodcastID AND p.UserID = h.UserID
                         WHERE h.UserID = ? AND h.ListenedSeconds > 0"
                    ).bind(user_id).bind(user_id).fetch_all(pool).await? {
                        history_rows.push((r.try_get("kind")?, r.try_get("item")?, r.try_get("secs")?, r.try_get("pid")?, r.try_get("podcastname")?,
                            r.try_get("feedurl")?, r.try_get("title")?, r.try_get("art")?, r.try_get("categories")?));
                    }
                }
                for r in sqlx::query(
                    "SELECT CAST(ep.EpisodeID AS SIGNED) AS owner, CAST(pe.PersonID AS SIGNED) AS pid, pe.Name AS name, pe.Img AS img
                     FROM EpisodePeople ep JOIN Persons pe ON pe.PersonID = ep.PersonID
                     WHERE ep.EpisodeID IN (SELECT ItemID FROM ListeningLog WHERE UserID = ? AND ItemKind = 0
                                            UNION SELECT EpisodeID FROM UserEpisodeHistory WHERE UserID = ?)"
                ).bind(user_id).bind(user_id).fetch_all(pool).await? {
                    episode_credits.push((r.try_get("owner")?, r.try_get("pid")?, r.try_get("name")?, r.try_get("img")?));
                }
                for r in sqlx::query(
                    "SELECT CAST(pp.PodcastID AS SIGNED) AS owner, CAST(pe.PersonID AS SIGNED) AS pid, pe.Name AS name, pe.Img AS img
                     FROM PodcastPeople pp JOIN Persons pe ON pe.PersonID = pp.PersonID
                     WHERE pp.PodcastID IN (SELECT PodcastID FROM Podcasts WHERE UserID = ?)"
                ).bind(user_id).fetch_all(pool).await? {
                    podcast_credits.push((r.try_get("owner")?, r.try_get("pid")?, r.try_get("name")?, r.try_get("img")?));
                }
                for r in sqlx::query(
                    "SELECT StartedAt AS startedat, CAST(Seconds AS SIGNED) AS secs FROM ListeningSessions
                     WHERE UserID = ? AND StartedAt >= ? AND StartedAt < ?"
                ).bind(user_id).bind(start_utc).bind(end_utc).fetch_all(pool).await? {
                    input.sessions.push((r.try_get("startedat")?, r.try_get("secs")?));
                }
                input.finished = if range.is_some() {
                    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM UserEpisodeState WHERE UserID = ? AND Completed = 1 AND CompletedAt >= ? AND CompletedAt < ?")
                        .bind(user_id).bind(start_utc).bind(end_utc).fetch_one(pool).await?
                } else {
                    sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM UserEpisodeState WHERE UserID = ? AND Completed = 1")
                        .bind(user_id).fetch_one(pool).await?
                };
                input.podcasts = sqlx::query_scalar::<_, i64>("SELECT CAST(PodcastID AS SIGNED) FROM Podcasts WHERE UserID = ?")
                    .bind(user_id).fetch_all(pool).await?.into_iter().collect();
            },
        });

        let categories = |raw: Option<String>| -> Vec<String> {
            let raw = raw.unwrap_or_default();
            let mut names: Vec<String> = self.parse_categories_json(&raw).unwrap_or_default().into_values().filter(|c| !c.is_empty()).collect();
            names.sort();
            names.dedup();
            names
        };
        for (kind, item, pid, podcast, feed, title, art, cats, pub_date, first) in item_rows {
            input.items.insert(
                (kind, item),
                ItemInfo {
                    podcast_id: pid,
                    podcast: podcast.unwrap_or_default(),
                    feed_url: feed.unwrap_or_default(),
                    title: title.unwrap_or_default(),
                    artwork: art.unwrap_or_default(),
                    categories: categories(cats),
                    pub_date,
                    first_listened: first,
                },
            );
        }
        let mut seen = HashSet::new();
        for (kind, item, secs, pid, podcast, feed, title, art, cats) in history_rows {
            if !seen.insert((kind, item)) {
                continue;
            }
            let info = ItemInfo {
                podcast_id: pid,
                podcast: podcast.unwrap_or_default(),
                feed_url: feed.unwrap_or_default(),
                title: title.unwrap_or_default(),
                artwork: art.unwrap_or_default(),
                categories: categories(cats),
                ..Default::default()
            };
            input.history.push(((kind, item), secs, info));
        }
        let credit = |rows: Vec<(i64, i64, String, Option<String>)>| {
            let mut map: HashMap<i64, Vec<Person>> = HashMap::new();
            for (owner, person_id, name, image) in rows {
                map.entry(owner).or_default().push(Person { person_id, name, image: image.filter(|i| !i.is_empty()) });
            }
            map
        };
        input.episode_people = credit(episode_credits);
        input.podcast_people = credit(podcast_credits);
        input.logged_days = log_days.iter().filter_map(|d| parse_day(d)).collect();

        let tz = tz.and_then(|t| t.parse::<Tz>().ok()).unwrap_or(Tz::UTC);
        Ok(build_report(&input, range, tz))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, m, d).unwrap()
    }

    fn row(d: NaiveDate, hour: u32, item: i64, client: &str, seconds: i64) -> LogRow {
        LogRow { day: d, hour, kind: 0, item, client: client.into(), seconds, ..Default::default() }
    }

    fn item(podcast: &str, id: Option<i64>, title: &str) -> ItemInfo {
        ItemInfo { podcast_id: id, podcast: podcast.into(), feed_url: format!("https://{podcast}"), title: title.into(), ..Default::default() }
    }

    fn input() -> ReportInput {
        let mut i = ReportInput::default();
        i.log = vec![
            row(day(11, 2), 8, 1, "web", 1200),
            LogRow { speed: 300, ads: 90, downloaded: 600, ..row(day(11, 3), 21, 1, "android", 1800) },
            row(day(11, 4), 21, 2, "", 600),
            row(day(11, 10), 9, 3, "sync", 400),
        ];
        i.items.insert((0, 1), ItemInfo { categories: vec!["Comedy".into()], ..item("Alpha", Some(10), "A1") });
        i.items.insert((0, 2), item("Alpha", Some(10), "A2"));
        i.items.insert((0, 3), item("Gone", Some(99), "G1"));
        i.podcasts = [10].into_iter().collect();
        i.episode_people.insert(1, vec![Person { person_id: 5, name: "Host".into(), image: None }]);
        i.podcast_people.insert(10, vec![Person { person_id: 6, name: "Pod Host".into(), image: None }]);
        i.sessions = vec![(day(11, 2).and_hms_opt(8, 0, 0).unwrap(), 1200), (day(11, 3).and_hms_opt(21, 0, 0).unwrap(), 2400)];
        i.logged_days = vec![day(10, 30), day(11, 2)];
        i
    }

    #[test]
    fn totals_tops_and_time() {
        let r = build_report(&input(), Some((day(11, 1), day(11, 30))), Tz::UTC);
        assert_eq!((r.seconds, r.active_days, r.episodes, r.podcasts), (4000, 4, 3, 2));
        assert_eq!(r.top_podcasts[0].name, "Alpha");
        assert_eq!((r.top_podcasts[0].seconds, r.top_podcasts[0].episodes, r.top_podcasts[0].podcast_id), (3600, 2, Some(10)));
        assert_eq!(r.top_podcasts[1].podcast_id, None, "unsubscribed: no link");
        assert_eq!(r.top_episodes[0].title, "A1");
        assert_eq!(r.top_episodes[2].episode_id, None);
        assert_eq!(r.top_categories[0].name, "Comedy");
        // Episode 1 has its own credit; episode 2 falls back to the podcast's.
        let people: Vec<_> = r.top_people.iter().map(|p| (p.name.as_str(), p.seconds)).collect();
        assert_eq!(people, vec![("Host", 3000), ("Pod Host", 600)]);
        assert_eq!(r.by_hour[21], 2400);
        assert_eq!(r.by_dow[2], 2200, "Tuesdays: 3 and 10 November");
        assert_eq!((r.biggest_day.as_ref().unwrap().date.as_str(), r.longest_streak), ("2026-11-03", 3));
        assert_eq!(r.busiest_week.as_ref().unwrap().date, "2026-11-02");
        assert!(r.by_month.is_empty(), "one month");
        assert_eq!((r.sessions.count, r.sessions.longest_seconds, r.sessions.average_seconds), (2, 2400, 1800));
        // Speed only from apps that report it: (3000 + 300) / 3000.
        assert_eq!(r.average_speed, Some(1.1));
        assert_eq!((r.time_saved.ads, r.downloaded_seconds), (90, 600));
        assert_eq!(r.ad_heavy.len(), 1);
        let clients: Vec<_> = r.by_client.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(clients, vec!["android", "web", "unknown", "sync"]);
        assert_eq!((r.recorded_since.as_deref(), r.months.clone()), (Some("2026-10-30"), vec!["2026-11".to_string(), "2026-10".to_string()]));
    }

    #[test]
    fn all_time_counts_history_before_the_log() {
        let mut i = input();
        // Episode 1 had more listening than the log saw; episode 4 only in history.
        i.history = vec![((0, 1), 5000, item("Alpha", Some(10), "A1")), ((0, 4), 700, item("Beta", Some(10), "B1"))];
        let r = build_report(&i, None, Tz::UTC);
        assert_eq!(r.seconds, 5000 + 600 + 400 + 700);
        assert_eq!(r.top_episodes[0].seconds, 5000);
        assert!(r.by_month.is_empty());
        // History isn't dated by the hour: the charts stay the log's.
        assert_eq!(r.by_hour.iter().sum::<i64>(), 4000);
    }

    #[test]
    fn months_of_a_year_and_release_lag() {
        let mut i = input();
        let at = |d: NaiveDate, h| d.and_hms_opt(h, 0, 0).unwrap();
        i.items.get_mut(&(0, 1)).unwrap().pub_date = Some(at(day(11, 2), 0));
        i.items.get_mut(&(0, 1)).unwrap().first_listened = Some(at(day(11, 2), 8));
        i.items.get_mut(&(0, 2)).unwrap().pub_date = Some(at(day(10, 1), 0));
        i.items.get_mut(&(0, 2)).unwrap().first_listened = Some(at(day(11, 4), 21));
        let r = build_report(&i, Some((day(1, 1), day(12, 31))), Tz::UTC);
        assert_eq!(r.by_month.len(), 12);
        assert_eq!(r.by_month[10].seconds, 4000);
        let lag = r.release_lag.unwrap();
        assert_eq!((lag.episodes, lag.within_day, lag.within_week), (2, 0.5, 0.5));
        assert_eq!(lag.median_days, 17.6);
    }
}
