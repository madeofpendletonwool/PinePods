//! Server backup and restore on SQLite. A backup is a SQL dump of the schema and data, taken
//! from one read snapshot. A restore loads it into a scratch database, then replaces every
//! table's rows from it in one write transaction, copying the columns both schemas have, so a
//! backup from an older Pinepods version still restores.

use std::path::{Path, PathBuf};

use futures::TryStreamExt;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteRow, SqliteSynchronous};
use sqlx::{Connection, Pool, Row, Sqlite, SqliteConnection, TypeInfo, ValueRef};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};

use super::dialect::lite::lock_writer;
use crate::error::{AppError, AppResult};

/// First line of every SQLite backup.
pub const HEADER: &str = "-- Pinepods SQLite backup";

/// Login-token tables: their rows are never backed up or restored.
const SKIP_DATA: &[&str] = &["Sessions", "GpodderSessions"];

/// Records the migrations applied to this database; a restore keeps the live one.
const MIGRATIONS: &str = "schema_migrations";

fn io_err(what: &str, e: impl std::fmt::Display) -> AppError {
    AppError::internal(&format!("{what}: {e}"))
}

fn ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn text_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Whether the file at `path` is a SQLite backup (and so not a pg_dump / mysqldump one).
pub async fn is_sqlite_backup(path: &Path) -> AppResult<bool> {
    let file = tokio::fs::File::open(path).await.map_err(|e| io_err("Failed to open backup file", e))?;
    let mut first = Vec::new();
    BufReader::new(file.take(256))
        .read_until(b'\n', &mut first)
        .await
        .map_err(|e| io_err("Failed to read backup file", e))?;
    Ok(String::from_utf8_lossy(&first).trim_end() == HEADER)
}

/// Write a backup of the whole database to `path`.
pub async fn dump(pool: &Pool<Sqlite>, path: &Path) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    // Under WAL a read transaction sees one snapshot throughout, and doesn't block writers.
    sqlx::query("BEGIN").execute(&mut *conn).await?;
    let result = dump_snapshot(&mut conn, path).await;
    let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(path).await;
    }
    result
}

async fn dump_snapshot(conn: &mut SqliteConnection, path: &Path) -> AppResult<()> {
    let file = tokio::fs::File::create(path).await.map_err(|e| io_err("Failed to create backup file", e))?;
    let mut out = BufWriter::new(file);
    let write_err = |e: std::io::Error| io_err("Failed to write backup file", e);

    let objects: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT type, name, sql FROM sqlite_master \
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' \
         ORDER BY CASE type WHEN 'table' THEN 0 ELSE 1 END, name",
    )
    .fetch_all(&mut *conn)
    .await?;

    out.write_all(format!("{HEADER}\n").as_bytes()).await.map_err(write_err)?;
    for (_, _, sql) in objects.iter().filter(|(kind, _, _)| kind == "table") {
        out.write_all(format!("{sql};\n").as_bytes()).await.map_err(write_err)?;
    }

    let mut tables: Vec<&str> = objects
        .iter()
        .filter(|(kind, name, _)| kind == "table" && !SKIP_DATA.contains(&name.as_str()))
        .map(|(_, name, _)| name.as_str())
        .collect();
    let has_sequence: bool = sqlx::query_scalar("SELECT COUNT(*) > 0 FROM sqlite_master WHERE name = 'sqlite_sequence'")
        .fetch_one(&mut *conn)
        .await?;
    if has_sequence {
        out.write_all(b"DELETE FROM sqlite_sequence;\n").await.map_err(write_err)?;
        tables.push("sqlite_sequence");
    }

    for table in tables {
        let select = format!("SELECT * FROM {}", ident(table));
        let mut rows = sqlx::query(sqlx::AssertSqlSafe(select)).fetch(&mut *conn);
        let prefix = format!("INSERT INTO {} VALUES(", ident(table));
        while let Some(row) = rows.try_next().await? {
            let mut line = prefix.clone();
            for i in 0..row.len() {
                if i > 0 {
                    line.push(',');
                }
                line.push_str(&value_literal(&row, i)?);
            }
            line.push_str(");\n");
            out.write_all(line.as_bytes()).await.map_err(write_err)?;
        }
    }

    for (_, _, sql) in objects.iter().filter(|(kind, _, _)| kind != "table") {
        out.write_all(format!("{sql};\n").as_bytes()).await.map_err(write_err)?;
    }
    out.flush().await.map_err(write_err)?;
    out.into_inner().sync_all().await.map_err(write_err)?;
    Ok(())
}

