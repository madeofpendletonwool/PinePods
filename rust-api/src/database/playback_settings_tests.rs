//! Run with explicit disposable database URLs: cargo test playback_settings_tests -- --ignored.
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
            sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
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
}

async fn check_settings(db: &DatabasePool) {
    for (user, podcast, expected) in [
        (1, 1, (1.75, 10, 20, false)),
        (1, 2, (2.25, 30, 40, true)),
        (1, 3, (1.75, 0, 0, false)),
        (1, 4, (1.75, 0, 0, true)),
        (2, 5, (1.0, 0, 0, false)),
        (1, 999, (1.75, 0, 0, false)),
        (999, 1, (1.0, 0, 0, false)),
    ] {
        for youtube in [false, true] {
            assert_eq!(
                db.get_play_episode_details(user, podcast, youtube)
                    .await
                    .unwrap(),
                expected
            );
        }
    }
}

#[tokio::test]
#[ignore = "requires PINEPODS_TEST_POSTGRES_URL pointing to a disposable test server"]
async fn postgres_playback_settings() {
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
    check_settings(&db).await;
    execute(&db, &format!("DROP SCHEMA {schema} CASCADE")).await;
    pool.close().await;
}

#[tokio::test]
#[ignore = "requires PINEPODS_TEST_MYSQL_URL pointing to a disposable test server"]
async fn mysql_playback_settings() {
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
    check_settings(&db).await;
    execute(&db, &format!("DROP DATABASE {schema}")).await;
    pool.close().await;
}
