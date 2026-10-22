//! Multi-device sync scenarios: what one device must see after another device's changes.

use std::time::Duration;

use super::{actions_for, stalled_host, strings};
use crate::harness::antennapod::*;
use crate::harness::gpodder::*;
use crate::harness::*;
use chrono::Utc;

const POD: &str = "https://feeds.example.test/show.xml";

fn ep(n: usize) -> String {
    format!("https://cdn.example.test/show/ep{n}.mp3")
}

// ---- cursors -----------------------------------------------------------------------------

/// A device that was offline uploads an action stamped an hour ago, after another device has
/// already synced. The other device's next sync must still get it.
#[tokio::test(flavor = "multi_thread")]
async fn late_upload_reaches_other_devices() {
    conformance("late upload", Known::None, async {
        let s = GpodderScene::start().await;
        let phone = s.device("phone").await;
        let tablet = s.device("tablet").await;
        let cursor_b = cursor(&tablet.episode_actions(0).await.ok_json());

        // Clear of the cursor's second even if the server rounds it up (the Go one does).
        tokio::time::sleep(Duration::from_millis(2100)).await;
        let an_hour_ago = Utc::now() - chrono::Duration::hours(1);
        phone.upload_episode_actions(&[play_action(POD, &ep(1), an_hour_ago, 0, 300, 1800)]).await;

        let body = tablet.episode_actions(cursor_b).await.ok_json();
        assert_eq!(actions_for(&body, &ep(1)).len(), 1, "offline play never reaches the tablet: {body}");
    })
    .await;
}

