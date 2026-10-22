//! Raw SQL helpers for tests that seed or inspect rows the API doesn't expose (migration
//! tests, refresh internals).

use pinepods_api::database::DatabasePool;

use super::TestApp;

/// Run `sql` on the test database. Write table names double-quoted (Postgres style); the quotes
/// are dropped for MariaDB. Only inline literals, no binds.
pub async fn exec(app: &TestApp, sql: &str) {
    match app.db() {
        DatabasePool::Postgres(pool) => {
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql.to_string())).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
        }
        DatabasePool::MySQL(pool) => {
            let sql = sql.replace('"', "");
            sqlx::raw_sql(sqlx::AssertSqlSafe(sql.clone())).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
        }
    }
}

/// First column of the first row as an integer (counts, ids, booleans), None for SQL NULL / no row.
pub async fn int(app: &TestApp, sql: &str) -> Option<i64> {
    use sqlx::Row;
    fn get<R: Row>(row: &R) -> Option<i64>
    where
        usize: sqlx::ColumnIndex<R>,
        i64: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
        i32: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
        bool: for<'r> sqlx::Decode<'r, R::Database> + sqlx::Type<R::Database>,
    {
        if let Ok(v) = row.try_get::<Option<i64>, _>(0) {
            return v;
        }
        if let Ok(v) = row.try_get::<Option<i32>, _>(0) {
            return v.map(i64::from);
        }
        row.try_get::<Option<bool>, _>(0).unwrap().map(i64::from)
    }
    match app.db() {
        DatabasePool::Postgres(pool) => sqlx::query(sqlx::AssertSqlSafe(sql.to_string()))
            .fetch_optional(pool)
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"))
            .and_then(|r| get(&r)),
        DatabasePool::MySQL(pool) => {
            let sql = sql.replace('"', "");
            sqlx::query(sqlx::AssertSqlSafe(sql.clone()))
                .fetch_optional(pool)
                .await
                .unwrap_or_else(|e| panic!("{sql}: {e}"))
                .and_then(|r| get(&r))
        }
    }
}

/// First column of the first row as text, None for SQL NULL / no row.
pub async fn text(app: &TestApp, sql: &str) -> Option<String> {
    use sqlx::Row;
    match app.db() {
        DatabasePool::Postgres(pool) => sqlx::query(sqlx::AssertSqlSafe(sql.to_string()))
            .fetch_optional(pool)
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"))
            .and_then(|r| r.try_get::<Option<String>, _>(0).unwrap()),
        DatabasePool::MySQL(pool) => {
            let sql = sql.replace('"', "");
            sqlx::query(sqlx::AssertSqlSafe(sql.clone()))
                .fetch_optional(pool)
                .await
                .unwrap_or_else(|e| panic!("{sql}: {e}"))
                .and_then(|r| r.try_get::<Option<String>, _>(0).unwrap())
        }
    }
}

/// Forget `versions` in schema_migrations and run the real migration runner again, so those
/// migrations apply to whatever old-shape data the test seeded.
pub async fn rerun_migrations(app: &TestApp, versions: &[&str]) {
    let list = versions.iter().map(|v| format!("'{v}'")).collect::<Vec<_>>().join(", ");
    exec(app, &format!("DELETE FROM schema_migrations WHERE version IN ({list})")).await;
    let env = app.env;
    let db_name = app.db_name.clone();
    tokio::task::spawn_blocking(move || super::env::run_migrations(env, &db_name)).await.unwrap();
}
