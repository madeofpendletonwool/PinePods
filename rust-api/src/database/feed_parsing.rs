//! RSS parsing helpers.

use super::*;

impl DatabasePool {
    // Custom function to extract raw iTunes durations before feed_rs processes them
    // This is needed because feed_rs incorrectly parses MM:SS durations as seconds only
    pub(super) fn extract_raw_itunes_durations(content: &str) -> std::collections::HashMap<String, String> {
        use regex::Regex;
        let mut raw_durations = std::collections::HashMap::new();
        
        // Professional-grade regex that handles real-world RSS feeds robustly
        // Uses DOTALL flag (?s) to match across newlines, handles CDATA, whitespace variations
        // Matches both <itunes:duration> and <duration> tags to be comprehensive
        let duration_regex = Regex::new(
            r"(?s)<item[^>]*>.*?<title(?:[^>]*)>(?:\s*<!\[CDATA\[)?\s*([^<\]]+?)(?:\]\]>)?\s*</title>.*?<(?:itunes:)?duration(?:[^>]*)>\s*(?:<!\[CDATA\[)?\s*([^<\]]+?)(?:\]\]>)?\s*</(?:itunes:)?duration>"
        ).unwrap();
        
        for caps in duration_regex.captures_iter(content) {
            if let (Some(title_match), Some(duration_match)) = (caps.get(1), caps.get(2)) {
                let title_str = title_match.as_str().trim();
                let duration_str = duration_match.as_str().trim();
                
                // Skip empty values
                if !title_str.is_empty() && !duration_str.is_empty() {
                    // Decode HTML entities in title to match feed-rs parsed titles
                    let decoded_title = title_str
                        .replace("&apos;", "'")
                        .replace("&quot;", "\"")
                        .replace("&amp;", "&")
                        .replace("&lt;", "<")
                        .replace("&gt;", ">");
                    raw_durations.insert(decoded_title, duration_str.to_string());
                }
            }
        }
        
        // Fallback: try reverse order (duration before title) for edge case XML structures
        let reverse_regex = Regex::new(
            r"(?s)<item[^>]*>.*?<(?:itunes:)?duration(?:[^>]*)>\s*(?:<!\[CDATA\[)?\s*([^<\]]+?)(?:\]\]>)?\s*</(?:itunes:)?duration>.*?<title(?:[^>]*)>(?:\s*<!\[CDATA\[)?\s*([^<\]]+?)(?:\]\]>)?\s*</title>"
        ).unwrap();
        
        for caps in reverse_regex.captures_iter(content) {
            if let (Some(duration_match), Some(title_match)) = (caps.get(1), caps.get(2)) {
                let title_str = title_match.as_str().trim();
                let duration_str = duration_match.as_str().trim();
                
                // Only add if not already found and both values are non-empty
                if !title_str.is_empty() && !duration_str.is_empty() {
                    // Decode HTML entities in title to match feed-rs parsed titles
                    let decoded_title = title_str
                        .replace("&apos;", "'")
                        .replace("&quot;", "\"")
                        .replace("&amp;", "&")
                        .replace("&lt;", "<")
                        .replace("&gt;", ">");
                    if !raw_durations.contains_key(&decoded_title) {
                        raw_durations.insert(decoded_title, duration_str.to_string());
                    }
                }
            }
        }
        
        raw_durations
    }

    // Parse RSS feed - matches Python RSS parsing logic
    pub(super) async fn parse_rss_feed(
        &self,
        content: &str,
        _podcast_id: i32,
        artwork_url: &str,
    ) -> AppResult<Vec<EpisodeData>> {
        self.parse_rss_feed_with_options(content, _podcast_id, artwork_url, false).await
    }

