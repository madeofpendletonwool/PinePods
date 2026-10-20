//! Tiny RSS 2.0 + iTunes + Podcasting 2.0 feed builder for fixtures.

use chrono::{DateTime, Duration, TimeZone, Utc};

#[derive(Clone)]
pub struct Episode {
    pub guid: Option<String>,
    pub title: String,
    pub description: String,
    pub pub_date: DateTime<Utc>,
    pub duration_secs: Option<u32>,
    pub enclosure_url: Option<String>,
    pub enclosure_type: String,
    pub enclosure_len: u64,
    pub image: Option<String>,
    pub episode_number: Option<i32>,
    pub season: Option<i32>,
    /// Raw XML appended inside <item> (podcast:transcript, chapters, …).
    pub extra: String,
}

impl Episode {
    /// `n` makes titles, GUIDs and enclosure URLs unique and orders pub dates (higher = newer).
    pub fn new(n: u32) -> Self {
        Episode {
            guid: Some(format!("guid-{n}")),
            title: format!("Episode {n}"),
            description: format!("Description of episode {n}"),
            pub_date: Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap() + Duration::days(n as i64),
            duration_secs: Some(1800 + n),
            enclosure_url: Some(format!("https://cdn.example.invalid/audio/ep{n}.mp3")),
            enclosure_type: "audio/mpeg".into(),
            enclosure_len: 12_345_678,
            image: None,
            episode_number: Some(n as i32),
            season: None,
            extra: String::new(),
        }
    }
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.into();
        self
    }
    pub fn guid(mut self, g: Option<&str>) -> Self {
        self.guid = g.map(str::to_string);
        self
    }
    pub fn description(mut self, d: &str) -> Self {
        self.description = d.into();
        self
    }
    pub fn pub_date(mut self, d: DateTime<Utc>) -> Self {
        self.pub_date = d;
        self
    }
    pub fn duration(mut self, secs: Option<u32>) -> Self {
        self.duration_secs = secs;
        self
    }
    pub fn enclosure(mut self, url: &str) -> Self {
        self.enclosure_url = Some(url.into());
        self
    }
    pub fn video(mut self) -> Self {
        self.enclosure_type = "video/mp4".into();
        self
    }
    pub fn extra(mut self, xml: &str) -> Self {
        self.extra.push_str(xml);
        self
    }
}

#[derive(Clone)]
pub struct Feed {
    pub title: String,
    pub author: String,
    pub description: String,
    pub link: String,
    pub image: String,
    pub categories: Vec<String>,
    pub explicit: bool,
    pub episodes: Vec<Episode>,
    /// Raw XML appended inside <channel>.
    pub extra: String,
}

impl Feed {
    pub fn new(title: &str) -> Self {
        Feed {
            title: title.into(),
            author: format!("{title} Author"),
            description: format!("All about {title}"),
            link: "https://example.invalid/show".into(),
            image: "https://example.invalid/art.jpg".into(),
            categories: vec!["Technology".into()],
            explicit: false,
            episodes: Vec::new(),
            extra: String::new(),
        }
    }
    /// A feed with episodes 1..=n.
    pub fn with_episodes(title: &str, n: u32) -> Self {
        let mut f = Feed::new(title);
        for i in 1..=n {
            f.episodes.push(Episode::new(i));
        }
        f
    }
    pub fn episode(mut self, e: Episode) -> Self {
        self.episodes.push(e);
        self
    }
    pub fn categories(mut self, cats: &[&str]) -> Self {
        self.categories = cats.iter().map(|c| c.to_string()).collect();
        self
    }
    pub fn author(mut self, a: &str) -> Self {
        self.author = a.into();
        self
    }
    pub fn extra(mut self, xml: &str) -> Self {
        self.extra.push_str(xml);
        self
    }

    pub fn to_xml(&self) -> String {
        let mut items = String::new();
        // Newest first, like real feeds.
        let mut eps = self.episodes.clone();
        eps.sort_by(|a, b| b.pub_date.cmp(&a.pub_date));
        for e in &eps {
            items.push_str("    <item>\n");
            items.push_str(&format!("      <title>{}</title>\n", esc(&e.title)));
            items.push_str(&format!("      <description>{}</description>\n", esc(&e.description)));
            items.push_str(&format!("      <pubDate>{}</pubDate>\n", e.pub_date.to_rfc2822()));
            if let Some(g) = &e.guid {
                items.push_str(&format!("      <guid isPermaLink=\"false\">{}</guid>\n", esc(g)));
            }
            if let Some(u) = &e.enclosure_url {
                items.push_str(&format!(
                    "      <enclosure url=\"{}\" length=\"{}\" type=\"{}\"/>\n",
                    esc(u),
                    e.enclosure_len,
                    e.enclosure_type
                ));
            }
            if let Some(d) = e.duration_secs {
                items.push_str(&format!("      <itunes:duration>{d}</itunes:duration>\n"));
            }
            if let Some(n) = e.episode_number {
                items.push_str(&format!("      <itunes:episode>{n}</itunes:episode>\n"));
            }
            if let Some(s) = e.season {
                items.push_str(&format!("      <itunes:season>{s}</itunes:season>\n"));
            }
            if let Some(img) = &e.image {
                items.push_str(&format!("      <itunes:image href=\"{}\"/>\n", esc(img)));
            }
            items.push_str(&e.extra);
            items.push_str("    </item>\n");
        }
        let cats: String = self
            .categories
            .iter()
            .map(|c| format!("    <itunes:category text=\"{}\"/>\n", esc(c)))
            .collect();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:podcast="https://podcastindex.org/namespace/1.0" xmlns:content="http://purl.org/rss/1.0/modules/content/">
  <channel>
    <title>{title}</title>
    <link>{link}</link>
    <description>{desc}</description>
    <language>en</language>
    <itunes:author>{author}</itunes:author>
    <itunes:explicit>{explicit}</itunes:explicit>
    <itunes:image href="{image}"/>
    <image><url>{image}</url><title>{title}</title><link>{link}</link></image>
{cats}{extra}{items}  </channel>
</rss>
"#,
            title = esc(&self.title),
            link = esc(&self.link),
            desc = esc(&self.description),
            author = esc(&self.author),
            explicit = if self.explicit { "true" } else { "false" },
            image = esc(&self.image),
            cats = cats,
            extra = self.extra,
            items = items,
        )
    }
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
