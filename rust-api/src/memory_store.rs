//! An in-process stand-in for Valkey, for a server running as one process: values with expiry,
//! sets and counters. Values are stored as Valkey encodes them, so code reading them back sees
//! the same thing on either.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

/// Stored values beyond this are evicted, soonest-expiring first (cached images are the bulk).
const MAX_BYTES: usize = 256 << 20;

enum Data {
    Bytes(Vec<u8>),
    Set(HashSet<String>),
}

struct Entry {
    data: Data,
    expires: Option<Instant>,
}

impl Entry {
    fn live(&self, now: Instant) -> bool {
        self.expires.is_none_or(|t| t > now)
    }

    fn size(&self, key: &str) -> usize {
        key.len()
            + match &self.data {
                Data::Bytes(b) => b.len(),
                Data::Set(s) => s.iter().map(|m| m.len() + 8).sum(),
            }
    }
}

#[derive(Default)]
struct Inner {
    map: HashMap<String, Entry>,
    bytes: usize,
}

impl Inner {
    /// The key's entry, if it's there and hasn't expired.
    fn live(&mut self, key: &str) -> Option<&mut Entry> {
        let now = Instant::now();
        if self.map.get(key).is_some_and(|e| !e.live(now)) {
            self.remove(key);
        }
        self.map.get_mut(key)
    }

    fn remove(&mut self, key: &str) -> Option<Entry> {
        let entry = self.map.remove(key)?;
        self.bytes -= entry.size(key);
        Some(entry)
    }

    fn insert(&mut self, key: &str, entry: Entry) {
        self.remove(key);
        self.bytes += entry.size(key);
        self.map.insert(key.to_string(), entry);
        if self.bytes > MAX_BYTES {
            self.evict();
        }
    }

    /// Change a set in place, keeping the byte count right.
    fn with_set<R>(&mut self, key: &str, create: bool, f: impl FnOnce(&mut HashSet<String>) -> R) -> Option<R> {
        let entry = match self.live(key) {
            Some(Entry { data: Data::Set(_), .. }) => self.remove(key),
            Some(_) => return None,
            None if create => Some(Entry { data: Data::Set(HashSet::new()), expires: None }),
            None => return None,
        }?;
        let Entry { data: Data::Set(mut set), expires } = entry else { unreachable!() };
        let out = f(&mut set);
        if !set.is_empty() {
            self.insert(key, Entry { data: Data::Set(set), expires });
        }
        Some(out)
    }

    fn sweep(&mut self) {
        let now = Instant::now();
        let dead: Vec<String> = self.map.iter().filter(|(_, e)| !e.live(now)).map(|(k, _)| k.clone()).collect();
        for key in dead {
            self.remove(&key);
        }
    }

    fn evict(&mut self) {
        self.sweep();
        if self.bytes <= MAX_BYTES {
            return;
        }
        let mut expiring: Vec<(Instant, String)> =
            self.map.iter().filter_map(|(k, e)| e.expires.map(|t| (t, k.clone()))).collect();
        expiring.sort();
        for (_, key) in expiring {
            if self.bytes <= MAX_BYTES * 3 / 4 {
                break;
            }
            self.remove(&key);
        }
    }
}

#[derive(Default)]
pub struct MemoryStore {
    inner: Mutex<Inner>,
}

impl MemoryStore {
    /// A store that drops expired entries every minute while it's in use.
    pub fn new() -> Arc<Self> {
        let store = Arc::new(Self::default());
        let weak: Weak<Self> = Arc::downgrade(&store);
        if let Ok(rt) = tokio::runtime::Handle::try_current() {
            rt.spawn(async move {
                let mut tick = tokio::time::interval(Duration::from_secs(60));
                loop {
                    tick.tick().await;
                    match weak.upgrade() {
                        Some(store) => store.lock().sweep(),
                        None => break,
                    }
                }
            });
        }
        store
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        match self.lock().live(key) {
            Some(Entry { data: Data::Bytes(b), .. }) => Some(b.clone()),
            _ => None,
        }
    }

    /// SET, with an optional expiry; `only_new` is NX. Whether it was set.
    pub fn set(&self, key: &str, value: Vec<u8>, ttl: Option<Duration>, only_new: bool) -> bool {
        let mut inner = self.lock();
        if only_new && inner.live(key).is_some() {
            return false;
        }
        inner.insert(key, Entry { data: Data::Bytes(value), expires: ttl.map(|t| Instant::now() + t) });
        true
    }

