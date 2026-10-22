// gpodder episode actions on the wire. Inbound parsing is lenient because clients disagree on
// timestamp and number formats; outbound is strict because AntennaPod resets an episode to the
// start for a `play` it doesn't like.

use chrono::{DateTime, NaiveDateTime, Timelike, Utc};
use serde::{de, Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionKind {
    Play,
    Download,
    Delete,
    New,
}

impl ActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ActionKind::Play => "play",
            ActionKind::Download => "download",
            ActionKind::Delete => "delete",
            ActionKind::New => "new",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "play" => Some(ActionKind::Play),
            "download" => Some(ActionKind::Download),
            "delete" => Some(ActionKind::Delete),
            "new" => Some(ActionKind::New),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for ActionKind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        ActionKind::parse(&s).ok_or_else(|| de::Error::custom(format!("unknown episode action {s:?}")))
    }
}

impl Serialize for ActionKind {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

/// One episode action as clients send it. `timestamp` is the client's action time, used only
/// for last-write-wins; it defaults to the receive time when a client leaves it out.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EpisodeAction {
    #[serde(rename = "podcast")]
    pub podcast_url: String,
    #[serde(rename = "episode")]
    pub episode_url: String,
    #[serde(default, deserialize_with = "non_empty_string")]
    pub guid: Option<String>,
    pub action: ActionKind,
    #[serde(default = "now_secs", deserialize_with = "timestamp")]
    pub timestamp: DateTime<Utc>,
    #[serde(default, deserialize_with = "lenient_int")]
    pub started: Option<i32>,
    #[serde(default, deserialize_with = "lenient_int")]
    pub position: Option<i32>,
    #[serde(default, deserialize_with = "lenient_int")]
    pub total: Option<i32>,
    #[serde(default, deserialize_with = "non_empty_string")]
    pub device: Option<String>,
}

/// An upload body: a bare array (the gpodder spec) or `{"actions": [...]}` (Nextcloud).
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum EpisodeActions {
    Bare(Vec<EpisodeAction>),
    Wrapped { actions: Vec<EpisodeAction> },
}

impl EpisodeActions {
    pub fn into_vec(self) -> Vec<EpisodeAction> {
        match self {
            EpisodeActions::Bare(v) | EpisodeActions::Wrapped { actions: v } => v,
        }
    }
}

/// An action as sent to clients and servers. Build it with [`EpisodeAction::outbound`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutboundAction {
    pub podcast: String,
    pub episode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guid: Option<String>,
    pub action: ActionKind,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i32>,
}

impl EpisodeAction {
    /// The wire form, or None for a `play` that must not be sent. AntennaPod resets the episode
    /// to the start for a `play` without `started >= 0`, `position > 0` and `total > 0`, so a
    /// play with no position is dropped and `total` falls back to `known_duration`, then to
    /// `position` (AntennaPod uses its own duration for "almost finished"). AntennaPod also
    /// ignores UTC offsets, so the timestamp is plain UTC without one.
    pub fn outbound(&self, known_duration: Option<i32>) -> Option<OutboundAction> {
        let (started, position, total) = if self.action == ActionKind::Play {
            let position = self.position.filter(|p| *p > 0)?;
            let total = self
                .total
                .filter(|t| *t > 0)
                .or(known_duration.filter(|d| *d > 0))
                .unwrap_or(position);
            (Some(self.started.unwrap_or(0).clamp(0, position)), Some(position), Some(total))
        } else {
            (None, None, None)
        };
        Some(OutboundAction {
            podcast: self.podcast_url.clone(),
            episode: self.episode_url.clone(),
            guid: self.guid.clone().filter(|g| !g.is_empty()),
            action: self.action,
            timestamp: format_timestamp(self.timestamp),
            device: self.device.clone(),
            started,
            position,
            total,
        })
    }
}

pub fn format_timestamp(ts: DateTime<Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%S").to_string()
}

