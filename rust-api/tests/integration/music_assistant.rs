//! What a Music Assistant speaker plays after an episode: playlist, auto-play-next, queue.

use crate::harness::*;
use pinepods_api::services::music_assistant::{next_pinepods_episode, NextEpisode};

async fn setup(app: &TestApp) -> (TestUser, i32, Vec<i32>) {
    let user = app.user("speaker").await;
    let url = app.feed("ma-next.xml", &Feed::with_episodes("Speaker Show", 4));
    let pod = app.subscribe(&user, &url, 4).await;
    let mut ids = Vec::new();
    for n in 1..=4 {
        ids.push(app.episode_id(&user, pod, &format!("Episode {n}")).await);
    }
    (user, pod, ids)
}

/// Queue `ids` in that order.
async fn queue_in_order(app: &TestApp, user: &TestUser, ids: &[i32]) {
    let mut playing: Option<i32> = None;
    for &id in ids {
        let mut body = json!({ "episode_id": id, "user_id": user.id, "is_youtube": false });
        if let Some(p) = playing {
            body["playing_episode_id"] = json!(p);
            body["playing_is_youtube"] = json!(false);
        }
        app.post("/api/data/queue_pod")
            .key(user)
            .json(&body)
            .send()
            .await
            .assert_ok();
        playing = Some(id);
    }
}

async fn next(
    app: &TestApp,
    user: &TestUser,
    ep: i32,
    playlist: Option<i32>,
    skip: bool,
) -> Option<NextEpisode> {
    next_pinepods_episode(app.db(), user.id, (ep, false), playlist, skip).await
}

fn episode(id: i32, playlist_id: Option<i32>) -> Option<NextEpisode> {
    Some(NextEpisode {
        episode_id: id,
        is_youtube: false,
        playlist_id,
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn finished_episode_moves_to_the_next_queued_one_even_after_completion() {
    let app = TestApp::new().await;
    let (user, _pod, ids) = setup(&app).await;
    queue_in_order(&app, &user, &[ids[0], ids[1], ids[2]]).await;

    // Read before completion, the way the bridge does.
    assert_eq!(
        next(&app, &user, ids[0], None, false).await,
        episode(ids[1], None)
    );
    app.db()
        .mark_episode_completed(ids[0], user.id, false, pinepods_api::services::sync::SyncOrigin::Pinepods)
        .await
        .unwrap();
    // Completion took the finished row out of the queue; the answer is the same.
    assert_eq!(
        next(&app, &user, ids[0], None, false).await,
        episode(ids[1], None)
    );
    assert_eq!(
        next(&app, &user, ids[2], None, false).await,
        episode(ids[1], None)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn skip_takes_the_row_after_the_current_one_and_stops_at_the_end() {
    let app = TestApp::new().await;
    let (user, _pod, ids) = setup(&app).await;
    queue_in_order(&app, &user, &[ids[0], ids[1], ids[2]]).await;

    assert_eq!(
        next(&app, &user, ids[1], None, true).await,
        episode(ids[2], None)
    );
    assert_eq!(next(&app, &user, ids[2], None, true).await, None);
    assert_eq!(
        next(&app, &user, ids[3], None, true).await,
        episode(ids[0], None)
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_play_next_beats_the_queue_and_matches_local_web() {
    let app = TestApp::new().await;
    let (user, pod, ids) = setup(&app).await;
    queue_in_order(&app, &user, &[ids[3]]).await;
    app.post("/api/data/enable_auto_play_next")
        .key(&user)
        .json(&json!({ "podcast_id": pod, "auto_play_next": true, "user_id": user.id }))
        .send()
        .await
        .assert_ok();

    let web = app
        .post("/api/data/get_next_podcast_episode")
        .key(&user)
        .json(&json!({ "episode_id": ids[1], "user_id": user.id }))
        .send()
        .await
        .ok_json();
    let expected = web["episodeid"]
        .as_i64()
        .expect("podcast has a next episode") as i32;
    assert_eq!(
        next(&app, &user, ids[1], None, false).await,
        episode(expected, None)
    );

    // A video id is not looked up as a podcast episode.
    let video = next_pinepods_episode(app.db(), user.id, (ids[1], true), None, false).await;
    assert_eq!(video, episode(ids[3], None));
}

#[tokio::test(flavor = "multi_thread")]
async fn playlist_next_comes_first_and_keeps_the_playlist() {
    let app = TestApp::new().await;
    let (user, pod, ids) = setup(&app).await;
    let created = app
        .post("/api/data/create_playlist")
        .key(&user)
        .json(&json!({
            "user_id": user.id,
            "name": "Speaker list",
            "include_unplayed": true,
            "include_partially_played": true,
            "include_played": false,
            "sort_order": "date_desc",
            "group_by_podcast": false,
            "icon_name": "ph-playlist",
            "podcast_ids": [pod],
        }))
        .send()
        .await
        .ok_json();
    let playlist = created["playlist_id"].as_i64().unwrap() as i32;
    let shown = app
        .get("/api/data/get_playlist_episodes")
        .key(&user)
        .query("user_id", user.id)
        .query("playlist_id", playlist)
        .query("limit", 100)
        .send()
        .await
        .ok_json();
    let first = shown["episodes"][0]["episodeid"]
        .as_i64()
        .unwrap_or_else(|| panic!("playlist rows: {shown}")) as i32;

    let web = app
        .post("/api/data/get_next_playlist_episode")
        .key(&user)
        .json(&json!({ "episode_id": first, "playlist_id": playlist, "user_id": user.id }))
        .send()
        .await
        .ok_json();
    let expected = web["episodeid"]
        .as_i64()
        .expect("playlist has a next episode") as i32;
    assert_ne!(expected, first);
    // Queue something else so the two answers differ.
    let queued = *ids
        .iter()
        .find(|id| **id != first && **id != expected)
        .unwrap();
    queue_in_order(&app, &user, &[queued]).await;
    assert_eq!(
        next(&app, &user, first, Some(playlist), false).await,
        episode(expected, Some(playlist))
    );
    // Without the playlist context, the queue decides.
    assert_eq!(
        next(&app, &user, first, None, false).await,
        episode(queued, None)
    );
}
