use web_sys::{window, HtmlElement};
use wasm_bindgen::JsCast;
use anyhow::Error;
use serde_json::Value;
use std::collections::HashMap;
use serde::de::value::MapAccessDeserializer;
use serde::de::MapAccess;
use std::fmt;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, de};
use std::marker::PhantomData;

pub fn deserialize_with_lowercase<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct CustomVisitor<T>(PhantomData<T>);

    impl<'de, T> Visitor<'de> for CustomVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = T;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a value that can be deserialized into the expected type")
        }

        fn visit_map<V>(self, mut map: V) -> Result<T, V::Error>
        where
            V: de::MapAccess<'de>,
        {
            let mut map_data = HashMap::new();
            while let Some((key, value)) = map.next_entry::<String, Value>()? {
                map_data.insert(key.to_lowercase(), value);
            }

            let json_value = serde_json::to_value(map_data).map_err(de::Error::custom)?;
            T::deserialize(json_value).map_err(de::Error::custom)
        }
    }

    deserializer.deserialize_map(CustomVisitor(PhantomData))
}

#[allow(dead_code)]
pub fn change_theme(theme: &str) {
    if let Some(window) = window() {
        if let Some(document) = window.document() {
            if let Some(root) = document.document_element() {
                if let Ok(html_element) = root.dyn_into::<HtmlElement>() {
                    let style = html_element.style();
                    match theme {
                        "light" => {
                            style.set_property("--button-color", "#new-dark-color").unwrap();
                            style.set_property("--background-color", "#new-dark-bg").unwrap();
                        },
                        "dark" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        "nordic" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        "abyss" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        "dracula" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        "kimbie" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        "hotdogstand - MY EYES" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        "neon" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        "wildberries" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        "greenie meanie" => {
                            style.set_property("--button-color", "#new-light-color").unwrap();
                            style.set_property("--background-color", "#new-light-bg").unwrap();
                        },
                        _ => {}
                    }
                }
            }
        }
    }
}