//! A user can't reach another user's *things* by id.
//!
//! cross_user swaps the user id; this swaps the resource ids. The victim gets one of
//! everything (a private podcast + episodes, collection, playlist, custom theme, extra API
//! key). The attacker then calls every secured operation that addresses a resource, with
//! their *own* user id but the victim's resource ids.
//!
//! Many handlers scope their SQL by the caller and simply touch nothing (often still
//! answering 200), so status codes prove little. Instead every probe is judged on evidence:
//!
//! * **leak** — the attacker's response contains the victim's secret strings;
//! * **tamper** — the victim's own view of their account changed;
//! * **contaminate** — the victim's private episodes now show up in the attacker's account
//!   (queue, saved, history, collections, playlists, home, feed).

use std::collections::BTreeMap;

use crate::harness::*;
use crate::support::openapi::{Operation, Spec};

/// Operations that legitimately accept another user's resource id. Each needs a reason.
const ALLOWED: &[(&str, &str)] = &[];

/// What the victim looks at to notice tampering. Synthesised from the spec as the victim.
const VICTIM_VIEWS: &[&str] = &[
    "GET /api/data/return_pods/{user_id}",
    "GET /api/data/podcast_episodes",
    "POST /api/data/get_play_episode_details",
    "POST /api/data/get_auto_download_status",
    "POST /api/data/podcast/notification_status",
    "POST /api/data/podcast/favorite_status",
    "GET /api/data/get_silence_trim",
    "GET /api/data/get_auto_transcribe",
    "GET /api/data/get_auto_ad_detect",
    "GET /api/data/get_ad_skip_auto_activate",
    "GET /api/data/get_feed_cutoff_days",
    "GET /api/data/{podcast_id}/merged",
    "GET /api/data/collections/user/{user_id}",
    "GET /api/data/collections/{collection_id}/episodes",
    "GET /api/data/get_playlists",
    "GET /api/data/user/custom_themes/{user_id}",
    "GET /api/data/get_api_info/{user_id}",
    "GET /api/data/get_queued_episodes",
    "GET /api/data/saved_episode_list/{user_id}",
    "GET /api/data/user_history/{user_id}",
    "GET /api/data/get_user_shared_links/{user_id}",
    "GET /api/sync/devices",
    "GET /api/sync/server/app-passwords",
];

/// Where the victim's episodes would surface if they leaked into the attacker's account.
const ATTACKER_VIEWS: &[&str] = &[
    "GET /api/data/get_queued_episodes",
    "GET /api/data/saved_episode_list/{user_id}",
    "GET /api/data/user_history/{user_id}",
    "GET /api/data/home_overview",
    "GET /api/data/return_episodes/{user_id}",
    "GET /api/data/get_user_shared_links/{user_id}",
];

#[derive(Clone, Copy)]
struct Ids {
    user: i32,
    podcast: i32,
    episode: i32,
    collection: i32,
    playlist: i32,
    theme: i32,
    api_key: i32,
    device: i32,
    app_password: i32,
}

fn overrides(ids: Ids) -> impl Fn(&str) -> Option<Value> {
    move |name: &str| match name {
        "user_id" | "userid" => Some(json!(ids.user)),
        "podcast_id" | "target_podcast_id" => Some(json!(ids.podcast)),
        "episode_id" | "playing_episode_id" => Some(json!(ids.episode)),
        "episode_ids" => Some(json!([ids.episode])),
        "podcast_ids" | "secondary_podcast_ids" => Some(json!([ids.podcast])),
        "collection_id" => Some(json!(ids.collection)),
        "playlist_id" => Some(json!(ids.playlist)),
        "theme_id" => Some(json!(ids.theme)),
        "api_id" => Some(json!(ids.api_key)),
        "device_id" => Some(json!(ids.device)),
        "app_password_id" => Some(json!(ids.app_password)),
        "limit" => Some(json!(100)),
        "offset" => Some(json!(0)),
        "sort_by" => Some(json!("date")),
        "sort_order" => Some(json!("desc")),
        "filter" => Some(json!("all")),
        _ => None,
    }
}

async fn call(app: &TestApp, spec: &Spec, op: &Operation, who: &TestUser, ids: Ids) -> crate::harness::http::TestResponse {
    let ov = overrides(ids);
    let mut req = app.request(op.method.clone(), &spec.uri(op, &ov)).key(who);
    if let Some((bytes, ct)) = spec.body(op, &ov) {
        req = req.raw_body(bytes, ct);
    }
    req.send().await
}

fn find_op(spec: &Spec, label: &str) -> Operation {
    spec.operations().into_iter().find(|o| o.label() == label).unwrap_or_else(|| panic!("{label} not in spec"))
}

