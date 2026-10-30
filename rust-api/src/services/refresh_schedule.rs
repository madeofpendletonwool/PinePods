//! When each feed is refreshed next: from how often it publishes, what its host and the feed
//! itself say about updates, and the configured maximum interval.

use std::sync::OnceLock;

use chrono::{DateTime, Datelike, Duration, Months, NaiveTime, Utc, Weekday};
use quick_xml::events::Event;
use tracing::{info, warn};

/// Shortest interval a feed's publishing pattern can earn.
pub const MIN_INTERVAL: Duration = Duration::minutes(15);
/// Longest interval for a feed that still publishes.
pub const ACTIVE_MAX_INTERVAL: Duration = Duration::hours(6);
/// Interval for feeds with no episode in `DORMANT_AFTER` (or marked complete).
pub const DORMANT_INTERVAL: Duration = Duration::hours(24);
const DORMANT_AFTER: Duration = Duration::days(90);
/// A feed with too few episodes to measure a pattern.
const DEFAULT_INTERVAL: Duration = Duration::hours(1);
/// Refresh this often relative to the typical gap between episodes.
const CADENCE_DIVISOR: i32 = 8;
const CADENCE_SAMPLE: usize = 10;
/// How long after a feed's announced publishing time it's refreshed.
const AFTER_EXPECTED: Duration = Duration::minutes(10);
const JITTER: f64 = 0.1;
/// Default `PINEPODS_REFRESH_MAX_INTERVAL`.
const DEFAULT_MAX_INTERVAL: Duration = Duration::hours(24);

/// What a feed and its host say about how often it changes.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct FeedHints {
    /// `Cache-Control: max-age`.
    pub max_age: Option<Duration>,
    /// `<sy:updatePeriod>` / `<sy:updateFrequency>`.
    pub update_period: Option<Duration>,
    /// `<podcast:updateFrequency rrule="...">`.
    pub update_rule: Option<UpdateRule>,
    /// `<podcast:updateFrequency complete="true">`: no more episodes are coming.
    pub complete: bool,
}

/// The next refresh of a feed: its interval and how long from now (jittered, possibly pulled in
/// to just after an announced publishing time).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Schedule {
    pub interval_minutes: i32,
    pub next_in_secs: i64,
}

/// The refresh interval for a feed whose recent episodes were published at `pub_dates`.
pub fn interval(pub_dates: &[DateTime<Utc>], hints: &FeedHints, now: DateTime<Utc>, max: Duration) -> Duration {
    let max = max.max(Duration::minutes(1));
    let mut dates: Vec<DateTime<Utc>> = pub_dates.to_vec();
    dates.sort_unstable_by(|a, b| b.cmp(a));
    dates.dedup();
    let dormant = hints.complete || dates.first().is_none_or(|newest| now - *newest > DORMANT_AFTER);
    if dormant {
        return DORMANT_INTERVAL.min(max);
    }
    dates.truncate(CADENCE_SAMPLE);
    let cadence = median_gap(&dates).map_or(DEFAULT_INTERVAL, |gap| gap / CADENCE_DIVISOR);
    // The host's hints only ever slow a feed down: many feeds carry a generic "hourly".
    let hint = [hints.max_age, hints.update_period].into_iter().flatten().min().unwrap_or(Duration::zero());
    cadence.max(hint).clamp(MIN_INTERVAL, ACTIVE_MAX_INTERVAL).min(max)
}

fn median_gap(newest_first: &[DateTime<Utc>]) -> Option<Duration> {
    let mut gaps: Vec<Duration> = newest_first.windows(2).map(|w| w[0] - w[1]).collect();
    if gaps.is_empty() {
        return None;
    }
    gaps.sort_unstable();
    let mid = gaps.len() / 2;
    Some(if gaps.len() % 2 == 0 { (gaps[mid - 1] + gaps[mid]) / 2 } else { gaps[mid] })
}

