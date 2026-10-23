//! Whether a user finished an episode is kept per user: marking, unmarking and bulk marking only
//! ever touch the caller's own episodes, and the upgrade carries existing completions over.

use pinepods_api::services::sync::SyncOrigin;

use crate::harness::*;

async fn completed(app: &TestApp, user: &TestUser, episode: i32) -> Option<i64> {
    int(app, &format!(r#"SELECT Completed FROM "UserEpisodeState" WHERE UserID = {} AND EpisodeID = {episode}"#, user.id)).await
}

async fn episode_flag(app: &TestApp, episode: i32) -> Option<i64> {
    int(app, &format!(r#"SELECT Completed FROM "Episodes" WHERE EpisodeID = {episode}"#)).await
}

#[tokio::test(flavor = "multi_thread")]
async fn completion_is_recorded_per_user() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 2));
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;
    let a1 = app.episode_id(&alice, a, "Episode 1").await;
    let a2 = app.episode_id(&alice, a, "Episode 2").await;
    let b1 = app.episode_id(&bob, b, "Episode 1").await;

    app.db().mark_episode_completed(a1, alice.id, false, SyncOrigin::Unlogged).await.unwrap();
    assert_eq!(completed(&app, &alice, a1).await, Some(1));
    assert_eq!(episode_flag(&app, a1).await, Some(1));

    app.db().mark_episode_uncompleted(a1, alice.id, false, SyncOrigin::Unlogged).await.unwrap();
    assert_eq!(completed(&app, &alice, a1).await, Some(0));
    assert_eq!(episode_flag(&app, a1).await, Some(0));

    // Bob's episode in Alice's bulk request is left alone.
    let (done, _) = app.db().bulk_mark_episodes_completed(vec![a1, a2, b1], alice.id, false).await.unwrap();
    assert_eq!(done, 2);
    assert_eq!(completed(&app, &alice, a2).await, Some(1));
    assert_eq!(completed(&app, &alice, b1).await, None);
    assert_eq!(completed(&app, &bob, b1).await, None);
    assert_eq!(episode_flag(&app, b1).await, Some(0));
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrading_carries_completed_episodes_over() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 2));
    let a = app.subscribe(&alice, &url, 2).await;
    let b = app.subscribe(&bob, &url, 2).await;
    let a1 = app.episode_id(&alice, a, "Episode 1").await;
    let b2 = app.episode_id(&bob, b, "Episode 2").await;

    // As before the upgrade: only the episode rows know.
    exec(&app, r#"DROP TABLE "UserEpisodeState""#).await;
    exec(&app, &format!(r#"UPDATE "Episodes" SET Completed = TRUE WHERE EpisodeID IN ({a1}, {b2})"#)).await;
    rerun_migrations(&app, &["081"]).await;

    assert_eq!(completed(&app, &alice, a1).await, Some(1));
    assert_eq!(completed(&app, &bob, b2).await, Some(1));
    assert_eq!(int(&app, r#"SELECT COUNT(*) FROM "UserEpisodeState""#).await, Some(2));
}
