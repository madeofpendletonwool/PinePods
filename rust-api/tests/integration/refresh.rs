//! Feed refresh efficiency and integrity: unchanged feeds skip
//! parsing, throttling hosts are backed off, racing refreshes can't duplicate episodes, adding a
//! podcast downloads its feed once, manual refresh revalidates, duration probes are bounded.

use pinepods_api::database::DatabasePool;

use crate::harness::*;

async fn title_of(app: &TestApp, user: &TestUser, podcast_id: i32, guid: &str) -> String {
    let id = int(app, &format!(r#"SELECT EpisodeID FROM "Episodes" WHERE PodcastID = {podcast_id} AND EpisodeGUID = '{guid}'"#))
        .await
        .unwrap();
    let eps = app.episodes(user, podcast_id).await;
    eps.iter().find(|e| e["Episodeid"] == id).unwrap()["Episodetitle"].as_str().unwrap().to_string()
}

async fn tamper_title(app: &TestApp, podcast_id: i32, guid: &str) {
    exec(app, &format!(r#"UPDATE "Episodes" SET EpisodeTitle = 'tampered' WHERE PodcastID = {podcast_id} AND EpisodeGUID = '{guid}'"#)).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unchanged_feed_is_not_reparsed_even_without_an_etag() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("No ETag Show", 2);
    let url = app.feed("plain.xml", &feed);
    let pod = app.subscribe(&user, &url, 2).await;

    // A local edit survives refreshes of an unchanged body: proof the feed wasn't applied again.
    tamper_title(&app, pod, "guid-1").await;
    app.refresh().await;
    app.refresh().await;
    assert_eq!(app.fixtures().hits("plain.xml").len(), 3, "the feed is still fetched every cycle");
    assert_eq!(title_of(&app, &user, pod, "guid-1").await, "tampered", "unchanged body must skip parse/apply");

    // A changed body is applied in full.
    app.feed("plain.xml", &feed.episode(Episode::new(3)));
    app.refresh().await;
    assert_eq!(app.episodes(&user, pod).await.len(), 3);
    assert_eq!(title_of(&app, &user, pod, "guid-1").await, "Episode 1");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_304_skips_parsing() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("ETag Show", 2);
    let url = app.fixtures().serve("etag.xml", Fixture::new("application/rss+xml", feed.to_xml()).etag("\"v1\""));
    let pod = app.subscribe(&user, &url, 2).await;
    app.refresh().await; // learns the ETag

    tamper_title(&app, pod, "guid-2").await;
    app.refresh().await;
    let hits = app.fixtures().hits("etag.xml");
    assert_eq!(hits.last().unwrap().headers.get("if-none-match").map(String::as_str), Some("\"v1\""));
    assert_eq!(title_of(&app, &user, pod, "guid-2").await, "tampered", "304 must skip parse/apply");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_throttling_host_is_left_alone_until_its_retry_after() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Busy Host", 2);
    let url = app.feed("busy.xml", &feed);
    let pod = app.subscribe(&user, &url, 2).await;

    app.fixtures().serve("busy.xml", Fixture::new("text/plain", "slow down").status(429).header("Retry-After", "3600"));
    app.refresh().await;
    assert_eq!(app.fixtures().hits("busy.xml").len(), 2, "a 429 is not retried within the cycle");
    let held = int(&app, &format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE PodcastID = {pod} AND RefreshNotBefore IS NOT NULL"#)).await;
    assert_eq!(held, Some(1), "Retry-After is recorded");

    // The host recovers, but we wait out its Retry-After.
    app.feed("busy.xml", &feed.clone().episode(Episode::new(3)));
    app.refresh().await;
    assert_eq!(app.fixtures().hits("busy.xml").len(), 2, "no request before Retry-After");
    assert_eq!(app.episodes(&user, pod).await.len(), 2);

    // Once the hold (and the failure backoff) has passed, the feed refreshes and the hold clears.
    exec(&app, &format!(r#"UPDATE "Podcasts" SET RefreshNotBefore = '2000-01-01 00:00:00', LastRefreshAttempt = '2000-01-01 00:00:00' WHERE PodcastID = {pod}"#)).await;
    app.refresh().await;
    assert_eq!(app.episodes(&user, pod).await.len(), 3);
    let held = int(&app, &format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE PodcastID = {pod} AND RefreshNotBefore IS NULL AND ConsecutiveFailures = 0"#)).await;
    assert_eq!(held, Some(1), "success clears the hold and the failure count");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_error_is_fetched_once_not_retried() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let url = app.feed("down.xml", &Feed::with_episodes("Down", 1));
    app.subscribe(&user, &url, 1).await;

    app.fixtures().serve("down.xml", Fixture::new("text/plain", "oops").status(500));
    app.refresh().await;
    assert_eq!(app.fixtures().hits("down.xml").len(), 2, "one request for the add, one for the refresh");
}

#[tokio::test(flavor = "multi_thread")]
async fn racing_refreshes_never_insert_an_episode_twice() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let mut feed = Feed::with_episodes("Racy", 1);
    let url = app.feed("racy.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;

    for n in 2..=40 {
        feed = feed.episode(Episode::new(n));
    }
    let xml = feed.to_xml();
    app.feed("racy.xml", &feed);
    let parsed = app.db().parse_feed_body(&xml, pod, "").await.unwrap();

    // Several appliers see the same 39 episodes as new at the same moment.
    let apply = || app.db().apply_parsed_episodes(pod, &parsed, "", None, false);
    let (a, b, c, d) = tokio::join!(apply(), apply(), apply(), apply());
    let reported: usize = [a, b, c, d].into_iter().map(|r| r.unwrap().len()).sum();
    assert_eq!(reported, 39, "each new episode is reported as new exactly once");
    assert_eq!(app.episodes(&user, pod).await.len(), 40);

    tokio::join!(app.refresh(), app.refresh());
    assert_eq!(app.episodes(&user, pod).await.len(), 40);
    let dups = int(&app, &format!(
        r#"SELECT COUNT(*) FROM (SELECT EpisodeGUID FROM "Episodes" WHERE PodcastID = {pod} GROUP BY EpisodeGUID HAVING COUNT(*) > 1) d"#
    ))
    .await;
    assert_eq!(dups, Some(0));
}

#[tokio::test(flavor = "multi_thread")]
async fn adding_a_podcast_downloads_its_feed_once() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let url = app.feed("once.xml", &Feed::with_episodes("Once", 3));
    app.subscribe(&user, &url, 3).await;
    assert_eq!(app.fixtures().hits("once.xml").len(), 1, "metadata and episodes come from one download");

    let custom = app.feed("custom.xml", &Feed::with_episodes("Custom", 2));
    let body = app
        .post("/api/data/add_custom_podcast")
        .key(&user)
        .json(&json!({ "user_id": user.id, "feed_url": custom }))
        .send()
        .await
        .ok_json();
    let pod = body["data"]["podcastid"].as_i64().unwrap_or_else(|| panic!("{body}")) as i32;
    assert_eq!(app.episodes(&user, pod).await.len(), 2);
    assert_eq!(app.fixtures().hits("custom.xml").len(), 1);
}

async fn manual_refresh(app: &TestApp, user: &TestUser) -> Vec<Value> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(1000);
    pinepods_api::handlers::refresh::run_refresh_process(user.id, false, tx, app.state.clone())
        .await
        .expect("manual refresh");
    let mut messages = Vec::new();
    while let Some(m) = rx.recv().await {
        messages.push(serde_json::to_value(&m).unwrap());
    }
    messages
}

#[tokio::test(flavor = "multi_thread")]
async fn manual_refresh_revalidates_and_skips_feeds_just_refreshed() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Manual", 2);
    let url = app.fixtures().serve("manual.xml", Fixture::new("application/rss+xml", feed.to_xml()).etag("\"v1\""));
    let pod = app.subscribe(&user, &url, 2).await;
    app.refresh().await;
    let after_scheduled = app.fixtures().hits("manual.xml").len();

    // The scheduled refresh just checked it: nothing to do.
    manual_refresh(&app, &user).await;
    assert_eq!(app.fixtures().hits("manual.xml").len(), after_scheduled, "refreshed under 2 minutes ago");

    let age = format!(r#"UPDATE "Podcasts" SET LastRefreshSuccess = '2000-01-01 00:00:00' WHERE PodcastID = {pod}"#);
    exec(&app, &age).await;
    manual_refresh(&app, &user).await;
    let hits = app.fixtures().hits("manual.xml");
    assert_eq!(hits.len(), after_scheduled + 1);
    assert_eq!(hits.last().unwrap().headers.get("if-none-match").map(String::as_str), Some("\"v1\""), "manual refresh is conditional");

    app.fixtures().serve("manual.xml", Fixture::new("application/rss+xml", feed.episode(Episode::new(3)).to_xml()).etag("\"v2\""));
    exec(&app, &age).await;
    let messages = manual_refresh(&app, &user).await;
    assert!(
        messages.iter().any(|m| m["new_episode"]["episodetitle"] == "Episode 3"),
        "new episodes are streamed: {messages:#?}"
    );
    assert_eq!(app.episodes(&user, pod).await.len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_back_catalogue_probes_the_newest_durations_first_and_backfills_the_rest() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    // ~2 MB so the Content-Length estimate kicks in; HEAD responses carry no body.
    let audio = Fixture::new("audio/mpeg", vec![0u8; 2_000_000]);
    let mut feed = Feed::new("Back Catalogue");
    for n in 1..=25 {
        let audio_url = app.fixtures().serve(&format!("audio/{n}.mp3"), audio.clone());
        let mut ep = Episode::new(n).duration(None).enclosure(&audio_url);
        ep.enclosure_len = 0;
        feed = feed.episode(ep);
    }
    let url = app.feed("catalogue.xml", &feed);
    let pod = app.subscribe(&user, &url, 25).await;

    eventually("every duration backfilled", || async {
        let eps = app.episodes(&user, pod).await;
        eps.iter().all(|e| e["Episodeduration"].as_i64().unwrap_or(0) > 0).then_some(())
    })
    .await;

    for n in 1..=25 {
        let hits = app.fixtures().hits(&format!("audio/{n}.mp3"));
        assert_eq!(hits.len(), 1, "episode {n} probed exactly once: {hits:?}");
        assert_eq!(hits[0].method, "HEAD");
    }
    // The ten newest were probed during the import, before any of the backfill.
    let order = app.fixtures().server.request_order(&app.fixtures().namespace, "/audio/");
    let first_ten: Vec<u32> = order[..10].iter().map(|p| p.trim_end_matches(".mp3").parse().unwrap()).collect();
    assert!(first_ten.iter().all(|n| *n >= 16), "newest first: {order:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_players_duration_fills_in_a_missing_one() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let mut ep = Episode::new(1).duration(None);
    ep.enclosure_len = 0;
    let url = app.feed("nodur.xml", &Feed::new("No Duration").episode(ep));
    let pod = app.subscribe(&user, &url, 1).await;
    let id = app.episode_id(&user, pod, "Episode 1").await;
    let duration = || async { app.episodes(&user, pod).await[0]["Episodeduration"].as_i64().unwrap_or(0) };
    assert_eq!(duration().await, 0, "the enclosure host is unreachable, so nothing could be estimated");

    let report = |d: f64| {
        app.post("/api/data/record_listen_duration")
            .key(&user)
            .json(&json!({ "episode_id": id, "user_id": user.id, "listen_duration": 30.0, "episode_duration": d }))
    };
    report(1234.4).send().await.assert_ok();
    assert_eq!(duration().await, 1234);
    report(999.0).send().await.assert_ok();
    assert_eq!(duration().await, 1234, "a known duration is never overwritten");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_dedupe_migration_merges_duplicate_episodes_and_their_state() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Dupes", 2);
    let url = app.feed("dupes.xml", &feed);
    let pod = app.subscribe(&user, &url, 2).await;
    let uid = user.id;
    let e1 = app.episode_id(&user, pod, "Episode 1").await;
    let e2 = app.episode_id(&user, pod, "Episode 2").await;

    // Recreate the pre-065 state: no unique index, duplicate rows from racing refreshes.
    match app.env.backend {
        Backend::Postgres => exec(&app, "DROP INDEX idx_episodes_podcastid_guid_unique").await,
        Backend::MariaDb => exec(&app, "DROP INDEX idx_episodes_podcastid_guidhash_unique ON Episodes").await,
    }
    let dup = |guid: &str, completed: bool| {
        format!(
            r#"INSERT INTO "Episodes" (PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration, Completed, EpisodeGUID)
               SELECT PodcastID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration, {completed}, EpisodeGUID
               FROM "Episodes" WHERE PodcastID = {pod} AND EpisodeGUID = '{guid}' ORDER BY EpisodeID LIMIT 1"#
        )
    };
    exec(&app, &dup("guid-1", false)).await;
    exec(&app, &dup("guid-1", true)).await;
    exec(&app, &dup("guid-2", false)).await;
    let ids = |guid: &str| {
        let app = &app;
        let guid = guid.to_string();
        async move {
            let mut out = Vec::new();
            let mut last = 0;
            while let Some(id) = int(app, &format!(
                r#"SELECT MIN(EpisodeID) FROM "Episodes" WHERE PodcastID = {pod} AND EpisodeGUID = '{guid}' AND EpisodeID > {last}"#
            ))
            .await
            {
                out.push(id);
                last = id;
            }
            out
        }
    };
    let g1 = ids("guid-1").await;
    let g2 = ids("guid-2").await;
    assert_eq!((g1.len(), g2.len()), (3, 2), "seeded duplicates");
    let (d1a, d1b, d2a) = (g1[1], g1[2], g2[1]);

    for sql in [
        // Episode 1: history on the survivor and a duplicate → max position, latest date.
        format!(r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES ({uid}, {e1}, '2026-03-02 10:00:00', 100)"#),
        format!(r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES ({uid}, {d1a}, '2026-03-01 10:00:00', 500)"#),
        // Episode 2: history only on the duplicate → moved to the survivor.
        format!(r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES ({uid}, {d2a}, '2026-03-03 10:00:00', 42)"#),
        format!(r#"INSERT INTO "SavedEpisodes" (UserID, EpisodeID, SaveDate) VALUES ({uid}, {d1a}, '2026-03-01 10:00:00')"#),
        format!(r#"INSERT INTO "SavedEpisodes" (UserID, EpisodeID, SaveDate) VALUES ({uid}, {d1b}, '2026-03-02 10:00:00')"#),
        format!(r#"INSERT INTO "EpisodeQueue" (QueueDate, UserID, EpisodeID, QueuePosition, is_youtube) VALUES ('2026-03-01 10:00:00', {uid}, {d1b}, 1, FALSE)"#),
        format!(r#"INSERT INTO "DownloadedEpisodes" (UserID, EpisodeID, DownloadedDate, DownloadedSize, DownloadedLocation) VALUES ({uid}, {d2a}, '2026-03-01 10:00:00', 10, '/downloads/x.mp3')"#),
        format!(r#"INSERT INTO "SharedEpisodes" (EpisodeID, SharedBy, ShareCode, ExpirationDate) VALUES ({d1a}, {uid}, 'dedupe-share-{d1a}', '2030-01-01 00:00:00')"#),
        format!(r#"INSERT INTO "EpisodeSkipSegments" (EpisodeID, Kind, StartTime, EndTime, Source) VALUES ({d1b}, 'silence', 1.0, 2.0, 'test')"#),
    ] {
        exec(&app, &sql).await;
    }

    // Run the real migration runner for 064 + 065 against this database.
    exec(&app, r#"DELETE FROM schema_migrations WHERE version IN ('064', '065')"#).await;
    let env = app.env;
    let db_name = app.db_name.clone();
    tokio::task::spawn_blocking(move || crate::harness::env::run_migrations(env, &db_name)).await.unwrap();

    let count = |sql: String| {
        let app = &app;
        async move { int(app, &sql).await.unwrap_or(0) }
    };
    assert_eq!(count(format!(r#"SELECT COUNT(*) FROM "Episodes" WHERE PodcastID = {pod}"#)).await, 2);
    assert_eq!(count(format!(r#"SELECT EpisodeCount FROM "Podcasts" WHERE PodcastID = {pod}"#)).await, 2);
    for table in ["UserEpisodeHistory", "SavedEpisodes", "EpisodeQueue", "DownloadedEpisodes", "SharedEpisodes", "EpisodeSkipSegments"] {
        assert_eq!(
            count(format!(r#"SELECT COUNT(*) FROM "{table}" WHERE EpisodeID IN ({d1a}, {d1b}, {d2a})"#)).await,
            0,
            "{table} still points at a removed duplicate"
        );
    }
    assert_eq!(count(format!(r#"SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = {uid} AND EpisodeID = {e1}"#)).await, 500);
    assert_eq!(
        count(format!(r#"SELECT COUNT(*) FROM "UserEpisodeHistory" WHERE UserID = {uid} AND EpisodeID = {e1} AND ListenDate = '2026-03-02 10:00:00'"#)).await,
        1,
        "latest listen date kept"
    );
    assert_eq!(count(format!(r#"SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = {uid} AND EpisodeID = {e2}"#)).await, 42);
    assert_eq!(count(format!(r#"SELECT COUNT(*) FROM "SavedEpisodes" WHERE UserID = {uid} AND EpisodeID = {e1}"#)).await, 1);
    assert_eq!(count(format!(r#"SELECT COUNT(*) FROM "EpisodeQueue" WHERE UserID = {uid} AND EpisodeID = {e1}"#)).await, 1);
    assert_eq!(count(format!(r#"SELECT COUNT(*) FROM "DownloadedEpisodes" WHERE UserID = {uid} AND EpisodeID = {e2}"#)).await, 1);
    assert_eq!(count(format!(r#"SELECT COUNT(*) FROM "SharedEpisodes" WHERE EpisodeID = {e1}"#)).await, 1);
    assert_eq!(count(format!(r#"SELECT COUNT(*) FROM "EpisodeSkipSegments" WHERE EpisodeID = {e1}"#)).await, 1);
    assert_eq!(count(format!(r#"SELECT COUNT(*) FROM "Episodes" WHERE EpisodeID = {e1} AND Completed = TRUE"#)).await, 1, "completion carried over");

    // The users see one copy, with the merged state.
    let eps = app.episodes(&user, pod).await;
    assert_eq!(eps.len(), 2, "{eps:#?}");
    let ep1 = eps.iter().find(|e| e["Episodeid"] == e1).unwrap();
    assert_eq!(ep1["saved"], true);
    assert_eq!(ep1["queued"], true);

    // And the pair is unique again.
    let insert = dup("guid-1", false);
    let rejected = match app.db() {
        DatabasePool::Postgres(pool) => sqlx::raw_sql(sqlx::AssertSqlSafe(insert)).execute(pool).await.is_err(),
        DatabasePool::MySQL(pool) => sqlx::raw_sql(sqlx::AssertSqlSafe(insert.replace('"', ""))).execute(pool).await.is_err(),
    };
    assert!(rejected, "a duplicate GUID insert must fail after 065");
}
