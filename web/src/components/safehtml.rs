use crate::components::context::{AppState, UIState};
use i18nrs::yew::use_translation;
use regex::Regex;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node};
use yew::prelude::*;
use yewdux::prelude::*;

// Original SafeHtml component properties
#[derive(Properties, PartialEq)]
pub struct Props {
    pub html: String,
    #[prop_or(true)]
    pub process_timecodes: bool,

    // Optional props for episode details when processing timecodes
    #[prop_or(None)]
    pub episode_url: Option<String>,
    #[prop_or(None)]
    pub episode_title: Option<String>,
    #[prop_or(None)]
    pub episode_description: Option<String>,
    #[prop_or(None)]
    pub episode_release_date: Option<String>,
    #[prop_or(None)]
    pub episode_artwork: Option<String>,
    #[prop_or(None)]
    pub episode_duration: Option<i32>,
    #[prop_or(None)]
    pub episode_id: Option<i32>,
    #[prop_or(None)]
    pub listen_duration: Option<i32>,
    #[prop_or(None)]
    pub is_youtube: Option<bool>,
}

// Function to convert timecode to seconds
fn timecode_to_seconds(hours: Option<&str>, minutes: &str, seconds: Option<&str>) -> i32 {
    let h = hours.map_or(0, |h| h.parse::<i32>().unwrap_or(0));
    let m = minutes.parse::<i32>().unwrap_or(0);
    let s = seconds.map_or(0, |s| s.parse::<i32>().unwrap_or(0));

    h * 3600 + m * 60 + s
}

// Add these debug functions at the top of your file
fn log_debug(message: &str) {
    web_sys::console::log_1(&format!("[DEBUG] {}", message).into());
}

fn log_error(message: &str) {
    web_sys::console::error_1(&format!("[ERROR] {}", message).into());
}

// Then modify the process_timecodes function to add logging
fn process_timecodes(
    html_content: &str,
    audio_dispatch: Dispatch<UIState>,
    episode_props: &Props,
    server_name: String,
    api_key: String,
    user_id: i32,
    current_ep: Option<i32>,
    start_episode_first_msg: String,
    start_episode_first_audio_msg: String,
) -> String {
    // If the content is empty, return early
    if html_content.is_empty() {
        log_debug("html_content is empty, returning early");
        return String::new();
    }

    // Create a temporary div to hold the HTML content
    let document = match web_sys::window().and_then(|win| win.document()) {
        Some(doc) => doc,
        None => {
            log_error("Failed to get document");
            return html_content.to_string();
        }
    };

    let temp_div = match document.create_element("div") {
        Ok(div) => div,
        Err(e) => {
            log_error(format!("Failed to create div: {:?}", e).as_str());
            return html_content.to_string();
        }
    };

    temp_div.set_inner_html(html_content);

    // Define regex patterns for different timecode formats that are compatible with WASM
    // Avoid using lookbehind/lookahead assertions which aren't supported
    let patterns = vec![
        // (00:00:00) or [00:00:00] - full format with parentheses/brackets
        r"\((\d{1,2}):(\d{2}):(\d{2})\)",
        r"\[(\d{1,2}):(\d{2}):(\d{2})\]",
        // (00:00) or [00:00] - minutes:seconds with parentheses/brackets
        r"\((\d{1,2}):(\d{2})\)",
        r"\[(\d{1,2}):(\d{2})\]",
        // 00:00:00 - full format without parentheses/brackets (use word boundaries instead)
        r"\b(\d{1,2}):(\d{2}):(\d{2})\b",
        // 00:00 - minutes:seconds without parentheses/brackets
        r"\b(\d{1,2}):(\d{2})\b",
        // 1h 23m 45s or 1h23m45s format
        r"(\d{1,2})h\s*(\d{1,2})m\s*(\d{1,2})s",
        r"(\d{1,2})h\s*(\d{1,2})m",
        r"(\d{1,2})m\s*(\d{1,2})s",
    ];

    // Process all text nodes recursively
    process_node(
        &temp_div,
        &patterns,
        &document,
        &audio_dispatch,
        episode_props,
        server_name,
        api_key,
        user_id,
        current_ep,
        start_episode_first_msg,
        start_episode_first_audio_msg,
    );

    // Return the processed HTML
    let result = temp_div.inner_html();
    result
}