/// An action uploaded in the same second the other device downloaded must not be lost to the
/// cursor boundary.
#[tokio::test(flavor = "multi_thread")]
async fn same_second_episode_upload_is_not_lost() {
    conformance("same-second actions", Known::None, async {
        let s = GpodderScene::start().await;
        let phone = s.device("phone").await;
        let tablet = s.device("tablet").await;
        align_to_second_start().await;
        let c = cursor(&tablet.episode_actions(0).await.ok_json());
        phone.upload_episode_actions(&[play_action(POD, &ep(1), Utc::now(), 0, 300, 1800)]).await;

        let body = tablet.episode_actions(c).await.ok_json();
        assert_eq!(actions_for(&body, &ep(1)).len(), 1, "same-second upload lost: {body}");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn same_second_subscription_change_is_not_lost() {
    conformance("same-second subscriptions", Known::None, async {
        let s = GpodderScene::start().await;
        let feed = s.app.feed("same-second.xml", &Feed::with_episodes("Same second", 1));
        let phone = s.device("phone").await;
        let tablet = s.device("tablet").await;
        let first = tablet.subscription_changes(0).await.ok_json();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        align_to_second_start().await;
        let c = cursor(&tablet.subscription_changes(cursor(&first)).await.ok_json());
        phone.upload_subscription_changes(&[&feed], &[]).await.assert_ok();

        let body = tablet.subscription_changes(c).await.ok_json();
        assert!(strings(&body, "add").contains(&feed), "same-second subscribe lost: {body}");
    })
    .await;
}

/// AntennaPod downloads, then uploads, and keeps the *upload's* timestamp as its next cursor.
/// A change another device makes in between must still reach it.
#[tokio::test(flavor = "multi_thread")]
async fn change_between_download_and_upload_still_arrives() {
    conformance("download-upload window", Known::Go, async {
        let s = GpodderScene::start().await;
        let phone = s.device("phone").await;
        let tablet = s.device("tablet").await;

        // Episodes.
        tablet.episode_actions(0).await.assert_ok();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        phone.upload_episode_actions(&[play_action(POD, &ep(1), Utc::now(), 0, 300, 1800)]).await;
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let up = tablet.upload_episode_actions(&[play_action(POD, &ep(2), Utc::now(), 0, 10, 1800)]).await;
        let next = cursor(&up.last().unwrap().json());
        let body = tablet.episode_actions(next).await.ok_json();
        assert_eq!(actions_for(&body, &ep(1)).len(), 1, "phone's action fell between tablet's download and upload: {body}");

        // Subscriptions.
        let feed = s.app.feed("window.xml", &Feed::with_episodes("Window", 1));
        let own = s.app.feed("own.xml", &Feed::with_episodes("Own", 1));
        let first = tablet.subscription_changes(0).await.ok_json();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        tablet.subscription_changes(cursor(&first)).await.assert_ok();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        phone.upload_subscription_changes(&[&feed], &[]).await.assert_ok();
        tokio::time::sleep(Duration::from_millis(1100)).await;
        let up = tablet.upload_subscription_changes(&[&own], &[]).await.ok_json();
        let body = tablet.subscription_changes(cursor(&up)).await.ok_json();
        assert!(strings(&body, "add").contains(&feed), "phone's subscribe fell between tablet's download and upload: {body}");
    })
    .await;
}

// ---- subscription folding ----------------------------------------------------------------

#[tokio::test(flavor = "multi_thread")]
async fn unsubscribe_then_resubscribe_shows_only_add() {
    conformance("fold", Known::None, async {
        let s = GpodderScene::start().await;
        let feed = s.app.feed("flip.xml", &Feed::with_episodes("Flip", 1));
        let phone = s.device("phone").await;
        let tablet = s.device("tablet").await;
        let c = cursor(&tablet.subscription_changes(0).await.ok_json());
        tokio::time::sleep(Duration::from_millis(1100)).await;

        phone.upload_subscription_changes(&[&feed], &[]).await.assert_ok();
        phone.upload_subscription_changes(&[], &[&feed]).await.assert_ok();
        phone.upload_subscription_changes(&[&feed], &[]).await.assert_ok();
        let body = tablet.subscription_changes(c).await.ok_json();
        assert!(strings(&body, "add").contains(&feed), "{body}");
        assert!(!strings(&body, "remove").contains(&feed), "URL in both add and remove: {body}");

        phone.upload_subscription_changes(&[], &[&feed]).await.assert_ok();
        let body = tablet.subscription_changes(c).await.ok_json();
        assert!(strings(&body, "remove").contains(&feed), "{body}");
        assert!(!strings(&body, "add").contains(&feed), "URL in both add and remove: {body}");
    })
    .await;
}

// ---- scale and feed fetching -------------------------------------------------------------

/// AntennaPod's first sync of a big library: 500 subscriptions in one POST, then 5,000
/// actions in POSTs of 30. Every request answers in under 2s, and the server fetches no feed
/// while answering.
#[tokio::test(flavor = "multi_thread")]
async fn big_first_sync_is_fast_and_fetches_nothing_inline() {
    conformance("big first sync", Known::None, async {
        let s = GpodderScene::start().await;
        let phone = s.device("phone").await;
        let feeds: Vec<String> = (0..500).map(|i| s.app.fixtures().url(&format!("big/{i}.xml"))).collect();
        let refs: Vec<&str> = feeds.iter().map(String::as_str).collect();

        let r = phone.upload_subscription_changes(&refs, &[]).await;
        r.assert_ok();
        assert!(r.elapsed < Duration::from_secs(2), "500-subscription upload took {:?}", r.elapsed);
        let fetched: usize = (0..500).map(|i| s.app.fixtures().hits(&format!("big/{i}.xml")).len()).sum();
        assert_eq!(fetched, 0, "the server fetched {fetched} feeds while answering the upload");

        let actions: Vec<_> = (0..5000)
            .map(|i| play_action(&feeds[i % 500], &format!("{}#ep{i}", feeds[i % 500]), Utc::now(), 0, 60, 1800))
            .collect();
        let responses = phone.upload_episode_actions(&actions).await;
        assert_eq!(responses.len(), 167);
        let slowest = responses.iter().map(|r| r.elapsed).max().unwrap();
        assert!(slowest < Duration::from_secs(2), "slowest 30-action batch took {slowest:?}");

        let tablet = s.device("tablet").await;
        let subs = tablet.subscription_changes(0).await;
        assert!(subs.elapsed < Duration::from_secs(2), "subscription download took {:?}", subs.elapsed);
        assert_eq!(strings(&subs.ok_json(), "add").len(), 500);
        let eps = tablet.episode_actions(0).await;
        assert!(eps.elapsed < Duration::from_secs(3), "episode download took {:?}", eps.elapsed);
        assert_eq!(eps.ok_json()["actions"].as_array().unwrap().len(), 5000);
    })
    .await;
}

/// A subscription upload never waits on the feed host: here every host hangs forever.
#[tokio::test(flavor = "multi_thread")]
async fn subscription_upload_never_waits_on_feed_hosts() {
    conformance("no inline fetch", Known::None, async {
        let s = GpodderScene::start().await;
        let host = stalled_host().await;
        let feeds: Vec<String> = (0..3).map(|i| format!("{host}/feed{i}.xml")).collect();
        let refs: Vec<&str> = feeds.iter().map(String::as_str).collect();
        let phone = s.device("phone").await;
        let r = tokio::time::timeout(Duration::from_secs(5), phone.upload_subscription_changes(&refs, &[]))
            .await
            .expect("subscription upload blocked on hanging feed hosts for 5s");
        r.assert_ok();
        assert!(r.elapsed < Duration::from_secs(2), "upload took {:?}", r.elapsed);
    })
    .await;
}

// ---- transactions ------------------------------------------------------------------------

/// A batch with an action the database can't store is handled atomically, and nothing the
/// failed batch left behind (open transactions, held connections, row locks) blocks the next
/// upload. Twelve failures in a row outlast any reasonably sized connection pool.
#[tokio::test(flavor = "multi_thread")]
async fn failed_batch_rolls_back_and_blocks_nothing() {
    conformance("rollback", Known::None, async {
        let s = GpodderScene::start().await;
        // A real Pinepods episode, so servers that mirror plays into listen history touch it.
        let feed_url = s.app.feed("rollback.xml", &Feed::with_episodes("Rollback", 1));
        let episode_url = "https://cdn.example.invalid/audio/ep1.mp3";
        if s.has_library() {
            s.app.subscribe(&s.user, &feed_url, 1).await;
        }
        let phone = s.device("phone").await;

        let good = play_action(&feed_url, episode_url, Utc::now(), 0, 100, 1800);
        let poison = play_action(&feed_url, &ep(99), Utc::now(), 0, 3_000_000_000, 1800);
        let mut first = None;
        for i in 0..12 {
            let r = tokio::time::timeout(Duration::from_secs(10), phone.upload_episode_batch(&[good.clone(), poison.clone()]))
                .await
                .unwrap_or_else(|_| panic!("failed batch #{} hung: earlier failed batches are still holding the server's connections", i + 1));
            first.get_or_insert(r);
        }
        let first = first.unwrap();
        if s.has_library() {
            let open = eventually("failed batches to be rolled back", || async {
                let n = s.app.abandoned_transactions().await;
                (n == 0).then_some(n)
            });
            tokio::time::timeout(Duration::from_secs(5), open)
                .await
                .unwrap_or_else(|_| panic!("failed batches left transactions open on the database (each holds a connection and its locks)"));
        }

        let second = tokio::time::timeout(
            Duration::from_secs(10),
            phone.upload_episode_batch(&[play_action(&feed_url, episode_url, Utc::now(), 0, 200, 1800)]),
        )
        .await
        .expect("upload after a failed batch hung (the failed transaction is still holding locks)");
        second.assert_ok();

        let body = s.device("tablet").await.episode_actions(0).await.ok_json();
        let positions: Vec<i64> = actions_for(&body, episode_url).iter().filter_map(|a| a["position"].as_i64()).collect();
        assert!(actions_for(&body, &ep(99)).is_empty(), "the unstorable action was stored: {body}");
        if (200..300).contains(&first.status) {
            assert!(positions.contains(&100), "batch accepted but its valid action is missing: {body}");
        } else {
            assert!(!positions.contains(&100), "batch rejected ({}) but part of it was stored: {body}", first.status);
        }
        assert!(positions.contains(&200), "{body}");
    })
    .await;
}

// ---- PUT semantics -----------------------------------------------------------------------

/// A device PUTting its own (short) list only changes what that device reported: other
/// devices' subscriptions survive.
///
/// The Go service passes this only because the tablet's subscriptions are still pending (not
/// yet imported into Podcasts, which its PUT diffs against). Once Pinepods imports them it
/// fails the same way as `put_keeps_podcasts_added_in_pinepods` (G5, fixed by the Phase 5 port).
#[tokio::test(flavor = "multi_thread")]
async fn put_from_one_device_keeps_other_devices_subscriptions() {
    conformance("put", Known::None, async {
        let s = GpodderScene::start().await;
        let a = s.app.feed("put-a.xml", &Feed::with_episodes("Put A", 1));
        let b = s.app.feed("put-b.xml", &Feed::with_episodes("Put B", 1));
        let c = s.app.fixtures().url("put-c.xml");
        let tablet = s.device("tablet").await;
        tablet.upload_subscription_changes(&[&a, &b], &[]).await.assert_ok();

        let phone = s.device("phone").await;
        phone.put(&format!("/api/2/subscriptions/{}/phone.json", s.user.username), &json!([c])).await.assert_ok();

        let all = strings(&s.device("laptop").await.subscription_changes(0).await.ok_json(), "add");
        for url in [&a, &b] {
            assert!(all.contains(url), "{url} was removed by another device's PUT: {all:?}");
        }
    })
    .await;
}

/// The same for podcasts added in Pinepods itself, which no gpodder device ever reported.
#[tokio::test(flavor = "multi_thread")]
async fn put_keeps_podcasts_added_in_pinepods() {
    conformance("put library", Known::Go, async {
        let s = GpodderScene::start().await;
        if !s.has_library() {
            return;
        }
        let a = s.app.feed("lib-a.xml", &Feed::with_episodes("Library A", 2));
        let pod = s.app.subscribe(&s.user, &a, 2).await;
        let ep = s.app.episode_id(&s.user, pod, "Episode 1").await;
        s.app
            .post("/api/data/save_episode")
            .key(&s.user)
            .json(&json!({ "episode_id": ep, "user_id": s.user.id, "is_youtube": false }))
            .send()
            .await
            .assert_ok();

        let phone = s.device("phone").await;
        let c = s.app.fixtures().url("lib-c.xml");
        for path in [format!("/api/2/subscriptions/{}/phone.json", s.user.username), format!("/subscriptions/{}/phone.json", s.user.username)] {
            let r = phone.put(&path, &json!([c])).await;
            assert!(r.status < 300, "PUT {path}: {} {}", r.status, r.body);
        }

        let pods = s.app.podcasts(&s.user).await;
        assert!(pods.iter().any(|p| p["feedurl"] == a.as_str()), "Pinepods podcast deleted by a device PUT: {pods:#?}");
        let saved = s.app.get(&format!("/api/data/saved_episode_list/{}", s.user.id)).key(&s.user).send().await.ok_json();
        assert!(saved.to_string().contains("Episode 1"), "saved episode lost: {saved}");
    })
    .await;
}

// ---- devices -----------------------------------------------------------------------------

/// An episode upload naming a device the server has never seen creates that device.
#[tokio::test(flavor = "multi_thread")]
async fn episode_upload_creates_unknown_device() {
    conformance("device auto-create", Known::None, async {
        let s = GpodderScene::start().await;
        let car = s.device("car").await;
        car.upload_episode_actions(&[play_action(POD, &ep(1), Utc::now(), 0, 30, 1800)]).await;
        let devices = car.devices().await.ok_json();
        assert_devices(&devices);
        assert!(devices.as_array().unwrap().iter().any(|d| d["id"] == "car"), "{devices}");
    })
    .await;
}
