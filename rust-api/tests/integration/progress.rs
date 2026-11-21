//! Listening progress: the newest observation of an episode wins whichever device reports it,
//! listened time counts only what was played, and the endpoints older apps call keep working
//! without being able to overwrite newer state.

use crate::harness::*;

/// A client clock, in unix milliseconds. Deliberately nowhere near the real time: only the gap
/// between `sent_at` and `at` may matter.
const CLOCK: i64 = 1_000_000_000_000;
const DAY_MS: i64 = 86_400_000;

async fn setup(app: &TestApp) -> (TestUser, i32, i32) {
    let user = app.user("listener").await;
    let url = app.feed("progress.xml", &Feed::with_episodes("Progress Show", 2));
    let pod = app.subscribe(&user, &url, 2).await;
    let ep = app.episode_id(&user, pod, "Episode 1").await; // 1801s long
    (user, pod, ep)
}

/// Send `updates` as a device whose clock reads `clock` and return the per-update results.
async fn send(app: &TestApp, key: &str, clock: i64, updates: Value) -> Vec<Value> {
    let body = app
        .post("/api/data/progress")
        .header("Api-Key", key)
        .json(&json!({ "sent_at": clock, "updates": updates }))
        .send()
        .await
        .ok_json();
    body["results"].as_array().cloned().expect("results")
}

/// Report `position`, observed `secs_ago` seconds before sending.
async fn report(app: &TestApp, user: &TestUser, ep: i32, position: f64, secs_ago: i64) -> Value {
    let update = json!([{ "episode_id": ep, "position": position, "at": CLOCK - secs_ago * 1000 }]);
    send(app, &user.api_key, CLOCK, update).await.remove(0)
}

/// The position report of apps that predate `/progress`.
async fn old_report(app: &TestApp, key: &str, user: &TestUser, ep: i32, secs: f64) {
    app.post("/api/data/record_listen_duration")
        .header("Api-Key", key)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "listen_duration": secs }))
        .send()
        .await
        .assert_ok();
}

async fn old_minute(app: &TestApp, key: &str, user: &TestUser) {
    app.put(&format!("/api/data/increment_listen_time/{}", user.id)).header("Api-Key", key).send().await.assert_ok();
}

