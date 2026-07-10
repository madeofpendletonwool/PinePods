use crate::components::audio::AudioPlayerProps;
use crate::components::context::{AppState, NotificationState, UIState};
use crate::requests::episode::Episode;
use i18nrs::yew::use_translation;
use regex::Regex;
use std::sync::OnceLock;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, HtmlElement, Node};
use yew::prelude::*;
use yewdux::prelude::*;

static TIMECODE_REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();

fn get_timecode_regexes() -> &'static Vec<Regex> {
    TIMECODE_REGEXES.get_or_init(|| {
        [
            r"\((\d{1,2}):(\d{2}):(\d{2})\)",
            r"\[(\d{1,2}):(\d{2}):(\d{2})\]",
            r"\((\d{1,2}):(\d{2})\)",
            r"\[(\d{1,2}):(\d{2})\]",
            r"\b(\d{1,2}):(\d{2}):(\d{2})\b",
            r"\b(\d{1,2}):(\d{2})\b",
            r"(\d{1,2})h\s*(\d{1,2})m\s*(\d{1,2})s",
            r"(\d{1,2})h\s*(\d{1,2})m",
            r"(\d{1,2})m\s*(\d{1,2})s",
        ]
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect()
    })
}

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
    #[prop_or(None)]
    pub is_video: Option<bool>,
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
    starting_episode_for_timecode_msg: String,
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

    // Process all text nodes recursively
    process_node(
        &temp_div,
        get_timecode_regexes(),
        &document,
        &audio_dispatch,
        episode_props,
        server_name,
        api_key,
        user_id,
        current_ep,
        start_episode_first_msg,
        start_episode_first_audio_msg,
        starting_episode_for_timecode_msg,
    );

    // Return the processed HTML
    let result = temp_div.inner_html();
    result
}