    pub(super) async fn parse_rss_feed_with_options(
        &self,
        content: &str,
        _podcast_id: i32,
        artwork_url: &str,
        estimate_missing_durations: bool,
    ) -> AppResult<Vec<EpisodeData>> {
        use chrono::Utc;
        use feed_rs::parser;
        use std::collections::HashMap;
        
        // Extract raw iTunes durations before feed_rs processes them
        let raw_durations = Self::extract_raw_itunes_durations(content);
        let item_images = crate::services::feed_metadata::item_images(content);
        
        // Use a custom id_generator that returns empty for guid-less entries. feed-rs's default
        // synthesizes an id (link+title hash, or a RANDOM uuid when there's no link) which would
        // make every refresh see a "new" guid and duplicate episodes. With this, entry.id is the
        // real <guid> or empty, and dedup falls back to URL/title when there's no guid.
        let feed = parser::Builder::new()
            .id_generator(|_links, _title, _uri| String::new())
            .build()
            .parse(crate::services::feed_metadata::strip_media_player(content).as_bytes())
            .map_err(|e| AppError::Internal(format!("RSS parsing error: {}", e)))?;
        
        let mut episodes = Vec::new();
        
        
        
        for entry in feed.entries {
            // EXACT Python replication: if not all(hasattr(entry, attr) for attr in ["title", "summary", "enclosures"]): continue
            if entry.title.is_none() {
                continue;
            }
            
            let mut episode = EpisodeData {
                title: String::new(),
                description: String::new(),
                url: String::new(),
                artwork_url: artwork_url.to_string(),
                pub_date: Utc::now(),
                duration: 0,
                is_video: false,
                player_url: None,
                // Real <guid> only (empty id_generator above); None when the feed has no guid.
                guid: if entry.id.trim().is_empty() {
                    None
                } else {
                    Some(entry.id.trim().to_string())
                },
            };

            // Create data map to pass to Python-style parsing functions
            let mut episode_data = HashMap::new();
            
            // Extract all data from feed_rs entry
            if let Some(title) = &entry.title {
                episode_data.insert("title".to_string(), title.content.clone());
            }
            
            if let Some(content) = &entry.content {
                if let Some(body) = &content.body {
                    episode_data.insert("content:encoded".to_string(), body.clone());
                }
            }
            
            if let Some(summary) = &entry.summary {
                episode_data.insert("summary".to_string(), summary.content.clone());
            }
            
            // Links for audio/video enclosures
            for link in &entry.links {
                if let Some(media_type) = &link.media_type {
                    if media_type.starts_with("audio/") || media_type.starts_with("video/") {
                        episode_data.insert("enclosure_url".to_string(), link.href.clone());
                        if let Some(length) = &link.length {
                            episode_data.insert("enclosure_length".to_string(), length.to_string());
                        }
                        if media_type.starts_with("video/") {
                            episode_data.insert("is_video".to_string(), "true".to_string());
                        }
                    }
                }
            }
            
            // Also check for RSS <enclosure> tags that might not be in links
            // feed_rs should put enclosures in the links, but as fallback check media
            
            // Published date - store under both keys for compatibility
            if let Some(published) = &entry.published {
                let date_str = published.to_rfc3339();
                // debug!("📅 DEBUG: feed-rs extracted published date: {}", date_str);
                episode_data.insert("published".to_string(), date_str.clone());
                episode_data.insert("pubDate".to_string(), date_str);
            } else {
                // warn!("⚠️  DEBUG: No published date found in feed-rs entry for episode: {:?}", entry.title);
            }
            
            // Media extensions
            for media in &entry.media {
                if let Some(duration) = &media.duration {
                    episode_data.insert("duration".to_string(), duration.as_secs().to_string());
                    // Don't use feed_rs processed duration for iTunes - we'll use raw values
                }
                
                // Check if we have a raw iTunes duration for this episode title
                if let Some(title) = &entry.title {
                    if let Some(raw_duration) = raw_durations.get(&title.content) {
                        episode_data.insert("itunes:duration".to_string(), raw_duration.clone());
                    }
                }
                
                for thumbnail in &media.thumbnails {
                    episode_data.insert("media_thumbnail_url".to_string(), thumbnail.image.uri.clone());
                }
                
                for content in &media.content {
                    if let Some(content_type) = &content.content_type {
                        let type_str = content_type.to_string();
                        if type_str.starts_with("audio/") || type_str.starts_with("video/") {
                            if let Some(url) = &content.url {
                                // Store as enclosure_url to match Python parsing logic
                                episode_data.insert("enclosure_url".to_string(), url.to_string());
                                if let Some(size) = &content.size {
                                    episode_data.insert("enclosure_length".to_string(), size.to_string());
                                }
                                if type_str.starts_with("video/") {
                                    episode_data.insert("is_video".to_string(), "true".to_string());
                                }
                            }
                        } else if type_str.starts_with("image/") {
                            if let Some(url) = &content.url {
                                episode_data.insert("media_image_url".to_string(), url.to_string());
                            }
                        }
                    }
                }
            }
            
            if let Some(image) = item_images.find(
                episode.guid.as_deref(),
                episode_data.get("enclosure_url").map(String::as_str),
                entry.title.as_ref().map(|t| t.content.as_str()),
            ) {
                episode_data.insert("episode_artwork_href".to_string(), image.to_string());
            }
            episode.player_url = entry.title.as_ref().and_then(|t| item_images.player(&t.content)).map(str::to_string);

            // Apply all the Python-style parsing logic with ALL fallbacks
            self.apply_python_style_parsing(&mut episode, &episode_data, artwork_url, estimate_missing_durations);
            
            if !episode.title.is_empty() {
                episodes.push(episode);
            }
        }
        
        Ok(episodes)
    }