async fn history(app: &TestApp, user: &TestUser, ep: i32, column: &str) -> Option<i64> {
    int(app, &format!(r#"SELECT {column} FROM "UserEpisodeHistory" WHERE UserID = {} AND EpisodeID = {ep}"#, user.id)).await
}

async fn position(app: &TestApp, user: &TestUser, ep: i32) -> Option<i64> {
    history(app, user, ep, "ListenDuration").await
}

async fn completed(app: &TestApp, user: &TestUser, ep: i32) -> bool {
    int(app, &format!(r#"SELECT COUNT(*) FROM "UserEpisodeState" WHERE UserID = {} AND EpisodeID = {ep} AND Completed = TRUE"#, user.id))
        .await
        == Some(1)
}

async fn stat(app: &TestApp, user: &TestUser, column: &str) -> i64 {
    int(app, &format!(r#"SELECT {column} FROM "UserStats" WHERE UserID = {}"#, user.id)).await.unwrap_or(0)
}

async fn mark(app: &TestApp, user: &TestUser, ep: i32, path: &str) {
    app.post(path).key(user).json(&json!({ "episode_id": ep, "user_id": user.id, "is_youtube": false })).send().await.assert_ok();
}

// --- conflicts between devices ---------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn a_rewind_is_saved() {
    let app = TestApp::new().await;
    let (user, pod, ep) = setup(&app).await;
    let first = report(&app, &user, ep, 600.4, 0).await;
    assert_eq!((first["applied"].clone(), first["position"].clone()), (json!(true), json!(600)), "{first}");
    let back = report(&app, &user, ep, 100.0, 0).await;
    assert_eq!((back["applied"].clone(), back["position"].clone()), (json!(true), json!(100)), "{back}");
    assert_eq!(position(&app, &user, ep).await, Some(100));
    let listed = app.episodes(&user, pod).await.into_iter().find(|e| e["Episodeid"] == ep).unwrap();
    assert_eq!(listed["Listenduration"], 100, "what every device resumes from");
    // Back to the very start is a position too.
    report(&app, &user, ep, 0.0, 0).await;
    assert_eq!(position(&app, &user, ep).await, Some(0));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_older_observation_loses_and_is_told_the_stored_state() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    report(&app, &user, ep, 500.0, 0).await;
    // Another device was further on an hour ago and only gets through now.
    let late = report(&app, &user, ep, 900.0, 3600).await;
    assert_eq!(late["applied"], false, "{late}");
    assert_eq!(late["position"], 500, "the reply carries what is stored: {late}");
    assert_eq!(position(&app, &user, ep).await, Some(500));
}

#[tokio::test(flavor = "multi_thread")]
async fn device_clocks_that_are_a_day_off_do_not_change_who_wins() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let (fast, slow) = (CLOCK + DAY_MS, CLOCK - DAY_MS);
    let one = |position: i32, at: i64| json!([{ "episode_id": ep, "position": position, "at": at }]);

    // The device a day ahead reports first. Going by its clock it would win for a day.
    send(&app, &user.api_key, fast, one(300, fast)).await;
    // The device a day behind then reports something it saw a minute before sending: older.
    let older = send(&app, &user.api_key, slow, one(900, slow - 60_000)).await.remove(0);
    assert_eq!(older["applied"], false, "{older}");
    // And then where it is now: newer, whatever its clock says.
    let newer = send(&app, &user.api_key, slow, one(200, slow)).await.remove(0);
    assert_eq!(newer["applied"], true, "{newer}");
    assert_eq!(position(&app, &user, ep).await, Some(200));

    // The stored time is the server's, so it is never in the future.
    let updated_at = newer["updated_at"].as_i64().unwrap();
    let now = chrono::Utc::now().timestamp();
    assert!((now - 120..=now + 5).contains(&updated_at), "updated_at {updated_at} vs now {now}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_offline_batch_is_applied_in_the_order_it_happened() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let other = app.episode_id(&user, app.podcasts(&user).await[0]["podcastid"].as_i64().unwrap() as i32, "Episode 2").await;
    // Sent newest first; the server orders them by when they happened.
    let results = send(
        &app,
        &user.api_key,
        CLOCK,
        json!([
            { "episode_id": ep, "position": 1801.0, "at": CLOCK - 100_000, "listened": 50.0, "finished": true },
            { "episode_id": other, "position": 42.0, "at": CLOCK - 200_000, "listened": 42.0 },
            { "episode_id": ep, "position": 400.0, "at": CLOCK - 900_000, "listened": 95.0 },
        ]),
    )
    .await;
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|r| r["applied"] == true && r["found"] == true), "{results:#?}");

    assert!(completed(&app, &user, ep).await, "the finished flag travels with the positions");
    assert_eq!(position(&app, &user, ep).await, Some(1801));
    assert_eq!(position(&app, &user, other).await, Some(42));
    assert_eq!(history(&app, &user, ep, "ListenedSeconds").await, Some(145));
    // 187 seconds played: three minutes and seven seconds waiting for the next one.
    assert_eq!((stat(&app, &user, "TimeListened").await, stat(&app, &user, "ListenSecondsCarry").await), (3, 7));

    // A position from before the episode was finished arrives afterwards: too old to matter.
    let stale = report(&app, &user, ep, 1500.0, 500).await;
    assert_eq!((stale["applied"].clone(), stale["position"].clone(), stale["completed"].clone()), (json!(false), json!(1801), json!(true)));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_episode_can_be_replayed() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    mark(&app, &user, ep, "/api/data/mark_episode_completed").await;
    assert_eq!(position(&app, &user, ep).await, Some(1801));

    let replay = report(&app, &user, ep, 5.0, 0).await;
    assert_eq!((replay["applied"].clone(), replay["position"].clone()), (json!(true), json!(5)), "{replay}");
    assert_eq!(replay["completed"], true, "it stays finished until the user says otherwise");
    report(&app, &user, ep, 640.0, 0).await;
    assert_eq!(position(&app, &user, ep).await, Some(640), "the replay's progress is kept");
    assert!(completed(&app, &user, ep).await);

    // Finishing it again puts the position back at the end.
    let again = send(&app, &user.api_key, CLOCK, json!([{ "episode_id": ep, "position": 1800.0, "at": CLOCK, "finished": true }])).await;
    assert_eq!(again[0]["position"], 1801, "{again:#?}");
    assert_eq!(position(&app, &user, ep).await, Some(1801));
}

#[tokio::test(flavor = "multi_thread")]
async fn finishing_does_not_undo_a_replay_started_since() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    // The web player restarted the episode just now; the phone's "finished" is ten minutes old.
    report(&app, &user, ep, 30.0, 0).await;
    let late = send(&app, &user.api_key, CLOCK, json!([{ "episode_id": ep, "position": 1801.0, "at": CLOCK - 600_000, "finished": true }])).await;
    assert_eq!((late[0]["applied"].clone(), late[0]["position"].clone(), late[0]["completed"].clone()), (json!(false), json!(30), json!(true)));
    assert!(completed(&app, &user, ep).await, "it was finished");
    assert_eq!(position(&app, &user, ep).await, Some(30), "but the newer position stands");
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_complete_applies_near_the_end() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    app.put("/api/data/update_auto_complete_seconds")
        .key(&user)
        .json(&json!({ "user_id": user.id, "seconds": 60 }))
        .send()
        .await
        .assert_ok();
    let far = report(&app, &user, ep, 1000.0, 0).await;
    assert_eq!(far["completed"], false);
    let near = report(&app, &user, ep, 1760.0, 0).await; // 41s left
    assert_eq!((near["completed"].clone(), near["position"].clone()), (json!(true), json!(1801)), "{near}");
    // Replaying the last minute doesn't snap back to the end again.
    let replay = report(&app, &user, ep, 1770.0, 0).await;
    assert_eq!(replay["position"], 1770, "{replay}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_players_length_is_stored_when_the_feed_gave_none() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    exec(&app, &format!(r#"UPDATE "Episodes" SET EpisodeDuration = 0 WHERE EpisodeID = {ep}"#)).await;
    send(&app, &user.api_key, CLOCK, json!([{ "episode_id": ep, "position": 10.0, "at": CLOCK, "duration": 2400.4 }])).await;
    assert_eq!(int(&app, &format!(r#"SELECT EpisodeDuration FROM "Episodes" WHERE EpisodeID = {ep}"#)).await, Some(2400));
}

// --- listened time ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn listened_time_is_what_was_played_not_where_the_position_is() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let listen = |position: f64, listened: f64, secs_ago: i64| {
        json!([{ "episode_id": ep, "position": position, "at": CLOCK - secs_ago * 1000, "listened": listened }])
    };
    send(&app, &user.api_key, CLOCK, listen(1200.0, 30.0, 0)).await; // skipped ahead, played 30s
    assert_eq!(history(&app, &user, ep, "ListenedSeconds").await, Some(30));
    send(&app, &user.api_key, CLOCK, listen(20.0, 20.0, 0)).await; // went back to the start
    assert_eq!(history(&app, &user, ep, "ListenedSeconds").await, Some(50), "a rewind takes nothing away");

    // Played on a device whose position is too old to win: still played.
    let lost = send(&app, &user.api_key, CLOCK, listen(700.0, 25.0, 3600)).await.remove(0);
    assert_eq!(lost["applied"], false);
    assert_eq!(history(&app, &user, ep, "ListenedSeconds").await, Some(75));
    assert_eq!((stat(&app, &user, "TimeListened").await, stat(&app, &user, "ListenSecondsCarry").await), (1, 15));

    // Marking an episode played is not listening to it.
    let other = app.episode_id(&user, app.podcasts(&user).await[0]["podcastid"].as_i64().unwrap() as i32, "Episode 2").await;
    mark(&app, &user, other, "/api/data/mark_episode_completed").await;
    assert_eq!(history(&app, &user, other, "ListenedSeconds").await, Some(0));
    let home = app.get("/api/data/home_overview").key(&user).query("user_id", user.id).send().await.ok_json();
    assert_eq!(home["weekly_stats"]["seconds_listened"], 75, "{home}");
}

#[tokio::test(flavor = "multi_thread")]
async fn listening_is_logged_per_day_and_client() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let logged = |client: &'static str| {
        let (app, user) = (&app, &user);
        async move {
            int(app, &format!(
                r#"SELECT CAST(COALESCE(SUM(Seconds), 0) AS INTEGER) FROM "ListeningLog" WHERE UserID = {} AND ItemKind = 0 AND ItemID = {ep} AND Client = '{client}'"#,
                user.id
            ))
            .await
            .unwrap_or(0)
        }
    };
    let play = |client: Option<&str>, position: f64, listened: f64| {
        let mut body = json!({ "updates": [{ "episode_id": ep, "position": position, "listened": listened }] });
        if let Some(c) = client {
            body["client"] = json!(c);
        }
        body
    };
    for body in [play(Some("android"), 100.0, 30.0), play(Some("ANDROID"), 150.0, 20.0), play(Some("web"), 200.0, 40.0), play(None, 250.0, 5.0), play(Some("toaster"), 300.0, 5.0)] {
        app.post("/api/data/progress").key(&user).json(&body).send().await.assert_ok();
    }
    assert_eq!((logged("android").await, logged("web").await, logged("").await), (50, 40, 10));

    // A position with no time played logs nothing.
    app.post("/api/data/progress").key(&user).json(&play(Some("web"), 900.0, 0.0)).send().await.assert_ok();
    assert_eq!(logged("web").await, 40);

    // Old apps' forward movement counts, under no client.
    old_report(&app, &user.api_key, &user, ep, 1000.0).await;
    old_report(&app, &user.api_key, &user, ep, 1003.0).await;
    assert_eq!(logged("").await, 13);
    let rows = int(&app, &format!(r#"SELECT COUNT(*) FROM "ListeningLog" WHERE UserID = {} AND ListenHour BETWEEN 0 AND 23"#, user.id)).await;
    assert!(rows.unwrap_or(0) >= 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn time_saved_and_what_was_played_outlive_the_podcast() {
    let app = TestApp::new().await;
    let (user, pod, ep) = setup(&app).await;
    let body = json!({ "client": "android", "updates": [
        { "episode_id": ep, "position": 400.0, "listened": 30.0, "speed_saved": 15.2, "silence_skipped": 2.4, "ads_skipped": 60.0 },
    ]});
    app.post("/api/data/progress").key(&user).json(&body).send().await.assert_ok();
    // Slower than 1x takes time back; a skip with no time played is still logged.
    let body = json!({ "client": "android", "updates": [
        { "episode_id": ep, "position": 500.0, "listened": 10.0, "speed_saved": -5.0 },
        { "episode_id": ep, "position": 590.0, "ads_skipped": 30.0 },
    ]});
    app.post("/api/data/progress").key(&user).json(&body).send().await.assert_ok();

    let sum = |column: &'static str| {
        let (app, user) = (&app, &user);
        async move { int(app, &format!(r#"SELECT CAST(SUM({column}) AS INTEGER) FROM "ListeningLog" WHERE UserID = {}"#, user.id)).await }
    };
    assert_eq!((sum("Seconds").await, sum("SpeedSaved").await, sum("SilenceSkipped").await, sum("AdsSkipped").await), (Some(40), Some(10), Some(2), Some(90)));

    let stats = app.get("/api/data/get_extended_stats").key(&user).query("user_id", user.id).send().await.ok_json();
    assert_eq!(stats["time_saved"], json!({ "speed": 10, "silence": 2, "ads": 90 }), "{stats}");
    let days = stats["daily_listening"].as_array().cloned().unwrap_or_default();
    assert_eq!(days.iter().map(|d| d["seconds"].as_i64().unwrap()).sum::<i64>(), 40, "the calendar reads the log: {days:?}");

    app.post("/api/data/remove_podcast_id").key(&user).json(&json!({ "podcast_id": pod, "user_id": user.id })).send().await.assert_ok();
    let item = |column: &str| format!(r#"SELECT {column} FROM "ListeningLogItems" WHERE UserID = {} AND ItemKind = 0 AND ItemID = {ep}"#, user.id);
    let item = (text(&app, &item("PodcastName")).await, text(&app, &item("ItemTitle")).await);
    assert_eq!(item, (Some("Progress Show".into()), Some("Episode 1".into())), "the log still says what was played");
    assert_eq!(sum("Seconds").await, Some(40));
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrading_fills_in_what_earlier_rows_played() {
    let app = TestApp::new().await;
    if app.upgrade_tests_unsupported() {
        return;
    }
    let (user, _pod, ep) = setup(&app).await;
    app.post("/api/data/progress").key(&user).json(&json!({ "updates": [{ "episode_id": ep, "position": 60.0, "listened": 60.0 }] })).send().await.assert_ok();
    // As logged before the items were kept.
    exec(&app, r#"DROP TABLE "ListeningLogItems""#).await;
    rerun_migrations(&app, &["094"]).await;
    let name = text(&app, &format!(r#"SELECT PodcastName FROM "ListeningLogItems" WHERE UserID = {} AND ItemID = {ep}"#, user.id)).await;
    assert_eq!(name.as_deref(), Some("Progress Show"));
}

// --- the request itself ----------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn episodes_that_are_not_the_users_are_reported_and_skipped() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let stranger = app.user("stranger").await;
    let url = app.feed("theirs.xml", &Feed::with_episodes("Their Show", 1));
    let pod = app.subscribe(&stranger, &url, 1).await;
    let theirs = app.episode_id(&stranger, pod, "Episode 1").await;

    let results = send(
        &app,
        &user.api_key,
        CLOCK,
        json!([
            { "episode_id": theirs, "position": 50.0, "at": CLOCK, "listened": 50.0 },
            { "episode_id": 987_654, "position": 50.0, "at": CLOCK },
            { "episode_id": ep, "position": 50.0, "at": CLOCK },
        ]),
    )
    .await;
    let found: Vec<bool> = results.iter().map(|r| r["found"].as_bool().unwrap()).collect();
    assert_eq!(found, [false, false, true], "{results:#?}");
    assert_eq!(position(&app, &user, ep).await, Some(50), "the rest of the batch is still applied");
    assert_eq!(position(&app, &stranger, theirs).await, None);
    assert_eq!(history(&app, &user, theirs, "ListenDuration").await, None);
    assert_eq!(stat(&app, &user, "ListenSecondsCarry").await, 0, "nothing is credited for an episode that isn't theirs");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_request_is_authenticated_and_bounded() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let body = json!({ "sent_at": CLOCK, "updates": [{ "episode_id": ep, "position": 10.0, "at": CLOCK }] });
    app.post("/api/data/progress").json(&body).send().await.assert_status(401);
    app.post("/api/data/progress").header("Api-Key", "not-a-key").json(&body).send().await.assert_status(401);
    assert_eq!(position(&app, &user, ep).await, None);

    let many: Vec<Value> = (0..501).map(|_| json!({ "episode_id": ep, "position": 1.0 })).collect();
    app.post("/api/data/progress").key(&user).json(&json!({ "updates": many })).send().await.assert_status(400);

    // No clock at all is fine: the update counts as now.
    let bare = app.post("/api/data/progress").key(&user).json(&json!({ "updates": [{ "episode_id": ep, "position": 12.0 }] })).send().await.ok_json();
    assert_eq!(bare["results"][0]["applied"], true, "{bare}");
}

// --- apps that predate /progress -------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn old_app_a_paused_player_repeating_itself_changes_nothing() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let old = app.login(&user.username, &user.password).await;
    old_report(&app, &old, &user, ep, 200.0).await;
    assert_eq!(position(&app, &user, ep).await, Some(200));
    // The user picks the episode up on a current app and goes back.
    report(&app, &user, ep, 50.0, 0).await;
    // The old app, paused at 200, keeps posting it.
    for _ in 0..3 {
        old_report(&app, &old, &user, ep, 200.0).await;
    }
    assert_eq!(position(&app, &user, ep).await, Some(50));
}

#[tokio::test(flavor = "multi_thread")]
async fn old_app_progress_moves_forward_and_its_own_rewind_is_kept() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let old = app.login(&user.username, &user.password).await;
    old_report(&app, &old, &user, ep, 100.0).await;
    old_report(&app, &old, &user, ep, 130.0).await;
    assert_eq!(position(&app, &user, ep).await, Some(130));
    // It was the last to write, so a lower position is the user going back on that app.
    old_report(&app, &old, &user, ep, 40.0).await;
    assert_eq!(position(&app, &user, ep).await, Some(40));
    old_report(&app, &old, &user, ep, 55.0).await;
    assert_eq!(position(&app, &user, ep).await, Some(55));
}

#[tokio::test(flavor = "multi_thread")]
async fn old_app_cannot_take_back_a_position_another_device_wrote() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let old = app.login(&user.username, &user.password).await;
    old_report(&app, &old, &user, ep, 200.0).await;
    report(&app, &user, ep, 500.0, 0).await; // another device since
    // Behind what is stored, and not its own position any more: the old rule, forward only.
    old_report(&app, &old, &user, ep, 215.0).await;
    old_report(&app, &old, &user, ep, 230.0).await;
    assert_eq!(position(&app, &user, ep).await, Some(500));
    // Once it passes the stored position it counts again.
    old_report(&app, &old, &user, ep, 520.0).await;
    assert_eq!(position(&app, &user, ep).await, Some(520));
}

#[tokio::test(flavor = "multi_thread")]
async fn old_app_flushing_a_stale_position_does_not_rewind_newer_progress() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    report(&app, &user, ep, 500.0, 0).await;
    // An old app comes back online and sends the one position its outbox kept. The server has
    // never heard from it, so there's nothing to say this is its own position.
    let old = app.login(&user.username, &user.password).await;
    old_report(&app, &old, &user, ep, 100.0).await;
    assert_eq!(position(&app, &user, ep).await, Some(500));
}

#[tokio::test(flavor = "multi_thread")]
async fn old_app_minutes_count_only_while_its_position_moves() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let old = app.login(&user.username, &user.password).await;
    // Paused: the app still asks for a minute every minute.
    old_minute(&app, &old, &user).await;
    old_report(&app, &old, &user, ep, 100.0).await;
    old_report(&app, &old, &user, ep, 100.0).await;
    old_minute(&app, &old, &user).await;
    assert_eq!(stat(&app, &user, "TimeListened").await, 0, "nothing is playing");

    old_report(&app, &old, &user, ep, 115.0).await;
    old_minute(&app, &old, &user).await;
    assert_eq!(stat(&app, &user, "TimeListened").await, 1);
    // Time played on the episode comes from the movement, capped by the time that passed.
    let listened = history(&app, &user, ep, "ListenedSeconds").await.unwrap();
    assert!((1..=10).contains(&listened), "15s of position in a moment credits at most the moment: {listened}");
}

#[tokio::test(flavor = "multi_thread")]
async fn old_app_history_at_play_start_does_not_overwrite_progress() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    let old = app.login(&user.username, &user.password).await;
    let start = |position: f64| {
        app.post("/api/data/record_podcast_history")
            .header("Api-Key", &old)
            .json(&json!({ "episode_id": ep, "user_id": user.id, "episode_pos": position, "is_youtube": false }))
    };
    // A first play puts the episode in the history.
    start(0.0).send().await.assert_ok();
    assert_eq!(position(&app, &user, ep).await, Some(0));
    report(&app, &user, ep, 500.0, 0).await;
    // The old web player starts the episode from a list it loaded before that.
    start(120.0).send().await.assert_ok();
    assert_eq!(position(&app, &user, ep).await, Some(500));
    // Nor does it take back the app's own progress: starting again isn't a rewind.
    old_report(&app, &old, &user, ep, 520.0).await;
    old_report(&app, &old, &user, ep, 535.0).await;
    start(0.0).send().await.assert_ok();
    assert_eq!(position(&app, &user, ep).await, Some(535));
}

#[tokio::test(flavor = "multi_thread")]
async fn old_app_reports_for_someone_elses_episode_are_refused() {
    let app = TestApp::new().await;
    let (user, _pod, _ep) = setup(&app).await;
    let stranger = app.user("stranger").await;
    let url = app.feed("theirs.xml", &Feed::with_episodes("Their Show", 1));
    let pod = app.subscribe(&stranger, &url, 1).await;
    let theirs = app.episode_id(&stranger, pod, "Episode 1").await;
    app.post("/api/data/record_listen_duration")
        .key(&user)
        .json(&json!({ "episode_id": theirs, "user_id": user.id, "listen_duration": 30.0 }))
        .send()
        .await
        .assert_status(404);
    app.post("/api/data/record_listen_duration")
        .key(&user)
        .json(&json!({ "episode_id": theirs, "user_id": stranger.id, "listen_duration": 30.0 }))
        .send()
        .await
        .assert_status(403);
    assert_eq!(position(&app, &stranger, theirs).await, None);
}
