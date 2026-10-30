//! Episodes shared by the subscribers of a feed: one row per episode, each user seeing it under
//! their own podcast, refreshes inserting once and handing new episodes to every subscriber,
//! unsubscribing touching only the leaver's state, the upgrade merging the copies users had, and
//! old episode ids still working afterwards.

use pinepods_api::services::sync::SyncOrigin;

use crate::harness::*;

async fn count(app: &TestApp, sql: String) -> i64 {
    int(app, &sql).await.unwrap_or(0)
}

async fn feed_episodes(app: &TestApp, pod: i32) -> i64 {
    count(app, format!(r#"SELECT COUNT(*) FROM "Episodes" WHERE FeedID = (SELECT FeedID FROM "Podcasts" WHERE PodcastID = {pod})"#)).await
}

async fn recent(app: &TestApp, user: &TestUser) -> Vec<Value> {
    let body = app.get(&format!("/api/data/return_episodes/{}", user.id)).key(user).send().await.ok_json();
    body["episodes"].as_array().cloned().unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn subscribers_share_episodes_under_their_own_podcast() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 3));
    let a = app.subscribe(&alice, &url, 3).await;
    let b = app.subscribe(&bob, &url, 3).await;

    assert_eq!(feed_episodes(&app, a).await, 3, "one row per episode, not per subscriber");
    assert_eq!(count(&app, format!(r#"SELECT EpisodeCount FROM "Podcasts" WHERE PodcastID = {b}"#)).await, 3);
    for (user, pod) in [(&alice, a), (&bob, b)] {
        let eps = recent(&app, user).await;
        assert_eq!(eps.len(), 3, "{eps:#?}");
        assert!(eps.iter().all(|e| e["podcastid"] == pod), "podcastid is the user's own podcast: {eps:#?}");
    }
    assert_eq!(app.episode_id(&alice, a, "Episode 2").await, app.episode_id(&bob, b, "Episode 2").await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refresh_inserts_once_and_hands_new_episodes_to_each_subscriber() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let feed = Feed::with_episodes("Show", 2);
    let url = app.feed("show.xml", &feed);
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;
    exec(&app, &format!(r#"UPDATE "Podcasts" SET AutoQueue = TRUE WHERE PodcastID = {b}"#)).await;

    app.feed("show.xml", &feed.episode(Episode::new(3)));
    set_next_refresh(&app, a, "2000-01-01 00:00:00").await;
    assert_eq!(pinepods_api::handlers::refresh::refresh_due_podcasts(&app.state).await.unwrap(), 1);

    assert_eq!(feed_episodes(&app, a).await, 3);
    let new = app.episode_id(&alice, a, "Episode 3").await;
    assert_eq!(app.episode_id(&bob, b, "Episode 3").await, new);
    let queued = |user: &TestUser| format!(r#"SELECT COUNT(*) FROM "EpisodeQueue" WHERE UserID = {} AND EpisodeID = {new}"#, user.id);
    assert_eq!(count(&app, queued(&bob)).await, 1, "auto-queued for the subscriber who asked");
    assert_eq!(count(&app, queued(&alice)).await, 0);
    for pod in [a, b] {
        assert_eq!(count(&app, format!(r#"SELECT EpisodeCount FROM "Podcasts" WHERE PodcastID = {pod}"#)).await, 3);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn unsubscribing_keeps_the_episodes_and_state_of_the_other_subscribers() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 2));
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;
    let e1 = app.episode_id(&alice, a, "Episode 1").await;
    for user in [&alice, &bob] {
        app.post("/api/data/save_episode").key(user).json(&json!({"episode_id": e1, "user_id": user.id, "is_youtube": false})).send().await.assert_ok();
        app.db().mark_episode_completed(e1, user.id, false, SyncOrigin::Unlogged).await.unwrap();
    }
    let saved = |user: &TestUser| format!(r#"SELECT COUNT(*) FROM "SavedEpisodes" WHERE UserID = {}"#, user.id);
    let history = |user: &TestUser| format!(r#"SELECT COUNT(*) FROM "UserEpisodeHistory" WHERE UserID = {}"#, user.id);

    app.db().remove_podcast_id(a, alice.id, SyncOrigin::Unlogged).await.unwrap();
    assert_eq!(feed_episodes(&app, b).await, 2);
    assert_eq!((count(&app, saved(&alice)).await, count(&app, history(&alice)).await), (0, 0), "alice's state went with her");
    assert_eq!((count(&app, saved(&bob)).await, count(&app, history(&bob)).await), (1, 1), "bob's stayed");
    let eps = app.episodes(&bob, b).await;
    assert_eq!(eps.len(), 2);

    app.db().remove_podcast_id(b, bob.id, SyncOrigin::Unlogged).await.unwrap();
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "Episodes" WHERE EpisodeID = {e1}"#)).await, 0, "the last subscriber takes them");
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrading_merges_the_copies_and_old_ids_keep_working() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 2));
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;
    let (e1, e2) = (app.episode_id(&alice, a, "Episode 1").await, app.episode_id(&alice, a, "Episode 2").await);

    // As before the upgrade: bob has his own copies, with no feed.
    match app.backend() {
        Backend::Postgres => exec(&app, "DROP INDEX idx_episodes_feedid_guid_unique").await,
        Backend::MariaDb => exec(&app, "DROP INDEX idx_episodes_feedid_guidhash_unique ON Episodes").await,
    }
    exec(
        &app,
        &format!(
            r#"INSERT INTO "Episodes" (PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration, EpisodeGUID)
               SELECT {b}, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration, EpisodeGUID
               FROM "Episodes" WHERE EpisodeID IN ({e1}, {e2}) ORDER BY EpisodeID"#
        ),
    )
    .await;
    let copy = |guid: &str| format!(r#"SELECT EpisodeID FROM "Episodes" WHERE PodcastID = {b} AND FeedID IS NULL AND EpisodeGUID = '{guid}'"#);
    let c1 = int(&app, &copy("guid-1")).await.unwrap();
    let c2 = int(&app, &copy("guid-2")).await.unwrap();
    let (uid_a, uid_b) = (alice.id, bob.id);
    for sql in [
        // Alice listened to the original, bob to both the original (earlier) and his copy.
        format!(r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES ({uid_a}, {e1}, '2026-03-01 10:00:00', 100)"#),
        format!(r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES ({uid_b}, {e1}, '2026-03-01 09:00:00', 50)"#),
        format!(r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES ({uid_b}, {c1}, '2026-03-02 10:00:00', 300)"#),
        format!(r#"INSERT INTO "UserEpisodeState" (UserID, EpisodeID, Completed) VALUES ({uid_b}, {c2}, TRUE)"#),
        format!(r#"INSERT INTO "SavedEpisodes" (UserID, EpisodeID, SaveDate) VALUES ({uid_b}, {c1}, '2026-03-01 10:00:00')"#),
        format!(r#"INSERT INTO "EpisodeQueue" (QueueDate, UserID, EpisodeID, QueuePosition, is_youtube) VALUES ('2026-03-01 10:00:00', {uid_b}, {c2}, 1, FALSE)"#),
        format!(r#"INSERT INTO "EpisodeSkipSegments" (EpisodeID, Kind, StartTime, EndTime, Source) VALUES ({c1}, 'silence', 1.0, 2.0, 'test')"#),
        format!(
            r#"INSERT INTO "GpodderSyncEpisodeActions" (UserID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, EpisodeID)
               VALUES ({uid_b}, '{url}', 'https://cdn.example.invalid/audio/ep1.mp3', 'play', 1700000000, 1, {c1})"#
        ),
    ] {
        exec(&app, &sql).await;
    }

    rerun_migrations(&app, &["082", "083", "118"]).await;
    app.state.merged_episodes.reset();

    assert_eq!(feed_episodes(&app, a).await, 2, "the copies are merged");
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "Episodes" WHERE EpisodeID IN ({c1}, {c2})"#)).await, 0);
    assert_eq!(count(&app, format!(r#"SELECT EpisodeID FROM "EpisodeIDAliases" WHERE OldEpisodeID = {c1}"#)).await, e1 as i64);
    let position = |user: i32, ep: i32| format!(r#"SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = {user} AND EpisodeID = {ep}"#);
    assert_eq!(count(&app, position(uid_a, e1)).await, 100, "alice's progress is hers");
    assert_eq!(count(&app, position(uid_b, e1)).await, 300, "bob keeps his furthest position");
    assert_eq!(
        count(&app, format!(r#"SELECT COUNT(*) FROM "UserEpisodeHistory" WHERE UserID = {uid_b} AND EpisodeID = {e1} AND ListenDate = '2026-03-02 10:00:00'"#)).await,
        1,
        "and his latest listen date"
    );
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "UserEpisodeState" WHERE UserID = {uid_b} AND EpisodeID = {e2} AND Completed = TRUE"#)).await, 1);
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "UserEpisodeState" WHERE UserID = {uid_a} AND EpisodeID = {e2}"#)).await, 0, "finished for bob only");
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "SavedEpisodes" WHERE UserID = {uid_b} AND EpisodeID = {e1}"#)).await, 1);
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "EpisodeQueue" WHERE UserID = {uid_b} AND EpisodeID = {e2}"#)).await, 1);
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "EpisodeSkipSegments" WHERE EpisodeID = {e1}"#)).await, 1);
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE UserID = {uid_b} AND EpisodeID = {e1}"#)).await, 1, "the sync log follows");

    let bobs = app.episodes(&bob, b).await;
    assert_eq!(bobs.len(), 2, "{bobs:#?}");
    let ep1 = bobs.iter().find(|e| e["Episodeid"] == e1).unwrap();
    assert_eq!((ep1["saved"].clone(), ep1["Listenduration"].clone()), (json!(true), json!(300)), "{ep1:#?}");

    // An app still holding bob's old id: the request lands on the merged episode.
    app.post("/api/data/save_episode").key(&bob).json(&json!({"episode_id": c2, "user_id": uid_b, "is_youtube": false})).send().await.assert_ok();
    assert_eq!(count(&app, format!(r#"SELECT COUNT(*) FROM "SavedEpisodes" WHERE UserID = {uid_b} AND EpisodeID = {e2}"#)).await, 1);
}