    // Apply Python-style parsing logic with all fallbacks
    pub(super) fn apply_python_style_parsing(&self, episode: &mut EpisodeData, data: &HashMap<String, String>, default_artwork: &str, estimate_missing_durations: bool) {
        // Title - REQUIRED field with robust cleaning
        if let Some(title) = data.get("title") {
            episode.title = self.clean_and_normalize_title(title);
        }
        // Skip episodes without titles - this is critical like Python version
        if episode.title.is_empty() {
            return;
        }
        
        // EXACT Python replication: parsed_description = entry.get('content', [{}])[0].get('value') or entry.get('summary') or "No description available"
        episode.description = self.parse_description_comprehensive(data);
        
        // EXACT Python replication: parsed_audio_url = entry.enclosures[0].href if entry.enclosures else ""
        episode.url = self.parse_audio_url_comprehensive(data);

        episode.is_video = data.get("is_video").map(|v| v == "true").unwrap_or(false);

        // Artwork with comprehensive fallbacks and validation like Python
        episode.artwork_url = self.parse_artwork_comprehensive(data, default_artwork);
        
        // Publication date with extensive format support and timezone handling
        episode.pub_date = self.parse_publication_date_comprehensive(data);
        
        // Duration parsing with extensive fallbacks like Python
        episode.duration = self.parse_duration_comprehensive(data, estimate_missing_durations);
    }

    // Clean and normalize titles like Python version
    pub(super) fn clean_and_normalize_title(&self, title: &str) -> String {
        // HTML entity decoding
        let title = self.decode_html_entities(title);
        
        // HTML tag stripping
        let title = self.strip_html_tags(&title);
        
        // Unicode normalization and whitespace cleaning
        let title = title.trim()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        
        // Truncate if too long (reasonable limit) - use char_indices for Unicode safety
        if title.chars().count() > 200 {
            let mut truncated = String::new();
            for (_i, ch) in title.char_indices() {
                if truncated.chars().count() >= 197 {
                    break;
                }
                truncated.push(ch);
            }
            format!("{}...", truncated)
        } else {
            title
        }
    }

    // Comprehensive description parsing with HTML cleaning
    pub(super) fn parse_description_comprehensive(&self, data: &HashMap<String, String>) -> String {
        // EXACT Python replication: entry.get('content', [{}])[0].get('value') or entry.get('summary') or "No description available"
        data.get("content:encoded")
            .or_else(|| data.get("content"))  
            .or_else(|| data.get("summary"))
            .filter(|s| !s.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| "No description available".to_string())
    }

    // EXACT Python replication: parsed_audio_url = entry.enclosures[0].href if entry.enclosures else ""
    pub(super) fn parse_audio_url_comprehensive(&self, data: &HashMap<String, String>) -> String {
        // Python: entry.enclosures[0].href if entry.enclosures else ""
        data.get("enclosure_url")
            .filter(|url| !url.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| String::new())
    }

    // Comprehensive artwork URL parsing - prioritizes episode-specific artwork
    pub(super) fn parse_artwork_comprehensive(&self, data: &HashMap<String, String>, default_artwork: &str) -> String {
        // The episode's <itunes:image> is an image by definition; CDN image URLs often have no
        // extension, so it skips the URL guess the other candidates need.
        if let Some(href) = data.get("episode_artwork_href").map(|h| h.trim()) {
            let lower = href.to_ascii_lowercase();
            if lower.starts_with("http://") || lower.starts_with("https://") {
                return self.validate_and_clean_url(href);
            }
        }
        // Priority order: episode-specific artwork first, then fallbacks
        let artwork_candidates = [
            data.get("media_image_url"),         // <media:content type="image/jpeg" url=""/>
            data.get("media_thumbnail_url"),     // <media:thumbnail url=""/>
            data.get("itunes:image"),           // Text content of <itunes:image>
            data.get("image"),                  // Text content of <image>
            data.get("thumbnail"),              // Generic thumbnail
            data.get("logo"),                   // Logo field
        ];
        
        for candidate in artwork_candidates.iter().flatten() {
            if !candidate.trim().is_empty() && self.is_valid_image_url(candidate) {
                let cleaned_url = self.validate_and_clean_url(candidate);
                return cleaned_url;
            }
        }
        
        // Use default podcast artwork as fallback
        default_artwork.to_string()
    }

