//! Sending mail: the settings page's test message, saved server settings, and password reset.

use crate::harness::mock_smtp::MockSmtp;
use crate::harness::*;

fn test_email(port: u16) -> Value {
    json!({
        "server_name": "127.0.0.1",
        "server_port": port.to_string(),
        "from_email": "pinepods@example.test",
        "encryption": "none",
        "auth_required": false,
        "email_username": "",
        "email_password": "",
        "to_email": "listener@example.test",
        "message": "hello from the settings page",
    })
}

async fn save_settings(app: &TestApp, port: u16) {
    let admin = app.admin().await.clone();
    app.post("/api/data/save_email_settings")
        .key(&admin)
        .json(&json!({ "email_settings": {
            "server_name": "127.0.0.1",
            "server_port": port.to_string(),
            "from_email": "pinepods@example.test",
            "send_mode": "SMTP",
            "encryption": "none",
            "auth_required": false,
            "email_username": "",
            "email_password": "",
        }}))
        .send()
        .await
        .assert_ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn the_test_email_reaches_the_recipient() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let smtp = MockSmtp::start().await;

    let body = app.post("/api/data/send_test_email")
        .key(&admin)
        .json(&test_email(smtp.port))
        .send()
        .await
        .ok_json();
    assert_eq!(body["email_status"], "Email sent successfully");

    let sent = smtp.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].from, "pinepods@example.test");
    assert_eq!(sent[0].to, ["listener@example.test"]);
    assert_eq!(sent[0].header("Subject").as_deref(), Some("PinePods - Test Email"));
    let html = sent[0].body();
    assert!(html.contains("hello from the settings page"), "{html}");
    // base64 of the sandbox's favicon.png: the logo is embedded in the message.
    assert!(html.contains("ZmFrZS1sb2dv"), "the logo should be embedded: {html}");
}

