//! Background jobs: the debounced push to external targets after local changes, the scheduled
//! sync and its failure backoff, the refresh job doing refresh only, the per-feed refresh lock,
//! and auto-complete at write time.

use std::time::{Duration, Instant};

use chrono::Utc;
use pinepods_api::services::scheduler;
use pinepods_api::services::sync::engine::{self, RunMode};
use pinepods_api::services::sync::engine_db::TargetKind;
use pinepods_api::services::sync::push::{self, PushDelays};

use crate::harness::gpodder::{play_action, GpodderClient};
use crate::harness::mock_gpodder::MockGpodder;
use crate::harness::*;

const REMOTE_USER: &str = "remote";
const REMOTE_PASS: &str = "remote-secret-pw";

struct Scene {
    app: TestApp,
    user: TestUser,
    mock: MockGpodder,
    target: i32,
    feed: String,
    pod: i32,
}

/// A user with one feed and an external gpodder target whose first (full) sync is done.
async fn scene() -> Scene {
    let app = TestApp::new().await;
    let user = app.user("pusher").await;
    let mock = MockGpodder::start().await;
    mock.add_user(REMOTE_USER, REMOTE_PASS);
    let feed = app.feed("show.xml", &Feed::with_episodes("Show", 3));
    let pod = app.subscribe(&user, &feed, 3).await;
    let target = app.db().sync_target_save(user.id, TargetKind::Gpodder, &mock.base_url, REMOTE_USER, REMOTE_PASS).await.unwrap();
    engine::run(app.db(), &app.state.task_spawner, user.id, target, RunMode::Incremental).await.unwrap();
    Scene { app, user, mock, target, feed, pod }
}

impl Scene {
    fn enable_push(&self, quiet_ms: u64, max_wait_ms: u64) {
        push::init(
            &self.app.state.task_spawner,
            PushDelays { quiet: Duration::from_millis(quiet_ms), max_wait: Duration::from_millis(max_wait_ms) },
        );
    }

    /// Runs against the mock so far (each one logs in once).
    fn runs(&self) -> usize {
        self.mock.requests().iter().filter(|r| r.path.contains("/auth/")).count()
    }

    fn pushed_positions(&self, episode_url: &str) -> Vec<i64> {
        self.mock
            .episode_actions(REMOTE_USER)
            .iter()
            .filter(|a| a["episode"] == episode_url && a["action"] == "play")
            .filter_map(|a| a["position"].as_i64())
            .collect()
    }

    async fn episode(&self, title: &str) -> (i32, String) {
        let id = self.app.episode_id(&self.user, self.pod, title).await;
        let url = text(&self.app, &format!(r#"SELECT EpisodeURL FROM "Episodes" WHERE EpisodeID = {id}"#)).await.unwrap();
        (id, url)
    }

    async fn set_backoff(&self, failures: i32, last_attempt: chrono::NaiveDateTime) {
        let at = last_attempt.format("%Y-%m-%d %H:%M:%S");
        exec(&self.app, &format!(r#"UPDATE "SyncTargets" SET ConsecutiveFailures = {failures}, LastAttemptAt = '{at}' WHERE TargetID = {}"#, self.target)).await;
    }

    async fn failures(&self) -> i32 {
        self.app.db().sync_target(self.target).await.unwrap().unwrap().consecutive_failures
    }
}

async fn record(app: &TestApp, user: &TestUser, ep: i32, secs: f64) {
    app.post("/api/data/record_listen_duration")
        .key(user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "listen_duration": secs }))
        .send()
        .await
        .assert_ok();
}

async fn manual_refresh(app: &TestApp, user: &TestUser) -> Vec<Value> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1000);
    pinepods_api::handlers::refresh::run_refresh_process(user.id, false, tx, app.state.clone()).await.expect("manual refresh");
    let mut messages = Vec::new();
    while let Some(m) = rx.recv().await {
        messages.push(serde_json::to_value(&m).unwrap());
    }
    messages
}

