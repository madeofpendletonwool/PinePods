//! Searching inside transcripts: feed transcripts are fetched and stored, generated ones are
//! found too, and each hit says when the words were said.

use crate::harness::sql;
use crate::harness::*;
use pinepods_api::services::feed_transcripts;

const VTT: &str = "WEBVTT\n\n00:00:01.000 --> 00:00:04.000\nWelcome to the show.\n\n00:01:30.000 --> 00:01:35.000\n<v Host>Today: Kubernetes networking.</v>\n";
const SRT: &str = "1\n00:00:02,000 --> 00:00:05,000\nNothing about containers here\n";

fn transcript_tag(url: &str, mime: &str) -> String {
    format!("      <podcast:transcript url=\"{}\" type=\"{mime}\" language=\"en\"/>\n", crate::harness::rss::esc(url))
}

async fn search(app: &TestApp, u: &TestUser, q: &str) -> Value {
    app.get("/api/data/transcripts/search").key(u).query("q", q).send().await.ok_json()
}

/// Transcript files fetched so far.
fn transcript_fetches(app: &TestApp) -> usize {
    let f = app.fixtures();
    f.server.request_order(&f.namespace, "/t/").len()
}

fn titles(res: &Value) -> Vec<String> {
    res["results"].as_array().unwrap().iter().map(|r| r["episode"]["episodetitle"].as_str().unwrap().to_string()).collect()
}

/// A show whose episode 2 publishes a VTT transcript and episode 1 an SRT one, with the feed
/// transcripts stored. Returns (listener, episode 2 id, episode 1 id).
async fn show_with_feed_transcripts(app: &TestApp) -> (TestUser, i32, i32) {
    let vtt = app.fixtures().serve("t/two.vtt", Fixture::new("text/vtt", VTT));
    let srt = app.fixtures().serve("t/one.srt", Fixture::new("application/x-subrip", SRT));
    let feed = Feed::new("Cloud Talk")
        .episode(Episode::new(1).extra(&transcript_tag(&srt, "application/x-subrip")))
        .episode(Episode::new(2).extra(&transcript_tag(&vtt, "text/vtt")));
    let user = app.user("listener").await;
    let url = app.fixtures().serve_feed("cloud.xml", feed.to_xml());
    let pod = app.subscribe(&user, &url, 2).await;
    let two = app.episode_id(&user, pod, "Episode 2").await;
    let one = app.episode_id(&user, pod, "Episode 1").await;
    feed_transcripts::run_due(app.db()).await.unwrap();
    (user, two, one)
}

