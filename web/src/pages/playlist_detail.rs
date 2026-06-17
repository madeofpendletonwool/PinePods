use crate::components::app_drawer::App_drawer;
use crate::components::audio::on_play_click;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, NotificationState, UIState};
use crate::components::episode_list_view::EpisodeListView;
use crate::components::gen_components::{Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::pages::playlists::{IconSelector, PodcastSelector};
use crate::requests::episode::Episode;
use crate::requests::pod_req::{self, PlaylistInfo, Podcast, UpdatePlaylistRequest};
use gloo_timers::future::TimeoutFuture;
use i18nrs::yew::use_translation;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub id: i32,
}

#[function_component(PlaylistDetail)]
pub fn playlist_detail(props: &Props) -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();

    let episodes = use_state(|| Rc::new(Vec::<Episode>::new()));
    let playlist_info = use_state(|| Option::<PlaylistInfo>::None);
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading = use_state(|| true);
    let loading_more = use_state(|| false);
    let error = use_state(|| None::<String>);

    // Edit modal state
    let show_edit_modal = use_state(|| false);
    let edit_name = use_state(String::new);
    let edit_description = use_state(String::new);
    let edit_icon_name = use_state(|| "ph-playlist".to_string());
    let edit_include_unplayed = use_state(|| true);
    let edit_include_partially_played = use_state(|| true);
    let edit_include_played = use_state(|| false);
    let edit_min_duration = use_state(String::new);
    let edit_max_duration = use_state(String::new);
    let edit_sort_order = use_state(|| "date_desc".to_string());
    let edit_group_by_podcast = use_state(|| false);
    let edit_max_episodes = use_state(String::new);
    let edit_play_progress_min = use_state(String::new);
    let edit_play_progress_max = use_state(String::new);
    let edit_time_filter_hours = use_state(String::new);
    let edit_selected_podcasts = use_state(|| Vec::<i32>::new());
    let edit_available_podcasts = use_state(|| Vec::<Podcast>::new());
    let edit_loading_podcasts = use_state(|| false);
    let edit_saving = use_state(|| false);

    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let playlist_id = props.id;

    // Initial page fetch
    {
        let episodes = episodes.clone();
        let playlist_info = playlist_info.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading = loading.clone();
        let error = error.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone(), playlist_id),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_playlist_episodes_paged(
                            &server_name,
                            &api_key.unwrap_or_default(),
                            &user_id,
                            playlist_id,
                            PAGE_SIZE,
                            0,
                        )
                        .await
                        {
                            Ok(page) => {
                                let new_offset = page.episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                playlist_info.set(Some(page.playlist_info));
                                episodes.set(Rc::new(page.episodes));
                                loading.set(false);
                            }
                            Err(e) => {
                                error.set(Some(e.to_string()));
                                loading.set(false);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    // Populate edit form when playlist_info loads
    {
        let playlist_info = playlist_info.clone();
        let edit_name = edit_name.clone();
        let edit_description = edit_description.clone();
        let edit_icon_name = edit_icon_name.clone();
        let edit_include_unplayed = edit_include_unplayed.clone();
        let edit_include_partially_played = edit_include_partially_played.clone();
        let edit_include_played = edit_include_played.clone();
        let edit_min_duration = edit_min_duration.clone();
        let edit_max_duration = edit_max_duration.clone();
        let edit_sort_order = edit_sort_order.clone();
        let edit_group_by_podcast = edit_group_by_podcast.clone();
        let edit_max_episodes = edit_max_episodes.clone();
        let edit_play_progress_min = edit_play_progress_min.clone();
        let edit_play_progress_max = edit_play_progress_max.clone();
        let edit_time_filter_hours = edit_time_filter_hours.clone();
        let edit_selected_podcasts = edit_selected_podcasts.clone();

        use_effect_with(playlist_info.clone(), move |playlist_info| {
            if let Some(info) = (**playlist_info).as_ref() {
                edit_name.set(info.name.clone());
                edit_description.set(info.description.clone().unwrap_or_default());
                edit_icon_name.set(info.icon_name.clone().unwrap_or_else(|| "ph-playlist".to_string()));
                edit_include_unplayed.set(info.include_unplayed.unwrap_or(true));
                edit_include_partially_played.set(info.include_partially_played.unwrap_or(true));
                edit_include_played.set(info.include_played.unwrap_or(false));
                edit_min_duration.set(info.min_duration.map(|v| v.to_string()).unwrap_or_default());
                edit_max_duration.set(info.max_duration.map(|v| v.to_string()).unwrap_or_default());
                edit_sort_order.set(info.sort_order.clone().unwrap_or_else(|| "date_desc".to_string()));
                edit_group_by_podcast.set(info.group_by_podcast.unwrap_or(false));
                edit_max_episodes.set(info.max_episodes.map(|v| v.to_string()).unwrap_or_default());
                edit_play_progress_min.set(info.play_progress_min.map(|v| v.to_string()).unwrap_or_default());
                edit_play_progress_max.set(info.play_progress_max.map(|v| v.to_string()).unwrap_or_default());
                edit_time_filter_hours.set(info.time_filter_hours.map(|v| v.to_string()).unwrap_or_default());
                edit_selected_podcasts.set(info.podcast_ids.clone().unwrap_or_default());
            }
            || ()
        });
    }

    // Load podcasts when edit modal opens
    {
        let available_podcasts = edit_available_podcasts.clone();
        let loading_podcasts = edit_loading_podcasts.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let show_edit_modal = show_edit_modal.clone();

        use_effect_with(show_edit_modal.clone(), move |show| {
            if **show {
                if let (Some(server_name), Some(api_key), Some(user_id)) =
                    (server_name, api_key, user_id)
                {
                    loading_podcasts.set(true);
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_podcasts(&server_name, &api_key, &user_id).await {
                            Ok(podcasts) => {
                                available_podcasts.set(podcasts);
                                loading_podcasts.set(false);
                            }
                            Err(_) => {
                                loading_podcasts.set(false);
                            }
                        }
                    });
                }
            }
            || ()
        });
    }

    // Load-more handler. EpisodeListView owns the sentinel/observer/display-count/ramp; this
    // callback fires only when the view runs out of buffered episodes and the parent reports
    // `backend_can_load_more`.
    let on_load_more = {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        use_callback((), move |_: (), _| {
            if *loading_more {
                return;
            }
            let current_offset = *offset;
            let current_total = *total;
            if current_offset >= current_total {
                return;
            }
            let Some(api_key) = api_key.clone() else { return; };
            let Some(user_id) = user_id.clone() else { return; };
            let Some(server_name) = server_name.clone() else { return; };
            loading_more.set(true);
            let episodes = episodes.clone();
            let total = total.clone();
            let offset = offset.clone();
            let loading_more = loading_more.clone();
            spawn_local(async move {
                if let Ok(page) = pod_req::call_get_playlist_episodes_paged(
                    &server_name,
                    &api_key.unwrap_or_default(),
                    &user_id,
                    playlist_id,
                    PAGE_SIZE,
                    current_offset,
                )
                .await
                {
                    TimeoutFuture::new(0).await;
                    let new_offset = current_offset + page.episodes.len() as i64;
                    let mut all = (**episodes).clone();
                    all.extend(page.episodes);
                    total.set(page.total);
                    offset.set(new_offset);
                    episodes.set(Rc::new(all));
                    TimeoutFuture::new(0).await;
                }
                loading_more.set(false);
            });
        })
    };

    // "Play from Top" button callback
    let play_from_top = {
        let episodes = episodes.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let audio_dispatch = audio_dispatch.clone();
        let audio_state = audio_state.clone();

        Callback::from(move |_: MouseEvent| {
            let ep_list = (*episodes).clone();
            let target = ep_list
                .iter()
                .find(|ep| !ep.completed)
                .or_else(|| ep_list.first())
                .cloned();

            if let Some(episode) = target {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let episode_id = episode.episodeid;
                    audio_dispatch.reduce_mut(move |state| {
                        state.loading_episode_id = Some(episode_id);
                    });
                    on_play_click(
                        episode,
                        api_key.unwrap_or_default(),
                        user_id,
                        server_name,
                        audio_dispatch.clone(),
                        audio_state.clone(),
                        false,
                        false,
                        Some(playlist_id),
                    )
                    .emit(MouseEvent::new("click").unwrap());
                }
            }
        })
    };

    // Open edit modal
    let on_edit_click = {
        let show_edit_modal = show_edit_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_edit_modal.set(true);
        })
    };

    // Close edit modal
    let on_edit_modal_close = {
        let show_edit_modal = show_edit_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_edit_modal.set(false);
        })
    };

    let on_edit_modal_background_click = {
        let show_edit_modal = show_edit_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_edit_modal.set(false);
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    // Submit edit form
    let on_edit_submit = {
        let edit_name = edit_name.clone();
        let edit_description = edit_description.clone();
        let edit_icon_name = edit_icon_name.clone();
        let edit_include_unplayed = edit_include_unplayed.clone();
        let edit_include_partially_played = edit_include_partially_played.clone();
        let edit_include_played = edit_include_played.clone();
        let edit_min_duration = edit_min_duration.clone();
        let edit_max_duration = edit_max_duration.clone();
        let edit_sort_order = edit_sort_order.clone();
        let edit_group_by_podcast = edit_group_by_podcast.clone();
        let edit_max_episodes = edit_max_episodes.clone();
        let edit_play_progress_min = edit_play_progress_min.clone();
        let edit_play_progress_max = edit_play_progress_max.clone();
        let edit_time_filter_hours = edit_time_filter_hours.clone();
        let edit_selected_podcasts = edit_selected_podcasts.clone();
        let edit_saving = edit_saving.clone();
        let show_edit_modal = show_edit_modal.clone();
        let playlist_info = playlist_info.clone();
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let edit_failed_msg = i18n.t("playlist_detail.edit_failed");

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();

            if *edit_saving {
                return;
            }

            let name = (*edit_name).clone();
            if name.trim().is_empty() {
                return;
            }

            let description = (*edit_description).clone();
            let icon = (*edit_icon_name).clone();
            let include_unplayed = *edit_include_unplayed;
            let include_partially_played = *edit_include_partially_played;
            let include_played = *edit_include_played;
            let min_duration: Option<i32> = edit_min_duration.parse().ok();
            let max_duration: Option<i32> = edit_max_duration.parse().ok();
            let sort_order = (*edit_sort_order).clone();
            let group_by_podcast = *edit_group_by_podcast;
            let max_episodes: Option<i32> = edit_max_episodes.parse().ok();
            let play_progress_min: Option<f32> = edit_play_progress_min.parse().ok();
            let play_progress_max: Option<f32> = edit_play_progress_max.parse().ok();
            let time_filter_hours: Option<i32> = edit_time_filter_hours.parse().ok();
            let selected_podcasts = (*edit_selected_podcasts).clone();

            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let server_name = server_name.clone();
            let edit_saving = edit_saving.clone();
            let show_edit_modal = show_edit_modal.clone();
            let playlist_info = playlist_info.clone();
            let episodes = episodes.clone();
            let total = total.clone();
            let offset = offset.clone();
            let edit_failed_msg = edit_failed_msg.clone();

            if let (Some(api_key), Some(user_id), Some(server_name)) =
                (api_key, user_id, server_name)
            {
                edit_saving.set(true);

                let podcast_ids = if selected_podcasts.is_empty() {
                    None
                } else {
                    Some(selected_podcasts)
                };

                let request = UpdatePlaylistRequest {
                    user_id,
                    playlist_id,
                    name: name.clone(),
                    description: if description.is_empty() { None } else { Some(description) },
                    podcast_ids,
                    include_unplayed,
                    include_partially_played,
                    include_played,
                    min_duration,
                    max_duration,
                    sort_order: sort_order.clone(),
                    group_by_podcast,
                    max_episodes,
                    icon_name: icon.clone(),
                    play_progress_min,
                    play_progress_max,
                    time_filter_hours,
                };

                wasm_bindgen_futures::spawn_local(async move {
                    let api_key_str = api_key.unwrap_or_default();
                    match pod_req::call_update_playlist(
                        &server_name,
                        &api_key_str,
                        request,
                    )
                    .await
                    {
                        Ok(_) => {
                            show_edit_modal.set(false);
                            // Refresh playlist by re-fetching episodes
                            if let Ok(page) = pod_req::call_get_playlist_episodes_paged(
                                &server_name,
                                &api_key_str,
                                &user_id,
                                playlist_id,
                                PAGE_SIZE,
                                0,
                            )
                            .await
                            {
                                let new_offset = page.episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                playlist_info.set(Some(page.playlist_info));
                                episodes.set(Rc::new(page.episodes));
                            }
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message = Some(format!("{}: {}", edit_failed_msg, e));
                            });
                        }
                    }
                    edit_saving.set(false);
                });
            }
        })
    };

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />

                if *loading {
                    { html! { <Loading/> } }
                } else {
                    if let Some(error_msg) = &*error {
                        <div class="error-message">
                            {error_msg}
                        </div>
                    } else {
                        <div class="section-container">
                            if let Some(info) = &*playlist_info {
                                <div class="playlist-header mb-6">
                                    <div class="flex items-start justify-between gap-4">
                                        <div class="flex items-center gap-4 min-w-0">
                                            <i class={classes!("text-4xl", info.icon_name.clone())}></i>
                                            <div>
                                                <h1 class="text-2xl font-bold item_container-text">{&info.name}</h1>
                                                if let Some(desc) = &info.description {
                                                    <p class="text-gray-600 dark:text-gray-400">{desc}</p>
                                                }
                                                <p class="text-sm item_container-text mt-1">
                                                    {format!("{} {}", info.episode_count.unwrap_or(0), &i18n.t("playlist_detail.episodes"))}
                                                </p>
                                            </div>
                                        </div>
                                        <div class="flex flex-col sm:flex-row items-stretch sm:items-center gap-2 w-full sm:w-auto">
                                            if !info.is_system_playlist.unwrap_or(false) {
                                                <button
                                                    onclick={on_edit_click}
                                                    class="btn-secondary flex items-center justify-center gap-2 px-4 py-2 rounded-lg"
                                                >
                                                    <i class="ph ph-pencil text-xl"></i>
                                                    {&i18n.t("playlist_detail.edit_playlist")}
                                                </button>
                                            }
                                            <button
                                                onclick={play_from_top}
                                                class="btn-primary flex items-center justify-center gap-2 px-4 py-2 rounded-lg"
                                                disabled={(*episodes).is_empty()}
                                            >
                                                <i class="ph ph-play-circle text-xl"></i>
                                                {&i18n.t("playlist_detail.play_from_top")}
                                            </button>
                                        </div>
                                    </div>
                                </div>
                            }

                            if (**episodes).is_empty() {
                                <div class="flex flex-col items-center justify-center p-8 mt-4 item-container rounded-lg shadow-md">
                                    <i class="ph ph-playlist-x text-6xl mb-4"></i>
                                    <h3 class="text-xl font-semibold mb-2 item_container-text">{&i18n.t("playlist_detail.no_episodes_found")}</h3>
                                    <p class="text-center item_container-text text-sm max-w-md">
                                        {
                                            match (*playlist_info).as_ref().map(|i| i.name.as_str()) {
                                                Some("Fresh Releases") => i18n.t("playlist_detail.fresh_releases_empty"),
                                                Some("Currently Listening") => i18n.t("playlist_detail.currently_listening_empty"),
                                                Some("Almost Done") => i18n.t("playlist_detail.almost_done_empty"),
                                                _ => i18n.t("playlist_detail.no_episodes_match_criteria")
                                            }
                                        }
                                    </p>
                                </div>
                            } else {
                                <div class="flex-grow overflow-y-auto">
                                    <EpisodeListView
                                        key={format!("playlist-{}", playlist_id)}
                                        episodes={(*episodes).clone()}
                                        backend_can_load_more={*offset < *total}
                                        loading_more={*loading_more}
                                        on_load_more={on_load_more.clone()}
                                    />
                                </div>
                            }
                        </div>
                    }
                }
                <AudioPlayerBar />
            </div>
            <App_drawer />

            // Edit playlist modal
            if *show_edit_modal {
                <div
                    tabindex="-1"
                    aria-hidden="true"
                    class="fixed inset-0 z-50 overflow-y-auto bg-black bg-opacity-25"
                    onclick={on_edit_modal_background_click}
                >
                    <div class="flex min-h-full items-center justify-center p-4">
                        <div class="modal-container relative w-full max-w-md rounded-lg shadow" onclick={stop_propagation}>
                            <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                                <h3 class="text-xl font-semibold">
                                    {&i18n.t("playlist_detail.edit_playlist")}
                                </h3>
                                <button onclick={on_edit_modal_close} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                                    <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                        <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                                    </svg>
                                    <span class="sr-only">{&i18n.t("common.close")}</span>
                                </button>
                            </div>
                            <div class="p-4 md:p-5">
                                <form class="space-y-4" action="#">
                                    <div>
                                        <label class="block mb-2 text-sm font-medium">{&i18n.t("playlists.name")}</label>
                                        <input
                                            type="text"
                                            class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                            value={(*edit_name).clone()}
                                            oninput={let edit_name = edit_name.clone(); Callback::from(move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                edit_name.set(input.value());
                                            })}
                                        />
                                    </div>

                                    <div>
                                        <label class="block mb-2 text-sm font-medium">{&i18n.t("playlists.description")}</label>
                                        <textarea
                                            class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                            value={(*edit_description).clone()}
                                            oninput={let edit_description = edit_description.clone(); Callback::from(move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                edit_description.set(input.value());
                                            })}
                                        />
                                    </div>

                                    <div>
                                        <label class="block mb-2 text-sm font-medium">{&i18n.t("playlists.icon")}</label>
                                        <IconSelector
                                            selected_icon={(*edit_icon_name).clone()}
                                            on_select={let edit_icon_name = edit_icon_name.clone(); Callback::from(move |new_icon| {
                                                edit_icon_name.set(new_icon);
                                            })}
                                        />
                                    </div>

                                    <div>
                                        <label class="block mb-2 text-sm font-medium">{&i18n.t("playlist_detail.edit_filter_by_podcasts")}</label>
                                        {
                                            if *edit_loading_podcasts {
                                                html! { <Loading/> }
                                            } else {
                                                html! {
                                                    <PodcastSelector
                                                        selected_podcasts={(*edit_selected_podcasts).clone()}
                                                        on_select={
                                                            let edit_selected_podcasts = edit_selected_podcasts.clone();
                                                            Callback::from(move |new_selection| {
                                                                edit_selected_podcasts.set(new_selection);
                                                            })
                                                        }
                                                        available_podcasts={(*edit_available_podcasts).clone()}
                                                    />
                                                }
                                            }
                                        }
                                    </div>

                                    <div>
                                        <label class="block mb-2 text-sm font-medium">{&i18n.t("playlist_detail.edit_episode_status")}</label>
                                        <div class="space-y-2">
                                            <label class="flex items-center gap-2 cursor-pointer">
                                                <input type="checkbox"
                                                    checked={*edit_include_unplayed}
                                                    onchange={let edit_include_unplayed = edit_include_unplayed.clone(); Callback::from(move |_| {
                                                        edit_include_unplayed.set(!*edit_include_unplayed);
                                                    })}
                                                />
                                                <span class="text-sm">{&i18n.t("playlist_detail.edit_include_unplayed")}</span>
                                            </label>
                                            <label class="flex items-center gap-2 cursor-pointer">
                                                <input type="checkbox"
                                                    checked={*edit_include_partially_played}
                                                    onchange={let edit_include_partially_played = edit_include_partially_played.clone(); Callback::from(move |_| {
                                                        edit_include_partially_played.set(!*edit_include_partially_played);
                                                    })}
                                                />
                                                <span class="text-sm">{&i18n.t("playlist_detail.edit_include_partially_played")}</span>
                                            </label>
                                            <label class="flex items-center gap-2 cursor-pointer">
                                                <input type="checkbox"
                                                    checked={*edit_include_played}
                                                    onchange={let edit_include_played = edit_include_played.clone(); Callback::from(move |_| {
                                                        edit_include_played.set(!*edit_include_played);
                                                    })}
                                                />
                                                <span class="text-sm">{&i18n.t("playlist_detail.edit_include_played")}</span>
                                            </label>
                                        </div>
                                    </div>

                                    <div class="grid grid-cols-2 gap-4">
                                        <div>
                                            <label class="block mb-2 text-sm font-medium">{&i18n.t("playlist_detail.edit_min_duration")}</label>
                                            <input
                                                type="number"
                                                class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                                value={(*edit_min_duration).clone()}
                                                oninput={let edit_min_duration = edit_min_duration.clone(); Callback::from(move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    edit_min_duration.set(input.value());
                                                })}
                                            />
                                        </div>
                                        <div>
                                            <label class="block mb-2 text-sm font-medium">{&i18n.t("playlist_detail.edit_max_duration")}</label>
                                            <input
                                                type="number"
                                                class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                                value={(*edit_max_duration).clone()}
                                                oninput={let edit_max_duration = edit_max_duration.clone(); Callback::from(move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    edit_max_duration.set(input.value());
                                                })}
                                            />
                                        </div>
                                    </div>

                                    <div>
                                        <label class="block mb-2 text-sm font-medium">{&i18n.t("playlists.sort_order")}</label>
                                        <select
                                            class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                            value={(*edit_sort_order).clone()}
                                            onchange={let edit_sort_order = edit_sort_order.clone(); Callback::from(move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                edit_sort_order.set(input.value());
                                            })}
                                        >
                                            <option value="date_desc" selected={*edit_sort_order == "date_desc"}>{"Newest First"}</option>
                                            <option value="date_asc" selected={*edit_sort_order == "date_asc"}>{"Oldest First"}</option>
                                            <option value="duration_desc" selected={*edit_sort_order == "duration_desc"}>{"Longest First"}</option>
                                            <option value="duration_asc" selected={*edit_sort_order == "duration_asc"}>{"Shortest First"}</option>
                                        </select>
                                    </div>

                                    <div class="grid grid-cols-2 gap-4">
                                        <div>
                                            <label class="block mb-2 text-sm font-medium">{&i18n.t("playlist_detail.edit_min_progress")}</label>
                                            <input
                                                type="number"
                                                class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                                value={(*edit_play_progress_min).clone()}
                                                oninput={let edit_play_progress_min = edit_play_progress_min.clone(); Callback::from(move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    edit_play_progress_min.set(input.value());
                                                })}
                                            />
                                        </div>
                                        <div>
                                            <label class="block mb-2 text-sm font-medium">{&i18n.t("playlist_detail.edit_max_progress")}</label>
                                            <input
                                                type="number"
                                                class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                                value={(*edit_play_progress_max).clone()}
                                                oninput={let edit_play_progress_max = edit_play_progress_max.clone(); Callback::from(move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    edit_play_progress_max.set(input.value());
                                                })}
                                            />
                                        </div>
                                    </div>

                                    <div class="grid grid-cols-2 gap-4">
                                        <div>
                                            <label class="block mb-2 text-sm font-medium">{&i18n.t("playlist_detail.edit_time_filter_hours")}</label>
                                            <input
                                                type="number"
                                                class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                                value={(*edit_time_filter_hours).clone()}
                                                oninput={let edit_time_filter_hours = edit_time_filter_hours.clone(); Callback::from(move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    edit_time_filter_hours.set(input.value());
                                                })}
                                            />
                                        </div>
                                        <div>
                                            <label class="block mb-2 text-sm font-medium">{&i18n.t("playlist_detail.edit_max_episodes")}</label>
                                            <input
                                                type="number"
                                                class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                                value={(*edit_max_episodes).clone()}
                                                oninput={let edit_max_episodes = edit_max_episodes.clone(); Callback::from(move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    edit_max_episodes.set(input.value());
                                                })}
                                            />
                                        </div>
                                    </div>

                                    <label class="flex items-center gap-2 cursor-pointer">
                                        <input type="checkbox"
                                            checked={*edit_group_by_podcast}
                                            onchange={let edit_group_by_podcast = edit_group_by_podcast.clone(); Callback::from(move |_| {
                                                edit_group_by_podcast.set(!*edit_group_by_podcast);
                                            })}
                                        />
                                        <span class="text-sm">{&i18n.t("playlist_detail.edit_group_by_podcast")}</span>
                                    </label>

                                    <button
                                        type="submit"
                                        onclick={on_edit_submit}
                                        disabled={*edit_saving}
                                        class="w-full btn-primary text-white focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-5 py-2.5 text-center"
                                    >
                                        if *edit_saving {
                                            {"Saving..."}
                                        } else {
                                            {&i18n.t("playlist_detail.edit_save")}
                                        }
                                    </button>
                                </form>
                            </div>
                        </div>
                    </div>
                </div>
            }
        </>
    }
}
