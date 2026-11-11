//! Feed fetching and episode ingest during refresh.

use super::*;

pub(super) const PG_PODCASTS: &str = r#""Podcasts""#;

// A refresh-state update writes the feed of the given podcast rows and, until those columns are
// dropped from Podcasts, the rows themselves. Postgres binds the ids as one array at `$n`.
pub(super) fn pg_refresh_targets(n: usize) -> [(&'static str, String); 2] {
    [
        (PG_PODCASTS, format!("podcastid = ANY(${n})")),
        (r#""Feeds""#, format!(r#"feedid IN (SELECT feedid FROM "Podcasts" WHERE podcastid = ANY(${n}))"#)),
    ]
}

pub(super) fn mysql_refresh_targets(ids: usize) -> [(&'static str, String); 2] {
    let marks = vec!["?"; ids].join(", ");
    [
        ("Podcasts", format!("PodcastID IN ({marks})")),
        ("Feeds", format!("FeedID IN (SELECT FeedID FROM Podcasts WHERE PodcastID IN ({marks}))")),
    ]
}

#[derive(Debug, Clone)]
pub struct EpisodeData {
    pub title: String,
    pub description: String,
    pub url: String,
    pub artwork_url: String,
    pub pub_date: DateTime<Utc>,
    pub duration: i32,
    pub is_video: bool,
    /// Stable RSS <guid> for this entry, when the feed supplies one. None when the feed has no
    /// guid (we deliberately do NOT use feed-rs's synthesized id, which can be a random UUID and
    /// would break dedup). Dedup falls back to URL/title when this is None.
    pub guid: Option<String>,
    /// The item's `<media:player>` URL, which 0.9.1 stored as the episode URL. Only used to find
    /// those rows.
    pub player_url: Option<String>,
}

/// A parsed episode about to be inserted, with the artwork and duration it will be stored with.
pub(super) struct NewEpisodeRow<'a> {
    ep: &'a EpisodeData,
    artwork: &'a str,
    duration: i32,
}

/// Rows per `INSERT ... UNNEST` statement when importing a back catalogue (Postgres).
pub(super) const EPISODE_INSERT_CHUNK: usize = 1000;

/// Episodes without a duration that get a HEAD probe during the refresh itself (newest first).
pub(super) const DURATION_PROBES_PER_APPLY: usize = 10;

pub(super) const DURATION_PROBE_CONCURRENCY: usize = 5;

/// HEAD probes the background duration backfill runs at once, server-wide.
pub(super) const DURATION_BACKFILL_CONCURRENCY: usize = 2;

/// What a feed group last stored, sent back on the next fetch (see `fetch_feed_conditional`).
#[derive(Clone, Copy, Default)]
pub struct FeedValidators<'a> {
    pub etag: Option<&'a str>,
    pub last_modified: Option<&'a str>,
    /// SHA-256 hex of the last body that was applied.
    pub content_hash: Option<&'a str>,
}

/// Result of a conditional feed fetch (see `fetch_feed_conditional`).
pub enum FeedFetch {
    /// 304, or a 200 whose body hash matches the stored one: skip parsing entirely. A 200 still
    /// carries fresh validators to persist.
    NotModified {
        etag: Option<String>,
        last_modified: Option<String>,
        /// The response's `Cache-Control: max-age`.
        max_age: Option<chrono::Duration>,
    },
    /// Feed body plus any fresh cache validators to persist for next time.
    Fetched {
        body: String,
        etag: Option<String>,
        last_modified: Option<String>,
        content_hash: String,
        max_age: Option<chrono::Duration>,
    },
    /// 429/503: the host asked us to back off, for `retry_after` if it said how long.
    Throttled {
        status: u16,
        retry_after: Option<std::time::Duration>,
    },
}

/// Comparable snapshot of an already-stored episode, used for no-op change detection during apply.
pub(super) struct ExistingEp {
    guid: Option<String>,
    title: String,
    description: String,
    url: String,
    artwork: String,
    duration: i32,
    is_video: bool,
}

/// Lookup maps for existing episodes of a podcast, keyed by GUID / URL-base / title, plus the
/// per-id comparable values.
#[derive(Default)]
pub(super) struct ExistingEpisodes {
    guid_to_id: std::collections::HashMap<String, i32>,
    url_to_id: std::collections::HashMap<String, i32>,
    title_to_id: std::collections::HashMap<String, i32>,
    by_id: std::collections::HashMap<i32, ExistingEp>,
}

dialect_fn!([pg_existing_from_row, lite_existing_from_row]
    pub(super) fn(row: &q::Row) -> ExistingEp {
    ExistingEp {
        guid: row.try_get::<Option<String>, _>("episodeguid").ok().flatten(),
        title: row.try_get::<Option<String>, _>("episodetitle").ok().flatten().unwrap_or_default(),
        description: row.try_get::<Option<String>, _>("episodedescription").ok().flatten().unwrap_or_default(),
        url: row.try_get::<Option<String>, _>("episodeurl").ok().flatten().unwrap_or_default(),
        artwork: row.try_get::<Option<String>, _>("episodeartwork").ok().flatten().unwrap_or_default(),
        duration: row.try_get::<Option<i32>, _>("episodeduration").ok().flatten().unwrap_or(0),
        is_video: row.try_get::<Option<bool>, _>("is_video").ok().flatten().unwrap_or(false),
    }
});

pub(super) fn mysql_existing_from_row(row: &sqlx::mysql::MySqlRow) -> ExistingEp {
    ExistingEp {
        guid: row.try_get::<Option<String>, _>("EpisodeGUID").ok().flatten(),
        title: row.try_get::<Option<String>, _>("EpisodeTitle").ok().flatten().unwrap_or_default(),
        description: row.try_get::<Option<String>, _>("EpisodeDescription").ok().flatten().unwrap_or_default(),
        url: row.try_get::<Option<String>, _>("EpisodeURL").ok().flatten().unwrap_or_default(),
        artwork: row.try_get::<Option<String>, _>("EpisodeArtwork").ok().flatten().unwrap_or_default(),
        duration: row.try_get::<Option<i32>, _>("EpisodeDuration").ok().flatten().unwrap_or(0),
        is_video: row.try_get::<Option<bool>, _>("IsVideo").ok().flatten().unwrap_or(false),
    }
}

impl DatabasePool {
    // Get user podcasts for refresh - matches Python refresh logic
    pub async fn get_user_podcasts_for_refresh(&self, user_id: i32) -> AppResult<Vec<crate::handlers::refresh::PodcastForRefresh>> {
        dialect_match!(self, {
            pg(pool) => {
                let rows = q::query(
                    r#"SELECT 
                        podcastid as id,
                        feedid as feed_id,
                        podcastname as name,
                        feedurl as feed_url,
                        artworkurl as artwork_url,
                        isyoutubechannel as is_youtube,
                        autodownload as auto_download,
                        autoqueue as auto_queue,
                        username as username,
                        password as password,
                        feedcutoffdays as feed_cutoff_days,
                        userid as user_id,
                        feedetag as etag,
                        feedlastmodified as last_modified,
                        feedcontenthash as content_hash,
                        refreshintervalminutes as refresh_interval,
                        (lastrefreshsuccess IS NOT NULL AND lastrefreshsuccess > NOW() - INTERVAL '2 minutes') as recently_refreshed,
                        (refreshnotbefore IS NOT NULL AND refreshnotbefore > NOW()) as throttled,
                        COALESCE(refreshpodcast, TRUE) as auto_refresh
                    FROM "Podcasts" 
                    WHERE userid = $1"#
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut podcasts = Vec::new();
                for row in rows {
                    podcasts.push(crate::handlers::refresh::PodcastForRefresh {
                        id: row.try_get("id")?,
                        feed_id: row.try_get("feed_id")?,
                        name: row.try_get("name")?,
                        feed_url: row.try_get("feed_url")?,
                        artwork_url: row.try_get("artwork_url").unwrap_or_default(),
                        is_youtube: row.try_get("is_youtube")?,
                        auto_download: row.try_get("auto_download")?,
                        auto_queue: row.try_get("auto_queue").unwrap_or(false),
                        username: row.try_get("username").ok(),
                        password: row.try_get("password").ok(),
                        feed_cutoff_days: row.try_get("feed_cutoff_days").ok(),
                        user_id: row.try_get("user_id")?,
                        etag: row.try_get("etag").ok().flatten(),
                        last_modified: row.try_get("last_modified").ok().flatten(),
                        content_hash: row.try_get("content_hash").ok().flatten(),
                        refresh_interval: row.try_get("refresh_interval").ok().flatten(),
                        recently_refreshed: row.try_get("recently_refreshed").unwrap_or(false),
                        throttled: row.try_get("throttled").unwrap_or(false),
                        auto_refresh: row.try_get("auto_refresh").unwrap_or(true),
                    });
                }
                Ok(podcasts)
            },
            mysql(pool) => {
                let rows = sqlx::query(
                    "SELECT 
                        PodcastID as id,
                        FeedID as feed_id,
                        PodcastName as name,
                        FeedURL as feed_url,
                        ArtworkURL as artwork_url,
                        IsYouTubeChannel as is_youtube,
                        AutoDownload as auto_download,
                        AutoQueue as auto_queue,
                        Username as username,
                        Password as password,
                        FeedCutoffDays as feed_cutoff_days,
                        UserID as user_id,
                        FeedETag as etag,
                        FeedLastModified as last_modified,
                        FeedContentHash as content_hash,
                        RefreshIntervalMinutes as refresh_interval,
                        (LastRefreshSuccess IS NOT NULL AND LastRefreshSuccess > NOW() - INTERVAL 2 MINUTE) as recently_refreshed,
                        (RefreshNotBefore IS NOT NULL AND RefreshNotBefore > NOW()) as throttled,
                        COALESCE(RefreshPodcast, 1) as auto_refresh
                    FROM Podcasts 
                    WHERE UserID = ?"
                )
                .bind(user_id)
                .fetch_all(pool)
                .await?;
                
                let mut podcasts = Vec::new();
                for row in rows {
                    podcasts.push(crate::handlers::refresh::PodcastForRefresh {
                        id: row.try_get("id")?,
                        feed_id: row.try_get("feed_id")?,
                        name: row.try_get("name")?,
                        feed_url: row.try_get("feed_url")?,
                        artwork_url: row.try_get("artwork_url").unwrap_or_default(),
                        is_youtube: row.try_get("is_youtube")?,
                        auto_download: row.try_get("auto_download")?,
                        auto_queue: row.try_get("auto_queue").unwrap_or(false),
                        username: row.try_get("username").ok(),
                        password: row.try_get("password").ok(),
                        feed_cutoff_days: row.try_get("feed_cutoff_days").ok(),
                        user_id: row.try_get("user_id")?,
                        etag: row.try_get("etag").ok().flatten(),
                        last_modified: row.try_get("last_modified").ok().flatten(),
                        content_hash: row.try_get("content_hash").ok().flatten(),
                        refresh_interval: row.try_get("refresh_interval").ok().flatten(),
                        recently_refreshed: row.try_get::<i64, _>("recently_refreshed").unwrap_or(0) != 0,
                        throttled: row.try_get::<i64, _>("throttled").unwrap_or(0) != 0,
                        auto_refresh: row.try_get::<i64, _>("auto_refresh").map(|v| v != 0).unwrap_or(true),
                    });
                }
                Ok(podcasts)
            },
        })
    }

    // Add episodes - matches Python add_episodes function exactly
    pub async fn add_episodes(
        &self,
        podcast_id: i32,
        feed_url: &str,
        artwork_url: &str,
        _auto_download: bool,
        username: Option<&str>,
        password: Option<&str>,
        feed_body: Option<&str>,
    ) -> AppResult<Option<i32>> {
        // Reuse the body the caller already fetched (add-podcast reads the feed's metadata first).
        let fetched;
        let content = match feed_body {
            Some(body) => body,
            None => {
                fetched = self.try_fetch_feed(feed_url, username, password).await?;
                fetched.as_str()
            }
        };

        // Parse the RSS feed, then apply via the shared dedup/insert path (GUID→URL→title dedup,
        // no-op UPDATE skipping, batched insert). This is the initial import of the back
        // catalogue, so no new-episode notifications.
        // The podcast's details as the feed gives them (not as a search result described it).
        let meta = crate::services::feed_metadata::parse(content);
        if let Some(meta) = &meta {
            self.store_feed_metadata(None, &[podcast_id], meta).await;
        }
        let artwork = meta.and_then(|m| m.artwork).unwrap_or_else(|| artwork_url.to_string());
        let artwork_url = artwork.as_str();
        let episodes = self.parse_rss_feed(content, podcast_id, artwork_url).await?;
        let new = self.apply_parsed_episodes(podcast_id, &episodes, artwork_url, None, false).await?;
        // A feed someone else already follows has its episodes already: count them, and match
        // sync actions for them, for this subscription too.
        if new.is_empty() {
            self.update_episode_count(podcast_id).await?;
            if let Err(e) = crate::services::sync::rematch(self, podcast_id).await {
                warn!("Matching logged sync actions for podcast {} failed: {}", podcast_id, e);
            }
        }
        self.store_feed_tags(&[podcast_id], content).await;
        // The next scheduled refresh can skip this exact body.
        let hash = crate::services::http::content_hash(content);
        if let Err(e) = self.store_feed_cache_validators(&[podcast_id], None, None, Some(&hash)).await {
            warn!("Storing feed hash for podcast {} failed: {}", podcast_id, e);
        }
        // Adding the podcast was its refresh: the next one follows its publishing pattern.
        let dates: Vec<DateTime<Utc>> = episodes.iter().map(|e| e.pub_date).collect();
        let hints = crate::services::refresh_schedule::parse_hints(content);
        let next = crate::services::refresh_schedule::schedule(&dates, &hints, Utc::now());
        if let Err(e) = self.set_refresh_schedule(&[podcast_id], next).await {
            warn!("Scheduling the next refresh of podcast {} failed: {}", podcast_id, e);
        }
        crate::services::people::podpeople::queue_sync(podcast_id);

        // Get the actual first episode ID (earliest by pub date)
        let first_id = self.get_first_episode_id(podcast_id, false).await?;

        Ok(first_id)
    }

    /// Store a fetched feed's Podcasting 2.0 tags (people, chapters, transcripts, podroll,
    /// funding, value) for each podcast row following it. Run after its episodes are applied.
    /// Failures are logged and never fail the refresh.
    pub async fn store_feed_tags(&self, podcast_ids: &[i32], content: &str) {
        let tags = crate::services::podcasting2::parse(content);
        for podcast_id in podcast_ids {
            if let Err(e) = crate::services::people::store::apply_feed_tags(self, *podcast_id, &tags).await {
                warn!("Storing Podcasting 2.0 tags for podcast {} failed: {}", podcast_id, e);
            }
        }
    }

    /// Public wrapper around the RSS parser so the refresh layer can parse a feed body once and
    /// apply it to multiple subscriber podcasts (cross-user fetch dedup).
    pub async fn parse_feed_body(
        &self,
        content: &str,
        podcast_id: i32,
        artwork_url: &str,
    ) -> AppResult<Vec<EpisodeData>> {
        self.parse_rss_feed(content, podcast_id, artwork_url).await
    }

    /// Apply a parsed episode list to a single podcast: dedup against existing rows, update
    /// changed episodes, insert new ones, and return ONLY the newly inserted episodes.
    ///
    /// Dedup priority is GUID → URL-base → title. GUID is the stable RSS identity; URL-base
    /// (URL without query string) handles feeds that regenerate tracking params; title is the
    /// last resort for feeds with neither. Existing rows are only UPDATEd when a comparable field
    /// actually changed (or to backfill a newly-available GUID), which eliminates the previous
    /// write storm that re-UPDATEd the entire back-catalogue on every refresh cycle.
    ///
    /// `cutoff_days`: when Some(d) with d > 0, episodes older than `now - d days` are skipped.
    pub async fn apply_parsed_episodes(
        &self,
        podcast_id: i32,
        episodes: &[EpisodeData],
        artwork_url: &str,
        cutoff_days: Option<i32>,
        notify: bool,
    ) -> AppResult<Vec<crate::handlers::podcasts::Episode>> {
        // Apply feed cutoff (previously honored only for YouTube channels).
        let cutoff_date = match cutoff_days {
            Some(days) if days > 0 => Some(Utc::now() - chrono::Duration::days(days as i64)),
            _ => None,
        };
        let candidates: Vec<&EpisodeData> = episodes
            .iter()
            .filter(|ep| cutoff_date.map_or(true, |cut| ep.pub_date >= cut))
            .collect();

        let parsed_count = candidates.len();
        let feed_id = self.feed_of_podcast(podcast_id).await?;
        let mut new_episodes = Vec::new();
        let mut updated_count: usize = 0;
        let mut pending: Vec<NewEpisodeRow> = Vec::new();
        let mut pending_guids: std::collections::HashSet<&str> = std::collections::HashSet::new();

        // Dedup key lists. url_bases/titles cover ALL candidates (not just guid-less ones) so a
        // new episode that now carries a GUID can still match a legacy row stored before GUIDs
        // existed (NULL guid) and backfill it instead of duplicating.
        let guids: Vec<String> = candidates.iter().filter_map(|ep| ep.guid.clone()).collect();
        let url_bases: Vec<String> = candidates
            .iter()
            .flat_map(|ep| [Some(ep.url.as_str()), ep.player_url.as_deref()])
            .flatten()
            .filter(|url| !url.is_empty())
            .map(|url| url.split('?').next().unwrap_or(url).to_string())
            .collect();
        let titles_no_url: Vec<String> = candidates
            .iter()
            .filter(|ep| ep.url.is_empty() || ep.guid.is_none())
            .map(|ep| ep.title.clone())
            .collect();

        let existing = self
            .fetch_existing_episodes(feed_id, &guids, &url_bases, &titles_no_url)
            .await?;

        for ep in candidates {
            // Resolve an existing row by GUID, then URL-base, then title.
            let existing_id = ep
                .guid
                .as_ref()
                .and_then(|g| existing.guid_to_id.get(g).copied())
                .or_else(|| {
                    if ep.url.is_empty() {
                        existing.title_to_id.get(&ep.title).copied()
                    } else {
                        let base = |url: &str| url.split('?').next().unwrap_or(url).to_string();
                        existing.url_to_id.get(&base(&ep.url)).copied()
                            // Rows 0.9.1 stored with the player page or no URL at all.
                            .or_else(|| ep.player_url.as_deref().and_then(|p| existing.url_to_id.get(&base(p)).copied()))
                            .or_else(|| {
                                existing.title_to_id.get(&ep.title).copied().filter(|id| {
                                    existing.by_id.get(id).is_some_and(|row| row.url.is_empty() && row.guid.is_none())
                                })
                            })
                    }
                });

            // The artwork we would store, with the podcast fallback applied. Compare against THIS
            // (not the raw parsed value) so an episode whose feed has no artwork doesn't look
            // "changed" every cycle just because the stored row holds the fallback artwork.
            let artwork = if ep.artwork_url.is_empty() { artwork_url } else { ep.artwork_url.as_str() };

            if let Some(episode_id) = existing_id {
                // Compare against the stored row; only UPDATE when something actually changed or we
                // can backfill a GUID. pubdate is intentionally excluded from change detection to
                // avoid timestamp-precision false positives — it virtually never changes for an
                // already-published episode, and not rewriting it is harmless.
                let needs_update = match existing.by_id.get(&episode_id) {
                    Some(cur) => {
                        // Compare URLs by base (without query string): many feeds regenerate
                        // tracking params every fetch, and rewriting the row for that alone is the
                        // write-amplification we're eliminating. A real URL change (e.g. CDN
                        // migration) changes the base and still triggers an update.
                        let cur_base = cur.url.split('?').next().unwrap_or(&cur.url);
                        let new_base = ep.url.split('?').next().unwrap_or(&ep.url);
                        cur.title != ep.title
                            || cur.description != ep.description
                            || cur_base != new_base
                            || cur.artwork != artwork
                            || cur.duration != ep.duration
                            || cur.is_video != ep.is_video
                            || (ep.guid.is_some() && cur.guid.as_deref() != ep.guid.as_deref())
                    }
                    None => true,
                };

                if needs_update {
                    dialect_match!(self, {
                        pg(pool) => {
                            q::query(
                                r#"UPDATE "Episodes"
                                   SET episodetitle = $1, episodedescription = $2, episodeurl = $3,
                                       episodeartwork = $4, episodepubdate = $5, episodeduration = $6,
                                       is_video = $7, episodeguid = COALESCE($8, episodeguid)
                                   WHERE episodeid = $9"#,
                            )
                            .bind(&ep.title)
                            .bind(&ep.description)
                            .bind(&ep.url)
                            .bind(artwork)
                            .bind(ep.pub_date)
                            .bind(ep.duration)
                            .bind(ep.is_video)
                            .bind(ep.guid.as_deref())
                            .bind(episode_id)
                            .execute(pool)
                            .await?;
                        },
                        mysql(pool) => {
                            sqlx::query(
                                "UPDATE Episodes
                                 SET EpisodeTitle = ?, EpisodeDescription = ?, EpisodeURL = ?,
                                     EpisodeArtwork = ?, EpisodePubDate = ?, EpisodeDuration = ?,
                                     IsVideo = ?, EpisodeGUID = COALESCE(?, EpisodeGUID)
                                 WHERE EpisodeID = ?",
                            )
                            .bind(&ep.title)
                            .bind(&ep.description)
                            .bind(&ep.url)
                            .bind(artwork)
                            .bind(ep.pub_date)
                            .bind(ep.duration)
                            .bind(ep.is_video)
                            .bind(ep.guid.as_deref())
                            .bind(episode_id)
                            .execute(pool)
                            .await?;
                        },
                    });
                    updated_count += 1;
                }
                // Existing episode — never returned as "new".
                continue;
            }

            // NEW episode: collected and inserted together below. A feed that lists the same GUID
            // twice yields one episode.
            if let Some(g) = &ep.guid {
                if !pending_guids.insert(g.as_str()) {
                    continue;
                }
            }
            pending.push(NewEpisodeRow { ep, artwork, duration: ep.duration });
        }

        // Durations: probe only the newest few episodes that lack one (a new podcast's back
        // catalogue would otherwise be hundreds of sequential HEAD requests before it shows up).
        // The rest are filled in by a background job after insert.
        let mut probe_order: Vec<usize> = (0..pending.len())
            .filter(|&i| pending[i].duration == 0 && !pending[i].ep.url.is_empty())
            .collect();
        probe_order.sort_by(|&a, &b| pending[b].ep.pub_date.cmp(&pending[a].ep.pub_date));
        let backfill_from = probe_order.len().min(DURATION_PROBES_PER_APPLY);
        use futures::StreamExt;
        let probed: Vec<(usize, Option<i32>)> = futures::stream::iter(probe_order[..backfill_from].to_vec())
            .map(|i| {
                let url = pending[i].ep.url.clone();
                async move { (i, self.estimate_duration_from_audio_url_async(&url).await) }
            })
            .buffer_unordered(DURATION_PROBE_CONCURRENCY)
            .collect()
            .await;
        for (i, est) in probed {
            if let Some(d) = est {
                debug!("Estimated duration {} seconds for new episode: {}", d, pending[i].ep.title);
                pending[i].duration = d;
            }
        }
        let needs_backfill: std::collections::HashSet<usize> = probe_order[backfill_from..].iter().copied().collect();

        let inserted = self.insert_new_episodes(podcast_id, feed_id, &pending).await?;
        let skipped_count = pending.len() - inserted.len();

        let mut backfill = Vec::new();
        for (i, episode_id) in inserted {
            let row = &pending[i];
            if needs_backfill.contains(&i) {
                backfill.push((episode_id, row.ep.url.clone()));
            }
            new_episodes.push(crate::handlers::podcasts::Episode {
                podcastid: podcast_id,
                podcastname: "".to_string(),
                episodetitle: row.ep.title.clone(),
                episodepubdate: row.ep.pub_date.format("%Y-%m-%dT%H:%M:%S").to_string(),
                episodedescription: row.ep.description.clone(),
                episodeartwork: row.artwork.to_string(),
                episodeurl: row.ep.url.clone(),
                episodeduration: row.duration,
                listenduration: None,
                episodeid: episode_id,
                completed: false,
                saved: false,
                queued: false,
                downloaded: false,
                is_youtube: false,
                is_video: row.ep.is_video,
            });
        }

        if !backfill.is_empty() {
            debug!("Podcast {}: backfilling {} episode durations in the background", podcast_id, backfill.len());
            let db = self.clone();
            tokio::spawn(async move { db.backfill_episode_durations(backfill).await });
        }

        tracing::debug!(
            "Podcast {}: parsed={} inserted={} updated={} skipped={}",
            podcast_id, parsed_count, new_episodes.len(), updated_count, skipped_count
        );

        // Every subscription of the feed sees the new episodes: recount, notify, and apply sync
        // actions that arrived before the episodes existed.
        if !new_episodes.is_empty() {
            for subscription in self.feed_subscriptions(feed_id).await? {
                if notify {
                    match self.load_notification_targets(subscription).await {
                        Ok(targets) if !targets.is_empty() => {
                            for ep in &new_episodes {
                                Self::send_new_episode_notifications(&targets, &ep.episodetitle).await;
                            }
                        }
                        Ok(_) => {}
                        Err(e) => warn!("Loading notification settings for podcast {} failed: {}", subscription, e),
                    }
                }
                self.update_episode_count(subscription).await?;
                if let Err(e) = crate::services::sync::rematch(self, subscription).await {
                    warn!("Matching logged sync actions for podcast {} failed: {}", subscription, e);
                }
            }
        }

        Ok(new_episodes)
    }

    /// Insert new episodes for one feed in a single transaction. Rows whose
    /// (FeedID, EpisodeGUID) already exists (another refresh got there first) are skipped by
    /// the unique index rather than duplicated. Returns (index into `rows`, EpisodeID) for each
    /// row actually inserted.
    pub(super) async fn insert_new_episodes(&self, podcast_id: i32, feed_id: i32, rows: &[NewEpisodeRow<'_>]) -> AppResult<Vec<(usize, i32)>> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        // Concurrent inserts of the same keys can deadlock (InnoDB takes shared locks on the
        // duplicates); the database rolls one transaction back, and running it again simply
        // skips what the other one inserted.
        let mut attempt = 0;
        loop {
            match self.insert_new_episodes_once(podcast_id, feed_id, rows).await {
                Err(e) if attempt < 3 && is_write_conflict(&e) => {
                    attempt += 1;
                    debug!("Episode insert for podcast {} hit a deadlock, retrying ({}/3)", podcast_id, attempt);
                    tokio::time::sleep(Duration::from_millis(20 * attempt as u64)).await;
                }
                other => return other,
            }
        }
    }

    pub(super) async fn insert_new_episodes_once(&self, podcast_id: i32, feed_id: i32, rows: &[NewEpisodeRow<'_>]) -> AppResult<Vec<(usize, i32)>> {
        let mut inserted = Vec::with_capacity(rows.len());
        dialect_match!(self, {
            pg(pool) => {
                // RETURNING can't carry the input position, so match rows back by GUID (unique
                // per feed, and deduplicated above) or, without one, by URL + title.
                let mut by_guid: HashMap<&str, usize> = HashMap::new();
                let mut by_url_title: HashMap<(String, String), Vec<usize>> = HashMap::new();
                for (i, r) in rows.iter().enumerate() {
                    match &r.ep.guid {
                        Some(g) => { by_guid.insert(g.as_str(), i); }
                        None => by_url_title.entry((r.ep.url.clone(), r.ep.title.clone())).or_default().push(i),
                    }
                }
                let mut tx = q::begin(pool).await?;
                for chunk in rows.chunks(EPISODE_INSERT_CHUNK) {
                    let returned = q::query(
                        r#"INSERT INTO "Episodes"
                           (podcastid, feedid, episodetitle, episodedescription, episodeurl, episodeartwork,
                            episodepubdate, episodeduration, is_video, episodeguid)
                           SELECT $1, $10, t.* FROM UNNEST($2::text[], $3::text[], $4::text[], $5::text[],
                                                      $6::timestamptz[], $7::int4[], $8::bool[], $9::text[]) AS t
                           ON CONFLICT DO NOTHING
                           RETURNING episodeid, episodeguid, episodeurl, episodetitle"#,
                    )
                    .bind(podcast_id)
                    .bind(chunk.iter().map(|r| r.ep.title.as_str()).collect::<Vec<_>>())
                    .bind(chunk.iter().map(|r| r.ep.description.as_str()).collect::<Vec<_>>())
                    .bind(chunk.iter().map(|r| r.ep.url.as_str()).collect::<Vec<_>>())
                    .bind(chunk.iter().map(|r| r.artwork).collect::<Vec<_>>())
                    .bind(chunk.iter().map(|r| r.ep.pub_date).collect::<Vec<_>>())
                    .bind(chunk.iter().map(|r| r.duration).collect::<Vec<_>>())
                    .bind(chunk.iter().map(|r| r.ep.is_video).collect::<Vec<_>>())
                    .bind(chunk.iter().map(|r| r.ep.guid.as_deref()).collect::<Vec<_>>())
                    .bind(feed_id)
                    .fetch_all(&mut *tx)
                    .await?;
                    for row in returned {
                        let id: i32 = row.try_get("episodeid")?;
                        let guid: Option<String> = row.try_get("episodeguid")?;
                        let index = match guid {
                            Some(g) => by_guid.get(g.as_str()).copied(),
                            None => {
                                let url: String = row.try_get("episodeurl")?;
                                let title: String = row.try_get("episodetitle")?;
                                by_url_title.get_mut(&(url, title)).and_then(|v| v.pop())
                            }
                        };
                        match index {
                            Some(i) => inserted.push((i, id)),
                            None => warn!("Inserted episode {} for podcast {} didn't match any parsed entry", id, podcast_id),
                        }
                    }
                }
                tx.commit().await?;
            },
            mysql(pool) => {
                // Row by row inside one transaction: a multi-row INSERT IGNORE can't say which rows
                // were skipped or which ids the others got (auto-increment may interleave with a
                // concurrent refresh), and the transaction makes per-row cost small.
                let mut tx = pool.begin().await?;
                for (i, r) in rows.iter().enumerate() {
                    let result = sqlx::query(
                        "INSERT IGNORE INTO Episodes
                         (PodcastID, FeedID, EpisodeTitle, EpisodeDescription, EpisodeURL, EpisodeArtwork, EpisodePubDate, EpisodeDuration, IsVideo, EpisodeGUID)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(podcast_id)
                    .bind(feed_id)
                    .bind(&r.ep.title)
                    .bind(&r.ep.description)
                    .bind(&r.ep.url)
                    .bind(r.artwork)
                    .bind(r.ep.pub_date)
                    .bind(r.duration)
                    .bind(r.ep.is_video)
                    .bind(r.ep.guid.as_deref())
                    .execute(&mut *tx)
                    .await?;
                    if result.rows_affected() == 1 {
                        inserted.push((i, result.last_insert_id() as i32));
                    }
                }
                tx.commit().await?;
            },
        });
        if inserted.len() < rows.len() {
            debug!(
                "Podcast {}: {} new episodes were already inserted by a concurrent refresh",
                podcast_id,
                rows.len() - inserted.len()
            );
        }
        Ok(inserted)
    }

    /// Estimate durations for episodes that were inserted without one, a few at a time across the
    /// whole server, and store them unless something (a player report) filled one in meanwhile.
    pub(super) async fn backfill_episode_durations(&self, episodes: Vec<(i32, String)>) {
        static PROBES: std::sync::LazyLock<tokio::sync::Semaphore> = std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(DURATION_BACKFILL_CONCURRENCY));
        let mut filled = 0usize;
        for (episode_id, url) in &episodes {
            let Ok(_permit) = PROBES.acquire().await else { return };
            let Some(duration) = self.estimate_duration_from_audio_url_async(url).await else { continue };
            if let Err(e) = self.set_episode_duration_if_missing(*episode_id, duration).await {
                warn!("Storing estimated duration for episode {} failed: {}", episode_id, e);
            } else {
                filled += 1;
            }
        }
        debug!("Duration backfill: filled {}/{} episodes", filled, episodes.len());
    }

    /// Store `duration` (seconds) for an episode whose duration is still unknown (NULL or 0).
    pub async fn set_episode_duration_if_missing(&self, episode_id: i32, duration: i32) -> AppResult<()> {
        if duration <= 0 {
            return Ok(());
        }
        dialect_match!(self, {
            pg(pool) => {
                q::query(
                    r#"UPDATE "Episodes" SET episodeduration = $1
                       WHERE episodeid = $2 AND COALESCE(episodeduration, 0) = 0"#,
                )
                .bind(duration)
                .bind(episode_id)
                .execute(pool)
                .await?;
            },
            mysql(pool) => {
                sqlx::query("UPDATE Episodes SET EpisodeDuration = ? WHERE EpisodeID = ? AND COALESCE(EpisodeDuration, 0) = 0")
                    .bind(duration)
                    .bind(episode_id)
                    .execute(pool)
                    .await?;
            },
        });
        Ok(())
    }

    /// Load existing episodes for a podcast that match any of the given GUIDs, URL bases, or
    /// titles, returning lookup maps plus the comparable field values (for no-op change detection).
    /// Runs at most three targeted queries instead of one scan per episode.
    pub(super) async fn fetch_existing_episodes(
        &self,
        feed_id: i32,
        guids: &[String],
        url_bases: &[String],
        titles_no_url: &[String],
    ) -> AppResult<ExistingEpisodes> {
        let mut out = ExistingEpisodes::default();

        dialect_match!(self, {
            pg(pool) => {
                if !guids.is_empty() {
                    let rows = q::query(
                        r#"SELECT episodeid, episodeguid, episodetitle, episodedescription, episodeurl,
                                  episodeartwork, episodeduration, is_video
                           FROM "Episodes"
                           WHERE feedid = $1 AND episodeguid = ANY($2)"#,
                    )
                    .bind(feed_id)
                    .bind(guids)
                    .fetch_all(pool)
                    .await?;
                    for row in rows {
                        if let Ok(id) = row.try_get::<i32, _>("episodeid") {
                            let ex = q::pick(pg_existing_from_row, lite_existing_from_row)(&row);
                            if let Some(g) = ex.guid.clone() { out.guid_to_id.insert(g, id); }
                            out.by_id.insert(id, ex);
                        }
                    }
                }
                if !url_bases.is_empty() {
                    let rows = q::query(
                        r#"SELECT episodeid, episodeguid, episodetitle, episodedescription, episodeurl,
                                  episodeartwork, episodeduration, is_video,
                                  split_part(episodeurl, '?', 1) AS url_base
                           FROM "Episodes"
                           WHERE feedid = $1 AND split_part(episodeurl, '?', 1) = ANY($2)"#,
                    )
                    .bind(feed_id)
                    .bind(url_bases)
                    .fetch_all(pool)
                    .await?;
                    for row in rows {
                        if let (Ok(id), Ok(base)) = (row.try_get::<i32, _>("episodeid"), row.try_get::<String, _>("url_base")) {
                            out.url_to_id.insert(base, id);
                            out.by_id.entry(id).or_insert_with(|| q::pick(pg_existing_from_row, lite_existing_from_row)(&row));
                        }
                    }
                }
                if !titles_no_url.is_empty() {
                    let rows = q::query(
                        r#"SELECT episodeid, episodeguid, episodetitle, episodedescription, episodeurl,
                                  episodeartwork, episodeduration, is_video
                           FROM "Episodes"
                           WHERE feedid = $1 AND episodetitle = ANY($2)"#,
                    )
                    .bind(feed_id)
                    .bind(titles_no_url)
                    .fetch_all(pool)
                    .await?;
                    for row in rows {
                        if let (Ok(id), Ok(t)) = (row.try_get::<i32, _>("episodeid"), row.try_get::<String, _>("episodetitle")) {
                            out.title_to_id.insert(t, id);
                            out.by_id.entry(id).or_insert_with(|| q::pick(pg_existing_from_row, lite_existing_from_row)(&row));
                        }
                    }
                }
            },
            mysql(pool) => {
                if !guids.is_empty() {
                    let placeholders = guids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                    let sql = format!(
                        "SELECT EpisodeID, EpisodeGUID, EpisodeTitle, EpisodeDescription, EpisodeURL, \
                         EpisodeArtwork, EpisodeDuration, IsVideo \
                         FROM Episodes WHERE FeedID = ? AND EpisodeGUID IN ({})",
                        placeholders
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(feed_id);
                    for g in guids { q = q.bind(g); }
                    let rows = q.fetch_all(pool).await?;
                    for row in rows {
                        if let Ok(id) = row.try_get::<i32, _>("EpisodeID") {
                            let ex = mysql_existing_from_row(&row);
                            if let Some(g) = ex.guid.clone() { out.guid_to_id.insert(g, id); }
                            out.by_id.insert(id, ex);
                        }
                    }
                }
                if !url_bases.is_empty() {
                    let placeholders = url_bases.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                    let sql = format!(
                        "SELECT EpisodeID, EpisodeGUID, EpisodeTitle, EpisodeDescription, EpisodeURL, \
                         EpisodeArtwork, EpisodeDuration, IsVideo, \
                         SUBSTRING_INDEX(EpisodeURL, '?', 1) AS url_base \
                         FROM Episodes WHERE FeedID = ? AND SUBSTRING_INDEX(EpisodeURL, '?', 1) IN ({})",
                        placeholders
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(feed_id);
                    for base in url_bases { q = q.bind(base); }
                    let rows = q.fetch_all(pool).await?;
                    for row in rows {
                        if let (Ok(id), Ok(base)) = (row.try_get::<i32, _>("EpisodeID"), row.try_get::<String, _>("url_base")) {
                            out.url_to_id.insert(base, id);
                            out.by_id.entry(id).or_insert_with(|| mysql_existing_from_row(&row));
                        }
                    }
                }
                if !titles_no_url.is_empty() {
                    let placeholders = titles_no_url.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
                    let sql = format!(
                        "SELECT EpisodeID, EpisodeGUID, EpisodeTitle, EpisodeDescription, EpisodeURL, \
                         EpisodeArtwork, EpisodeDuration, IsVideo \
                         FROM Episodes WHERE FeedID = ? AND EpisodeTitle IN ({})",
                        placeholders
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(feed_id);
                    for t in titles_no_url { q = q.bind(t); }
                    let rows = q.fetch_all(pool).await?;
                    for row in rows {
                        if let (Ok(id), Ok(t)) = (row.try_get::<i32, _>("EpisodeID"), row.try_get::<String, _>("EpisodeTitle")) {
                            out.title_to_id.insert(t, id);
                            out.by_id.entry(id).or_insert_with(|| mysql_existing_from_row(&row));
                        }
                    }
                }
            },
        });

        Ok(out)
    }

    /// Conditional feed fetch through the shared feed client. Sends If-None-Match /
    /// If-Modified-Since from the stored validators, and also compares the body's SHA-256 with the
    /// stored hash so hosts with missing or unreliable ETags still skip parse/apply when nothing
    /// changed. Status handling:
    /// - 304, or 200 with an unchanged body → `NotModified`;
    /// - 403/406 → one retry through the browser-UA / www fallbacks (the only case they help);
    /// - 429/503 → `Throttled` with the host's `Retry-After`, for the caller's backoff;
    /// - anything else (404/410, 5xx, transport errors) → error, with no second fetch.
    pub async fn fetch_feed_conditional(
        &self,
        url: &str,
        username: Option<&str>,
        password: Option<&str>,
        prev: FeedValidators<'_>,
    ) -> AppResult<FeedFetch> {
        // SSRF guard: url is a stored feed URL (originally user-supplied/imported).
        if let Err(reason) = crate::services::url_guard::ensure_safe_public_url_async(url).await {
            warn!("Refusing to refresh feed from unsafe URL {}: {}", url, reason);
            return Err(AppError::bad_request(format!("Refused to fetch feed URL: {}", reason)));
        }

        // Always issue the request (even with no prior validators) so we can capture ETag /
        // Last-Modified for next time.
        let mut request = crate::services::http::feed_client().get(url);
        if let (Some(user), Some(pass)) = (username, password) {
            request = request.basic_auth(user, Some(pass));
        }
        if let Some(etag) = prev.etag {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        if let Some(lm) = prev.last_modified {
            request = request.header(reqwest::header::IF_MODIFIED_SINCE, lm);
        }

        let response = request.send().await.map_err(|e| {
            warn!("Feed request failed for {}: {}", url, e);
            AppError::Http(e)
        })?;
        let status = response.status();
        let max_age = response
            .headers()
            .get(reqwest::header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok())
            .and_then(crate::services::refresh_schedule::max_age);

        let (body, etag, last_modified) = if status == reqwest::StatusCode::NOT_MODIFIED {
            debug!("Feed not modified (304): {}", url);
            return Ok(FeedFetch::NotModified { etag: None, last_modified: None, max_age });
        } else if status.is_success() {
            let header = |name: reqwest::header::HeaderName| {
                response.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
            };
            let etag = header(reqwest::header::ETAG);
            let last_modified = header(reqwest::header::LAST_MODIFIED);
            (response.text().await.map_err(AppError::Http)?, etag, last_modified)
        } else if status == reqwest::StatusCode::FORBIDDEN || status == reqwest::StatusCode::NOT_ACCEPTABLE {
            debug!("Feed {} returned {}, retrying with fallbacks", url, status);
            (self.fetch_feed_fallbacks(url, username, password, status).await?, None, None)
        } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS || status == reqwest::StatusCode::SERVICE_UNAVAILABLE {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| crate::services::http::parse_retry_after(v, Utc::now()));
            warn!("Feed {} throttled (HTTP {}), retry after {:?}", url, status.as_u16(), retry_after);
            return Ok(FeedFetch::Throttled { status: status.as_u16(), retry_after });
        } else if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::GONE {
            warn!("Feed {} is gone (HTTP {})", url, status.as_u16());
            return Err(AppError::external_error(&format!("Feed gone: HTTP {}", status.as_u16())));
        } else {
            warn!("Feed {} failed with HTTP {}", url, status.as_u16());
            return Err(AppError::external_error(&format!("Feed request failed: HTTP {}", status.as_u16())));
        };

        let content_hash = crate::services::http::content_hash(&body);
        if prev.content_hash == Some(content_hash.as_str()) {
            debug!("Feed body unchanged (hash match), skipping parse: {}", url);
            return Ok(FeedFetch::NotModified { etag, last_modified, max_age });
        }
        Ok(FeedFetch::Fetched { body, etag, last_modified, content_hash, max_age })
    }

    /// Persist fresh conditional-GET validators (ETag / Last-Modified) and, when given, the body
    /// hash for the feed of these podcast rows (and the rows), so the next cycle can skip it. A
    /// `None` hash leaves the stored one alone (a 304 carries no body).
    pub async fn store_feed_cache_validators(
        &self,
        podcast_ids: &[i32],
        etag: Option<&str>,
        last_modified: Option<&str>,
        content_hash: Option<&str>,
    ) -> AppResult<()> {
        if podcast_ids.is_empty() {
            return Ok(());
        }
        dialect_match!(self, {
            pg(pool) => {
                for (table, rows) in pg_refresh_targets(4) {
                    q::query(sqlx::AssertSqlSafe(format!(
                        r#"UPDATE {table} SET feedetag = $1, feedlastmodified = $2,
                                  feedcontenthash = COALESCE($3, feedcontenthash)
                           WHERE {rows}"#
                    )))
                    .bind(etag)
                    .bind(last_modified)
                    .bind(content_hash)
                    .bind(podcast_ids)
                    .execute(pool)
                    .await?;
                }
            },
            mysql(pool) => {
                for (table, rows) in mysql_refresh_targets(podcast_ids.len()) {
                    let sql = format!(
                        "UPDATE {table} SET FeedETag = ?, FeedLastModified = ?, \
                         FeedContentHash = COALESCE(?, FeedContentHash) WHERE {rows}"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str()))
                        .bind(etag)
                        .bind(last_modified)
                        .bind(content_hash);
                    for id in podcast_ids { q = q.bind(id); }
                    q.execute(pool).await?;
                }
            },
        });
        Ok(())
    }

    /// Mark a successful refresh for the feed of these podcast rows: clears the failure counter
    /// and any Retry-After hold, records the timestamps used for backoff decisions, and schedules
    /// the next refresh.
    pub async fn record_refresh_success(&self, podcast_ids: &[i32], next: crate::services::refresh_schedule::Schedule) -> AppResult<()> {
        if podcast_ids.is_empty() {
            return Ok(());
        }
        dialect_match!(self, {
            pg(pool) => {
                for (table, rows) in pg_refresh_targets(1) {
                    let notified = if table == PG_PODCASTS { ", problemnotified = FALSE" } else { "" };
                    q::query(sqlx::AssertSqlSafe(format!(
                        r#"UPDATE {table}
                           SET lastrefreshattempt = NOW(), lastrefreshsuccess = NOW(),
                               consecutivefailures = 0, lasterrormessage = NULL, refreshnotbefore = NULL,
                               failingsince = NULL, lasterrorkind = NULL{notified},
                               refreshintervalminutes = $2, nextrefreshat = NOW() + make_interval(secs => $3::BIGINT)
                           WHERE {rows}"#
                    )))
                    .bind(podcast_ids)
                    .bind(next.interval_minutes)
                    .bind(next.next_in_secs)
                    .execute(pool)
                    .await?;
                }
            },
            mysql(pool) => {
                for (table, rows) in mysql_refresh_targets(podcast_ids.len()) {
                    let notified = if table == "Podcasts" { "ProblemNotified = FALSE, " } else { "" };
                    let sql = format!(
                        "UPDATE {table} SET LastRefreshAttempt = NOW(), LastRefreshSuccess = NOW(), \
                         ConsecutiveFailures = 0, LastErrorMessage = NULL, RefreshNotBefore = NULL, \
                         FailingSince = NULL, LastErrorKind = NULL, {notified}\
                         RefreshIntervalMinutes = ?, NextRefreshAt = NOW() + INTERVAL ? SECOND \
                         WHERE {rows}"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(next.interval_minutes).bind(next.next_in_secs);
                    for id in podcast_ids { q = q.bind(id); }
                    q.execute(pool).await?;
                }
            },
        });
        Ok(())
    }

    /// Schedule the next refresh of the feed of these podcast rows without recording a refresh
    /// (after the import that added a podcast).
    pub async fn set_refresh_schedule(&self, podcast_ids: &[i32], next: crate::services::refresh_schedule::Schedule) -> AppResult<()> {
        if podcast_ids.is_empty() {
            return Ok(());
        }
        dialect_match!(self, {
            pg(pool) => {
                for (table, rows) in pg_refresh_targets(1) {
                    q::query(sqlx::AssertSqlSafe(format!(
                        r#"UPDATE {table} SET refreshintervalminutes = $2, nextrefreshat = NOW() + make_interval(secs => $3::BIGINT)
                           WHERE {rows}"#
                    )))
                    .bind(podcast_ids)
                    .bind(next.interval_minutes)
                    .bind(next.next_in_secs)
                    .execute(pool)
                    .await?;
                }
            },
            mysql(pool) => {
                for (table, rows) in mysql_refresh_targets(podcast_ids.len()) {
                    let sql = format!(
                        "UPDATE {table} SET RefreshIntervalMinutes = ?, NextRefreshAt = NOW() + INTERVAL ? SECOND WHERE {rows}"
                    );
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(next.interval_minutes).bind(next.next_in_secs);
                    for id in podcast_ids { q = q.bind(id); }
                    q.execute(pool).await?;
                }
            },
        });
        Ok(())
    }

    /// Publishing dates of a podcast's newest episodes (videos for a YouTube channel).
    pub async fn recent_pub_dates(&self, podcast_id: i32, youtube: bool, limit: i64) -> AppResult<Vec<DateTime<Utc>>> {
        let (table, column) = if youtube { ("YouTubeVideos", "PublishedAt") } else { ("Episodes", "EpisodePubDate") };
        let dates: Vec<chrono::NaiveDateTime> = dialect_match!(self, {
            pg(pool) => {
                q::query_scalar(sqlx::AssertSqlSafe(format!(
                    r#"SELECT {column} FROM "{table}" WHERE podcastid = $1 AND {column} IS NOT NULL ORDER BY {column} DESC LIMIT $2"#
                )))
                .bind(podcast_id)
                .bind(limit)
                .fetch_all(pool)
                .await?
            },
            mysql(pool) => {
                sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT {column} FROM {table} WHERE PodcastID = ? AND {column} IS NOT NULL ORDER BY {column} DESC LIMIT ?"
                )))
                .bind(podcast_id)
                .bind(limit)
                .fetch_all(pool)
                .await?
            },
        });
        Ok(dates.into_iter().map(|d| d.and_utc()).collect())
    }

    /// Record a failed refresh: bumps the consecutive-failure counter (used for backoff), stores
    /// the error and its kind for the feed problems list, and notes when the failures began.
    /// `not_before` (a host's Retry-After) additionally holds the feed back until then, on top of
    /// the usual failure backoff.
    pub async fn record_refresh_failure(
        &self,
        podcast_ids: &[i32],
        kind: crate::services::feed_health::FeedErrorKind,
        error: &str,
        not_before: Option<std::time::Duration>,
    ) -> AppResult<()> {
        if podcast_ids.is_empty() {
            return Ok(());
        }
        // Truncate overly long errors so a giant message can't bloat the row.
        let error = match error.char_indices().nth(1000) {
            Some((i, _)) => &error[..i],
            None => error,
        };
        let wait_secs = not_before.map(|d| d.as_secs() as i64);
        // The next refresh waits out the feed's interval, the failure backoff and any hold.
        use crate::services::scheduler::{BACKOFF_MAX_STEPS, BACKOFF_STEP_MINUTES};
        const DEFAULT_MINUTES: i64 = 60;
        dialect_match!(self, {
            pg(pool) => {
                for (table, rows) in pg_refresh_targets(3) {
                    q::query(sqlx::AssertSqlSafe(format!(
                        r#"UPDATE {table}
                           SET nextrefreshat = NOW() + make_interval(secs => GREATEST(
                                   COALESCE(refreshintervalminutes, {DEFAULT_MINUTES}) * 60,
                                   {BACKOFF_STEP_MINUTES} * 60 * LEAST(COALESCE(consecutivefailures, 0) + 1, {BACKOFF_MAX_STEPS}),
                                   COALESCE($2::BIGINT, 0))),
                               lastrefreshattempt = NOW(),
                               consecutivefailures = COALESCE(consecutivefailures, 0) + 1,
                               lasterrormessage = $1,
                               refreshnotbefore = CASE WHEN $2::BIGINT IS NULL THEN NULL
                                                       ELSE NOW() + make_interval(secs => $2::BIGINT) END,
                               failingsince = COALESCE(failingsince, NOW()), lasterrorkind = $4
                           WHERE {rows}"#
                    )))
                    .bind(error)
                    .bind(wait_secs)
                    .bind(podcast_ids)
                    .bind(kind.as_str())
                    .execute(pool)
                    .await?;
                }
            },
            mysql(pool) => {
                for (table, rows) in mysql_refresh_targets(podcast_ids.len()) {
                    // NextRefreshAt first: MySQL assigns left to right, so it still sees the old count.
                    let sql = format!(
                        "UPDATE {table} SET NextRefreshAt = NOW() + INTERVAL GREATEST( \
                             COALESCE(RefreshIntervalMinutes, {DEFAULT_MINUTES}) * 60, \
                             {BACKOFF_STEP_MINUTES} * 60 * LEAST(COALESCE(ConsecutiveFailures, 0) + 1, {BACKOFF_MAX_STEPS}), \
                             COALESCE(?, 0)) SECOND, \
                         LastRefreshAttempt = NOW(), \
                         ConsecutiveFailures = COALESCE(ConsecutiveFailures, 0) + 1, LastErrorMessage = ?, \
                         RefreshNotBefore = NOW() + INTERVAL ? SECOND, \
                         FailingSince = COALESCE(FailingSince, NOW()), LastErrorKind = ? \
                         WHERE {rows}"
                    );
                    // NOW() + INTERVAL NULL SECOND is NULL, which clears the hold as intended.
                    let mut q = sqlx::query(sqlx::AssertSqlSafe(sql.as_str())).bind(wait_secs).bind(error).bind(wait_secs).bind(kind.as_str());
                    for id in podcast_ids { q = q.bind(id); }
                    q.execute(pool).await?;
                }
            },
        });
        Ok(())
    }

    // Try to fetch RSS feed - matches Python try_fetch_feed function
    pub(crate) async fn try_fetch_feed(
        &self,
        url: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> AppResult<String> {
        debug!("try_fetch_feed called with URL: {}", url);

        // SSRF guard: url is a stored feed URL (originally user-supplied/imported).
        if let Err(reason) = crate::services::url_guard::ensure_safe_public_url_async(url).await {
            warn!("Refusing to fetch feed from unsafe URL {}: {}", url, reason);
            return Err(AppError::bad_request(format!("Refused to fetch feed URL: {}", reason)));
        }

        // PinePods UA first — many hosts (e.g. Omny) return full episode lists to podcast apps
        // but truncated/paginated feeds to browser UAs.
        let response = Self::feed_get(crate::services::http::feed_client(), url, username, password)
            .send()
            .await
            .map_err(|e| {
                warn!("HTTP request failed for {}: {}", url, e);
                AppError::Http(e)
            })?;

        if response.status().is_success() {
            debug!("Request succeeded with status: {}", response.status());
            return Ok(response.text().await.map_err(AppError::Http)?);
        }
        debug!("Feed {} answered {}, retrying with fallbacks", url, response.status());
        self.fetch_feed_fallbacks(url, username, password, response.status()).await
    }

    /// GET a feed with basic auth when credentials are stored.
    pub(super) fn feed_get(client: &reqwest::Client, url: &str, username: Option<&str>, password: Option<&str>) -> reqwest::RequestBuilder {
        let request = client.get(url);
        match (username, password) {
            (Some(user), Some(pass)) => request.basic_auth(user, Some(pass)),
            _ => request,
        }
    }

    /// After the normal request answered `status`: retry once with a browser UA (hosts that block
    /// podcast clients), then once on the www/non-www variant of the URL. When both fail too, the
    /// error is the feed's own answer, so a gone feed is reported as gone.
    pub(super) async fn fetch_feed_fallbacks(&self, url: &str, username: Option<&str>, password: Option<&str>, status: reqwest::StatusCode) -> AppResult<String> {
        match Self::feed_get(crate::services::http::browser_feed_client(), url, username, password).send().await {
            Ok(browser) if browser.status().is_success() => {
                debug!("Browser UA request for {} succeeded", url);
                return Ok(browser.text().await.map_err(AppError::Http)?);
            }
            Ok(browser) => debug!("Browser UA request for {} answered {}", url, browser.status()),
            Err(e) => debug!("Browser UA request for {} failed: {}", url, e),
        }

        let alternate_url = if url.contains("://www.") {
            url.replace("://www.", "://")
        } else {
            url.replace("://", "://www.")
        };
        // SSRF guard: alternate_url is a www/non-www variant not covered by the up-front check.
        match crate::services::url_guard::ensure_safe_public_url_async(&alternate_url).await {
            Err(reason) => debug!("Not trying alternate feed URL {}: {}", alternate_url, reason),
            Ok(()) => match Self::feed_get(crate::services::http::feed_client(), &alternate_url, username, password).send().await {
                Ok(alt) if alt.status().is_success() => {
                    debug!("Alternate feed URL {} succeeded", alternate_url);
                    return Ok(alt.text().await.map_err(AppError::Http)?);
                }
                Ok(alt) => debug!("Alternate feed URL {} answered {}", alternate_url, alt.status()),
                Err(e) => debug!("Alternate feed URL {} failed: {}", alternate_url, e),
            },
        }
        warn!("Feed {} answered HTTP {} (also with a browser user agent and at {})", url, status.as_u16(), alternate_url);
        Err(AppError::bad_request(&format!("Feed request failed: HTTP {}", status)))
    }

    // Get stored authentication credentials for a feed URL
    pub async fn get_feed_auth_credentials(&self, feed_url: &str, user_id: i32) -> AppResult<(Option<String>, Option<String>)> {
        dialect_match!(self, {
            pg(pool) => {
                let result = q::query(r#"SELECT username, password FROM "Podcasts" WHERE feedurl = $1 AND userid = $2 LIMIT 1"#)
                    .bind(feed_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = result {
                    let username: Option<String> = row.try_get("username").ok().flatten();
                    let password: Option<String> = row.try_get("password").ok().flatten();
                    Ok((username, password))
                } else {
                    Ok((None, None))
                }
            },
            mysql(pool) => {
                let result = sqlx::query("SELECT Username, Password FROM Podcasts WHERE FeedURL = ? AND UserID = ? LIMIT 1")
                    .bind(feed_url)
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
                
                if let Some(row) = result {
                    let username: Option<String> = row.try_get("Username").ok().flatten();
                    let password: Option<String> = row.try_get("Password").ok().flatten();
                    Ok((username, password))
                } else {
                    Ok((None, None))
                }
            },
        })
    }
}
