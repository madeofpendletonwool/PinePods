//! Subscribing to feeds, importing episodes, refreshing, unsubscribing.

use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
async fn subscribing_imports_feed_metadata_and_every_episode() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("The Rust Hour", 3).author("Ferris").categories(&["Technology", "Education"]);
    let url = app.feed("rust-hour.xml", &feed);

    let podcast_id = app.subscribe(&user, &url, 3).await;

    let pods = app.podcasts(&user).await;
    assert_eq!(pods.len(), 1, "{pods:#?}");
    let pod = &pods[0];
    assert_eq!(pod["podcastid"], podcast_id);
    // The server re-parses the feed rather than trusting client-supplied metadata.
    assert_eq!(pod["podcastname"], "The Rust Hour");
    assert_eq!(pod["author"], "Ferris");
    assert_eq!(pod["feedurl"], url);
    let cats = pod["categories"].to_string();
    assert!(cats.contains("Technology") && cats.contains("Education"), "categories: {cats}");

    let eps = app.episodes(&user, podcast_id).await;
    let titles: Vec<&str> = eps.iter().map(|e| e["Episodetitle"].as_str().unwrap()).collect();
    assert_eq!(titles, ["Episode 3", "Episode 2", "Episode 1"], "newest first");
    let ep1 = eps.iter().find(|e| e["Episodetitle"] == "Episode 1").unwrap();
    assert_eq!(ep1["Episodeurl"], "https://cdn.example.invalid/audio/ep1.mp3");
    assert_eq!(ep1["Episodeduration"], 1801);
    assert_eq!(ep1["Completed"], false);
    assert_eq!(ep1["saved"], false);
    assert_eq!(ep1["queued"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn check_podcast_reports_subscription_per_user() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 1));
    app.subscribe(&alice, &url, 1).await;

    let check = |u: &TestUser| {
        app.get("/api/data/check_podcast")
            .key(u)
            .query("user_id", u.id)
            .query("podcast_name", "Show")
            .query("podcast_url", &url)
    };
    assert_eq!(check(&alice).send().await.ok_json()["exists"], true);
    assert_eq!(check(&bob).send().await.ok_json()["exists"], false);
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_adds_new_episodes_without_duplicating_existing_ones() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let mut feed = Feed::with_episodes("Growing Show", 2);
    let url = app.feed("growing.xml", &feed);
    let podcast_id = app.subscribe(&user, &url, 2).await;

    feed = feed.episode(Episode::new(3)).episode(Episode::new(4));
    app.feed("growing.xml", &feed);
    app.refresh().await;
    assert_eq!(app.episodes(&user, podcast_id).await.len(), 4);

    // Refreshing an unchanged feed is a no-op.
    app.refresh().await;
    app.refresh().await;
    let titles: Vec<String> = app
        .episodes(&user, podcast_id)
        .await
        .iter()
        .map(|e| e["Episodetitle"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(titles, ["Episode 4", "Episode 3", "Episode 2", "Episode 1"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retitled_episode_with_the_same_guid_is_updated_not_duplicated() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::new("Edits").episode(Episode::new(1).title("Original title"));
    let url = app.feed("edits.xml", &feed);
    let podcast_id = app.subscribe(&user, &url, 1).await;

    let edited = Feed::new("Edits").episode(Episode::new(1).title("Corrected title"));
    app.feed("edits.xml", &edited);
    app.refresh().await;

    let eps = app.episodes(&user, podcast_id).await;
    assert_eq!(eps.len(), 1, "same GUID must not create a second episode: {eps:#?}");
    assert_eq!(eps[0]["Episodetitle"], "Corrected title", "feed edits propagate");
}

#[tokio::test(flavor = "multi_thread")]
async fn episodes_without_guids_are_deduplicated_across_refreshes() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::new("No GUIDs")
        .episode(Episode::new(1).guid(None))
        .episode(Episode::new(2).guid(None));
    let url = app.feed("noguid.xml", &feed);
    let podcast_id = app.subscribe(&user, &url, 2).await;

    app.refresh().await;
    app.refresh().await;
    assert_eq!(app.episodes(&user, podcast_id).await.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_uses_conditional_get_and_honours_304() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Cached Show", 2);
    let url = app.fixtures().serve("cached.xml", Fixture::new("application/rss+xml", feed.to_xml()).etag("\"v1\""));
    let podcast_id = app.subscribe(&user, &url, 2).await;

    // First scheduled refresh learns the ETag, the next one must send it back.
    app.refresh().await;
    app.refresh().await;
    let hits = app.fixtures().hits("cached.xml");
    assert!(
        hits.iter().any(|h| h.headers.get("if-none-match").map(String::as_str) == Some("\"v1\"")),
        "no conditional request among {} fetches: {:?}",
        hits.len(),
        hits.iter().map(|h| &h.headers).collect::<Vec<_>>()
    );
    assert_eq!(app.episodes(&user, podcast_id).await.len(), 2, "304 must leave episodes untouched");

    // A changed feed (new ETag) is picked up again.
    let feed = feed.episode(Episode::new(3));
    app.fixtures().serve("cached.xml", Fixture::new("application/rss+xml", feed.to_xml()).etag("\"v2\""));
    app.refresh().await;
    assert_eq!(app.episodes(&user, podcast_id).await.len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_feed_keeps_its_episodes_and_does_not_break_other_refreshes() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let broken_url = app.feed("broken.xml", &Feed::with_episodes("Soon Broken", 2));
    let healthy = Feed::with_episodes("Healthy", 1);
    let healthy_url = app.feed("healthy.xml", &healthy);
    let broken_id = app.subscribe(&user, &broken_url, 2).await;
    let healthy_id = app.subscribe(&user, &healthy_url, 1).await;

    app.fixtures().serve("broken.xml", Fixture::new("text/plain", "upstream exploded").status(500));
    app.feed("healthy.xml", &healthy.episode(Episode::new(2)));
    app.refresh().await;

    assert_eq!(app.episodes(&user, broken_id).await.len(), 2, "existing episodes survive a failed fetch");
    assert_eq!(app.episodes(&user, healthy_id).await.len(), 2, "other feeds still refresh");
}

#[tokio::test(flavor = "multi_thread")]
async fn two_users_on_the_same_feed_each_see_it() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let feed = Feed::with_episodes("Shared Show", 2);
    let url = app.feed("shared.xml", &feed);
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;

    app.feed("shared.xml", &feed.episode(Episode::new(3)));
    app.refresh().await;
    assert_eq!(app.episodes(&alice, a).await.len(), 3);
    assert_eq!(app.episodes(&bob, b).await.len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn unsubscribing_removes_only_that_users_podcast() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let url = app.feed("leaving.xml", &Feed::with_episodes("Leaving", 2));
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;

    let body = app
        .post("/api/data/remove_podcast_id")
        .key(&alice)
        .json(&json!({ "user_id": alice.id, "podcast_id": a }))
        .send()
        .await
        .ok_json();
    assert_eq!(body["success"], true);

    assert!(app.podcasts(&alice).await.is_empty());
    assert_eq!(app.podcasts(&bob).await.len(), 1);
    assert_eq!(app.episodes(&bob, b).await.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_user_cannot_unsubscribe_someone_else() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let mallory = app.user("mallory").await;
    let url = app.feed("mine.xml", &Feed::with_episodes("Mine", 1));
    let a = app.subscribe(&alice, &url, 1).await;

    let resp = app
        .post("/api/data/remove_podcast_id")
        .key(&mallory)
        .json(&json!({ "user_id": mallory.id, "podcast_id": a }))
        .send()
        .await;
    assert!(!resp.is_success() || resp.json()["success"] == false, "{}", resp.text());
    assert_eq!(app.podcasts(&alice).await.len(), 1, "alice's subscription must survive");
}
