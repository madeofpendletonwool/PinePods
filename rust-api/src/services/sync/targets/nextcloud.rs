// The Nextcloud gpoddersync app, at the paths AntennaPod's NextcloudSyncService uses. Basic
// auth on every request (the login flow's app password); the app has no devices.

use async_trait::async_trait;
use reqwest::Method;
use serde_json::json;

use super::{pulled_actions, sub_changes, Http, PulledActions, SubChanges, SyncTarget};
use crate::error::AppResult;
use crate::services::sync::model::OutboundAction;

pub const PUSH_BATCH: usize = 100;
const APP: [&str; 3] = ["index.php", "apps", "gpoddersync"];

pub struct NextcloudTarget {
    http: Http,
}

impl NextcloudTarget {
    pub fn new(url: &str, username: &str, password: &str) -> AppResult<Self> {
        // The app paths start with index.php; a pasted ".../index.php" would double it.
        let base = url.trim().trim_end_matches('/');
        let base = base.strip_suffix("/index.php").unwrap_or(base);
        Ok(NextcloudTarget { http: Http::new(base, username, password)? })
    }

    fn url(&self, path: &[&str], since: Option<i64>) -> reqwest::Url {
        let segments: Vec<&str> = APP.iter().chain(path).copied().collect();
        let query: Vec<(&str, String)> = since.map(|s| ("since", s.to_string())).into_iter().collect();
        self.http.url(&segments, &query)
    }
}

#[async_trait]
impl SyncTarget for NextcloudTarget {
    async fn login(&mut self) -> AppResult<()> {
        Ok(())
    }

    async fn pull_subscriptions(&self, since: i64) -> AppResult<SubChanges> {
        let url = self.url(&["subscriptions"], Some(since));
        let what = format!("GET {}", url.path());
        sub_changes(&self.http.send(Method::GET, url, None).await?, &what)
    }

    async fn push_subscriptions(&self, add: &[String], remove: &[String]) -> AppResult<()> {
        let url = self.url(&["subscription_change", "create"], None);
        self.http.send(Method::POST, url, Some(&json!({ "add": add, "remove": remove }))).await?;
        Ok(())
    }

    async fn pull_actions(&self, since: i64) -> AppResult<PulledActions> {
        let url = self.url(&["episode_action"], Some(since));
        let what = format!("GET {}", url.path());
        pulled_actions(&self.http.send(Method::GET, url, None).await?, &what)
    }

    async fn push_actions(&self, batch: &[OutboundAction]) -> AppResult<()> {
        let url = self.url(&["episode_action", "create"], None);
        self.http.send(Method::POST, url, Some(&serde_json::to_value(batch)?)).await?;
        Ok(())
    }

    fn push_batch_size(&self) -> usize {
        PUSH_BATCH
    }
}
