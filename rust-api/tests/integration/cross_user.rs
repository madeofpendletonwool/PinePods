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
    // a healthy fraction must demonstrably work for the account owner.
    assert!(
        controls_ok * 100 >= ops.len() * 60,
        "only {controls_ok}/{} probes succeeded for the owner — synthesized requests are too broken to prove anything",
        ops.len()
    );
    assert!(
        failures.is_empty(),
        "{} of {} user-scoped endpoints let user B act on user A:\n{}",
        failures.len(),
        ops.len(),
        failures.iter().map(|(k, v)| format!("  {k}: {v}")).collect::<Vec<_>>().join("\n")
    );
}
