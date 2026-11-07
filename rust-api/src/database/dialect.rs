//! PostgreSQL query code that also runs on SQLite.
//!
//! [`dialect_match!`] compiles a PostgreSQL arm twice: once against [`pg`], where `q::query` is
//! plain `sqlx::query`, and once against [`lite`], where `q::query` rewrites the PostgreSQL text
//! for SQLite (see [`translate`]) and binds values the way the SQLite schema stores them. The
//! SQLite schema keeps PostgreSQL's lowercase column names, so rows read the same in both.
//!
//! Inside an arm, go through `q::` for queries, transactions and types (`q::query`,
//! `q::query_as::<T>`, `q::query_scalar::<T>`, `q::begin`, `q::Tx`, `q::Conn`, `q::Row`). Give
//! an arm of its own (`sqlite(pool) => ...`) to the rare query [`translate`] can't express.

use std::sync::LazyLock;

use regex::Regex;

/// `match` on a [`DatabasePool`](super::DatabasePool), sharing the PostgreSQL arm with SQLite.
///
/// ```ignore
/// dialect_match!(self, {
///     pg(pool) => { q::query(r#"SELECT ... FROM "Users" WHERE userid = $1"#).bind(id).fetch_one(pool).await? },
///     mysql(pool) => { sqlx::query("SELECT ... FROM Users WHERE UserID = ?").bind(id).fetch_one(pool).await? },
/// })
/// ```
macro_rules! dialect_match {
    ($db:expr, { pg($pg:pat) => $pg_body:expr, sqlite($lite:pat) => $lite_body:expr, mysql($my:pat) => $my_body:expr $(,)? }) => {
        match $db {
            $crate::database::DatabasePool::Postgres($pg) => {
                #[allow(unused_imports)]
                use $crate::database::dialect::pg as q;
                $pg_body
            }
            $crate::database::DatabasePool::Sqlite($lite) => {
                #[allow(unused_imports)]
                use $crate::database::dialect::lite as q;
                $lite_body
            }
            $crate::database::DatabasePool::MySQL($my) => $my_body,
        }
    };
    ($db:expr, { pg($pg:pat) => $pg_body:expr, mysql($my:pat) => $my_body:expr $(,)? }) => {
        match $db {
            $crate::database::DatabasePool::Postgres($pg) => {
                #[allow(unused_imports)]
                use $crate::database::dialect::pg as q;
                $pg_body
            }
            $crate::database::DatabasePool::Sqlite($pg) => {
                #[allow(unused_imports)]
                use $crate::database::dialect::lite as q;
                $pg_body
            }
            $crate::database::DatabasePool::MySQL($my) => $my_body,
        }
    };
}
pub(crate) use dialect_match;

