//! A podcast's details (name, artwork, author, description, website) follow its feed: refreshes
//! apply a rebrand, keep what the user edited by hand, and never blank a field the feed dropped.

use serde_json::json;

use crate::harness::*;

async fn column(app: &TestApp, podcast_id: i32, col: &str) -> Option<String> {
    text(app, &format!(r#"SELECT {col} FROM "Podcasts" WHERE PodcastID = {podcast_id}"#)).await
}

async fn episode_artwork(app: &TestApp, podcast_id: i32, guid: &str) -> Option<String> {
    text(
        app,
        &format!(
            r#"SELECT EpisodeArtwork FROM "Episodes" WHERE EpisodeGUID = '{guid}'
               AND FeedID = (SELECT FeedID FROM "Podcasts" WHERE PodcastID = {podcast_id})"#
        ),
    )
    .await
}

fn rebranded(mut feed: Feed) -> Feed {
    feed.title = "New Name".into();
    feed.author = "New Hosts".into();
    feed.description = "The show, rebranded".into();
    feed.link = "https://example.invalid/new-site".into();
    feed.image = "https://example.invalid/new-art.jpg".into();
    feed
}

async fn edit(app: &TestApp, user: &TestUser, podcast_id: i32, fields: serde_json::Value) {
    let mut body = json!({ "user_id": user.id, "podcast_id": podcast_id });
    body.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    let resp = app.put("/api/data/update_podcast_info").key(user).json(&body).send().await.ok_json();
    assert_eq!(resp["success"], true, "{resp}");
}

async fn refresh_details(app: &TestApp, user: &TestUser, podcast_id: i32, reset_edits: bool) {
    let resp = app
        .post("/api/data/refresh_podcast_details")
        .key(user)
        .json(&json!({ "podcast_id": podcast_id, "reset_edits": reset_edits }))
        .send()
        .await
        .ok_json();
    assert_eq!(resp["success"], true, "{resp}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rebranded_feed_updates_the_podcast_and_its_fallback_artwork() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let mut own_art = Episode::new(2);
    own_art.image = Some("https://example.invalid/ep2.jpg".into());
    let feed = Feed::new("Old Name").episode(Episode::new(1)).episode(own_art);
    let url = app.feed("rebrand.xml", &feed);
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;
    assert_eq!(episode_artwork(&app, a, "guid-1").await.as_deref(), Some("https://example.invalid/art.jpg"));
    assert_eq!(episode_artwork(&app, a, "guid-2").await.as_deref(), Some("https://example.invalid/ep2.jpg"));

    // The rebranded feed also dropped episode 1; it keeps its place but gets the new artwork.
    let mut new_feed = rebranded(feed);
    new_feed.episodes.retain(|e| e.guid.as_deref() != Some("guid-1"));
    app.feed("rebrand.xml", &new_feed.episode(Episode::new(3)));
    app.refresh().await;

    for pod in [a, b] {
        assert_eq!(column(&app, pod, "PodcastName").await.as_deref(), Some("New Name"));
        assert_eq!(column(&app, pod, "Author").await.as_deref(), Some("New Hosts"));
        assert_eq!(column(&app, pod, "Description").await.as_deref(), Some("The show, rebranded"));
        assert_eq!(column(&app, pod, "WebsiteURL").await.as_deref(), Some("https://example.invalid/new-site"));
        assert_eq!(column(&app, pod, "ArtworkURL").await.as_deref(), Some("https://example.invalid/new-art.jpg"));
    }
    assert_eq!(episode_artwork(&app, a, "guid-1").await.as_deref(), Some("https://example.invalid/new-art.jpg"));
    assert_eq!(episode_artwork(&app, a, "guid-3").await.as_deref(), Some("https://example.invalid/new-art.jpg"));
    assert_eq!(episode_artwork(&app, a, "guid-2").await.as_deref(), Some("https://example.invalid/ep2.jpg"), "own artwork stays");
}

#[tokio::test(flavor = "multi_thread")]
async fn hand_edited_details_survive_refreshes_until_reset() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Old Name", 1);
    let url = app.feed("edited.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    edit(&app, &user, pod, json!({ "podcast_name": "My Name" })).await;
    app.feed("edited.xml", &rebranded(feed).episode(Episode::new(2)));
    app.refresh().await;
    assert_eq!(column(&app, pod, "PodcastName").await.as_deref(), Some("My Name"), "the user's name is kept");
    assert_eq!(column(&app, pod, "ArtworkURL").await.as_deref(), Some("https://example.invalid/new-art.jpg"), "unedited fields follow the feed");

    // Refreshing the details keeps edits; resetting hands them back to the feed, even though the
    // feed hasn't changed since the last refresh.
    refresh_details(&app, &user, pod, false).await;
    assert_eq!(column(&app, pod, "PodcastName").await.as_deref(), Some("My Name"));
    refresh_details(&app, &user, pod, true).await;
    assert_eq!(column(&app, pod, "PodcastName").await.as_deref(), Some("New Name"));
    assert_eq!(column(&app, pod, "EditedMetadata").await, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn saving_unchanged_details_marks_nothing_as_edited() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Same Name", 1);
    let url = app.feed("unchanged.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    edit(&app, &user, pod, json!({ "podcast_name": "Same Name", "author": "Someone Else" })).await;
    assert_eq!(column(&app, pod, "EditedMetadata").await.as_deref(), Some("author"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_feed_dropping_a_field_does_not_blank_it() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Keeps Its Name", 1).author("The Author");
    let url = app.feed("dropped.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    let mut broken = feed.episode(Episode::new(2));
    broken.title = String::new();
    broken.author = String::new();
    app.feed("dropped.xml", &broken);
    app.refresh().await;
    assert_eq!(app.episodes(&user, pod).await.len(), 2);
    assert_eq!(column(&app, pod, "PodcastName").await.as_deref(), Some("Keeps Its Name"));
    assert_eq!(column(&app, pod, "Author").await.as_deref(), Some("The Author"));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_upgrade_reads_every_feed_in_full_once() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Current Name", 1);
    let url = app.fixtures().serve("upgrade.xml", Fixture::new("application/rss+xml", feed.to_xml()).etag("\"v1\""));
    let pod = app.subscribe(&user, &url, 1).await;
    app.refresh().await; // learns the ETag

    // Details stored by an older release that never updated them.
    exec(&app, &format!(r#"UPDATE "Podcasts" SET PodcastName = 'Stale Name' WHERE PodcastID = {pod}"#)).await;
    app.refresh().await;
    assert_eq!(column(&app, pod, "PodcastName").await.as_deref(), Some("Stale Name"), "a 304 doesn't reapply");

    rerun_migrations(&app, &["085"]).await;
    app.refresh().await;
    assert_eq!(column(&app, pod, "PodcastName").await.as_deref(), Some("Current Name"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_episodes_own_itunes_image_is_kept() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    // Only <itunes:image> and no iTunes title/summary (which feed-rs needs to keep it), on a CDN
    // address without a file extension.
    let mut ep = Episode::new(1);
    ep.image = Some("https://img.example.invalid/a1b2c3".into());
    let url = app.feed("episode-art.xml", &Feed::new("Art Show").episode(ep).episode(Episode::new(2)));
    let pod = app.subscribe(&user, &url, 2).await;
    assert_eq!(episode_artwork(&app, pod, "guid-1").await.as_deref(), Some("https://img.example.invalid/a1b2c3"));
    assert_eq!(episode_artwork(&app, pod, "guid-2").await.as_deref(), Some("https://example.invalid/art.jpg"));
}