/// When to refresh next: `interval` scaled by `jitter` (-1.0..=1.0 → ±10%), at most `max`, or just
/// after the feed's next announced publishing time when that comes sooner.
pub fn next_refresh_in(
    interval: Duration,
    hints: &FeedHints,
    newest: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    jitter: f64,
    max: Duration,
) -> Duration {
    let factor = 1.0 + JITTER * jitter.clamp(-1.0, 1.0);
    let mut delay = Duration::seconds((interval.num_seconds() as f64 * factor).round() as i64).min(max.max(Duration::minutes(1)));
    if let Some(rule) = hints.update_rule.as_ref().filter(|_| !hints.complete) {
        if let Some(expected) = rule.next_after(now, newest.unwrap_or(now)) {
            delay = delay.min(expected + AFTER_EXPECTED - now);
        }
    }
    delay.max(Duration::minutes(1))
}

/// The schedule after a refresh, with random jitter.
pub fn schedule(pub_dates: &[DateTime<Utc>], hints: &FeedHints, now: DateTime<Utc>) -> Schedule {
    let max = max_interval();
    schedule_with(interval(pub_dates, hints, now, max), pub_dates, hints, now, max)
}

/// The schedule for a known interval (a feed that didn't change keeps the one it had).
pub fn schedule_with(interval: Duration, pub_dates: &[DateTime<Utc>], hints: &FeedHints, now: DateTime<Utc>, max: Duration) -> Schedule {
    use rand::RngExt;
    let jitter = rand::rng().random_range(-1.0..=1.0);
    let newest = pub_dates.iter().max().copied();
    Schedule {
        interval_minutes: interval.num_minutes().clamp(1, i64::from(i32::MAX)) as i32,
        next_in_secs: next_refresh_in(interval, hints, newest, now, jitter, max).num_seconds(),
    }
}

/// Whether the newest of `pub_dates` is old enough for the feed to count as dormant.
pub fn is_dormant(pub_dates: &[DateTime<Utc>], now: DateTime<Utc>) -> bool {
    pub_dates.iter().max().is_none_or(|newest| now - *newest > DORMANT_AFTER)
}

/// `max-age` from a `Cache-Control` header; none when the response mustn't be cached.
pub fn max_age(cache_control: &str) -> Option<Duration> {
    let directives: Vec<String> = cache_control.split(',').map(|d| d.trim().to_ascii_lowercase()).collect();
    if directives.iter().any(|d| d == "no-store" || d == "no-cache") {
        return None;
    }
    directives
        .iter()
        .find_map(|d| d.strip_prefix("max-age="))
        .and_then(|v| v.trim_matches('"').parse::<i64>().ok())
        .filter(|s| *s > 0)
        .map(Duration::seconds)
}

/// The update hints in a feed body (channel level; items are skipped).
pub fn parse_hints(feed: &str) -> FeedHints {
    let mut reader = quick_xml::Reader::from_str(feed);
    let mut hints = FeedHints::default();
    let (mut period, mut frequency) = (None::<Duration>, None::<i32>);
    let mut in_item = 0usize;
    // The sy: element whose text comes next.
    let mut reading: Option<&'static str> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(Event::Start(e)) if e.name().local_name().as_ref() == "item" => in_item += 1,
            Ok(Event::End(e)) if e.name().local_name().as_ref() == "item" => in_item = in_item.saturating_sub(1),
            Ok(Event::Start(e)) if in_item == 0 => reading = channel_hint(&e, &mut hints),
            Ok(Event::Empty(e)) if in_item == 0 => {
                channel_hint(&e, &mut hints);
            }
            Ok(Event::Text(t)) if in_item == 0 => {
                let Some(what) = reading.take() else { continue };
                let text = t.xml10_content().trim().to_ascii_lowercase();
                if what == "period" {
                    period = match text.as_str() {
                        "hourly" => Some(Duration::hours(1)),
                        "daily" => Some(Duration::days(1)),
                        "weekly" => Some(Duration::weeks(1)),
                        "monthly" => Some(Duration::days(30)),
                        "yearly" => Some(Duration::days(365)),
                        _ => None,
                    };
                } else {
                    frequency = text.parse().ok().filter(|f| *f > 0);
                }
            }
            Ok(Event::End(_)) => reading = None,
            _ => {}
        }
    }
    // A frequency alone is per day, the period's default.
    if period.is_some() || frequency.is_some() {
        hints.update_period = Some(period.unwrap_or(Duration::days(1)) / frequency.unwrap_or(1));
    }
    hints
}

