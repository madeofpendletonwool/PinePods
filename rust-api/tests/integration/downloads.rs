//! Server downloads: fetching an episode to disk, playing it back, sharing one copy between
//! users, deleting it, and the files written next to it.

use crate::harness::files::{assert_gone, mp3_bytes};
use crate::harness::*;

struct Show {
    app: TestApp,
    user: TestUser,
    pod: i32,
    episode: i32,
    audio_path: &'static str,
}

/// A user following a show whose one episode has real audio behind it.
async fn show() -> Show {
    let app = TestApp::new().await;
    let user = app.user("saver").await;
    let audio_path = "audio/ep1.mp3";
    let audio = app.fixtures().serve(audio_path, Fixture::new("audio/mpeg", mp3_bytes()));
    let feed = Feed::new(&format!("Download Show {}", app.tag)).episode(Episode::new(1).enclosure(&audio));
    let pod = app.subscribe(&user, &app.feed("show.xml", &feed), 1).await;
    let episode = app.episode_id(&user, pod, "Episode 1").await;
    Show { app, user, pod, episode, audio_path }
}

async fn start_download(app: &TestApp, user: &TestUser, episode: i32) {
    app.post("/api/data/download_podcast")
        .key(user)
        .json(&json!({ "episode_id": episode, "user_id": user.id }))
        .send()
        .await
        .assert_ok();
}

async fn location(app: &TestApp, user: &TestUser, episode: i32) -> Option<String> {
    text(app, &format!(
        r#"SELECT DownloadedLocation FROM "DownloadedEpisodes" WHERE UserID = {} AND EpisodeID = {episode}"#,
        user.id
    ))
    .await
}

async fn downloaded(app: &TestApp, user: &TestUser, episode: i32) -> std::path::PathBuf {
    start_download(app, user, episode).await;
    wait_for_download(app, user, episode).await
}

async fn wait_for_download(app: &TestApp, user: &TestUser, episode: i32) -> std::path::PathBuf {
    let path = eventually("episode downloaded", || async { location(app, user, episode).await }).await;
    std::path::PathBuf::from(path)
}