/// A column value as a SQL literal, by the value's own storage class.
fn value_literal(row: &SqliteRow, i: usize) -> Result<String, sqlx::Error> {
    let raw = row.try_get_raw(i)?;
    if raw.is_null() {
        return Ok("NULL".into());
    }
    let kind = raw.type_info().name().to_string();
    Ok(match kind.as_str() {
        "INTEGER" => row.try_get_unchecked::<i64, _>(i)?.to_string(),
        "REAL" => {
            let f: f64 = row.try_get_unchecked(i)?;
            match f {
                f if f.is_finite() => format!("{f:?}"),
                f if f > 0.0 => "9e999".into(),
                _ => "-9e999".into(),
            }
        }
        "BLOB" => format!("X'{}'", hex::encode(row.try_get_unchecked::<Vec<u8>, _>(i)?)),
        _ => text_literal(&row.try_get_unchecked::<String, _>(i)?),
    })
}

/// Splits SQL text into statements, fed one line at a time. Knows quoted strings and
/// identifiers, `--` comments, and `CREATE TRIGGER … END;` bodies.
#[derive(Default)]
struct Splitter {
    current: String,
    quote: Option<char>,
}

impl Splitter {
    fn feed(&mut self, line: &str, mut emit: impl FnMut(String)) {
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if let Some(q) = self.quote {
                self.current.push(c);
                if c == q {
                    self.quote = None;
                }
                continue;
            }
            match c {
                '\'' | '"' => {
                    self.quote = Some(c);
                    self.current.push(c);
                }
                '-' if chars.peek() == Some(&'-') => {
                    // A comment runs to the end of the line.
                    self.current.push('\n');
                    break;
                }
                ';' if !self.in_trigger_body() => {
                    let statement = std::mem::take(&mut self.current);
                    let statement = statement.trim();
                    if !statement.is_empty() {
                        emit(statement.to_string());
                    }
                }
                _ => self.current.push(c),
            }
        }
    }

    /// Inside a trigger, `;` ends the statement only after its closing `END`.
    fn in_trigger_body(&self) -> bool {
        let upper = self.current.trim().to_ascii_uppercase();
        upper.starts_with("CREATE TRIGGER") && !upper.ends_with("END")
    }
}

/// Restore the backup at `path` over the database's current contents.
pub async fn restore(pool: &Pool<Sqlite>, path: &Path) -> AppResult<()> {
    let live = pool.connect_options().get_filename().to_path_buf();
    let scratch = live.with_file_name(format!(".restore-{}.db", uuid::Uuid::new_v4()));
    let result = async {
        load_scratch(path, &scratch).await?;
        replace_live_data(pool, &scratch).await
    }
    .await;
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let mut p = scratch.clone().into_os_string();
        p.push(suffix);
        let _ = tokio::fs::remove_file(PathBuf::from(p)).await;
    }
    result
}

/// Run the backup's SQL into a new database file.
async fn load_scratch(path: &Path, scratch: &Path) -> AppResult<()> {
    let options = SqliteConnectOptions::new()
        .filename(scratch)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Off)
        .synchronous(SqliteSynchronous::Off)
        .foreign_keys(false);
    let mut conn = SqliteConnection::connect_with(&options).await?;
    sqlx::query("BEGIN").execute(&mut conn).await?;

    let file = tokio::fs::File::open(path).await.map_err(|e| io_err("Failed to open backup file", e))?;
    let mut reader = BufReader::new(file);
    let mut splitter = Splitter::default();
    let mut batch = String::new();
    let mut line = String::new();
    loop {
        line.clear();
        let n = reader.read_line(&mut line).await.map_err(|e| io_err("Failed to read backup file", e))?;
        if n == 0 {
            break;
        }
        splitter.feed(&line, |statement| {
            // Transaction control and pragmas from a hand-made dump are ours to decide.
            let upper = statement.to_ascii_uppercase();
            if ["BEGIN", "COMMIT", "END", "ROLLBACK", "PRAGMA"].iter().any(|k| upper.starts_with(k)) {
                return;
            }
            batch.push_str(&statement);
            batch.push_str(";\n");
        });
        if batch.len() > 1 << 20 {
            run_batch(&mut conn, &mut batch).await?;
        }
    }
    if !splitter.current.trim().is_empty() {
        return Err(AppError::bad_request("The backup file ends partway through a statement"));
    }
    run_batch(&mut conn, &mut batch).await?;
    sqlx::query("COMMIT").execute(&mut conn).await?;
    conn.close().await?;
    Ok(())
}

async fn run_batch(conn: &mut SqliteConnection, batch: &mut String) -> AppResult<()> {
    if batch.is_empty() {
        return Ok(());
    }
    let sql = std::mem::take(batch);
    sqlx::raw_sql(sqlx::AssertSqlSafe(sql))
        .execute(&mut *conn)
        .await
        .map_err(|e| AppError::bad_request(&format!("The backup file couldn't be loaded: {e}")))?;
    Ok(())
}