// Helper function to process nodes recursively with error handling
fn process_node(
    node: &Node,
    patterns: &[Regex],
    document: &web_sys::Document,
    audio_dispatch: &Dispatch<UIState>,
    episode_props: &Props,
    server_name: String,
    api_key: String,
    user_id: i32,
    currently_playing_id: Option<i32>,
    start_episode_first_msg: String,
    start_episode_first_audio_msg: String,
    starting_episode_for_timecode_msg: String,
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

            // Try each pre-compiled pattern
            for (idx, regex) in patterns.iter().enumerate() {
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

                        // Extract time components — dispatch by index matching get_timecode_regexes order:
                        // 0,1,4,6 → h:m:s   2,3,5,8 → m:s   7 → h:m
                        let (hours, minutes, seconds) = match idx {
                            0 | 1 | 4 | 6 => {
                                let hrs = cap.get(1).map(|m| m.as_str());
                                let mins = cap.get(2).map(|m| m.as_str()).unwrap_or("0");
                                let secs = cap.get(3).map(|m| m.as_str());
                                (hrs, mins, secs)
                            }
                            7 => {
                                let hrs = cap.get(1).map(|m| m.as_str());
                                let mins = cap.get(2).map(|m| m.as_str()).unwrap_or("0");
                                (hrs, mins, None)
                            }
                            _ => {
                                let mins = cap.get(1).map(|m| m.as_str()).unwrap_or("0");
                                let secs = cap.get(2).map(|m| m.as_str());
                                (None, mins, secs)
                            }
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

                        // Episode metadata for auto-play when not currently playing
                        if let Some(url) = &episode_props.episode_url {
                            let _ = span.set_attribute("data-episode-url", url);
                        }
                        if let Some(title) = &episode_props.episode_title {
                            let _ = span.set_attribute("data-episode-title", title);
                        }
                        if let Some(artwork) = &episode_props.episode_artwork {
                            let _ = span.set_attribute("data-episode-artwork", artwork);
                        }
                        if let Some(dur) = &episode_props.episode_duration {
                            let _ = span.set_attribute("data-episode-duration", &dur.to_string());
                        }
                        let _ = span.set_attribute("data-is-youtube", &episode_props.is_youtube.unwrap_or(false).to_string());
                        let _ = span.set_attribute("data-is-video", &episode_props.is_video.unwrap_or(false).to_string());

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

                        // Add inline handler — passes the element so JS can read data-* attrs
                        let onclick_handler = if let Some(ep_id) = episode_id {
                            format!(
                                "event.preventDefault(); \
                                 event.stopPropagation(); \
                                 window.handleTimecodeClick && window.handleTimecodeClick(event.currentTarget, {}, {}); \
                                 return false;",
                                seconds, ep_id
                            )
                        } else {
                            format!(
                                "event.preventDefault(); \
                                 event.stopPropagation(); \
                                 window.handleTimecodeClick && window.handleTimecodeClick(event.currentTarget, {}, -1); \
                                 return false;",
                                seconds
                            )
                        };

                        match span.set_attribute("onclick", &onclick_handler) {
                            Ok(_) => {}
                            Err(e) => log_error(
                                format!("Failed to set onclick attribute: {:?}", e).as_str(),
                            ),
                        }

                        // Always overwrite handleTimecodeClick so it carries episode data
                        let function_text = r#"
                            window.handleTimecodeClick = function(element, seconds, episodeId) {
                                var url      = element ? element.getAttribute('data-episode-url')     : null;
                                var title    = element ? element.getAttribute('data-episode-title')   : '';
                                var artwork  = element ? element.getAttribute('data-episode-artwork') : '';
                                var duration = element ? parseInt(element.getAttribute('data-episode-duration') || '0', 10) : 0;
                                var isYt     = element ? element.getAttribute('data-is-youtube') === 'true' : false;
                                var isVid    = element ? element.getAttribute('data-is-video')   === 'true' : false;
                                document.dispatchEvent(new CustomEvent('timecode-click', {
                                    detail: {
                                        seconds: seconds, episodeId: episodeId,
                                        episodeUrl: url, episodeTitle: title, episodeArtwork: artwork,
                                        episodeDuration: duration, isYoutube: isYt, isVideo: isVid
                                    },
                                    bubbles: true,
                                    cancelable: true
                                }));
                                return false;
                            };
                        "#;

                        match js_sys::eval(function_text) {
                            Ok(_) => {}
                            Err(e) => log_error(
                                format!("Failed to register handleTimecodeClick: {:?}", e).as_str(),
                            ),
                        }

                        // Register a document-level event listener for 'timecode-click' custom events
                        if js_sys::eval("typeof window.timecodeV2DelegationAdded === 'undefined'")
                            .unwrap()
                            .as_bool()
                            .unwrap_or(true)
                        {
                            let dispatch_clone = dispatch.clone();
                            let start_episode_first_msg_delegation = start_episode_first_msg.clone();
                            let start_episode_first_audio_msg_delegation = start_episode_first_audio_msg.clone();
                            let starting_episode_for_timecode_msg_delegation = starting_episode_for_timecode_msg.clone();

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

                                            // Extract episode metadata for auto-play
                                            let ep_url = js_sys::Reflect::get(&detail, &JsValue::from_str("episodeUrl")).ok().and_then(|v| v.as_string());
                                            let ep_title = js_sys::Reflect::get(&detail, &JsValue::from_str("episodeTitle")).ok().and_then(|v| v.as_string()).unwrap_or_default();
                                            let ep_artwork = js_sys::Reflect::get(&detail, &JsValue::from_str("episodeArtwork")).ok().and_then(|v| v.as_string()).unwrap_or_default();
                                            let ep_duration = js_sys::Reflect::get(&detail, &JsValue::from_str("episodeDuration")).ok().and_then(|v| v.as_f64()).unwrap_or(0.0) as i32;
                                            let ev_is_youtube = js_sys::Reflect::get(&detail, &JsValue::from_str("isYoutube")).ok().and_then(|v| v.as_bool()).unwrap_or(false);
                                            let ev_is_video = js_sys::Reflect::get(&detail, &JsValue::from_str("isVideo")).ok().and_then(|v| v.as_bool()).unwrap_or(false);

                                            // Check current state
                                            let mut is_current_episode = false;
                                            let mut has_media = false;

                                            dispatch_clone.reduce_mut(|state| {
                                                has_media = state.media_element.is_some() || state.audio_element.is_some();

                                                if let Some(current) = &state.currently_playing {
                                                    is_current_episode = current.episode_id == event_episode_id;
                                                }
                                            });

                                            if is_current_episode && has_media {
                                                // Seek in the currently playing episode
                                                dispatch_clone.reduce_mut(|state| {
                                                    let t = seconds as f64;
                                                    if let Some(media) = &state.media_element {
                                                        media.set_current_time(t);
                                                        state.current_time_seconds = t;
                                                        if !state.audio_playing.unwrap_or(false) {
                                                            let _ = media.play();
                                                            state.audio_playing = Some(true);
                                                        }
                                                    } else if let Some(audio) = &state.audio_element {
                                                        audio.set_current_time(t);
                                                        state.current_time_seconds = t;
                                                        if !state.audio_playing.unwrap_or(false) {
                                                            let _ = audio.play();
                                                            state.audio_playing = Some(true);
                                                        }
                                                    }
                                                });
                                            } else if !is_current_episode {
                                                if let Some(url) = ep_url {
                                                    // Auto-play the episode from the timecode
                                                    let dispatch_for_media = dispatch_clone.clone();
                                                    let url_clone = url.clone();
                                                    let ep_title_clone = ep_title.clone();
                                                    let ep_artwork_clone = ep_artwork.clone();
                                                    let starting_msg = starting_episode_for_timecode_msg_delegation.clone();
                                                    dispatch_clone.reduce_mut(move |state| {
                                                        state.audio_playing = Some(true);
                                                        state.playback_speed = 1.0;
                                                        // Keep the live session volume; do NOT reset on episode switch (#775).
                                                        state.offline = Some(false);
                                                        let ep = Episode {
                                                            episodeid: event_episode_id,
                                                            episodeurl: url_clone.clone(),
                                                            episodetitle: ep_title_clone.clone(),
                                                            episodeartwork: ep_artwork_clone.clone(),
                                                            episodeduration: ep_duration,
                                                            is_youtube: ev_is_youtube,
                                                            is_video: ev_is_video,
                                                            ..Episode::default()
                                                        };
                                                        state.currently_playing = Some(AudioPlayerProps {
                                                            episode: ep,
                                                            src: url_clone.clone(),
                                                            title: ep_title_clone.clone(),
                                                            description: String::new(),
                                                            release_date: String::new(),
                                                            artwork_url: ep_artwork_clone.clone(),
                                                            duration: ep_duration.to_string(),
                                                            episode_id: event_episode_id,
                                                            duration_sec: ep_duration as f64,
                                                            start_pos_sec: seconds as f64,
                                                            end_pos_sec: 0.0,
                                                            offline: false,
                                                            is_youtube: ev_is_youtube,
                                                            is_video: ev_is_video,
                                                        });
                                                        state.set_media_source(url_clone.clone(), ev_is_video, dispatch_for_media);
                                                        let session_vol = state.audio_volume;
                                                        if let Some(media) = &state.media_element {
                                                            media.set_current_time(seconds as f64);
                                                            // Apply the live session volume to the new element (#828/#775)
                                                            media.set_volume(session_vol / 100.0);
                                                            let _ = media.play();
                                                        }
                                                    });
                                                    Dispatch::<NotificationState>::global().reduce_mut(move |ns| {
                                                        ns.info_message = Some(starting_msg);
                                                    });
                                                } else {
                                                    // No URL available
                                                    let msg = start_episode_first_msg_delegation.clone();
                                                    Dispatch::<NotificationState>::global().reduce_mut(move |ns| {
                                                        ns.info_message = Some(msg);
                                                    });
                                                }
                                            } else {
                                                // is_current_episode but no media element yet
                                                let msg = start_episode_first_audio_msg_delegation.clone();
                                                Dispatch::<NotificationState>::global().reduce_mut(move |ns| {
                                                    ns.info_message = Some(msg);
                                                });
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
                                            js_sys::eval("window.timecodeV2DelegationAdded = true;");
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
                                dispatch.reduce_mut(|state| {
                                    let time = start_time as f64;
                                    if let Some(media) = &state.media_element {
                                        media.set_current_time(time);
                                        state.current_time_seconds = time;
                                        if !state.audio_playing.unwrap_or(false) {
                                            let _ = media.play();
                                            state.audio_playing = Some(true);
                                        }
                                    } else if let Some(audio) = &state.audio_element {
                                        audio.set_current_time(time);
                                        state.current_time_seconds = time;
                                        if !state.audio_playing.unwrap_or(false) {
                                            let _ = audio.play();
                                            state.audio_playing = Some(true);
                                        }
                                    }
                                });
                            } else {
                                let msg = start_episode_first_msg_click.clone();
                                Dispatch::<NotificationState>::global().reduce_mut(move |ns| {
                                    ns.info_message = Some(msg);
                                });
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
                starting_episode_for_timecode_msg.clone(),
            );
        }
    }
}

// Hook-free HTML renderer for the common "just display this sanitized HTML" case.
// Use this instead of `SafeHtml` when you do NOT need timecode processing — it skips the
// four hook registrations (use_translation / use_store / use_selector / use_dispatch) that
// SafeHtml installs unconditionally. Critical for hot lists (EpisodeListItem renders ~50+
// of these per page, where the SafeHtml hook overhead is significant).
//
// Props are PartialEq so Yew will skip re-rendering when the html string is unchanged —
// meaning no new <div> allocation when the parent re-renders with the same content.
#[derive(Properties, PartialEq)]
pub struct RawHtmlProps {
    pub html: String,
}

#[function_component(RawHtml)]
pub fn raw_html(props: &RawHtmlProps) -> Html {
    let div = match gloo_utils::document().create_element("div") {
        Ok(el) => el,
        Err(_) => gloo_utils::document().create_element("span").unwrap(),
    };
    div.set_inner_html(&props.html);
    Html::VRef(div.into())
}

// Also update the SafeHtml component to add logging
#[function_component(SafeHtml)]
pub fn safe_html(props: &Props) -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    // Selector: only re-render when the playing episode ID changes, not on every time tick
    let current_ep = use_selector(|state: &UIState| {
        state.currently_playing.as_ref().map(|ud| ud.episode_id)
    });
    let current_ep = *current_ep;
    let audio_dispatch = use_dispatch::<UIState>();

    // Pre-capture translation strings
    let start_episode_first_msg = i18n.t("safehtml.start_episode_first");
    let start_episode_first_audio_msg = i18n.t("safehtml.start_episode_first_audio");
    let starting_episode_for_timecode_msg = i18n.t("safehtml.starting_episode_for_timecode");

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
            starting_episode_for_timecode_msg,
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
