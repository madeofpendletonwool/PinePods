//! The language list the login page and settings read from the deployed translation files.

use crate::harness::*;

#[tokio::test(flavor = "multi_thread")]
async fn languages_come_from_the_translation_files_with_english_first() {
    let app = TestApp::new().await;
    let body = app.get("/api/data/get_available_languages").send().await.ok_json();
    let languages = body["languages"].as_array().unwrap();

    assert_eq!(languages[0]["code"], "en");
    assert_eq!(languages[0]["name"], "English");
    let codes: Vec<&str> = languages.iter().map(|l| l["code"].as_str().unwrap()).collect();
    assert!(codes.contains(&"de"), "{codes:?}");
    let german = languages.iter().find(|l| l["code"] == "de").unwrap();
    assert_eq!(german["name"], "Deutsch");
    let mut rest = codes[1..].to_vec();
    rest.sort();
    assert_eq!(rest, codes[1..], "the others are sorted");
}

#[tokio::test(flavor = "multi_thread")]
async fn broken_or_unrelated_files_are_left_out() {
    let app = TestApp::new().await;
    let body = app.get("/api/data/get_available_languages").send().await.ok_json();
    let codes: Vec<&str> = body["languages"].as_array().unwrap().iter().map(|l| l["code"].as_str().unwrap()).collect();

    assert!(!codes.contains(&"broken"), "invalid JSON isn't a language: {codes:?}");
    assert!(!codes.contains(&"notes"), "only .json files count: {codes:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn every_translation_in_the_repo_has_a_proper_name() {
    let app = TestApp::new().await;
    let body = app.get("/api/data/get_available_languages").send().await.ok_json();
    for language in body["languages"].as_array().unwrap() {
        assert_ne!(language["name"], language["code"], "{language} needs a name in get_available_languages");
    }
}
