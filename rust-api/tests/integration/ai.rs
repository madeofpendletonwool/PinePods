//! AI features against a stand-in for the `pinepods-ai` sidecar: transcripts, ad detection and
//! per-user review, silence detection, automatic runs on new episodes, and model management.

use crate::harness::files::{assert_gone, mp3_bytes, wav_with_gap};
use crate::harness::mock_ai::{AUDIO_CRASH, AUDIO_ERROR, MODEL_WHOLE_EPISODE, SPONSOR_LINE};
use crate::harness::*;

struct Show {
    app: TestApp,
    user: TestUser,
    pod: i32,
    episode: i32,
}

async fn show_with_audio(audio: Vec<u8>) -> Show {
    let app = TestApp::new().await;
    app.connect_ai().await;
    let user = app.user("listener").await;
    let url = app.fixtures().serve("audio/ep1.mp3", Fixture::new("audio/mpeg", audio));
    let feed = Feed::new(&format!("AI Show {}", app.tag)).episode(Episode::new(1).enclosure(&url));
    let pod = app.subscribe(&user, &app.feed("show.xml", &feed), 1).await;
    let episode = app.episode_id(&user, pod, "Episode 1").await;
    Show { app, user, pod, episode }
}

async fn show() -> Show {
    show_with_audio(mp3_bytes()).await
}

/// Sets the LLM the sidecar is told to use; the tag keeps this test's requests identifiable.
async fn use_llm(app: &TestApp, model: &str) {
    let admin = app.admin().await.clone();
    app.post("/api/data/ai_settings")
        .key(&admin)
        .json(&json!({ "transcription_model": format!("whisper-{}", app.tag), "llm_backend": "local", "llm_model": model }))
        .send()
        .await
        .assert_ok();
}

async fn transcript(app: &TestApp, user: &TestUser, episode: i32) -> Value {
    app.get("/api/data/episode_transcript")
        .key(user)
        .query("episode_id", episode)
        .query("user_id", user.id)
        .send()
        .await
        .ok_json()["transcript"]
        .clone()
}

async fn wait_for_transcript(app: &TestApp, user: &TestUser, episode: i32, status: &str) -> Value {
    eventually(&format!("transcript {status}"), || async {
        let t = transcript(app, user, episode).await;
        (t["status"] == status).then_some(t)
    })
    .await
}

async fn start_transcript(app: &TestApp, user: &TestUser, episode: i32, force: bool) {
    app.post("/api/data/transcribe_episode")
        .key(user)
        .json(&json!({ "episode_id": episode, "user_id": user.id, "force": force }))
        .send()
        .await
        .assert_ok();
}

async fn start_detection(app: &TestApp, user: &TestUser, episode: i32) {
    app.post("/api/data/detect_ads")
        .key(user)
        .json(&json!({ "episode_id": episode, "user_id": user.id }))
        .send()
        .await
        .assert_ok();
}

async fn segments(app: &TestApp, user: &TestUser, episode: i32, kind: &str) -> Vec<Value> {
    let body = app.get("/api/data/episode_skip_segments")
        .key(user)
        .query("episode_id", episode)
        .query("user_id", user.id)
        .send()
        .await
        .ok_json();
    body["segments"].as_array().unwrap().iter().filter(|s| s["kind"] == kind).cloned().collect()
}

async fn wait_for_ads(app: &TestApp, user: &TestUser, episode: i32) -> Vec<Value> {
    eventually("ads detected", || async {
        let ads = segments(app, user, episode, "ad").await;
        (!ads.is_empty()).then_some(ads)
    })
    .await
}

async fn set_flag(app: &TestApp, user: &TestUser, pod: i32, path: &str) {
    app.post(path)
        .key(user)
        .json(&json!({ "podcast_id": pod, "user_id": user.id, "enabled": true }))
        .send()
        .await
        .assert_ok();
}

