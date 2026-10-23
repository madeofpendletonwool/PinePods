//! YouTube channel refresh: new videos are found through the channel's RSS feed and yt-dlp only
//! fetches those, one run for all of them; a channel is fetched once for all its subscribers and
//! never further back than the longest cutoff among them; yt-dlp's listing stands in when the
//! feed fails; cleanup keeps the videos users hold on to.

use pinepods_api::handlers::refresh::{run_refresh_process, RefreshMessage};

use crate::harness::*;

async fn failures(app: &TestApp, pod: i32) -> Option<i64> {
    int(app, &format!(r#"SELECT COALESCE(ConsecutiveFailures, 0) FROM "Podcasts" WHERE PodcastID = {pod}"#)).await
}

#[tokio::test(flavor = "multi_thread")]
async fn only_new_videos_are_fetched_with_ytdlp_in_one_run() {
    let app = TestApp::new().await;
    let user = app.user("watcher").await;
    let channel = app.youtube_channel("one").video(1, 3).video(2, 2).video(3, 1);
    app.publish_channel(&channel);

    let pod = app.subscribe_channel(&user, &channel, 30).await;
    assert_eq!(app.video_ids(pod).await, vec![channel.video_id(1), channel.video_id(2), channel.video_id(3)]);
    let first = app.ytdlp_detail_requests(&channel);
    assert_eq!(first.len(), 1, "the import fetches every video in one yt-dlp run: {first:?}");
    assert_eq!(first[0].len(), 3);

    let channel = channel.video(4, 0);
    app.publish_channel(&channel);
    app.refresh().await;
    assert_eq!(app.video_ids(pod).await.len(), 4);
    let requests = app.ytdlp_detail_requests(&channel);
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[1], vec![channel.video_id(4)], "yt-dlp only runs for the new video");
    let title = text(&app, &format!(r#"SELECT VideoTitle FROM "YouTubeVideos" WHERE PodcastID = {pod} AND YouTubeVideoID = '{}'"#, channel.video_id(4))).await;
    assert_eq!(title.as_deref(), Some("Video 4"));
    let duration = int(&app, &format!(r#"SELECT Duration FROM "YouTubeVideos" WHERE PodcastID = {pod} AND YouTubeVideoID = '{}'"#, channel.video_id(4))).await;
    assert_eq!(duration, Some(604), "duration comes from yt-dlp");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unchanged_channel_feed_runs_no_ytdlp() {
    let app = TestApp::new().await;
    let user = app.user("watcher").await;
    let mut channel = app.youtube_channel("same").video(1, 2).video(2, 1);
    app.publish_channel(&channel);
    let pod = app.subscribe_channel(&user, &channel, 30).await;
    let calls = app.ytdlp_calls(&channel).len();

    // View counts change on every fetch; that alone isn't a change.
    for views in [50, 900] {
        channel.views = views;
        app.publish_channel(&channel);
        app.refresh().await;
    }
    assert_eq!(app.channel_feed_hits(&channel), 3, "the feed is still fetched every cycle");
    assert_eq!(app.ytdlp_calls(&channel).len(), calls, "no yt-dlp run for an unchanged feed");
    assert_eq!(app.video_ids(pod).await.len(), 2);
    assert_eq!(failures(&app, pod).await, Some(0));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_channel_is_fetched_once_for_all_subscribers_up_to_the_longest_cutoff() {
    let app = TestApp::new().await;
    let long = app.user("long").await;
    let short = app.user("short").await;
    let channel = app.youtube_channel("shared").video(1, 90).video(2, 45).video(3, 5);
    app.publish_channel(&channel);

    let long_pod = app.subscribe_channel(&long, &channel, 60).await;
    let short_pod = app.subscribe_channel(&short, &channel, 30).await;
    assert_eq!(app.video_ids(long_pod).await, vec![channel.video_id(2), channel.video_id(3)]);
    assert_eq!(app.video_ids(short_pod).await, vec![channel.video_id(3)]);

    let channel = channel.video(4, 0);
    app.publish_channel(&channel);
    let hits = app.channel_feed_hits(&channel);
    app.refresh().await;
    assert_eq!(app.channel_feed_hits(&channel), hits + 1, "one fetch for both subscribers");
    assert_eq!(app.ytdlp_detail_requests(&channel).last().unwrap(), &vec![channel.video_id(4)], "one yt-dlp run for both");
    assert!(app.video_ids(long_pod).await.contains(&channel.video_id(4)));
    assert!(app.video_ids(short_pod).await.contains(&channel.video_id(4)));

    let asked: Vec<String> = app.ytdlp_detail_requests(&channel).concat();
    assert!(!asked.contains(&channel.video_id(1)), "nothing past the longest cutoff is fetched: {asked:?}");
    assert!(app.ytdlp_calls(&channel).iter().all(|c| !c.contains(&channel.video_id(1))));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_failing_feed_falls_back_to_the_ytdlp_listing() {
    let app = TestApp::new().await;
    let user = app.user("watcher").await;
    let channel = app.youtube_channel("fallback").video(1, 2);
    app.publish_channel(&channel);
    let pod = app.subscribe_channel(&user, &channel, 30).await;

    let channel = channel.video(2, 0);
    app.publish_channel(&channel);
    app.fixtures().serve(&format!("youtube/{}.xml", channel.id), Fixture::new("text/html", "oops").status(500));
    app.refresh().await;

    assert!(app.ytdlp_calls(&channel).iter().any(|c| c.contains("--flat-playlist") && c.contains("--playlist-end 15")));
    assert_eq!(app.ytdlp_detail_requests(&channel).last().unwrap(), &vec![channel.video_id(2)]);
    assert_eq!(app.video_ids(pod).await.len(), 2);
    assert_eq!(failures(&app, pod).await, Some(0), "the channel was refreshed");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_channel_failing_both_ways_is_a_feed_problem() {
    let app = TestApp::new().await;
    let user = app.user("watcher").await;
    let channel = app.youtube_channel("gone").video(1, 2);
    app.publish_channel(&channel);
    let pod = app.subscribe_channel(&user, &channel, 30).await;

    app.fixtures().serve(&format!("youtube/{}.xml", channel.id), Fixture::new("text/html", "gone").status(404));
    let _ = std::fs::remove_file(app.env.ytdlp_dir.join("channels").join(format!("{}.jsonl", channel.id)));
    app.refresh().await;

    assert_eq!(failures(&app, pod).await, Some(1));
    let kind = text(&app, &format!(r#"SELECT LastErrorKind FROM "Podcasts" WHERE PodcastID = {pod}"#)).await;
    assert_eq!(kind.as_deref(), Some("gone"));
    assert_eq!(app.video_ids(pod).await.len(), 1, "stored videos stay");
}

#[tokio::test(flavor = "multi_thread")]
async fn videos_ytdlp_cannot_read_are_skipped_without_failing_the_channel() {
    let app = TestApp::new().await;
    let user = app.user("watcher").await;
    let channel = app.youtube_channel("members").video(1, 2);
    app.publish_channel(&channel);
    let pod = app.subscribe_channel(&user, &channel, 30).await;

    let channel = channel.video(2, 1).video(3, 0);
    app.publish_channel(&channel);
    app.hide_video(&channel.video_id(2));
    app.refresh().await;

    assert_eq!(app.video_ids(pod).await, vec![channel.video_id(1), channel.video_id(3)]);
    assert_eq!(failures(&app, pod).await, Some(0));
}

#[tokio::test(flavor = "multi_thread")]
async fn the_manual_refresh_streams_new_videos() {
    let app = TestApp::new().await;
    let user = app.user("watcher").await;
    let channel = app.youtube_channel("stream").video(1, 2);
    app.publish_channel(&channel);
    let pod = app.subscribe_channel(&user, &channel, 30).await;
    // The manual refresh skips a channel refreshed moments ago.
    exec(&app, &format!(r#"UPDATE "Podcasts" SET LastRefreshSuccess = '2000-01-01 00:00:00' WHERE PodcastID = {pod}"#)).await;
    exec(&app, r#"UPDATE "Feeds" SET LastRefreshSuccess = '2000-01-01 00:00:00'"#).await;

    let channel = channel.video(2, 0);
    app.publish_channel(&channel);
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    run_refresh_process(user.id, false, tx, app.state.clone()).await.unwrap();

    let mut streamed = Vec::new();
    while let Ok(msg) = rx.try_recv() {
        if let RefreshMessage::NewEpisode(new) = msg {
            streamed.push(new.new_episode);
        }
    }
    assert_eq!(streamed.len(), 1, "the new video is sent over the websocket");
    let video = &streamed[0];
    assert_eq!(video.podcastid, pod);
    assert_eq!(video.episodetitle, "Video 2");
    assert!(video.is_youtube);
    assert_eq!(video.episodeurl, format!("https://www.youtube.com/watch?v={}", channel.video_id(2)));
}

#[tokio::test(flavor = "multi_thread")]
async fn cleanup_keeps_old_videos_users_hold_on_to() {
    let app = TestApp::new().await;
    let user = app.user("keeper").await;
    let channel = app.youtube_channel("old").video(1, 6).video(2, 5).video(3, 4).video(4, 3).video(5, 2).video(6, 1);
    app.publish_channel(&channel);
    let pod = app.subscribe_channel(&user, &channel, 30).await;

    let id_of = |n: u32| {
        let yt = channel.video_id(n);
        let app = &app;
        async move {
            int(app, &format!(r#"SELECT VideoID FROM "YouTubeVideos" WHERE PodcastID = {pod} AND YouTubeVideoID = '{yt}'"#))
                .await
                .unwrap()
        }
    };
    let u = user.id;
    exec(&app, &format!(r#"INSERT INTO "UserVideoHistory" (UserID, VideoID, ListenDate, ListenDuration) VALUES ({u}, {}, '2026-01-01 00:00:00', 30)"#, id_of(1).await)).await;
    exec(&app, &format!(r#"INSERT INTO "SavedVideos" (UserID, VideoID) VALUES ({u}, {})"#, id_of(2).await)).await;
    exec(&app, &format!(r#"INSERT INTO "EpisodeQueue" (UserID, EpisodeID, QueuePosition, is_youtube) VALUES ({u}, {}, 1, TRUE)"#, id_of(3).await)).await;
    let playlist = match int(&app, &format!(r#"SELECT MIN(PlaylistID) FROM "Playlists" WHERE UserID = {u}"#)).await {
        Some(id) => id,
        None => {
            exec(&app, &format!(r#"INSERT INTO "Playlists" (UserID, Name) VALUES ({u}, 'Mine')"#)).await;
            int(&app, &format!(r#"SELECT MIN(PlaylistID) FROM "Playlists" WHERE UserID = {u}"#)).await.unwrap()
        }
    };
    exec(&app, &format!(r#"INSERT INTO "PlaylistContents" (PlaylistID, VideoID, Position) VALUES ({playlist}, {}, 1)"#, id_of(4).await)).await;
    exec(&app, &format!(r#"INSERT INTO "DownloadedVideos" (UserID, VideoID, DownloadedSize, DownloadedLocation) VALUES ({u}, {}, 1, '/tmp/x.mp3')"#, id_of(5).await)).await;
    // A podcast episode queued under the same number as an old video isn't the video.
    exec(&app, &format!(r#"INSERT INTO "EpisodeQueue" (UserID, EpisodeID, QueuePosition, is_youtube) VALUES ({u}, {}, 2, FALSE)"#, id_of(6).await)).await;

    exec(&app, &format!(r#"UPDATE "YouTubeVideos" SET PublishedAt = '2020-01-01 00:00:00' WHERE PodcastID = {pod}"#)).await;
    app.db().remove_old_youtube_videos(pod, chrono::Utc::now() - chrono::Duration::days(30)).await.unwrap();

    let kept = app.video_ids(pod).await;
    assert_eq!(
        kept,
        vec![channel.video_id(1), channel.video_id(2), channel.video_id(3), channel.video_id(4), channel.video_id(5)],
        "history, saved, queued, playlist and downloaded videos stay"
    );
    let queued = int(&app, &format!(r#"SELECT COUNT(*) FROM "EpisodeQueue" WHERE UserID = {u}"#)).await;
    assert_eq!(queued, Some(2), "queue rows are left alone");
}
