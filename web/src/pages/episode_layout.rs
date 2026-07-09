use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, EpisodeNavigationState, EpisodeStatusState, NotificationState, PageLoadState, PodcastFeedState, SearchState, UIState};
use crate::requests::episode::Episode;
use crate::components::gen_components::{FallbackImage, Search_nav, UseScrollToTop};
use crate::components::gen_funcs::{
    format_error_message, get_default_sort_direction, get_filter_preference, set_filter_preference,
};
use crate::components::loading::Loading;
use crate::components::episode_list_view::EpisodeListView;
use crate::pages::podcast_layout::ClickedFeedURL;
use crate::requests::pod_req::{
    call_add_category, call_add_podcast, call_adjust_silence_trim, call_adjust_skip_times,
    call_get_silence_trim, SilenceTrimRequest,
    call_adjust_auto_transcribe, call_get_ai_status, call_get_auto_transcribe, AutoTranscribeRequest,
    call_get_auto_ad_detect, call_adjust_auto_ad_detect, AutoAdDetectRequest,
    call_get_ad_skip_auto_activate, call_adjust_ad_skip_auto_activate,
    call_bulk_download_episodes,
    call_bulk_mark_episodes_completed, call_bulk_queue_episodes, call_bulk_save_episodes,
    call_check_podcast, call_clear_playback_speed, call_download_all_podcast,
    call_enable_auto_download, call_enable_auto_play_next, call_enable_auto_queue,
    call_fetch_podcasting_2_pod_data, call_get_auto_download_status,
    call_get_auto_play_next_status, call_get_auto_queue_status,
    call_get_feed_cutoff_days, call_get_merged_podcasts, call_get_play_episode_details,
    call_get_podcast_details, call_get_podcast_id_from_ep, call_get_podcast_id_from_ep_name,
    call_get_podcast_favorite_status, call_get_podcast_notifications_status, call_get_podcasts,
    call_get_rss_key, call_merge_podcasts, call_remove_category, call_remove_podcasts_name,
    call_remove_youtube_channel, call_set_playback_speed, call_toggle_podcast_favorite,
    call_toggle_podcast_notifications,
    call_unmerge_podcast, call_update_feed_cutoff_days, call_update_podcast_info,
    call_get_podcast_auto_delete_days, call_set_podcast_auto_delete_days,
    call_clear_podcast_auto_delete_days,
    AddCategoryRequest, AutoDownloadRequest, BulkEpisodeActionRequest, ClearPlaybackSpeedRequest,
    DownloadAllPodcastRequest, FetchPodcasting2PodDataRequest, AutoPlayNextRequest, AutoQueueRequest,
    PlaybackSpeedRequest, SetAutoDeleteDaysRequest, ClearAutoDeleteDaysRequest,
    PodcastDetails, PodcastValues, RemoveCategoryRequest, RemovePodcastValuesName,
    RemoveYouTubeChannelValues, SkipTimesRequest, UpdateFeedCutoffDaysRequest,
};
use crate::requests::search_pods::call_get_podcast_details_dynamic;
use crate::requests::search_pods::call_get_podcast_episodes;
use crate::requests::setting_reqs::{
    call_get_podcast_cover_preference, call_set_global_podcast_cover_preference,
};

use htmlentity::entity::decode;
use htmlentity::entity::ICodedDataTrait;
use i18nrs::yew::use_translation;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen_futures::spawn_local;
use web_sys::Element;
use web_sys::{console, window, Event, HtmlInputElement, MouseEvent, UrlSearchParams};
use yew::prelude::*;
use yew::Properties;
use yew::{function_component, html, use_effect_with, use_node_ref, Callback, Html, TargetCast};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[allow(dead_code)]
fn add_icon() -> Html {
    html! {
        <i class="ph ph-plus-circle text-2xl"></i>
    }
}

#[allow(dead_code)]
fn payments_icon() -> Html {
    html! {
        <i class="ph ph-money-wavy text-2xl"></i>
    }
}

#[allow(dead_code)]
fn rss_icon() -> Html {
    html! {
        <i class="ph ph-rss text-2xl"></i>
    }
}

#[allow(dead_code)]
fn website_icon() -> Html {
    html! {
        <i class="ph ph-globe text-2xl"></i>
    }
}

#[allow(dead_code)]
fn trash_icon() -> Html {
    html! {
        <i class="ph ph-trash text-2xl"></i>

    }
}

#[allow(dead_code)]
fn settings_icon() -> Html {
    html! {
        <i class="ph ph-gear text-2xl"></i>

    }
}
#[allow(dead_code)]
fn download_icon() -> Html {
    html! {
        <i class="ph ph-download text-2xl"></i>

    }
}
#[allow(dead_code)]
fn no_icon() -> Html {
    html! {}
}

#[allow(dead_code)]
fn play_icon() -> Html {
    html! {
    <svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 -960 960 960" width="24"><path d="m380-300 280-180-280-180v360ZM480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Zm0-80q134 0 227-93t93-227q0-134-93-227t-227-93q-134 0-227 93t-93 227q0 134 93 227t227 93Zm0-320Z"/></svg>
        }
}

#[allow(dead_code)]
fn pause_icon() -> Html {
    html! {
        <svg xmlns="http://www.w3.org/2000/svg" height="24" viewBox="0 -960 960 960" width="24"><path d="M360-320h80v-320h-80v320Zm160 0h80v-320h-80v320ZM480-80q-83 0-156-31.5T197-197q-54-54-85.5-127T80-480q0-83 31.5-156T197-763q54-54 127-85.5T480-880q83 0 156 31.5T763-763q54 54 85.5 127T880-480q0 83-31.5 156T763-197q-54 54-127 85.5T480-80Zm0-80q134 0 227-93t93-227q0-134-93-227t-227-93q-134 0-227 93t-93 227q0 134 93 227t227 93Zm0-320Z"/></svg>
    }
}

#[derive(Properties, PartialEq)]
pub struct Props {
    pub html: String,
}

#[allow(dead_code)]
fn sanitize_html(html: &str) -> String {
    let cleaned_html = ammonia::clean(html);
    let decoded_data = decode(cleaned_html.as_bytes());
    match decoded_data.to_string() {
        Ok(decoded_html) => decoded_html,
        Err(_) => String::from("Invalid HTML content"),
    }
}

#[allow(dead_code)]
fn get_rss_base_url() -> String {
    let window = window().expect("no global `window` exists");
    let location = window.location();
    let current_url = location
        .href()
        .unwrap_or_else(|_| "Unable to retrieve URL".to_string());

    if let Some(storage) = window.local_storage().ok().flatten() {
        if let Ok(Some(auth_state)) = storage.get_item("userAuthState") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&auth_state) {
                if let Some(server_name) = json
                    .get("auth_details")
                    .and_then(|auth| auth.get("server_name"))
                    .and_then(|name| name.as_str())
                {
                    return format!("{}/rss", server_name);
                }
            }
        }
    }
    // Fallback to using the current URL's origin
    format!(
        "{}/rss",
        current_url.split('/').take(3).collect::<Vec<_>>().join("/")
    )
}

#[derive(Clone, PartialEq)]
#[allow(dead_code)]
pub enum EpisodeSortDirection {
    NewestFirst,
    OldestFirst,
    ShortestFirst,
    LongestFirst,
    TitleAZ,
    TitleZA,
}

#[derive(Properties, PartialEq)]
pub struct PodcastMergeSelectorProps {
    pub selected_podcasts: Vec<i32>,
    pub on_select: Callback<Vec<i32>>,
    pub available_podcasts: Vec<crate::requests::pod_req::Podcast>,
    pub loading: bool,
}