/// A helper written once for PostgreSQL and SQLite: defines `$pg` and `$lite`, each with `q`
/// bound to its dialect (in the signature too). A shared arm calls the right one with
/// `q::pick($pg, $lite)(...)`; methods work the same way (`q::pick(Self::$pg, Self::$lite)`).
macro_rules! dialect_fn {
    ([$pg:ident, $lite:ident] $(#[$m:meta])* $vis:vis async fn ($($params:tt)*) $(-> $ret:ty)? $body:block) => {
        $crate::database::dialect::dialect_fn!(@one $pg, pg, [$(#[$m])*] $vis [async] ($($params)*) [$($ret)?] $body);
        $crate::database::dialect::dialect_fn!(@one $lite, lite, [$(#[$m])*] $vis [async] ($($params)*) [$($ret)?] $body);
    };
    ([$pg:ident, $lite:ident] $(#[$m:meta])* $vis:vis fn ($($params:tt)*) $(-> $ret:ty)? $body:block) => {
        $crate::database::dialect::dialect_fn!(@one $pg, pg, [$(#[$m])*] $vis [] ($($params)*) [$($ret)?] $body);
        $crate::database::dialect::dialect_fn!(@one $lite, lite, [$(#[$m])*] $vis [] ($($params)*) [$($ret)?] $body);
    };
    (@one $name:ident, $d:ident, [$($attr:tt)*] $vis:vis [$($async:tt)?] ($($params:tt)*) [$($ret:ty)?] $body:block) => {
        #[allow(non_snake_case, unused_imports)]
        mod $name {
            use super::*;
            use $crate::database::dialect::$d as q;
            $($attr)*
            pub $($async)? fn $name($($params)*) $(-> $ret)? $body
        }
        #[allow(unused_imports)]
        $vis use self::$name::$name;
    };
}
pub(crate) use dialect_fn;

/// [`dialect_fn!`] for methods on `DatabasePool`.
macro_rules! dialect_method {
    ([$pg:ident, $lite:ident] $(#[$m:meta])* $vis:vis async fn ($($params:tt)*) $(-> $ret:ty)? $body:block) => {
        $crate::database::dialect::dialect_method!(@one $pg, pg, [$(#[$m])*] ($($params)*) [$($ret)?] $body);
        $crate::database::dialect::dialect_method!(@one $lite, lite, [$(#[$m])*] ($($params)*) [$($ret)?] $body);
    };
    (@one $name:ident, $d:ident, [$($attr:tt)*] ($($params:tt)*) [$($ret:ty)?] $body:block) => {
        #[allow(non_snake_case, unused_imports)]
        mod $name {
            use super::*;
            use $crate::database::dialect::$d as q;
            impl DatabasePool {
                $($attr)*
                pub(crate) async fn $name($($params)*) $(-> $ret)? $body
            }
        }
    };
}
pub(crate) use dialect_method;

/// PostgreSQL: plain sqlx.
pub mod pg {
    use sqlx::postgres::{PgArguments, PgRow};
    use sqlx::{FromRow, Postgres};

    pub type Db = Postgres;
    pub type Row = PgRow;
    pub type Conn = sqlx::PgConnection;
    pub type Tx<'c> = sqlx::Transaction<'c, Postgres>;
    pub type Pool = sqlx::Pool<Postgres>;

    pub fn query<'q>(sql: impl sqlx::SqlSafeStr) -> sqlx::query::Query<'q, Postgres, PgArguments> {
        sqlx::query(sql)
    }

    pub fn query_as<'q, O>(sql: impl sqlx::SqlSafeStr) -> sqlx::query::QueryAs<'q, Postgres, O, PgArguments>
    where
        O: for<'r> FromRow<'r, PgRow>,
    {
        sqlx::query_as(sql)
    }

    pub fn query_scalar<'q, O>(sql: impl sqlx::SqlSafeStr) -> sqlx::query::QueryScalar<'q, Postgres, O, PgArguments>
    where
        (O,): for<'r> FromRow<'r, PgRow>,
    {
        sqlx::query_scalar(sql)
    }

    pub async fn begin(pool: &Pool) -> Result<Tx<'static>, sqlx::Error> {
        pool.begin().await
    }

    /// The driver's own row, for helpers generic over `sqlx::Row`.
    pub fn raw(row: &Row) -> &Row {
        row
    }

    /// Of a PostgreSQL / SQLite pair of helpers, the PostgreSQL one.
    pub fn pick<A, B>(pg: A, _lite: B) -> A {
        pg
    }

    /// An `int[]` column.
    pub fn int_array(row: &Row, col: &str) -> Result<Option<Vec<i32>>, sqlx::Error> {
        use sqlx::Row as _;
        row.try_get(col)
    }

    /// A `numeric` column as a float.
    pub fn decimal(row: &Row, col: &str) -> Result<Option<f64>, sqlx::Error> {
        use bigdecimal::ToPrimitive;
        use sqlx::Row as _;
        Ok(row.try_get::<Option<bigdecimal::BigDecimal>, _>(col)?.and_then(|d| d.to_f64()))
    }
}

/// SQLite: PostgreSQL text translated, values bound as the SQLite schema stores them.
pub mod lite {
    use std::marker::PhantomData;

    use sqlx::error::BoxDynError;
    use sqlx::sqlite::{SqliteArguments, SqliteQueryResult, SqliteRow};
    use sqlx::{Arguments as _, Executor, FromRow, Sqlite};

    pub type Db = Sqlite;
    pub type Row = LiteRow;
    pub type Conn = sqlx::SqliteConnection;
    pub type Tx<'c> = LiteTx<'c>;
    pub type Pool = sqlx::Pool<Sqlite>;

    /// A write transaction holding its database's writer lock (see [`begin`]). Derefs to the
    /// connection, like `sqlx::Transaction`, so `&mut *tx` is an executor.
    pub struct LiteTx<'c> {
        tx: sqlx::Transaction<'c, Sqlite>,
        _writer: Option<tokio::sync::OwnedMutexGuard<()>>,
    }

    impl<'c> LiteTx<'c> {
        pub async fn commit(self) -> Result<(), sqlx::Error> {
            self.tx.commit().await
        }

        pub async fn rollback(self) -> Result<(), sqlx::Error> {
            self.tx.rollback().await
        }
    }

    impl std::ops::Deref for LiteTx<'_> {
        type Target = sqlx::SqliteConnection;
        fn deref(&self) -> &sqlx::SqliteConnection {
            &self.tx
        }
    }

    impl std::ops::DerefMut for LiteTx<'_> {
        fn deref_mut(&mut self) -> &mut sqlx::SqliteConnection {
            &mut self.tx
        }
    }

    /// One writer lock per database file.
    fn writer_lock(pool: &Pool) -> std::sync::Arc<tokio::sync::Mutex<()>> {
        static LOCKS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<std::path::PathBuf, std::sync::Arc<tokio::sync::Mutex<()>>>>> =
            std::sync::LazyLock::new(Default::default);
        let path = pool.connect_options().get_filename().to_path_buf();
        LOCKS.lock().unwrap().entry(path).or_default().clone()
    }

    /// A SQLite row that, like PostgreSQL, refuses to decode NULL into a non-`Option` type
    /// (sqlx's SQLite driver would hand back `""` or `0`, and code relies on the error, e.g.
    /// `try_get::<String, _>(..).ok()`). Everything else goes through to the `SqliteRow`.
    pub struct LiteRow(pub SqliteRow);

    impl std::ops::Deref for LiteRow {
        type Target = SqliteRow;
        fn deref(&self) -> &SqliteRow {
            &self.0
        }
    }

    impl LiteRow {
        pub fn try_get<'r, T, I>(&'r self, index: I) -> Result<T, sqlx::Error>
        where
            I: sqlx::ColumnIndex<SqliteRow> + std::fmt::Debug,
            T: sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite>,
        {
            use sqlx::{Row as _, ValueRef as _};
            let raw = self.0.try_get_raw(&index)?;
            if raw.is_null() && !std::any::type_name::<T>().starts_with("core::option::Option<") {
                return Err(sqlx::Error::ColumnDecode { index: format!("{index:?}"), source: Box::new(sqlx::error::UnexpectedNullError) });
            }
            self.0.try_get(index)
        }

        pub fn get<'r, T, I>(&'r self, index: I) -> T
        where
            I: sqlx::ColumnIndex<SqliteRow> + std::fmt::Debug,
            T: sqlx::Decode<'r, Sqlite> + sqlx::Type<Sqlite>,
        {
            self.try_get(index).unwrap_or_else(|e| panic!("{e}"))
        }
    }

    /// A query on its way to SQLite. Mirrors the parts of `sqlx::query::Query` the code uses.
    #[must_use = "query must be executed to affect database"]
    pub struct Query {
        sql: String,
        args: SqliteArguments,
        err: Option<BoxDynError>,
    }

    pub fn query(sql: impl sqlx::SqlSafeStr) -> Query {
        Query { sql: super::translate(sql.into_sql_str().as_str()), args: SqliteArguments::default(), err: None }
    }

    impl Query {
        pub fn bind<T: Bind>(mut self, value: T) -> Self {
            if self.err.is_none() {
                if let Err(e) = value.add_to(&mut self.args) {
                    self.err = Some(e);
                }
            }
            self
        }

        fn finish(self) -> Result<(String, sqlx::query::Query<'static, Sqlite, SqliteArguments>), sqlx::Error> {
            match self.err {
                Some(e) => Err(sqlx::Error::Encode(e)),
                None => Ok((self.sql.clone(), sqlx::query_with(sqlx::AssertSqlSafe(self.sql), self.args))),
            }
        }

        pub async fn execute<'e, 'c: 'e, E>(self, executor: E) -> Result<SqliteQueryResult, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            let (sql, q) = self.finish()?;
            q.execute(executor).await.inspect_err(|e| log_failure(&sql, e))
        }

        pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<LiteRow, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            let (sql, q) = self.finish()?;
            q.fetch_one(executor).await.map(LiteRow).inspect_err(|e| log_failure(&sql, e))
        }

        pub async fn fetch_optional<'e, 'c: 'e, E>(self, executor: E) -> Result<Option<LiteRow>, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            let (sql, q) = self.finish()?;
            q.fetch_optional(executor).await.map(|r| r.map(LiteRow)).inspect_err(|e| log_failure(&sql, e))
        }

        pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<LiteRow>, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            let (sql, q) = self.finish()?;
            q.fetch_all(executor).await.map(|v| v.into_iter().map(LiteRow).collect()).inspect_err(|e| log_failure(&sql, e))
        }
    }

    /// `query_as::<T>`: rows mapped through `FromRow`.
    pub struct QueryAs<O> {
        inner: Query,
        _o: PhantomData<O>,
    }

    pub fn query_as<O>(sql: impl sqlx::SqlSafeStr) -> QueryAs<O>
    where
        O: for<'r> FromRow<'r, SqliteRow>,
    {
        QueryAs { inner: query(sql), _o: PhantomData }
    }

    impl<O> QueryAs<O>
    where
        O: for<'r> FromRow<'r, SqliteRow> + Send + Unpin,
    {
        pub fn bind<T: Bind>(self, value: T) -> Self {
            QueryAs { inner: self.inner.bind(value), _o: PhantomData }
        }

        pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<O, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            O::from_row(&self.inner.fetch_one(executor).await?.0)
        }

        pub async fn fetch_optional<'e, 'c: 'e, E>(self, executor: E) -> Result<Option<O>, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            self.inner.fetch_optional(executor).await?.map(|r| O::from_row(&r.0)).transpose()
        }

        pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            self.inner.fetch_all(executor).await?.iter().map(|r| O::from_row(&r.0)).collect()
        }
    }

    /// `query_scalar::<T>`: the first column of each row.
    pub struct QueryScalar<O> {
        inner: Query,
        _o: PhantomData<O>,
    }

    pub fn query_scalar<O>(sql: impl sqlx::SqlSafeStr) -> QueryScalar<O>
    where
        (O,): for<'r> FromRow<'r, SqliteRow>,
    {
        QueryScalar { inner: query(sql), _o: PhantomData }
    }

    impl<O> QueryScalar<O>
    where
        (O,): for<'r> FromRow<'r, SqliteRow>,
        O: Send + Unpin,
    {
        pub fn bind<T: Bind>(self, value: T) -> Self {
            QueryScalar { inner: self.inner.bind(value), _o: PhantomData }
        }

        fn first(row: &SqliteRow) -> Result<O, sqlx::Error> {
            <(O,)>::from_row(row).map(|(o,)| o)
        }

        pub async fn fetch_one<'e, 'c: 'e, E>(self, executor: E) -> Result<O, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            Self::first(&self.inner.fetch_one(executor).await?.0)
        }

        pub async fn fetch_optional<'e, 'c: 'e, E>(self, executor: E) -> Result<Option<O>, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            self.inner.fetch_optional(executor).await?.map(|r| Self::first(&r.0)).transpose()
        }

        pub async fn fetch_all<'e, 'c: 'e, E>(self, executor: E) -> Result<Vec<O>, sqlx::Error>
        where
            E: 'e + Executor<'c, Database = Sqlite>,
        {
            self.inner.fetch_all(executor).await?.iter().map(|r| Self::first(&r.0)).collect()
        }
    }

    /// Translated text of a failed statement, for `PINEPODS_LOG`-style debugging.
    fn log_failure(sql: &str, e: &sqlx::Error) {
        if matches!(e, sqlx::Error::Database(_)) {
            tracing::debug!("SQLite query failed: {e}\n{sql}");
        }
    }

    /// SQLite has one writer at a time. Write transactions queue here first, without holding
    /// a pool connection (otherwise waiting writers can take every connection while the
    /// writer needs one more), then take SQLite's write lock up front with BEGIN IMMEDIATE so
    /// none fails upgrading from reading to writing. A transaction opened while this task
    /// already holds the lock goes ahead after a while instead of waiting forever.
    pub async fn begin(pool: &Pool) -> Result<Tx<'static>, sqlx::Error> {
        let writer = lock_writer(pool).await;
        Ok(LiteTx { tx: pool.begin_with("BEGIN IMMEDIATE").await?, _writer: writer })
    }

    /// Wait for the database's writer lock, for work that manages its own transaction.
    pub async fn lock_writer(pool: &Pool) -> Option<tokio::sync::OwnedMutexGuard<()>> {
        let lock = writer_lock(pool);
        match tokio::time::timeout(std::time::Duration::from_secs(30), lock.lock_owned()).await {
            Ok(guard) => Some(guard),
            Err(_) => {
                tracing::warn!("SQLite writer lock still held after 30s; starting the transaction anyway");
                None
            }
        }
    }

    /// The driver's own row, for helpers generic over `sqlx::Row` (they decode NULL the
    /// SQLite way).
    pub fn raw(row: &Row) -> &SqliteRow {
        &row.0
    }

    /// Of a PostgreSQL / SQLite pair of helpers, the SQLite one.
    pub fn pick<A, B>(_pg: A, lite: B) -> B {
        lite
    }

    /// An `int[]` column, which SQLite stores as a JSON array.
    pub fn int_array(row: &Row, col: &str) -> Result<Option<Vec<i32>>, sqlx::Error> {
        match row.try_get::<Option<String>, _>(col)? {
            None => Ok(None),
            Some(s) => serde_json::from_str(&s).map(Some).map_err(|e| sqlx::Error::Decode(Box::new(e))),
        }
    }

    /// A `numeric` column as a float.
    pub fn decimal(row: &Row, col: &str) -> Result<Option<f64>, sqlx::Error> {
        match row.try_get::<Option<f64>, _>(col) {
            Ok(v) => Ok(v),
            Err(_) => Ok(row.try_get::<Option<i64>, _>(col)?.map(|v| v as f64)),
        }
    }

    /// A value bound to a SQLite query. PostgreSQL arrays go in as JSON arrays (what the
    /// translated `= ANY(...)` reads, and how array columns are stored), timestamps as naive
    /// UTC text so they compare correctly with `CURRENT_TIMESTAMP`, numerics as floats.
    pub trait Bind {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError>;
    }

    macro_rules! direct {
        ($($t:ty),*) => {$(
            impl Bind for $t {
                fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
                    args.add(self)
                }
            }
        )*};
    }
    direct!(bool, i8, i16, i32, i64, u8, u16, u32, f32, f64, String, &str, chrono::NaiveDate, chrono::NaiveTime, sqlx::types::Uuid);

    impl Bind for u64 {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            args.add(i64::try_from(self)?)
        }
    }

    impl Bind for usize {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            args.add(i64::try_from(self)?)
        }
    }

    impl Bind for chrono::NaiveDateTime {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            args.add(self.format("%Y-%m-%d %H:%M:%S%.f").to_string())
        }
    }

    impl<Tz: chrono::TimeZone> Bind for chrono::DateTime<Tz> {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            self.naive_utc().add_to(args)
        }
    }

    impl Bind for bigdecimal::BigDecimal {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            use bigdecimal::ToPrimitive;
            args.add(self.to_f64())
        }
    }

    impl Bind for serde_json::Value {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            args.add(self.to_string())
        }
    }

    impl<T: serde::Serialize> Bind for sqlx::types::Json<T> {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            args.add(serde_json::to_string(&self.0)?)
        }
    }

    /// An element of a PostgreSQL array bind, as it goes into the JSON array.
    pub trait JsonElem {
        fn json(&self) -> serde_json::Value;
    }

    macro_rules! json_elem {
        ($($t:ty),*) => {$(
            impl JsonElem for $t {
                fn json(&self) -> serde_json::Value {
                    serde_json::json!(self)
                }
            }
        )*};
    }
    json_elem!(i16, i32, i64, f64, bool, String, &str);

    impl JsonElem for chrono::NaiveDateTime {
        fn json(&self) -> serde_json::Value {
            serde_json::Value::String(self.format("%Y-%m-%d %H:%M:%S%.f").to_string())
        }
    }

    impl<Tz: chrono::TimeZone> JsonElem for chrono::DateTime<Tz> {
        fn json(&self) -> serde_json::Value {
            self.naive_utc().json()
        }
    }

    impl<T: JsonElem> JsonElem for Option<T> {
        fn json(&self) -> serde_json::Value {
            self.as_ref().map_or(serde_json::Value::Null, JsonElem::json)
        }
    }

    impl<T: JsonElem> Bind for Vec<T> {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            self.as_slice().add_to(args)
        }
    }

    impl<T: JsonElem> Bind for &[T] {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            args.add(serde_json::Value::Array(self.iter().map(JsonElem::json).collect()).to_string())
        }
    }

    impl<T: Bind> Bind for Option<T> {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            match self {
                Some(v) => v.add_to(args),
                None => args.add(Option::<i64>::None),
            }
        }
    }

    impl<T: Bind + Clone> Bind for &T {
        fn add_to(self, args: &mut SqliteArguments) -> Result<(), BoxDynError> {
            self.clone().add_to(args)
        }
    }
}

