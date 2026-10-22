//! `TestApp`: one fully isolated PinePods server per test.
//!
//! Each test gets its own database cloned from the migrated template, a real `AppState`
//! (real `DatabasePool`, real Valkey), and the exact production router from
//! `pinepods_api::create_app`. Requests go through the router in-process (no sockets), so a
//! test exercises routing, extractors, auth checks, handlers and SQL exactly as production
//! does — minus background schedulers, which tests trigger explicitly when they need them.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use argon2::password_hash::PasswordHasher;
use axum::Router;
use base64::Engine as _;
use serde_json::json;
use sqlx::{Connection, Executor, MySqlConnection, PgConnection};

use pinepods_api::config::{
    ApiConfig, Config, DatabaseConfig, EmailConfig, OIDCConfig, RedisConfig, SecurityConfig, ServerConfig,
};
use pinepods_api::database::DatabasePool;
use pinepods_api::redis_client::RedisClient;
use pinepods_api::AppState;

use super::env::{self, col_string, Backend, TestEnv};
use super::fixtures::Fixtures;
use super::http::RequestBuilder;

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// IDs handed out in a test's database start at `slot * SLOT_WIDTH`. Valkey is shared by
/// every test, and many keys embed user/episode ids (now-playing, rate limits, caches), so
/// giving each test a disjoint id range keeps parallel tests from seeing each other's state.
const SLOT_WIDTH: i64 = 10_000;
const SLOTS: u64 = 200_000; // * SLOT_WIDTH stays below i32::MAX

#[derive(Clone, Debug)]
pub struct TestUser {
    pub id: i32,
    pub username: String,
    pub password: String,
    pub email: String,
    pub api_key: String,
}

pub struct TestApp {
    pub env: &'static TestEnv,
    pub state: AppState,
    pub router: Router,
    pub db_name: String,
    /// Unique per test; suffix for usernames etc. (login rate-limit counters live in Valkey).
    pub tag: String,
    fixtures: Fixtures,
    admin: tokio::sync::OnceCell<TestUser>,
}

impl TestApp {
    pub async fn new() -> TestApp {
        Self::with_config(|_| {}).await
    }

    /// Build an app after letting the test tweak the server config (e.g. OIDC settings).
    pub async fn with_config(tweak: impl FnOnce(&mut Config)) -> TestApp {
        init_logging();
        let env = env::env();
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let slot = ((pid as u64).wrapping_mul(7919) + n as u64) % SLOTS + 1;
        let tag = format!("{pid:x}{n:x}");
        let db_name = format!("pt_{tag}");
        let id_base = slot as i64 * SLOT_WIDTH;

        clone_template(env, &db_name, id_base).await;

        let fixtures = Fixtures { server: env.fixtures.clone(), namespace: format!("t-{tag}") };
        let mut config = Config {
            database: DatabaseConfig {
                db_type: env.backend.db_type().to_string(),
                host: env.db_host.clone(),
                port: env.db_port,
                username: env.db_user.clone(),
                password: env.db_password.clone(),
                name: db_name.clone(),
                max_connections: 8,
                min_connections: 0,
            },
            redis: RedisConfig {
                host: env.valkey_host.clone(),
                port: env.valkey_port,
                max_connections: 4,
                password: None,
                username: None,
                database: None,
            },
            server: ServerConfig { port: 0, host: "127.0.0.1".into() },
            security: SecurityConfig {
                api_key_header: "pinepods_api".into(),
                jwt_secret: "integration-test-secret".into(),
                password_salt_rounds: 4,
            },
            email: EmailConfig {
                smtp_server: None,
                smtp_port: None,
                smtp_username: None,
                smtp_password: None,
                from_email: None,
            },
            oidc: OIDCConfig {
                disable_standard_login: false,
                provider_name: None,
                client_id: None,
                client_secret: None,
                authorization_url: None,
                token_url: None,
                user_info_url: None,
                button_text: None,
                scope: None,
                button_color: None,
                button_text_color: None,
                icon_svg: None,
                name_claim: None,
                email_claim: None,
                username_claim: None,
                roles_claim: None,
                user_role: None,
                admin_role: None,
            },
            api: ApiConfig {
                search_api_url: fixtures.url("search-api/api/search"),
                people_api_url: fixtures.url("people-api"),
            },
        };
        tweak(&mut config);

        let db_pool = DatabasePool::new(&config).await.expect("connect test database");
        let redis_client = RedisClient::new(&config).await.expect("connect test valkey");
        let instance_id: Arc<str> = Arc::from(format!("test-{tag}").as_str());
        let task_manager = Arc::new(pinepods_api::services::task_manager::TaskManager::new(
            redis_client.clone(),
            instance_id.clone(),
        ));
        let task_spawner = Arc::new(pinepods_api::services::tasks::TaskSpawner::new(
            task_manager.clone(),
            db_pool.clone(),
        ));
        let state = AppState {
            db_pool,
            redis_client: redis_client.clone(),
            config,
            task_manager,
            task_spawner,
            websocket_manager: Arc::new(pinepods_api::handlers::websocket::WebSocketManager::new()),
            now_playing_manager: Arc::new(pinepods_api::handlers::nowplaying::NowPlayingManager::new()),
            instance_id,
            import_progress_manager: Arc::new(pinepods_api::redis_manager::ImportProgressManager::new(
                redis_client,
            )),
            notification_manager: Arc::new(pinepods_api::redis_manager::NotificationManager::new()),
            restore_in_progress: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ai_available: pinepods_api::services::ai_client::AiAvailability::new(),
        };
        let router = pinepods_api::create_app(state.clone());

        TestApp { env, state, router, db_name, tag, fixtures, admin: tokio::sync::OnceCell::new() }
    }