/// Epoch seconds, or milliseconds when the value is too large to be seconds (year 5000+).
fn from_epoch(n: i64) -> Option<DateTime<Utc>> {
    if n.abs() >= 100_000_000_000 {
        DateTime::from_timestamp_millis(n)
    } else {
        DateTime::from_timestamp(n, 0)
    }
}

/// ISO 8601 with or without `Z`/offset (no offset = UTC), optional fractional seconds or a
/// space instead of `T`; or epoch seconds as a string. Truncated to whole seconds, the log's
/// resolution.
pub fn parse_timestamp(s: &str) -> Option<DateTime<Utc>> {
    parse_timestamp_exact(s).and_then(|t| t.with_nanosecond(0))
}

fn parse_timestamp_exact(s: &str) -> Option<DateTime<Utc>> {
    let s = s.trim();
    if let Ok(n) = s.parse::<i64>() {
        return from_epoch(n);
    }
    if let Ok(f) = s.parse::<f64>() {
        return f.is_finite().then(|| from_epoch(f as i64)).flatten();
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt.and_utc());
        }
    }
    None
}

fn now_secs() -> DateTime<Utc> {
    let now = Utc::now();
    now.with_nanosecond(0).unwrap_or(now)
}

fn timestamp<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Number(n) => n
            .as_i64()
            .and_then(from_epoch)
            .or_else(|| n.as_f64().filter(|f| f.is_finite()).and_then(|f| from_epoch(f as i64)))
            .and_then(|t| t.with_nanosecond(0))
            .ok_or_else(|| de::Error::custom(format!("bad timestamp {n}"))),
        serde_json::Value::String(s) => parse_timestamp(&s).ok_or_else(|| de::Error::custom(format!("bad timestamp {s:?}"))),
        serde_json::Value::Null => Ok(now_secs()),
        other => Err(de::Error::custom(format!("bad timestamp {other}"))),
    }
}

/// Integers, floats (truncated) or numeric strings; null and "" are None.
fn lenient_int<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i32>, D::Error> {
    let clamp = |n: i64| n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    match serde_json::Value::deserialize(d)? {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(n) => Ok(n.as_i64().or_else(|| n.as_f64().filter(|f| f.is_finite()).map(|f| f as i64)).map(clamp)),
        serde_json::Value::String(s) if s.trim().is_empty() => Ok(None),
        serde_json::Value::String(s) => {
            let s = s.trim();
            s.parse::<i64>()
                .ok()
                .or_else(|| s.parse::<f64>().ok().filter(|f| f.is_finite()).map(|f| f as i64))
                .map(|n| Some(clamp(n)))
                .ok_or_else(|| de::Error::custom(format!("bad number {s:?}")))
        }
        other => Err(de::Error::custom(format!("bad number {other}"))),
    }
}

