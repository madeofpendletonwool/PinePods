use ammonia::Builder;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use gloo::events::EventListener;
use gloo_timers::callback::Timeout;
use std::collections::HashMap;
use std::str::FromStr;
use wasm_bindgen::JsCast;
use web_sys::{DomParser, HtmlElement, SupportedType, TouchEvent};
use yew::prelude::*;

// Gravatar URL generation functions (outside of use_effect_with)
pub fn calculate_gravatar_hash(email: &String) -> String {
    format!("{:x}", md5::compute(email.to_lowercase()))
}

pub fn generate_gravatar_url(email: &Option<String>, size: usize) -> String {
    let hash = calculate_gravatar_hash(&email.clone().unwrap());
    format!("https://gravatar.com/avatar/{}?s={}", hash, size)
}

// pub fn format_date(date_str: &str) -> String {
//     let date =
//         chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S").unwrap_or_else(|_| {
//             chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0)
//                 .unwrap()
//                 .naive_utc()
//         }); // Fallback for parsing error
//     date.format("%m-%d-%Y").to_string()
// }

pub fn format_date(date_str: &str) -> String {
    // Try parsing with the MySQL format
    let date = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
        // If that fails, try the PostgreSQL format
        .or_else(|_| chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S%.f"))
        // If both parsing attempts fail, fallback to the Unix epoch
        .unwrap_or_else(|_| {
            DateTime::<Utc>::from_timestamp(0, 0)
                .map(|dt| dt.naive_utc())
                .expect("invalid timestamp")
        });

    date.format("%m-%d-%Y").to_string()
}

#[derive(Clone)]
pub enum DateFormat {
    MDY,
    DMY,
    YMD,
    JUL,
    ISO,
    USA,
    EUR,
    JIS,
}

pub fn match_date_format(date_format: Option<&str>) -> DateFormat {
    let date_format = match date_format {
        Some("MDY") => DateFormat::MDY,
        Some("DMY") => DateFormat::DMY,
        Some("YMD") => DateFormat::YMD,
        Some("JUL") => DateFormat::JUL,
        Some("ISO") => DateFormat::ISO,
        Some("USA") => DateFormat::USA,
        Some("EUR") => DateFormat::EUR,
        Some("JIS") => DateFormat::JIS,
        _ => DateFormat::ISO, // default to ISO if the format is not recognized
    };
    date_format
}

pub fn parse_date(date_str: &str, user_tz: &Option<String>) -> DateTime<Tz> {
    let naive_datetime = NaiveDateTime::parse_from_str(date_str, "%a, %d %b %Y %H:%M:%S %z")
        .or_else(|_| NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S"))
        .unwrap_or_else(|_| Utc::now().naive_utc());

    let datetime_utc = Utc.from_utc_datetime(&naive_datetime);
    let tz: Tz = user_tz
        .as_ref()
        .and_then(|tz| Tz::from_str(tz).ok())
        .unwrap_or_else(|| chrono_tz::UTC);
    datetime_utc.with_timezone(&tz)
}

pub fn format_datetime(
    datetime: &DateTime<Tz>,
    hour_preference: &Option<i16>,
    date_format: DateFormat,
) -> String {
    let format_str = match date_format {
        DateFormat::MDY => "%m-%d-%Y",
        DateFormat::DMY => "%d-%m-%Y",
        DateFormat::YMD => "%Y-%m-%d",
        DateFormat::JUL => "%y/%j",
        DateFormat::ISO => "%Y-%m-%d",
        DateFormat::USA => "%m/%d/%Y",
        DateFormat::EUR => "%d.%m.%Y",
        DateFormat::JIS => "%Y-%m-%d",
    };

    match hour_preference {
        Some(12) => datetime
            .format(&format!("{} %l:%M %p", format_str))
            .to_string(),
        _ => datetime
            .format(&format!("{} %H:%M", format_str))
            .to_string(),
    }
}

pub fn truncate_description(description: String, max_length: usize) -> (String, bool) {
    let is_truncated = description.len() > max_length;

    let truncated_html = if is_truncated {
        description.chars().take(max_length).collect::<String>() + "..."
    } else {
        description.to_string()
    };

    (truncated_html, is_truncated)
}

pub fn sanitize_html_with_blank_target(description: &str) -> String {
    // Create the inner HashMap for attribute "target" with value "_blank"
    let mut attribute_values = HashMap::new();
    attribute_values.insert("target", "_blank");

    // Create the outer HashMap for tag "a"
    let mut tag_attribute_values = HashMap::new();
    tag_attribute_values.insert("a", attribute_values);

    // Configure the builder with the correct attribute values
    let mut builder = Builder::default();
    builder.add_tags(&["a"]); // ensure <a> tags are allowed
    builder.add_tag_attributes("a", &["href", "target"]); // allow href and target attributes on <a> tags
    builder.set_tag_attribute_values(tag_attribute_values); // set target="_blank" on all <a> tags

    // Clean the input HTML with the specified builder
    builder.clean(description).to_string()
}

pub fn encode_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)?
        .to_string();
    Ok(password_hash)
}

