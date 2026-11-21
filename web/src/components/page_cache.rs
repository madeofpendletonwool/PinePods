//! Last-seen page data, so Home / Feed / Podcasts render instantly and refresh in the
//! background (stale-while-revalidate).
//!
//! Two layers: an in-memory map that survives client-side navigation, and IndexedDB that
//! survives reloads and app relaunches. Keys include the server and user, so switching
//! accounts never shows someone else's data; logout clears both layers.

use serde::de::DeserializeOwned;
use serde::Serialize;
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(inline_js = r#"
const DB = 'pinepods-page-cache', STORE = 'pages';
let dbPromise = null;
function db() {
    if (!dbPromise) {
        dbPromise = new Promise((resolve, reject) => {
            const req = indexedDB.open(DB, 1);
            req.onupgradeneeded = () => req.result.createObjectStore(STORE);
            req.onsuccess = () => resolve(req.result);
            req.onerror = () => reject(req.error);
        });
        dbPromise.catch(() => { dbPromise = null; });
    }
    return dbPromise;
}
export async function pageCacheGet(key) {
    try {
        const d = await db();
        return await new Promise((resolve) => {
            const req = d.transaction(STORE, 'readonly').objectStore(STORE).get(key);
            req.onsuccess = () => resolve(typeof req.result === 'string' ? req.result : null);
            req.onerror = () => resolve(null);
        });
    } catch (e) { return null; }
}
export async function pageCachePut(key, value) {
    try {
        const d = await db();
        d.transaction(STORE, 'readwrite').objectStore(STORE).put(value, key);
    } catch (e) {}
}
export async function pageCacheClear() {
    try {
        const d = await db();
        await new Promise((resolve) => {
            const tx = d.transaction(STORE, 'readwrite');
            tx.objectStore(STORE).clear();
            tx.oncomplete = resolve;
            tx.onerror = resolve;
            tx.onabort = resolve;
        });
    } catch (e) {}
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = pageCacheGet)]
    async fn js_get(key: &str) -> JsValue;
    #[wasm_bindgen(js_name = pageCachePut)]
    async fn js_put(key: &str, value: &str);
    #[wasm_bindgen(js_name = pageCacheClear)]
    async fn js_clear();
}

/// Bump when a cached shape changes incompatibly; old entries are then ignored.
const VERSION: u32 = 1;

thread_local! {
    static MEMORY: RefCell<HashMap<String, Rc<dyn Any>>> = RefCell::new(HashMap::new());
}

/// Cache key for one page of one user on one server.
pub fn key(page: &str, server: &str, user_id: i32) -> String {
    format!("v{VERSION}|{server}|{user_id}|{page}")
}

/// The value from this session, if the page was already visited.
pub fn memory<T: Clone + 'static>(key: &str) -> Option<T> {
    MEMORY.with(|m| m.borrow().get(key).and_then(|v| v.downcast_ref::<T>().cloned()))
}

/// The value saved by an earlier session. Unreadable or outdated entries count as missing.
pub async fn stored<T: DeserializeOwned + Clone + 'static>(key: &str) -> Option<T> {
    let text = js_get(key).await.as_string()?;
    let value: T = serde_json::from_str(&text).ok()?;
    MEMORY.with(|m| m.borrow_mut().insert(key.to_string(), Rc::new(value.clone())));
    Some(value)
}

/// Remember a fresh response in memory and in IndexedDB.
pub fn put<T: Serialize + Clone + 'static>(key: &str, value: &T) {
    MEMORY.with(|m| m.borrow_mut().insert(key.to_string(), Rc::new(value.clone())));
    if let Ok(text) = serde_json::to_string(value) {
        let key = key.to_string();
        wasm_bindgen_futures::spawn_local(async move { js_put(&key, &text).await });
    }
}

/// Forget everything (logout).
pub async fn clear() {
    MEMORY.with(|m| m.borrow_mut().clear());
    js_clear().await;
}
