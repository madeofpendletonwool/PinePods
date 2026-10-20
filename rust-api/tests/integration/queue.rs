//! The "now playing + up next" queue: position 1 plays next.

use crate::harness::*;

/// A user subscribed to a 5-episode show; returns (user, episode ids for "Episode 1..=5").
async fn setup(app: &TestApp) -> (TestUser, Vec<i32>) {
    let user = app.user("listener").await;
    let url = app.feed("queue-show.xml", &Feed::with_episodes("Queue Show", 5));
    let pod = app.subscribe(&user, &url, 5).await;
    let mut ids = Vec::new();
    for n in 1..=5 {
        ids.push(app.episode_id(&user, pod, &format!("Episode {n}")).await);
    }
    (user, ids)
}

async fn queue(app: &TestApp, user: &TestUser, episode: i32, playing: Option<i32>) {
    let mut body = json!({ "episode_id": episode, "user_id": user.id, "is_youtube": false });
    if let Some(p) = playing {
        body["playing_episode_id"] = json!(p);
        body["playing_is_youtube"] = json!(false);
    }
    app.post("/api/data/queue_pod").key(user).json(&body).send().await.assert_ok();
}

/// Queued episode ids in play order, asserting positions are 1..=n with no gaps.
async fn queued(app: &TestApp, user: &TestUser) -> Vec<i32> {
    let body = app.get("/api/data/get_queued_episodes").key(user).query("user_id", user.id).send().await.ok_json();
    let mut rows: Vec<(i64, i32)> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| (e["queueposition"].as_i64().unwrap(), e["episodeid"].as_i64().unwrap() as i32))
        .collect();
    rows.sort();
    let positions: Vec<i64> = rows.iter().map(|r| r.0).collect();
    assert_eq!(positions, (1..=rows.len() as i64).collect::<Vec<_>>(), "queue positions must be contiguous: {body}");
    rows.into_iter().map(|r| r.1).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_with_nothing_playing_puts_the_episode_next() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    queue(&app, &user, ep[0], None).await;
    queue(&app, &user, ep[1], None).await;
    queue(&app, &user, ep[2], None).await;
    assert_eq!(queued(&app, &user).await, [ep[2], ep[1], ep[0]], "latest add plays next");
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_while_playing_slots_in_right_after_the_playing_episode() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    for &e in &[ep[2], ep[1], ep[0]] {
        queue(&app, &user, e, None).await;
    }
    // Queue is [0, 1, 2]; episode 0 is playing.
    queue(&app, &user, ep[3], Some(ep[0])).await;
    assert_eq!(queued(&app, &user).await, [ep[0], ep[3], ep[1], ep[2]]);

    // A playing anchor that isn't in the queue falls back to the top.
    queue(&app, &user, ep[4], Some(999_999_999)).await;
    assert_eq!(queued(&app, &user).await[0], ep[4]);
}

#[tokio::test(flavor = "multi_thread")]
async fn queueing_the_same_episode_twice_does_not_duplicate_it() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    queue(&app, &user, ep[0], None).await;
    queue(&app, &user, ep[0], None).await;
    assert_eq!(queued(&app, &user).await, [ep[0]]);
}

#[tokio::test(flavor = "multi_thread")]
async fn bump_moves_an_episode_to_the_top() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    for &e in &[ep[2], ep[1], ep[0]] {
        queue(&app, &user, e, None).await;
    }
    app.post("/api/data/queue_bump")
        .key(&user)
        .json(&json!({ "episode_id": ep[2], "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(queued(&app, &user).await, [ep[2], ep[0], ep[1]]);
}

#[tokio::test(flavor = "multi_thread")]
async fn reorder_applies_the_exact_order_given() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    for &e in &ep[..4] {
        queue(&app, &user, e, None).await;
    }
    let order = [ep[1], ep[3], ep[0], ep[2]];
    let items: Vec<Value> = order.iter().map(|id| json!({ "episode_id": id, "is_youtube": false })).collect();
    app.post("/api/data/reorder_queue")
        .key(&user)
        .query("user_id", user.id)
        .json(&json!({ "episodes": items }))
        .send()
        .await
        .assert_ok();
    assert_eq!(queued(&app, &user).await, order);

    // Legacy id-only clients still work.
    let legacy = [ep[2], ep[0], ep[3], ep[1]];
    app.post("/api/data/reorder_queue")
        .key(&user)
        .query("user_id", user.id)
        .json(&json!({ "episode_ids": legacy }))
        .send()
        .await
        .assert_ok();
    assert_eq!(queued(&app, &user).await, legacy);
}

#[tokio::test(flavor = "multi_thread")]
async fn removing_closes_the_gap() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    for &e in &[ep[2], ep[1], ep[0]] {
        queue(&app, &user, e, None).await;
    }
    app.post("/api/data/remove_queued_pod")
        .key(&user)
        .json(&json!({ "episode_id": ep[1], "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(queued(&app, &user).await, [ep[0], ep[2]]);
}

#[tokio::test(flavor = "multi_thread")]
async fn completing_an_episode_takes_it_out_of_the_queue() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    for &e in &[ep[1], ep[0]] {
        queue(&app, &user, e, None).await;
    }
    app.post("/api/data/mark_episode_completed")
        .key(&user)
        .json(&json!({ "episode_id": ep[0], "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(queued(&app, &user).await, [ep[1]]);
}

#[tokio::test(flavor = "multi_thread")]
async fn clear_empties_only_this_users_queue() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    let other = app.user("other").await;
    let url = app.feed("other.xml", &Feed::with_episodes("Other Show", 1));
    let other_pod = app.subscribe(&other, &url, 1).await;
    let other_ep = app.episode_id(&other, other_pod, "Episode 1").await;
    queue(&app, &other, other_ep, None).await;

    for &e in &ep[..3] {
        queue(&app, &user, e, None).await;
    }
    app.post("/api/data/clear_queue").key(&user).json(&json!({ "user_id": user.id })).send().await.assert_ok();
    assert!(queued(&app, &user).await.is_empty());
    assert_eq!(queued(&app, &other).await, [other_ep]);
}

#[tokio::test(flavor = "multi_thread")]
async fn bulk_queue_lands_the_batch_on_top_in_order() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    queue(&app, &user, ep[4], None).await;
    app.post("/api/data/bulk_queue_episodes")
        .key(&user)
        .json(&json!({ "episode_ids": [ep[0], ep[1], ep[2]], "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(queued(&app, &user).await, [ep[0], ep[1], ep[2], ep[4]]);
}

#[tokio::test(flavor = "multi_thread")]
async fn queued_flag_shows_on_episode_listings() {
    let app = TestApp::new().await;
    let (user, ep) = setup(&app).await;
    queue(&app, &user, ep[0], None).await;
    let pods = app.podcasts(&user).await;
    let pod = pods[0]["podcastid"].as_i64().unwrap() as i32;
    let eps = app.episodes(&user, pod).await;
    let flag = |id: i32| eps.iter().find(|e| e["Episodeid"] == id).unwrap()["queued"].clone();
    assert_eq!(flag(ep[0]), true);
    assert_eq!(flag(ep[1]), false);
}
