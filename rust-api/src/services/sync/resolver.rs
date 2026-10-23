// EpisodeResolver: the one place sync matches a gpodder action to a local episode. Episodes
// are shared by everyone subscribed to their feed; matching is scoped to the feeds the user
// subscribes to. Match order, first hit wins:
//   1. GUID, within the user's podcast whose FeedURL is the action's podcast URL;
//   2. exact EpisodeURL, any of the user's podcasts;
//   3. URL without its query string (tracking parameters differ between clients/redirectors):
//      a stored URL equal to the action's base, in any of the user's podcasts (indexed), or a
//      stored URL whose base matches, within that feed's podcast only (a scan, so kept narrow).
// Ties (the same URL in two of the user's podcasts) go to the lowest EpisodeID.

use crate::database::DatabasePool;
use crate::error::AppResult;

#[derive(Debug, Clone, Copy)]
pub struct ResolveKey<'a> {
    pub podcast_url: &'a str,
    pub episode_url: &'a str,
    pub guid: Option<&'a str>,
}

impl<'a> ResolveKey<'a> {
    pub fn of(action: &'a super::EpisodeAction) -> Self {
        ResolveKey { podcast_url: &action.podcast_url, episode_url: &action.episode_url, guid: action.guid.as_deref() }
    }
}

/// `url` without its query string (matches the SQL side: split_part / SUBSTRING_INDEX on '?').
pub fn url_base(url: &str) -> &str {
    url.split('?').next().unwrap_or(url)
}

/// Local EpisodeID for each key (None = unmatched), in order. One query per batch of keys.
pub async fn resolve(db: &DatabasePool, user_id: i32, keys: &[ResolveKey<'_>]) -> AppResult<Vec<Option<i32>>> {
    if keys.is_empty() {
        return Ok(Vec::new());
    }
    db.sync_resolve(user_id, keys).await
}

#[cfg(test)]
mod tests {
    use super::url_base;

    #[test]
    fn url_base_strips_the_query() {
        assert_eq!(url_base("https://cdn/ep1.mp3?utm=x&a=b"), "https://cdn/ep1.mp3");
        assert_eq!(url_base("https://cdn/ep1.mp3"), "https://cdn/ep1.mp3");
    }
}