    // Comprehensive publication date parsing
    pub(super) fn parse_publication_date_comprehensive(&self, data: &HashMap<String, String>) -> DateTime<Utc> {
        // debug!("🔍 DEBUG: Date parsing - episode_data keys: {:?}", data.keys().collect::<Vec<_>>());
        
        // Multiple date field sources
        let date_candidates = [
            data.get("pubDate"),
            data.get("published"),
            data.get("dc:date"),
            data.get("updated"),
            data.get("lastBuildDate"),
            data.get("date"),
        ];
        
        for (_i, date_str) in date_candidates.iter().enumerate() {
            if let Some(date_str) = date_str {
                // debug!("📅 DEBUG: Trying date candidate {}: '{}'", i, date_str);
                if let Some(parsed_date) = self.try_parse_date(date_str) {
                    // Validate date is reasonable (not too far in future, not before 1990)
                    let now = Utc::now();
                    let year_1990 = DateTime::parse_from_rfc3339("1990-01-01T00:00:00Z").unwrap().with_timezone(&Utc);
                    let one_year_future = now + chrono::Duration::days(365);
                    
                    if parsed_date >= year_1990 && parsed_date <= one_year_future {
                        // debug!("✅ DEBUG: Successfully parsed date: {}", parsed_date);
                        return parsed_date;
                    } else {
                        // error!("❌ DEBUG: Date {} outside valid range", parsed_date);
                    }
                } else {
                    // error!("❌ DEBUG: Failed to parse date string: '{}'", date_str);
                }
            }
        }
        
        // Fallback to current time like Python version
        // warn!("⚠️  DEBUG: No valid date found, falling back to current time");
        Utc::now()
    }

