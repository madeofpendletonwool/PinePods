// Podping relay. Podcast hosts announce feed updates on the Hive blockchain (custom_json
// operations with ids starting "pp_"). This follows the chain through public Hive API nodes and
// keeps the last day of announced feed URLs, so Pinepods servers can ask "what changed since
// N?" with one cheap request and match the URLs against their own subscriptions locally. Nothing
// about a server's subscriptions is sent here.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use actix_web::{web, HttpResponse, Responder};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const DEFAULT_NODES: &str = "https://api.hive.blog,https://api.deathwing.me,https://api.openhive.network";
/// Pings kept for clients to catch up on.
const KEEP_SECS: i64 = 24 * 3600;
const MAX_KEPT: usize = 500_000;
/// Blocks fetched per request (Hive makes one every 3 seconds).
const BATCH: u64 = 50;
const MAX_PAGE: usize = 1_000;

#[derive(Debug, Clone, Serialize)]
pub struct Ping {
    pub seq: u64,
    pub url: String,
    pub reason: String,
    pub medium: String,
    /// Unix seconds of the block it was in.
    pub timestamp: i64,
}

#[derive(Default)]
struct Inner {
    pings: VecDeque<Ping>,
    next_seq: u64,
    last_block: u64,
    last_block_time: i64,
    last_error: Option<String>,
    node: Option<String>,
}

#[derive(Clone)]
pub struct Podping {
    inner: Arc<Mutex<Inner>>,
    enabled: bool,
    /// When this relay started. Sequence numbers restart with it, so a client that sees a new
    /// epoch starts over from `next` (asking with `limit=0` returns just that).
    epoch: i64,
}

impl Podping {
    pub fn from_env() -> Self {
        let enabled = std::env::var("PODPING_ENABLED").map(|v| !matches!(v.trim(), "0" | "false" | "no")).unwrap_or(true);
        Podping { inner: Arc::new(Mutex::new(Inner { next_seq: 1, ..Default::default() })), enabled, epoch: chrono::Utc::now().timestamp() }
    }

    /// Follow the chain in the background (when enabled).
    pub fn start(&self) {
        if !self.enabled {
            info!("Podping relay is off (PODPING_ENABLED)");
            return;
        }
        let this = self.clone();
        actix_web::rt::spawn(async move { this.follow().await });
    }