/// Find the id of the object in `v` that has a string field equal to `marker`.
fn id_of_named(v: &Value, marker: &str) -> Option<i32> {
    match v {
        Value::Object(o) => {
            if o.values().any(|x| x.as_str() == Some(marker)) {
                return o
                    .iter()
                    .find(|(k, x)| k.to_lowercase().ends_with("id") && !k.to_lowercase().contains("user") && x.is_i64())
                    .map(|(_, x)| x.as_i64().unwrap() as i32);
            }
            o.values().find_map(|x| id_of_named(x, marker))
        }
        Value::Array(a) => a.iter().find_map(|x| id_of_named(x, marker)),
        _ => None,
    }
}

/// Largest value of any `api_id`-like field (the key we just created).
fn newest_api_id(v: &Value) -> Option<i32> {
    fn walk(v: &Value, best: &mut Option<i32>) {
        match v {
            Value::Object(o) => {
                for (k, x) in o {
                    if matches!(k.to_lowercase().as_str(), "apikeyid" | "api_id" | "apiid") && x.is_i64() {
                        let id = x.as_i64().unwrap() as i32;
                        *best = Some(best.map_or(id, |b| b.max(id)));
                    }
                    walk(x, best);
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, best)),
            _ => {}
        }
    }
    let mut best = None;
    walk(v, &mut best);
    best
}

/// Volatile fields that legitimately change between reads.
fn normalise(v: &mut Value) {
    match v {
        Value::Object(o) => {
            o.retain(|k, _| !matches!(k.as_str(), "last_updated" | "lastupdated" | "task_id" | "timestamp"));
            o.values_mut().for_each(normalise);
        }
        Value::Array(a) => a.iter_mut().for_each(normalise),
        _ => {}
    }
}

async fn snapshot(app: &TestApp, spec: &Spec, labels: &[&str], who: &TestUser, ids: Ids) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for label in labels {
        let resp = call(app, spec, &find_op(spec, label), who, ids).await;
        let mut v = serde_json::from_slice::<Value>(&resp.body).unwrap_or(Value::String(resp.text()));
        normalise(&mut v);
        out.insert(label.to_string(), format!("{} {}", resp.status.as_u16(), v));
    }
    out
}

fn resource_addressed(spec: &Spec, op: &Operation) -> bool {
    let marker = |name: &str| match name {
        "podcast_id" | "target_podcast_id" | "episode_id" | "playing_episode_id" | "collection_id" | "playlist_id"
        | "theme_id" | "api_id" | "device_id" | "app_password_id" => Some(json!(-777)),
        "episode_ids" | "podcast_ids" | "secondary_podcast_ids" => Some(json!([-777])),
        _ => None,
    };
    let in_params = op
        .op
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|p| marker(p["name"].as_str().unwrap_or("")).is_some());
    in_params || spec.body(op, &marker).is_some_and(|(b, _)| String::from_utf8_lossy(&b).contains("-777"))
}

