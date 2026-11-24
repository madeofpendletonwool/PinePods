//! Translation context shared by every component.
//!
//! Replaces `i18nrs::yew::{I18nProvider, use_translation}`, whose context held the `I18n`
//! by value: each `use_translation()` call cloned (and later dropped) the parsed
//! translations for every language, on every render of every component. That was most of
//! the cost of rendering an episode card. Here the `I18n` sits behind an `Rc`, so the hook
//! is a pointer copy. The API (`let (i18n, set_language) = use_translation()`, `i18n.t(..)`)
//! is unchanged.

use i18nrs::{I18n, I18nConfig, StorageType};
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use yew::prelude::*;

/// Where the chosen language is remembered (the same key i18nrs used).
const STORAGE_KEY: &str = "i18nrs";

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
    pub translations: HashMap<&'static str, &'static str>,
    #[prop_or("en".to_string())]
    pub default_language: String,
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

fn stored_language() -> Option<String> {
    web_sys::window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item(STORAGE_KEY).ok().flatten())
}

#[function_component(TranslationProvider)]
pub fn translation_provider(props: &TranslationProviderProps) -> Html {
    // Parsed once; re-renders of the provider don't re-parse every language file.
    let ctx = use_state(|| {
        let mut i18n = I18n::new(
            I18nConfig { translations: props.translations.clone() },
            props.translations.clone(),
            props.default_language.clone(),
        )
        .unwrap_or_else(|err| panic!("Failed to initialize I18n: {err}"));
        let language = stored_language().unwrap_or_else(|| props.default_language.clone());
        if let Err(err) = i18n.set_translation_language(&language, &StorageType::LocalStorage, STORAGE_KEY) {
            web_sys::console::log_1(&format!("i18n error: {err}").into());
        }
        set_text_direction(i18n.get_current_language());
        Translations(Rc::new(i18n))
    });

    // The live value, read by the callback: callers keep the callback across renders (and
    // async gaps), so whatever it captured may be out of date.
    let live = use_mut_ref(|| (*ctx).clone());
    let set_language = {
        let ctx = ctx.clone();
        let live = live.clone();
        use_callback((), move |language: String, _| {
            let current = live.borrow().clone();
            if current.get_current_language() == language {
                return;
            }
            let mut i18n = (*current.0).clone();
            match i18n.set_translation_language(&language, &StorageType::LocalStorage, STORAGE_KEY) {
                Ok(()) => {
                    set_text_direction(&language);
                    let next = Translations(Rc::new(i18n));
                    *live.borrow_mut() = next.clone();
                    ctx.set(next);
                }
                Err(err) => web_sys::console::log_1(&format!("i18n error: {err}").into()),
            }
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