#[derive(Debug, PartialEq)]
pub enum ValidationError {
    UsernameTooShort,
    PasswordTooShort,
    InvalidEmail,
}

pub fn validate_user_input(username: &str, password: &str, email: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if username.len() < 4 {
        errors.push(ValidationError::UsernameTooShort);
    }

    if password.len() < 6 {
        errors.push(ValidationError::PasswordTooShort);
    }

    let email_regex = regex::Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap();
    if !email_regex.is_match(email) {
        errors.push(ValidationError::InvalidEmail);
    }

    errors
}

pub fn unix_timestamp_to_datetime_string(timestamp: i64) -> String {
    // Convert Unix timestamp to DateTime<Utc>, then to NaiveDateTime
    let datetime = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_else(|| Utc::now())
        .naive_utc();

    // Format with milliseconds padding
    format!("{}.000", datetime.format("%Y-%m-%d %H:%M:%S"))
}

pub fn validate_username(username: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if username.len() < 4 {
        errors.push(ValidationError::UsernameTooShort);
    }

    errors
}

#[allow(dead_code)]
pub fn validate_password(password: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    if password.len() < 6 {
        errors.push(ValidationError::PasswordTooShort);
    }

    errors
}

pub fn validate_email(email: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();

    let email_regex = regex::Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap();
    if !email_regex.is_match(email) {
        errors.push(ValidationError::InvalidEmail);
    }

    errors
}

pub fn parse_opml(opml_content: &str) -> Vec<(String, String)> {
    let parser = DomParser::new().unwrap();
    let doc = parser
        .parse_from_string(opml_content, SupportedType::TextXml)
        .unwrap()
        .dyn_into::<web_sys::Document>()
        .unwrap();

    let mut podcasts = Vec::new();
    let outlines = doc.query_selector_all("outline").unwrap();
    for i in 0..outlines.length() {
        if let Some(outline) = outlines
            .item(i)
            .and_then(|o| o.dyn_into::<web_sys::Element>().ok())
        {
            let title = outline.get_attribute("title").unwrap_or_default();
            let text = outline.get_attribute("text").unwrap_or_default();
            let final_title = if title.is_empty() { text } else { title };
            let xml_url = outline.get_attribute("xmlUrl").unwrap_or_default();
            podcasts.push((final_title, xml_url));
        }
    }
    podcasts
}