// ---------------------------------------------------------------------------------------------
// PostgreSQL -> SQLite query text
// ---------------------------------------------------------------------------------------------

/// Rewrite PostgreSQL query text for SQLite. Both accept `$n` placeholders, double-quoted
/// identifiers, `RETURNING`, `ON CONFLICT`, `FILTER`, window functions and `IS DISTINCT FROM`;
/// this handles what SQLite lacks: `::` casts, `NOW()` and interval arithmetic,
/// `EXTRACT(EPOCH ...)`, `= ANY(array)`, `ILIKE`, `GREATEST`/`LEAST`, `FOR UPDATE` and
/// `TRUNCATE`.
pub fn translate(pg: &str) -> String {
    // Most query text is static, so the same strings come back again and again.
    static CACHE: LazyLock<std::sync::Mutex<std::collections::HashMap<String, String>>> = LazyLock::new(Default::default);
    if let Some(hit) = CACHE.lock().unwrap().get(pg) {
        return hit.clone();
    }
    let out = translate_uncached(pg);
    let mut cache = CACHE.lock().unwrap();
    if cache.len() >= 4096 {
        cache.clear();
    }
    cache.insert(pg.to_string(), out.clone());
    out
}

fn translate_uncached(pg: &str) -> String {
    let (masked, literals) = mask_literals(pg);
    let mut s = rewrite_casts(&masked);
    s = strip_time_zones(&s);
    s = rewrite_epoch(&s);
    s = NOW_RE.replace_all(&s, "strftime('%Y-%m-%d %H:%M:%f', 'now')").into_owned();
    s = rewrite_intervals(&s, &literals);
    s = rewrite_make_interval(&s);
    s = rewrite_call(&s, &STRING_TO_ARRAY_CALL, |a| format!("('[\"' || replace({}, {}, '\",\"') || '\"]')", a[0], a[1]));
    s = rewrite_call(&s, &RIGHT_CALL, |a| format!("substr({}, -({}))", a[0], a[1]));
    s = rewrite_call(&s, &TO_CHAR_CALL, |a| format!("strftime({}, {})", to_char_format(&a[1], &literals), a[0]));
    s = rewrite_any(&s);
    s = rewrite_unnest(&s);
    s = rewrite_call(&s, &FLOOR_CALL, |a| {
        let x = &a[0];
        format!("(CASE WHEN ({x}) < CAST(({x}) AS INTEGER) THEN CAST(({x}) AS INTEGER) - 1 ELSE CAST(({x}) AS INTEGER) END)")
    });
    s = rewrite_call(&s, &SPLIT_PART_CALL, |a| {
        let (x, d) = (&a[0], &a[1]);
        format!("(CASE WHEN instr({x}, {d}) > 0 THEN substr({x}, 1, instr({x}, {d}) - 1) ELSE {x} END)")
    });
    s = BOOL_OR_RE.replace_all(&s, "MAX(").into_owned();
    s = BOOL_AND_RE.replace_all(&s, "MIN(").into_owned();
    s = upsert_select_where(&s);
    s = TARGET_ALIAS_RE.replace_all(&s, "$1 $2 AS $3 $4").into_owned();
    s = unwrap_compound_members(&s);
    s = ILIKE_RE.replace_all(&s, "LIKE").into_owned();
    s = GREATEST_RE.replace_all(&s, "MAX(").into_owned();
    s = LEAST_RE.replace_all(&s, "MIN(").into_owned();
    s = FOR_UPDATE_RE.replace_all(&s, "").into_owned();
    s = TRUNCATE_RE
        .replace_all(&s, |c: &regex::Captures| format!("DELETE FROM {}", c[1].trim()))
        .into_owned();
    unmask_literals(&s, &literals)
}

