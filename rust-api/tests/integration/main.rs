//! rust-api integration suite: every test boots the real router against its own freshly
//! migrated database (Postgres or MariaDB) plus Valkey. Run with `scripts/integration-tests.sh`.

mod harness;
mod support;

mod admin_only;
mod auth_coverage;
mod cross_user;
mod foreign_resources;
mod library;
mod playback;
mod podcasts;
mod queue;
mod smoke;
