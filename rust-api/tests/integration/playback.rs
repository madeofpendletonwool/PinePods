//! Listening: progress, history, completion, per-podcast playback settings, stats.

use crate::harness::*;

async fn setup(app: &TestApp) -> (TestUser, i32, i32) {
    let user = app.user("listener").await;
    let url = app.feed("play.xml", &Feed::with_episodes("Play Show", 2));
    let pod = app.subscribe(&user, &url, 2).await;
    let ep = app.episode_id(&user, pod, "Episode 1").await; // duration 1801s
    (user, pod, ep)
}

async fn record(app: &TestApp, user: &TestUser, ep: i32, secs: f64) {
    app.post("/api/data/record_listen_duration")
        .key(user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "listen_duration": secs }))
        .send()
        .await
        .assert_ok();
}

async fn episode(app: &TestApp, user: &TestUser, pod: i32, ep: i32) -> Value {
    app.episodes(user, pod).await.into_iter().find(|e| e["Episodeid"] == ep).expect("episode")
}

async fn history(app: &TestApp, user: &TestUser) -> Vec<Value> {
    let body = app.get(&format!("/api/data/user_history/{}", user.id)).key(user).send().await.ok_json();
    body["data"].as_array().cloned().unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn listen_progress_is_saved_and_shows_everywhere() {
    let app = TestApp::new().await;
    let (user, pod, ep) = setup(&app).await;
    record(&app, &user, ep, 120.0).await;
    record(&app, &user, ep, 245.0).await; // later progress overwrites

    assert_eq!(episode(&app, &user, pod, ep).await["Listenduration"], 245);
    let h = history(&app, &user).await;
    assert_eq!(h.len(), 1, "{h:#?}");
    assert_eq!(h[0]["episodeid"], ep);
    assert_eq!(h[0]["listenduration"], 245);

    let home = app.get("/api/data/home_overview").key(&user).query("user_id", user.id).send().await.ok_json();
    let in_progress = home["in_progress_episodes"].as_array().unwrap();
    assert!(in_progress.iter().any(|e| e["episodeid"] == ep), "home in-progress: {home}");
}

#[tokio::test(flavor = "multi_thread")]
async fn history_records_positions_in_seconds() {
    let app = TestApp::new().await;
    let (user, _pod, ep) = setup(&app).await;
    app.post("/api/data/record_podcast_history")
        .key(&user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "episode_pos": 130.0, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(history(&app, &user).await[0]["listenduration"], 130);
}

#[tokio::test(flavor = "multi_thread")]
async fn progress_is_per_user() {
    let app = TestApp::new().await;
    let (alice, pod_a, ep_a) = setup(&app).await;
    let bob = app.user("bob").await;
    let url = app.fixtures().url("play.xml");
    let pod_b = app.subscribe(&bob, &url, 2).await;
    let ep_b = app.episode_id(&bob, pod_b, "Episode 1").await;

    record(&app, &alice, ep_a, 500.0).await;
    assert_eq!(episode(&app, &alice, pod_a, ep_a).await["Listenduration"], 500);
    assert!(episode(&app, &bob, pod_b, ep_b).await["Listenduration"].is_null());
    assert!(history(&app, &bob).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn mark_completed_and_uncompleted_round_trip() {
    let app = TestApp::new().await;
    let (user, pod, ep) = setup(&app).await;
    let mark = |path: &'static str| {
        app.post(path).key(&user).json(&json!({ "episode_id": ep, "user_id": user.id, "is_youtube": false }))
    };
    mark("/api/data/mark_episode_completed").send().await.assert_ok();
    assert_eq!(episode(&app, &user, pod, ep).await["Completed"], true);
    mark("/api/data/mark_episode_uncompleted").send().await.assert_ok();
    assert_eq!(episode(&app, &user, pod, ep).await["Completed"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_complete_threshold_marks_episodes_done_near_the_end() {
    let app = TestApp::new().await;
    let (user, pod, ep) = setup(&app).await;
    app.put("/api/data/update_auto_complete_seconds")
        .key(&user)
        .json(&json!({ "user_id": user.id, "seconds": 60 }))
        .send()
        .await
        .assert_ok();
    let got = app.get(&format!("/api/data/get_auto_complete_seconds/{}", user.id)).key(&user).send().await.ok_json();
    assert_eq!(got["auto_complete_seconds"], 60);

    record(&app, &user, ep, 1000.0).await;
    assert_eq!(episode(&app, &user, pod, ep).await["Completed"], false, "far from the end");
    record(&app, &user, ep, 1760.0).await; // 41s left of 1801
    assert_eq!(episode(&app, &user, pod, ep).await["Completed"], true, "within the threshold");
}

#[tokio::test(flavor = "multi_thread")]
async fn per_podcast_speed_and_skips_override_the_user_default() {
    let app = TestApp::new().await;
    let (user, pod, _ep) = setup(&app).await;
    let details = || async {
        app.post("/api/data/get_play_episode_details")
            .key(&user)
            .json(&json!({ "user_id": user.id, "podcast_id": pod }))
            .send()
            .await
            .ok_json()
    };
    let d = details().await;
    assert_eq!(d["playback_speed_customized"], false, "{d}");
    assert_eq!(d["start_skip"], 0);

    app.post("/api/data/user/set_playback_speed")
        .key(&user)
        .json(&json!({ "user_id": user.id, "playback_speed": 1.2 }))
        .send()
        .await
        .assert_ok();
    assert_eq!(details().await["playback_speed"], 1.2, "user default applies (speeds are stored to one decimal)");

    app.post("/api/data/podcast/set_playback_speed")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod, "playback_speed": 1.7 }))
        .send()
        .await
        .assert_ok();
    app.post("/api/data/adjust_skip_times")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod, "start_skip": 30, "end_skip": 15 }))
        .send()
        .await
        .assert_ok();
    let d = details().await;
    assert_eq!(d["playback_speed"], 1.7);
    assert_eq!(d["playback_speed_customized"], true);
    assert_eq!(d["start_skip"], 30);
    assert_eq!(d["end_skip"], 15);

    app.post("/api/data/clear_podcast_playback_speed")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod }))
        .send()
        .await
        .assert_ok();
    let d = details().await;
    assert_eq!(d["playback_speed"], 1.2, "back to the user default: {d}");
    assert_eq!(d["playback_speed_customized"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn every_user_has_stats_that_count_activity() {
    let app = TestApp::new().await;
    let (user, _pod, _ep) = setup(&app).await;
    let stats = || async { app.get("/api/data/get_stats").key(&user).query("user_id", user.id).send().await.ok_json() };
    let before = stats().await;
    assert_eq!(before["PodcastsAdded"], 1, "{before}");

    app.put(&format!("/api/data/increment_played/{}", user.id)).key(&user).send().await.assert_ok();
    let after = stats().await;
    assert_eq!(
        after["PodcastsPlayed"].as_i64().unwrap(),
        before["PodcastsPlayed"].as_i64().unwrap() + 1
    );
}
