//! Server backup and restore on SQLite (PostgreSQL and MariaDB use pg_dump / mysqldump).

use crate::harness::*;

async fn fullname(app: &TestApp, id: i32) -> Option<String> {
    text(app, &format!(r#"SELECT fullname FROM "Users" WHERE userid = {id}"#)).await
}

#[tokio::test]
async fn sqlite_backup_restores_the_data() {
    let app = TestApp::new().await;
    if app.backend() != Backend::Sqlite {
        return;
    }
    let admin = app.admin().await;
    let kept = app.user("kept").await;
    let name = "Before; it''s -- not a comment";
    exec(&app, &format!(r#"UPDATE "Users" SET fullname = '{name}' WHERE userid = {}"#, kept.id)).await;

    let res = app.post("/api/data/backup_server").key(admin).json(&json!({ "database_pass": "" })).send().await;
    res.assert_ok();
    let dump = res.text();
    assert!(dump.starts_with("-- Pinepods SQLite backup\n"), "{}", &dump[..dump.len().min(200)]);
    let path = std::env::temp_dir().join(format!("pinepods-test-backup-{}-{}.sql", std::process::id(), kept.id));
    std::fs::write(&path, &dump).unwrap();

    exec(&app, &format!(r#"UPDATE "Users" SET fullname = 'After' WHERE userid = {}"#, kept.id)).await;
    let extra = app.user("extra").await;

    app.db().restore_server_data_from_path(&path).await.unwrap();
    assert_eq!(fullname(&app, kept.id).await.as_deref(), Some("Before; it's -- not a comment"));
    assert_eq!(fullname(&app, extra.id).await, None);
    // The restored admin key still works, and new rows still get ids.
    app.user("later").await;

    // A backup from a newer version is refused and changes nothing.
    std::fs::write(
        &path,
        format!("{dump}INSERT INTO schema_migrations (version, name, checksum) VALUES ('999', 'future', 'x');\n"),
    )
    .unwrap();
    exec(&app, &format!(r#"UPDATE "Users" SET fullname = 'Unchanged' WHERE userid = {}"#, kept.id)).await;
    let err = app.db().restore_server_data_from_path(&path).await.unwrap_err();
    assert!(format!("{err:?}").contains("newer"), "{err:?}");
    assert_eq!(fullname(&app, kept.id).await.as_deref(), Some("Unchanged"));

    // So is a PostgreSQL dump.
    std::fs::write(&path, "--\n-- PostgreSQL database dump\n--\n\"EpisodeIDAliases\"\n").unwrap();
    let err = app.db().restore_server_data_from_path(&path).await.unwrap_err();
    assert!(format!("{err:?}").contains("PostgreSQL or MySQL"), "{err:?}");

    let _ = std::fs::remove_file(&path);
}
