// Podping: podcast hosts announce feed updates on the Hive blockchain, and the search service
// relays them. When an admin turns it on, Pinepods asks the relay every minute which feeds
// were announced, matches them against its own subscriptions locally (the relay never learns
// what this server follows) and refreshes the matches straight away.

use std::collections::{HashMap, HashSet};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::database::DatabasePool;
use crate::error::AppResult;
use crate::AppState;

const DEFAULT_INTERVAL: Duration = Duration::from_secs(60);
/// A feed announced again this soon after a Podping refresh waits until the time is up.
const COOLDOWN: Duration = Duration::from_secs(5 * 60);
/// Pages fetched per poll when the relay has a backlog.
const MAX_PAGES: usize = 10;

/// The relay's address: `PINEPODS_PODPING_URL`, else the search service from `SEARCH_API_URL`.
pub fn relay_url() -> String {
    if let Some(url) = std::env::var("PINEPODS_PODPING_URL").ok().map(|u| u.trim().trim_end_matches('/').to_string()).filter(|u| !u.is_empty()) {
        return url;
    }
    search_service_base()
}

/// The search service's base URL, from `SEARCH_API_URL` (".../api/search").
pub fn search_service_base() -> String {
    let search = std::env::var("SEARCH_API_URL").unwrap_or_else(|_| "https://search.pinepods.online/api/search".to_string());
    let search = search.trim().trim_end_matches('/');
    search.strip_suffix("/api/search").unwrap_or(search).to_string()
}

/// What this server's Podping client has done, for the admin settings.
#[derive(Debug, Clone, Default, Serialize, utoipa::ToSchema)]
pub struct ClientStatus {
    /// Unix seconds.
    pub last_poll_at: Option<i64>,
    pub last_success_at: Option<i64>,
    pub last_error: Option<String>,
    /// Announcements seen since this server started.
    pub pings_seen: u64,
    /// Feeds refreshed because of one.
    pub feeds_refreshed: u64,
    pub last_refresh_at: Option<i64>,
}

#[derive(Default)]
struct Client {
    status: ClientStatus,
    /// (relay epoch, last sequence seen); None until joined.
    cursor: Option<(i64, u64)>,
    /// When each feed was last refreshed because of an announcement.
    refreshed_at: HashMap<String, Instant>,
    /// Feeds announced again during their cooldown.
    deferred: HashSet<String>,
}

impl Client {
    /// Of the feeds announced (plus the deferred ones), those to refresh now; the others are
    /// deferred.
    fn due(&mut self, announced: Vec<String>) -> Vec<String> {
        let now = Instant::now();
        self.refreshed_at.retain(|_, at| now.duration_since(*at) < COOLDOWN);
        let mut due = Vec::new();
        for url in announced.into_iter().chain(std::mem::take(&mut self.deferred)) {
            if self.refreshed_at.contains_key(&url) {
                self.deferred.insert(url);
            } else {
                self.refreshed_at.insert(url.clone(), now);
                due.push(url);
            }
        }
        due
    }
}

static CLIENT: LazyLock<Mutex<Client>> = LazyLock::new(|| Mutex::new(Client::default()));

pub fn client_status() -> ClientStatus {
    CLIENT.lock().unwrap().status.clone()
}

#[derive(Debug, Deserialize)]
struct Ping {
    url: String,
}

#[derive(Debug, Deserialize)]
struct Updates {
    epoch: i64,
    pings: Vec<Ping>,
    next: u64,
    #[serde(default)]
    more: bool,
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Poll the relay in the background for as long as the server runs.
pub fn init(state: AppState) {
    let interval = std::env::var("PINEPODS_PODPING_INTERVAL")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_INTERVAL);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(interval).await;
            match state.db_pool.podping_enabled().await {
                Ok(true) => {
                    if let Err(e) = poll(&state).await {
                        debug!("Podping poll failed: {}", e);
                    }
                }
                Ok(false) => CLIENT.lock().unwrap().cursor = None,
                Err(e) => debug!("Reading the Podping setting failed: {}", e),
            }
        }
    });
}

/// One poll: join the relay if needed, read what was announced since, refresh the matches.
pub async fn poll(state: &AppState) -> AppResult<usize> {
    poll_relay(state, &relay_url()).await
}