/// A `TO_CHAR` date format (a masked literal) as an `strftime` format literal.
fn to_char_format(fmt: &str, lits: &[String]) -> String {
    let Some(idx) = fmt.trim().strip_prefix('\x01').and_then(|r| r.strip_suffix('\x02')).and_then(|n| n.parse::<usize>().ok()) else {
        return fmt.to_string();
    };
    let pg = lits[idx].trim_matches('\'');
    let mut out = String::new();
    let mut rest = pg;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('"') {
            let end = r.find('"').unwrap_or(r.len());
            out.push_str(&r[..end]);
            rest = r.get(end + 1..).unwrap_or("");
            continue;
        }
        let (tok, n) = [("YYYY", "%Y"), ("HH24", "%H"), ("MM", "%m"), ("DD", "%d"), ("MI", "%M"), ("SS", "%S"), ("MS", "%f")]
            .iter()
            .find(|(t, _)| rest.starts_with(t))
            .map(|(t, f)| (*f, t.len()))
            .unwrap_or_else(|| (&rest[..rest.chars().next().unwrap().len_utf8()], rest.chars().next().unwrap().len_utf8()));
        out.push_str(tok);
        rest = &rest[n..];
    }
    format!("'{out}'")
}

static NOW_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(NOW|CLOCK_TIMESTAMP)\(\)").unwrap());
static BOOL_OR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bBOOL_OR\s*\(").unwrap());
static BOOL_AND_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bBOOL_AND\s*\(").unwrap());
static ILIKE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bILIKE\b").unwrap());
static GREATEST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bGREATEST\s*\(").unwrap());
static LEAST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bLEAST\s*\(").unwrap());
static FOR_UPDATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\s+FOR\s+(?:NO\s+KEY\s+UPDATE|UPDATE|KEY\s+SHARE|SHARE)(?:\s+OF\s+[\w"]+(?:\s*,\s*[\w"]+)*)?(?:\s+(?:NOWAIT|SKIP\s+LOCKED))?\b"#).unwrap()
});
static TRUNCATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\bTRUNCATE\s+(?:TABLE\s+)?([\w"., ]+?)(?:\s+RESTART\s+IDENTITY)?(?:\s+CASCADE)?\s*(;|$)"#).unwrap()
});

/// Placeholder for the n-th string literal: `\x01n\x02`. Keeps literal text away from the
/// rewrites and makes a literal one operand token.
fn mask_literals(sql: &str) -> (String, Vec<String>) {
    let mut out = String::with_capacity(sql.len());
    let mut lits = Vec::new();
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\'' {
            let mut lit = String::from("'");
            while let Some(d) = chars.next() {
                lit.push(d);
                if d == '\'' {
                    if chars.peek() == Some(&'\'') {
                        lit.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
            }
            out.push('\x01');
            out.push_str(&lits.len().to_string());
            out.push('\x02');
            lits.push(lit);
        } else {
            out.push(c);
        }
    }
    (out, lits)
}

fn unmask_literals(s: &str, lits: &[String]) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('\x01') {
        out.push_str(&rest[..i]);
        let j = rest[i..].find('\x02').unwrap() + i;
        out.push_str(&lits[rest[i + 1..j].parse::<usize>().unwrap()]);
        rest = &rest[j + 1..];
    }
    out.push_str(rest);
    out
}

/// Start of the operand that ends at `end` (exclusive): a parenthesised group (with its
/// function name), a masked literal, or an identifier / qualified name / placeholder / number.
fn operand_start(s: &[u8], end: usize) -> usize {
    let mut i = end;
    if i > 0 && s[i - 1] == b')' {
        let mut depth = 0;
        while i > 0 {
            i -= 1;
            match s[i] {
                b')' => depth += 1,
                b'(' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ => {}
            }
        }
    } else if i > 0 && s[i - 1] == b'\x02' {
        while i > 0 && s[i - 1] != b'\x01' {
            i -= 1;
        }
        return i - 1;
    }
    while i > 0 && (s[i - 1].is_ascii_alphanumeric() || matches!(s[i - 1], b'_' | b'.' | b'"' | b'$')) {
        i -= 1;
    }
    // `CASE ... END`: the operand reaches back to the matching CASE.
    if s[i..end].eq_ignore_ascii_case(b"END") {
        let text = std::str::from_utf8(&s[..i]).unwrap_or_default().to_ascii_uppercase();
        let mut depth = 1;
        let words: Vec<(usize, &str)> = CASE_WORD_RE.find_iter(&text).map(|m| (m.start(), m.as_str())).collect();
        for (pos, w) in words.into_iter().rev() {
            depth += if w == "END" { 1 } else { -1 };
            if depth == 0 {
                return pos;
            }
        }
    }
    i
}

static CASE_WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(CASE|END)\b").unwrap());

/// End (exclusive) of the parenthesised group opening at `open`.
fn group_end(s: &[u8], open: usize) -> usize {
    let mut depth = 0;
    for (i, &c) in s.iter().enumerate().skip(open) {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    s.len()
}

static CAST_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^::\s*(double precision|character varying|timestamp with(?:out)? time zone|[a-z_][a-z0-9_]*)(\s*\(\s*\d+(?:\s*,\s*\d+)?\s*\))?(\[\])?",
    )
    .unwrap()
});

