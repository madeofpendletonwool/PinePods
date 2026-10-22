//! rust-api integration suite: every test boots the real router against its own freshly
//! migrated database (Postgres or MariaDB) plus Valkey. Run with `scripts/integration-tests.sh`.

mod harness;
mod support;

mod accounts;
mod admin_only;
mod auth_coverage;
mod cross_user;
mod feed_health;
mod feeds;
mod foreign_resources;
mod gpodder;
mod gpodder_server;
mod library;
mod opml;
mod people;
mod playback;
mod podcasting20;
mod podcasts;
mod queue;
mod refresh;
mod scheduling;
mod settings;
mod smoke;
mod sync_core;
mod sync_engine;
mod sync_migrations;
mod sync_settings;