/// `poll` against the relay at `base`. Returns how many feeds were refreshed.
pub async fn poll_relay(state: &AppState, base: &str) -> AppResult<usize> {
    let client = crate::services::http::service_client();
    let mut refreshed = 0;
    for _ in 0..MAX_PAGES {
        let cursor = CLIENT.lock().unwrap().cursor;
        // Joining (or the relay restarted): start from its current position.
        let url = match cursor {
            Some((_, after)) => format!("{base}/api/podping/updates?after={after}"),
            None => format!("{base}/api/podping/updates?limit=0"),
        };
        CLIENT.lock().unwrap().status.last_poll_at = Some(now());
        let result: Result<Updates, String> = async {
            let resp = client.get(&url).timeout(Duration::from_secs(20)).send().await.map_err(|e| e.to_string())?;
            if !resp.status().is_success() {
                return Err(format!("the relay answered HTTP {}", resp.status().as_u16()));
            }
            resp.json::<Updates>().await.map_err(|e| format!("unexpected answer from the relay: {e}"))
        }
        .await;
        let updates = match result {
            Ok(u) => u,
            Err(e) => {
                let mut c = CLIENT.lock().unwrap();
                if c.status.last_error.as_deref() != Some(e.as_str()) {
                    warn!("Podping: asking {} failed: {}", base, e);
                }
                c.status.last_error = Some(e);
                return Ok(refreshed);
            }
        };
        let rejoin = cursor.is_some_and(|(epoch, _)| epoch != updates.epoch);
        {
            let mut c = CLIENT.lock().unwrap();
            c.status.last_success_at = Some(now());
            c.status.last_error = None;
            if rejoin {
                c.cursor = None;
            } else {
                c.cursor = Some((updates.epoch, updates.next));
                if cursor.is_some() {
                    c.status.pings_seen += updates.pings.len() as u64;
                }
            }
        }
        if cursor.is_none() {
            info!("Podping: following {} from announcement {}", base, updates.next);
            return Ok(refreshed);
        }
        if rejoin {
            info!("Podping: the relay restarted; following it again");
            continue;
        }
        let urls: Vec<String> = updates.pings.into_iter().map(|p| p.url).collect();
        let feeds = state.db_pool.subscribed_feeds_among(&urls).await?;
        let feeds = CLIENT.lock().unwrap().due(feeds);
        if !feeds.is_empty() {
            debug!("Podping: refreshing {} announced feeds", feeds.len());
            let n = crate::handlers::refresh::refresh_feed_urls(state, &feeds).await?;
            refreshed += n;
            let mut c = CLIENT.lock().unwrap();
            c.status.feeds_refreshed += n as u64;
            c.status.last_refresh_at = Some(now());
        }
        if !updates.more {
            break;
        }
    }
    Ok(refreshed)
}

/// The ways one feed URL may be written: as announced, other scheme, with or without a trailing
/// slash.
pub fn url_variants(url: &str) -> Vec<String> {
    let url = url.trim();
    let mut out = vec![url.to_string()];
    let other = if let Some(rest) = url.strip_prefix("https://") {
        format!("http://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        return out;
    };
    out.push(other);
    for u in out.clone() {
        out.push(match u.strip_suffix('/') {
            Some(trimmed) => trimmed.to_string(),
            None => format!("{u}/"),
        });
    }
    out
}

impl DatabasePool {
    pub async fn podping_enabled(&self) -> AppResult<bool> {
        Ok(match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query_scalar::<_, bool>(r#"SELECT podpingenabled FROM "AppSettings" ORDER BY appsettingsid LIMIT 1"#)
                    .fetch_optional(pool)
                    .await?
                    .unwrap_or(false)
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query_scalar::<_, i8>("SELECT PodpingEnabled FROM AppSettings ORDER BY AppSettingsID LIMIT 1")
                    .fetch_optional(pool)
                    .await?
                    .is_some_and(|v| v != 0)
            }
        })
    }

    pub async fn set_podping_enabled(&self, enabled: bool) -> AppResult<()> {
        match self {
            DatabasePool::Postgres(pool) => {
                sqlx::query(r#"UPDATE "AppSettings" SET podpingenabled = $1"#).bind(enabled).execute(pool).await?;
            }
            DatabasePool::MySQL(pool) => {
                sqlx::query("UPDATE AppSettings SET PodpingEnabled = ?").bind(enabled).execute(pool).await?;
            }
        }
        Ok(())
    }

    /// The stored feed URLs (any user) that match one of `urls`, allowing for scheme and
    /// trailing-slash differences.
    pub async fn subscribed_feeds_among(&self, urls: &[String]) -> AppResult<Vec<String>> {
        let wanted: Vec<String> = urls.iter().flat_map(|u| url_variants(u)).collect::<HashSet<_>>().into_iter().collect();
        let mut found = HashSet::new();
        for chunk in wanted.chunks(500) {
            let rows: Vec<String> = match self {
                DatabasePool::Postgres(pool) => sqlx::query_scalar(r#"SELECT DISTINCT feedurl FROM "Podcasts" WHERE feedurl = ANY($1)"#)
                    .bind(chunk)
                    .fetch_all(pool)
                    .await?,
                DatabasePool::MySQL(pool) => {
                    let sql = format!("SELECT DISTINCT FeedURL FROM Podcasts WHERE FeedURL IN ({})", vec!["?"; chunk.len()].join(", "));
                    let mut q = sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str()));
                    for u in chunk {
                        q = q.bind(u);
                    }
                    q.fetch_all(pool).await?
                }
            };
            found.extend(rows);
        }
        Ok(found.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variants_cover_scheme_and_trailing_slash() {
        let v = url_variants("https://feeds.example/show");
        for u in ["https://feeds.example/show", "http://feeds.example/show", "https://feeds.example/show/", "http://feeds.example/show/"] {
            assert!(v.contains(&u.to_string()), "{u} in {v:?}");
        }
        assert_eq!(url_variants("ftp://x"), vec!["ftp://x".to_string()]);
    }

    #[test]
    fn feeds_announced_again_wait_for_the_cooldown() {
        let mut c = Client::default();
        assert_eq!(c.due(vec!["a".into(), "b".into()]), ["a", "b"]);
        assert!(c.due(vec!["a".into()]).is_empty());
        assert!(c.deferred.contains("a"));
        // Once the cooldown is over the deferred feed goes, even without a new announcement.
        c.refreshed_at.insert("a".into(), Instant::now() - COOLDOWN);
        assert_eq!(c.due(vec![]), ["a"]);
        assert!(c.deferred.is_empty());
    }
}