/// `x::type` -> `CAST(x AS ...)` where SQLite's result differs, otherwise just `x`.
fn rewrite_casts(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("::") {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        let Some(m) = CAST_RE.captures(tail) else {
            out.push_str("::");
            rest = &rest[i + 2..];
            continue;
        };
        let ty = m[1].to_ascii_lowercase();
        let is_array = m.get(3).is_some();
        let start = operand_start(out.as_bytes(), out.len());
        let operand = out[start..].to_string();
        out.truncate(start);
        let replaced = if is_array {
            operand
        } else {
            match ty.as_str() {
                "int" | "integer" | "int4" | "int8" | "bigint" | "smallint" | "int2" => {
                    format!("CAST(ROUND({operand}) AS INTEGER)")
                }
                "float" | "float4" | "float8" | "double precision" | "real" | "numeric" | "decimal" => {
                    format!("CAST({operand} AS REAL)")
                }
                "text" | "varchar" | "character varying" | "char" | "bpchar" => format!("CAST({operand} AS TEXT)"),
                "date" => format!("date({operand})"),
                _ => operand, // timestamps, booleans, json: SQLite stores them as they are
            }
        };
        out.push_str(&replaced);
        rest = &tail[m.get(0).unwrap().end()..];
    }
    out.push_str(rest);
    out
}