/// Note a channel element's hint; returns which sy: value its text holds.
fn channel_hint(e: &quick_xml::events::BytesStart, hints: &mut FeedHints) -> Option<&'static str> {
    let name = e.name();
    let attr = |key: &str| {
        e.attributes()
            .flatten()
            .find(|a| a.key.as_ref() == key)
            .and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.trim().to_string()))
    };
    match name.local_name().as_ref() {
        "updatePeriod" => Some("period"),
        "updateFrequency" => {
            let (rrule, complete) = (attr("rrule"), attr("complete"));
            let podcast = name.prefix().is_some_and(|p| p.as_ref() == "podcast");
            if !(podcast || rrule.is_some() || complete.is_some()) {
                return Some("frequency");
            }
            hints.complete |= complete.is_some_and(|c| c.eq_ignore_ascii_case("true"));
            if let Some(rule) = rrule.and_then(|r| UpdateRule::parse(&r, attr("dtstart").as_deref())) {
                hints.update_rule = Some(rule);
            }
            None
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Freq {
    Hourly,
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

/// The subset of an RFC 5545 recurrence rule feeds use to announce when episodes come out:
/// FREQ, INTERVAL, BYDAY (daily/weekly) and UNTIL, anchored at `dtstart`.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateRule {
    freq: Freq,
    interval: u32,
    by_day: Vec<Weekday>,
    until: Option<DateTime<Utc>>,
    start: Option<DateTime<Utc>>,
}

impl UpdateRule {
    pub fn parse(rrule: &str, dtstart: Option<&str>) -> Option<UpdateRule> {
        let mut freq = None;
        let (mut interval, mut by_day, mut until) = (1u32, Vec::new(), None);
        for part in rrule.trim().trim_start_matches("RRULE:").split(';') {
            let Some((key, value)) = part.split_once('=') else { continue };
            match key.trim().to_ascii_uppercase().as_str() {
                "FREQ" => {
                    freq = match value.trim().to_ascii_uppercase().as_str() {
                        "HOURLY" => Some(Freq::Hourly),
                        "DAILY" => Some(Freq::Daily),
                        "WEEKLY" => Some(Freq::Weekly),
                        "MONTHLY" => Some(Freq::Monthly),
                        "YEARLY" => Some(Freq::Yearly),
                        _ => None,
                    }
                }
                "INTERVAL" => interval = value.trim().parse().ok().filter(|i| *i > 0)?,
                "BYDAY" => {
                    // Ordinals ("1MO", "-1FR") are treated as plain weekdays.
                    by_day = value
                        .split(',')
                        .filter_map(|d| {
                            let d = d.trim().trim_start_matches(|c: char| c == '+' || c == '-' || c.is_ascii_digit());
                            match d.to_ascii_uppercase().as_str() {
                                "MO" => Some(Weekday::Mon),
                                "TU" => Some(Weekday::Tue),
                                "WE" => Some(Weekday::Wed),
                                "TH" => Some(Weekday::Thu),
                                "FR" => Some(Weekday::Fri),
                                "SA" => Some(Weekday::Sat),
                                "SU" => Some(Weekday::Sun),
                                _ => None,
                            }
                        })
                        .collect()
                }
                "UNTIL" => until = parse_time(value),
                _ => {}
            }
        }
        Some(UpdateRule { freq: freq?, interval, by_day, until, start: dtstart.and_then(parse_time) })
    }

    /// The first publishing time after `now`. Without a `dtstart` the rule is anchored at
    /// `anchor` (the newest episode).
    pub fn next_after(&self, now: DateTime<Utc>, anchor: DateTime<Utc>) -> Option<DateTime<Utc>> {
        let start = self.start.unwrap_or(anchor);
        let interval = i64::from(self.interval);
        let next = match self.freq {
            Freq::Daily | Freq::Weekly if !self.by_day.is_empty() => {
                let time = start.time();
                let start_day = start.date_naive();
                let first = now.date_naive().max(start_day);
                (0..=7 * interval + 7)
                    .map(|d| first + Duration::days(d))
                    .map(|day| day.and_time(time).and_utc())
                    .filter(|t| *t > now && *t >= start)
                    .find(|t| {
                        let day = t.date_naive();
                        let aligned = match self.freq {
                            Freq::Weekly => {
                                let monday = |d: chrono::NaiveDate| d - Duration::days(i64::from(d.weekday().num_days_from_monday()));
                                ((monday(day) - monday(start_day)).num_days() / 7) % interval == 0
                            }
                            _ => (day - start_day).num_days() % interval == 0,
                        };
                        aligned && self.by_day.contains(&t.weekday())
                    })?
            }
            Freq::Hourly | Freq::Daily | Freq::Weekly => {
                let step = match self.freq {
                    Freq::Hourly => Duration::hours(interval),
                    Freq::Daily => Duration::days(interval),
                    _ => Duration::weeks(interval),
                };
                if start > now {
                    start
                } else {
                    let steps = (now - start).num_seconds() / step.num_seconds() + 1;
                    start + step * i32::try_from(steps).ok()?
                }
            }
            Freq::Monthly | Freq::Yearly => {
                let months = if self.freq == Freq::Monthly { self.interval } else { self.interval * 12 };
                let mut t = start;
                // Bounded: a rule anchored centuries ago isn't worth walking.
                for _ in 0..5_000 {
                    if t > now {
                        break;
                    }
                    t = t.checked_add_months(Months::new(months))?;
                }
                (t > now).then_some(t)?
            }
        };
        self.until.is_none_or(|until| next <= until).then_some(next)
    }
}

/// An iCalendar/ISO 8601 date or date-time, read as UTC when it has no zone.
fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    let v = value.trim();
    if let Ok(t) = DateTime::parse_from_rfc3339(v) {
        return Some(t.with_timezone(&Utc));
    }
    let v = v.trim_end_matches('Z');
    for format in ["%Y%m%dT%H%M%S", "%Y-%m-%dT%H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(t) = chrono::NaiveDateTime::parse_from_str(v, format) {
            return Some(t.and_utc());
        }
    }
    for format in ["%Y%m%d", "%Y-%m-%d"] {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(v, format) {
            return Some(d.and_time(NaiveTime::MIN).and_utc());
        }
    }
    None
}