fn stream_uri(_user: &TestUser, episode: i32) -> String {
    format!("/api/data/stream/{episode}")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_download_is_saved_listed_and_playable() {
    let s = show().await;
    let path = downloaded(&s.app, &s.user, s.episode).await;

    let on_disk = std::fs::read(&path).expect("downloaded file");
    let frames = mp3_bytes();
    assert!(on_disk.windows(frames.len()).any(|w| w == frames), "the file holds the episode's audio");
    assert!(path.starts_with(crate::harness::files::data_dir().join("downloads")), "{}", path.display());

    let list = s.app
        .get("/api/data/download_episode_list")
        .key(&s.user)
        .query("user_id", s.user.id)
        .send()
        .await
        .ok_json();
    let listed = list["downloaded_episodes"].as_array().unwrap();
    assert_eq!(listed.len(), 1, "{list}");
    assert_eq!(listed[0]["episodeid"], s.episode, "{list}");

    let played = s.app
        .get(&stream_uri(&s.user, s.episode))
        .query("api_key", &s.user.api_key)
        .query("user_id", s.user.id)
        .send()
        .await;
    played.assert_ok();
    assert_eq!(played.body, on_disk);
}

#[tokio::test(flavor = "multi_thread")]
async fn players_can_seek_in_a_download() {
    let s = show().await;
    let path = downloaded(&s.app, &s.user, s.episode).await;
    let on_disk = std::fs::read(&path).unwrap();

    let part = s.app
        .get(&stream_uri(&s.user, s.episode))
        .query("api_key", &s.user.api_key)
        .query("user_id", s.user.id)
        .header("Range", "bytes=10-19")
        .send()
        .await;
    part.assert_status(206);
    assert_eq!(part.body, on_disk[10..20]);
}

#[tokio::test(flavor = "multi_thread")]
async fn only_the_owner_can_play_a_download() {
    let s = show().await;
    downloaded(&s.app, &s.user, s.episode).await;
    let other = s.app.user("other").await;

    let resp = s.app
        .get(&stream_uri(&s.user, s.episode))
        .query("api_key", &other.api_key)
        .query("user_id", s.user.id)
        .send()
        .await;
    resp.assert_status(403);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_episode_that_is_not_downloaded_has_nothing_to_stream() {
    let s = show().await;
    let resp = s.app
        .get(&stream_uri(&s.user, s.episode))
        .query("api_key", &s.user.api_key)
        .query("user_id", s.user.id)
        .send()
        .await;
    resp.assert_status(404);
}

#[tokio::test(flavor = "multi_thread")]
async fn asking_again_does_not_fetch_the_episode_again() {
    let s = show().await;
    downloaded(&s.app, &s.user, s.episode).await;
    let fetched = s.app.fixtures().hits(s.audio_path).len();

    let again = s.app
        .post("/api/data/download_podcast")
        .key(&s.user)
        .json(&json!({ "episode_id": s.episode, "user_id": s.user.id }))
        .send()
        .await
        .ok_json();
    assert_eq!(again["detail"], "Content already downloaded.");
    assert_eq!(s.app.fixtures().hits(s.audio_path).len(), fetched);
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_a_download_removes_the_file() {
    let s = show().await;
    let path = downloaded(&s.app, &s.user, s.episode).await;

    s.app.post("/api/data/delete_episode")
        .key(&s.user)
        .json(&json!({ "episode_id": s.episode, "user_id": s.user.id }))
        .send()
        .await
        .assert_ok();

    assert_gone(&path);
    assert_eq!(location(&s.app, &s.user, s.episode).await, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn bulk_deleting_downloads_removes_the_files() {
    let s = show().await;
    let path = downloaded(&s.app, &s.user, s.episode).await;

    let body = s.app
        .post("/api/data/bulk_delete_downloaded_episodes")
        .key(&s.user)
        .json(&json!({ "episode_ids": [s.episode], "user_id": s.user.id }))
        .send()
        .await
        .ok_json();
    assert_eq!(body["processed_count"], 1, "{body}");
    assert_gone(&path);
}

#[tokio::test(flavor = "multi_thread")]
async fn unsubscribing_removes_downloaded_files() {
    let s = show().await;
    let path = downloaded(&s.app, &s.user, s.episode).await;

    s.app.post("/api/data/remove_podcast_id")
        .key(&s.user)
        .json(&json!({ "podcast_id": s.pod, "user_id": s.user.id }))
        .send()
        .await
        .assert_ok();

    eventually("download removed", || async { (!path.exists()).then_some(()) }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn two_users_share_one_fetch_and_keep_their_own_copy_when_the_other_deletes() {
    let s = show().await;
    let first = downloaded(&s.app, &s.user, s.episode).await;

    let second_user = s.app.user("partner").await;
    let feed_url = s.app.fixtures().url("show.xml");
    s.app.subscribe(&second_user, &feed_url, 1).await;
    let second = downloaded(&s.app, &second_user, s.episode).await;

    assert_eq!(s.app.fixtures().hits(s.audio_path).len(), 1, "the second user reuses the first copy");
    assert_ne!(first, second, "each user has their own file");
    assert_eq!(std::fs::read(&first).unwrap(), std::fs::read(&second).unwrap());

    s.app.post("/api/data/delete_episode")
        .key(&s.user)
        .json(&json!({ "episode_id": s.episode, "user_id": s.user.id }))
        .send()
        .await
        .assert_ok();
    assert_gone(&first);
    let played = s.app
        .get(&stream_uri(&second_user, s.episode))
        .query("api_key", &second_user.api_key)
        .query("user_id", second_user.id)
        .send()
        .await;
    played.assert_ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_web_page_is_not_saved_as_an_episode() {
    let app = TestApp::new().await;
    let user = app.user("saver").await;
    let page = app.fixtures().serve("gone.mp3", Fixture::new("text/html", "<html>moved</html>"));
    let feed = Feed::new(&format!("Page Show {}", app.tag)).episode(Episode::new(1).enclosure(&page));
    let pod = app.subscribe(&user, &app.feed("show.xml", &feed), 1).await;
    let episode = app.episode_id(&user, pod, "Episode 1").await;

    start_download(&app, &user, episode).await;
    eventually("the page was fetched", || async { (!app.fixtures().hits("gone.mp3").is_empty()).then_some(()) }).await;
    eventually("the download task finished", || async {
        let tasks = app.get(&format!("/api/tasks/user/{}", user.id)).key(&user).send().await.ok_json();
        let finished = tasks.as_array().is_some_and(|all| {
            all.iter().any(|t| t["status"].as_str().is_some_and(|s| s.eq_ignore_ascii_case("failed")))
        });
        finished.then_some(())
    })
    .await;
    assert_eq!(location(&app, &user, episode).await, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_admin_can_turn_server_downloads_off() {
    let s = show().await;
    let admin = s.app.admin().await.clone();
    s.app.post("/api/data/enable_disable_downloads").key(&admin).send().await.assert_ok();

    let resp = s.app
        .post("/api/data/download_podcast")
        .key(&s.user)
        .json(&json!({ "episode_id": s.episode, "user_id": s.user.id }))
        .send()
        .await;
    resp.assert_status(403);
    assert!(s.app.fixtures().hits(s.audio_path).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn download_all_fetches_every_episode() {
    let app = TestApp::new().await;
    let user = app.user("saver").await;
    let mut feed = Feed::new(&format!("Bulk Show {}", app.tag));
    for n in 1..=3 {
        let audio = app.fixtures().serve(&format!("audio/{n}.mp3"), Fixture::new("audio/mpeg", mp3_bytes()));
        feed = feed.episode(Episode::new(n).enclosure(&audio));
    }
    let pod = app.subscribe(&user, &app.feed("show.xml", &feed), 3).await;

    app.post("/api/data/download_all_podcast")
        .key(&user)
        .json(&json!({ "podcast_id": pod, "user_id": user.id }))
        .send()
        .await
        .assert_ok();

    let count = format!(r#"SELECT COUNT(*) FROM "DownloadedEpisodes" WHERE UserID = {}"#, user.id);
    eventually("all three downloaded", || async { (int(&app, &count).await == Some(3)).then_some(()) }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn auto_download_fetches_new_episodes_on_refresh() {
    let s = show().await;
    s.app.post("/api/data/enable_auto_download")
        .key(&s.user)
        .json(&json!({ "podcast_id": s.pod, "auto_download": true, "user_id": s.user.id }))
        .send()
        .await
        .assert_ok();

    let audio = s.app.fixtures().serve("audio/ep2.mp3", Fixture::new("audio/mpeg", mp3_bytes()));
    let first = s.app.fixtures().url(s.audio_path);
    let feed = Feed::new(&format!("Download Show {}", s.app.tag))
        .episode(Episode::new(1).enclosure(&first))
        .episode(Episode::new(2).enclosure(&audio));
    s.app.feed("show.xml", &feed);
    s.app.refresh().await;

    let episode = s.app.episode_id(&s.user, s.pod, "Episode 2").await;
    wait_for_download(&s.app, &s.user, episode).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn downloads_are_tagged_for_other_players() {
    let s = show().await;
    let path = downloaded(&s.app, &s.user, s.episode).await;

    let tag = eventually("tags written", || {
        let path = path.clone();
        async move { id3::Tag::read_from_path(&path).ok() }
    })
    .await;
    use id3::TagLike;
    assert_eq!(tag.title(), Some("Episode 1"));
    assert_eq!(tag.album().map(str::to_string), Some(format!("Download Show {}", s.app.tag)));
}

#[tokio::test(flavor = "multi_thread")]
async fn cover_and_metadata_files_are_written_when_turned_on() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let user = app.user("saver").await;
    app.post("/api/data/download_metadata_settings")
        .key(&admin)
        .json(&json!({
            "folder_cover": true,
            "episode_cover": false,
            "metadata_sidecar": true,
            "metadata_format": "json",
            "metadata_subfolder": false,
        }))
        .send()
        .await
        .assert_ok();

    let audio = app.fixtures().serve("audio/ep1.mp3", Fixture::new("audio/mpeg", mp3_bytes()));
    let art = app.fixtures().serve("cover.jpg", Fixture::new("image/jpeg", vec![0xFF, 0xD8, 0xFF, 0xD9]));
    let mut feed = Feed::new(&format!("Sidecar Show {}", app.tag)).episode(Episode::new(1).enclosure(&audio));
    feed.image = art;
    let pod = app.subscribe(&user, &app.feed("show.xml", &feed), 1).await;
    let episode = app.episode_id(&user, pod, "Episode 1").await;

    let path = downloaded(&app, &user, episode).await;
    let folder = path.parent().unwrap().to_path_buf();
    let sidecar = path.with_extension("json");

    eventually("folder.jpg written", || {
        let cover = folder.join("folder.jpg");
        async move { cover.exists().then_some(()) }
    })
    .await;
    let metadata = eventually("metadata written", || {
        let sidecar = sidecar.clone();
        async move { std::fs::read_to_string(&sidecar).ok() }
    })
    .await;
    let metadata: Value = serde_json::from_str(&metadata).expect("json sidecar");
    assert_eq!(metadata["title"], "Episode 1", "{metadata}");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_episode_file_endpoint_sends_the_episode() {
    let s = show().await;
    let resp = s.app.get(&format!("/api/episodes/{}/download", s.episode)).key(&s.user).send().await;
    resp.assert_ok();
    let frames = mp3_bytes();
    assert!(resp.body.windows(frames.len()).any(|w| w == frames), "response holds the audio");
    let disposition = resp.headers.get("content-disposition").and_then(|v| v.to_str().ok()).unwrap_or_default();
    assert!(disposition.contains("attachment"), "{disposition}");
}
