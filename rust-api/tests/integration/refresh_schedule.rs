//! Per-feed refresh scheduling: the interval a feed earns, the every-minute tick refreshing only
//! due feeds, failure backoff and Retry-After pushing the next refresh out, manual refresh, and
//! the upgrade backfill.

use chrono::{Duration, Utc};
use pinepods_api::handlers::refresh;
use pinepods_api::services::scheduler;

use crate::harness::*;

/// Seconds from now until the podcast's next refresh.
async fn due_in(app: &TestApp, pod: i32) -> i64 {
    let sql = match app.backend() {
        Backend::Postgres => format!(r#"SELECT CAST(EXTRACT(EPOCH FROM (nextrefreshat - NOW())) AS BIGINT) FROM "Podcasts" WHERE podcastid = {pod}"#),
        Backend::MariaDb => format!("SELECT TIMESTAMPDIFF(SECOND, NOW(), NextRefreshAt) FROM Podcasts WHERE PodcastID = {pod}"),
    };
    int(app, &sql).await.unwrap_or_else(|| panic!("podcast {pod} has no next refresh"))
}

async fn interval(app: &TestApp, pod: i32) -> Option<i64> {
    int(app, &format!(r#"SELECT RefreshIntervalMinutes FROM "Podcasts" WHERE PodcastID = {pod}"#)).await
}

async fn set_next(app: &TestApp, pod: i32, at: &str) {
    set_next_refresh(app, pod, at).await;
}

async fn make_due(app: &TestApp, pod: i32) {
    set_next(app, pod, "2000-01-01 00:00:00").await;
}

/// `minutes` from now, give or take the 10% jitter (and a few seconds of test time).
fn assert_about(secs: i64, minutes: i64) {
    let (lo, hi) = (minutes * 60 * 9 / 10 - 5, minutes * 60 * 11 / 10 + 5);
    assert!((lo..=hi).contains(&secs), "{secs}s is not about {minutes} minutes");
}

/// A feed with `n` episodes `gap` apart, the newest `newest_ago` ago.
fn spaced(title: &str, n: u32, newest_ago: Duration, gap: Duration) -> Feed {
    let mut feed = Feed::new(title);
    for i in 0..n {
        feed = feed.episode(Episode::new(i + 1).pub_date(Utc::now() - newest_ago - gap * (n - 1 - i) as i32));
    }
    feed
}

/// Subscribe and wait for the import to schedule the podcast's next refresh.
async fn subscribe(app: &TestApp, user: &TestUser, url: &str, episodes: usize) -> i32 {
    let pod = app.subscribe(user, url, episodes).await;
    eventually("the import scheduled the next refresh", || async { interval(app, pod).await.map(|_| ()) }).await;
    pod
}

async fn tick(app: &TestApp) -> usize {
    refresh::refresh_due_podcasts(&app.state).await.expect("refresh tick")
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_a_podcast_schedules_it_and_the_tick_waits_until_it_is_due() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    // Episodes an hour apart: an eighth of that is under the 15 minute floor.
    let url = app.feed("hourly.xml", &Feed::with_episodes("Hourly", 5));
    let pod = subscribe(&app, &user, &url, 5).await;
    assert_eq!(interval(&app, pod).await, Some(15));
    assert_about(due_in(&app, pod).await, 15);

    let hits = app.fixtures().hits("hourly.xml").len();
    assert_eq!(tick(&app).await, 0);
    scheduler::run_refresh_job(&app.state).await.unwrap();
    assert_eq!(app.fixtures().hits("hourly.xml").len(), hits, "not due yet");

    make_due(&app, pod).await;
    assert_eq!(tick(&app).await, 1);
    assert_eq!(app.fixtures().hits("hourly.xml").len(), hits + 1);
    assert_about(due_in(&app, pod).await, 15);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_tick_refreshes_a_shared_feed_once_when_any_subscriber_is_due() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let daily = |n| spaced("Daily", n, Duration::hours(2), Duration::days(1));
    let url = app.feed("daily.xml", &daily(5));
    let other = app.feed("other.xml", &Feed::with_episodes("Other", 2));
    let a = subscribe(&app, &alice, &url, 5).await;
    let b = subscribe(&app, &bob, &url, 5).await;
    let o = subscribe(&app, &alice, &other, 2).await;
    assert_eq!(interval(&app, a).await, Some(180), "a daily show: every 3 hours");

    app.feed("daily.xml", &daily(6));
    make_due(&app, a).await;
    let (hits, other_hits) = (app.fixtures().hits("daily.xml").len(), app.fixtures().hits("other.xml").len());
    assert_eq!(tick(&app).await, 1);
    assert_eq!(app.fixtures().hits("daily.xml").len(), hits + 1, "fetched once for both");
    assert_eq!(app.fixtures().hits("other.xml").len(), other_hits, "not due");
    app.episode_id(&alice, a, "Episode 6").await;
    app.episode_id(&bob, b, "Episode 6").await;
    for pod in [a, b] {
        assert_eq!(interval(&app, pod).await, Some(180));
        assert_about(due_in(&app, pod).await, 180);
    }
    assert!(due_in(&app, o).await > 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_feed_and_its_host_can_slow_refreshes_down() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let hourly = Feed::with_episodes("Syndicated", 5).extra("<sy:updatePeriod>daily</sy:updatePeriod>");
    let pod = subscribe(&app, &user, &app.feed("sy.xml", &hourly), 5).await;
    assert_eq!(interval(&app, pod).await, Some(360), "daily, capped at 6 hours for a live feed");

    // The host's max-age is seen on the refresh after adding, even when the body is unchanged.
    let xml = Feed::with_episodes("Cached", 5).to_xml();
    let url = app.fixtures().serve("cached.xml", Fixture::new("application/rss+xml", xml).header("Cache-Control", "public, max-age=7200"));
    let cached = subscribe(&app, &user, &url, 5).await;
    assert_eq!(interval(&app, cached).await, Some(15));
    make_due(&app, cached).await;
    tick(&app).await;
    assert_eq!(interval(&app, cached).await, Some(120));

    // An announced publishing time pulls the next refresh in to just after it.
    let soon = (Utc::now() + Duration::minutes(20)).format("%Y-%m-%dT%H:%M:%SZ");
    let weekly = spaced("Announced", 5, Duration::days(1), Duration::weeks(1))
        .extra(&format!(r#"<podcast:updateFrequency rrule="FREQ=WEEKLY" dtstart="{soon}">Weekly</podcast:updateFrequency>"#));
    let announced = subscribe(&app, &user, &app.feed("announced.xml", &weekly), 5).await;
    assert_eq!(interval(&app, announced).await, Some(360));
    let secs = due_in(&app, announced).await;
    assert!((29 * 60..=30 * 60 + 5).contains(&secs), "{secs}s: 10 minutes after the announced time");
}

#[tokio::test(flavor = "multi_thread")]
async fn failures_and_retry_after_push_the_next_refresh_out() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Flaky", 3);
    let url = app.feed("flaky.xml", &feed);
    let pod = subscribe(&app, &user, &url, 3).await;
    let hits = || app.fixtures().hits("flaky.xml").len();
    let failures_sql = format!(r#"SELECT ConsecutiveFailures FROM "Podcasts" WHERE PodcastID = {pod}"#);
    let failures = || int(&app, &failures_sql);

    // One failure: the 30 minute backoff outlasts the 15 minute interval.
    app.fixtures().serve("flaky.xml", Fixture::new("text/plain", "oops").status(502));
    make_due(&app, pod).await;
    tick(&app).await;
    assert_eq!(failures().await, Some(1));
    let secs = due_in(&app, pod).await;
    assert!((29 * 60..=30 * 60 + 5).contains(&secs), "{secs}s");

    // Due by the clock but still in backoff: left alone until the backoff has passed.
    make_due(&app, pod).await;
    let before = hits();
    assert_eq!(tick(&app).await, 0);
    assert_eq!(hits(), before);
    set_refresh_state(&app, pod, "LastRefreshAttempt = '2000-01-01 00:00:00'").await;
    app.feed("flaky.xml", &feed);
    assert_eq!(tick(&app).await, 1);
    assert_eq!(failures().await, Some(0));
    assert_about(due_in(&app, pod).await, 15);

    // A host asking for two hours gets them, and the hold keeps the tick away.
    let throttled = Fixture::new("text/plain", "slow down").status(429).header("Retry-After", "7200");
    app.fixtures().serve("flaky.xml", throttled);
    make_due(&app, pod).await;
    tick(&app).await;
    let secs = due_in(&app, pod).await;
    assert!((7195..=7205).contains(&secs), "{secs}s");
    make_due(&app, pod).await;
    set_refresh_state(&app, pod, "LastRefreshAttempt = '2000-01-01 00:00:00'").await;
    let before = hits();
    assert_eq!(tick(&app).await, 0);
    assert_eq!(hits(), before, "held by Retry-After");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manual_refresh_fetches_now_and_reschedules() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 2));
    let pod = subscribe(&app, &user, &url, 2).await;
    set_next(&app, pod, "2100-01-01 00:00:00").await;
    app.feed("show.xml", &Feed::with_episodes("Show", 3));

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    refresh::run_refresh_process(user.id, false, tx, app.state.clone()).await.expect("manual refresh");
    while rx.recv().await.is_some() {}
    app.episode_id(&user, pod, "Episode 3").await;
    assert_about(due_in(&app, pod).await, 15);

    // The feed problems list's "Refresh now" does the same.
    set_next(&app, pod, "2100-01-01 00:00:00").await;
    exec(&app, &format!(r#"UPDATE "Podcasts" SET LastRefreshSuccess = NULL WHERE PodcastID = {pod}"#)).await;
    let before = app.fixtures().hits("show.xml").len();
    assert!(refresh::refresh_podcast_now(&app.state, user.id, pod).await.unwrap());
    assert_eq!(app.fixtures().hits("show.xml").len(), before + 1);
    assert_about(due_in(&app, pod).await, 15);
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrading_backfills_intervals_and_spreads_the_first_refreshes() {
    let app = TestApp::new().await;
    let user = app.user("upgrader").await;
    let daily = subscribe(&app, &user, &app.feed("daily.xml", &spaced("Daily", 10, Duration::hours(3), Duration::days(1))), 10).await;
    let dormant = subscribe(&app, &user, &app.feed("old.xml", &spaced("Old", 4, Duration::days(200), Duration::days(7))), 4).await;
    let single = subscribe(&app, &user, &app.feed("one.xml", &spaced("One", 1, Duration::days(2), Duration::days(1))), 1).await;

    // The columns as before the upgrade: not there.
    exec(&app, r#"ALTER TABLE "Podcasts" DROP COLUMN NextRefreshAt, DROP COLUMN RefreshIntervalMinutes"#).await;
    rerun_migrations(&app, &["075"]).await;

    assert_eq!(interval(&app, daily).await, Some(180));
    assert_eq!(interval(&app, dormant).await, Some(24 * 60));
    assert_eq!(interval(&app, single).await, Some(60));
    for pod in [daily, dormant, single] {
        let secs = due_in(&app, pod).await;
        assert!((-5..30 * 60).contains(&secs), "first refresh within the old 30 minute cycle: {secs}s");
    }

    // A row added later waits a few minutes for its import to schedule it.
    exec(&app, &format!(r#"INSERT INTO "Podcasts" (PodcastName, FeedURL, UserID) VALUES ('New', 'https://example.invalid/new.xml', {})"#, user.id)).await;
    let new = int(&app, r#"SELECT PodcastID FROM "Podcasts" WHERE PodcastName = 'New'"#).await.unwrap() as i32;
    let secs = due_in(&app, new).await;
    assert!((4 * 60..=5 * 60 + 5).contains(&secs), "{secs}s");
    assert_eq!(interval(&app, new).await, None);
}
