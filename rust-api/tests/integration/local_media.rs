//! Podcasts made from folders of audio on the server.

use std::path::Path;

use id3::TagLike;

use crate::harness::files::{local_media_folder, mp3_bytes};
use crate::harness::*;

fn add_track(dir: &Path, file: &str, title: &str) {
    let path = dir.join(file);
    std::fs::write(&path, mp3_bytes()).unwrap();
    let mut tag = id3::Tag::new();
    tag.set_title(title);
    tag.write_to_path(&path, id3::Version::Id3v24).unwrap();
}

async fn add_podcast(app: &TestApp, user: &TestUser, dir: &str) -> i32 {
    let body = app.post("/api/data/add_local_podcast")
        .key(user)
        .json(&json!({ "user_id": user.id, "directory_path": dir, "podcast_name": format!("Local {dir}") }))
        .send()
        .await
        .ok_json();
    body["data"]["podcastid"].as_i64().unwrap_or_else(|| panic!("no podcast id in {body}")) as i32
}

#[tokio::test(flavor = "multi_thread")]
async fn folders_can_be_browsed_with_their_audio_counts() {
    let app = TestApp::new().await;
    let user = app.user("curator").await;
    let name = format!("browse-{}", app.tag);
    let dir = local_media_folder(&name);
    add_track(&dir, "a.mp3", "A");
    add_track(&dir, "b.mp3", "B");
    std::fs::write(dir.join("notes.txt"), "not audio").unwrap();

    let root = app.get("/api/data/list_local_directories").key(&user).send().await.ok_json();
    let entry = root["directories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["name"] == name.as_str())
        .unwrap_or_else(|| panic!("{name} not listed: {root}"))
        .clone();
    assert_eq!(entry["audio_count"], 2);
    assert_eq!(entry["path"], name.as_str());
    assert!(root["directories"].as_array().unwrap().iter().all(|d| d["name"] != "_artwork"));
}

#[tokio::test(flavor = "multi_thread")]
async fn paths_outside_the_media_folder_are_refused() {
    let app = TestApp::new().await;
    let user = app.user("curator").await;
    local_media_folder(&format!("safe-{}", app.tag));

    for path in ["..", "../..", "/etc"] {
        let listed = app.get("/api/data/list_local_directories").key(&user).query("path", path).send().await;
        listed.assert_status(400);
        let added = app.post("/api/data/add_local_podcast")
            .key(&user)
            .json(&json!({ "user_id": user.id, "directory_path": path, "podcast_name": "Escape" }))
            .send()
            .await;
        added.assert_status(400);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_becomes_a_podcast_with_playable_episodes() {
    let app = TestApp::new().await;
    let user = app.user("curator").await;
    let name = format!("show-{}", app.tag);
    let dir = local_media_folder(&name);
    add_track(&dir, "one.mp3", "Track One");
    add_track(&dir, "two.mp3", "Track Two");

    let pod = add_podcast(&app, &user, &name).await;
    assert_eq!(app.episodes(&user, pod).await.len(), 2);
    let episode = app.episode_id(&user, pod, "Track One").await;

    let played = app.get(&format!("/api/data/stream/{episode}"))
        .query("api_key", &user.api_key)
        .query("user_id", user.id)
        .send()
        .await;
    played.assert_ok();
    assert_eq!(played.body, std::fs::read(dir.join("one.mp3")).unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn only_the_owner_can_play_a_local_episode() {
    let app = TestApp::new().await;
    let owner = app.user("curator").await;
    let other = app.user("other").await;
    let name = format!("private-{}", app.tag);
    add_track(&local_media_folder(&name), "one.mp3", "Track One");
    let pod = add_podcast(&app, &owner, &name).await;
    let episode = app.episode_id(&owner, pod, "Track One").await;

    let resp = app.get(&format!("/api/data/stream/{episode}"))
        .query("api_key", &other.api_key)
        .query("user_id", other.id)
        .send()
        .await;
    assert!(!resp.is_success(), "got {}", resp.status);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_folder_cannot_be_added_twice_or_without_audio() {
    let app = TestApp::new().await;
    let user = app.user("curator").await;
    let name = format!("once-{}", app.tag);
    add_track(&local_media_folder(&name), "one.mp3", "Track One");
    add_podcast(&app, &user, &name).await;

    let again = app.post("/api/data/add_local_podcast")
        .key(&user)
        .json(&json!({ "user_id": user.id, "directory_path": name, "podcast_name": "Again" }))
        .send()
        .await;
    again.assert_status(400);

    let empty = format!("empty-{}", app.tag);
    local_media_folder(&empty);
    let resp = app.post("/api/data/add_local_podcast")
        .key(&user)
        .json(&json!({ "user_id": user.id, "directory_path": empty, "podcast_name": "Empty" }))
        .send()
        .await;
    resp.assert_status(400);
}

#[tokio::test(flavor = "multi_thread")]
async fn refreshing_picks_up_new_files() {
    let app = TestApp::new().await;
    let user = app.user("curator").await;
    let name = format!("growing-{}", app.tag);
    let dir = local_media_folder(&name);
    add_track(&dir, "one.mp3", "Track One");
    let pod = add_podcast(&app, &user, &name).await;

    add_track(&dir, "two.mp3", "Track Two");
    app.post("/api/data/refresh_local_podcast")
        .key(&user)
        .json(&json!({ "user_id": user.id, "podcast_id": pod }))
        .send()
        .await
        .assert_ok();

    assert_eq!(app.episodes(&user, pod).await.len(), 2);
    app.episode_id(&user, pod, "Track Two").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_cover_in_the_folder_becomes_the_artwork() {
    let app = TestApp::new().await;
    let user = app.user("curator").await;
    let name = format!("covered-{}", app.tag);
    let dir = local_media_folder(&name);
    add_track(&dir, "one.mp3", "Track One");
    std::fs::write(dir.join("cover.jpg"), [0xFF, 0xD8, 0xFF, 0xD9]).unwrap();

    let pod = add_podcast(&app, &user, &name).await;
    let podcasts = app.podcasts(&user).await;
    let artwork = podcasts
        .iter()
        .find(|p| p["podcastid"] == pod)
        .and_then(|p| p["artworkurl"].as_str())
        .unwrap_or_else(|| panic!("no artwork in {podcasts:#?}"))
        .to_string();
    assert!(artwork.starts_with("/api/local-media/_artwork/"), "{artwork}");

    let served = app.get(&artwork).send().await;
    served.assert_ok();
    assert_eq!(served.body, [0xFF, 0xD8, 0xFF, 0xD9]);
}
