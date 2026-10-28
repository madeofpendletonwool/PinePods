//! Outbound feeds: per-user RSS (with RSS keys and podcast scopes) and public episode share links.

use crate::harness::*;

struct Setup {
    user: TestUser,
    alpha: i32,
    beta: i32,
}

/// A user subscribed to two shows whose episode titles are globally unique.
async fn setup(app: &TestApp, name: &str) -> Setup {
    let user = app.user(name).await;
    let show = |label: &str| {
        Feed::new(&format!("{label} Show {name}"))
            .episode(Episode::new(1).title(&format!("{label} first {name}")))
            .episode(Episode::new(2).title(&format!("{label} second {name}")))
    };
    let a = app.feed(&format!("alpha-{name}.xml"), &show("Alpha"));
    let b = app.feed(&format!("beta-{name}.xml"), &show("Beta"));
    let alpha = app.subscribe(&user, &a, 2).await;
    let beta = app.subscribe(&user, &b, 2).await;
    Setup { user, alpha, beta }
}

async fn rss_key(app: &TestApp, u: &TestUser) -> String {
    let body = app.get("/api/data/get_rss_key").key(u).query("user_id", u.id).send().await.ok_json();
    body["rss_key"].as_str().expect("rss_key").to_string()
}

async fn enable_rss(app: &TestApp, u: &TestUser) {
    let body = app.post("/api/data/toggle_rss_feeds").key(u).send().await.ok_json();
    assert_eq!(body["enabled"], true, "{body}");
}

fn feed(app: &TestApp, user_id: i32, key: &str) -> crate::harness::http::RequestBuilder {
    app.get(&format!("/api/feed/{user_id}")).query("api_key", key)
}

#[tokio::test(flavor = "multi_thread")]
async fn rss_feeds_are_off_until_the_user_enables_them() {
    let app = TestApp::new().await;
    let s = setup(&app, "owner").await;
    let key = rss_key(&app, &s.user).await;

    let status = || async { app.get("/api/data/rss_feed_status").key(&s.user).send().await.ok_json() };
    assert_eq!(status().await, false);
    feed(&app, s.user.id, &key).send().await.assert_status(403);

    enable_rss(&app, &s.user).await;
    assert_eq!(status().await, true);
    let resp = feed(&app, s.user.id, &key).send().await;
    resp.assert_ok();
    assert_eq!(resp.headers["content-type"], "application/rss+xml; charset=utf-8");
    let xml = resp.text();
    assert!(xml.starts_with("<?xml") && xml.contains("<rss"), "{xml}");
    for title in ["Alpha first owner", "Alpha second owner", "Beta first owner", "Beta second owner"] {
        assert!(xml.contains(title), "missing {title}:\n{xml}");
    }

    let only_alpha = feed(&app, s.user.id, &key).query("podcast_id", s.alpha).send().await.text();
    assert!(only_alpha.contains("Alpha first owner"));
    assert!(!only_alpha.contains("Beta first owner"), "podcast_id filters the feed:\n{only_alpha}");

    let limited = feed(&app, s.user.id, &key).query("limit", 1).send().await.text();
    assert_eq!(limited.matches("<item>").count(), 1, "{limited}");
}

#[tokio::test(flavor = "multi_thread")]
async fn one_users_rss_key_never_reveals_another_users_feed() {
    let app = TestApp::new().await;
    let alice = setup(&app, "alice").await;
    let bob = setup(&app, "bob").await;
    enable_rss(&app, &alice.user).await;
    enable_rss(&app, &bob.user).await;
    let alice_key = rss_key(&app, &alice.user).await;

    // The user id in the path is cosmetic; the key decides whose feed it is.
    let xml = feed(&app, bob.user.id, &alice_key).send().await.text();
    assert!(xml.contains("Alpha first alice"), "{xml}");
    assert!(!xml.contains("bob"), "bob's shows must not appear:\n{xml}");

    let resp = feed(&app, alice.user.id, &alice_key).query("podcast_id", bob.alpha).send().await;
    assert!(!resp.text().contains("bob"), "someone else's podcast id yields nothing of theirs: {}", resp.text());

    feed(&app, alice.user.id, "not-a-key").send().await.assert_status(401);
}

#[tokio::test(flavor = "multi_thread")]
async fn podcast_scoped_rss_keys_only_cover_their_podcasts() {
    let app = TestApp::new().await;
    let s = setup(&app, "scoped").await;
    let other = setup(&app, "other").await;
    enable_rss(&app, &s.user).await;

    let created = app
        .post("/api/data/create_api_key")
        .key(&s.user)
        .json(&json!({ "user_id": s.user.id, "rssonly": true, "podcast_ids": [s.alpha] }))
        .send()
        .await
        .ok_json();
    let scoped = created["rss_key"].as_str().unwrap_or_else(|| panic!("{created}")).to_string();

    let xml = feed(&app, s.user.id, &scoped).send().await.text();
    assert!(xml.contains("Alpha first scoped"), "{xml}");
    assert!(!xml.contains("Beta first scoped"), "a scoped key without podcast_id must stay in scope:\n{xml}");

    let out_of_scope = feed(&app, s.user.id, &scoped).query("podcast_id", s.beta).send().await;
    assert!(!out_of_scope.is_success(), "{}", out_of_scope.text());
    feed(&app, s.user.id, &scoped).query("source", "queue").send().await.assert_status(403);

    // The user's universal key is unaffected and still sees everything.
    let universal = rss_key(&app, &s.user).await;
    assert_ne!(universal, scoped);
    assert!(feed(&app, s.user.id, &universal).send().await.text().contains("Beta first scoped"));

    app.post("/api/data/create_api_key")
        .key(&s.user)
        .json(&json!({ "user_id": s.user.id, "rssonly": true, "podcast_ids": [other.alpha] }))
        .send()
        .await
        .assert_status(404);
}

