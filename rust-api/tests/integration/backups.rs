//! Backups and restores with the real dump tools (`pg_dump`/`psql`, `mysqldump`/`mysql`):
//! the backup folder, uploading a dump, and scheduled backups with retention.

use std::sync::Arc;

use crate::harness::files::{assert_gone, BackupEnv};
use crate::harness::*;

const BOUNDARY: &str = "pinepodstestboundary";
const MULTIPART: &str = "multipart/form-data; boundary=pinepodstestboundary";

fn upload(filename: &str, content: &[u8]) -> Vec<u8> {
    let mut body = format!(
        "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"backup_file\"; filename=\"{filename}\"\r\nContent-Type: application/sql\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(content);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

async fn manual_backup(app: &TestApp, admin: &TestUser) -> String {
    let body = app.post("/api/data/manual_backup_to_directory").key(admin).json(&json!({})).send().await.ok_json();
    let filename = body["filename"].as_str().unwrap().to_string();
    let task = body["task_id"].as_str().unwrap().to_string();
    wait_for_task(app, admin, &task).await;
    // The admin who started it can follow it.
    app.get(&format!("/api/tasks/{task}")).key(admin).send().await.assert_ok();
    filename
}

/// Wait for a background task to finish and return its final state. Reads the task store, not
/// the API: mid-restore the API key tables may be empty, so requests can't authenticate.
async fn finished_task(app: &TestApp, _user: &TestUser, task: &str) -> Value {
    eventually("task finished", || async {
        let info = app.state.task_manager.get_task(task).await.ok()?;
        let info = serde_json::to_value(info).unwrap();
        matches!(info["status"].as_str(), Some("SUCCESS" | "FAILED")).then_some(info)
    })
    .await
}

/// Wait for a background task, failing the test if the task failed.
async fn wait_for_task(app: &TestApp, user: &TestUser, task: &str) {
    let info = finished_task(app, user, task).await;
    assert_eq!(info["status"], "SUCCESS", "task failed: {info}");
}

async fn wait_for_restore_to_end(app: &TestApp) {
    eventually("restore finished", || async {
        let status = app.get("/api/data/restore_status").send().await.ok_json();
        (status["restore_in_progress"] == false).then_some(())
    })
    .await;
}

async fn files(app: &TestApp, admin: &TestUser) -> Vec<String> {
    let body = app.post("/api/data/list_backup_files").key(admin).json(&json!({})).send().await.ok_json();
    body["backup_files"]
        .as_array()
        .unwrap_or_else(|| panic!("no backup_files in {body}"))
        .iter()
        .map(|f| f["filename"].as_str().unwrap().to_string())
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_manual_backup_lands_in_the_folder_and_can_be_deleted() {
    let app = TestApp::new().await;
    let backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();

    let filename = manual_backup(&app, &admin).await;
    assert_eq!(files(&app, &admin).await, [filename.clone()]);
    let path = backups.dir().join(&filename);
    assert!(std::fs::metadata(&path).unwrap().len() > 0);

    app.post("/api/data/delete_backup_file")
        .key(&admin)
        .json(&json!({ "backup_filename": filename }))
        .send()
        .await
        .assert_ok();
    assert_gone(&path);
    assert!(files(&app, &admin).await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn only_sql_files_in_the_folder_are_listed() {
    let app = TestApp::new().await;
    let backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    std::fs::write(backups.dir().join("keep.sql"), "-- dump").unwrap();
    std::fs::write(backups.dir().join("notes.txt"), "not a backup").unwrap();
    std::fs::write(backups.dir().join(".restore_upload_1.tmp"), "partial").unwrap();

    assert_eq!(files(&app, &admin).await, ["keep.sql"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn backup_file_names_cannot_leave_the_folder() {
    let app = TestApp::new().await;
    let backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    let outside = backups.dir().parent().unwrap().join(format!("outside-{}.sql", app.tag));
    std::fs::write(&outside, "-- precious").unwrap();
    let escape = format!("../{}", outside.file_name().unwrap().to_str().unwrap());

    for path in ["/api/data/delete_backup_file", "/api/data/restore_backup_file"] {
        for name in [escape.as_str(), "..\\x.sql", "notes.txt"] {
            app.post(path).key(&admin).json(&json!({ "backup_filename": name })).send().await.assert_status(400);
        }
        app.post(path).key(&admin).json(&json!({ "backup_filename": "missing.sql" })).send().await.assert_status(404);
    }
    assert!(outside.exists());
    std::fs::remove_file(outside).unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn restoring_a_backup_file_brings_the_data_back() {
    let app = TestApp::new().await;
    let _backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    let user = app.user("listener").await;
    let pod = app.subscribe(&user, &app.feed("show.xml", &Feed::with_episodes("Keeper", 2)), 2).await;

    let filename = manual_backup(&app, &admin).await;

    app.post("/api/data/remove_podcast_id")
        .key(&user)
        .json(&json!({ "podcast_id": pod, "user_id": user.id }))
        .send()
        .await
        .assert_ok();
    assert!(app.podcasts(&user).await.is_empty());

    let started = app.post("/api/data/restore_backup_file")
        .key(&admin)
        .json(&json!({ "backup_filename": filename }))
        .send()
        .await
        .ok_json();
    wait_for_task(&app, &admin, started["task_id"].as_str().unwrap()).await;
    wait_for_restore_to_end(&app).await;

    let podcasts = app.podcasts(&user).await;
    assert_eq!(podcasts.len(), 1, "{podcasts:?}");
    assert_eq!(app.episodes(&user, pod).await.len(), 2);
    app.login(&user.username, &user.password).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_downloaded_server_backup_can_be_uploaded_to_restore() {
    let app = TestApp::new().await;
    let backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    let user = app.user("listener").await;
    let pod = app.subscribe(&user, &app.feed("show.xml", &Feed::with_episodes("Uploaded", 2)), 2).await;

    let dump = app.post("/api/data/backup_server")
        .key(&admin)
        .json(&json!({ "database_pass": app.env.db_password }))
        .send()
        .await;
    dump.assert_ok();
    assert!(dump.text().contains("Uploaded"), "the dump holds the data");

    app.post("/api/data/remove_podcast_id")
        .key(&user)
        .json(&json!({ "podcast_id": pod, "user_id": user.id }))
        .send()
        .await
        .assert_ok();

    let started = app.post("/api/data/restore_server")
        .key(&admin)
        .raw_body(upload("server.sql", &dump.body), MULTIPART)
        .send()
        .await
        .ok_json();
    wait_for_task(&app, &admin, started["task_id"].as_str().unwrap()).await;
    wait_for_restore_to_end(&app).await;

    assert_eq!(app.podcasts(&user).await.len(), 1);
    assert_eq!(app.episodes(&user, pod).await.len(), 2);
    let leftovers: Vec<_> = std::fs::read_dir(backups.dir()).unwrap().flatten().collect();
    assert!(leftovers.is_empty(), "the upload is cleaned up: {leftovers:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_backup_from_before_shared_episodes_is_refused_and_changes_nothing() {
    let app = TestApp::new().await;
    let _backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    let user = app.user("listener").await;
    app.subscribe(&user, &app.feed("show.xml", &Feed::with_episodes("Show", 1)), 1).await;

    let started = app.post("/api/data/restore_server")
        .key(&admin)
        .raw_body(upload("old.sql", b"-- an old dump with no aliases table\nSELECT 1;\n"), MULTIPART)
        .send()
        .await
        .ok_json();
    let info = finished_task(&app, &admin, started["task_id"].as_str().unwrap()).await;
    assert_eq!(info["status"], "FAILED", "{info}");
    wait_for_restore_to_end(&app).await;

    assert_eq!(app.podcasts(&user).await.len(), 1, "the data is still there");
}

#[tokio::test(flavor = "multi_thread")]
async fn uploads_must_be_sql_files() {
    let app = TestApp::new().await;
    let _backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();

    app.post("/api/data/restore_server")
        .key(&admin)
        .raw_body(upload("backup.zip", b"PK"), MULTIPART)
        .send()
        .await
        .assert_status(400);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_restore_is_refused_while_one_runs() {
    let app = TestApp::new().await;
    let backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    std::fs::write(backups.dir().join("some.sql"), "-- dump").unwrap();
    app.state.restore_in_progress.store(true, std::sync::atomic::Ordering::SeqCst);

    app.post("/api/data/restore_backup_file")
        .key(&admin)
        .json(&json!({ "backup_filename": "some.sql" }))
        .send()
        .await
        .assert_status(409);
    let status = app.get("/api/data/restore_status").send().await.ok_json();
    assert_eq!(status["restore_in_progress"], true);
}

async fn schedule(app: &TestApp, admin: &TestUser, cron: &str, enabled: bool, keep: Option<i32>) -> u16 {
    app.post("/api/data/schedule_backup")
        .key(admin)
        .json(&json!({ "user_id": admin.id, "cron_schedule": cron, "enabled": enabled, "retention_count": keep }))
        .send()
        .await
        .status
        .as_u16()
}

/// Make the schedule overdue, as if its last run was long ago.
async fn make_due(app: &TestApp) {
    let column = match app.backend() {
        Backend::Postgres | Backend::Sqlite => "last_run",
        Backend::MariaDb => "LastRun",
    };
    exec(app, &format!(r#"UPDATE "ScheduledBackups" SET {column} = '2020-01-01 00:00:00'"#)).await;
}

fn scheduled_files(backups: &BackupEnv) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(backups.dir())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("scheduled_backup_"))
        .collect();
    names.sort();
    names
}

#[tokio::test(flavor = "multi_thread")]
async fn a_due_schedule_writes_a_backup_once() {
    let app = TestApp::new().await;
    let backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    assert_eq!(schedule(&app, &admin, "0 0 2 * * *", true, None).await, 200);
    make_due(&app).await;
    let state = Arc::new(app.state.clone());

    pinepods_api::services::scheduler::run_backup_job(state.clone()).await.unwrap();
    assert_eq!(scheduled_files(&backups).len(), 1);
    let path = backups.dir().join(&scheduled_files(&backups)[0]);
    assert!(std::fs::metadata(path).unwrap().len() > 0);

    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    pinepods_api::services::scheduler::run_backup_job(state).await.unwrap();
    assert_eq!(scheduled_files(&backups).len(), 1, "not due again until the next occurrence");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_disabled_schedule_does_nothing() {
    let app = TestApp::new().await;
    let backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    assert_eq!(schedule(&app, &admin, "0 0 2 * * *", false, None).await, 200);
    make_due(&app).await;

    pinepods_api::services::scheduler::run_backup_job(Arc::new(app.state.clone())).await.unwrap();
    assert!(scheduled_files(&backups).is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn retention_removes_old_scheduled_backups_but_not_manual_ones() {
    let app = TestApp::new().await;
    let backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    assert_eq!(schedule(&app, &admin, "0 0 2 * * *", true, Some(2)).await, 200);
    for n in 1..=3 {
        std::fs::write(backups.dir().join(format!("scheduled_backup_2020010{n}_000000.sql")), "-- old").unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    std::fs::write(backups.dir().join("manual_backup_old.sql"), "-- manual").unwrap();
    make_due(&app).await;

    pinepods_api::services::scheduler::run_backup_job(Arc::new(app.state.clone())).await.unwrap();

    let left = scheduled_files(&backups);
    assert_eq!(left.len(), 2, "{left:?}");
    assert!(left.contains(&"scheduled_backup_20200103_000000.sql".to_string()), "the newest old one stays: {left:?}");
    assert!(backups.dir().join("manual_backup_old.sql").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn schedules_can_be_read_back_and_bad_ones_are_refused() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();

    assert_eq!(schedule(&app, &admin, "not a cron", true, None).await, 400);
    assert_eq!(schedule(&app, &admin, "0 0 3 * * 1", true, Some(5)).await, 200);

    let body = app.post("/api/data/get_scheduled_backup")
        .key(&admin)
        .json(&json!({ "user_id": admin.id }))
        .send()
        .await
        .ok_json();
    assert_eq!(body["schedule"], "0 0 3 * * 1", "{body}");
    assert_eq!(body["enabled"], true, "{body}");
    assert_eq!(body["retention_count"], 5, "{body}");
}

/// Holds a lock that lets the app read `Users` but keeps a restore from clearing it, so a
/// restore stays running until this is released.
enum RestoreBlocker {
    Postgres(sqlx::Transaction<'static, sqlx::Postgres>),
    MariaDb(sqlx::pool::PoolConnection<sqlx::MySql>),
}

impl RestoreBlocker {
    async fn hold(app: &TestApp) -> RestoreBlocker {
        use pinepods_api::database::DatabasePool;
        use sqlx::Executor;
        match app.db() {
            DatabasePool::Postgres(pool) => {
                let mut tx = pool.begin().await.unwrap();
                tx.execute(r#"LOCK TABLE "Users" IN ACCESS SHARE MODE"#).await.unwrap();
                RestoreBlocker::Postgres(tx)
            }
            DatabasePool::MySQL(pool) => {
                let mut conn = pool.acquire().await.unwrap();
                conn.execute("LOCK TABLES Users READ").await.unwrap();
                RestoreBlocker::MariaDb(conn)
            }
            DatabasePool::Sqlite(_) => unreachable!("skipped on SQLite"),
        }
    }

    async fn release(self) {
        use sqlx::Executor;
        match self {
            RestoreBlocker::Postgres(tx) => tx.rollback().await.unwrap(),
            RestoreBlocker::MariaDb(mut conn) => {
                conn.execute("UNLOCK TABLES").await.unwrap();
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_running_restore_is_reported_until_it_finishes() {
    let app = TestApp::new().await;
    // Holding the restore open needs a table lock, which SQLite doesn't have.
    if app.backend() == Backend::Sqlite {
        return;
    }
    let _backups = BackupEnv::claim(&app).await;
    let admin = app.admin().await.clone();
    let filename = manual_backup(&app, &admin).await;

    let blocker = RestoreBlocker::hold(&app).await;
    let started = app.post("/api/data/restore_backup_file")
        .key(&admin)
        .json(&json!({ "backup_filename": filename }))
        .send()
        .await
        .ok_json();
    let task = started["task_id"].as_str().unwrap().to_string();
    // Give the restore time to start and reach the lock.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let status = app.get("/api/data/restore_status").send().await.ok_json();
    assert_eq!(status["restore_in_progress"], true, "the page overlay relies on this");

    // Not before this: anything using the database waits for the restore's locks.
    blocker.release().await;
    wait_for_task(&app, &admin, &task).await;
    wait_for_restore_to_end(&app).await;
}