pub fn format_time(time_in_seconds: f64) -> String {
    let hours = (time_in_seconds / 3600.0).floor() as i32;
    let minutes = ((time_in_seconds % 3600.0) / 60.0).floor() as i32;
    let seconds = (time_in_seconds % 60.0).floor() as i32;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

pub fn format_time_rm_hour(time_in_seconds: f64) -> String {
    let hours = (time_in_seconds / 3600.0).floor() as i32;
    let minutes = ((time_in_seconds % 3600.0) / 60.0).floor() as i32;
    let seconds = (time_in_seconds % 60.0).floor() as i32;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}

pub fn format_time_mins(time_in_minutes: i32) -> String {
    let time_in_minutes = time_in_minutes as f64;
    let hours = (time_in_minutes / 60.0).floor() as i32;
    let minutes = (time_in_minutes % 60.0).floor() as i32;
    format!("{:02}:{:02}", hours, minutes)
}

pub fn convert_time_to_seconds(time: &str) -> Result<u32, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = time.split(':').collect();

    match parts.len() {
        3 => {
            let hours: u32 = parts[0].parse()?;
            let minutes: u32 = parts[1].parse()?;
            let seconds: u32 = parts[2].parse()?;
            Ok(hours * 3600 + minutes * 60 + seconds)
        }
        2 => {
            let minutes: u32 = parts[0].parse()?;
            let seconds: u32 = parts[1].parse()?;
            Ok(minutes * 60 + seconds)
        }
        1 => {
            let seconds: u32 = parts[0].parse()?;
            Ok(seconds)
        }
        _ => Err("Invalid time format".into()),
    }
}

pub fn strip_images_from_html(html: &str) -> String {
    let document = web_sys::window().unwrap().document().unwrap();

    // Create a temporary div to parse the HTML
    let temp_div = document.create_element("div").unwrap();
    temp_div.set_inner_html(html);

    // Remove all img elements
    if let Ok(images) = temp_div.query_selector_all("img") {
        for i in 0..images.length() {
            if let Some(img) = images.item(i) {
                if let Some(parent) = img.parent_node() {
                    let _ = parent.remove_child(&img);
                }
            }
        }
    }

    temp_div.inner_html()
}

#[hook]
pub fn use_long_press(
    on_long_press: Callback<TouchEvent>,
    delay_ms: Option<u32>,
) -> (
    Callback<TouchEvent>,
    Callback<TouchEvent>,
    Callback<TouchEvent>,
    UseStateHandle<bool>,
) {
    let timeout_handle = use_state(|| None::<Timeout>);
    let is_long_press = use_state(|| false);
    let start_position = use_state(|| None::<(i32, i32)>);

    // Configure the threshold for movement that cancels a long press
    let movement_threshold = 10; // pixels
    let delay = delay_ms.unwrap_or(500); // Default to 500ms

    let on_touch_start = {
        let timeout_handle = timeout_handle.clone();
        let is_long_press = is_long_press.clone();
        let start_position = start_position.clone();
        let on_long_press = on_long_press.clone();

        Callback::from(move |event: TouchEvent| {
            event.prevent_default();

            // Store the initial touch position
            if let Some(touch) = event.touches().get(0) {
                start_position.set(Some((touch.client_x(), touch.client_y())));
            }

            // Reset long press state
            is_long_press.set(false);

            // Clear any existing timeout
            timeout_handle.set(None);

            // Create a timeout that will trigger the long press
            let on_long_press_clone = on_long_press.clone();
            let event_clone = event.clone();
            let is_long_press_clone = is_long_press.clone();

            let timeout = Timeout::new(delay, move || {
                is_long_press_clone.set(true);
                on_long_press_clone.emit(event_clone);
            });

            timeout_handle.set(Some(timeout));
        })
    };

    let on_touch_end = {
        let timeout_handle = timeout_handle.clone();

        Callback::from(move |_event: TouchEvent| {
            // Clear the timeout if the touch ends before the long press is triggered
            timeout_handle.set(None);
        })
    };

    let on_touch_move = {
        let timeout_handle = timeout_handle.clone();
        let start_position = start_position.clone();

        Callback::from(move |event: TouchEvent| {
            // If the touch moves too much, cancel the long press
            if let Some((start_x, start_y)) = *start_position {
                if let Some(touch) = event.touches().get(0) {
                    let current_x = touch.client_x();
                    let current_y = touch.client_y();

                    let distance_x = (current_x - start_x).abs();
                    let distance_y = (current_y - start_y).abs();

                    if distance_x > movement_threshold || distance_y > movement_threshold {
                        // Movement exceeded threshold, cancel the long press
                        timeout_handle.set(None);
                    }
                }
            }
        })
    };

    (on_touch_start, on_touch_end, on_touch_move, is_long_press)
}

