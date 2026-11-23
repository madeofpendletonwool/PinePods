//! Episodes of podcasts the user hasn't subscribed to: resolving one gives a real episode that can
//! be played, queued and saved, under a podcast row that stays out of the library, sync and
//! refreshes. Subscribing later keeps that row and everything done with it.

use pinepods_api::services::sync;

use crate::harness::*;
use crate::harness::http::TestResponse;

const EP2_URL: &str = "https://cdn.example.invalid/audio/ep2.mp3";

async fn resolve(app: &TestApp, user: &TestUser, feed_url: &str, episode_url: &str, title: &str) -> TestResponse {
    app.post("/api/data/resolve_feed_episode")
        .key(user)
        .json(&json!({ "user_id": user.id, "feed_url": feed_url, "episode_url": episode_url, "episode_title": title }))
        .send()
        .await
}

/// (episode id, podcast id) of "Episode 2".
async fn resolve_ep2(app: &TestApp, user: &TestUser, feed_url: &str) -> (i32, i32) {
    let body = resolve(app, user, feed_url, EP2_URL, "Episode 2").await.ok_json();
    assert_eq!(body["subscribed"], false, "{body}");
    (body["episode_id"].as_i64().unwrap() as i32, body["podcast_id"].as_i64().unwrap() as i32)
}

