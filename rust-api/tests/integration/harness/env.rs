//! Process-wide test environment: which database backend we're on, where the throwaway
//! servers live, the migrated template database every test clones, and the sandboxed network.
//!
//! Initialised exactly once per test process (tests run in parallel threads, and nextest
//! may run them in parallel *processes* — template creation is guarded by a server-side lock
//! so both work).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use sqlx::{Connection, Executor, MySqlConnection, PgConnection, Row};

use super::fixtures::FixtureServer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Postgres,
    MariaDb,
}

impl Backend {
    /// The `DB_TYPE` string the server config expects.
    pub fn db_type(self) -> &'static str {
        match self {
            Backend::Postgres => "postgresql",
            Backend::MariaDb => "mariadb",
        }
    }
}

pub struct TestEnv {
    pub backend: Backend,
    pub db_host: String,
    pub db_port: u16,
    pub db_user: String,
    pub db_password: String,
    pub valkey_host: String,
    pub valkey_port: u16,
    /// Name of the fully-migrated database every test clones.
    pub template: String,
    pub fixtures: FixtureServer,
}

static ENV: OnceLock<TestEnv> = OnceLock::new();

fn var(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

pub fn env() -> &'static TestEnv {
    ENV.get_or_init(|| {
        // Built on a dedicated thread with its own runtime: callers are inside a
        // #[tokio::test] runtime that may not allow blocking, and the template build is
        // genuinely blocking (it shells out to the Python migration runner).
        std::thread::spawn(init)
            .join()
            .unwrap_or_else(|_| panic!("integration test environment failed to initialise (see output above)"))
    })
}

fn init() -> TestEnv {
    sandbox_network();
    pinepods_api::services::url_guard::allow_loopback_for_tests();

    let backend = match var("PINEPODS_TEST_DB_TYPE", "postgresql").to_lowercase().as_str() {
        "postgresql" | "postgres" => Backend::Postgres,
        "mariadb" | "mysql" => Backend::MariaDb,
        other => panic!("PINEPODS_TEST_DB_TYPE={other}: expected postgresql or mariadb"),
    };
    let (default_port, default_user) = match backend {
        Backend::Postgres => ("55433", "pinepods"),
        Backend::MariaDb => ("33307", "root"),
    };

    let mut env = TestEnv {
        backend,
        db_host: var("PINEPODS_TEST_DB_HOST", "127.0.0.1"),
        db_port: var("PINEPODS_TEST_DB_PORT", default_port).parse().expect("PINEPODS_TEST_DB_PORT"),
        db_user: var("PINEPODS_TEST_DB_USER", default_user),
        db_password: var("PINEPODS_TEST_DB_PASSWORD", "testpass"),
        valkey_host: var("PINEPODS_TEST_VALKEY_HOST", "127.0.0.1"),
        valkey_port: var("PINEPODS_TEST_VALKEY_PORT", "56380").parse().expect("PINEPODS_TEST_VALKEY_PORT"),
        template: String::new(),
        fixtures: FixtureServer::start(),
    };

    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    env.template = rt.block_on(ensure_template(&env));
    env
}

