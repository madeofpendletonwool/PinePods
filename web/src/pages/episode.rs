use crate::components::app_drawer::App_drawer;
use crate::components::audio::on_play_click;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, EpisodeDetailState, EpisodeNavigationState, EpisodeStatusState, NotificationState, PageLoadState, UIState, UserPreferencesState};
use crate::components::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::gen_funcs::format_error_message;
use crate::components::gen_funcs::{
    format_datetime, format_time, match_date_format, parse_date, sanitize_html_with_blank_target,
};
use crate::components::loading::Loading;
use crate::components::safehtml::SafeHtml;
use crate::requests::pod_req;
use crate::requests::pod_req::{
    call_check_podcast, call_create_share_link, call_download_episode, call_download_episode_file,
    call_fetch_podcasting_2_data, call_get_episode_id, call_get_episode_metadata,
    call_mark_episode_completed, call_mark_episode_uncompleted, call_queue_episode,
    call_remove_downloaded_episode, call_remove_queued_episode, call_remove_saved_episode,
    call_save_episode, DownloadEpisodeRequest, EpisodeRequest, FetchPodcasting2DataRequest,
    MarkEpisodeCompletedRequest, QueuePodcastRequest, SavePodcastRequest, Transcript,
    call_get_ai_status, call_get_episode_transcript, call_transcribe_episode, StoredTranscript,
    call_get_episode_skip_segments, SkipSegment, call_detect_ads, call_adjust_ad_segment_review,
    AdSegmentReviewRequest,
};
use crate::requests::search_pods::{call_get_podcast_details_dynamic, call_parse_podcast_url};
use i18nrs::yew::use_translation;
use regex::Regex;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::UrlSearchParams;
use web_sys::{window, Headers, Request, RequestInit, Response};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::{BrowserHistory, History};
use yew_router::hooks::use_location;
use yewdux::prelude::*;

#[allow(dead_code)]
async fn fallback_to_podcast_parsing(
    server_name: String,
    api_key: Option<String>,
    episode_url_clone: String,
    audio_url_clone: String,
    podcast_title_clone: String,
    podcast_index_id: i32,
    is_youtube: bool,
    episode_id: i32,
    _dispatch: Dispatch<AppState>,
    error_clone: UseStateHandle<Option<String>>,
    aud_dispatch: Dispatch<UIState>,
    ep_2_loading_clone: UseStateHandle<bool>,
    _ui_state: Rc<AppState>,
    window: web_sys::Window,
    user_id: i32,
    loading_clone: UseStateHandle<bool>,
) {
    match call_parse_podcast_url(server_name.clone(), &api_key, &episode_url_clone).await {
        Ok(result) => {
            if let Some(ep) = result
                .episodes
                .iter()
                .find(|ep| ep.episodeurl == audio_url_clone)
                .cloned()
            {
                let episodeduration = ep.episodeduration;
                if episodeduration != 0 {
                    let ep_url = episode_url_clone.clone();
                    let aud_url = audio_url_clone.clone();
                    let podcast_title = podcast_title_clone.clone();
                    let ep_for_detail = ep.clone();

                    Dispatch::<EpisodeNavigationState>::global().reduce_mut(move |s| {
                        s.selected_episode_id = Some(episode_id);
                        s.selected_episode_url = Some(ep_url.clone());
                        s.selected_episode_audio_url = Some(aud_url.clone());
                        s.selected_podcast_title = Some(podcast_title.clone());
                    });
                    Dispatch::<EpisodeDetailState>::global().reduce_mut(move |s| {
                        s.fetched_episode = Some(ep_for_detail);
                    });

                    // Handle podcast 2.0 data if needed
                    if let Some(episode_id) = Dispatch::<EpisodeNavigationState>::global().get().selected_episode_id {
                        let chap_request = FetchPodcasting2DataRequest {
                            episode_id,
                            user_id: user_id,
                        };

                        if !is_youtube {
                            let server_name_clone = server_name.clone();
                            let api_key_clone = api_key.clone();
                            wasm_bindgen_futures::spawn_local(async move {
                                match call_fetch_podcasting_2_data(
                                    &server_name_clone,
                                    &api_key_clone,
                                    &chap_request,
                                )
                                .await
                                {
                                    Ok(response) => {
                                        aud_dispatch.reduce_mut(|state| {
                                            state.episode_page_transcript =
                                                Some(response.transcripts);
                                            state.episode_page_people = Some(response.people);
                                            state.episode_page_chapters = Some(response.chapters);
                                        });
                                        ep_2_loading_clone.set(false);
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(
                                            &format!("Error fetching podcast 2.0 data: {}", e)
                                                .into(),
                                        );
                                        aud_dispatch.reduce_mut(|state| {
                                            state.episode_page_transcript = None;
                                            state.episode_page_people = None;
                                            state.episode_page_chapters = None;
                                        });
                                    }
                                }
                            });
                        }
                    }

                    // Update the URL with the parameters
                    let mut new_url = window.location().origin().unwrap();
                    new_url.push_str(&window.location().pathname().unwrap());
                    new_url.push_str("?podcast_title=");
                    new_url.push_str(&urlencoding::encode(&podcast_title_clone));
                    new_url.push_str("&episode_url=");
                    new_url.push_str(&urlencoding::encode(&episode_url_clone));
                    new_url.push_str("&audio_url=");
                    new_url.push_str(&urlencoding::encode(&audio_url_clone));
                    new_url.push_str("&podcast_index_id=");
                    new_url.push_str(&podcast_index_id.to_string());
                    new_url.push_str("&is_youtube=");
                    new_url.push_str(&is_youtube.to_string());

                    window
                        .history()
                        .expect("should have a history")
                        .replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(&new_url))
                        .expect("should push state");

                    loading_clone.set(false);
                } else {
                    error_clone.set(Some("Failed to parse duration".to_string()));
                }
            }
        }
        Err(e) => {
            error_clone.set(Some(e.to_string()));
        }
    }
}

#[allow(dead_code)]
fn get_current_url() -> String {
    let window = window().expect("no global `window` exists");
    let location = window.location();
    let current_url = location
        .href()
        .unwrap_or_else(|_| "Unable to retrieve URL".to_string());

    // Get the server URL from local storage
    if let Some(storage) = window.local_storage().ok().flatten() {
        if let Ok(Some(auth_state)) = storage.get_item("userAuthState") {
            // Parse the JSON string
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&auth_state) {
                if let Some(server_name) = json
                    .get("auth_details")
                    .and_then(|auth| auth.get("server_name"))
                    .and_then(|name| name.as_str())
                {
                    // Replace tauri.localhost with the server name
                    return current_url.replace("http://tauri.localhost", server_name);
                }
            }
        }
    }

    // Return the original URL if we couldn't get the server name
    current_url
}

#[derive(Properties, PartialEq)]
pub struct TranscriptModalProps {
    pub transcripts: Vec<Transcript>,
    pub onclose: Callback<()>,
    pub server_name: String,
    pub api_key: String,
}

#[derive(Properties, PartialEq)]
pub struct TranscriptInlineProps {
    pub transcripts: Vec<Transcript>,
    pub server_name: String,
    pub api_key: String,
    /// Detected ad time ranges (seconds) to highlight within the transcript (#790).
    #[prop_or_default]
    pub ad_ranges: Vec<(f64, f64)>,
}

/// One timed transcript cue parsed from SRT/VTT.
struct TranscriptCue {
    start: f64,
    end: f64,
    text: String,
}

