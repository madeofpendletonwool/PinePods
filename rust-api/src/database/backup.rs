//! Backup, restore and scheduled backups.

use super::*;

// A single enabled scheduled-backup row, used by the background scheduler to decide
// when a backup is due. retention_count: None/0 = keep all backups.
#[derive(Debug, Clone)]
pub struct ScheduledBackupRow {
    pub user_id: i32,
    pub cron_schedule: String,
    pub retention_count: Option<i32>,
    pub last_run: Option<DateTime<Utc>>,
}

// A data dump lists every table, empty or not; EpisodeIDAliases only exists once episodes are
// shared between users.
pub(super) async fn dump_has_shared_episodes(path: &std::path::Path) -> AppResult<bool> {
    use tokio::io::AsyncBufReadExt;
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| AppError::internal(&format!("Failed to open backup file: {}", e)))?;
    let mut lines = tokio::io::BufReader::new(file).lines();
    while let Some(line) = lines.next_line().await.map_err(|e| AppError::internal(&format!("Failed to read backup file: {}", e)))? {
        if line.contains(r#""EpisodeIDAliases""#) || line.contains("`EpisodeIDAliases`") {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Strip the trailing `\r`/`\n` from a raw line read with `read_until(b'\n')`.
pub(super) fn trim_line_eol(line: &[u8]) -> &[u8] {
    let mut end = line.len();
    while end > 0 && (line[end - 1] == b'\n' || line[end - 1] == b'\r') {
        end -= 1;
    }
    &line[..end]
}

/// True for the lone `\.` line that terminates a Postgres `COPY ... FROM stdin` data block.
pub(super) fn is_pg_copy_terminator(line: &[u8]) -> bool {
    trim_line_eol(line) == b"\\."
}

/// Stream a Postgres plain-text dump from `reader` to `writer`, dropping the `COPY` data
/// blocks for any table in `skip_tables`. Everything else is passed through byte-for-byte.
///
/// Works line-by-line (bounded memory) with a small state machine so that data rows which
/// happen to start with the text `COPY` are never mistaken for statement-level COPY headers:
/// once inside a COPY block we only watch for its `\.` terminator. Table names are matched as
/// the exact double-quoted identifier pg_dump emits (`"Sessions"`), so `"Sessions"` does not
/// match inside `"GpodderSessions"`.
pub(super) async fn stream_pg_dump_skipping_tables<R, W>(
    reader: R,
    writer: &mut W,
    skip_tables: &[&str],
) -> std::io::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    enum CopyState {
        Normal,
        Pass,
        Skip,
    }

    let needles: Vec<String> = skip_tables.iter().map(|t| format!("\"{}\"", t)).collect();
    let mut buf_reader = BufReader::new(reader);
    let mut state = CopyState::Normal;
    let mut line: Vec<u8> = Vec::new();

    loop {
        line.clear();
        if buf_reader.read_until(b'\n', &mut line).await? == 0 {
            break;
        }
        match state {
            CopyState::Normal => {
                let mut drop_line = false;
                if line.starts_with(b"COPY ") {
                    let header = String::from_utf8_lossy(&line);
                    if header.contains(" FROM stdin;") {
                        if needles.iter().any(|n| header.contains(n.as_str())) {
                            state = CopyState::Skip;
                            drop_line = true; // drop the COPY header for skipped tables
                        } else {
                            state = CopyState::Pass;
                        }
                    }
                }
                if !drop_line {
                    writer.write_all(&line).await?;
                }
            }
            CopyState::Pass => {
                writer.write_all(&line).await?;
                if is_pg_copy_terminator(&line) {
                    state = CopyState::Normal;
                }
            }
            CopyState::Skip => {
                if is_pg_copy_terminator(&line) {
                    state = CopyState::Normal;
                }
                // drop every line inside a skipped COPY block, including the terminator
            }
        }
    }
    Ok(())
}

/// Stream a mysqldump file from `reader` to `writer`, dropping the data-loading statements
/// (`INSERT INTO` / `LOCK TABLES`) for any table in `skip_tables`. mysqldump quotes table
/// names with backticks (`` `Sessions` ``), and `--complete-insert` emits one statement per
/// line, so a prefix match per line is sufficient. Secondary to the Postgres path.
pub(super) async fn stream_mysql_dump_skipping_tables<R, W>(
    reader: R,
    writer: &mut W,
    skip_tables: &[&str],
) -> std::io::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let prefixes: Vec<String> = skip_tables
        .iter()
        .flat_map(|t| {
            vec![
                format!("INSERT INTO `{}`", t),
                format!("LOCK TABLES `{}`", t),
            ]
        })
        .collect();

    let mut buf_reader = BufReader::new(reader);
    let mut line: Vec<u8> = Vec::new();

    loop {
        line.clear();
        if buf_reader.read_until(b'\n', &mut line).await? == 0 {
            break;
        }
        let trimmed = trim_line_eol(&line);
        // Only the INSERT/LOCK statements start with these prefixes, so checking the prefix is
        // cheap and avoids inspecting (and allocating for) every data line.
        let drop_line = (trimmed.starts_with(b"INSERT INTO `") || trimmed.starts_with(b"LOCK TABLES `"))
            && {
                let text = String::from_utf8_lossy(trimmed);
                prefixes.iter().any(|p| text.starts_with(p.as_str()))
            };
        if !drop_line {
            writer.write_all(&line).await?;
        }
    }
    Ok(())
}

