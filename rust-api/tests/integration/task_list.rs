//! Activity tasks: finished ones can be dismissed or cleared on the server, queued ones
//! cancelled, and tasks whose process is gone are failed instead of lingering.

use crate::harness::*;

async fn tasks(app: &TestApp, user: &TestUser) -> Vec<Value> {
    let all = app.get(&format!("/api/tasks/user/{}", user.id)).key(user).send().await.ok_json();
    all.as_array().unwrap().clone()
}

fn status_of<'a>(list: &'a [Value], id: &str) -> Option<&'a str> {
    list.iter().find(|t| t["id"] == id).map(|t| t["status"].as_str().unwrap())
}

#[tokio::test]
async fn finished_tasks_are_dismissed_and_cleared_on_the_server() {
    let app = TestApp::new().await;
    let user = app.user("tasks_done").await;
    let other = app.user("tasks_other").await;
    let tm = &app.state.task_manager;

    let done = tm.create_task("feed_refresh".into(), user.id).await.unwrap();
    tm.complete_task(&done, None, None).await.unwrap();
    let failed = tm.create_task("feed_refresh".into(), user.id).await.unwrap();
    tm.fail_task(&failed, "nope".into()).await.unwrap();
    let running = tm.create_task("feed_refresh".into(), user.id).await.unwrap();
    tm.update_task_progress(&running, 10.0, None).await.unwrap();

    // Someone else's task can't be touched, and an unfinished one can't be dismissed.
    app.post(&format!("/api/tasks/{done}/dismiss")).key(&other).send().await.assert_status(404);
    app.post(&format!("/api/tasks/{running}/dismiss")).key(&user).send().await.assert_status(400);

    app.post(&format!("/api/tasks/{done}/dismiss")).key(&user).send().await.assert_ok();
    let list = tasks(&app, &user).await;
    assert_eq!(status_of(&list, &done), None, "dismissed task stays gone after a refetch");
    assert_eq!(status_of(&list, &failed), Some("FAILED"));

    let removed = app.post("/api/tasks/clear_finished").key(&user).send().await.ok_json();
    assert_eq!(removed["removed"], 1);
    let list = tasks(&app, &user).await;
    assert_eq!(list.len(), 1);
    assert_eq!(status_of(&list, &running), Some("DOWNLOADING"));
}

#[tokio::test]
async fn only_queued_transcriptions_can_be_cancelled() {
    let app = TestApp::new().await;
    let user = app.user("tasks_cancel").await;
    let tm = &app.state.task_manager;

    let queued = tm.create_task_with_item_id("transcribe_episode".into(), user.id, Some(1)).await.unwrap();
    let refresh = tm.create_task("feed_refresh".into(), user.id).await.unwrap();

    app.post(&format!("/api/tasks/{refresh}/cancel")).key(&user).send().await.assert_status(400);
    assert!(tm.still_queued(&queued).await);

    app.post(&format!("/api/tasks/{queued}/cancel")).key(&user).send().await.assert_ok();
    assert!(!tm.still_queued(&queued).await, "the bulk runner skips it");
    let list = tasks(&app, &user).await;
    assert_eq!(status_of(&list, &queued), Some("FAILED"));
    app.post(&format!("/api/tasks/{queued}/cancel")).key(&user).send().await.assert_status(400);
}

#[tokio::test]
async fn tasks_left_behind_by_a_dead_process_are_failed() {
    let app = TestApp::new().await;
    let user = app.user("tasks_orphan").await;
    let redis = &app.state.redis_client;

    let orphan = |id: &str, owner: Option<&str>| {
        serde_json::json!({
            "id": id, "task_type": "transcribe_episode", "user_id": user.id, "status": "PENDING",
            "progress": 0.0, "message": "Queued", "created_at": chrono::Utc::now(),
            "updated_at": chrono::Utc::now(), "result": null, "owner": owner,
        })
        .to_string()
    };
    for (id, owner) in [("orphan-gone", Some("instance-that-exited")), ("orphan-legacy", None)] {
        redis.set_ex(&format!("task:{id}"), orphan(id, owner), 3600).await.unwrap();
        redis.sadd(&format!("task:index:{}", user.id), id).await.unwrap();
    }
    let live = app.state.task_manager.create_task("feed_refresh".into(), user.id).await.unwrap();

    let list = tasks(&app, &user).await;
    assert_eq!(status_of(&list, "orphan-gone"), Some("FAILED"));
    assert_eq!(status_of(&list, "orphan-legacy"), Some("FAILED"));
    assert_eq!(status_of(&list, &live), Some("PENDING"), "this process's own task is left alone");
}