// ---------------------------------------------------------------------------------------------
// Event-driven push
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_burst_of_progress_saves_is_pushed_once() {
    let s = scene().await;
    s.enable_push(400, 5_000);
    let (ep, url) = s.episode("Episode 1").await;
    let before = s.runs();

    for pos in [100.0, 200.0, 300.0, 400.0, 500.0] {
        record(&s.app, &s.user, ep, pos).await;
    }
    assert!(push::pending(s.app.db(), s.user.id), "the saves schedule a push");
    eventually("the push ran", || async { (s.runs() > before).then_some(()) }).await;
    eventually("the push finished", || async { s.pushed_positions(&url).contains(&500).then_some(()) }).await;
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    assert_eq!(s.runs(), before + 1, "one run for the whole burst");
    assert_eq!(s.pushed_positions(&url), vec![500], "the coalesced position, once");
}

#[tokio::test(flavor = "multi_thread")]
async fn continuous_playback_is_pushed_within_the_cap() {
    let s = scene().await;
    let (quiet, cap) = (500, 1_500);
    s.enable_push(quiet, cap);
    let (ep, url) = s.episode("Episode 1").await;
    let before = s.runs();

    // A save every 150ms never leaves the 500ms quiet gap a plain debounce waits for.
    let start = Instant::now();
    let mut pos = 10.0;
    let mut pushed_after = None;
    while start.elapsed() < Duration::from_secs(6) {
        record(&s.app, &s.user, ep, pos).await;
        pos += 5.0;
        if s.runs() > before {
            pushed_after = Some(start.elapsed());
            break;
        }
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    let pushed_after = pushed_after.expect("a push while playback continued");
    assert!(pushed_after >= Duration::from_millis(cap), "not before the cap: {pushed_after:?}");
    eventually("a position reached the target", || async { (!s.pushed_positions(&url).is_empty()).then_some(()) }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn no_push_without_an_enabled_target_or_for_the_targets_own_changes() {
    let s = scene().await;
    s.enable_push(300, 1_000);
    let (ep, url) = s.episode("Episode 2").await;

    // A change pulled from the target is applied, logged as the target's, and not pushed back.
    let phone = GpodderClient::new(&s.mock.base_url, REMOTE_USER, REMOTE_PASS, "phone");
    phone.login().await.assert_ok();
    phone.configure_device("Phone", "mobile").await.assert_ok();
    phone.upload_episode_actions(&[play_action(&s.feed, &url, Utc::now(), 0, 640, 1802)]).await;
    let summary = engine::run(s.app.db(), &s.app.state.task_spawner, s.user.id, s.target, RunMode::Incremental).await.unwrap();
    assert_eq!(summary.actions_applied, 1, "{summary:?}");
    assert!(!push::pending(s.app.db(), s.user.id), "pulled changes don't schedule a push");

    // The internal server on, the target disabled: events are still logged, but nothing is pushed.
    // Disabled first: turning the server on also starts a sync of enabled targets in the background.
    exec(&s.app, &format!(r#"UPDATE "SyncTargets" SET Enabled = FALSE WHERE TargetID = {}"#, s.target)).await;
    s.app.post("/api/data/gpodder/toggle").key(&s.user).json(&json!({ "enabled": true })).send().await.assert_ok();
    let runs = s.runs();
    record(&s.app, &s.user, ep, 900.0).await;
    let logged = int(&s.app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE UserID = {} AND Origin = 'pinepods' AND Position = 900"#, s.user.id)).await;
    assert_eq!(logged, Some(1), "the event was logged for the internal server");
    assert!(!push::pending(s.app.db(), s.user.id), "no enabled target, no push");
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(s.runs(), runs);
}

#[tokio::test(flavor = "multi_thread")]
async fn uploads_from_internal_server_clients_reach_the_target() {
    let s = scene().await;
    s.enable_push(300, 1_000);
    let server = s.app.rust_gpodder_server().await;
    let password = s.app.enable_gpodder(&server, &s.user).await;
    let (_, url) = s.episode("Episode 3").await;

    let antennapod = GpodderClient::new(&server.base_url, &s.user.username, &password, "antennapod");
    antennapod.login().await.assert_ok();
    antennapod.configure_device("AntennaPod", "mobile").await.assert_ok();
    let other_feed = s.app.feed("other.xml", &Feed::with_episodes("Other", 1));
    antennapod.upload_subscription_changes(&[&other_feed], &[]).await.assert_ok();
    antennapod.upload_episode_batch(&[play_action(&s.feed, &url, Utc::now(), 0, 777, 1803)]).await.assert_ok();

    eventually("the client's play reached the target", || async { s.pushed_positions(&url).contains(&777).then_some(()) }).await;
    eventually("the client's subscription reached the target", || async {
        s.mock.subscriptions(REMOTE_USER).contains(&other_feed).then_some(())
    })
    .await;
}

// ---------------------------------------------------------------------------------------------
// Scheduled sync
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn the_sync_job_skips_targets_in_backoff_and_sync_now_does_not() {
    let s = scene().await;
    let now = Utc::now().naive_utc();

    s.set_backoff(3, now).await; // 90 minutes
    let before = s.runs();
    let job = engine::run_due(s.app.db(), &s.app.state.task_spawner).await.unwrap();
    assert_eq!((job.synced, job.held), (0, 1), "{job:?}");
    assert_eq!(s.runs(), before, "nothing sent while in backoff");

    // Sync now ignores the backoff, and its success clears it.
    let synced = s.app.post("/api/gpodder/sync").key(&s.user).send().await.ok_json();
    assert_eq!(synced["success"], true, "{synced}");
    assert_eq!(s.runs(), before + 1);
    assert_eq!(s.failures().await, 0);

    // Once the backoff has passed, the job syncs it again.
    s.set_backoff(3, now - chrono::Duration::minutes(91)).await;
    scheduler::run_sync_job(&s.app.state).await.unwrap();
    assert_eq!(s.runs(), before + 2);
    assert_eq!(s.failures().await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_refresh_job_only_refreshes() {
    let s = scene().await;
    let feed_hits = s.app.fixtures().hits("show.xml").len();
    let runs = s.runs();
    s.app.feed("show.xml", &Feed::with_episodes("Show", 4));
    set_next_refresh(&s.app, s.pod, "2000-01-01 00:00:00").await;

    scheduler::run_refresh_job(&s.app.state).await.unwrap();
    assert_eq!(s.app.fixtures().hits("show.xml").len(), feed_hits + 1, "the feed was refreshed");
    s.app.episode_id(&s.user, s.pod, "Episode 4").await;
    assert_eq!(s.runs(), runs, "no sync from the refresh job");
}

// ---------------------------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn scheduled_and_manual_refresh_never_fetch_a_feed_at_once() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let slow = |n| Fixture::new("application/rss+xml", Feed::with_episodes("Slow", n).to_xml()).delay(Duration::from_millis(800));
    let serve = |n| app.fixtures().serve("slow.xml", slow(n));
    let hits = || app.fixtures().hits("slow.xml").len();
    let url = serve(2);
    let pod = app.subscribe(&user, &url, 2).await;
    let forget = format!(r#"UPDATE "Podcasts" SET LastRefreshSuccess = NULL WHERE PodcastID = {pod}"#);
    let forget_refresh = || exec(&app, &forget);

    // Scheduled first: the manual refresh finds the feed busy and skips it.
    forget_refresh().await;
    serve(3);
    let before = hits();
    let (_, messages) = tokio::join!(app.refresh(), async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        manual_refresh(&app, &user).await
    });
    assert_eq!(hits(), before + 1, "fetched once");
    assert!(!messages.iter().any(|m| m.get("new_episode").is_some()), "the manual refresh skipped it: {messages:?}");
    app.episode_id(&user, pod, "Episode 3").await;

    // Manual first: the scheduled refresh waits for it, then fetches.
    forget_refresh().await;
    serve(4);
    let before = hits();
    let (messages, _) = tokio::join!(manual_refresh(&app, &user), async {
        tokio::time::sleep(Duration::from_millis(200)).await;
        app.refresh().await
    });
    assert_eq!(hits(), before + 2);
    assert_eq!(messages.iter().filter(|m| m.get("new_episode").is_some()).count(), 1, "{messages:?}");
    assert_eq!(app.fixtures().max_in_flight("slow.xml"), 1, "never two fetches of the feed at once");
    let copies = int(&app, &format!(r#"SELECT COUNT(*) FROM "Episodes" WHERE PodcastID = {pod}"#)).await;
    assert_eq!(copies, Some(4));
}

#[tokio::test(flavor = "multi_thread")]
async fn playlist_counts_follow_new_episodes() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = app.feed("counted.xml", &Feed::with_episodes("Counted", 2));
    let pod = app.subscribe(&user, &feed, 2).await;
    // Weekend Marathon: every episode of 30 minutes or more (the fixtures run 1801s+).
    let lists = app.get("/api/data/get_playlists").key(&user).query("user_id", user.id).send().await.ok_json();
    let marathon = lists["playlists"].as_array().unwrap().iter().find(|p| p["name"] == "Weekend Marathon").cloned();
    let marathon = marathon.unwrap_or_else(|| panic!("default playlists: {lists}"));
    assert_eq!(marathon["episode_count"], 2, "counted on read: {marathon}");

    app.feed("counted.xml", &Feed::with_episodes("Counted", 3));
    app.refresh().await;
    app.episode_id(&user, pod, "Episode 3").await;
    let stored = int(&app, &format!(r#"SELECT EpisodeCount FROM "Playlists" WHERE UserID = {} AND Name = 'Weekend Marathon'"#, user.id)).await;
    assert_eq!(stored, Some(3), "the refresh recounted this user's playlists");
}

// ---------------------------------------------------------------------------------------------
// Auto-complete
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn auto_complete_happens_at_write_time_and_the_sweep_only_looks_back_a_week() {
    let s = scene().await;
    let app = &s.app;
    app.put("/api/data/update_auto_complete_seconds")
        .key(&s.user)
        .json(&json!({ "user_id": s.user.id, "seconds": 60 }))
        .send()
        .await
        .assert_ok();
    let completed = |id: i32| async move { int(app, &format!(r#"SELECT CASE WHEN Completed THEN 1 ELSE 0 END FROM "Episodes" WHERE EpisodeID = {id}"#)).await == Some(1) };

    // A client of the internal server reports a position near the end: completed on upload.
    let server = app.rust_gpodder_server().await;
    let password = app.enable_gpodder(&server, &s.user).await;
    let (e1, url1) = s.episode("Episode 1").await;
    let client = GpodderClient::new(&server.base_url, &s.user.username, &password, "antennapod");
    client.login().await.assert_ok();
    client.upload_episode_batch(&[play_action(&s.feed, &url1, Utc::now(), 0, 1770, 1801)]).await.assert_ok();
    assert!(completed(e1).await, "completed when the position was written");

    // Positions written without completing them (as before an upgrade): the refresh leaves them,
    // the nightly sweep completes only the recent one.
    let (e2, _) = s.episode("Episode 2").await;
    let (e3, _) = s.episode("Episode 3").await;
    let ago = |days: i64| (Utc::now() - chrono::Duration::days(days)).naive_utc().format("%Y-%m-%d %H:%M:%S").to_string();
    for (ep, days) in [(e2, 1), (e3, 10)] {
        exec(
            app,
            &format!(
                r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES ({}, {ep}, '{}', 1790)"#,
                s.user.id,
                ago(days)
            ),
        )
        .await;
    }
    scheduler::run_refresh_job(&app.state).await.unwrap();
    assert!(!completed(e2).await, "the refresh doesn't scan for auto-complete");
    pinepods_api::handlers::tasks::auto_complete_episodes_internal(&app.state).await.unwrap();
    assert!(completed(e2).await, "the sweep completes recent history");
    assert!(!completed(e3).await, "history older than a week is left alone");
}
