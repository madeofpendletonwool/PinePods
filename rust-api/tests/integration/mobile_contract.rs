//! Runs the mobile app's API client against this server: boots it on a real socket, seeds a
//! user and a show, then runs `mobile/test/contract/` with `flutter test`.

use crate::harness::files::mp3_bytes;
use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs Flutter; see tests/README.md"]
async fn mobile_contract() {
    let app = TestApp::new().await;
    let port = app.listen().await;
    let user = app.user("mobile").await;

    let title = format!("Contract Show {}", app.tag);
    let mut feed = Feed::new(&title);
    for n in 1..=3 {
        let audio = app.fixtures().serve(&format!("audio/{n}.mp3"), Fixture::new("audio/mpeg", mp3_bytes()));
        feed = feed.episode(Episode::new(n).enclosure(&audio));
    }
    let pod = app.subscribe(&user, &app.feed("show.xml", &feed), 3).await;

    let mobile = crate::harness::env::repo_root().join("mobile");
    let output = tokio::process::Command::new("flutter")
        .args(["test", "test/contract/server_contract_test.dart", "--reporter", "expanded"])
        .current_dir(&mobile)
        .env("PINEPODS_CONTRACT_SERVER", format!("http://127.0.0.1:{port}"))
        .env("PINEPODS_CONTRACT_USERNAME", &user.username)
        .env("PINEPODS_CONTRACT_PASSWORD", &user.password)
        .env("PINEPODS_CONTRACT_USER_ID", user.id.to_string())
        .env("PINEPODS_CONTRACT_PODCAST_ID", pod.to_string())
        .env("PINEPODS_CONTRACT_SHOW_TITLE", &title)
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("http_proxy")
        .env_remove("https_proxy")
        .env_remove("all_proxy")
        .output()
        .await
        .expect("run flutter test");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "contract tests failed:\n{stdout}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!stdout.contains("skipped"), "the tests should have run against the server:\n{stdout}");
}