#[tokio::test(flavor = "multi_thread")]
async fn rss_keys_only_stream_their_own_users_files() {
    let app = TestApp::new().await;
    let alice = setup(&app, "alice").await;
    let bob = setup(&app, "bob").await;
    let alice_key = rss_key(&app, &alice.user).await;
    let bob_ep = app.episode_id(&bob.user, bob.alpha, "Alpha first bob").await;
    let alice_ep = app.episode_id(&alice.user, alice.alpha, "Alpha first alice").await;

    let stream = |ep: i32, user_id: i32| {
        app.get(&format!("/api/data/stream/{ep}")).query("api_key", &alice_key).query("user_id", user_id)
    };
    stream(bob_ep, bob.user.id).send().await.assert_status(403);
    let own = stream(alice_ep, alice.user.id).send().await;
    assert!(!matches!(own.status.as_u16(), 401 | 403), "own episode is authorised (404 = not downloaded): {}", own.text());
}

#[tokio::test(flavor = "multi_thread")]
async fn collection_feeds_serve_saved_episodes_and_stay_per_user() {
    let app = TestApp::new().await;
    let alice = setup(&app, "alice").await;
    let bob = setup(&app, "bob").await;
    enable_rss(&app, &alice.user).await;
    let key = rss_key(&app, &alice.user).await;
    let ep = app.episode_id(&alice.user, alice.alpha, "Alpha second alice").await;
    app.post("/api/data/save_episode")
        .key(&alice.user)
        .json(&json!({ "episode_id": ep, "user_id": alice.user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();

    let saved = feed(&app, alice.user.id, &key).query("source", "saved").send().await.text();
    assert!(saved.contains("Alpha second alice"), "{saved}");
    assert!(!saved.contains("Alpha first alice"), "only saved episodes:\n{saved}");

    // Bob's collection id requested with alice's key must not serve bob's episodes.
    let bob_ep = app.episode_id(&bob.user, bob.alpha, "Alpha first bob").await;
    let c = app
        .post("/api/data/collections/create")
        .key(&bob.user)
        .json(&json!({ "user_id": bob.user.id, "name": "Bob's picks" }))
        .send()
        .await
        .ok_json()["collection_id"]
        .as_i64()
        .unwrap();
    app.post(&format!("/api/data/collections/{c}/add_episode"))
        .key(&bob.user)
        .json(&json!({ "user_id": bob.user.id, "episode_id": bob_ep, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    let resp = feed(&app, alice.user.id, &key).query("source", "collection").query("id", c).send().await;
    assert!(!resp.text().contains("bob"), "{}", resp.text());
}

#[tokio::test(flavor = "multi_thread")]
async fn share_links_are_public_until_deleted() {
    let app = TestApp::new().await;
    let s = setup(&app, "sharer").await;
    let other = app.user("other").await;
    let ep = app.episode_id(&s.user, s.alpha, "Alpha first sharer").await;

    let share = app.post(&format!("/api/data/share_episode/{ep}")).key(&s.user).send().await.ok_json();
    let code = share["url_key"].as_str().expect("url_key").to_string();

    // Anyone with the link can see the episode, no API key needed.
    let public = app.get(&format!("/api/data/episode_by_url/{code}")).send().await.ok_json();
    assert_eq!(public["episode"]["episodetitle"], "Alpha first sharer", "{public}");
    app.get("/api/data/episode_by_url/not-a-real-code").send().await.assert_status(404);

    let links = |u: &TestUser| app.get(&format!("/api/data/get_user_shared_links/{}", u.id)).key(u);
    let mine = links(&s.user).send().await.ok_json();
    let mine = mine["shared_links"].as_array().unwrap();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0]["share_code"], code);
    assert_eq!(mine[0]["episode_title"], "Alpha first sharer");
    assert_eq!(mine[0]["podcast_name"], "Alpha Show sharer");
    app.get(&format!("/api/data/get_user_shared_links/{}", s.user.id)).key(&other).send().await.assert_status(403);

    let extend = |u: &TestUser, days: i64| {
        app.put("/api/data/extend_shared_link").key(u).json(&json!({ "share_code": code, "days": days }))
    };
    extend(&s.user, 0).send().await.assert_status(400);
    extend(&other, 30).send().await.assert_status(404);
    extend(&s.user, 30).send().await.assert_ok();

    let delete = |u: &TestUser| app.delete("/api/data/delete_shared_link").key(u).json(&json!({ "share_code": code }));
    delete(&other).send().await.assert_status(404);
    app.get(&format!("/api/data/episode_by_url/{code}")).send().await.assert_ok();
    delete(&s.user).send().await.assert_ok();
    app.get(&format!("/api/data/episode_by_url/{code}")).send().await.assert_status(404);
}
