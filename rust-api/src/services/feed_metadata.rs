//! A podcast's own details (name, artwork, author, description, website) as its feed currently
//! gives them. Adding a podcast stores them, and every refresh that reads a changed feed brings
//! the stored ones up to date, so a rebranded show follows its feed. A field the user edited is
//! theirs: refreshes leave it alone until they reset it.

use quick_xml::events::{BytesStart, Event};
use sqlx::Row;
use tracing::{debug, warn};

use crate::database::DatabasePool;
use crate::error::AppResult;

/// The fields a refresh keeps in step with the feed, by their `Podcasts` column (lowercase).
/// `Podcasts.EditedMetadata` lists the ones the user edited, comma-separated.
pub const FIELDS: [&str; 5] = ["podcastname", "description", "author", "artworkurl", "websiteurl"];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FeedMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    pub artwork: Option<String>,
    pub website: Option<String>,
}

impl FeedMetadata {
    /// `(column, value)` for each field the feed gives.
    fn fields(&self) -> Vec<(&'static str, &str)> {
        [
            ("podcastname", &self.name),
            ("description", &self.description),
            ("author", &self.author),
            ("artworkurl", &self.artwork),
            ("websiteurl", &self.website),
        ]
        .into_iter()
        .filter_map(|(col, v)| v.as_deref().map(|v| (col, v)))
        .collect()
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Read a feed's details. None when it isn't a feed; a field the feed leaves out or empty is None.
pub fn parse(body: &str) -> Option<FeedMetadata> {
    let feed = feed_rs::parser::parse(body.as_bytes()).ok()?;
    // The <link> to the show's site, not the feed's own atom:link rel="self".
    let website = feed
        .links
        .iter()
        .find(|l| matches!(l.rel.as_deref(), None | Some("alternate")))
        .map(|l| l.href.clone());
    // <itunes:image> is what podcast apps show and what shows update when they rebrand; feed-rs
    // prefers the RSS <image>, which is often older or smaller.
    let artwork = channel_itunes_image(body)
        .or_else(|| feed.logo.as_ref().map(|l| l.uri.clone()))
        .or_else(|| feed.icon.as_ref().map(|i| i.uri.clone()));
    Some(FeedMetadata {
        name: non_empty(feed.title.map(|t| t.content)),
        description: non_empty(feed.description.map(|d| d.content)),
        author: non_empty(feed.authors.first().and_then(|a| a.name.clone())),
        artwork: non_empty(artwork),
        website: non_empty(website),
    })
}

/// The channel's `<itunes:image href>`, ignoring the ones on episodes.
fn channel_itunes_image(body: &str) -> Option<String> {
    let mut reader = quick_xml::Reader::from_str(body);
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                if matches!(name.as_ref(), "item" | "entry") {
                    return None;
                }
                if name.prefix().is_some_and(|p| p.as_ref() == "itunes") && name.local_name().as_ref() == "image" {
                    if let Some(href) = attr(&e, "href") {
                        return Some(href);
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
    }
}

/// Each episode's own `<itunes:image>`. feed-rs drops an item's iTunes tags unless it also has an
/// iTunes title or summary, so these are read from the XML directly.
#[derive(Debug, Default)]
pub struct ItemImages {
    by_guid: std::collections::HashMap<String, String>,
    by_enclosure: std::collections::HashMap<String, String>,
    by_title: std::collections::HashMap<String, String>,
}

impl ItemImages {
    /// The image of the episode with this GUID, else this enclosure URL, else this title.
    pub fn find(&self, guid: Option<&str>, enclosure: Option<&str>, title: Option<&str>) -> Option<&str> {
        guid.and_then(|g| self.by_guid.get(g.trim()))
            .or_else(|| enclosure.and_then(|e| self.by_enclosure.get(e.trim())))
            .or_else(|| title.and_then(|t| self.by_title.get(t.trim())))
            .map(String::as_str)
    }
}

#[derive(Default)]
struct Item {
    guid: Option<String>,
    enclosure: Option<String>,
    title: Option<String>,
    image: Option<String>,
}

fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key)
        .and_then(|a| a.normalized_value(quick_xml::XmlVersion::Implicit1_0).ok().map(|v| v.trim().to_string()))
        .filter(|v| !v.is_empty())
}

/// An item's direct child carrying an attribute we keep.
fn read_item_tag(e: &BytesStart, depth: usize, item: &mut Item) {
    if depth != 1 {
        return;
    }
    let name = e.name();
    if name.prefix().is_none() && name.as_ref() == "enclosure" {
        item.enclosure = item.enclosure.take().or_else(|| attr(e, "url"));
    } else if name.prefix().is_some_and(|p| p.as_ref() == "itunes") && name.local_name().as_ref() == "image" {
        item.image = item.image.take().or_else(|| attr(e, "href"));
    }
}

pub fn item_images(body: &str) -> ItemImages {
    let mut images = ItemImages::default();
    let mut reader = quick_xml::Reader::from_str(body);
    let mut item: Option<Item> = None;
    // Depth inside the current <item>: 1 for its direct children.
    let mut depth = 0usize;
    // The direct child whose text is being read (title or guid).
    let mut capture: Option<&'static str> = None;
    let mut text = String::new();
    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(ev) => ev,
        };
        match event {
            Event::Start(e) => {
                let name = e.name();
                if item.is_none() {
                    if name.as_ref() == "item" {
                        item = Some(Item::default());
                        depth = 0;
                    }
                    continue;
                }
                depth += 1;
                if depth == 1 && name.prefix().is_none() {
                    capture = match name.as_ref() {
                        "title" => Some("title"),
                        "guid" => Some("guid"),
                        _ => None,
                    };
                    text.clear();
                }
                read_item_tag(&e, depth, item.as_mut().unwrap());
            }
            Event::Empty(e) => {
                if let Some(it) = item.as_mut() {
                    read_item_tag(&e, depth + 1, it);
                }
            }
            Event::Text(t) if capture.is_some() => text.push_str(&t.xml10_content()),
            Event::GeneralRef(r) if capture.is_some() => {
                text.push('&');
                text.push_str(&r.xml10_content());
                text.push(';');
            }
            Event::CData(c) if capture.is_some() => text.push_str(&quick_xml::escape::escape(c.into_inner().into_owned())),
            Event::End(e) => {
                let Some(it) = item.as_mut() else { continue };
                if depth == 0 {
                    if e.name().as_ref() == "item" {
                        let it = item.take().unwrap();
                        if let Some(image) = it.image {
                            if let Some(g) = it.guid {
                                images.by_guid.entry(g).or_insert_with(|| image.clone());
                            }
                            if let Some(u) = it.enclosure {
                                images.by_enclosure.entry(u).or_insert_with(|| image.clone());
                            }
                            if let Some(t) = it.title {
                                images.by_title.entry(t).or_insert(image);
                            }
                        }
                    }
                    continue;
                }
                if depth == 1 {
                    if let Some(field) = capture.take() {
                        let raw = std::mem::take(&mut text);
                        let v = quick_xml::escape::unescape(&raw).map(|c| c.trim().to_string()).unwrap_or_else(|_| raw.trim().to_string());
                        if !v.is_empty() {
                            match field {
                                "title" => it.title = Some(v),
                                _ => it.guid = Some(v),
                            }
                        }
                    }
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    images
}

/// Add `fields` to an `EditedMetadata` list, keeping known fields only.
pub fn merge_edited(current: Option<&str>, fields: &[&str]) -> Option<String> {
    let mut all: Vec<&str> = current.unwrap_or("").split(',').map(str::trim).filter(|f| FIELDS.contains(f)).collect();
    for f in fields {
        if FIELDS.contains(f) && !all.contains(f) {
            all.push(f);
        }
    }
    (!all.is_empty()).then(|| all.join(","))
}

impl DatabasePool {
    /// Bring these podcast rows' details up to the feed's, skipping fields their owner edited.
    /// Episodes of the feed still showing a replaced podcast artwork (they had none of their own)
    /// get the new one. Failures are logged and never fail the refresh.
    pub async fn store_feed_metadata(&self, feed_id: Option<i32>, podcast_ids: &[i32], meta: &FeedMetadata) {
        if podcast_ids.is_empty() {
            return;
        }
        if let Err(e) = self.apply_feed_metadata(feed_id, podcast_ids, meta).await {
            warn!("Updating podcast details from the feed failed for podcasts {:?}: {}", podcast_ids, e);
        }
    }

    async fn apply_feed_metadata(&self, feed_id: Option<i32>, podcast_ids: &[i32], meta: &FeedMetadata) -> AppResult<()> {
        let feed_id = match feed_id {
            Some(id) => Some(id),
            None => self.feed_of_podcast(podcast_ids[0]).await.ok(),
        };
        // Every podcast artwork this feed's rows hold, so episodes that took one as their
        // fallback follow the new artwork too.
        let old_artwork: Vec<String> = match &meta.artwork {
            Some(new) => self
                .podcast_artworks(podcast_ids)
                .await?
                .into_iter()
                .filter(|a| a != new && !a.is_empty())
                .collect(),
            None => Vec::new(),
        };

        let mut changed = 0u64;
        for (column, value) in meta.fields() {
            changed += match self {
                DatabasePool::Postgres(pool) => {
                    // Column names come from FIELDS, never from input.
                    sqlx::query(sqlx::AssertSqlSafe(format!(
                        r#"UPDATE "Podcasts" SET {column} = $1
                           WHERE podcastid = ANY($2)
                             AND {column} IS DISTINCT FROM $1
                             AND NOT ($3 = ANY(string_to_array(COALESCE(editedmetadata, ''), ',')))"#
                    )))
                    .bind(value)
                    .bind(podcast_ids)
                    .bind(column)
                    .execute(pool)
                    .await?
                    .rows_affected()
                }
                DatabasePool::MySQL(pool) => {
                    let marks = vec!["?"; podcast_ids.len()].join(", ");
                    let sql = format!(
                        "UPDATE Podcasts SET {column} = ? \
                         WHERE PodcastID IN ({marks}) \
                           AND NOT ({column} <=> ?) \
                           AND FIND_IN_SET(?, COALESCE(EditedMetadata, '')) = 0"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(value);
                    for id in podcast_ids {
                        q = q.bind(id);
                    }
                    q.bind(value).bind(column).execute(pool).await?.rows_affected()
                }
            };
        }

        if let (Some(feed_id), Some(new), false) = (feed_id, &meta.artwork, old_artwork.is_empty()) {
            let n = match self {
                DatabasePool::Postgres(pool) => {
                    sqlx::query(r#"UPDATE "Episodes" SET episodeartwork = $1 WHERE feedid = $2 AND episodeartwork = ANY($3)"#)
                        .bind(new)
                        .bind(feed_id)
                        .bind(&old_artwork)
                        .execute(pool)
                        .await?
                        .rows_affected()
                }
                DatabasePool::MySQL(pool) => {
                    let marks = vec!["?"; old_artwork.len()].join(", ");
                    let sql = format!("UPDATE Episodes SET EpisodeArtwork = ? WHERE FeedID = ? AND EpisodeArtwork IN ({marks})");
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(new).bind(feed_id);
                    for a in &old_artwork {
                        q = q.bind(a);
                    }
                    q.execute(pool).await?.rows_affected()
                }
            };
            debug!("Feed {}: {} episodes moved to the new podcast artwork", feed_id, n);
        }
        if changed > 0 {
            debug!("Updated {} podcast details from the feed for podcasts {:?}", changed, podcast_ids);
        }
        Ok(())
    }

    async fn podcast_artworks(&self, podcast_ids: &[i32]) -> AppResult<Vec<String>> {
        Ok(match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar(r#"SELECT DISTINCT artworkurl FROM "Podcasts" WHERE podcastid = ANY($1) AND artworkurl IS NOT NULL"#)
                    .bind(podcast_ids)
                    .fetch_all(pool)
                    .await?
            }
            DatabasePool::MySQL(pool) => {
                let marks = vec!["?"; podcast_ids.len()].join(", ");
                let sql = format!("SELECT DISTINCT ArtworkURL FROM Podcasts WHERE PodcastID IN ({marks}) AND ArtworkURL IS NOT NULL");
                let mut q = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()));
                for id in podcast_ids {
                    q = q.bind(id);
                }
                q.fetch_all(pool).await?
            }
        })
    }

    /// Mark the details a user is about to change by hand as theirs, so refreshes keep them.
    /// `edits` are `(column, new value)`; only values that differ from the stored ones count.
    pub async fn mark_metadata_edited(&self, podcast_id: i32, user_id: i32, edits: &[(&str, &str)]) -> AppResult<()> {
        if edits.is_empty() {
            return Ok(());
        }
        let current = match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(
                    r#"SELECT podcastname, description, author, artworkurl, websiteurl, editedmetadata
                       FROM "Podcasts" WHERE podcastid = $1 AND userid = $2"#,
                )
                .bind(podcast_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await?
                .map(|r| {
                    let cols: Vec<Option<String>> = (0..6).map(|i| r.try_get::<Option<String>, _>(i).ok().flatten()).collect();
                    cols
                })
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query(
                    "SELECT PodcastName, Description, Author, ArtworkURL, WebsiteURL, EditedMetadata
                     FROM Podcasts WHERE PodcastID = ? AND UserID = ?",
                )
                .bind(podcast_id)
                .bind(user_id)
                .fetch_optional(pool)
                .await?
                .map(|r| (0..6).map(|i| r.try_get::<Option<String>, _>(i).ok().flatten()).collect())
            }
        };
        let Some(current) = current else { return Ok(()) };
        let edited: Vec<&str> = edits
            .iter()
            .filter(|(col, value)| {
                FIELDS
                    .iter()
                    .position(|f| f == col)
                    .is_some_and(|i| current[i].as_deref() != Some(*value))
            })
            .map(|(col, _)| *col)
            .collect();
        if edited.is_empty() {
            return Ok(());
        }
        let merged = merge_edited(current[5].as_deref(), &edited);
        self.set_edited_metadata(podcast_id, user_id, merged.as_deref()).await
    }

    /// Hand every detail of a podcast back to its feed (the next feed read restores them).
    pub async fn clear_metadata_edits(&self, podcast_id: i32, user_id: i32) -> AppResult<()> {
        self.set_edited_metadata(podcast_id, user_id, None).await
    }

    async fn set_edited_metadata(&self, podcast_id: i32, user_id: i32, value: Option<&str>) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "Podcasts" SET editedmetadata = $1 WHERE podcastid = $2 AND userid = $3"#)
                    .bind(value)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE Podcasts SET EditedMetadata = ? WHERE PodcastID = ? AND UserID = ?")
                    .bind(value)
                    .bind(podcast_id)
                    .bind(user_id)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FEED: &str = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:atom="http://www.w3.org/2005/Atom">
<channel>
  <atom:link href="https://feeds.example/show.xml" rel="self" type="application/rss+xml"/>
  <title>New Name</title>
  <link>https://show.example</link>
  <description><![CDATA[<p>About the show</p>]]></description>
  <itunes:author>The Hosts</itunes:author>
  <image><url>https://cdn.example/old-small.jpg</url><title>New Name</title><link>https://show.example</link></image>
  <itunes:image href="https://cdn.example/new.jpg"/>
  <item>
    <title>E1</title>
    <itunes:image href="https://cdn.example/e1.jpg"/>
    <enclosure url="https://cdn.example/e1.mp3" type="audio/mpeg" length="1"/>
  </item>
</channel>
</rss>"#;

    #[test]
    fn reads_channel_details() {
        let m = parse(FEED).unwrap();
        assert_eq!(m.name.as_deref(), Some("New Name"));
        assert_eq!(m.author.as_deref(), Some("The Hosts"));
        assert_eq!(m.website.as_deref(), Some("https://show.example/"));
        assert_eq!(m.artwork.as_deref(), Some("https://cdn.example/new.jpg"));
        assert_eq!(m.description.as_deref(), Some("<p>About the show</p>"));
    }

    #[test]
    fn episode_image_is_not_the_podcast_artwork() {
        let feed = FEED.replace(r#"<itunes:image href="https://cdn.example/new.jpg"/>"#, "");
        assert_eq!(parse(&feed).unwrap().artwork.as_deref(), Some("https://cdn.example/old-small.jpg"));
    }

    #[test]
    fn empty_fields_are_left_out() {
        let feed = FEED.replace("<title>New Name</title>", "<title>  </title>").replace("<itunes:author>The Hosts</itunes:author>", "");
        let m = parse(&feed).unwrap();
        assert_eq!(m.name, None);
        assert_eq!(m.author, None);
        assert!(m.fields().iter().all(|(col, _)| *col != "podcastname" && *col != "author"));
    }

    #[test]
    fn not_a_feed() {
        assert_eq!(parse("<html><body>nope</body></html>"), None);
    }

    #[test]
    fn reads_each_episodes_own_image() {
        let feed = r#"<rss xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd"><channel>
<title>Show</title><itunes:image href="https://cdn.example/show.jpg"/>
<item><title><![CDATA[Tom & Jerry]]></title><guid isPermaLink="false"> g-1 </guid>
  <enclosure url="https://cdn.example/1.mp3" type="audio/mpeg"/>
  <itunes:image href="https://img.example/abc123"/></item>
<item><title>Bits &amp; Bobs</title><enclosure url="https://cdn.example/2.mp3" type="audio/mpeg"/>
  <podcast:images xmlns:podcast="x"><itunes:image href="https://wrong.example/nested.jpg"/></podcast:images>
  <itunes:image href="https://img.example/2.jpg"/></item>
<item><title>No Image</title><guid>g-3</guid></item>
</channel></rss>"#;
        let images = item_images(feed);
        assert_eq!(images.find(Some("g-1"), None, None), Some("https://img.example/abc123"));
        assert_eq!(images.find(None, Some("https://cdn.example/1.mp3"), None), Some("https://img.example/abc123"));
        assert_eq!(images.find(None, None, Some("Tom & Jerry")), Some("https://img.example/abc123"));
        assert_eq!(images.find(None, Some("https://cdn.example/2.mp3"), None), Some("https://img.example/2.jpg"));
        assert_eq!(images.find(None, None, Some("Bits & Bobs")), Some("https://img.example/2.jpg"));
        assert_eq!(images.find(Some("g-3"), None, Some("No Image")), None);
    }

    #[test]
    fn merges_edited_fields() {
        assert_eq!(merge_edited(None, &["podcastname"]).as_deref(), Some("podcastname"));
        assert_eq!(merge_edited(Some("podcastname"), &["artworkurl", "podcastname"]).as_deref(), Some("podcastname,artworkurl"));
        assert_eq!(merge_edited(Some("bogus"), &["feedurl"]), None);
    }
}