impl DatabasePool {
    /// Write a SQLite server backup to `path`; `None` when this isn't a SQLite database
    /// (the others are backed up with pg_dump / mysqldump).
    pub async fn sqlite_backup_to(&self, path: &std::path::Path) -> Option<AppResult<()>> {
        match self {
            DatabasePool::Sqlite(pool) => Some(super::sqlite_backup::dump(pool, path).await),
            _ => None,
        }
    }

    // Backup user data - matches Python backup_user function exactly
    pub async fn backup_user(&self, user_id: i32) -> AppResult<String> {
        let podcasts = dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT podcastname, feedurl FROM "Podcasts" WHERE userid = $1 AND (username IS NULL OR username = '') AND (password IS NULL OR password = '')"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut podcasts = Vec::new();
                for row in rows {
                    podcasts.push((
                        row.try_get::<String, _>("podcastname")?,
                        row.try_get::<String, _>("feedurl")?,
                    ));
                }
                podcasts
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT PodcastName, FeedURL FROM Podcasts WHERE UserID = ? AND (Username IS NULL OR Username = '') AND (Password IS NULL OR Password = '')"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;

                let mut podcasts = Vec::new();
                for row in rows {
                    podcasts.push((
                        row.try_get::<String, _>("PodcastName")?,
                        row.try_get::<String, _>("FeedURL")?,
                    ));
                }
                podcasts
            },
        });

        // Generate OPML content
        let mut opml_content = String::from(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<opml version="2.0">
  <head>
    <title>Podcast Subscriptions</title>
  </head>
  <body>
"#
        );

        for (podcast_name, feed_url) in podcasts {
            // Escape XML characters in podcast name and feed URL
            let escaped_name = podcast_name
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&#39;");
            
            let escaped_url = feed_url
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;");

            opml_content.push_str(&format!(
                r#"    <outline text="{}" title="{}" type="rss" xmlUrl="{}" />
"#,
                escaped_name, escaped_name, escaped_url
            ));
        }

        opml_content.push_str("  </body>\n</opml>");
        Ok(opml_content)
    }

    // Add more database operations as needed...

    // Restore server data from an on-disk SQL dump, streaming the file into the
    // psql/mysql client so memory usage stays bounded regardless of dump size.
    //
    // Postgres restores are fully atomic: the truncate + data load run inside a single
    // psql transaction (--single-transaction + ON_ERROR_STOP), so a failure rolls back
    // and leaves the pre-restore data untouched. Crucially, MVCC means concurrent app
    // connections keep seeing the old committed data until COMMIT, so the database never
    // appears empty mid-restore (which previously triggered the "create first user" flow
    // and a write race that corrupted the restore).
    pub async fn restore_server_data_from_path(&self, path: &std::path::Path) -> AppResult<()> {
        // A SQLite backup only restores on SQLite, and SQLite only restores its own backups.
        let sqlite_file = super::sqlite_backup::is_sqlite_backup(path).await?;
        if let DatabasePool::Sqlite(pool) = self {
            if !sqlite_file {
                return Err(AppError::bad_request(
                    "This backup came from a PostgreSQL or MySQL server. A SQLite server can only restore its own backups.",
                ));
            }
            return super::sqlite_backup::restore(pool, path).await;
        }
        if sqlite_file {
            return Err(AppError::bad_request("This is a SQLite backup and can only be restored on a SQLite server."));
        }

        // Transient auth tables that hold only disposable login tokens. They are never worth
        // restoring, and historically their schema has drifted (e.g. an upgraded instance can
        // still carry the legacy `Sessions(sessionid, userid, value, expire)` layout while a
        // fresh install has `Sessions(SessionID, UserID, SessionToken, ...)`). Restoring their
        // rows can therefore fail the column match and, under --single-transaction +
        // ON_ERROR_STOP, roll back the entire restore. We strip these tables' data from the
        // incoming dump so the rest of the (compatible) data still loads. GpodderSyncState is
        // deliberately NOT included here — it holds real per-device sync watermarks.
        const SKIP_TABLES: &[&str] = &["Sessions", "GpodderSessions"];

        use tokio::process::Command;
        use tokio::io::AsyncWriteExt;

        // Episodes are shared between users now; a backup from before that holds one copy of
        // each episode per user, which this schema can't take.
        if !dump_has_shared_episodes(path).await? {
            return Err(AppError::bad_request(
                "This backup is from a Pinepods version before episodes were shared between users and can't be restored here",
            ));
        }

        dialect_match!(self, {
            pg(_) => {
                let host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
                let port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
                let database = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods".to_string());
                let username = std::env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
                let password = std::env::var("DB_PASSWORD").unwrap_or_else(|_| "password".to_string());

                // Feed the whole restore through psql stdin as one transaction:
                //   1. disable FK/triggers for the load,
                //   2. truncate every table in the public schema,
                //   3. stream the data-only dump,
                //   4. re-enable triggers.
                // --single-transaction wraps it in BEGIN/COMMIT; ON_ERROR_STOP makes it
                // all-or-nothing (otherwise psql skips failing statements, exits 0, and
                // silently drops rows). The dump is streamed, so memory stays bounded.
                let mut cmd = Command::new("psql");
                cmd.arg("--host").arg(&host)
                   .arg("--port").arg(&port)
                   .arg("--username").arg(&username)
                   .arg("--no-password")
                   .arg("--dbname").arg(&database)
                   .arg("--quiet")
                   .arg("--single-transaction")
                   .arg("-v").arg("ON_ERROR_STOP=1")
                   .env("PGPASSWORD", &password)
                   .stdin(std::process::Stdio::piped())
                   .stdout(std::process::Stdio::piped())
                   .stderr(std::process::Stdio::piped());

                let mut child = cmd.spawn()
                    .map_err(|e| AppError::internal(&format!("Failed to start psql: {}", e)))?;

                const PREAMBLE: &str = "SET session_replication_role = replica;\n\
                    DO $$ DECLARE r RECORD; BEGIN \
                    FOR r IN SELECT tablename FROM pg_tables WHERE schemaname = 'public' LOOP \
                    EXECUTE format('TRUNCATE TABLE %I RESTART IDENTITY CASCADE', r.tablename); \
                    END LOOP; END $$;\n";
                const POSTAMBLE: &str = "\nSET session_replication_role = DEFAULT;\n";

                // psql writes to stdout/stderr (sequence setval rows, NOTICE/COPY messages)
                // as it consumes stdin. If we wrote the whole dump and only drained those pipes
                // afterward, a large restore would deadlock: psql blocks once its ~64KB output
                // pipe fills, stops reading stdin, and our writer blocks forever. So drain
                // stdin/stdout/stderr concurrently and only then reap the child.
                let mut stdin = child.stdin.take()
                    .ok_or_else(|| AppError::internal("psql stdin was not piped"))?;
                let mut stdout = child.stdout.take()
                    .ok_or_else(|| AppError::internal("psql stdout was not piped"))?;
                let mut stderr_pipe = child.stderr.take()
                    .ok_or_else(|| AppError::internal("psql stderr was not piped"))?;

                let path_buf = path.to_path_buf();
                let writer = async move {
                    stdin.write_all(PREAMBLE.as_bytes()).await
                        .map_err(|e| AppError::internal(&format!("Failed to write restore preamble: {}", e)))?;
                    let file = tokio::fs::File::open(&path_buf).await
                        .map_err(|e| AppError::internal(&format!("Failed to open backup file: {}", e)))?;
                    // Stream the dump through, dropping COPY blocks for the transient session
                    // tables so a legacy/mismatched `Sessions` schema can't abort the restore.
                    stream_pg_dump_skipping_tables(file, &mut stdin, SKIP_TABLES).await
                        .map_err(|e| AppError::internal(&format!("Failed to stream SQL to psql: {}", e)))?;
                    stdin.write_all(POSTAMBLE.as_bytes()).await
                        .map_err(|e| AppError::internal(&format!("Failed to write restore postamble: {}", e)))?;
                    stdin.shutdown().await
                        .map_err(|e| AppError::internal(&format!("Failed to close psql stdin: {}", e)))?;
                    Ok::<(), AppError>(())
                };

                use tokio::io::AsyncReadExt;
                let read_out = async move {
                    let mut buf = Vec::new();
                    let _ = stdout.read_to_end(&mut buf).await;
                    buf
                };
                let read_err = async move {
                    let mut buf = Vec::new();
                    let _ = stderr_pipe.read_to_end(&mut buf).await;
                    buf
                };

                let (write_res, _out, err_buf) = tokio::join!(writer, read_out, read_err);

                let status = child.wait().await
                    .map_err(|e| AppError::internal(&format!("Failed to wait for psql: {}", e)))?;

                let stderr = String::from_utf8_lossy(&err_buf);
                if !status.success() {
                    // psql's own error (e.g. ON_ERROR_STOP) is more useful than a broken-pipe
                    // write error, so report stderr/status here regardless of the writer result.
                    return Err(AppError::internal(&format!("psql restore failed (rolled back): {}", stderr.trim())));
                }
                // psql exited clean; surface a writer error only if one slipped through.
                write_res?;
                if !stderr.trim().is_empty() {
                    tracing::warn!("psql restore reported messages: {}", stderr.trim());
                }
                tracing::info!("psql restore completed successfully");
            },
            mysql(_) => {
                // MySQL TRUNCATE/DDL auto-commits, so the load can't be a single atomic
                // transaction like Postgres. Clear via transactional DELETE (FK checks
                // off) first, then stream the dump; mysql stops on the first error.
                self.clear_all_data().await?;

                let host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
                let port = std::env::var("DB_PORT").unwrap_or_else(|_| "3306".to_string());
                let database = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods".to_string());
                let username = std::env::var("DB_USER").unwrap_or_else(|_| "root".to_string());
                let password = std::env::var("DB_PASSWORD").unwrap_or_else(|_| "password".to_string());

                let mut cmd = Command::new("mysql");
                cmd.arg("--host").arg(&host)
                   .arg("--port").arg(&port)
                   .arg("--user").arg(&username)
                   .arg(format!("--password={}", password))
                   .arg("--skip-ssl")
                   .arg("--default-auth=mysql_native_password")
                   .arg(&database)
                   .stdin(std::process::Stdio::piped())
                   .stdout(std::process::Stdio::piped())
                   .stderr(std::process::Stdio::piped());

                let mut child = cmd.spawn()
                    .map_err(|e| AppError::internal(&format!("Failed to start mysql: {}", e)))?;

                // Drain stdin/stdout/stderr concurrently. Writing the whole dump first and
                // only reading mysql's output afterward deadlocks on large restores once
                // mysql's ~64KB output pipe fills and it stops reading stdin (see the
                // Postgres branch above for the full explanation).
                let mut stdin = child.stdin.take()
                    .ok_or_else(|| AppError::internal("mysql stdin was not piped"))?;
                let mut stdout = child.stdout.take()
                    .ok_or_else(|| AppError::internal("mysql stdout was not piped"))?;
                let mut stderr_pipe = child.stderr.take()
                    .ok_or_else(|| AppError::internal("mysql stderr was not piped"))?;

                let path_buf = path.to_path_buf();
                let writer = async move {
                    let file = tokio::fs::File::open(&path_buf).await
                        .map_err(|e| AppError::internal(&format!("Failed to open backup file: {}", e)))?;
                    // Drop INSERT/LOCK statements for the transient session tables so a
                    // legacy/mismatched schema can't abort the load (see the Postgres branch).
                    stream_mysql_dump_skipping_tables(file, &mut stdin, SKIP_TABLES).await
                        .map_err(|e| AppError::internal(&format!("Failed to stream SQL to mysql: {}", e)))?;
                    // Drop stdin to signal EOF to mysql.
                    drop(stdin);
                    Ok::<(), AppError>(())
                };

                use tokio::io::AsyncReadExt;
                let read_out = async move {
                    let mut buf = Vec::new();
                    let _ = stdout.read_to_end(&mut buf).await;
                    buf
                };
                let read_err = async move {
                    let mut buf = Vec::new();
                    let _ = stderr_pipe.read_to_end(&mut buf).await;
                    buf
                };

                let (write_res, _out, err_buf) = tokio::join!(writer, read_out, read_err);

                let status = child.wait().await
                    .map_err(|e| AppError::internal(&format!("Failed to wait for mysql: {}", e)))?;

                // mysql in batch mode stops on the first error and exits non-zero
                // (no --force), so a non-zero status means the restore did not complete.
                let stderr = String::from_utf8_lossy(&err_buf);
                if !status.success() {
                    return Err(AppError::internal(&format!("mysql restore failed: {}", stderr.trim())));
                }
                write_res?;
                if !stderr.trim().is_empty() {
                    tracing::warn!("mysql restore reported messages: {}", stderr.trim());
                }
                tracing::info!("mysql restore completed successfully");
            },
        });

        Ok(())
    }

    // Clear all data from tables while preserving schema
    pub(super) async fn clear_all_data(&self) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                // Disable foreign key constraints temporarily
                q::query("SET session_replication_role = replica;").execute(pool).await?;
                
                // Get all table names
                let tables = q::query(r#"
                    SELECT tablename FROM pg_tables 
                    WHERE schemaname = 'public' 
                    AND tablename NOT LIKE '%_seq'
                "#)
                .fetch_all(pool)
                .await?;
                
                // Clear each table
                for table in tables {
                    let table_name: String = table.try_get("tablename")?;
                    let query = format!(r#"TRUNCATE TABLE "{}" RESTART IDENTITY CASCADE"#, table_name);
                    q::query(sqlx::AssertSqlSafe(query.as_str())).execute(pool).await?;
                }
                
                // Re-enable foreign key constraints
                q::query("SET session_replication_role = DEFAULT;").execute(pool).await?;
            },
            mysql(pool) => {
                // Run the whole clear on ONE pinned connection. `SET FOREIGN_KEY_CHECKS = 0` is
                // a session variable, so issuing each statement via `.execute(pool)` (which can
                // hand back a different pooled connection each time) leaves FK checks ON for the
                // DELETEs -- and a DELETE on a parent table then fails with ERROR 1451, aborting
                // the restore before the dump is ever loaded. Acquiring a single connection keeps
                // FK checks disabled across every DELETE.
                let mut conn = pool.acquire().await?;

                // Disable foreign key checks
                sqlx::query("SET FOREIGN_KEY_CHECKS = 0;").execute(&mut *conn).await?;

                // Get all table names
                let database = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods".to_string());
                let tables_query = format!(r#"
                    SELECT TABLE_NAME FROM information_schema.TABLES
                    WHERE TABLE_SCHEMA = '{}' AND TABLE_TYPE = 'BASE TABLE'
                "#, database);
                let tables = sqlx::query(sqlx::AssertSqlSafe(tables_query.as_str()))
                .fetch_all(&mut *conn)
                .await?;

                // Clear each table
                for table in tables {
                    // Handle both VARCHAR and VARBINARY cases for TABLE_NAME
                    let table_name: String = if let Ok(name) = table.try_get::<String, _>("TABLE_NAME") {
                        name
                    } else if let Ok(name_bytes) = table.try_get::<Vec<u8>, _>("TABLE_NAME") {
                        String::from_utf8(name_bytes).map_err(|_| AppError::internal("Invalid table name encoding"))?
                    } else {
                        return Err(AppError::internal("Could not retrieve table name"));
                    };
                    // Use DELETE instead of TRUNCATE to avoid foreign key constraint issues
                    let query = format!("DELETE FROM `{}`", table_name);
                    sqlx::query(sqlx::AssertSqlSafe(query.as_str())).execute(&mut *conn).await?;
                }

                // Re-enable foreign key checks
                sqlx::query("SET FOREIGN_KEY_CHECKS = 1;").execute(&mut *conn).await?;
            },
        });
        Ok(())
    }

    // Set scheduled backup configuration. retention_count: None/Some(0) = keep all backups.
    pub async fn set_scheduled_backup(&self, user_id: i32, cron_schedule: &str, enabled: bool, retention_count: Option<i32>) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"
                    INSERT INTO "ScheduledBackups" (userid, cron_schedule, enabled, retention_count, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, NOW(), NOW())
                    ON CONFLICT (userid)
                    DO UPDATE SET
                        cron_schedule = EXCLUDED.cron_schedule,
                        enabled = EXCLUDED.enabled,
                        retention_count = EXCLUDED.retention_count,
                        updated_at = NOW()
                "#)
                .bind(user_id)
                .bind(cron_schedule)
                .bind(enabled)
                .bind(retention_count)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                sqlx::query(r#"
                    INSERT INTO ScheduledBackups (UserID, CronSchedule, Enabled, RetentionCount, CreatedAt, UpdatedAt)
                    VALUES (?, ?, ?, ?, NOW(), NOW())
                    ON DUPLICATE KEY UPDATE
                        CronSchedule = VALUES(CronSchedule),
                        Enabled = VALUES(Enabled),
                        RetentionCount = VALUES(RetentionCount),
                        UpdatedAt = NOW()
                "#)
                .bind(user_id)
                .bind(cron_schedule)
                .bind(enabled)
                .bind(retention_count)
                .execute(pool)
                .await?;
            },
        });
        Ok(())
    }

    // Get scheduled backup configuration
    pub async fn get_scheduled_backup(&self, user_id: i32) -> AppResult<serde_json::Value> {
        dialect_match!(self, {
            pg(pool) => {
                let row = q::query(r#"
                    SELECT cron_schedule, enabled, retention_count, created_at, updated_at
                    FROM "ScheduledBackups"
                    WHERE userid = $1
                "#)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    Ok(serde_json::json!({
                        "schedule": row.get::<String, _>("cron_schedule"),
                        "enabled": row.get::<bool, _>("enabled"),
                        "retention_count": row.get::<Option<i32>, _>("retention_count"),
                        "created_at": row.get::<chrono::NaiveDateTime, _>("created_at").format("%Y-%m-%dT%H:%M:%S").to_string(),
                        "updated_at": row.get::<chrono::NaiveDateTime, _>("updated_at").format("%Y-%m-%dT%H:%M:%S").to_string()
                    }))
                } else {
                    Ok(serde_json::json!({
                        "schedule": null,
                        "enabled": false,
                        "retention_count": null,
                        "created_at": null,
                        "updated_at": null
                    }))
                }
            },
            mysql(pool) => {
                let row = sqlx::query(r#"
                    SELECT CronSchedule, Enabled, RetentionCount, CreatedAt, UpdatedAt
                    FROM ScheduledBackups
                    WHERE UserID = ?
                "#)
                .bind(user_id)
                .fetch_optional(pool)
                .await?;

                if let Some(row) = row {
                    let created_datetime = row.try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>("CreatedAt")?;
                    let updated_datetime = row.try_get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>("UpdatedAt")?;

                    Ok(serde_json::json!({
                        "schedule": row.try_get::<String, _>("CronSchedule")?,
                        "enabled": row.try_get::<bool, _>("Enabled")?,
                        "retention_count": row.try_get::<Option<i32>, _>("RetentionCount")?,
                        "created_at": created_datetime.format("%Y-%m-%dT%H:%M:%S").to_string(),
                        "updated_at": updated_datetime.format("%Y-%m-%dT%H:%M:%S").to_string()
                    }))
                } else {
                    Ok(serde_json::json!({
                        "schedule": null,
                        "enabled": false,
                        "retention_count": null,
                        "created_at": null,
                        "updated_at": null
                    }))
                }
            },
        })
    }

    // Get all enabled scheduled backups (used by the background scheduler to decide what is due)
    pub async fn get_enabled_scheduled_backups(&self) -> AppResult<Vec<ScheduledBackupRow>> {
        let mut backups = Vec::new();
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(r#"
                    SELECT userid, cron_schedule, retention_count, last_run
                    FROM "ScheduledBackups"
                    WHERE enabled = TRUE
                "#)
                .fetch_all(pool)
                .await?;

                for row in rows {
                    backups.push(ScheduledBackupRow {
                        user_id: row.get::<i32, _>("userid"),
                        cron_schedule: row.get::<String, _>("cron_schedule"),
                        retention_count: row.get::<Option<i32>, _>("retention_count"),
                        last_run: row.get::<Option<chrono::NaiveDateTime>, _>("last_run")
                            .map(|dt| DateTime::<Utc>::from_naive_utc_and_offset(dt, Utc)),
                    });
                }
            },
            mysql(pool) => {
                let rows = sqlx::query(r#"
                    SELECT UserID, CronSchedule, RetentionCount, LastRun
                    FROM ScheduledBackups
                    WHERE Enabled = TRUE
                "#)
                .fetch_all(pool)
                .await?;

                for row in rows {
                    backups.push(ScheduledBackupRow {
                        user_id: row.try_get::<i32, _>("UserID")?,
                        cron_schedule: row.try_get::<String, _>("CronSchedule")?,
                        retention_count: row.try_get::<Option<i32>, _>("RetentionCount")?,
                        last_run: row.try_get::<Option<DateTime<Utc>>, _>("LastRun")?,
                    });
                }
            },
        });
        Ok(backups)
    }

    // Record the time the scheduler last ran a given user's scheduled backup
    pub async fn update_scheduled_backup_last_run(&self, user_id: i32, ts: DateTime<Utc>) -> AppResult<()> {
        dialect_match!(self, {
            pg(pool) => {
                q::query(r#"UPDATE "ScheduledBackups" SET last_run = $1 WHERE userid = $2"#)
                    .bind(ts.naive_utc())
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
            mysql(pool) => {
                sqlx::query(r#"UPDATE ScheduledBackups SET LastRun = ? WHERE UserID = ?"#)
                    .bind(ts)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    // Execute backup to file (called by scheduler)
    pub async fn execute_scheduled_backup(&self, _user_id: i32) -> AppResult<String> {
        use tokio::process::Command;
        use chrono::Utc;

        // Generate backup filename with timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let backup_filename = format!("scheduled_backup_{}.sql", timestamp);
        let backup_path = format!("{}/{}", crate::paths::backups_dir(), backup_filename);

        // Ensure backup directory exists (matches manual_backup_to_directory)
        if let Err(e) = std::fs::create_dir_all(crate::paths::backups_dir()) {
            return Err(AppError::internal(&format!("Failed to create backup directory: {}", e)));
        }

        if let Some(result) = self.sqlite_backup_to(std::path::Path::new(&backup_path)).await {
            return result.map(|_| backup_filename);
        }

        // Get database password from environment
        let db_password = std::env::var("DB_PASSWORD")
            .map_err(|_| AppError::internal("Database password not found in environment"))?;

        dialect_match!(self, {
            pg(_) => {
                let db_host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
                let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "5432".to_string());
                let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "postgres".to_string());
                let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods_database".to_string());

                // Use the same restore-safe flags as manual_backup_to_directory so the
                // resulting dump can be cleanly restored via `psql -f`.
                let mut cmd = Command::new("pg_dump");
                cmd.arg("-h").arg(&db_host)
                   .arg("-p").arg(&db_port)
                   .arg("-U").arg(&db_user)
                   .arg("-d").arg(&db_name)
                   .arg("--clean")
                   .arg("--if-exists")
                   .arg("--no-owner")
                   .arg("--no-privileges")
                   // Transient login-token tables: excluded for security and to avoid
                   // cross-version schema drift on restore.
                   .arg("--exclude-table-data=public.\"Sessions\"")
                   .arg("--exclude-table-data=public.\"GpodderSessions\"")
                   .arg("-f").arg(&backup_path)
                   .env("PGPASSWORD", &db_password);

                let output = cmd.output().await
                    .map_err(|e| AppError::internal(&format!("Failed to execute backup: {}", e)))?;

                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    return Err(AppError::internal(&format!("Backup failed: {}", error_msg)));
                }
            },
            mysql(_) => {
                let db_host = std::env::var("DB_HOST").unwrap_or_else(|_| "localhost".to_string());
                let db_port = std::env::var("DB_PORT").unwrap_or_else(|_| "3306".to_string());
                let db_user = std::env::var("DB_USER").unwrap_or_else(|_| "mysql".to_string());
                let db_name = std::env::var("DB_NAME").unwrap_or_else(|_| "pinepods_database".to_string());

                let mut cmd = Command::new("mysqldump");
                cmd.arg("-h").arg(&db_host)
                   .arg("-P").arg(&db_port)
                   .arg("-u").arg(&db_user)
                   .arg(format!("-p{}", &db_password))
                   .arg(&db_name)
                   .arg("--result-file").arg(&backup_path)
                   .arg("--single-transaction")
                   .arg("--routines")
                   .arg("--triggers")
                   // Transient login-token tables: excluded for security and to avoid
                   // cross-version schema drift on restore.
                   .arg(format!("--ignore-table={}.Sessions", db_name))
                   .arg(format!("--ignore-table={}.GpodderSessions", db_name));

                let output = cmd.output().await
                    .map_err(|e| AppError::internal(&format!("Failed to execute backup: {}", e)))?;

                if !output.status.success() {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    return Err(AppError::internal(&format!("Backup failed: {}", error_msg)));
                }
            },
        });

        Ok(backup_filename)
    }
}

#[cfg(test)]
mod restore_filter_tests {
    use super::{stream_mysql_dump_skipping_tables, stream_pg_dump_skipping_tables};

    async fn run_pg(input: &str, skip: &[&str]) -> String {
        let mut out: Vec<u8> = Vec::new();
        stream_pg_dump_skipping_tables(input.as_bytes(), &mut out, skip)
            .await
            .unwrap();
        String::from_utf8(out).unwrap()
    }

    #[tokio::test]
    async fn pg_strips_skipped_table_copy_block_and_keeps_others() {
        let input = concat!(
            "SET something = 1;\n",
            "COPY public.\"Podcasts\" (id, title) FROM stdin;\n",
            "1\tHello\n",
            "2\tCOPY public.\"Sessions\" looks like a header but is data\n",
            "\\.\n",
            "COPY public.\"Sessions\" (sessionid, userid, value, expire) FROM stdin;\n",
            "1\t2\tsecret\t1234\n",
            "\\.\n",
            "COPY public.\"Episodes\" (id) FROM stdin;\n",
            "9\n",
            "\\.\n",
        );
        let out = run_pg(input, &["Sessions", "GpodderSessions"]).await;
        // Podcasts block preserved verbatim, including the data row that starts with COPY.
        assert!(out.contains("COPY public.\"Podcasts\" (id, title) FROM stdin;"));
        assert!(out.contains("2\tCOPY public.\"Sessions\" looks like a header but is data"));
        // Episodes block preserved.
        assert!(out.contains("COPY public.\"Episodes\" (id) FROM stdin;"));
        assert!(out.contains("\n9\n"));
        // Sessions header + data fully removed.
        assert!(!out.contains("sessionid, userid, value, expire"));
        assert!(!out.contains("1\t2\tsecret\t1234"));
    }

    #[tokio::test]
    async fn pg_does_not_match_substring_table_name() {
        let input = concat!(
            "COPY public.\"GpodderSyncState\" (id) FROM stdin;\n",
            "1\n",
            "\\.\n",
        );
        let out = run_pg(input, &["Sessions"]).await;
        // "Sessions" must not match inside "GpodderSyncState"/"GpodderSessions".
        assert!(out.contains("GpodderSyncState"));
        assert!(out.contains("\n1\n"));
    }

    #[tokio::test]
    async fn mysql_strips_skipped_table_inserts() {
        let input = concat!(
            "INSERT INTO `Podcasts` (id) VALUES (1);\n",
            "LOCK TABLES `Sessions` WRITE;\n",
            "INSERT INTO `Sessions` (sessionid) VALUES (1);\n",
            "UNLOCK TABLES;\n",
            "INSERT INTO `Episodes` (id) VALUES (9);\n",
        );
        let mut out: Vec<u8> = Vec::new();
        stream_mysql_dump_skipping_tables(input.as_bytes(), &mut out, &["Sessions"])
            .await
            .unwrap();
        let out = String::from_utf8(out).unwrap();
        assert!(out.contains("INSERT INTO `Podcasts`"));
        assert!(out.contains("INSERT INTO `Episodes`"));
        assert!(!out.contains("INSERT INTO `Sessions`"));
        assert!(!out.contains("LOCK TABLES `Sessions`"));
    }
}
