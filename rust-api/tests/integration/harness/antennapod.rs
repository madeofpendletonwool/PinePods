//! AntennaPod's gpodder response parsing, as assertions.
//!
//! Ported from `AntennaPod/net/sync` (`ResponseMapper`, `EpisodeAction.readFromJsonObject`,
//! `GpodnetUploadChangesResponse`, `GpodnetService.readDeviceFromJsonObject`). AntennaPod is
//! the client most Pinepods users sync, and it fails quietly: a `play` action it can't read
//! still applies, with position −1 s, which puts the episode back to the start (#636).
//! Each `check_*` returns the list of rule violations; the `assert_*` wrappers panic on any.

use serde_json::Value;

/// `JSONObject.getLong`: a number, or a string holding one.
fn is_long(v: &Value) -> bool {
    v.as_i64().is_some() || v.as_str().is_some_and(|s| s.trim().parse::<i64>().is_ok())
}

/// `JSONObject.getJSONArray` then `getString(i)` on each element.
fn string_array(v: &Value, key: &str, errs: &mut Vec<String>) {
    match v.get(key) {
        Some(Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                if !item.is_string() {
                    errs.push(format!("{key}[{i}] is not a string: {item}"));
                }
            }
        }
        Some(other) => errs.push(format!("`{key}` must be an array, got {other}")),
        None => errs.push(format!("`{key}` is missing")),
    }
}

/// `optInt(key, -1)`: numbers (truncated) and numeric strings; anything else is −1.
fn opt_int(action: &Value, key: &str) -> i64 {
    match action.get(key) {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)).unwrap_or(-1),
        Some(Value::String(s)) => s.trim().parse::<f64>().map(|f| f as i64).unwrap_or(-1),
        _ => -1,
    }
}

/// The position AntennaPod applies for a remote `play` action, in seconds. `None` means the
/// action is read with position −1, i.e. the episode is reset to the start.
pub fn play_position(action: &Value) -> Option<i64> {
    let started = opt_int(action, "started");
    let position = opt_int(action, "position");
    let total = opt_int(action, "total");
    (started >= 0 && position > 0 && total > 0).then_some(position)
}

/// How AntennaPod reads an action's `timestamp`: `SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss")`
/// in UTC. It parses a prefix and ignores the rest, so a fractional part or `Z` is harmless,
/// but any offset is silently dropped and a non-UTC one shifts the time. Returns the UTC
/// time it ends up with, or why it can't read it.
pub fn action_timestamp(action: &Value) -> Result<Option<chrono::NaiveDateTime>, String> {
    let raw = match action.get("timestamp") {
        None | Some(Value::Null) => return Ok(None),
        Some(Value::String(s)) if s.is_empty() => return Ok(None),
        Some(Value::String(s)) => s.as_str(),
        // optString on a number yields its digits, which the date parser rejects.
        Some(other) => return Err(format!("timestamp {other} is not a yyyy-MM-ddTHH:mm:ss string")),
    };
    let head = raw.get(..19).ok_or_else(|| format!("timestamp {raw:?} is too short"))?;
    let parsed = chrono::NaiveDateTime::parse_from_str(head, "%Y-%m-%dT%H:%M:%S")
        .map_err(|e| format!("timestamp {raw:?} doesn't start with yyyy-MM-ddTHH:mm:ss ({e})"))?;
    let mut rest = &raw[19..];
    if let Some(frac) = rest.strip_prefix('.') {
        rest = frac.trim_start_matches(|c: char| c.is_ascii_digit());
    }
    match rest {
        "" | "Z" | "+00:00" | "-00:00" | "+0000" | "+00" => Ok(Some(parsed)),
        offset => Err(format!("timestamp {raw:?} has offset {offset:?}, which AntennaPod ignores (it reads the time as UTC)")),
    }
}