/// `PINEPODS_REFRESH_MAX_INTERVAL`: minutes, or a number with s/m/h/d.
pub fn parse_max_interval(value: &str) -> Option<Duration> {
    let v = value.trim().to_ascii_lowercase();
    let (number, unit) = match v.find(|c: char| !c.is_ascii_digit()) {
        Some(i) => v.split_at(i),
        None => (v.as_str(), "m"),
    };
    let n: i64 = number.parse().ok().filter(|n| *n > 0)?;
    let d = match unit.trim() {
        "s" | "sec" | "secs" | "seconds" => Duration::seconds(n),
        "m" | "min" | "mins" | "minutes" => Duration::minutes(n),
        "h" | "hr" | "hours" => Duration::hours(n),
        "d" | "day" | "days" => Duration::days(n),
        _ => return None,
    };
    Some(d.max(Duration::minutes(1)))
}

/// The gap between consecutive runs of a cron schedule (the old `PINEPODS_REFRESH_CRON`).
fn cron_interval(cron: &str) -> Option<Duration> {
    use croner::parser::{CronParser, Seconds};
    let parsed = CronParser::builder().seconds(Seconds::Optional).sloppy_ranges(true).build().parse(cron).ok()?;
    let first = parsed.find_next_occurrence(&Utc::now(), false).ok()?;
    let second = parsed.find_next_occurrence(&first, false).ok()?;
    Some((second - first).max(Duration::minutes(1)))
}

