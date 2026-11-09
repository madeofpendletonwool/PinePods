//! A generated transcript downloads as SRT, WebVTT or plain text.

use crate::harness::*;
use crate::harness::http::TestResponse;

async fn export(app: &TestApp, user: &TestUser, ep: i32, format: &str) -> TestResponse {
    app.get("/api/data/episode_transcript_export")
        .key(user)
        .query("episode_id", ep)
        .query("user_id", user.id)
        .query("format", format)
        .send()
        .await
}

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_transcript_downloads_in_each_format() {
    let app = TestApp::new().await;
    let user = app.user("reader").await;
    let url = app.feed("talk.xml", &Feed::with_episodes("Talk Show", 2));
    let pod = app.subscribe(&user, &url, 2).await;
    let ep = app.episode_id(&user, pod, "Episode 1").await;
    let unfinished = app.episode_id(&user, pod, "Episode 2").await;

    export(&app, &user, ep, "srt").await.assert_status(404);

    // The MariaDB helper drops double quotes, so build the JSON there instead of quoting it.
    let segments = if app.backend() == Backend::MariaDb {
        "JSON_ARRAY(JSON_OBJECT('start', 0.0, 'end', 1.5, 'text', ' Hello there.'), \
         JSON_OBJECT('start', 61.25, 'end', 3725.0, 'text', 'General Kenobi.'))"
            .to_string()
    } else {
        r#"'[{"start": 0.0, "end": 1.5, "text": " Hello there."}, {"start": 61.25, "end": 3725.0, "text": "General Kenobi."}]'"#
            .to_string()
    };
    exec(&app, &format!(
        r#"INSERT INTO "EpisodeTranscripts" (EpisodeID, Source, Status, TranscriptText, Segments)
           VALUES ({ep}, 'generated', 'complete', 'Hello there. General Kenobi.', {segments})"#,
    ))
    .await;
    exec(&app, &format!(
        r#"INSERT INTO "EpisodeTranscripts" (EpisodeID, Source, Status) VALUES ({unfinished}, 'generated', 'running')"#,
    ))
    .await;

    let srt = export(&app, &user, ep, "srt").await;
    srt.assert_ok();
    assert_eq!(
        srt.text(),
        "1\n00:00:00,000 --> 00:00:01,500\nHello there.\n\n2\n00:01:01,250 --> 01:02:05,000\nGeneral Kenobi.\n\n"
    );
    let disposition = srt.headers.get("content-disposition").unwrap().to_str().unwrap();
    assert!(disposition.contains(&format!("episode-{ep}-transcript.srt")), "{disposition}");

    let vtt = export(&app, &user, ep, "vtt").await;
    vtt.assert_ok();
    assert!(vtt.text().starts_with("WEBVTT\n\n00:00:00.000 --> 00:00:01.500\nHello there.\n"), "{}", vtt.text());
    assert_eq!(vtt.headers.get("content-type").unwrap(), "text/vtt; charset=utf-8");

    let txt = export(&app, &user, ep, "txt").await;
    txt.assert_ok();
    assert_eq!(txt.text(), "Hello there. General Kenobi.\n");

    export(&app, &user, unfinished, "txt").await.assert_status(404);
    export(&app, &user, ep, "docx").await.assert_status(400);
}

#[tokio::test(flavor = "multi_thread")]
async fn another_users_transcript_is_not_exported() {
    let app = TestApp::new().await;
    let owner = app.user("owner").await;
    let other = app.user("other").await;
    let url = app.feed("private.xml", &Feed::with_episodes("Private Show", 1));
    let pod = app.subscribe(&owner, &url, 1).await;
    let ep = app.episode_id(&owner, pod, "Episode 1").await;
    exec(&app, &format!(
        r#"INSERT INTO "EpisodeTranscripts" (EpisodeID, Source, Status, TranscriptText) VALUES ({ep}, 'generated', 'complete', 'secret')"#,
    ))
    .await;

    let status = export(&app, &other, ep, "txt").await.status.as_u16();
    assert!(status == 403 || status == 404, "got {status}");
    app.get("/api/data/episode_transcript_export")
        .key(&other)
        .query("episode_id", ep)
        .query("user_id", owner.id)
        .query("format", "txt")
        .send()
        .await
        .assert_status(403);
}