/// Parse an SRT/VTT timestamp (`HH:MM:SS,mmm`, `MM:SS.mmm`, …) into seconds.
fn parse_ts(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (h.trim().parse::<f64>().ok()?, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        [m, s] => (0.0, m.trim().parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + sec)
}

/// Parse SRT/VTT content into timed cues, so we can render the transcript as timestamped,
/// clickable segments instead of an undifferentiated text blob (#790). Returns empty for content
/// that has no cue timings (e.g. plain text / HTML), so callers can fall back to paragraphs.
fn parse_transcript_cues(content: &str) -> Vec<TranscriptCue> {
    let speaker_re = Regex::new(r"<v\s+[^>]*>").unwrap();
    let mut cues = Vec::new();
    let mut lines = content.lines().peekable();
    while let Some(line) = lines.next() {
        let Some(idx) = line.find("-->") else { continue };
        let start = parse_ts(&line[..idx]);
        let end = parse_ts(line[idx + 3..].split_whitespace().next().unwrap_or(""));
        let (Some(start), Some(end)) = (start, end) else { continue };
        let mut text_parts: Vec<String> = Vec::new();
        while let Some(peek) = lines.peek() {
            if peek.trim().is_empty() || peek.contains("-->") {
                break;
            }
            let l = lines.next().unwrap();
            let l = speaker_re.replace_all(l, "");
            let l = l.replace("</v>", "");
            let t = l.trim();
            if !t.is_empty() {
                text_parts.push(t.to_string());
            }
        }
        let text = text_parts.join(" ");
        if !text.is_empty() {
            cues.push(TranscriptCue { start, end, text });
        }
    }
    cues
}

/// Fetches and renders a transcript's text directly (no modal chrome), so it
/// can be shown inline inside the episode page's Transcript tab.
#[function_component(TranscriptInline)]
pub fn transcript_inline(props: &TranscriptInlineProps) -> Html {
    let transcript_content = use_state(|| None::<String>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);

    {
        let transcripts = props.transcripts.clone();
        let transcript_content = transcript_content.clone();
        let loading = loading.clone();
        let error = error.clone();
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();

        use_effect_with(transcripts, move |transcripts| {
            if !transcripts.is_empty() {
                let transcript = transcripts
                    .iter()
                    .find(|t| t.mime_type == "text/vtt")
                    .or_else(|| transcripts.iter().find(|t| t.mime_type == "application/srt"))
                    .unwrap_or(&transcripts[0])
                    .clone();

                let url = transcript.url.clone();
                let mime_type = transcript.mime_type.clone();

                spawn_local(async move {
                    let api_url = format!("{}/api/data/fetch_transcript", server_name);
                    let request_body = serde_json::json!({ "url": url });

                    let opts = RequestInit::new();
                    opts.set_method("POST");
                    opts.set_body(&JsValue::from_str(&request_body.to_string()));
                    let headers = Headers::new().unwrap();
                    headers.set("Content-Type", "application/json").unwrap();
                    headers.set("Api-Key", &api_key).unwrap();
                    opts.set_headers(&headers);

                    let request = Request::new_with_str_and_init(&api_url, &opts).unwrap();
                    let window = web_sys::window().unwrap();
                    match JsFuture::from(window.fetch_with_request(&request)).await {
                        Ok(resp_value) => match resp_value.dyn_into::<Response>() {
                            Ok(resp) => match JsFuture::from(resp.text().unwrap()).await {
                                Ok(text) => {
                                    let text_str = text.as_string().unwrap();
                                    match serde_json::from_str::<serde_json::Value>(&text_str) {
                                        Ok(json) => {
                                            if json["success"].as_bool().unwrap_or(false) {
                                                let content = json["content"].as_str().unwrap_or("");
                                                let cleaned_text = match mime_type.as_str() {
                                                    "text/html" => {
                                                        let div = web_sys::window()
                                                            .unwrap()
                                                            .document()
                                                            .unwrap()
                                                            .create_element("div")
                                                            .unwrap();
                                                        div.set_inner_html(content);
                                                        div.text_content().unwrap_or_default()
                                                    }
                                                    // Keep raw SRT/VTT so the render can parse
                                                    // timecodes into clickable cues (#790).
                                                    _ => content.to_string(),
                                                };
                                                transcript_content.set(Some(cleaned_text));
                                                loading.set(false);
                                            } else {
                                                let error_msg =
                                                    json["error"].as_str().unwrap_or("Unknown error");
                                                error.set(Some(format!("Backend error: {}", error_msg)));
                                                loading.set(false);
                                            }
                                        }
                                        Err(e) => {
                                            error.set(Some(format!(
                                                "Failed to parse JSON response: {:?}",
                                                e
                                            )));
                                            loading.set(false);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to read response text: {:?}", e)));
                                    loading.set(false);
                                }
                            },
                            Err(e) => {
                                error.set(Some(format!("Failed to parse response: {:?}", e)));
                                loading.set(false);
                            }
                        },
                        Err(e) => {
                            error.set(Some(format!("Failed to fetch transcript via backend: {:?}", e)));
                            loading.set(false);
                        }
                    }
                });
            }
            move || ()
        });
    }

    let content_ref = use_node_ref();
    use_effect_with(
        (content_ref.clone(), transcript_content.clone()),
        move |(content_ref, content)| {
            if let Some(content) = content.as_ref() {
                if content.contains('<') && content.contains('>') {
                    if let Some(element) = content_ref.cast::<web_sys::HtmlElement>() {
                        element.set_inner_html(content);
                    }
                }
            }
            || ()
        },
    );

    // Seek the global player to a transcript timestamp when a cue is clicked.
    let (ui_state, _) = use_store::<UIState>();
    let seek = {
        let ui_state = ui_state.clone();
        Callback::from(move |t: f64| {
            if let Some(me) = ui_state.media_element.as_ref() {
                me.set_current_time(t);
            } else if let Some(ae) = ui_state.audio_element.as_ref() {
                ae.set_current_time(t);
            }
        })
    };
    let ad_ranges = props.ad_ranges.clone();

    html! {
        <div class="ep-transcript-content space-y-4 item_container-text prose dark:prose-invert max-w-none">
            if *loading {
                <div class="flex justify-center items-center h-32">
                    <div class="animate-spin rounded-full h-12 w-12 border-4 border-current border-t-transparent"></div>
                </div>
            } else if let Some(err) = &*error {
                <div class="text-red-500 dark:text-red-400 p-2">{err}</div>
            } else if let Some(content) = &*transcript_content {
                {{
                    let cues = parse_transcript_cues(content);
                    if !cues.is_empty() {
                        // Timecoded, clickable transcript with ad ranges highlighted (#790).
                        html! {
                            <div class="ep-transcript-cues">
                                { for cues.iter().map(|cue| {
                                    let is_ad = ad_ranges.iter().any(|(s, e)| cue.start < *e && cue.end > *s);
                                    let start = cue.start;
                                    let seek = seek.clone();
                                    let onclick = Callback::from(move |_: MouseEvent| seek.emit(start));
                                    let cls = if is_ad { "ep-transcript-cue is-ad" } else { "ep-transcript-cue" };
                                    html! {
                                        <div class={cls}>
                                            <button class="ep-transcript-cue-ts" {onclick} title="Jump to this point">
                                                { format_time(start as i32) }
                                            </button>
                                            <span class="ep-transcript-cue-text">{ &cue.text }</span>
                                        </div>
                                    }
                                }) }
                            </div>
                        }
                    } else if content.contains('<') && content.contains('>') {
                        html! { <div class="transcript-content" ref={content_ref}/> }
                    } else {
                        html! {
                            <div>
                                { for content.split('\n').map(|line| html! { <p>{line}</p> }) }
                            </div>
                        }
                    }
                }}
            }
        </div>
    }
}

#[function_component(TranscriptModal)]
pub fn transcript_modal(props: &TranscriptModalProps) -> Html {
    let transcript_content = use_state(|| None::<String>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);

    let (i18n, _) = use_translation();
    let i18n_episode_transcript = i18n.t("episode.episode_transcript").to_string();

    // Load transcript content when component mounts
    {
        let transcripts = props.transcripts.clone();
        let transcript_content = transcript_content.clone();
        let loading = loading.clone();
        let error = error.clone();
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();

        use_effect_with(transcripts, move |transcripts| {
            if !transcripts.is_empty() {
                // Clone the transcripts at the beginning to avoid lifetime issues
                let transcript = transcripts
                    .iter()
                    .find(|t| t.mime_type == "text/vtt")
                    .or_else(|| {
                        transcripts
                            .iter()
                            .find(|t| t.mime_type == "application/srt")
                    })
                    .unwrap_or(&transcripts[0])
                    .clone(); // Clone here to avoid lifetime issues

                let url = transcript.url.clone();
                let mime_type = transcript.mime_type.clone();

                spawn_local(async move {
                    let speaker_regex = Regex::new(r"<v\s+[^>]+>").unwrap();
                    let simple_speaker_regex = Regex::new(r"<v\s+").unwrap();

                    // Use backend proxy to fetch transcript
                    let api_url = format!("{}/api/data/fetch_transcript", server_name);
                    let request_body = serde_json::json!({
                        "url": url
                    });

                    let opts = RequestInit::new();
                    opts.set_method("POST");
                    opts.set_body(&JsValue::from_str(&request_body.to_string()));
                    let headers = Headers::new().unwrap();
                    headers.set("Content-Type", "application/json").unwrap();
                    headers.set("Api-Key", &api_key).unwrap();
                    opts.set_headers(&headers);

                    let request = Request::new_with_str_and_init(&api_url, &opts).unwrap();

                    let window = web_sys::window().unwrap();
                    match JsFuture::from(window.fetch_with_request(&request)).await {
                        Ok(resp_value) => {
                            match resp_value.dyn_into::<Response>() {
                                Ok(resp) => {
                                    match JsFuture::from(resp.text().unwrap()).await {
                                        Ok(text) => {
                                            let text_str = text.as_string().unwrap();

                                            // Parse the JSON response from backend
                                            match serde_json::from_str::<serde_json::Value>(
                                                &text_str,
                                            ) {
                                                Ok(json) => {
                                                    if json["success"].as_bool().unwrap_or(false) {
                                                        let content =
                                                            json["content"].as_str().unwrap_or("");

                                                        // Basic parsing to clean up the transcript text
                                                        let cleaned_text = match mime_type.as_str()
                                                        {
                                                            "text/html" => {
                                                                // For HTML content, we'll sanitize but preserve formatting
                                                                let div = web_sys::window()
                                                                    .unwrap()
                                                                    .document()
                                                                    .unwrap()
                                                                    .create_element("div")
                                                                    .unwrap();
                                                                div.set_inner_html(content);
                                                                div.text_content()
                                                                    .unwrap_or_default()
                                                            }
                                                            _ => {
                                                                // For other formats (VTT, SRT), clean up as before
                                                                content
                                                                    .lines()
                                                                    .filter(|line| {
                                                                        !line.trim().is_empty()
                                                                            && !line
                                                                                .trim()
                                                                                .parse::<i32>()
                                                                                .is_ok()
                                                                            && !line.starts_with(
                                                                                "WEBVTT",
                                                                            )
                                                                            && !line.contains("-->")
                                                                    })
                                                                    .map(|line| {
                                                                        let line = speaker_regex
                                                                            .replace_all(line, "");
                                                                        let line =
                                                                            simple_speaker_regex
                                                                                .replace_all(
                                                                                    &line, "",
                                                                                );
                                                                        line.trim().to_string()
                                                                    })
                                                                    .filter(|line| !line.is_empty())
                                                                    .collect::<Vec<_>>()
                                                                    .join("\n")
                                                            }
                                                        };

                                                        transcript_content.set(Some(cleaned_text));
                                                        loading.set(false);
                                                    } else {
                                                        let error_msg = json["error"]
                                                            .as_str()
                                                            .unwrap_or("Unknown error");
                                                        error.set(Some(format!(
                                                            "Backend error: {}",
                                                            error_msg
                                                        )));
                                                        loading.set(false);
                                                    }
                                                }
                                                Err(e) => {
                                                    error.set(Some(format!(
                                                        "Failed to parse JSON response: {:?}",
                                                        e
                                                    )));
                                                    loading.set(false);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error.set(Some(format!(
                                                "Failed to read response text: {:?}",
                                                e
                                            )));
                                            loading.set(false);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Failed to parse response: {:?}", e)));
                                    loading.set(false);
                                }
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!(
                                "Failed to fetch transcript via backend: {:?}",
                                e
                            )));
                            loading.set(false);
                        }
                    }
                });
            }

            move || ()
        });
    }

    let content_ref = use_node_ref();

    use_effect_with(
        (content_ref.clone(), transcript_content.clone()),
        move |(content_ref, content)| {
            if let Some(content) = content.as_ref() {
                if content.contains('<') && content.contains('>') {
                    if let Some(element) = content_ref.cast::<web_sys::HtmlElement>() {
                        element.set_inner_html(content);
                    }
                }
            }
            || ()
        },
    );

    html! {
        <div class="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50"
             onclick={props.onclose.reform(|_| ())}>

            <div class="item_container-text bg-custom-light dark:bg-custom-dark w-full max-w-3xl max-h-[80vh] rounded-lg shadow-lg p-6 m-4 relative overflow-hidden"
                 onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>

                // Close button
                <button
                    class="absolute right-4 top-4 p-2 rounded-full"
                    onclick={props.onclose.reform(|_| ())}
                >
                    <i class="ph ph-x text-2xl ml-2"></i>
                </button>

                <h2 class="text-2xl font-bold mb-4 pr-12 item_container-text">{&i18n_episode_transcript}</h2>

                <div class="overflow-y-auto max-h-[calc(80vh-8rem)]">
                    if *loading {
                        <div class="flex justify-center items-center h-64">
                            <div class="animate-spin rounded-full h-16 w-16 border-4 border-current border-t-transparent">
                            </div>
                        </div>
                    } else if let Some(err) = &*error {
                        <div class="text-red-500 dark:text-red-400 p-4">
                            {err}
                        </div>
                    } else if let Some(content) = &*transcript_content {
                        <div class="space-y-4 item_container-text prose dark:prose-invert max-w-none">
                            {
                                // If the content contains HTML tags, render it safely
                                if content.contains('<') && content.contains('>') {
                                    html! {
                                        <div class="transcript-content" ref={content_ref}/>
                                    }
                                } else {
                                    html! {
                                        <div>
                                            { for content.split('\n').map(|line| {
                                                html! {
                                                    <p>{line}</p>
                                                }
                                            })}
                                        </div>
                                    }
                                }
                            }
                        </div>
                    }
                </div>
            </div>
        </div>
    }
}

#[derive(Clone, PartialEq)]
enum PageState {
    Loading,
    Ok(crate::requests::episode::Episode),
    Err(String),
}

#[derive(Clone, Copy, PartialEq)]
enum EpisodeTab {
    Notes,
    Chapters,
    Transcript,
    People,
}

/// Build the internal `Transcript` entry that renders a stored AI transcript through the same
/// pipeline as feed transcripts (the backend resolves this URL to SRT).
fn ai_transcript_entry(episode_id: i32) -> Transcript {
    Transcript {
        url: format!("pinepods-internal://transcript/{}", episode_id),
        mime_type: "application/srt".to_string(),
        language: Some("AI".to_string()),
        rel: Some("ai".to_string()),
    }
}

/// Fire a transcription request for `episode_id`, showing a notification and flipping the local
/// state to "running" optimistically. Shared by the desktop tab and the mobile actions.
fn spawn_transcribe(
    server_name: String,
    api_key: Option<String>,
    user_id: i32,
    episode_id: i32,
    submitting: UseStateHandle<bool>,
    transcript: UseStateHandle<Option<StoredTranscript>>,
    started_msg: String,
    error_msg: String,
) {
    submitting.set(true);
    wasm_bindgen_futures::spawn_local(async move {
        match call_transcribe_episode(&server_name, &api_key, episode_id, user_id, false).await {
            Ok(_) => {
                Dispatch::<NotificationState>::global()
                    .reduce_mut(|s| s.info_message = Some(started_msg));
                transcript.set(Some(StoredTranscript {
                    source: "generated".to_string(),
                    language: None,
                    model: None,
                    status: "running".to_string(),
                    full_text: None,
                    segments: None,
                }));
            }
            Err(e) => {
                Dispatch::<NotificationState>::global()
                    .reduce_mut(|s| s.error_message = Some(format!("{}: {}", error_msg, e)));
            }
        }
        submitting.set(false);
    });
}

#[derive(Properties, PartialEq)]
pub struct TranscriptTabProps {
    pub episode_id: i32,
    /// Feed (built-in) transcripts for the episode, if any.
    pub feed_transcripts: Vec<Transcript>,
    pub server_name: String,
    pub api_key: String,
    pub ai_available: bool,
}

/// Unified transcript view for the episode's Transcript tab (#726): shows the built-in transcript
/// if present, an AI transcript if one exists, a source selector when both are available, and a
/// button to generate (or regenerate) an AI transcript when the sidecar is connected.
#[function_component(TranscriptTab)]
pub fn transcript_tab(props: &TranscriptTabProps) -> Html {
    let (i18n, _) = use_translation();
    let (app, _) = use_store::<AppState>();
    let user_id = app.user_details.as_ref().map(|ud| ud.UserID);

    let ai_transcript = use_state(|| Option::<StoredTranscript>::None);
    let selected = use_state(|| 0usize);
    let submitting = use_state(|| false);
    let episode_id = props.episode_id;

    // Load any stored AI transcript for this episode.
    {
        let ai_transcript = ai_transcript.clone();
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();
        use_effect_with((episode_id, user_id), move |(episode_id, user_id)| {
            let episode_id = *episode_id;
            if let Some(user_id) = *user_id {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(t) = call_get_episode_transcript(&server_name, &Some(api_key), user_id, episode_id).await {
                        ai_transcript.set(t);
                    }
                });
            }
            || ()
        });
    }

    // Load detected ad segments for inline review + transcript highlighting (#790).
    let ad_segments = use_state(Vec::<SkipSegment>::new);
    let ad_refresh = use_state(|| 0u32);
    {
        let ad_segments = ad_segments.clone();
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();
        let refresh = *ad_refresh;
        use_effect_with((episode_id, user_id, refresh), move |(episode_id, user_id, _)| {
            let episode_id = *episode_id;
            if let Some(user_id) = *user_id {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(segs) = call_get_episode_skip_segments(&server_name, &Some(api_key), user_id, episode_id).await {
                        ad_segments.set(segs.into_iter().filter(|s| s.kind == "ad").collect());
                    }
                });
            }
            || ()
        });
    }
    let ad_ranges: Vec<(f64, f64)> = ad_segments.iter().map(|s| (s.start_time, s.end_time)).collect();

    let status = ai_transcript.as_ref().map(|t| t.status.clone());
    let ai_complete = status.as_deref() == Some("complete");
    let is_running = matches!(status.as_deref(), Some("running") | Some("pending"));

    // Assemble selectable sources: built-in feed transcript(s) + the AI one (if complete).
    let mut sources: Vec<(String, Transcript)> = Vec::new();
    for t in &props.feed_transcripts {
        let label = t
            .language
            .clone()
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| i18n.t("episode.transcript_builtin").to_string());
        sources.push((label, t.clone()));
    }
    if ai_complete {
        sources.push((i18n.t("episode.transcript_ai").to_string(), ai_transcript_entry(episode_id)));
    }

    let on_transcribe = {
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();
        let submitting = submitting.clone();
        let ai_transcript = ai_transcript.clone();
        let started = i18n.t("episode.transcription_started").to_string();
        let err = i18n.t("episode.transcription_start_error").to_string();
        Callback::from(move |_: MouseEvent| {
            if let Some(user_id) = user_id {
                spawn_transcribe(
                    server_name.clone(), Some(api_key.clone()), user_id, episode_id,
                    submitting.clone(), ai_transcript.clone(), started.clone(), err.clone(),
                );
            }
        })
    };

    let sel_idx = (*selected).min(sources.len().saturating_sub(1));

    // Source selector — only when there's a real choice.
    let selector = if sources.len() > 1 {
        html! {
            <div class="ep-transcript-sources">
                { for sources.iter().enumerate().map(|(i, (label, _))| {
                    let selected = selected.clone();
                    let onclick = Callback::from(move |_: MouseEvent| selected.set(i));
                    let cls = if i == sel_idx { "ep-transcript-source-btn active" } else { "ep-transcript-source-btn" };
                    html! { <button class={cls} {onclick}>{ label.clone() }</button> }
                }) }
            </div>
        }
    } else {
        html! {}
    };

    let content = if let Some((_, t)) = sources.get(sel_idx) {
        html! {
            <TranscriptInline
                transcripts={vec![t.clone()]}
                server_name={props.server_name.clone()}
                api_key={props.api_key.clone()}
                ad_ranges={ad_ranges.clone()}
            />
        }
    } else {
        html! {}
    };

    // Generate / regenerate action (only when the AI sidecar is connected).
    let action = if !props.ai_available {
        html! {}
    } else if is_running {
        html! { <p class="ep-ai-transcript-status ep-transcript-gen">{ i18n.t("episode.transcribing") }</p> }
    } else {
        let (icon, label) = if *submitting {
            ("ph-circle-notch", i18n.t("episode.transcribe_starting"))
        } else if ai_complete {
            ("ph-arrow-clockwise", i18n.t("episode.regenerate_ai_transcript"))
        } else {
            ("ph-text-aa", i18n.t("episode.generate_ai_transcript"))
        };
        html! {
            <button class="ep-ai-transcript-btn ep-transcript-gen" onclick={on_transcribe} disabled={*submitting}>
                <i class={format!("ph {}", icon)}></i>{ label }
            </button>
        }
    };

    // Nothing to show at all (no transcript and can't generate) — render empty.
    if sources.is_empty() && !props.ai_available {
        return html! {};
    }

    // Ad detection (#790): manual trigger + per-user confirm/deny of detected ads.
    let on_detect_ads = {
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();
        let started = i18n.t("episode.ad_detection_started").to_string();
        let err = i18n.t("episode.ad_detection_error").to_string();
        Callback::from(move |_: MouseEvent| {
            if let Some(user_id) = user_id {
                let (server_name, api_key) = (server_name.clone(), api_key.clone());
                let (started, err) = (started.clone(), err.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    match call_detect_ads(&server_name, &Some(api_key), episode_id, user_id, true).await {
                        Ok(_) => Dispatch::<NotificationState>::global().reduce_mut(|s| s.info_message = Some(started)),
                        Err(e) => Dispatch::<NotificationState>::global().reduce_mut(|s| s.error_message = Some(format!("{}: {}", err, e))),
                    }
                });
            }
        })
    };

    let review_cb = {
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();
        let ad_refresh = ad_refresh.clone();
        Callback::from(move |(segment_id, status): (i32, String)| {
            if let Some(user_id) = user_id {
                let (server_name, api_key) = (server_name.clone(), api_key.clone());
                let ad_refresh = ad_refresh.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let req = AdSegmentReviewRequest { segment_id, user_id, status };
                    let _ = call_adjust_ad_segment_review(&server_name, &Some(api_key), &req).await;
                    ad_refresh.set(*ad_refresh + 1);
                });
            }
        })
    };

    let detect_action = if props.ai_available {
        html! {
            <button class="ep-ai-transcript-btn ep-transcript-gen" onclick={on_detect_ads}>
                <i class="ph ph-magnifying-glass"></i>{ i18n.t("episode.detect_ads") }
            </button>
        }
    } else {
        html! {}
    };

    let ad_review = if !ad_segments.is_empty() {
        html! {
            <div class="ep-ad-review">
                <h4 class="ep-ad-review-title">{ i18n.t("episode.detected_ads") }</h4>
                { for ad_segments.iter().map(|seg| {
                    let seg_id = seg.segment_id;
                    let status = seg.status.clone().unwrap_or_default();
                    let range = format!("{} – {}", format_time(seg.start_time as i32), format_time(seg.end_time as i32));
                    let skipping = matches!(status.as_str(), "active" | "confirmed");
                    let confirm = { let r = review_cb.clone(); Callback::from(move |_: MouseEvent| r.emit((seg_id, "confirmed".to_string()))) };
                    let deny = { let r = review_cb.clone(); Callback::from(move |_: MouseEvent| r.emit((seg_id, "rejected".to_string()))) };
                    html! {
                        <div class="ep-ad-review-row">
                            <span class="ep-ad-review-range">{ range }</span>
                            <span class={ if skipping { "ep-ad-status skipping" } else { "ep-ad-status kept" } }>
                                { if skipping { i18n.t("episode.ad_skipping") } else { i18n.t("episode.ad_kept") } }
                            </span>
                            <button class="ep-ad-btn confirm" onclick={confirm} title="Skip this ad">
                                <i class="ph ph-check"></i>
                            </button>
                            <button class="ep-ad-btn deny" onclick={deny} title="Keep this ad">
                                <i class="ph ph-x"></i>
                            </button>
                        </div>
                    }
                }) }
            </div>
        }
    } else {
        html! {}
    };

    html! {
        <div class="ep-pane">
            { selector }
            { content }
            { action }
            { detect_action }
            { ad_review }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct MobileTranscriptActionsProps {
    pub episode_id: i32,
    pub server_name: String,
    pub api_key: String,
    pub ai_available: bool,
    /// Opens the transcript modal with the given source(s).
    pub on_view: Callback<Vec<Transcript>>,
}

/// AI transcript actions for the mobile transcript section: view the AI transcript in the same
/// modal used for feed transcripts, and generate/regenerate one.
#[function_component(MobileTranscriptActions)]
pub fn mobile_transcript_actions(props: &MobileTranscriptActionsProps) -> Html {
    let (i18n, _) = use_translation();
    let (app, _) = use_store::<AppState>();
    let user_id = app.user_details.as_ref().map(|ud| ud.UserID);

    let ai_transcript = use_state(|| Option::<StoredTranscript>::None);
    let submitting = use_state(|| false);
    let episode_id = props.episode_id;

    {
        let ai_transcript = ai_transcript.clone();
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();
        use_effect_with((episode_id, user_id), move |(episode_id, user_id)| {
            let episode_id = *episode_id;
            if let Some(user_id) = *user_id {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(t) = call_get_episode_transcript(&server_name, &Some(api_key), user_id, episode_id).await {
                        ai_transcript.set(t);
                    }
                });
            }
            || ()
        });
    }

    let status = ai_transcript.as_ref().map(|t| t.status.clone());
    let ai_complete = status.as_deref() == Some("complete");
    let is_running = matches!(status.as_deref(), Some("running") | Some("pending"));

    if !props.ai_available && !ai_complete {
        return html! {};
    }

    let on_transcribe = {
        let server_name = props.server_name.clone();
        let api_key = props.api_key.clone();
        let submitting = submitting.clone();
        let ai_transcript = ai_transcript.clone();
        let started = i18n.t("episode.transcription_started").to_string();
        let err = i18n.t("episode.transcription_start_error").to_string();
        Callback::from(move |_: MouseEvent| {
            if let Some(user_id) = user_id {
                spawn_transcribe(
                    server_name.clone(), Some(api_key.clone()), user_id, episode_id,
                    submitting.clone(), ai_transcript.clone(), started.clone(), err.clone(),
                );
            }
        })
    };

    let view_ai = {
        let on_view = props.on_view.clone();
        Callback::from(move |_: MouseEvent| on_view.emit(vec![ai_transcript_entry(episode_id)]))
    };

    html! {
        <>
            { if ai_complete {
                html! {
                    <button class="ep-mobile-transcript-btn" onclick={view_ai}>
                        <i class="ph ph-scroll"></i>{ i18n.t("episode.view_ai_transcript") }
                    </button>
                }
            } else { html! {} } }
            { if is_running {
                html! { <p class="ep-ai-transcript-status">{ i18n.t("episode.transcribing") }</p> }
            } else if props.ai_available {
                let (icon, label) = if *submitting {
                    ("ph-circle-notch", i18n.t("episode.transcribe_starting"))
                } else if ai_complete {
                    ("ph-arrow-clockwise", i18n.t("episode.regenerate_ai_transcript"))
                } else {
                    ("ph-text-aa", i18n.t("episode.generate_ai_transcript"))
                };
                html! {
                    <button class="ep-mobile-transcript-btn" onclick={on_transcribe} disabled={*submitting}>
                        <i class={format!("ph {}", icon)}></i>{ label }
                    </button>
                }
            } else { html! {} } }
        </>
    }
}