// Helper function to process nodes recursively with error handling
fn process_node(
    node: &Node,
    patterns: &Vec<&str>,
    document: &web_sys::Document,
    audio_dispatch: &Dispatch<UIState>,
    episode_props: &Props,
    server_name: String,
    api_key: String,
    user_id: i32,
    currently_playing_id: Option<i32>,
    start_episode_first_msg: String,
    start_episode_first_audio_msg: String,
) {
    // Skip processing if node is a script, style, or already a link
    if let Some(element) = node.dyn_ref::<Element>() {
        let tag_name = element.tag_name().to_lowercase();
        if tag_name == "script" || tag_name == "style" || tag_name == "a" {
            return;
        }
    }

    // Process text nodes
    if let Some(text) = node.node_value() {
        if !text.trim().is_empty() {
            let mut has_timecode = false;
            let mut replacements = vec![];

            // Try each pattern
            for &pattern in patterns {
                // Use safely compiled regex
                let regex = match Regex::new(pattern) {
                    Ok(r) => r,
                    Err(e) => {
                        log_error(
                            format!("Failed to compile regex pattern {}: {:?}", pattern, e)
                                .as_str(),
                        );
                        continue; // Skip this pattern if it fails to compile
                    }
                };

                if regex.is_match(&text) {
                    for cap in regex.captures_iter(&text) {
                        let full_match = match cap.get(0) {
                            Some(m) => m,
                            None => continue,
                        };

                        let start_idx = full_match.start();
                        let end_idx = full_match.end();
                        // ADD THE URL CHECK RIGHT HERE! - Before checking for duplicates
                        let text_before = if start_idx > 0 {
                            &text[..start_idx]
                        } else {
                            ""
                        };
                        let text_after = if end_idx < text.len() {
                            &text[end_idx..]
                        } else {
                            ""
                        };

                        // Skip if this looks like part of a URL
                        if text_before.ends_with("http://")
                            || text_before.ends_with("https://")
                            || text_before.ends_with("www.")
                            || text_after.starts_with(".com")
                            || text_after.starts_with(".net")
                            || text_after.starts_with(".org")
                            || text_after.starts_with(".online")
                        {
                            // Added .online for your specific case
                            continue;
                        }

                        // Check if this timecode has already been processed
                        // (Avoid duplicating the same timecode in overlapping matches)
                        if replacements.iter().any(|(s, e, _, _)| {
                            (*s <= start_idx && *e > start_idx) || (*s < end_idx && *e >= end_idx)
                        }) {
                            continue;
                        }

                        // Extract time components based on the pattern
                        let (hours, minutes, seconds) = if pattern
                            == r"\((\d{1,2}):(\d{2}):(\d{2})\)"
                            || pattern == r"\[(\d{1,2}):(\d{2}):(\d{2})\]"
                            || pattern == r"\b(\d{1,2}):(\d{2}):(\d{2})\b"
                        {
                            // Full format with hours, minutes, seconds
                            let hrs = cap.get(1).map(|m| m.as_str());
                            let mins = cap.get(2).map(|m| m.as_str()).unwrap_or("0");
                            let secs = cap.get(3).map(|m| m.as_str());

                            (hrs, mins, secs)
                        } else if pattern == r"\((\d{1,2}):(\d{2})\)"
                            || pattern == r"\[(\d{1,2}):(\d{2})\]"
                            || pattern == r"\b(\d{1,2}):(\d{2})\b"
                        {
                            // Format with just minutes and seconds
                            let mins = cap.get(1).map(|m| m.as_str()).unwrap_or("0");
                            let secs = cap.get(2).map(|m| m.as_str());

                            (None, mins, secs)
                        } else if pattern == r"(\d{1,2})h\s*(\d{1,2})m\s*(\d{1,2})s" {
                            // 1h 23m 45s format
                            let hrs = cap.get(1).map(|m| m.as_str());
                            let mins = cap.get(2).map(|m| m.as_str()).unwrap_or("0");
                            let secs = cap.get(3).map(|m| m.as_str());

                            (hrs, mins, secs)
                        } else if pattern == r"(\d{1,2})h\s*(\d{1,2})m" {
                            // 1h 23m format
                            let hrs = cap.get(1).map(|m| m.as_str());
                            let mins = cap.get(2).map(|m| m.as_str()).unwrap_or("0");

                            (hrs, mins, None)
                        } else if pattern == r"(\d{1,2})m\s*(\d{1,2})s" {
                            // 23m 45s format
                            let mins = cap.get(1).map(|m| m.as_str()).unwrap_or("0");
                            let secs = cap.get(2).map(|m| m.as_str());

                            (None, mins, secs)
                        } else {
                            // Fallback (shouldn't happen with our patterns)
                            (None, "0", None)
                        };

                        let total_seconds = timecode_to_seconds(hours, minutes, seconds);

                        replacements.push((
                            start_idx,
                            end_idx,
                            text[start_idx..end_idx].to_string(),
                            total_seconds,
                        ));
                        has_timecode = true;
                    }
                }
            }
            if has_timecode && !replacements.is_empty() {
                // Sort replacements by start position (normal order, not reverse)
                replacements.sort_by(|a, b| a.0.cmp(&b.0));

                // Get the parent node to replace the text node
                if let Some(parent) = node.parent_node() {
                    // Create a new document fragment
                    let fragment = document.create_document_fragment();

                    let mut last_end = 0;

                    // Process each replacement
                    for (start, end, match_text, seconds) in &replacements {
                        // Add text before this timecode
                        if *start > last_end {
                            let before_text = document.create_text_node(&text[last_end..*start]);
                            match fragment.append_child(&before_text) {
                                Ok(_) => {}
                                Err(e) => log_error(
                                    format!("Failed to append before_text: {:?}", e).as_str(),
                                ),
                            }
                        }

                        // Create span element for the timecode
                        let span = match document.create_element("span") {
                            Ok(el) => el,
                            Err(e) => {
                                log_error(
                                    format!("Failed to create span element: {:?}", e).as_str(),
                                );
                                continue;
                            }
                        };

                        span.set_text_content(Some(match_text));

                        // Style and attributes
                        match span.set_attribute(
                            "style",
                            "color: #3498db; cursor: pointer; text-decoration: none; display: inline-block;",
                        ) {
                            Ok(_) => {},
                            Err(e) => log_error(format!("Failed to set style attribute: {:?}", e).as_str()),
                        }

                        match span.set_attribute("data-timecode", &seconds.to_string()) {
                            Ok(_) => {}
                            Err(e) => log_error(
                                format!("Failed to set data-timecode attribute: {:?}", e).as_str(),
                            ),
                        }

                        match span.set_attribute("role", "button") {
                            Ok(_) => {}
                            Err(e) => {
                                log_error(format!("Failed to set role attribute: {:?}", e).as_str())
                            }
                        }

                        match span.set_attribute("tabindex", "0") {
                            Ok(_) => {}
                            Err(e) => log_error(
                                format!("Failed to set tabindex attribute: {:?}", e).as_str(),
                            ),
                        }

                        match span.set_attribute("class", "timecode-link") {
                            Ok(_) => {}
                            Err(e) => log_error(
                                format!("Failed to set class attribute: {:?}", e).as_str(),
                            ),
                        }

                        // Create click handler
                        let dispatch = audio_dispatch.clone();
                        let start_time = *seconds;

                        // Copy all episode props for the closure
                        let episode_id = episode_props.episode_id;

                        // Add inline handler
                        // Add inline handler
                        let onclick_handler = if let Some(ep_id) = episode_id {
                            format!(
                                "event.preventDefault(); \
                                 event.stopPropagation(); \
                                 console.log('Timecode span clicked inline: {}s, episode: {}'); \
                                 window.handleTimecodeClick && window.handleTimecodeClick({}, {}); \
                                 return false;",
                                seconds, ep_id, seconds, ep_id
                            )
                        } else {
                            format!(
                                "event.preventDefault(); \
                                 event.stopPropagation(); \
                                 console.log('Timecode span clicked inline: {}s'); \
                                 window.handleTimecodeClick && window.handleTimecodeClick({}, -1); \
                                 return false;",
                                seconds, seconds
                            )
                        };

                        match span.set_attribute("onclick", &onclick_handler) {
                            Ok(_) => {}
                            Err(e) => log_error(
                                format!("Failed to set onclick attribute: {:?}", e).as_str(),
                            ),
                        }

                        // Add global handleTimecodeClick function if it doesn't exist yet
                        // Add global handleTimecodeClick function if it doesn't exist yet
                        let function_text = r#"
                            if (!window.handleTimecodeClick) {
                                window.handleTimecodeClick = function(seconds, episodeId) {
                                    // Dispatch a custom event that our Rust code can listen for
                                    document.dispatchEvent(new CustomEvent('timecode-click', {
                                        detail: { seconds: seconds, episodeId: episodeId },
                                        bubbles: true,
                                        cancelable: true
                                    }));

                                    return false;
                                };

                            }
                        "#;

                        // Only add the function once
                        if js_sys::eval("typeof window.handleTimecodeClick === 'undefined'")
                            .unwrap()
                            .as_bool()
                            .unwrap_or(true)
                        {
                            match js_sys::eval(function_text) {
                                Ok(_) => {}
                                Err(e) => log_error(
                                    format!("Failed to add global function: {:?}", e).as_str(),
                                ),
                            }
                        }

                        // Register a document-level event listener for 'timecode-click' custom events
                        if js_sys::eval("typeof window.timecodeDelegationAdded === 'undefined'")
                            .unwrap()
                            .as_bool()
                            .unwrap_or(true)
                        {
                            let dispatch_clone = dispatch.clone();
                            let start_episode_first_msg_delegation = start_episode_first_msg.clone();
                            let start_episode_first_audio_msg_delegation = start_episode_first_audio_msg.clone();

                            let delegation_handler = Closure::wrap(Box::new(
                                move |e: web_sys::CustomEvent| {
                                    if let Some(detail) = e.detail().dyn_ref::<js_sys::Object>() {
                                        let js_seconds = js_sys::Reflect::get(
                                            &detail,
                                            &JsValue::from_str("seconds"),
                                        )
                                        .unwrap_or(JsValue::from(0));

                                        let js_episode_id = js_sys::Reflect::get(
                                            &detail,
                                            &JsValue::from_str("episodeId"),
                                        )
                                        .unwrap_or(JsValue::from(-1));

                                        if let (Some(seconds_f64), Some(event_episode_id_f64)) =
                                            (js_seconds.as_f64(), js_episode_id.as_f64())
                                        {
                                            let seconds = seconds_f64 as i32;
                                            let event_episode_id = event_episode_id_f64 as i32;

                                            web_sys::console::log_1(
                                                &format!(
                                                    "Handling timecode click via delegation: {}s, episode: {}",
                                                    seconds, event_episode_id
                                                )
                                                .into(),
                                            );

                                            // Get current playing ID from state
                                            let mut is_current_episode = false;
                                            let mut has_audio_element = false;

                                            dispatch_clone.reduce_mut(|state| {
                                                has_audio_element = state.audio_element.is_some();

                                                if let Some(current) = &state.currently_playing {
                                                    is_current_episode = current.episode_id == event_episode_id;
                                                    web_sys::console::log_1(
                                                        &format!(
                                                            "Delegation: Comparing episode IDs: {} vs {}, match: {}",
                                                            event_episode_id, current.episode_id, is_current_episode
                                                        )
                                                        .into(),
                                                    );
                                                }
                                            });

                                            if is_current_episode && has_audio_element {
                                                // Now use dispatch to handle the click
                                                dispatch_clone.reduce_mut(|state| {
                                                    if let Some(audio_element) = state.audio_element.as_ref() {
                                                        web_sys::console::log_1(&format!("Found audio element, seeking to {}s", seconds).into());

                                                        // Set the current time
                                                        audio_element.set_current_time(seconds as f64);
                                                        state.current_time_seconds = seconds as f64;

                                                        // If paused, start playing
                                                        if !state.audio_playing.unwrap_or(false) {
                                                            web_sys::console::log_1(&"Starting playback".into());
                                                            let _ = audio_element.play();
                                                            state.audio_playing = Some(true);
                                                        }
                                                    }
                                                });
                                            } else if !is_current_episode {
                                                web_sys::console::log_1(
                                                    &"Delegation: Not the current episode".into(),
                                                );
                                                // Alert the user
                                                if let Some(window) = web_sys::window() {
                                                    let _ = window.alert_with_message(&start_episode_first_msg_delegation);
                                                }
                                            } else {
                                                web_sys::console::error_1(
                                                    &"No audio element available".into(),
                                                );
                                                if let Some(window) = web_sys::window() {
                                                    let _ = window.alert_with_message(&start_episode_first_audio_msg_delegation);
                                                }
                                            }
                                        }
                                    }
                                },
                            )
                                as Box<dyn FnMut(_)>);

                            // Add the event listener to the document
                            if let Some(document) = web_sys::window().and_then(|win| win.document())
                            {
                                match document.add_event_listener_with_callback(
                                    "timecode-click",
                                    delegation_handler.as_ref().unchecked_ref(),
                                ) {
                                    Ok(_) => {
                                        delegation_handler.forget();

                                        // Mark as added so we don't add it multiple times
                                        let _ =
                                            js_sys::eval("window.timecodeDelegationAdded = true;");
                                    }
                                    Err(e) => log_error(
                                        format!(
                                            "Failed to add document-level event handler: {:?}",
                                            e
                                        )
                                        .as_str(),
                                    ),
                                }
                            }
                        }

                        // Create the standard click handler
                        let current_playing_id = currently_playing_id.clone(); // Clone for the closure
                        let start_episode_first_msg_click = start_episode_first_msg.clone();
                        let click_handler = Closure::wrap(Box::new(move |e: web_sys::MouseEvent| {
                            web_sys::console::log_1(
                                &"TIMECODE SPAN CLICKED - HANDLER STARTED".into(),
                            );

                            // Critical - prevent default before doing anything else
                            e.prevent_default();
                            e.stop_propagation();

                            // Check if this episode is currently playing by comparing IDs
                            let is_current_episode = match (episode_id, current_playing_id) {
                                (Some(ep_id), Some(current_id)) => {
                                    let is_match = ep_id == current_id;
                                    web_sys::console::log_1(
                                        &format!(
                                            "Comparing episode IDs: {} vs {}, match: {}",
                                            ep_id, current_id, is_match
                                        )
                                        .into(),
                                    );
                                    is_match
                                }
                                _ => {
                                    web_sys::console::log_1(
                                        &"Missing episode ID for comparison".into(),
                                    );
                                    false
                                }
                            };

                            if is_current_episode {
                                // Episode IS currently playing - seek to the timecode
                                web_sys::console::log_1(&"Episode is current, seeking only".into());

                                dispatch.reduce_mut(|state| {
                                    if let Some(audio_element) = state.audio_element.as_ref() {
                                        let time = start_time as f64;
                                        web_sys::console::log_1(
                                            &format!("Setting current time to {}", time).into(),
                                        );

                                        audio_element.set_current_time(time);
                                        state.current_time_seconds = time;

                                        // Update formatted time display
                                        let hours = (time / 3600.0).floor() as i32;
                                        let minutes = ((time % 3600.0) / 60.0).floor() as i32;
                                        let seconds = (time % 60.0).floor() as i32;
                                        state.current_time_formatted =
                                            format!("{:02}:{:02}:{:02}", hours, minutes, seconds);

                                        // If paused, start playing
                                        if !state.audio_playing.unwrap_or(false) {
                                            let _ = audio_element.play();
                                            state.audio_playing = Some(true);
                                        }
                                    }
                                });
                            } else {
                                // Not the current episode - show message
                                web_sys::console::log_1(&"Not the current episode".into());

                                // Alert the user
                                if let Some(window) = web_sys::window() {
                                    let _ = window.alert_with_message(&start_episode_first_msg_click);
                                }
                            }

                            // Provide visual feedback on click
                            if let Some(element) = e.current_target() {
                                if let Some(element) = element.dyn_ref::<HtmlElement>() {
                                    match element
                                        .style()
                                        .set_property("text-decoration", "underline")
                                    {
                                        Ok(_) => {}
                                        Err(e) => web_sys::console::error_1(
                                            &format!("Failed to add visual feedback: {:?}", e)
                                                .into(),
                                        ),
                                    }

                                    // Reset after delay
                                    let element_clone = element.clone();
                                    let timeout_callback = Closure::wrap(Box::new(move || {
                                        match element_clone
                                            .style()
                                            .set_property("text-decoration", "none")
                                        {
                                            Ok(_) => {}
                                            Err(e) => web_sys::console::error_1(
                                                &format!(
                                                    "Failed to reset visual feedback: {:?}",
                                                    e
                                                )
                                                .into(),
                                            ),
                                        }
                                    })
                                        as Box<dyn FnMut()>);

                                    match web_sys::window()
                                        .unwrap()
                                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                                            timeout_callback.as_ref().unchecked_ref(),
                                            500,
                                        ) {
                                        Ok(_) => {}
                                        Err(e) => web_sys::console::error_1(
                                            &format!(
                                                "Failed to set visual feedback timeout: {:?}",
                                                e
                                            )
                                            .into(),
                                        ),
                                    }

                                    timeout_callback.forget();
                                }
                            }
                        })
                            as Box<dyn FnMut(_)>);

                        // Attach the click handler
                        match span.add_event_listener_with_callback(
                            "click",
                            click_handler.as_ref().unchecked_ref(),
                        ) {
                            Ok(_) => {
                                click_handler.forget();
                            }
                            Err(e) => {
                                web_sys::console::error_1(
                                    &format!("Failed to add click handler: {:?}", e).into(),
                                );
                            }
                        }

                        // Also add mousedown handler for better coverage
                        let mousedown_handler =
                            Closure::wrap(Box::new(move |_e: web_sys::MouseEvent| {
                                web_sys::console::log_1(
                                    &format!("MOUSEDOWN detected on timecode {}s", start_time)
                                        .into(),
                                );
                            }) as Box<dyn FnMut(_)>);

                        match span.add_event_listener_with_callback(
                            "mousedown",
                            mousedown_handler.as_ref().unchecked_ref(),
                        ) {
                            Ok(_) => {
                                mousedown_handler.forget();
                            }
                            Err(_) => {}
                        }

                        // Add the span to the fragment
                        match fragment.append_child(&span) {
                            Ok(_) => {}
                            Err(e) => log_error(format!("Failed to append span: {:?}", e).as_str()),
                        }

                        last_end = *end;
                    }

                    // Add any remaining text after the last replacement
                    if last_end < text.len() {
                        let after_text = document.create_text_node(&text[last_end..]);
                        match fragment.append_child(&after_text) {
                            Ok(_) => {}
                            Err(e) => {
                                log_error(format!("Failed to append after_text: {:?}", e).as_str())
                            }
                        }
                    }

                    // Replace the original text node with our fragment
                    match parent.replace_child(&fragment, node) {
                        Ok(_) => {}
                        Err(e) => log_error(format!("Failed to replace node: {:?}", e).as_str()),
                    }
                } else {
                    log_error("No parent node found for replacement");
                }
            }
        }
    } else if node.has_child_nodes() {
        // Process child nodes recursively (but create a copy to avoid mutation during iteration)
        let child_count = node.child_nodes().length();
        let mut children = Vec::with_capacity(child_count as usize);

        for i in 0..child_count {
            if let Some(child) = node.child_nodes().get(i) {
                children.push(child);
            }
        }

        for child in children {
            process_node(
                &child,
                patterns,
                document,
                audio_dispatch,
                episode_props,
                server_name.clone(),
                api_key.clone(),
                user_id,
                currently_playing_id,
                start_episode_first_msg.clone(),
                start_episode_first_audio_msg.clone(),
            );
        }
    }
}

