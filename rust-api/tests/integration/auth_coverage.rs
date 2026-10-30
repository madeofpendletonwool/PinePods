//! Every endpoint in the API either rejects strangers or is *deliberately* public.
//!
//! PinePods enforces auth inside each handler (no middleware), so a single forgotten
//! `extract_api_key` exposes an endpoint. These tests walk the live OpenAPI document — the
//! same one the server builds at startup — so a new route is covered the moment it's added:
//!
//! * a secured route must answer 401/403 to a missing key and to a bogus key (never 2xx, and
//!   never 5xx — a crash on bad credentials is its own bug);
//! * a route that isn't marked secured must be listed in [`PUBLIC`] with a reason, so making
//!   something public is a reviewed decision rather than an accident.

use std::collections::BTreeMap;

use axum::http::Method;

use crate::harness::*;
use crate::support::openapi::{Operation, Spec};

// The strings say why a route is public; only the variant is checked.
#[allow(dead_code)]
#[derive(Clone, Copy)]
enum Public {
    /// Anyone may call it.
    Open(&'static str),
    /// No `Api-Key`, but it demands some other credential (rss key, share key, basic auth, …):
    /// an anonymous request with junk credentials must still be refused.
    Gated(&'static str),
}

const PUBLIC: &[(&str, Public)] = &[
    ("GET /api/pinepods_check", Public::Open("instance discovery for clients")),
    ("GET /api/health", Public::Open("container health check")),
    ("GET /api/data/self_service_status", Public::Open("login page: is sign-up enabled")),
    ("GET /api/data/public_oidc_providers", Public::Open("login page: OIDC buttons")),
    ("GET /api/data/restore_status", Public::Open("setup page polls during a restore")),
    ("GET /api/data/get_available_languages", Public::Open("language picker before login")),
    ("GET /api/data/get_server_default_language", Public::Open("UI language before login")),
    ("GET /api/proxy/image", Public::Open("<img> tags can't send Api-Key; SSRF-guarded")),
    ("POST /api/auth/store_state", Public::Open("OIDC login start (pre-auth)")),
    ("GET /api/auth/callback", Public::Gated("OIDC callback; requires a stored state")),
    ("POST /api/data/create_first", Public::Gated("only works while no admin exists")),
    ("GET /api/data/get_key", Public::Gated("HTTP Basic username/password")),
    ("POST /api/data/verify_mfa_and_get_key", Public::Gated("MFA session token")),
    ("POST /api/data/reset_password_create_code", Public::Open("forgot-password email (no account leak)")),
    ("POST /api/data/verify_and_reset_password", Public::Gated("emailed reset code")),
    ("GET /api/data/episode_by_url/{url_key}", Public::Gated("share link key")),
    ("GET /api/feed/{user_id}", Public::Gated("per-user RSS key in ?api_key")),
    ("POST /api/init/startup_tasks", Public::Gated("background-tasks user's key in body")),
];

/// Routes served outside the OpenAPI document; checked explicitly so they can't drift.
const UNSPEC_SECURED: &[(&str, &str)] = &[
    ("POST", "/api/podcasts/notification_status"),
    ("GET", "/ws/api/tasks/1"),
    ("GET", "/ws/api/data/episodes/1"),
    ("GET", "/ws/api/nowplaying/1"),
];

const BOGUS_KEY: &str = "not-a-real-key-0000000000000000000000000000000000000000000000000000";

fn send(app: &TestApp, spec: &Spec, op: &Operation, key: Option<&str>) -> crate::harness::http::RequestBuilder {
    let none = |_: &str| None;
    let uri = spec.uri(op, &none);
    let mut req = app.request(op.method.clone(), &uri);
    if let Some((bytes, ct)) = spec.body(op, &none) {
        req = req.raw_body(bytes, ct);
    }
    if let Some(k) = key {
        req = req.api_key(k);
    }
    req
}

fn rejected(status: u16) -> bool {
    status == 401 || status == 403
}

#[tokio::test(flavor = "multi_thread")]
async fn every_secured_endpoint_rejects_missing_and_bogus_keys() {
    let app = TestApp::new().await;
    // An admin exists so first-run paths behave like a real instance.
    app.admin().await;
    let spec = Spec::load();

    let mut failures = BTreeMap::new();
    let mut checked = 0;
    for op in spec.operations().iter().filter(|o| o.secured) {
        checked += 1;
        for (label, key) in [("no key", None), ("bogus key", Some(BOGUS_KEY))] {
            let resp = send(&app, &spec, op, key).send().await;
            if !rejected(resp.status.as_u16()) {
                let body: String = resp.text().chars().take(160).collect();
                failures.insert(format!("{} [{label}]", op.label()), format!("{} {body}", resp.status));
            }
        }
    }

    assert!(checked > 200, "spec looks truncated: only {checked} secured operations");
    assert!(
        failures.is_empty(),
        "{} secured endpoint probe(s) did not answer 401/403:\n{}",
        failures.len(),
        failures.iter().map(|(k, v)| format!("  {k}: {v}")).collect::<Vec<_>>().join("\n")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn public_endpoints_are_exactly_the_reviewed_list() {
    let spec = Spec::load();
    let actual: Vec<String> = spec.operations().iter().filter(|o| !o.secured).map(Operation::label).collect();
    let expected: Vec<String> = PUBLIC.iter().map(|(l, _)| l.to_string()).collect();

    let unexpected: Vec<_> = actual.iter().filter(|l| !expected.contains(l)).collect();
    let stale: Vec<_> = expected.iter().filter(|l| !actual.contains(l)).collect();
    assert!(
        unexpected.is_empty() && stale.is_empty(),
        "public surface changed.\n  newly public (add `security((\"api_key\" = []))` or list it in PUBLIC with a reason): {unexpected:?}\n  listed but no longer public/present: {stale:?}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn gated_public_endpoints_refuse_junk_credentials() {
    let app = TestApp::new().await;
    app.admin().await;
    let spec = Spec::load();
    let ops = spec.operations();

    let mut failures = Vec::new();
    for (label, kind) in PUBLIC {
        let Public::Gated(_) = kind else { continue };
        let op = ops.iter().find(|o| &o.label() == label).unwrap_or_else(|| panic!("{label} missing from spec"));
        let mut req = send(&app, &spec, op, None);
        if *label == "GET /api/data/get_key" {
            req = req.basic_auth("nobody", "wrong");
        }
        let resp = req.send().await;
        let s = resp.status.as_u16();
        // The MFA step reports failure in-band (200 + status) — fine as long as no key leaks.
        let in_band_refusal = *label == "POST /api/data/verify_mfa_and_get_key"
            && s == 200
            && resp.json()["retrieved_key"].is_null();
        if !(400..500).contains(&s) && !in_band_refusal {
            failures.push(format!("  {label}: {} {}", resp.status, resp.text().chars().take(160).collect::<String>()));
        }
    }
    assert!(failures.is_empty(), "gated endpoints accepted junk credentials or crashed:\n{}", failures.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
async fn routes_outside_the_spec_reject_missing_keys() {
    let app = TestApp::new().await;
    for (method, path) in UNSPEC_SECURED {
        let method: Method = method.parse().unwrap();
        let mut req = app.request(method.clone(), path);
        if method == Method::POST {
            req = req.json(&serde_json::json!({ "user_id": 1, "podcast_id": 1 }));
        }
        let resp = req.send().await;
        assert!(
            rejected(resp.status.as_u16()) || resp.status.as_u16() == 400,
            "{method} {path} without a key → {} {}",
            resp.status,
            resp.text()
        );
    }
}
