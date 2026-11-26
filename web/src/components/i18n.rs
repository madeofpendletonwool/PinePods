//! Translation context shared by every component.
//!
//! English is built in, as the fallback for missing keys. Other languages are fetched from
//! `/static/translations/<lang>.json` when chosen, so the app doesn't carry every language.
//! The context holds the translations behind an `Rc`, so `use_translation()` is a pointer
//! copy. The API (`let (i18n, set_language) = use_translation()`, `i18n.t(..)`) is unchanged.

use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use yew::prelude::*;

/// Where the chosen language is remembered.
const STORAGE_KEY: &str = "i18nrs";

const ENGLISH: &str = include_str!("../translations/en.json");

thread_local! {
    static ENGLISH_STRINGS: Rc<Value> =
        Rc::new(serde_json::from_str(ENGLISH).expect("en.json is valid JSON"));
    /// Languages fetched this visit.
    static LOADED: RefCell<HashMap<String, Rc<Value>>> = RefCell::new(HashMap::new());
}

/// The strings for one language, with English underneath.
#[derive(Clone)]
pub struct I18n {
    language: String,
    strings: Rc<Value>,
    english: Rc<Value>,
}

impl I18n {
    fn english() -> Self {
        let english = ENGLISH_STRINGS.with(Rc::clone);
        I18n { language: "en".to_string(), strings: english.clone(), english }
    }

    /// The loaded strings for `language`, if it is English or has been fetched.
    fn loaded(language: &str) -> Option<Self> {
        if language == "en" {
            return Some(Self::english());
        }
        let strings = LOADED.with(|l| l.borrow().get(language).cloned())?;
        Some(I18n { language: language.to_string(), strings, english: ENGLISH_STRINGS.with(Rc::clone) })
    }

    pub fn get_current_language(&self) -> &str {
        &self.language
    }

    /// The text for a dotted key such as `"settings.title"`, falling back to English.
    pub fn t(&self, key: &str) -> String {
        let lookup = |json: &Value| {
            key.split('.')
                .try_fold(json, |current, part| current.get(part))
                .filter(|v| !matches!(v, Value::String(s) if s.is_empty()))
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
        };
        lookup(&self.strings)
            .or_else(|| lookup(&self.english))
            .unwrap_or_else(|| format!("Key '{key}' not found for language '{}'", self.language))
    }
}

/// Fetches and keeps a language's strings, unless already loaded.
async fn load(language: &str) -> Result<(), String> {
    if I18n::loaded(language).is_some() {
        return Ok(());
    }
    // Language codes come from the server; keep them to a file name.
    if language.is_empty() || !language.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("invalid language '{language}'"));
    }
    let url = format!("/static/translations/{language}.json");
    let response = gloo_net::http::Request::get(&url).send().await.map_err(|e| e.to_string())?;
    if !response.ok() {
        return Err(format!("{url}: HTTP {}", response.status()));
    }
    let strings: Value = response.json().await.map_err(|e| e.to_string())?;
    LOADED.with(|l| l.borrow_mut().insert(language.to_string(), Rc::new(strings)));
    Ok(())
}

/// Loads the remembered language before the first render, so it doesn't open in English.
pub async fn preload() {
    if let Some(language) = stored_language() {
        if let Err(err) = load(&language).await {
            web_sys::console::log_1(&format!("i18n error: {err}").into());
        }
    }
}

/// A cheap handle to the shared translations. Derefs to [`I18n`].
#[derive(Clone)]
pub struct Translations(Rc<I18n>);

impl PartialEq for Translations {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Deref for Translations {
    type Target = I18n;
    fn deref(&self) -> &I18n {
        &self.0
    }
}

#[derive(Properties, PartialEq)]
pub struct TranslationProviderProps {
    pub children: Html,
}

fn set_text_direction(language: &str) {
    let rtl = matches!(language, "ar" | "he" | "fa" | "ur" | "ps" | "ku" | "sd");
    if let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        let _ = root.set_attribute("dir", if rtl { "rtl" } else { "ltr" });
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|w| w.local_storage().ok().flatten())
}

fn stored_language() -> Option<String> {
    local_storage().and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
}

#[function_component(TranslationProvider)]
pub fn translation_provider(props: &TranslationProviderProps) -> Html {
    let ctx = use_state(|| {
        let i18n = stored_language()
            .and_then(|language| I18n::loaded(&language))
            .unwrap_or_else(I18n::english);
        set_text_direction(&i18n.language);
        Translations(Rc::new(i18n))
    });

    // The language last asked for, read by the callback: callers keep the callback across
    // renders (and async gaps), and a slow fetch must not undo a later choice.
    let wanted = use_mut_ref(|| ctx.language.clone());
    let set_language = {
        let ctx = ctx.clone();
        use_callback((), move |language: String, _| {
            if *wanted.borrow() == language {
                return;
            }
            *wanted.borrow_mut() = language.clone();
            let ctx = ctx.clone();
            let wanted = wanted.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(err) = load(&language).await {
                    web_sys::console::log_1(&format!("i18n error: {err}").into());
                    // Let a later request for the same language try again.
                    if *wanted.borrow() == language {
                        wanted.borrow_mut().clear();
                    }
                    return;
                }
                if *wanted.borrow() != language {
                    return;
                }
                if let Some(i18n) = I18n::loaded(&language) {
                    if let Some(storage) = local_storage() {
                        let _ = storage.set_item(STORAGE_KEY, &language);
                    }
                    set_text_direction(&language);
                    ctx.set(Translations(Rc::new(i18n)));
                }
            });
        })
    };

    html! {
        <ContextProvider<Translations> context={(*ctx).clone()}>
            <ContextProvider<Callback<String>> context={set_language}>
                { props.children.clone() }
            </ContextProvider<Callback<String>>>
        </ContextProvider<Translations>>
    }
}

/// The shared translations and a callback that switches language.
#[hook]
pub fn use_translation() -> (Translations, Callback<String>) {
    let i18n = use_context::<Translations>().expect("No translation context provided");
    let set_language = use_context::<Callback<String>>().expect("No set_language context found");
    (i18n, set_language)
}
