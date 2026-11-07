//! User and per-podcast settings: everything a settings page saves must read back the same.

use crate::harness::*;

async fn time_info(app: &TestApp, u: &TestUser) -> Value {
    app.get("/api/data/get_time_info").key(u).query("user_id", u.id).send().await.ok_json()
}

#[tokio::test(flavor = "multi_thread")]
async fn display_preferences_round_trip_and_stay_per_user() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let bob = app.user("bob").await;
    let theme = |u: &TestUser| app.get(&format!("/api/data/get_theme/{}", u.id)).key(u);
    let bob_theme = theme(&bob).send().await.ok_json();

    app.put("/api/data/user/set_theme")
        .key(&alice)
        .json(&json!({ "user_id": alice.id, "new_theme": "Nordic" }))
        .send()
        .await
        .assert_ok();
    assert_eq!(theme(&alice).send().await.ok_json()["theme"], "Nordic");
    assert_eq!(theme(&bob).send().await.ok_json(), bob_theme, "alice's theme must not leak to bob");

    app.post("/api/data/startpage")
        .key(&alice)
        .query("user_id", alice.id)
        .query("startpage", "queue")
        .send()
        .await
        .assert_ok();
    let sp = app.get("/api/data/startpage").key(&alice).query("user_id", alice.id).send().await.ok_json();
    assert_eq!(sp["StartPage"], "queue");

    app.post("/api/data/user/set_default_volume")
        .key(&alice)
        .json(&json!({ "user_id": alice.id, "volume": 35 }))
        .send()
        .await
        .assert_ok();
    let vol = app
        .post("/api/data/get_default_volume")
        .key(&alice)
        .json(&json!({ "user_id": alice.id }))
        .send()
        .await
        .ok_json();
    assert_eq!(vol["default_volume"], 35);
}

#[tokio::test(flavor = "multi_thread")]
async fn time_and_date_preferences_round_trip() {
    let app = TestApp::new().await;
    let user = app.user("clock").await;

    // First-run setup writes all three at once.
    app.post("/api/data/setup_time_info")
        .key(&user)
        .json(&json!({ "user_id": user.id, "timezone": "Europe/Berlin", "hour_pref": 24, "date_format": "DMY" }))
        .send()
        .await
        .assert_ok();
    let t = time_info(&app, &user).await;
    assert_eq!(t["timezone"], "Europe/Berlin", "{t}");
    assert_eq!(t["hour_pref"], 24);
    assert_eq!(t["date_format"], "DMY");

    // The settings page updates them one at a time.
    for (path, body) in [
        ("/api/data/update_timezone", json!({ "user_id": user.id, "timezone": "America/Chicago" })),
        ("/api/data/update_time_format", json!({ "user_id": user.id, "hour_pref": 12 })),
        ("/api/data/update_date_format", json!({ "user_id": user.id, "date_format": "MDY" })),
    ] {
        app.put(path).key(&user).json(&body).send().await.assert_ok();
    }
    let t = time_info(&app, &user).await;
    assert_eq!(t["timezone"], "America/Chicago", "{t}");
    assert_eq!(t["hour_pref"], 12);
    assert_eq!(t["date_format"], "MDY");
}

#[tokio::test(flavor = "multi_thread")]
async fn timezones_must_be_real_zone_names() {
    let app = TestApp::new().await;
    let user = app.user("clock").await;

    for tz in ["UTC'", "Z','D')||(SELECT current_user)||('", "Not/AZone"] {
        app.put("/api/data/update_timezone")
            .key(&user)
            .json(&json!({ "user_id": user.id, "timezone": tz }))
            .send()
            .await
            .assert_status(400);
        app.post("/api/data/setup_time_info")
            .key(&user)
            .json(&json!({ "user_id": user.id, "timezone": tz, "hour_pref": 24, "date_format": "DMY" }))
            .send()
            .await
            .assert_status(400);
    }
    assert_ne!(time_info(&app, &user).await["timezone"], "UTC'");
}

#[tokio::test(flavor = "multi_thread")]
async fn language_preference_round_trips() {
    let app = TestApp::new().await;
    let user = app.user("polyglot").await;
    let current = || async { app.get("/api/data/get_user_language").key(&user).query("user_id", user.id).send().await.ok_json()["language"].clone() };
    assert_eq!(current().await, "en", "new users default to English");
    app.put("/api/data/update_user_language")
        .key(&user)
        .json(&json!({ "user_id": user.id, "language": "de" }))
        .send()
        .await
        .assert_ok();
    assert_eq!(current().await, "de");
}

