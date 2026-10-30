//! Accounts: login, rate limiting, MFA, API keys, profile changes, deletion, admin rights, sign-up.

use crate::harness::*;
use pinepods_api::database::{build_totp, DatabasePool};

async fn login_status(app: &TestApp, username: &str, password: &str) -> u16 {
    app.get("/api/data/get_key").basic_auth(username, password).send().await.status.as_u16()
}

async fn key_works(app: &TestApp, key: &str) -> bool {
    app.get("/api/data/verify_key").api_key(key).send().await.is_success()
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_credentials_and_bad_keys_are_rejected() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;

    assert_eq!(login_status(&app, &user.username, "wrong-password").await, 401);
    assert_eq!(login_status(&app, &format!("ghost{}", app.tag), "whatever").await, 401);
    assert!(!app.get("/api/data/get_key").send().await.is_success(), "no credentials");

    assert!(key_works(&app, &user.api_key).await);
    assert!(!key_works(&app, "not-a-real-key").await);
    app.get("/api/data/get_user").api_key("not-a-real-key").send().await.assert_status(401);
    let me = app.get("/api/data/get_user").key(&user).send().await.ok_json();
    assert_eq!(me["retrieved_id"], user.id);
}

#[tokio::test(flavor = "multi_thread")]
async fn logging_in_again_returns_the_existing_key() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    assert_eq!(app.login(&user.username, &user.password).await, user.api_key, "login reuses the existing key");
}

#[tokio::test(flavor = "multi_thread")]
async fn login_attempts_are_rate_limited_per_username() {
    let app = TestApp::new().await;
    let victim = app.user("victim").await;
    let bystander = app.user("bystander").await;

    // 10 attempts per minute; creating the test user already used one.
    for _ in 0..9 {
        assert_eq!(login_status(&app, &victim.username, "guess").await, 401);
    }
    assert_eq!(login_status(&app, &victim.username, "guess").await, 429);
    assert_eq!(
        login_status(&app, &victim.username, &victim.password).await,
        429,
        "the correct password is throttled too while the window is open"
    );
    assert_eq!(
        login_status(&app, &victim.username.to_uppercase(), &victim.password).await,
        429,
        "the bucket ignores username case"
    );
    assert_eq!(login_status(&app, &bystander.username, &bystander.password).await, 200, "other accounts unaffected");
}

#[tokio::test(flavor = "multi_thread")]
async fn mfa_enrolment_then_login_requires_a_valid_code() {
    let app = TestApp::new().await;
    let user = app.user("secure").await;

    let check = || async {
        app.get(&format!("/api/data/check_mfa_enabled/{}", user.id)).key(&user).send().await.ok_json()["mfa_enabled"]
            .clone()
    };
    assert_eq!(check().await, false);

    // Enrolment: the server issues a secret, the user proves they can generate codes for it.
    let gen = app.get(&format!("/api/data/generate_mfa_secret/{}", user.id)).key(&user).send().await.ok_json();
    let secret = gen["secret"].as_str().expect("secret").to_string();
    assert!(gen["qr_code_svg"].as_str().unwrap_or_default().contains("<svg"), "{gen}");
    let code = || build_totp(&secret, "test").unwrap().generate_current().to_string();

    let bad = app
        .post("/api/data/verify_temp_mfa")
        .key(&user)
        .json(&json!({ "user_id": user.id, "mfa_code": "abcdef" }))
        .send()
        .await
        .ok_json();
    assert_eq!(bad["verified"], false);
    assert_eq!(check().await, false, "a wrong code must not enable MFA");

    let ok = app
        .post("/api/data/verify_temp_mfa")
        .key(&user)
        .json(&json!({ "user_id": user.id, "mfa_code": code() }))
        .send()
        .await
        .ok_json();
    assert_eq!(ok["verified"], true);
    assert_eq!(check().await, true);

    // Password alone no longer yields a key.
    let start = || async {
        let body = app.get("/api/data/get_key").basic_auth(&user.username, &user.password).send().await.ok_json();
        assert_eq!(body["status"], "mfa_required", "{body}");
        assert!(body["retrieved_key"].is_null(), "no key before the second factor: {body}");
        body["mfa_session_token"].as_str().unwrap().to_string()
    };
    let a = &app;
    let finish = |token: String, mfa_code: String| async move {
        a.post("/api/data/verify_mfa_and_get_key")
            .json(&json!({ "mfa_session_token": token, "mfa_code": mfa_code }))
            .send()
            .await
            .ok_json()
    };

    let token = start().await;
    let wrong = finish(token.clone(), "abcdef".into()).await;
    assert_eq!(wrong["verified"], false);
    assert!(wrong["retrieved_key"].is_null());
    let reused = finish(token, code()).await;
    assert_eq!(reused["status"], "invalid_session", "a failed attempt burns the session: {reused}");

    let forged = finish("made-up-session-token".into(), code()).await;
    assert_eq!(forged["status"], "invalid_session");

    let good = finish(start().await, code()).await;
    assert_eq!(good["verified"], true, "{good}");
    assert!(key_works(&app, good["retrieved_key"].as_str().unwrap()).await);

    // Disabling MFA restores plain password login.
    app.delete("/api/data/delete_mfa").key(&user).send().await.assert_ok();
    assert_eq!(check().await, false);
    assert_eq!(app.login(&user.username, &user.password).await, user.api_key);
}