/// The longest any feed waits between refreshes.
pub fn max_interval() -> Duration {
    static MAX: OnceLock<Duration> = OnceLock::new();
    *MAX.get_or_init(|| {
        let var = |name| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        if let Some(value) = var("PINEPODS_REFRESH_MAX_INTERVAL") {
            if let Some(d) = parse_max_interval(&value) {
                info!("Feeds are refreshed at least every {} minutes", d.num_minutes());
                return d;
            }
            warn!("Ignoring PINEPODS_REFRESH_MAX_INTERVAL={value:?}: expected minutes or a number with s/m/h/d");
        } else if let Some(cron) = var("PINEPODS_REFRESH_CRON") {
            warn!("PINEPODS_REFRESH_CRON is deprecated: feeds now refresh on their own schedule. Set PINEPODS_REFRESH_MAX_INTERVAL instead");
            if let Some(d) = cron_interval(&cron) {
                info!("Feeds are refreshed at least every {} minutes (from PINEPODS_REFRESH_CRON)", d.num_minutes());
                return d;
            }
            warn!("Ignoring PINEPODS_REFRESH_CRON={cron:?}: not a valid cron expression");
        }
        DEFAULT_MAX_INTERVAL
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    const NOW: &str = "2026-10-21T12:00:00Z";
    const MAX: Duration = Duration::hours(24);

    /// `n` episodes, the newest `newest_ago` before now, `gap` apart.
    fn episodes(n: i64, newest_ago: Duration, gap: Duration) -> Vec<DateTime<Utc>> {
        (0..n).map(|i| at(NOW) - newest_ago - gap * i as i32).collect()
    }

    fn none() -> FeedHints {
        FeedHints::default()
    }

    #[test]
    fn interval_is_an_eighth_of_the_median_gap() {
        let daily = episodes(10, Duration::hours(2), Duration::days(1));
        assert_eq!(interval(&daily, &none(), at(NOW), MAX), Duration::hours(3));

        // One long break doesn't move the median.
        let mut dates = episodes(9, Duration::hours(1), Duration::hours(16));
        dates.push(dates[8] - Duration::days(60));
        assert_eq!(interval(&dates, &none(), at(NOW), MAX), Duration::hours(2));

        // Only the newest ten count; the order they come in doesn't matter.
        let mut dates = episodes(10, Duration::zero(), Duration::hours(8));
        dates.extend(episodes(20, Duration::days(10), Duration::days(30)));
        dates.reverse();
        assert_eq!(interval(&dates, &none(), at(NOW), MAX), Duration::hours(1));
    }

    #[test]
    fn interval_is_clamped_between_15_minutes_and_6_hours() {
        let hourly = episodes(10, Duration::minutes(5), Duration::hours(1));
        assert_eq!(interval(&hourly, &none(), at(NOW), MAX), MIN_INTERVAL);
        let weekly = episodes(10, Duration::days(2), Duration::weeks(1));
        assert_eq!(interval(&weekly, &none(), at(NOW), MAX), ACTIVE_MAX_INTERVAL);
    }

    #[test]
    fn episodes_published_together_and_single_episodes() {
        // A back catalogue dropped at once has no gaps to measure.
        let batch = vec![at(NOW) - Duration::days(1); 5];
        assert_eq!(interval(&batch, &none(), at(NOW), MAX), Duration::hours(1));
        assert_eq!(interval(&batch[..1], &none(), at(NOW), MAX), Duration::hours(1));
    }

    #[test]
    fn dormant_and_complete_feeds_wait_a_day() {
        let old = episodes(10, Duration::days(91), Duration::days(1));
        assert_eq!(interval(&old, &none(), at(NOW), MAX), DORMANT_INTERVAL);
        assert_eq!(interval(&[], &none(), at(NOW), MAX), DORMANT_INTERVAL);
        let recent = episodes(10, Duration::days(89), Duration::days(1));
        assert_eq!(interval(&recent, &none(), at(NOW), MAX), Duration::hours(3));
        let complete = FeedHints { complete: true, ..none() };
        assert_eq!(interval(&recent, &complete, at(NOW), MAX), DORMANT_INTERVAL);
        assert!(is_dormant(&old, at(NOW)) && !is_dormant(&recent, at(NOW)));
    }

    #[test]
    fn host_hints_only_slow_a_feed_down() {
        let hourly = episodes(10, Duration::zero(), Duration::hours(1));
        let daily = episodes(10, Duration::zero(), Duration::days(1));
        let hints = |max_age: Option<i64>, period: Option<i64>| FeedHints {
            max_age: max_age.map(Duration::minutes),
            update_period: period.map(Duration::minutes),
            ..none()
        };
        assert_eq!(interval(&hourly, &hints(Some(60), None), at(NOW), MAX), Duration::hours(1));
        assert_eq!(interval(&hourly, &hints(None, Some(120)), at(NOW), MAX), Duration::hours(2));
        // The smaller of the two counts.
        assert_eq!(interval(&hourly, &hints(Some(90), Some(120)), at(NOW), MAX), Duration::minutes(90));
        // A generic "hourly" doesn't speed up a daily show.
        assert_eq!(interval(&daily, &hints(None, Some(60)), at(NOW), MAX), Duration::hours(3));
        // Never past the 6 hour ceiling for a live feed.
        assert_eq!(interval(&hourly, &hints(Some(60 * 48), Some(60 * 48)), at(NOW), MAX), ACTIVE_MAX_INTERVAL);
    }

    #[test]
    fn the_configured_maximum_caps_everything() {
        let old = episodes(3, Duration::days(200), Duration::days(1));
        let weekly = episodes(10, Duration::days(1), Duration::weeks(1));
        let hourly = episodes(10, Duration::zero(), Duration::minutes(10));
        let max = Duration::minutes(30);
        assert_eq!(interval(&old, &none(), at(NOW), max), max);
        assert_eq!(interval(&weekly, &none(), at(NOW), max), max);
        // Below the 15 minute floor when that's what was asked for.
        assert_eq!(interval(&hourly, &none(), at(NOW), Duration::minutes(5)), Duration::minutes(5));
    }

    #[test]
    fn jitter_is_ten_percent_and_stays_under_the_maximum() {
        let i = Duration::hours(2);
        assert_eq!(next_refresh_in(i, &none(), None, at(NOW), 0.0, MAX), i);
        assert_eq!(next_refresh_in(i, &none(), None, at(NOW), 1.0, MAX), Duration::minutes(132));
        assert_eq!(next_refresh_in(i, &none(), None, at(NOW), -1.0, MAX), Duration::minutes(108));
        assert_eq!(next_refresh_in(i, &none(), None, at(NOW), 1.0, i), i);
        for _ in 0..100 {
            let s = schedule_with(i, &[], &none(), at(NOW), MAX);
            assert!((108 * 60..=132 * 60).contains(&s.next_in_secs), "{s:?}");
            assert_eq!(s.interval_minutes, 120);
        }
    }

    #[test]
    fn an_announced_publishing_time_pulls_the_refresh_in() {
        // Weekly on Wednesdays at 09:00; now is Wednesday 2026-10-21 12:00.
        let rule = UpdateRule::parse("FREQ=WEEKLY;BYDAY=WE", Some("2026-01-07T09:00:00Z")).unwrap();
        assert_eq!(rule.next_after(at(NOW), at(NOW)), Some(at("2026-10-28T09:00:00Z")));
        let soon = UpdateRule::parse("FREQ=DAILY", Some("2026-10-01T12:30:00Z")).unwrap();
        let hints = FeedHints { update_rule: Some(soon), ..none() };
        assert_eq!(next_refresh_in(Duration::hours(6), &hints, None, at(NOW), 0.0, MAX), Duration::minutes(40));
        // Later than the interval: no effect.
        let later = FeedHints { update_rule: Some(rule), ..none() };
        assert_eq!(next_refresh_in(Duration::hours(6), &later, None, at(NOW), 0.0, MAX), Duration::hours(6));
        // A complete feed expects nothing.
        let done = FeedHints { complete: true, ..hints };
        assert_eq!(next_refresh_in(Duration::hours(6), &done, None, at(NOW), 0.0, MAX), Duration::hours(6));
    }

    #[test]
    fn update_rules() {
        let next = |rrule: &str, start: Option<&str>| UpdateRule::parse(rrule, start).and_then(|r| r.next_after(at(NOW), at("2026-10-20T06:00:00Z")));
        assert_eq!(next("FREQ=DAILY", Some("2026-10-01T08:00:00Z")), Some(at("2026-10-22T08:00:00Z")));
        assert_eq!(next("FREQ=HOURLY;INTERVAL=3", Some("2026-10-21T00:30:00Z")), Some(at("2026-10-21T12:30:00Z")));
        assert_eq!(next("FREQ=WEEKLY;BYDAY=MO,TH", Some("2026-10-05T07:00:00Z")), Some(at("2026-10-22T07:00:00Z")));
        // Every other week: 2026-10-05 is week 0, so the 19th is on and the 26th is off.
        assert_eq!(next("FREQ=WEEKLY;INTERVAL=2;BYDAY=MO", Some("2026-10-05T07:00:00Z")), Some(at("2026-11-02T07:00:00Z")));
        assert_eq!(next("FREQ=MONTHLY", Some("2025-01-15T10:00:00Z")), Some(at("2026-11-15T10:00:00Z")));
        assert_eq!(next("FREQ=YEARLY", Some("2020-03-01")), Some(at("2027-03-01T00:00:00Z")));
        assert_eq!(next("FREQ=DAILY;UNTIL=20261001T000000Z", Some("2026-01-01T08:00:00Z")), None);
        assert_eq!(next("FREQ=WEEKLY", Some("2026-11-05T09:00:00Z")), Some(at("2026-11-05T09:00:00Z")), "starts in the future");
        // No dtstart: anchored at the newest episode (Tuesday 06:00).
        assert_eq!(next("FREQ=WEEKLY", None), Some(at("2026-10-27T06:00:00Z")));
        assert_eq!(next("FREQ=WEEKLY;BYDAY=1MO", Some("2026-10-05T07:00:00Z")), Some(at("2026-10-26T07:00:00Z")), "ordinals read as weekdays");
        assert!(UpdateRule::parse("BYDAY=MO", None).is_none(), "FREQ is required");
        assert!(UpdateRule::parse("FREQ=SECONDLY", None).is_none());
    }

    #[test]
    fn cache_control_max_age() {
        assert_eq!(max_age("public, max-age=3600"), Some(Duration::hours(1)));
        assert_eq!(max_age("Max-Age=\"600\", must-revalidate"), Some(Duration::minutes(10)));
        assert_eq!(max_age("max-age=0"), None);
        assert_eq!(max_age("no-cache, max-age=3600"), None);
        assert_eq!(max_age("private"), None);
    }

    #[test]
    fn hints_from_a_feed() {
        let feed = r#"<?xml version="1.0"?>
<rss xmlns:sy="http://purl.org/rss/1.0/modules/syndication/" xmlns:podcast="https://podcastindex.org/namespace/1.0">
<channel>
  <title>Show</title>
  <sy:updatePeriod> weekly </sy:updatePeriod>
  <sy:updateFrequency>2</sy:updateFrequency>
  <podcast:updateFrequency rrule="FREQ=WEEKLY;BYDAY=TU" dtstart="2026-01-06T05:00:00Z">Tuesdays</podcast:updateFrequency>
  <item><title>Ep</title><sy:updatePeriod>hourly</sy:updatePeriod></item>
</channel></rss>"#;
        let hints = parse_hints(feed);
        assert_eq!(hints.update_period, Some(Duration::hours(84)));
        assert!(!hints.complete);
        let rule = hints.update_rule.unwrap();
        assert_eq!(rule.next_after(at(NOW), at(NOW)), Some(at("2026-10-27T05:00:00Z")));

        let done = parse_hints(r#"<rss><channel><podcast:updateFrequency complete="true"/></channel></rss>"#);
        assert!(done.complete && done.update_rule.is_none());
        assert_eq!(parse_hints("<rss><channel><sy:updatePeriod>daily</sy:updatePeriod></channel></rss>").update_period, Some(Duration::days(1)));
        assert_eq!(parse_hints("<rss><channel><title>x</title></channel></rss>"), FeedHints::default());
        assert_eq!(parse_hints("not xml at all <<<"), FeedHints::default());
    }

    #[test]
    fn max_interval_values() {
        assert_eq!(parse_max_interval("90"), Some(Duration::minutes(90)));
        assert_eq!(parse_max_interval("6h"), Some(Duration::hours(6)));
        assert_eq!(parse_max_interval(" 1d "), Some(Duration::days(1)));
        assert_eq!(parse_max_interval("45m"), Some(Duration::minutes(45)));
        assert_eq!(parse_max_interval("10s"), Some(Duration::minutes(1)), "at least a minute");
        assert_eq!(parse_max_interval("0"), None);
        assert_eq!(parse_max_interval("soon"), None);
        assert_eq!(parse_max_interval("5w"), None);
        assert_eq!(cron_interval("0 */30 * * * *"), Some(Duration::minutes(30)));
        assert_eq!(cron_interval("*/15 * * * *"), Some(Duration::minutes(15)));
        assert_eq!(cron_interval("not a cron"), None);
    }
}
