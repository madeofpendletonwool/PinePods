//! The SQLite schema baseline is generated from the PostgreSQL migrations; it must be
//! regenerated whenever they change.

use std::process::Command;

use crate::harness::*;

#[tokio::test]
async fn sqlite_baseline_matches_the_migrations() {
    let app = TestApp::new().await;
    if app.backend() != Backend::Postgres {
        return;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let python = std::env::var("PINEPODS_TEST_PYTHON").unwrap_or_else(|_| "python3".into());
    let out = Command::new(&python)
        .args(["-m", "database_functions.sqlite_baseline", "--check"])
        .arg(env::pg_url(env::env(), &app.db_name))
        .current_dir(root)
        .output()
        .expect("run the baseline generator");
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
