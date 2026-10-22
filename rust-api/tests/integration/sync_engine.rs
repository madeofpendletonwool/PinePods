//! External sync engine against the mock gpodder / Nextcloud server: first (full) sync, cursors,
//! batches, echo suppression, conflicts, what goes on the wire, stored errors, and the settings
//! endpoints that create and remove the target.

use std::collections::HashSet;

use chrono::{Duration, Utc};
use pinepods_api::services::sync::engine::{self, RunMode, RunSummary};
use pinepods_api::services::sync::engine_db::{SyncTargetRow, TargetKind};

use crate::harness::antennapod::check_episode_action;
use crate::harness::gpodder::{play_action, GpodderClient};
use crate::harness::mock_gpodder::MockGpodder;
use crate::harness::*;

const REMOTE_USER: &str = "remote";
const REMOTE_PASS: &str = "remote-secret-pw";

fn ep_url(n: u32) -> String {
    format!("https://cdn.example.invalid/audio/ep{n}.mp3")
}

struct Scene {
    app: TestApp,
    user: TestUser,
    mock: MockGpodder,
    kind: TargetKind,
    target: i32,
}

/// An app, a user and a remote account on the mock; no target yet.
async fn setup() -> (TestApp, TestUser, MockGpodder) {
    let app = TestApp::new().await;
    let user = app.user("syncer").await;
    let mock = MockGpodder::start().await;
    mock.add_user(REMOTE_USER, REMOTE_PASS);
    (app, user, mock)
}

async fn with_target((app, user, mock): (TestApp, TestUser, MockGpodder), kind: TargetKind) -> Scene {
    let target = app.db().sync_target_save(user.id, kind, &mock.base_url, REMOTE_USER, REMOTE_PASS).await.unwrap();
    Scene { app, user, mock, kind, target }
}

impl Scene {
    async fn run(&self, mode: RunMode) -> Result<RunSummary, String> {
        engine::run(self.app.db(), &self.app.state.task_spawner, self.user.id, self.target, mode).await.map_err(|e| e.to_string())
    }

    async fn target(&self) -> SyncTargetRow {
        self.app.db().sync_target(self.target).await.unwrap().unwrap()
    }

