use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CACHE: RefCell<HashMap<String, (f64, String)>> = RefCell::new(HashMap::new());
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

pub fn get(key: &str, ttl_ms: f64) -> Option<String> {
    CACHE.with(|c| {
        let c = c.borrow();
        if let Some((ts, val)) = c.get(key) {
            if now_ms() - ts < ttl_ms {
                return Some(val.clone());
            }
        }
        None
    })
}

pub fn set(key: String, value: String) {
    CACHE.with(|c| {
        c.borrow_mut().insert(key, (now_ms(), value));
    });
}

pub fn invalidate_prefix(prefix: &str) {
    CACHE.with(|c| {
        c.borrow_mut().retain(|k, _| !k.starts_with(prefix));
    });
}