/// Make every outbound HTTP request from the server fail fast unless it targets the local
/// fixture server. reqwest honours these variables when a client is built, so set them
/// before any `AppState` exists. Port 9 (discard) on loopback refuses instantly.
fn sandbox_network() {
    for (k, v) in [
        ("HTTP_PROXY", "http://127.0.0.1:9"),
        ("HTTPS_PROXY", "http://127.0.0.1:9"),
        ("ALL_PROXY", "http://127.0.0.1:9"),
        ("http_proxy", "http://127.0.0.1:9"),
        ("https_proxy", "http://127.0.0.1:9"),
        ("all_proxy", "http://127.0.0.1:9"),
        ("NO_PROXY", "127.0.0.1,localhost"),
        ("no_proxy", "127.0.0.1,localhost"),
    ] {
        // SAFETY (edition 2021 API): runs once, before the server code under test starts
        // any threads that read the environment.
        std::env::set_var(k, v);
    }
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// The template is named after a hash of everything that defines the schema, so editing a
/// migration transparently produces (and caches) a fresh template.
fn schema_fingerprint() -> String {
    let root = repo_root();
    let mut h = DefaultHasher::new();
    for rel in [
        "database_functions/migration_definitions.py",
        "database_functions/migrations.py",
        "startup/setup_database_new.py",
    ] {
        std::fs::read(root.join(rel))
            .unwrap_or_else(|e| panic!("read {rel}: {e}"))
            .hash(&mut h);
    }
    format!("{:012x}", h.finish() & 0xffff_ffff_ffff)
}

fn not_running_hint(env: &TestEnv, err: impl std::fmt::Display) -> String {
    format!(
        "cannot reach the test {:?} server at {}:{} ({err}).\n\
         Start the throwaway services with `rust-api/scripts/integration-tests.sh` (which also runs \
         the suite), or `docker compose -f rust-api/tests/docker-compose.yml up -d --wait`.",
        env.backend, env.db_host, env.db_port
    )
}

pub fn pg_url(env: &TestEnv, db: &str) -> String {
    format!(
        "postgres://{}:{}@{}:{}/{}",
        env.db_user, env.db_password, env.db_host, env.db_port, db
    )
}

pub fn mysql_url(env: &TestEnv, db: Option<&str>) -> String {
    format!(
        "mysql://{}:{}@{}:{}/{}",
        env.db_user,
        env.db_password,
        env.db_host,
        env.db_port,
        db.unwrap_or("")
    )
}

async fn ensure_template(env: &TestEnv) -> String {
    let name = format!("pinepods_tmpl_{}", schema_fingerprint());
    match env.backend {
        Backend::Postgres => {
            let mut conn = PgConnection::connect(&pg_url(env, "postgres"))
                .await
                .unwrap_or_else(|e| panic!("{}", not_running_hint(env, e)));
            conn.execute("SELECT pg_advisory_lock(7283110)").await.unwrap();
            let exists = sqlx::query("SELECT 1 FROM pg_database WHERE datname = $1")
                .bind(&name)
                .fetch_optional(&mut conn)
                .await
                .unwrap()
                .is_some();
            if !exists {
                let building = format!("{name}_building");
                conn.execute(sqlx::AssertSqlSafe(format!(r#"DROP DATABASE IF EXISTS "{building}" WITH (FORCE)"#)))
                    .await
                    .unwrap();
                conn.execute(sqlx::AssertSqlSafe(format!(r#"CREATE DATABASE "{building}""#))).await.unwrap();
                run_migrations(env, &building);
                // Publish atomically so a half-migrated template is never cloned.
                conn.execute(sqlx::AssertSqlSafe(format!(r#"ALTER DATABASE "{building}" RENAME TO "{name}""#)))
                    .await
                    .unwrap();
            }
            conn.execute("SELECT pg_advisory_unlock(7283110)").await.unwrap();
        }
        Backend::MariaDb => {
            let mut conn = MySqlConnection::connect(&mysql_url(env, None))
                .await
                .unwrap_or_else(|e| panic!("{}", not_running_hint(env, e)));
            sqlx::query("SELECT GET_LOCK('pinepods_tmpl', 600)")
                .fetch_one(&mut conn)
                .await
                .unwrap();
            // MariaDB can't rename databases, so a marker table records "fully migrated".
            let ready = sqlx::query(
                "SELECT 1 FROM information_schema.tables WHERE table_schema = ? AND table_name = '_tmpl_ready'",
            )
            .bind(&name)
            .fetch_optional(&mut conn)
            .await
            .unwrap()
            .is_some();
            if !ready {
                conn.execute(sqlx::AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{name}`"))).await.unwrap();
                conn.execute(sqlx::AssertSqlSafe(format!("CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci")))
                .await
                .unwrap();
                run_migrations(env, &name);
                conn.execute(sqlx::AssertSqlSafe(format!("CREATE TABLE `{name}`.`_tmpl_ready` (x INT)")))
                    .await
                    .unwrap();
            }
            sqlx::query("SELECT RELEASE_LOCK('pinepods_tmpl')")
                .fetch_one(&mut conn)
                .await
                .unwrap();
        }
    }
    name
}

/// Run the real production migration path (startup/setup_database_new.py) into `db_name`.
pub fn run_migrations(env: &TestEnv, db_name: &str) {
    let python = var("PINEPODS_TEST_PYTHON", "python3");
    let root = repo_root();
    eprintln!("[harness] migrating template database {db_name} ({:?}) with {python}", env.backend);
    let out = Command::new(&python)
        .arg(root.join("startup/setup_database_new.py"))
        .current_dir(&root)
        // Migration 004 seeds an admin from these if present; a developer's shell may well
        // have USERNAME set, which would silently plant an extra admin in every test DB.
        .env_remove("USERNAME")
        .env_remove("PASSWORD")
        .env_remove("FULLNAME")
        .env_remove("EMAIL")
        .env("DB_TYPE", env.backend.db_type())
        .env("DB_HOST", &env.db_host)
        .env("DB_PORT", env.db_port.to_string())
        .env("DB_USER", &env.db_user)
        .env("DB_PASSWORD", &env.db_password)
        .env("DB_NAME", db_name)
        .output()
        .unwrap_or_else(|e| {
            panic!(
                "could not run {python} for migrations ({e}). Set PINEPODS_TEST_PYTHON to a python \
                 with rust-api/tests/requirements.txt installed, or use scripts/integration-tests.sh"
            )
        });
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        let tail = |s: &str| s.lines().rev().take(40).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        panic!(
            "schema migrations failed for template {db_name}:\n--- stdout ---\n{}\n--- stderr ---\n{}",
            tail(&stdout),
            tail(&stderr)
        );
    }
}

/// Shared helper for per-test databases: connect to the server's maintenance database.
pub async fn admin_pg(env: &TestEnv) -> PgConnection {
    PgConnection::connect(&pg_url(env, "postgres")).await.expect("connect postgres admin")
}

pub async fn admin_mysql(env: &TestEnv) -> MySqlConnection {
    MySqlConnection::connect(&mysql_url(env, None)).await.expect("connect mariadb admin")
}

/// Row helper used by the MariaDB clone (column 1 of SHOW CREATE TABLE).
pub fn col_string(row: &sqlx::mysql::MySqlRow, idx: usize) -> String {
    row.try_get::<String, _>(idx)
        .or_else(|_| row.try_get::<Vec<u8>, _>(idx).map(|b| String::from_utf8_lossy(&b).into_owned()))
        .expect("string column")
}
