//! Logic the pages rely on that doesn't need a browser.

use wasm_bindgen_test::*;

use crate::components::gen_funcs::*;

#[wasm_bindgen_test]
fn dates_from_either_database_format_the_same() {
    assert_eq!(format_date("2026-03-09T14:05:06"), "03-09-2026");
    assert_eq!(format_date("2026-03-09T14:05:06.123456"), "03-09-2026");
}

#[wasm_bindgen_test]
fn an_unreadable_date_falls_back_instead_of_failing() {
    assert_eq!(format_date("not a date"), "01-01-1970");
}

#[wasm_bindgen_test]
fn long_descriptions_are_cut_and_flagged() {
    assert_eq!(truncate_description("short".into(), 10), ("short".to_string(), false));
    assert_eq!(truncate_description("exactly ten".into(), 11), ("exactly ten".to_string(), false));
    assert_eq!(truncate_description("a longer description".into(), 8), ("a longer...".to_string(), true));
}

#[wasm_bindgen_test]
fn truncating_counts_characters_not_bytes() {
    let text = "é".repeat(10);
    assert_eq!(truncate_description(text.clone(), 15), (text, false), "nothing was cut, so no ellipsis");
    assert_eq!(truncate_description("日本語のテキスト".into(), 3), ("日本語...".to_string(), true));
}

