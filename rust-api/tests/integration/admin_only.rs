//! Server-wide operations are admin-only.
//!
//! A regular user must get 403 (or 401) from each of these; the admin's own call is the
//! control proving the refusal is an authorization decision, not a malformed request.

use std::collections::BTreeMap;

use serde_json::json;

use crate::harness::*;
use crate::support::openapi::Spec;

/// Operations that affect the whole instance (other users, server config, the filesystem,
/// outbound mail, server-wide jobs). Adding an admin feature? Add it here.
pub const ADMIN_ONLY: &[&str] = &[
    // Users
    "POST /api/data/add_user",
    "GET /api/data/get_user_info",
    "PUT /api/data/user/set_isadmin",
    "DELETE /api/data/user/delete/{user_id}",
    // Instance settings
    "POST /api/data/enable_disable_downloads",
    "POST /api/data/enable_disable_self_service",
    // (GET /api/data/admin_self_service_status is any-user: same data as the public
    //  self_service_status.)
    "POST /api/data/download_metadata_settings",
    "GET /api/data/get_email_settings",
    "POST /api/data/save_email_settings",
    "POST /api/data/send_test_email",
    "POST /api/data/send_email",
    "GET /api/data/list_oidc_providers",
    "POST /api/data/add_oidc_provider",
    "POST /api/data/oidc_discovery",
    "PUT /api/data/update_oidc_provider/{provider_id}",
    "POST /api/data/remove_oidc_provider",
    "GET /api/data/ai_settings",
    "POST /api/data/ai_settings",
    "GET /api/data/ai_models",
    "POST /api/data/ai_pull_model",
    "GET /api/data/ytdlp_settings",
    "POST /api/data/ytdlp_settings",
    "POST /api/data/ytdlp_update",
    // Backups / restore / server filesystem
    "POST /api/data/backup_server",
    "POST /api/data/restore_server",
    "POST /api/data/schedule_backup",
    "POST /api/data/get_scheduled_backup",
    "POST /api/data/list_backup_files",
    "POST /api/data/restore_backup_file",
    "POST /api/data/manual_backup_to_directory",
    "POST /api/data/delete_backup_file",
    // (GET /api/data/list_local_directories is any-user by design: local podcasts are a
    //  per-user setting, and the handler confines browsing to the local-media root.)
    // Server-wide jobs
    "GET /api/data/refresh_pods",
    "GET /api/data/refresh_gpodder_subscriptions",
    "GET /api/data/refresh_nextcloud_subscriptions",
];

/// Scheduler hooks: only the background-tasks key (user 1) may call these — not even admins.
pub const SYSTEM_ONLY: &[&str] = &[
    "GET /api/data/refresh_hosts",
    "GET /api/data/cleanup_tasks",
    "GET /api/data/auto_complete_episodes",
    "GET /api/data/update_playlists",
];

#[tokio::test(flavor = "multi_thread")]
async fn regular_users_cannot_use_admin_endpoints() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let user = app.user("regular").await;
    // A third account so "delete user {id}" targets someone other than the caller.
    let bystander = app.user("bystander").await;
    let spec = Spec::load();
    let ops = spec.operations();

    let bystander_id = bystander.id;
    let ov = move |name: &str| match name {
        "user_id" | "userid" => Some(json!(bystander_id)),
        _ => None,
    };

    let mut failures = BTreeMap::new();
    for label in ADMIN_ONLY {
        let op = ops
            .iter()
            .find(|o| &o.label() == label)
            .unwrap_or_else(|| panic!("{label} is no longer in the API; update ADMIN_ONLY"));
        let build = |who: &TestUser| {
            let mut req = app.request(op.method.clone(), &spec.uri(op, &ov)).key(who);
            if let Some((bytes, ct)) = spec.body(op, &ov) {
                req = req.raw_body(bytes, ct);
            }
            req
        };
        let as_user = build(&user).send().await;
        let s = as_user.status.as_u16();
        if s != 403 && s != 401 {
            failures.insert(
                label.to_string(),
                format!("regular user got {} {}", as_user.status, as_user.text().chars().take(140).collect::<String>()),
            );
            continue;
        }
        // Skip the control for operations that would wreck the rest of the run.
        if label.contains("restore") || label.contains("delete") {
            continue;
        }
        let as_admin = build(&admin).send().await;
        let a = as_admin.status.as_u16();
        if a == 401 || a == 403 {
            failures.insert(label.to_string(), format!("admin control was refused too: {}", as_admin.status));
        }
    }

    for label in SYSTEM_ONLY {
        let op = ops
            .iter()
            .find(|o| &o.label() == label)
            .unwrap_or_else(|| panic!("{label} is no longer in the API; update SYSTEM_ONLY"));
        for who in [&user, &admin] {
            let mut req = app.request(op.method.clone(), &spec.uri(op, &ov)).key(who);
            if let Some((bytes, ct)) = spec.body(op, &ov) {
                req = req.raw_body(bytes, ct);
            }
            let resp = req.send().await;
            let s = resp.status.as_u16();
            if s != 403 && s != 401 {
                failures.insert(format!("{label} [{}]", who.username), format!("got {}", resp.status));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} admin-only endpoint(s) misbehaved:\n{}",
        failures.len(),
        failures.iter().map(|(k, v)| format!("  {k}: {v}")).collect::<Vec<_>>().join("\n")
    );
}
