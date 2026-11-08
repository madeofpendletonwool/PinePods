//! In-app notifications: kept in the database, listed, marked read, dismissed and cleared.

use pinepods_api::services::task_manager::MsgSeverity;

use crate::harness::*;

async fn push(app: &TestApp, user: &TestUser, title: &str, severity: MsgSeverity) -> String {
    app.state
        .task_manager
        .push_notification(user.id, "test", severity, title, Some("body".into()), None, None, None, None)
        .await
        .unwrap()
        .id
}

async fn listed(app: &TestApp, user: &TestUser) -> Vec<Value> {
    let all = app.get(&format!("/api/data/notifications?user_id={}", user.id)).key(user).send().await.ok_json();
    all.as_array().unwrap().clone()
}

fn titles(list: &[Value]) -> Vec<&str> {
    list.iter().map(|n| n["title"].as_str().unwrap()).collect()
}

#[tokio::test]
async fn notifications_are_listed_read_dismissed_and_cleared() {
    let app = TestApp::new().await;
    let user = app.user("notified").await;
    let other = app.user("bystander").await;

    let first = push(&app, &user, "first", MsgSeverity::Info).await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let second = push(&app, &user, "second", MsgSeverity::Error).await;
    push(&app, &other, "theirs", MsgSeverity::Info).await;

    let list = listed(&app, &user).await;
    assert_eq!(titles(&list), ["second", "first"], "newest first, only this user's");
    assert!(list.iter().all(|n| n["read"] == false));

    let read = |id: &str| format!("/api/data/notifications/{id}/read?user_id={}", user.id);
    app.post(&read(&first)).key(&user).send().await.assert_ok();
    let list = listed(&app, &user).await;
    assert_eq!(list.iter().find(|n| n["id"] == first.as_str()).unwrap()["read"], true);

    app.post(&format!("/api/data/notifications/clear?user_id={}&read_only=true", user.id)).key(&user).send().await.assert_ok();
    assert_eq!(titles(&listed(&app, &user).await), ["second"]);

    // Expired ones aren't listed.
    exec(&app, &format!(r#"UPDATE "InAppNotifications" SET ExpiresAt = '2000-01-01 00:00:00' WHERE NotificationID = '{second}'"#)).await;
    assert!(listed(&app, &user).await.is_empty());

    let third = push(&app, &user, "third", MsgSeverity::Warning).await;
    app.post(&format!("/api/data/notifications/{third}/dismiss?user_id={}", user.id)).key(&user).send().await.assert_ok();
    assert!(listed(&app, &user).await.is_empty());

    push(&app, &user, "fourth", MsgSeverity::Success).await;
    app.post(&format!("/api/data/notifications/clear?user_id={}&read_only=false", user.id)).key(&user).send().await.assert_ok();
    assert!(listed(&app, &user).await.is_empty());
    assert_eq!(titles(&listed(&app, &other).await), ["theirs"]);
}
