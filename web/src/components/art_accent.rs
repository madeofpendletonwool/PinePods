//! Accent colours pulled from episode artwork, used for the play ring, podcast name and
//! other card details. Extraction runs in the browser through the server's image proxy
//! (which allows cross-origin reads), one image at a time, and results are kept in
//! localStorage so each artwork is only analysed once per browser.

use crate::components::context::AppState;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use yew::prelude::*;
use yewdux::prelude::*;

#[wasm_bindgen(inline_js = r#"
const KEY = 'pinepods_art_colors', MAX = 800, SIZE = 40;
let saved = null, dirty = false, writeTimer = null, tail = Promise.resolve();
const pending = new Map();

function store() {
    if (saved) return saved;
    try { saved = JSON.parse(localStorage.getItem(KEY) || '{}') || {}; } catch (e) { saved = {}; }
    return saved;
}
function remember(url, value) {
    const s = store();
    delete s[url];
    s[url] = value;
    if (writeTimer) return;
    writeTimer = setTimeout(() => {
        writeTimer = null;
        try {
            const keys = Object.keys(s);
            for (const k of keys.slice(0, Math.max(0, keys.length - MAX))) delete s[k];
            localStorage.setItem(KEY, JSON.stringify(s));
        } catch (e) {}
    }, 1500);
}

function rgbToHsl(r, g, b) {
    r /= 255; g /= 255; b /= 255;
    const max = Math.max(r, g, b), min = Math.min(r, g, b), l = (max + min) / 2;
    if (max === min) return [0, 0, l];
    const d = max - min;
    const s = l > 0.5 ? d / (2 - max - min) : d / (max + min);
    let h;
    if (max === r) h = (g - b) / d + (g < b ? 6 : 0);
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    return [h * 60, s, l];
}
function hslToRgb(h, s, l) {
    const k = (n) => (n + h / 30) % 12, a = s * Math.min(l, 1 - l);
    const f = (n) => l - a * Math.max(-1, Math.min(k(n) - 3, Math.min(9 - k(n), 1)));
    return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255)];
}

// The most prominent saturated hue, normalised to a mid tone so CSS can push it lighter
// or darker for the theme. Empty string when the artwork has no real colour.
function pick(data) {
    const bins = new Array(24).fill(null).map(() => ({ w: 0, r: 0, g: 0, b: 0 }));
    for (let i = 0; i < data.length; i += 4) {
        if (data[i + 3] < 200) continue;
        const r = data[i], g = data[i + 1], b = data[i + 2];
        const [h, s, l] = rgbToHsl(r, g, b);
        if (s < 0.22 || l < 0.12 || l > 0.92) continue;
        const w = s * (1 - Math.abs(l - 0.5) * 1.4);
        const bin = bins[Math.floor(h / 15) % 24];
        bin.w += w; bin.r += r * w; bin.g += g * w; bin.b += b * w;
    }
    let best = null, total = 0;
    for (const bin of bins) { total += bin.w; if (!best || bin.w > best.w) best = bin; }
    // Ignore a few stray coloured pixels on otherwise grey artwork.
    if (!best || best.w < (data.length / 4) * 0.02) return '';
    const [h, s] = rgbToHsl(best.r / best.w, best.g / best.w, best.b / best.w);
    const [r, g, b] = hslToRgb(h, Math.min(0.85, Math.max(0.5, s)), 0.5);
    return `${r} ${g} ${b}`;
}

function extract(src) {
    return new Promise((resolve) => {
        const img = new Image();
        img.crossOrigin = 'anonymous';
        img.decoding = 'async';
        const timer = setTimeout(() => resolve(null), 10000);
        img.onload = () => {
            clearTimeout(timer);
            try {
                const canvas = document.createElement('canvas');
                canvas.width = SIZE; canvas.height = SIZE;
                const ctx = canvas.getContext('2d', { willReadFrequently: true });
                ctx.drawImage(img, 0, 0, SIZE, SIZE);
                resolve(pick(ctx.getImageData(0, 0, SIZE, SIZE).data));
            } catch (e) { resolve(null); }
        };
        img.onerror = () => { clearTimeout(timer); resolve(null); };
        img.src = src;
    });
}

