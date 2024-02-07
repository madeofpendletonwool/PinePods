use web_sys::{window, HtmlElement};
use wasm_bindgen::JsCast;

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