#[tokio::test(flavor = "multi_thread")]
async fn signing_in_to_the_mail_server_uses_the_given_account() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let smtp = MockSmtp::start().await;
    let mut request = test_email(smtp.port);
    request["auth_required"] = json!(true);
    request["email_username"] = json!("mailer");
    request["email_password"] = json!("mail-secret");

    app.post("/api/data/send_test_email").key(&admin).json(&request).send().await.assert_ok();

    assert_eq!(smtp.sent()[0].auth.as_deref(), Some("mailer\0mail-secret"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_rejected_mail_login_is_reported() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let smtp = MockSmtp::rejecting_logins().await;
    let mut request = test_email(smtp.port);
    request["auth_required"] = json!(true);
    request["email_username"] = json!("mailer");
    request["email_password"] = json!("wrong");

    let resp = app.post("/api/data/send_test_email").key(&admin).json(&request).send().await;

    assert!(!resp.is_success(), "got {}", resp.status);
    assert!(resp.text().contains("authentication"), "{}", resp.text());
    assert!(smtp.sent().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn bad_mail_settings_are_refused() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    for (field, value) in [("server_port", "not-a-port"), ("from_email", "nope"), ("to_email", "nope")] {
        let mut request = test_email(25);
        request[field] = json!(value);
        app.post("/api/data/send_test_email").key(&admin).json(&request).send().await.assert_status(400);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unreachable_mail_server_is_reported() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let port = {
        let smtp = MockSmtp::start().await;
        smtp.port
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let resp = app.post("/api/data/send_test_email").key(&admin).json(&test_email(port)).send().await;
    assert!(!resp.is_success(), "got {}", resp.status);
}

#[tokio::test(flavor = "multi_thread")]
async fn saved_settings_send_mail_and_keep_the_password_private() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let smtp = MockSmtp::start().await;
    app.post("/api/data/save_email_settings")
        .key(&admin)
        .json(&json!({ "email_settings": {
            "server_name": "127.0.0.1",
            "server_port": smtp.port.to_string(),
            "from_email": "pinepods@example.test",
            "send_mode": "SMTP",
            "encryption": "none",
            "auth_required": true,
            "email_username": "mailer",
            "email_password": "stored-mail-secret",
        }}))
        .send()
        .await
        .assert_ok();

    let shown = app.get("/api/data/get_email_settings").key(&admin).send().await;
    shown.assert_ok();
    assert!(!shown.text().contains("stored-mail-secret"), "{}", shown.text());

    app.post("/api/data/send_email")
        .key(&admin)
        .json(&json!({ "to_email": "someone@example.test", "subject": "Notice", "message": "line one\nline two" }))
        .send()
        .await
        .assert_ok();

    let sent = smtp.sent();
    assert_eq!(sent[0].to, ["someone@example.test"]);
    assert_eq!(sent[0].auth.as_deref(), Some("mailer\0stored-mail-secret"));
    assert!(sent[0].body().contains("line one<br>line two"), "{}", sent[0].body());
}

#[tokio::test(flavor = "multi_thread")]
async fn saving_settings_without_retyping_the_password_keeps_it() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let smtp = MockSmtp::start().await;
    let save = |from: &'static str, password: &'static str| {
        app.post("/api/data/save_email_settings").key(&admin).json(&json!({ "email_settings": {
            "server_name": "127.0.0.1",
            "server_port": smtp.port.to_string(),
            "from_email": from,
            "send_mode": "SMTP",
            "encryption": "none",
            "auth_required": true,
            "email_username": "mailer",
            "email_password": password,
        }}))
    };
    save("first@example.test", "kept-secret").send().await.assert_ok();
    save("second@example.test", "").send().await.assert_ok();

    app.post("/api/data/send_email")
        .key(&admin)
        .json(&json!({ "to_email": "someone@example.test", "subject": "Notice", "message": "hi" }))
        .send()
        .await
        .assert_ok();

    let sent = smtp.sent();
    assert_eq!(sent[0].from, "second@example.test");
    assert_eq!(sent[0].auth.as_deref(), Some("mailer\0kept-secret"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reset_code_from_email_changes_the_password() {
    let app = TestApp::new().await;
    let smtp = MockSmtp::start().await;
    save_settings(&app, smtp.port).await;
    let user = app.user("forgetful").await;

    let body = app.post("/api/data/reset_password_create_code")
        .json(&json!({ "username": user.username, "email": user.email }))
        .send()
        .await
        .ok_json();
    assert_eq!(body["code_created"], true);

    let sent = smtp.sent();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to, [user.email.clone()]);
    assert_eq!(sent[0].header("Subject").as_deref(), Some("PinePods - Password Reset Code"));
    let html = sent[0].body();
    let code = html
        .split("code-block\">")
        .nth(1)
        .and_then(|rest| rest.split('<').next())
        .unwrap_or_else(|| panic!("no code in {html}"))
        .to_string();
    assert_eq!(code.len(), 6, "{code}");

    let wrong = app.post("/api/data/verify_and_reset_password")
        .json(&json!({ "reset_code": "WRONG1", "email": user.email, "new_password": hash_password("Never-used-1!") }))
        .send()
        .await;
    assert!(wrong.status.as_u16() >= 400 || wrong.text().contains("false"), "{}", wrong.text());
    app.login(&user.username, &user.password).await;

    let new_password = "Brand-New-pass-1!";
    app.post("/api/data/verify_and_reset_password")
        .json(&json!({ "reset_code": code, "email": user.email, "new_password": hash_password(new_password) }))
        .send()
        .await
        .assert_ok();

    app.login(&user.username, new_password).await;
    let old = app.get("/api/data/get_key").basic_auth(&user.username, &user.password).send().await;
    assert!(!old.is_success(), "the old password should stop working");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_reset_request_for_an_unknown_account_sends_nothing_and_looks_the_same() {
    let app = TestApp::new().await;
    let smtp = MockSmtp::start().await;
    save_settings(&app, smtp.port).await;

    let body = app.post("/api/data/reset_password_create_code")
        .json(&json!({ "username": format!("nobody{}", app.tag), "email": format!("nobody{}@example.test", app.tag) }))
        .send()
        .await
        .ok_json();

    assert_eq!(body["code_created"], true);
    assert!(smtp.sent().is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failed_reset_email_leaves_no_usable_code() {
    let app = TestApp::new().await;
    let port = {
        let smtp = MockSmtp::start().await;
        smtp.port
    };
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    save_settings(&app, port).await;
    let user = app.user("unlucky").await;

    app.post("/api/data/reset_password_create_code")
        .json(&json!({ "username": user.username, "email": user.email }))
        .send()
        .await
        .assert_ok();

    let codes = format!(r#"SELECT COUNT(*) FROM "Users" WHERE UserID = {} AND Reset_Code IS NOT NULL"#, user.id);
    assert_eq!(int(&app, &codes).await, Some(0));
}