#[function_component(PodcastMergeSelector)]
pub fn podcast_merge_selector(props: &PodcastMergeSelectorProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_loading_podcasts = i18n.t("episodes_layout.loading_podcasts").to_string();
    let i18n_select_podcasts_to_merge_hint = i18n.t("episodes_layout.select_podcasts_to_merge").to_string();
    let is_open = use_state(|| false);
    let dropdown_ref = use_node_ref();

    // Handle clicking outside to close dropdown
    {
        let is_open = is_open.clone();
        let dropdown_ref = dropdown_ref.clone();

        use_effect_with(dropdown_ref.clone(), move |dropdown_ref| {
            let document = web_sys::window().unwrap().document().unwrap();
            let dropdown_element = dropdown_ref.cast::<HtmlInputElement>();

            let listener =
                wasm_bindgen::closure::Closure::wrap(Box::new(move |event: web_sys::Event| {
                    if let Some(target) = event.target() {
                        if let Some(dropdown) = &dropdown_element {
                            if let Ok(node) = target.dyn_into::<web_sys::Node>() {
                                if !dropdown.contains(Some(&node)) {
                                    is_open.set(false);
                                }
                            }
                        }
                    }
                }) as Box<dyn FnMut(_)>);

            document
                .add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
                .unwrap();

            move || {
                document
                    .remove_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
                    .unwrap();
            }
        });
    }

    let toggle_dropdown = {
        let is_open = is_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            is_open.set(!*is_open);
        })
    };

    let toggle_podcast_selection = {
        let selected = props.selected_podcasts.clone();
        let on_select = props.on_select.clone();

        Callback::from(move |podcast_id: i32| {
            let mut new_selection = selected.clone();
            if let Some(pos) = new_selection.iter().position(|&id| id == podcast_id) {
                new_selection.remove(pos);
            } else {
                new_selection.push(podcast_id);
            }
            on_select.emit(new_selection);
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    html! {
        <div class="relative" ref={dropdown_ref}>
            <button
                type="button"
                onclick={toggle_dropdown.clone()}
                class="search-bar-input border text-sm rounded-lg block w-full p-2.5 flex items-center"
                disabled={props.loading}
            >
                <div class="flex items-center flex-grow">
                    if props.loading {
                        <span class="flex-grow text-left">{ &i18n_loading_podcasts }</span>
                    } else if props.selected_podcasts.is_empty() {
                        <span class="flex-grow text-left">{ &i18n_select_podcasts_to_merge_hint }</span>
                    } else {
                        <span class="flex-grow text-left">
                            {format!("{} {} selected",
                                props.selected_podcasts.len(),
                                if props.selected_podcasts.len() == 1 { "podcast" } else { "podcasts" }
                            )}
                        </span>
                    }
                    <i class={classes!(
                        "ph",
                        "ph-caret-down",
                        "transition-transform",
                        "duration-200",
                        if *is_open { "rotate-180" } else { "" }
                    )}></i>
                </div>
            </button>

            if *is_open && !props.loading {
                <div
                    class="absolute z-50 mt-1 w-full rounded-lg shadow-lg modal-container max-h-[400px] overflow-y-auto"
                    onclick={stop_propagation}
                >
                    <div class="max-h-[400px] overflow-y-auto p-2 space-y-1">
                        {
                            props.available_podcasts.iter().map(|podcast| {
                                let is_selected = props.selected_podcasts.contains(&podcast.podcastid);
                                let onclick = {
                                    let toggle = toggle_podcast_selection.clone();
                                    let id = podcast.podcastid;
                                    Callback::from(move |_| toggle.emit(id))
                                };

                                html! {
                                    <div
                                        key={podcast.podcastid}
                                        {onclick}
                                        class={classes!(
                                            "flex",
                                            "items-center",
                                            "p-2",
                                            "rounded-lg",
                                            "cursor-pointer",
                                            "hover:bg-gray-700",
                                            "transition-colors",
                                            if is_selected { "bg-gray-700" } else { "" }
                                        )}
                                    >
                                        <FallbackImage
                                            src={podcast.artworkurl.clone().unwrap_or_else(|| "/static/assets/favicon.png".to_string())}
                                            alt={format!("Cover for {}", podcast.podcastname)}
                                            class="w-12 h-12 rounded object-cover"
                                        />
                                        <span class="ml-3 flex-grow truncate">
                                            {&podcast.podcastname}
                                        </span>
                                        if is_selected {
                                            <i class="ph ph-check text-blue-500 text-xl"></i>
                                        }
                                    </div>
                                }
                            }).collect::<Html>()
                        }
                    </div>
                </div>
            }
        </div>
    }
}

#[function_component(EpisodeLayout)]
pub fn episode_layout() -> Html {
    let (i18n, _) = use_translation();
    let is_added = use_state(|| false);
    let (search_state, _search_dispatch) = use_store::<AppState>();
    let (search_data, _) = use_store::<SearchState>();
    let (state, _dispatch) = use_store::<UIState>();
    let (podcast_state, _podcast_dispatch) = use_store::<PodcastFeedState>();
    let podcast_feed_results = search_data.podcast_feed_results.clone();
    let clicked_podcast_info = podcast_state.clicked_podcast_info.clone();

    // Capture i18n strings before they get moved - this is a large component with many strings
    let i18n_youtube_channel_successfully_removed = i18n
        .t("episodes_layout.youtube_channel_successfully_removed")
        .to_string();
    let i18n_podcast_successfully_removed = i18n
        .t("episodes_layout.podcast_successfully_removed")
        .to_string();
    let i18n_failed_to_remove_youtube_channel = i18n
        .t("episodes_layout.failed_to_remove_youtube_channel")
        .to_string();
    let i18n_failed_to_remove_podcast = i18n
        .t("episodes_layout.failed_to_remove_podcast")
        .to_string();
    let i18n_playback_speed_updated = i18n.t("episodes_layout.playback_speed_updated").to_string();
    let i18n_error_updating_playback_speed = i18n
        .t("episodes_layout.error_updating_playback_speed")
        .to_string();
    let i18n_podcast_successfully_added = i18n
        .t("episodes_layout.podcast_successfully_added")
        .to_string();
    let i18n_failed_to_add_podcast = i18n.t("episodes_layout.failed_to_add_podcast").to_string();
    let i18n_no_categories_available = i18n
        .t("episodes_layout.no_categories_available")
        .to_string();

    // Additional i18n strings used throughout the component
    let i18n_category_name_cannot_be_empty = i18n
        .t("episodes_layout.category_name_cannot_be_empty")
        .to_string();
    let i18n_loading_rss_key = i18n.t("episodes_layout.loading_rss_key").to_string();
    let i18n_rss_feed_url = i18n.t("episodes_layout.rss_feed_url").to_string();
    let i18n_rss_feed_note = i18n.t("episodes_layout.rss_feed_note").to_string();
    let i18n_rss_feed_instruction = i18n.t("episodes_layout.rss_feed_instruction").to_string();
    let i18n_rss_feed_warning = i18n.t("episodes_layout.rss_feed_warning").to_string();
    let i18n_download_future_episodes = i18n
        .t("episodes_layout.download_future_episodes")
        .to_string();
    let i18n_get_notifications_new_episodes = i18n
        .t("episodes_layout.get_notifications_new_episodes")
        .to_string();
    let i18n_default_playback_speed = i18n.t("episodes_layout.default_playback_speed").to_string();
    let i18n_playback_speed_description = i18n
        .t("episodes_layout.playback_speed_description")
        .to_string();
    let i18n_playback_speed_custom_badge = i18n
        .t("episodes_layout.playback_speed_custom_badge")
        .to_string();
    let i18n_playback_speed_global_badge = i18n
        .t("episodes_layout.playback_speed_global_badge")
        .to_string();
    let i18n_auto_delete_label = i18n.t("episodes_layout.auto_delete_label").to_string();
    let i18n_auto_delete_days_unit = i18n.t("episodes_layout.auto_delete_days_unit").to_string();
    let i18n_auto_delete_description = i18n
        .t("episodes_layout.auto_delete_description")
        .to_string();
    let i18n_auto_delete_updated = i18n.t("episodes_layout.auto_delete_updated").to_string();
    let i18n_error_updating_auto_delete = i18n
        .t("episodes_layout.error_updating_auto_delete")
        .to_string();
    let i18n_auto_delete_reset_default = i18n
        .t("episodes_layout.auto_delete_reset_default")
        .to_string();
    let i18n_error_resetting_auto_delete = i18n
        .t("episodes_layout.error_resetting_auto_delete")
        .to_string();
    let i18n_auto_delete_custom_badge = i18n
        .t("episodes_layout.auto_delete_custom_badge")
        .to_string();
    let i18n_auto_delete_global_badge = i18n
        .t("episodes_layout.auto_delete_global_badge")
        .to_string();
    let i18n_auto_skip_intros_outros = i18n
        .t("episodes_layout.auto_skip_intros_outros")
        .to_string();
    let i18n_start_skip_seconds = i18n.t("episodes_layout.start_skip_seconds").to_string();
    let i18n_end_skip_seconds = i18n.t("episodes_layout.end_skip_seconds").to_string();
    let i18n_youtube_download_limit = i18n.t("episodes_layout.youtube_download_limit").to_string();
    let i18n_youtube_limit_description = i18n
        .t("episodes_layout.youtube_limit_description")
        .to_string();
    let i18n_adjust_podcast_categories = i18n
        .t("episodes_layout.adjust_podcast_categories")
        .to_string();
    let i18n_loading = i18n.t("episodes_layout.loading").to_string();
    let i18n_new_category_placeholder = i18n
        .t("episodes_layout.new_category_placeholder")
        .to_string();
    let i18n_download_all_confirmation = i18n
        .t("episodes_layout.download_all_confirmation")
        .to_string();
    let i18n_yes_download_all = i18n.t("episodes_layout.yes_download_all").to_string();
    let i18n_no_take_me_back = i18n.t("episodes_layout.no_take_me_back").to_string();
    let i18n_delete_podcast_confirmation = i18n
        .t("episodes_layout.delete_podcast_confirmation")
        .to_string();
    let i18n_yes_delete_podcast = i18n.t("episodes_layout.yes_delete_podcast").to_string();
    let i18n_show_only = i18n.t("episodes_layout.show_only").to_string();
    let i18n_showing_only_completed = i18n.t("episodes_layout.showing_only_completed").to_string();
    let i18n_hide = i18n.t("episodes_layout.hide").to_string();
    let i18n_hiding_completed = i18n.t("episodes_layout.hiding_completed").to_string();
    let i18n_all = i18n.t("episodes_layout.all").to_string();
    let i18n_showing_all_episodes = i18n.t("episodes_layout.showing_all_episodes").to_string();
    let i18n_episode_count = i18n.t("episodes_layout.episode_count").to_string();
    let i18n_authors = i18n.t("episodes_layout.authors").to_string();
    let i18n_explicit = i18n.t("episodes_layout.explicit").to_string();
    let i18n_yes = i18n.t("episodes_layout.yes").to_string();
    let i18n_no = i18n.t("episodes_layout.no").to_string();
    let i18n_search_episodes_placeholder = i18n
        .t("episodes_layout.search_episodes_placeholder")
        .to_string();
    let i18n_newest_first = i18n.t("episodes_layout.newest_first").to_string();
    let i18n_oldest_first = i18n.t("episodes_layout.oldest_first").to_string();
    let i18n_shortest_first = i18n.t("episodes_layout.shortest_first").to_string();
    let i18n_longest_first = i18n.t("episodes_layout.longest_first").to_string();
    let i18n_title_az = i18n.t("episodes_layout.title_az").to_string();
    let i18n_title_za = i18n.t("episodes_layout.title_za").to_string();
    let i18n_clear_all = i18n.t("episodes_layout.clear_all").to_string();
    let i18n_in_progress = i18n.t("episodes_layout.in_progress").to_string();
    let i18n_exit_select = i18n.t("episodes_layout.exit_select").to_string();
    let i18n_select = i18n.t("episodes_layout.select").to_string();
    let i18n_deselect_all = i18n.t("episodes_layout.deselect_all").to_string();
    let i18n_select_all = i18n.t("episodes_layout.select_all").to_string();
    let i18n_select_unplayed = i18n.t("episodes_layout.select_unplayed").to_string();
    let i18n_select_in_progress = i18n.t("episodes_layout.select_in_progress").to_string();
    let i18n_mark_complete = i18n.t("episodes_layout.mark_complete").to_string();
    let i18n_queue_episodes = i18n.t("episodes_layout.queue_episodes").to_string();
    let i18n_download_episodes = i18n.t("episodes_layout.download_episodes").to_string();
    let i18n_no_episodes_found = i18n.t("episodes_layout.no_episodes_found").to_string();
    let i18n_no_episodes_description = i18n
        .t("episodes_layout.no_episodes_description")
        .to_string();
    let i18n_youtube_episode_limit_updated = i18n
        .t("episodes_layout.youtube_episode_limit_updated")
        .to_string();
    let i18n_skip_times_adjusted = i18n.t("episodes_layout.skip_times_adjusted").to_string();
    let i18n_error_adjusting_skip_times = i18n
        .t("episodes_layout.error_adjusting_skip_times")
        .to_string();
    let i18n_playback_speed_reset_default = i18n
        .t("episodes_layout.playback_speed_reset_default")
        .to_string();
    let i18n_error_resetting_playback_speed = i18n
        .t("episodes_layout.error_resetting_playback_speed")
        .to_string();
    let i18n_auto_play_next_episode = i18n.t("episodes_layout.auto_play_next_episode").to_string();
    let i18n_auto_queue_new_episodes = i18n.t("episodes_layout.auto_queue_new_episodes").to_string();
    let i18n_favorite_podcast = i18n.t("episodes_layout.favorite_podcast").to_string();
    let i18n_use_podcast_covers = i18n.t("episodes_layout.use_podcast_covers").to_string();
    let i18n_podcast_cover_hint = i18n.t("episodes_layout.podcast_cover_hint").to_string();
    let i18n_merge_podcasts = i18n.t("episodes_layout.merge_podcasts").to_string();
    let i18n_merge_description = i18n.t("episodes_layout.merge_description").to_string();
    let i18n_currently_merged_podcasts = i18n.t("episodes_layout.currently_merged_podcasts").to_string();
    let i18n_unmerge = i18n.t("episodes_layout.unmerge").to_string();
    let i18n_select_podcasts_to_merge = i18n.t("episodes_layout.select_podcasts_to_merge").to_string();
    let i18n_hosts = i18n.t("episodes_layout.hosts").to_string();
    let i18n_video = i18n.t("episodes_layout.video").to_string();
    let loading = use_state(|| true);
    let is_subscribing = use_state(|| false);
    let page_state = use_state(|| PageState::Hidden);
    let episode_search_term = use_state(|| String::new());
    // Debounced view of episode_search_term. Search keystrokes update `episode_search_term`
    // for input responsiveness; a 300 ms timer copies it into `debounced_search_term`, which
    // the backend-reload effect actually watches. Avoids one HTTP request per keystroke.
    let debounced_search_term = use_state(|| String::new());

    // Initialize sort direction - will be updated when podcast_id changes
    let episode_sort_direction = use_state(|| Some(EpisodeSortDirection::NewestFirst));

    let completed_filter_state = use_state(|| CompletedFilter::ShowAll);
    let show_in_progress = use_state(|| false);
    // Set to Some(podcast_id) by the localStorage-load effect after it applies the per-podcast
    // sort/filter prefs. The reload effect gates on this matching the current podcast_id, so we
    // don't fire a backend fetch with stale sort/filter from the previous podcast.
    let prefs_loaded_for_podcast = use_state(|| None::<i32>);
    let _reload_offset = use_state(|| 0i64);
    let loading_more = use_state(|| false);
    let notification_status = use_state(|| false);
    let favorite_status = use_state(|| false);
    let feed_cutoff_days = use_state(|| 0);
    let feed_cutoff_days_input = use_state(|| "0".to_string());
    let auto_delete_days = use_state(|| 0);
    let auto_delete_days_input = use_state(|| "0".to_string());
    let auto_delete_customized = use_state(|| false);
    let playback_speed = use_state(|| 1.0);
    let playback_speed_customized = use_state(|| false);
    let use_podcast_covers = use_state(|| false);
    let playback_speed_input = playback_speed.clone();
    let playback_speed_clone = playback_speed.clone();
    let playback_speed_customized_render = playback_speed_customized.clone();
    let auto_delete_customized_render = auto_delete_customized.clone();
    let rss_key_state = use_state(|| None::<String>);

    // Bulk selection state
    let selected_episodes = use_state(|| HashSet::<i32>::new());
    let is_selecting = use_state(|| false);

    let history = BrowserHistory::new();
    // let node_ref = use_node_ref();
    let user_id = search_state
        .user_details
        .as_ref()
        .map(|ud| ud.UserID.clone());
    let api_key = search_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let server_name = search_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let _podcast_added = podcast_state.podcast_added.unwrap_or_default();
    let pod_url = use_state(|| String::new());
    let new_category = use_state(|| String::new());

    // Edit podcast form state
    let edit_feed_url = use_state(|| String::new());
    let edit_username = use_state(|| String::new());
    let edit_password = use_state(|| String::new());
    let edit_podcast_name = use_state(|| String::new());
    let edit_description = use_state(|| String::new());
    let edit_author = use_state(|| String::new());
    let edit_artwork_url = use_state(|| String::new());
    let edit_website_url = use_state(|| String::new());
    let edit_podcast_index_id = use_state(|| String::new());

    // Merge podcast state
    let selected_podcasts_to_merge = use_state(|| Vec::<i32>::new());
    let available_podcasts_for_merge =
        use_state(|| Vec::<crate::requests::pod_req::Podcast>::new());
    let current_merged_podcasts = use_state(|| Vec::<i32>::new());
    let merged_podcast_details = use_state(|| HashMap::<i32, PodcastDetails>::new());
    let loading_merge_data = use_state(|| false);

    // Pre-populate edit form when modal opens
    {
        let edit_feed_url = edit_feed_url.clone();
        let edit_username = edit_username.clone();
        let edit_password = edit_password.clone();
        let edit_podcast_name = edit_podcast_name.clone();
        let edit_description = edit_description.clone();
        let edit_author = edit_author.clone();
        let edit_artwork_url = edit_artwork_url.clone();
        let edit_website_url = edit_website_url.clone();
        let edit_podcast_index_id = edit_podcast_index_id.clone();
        let clicked_podcast_info = clicked_podcast_info.clone();
        let page_state = page_state.clone();

        use_effect_with(
            (page_state.clone(), clicked_podcast_info.clone()),
            move |(page_state, podcast_info)| {
                if **page_state == PageState::EditPodcast {
                    if let Some(info) = podcast_info {
                        edit_feed_url.set(info.feedurl.clone());
                        edit_username.set(String::new()); // Username not available in current podcast info
                        edit_password.set(String::new()); // Password not available in current podcast info
                        edit_podcast_name.set(info.podcastname.clone());
                        edit_description.set(info.description.clone());
                        edit_author.set(info.author.clone());
                        edit_artwork_url.set(info.artworkurl.clone());
                        edit_website_url.set(info.websiteurl.clone());
                        edit_podcast_index_id.set(info.podcastindexid.to_string());
                    }
                }
            },
        );
    }

    let new_cat_in = new_category.clone();
    let new_category_input = Callback::from(move |e: InputEvent| {
        if let Some(input_element) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
            let value = input_element.value(); // Get the value as a String
            new_cat_in.set(value); // Set the state with the String
        }
    });

    // Add this near the start of the component
    let audio_dispatch = _dispatch.clone();

    // Clear podcast metadata when component mounts
    use_effect_with((), move |_| {
        audio_dispatch.reduce_mut(|state| {
            state.podcast_value4value = None;
            state.podcast_funding = None;
            state.podcast_podroll = None;
            state.podcast_people = None;
        });
        || ()
    });

    {
        let audio_dispatch = _dispatch.clone();

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

    // On mount, check if the podcast is in the database
    let effect_user_id = user_id.clone();
    let effect_api_key = api_key.clone();
    let loading_ep = loading.clone();

    {
        let is_added = is_added.clone();
        let podcast = clicked_podcast_info.clone();
        let user_id = effect_user_id.clone();
        let api_key = effect_api_key.clone();
        let server_name = server_name.clone();
        let click_history = history.clone();
        let pod_load_url = pod_url.clone();
        let pod_loading_ep = loading.clone();

        fn emit_click(callback: Callback<MouseEvent>) {
            callback.emit(MouseEvent::new("click").unwrap());
        }

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |(api_key, user_id, server_name)| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let is_added = is_added.clone();

                    if podcast.is_none() {
                        let window = web_sys::window().expect("no global window exists");
                        let search_params = window.location().search().unwrap();
                        let url_params = UrlSearchParams::new_with_str(&search_params).unwrap();

                        let podcast_title = url_params.get("podcast_title").unwrap_or_default();
                        let podcast_url = url_params.get("podcast_url").unwrap_or_default();
                        let podcast_index_id = 0;
                        if !podcast_title.is_empty() && !podcast_url.is_empty() {
                            let podcast_info = ClickedFeedURL {
                                podcastid: 0,
                                podcastname: podcast_title.clone(),
                                feedurl: podcast_url.clone(),
                                description: String::new(),
                                author: String::new(),
                                artworkurl: String::new(),
                                explicit: false,
                                episodecount: 0,
                                categories: None,
                                websiteurl: String::new(),
                                podcastindexid: podcast_index_id,
                                is_youtube: Some(false),
                            };

                            let api_key = api_key.clone();
                            let user_id = user_id.clone();
                            let server_name = server_name.clone();
                            spawn_local(async move {
                                let added = call_check_podcast(
                                    &server_name,
                                    &api_key.clone().unwrap(),
                                    user_id,
                                    podcast_info.podcastname.as_str(),
                                    podcast_info.feedurl.as_str(),
                                )
                                .await
                                .unwrap_or_default()
                                .exists;
                                is_added.set(added);

                                let podcast_details = call_get_podcast_details_dynamic(
                                    &server_name,
                                    &api_key.clone().unwrap(),
                                    user_id,
                                    podcast_info.podcastname.as_str(),
                                    podcast_info.feedurl.as_str(),
                                    podcast_info.podcastindexid,
                                    added,
                                    Some(false),
                                )
                                .await
                                .unwrap();

                                fn categories_to_string(
                                    categories: Option<HashMap<String, String>>,
                                ) -> Option<String> {
                                    categories.map(|map| {
                                        map.values().cloned().collect::<Vec<String>>().join(", ")
                                    })
                                }
                                let podcast_categories_str =
                                    categories_to_string(podcast_details.details.categories);

                                // Execute the same process as when a podcast is clicked
                                let on_title_click = create_on_title_click(
                                    server_name,
                                    Some(Some(api_key.clone().unwrap())),
                                    &click_history,
                                    podcast_details.details.podcastid,
                                    podcast_details.details.podcastindexid,
                                    podcast_details.details.podcastname,
                                    podcast_details.details.feedurl,
                                    podcast_details.details.description,
                                    podcast_details.details.author,
                                    podcast_details.details.artworkurl,
                                    podcast_details.details.explicit,
                                    podcast_details.details.episodecount,
                                    podcast_categories_str, // assuming no categories in local storage
                                    podcast_details.details.websiteurl,
                                    user_id,
                                    podcast_details.details.is_youtube.unwrap(),
                                );
                                emit_click(on_title_click);
                                let window = web_sys::window().expect("no global window exists");
                                let location = window.location();

                                let mut new_url = location.origin().unwrap();
                                new_url.push_str(&location.pathname().unwrap());
                                new_url.push_str("?podcast_title=");
                                new_url.push_str(&urlencoding::encode(&podcast_info.podcastname));
                                new_url.push_str("&podcast_url=");
                                new_url.push_str(&urlencoding::encode(&podcast_info.feedurl));
                                pod_load_url.set(new_url.clone());
                            });
                        }
                    } else {
                        let podcast = podcast.unwrap();

                        // Update the URL with query parameters
                        let window = web_sys::window().expect("no global window exists");
                        let history = window.history().expect("should have a history");
                        let location = window.location();

                        let mut new_url = location.origin().unwrap();
                        new_url.push_str(&location.pathname().unwrap());
                        new_url.push_str("?podcast_title=");
                        new_url.push_str(&urlencoding::encode(&podcast.podcastname));
                        new_url.push_str("&podcast_url=");
                        new_url.push_str(&urlencoding::encode(&podcast.feedurl));

                        history
                            .replace_state_with_url(
                                &wasm_bindgen::JsValue::NULL,
                                "",
                                Some(&new_url),
                            )
                            .expect("should push state");

                        let api_key = api_key.clone();
                        let user_id = user_id.clone();
                        let server_name = server_name.clone();
                        spawn_local(async move {
                            let added = call_check_podcast(
                                &server_name,
                                &api_key.unwrap(),
                                user_id,
                                podcast.podcastname.as_str(),
                                podcast.feedurl.as_str(),
                            )
                            .await
                            .unwrap_or_default()
                            .exists;
                            is_added.set(added);
                            if *is_added.clone() != true {
                                pod_loading_ep.set(false);
                            }
                        });
                    }
                }
                || ()
            },
        );
    }

    let podcast_info = podcast_state.clicked_podcast_info.clone();
    let load_link = loading.clone();

    use_effect_with(podcast_info.clone(), {
        let pod_url = pod_url.clone();
        move |podcast_info| {
            if let Some(info) = podcast_info {
                let window = window().expect("no global window exists");
                let history = window.history().expect("should have a history");
                let location = window.location();

                let mut new_url = location.origin().unwrap();
                new_url.push_str(&location.pathname().unwrap());
                new_url.push_str("?podcast_title=");
                new_url.push_str(&urlencoding::encode(&info.podcastname));
                new_url.push_str("&podcast_url=");
                new_url.push_str(&urlencoding::encode(&info.feedurl));
                pod_url.set(new_url.clone());
                load_link.set(false);

                history
                    .replace_state_with_url(&JsValue::NULL, "", Some(&new_url))
                    .expect("should push state");
            }
            || {}
        }
    });

    let download_status = use_state(|| false);
    let auto_play_next_status = use_state(|| false);
    let auto_queue_status = use_state(|| false);
    let podcast_id = use_state(|| 0);
    let start_skip = use_state(|| 0);
    let end_skip = use_state(|| 0);
    // Silence-trim (#727) per-podcast settings
    let trim_silence = use_state(|| false);
    let silence_threshold = use_state(|| 2i32);
    // Transcription (#726): per-podcast auto-transcribe + whether the AI sidecar is up
    let auto_transcribe = use_state(|| false);
    let ai_available = use_state(|| false);
    // Ad detection (#790): per-podcast auto-detect + skip immediately vs. confirm-first
    let auto_ad_detect = use_state(|| false);
    let ad_skip_auto_activate = use_state(|| true);

    // Load merge-related data when edit modal opens
    {
        let available_podcasts_for_merge = available_podcasts_for_merge.clone();
        let current_merged_podcasts = current_merged_podcasts.clone();
        let merged_podcast_details = merged_podcast_details.clone();
        let loading_merge_data = loading_merge_data.clone();
        let page_state = page_state.clone();
        let clicked_podcast_info = clicked_podcast_info.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let podcast_id = podcast_id.clone();

        use_effect_with(
            (page_state.clone(), clicked_podcast_info.clone()),
            move |(page_state, podcast_info)| {
                if **page_state == PageState::EditPodcast {
                    if let (Some(api_key), Some(server_name), Some(user_id), Some(_podcast_info)) = (
                        api_key.as_ref(),
                        server_name.as_ref(),
                        user_id.as_ref(),
                        podcast_info.as_ref(),
                    ) {
                        loading_merge_data.set(true);

                        // Load available podcasts
                        let available_podcasts_for_merge = available_podcasts_for_merge.clone();
                        let current_merged_podcasts = current_merged_podcasts.clone();
                        let merged_podcast_details = merged_podcast_details.clone();
                        let loading_merge_data = loading_merge_data.clone();
                        let api_key = api_key.clone();
                        let server_name = server_name.clone();
                        let user_id = *user_id;
                        let current_podcast_id = *podcast_id;

                        spawn_local(async move {
                            // Load all available podcasts (keep all for name lookups)
                            match call_get_podcasts(&server_name, &api_key, &user_id).await {
                                Ok(podcasts) => {
                                    available_podcasts_for_merge.set(podcasts);
                                }
                                Err(e) => {
                                    console::log_1(
                                        &format!("Error loading podcasts for merge: {}", e).into(),
                                    );
                                }
                            }

                            // Load current merged podcasts
                            match call_get_merged_podcasts(
                                &server_name,
                                &api_key,
                                current_podcast_id,
                            )
                            .await
                            {
                                Ok(merged_ids) => {
                                    current_merged_podcasts.set(merged_ids.clone());

                                    // Fetch details for each merged podcast
                                    let mut details_map = HashMap::new();
                                    for &merged_id in &merged_ids {
                                        match call_get_podcast_details(
                                            &server_name,
                                            &api_key.as_ref().unwrap(),
                                            user_id,
                                            merged_id,
                                        )
                                        .await
                                        {
                                            Ok(details) => {
                                                details_map.insert(merged_id, details);
                                            }
                                            Err(e) => {
                                                console::log_1(
                                                    &format!(
                                                        "Error loading details for merged podcast {}: {}",
                                                        merged_id, e
                                                    )
                                                    .into(),
                                                );
                                            }
                                        }
                                    }
                                    merged_podcast_details.set(details_map);
                                }
                                Err(e) => {
                                    console::log_1(
                                        &format!("Error loading merged podcasts: {}", e).into(),
                                    );
                                    current_merged_podcasts.set(Vec::new());
                                }
                            }

                            loading_merge_data.set(false);
                        });
                    }
                }
            },
        );
    }

    // Tracks how many episodes have been synced into EpisodeStatusState so appends only
    // process new episodes instead of rebuilding the entire completed/saved/queued sets.
    let synced_episode_count = use_state(|| 0usize);

    // Update sort direction when podcast_id changes to load per-podcast preferences
    {
        let episode_sort_direction = episode_sort_direction.clone();
        let completed_filter_state = completed_filter_state.clone();
        let synced_episode_count = synced_episode_count.clone();
        let prefs_loaded_for_podcast = prefs_loaded_for_podcast.clone();
        let podcast_id_clone = podcast_id.clone();
        use_effect_with(podcast_id_clone, move |podcast_id| {
            if **podcast_id > 0 {
                let preference_key = format!("podcast_{}", **podcast_id);
                let saved_preference = get_filter_preference(&preference_key);
                let new_direction = match saved_preference.as_deref() {
                    Some("newest") => Some(EpisodeSortDirection::NewestFirst),
                    Some("oldest") => Some(EpisodeSortDirection::OldestFirst),
                    Some("shortest") => Some(EpisodeSortDirection::ShortestFirst),
                    Some("longest") => Some(EpisodeSortDirection::LongestFirst),
                    Some("title_az") => Some(EpisodeSortDirection::TitleAZ),
                    Some("title_za") => Some(EpisodeSortDirection::TitleZA),
                    _ => Some(EpisodeSortDirection::NewestFirst), // Default to newest first
                };
                episode_sort_direction.set(new_direction);

                let completed_key = format!("podcast_{}_completed_filter", **podcast_id);
                let saved_completed = get_filter_preference(&completed_key);
                let new_completed_filter = match saved_completed.as_deref() {
                    Some("show_only") => CompletedFilter::ShowOnly,
                    Some("hide") => CompletedFilter::Hide,
                    _ => CompletedFilter::ShowAll,
                };
                completed_filter_state.set(new_completed_filter);
                synced_episode_count.set(0);
                // Signal the backend-reload effect that prefs for this podcast are loaded so
                // it can fetch with the right sort/filter. Without this gate, the reload effect
                // would fire once with the previous podcast's sort and then again after the
                // pref load — two requests, one with stale params.
                prefs_loaded_for_podcast.set(Some(**podcast_id));
            }
            || ()
        });
    }

    {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let download_status = download_status.clone();
        let auto_play_next_status = auto_play_next_status.clone();
        let auto_queue_status = auto_queue_status.clone();
        let notification_effect = notification_status.clone();
        let favorite_effect = favorite_status.clone();
        // let episode_name = episode_name_pre.clone();
        // let episode_url = episode_url_pre.clone();
        let user_id = search_state.user_details.as_ref().map(|ud| ud.UserID);
        let effect_start_skip = start_skip.clone();
        let effect_end_skip = end_skip.clone();
        let effect_playback_speed = playback_speed.clone();
        let effect_playback_speed_customized = playback_speed_customized.clone();
        let effect_added = is_added.clone();
        let feed_cutoff_days = feed_cutoff_days.clone();
        let feed_cutoff_days_input = feed_cutoff_days_input.clone();
        let effect_auto_delete_days = auto_delete_days.clone();
        let effect_auto_delete_days_input = auto_delete_days_input.clone();
        let effect_auto_delete_customized = auto_delete_customized.clone();
        let audio_dispatch = _dispatch.clone();
        let click_feed_results = search_data.podcast_feed_results.clone();
        let clicked_podcast_info_effect = podcast_state.clicked_podcast_info.clone();

        use_effect_with(
            (*podcast_id, *effect_added),
            move |_| {
                let episode_name = click_feed_results
                    .as_ref()
                    .and_then(|r| r.episodes.get(0))
                    .and_then(|ep| Some(ep.episodetitle.clone()))
                    .unwrap_or_default();

                let episode_url = click_feed_results
                    .as_ref()
                    .and_then(|results| results.episodes.get(0))
                    .and_then(|episode| Some(episode.episodeurl.clone()))
                    .unwrap_or_default();

                let bool_true = *effect_added; // Dereference here

                if !bool_true {
                } else {
                    let api_key = api_key.clone();
                    let server_name = server_name.clone();
                    let podcast_id = podcast_id.clone();
                    let download_status = download_status.clone();
                    let auto_play_next_status = auto_play_next_status.clone();
                    let auto_queue_status = auto_queue_status.clone();
                    let episode_name = episode_name;
                    let episode_url = episode_url;
                    let user_id = user_id.unwrap();

                    // Use podcast_id from clicked_podcast_info when available so we can skip
                    // the call_get_podcast_id_from_ep_name round-trip for subscribed podcasts.
                    let known_podcast_id = clicked_podcast_info_effect
                        .as_ref()
                        .map(|info| info.podcastid)
                        .filter(|&id| id > 0);

                    // Proceed if we have a known podcast id OR a valid first-episode URL to look it up
                    if known_podcast_id.is_some()
                        || (!episode_name.is_empty() && !episode_url.is_empty())
                    {
                        wasm_bindgen_futures::spawn_local(async move {
                            if let (Some(api_key), Some(server_name)) =
                                (api_key.as_ref(), server_name.as_ref())
                            {
                                // Resolve the podcast_id: either already known or look it up.
                                let id = if let Some(id) = known_podcast_id {
                                    podcast_id.set(id);
                                    id
                                } else {
                                    match call_get_podcast_id_from_ep_name(
                                        &server_name,
                                        &api_key,
                                        episode_name,
                                        episode_url,
                                        user_id,
                                    )
                                    .await
                                    {
                                        Ok(id) => {
                                            podcast_id.set(id);
                                            id
                                        }
                                        Err(e) => {
                                            web_sys::console::log_1(
                                                &format!(
                                                    "Error getting podcast id from ep name: {}",
                                                    e
                                                )
                                                .into(),
                                            );
                                            return;
                                        }
                                    }
                                };
                                {
                                        // Unlock page immediately — episodes already in SearchState.
                                        loading_ep.set(false);
                                        // Fetch all podcast settings in parallel.
                                        let key_opt = Some(api_key.clone().unwrap());
                                        let key_str = api_key.clone().unwrap();
                                        let server = server_name.clone();
                                        let (auto_dl_result, auto_play_result, auto_queue_result, cutoff_result, notif_result, fav_result, play_details_result, auto_delete_result) = futures::join!(
                                            call_get_auto_download_status(
                                                &server,
                                                user_id,
                                                &key_opt,
                                                id,
                                            ),
                                            call_get_auto_play_next_status(
                                                &server,
                                                user_id,
                                                &key_opt,
                                                id,
                                            ),
                                            call_get_auto_queue_status(
                                                &server,
                                                user_id,
                                                &key_opt,
                                                id,
                                            ),
                                            call_get_feed_cutoff_days(
                                                &server,
                                                &key_opt,
                                                id,
                                                user_id,
                                            ),
                                            call_get_podcast_notifications_status(
                                                server.clone(),
                                                key_str.clone(),
                                                user_id,
                                                id,
                                            ),
                                            call_get_podcast_favorite_status(
                                                server.clone(),
                                                key_str.clone(),
                                                user_id,
                                                id,
                                            ),
                                            call_get_play_episode_details(
                                                &server,
                                                &key_opt,
                                                user_id,
                                                id,
                                                false,
                                            ),
                                            call_get_podcast_auto_delete_days(
                                                &server,
                                                &key_opt,
                                                id,
                                                user_id,
                                            ),
                                        );
                                        match auto_dl_result {
                                            Ok(status) => { download_status.set(status); }
                                            Err(e) => { web_sys::console::log_1(&format!("Error getting auto-download status: {}", e).into()); }
                                        }
                                        match auto_play_result {
                                            Ok(status) => { auto_play_next_status.set(status); }
                                            Err(e) => { web_sys::console::log_1(&format!("Error getting auto-play-next status: {}", e).into()); }
                                        }
                                        match auto_queue_result {
                                            Ok(status) => { auto_queue_status.set(status); }
                                            Err(e) => { web_sys::console::log_1(&format!("Error getting auto-queue status: {}", e).into()); }
                                        }
                                        match cutoff_result {
                                            Ok(days) => {
                                                feed_cutoff_days.set(days);
                                                feed_cutoff_days_input.set(days.to_string());
                                            }
                                            Err(e) => { web_sys::console::log_1(&format!("Error getting feed cutoff days: {}", e).into()); }
                                        }
                                        match notif_result {
                                            Ok(status) => { notification_effect.set(status); }
                                            Err(e) => { web_sys::console::log_1(&format!("Error getting notification status: {}", e).into()); }
                                        }
                                        match fav_result {
                                            Ok(status) => { favorite_effect.set(status); }
                                            Err(e) => { web_sys::console::log_1(&format!("Error getting favorite status: {}", e).into()); }
                                        }
                                        match play_details_result {
                                            Ok((speed, start, end, customized)) => {
                                                effect_start_skip.set(start);
                                                effect_end_skip.set(end);
                                                effect_playback_speed.set(speed as f64);
                                                effect_playback_speed_customized.set(customized);
                                            }
                                            Err(e) => { web_sys::console::log_1(&format!("Error getting auto-skip times: {}", e).into()); }
                                        }
                                        match auto_delete_result {
                                            Ok((days, customized)) => {
                                                effect_auto_delete_days.set(days);
                                                effect_auto_delete_days_input.set(days.to_string());
                                                effect_auto_delete_customized.set(customized);
                                            }
                                            Err(e) => { web_sys::console::log_1(&format!("Error getting auto-delete days: {}", e).into()); }
                                        }
                                        let chap_request = FetchPodcasting2PodDataRequest {
                                            podcast_id: id,
                                            user_id,
                                        };
                                        match call_fetch_podcasting_2_pod_data(
                                            &server_name,
                                            &api_key,
                                            &chap_request,
                                        )
                                        .await
                                        {
                                            Ok(response) => {
                                                // let chapters = response.chapters.clone(); // Clone chapters to avoid move issue
                                                let value = response.value.clone();
                                                let funding = response.funding.clone();
                                                let podroll = response.podroll.clone();
                                                let people = response.people.clone();
                                                audio_dispatch.reduce_mut(|state| {
                                                    state.podcast_value4value = Some(value);
                                                    state.podcast_funding = Some(funding);
                                                    state.podcast_podroll = Some(podroll);
                                                    state.podcast_people = Some(people);
                                                });
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(
                                                    &format!("Error fetching 2.0 data: {}", e)
                                                        .into(),
                                                );
                                            }
                                        }
                                    }
                            }
                        });
                    }
                }
                || ()
            },
        );
    }

    // Load podcast cover preference when podcast_id changes
    {
        let use_podcast_covers = use_podcast_covers.clone();
        let podcast_id = podcast_id.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();

        use_effect_with(podcast_id.clone(), move |podcast_id| {
            if **podcast_id > 0 {
                let use_podcast_covers = use_podcast_covers.clone();
                let podcast_id_val = **podcast_id;

                if let (Some(api_key), Some(server_name), Some(user_id)) = (
                    api_key.as_ref().and_then(|k| k.clone()),
                    server_name.as_ref().map(|s| s.clone()),
                    user_id.as_ref().cloned(),
                ) {
                    wasm_bindgen_futures::spawn_local(async move {
                        match call_get_podcast_cover_preference(
                            &server_name,
                            &api_key,
                            user_id,
                            Some(podcast_id_val),
                        )
                        .await
                        {
                            Ok(current_preference) => {
                                use_podcast_covers.set(current_preference);
                            }
                            Err(_) => {
                                // If API call fails, default to false
                                use_podcast_covers.set(false);
                            }
                        }
                    });
                }
            }
            || ()
        });
    }

    let open_in_new_tab = Callback::from(move |url: String| {
        let window = web_sys::window().unwrap();
        window.open_with_url_and_target(&url, "_blank").unwrap();
    });

    // Function to handle link clicks
    let history_handle = history.clone();
    let handle_click = Callback::from(move |event: MouseEvent| {
        if let Some(target) = event.target_dyn_into::<web_sys::HtmlElement>() {
            if let Some(href) = target.get_attribute("href") {
                event.prevent_default();
                if href.starts_with("http") {
                    // External link, open in a new tab
                    web_sys::window()
                        .unwrap()
                        .open_with_url_and_target(&href, "_blank")
                        .unwrap();
                } else {
                    // Internal link, use Yew Router to navigate
                    history_handle.push(href);
                }
            }
        }
    });

    let node_ref = use_node_ref();

    use_effect_with((), move |_| {
        if let Some(container) = node_ref.cast::<web_sys::HtmlElement>() {
            if let Ok(links) = container.query_selector_all("a") {
                for i in 0..links.length() {
                    if let Some(link) = links.item(i) {
                        let link = link.dyn_into::<web_sys::HtmlElement>().unwrap();
                        let handle_click_clone = handle_click.clone();
                        let listener =
                            gloo_events::EventListener::new(&link, "click", move |event| {
                                handle_click_clone
                                    .emit(event.clone().dyn_into::<web_sys::MouseEvent>().unwrap());
                            });
                        listener.forget(); // Prevent listener from being dropped
                    }
                }
            }
        }

        || ()
    });

    let delete_history = history.clone();
    let delete_all_click = {
        let add_dispatch = _search_dispatch.clone();
        let pod_values = clicked_podcast_info.clone();

        let user_id_og = user_id.clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let app_dispatch = _search_dispatch.clone();
        let call_is_added = is_added.clone();
        let page_state = page_state.clone();
        let i18n_youtube_channel_successfully_removed =
            i18n_youtube_channel_successfully_removed.clone();
        let i18n_podcast_successfully_removed = i18n_podcast_successfully_removed.clone();
        let i18n_failed_to_remove_youtube_channel = i18n_failed_to_remove_youtube_channel.clone();
        let i18n_failed_to_remove_podcast = i18n_failed_to_remove_podcast.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let i18n_youtube_channel_successfully_removed =
                i18n_youtube_channel_successfully_removed.clone();
            let i18n_podcast_successfully_removed = i18n_podcast_successfully_removed.clone();
            let i18n_failed_to_remove_youtube_channel =
                i18n_failed_to_remove_youtube_channel.clone();
            let i18n_failed_to_remove_podcast = i18n_failed_to_remove_podcast.clone();
            let hist = delete_history.clone();
            let page_state = page_state.clone();
            let pod_title_og = pod_values.clone().unwrap().podcastname.clone();
            let pod_feed_url_og = pod_values.clone().unwrap().feedurl.clone();
            let is_youtube = pod_values.clone().unwrap().is_youtube.unwrap_or(false);
            Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(true));
            let is_added_inner = call_is_added.clone();
            let call_dispatch = add_dispatch.clone();
            let pod_title = pod_title_og.clone();
            let pod_title_yt = pod_title_og.clone();
            let pod_feed_url = pod_feed_url_og.clone();
            let pod_feed_url_yt = pod_feed_url_og.clone();
            let pod_feed_url_check = pod_feed_url_og.clone();
            let user_id = user_id_og.clone().unwrap();
            let podcast_values = RemovePodcastValuesName {
                podcast_name: pod_title,
                podcast_url: pod_feed_url,
                user_id,
            };
            let remove_channel = RemoveYouTubeChannelValues {
                user_id,
                channel_name: pod_title_yt,
                channel_url: pod_feed_url_yt,
            };
            let api_key_call = api_key_clone.clone();
            let server_name_call = server_name_clone.clone();
            let _app_dispatch = app_dispatch.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let _dispatch_wasm = call_dispatch.clone();
                let api_key_wasm = api_key_call.clone().unwrap();
                let server_name_wasm = server_name_call.clone();

                let result = if pod_feed_url_check.starts_with("https://www.youtube.com") {
                    call_remove_youtube_channel(
                        &server_name_wasm.unwrap(),
                        &api_key_wasm,
                        &remove_channel,
                    )
                    .await
                } else {
                    call_remove_podcasts_name(
                        &server_name_wasm.unwrap(),
                        &api_key_wasm,
                        &podcast_values,
                    )
                    .await
                };

                match result {
                    Ok(success) => {
                        if success {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Some(
                                    if pod_feed_url_check.starts_with("https://www.youtube.com") {
                                        i18n_youtube_channel_successfully_removed
                                    } else {
                                        i18n_podcast_successfully_removed
                                    },
                                )
                            });
                            Dispatch::<PodcastFeedState>::global().reduce_mut(|state| {
                                state.podcast_added = Some(false);
                            });
                            Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                                state.is_loading = Some(false);
                            });
                            is_added_inner.set(false);

                            if pod_feed_url_check.starts_with("https://www.youtube.com") {
                                hist.push("/podcasts");
                            }
                        } else {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(if is_youtube {
                                    i18n_failed_to_remove_youtube_channel
                                } else {
                                    i18n_failed_to_remove_podcast
                                })
                            });
                            Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(false));
                        }
                        page_state.set(PageState::Hidden);
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message =
                                Some(format!("Error removing content: {:?}", formatted_error))
                        });
                        Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(false));
                    }
                }
            });
        })
    };

    let download_server_name = server_name.clone();
    let download_api_key = api_key.clone();
    let download_dispatch = _search_dispatch.clone();
    let download_feed_results = search_data.podcast_feed_results.clone();

    let download_all_click = {
        let call_dispatch = download_dispatch.clone();
        let server_name_copy = download_server_name.clone();
        let api_key_copy = download_api_key.clone();
        let user_id_copy = user_id.clone();
        let feed_results_copy = download_feed_results.clone();
        let page_state_copy = page_state.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let server_name = server_name_copy.clone();
            let api_key = api_key_copy.clone();
            let feed_results = feed_results_copy.clone();
            let _call_down_dispatch = call_dispatch.clone();
            let page_state = page_state_copy.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let episode_id = match feed_results
                    .as_ref()
                    .and_then(|results| results.episodes.get(0))
                    .and_then(|episode| Some(episode.episodeid))
                {
                    Some(id) => id,
                    None => {
                        eprintln!("No episode_id found");
                        return;
                    }
                };
                let is_youtube = match feed_results
                    .as_ref()
                    .and_then(|results| results.episodes.get(0))
                    .and_then(|episode| Some(episode.is_youtube))
                {
                    Some(id) => id,
                    None => {
                        eprintln!("No is_youtube info found");
                        return;
                    }
                };
                let ep_api_key = api_key.clone();
                let ep_server_name = server_name.clone();
                let ep_user_id = user_id_copy.clone();
                match call_get_podcast_id_from_ep(
                    &ep_server_name.unwrap(),
                    &ep_api_key.unwrap(),
                    episode_id,
                    ep_user_id.unwrap(),
                    Some(is_youtube),
                )
                .await
                {
                    Ok(podcast_id) => {
                        let request = DownloadAllPodcastRequest {
                            podcast_id,
                            user_id: user_id_copy.unwrap(),
                        };

                        match call_download_all_podcast(
                            &server_name.unwrap(),
                            &api_key.flatten(),
                            &request,
                        )
                        .await
                        {
                            Ok(success_message) => {
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.info_message =
                                        Option::from(format!("{}", success_message))
                                });
                                page_state.set(PageState::Hidden);
                            }
                            Err(e) => {
                                let formatted_error = format_error_message(&e.to_string());
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message =
                                        Option::from(format!("{}", formatted_error))
                                });
                                page_state.set(PageState::Hidden);
                            }
                        }
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Option::from(format!(
                                "Failed to get podcast ID: {}",
                                formatted_error
                            ))
                        });
                        page_state.set(PageState::Hidden);
                    }
                }
            });
        })
    };

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
        Download,
        Delete,
        RSSFeed,
        EditPodcast,
    }

    let button_content = if *is_subscribing {
        html! { <i class="ph ph-spinner animate-spin text-2xl"></i> }
    } else if *is_added {
        trash_icon()
    } else {
        add_icon()
    };

    let setting_content = if *is_added {
        settings_icon()
    } else {
        no_icon()
    };
    let download_all = if *is_added {
        download_icon()
    } else {
        no_icon()
    };

    let payment_icon = { payments_icon() };
    let rss_icon = { rss_icon() };

    let website_icon = { website_icon() };

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

    let toggle_edit_podcast = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::EditPodcast);
        })
    };

    let toggle_download = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let download_status = download_status.clone();
        let podcast_id = podcast_id.clone();
        let user_id = user_id.clone();

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let download_status = download_status.clone();
            let auto_download = !*download_status;
            let pod_id_deref = *podcast_id.clone();
            let user_id = user_id.clone().unwrap();

            let request_data = AutoDownloadRequest {
                podcast_id: pod_id_deref, // Replace with the actual podcast ID
                user_id,
                auto_download,
            };

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    match call_enable_auto_download(
                        &server_name,
                        &api_key.clone().unwrap(),
                        &request_data,
                    )
                    .await
                    {
                        Ok(_) => {
                            download_status.set(auto_download);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error enabling/disabling downloads: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    let toggle_auto_play_next = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let auto_play_next_status = auto_play_next_status.clone();
        let podcast_id = podcast_id.clone();
        let user_id = user_id.clone();

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let auto_play_next_status = auto_play_next_status.clone();
            let auto_play_next = !*auto_play_next_status;
            let pod_id_deref = *podcast_id.clone();
            let user_id = user_id.clone().unwrap();

            let request_data = AutoPlayNextRequest {
                podcast_id: pod_id_deref,
                user_id,
                auto_play_next,
            };

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    match call_enable_auto_play_next(
                        &server_name,
                        &api_key.clone().unwrap(),
                        &request_data,
                    )
                    .await
                    {
                        Ok(_) => {
                            auto_play_next_status.set(auto_play_next);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error enabling/disabling auto-play-next: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    let toggle_auto_queue = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let auto_queue_status = auto_queue_status.clone();
        let podcast_id = podcast_id.clone();
        let user_id = user_id.clone();

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let auto_queue_status = auto_queue_status.clone();
            let auto_queue = !*auto_queue_status;
            let pod_id_deref = *podcast_id.clone();
            let user_id = user_id.clone().unwrap();

            let request_data = AutoQueueRequest {
                podcast_id: pod_id_deref,
                user_id,
                auto_queue,
            };

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    match call_enable_auto_queue(
                        &server_name,
                        &api_key.clone().unwrap(),
                        &request_data,
                    )
                    .await
                    {
                        Ok(_) => {
                            auto_queue_status.set(auto_queue);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error enabling/disabling auto-queue: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    let playback_speed_input_handler = Callback::from(move |e: InputEvent| {
        if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
            let value = input.value().parse::<f64>().unwrap_or(1.0);
            // Constrain to reasonable values (0.5 to 3.0)
            let value = value.max(0.5).min(2.0);
            playback_speed_input.set(value);
        }
    });

    // Create the save playback speed function
    let save_playback_speed = {
        let playback_speed = playback_speed.clone();
        let playback_speed_customized = playback_speed_customized.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let dispatch = _search_dispatch.clone();
        let i18n_playback_speed_updated = i18n_playback_speed_updated.clone();
        let i18n_error_updating_playback_speed = i18n_error_updating_playback_speed.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let i18n_playback_speed_updated = i18n_playback_speed_updated.clone();
            let i18n_error_updating_playback_speed = i18n_error_updating_playback_speed.clone();
            let _call_dispatch = dispatch.clone();
            let speed = *playback_speed;
            let playback_speed_customized = playback_speed_customized.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap();
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    let request = PlaybackSpeedRequest {
                        podcast_id,
                        user_id,
                        playback_speed: speed,
                    };

                    match call_set_playback_speed(&server_name, &api_key, &request).await {
                        Ok(_) => {
                            playback_speed_customized.set(true);
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Option::from(i18n_playback_speed_updated)
                            });
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error updating playback speed: {}", e).into(),
                            );
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Option::from(i18n_error_updating_playback_speed)
                            });
                        }
                    }
                }
            });
        })
    };

    // Create the clear playback speed function
    let clear_playback_speed = {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let dispatch = _search_dispatch.clone();
        let playback_speed = playback_speed.clone();
        let playback_speed_customized = playback_speed_customized.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let i18n_playback_speed_reset_default = i18n_playback_speed_reset_default.clone();
            let i18n_error_resetting_playback_speed = i18n_error_resetting_playback_speed.clone();
            let _call_dispatch = dispatch.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap();
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;
            let playback_speed = playback_speed.clone();
            let playback_speed_customized = playback_speed_customized.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    let request = ClearPlaybackSpeedRequest {
                        podcast_id,
                        user_id,
                    };
                    match call_clear_playback_speed(&server_name, &api_key, &request).await {
                        Ok(_) => {
                            playback_speed_customized.set(false);
                            // Re-fetch so the displayed value reflects the global default now in effect
                            if let Ok((speed, _start, _end, customized)) = call_get_play_episode_details(
                                &server_name,
                                api_key,
                                user_id,
                                podcast_id,
                                false,
                            )
                            .await
                            {
                                playback_speed.set(speed as f64);
                                playback_speed_customized.set(customized);
                            }
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Option::from(i18n_playback_speed_reset_default)
                            });
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error resetting playback speed: {}", e).into(),
                            );
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Option::from(i18n_error_resetting_playback_speed)
                            });
                        }
                    }
                }
            });
        })
    };

    // --- Per-podcast auto-delete-downloads override (#655) ---
    let auto_delete_days_input_handler = {
        let auto_delete_days_input = auto_delete_days_input.clone();
        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                auto_delete_days_input.set(input.value());
            }
        })
    };

    let save_auto_delete_days = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let podcast_id = podcast_id.clone();
        let auto_delete_days_input = auto_delete_days_input.clone();
        let auto_delete_days = auto_delete_days.clone();
        let auto_delete_customized = auto_delete_customized.clone();
        let user_id = user_id.clone();
        let i18n_auto_delete_updated = i18n_auto_delete_updated.clone();
        let i18n_error_updating_auto_delete = i18n_error_updating_auto_delete.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let i18n_auto_delete_updated = i18n_auto_delete_updated.clone();
            let i18n_error_updating_auto_delete = i18n_error_updating_auto_delete.clone();

            if let (Some(server_val), Some(key_val), Some(user_val)) = (
                server_name.as_ref(),
                api_key.as_ref().and_then(|k| k.as_ref()),
                user_id,
            ) {
                let pod_id = *podcast_id;
                let days = (*auto_delete_days_input).parse::<i32>().unwrap_or(0).max(0);
                let request_data = SetAutoDeleteDaysRequest {
                    podcast_id: pod_id,
                    user_id: user_val,
                    days,
                };
                let server_val = server_val.clone();
                let key_val = key_val.clone();
                let auto_delete_days = auto_delete_days.clone();
                let auto_delete_customized = auto_delete_customized.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_set_podcast_auto_delete_days(&server_val, &Some(key_val), &request_data).await {
                        Ok(_) => {
                            auto_delete_days.set(days);
                            auto_delete_customized.set(true);
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Option::from(i18n_auto_delete_updated)
                            });
                        }
                        Err(err) => {
                            web_sys::console::log_1(&format!("Error updating auto-delete days: {}", err).into());
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Option::from(i18n_error_updating_auto_delete)
                            });
                        }
                    }
                });
            }
        })
    };

    let clear_auto_delete_days = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let podcast_id = podcast_id.clone();
        let auto_delete_days = auto_delete_days.clone();
        let auto_delete_days_input = auto_delete_days_input.clone();
        let auto_delete_customized = auto_delete_customized.clone();
        let user_id = user_id.clone();
        let i18n_auto_delete_reset_default = i18n_auto_delete_reset_default.clone();
        let i18n_error_resetting_auto_delete = i18n_error_resetting_auto_delete.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let i18n_auto_delete_reset_default = i18n_auto_delete_reset_default.clone();
            let i18n_error_resetting_auto_delete = i18n_error_resetting_auto_delete.clone();

            if let (Some(server_val), Some(key_val), Some(user_val)) = (
                server_name.as_ref(),
                api_key.as_ref().and_then(|k| k.as_ref()),
                user_id,
            ) {
                let pod_id = *podcast_id;
                let request_data = ClearAutoDeleteDaysRequest {
                    podcast_id: pod_id,
                    user_id: user_val,
                };
                let server_val = server_val.clone();
                let key_val = key_val.clone();
                let auto_delete_days = auto_delete_days.clone();
                let auto_delete_days_input = auto_delete_days_input.clone();
                let auto_delete_customized = auto_delete_customized.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_clear_podcast_auto_delete_days(&server_val, &Some(key_val.clone()), &request_data).await {
                        Ok(_) => {
                            auto_delete_customized.set(false);
                            // Re-fetch so the displayed value reflects the global default now in effect
                            if let Ok((days, customized)) = call_get_podcast_auto_delete_days(
                                &server_val,
                                &Some(key_val),
                                pod_id,
                                user_val,
                            )
                            .await
                            {
                                auto_delete_days.set(days);
                                auto_delete_days_input.set(days.to_string());
                                auto_delete_customized.set(customized);
                            }
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Option::from(i18n_auto_delete_reset_default)
                            });
                        }
                        Err(err) => {
                            web_sys::console::log_1(&format!("Error resetting auto-delete days: {}", err).into());
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Option::from(i18n_error_resetting_auto_delete)
                            });
                        }
                    }
                });
            }
        })
    };

    // Add this callback for handling input changes
    let feed_cutoff_days_input_handler = {
        let feed_cutoff_days_input = feed_cutoff_days_input.clone();

        Callback::from(move |e: InputEvent| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                feed_cutoff_days_input.set(input.value());
            }
        })
    };

    // Add this callback for saving the feed cutoff days
    let save_feed_cutoff_days = {
        let dispatch_vid = _search_dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let podcast_id = podcast_id.clone();
        let feed_cutoff_days_input = feed_cutoff_days_input.clone();
        let feed_cutoff_days = feed_cutoff_days.clone();
        let user_id = search_state.user_details.as_ref().map(|ud| ud.UserID);

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let i18n_youtube_episode_limit_updated = i18n_youtube_episode_limit_updated.clone();
            let _dispatch_wasm = dispatch_vid.clone();

            // Extract the values directly without creating intermediate variables
            if let (Some(server_val), Some(key_val), Some(user_val)) = (
                server_name.as_ref(),
                api_key.as_ref().and_then(|k| k.as_ref()),
                user_id,
            ) {
                let pod_id = *podcast_id;
                let days_str = (*feed_cutoff_days_input).clone();
                let days = days_str.parse::<i32>().unwrap_or(0);
                let request_data = UpdateFeedCutoffDaysRequest {
                    podcast_id: pod_id,
                    user_id: user_val,
                    feed_cutoff_days: days,
                };

                // Clone everything needed for the async block
                let server_val = server_val.clone();
                let key_val = key_val.clone();
                let feed_cutoff_days = feed_cutoff_days.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match call_update_feed_cutoff_days(&server_val, &Some(key_val), &request_data)
                        .await
                    {
                        Ok(_) => {
                            feed_cutoff_days.set(days);
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message =
                                    Option::from(i18n_youtube_episode_limit_updated)
                            });
                            // No need to update a ClickedFeedURL or PodcastInfo struct
                            // Just update the state
                        }
                        Err(err) => {
                            web_sys::console::log_1(
                                &format!("Error updating feed cutoff days: {}", err).into(),
                            );
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Option::from(format!(
                                    "Error updating feed cutoff days: {:?}",
                                    err
                                ))
                            });
                        }
                    }
                });
            }
        })
    };

    let toggle_notifications = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let notification_status = notification_status.clone();
        let podcast_id = podcast_id.clone();
        let user_id = user_id.clone();
        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let notification_status = notification_status.clone();
            let enabled = !*notification_status;
            let pod_id_deref = *podcast_id.clone();
            let user_id = user_id.clone().unwrap();

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    match call_toggle_podcast_notifications(
                        server_name.clone(),
                        api_key.clone().unwrap(),
                        user_id,
                        pod_id_deref,
                        enabled,
                    )
                    .await
                    {
                        Ok(_) => {
                            notification_status.set(enabled);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error toggling notifications: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    let toggle_favorite = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let favorite_status = favorite_status.clone();
        let podcast_id = podcast_id.clone();
        let user_id = user_id.clone();
        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let favorite_status = favorite_status.clone();
            let is_favorite = !*favorite_status;
            let pod_id_deref = *podcast_id.clone();
            let user_id = user_id.clone().unwrap();

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    match call_toggle_podcast_favorite(
                        server_name.clone(),
                        api_key.clone().unwrap(),
                        user_id,
                        pod_id_deref,
                        is_favorite,
                    )
                    .await
                    {
                        Ok(_) => {
                            favorite_status.set(is_favorite);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error toggling favorite: {}", e).into(),
                            );
                        }
                    }
                }
            });
        })
    };

    let toggle_podcast_covers = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let use_podcast_covers = use_podcast_covers.clone();
        let podcast_id = podcast_id.clone();
        let user_id = user_id.clone();
        let dispatch = _search_dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let use_podcast_covers = use_podcast_covers.clone();
            let new_setting = !*use_podcast_covers;
            let pod_id_deref = *podcast_id.clone();
            let user_id = user_id.clone().unwrap();
            let _dispatch = dispatch.clone();

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    match call_set_global_podcast_cover_preference(
                        server_name,
                        &api_key.clone().unwrap(),
                        user_id,
                        new_setting,
                        Some(pod_id_deref),
                    )
                    .await
                    {
                        Ok(_) => {
                            use_podcast_covers.set(new_setting);
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Some(format!(
                                    "Podcast cover preference {} for this podcast",
                                    if new_setting { "enabled" } else { "disabled" }
                                ));
                            });
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error updating podcast cover preference: {}", e));
                            });
                        }
                    }
                }
            });
        })
    };

    let start_skip_call = start_skip.clone();
    let end_skip_call = end_skip.clone();
    let start_skip_call_button = start_skip.clone();
    let end_skip_call_button = end_skip.clone();
    let skip_dispatch = _search_dispatch.clone();

    // Save the skip times to the server
    let save_skip_times = {
        let start_skip = start_skip.clone();
        let end_skip = end_skip.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let skip_dispatch = skip_dispatch.clone();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let i18n_skip_times_adjusted = i18n_skip_times_adjusted.clone();
            let i18n_error_adjusting_skip_times = i18n_error_adjusting_skip_times.clone();
            let _skip_call_dispatch = skip_dispatch.clone();
            let start_skip = *start_skip;
            let end_skip = *end_skip;
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap();
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    let request = SkipTimesRequest {
                        podcast_id,
                        start_skip,
                        end_skip,
                        user_id,
                    };

                    match call_adjust_skip_times(&server_name, &api_key, &request).await {
                        Ok(_) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Option::from(i18n_skip_times_adjusted)
                            });
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error updating skip times: {}", e).into(),
                            );
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Option::from(i18n_error_adjusting_skip_times)
                            });
                        }
                    }
                }
            });
        })
    };

    // Load per-podcast silence-trim settings when the podcast id becomes known.
    {
        let trim_silence = trim_silence.clone();
        let silence_threshold = silence_threshold.clone();
        let auto_transcribe = auto_transcribe.clone();
        let ai_available = ai_available.clone();
        let auto_ad_detect = auto_ad_detect.clone();
        let ad_skip_auto_activate = ad_skip_auto_activate.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let podcast_id = podcast_id.clone();
        use_effect_with(*podcast_id, move |pid| {
            let pid = *pid;
            if pid != 0 {
                if let (Some(api_key), Some(server_name), Some(user_id)) =
                    (api_key.clone(), server_name.clone(), user_id.clone())
                {
                    let at_api_key = api_key.clone();
                    let at_server_name = server_name.clone();
                    let auto_transcribe = auto_transcribe.clone();
                    let ai_available = ai_available.clone();
                    let auto_ad_detect = auto_ad_detect.clone();
                    let ad_skip_auto_activate = ad_skip_auto_activate.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(settings) = call_get_silence_trim(
                            &server_name,
                            &api_key,
                            user_id,
                            pid,
                        )
                        .await
                        {
                            trim_silence.set(settings.enabled);
                            silence_threshold.set(settings.threshold);
                        }
                    });
                    // Transcription (#726) + ad detection (#790): AI availability + per-podcast opt-ins.
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Ok(up) = call_get_ai_status(&at_server_name, &at_api_key).await {
                            ai_available.set(up);
                        }
                        if let Ok(enabled) =
                            call_get_auto_transcribe(&at_server_name, &at_api_key, user_id, pid).await
                        {
                            auto_transcribe.set(enabled);
                        }
                        if let Ok(enabled) =
                            call_get_auto_ad_detect(&at_server_name, &at_api_key, user_id, pid).await
                        {
                            auto_ad_detect.set(enabled);
                        }
                        if let Ok(enabled) =
                            call_get_ad_skip_auto_activate(&at_server_name, &at_api_key, user_id, pid).await
                        {
                            ad_skip_auto_activate.set(enabled);
                        }
                    });
                }
            }
            || ()
        });
    }

    // Save the silence-trim settings to the server
    let save_silence_trim = {
        let trim_silence = trim_silence.clone();
        let silence_threshold = silence_threshold.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        let silence_updated_msg = i18n.t("episodes_layout.silence_trim_updated").to_string();
        let silence_error_msg = i18n.t("episodes_layout.silence_trim_error").to_string();

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let enabled = *trim_silence;
            let threshold = *silence_threshold;
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap();
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;
            let silence_updated_msg = silence_updated_msg.clone();
            let silence_error_msg = silence_error_msg.clone();

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key.as_ref(), server_name.as_ref())
                {
                    let request = SilenceTrimRequest {
                        podcast_id,
                        user_id,
                        enabled,
                        threshold,
                    };
                    match call_adjust_silence_trim(server_name, api_key, &request)
                        .await
                    {
                        Ok(_) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Option::from(silence_updated_msg.clone())
                            });
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error updating silence trim: {}", e).into(),
                            );
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Option::from(silence_error_msg.clone())
                            });
                        }
                    }
                }
            });
        })
    };

    // UI bindings for the silence-trim controls
    let trim_checked = *trim_silence;
    let threshold_val = *silence_threshold;
    let trim_toggle = {
        let trim_silence = trim_silence.clone();
        Callback::from(move |_: MouseEvent| {
            trim_silence.set(!*trim_silence);
        })
    };
    let threshold_change = {
        let silence_threshold = silence_threshold.clone();
        Callback::from(move |e: Event| {
            if let Some(sel) = e.target_dyn_into::<web_sys::HtmlSelectElement>() {
                silence_threshold.set(sel.value().parse::<i32>().unwrap_or(2));
            }
        })
    };

    // Auto-transcribe (#726): toggle immediately persists (no separate save button).
    let ai_available_val = *ai_available;
    let auto_transcribe_checked = *auto_transcribe;
    let auto_transcribe_toggle = {
        let auto_transcribe = auto_transcribe.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        Callback::from(move |_: MouseEvent| {
            let enabled = !*auto_transcribe;
            auto_transcribe.set(enabled);
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap_or(0);
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;
            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let request = AutoTranscribeRequest { podcast_id, user_id, enabled };
                    if let Err(e) =
                        call_adjust_auto_transcribe(&server_name, &api_key, &request).await
                    {
                        web_sys::console::log_1(&format!("Error updating auto-transcribe: {}", e).into());
                    }
                }
            });
        })
    };

    // Auto ad-detect (#790): only enabled when auto-transcribe is on (ads need the transcript).
    let auto_ad_detect_checked = *auto_ad_detect;
    let ad_skip_auto_activate_checked = *ad_skip_auto_activate;
    let auto_ad_detect_toggle = {
        let auto_ad_detect = auto_ad_detect.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        Callback::from(move |_: MouseEvent| {
            let enabled = !*auto_ad_detect;
            auto_ad_detect.set(enabled);
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap_or(0);
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;
            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let request = AutoAdDetectRequest { podcast_id, user_id, enabled };
                    if let Err(e) = call_adjust_auto_ad_detect(&server_name, &api_key, &request).await {
                        web_sys::console::log_1(&format!("Error updating auto ad-detect: {}", e).into());
                    }
                }
            });
        })
    };
    let ad_skip_auto_activate_toggle = {
        let ad_skip_auto_activate = ad_skip_auto_activate.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let podcast_id = podcast_id.clone();
        Callback::from(move |_: MouseEvent| {
            let enabled = !*ad_skip_auto_activate;
            ad_skip_auto_activate.set(enabled);
            let api_key = api_key.clone();
            let user_id = user_id.clone().unwrap_or(0);
            let server_name = server_name.clone();
            let podcast_id = *podcast_id;
            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let request = AutoAdDetectRequest { podcast_id, user_id, enabled };
                    if let Err(e) = call_adjust_ad_skip_auto_activate(&server_name, &api_key, &request).await {
                        web_sys::console::log_1(&format!("Error updating ad-skip auto-activate: {}", e).into());
                    }
                }
            });
        })
    };

    // let onclick_cat = new_category
    let app_dispatch_add = _search_dispatch.clone();
    let onclick_add = {
        // let dispatch = dispatch.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone(); // Assuming user_id is an Option<i32> or similar
        let podcast_id = podcast_id.clone(); // Assuming this is available in your context
        let new_category = new_category.clone(); // Assuming this is a state that stores the new category input

        Callback::from(move |event: web_sys::MouseEvent| {
            event.prevent_default(); // Prevent the default form submit or page reload behavior
            let _app_dispatch = app_dispatch_add.clone();
            if new_category.is_empty() {
                web_sys::console::log_1(&i18n_category_name_cannot_be_empty.clone().into());
                return;
            }

            // let dispatch = dispatch.clone();
            let server_name = server_name.clone().unwrap();
            let api_key = api_key.clone().unwrap();
            let user_id = user_id.clone().unwrap(); // Assuming user_id is Some(i32)
            let podcast_id = *podcast_id; // Assuming podcast_id is Some(i32)
            let category_name = (*new_category).clone();
            let cat_name_dis = category_name.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let request_data = AddCategoryRequest {
                    podcast_id,
                    user_id,
                    category: category_name,
                };

                // Await the async function call
                let response = call_add_category(&server_name, &api_key, &request_data).await;

                // Match on the awaited response
                match response {
                    Ok(_) => {
                        Dispatch::<PodcastFeedState>::global().reduce_mut(|state| {
                            if let Some(ref mut podcast_info) = state.clicked_podcast_info {
                                if let Some(ref mut categories) = podcast_info.categories {
                                    // Add the new category to the HashMap
                                    categories.insert(cat_name_dis.clone(), cat_name_dis.clone());
                                } else {
                                    // Initialize the HashMap if it's None
                                    let mut new_map = HashMap::new();
                                    new_map.insert(cat_name_dis.clone(), cat_name_dis);
                                    podcast_info.categories = Some(new_map);
                                }
                            }
                        });
                    }
                    Err(err) => {
                        web_sys::console::log_1(&format!("Error adding category: {}", err).into());
                    }
                }
            });
        })
    };

    let category_to_remove = use_state(|| None::<String>);
    let onclick_remove = {
        let category_to_remove = category_to_remove.clone();
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            let target = event.target_unchecked_into::<Element>();
            let closest_button = target.closest("button").unwrap();
            if let Some(button) = closest_button {
                if let Some(category) = button.get_attribute("data-category") {
                    category_to_remove.set(Some(category));
                }
            }
        })
    };

    let _app_dispatch = _search_dispatch.clone();

    {
        let category_to_remove = category_to_remove.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id;
        let podcast_id = *podcast_id;

        use_effect_with(category_to_remove, move |category_to_remove| {
            if let Some(category) = (**category_to_remove).clone() {
                let server_name = server_name.clone().unwrap();
                let api_key = api_key.clone().unwrap();
                let user_id = user_id.unwrap();
                let category_request = category.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let request_data = RemoveCategoryRequest {
                        podcast_id,
                        user_id,
                        category,
                    };
                    // Your API call here
                    let response =
                        call_remove_category(&server_name, &api_key, &request_data).await;
                    match response {
                        Ok(_) => {
                            Dispatch::<PodcastFeedState>::global().reduce_mut(|state| {
                                if let Some(ref mut podcast_info) = state.clicked_podcast_info {
                                    if let Some(ref mut categories) = podcast_info.categories {
                                        // Filter the HashMap and collect back into HashMap
                                        *categories = categories
                                            .clone()
                                            .into_iter()
                                            .filter(|(_, cat)| cat != &category_request) // Ensure you're comparing correctly
                                            .collect();
                                    }
                                }
                            });
                        }
                        Err(err) => {
                            web_sys::console::log_1(
                                &format!("Error removing category: {}", err).into(),
                            );
                        }
                    }
                });
            }
            || ()
        });
    }

    // Fetch RSS key when RSS feed modal is shown
    {
        let rss_key_state = rss_key_state.clone();
        let server_name = search_state
            .auth_details
            .as_ref()
            .map(|ud| ud.server_name.clone());
        let api_key = api_key.clone().flatten();
        let page_state_clone = page_state.clone();

        use_effect_with(
            (page_state_clone.clone(), rss_key_state.is_none()),
            move |(current_page_state, rss_key_is_none)| {
                if matches!(**current_page_state, PageState::RSSFeed) && *rss_key_is_none {
                    if let (Some(server_name), Some(api_key), Some(user_id)) =
                        (server_name.clone(), api_key.clone(), user_id.clone())
                    {
                        let rss_key_state = rss_key_state.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            match call_get_rss_key(&server_name, &Some(api_key), user_id).await {
                                Ok(rss_key) => {
                                    rss_key_state.set(Some(rss_key));
                                }
                                Err(e) => {
                                    web_sys::console::log_1(
                                        &format!("Failed to fetch RSS key: {}", e).into(),
                                    );
                                }
                            }
                        });
                    }
                }
                || ()
            },
        );
    }

    let rss_feed_modal = {
        let rss_key_state_clone = rss_key_state.clone();

        let rss_url = match (*rss_key_state_clone).as_ref() {
            Some(rss_key) => format!(
                "{}/{}?api_key={}&podcast_id={}",
                get_rss_base_url(),
                user_id.clone().unwrap_or_default(),
                rss_key,
                *podcast_id
            ),
            None => i18n_loading_rss_key.clone(),
        };

        let copy_onclick = {
            let rss_url = rss_url.clone();
            Callback::from(move |_| {
                if let Some(window) = web_sys::window() {
                    let _ = window.navigator().clipboard().write_text(&rss_url);
                }
            })
        };

        html! {
            <div id="rss_feed_modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
                <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                    <div class="modal-container relative rounded-lg shadow">
                        <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                            <h3 class="text-xl font-semibold">
                                {&i18n_rss_feed_url}
                            </h3>
                            <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                                <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                                </svg>
                                <span class="sr-only">{&i18n.t("episodes_layout.close_modal")}</span>
                            </button>
                        </div>
                        <div class="p-4 md:p-5">
                            <div>
                                <label for="rss_link" class="block mb-2 text-sm font-medium">{&i18n_rss_feed_note}</label>
                                <label for="rss_link" class="block mb-2 text-sm font-medium">{&i18n_rss_feed_instruction}</label>
                                <div class="relative">
                                    <input
                                        type="text"
                                        id="rss_link"
                                        class="input-black w-full px-3 py-2 border border-gray-300 rounded-md pr-20"
                                        value={rss_url}
                                        readonly=true
                                    />
                                    <button
                                        onclick={copy_onclick}
                                        class="absolute right-2 top-1/2 transform -translate-y-1/2 px-4 py-1 text-sm text-blue-600 hover:text-blue-800 dark:text-blue-400 dark:hover:text-blue-300"
                                    >
                                        {&i18n.t("episodes_layout.copy")}
                                    </button>
                                </div>
                                <p class="mt-2 text-sm text-gray-500">{&i18n_rss_feed_warning}</p>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        }
    };

    // Define the modal components
    let clicked_feed = clicked_podcast_info.clone();
    let podcast_option_model = html! {
        <div id="podcast_option_model" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25 py-8" onclick={on_background_click.clone()}>
            <div class="modal-container relative w-full max-w-md max-h-full rounded-lg shadow mx-4 overflow-hidden flex flex-col" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow flex-1 flex flex-col overflow-hidden">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t flex-shrink-0">
                        <h3 class="text-xl font-semibold">
                            {&i18n.t("episodes_layout.podcast_options")}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{&i18n.t("episodes_layout.close_modal")}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5 overflow-y-auto flex-1">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{&i18n_download_future_episodes}</label>
                                <label class="inline-flex relative items-center cursor-pointer">
                                    <input type="checkbox" checked={*download_status} class="sr-only peer" onclick={toggle_download} />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                </label>
                            </div>
                            <div>
                                <label for="auto_play_next" class="block mb-2 text-sm font-medium">{ &i18n_auto_play_next_episode }</label>
                                <label class="inline-flex relative items-center cursor-pointer">
                                    <input type="checkbox" checked={*auto_play_next_status} class="sr-only peer" onclick={toggle_auto_play_next} />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                </label>
                            </div>
                            <div>
                                <label for="auto_queue" class="block mb-2 text-sm font-medium">{ &i18n_auto_queue_new_episodes }</label>
                                <label class="inline-flex relative items-center cursor-pointer">
                                    <input type="checkbox" checked={*auto_queue_status} class="sr-only peer" onclick={toggle_auto_queue} />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                </label>
                            </div>
                            <div>
                                <label for="notification_settings" class="block mb-2 text-sm font-medium">{&i18n_get_notifications_new_episodes}</label>
                                <label class="inline-flex relative items-center cursor-pointer">
                                    <input
                                        type="checkbox"
                                        checked={*notification_status}
                                        class="sr-only peer"
                                        onclick={toggle_notifications}
                                    />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                </label>
                            </div>
                            <div>
                                <label for="favorite_settings" class="block mb-2 text-sm font-medium">{ &i18n_favorite_podcast }</label>
                                <label class="inline-flex relative items-center cursor-pointer">
                                    <input
                                        type="checkbox"
                                        checked={*favorite_status}
                                        class="sr-only peer"
                                        onclick={toggle_favorite}
                                    />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                </label>
                            </div>

                            <div class="mt-4">
                                <label for="playback-speed" class="block mb-2 text-sm font-medium">{&i18n_default_playback_speed}</label>
                                <div class="flex items-center space-x-2">
                                    <input
                                        type="number"
                                        id="playback-speed"
                                        value={format!("{:.1}", *playback_speed_clone)} // Format to 1 decimal place
                                        class="email-input border text-sm rounded-lg p-2.5 w-20"
                                        oninput={playback_speed_input_handler}
                                        min="0.5"
                                        max="2.0"
                                        step="0.1"
                                    />
                                    <span class="text-sm">{"x"}</span>
                                    <button
                                        class="save-button font-bold py-2 px-4 rounded"
                                        onclick={save_playback_speed}
                                    >
                                        {&i18n.t("episodes_layout.save")}
                                    </button>
                                    <button
                                        class="clear-button bg-gray-300 hover:bg-gray-400 text-gray-800 font-bold py-2 px-4 rounded"
                                        onclick={clear_playback_speed}
                                    >
                                        {&i18n.t("episodes_layout.reset")}
                                    </button>
                                </div>
                                <div class="mt-2">
                                    if *playback_speed_customized_render {
                                        <span class="inline-flex items-center gap-1 text-xs font-medium px-2 py-0.5 rounded-full bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200">
                                            <i class="ph ph-pencil-simple"></i>
                                            {&i18n_playback_speed_custom_badge}
                                        </span>
                                    } else {
                                        <span class="inline-flex items-center gap-1 text-xs font-medium px-2 py-0.5 rounded-full bg-gray-200 text-gray-700 dark:bg-gray-700 dark:text-gray-200">
                                            <i class="ph ph-globe"></i>
                                            {&i18n_playback_speed_global_badge}
                                        </span>
                                    }
                                </div>
                                <p class="text-xs text-gray-500 mt-1">{&i18n_playback_speed_description}</p>
                            </div>

                            <div class="mt-4">
                                <label for="auto-delete-days" class="block mb-2 text-sm font-medium">{&i18n_auto_delete_label}</label>
                                <div class="flex items-center space-x-2">
                                    <input
                                        type="number"
                                        id="auto-delete-days"
                                        value={(*auto_delete_days_input).clone()}
                                        class="email-input border text-sm rounded-lg p-2.5 w-20"
                                        oninput={auto_delete_days_input_handler}
                                        min="0"
                                        max="3650"
                                        step="1"
                                    />
                                    <span class="text-sm">{&i18n_auto_delete_days_unit}</span>
                                    <button
                                        class="save-button font-bold py-2 px-4 rounded"
                                        onclick={save_auto_delete_days}
                                    >
                                        {&i18n.t("episodes_layout.save")}
                                    </button>
                                    <button
                                        class="clear-button bg-gray-300 hover:bg-gray-400 text-gray-800 font-bold py-2 px-4 rounded"
                                        onclick={clear_auto_delete_days}
                                    >
                                        {&i18n.t("episodes_layout.reset")}
                                    </button>
                                </div>
                                <div class="mt-2">
                                    if *auto_delete_customized_render {
                                        <span class="inline-flex items-center gap-1 text-xs font-medium px-2 py-0.5 rounded-full bg-blue-100 text-blue-800 dark:bg-blue-900 dark:text-blue-200">
                                            <i class="ph ph-pencil-simple"></i>
                                            {&i18n_auto_delete_custom_badge}
                                        </span>
                                    } else {
                                        <span class="inline-flex items-center gap-1 text-xs font-medium px-2 py-0.5 rounded-full bg-gray-200 text-gray-700 dark:bg-gray-700 dark:text-gray-200">
                                            <i class="ph ph-globe"></i>
                                            {&i18n_auto_delete_global_badge}
                                        </span>
                                    }
                                </div>
                                <p class="text-xs text-gray-500 mt-1">{&i18n_auto_delete_description}</p>
                            </div>

                            <div class="mt-4">
                                <label class="block mb-2 text-sm font-medium">{ &i18n_use_podcast_covers }</label>
                                <div class="flex items-center space-x-2">
                                    <label class="relative inline-flex items-center cursor-pointer">
                                        <input
                                            type="checkbox"
                                            checked={*use_podcast_covers}
                                            class="sr-only peer"
                                            onclick={toggle_podcast_covers.clone()}
                                        />
                                        <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                    </label>
                                </div>
                                <p class="text-xs text-gray-500 mt-1">{ &i18n_podcast_cover_hint }</p>
                            </div>

                            <div class="mt-4">
                                <label for="auto-skip" class="block mb-2 text-sm font-medium">{&i18n_auto_skip_intros_outros}</label>
                                <div class="flex items-center space-x-2">
                                    <div class="flex items-center space-x-2">
                                        <label for="start-skip" class="block text-sm font-medium">{&i18n_start_skip_seconds}</label>
                                        <input
                                            type="number"
                                            id="start-skip"
                                            value={start_skip_call_button.to_string()}
                                            class="email-input border text-sm rounded-lg p-2.5 w-16"
                                            oninput={Callback::from(move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    let value = input.value().parse::<i32>().unwrap_or(0);
                                                    start_skip_call.set(value);
                                                }
                                            })}
                                        />
                                    </div>
                                    <div class="flex items-center space-x-2">
                                        <label for="end-skip" class="block text-sm font-medium">{&i18n_end_skip_seconds}</label>
                                        <input
                                            type="number"
                                            id="end-skip"
                                            value={end_skip_call_button.to_string()}
                                            class="email-input border text-sm rounded-lg p-2.5 w-16"
                                            oninput={Callback::from(move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    let value = input.value().parse::<i32>().unwrap_or(0);
                                                    end_skip_call.set(value);
                                                }
                                            })}
                                        />
                                    </div>
                                    <button
                                        class="download-button font-bold py-2 px-4 rounded"
                                        onclick={save_skip_times}
                                    >
                                        {&i18n.t("episodes_layout.confirm")}
                                    </button>
                                </div>
                            </div>

                            <div class="mt-4">
                                <label for="trim-silence" class="block mb-2 text-sm font-medium">{ &i18n.t("episodes_layout.trim_silence") }</label>
                                <label class="inline-flex relative items-center cursor-pointer">
                                    <input
                                        type="checkbox"
                                        id="trim-silence"
                                        checked={trim_checked}
                                        class="sr-only peer"
                                        onclick={trim_toggle}
                                    />
                                    <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                    <span class="ms-3 text-sm">{ &i18n.t("episodes_layout.trim_silence_desc") }</span>
                                </label>
                                // Aggressiveness on its own line.
                                <div class="flex items-center space-x-2 mt-3">
                                    <label for="silence-threshold" class="block text-sm font-medium">{ &i18n.t("episodes_layout.aggressiveness") }</label>
                                    <select
                                        id="silence-threshold"
                                        class="email-input border text-sm rounded-lg p-2.5"
                                        onchange={threshold_change}
                                    >
                                        <option value="1" selected={threshold_val == 1}>{ &i18n.t("episodes_layout.silence_low") }</option>
                                        <option value="2" selected={threshold_val == 2}>{ &i18n.t("episodes_layout.silence_medium") }</option>
                                        <option value="3" selected={threshold_val == 3}>{ &i18n.t("episodes_layout.silence_high") }</option>
                                    </select>
                                    <button
                                        class="download-button font-bold py-2 px-4 rounded"
                                        onclick={save_silence_trim}
                                    >
                                        {&i18n.t("episodes_layout.confirm")}
                                    </button>
                                </div>
                            </div>

                            {
                                // Auto-transcribe control — only meaningful when the AI sidecar is up.
                                if ai_available_val {
                                    html! {
                                        <div class="mt-4">
                                            <label for="auto-transcribe" class="block mb-2 text-sm font-medium">{ &i18n.t("episodes_layout.auto_transcribe") }</label>
                                            <label class="inline-flex relative items-center cursor-pointer">
                                                <input
                                                    type="checkbox"
                                                    id="auto-transcribe"
                                                    checked={auto_transcribe_checked}
                                                    class="sr-only peer"
                                                    onclick={auto_transcribe_toggle}
                                                />
                                                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                                <span class="ms-3 text-sm">{ &i18n.t("episodes_layout.auto_transcribe_desc") }</span>
                                            </label>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }

                            {
                                // Auto ad-detect (#790) — enabled only when auto-transcribe is on.
                                if ai_available_val {
                                    html! {
                                        <div class="mt-4">
                                            <label for="auto-ad-detect" class="block mb-2 text-sm font-medium">{ &i18n.t("episodes_layout.auto_ad_detect") }</label>
                                            <label class={ if auto_transcribe_checked { "inline-flex relative items-center cursor-pointer" } else { "inline-flex relative items-center cursor-not-allowed opacity-50" } }>
                                                <input
                                                    type="checkbox"
                                                    id="auto-ad-detect"
                                                    checked={auto_ad_detect_checked}
                                                    disabled={!auto_transcribe_checked}
                                                    class="sr-only peer"
                                                    onclick={auto_ad_detect_toggle}
                                                />
                                                <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                                <span class="ms-3 text-sm">{ &i18n.t("episodes_layout.auto_ad_detect_desc") }</span>
                                            </label>
                                            {
                                                // Skip-immediately vs. confirm-first — only when auto-detect is on.
                                                if auto_transcribe_checked && auto_ad_detect_checked {
                                                    html! {
                                                        <label class="inline-flex relative items-center cursor-pointer mt-3">
                                                            <input
                                                                type="checkbox"
                                                                id="ad-skip-auto-activate"
                                                                checked={ad_skip_auto_activate_checked}
                                                                class="sr-only peer"
                                                                onclick={ad_skip_auto_activate_toggle}
                                                            />
                                                            <div class="w-11 h-6 bg-gray-200 peer-focus:outline-none peer-focus:ring-4 peer-focus:ring-blue-300 dark:peer-focus:ring-blue-800 rounded-full peer dark:bg-gray-700 peer-checked:after:translate-x-full rtl:peer-checked:after:-translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:start-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all dark:border-gray-600 peer-checked:bg-blue-600"></div>
                                                            <span class="ms-3 text-sm">{ &i18n.t("episodes_layout.ad_skip_auto_activate_desc") }</span>
                                                        </label>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }

                            {
                                if let Some(info) = &podcast_info {
                                    if info.is_youtube.unwrap_or(false) {
                                        html! {
                                            <div class="mt-4">
                                                <label for="feed-cutoff" class="block mb-2 text-sm font-medium">{&i18n_youtube_download_limit}</label>
                                                <div class="flex items-center space-x-2">
                                                    <input
                                                        type="number"
                                                        id="feed-cutoff"
                                                        value={(*feed_cutoff_days_input).clone()}
                                                        class="email-input border text-sm rounded-lg p-2.5 w-24"
                                                        oninput={feed_cutoff_days_input_handler}
                                                        min="0"
                                                    />
                                                    <span class="text-sm text-gray-500">{"0 = No limit"}</span>
                                                    <button
                                                        class="download-button font-bold py-2 px-4 rounded"
                                                        onclick={save_feed_cutoff_days}
                                                    >
                                                        {&i18n.t("episodes_layout.save")}
                                                    </button>
                                                </div>
                                                <p class="text-xs text-gray-500 mt-1">{&i18n_youtube_limit_description}</p>
                                            </div>
                                        }
                                    } else {
                                        html! {}  // Render nothing if it's not a YouTube podcast
                                    }
                                } else {
                                    html! {}  // Render nothing if podcast_info is None
                                }
                            }

                            // Categories section of the modal
                            <div>
                                <label for="category_adjust" class="block mb-2 text-sm font-medium">
                                    {&i18n_adjust_podcast_categories}
                                </label>
                                <div class="flex flex-wrap gap-2">
                                {
                                    if let Some(feed) = clicked_feed.as_ref() {
                                        if let Some(categories) = &feed.categories {
                                            html! {
                                                <>
                                                    { categories.iter().map(|(_, category_name)| {
                                                        let category_name = category_name.clone();
                                                        let onclick_remove = onclick_remove.clone();
                                                        let category_to_remove = category_to_remove.clone();

                                                        let remove_callback = {
                                                            let category_name = category_name.clone();
                                                            let onclick_remove = onclick_remove.clone();
                                                            Callback::from(move |e: MouseEvent| {
                                                                e.prevent_default();
                                                                onclick_remove.emit(e);
                                                                category_to_remove.set(Some(category_name.clone()));
                                                            })
                                                        };

                                                        html! {
                                                            <div class="category-tag">
                                                                <span>{&category_name}</span>
                                                                <button
                                                                    class="category-remove-btn"
                                                                    onclick={remove_callback}
                                                                    data-category={category_name.clone()}
                                                                >
                                                                    <i class="ph ph-trash text-lg" />
                                                                </button>
                                                            </div>
                                                        }
                                                    }).collect::<Html>() }
                                                </>
                                            }
                                        } else {
                                            html! { <p class="text-sm text-muted">{ &i18n_no_categories_available }</p> }
                                        }
                                    } else {
                                        html! { <p class="text-sm text-muted">{ &i18n_loading }</p> }
                                    }
                                }
                                </div>

                                <div class="relative mt-4">
                                    <input
                                        type="text"
                                        id="new_category"
                                        class="category-input w-full px-4 py-3 pr-24 rounded-lg border"
                                        placeholder={i18n_new_category_placeholder.clone()}
                                        value={(*new_category).clone()}
                                        oninput={new_category_input}
                                    />
                                    <button
                                        class="category-add-btn"
                                        onclick={onclick_add}
                                    >
                                        <i class="ph ph-plus text-lg" />
                                        <span class="hidden md:inline">{&i18n.t("episodes_layout.add")}</span>
                                    </button>
                                </div>
                            </div>

                            // Edit podcast info button
                            <div class="mt-4">
                                <button
                                    type="button"
                                    class="download-button font-bold py-2 px-4 rounded"
                                    onclick={toggle_edit_podcast}
                                >
                                    {&i18n.t("episodes_layout.modify_podcast_info")}
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define the modal components
    let download_all_model = html! {
        <div id="download_all_modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {&i18n.t("episodes_layout.verify_downloads")}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{&i18n.t("episodes_layout.close_modal")}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{&i18n_download_all_confirmation}</label>
                                <div class="flex justify-between space-x-4">
                                    <button onclick={download_all_click} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {&i18n_yes_download_all}
                                    </button>
                                    <button onclick={on_close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {&i18n_no_take_me_back}
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define the modal components
    let delete_pod_model = html! {
        <div id="delete_pod_model" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {&i18n.t("episodes_layout.delete_podcast")}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{&i18n.t("episodes_layout.close_modal")}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{&i18n_delete_podcast_confirmation}</label>
                                <div class="flex justify-between space-x-4">
                                    <button onclick={delete_all_click} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {&i18n_yes_delete_podcast}
                                    </button>
                                    <button onclick={on_close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {&i18n_no_take_me_back}
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define the edit podcast modal
    let edit_podcast_modal = {
        html! {
            <div id="edit_podcast_modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25 py-8" onclick={on_background_click.clone()}>
                <div class="modal-container relative w-full max-w-md max-h-full rounded-lg shadow mx-4 overflow-hidden flex flex-col" onclick={stop_propagation.clone()}>
                    <div class="modal-container relative rounded-lg shadow flex-1 flex flex-col overflow-hidden">
                        <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t flex-shrink-0">
                            <h3 class="text-xl font-semibold">
                                {&i18n.t("episodes_layout.edit_podcast_info")}
                            </h3>
                            <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                                <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                                </svg>
                                <span class="sr-only">{&i18n.t("episodes_layout.close_modal")}</span>
                            </button>
                        </div>
                        <div class="p-4 md:p-5 overflow-y-auto flex-1">
                            <form class="space-y-4" action="#">
                                <div>
                                    <label for="edit_feed_url" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.feed_url")}
                                    </label>
                                    <input
                                        type="url"
                                        id="edit_feed_url"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.feed_url_placeholder").to_string()}
                                        value={(*edit_feed_url).clone()}
                                        oninput={Callback::from({
                                            let edit_feed_url = edit_feed_url.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    edit_feed_url.set(input.value());
                                                }
                                            }
                                        })}
                                    />
                                </div>
                                <div>
                                    <label for="edit_username" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.username")}
                                    </label>
                                    <input
                                        type="text"
                                        id="edit_username"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.username_placeholder").to_string()}
                                        value={(*edit_username).clone()}
                                        oninput={Callback::from({
                                            let edit_username = edit_username.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    edit_username.set(input.value());
                                                }
                                            }
                                        })}
                                    />
                                </div>
                                <div>
                                    <label for="edit_password" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.password")}
                                    </label>
                                    <input
                                        type="password"
                                        id="edit_password"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.password_placeholder").to_string()}
                                        value={(*edit_password).clone()}
                                        oninput={Callback::from({
                                            let edit_password = edit_password.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    edit_password.set(input.value());
                                                }
                                            }
                                        })}
                                    />
                                </div>
                                <div>
                                    <label for="edit_podcast_name" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.podcast_name")}
                                    </label>
                                    <input
                                        type="text"
                                        id="edit_podcast_name"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.podcast_name_placeholder").to_string()}
                                        value={(*edit_podcast_name).clone()}
                                        oninput={Callback::from({
                                            let edit_podcast_name = edit_podcast_name.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    edit_podcast_name.set(input.value());
                                                }
                                            }
                                        })}
                                    />
                                </div>
                                <div>
                                    <label for="edit_description" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.description")}
                                    </label>
                                    <textarea
                                        id="edit_description"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.description_placeholder").to_string()}
                                        value={(*edit_description).clone()}
                                        oninput={Callback::from({
                                            let edit_description = edit_description.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<web_sys::HtmlTextAreaElement>() {
                                                    edit_description.set(input.value());
                                                }
                                            }
                                        })}
                                        rows="3"
                                    />
                                </div>
                                <div>
                                    <label for="edit_author" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.author")}
                                    </label>
                                    <input
                                        type="text"
                                        id="edit_author"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.author_placeholder").to_string()}
                                        value={(*edit_author).clone()}
                                        oninput={Callback::from({
                                            let edit_author = edit_author.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    edit_author.set(input.value());
                                                }
                                            }
                                        })}
                                    />
                                </div>
                                <div>
                                    <label for="edit_artwork_url" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.artwork_url")}
                                    </label>
                                    <input
                                        type="url"
                                        id="edit_artwork_url"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.artwork_url_placeholder").to_string()}
                                        value={(*edit_artwork_url).clone()}
                                        oninput={Callback::from({
                                            let edit_artwork_url = edit_artwork_url.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    edit_artwork_url.set(input.value());
                                                }
                                            }
                                        })}
                                    />
                                </div>
                                <div>
                                    <label for="edit_website_url" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.website_url")}
                                    </label>
                                    <input
                                        type="url"
                                        id="edit_website_url"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.website_url_placeholder").to_string()}
                                        value={(*edit_website_url).clone()}
                                        oninput={Callback::from({
                                            let edit_website_url = edit_website_url.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    edit_website_url.set(input.value());
                                                }
                                            }
                                        })}
                                    />
                                </div>
                                <div>
                                    <label for="edit_podcast_index_id" class="block mb-2 text-sm font-medium">
                                        {&i18n.t("episodes_layout.podcast_index_id")}
                                    </label>
                                    <input
                                        type="number"
                                        id="edit_podcast_index_id"
                                        class="email-input border text-sm rounded-lg p-2.5 w-full"
                                        placeholder={i18n.t("episodes_layout.podcast_index_id_placeholder").to_string()}
                                        value={(*edit_podcast_index_id).clone()}
                                        oninput={Callback::from({
                                            let edit_podcast_index_id = edit_podcast_index_id.clone();
                                            move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                                    edit_podcast_index_id.set(input.value());
                                                }
                                            }
                                        })}
                                    />
                                </div>

                                // Merge Podcasts Section
                                <div class="mb-4 p-4 border rounded-lg">
                                    <h4 class="text-lg font-semibold mb-3">{ &i18n_merge_podcasts }</h4>
                                    <p class="text-sm text-gray-600 mb-3">
                                        { &i18n_merge_description }
                                    </p>

                                    // Show currently merged podcasts
                                    if !(*current_merged_podcasts).is_empty() {
                                        <div class="mb-4">
                                            <label class="block mb-2 text-sm font-medium">
                                                { &i18n_currently_merged_podcasts }
                                            </label>
                                            <div>
                                                {
                                                    (*current_merged_podcasts).iter().map(|&merged_id| {
                                                        // Get podcast name from fetched details
                                                        let podcast_name = (*merged_podcast_details)
                                                            .get(&merged_id)
                                                            .map(|details| details.podcastname.clone())
                                                            .unwrap_or_else(|| format!("Podcast ID {}", merged_id));

                                                        let api_key = api_key.clone();
                                                        let server_name = server_name.clone();
                                                        let podcast_id = podcast_id.clone();
                                                        let user_id = user_id.clone();
                                                        let current_merged_podcasts = current_merged_podcasts.clone();
                                                        let merged_podcast_details = merged_podcast_details.clone();
                                                        let dispatch = _search_dispatch.clone();

                                                        html! {
                                                            <div class="merged-podcast-item">
                                                                <span class="merged-podcast-name">{podcast_name}</span>
                                                                <button
                                                                    type="button"
                                                                    class="clear-button unmerge-button"
                                                                    onclick={Callback::from(move |_| {
                                                                        let api_key = api_key.clone();
                                                                        let server_name = server_name.clone();
                                                                        let user_id = user_id.clone();
                                                                        let primary_id = *podcast_id;
                                                                        let current_merged_podcasts = current_merged_podcasts.clone();
                                                                        let merged_podcast_details = merged_podcast_details.clone();
                                                                        let _dispatch = dispatch.clone();

                                                                        spawn_local(async move {
                                                                            if let (Some(api_key), Some(server_name), Some(user_id)) = (api_key.as_ref(), server_name.as_ref(), user_id.as_ref()) {
                                                                                match call_unmerge_podcast(server_name, &api_key, primary_id, merged_id).await {
                                                                                    Ok(_) => {
                                                                                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                            state.info_message = Some("Podcast unmerged successfully".to_string())
                                                                                        });

                                                                                        // Reload merged podcasts list and details
                                                                                        match call_get_merged_podcasts(server_name, &api_key, primary_id).await {
                                                                                            Ok(merged_ids) => {
                                                                                                current_merged_podcasts.set(merged_ids.clone());

                                                                                                // Fetch details for remaining merged podcasts
                                                                                                let mut details_map = HashMap::new();
                                                                                                for &id in &merged_ids {
                                                                                                    if let Ok(details) = call_get_podcast_details(
                                                                                                        server_name,
                                                                                                        api_key.as_deref().unwrap(),
                                                                                                        *user_id,
                                                                                                        id,
                                                                                                    ).await {
                                                                                                        details_map.insert(id, details);
                                                                                                    }
                                                                                                }
                                                                                                merged_podcast_details.set(details_map);
                                                                                            },
                                                                                            Err(e) => {
                                                                                                console::log_1(&format!("Error reloading merged podcasts: {}", e).into());
                                                                                            }
                                                                                        }
                                                                                    },
                                                                                    Err(e) => {
                                                                                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                            state.error_message = Some(format!("Failed to unmerge podcast: {}", e))
                                                                                        });
                                                                                    }
                                                                                }
                                                                            }
                                                                        });
                                                                    })}
                                                                >
                                                                    { &i18n_unmerge }
                                                                </button>
                                                            </div>
                                                        }
                                                    }).collect::<Html>()
                                                }
                                            </div>
                                        </div>
                                    }

                                    // Podcast selector for merging
                                    <div class="mb-3">
                                        <label class="block mb-2 text-sm font-medium">
                                            { &i18n_select_podcasts_to_merge }
                                        </label>
                                        <PodcastMergeSelector
                                            selected_podcasts={(*selected_podcasts_to_merge).clone()}
                                            on_select={{
                                                let selected_podcasts_to_merge = selected_podcasts_to_merge.clone();
                                                Callback::from(move |new_selection| {
                                                    selected_podcasts_to_merge.set(new_selection);
                                                })
                                            }}
                                            available_podcasts={(*available_podcasts_for_merge).clone()}
                                            loading={*loading_merge_data}
                                        />
                                    </div>

                                    // Merge button
                                    if !(*selected_podcasts_to_merge).is_empty() {
                                        <button
                                            type="button"
                                            class="save-button font-bold py-2 px-4 rounded mr-2"
                                            onclick={{
                                                let selected_podcasts_to_merge = selected_podcasts_to_merge.clone();
                                                let clicked_podcast_info = clicked_podcast_info.clone();
                                                let api_key = api_key.clone();
                                                let server_name = server_name.clone();
                                                let user_id = user_id.clone();
                                                let podcast_id = podcast_id.clone();
                                                let current_merged_podcasts = current_merged_podcasts.clone();
                                                let merged_podcast_details = merged_podcast_details.clone();
                                                let dispatch = _search_dispatch.clone();

                                                Callback::from(move |_| {
                                                    if let (Some(api_key), Some(server_name), Some(user_id), Some(_podcast_info)) =
                                                        (api_key.as_ref(), server_name.as_ref(), user_id.as_ref(), clicked_podcast_info.as_ref()) {

                                                        let podcast_ids = (*selected_podcasts_to_merge).clone();
                                                        let primary_id = *podcast_id;
                                                        let selected_podcasts_to_merge = selected_podcasts_to_merge.clone();
                                                        let api_key = api_key.clone();
                                                        let server_name = server_name.clone();
                                                        let user_id = *user_id;
                                                        let current_merged_podcasts = current_merged_podcasts.clone();
                                                        let merged_podcast_details = merged_podcast_details.clone();
                                                        let _dispatch = dispatch.clone();

                                                        spawn_local(async move {
                                                            match call_merge_podcasts(&server_name, &api_key, primary_id, &podcast_ids).await {
                                                                Ok(_response) => {
                                                                    // Clear selection after successful merge
                                                                    selected_podcasts_to_merge.set(Vec::new());

                                                                    // Show success message
                                                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                        state.info_message = Some(format!("Successfully merged {} podcast(s)", podcast_ids.len()))
                                                                    });

                                                                    // Reload merged podcasts list and details
                                                                    match call_get_merged_podcasts(&server_name, &api_key, primary_id).await {
                                                                        Ok(merged_ids) => {
                                                                            current_merged_podcasts.set(merged_ids.clone());

                                                                            // Fetch details for all merged podcasts
                                                                            let mut details_map = HashMap::new();
                                                                            for &merged_id in &merged_ids {
                                                                                if let Ok(details) = call_get_podcast_details(
                                                                                    &server_name,
                                                                                    api_key.as_deref().unwrap(),
                                                                                    user_id,
                                                                                    merged_id,
                                                                                ).await {
                                                                                    details_map.insert(merged_id, details);
                                                                                }
                                                                            }
                                                                            merged_podcast_details.set(details_map);
                                                                        },
                                                                        Err(e) => {
                                                                            console::log_1(&format!("Error reloading merged podcasts: {}", e).into());
                                                                        }
                                                                    }
                                                                },
                                                                Err(e) => {
                                                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                        state.error_message = Some(format!("Failed to merge podcasts: {}", e))
                                                                    });
                                                                }
                                                            }
                                                        });
                                                    }
                                                })
                                            }}
                                        >
                                            {format!("Merge {} Podcasts", (*selected_podcasts_to_merge).len())}
                                        </button>
                                    }
                                </div>

                                <div class="flex justify-between space-x-4">
                                    <button
                                        type="button"
                                        class="save-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                                        onclick={{
                                            let edit_feed_url = edit_feed_url.clone();
                                            let edit_username = edit_username.clone();
                                            let edit_password = edit_password.clone();
                                            let edit_podcast_name = edit_podcast_name.clone();
                                            let edit_description = edit_description.clone();
                                            let edit_author = edit_author.clone();
                                            let edit_artwork_url = edit_artwork_url.clone();
                                            let edit_website_url = edit_website_url.clone();
                                            let edit_podcast_index_id = edit_podcast_index_id.clone();
                                            let clicked_podcast_info = clicked_podcast_info.clone();
                                            let api_key = api_key.clone();
                                            let server_name = server_name.clone();
                                            let user_id = user_id.clone();
                                            let page_state = page_state.clone();
                                            let dispatch = _search_dispatch.clone();
                                            let podcast_id = podcast_id.clone();

                                            Callback::from(move |_| {
                                                let edit_feed_url = edit_feed_url.clone();
                                                let edit_username = edit_username.clone();
                                                let edit_password = edit_password.clone();
                                                let edit_podcast_name = edit_podcast_name.clone();
                                                let edit_description = edit_description.clone();
                                                let edit_author = edit_author.clone();
                                                let edit_artwork_url = edit_artwork_url.clone();
                                                let edit_website_url = edit_website_url.clone();
                                                let edit_podcast_index_id = edit_podcast_index_id.clone();
                                                let clicked_podcast_info = clicked_podcast_info.clone();
                                                let api_key = api_key.clone();
                                                let server_name = server_name.clone();
                                                let user_id = user_id.clone();
                                                let page_state = page_state.clone();
                                                let _dispatch = dispatch.clone();

                                                if let Some(podcast_info) = clicked_podcast_info.as_ref() {
                                                    let current_feed_url = podcast_info.feedurl.clone();
                                                    let current_podcast_name = podcast_info.podcastname.clone();
                                                    let current_description = podcast_info.description.clone();
                                                    let current_author = podcast_info.author.clone();
                                                    let current_artwork_url = podcast_info.artworkurl.clone();
                                                    let current_website_url = podcast_info.websiteurl.clone();
                                                    let current_podcast_index_id = podcast_info.podcastindexid.to_string();
                                                    let current_podcast_id = *podcast_id;

                                                    spawn_local(async move {
                                                        // Check if podcast is loaded
                                                        if current_podcast_id == 0 {
                                                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                state.error_message = Some("Please wait for podcast to load before editing".to_string())
                                                            });
                                                            return;
                                                        }

                                                        let feed_url = if (*edit_feed_url).trim().is_empty() || *edit_feed_url == current_feed_url {
                                                            None
                                                        } else {
                                                            Some((*edit_feed_url).clone())
                                                        };

                                                        let username = if (*edit_username).trim().is_empty() {
                                                            None
                                                        } else {
                                                            Some((*edit_username).clone())
                                                        };

                                                        let password = if (*edit_password).trim().is_empty() {
                                                            None
                                                        } else {
                                                            Some((*edit_password).clone())
                                                        };


                                                        let podcast_name = if (*edit_podcast_name).trim().is_empty() || *edit_podcast_name == current_podcast_name {
                                                            None
                                                        } else {
                                                            Some((*edit_podcast_name).clone())
                                                        };

                                                        let description = if (*edit_description).trim().is_empty() || *edit_description == current_description {
                                                            None
                                                        } else {
                                                            Some((*edit_description).clone())
                                                        };

                                                        let author = if (*edit_author).trim().is_empty() || *edit_author == current_author {
                                                            None
                                                        } else {
                                                            Some((*edit_author).clone())
                                                        };

                                                        let artwork_url = if (*edit_artwork_url).trim().is_empty() || *edit_artwork_url == current_artwork_url {
                                                            None
                                                        } else {
                                                            Some((*edit_artwork_url).clone())
                                                        };

                                                        let website_url = if (*edit_website_url).trim().is_empty() || *edit_website_url == current_website_url {
                                                            None
                                                        } else {
                                                            Some((*edit_website_url).clone())
                                                        };

                                                        let podcast_index_id = if (*edit_podcast_index_id).trim().is_empty() || *edit_podcast_index_id == current_podcast_index_id {
                                                            None
                                                        } else {
                                                            (*edit_podcast_index_id).parse::<i32>().ok()
                                                        };

                                                        // Check if any changes were made
                                                        if feed_url.is_none() && username.is_none() && password.is_none() &&
                                                           podcast_name.is_none() && description.is_none() && author.is_none() &&
                                                           artwork_url.is_none() && website_url.is_none() && podcast_index_id.is_none() {
                                                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                state.info_message = Some("No changes to save".to_string())
                                                            });
                                                            return;
                                                        }

                                                        match call_update_podcast_info(
                                                            &server_name.unwrap_or_default(),
                                                            &api_key.unwrap(),
                                                            user_id.unwrap_or_default(),
                                                            current_podcast_id,
                                                            feed_url,
                                                            username,
                                                            password,
                                                            podcast_name,
                                                            description,
                                                            author,
                                                            artwork_url,
                                                            website_url,
                                                            podcast_index_id,
                                                        ).await {
                                                            Ok(response) => {
                                                                if response.success {
                                                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                        state.info_message = Some("Podcast updated successfully".to_string())
                                                                    });
                                                                    page_state.set(PageState::Hidden);
                                                                } else {
                                                                    Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                        state.error_message = Some(format!("Update failed: {}", response.message))
                                                                    });
                                                                }
                                                            }
                                                            Err(e) => {
                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                    state.error_message = Some(format!("Error updating podcast: {}", e))
                                                                });
                                                            }
                                                        }
                                                    });
                                                }
                                            })
                                        }}
                                    >
                                        {&i18n.t("episodes_layout.save_changes")}
                                    </button>
                                    <button
                                        type="button"
                                        onclick={on_close_modal.clone()}
                                        class="clear-button bg-gray-300 hover:bg-gray-400 text-gray-800 font-bold py-2 px-4 rounded"
                                    >
                                        {&i18n.t("episodes_layout.cancel")}
                                    </button>
                                </div>
                            </form>
                        </div>
                    </div>
                </div>
            </div>
        }
    };

    // Define the callback functions
    let toggle_settings = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Shown);
        })
    };

    let toggle_download = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Download);
        })
    };

    let toggle_delete = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::Delete);
        })
    };

    let toggle_podcast = {
        let add_dispatch = _search_dispatch.clone();
        let pod_values = clicked_podcast_info.clone();
        let user_id_og = user_id.clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let user_id_clone = user_id.clone();
        let app_dispatch = _search_dispatch.clone();

        let is_added = is_added.clone();
        let added_id = podcast_id.clone();
        let is_subscribing_toggle = is_subscribing.clone();

        if *is_added == true {
            toggle_delete
        } else {
            Callback::from(move |_: MouseEvent| {
                let i18n_podcast_successfully_added = i18n_podcast_successfully_added.clone();
                let i18n_failed_to_add_podcast = i18n_failed_to_add_podcast.clone();
                let callback_podcast_id = added_id.clone();
                // Use podcastindexid, not podcastid: get_podcast_details_dynamic always
                // returns podcastid=0 for a podcast that hasn't been added yet (there's no
                // row in the Podcasts table for it), while podcastindexid carries the real
                // Podcast Index feed ID threaded through from the search result. Passing
                // podcastid here persists podcast_index_id=0 on add, so the podcast shows
                // up as unmatched on the Podcast Index Matching settings page even though
                // it was subscribed straight from a Podcast Index search result (#886).
                let podcast_id_og = pod_values.clone().unwrap().podcastindexid.clone();
                let pod_title_og = pod_values.clone().unwrap().podcastname.clone();
                let pod_artwork_og = pod_values.clone().unwrap().artworkurl.clone();
                let pod_author_og = pod_values.clone().unwrap().author.clone();
                let categories_og = pod_values.clone().unwrap().categories.unwrap().clone();
                let pod_description_og = pod_values.clone().unwrap().description.clone();
                let pod_episode_count_og = pod_values.clone().unwrap().episodecount.clone();
                let pod_feed_url_og = pod_values.clone().unwrap().feedurl.clone();
                let pod_website_og = pod_values.clone().unwrap().websiteurl.clone();
                let pod_explicit_og = pod_values.clone().unwrap().explicit.clone();
                let _app_dispatch = app_dispatch.clone();
                Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(true));
                let is_added_inner = is_added.clone();
                let is_subscribing_inner = is_subscribing_toggle.clone();
                is_subscribing_inner.set(true);
                let call_dispatch = add_dispatch.clone();
                let pod_title = pod_title_og.clone();
                let pod_artwork = pod_artwork_og.clone();
                let pod_author = pod_author_og.clone();
                let categories = categories_og.clone();
                let pod_description = pod_description_og.clone();
                let pod_episode_count = pod_episode_count_og.clone();
                let pod_feed_url = pod_feed_url_og.clone();
                let pod_website = pod_website_og.clone();
                let pod_explicit = pod_explicit_og.clone();
                let user_id = user_id_og.clone().unwrap();
                let podcast_values = PodcastValues {
                    pod_title,
                    pod_artwork,
                    pod_author,
                    categories,
                    pod_description,
                    pod_episode_count,
                    pod_feed_url,
                    pod_website,
                    pod_explicit,
                    user_id,
                };
                let api_key_call = api_key_clone.clone();
                let server_name_call = server_name_clone.clone();
                let user_id_call = user_id_clone.clone();
                let _ = call_dispatch; // will be dropped; dispatch happens via app_dispatch

                wasm_bindgen_futures::spawn_local(async move {
                    let api_key_wasm = api_key_call.clone().unwrap();
                    let user_id_wasm = user_id_call.clone().unwrap();
                    let server_name_wasm = server_name_call.clone();
                    let pod_values_clone = podcast_values.clone();

                    match call_add_podcast(
                        &server_name_wasm.clone().unwrap(),
                        &api_key_wasm,
                        user_id_wasm,
                        &pod_values_clone,
                        podcast_id_og,
                    )
                    .await
                    {
                        Ok(response_body) => {
                            if response_body.success {
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.info_message =
                                        Option::from(i18n_podcast_successfully_added)
                                });
                                is_added_inner.set(true);

                                let call_podcast_id = response_body.podcast_id;
                                callback_podcast_id.set(call_podcast_id);

                                let episode_id = Some(response_body.first_episode_id);
                                Dispatch::<EpisodeNavigationState>::global().reduce_mut(move |s| {
                                    s.selected_episode_id = episode_id;
                                });

                                // The backend ingests episodes asynchronously after returning success,
                                // so poll until the DB has episodes (up to ~30 seconds).
                                let server_for_poll = server_name_wasm.clone().unwrap_or_default();
                                const MAX_POLL_ATTEMPTS: u32 = 15;
                                const POLL_INTERVAL_MS: u32 = 2_000;
                                let mut episodes_loaded = false;

                                for attempt in 0..MAX_POLL_ATTEMPTS {
                                    match call_get_podcast_episodes(
                                        &server_for_poll,
                                        &api_key_wasm,
                                        &user_id_wasm,
                                        &call_podcast_id,
                                        Some(50),
                                        Some(0),
                                        None,
                                        None,
                                        None,
                                        None,
                                    )
                                    .await
                                    {
                                        Ok(result) if !result.episodes.is_empty() => {
                                            Dispatch::<SearchState>::global().reduce_mut(move |state| {
                                                state.podcast_feed_results = Some(result);
                                            });
                                            Dispatch::<PodcastFeedState>::global().reduce_mut(|state| {
                                                state.podcast_added = Some(true);
                                            });
                                            Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                                                state.is_loading = Some(false);
                                            });
                                            episodes_loaded = true;
                                            break;
                                        }
                                        Ok(_) => {
                                            // Episodes not ready yet — wait and retry
                                            if attempt + 1 < MAX_POLL_ATTEMPTS {
                                                TimeoutFuture::new(POLL_INTERVAL_MS).await;
                                            }
                                        }
                                        Err(e) => {
                                            web_sys::console::log_1(
                                                &format!("Error fetching episodes on attempt {}: {:?}", attempt, e).into(),
                                            );
                                            break;
                                        }
                                    }
                                }

                                if !episodes_loaded {
                                    // Polling exhausted or errored — mark as subscribed anyway
                                    Dispatch::<PodcastFeedState>::global().reduce_mut(|state| {
                                        state.podcast_added = Some(true);
                                    });
                                    Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                                        state.is_loading = Some(false);
                                    });
                                }
                                is_subscribing_inner.set(false);
                            } else {
                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.error_message = Option::from(i18n_failed_to_add_podcast)
                                });
                                Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(false));
                                is_subscribing_inner.set(false);
                            }
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Option::from(format!(
                                    "Error adding podcast: {:?}",
                                    formatted_error
                                ))
                            });
                            Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(false));
                            is_subscribing_inner.set(false);
                        }
                    }
                });
            })
        }
    };

    #[derive(Clone, PartialEq)]
    enum CompletedFilter {
        ShowAll,
        ShowOnly,
        Hide,
    }

    // Sort/search/filter run on the backend now — the effect below just keeps `filtered_episodes`
    // in sync with `podcast_feed_results.episodes`. Pre-existing consumers (select_older,
    // select_newer, EpisodeListView, etc.) read from `filtered_episodes`, so we maintain it
    // as a thin pass-through Rc instead of restructuring every call site.
    let filtered_episodes = use_state(|| Rc::new(Vec::<Episode>::new()));
    let processed_episode_count = use_state(|| 0usize);

    {
        let filtered_episodes = filtered_episodes.clone();
        let processed_episode_count = processed_episode_count.clone();
        use_effect_with(podcast_feed_results.clone(), move |pfr| {
            if let Some(results) = pfr.as_ref() {
                // Always replace — backend already filtered/sorted, so the order is canonical.
                filtered_episodes.set(Rc::new(results.episodes.clone()));
                processed_episode_count.set(results.episodes.len());
            } else {
                if !filtered_episodes.is_empty() {
                    filtered_episodes.set(Rc::new(Vec::new()));
                }
                if *processed_episode_count != 0 {
                    processed_episode_count.set(0);
                }
            }
            || ()
        });
    }

    // Map the frontend's EpisodeSortDirection enum to the backend's (sort_by, sort_order) pair.
    fn sort_to_params(dir: &Option<EpisodeSortDirection>) -> (&'static str, &'static str) {
        match dir {
            Some(EpisodeSortDirection::OldestFirst)   => ("date", "asc"),
            Some(EpisodeSortDirection::ShortestFirst) => ("duration", "asc"),
            Some(EpisodeSortDirection::LongestFirst)  => ("duration", "desc"),
            Some(EpisodeSortDirection::TitleAZ)       => ("title", "asc"),
            Some(EpisodeSortDirection::TitleZA)       => ("title", "desc"),
            _                                         => ("date", "desc"),
        }
    }
    // The frontend has 4 filter states (3-way completed × show_in_progress toggle); collapse
    // to the backend's "all" | "completed" | "incomplete" | "in_progress" vocabulary.
    fn completed_to_filter(c: &CompletedFilter, show_in_progress: bool) -> &'static str {
        if show_in_progress { return "in_progress"; }
        match c {
            CompletedFilter::ShowOnly => "completed",
            CompletedFilter::Hide     => "incomplete",
            CompletedFilter::ShowAll  => "all",
        }
    }

    // Debounce the raw search input: a 300 ms timer copies `episode_search_term` into
    // `debounced_search_term`, and only the latter feeds the backend-reload effect. Otherwise
    // every keystroke would fire its own HTTP request.
    {
        let debounced_search_term = debounced_search_term.clone();
        let term = (*episode_search_term).clone();
        use_effect_with(term.clone(), move |t| {
            let t = t.clone();
            let debounced = debounced_search_term.clone();
            let timeout = gloo_timers::callback::Timeout::new(300, move || {
                debounced.set(t);
            });
            // Cancel the pending timer if the user types again before it fires.
            move || drop(timeout)
        });
    }

    // Backend reload: fetch from offset 0 with current sort/search/filter whenever any of those
    // (or podcast_id / auth) changes. Gated on `prefs_loaded_for_podcast == Some(podcast_id)`
    // so we don't fire mid-mount with stale per-podcast prefs from the previous page.
    {
        let api_key_dep = api_key.clone();
        let user_id_dep = user_id;
        let server_name_dep = server_name.clone();
        let podcast_id_dep = *podcast_id;
        let sort_dep = (*episode_sort_direction).clone();
        let completed_dep = (*completed_filter_state).clone();
        let show_in_progress_dep = *show_in_progress;
        let search_dep = (*debounced_search_term).clone();
        let prefs_loaded_dep = *prefs_loaded_for_podcast;
        let synced_episode_count_eff = synced_episode_count.clone();

        use_effect_with(
            (
                api_key_dep,
                user_id_dep,
                server_name_dep,
                podcast_id_dep,
                sort_dep,
                completed_dep,
                show_in_progress_dep,
                search_dep,
                prefs_loaded_dep,
            ),
            move |(api_key, user_id, server_name, pid, sort, completed, show_ip, search, prefs_loaded)| {
                if *pid <= 0 || Some(*pid) != *prefs_loaded {
                    return Box::new(|| ()) as Box<dyn FnOnce()>;
                }
                let (Some(api_key_outer), Some(uid), Some(server_inner)) =
                    (api_key.clone(), *user_id, server_name.clone())
                else {
                    return Box::new(|| ()) as Box<dyn FnOnce()>;
                };
                let Some(api_key_str) = api_key_outer.as_deref().map(|s| s.to_string()) else {
                    return Box::new(|| ()) as Box<dyn FnOnce()>;
                };
                let (sort_by, sort_order) = sort_to_params(sort);
                let filter_str = completed_to_filter(completed, *show_ip);
                let search_str = search.clone();
                let pid_inner = *pid;
                let synced_episode_count_eff = synced_episode_count_eff.clone();
                spawn_local(async move {
                    match call_get_podcast_episodes(
                        &server_inner,
                        &Some(api_key_str),
                        &uid,
                        &pid_inner,
                        Some(50),
                        Some(0),
                        Some(sort_by),
                        Some(sort_order),
                        Some(&search_str),
                        Some(filter_str),
                    )
                    .await
                    {
                        Ok(result) => {
                            Dispatch::<SearchState>::global().reduce_mut(|state| {
                                state.podcast_feed_results = Some(result);
                            });
                            // Reset the status-sync counter so the saved/completed/queued sync
                            // effect rebuilds against the new result set rather than treating
                            // it as an append.
                            synced_episode_count_eff.set(0);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error reloading podcast episodes: {}", e).into(),
                            );
                        }
                    }
                });
                Box::new(|| ()) as Box<dyn FnOnce()>
            },
        );
    }

    // Sync completed/saved/queued status into EpisodeStatusState whenever episodes load.
    // On initial load (already_synced == 0) or podcast change (total < already_synced): full
    // replace. On append: only extend with new episodes so existing cards skip re-evaluation.
    {
        let podcast_feed_results = podcast_feed_results.clone();
        let synced_episode_count = synced_episode_count.clone();
        use_effect_with(podcast_feed_results, move |results| {
            if let Some(results) = results.as_ref() {
                let all_episodes = &results.episodes;
                let already_synced = *synced_episode_count;
                let total_now = all_episodes.len();

                // If total_now < already_synced the podcast changed — treat as initial.
                let is_initial = already_synced == 0 || total_now < already_synced;

                if total_now != already_synced {
                    let new_episodes: &[Episode] = if is_initial {
                        &all_episodes[..]
                    } else {
                        &all_episodes[already_synced..]
                    };

                    let new_completed: std::collections::HashSet<i32> = new_episodes
                        .iter()
                        .filter(|ep| ep.completed)
                        .map(|ep| ep.episodeid)
                        .collect();
                    let new_saved: Vec<Episode> = new_episodes
                        .iter()
                        .filter(|ep| ep.saved)
                        .cloned()
                        .collect();
                    let new_queued: Vec<i32> = new_episodes
                        .iter()
                        .filter(|ep| ep.queued)
                        .map(|ep| ep.episodeid)
                        .collect();

                    Dispatch::<EpisodeStatusState>::global().reduce_mut(move |state| {
                        if is_initial {
                            state.completed_episodes = new_completed;
                            state.saved_episodes = new_saved;
                            state.queued_episode_ids = Some(new_queued);
                        } else {
                            state.completed_episodes.extend(new_completed);
                            state.saved_episodes.extend(new_saved);
                            if let Some(ref mut q) = state.queued_episode_ids {
                                q.extend(new_queued);
                            } else {
                                state.queued_episode_ids = Some(new_queued);
                            }
                        }
                    });
                    synced_episode_count.set(total_now);
                }
            }
            || ()
        });
    }

    // Load-more handler: EpisodeListView owns the sentinel, observer, display_count, and
    // initial ramp. This callback only fires when the view has run out of locally-buffered
    // episodes AND backend_can_load_more is true. The two `TimeoutFuture::new(0).await` yields
    // let the spinner paint before the work and let new cards paint after, keeping the page
    // responsive during the fetch.
    //
    // SearchState and PodcastFeedState are read via `Dispatch::get()` inside the body so the
    // offset reflects the latest already-loaded count. The Rc<State> snapshots captured by the
    // outer closure would be stale on every fire after the first append — that was the
    // offset=0 bug in the first pass of this migration.
    let on_load_more = {
        let loading_more = loading_more.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let episode_sort_direction = episode_sort_direction.clone();
        let completed_filter_state = completed_filter_state.clone();
        let show_in_progress = show_in_progress.clone();
        let debounced_search_term = debounced_search_term.clone();
        use_callback((), move |_: (), _| {
            if *loading_more {
                return;
            }
            let search_state = Dispatch::<SearchState>::global().get();
            let podcast_state = Dispatch::<PodcastFeedState>::global().get();
            let loaded = search_state
                .podcast_feed_results
                .as_ref()
                .map(|r| r.episodes.len() as i64)
                .unwrap_or(0);
            let podcast_id_val = podcast_state
                .clicked_podcast_info
                .as_ref()
                .map(|i| i.podcastid)
                .unwrap_or(0);
            let Some(api_key_inner) = api_key.as_ref() else { return; };
            let Some(server_inner) = server_name.as_ref() else { return; };
            let Some(uid) = user_id else { return; };
            let Some(key_str) = api_key_inner.as_deref() else { return; };
            let key = key_str.to_string();
            let server = server_inner.clone();
            // Read sort/filter/search at fire time so a sort/filter change between renders
            // doesn't get clobbered by a stale snapshot from when the callback was created.
            let (sort_by, sort_order) = sort_to_params(&*episode_sort_direction);
            let filter_str = completed_to_filter(&*completed_filter_state, *show_in_progress);
            let search_str = (*debounced_search_term).clone();
            loading_more.set(true);
            let loading_more = loading_more.clone();
            spawn_local(async move {
                match call_get_podcast_episodes(
                    &server,
                    &Some(key),
                    &uid,
                    &podcast_id_val,
                    Some(50),
                    Some(loaded),
                    Some(sort_by),
                    Some(sort_order),
                    Some(&search_str),
                    Some(filter_str),
                )
                .await
                {
                    Ok(result) => {
                        TimeoutFuture::new(0).await;
                        Dispatch::<SearchState>::global().reduce_mut(|state| {
                            if let Some(ref mut existing) = state.podcast_feed_results {
                                existing.episodes.extend(result.episodes);
                                existing.total = result.total;
                            }
                        });
                        TimeoutFuture::new(0).await;
                    }
                    Err(e) => {
                        web_sys::console::log_1(
                            &format!("Error loading more episodes: {}", e).into(),
                        );
                    }
                }
                loading_more.set(false);
            });
        })
    };

    // Stable callbacks via use_callback with `()` deps: the Callback object is reused across
    // every parent render. Critical because EpisodeListItem props include these callbacks —
    // if they were fresh `Callback::from(...)` per render, every already-mounted card would
    // fail PartialEq and re-run its function body on every parent re-render (display_count
    // bump, backend append, anything). With use_callback the captured UseStateHandle deref
    // still gives the *current* state value when the callback fires, so behaviour is
    // unchanged — only the identity is stabilised.
    let on_episode_checkbox_change = {
        let selected_episodes = selected_episodes.clone();
        use_callback((), move |episode_id: i32, _| {
            let mut current = (*selected_episodes).clone();
            if current.contains(&episode_id) {
                current.remove(&episode_id);
            } else {
                current.insert(episode_id);
            }
            selected_episodes.set(current);
        })
    };

    let on_select_older = {
        let filtered_episodes = filtered_episodes.clone();
        let selected_episodes = selected_episodes.clone();
        use_callback((), move |cutoff_episode_id: i32, _| {
            let episodes = &*filtered_episodes;
            let cutoff_index = episodes
                .iter()
                .position(|ep| ep.episodeid == cutoff_episode_id)
                .unwrap_or(0);
            let older_ids: HashSet<i32> = episodes
                .iter()
                .skip(cutoff_index)
                .map(|ep| ep.episodeid)
                .collect();
            let mut current = (*selected_episodes).clone();
            current.extend(older_ids);
            selected_episodes.set(current);
        })
    };

    let on_select_newer = {
        let filtered_episodes = filtered_episodes.clone();
        let selected_episodes = selected_episodes.clone();
        use_callback((), move |cutoff_episode_id: i32, _| {
            let episodes = &*filtered_episodes;
            let cutoff_index = episodes
                .iter()
                .position(|ep| ep.episodeid == cutoff_episode_id)
                .unwrap_or(0);
            let newer_ids: HashSet<i32> = episodes
                .iter()
                .take(cutoff_index + 1)
                .map(|ep| ep.episodeid)
                .collect();
            let mut current = (*selected_episodes).clone();
            current.extend(newer_ids);
            selected_episodes.set(current);
        })
    };

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggle_description(guid: &str);
    }

    let web_link = open_in_new_tab.clone();
    let pod_layout_data = clicked_podcast_info.clone();

    let (completed_icon, completed_text, completed_title) = match *completed_filter_state {
        CompletedFilter::ShowOnly => (
            "ph-check-circle",
            &i18n_show_only,
            &i18n_showing_only_completed,
        ),
        CompletedFilter::Hide => ("ph-x-circle", &i18n_hide, &i18n_hiding_completed),
        CompletedFilter::ShowAll => ("ph-circle", &i18n_all, &i18n_showing_all_episodes),
    };

    html! {
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                {
                    match *page_state {
                    PageState::Shown => podcast_option_model,
                    PageState::Download => download_all_model,
                    PageState::Delete => delete_pod_model,
                    PageState::RSSFeed => rss_feed_modal,
                    PageState::EditPodcast => edit_podcast_modal,
                    _ => html! {},
                    }
                }
                {
                    if *loading { // If loading is true, display the loading animation
                        html! { <Loading/> }
                    } else {
                        html! {
                            <>
                                {
                                    if let Some(podcast_info) = pod_layout_data {
                                        let sanitized_title = podcast_info.podcastname.replace(|c: char| !c.is_alphanumeric(), "-");
                                        let desc_id = format!("desc-{}", sanitized_title);
                                        let pod_link = podcast_info.websiteurl.clone();

                                        let toggle_description = {
                                            let desc_id = desc_id.clone();
                                            Callback::from(move |_| {
                                                let desc_id = desc_id.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    let window = web_sys::window().expect("no global `window` exists");
                                                    let function = window
                                                        .get("toggle_description")
                                                        .expect("should have `toggle_description` as a function")
                                                        .dyn_into::<js_sys::Function>()
                                                        .unwrap();
                                                    let this = JsValue::NULL;
                                                    let guid = JsValue::from_str(&desc_id);
                                                    function.call1(&this, &guid).unwrap();
                                                });
                                            })
                                        };
                                        let sanitized_description = sanitize_html(&podcast_info.description);
                                        let is_video_podcast = search_data.podcast_feed_results
                                            .as_ref()
                                            .map(|r| r.episodes.iter().any(|e| e.is_video))
                                            .unwrap_or(false);
                                        let layout = if state.is_mobile.unwrap_or(false) {
                                            let server_for_proxy = server_name.clone().unwrap_or_default();
                                            html! {
                                                <div class="mobile-layout ep-mobile-pod-header">

                                                    // ── Compact header: artwork + title + actions ──
                                                    <div class="ep-mobile-header">
                                                        <FallbackImage
                                                            src={podcast_info.artworkurl.clone()}
                                                            alt={format!("Cover for {}", &podcast_info.podcastname)}
                                                            class="ep-mobile-art rounded-corners"
                                                        />
                                                        <div class="ep-mobile-meta">
                                                            <div class="flex items-center gap-2">
                                                                <h2 class="ep-mobile-title">{ &podcast_info.podcastname }</h2>
                                                                { if is_video_podcast {
                                                                    html! {
                                                                        <span class="inline-flex items-center gap-1 bg-opacity-80 bg-gray-700 text-white text-xs px-1.5 py-0.5 rounded-full">
                                                                            <i class="ph ph-television"></i>
                                                                            { &i18n_video }
                                                                        </span>
                                                                    }
                                                                } else { html! {} }}
                                                            </div>
                                                            <p class="ep-mobile-podcast-name">{ &podcast_info.author }</p>
                                                            <p class="ep-mobile-subinfo">
                                                                { format!("{}{} · {}{}",
                                                                    i18n_episode_count,
                                                                    &podcast_info.episodecount,
                                                                    i18n_explicit,
                                                                    if podcast_info.explicit { i18n_yes.clone() } else { i18n_no.clone() }
                                                                )}
                                                            </p>
                                                        </div>
                                                    </div>

                                                    // ── Icon action bar ──
                                                    <div class={if podcast_state.podcast_added.unwrap_or(false) { "ep-mobile-actions" } else { "ep-mobile-actions ep-mobile-actions-sparse" }}>
                                                        // Subscribe / unsubscribe — always shown
                                                        <button onclick={toggle_podcast} title="Add or remove podcast" class="ep-mobile-action-btn ep-mobile-action-play">
                                                            { button_content }
                                                        </button>
                                                        // Website link — always shown
                                                        <button
                                                            onclick={
                                                                let pod_link = pod_link.clone();
                                                                Callback::from(move |_| web_link.emit(pod_link.clone()))
                                                            }
                                                            title="Visit podcast website"
                                                            class="ep-mobile-action-btn"
                                                        >
                                                            { website_icon }
                                                        </button>
                                                        // Subscribed-only actions
                                                        { if podcast_state.podcast_added.unwrap_or(false) {
                                                            html! {
                                                                <>
                                                                <button onclick={toggle_download} title="Download all episodes" class="ep-mobile-action-btn">
                                                                    { download_all }
                                                                </button>
                                                                <button
                                                                    onclick={
                                                                        let page_state = page_state.clone();
                                                                        Callback::from(move |_| page_state.set(PageState::RSSFeed))
                                                                    }
                                                                    title="Get RSS Feed URL"
                                                                    class="ep-mobile-action-btn"
                                                                >
                                                                    { rss_icon }
                                                                </button>
                                                                <button onclick={toggle_settings} title="Podcast settings" class="ep-mobile-action-btn">
                                                                    { setting_content }
                                                                </button>
                                                                </>
                                                            }
                                                        } else { html! {} }}
                                                        // Funding buttons (if any)
                                                        { if let Some(funding_list) = &state.podcast_funding {
                                                            if !funding_list.is_empty() {
                                                                let funding_list_clone = funding_list.clone();
                                                                html! {
                                                                    <>
                                                                    { for funding_list_clone.iter().map(|funding| {
                                                                        let open_in_new_tab = open_in_new_tab.clone();
                                                                        let payment_icon = payment_icon.clone();
                                                                        let url = funding.url.clone();
                                                                        html! {
                                                                            <button
                                                                                onclick={Callback::from(move |_| open_in_new_tab.emit(url.clone()))}
                                                                                title={funding.description.clone()}
                                                                                class="ep-mobile-action-btn"
                                                                            >
                                                                                { payment_icon }
                                                                            </button>
                                                                        }
                                                                    })}
                                                                    </>
                                                                }
                                                            } else { html! {} }
                                                        } else { html! {} }}
                                                    </div>

                                                    // ── Description (collapsed) ──
                                                    <div class="ep-mobile-desc-section">
                                                        <div class="item-header-description desc-collapsed" id={desc_id.clone()} onclick={toggle_description.clone()}>
                                                            { sanitized_description }
                                                            <button class="toggle-desc-btn" onclick={toggle_description}>{ "" }</button>
                                                        </div>
                                                    </div>

                                                    // ── Host strip or unmatched warning ──
                                                    { if !podcast_info.is_youtube.unwrap_or(false) {
                                                        if podcast_info.podcastindexid == 0 {
                                                            html! {
                                                                <div class="ep-mobile-hosts">
                                                                    <div class="import-box">
                                                                        <p class="item_container-text text-sm">
                                                                            {"⚠️ This podcast isn't matched to Podcast Index. "}
                                                                            <a href="/settings#podcast-index-matching" class="item_container-text underline hover:opacity-80 font-semibold">
                                                                                {&i18n.t("episodes_layout.match_it_here")}
                                                                            </a>
                                                                            {" to enable host and guest information."}
                                                                        </p>
                                                                    </div>
                                                                </div>
                                                            }
                                                        } else if let Some(people) = &state.podcast_people {
                                                            if !people.is_empty() {
                                                                let has_unknown_host = people.len() == 1
                                                                    && people[0].name == "Unknown Host"
                                                                    && people[0].role == Some("Host".to_string());
                                                                if has_unknown_host {
                                                                    let people_url = search_state.server_details.as_ref()
                                                                        .and_then(|sd| sd.people_url.as_ref())
                                                                        .cloned()
                                                                        .unwrap_or_default();
                                                                    let host_url = format!("{}/podcast/{}", people_url, podcast_info.podcastindexid);
                                                                    html! {
                                                                        <div class="ep-mobile-hosts">
                                                                            <p class="ep-mobile-section-label">{ &i18n_hosts }</p>
                                                                            <p class="ep-mobile-no-hosts-msg">
                                                                                { i18n.t("host_component.no_hosts_found") }
                                                                                <a href={host_url} target="_blank" class="ep-mobile-no-hosts-link">
                                                                                    { i18n.t("host_component.add_hosts_here") }
                                                                                </a>
                                                                            </p>
                                                                        </div>
                                                                    }
                                                                } else {
                                                                    let server = server_for_proxy.clone();
                                                                    html! {
                                                                        <div class="ep-mobile-hosts">
                                                                            <p class="ep-mobile-section-label">{ &i18n_hosts }</p>
                                                                            <div class="ep-mobile-hosts-scroll">
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

                                                    // ── Categories ──
                                                    { if let Some(categories) = &podcast_info.categories {
                                                        if !categories.is_empty() {
                                                            html! {
                                                                <div class="ep-mobile-categories">
                                                                    { for categories.iter().map(|(_, category_name)| {
                                                                        html! { <span class="category-box">{ category_name }</span> }
                                                                    })}
                                                                </div>
                                                            }
                                                        } else { html! {} }
                                                    } else { html! {} }}

                                                </div>
                                            }
                                        } else {
                                            let pod_link = podcast_info.feedurl.clone();
                                            let server = server_name.clone().unwrap_or_default();
                                            let subscribed = podcast_state.podcast_added.unwrap_or(false);
                                            let backdrop_style = format!("background-image: url('{}');", &podcast_info.artworkurl);
                                            let eyebrow_text = match podcast_info.categories.as_ref().and_then(|c| c.values().next()) {
                                                Some(cat) => format!("{} · {}", i18n.t("episodes_layout.podcast"), cat),
                                                None => i18n.t("episodes_layout.podcast").to_string(),
                                            };
                                            let sub_label = if *is_subscribing {
                                                i18n.t("episodes_layout.subscribing").to_string()
                                            } else if subscribed {
                                                i18n.t("episodes_layout.subscribed").to_string()
                                            } else {
                                                i18n.t("episodes_layout.subscribe").to_string()
                                            };
                                            let primary_class = if subscribed { "pd-btn-primary is-subscribed" } else { "pd-btn-primary" };
                                            html! {
                                                <div class="pd-banner">
                                                    <div class="pd-banner-backdrop" style={backdrop_style}></div>
                                                    <div class="pd-banner-scrim"></div>
                                                    <div class="pd-banner-inner">
                                                        <div class="pd-banner-top">
                                                            <FallbackImage
                                                                src={podcast_info.artworkurl.clone()}
                                                                alt={format!("Cover for {}", &podcast_info.podcastname)}
                                                                class={"pd-banner-cover"}
                                                            />
                                                            <div class="pd-banner-head">
                                                                <div class="pd-eyebrow">
                                                                    <i class="ph ph-microphone-stage"></i>
                                                                    <span>{ eyebrow_text }</span>
                                                                </div>
                                                                <div class="pd-banner-title-row">
                                                                    <h2 class="pd-banner-title">{ &podcast_info.podcastname }</h2>
                                                                    { if is_video_podcast {
                                                                        html! {
                                                                            <span class="inline-flex items-center gap-1 bg-opacity-80 bg-gray-700 text-white text-xs px-2 py-1 rounded-full self-center">
                                                                                <i class="ph ph-television"></i>
                                                                                { &i18n_video }
                                                                            </span>
                                                                        }
                                                                    } else { html! {} }}
                                                                </div>
                                                                <div class="pd-banner-meta">
                                                                    <span class="pd-author">{ &i18n_authors }<strong>{ &podcast_info.author }</strong></span>
                                                                    <span class="pd-dot">{"•"}</span>
                                                                    <span class="pd-stat"><i class="ph ph-stack"></i>{ format!("{}{}", i18n_episode_count, &podcast_info.episodecount) }</span>
                                                                    <span class="pd-dot">{"•"}</span>
                                                                    <span class="pd-stat">{ format!("{}{}", i18n_explicit, if podcast_info.explicit { i18n_yes.clone() } else { i18n_no.clone() }) }</span>
                                                                </div>
                                                                <div class="pd-banner-actions">
                                                                    <button onclick={toggle_podcast} title={i18n.t("episodes_layout.add_remove_podcast").to_string()} class={primary_class}>
                                                                        { button_content }
                                                                        <span>{ sub_label }</span>
                                                                    </button>
                                                                    { if subscribed {
                                                                        html! {
                                                                            <button onclick={toggle_download} title={i18n.t("episodes_layout.download_all_episodes").to_string()} class="pd-icon-btn">
                                                                                { download_all }
                                                                            </button>
                                                                        }
                                                                    } else { html! {} } }
                                                                    <button
                                                                        onclick={Callback::from(move |_| web_link.clone().emit(pod_link.to_string()))}
                                                                        title="Visit external podcast website" class="pd-icon-btn">
                                                                        { website_icon }
                                                                    </button>
                                                                    { if subscribed {
                                                                        html! {
                                                                            <button onclick={toggle_settings} title={i18n.t("episodes_layout.podcast_specific_settings").to_string()} class="pd-icon-btn">
                                                                                { setting_content }
                                                                            </button>
                                                                        }
                                                                    } else { html! {} } }
                                                                    { if subscribed {
                                                                        html! {
                                                                            <button
                                                                                onclick={
                                                                                    let page_state = page_state.clone();
                                                                                    Callback::from(move |_| { page_state.set(PageState::RSSFeed); })
                                                                                }
                                                                                title="Get RSS Feed URL"
                                                                                class="pd-icon-btn">
                                                                                { rss_icon }
                                                                            </button>
                                                                        }
                                                                    } else { html! {} } }
                                                                    { if let Some(funding_list) = &state.podcast_funding {
                                                                        if !funding_list.is_empty() {
                                                                            let funding_list_clone = funding_list.clone();
                                                                            html! {
                                                                                <>
                                                                                { for funding_list_clone.iter().map(|funding| {
                                                                                    let open_in_new_tab = open_in_new_tab.clone();
                                                                                    let payment_icon = payment_icon.clone();
                                                                                    let url = funding.url.clone();
                                                                                    html! {
                                                                                        <button
                                                                                            onclick={Callback::from(move |_| open_in_new_tab.emit(url.clone()))}
                                                                                            title={funding.description.clone()}
                                                                                            class="pd-icon-btn"
                                                                                        >
                                                                                            { payment_icon }
                                                                                        </button>
                                                                                    }
                                                                                })}
                                                                                </>
                                                                            }
                                                                        } else { html! {} }
                                                                    } else { html! {} } }
                                                                </div>
                                                            </div>
                                                        </div>

                                                        <div class="item-header-description desc-collapsed pd-banner-desc" id={desc_id.clone()} onclick={toggle_description.clone()}>
                                                            { sanitized_description }
                                                            <button class="toggle-desc-btn" onclick={toggle_description}>{ "" }</button>
                                                        </div>

                                                        { if !podcast_info.is_youtube.unwrap_or(false) {
                                                            if podcast_info.podcastindexid == 0 {
                                                                html! {
                                                                    <div class="pd-banner-warning">
                                                                        <div class="import-box">
                                                                            <p class="item_container-text text-sm">
                                                                                {"⚠️ This podcast isn't matched to Podcast Index. "}
                                                                                <a href="/settings#podcast-index-matching" class="item_container-text underline hover:opacity-80 font-semibold">
                                                                                    {&i18n.t("episodes_layout.match_it_here")}
                                                                                </a>
                                                                                {" to enable host and guest information."}
                                                                            </p>
                                                                        </div>
                                                                    </div>
                                                                }
                                                            } else if let Some(people) = &state.podcast_people {
                                                                if !people.is_empty() {
                                                                    let has_unknown_host = people.len() == 1
                                                                        && people[0].name == "Unknown Host"
                                                                        && people[0].role == Some("Host".to_string());
                                                                    if has_unknown_host {
                                                                        let people_url = search_state.server_details.as_ref()
                                                                            .and_then(|sd| sd.people_url.as_ref())
                                                                            .cloned()
                                                                            .unwrap_or_default();
                                                                        let host_url = format!("{}/podcast/{}", people_url, podcast_info.podcastindexid);
                                                                        html! {
                                                                            <div class="pd-banner-nohosts">
                                                                                <span class="pd-host-by">{ &i18n_hosts }</span>
                                                                                <p class="pd-banner-nohosts-msg">
                                                                                    { i18n.t("host_component.no_hosts_found") }
                                                                                    <a href={host_url} target="_blank" class="pd-banner-nohosts-link">
                                                                                        { i18n.t("host_component.add_hosts_here") }
                                                                                    </a>
                                                                                </p>
                                                                            </div>
                                                                        }
                                                                    } else {
                                                                        let names: Vec<String> = people.iter().map(|p| p.name.clone()).collect();
                                                                        let names_joined = match names.len() {
                                                                            0 => String::new(),
                                                                            1 => names[0].clone(),
                                                                            2 => format!("{} & {}", names[0], names[1]),
                                                                            _ => {
                                                                                let (last, rest) = names.split_last().unwrap();
                                                                                format!("{} & {}", rest.join(", "), last)
                                                                            }
                                                                        };
                                                                        let server2 = server.clone();
                                                                        html! {
                                                                            <div class="pd-banner-hosts">
                                                                                <div class="pd-host-stack">
                                                                                    { for people.iter().take(6).map(|person| {
                                                                                        let name = person.name.clone();
                                                                                        let img_url = person.img.as_ref().map(|url| {
                                                                                            format!("{}/api/proxy/image?url={}", server2, urlencoding::encode(url))
                                                                                        });
                                                                                        let hist = history.clone();
                                                                                        let nav_name = name.clone();
                                                                                        let on_chip_click = Callback::from(move |_: MouseEvent| {
                                                                                            hist.push(format!("/person/{}", nav_name));
                                                                                        });
                                                                                        html! {
                                                                                            <div class="pd-host-avatar-wrap" title={name.clone()} onclick={on_chip_click}>
                                                                                                { if let Some(src) = img_url {
                                                                                                    html! { <img src={src} alt={name.clone()} class="pd-host-avatar" /> }
                                                                                                } else {
                                                                                                    html! { <div class="pd-host-avatar pd-host-avatar-ph"><i class="ph ph-user"></i></div> }
                                                                                                }}
                                                                                            </div>
                                                                                        }
                                                                                    })}
                                                                                </div>
                                                                                <div class="pd-host-label">
                                                                                    <span class="pd-host-by">{ i18n.t("episodes_layout.hosted_by") }</span>
                                                                                    <span class="pd-host-names">{ names_joined }</span>
                                                                                </div>
                                                                            </div>
                                                                        }
                                                                    }
                                                                } else { html! {} }
                                                            } else { html! {} }
                                                        } else { html! {} } }

                                                        { if let Some(categories) = &podcast_info.categories {
                                                            if !categories.is_empty() {
                                                                html! {
                                                                    <div class="pd-banner-cats">
                                                                        { for categories.values().map(|category_name| {
                                                                            html! { <span class="category-box">{ category_name }</span> }
                                                                        }) }
                                                                    </div>
                                                                }
                                                            } else { html! {} }
                                                        } else { html! {} } }
                                                    </div>
                                                </div>
                                            }
                                        };

                                        layout
                                    } else {
                                        html! {}
                                    }
                                }
                                {
                                    // Modern mobile-friendly filter bar
                                    html! {
                                        <div class="pfb-section">
                                            <div class="pfb-bar">
                                                <div class="sp-input">
                                                    <i class="ph ph-headphones sp-search-ico"></i>
                                                    <input
                                                        type="text"
                                                        placeholder={i18n_search_episodes_placeholder.clone()}
                                                        value={(*episode_search_term).clone()}
                                                        oninput={
                                                            let episode_search_term = episode_search_term.clone();
                                                            Callback::from(move |e: InputEvent| {
                                                                if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                                    episode_search_term.set(input.value());
                                                                }
                                                            })
                                                        }
                                                    />
                                                </div>
                                                <div class="pfb-sort">
                                                    <select
                                                        class="pfb-sort-select"
                                                        onchange={
                                                            let episode_sort_direction = episode_sort_direction.clone();
                                                            let podcast_id_clone = podcast_id.clone();
                                                            Callback::from(move |e: Event| {
                                                                let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                                let value = target.value();

                                                                if *podcast_id_clone > 0 {
                                                                    let preference_key = format!("podcast_{}", *podcast_id_clone);
                                                                    set_filter_preference(&preference_key, &value);
                                                                }

                                                                match value.as_str() {
                                                                    "newest" => episode_sort_direction.set(Some(EpisodeSortDirection::NewestFirst)),
                                                                    "oldest" => episode_sort_direction.set(Some(EpisodeSortDirection::OldestFirst)),
                                                                    "shortest" => episode_sort_direction.set(Some(EpisodeSortDirection::ShortestFirst)),
                                                                    "longest" => episode_sort_direction.set(Some(EpisodeSortDirection::LongestFirst)),
                                                                    "title_az" => episode_sort_direction.set(Some(EpisodeSortDirection::TitleAZ)),
                                                                    "title_za" => episode_sort_direction.set(Some(EpisodeSortDirection::TitleZA)),
                                                                    _ => episode_sort_direction.set(None),
                                                                }
                                                            })
                                                        }
                                                    >
                                                        <option value="newest" selected={
                                                            let preference_key = if *podcast_id > 0 { format!("podcast_{}", *podcast_id) } else { "episodes".to_string() };
                                                            get_filter_preference(&preference_key).unwrap_or_else(|| get_default_sort_direction().to_string()) == "newest"
                                                        }>{&i18n_newest_first}</option>
                                                        <option value="oldest" selected={
                                                            let preference_key = if *podcast_id > 0 { format!("podcast_{}", *podcast_id) } else { "episodes".to_string() };
                                                            get_filter_preference(&preference_key).unwrap_or_else(|| get_default_sort_direction().to_string()) == "oldest"
                                                        }>{&i18n_oldest_first}</option>
                                                        <option value="shortest" selected={
                                                            let preference_key = if *podcast_id > 0 { format!("podcast_{}", *podcast_id) } else { "episodes".to_string() };
                                                            get_filter_preference(&preference_key).unwrap_or_else(|| get_default_sort_direction().to_string()) == "shortest"
                                                        }>{&i18n_shortest_first}</option>
                                                        <option value="longest" selected={
                                                            let preference_key = if *podcast_id > 0 { format!("podcast_{}", *podcast_id) } else { "episodes".to_string() };
                                                            get_filter_preference(&preference_key).unwrap_or_else(|| get_default_sort_direction().to_string()) == "longest"
                                                        }>{&i18n_longest_first}</option>
                                                        <option value="title_az" selected={
                                                            let preference_key = if *podcast_id > 0 { format!("podcast_{}", *podcast_id) } else { "episodes".to_string() };
                                                            get_filter_preference(&preference_key).unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_az"
                                                        }>{&i18n_title_az}</option>
                                                        <option value="title_za" selected={
                                                            let preference_key = if *podcast_id > 0 { format!("podcast_{}", *podcast_id) } else { "episodes".to_string() };
                                                            get_filter_preference(&preference_key).unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_za"
                                                        }>{&i18n_title_za}</option>
                                                    </select>
                                                    <i class="ph ph-caret-down pfb-sort-arrow"></i>
                                                </div>
                                            </div>
                                            <div class="sp-chips pfb-chips">
                                                <button
                                                    onclick={
                                                        let show_in_progress = show_in_progress.clone();
                                                        let episode_search_term = episode_search_term.clone();
                                                        let completed_filter_state = completed_filter_state.clone();
                                                        let podcast_id_clone = podcast_id.clone();
                                                        Callback::from(move |_| {
                                                            completed_filter_state.set(CompletedFilter::ShowAll);
                                                            show_in_progress.set(false);
                                                            episode_search_term.set(String::new());
                                                            if *podcast_id_clone > 0 {
                                                                let completed_key = format!("podcast_{}_completed_filter", *podcast_id_clone);
                                                                set_filter_preference(&completed_key, "show_all");
                                                            }
                                                        })
                                                    }
                                                    class="sp-chip"
                                                >
                                                    <i class="ph ph-broom"></i>
                                                    <span>{&i18n_clear_all}</span>
                                                </button>
                                                <button
                                                    onclick={
                                                        let completed_filter_state = completed_filter_state.clone();
                                                        let podcast_id_clone = podcast_id.clone();
                                                        Callback::from(move |_| {
                                                            let new_filter = match *completed_filter_state {
                                                                CompletedFilter::ShowAll => CompletedFilter::ShowOnly,
                                                                CompletedFilter::ShowOnly => CompletedFilter::Hide,
                                                                CompletedFilter::Hide => CompletedFilter::ShowAll,
                                                            };
                                                            if *podcast_id_clone > 0 {
                                                                let completed_key = format!("podcast_{}_completed_filter", *podcast_id_clone);
                                                                let value = match new_filter {
                                                                    CompletedFilter::ShowOnly => "show_only",
                                                                    CompletedFilter::Hide => "hide",
                                                                    CompletedFilter::ShowAll => "show_all",
                                                                };
                                                                set_filter_preference(&completed_key, value);
                                                            }
                                                            completed_filter_state.set(new_filter);
                                                        })
                                                    }
                                                    title={completed_title.clone()}
                                                    class={classes!(
                                                        "sp-chip",
                                                        match *completed_filter_state {
                                                            CompletedFilter::ShowOnly => "is-active",
                                                            CompletedFilter::Hide => "is-alert",
                                                            CompletedFilter::ShowAll => ""
                                                        }
                                                    )}
                                                >
                                                    <i class={classes!("ph", completed_icon)}></i>
                                                    <span>{completed_text}</span>
                                                </button>
                                                <button
                                                    onclick={
                                                        let show_in_progress = show_in_progress.clone();
                                                        Callback::from(move |_| {
                                                            show_in_progress.set(!*show_in_progress);
                                                        })
                                                    }
                                                    class={classes!("sp-chip", if *show_in_progress { "is-active" } else { "" })}
                                                >
                                                    <i class="ph ph-hourglass-medium"></i>
                                                    <span>{&i18n_in_progress}</span>
                                                </button>
                                                <button
                                                    onclick={
                                                        let is_selecting = is_selecting.clone();
                                                        let selected_episodes = selected_episodes.clone();
                                                        Callback::from(move |_| {
                                                            if *is_selecting {
                                                                selected_episodes.set(HashSet::new());
                                                            }
                                                            is_selecting.set(!*is_selecting);
                                                        })
                                                    }
                                                    class={classes!("sp-chip", if *is_selecting { "is-active" } else { "" })}
                                                >
                                                    <i class="ph ph-check-square"></i>
                                                    <span>{if *is_selecting { &i18n_exit_select } else { &i18n_select }}</span>
                                                </button>
                                            </div>

                                            // Smart selection buttons when in selection mode
                                            {
                                                if *is_selecting {
                                                    let filtered_episodes_clone = filtered_episodes.clone();
                                                    let selected_episodes_clone = selected_episodes.clone();

                                                    html! {
                                                        <div class="flex gap-2 mt-4 flex-wrap">
                                                            // Select All / Deselect All
    <button
        onclick={
            let filtered_episodes = filtered_episodes_clone.clone();
            let selected_episodes = selected_episodes_clone.clone();
            Callback::from(move |_| {
                let all_ids: HashSet<i32> = filtered_episodes.iter()
                    .map(|ep| ep.episodeid)
                    .collect();

                let current = (*selected_episodes).clone();
                if current.len() == all_ids.len() && all_ids.iter().all(|id| current.contains(id)) {
                    // Deselect all
                    selected_episodes.set(HashSet::new());
                } else {
                    // Select all
                    selected_episodes.set(all_ids);
                }
            })
        }
        class="bulk-select-button"
    >
        {
            {
                // this extra block is an expression, so valid
                let all_ids: HashSet<i32> = filtered_episodes_clone.iter()
                    .map(|ep| ep.episodeid)
                    .collect();
                let current = (*selected_episodes_clone).clone();
                if current.len() == all_ids.len() && all_ids.iter().all(|id| current.contains(id)) {
                    &i18n_deselect_all
                } else {
                    &i18n_select_all
                }
            }
        }
    </button>


                                                            // Select Unplayed Only
                                                            <button
                                                                onclick={
                                                                    let filtered_episodes = filtered_episodes_clone.clone();
                                                                    let selected_episodes = selected_episodes_clone.clone();
                                                                    Callback::from(move |_| {
                                                                        let unplayed_ids: HashSet<i32> = filtered_episodes.iter()
                                                                            .filter(|ep| !ep.completed)
                                                                            .map(|ep| ep.episodeid)
                                                                            .collect();
                                                                        selected_episodes.set(unplayed_ids);
                                                                    })
                                                                }
                                                                class="bulk-filter-button"
                                                            >
                                                                {&i18n_select_unplayed}
                                                            </button>

                                                            // Select In Progress Only
                                                            <button
                                                                onclick={
                                                                    let filtered_episodes = filtered_episodes_clone.clone();
                                                                    let selected_episodes = selected_episodes_clone.clone();
                                                                    Callback::from(move |_| {
                                                                        let in_progress_ids: HashSet<i32> = filtered_episodes.iter()
                                                                            .filter(|ep| !ep.completed && ep.listenduration > 0)
                                                                            .map(|ep| ep.episodeid)
                                                                            .collect();
                                                                        selected_episodes.set(in_progress_ids);
                                                                    })
                                                                }
                                                                class="bulk-filter-button"
                                                            >
                                                                {&i18n_select_in_progress}
                                                            </button>
                                                        </div>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                    }
                                }


                                // Bulk action toolbar
                                {
                                    if *is_selecting && !selected_episodes.is_empty() {
                                        let selected_count = selected_episodes.len();
                                        let selected_ids: Vec<i32> = selected_episodes.iter().cloned().collect();
                                        let user_id_value = user_id.unwrap_or(0);

                                        html! {
                                            <div class="bulk-actions-bar">
                                                <div class="bulk-actions-bar__count">
                                                    <i class="ph ph-check-circle"></i>
                                                    {format!("{} episode{} selected", selected_count, if selected_count == 1 { "" } else { "s" })}
                                                </div>
                                                <div class="bulk-actions-bar__actions">
                                                        // Mark Complete button
                                                        <button
                                                            onclick={
                                                                let selected_ids = selected_ids.clone();
                                                                let api_key = api_key.clone();
                                                                let server_name = server_name.clone();
                                                                let dispatch = _search_dispatch.clone();
                                                                let selected_episodes = selected_episodes.clone();
                                                                Callback::from(move |_| {
                                                                    let selected_ids = selected_ids.clone();
                                                                    let api_key = api_key.clone();
                                                                    let server_name = server_name.clone();
                                                                    let _dispatch = dispatch.clone();
                                                                    let selected_episodes = selected_episodes.clone();
                                                                    spawn_local(async move {
                                                                        let request = BulkEpisodeActionRequest {
                                                                            episode_ids: selected_ids,
                                                                            user_id: user_id_value,
                                                                            is_youtube: None,
                                                                        };
                                                                        match call_bulk_mark_episodes_completed(
                                                                            &server_name.unwrap_or_default(),
                                                                            &api_key.flatten(),
                                                                            &request
                                                                        ).await {
                                                                            Ok(message) => {
                                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                    state.info_message = Some(message);
                                                                                });
                                                                                selected_episodes.set(HashSet::new());
                                                                            }
                                                                            Err(e) => {
                                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                    state.error_message = Some(format!("Error: {}", e));
                                                                                });
                                                                            }
                                                                        }
                                                                    });
                                                                })
                                                            }
                                                            class="btn btn-secondary"
                                                        >
                                                            <i class="ph ph-check-circle"></i>
                                                            {&i18n_mark_complete}
                                                        </button>

                                                        // Save button
                                                        <button
                                                            onclick={
                                                                let selected_ids = selected_ids.clone();
                                                                let api_key = api_key.clone();
                                                                let server_name = server_name.clone();
                                                                let dispatch = _search_dispatch.clone();
                                                                let selected_episodes = selected_episodes.clone();
                                                                Callback::from(move |_| {
                                                                    let selected_ids = selected_ids.clone();
                                                                    let api_key = api_key.clone();
                                                                    let server_name = server_name.clone();
                                                                    let _dispatch = dispatch.clone();
                                                                    let selected_episodes = selected_episodes.clone();
                                                                    spawn_local(async move {
                                                                        let request = BulkEpisodeActionRequest {
                                                                            episode_ids: selected_ids,
                                                                            user_id: user_id_value,
                                                                            is_youtube: None,
                                                                        };
                                                                        match call_bulk_save_episodes(
                                                                            &server_name.unwrap_or_default(),
                                                                            &api_key.flatten(),
                                                                            &request
                                                                        ).await {
                                                                            Ok(message) => {
                                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                    state.info_message = Some(message);
                                                                                });
                                                                                selected_episodes.set(HashSet::new());
                                                                            }
                                                                            Err(e) => {
                                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                    state.error_message = Some(format!("Error: {}", e));
                                                                                });
                                                                            }
                                                                        }
                                                                    });
                                                                })
                                                            }
                                                            class="btn btn-secondary"
                                                        >
                                                            <i class="ph ph-star"></i>
                                                            {&i18n.t("episodes_layout.save")}
                                                        </button>

                                                        // Queue button
                                                        <button
                                                            onclick={
                                                                let selected_ids = selected_ids.clone();
                                                                let api_key = api_key.clone();
                                                                let server_name = server_name.clone();
                                                                let dispatch = _search_dispatch.clone();
                                                                let selected_episodes = selected_episodes.clone();
                                                                Callback::from(move |_| {
                                                                    let selected_ids = selected_ids.clone();
                                                                    let api_key = api_key.clone();
                                                                    let server_name = server_name.clone();
                                                                    let _dispatch = dispatch.clone();
                                                                    let selected_episodes = selected_episodes.clone();
                                                                    spawn_local(async move {
                                                                        let request = BulkEpisodeActionRequest {
                                                                            episode_ids: selected_ids,
                                                                            user_id: user_id_value,
                                                                            is_youtube: None,
                                                                        };
                                                                        match call_bulk_queue_episodes(
                                                                            &server_name.unwrap_or_default(),
                                                                            &api_key.flatten(),
                                                                            &request
                                                                        ).await {
                                                                            Ok(message) => {
                                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                    state.info_message = Some(message);
                                                                                });
                                                                                selected_episodes.set(HashSet::new());
                                                                            }
                                                                            Err(e) => {
                                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                    state.error_message = Some(format!("Error: {}", e));
                                                                                });
                                                                            }
                                                                        }
                                                                    });
                                                                })
                                                            }
                                                            class="btn btn-secondary"
                                                        >
                                                            <i class="ph ph-list-plus"></i>
                                                            {&i18n_queue_episodes}
                                                        </button>

                                                        // Download button
                                                        <button
                                                            onclick={
                                                                let selected_ids = selected_ids.clone();
                                                                let api_key = api_key.clone();
                                                                let server_name = server_name.clone();
                                                                let dispatch = _search_dispatch.clone();
                                                                let selected_episodes = selected_episodes.clone();
                                                                Callback::from(move |_| {
                                                                    let selected_ids = selected_ids.clone();
                                                                    let api_key = api_key.clone();
                                                                    let server_name = server_name.clone();
                                                                    let _dispatch = dispatch.clone();
                                                                    let selected_episodes = selected_episodes.clone();
                                                                    spawn_local(async move {
                                                                        let request = BulkEpisodeActionRequest {
                                                                            episode_ids: selected_ids,
                                                                            user_id: user_id_value,
                                                                            is_youtube: None,
                                                                        };
                                                                        match call_bulk_download_episodes(
                                                                            &server_name.unwrap_or_default(),
                                                                            &api_key.flatten(),
                                                                            &request
                                                                        ).await {
                                                                            Ok(message) => {
                                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                    state.info_message = Some(message);
                                                                                });
                                                                                selected_episodes.set(HashSet::new());
                                                                            }
                                                                            Err(e) => {
                                                                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                                                                    state.error_message = Some(format!("Error: {}", e));
                                                                                });
                                                                            }
                                                                        }
                                                                    });
                                                                })
                                                            }
                                                            class="btn btn-secondary"
                                                        >
                                                            <i class="ph ph-download-simple"></i>
                                                            {&i18n_download_episodes}
                                                        </button>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }

                                {
                                    if *is_subscribing {
                                        html! {
                                            <div class="flex flex-col items-center justify-center py-12 gap-4">
                                                <div class="spinner"></div>
                                                <p class="item_container-text">{ &i18n_loading }</p>
                                            </div>
                                        }
                                    } else if let (Some(_), Some(podcast_info)) = (podcast_feed_results, &clicked_podcast_info) {
                                        let _podcast_link_clone = podcast_info.feedurl.clone();
                                        let _podcast_title = podcast_info.podcastname.clone();

                                        // Episode selection callback
                                        let selected_episodes_clone = selected_episodes.clone();
                                        let _on_episode_select = Callback::from(move |(episode_id, is_selected): (i32, bool)| {
                                            selected_episodes_clone.set({
                                                let mut current = (*selected_episodes_clone).clone();
                                                if is_selected {
                                                    current.insert(episode_id);
                                                } else {
                                                    current.remove(&episode_id);
                                                }
                                                current
                                            });
                                        });

                                        // on_select_older / on_select_newer hoisted to
                                        // use_callback at the top of the function (stable
                                        // identity across renders).

                                        // Key encodes the filter signature so a sort / completed-filter /
                                        // in-progress / podcast switch remounts the view and re-runs the
                                        // ramp. Search term is deliberately omitted — including it would
                                        // remount on every keystroke.
                                        let sort_code = match *episode_sort_direction {
                                            Some(EpisodeSortDirection::NewestFirst) => "n",
                                            Some(EpisodeSortDirection::OldestFirst) => "o",
                                            Some(EpisodeSortDirection::ShortestFirst) => "s",
                                            Some(EpisodeSortDirection::LongestFirst) => "l",
                                            Some(EpisodeSortDirection::TitleAZ) => "a",
                                            Some(EpisodeSortDirection::TitleZA) => "z",
                                            None => "_",
                                        };
                                        let filter_code = match *completed_filter_state {
                                            CompletedFilter::ShowAll => "a",
                                            CompletedFilter::ShowOnly => "c",
                                            CompletedFilter::Hide => "h",
                                        };
                                        let view_key = format!(
                                            "elv-{}-{}-{}-{}",
                                            *podcast_id, sort_code, filter_code, *show_in_progress
                                        );

                                        let loaded_count = search_data
                                            .podcast_feed_results
                                            .as_ref()
                                            .map(|r| r.episodes.len() as i64)
                                            .unwrap_or(0);
                                        let backend_total = search_data
                                            .podcast_feed_results
                                            .as_ref()
                                            .map(|r| r.total)
                                            .unwrap_or(0);
                                        let backend_can_load_more =
                                            loaded_count > 0 && loaded_count < backend_total;
                                        let episodes_rc: Rc<Vec<Episode>> = (*filtered_episodes).clone();
                                        let selected_eps_rc: Rc<HashSet<i32>> =
                                            Rc::new((*selected_episodes).clone());

                                        html! {
                                            <div class="flex-grow overflow-y-auto">
                                                <EpisodeListView
                                                    key={view_key}
                                                    episodes={episodes_rc}
                                                    backend_can_load_more={backend_can_load_more}
                                                    loading_more={*loading_more}
                                                    on_load_more={on_load_more.clone()}
                                                    is_delete_mode={*is_selecting}
                                                    on_checkbox_change={on_episode_checkbox_change.clone()}
                                                    on_select_above={on_select_newer.clone()}
                                                    on_select_below={on_select_older.clone()}
                                                    selected_episodes={selected_eps_rc}
                                                />
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <div class="empty-episodes-container" id="episode-container">
                                                <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                                <h1 class="page-subtitles">{ &i18n_no_episodes_found }</h1>
                                                <p class="page-paragraphs">{&i18n_no_episodes_description}</p>
                                            </div>
                                        }
                                    }
                                }
                            </>
                        }
                    }
                }
            <App_drawer />
            <AudioPlayerBar />
            </div>

        }
}