#[function_component(Episode)]
pub fn epsiode() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let (episode_detail_state, _) = use_store::<EpisodeDetailState>();
    let (prefs_state, _) = use_store::<UserPreferencesState>();

    // Reactive key derived from the URL query string. yew_router matches only the path
    // (`/episode`), so navigating between `?episode_id=A` and `?episode_id=B` does not remount
    // this component. Subscribing to the location here makes the component re-render on any
    // query change; feeding this key into the metadata fetch effect below re-runs the fetch.
    let ep_query_key = use_location()
        .map(|loc| loc.query_str().to_string())
        .unwrap_or_default();

    // let error = use_state(|| None);
    let shared_url = use_state(|| Option::<String>::None);

    // Capture i18n strings before they get moved
    let i18n_copy_shared_link = i18n.t("episode.copy_shared_link").to_string();
    let i18n_close_modal = i18n.t("common.close_modal").to_string();
    let i18n_copy = i18n.t("common.copy").to_string();
    let i18n_transcript = i18n.t("episode.transcript").to_string();
    let i18n_view_transcript = i18n.t("episode.view_transcript").to_string();
    let i18n_in_this_episode = i18n.t("episode.in_this_episode").to_string();
    let i18n_play = i18n.t("episode.play").to_string();
    let i18n_pause = i18n.t("episode.pause").to_string();
    let i18n_add_to_queue = i18n.t("episode.add_to_queue").to_string();
    let i18n_remove_from_queue = i18n.t("episode.remove_from_queue").to_string();
    let i18n_save = i18n.t("episode.save").to_string();
    let i18n_unsave = i18n.t("episode.unsave").to_string();
    let i18n_failed_to_parse_duration = i18n.t("episode.failed_to_parse_duration").to_string();
    let i18n_no_episode_data_received = i18n.t("episode.no_episode_data_received").to_string();
    let i18n_download = i18n.t("episode.download").to_string();
    let i18n_remove_download = i18n.t("episode.remove_download").to_string();
    let i18n_mark_complete = i18n.t("episode.mark_complete").to_string();
    let i18n_mark_incomplete = i18n.t("episode.mark_incomplete").to_string();
    let i18n_mark_episode_complete = i18n.t("episode.mark_episode_complete").to_string();
    let i18n_mark_episode_incomplete = i18n.t("episode.mark_episode_incomplete").to_string();
    let i18n_share_episode = i18n.t("episode.share_episode").to_string();
    let i18n_download_episode = i18n.t("episode.download_episode").to_string();
    let i18n_downloading = i18n.t("episode.downloading").to_string();
    let i18n_share_link_description = i18n.t("episode.share_link_description").to_string();
    let i18n_current_page_link_description =
        i18n.t("episode.current_page_link_description").to_string();
    let i18n_add_podcast_to_enable_actions =
        i18n.t("episode.add_podcast_to_enable_actions").to_string();
    let i18n_this_item_contains_no_media =
        i18n.t("episode.this_item_contains_no_media").to_string();
    let i18n_unable_to_display_episode = i18n.t("episode.unable_to_display_episode").to_string();
    let i18n_something_went_wrong_description = i18n
        .t("episode.something_went_wrong_description")
        .to_string();
    let i18n_cover_alt_text = i18n.t("episode.cover_alt_text").to_string();
    let i18n_failed_to_find_podcast_metadata =
        i18n.t("failed_to_find_podcast_metadata").to_string();
    let i18n_no_hosts_found = i18n.t("host_component.no_hosts_found").to_string();
    let i18n_add_hosts_here = i18n.t("host_component.add_hosts_here").to_string();

    let (app_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();

    let api_key = app_state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = app_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = app_state
        .auth_details
        .as_ref()
        .map(|ud: &crate::requests::login_requests::LoginServerRequest| ud.server_name.clone());
    let history = BrowserHistory::new();

    //let episode_id = state.selected_episode_id.clone();
    let ep_in_db = use_state(|| false);
    let ep_2_loading = use_state(|| true);

    // Whether the optional AI sidecar is connected — gates the Transcript tab + generate action.
    let ai_available = use_state(|| false);
    {
        let ai_available = ai_available.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        use_effect_with((), move |_| {
            if let (Some(server_name), Some(api_key)) = (server_name, api_key) {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(up) = call_get_ai_status(&server_name, &api_key).await {
                        ai_available.set(up);
                    }
                });
            }
            || ()
        });
    }

    let page_state = use_state(|| PageState::Loading);

    {
        let audio_dispatch = audio_dispatch.clone();

        // Initial check when the component is mounted
        {
            let window = window().unwrap();
            let width = window.inner_width().unwrap().as_f64().unwrap();
            let new_is_mobile = width < 768.0;
            audio_dispatch.reduce_mut(|state| state.is_mobile = Some(new_is_mobile));
        }

        // Resize event listener
        use_effect_with((), move |_| {
            let window = window().unwrap();
            let closure_window = window.clone();
            let closure = Closure::wrap(Box::new(move || {
                let width = closure_window.inner_width().unwrap().as_f64().unwrap();
                let new_is_mobile = width < 768.0;
                audio_dispatch.reduce_mut(|state| state.is_mobile = Some(new_is_mobile));
            }) as Box<dyn Fn()>);

            window
                .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
                .unwrap();

            closure.forget(); // Ensure the closure is not dropped prematurely

            || ()
        });
    }

    // gets podcast 2.0 data from server. Not available for youtube episodes
    let setup_podcast_2_data = {
        let (_, audio_dispatch) = use_store::<UIState>();
        let user_id = user_id.clone().unwrap();
        let api_key = api_key.clone().unwrap();
        let server_name = server_name.clone().unwrap();
        let ep_2_loading = ep_2_loading.clone();

        move |episode_id: i32| {
            // Use fetched_episode_id directly since we already have it
            let chap_request = FetchPodcasting2DataRequest {
                episode_id: episode_id,
                user_id: user_id,
            };
            let ep_2_loading = ep_2_loading.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match call_fetch_podcasting_2_data(&server_name, &api_key, &chap_request).await {
                    Ok(response) => {
                        audio_dispatch.reduce_mut(|state| {
                            state.episode_page_transcript = Some(response.transcripts);
                            state.episode_page_people = Some(response.people);
                            state.episode_page_chapters = Some(response.chapters);
                        });
                        ep_2_loading.set(false);
                    }
                    Err(e) => {
                        web_sys::console::log_1(
                            &format!("Error fetching podcast 2.0 data: {}", e).into(),
                        );
                        audio_dispatch.reduce_mut(|state| {
                            state.episode_page_transcript = None;
                            state.episode_page_people = None;
                            state.episode_page_chapters = None;
                        });
                        ep_2_loading.set(false);
                    }
                }
            })
        }
    };

    // Tries to get episode metadata from server db
    // If successful, the page state is set to Ok and the ep_in_db flag is set to true
    let get_episode_from_id = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let page_state = page_state.setter();
        let ep_in_db = ep_in_db.clone();
        let window = web_sys::window().expect("Window should exist");

        move |episode_id: i32, person_ep: bool, youtube: bool| {
            // On a clean loading of this page these vars may not exist yet
            // Not existing is not an error, just ignore them until the next render
            let server_name = match server_name {
                Some(s) => s,
                None => {
                    page_state.set(PageState::Loading);
                    return;
                }
            };

            let api_key = match api_key {
                Some(s) => s,
                None => {
                    page_state.set(PageState::Loading);
                    return;
                }
            };

            let user_id = match user_id {
                Some(s) => s,
                None => {
                    page_state.set(PageState::Loading);
                    return;
                }
            };

            spawn_local(async move {
                let req = &EpisodeRequest {
                    episode_id: episode_id,
                    user_id: user_id,
                    person_episode: person_ep,
                    is_youtube: youtube,
                };

                match call_get_episode_metadata(&server_name, api_key, req).await {
                    Ok(ep) => {
                        ep_in_db.set(true);
                        if !youtube {
                            setup_podcast_2_data(episode_id);
                        };
                        page_state.set(PageState::Ok(ep));

                        let mut new_url = window.location().origin().unwrap();
                        if youtube {
                            new_url.push_str(&format!("/episode?episode_id={}&youtube=true", episode_id));
                        } else {
                            new_url.push_str(&format!("/episode?episode_id={}", episode_id));
                        }

                        if window.location().to_string() != new_url {
                            window
                                .history()
                                .expect("Window should have History")
                                .replace_state_with_url(
                                    &wasm_bindgen::JsValue::NULL,
                                    "",
                                    Some(&new_url),
                                )
                                .expect("History should accept url");
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Err(e.to_string()));
                    }
                }
            });
        }
    };

    let get_episode_from_params = {
        let server_name = server_name.clone().unwrap();
        let api_key = api_key.clone().flatten();
        let page_state = page_state.setter();
        let window = web_sys::window().expect("Window should exist");
        let _dispatch = dispatch.clone();

        move |title: &str,
              podcast_url: &str,
              audio_url: &str,
              podcast_index_id: i32,
              is_youtube: bool| {
            let title = title.to_string();
            let episode_url = podcast_url.to_string();
            let audio_url = audio_url.to_string();
            spawn_local(async move {
                match call_parse_podcast_url(server_name, &api_key, &episode_url).await {
                    Ok(result) => {
                        if let Some(ep) = result
                            .episodes
                            .iter()
                            .find(|ep| ep.episodeurl == audio_url)
                            .cloned()
                        {
                            // TODO: if id is found and valid, switch to get_episode_from_id?
                            if ep.episodeduration != 0 {
                                page_state.set(PageState::Ok(ep.clone()));
                                let ep_url = episode_url.clone();
                                let aud_url = audio_url.clone();
                                let podcast_title = title.clone();
                                let ep_for_detail = ep.clone();

                                Dispatch::<EpisodeNavigationState>::global().reduce_mut(move |s| {
                                    s.selected_episode_id = Some(0);
                                    s.selected_episode_url = Some(ep_url.to_string());
                                    s.selected_episode_audio_url = Some(aud_url.to_string());
                                    s.selected_podcast_title = Some(podcast_title.to_string());
                                });
                                Dispatch::<EpisodeDetailState>::global().reduce_mut(move |s| {
                                    s.fetched_episode = Some(ep_for_detail);
                                });

                                // Update the URL with the parameters
                                let mut new_url = window.location().origin().unwrap();
                                new_url.push_str(&window.location().pathname().unwrap());
                                new_url.push_str("?podcast_title=");
                                new_url.push_str(&urlencoding::encode(&title));
                                new_url.push_str("&episode_url=");
                                new_url.push_str(&urlencoding::encode(&episode_url));
                                new_url.push_str("&audio_url=");
                                new_url.push_str(&urlencoding::encode(&audio_url));
                                new_url.push_str("&podcast_index_id=");
                                new_url.push_str(&podcast_index_id.to_string());
                                new_url.push_str("&is_youtube=");
                                new_url.push_str(&is_youtube.to_string());

                                window
                                    .history()
                                    .expect("should have a history")
                                    .replace_state_with_url(
                                        &wasm_bindgen::JsValue::NULL,
                                        "",
                                        Some(&new_url),
                                    )
                                    .expect("should push state");
                            } else {
                                page_state
                                    .set(PageState::Err(i18n_failed_to_parse_duration.clone()));
                            }
                        }
                    }
                    Err(e) => {
                        page_state.set(PageState::Err(e.to_string()));
                    }
                }
            });
        }
    };

    // Gather episode metadata, either from db or url params
    {
        let page_state = page_state.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        // Depend on the query key too so switching episodes on the same route re-fetches.
        use_effect_with((server_name.clone(), ep_query_key.clone()), move |_| {
            let window = web_sys::window().expect("no global window exists");
            let search_params = window.location().search().unwrap();
            let url_params = UrlSearchParams::new_with_str(&search_params).unwrap();

            // if id is passed, get metadata from db
            if let Some(id_param) = url_params.get("episode_id") {
                match id_param.parse::<i32>() {
                    Ok(ep_id) => {
                        web_sys::console::log_1(
                            &format!("Getting metadata for episode {}", ep_id).into(),
                        );
                        let youtube_episode = url_params
                            .get("youtube")
                            .and_then(|s| s.parse::<bool>().ok())
                            .unwrap_or(false);
                        let person_episode = url_params
                            .get("person")
                            .and_then(|s| s.parse::<bool>().ok())
                            .unwrap_or(false);

                        get_episode_from_id(ep_id, person_episode, youtube_episode);
                    }
                    Err(e) => {
                        page_state.set(PageState::Err(e.to_string()));
                        return;
                    }
                };
            } else {
                // If no episode_id, try anything else provided as params
                let podcast_title = url_params.get("podcast_title").unwrap_or_default();
                let episode_url = url_params.get("episode_url").unwrap_or_default();
                let audio_url = url_params.get("audio_url").unwrap_or_default();
                let podcast_index_id = url_params
                    .get("podcast_index_id")
                    .and_then(|id| id.parse::<i32>().ok())
                    .unwrap_or(0);
                let is_youtube = url_params // Add this
                    .get("is_youtube")
                    .and_then(|v| v.parse::<bool>().ok())
                    .unwrap_or(false);

                let server_name = match server_name {
                    Some(s) => s,
                    None => {
                        page_state.set(PageState::Loading);
                        return;
                    }
                };

                let api_key = match api_key {
                    Some(s) => s,
                    None => {
                        page_state.set(PageState::Loading);
                        return;
                    }
                };

                let user_id = match user_id {
                    Some(s) => s,
                    None => {
                        page_state.set(PageState::Loading);
                        return;
                    }
                };

                web_sys::console::log_1(
                    &format!("!!!!! {} | {} | {}", podcast_title, episode_url, audio_url).into(),
                );

                // check if episode is in database, then get info via episode_id
                if !podcast_title.is_empty() && !episode_url.is_empty() && !audio_url.is_empty() {
                    wasm_bindgen_futures::spawn_local(async move {
                        // First, try to get the episode ID
                        match call_get_episode_id(
                            &server_name,
                            &api_key.clone().unwrap(),
                            &user_id,
                            &podcast_title,
                            &audio_url,
                            is_youtube,
                        )
                        .await
                        {
                            Ok(ep_id) => {
                                get_episode_from_id(ep_id, false, is_youtube);
                                return;
                            }
                            Err(_) => {
                                get_episode_from_params(
                                    &podcast_title,
                                    &episode_url,
                                    &audio_url,
                                    podcast_index_id,
                                    is_youtube,
                                );
                                return;
                            }
                        }
                    });
                } else {
                    page_state.set(PageState::Err(
                        i18n_failed_to_find_podcast_metadata.clone(),
                    ));
                    return;
                }
            }
        });
    }

    let active_tab = use_state(|| EpisodeTab::Notes);
    let completion_status = use_state(|| false);
    let queue_status = use_state(|| false);
    let save_status = use_state(|| false);
    let download_status = use_state(|| false);
    let download_in_progress = use_state(|| false);

    {
        let completion_status = completion_status.clone();
        let queue_status = queue_status.clone();
        let save_status = save_status.clone();
        let download_status = download_status.clone();
        let page_state = page_state.clone();

        // TODO: use app state for these values?
        use_effect_with(page_state.clone(), move |_| {
            if let PageState::Ok(episode) = &*page_state {
                completion_status.set(episode.completed);
                queue_status.set(episode.queued);
                save_status.set(episode.saved);
                download_status.set(episode.downloaded);
            } else {
                web_sys::console::log_1(&i18n_no_episode_data_received.clone().into());
            }
            || ()
        });
    }

    let modal_state = use_state(|| ModalState::Hidden);

    // Represents visibility of popup modals in render
    #[derive(Clone, PartialEq)]
    enum ModalState {
        Hidden,
        ShowShare,
    }

    let on_close_modal = {
        let modal_state = modal_state.clone();
        Callback::from(move |_| {
            modal_state.set(ModalState::Hidden);
        })
    };

    let on_background_click = {
        let on_close_modal = on_close_modal.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            if element.tag_name() == "DIV" {
                on_close_modal.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let shared_url_copy = shared_url.clone();
    let create_share_link = {
        let api_key = api_key.clone();
        let server_name: Option<String> = server_name.clone();
        let page_state_create = modal_state.clone(); // Handle modal visibility
        let shared_url_create = shared_url_copy.clone(); // Store the created shared link
        let page_state = page_state.clone();

        Callback::from(move |_| {
            let episode_id = if let PageState::Ok(episode) = &*page_state {
                episode.episodeid
            } else {
                web_sys::console::log_1(
                    &("Cannot create share link -- PageState is not Ok").into(),
                );
                return;
            };

            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let shared_url_copy = shared_url_create.clone();
            let page_state_copy = page_state_create.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let api_key_copy = api_key.clone();
                if let (Some(_api_key), Some(server_name)) =
                    (api_key.as_ref(), server_name.as_ref())
                {
                    match call_create_share_link(
                        &server_name,
                        &api_key_copy.unwrap().unwrap(),
                        episode_id,
                    )
                    .await
                    {
                        Ok(url_key) => {
                            let full_url = format!("{}/shared_episode/{}", server_name, url_key);
                            shared_url_copy.set(Some(full_url)); // Store the generated link
                            page_state_copy.set(ModalState::ShowShare); // Show the modal
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error creating share link: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    let download_episode_file = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let download_in_progress = download_in_progress.clone();

        Callback::from(move |episode_id: i32| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let download_in_progress = download_in_progress.clone();

            // Set download in progress state
            download_in_progress.set(true);

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(_api_key), Some(server_name)) =
                    (api_key.as_ref(), server_name.as_ref())
                {
                    match call_download_episode_file(&server_name, &api_key.unwrap(), episode_id)
                        .await
                    {
                        Ok(_) => {
                            web_sys::console::log_1(&"Episode download started".into());
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error downloading episode file: {}", e).into(),
                            );
                        }
                    }
                }
                // Clear download in progress state
                download_in_progress.set(false);
            });
        })
    };

    // Define the modal for showing the shareable link
    let share_url_modal = {
        let shared_url_copy = {
            let shared_url = shared_url.clone();
            Callback::from(move |_| {
                if let Some(url) = shared_url.as_ref() {
                    if let Some(window) = web_sys::window() {
                        let _ = window.navigator().clipboard().write_text(url);
                    }
                }
            })
        };

        let current_url_copy = {
            let current_url = get_current_url();
            Callback::from(move |_| {
                if let Some(window) = web_sys::window() {
                    let _ = window.navigator().clipboard().write_text(&current_url);
                }
            })
        };

        html! {
            <div id="share_url_modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
                <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                    <div class="modal-container relative rounded-lg shadow">
                        <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                            <h3 class="text-xl font-semibold">
                                {&i18n_copy_shared_link}
                            </h3>
                            <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                                <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                                </svg>
                                <span class="sr-only">{&i18n_close_modal}</span>
                            </button>
                        </div>
                        <div class="p-4 md:p-5">
                            <div>
                                <label for="share_link" class="block mb-2 text-sm font-medium">{&i18n_share_link_description}</label>
                                <div class="relative">
                                    <input
                                        type="text"
                                        id="share_link"
                                        class="input-black w-full px-3 py-2 border border-gray-300 rounded-md pr-20"
                                        value={shared_url.as_ref().map(|url| url.clone()).unwrap_or_else(|| "".to_string())}
                                        readonly=true
                                    />
                                    <button
                                        onclick={shared_url_copy}
                                        class="absolute right-2 top-1/2 transform -translate-y-1/2 px-4 py-1 text-sm text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300"
                                    >
                                        {&i18n_copy}
                                    </button>
                                </div>
                            </div>
                            <div class="mt-4">
                                <label for="current_link" class="block mb-2 text-sm font-medium">{&i18n_current_page_link_description}</label>
                                <div class="relative">
                                    <input
                                        type="text"
                                        id="current_link"
                                        class="input-black w-full px-3 py-2 border border-gray-300 rounded-md pr-20"
                                        value={get_current_url()}
                                        readonly=true
                                    />
                                    <button
                                        onclick={current_url_copy}
                                        class="absolute right-2 top-1/2 transform -translate-y-1/2 px-4 py-1 text-sm text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300"
                                    >
                                        {&i18n_copy}
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        }
    };

    let page_state = page_state.clone();
    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                match *modal_state {
                ModalState::ShowShare => share_url_modal,
                _ => html! {},
                }
            }
            {
                match &*page_state{
                    PageState::Loading => {
                        html! { <Loading/> }
                    },
                    PageState::Err(e) => {
                        web_sys::console::log_1(&format!("Error: {}", e).into());
                        let mut msg = i18n_something_went_wrong_description.clone();
                        msg.push_str("<br/><br/>");
                        msg.push_str(e);
                        empty_message(
                            &i18n_unable_to_display_episode,
                            &msg
                        )
                    },
                    PageState::Ok(episode) => {
                        let _episode_id = episode.episodeid;
                        let episode_url_clone = episode.episodeurl.clone();
                        let _episode_title_clone = episode.episodetitle.clone();
                        let _episode_descripton_clone = episode.episodedescription.clone();
                        let _episode_release_clone = episode.episodepubdate.clone();
                        let _episode_artwork_clone = episode.episodeartwork.clone();
                        let _episode_duration_clone = episode.episodeduration.clone();
                        let podcast_of_episode = episode.podcastid.clone();
                        let _episode_listened_clone = episode.listenduration.clone();
                        let episode_id = episode.episodeid;
                        let episode_is_youtube = episode.is_youtube.clone();

                        let sanitized_description = sanitize_html_with_blank_target(&episode.episodedescription.clone());
                        let description = sanitized_description;

                        let episode_id = episode_id.clone();

                        let user_id_play = user_id.clone();
                        let server_name_play = server_name.clone();
                        let api_key_play = api_key.clone();
                        let audio_dispatch = audio_dispatch.clone();

                        let is_playing = {
                            let audio_state = audio_state.clone();
                            let episode_id = episode_id;

                            audio_state.currently_playing.as_ref().map_or(false, |current| {
                                current.episode_id == episode_id && audio_state.audio_playing.unwrap_or(false)
                            })
                        };

                        // Create the play toggle handler
                        let handle_play_click = {
                            let audio_state = audio_state.clone();
                            let audio_dispatch = audio_dispatch.clone();
                            let episode_id = episode_id.clone();

                            // Create the base on_play_click callback
                            let on_play = on_play_click(
                                episode.clone(),
                                api_key_play.unwrap().unwrap(),
                                user_id_play.unwrap(),
                                server_name_play.unwrap(),
                                audio_dispatch.clone(),
                                audio_state.clone(),
                                false,
                                false,
                                None,
                            );

                            Callback::from(move |e: MouseEvent| {
                                let is_this_episode_loaded = audio_state.currently_playing.as_ref()
                                    .map_or(false, |current| current.episode_id == episode_id);

                                if is_this_episode_loaded {
                                    // If this episode is loaded, just toggle playback
                                    audio_dispatch.reduce_mut(|state| {
                                        state.toggle_playback();
                                    });
                                } else {
                                    // If this episode isn't loaded, use the base on_play_click
                                    on_play.emit(e);
                                }
                            })
                        };

                        let complete_server_name = server_name.clone();
                        let complete_api_key = api_key.clone();
                        let complete_post = dispatch.clone();
                        let complete_status_clone = completion_status.clone();
                        let user_id_complete = user_id.clone();

                        let on_complete_episode = {
                            Callback::from(move |_| {
                                let completion_status = complete_status_clone.clone();
                                let _post_dispatch = complete_post.clone();
                                let server_name_copy = complete_server_name.clone();
                                let api_key_copy = complete_api_key.clone();
                                let is_youtube = episode_is_youtube;
                                let request = MarkEpisodeCompletedRequest {
                                    episode_id: episode_id,
                                    user_id: user_id_complete.unwrap(), // replace with the actual user ID
                                    is_youtube,
                                };
                                let server_name = server_name_copy; // replace with the actual server name
                                let api_key = api_key_copy; // replace with the actual API key
                                let future = async move {
                                    // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                                    match call_mark_episode_completed(
                                        &server_name.unwrap(),
                                        &api_key.flatten(),
                                        &request,
                                    )
                                    .await
                                    {
                                        Ok(_success_message) => {
                                            completion_status.set(true);
                                            Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                                                if !state.completed_episodes.remove(&episode_id) {
                                                    state.completed_episodes.insert(episode_id);
                                                }
                                            });
                                        }
                                        Err(e) => {
                                            let formatted_error = format_error_message(&e.to_string());
                                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                state.error_message = Option::from(format!("{}", formatted_error))
});
                                            // Handle error, e.g., display the error message
                                        }
                                    }
                                };
                                wasm_bindgen_futures::spawn_local(future);
                                // dropdown_open.set(false);
                            })
                        };

                        let uncomplete_server_name = server_name.clone();
                        let uncomplete_api_key = api_key.clone();
                        let uncomplete_post = dispatch.clone();
                        let user_id_uncomplete = user_id.clone();
                        let uncomplete_status_clone = completion_status.clone();
                        let is_youtube = episode_is_youtube;

                        let on_uncomplete_episode = {
                            Callback::from(move |_| {
                                let completion_status = uncomplete_status_clone.clone();
                                let _post_dispatch = uncomplete_post.clone();
                                let server_name_copy = uncomplete_server_name.clone();
                                let api_key_copy = uncomplete_api_key.clone();
                                let request = MarkEpisodeCompletedRequest {
                                    episode_id: episode_id,
                                    user_id: user_id_uncomplete.unwrap(), // replace with the actual user ID
                                    is_youtube,
                                };
                                let server_name = server_name_copy; // replace with the actual server name
                                let api_key = api_key_copy; // replace with the actual API key
                                let future = async move {
                                    // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                                    match call_mark_episode_uncompleted(
                                        &server_name.unwrap(),
                                        &api_key.flatten(),
                                        &request,
                                    )
                                    .await
                                    {
                                        Ok(_message) => {
                                            completion_status.set(false);
                                            Dispatch::<EpisodeStatusState>::global().reduce_mut(|state| {
                                                if !state.completed_episodes.remove(&episode_id) {
                                                    state.completed_episodes.insert(episode_id);
                                                }
                                            });
                                        }
                                        Err(e) => {
                                            let formatted_error = format_error_message(&e.to_string());
                                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                state.error_message = Option::from(format!("{}", formatted_error))
});
                                            // Handle error, e.g., display the error message
                                        }
                                    }
                                };
                                wasm_bindgen_futures::spawn_local(future);
                                // dropdown_open.set(false);
                            })
                        };

                        let toggle_completion = {
                            let completion_status = completion_status.clone();
                            Callback::from(move |_| {
                                // Toggle the completion status
                                if *completion_status {
                                    on_uncomplete_episode.emit(());
                                } else {
                                    on_complete_episode.emit(());
                                }
                            })
                        };

                        // First, create all the toggle functions
                        let queue_status = queue_status.clone();
                        let save_status = save_status.clone();
                        let download_status = download_status.clone();

                        let toggle_queue = {
                            let queue_status = queue_status.clone();
                            let server_name_queue = server_name.clone();
                            let api_key_queue = api_key.clone();
                            let user_id_queue = user_id.clone();
                            let dispatch_queue = _post_dispatch.clone();
                            let episode_id = episode_id;
                            let is_youtube = episode_is_youtube;

                            Callback::from(move |_: MouseEvent| {
                                let server_name_copy = server_name_queue.clone();
                                let api_key_copy = api_key_queue.clone();
                                let _queue_post = dispatch_queue.clone();
                                let queue_status = queue_status.clone();
                                let is_queued = *queue_status;
                                let request = QueuePodcastRequest {
                                    episode_id,
                                    user_id: user_id_queue.unwrap(),
                                    is_youtube,
                                };

                                let future = async move {
                                    let result = if is_queued {
                                        call_remove_queued_episode(&server_name_copy.unwrap(), &api_key_copy.flatten(), &request).await
                                    } else {
                                        call_queue_episode(&server_name_copy.unwrap(), &api_key_copy.flatten(), &request).await
                                    };

                                    match result {
                                        Ok(_success_message) => {
                                            queue_status.set(!is_queued); // Toggle the state after successful API call
                                        },
                                        Err(e) => {
                                            let formatted_error = format_error_message(&e.to_string());
                                            Dispatch::<NotificationState>::global().reduce_mut(|state| state.error_message = Some(format!("{}", formatted_error)));
                                        }
                                    }
                                };
                                wasm_bindgen_futures::spawn_local(future);
                            })
                        };

                        let toggle_save = {
                            let save_status = save_status.clone();
                            let saved_server_name = server_name.clone();
                            let saved_api_key = api_key.clone();
                            let save_post = _post_dispatch.clone();
                            let user_id_save = user_id.clone();
                            let episode_id = episode_id;
                            let is_youtube = episode_is_youtube;

                            Callback::from(move |_: MouseEvent| {
                                let server_name_copy = saved_server_name.clone();
                                let api_key_copy = saved_api_key.clone();
                                let _post_state = save_post.clone();
                                let is_saved = *save_status;
                                let save_status = save_status.clone();
                                let request = SavePodcastRequest {
                                    episode_id,
                                    user_id: user_id_save.unwrap(),
                                    is_youtube,
                                };

                                let future = async move {
                                    let result = if is_saved {
                                        call_remove_saved_episode(&server_name_copy.unwrap(), &api_key_copy.flatten(), &request).await
                                    } else {
                                        call_save_episode(&server_name_copy.unwrap(), &api_key_copy.flatten(), &request).await
                                    };

                                    match result {
                                        Ok(_success_message) => {
                                            save_status.set(!is_saved); // Toggle the state after successful API call
                                        },
                                        Err(e) => {
                                            let formatted_error = format_error_message(&e.to_string());
                                            Dispatch::<NotificationState>::global().reduce_mut(|state| state.error_message = Some(format!("{}", formatted_error)));
                                        }
                                    }
                                };
                                wasm_bindgen_futures::spawn_local(future);
                            })
                        };

                        let toggle_download = {
                            let download_status = download_status.clone();
                            let download_server_name = server_name.clone();
                            let download_api_key = api_key.clone();
                            let download_post = _post_dispatch.clone();
                            let user_id_download = user_id.clone();

                            Callback::from(move |_: MouseEvent| {
                                let server_name_copy = download_server_name.clone();
                                let api_key_copy = download_api_key.clone();
                                let _post_state = download_post.clone();
                                let is_downloaded = *download_status;
                                let download_status = download_status.clone();
                                let request = DownloadEpisodeRequest {
                                    episode_id,
                                    user_id: user_id_download.unwrap(),
                                    is_youtube,
                                };

                                // Set loading state
                                Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(true));

                                let future = async move {
                                    let result = if is_downloaded {
                                        call_remove_downloaded_episode(&server_name_copy.unwrap(), &api_key_copy.flatten(), &request).await
                                    } else {
                                        call_download_episode(&server_name_copy.unwrap(), &api_key_copy.flatten(), &request).await
                                    };

                                    match result {
                                        Ok(_success_message) => {
                                            download_status.set(!is_downloaded); // Toggle the state after successful API call
                                        },
                                        Err(e) => {
                                            let formatted_error = format_error_message(&e.to_string());
                                            Dispatch::<NotificationState>::global().reduce_mut(|state| state.error_message = Some(format!("{}", formatted_error)));
                                        }
                                    }

                                    // Clear loading state
                                    Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(false));
                                };
                                wasm_bindgen_futures::spawn_local(future);
                            })
                        };

                        let datetime = parse_date(&episode.episodepubdate, &prefs_state.user_tz);
                        let date_format = match_date_format(prefs_state.date_format.as_deref());
                        let format_duration = format_time(episode.episodeduration);
                        let format_release = format!("{}", format_datetime(&datetime, &prefs_state.hour_preference, date_format));

                        let on_title_click = {
                            let dispatch = dispatch.clone();
                            let server_name = server_name.clone();
                            let api_key = api_key.clone();
                            let podcast_id = podcast_of_episode.clone();
                            let user_id = user_id.clone();
                            let history = history.clone();

                            // Get the URL parameters for fallback
                            let window = web_sys::window().expect("no global window exists");
                            let search_params = window.location().search().unwrap();
                            let url_params = web_sys::UrlSearchParams::new_with_str(&search_params).unwrap();
                            let podcast_title = url_params.get("podcast_title").unwrap_or_default();
                            let podcast_url = url_params.get("episode_url").unwrap_or_default();

                            Callback::from(move |event: MouseEvent| {
                                let _dispatch = dispatch.clone();
                                let server_name = server_name.clone();
                                let api_key = api_key.clone();
                                let podcast_id = podcast_id.clone();
                                let user_id = user_id.clone();
                                let history = history.clone();
                                let podcast_title = podcast_title.clone();
                                let podcast_url = podcast_url.clone();

                                wasm_bindgen_futures::spawn_local(async move {
                                    // Try the regular call first if we have a non-zero podcast_id
                                    let result = if podcast_id != 0 {
                                        pod_req::call_get_podcast_details(
                                            &server_name.clone().unwrap(),
                                            &api_key.clone().unwrap().unwrap(),
                                            user_id.unwrap(),
                                            podcast_id
                                        ).await
                                    } else {
                                        let is_added = call_check_podcast(
                                            &server_name.clone().unwrap(),
                                            &api_key.clone().unwrap().unwrap(),
                                            user_id.unwrap(),
                                            &podcast_title,
                                            &podcast_url,
                                        ).await
                                        .map(|response| response.exists)
                                        .unwrap_or(false);

                                        // Fallback to dynamic call if podcast_id is 0
                                        call_get_podcast_details_dynamic(
                                            &server_name.clone().unwrap(),
                                            &api_key.clone().unwrap().unwrap(),
                                            user_id.unwrap(),
                                            &podcast_title,
                                            &podcast_url,
                                            0,
                                            is_added,  // Use the result from check_podcast
                                            Some(false),
                                        ).await.map(|response| response.details.into_podcast_details())
                                    };

                                    match result {
                                        Ok(details) => {
                                            let final_click_action = create_on_title_click(
                                                server_name.unwrap(),
                                                api_key,
                                                &history,
                                                details.podcastid,
                                                details.podcastindexid,
                                                details.podcastname,
                                                details.feedurl,
                                                details.description,
                                                details.author,
                                                details.artworkurl,
                                                details.explicit,
                                                details.episodecount,
                                                Some(details.categories.values().cloned().collect::<Vec<_>>().join(", ")),
                                                details.websiteurl,
                                                user_id.unwrap(),
                                                details.is_youtube, // assuming we renamed this field
                                            );
                                            final_click_action.emit(event);
                                        },
                                        Err(error) => {
                                            web_sys::console::log_1(&format!(
                                                "Error fetching podcast details: {}", error
                                            ).into());
                                            let formatted_error = format_error_message(&error.to_string());
                                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                state.error_message = Some(format!(
                                                    "Failed to load details: {}", formatted_error
                                                ));
                                            });
                                        }
                                    }
                                });
                            })
                        };
                        let episode_url_check = episode_url_clone;
                        let should_show_buttons = !episode_url_check.is_empty();

                        // Before creating the play toggle handler, add this:
                        let listen_duration_percentage =
                            if episode.episodeduration > 0 {
                                ((episode.listenduration as f64 / episode.episodeduration as f64) * 100.0).min(100f64)
                        } else {
                            0.0
                        };

                        // In the Episode component rendering logic, add this where appropriate:
                        let progress_bar = if episode.listenduration != 0 {
                            if episode.listenduration > 0 {
                                let listen_duration_formatted = format_time(episode.listenduration);
                                html! {
                                    <div class="flex flex-col space-y-1 mt-2">
                                        <div class="flex items-center space-x-2">
                                            <span class="item_container-text">{listen_duration_formatted}</span>
                                            <div class="progress-bar-container">
                                                <div class="progress-bar" style={format!("width: {}%;", listen_duration_percentage)}></div>
                                            </div>
                                            <span class="item_container-text">{&format_duration}</span>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        } else {
                            html! {}
                        };

                        let layout = if audio_state.is_mobile.unwrap_or(false) {
                            let server_for_proxy = server_name.clone().unwrap_or_default();
                            let history_for_host = history.clone();
                            html! {
                                <div class="mobile-layout ep-mobile-page">

                                    // ── Compact header: artwork + meta ──
                                    <div class="ep-mobile-header">
                                        <FallbackImage
                                            src={episode.episodeartwork.clone()}
                                            alt={format!("{}{}", i18n_cover_alt_text, episode.episodeartwork.clone())}
                                            class="ep-mobile-art rounded-corners"
                                        />
                                        <div class="ep-mobile-meta">
                                            <p class="ep-mobile-podcast-name" onclick={on_title_click.clone()}>
                                                { &episode.podcastname }
                                            </p>
                                            <h2 class="ep-mobile-title">
                                                { &episode.episodetitle }
                                                { if *completion_status {
                                                    html! { <i class="ph ph-check-circle ep-mobile-complete-badge"></i> }
                                                } else { html! {} }}
                                            </h2>
                                            <p class="ep-mobile-subinfo">
                                                { format!("{} · {}", format_duration, format_release) }
                                            </p>
                                        </div>
                                    </div>

                                    // ── Progress bar ──
                                    <div class="ep-mobile-progress">
                                        { progress_bar }
                                    </div>

                                    // ── Icon action bar ──
                                    { if should_show_buttons {
                                        html! {
                                            <div class="ep-mobile-actions">
                                                <button
                                                    onclick={handle_play_click}
                                                    class="ep-mobile-action-btn ep-mobile-action-play"
                                                    title={if is_playing { i18n_pause.clone() } else { i18n_play.clone() }}
                                                >
                                                    { if is_playing {
                                                        html! { <i class="ph ph-pause"></i> }
                                                    } else {
                                                        html! { <i class="ph ph-play"></i> }
                                                    }}
                                                </button>
                                                { if *ep_in_db {
                                                    html! {
                                                        <>
                                                        <button
                                                            onclick={toggle_queue}
                                                            class={classes!("ep-mobile-action-btn", (*queue_status).then_some("ep-mobile-active"))}
                                                            title={if *queue_status { i18n_remove_from_queue.clone() } else { i18n_add_to_queue.clone() }}
                                                        >
                                                            <i class="ph ph-queue"></i>
                                                        </button>
                                                        <button
                                                            onclick={toggle_save}
                                                            class={classes!("ep-mobile-action-btn", (*save_status).then_some("ep-mobile-active"))}
                                                            title={if *save_status { i18n_unsave.clone() } else { i18n_save.clone() }}
                                                        >
                                                            { if *save_status {
                                                                html! { <i class="ph ph-heart-break"></i> }
                                                            } else {
                                                                html! { <i class="ph ph-heart"></i> }
                                                            }}
                                                        </button>
                                                        <button
                                                            onclick={toggle_download}
                                                            class={classes!("ep-mobile-action-btn", (*download_status).then_some("ep-mobile-active"))}
                                                            title={if *download_status { i18n_remove_download.clone() } else { i18n_download.clone() }}
                                                        >
                                                            { if *download_in_progress {
                                                                html! { <i class="ph ph-spinner animate-spin"></i> }
                                                            } else if *download_status {
                                                                html! { <i class="ph ph-trash"></i> }
                                                            } else {
                                                                html! { <i class="ph ph-download"></i> }
                                                            }}
                                                        </button>
                                                        <button
                                                            onclick={toggle_completion}
                                                            class={classes!("ep-mobile-action-btn", (*completion_status).then_some("ep-mobile-active"))}
                                                            title={if *completion_status { i18n_mark_incomplete.clone() } else { i18n_mark_complete.clone() }}
                                                        >
                                                            { if *completion_status {
                                                                html! { <i class="ph ph-x-circle"></i> }
                                                            } else {
                                                                html! { <i class="ph ph-check-fat"></i> }
                                                            }}
                                                        </button>
                                                        </>
                                                    }
                                                } else { html! {} }}
                                                <button
                                                    onclick={create_share_link.clone()}
                                                    class="ep-mobile-action-btn"
                                                    title={i18n_share_episode.clone()}
                                                >
                                                    <i class="ph ph-share-network"></i>
                                                </button>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <div class="ep-mobile-actions">
                                                <p class="ep-mobile-no-media item_container-text">
                                                    { &i18n_this_item_contains_no_media }
                                                </p>
                                            </div>
                                        }
                                    }}

                                    // ── Host strip ──
                                    { if !*ep_2_loading {
                                        if let Some(people) = &audio_state.episode_page_people {
                                            if !people.is_empty() {
                                                let has_unknown_host = people.len() == 1
                                                    && people[0].name == "Unknown Host"
                                                    && people[0].role == Some("Host".to_string());
                                                if has_unknown_host {
                                                    let people_url = state.server_details.as_ref()
                                                        .and_then(|sd| sd.people_url.as_ref())
                                                        .cloned()
                                                        .unwrap_or_default();
                                                    let host_url = format!("{}/podcast/{}", people_url, episode.podcastid);
                                                    html! {
                                                        <div class="ep-mobile-hosts">
                                                            <p class="ep-mobile-section-label">{ &i18n_in_this_episode }</p>
                                                            <p class="ep-mobile-no-hosts-msg">
                                                                { i18n_no_hosts_found.clone() }
                                                                <a href={host_url} target="_blank" class="ep-mobile-no-hosts-link">
                                                                    { i18n_add_hosts_here.clone() }
                                                                </a>
                                                            </p>
                                                        </div>
                                                    }
                                                } else {
                                                    let server = server_for_proxy.clone();
                                                    html! {
                                                        <div class="ep-mobile-hosts">
                                                            <p class="ep-mobile-section-label">{ &i18n_in_this_episode }</p>
                                                            <div class="ep-mobile-hosts-scroll">
                                                            { for people.iter().map(|person| {
                                                                let name = person.name.clone();
                                                                let role = person.role.clone();
                                                                let img_url = person.img.as_ref().map(|url| {
                                                                    format!("{}/api/proxy/image?url={}", server, urlencoding::encode(url))
                                                                });
                                                                let hist = history_for_host.clone();
                                                                let nav_name = name.clone();
                                                                let on_chip_click = Callback::from(move |_: MouseEvent| {
                                                                    hist.push(format!("/person/{}", nav_name));
                                                                });
                                                                html! {
                                                                    <div class="ep-mobile-host-chip" onclick={on_chip_click}>
                                                                        { if let Some(src) = img_url {
                                                                            html! { <img src={src} alt={name.clone()} class="ep-mobile-host-avatar" /> }
                                                                        } else {
                                                                            html! { <div class="ep-mobile-host-avatar ep-mobile-host-placeholder"><i class="ph ph-user"></i></div> }
                                                                        }}
                                                                        <span class="ep-mobile-host-name">{ &person.name }</span>
                                                                        { if let Some(r) = &role {
                                                                            html! { <span class="ep-mobile-host-role">{ r }</span> }
                                                                        } else { html! {} }}
                                                                    </div>
                                                                }
                                                            })}
                                                            </div>
                                                        </div>
                                                    }
                                                }
                                            } else { html! {} }
                                        } else { html! {} }
                                    } else { html! {} }}

                                    // ── Transcript (built-in + AI, #726) ──
                                    { {
                                        let feed = audio_state.episode_page_transcript.clone().unwrap_or_default();
                                        let has_feed = !feed.is_empty();
                                        if has_feed || *ai_available {
                                            // Opens the shared transcript modal with the chosen source(s).
                                            let on_view = Callback::from(move |sources: Vec<Transcript>| {
                                                Dispatch::<EpisodeDetailState>::global().reduce_mut(move |s| {
                                                    s.show_transcript_modal = Some(true);
                                                    s.current_transcripts = Some(sources);
                                                });
                                            });
                                            let on_view_feed = {
                                                let on_view = on_view.clone();
                                                let feed = feed.clone();
                                                Callback::from(move |_: MouseEvent| on_view.emit(feed.clone()))
                                            };
                                            html! {
                                                <>
                                                <div class="ep-mobile-transcript">
                                                    { if has_feed {
                                                        html! {
                                                            <button onclick={on_view_feed} class="ep-mobile-transcript-btn font-bold">
                                                                <i class="ph ph-scroll"></i>
                                                                { &i18n_view_transcript }
                                                            </button>
                                                        }
                                                    } else { html! {} } }
                                                    <MobileTranscriptActions
                                                        episode_id={episode.episodeid}
                                                        server_name={server_name.clone().unwrap_or_default()}
                                                        api_key={api_key.clone().unwrap_or_default().unwrap_or_default()}
                                                        ai_available={*ai_available}
                                                        on_view={on_view.clone()}
                                                    />
                                                </div>
                                                if let Some(show_modal) = episode_detail_state.show_transcript_modal {
                                                    if show_modal {
                                                        <TranscriptModal
                                                            transcripts={episode_detail_state.current_transcripts.clone().unwrap_or_default()}
                                                            server_name={server_name.clone().unwrap_or_default()}
                                                            api_key={api_key.clone().unwrap_or_default().unwrap_or_default()}
                                                            onclose={Callback::from(move |_| {
                                                                Dispatch::<EpisodeDetailState>::global().reduce_mut(|s| {
                                                                    s.show_transcript_modal = Some(false);
                                                                });
                                                            })}
                                                        />
                                                    }
                                                }
                                                </>
                                            }
                                        } else { html! {} }
                                    } }

                                    // ── Description ──
                                    <div class="ep-mobile-desc episode-single-desc episode-description">
                                        <div class="item_container-text episode-description-container">
                                            <SafeHtml
                                                html={description}
                                                episode_url={Some(episode.episodeurl.clone())}
                                                episode_title={Some(episode.episodetitle.clone())}
                                                episode_description={Some(episode.episodedescription.clone())}
                                                episode_release_date={Some(episode.episodepubdate.clone())}
                                                episode_artwork={Some(episode.episodeartwork.clone())}
                                                episode_duration={episode.episodeduration}
                                                episode_id={Some(episode.episodeid)}
                                                is_youtube={episode.is_youtube}
                                                is_video={Some(episode.is_video)}
                                            />
                                        </div>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="episode-layout-container">
                                    // ===== HERO =====
                                    <div class="ep-hero">
                                        <div class="ep-hero-cover" onclick={on_title_click.clone()} title={episode.podcastname.clone()}>
                                            <FallbackImage
                                                src={episode.episodeartwork.clone()}
                                                alt={format!("{}{}", i18n_cover_alt_text, episode.episodeartwork.clone())}
                                                class="ep-hero-cover-img"
                                            />
                                        </div>
                                        <div class="ep-hero-body">
                                            <div class="ep-hero-show" onclick={on_title_click.clone()}>
                                                <i class="ph ph-microphone-stage"></i>
                                                <span>{ &episode.podcastname }</span>
                                            </div>
                                            <h1 class="ep-hero-title">{ &episode.episodetitle }</h1>
                                            <div class="ep-hero-meta">
                                                <span><i class="ph ph-calendar-blank"></i>{ &format_release }</span>
                                                <span class="ep-meta-dot">{"•"}</span>
                                                <span><i class="ph ph-clock"></i>{ &format_duration }</span>
                                                { if *completion_status {
                                                    html! {
                                                        <>
                                                        <span class="ep-meta-dot">{"•"}</span>
                                                        <span class="ep-meta-played"><i class="ph ph-check-circle"></i>{ i18n.t("episode.played") }</span>
                                                        </>
                                                    }
                                                } else { html! {} } }
                                            </div>
                                            { progress_bar }
                                            <div class="ep-hero-actions">
                                                { if should_show_buttons {
                                                    html! {
                                                        <>
                                                        <button onclick={handle_play_click} class="ep-hero-play" title={if is_playing { i18n_pause.clone() } else { i18n_play.clone() }}>
                                                            { if is_playing { html! { <i class="ph ph-pause"></i> } } else { html! { <i class="ph ph-play"></i> } } }
                                                            <span>{ if is_playing { &i18n_pause } else { &i18n_play } }</span>
                                                        </button>
                                                        { if *ep_in_db {
                                                            html! {
                                                                <>
                                                                <button onclick={toggle_queue} class={classes!("ep-action", (*queue_status).then_some("is-active"))} title={if *queue_status { i18n_remove_from_queue.clone() } else { i18n_add_to_queue.clone() }}>
                                                                    <i class="ph ph-queue"></i>
                                                                    <span>{ if *queue_status { &i18n_remove_from_queue } else { &i18n_add_to_queue } }</span>
                                                                </button>
                                                                <button onclick={toggle_save} class={classes!("ep-action", (*save_status).then_some("is-active"))} title={if *save_status { i18n_unsave.clone() } else { i18n_save.clone() }}>
                                                                    { if *save_status { html! { <i class="ph ph-heart-break"></i> } } else { html! { <i class="ph ph-heart"></i> } } }
                                                                    <span>{ if *save_status { &i18n_unsave } else { &i18n_save } }</span>
                                                                </button>
                                                                <button onclick={toggle_download} class={classes!("ep-action", (*download_status).then_some("is-active"))} title={if *download_status { i18n_remove_download.clone() } else { i18n_download.clone() }}>
                                                                    { if *download_status { html! { <i class="ph ph-trash"></i> } } else { html! { <i class="ph ph-download"></i> } } }
                                                                    <span>{ if *download_status { &i18n_remove_download } else { &i18n_download } }</span>
                                                                </button>
                                                                <button onclick={toggle_completion} class={classes!("ep-action", (*completion_status).then_some("is-active"))} title={if *completion_status { i18n_mark_episode_incomplete.clone() } else { i18n_mark_episode_complete.clone() }}>
                                                                    { if *completion_status { html! { <i class="ph ph-x-circle"></i> } } else { html! { <i class="ph ph-check-fat"></i> } } }
                                                                    <span>{ if *completion_status { &i18n_mark_episode_incomplete } else { &i18n_mark_episode_complete } }</span>
                                                                </button>
                                                                </>
                                                            }
                                                        } else {
                                                            html! { <span class="ep-actions-note item_container-text">{ &i18n_add_podcast_to_enable_actions }</span> }
                                                        } }
                                                        </>
                                                    }
                                                } else {
                                                    html! { <span class="ep-actions-note item_container-text">{ &i18n_this_item_contains_no_media }</span> }
                                                } }
                                                <button class="ep-action" onclick={create_share_link.clone()} title={i18n_share_episode.clone()}>
                                                    <i class="ph ph-share-network"></i>
                                                    <span>{ &i18n_share_episode }</span>
                                                </button>
                                                <button
                                                    class="ep-action"
                                                    onclick={ move |_| { download_episode_file.emit({ episode_id }) } }
                                                    disabled={*download_in_progress}
                                                    title={i18n_download_episode.clone()}
                                                >
                                                    { if *download_in_progress {
                                                        html! {
                                                            <>
                                                            <i class="ph ph-spinner animate-spin"></i>
                                                            <span>{ &i18n_downloading }</span>
                                                            </>
                                                        }
                                                    } else {
                                                        html! {
                                                            <>
                                                            <i class="ph ph-download-simple"></i>
                                                            <span>{ &i18n_download_episode }</span>
                                                            </>
                                                        }
                                                    } }
                                                </button>
                                            </div>
                                        </div>
                                    </div>

                                    // ===== TABBED BODY (notes / chapters / transcript / people) =====
                                    {{
                                        let chapter_count = audio_state.episode_page_chapters.as_ref().map_or(0, |c| c.len());
                                        let has_chapters = chapter_count > 0;
                                        let has_feed_transcript = audio_state.episode_page_transcript.as_ref().map_or(false, |t| !t.is_empty());
                                        // The Transcript tab is available if there's a built-in transcript OR the AI
                                        // sidecar is connected (so the user can generate one).
                                        let has_transcript = has_feed_transcript || *ai_available;
                                        let has_people = !*ep_2_loading && audio_state.episode_page_people.as_ref().map_or(false, |p| !p.is_empty());
                                        let effective_tab = match *active_tab {
                                            EpisodeTab::Chapters if has_chapters => EpisodeTab::Chapters,
                                            EpisodeTab::Transcript if has_transcript => EpisodeTab::Transcript,
                                            EpisodeTab::People if has_people => EpisodeTab::People,
                                            _ => EpisodeTab::Notes,
                                        };
                                        let tab_btn = |tab: EpisodeTab, icon: &'static str, label: String, badge: Option<usize>| -> Html {
                                            let active_tab = active_tab.clone();
                                            let is_active = effective_tab == tab;
                                            let onclick = Callback::from(move |_| active_tab.set(tab));
                                            html! {
                                                <button class={classes!("ep-tab", is_active.then_some("is-active"))} onclick={onclick}>
                                                    <i class={classes!("ph", icon)}></i>
                                                    <span>{ label }</span>
                                                    { if let Some(b) = badge { html! { <span class="ep-tab-badge">{ b }</span> } } else { html! {} } }
                                                </button>
                                            }
                                        };
                                        html! {
                                            <>
                                            <div class="ep-tabs" role="tablist">
                                                { tab_btn(EpisodeTab::Notes, "ph-note", i18n.t("episode.episode_notes").to_string(), None) }
                                                { if has_chapters { tab_btn(EpisodeTab::Chapters, "ph-list-numbers", i18n.t("audio.chapters").to_string(), Some(chapter_count)) } else { html! {} } }
                                                { if has_transcript { tab_btn(EpisodeTab::Transcript, "ph-scroll", i18n_transcript.clone(), None) } else { html! {} } }
                                                { if has_people { tab_btn(EpisodeTab::People, "ph-users-three", i18n_in_this_episode.clone(), None) } else { html! {} } }
                                            </div>

                                            { match effective_tab {
                                                EpisodeTab::Notes => html! {
                                                    <div class="ep-pane ep-about">
                                                        <div class="item_container-text episode-description-container">
                                                            <SafeHtml
                                                                html={description}
                                                                episode_url={Some(episode.episodeurl.clone())}
                                                                episode_title={Some(episode.episodetitle.clone())}
                                                                episode_description={Some(episode.episodedescription.clone())}
                                                                episode_release_date={Some(episode.episodepubdate.clone())}
                                                                episode_artwork={Some(episode.episodeartwork.clone())}
                                                                episode_duration={episode.episodeduration}
                                                                episode_id={Some(episode.episodeid)}
                                                                is_youtube={episode.is_youtube}
                                                                is_video={Some(episode.is_video)}
                                                            />
                                                        </div>
                                                    </div>
                                                },
                                                EpisodeTab::Chapters => {
                                                    let chapters = audio_state.episode_page_chapters.as_deref().unwrap_or(&[]);
                                                    let loaded = audio_state.currently_playing.as_ref().map_or(false, |c| c.episode_id == episode_id);
                                                    let now_playing = audio_state.audio_playing.unwrap_or(false);
                                                    let cur = audio_state.current_time_seconds;
                                                    html! {
                                                        <div class="ep-pane">
                                                            <ol class="ep-chapters">
                                                                { for chapters.iter().enumerate().map(|(idx, chapter)| {
                                                                    let start = chapter.startTime.unwrap_or(0);
                                                                    let end = chapters.get(idx + 1).and_then(|c| c.startTime).unwrap_or(i32::MAX);
                                                                    let is_current = loaded && cur >= start as f64 && cur < end as f64;
                                                                    let is_active_playing = is_current && now_playing;
                                                                    let server = server_name.clone().unwrap_or_default();
                                                                    let img_url = chapter.img.as_ref()
                                                                        .filter(|u| !u.is_empty())
                                                                        .map(|url| format!("{}/api/proxy/image?url={}", server, urlencoding::encode(url)));
                                                                    let audio_dispatch = audio_dispatch.clone();
                                                                    let episode_for_play = episode.clone();
                                                                    let api_key_c = api_key.clone();
                                                                    let server_name_c = server_name.clone();
                                                                    let on_jump = Callback::from(move |e: MouseEvent| {
                                                                        // If this chapter is the one currently playing, the button pauses.
                                                                        if is_active_playing {
                                                                            audio_dispatch.reduce_mut(|state| {
                                                                                if let Some(media_element) = state.media_element.as_ref() {
                                                                                    let _ = media_element.pause();
                                                                                } else if let Some(audio_element) = state.audio_element.as_ref() {
                                                                                    let _ = audio_element.pause();
                                                                                }
                                                                                state.audio_playing = Some(false);
                                                                            });
                                                                            return;
                                                                        }
                                                                        let loaded = Dispatch::<UIState>::global().get()
                                                                            .currently_playing.as_ref()
                                                                            .map_or(false, |c| c.episode_id == episode_id);
                                                                        if loaded {
                                                                            let st = start as f64;
                                                                            audio_dispatch.reduce_mut(|state| {
                                                                                let hours = (st / 3600.0).floor() as i32;
                                                                                let minutes = ((st % 3600.0) / 60.0).floor() as i32;
                                                                                let seconds = (st % 60.0).floor() as i32;
                                                                                let fmt = format!("{:02}:{:02}:{:02}", hours, minutes, seconds);
                                                                                if let Some(media_element) = state.media_element.as_ref() {
                                                                                    media_element.set_current_time(st);
                                                                                    let _ = media_element.play();
                                                                                    state.current_time_seconds = st;
                                                                                    state.current_time_formatted = fmt;
                                                                                } else if let Some(audio_element) = state.audio_element.as_ref() {
                                                                                    audio_element.set_current_time(st);
                                                                                    let _ = audio_element.play();
                                                                                    state.current_time_seconds = st;
                                                                                    state.current_time_formatted = fmt;
                                                                                }
                                                                                state.audio_playing = Some(true);
                                                                            });
                                                                        } else {
                                                                            let mut ep = episode_for_play.clone();
                                                                            ep.listenduration = start;
                                                                            let cb = on_play_click(
                                                                                ep,
                                                                                api_key_c.clone().unwrap().unwrap(),
                                                                                user_id.unwrap(),
                                                                                server_name_c.clone().unwrap(),
                                                                                audio_dispatch.clone(),
                                                                                Dispatch::<UIState>::global().get(),
                                                                                false,
                                                                                false,
                                                                                None,
                                                                            );
                                                                            cb.emit(e);
                                                                        }
                                                                    });
                                                                    html! {
                                                                        <li class={classes!("ep-chapter", is_current.then_some("is-current"))} onclick={on_jump} title={chapter.title.clone()}>
                                                                            <span class="ep-chapter-play"><i class={classes!("ph", if is_active_playing { "ph-pause" } else { "ph-play" })}></i></span>
                                                                            <span class="ep-chapter-num">{ format!("{:02}", idx + 1) }</span>
                                                                            { if let Some(src) = img_url {
                                                                                html! { <img src={src} class="ep-chapter-thumb" alt="" /> }
                                                                            } else { html! {} } }
                                                                            <span class="ep-chapter-title">{ &chapter.title }</span>
                                                                            <span class="ep-chapter-time">{ format_time(start) }</span>
                                                                        </li>
                                                                    }
                                                                }) }
                                                            </ol>
                                                        </div>
                                                    }
                                                },
                                                EpisodeTab::Transcript => html! {
                                                    <TranscriptTab
                                                        episode_id={episode.episodeid}
                                                        feed_transcripts={audio_state.episode_page_transcript.clone().unwrap_or_default()}
                                                        server_name={server_name.clone().unwrap_or_default()}
                                                        api_key={api_key.clone().unwrap_or_default().unwrap_or_default()}
                                                        ai_available={*ai_available}
                                                    />
                                                },
                                                EpisodeTab::People => {
                                                    if let Some(people) = &audio_state.episode_page_people {
                                                        let has_unknown_host = people.len() == 1
                                                            && people[0].name == "Unknown Host"
                                                            && people[0].role == Some("Host".to_string());
                                                        if has_unknown_host {
                                                            let people_url = state.server_details.as_ref()
                                                                .and_then(|sd| sd.people_url.as_ref())
                                                                .cloned()
                                                                .unwrap_or_default();
                                                            let host_url = format!("{}/podcast/{}", people_url, episode.podcastid);
                                                            html! {
                                                                <div class="ep-pane">
                                                                    <p class="ep-nohosts-msg">
                                                                        { i18n_no_hosts_found.clone() }
                                                                        <a href={host_url} target="_blank" class="ep-nohosts-link">
                                                                            { i18n_add_hosts_here.clone() }
                                                                        </a>
                                                                    </p>
                                                                </div>
                                                            }
                                                        } else {
                                                            let server = server_name.clone().unwrap_or_default();
                                                            html! {
                                                                <div class="ep-pane">
                                                                    <div class="ep-people">
                                                                        { for people.iter().map(|person| {
                                                                            let name = person.name.clone();
                                                                            let role = person.role.clone();
                                                                            let img_url = person.img.as_ref().map(|url| {
                                                                                format!("{}/api/proxy/image?url={}", server, urlencoding::encode(url))
                                                                            });
                                                                            let hist = history.clone();
                                                                            let nav_name = name.clone();
                                                                            let on_chip_click = Callback::from(move |_: MouseEvent| {
                                                                                hist.push(format!("/person/{}", nav_name));
                                                                            });
                                                                            html! {
                                                                                <div class="ep-person" onclick={on_chip_click} title={name.clone()}>
                                                                                    { if let Some(src) = img_url {
                                                                                        html! { <img src={src} alt={name.clone()} class="ep-person-avatar" /> }
                                                                                    } else {
                                                                                        html! { <div class="ep-person-avatar ep-person-avatar-ph"><i class="ph ph-user"></i></div> }
                                                                                    } }
                                                                                    <div class="ep-person-body">
                                                                                        <div class="ep-person-name">{ &person.name }</div>
                                                                                        { if let Some(r) = &role {
                                                                                            html! { <span class="ep-person-role">{ r }</span> }
                                                                                        } else { html! {} } }
                                                                                    </div>
                                                                                </div>
                                                                            }
                                                                        }) }
                                                                    </div>
                                                                </div>
                                                            }
                                                        }
                                                    } else { html! {} }
                                                },
                                            } }
                                            </>
                                        }
                                    }}

                                    // ===== TRANSCRIPT MODAL =====
                                    { if let Some(show_modal) = episode_detail_state.show_transcript_modal {
                                        if show_modal {
                                            html! {
                                                <TranscriptModal
                                                    transcripts={episode_detail_state.current_transcripts.clone().unwrap_or_default()}
                                                    server_name={server_name.clone().unwrap_or_default()}
                                                    api_key={api_key.clone().unwrap().unwrap_or_default()}
                                                    onclose={Callback::from(move |_| {
                                                        Dispatch::<EpisodeDetailState>::global().reduce_mut(|s| {
                                                            s.show_transcript_modal = Some(false);
                                                        });
                                                    })}
                                                />
                                            }
                                        } else { html! {} }
                                    } else { html! {} } }
                                </div>
                            }
                        };
                        layout
                    }
                }
            }

            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! {
                        <AudioPlayer
                            episode={audio_props.episode.clone()}
                            src={audio_props.src.clone()}
                            title={audio_props.title.clone()}
                            description={audio_props.description.clone()}
                            release_date={audio_props.release_date.clone()}
                            artwork_url={audio_props.artwork_url.clone()}
                            duration={audio_props.duration.clone()}
                            episode_id={audio_props.episode_id.clone()}
                            duration_sec={audio_props.duration_sec.clone()}
                            start_pos_sec={audio_props.start_pos_sec.clone()}
                            end_pos_sec={audio_props.end_pos_sec.clone()}
                            offline={audio_props.offline.clone()}
                            is_youtube={audio_props.is_youtube.clone()}
                        is_video={audio_props.is_video.clone()}
                        />
                    }
                } else {
                    html! {}
                }
            }
        </div>
        <App_drawer />
        </>
    }
}
