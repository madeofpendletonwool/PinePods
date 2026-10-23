//! Feeds shared by subscriptions: one feed per address and credentials, one fetch per feed,
//! uploads kept per owner, feeds following address changes, orphan cleanup, and the upgrade
//! that creates feeds from existing podcasts.

use pinepods_api::handlers::refresh;
use pinepods_api::services::sync::SyncOrigin;

use crate::harness::*;

async fn feed_of(app: &TestApp, pod: i32) -> Option<i64> {
    int(app, &format!(r#"SELECT FeedID FROM "Podcasts" WHERE PodcastID = {pod}"#)).await
}

async fn feeds(app: &TestApp) -> i64 {
    int(app, r#"SELECT COUNT(*) FROM "Feeds""#).await.unwrap()
}

/// A podcast row added straight to the database, then linked like the add paths do.
async fn insert_podcast(app: &TestApp, user: &TestUser, url: &str, creds: Option<(&str, &str)>) -> i32 {
    let (username, password) = match creds {
        Some((u, p)) => (format!("'{u}'"), format!("'{p}'")),
        None => ("NULL".to_string(), "NULL".to_string()),
    };
    exec(
        app,
        &format!(
            r#"INSERT INTO "Podcasts" (PodcastName, FeedURL, UserID, Username, Password) VALUES ('P', '{url}', {}, {username}, {password})"#,
            user.id
        ),
    )
    .await;
    let pod = int(app, r#"SELECT MAX(PodcastID) FROM "Podcasts""#).await.unwrap() as i32;
    app.db().link_feed(pod).await.unwrap();
    pod
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribers_share_one_feed_and_one_fetch() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("shared.xml", &Feed::with_episodes("Shared", 2));
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;

    let feed = feed_of(&app, a).await.expect("linked when added");
    assert_eq!(feed_of(&app, b).await, Some(feed));
    assert_eq!(feeds(&app).await, 1);

    set_next_refresh(&app, a, "2000-01-01 00:00:00").await;
    let before = app.fixtures().hits("shared.xml").len();
    assert_eq!(refresh::refresh_due_podcasts(&app.state).await.unwrap(), 1);
    assert_eq!(app.fixtures().hits("shared.xml").len(), before + 1, "fetched once for both");
    let waiting = format!(r#"SELECT COUNT(*) FROM "Feeds" WHERE FeedID = {feed} AND NextRefreshAt > NOW()"#);
    assert_eq!(int(&app, &waiting).await, Some(1), "rescheduled");
    assert_eq!(refresh::refresh_due_podcasts(&app.state).await.unwrap(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn private_feeds_are_keyed_by_credentials_and_uploads_by_owner() {
    let app = TestApp::new().await;
    let (alice, bob, carol) = (app.user("alice").await, app.user("bob").await, app.user("carol").await);
    let url = "https://example.invalid/private.xml";
    let a = insert_podcast(&app, &alice, url, Some(("member", "secret"))).await;
    let b = insert_podcast(&app, &bob, url, Some(("member", "secret"))).await;
    let c = insert_podcast(&app, &carol, url, Some(("member", "other"))).await;
    let public = insert_podcast(&app, &carol, "https://example.invalid/public.xml", None).await;
    let public2 = insert_podcast(&app, &alice, "https://example.invalid/public.xml", None).await;
    let local_a = insert_podcast(&app, &alice, "local://shows/1", None).await;
    let local_b = insert_podcast(&app, &bob, "local://shows/1", None).await;

    assert_eq!(feed_of(&app, a).await, feed_of(&app, b).await, "same credentials share the feed");
    assert_ne!(feed_of(&app, a).await, feed_of(&app, c).await, "other credentials don't");
    assert_eq!(feed_of(&app, public).await, feed_of(&app, public2).await);
    assert_ne!(feed_of(&app, local_a).await, feed_of(&app, local_b).await, "uploads stay with their owner");
    assert_eq!(feeds(&app).await, 5);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manual_refresh_refreshes_the_feed_for_every_subscriber() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let feed = Feed::with_episodes("Shared", 2);
    let url = app.feed("shared.xml", &feed);
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;

    app.feed("shared.xml", &feed.episode(Episode::new(3)));
    assert!(refresh::refresh_podcast_now(&app.state, alice.id, a).await.unwrap());
    assert_eq!(app.episodes(&alice, a).await.len(), 3);
    assert_eq!(app.episodes(&bob, b).await.len(), 3, "the other subscriber isn't left behind");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_address_moves_the_subscription_to_that_feed() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let url = app.feed("old.xml", &Feed::with_episodes("Show", 1));
    let pod = app.subscribe(&alice, &url, 1).await;
    let old = feed_of(&app, pod).await.unwrap();

    exec(&app, &format!(r#"UPDATE "Podcasts" SET FeedURL = 'https://example.invalid/new.xml' WHERE PodcastID = {pod}"#)).await;
    assert_eq!(app.db().link_unlinked_feeds().await.unwrap(), 1);
    let new = feed_of(&app, pod).await.unwrap();
    assert_ne!(new, old);
    let url_sql = format!(r#"SELECT FeedURL FROM "Feeds" WHERE FeedID = {new}"#);
    assert_eq!(text(&app, &url_sql).await.as_deref(), Some("https://example.invalid/new.xml"));
    assert_eq!(app.db().link_unlinked_feeds().await.unwrap(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn feeds_without_subscribers_are_deleted() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("shared.xml", &Feed::with_episodes("Shared", 1));
    let a = app.subscribe(&alice, &url, 1).await;
    let b = app.subscribe(&bob, &url, 1).await;
    let feed = feed_of(&app, a).await.unwrap();
    exec(&app, &format!(r#"UPDATE "Feeds" SET CreatedAt = '2000-01-01 00:00:00' WHERE FeedID = {feed}"#)).await;

    app.db().remove_podcast_id(a, alice.id, SyncOrigin::Unlogged).await.unwrap();
    assert_eq!(app.db().delete_orphan_feeds().await.unwrap(), 0, "bob still subscribes");
    app.db().remove_podcast_id(b, bob.id, SyncOrigin::Unlogged).await.unwrap();
    assert_eq!(app.db().delete_orphan_feeds().await.unwrap(), 1);
    assert_eq!(feeds(&app).await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrading_creates_feeds_with_the_merged_refresh_state() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let shared = app.feed("shared.xml", &Feed::with_episodes("Shared", 1));
    let other = app.feed("other.xml", &Feed::with_episodes("Other", 1));
    let a = app.subscribe(&alice, &shared, 1).await;
    let b = app.subscribe(&bob, &shared, 1).await;
    let c = app.subscribe(&bob, &other, 1).await;

    // As before the upgrade: no feeds, each podcast with its own refresh state.
    exec(&app, r#"DELETE FROM "Feeds""#).await;
    exec(
        &app,
        &format!(
            r#"UPDATE "Podcasts" SET NextRefreshAt = '2030-01-01 00:00:00', ConsecutiveFailures = 3, LastErrorKind = 'server',
                      FeedContentHash = 'h1', FeedETag = NULL WHERE PodcastID = {a}"#
        ),
    )
    .await;
    exec(
        &app,
        &format!(
            r#"UPDATE "Podcasts" SET NextRefreshAt = '2029-01-01 00:00:00', ConsecutiveFailures = 0, LastErrorKind = NULL,
                      FeedContentHash = 'h1', FeedETag = 'e1' WHERE PodcastID = {b}"#
        ),
    )
    .await;
    exec(&app, &format!(r#"UPDATE "Podcasts" SET FeedContentHash = 'h2' WHERE PodcastID = {c}"#)).await;
    rerun_migrations(&app, &["080"]).await;

    let feed = feed_of(&app, a).await.expect("linked");
    assert_eq!(feed_of(&app, b).await, Some(feed));
    assert_ne!(feed_of(&app, c).await, Some(feed));
    assert_eq!(feeds(&app).await, 2);
    let merged = format!(
        r#"SELECT COUNT(*) FROM "Feeds" WHERE FeedID = {feed} AND NextRefreshAt = '2029-01-01 00:00:00'
           AND ConsecutiveFailures = 0 AND LastErrorKind IS NULL AND FeedContentHash = 'h1' AND FeedETag = 'e1'"#
    );
    assert_eq!(int(&app, &merged).await, Some(1), "earliest refresh, healthiest state, agreed hash");

    // The server finds the feeds the upgrade created.
    assert_eq!(app.db().link_feed(a).await.unwrap().map(i64::from), Some(feed));
    assert_eq!(app.db().link_unlinked_feeds().await.unwrap(), 0);
    assert_eq!(feeds(&app).await, 2);
}