// "r g b", "" for artwork without a usable colour, or undefined when not analysed yet.
export function artColorCached(url) {
    const v = store()[url];
    return typeof v === 'string' ? v : undefined;
}

export function artColorResolve(url, proxied) {
    const known = artColorCached(url);
    if (known !== undefined) return Promise.resolve(known);
    if (pending.has(url)) return pending.get(url);
    const job = tail.then(() => extract(proxied)).then((color) => {
        pending.delete(url);
        // A failed load isn't remembered, so it's retried on a later visit.
        if (color !== null) remember(url, color);
        return color || '';
    });
    tail = job.catch(() => {});
    pending.set(url, job);
    return job;
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = artColorCached)]
    fn js_cached(url: &str) -> JsValue;
    #[wasm_bindgen(js_name = artColorResolve)]
    async fn js_resolve(url: &str, proxied: &str) -> JsValue;
}

const SETTING_KEY: &str = "pinepods_art_accents";

/// Per-browser appearance choices for episode cards.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Store)]
pub struct CardAppearance {
    /// Tint card details with colours from the episode artwork.
    pub art_accents: bool,
}

impl Default for CardAppearance {
    fn default() -> Self {
        let disabled = web_sys::window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item(SETTING_KEY).ok().flatten())
            .map_or(false, |v| v == "off");
        Self { art_accents: !disabled }
    }
}

impl CardAppearance {
    pub fn set_art_accents(enabled: bool) {
        if let Some(storage) = web_sys::window().and_then(|w| w.local_storage().ok().flatten()) {
            let _ = storage.set_item(SETTING_KEY, if enabled { "on" } else { "off" });
        }
        Dispatch::<CardAppearance>::global().reduce_mut(|s| s.art_accents = enabled);
    }

    /// The localStorage key, so logout can keep this preference.
    pub const STORAGE_KEY: &'static str = SETTING_KEY;
}

fn cached(url: &str) -> Option<Option<String>> {
    js_cached(url).as_string().map(|c| (!c.is_empty()).then_some(c))
}

/// The artwork's accent as `"r g b"` (for `--art-rgb`), or None when disabled, still
/// loading, or the artwork has no usable colour.
#[hook]
pub fn use_art_accent(art_url: &str) -> Option<String> {
    let enabled = *use_selector(|s: &CardAppearance| s.art_accents);
    let server = use_selector(|s: &AppState| {
        s.auth_details.as_ref().map(|a| a.server_name.clone()).unwrap_or_default()
    });
    let url = art_url.to_string();
    let color = {
        let url = url.clone();
        use_state(move || if enabled && !url.is_empty() { cached(&url).flatten() } else { None })
    };

    {
        let color = color.clone();
        let server: Rc<String> = server.clone();
        use_effect_with((url, enabled), move |(url, enabled)| {
            let alive = Rc::new(std::cell::Cell::new(true));
            if !*enabled || url.is_empty() {
                color.set(None);
            } else if let Some(known) = cached(url) {
                color.set(known);
            } else {
                let url = url.clone();
                let proxied = format!(
                    "{}/api/proxy/image?url={}",
                    server,
                    urlencoding::encode(&url)
                );
                let alive = alive.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let value = js_resolve(&url, &proxied).await.as_string().unwrap_or_default();
                    if alive.get() {
                        color.set((!value.is_empty()).then_some(value));
                    }
                });
            }
            move || alive.set(false)
        });
    }

    if enabled { (*color).clone() } else { None }
}

/// Inline style carrying the accent for CSS (`--art-rgb`), or empty.
pub fn accent_style(accent: &Option<String>) -> String {
    accent.as_ref().map(|c| format!("--art-rgb: {c};")).unwrap_or_default()
}
