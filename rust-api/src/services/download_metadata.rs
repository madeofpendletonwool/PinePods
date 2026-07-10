//! Shared helpers for embedding metadata into downloaded episode files and for
//! writing optional sidecar artifacts (cover images, metadata files) next to them.
//!
//! Two download paths use these helpers: the server-side background download
//! (`services::tasks`) and the on-demand client download (`handlers::episodes`).
//! Keeping the tagging logic here means enrichment (#533 feed URL, description,
//! GUID) lives in one place. Sidecar writing (#451/#658) only applies to the
//! server-side path, which owns a real per-podcast folder on disk.

use std::path::Path;

/// Everything needed to tag a downloaded episode mp3 (ID3v2.4) and to build sidecars.
#[derive(Clone, Debug)]
pub struct EpisodeMetadata {
    pub title: String,
    /// Podcast author; caller resolves the "Unknown" fallback.
    pub artist: String,
    /// Podcast name.
    pub album: String,
    pub date: Option<chrono::NaiveDateTime>,
    pub description: Option<String>,
    pub feed_url: Option<String>,
    pub episode_url: Option<String>,
    pub guid: Option<String>,
    /// Episode duration in seconds.
    pub duration: Option<i32>,
    pub episode_artwork: Option<String>,
    pub podcast_artwork: Option<String>,
}

impl EpisodeMetadata {
    /// Cover URL to embed in the mp3: episode art first, podcast art as fallback.
    fn embed_cover_url(&self) -> Option<&str> {
        self.episode_artwork
            .as_deref()
            .or(self.podcast_artwork.as_deref())
    }

    /// Cover URL for per-episode sidecars: same precedence as the embedded cover.
    fn episode_cover_url(&self) -> Option<&str> {
        self.embed_cover_url()
    }
}

/// Admin-controlled options for the extra files written to the download tree.
/// Mirrors the `AppSettings` columns added in migration 055.
#[derive(Clone, Debug)]
pub struct DownloadSettings {
    /// Save the podcast cover as `folder.jpg` at the podcast-folder root (#658).
    pub folder_cover: bool,
    /// Save the episode cover art as a sidecar image (#451).
    pub episode_cover: bool,
    /// Save an episode metadata sidecar (#451).
    pub metadata_sidecar: bool,
    /// One of `json` | `xml` | `ffmetadata` | `both`.
    pub metadata_format: String,
    /// Write episode sidecars into a `metadata/` subfolder rather than alongside the mp3.
    pub metadata_subfolder: bool,
}

impl DownloadSettings {
    /// Defaults matching the migration: all file-writing off, format `both`, subfolder on.
    pub fn disabled() -> Self {
        Self {
            folder_cover: false,
            episode_cover: false,
            metadata_sidecar: false,
            metadata_format: "both".to_string(),
            metadata_subfolder: true,
        }
    }

    /// True if any file-writing option is enabled (lets callers skip work entirely).
    pub fn any_enabled(&self) -> bool {
        self.folder_cover || self.episode_cover || self.metadata_sidecar
    }
}