    // Try to parse a date string with multiple formats
    pub(super) fn try_parse_date(&self, date_str: &str) -> Option<DateTime<Utc>> {
        let date_str = date_str.trim();
        // debug!("🔧 DEBUG: Attempting to parse date: '{}'", date_str);
        
        // RFC 2822 format (most common in RSS)
        if let Ok(parsed) = DateTime::parse_from_rfc2822(date_str) {
            // debug!("✅ DEBUG: Parsed as RFC 2822: {}", parsed);
            return Some(parsed.with_timezone(&Utc));
        } else {
            // error!("❌ DEBUG: Failed to parse as RFC 2822");
        }
        
        // RFC 3339/ISO 8601 format
        if let Ok(parsed) = DateTime::parse_from_rfc3339(date_str) {
            // debug!("✅ DEBUG: Parsed as RFC 3339: {}", parsed);
            return Some(parsed.with_timezone(&Utc));
        } else {
            // error!("❌ DEBUG: Failed to parse as RFC 3339");
        }
        
        // Common custom formats found in real feeds
        let formats = [
            "%Y-%m-%d %H:%M:%S %z",
            "%Y-%m-%dT%H:%M:%S%z",
            "%Y-%m-%d %H:%M:%S",
            "%Y-%m-%dT%H:%M:%S",
            "%a, %d %b %Y %H:%M:%S %z",
            "%a, %d %b %Y %H:%M:%S",
            "%d %b %Y %H:%M:%S %z",
            "%d %b %Y %H:%M:%S",
            "%Y-%m-%d",
            "%d/%m/%Y",
            "%m/%d/%Y",
            "%b %d, %Y",
            "%B %d, %Y",
            "%Y%m%d",
        ];
        
        // Try parsing with timezone
        for format in &formats[..8] {
            if let Ok(parsed) = DateTime::parse_from_str(date_str, format) {
                return Some(parsed.with_timezone(&Utc));
            }
        }
        
        // Try parsing as naive datetime (assume UTC)
        for format in &formats[8..] {
            if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(date_str, format) {
                return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc));
            }
        }
        
        // Try parsing date only (assume midnight UTC)
        for format in &formats[10..] {
            if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(date_str, format) {
                if let Some(naive_datetime) = naive_date.and_hms_opt(0, 0, 0) {
                    return Some(DateTime::<Utc>::from_naive_utc_and_offset(naive_datetime, Utc));
                }
            }
        }
        
        None
    }

    // Validate and clean URLs
    pub(super) fn validate_and_clean_url(&self, url: &str) -> String {
        let url = url.trim();
        
        // Basic URL validation
        if !url.starts_with("http://") && !url.starts_with("https://") {
            if url.starts_with("//") {
                return format!("https:{}", url);
            } else if url.starts_with("/") {
                // Relative URL - can't fix without base URL
                return url.to_string();
            } else {
                return format!("https://{}", url);
            }
        }
        
        url.to_string()
    }

    // Check if URL is likely a valid image
    pub(super) fn is_valid_image_url(&self, url: &str) -> bool {
        let url_lower = url.to_lowercase();
        url_lower.contains(".jpg") || 
        url_lower.contains(".jpeg") || 
        url_lower.contains(".png") || 
        url_lower.contains(".gif") || 
        url_lower.contains(".webp") || 
        url_lower.contains(".svg") ||
        url_lower.contains("image") ||
        url_lower.contains("artwork") ||
        url_lower.contains("thumbnail") ||
        url_lower.contains("cover")
    }

    pub(super) fn decode_html_entities(&self, text: &str) -> String {
        text.replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#x27;", "'")
            .replace("&#39;", "'")
            .replace("&apos;", "'")
            .replace("&nbsp;", " ")
            .replace("&#160;", " ")
            .replace("&mdash;", "—")
            .replace("&ndash;", "–")
            .replace("&hellip;", "…")
            .replace("&rsquo;", "'")
            .replace("&lsquo;", "'")
            .replace("&rdquo;", "\"")
            .replace("&ldquo;", "\"")
    }

    // // Check if URL is likely an audio file
    // fn is_audio_url(&self, url: &str) -> bool {
    //     let url_lower = url.to_lowercase();
    //     url_lower.contains(".mp3") || 
    //     url_lower.contains(".m4a") || 
    //     url_lower.contains(".wav") || 
    //     url_lower.contains(".ogg") || 
    //     url_lower.contains(".aac") || 
    //     url_lower.contains(".flac") || 
    //     url_lower.contains(".wma") || 
    //     url_lower.contains(".opus") ||
    //     url_lower.contains("audio") ||
    //     url_lower.contains("/podcast/") ||
    //     url_lower.contains("/episode/") ||
    //     url_lower.contains(".podtrac.com") ||
    //     url_lower.contains("libsyn.com") ||
    //     url_lower.contains("soundcloud.com") ||
    //     url_lower.contains("spotify.com")
    // }
    
    
    

    
    // Strip HTML tags (basic but effective)
    pub(super) fn strip_html_tags(&self, text: &str) -> String {
        let mut result = String::new();
        let mut in_tag = false;
        
        for ch in text.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => {
                    in_tag = false;
                    result.push(' '); // Replace tags with space
                }
                _ if !in_tag => result.push(ch),
                _ => {} // Skip characters inside tags
            }
        }
        
        result
    }

    // Comprehensive duration parsing matching Python logic
    pub(super) fn parse_duration_comprehensive(&self, data: &HashMap<String, String>, estimate_missing_durations: bool) -> i32 {
        // Priority order like Python version
        let duration_candidates = [
            data.get("itunes:duration"),
            data.get("duration"),
            data.get("itunes:duration_seconds"),
            data.get("length"),
            data.get("time"),
        ];
        
        for candidate in duration_candidates.iter().flatten() {
            if let Some(duration) = self.parse_duration_string(candidate) {
                if duration > 0 && duration < 86400 { // Reasonable range: 0-24 hours
                    return duration;
                }
            }
        }
        
        // Try to estimate from file size if available (like Python version)
        if let Some(length_str) = data.get("enclosure_length") {
            if let Ok(file_size) = length_str.parse::<i64>() {
                if file_size > 1_000_000 { // > 1MB
                    return self.estimate_duration_from_file_size(file_size);
                }
            }
        }
        
        // Network-based duration estimation (HTTP HEAD) is intentionally NOT done here. The parser
        // is synchronous and was previously doing tokio::task::block_in_place to block a runtime
        // worker on a nested block_on — an anti-pattern that can starve the async runtime under
        // concurrent refresh. Estimation for genuinely new episodes now happens asynchronously in
        // apply_parsed_episodes() instead.
        let _ = estimate_missing_durations;

        // Default to 0 if no duration can be determined
        0
    }

    // Parse duration string - EXACT Python logic replication
    pub(super) fn parse_duration_string(&self, duration_str: &str) -> Option<i32> {
        let duration_str = duration_str.trim();
        
        if duration_str.contains(':') {
            // Python: time_parts = list(map(int, duration_str.split(':')))
            let parts: Result<Vec<i32>, _> = duration_str.split(':')
                .map(|part| part.parse::<i32>())
                .collect();
                
            if let Ok(mut time_parts) = parts {
                // Python: while len(time_parts) < 3: time_parts.insert(0, 0)
                while time_parts.len() < 3 {
                    time_parts.insert(0, 0);
                }
                
                if time_parts.len() >= 3 {
                    let h = time_parts[0];
                    let m = time_parts[1]; 
                    let s = time_parts[2];
                    
                    // Python: parsed_duration = h * 3600 + m * 60 + s
                    let duration = h * 3600 + m * 60 + s;
                    if duration >= 0 {
                        return Some(duration);
                    }
                }
            }
        } else if duration_str.chars().all(|c| c.is_ascii_digit()) {
            // Python: elif duration_str.isdigit(): parsed_duration = int(duration_str)
            if let Ok(duration) = duration_str.parse::<i32>() {
                return Some(duration);
            }
        }
        
        None
    }

    // Estimate duration from file size like Python version
    pub(super) fn estimate_duration_from_file_size(&self, file_size_bytes: i64) -> i32 {
        // Assume 128 kbps average bitrate like Python version
        let bitrate_kbps = 128;
        let bytes_per_second = (bitrate_kbps * 1000) / 8;
        (file_size_bytes / bytes_per_second) as i32
    }

    // NEW: Estimate duration by fetching HTTP HEAD request to get Content-Length
    // This is a fallback for when RSS feeds don't include duration or file size
    pub(super) async fn estimate_duration_from_audio_url_async(&self, audio_url: &str) -> Option<i32> {
        debug!("Attempting to estimate duration from audio URL: {}", audio_url);

        // SSRF guard: audio_url is the attacker-controlled RSS <enclosure url>.
        // Reject loopback/private/link-local/reserved destinations before any
        // server-side request (incl. redirect hops).
        if let Err(reason) = crate::services::url_guard::ensure_safe_public_url_async(audio_url).await {
            debug!("Refusing to estimate duration for unsafe URL: {}", reason);
            return None;
        }

        // Shared probe client: 10s timeout, redirects re-validated so a public URL cannot 30x
        // into the internal network.
        let client = crate::services::http::probe_client();

        // Make HEAD request to get Content-Length without downloading the file
        match client.head(audio_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    if let Some(content_length) = response.headers().get("content-length") {
                        if let Ok(content_length_str) = content_length.to_str() {
                            if let Ok(file_size) = content_length_str.parse::<i64>() {
                                if file_size > 1_000_000 { // > 1MB, reasonable audio file
                                    let estimated_duration = self.estimate_duration_from_file_size(file_size);
                                    debug!("Estimated duration from file size {}: {} seconds", file_size, estimated_duration);
                                    return Some(estimated_duration);
                                }
                            }
                        }
                    }
                    debug!("No Content-Length header found in response");
                } else {
                    debug!("HEAD request failed with status: {}", response.status());
                }
            }
            Err(e) => {
                debug!("HTTP HEAD request failed for {}: {}", audio_url, e);
            }
        }
        
        None
    }

    // Helper function to parse feed content into podcast values (extracted to avoid duplication)
    pub(super) async fn parse_feed_content_to_values(&self, content: &str, feed_url: &str, user_id: i32) -> AppResult<std::collections::HashMap<String, String>> {
        use feed_rs::parser;
        
        // Parse RSS feed using feed-rs
        let feed = parser::parse(content.as_bytes())
            .map_err(|e| AppError::external_error(&format!("Failed to parse RSS feed: {}", e)))?;
        
        // Extract podcast metadata exactly as Python implementation
        let mut podcast_values = std::collections::HashMap::new();
        
        podcast_values.insert("feedurl".to_string(), feed_url.to_string());
        podcast_values.insert("userid".to_string(), user_id.to_string());
        // The same reading of the feed's details as the refresh, so the first refresh doesn't
        // "change" them.
        let meta = crate::services::feed_metadata::parse(content).unwrap_or_default();
        podcast_values.insert("podcastname".to_string(), meta.name.unwrap_or_default());
        podcast_values.insert("description".to_string(), meta.description.unwrap_or_default());
        podcast_values.insert("author".to_string(), meta.author.unwrap_or_default());
        podcast_values.insert("websiteurl".to_string(), meta.website.unwrap_or_default());
        podcast_values.insert("explicit".to_string(), "False".to_string()); // Default to False
        podcast_values.insert("episodecount".to_string(), feed.entries.len().to_string());
        podcast_values.insert("artworkurl".to_string(), meta.artwork.unwrap_or_default());
        
        // Extract categories - convert to dict format like Python.
        // Skip empty/whitespace category terms so feeds with blank category
        // elements don't produce a stray blank category (e.g. {"0": ""}).
        let category_terms: Vec<String> = feed.categories
            .iter()
            .map(|cat| cat.term.trim().to_string())
            .filter(|term| !term.is_empty())
            .collect();
        let categories = if !category_terms.is_empty() {
            let cat_dict: std::collections::HashMap<String, String> = category_terms
                .into_iter()
                .enumerate()
                .map(|(i, term)| (i.to_string(), term))
                .collect();
            serde_json::to_string(&cat_dict).unwrap_or_default()
        } else {
            "{}".to_string()
        };
        podcast_values.insert("categories".to_string(), categories);
        
        // Set default values for fields not in RSS
        podcast_values.insert("podcastindexid".to_string(), "0".to_string());
        podcast_values.insert("episodeupdatecount".to_string(), "0".to_string());
        
        debug!("Extracted podcast values: {}", podcast_values.get("podcastname").unwrap_or(&"Unknown".to_string()));
        
        Ok(podcast_values)
    }

    // Parse feed episodes using feed-rs and return JSON data (for frontend consumption)
    pub async fn parse_feed_episodes(&self, feed_url: &str, user_id: i32) -> AppResult<Vec<serde_json::Value>> {
        use feed_rs::parser;
        use reqwest::header::AUTHORIZATION;
        
        // Get stored authentication credentials for this feed
        let (username, password) = self.get_feed_auth_credentials(feed_url, user_id).await?;

        // SSRF guard: feed_url originates from user-supplied/imported feed data.
        if let Err(reason) = crate::services::url_guard::ensure_safe_public_url_async(feed_url).await {
            warn!("Refusing to fetch feed episodes from unsafe URL {}: {}", feed_url, reason);
            return Err(AppError::bad_request(format!("Refused to fetch feed URL: {}", reason)));
        }

        // Build HTTP client with proper configuration
        let client = reqwest::Client::builder()
            .user_agent("PinePods/1.0")
            .timeout(std::time::Duration::from_secs(30))
            .redirect(crate::services::url_guard::guarded_redirect_policy())
            .build()
            .map_err(|e| AppError::external_error(&format!("Failed to build HTTP client: {}", e)))?;
        
        let mut request = client.get(feed_url);
        
        // Add authentication if available
        if let (Some(user), Some(pass)) = (username.as_deref(), password.as_deref()) {
            debug!("Using stored authentication for feed: {}", feed_url);
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", user, pass));
            request = request.header(AUTHORIZATION, format!("Basic {}", encoded));
        }
        
        // Fetch RSS feed
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(AppError::external_error(&format!("Failed to fetch RSS feed: {}", response.status())));
        }
        
        let content = response.text().await?;

        // Parse RSS feed using feed-rs
        let feed = parser::parse(crate::services::feed_metadata::strip_media_player(&content).as_bytes())
            .map_err(|e| AppError::external_error(&format!("Failed to parse RSS feed: {}", e)))?;

        // Extract podcast-level artwork as fallback
        let podcast_artwork = feed.logo.as_ref().map(|l| l.uri.clone())
            .or_else(|| feed.icon.as_ref().map(|i| i.uri.clone()))
            .unwrap_or_default();
        
        let mut episodes = Vec::new();
        
        for entry in feed.entries.iter() {
            // Skip episodes without titles
            if entry.title.is_none() {
                continue;
            }
            
            // Extract episode data using same logic as add_episodes
            let mut episode_data = std::collections::HashMap::new();
            
            if let Some(title) = &entry.title {
                episode_data.insert("title".to_string(), title.content.clone());
            }
            
            if let Some(summary) = &entry.summary {
                episode_data.insert("summary".to_string(), summary.content.clone());
            }
            
            if let Some(content) = &entry.content {
                if let Some(body) = &content.body {
                    episode_data.insert("content:encoded".to_string(), body.clone());
                }
            }
            
            // Extract audio/video URL from media or links
            let mut audio_url = String::new();
            let mut enclosure_length = String::new();
            let mut is_video_episode = false;

            // Check media objects first
            for media in &entry.media {
                for content in &media.content {
                    if let Some(url) = &content.url {
                        if let Some(media_type) = &content.content_type {
                            let type_str = media_type.to_string();
                            if type_str.starts_with("audio/") || type_str.starts_with("video/") {
                                audio_url = url.to_string();
                                if let Some(size) = content.size {
                                    enclosure_length = size.to_string();
                                }
                                if type_str.starts_with("video/") {
                                    is_video_episode = true;
                                }
                                break;
                            }
                        }
                    }
                }
                if !audio_url.is_empty() { break; }
            }

            // Fallback to links if no media found
            if audio_url.is_empty() {
                for link in &entry.links {
                    if let Some(media_type) = &link.media_type {
                        if media_type.starts_with("audio/") || media_type.starts_with("video/") {
                            audio_url = link.href.clone();
                            if let Some(length) = &link.length {
                                enclosure_length = length.to_string();
                            }
                            if media_type.starts_with("video/") {
                                is_video_episode = true;
                            }
                            break;
                        }
                    }
                }
            }
            
            // Extract episode artwork (with fallback to podcast artwork)
            let mut episode_artwork = podcast_artwork.clone();
            for media in &entry.media {
                if !media.thumbnails.is_empty() {
                    episode_artwork = media.thumbnails[0].image.uri.clone();
                    break;
                }
            }
            
            // Extract duration from media or iTunes tags
            let mut duration_seconds = 0;
            
            // First try media objects (works for some feeds)
            for media in &entry.media {
                if let Some(duration) = &media.duration {
                    duration_seconds = duration.as_secs() as i32;
                    break;
                }
            }
            
            // If media duration is suspicious (< 3600 seconds = 1 hour), 
            // try to extract iTunes duration from raw RSS to work around feed-rs parsing bugs
            if duration_seconds > 0 && duration_seconds < 3600 {
                if let Some(title) = &entry.title {
                    if let Some(corrected_duration) = Self::extract_itunes_duration_from_raw(&content, &title.content) {
                        if corrected_duration != duration_seconds as u64 {
                            info!("🔧 DURATION CORRECTED: '{}' feed-rs={} -> iTunes={}", 
                                title.content, duration_seconds, corrected_duration);
                            duration_seconds = corrected_duration as i32;
                        }
                    }
                }
            }
            
            // Extract publication date
            let pub_date = if let Some(published) = &entry.published {
                published.to_rfc3339()
            } else {
                chrono::Utc::now().to_rfc3339()
            };
            
            // Create episode JSON object matching frontend Episode structure
            let episode_json = serde_json::json!({
                "title": entry.title.as_ref().map(|t| t.content.clone()),
                "description": entry.summary.as_ref().map(|s| s.content.clone()),
                "pub_date": pub_date,
                "enclosure_url": if audio_url.is_empty() { None } else { Some(audio_url) },
                "enclosure_length": if enclosure_length.is_empty() { None } else { Some(enclosure_length) },
                "artwork": if episode_artwork.is_empty() { None } else { Some(episode_artwork) },
                "content": entry.content.as_ref().and_then(|c| c.body.clone()),
                "duration": duration_seconds,
                "guid": entry.id.clone(),
                "is_video": is_video_episode
            });
            
            episodes.push(episode_json);
        }
        
        Ok(episodes)
    }

    // Extract iTunes duration from raw RSS for a specific episode title (workaround for feed-rs bugs)
    pub(super) fn extract_itunes_duration_from_raw(raw_rss: &str, episode_title: &str) -> Option<u64> {
        // HTML encode the episode title to match raw RSS format
        // IMPORTANT: encode & first, otherwise it will double-encode other entities
        let html_encoded_title = episode_title
            .replace("&", "&amp;")
            .replace("'", "&apos;")
            .replace("\"", "&quot;")
            .replace("<", "&lt;")
            .replace(">", "&gt;");
        
        // Try different title formats - RSS titles can vary from feed-rs parsed titles
        let search_patterns = vec![
            format!("<title><![CDATA[{}]]></title>", episode_title),
            format!("<title>{}</title>", episode_title),
            format!("<title><![CDATA[{}]]></title>", html_encoded_title),
            format!("<title>{}</title>", html_encoded_title),
            // Also try searching for the core title without extra formatting
            episode_title.split(" (").next().unwrap_or(episode_title).to_string(),
            html_encoded_title.split(" (").next().unwrap_or(&html_encoded_title).to_string(),
        ];
        
        let mut item_pos = None;
        let mut item_end_pos = None;
        
        // Find any item that contains this title or a partial match
        for pattern in &search_patterns {
            if let Some(pos) = raw_rss.find(pattern) {
                // Find the start of the <item> block containing this title
                let item_start = raw_rss[..pos].rfind("<item>").unwrap_or(0);
                if let Some(end) = raw_rss[pos..].find("</item>") {
                    item_pos = Some(item_start);
                    item_end_pos = Some(pos + end + "</item>".len());
                    break;
                }
            }
        }
        
        // If exact matches failed, try a broader search by looking through all items
        if item_pos.is_none() {
            let mut start = 0;
            while let Some(item_start) = raw_rss[start..].find("<item>") {
                let absolute_start = start + item_start;
                if let Some(item_end) = raw_rss[absolute_start..].find("</item>") {
                    let absolute_end = absolute_start + item_end + "</item>".len();
                    let item_block = &raw_rss[absolute_start..absolute_end];
                    
                    // Check if this item contains any part of our episode title
                    let title_core = episode_title.split(" (").next().unwrap_or(episode_title);
                    if item_block.contains(title_core) {
                        item_pos = Some(absolute_start);
                        item_end_pos = Some(absolute_end);
                        break;
                    }
                    
                    start = absolute_end;
                } else {
                    break;
                }
            }
        }
        
        // Extract duration from the found item block
        if let (Some(start), Some(end)) = (item_pos, item_end_pos) {
            let item_block = &raw_rss[start..end];
            
            // Look for <itunes:duration> in this item block
            if let Some(duration_start) = item_block.find("<itunes:duration>") {
                let duration_content_start = duration_start + "<itunes:duration>".len();
                if let Some(duration_end) = item_block[duration_content_start..].find("</itunes:duration>") {
                    let duration_str = &item_block[duration_content_start..duration_content_start + duration_end];
                    return Self::parse_itunes_duration(duration_str.trim());
                }
            }
        }
        
        None
    }

    // Parse iTunes duration string (HH:MM:SS, MM:SS, or seconds) to total seconds
    pub(super) fn parse_itunes_duration(duration_str: &str) -> Option<u64> {
        if duration_str.is_empty() {
            return None;
        }
        
        // If it's just a number, treat as seconds
        if let Ok(seconds) = duration_str.parse::<u64>() {
            return Some(seconds);
        }
        
        // Split by colons for time format
        let parts: Vec<&str> = duration_str.split(':').collect();
        
        match parts.len() {
            1 => {
                // Just seconds
                parts[0].parse::<u64>().ok()
            },
            2 => {
                // MM:SS
                let minutes = parts[0].parse::<u64>().ok()?;
                let seconds = parts[1].parse::<u64>().ok()?;
                Some(minutes * 60 + seconds)
            },
            3 => {
                // HH:MM:SS
                let hours = parts[0].parse::<u64>().ok()?;
                let minutes = parts[1].parse::<u64>().ok()?;
                let seconds = parts[2].parse::<u64>().ok()?;
                Some(hours * 3600 + minutes * 60 + seconds)
            },
            _ => None
        }
    }
}
