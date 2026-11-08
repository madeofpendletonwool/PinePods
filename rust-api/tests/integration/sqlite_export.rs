//! Moving a PostgreSQL or MariaDB server to SQLite and back: `database_functions.sqlite_export`
//! writes a SQLite backup, which a new SQLite server restores; `database_functions.sqlite_import`
//! fills a new PostgreSQL or MariaDB server from a SQLite database or backup. The library looks
//! the same each time.

use std::path::{Path, PathBuf};
use std::process::Command;

use pinepods_api::config::DatabaseConfig;

use crate::harness::*;

fn python() -> String {
    std::env::var("PINEPODS_TEST_PYTHON").unwrap_or_else(|_| "python3".into())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

fn run(cmd: &mut Command, what: &str) {
    let out = cmd.current_dir(repo_root()).output().unwrap_or_else(|e| panic!("{what}: {e}"));
    assert!(
        out.status.success(),
        "{what} failed:\n{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

/// Ids and titles, in order, of a list of rows (a stable view across databases).
fn ids(rows: &Value, id: &str, title: &str) -> Vec<(i64, String)> {
    rows.as_array()
        .unwrap_or_else(|| panic!("not a list: {rows}"))
        .iter()
        .map(|r| (r[id].as_i64().unwrap(), r[title].as_str().unwrap_or_default().to_string()))
        .collect()
}

async fn library(app: &TestApp, u: &TestUser, playlist: i32) -> Value {
    let get = |path: String| async move { app.get(&path).key(u).query("user_id", u.id).send().await.ok_json() };
    let pods = get(format!("/api/data/return_pods/{}", u.id)).await;
    let saved = get(format!("/api/data/saved_episode_list/{}", u.id)).await;
    let queued = get("/api/data/get_queued_episodes".into()).await;
    let playlists = get("/api/data/get_playlists".into()).await;
    let in_playlist = app
        .get("/api/data/get_playlist_episodes")
        .key(u)
        .query("user_id", u.id)
        .query("playlist_id", playlist)
        .query("limit", 100)
        .send()
        .await
        .ok_json();
    let mut names: Vec<String> =
        playlists["playlists"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect();
    names.sort();
    json!({
        "pods": ids(&pods["pods"], "podcastid", "podcastname"),
        "saved": ids(&saved["saved_episodes"], "episodeid", "episodetitle"),
        "queued": ids(&queued["data"], "episodeid", "episodetitle"),
        "playlists": names,
        "in_playlist": ids(&in_playlist["episodes"], "episodeid", "episodetitle"),
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_moves_to_sqlite_and_back() {
    let source = TestApp::new().await;
    if source.backend() == Backend::Sqlite {
        return;
    }
    let user = source.user("mover").await;
    let url = source.feed("mover.xml", &Feed::with_episodes("Moving Show", 4));
    let pod = source.subscribe(&user, &url, 4).await;
    let ep1 = source.episode_id(&user, pod, "Episode 1").await;
    let ep2 = source.episode_id(&user, pod, "Episode 2").await;
    let body = |ep: i32| json!({ "episode_id": ep, "user_id": user.id, "is_youtube": false });
    source.post("/api/data/save_episode").key(&user).json(&body(ep1)).send().await.assert_ok();
    source.post("/api/data/queue_pod").key(&user).json(&body(ep2)).send().await.assert_ok();
    // A playlist limited to one podcast (an integer array on PostgreSQL, JSON on MariaDB).
    let playlist = source
        .post("/api/data/create_playlist")
        .key(&user)
        .json(&json!({
            "user_id": user.id, "name": "Only this show", "include_unplayed": true,
            "include_partially_played": true, "include_played": false, "sort_order": "date_desc",
            "group_by_podcast": false, "icon_name": "ph-playlist", "podcast_ids": [pod],
        }))
        .send()
        .await
        .ok_json()["playlist_id"]
        .as_i64()
        .unwrap() as i32;
    exec(&source, &format!(r#"UPDATE "Users" SET PlaybackSpeed = 1.5, LastSyncTime = '2026-01-02 03:04:05' WHERE UserID = {}"#, user.id)).await;
    let before = library(&source, &user, playlist).await;

    let dir = std::env::temp_dir().join(format!("pinepods-export-{}-{}", std::process::id(), user.id));
    std::fs::create_dir_all(&dir).unwrap();
    let export = dir.join("export.sql");
    let target_db = dir.join("pinepods.db");
    let test_env = env::env();
    run(
        Command::new(python())
            .args(["-m", "database_functions.sqlite_export"])
            .arg(&export)
            .env("DB_TYPE", test_env.backend.db_type())
            .env("DB_HOST", &test_env.db_host)
            .env("DB_PORT", test_env.db_port.to_string())
            .env("DB_USER", &test_env.db_user)
            .env("DB_PASSWORD", &test_env.db_password)
            .env("DB_NAME", &source.db_name),
        "export",
    );
    // A new SQLite server, set up the way the container sets one up.
    run(
        Command::new(python())
            .arg("startup/setup_database_new.py")
            .env_remove("USERNAME")
            .env_remove("PASSWORD")
            .env_remove("FULLNAME")
            .env_remove("EMAIL")
            .env("DB_TYPE", "sqlite")
            .env("DB_PATH", &target_db),
        "SQLite setup",
    );

    let path = target_db.to_string_lossy().into_owned();
    let target = TestApp::with_config(move |c| {
        c.database = DatabaseConfig {
            db_type: "sqlite".into(),
            host: String::new(),
            port: 0,
            username: String::new(),
            password: String::new(),
            name: path,
            max_connections: 4,
            min_connections: 0,
        };
        c.redis = None;
    })
    .await;
    target.db().restore_server_data_from_path(&export).await.unwrap();

    // The same user, key and library on SQLite.
    assert_eq!(library(&target, &user, playlist).await, before);
    assert_eq!(
        text(&target, &format!(r#"SELECT CAST(playbackspeed AS TEXT) || ' ' || lastsynctime FROM "Users" WHERE userid = {}"#, user.id)).await.as_deref(),
        Some("1.5 2026-01-02 03:04:05")
    );
    // Every table's rows came across (login sessions aren't exported).
    let tables: Vec<String> = match target.db() {
        pinepods_api::database::DatabasePool::Sqlite(pool) => sqlx::query_scalar(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT IN ('schema_migrations', 'Sessions', 'GpodderSessions')",
        )
        .fetch_all(pool)
        .await
        .unwrap(),
        _ => unreachable!(),
    };
    for table in &tables {
        let count = format!(r#"SELECT COUNT(*) FROM "{table}""#);
        assert_eq!(int(&target, &count).await, int(&source, &count).await, "rows in {table}");
    }
    // New rows still get fresh ids after the move.
    target.subscribe(&user, &target.feed("after.xml", &Feed::with_episodes("After Show", 1)), 1).await;
    let on_sqlite = library(&target, &user, playlist).await;

    // Back to PostgreSQL or MariaDB, from the database file and from a backup.
    let backup = dir.join("sqlite_backup.sql");
    target.db().sqlite_backup_to(&backup).await.unwrap().unwrap();
    let back = TestApp::new().await;
    for (source_file, replace) in [(&target_db, false), (&backup, true)] {
        let mut cmd = Command::new(python());
        cmd.args(["-m", "database_functions.sqlite_import"])
            .arg(source_file)
            .env("DB_TYPE", test_env.backend.db_type())
            .env("DB_HOST", &test_env.db_host)
            .env("DB_PORT", test_env.db_port.to_string())
            .env("DB_USER", &test_env.db_user)
            .env("DB_PASSWORD", &test_env.db_password)
            .env("DB_NAME", &back.db_name);
        if replace {
            cmd.arg("--replace");
        }
        run(&mut cmd, "import");

        assert_eq!(library(&back, &user, playlist).await, on_sqlite);
        let settings = format!(
            r#"SELECT COUNT(*) FROM "Users" WHERE UserID = {} AND PlaybackSpeed = 1.5 AND LastSyncTime IS NOT NULL"#,
            user.id
        );
        assert_eq!(int(&back, &settings).await, Some(1));
        for table in &tables {
            let count = format!(r#"SELECT COUNT(*) FROM "{table}""#);
            assert_eq!(int(&back, &count).await, int(&target, &count).await, "rows in {table} after import");
        }
        back.subscribe(&user, &back.feed("later.xml", &Feed::with_episodes("Later Show", 1)), 1).await;
        // The next import replaces everything again, including this podcast.
    }
    // Without --replace, an import won't overwrite a server that has podcasts.
    let out = Command::new(python())
        .args(["-m", "database_functions.sqlite_import"])
        .arg(&backup)
        .current_dir(repo_root())
        .env("DB_TYPE", test_env.backend.db_type())
        .env("DB_HOST", &test_env.db_host)
        .env("DB_PORT", test_env.db_port.to_string())
        .env("DB_USER", &test_env.db_user)
        .env("DB_PASSWORD", &test_env.db_password)
        .env("DB_NAME", &back.db_name)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--replace"));

    let _ = std::fs::remove_dir_all(&dir);
}
