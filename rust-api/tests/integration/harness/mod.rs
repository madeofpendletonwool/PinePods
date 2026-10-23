//! Integration-test harness. See `tests/README.md` for how the suite is wired and run.
#![allow(dead_code)] // shared helpers; each test module uses a different subset

pub mod antennapod;
pub mod app;
pub mod env;
pub mod fixtures;
pub mod flows;
pub mod gpodder;
pub mod http;
pub mod mock_gpodder;
pub mod rss;
pub mod sql;
pub mod youtube;

#[allow(unused_imports)]
pub use app::{hash_password, TestApp, TestUser};
#[allow(unused_imports)]
pub use env::Backend;
#[allow(unused_imports)]
pub use fixtures::Fixture;
#[allow(unused_imports)]
pub use rss::{Episode, Feed};
#[allow(unused_imports)]
pub use flows::eventually;
#[allow(unused_imports)]
pub use serde_json::{json, Value};
#[allow(unused_imports)]
pub use sql::{exec, int, rerun_migrations, set_next_refresh, set_refresh_state, text};
