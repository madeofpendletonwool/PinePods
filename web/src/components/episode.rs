use super::app_drawer::App_drawer;
use super::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::audio::on_play_click;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, UIState};
use crate::components::gen_funcs::format_error_message;
use crate::components::gen_funcs::{
    convert_time_to_seconds, format_datetime, format_time, match_date_format, parse_date,
    sanitize_html_with_blank_target,
};
use crate::components::host_component::HostDropdown;
use crate::components::safehtml::SafeHtml;
use crate::requests::pod_req;
use crate::requests::pod_req::{
    call_check_podcast, call_create_share_link, call_download_episode, call_download_episode_file,
    call_fetch_podcasting_2_data, call_get_episode_id, call_mark_episode_completed,
    call_mark_episode_uncompleted, call_queue_episode, call_remove_downloaded_episode,
    call_remove_queued_episode, call_remove_saved_episode, call_save_episode,
    DownloadEpisodeRequest, EpisodeInfo, EpisodeMetadataResponse, EpisodeRequest,
    FetchPodcasting2DataRequest, MarkEpisodeCompletedRequest, QueuePodcastRequest,
    SavePodcastRequest, Transcript,
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
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

#[allow(dead_code)]
async fn fallback_to_podcast_parsing(
    server_name: String,
    api_key: Option<String>,
    episode_url_clone: String,
    audio_url_clone: String,
    podcast_title_clone: String,
    podcast_index_id: i64,
    is_youtube: bool,
    episode_id: i32,
    dispatch: Dispatch<AppState>,
    error_clone: UseStateHandle<Option<String>>,
    aud_dispatch: Dispatch<UIState>,
    ep_2_loading_clone: UseStateHandle<bool>,
    ui_state: Rc<AppState>,
    window: web_sys::Window,
    user_id: i32,
    loading_clone: UseStateHandle<bool>,
) {
    match call_parse_podcast_url(server_name.clone(), &api_key, &episode_url_clone).await {
        Ok(result) => {
            if let Some(ep) = result
                .episodes
                .iter()
                .find(|ep| ep.enclosure_url.as_ref() == Some(&audio_url_clone))
                .cloned()
            {
                let time_sec = convert_time_to_seconds(ep.duration.unwrap_or_default().as_str());
                if let Ok(episodeduration) = time_sec {
                    let ep_url = episode_url_clone.clone();
                    let aud_url = audio_url_clone.clone();
                    let podcast_title = podcast_title_clone.clone();
                    let episodeduration: i32 = episodeduration.try_into().unwrap_or(0);

                    dispatch.reduce_mut(move |state| {
                        state.fetched_episode = Some(EpisodeMetadataResponse {
                            episode: EpisodeInfo {
                                episodetitle: ep.title.unwrap_or_default(),
                                podcastname: podcast_title.clone(),
                                podcastid: 0,
                                podcastindexid: Some(podcast_index_id),
                                feedurl: ep_url.clone(),
                                episodepubdate: ep.pub_date.unwrap_or_default(),
                                // Apply sanitize_html_with_blank_target here too
                                episodedescription: sanitize_html_with_blank_target(
                                    &ep.description.unwrap_or_default(),
                                ),
                                episodeartwork: ep.artwork.unwrap_or_default(),
                                episodeurl: aud_url.clone(),
                                episodeduration,
                                listenduration: Some(episodeduration),
                                episodeid: episode_id,
                                completed: false,
                                is_downloaded: false,
                                is_queued: false,
                                is_saved: false,
                                is_youtube: false,
                            },
                        });
                        state.selected_episode_id = Some(episode_id);
                        state.selected_episode_url = Some(ep_url.clone());
                        state.selected_episode_audio_url = Some(aud_url.clone());
                        state.selected_podcast_title = Some(podcast_title.clone());
                    });

                    // Handle podcast 2.0 data if needed
                    if let Some(episode_id) = ui_state.selected_episode_id {
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

#[function_component(Episode)]
pub fn epsiode() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();

    let error = use_state(|| None);
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

    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();

    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let history = BrowserHistory::new();
    let episode_id = state.selected_episode_id.clone();
    let ep_in_db = use_state(|| false);
    let loading = use_state(|| true); // Initial loading state set to true
    let ep_2_loading = use_state(|| true);
    let initial_fetch_complete = use_state(|| false);

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

    // Fetch episode on component mount
    {
        let ep_2_loading_clone = ep_2_loading.clone();
        let initial_fetch_complete = initial_fetch_complete.clone();
        // let episodes = episodes.clone();
        let error = error.clone();
        let api_key = post_state
            .auth_details
            .as_ref()
            .map(|ud| ud.api_key.clone());
        let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = post_state
            .auth_details
            .as_ref()
            .map(|ud| ud.server_name.clone());
        let effect_dispatch = dispatch.clone();
        let aud_dispatch = audio_dispatch.clone();
        let effect_pod_state = state.clone();
        let loading_clone = loading.clone();
        let ui_state = state.clone();

        let episode_id = state.selected_episode_id.clone();
        // fetch_episodes(api_key.flatten(), user_id, server_name, dispatch, error, pod_req::call_get_recent_eps);
        let effect_ep_in_db = ep_in_db.clone();
        use_effect_with(
            (
                api_key.clone(),
                user_id.clone(),
                server_name.clone(),
                episode_id.clone(),
            ),
            move |_| {
                let error_clone = error.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    // Check if the current episode_id is the same as the already fetched episode
                    let should_reload = if let Some(current_episode_id) = episode_id {
                        if let Some(fetched_episode) = &effect_pod_state.fetched_episode {
                            // Only reload if the episode ID has actually changed
                            fetched_episode.episode.episodeid != current_episode_id
                        } else {
                            // No episode is currently loaded, so we need to load
                            true
                        }
                    } else {
                        // No episode ID provided, need to load based on URL params
                        true
                    };

                    if !should_reload {
                        // Episode is already loaded and it's the same one, no need to reload
                        loading_clone.set(false);
                    } else {
                        // Reset loading state when episode_id changes
                        loading_clone.set(true);

                        // Clear previous episode data when transitioning to a different episode
                        effect_dispatch.reduce_mut(|state| {
                            state.fetched_episode = None;
                        });
                        let dispatch = effect_dispatch.clone();

                        // Check if the URL contains the parameters for the episode
                        let window = web_sys::window().expect("no global window exists");
                        let search_params = window.location().search().unwrap();
                        let url_params = UrlSearchParams::new_with_str(&search_params).unwrap();

                        let podcast_title = url_params.get("podcast_title").unwrap_or_default();
                        let episode_url = url_params.get("episode_url").unwrap_or_default();
                        let audio_url = url_params.get("audio_url").unwrap_or_default();
                        let podcast_index_id = url_params
                            .get("podcast_index_id")
                            .and_then(|id| id.parse::<i64>().ok())
                            .unwrap_or(0);
                        let is_youtube = url_params // Add this
                            .get("is_youtube")
                            .and_then(|v| v.parse::<bool>().ok())
                            .unwrap_or(false);

                        if !podcast_title.is_empty()
                            && !episode_url.is_empty()
                            && !audio_url.is_empty()
                        {
                            // URL contains episode parameters, handle the episode setup
                            let podcast_title_clone = podcast_title.clone();
                            let episode_url_clone = episode_url.clone();
                            let audio_url_clone = audio_url.clone();
                            let pod_title_clone2 = podcast_title.clone();
                            let episode_url_clone2 = episode_url.clone();
                            let audio_url_clone2 = audio_url.clone();

                            wasm_bindgen_futures::spawn_local(async move {
                                // First, try to get the episode ID
                                match call_get_episode_id(
                                    &server_name,
                                    &api_key.clone().unwrap(),
                                    &user_id,
                                    &podcast_title_clone,
                                    &audio_url_clone,
                                    is_youtube,
                                )
                                .await
                                {
                                    Ok(fetched_episode_id) => {
                                        // If we have an episode ID, use get_episode_metadata for consistent HTML processing
                                        let episode_request = EpisodeRequest {
                                            episode_id: fetched_episode_id,
                                            user_id: user_id.clone(),
                                            person_episode: false,
                                            is_youtube,
                                        };

                                        match pod_req::call_get_episode_metadata(
                                            &server_name,
                                            api_key.clone(),
                                            &episode_request,
                                        )
                                        .await
                                        {
                                            Ok(fetched_episode) => {
                                                // Successfully got episode metadata - this will have proper HTML
                                                dispatch.reduce_mut(move |state| {
                                                    state.selected_episode_id =
                                                        Some(fetched_episode_id);
                                                    state.fetched_episode =
                                                        Some(EpisodeMetadataResponse {
                                                            episode: fetched_episode,
                                                        });
                                                    state.selected_episode_url =
                                                        Some(episode_url_clone2.clone());
                                                    state.selected_episode_audio_url =
                                                        Some(audio_url_clone2.clone());
                                                    state.selected_podcast_title =
                                                        Some(pod_title_clone2.clone());
                                                });

                                                // Setup podcasting 2.0 data
                                                let user_id_clone = user_id.clone();
                                                let api_key_clone = api_key.clone();
                                                let server_name_clone = server_name.clone();

                                                // Use fetched_episode_id directly since we already have it
                                                let chap_request = FetchPodcasting2DataRequest {
                                                    episode_id: fetched_episode_id,
                                                    user_id: user_id_clone,
                                                };

                                                if !is_youtube {
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
                                                                    state.episode_page_people =
                                                                        Some(response.people);
                                                                });
                                                                ep_2_loading_clone.set(false);
                                                            }
                                                            Err(e) => {
                                                                web_sys::console::log_1(&format!("Error fetching podcast 2.0 data: {}", e).into());
                                                                aud_dispatch.reduce_mut(|state| {
                                                                    state.episode_page_transcript =
                                                                        None;
                                                                    state.episode_page_people =
                                                                        None;
                                                                });
                                                            }
                                                        }
                                                    });
                                                }

                                                // Update the URL with the parameters if they are not already there
                                                let mut new_url =
                                                    window.location().origin().unwrap();
                                                new_url.push_str(
                                                    &window.location().pathname().unwrap(),
                                                );
                                                new_url.push_str("?podcast_title=");
                                                new_url.push_str(&urlencoding::encode(
                                                    &podcast_title_clone,
                                                ));
                                                new_url.push_str("&episode_url=");
                                                new_url.push_str(&urlencoding::encode(
                                                    &episode_url_clone,
                                                ));
                                                new_url.push_str("&audio_url=");
                                                new_url.push_str(&urlencoding::encode(
                                                    &audio_url_clone,
                                                ));
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

                                                effect_ep_in_db.set(true);
                                                loading_clone.set(false);
                                            }
                                            Err(e) => {
                                                // Metadata call failed, fall back to parsing from feed
                                                web_sys::console::log_1(
                                                    &format!("Metadata call failed: {}", e).into(),
                                                );
                                                fallback_to_podcast_parsing(
                                                    server_name.clone(),
                                                    api_key.clone(),
                                                    episode_url_clone.clone(),
                                                    audio_url_clone.clone(),
                                                    podcast_title_clone.clone(),
                                                    podcast_index_id,
                                                    is_youtube,
                                                    fetched_episode_id,
                                                    dispatch.clone(),
                                                    error_clone.clone(),
                                                    aud_dispatch.clone(),
                                                    ep_2_loading_clone.clone(),
                                                    ui_state.clone(),
                                                    window,
                                                    user_id.clone(),
                                                    loading_clone.clone(),
                                                )
                                                .await;
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        // Couldn't get episode ID, fall back to parsing podcast feed
                                        match call_parse_podcast_url(
                                            server_name.clone(),
                                            &api_key,
                                            &episode_url_clone,
                                        )
                                        .await
                                        {
                                            Ok(result) => {
                                                if let Some(ep) = result
                                                    .episodes
                                                    .iter()
                                                    .find(|ep| {
                                                        ep.enclosure_url.as_ref()
                                                            == Some(&audio_url_clone)
                                                    })
                                                    .cloned()
                                                {
                                                    let time_sec = convert_time_to_seconds(
                                                        ep.duration.unwrap_or_default().as_str(),
                                                    );
                                                    if let Ok(episodeduration) = time_sec {
                                                        let ep_url = episode_url_clone.clone();
                                                        let aud_url = audio_url_clone.clone();
                                                        let podcast_title =
                                                            podcast_title_clone.clone();
                                                        let episodeduration: i32 =
                                                            episodeduration.try_into().unwrap_or(0);

                                                        dispatch.reduce_mut(move |state| {
                                                        state.fetched_episode = Some(EpisodeMetadataResponse {
                                                            episode: EpisodeInfo {
                                                                episodetitle: ep.title.unwrap_or_default(),
                                                                podcastname: podcast_title.clone(),
                                                                podcastid: 0,
                                                                podcastindexid: Some(podcast_index_id),
                                                                feedurl: ep_url.clone(),
                                                                episodepubdate: ep.pub_date.unwrap_or_default(),
                                                                // KEY CHANGE: Apply sanitize_html_with_blank_target here
                                                                episodedescription: sanitize_html_with_blank_target(
                                                                    &ep.description.unwrap_or_default()
                                                                ),
                                                                episodeartwork: ep.artwork.unwrap_or_default(),
                                                                episodeurl: audio_url.clone(),
                                                                episodeduration,
                                                                listenduration: Some(episodeduration),
                                                                episodeid: 0, // Set the episode ID to 0
                                                                completed: false,
                                                                is_downloaded: false,
                                                                is_queued: false,
                                                                is_saved: false,
                                                                is_youtube: false,
                                                            },
                                                        });
                                                        state.selected_episode_id = Some(0);
                                                        state.selected_episode_url = Some(ep_url.clone());
                                                        state.selected_episode_audio_url = Some(aud_url.clone());
                                                        state.selected_podcast_title = Some(podcast_title.clone());
                                                    });

                                                        // Update the URL with the parameters
                                                        let mut new_url =
                                                            window.location().origin().unwrap();
                                                        new_url.push_str(
                                                            &window.location().pathname().unwrap(),
                                                        );
                                                        new_url.push_str("?podcast_title=");
                                                        new_url.push_str(&urlencoding::encode(
                                                            &podcast_title_clone,
                                                        ));
                                                        new_url.push_str("&episode_url=");
                                                        new_url.push_str(&urlencoding::encode(
                                                            &episode_url_clone,
                                                        ));
                                                        new_url.push_str("&audio_url=");
                                                        new_url.push_str(&urlencoding::encode(
                                                            &audio_url_clone,
                                                        ));
                                                        new_url.push_str("&podcast_index_id=");
                                                        new_url.push_str(
                                                            &podcast_index_id.to_string(),
                                                        );
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

                                                        loading_clone.set(false);
                                                    } else {
                                                        error_clone.set(Some(
                                                            i18n_failed_to_parse_duration.clone(),
                                                        ));
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error_clone.set(Some(e.to_string()));
                                            }
                                        }
                                    }
                                }
                            });
                        } else if let Some(id) = episode_id {
                            // Handle the case where no URL parameters are provided (original behavior)
                            if id == 0 {
                                let feed_url =
                                    effect_pod_state.selected_episode_url.clone().unwrap();
                                let podcast_title =
                                    effect_pod_state.selected_podcast_title.clone().unwrap();
                                let audio_url =
                                    effect_pod_state.selected_episode_audio_url.clone().unwrap();
                                // Update the URL with the parameters if they are not already there
                                let mut new_url = window.location().origin().unwrap();
                                new_url.push_str(&window.location().pathname().unwrap());
                                new_url.push_str("?podcast_title=");
                                new_url.push_str(&urlencoding::encode(&podcast_title));
                                new_url.push_str("&episode_url=");
                                new_url.push_str(&urlencoding::encode(&feed_url));
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

                                wasm_bindgen_futures::spawn_local(async move {
                                    match call_parse_podcast_url(server_name, &api_key, &feed_url)
                                        .await
                                    {
                                        Ok(result) => {
                                            if let Some(ep) = result
                                                .episodes
                                                .iter()
                                                .find(|ep| {
                                                    ep.enclosure_url.as_ref()
                                                        == Some(&audio_url.clone())
                                                })
                                                .cloned()
                                            {
                                                let time_sec = convert_time_to_seconds(
                                                    ep.duration.unwrap_or_default().as_str(),
                                                );
                                                if let Ok(episodeduration) = time_sec {
                                                    let episodeduration: i32 =
                                                        episodeduration.try_into().unwrap_or(0);
                                                    dispatch.reduce_mut(move |state| {
                                                        state.fetched_episode =
                                                            Some(EpisodeMetadataResponse {
                                                                episode: EpisodeInfo {
                                                                    episodetitle: ep
                                                                        .title
                                                                        .unwrap_or_default(),
                                                                    podcastname: podcast_title
                                                                        .clone(),
                                                                    podcastid: 0,
                                                                    podcastindexid: Some(
                                                                        podcast_index_id,
                                                                    ),
                                                                    feedurl: feed_url.clone(),
                                                                    episodepubdate: ep
                                                                        .pub_date
                                                                        .unwrap_or_default(),
                                                                    episodedescription: ep
                                                                        .description
                                                                        .unwrap_or_default(),
                                                                    episodeartwork: ep
                                                                        .artwork
                                                                        .unwrap_or_default(),
                                                                    episodeurl: audio_url.clone(),
                                                                    episodeduration,
                                                                    listenduration: Some(
                                                                        episodeduration,
                                                                    ),
                                                                    episodeid: 0,
                                                                    completed: false,
                                                                    is_downloaded: false,
                                                                    is_queued: false,
                                                                    is_saved: false,
                                                                    is_youtube: false,
                                                                },
                                                            });
                                                    });
                                                    loading_clone.set(false);
                                                } else {
                                                    error_clone.set(Some(
                                                        i18n_failed_to_parse_duration.clone(),
                                                    ));
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error_clone.set(Some(e.to_string()));
                                        }
                                    }
                                });
                            } else {
                                let episode_request = EpisodeRequest {
                                    episode_id: id,
                                    user_id: user_id.clone(),
                                    person_episode: effect_pod_state
                                        .person_episode
                                        .unwrap_or(false), // Defaults to false if None
                                    is_youtube: effect_pod_state
                                        .selected_is_youtube
                                        .unwrap_or(false),
                                };
                                effect_ep_in_db.set(true);
                                wasm_bindgen_futures::spawn_local(async move {
                                    match pod_req::call_get_episode_metadata(
                                        &server_name,
                                        api_key.clone(),
                                        &episode_request,
                                    )
                                    .await
                                    {
                                        Ok(fetched_episode) => {
                                            let user_id_clone = user_id.clone();
                                            let api_key_clone = api_key.clone();
                                            let server_name_clone = server_name.clone();

                                            // After setting fetched_episode and before setting loading_clone.set(false):
                                            if let Some(episode_id) = ui_state.selected_episode_id {
                                                let chap_request = FetchPodcasting2DataRequest {
                                                    episode_id,
                                                    user_id: user_id_clone,
                                                };

                                                if !is_youtube {
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
                                                                    state.episode_page_people =
                                                                        Some(response.people);
                                                                });
                                                                ep_2_loading_clone.set(false);
                                                            }
                                                            Err(e) => {
                                                                web_sys::console::log_1(&format!("Error fetching podcast 2.0 data: {}", e).into());
                                                                aud_dispatch.reduce_mut(|state| {
                                                                    state.episode_page_transcript =
                                                                        None;
                                                                    state.episode_page_people =
                                                                        None;
                                                                });
                                                            }
                                                        }
                                                    });
                                                }
                                            }
                                            let episode_url = fetched_episode.feedurl.clone();
                                            let podcast_title = fetched_episode.podcastname.clone();
                                            let audio_url = fetched_episode.episodeurl.clone();
                                            let real_episode_id = fetched_episode.episodeid.clone();
                                            let podcast_index_id_push =
                                                fetched_episode.podcastindexid.clone();
                                            dispatch.reduce_mut(move |state| {
                                                state.selected_episode_id = Some(real_episode_id);
                                                state.fetched_episode =
                                                    Some(EpisodeMetadataResponse {
                                                        episode: EpisodeInfo {
                                                            episodetitle: fetched_episode
                                                                .episodetitle
                                                                .clone(),
                                                            podcastname: fetched_episode
                                                                .podcastname
                                                                .clone(),
                                                            podcastid: fetched_episode.podcastid,
                                                            podcastindexid: fetched_episode
                                                                .podcastindexid,
                                                            feedurl: fetched_episode
                                                                .feedurl
                                                                .clone(),
                                                            episodepubdate: fetched_episode
                                                                .episodepubdate
                                                                .clone(),
                                                            episodedescription:
                                                                sanitize_html_with_blank_target(
                                                                    &fetched_episode
                                                                        .episodedescription,
                                                                ),
                                                            episodeartwork: fetched_episode
                                                                .episodeartwork
                                                                .clone(),
                                                            episodeurl: fetched_episode
                                                                .episodeurl
                                                                .clone(),
                                                            episodeduration: fetched_episode
                                                                .episodeduration,
                                                            listenduration: fetched_episode
                                                                .listenduration,
                                                            episodeid: fetched_episode.episodeid,
                                                            completed: fetched_episode.completed,
                                                            is_downloaded: fetched_episode
                                                                .is_downloaded,
                                                            is_queued: fetched_episode.is_queued,
                                                            is_saved: fetched_episode.is_saved,
                                                            is_youtube: fetched_episode.is_youtube,
                                                        },
                                                    });
                                            });

                                            // Add URL parameters
                                            let window =
                                                web_sys::window().expect("no global window exists");
                                            let mut new_url = window.location().origin().unwrap();
                                            new_url
                                                .push_str(&window.location().pathname().unwrap());
                                            new_url.push_str("?podcast_title=");
                                            new_url.push_str(&urlencoding::encode(&podcast_title));
                                            new_url.push_str("&episode_url=");
                                            new_url.push_str(&urlencoding::encode(&episode_url));
                                            new_url.push_str("&audio_url=");
                                            new_url.push_str(&urlencoding::encode(&audio_url));
                                            new_url.push_str("&podcast_index_id=");
                                            new_url.push_str(
                                                &podcast_index_id_push.unwrap_or(0).to_string(),
                                            );
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
                                            loading_clone.set(false);
                                        }
                                        Err(e) => {
                                            error_clone.set(Some(e.to_string()));
                                        }
                                    }
                                });
                            }
                        }
                        initial_fetch_complete.set(true);
                    } // Close the else block
                }
                || ()
            },
        );
    }

    let completion_status = use_state(|| false);
    let queue_status = use_state(|| false);
    let save_status = use_state(|| false);
    let download_status = use_state(|| false);
    let download_in_progress = use_state(|| false);

    {
        let state = state.clone();
        let completion_status = completion_status.clone();
        let queue_status = queue_status.clone();
        let save_status = save_status.clone();
        let download_status = download_status.clone();

        use_effect_with(state.fetched_episode.clone(), move |_| {
            if let Some(episode) = &state.fetched_episode {
                completion_status.set(episode.episode.completed);
                queue_status.set(episode.episode.is_queued);
                save_status.set(episode.episode.is_saved);
                download_status.set(episode.episode.is_downloaded);
            } else {
                web_sys::console::log_1(&i18n_no_episode_data_received.clone().into());
            }
            || ()
        });
    }

    let page_state = use_state(|| PageState::Hidden);

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
    }

    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
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
        let server_name = server_name.clone();
        let episode_id = episode_id.clone();
        let page_state_create = page_state.clone(); // Handle modal visibility
        let shared_url_create = shared_url_copy.clone(); // Store the created shared link

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let ep_id_deref = episode_id.clone().unwrap();
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
                        ep_id_deref,
                    )
                    .await
                    {
                        Ok(url_key) => {
                            let full_url = format!("{}/shared_episode/{}", server_name, url_key);
                            shared_url_copy.set(Some(full_url)); // Store the generated link
                            page_state_copy.set(PageState::Shown); // Show the modal
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
        let episode_id = episode_id.clone();
        let dispatch = _post_dispatch.clone();
        let download_in_progress = download_in_progress.clone();

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let ep_id_deref = episode_id.clone().unwrap();
            let dispatch = dispatch.clone();
            let download_in_progress = download_in_progress.clone();

            // Set download in progress state
            download_in_progress.set(true);

            // Set global loading state
            dispatch.reduce_mut(|state| {
                state.is_loading = Some(true);
                web_sys::console::log_1(&"Loading state set to true".into());
            });

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(_api_key), Some(server_name)) =
                    (api_key.as_ref(), server_name.as_ref())
                {
                    match call_download_episode_file(&server_name, &api_key.unwrap(), ep_id_deref)
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

                // Clear global loading state
                dispatch.reduce_mut(|state| {
                    state.is_loading = Some(false);
                    web_sys::console::log_1(&"Loading state set to false".into());
                });
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

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                match *page_state {
                PageState::Shown => share_url_modal,
                _ => html! {},
                }
            }
            {
                if *loading { // If loading is true, display the loading animation
                    html! {
                        <div class="loading-animation">
                            <div class="frame1"></div>
                            <div class="frame2"></div>
                            <div class="frame3"></div>
                            <div class="frame4"></div>
                            <div class="frame5"></div>
                            <div class="frame6"></div>
                        </div>
                    }
                } else {
                    if let Some(episode) = state.fetched_episode.clone() {
                        let episode_url_clone = episode.episode.episodeurl.clone();
                        let episode_title_clone = episode.episode.episodetitle.clone();
                        let episode_descripton_clone = episode.episode.episodedescription.clone();
                        let episode_release_clone = episode.episode.episodepubdate.clone();
                        let episode_artwork_clone = episode.episode.episodeartwork.clone();
                        let episode_duration_clone = episode.episode.episodeduration.clone();
                        let podcast_of_episode = episode.episode.podcastid.clone();
                        let episode_listened_clone = episode.episode.listenduration.clone();
                        let episode_id_clone = episode.episode.episodeid.clone();
                        let episode_is_youtube = episode.episode.is_youtube.clone();

                        let sanitized_description = sanitize_html_with_blank_target(&episode.episode.episodedescription.clone());
                        let description = sanitized_description;

                        let episode_url_for_closure = episode_url_clone.clone();
                        let episode_title_for_closure = episode_title_clone.clone();
                        let episode_description_for_closure = episode_descripton_clone.clone();
                        let episode_release_date_for_closure = episode_release_clone.clone();
                        let episode_artwork_for_closure = episode_artwork_clone.clone();
                        let episode_duration_for_closure = episode_duration_clone.clone();
                        let episode_id_for_closure = episode_id_clone.clone();
                        let listener_duration_for_closure = episode_listened_clone.clone();

                        let user_id_play = user_id.clone();
                        let server_name_play = server_name.clone();
                        let api_key_play = api_key.clone();
                        let audio_dispatch = audio_dispatch.clone();

                        let is_playing = {
                            let audio_state = audio_state.clone();
                            let episode_id = episode_id_for_closure;

                            audio_state.currently_playing.as_ref().map_or(false, |current| {
                                current.episode_id == episode_id && audio_state.audio_playing.unwrap_or(false)
                            })
                        };

                        // Create the play toggle handler
                        let handle_play_click = {
                            let audio_state = audio_state.clone();
                            let audio_dispatch = audio_dispatch.clone();
                            let episode_id = episode_id_for_closure.clone();

                            // Create the base on_play_click callback
                            let on_play = on_play_click(
                                episode_url_for_closure.clone(),
                                episode_title_for_closure.clone(),
                                episode_description_for_closure.clone(),
                                episode_release_date_for_closure.clone(),
                                episode_artwork_for_closure.clone(),
                                episode_duration_for_closure.clone(),
                                episode_id_for_closure.clone(),
                                listener_duration_for_closure.clone(),
                                api_key_play.unwrap().unwrap(),
                                user_id_play.unwrap(),
                                server_name_play.unwrap(),
                                audio_dispatch.clone(),
                                audio_state.clone(),
                                None,
                                Some(episode_is_youtube),
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
                                let post_dispatch = complete_post.clone();
                                let server_name_copy = complete_server_name.clone();
                                let api_key_copy = complete_api_key.clone();
                                let is_youtube = episode_is_youtube;
                                let request = MarkEpisodeCompletedRequest {
                                    episode_id: episode_id_for_closure,
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
                                            post_dispatch.reduce_mut(|state| {
                                                if let Some(completed_episodes) = state.completed_episodes.as_mut() {
                                                    if let Some(pos) =
                                                        completed_episodes.iter().position(|&id| id == episode_id_for_closure)
                                                    {
                                                        completed_episodes.remove(pos);
                                                    } else {
                                                        completed_episodes.push(episode_id_for_closure);
                                                    }
                                                } else {
                                                    state.completed_episodes = Some(vec![episode_id_for_closure]);
                                                }
                                            });
                                        }
                                        Err(e) => {
                                            let formatted_error = format_error_message(&e.to_string());
                                            post_dispatch.reduce_mut(|state| {
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
                                let post_dispatch = uncomplete_post.clone();
                                let server_name_copy = uncomplete_server_name.clone();
                                let api_key_copy = uncomplete_api_key.clone();
                                let request = MarkEpisodeCompletedRequest {
                                    episode_id: episode_id_for_closure,
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
                                            post_dispatch.reduce_mut(|state| {
                                                if let Some(completed_episodes) = state.completed_episodes.as_mut() {
                                                    if let Some(pos) =
                                                        completed_episodes.iter().position(|&id| id == episode_id_for_closure)
                                                    {
                                                        completed_episodes.remove(pos);
                                                    } else {
                                                        completed_episodes.push(episode_id_for_closure);
                                                    }
                                                } else {
                                                    state.completed_episodes = Some(vec![episode_id_for_closure]);
                                                }
                                            });
                                        }
                                        Err(e) => {
                                            let formatted_error = format_error_message(&e.to_string());
                                            post_dispatch.reduce_mut(|state| {
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
                            let episode_id = episode_id_for_closure;
                            let is_youtube = episode_is_youtube;

                            Callback::from(move |_: MouseEvent| {
                                let server_name_copy = server_name_queue.clone();
                                let api_key_copy = api_key_queue.clone();
                                let queue_post = dispatch_queue.clone();
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
                                            queue_post.reduce_mut(|state| state.error_message = Some(format!("{}", formatted_error)));
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
                            let episode_id = episode_id_for_closure;
                            let is_youtube = episode_is_youtube;

                            Callback::from(move |_: MouseEvent| {
                                let server_name_copy = saved_server_name.clone();
                                let api_key_copy = saved_api_key.clone();
                                let post_state = save_post.clone();
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
                                            post_state.reduce_mut(|state| state.error_message = Some(format!("{}", formatted_error)));
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
                            let episode_id = episode_id_for_closure;

                            Callback::from(move |_: MouseEvent| {
                                let server_name_copy = download_server_name.clone();
                                let api_key_copy = download_api_key.clone();
                                let post_state = download_post.clone();
                                let is_downloaded = *download_status;
                                let download_status = download_status.clone();
                                let request = DownloadEpisodeRequest {
                                    episode_id,
                                    user_id: user_id_download.unwrap(),
                                    is_youtube,
                                };

                                // Set loading state
                                post_state.reduce_mut(|state| state.is_loading = Some(true));

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
                                            post_state.reduce_mut(|state| state.error_message = Some(format!("{}", formatted_error)));
                                        }
                                    }

                                    // Clear loading state
                                    post_state.reduce_mut(|state| state.is_loading = Some(false));
                                };
                                wasm_bindgen_futures::spawn_local(future);
                            })
                        };

                        let datetime = parse_date(&episode.episode.episodepubdate, &state.user_tz);
                        let date_format = match_date_format(state.date_format.as_deref());
                        let format_duration = format_time(episode.episode.episodeduration as f64);
                        let format_release = format!("{}", format_datetime(&datetime, &state.hour_preference, date_format));

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
                                let dispatch = dispatch.clone();
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
                                            &podcast_id
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
                                                dispatch.clone(),
                                                server_name.unwrap(),
                                                api_key,
                                                &history,
                                                details.podcastindexid.unwrap_or(0),
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
                                            dispatch.reduce_mut(move |state| {
                                                let formatted_error = format_error_message(&error.to_string());
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

                        // let format_duration = format!("Duration: {} minutes", e / 60); // Assuming duration is in seconds
                        // let format_release = format!("Released on: {}", &episode.episode.EpisodePubDate);
                        // Before creating the play toggle handler, add this:
                        let listen_duration_percentage = if let Some(listen_duration) = episode.episode.listenduration {
                            if episode.episode.episodeduration > 0 {
                                ((listen_duration as f64 / episode.episode.episodeduration as f64) * 100.0).min(100.0)
                            } else {
                                0.0
                            }
                        } else {
                            0.0
                        };

                        // In the Episode component rendering logic, add this where appropriate:
                        let progress_bar = if let Some(listen_duration) = episode.episode.listenduration {
                            if listen_duration > 0 {
                                let listen_duration_formatted = format_time(listen_duration as f64);
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
                            html! {
                                <div class="mobile-layout">
                                <div class="episode-layout-container">
                                        <div class="item-header-mobile-cover-container">
                                            <FallbackImage
                                                src={episode.episode.episodeartwork.clone()}
                                                // onclick={on_title_click.clone()}
                                                alt={format!("{}{}", i18n_cover_alt_text, episode.episode.episodeartwork.clone())}
                                                class="episode-artwork rounded-corners"
                                            />
                                        </div>
                                            <div class="episode-details">
                                            <p class="item-header-pod mt-2 justify-center items-center" onclick={on_title_click.clone()}>{ &episode.episode.podcastname }</p>
                                            <div class="items-center space-x-2 cursor-pointer">
                                                <h2 class="episode-title item-header-title">
                                                    { &episode.episode.episodetitle }
                                                    {
                                                        if *completion_status.clone() {
                                                            html! {
                                                                <i class="ph ph-check-circle text-2xl text-green-500 ml-2"></i>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        html! {
                                                            <>
                                                                <button onclick={create_share_link.clone()} class="ml-2">
                                                                    <i class="ph ph-share-network text-2xl"></i>
                                                                </button>
                                                                <button onclick={download_episode_file.clone()} class="ml-2" disabled={*download_in_progress}>
                                                                    {
                                                                        if *download_in_progress {
                                                                            html! { <i class="ph ph-spinner text-2xl animate-spin"></i> }
                                                                        } else {
                                                                            html! { <i class="ph ph-download text-2xl"></i> }
                                                                        }
                                                                    }
                                                                </button>
                                                            </>
                                                        }
                                                    }
                                                </h2>
                                            </div>
                                            // <h2 class="episode-title">{ &episode.episode.episodetitle }</h2>
                                            <div class="flex justify-center items-center item-header-details">
                                                <p class="episode-duration">{ format_duration }</p>
                                                <span class="episode-duration">{"\u{00a0}-\u{00a0}"}</span>
                                                <p class="episode-release-date">{ format_release }</p>
                                            </div>
                                            { progress_bar }


                                            {
                                                if let Some(transcript) = &audio_state.episode_page_transcript {
                                                    if !transcript.is_empty() {
                                                        let transcript_clone = transcript.clone();
                                                        let dispatch = dispatch.clone();
                                                        let dispatch_call = dispatch.clone();
                                                        html! {
                                                            <>
                                                            <div class="header-info pb-2 pt-2">
                                                                <button
                                                                    onclick={Callback::from(move |_| {
                                                                        dispatch_call.reduce_mut(|state| {
                                                                            state.show_transcript_modal = Some(true);
                                                                            state.current_transcripts = Some(transcript_clone.clone());
                                                                        });
                                                                    })}
                                                                    title={i18n_transcript.clone()}
                                                                    class="font-bold item-container-button"
                                                                >
                                                                    { &i18n_view_transcript }
                                                                </button>
                                                            </div>

                                                            if let Some(show_modal) = state.show_transcript_modal {
                                                                if show_modal {
                                                                    <TranscriptModal
                                                                        transcripts={state.current_transcripts.clone().unwrap_or_default()}
                                                                        server_name={server_name.unwrap_or_default()}
                                                                        api_key={api_key.unwrap().unwrap_or_default()}
                                                                        onclose={Callback::from(move |_| {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.show_transcript_modal = Some(false);
                                                                            });
                                                                        })}
                                                                    />
                                                                }
                                                            }
                                                            </>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            {
                                                if !*ep_2_loading {
                                                    if let Some(people) = &audio_state.episode_page_people {
                                                        if !people.is_empty() {
                                                            // Add key prop that changes with episode
                                                            html! {
                                                                <div class="header-info mb-2 overflow-x-auto whitespace-nowrap scroll-container">
                                                                    <HostDropdown
                                                                        key={format!("host-{}", episode.episode.episodeid)} // Add this key prop
                                                                        title={i18n_in_this_episode.clone()}
                                                                        hosts={people.clone()}
                                                                        podcast_feed_url={episode.episode.episodeurl.clone()}
                                                                        podcast_id={episode.episode.podcastid}
                                                                        podcast_index_id={episode.episode.podcastindexid.unwrap_or(0)}
                                                                    />
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                    <div class="episode-action-buttons">
                                    {
                                        if should_show_buttons {
                                            html! {
                                                <>
                                                    // Play button always shown if media exists
                                                    <div class="button-row">
                                                        <button onclick={handle_play_click} class="play-button flex items-center justify-center gap-2 mt-2">
                                                            { if is_playing {
                                                                html! { <i class="ph ph-pause text-2xl"></i> }
                                                            } else {
                                                                html! { <i class="ph ph-play text-2xl"></i> }
                                                            }}
                                                            { if is_playing { &i18n_pause } else { &i18n_play } }
                                                        </button>

                                                        // Other buttons only if episode is in database
                                                        {
                                                            if *ep_in_db {
                                                                html! {
                                                                    <>
                                                                        <button onclick={toggle_queue} class="queue-button flex items-center justify-center gap-2 mt-2">
                                                                            { if *queue_status {
                                                                                html! { <i class="ph ph-queue text-2xl"></i> }
                                                                            } else {
                                                                                html! { <i class="ph ph-queue text-2xl"></i> }
                                                                            }}
                                                                            { if *queue_status { &i18n_remove_from_queue } else { &i18n_add_to_queue } }
                                                                        </button>
                                                                        <button onclick={toggle_save} class="save-button flex items-center justify-center gap-2 mt-2">
                                                                            { if *save_status {
                                                                                html! { <i class="ph ph-heart-break text-2xl"></i> }
                                                                            } else {
                                                                                html! { <i class="ph ph-heart text-2xl"></i> }
                                                                            }}
                                                                            { if *save_status { &i18n_unsave } else { &i18n_save } }
                                                                        </button>
                                                                    </>
                                                                }
                                                            } else {
                                                                html! {
                                                                    <p class="no-media-warning item_container-text">
                                                                        {&i18n_add_podcast_to_enable_actions}
                                                                    </p>
                                                                }
                                                            }
                                                        }
                                                    </div>

                                                    // Second row of buttons only if in database
                                                    {
                                                        if *ep_in_db {
                                                            html! {
                                                                <div class="button-row">
                                                                    <button onclick={toggle_download} class="download-button-ep flex items-center justify-center gap-2 mt-2">
                                                                        { if *download_status {
                                                                            html! { <i class="ph ph-trash text-2xl"></i> }
                                                                        } else {
                                                                            html! { <i class="ph ph-download text-2xl"></i> }
                                                                        }}
                                                                        { if *download_status { &i18n_remove_download } else { &i18n_download } }
                                                                    </button>
                                                                    <button onclick={toggle_completion} class="download-button-ep flex items-center justify-center gap-2 mt-2">
                                                                        { if *completion_status {
                                                                            html! { <i class="ph ph-x-circle text-2xl"></i> }
                                                                        } else {
                                                                            html! { <i class="ph ph-check-fat text-2xl"></i> }
                                                                        }}
                                                                        { if *completion_status { &i18n_mark_incomplete } else { &i18n_mark_complete } }
                                                                    </button>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {} // Empty VNode when not in database
                                                        }
                                                    }
                                                </>
                                            }
                                        } else {
                                            html! {
                                                <p class="no-media-warning item_container-text play-button">
                                                    {&i18n_this_item_contains_no_media}
                                                </p>
                                            }
                                        }
                                    }

                                    </div>
                                    <div class="episode-single-desc episode-description">
                                    // <p>{ description }</p>
                                    <div class="item_container-text episode-description-container">
                                        <SafeHtml
                                            html={description}
                                            episode_url={Some(episode.episode.episodeurl.clone())}
                                            episode_title={Some(episode.episode.episodetitle.clone())}
                                            episode_description={Some(episode.episode.episodedescription.clone())}
                                            episode_release_date={Some(episode.episode.episodepubdate.clone())}
                                            episode_artwork={Some(episode.episode.episodeartwork.clone())}
                                            episode_duration={episode.episode.episodeduration}
                                            episode_id={Some(episode.episode.episodeid)}
                                            is_youtube={episode.episode.is_youtube}
                                        />
                                    </div>
                                    </div>
                                </div>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="episode-layout-container">
                                    <div class="episode-top-info">
                                    <FallbackImage
                                        src={episode.episode.episodeartwork.clone()}
                                        alt={format!("{}{}", i18n_cover_alt_text, episode.episode.episodeartwork.clone())}
                                        class="episode-artwork rounded-corners"
                                        style="max-width: 375px; width: 25%; height: auto;"
                                    />
                                        // Add overflow-hidden to episode-details to prevent children from expanding it
                                        <div class="episode-details overflow-hidden">
                                            <h1 class="podcast-title hover-pointer-podcast-name" onclick={on_title_click.clone()}>{ &episode.episode.podcastname }</h1>
                                            // Add max-w-full to ensure title container stays within bounds
                                            <div class="flex items-center space-x-2 cursor-pointer max-w-full">
                                                <h2 class="episode-title truncate">{ &episode.episode.episodetitle }</h2>
                                                {
                                                    if *completion_status.clone() {
                                                        html! {
                                                            <i class="ph ph-check-circle text-2xl text-green-500 ml-2"></i>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </div>
                                            // <h2 class="episode-title">{ &episode.episode.episodetitle }</h2>
                                            <p class="episode-duration">{ format_duration }</p>
                                            <p class="episode-release-date">{ format_release }</p>
                                            { progress_bar }
                                            {
                                                if let Some(transcript) = &audio_state.episode_page_transcript {
                                                    if !transcript.is_empty() {
                                                        let transcript_clone = transcript.clone();
                                                        let dispatch = dispatch.clone();
                                                        let dispatch_call = dispatch.clone();
                                                        html! {
                                                            <>
                                                            <div class="header-info pb-2 pt-2">
                                                                <button
                                                                    onclick={Callback::from(move |_| {
                                                                        dispatch_call.reduce_mut(|state| {
                                                                            state.show_transcript_modal = Some(true);
                                                                            state.current_transcripts = Some(transcript_clone.clone());
                                                                        });
                                                                    })}
                                                                    title={i18n_transcript.clone()}
                                                                    class="font-bold item-container-button"
                                                                >
                                                                    { &i18n_view_transcript }
                                                                </button>
                                                            </div>

                                                            if let Some(show_modal) = state.show_transcript_modal {
                                                                if show_modal {
                                                                    <TranscriptModal
                                                                        transcripts={state.current_transcripts.clone().unwrap_or_default()}
                                                                        server_name={server_name.unwrap_or_default()}
                                                                        api_key={api_key.unwrap().unwrap_or_default()}
                                                                        onclose={Callback::from(move |_| {
                                                                            dispatch.reduce_mut(|state| {
                                                                                state.show_transcript_modal = Some(false);
                                                                            });
                                                                        })}
                                                                    />
                                                                }
                                                            }
                                                            </>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            {
                                                if !*ep_2_loading {
                                                    if let Some(people) = &audio_state.episode_page_people {
                                                        if !people.is_empty() {
                                                            html! {
                                                                // Modified container classes
                                                                <div class="header-info mb-2 w-full overflow-hidden">
                                                                    <div class="overflow-x-auto whitespace-nowrap" style="max-width: calc(100% - 2rem);">
                                                                        <HostDropdown
                                                                            title={i18n_in_this_episode.clone()}
                                                                            hosts={people.clone()}
                                                                            podcast_feed_url={episode.episode.episodeurl.clone()}
                                                                            podcast_id={episode.episode.podcastid}
                                                                            podcast_index_id={episode.episode.podcastindexid.unwrap_or(0)}
                                                                        />
                                                                    </div>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            <div class="flex gap-2">
                                                <button
                                                    class="share-button font-bold py-2 px-4 rounded"
                                                    onclick={create_share_link.clone()}
                                                >
                                                    {&i18n_share_episode}
                                                </button>
                                                <button
                                                    class="download-button font-bold py-2 px-4 rounded"
                                                    onclick={download_episode_file.clone()}
                                                    disabled={post_state.is_loading.unwrap_or(false)}
                                                >
                                                    {
                                                        if post_state.is_loading.unwrap_or(false) {
                                                            web_sys::console::log_1(&"UI: Showing loading spinner".into());
                                                            html! {
                                                                <>
                                                                    <div class="animate-spin inline-block w-4 h-4 mr-2 border-2 border-gray-300 border-t-white rounded-full"></div>
                                                                    {&i18n_downloading}
                                                                </>
                                                            }
                                                        } else {
                                                            web_sys::console::log_1(&format!("UI: Not loading, state: {:?}", post_state.is_loading).into());
                                                            html! { {&i18n_download_episode} }
                                                        }
                                                    }
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                    <div class="episode-action-buttons">
                                    {
                                        if should_show_buttons {
                                            html! {
                                                <>
                                                <button onclick={handle_play_click} class="play-button flex items-center justify-center gap-2">
                                                    { if is_playing {
                                                        html! { <i class="ph ph-pause text-2xl"></i> }
                                                    } else {
                                                        html! { <i class="ph ph-play text-2xl"></i> }
                                                    }}
                                                    { if is_playing { &i18n_pause } else { &i18n_play } }
                                                </button>
                                                {
                                                    if *ep_in_db {
                                                        html! {
                                                            <>
                                                            <button onclick={toggle_queue} class="queue-button flex items-center justify-center gap-2">
                                                                { if *queue_status {
                                                                    html! { <i class="ph ph-queue text-2xl"></i> }
                                                                } else {
                                                                    html! { <i class="ph ph-queue text-2xl"></i> }
                                                                }}
                                                                { if *queue_status { &i18n_remove_from_queue } else { &i18n_add_to_queue } }
                                                            </button>
                                                            <button onclick={toggle_save} class="save-button flex items-center justify-center gap-2">
                                                                { if *save_status {
                                                                    html! { <i class="ph ph-heart-break text-2xl"></i> }
                                                                } else {
                                                                    html! { <i class="ph ph-heart text-2xl"></i> }
                                                                }}
                                                                { if *save_status { &i18n_unsave } else { &i18n_save } }
                                                            </button>
                                                            <button onclick={toggle_download} class="download-button-ep flex items-center justify-center gap-2">
                                                                { if *download_status {
                                                                    html! { <i class="ph ph-trash text-2xl"></i> }
                                                                } else {
                                                                    html! { <i class="ph ph-download text-2xl"></i> }
                                                                }}
                                                                { if *download_status { &i18n_remove_download } else { &i18n_download } }
                                                            </button>
                                                            <button onclick={toggle_completion} class="download-button-ep flex items-center justify-center gap-2">
                                                                { if *completion_status {
                                                                    html! { <i class="ph ph-x-circle text-2xl"></i> }
                                                                } else {
                                                                    html! { <i class="ph ph-check-fat text-2xl"></i> }
                                                                }}
                                                                { if *completion_status { &i18n_mark_episode_incomplete } else { &i18n_mark_episode_complete } }
                                                            </button>
                                                            </>
                                                        }
                                                    } else {
                                                        html! {
                                                            <p class="no-media-warning item_container-text play-button">
                                                                {&i18n_add_podcast_to_enable_actions}
                                                            </p>
                                                        }
                                                    }
                                                }
                                                </>
                                            }
                                        } else {
                                            html! {
                                                <p class="no-media-warning item_container-text play-button">
                                                    {&i18n_this_item_contains_no_media}
                                                </p>
                                            }
                                        }
                                    }

                                    </div>
                                    <hr class="episode-divider" />
                                    <div class="episode-single-desc episode-description">
                                    // <p>{ description }</p>
                                    <div class="item_container-text episode-description-container">
                                        <SafeHtml
                                            html={description}
                                            episode_url={Some(episode.episode.episodeurl.clone())}
                                            episode_title={Some(episode.episode.episodetitle.clone())}
                                            episode_description={Some(episode.episode.episodedescription.clone())}
                                            episode_release_date={Some(episode.episode.episodepubdate.clone())}
                                            episode_artwork={Some(episode.episode.episodeartwork.clone())}
                                            episode_duration={episode.episode.episodeduration}
                                            episode_id={Some(episode.episode.episodeid)}
                                            is_youtube={episode.episode.is_youtube}
                                        />
                                    </div>
                                    </div>
                                </div>
                            }
                        };  // Add semicolon here
                        // item

                        layout
                    } else {
                        empty_message(
                            &i18n_unable_to_display_episode,
                            &i18n_something_went_wrong_description
                        )
                    }
                }
            }
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
            } else {
                html! {}
            }
        }
        </div>
        <App_drawer />
        </>
    }
}
