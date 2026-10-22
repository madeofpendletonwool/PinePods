//! The Pinepods gpodder server beyond the protocol suite: authentication, upload cursors, PUT
//! diffs, and turning the server on from the settings page.

use std::time::Duration;

use chrono::Utc;

use crate::gpodder::{actions_for, strings};
use crate::harness::gpodder::*;
use crate::harness::*;

const POD: &str = "https://feeds.example.test/show.xml";

fn ep(n: usize) -> String {
    format!("https://cdn.example.test/show/ep{n}.mp3")
}

async fn scene() -> GpodderScene {
    let app = TestApp::new().await;
    let user = app.user("srv").await;
    let server = app.rust_gpodder_server().await;
    app.enable_gpodder(&server, &user).await;
    GpodderScene { server, app, user }
}

/// A client that never logs in and sends HTTP Basic with every request.
fn basic_client(s: &GpodderScene, password: &str) -> (GpodderClient, Option<String>) {
    let c = GpodderClient::new(&s.server.base_url, &s.user.username, password, "basic-only");
    let auth = Some(GpodderClient::basic_header(&s.user.username, password));
    (c, auth)
}

async fn devices_with(s: &GpodderScene, password: &str) -> GResp {
    let (c, auth) = basic_client(s, password);
    c.send(reqwest::Method::GET, &format!("/api/2/devices/{}.json", s.user.username), None, auth).await
}

