//! Logic the pages rely on that doesn't need a browser.

use wasm_bindgen_test::*;

use crate::components::gen_funcs::*;

#[wasm_bindgen_test]
fn dates_from_either_database_format_the_same() {
    assert_eq!(format_date("2026-03-09T14:05:06"), "03-09-2026");
    assert_eq!(format_date("2026-03-09T14:05:06.123456"), "03-09-2026");
}

#[wasm_bindgen_test]
fn an_unreadable_date_falls_back_instead_of_failing() {
    assert_eq!(format_date("not a date"), "01-01-1970");
}

#[wasm_bindgen_test]
fn long_descriptions_are_cut_and_flagged() {
    assert_eq!(truncate_description("short".into(), 10), ("short".to_string(), false));
    assert_eq!(truncate_description("exactly ten".into(), 11), ("exactly ten".to_string(), false));
    assert_eq!(truncate_description("a longer description".into(), 8), ("a longer...".to_string(), true));
}

#[wasm_bindgen_test]
fn truncating_counts_characters_not_bytes() {
    let text = "é".repeat(10);
    assert_eq!(truncate_description(text.clone(), 15), (text, false), "nothing was cut, so no ellipsis");
    assert_eq!(truncate_description("日本語のテキスト".into(), 3), ("日本語...".to_string(), true));
}

#[wasm_bindgen_test]
fn links_in_descriptions_open_elsewhere_and_scripts_are_removed() {
    let clean = sanitize_html_with_blank_target(
        r#"<a href="https://example.com" onclick="steal()">site</a><script>steal()</script>"#,
    );
    assert!(clean.contains(r#"target="_blank""#), "{clean}");
    assert!(clean.contains(r#"href="https://example.com""#), "{clean}");
    assert!(!clean.contains("script"), "{clean}");
    assert!(!clean.contains("onclick"), "{clean}");
}

#[wasm_bindgen_test]
fn javascript_links_are_removed() {
    let clean = sanitize_html_with_blank_target(r#"<a href="javascript:steal()">site</a>"#);
    assert!(!clean.contains("javascript:"), "{clean}");
}

#[wasm_bindgen_test]
fn sign_up_details_are_checked() {
    assert!(validate_user_input("sam", "secret1", "sam@example.com").is_empty());
    assert_eq!(
        validate_user_input("  ", "short", "nope"),
        vec![ValidationError::UsernameTooShort, ValidationError::PasswordTooShort, ValidationError::InvalidEmail],
    );
    assert_eq!(validate_username(""), vec![ValidationError::UsernameTooShort]);
    assert_eq!(validate_password("12345"), vec![ValidationError::PasswordTooShort]);
    assert!(validate_password("123456").is_empty());
    for bad in ["", "plain", "a@b", "a b@c.d", "@c.d"] {
        assert_eq!(validate_email(bad), vec![ValidationError::InvalidEmail], "{bad}");
    }
    assert!(validate_email("a.b+c@sub.example.org").is_empty());
}

#[wasm_bindgen_test]
fn passwords_are_hashed_with_argon2_and_a_fresh_salt() {
    let first = encode_password("secret").unwrap();
    let second = encode_password("secret").unwrap();
    assert!(first.starts_with("$argon2id$"), "{first}");
    assert_ne!(first, second);
    assert!(!first.contains("secret"));
}

#[wasm_bindgen_test]
fn times_are_shown_as_clocks() {
    assert_eq!(format_time(0), "00:00:00");
    assert_eq!(format_time(3725), "01:02:05");
    assert_eq!(format_time_rm_hour(125), "02:05");
    assert_eq!(format_time_rm_hour(3725), "01:02:05");
    assert_eq!(format_time_mins(125), "02:05");
}

#[wasm_bindgen_test]
fn typed_times_become_seconds() {
    assert_eq!(convert_time_to_seconds("01:02:05").unwrap(), 3725);
    assert_eq!(convert_time_to_seconds("02:05").unwrap(), 125);
    assert_eq!(convert_time_to_seconds("45").unwrap(), 45);
    for bad in ["", "abc", "1:2:3:4", "1:xx"] {
        assert!(convert_time_to_seconds(bad).is_err(), "{bad}");
    }
}

#[wasm_bindgen_test]
fn timestamps_become_the_servers_date_format() {
    assert_eq!(unix_timestamp_to_datetime_string(0), "1970-01-01 00:00:00.000");
    assert_eq!(unix_timestamp_to_datetime_string(1_700_000_000), "2023-11-14 22:13:20.000");
}

#[wasm_bindgen_test]
fn server_errors_are_reduced_to_their_message() {
    assert_eq!(format_error_message(r#"{"detail":"Not allowed"}"#), "Not allowed");
    assert_eq!(format_error_message(r#"{"message":"Try later"}"#), "Try later");
    assert_eq!(format_error_message(r#"{"error":"Broken"}"#), "Broken");
    assert_eq!(format_error_message("Error sending test notification: bad topic"), "bad topic");
    assert_eq!(format_error_message("plain failure"), "plain failure");
    assert_eq!(format_error_message(r#"{"other":1}"#), r#"{"other":1}"#);
}

#[wasm_bindgen_test]
fn gravatars_use_the_address_hash_and_a_size() {
    let url = generate_gravatar_url(&Some("someone@example.com".to_string()), 80);
    assert!(url.contains(&calculate_gravatar_hash(&"someone@example.com".to_string())), "{url}");
    assert!(url.contains("80"), "{url}");
    assert!(!url.contains("someone@example.com"), "the address itself isn't sent: {url}");
}
