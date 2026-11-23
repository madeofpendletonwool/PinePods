//! The inbox (recent episodes the user hasn't dealt with) and episode labels, including labels
//! as a smart playlist filter.

use crate::harness::*;

fn ids(list: &Value) -> Vec<i32> {
    list.as_array()
        .unwrap_or_else(|| panic!("not a list: {list}"))
        .iter()
        .map(|e| e["episodeid"].as_i64().unwrap() as i32)
        .collect()
}

async fn inbox(app: &TestApp, u: &TestUser, query: &[(&str, &str)]) -> Value {
    let mut req = app.get("/api/data/inbox").key(u);
    for (k, v) in query {
        req = req.query(k, v);
    }
    req.send().await.ok_json()
}

async fn inbox_ids(app: &TestApp, u: &TestUser) -> Vec<i32> {
    ids(&inbox(app, u, &[]).await["episodes"])
}

async fn count(app: &TestApp, u: &TestUser) -> i64 {
    app.get("/api/data/inbox/count").key(u).send().await.ok_json()["count"].as_i64().unwrap()
}

fn refs(eps: &[i32]) -> Value {
    json!({ "episodes": eps.iter().map(|id| json!({ "episode_id": id, "is_youtube": false })).collect::<Vec<_>>() })
}

/// A user subscribed to a four-episode show: the podcast id and episode ids 1..=4.
async fn setup(app: &TestApp, name: &str) -> (TestUser, i32, Vec<i32>) {
    let user = app.user(name).await;
    let url = app.feed(&format!("{name}.xml"), &Feed::with_episodes("Inbox Show", 4));
    let pod = app.subscribe(&user, &url, 4).await;
    let mut eps = Vec::new();
    for n in 1..=4 {
        eps.push(app.episode_id(&user, pod, &format!("Episode {n}")).await);
    }
    (user, pod, eps)
}

#[tokio::test(flavor = "multi_thread")]
async fn new_episodes_start_in_the_inbox_newest_first() {
    let app = TestApp::new().await;
    let (user, _, eps) = setup(&app, "fresh").await;
    let body = inbox(&app, &user, &[]).await;
    assert_eq!(ids(&body["episodes"]), [eps[3], eps[2], eps[1], eps[0]]);
    assert_eq!(body["total"], 4);
    assert_eq!(count(&app, &user).await, 4);
    let oldest = inbox(&app, &user, &[("sort", "oldest"), ("limit", "2"), ("offset", "1")]).await;
    assert_eq!(ids(&oldest["episodes"]), [eps[1], eps[2]]);
    assert_eq!(oldest["total"], 4);
}

#[tokio::test(flavor = "multi_thread")]
async fn acting_on_an_episode_anywhere_takes_it_out() {
    let app = TestApp::new().await;
    let (user, _, eps) = setup(&app, "actor").await;
    let body = |id: i32| json!({ "episode_id": id, "user_id": user.id, "is_youtube": false });

    app.post("/api/data/queue_pod").key(&user).json(&body(eps[0])).send().await.assert_ok();
    app.post("/api/data/save_episode").key(&user).json(&body(eps[1])).send().await.assert_ok();
    app.post("/api/data/mark_episode_completed").key(&user).json(&body(eps[2])).send().await.assert_ok();
    assert_eq!(inbox_ids(&app, &user).await, [eps[3]]);

    // Listening counts too.
    app.post("/api/data/record_listen_duration")
        .key(&user)
        .json(&json!({ "episode_id": eps[3], "user_id": user.id, "listen_duration": 30.0 }))
        .send()
        .await
        .assert_ok();
    assert!(inbox_ids(&app, &user).await.is_empty());

    // Taking it back out of the queue returns it: nothing else has been done with it.
    app.post("/api/data/remove_queued_pod").key(&user).json(&body(eps[0])).send().await.assert_ok();
    assert_eq!(inbox_ids(&app, &user).await, [eps[0]]);
}

