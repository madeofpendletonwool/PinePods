//! The AntennaPod parser rules themselves, checked against hand-written bodies so a wrong
//! rule can't make a broken server look conformant (or the other way round).

use crate::harness::antennapod::*;
use serde_json::json;

#[test]
fn play_needs_started_position_and_total() {
    let ok = json!({ "podcast": "p", "episode": "e", "action": "play", "started": 0, "position": 600, "total": 1800 });
    assert_eq!(play_position(&ok), Some(600));
    for (k, v) in [("started", json!(-1)), ("position", json!(0)), ("total", json!(0))] {
        let mut bad = ok.clone();
        bad[k] = v;
        assert_eq!(play_position(&bad), None, "{bad}");
    }
    let mut missing = ok.clone();
    missing.as_object_mut().unwrap().remove("total");
    assert_eq!(play_position(&missing), None);
    // optInt coerces numeric strings and truncates doubles.
    let coerced = json!({ "action": "play", "started": "0", "position": 600.9, "total": "1800" });
    assert_eq!(play_position(&coerced), Some(600));
}

#[test]
fn timestamps_are_read_as_utc_prefixes() {
    let at = |s: &str| action_timestamp(&json!({ "timestamp": s }));
    let want = chrono::NaiveDate::from_ymd_opt(2026, 1, 2).unwrap().and_hms_opt(3, 4, 5).unwrap();
    for s in ["2026-01-02T03:04:05", "2026-01-02T03:04:05Z", "2026-01-02T03:04:05.123Z", "2026-01-02T03:04:05+00:00"] {
        assert_eq!(at(s), Ok(Some(want)), "{s}");
    }
    assert!(at("2026-01-02T05:04:05+02:00").is_err(), "non-UTC offset is silently ignored by AntennaPod");
    assert!(at("2026-01-02 03:04:05").is_err());
    assert!(action_timestamp(&json!({ "timestamp": 1767323045 })).is_err(), "unix seconds don't parse");
    assert_eq!(action_timestamp(&json!({})), Ok(None));
}

#[test]
fn responses_need_arrays_and_integer_timestamps() {
    assert!(check_subscription_changes(&json!({ "add": [], "remove": [], "timestamp": 1 })).is_empty());
    assert!(!check_subscription_changes(&json!({ "add": null, "remove": [], "timestamp": 1 })).is_empty());
    assert!(!check_subscription_changes(&json!({ "add": [], "timestamp": 1 })).is_empty());
    assert!(!check_subscription_changes(&json!({ "add": [], "remove": [], "timestamp": "soon" })).is_empty());

    assert!(check_upload_response(&json!({ "timestamp": 1, "update_urls": [["a", "b"]] })).is_empty());
    assert!(!check_upload_response(&json!({ "timestamp": 1 })).is_empty());
    assert!(!check_upload_response(&json!({ "timestamp": 1, "update_urls": [["a"]] })).is_empty());

    assert!(check_episode_actions_response(&json!({ "actions": [], "timestamp": "12" })).is_empty());
    let unknown = json!({ "actions": [{ "podcast": "p", "episode": "e", "action": "stream" }], "timestamp": 1 });
    assert!(!check_episode_actions_response(&unknown).is_empty());

    assert!(check_devices(&json!([{ "id": "a", "caption": "A", "type": "mobile", "subscriptions": 3 }])).is_empty());
    assert!(!check_devices(&json!([{ "id": "a", "caption": null, "type": "mobile", "subscriptions": 3 }])).is_empty());
}
