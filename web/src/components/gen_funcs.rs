use std::collections::HashMap;
use ammonia::Builder;
use web_sys::{DomParser, SupportedType};
use wasm_bindgen::JsCast;
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHasher, SaltString
    },
    Argon2
};
use chrono::{DateTime, NaiveDateTime, Utc, TimeZone};
use chrono_tz::Tz;


pub fn format_date(date_str: &str) -> String {
    let date = chrono::NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
        .unwrap_or_else(|_| chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap().naive_utc()); // Fallback for parsing error
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
    let naive_datetime = NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S").unwrap_or_else(|_| Utc::now().naive_utc());
    let datetime_utc = Utc.from_utc_datetime(&naive_datetime);
    let tz: Tz = user_tz.as_ref().and_then(|tz| tz.parse().ok()).unwrap_or_else(|| chrono_tz::UTC);
    datetime_utc.with_timezone(&tz)
}

pub fn format_datetime(datetime: &DateTime<Tz>, hour_preference: &Option<i16>, date_format: DateFormat) -> String {
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
        Some(12) => datetime.format(&format!("{} %l:%M %p", format_str)).to_string(),
        _ => datetime.format(&format!("{} %H:%M", format_str)).to_string(),
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
// pub fn sanitize_html(description: &str) -> String {
//     let sanitized_html = clean(description);
// }

// pub fn check_auth(effect_dispatch: Dispatch<AppState>) {
//     console::log_1(&"Checking authentication... Pre use_effect".into());
//     use_effect_with(
//         (),
//         move |_| {
//             let effect_dispatch_clone = effect_dispatch.clone();

//             spawn_local(async move {
//                 let window = window().expect("no global `window` exists");
//                 let location = window.location();
//                 let current_route = location.href().expect("should be able to get href");
//                 console::log_1(&current_route.clone().into());
//                 console::log_1(&"Checking authentication... Inside Check_auth".into());
//                 use_check_authentication(effect_dispatch_clone, &current_route);
//             });

//             || ()
//         }
//     );
// }



pub fn encode_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2.hash_password(password.as_bytes(), &salt)?.to_string();
    Ok(password_hash)
}


pub fn validate_user_input(username: &str, password: &str, email: &str) -> Result<(), String> {
    if username.len() < 4 {
        return Err("Username must be at least 4 characters long".to_string());
    }

    if password.len() < 6 {
        return Err("Password must be at least 6 characters long".to_string());
    }

    let email_regex = regex::Regex::new(r"^[^@\s]+@[^@\s]+\.[^@\s]+$").unwrap();
    if !email_regex.is_match(email) {
        return Err("Email is not in a valid format".to_string());
    }

    Ok(())
}

pub fn parse_opml(opml_content: &str) -> Vec<(String, String)> {
    let parser = DomParser::new().unwrap();
    let doc = parser.parse_from_string(opml_content, SupportedType::TextXml)
        .unwrap()
        .dyn_into::<web_sys::Document>()
        .unwrap();

    let mut podcasts = Vec::new();
    let outlines = doc.query_selector_all("outline").unwrap();
    for i in 0..outlines.length() {
        if let Some(outline) = outlines.item(i).and_then(|o| o.dyn_into::<web_sys::Element>().ok()) {
            let title = outline.get_attribute("title").unwrap_or_default();
            let xml_url = outline.get_attribute("xmlUrl").unwrap_or_default();
            podcasts.push((title, xml_url));
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