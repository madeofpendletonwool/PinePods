//! The listening report: totals, top lists, sessions, downloads and time saved for a period or
//! all time, from what the apps report.

use crate::harness::*;

async fn report(app: &TestApp, user: &TestUser, period: Option<(&str, &str)>) -> Value {
    let mut req = app.get("/api/data/listening_report").key(user).query("user_id", user.id);
    if let Some((start, end)) = period {
        req = req.query("start", start).query("end", end);
    }
    req.send().await.ok_json()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_report_covers_what_was_played() {
    let app = TestApp::new().await;
    let user = app.user("reporter").await;
    let url = app.feed("report.xml", &Feed::with_episodes("Report Show", 2));
    let pod = app.subscribe(&user, &url, 2).await;
    let (e1, e2) = (app.episode_id(&user, pod, "Episode 1").await, app.episode_id(&user, pod, "Episode 2").await);
    let now: i64 = 2_000_000_000_000;
    let send = |updates: Value| {
        let (app, user) = (&app, &user);
        async move {
            app.post("/api/data/progress").key(user).json(&json!({ "sent_at": now, "client": "android", "updates": updates })).send().await.assert_ok();
        }
    };
    // Two hours ago, one stretch; then two episodes back to back just now.
    send(json!([{ "episode_id": e1, "position": 60.0, "listened": 60.0, "at": now - 7_200_000 }])).await;
    send(json!([{ "episode_id": e1, "position": 360.0, "listened": 300.0, "speed_saved": 60.0, "downloaded": true, "at": now - 130_000 }])).await;
    send(json!([{ "episode_id": e2, "position": 120.0, "listened": 120.0, "at": now }])).await;

    let all = report(&app, &user, None).await;
    assert_eq!(all["seconds"], 480, "{all}");
    assert_eq!((all["episodes"].clone(), all["podcasts"].clone()), (json!(2), json!(1)));
    assert_eq!(all["top_podcasts"][0]["name"], "Report Show");
    assert_eq!(all["top_podcasts"][0]["podcast_id"], pod);
    assert_eq!(all["top_episodes"][0]["episode_id"], e1);
    assert_eq!(all["top_categories"][0]["name"], "Technology");
    assert_eq!(all["sessions"]["count"], 2, "a two-hour gap ends a session: {all}");
    assert_eq!(all["sessions"]["longest_seconds"], 420);
    assert_eq!(all["downloaded_seconds"], 300);
    assert_eq!(all["average_speed"], json!(1.13));
    assert_eq!(all["by_client"][0], json!({ "name": "android", "seconds": 480 }));
    assert_eq!(all["release_lag"]["episodes"], 2, "{all}");
    let month = all["months"][0].as_str().unwrap().to_string();

    // A period: the month that was logged, and one with nothing in it.
    let (y, m) = month.split_once('-').unwrap();
    let last = chrono::NaiveDate::from_ymd_opt(y.parse().unwrap(), m.parse().unwrap(), 1).unwrap().checked_add_months(chrono::Months::new(1)).unwrap().pred_opt().unwrap();
    let start = format!("{month}-01");
    let end = last.format("%Y-%m-%d").to_string();
    let this_month = report(&app, &user, Some((&start, &end))).await;
    assert_eq!(this_month["seconds"], 480);
    assert_eq!(this_month["daily"].as_array().unwrap().iter().map(|d| d["seconds"].as_i64().unwrap()).sum::<i64>(), 480);
    let empty = report(&app, &user, Some(("2001-01-01", "2001-12-31"))).await;
    assert_eq!((empty["seconds"].clone(), empty["sessions"]["count"].clone()), (json!(0), json!(0)));
    assert_eq!(empty["by_month"].as_array().unwrap().len(), 12);

    // Bad periods and other users.
    let base = || app.get("/api/data/listening_report").key(&user).query("user_id", user.id);
    base().query("start", "2026-01-01").send().await.assert_status(400);
    base().query("start", "2026-02-01").query("end", "2026-01-01").send().await.assert_status(400);
    let other = app.user("nosy").await;
    app.get("/api/data/listening_report").key(&other).query("user_id", user.id).send().await.assert_status(403);

    // The episode's podcast removed: still in the report, without a link.
    app.post("/api/data/remove_podcast_id").key(&user).json(&json!({ "podcast_id": pod, "user_id": user.id })).send().await.assert_ok();
    let after = report(&app, &user, None).await;
    assert_eq!((after["seconds"].clone(), after["top_podcasts"][0]["name"].clone()), (json!(480), json!("Report Show")));
    assert!(after["top_podcasts"][0]["podcast_id"].is_null(), "{after}");
}
