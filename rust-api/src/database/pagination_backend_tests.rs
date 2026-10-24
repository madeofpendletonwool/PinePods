//! Run with explicit disposable database URLs: cargo test pagination_backend_tests -- --ignored.
//! Each backend gets a unique schema/database; existing tables are never touched.

use super::DatabasePool;

async fn execute(db: &DatabasePool, sql: &str) {
    match db {
        DatabasePool::Postgres(pool) => {
            sqlx::query(sqlx::AssertSqlSafe(sql))
                .execute(pool)
                .await
                .unwrap();
        }
        DatabasePool::MySQL(pool) => {
            // Column names are case-insensitive; table names retain their schema spelling.
            let sql = sql.replace('"', "").replace("is_video", "IsVideo");
            // MySQL does not support USE through the prepared-statement protocol.
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql.as_str()))
                .execute(pool)
                .await
                .unwrap();
        }
    }
}

async fn fixtures(db: &DatabasePool) {
    for sql in [
        r#"CREATE TABLE "Users" (userid INT PRIMARY KEY, usepodcastcovers BOOLEAN DEFAULT FALSE, playbackspeed DECIMAL(3,2))"#,
        r#"CREATE TABLE "Podcasts" (podcastid INT PRIMARY KEY, userid INT, podcastname TEXT, artworkurl TEXT,
            usepodcastcovers BOOLEAN DEFAULT FALSE, usepodcastcoverscustomized BOOLEAN DEFAULT FALSE,
            mergedpodcastids TEXT, playbackspeed DECIMAL(3,2), playbackspeedcustomized BOOLEAN,
            startskip INT, endskip INT)"#,
        r#"CREATE TABLE "Episodes" (episodeid INT PRIMARY KEY, podcastid INT, episodetitle TEXT,
            episodepubdate TIMESTAMP, episodedescription TEXT, episodeartwork TEXT, episodeurl TEXT,
            episodeduration INT, completed BOOLEAN DEFAULT FALSE, is_video BOOLEAN DEFAULT FALSE)"#,
        r#"CREATE TABLE "YouTubeVideos" (videoid INT PRIMARY KEY, podcastid INT, videotitle TEXT,
            publishedat TIMESTAMP, videodescription TEXT, thumbnailurl TEXT, videourl TEXT,
            duration INT, listenposition INT, completed BOOLEAN DEFAULT FALSE)"#,
        r#"CREATE TABLE "UserEpisodeHistory" (userid INT, episodeid INT, listenduration INT)"#,
        r#"CREATE TABLE "SavedEpisodes" (userid INT, episodeid INT)"#,
        r#"CREATE TABLE "DownloadedEpisodes" (userid INT, episodeid INT)"#,
        r#"CREATE TABLE "EpisodeQueue" (userid INT, episodeid INT, is_youtube BOOLEAN)"#,
        r#"CREATE TABLE "SavedVideos" (userid INT, videoid INT)"#,
        r#"CREATE TABLE "DownloadedVideos" (userid INT, videoid INT)"#,
        r#"INSERT INTO "Users" VALUES (1, FALSE, 1.75), (2, FALSE, NULL)"#,
        r#"INSERT INTO "Podcasts" (podcastid, userid, podcastname, mergedpodcastids, playbackspeed,
            playbackspeedcustomized, startskip, endskip) VALUES
            (1, 1, 'Main', '[2,3,2]', 2.50, FALSE, 10, 20),
            (2, 1, 'Merged', NULL, 2.25, TRUE, 30, 40),
            (3, 2, 'Other user', NULL, 3.00, TRUE, 50, 60),
            (4, 1, 'Null override', NULL, NULL, TRUE, NULL, NULL),
            (5, 2, 'Defaults', NULL, NULL, NULL, NULL, NULL)"#,
    ] {
        execute(db, sql).await;
    }
    for id in 1..=6 {
        let podcast = if id % 2 == 0 { 2 } else { 1 };
        let done = if id <= 2 { "TRUE" } else { "FALSE" };
        // Adjacent items tie on each sort key; RSS and YouTube also share IDs.
        let key = (id + 1) / 2;
        execute(db, &format!(r#"INSERT INTO "Episodes" VALUES
            ({id}, {podcast}, 'Title {key}', '2026-01-0{key} 00:00:00', 'searchable', '', 'https://example.test/a',
            {duration}, {done}, FALSE)"#, duration = 700 - key * 100)).await;
        execute(db, &format!(r#"INSERT INTO "YouTubeVideos" VALUES
            ({id}, {podcast}, 'Title {key}', '2026-01-0{key} 00:00:00', 'searchable', '', 'https://example.test/v',
            {duration}, 15, {done})"#, duration = 700 - key * 100)).await;
        execute(
            db,
            &format!(r#"INSERT INTO "UserEpisodeHistory" VALUES (1, {id}, 15)"#),
        )
        .await;
    }
    // A merged ID belonging to someone else must not leak into either results or totals.
    execute(
        db,
        r#"INSERT INTO "Episodes" VALUES
        (99, 3, 'Other', '2026-01-01 00:00:00', '', '', '', 1, FALSE, FALSE)"#,
    )
    .await;
}

async fn check_pages(db: &DatabasePool) {
    for sort in ["date", "title", "duration"] {
        for direction in ["asc", "desc"] {
            for filter in ["", "completed", "incomplete", "in_progress"] {
                let mut expected: Vec<(bool, i32)> = (1..=6)
                    .filter(|id| match filter {
                        "completed" => *id <= 2,
                        "incomplete" | "in_progress" => *id > 2,
                        _ => true,
                    })
                    .flat_map(|id| [(false, id), (true, id)])
                    .collect();
                expected.sort_by_key(|(youtube, id)| {
                    let mut key = (id + 1) / 2;
                    if sort == "duration" {
                        key = -key;
                    }
                    if direction == "desc" {
                        key = -key;
                    }
                    (key, *youtube, *id)
                });
                let mut actual = Vec::new();
                for offset in (0..=expected.len()).step_by(3) {
                    let (page, total) = db
                        .return_podcast_episodes_capitalized(
                            1,
                            1,
                            Some(3),
                            Some(offset as i64),
                            sort,
                            direction,
                            "",
                            filter,
                        )
                        .await
                        .unwrap();
                    assert_eq!(total, expected.len() as i64);
                    actual.extend(page.into_iter().map(|e| (e.is_youtube, e.episodeid)));
                }
                assert_eq!(actual, expected, "{sort}/{direction}/{filter}");
            }
        }
    }
    let (page, total) = db
        .return_podcast_episodes_capitalized(
            1,
            1,
            Some(2),
            Some(0),
            "date",
            "asc",
            "Title 2",
            "in_progress",
        )
        .await
        .unwrap();
    assert_eq!(total, 4);
    assert_eq!(
        page.iter().map(|e| e.episodeid).collect::<Vec<_>>(),
        vec![3, 4]
    );
    let (page, total) = db
        .return_podcast_episodes_capitalized(2, 1, Some(3), Some(0), "date", "asc", "", "")
        .await
        .unwrap();
    assert!(page.is_empty());
    assert_eq!(total, 0);
}

#[tokio::test]
#[ignore = "requires PINEPODS_TEST_POSTGRES_URL pointing to a disposable test server"]
async fn postgres_mixed_source_pagination() {
    let url = std::env::var("PINEPODS_TEST_POSTGRES_URL").expect("explicit test URL required");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect(&url)
        .await
        .unwrap();
    let db = DatabasePool::Postgres(pool.clone());
    let schema = format!("pinepods_test_{}", uuid::Uuid::new_v4().simple());
    execute(&db, &format!("CREATE SCHEMA {schema}")).await;
    execute(&db, &format!("SET search_path TO {schema}")).await;
    fixtures(&db).await;
    check_pages(&db).await;
    execute(&db, &format!("DROP SCHEMA {schema} CASCADE")).await;
    pool.close().await;
}

#[tokio::test]
#[ignore = "requires PINEPODS_TEST_MYSQL_URL pointing to a disposable test server"]
async fn mysql_mixed_source_pagination() {
    let url = std::env::var("PINEPODS_TEST_MYSQL_URL").expect("explicit test URL required");
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .max_connections(1)
        .idle_timeout(None)
        .max_lifetime(None)
        .connect(&url)
        .await
        .unwrap();
    let db = DatabasePool::MySQL(pool.clone());
    let schema = format!("pinepods_test_{}", uuid::Uuid::new_v4().simple());
    execute(&db, &format!("CREATE DATABASE {schema}")).await;
    execute(&db, &format!("USE {schema}")).await;
    fixtures(&db).await;
    check_pages(&db).await;
    execute(&db, &format!("DROP DATABASE {schema}")).await;
    pool.close().await;
}