/// Embed ID3v2.4 tags into a downloaded mp3: title/artist/album/date/genre/cover
/// (as before) plus the feed URL (#533), episode description, episode URL and GUID.
pub async fn add_podcast_metadata(
    file_path: &Path,
    meta: &EpisodeMetadata,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use chrono::Datelike; // for year()/month()/day()
    use id3::TagLike;

    let mut tag = id3::Tag::new();
    tag.set_title(&meta.title);
    tag.set_artist(&meta.artist);
    tag.set_album(&meta.album);

    if let Some(date) = meta.date {
        tag.set_date_recorded(id3::Timestamp {
            year: date.year(),
            month: Some(date.month() as u8),
            day: Some(date.day() as u8),
            hour: None,
            minute: None,
            second: None,
        });
    }

    tag.set_genre("Podcast");

    // Episode description as a comment frame (COMM).
    if let Some(desc) = meta.description.as_deref() {
        if !desc.is_empty() {
            tag.add_frame(id3::frame::Comment {
                lang: "eng".to_string(),
                description: String::new(),
                text: desc.to_string(),
            });
        }
    }

    // Feed URL (#533): a custom TXXX frame plus the official-source URL link frame
    // (WOAS) that most tag tools understand.
    if let Some(feed_url) = meta.feed_url.as_deref() {
        if !feed_url.is_empty() {
            tag.add_frame(id3::frame::ExtendedText {
                description: "FEED_URL".to_string(),
                value: feed_url.to_string(),
            });
            tag.add_frame(id3::Frame::with_content(
                "WOAS",
                id3::Content::Link(feed_url.to_string()),
            ));
        }
    }

    // Episode URL + GUID for round-tripping back to the source item.
    if let Some(url) = meta.episode_url.as_deref() {
        if !url.is_empty() {
            tag.add_frame(id3::frame::ExtendedText {
                description: "EPISODE_URL".to_string(),
                value: url.to_string(),
            });
        }
    }
    if let Some(guid) = meta.guid.as_deref() {
        if !guid.is_empty() {
            tag.add_frame(id3::frame::ExtendedText {
                description: "EPISODE_GUID".to_string(),
                value: guid.to_string(),
            });
        }
    }

    // Cover art (episode first, podcast fallback).
    if let Some(cover_url) = meta.embed_cover_url() {
        if let Ok(artwork_data) = download_artwork(cover_url).await {
            let mime_type = sniff_image_mime(&artwork_data);
            tag.add_frame(id3::frame::Picture {
                mime_type: mime_type.to_string(),
                picture_type: id3::frame::PictureType::CoverFront,
                description: "Cover".to_string(),
                data: artwork_data,
            });
        }
    }

    tag.write_to_path(file_path, id3::Version::Id3v24)?;
    Ok(())
}

/// Fetch artwork bytes (5MB cap). Shared by tagging and sidecar writers.
pub async fn download_artwork(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header("User-Agent", "PinePods/1.0")
        .send()
        .await?;

    if response.status().is_success() {
        let bytes = response.bytes().await?;
        if bytes.len() > 5 * 1024 * 1024 {
            return Err("Artwork too large".into());
        }
        Ok(bytes.to_vec())
    } else {
        Err(format!("Failed to download artwork: HTTP {}", response.status()).into())
    }
}

fn sniff_image_mime(data: &[u8]) -> &'static str {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        "image/jpeg"
    } else if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else {
        "image/jpeg"
    }
}

fn image_extension(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "png"
    } else {
        "jpg"
    }
}

/// Write the optional sidecar artifacts for a freshly-downloaded episode. Never
/// returns an error: individual failures are logged and skipped so a sidecar
/// problem can't fail the download (matching the tolerant tagging call site).
///
/// * `download_dir` — the per-podcast folder (where `folder.jpg` lives).
/// * `audio_path` — the written mp3; its file stem names the episode sidecars.
pub async fn write_sidecars(
    download_dir: &Path,
    audio_path: &Path,
    meta: &EpisodeMetadata,
    settings: &DownloadSettings,
) {
    // Podcast cover as folder.jpg at the podcast-folder root (independent of the
    // metadata-subfolder setting). Written once — skipped if already present so we
    // don't re-download it on every episode of the same podcast.
    if settings.folder_cover {
        if let Some(url) = meta.podcast_artwork.as_deref() {
            if let Err(e) = write_folder_cover(download_dir, url).await {
                tracing::warn!(
                    "Failed to write folder cover in {}: {}",
                    download_dir.display(),
                    e
                );
            }
        }
    }

    if !settings.episode_cover && !settings.metadata_sidecar {
        return;
    }

    let stem = audio_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("episode")
        .to_string();

    let sidecar_dir = if settings.metadata_subfolder {
        download_dir.join("metadata")
    } else {
        download_dir.to_path_buf()
    };

    if settings.metadata_subfolder {
        if let Err(e) = std::fs::create_dir_all(&sidecar_dir) {
            tracing::warn!(
                "Failed to create metadata dir {}: {}",
                sidecar_dir.display(),
                e
            );
            return;
        }
    }

    if settings.episode_cover {
        if let Some(url) = meta.episode_cover_url() {
            if let Err(e) = write_episode_cover(&sidecar_dir, &stem, url).await {
                tracing::warn!("Failed to write episode cover sidecar: {}", e);
            }
        }
    }

    if settings.metadata_sidecar {
        if let Err(e) = write_metadata_sidecar(&sidecar_dir, &stem, meta, &settings.metadata_format) {
            tracing::warn!("Failed to write metadata sidecar: {}", e);
        }
    }
}