#[tokio::test(flavor = "multi_thread")]
async fn users_cannot_enrol_or_inspect_mfa_for_someone_else() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let mallory = app.user("mallory").await;

    app.get(&format!("/api/data/generate_mfa_secret/{}", alice.id)).key(&mallory).send().await.assert_status(403);
    app.get(&format!("/api/data/check_mfa_enabled/{}", alice.id)).key(&mallory).send().await.assert_status(403);
    app.post("/api/data/save_mfa_secret")
        .key(&mallory)
        .json(&json!({ "user_id": alice.id, "mfa_secret": "JBSWY3DPEHPK3PXP" }))
        .send()
        .await
        .assert_status(403);
    assert_eq!(app.login(&alice.username, &alice.password).await, alice.api_key, "alice can still log in without MFA");
}

async fn api_keys(app: &TestApp, user: &TestUser) -> Vec<Value> {
    let body = app.get(&format!("/api/data/get_api_info/{}", user.id)).key(user).send().await.ok_json();
    body["api_info"].as_array().cloned().unwrap_or_default()
}

fn key_id(keys: &[Value], key: &str) -> String {
    let last4 = &key[key.len() - 4..];
    let matching: Vec<&Value> = keys.iter().filter(|k| k["lastfourdigits"] == last4).collect();
    assert_eq!(matching.len(), 1, "expected exactly one key ending {last4}: {keys:#?}");
    matching[0]["apikeyid"].to_string()
}

async fn delete_key(app: &TestApp, as_user: &TestUser, api_id: &str) -> u16 {
    app.delete("/api/data/delete_api_key")
        .key(as_user)
        .json(&json!({ "api_id": api_id }))
        .send()
        .await
        .status
        .as_u16()
}