/// One action from `GET /api/2/episodes/{user}.json`.
pub fn check_episode_action(i: usize, a: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    for key in ["podcast", "episode", "action"] {
        if !a.get(key).and_then(Value::as_str).is_some_and(|s| !s.is_empty()) {
            errs.push(format!("actions[{i}].{key} missing or empty, so AntennaPod drops the action: {a}"));
        }
    }
    let action = a.get("action").and_then(Value::as_str).unwrap_or_default().to_ascii_lowercase();
    if !["new", "download", "play", "delete"].contains(&action.as_str()) {
        errs.push(format!("actions[{i}].action {action:?} is not one AntennaPod knows: {a}"));
    }
    if let Err(e) = action_timestamp(a) {
        errs.push(format!("actions[{i}]: {e}"));
    }
    if action == "play" && play_position(a).is_none() {
        errs.push(format!(
            "actions[{i}] is a play action without started >= 0, position > 0, total > 0; \
             AntennaPod resets the episode to 0: {a}"
        ));
    }
    errs
}

pub fn check_episode_actions_response(body: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    if !body.get("timestamp").is_some_and(is_long) {
        errs.push(format!("`timestamp` must be an integer, got {:?}", body.get("timestamp")));
    }
    match body.get("actions") {
        Some(Value::Array(actions)) => {
            for (i, a) in actions.iter().enumerate() {
                if !a.is_object() {
                    errs.push(format!("actions[{i}] is not an object: {a}"));
                    continue;
                }
                errs.extend(check_episode_action(i, a));
            }
        }
        other => errs.push(format!("`actions` must be an array, got {other:?}")),
    }
    errs
}

pub fn check_subscription_changes(body: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    string_array(body, "add", &mut errs);
    string_array(body, "remove", &mut errs);
    if !body.get("timestamp").is_some_and(is_long) {
        errs.push(format!("`timestamp` must be an integer, got {:?}", body.get("timestamp")));
    }
    errs
}

/// Response to `POST` subscriptions or episodes: `timestamp` + `update_urls` pairs.
pub fn check_upload_response(body: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    if !body.get("timestamp").is_some_and(is_long) {
        errs.push(format!("`timestamp` must be an integer, got {:?}", body.get("timestamp")));
    }
    match body.get("update_urls") {
        Some(Value::Array(pairs)) => {
            for (i, p) in pairs.iter().enumerate() {
                let ok = p.as_array().is_some_and(|p| p.len() >= 2 && p[0].is_string() && p[1].is_string());
                if !ok {
                    errs.push(format!("update_urls[{i}] is not a [old, new] string pair: {p}"));
                }
            }
        }
        other => errs.push(format!("`update_urls` must be an array, got {other:?}")),
    }
    errs
}

pub fn check_devices(body: &Value) -> Vec<String> {
    let mut errs = Vec::new();
    let Some(devices) = body.as_array() else {
        return vec![format!("device list must be an array, got {body}")];
    };
    for (i, d) in devices.iter().enumerate() {
        for key in ["id", "caption", "type"] {
            if !d.get(key).is_some_and(Value::is_string) {
                errs.push(format!("devices[{i}].{key} must be a string: {d}"));
            }
        }
        if !d.get("subscriptions").is_some_and(|v| v.as_i64().is_some()) {
            errs.push(format!("devices[{i}].subscriptions must be an integer: {d}"));
        }
    }
    errs
}

fn assert_none(what: &str, errs: Vec<String>, body: &Value) {
    assert!(errs.is_empty(), "AntennaPod can't use this {what}:\n  - {}\nbody: {body}", errs.join("\n  - "));
}

pub fn assert_episode_actions_response(body: &Value) {
    assert_none("episode actions response", check_episode_actions_response(body), body);
}
pub fn assert_subscription_changes(body: &Value) {
    assert_none("subscription changes response", check_subscription_changes(body), body);
}
pub fn assert_upload_response(body: &Value) {
    assert_none("upload response", check_upload_response(body), body);
}
pub fn assert_devices(body: &Value) {
    assert_none("device list", check_devices(body), body);
}

/// `timestamp` of a response, as AntennaPod stores it for its next `since`.
pub fn cursor(body: &Value) -> i64 {
    let t = &body["timestamp"];
    t.as_i64()
        .or_else(|| t.as_str().and_then(|s| s.trim().parse().ok()))
        .unwrap_or_else(|| panic!("no usable timestamp in {body}"))
}