static EPOCH_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bEXTRACT\s*\(\s*EPOCH\s+FROM\s+").unwrap());
static EXTRACT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\bEXTRACT\s*\(\s*(YEAR|MONTH|DAY|HOUR|MINUTE|SECOND|DOW)\s+FROM\s+").unwrap());

/// `EXTRACT(EPOCH FROM x)` -> seconds since 1970 as a float, like PostgreSQL.
fn rewrite_epoch(s: &str) -> String {
    let mut out = s.to_string();
    while let Some(m) = EPOCH_RE.find(&out) {
        let open = out[m.start()..].find('(').unwrap() + m.start();
        let end = group_end(out.as_bytes(), open);
        let inner = out[m.end()..end - 1].trim().to_string();
        let inner = if NOW_RE.is_match(&inner) { "'now'".to_string() } else { inner };
        out.replace_range(m.start()..end, &format!("unixepoch({inner}, 'subsec')"));
    }
    while let Some(c) = EXTRACT_RE.captures(&out) {
        let (start, after) = (c.get(0).unwrap().start(), c.get(0).unwrap().end());
        let field = c[1].to_ascii_uppercase();
        let open = out[start..].find('(').unwrap() + start;
        let end = group_end(out.as_bytes(), open);
        let inner = out[after..end - 1].trim().to_string();
        let f = match field.as_str() {
            "YEAR" => "%Y",
            "MONTH" => "%m",
            "DAY" => "%d",
            "HOUR" => "%H",
            "MINUTE" => "%M",
            "SECOND" => "%S",
            _ => "%w",
        };
        out.replace_range(start..end, &format!("CAST(strftime('{f}', {inner}) AS INTEGER)"));
    }
    out
}

static INTERVAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)([-+])\s*(\(\s*)?INTERVAL\s*\x01(\d+)\x02(?:\s+(YEAR|MONTH|DAY|HOUR|MINUTE|SECOND)S?\b)?(\s*\*\s*)?").unwrap()
});

static AT_TIME_ZONE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\s+AT\s+TIME\s+ZONE\s+").unwrap());

/// Drop `AT TIME ZONE zone`: SQLite has no time zones, and timestamps are stored in UTC. The
/// queries convert both sides of a comparison to the same zone, so dropping it keeps them
/// equal (bar a DST change inside the window).
fn strip_time_zones(s: &str) -> String {
    let mut out = s.to_string();
    while let Some((start, after)) = AT_TIME_ZONE_RE.find(&out).map(|m| (m.start(), m.end())) {
        let b = out.as_bytes();
        let mut end = after;
        if b.get(end) == Some(&b'\x01') {
            end += out[end..].find('\x02').unwrap() + 1;
        } else {
            while end < b.len() && (b[end].is_ascii_alphanumeric() || matches!(b[end], b'_' | b'.' | b'"' | b'$')) {
                end += 1;
            }
            if b.get(end) == Some(&b'(') {
                end = group_end(b, end);
            }
        }
        out.replace_range(start..end, "");
    }
    out
}

/// `base - INTERVAL 'n unit'` -> `datetime(base, '-n unit')`, and
/// `base - (INTERVAL 'n unit' * (k))` -> `datetime(base, '-' || ((k) * n) || ' unit')`.
fn rewrite_intervals(s: &str, lits: &[String]) -> String {
    let mut out = s.to_string();
    while let Some(c) = INTERVAL_RE.captures(&out) {
        let whole = c.get(0).unwrap();
        let sign = c[1].to_string();
        let paren = c.get(2).is_some();
        let lit = &lits[c[3].parse::<usize>().unwrap()];
        let lit = lit.trim_matches('\'').trim();
        let (n, unit) = match c.get(4) {
            Some(u) => (lit.to_string(), u.as_str().to_ascii_lowercase()),
            None => lit.split_once(' ').map(|(n, u)| (n.to_string(), u.trim().to_string())).unwrap_or(("0".into(), "seconds".into())),
        };
        let base_end = out[..whole.start()].trim_end().len();
        let base_start = operand_start(out.as_bytes(), base_end);
        let base = out[base_start..base_end].to_string();
        let (modifier, end) = if c.get(5).is_some() {
            let after = whole.end();
            let mult_end = if out.as_bytes().get(after) == Some(&b'(') {
                group_end(out.as_bytes(), after)
            } else {
                after + out[after..].find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' || ch == '.')).unwrap_or(out.len() - after)
            };
            let mult = out[after..mult_end].to_string();
            let close = if paren { out[mult_end..].find(')').map(|k| mult_end + k + 1).unwrap_or(mult_end) } else { mult_end };
            let neg = if sign == "-" { "-" } else { "" };
            (format!("({neg}(({mult}) * {n})) || ' {unit}'"), close)
        } else {
            let close = if paren { out[whole.end()..].find(')').map(|k| whole.end() + k + 1).unwrap_or(whole.end()) } else { whole.end() };
            (format!("'{sign}{n} {unit}'"), close)
        };
        out.replace_range(base_start..end, &format!("datetime({base}, {modifier})"));
    }
    out
}

static ANY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(=|<>|!=)\s*(ANY|ALL)\s*\(").unwrap());

/// `x = ANY(arr)` -> `x IN (SELECT value FROM json_each(arr))`; `x <> ALL(arr)` -> `NOT IN`.
fn rewrite_any(s: &str) -> String {
    let mut out = s.to_string();
    while let Some(c) = ANY_RE.captures(&out) {
        let whole = c.get(0).unwrap();
        let open = whole.end() - 1;
        let end = group_end(out.as_bytes(), open);
        let inner = out[open + 1..end - 1].trim().to_string();
        let negate = c[1] != *"=" && c[2].eq_ignore_ascii_case("ALL");
        let kw = if negate { "NOT IN" } else { "IN" };
        out.replace_range(whole.start()..end, &format!("{kw} (SELECT value FROM json_each({inner}))"));
    }
    out
}

/// Split the comma-separated arguments of the call whose `(` is at `open`.
fn call_args(s: &str, open: usize) -> (Vec<String>, usize) {
    let end = group_end(s.as_bytes(), open);
    let inner = &s[open + 1..end - 1];
    let (mut args, mut depth, mut cur) = (Vec::new(), 0i32, String::new());
    for ch in inner.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(cur.trim().to_string());
                cur.clear();
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    args.push(cur.trim().to_string());
    (args, end)
}

macro_rules! call_re {
    ($($name:ident = $f:literal),*) => {$(
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new(concat!(r"(?i)\b", $f, r"\s*\(")).unwrap());
    )*};
}
call_re!(FLOOR_CALL = "FLOOR", SPLIT_PART_CALL = "split_part", STRING_TO_ARRAY_CALL = "string_to_array", RIGHT_CALL = "RIGHT", TO_CHAR_CALL = "TO_CHAR");

/// Replace every call `re` matches (`name(`) with `f(args)`.
fn rewrite_call(s: &str, re: &Regex, f: impl Fn(&[String]) -> String) -> String {
    if !re.is_match(s) {
        return s.to_string();
    }
    let mut out = s.to_string();
    let mut from = 0;
    while let Some((start, open)) = re.find_at(&out, from).map(|m| (m.start(), m.end() - 1)) {
        let (args, end) = call_args(&out, open);
        let new = f(&args);
        out.replace_range(start..end, &new);
        from = start + new.len();
    }
    out
}