    async fn follow(self) {
        let nodes: Vec<String> = std::env::var("HIVE_API_NODES")
            .unwrap_or_else(|_| DEFAULT_NODES.to_string())
            .split(',')
            .map(|n| n.trim().trim_end_matches('/').to_string())
            .filter(|n| !n.is_empty())
            .collect();
        let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().expect("http client");
        let mut node = 0usize;
        let mut next_block: Option<u64> = None;
        info!("Podping relay following Hive through {} node(s)", nodes.len());
        loop {
            let url = &nodes[node % nodes.len()];
            match self.step(&client, url, &mut next_block).await {
                Ok(caught_up) => {
                    self.inner.lock().unwrap().node = Some(url.clone());
                    if caught_up {
                        actix_web::rt::time::sleep(Duration::from_secs(3)).await;
                    }
                }
                Err(e) => {
                    warn!("Podping: {} failed: {}", url, e);
                    self.inner.lock().unwrap().last_error = Some(format!("{url}: {e}"));
                    node += 1;
                    actix_web::rt::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Fetch the next batch of blocks. True when there was nothing new yet.
    async fn step(&self, client: &reqwest::Client, node: &str, next_block: &mut Option<u64>) -> Result<bool, String> {
        let head = rpc(client, node, "condenser_api.get_dynamic_global_properties", json!([])).await?["head_block_number"]
            .as_u64()
            .ok_or("no head_block_number")?;
        // Start at the head: history before this relay started isn't needed.
        let start = *next_block.get_or_insert(head);
        if start > head {
            return Ok(true);
        }
        let count = (head - start + 1).min(BATCH);
        let result = rpc(client, node, "block_api.get_block_range", json!({ "starting_block_num": start, "count": count })).await?;
        let blocks = result["blocks"].as_array().ok_or("no blocks")?;
        if blocks.is_empty() {
            return Ok(true);
        }
        let mut found = 0usize;
        for (i, block) in blocks.iter().enumerate() {
            let time = block["timestamp"]
                .as_str()
                .and_then(|t| chrono::NaiveDateTime::parse_from_str(t, "%Y-%m-%dT%H:%M:%S").ok())
                .map(|t| t.and_utc().timestamp())
                .unwrap_or_else(|| chrono::Utc::now().timestamp());
            let pings = block_pings(block);
            found += pings.len();
            self.record(start + i as u64, time, pings);
        }
        *next_block = Some(start + blocks.len() as u64);
        debug!("Podping: blocks {}..{}: {} feed updates", start, start + blocks.len() as u64 - 1, found);
        Ok(false)
    }

    fn record(&self, block: u64, time: i64, pings: Vec<(String, String, String)>) {
        let mut inner = self.inner.lock().unwrap();
        for (url, reason, medium) in pings {
            let seq = inner.next_seq;
            inner.next_seq += 1;
            inner.pings.push_back(Ping { seq, url, reason, medium, timestamp: time });
        }
        let cutoff = time - KEEP_SECS;
        while inner.pings.front().is_some_and(|p| p.timestamp < cutoff) || inner.pings.len() > MAX_KEPT {
            inner.pings.pop_front();
        }
        inner.last_block = block;
        inner.last_block_time = time;
        inner.last_error = None;
    }

    /// Pings after `after`, oldest first, and the cursor to ask with next.
    pub fn since(&self, after: u64, limit: usize) -> Value {
        let (pings, next, gap, more) = {
            let inner = self.inner.lock().unwrap();
            let oldest = inner.pings.front().map(|p| p.seq).unwrap_or(inner.next_seq);
            // Sequence numbers are consecutive, so the first one after `after` is found by position
            // instead of scanning the day of pings kept.
            let start = ((after + 1).saturating_sub(oldest) as usize).min(inner.pings.len());
            let pings: Vec<Ping> = inner.pings.range(start..).take(limit).cloned().collect();
            let next = pings.last().map(|p| p.seq).unwrap_or_else(|| after.max(inner.next_seq.saturating_sub(1)));
            // The client asked for pings this relay no longer has (or never saw): it missed some.
            (pings, next, after > 0 && after + 1 < oldest, inner.pings.back().is_some_and(|p| p.seq > next))
        };
        json!({ "epoch": self.epoch, "pings": pings, "next": next, "gap": gap, "more": more })
    }

    pub fn status(&self) -> Value {
        let inner = self.inner.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        json!({
            "epoch": self.epoch,
            "enabled": self.enabled,
            "following": self.enabled && inner.last_block > 0 && now - inner.last_block_time < 120,
            "last_block": inner.last_block,
            "last_block_time": (inner.last_block > 0).then_some(inner.last_block_time),
            "pings_kept": inner.pings.len(),
            "next": inner.next_seq.saturating_sub(1),
            "node": inner.node,
            "last_error": inner.last_error,
        })
    }
}

async fn rpc(client: &reqwest::Client, node: &str, method: &str, params: Value) -> Result<Value, String> {
    let body = json!({ "jsonrpc": "2.0", "method": method, "params": params, "id": 1 });
    let resp = client.post(node).json(&body).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let mut v: Value = resp.json().await.map_err(|e| e.to_string())?;
    if let Some(err) = v.get("error") {
        return Err(err.to_string());
    }
    Ok(v["result"].take())
}

/// (url, reason, medium) of every feed a block's podpings announce. Accepts the current
/// (`iris`) and the original (`urls`/`url`) formats.
pub fn block_pings(block: &Value) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for tx in block["transactions"].as_array().into_iter().flatten() {
        for op in tx["operations"].as_array().into_iter().flatten() {
            if op["type"] != "custom_json_operation" {
                continue;
            }
            let value = &op["value"];
            if !value["id"].as_str().is_some_and(|id| id.starts_with("pp_")) {
                continue;
            }
            let Some(payload) = value["json"].as_str().and_then(|j| serde_json::from_str::<Value>(j).ok()) else { continue };
            let reason = payload["reason"].as_str().unwrap_or("update").to_string();
            let medium = payload["medium"].as_str().unwrap_or("podcast").to_string();
            let urls = payload["iris"].as_array().or_else(|| payload["urls"].as_array()).cloned().unwrap_or_default();
            let single = payload["url"].as_str().map(|u| Value::String(u.to_string()));
            for url in urls.into_iter().chain(single) {
                if let Some(url) = url.as_str().map(str::trim).filter(|u| u.starts_with("http://") || u.starts_with("https://")) {
                    out.push((url.to_string(), reason.clone(), medium.clone()));
                }
            }
        }
    }
    out
}

#[derive(Deserialize)]
pub struct UpdatesQuery {
    #[serde(default)]
    after: u64,
    limit: Option<usize>,
}

/// GET /api/podping/updates?after=<seq>: feed URLs announced since `after`.
pub async fn updates_handler(podping: web::Data<Podping>, query: web::Query<UpdatesQuery>) -> impl Responder {
    if !podping.enabled {
        return HttpResponse::NotFound().json(json!({ "error": "podping relay is off" }));
    }
    // Servers that are caught up all ask for the same URL, so a cache in front can answer them.
    HttpResponse::Ok()
        .insert_header(("Cache-Control", "public, max-age=10"))
        .json(podping.since(query.after, query.limit.unwrap_or(MAX_PAGE).min(MAX_PAGE)))
}

/// GET /api/podping/status
pub async fn status_handler(podping: web::Data<Podping>) -> impl Responder {
    HttpResponse::Ok().json(podping.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn op(id: &str, payload: Value) -> Value {
        json!({ "type": "custom_json_operation", "value": { "id": id, "required_posting_auths": ["podping.aaa"], "json": payload.to_string() } })
    }

    #[test]
    fn reads_both_podping_formats_and_skips_other_ops() {
        let block = json!({ "transactions": [
            { "operations": [
                op("pp_podcast_update", json!({ "version": "1.1", "medium": "podcast", "reason": "update", "iris": ["https://a.example/feed", "ftp://nope"] })),
                op("pp_podcast_live", json!({ "version": "1.1", "medium": "music", "reason": "live", "iris": ["https://b.example/feed"] })),
                op("podping", json!({ "version": "0.3", "urls": ["https://c.example/feed"] })),
                op("not_podping", json!({ "iris": ["https://d.example/feed"] })),
                { "type": "vote_operation", "value": {} },
            ]},
            { "operations": [ op("pp_old", json!({ "url": "https://e.example/feed" })) ] },
        ]});
        let urls: Vec<String> = block_pings(&block).into_iter().map(|p| p.0).collect();
        assert_eq!(urls, ["https://a.example/feed", "https://b.example/feed", "https://e.example/feed"]);
        assert_eq!(block_pings(&block)[1].1, "live");
    }

    #[test]
    fn clients_page_through_and_notice_gaps() {
        let p = Podping { inner: Arc::new(Mutex::new(Inner { next_seq: 1, ..Default::default() })), enabled: true, epoch: 1 };
        let now = chrono::Utc::now().timestamp();
        p.record(10, now, (0..5).map(|i| (format!("https://f{i}.example/feed"), "update".into(), "podcast".into())).collect());
        let first = p.since(0, 3);
        assert_eq!(first["pings"].as_array().unwrap().len(), 3);
        assert_eq!((first["next"].as_u64(), first["more"].as_bool(), first["gap"].as_bool()), (Some(3), Some(true), Some(false)));
        let rest = p.since(3, 100);
        assert_eq!((rest["pings"].as_array().unwrap().len(), rest["next"].as_u64(), rest["more"].as_bool()), (2, Some(5), Some(false)));
        assert_eq!(p.since(5, 100)["pings"], json!([]));
        assert_eq!(p.since(5, 100)["next"].as_u64(), Some(5));
        // Joining: no pings, just the cursor.
        assert_eq!((p.since(0, 0)["pings"].as_array().unwrap().len(), p.since(0, 0)["next"].as_u64()), (0, Some(5)));

        // Old pings age out; a client behind them hears about the gap.
        p.record(11, now + KEEP_SECS + 10, vec![("https://g.example/feed".into(), "update".into(), "podcast".into())]);
        assert_eq!(p.since(2, 100)["gap"].as_bool(), Some(true));
    }
}
