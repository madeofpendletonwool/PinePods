//! One user can never read or change another user's data.
//!
//! For every secured operation that names a user (a `user_id`/`userid` path, query or body
//! field), send user B's key with user A's id. A 2xx means B just acted on A's account.
//! Like auth_coverage, this walks the live OpenAPI document, so new endpoints are covered
//! automatically.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::harness::*;
use crate::support::openapi::{Operation, Spec};

const USER_FIELDS: &[&str] = &["user_id", "userid", "UserID"];

/// Operations that take a user id but are legitimately callable on someone else's account.
/// Every entry needs a reason; an empty list is the goal.
const ALLOWED_CROSS_USER: &[(&str, &str)] = &[];

fn names_a_user(spec: &Spec, op: &Operation) -> bool {
    let params = op.op.get("parameters").and_then(Value::as_array).cloned().unwrap_or_default();
    if params.iter().any(|p| USER_FIELDS.contains(&p["name"].as_str().unwrap_or(""))) {
        return true;
    }
    // A body that mentions a user field once the example is generated with overrides.
    let marker = |name: &str| USER_FIELDS.contains(&name).then(|| json!(-424242));
    spec.body(op, &marker)
        .map(|(b, _)| String::from_utf8_lossy(&b).contains("-424242"))
        .unwrap_or(false)
}

/// Destructive operations run last so they can't mask probes that come after them.
fn destructive(op: &Operation) -> bool {
    op.method == axum::http::Method::DELETE
        || ["delete", "remove", "clear", "restore"].iter().any(|w| op.template.contains(w))
}

#[tokio::test(flavor = "multi_thread")]
async fn users_cannot_act_on_each_others_accounts() {
    let app = TestApp::new().await;
    let victim = app.user("victim").await;
    let attacker = app.user("attacker").await;
    let spec = Spec::load();

    let mut ops: Vec<Operation> = spec
        .operations()
        .into_iter()
        .filter(|o| o.secured && names_a_user(&spec, o))
        .collect();
    ops.sort_by_key(destructive);

    let victim_id = victim.id;
    let ov = move |name: &str| USER_FIELDS.contains(&name).then(|| json!(victim_id));

    let mut failures = BTreeMap::new();
    let mut controls_ok = 0;
    for op in &ops {
        if ALLOWED_CROSS_USER.iter().any(|(l, _)| *l == op.label()) {
            continue;
        }
        let build = |who: &TestUser| {
            let mut req = app.request(op.method.clone(), &spec.uri(op, &ov)).key(who);
            if let Some((bytes, ct)) = spec.body(op, &ov) {
                req = req.raw_body(bytes, ct);
            }
            req
        };
        // Attacker first (it must not succeed), then the victim's own call as a control:
        // if the victim *can* do it, the attacker's refusal really was an access check.
        let resp = build(&attacker).send().await;
        if resp.is_success() {
            failures.insert(op.label(), format!("{} {}", resp.status, resp.text().chars().take(140).collect::<String>()));
        }
        if !destructive(op) && build(&victim).send().await.is_success() {
            controls_ok += 1;
        }
    }

    eprintln!("cross_user: {} user-scoped operations, {controls_ok} owner controls succeeded", ops.len());
    assert!(ops.len() > 80, "only {} user-scoped operations found; spec walk is broken", ops.len());
    // Guard against the probe silently degrading (e.g. every request failing validation):
    // a healthy fraction of the owner calls made (destructive ones are skipped) must work.
    let controls = ops.iter().filter(|op| !destructive(op)).count();
    assert!(
        controls_ok * 100 >= controls * 60,
        "only {controls_ok}/{controls} probes succeeded for the owner — synthesized requests are too broken to prove anything",
    );
    assert!(
        failures.is_empty(),
        "{} of {} user-scoped endpoints let user B act on user A:\n{}",
        failures.len(),
        ops.len(),
        failures.iter().map(|(k, v)| format!("  {k}: {v}")).collect::<Vec<_>>().join("\n")
    );
}

/// The probe above fills parameters with placeholders, so it can't tell when a lookup by a real
/// podcast leaks: these name one, as the user and as someone else.
#[tokio::test(flavor = "multi_thread")]
async fn another_users_podcast_reads_like_a_missing_one() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("private.xml", &Feed::with_episodes("Private Show", 1));
    let pod = app.subscribe(&alice, &url, 1).await;

    let details = |as_user: &TestUser, user_id: i32| {
        app.get("/api/data/get_podcast_details_dynamic")
            .key(as_user)
            .query("user_id", user_id)
            .query("podcast_title", "Private Show")
            .query("podcast_url", url.clone())
            .query("podcast_index_id", 0)
            .query("added", true)
    };
    details(&alice, alice.id).send().await.assert_ok();
    details(&bob, alice.id).send().await.assert_status(403);

    let merged = format!("/api/data/{pod}/merged");
    app.get(&merged).key(&alice).send().await.assert_ok();
    app.get(&merged).key(&bob).send().await.assert_status(404);

    for path in ["get_auto_transcribe", "get_silence_trim", "get_auto_ad_detect", "get_ad_skip_auto_activate"] {
        let setting = |as_user: &TestUser| {
            app.get(&format!("/api/data/{path}")).key(as_user).query("user_id", as_user.id).query("podcast_id", pod)
        };
        setting(&alice).send().await.assert_ok();
        let resp = setting(&bob).send().await;
        assert_eq!(resp.status.as_u16(), 404, "{path}: {}", resp.text());
    }
}

/// The transcript proxy fetches a caller-supplied URL and returns the body.
#[tokio::test(flavor = "multi_thread")]
async fn transcript_proxy_stays_off_internal_hosts_and_other_users_episodes() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("private.xml", &Feed::with_episodes("Private Show", 1));
    let pod = app.subscribe(&alice, &url, 1).await;
    let eps = app.episodes(&alice, pod).await;
    let ep = eps[0]["episodeid"].as_i64().or(eps[0]["Episodeid"].as_i64()).expect("episode id");

    let fetch = |as_user: &TestUser, url: String| {
        app.post("/api/data/fetch_transcript").key(as_user).json(&json!({ "url": url }))
    };

    for internal in ["http://169.254.169.254/latest/meta-data/", "http://10.0.0.5/", "file:///etc/passwd"] {
        fetch(&bob, internal.to_string()).send().await.assert_status(400);
    }

    let ai = format!("pinepods-internal://transcript/{ep}");
    fetch(&alice, ai.clone()).send().await.assert_ok();
    fetch(&bob, ai).send().await.assert_status(404);
}

/// Downloads go out with the podcast's stored feed credentials, so only the owner may ask.
#[tokio::test(flavor = "multi_thread")]
async fn another_users_episode_download_reads_like_a_missing_one() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("private.xml", &Feed::with_episodes("Private Show", 1));
    let pod = app.subscribe(&alice, &url, 1).await;
    let eps = app.episodes(&alice, pod).await;
    let ep = eps[0]["episodeid"].as_i64().or(eps[0]["Episodeid"].as_i64()).expect("episode id");

    let resp = app.get(&format!("/api/episodes/{ep}/download")).key(&bob).send().await;
    assert_eq!(resp.status.as_u16(), 404, "{}", resp.text());
    let own = app.get(&format!("/api/episodes/{ep}/download")).key(&alice).send().await;
    assert_ne!(own.status.as_u16(), 404, "{}", own.text());
}