#[tokio::test(flavor = "multi_thread")]
async fn per_podcast_toggles_round_trip() {
    let app = TestApp::new().await;
    let user = app.user("tuner").await;
    let url = app.feed("tuned.xml", &Feed::with_episodes("Tuned", 1));
    let pod = app.subscribe(&user, &url, 1).await;
    let ids = json!({ "user_id": user.id, "podcast_id": pod });
    let read = |path: &'static str, field: &'static str| {
        let ids = ids.clone();
        let app = &app;
        let user = &user;
        async move { app.post(path).key(user).json(&ids).send().await.ok_json()[field].clone() }
    };

    assert_eq!(read("/api/data/get_auto_download_status", "auto_download").await, false);
    assert_eq!(read("/api/data/get_auto_queue_status", "auto_queue").await, false);
    assert_eq!(read("/api/data/podcast/notification_status", "enabled").await, false);

    for on in [true, false] {
        app.post("/api/data/enable_auto_download")
            .key(&user)
            .json(&json!({ "user_id": user.id, "podcast_id": pod, "auto_download": on }))
            .send()
            .await
            .assert_ok();
        app.post("/api/data/enable_auto_queue")
            .key(&user)
            .json(&json!({ "user_id": user.id, "podcast_id": pod, "auto_queue": on }))
            .send()
            .await
            .assert_ok();
        app.put("/api/data/podcast/toggle_notifications")
            .key(&user)
            .json(&json!({ "user_id": user.id, "podcast_id": pod, "enabled": on }))
            .send()
            .await
            .assert_ok();
        assert_eq!(read("/api/data/get_auto_download_status", "auto_download").await, on);
        assert_eq!(read("/api/data/get_auto_queue_status", "auto_queue").await, on);
        assert_eq!(read("/api/data/podcast/notification_status", "enabled").await, on);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn notification_settings_round_trip_per_platform() {
    let app = TestApp::new().await;
    let user = app.user("notified").await;
    app.put("/api/data/user/notification_settings")
        .key(&user)
        .json(&json!({
            "user_id": user.id,
            "platform": "ntfy",
            "enabled": true,
            "ntfy_topic": "pinepods-test",
            "ntfy_server_url": "https://ntfy.example.invalid",
        }))
        .send()
        .await
        .assert_ok();
    app.put("/api/data/user/notification_settings")
        .key(&user)
        .json(&json!({
            "user_id": user.id,
            "platform": "gotify",
            "enabled": false,
            "gotify_url": "https://gotify.example.invalid",
            "gotify_token": "secret-token",
        }))
        .send()
        .await
        .assert_ok();

    let body = app.get("/api/data/user/notification_settings").key(&user).query("user_id", user.id).send().await.ok_json();
    let settings = body["settings"].as_array().unwrap_or_else(|| panic!("{body}"));
    let platform = |name: &str| {
        settings.iter().find(|s| s["platform"] == name).unwrap_or_else(|| panic!("no {name} row: {body}")).clone()
    };
    let ntfy = platform("ntfy");
    assert_eq!(ntfy["enabled"], true);
    assert_eq!(ntfy["ntfy_topic"], "pinepods-test");
    assert_eq!(ntfy["ntfy_server_url"], "https://ntfy.example.invalid");
    let gotify = platform("gotify");
    assert_eq!(gotify["enabled"], false);
    assert_eq!(gotify["gotify_url"], "https://gotify.example.invalid");
}

#[tokio::test(flavor = "multi_thread")]
async fn http_notifications_announce_new_episodes() {
    let app = TestApp::new().await;
    let user = app.user("hooked").await;
    let feed = Feed::with_episodes("Hooked Show", 1);
    let url = app.feed("hooked.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    let hook = app.fixtures().serve("hook", Fixture::new("application/json", "{}"));
    app.put("/api/data/user/notification_settings")
        .key(&user)
        .json(&json!({
            "user_id": user.id,
            "platform": "http",
            "enabled": true,
            "http_url": hook,
            "http_token": "hook-token",
            "http_method": "POST",
        }))
        .send()
        .await
        .assert_ok();
    app.put("/api/data/podcast/toggle_notifications")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod, "enabled": true }))
        .send()
        .await
        .assert_ok();

    app.feed("hooked.xml", &feed.episode(Episode::new(2).title("Fresh hooked episode")));
    app.refresh().await;

    let hits = app.fixtures().hits("hook");
    assert_eq!(hits.len(), 1, "one notification per new episode: {hits:?}");
    assert_eq!(hits[0].method, "POST");
    assert_eq!(hits[0].headers.get("authorization").map(String::as_str), Some("Bearer hook-token"));
    let body: Value = serde_json::from_slice(&hits[0].body).expect("json body");
    let message = body["message"].as_str().unwrap_or_default();
    assert!(message.contains("Hooked Show") && message.contains("Fresh hooked episode"), "{body}");
}