// Also update the SafeHtml component to add logging
#[function_component(SafeHtml)]
pub fn safe_html(props: &Props) -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let (ui_state, _ui_dispatch) = use_store::<UIState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let current_ep = ui_state
        .currently_playing
        .as_ref()
        .map(|ud| ud.episode_id.clone());

    // Debug all the episode props being passed
    // log_debug(format!("Episode id: {:?}", props.episode_id).as_str());
    // log_debug(format!("Currently play Episode id: {:?}", current_ep).as_str());

    let (_, audio_dispatch) = use_store::<UIState>();

    // Pre-capture translation strings
    let start_episode_first_msg = i18n.t("safehtml.start_episode_first");
    let start_episode_first_audio_msg = i18n.t("safehtml.start_episode_first_audio");

    // Only get the audio_dispatch when timecode processing is enabled
    let processed_html = if props.process_timecodes && server_name.is_some() && api_key.is_some() && user_id.is_some() {
        process_timecodes(
            &props.html,
            audio_dispatch,
            props,
            server_name.unwrap(),
            api_key.unwrap().unwrap(),
            user_id.unwrap(),
            current_ep,
            start_episode_first_msg,
            start_episode_first_audio_msg,
        )
    } else {
        log_debug("Timecode processing disabled, returning original HTML");
        props.html.clone()
    };

    // Create the div just like the original implementation
    let div = match gloo_utils::document().create_element("div") {
        Ok(el) => el,
        Err(e) => {
            log_error(format!("Failed to create div: {:?}", e).as_str());
            // Fallback - we need to return something
            gloo_utils::document().create_element("span").unwrap()
        }
    };

    div.set_inner_html(&processed_html);

    // Return the VRef just like the original implementation
    Html::VRef(div.into())
}
