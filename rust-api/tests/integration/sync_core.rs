//! Sync core: the episode resolver, local writes recorded in the
//! sync log (events), applying remote actions and subscription changes, and rematching actions
//! that arrived before their episode.

use chrono::{DateTime, Utc};
use pinepods_api::database::DatabasePool;
use pinepods_api::services::sync::{self, EpisodeAction, ResolveKey, SyncOrigin};
use sqlx::Row;

use crate::harness::*;

async fn enable_sync(app: &TestApp, user: &TestUser) {
    exec(app, &format!(r#"UPDATE "Users" SET GpodderServerEnabled = TRUE WHERE UserID = {}"#, user.id)).await;
    sync::invalidate_sync_enabled(user.id);
}

/// One row of GpodderSyncEpisodeActions, oldest first.
#[derive(Debug, Clone, PartialEq)]
struct ActionRow {
    id: i64,
    action: String,
    origin: String,
    episode_id: Option<i64>,
    position: Option<i64>,
    total: Option<i64>,
    guid: Option<String>,
    timestamp: i64,
    server_ts: i64,
    device: Option<String>,
}

async fn action_log(app: &TestApp, user: &TestUser) -> Vec<ActionRow> {
    let sql = format!(
        r#"SELECT a.ActionID AS id, a.Action AS action, a.Origin AS origin, a.EpisodeID AS episode_id, a.Position AS position,
                  a.Total AS total, a.Guid AS guid, a.Timestamp AS ts, a.ServerTs AS server_ts, d.DeviceName AS device
           FROM "GpodderSyncEpisodeActions" a LEFT JOIN "GpodderDevices" d ON d.DeviceID = a.DeviceID
           WHERE a.UserID = {} ORDER BY a.ActionID"#,
        user.id
    );
    fn row<R: Row>(r: &R) -> ActionRow
    where
        for<'a> &'a str: sqlx::ColumnIndex<R>,
        i32: for<'q> sqlx::Decode<'q, R::Database> + sqlx::Type<R::Database>,
        i64: for<'q> sqlx::Decode<'q, R::Database> + sqlx::Type<R::Database>,
        String: for<'q> sqlx::Decode<'q, R::Database> + sqlx::Type<R::Database>,
    {
        let int = |c: &str| r.try_get::<Option<i32>, _>(c).map(|v| v.map(i64::from)).or_else(|_| r.try_get::<Option<i64>, _>(c)).unwrap();
        ActionRow {
            id: int("id").unwrap(),
            action: r.try_get("action").unwrap(),
            origin: r.try_get("origin").unwrap(),
            episode_id: int("episode_id"),
            position: int("position"),
            total: int("total"),
            guid: r.try_get("guid").unwrap(),
            timestamp: int("ts").unwrap(),
            server_ts: int("server_ts").unwrap(),
            device: r.try_get("device").unwrap(),
        }
    }
    match app.db() {
        DatabasePool::Postgres(pool) => {
            sqlx::query(sqlx::AssertSqlSafe(sql)).fetch_all(pool).await.unwrap().iter().map(row).collect()
        }
        DatabasePool::MySQL(pool) => sqlx::query(sqlx::AssertSqlSafe(sql.replace('"', "")))
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .map(row)
            .collect(),
    }
}

/// `(action, origin, url)` rows of GpodderSyncSubscriptions, oldest first.
async fn subscription_log(app: &TestApp, user: &TestUser) -> Vec<(String, String, String)> {
    let sql = format!(
        r#"SELECT Action AS action, Origin AS origin, PodcastURL AS url FROM "GpodderSyncSubscriptions" WHERE UserID = {} ORDER BY SubscriptionID"#,
        user.id
    );
    let get = |a: String, o: String, u: String| (a, o, u);
    match app.db() {
        DatabasePool::Postgres(pool) => sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .map(|r| get(r.get("action"), r.get("origin"), r.get("url")))
            .collect(),
        DatabasePool::MySQL(pool) => sqlx::query(sqlx::AssertSqlSafe(sql.replace('"', "")))
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .map(|r| get(r.get("action"), r.get("origin"), r.get("url")))
            .collect(),
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

async fn mark(app: &TestApp, user: &TestUser, ep: i32, path: &str) {
    app.post(path)
        .key(user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
}

async fn position(app: &TestApp, user: &TestUser, ep: i32) -> Option<i64> {
    int(app, &format!(r#"SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = {} AND EpisodeID = {ep}"#, user.id)).await
}

async fn completed(app: &TestApp, ep: i32) -> bool {
    int(app, &format!(r#"SELECT Completed FROM "Episodes" WHERE EpisodeID = {ep}"#)).await == Some(1)
}

async fn listen_date(app: &TestApp, user: &TestUser, ep: i32) -> Option<i64> {
    let sql = match app.backend() {
        Backend::Postgres => format!(
            r#"SELECT FLOOR(EXTRACT(EPOCH FROM ListenDate))::BIGINT FROM "UserEpisodeHistory" WHERE UserID = {} AND EpisodeID = {ep}"#,
            user.id
        ),
        Backend::MariaDb => format!(
            "SELECT TIMESTAMPDIFF(SECOND, '1970-01-01 00:00:00', ListenDate) FROM UserEpisodeHistory WHERE UserID = {} AND EpisodeID = {ep}",
            user.id
        ),
    };
    int(app, &sql).await
}

fn ts(secs_ago: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(Utc::now().timestamp() - secs_ago, 0).unwrap()
}

fn action(v: Value) -> EpisodeAction {
    serde_json::from_value(v).unwrap()
}

fn play(feed: &str, ep_url: &str, position: i32, total: Option<i32>, at: DateTime<Utc>) -> EpisodeAction {
    action(json!({
        "podcast": feed, "episode": ep_url, "action": "play",
        "timestamp": at.timestamp(), "started": 0, "position": position, "total": total,
    }))
}

fn ep_url(n: u32) -> String {
    format!("https://cdn.example.invalid/audio/ep{n}.mp3")
}

/// Put the episode's history row at a known point in the past, so tests choose which actions
/// are newer.
async fn set_listen_date(app: &TestApp, user: &TestUser, ep: i32, at: DateTime<Utc>) {
    exec(
        app,
        &format!(
            r#"UPDATE "UserEpisodeHistory" SET ListenDate = '{}' WHERE UserID = {} AND EpisodeID = {ep}"#,
            at.format("%Y-%m-%d %H:%M:%S"),
            user.id
        ),
    )
    .await;
}

// ---------------------------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn resolver_matches_guid_then_url_then_url_base_within_the_user() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let carol = app.user("carol").await;
    let feed = Feed::new("Resolver Show")
        .episode(Episode::new(1))
        .episode(Episode::new(2))
        .episode(Episode::new(3))
        .episode(Episode::new(4).enclosure("https://cdn.example.invalid/audio/ep4.mp3?token=abc"));
    let url = app.feed("resolver.xml", &feed);
    let pod_a = app.subscribe(&alice, &url, 4).await;
    let pod_b = app.subscribe(&bob, &url, 4).await;
    let a = |n: u32| {
        let app = &app;
        let alice = &alice;
        async move { app.episode_id(alice, pod_a, &format!("Episode {n}")).await }
    };
    let (a1, a2, a3, a4) = (a(1).await, a(2).await, a(3).await, a(4).await);
    let b1 = app.episode_id(&bob, pod_b, "Episode 1").await;

    let ep2 = ep_url(2);
    let ep3_tracked = format!("{}?utm_source=app", ep_url(3));
    let keys = [
        // GUID wins, even though the URL is unknown
        ResolveKey { podcast_url: &url, episode_url: "https://elsewhere.invalid/renamed.mp3", guid: Some("guid-1") },
        // a GUID only counts within the feed it's sent with; the URL still matches
        ResolveKey { podcast_url: "https://other.invalid/feed.xml", episode_url: &ep2, guid: Some("guid-1") },
        // stored URL = the action's URL without its query string
        ResolveKey { podcast_url: &url, episode_url: &ep3_tracked, guid: None },
        // stored URL has a different query string: matched by base within the feed
        ResolveKey { podcast_url: &url, episode_url: "https://cdn.example.invalid/audio/ep4.mp3?token=zzz", guid: None },
        // ...but not across feeds (that would be a scan of the whole library)
        ResolveKey { podcast_url: "https://other.invalid/feed.xml", episode_url: "https://cdn.example.invalid/audio/ep4.mp3", guid: None },
        ResolveKey { podcast_url: &url, episode_url: "https://cdn.example.invalid/audio/missing.mp3", guid: Some("guid-999") },
    ];
    let got = sync::resolve(app.db(), alice.id, &keys).await.unwrap();
    assert_eq!(got, vec![Some(a1), Some(a2), Some(a3), Some(a4), None, None]);

    // Always scoped to the user: bob gets his own copy, carol (not subscribed) nothing.
    let got = sync::resolve(app.db(), bob.id, &keys[..1]).await.unwrap();
    assert_eq!(got, vec![Some(b1)]);
    assert_ne!(b1, a1);
    let got = sync::resolve(app.db(), carol.id, &keys).await.unwrap();
    assert!(got.iter().all(Option::is_none), "{got:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn resolver_handles_batches_larger_than_one_query() {
    let app = TestApp::new().await;
    let user = app.user("batcher").await;
    let url = app.feed("batch.xml", &Feed::with_episodes("Batch", 3));
    let pod = app.subscribe(&user, &url, 3).await;
    let e3 = app.episode_id(&user, pod, "Episode 3").await;

    // 1201 keys span three resolver queries; the order of answers must survive the chunking.
    let urls: Vec<String> = (0..1201).map(|i| if i % 400 == 399 { ep_url(3) } else { format!("https://x.invalid/{i}.mp3") }).collect();
    let keys: Vec<ResolveKey> = urls.iter().map(|u| ResolveKey { podcast_url: &url, episode_url: u, guid: None }).collect();
    let got = sync::resolve(app.db(), user.id, &keys).await.unwrap();
    assert_eq!(got.len(), 1201);
    for (i, id) in got.iter().enumerate() {
        assert_eq!(*id, (i % 400 == 399).then_some(e3), "key {i}");
    }
}

// ---------------------------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn local_changes_are_logged_and_progress_coalesces_until_pushed() {
    let app = TestApp::new().await;
    let user = app.user("syncer").await;
    enable_sync(&app, &user).await;
    let url = app.feed("events.xml", &Feed::with_episodes("Event Show", 2));
    let pod = app.subscribe(&user, &url, 2).await;
    let ep = app.episode_id(&user, pod, "Episode 1").await;

    // Subscribing is an `add` from the pinepods device, created on first use.
    assert_eq!(subscription_log(&app, &user).await, vec![("add".into(), "pinepods".into(), url.clone())]);
    let device = text(&app, &format!(r#"SELECT DeviceName FROM "GpodderDevices" WHERE UserID = {} AND SystemRole = 'pinepods'"#, user.id)).await;
    assert_eq!(device.as_deref(), Some("pinepods"));

    // Progress saves every few seconds collapse into one play row, timestamps bumped.
    record(&app, &user, ep, 100.0).await;
    let first = action_log(&app, &user).await;
    assert_eq!(first.len(), 1);
    exec(&app, &format!(r#"UPDATE "GpodderSyncEpisodeActions" SET ServerTs = 1, Timestamp = 1 WHERE ActionID = {}"#, first[0].id)).await;
    record(&app, &user, ep, 250.0).await;
    record(&app, &user, ep, 200.0).await; // not stored (progress only moves forward locally), so not logged
    let log = action_log(&app, &user).await;
    assert_eq!(log.len(), 1, "{log:#?}");
    let row = &log[0];
    assert_eq!((row.action.as_str(), row.origin.as_str()), ("play", "pinepods"));
    assert_eq!((row.position, row.total), (Some(250), Some(1801)));
    assert_eq!((row.episode_id, row.guid.as_deref(), row.device.as_deref()), (Some(ep as i64), Some("guid-1"), Some("pinepods")));
    assert!(row.server_ts > 1 && row.timestamp > 1, "coalescing bumps ServerTs/Timestamp: {row:?}");

    // Once a target has pushed that row, the next save starts a new one.
    let pushed = row.id;
    exec(
        &app,
        &format!(
            r#"INSERT INTO "SyncTargets" (UserID, Kind, Url, Username, EncryptedSecret, ActionsPushCursor) VALUES ({}, 'gpodder', 'https://gpodder.invalid', 'u', 's', {pushed})"#,
            user.id
        ),
    )
    .await;
    record(&app, &user, ep, 400.0).await;
    let log = action_log(&app, &user).await;
    assert_eq!(log.iter().map(|r| r.position).collect::<Vec<_>>(), vec![Some(250), Some(400)]);

    // Completing updates the unpushed row to position = total = duration.
    mark(&app, &user, ep, "/api/data/mark_episode_completed").await;
    let log = action_log(&app, &user).await;
    assert_eq!(log.len(), 2);
    assert_eq!((log[1].action.as_str(), log[1].position, log[1].total), ("play", Some(1801), Some(1801)));

    // Uncompleting is a `new`.
    mark(&app, &user, ep, "/api/data/mark_episode_uncompleted").await;
    let log = action_log(&app, &user).await;
    assert_eq!(log.len(), 3);
    assert_eq!((log[2].action.as_str(), log[2].position, log[2].episode_id), ("new", None, Some(ep as i64)));

    // ...and progress after a `new` starts a new play row rather than rewriting the `new`.
    record(&app, &user, ep, 30.0).await;
    let log = action_log(&app, &user).await;
    assert_eq!(log.iter().map(|r| r.action.as_str()).collect::<Vec<_>>(), vec!["play", "play", "new", "play"]);

    // Unsubscribing is a `remove`; the log keeps its rows, unmatched now (EpisodeID SET NULL).
    app.post("/api/data/remove_podcast_id")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod }))
        .send()
        .await
        .assert_ok();
    assert_eq!(
        subscription_log(&app, &user).await,
        vec![("add".into(), "pinepods".into(), url.clone()), ("remove".into(), "pinepods".into(), url.clone())]
    );
    assert!(action_log(&app, &user).await.iter().all(|r| r.episode_id.is_none()));
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_is_logged_for_users_without_sync() {
    let app = TestApp::new().await;
    let user = app.user("loner").await;
    let url = app.feed("quiet.xml", &Feed::with_episodes("Quiet Show", 1));
    let pod = app.subscribe(&user, &url, 1).await;
    let ep = app.episode_id(&user, pod, "Episode 1").await;
    record(&app, &user, ep, 100.0).await;
    mark(&app, &user, ep, "/api/data/mark_episode_completed").await;
    mark(&app, &user, ep, "/api/data/mark_episode_uncompleted").await;
    app.post("/api/data/remove_podcast_id")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod }))
        .send()
        .await
        .assert_ok();

    assert!(action_log(&app, &user).await.is_empty());
    assert!(subscription_log(&app, &user).await.is_empty());
    let devices = int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderDevices" WHERE UserID = {}"#, user.id)).await;
    assert_eq!(devices, Some(0), "no pinepods device is created for a user who doesn't sync");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_pinepods_device_avoids_a_name_a_client_owns() {
    let app = TestApp::new().await;
    let user = app.user("crowded").await;
    exec(
        &app,
        &format!(r#"INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, DeviceCaption) VALUES ({}, 'pinepods', 'mobile', 'A phone')"#, user.id),
    )
    .await;
    enable_sync(&app, &user).await;
    let url = app.feed("crowded.xml", &Feed::with_episodes("Crowded", 1));
    app.subscribe(&user, &url, 1).await;

    let device = text(&app, &format!(r#"SELECT DeviceName FROM "GpodderDevices" WHERE UserID = {} AND SystemRole = 'pinepods'"#, user.id)).await;
    assert_eq!(device.as_deref(), Some("pinepods-server"));
    assert_eq!(subscription_log(&app, &user).await.len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_old_internal_device_is_claimed_as_the_pinepods_device() {
    let app = TestApp::new().await;
    let user = app.user("legacy").await;
    let legacy = format!("pinepods-internal-{}", user.id);
    exec(
        &app,
        &format!(r#"INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType, DeviceCaption) VALUES ({}, '{legacy}', 'server', 'Pinepods')"#, user.id),
    )
    .await;
    enable_sync(&app, &user).await;
    let url = app.feed("legacy.xml", &Feed::with_episodes("Legacy", 1));
    app.subscribe(&user, &url, 1).await;

    let devices = int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderDevices" WHERE UserID = {}"#, user.id)).await;
    assert_eq!(devices, Some(1), "no second Pinepods device");
    let device = text(&app, &format!(r#"SELECT DeviceName FROM "GpodderDevices" WHERE UserID = {} AND SystemRole = 'pinepods'"#, user.id)).await;
    assert_eq!(device, Some(legacy));
}

// ---------------------------------------------------------------------------------------------
// Apply
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn last_write_wins_and_positions_can_go_backwards() {
    let app = TestApp::new().await;
    let user = app.user("lww").await;
    enable_sync(&app, &user).await;
    let url = app.feed("lww.xml", &Feed::with_episodes("LWW", 1));
    let pod = app.subscribe(&user, &url, 1).await;
    let ep = app.episode_id(&user, pod, "Episode 1").await;
    record(&app, &user, ep, 900.0).await;
    set_listen_date(&app, &user, ep, ts(3600)).await;
    let logged = action_log(&app, &user).await.len();

    // Older than the local change: ignored.
    let older = play(&url, &ep_url(1), 100, Some(1801), ts(7200));
    let s = sync::apply_episode_actions(app.db(), user.id, &[older], SyncOrigin::Client).await.unwrap();
    assert_eq!((s.matched, s.applied, s.older), (1, 0, 1));
    assert_eq!(position(&app, &user, ep).await, Some(900));

    // Newer, with a smaller position: applied (no more GREATEST), as of the action's time.
    // Of two actions for the same episode in one batch, only the newest counts.
    let at = ts(60);
    let batch = [play(&url, &ep_url(1), 50, Some(1801), ts(600)), play(&url, &ep_url(1), 300, Some(1801), at)];
    let s = sync::apply_episode_actions(app.db(), user.id, &batch, SyncOrigin::Client).await.unwrap();
    assert_eq!((s.received, s.applied), (2, 1));
    assert_eq!(position(&app, &user, ep).await, Some(300));
    assert_eq!(listen_date(&app, &user, ep).await, Some(at.timestamp()));
    assert!(!completed(&app, ep).await);

    // A client's change was already logged by the server: nothing new in the log.
    assert_eq!(action_log(&app, &user).await.len(), logged);
}

#[tokio::test(flavor = "multi_thread")]
async fn completion_threshold_new_and_ignored_actions() {
    let app = TestApp::new().await;
    let user = app.user("finisher").await;
    let url = app.feed("threshold.xml", &Feed::with_episodes("Threshold", 4));
    let pod = app.subscribe(&user, &url, 4).await;
    let ep = |n: u32| {
        let (app, user) = (&app, &user);
        async move { app.episode_id(user, pod, &format!("Episode {n}")).await }
    };
    let (e1, e2, e3, e4) = (ep(1).await, ep(2).await, ep(3).await, ep(4).await);
    let apply = |actions: Vec<EpisodeAction>| {
        let (app, user) = (&app, &user);
        async move { sync::apply_episode_actions(app.db(), user.id, &actions, SyncOrigin::Client).await.unwrap() }
    };
    let at = ts(500);

    // Auto-complete off: within 60s of the end completes (durations are 1800 + n).
    apply(vec![play(&url, &ep_url(1), 1801 - 61, None, at), play(&url, &ep_url(2), 1802 - 60, None, at)]).await;
    assert!(!completed(&app, e1).await);
    assert_eq!(position(&app, &user, e1).await, Some(1740));
    assert!(completed(&app, e2).await);
    assert_eq!(listen_date(&app, &user, e2).await, Some(at.timestamp()), "completion is as of the action");

    // The user's setting wins when it's on.
    exec(&app, &format!(r#"UPDATE "UserSettings" SET AutoCompleteSeconds = 120 WHERE UserID = {}"#, user.id)).await;
    apply(vec![play(&url, &ep_url(3), 1803 - 100, None, at)]).await;
    assert!(completed(&app, e3).await);

    // `new` uncompletes and resets the position.
    apply(vec![action(json!({"podcast": url, "episode": ep_url(2), "action": "new", "timestamp": ts(400).timestamp()}))]).await;
    assert!(!completed(&app, e2).await);
    assert_eq!(position(&app, &user, e2).await, Some(0));

    // download/delete change nothing, even when they're the newest action.
    let s = apply(vec![
        action(json!({"podcast": url, "episode": ep_url(1), "action": "download", "timestamp": ts(10).timestamp()})),
        action(json!({"podcast": url, "episode": ep_url(1), "action": "delete", "timestamp": ts(5).timestamp()})),
    ])
    .await;
    assert_eq!(s.applied, 0);
    assert_eq!(position(&app, &user, e1).await, Some(1740));
    assert!(!completed(&app, e1).await);

    // Unknown local duration: the action's total is stored and used for the threshold.
    exec(&app, &format!(r#"UPDATE "Episodes" SET EpisodeDuration = 0 WHERE EpisodeID = {e4}"#)).await;
    apply(vec![play(&url, &ep_url(4), 2390, Some(2400), ts(300))]).await;
    assert_eq!(int(&app, &format!(r#"SELECT EpisodeDuration FROM "Episodes" WHERE EpisodeID = {e4}"#)).await, Some(2400));
    assert!(completed(&app, e4).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn applied_changes_are_tagged_with_their_origin_and_not_echoed() {
    let app = TestApp::new().await;
    let user = app.user("tagged").await;
    enable_sync(&app, &user).await;
    let url = app.feed("tagged.xml", &Feed::with_episodes("Tagged", 2));
    let pod = app.subscribe(&user, &url, 2).await;
    let e1 = app.episode_id(&user, pod, "Episode 1").await;
    let e2 = app.episode_id(&user, pod, "Episode 2").await;
    assert!(action_log(&app, &user).await.is_empty());

    // Internal-server client: applied, nothing logged (the server logged it), no pinepods row.
    sync::apply_episode_actions(app.db(), user.id, &[play(&url, &ep_url(1), 120, Some(1801), ts(60))], SyncOrigin::Client)
        .await
        .unwrap();
    assert_eq!(position(&app, &user, e1).await, Some(120));
    assert!(action_log(&app, &user).await.is_empty());

    // External target: logged as target:<id>, matched or not; still no pinepods rows. Completing
    // through apply doesn't produce a pinepods `play` either.
    let at = ts(30);
    let actions = [
        play(&url, &ep_url(2), 1802, Some(1802), at),
        play(&url, "https://cdn.example.invalid/audio/not-yet.mp3", 50, None, at),
    ];
    sync::apply_episode_actions(app.db(), user.id, &actions, SyncOrigin::Target(7)).await.unwrap();
    assert!(completed(&app, e2).await);
    let log = action_log(&app, &user).await;
    assert_eq!(log.iter().map(|r| (r.origin.as_str(), r.episode_id)).collect::<Vec<_>>(), vec![("target:7", Some(e2 as i64)), ("target:7", None)]);
    assert_eq!(log[0].timestamp, at.timestamp(), "the action's own time is kept");

    // A subscription removed by a target: logged as the target's, not as a pinepods remove.
    let s = sync::apply_subscription_changes(app.db(), &app.state.task_spawner, user.id, &[], &[url.clone()], SyncOrigin::Target(7))
        .await
        .unwrap();
    assert_eq!(s.removed, 1);
    assert!(app.podcasts(&user).await.is_empty());
    assert_eq!(
        subscription_log(&app, &user).await,
        vec![("add".into(), "pinepods".into(), url.clone()), ("remove".into(), "target:7".into(), url.clone())]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn an_action_for_an_episode_not_yet_ingested_is_applied_after_refresh() {
    let app = TestApp::new().await;
    let user = app.user("early").await;
    enable_sync(&app, &user).await;
    let feed = Feed::with_episodes("Early Bird", 1);
    let url = app.feed("early.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    // A client uploads a play for episode 2 before Pinepods has seen it (as the gpodder server
    // stores it: unmatched).
    let at = ts(120).timestamp();
    exec(
        &app,
        &format!(
            r#"INSERT INTO "GpodderSyncEpisodeActions" (UserID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Started, Position, Total, Origin)
               VALUES ({}, '{url}', '{}', 'play', {at}, {at}, 0, 700, 1802, 'client')"#,
            user.id,
            ep_url(2)
        ),
    )
    .await;
    assert_eq!(action_log(&app, &user).await[0].episode_id, None);

    app.feed("early.xml", &feed.episode(Episode::new(2)));
    app.refresh().await;
    let e2 = app.episode_id(&user, pod, "Episode 2").await;
    assert_eq!(action_log(&app, &user).await[0].episode_id, Some(e2 as i64), "matched after ingest");
    assert_eq!(position(&app, &user, e2).await, Some(700), "and applied");
    assert_eq!(listen_date(&app, &user, e2).await, Some(at));
}

#[tokio::test(flavor = "multi_thread")]
async fn subscription_adds_are_queued_once_and_never_fetched_inline() {
    let app = TestApp::new().await;
    let user = app.user("queuer").await;
    enable_sync(&app, &user).await;
    let url = app.feed("queued.xml", &Feed::with_episodes("Queued", 2));
    let tasks = &app.state.task_spawner;

    // Two devices report the same feed at the same moment (one of them twice, untrimmed).
    let first = [url.clone(), format!(" {url} ")];
    let second = [url.clone()];
    let (a, b) = tokio::join!(
        sync::apply_subscription_changes(app.db(), tasks, user.id, &first, &[], SyncOrigin::Client),
        sync::apply_subscription_changes(app.db(), tasks, user.id, &second, &[], SyncOrigin::Client),
    );
    assert_eq!(a.unwrap().queued + b.unwrap().queued, 1, "one import per (user, url)");

    let pod = eventually("queued podcast imported", || async {
        app.podcasts(&user).await.first().and_then(|p| p["podcastid"].as_i64())
    })
    .await as i32;
    eventually("episodes imported", || async { (app.episodes(&user, pod).await.len() == 2).then_some(()) }).await;
    assert_eq!(app.fixtures().hits("queued.xml").len(), 1, "the feed is fetched once, by the import");

    // Already subscribed: nothing to queue. A client's add isn't logged again as a pinepods add.
    let again = sync::apply_subscription_changes(app.db(), tasks, user.id, &[url.clone()], &[], SyncOrigin::Client).await.unwrap();
    assert_eq!(again.queued, 0);
    assert!(subscription_log(&app, &user).await.is_empty());

    // Contradictory or unusable changes are ignored.
    let s = sync::apply_subscription_changes(
        app.db(),
        tasks,
        user.id,
        &[url.clone(), "ftp://files.invalid/feed".into()],
        &[url.clone()],
        SyncOrigin::Client,
    )
    .await
    .unwrap();
    assert_eq!((s.queued, s.removed, s.rejected), (0, 0, 2));
    assert_eq!(app.podcasts(&user).await.len(), 1);
}

/// Nightly compaction drops superseded log rows past the retention window, keeps the newest row
/// per item, and leaves rows an external target hasn't pushed yet.
#[tokio::test(flavor = "multi_thread")]
async fn compaction_keeps_the_latest_state_and_the_outbox() {
    let app = TestApp::new().await;
    let user = app.user("compact").await;
    let other = app.user("compact-other").await;
    let old = Utc::now().timestamp() - 100 * 86_400;
    let recent = Utc::now().timestamp() - 86_400;
    let pod = "https://feeds.example.test/compact.xml";
    let ep = |n: i32| format!("https://cdn.example.test/compact/ep{n}.mp3");
    let action = |uid: i32, ep: &str, kind: &str, at: i64| {
        format!("({uid}, '{pod}', '{ep}', '{kind}', {at}, {at}, 0, 60, 600, 'client')")
    };
    let rows = [
        action(user.id, &ep(1), "play", old),       // superseded by the next play: deleted
        action(user.id, &ep(1), "play", old + 10),  // superseded by the recent play: deleted
        action(user.id, &ep(1), "download", old),   // newest download of ep1: kept
        action(user.id, &ep(1), "play", recent),    // newest: kept
        action(user.id, &ep(2), "play", old),       // only row for ep2: kept
        action(other.id, &ep(1), "play", old),      // another user's newest: kept
        action(other.id, &ep(1), "play", old + 10),
    ];
    exec(
        &app,
        &format!(
            r#"INSERT INTO "GpodderSyncEpisodeActions" (UserID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Started, Position, Total, Origin) VALUES {}"#,
            rows.join(", ")
        ),
    )
    .await;
    for (url, kind, at) in [("https://a.example/feed", "add", old), ("https://a.example/feed", "remove", old + 5), ("https://a.example/feed", "add", old + 9), ("https://b.example/feed", "add", old)] {
        exec(&app, &format!(
            r#"INSERT INTO "GpodderSyncSubscriptions" (UserID, PodcastURL, Action, Timestamp, ServerTs, Origin) VALUES ({}, '{url}', '{kind}', {at}, {at}, 'client')"#,
            user.id
        ))
        .await;
    }
    let ids = |table: &'static str, id: &'static str, uid: i32| {
        let app = &app;
        async move {
            let n = int(app, &format!(r#"SELECT COUNT(*) FROM "{table}" WHERE UserID = {uid}"#)).await.unwrap();
            let min = int(app, &format!(r#"SELECT MIN({id}) FROM "{table}" WHERE UserID = {uid}"#)).await.unwrap();
            (n, min)
        }
    };
    let (_, first_action) = ids("GpodderSyncEpisodeActions", "ActionID", user.id).await;
    let (_, first_sub) = ids("GpodderSyncSubscriptions", "SubscriptionID", user.id).await;

    // A target that has pushed only the first action and the first subscription row.
    exec(&app, &format!(
        r#"INSERT INTO "SyncTargets" (UserID, Kind, Url, Username, EncryptedSecret, SubsPushCursor, ActionsPushCursor) VALUES ({}, 'gpodder', 'https://gpodder.invalid', 'u', 's', {first_sub}, {first_action})"#,
        user.id
    ))
    .await;
    let s = sync::compact::run(app.db()).await.unwrap();
    assert_eq!((s.actions, s.subscriptions), (1 + 1, 1), "only pushed rows go (plus the other user's, who has no target)");
    assert_eq!(ids("GpodderSyncEpisodeActions", "ActionID", user.id).await.0, 4);

    // Once everything is pushed, all superseded old rows go and the newest per item stays.
    exec(&app, &format!(r#"UPDATE "SyncTargets" SET SubsPushCursor = 9000000000000000000, ActionsPushCursor = 9000000000000000000 WHERE UserID = {}"#, user.id)).await;
    let s = sync::compact::run(app.db()).await.unwrap();
    assert_eq!((s.actions, s.subscriptions), (1, 1));
    let kept = |sql: String| {
        let app = &app;
        async move { int(app, &sql).await.unwrap() }
    };
    assert_eq!(kept(format!(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE UserID = {} AND Action = 'play' AND EpisodeURL = '{}'"#, user.id, ep(1))).await, 1);
    assert_eq!(kept(format!(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE UserID = {} AND ServerTs = {recent}"#, user.id)).await, 1, "the newest play stays");
    assert_eq!(ids("GpodderSyncEpisodeActions", "ActionID", user.id).await.0, 3, "newest play + download of ep1, ep2's play");
    assert_eq!(ids("GpodderSyncEpisodeActions", "ActionID", other.id).await.0, 1);
    assert_eq!(kept(format!(r#"SELECT COUNT(*) FROM "GpodderSyncSubscriptions" WHERE UserID = {} AND Action = 'add'"#, user.id)).await, 2, "latest per feed");

    // Nothing left to do.
    assert_eq!(sync::compact::run(app.db()).await.unwrap(), Default::default());
}
