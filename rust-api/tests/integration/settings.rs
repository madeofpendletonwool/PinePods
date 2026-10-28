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
