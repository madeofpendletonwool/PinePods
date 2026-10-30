//! YouTube without YouTube: channel feeds (videos.xml) come from the fixture server and yt-dlp is
//! a stub script that answers from files and logs every call.
//!
//! The stub is installed once per test process (`YTDLP_PATH`). For each `watch?v=<id>` argument
//! it prints `videos/<id>.json`, for a `/channel/<id>` argument `channels/<id>.jsonl` (or the
//! channel page `channels/<id>.json` under `--dump-single-json`), and fails like yt-dlp for
//! anything it has no file for. Tests isolate themselves by id (channel and video
//! ids embed the test's tag).

use std::path::{Path, PathBuf};

use chrono::{DateTime, Duration, Utc};
use serde_json::json;

use super::{eventually, int, TestApp, TestUser};
use super::fixtures::Fixture;

const STUB: &str = r#"#!/bin/sh
dir="$(dirname "$0")"
echo "$*" >> "$dir/calls.log"
status=0
single=0
for a in "$@"; do [ "$a" = "--dump-single-json" ] && single=1; done
for a in "$@"; do
  case "$a" in
    *watch\?v=*)
      id="${a##*v=}"
      if [ -f "$dir/videos/$id.json" ]; then cat "$dir/videos/$id.json"; echo; else echo "ERROR: [youtube] $id: Video unavailable" >&2; status=1; fi ;;
    */channel/*)
      ch="${a#*/channel/}"; ch="${ch%%/*}"
      if [ $single = 1 ]; then file="$dir/channels/$ch.json"; else file="$dir/channels/$ch.jsonl"; fi
      if [ -f "$file" ]; then cat "$file"; else echo "ERROR: [youtube:tab] $ch: This channel does not exist." >&2; status=1; fi ;;
  esac
done
exit $status
"#;

/// Write the stub into `dir` and point the server at it. Call before any server code runs.
pub fn install_stub(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir.join("videos")).unwrap();
    std::fs::create_dir_all(dir.join("channels")).unwrap();
    let bin = dir.join("yt-dlp");
    std::fs::write(&bin, STUB).unwrap();
    std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
    // SAFETY (edition 2021 API): runs once, before the server code under test starts threads.
    std::env::set_var("YTDLP_PATH", &bin);
}

#[derive(Clone)]
pub struct Video {
    pub id: String,
    pub title: String,
    pub published: DateTime<Utc>,
    pub duration: u32,
}

#[derive(Clone)]
pub struct Channel {
    pub id: String,
    pub name: String,
    /// Newest first, like the real feed.
    pub videos: Vec<Video>,
    /// View counts in the feed, which change on every fetch.
    pub views: u64,
}

impl Channel {
    /// Add a video published `days_ago` days ago (keeps newest first).
    pub fn video(mut self, n: u32, days_ago: i64) -> Self {
        let id = format!("{}v{n}", self.id);
        self.videos.push(Video {
            title: format!("Video {n}"),
            published: Utc::now() - Duration::days(days_ago) - Duration::minutes(n as i64),
            duration: 600 + n,
            id,
        });
        self.videos.sort_by(|a, b| b.published.cmp(&a.published));
        self
    }

    /// Like `video`, with a title and a length in minutes.
    pub fn video_as(mut self, n: u32, days_ago: i64, title: &str, minutes: u32) -> Self {
        self = self.video(n, days_ago);
        let v = self.videos.iter_mut().find(|v| v.id.ends_with(&format!("v{n}"))).unwrap();
        v.title = title.to_string();
        v.duration = minutes * 60;
        self
    }

    pub fn video_id(&self, n: u32) -> String {
        format!("{}v{n}", self.id)
    }

    pub fn url(&self) -> String {
        format!("https://www.youtube.com/channel/{}", self.id)
    }

    pub fn to_atom(&self) -> String {
        let entries: String = self
            .videos
            .iter()
            .map(|v| {
                format!(
                    r#"<entry><id>yt:video:{id}</id><yt:videoId>{id}</yt:videoId><yt:channelId>{ch}</yt:channelId>
<title>{title}</title><link rel="alternate" href="https://www.youtube.com/watch?v={id}"/>
<author><name>{name}</name></author><published>{date}</published><updated>{date}</updated>
<media:group><media:title>{title}</media:title><media:thumbnail url="https://i.ytimg.com/vi/{id}/hqdefault.jpg" width="480" height="360"/>
<media:description>About {title}</media:description>
<media:community><media:starRating count="3" average="5.00" min="1" max="5"/><media:statistics views="{views}"/></media:community></media:group></entry>
"#,
                    id = v.id,
                    ch = self.id,
                    title = v.title,
                    name = self.name,
                    date = v.published.to_rfc3339(),
                    views = self.views,
                )
            })
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns:yt="http://www.youtube.com/xml/schemas/2015" xmlns:media="http://search.yahoo.com/mrss/" xmlns="http://www.w3.org/2005/Atom">
<link rel="self" href="https://www.youtube.com/feeds/videos.xml?channel_id={ch}"/>
<id>yt:channel:{ch}</id><yt:channelId>{ch}</yt:channelId><title>{name}</title>
<author><name>{name}</name><uri>https://www.youtube.com/channel/{ch}</uri></author>
{entries}</feed>
"#,
            ch = self.id,
            name = self.name,
        )
    }

    fn video_json(&self, v: &Video) -> serde_json::Value {
        json!({
            "id": v.id,
            "title": v.title,
            "description": format!("About {}", v.title),
            "timestamp": v.published.timestamp(),
            "upload_date": v.published.format("%Y%m%d").to_string(),
            "duration": v.duration as f64,
            "channel": self.name,
            "thumbnail": format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", v.id),
        })
    }
}

impl TestApp {
    fn ytdlp_dir(&self) -> &PathBuf {
        &self.env.ytdlp_dir
    }

    /// A channel unique to this test.
    pub fn youtube_channel(&self, name: &str) -> Channel {
        Channel { id: format!("UC{}{name}", self.tag), name: format!("{name} Channel"), videos: Vec::new(), views: 1 }
    }

    /// Serve the channel's feed and give the yt-dlp stub its videos.
    pub fn publish_channel(&self, channel: &Channel) {
        self.fixtures().serve(&format!("youtube/{}.xml", channel.id), Fixture::new("application/atom+xml", channel.to_atom()));
        let dir = self.ytdlp_dir();
        let mut listing = String::new();
        for v in &channel.videos {
            let line = channel.video_json(v).to_string();
            std::fs::write(dir.join("videos").join(format!("{}.json", v.id)), &line).unwrap();
            listing.push_str(&line);
            listing.push('\n');
        }
        std::fs::write(dir.join("channels").join(format!("{}.jsonl", channel.id)), listing).unwrap();
        let page = json!({
            "id": channel.id,
            "channel": channel.name,
            "description": format!("About {}", channel.name),
            "thumbnails": [
                {"id": "banner_uncropped", "url": format!("https://yt3.example/{}/banner", channel.id)},
                {"id": "avatar_uncropped", "url": format!("https://yt3.example/{}/avatar", channel.id)},
            ],
            "entries": [],
        });
        std::fs::write(dir.join("channels").join(format!("{}.json", channel.id)), page.to_string()).unwrap();
    }

    /// Make yt-dlp unable to read this video (members-only, removed, ...).
    pub fn hide_video(&self, video_id: &str) {
        let _ = std::fs::remove_file(self.ytdlp_dir().join("videos").join(format!("{video_id}.json")));
    }

    /// Feed requests for the channel so far.
    pub fn channel_feed_hits(&self, channel: &Channel) -> usize {
        self.fixtures().hits(&format!("youtube/{}.xml", channel.id)).len()
    }

    /// yt-dlp calls that mention the channel or one of its videos, as their argument lines.
    pub fn ytdlp_calls(&self, channel: &Channel) -> Vec<String> {
        std::fs::read_to_string(self.ytdlp_dir().join("calls.log"))
            .unwrap_or_default()
            .lines()
            .filter(|l| l.contains(&channel.id))
            .map(str::to_string)
            .collect()
    }

    /// The video ids yt-dlp was asked for details of (`--dump-json` runs on videos).
    pub fn ytdlp_detail_requests(&self, channel: &Channel) -> Vec<Vec<String>> {
        self.ytdlp_calls(channel)
            .iter()
            .filter(|l| l.contains("--dump-json") && l.contains("watch?v="))
            .map(|l| l.split(' ').filter_map(|a| a.split_once("watch?v=").map(|(_, id)| id.to_string())).collect())
            .collect()
    }

    /// Subscribe through `/api/data/youtube/subscribe` and wait for the first import to finish.
    pub async fn subscribe_channel(&self, user: &TestUser, channel: &Channel, cutoff_days: i32) -> i32 {
        let resp = self
            .post(&format!(
                "/api/data/youtube/subscribe?channel_id={}&user_id={}&feed_cutoff={cutoff_days}",
                channel.id, user.id
            ))
            .key(user)
            .send()
            .await;
        resp.assert_ok();
        let pod = resp.json()["podcast_id"].as_i64().expect("podcast_id") as i32;
        eventually("channel import", || async {
            int(self, &format!(r#"SELECT COUNT(*) FROM "Podcasts" WHERE PodcastID = {pod} AND LastRefreshSuccess IS NOT NULL"#))
                .await
                .filter(|n| *n == 1)
        })
        .await;
        pod
    }

    /// YouTube ids of a channel podcast's stored videos, sorted.
    pub async fn video_ids(&self, podcast_id: i32) -> Vec<String> {
        let mut ids: Vec<String> = self.db().youtube_video_ids(podcast_id).await.unwrap().into_iter().collect();
        ids.sort();
        ids
    }
}