#[tokio::test(flavor = "multi_thread")]
async fn users_cannot_reach_each_others_resources_by_id() {
    let app = TestApp::new().await;
    let spec = Spec::load();
    let victim = app.user("victim").await;
    let attacker = app.user("attacker").await;

    // --- the victim's private world -----------------------------------------------------
    let secret = format!("s3cr3t{}", app.tag);
    let mut feed = Feed::new(&format!("Private Show {secret}"));
    for n in 1..=2 {
        feed = feed.episode(
            Episode::new(n)
                .title(&format!("Private episode {n} {secret}"))
                .enclosure(&format!("https://cdn.example.invalid/{secret}/ep{n}.mp3")),
        );
    }
    let url = app.feed("victim.xml", &feed);
    let podcast = app.subscribe(&victim, &url, 2).await;
    let episode = app.episode_id(&victim, podcast, &format!("Private episode 1 {secret}")).await;

    let partial = Ids { user: victim.id, podcast, episode, collection: 0, playlist: 0, theme: 0, api_key: 0, device: 0, app_password: 0 };
    for label in [
        "POST /api/data/collections/create",
        "POST /api/data/create_playlist",
        "POST /api/data/user/custom_themes",
        "POST /api/data/create_api_key",
    ] {
        let op = find_op(&spec, label);
        let name = format!("victim-{}", app.tag);
        let ov = move |field: &str| match field {
            "name" => Some(json!(name.clone())),
            "rssonly" => Some(json!(false)),
            "podcast_ids" => Some(Value::Null),
            "sort_order" => Some(json!("date_desc")),
            other => overrides(partial)(other),
        };
        let (bytes, ct) = spec.body(&op, &ov).unwrap();
        app.request(op.method.clone(), &spec.uri(&op, &ov)).key(&victim).raw_body(bytes, ct).send().await.assert_ok();
    }
    let marker = format!("victim-{}", app.tag);
    let fetch = |path: String| {
        let victim = victim.clone();
        let app = &app;
        async move { app.get(&path).key(&victim).query("user_id", victim.id).send().await.ok_json() }
    };
    let collections = fetch(format!("/api/data/collections/user/{}", victim.id)).await;
    let playlists = fetch("/api/data/get_playlists".into()).await;
    let themes = fetch(format!("/api/data/user/custom_themes/{}", victim.id)).await;
    let keys = fetch(format!("/api/data/get_api_info/{}", victim.id)).await;
    // A gpodder app on the victim's PinePods server, and an app password for it.
    app.put("/api/sync/server").key(&victim).json(&json!({ "enabled": true })).send().await.assert_ok();
    exec(&app, &format!(r#"INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType) VALUES ({}, 'victim-phone', 'mobile')"#, victim.id)).await;
    let app_password = app.post("/api/sync/server/app-passwords").key(&victim).json(&json!({ "label": "victim" })).send().await.ok_json();
    let devices = fetch("/api/sync/devices".into()).await;
    let v_ids = Ids {
        collection: id_of_named(&collections, &marker).unwrap_or_else(|| panic!("collection id in {collections}")),
        playlist: id_of_named(&playlists, &marker).unwrap_or_else(|| panic!("playlist id in {playlists}")),
        theme: id_of_named(&themes, &marker).unwrap_or_else(|| panic!("theme id in {themes}")),
        api_key: newest_api_id(&keys).unwrap_or_else(|| panic!("api key id in {keys}")),
        device: devices
            .as_array()
            .and_then(|d| d.iter().find(|d| d["name"] == "victim-phone"))
            .and_then(|d| d["id"].as_i64())
            .unwrap_or_else(|| panic!("device id in {devices}")) as i32,
        app_password: app_password["id"].as_i64().unwrap() as i32,
        ..partial
    };
    // Put the private episode in the victim's queue/saved/collection so tampering is visible.
    app.post("/api/data/save_episode").key(&victim).json(&json!({"episode_id": episode, "user_id": victim.id, "is_youtube": false})).send().await.assert_ok();
    app.post("/api/data/queue_pod").key(&victim).json(&json!({"episode_id": episode, "user_id": victim.id, "is_youtube": false})).send().await.assert_ok();

    // --- probe ---------------------------------------------------------------------------
    let a_ids = Ids { user: attacker.id, ..v_ids };
    let attacker_self = Ids { user: attacker.id, podcast: 0, episode: 0, collection: 0, playlist: 0, theme: 0, api_key: 0, device: 0, app_password: 0 };

    let mut ops: Vec<Operation> =
        spec.operations().into_iter().filter(|o| o.secured && resource_addressed(&spec, o)).collect();
    ops.sort_by_key(|o| o.method == axum::http::Method::DELETE || o.template.contains("remove") || o.template.contains("delete"));

    let mut before = snapshot(&app, &spec, VICTIM_VIEWS, &victim, v_ids).await;
    let mut mine_before = snapshot(&app, &spec, ATTACKER_VIEWS, &attacker, attacker_self).await;
    let mut failures: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for op in &ops {
        if ALLOWED.iter().any(|(l, _)| *l == op.label()) {
            continue;
        }
        let resp = call(&app, &spec, op, &attacker, a_ids).await;
        if resp.text().contains(&secret) {
            failures.entry(op.label()).or_default().push(format!("leak: {} {}", resp.status, resp.text().chars().take(160).collect::<String>()));
        }

        let after = snapshot(&app, &spec, VICTIM_VIEWS, &victim, v_ids).await;
        for (view, was) in &before {
            if after.get(view) != Some(was) {
                failures.entry(op.label()).or_default().push(format!(
                    "tamper: victim's {view} changed\n        before: {}\n        after:  {}",
                    was.chars().take(200).collect::<String>(),
                    after[view].chars().take(200).collect::<String>()
                ));
            }
        }
        before = after;

        let mine = snapshot(&app, &spec, ATTACKER_VIEWS, &attacker, attacker_self).await;
        for (view, body) in &mine {
            let was_clean = !mine_before.get(view).is_some_and(|b| b.contains(&secret));
            if body.contains(&secret) && was_clean {
                failures.entry(op.label()).or_default().push(format!("contaminate: attacker's {view} now shows the victim's episode"));
            }
        }
        mine_before = mine;
    }

    eprintln!("foreign_resources: probed {} resource-addressed operations", ops.len());
    assert!(ops.len() > 50, "only {} resource-addressed operations found; spec walk is broken", ops.len());
    assert!(
        failures.is_empty(),
        "{} operation(s) let a user reach another user's resources:\n{}",
        failures.len(),
        failures.iter().map(|(k, v)| format!("  {k}:\n    {}", v.join("\n    "))).collect::<Vec<_>>().join("\n")
    );
}