#[wasm_bindgen_test]
fn links_in_descriptions_open_elsewhere_and_scripts_are_removed() {
    let clean = sanitize_html_with_blank_target(
        r#"<a href="https://example.com" onclick="steal()">site</a><script>steal()</script>"#,
    );
    assert!(clean.contains(r#"target="_blank""#), "{clean}");
    assert!(clean.contains(r#"href="https://example.com""#), "{clean}");
    assert!(!clean.contains("script"), "{clean}");
    assert!(!clean.contains("onclick"), "{clean}");
}

#[wasm_bindgen_test]
fn javascript_links_are_removed() {
    let clean = sanitize_html_with_blank_target(r#"<a href="javascript:steal()">site</a>"#);
    assert!(!clean.contains("javascript:"), "{clean}");
}

#[wasm_bindgen_test]
fn sign_up_details_are_checked() {
    assert!(validate_user_input("sam", "secret1", "sam@example.com").is_empty());
    assert_eq!(
        validate_user_input("  ", "short", "nope"),
        vec![ValidationError::UsernameTooShort, ValidationError::PasswordTooShort, ValidationError::InvalidEmail],
    );
    assert_eq!(validate_username(""), vec![ValidationError::UsernameTooShort]);
    assert_eq!(validate_password("12345"), vec![ValidationError::PasswordTooShort]);
    assert!(validate_password("123456").is_empty());
    for bad in ["", "plain", "a@b", "a b@c.d", "@c.d"] {
        assert_eq!(validate_email(bad), vec![ValidationError::InvalidEmail], "{bad}");
    }
    assert!(validate_email("a.b+c@sub.example.org").is_empty());
}

#[wasm_bindgen_test]
fn passwords_are_hashed_with_argon2_and_a_fresh_salt() {
    let first = encode_password("secret").unwrap();
    let second = encode_password("secret").unwrap();
    assert!(first.starts_with("$argon2id$"), "{first}");
    assert_ne!(first, second);
    assert!(!first.contains("secret"));
}

#[wasm_bindgen_test]
fn times_are_shown_as_clocks() {
    assert_eq!(format_time(0), "00:00:00");
    assert_eq!(format_time(3725), "01:02:05");
    assert_eq!(format_time_rm_hour(125), "02:05");
    assert_eq!(format_time_rm_hour(3725), "01:02:05");
    assert_eq!(format_time_mins(125), "02:05");
}

#[wasm_bindgen_test]
fn typed_times_become_seconds() {
    assert_eq!(convert_time_to_seconds("01:02:05").unwrap(), 3725);
    assert_eq!(convert_time_to_seconds("02:05").unwrap(), 125);
    assert_eq!(convert_time_to_seconds("45").unwrap(), 45);
    for bad in ["", "abc", "1:2:3:4", "1:xx"] {
        assert!(convert_time_to_seconds(bad).is_err(), "{bad}");
    }
}

#[wasm_bindgen_test]
fn timestamps_become_the_servers_date_format() {
    assert_eq!(unix_timestamp_to_datetime_string(0), "1970-01-01 00:00:00.000");
    assert_eq!(unix_timestamp_to_datetime_string(1_700_000_000), "2023-11-14 22:13:20.000");
}

#[wasm_bindgen_test]
fn server_errors_are_reduced_to_their_message() {
    assert_eq!(format_error_message(r#"{"detail":"Not allowed"}"#), "Not allowed");
    assert_eq!(format_error_message(r#"{"message":"Try later"}"#), "Try later");
    assert_eq!(format_error_message(r#"{"error":"Broken"}"#), "Broken");
    assert_eq!(format_error_message("Error sending test notification: bad topic"), "bad topic");
    assert_eq!(format_error_message("plain failure"), "plain failure");
    assert_eq!(format_error_message(r#"{"other":1}"#), r#"{"other":1}"#);
}

#[wasm_bindgen_test]
fn gravatars_use_the_address_hash_and_a_size() {
    let url = generate_gravatar_url(&Some("someone@example.com".to_string()), 80);
    assert!(url.contains(&calculate_gravatar_hash(&"someone@example.com".to_string())), "{url}");
    assert!(url.contains("80"), "{url}");
    assert!(!url.contains("someone@example.com"), "the address itself isn't sent: {url}");
}

mod media_session {
    use crate::components::media_session::{effect, Action, Effect, SKIP_SECONDS};
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn track_keys_skip_within_the_episode() {
        assert_eq!(effect(Action::NextTrack, None, None), Some(Effect::SeekBy(SKIP_SECONDS)));
        assert_eq!(effect(Action::PreviousTrack, None, None), Some(Effect::SeekBy(-SKIP_SECONDS)));
    }

    #[wasm_bindgen_test]
    fn seek_keys_use_the_offset_the_os_asks_for() {
        assert_eq!(effect(Action::SeekForward, Some(30.0), None), Some(Effect::SeekBy(30.0)));
        assert_eq!(effect(Action::SeekBackward, Some(10.0), None), Some(Effect::SeekBy(-10.0)));
        assert_eq!(effect(Action::SeekForward, None, None), Some(Effect::SeekBy(SKIP_SECONDS)));
        for bad in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            assert_eq!(effect(Action::SeekBackward, Some(bad), None), Some(Effect::SeekBy(-SKIP_SECONDS)));
        }
    }

    #[wasm_bindgen_test]
    fn scrubbing_needs_a_usable_time() {
        assert_eq!(effect(Action::SeekTo, None, Some(42.5)), Some(Effect::SeekTo(42.5)));
        assert_eq!(effect(Action::SeekTo, None, Some(0.0)), Some(Effect::SeekTo(0.0)));
        assert_eq!(effect(Action::SeekTo, None, None), None);
        assert_eq!(effect(Action::SeekTo, None, Some(-1.0)), None);
        assert_eq!(effect(Action::SeekTo, None, Some(f64::NAN)), None);
    }

    #[wasm_bindgen_test]
    fn play_and_pause_are_explicit_not_toggles() {
        assert_eq!(effect(Action::Play, None, None), Some(Effect::Play));
        assert_eq!(effect(Action::Pause, None, None), Some(Effect::Pause));
    }

    #[wasm_bindgen_test]
    fn every_action_has_its_spec_name() {
        let names: Vec<_> = Action::ALL.iter().map(|a| a.name()).collect();
        assert_eq!(
            names,
            ["play", "pause", "seekbackward", "seekforward", "previoustrack", "nexttrack", "seekto"]
        );
    }
}

// --- listening progress ------------------------------------------------------------------

mod progress {
    use wasm_bindgen_test::*;

    use crate::components::progress::*;

    const T0: f64 = 1_700_000_000_000.0;

    fn playing(position: f64) -> Sample {
        Sample { episode_id: 7, is_youtube: false, position, duration: 1800.0, playing: true, rate: 1.0, downloaded: false }
    }

    fn paused(position: f64) -> Sample {
        Sample { playing: false, ..playing(position) }
    }

    /// Play from `from` for `seconds`, looking once a second; returns every report made.
    fn play(tracker: &mut Tracker, start: f64, from: f64, seconds: u32) -> Vec<Update> {
        (1..=seconds).flat_map(|s| tracker.observe(start + s as f64 * 1000.0, Some(playing(from + s as f64)))).collect()
    }

    fn update(episode_id: i32, position: f64, at: f64, listened: f64) -> Update {
        Update { episode_id, is_youtube: false, position, at, listened, duration: None, finished: false, speed_saved: 0.0, silence_skipped: 0.0, ads_skipped: 0.0, downloaded: false }
    }

    #[wasm_bindgen_test]
    fn time_saved_by_speed_and_skips_is_reported() {
        let mut t = Tracker::default();
        let fast = |position: f64| Sample { rate: 1.5, ..playing(position) };
        t.observe(T0, Some(fast(0.0)));
        // 30s at 1.5x covers 45s of the episode: 15s saved.
        let mut reports = Vec::new();
        for s in 1..=30 {
            if s == 10 {
                t.skipped(Skipped::Silence, 2.5);
                t.skipped(Skipped::Ad, 60.0);
            }
            reports.extend(t.observe(T0 + s as f64 * 1000.0, Some(fast(s as f64 * 1.5))));
        }
        let total = |f: fn(&Update) -> f64| reports.iter().map(f).sum::<f64>();
        assert_eq!(total(|u| u.listened), 30.0);
        assert!((total(|u| u.speed_saved) - 15.0).abs() < 1e-9);
        assert_eq!((total(|u| u.silence_skipped), total(|u| u.ads_skipped)), (2.5, 60.0));

        // Settling a sent entry that moved on keeps only what wasn't delivered.
        let mut p = Pending::default();
        let sent = Update { speed_saved: 5.0, ads_skipped: 30.0, ..update(1, 100.0, T0, 10.0) };
        p.merge(sent.clone());
        p.merge(Update { speed_saved: 2.0, ..update(1, 110.0, T0 + 10_000.0, 4.0) });
        p.settle(&[sent]);
        let left = p.get(1, false).unwrap();
        assert_eq!((left.listened, left.speed_saved, left.ads_skipped), (4.0, 2.0, 0.0));
    }

    #[wasm_bindgen_test]
    fn a_playing_episode_is_reported_every_half_minute() {
        let mut t = Tracker::default();
        // Starting isn't progress: the position came from wherever it was last saved.
        assert!(t.observe(T0, Some(playing(100.0))).is_empty());

        let reports = play(&mut t, T0, 100.0, 65);
        assert_eq!(reports.iter().map(|u| u.position).collect::<Vec<_>>(), vec![130.0, 160.0]);
        assert_eq!(reports.iter().map(|u| u.listened).collect::<Vec<_>>(), vec![30.0, 30.0], "time played since the last report");
        assert!(reports.iter().all(|u| u.duration == Some(1800.0) && !u.finished));
    }

    #[wasm_bindgen_test]
    fn nothing_is_reported_while_paused() {
        let mut t = Tracker::default();
        t.observe(T0, Some(playing(100.0)));
        let on_pause = t.observe(T0 + 5000.0, Some(paused(105.0)));
        assert_eq!(on_pause.len(), 1, "a pause is reported right away");
        assert_eq!((on_pause[0].position, on_pause[0].listened), (105.0, 5.0));
        for minute in 1..=120 {
            assert!(t.observe(T0 + 5000.0 + minute as f64 * 60_000.0, Some(paused(105.0))).is_empty());
        }
        // Playing again counts from the resume, not from the pause.
        let now = T0 + 3.0 * 3_600_000.0;
        t.observe(now, Some(playing(105.0)));
        let after = t.observe(now + 1000.0, Some(playing(106.0)));
        assert!(after.iter().all(|u| u.listened <= 1.0), "{after:?}");
    }

    #[wasm_bindgen_test]
    fn a_seek_is_reported_in_either_direction() {
        let mut t = Tracker::default();
        t.observe(T0, Some(playing(600.0)));
        let back = t.observe(T0 + 1000.0, Some(playing(30.0)));
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].position, 30.0);
        let ahead = t.observe(T0 + 2000.0, Some(playing(900.0)));
        assert_eq!(ahead[0].position, 900.0);
        // While paused too.
        t.observe(T0 + 3000.0, Some(paused(901.0)));
        let scrubbed = t.observe(T0 + 4000.0, Some(paused(120.0)));
        assert_eq!(scrubbed.len(), 1);
        assert_eq!((scrubbed[0].position, scrubbed[0].listened), (120.0, 0.0));
    }

    #[wasm_bindgen_test]
    fn faster_playback_is_not_a_seek_and_counts_real_time() {
        let mut t = Tracker::default();
        let fast = |position: f64| Sample { rate: 2.0, ..playing(position) };
        t.observe(T0, Some(fast(0.0)));
        let mut reports = Vec::new();
        for s in 1..=30 {
            reports.extend(t.observe(T0 + s as f64 * 1000.0, Some(fast(2.0 * s as f64))));
        }
        assert_eq!(reports.len(), 1, "{reports:?}");
        assert_eq!((reports[0].position, reports[0].listened), (60.0, 30.0));
    }

    #[wasm_bindgen_test]
    fn switching_episodes_reports_where_the_old_one_was_left() {
        let mut t = Tracker::default();
        t.observe(T0, Some(playing(100.0)));
        play(&mut t, T0, 100.0, 10);
        let other = Sample { episode_id: 8, ..playing(0.0) };
        let reports = t.observe(T0 + 11_000.0, Some(other));
        assert_eq!(reports.iter().map(|u| (u.episode_id, u.position)).collect::<Vec<_>>(), vec![(7, 110.0)]);
        assert_eq!(reports[0].listened, 10.0);
        // Closing the player reports too, with only the new episode's time.
        for s in 1..=4 {
            t.observe(T0 + 11_000.0 + s as f64 * 1000.0, Some(Sample { position: s as f64, ..other }));
        }
        let closed = t.observe(T0 + 16_000.0, None);
        assert_eq!((closed[0].episode_id, closed[0].position, closed[0].listened), (8, 4.0, 4.0));
    }

    #[wasm_bindgen_test]
    fn time_the_page_spent_asleep_is_not_listening() {
        let mut t = Tracker::default();
        t.observe(T0, Some(playing(100.0)));
        // The laptop lid closes for an hour mid-play.
        let woke = t.observe(T0 + 3_600_000.0, Some(playing(101.0)));
        assert!(woke.iter().all(|u| u.listened == 0.0), "{woke:?}");
    }

    #[wasm_bindgen_test]
    fn finishing_reports_the_end_and_leaving_reports_what_is_unsent() {
        let mut t = Tracker::default();
        assert!(t.finish(T0).is_none() && t.leave(T0).is_none(), "nothing loaded");
        t.observe(T0, Some(playing(1790.0)));
        play(&mut t, T0, 1790.0, 8);
        let left = t.leave(T0 + 8500.0).expect("eight seconds unsent");
        assert_eq!((left.position, left.listened, left.finished), (1798.0, 8.0, false));
        assert!(t.leave(T0 + 8600.0).is_none(), "already reported");
        let done = t.finish(T0 + 10_000.0).unwrap();
        assert_eq!((done.position, done.finished), (1800.0, true));
        assert!(t.observe(T0 + 11_000.0, Some(paused(1800.0))).is_empty(), "the stop at the end isn't another report");
    }

    #[wasm_bindgen_test]
    fn pending_keeps_one_entry_per_episode() {
        let mut p = Pending::default();
        p.merge(update(1, 100.0, T0, 30.0));
        p.merge(update(2, 5.0, T0, 5.0));
        p.merge(update(1, 40.0, T0 + 1000.0, 12.0)); // went back
        assert_eq!(p.0.len(), 2);
        let one = p.get(1, false).unwrap();
        assert_eq!((one.position, one.at, one.listened), (40.0, T0 + 1000.0, 42.0));
        // Something older arriving late adds its time but not its position.
        p.merge(update(1, 900.0, T0 - 5000.0, 3.0));
        assert_eq!((p.get(1, false).unwrap().position, p.get(1, false).unwrap().listened), (40.0, 45.0));
        assert!(p.get(1, true).is_none(), "a video with the same id is something else");
    }

    #[wasm_bindgen_test]
    fn only_what_the_server_received_is_cleared() {
        let mut p = Pending::default();
        p.merge(update(1, 100.0, T0, 30.0));
        p.merge(update(2, 5.0, T0, 5.0));
        let sent = p.0.clone();
        // Episode 1 moved on while the request was in flight.
        p.merge(update(1, 130.0, T0 + 30_000.0, 30.0));
        p.settle(&sent);
        assert_eq!(p.0.len(), 1);
        let left = &p.0[0];
        assert_eq!((left.episode_id, left.position, left.listened), (1, 130.0, 30.0), "the delivered 30s aren't sent again");
    }

    #[wasm_bindgen_test]
    fn pending_survives_storage() {
        let mut p = Pending::default();
        p.merge(Update { duration: Some(1800.0), finished: true, ..update(1, 1800.0, T0, 12.5) });
        let text = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Pending>(&text).unwrap(), p);
        assert_eq!(serde_json::from_str::<Pending>("not json").unwrap_or_default(), Pending::default());
    }

    #[wasm_bindgen_test]
    fn the_request_carries_both_clock_readings_as_whole_milliseconds() {
        let body: serde_json::Value = serde_json::from_str(&request_body(&[update(1, 12.5, T0 - 4000.0, 3.0)], T0 + 0.7)).unwrap();
        assert_eq!(body["sent_at"], 1_700_000_000_000i64);
        assert_eq!(body["client"], crate::components::progress::CLIENT);
        assert_eq!(body["updates"][0]["at"], 1_699_999_996_000i64);
        assert_eq!(body["updates"][0]["position"], 12.5);
        assert_eq!(body["updates"][0]["listened"], 3.0);
        assert_eq!(body["updates"][0]["finished"], false);
    }

    #[wasm_bindgen_test]
    fn an_episode_resumes_where_it_was_left_unless_that_is_the_end() {
        assert_eq!(resume_from(640.0, 1800.0), 640.0);
        assert_eq!(resume_from(1800.0, 1800.0), 0.0, "finished: start over");
        assert_eq!(resume_from(1799.0, 1800.0), 0.0);
        assert_eq!(resume_from(1700.0, 1800.0), 1700.0, "near the end is still a place to resume");
        assert_eq!(resume_from(640.0, 0.0), 640.0, "length unknown");
        assert_eq!(resume_from(-3.0, 1800.0), 0.0);
        assert_eq!(resume_from(f64::NAN, 1800.0), 0.0);
    }
}
