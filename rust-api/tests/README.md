# rust-api integration tests

Every test boots the real axum router (`pinepods_api::create_app`) against its **own freshly
migrated database** (Postgres or MariaDB) plus Valkey, and drives it over HTTP exactly as the
web and mobile clients do. Nothing is mocked except the outside internet.

## Running

```sh
scripts/integration-tests.sh                    # both backends, every test
scripts/integration-tests.sh --db postgres      # one backend (postgres | mariadb)
scripts/integration-tests.sh -- queue           # extra args go to `cargo test` (name filter)
scripts/integration-tests.sh -- accounts -- --nocapture
scripts/integration-tests.sh down               # stop the throwaway containers
scripts/integration-tests.sh --gpodder go -- gpodder   # gpodder protocol tests (go | rust | mock)
```

The script starts Postgres, MariaDB and Valkey from `tests/docker-compose.yml` (off-default
ports, tmpfs storage) and builds a Python venv under `target/` for the migration runner. CI
(`.github/workflows/rust-api-tests.yml`) runs the same thing as a matrix over both databases,
using service containers (`PINEPODS_TEST_EXTERNAL_SERVICES=1`).

Useful environment variables:

| Variable | Meaning |
|---|---|
| `PINEPODS_TEST_DB_TYPE` | `postgresql` or `mariadb` (set by the script) |
| `PINEPODS_TEST_DB_HOST` / `_PORT` / `_USER` / `_PASSWORD` | database server |
| `PINEPODS_TEST_VALKEY_HOST` / `_PORT` | Valkey server |
| `PINEPODS_TEST_PYTHON` | Python used to run `startup/setup_database_new.py` |
| `PINEPODS_TEST_KEEP_DB` | keep each test's database afterwards for inspection |
| `PINEPODS_TEST_EXTERNAL_SERVICES` | don't start containers; use the ones above |
| `PINEPODS_TEST_GPODDER` | gpodder server the protocol tests talk to: `go` (default), `rust`, `mock` |
| `PINEPODS_TEST_GPODDER_BIN` | prebuilt `gpodder-api` binary instead of `go build` |
| `PINEPODS_TEST_GPODDER_STRICT` | fail on known gpodder failures instead of recording them |

## How it works

- **Schema.** On first use the harness runs the real migrations (`startup/setup_database_new.py`)
  once into a template database named after a hash of the migration files, so a migration change
  rebuilds it automatically. Each test then clones the template (`CREATE DATABASE … TEMPLATE` on
  Postgres, a table copy on MariaDB) and drops its clone when the `TestApp` is dropped.
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
  feeds.rs, opml.rs, podcasting20.rs   feature behaviour
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

- `go` (default): the current `gpodder-api/` service, built with `go build` and started per
  test against that test's database, so Go must be installed.
- `rust`: the rust-api router on a real socket, for the Rust port.
- `mock`: the in-memory reference server in `harness/mock_gpodder.rs`. It follows the
  cursor rules, so every test must pass against it; that's how the tests themselves are
  checked. It also stands in for external gpodder/Nextcloud servers in sync engine tests.

`harness/antennapod.rs` holds AntennaPod's response parsing as assertions: a `play` action
without `started >= 0`, `position > 0` and `total > 0` resets the episode to 0 there.

Each test names the finding it covers and where it is known to fail (`Known::Go`,
`Known::GoMariaDb`). A known failure is printed and appended to
`target/gpodder-conformance/<server>-<db>.txt` instead of failing the run (the script prints
these at the end); a known failure that starts passing fails the test, so the entry gets
removed. `PINEPODS_TEST_GPODDER_STRICT=1` shows the real failures.

## Not covered yet

Downloads and backups (paths under `/opt/pinepods` are hard-coded), the translations list
(`/var/www/html/static/translations`), Pinepods' own gpodder/Nextcloud sync client, and
YouTube channels.