async fn session_count(s: &GpodderScene) -> i64 {
    int(&s.app, &format!(r#"SELECT COUNT(*) FROM "GpodderSessions" WHERE UserID = {}"#, s.user.id)).await.unwrap()
}

// ---- authentication -----------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn app_passwords_and_the_user_password_both_log_in() {
    let s = scene().await;
    let uid = s.user.id;
    exec(&s.app, &format!(
        r#"INSERT INTO "GpodderAppPasswords" (UserID, Label, PasswordHash) VALUES ({uid}, 'phone', '{}')"#,
        hash_password("app-secret-1")
    ))
    .await;

    devices_with(&s, "app-secret-1").await.assert_ok();
    let used = int(&s.app, &format!(r#"SELECT COUNT(*) FROM "GpodderAppPasswords" WHERE UserID = {uid} AND LastUsedAt IS NOT NULL"#)).await;
    assert_eq!(used, Some(1), "LastUsedAt is recorded");
    devices_with(&s, &s.user.password).await.assert_ok();

    let bad = devices_with(&s, "app-secret-2").await;
    bad.assert_status(401);
    bad.json(); // one JSON document
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rejected_request_is_one_401_with_a_basic_challenge() {
    let s = scene().await;
    let resp = reqwest::Client::builder()
        .no_proxy()
        .build()
        .unwrap()
        .get(format!("{}/api/2/episodes/{}.json", s.server.base_url, s.user.username))
        .header("Authorization", GpodderClient::basic_header(&s.user.username, "nope"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 401);
    assert!(resp.headers().get("www-authenticate").is_some_and(|v| v.to_str().unwrap().starts_with("Basic")));
    let body: Value = serde_json::from_str(&resp.text().await.unwrap()).expect("one JSON document");
    assert_eq!(body, json!({ "error": "unauthorized" }));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_session_cookie_authenticates_and_login_is_idempotent() {
    let s = scene().await;
    let phone = s.device("phone").await; // logs in with Basic, keeps the cookie
    assert_eq!(session_count(&s).await, 1);
    phone.login().await.assert_ok();
    assert_eq!(session_count(&s).await, 1, "logging in again with a valid cookie keeps the session");

    // The cookie alone is enough.
    phone.episode_actions(0).await.assert_ok();

    phone.post(&format!("/api/2/auth/{}/logout.json", s.user.username), &json!({})).await.assert_ok();
    assert_eq!(session_count(&s).await, 0);
    phone.episode_actions(0).await.assert_status(401);
}

#[tokio::test(flavor = "multi_thread")]
async fn verified_basic_credentials_are_cached() {
    let s = scene().await;
    let old = s.user.password.clone();
    devices_with(&s, &old).await.assert_ok();

    // With the stored hash replaced, the old password still works from the cache (it expires
    // after 10 minutes) and the new one is verified against the database.
    exec(&s.app, &format!(r#"UPDATE "Users" SET Hashed_PW = '{}' WHERE UserID = {}"#, hash_password("changed-pw"), s.user.id)).await;
    devices_with(&s, &old).await.assert_ok();
    devices_with(&s, "changed-pw").await.assert_ok();
    devices_with(&s, "neither").await.assert_status(401);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_user_without_the_server_enabled_is_refused() {
    let s = scene().await;
    let phone = s.device("phone").await;
    s.app.post("/api/data/gpodder/toggle").key(&s.user).json(&json!({ "enabled": false })).send().await.assert_ok();

    phone.episode_actions(0).await.assert_status(401);
    devices_with(&s, &s.user.password).await.assert_status(401);
    phone.login().await.assert_status(401);
}

// ---- upload cursors -----------------------------------------------------------------------

fn cursor(body: &Value) -> i64 {
    body["timestamp"].as_i64().unwrap_or_else(|| panic!("no integer timestamp: {body}"))
}

/// An episode upload returns the cursor that session was last given by a download, so each
/// session gets its own, and a client without a session gets the request time.
#[tokio::test(flavor = "multi_thread")]
async fn episode_upload_returns_this_sessions_download_cursor() {
    let s = scene().await;
    let phone = s.device("phone").await;
    let tablet = s.device("tablet").await;

    let c_phone = cursor(&phone.episode_actions(0).await.ok_json());
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let c_tablet = cursor(&tablet.episode_actions(0).await.ok_json());
    assert!(c_tablet > c_phone);

    let up = phone.upload_episode_actions(&[play_action(POD, &ep(1), Utc::now(), 0, 60, 600)]).await;
    assert_eq!(cursor(&up[0].json()), c_phone);
    let up = tablet.upload_episode_actions(&[play_action(POD, &ep(2), Utc::now(), 0, 60, 600)]).await;
    assert_eq!(cursor(&up[0].json()), c_tablet);

    let (c, auth) = basic_client(&s, &s.user.password);
    let r = c
        .send(reqwest::Method::POST, &format!("/api/2/episodes/{}.json", s.user.username), Some(&json!([play_action(POD, &ep(3), Utc::now(), 0, 60, 600)])), auth)
        .await;
    assert!(cursor(&r.ok_json()) >= c_tablet, "no session: request time");
}

/// A subscription upload returns the cursor that device was last given by a download.
#[tokio::test(flavor = "multi_thread")]
async fn subscription_upload_returns_this_devices_download_cursor() {
    let s = scene().await;
    let phone = s.device("phone").await;
    let tablet = s.device("tablet").await;
    let c_phone = cursor(&phone.subscription_changes(0).await.ok_json());
    tokio::time::sleep(Duration::from_millis(2100)).await;
    let c_tablet = cursor(&tablet.subscription_changes(0).await.ok_json());
    assert!(c_tablet > c_phone);

    let a = s.app.fixtures().url("cursor-a.xml");
    let b = s.app.fixtures().url("cursor-b.xml");
    assert_eq!(cursor(&phone.upload_subscription_changes(&[&a], &[]).await.ok_json()), c_phone);
    assert_eq!(cursor(&tablet.upload_subscription_changes(&[&b], &[]).await.ok_json()), c_tablet);

    let car = s.device("car").await; // never downloaded
    let c = cursor(&car.upload_subscription_changes(&[], &[]).await.ok_json());
    assert!(c >= c_tablet, "a device that never downloaded gets the request time");
}

// ---- PUT ----------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn put_diffs_against_what_that_device_reported() {
    let s = scene().await;
    let a = s.app.feed("put-a.xml", &Feed::with_episodes("Put A", 1));
    let b = s.app.feed("put-b.xml", &Feed::with_episodes("Put B", 1));
    let phone = s.device("phone").await;
    let u = s.user.username.clone();
    let feeds = || async { s.app.podcasts(&s.user).await.iter().filter_map(|p| p["feedurl"].as_str().map(str::to_string)).collect::<Vec<_>>() };

    phone.put(&format!("/api/2/subscriptions/{u}/phone.json"), &json!([a, b])).await.assert_ok();
    eventually("both feeds imported", || async {
        let f = feeds().await;
        (f.contains(&a) && f.contains(&b)).then_some(())
    })
    .await;
    let laptop = s.device("laptop").await;
    let c = cursor(&laptop.subscription_changes(0).await.ok_json());

    // The phone drops A: only A is removed.
    phone.put(&format!("/api/2/subscriptions/{u}/phone.json"), &json!([b])).await.assert_ok();
    eventually("A removed", || async { (!feeds().await.contains(&a)).then_some(()) }).await;
    assert!(feeds().await.contains(&b));
    let changes = laptop.subscription_changes(c).await.ok_json();
    assert!(strings(&changes, "remove").contains(&a), "{changes}");

    // A device that never reported B can't remove it by sending an empty list.
    s.device("tablet").await.put(&format!("/api/2/subscriptions/{u}/tablet.json"), &json!([])).await.assert_ok();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(feeds().await.contains(&b), "another device's empty PUT removed B");

    let phone_id = int(&s.app, &format!(r#"SELECT DeviceID FROM "GpodderDevices" WHERE UserID = {} AND DeviceName = 'phone'"#, s.user.id)).await.unwrap();
    let listed = text(&s.app, &format!(r#"SELECT MIN(PodcastURL) FROM "GpodderDeviceSubscriptions" WHERE DeviceID = {phone_id}"#)).await;
    let count = int(&s.app, &format!(r#"SELECT COUNT(*) FROM "GpodderDeviceSubscriptions" WHERE DeviceID = {phone_id}"#)).await;
    assert_eq!((listed, count), (Some(b.clone()), Some(1)), "the phone's list is [B]");
}

// ---- settings page ------------------------------------------------------------------------

/// Turning the server on in Settings (the old toggle endpoint) needs nothing else: clients log in
/// with the user's password and their first sync gets the library and the listening state.
#[tokio::test(flavor = "multi_thread")]
async fn enabling_the_server_from_settings_publishes_the_library_and_progress() {
    let app = TestApp::new().await;
    let user = app.user("enable").await;
    let feed_url = app.feed("enable.xml", &Feed::with_episodes("Enable", 2));
    let pod = app.subscribe(&user, &feed_url, 2).await;
    let e1 = app.episode_id(&user, pod, "Episode 1").await;
    app.post("/api/data/record_listen_duration")
        .key(&user)
        .json(&json!({ "episode_id": e1, "user_id": user.id, "listen_duration": 120.0 }))
        .send()
        .await
        .assert_ok();
    let e1_url = text(&app, &format!(r#"SELECT EpisodeURL FROM "Episodes" WHERE EpisodeID = {e1}"#)).await.unwrap();

    let server = app.rust_gpodder_server().await;
    let resp = app.post("/api/data/gpodder/toggle").key(&user).json(&json!({ "enabled": true })).send().await.ok_json();
    assert_eq!(resp["gpodder_enabled"], true, "{resp}");
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderDevices" WHERE UserID = {} AND SystemRole = 'pinepods'"#, user.id)).await, Some(1));

    let s = GpodderScene { server, app, user };
    let phone = s.device("phone").await;
    let subs = phone.subscription_changes(0).await.ok_json();
    assert!(strings(&subs, "add").contains(&feed_url), "{subs}");
    let eps = phone.episode_actions(0).await.ok_json();
    let got = actions_for(&eps, &e1_url);
    assert_eq!(got.len(), 1, "{eps}");
    assert_eq!(got[0]["position"], 120, "{}", got[0]);

    // Enabling again doesn't publish the same state twice.
    s.app.post("/api/data/gpodder/toggle").key(&s.user).json(&json!({ "enabled": true })).send().await.assert_ok();
    assert_eq!(actions_for(&phone.episode_actions(0).await.ok_json(), &e1_url).len(), 1);
}