#[tokio::test(flavor = "multi_thread")]
async fn dismissing_restoring_and_clearing() {
    let app = TestApp::new().await;
    let (user, pod, eps) = setup(&app, "triage").await;

    let r = app.post("/api/data/inbox/dismiss").key(&user).json(&refs(&[eps[3], eps[2]])).send().await.ok_json();
    assert_eq!(r["changed"], 2);
    // Twice is a no-op.
    let r = app.post("/api/data/inbox/dismiss").key(&user).json(&refs(&[eps[3]])).send().await.ok_json();
    assert_eq!(r["changed"], 0);
    assert_eq!(inbox_ids(&app, &user).await, [eps[1], eps[0]]);

    app.post("/api/data/inbox/restore").key(&user).json(&refs(&[eps[3]])).send().await.assert_ok();
    assert_eq!(inbox_ids(&app, &user).await, [eps[3], eps[1], eps[0]]);

    // Clearing one podcast only touches that podcast.
    let r = app.post("/api/data/inbox/clear").key(&user).json(&json!({ "podcast_id": pod + 1000 })).send().await.ok_json();
    assert_eq!(r["changed"], 0);
    let r = app.post("/api/data/inbox/clear").key(&user).json(&json!({})).send().await.ok_json();
    assert_eq!(r["changed"], 3);
    assert_eq!(count(&app, &user).await, 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_podcast_can_stay_out_and_the_window_is_adjustable() {
    let app = TestApp::new().await;
    let (user, pod, eps) = setup(&app, "window").await;
    let path = format!("/api/data/podcast/{pod}/inbox");

    assert_eq!(app.get(&path).key(&user).send().await.ok_json()["inbox"], true);
    app.put(&path).key(&user).json(&json!({ "inbox": false })).send().await.assert_ok();
    assert_eq!(app.get(&path).key(&user).send().await.ok_json()["inbox"], false);
    assert!(inbox_ids(&app, &user).await.is_empty());
    app.put(&path).key(&user).json(&json!({ "inbox": true })).send().await.assert_ok();
    assert_eq!(inbox_ids(&app, &user).await.len(), 4);

    // Someone else's podcast is not found.
    let other = app.user("other").await;
    app.get(&path).key(&other).send().await.assert_status(404);
    app.put(&path).key(&other).json(&json!({ "inbox": false })).send().await.assert_status(404);

    // The fixture's episodes are about a week old.
    let settings = app.get("/api/data/inbox/settings").key(&user).send().await.ok_json();
    assert_eq!(settings["days"], 30);
    app.put("/api/data/inbox/settings").key(&user).json(&json!({ "days": 3 })).send().await.assert_ok();
    assert!(inbox_ids(&app, &user).await.is_empty());
    app.put("/api/data/inbox/settings").key(&user).json(&json!({ "days": 0 })).send().await.assert_status(400);
    app.put("/api/data/inbox/settings").key(&user).json(&json!({ "days": 365 })).send().await.assert_ok();
    assert_eq!(app.get("/api/data/inbox/settings").key(&user).send().await.ok_json()["days"], 365);
    assert_eq!(inbox_ids(&app, &user).await.len(), eps.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn labels_filter_the_inbox_and_smart_playlists() {
    let app = TestApp::new().await;
    let (user, _, eps) = setup(&app, "labeler").await;

    let later = app.post("/api/data/labels").key(&user).json(&json!({ "name": "Later", "color": "orange" })).send().await.ok_json();
    let later = later["label_id"].as_i64().unwrap() as i32;
    let listen = app.post("/api/data/labels").key(&user).json(&json!({ "name": "Listen" })).send().await.ok_json();
    assert_eq!(listen["color"], "blue");
    let listen = listen["label_id"].as_i64().unwrap() as i32;
    // Names are unique ignoring case; colors come from a fixed set.
    app.post("/api/data/labels").key(&user).json(&json!({ "name": "later" })).send().await.assert_status(409);
    app.post("/api/data/labels").key(&user).json(&json!({ "name": "X", "color": "plaid" })).send().await.assert_status(400);
    app.post("/api/data/labels").key(&user).json(&json!({ "name": "  " })).send().await.assert_status(400);

    let assign = |label: i32, eps: Vec<i32>, assigned: bool| {
        let mut body = refs(&eps);
        body["assigned"] = json!(assigned);
        app.post(&format!("/api/data/labels/{label}/episodes")).key(&user).json(&body)
    };
    assert_eq!(assign(later, vec![eps[0], eps[1]], true).send().await.ok_json()["changed"], 2);
    assign(listen, vec![eps[2]], true).send().await.assert_ok();

    let assignments = app.get("/api/data/labels/assignments").key(&user).send().await.ok_json();
    assert_eq!(assignments.as_array().unwrap().len(), 3);
    let labels = app.get("/api/data/labels").key(&user).send().await.ok_json();
    assert_eq!(labels[0]["name"], "Later");
    assert_eq!(labels[0]["episode_count"], 2);

    // Labels don't take an episode out of the inbox; they filter it.
    assert_eq!(inbox_ids(&app, &user).await.len(), 4);
    let later_q = later.to_string();
    assert_eq!(ids(&inbox(&app, &user, &[("label_id", later_q.as_str())]).await["episodes"]), [eps[1], eps[0]]);
    assert_eq!(ids(&inbox(&app, &user, &[("unlabeled", "true")]).await["episodes"]), [eps[3]]);

    // Clearing under a filter only dismisses what it shows.
    let r = app.post("/api/data/inbox/clear").key(&user).json(&json!({ "unlabeled": true })).send().await.ok_json();
    assert_eq!(r["changed"], 1);
    assert_eq!(inbox_ids(&app, &user).await, [eps[2], eps[1], eps[0]]);

    // A smart playlist limited to a label.
    let created = app
        .post("/api/data/create_playlist")
        .key(&user)
        .json(&json!({
            "user_id": user.id, "name": "Later list", "include_unplayed": true, "include_partially_played": true,
            "include_played": false, "sort_order": "date_asc", "group_by_podcast": false, "icon_name": "ph-playlist",
            "label_ids": [later],
        }))
        .send()
        .await
        .ok_json();
    let playlist = created["playlist_id"].as_i64().unwrap() as i32;
    let body = app
        .get("/api/data/get_playlist_episodes")
        .key(&user)
        .query("user_id", user.id)
        .query("playlist_id", playlist)
        .send()
        .await
        .ok_json();
    assert_eq!(ids(&body["episodes"]), [eps[0], eps[1]]);
    assert_eq!(body["playlist_info"]["label_ids"], json!([later]));
    let all = app.get("/api/data/get_playlists").key(&user).query("user_id", user.id).send().await.ok_json();
    let mine = all["playlists"].as_array().unwrap().iter().find(|p| p["playlist_id"] == playlist).unwrap().clone();
    assert_eq!(mine["label_ids"], json!([later]));
    assert_eq!(mine["episode_count"], 2);

    // Taking a label off, renaming and deleting.
    assign(later, vec![eps[0]], false).send().await.assert_ok();
    assert_eq!(ids(&inbox(&app, &user, &[("label_id", later_q.as_str())]).await["episodes"]), [eps[1]]);
    app.put(&format!("/api/data/labels/{later}")).key(&user).json(&json!({ "name": "Listen", "color": "red" })).send().await.assert_status(409);
    app.put(&format!("/api/data/labels/{later}")).key(&user).json(&json!({ "name": "Someday", "color": "red" })).send().await.assert_ok();
    app.delete(&format!("/api/data/labels/{later}")).key(&user).send().await.assert_ok();
    let labels = app.get("/api/data/labels").key(&user).send().await.ok_json();
    assert_eq!(labels.as_array().unwrap().len(), 1);
    assert_eq!(app.get("/api/data/labels/assignments").key(&user).send().await.ok_json().as_array().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn labels_and_inboxes_are_per_user() {
    let app = TestApp::new().await;
    let (owner, _, eps) = setup(&app, "owner").await;
    let intruder = app.user("intruder").await;

    let label = app.post("/api/data/labels").key(&owner).json(&json!({ "name": "Mine" })).send().await.ok_json();
    let label = label["label_id"].as_i64().unwrap() as i32;
    let mut body = refs(&[eps[0]]);
    body["assigned"] = json!(true);
    app.post(&format!("/api/data/labels/{label}/episodes")).key(&intruder).json(&body).send().await.assert_status(404);
    app.put(&format!("/api/data/labels/{label}")).key(&intruder).json(&json!({ "name": "Theirs" })).send().await.assert_status(404);
    app.delete(&format!("/api/data/labels/{label}")).key(&intruder).send().await.assert_status(404);

    // The intruder's own label can't go on the owner's episode.
    let theirs = app.post("/api/data/labels").key(&intruder).json(&json!({ "name": "Mine" })).send().await.ok_json();
    let theirs = theirs["label_id"].as_i64().unwrap() as i32;
    app.post(&format!("/api/data/labels/{theirs}/episodes")).key(&intruder).json(&body).send().await.assert_status(404);

    // Dismissing someone else's episode only changes the caller's own inbox.
    app.post("/api/data/inbox/dismiss").key(&intruder).json(&refs(&eps)).send().await.assert_ok();
    assert_eq!(inbox_ids(&app, &owner).await.len(), 4);
    assert!(inbox_ids(&app, &intruder).await.is_empty());
}
