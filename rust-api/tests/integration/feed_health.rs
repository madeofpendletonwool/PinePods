//! Feed problems: podcasts whose refresh keeps failing (kind, reporting once, recovery) and feeds
//! from sync apps that couldn't be added (backoff, retry, give up, remove, dismiss).

use crate::gpodder::strings;
use crate::harness::gpodder::*;
use crate::harness::*;

async fn problems(app: &TestApp, user: &TestUser) -> Value {
    app.get("/api/data/feed_problems").key(user).send().await.ok_json()
}

async fn messages(app: &TestApp, user: &TestUser, category: &str) -> Vec<Value> {
    let all = app.get(&format!("/api/data/notifications?user_id={}", user.id)).key(user).send().await.ok_json();
    all.as_array().unwrap().iter().filter(|m| m["category"] == category).cloned().collect()
}

/// Let the failure backoff pass, so the next refresh fetches the feed again.
async fn due_again(app: &TestApp, pod: i32) {
    exec(app, &format!(r#"UPDATE "Podcasts" SET LastRefreshAttempt = '2000-01-01 00:00:00' WHERE PodcastID = {pod}"#)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_feed_is_listed_reported_once_and_cleared_when_it_recovers() {
    let app = TestApp::new().await;
    let user = app.user("health").await;
    let other = app.user("health-other").await;
    let feed = Feed::with_episodes("Flaky", 1);
    let url = app.feed("flaky.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    app.fixtures().serve("flaky.xml", Fixture::new("text/plain", "oops").status(502));
    app.refresh().await;
    let p = problems(&app, &user).await;
    let f = &p["feeds"][0];
    assert_eq!((f["podcast_id"].as_i64(), f["kind"].as_str(), f["failures"].as_i64()), (Some(pod as i64), Some("server"), Some(1)), "{p}");
    assert_eq!(f["reported"], false);
    assert!(f["failing_since"].as_i64().is_some());
    assert_eq!(problems(&app, &other).await["feeds"], json!([]), "only the owner sees it");
    assert!(messages(&app, &user, "feed_problem").await.is_empty(), "not reported before a day of failures");

    // A day later and still failing: reported, once.
    exec(&app, &format!(r#"UPDATE "Podcasts" SET FailingSince = '2000-01-01 00:00:00' WHERE PodcastID = {pod}"#)).await;
    due_again(&app, pod).await;
    app.refresh().await;
    let reported = messages(&app, &user, "feed_problem").await;
    assert_eq!(reported.len(), 1, "{reported:?}");
    assert_eq!(reported[0]["podcast_id"].as_i64(), Some(pod as i64));
    assert_eq!(reported[0]["severity"], "warning");
    due_again(&app, pod).await;
    app.refresh().await;
    assert_eq!(messages(&app, &user, "feed_problem").await.len(), 1, "one message per run of failures");
    assert_eq!(problems(&app, &user).await["feeds"][0]["reported"], true);

    // It works again: off the list, and the user hears so.
    app.feed("flaky.xml", &feed);
    due_again(&app, pod).await;
    app.refresh().await;
    assert_eq!(problems(&app, &user).await["feeds"], json!([]));
    assert_eq!(messages(&app, &user, "feed_recovered").await.len(), 1);
    let cleared = int(&app, &format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE PodcastID = {pod} AND FailingSince IS NULL AND LastErrorKind IS NULL AND NOT ProblemNotified"#)).await;
    assert_eq!(cleared, Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_gone_feed_is_reported_after_three_failures_and_can_be_refreshed_from_the_list() {
    let app = TestApp::new().await;
    let user = app.user("gone").await;
    let feed = Feed::with_episodes("Moved", 1);
    let url = app.feed("moved.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    app.fixtures().serve("moved.xml", Fixture::new("text/plain", "not here").status(404));
    for _ in 0..3 {
        due_again(&app, pod).await;
        app.refresh().await;
    }
    assert_eq!(problems(&app, &user).await["feeds"][0]["kind"], "gone");
    assert_eq!(messages(&app, &user, "feed_problem").await.len(), 1, "gone feeds don't wait a day");

    // "Refresh now" from the list, after the host fixed it.
    app.feed("moved.xml", &feed.clone().episode(Episode::new(2)));
    let after = app.post(&format!("/api/data/feed_problems/{pod}/refresh")).key(&user).send().await.ok_json();
    assert_eq!(after["feeds"], json!([]), "{after}");
    assert_eq!(app.episodes(&user, pod).await.len(), 2);

    let other = app.user("gone-other").await;
    app.post(&format!("/api/data/feed_problems/{pod}/refresh")).key(&other).send().await.assert_status(404);
}

// ---- failed sync imports ------------------------------------------------------------------

async fn import_failures(app: &TestApp, user: &TestUser) -> Vec<Value> {
    problems(app, user).await["imports"].as_array().unwrap().clone()
}

async fn wait_for_failure(app: &TestApp, user: &TestUser, url: &str, attempts: i64) -> Value {
    eventually(&format!("failed import of {url} recorded ({attempts} attempts)"), || async {
        import_failures(app, user).await.into_iter().find(|f| f["feed_url"] == url && f["attempts"].as_i64() == Some(attempts))
    })
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_feed_an_app_subscribed_to_that_fails_is_retried_and_imports_when_it_works() {
    let s = GpodderScene::start().await;
    let phone = s.device("phone").await;
    let url = s.app.fixtures().url("not-yet.xml");
    phone.upload_subscription_changes(&[&url], &[]).await.assert_ok();

    let f = wait_for_failure(&s.app, &s.user, &url, 1).await;
    assert_eq!((f["source"].as_str(), f["kind"].as_str()), (Some("client"), Some("gone")), "{f}");
    let wait = f["next_attempt_at"].as_i64().unwrap() - f["last_attempt_at"].as_i64().unwrap();
    assert!((3500..=3700).contains(&wait), "first retry after an hour: {wait}s");

    // The feed appears; retrying now imports it and clears the failure.
    s.app.feed("not-yet.xml", &Feed::with_episodes("Late", 1));
    let id = f["id"].as_i64().unwrap();
    s.app.post(&format!("/api/data/feed_problems/imports/{id}/retry")).key(&s.user).send().await.ok_json();
    eventually("the retried import cleared the failure", || async { import_failures(&s.app, &s.user).await.is_empty().then_some(()) }).await;
    let imported = int(&s.app, &format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE UserID = {} AND FeedURL = '{url}'"#, s.user.id)).await;
    assert_eq!(imported, Some(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_import_gives_up_after_its_retries_and_tells_the_user() {
    let s = GpodderScene::start().await;
    let phone = s.device("phone").await;
    let url = s.app.fixtures().url("never.xml");
    phone.upload_subscription_changes(&[&url], &[]).await.assert_ok();
    let f = wait_for_failure(&s.app, &s.user, &url, 1).await;
    let id = f["id"].as_i64().unwrap();

    exec(&s.app, &format!(r#"UPDATE "SyncImportFailures" SET Attempts = 9 WHERE FailureID = {id}"#)).await;
    s.app.post(&format!("/api/data/feed_problems/imports/{id}/retry")).key(&s.user).send().await.ok_json();
    let f = wait_for_failure(&s.app, &s.user, &url, 10).await;
    assert_eq!(f["next_attempt_at"], Value::Null, "given up: {f}");
    let told = messages(&s.app, &s.user, "sync_import_failed").await;
    assert_eq!(told.len(), 1, "{told:?}");

    // Due imports are only those still being retried.
    assert!(s.app.db().sync_imports_due(100).await.unwrap().iter().all(|(_, u, _)| *u != url));
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_a_failed_import_unsubscribes_the_apps_and_dismissing_only_hides_it() {
    let s = GpodderScene::start().await;
    let phone = s.device("phone").await;
    let (gone, kept) = (s.app.fixtures().url("gone-a.xml"), s.app.fixtures().url("gone-b.xml"));
    phone.upload_subscription_changes(&[&gone, &kept], &[]).await.assert_ok();
    let a = wait_for_failure(&s.app, &s.user, &gone, 1).await["id"].as_i64().unwrap();
    let b = wait_for_failure(&s.app, &s.user, &kept, 1).await["id"].as_i64().unwrap();
    let before = phone.subscription_changes(0).await.ok_json()["timestamp"].as_i64().unwrap();

    // Other users can't touch them.
    let other = s.app.user("imports-other").await;
    s.app.post(&format!("/api/data/feed_problems/imports/{a}/remove")).key(&other).send().await.assert_status(404);
    s.app.delete(&format!("/api/data/feed_problems/imports/{b}")).key(&other).send().await.assert_status(404);
    s.app.post(&format!("/api/data/feed_problems/imports/{a}/retry")).key(&other).send().await.assert_status(404);

    s.app.post(&format!("/api/data/feed_problems/imports/{a}/remove")).key(&s.user).send().await.ok_json();
    s.app.delete(&format!("/api/data/feed_problems/imports/{b}")).key(&s.user).send().await.ok_json();
    assert!(import_failures(&s.app, &s.user).await.is_empty());

    // The app hears about the removal; the dismissed one stays subscribed for it.
    let changes = phone.subscription_changes(before).await.ok_json();
    assert_eq!(strings(&changes, "remove"), vec![gone.clone()], "{changes}");
    let all = phone.subscription_changes(0).await.ok_json();
    assert!(strings(&all, "add").contains(&kept) && !strings(&all, "add").contains(&gone), "{all}");
}

#[tokio::test(flavor = "multi_thread")]
async fn feeds_already_failing_count_from_their_last_attempt_after_the_upgrade() {
    let app = TestApp::new().await;
    let user = app.user("upgrade-health").await;
    let url = app.feed("old-fail.xml", &Feed::with_episodes("Old", 1));
    let pod = app.subscribe(&user, &url, 1).await;
    exec(&app, &format!(
        r#"UPDATE "Podcasts" SET ConsecutiveFailures = 4, LastRefreshAttempt = '2026-01-02 03:04:05', LastErrorMessage = 'Feed gone: HTTP 404' WHERE PodcastID = {pod}"#
    ))
    .await;
    exec(&app, r#"DROP TABLE "SyncImportFailures""#).await;
    for column in ["FailingSince", "LastErrorKind", "ProblemNotified"] {
        exec(&app, &format!(r#"ALTER TABLE "Podcasts" DROP COLUMN {column}"#)).await;
    }

    rerun_migrations(&app, &["071"]).await;
    rerun_migrations(&app, &["071"]).await;

    let since = int(&app, &format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE PodcastID = {pod} AND FailingSince = LastRefreshAttempt"#)).await;
    assert_eq!(since, Some(1));
    let p = problems(&app, &user).await;
    assert_eq!((p["feeds"][0]["failures"].as_i64(), p["feeds"][0]["kind"].as_str()), (Some(4), Some("other")), "{p}");
    assert_eq!(p["imports"], json!([]));
}
