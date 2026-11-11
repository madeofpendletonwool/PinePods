# rust-api integration tests

Every test boots the real axum router (`pinepods_api::create_app`) against its **own freshly
migrated database** (Postgres, MariaDB or SQLite) plus Valkey (SQLite runs use the in-memory
store instead, as a SQLite install does by default), and drives it over HTTP exactly as the
web and mobile clients do. Nothing is mocked except the outside internet.

## Running

```sh
scripts/integration-tests.sh                    # every backend, every test
scripts/integration-tests.sh --db postgres      # one backend (postgres | mariadb | sqlite)
scripts/integration-tests.sh -- queue           # extra args go to `cargo test` (name filter)
scripts/integration-tests.sh -- accounts -- --nocapture
scripts/integration-tests.sh down               # stop the throwaway containers
scripts/integration-tests.sh --gpodder mock -- gpodder # gpodder protocol tests (rust | mock)
```

The script starts Postgres, MariaDB and Valkey from `tests/docker-compose.yml` (off-default
ports, tmpfs storage) and builds a Python venv under `target/` for the migration runner. CI
(`.github/workflows/rust-api-tests.yml`) runs the same thing as a matrix over the three databases,
using service containers (`PINEPODS_TEST_EXTERNAL_SERVICES=1`).

Useful environment variables:

| Variable | Meaning |
|---|---|
| `PINEPODS_TEST_DB_TYPE` | `postgresql`, `mariadb` or `sqlite` (set by the script) |
| `PINEPODS_TEST_DB_HOST` / `_PORT` / `_USER` / `_PASSWORD` | database server |
| `PINEPODS_TEST_VALKEY_HOST` / `_PORT` | Valkey server |
| `PINEPODS_TEST_PYTHON` | Python used to run `startup/setup_database_new.py` |
| `PINEPODS_TEST_KEEP_DB` | keep each test's database afterwards for inspection |
| `PINEPODS_TEST_EXTERNAL_SERVICES` | don't start containers; use the ones above |
| `PINEPODS_TEST_GPODDER` | gpodder server the protocol tests talk to: `rust` (default), `mock` |
| `PINEPODS_TEST_LOG` | print the server's logs with this filter (e.g. `warn`); add `-- --nocapture` |

## How it works

- **Schema.** On first use the harness runs the real migrations (`startup/setup_database_new.py`)
  once into a template database named after a hash of the migration files, so a migration change
  rebuilds it automatically. Each test then clones the template (`CREATE DATABASE … TEMPLATE` on
  Postgres, a table copy on MariaDB, a file copy on SQLite) and drops its clone when the `TestApp`
  is dropped. The SQLite template lives under `target/sqlite-templates`.
- **SQLite.** SQLite runs the PostgreSQL query arms through a SQL translation layer
  (`src/database/dialect.rs`), so a new query needs no SQLite code of its own, but run
  `--db sqlite` to check it translates. Tests that replay old migrations skip SQLite
  (`app.upgrade_tests_unsupported()`): new SQLite databases start from a schema baseline.
  `schema_baseline.rs` fails (on Postgres) when the migrations no longer match that baseline;
  regenerate it with `python -m database_functions.sqlite_baseline <dsn of a freshly migrated
  Postgres database>`.
- **Isolation.** Tests run in parallel. Each gets its own database, a unique tag used in
  usernames and fixture paths, and an ID range (sequences / `AUTO_INCREMENT` offset per test) so
  keys in the shared Valkey never collide.
- **No internet.** `HTTP(S)_PROXY` points at a dead port, so any accidental outbound request
  fails fast. Feeds, chapters JSON, search APIs etc. are served by a local fixture server
  (`harness/fixtures.rs`) that also records every request (headers included) for assertions.
  The SSRF guard is told to allow loopback for this.
- **Real clients' view.** Users are created the way the apps do it (`create_first`, `add_user`,
  login via `get_key`), passwords are argon2-hashed client-side like the web app does, and
  background work (refresh, imports) is awaited through the same endpoints the UI polls.

## Layout

