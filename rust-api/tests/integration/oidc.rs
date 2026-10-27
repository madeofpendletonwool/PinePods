//! OIDC login: the callback's token exchange and provider discovery, against a fixture IdP.

use crate::harness::*;

/// Registers a provider whose token and userinfo endpoints are fixtures; returns its client_id.
async fn add_provider(app: &TestApp, email: &str) -> String {
    let admin = app.admin().await;
    let fx = app.fixtures();
    let token_url = fx.serve(
        "idp/token",
        Fixture::new("application/json", r#"{"access_token":"fixture-token","token_type":"Bearer"}"#),
    );
    let userinfo_url = fx.serve(
        "idp/userinfo",
        Fixture::new("application/json", json!({ "email": email, "name": "Oidc User" }).to_string()),
    );
    let client_id = format!("pinepods-{}", fx.namespace);
    app.post("/api/data/add_oidc_provider")
        .key(admin)
        .json(&json!({
            "provider_name": "Fixture IdP",
            "client_id": client_id,
            "client_secret": "fixture-secret",
            "authorization_url": fx.url("idp/authorize"),
            "token_url": token_url,
            "user_info_url": userinfo_url,
            "button_text": "Fixture",
            "scope": "openid email profile",
            "button_color": "#000000",
            "button_text_color": "#ffffff",
        }))
        .send()
        .await
        .assert_ok();
    client_id
}

fn form_field(body: &[u8], key: &str) -> Option<String> {
    url::form_urlencoded::parse(body).find(|(k, _)| k == key).map(|(_, v)| v.into_owned())
}

#[tokio::test(flavor = "multi_thread")]
async fn token_exchange_repeats_the_clients_redirect_uri() {
    let app = TestApp::new().await;
    let client_id = add_provider(&app, "oidc-port@example.test").await;

    // Served on a non-default port behind a proxy whose Host header drops it.
    let redirect_uri = "https://pods.example.test:12345/api/auth/callback";
    let state = format!("state-{}", app.fixtures().namespace);
    app.post("/api/auth/store_state")
        .json(&json!({
            "state": state,
            "client_id": client_id,
            "origin_url": "https://pods.example.test:12345",
            "redirect_uri": redirect_uri,
        }))
        .send()
        .await
        .assert_ok();

    let resp = app
        .get("/api/auth/callback")
        .header("host", "pods.example.test")
        .header("x-forwarded-proto", "https")
        .query("code", "fixture-code")
        .query("state", &state)
        .send()
        .await;
    let location = resp.headers.get("location").and_then(|v| v.to_str().ok()).unwrap_or_default().to_string();
    assert!(location.contains("api_key="), "{} {location} {}", resp.status, resp.text());
    assert!(location.starts_with("https://pods.example.test:12345/oauth/callback"), "{location}");

    let hits = app.fixtures().hits("idp/token");
    assert_eq!(hits.len(), 1);
    assert_eq!(form_field(&hits[0].body, "redirect_uri").as_deref(), Some(redirect_uri));
}

#[tokio::test(flavor = "multi_thread")]
async fn token_exchange_falls_back_to_request_host_for_old_clients() {
    let app = TestApp::new().await;
    let client_id = add_provider(&app, "oidc-legacy@example.test").await;
    let state = format!("state-{}", app.fixtures().namespace);
    app.post("/api/auth/store_state")
        .json(&json!({ "state": state, "client_id": client_id, "origin_url": "https://pods.example.test" }))
        .send()
        .await
        .assert_ok();

    app.get("/api/auth/callback")
        .header("host", "pods.example.test")
        .header("x-forwarded-proto", "https")
        .query("code", "fixture-code")
        .query("state", &state)
        .send()
        .await;

    let hits = app.fixtures().hits("idp/token");
    assert_eq!(hits.len(), 1);
    assert_eq!(
        form_field(&hits[0].body, "redirect_uri").as_deref(),
        Some("https://pods.example.test/api/auth/callback")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn store_state_rejects_foreign_redirect_uris() {
    let app = TestApp::new().await;
    for bad in ["https://evil.example.test/collect", "pinepods://auth/callback", "not a url"] {
        app.post("/api/auth/store_state")
            .json(&json!({ "state": "s", "client_id": "c", "redirect_uri": bad }))
            .send()
            .await
            .assert_status(400);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn discovery_fills_endpoints_for_admins() {
    let app = TestApp::new().await;
    let admin = app.admin().await;
    let fx = app.fixtures();
    let issuer = fx.url("issuer");
    fx.serve(
        "issuer/.well-known/openid-configuration",
        Fixture::new(
            "application/json",
            json!({
                "issuer": issuer,
                "authorization_endpoint": format!("{issuer}/authorize"),
                "token_endpoint": format!("{issuer}/token"),
                "userinfo_endpoint": format!("{issuer}/userinfo"),
                "jwks_uri": format!("{issuer}/jwks"),
                "scopes_supported": ["openid", "email", "profile"],
            })
            .to_string(),
        ),
    );

    // Both the issuer and the full document URL work.
    for url in [issuer.clone(), format!("{issuer}/.well-known/openid-configuration")] {
        let doc = app.post("/api/data/oidc_discovery").key(admin).json(&json!({ "url": url })).send().await.ok_json();
        assert_eq!(doc["token_endpoint"], format!("{issuer}/token"));
        assert_eq!(doc["userinfo_endpoint"], format!("{issuer}/userinfo"));
    }

    let missing = app.post("/api/data/oidc_discovery").key(admin).json(&json!({ "url": fx.url("nowhere") })).send().await;
    missing.assert_status(400);
    assert!(missing.text().contains("404"), "{}", missing.text());
}
