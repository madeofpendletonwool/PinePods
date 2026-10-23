//! Ad times only fit the copy of an episode they were detected on, so the server records that
//! copy, tells each user whether their download is it, and links new downloads to it; finished
//! episodes' server downloads can be deleted automatically.

use pinepods_api::services::{ad_detection, audio_processing};
use serde_json::{json, Value};

use crate::harness::*;

fn scratch_dir() -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("pinepods-ad-downloads-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(dir: &std::path::Path, name: &str, bytes: &[u8]) -> String {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

async fn add_download(app: &TestApp, user: &TestUser, episode: i32, path: &str, downloaded_at: &str) {
    exec(app, &format!(
        r#"INSERT INTO "DownloadedEpisodes" (UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation)
           VALUES ({}, {episode}, '{downloaded_at}', 10, '{path}')"#,
        user.id,
    ))
    .await;
}

async fn has_download(app: &TestApp, user: &TestUser, episode: i32) -> bool {
    int(app, &format!(r#"SELECT COUNT(*) FROM "DownloadedEpisodes" WHERE UserID = {} AND EpisodeID = {episode}"#, user.id))
        .await
        == Some(1)
}

async fn finish(app: &TestApp, user: &TestUser, episode: i32, at: &str) {
    app.db().mark_episode_completed(episode, user.id, false, pinepods_api::services::sync::SyncOrigin::Unlogged).await.unwrap();
    exec(app, &format!(r#"UPDATE "UserEpisodeState" SET CompletedAt = '{at}' WHERE UserID = {} AND EpisodeID = {episode}"#, user.id)).await;
}

async fn delete_on_complete(app: &TestApp, user: &TestUser, podcast_id: Option<i32>) -> Value {
    app.post("/api/data/get_auto_delete_on_complete")
        .key(user)
        .json(&json!({ "user_id": user.id, "podcast_id": podcast_id }))
        .send()
        .await
        .assert_status(200)
        .json()
}

#[tokio::test(flavor = "multi_thread")]
async fn delete_when_finished_has_a_default_and_podcast_overrides() {
    let app = TestApp::new().await;
    let alice = app.user("alice").await;
    let pod = app.subscribe(&alice, &app.feed("show.xml", &Feed::with_episodes("Show", 1)), 1).await;

    assert_eq!(delete_on_complete(&app, &alice, None).await["enabled"], false);
    app.post("/api/data/user/set_auto_delete_on_complete")
        .key(&alice)
        .json(&json!({ "user_id": alice.id, "enabled": true }))
        .send()
        .await
        .assert_status(200);
    assert_eq!(delete_on_complete(&app, &alice, None).await["enabled"], true);
    assert_eq!(delete_on_complete(&app, &alice, Some(pod)).await, json!({ "enabled": true, "customized": false }));

    for (enabled, expected) in [
        (json!(false), json!({ "enabled": false, "customized": true })),
        (Value::Null, json!({ "enabled": true, "customized": false })),
    ] {
        app.post("/api/data/podcast/set_auto_delete_on_complete")
            .key(&alice)
            .json(&json!({ "user_id": alice.id, "podcast_id": pod, "enabled": enabled }))
            .send()
            .await
            .assert_status(200);
        assert_eq!(delete_on_complete(&app, &alice, Some(pod)).await, expected);
    }

    let bob = app.user("bob").await;
    app.post("/api/data/user/set_auto_delete_on_complete")
        .key(&bob)
        .json(&json!({ "user_id": alice.id, "enabled": false }))
        .send()
        .await
        .assert_status(403);
}

#[tokio::test(flavor = "multi_thread")]
async fn finished_episodes_lose_their_server_download() {
    let app = TestApp::new().await;
    let (alice, bob) = (app.user("alice").await, app.user("bob").await);
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 3));
    let a = app.subscribe(&alice, &url, 3).await;
    app.subscribe(&bob, &url, 3).await;
    let e1 = app.episode_id(&alice, a, "Episode 1").await;
    let e2 = app.episode_id(&alice, a, "Episode 2").await;
    let e3 = app.episode_id(&alice, a, "Episode 3").await;
    let dir = scratch_dir();
    let alice_e1 = write(&dir, "alice-e1.mp3", b"one");
    add_download(&app, &alice, e1, &alice_e1, "2026-01-01 10:00:00").await;
    add_download(&app, &alice, e2, &write(&dir, "alice-e2.mp3", b"two"), "2026-01-01 10:00:00").await;
    add_download(&app, &alice, e3, &write(&dir, "alice-e3.mp3", b"three"), "2026-02-01 10:00:00").await;
    add_download(&app, &bob, e1, &write(&dir, "bob-e1.mp3", b"one"), "2026-01-01 10:00:00").await;

    finish(&app, &alice, e1, "2026-01-02 10:00:00").await;
    // Finished before it was downloaded again: kept to be heard again.
    finish(&app, &alice, e3, "2026-01-02 10:00:00").await;
    finish(&app, &bob, e1, "2026-01-02 10:00:00").await;
    exec(&app, &format!(r#"UPDATE "Users" SET AutoDeleteOnComplete = TRUE WHERE UserID = {}"#, alice.id)).await;

    app.db().auto_delete_finished_downloads().await.unwrap();

    assert!(!has_download(&app, &alice, e1).await);
    assert!(!std::path::Path::new(&alice_e1).exists());
    assert!(has_download(&app, &alice, e2).await, "not finished");
    assert!(has_download(&app, &alice, e3).await, "downloaded after finishing");
    assert!(has_download(&app, &bob, e1).await, "bob didn't turn it on");

    // A podcast can opt out of the user's default.
    exec(&app, &format!(r#"UPDATE "Users" SET AutoDeleteOnComplete = FALSE WHERE UserID = {}"#, alice.id)).await;
    exec(&app, &format!(r#"UPDATE "Podcasts" SET AutoDeleteOnComplete = TRUE WHERE UserID = {}"#, bob.id)).await;
    app.db().auto_delete_finished_downloads().await.unwrap();
    assert!(!has_download(&app, &bob, e1).await);
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_user_learns_whether_their_download_is_the_copy_ads_were_found_on() {
    let app = TestApp::new().await;
    let (alice, bob, carol) = (app.user("alice").await, app.user("bob").await, app.user("carol").await);
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 1));
    let a = app.subscribe(&alice, &url, 1).await;
    app.subscribe(&bob, &url, 1).await;
    app.subscribe(&carol, &url, 1).await;
    let ep = app.episode_id(&alice, a, "Episode 1").await;
    let dir = scratch_dir();
    let analyzed = write(&dir, "alice.mp3", b"show with the ad from monday");
    add_download(&app, &alice, ep, &analyzed, "2026-01-01 10:00:00").await;
    add_download(&app, &bob, ep, &write(&dir, "bob.mp3", b"show with a different ad"), "2026-01-01 10:00:00").await;
    let fp = audio_processing::audio_fingerprint(&analyzed).await.unwrap();
    exec(&app, &format!(
        r#"INSERT INTO "EpisodeTranscripts" (EpisodeID, Source, Status, AudioFingerprint, AudioDuration, AudioSource)
           VALUES ({ep}, 'generated', 'complete', '{fp}', 1800.5, 'download')"#,
    ))
    .await;

    let skip_info = |user: &TestUser| {
        app.get("/api/data/episode_skip_segments").key(user).query("episode_id", ep).query("user_id", user.id).send()
    };
    // No ads yet: nothing to say about copies.
    assert_eq!(skip_info(&alice).await.assert_status(200).json()["ad_audio"], Value::Null);

    exec(&app, &format!(
        r#"INSERT INTO "EpisodeSkipSegments" (EpisodeID, Kind, StartTime, EndTime, Source) VALUES ({ep}, 'ad', 0, 60, 'auto-ad')"#,
    ))
    .await;
    let alice_view = skip_info(&alice).await.assert_status(200).json();
    assert_eq!(alice_view["ad_audio"]["download"], "match", "{alice_view:#?}");
    assert_eq!(alice_view["ad_audio"]["analyzed"]["duration"], 1800.5);
    assert_eq!(alice_view["ad_audio"]["analyzed"]["source"], "download");
    assert_eq!(skip_info(&bob).await.json()["ad_audio"]["download"], "mismatch");
    assert_eq!(skip_info(&carol).await.json()["ad_audio"]["download"], "none");

    // Bob's fingerprint was worked out on the way and kept.
    assert!(text(&app, &format!(r#"SELECT AudioFingerprint FROM "DownloadedEpisodes" WHERE UserID = {}"#, bob.id)).await.is_some());

    // Carol's new download links the analyzed copy rather than bob's.
    let carol_path = dir.join("carol.mp3");
    let (size, carol_fp) = audio_processing::reuse_download(app.db(), ep, carol.id, &carol_path).await.unwrap();
    assert_eq!(size, std::fs::metadata(&analyzed).unwrap().len());
    assert_eq!(carol_fp.as_deref(), Some(fp.as_str()));
    assert_eq!(std::fs::read(&carol_path).unwrap(), std::fs::read(&analyzed).unwrap());

    // Clearing the ads drops them and the scanned flag so detection runs again.
    exec(&app, &format!(r#"UPDATE "Episodes" SET AdsDetected = TRUE WHERE EpisodeID = {ep}"#)).await;
    assert!(ad_detection::clear_ad_segments(app.db(), ep).await.unwrap());
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "EpisodeSkipSegments" WHERE EpisodeID = {ep}"#)).await, Some(0));
    assert!(!ad_detection::has_ad_segments(app.db(), ep).await);
    let _ = std::fs::remove_dir_all(dir);
}
