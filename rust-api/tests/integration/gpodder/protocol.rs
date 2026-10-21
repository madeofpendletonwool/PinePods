//! Response shapes and auth: what a client gets back for each request, checked against
//! AntennaPod's parser and the gpodder v2 spec.

use super::{actions_for, strings};
use crate::harness::antennapod::*;
use crate::harness::gpodder::*;
use crate::harness::*;
use chrono::{TimeZone, Utc};

fn t0() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap()
}

const POD: &str = "https://feeds.example.test/show.xml";
const EP: &str = "https://cdn.example.test/show/ep1.mp3";

#[tokio::test(flavor = "multi_thread")]
async fn device_list_parses_in_antennapod() {
    conformance("AP devices", Known::None, async {
        let s = GpodderScene::start().await;
        let phone = s.device("phone").await;
        phone.configure_device("Pixel", "mobile").await.assert_ok();
        let body = phone.devices().await.ok_json();
        assert_devices(&body);
        let found = body.as_array().unwrap().iter().any(|d| d["id"] == "phone" && d["caption"] == "Pixel" && d["type"] == "mobile");
        assert!(found, "configured device missing from {body}");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn first_sync_of_an_empty_account_parses() {
    conformance("G8 first sync", Known::None, async {
        let s = GpodderScene::start().await;
        let phone = s.device("phone").await;
        assert_subscription_changes(&phone.subscription_changes(0).await.ok_json());
        assert_episode_actions_response(&phone.episode_actions(0).await.ok_json());
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn play_action_round_trips_for_antennapod() {
    conformance("G11 play round-trip", Known::None, async {
        let s = GpodderScene::start().await;
        let phone = s.device("phone").await;
        let up = phone.upload_episode_actions(&[play_action(POD, EP, t0(), 0, 600, 1800)]).await;
        assert_upload_response(&up[0].json());

        let tablet = s.device("tablet").await;
        let body = tablet.episode_actions(0).await.ok_json();
        assert_episode_actions_response(&body);
        let got = actions_for(&body, EP);
        assert_eq!(got.len(), 1, "{body}");
        assert_eq!(play_position(got[0]), Some(600), "{}", got[0]);
        assert_eq!(action_timestamp(got[0]).unwrap(), Some(t0().naive_utc()), "{}", got[0]);
    })
    .await;
}

/// gPodder desktop and other clients send `play` with only a position; AntennaPod must still
/// resume there rather than reset to 0 (R4, #636).
#[tokio::test(flavor = "multi_thread")]
async fn play_without_started_or_total_still_resumes_in_antennapod() {
    conformance("R4 play fields", Known::None, async {
        let s = GpodderScene::start().await;
        let desktop = s.device("desktop").await;
        let mut a = play_action(POD, EP, t0(), 0, 600, 0);
        a.as_object_mut().unwrap().remove("started");
        a.as_object_mut().unwrap().remove("total");
        desktop.upload_episode_actions(&[a]).await;

        let phone = s.device("phone").await;
        let body = phone.episode_actions(0).await.ok_json();
        let got = actions_for(&body, EP);
        assert_eq!(got.len(), 1, "{body}");
        assert_eq!(play_position(got[0]), Some(600), "AntennaPod would reset the episode: {}", got[0]);
    })
    .await;
}

/// Uploaded timestamps with an offset, or as unix seconds, come back as UTC that AntennaPod
/// reads correctly.
#[tokio::test(flavor = "multi_thread")]
async fn uploaded_timestamps_come_back_as_utc() {
    conformance("G11 timestamps", Known::None, async {
        let s = GpodderScene::start().await;
        let phone = s.device("phone").await;
        let mut offset = play_action(POD, "https://cdn.example.test/show/offset.mp3", t0(), 0, 60, 600);
        offset["timestamp"] = json!("2026-01-02T05:04:05+02:00");
        let mut unix = play_action(POD, "https://cdn.example.test/show/unix.mp3", t0(), 0, 60, 600);
        unix["timestamp"] = json!(t0().timestamp());
        phone.upload_episode_actions(&[offset, unix]).await;

        let body = s.device("tablet").await.episode_actions(0).await.ok_json();
        assert_episode_actions_response(&body);
        for ep in ["https://cdn.example.test/show/offset.mp3", "https://cdn.example.test/show/unix.mp3"] {
            let got = actions_for(&body, ep);
            assert_eq!(got.len(), 1, "{ep}: {body}");
            assert_eq!(action_timestamp(got[0]).unwrap(), Some(t0().naive_utc()), "{ep}: {}", got[0]);
        }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn subscription_upload_is_seen_by_other_devices() {
    conformance("G8 subscriptions", Known::None, async {
        let s = GpodderScene::start().await;
        let feed = s.app.feed("sub.xml", &Feed::with_episodes("Sub", 1));
        let phone = s.device("phone").await;
        let up = phone.upload_subscription_changes(&[&feed], &[]).await.ok_json();
        assert_upload_response(&up);

        let body = s.device("tablet").await.subscription_changes(0).await.ok_json();
        assert_subscription_changes(&body);
        assert!(strings(&body, "add").contains(&feed), "{body}");
    })
    .await;
}

/// The spec: a URL in both `add` and `remove` of one upload is a 400.
#[tokio::test(flavor = "multi_thread")]
async fn same_url_in_add_and_remove_is_rejected() {
    conformance("G6 add+remove", Known::None, async {
        let s = GpodderScene::start().await;
        let feed = s.app.fixtures().url("both.xml");
        let phone = s.device("phone").await;
        phone.upload_subscription_changes(&[&feed], &[&feed]).await.assert_status(400);
    })
    .await;
}

/// `GET /api/2/subscriptions/{user}.json`: every subscription of the user (since 2.11).
#[tokio::test(flavor = "multi_thread")]
async fn all_subscriptions_endpoint_lists_the_library() {
    conformance("G7 all-subscriptions", Known::Go, async {
        let s = GpodderScene::start().await;
        let feed = s.app.feed("all.xml", &Feed::with_episodes("All", 1));
        let phone = s.device("phone").await;
        phone.upload_subscription_changes(&[&feed], &[]).await.assert_ok();
        let body = phone.get(&format!("/api/2/subscriptions/{}.json", s.user.username)).await.ok_json();
        let urls: Vec<&str> = body.as_array().unwrap_or_else(|| panic!("expected an array: {body}")).iter().filter_map(|v| v.as_str()).collect();
        assert!(urls.contains(&feed.as_str()), "{body}");
    })
    .await;
}

/// Simple API `GET /subscriptions/{user}/{device}.{format}` in json, txt and opml.
#[tokio::test(flavor = "multi_thread")]
async fn simple_api_serves_every_format() {
    conformance("G7 simple-formats", Known::Go, async {
        let s = GpodderScene::start().await;
        let feed = s.app.feed("simple.xml", &Feed::with_episodes("Simple", 1));
        let phone = s.device("phone").await;
        phone.upload_subscription_changes(&[&feed], &[]).await.assert_ok();
        let u = &s.user.username;

        let json_body = phone.get(&format!("/subscriptions/{u}/phone.json")).await.ok_json();
        assert!(json_body.as_array().is_some_and(|a| a.iter().any(|v| v == &json!(feed))), "json: {json_body}");
        let txt = phone.get(&format!("/subscriptions/{u}/phone.txt")).await;
        txt.assert_ok();
        assert!(txt.body.lines().any(|l| l.trim() == feed), "txt: {}", txt.body);
        let opml = phone.get(&format!("/subscriptions/{u}/phone.opml")).await;
        opml.assert_ok();
        assert!(opml.body.contains("<opml") && opml.body.contains(&feed), "opml: {}", opml.body);
    })
    .await;
}

// ---- auth (G3) ----------------------------------------------------------------------------

/// A rejected request gets one 401 with one JSON body, and its handler never runs.
#[tokio::test(flavor = "multi_thread")]
async fn wrong_password_is_one_clean_401() {
    conformance("G3 auth", Known::None, async {
        let s = GpodderScene::start().await;
        let u = &s.user.username;
        let intruder = GpodderClient::new(&s.server.base_url, u, "not-the-password", "phone");
        let bad = Some(GpodderClient::basic_header(u, "not-the-password"));

        let r = intruder.send(reqwest::Method::GET, &format!("/api/2/episodes/{u}.json?since=0"), None, bad.clone()).await;
        r.assert_status(401);
        r.json(); // exactly one JSON document

        let actions = json!([play_action(POD, EP, t0(), 0, 600, 1800)]);
        let r = intruder.send(reqwest::Method::POST, &format!("/api/2/episodes/{u}.json"), Some(&actions), bad.clone()).await;
        r.assert_status(401);
        r.json();
        let r = intruder
            .send(reqwest::Method::POST, &format!("/api/2/subscriptions/{u}/phone.json"), Some(&json!({ "add": [POD], "remove": [] })), bad)
            .await;
        r.assert_status(401);
        r.json();

        let owner = s.device("phone").await;
        let body = owner.episode_actions(0).await.ok_json();
        assert!(actions_for(&body, EP).is_empty(), "rejected upload was stored: {body}");
        let subs = owner.subscription_changes(0).await.ok_json();
        assert!(!strings(&subs, "add").contains(&POD.to_string()), "rejected subscription was stored: {subs}");
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn no_credentials_is_401() {
    conformance("G3 anonymous", Known::None, async {
        let s = GpodderScene::start().await;
        let anon = GpodderClient::new(&s.server.base_url, &s.user.username, "", "phone");
        let r = anon.episode_actions(0).await;
        r.assert_status(401);
        r.json();
    })
    .await;
}

/// Another user's password or session can't read or write this user's data.
#[tokio::test(flavor = "multi_thread")]
async fn other_users_credentials_are_refused() {
    conformance("G3 cross-user", Known::None, async {
        let s = GpodderScene::start().await;
        let owner = s.device("phone").await;
        owner.upload_episode_actions(&[play_action(POD, EP, t0(), 0, 600, 1800)]).await;

        let mallory = s.other_user("mallory").await;
        let m = GpodderClient::new(&s.server.base_url, &mallory.username, &mallory.password, "evil");
        m.login().await.assert_ok(); // a valid session, for mallory
        let victim = &s.user.username;
        let basic = || Some(GpodderClient::basic_header(&mallory.username, &mallory.password));

        // Session cookie on the victim's paths.
        let r = m.episode_actions_of(victim).await;
        assert!(matches!(r.status, 401 | 403), "read with mallory's session: {} {}", r.status, r.body);
        // Mallory's Basic credentials on the victim's paths.
        let r = m.send(reqwest::Method::GET, &format!("/api/2/episodes/{victim}.json?since=0"), None, basic()).await;
        assert!(matches!(r.status, 401 | 403), "read with mallory's password: {} {}", r.status, r.body);
        let forged = json!([play_action(POD, EP, Utc::now(), 0, 5, 1800)]);
        let r = m.send(reqwest::Method::POST, &format!("/api/2/episodes/{victim}.json"), Some(&forged), basic()).await;
        assert!(matches!(r.status, 401 | 403), "write with mallory's password: {} {}", r.status, r.body);
        let r = m
            .send(reqwest::Method::POST, &format!("/api/2/subscriptions/{victim}/evil.json"), Some(&json!({ "add": [], "remove": [POD] })), basic())
            .await;
        assert!(matches!(r.status, 401 | 403), "unsubscribe with mallory's password: {} {}", r.status, r.body);

        let body = owner.episode_actions(0).await.ok_json();
        let got = actions_for(&body, EP);
        assert_eq!(got.len(), 1, "victim's actions changed: {body}");
        assert_eq!(play_position(got[0]), Some(600));
    })
    .await;
}

impl GpodderClient {
    async fn episode_actions_of(&self, user: &str) -> GResp {
        self.get(&format!("/api/2/episodes/{user}.json?since=0")).await
    }
}