    fn push_actions_path(&self) -> &'static str {
        match self.kind {
            TargetKind::Gpodder => "/api/2/episodes/",
            TargetKind::Nextcloud => "/index.php/apps/gpoddersync/episode_action/create",
        }
    }

    fn push_subscriptions_path(&self) -> &'static str {
        match self.kind {
            TargetKind::Gpodder => "/api/2/subscriptions/",
            TargetKind::Nextcloud => "/index.php/apps/gpoddersync/subscription_change/create",
        }
    }

    /// Episode actions Pinepods uploaded successfully, in order.
    fn pushed_actions(&self) -> Vec<Value> {
        pushed(&self.mock, self.push_actions_path())
            .into_iter()
            .flat_map(|b| match b {
                Value::Array(a) => a,
                other => other["actions"].as_array().cloned().unwrap_or_default(),
            })
            .collect()
    }

    /// `(add, remove)` URLs Pinepods uploaded successfully.
    fn pushed_subscriptions(&self) -> (Vec<String>, Vec<String>) {
        let mut out = (Vec::new(), Vec::new());
        for b in pushed(&self.mock, self.push_subscriptions_path()) {
            out.0.extend(strings(&b["add"]));
            out.1.extend(strings(&b["remove"]));
        }
        out
    }

    /// Another device on the remote account (a phone running AntennaPod).
    async fn phone(&self) -> Phone {
        let client = GpodderClient::new(&self.mock.base_url, REMOTE_USER, REMOTE_PASS, "phone");
        if self.kind == TargetKind::Gpodder {
            client.login().await.assert_ok();
            client.configure_device("Phone", "mobile").await.assert_ok();
        }
        Phone { client, kind: self.kind }
    }

    async fn log_counts(&self) -> (i64, i64) {
        let uid = self.user.id;
        (
            int(&self.app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncSubscriptions" WHERE UserID = {uid}"#)).await.unwrap(),
            int(&self.app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE UserID = {uid}"#)).await.unwrap(),
        )
    }

    async fn subscribed(&self, feed: &str) -> bool {
        int(&self.app, &format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE UserID = {} AND FeedURL = '{feed}'"#, self.user.id)).await == Some(1)
    }
}

struct Phone {
    client: GpodderClient,
    kind: TargetKind,
}

impl Phone {
    async fn subscriptions(&self, add: &[&str], remove: &[&str]) {
        match self.kind {
            TargetKind::Gpodder => {
                self.client.upload_subscription_changes(add, remove).await.assert_ok();
            }
            TargetKind::Nextcloud => {
                let auth = Some(GpodderClient::basic_header(REMOTE_USER, REMOTE_PASS));
                let body = json!({ "add": add, "remove": remove });
                let path = "/index.php/apps/gpoddersync/subscription_change/create";
                self.client.send(reqwest::Method::POST, path, Some(&body), auth).await.assert_ok();
            }
        }
    }

    async fn actions(&self, actions: &[Value]) {
        match self.kind {
            TargetKind::Gpodder => {
                self.client.upload_episode_actions(actions).await;
            }
            TargetKind::Nextcloud => {
                let auth = Some(GpodderClient::basic_header(REMOTE_USER, REMOTE_PASS));
                let path = "/index.php/apps/gpoddersync/episode_action/create";
                self.client.send(reqwest::Method::POST, path, Some(&Value::Array(actions.to_vec())), auth).await.assert_ok();
            }
        }
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
}

/// Bodies of Pinepods' successful POSTs to paths starting with `prefix`.
fn pushed(mock: &MockGpodder, prefix: &str) -> Vec<Value> {
    mock.requests()
        .into_iter()
        .filter(|r| r.method == "POST" && r.path.starts_with(prefix) && r.status == 200)
        .filter(|r| !r.path.contains("/devices/") && !r.path.contains("/auth/"))
        .map(|r| serde_json::from_str(&r.body).unwrap())
        .collect()
}

async fn record(app: &TestApp, user: &TestUser, ep: i32, secs: f64) {
    app.post("/api/data/record_listen_duration")
        .key(user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "listen_duration": secs }))
        .send()
        .await
        .assert_ok();
}

async fn mark_completed(app: &TestApp, user: &TestUser, ep: i32) {
    app.post("/api/data/mark_episode_completed")
        .key(user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
}

async fn unsubscribe(app: &TestApp, user: &TestUser, podcast_id: i32) {
    app.post("/api/data/remove_podcast_id")
        .key(user)
        .json(&json!({ "user_id": user.id, "podcast_id": podcast_id }))
        .send()
        .await
        .assert_ok();
}

async fn position(app: &TestApp, user: &TestUser, ep: i32) -> Option<i64> {
    int(app, &format!(r#"SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = {} AND EpisodeID = {ep}"#, user.id)).await
}

fn play_of<'a>(actions: &'a [Value], episode: &str) -> Vec<&'a Value> {
    actions.iter().filter(|a| a["episode"] == episode && a["action"] == "play").collect()
}

/// Insert `n` unpushed Pinepods progress rows straight into the log; returns their ids.
async fn seed_progress_rows(app: &TestApp, user: &TestUser, n: u32) -> Vec<i64> {
    let ts = Utc::now().timestamp();
    let values: Vec<String> = (0..n)
        .map(|i| {
            format!(
                "({}, 'https://feeds.example.invalid/bulk.xml', 'https://cdn.example.invalid/bulk/{i}.mp3', 'play', {ts}, {ts}, 0, {}, 3000, 'pinepods')",
                user.id,
                100 + i
            )
        })
        .collect();
    exec(
        app,
        &format!(
            r#"INSERT INTO "GpodderSyncEpisodeActions" (UserID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Started, Position, Total, Origin) VALUES {}"#,
            values.join(", ")
        ),
    )
    .await;
    let mut ids = Vec::new();
    for i in 0..n {
        ids.push(
            int(
                app,
                &format!(
                    r#"SELECT ActionID FROM "GpodderSyncEpisodeActions" WHERE UserID = {} AND EpisodeURL = 'https://cdn.example.invalid/bulk/{i}.mp3'"#,
                    user.id
                ),
            )
            .await
            .unwrap(),
        );
    }
    ids
}

// ---------------------------------------------------------------------------------------------
// First sync
// ---------------------------------------------------------------------------------------------

/// Local library + progress meet a remote account with another feed and a play: the first run
/// is a full sync that merges both ways; running it again changes nothing.
async fn first_sync_merges_and_is_idempotent(kind: TargetKind) {
    let base = setup().await;
    let (app, user) = (&base.0, &base.1);
    let feed_a = app.feed("a.xml", &Feed::with_episodes("Show A", 3));
    let pod_a = app.subscribe(user, &feed_a, 3).await;
    let e1 = app.episode_id(user, pod_a, "Episode 1").await;
    let e2 = app.episode_id(user, pod_a, "Episode 2").await;
    let e3 = app.episode_id(user, pod_a, "Episode 3").await;
    record(app, user, e1, 300.0).await;
    mark_completed(app, user, e2).await;
    let feed_b = app.feed("b.xml", &Feed::with_episodes("Show B", 2));

    let s = with_target(base, kind).await;
    let phone = s.phone().await;
    phone.subscriptions(&[&feed_b, &feed_a], &[]).await;
    phone.actions(&[play_action(&feed_a, &ep_url(3), Utc::now() - Duration::seconds(60), 0, 120, 1803)]).await;

    let summary = s.run(RunMode::Incremental).await.unwrap();
    assert!(summary.full, "a new target starts with a full sync");

    // Remote → local: B is imported in the background, the phone's position applied.
    eventually("feed B imported", || async { s.subscribed(&feed_b).await.then_some(()) }).await;
    assert_eq!(position(&s.app, &s.user, e3).await, Some(120));

    // Local → remote: the library and the listening state.
    let remote: HashSet<String> = s.mock.subscriptions(REMOTE_USER).into_iter().collect();
    assert_eq!(remote, HashSet::from([feed_a.clone(), feed_b.clone()]));
    let pushed = s.pushed_actions();
    let p1 = play_of(&pushed, &ep_url(1));
    assert_eq!(p1.len(), 1, "{pushed:#?}");
    assert_eq!((p1[0]["position"].as_i64(), p1[0]["guid"].as_str()), (Some(300), Some("guid-1")));
    let p2 = play_of(&pushed, &ep_url(2));
    assert_eq!((p2[0]["position"].as_i64(), p2[0]["total"].as_i64()), (Some(1802), Some(1802)), "completed = played to the end");

    let t = s.target().await;
    assert!(!t.needs_full_sync && t.last_success_at.is_some() && t.last_error.is_none(), "{t:?}");
    assert!(t.subs_pull_cursor > 0 && t.actions_pull_cursor > 0);
    if kind == TargetKind::Gpodder {
        assert_eq!(t.device_name.as_deref(), Some("pinepods"));
    }

    // Again, as a forced full sync: nothing new on either side.
    let logged = s.log_counts().await;
    let positions = (position(&s.app, &s.user, e1).await, position(&s.app, &s.user, e3).await);
    let again = s.run(RunMode::Full).await.unwrap();
    assert_eq!((again.subscriptions_queued, again.subscriptions_pushed, again.actions_pulled), (0, 0, 0), "{again:?}");
    assert_eq!(s.log_counts().await, logged, "pulling back what was pushed logs nothing");
    assert_eq!((position(&s.app, &s.user, e1).await, position(&s.app, &s.user, e3).await), positions);
    let remote_again: HashSet<String> = s.mock.subscriptions(REMOTE_USER).into_iter().collect();
    assert_eq!(remote_again, remote);

    // And incrementally: nothing to push.
    let idle = s.run(RunMode::Incremental).await.unwrap();
    assert_eq!((idle.full, idle.subscriptions_pushed, idle.actions_pushed, idle.actions_pulled), (false, 0, 0, 0), "{idle:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn gpodder_first_sync_merges_both_ways_and_is_idempotent() {
    first_sync_merges_and_is_idempotent(TargetKind::Gpodder).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn nextcloud_first_sync_merges_both_ways_and_is_idempotent() {
    first_sync_merges_and_is_idempotent(TargetKind::Nextcloud).await;
}

// ---------------------------------------------------------------------------------------------
// Cursors and batches
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn cursors_only_advance_when_the_whole_run_succeeds() {
    let base = setup().await;
    let feed = base.0.feed("c.xml", &Feed::with_episodes("Show C", 1));
    let pod = base.0.subscribe(&base.1, &feed, 1).await;
    let ep = base.0.episode_id(&base.1, pod, "Episode 1").await;
    let s = with_target(base, TargetKind::Gpodder).await;
    s.run(RunMode::Incremental).await.unwrap();

    record(&s.app, &s.user, ep, 500.0).await;
    let feed_d = s.app.feed("d.xml", &Feed::with_episodes("Show D", 1));
    s.phone().await.subscriptions(&[&feed_d], &[]).await;
    s.mock.fail_after(Some("POST"), "/api/2/episodes/", 0, 503, 1);

    let before = s.target().await;
    let err = s.run(RunMode::Incremental).await.unwrap_err();
    assert!(err.contains("HTTP 503"), "{err}");
    let failed = s.target().await;
    assert_eq!(
        (failed.subs_pull_cursor, failed.actions_pull_cursor, failed.actions_push_cursor),
        (before.subs_pull_cursor, before.actions_pull_cursor, before.actions_push_cursor),
        "a failed push moves no cursor"
    );
    assert_eq!(failed.last_success_at, before.last_success_at);
    assert_eq!(failed.consecutive_failures, 1);
    assert!(failed.last_error.as_deref().is_some_and(|e| e.contains("HTTP 503")), "{failed:?}");

    s.run(RunMode::Incremental).await.unwrap();
    let ok = s.target().await;
    assert!(ok.actions_push_cursor > before.actions_push_cursor);
    assert!(ok.subs_pull_cursor >= before.subs_pull_cursor && ok.actions_pull_cursor >= before.actions_pull_cursor);
    assert_eq!((ok.consecutive_failures, ok.last_error.as_deref()), (0, None));
    assert_eq!(play_of(&s.pushed_actions(), &ep_url(1)).last().and_then(|a| a["position"].as_i64()), Some(500));
    eventually("feed D imported", || async { s.subscribed(&feed_d).await.then_some(()) }).await;
}

/// A batch failing halfway leaves the cursor after the last accepted batch; the next run sends
/// only the rest.
async fn failed_batch_resumes_after_the_last_good_one(kind: TargetKind) {
    let s = with_target(setup().await, kind).await;
    s.run(RunMode::Incremental).await.unwrap();
    let batch = match kind {
        TargetKind::Gpodder => 200,
        TargetKind::Nextcloud => 100,
    };
    let ids = seed_progress_rows(&s.app, &s.user, (batch * 2 + 50) as u32).await;
    s.mock.fail_after(Some("POST"), s.push_actions_path(), 1, 500, 1);

    let err = s.run(RunMode::Incremental).await.unwrap_err();
    assert!(err.contains("HTTP 500"), "{err}");
    assert_eq!(s.target().await.actions_push_cursor, ids[batch - 1], "cursor stops after the first batch");
    let first = s.pushed_actions().len();
    assert_eq!(first, batch);

    s.run(RunMode::Incremental).await.unwrap();
    let all = s.pushed_actions();
    assert_eq!(all.len(), ids.len(), "the second run sends only what's left");
    let distinct: HashSet<&str> = all.iter().filter_map(|a| a["episode"].as_str()).collect();
    assert_eq!(distinct.len(), ids.len());
    assert_eq!(s.target().await.actions_push_cursor, *ids.last().unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn gpodder_failed_batch_resumes_after_the_last_good_one() {
    failed_batch_resumes_after_the_last_good_one(TargetKind::Gpodder).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn nextcloud_failed_batch_resumes_after_the_last_good_one() {
    failed_batch_resumes_after_the_last_good_one(TargetKind::Nextcloud).await;
}

/// Progress keeps rewriting the newest unpushed row. If that happens between reading a batch and
/// advancing the cursor, the cursor stops below the row so the newer position still goes out.
#[tokio::test(flavor = "multi_thread")]
async fn a_row_rewritten_during_a_push_is_sent_again() {
    use pinepods_api::services::sync::engine_db::Outbox;
    let base = setup().await;
    let feed = base.0.feed("inflight.xml", &Feed::with_episodes("In Flight", 2));
    let pod = base.0.subscribe(&base.1, &feed, 2).await;
    let e1 = base.0.episode_id(&base.1, pod, "Episode 1").await;
    let e2 = base.0.episode_id(&base.1, pod, "Episode 2").await;
    let s = with_target(base, TargetKind::Gpodder).await;
    s.run(RunMode::Incremental).await.unwrap();

    record(&s.app, &s.user, e1, 100.0).await;
    record(&s.app, &s.user, e2, 200.0).await;
    let origin = format!("target:{}", s.target);
    let cursor = s.target().await.actions_push_cursor;
    let read = s.app.db().sync_pending_actions(s.user.id, &origin, cursor, 200).await.unwrap();
    assert_eq!(read.len(), 2);
    record(&s.app, &s.user, e1, 150.0).await; // coalesced into the first row, in flight
    let sent: Vec<(i64, String)> = read.iter().map(|r| (r.id, r.fingerprint.clone())).collect();
    let next = s.app.db().sync_advance_push_cursor(s.target, s.user.id, Outbox::Actions, &origin, cursor, &sent).await.unwrap();
    assert_eq!(next, read[0].id - 1, "the cursor stays below the rewritten row");

    s.run(RunMode::Incremental).await.unwrap();
    let pushed = s.pushed_actions();
    assert_eq!(play_of(&pushed, &ep_url(1)).last().and_then(|a| a["position"].as_i64()), Some(150));
    assert_eq!(s.target().await.actions_push_cursor, read[1].id);
}

// ---------------------------------------------------------------------------------------------
// Echoes, conflicts
// ---------------------------------------------------------------------------------------------

/// "both": AntennaPod on the Pinepods server and a phone on Nextcloud. Each side's changes
/// reach the other; nothing is sent back to where it came from.
#[tokio::test(flavor = "multi_thread")]
async fn changes_cross_between_the_internal_server_and_nextcloud_without_echoes() {
    let base = setup().await;
    let server = base.0.rust_gpodder_server().await;
    let password = base.0.enable_gpodder(&server, &base.1).await;
    let feed = base.0.feed("both.xml", &Feed::with_episodes("Both Show", 2));
    base.0.subscribe(&base.1, &feed, 2).await;
    let s = with_target(base, TargetKind::Nextcloud).await;
    s.run(RunMode::Incremental).await.unwrap();

    let feed_client = s.app.feed("from-antennapod.xml", &Feed::with_episodes("AntennaPod Pick", 1));
    let feed_phone = s.app.feed("from-phone.xml", &Feed::with_episodes("Phone Pick", 1));
    let antennapod = GpodderClient::new(&server.base_url, &s.user.username, &password, "antennapod");
    antennapod.login().await.assert_ok();
    antennapod.upload_subscription_changes(&[&feed_client], &[]).await.assert_ok();
    antennapod.upload_episode_actions(&[play_action(&feed, &ep_url(1), Utc::now() - Duration::seconds(30), 0, 400, 1801)]).await;
    let phone = s.phone().await;
    phone.subscriptions(&[&feed_phone], &[]).await;
    phone.actions(&[play_action(&feed, &ep_url(2), Utc::now() - Duration::seconds(20), 0, 500, 1802)]).await;
    let after_phone = s.mock.requests().len();

    s.run(RunMode::Incremental).await.unwrap();
    let new_requests: Vec<_> = s.mock.requests().into_iter().skip(after_phone).collect();
    let sent: String = new_requests.iter().filter(|r| r.method == "POST").map(|r| r.body.clone()).collect();
    assert!(sent.contains(&feed_client) && sent.contains(&ep_url(1)), "AntennaPod's changes go to Nextcloud: {sent}");
    assert!(!sent.contains(&feed_phone) && !sent.contains(&ep_url(2)), "the phone's changes aren't sent back: {sent}");

    // ...but they reach AntennaPod through the Pinepods server.
    let actions = antennapod.episode_actions(0).await.ok_json();
    assert!(play_of(actions["actions"].as_array().unwrap(), &ep_url(2)).iter().any(|a| a["position"] == 500), "{actions}");
    eventually("the phone's feed imported", || async { s.subscribed(&feed_phone).await.then_some(()) }).await;
    let library = antennapod.subscription_changes(0).await.ok_json();
    assert!(strings(&library["add"]).contains(&feed_phone), "{library}");

    // Next run: Nextcloud hands back what Pinepods sent; none of it is logged or sent again.
    eventually("AntennaPod's feed imported", || async { s.subscribed(&feed_client).await.then_some(()) }).await;
    let logged = s.log_counts().await;
    let before = s.mock.requests().len();
    s.run(RunMode::Incremental).await.unwrap();
    assert_eq!(s.log_counts().await, logged);
    let posts = s.mock.requests().into_iter().skip(before).filter(|r| r.method == "POST").count();
    assert_eq!(posts, 0, "nothing to push");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_local_change_not_pushed_yet_wins_over_the_remote_one() {
    let base = setup().await;
    let feed_a = base.0.feed("keep-a.xml", &Feed::with_episodes("Show A", 1));
    let pod_a = base.0.subscribe(&base.1, &feed_a, 1).await;
    let s = with_target(base, TargetKind::Gpodder).await;
    s.run(RunMode::Incremental).await.unwrap();
    assert!(s.mock.subscriptions(REMOTE_USER).contains(&feed_a));

    // Locally: unsubscribe A, subscribe B. Remotely, before Pinepods pushes: the phone re-adds
    // A and removes B.
    unsubscribe(&s.app, &s.user, pod_a).await;
    let feed_b = s.app.feed("keep-b.xml", &Feed::with_episodes("Show B", 1));
    s.app.subscribe(&s.user, &feed_b, 1).await;
    let phone = s.phone().await;
    phone.subscriptions(&[&feed_a], &[&feed_b]).await;

    let summary = s.run(RunMode::Incremental).await.unwrap();
    assert_eq!((summary.subscriptions_queued, summary.subscriptions_removed), (0, 0), "{summary:?}");
    assert!(!s.subscribed(&feed_a).await && s.subscribed(&feed_b).await);
    let remote = s.mock.subscriptions(REMOTE_USER);
    assert!(!remote.contains(&feed_a) && remote.contains(&feed_b), "{remote:?}");
    let (added, removed) = s.pushed_subscriptions();
    assert!(added.contains(&feed_b) && removed.contains(&feed_a), "{added:?} / {removed:?}");
}

// ---------------------------------------------------------------------------------------------
// The wire
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn outbound_plays_are_antennapod_safe() {
    let base = setup().await;
    let url1 = base.0.feed("wire1.xml", &Feed::new("Wire One").episode(Episode::new(1).duration(None)));
    let url = base.0.feed("wire2.xml", &Feed::new("Wire Two").episode(Episode::new(2)));
    let pod1 = base.0.subscribe(&base.1, &url1, 1).await;
    let pod = base.0.subscribe(&base.1, &url, 1).await;
    let e1 = base.0.episode_id(&base.1, pod1, "Episode 1").await;
    let e2 = base.0.episode_id(&base.1, pod, "Episode 2").await;
    record(&base.0, &base.1, e1, 90.0).await;
    let s = with_target(base, TargetKind::Gpodder).await;
    s.run(RunMode::Incremental).await.unwrap();

    // No known duration (the parser estimates one from the enclosure size otherwise).
    exec(&s.app, &format!(r#"UPDATE "Episodes" SET EpisodeDuration = 0 WHERE EpisodeID = {e1}"#)).await;
    record(&s.app, &s.user, e1, 120.0).await;
    let (uid, ts) = (s.user.id, Utc::now().timestamp());
    exec(
        &s.app,
        &format!(
            r#"INSERT INTO "GpodderSyncEpisodeActions" (UserID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Started, Position, Total, EpisodeID, Origin) VALUES
               ({uid}, '{url}', '{e2u}', 'play', {ts}, {ts}, NULL, 600, NULL, {e2}, 'client'),
               ({uid}, '{url}', '{e2u}', 'play', {ts}, {ts}, NULL, 0, NULL, {e2}, 'client'),
               ({uid}, '{url}', '{e2u}', 'play', {ts}, {ts}, NULL, NULL, NULL, {e2}, 'client')"#,
            e2u = ep_url(2)
        ),
    )
    .await;
    s.run(RunMode::Incremental).await.unwrap();

    let pushed = s.pushed_actions();
    for (i, a) in pushed.iter().enumerate() {
        assert_eq!(check_episode_action(i, a), Vec::<String>::new());
        assert!(!a["timestamp"].as_str().unwrap().contains('+') && !a["timestamp"].as_str().unwrap().ends_with('Z'), "{a}");
        assert_eq!(a["device"], "pinepods", "{a}");
    }
    let p1 = play_of(&pushed, &ep_url(1));
    assert_eq!((p1.last().unwrap()["position"].as_i64(), p1.last().unwrap()["total"].as_i64()), (Some(120), Some(120)));
    let p2 = play_of(&pushed, &ep_url(2));
    assert_eq!(p2.len(), 1, "plays without a position aren't sent: {p2:#?}");
    assert_eq!((p2[0]["position"].as_i64(), p2[0]["total"].as_i64(), p2[0]["guid"].as_str()), (Some(600), Some(1802), Some("guid-2")));
}

#[tokio::test(flavor = "multi_thread")]
async fn pinepods_never_syncs_as_another_clients_device() {
    let s = with_target(setup().await, TargetKind::Gpodder).await;
    s.phone().await; // creates the phone's device
    // Older setups stored a device picked from the server's list, often the phone's.
    exec(&s.app, &format!(r#"UPDATE "SyncTargets" SET DeviceName = 'phone' WHERE TargetID = {}"#, s.target)).await;

    let before = s.mock.requests().len();
    s.run(RunMode::Incremental).await.unwrap();
    assert_eq!(s.target().await.device_name.as_deref(), Some("pinepods"));
    let paths: Vec<String> = s.mock.requests().into_iter().skip(before).map(|r| r.path).collect();
    assert!(paths.iter().any(|p| p == "/api/2/subscriptions/remote/pinepods.json"), "{paths:?}");
    assert!(!paths.iter().any(|p| p.contains("/phone")), "{paths:?}");
    let devices = GpodderClient::new(&s.mock.base_url, REMOTE_USER, REMOTE_PASS, "x");
    devices.login().await.assert_ok();
    let list = devices.devices().await.ok_json();
    let phone = list.as_array().unwrap().iter().find(|d| d["id"] == "phone").unwrap();
    assert_eq!(phone["type"], "mobile", "the phone's device is left alone: {list}");
}

#[tokio::test(flavor = "multi_thread")]
async fn stored_errors_never_contain_credentials() {
    let secret = "hunter2-very-secret";
    let (app, user, mock) = setup().await;
    let port = mock.base_url.rsplit(':').next().unwrap().to_string();
    let cases = [
        (TargetKind::Gpodder, mock.base_url.clone(), "401"),
        (TargetKind::Gpodder, format!("http://{REMOTE_USER}:{secret}@127.0.0.1:{port}"), "401"),
        (TargetKind::Nextcloud, mock.base_url.clone(), "401"),
        (TargetKind::Gpodder, "http://127.0.0.1:9".to_string(), "connect"),
    ];
    for (kind, url, expect) in cases {
        let target = app.db().sync_target_save(user.id, kind, &url, REMOTE_USER, secret).await.unwrap();
        let err = engine::run(app.db(), &app.state.task_spawner, user.id, target, RunMode::Incremental).await.unwrap_err().to_string();
        let stored = app.db().sync_target(target).await.unwrap().unwrap().last_error.unwrap();
        assert!(stored.contains(expect), "{url}: {stored}");
        assert!(!stored.contains(secret) && !err.contains(secret), "{url}: {stored} / {err}");
    }
}

// ---------------------------------------------------------------------------------------------
// Settings endpoints
// ---------------------------------------------------------------------------------------------

async fn sync_type(app: &TestApp, user: &TestUser) -> String {
    text(app, &format!(r#"SELECT Pod_Sync_Type FROM "Users" WHERE UserID = {}"#, user.id)).await.unwrap_or_default()
}

async fn targets(app: &TestApp, user: &TestUser) -> Vec<SyncTargetRow> {
    app.db().sync_targets_for_user(user.id).await.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn settings_endpoints_create_replace_and_remove_the_target() {
    let (app, user, mock) = setup().await;
    let feed = app.feed("settings.xml", &Feed::with_episodes("Settings Show", 1));
    app.subscribe(&user, &feed, 1).await;
    exec(&app, &format!(r#"UPDATE "Users" SET SyncNotice = 'reenter_external' WHERE UserID = {}"#, user.id)).await;

    let creds = |password: &str| json!({ "gpodder_url": mock.base_url, "gpodder_username": REMOTE_USER, "gpodder_password": password });
    app.post("/api/data/verify_gpodder_auth").key(&user).json(&creds(REMOTE_PASS)).send().await.assert_ok();
    app.post("/api/data/verify_gpodder_auth").key(&user).json(&creds("wrong")).send().await.assert_status(401);

    // Adding the server saves an encrypted target and starts its first sync.
    app.post("/api/data/add_gpodder_server").key(&user).json(&creds(REMOTE_PASS)).send().await.assert_ok();
    let t = targets(&app, &user).await;
    assert_eq!(t.len(), 1);
    assert_eq!((t[0].kind, t[0].url.as_str(), t[0].username.as_str(), t[0].enabled), (TargetKind::Gpodder, mock.base_url.as_str(), REMOTE_USER, true));
    assert_ne!(t[0].encrypted_secret, REMOTE_PASS);
    assert_eq!(app.db().decrypt_password(&t[0].encrypted_secret).await.unwrap(), REMOTE_PASS);
    assert_eq!(text(&app, &format!(r#"SELECT SyncNotice FROM "Users" WHERE UserID = {}"#, user.id)).await, None);
    assert_eq!(sync_type(&app, &user).await, "external");
    let first = eventually("first sync finished", || async {
        targets(&app, &user).await.into_iter().next().filter(|t| t.last_success_at.is_some())
    })
    .await;
    assert!(!first.needs_full_sync);
    assert!(mock.subscriptions(REMOTE_USER).contains(&feed));

    // The internal server alongside it is "both"; switching it off leaves the target.
    app.post("/api/data/gpodder/toggle").key(&user).json(&json!({ "enabled": true })).send().await.assert_ok();
    assert_eq!(sync_type(&app, &user).await, "both");
    app.post("/api/data/gpodder/toggle").key(&user).json(&json!({ "enabled": false })).send().await.assert_ok();
    assert_eq!(sync_type(&app, &user).await, "external");
    assert_eq!(targets(&app, &user).await.len(), 1);

    // Sync now / force full sync run the engine.
    let synced = app.post("/api/gpodder/sync").key(&user).send().await.ok_json();
    assert_eq!(synced["success"], true, "{synced}");
    let full_pulls = || mock.requests().iter().filter(|r| r.path.starts_with("/api/2/subscriptions/") && r.query.as_deref() == Some("since=0")).count();
    let before = full_pulls();
    let forced = app.post("/api/gpodder/sync/force").key(&user).send().await.ok_json();
    assert_eq!(forced["success"], true, "{forced}");
    assert_eq!(full_pulls(), before + 1);

    // The Nextcloud login flow replaces the gpodder target.
    let poll = app.fixtures().serve(
        "nextcloud-poll",
        Fixture::new("application/json", json!({ "server": mock.base_url, "loginName": REMOTE_USER, "appPassword": REMOTE_PASS }).to_string()),
    );
    app.post("/api/data/add_nextcloud_server")
        .key(&user)
        .json(&json!({ "user_id": user.id, "token": "poll-token", "poll_endpoint": poll, "nextcloud_url": mock.base_url }))
        .send()
        .await
        .assert_ok();
    let nc = eventually("Nextcloud target synced", || async {
        targets(&app, &user).await.into_iter().find(|t| t.kind == TargetKind::Nextcloud && t.last_success_at.is_some())
    })
    .await;
    assert_eq!((nc.username.as_str(), targets(&app, &user).await.len()), (REMOTE_USER, 1));
    assert_eq!(sync_type(&app, &user).await, "nextcloud");

    // Removing sync removes the target.
    app.delete("/api/data/remove_podcast_sync").key(&user).json(&json!({ "user_id": user.id })).send().await.assert_ok();
    assert!(targets(&app, &user).await.is_empty());
    assert_eq!(sync_type(&app, &user).await, "None");
}
