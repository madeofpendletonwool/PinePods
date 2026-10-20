//! Integration-test harness. See `tests/README.md` for how the suite is wired and run.
#![allow(dead_code)] // shared helpers; each test module uses a different subset

pub mod app;
pub mod env;
pub mod fixtures;
pub mod http;
pub mod rss;

#[allow(unused_imports)]
pub use app::{hash_password, TestApp, TestUser};
#[allow(unused_imports)]
pub use env::Backend;
#[allow(unused_imports)]
pub use fixtures::Fixture;
#[allow(unused_imports)]
pub use rss::{Episode, Feed};
