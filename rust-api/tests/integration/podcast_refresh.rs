//! Refreshing one podcast now and turning its automatic refreshes off, for feeds and local
//! folders alike.

use crate::harness::*;

/// Make every feed due for the scheduled refresh.
async fn all_due(app: &TestApp) {
    exec(app, r#"UPDATE "Feeds" SET NextRefreshAt = NULL"#).await;
}

async fn scheduled_refresh(app: &TestApp) {
    all_due(app).await;
    pinepods_api::handlers::refresh::refresh_due_podcasts(&app.state).await.expect("scheduled refresh");
}

async fn refresh_all(app: &TestApp, user: &TestUser) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1000);
    pinepods_api::handlers::refresh::run_refresh_process(user.id, false, tx, app.state.clone())
        .await
        .expect("refresh all");
    while rx.recv().await.is_some() {}
}

async fn refresh_now(app: &TestApp, user: &TestUser, pod: i32) -> Value {
    app.post(&format!("/api/data/podcast/{pod}/refresh")).key(user).send().await.ok_json()
}

async fn set_auto_refresh(app: &TestApp, user: &TestUser, pod: i32, on: bool) {
    app.put(&format!("/api/data/podcast/{pod}/auto_refresh"))
        .key(user)
        .json(&json!({ "auto_refresh": on }))
        .send()
        .await
        .ok_json();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_podcast_with_automatic_refresh_off_is_only_refreshed_on_request() {
    let app = TestApp::new().await;
    let user = app.user("pauser").await;
    let feed = Feed::with_episodes("Paused", 1);
    let url = app.feed("paused.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    let status = |app: &TestApp, user: &TestUser| {
        let req = app.get(&format!("/api/data/podcast/{pod}/auto_refresh")).key(user);
        async move { req.send().await.ok_json()["auto_refresh"].clone() }
    };
    assert_eq!(status(&app, &user).await, true, "on by default");
    set_auto_refresh(&app, &user, pod, false).await;
    assert_eq!(status(&app, &user).await, false);

    // A new episode: neither the schedule, nor refresh all, nor the admin refresh fetch it.
    app.feed("paused.xml", &feed.clone().episode(Episode::new(2)));
    let hits = app.fixtures().hits("paused.xml").len();
    scheduled_refresh(&app).await;
    refresh_all(&app, &user).await;
    app.refresh().await;
    assert_eq!(app.fixtures().hits("paused.xml").len(), hits, "not fetched while off");
    assert_eq!(app.episodes(&user, pod).await.len(), 1);

    // Asked for it: refreshed, even straight after another refresh.
    let result = refresh_now(&app, &user, pod).await;
    assert_eq!(result["status"], "refreshed", "{result}");
    assert_eq!(result["new_episodes"].as_array().map(Vec::len), Some(1), "{result}");
    assert_eq!(result["new_episodes"][0]["episodetitle"], "Episode 2");
    let again = refresh_now(&app, &user, pod).await;
    assert_eq!((again["status"].as_str(), again["new_episodes"].as_array().map(Vec::len)), (Some("refreshed"), Some(0)));
    assert_eq!(app.fixtures().hits("paused.xml").len(), hits + 2, "a single refresh always looks");

    // A dead feed that's off stays out of the feed problems list.
    app.fixtures().serve("paused.xml", Fixture::new("text/plain", "gone").status(404));
    let failed = refresh_now(&app, &user, pod).await;
    assert_eq!(failed["status"], "failed", "{failed}");
    assert!(failed["detail"].as_str().is_some_and(|d| d.contains("404")), "{failed}");
    let problems = app.get("/api/data/feed_problems").key(&user).send().await.ok_json();
    assert_eq!(problems["feeds"], json!([]), "{problems}");
    set_auto_refresh(&app, &user, pod, true).await;
    let problems = app.get("/api/data/feed_problems").key(&user).send().await.ok_json();
    assert_eq!(problems["feeds"][0]["podcast_id"].as_i64(), Some(pod as i64), "listed again once on: {problems}");

    // Only the owner.
    let other = app.user("pauser-other").await;
    app.post(&format!("/api/data/podcast/{pod}/refresh")).key(&other).send().await.assert_status(404);
    app.put(&format!("/api/data/podcast/{pod}/auto_refresh"))
        .key(&other)
        .json(&json!({ "auto_refresh": false }))
        .send()
        .await
        .assert_status(404);
    app.get(&format!("/api/data/podcast/{pod}/auto_refresh")).key(&other).send().await.assert_status(404);
}

/// A folder for a local podcast, outside the local-media mount (the scan reads any path).
struct LocalFolder(std::path::PathBuf);

impl LocalFolder {
    fn new() -> LocalFolder {
        let dir = std::env::temp_dir().join(format!("pinepods-local-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        LocalFolder(dir)
    }

    fn add(&self, name: &str) {
        std::fs::write(self.0.join(name), b"not really audio").unwrap();
    }
}

impl Drop for LocalFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn add_local_podcast(app: &TestApp, user: &TestUser, folder: &LocalFolder) -> i32 {
    let db = app.db();
    let feed_url = format!("local://{}", folder.0.display());
    let pod = db.add_local_podcast("My Recordings", &feed_url, "", "", "", false, user.id).await.unwrap();
    let candidates = pinepods_api::handlers::local_podcast::scan_local_directory(&folder.0, &Default::default()).unwrap();
    db.add_local_episodes(pod, user.id, &candidates).await.unwrap();
    pod
}

#[tokio::test(flavor = "multi_thread")]
async fn a_local_folder_is_checked_for_new_files_on_schedule_and_on_request() {
    let app = TestApp::new().await;
    let user = app.user("recorder").await;
    let folder = LocalFolder::new();
    folder.add("01 first.mp3");
    folder.add("notes.txt");
    let pod = add_local_podcast(&app, &user, &folder).await;
    assert_eq!(app.episodes(&user, pod).await.len(), 1);

    // On the schedule, without importing a file twice.
    folder.add("02 second.mp3");
    scheduled_refresh(&app).await;
    scheduled_refresh(&app).await;
    assert_eq!(app.episodes(&user, pod).await.len(), 2);
    let next = int(&app, &format!(r#"SELECT RefreshIntervalMinutes FROM "Podcasts" WHERE PodcastID = {pod}"#)).await;
    assert_eq!(next, Some(15), "looked at again in 15 minutes");

    // Refresh all and a single refresh.
    folder.add("03 third.mp3");
    refresh_all(&app, &user).await;
    assert_eq!(app.episodes(&user, pod).await.len(), 3);
    folder.add("04 fourth.ogg");
    let result = refresh_now(&app, &user, pod).await;
    assert_eq!(result["status"], "refreshed", "{result}");
    assert_eq!(result["new_episodes"][0]["episodetitle"], "04 fourth", "{result}");

    // Off: only a refresh on request picks up new files.
    set_auto_refresh(&app, &user, pod, false).await;
    folder.add("05 fifth.mp3");
    scheduled_refresh(&app).await;
    refresh_all(&app, &user).await;
    assert_eq!(app.episodes(&user, pod).await.len(), 4);
    assert_eq!(refresh_now(&app, &user, pod).await["new_episodes"].as_array().map(Vec::len), Some(1));
    set_auto_refresh(&app, &user, pod, true).await;

    // The folder is gone: a feed problem, like a feed that's gone.
    std::fs::remove_dir_all(&folder.0).unwrap();
    let failed = refresh_now(&app, &user, pod).await;
    assert_eq!(failed["status"], "failed", "{failed}");
    let problems = app.get("/api/data/feed_problems").key(&user).send().await.ok_json();
    assert_eq!(problems["feeds"][0]["kind"], "gone", "{problems}");
}