async fn check_podcast(app: &TestApp, user: &TestUser, feed_url: &str) -> bool {
    let name = text(app, &format!(r#"SELECT PodcastName FROM "Podcasts" WHERE FeedURL = '{feed_url}' AND UserID = {}"#, user.id))
        .await
        .unwrap_or_default();
    app.get("/api/data/check_podcast")
        .key(user)
        .query("user_id", user.id)
        .query("podcast_name", name)
        .query("podcast_url", feed_url)
        .send()
        .await
        .ok_json()["exists"]
        .as_bool()
        .unwrap()
}

async fn queue(app: &TestApp, user: &TestUser, episode: i32) {
    app.post("/api/data/queue_pod")
        .key(user)
        .json(&json!({ "episode_id": episode, "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
}

async fn queued(app: &TestApp, user: &TestUser) -> Vec<i64> {
    let body = app.get("/api/data/get_queued_episodes").key(user).query("user_id", user.id).send().await.ok_json();
    body["data"].as_array().unwrap().iter().map(|e| e["episodeid"].as_i64().unwrap()).collect()
}

async fn recent_episodes(app: &TestApp, user: &TestUser) -> usize {
    let body = app.get(&format!("/api/data/return_episodes/{}", user.id)).key(user).send().await.ok_json();
    body["episodes"].as_array().unwrap().len()
}

async fn subscription_log(app: &TestApp, user: &TestUser) -> Vec<String> {
    let n = int(app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncSubscriptions" WHERE UserID = {}"#, user.id)).await.unwrap();
    let mut out = Vec::new();
    for i in 0..n {
        let sql = format!(
            r#"SELECT Action FROM "GpodderSyncSubscriptions" WHERE UserID = {} ORDER BY SubscriptionID LIMIT 1 OFFSET {i}"#,
            user.id
        );
        out.push(text(app, &sql).await.unwrap());
    }
    out
}

async fn podcasts_added(app: &TestApp, user: &TestUser) -> i64 {
    int(app, &format!(r#"SELECT PodcastsAdded FROM "UserStats" WHERE UserID = {}"#, user.id)).await.unwrap_or(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_episode_can_be_used_without_subscribing() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let url = app.feed("unsub.xml", &Feed::with_episodes("Unsubscribed Show", 3));

    let (ep, pod) = resolve_ep2(&app, &user, &url).await;
    assert_eq!(app.episode_id(&user, pod, "Episode 2").await, ep);

    // Resolving again reuses the row and doesn't fetch the feed again.
    let fetches = app.fixtures().hits("unsub.xml").len();
    assert_eq!(resolve_ep2(&app, &user, &url).await, (ep, pod));
    assert_eq!(app.fixtures().hits("unsub.xml").len(), fetches);

    // Usable like any episode.
    queue(&app, &user, ep).await;
    assert_eq!(queued(&app, &user).await, [ep as i64]);
    app.post("/api/data/record_listen_duration")
        .key(&user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "listen_duration": 120.0 }))
        .send()
        .await
        .assert_ok();
    let history = format!(r#"SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = {} AND EpisodeID = {ep}"#, user.id);
    assert_eq!(int(&app, &history).await, Some(120));

    // But it isn't a subscription.
    assert!(app.podcasts(&user).await.is_empty(), "not in the library");
    assert!(!check_podcast(&app, &user, &url).await, "reported as not subscribed");
    assert_eq!(recent_episodes(&app, &user).await, 0, "its episodes stay out of the feed");
    assert_eq!(podcasts_added(&app, &user).await, 0);
    let details = app.get("/api/data/get_podcast_details").key(&user).query("user_id", user.id).query("podcast_id", pod).send().await.ok_json();
    assert_eq!(details["details"]["subscribed"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribing_later_keeps_the_row_and_what_was_done_with_it() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    exec(&app, &format!(r#"UPDATE "Users" SET GpodderServerEnabled = TRUE WHERE UserID = {}"#, user.id)).await;
    sync::invalidate_sync_enabled(user.id);
    let url = app.feed("later.xml", &Feed::with_episodes("Later Show", 3));

    let (ep, pod) = resolve_ep2(&app, &user, &url).await;
    queue(&app, &user, ep).await;
    assert!(subscription_log(&app, &user).await.is_empty(), "not synced as a subscription");

    assert_eq!(app.subscribe(&user, &url, 3).await, pod, "the same row");
    assert_eq!(app.episode_id(&user, pod, "Episode 2").await, ep, "the same episode");
    assert_eq!(queued(&app, &user).await, [ep as i64], "still queued");
    assert!(check_podcast(&app, &user, &url).await);
    assert_eq!(app.podcasts(&user).await.len(), 1);
    assert_eq!(recent_episodes(&app, &user).await, 3);
    assert_eq!(podcasts_added(&app, &user).await, 1);
    assert_eq!(subscription_log(&app, &user).await, ["add"]);

    // Resolving an episode of a subscribed podcast just finds it.
    let body = resolve(&app, &user, &url, EP2_URL, "Episode 2").await.ok_json();
    assert_eq!((body["episode_id"].as_i64(), body["subscribed"].as_bool()), (Some(ep as i64), Some(true)));
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_an_unsubscribed_row_is_not_an_unsubscribe() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    exec(&app, &format!(r#"UPDATE "Users" SET GpodderServerEnabled = TRUE WHERE UserID = {}"#, user.id)).await;
    sync::invalidate_sync_enabled(user.id);
    let url = app.feed("gone.xml", &Feed::with_episodes("Gone Show", 2));
    let (_, pod) = resolve_ep2(&app, &user, &url).await;

    app.post("/api/data/remove_podcast_id")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod }))
        .send()
        .await
        .assert_ok();
    assert!(subscription_log(&app, &user).await.is_empty());
    assert_eq!(podcasts_added(&app, &user).await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn feeds_with_only_unsubscribed_rows_are_not_refreshed() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let url = app.feed("norefresh.xml", &Feed::with_episodes("No Refresh", 2));
    let (_, pod) = resolve_ep2(&app, &user, &url).await;
    exec(&app, r#"UPDATE "Feeds" SET NextRefreshAt = '2000-01-01 00:00:00'"#).await;
    set_next_refresh(&app, pod, "2000-01-01 00:00:00").await;

    let fetches = app.fixtures().hits("norefresh.xml").len();
    assert_eq!(pinepods_api::handlers::refresh::refresh_due_podcasts(&app.state).await.unwrap(), 0);
    app.refresh().await;
    assert_eq!(app.fixtures().hits("norefresh.xml").len(), fetches);
}

#[tokio::test(flavor = "multi_thread")]
async fn unused_rows_are_cleaned_up_and_used_ones_kept() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let idle_url = app.feed("idle.xml", &Feed::with_episodes("Idle", 2));
    let used_url = app.feed("used.xml", &Feed::with_episodes("Used", 2));
    let (_, idle) = resolve_ep2(&app, &user, &idle_url).await;
    let (ep, used) = resolve_ep2(&app, &user, &used_url).await;
    app.post("/api/data/save_episode")
        .key(&user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();

    // Recently used rows stay even with nothing attached.
    assert_eq!(app.db().cleanup_unused_unsubscribed_podcasts().await.unwrap(), 0);

    exec(&app, r#"UPDATE "Podcasts" SET UnsubscribedSince = '2000-01-01 00:00:00'"#).await;
    assert_eq!(app.db().cleanup_unused_unsubscribed_podcasts().await.unwrap(), 1);
    let left = |pod: i32| format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE PodcastID = {pod}"#);
    assert_eq!(int(&app, &left(idle)).await, Some(0), "nothing used it");
    assert_eq!(int(&app, &left(used)).await, Some(1), "a saved episode keeps it");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unknown_episode_is_not_found() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let url = app.feed("missing.xml", &Feed::with_episodes("Missing", 1));
    resolve(&app, &user, &url, "https://cdn.example.invalid/audio/nope.mp3", "Nope").await.assert_status(404);
}
