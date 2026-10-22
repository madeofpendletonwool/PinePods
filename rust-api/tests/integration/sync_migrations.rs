//! Sync data model migrations: old-shape data is seeded, the
//! migrations are forgotten and re-run through the real runner, and the result is checked.
//! 068 moves Users.Pod_Sync_Type to GpodderServerEnabled / SyncTargets / app passwords; 111/112
//! extend the action and subscription logs; 113 marks Pinepods' own device; 069 makes listen
//! history one row per user and episode on MySQL.

use pinepods_api::services::auth::verify_password;

use crate::harness::*;

/// `pg` on Postgres, `my` on MariaDB (for DDL that differs beyond quoting).
async fn exec_per_backend(app: &TestApp, pg: &str, my: &str) {
    match app.backend() {
        Backend::Postgres => exec(app, pg).await,
        Backend::MariaDb => exec(app, my).await,
    }
}

async fn set_sync(app: &TestApp, user: &TestUser, kind: &str, url: &str, login: &str, token: &str, device: Option<&str>) {
    let device = device.map(|d| format!("'{d}'")).unwrap_or_else(|| "NULL".into());
    exec(
        app,
        &format!(
            r#"UPDATE "Users" SET Pod_Sync_Type = '{kind}', GpodderUrl = '{url}', GpodderLoginName = '{login}',
               GpodderToken = '{token}', DefaultGpodderDevice = {device} WHERE UserID = {}"#,
            user.id
        ),
    )
    .await;
}