static MAKE_INTERVAL_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)([-+])\s*make_interval\s*\(").unwrap());

/// `base + make_interval(secs => x)` -> `datetime(base, (x) || ' seconds')`.
fn rewrite_make_interval(s: &str) -> String {
    let mut out = s.to_string();
    while let Some(c) = MAKE_INTERVAL_RE.captures(&out) {
        let whole = c.get(0).unwrap();
        let neg = &c[1] == "-";
        let (args, end) = call_args(&out, whole.end() - 1);
        let (unit, value) = args[0].split_once("=>").map(|(u, v)| (u.trim().to_ascii_lowercase(), v.trim().to_string())).unwrap_or(("secs".into(), args[0].clone()));
        let unit = match unit.as_str() {
            "secs" => "seconds",
            "mins" => "minutes",
            other => other,
        };
        let base_end = out[..whole.start()].trim_end().len();
        let base_start = operand_start(out.as_bytes(), base_end);
        let base = out[base_start..base_end].to_string();
        let value = if neg { format!("-({value})") } else { format!("({value})") };
        out.replace_range(base_start..end, &format!("datetime({base}, {value} || ' {unit}')"));
    }
    out
}

static UNNEST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bUNNEST\s*\(").unwrap());
static UNNEST_ALIAS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(WITH\s+ORDINALITY\s+)?AS\s+(\w+)\s*(?:\(([^)]*)\))?").unwrap()
});

/// `UNNEST(a, b) [WITH ORDINALITY] AS t(x, y[, n])` -> the JSON arrays zipped by index.
fn rewrite_unnest(s: &str) -> String {
    let mut out = s.to_string();
    while let Some(m) = UNNEST_RE.find(&out) {
        let (arrays, end) = call_args(&out, m.end() - 1);
        let Some(alias) = UNNEST_ALIAS_RE.captures(&out[end..]) else {
            // `SELECT a, UNNEST(arr) [WHERE ...]`: a set-returning last select item.
            let new = format!("unnest_j.value FROM json_each({}) AS unnest_j", arrays[0]);
            out.replace_range(m.start()..end, &new);
            continue;
        };
        let ordinality = alias.get(1).is_some();
        let name = alias[2].to_string();
        let mut cols: Vec<String> = alias.get(3).map(|c| c.as_str().split(',').map(|x| x.trim().to_string()).collect()).unwrap_or_default();
        while cols.len() < arrays.len() {
            cols.push(format!("c{}", cols.len() + 1));
        }
        // Walk the first array; take the others by index (a join on json_each keys gets
        // planned as a cross product).
        let mut select: Vec<String> = arrays
            .iter()
            .enumerate()
            .map(|(i, a)| if i == 0 { format!("j0.value AS {}", cols[0]) } else { format!("({a} ->> j0.key) AS {}", cols[i]) })
            .collect();
        if ordinality {
            select.push(format!("j0.key + 1 AS {}", cols.get(arrays.len()).cloned().unwrap_or_else(|| "ordinality".into())));
        }
        let from = format!("json_each({}) j0", arrays[0]);
        let alias_end = end + alias.get(0).unwrap().end();
        out.replace_range(m.start()..alias_end, &format!("(SELECT {} FROM {from}) AS {name} ", select.join(", ")));
    }
    out
}

static TARGET_ALIAS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)\b(UPDATE|DELETE\s+FROM)\s+("[^"]+"|\w+)\s+([a-z_]\w*)\s+(SET|WHERE|USING)\b"#).unwrap()
});

