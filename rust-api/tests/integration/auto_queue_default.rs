//! The per-user auto-queue default (#648): with it on, a new subscription starts with AutoQueue
//! on, so its new episodes are queued on refresh. Its back catalogue is not queued, and podcasts
//! the user already had keep their own setting.

use crate::harness::*;

async fn set_default(app: &TestApp, user: &TestUser, enabled: bool) {
    let body = app
        .put("/api/data/update_auto_queue_new_podcasts")
        .key(user)
        .json(&json!({ "user_id": user.id, "enabled": enabled }))
        .send()
        .await
        .ok_json();
    assert_eq!(body["enabled"], enabled, "{body}");
}

async fn default_of(app: &TestApp, user: &TestUser) -> bool {
    let body = app.get(&format!("/api/data/get_auto_queue_new_podcasts/{}", user.id)).key(user).send().await.ok_json();
    body["enabled"].as_bool().unwrap_or_else(|| panic!("{body}"))
}

async fn auto_queue(app: &TestApp, user: &TestUser, podcast_id: i32) -> bool {
    let body = app
        .post("/api/data/get_auto_queue_status")
        .key(user)
        .json(&json!({ "podcast_id": podcast_id, "user_id": user.id }))
        .send()
        .await
        .ok_json();
    body["auto_queue"].as_bool().unwrap_or_else(|| panic!("{body}"))
}

async fn queue_len(app: &TestApp, user: &TestUser) -> i64 {
    int(app, &format!(r#"SELECT COUNT(*) FROM "EpisodeQueue" WHERE UserID = {}"#, user.id)).await.unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn new_subscriptions_are_not_auto_queued_by_default() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    assert!(!default_of(&app, &user).await);

    let pod = app.subscribe(&user, &app.feed("show.xml", &Feed::with_episodes("Show", 2)), 2).await;
    assert!(!auto_queue(&app, &user, pod).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn with_the_default_on_a_new_subscription_queues_only_episodes_that_arrive_later() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let earlier = app.subscribe(&user, &app.feed("earlier.xml", &Feed::with_episodes("Earlier", 1)), 1).await;
    set_default(&app, &user, true).await;
    assert!(default_of(&app, &user).await);

    let feed = Feed::with_episodes("Show", 3);
    let pod = app.subscribe(&user, &app.feed("show.xml", &feed), 3).await;
    assert!(auto_queue(&app, &user, pod).await, "the new subscription starts with auto-queue on");
    assert!(!auto_queue(&app, &user, earlier).await, "a podcast the user already had keeps its setting");
    assert_eq!(queue_len(&app, &user).await, 0, "the back catalogue is not queued");

    app.feed("show.xml", &feed.episode(Episode::new(4)));
    set_next_refresh(&app, pod, "2000-01-01 00:00:00").await;
    assert_eq!(pinepods_api::handlers::refresh::refresh_due_podcasts(&app.state).await.unwrap(), 1);
    let new = app.episode_id(&user, pod, "Episode 4").await;
    let queued = int(&app, &format!(r#"SELECT COUNT(*) FROM "EpisodeQueue" WHERE UserID = {} AND EpisodeID = {new}"#, user.id)).await;
    assert_eq!(queued, Some(1), "the episode that arrived after subscribing is queued");
    assert_eq!(queue_len(&app, &user).await, 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn opml_imports_follow_the_default_too() {
    let app = TestApp::new().await;
    let user = app.user("importer").await;
    set_default(&app, &user, true).await;
    let url = app.feed("imported.xml", &Feed::with_episodes("Imported", 1));
    let body = app
        .post("/api/data/import_opml")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcasts": [url] }))
        .send()
        .await
        .ok_json();
    assert_eq!(body["success"], true, "{body}");
    let pod = eventually("the imported podcast", || async {
        app.podcasts(&user).await.first().and_then(|p| p["podcastid"].as_i64()).map(|id| id as i32)
    })
    .await;
    assert!(auto_queue(&app, &user, pod).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn turning_the_default_off_again_stops_applying_it() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    set_default(&app, &user, true).await;
    set_default(&app, &user, false).await;
    assert!(!default_of(&app, &user).await);
    let pod = app.subscribe(&user, &app.feed("show.xml", &Feed::with_episodes("Show", 1)), 1).await;
    assert!(!auto_queue(&app, &user, pod).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_auto_queue_default_is_private() {
    let app = TestApp::new().await;
    let (user, other) = (app.user("owner").await, app.user("snoop").await);
    app.get(&format!("/api/data/get_auto_queue_new_podcasts/{}", user.id)).key(&other).send().await.assert_status(403);
    app.put("/api/data/update_auto_queue_new_podcasts")
        .key(&other)
        .json(&json!({ "user_id": user.id, "enabled": true }))
        .send()
        .await
        .assert_status(403);
    assert!(!default_of(&app, &user).await);
}