/// A hook for setting up a context menu triggered by long press.
///
/// # Returns
/// - State tracking if the context menu is open
/// - State tracking the position of the context menu
/// - A reference to attach to the context button
/// - A callback to handle touchstart events
/// - A callback to handle touchend events
/// - A callback to handle touchmove events
/// - A callback to close the context menu
#[hook]
pub fn use_context_menu_long_press(
    delay_ms: Option<u32>,
) -> (
    UseStateHandle<bool>,
    UseStateHandle<(i32, i32)>,
    NodeRef,
    Callback<TouchEvent>,
    Callback<TouchEvent>,
    Callback<TouchEvent>,
    Callback<()>,
) {
    let show_context_menu = use_state(|| false);
    let context_menu_position = use_state(|| (0, 0));
    let context_button_ref = use_node_ref();

    // Long press handler - simulate clicking the context button or show menu
    let on_long_press = {
        let context_button_ref = context_button_ref.clone();
        let show_context_menu = show_context_menu.clone();
        let context_menu_position = context_menu_position.clone();

        Callback::from(move |event: TouchEvent| {
            if let Some(touch) = event.touches().get(0) {
                // Record position for the context menu
                context_menu_position.set((touch.client_x(), touch.client_y()));

                // Find and click the context button (if it exists)
                if let Some(button) = context_button_ref.cast::<HtmlElement>() {
                    button.click();
                } else {
                    // If the button doesn't exist (maybe on mobile where it's hidden)
                    // we'll just set our state to show the menu
                    show_context_menu.set(true);
                }
            }
        })
    };

    // Setup long press detection
    let (on_touch_start, on_touch_end, on_touch_move, is_long_press) =
        use_long_press(on_long_press, delay_ms); // default to 600ms for long press

    // When long press is detected through the hook, update our state
    {
        let show_context_menu = show_context_menu.clone();
        use_effect_with(*is_long_press, move |is_pressed| {
            if *is_pressed {
                show_context_menu.set(true);
            }
            || ()
        });
    }

    // Close context menu callback
    let close_context_menu = {
        let show_context_menu = show_context_menu.clone();
        Callback::from(move |_| {
            show_context_menu.set(false);
        })
    };

    (
        show_context_menu,
        context_menu_position,
        context_button_ref,
        on_touch_start,
        on_touch_end,
        on_touch_move,
        close_context_menu,
    )
}

use serde_json::Value;

/// Format error messages to be more user-friendly
/// This function attempts to extract meaningful messages from error responses
pub fn format_error_message(error: &str) -> String {
    // Check if the error is in JSON format
    if let Ok(json) = serde_json::from_str::<Value>(error) {
        // Try to extract detail field from JSON
        if let Some(detail) = json.get("detail") {
            if let Some(detail_str) = detail.as_str() {
                return detail_str.to_string();
            }
        }

        // Try to extract message field from JSON
        if let Some(message) = json.get("message") {
            if let Some(message_str) = message.as_str() {
                return message_str.to_string();
            }
        }

        // Try to extract error field from JSON
        if let Some(error_field) = json.get("error") {
            if let Some(error_str) = error_field.as_str() {
                return error_str.to_string();
            }
        }
    }

    // If we can't parse as JSON or find specific fields, check for common patterns
    if error.contains("Error sending test notification:") {
        return error
            .replace("Error sending test notification:", "")
            .trim()
            .to_string();
    }

    // Check if error contains nested JSON strings and try to clean them up
    if error.contains("{\"") && error.contains("\"}") {
        let mut cleaned = error.to_string();
        // Remove common wrapper phrases
        let patterns = ["Error:", "Failed:", "Error sending", "Failed to"];
        for pattern in &patterns {
            cleaned = cleaned.replace(pattern, "");
        }

        // Try to parse any JSON strings within the error
        if let Ok(json) = serde_json::from_str::<Value>(&cleaned.trim()) {
            if let Some(detail) = json.get("detail") {
                if let Some(detail_str) = detail.as_str() {
                    return detail_str.to_string();
                }
            }
        }
    }

    // Return the original error as a fallback
    error.to_string()
}