static SET_OP_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(UNION|INTERSECT|EXCEPT)\b").unwrap());
static SET_OP_BEFORE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\b(UNION(\s+ALL)?|INTERSECT|EXCEPT)\s*$").unwrap());
static PAREN_SELECT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\(\s*SELECT\b").unwrap());

/// SQLite takes no parentheses around the members of a UNION: `(SELECT ...) UNION ALL
/// (SELECT ...)` becomes `SELECT * FROM (SELECT ...) UNION ALL SELECT * FROM (SELECT ...)`,
/// which keeps each member's own ORDER BY / LIMIT.
fn unwrap_compound_members(s: &str) -> String {
    let mut out = s.to_string();
    let mut from = 0;
    while let Some(open) = PAREN_SELECT_RE.find_at(&out, from).map(|m| m.start()) {
        let end = group_end(out.as_bytes(), open);
        let before = out[..open].trim_end();
        let position = before.is_empty() || before.ends_with('(') || SET_OP_BEFORE_RE.is_match(before);
        let member = position && (SET_OP_RE.is_match(&out[end..]) || SET_OP_BEFORE_RE.is_match(before));
        if member {
            out.insert_str(open, "SELECT * FROM ");
            from = open + "SELECT * FROM ".len() + 1;
        } else {
            from = open + 1;
        }
    }
    out
}

static UPSERT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\bON\s+CONFLICT\b").unwrap());

/// SQLite parses `INSERT ... SELECT ... ON CONFLICT` only when the SELECT has a WHERE clause.
fn upsert_select_where(s: &str) -> String {
    let Some(m) = UPSERT_RE.find(s) else { return s.to_string() };
    let head = &s[..m.start()];
    let upper = head.to_ascii_uppercase();
    if !upper.trim_start().starts_with("INSERT") && !upper.contains("INSERT INTO") {
        return s.to_string();
    }
    // Top-level SELECT / WHERE in the INSERT head (outside parentheses).
    let (mut depth, mut sel, mut wh) = (0i32, None, None);
    let b = upper.as_bytes();
    for i in 0..b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            _ if depth == 0 => {
                let word = |w: &str| upper[i..].starts_with(w) && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) && !upper[i + w.len()..].starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_');
                if word("SELECT") {
                    sel = Some(i);
                    wh = None;
                } else if word("WHERE") {
                    wh = Some(i);
                }
            }
            _ => {}
        }
    }
    if sel.is_some() && wh.is_none() {
        format!("{} WHERE true {}", head.trim_end(), &s[m.start()..])
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::translate;

    #[test]
    fn casts() {
        assert_eq!(translate("SELECT $1::text, x::int, (a / b)::float FROM t"), "SELECT CAST($1 AS TEXT), CAST(ROUND(x) AS INTEGER), CAST((a / b) AS REAL) FROM t");
        assert_eq!(translate("SELECT COALESCE(x, 0)::bigint, $2::int[], y::timestamptz, 'a::b'"), "SELECT CAST(ROUND(COALESCE(x, 0)) AS INTEGER), $2, y, 'a::b'");
        assert_eq!(translate(r#"SELECT "T".col::varchar(255)"#), r#"SELECT CAST("T".col AS TEXT)"#);
        assert_eq!(translate("SELECT CASE WHEN a THEN CASE WHEN b THEN 1 END ELSE 2 END::int"), "SELECT CAST(ROUND(CASE WHEN a THEN CASE WHEN b THEN 1 END ELSE 2 END) AS INTEGER)");
    }

    #[test]
    fn time() {
        assert_eq!(
            translate("WHERE d >= NOW() - INTERVAL '30 days' AND e < NOW()"),
            "WHERE d >= datetime(strftime('%Y-%m-%d %H:%M:%f', 'now'), '-30 days') AND e < strftime('%Y-%m-%d %H:%M:%f', 'now')"
        );
        assert_eq!(
            translate("WHERE a < NOW() - (INTERVAL '1 minute' * (5 * 2))"),
            "WHERE a < datetime(strftime('%Y-%m-%d %H:%M:%f', 'now'), (-(((5 * 2)) * 1)) || ' minute')"
        );
        assert_eq!(
            translate("WHERE e.d AT TIME ZONE 'UTC' AT TIME ZONE COALESCE(u.tz, 'UTC') > (CURRENT_TIMESTAMP AT TIME ZONE $1 - INTERVAL '5' HOUR)"),
            "WHERE e.d > (datetime(CURRENT_TIMESTAMP, '-5 hour'))"
        );
        assert_eq!(translate("SELECT EXTRACT(EPOCH FROM x::timestamptz)::bigint"), "SELECT CAST(ROUND(unixepoch(x, 'subsec')) AS INTEGER)");
    }

    #[test]
    fn arrays_and_misc() {
        assert_eq!(translate("WHERE id = ANY($1) AND n <> ALL($2)"), "WHERE id IN (SELECT value FROM json_each($1)) AND n NOT IN (SELECT value FROM json_each($2))");
        assert_eq!(translate("WHERE a ILIKE $1 AND GREATEST(a, b) > LEAST(c, d) FOR UPDATE"), "WHERE a LIKE $1 AND MAX(a, b) > MIN(c, d)");
        assert_eq!(translate("SELECT a FROM t WHERE u = $1 FOR SHARE"), "SELECT a FROM t WHERE u = $1");
        assert_eq!(translate(r#"SELECT RIGHT(k, 4), TO_CHAR(MIN(d), 'YYYY-MM-DD"T"HH24:MI:SS')"#), "SELECT substr(k, -(4)), strftime('%Y-%m-%dT%H:%M:%S', MIN(d))");
        assert_eq!(translate("WHERE NOT ($3 = ANY(string_to_array(x, ',')))"), "WHERE NOT ($3 IN (SELECT value FROM json_each(('[\"' || replace(x, ',', '\",\"') || '\"]'))))");
        assert_eq!(translate(r#"TRUNCATE TABLE "Episodes" CASCADE"#), r#"DELETE FROM "Episodes""#);
        assert_eq!(translate("SELECT FLOOR(x / 2)"), "SELECT (CASE WHEN (x / 2) < CAST((x / 2) AS INTEGER) THEN CAST((x / 2) AS INTEGER) - 1 ELSE CAST((x / 2) AS INTEGER) END)");
        assert_eq!(translate("WHERE split_part(u, '?', 1) = b"), "WHERE (CASE WHEN instr(u, '?') > 0 THEN substr(u, 1, instr(u, '?') - 1) ELSE u END) = b");
    }

    #[test]
    fn unnest_and_upsert() {
        assert_eq!(
            translate("INSERT INTO t (a, b, c) SELECT $1, u.x, u.y FROM UNNEST($2::text[], $3::int[]) AS u(x, y) ON CONFLICT DO NOTHING"),
            "INSERT INTO t (a, b, c) SELECT $1, u.x, u.y FROM (SELECT j0.value AS x, ($3 ->> j0.key) AS y FROM json_each($2) j0) AS u WHERE true ON CONFLICT DO NOTHING"
        );
        assert_eq!(
            translate("SELECT i.a, i.n FROM UNNEST($1::text[]) WITH ORDINALITY AS i(a, n)"),
            "SELECT i.a, i.n FROM (SELECT j0.value AS a, j0.key + 1 AS n FROM json_each($1) j0) AS i "
        );
        assert_eq!(
            translate("UPDATE x SET n = NOW() + make_interval(secs => $3::BIGINT) WHERE g > CURRENT_TIMESTAMP - make_interval(hours => $2)"),
            "UPDATE x SET n = datetime(strftime('%Y-%m-%d %H:%M:%f', 'now'), (CAST(ROUND($3) AS INTEGER)) || ' seconds') WHERE g > datetime(CURRENT_TIMESTAMP, -($2) || ' hours')"
        );
        assert_eq!(translate("INSERT INTO t (a) VALUES ($1) ON CONFLICT (a) DO NOTHING"), "INSERT INTO t (a) VALUES ($1) ON CONFLICT (a) DO NOTHING");
        assert_eq!(
            translate("INSERT INTO d (a, b) SELECT $1, UNNEST($2::text[]) ON CONFLICT DO NOTHING"),
            "INSERT INTO d (a, b) SELECT $1, unnest_j.value FROM json_each($2) AS unnest_j WHERE true ON CONFLICT DO NOTHING"
        );
        assert_eq!(translate(r#"UPDATE "Feeds" f SET a = 1 WHERE f.b = 2"#), r#"UPDATE "Feeds" AS f SET a = 1 WHERE f.b = 2"#);
        assert_eq!(translate(r#"UPDATE "Feeds" SET a = 1"#), r#"UPDATE "Feeds" SET a = 1"#);
        assert_eq!(translate("SELECT a FROM x WHERE b IN (SELECT b FROM y) UNION ALL SELECT a FROM z"), "SELECT a FROM x WHERE b IN (SELECT b FROM y) UNION ALL SELECT a FROM z");
        assert_eq!(translate("SELECT AVG(EXTRACT(HOUR FROM d))"), "SELECT AVG(CAST(strftime('%H', d) AS INTEGER))");
        assert_eq!(
            translate("SELECT * FROM ((SELECT a FROM x) UNION ALL (SELECT a FROM y LIMIT 2)) t WHERE (SELECT 1) = 1"),
            "SELECT * FROM (SELECT * FROM (SELECT a FROM x) UNION ALL SELECT * FROM (SELECT a FROM y LIMIT 2)) t WHERE (SELECT 1) = 1"
        );
    }
}