/// Replace every live table's rows with the scratch database's, in one transaction.
async fn replace_live_data(pool: &Pool<Sqlite>, scratch: &Path) -> AppResult<()> {
    let _writer = lock_writer(pool).await;
    let mut conn = pool.acquire().await?;
    // Foreign keys can only be switched outside a transaction, and ATTACH needs one too.
    sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await?;
    sqlx::query("ATTACH DATABASE ? AS bak")
        .bind(scratch.to_string_lossy().into_owned())
        .execute(&mut *conn)
        .await?;

    let result = copy_tables(&mut conn).await;
    if result.is_err() {
        let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
    }
    let detached = sqlx::query("DETACH DATABASE bak").execute(&mut *conn).await.is_ok();
    let fk_on = sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.is_ok();
    if !(detached && fk_on) {
        // Don't hand a half-reset connection back to the pool.
        let _ = conn.detach().close().await;
    }
    result
}

async fn copy_tables(conn: &mut SqliteConnection) -> AppResult<()> {
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    let has_migrations: bool =
        sqlx::query_scalar("SELECT COUNT(*) > 0 FROM bak.sqlite_master WHERE type = 'table' AND name = ?")
            .bind(MIGRATIONS)
            .fetch_one(&mut *conn)
            .await?;
    if !has_migrations {
        return Err(AppError::bad_request("This file isn't a complete Pinepods backup"));
    }
    let newer: Vec<String> = sqlx::query_scalar(
        "SELECT version FROM bak.schema_migrations WHERE version NOT IN (SELECT version FROM main.schema_migrations)",
    )
    .fetch_all(&mut *conn)
    .await?;
    if !newer.is_empty() {
        return Err(AppError::bad_request(
            "This backup is from a newer Pinepods version. Upgrade Pinepods before restoring it.",
        ));
    }

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM main.sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> ? ORDER BY name",
    )
    .bind(MIGRATIONS)
    .fetch_all(&mut *conn)
    .await?;

    for table in &tables {
        sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM main.{}", ident(table))))
            .execute(&mut *conn)
            .await?;
        if SKIP_DATA.contains(&table.as_str()) {
            continue;
        }
        let columns: Vec<String> = sqlx::query_scalar(
            "SELECT m.name FROM pragma_table_info(?1, 'main') m \
             JOIN pragma_table_info(?1, 'bak') b ON b.name = m.name ORDER BY m.cid",
        )
        .bind(table)
        .fetch_all(&mut *conn)
        .await?;
        if columns.is_empty() {
            continue;
        }
        let list = columns.iter().map(|c| ident(c)).collect::<Vec<_>>().join(", ");
        let insert = format!(
            "INSERT INTO main.{t} ({list}) SELECT {list} FROM bak.{t}",
            t = ident(table)
        );
        sqlx::query(sqlx::AssertSqlSafe(insert))
            .execute(&mut *conn)
            .await
            .map_err(|e| AppError::bad_request(&format!("Restoring {table} failed: {e}")))?;
    }

    let sequences: i64 = sqlx::query_scalar(
        "SELECT (SELECT COUNT(*) FROM main.sqlite_master WHERE name = 'sqlite_sequence') \
              + (SELECT COUNT(*) FROM bak.sqlite_master WHERE name = 'sqlite_sequence')",
    )
    .fetch_one(&mut *conn)
    .await?;
    if sequences == 2 {
        sqlx::query("DELETE FROM main.sqlite_sequence").execute(&mut *conn).await?;
        sqlx::query(
            "INSERT INTO main.sqlite_sequence (name, seq) SELECT name, seq FROM bak.sqlite_sequence \
             WHERE name IN (SELECT name FROM main.sqlite_master WHERE type = 'table')",
        )
        .execute(&mut *conn)
        .await?;
    }

    sqlx::query("COMMIT").execute(&mut *conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Splitter;

    fn split(text: &str) -> Vec<String> {
        let mut s = Splitter::default();
        let mut out = Vec::new();
        for line in text.split_inclusive('\n') {
            s.feed(line, |st| out.push(st));
        }
        out
    }

    #[test]
    fn splits_statements() {
        let text = "-- Pinepods SQLite backup\nCREATE TABLE \"a;b\" (x TEXT);\nINSERT INTO \"a;b\" VALUES('it''s; -- not\na comment');INSERT INTO t VALUES(1);\n\
                    CREATE TRIGGER tr AFTER INSERT ON t BEGIN UPDATE t SET x = 1; END;\n";
        assert_eq!(
            split(text),
            vec![
                "CREATE TABLE \"a;b\" (x TEXT)".to_string(),
                "INSERT INTO \"a;b\" VALUES('it''s; -- not\na comment')".to_string(),
                "INSERT INTO t VALUES(1)".to_string(),
                "CREATE TRIGGER tr AFTER INSERT ON t BEGIN UPDATE t SET x = 1; END".to_string(),
            ]
        );
    }
}
