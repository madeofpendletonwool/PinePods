use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
async fn boots_and_logs_in() {
    let app = TestApp::new().await;
    app.get("/api/pinepods_check").send().await.assert_ok();
    let admin = app.admin().await;
    let body = app.get("/api/data/verify_key").key(admin).send().await.ok_json();
    assert_eq!(body["status"], "success", "{body}");
    let alice = app.user("alice").await;
    assert_ne!(alice.id, admin.id);
}