async fn clear_auto_activate(app: &TestApp, user: &TestUser, pod: i32) {
    app.post("/api/data/adjust_ad_skip_auto_activate")
        .key(user)
        .json(&json!({ "podcast_id": pod, "user_id": user.id, "enabled": false }))
        .send()
        .await
        .assert_ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn ai_features_are_off_until_the_sidecar_is_found() {
    let app = TestApp::new().await;
    let user = app.user("listener").await;
    let pod = app.subscribe(&user, &app.feed("show.xml", &Feed::with_episodes("Show", 1)), 1).await;
    let episode = app.episode_id(&user, pod, "Episode 1").await;

    let status = app.get("/api/data/ai_status").key(&user).send().await.ok_json();
    assert_eq!(status["available"], false, "{status}");
    assert_eq!(status["transcription_ready"], false, "{status}");

    for path in ["/api/data/transcribe_episode", "/api/data/detect_ads"] {
        app.post(path)
            .key(&user)
            .json(&json!({ "episode_id": episode, "user_id": user.id }))
            .send()
            .await
            .assert_status(503);
    }
    let admin = app.admin().await.clone();
    app.get("/api/data/ai_models").key(&admin).send().await.assert_status(503);
}

#[tokio::test(flavor = "multi_thread")]
async fn status_reports_what_each_capability_needs() {
    let app = TestApp::new().await;
    app.connect_ai().await;
    let user = app.user("listener").await;

    let status = app.get("/api/data/ai_status").key(&user).send().await.ok_json();
    assert_eq!(status["available"], true, "{status}");
    assert_eq!(status["transcription_ready"], true, "{status}");

    use_llm(&app, "some-model").await;
    let status = app.get("/api/data/ai_status").key(&user).send().await.ok_json();
    assert_eq!(status["ad_removal_ready"], true, "{status}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_transcript_is_made_from_the_episode_audio_and_stored() {
    let s = show().await;
    use_llm(&s.app, "unused").await;
    start_transcript(&s.app, &s.user, s.episode, false).await;

    let t = wait_for_transcript(&s.app, &s.user, s.episode, "complete").await;
    assert!(t["full_text"].as_str().unwrap().contains(SPONSOR_LINE), "{t}");
    assert_eq!(t["language"], "en");
    let segments: Value = serde_json::from_str(t["segments"].as_str().unwrap()).unwrap();
    assert_eq!(segments.as_array().unwrap().len(), 3, "{segments}");

    let suffix = format!("/{}.mp3", s.episode);
    let request = s.app.env.ai.requests("/transcribe")
        .into_iter()
        .find(|r| r.body["file_path"].as_str().is_some_and(|p| p.ends_with(&suffix)))
        .expect("the sidecar was asked");
    assert_eq!(request.audio.as_deref(), Some(mp3_bytes().as_slice()), "it could read the episode's audio");
    assert_eq!(request.body["model"], format!("whisper-{}", s.app.tag), "the chosen model is used");
    assert_gone(std::path::Path::new(request.body["file_path"].as_str().unwrap()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_finished_transcript_is_not_made_again_unless_forced() {
    let s = show().await;
    use_llm(&s.app, "unused").await;
    let suffix = format!("/{}.mp3", s.episode);
    let asked = |app: &TestApp| {
        app.env.ai.requests("/transcribe")
            .iter()
            .filter(|r| r.body["file_path"].as_str().is_some_and(|p| p.ends_with(&suffix)))
            .count()
    };

    start_transcript(&s.app, &s.user, s.episode, false).await;
    wait_for_transcript(&s.app, &s.user, s.episode, "complete").await;
    assert_eq!(asked(&s.app), 1);

    start_transcript(&s.app, &s.user, s.episode, false).await;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(asked(&s.app), 1);

    start_transcript(&s.app, &s.user, s.episode, true).await;
    eventually("transcribed again", || async { (asked(&s.app) == 2).then_some(()) }).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sidecar_failure_marks_the_transcript_failed() {
    let s = show_with_audio(AUDIO_ERROR.to_vec()).await;
    start_transcript(&s.app, &s.user, s.episode, false).await;
    wait_for_transcript(&s.app, &s.user, s.episode, "failed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_sidecar_crash_mid_job_marks_the_transcript_failed() {
    let s = show_with_audio(AUDIO_CRASH.to_vec()).await;
    start_transcript(&s.app, &s.user, s.episode, false).await;
    wait_for_transcript(&s.app, &s.user, s.episode, "failed").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_transcript_can_be_retried_after_a_failure() {
    let s = show_with_audio(AUDIO_ERROR.to_vec()).await;
    start_transcript(&s.app, &s.user, s.episode, false).await;
    wait_for_transcript(&s.app, &s.user, s.episode, "failed").await;

    s.app.fixtures().serve("audio/ep1.mp3", Fixture::new("audio/mpeg", mp3_bytes()));
    start_transcript(&s.app, &s.user, s.episode, false).await;
    wait_for_transcript(&s.app, &s.user, s.episode, "complete").await;
}

#[tokio::test(flavor = "multi_thread")]
async fn other_users_cannot_process_your_episodes() {
    let s = show().await;
    let other = s.app.user("other").await;
    for (who, target) in [(&other, s.user.id), (&other, other.id)] {
        let resp = s.app.post("/api/data/transcribe_episode")
            .key(who)
            .json(&json!({ "episode_id": s.episode, "user_id": target }))
            .send()
            .await;
        assert!(!resp.is_success(), "got {}", resp.status);
    }
    assert_eq!(transcript(&s.app, &s.user, s.episode).await, Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn detecting_ads_transcribes_first_and_stores_the_ad_times() {
    let s = show().await;
    use_llm(&s.app, &format!("llm-{}", s.app.tag)).await;
    start_detection(&s.app, &s.user, s.episode).await;

    let ads = wait_for_ads(&s.app, &s.user, s.episode).await;
    assert_eq!(ads.len(), 1, "{ads:?}");
    assert_eq!(ads[0]["start_time"], 10.0);
    assert_eq!(ads[0]["end_time"], 20.0);
    assert_eq!(wait_for_transcript(&s.app, &s.user, s.episode, "complete").await["language"], "en");

    let model = format!("llm-{}", s.app.tag);
    let asked = s.app.env.ai.requests("/detect_ads");
    let request = asked.iter().find(|r| r.body["llm"]["model"] == model.as_str()).expect("asked the LLM");
    assert_eq!(request.body["llm"]["backend"], "local");
    assert_eq!(request.body["segments"].as_array().unwrap().len(), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_span_covering_most_of_the_episode_is_ignored() {
    let s = show().await;
    use_llm(&s.app, MODEL_WHOLE_EPISODE).await;
    start_detection(&s.app, &s.user, s.episode).await;

    let scanned = format!(r#"SELECT COUNT(*) FROM "Episodes" WHERE EpisodeID = {} AND AdsDetected = TRUE"#, s.episode);
    eventually("scan finished", || async { (int(&s.app, &scanned).await == Some(1)).then_some(()) }).await;
    assert!(segments(&s.app, &s.user, s.episode, "ad").await.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn with_auto_activate_off_ads_wait_for_confirmation_and_each_user_decides() {
    let s = show().await;
    use_llm(&s.app, "llm").await;
    let partner = s.app.user("partner").await;
    let feed_url = s.app.fixtures().url("show.xml");
    let partner_pod = s.app.subscribe(&partner, &feed_url, 1).await;
    clear_auto_activate(&s.app, &s.user, s.pod).await;
    clear_auto_activate(&s.app, &partner, partner_pod).await;

    start_detection(&s.app, &s.user, s.episode).await;
    let ads = wait_for_ads(&s.app, &s.user, s.episode).await;
    assert_eq!(ads[0]["status"], "pending", "{ads:?}");
    let segment = ads[0]["segment_id"].as_i64().unwrap();

    s.app.post("/api/data/adjust_ad_segment_review")
        .key(&s.user)
        .json(&json!({ "segment_id": segment, "user_id": s.user.id, "status": "confirmed" }))
        .send()
        .await
        .assert_ok();

    assert_eq!(segments(&s.app, &s.user, s.episode, "ad").await[0]["status"], "confirmed");
    assert_eq!(segments(&s.app, &partner, s.episode, "ad").await[0]["status"], "pending");
}

#[tokio::test(flavor = "multi_thread")]
async fn ads_are_skipped_by_default_until_a_user_rejects_them() {
    let s = show().await;
    use_llm(&s.app, "llm").await;

    start_detection(&s.app, &s.user, s.episode).await;
    let ads = wait_for_ads(&s.app, &s.user, s.episode).await;
    assert_eq!(ads[0]["status"], "active", "{ads:?}");

    s.app.post("/api/data/adjust_ad_segment_review")
        .key(&s.user)
        .json(&json!({ "segment_id": ads[0]["segment_id"], "user_id": s.user.id, "status": "rejected" }))
        .send()
        .await
        .assert_ok();
    assert_eq!(segments(&s.app, &s.user, s.episode, "ad").await[0]["status"], "rejected");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_review_needs_a_real_decision() {
    let s = show().await;
    use_llm(&s.app, "llm").await;
    start_detection(&s.app, &s.user, s.episode).await;
    let ads = wait_for_ads(&s.app, &s.user, s.episode).await;

    let resp = s.app.post("/api/data/adjust_ad_segment_review")
        .key(&s.user)
        .json(&json!({ "segment_id": ads[0]["segment_id"], "user_id": s.user.id, "status": "whatever" }))
        .send()
        .await;
    assert!(!resp.is_success(), "got {}", resp.status);
    assert_eq!(segments(&s.app, &s.user, s.episode, "ad").await[0]["status"], "active");
}

#[tokio::test(flavor = "multi_thread")]
async fn new_episodes_are_transcribed_and_scanned_when_the_podcast_opts_in() {
    let s = show().await;
    use_llm(&s.app, "llm").await;
    set_flag(&s.app, &s.user, s.pod, "/api/data/adjust_auto_transcribe").await;
    set_flag(&s.app, &s.user, s.pod, "/api/data/adjust_auto_ad_detect").await;

    let first = s.app.fixtures().url("audio/ep1.mp3");
    let second = s.app.fixtures().serve("audio/ep2.mp3", Fixture::new("audio/mpeg", mp3_bytes()));
    let feed = Feed::new(&format!("AI Show {}", s.app.tag))
        .episode(Episode::new(1).enclosure(&first))
        .episode(Episode::new(2).enclosure(&second));
    s.app.feed("show.xml", &feed);
    s.app.refresh().await;
    let fresh = s.app.episode_id(&s.user, s.pod, "Episode 2").await;

    wait_for_transcript(&s.app, &s.user, fresh, "complete").await;
    wait_for_ads(&s.app, &s.user, fresh).await;
    assert_eq!(transcript(&s.app, &s.user, s.episode).await, Value::Null, "older episodes are left alone");
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_runs_for_podcasts_that_did_not_opt_in() {
    let s = show().await;
    use_llm(&s.app, "llm").await;
    let first = s.app.fixtures().url("audio/ep1.mp3");
    let second = s.app.fixtures().serve("audio/ep2.mp3", Fixture::new("audio/mpeg", mp3_bytes()));
    let feed = Feed::new(&format!("AI Show {}", s.app.tag))
        .episode(Episode::new(1).enclosure(&first))
        .episode(Episode::new(2).enclosure(&second));
    s.app.feed("show.xml", &feed);
    s.app.refresh().await;
    let fresh = s.app.episode_id(&s.user, s.pod, "Episode 2").await;

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    assert_eq!(transcript(&s.app, &s.user, fresh).await, Value::Null);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_llm_api_key_reaches_the_sidecar_but_never_the_browser() {
    let s = show().await;
    let admin = s.app.admin().await.clone();
    let model = format!("remote-{}", s.app.tag);
    s.app.post("/api/data/ai_settings")
        .key(&admin)
        .json(&json!({
            "transcription_model": "base",
            "llm_backend": "remote",
            "llm_model": model,
            "llm_url": "https://llm.example.invalid/v1",
            "llm_api_key": "very-secret-llm-key",
        }))
        .send()
        .await
        .assert_ok();

    let shown = s.app.get("/api/data/ai_settings").key(&admin).send().await;
    shown.assert_ok();
    assert!(!shown.text().contains("very-secret-llm-key"), "{}", shown.text());
    assert_eq!(shown.json()["settings"]["has_api_key"], true);

    start_detection(&s.app, &s.user, s.episode).await;
    wait_for_ads(&s.app, &s.user, s.episode).await;
    let asked = s.app.env.ai.requests("/detect_ads");
    let request = asked.iter().find(|r| r.body["llm"]["model"] == model.as_str()).expect("asked the LLM");
    assert_eq!(request.body["llm"]["api_key"], "very-secret-llm-key");
    assert_eq!(request.body["llm"]["url"], "https://llm.example.invalid/v1");
}

#[tokio::test(flavor = "multi_thread")]
async fn leaving_the_api_key_blank_keeps_it_and_clearing_removes_it() {
    let app = TestApp::new().await;
    let admin = app.admin().await.clone();
    let save = |extra: Value| {
        let mut body = json!({ "transcription_model": "base", "llm_backend": "remote", "llm_url": "https://llm.example.invalid" });
        body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        app.post("/api/data/ai_settings").key(&admin).json(&body)
    };
    let has_key = || async {
        app.get("/api/data/ai_settings").key(&admin).send().await.ok_json()["settings"]["has_api_key"].clone()
    };

    save(json!({ "llm_api_key": "first-key" })).send().await.assert_ok();
    assert_eq!(has_key().await, true);
    save(json!({})).send().await.assert_ok();
    assert_eq!(has_key().await, true);
    save(json!({ "clear_api_key": true })).send().await.assert_ok();
    assert_eq!(has_key().await, false);
}

#[tokio::test(flavor = "multi_thread")]
async fn admins_can_list_and_pull_models() {
    let app = TestApp::new().await;
    app.connect_ai().await;
    let admin = app.admin().await.clone();

    let body = app.get("/api/data/ai_models").key(&admin).send().await.ok_json();
    assert_eq!(body["models"]["whisper"], json!(["base"]));
    assert_eq!(body["models"]["disk"]["free"], 60);

    let model = format!("pulled-{}", app.tag);
    let started = app.post("/api/data/ai_pull_model")
        .key(&admin)
        .json(&json!({ "kind": "whisper", "model": model }))
        .send()
        .await
        .ok_json();
    assert!(started["task_id"].is_string(), "{started}");
    let sidecar = app.env.ai.clone();
    eventually("the sidecar was asked to pull", || {
        let sidecar = sidecar.clone();
        let model = model.clone();
        async move { sidecar.requests("/models/pull").iter().any(|r| r.body["model"] == model.as_str()).then_some(()) }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn silence_in_a_download_is_found_and_offered_as_skips() {
    let s = show_with_audio(wav_with_gap(3, 4)).await;
    s.app.post("/api/data/adjust_silence_trim")
        .key(&s.user)
        .json(&json!({ "podcast_id": s.pod, "user_id": s.user.id, "enabled": true, "threshold": 2 }))
        .send()
        .await
        .assert_ok();
    s.app.post("/api/data/download_podcast")
        .key(&s.user)
        .json(&json!({ "episode_id": s.episode, "user_id": s.user.id }))
        .send()
        .await
        .assert_ok();

    let gaps = eventually("silence detected", || async {
        let gaps = segments(&s.app, &s.user, s.episode, "silence").await;
        (!gaps.is_empty()).then_some(gaps)
    })
    .await;
    assert_eq!(gaps.len(), 1, "one gap in this audio: {gaps:?}");
    let start = gaps[0]["start_time"].as_f64().unwrap();
    let end = gaps[0]["end_time"].as_f64().unwrap();
    assert!((2.5..=3.5).contains(&start), "gap starts at 3s: {gaps:?}");
    assert!((6.5..=7.5).contains(&end), "gap ends at 7s: {gaps:?}");
}
