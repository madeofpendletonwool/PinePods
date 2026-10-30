//! The mock's Nextcloud side and failure injection, which the external sync engine tests
//! rely on. These run regardless of `PINEPODS_TEST_GPODDER`.

use crate::harness::gpodder::GpodderClient;
use crate::harness::mock_gpodder::MockGpodder;
use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
async fn mock_nextcloud_round_trip() {
    let mock = MockGpodder::start().await;
    mock.add_user("nc", "pw");
    let c = GpodderClient::new(&mock.base_url, "nc", "pw", "unused");
    let auth = || Some(GpodderClient::basic_header("nc", "pw"));
    let base = "/index.php/apps/gpoddersync";

    let r = c
        .send(reqwest::Method::POST, &format!("{base}/subscription_change/create"), Some(&json!({ "add": ["https://a.example/feed"], "remove": [] })), auth())
        .await;
    r.assert_ok();
    let actions = json!([{ "podcast": "https://a.example/feed", "episode": "https://a.example/1.mp3", "action": "play",
                            "timestamp": "2026-01-02T03:04:05", "started": 0, "position": 60, "total": 600 }]);
    c.send(reqwest::Method::POST, &format!("{base}/episode_action/create"), Some(&actions), auth()).await.assert_ok();

    let subs = c.send(reqwest::Method::GET, &format!("{base}/subscriptions?since=0"), None, auth()).await.ok_json();
    assert_eq!(subs["add"], json!(["https://a.example/feed"]), "{subs}");
    let eps = c.send(reqwest::Method::GET, &format!("{base}/episode_action?since=0"), None, auth()).await.ok_json();
    assert_eq!(eps["actions"][0]["position"], 60, "{eps}");

    c.send(reqwest::Method::GET, &format!("{base}/subscriptions?since=0"), None, Some(GpodderClient::basic_header("nc", "wrong")))
        .await
        .assert_status(401);
    assert_eq!(mock.subscriptions("nc"), vec!["https://a.example/feed".to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn mock_failure_injection() {
    let mock = MockGpodder::start().await;
    mock.add_user("u", "pw");
    let c = GpodderClient::new(&mock.base_url, "u", "pw", "dev");
    c.login().await.assert_ok();
    mock.fail_next("/api/2/episodes", 503, 1);
    c.episode_actions(0).await.assert_status(503);
    c.episode_actions(0).await.assert_ok();
    let log = mock.requests();
    assert_eq!(log.iter().filter(|r| r.path.starts_with("/api/2/episodes")).map(|r| r.status).collect::<Vec<_>>(), vec![503, 200]);
}