#[tokio::test(flavor = "multi_thread")]
async fn feed_transcripts_are_stored_and_searchable_with_timestamps() {
    let app = TestApp::new().await;
    let (user, two, one) = show_with_feed_transcripts(&app).await;

    let stored = sql::int(&app, r#"SELECT COUNT(*) FROM "EpisodeTranscripts" WHERE source = 'feed' AND status = 'complete'"#).await;
    assert_eq!(stored, Some(2));

    let res = search(&app, &user, "kubernetes").await;
    assert_eq!(res["total"], 1, "{res:#}");
    assert_eq!(res["searchable_episodes"], 2);
    assert_eq!(res["terms"], json!(["kubernetes"]));
    let hit = &res["results"][0];
    assert_eq!(hit["episode"]["episodeid"], two);
    assert_eq!(hit["source"], "feed");
    assert_eq!(hit["mentions"], 1);
    assert_eq!(hit["moments"][0]["start"], 90.0);
    assert!(hit["moments"][0]["text"].as_str().unwrap().contains("Today: Kubernetes networking."), "{hit:#}");

    // Phrases, every word required, exclusions.
    assert_eq!(titles(&search(&app, &user, "\"kubernetes networking\"").await), ["Episode 2"]);
    assert_eq!(titles(&search(&app, &user, "kubernetes containers").await), Vec::<String>::new());
    let res = search(&app, &user, "containers -kubernetes").await;
    assert_eq!(res["results"][0]["episode"]["episodeid"], one, "{res:#}");
    assert_eq!(search(&app, &user, "-kubernetes").await["total"], 0, "only exclusions match nothing");

    // Stored once: the next run has nothing to fetch.
    let before = transcript_fetches(&app);
    feed_transcripts::run_due(app.db()).await.unwrap();
    assert_eq!(transcript_fetches(&app), before);
}

#[tokio::test(flavor = "multi_thread")]
async fn only_your_own_podcasts_are_searched() {
    let app = TestApp::new().await;
    let (user, _, _) = show_with_feed_transcripts(&app).await;
    assert_eq!(search(&app, &user, "kubernetes").await["total"], 1);

    let stranger = app.user("stranger").await;
    let res = search(&app, &stranger, "kubernetes").await;
    assert_eq!(res["total"], 0, "{res:#}");
    assert_eq!(res["searchable_episodes"], 0);

    app.get("/api/data/transcripts/search").query("q", "kubernetes").send().await.assert_status(401);
}

/// Store a generated transcript the way the transcription job leaves it.
async fn insert_generated(app: &TestApp, episode_id: i32, text: &str, segments: &str) {
    use pinepods_api::database::DatabasePool;
    match app.db() {
        DatabasePool::Postgres(pool) => sqlx::query(
            r#"INSERT INTO "EpisodeTranscripts" (episodeid, source, model, transcripttext, segments, status) VALUES ($1, 'generated', 'base', $2, $3::jsonb, 'complete')"#,
        )
        .bind(episode_id).bind(text).bind(segments).execute(pool).await.map(|_| ()),
        DatabasePool::Sqlite(pool) => sqlx::query(
            r#"INSERT INTO "EpisodeTranscripts" (episodeid, source, model, transcripttext, segments, status) VALUES ($1, 'generated', 'base', $2, $3, 'complete')"#,
        )
        .bind(episode_id).bind(text).bind(segments).execute(pool).await.map(|_| ()),
        DatabasePool::MySQL(pool) => sqlx::query(
            "INSERT INTO EpisodeTranscripts (EpisodeID, Source, Model, TranscriptText, Segments, Status) VALUES (?, 'generated', 'base', ?, ?, 'complete')",
        )
        .bind(episode_id).bind(text).bind(segments).execute(pool).await.map(|_| ()),
    }
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn generated_transcripts_are_searched_and_preferred_over_the_feed_one() {
    let app = TestApp::new().await;
    let (user, two, _) = show_with_feed_transcripts(&app).await;
    insert_generated(
        &app,
        two,
        "intro words. kubernetes again here",
        r#"[{"start": 0.0, "end": 3.0, "text": "intro words."}, {"start": 125.5, "end": 128.0, "text": "kubernetes again here"}]"#,
    )
    .await;

    let res = search(&app, &user, "kubernetes").await;
    assert_eq!(res["total"], 1, "one hit per episode: {res:#}");
    assert_eq!(res["results"][0]["source"], "generated");
    assert_eq!(res["results"][0]["moments"][0]["start"], 125.5);

    // Words only in the feed transcript still find the episode, through that transcript.
    let res = search(&app, &user, "welcome").await;
    assert_eq!(res["results"][0]["source"], "feed", "{res:#}");
}

#[tokio::test(flavor = "multi_thread")]
async fn transcripts_that_cant_be_fetched_are_retried_later_not_every_run() {
    let app = TestApp::new().await;
    let missing = app.fixtures().url("t/missing.vtt");
    let feed = Feed::new("Broken Links").episode(Episode::new(1).extra(&transcript_tag(&missing, "text/vtt")));
    let user = app.user("listener").await;
    let url = app.fixtures().serve_feed("broken.xml", feed.to_xml());
    app.subscribe(&user, &url, 1).await;

    feed_transcripts::run_due(app.db()).await.unwrap();
    let failed = sql::int(&app, r#"SELECT COUNT(*) FROM "EpisodeTranscripts" WHERE source = 'feed' AND status = 'failed'"#).await;
    assert_eq!(failed, Some(1));
    let before = transcript_fetches(&app);
    feed_transcripts::run_due(app.db()).await.unwrap();
    assert_eq!(transcript_fetches(&app), before, "a failed fetch waits before it's tried again");
    assert_eq!(search(&app, &user, "anything").await["searchable_episodes"], 0);
}
