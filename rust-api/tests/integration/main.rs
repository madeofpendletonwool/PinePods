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
mod moved_feeds;
mod opml;
mod people;
mod playback;
mod podcasting20;
mod podcasts;
mod podping;
mod queue;
mod refresh;
mod refresh_schedule;
mod scheduling;
mod settings;
mod shared_episodes;
mod shared_feeds;
mod smoke;
mod sync_core;
mod sync_engine;
mod sync_migrations;
mod sync_settings;
mod user_episode_state;