    pub fn get_del(&self, key: &str) -> Option<Vec<u8>> {
        let mut inner = self.lock();
        match inner.live(key) {
            Some(Entry { data: Data::Bytes(_), .. }) => match inner.remove(key) {
                Some(Entry { data: Data::Bytes(b), .. }) => Some(b),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn del(&self, key: &str) -> bool {
        let mut inner = self.lock();
        inner.live(key).is_some() && inner.remove(key).is_some()
    }

    pub fn exists(&self, key: &str) -> bool {
        self.lock().live(key).is_some()
    }

    pub fn expire(&self, key: &str, ttl: Duration) -> bool {
        match self.lock().live(key) {
            Some(entry) => {
                entry.expires = Some(Instant::now() + ttl);
                true
            }
            None => false,
        }
    }

    /// INCRBY: a missing key counts from 0; the expiry is kept.
    pub fn incr_by(&self, key: &str, by: i64) -> Result<i64, String> {
        let mut inner = self.lock();
        let (current, expires) = match inner.live(key) {
            None => (0, None),
            Some(Entry { data: Data::Bytes(b), expires }) => {
                let n = std::str::from_utf8(b).ok().and_then(|s| s.parse::<i64>().ok());
                (n.ok_or("value is not an integer")?, *expires)
            }
            Some(_) => return Err("value is not an integer".into()),
        };
        let next = current.checked_add(by).ok_or("increment would overflow")?;
        inner.insert(key, Entry { data: Data::Bytes(next.to_string().into_bytes()), expires });
        Ok(next)
    }

    pub fn sadd(&self, key: &str, member: &str) -> bool {
        self.lock().with_set(key, true, |s| s.insert(member.to_string())).unwrap_or(false)
    }

    pub fn srem(&self, key: &str, member: &str) -> bool {
        self.lock().with_set(key, false, |s| s.remove(member)).unwrap_or(false)
    }

    pub fn smembers(&self, key: &str) -> Vec<String> {
        match self.lock().live(key) {
            Some(Entry { data: Data::Set(s), .. }) => s.iter().cloned().collect(),
            _ => Vec::new(),
        }
    }

    #[cfg(test)]
    fn bytes(&self) -> usize {
        self.lock().bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn values_sets_counters_and_expiry() {
        let s = MemoryStore::new();
        assert!(s.set("a", b"1".to_vec(), None, false));
        assert!(!s.set("a", b"2".to_vec(), None, true));
        assert_eq!(s.get("a"), Some(b"1".to_vec()));
        assert_eq!(s.incr_by("a", 4), Ok(5));
        assert_eq!(s.incr_by("n", 1), Ok(1));
        assert_eq!(s.get_del("a"), Some(b"5".to_vec()));
        assert!(!s.exists("a"));

        assert!(s.sadd("set", "x"));
        assert!(!s.sadd("set", "x"));
        s.sadd("set", "y");
        let mut members = s.smembers("set");
        members.sort();
        assert_eq!(members, ["x", "y"]);
        assert!(s.srem("set", "x"));
        assert!(s.srem("set", "y"));
        assert!(!s.exists("set"), "an empty set is gone, as in Valkey");

        s.set("t", b"v".to_vec(), Some(Duration::from_millis(20)), false);
        assert!(s.exists("t"));
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert_eq!(s.get("t"), None);
        assert!(!s.expire("t", Duration::from_secs(1)));

        assert!(s.del("n"));
        assert_eq!(s.bytes(), 0);
    }

    #[tokio::test]
    async fn evicts_soonest_expiring_when_full() {
        let s = MemoryStore::new();
        s.set("keep", b"k".to_vec(), None, false);
        let chunk = vec![0u8; MAX_BYTES / 4];
        for i in 0..6 {
            s.set(&format!("img{i}"), chunk.clone(), Some(Duration::from_secs(100 + i)), false);
        }
        assert!(s.bytes() <= MAX_BYTES);
        assert!(s.exists("keep"));
        assert!(!s.exists("img0"));
        assert!(s.exists("img5"));
    }
}