#[tokio::test(flavor = "multi_thread")]
async fn api_keys_can_be_created_listed_and_revoked() {
    let app = TestApp::new().await;
    let user = app.user("keys").await;
    let other = app.user("other").await;

    let created = app
        .post("/api/data/create_api_key")
        .key(&user)
        .json(&json!({ "user_id": user.id, "rssonly": false }))
        .send()
        .await
        .ok_json();
    let new_key = created["api_key"].as_str().expect("api_key").to_string();
    assert_eq!(new_key.len(), 64);
    assert!(key_works(&app, &new_key).await);

    let keys = api_keys(&app, &user).await;
    assert_eq!(keys.len(), 2, "{keys:#?}");
    assert!(keys.iter().all(|k| k["userid"] == user.id), "a user only sees their own keys");
    assert!(keys.iter().all(|k| k.get("apikey").is_none()), "full keys are never listed");

    let new_id = key_id(&keys, &new_key);
    let login_id = key_id(&keys, &user.api_key);

    assert_eq!(delete_key(&app, &other, &new_id).await, 403, "someone else's key");
    assert_eq!(delete_key(&app, &user, &login_id).await, 403, "the key making the request");
    assert!(key_works(&app, &new_key).await);

    assert_eq!(delete_key(&app, &user, &new_id).await, 200);
    assert!(!key_works(&app, &new_key).await, "a revoked key stops working immediately");
    assert!(key_works(&app, &user.api_key).await);

    // With only one key left, deleting it (from another session) would lock the user out.
    let admin = app.admin().await;
    assert_eq!(delete_key(&app, admin, &login_id).await, 403, "a user's final key is protected");

    app.post("/api/data/create_api_key")
        .key(&other)
        .json(&json!({ "user_id": user.id, "rssonly": false }))
        .send()
        .await
        .assert_status(403);
}

#[tokio::test(flavor = "multi_thread")]
async fn password_username_email_and_name_changes_take_effect() {
    let app = TestApp::new().await;
    let user = app.user("changer").await;

    app.put(&format!("/api/data/set_password/{}", user.id))
        .key(&user)
        .json(&json!({ "hash_pw": hash_password("Brand-new-pass-1!") }))
        .send()
        .await
        .assert_ok();
    assert_eq!(login_status(&app, &user.username, &user.password).await, 401, "old password is dead");
    assert_eq!(login_status(&app, &user.username, "Brand-new-pass-1!").await, 200);

    let new_name = format!("Renamed{}", app.tag);
    app.put("/api/data/user/set_username")
        .key(&user)
        .json(&json!({ "user_id": user.id, "new_username": new_name }))
        .send()
        .await
        .assert_ok();
    assert_eq!(login_status(&app, &new_name.to_lowercase(), "Brand-new-pass-1!").await, 200, "stored lowercased");
    assert_eq!(login_status(&app, &user.username, "Brand-new-pass-1!").await, 401, "old username is gone");

    app.put("/api/data/user/set_email")
        .key(&user)
        .json(&json!({ "user_id": user.id, "new_email": "new@example.test" }))
        .send()
        .await
        .assert_ok();
    app.put(&format!("/api/data/set_fullname/{}", user.id))
        .key(&user)
        .query("new_name", "Fresh Name")
        .send()
        .await
        .assert_ok();

    let details = app.get(&format!("/api/data/user_details_id/{}", user.id)).key(&user).send().await.ok_json();
    assert_eq!(details["Email"], "new@example.test");
    assert_eq!(details["Fullname"], "Fresh Name");
    assert_eq!(details["Username"], new_name.to_lowercase());
    assert!(details["Hashed_PW"].is_null(), "the password hash must never be sent to clients: {details}");
    assert!(details["Salt"].is_null());

    assert!(key_works(&app, &user.api_key).await, "profile changes don't revoke existing keys");
}

#[tokio::test(flavor = "multi_thread")]
async fn users_cannot_change_someone_elses_account_but_admins_can() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let mallory = app.user("mallory").await;
    let admin = app.admin().await;

    let set_pw = |as_user: &TestUser, pw: &str| {
        app.put(&format!("/api/data/set_password/{}", alice.id))
            .key(as_user)
            .json(&json!({ "hash_pw": hash_password(pw) }))
    };
    set_pw(&mallory, "Taken-over-1!").send().await.assert_status(403);
    for (path, body) in [
        ("/api/data/user/set_username", json!({ "user_id": alice.id, "new_username": "pwned" })),
        ("/api/data/user/set_email", json!({ "user_id": alice.id, "new_email": "evil@example.test" })),
    ] {
        app.put(path).key(&mallory).json(&body).send().await.assert_status(403);
    }
    assert_eq!(login_status(&app, &alice.username, &alice.password).await, 200);

    set_pw(admin, "Admin-reset-1!").send().await.assert_ok();
    assert_eq!(login_status(&app, &alice.username, "Admin-reset-1!").await, 200, "admins can reset passwords");
}

