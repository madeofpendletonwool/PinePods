//! The sync settings API (/api/sync/*): the PinePods gpodder server with its app passwords and
//! devices, the external server's test/save/status/run/remove, statistics, and the old
//! /api/gpodder routes answering in their old shapes on top of it.

use pinepods_api::services::sync::engine_db::TargetKind;

use crate::harness::gpodder::GpodderClient;
use crate::harness::mock_gpodder::MockGpodder;
use crate::harness::*;

const REMOTE_USER: &str = "remote";
const REMOTE_PASS: &str = "remote-secret-pw";

async fn sync_type(app: &TestApp, user: &TestUser) -> String {
    text(app, &format!(r#"SELECT Pod_Sync_Type FROM "Users" WHERE UserID = {}"#, user.id)).await.unwrap_or_default()
}

fn creds(kind: &str, mock: &MockGpodder, password: Option<&str>) -> Value {
    json!({ "kind": kind, "url": mock.base_url, "username": REMOTE_USER, "password": password })
}

#[tokio::test(flavor = "multi_thread")]
async fn server_toggle_and_app_passwords() {
    let app = TestApp::new().await;
    let user = app.user("appsync").await;
    let other = app.user("appsync-other").await;
    let server = app.rust_gpodder_server().await;
    let feed = app.feed("server.xml", &Feed::with_episodes("Server Show", 1));
    app.subscribe(&user, &feed, 1).await;

    let info = app.get("/api/sync/server").key(&user).send().await.ok_json();
    assert_eq!((info["enabled"].as_bool(), info["username"].as_str()), (Some(false), Some(user.username.as_str())));
    assert!(info["server_url"].as_str().is_some_and(|u| u.starts_with("http")), "{info}");
    assert_eq!(info["notice"], Value::Null);

    let on = app.put("/api/sync/server").key(&user).json(&json!({ "enabled": true })).send().await.ok_json();
    assert_eq!(on["enabled"], true);
    assert_eq!(sync_type(&app, &user).await, "gpodder");

    // Created once: the password is only in the creation response.
    let created = app.post("/api/sync/server/app-passwords").key(&user).json(&json!({ "label": "Phone" })).send().await.ok_json();
    let password = created["password"].as_str().unwrap().to_string();
    let id = created["id"].as_i64().unwrap();
    assert_eq!(created["label"], "Phone");
    let list = app.get("/api/sync/server/app-passwords").key(&user).send().await;
    assert!(!list.text().contains(&password) && !list.text().contains("argon2"), "{}", list.text());
    let list = list.ok_json();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!((list[0]["id"].as_i64(), list[0]["last_used_at"].clone()), (Some(id), Value::Null));

    // It works for a gpodder app, and its use is recorded.
    let phone = GpodderClient::new(&server.base_url, &user.username, &password, "phone");
    phone.login().await.assert_ok();
    phone.subscription_changes(0).await.assert_ok();
    let list = app.get("/api/sync/server/app-passwords").key(&user).send().await.ok_json();
    assert!(list[0]["last_used_at"].as_i64().is_some(), "{list}");

    // Other users can't see or revoke it.
    let theirs = app.get("/api/sync/server/app-passwords").key(&other).send().await.ok_json();
    assert_eq!(theirs, json!([]));
    app.delete(&format!("/api/sync/server/app-passwords/{id}")).key(&other).send().await.assert_status(404);
    GpodderClient::new(&server.base_url, &user.username, &password, "tablet").login().await.assert_ok();

    // Revoked: Basic logins and the session it opened stop working at once.
    app.delete(&format!("/api/sync/server/app-passwords/{id}")).key(&user).send().await.assert_ok();
    GpodderClient::new(&server.base_url, &user.username, &password, "tablet").login().await.assert_status(401);
    phone.subscription_changes(0).await.assert_status(401);
    assert_eq!(app.get("/api/sync/server/app-passwords").key(&user).send().await.ok_json(), json!([]));

    // Off again: the user's own password stops working for apps too.
    let off = app.put("/api/sync/server").key(&user).json(&json!({ "enabled": false })).send().await.ok_json();
    assert_eq!(off["enabled"], false);
    assert_eq!(sync_type(&app, &user).await, "None");
    GpodderClient::new(&server.base_url, &user.username, &user.password, "laptop").login().await.assert_status(401);
}

#[tokio::test(flavor = "multi_thread")]
async fn devices_can_be_renamed_and_removed_except_pinepods_own() {
    let app = TestApp::new().await;
    let user = app.user("devices").await;
    let other = app.user("devices-other").await;
    let server = app.rust_gpodder_server().await;
    app.put("/api/sync/server").key(&user).json(&json!({ "enabled": true })).send().await.assert_ok();
    let phone = GpodderClient::new(&server.base_url, &user.username, &user.password, "phone");
    phone.login().await.assert_ok();
    phone.subscription_changes(0).await.assert_ok();

    let devices = app.get("/api/sync/devices").key(&user).send().await.ok_json();
    let list = devices.as_array().unwrap();
    let own = list.iter().find(|d| d["protected"] == true).unwrap_or_else(|| panic!("Pinepods' device in {devices}"));
    let phone_dev = list.iter().find(|d| d["name"] == "phone").unwrap_or_else(|| panic!("phone in {devices}"));
    assert!(phone_dev["last_seen"].as_i64().is_some(), "{devices}");
    let (own_id, phone_id) = (own["id"].as_i64().unwrap(), phone_dev["id"].as_i64().unwrap());

    // Pinepods' own device is shown but can't be renamed or removed.
    app.put(&format!("/api/sync/devices/{own_id}")).key(&user).json(&json!({ "caption": "Mine" })).send().await.assert_status(403);
    app.delete(&format!("/api/sync/devices/{own_id}")).key(&user).send().await.assert_status(403);

    // Someone else's device doesn't exist for this user.
    app.put(&format!("/api/sync/devices/{phone_id}")).key(&other).json(&json!({ "caption": "Hijacked" })).send().await.assert_status(404);
    app.delete(&format!("/api/sync/devices/{phone_id}")).key(&other).send().await.assert_status(404);
    assert_eq!(app.get("/api/sync/devices").key(&other).send().await.ok_json(), json!([]));

    app.put(&format!("/api/sync/devices/{phone_id}")).key(&user).json(&json!({ "caption": "  " })).send().await.assert_status(400);
    app.put(&format!("/api/sync/devices/{phone_id}")).key(&user).json(&json!({ "caption": "My phone" })).send().await.assert_ok();
    let devices = app.get("/api/sync/devices").key(&user).send().await.ok_json();
    assert!(devices.as_array().unwrap().iter().any(|d| d["id"].as_i64() == Some(phone_id) && d["caption"] == "My phone"), "{devices}");

    app.delete(&format!("/api/sync/devices/{phone_id}")).key(&user).send().await.assert_ok();
    let devices = app.get("/api/sync/devices").key(&user).send().await.ok_json();
    assert_eq!(devices.as_array().unwrap().len(), 1, "{devices}");
    assert_eq!(devices[0]["protected"], true);

    // The app gets a new device the next time it syncs.
    phone.subscription_changes(0).await.assert_ok();
    let devices = app.get("/api/sync/devices").key(&user).send().await.ok_json();
    assert!(devices.as_array().unwrap().iter().any(|d| d["name"] == "phone"), "{devices}");

    let stats = app.get("/api/sync/stats").key(&user).send().await.ok_json();
    assert_eq!((stats["server_enabled"].as_bool(), stats["devices"].as_i64()), (Some(true), Some(1)), "{stats}");
}

#[tokio::test(flavor = "multi_thread")]
async fn testing_a_connection_saves_nothing() {
    let app = TestApp::new().await;
    let user = app.user("tester").await;
    let mock = MockGpodder::start().await;
    mock.add_user(REMOTE_USER, REMOTE_PASS);

    let bad = app.post("/api/sync/target/test").key(&user).json(&creds("gpodder", &mock, Some("wrong"))).send().await.ok_json();
    assert_eq!(bad["success"], false, "{bad}");
    assert!(!bad["message"].as_str().unwrap().contains("wrong"), "{bad}");

    let ok = app.post("/api/sync/target/test").key(&user).json(&creds("gpodder", &mock, Some(REMOTE_PASS))).send().await.ok_json();
    assert_eq!((ok["success"].as_bool(), ok["device_name"].as_str()), (Some(true), Some("pinepods")), "{ok}");
    assert!(ok["devices"].is_array());

    let nc = app.post("/api/sync/target/test").key(&user).json(&creds("nextcloud", &mock, Some(REMOTE_PASS))).send().await.ok_json();
    assert_eq!(nc["success"], true, "{nc}");

    // No password and nothing stored to reuse.
    app.post("/api/sync/target/test").key(&user).json(&creds("gpodder", &mock, None)).send().await.assert_status(400);
    app.post("/api/sync/target/test").key(&user).json(&json!({ "kind": "ftp", "url": mock.base_url, "username": "x", "password": "y" })).send().await.assert_status(400);

    assert!(app.db().sync_targets_for_user(user.id).await.unwrap().is_empty());
    assert_eq!(sync_type(&app, &user).await, "None");
    assert!(mock.requests().iter().all(|r| r.method == "GET" || r.path.contains("/auth/")), "only logins and reads: {:?}", mock.requests().iter().map(|r| format!("{} {}", r.method, r.path)).collect::<Vec<_>>());
    assert_eq!(app.get("/api/sync/target").key(&user).send().await.ok_json()["target"], Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn target_setup_status_runs_and_removal() {
    let app = TestApp::new().await;
    let user = app.user("targeted").await;
    let other = app.user("targeted-other").await;
    let mock = MockGpodder::start().await;
    mock.add_user(REMOTE_USER, REMOTE_PASS);
    let feed = app.feed("target.xml", &Feed::with_episodes("Target Show", 1));
    app.subscribe(&user, &feed, 1).await;
    exec(&app, &format!(r#"UPDATE "Users" SET SyncNotice = 'reenter_external' WHERE UserID = {}"#, user.id)).await;
    assert_eq!(app.get("/api/sync/server").key(&user).send().await.ok_json()["notice"], "reenter_external");

    // Wrong credentials are refused and nothing is saved.
    app.put("/api/sync/target").key(&user).json(&creds("gpodder", &mock, Some("wrong"))).send().await.assert_status(400);
    assert!(app.db().sync_targets_for_user(user.id).await.unwrap().is_empty());

    // Saving starts the first sync as a task and clears the notice.
    let saved = app.put("/api/sync/target").key(&user).json(&creds("gpodder", &mock, Some(REMOTE_PASS))).send().await;
    assert!(!saved.text().contains(REMOTE_PASS), "{}", saved.text());
    let saved = saved.ok_json();
    assert_eq!((saved["target"]["kind"].as_str(), saved["target"]["needs_full_sync"].as_bool()), (Some("gpodder"), Some(true)));
    assert!(saved["task_id"].is_string(), "{saved}");
    assert_eq!(app.get("/api/sync/server").key(&user).send().await.ok_json()["notice"], Value::Null);
    assert_eq!(sync_type(&app, &user).await, "external");
    let synced = eventually("first sync", || async {
        let t = app.get("/api/sync/target").key(&user).send().await.ok_json()["target"].clone();
        t["last_success_at"].as_i64().is_some().then_some(t)
    })
    .await;
    assert_eq!((synced["needs_full_sync"].as_bool(), synced["device_name"].as_str()), (Some(false), Some("pinepods")), "{synced}");
    assert!(mock.subscriptions(REMOTE_USER).contains(&feed));

    // The stored password is never sent back.
    let stored = app.db().sync_targets_for_user(user.id).await.unwrap()[0].encrypted_secret.clone();
    let shown = app.get("/api/sync/target").key(&user).send().await.text();
    assert!(!shown.contains(REMOTE_PASS) && !shown.contains(&stored), "{shown}");

    // Sync now.
    let run = app.post("/api/sync/run").key(&user).send().await.ok_json();
    assert_eq!((run["success"].as_bool(), run["summary"]["full"].as_bool()), (Some(true), Some(false)), "{run}");

    // A failure shows on the status, with the backoff for automatic runs.
    mock.fail_next("/api/2/", 500, 1);
    let failed = app.post("/api/sync/run").key(&user).send().await.ok_json();
    assert_eq!(failed["success"], false, "{failed}");
    assert!(failed["message"].as_str().is_some_and(|m| m.contains("500")), "{failed}");
    let t = &failed["target"];
    assert_eq!((t["consecutive_failures"].as_i64(), t["in_backoff"].as_bool()), (Some(1), Some(true)), "{t}");
    assert!(t["last_error"].as_str().is_some_and(|e| e.contains("500")) && t["retry_at"].as_i64() > t["last_attempt_at"].as_i64(), "{t}");
    let stats = app.get("/api/sync/stats").key(&user).send().await.ok_json();
    assert_eq!(stats["target"]["kind"], "gpodder", "{stats}");
    assert!(stats["target"]["pending_actions"].is_i64(), "{stats}");

    // Force full sync pulls everything again and clears the failure.
    let full_pulls = || mock.requests().iter().filter(|r| r.path.starts_with("/api/2/subscriptions/") && r.query.as_deref() == Some("since=0")).count();
    let before = full_pulls();
    let full = app.post("/api/sync/run?full=true").key(&user).send().await.ok_json();
    assert_eq!((full["success"].as_bool(), full["summary"]["full"].as_bool()), (Some(true), Some(true)), "{full}");
    assert_eq!(full_pulls(), before + 1);
    assert_eq!((full["target"]["consecutive_failures"].as_i64(), full["target"]["in_backoff"].as_bool()), (Some(0), Some(false)));

    // Saving the same account again without the password keeps it; the device can be chosen.
    let renamed = app
        .put("/api/sync/target")
        .key(&user)
        .json(&json!({ "kind": "gpodder", "url": mock.base_url, "username": REMOTE_USER, "device_name": "pinepods-home" }))
        .send()
        .await
        .ok_json();
    assert_eq!(renamed["target"]["device_name"], "pinepods-home", "{renamed}");
    let t = app.db().sync_targets_for_user(user.id).await.unwrap();
    assert_eq!(app.db().decrypt_password(&t[0].encrypted_secret).await.unwrap(), REMOTE_PASS);

    // Other users see and run nothing of it.
    assert_eq!(app.get("/api/sync/target").key(&other).send().await.ok_json()["target"], Value::Null);
    app.post("/api/sync/run").key(&other).send().await.assert_status(400);
    app.delete("/api/sync/target").key(&other).send().await.assert_ok();
    assert_eq!(app.db().sync_targets_for_user(user.id).await.unwrap().len(), 1);
    let theirs = app.get("/api/sync/stats").key(&other).send().await.ok_json();
    assert_eq!((theirs["target"].clone(), theirs["total_episode_actions"].as_i64()), (Value::Null, Some(0)), "{theirs}");

    app.delete("/api/sync/target").key(&user).send().await.assert_ok();
    assert_eq!(app.get("/api/sync/target").key(&user).send().await.ok_json()["target"], Value::Null);
    assert_eq!(sync_type(&app, &user).await, "None");
}

#[tokio::test(flavor = "multi_thread")]
async fn internal_server_and_target_side_by_side() {
    let app = TestApp::new().await;
    let user = app.user("sidebyside").await;
    let mock = MockGpodder::start().await;
    mock.add_user(REMOTE_USER, REMOTE_PASS);

    app.put("/api/sync/target").key(&user).json(&creds("nextcloud", &mock, Some(REMOTE_PASS))).send().await.assert_ok();
    app.put("/api/sync/server").key(&user).json(&json!({ "enabled": true })).send().await.assert_ok();
    assert_eq!(sync_type(&app, &user).await, "both");
    let t = app.get("/api/sync/target").key(&user).send().await.ok_json();
    assert_eq!(t["target"]["kind"], "nextcloud", "{t}");

    // Each side switches off without touching the other.
    app.delete("/api/sync/target").key(&user).send().await.assert_ok();
    assert_eq!(sync_type(&app, &user).await, "gpodder");
    assert_eq!(app.get("/api/sync/server").key(&user).send().await.ok_json()["enabled"], true);
    app.put("/api/sync/target").key(&user).json(&creds("gpodder", &mock, Some(REMOTE_PASS))).send().await.assert_ok();
    app.put("/api/sync/server").key(&user).json(&json!({ "enabled": false })).send().await.assert_ok();
    assert_eq!(sync_type(&app, &user).await, "external");
    assert_eq!(app.db().sync_targets_for_user(user.id).await.unwrap()[0].kind, TargetKind::Gpodder);
}

#[tokio::test(flavor = "multi_thread")]
async fn old_routes_keep_their_shapes() {
    let app = TestApp::new().await;
    let user = app.user("legacy").await;
    let mock = MockGpodder::start().await;
    mock.add_user(REMOTE_USER, REMOTE_PASS);
    let feed = app.feed("legacy.xml", &Feed::with_episodes("Legacy Show", 1));
    app.subscribe(&user, &feed, 1).await;
    let has = |v: &Value, keys: &[&str]| keys.iter().all(|k| v.get(*k).is_some());

    // Nothing configured.
    let stats = app.get("/api/gpodder/gpodder_statistics").key(&user).send().await.ok_json();
    assert_eq!((stats["sync_type"].as_str(), stats["sync_enabled"].as_bool()), (Some("None"), Some(false)));
    let user_stats = app.get("/api/data/get_stats").key(&user).query("user_id", user.id).send().await.ok_json();
    assert_eq!((user_stats["Pod_Sync_Type"].as_str(), user_stats["GpodderUrl"].as_str()), (Some("None"), Some("")), "{user_stats}");

    // External gpodder only: old clients see "gpodder" with the server's URL.
    app.put("/api/sync/target").key(&user).json(&creds("gpodder", &mock, Some(REMOTE_PASS))).send().await.assert_ok();
    eventually("first sync", || async {
        app.db().sync_targets_for_user(user.id).await.unwrap().into_iter().find(|t| t.last_success_at.is_some())
    })
    .await;
    let user_stats = app.get("/api/data/get_stats").key(&user).query("user_id", user.id).send().await.ok_json();
    assert_eq!((user_stats["Pod_Sync_Type"].as_str(), user_stats["GpodderUrl"].as_str()), (Some("gpodder"), Some(mock.base_url.as_str())));
    assert_eq!((user_stats["SyncServerEnabled"].as_bool(), user_stats["SyncTargetKind"].as_str()), (Some(false), Some("gpodder")));
    let devices = app.get("/api/gpodder/devices").key(&user).send().await.ok_json();
    assert!(devices.as_array().unwrap().iter().any(|d| d["id"] == "pinepods" && d["is_default"] == true), "{devices}");
    let stats = app.get("/api/gpodder/gpodder_statistics").key(&user).send().await.ok_json();
    assert_eq!((stats["server_url"].as_str(), stats["connection_status"].as_str()), (Some(mock.base_url.as_str()), Some("Connected")), "{stats}");
    assert_eq!(stats["total_subscriptions"], 1);
    let tested = app
        .get("/api/gpodder/test-connection")
        .key(&user)
        .query("user_id", user.id)
        .query("gpodder_url", &mock.base_url)
        .query("gpodder_username", REMOTE_USER)
        .query("gpodder_password", REMOTE_PASS)
        .send()
        .await
        .ok_json();
    assert!(tested["success"] == true && has(&tested["data"], &["auth_type", "has_devices"]), "{tested}");
    let made = app
        .post("/api/gpodder/devices")
        .key(&user)
        .json(&json!({ "device_name": "kitchen", "device_type": "other", "device_caption": "Kitchen" }))
        .send()
        .await
        .ok_json();
    assert!(has(&made, &["id", "name", "type", "caption", "last_sync", "is_active", "is_remote", "is_default"]), "{made}");
    let devices = app.get("/api/gpodder/devices").key(&user).send().await.ok_json();
    assert!(devices.as_array().unwrap().iter().any(|d| d["id"] == "kitchen"), "{devices}");

    // Internal server on as well ("both").
    let toggled = app.post("/api/data/gpodder/toggle").key(&user).json(&json!({ "enabled": true })).send().await.ok_json();
    assert!(has(&toggled, &["sync_type", "gpodder_enabled", "external_enabled", "external_url", "api_url"]), "{toggled}");
    assert_eq!(toggled["sync_type"], "both");
    let status = app.get("/api/data/gpodder/status").key(&user).send().await.ok_json();
    assert!(has(&status, &["sync_type", "gpodder_enabled", "external_enabled", "external_url", "api_url"]), "{status}");
    assert_eq!((status["external_enabled"].as_bool(), status["external_url"].as_str()), (Some(true), Some(mock.base_url.as_str())));
    let user_stats = app.get("/api/data/get_stats").key(&user).query("user_id", user.id).send().await.ok_json();
    assert_eq!(user_stats["GpodderUrl"], "http://localhost:8042", "older clients compare against the internal marker");
    assert_eq!((user_stats["SyncServerEnabled"].as_bool(), user_stats["SyncServerUrl"].clone()), (Some(true), Value::Null), "{user_stats}");
    let devices = app.get(&format!("/api/gpodder/devices/{}", user.id)).key(&user).send().await.ok_json();
    let own = devices.as_array().unwrap().iter().find(|d| d["is_default"] == true).unwrap_or_else(|| panic!("{devices}")).clone();
    assert!(has(&own, &["id", "name", "type", "caption", "last_sync", "is_active", "is_remote"]), "{own}");
    let default = app.get("/api/gpodder/default_device").key(&user).send().await.ok_json();
    assert_eq!(default["id"], own["id"]);

    // Picking a default device is gone but still answers success.
    let set = app.post("/api/gpodder/set_default/whatever").key(&user).send().await.ok_json();
    assert_eq!((set["success"].as_bool(), set["data"].clone()), (Some(true), Value::Null));

    let stats = app.get("/api/gpodder/gpodder_statistics").key(&user).send().await.ok_json();
    assert!(
        has(&stats, &["server_url", "sync_type", "sync_enabled", "server_devices", "total_devices", "server_subscriptions", "total_subscriptions", "recent_episode_actions", "total_episode_actions", "connection_status", "last_sync_timestamp", "api_endpoints_tested"]),
        "{stats}"
    );
    assert_eq!(stats["sync_type"], "both");
    let synced = app.post("/api/gpodder/sync").key(&user).send().await.ok_json();
    assert_eq!(synced["success"], true, "{synced}");
}