async fn write_folder_cover(
    download_dir: &Path,
    url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for name in ["folder.jpg", "folder.png"] {
        if download_dir.join(name).exists() {
            return Ok(());
        }
    }
    let data = download_artwork(url).await?;
    let ext = image_extension(&data);
    std::fs::write(download_dir.join(format!("folder.{}", ext)), &data)?;
    Ok(())
}

async fn write_episode_cover(
    dir: &Path,
    stem: &str,
    url: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let data = download_artwork(url).await?;
    let ext = image_extension(&data);
    std::fs::write(dir.join(format!("{}.{}", stem, ext)), &data)?;
    Ok(())
}

fn write_metadata_sidecar(
    dir: &Path,
    stem: &str,
    meta: &EpisodeMetadata,
    format: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match format {
        "json" => write_json_sidecar(dir, stem, meta)?,
        "xml" => write_xml_sidecar(dir, stem, meta)?,
        "ffmetadata" => write_ffmetadata_sidecar(dir, stem, meta)?,
        // "both" (the default) and any unexpected value fall through to JSON + XML.
        _ => {
            write_json_sidecar(dir, stem, meta)?;
            write_xml_sidecar(dir, stem, meta)?;
        }
    }
    Ok(())
}

fn iso_date(meta: &EpisodeMetadata) -> Option<String> {
    meta.date.map(|d| d.format("%Y-%m-%dT%H:%M:%S").to_string())
}

fn write_json_sidecar(
    dir: &Path,
    stem: &str,
    meta: &EpisodeMetadata,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let value = serde_json::json!({
        "title": meta.title,
        "album": meta.album,
        "artist": meta.artist,
        "description": meta.description,
        "date": iso_date(meta),
        "feed_url": meta.feed_url,
        "episode_url": meta.episode_url,
        "guid": meta.guid,
        "duration": meta.duration,
        "image": meta.episode_artwork.clone().or_else(|| meta.podcast_artwork.clone()),
    });
    let text = serde_json::to_string_pretty(&value)?;
    std::fs::write(dir.join(format!("{}.json", stem)), text)?;
    Ok(())
}

fn write_xml_sidecar(
    dir: &Path,
    stem: &str,
    meta: &EpisodeMetadata,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // A reconstructed RSS <item> node (synthesized from stored fields, not the
    // verbatim feed node — raw item XML isn't retained). The itunes namespace is
    // declared inline so the fragment is well-formed on its own.
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(
        "<item xmlns:itunes=\"http://www.itunes.com/dtds/podcast-1.0.dtd\">\n",
    );
    xml.push_str(&format!("  <title>{}</title>\n", xml_escape(&meta.title)));
    if let Some(desc) = &meta.description {
        xml.push_str(&format!("  <description>{}</description>\n", xml_escape(desc)));
    }
    if let Some(url) = &meta.episode_url {
        xml.push_str(&format!(
            "  <enclosure url=\"{}\" type=\"audio/mpeg\"/>\n",
            xml_escape(url)
        ));
    }
    if let Some(guid) = &meta.guid {
        xml.push_str(&format!("  <guid>{}</guid>\n", xml_escape(guid)));
    }
    if let Some(date) = meta.date {
        xml.push_str(&format!(
            "  <pubDate>{}</pubDate>\n",
            date.format("%a, %d %b %Y %H:%M:%S +0000")
        ));
    }
    if let Some(dur) = meta.duration {
        xml.push_str(&format!("  <itunes:duration>{}</itunes:duration>\n", dur));
    }
    if let Some(img) = meta.episode_artwork.as_deref().or(meta.podcast_artwork.as_deref()) {
        xml.push_str(&format!("  <itunes:image href=\"{}\"/>\n", xml_escape(img)));
    }
    xml.push_str("</item>\n");
    std::fs::write(dir.join(format!("{}.xml", stem)), xml)?;
    Ok(())
}

