//! Podping: an admin turns it on, Pinepods joins the relay at its current position, and feeds
//! the relay announces are refreshed right away when this server has them.

use pinepods_api::services::podping;

use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
async fn announced_feeds_are_refreshed_and_the_admin_sees_the_state() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let user = app.user("podping").await;
    let feed = Feed::with_episodes("Pinged", 1);
    let url = app.feed("pinged.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    // Off by default, and only admins can see or change it.
    let status = app.get("/api/data/update_services").key(admin).send().await.ok_json();
    assert_eq!(status["podping"]["enabled"], false, "{status}");
    app.get("/api/data/update_services").key(&user).send().await.assert_status(403);
    app.put("/api/data/update_services/podping").key(&user).json(&json!({ "enabled": true })).send().await.assert_status(403);
    app.put("/api/data/update_services/podping").key(admin).json(&json!({ "enabled": true })).send().await.ok_json();
    assert!(app.db().podping_enabled().await.unwrap());

    // The relay announces the feed (written with the other scheme and a trailing slash).
    let announced = format!("{}/", url.replacen("http://", "https://", 1));
    let relay = app.fixtures().url("relay");
    app.fixtures().serve(
        "relay/api/podping/updates",
        Fixture::new("application/json", json!({ "epoch": 7, "next": 42, "more": false, "pings": [{ "seq": 42, "url": announced }] }).to_string()),
    );
    app.feed("pinged.xml", &feed.clone().episode(Episode::new(2)));

    // The first poll only joins: what was announced before isn't replayed.
    podping::poll_relay(&app.state, &relay).await.unwrap();
    assert_eq!(app.episodes(&user, pod).await.len(), 1);
    let hits = app.fixtures().hits("relay/api/podping/updates");
    assert_eq!(hits.last().unwrap().query.as_deref(), Some("limit=0"));

    // Next poll: the announced feed is refreshed now, not at the next scheduled refresh.
    assert_eq!(podping::poll_relay(&app.state, &relay).await.unwrap(), 1);
    assert_eq!(app.episodes(&user, pod).await.len(), 2);
    let hits = app.fixtures().hits("relay/api/podping/updates");
    assert_eq!(hits.last().unwrap().query.as_deref(), Some("after=42"));
    let client = podping::client_status();
    assert!(client.feeds_refreshed >= 1 && client.last_error.is_none(), "{client:?}");

    // A relay that doesn't answer shows as an error, and nothing breaks.
    app.fixtures().serve("relay/api/podping/updates", Fixture::new("text/plain", "down").status(502));
    podping::poll_relay(&app.state, &relay).await.unwrap();
    assert!(podping::client_status().last_error.is_some_and(|e| e.contains("502")));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_search_service_that_is_down_is_reported_as_such() {
    let app = TestApp::new().await;
    let user = app.user("search-down").await;
    // Outbound requests fail in tests (the proxy is a dead port), as if the service were down.
    let r = app.get("/api/data/proxy_search?query=anything&index=podcastindex").key(&user).send().await;
    r.assert_status(503);
    let body = r.json();
    assert!(body["message"].as_str().is_some_and(|m| m.contains("search service")), "{body}");
}