/// Every row in any table whose `UserID` column points at `user_id`.
async fn rows_referencing_user(db: &DatabasePool, user_id: i32) -> Vec<(String, i64)> {
    let mut found = Vec::new();
    match db {
        DatabasePool::Postgres(pool) => {
            let tables: Vec<String> = sqlx::query_scalar(
                "SELECT table_name::text FROM information_schema.columns \
                 WHERE table_schema = 'public' AND column_name = 'userid'",
            )
            .fetch_all(pool)
            .await
            .unwrap();
            for t in tables {
                let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(r#"SELECT COUNT(*) FROM "{t}" WHERE userid = $1"#)))
                    .bind(user_id)
                    .fetch_one(pool)
                    .await
                    .unwrap();
                if n > 0 {
                    found.push((t, n));
                }
            }
        }
        DatabasePool::MySQL(pool) => {
            let tables: Vec<String> = sqlx::query_scalar(
                "SELECT CAST(TABLE_NAME AS CHAR) FROM INFORMATION_SCHEMA.COLUMNS \
                 WHERE TABLE_SCHEMA = DATABASE() AND COLUMN_NAME = 'UserID'",
            )
            .fetch_all(pool)
            .await
            .unwrap();
            for t in tables {
                let n: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM `{t}` WHERE UserID = ?")))
                    .bind(user_id)
                    .fetch_one(pool)
                    .await
                    .unwrap();
                if n > 0 {
                    found.push((t, n));
                }
            }
        }
    }
    found.sort();
    found
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_user_removes_their_access_and_every_row_they_owned() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let doomed = app.user("doomed").await;
    let survivor = app.user("survivor").await;

    // Give the doomed user a footprint in as many tables as possible.
    let url = app.feed("doomed.xml", &Feed::with_episodes("Doomed Show", 2));
    let pod = app.subscribe(&doomed, &url, 2).await;
    let ep = app.episode_id(&doomed, pod, "Episode 1").await;
    let survivor_pod = app.subscribe(&survivor, &url, 2).await;
    let body = json!({ "episode_id": ep, "user_id": doomed.id, "is_youtube": false });
    for path in ["/api/data/save_episode", "/api/data/queue_pod", "/api/data/mark_episode_completed"] {
        app.post(path).key(&doomed).json(&body).send().await.assert_ok();
    }
    app.post("/api/data/record_listen_duration")
        .key(&doomed)
        .json(&json!({ "episode_id": ep, "user_id": doomed.id, "listen_duration": 60.0 }))
        .send()
        .await
        .assert_ok();
    let collection = app
        .post("/api/data/collections/create")
        .key(&doomed)
        .json(&json!({ "user_id": doomed.id, "name": "Mine" }))
        .send()
        .await
        .ok_json()["collection_id"]
        .as_i64()
        .unwrap();
    app.post(&format!("/api/data/collections/{collection}/add_episode"))
        .key(&doomed)
        .json(&json!({ "user_id": doomed.id, "episode_id": ep, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    for rssonly in [false, true] {
        app.post("/api/data/create_api_key")
            .key(&doomed)
            .json(&json!({ "user_id": doomed.id, "rssonly": rssonly }))
            .send()
            .await
            .assert_ok();
    }
    assert!(!rows_referencing_user(app.db(), doomed.id).await.is_empty());

    app.delete(&format!("/api/data/user/delete/{}", doomed.id)).key(&doomed).send().await.assert_status(403);
    app.delete(&format!("/api/data/user/delete/{}", doomed.id)).key(admin).send().await.assert_ok();

    assert!(!key_works(&app, &doomed.api_key).await, "a deleted user's keys stop working");
    assert_eq!(login_status(&app, &doomed.username, &doomed.password).await, 401);
    let leftovers = rows_referencing_user(app.db(), doomed.id).await;
    assert!(leftovers.is_empty(), "rows left behind for the deleted user: {leftovers:?}");

    assert_eq!(app.episodes(&survivor, survivor_pod).await.len(), 2, "other users' copies of the feed survive");

    // The username is free again.
    app.post("/api/data/add_user")
        .key(admin)
        .json(&json!({
            "fullname": "Reborn",
            "username": doomed.username,
            "email": doomed.email,
            "hash_pw": hash_password("Reborn-pass-1!"),
        }))
        .send()
        .await
        .assert_ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_last_admin_cannot_be_deleted_or_demoted() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let a = &app;
    let final_admin = |id: i32| async move {
        a.get(&format!("/api/data/user/final_admin/{id}")).key(admin).send().await.ok_json()["final_admin"].clone()
    };
    assert_eq!(final_admin(admin.id).await, true);

    app.delete(&format!("/api/data/user/delete/{}", admin.id)).key(admin).send().await.assert_status(403);
    app.put("/api/data/user/set_isadmin")
        .key(admin)
        .json(&json!({ "user_id": admin.id, "isadmin": false }))
        .send()
        .await
        .assert_status(403);
    let check = app.get(&format!("/api/data/user_admin_check/{}", admin.id)).key(admin).send().await.ok_json();
    assert_eq!(check["is_admin"], true);

    // The background-tasks user (id 1) drives refreshes and can never be deleted.
    app.delete("/api/data/user/delete/1").key(admin).send().await.assert_status(403);

    // Once there is a second admin, either can step down.
    let deputy = app.second_admin("deputy").await;
    assert_eq!(final_admin(admin.id).await, false);
    app.put("/api/data/user/set_isadmin")
        .key(admin)
        .json(&json!({ "user_id": admin.id, "isadmin": false }))
        .send()
        .await
        .assert_ok();
    let check = app.get(&format!("/api/data/user_admin_check/{}", admin.id)).key(admin).send().await.ok_json();
    assert_eq!(check["is_admin"], false);
    app.delete(&format!("/api/data/user/delete/{}", deputy.id)).key(&deputy).send().await.assert_status(403);
}

#[tokio::test(flavor = "multi_thread")]
async fn self_service_sign_up_follows_the_admin_toggle() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let status = || async { app.get("/api/data/self_service_status").send().await.ok_json() };
    let s = status().await;
    assert_eq!(s["status"], false, "sign-up is off by default: {s}");
    assert_eq!(s["first_admin_created"], true);

    let username = format!("Newbie{}", app.tag);
    let sign_up = |email: &str| {
        app.post("/api/data/add_login_user").json(&json!({
            "fullname": "New Person",
            "username": username,
            "email": email,
            "hash_pw": hash_password("Signup-pass-1!"),
        }))
    };
    sign_up("newbie@example.test").send().await.assert_status(403);

    app.post("/api/data/enable_disable_self_service").key(admin).send().await.assert_ok();
    assert_eq!(status().await["status"], true);

    let created = sign_up("newbie@example.test").send().await.ok_json();
    let id = created["user_id"].as_i64().unwrap() as i32;
    let key = app.login(&username.to_lowercase(), "Signup-pass-1!").await;
    let check = app.get(&format!("/api/data/user_admin_check/{id}")).api_key(&key).send().await.ok_json();
    assert_eq!(check["is_admin"], false, "self-registered users are never admins");

    let dup = sign_up("other@example.test").send().await;
    assert!(!dup.is_success(), "duplicate username must be refused: {}", dup.text());

    app.post("/api/data/enable_disable_self_service").key(admin).send().await.assert_ok();
    assert_eq!(status().await["status"], false);
}