fn write_ffmetadata_sidecar(
    dir: &Path,
    stem: &str,
    meta: &EpisodeMetadata,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut s = String::from(";FFMETADATA1\n");
    s.push_str(&format!("title={}\n", ff_escape(&meta.title)));
    s.push_str(&format!("album={}\n", ff_escape(&meta.album)));
    s.push_str(&format!("artist={}\n", ff_escape(&meta.artist)));
    s.push_str("genre=Podcast\n");
    if let Some(date) = meta.date {
        s.push_str(&format!("date={}\n", date.format("%Y-%m-%d")));
    }
    if let Some(desc) = &meta.description {
        s.push_str(&format!("description={}\n", ff_escape(desc)));
    }
    if let Some(url) = &meta.feed_url {
        s.push_str(&format!("PODCASTFEEDURL={}\n", ff_escape(url)));
    }
    if let Some(url) = &meta.episode_url {
        s.push_str(&format!("EPISODEURL={}\n", ff_escape(url)));
    }
    if let Some(guid) = &meta.guid {
        s.push_str(&format!("EPISODEGUID={}\n", ff_escape(guid)));
    }
    std::fs::write(dir.join(format!("{}.txt", stem)), s)?;
    Ok(())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Escape FFMETADATA special characters (`= ; # \` and newlines) with a backslash.
fn ff_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '=' | ';' | '#' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            '\n' => {
                out.push('\\');
                out.push('\n');
            }
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> EpisodeMetadata {
        EpisodeMetadata {
            title: "A & B <tag> \"quote\"".to_string(),
            artist: "Host; Name".to_string(),
            album: "My Podcast #1".to_string(),
            date: chrono::NaiveDate::from_ymd_opt(2026, 7, 4)
                .and_then(|d| d.and_hms_opt(8, 30, 0)),
            description: "Line one & <b>bold</b>".to_string().into(),
            feed_url: Some("https://example.com/feed?a=1&b=2".to_string()),
            episode_url: Some("https://example.com/ep1.mp3".to_string()),
            guid: Some("guid-123".to_string()),
            duration: Some(3600),
            episode_artwork: Some("https://example.com/ep.jpg".to_string()),
            podcast_artwork: Some("https://example.com/pod.jpg".to_string()),
        }
    }

    #[test]
    fn json_sidecar_is_valid_and_has_fields() {
        let dir = std::env::temp_dir().join(format!("pp_json_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_json_sidecar(&dir, "ep", &sample()).unwrap();
        let text = std::fs::read_to_string(dir.join("ep.json")).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["album"], "My Podcast #1");
        assert_eq!(value["feed_url"], "https://example.com/feed?a=1&b=2");
        assert_eq!(value["duration"], 3600);
        assert_eq!(value["date"], "2026-07-04T08:30:00");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn xml_sidecar_escapes_special_chars() {
        let dir = std::env::temp_dir().join(format!("pp_xml_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_xml_sidecar(&dir, "ep", &sample()).unwrap();
        let xml = std::fs::read_to_string(dir.join("ep.xml")).unwrap();
        // The raw '<tag>' from the title must not appear unescaped inside the title.
        assert!(xml.contains("<title>A &amp; B &lt;tag&gt; &quot;quote&quot;</title>"));
        assert!(xml.contains("xmlns:itunes="));
        assert!(xml.contains("<itunes:duration>3600</itunes:duration>"));
        // No stray unescaped ampersand from the feed/query string.
        assert!(!xml.contains("a=1&b=2"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ffmetadata_escapes_reserved_chars() {
        let dir = std::env::temp_dir().join(format!("pp_ff_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        write_ffmetadata_sidecar(&dir, "ep", &sample()).unwrap();
        let text = std::fs::read_to_string(dir.join("ep.txt")).unwrap();
        assert!(text.starts_with(";FFMETADATA1\n"));
        assert!(text.contains("album=My Podcast \\#1"));
        assert!(text.contains("artist=Host\\; Name"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn image_helpers_sniff_magic_bytes() {
        assert_eq!(image_extension(&[0x89, 0x50, 0x4E, 0x47]), "png");
        assert_eq!(image_extension(&[0xFF, 0xD8, 0xFF]), "jpg");
        assert_eq!(sniff_image_mime(&[0x89, 0x50, 0x4E, 0x47]), "image/png");
        assert_eq!(sniff_image_mime(&[0xFF, 0xD8, 0xFF]), "image/jpeg");
    }
}