/// Notification settings for one platform, plus notifications on for the podcast at `pod`.
async fn notify_on(app: &TestApp, user: &TestUser, pod: i32, settings: Value) {
    let mut body = settings;
    body["user_id"] = json!(user.id);
    body["enabled"] = json!(true);
    app.put("/api/data/user/notification_settings").key(user).json(&body).send().await.assert_ok();
    app.put("/api/data/podcast/toggle_notifications")
        .key(user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod, "enabled": true }))
        .send()
        .await
        .assert_ok();
}

async fn send_test_notification(app: &TestApp, user: &TestUser, platform: &str) -> u16 {
    app.post("/api/data/user/test_notification")
        .key(user)
        .json(&json!({ "user_id": user.id, "platform": platform }))
        .send()
        .await
        .status
        .as_u16()
}

#[tokio::test(flavor = "multi_thread")]
async fn ntfy_announces_new_episodes_with_the_access_token() {
    let app = TestApp::new().await;
    let user = app.user("ntfy").await;
    let feed = Feed::with_episodes("Ntfy Show", 1);
    let url = app.feed("show.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;
    app.fixtures().serve("ntfy/alerts", Fixture::new("application/json", "{}"));
    notify_on(&app, &user, pod, json!({
        "platform": "ntfy",
        "ntfy_topic": "alerts",
        "ntfy_server_url": app.fixtures().url("ntfy"),
        "ntfy_access_token": "ntfy-token",
    }))
    .await;

    app.feed("show.xml", &feed.episode(Episode::new(2).title("Fresh ntfy episode")));
    app.refresh().await;

    let hits = app.fixtures().hits("ntfy/alerts");
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert_eq!(hits[0].headers.get("authorization").map(String::as_str), Some("Bearer ntfy-token"));
    let message = String::from_utf8_lossy(&hits[0].body);
    assert!(message.contains("Ntfy Show") && message.contains("Fresh ntfy episode"), "{message}");
}

#[tokio::test(flavor = "multi_thread")]
async fn ntfy_can_sign_in_with_a_username_and_password() {
    let app = TestApp::new().await;
    let user = app.user("ntfy").await;
    let pod = app.subscribe(&user, &app.feed("show.xml", &Feed::with_episodes("Show", 1)), 1).await;
    app.fixtures().serve("ntfy/alerts", Fixture::new("application/json", "{}"));
    notify_on(&app, &user, pod, json!({
        "platform": "ntfy",
        "ntfy_topic": "alerts",
        "ntfy_server_url": app.fixtures().url("ntfy"),
        "ntfy_username": "listener",
        "ntfy_password": "ntfy-pass",
    }))
    .await;

    assert_eq!(send_test_notification(&app, &user, "ntfy").await, 200);

    let hits = app.fixtures().hits("ntfy/alerts");
    assert_eq!(hits.len(), 1, "{hits:?}");
    let expected = pinepods_basic("listener", "ntfy-pass");
    assert_eq!(hits[0].headers.get("authorization"), Some(&expected));
    assert_eq!(String::from_utf8_lossy(&hits[0].body), "Test notification from PinePods");
}

fn pinepods_basic(user: &str, password: &str) -> String {
    crate::harness::app::basic(user, password)
}

#[tokio::test(flavor = "multi_thread")]
async fn gotify_announces_new_episodes_and_sends_test_messages() {
    let app = TestApp::new().await;
    let user = app.user("gotify").await;
    let feed = Feed::with_episodes("Gotify Show", 1);
    let url = app.feed("show.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;
    app.fixtures().serve("gotify/message", Fixture::new("application/json", "{}"));
    notify_on(&app, &user, pod, json!({
        "platform": "gotify",
        "gotify_url": app.fixtures().url("gotify"),
        "gotify_token": "gotify-token",
    }))
    .await;

    assert_eq!(send_test_notification(&app, &user, "gotify").await, 200);
    app.feed("show.xml", &feed.episode(Episode::new(2).title("Fresh gotify episode")));
    app.refresh().await;

    let hits = app.fixtures().hits("gotify/message");
    assert_eq!(hits.len(), 2, "{hits:?}");
    for hit in &hits {
        assert!(hit.query.as_deref().is_some_and(|q| q.contains("token=gotify-token")), "{hit:?}");
    }
    let test: Value = serde_json::from_slice(&hits[0].body).expect("json body");
    assert_eq!(test["title"], "PinePods Test");
    let announced: Value = serde_json::from_slice(&hits[1].body).expect("json body");
    let message = announced["message"].as_str().unwrap_or_default();
    assert!(message.contains("Fresh gotify episode"), "{announced}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_notification_service_is_reported_and_does_not_stop_the_refresh() {
    let app = TestApp::new().await;
    let user = app.user("unlucky").await;
    let feed = Feed::with_episodes("Show", 1);
    let url = app.feed("show.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;
    app.fixtures().serve("ntfy/alerts", Fixture::new("text/plain", "nope").status(500));
    notify_on(&app, &user, pod, json!({
        "platform": "ntfy",
        "ntfy_topic": "alerts",
        "ntfy_server_url": app.fixtures().url("ntfy"),
    }))
    .await;

    assert_eq!(send_test_notification(&app, &user, "ntfy").await, 400);
    app.feed("show.xml", &feed.episode(Episode::new(2)));
    app.refresh().await;
    assert_eq!(app.episodes(&user, pod).await.len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn notifications_stay_quiet_for_podcasts_that_did_not_ask() {
    let app = TestApp::new().await;
    let user = app.user("quiet").await;
    let feed = Feed::with_episodes("Quiet Show", 1);
    let url = app.feed("show.xml", &feed);
    app.subscribe(&user, &url, 1).await;
    app.fixtures().serve("ntfy/alerts", Fixture::new("application/json", "{}"));
    app.put("/api/data/user/notification_settings")
        .key(&user)
        .json(&json!({
            "user_id": user.id,
            "platform": "ntfy",
            "enabled": true,
            "ntfy_topic": "alerts",
            "ntfy_server_url": app.fixtures().url("ntfy"),
        }))
        .send()
        .await
        .assert_ok();

    app.feed("show.xml", &feed.episode(Episode::new(2)));
    app.refresh().await;

    assert!(app.fixtures().hits("ntfy/alerts").is_empty());
}

fn theme_body(user_id: i32, name: &str) -> Value {
    let mut body = json!({ "user_id": user_id, "name": name });
    for field in [
        "background_color", "button_color", "container_button_color", "button_text_color", "text_color",
        "text_secondary_color", "border_color", "accent_color", "prog_bar_color", "error_color", "bonus_color",
        "secondary_background", "container_background", "standout_color", "hover_color", "link_color",
        "thumb_color", "unfilled_color", "check_box_color",
    ] {
        body[field] = json!("#123456");
    }
    body
}

#[tokio::test(flavor = "multi_thread")]
async fn custom_themes_can_be_created_listed_and_deleted_by_their_owner() {
    let app = TestApp::new().await;
    let user = app.user("designer").await;
    let other = app.user("other").await;
    let list = |u: &TestUser| {
        let app = &app;
        let u = u.clone();
        async move {
            app.get(&format!("/api/data/user/custom_themes/{}", u.id)).key(&u).send().await.ok_json()["themes"]
                .as_array()
                .cloned()
                .unwrap_or_default()
        }
    };

    let created = app.post("/api/data/user/custom_themes").key(&user).json(&theme_body(user.id, "Midnight")).send().await.ok_json();
    let themes = list(&user).await;
    assert_eq!(themes.len(), 1, "{themes:#?} / created {created}");
    assert_eq!(themes[0]["name"], "Midnight");
    assert_eq!(themes[0]["accent_color"], "#123456");
    assert!(list(&other).await.is_empty());
    let theme_id = themes[0]["themeid"].as_i64().unwrap_or_else(|| panic!("theme_id: {:#?}", themes[0]));

    app.delete("/api/data/user/custom_themes")
        .key(&other)
        .json(&json!({ "user_id": other.id, "theme_id": theme_id }))
        .send()
        .await;
    assert_eq!(list(&user).await.len(), 1, "another user cannot delete it");

    app.delete("/api/data/user/custom_themes")
        .key(&user)
        .json(&json!({ "user_id": user.id, "theme_id": theme_id }))
        .send()
        .await
        .assert_ok();
    assert!(list(&user).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn gravatar_is_off_until_the_user_turns_it_on() {
    let app = TestApp::new().await;
    let user = app.user("avatar").await;
    let details = || async { app.get(&format!("/api/data/user_details_id/{}", user.id)).key(&user).send().await.ok_json() };
    assert_eq!(details().await["UseGravatar"], false);

    app.post("/api/data/user/set_use_gravatar")
        .key(&user)
        .json(&json!({ "user_id": user.id, "enabled": true }))
        .send()
        .await
        .assert_ok();
    assert_eq!(details().await["UseGravatar"], true);
}