    pub fn backend(&self) -> Backend {
        self.env.backend
    }

    pub fn db(&self) -> &DatabasePool {
        &self.state.db_pool
    }

    pub fn fixtures(&self) -> &Fixtures {
        &self.fixtures
    }

    // ---- requests ------------------------------------------------------------------------

    pub fn request(&self, method: axum::http::Method, path: &str) -> RequestBuilder {
        RequestBuilder::new(self.router.clone(), method, path)
    }
    pub fn get(&self, path: &str) -> RequestBuilder {
        self.request(axum::http::Method::GET, path)
    }
    pub fn post(&self, path: &str) -> RequestBuilder {
        self.request(axum::http::Method::POST, path)
    }
    pub fn put(&self, path: &str) -> RequestBuilder {
        self.request(axum::http::Method::PUT, path)
    }
    pub fn delete(&self, path: &str) -> RequestBuilder {
        self.request(axum::http::Method::DELETE, path)
    }

    // ---- users ---------------------------------------------------------------------------

    /// The instance's first admin, created through the real first-run endpoint.
    pub async fn admin(&self) -> &TestUser {
        self.admin
            .get_or_init(|| async {
                let username = format!("admin{}", self.tag);
                let password = "Admin-pass-123!".to_string();
                let email = format!("{username}@example.test");
                let resp = self
                    .post("/api/data/create_first")
                    .json(&json!({
                        "username": username,
                        "password": hash_password(&password),
                        "email": email,
                        "fullname": "Test Admin",
                    }))
                    .send()
                    .await;
                resp.assert_ok();
                let id = resp.json()["user_id"].as_i64().expect("user_id") as i32;
                let api_key = self.login(&username, &password).await;
                TestUser { id, username, password, email, api_key }
            })
            .await
    }

    /// A regular (non-admin) user created by the admin through `/api/data/add_user`.
    pub async fn user(&self, name: &str) -> TestUser {
        let admin = self.admin().await.clone();
        let username = format!("{name}{}", self.tag);
        let password = format!("{name}-Pass-456!");
        let email = format!("{username}@example.test");
        let resp = self
            .post("/api/data/add_user")
            .key(&admin)
            .json(&json!({
                "fullname": format!("{name} Tester"),
                "username": username,
                "email": email,
                "hash_pw": hash_password(&password),
            }))
            .send()
            .await;
        resp.assert_ok();
        let id = resp.json()["user_id"].as_i64().expect("user_id") as i32;
        let api_key = self.login(&username, &password).await;
        TestUser { id, username, password, email, api_key }
    }

    /// A second admin (promoted via the real endpoint).
    pub async fn second_admin(&self, name: &str) -> TestUser {
        let u = self.user(name).await;
        let admin = self.admin().await.clone();
        self.put("/api/data/user/set_isadmin")
            .key(&admin)
            .json(&json!({ "user_id": u.id, "isadmin": true }))
            .send()
            .await
            .assert_ok();
        u
    }

    /// Username/password login → API key (`/api/data/get_key`, HTTP Basic).
    pub async fn login(&self, username: &str, password: &str) -> String {
        let resp = self.get("/api/data/get_key").basic_auth(username, password).send().await;
        resp.assert_ok();
        let body = resp.json();
        assert_eq!(body["status"], "success", "login response: {body}");
        body["retrieved_key"].as_str().expect("retrieved_key").to_string()
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        if std::env::var("PINEPODS_TEST_KEEP_DB").is_ok() {
            eprintln!("[harness] keeping test database {}", self.db_name);
            return;
        }
        let env = self.env;
        let name = self.db_name.clone();
        // Drop is sync and we may be inside a runtime: do the async teardown on a fresh thread.
        let _ = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async {
                match env.backend {
                    Backend::Postgres => {
                        let mut c = env::admin_pg(env).await;
                        let _ = c
                            .execute(sqlx::AssertSqlSafe(format!(r#"DROP DATABASE IF EXISTS "{name}" WITH (FORCE)"#)))
                            .await;
                    }
                    Backend::MariaDb => {
                        let mut c = env::admin_mysql(env).await;
                        let _ = c.execute(sqlx::AssertSqlSafe(format!("DROP DATABASE IF EXISTS `{name}`"))).await;
                    }
                }
            });
        })
        .join();
    }
}

