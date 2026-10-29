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

#[tokio::test(flavor = "multi_thread")]
async fn password_protected_feeds_send_their_credentials_on_every_fetch() {
    let app = TestApp::new().await;
    let user = app.user("patron").await;
    let feed = Feed::with_episodes("Premium Show", 2);
    let url = app.feed("premium.xml", &feed);

    let body = app
        .post("/api/data/add_custom_podcast")
        .key(&user)
        .json(&json!({ "user_id": user.id, "feed_url": url, "username": "patron", "password": "s3cret:pass" }))
        .send()
        .await
        .ok_json();
    let podcast_id = body["data"]["podcastid"].as_i64().unwrap_or_else(|| panic!("{body}")) as i32;
    eventually("premium episodes", || async { (app.episodes(&user, podcast_id).await.len() == 2).then_some(()) }).await;

    app.feed("premium.xml", &feed.episode(Episode::new(3)));
    app.refresh().await;
    assert_eq!(app.episodes(&user, podcast_id).await.len(), 3);

    // base64("patron:s3cret:pass")
    let expected = "Basic cGF0cm9uOnMzY3JldDpwYXNz";
    let hits = app.fixtures().hits("premium.xml");
    assert!(hits.len() >= 2, "{hits:?}");
    for h in &hits {
        assert_eq!(h.headers.get("authorization").map(String::as_str), Some(expected), "every fetch authenticates: {h:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn merged_podcasts_show_as_one_and_can_be_split_again() {
    let app = TestApp::new().await;
    let user = app.user("merger").await;
    let main_url = app.feed("main.xml", &Feed::new("Main Feed").episode(Episode::new(1).title("Main ep")));
    let extra_url = app.feed("extra.xml", &Feed::new("Bonus Feed").episode(Episode::new(1).title("Bonus ep")));
    let main = app.subscribe(&user, &main_url, 1).await;
    let extra = app.subscribe(&user, &extra_url, 1).await;

    let merge = |primary: i32, secondary: Vec<i32>| {
        app.post(&format!("/api/data/{primary}/merge")).key(&user).json(&json!({ "secondary_podcast_ids": secondary }))
    };
    let merged_ids = || async {
        app.get(&format!("/api/data/{main}/merged")).key(&user).send().await.ok_json()["merged_podcast_ids"].clone()
    };
    let names = || async {
        let mut n: Vec<String> =
            app.podcasts(&user).await.iter().map(|p| p["podcastname"].as_str().unwrap().to_string()).collect();
        n.sort();
        n
    };

    assert_eq!(merge(main, vec![main]).send().await.ok_json()["success"], false, "cannot merge into itself");

    let r = merge(main, vec![extra]).send().await.ok_json();
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(merged_ids().await, json!([extra]));
    assert_eq!(names().await, ["Main Feed"], "the secondary podcast is hidden from the list");
    let titles: Vec<String> =
        app.episodes(&user, main).await.iter().map(|e| e["Episodetitle"].as_str().unwrap().to_string()).collect();
    assert!(titles.contains(&"Main ep".to_string()) && titles.contains(&"Bonus ep".to_string()), "{titles:?}");

    let r = app.post(&format!("/api/data/{main}/unmerge/{extra}")).key(&user).send().await.ok_json();
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(merged_ids().await, json!([]));
    assert_eq!(names().await, ["Bonus Feed", "Main Feed"]);
    let titles: Vec<String> =
        app.episodes(&user, main).await.iter().map(|e| e["Episodetitle"].as_str().unwrap().to_string()).collect();
    assert_eq!(titles, ["Main ep"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn podcasts_cannot_be_merged_with_someone_elses() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let mallory = app.user("mallory").await;
    let a = app.subscribe(&alice, &app.feed("a.xml", &Feed::with_episodes("Alice Show", 1)), 1).await;
    let m = app.subscribe(&mallory, &app.feed("m.xml", &Feed::with_episodes("Mallory Show", 1)), 1).await;

    let r = app.post(&format!("/api/data/{m}/merge")).key(&mallory).json(&json!({ "secondary_podcast_ids": [a] })).send().await;
    assert!(!r.is_success() || r.json()["success"] == false, "{}", r.text());
    assert_eq!(app.podcasts(&alice).await.len(), 1, "alice's podcast must stay visible");
}

#[tokio::test(flavor = "multi_thread")]
async fn search_finds_episodes_in_your_own_library_only() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let needle = format!("Zebra{}", app.tag);
    let feed = Feed::new("Search Show")
        .episode(Episode::new(1).title(&format!("All about the {needle}")))
        .episode(Episode::new(2).title("Something else"));
    let url = app.feed("search.xml", &feed);
    app.subscribe(&alice, &url, 2).await;
    app.subscribe(&bob, &app.feed("bob.xml", &Feed::new("Bob Show").episode(Episode::new(1).title(&needle))), 1).await;

    let search = |u: &TestUser, term: &str| {
        app.post("/api/data/search_data").key(u).json(&json!({ "user_id": u.id, "search_term": term }))
    };
    let hits = search(&alice, &needle.to_lowercase()).send().await.ok_json();
    let titles: Vec<&str> = hits["data"].as_array().unwrap().iter().filter_map(|e| e["episodetitle"].as_str()).collect();
    assert_eq!(titles, [format!("All about the {needle}").as_str()], "{hits}");
    assert_eq!(hits["total"], 1);

    assert_eq!(search(&alice, "nothing-matches-this").send().await.ok_json()["total"], 0);
    assert_eq!(search(&bob, &needle).send().await.ok_json()["total"], 1, "bob finds only his own copy");
    let r = app.post("/api/data/search_data").key(&bob).json(&json!({ "user_id": alice.id, "search_term": needle })).send().await;
    assert!(!r.is_success(), "searching someone else's library: {}", r.text());
}

/// An empty search is refused here; the search service is never asked (it would fail, and the
/// client would report the search service as down).
#[tokio::test(flavor = "multi_thread")]
async fn empty_searches_are_refused_without_asking_the_search_service() {
    let app = TestApp::new().await;
    let user = app.user("searcher").await;
    for query in ["", "%20%20"] {
        app.get(&format!("/api/data/proxy_search?query={query}&index=podcast_index")).key(&user).send().await.assert_status(400);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn paged_episodes_sort_and_filter_the_whole_podcast() {
    let app = TestApp::new().await;
    let user = app.user("pager").await;
    let url = app.feed("long.xml", &Feed::with_episodes("Long Show", 10));
    let pod = app.subscribe(&user, &url, 10).await;

    let page = |sort_by: &'static str, sort_order: &'static str, filter: &'static str, limit: i32| {
        let (app, user) = (&app, &user);
        async move {
            let body = app
                .get("/api/data/podcast_episodes")
                .key(user)
                .query("user_id", user.id)
                .query("podcast_id", pod)
                .query("limit", limit)
                .query("sort_by", sort_by)
                .query("sort_order", sort_order)
                .query("filter", filter)
                .send()
                .await
                .ok_json();
            body["episodes"].as_array().unwrap().iter().map(|e| e["Episodeid"].as_i64().unwrap()).collect::<Vec<_>>()
        }
    };

    for (sort_by, order) in [("date", "asc"), ("title", "asc"), ("title", "desc")] {
        let all = page(sort_by, order, "all", 1000).await;
        assert_eq!(all.len(), 10);
        assert_eq!(page(sort_by, order, "all", 3).await, all[..3], "{sort_by} {order}");
    }

    let oldest = page("date", "asc", "all", 1).await[0];
    app.post("/api/data/mark_episode_completed")
        .key(&user)
        .json(&json!({ "episode_id": oldest, "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(page("date", "desc", "completed", 1).await, [oldest]);
}

#[tokio::test(flavor = "multi_thread")]
async fn podcast_list_reports_each_podcasts_oldest_and_newest_episode_dates() {
    let app = TestApp::new().await;
    let user = app.user("dates").await;
    let day = |d: i64| chrono::Utc::now() - chrono::Duration::days(d);
    // The old show started recently but its last episode is old; the active show is the reverse.
    let stale = Feed::new("Stale").episode(Episode::new(1).pub_date(day(30))).episode(Episode::new(2).pub_date(day(20)));
    let active = Feed::new("Active").episode(Episode::new(1).pub_date(day(400))).episode(Episode::new(2).pub_date(day(1)));
    app.subscribe(&user, &app.feed("stale.xml", &stale), 2).await;
    app.subscribe(&user, &app.feed("active.xml", &active), 2).await;

    let pods = app.podcasts(&user).await;
    let dates = |name: &str| {
        let p = pods.iter().find(|p| p["podcastname"] == name).unwrap();
        (p["oldest_episode_date"].as_str().unwrap().to_string(), p["newest_episode_date"].as_str().unwrap().to_string())
    };
    let (stale_old, stale_new) = dates("Stale");
    let (active_old, active_new) = dates("Active");
    assert!(stale_old < stale_new && active_old < active_new);
    assert!(active_new > stale_new, "active's latest episode is the newest");
    assert!(active_old < stale_old);

    let extra = app.get(&format!("/api/data/return_pods_extra/{}", user.id)).key(&user).send().await.ok_json();
    let active = extra["pods"].as_array().unwrap().iter().find(|p| p["podcastname"] == "Active").unwrap();
    assert_eq!(active["oldest_episode_date"].as_str(), Some(active_old.as_str()));
}
