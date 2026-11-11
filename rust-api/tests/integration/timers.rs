//! The real job scheduler: its cron timers fire and do their work on their own.

use std::sync::Arc;
use std::time::{Duration, Instant};

use pinepods_api::services::scheduler::BackgroundScheduler;

use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
async fn the_scheduler_refreshes_due_feeds_on_its_own() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let feed = Feed::with_episodes("Timed Show", 1);
    let url = app.feed("show.xml", &feed);
    let pod = app.subscribe(&user, &url, 1).await;
    app.feed("show.xml", &feed.episode(Episode::new(2).title("Arrived by timer")));
    set_next_refresh(&app, pod, "2020-01-01 00:00:00").await;

    let scheduler = BackgroundScheduler::new().await.unwrap();
    scheduler.start(Arc::new(app.state.clone())).await.unwrap();

    // The refresh job runs at second 0 of every minute.
    let deadline = Instant::now() + Duration::from_secs(70);
    loop {
        if app.episodes(&user, pod).await.iter().any(|e| e["Episodetitle"] == "Arrived by timer") {
            return;
        }
        assert!(Instant::now() < deadline, "the refresh job never picked the feed up");
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_bad_sync_schedule_does_not_silently_disable_the_others() {
    let app = TestApp::new().await;
    let scheduler = BackgroundScheduler::new().await.unwrap();
    scheduler.start(Arc::new(app.state.clone())).await.expect("every built-in schedule is valid");
}