/// What the web/mobile clients do before sending a password to `create_first` / `add_user`.
pub fn hash_password(password: &str) -> String {
    argon2::Argon2::default()
        .hash_password(password.as_bytes())
        .expect("argon2 hash")
        .to_string()
}

pub fn basic(username: &str, password: &str) -> String {
    format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"))
    )
}

async fn clone_template(env: &TestEnv, db_name: &str, id_base: i64) {
    match env.backend {
        Backend::Postgres => {
            let mut admin = env::admin_pg(env).await;
            let create = format!(r#"CREATE DATABASE "{db_name}" TEMPLATE "{}""#, env.template);
            // Concurrent clones of one template can transiently collide; retry briefly.
            let mut attempt = 0;
            loop {
                match admin.execute(sqlx::AssertSqlSafe(create.clone())).await {
                    Ok(_) => break,
                    Err(e) if attempt < 20 && e.to_string().contains("being accessed by other users") => {
                        attempt += 1;
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    Err(e) => panic!("create test database {db_name}: {e}"),
                }
            }
            let mut c = PgConnection::connect(&env::pg_url(env, db_name)).await.unwrap();
            c.execute(sqlx::AssertSqlSafe(format!(
                "DO $$ DECLARE r record; BEGIN \
                   FOR r IN SELECT schemaname, sequencename FROM pg_sequences LOOP \
                     EXECUTE format('SELECT setval(%L, {id_base})', quote_ident(r.schemaname) || '.' || quote_ident(r.sequencename)); \
                   END LOOP; END $$"
            )))
            .await
            .expect("offset sequences");
        }
        Backend::MariaDb => {
            let tmpl = &env.template;
            let mut admin = env::admin_mysql(env).await;
            admin
                .execute(sqlx::AssertSqlSafe(format!("CREATE DATABASE `{db_name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci")))
                .await
                .unwrap();
            let mut c = MySqlConnection::connect(&env::mysql_url(env, Some(db_name))).await.unwrap();
            c.execute("SET FOREIGN_KEY_CHECKS = 0").await.unwrap();
            let tables: Vec<String> = sqlx::query(
                "SELECT table_name FROM information_schema.tables \
                 WHERE table_schema = ? AND table_type = 'BASE TABLE' AND table_name <> '_tmpl_ready'",
            )
            .bind(tmpl)
            .fetch_all(&mut c)
            .await
            .unwrap()
            .iter()
            .map(|r| col_string(r, 0))
            .collect();
            for t in &tables {
                let row = sqlx::query(sqlx::AssertSqlSafe(format!("SHOW CREATE TABLE `{tmpl}`.`{t}`"))).fetch_one(&mut c).await.unwrap();
                // Unqualified: the connection's current database is the new one, so the
                // table *and* its FOREIGN KEY ... REFERENCES targets land there.
                c.execute(sqlx::AssertSqlSafe(col_string(&row, 1))).await.unwrap_or_else(|e| panic!("clone {t}: {e}"));
                c.execute(sqlx::AssertSqlSafe(format!("INSERT INTO `{t}` SELECT * FROM `{tmpl}`.`{t}`")))
                    .await
                    .unwrap_or_else(|e| panic!("copy {t}: {e}"));
            }
            let auto: Vec<String> = sqlx::query(
                "SELECT DISTINCT table_name FROM information_schema.columns \
                 WHERE table_schema = ? AND extra LIKE '%auto_increment%'",
            )
            .bind(db_name)
            .fetch_all(&mut c)
            .await
            .unwrap()
            .iter()
            .map(|r| col_string(r, 0))
            .collect();
            for t in auto {
                c.execute(sqlx::AssertSqlSafe(format!("ALTER TABLE `{t}` AUTO_INCREMENT = {}", id_base + 1)))
                    .await
                    .unwrap();
            }
            c.execute("SET FOREIGN_KEY_CHECKS = 1").await.unwrap();
        }
    }
}

/// `PINEPODS_TEST_LOG=<filter>` (e.g. `warn` or `pinepods_api=debug`) prints the server's logs,
/// for finding out why a request failed. Run with `-- --nocapture`.
fn init_logging() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        if let Ok(filter) = std::env::var("PINEPODS_TEST_LOG") {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
                .with_test_writer()
                .try_init();
        }
    });
}