async fn legacy_hashes(app: &TestApp, user: &TestUser) -> Vec<String> {
    let mut out = Vec::new();
    let mut last = 0;
    while let Some(id) = int(app, &format!(
        r#"SELECT MIN(AppPasswordID) FROM "GpodderAppPasswords" WHERE UserID = {} AND Label = 'legacy token' AND AppPasswordID > {last}"#,
        user.id
    ))
    .await
    {
        out.push(text(app, &format!(r#"SELECT PasswordHash FROM "GpodderAppPasswords" WHERE AppPasswordID = {id}"#)).await.unwrap());
        last = id;
    }
    out
}

#[tokio::test]
async fn pod_sync_type_settings_move_to_the_new_sync_model() {
    let app = TestApp::new().await;
    let internal = app.user("internal").await;
    let both = app.user("both").await;
    let external = app.user("external").await;
    let nextcloud = app.user("nextcloud").await;
    let broken = app.user("broken").await;
    let unsynced = app.user("unsynced").await;

    let internal_token = "a".repeat(40) + "internal-token-123456789012345";
    let both_token = "b".repeat(40) + "internal-token-987654321098765";
    let ext_secret = app.db().encrypt_password("ext-secret").await.unwrap();
    let nc_secret = app.db().encrypt_password("nc-app-password").await.unwrap();
    set_sync(&app, &internal, "gpodder", "http://localhost:8042", &internal.username, &internal_token, Some("pinepods-internal-1")).await;
    set_sync(&app, &both, "both", "http://localhost:8042", &both.username, &both_token, None).await;
    set_sync(&app, &external, "external", "https://gpodder.example", "ext-user", &ext_secret, Some("pinepods-ext")).await;
    set_sync(&app, &nextcloud, "nextcloud", "https://cloud.example", "nc-user", &nc_secret, Some("ignored")).await;
    set_sync(&app, &broken, "external", "https://gpodder.example", "ext-user", "not-a-fernet-token", None).await;

    rerun_migrations(&app, &["068"]).await;

    let enabled = |u: &TestUser| {
        let sql = format!(r#"SELECT GpodderServerEnabled FROM "Users" WHERE UserID = {}"#, u.id);
        let app = &app;
        async move { int(app, &sql).await == Some(1) }
    };
    let notice = |u: &TestUser| {
        let sql = format!(r#"SELECT SyncNotice FROM "Users" WHERE UserID = {}"#, u.id);
        let app = &app;
        async move { text(app, &sql).await }
    };
    let targets = |u: &TestUser| {
        let sql = format!(r#"SELECT COUNT(*) FROM "SyncTargets" WHERE UserID = {}"#, u.id);
        let app = &app;
        async move { int(app, &sql).await.unwrap() }
    };

    // gpodder: internal server on, the old token is now an app password that still verifies.
    assert!(enabled(&internal).await);
    let hashes = legacy_hashes(&app, &internal).await;
    assert_eq!(hashes.len(), 1);
    assert!(hashes[0].starts_with("$argon2"), "stored hashed, not as the token");
    assert!(verify_password(&internal_token, &hashes[0]).unwrap(), "legacy token verifies as an app password");
    assert!(!verify_password(&both_token, &hashes[0]).unwrap());
    assert_eq!(notice(&internal).await, None);
    assert_eq!(targets(&internal).await, 0);

    // both: internal on with its token kept, external creds were already lost → banner flag.
    assert!(enabled(&both).await);
    let hashes = legacy_hashes(&app, &both).await;
    assert_eq!(hashes.len(), 1);
    assert!(verify_password(&both_token, &hashes[0]).unwrap());
    assert_eq!(notice(&both).await.as_deref(), Some("reenter_external"));
    assert_eq!(targets(&both).await, 0);

    // external: a gpodder target with the same encrypted secret and device.
    assert!(!enabled(&external).await);
    assert!(legacy_hashes(&app, &external).await.is_empty());
    assert_eq!(notice(&external).await, None);
    let uid = external.id;
    let col = |c: &str| format!(r#"SELECT {c} FROM "SyncTargets" WHERE UserID = {uid}"#);
    assert_eq!(text(&app, &col("Kind")).await.as_deref(), Some("gpodder"));
    assert_eq!(text(&app, &col("Url")).await.as_deref(), Some("https://gpodder.example"));
    assert_eq!(text(&app, &col("Username")).await.as_deref(), Some("ext-user"));
    assert_eq!(text(&app, &col("DeviceName")).await.as_deref(), Some("pinepods-ext"));
    let secret = text(&app, &col("EncryptedSecret")).await.unwrap();
    assert_eq!(app.db().decrypt_password(&secret).await.unwrap(), "ext-secret");
    assert_eq!(int(&app, &col("Enabled")).await, Some(1));
    assert_eq!(int(&app, &col("NeedsFullSync")).await, Some(1));
    for cursor in ["SubsPullCursor", "ActionsPullCursor", "SubsPushCursor", "ActionsPushCursor", "ConsecutiveFailures"] {
        assert_eq!(int(&app, &col(cursor)).await, Some(0), "{cursor}");
    }

    // nextcloud: a nextcloud target, no device name.
    let uid = nextcloud.id;
    let col = |c: &str| format!(r#"SELECT {c} FROM "SyncTargets" WHERE UserID = {uid}"#);
    assert_eq!(text(&app, &col("Kind")).await.as_deref(), Some("nextcloud"));
    assert_eq!(text(&app, &col("Username")).await.as_deref(), Some("nc-user"));
    assert_eq!(text(&app, &col("DeviceName")).await, None);
    let secret = text(&app, &col("EncryptedSecret")).await.unwrap();
    assert_eq!(app.db().decrypt_password(&secret).await.unwrap(), "nc-app-password");
    assert!(!enabled(&nextcloud).await);

    // A secret that doesn't decrypt isn't carried over; the user is asked to re-enter it.
    assert_eq!(targets(&broken).await, 0);
    assert_eq!(notice(&broken).await.as_deref(), Some("reenter_external"));

    // No sync configured: untouched.
    assert!(!enabled(&unsynced).await);
    assert_eq!(targets(&unsynced).await, 0);
    assert_eq!(notice(&unsynced).await, None);

    // The old columns stay populated for the old sync code until that code is removed.
    assert_eq!(
        text(&app, &format!(r#"SELECT GpodderToken FROM "Users" WHERE UserID = {}"#, internal.id)).await.as_deref(),
        Some(internal_token.as_str())
    );
    assert_eq!(
        text(&app, &format!(r#"SELECT Pod_Sync_Type FROM "Users" WHERE UserID = {}"#, external.id)).await.as_deref(),
        Some("external")
    );

    // Running it again changes nothing.
    rerun_migrations(&app, &["068"]).await;
    assert_eq!(legacy_hashes(&app, &internal).await.len(), 1);
    assert_eq!(targets(&external).await, 1);
    assert_eq!(targets(&nextcloud).await, 1);
}

#[tokio::test]
async fn sync_logs_survive_device_removal_and_device_lists_are_backfilled() {
    let app = TestApp::new().await;
    let user = app.user("logs").await;
    let uid = user.id;
    let feed = Feed::with_episodes("Logged", 2);
    let feed_url = app.feed("logged.xml", &feed);
    let pod = app.subscribe(&user, &feed_url, 2).await;
    let e1 = app.episode_id(&user, pod, "Episode 1").await;
    let e1_url = text(&app, &format!(r#"SELECT EpisodeURL FROM "Episodes" WHERE EpisodeID = {e1}"#)).await.unwrap();

    // Recreate the pre-111/112 shape: device FKs ON DELETE CASCADE (under names the migration
    // has to look up), no EpisodeID FK, subscription DeviceID NOT NULL.
    exec_per_backend(
        &app,
        r#"ALTER TABLE "GpodderSyncEpisodeActions" DROP CONSTRAINT fk_gpodder_episode_actions_device,
           DROP CONSTRAINT fk_gpodder_episode_actions_episode,
           ADD CONSTRAINT legacy_actions_device FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE"#,
        "ALTER TABLE GpodderSyncEpisodeActions DROP FOREIGN KEY fk_gpodder_episode_actions_device,
           DROP FOREIGN KEY fk_gpodder_episode_actions_episode",
    )
    .await;
    exec_per_backend(
        &app,
        r#"ALTER TABLE "GpodderSyncSubscriptions" DROP CONSTRAINT fk_gpodder_subscriptions_device,
           ALTER COLUMN DeviceID SET NOT NULL,
           ALTER COLUMN ServerTs DROP NOT NULL,
           ADD CONSTRAINT legacy_subs_device FOREIGN KEY (DeviceID) REFERENCES "GpodderDevices"(DeviceID) ON DELETE CASCADE"#,
        "ALTER TABLE GpodderSyncSubscriptions DROP FOREIGN KEY fk_gpodder_subscriptions_device",
    )
    .await;
    if app.backend() == Backend::MariaDb {
        exec(&app, "ALTER TABLE GpodderSyncEpisodeActions ADD CONSTRAINT legacy_actions_device FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE").await;
        exec(&app, "ALTER TABLE GpodderSyncSubscriptions MODIFY COLUMN DeviceID INT NOT NULL, MODIFY COLUMN ServerTs BIGINT NULL").await;
        exec(&app, "ALTER TABLE GpodderSyncSubscriptions ADD CONSTRAINT legacy_subs_device FOREIGN KEY (DeviceID) REFERENCES GpodderDevices(DeviceID) ON DELETE CASCADE").await;
    }

    for name in ["phone", "tablet"] {
        exec(&app, &format!(r#"INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType) VALUES ({uid}, '{name}', 'mobile')"#)).await;
    }
    let device = |name: &str| {
        let sql = format!(r#"SELECT DeviceID FROM "GpodderDevices" WHERE UserID = {uid} AND DeviceName = '{name}'"#);
        let app = &app;
        async move { int(app, &sql).await.unwrap() }
    };
    let (phone, tablet) = (device("phone").await, device("tablet").await);

    for (url, ts) in [(e1_url.as_str(), 1000), ("https://cdn.example/unknown.mp3", 1001)] {
        exec(&app, &format!(
            r#"INSERT INTO "GpodderSyncEpisodeActions" (UserID, DeviceID, PodcastURL, EpisodeURL, Action, Timestamp, ServerTs, Position)
               VALUES ({uid}, {phone}, '{feed_url}', '{url}', 'play', {ts}, {ts}, 30)"#
        ))
        .await;
    }
    // phone: A added then removed, B added, C added and removed in the same second → [B].
    // tablet: A added → [A].
    for (dev, url, action, ts) in [
        (phone, "https://a.example/feed", "add", 100),
        (phone, "https://b.example/feed", "add", 100),
        (phone, "https://a.example/feed", "remove", 200),
        (phone, "https://c.example/feed", "add", 300),
        (phone, "https://c.example/feed", "remove", 300),
        (tablet, "https://a.example/feed", "add", 150),
    ] {
        exec(&app, &format!(
            r#"INSERT INTO "GpodderSyncSubscriptions" (UserID, DeviceID, PodcastURL, Action, Timestamp) VALUES ({uid}, {dev}, '{url}', '{action}', {ts})"#
        ))
        .await;
    }
    exec(&app, r#"UPDATE "GpodderSyncSubscriptions" SET ServerTs = NULL"#).await;

    rerun_migrations(&app, &["111", "112", "114"]).await;

    // 111: actions resolved by exact URL within the user's podcasts; unknown stays unmatched.
    assert_eq!(int(&app, &format!(r#"SELECT EpisodeID FROM "GpodderSyncEpisodeActions" WHERE EpisodeURL = '{e1_url}'"#)).await, Some(e1 as i64));
    assert_eq!(int(&app, r#"SELECT EpisodeID FROM "GpodderSyncEpisodeActions" WHERE EpisodeURL = 'https://cdn.example/unknown.mp3'"#).await, None);
    assert_eq!(text(&app, &format!(r#"SELECT Origin FROM "GpodderSyncEpisodeActions" WHERE UserID = {uid} AND Timestamp = 1000"#)).await.as_deref(), Some("client"));
    // 112: ServerTs backfilled from Timestamp.
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncSubscriptions" WHERE UserID = {uid} AND (ServerTs IS NULL OR ServerTs <> Timestamp)"#)).await, Some(0));

    // Device lists replayed from the log.
    let list = |dev: i64| {
        let app = &app;
        async move {
            let n = int(app, &format!(r#"SELECT COUNT(*) FROM "GpodderDeviceSubscriptions" WHERE DeviceID = {dev}"#)).await.unwrap();
            let url = text(app, &format!(r#"SELECT MIN(PodcastURL) FROM "GpodderDeviceSubscriptions" WHERE DeviceID = {dev}"#)).await;
            (n, url)
        }
    };
    assert_eq!(list(phone).await, (1, Some("https://b.example/feed".to_string())));
    assert_eq!(list(tablet).await, (1, Some("https://a.example/feed".to_string())));

    // Removing a device keeps its log rows (DeviceID → NULL); its list goes with it.
    exec(&app, &format!(r#"DELETE FROM "GpodderDevices" WHERE DeviceID = {phone}"#)).await;
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE UserID = {uid} AND DeviceID IS NULL"#)).await, Some(2));
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncSubscriptions" WHERE UserID = {uid} AND DeviceID IS NULL"#)).await, Some(5));
    assert_eq!(list(phone).await.0, 0);
    assert_eq!(list(tablet).await.0, 1);

    // Removing the podcast leaves its actions in the log, unmatched.
    app.post("/api/data/remove_podcast_id")
        .key(&user)
        .json(&json!({ "user_id": uid, "podcast_id": pod }))
        .send()
        .await
        .assert_ok();
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE UserID = {uid}"#)).await, Some(2));
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderSyncEpisodeActions" WHERE UserID = {uid} AND EpisodeID IS NOT NULL"#)).await, Some(0));
}

#[tokio::test]
async fn the_internal_sync_device_is_marked_as_the_pinepods_device() {
    let app = TestApp::new().await;
    let user = app.user("device").await;
    let uid = user.id;
    for name in [format!("pinepods-internal-{uid}"), "pinepods".to_string(), "antennapod".to_string()] {
        exec(&app, &format!(r#"INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType) VALUES ({uid}, '{name}', 'server')"#)).await;
    }
    rerun_migrations(&app, &["113"]).await;

    assert_eq!(
        text(&app, &format!(r#"SELECT DeviceName FROM "GpodderDevices" WHERE UserID = {uid} AND SystemRole = 'pinepods'"#)).await,
        Some(format!("pinepods-internal-{uid}"))
    );
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderDevices" WHERE UserID = {uid} AND SystemRole IS NOT NULL"#)).await, Some(1));
    // One Pinepods device per user.
    let second = format!(r#"UPDATE "GpodderDevices" SET SystemRole = 'pinepods' WHERE UserID = {uid} AND DeviceName = 'pinepods'"#);
    let rejected = match app.db() {
        pinepods_api::database::DatabasePool::Postgres(pool) => sqlx::raw_sql(sqlx::AssertSqlSafe(second)).execute(pool).await.is_err(),
        pinepods_api::database::DatabasePool::MySQL(pool) => sqlx::raw_sql(sqlx::AssertSqlSafe(second.replace('"', ""))).execute(pool).await.is_err(),
    };
    assert!(rejected, "a second pinepods device must be rejected");
}

#[tokio::test]
async fn expired_gpodder_sessions_are_cleaned_up() {
    let app = TestApp::new().await;
    let user = app.user("sessions").await;
    let uid = user.id;
    for (token, expires) in [("old", "2020-01-01 00:00:00"), ("live", "2099-01-01 00:00:00")] {
        exec(&app, &format!(
            r#"INSERT INTO "GpodderSessions" (UserID, SessionToken, ExpiresAt) VALUES ({uid}, '{token}-{uid}', '{expires}')"#
        ))
        .await;
    }
    assert!(app.db().cleanup_expired_gpodder_sessions().await.unwrap() >= 1);
    assert_eq!(int(&app, &format!(r#"SELECT COUNT(*) FROM "GpodderSessions" WHERE UserID = {uid}"#)).await, Some(1));
    assert_eq!(
        text(&app, &format!(r#"SELECT SessionToken FROM "GpodderSessions" WHERE UserID = {uid}"#)).await,
        Some(format!("live-{uid}"))
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn listen_history_duplicates_are_merged_and_made_unique() {
    let app = TestApp::new().await;
    let user = app.user("dupes").await;
    let url = app.feed("dupes.xml", &Feed::with_episodes("Dupes", 2));
    let pod = app.subscribe(&user, &url, 2).await;
    let e1 = app.episode_id(&user, pod, "Episode 1").await;
    let e2 = app.episode_id(&user, pod, "Episode 2").await;

    // The pre-069 MySQL shape: no unique key, so upserts inserted duplicates.
    exec_per_backend(
        &app,
        r#"ALTER TABLE "UserEpisodeHistory" DROP CONSTRAINT user_episode_unique"#,
        "ALTER TABLE UserEpisodeHistory DROP INDEX user_episode_unique",
    )
    .await;
    let uid = user.id;
    exec(
        &app,
        &format!(
            r#"INSERT INTO "UserEpisodeHistory" (UserID, EpisodeID, ListenDate, ListenDuration) VALUES
               ({uid}, {e1}, '2026-01-01 00:00:00', 100),
               ({uid}, {e1}, '2026-03-01 00:00:00', 300),
               ({uid}, {e1}, '2026-03-01 00:00:00', 250),
               ({uid}, {e1}, NULL, 900),
               ({uid}, {e2}, '2026-02-01 00:00:00', 42)"#
        ),
    )
    .await;

    rerun_migrations(&app, &["069"]).await;

    let rows = |ep: i32| {
        let sql = format!(r#"SELECT COUNT(*) FROM "UserEpisodeHistory" WHERE UserID = {uid} AND EpisodeID = {ep}"#);
        let app = &app;
        async move { int(app, &sql).await }
    };
    assert_eq!(rows(e1).await, Some(1));
    assert_eq!(rows(e2).await, Some(1));
    // The most recent row survives; on a tie, the one written last.
    let kept = int(&app, &format!(r#"SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = {uid} AND EpisodeID = {e1}"#)).await;
    assert_eq!(kept, Some(250));

    // Upserts now update in place on both backends.
    app.post("/api/data/mark_episode_completed")
        .key(&user)
        .json(&json!({ "episode_id": e2, "user_id": uid, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
    assert_eq!(rows(e2).await, Some(1));
    assert_eq!(
        int(&app, &format!(r#"SELECT ListenDuration FROM "UserEpisodeHistory" WHERE UserID = {uid} AND EpisodeID = {e2}"#)).await,
        Some(1802)
    );
}

/// Rows the Go server wrote after 112 have no ServerTs: 114 backfills it from Timestamp (Go's
/// receive time) and makes the column required.
#[tokio::test(flavor = "multi_thread")]
async fn subscription_server_ts_is_backfilled_and_required() {
    let app = TestApp::new().await;
    let user = app.user("serverts").await;
    let uid = user.id;
    exec_per_backend(
        &app,
        r#"ALTER TABLE "GpodderSyncSubscriptions" ALTER COLUMN ServerTs DROP NOT NULL"#,
        "ALTER TABLE GpodderSyncSubscriptions MODIFY COLUMN ServerTs BIGINT NULL",
    )
    .await;
    exec(&app, &format!(r#"INSERT INTO "GpodderDevices" (UserID, DeviceName, DeviceType) VALUES ({uid}, 'phone', 'mobile')"#)).await;
    let phone = int(&app, &format!(r#"SELECT DeviceID FROM "GpodderDevices" WHERE UserID = {uid}"#)).await.unwrap();
    exec(&app, &format!(
        r#"INSERT INTO "GpodderSyncSubscriptions" (UserID, DeviceID, PodcastURL, Action, Timestamp, ServerTs)
           VALUES ({uid}, {phone}, 'https://go.example/feed', 'add', 1700000000, NULL),
                  ({uid}, {phone}, 'https://rust.example/feed', 'add', 1600000000, 1700000500)"#
    ))
    .await;

    rerun_migrations(&app, &["114"]).await;

    let ts = |url: &str| {
        let sql = format!(r#"SELECT ServerTs FROM "GpodderSyncSubscriptions" WHERE PodcastURL = '{url}'"#);
        let app = &app;
        async move { int(app, &sql).await }
    };
    assert_eq!(ts("https://go.example/feed").await, Some(1_700_000_000), "NULL ServerTs is backfilled from Timestamp");
    assert_eq!(ts("https://rust.example/feed").await, Some(1_700_000_500), "an existing ServerTs is kept");
    let nullable = match app.backend() {
        Backend::Postgres => int(&app, "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'GpodderSyncSubscriptions' AND column_name = 'serverts' AND is_nullable = 'YES'").await,
        Backend::MariaDb => int(&app, "SELECT COUNT(*) FROM INFORMATION_SCHEMA.COLUMNS WHERE TABLE_SCHEMA = DATABASE() AND TABLE_NAME = 'GpodderSyncSubscriptions' AND COLUMN_NAME = 'ServerTs' AND IS_NULLABLE = 'YES'").await,
    };
    assert_eq!(nullable, Some(0), "ServerTs is NOT NULL after 114");
}