```
integration/
  main.rs            module list
  harness/           TestApp, HTTP builder, fixture server, RSS builder, common flows
  support/openapi.rs the generated OpenAPI spec, used to enumerate every operation
  auth_coverage.rs   every operation in the spec rejects missing/invalid API keys
  admin_only.rs      admin/system-only operations reject regular users
  cross_user.rs      user-scoped operations reject another user's id
  foreign_resources.rs  probes every operation with another user's resource ids
  gpodder/           gpodder protocol conformance (see below)
  accounts.rs, settings.rs, podcasts.rs, queue.rs, playback.rs, library.rs,
  feeds.rs, opml.rs, podcasting20.rs, downloads.rs, backups.rs, local_media.rs,
  email.rs, translations.rs, ai.rs, now_playing.rs, timers.rs   feature behaviour
  mobile_contract.rs  the mobile app's API client against this server
```

The first four files are driven by the OpenAPI spec, so a **new endpoint is covered by the
auth and access checks automatically**. If it legitimately needs different treatment (public,
admin-only, any-user) the lists at the top of those files say so, and each entry needs a reason.

## Writing a test

```rust
use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
async fn queueing_puts_the_episode_next() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let url = app.feed("show.xml", &Feed::with_episodes("Show", 2));
    let pod = app.subscribe(&user, &url, 2).await;            // waits for the episodes
    let ep = app.episode_id(&user, pod, "Episode 1").await;

    app.post("/api/data/queue_pod")
        .key(&user)
        .json(&json!({ "episode_id": ep, "user_id": user.id, "is_youtube": false }))
        .send()
        .await
        .assert_ok();
}
```

Guidelines:

- Name tests after the behaviour a user relies on, and assert what they would see.
- Test on both backends; differences between them have been a steady source of bugs.
- For anything that touches another user's data, assert both that the request fails *and*
  that the victim's data is unchanged.
- Use `eventually(...)` for background work instead of sleeping.
- When a test finds a bug, fix the bug in the same change and keep the test.

## gpodder protocol tests

`gpodder/` checks the gpodder sync server over real HTTP, the way AntennaPod and other
clients use it. `PINEPODS_TEST_GPODDER` picks the server:

- `rust` (default): the rust-api router on a real socket.
- `mock`: the in-memory reference server in `harness/mock_gpodder.rs`. It follows the
  cursor rules, so every test must pass against it; that's how the tests themselves are
  checked. It also stands in for external gpodder/Nextcloud servers in sync engine tests.

`harness/antennapod.rs` holds AntennaPod's response parsing as assertions: a `play` action
without `started >= 0`, `position > 0` and `total > 0` resets the episode to 0 there.

Library assertions are skipped against `mock`, which has no Pinepods database behind it.

## Files, mail, AI and sockets

- **Files.** `PINEPODS_DATA_DIR` and `PINEPODS_WEB_ROOT` move the server's storage, so tests use a
  sandbox instead of `/opt/pinepods` (`harness/files.rs`). They're process-wide: tests tell their
  files apart by unique podcast and folder names.
- **Backups** run the real `pg_dump`/`psql`/`mysqldump`/`mysql`. They read `DB_*` from the
  environment and share a folder, so those tests take turns through `BackupEnv`. Without the tools
  installed, the script runs the ones inside the test containers.
- **AI** uses a stand-in sidecar with the real protocol (`harness/mock_ai.rs`). Tests choose the
  outcome through the audio or model name it's given.
- **Email** goes to a small SMTP server on a real socket (`harness/mock_smtp.rs`).
- **WebSockets** need a real socket: `app.listen()` serves the router on one.
- **Silence detection** needs `ffmpeg`.

## The mobile app against this server

`mobile_contract.rs` boots the server, seeds it, and runs `mobile/test/contract/` with `flutter test`,
so the app's API client is tested against real responses. It needs Flutter, so it's ignored by default:

```sh
scripts/integration-tests.sh --db postgres -- -- --ignored mobile_contract
```

## Not covered yet

External services are stood in for rather than called: real ntfy/Gotify servers, real OIDC providers,
real whisper and LLM models. On-device playback (CarPlay, Android Auto, Bluetooth, background audio)
needs hardware.