fn non_empty_string<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(d)?.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    fn parse(v: serde_json::Value) -> EpisodeAction {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn inbound_timestamps_in_every_client_format() {
        let want = ts("2026-10-21T12:34:56Z");
        for t in [
            json!("2026-10-21T12:34:56"),
            json!("2026-10-21T12:34:56Z"),
            json!("2026-10-21T14:34:56+02:00"),
            json!("2026-10-21T12:34:56.789"),
            json!("2026-10-21 12:34:56"),
            json!(1_792_586_096),
            json!("1792586096"),
            json!(1_792_586_096_000_i64),
            json!(1_792_586_096.4),
        ] {
            let a = parse(json!({"podcast": "p", "episode": "e", "action": "play", "timestamp": t}));
            assert_eq!(a.timestamp, want, "{t}");
        }
    }

    #[test]
    fn inbound_missing_timestamp_is_now_and_garbage_is_an_error() {
        let before = now_secs();
        let a = parse(json!({"podcast": "p", "episode": "e", "action": "download"}));
        assert!(a.timestamp >= before && a.timestamp <= Utc::now());
        let bad: Result<EpisodeAction, _> =
            serde_json::from_value(json!({"podcast": "p", "episode": "e", "action": "play", "timestamp": "yesterday"}));
        assert!(bad.is_err());
    }

    #[test]
    fn inbound_fields_are_lenient() {
        let a = parse(json!({
            "podcast": "https://f/feed.xml", "episode": "https://f/1.mp3", "guid": "",
            "action": "PLAY", "timestamp": "2026-10-21T12:00:00",
            "started": "10", "position": 120.7, "total": null, "device": "phone"
        }));
        assert_eq!(a.action, ActionKind::Play);
        assert_eq!((a.started, a.position, a.total), (Some(10), Some(120), None));
        assert_eq!(a.guid, None, "an empty guid is no guid");
        assert_eq!(a.device.as_deref(), Some("phone"));
        let bad: Result<EpisodeAction, _> =
            serde_json::from_value(json!({"podcast": "p", "episode": "e", "action": "flattr"}));
        assert!(bad.is_err(), "unknown actions are rejected");
    }

    #[test]
    fn inbound_bare_array_or_wrapped() {
        let one = json!({"podcast": "p", "episode": "e", "action": "new", "timestamp": 0});
        let bare: EpisodeActions = serde_json::from_value(json!([one.clone(), one.clone()])).unwrap();
        let wrapped: EpisodeActions = serde_json::from_value(json!({"actions": [one]})).unwrap();
        assert_eq!(bare.into_vec().len(), 2);
        assert_eq!(wrapped.into_vec().len(), 1);
    }

    fn play(position: Option<i32>, total: Option<i32>) -> EpisodeAction {
        EpisodeAction {
            podcast_url: "https://f/feed.xml".into(),
            episode_url: "https://f/1.mp3".into(),
            guid: Some("g1".into()),
            action: ActionKind::Play,
            timestamp: ts("2026-10-21T12:34:56.9+00:00"),
            started: None,
            position,
            total,
            device: None,
        }
    }

    #[test]
    fn outbound_play_always_has_started_position_total() {
        let out = serde_json::to_value(play(Some(120), Some(3600)).outbound(None).unwrap()).unwrap();
        assert_eq!(
            out,
            json!({
                "podcast": "https://f/feed.xml", "episode": "https://f/1.mp3", "guid": "g1",
                "action": "play", "timestamp": "2026-10-21T12:34:56",
                "started": 0, "position": 120, "total": 3600
            })
        );
        // total falls back to the known duration, then to position
        assert_eq!(play(Some(120), None).outbound(Some(900)).unwrap().total, Some(900));
        assert_eq!(play(Some(120), Some(0)).outbound(None).unwrap().total, Some(120));
    }

    #[test]
    fn outbound_never_sends_a_play_without_a_position() {
        assert!(play(None, Some(100)).outbound(Some(100)).is_none());
        assert!(play(Some(0), Some(100)).outbound(Some(100)).is_none());
        assert!(play(Some(-5), Some(100)).outbound(Some(100)).is_none());
    }

    #[test]
    fn outbound_non_play_has_no_position_fields() {
        let mut a = play(Some(50), Some(100));
        a.action = ActionKind::New;
        a.guid = None;
        let out = serde_json::to_value(a.outbound(None).unwrap()).unwrap();
        assert_eq!(
            out,
            json!({"podcast": "https://f/feed.xml", "episode": "https://f/1.mp3", "action": "new", "timestamp": "2026-10-21T12:34:56"})
        );
    }

    #[test]
    fn round_trip_keeps_the_action() {
        let a = play(Some(300), Some(1200));
        let back: EpisodeAction = serde_json::from_value(serde_json::to_value(a.outbound(None).unwrap()).unwrap()).unwrap();
        assert_eq!((back.position, back.total, back.started), (Some(300), Some(1200), Some(0)));
        assert_eq!(back.timestamp, ts("2026-10-21T12:34:56Z"));
        assert_eq!(back.guid.as_deref(), Some("g1"));
    }
}
