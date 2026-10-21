//! rust-api integration suite: every test boots the real router against its own freshly
//! migrated database (Postgres or MariaDB) plus Valkey. Run with `scripts/integration-tests.sh`.

mod harness;
mod support;

mod accounts;
mod admin_only;
mod auth_coverage;
mod cross_user;
mod feeds;
mod foreign_resources;
mod library;
mod opml;
mod playback;
mod podcasts;
mod queue;
mod settings;
mod smoke;
