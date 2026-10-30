// Log compaction. The subscription and episode-action logs keep the newest row per item forever
// (a download from 0 still returns the full state); older rows for the same item are deleted once
// they're past the retention window and every external target has pushed them.

use std::collections::HashMap;

use tracing::{debug, info, warn};

use crate::database::DatabasePool;
use crate::error::AppResult;

/// Days a superseded log row is kept. 0 turns compaction off.
pub const RETENTION_ENV: &str = "PINEPODS_SYNC_LOG_RETENTION_DAYS";
const DEFAULT_RETENTION_DAYS: i64 = 90;

pub fn retention_days() -> i64 {
    std::env::var(RETENTION_ENV).ok().and_then(|v| v.trim().parse::<i64>().ok()).filter(|d| *d >= 0).unwrap_or(DEFAULT_RETENTION_DAYS)
}

/// One log row as compaction sees it: what item it's about, and when it arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRow {
    pub id: i64,
    pub key: String,
    pub server_ts: i64,
}

/// Rows to delete: older than `cutoff`, at or below `pushed` (the lowest push cursor of the user's
/// targets; `None` without targets), and not the newest row for their key.
pub fn superseded(rows: &[LogRow], cutoff: i64, pushed: Option<i64>) -> Vec<i64> {
    let mut newest: HashMap<&str, i64> = HashMap::new();
    for r in rows {
        let id = newest.entry(r.key.as_str()).or_insert(r.id);
        *id = (*id).max(r.id);
    }
    rows.iter()
        .filter(|r| r.server_ts < cutoff && pushed.is_none_or(|p| r.id <= p) && newest[r.key.as_str()] != r.id)
        .map(|r| r.id)
        .collect()
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CompactSummary {
    pub subscriptions: u64,
    pub actions: u64,
}

/// Compact every user's logs. Failures for one user are logged and don't stop the others.
pub async fn run(db: &DatabasePool) -> AppResult<CompactSummary> {
    let days = retention_days();
    if days == 0 {
        debug!("Sync log compaction is off ({RETENTION_ENV}=0)");
        return Ok(CompactSummary::default());
    }
    let cutoff = chrono::Utc::now().timestamp() - days * 86_400;
    let mut total = CompactSummary::default();
    for user_id in db.sync_log_users_before(cutoff).await? {
        match db.sync_compact_user(user_id, cutoff).await {
            Ok(s) => {
                total.subscriptions += s.subscriptions;
                total.actions += s.actions;
            }
            Err(e) => warn!("Compacting the sync log of user {} failed: {}", user_id, e),
        }
    }
    if total != CompactSummary::default() {
        info!("Sync log compaction removed {} subscription and {} episode action rows older than {} days", total.subscriptions, total.actions, days);
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, key: &str, ts: i64) -> LogRow {
        LogRow { id, key: key.into(), server_ts: ts }
    }

    #[test]
    fn keeps_the_newest_row_per_item() {
        let rows = [row(1, "a", 10), row(2, "a", 20), row(3, "b", 10), row(4, "a", 30)];
        assert_eq!(superseded(&rows, 100, None), vec![1, 2]);
        // Recent rows stay even when superseded.
        assert_eq!(superseded(&rows, 15, None), vec![1]);
    }

    #[test]
    fn never_deletes_rows_a_target_has_not_pushed() {
        let rows = [row(1, "a", 10), row(2, "a", 20), row(3, "a", 30)];
        assert_eq!(superseded(&rows, 100, Some(1)), vec![1]);
        assert_eq!(superseded(&rows, 100, Some(0)), Vec::<i64>::new());
    }
}
