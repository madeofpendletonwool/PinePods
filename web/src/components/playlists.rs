use super::app_drawer::App_drawer;
use super::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::gen_funcs::format_error_message;
use crate::requests::pod_req::{self, CreatePlaylistRequest, Playlist, Podcast};
use gloo_events::EventListener;
use std::collections::HashSet;
use wasm_bindgen::JsCast;
use web_sys::{HtmlElement, HtmlInputElement};
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

// Enum to track which modal should be shown
#[derive(PartialEq)]
enum ModalState {
    Hidden,
    Create,
    Delete,
    BulkDelete,
}

#[derive(Properties, PartialEq, Clone)]
struct PlaylistCardProps {
    playlist: Playlist,
    on_delete: Callback<MouseEvent>,
    on_select: Callback<MouseEvent>,
    is_selectable: bool,
    is_selected: bool,
    on_toggle_select: Callback<i32>,
}

#[function_component(PlaylistCard)]
fn playlist_card(props: &PlaylistCardProps) -> Html {
    let on_checkbox_change = {
        let playlist_id = props.playlist.playlist_id;
        let on_toggle_select = props.on_toggle_select.clone();

        Callback::from(move |e: Event| {
            e.stop_propagation();
            on_toggle_select.emit(playlist_id);
        })
    };

    html! {
        <div class={classes!(
            "playlist-card",
            if props.is_selected { "playlist-card-selected" } else { "" },
            if props.playlist.is_system_playlist { "system-playlist" } else { "" }
        )}>
            <div class="playlist-card-stack" onclick={props.on_select.clone()}>
                <div class="playlist-card-content">
                    {
                        if props.is_selectable {
                            html! {
                                <div class="absolute top-2 left-2 z-10" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                    <input
                                        type="checkbox"
                                        checked={props.is_selected}
                                        onchange={on_checkbox_change}
                                        class="w-5 h-5 text-blue-600 rounded focus:ring-blue-500"
                                    />
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                    <i class={classes!("ph", props.playlist.icon_name.clone(), "playlist-icon")}></i>
                    <div class="playlist-info">
                        <h3 class="playlist-title">
                            {&props.playlist.name}
                            {
                                if props.playlist.is_system_playlist {
                                    html! { <span class="text-xs ml-2 text-gray-400">{"(System)"}</span> }
                                } else {
                                    html! {}
                                }
                            }
                        </h3>
                        <span class="playlist-count">{format!("{} episodes", props.playlist.episode_count.unwrap_or(0))}</span>
                        if let Some(description) = &props.playlist.description {
                            <p class="playlist-description">{description}</p>
                        }
                    </div>
                </div>
            </div>
            {
                if !props.is_selectable && !props.playlist.is_system_playlist {
                    html! {
                        <button
                            onclick={props.on_delete.clone()}
                            class="delete-button absolute top-2 right-2 item-container-button selector-button rounded-full p-2"
                        >
                            <i class="ph ph-trash text-xl"></i>
                        </button>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct IconSelectorProps {
    pub selected_icon: String,
    pub on_select: Callback<String>,
}

#[function_component(IconSelector)]
pub fn icon_selector(props: &IconSelectorProps) -> Html {
    let is_open = use_state(|| false);
    // Common playlist/audio icons from Phosphor
    let icons = vec![
        "ph-playlist",
        "ph-music-notes",
        "ph-play-circle",
        "ph-headphones",
        "ph-star",
        "ph-heart",
        "ph-bookmark",
        "ph-clock",
        "ph-calendar",
        "ph-timer",
        "ph-music-note",
        "ph-shuffle",
        "ph-repeat",
        "ph-microphone-stage",
        "ph-radio",
        "ph-speaker-high",
        "ph-broadcast",
        "ph-waveform",
        "ph-ear",
        "ph-file-audio",
        "ph-speaker-hifi",
        "ph-wave-sawtooth",
        "ph-soundcloud-logo",
        "ph-globe-stand",
        "ph-vinyl-record",
        "ph-microphone",
        "ph-queue",
        "ph-list-plus",
        "ph-list-checks",
        "ph-airplane-landing",
        "ph-air-traffic-control",
        "ph-alien",
        "ph-music-notes-plus",
        "ph-music-notes-simple",
        "ph-speaker-simple-high",
        "ph-speaker-simple-low",
        "ph-speaker-simple-x",
        "ph-wave-sine",
        "ph-wave-triangle",
        "ph-wave-square",
        "ph-acorn",
        "ph-spotify-logo",
        "ph-airplay",
        "ph-note",
        "ph-equalizer",
        "ph-anchor",
        "ph-android-logo",
        "ph-aperture",
        "ph-armchair",
        "ph-app-window",
        "ph-asclepius",
        "ph-axe",
        "ph-baby",
        "ph-avocado",
        "ph-backpack",
        "ph-bank",
        "ph-barn",
        "ph-barcode",
        "ph-baseball",
        "ph-barbell",
        "ph-bathtub",
        "ph-battery-charging",
        "ph-basketball",
        "ph-beer-stein",
        "ph-beanie",
        "ph-bell",
        "ph-bicycle",
        "ph-biohazard",
        "ph-binoculars",
        "ph-bird",
        "ph-boat",
        "ph-bone",
        "ph-bowl-food",
        "ph-boot",
        "ph-bowling-ball",
        "ph-brain",
        "ph-broom",
        "ph-bridge",
        "ph-cactus",
        "ph-cake",
        "ph-campfire",
        "ph-car-profile",
        "ph-cat",
        "ph-cell-tower",
        "ph-cheese",
        "ph-circuitry",
        "ph-city",
        "ph-clipboard",
        "ph-cloud",
        "ph-clover",
        "ph-club",
        "ph-coffee",
        "ph-code",
        "ph-coffee-bean",
        "ph-compass",
        "ph-cow",
        "ph-cross",
        "ph-crown",
        "ph-dna",
        "ph-dress",
        "ph-drop",
        "ph-engine",
        "ph-envelope",
        "ph-eye",
        "ph-eyeglasses",
        "ph-eyes",
        "ph-feather",
        "ph-film-slate",
        "ph-film-reel",
        "ph-fire",
        "ph-fish",
        "ph-flashlight",
        "ph-floppy-disk",
        "ph-flying-saucer",
        "ph-football",
        "ph-game-controller",
        "ph-gavel",
        "ph-ghost",
        "ph-gift",
        "ph-goggles",
        "ph-golf",
        "ph-graphics-card",
        "ph-guitar",
        "ph-hamburger",
        "ph-hammer",
        "ph-hand-eye",
        "ph-gear",
        "ph-hand-peace",
        "ph-hand-waving",
        "ph-hard-hat",
        "ph-heart",
        "ph-heart-break",
        "ph-highlighter",
        "ph-hockey",
        "ph-hoodie",
        "ph-horse",
        "ph-hospital",
        "ph-hourglass-medium",
        "ph-house-line",
        "ph-hurricane",
        "ph-ice-cream",
        "ph-image",
        "ph-infinity",
        "ph-info",
        "ph-island",
        "ph-jeep",
        "ph-joystick",
        "ph-key",
        "ph-keyhole",
        "ph-ladder",
        "ph-knife",
        "ph-lamp",
        "ph-leaf",
        "ph-lego",
        "ph-lego-smiley",
        "ph-lightbulb",
        "ph-lighthouse",
        "ph-link",
        "ph-linux-logo",
        "ph-log",
        "ph-magic-wand",
        "ph-magnet",
        "ph-mailbox",
        "ph-map-pin",
        "ph-meteor",
        "ph-microscope",
        "ph-moon",
        "ph-moped",
        "ph-mouse",
        "ph-mosque",
        "ph-mountains",
        "ph-motorcycle",
        "ph-onigiri",
        "ph-office-chair",
        "ph-paint-brush",
        "ph-oven",
        "ph-orange-slice",
        "ph-package",
        "ph-palette",
        "ph-paint-bucket",
        "ph-pants",
        "ph-paper-plane",
        "ph-paperclip",
        "ph-parachute",
        "ph-paw-print",
        "ph-peace",
        "ph-pen",
        "ph-pencil",
        "ph-pentagram",
        "ph-pepper",
        "ph-person",
        "ph-person-simple-hike",
        "ph-phone",
        "ph-pi",
        "ph-piano-keys",
        "ph-ping-pong",
        "ph-pizza",
        "ph-plant",
        "ph-push-pin",
        "ph-question-mark",
        "ph-rabbit",
        "ph-puzzle-piece",
        "ph-rainbow",
        "ph-ranking",
        "ph-recycle",
        "ph-repeat",
        "ph-scales",
        "ph-sailboat",
        "ph-scissors",
        "ph-scribble",
        "ph-security-camera",
        "ph-shooting-star",
        "ph-shrimp",
        "ph-shovel",
        "ph-shuffle",
        "ph-skull",
        "ph-signature",
        "ph-smiley",
        "ph-sneaker",
        "ph-snowflake",
        "ph-sock",
        "ph-student",
        "ph-sword",
        "ph-target",
        "ph-terminal-window",
        "ph-thermometer",
        "ph-thumbs-down",
        "ph-thumbs-up",
        "ph-ticket",
        "ph-tire",
        "ph-toilet",
        "ph-tooth",
        "ph-toolbox",
        "ph-tractor",
        "ph-train",
        "ph-tram",
        "ph-trash",
        "ph-tree",
        "ph-treasure-chest",
        "ph-umbrella",
        "ph-user",
        "ph-watch",
        "ph-washing-machine",
        "ph-windmill",
        "ph-wrench",
        "ph-yin-yang",
    ];

    let toggle_dropdown = {
        let is_open = is_open.clone();
        Callback::from(move |_| {
            is_open.set(!*is_open);
        })
    };

    let close_dropdown = {
        let is_open = is_open.clone();
        Callback::from(move |_| {
            is_open.set(false);
        })
    };

    let selected_name = props.selected_icon.replace("ph-", "");

    html! {
        <div class="relative">
            <button
                type="button"
                onclick={toggle_dropdown}
                class="search-bar-input border text-sm rounded-lg w-full p-2.5 flex items-center justify-between"
            >
                <div class="flex items-center">
                    <i class={classes!("ph", props.selected_icon.clone(), "text-2xl", "mr-2")}></i>
                    <span class="item_container-text">{selected_name}</span>
                </div>
                <i class={classes!("ph", "ph-caret-down", "ml-2")}></i>
            </button>

            if *is_open {
                <div
                    class="absolute z-50 mt-1 w-full rounded-lg shadow-lg modal-container max-h-[400px] overflow-y-auto"
                    onclick={close_dropdown.clone()}
                >
                    <div class="grid grid-cols-5 gap-2 p-3">
                        {
                            icons.iter().map(|icon| {
                                let icon_name = icon.replace("ph-", "");
                                let on_select = props.on_select.clone();
                                let is_selected = &props.selected_icon == icon;
                                let onclick = {
                                    let icon = icon.to_string();
                                    Callback::from(move |_| on_select.emit(icon.clone()))
                                };
                                html! {
                                    <button
                                        type="button"
                                        {onclick}
                                        class={classes!(
                                            "flex", "flex-col", "items-center", "justify-center",
                                            "p-3", "rounded-lg", "hover:bg-gray-700", "transition-colors",
                                            "border", "border-gray-700",
                                            if is_selected { "bg-gray-700 ring-2 ring-blue-500" } else { "" }
                                        )}
                                        title={icon_name}
                                    >
                                        <i class={classes!("ph", icon.to_string(), "text-3xl")}></i>
                                    </button>
                                }
                            }).collect::<Html>()
                        }
                    </div>
                </div>
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct PodcastSelectorProps {
    pub selected_podcasts: Vec<i32>,
    pub on_select: Callback<Vec<i32>>,
    pub available_podcasts: Vec<Podcast>,
}

#[function_component(PodcastSelector)]
pub fn podcast_selector(props: &PodcastSelectorProps) -> Html {
    let is_open = use_state(|| false);
    let dropdown_ref = use_node_ref();

    // Handle clicking outside to close dropdown
    {
        let is_open = is_open.clone();
        let dropdown_ref = dropdown_ref.clone();

        use_effect_with(dropdown_ref.clone(), move |dropdown_ref| {
            let document = web_sys::window().unwrap().document().unwrap();
            let dropdown_element = dropdown_ref.cast::<HtmlElement>();

            let listener = EventListener::new(&document, "click", move |event| {
                if let Some(target) = event.target() {
                    if let Some(dropdown) = &dropdown_element {
                        if let Ok(node) = target.dyn_into::<web_sys::Node>() {
                            if !dropdown.contains(Some(&node)) {
                                is_open.set(false);
                            }
                        }
                    }
                }
            });

            || drop(listener)
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
            >
                <div class="flex items-center flex-grow">
                    if props.selected_podcasts.is_empty() {
                        <span class="flex-grow text-left">{"Filter by Podcasts (Optional)"}</span>
                    } else {
                        <span class="flex-grow text-left">
                            {format!("{} Podcast{} Selected",
                                props.selected_podcasts.len(),
                                if props.selected_podcasts.len() == 1 { "" } else { "s" }
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

            if *is_open {
                <div
                    class="absolute z-50 mt-1 w-full rounded-lg shadow-lg modal-container"
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

#[function_component(Playlists)]
pub fn playlists() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, _) = use_store::<UIState>();
    let modal_state = use_state(|| ModalState::Hidden);
    let selected_playlist_id = use_state(|| None::<i32>);
    let is_selection_mode = use_state(|| false);
    let selected_playlists = use_state(|| HashSet::<i32>::new());
    let is_loading_delete = use_state(|| false);

    // Form states for creating playlists
    let name = use_state(String::new);
    let description = use_state(String::new);
    let include_unplayed = use_state(|| true);
    let include_partially_played = use_state(|| true);
    let include_played = use_state(|| false);
    let min_duration = use_state(String::new);
    let max_duration = use_state(String::new);
    let sort_order = use_state(|| "date_desc".to_string());
    let group_by_podcast = use_state(|| false);
    let max_episodes = use_state(String::new);
    let icon_name = use_state(|| "ph-playlist".to_string());
    let play_progress_min = use_state(|| "".to_string());
    let play_progress_max = use_state(|| "".to_string());
    let time_filter_hours = use_state(|| "".to_string());

    // Loading state
    let loading = use_state(|| false);

    // Effect to load playlists
    {
        let dispatch = dispatch.clone();
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_playlists(&server_name, &api_key.unwrap(), user_id)
                            .await
                        {
                            Ok(playlist_response) => {
                                dispatch.reduce_mut(move |state| {
                                    state.playlists = Some(playlist_response.playlists);
                                });
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error fetching playlists: {:?}", e).into(),
                                );
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    let selected_podcasts = use_state(|| Vec::new());
    let available_podcasts = use_state(|| Vec::<Podcast>::new());
    let loading_podcasts = use_state(|| false);

    // Load podcasts when creating a playlist
    {
        let available_podcasts = available_podcasts.clone();
        let loading_podcasts = loading_podcasts.clone();
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

        use_effect_with(modal_state.clone(), move |modal_state| {
            if let ModalState::Create = **modal_state {
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
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error fetching podcasts: {:?}", e).into(),
                                );
                                loading_podcasts.set(false);
                            }
                        }
                    });
                }
            }
            || ()
        });
    }

    let on_create_click = {
        let modal_state = modal_state.clone();
        Callback::from(move |_| {
            modal_state.set(ModalState::Create);
        })
    };

    let toggle_selection_mode = {
        let is_selection_mode = is_selection_mode.clone();
        let selected_playlists = selected_playlists.clone();

        Callback::from(move |_| {
            is_selection_mode.set(!*is_selection_mode);
            if *is_selection_mode {
                selected_playlists.set(HashSet::new());
            }
        })
    };

    let on_toggle_select_playlist = {
        let selected_playlists = selected_playlists.clone();
        let playlists = state.playlists.clone();

        Callback::from(move |playlist_id: i32| {
            // Only toggle selection for non-system playlists
            if let Some(playlists_vec) = &playlists {
                if let Some(playlist) = playlists_vec.iter().find(|p| p.playlist_id == playlist_id)
                {
                    if playlist.is_system_playlist {
                        return; // Don't allow selection of system playlists
                    }
                }
            }

            selected_playlists.set({
                let mut set = (*selected_playlists).clone();
                if set.contains(&playlist_id) {
                    set.remove(&playlist_id);
                } else {
                    set.insert(playlist_id);
                }
                set
            });
        })
    };

    let on_delete_selected = {
        let selected_playlists = selected_playlists.clone();
        let modal_state = modal_state.clone();

        Callback::from(move |_| {
            if !selected_playlists.is_empty() {
                modal_state.set(ModalState::BulkDelete);
            }
        })
    };

    let on_modal_close = {
        let modal_state = modal_state.clone();
        Callback::from(move |_| {
            modal_state.set(ModalState::Hidden);
        })
    };

    let on_modal_background_click = {
        let on_modal_close = on_modal_close.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            // Check for DIV tag name instead of specific ID
            if element.tag_name() == "DIV" {
                e.prevent_default();
                on_modal_close.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.prevent_default();
        e.stop_propagation();
    });

    // Single playlist delete handler
    let on_delete_confirm = {
        let selected_playlist_id = selected_playlist_id.clone();
        let modal_state = modal_state.clone();
        let dispatch = dispatch.clone();

        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

        Callback::from(move |_| {
            if let (Some(playlist_id), Some(api_key), Some(user_id), Some(server_name)) = (
                *selected_playlist_id,
                api_key.clone(),
                user_id.clone(),
                server_name.clone(),
            ) {
                let dispatch = dispatch.clone();
                let modal_state = modal_state.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match pod_req::call_delete_playlist(
                        &server_name,
                        &api_key.clone().unwrap(),
                        user_id,
                        playlist_id,
                    )
                    .await
                    {
                        Ok(_) => {
                            // Refresh playlist list
                            if let Ok(playlists) = pod_req::call_get_playlists(
                                &server_name,
                                &api_key.unwrap(),
                                user_id,
                            )
                            .await
                            {
                                dispatch.reduce_mut(move |state| {
                                    state.playlists = Some(playlists.playlists);
                                });
                            }
                            dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Playlist deleted successfully".to_string());
                            });
                            modal_state.set(ModalState::Hidden);
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to delete playlist: {}", formatted_error));
                            });
                        }
                    }
                });
            }
        })
    };

    // Bulk delete selected playlists handler
    let on_bulk_delete_confirm = {
        let selected_playlists = selected_playlists.clone();
        let modal_state = modal_state.clone();
        let dispatch = dispatch.clone();
        let is_loading_delete = is_loading_delete.clone();
        let is_selection_mode = is_selection_mode.clone();

        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

        Callback::from(move |_| {
            if let (Some(api_key), Some(user_id), Some(server_name)) =
                (api_key.clone(), user_id.clone(), server_name.clone())
            {
                let selected_playlists_vec: Vec<i32> =
                    (*selected_playlists).iter().cloned().collect();
                if selected_playlists_vec.is_empty() {
                    return;
                }

                let dispatch = dispatch.clone();
                let modal_state = modal_state.clone();
                let is_loading_delete = is_loading_delete.clone();
                let is_selection_mode = is_selection_mode.clone();
                is_loading_delete.set(true);

                wasm_bindgen_futures::spawn_local(async move {
                    let mut success_count = 0;
                    let mut error_count = 0;

                    for playlist_id in &selected_playlists_vec {
                        match pod_req::call_delete_playlist(
                            &server_name,
                            &api_key.clone().unwrap(),
                            user_id,
                            *playlist_id,
                        )
                        .await
                        {
                            Ok(_) => {
                                success_count += 1;
                            }
                            Err(_) => {
                                error_count += 1;
                            }
                        }
                    }

                    // Refresh playlist list
                    if let Ok(playlists) =
                        pod_req::call_get_playlists(&server_name, &api_key.unwrap(), user_id).await
                    {
                        dispatch.reduce_mut(move |state| {
                            state.playlists = Some(playlists.playlists);
                        });
                    }

                    // Show result message
                    if error_count == 0 {
                        dispatch.reduce_mut(|state| {
                            state.info_message = Some(format!(
                                "{} playlist{} deleted successfully",
                                success_count,
                                if success_count == 1 { "" } else { "s" }
                            ));
                        });
                    } else if success_count == 0 {
                        dispatch.reduce_mut(|state| {
                            state.error_message = Some("Failed to delete playlists".to_string());
                        });
                    } else {
                        dispatch.reduce_mut(|state| {
                            state.info_message = Some(format!(
                                "{} playlist{} deleted, {} failed",
                                success_count,
                                if success_count == 1 { "" } else { "s" },
                                error_count
                            ));
                        });
                    }

                    modal_state.set(ModalState::Hidden);
                    is_loading_delete.set(false);
                    is_selection_mode.set(false);
                });
            }
        })
    };

    // Create playlist handler
    let on_create_submit = {
        let name = name.clone();
        let description = description.clone();
        let include_unplayed = include_unplayed.clone();
        let include_partially_played = include_partially_played.clone();
        let include_played = include_played.clone();
        let min_duration = min_duration.clone();
        let max_duration = max_duration.clone();
        let sort_order = sort_order.clone();
        let group_by_podcast = group_by_podcast.clone();
        let max_episodes = max_episodes.clone();
        let icon_name = icon_name.clone();
        let modal_state = modal_state.clone();
        let loading = loading.clone();
        let dispatch = dispatch.clone();
        let play_progress_min = play_progress_min.clone();
        let play_progress_max = play_progress_max.clone();
        let time_filter_hours = time_filter_hours.clone();
        let selected_pods_create = selected_podcasts.clone();

        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let selected_pods_call = selected_pods_create.clone();
            let play_progress_min_call = play_progress_min.clone();
            let play_progress_max_call = play_progress_max.clone();
            let time_filter_call = time_filter_hours.clone();
            let loading_clone = loading.clone();
            if let (Some(api_key), Some(user_id), Some(server_name)) =
                (api_key.clone(), user_id.clone(), server_name.clone())
            {
                loading.set(true);
                let playlist_request = CreatePlaylistRequest {
                    user_id,
                    name: (*name).clone(),
                    description: Some((*description).clone()),
                    podcast_ids: if selected_pods_call.is_empty() {
                        None
                    } else {
                        Some((*selected_pods_call).clone())
                    },
                    include_unplayed: *include_unplayed,
                    include_partially_played: *include_partially_played,
                    include_played: *include_played,
                    min_duration: min_duration.parse().ok(),
                    max_duration: max_duration.parse().ok(),
                    sort_order: (*sort_order).clone(),
                    group_by_podcast: *group_by_podcast,
                    max_episodes: max_episodes.parse().ok(),
                    icon_name: (*icon_name).clone(),
                    play_progress_min: play_progress_min_call.parse().ok(),
                    play_progress_max: play_progress_max_call.parse().ok(),
                    time_filter_hours: time_filter_call.parse().ok(),
                };

                let dispatch = dispatch.clone();
                let modal_state = modal_state.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match pod_req::call_create_playlist(
                        &server_name,
                        &api_key.clone().unwrap(),
                        playlist_request,
                    )
                    .await
                    {
                        Ok(_) => {
                            // Refresh playlist list
                            if let Ok(playlists) = pod_req::call_get_playlists(
                                &server_name,
                                &api_key.unwrap(),
                                user_id,
                            )
                            .await
                            {
                                dispatch.reduce_mut(move |state| {
                                    state.playlists = Some(playlists.playlists);
                                });
                            }
                            dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Playlist created successfully".to_string());
                            });
                            modal_state.set(ModalState::Hidden);
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to create playlist: {}", formatted_error));
                            });
                        }
                    }
                    loading_clone.set(false);
                });
            }
        })
    };

    let create_modal = html! {
        <div
            id="create-playlist-modal"
            tabindex="-1"
            aria-hidden="true"
            class="fixed inset-0 z-50 overflow-y-auto bg-black bg-opacity-25"
            onclick={on_modal_background_click.clone()}
        >
            <div class="flex min-h-full items-center justify-center p-4">
                <div class="modal-container relative w-full max-w-md rounded-lg shadow" onclick={stop_propagation.clone()}>
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Create New Playlist"}
                        </h3>
                        <button onclick={on_modal_close.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Name"}</label>
                                <input
                                    type="text"
                                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                    value={(*name).clone()}
                                    oninput={let name = name.clone(); Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        name.set(input.value());
                                    })}
                                />
                            </div>

                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Description"}</label>
                                <textarea
                                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                    value={(*description).clone()}
                                    oninput={let description = description.clone(); Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        description.set(input.value());
                                    })}
                                />
                            </div>

                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Icon"}</label>
                                <IconSelector
                                    selected_icon={(*icon_name).clone()}
                                    on_select={let icon_name = icon_name.clone(); Callback::from(move |new_icon| {
                                        icon_name.set(new_icon);
                                    })}
                                />
                            </div>

                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Filter by Podcasts"}</label>
                                {
                                    if *loading_podcasts {
                                        html! {
                                            <div class="flex justify-center p-4">
                                                <div class="loading-animation">
                                                    <div class="frame1"></div>
                                                    <div class="frame2"></div>
                                                    <div class="frame3"></div>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <PodcastSelector
                                                selected_podcasts={(*selected_podcasts).clone()}
                                                on_select={
                                                    let selected_podcasts = selected_podcasts.clone();
                                                    Callback::from(move |new_selection| {
                                                        selected_podcasts.set(new_selection);
                                                    })
                                                }
                                                available_podcasts={(*available_podcasts).clone()}
                                            />
                                        }
                                    }
                                }
                            </div>

                            // Episode Filters
                            // Fix the Episode Filters section
                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Episode Filters"}</label>
                                <div class="space-y-2">
                                    <div class="flex items-center">
                                        <input
                                            type="checkbox"
                                            class="mr-2"
                                            checked={*include_unplayed}
                                            onchange={let include_unplayed = include_unplayed.clone(); Callback::from(move |e: Event| {
                                                if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                    include_unplayed.set(input.checked());
                                                }
                                            })}
                                            onclick={Callback::from(move |e: MouseEvent| {
                                                e.stop_propagation(); // Explicitly stop propagation for this input
                                            })}
                                        />
                                        <span>{"Include Unplayed"}</span>
                                    </div>
                                    <div class="flex items-center">
                                        <input
                                            type="checkbox"
                                            class="mr-2"
                                            checked={*include_partially_played}
                                            onchange={let include_partially_played = include_partially_played.clone(); Callback::from(move |e: Event| {
                                                if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                    include_partially_played.set(input.checked());
                                                }
                                            })}
                                            onclick={Callback::from(move |e: MouseEvent| {
                                                e.stop_propagation(); // Explicitly stop propagation for this input
                                            })}
                                        />
                                        <span>{"Include Partially Played"}</span>
                                    </div>
                                    <div class="flex items-center">
                                        <input
                                            type="checkbox"
                                            class="mr-2"
                                            checked={*include_played}
                                            onchange={let include_played = include_played.clone(); Callback::from(move |e: Event| {
                                                if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                    include_played.set(input.checked());
                                                }
                                            })}
                                            onclick={Callback::from(move |e: MouseEvent| {
                                                e.stop_propagation(); // Explicitly stop propagation for this input
                                            })}
                                        />
                                        <span>{"Include Played"}</span>
                                    </div>
                                </div>
                            </div>

                            // Duration Range
                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Duration Range (minutes)"}</label>
                                <div class="grid grid-cols-2 gap-4">
                                    <input
                                        type="number"
                                        placeholder="Min"
                                        class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                        value={(*min_duration).clone()}
                                        oninput={let min_duration = min_duration.clone(); Callback::from(move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            min_duration.set(input.value());
                                        })}
                                    />
                                    <input
                                        type="number"
                                        placeholder="Max"
                                        class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                        value={(*max_duration).clone()}
                                        oninput={let max_duration = max_duration.clone(); Callback::from(move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            max_duration.set(input.value());
                                        })}
                                    />
                                </div>
                            </div>


                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Play Progress Range (%)"}</label>
                                <div class="grid grid-cols-2 gap-4">
                                    <input
                                        type="number"
                                        min="0"
                                        max="100"
                                        placeholder="Min %"
                                        disabled={!*include_partially_played}
                                        class={classes!(
                                            "search-bar-input",
                                            "border",
                                            "text-sm",
                                            "rounded-lg",
                                            "block",
                                            "w-full",
                                            "p-2.5",
                                            if !*include_partially_played { "opacity-50 cursor-not-allowed" } else { "" }
                                        )}
                                        value={(*play_progress_min).clone()}
                                        oninput={let play_progress_min = play_progress_min.clone();
                                            Callback::from(move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                play_progress_min.set(input.value());
                                            })
                                        }
                                    />
                                    <input
                                        type="number"
                                        min="0"
                                        max="100"
                                        placeholder="Max %"
                                        disabled={!*include_partially_played}
                                        class={classes!(
                                            "search-bar-input",
                                            "border",
                                            "text-sm",
                                            "rounded-lg",
                                            "block",
                                            "w-full",
                                            "p-2.5",
                                            if !*include_partially_played { "opacity-50 cursor-not-allowed" } else { "" }
                                        )}
                                        value={(*play_progress_max).clone()}
                                        oninput={let play_progress_max = play_progress_max.clone(); Callback::from(move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            play_progress_max.set(input.value());
                                        })}
                                    />
                                </div>
                            </div>

                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Time Filter (hours)"}</label>
                                <input
                                    type="number"
                                    min="0"
                                    placeholder="Hours"
                                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                    value={(*time_filter_hours).clone()}
                                    oninput={let time_filter_hours = time_filter_hours.clone(); Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        time_filter_hours.set(input.value());
                                    })}
                                />
                            </div>

                            // Sort Order
                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Sort Order"}</label>
                                <select
                                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                    onchange={let sort_order = sort_order.clone(); Callback::from(move |e: Event| {
                                        let select: HtmlInputElement = e.target_unchecked_into();
                                        sort_order.set(select.value());
                                    })}
                                >
                                    <option value="date_desc">{"Newest First"}</option>
                                    <option value="date_asc">{"Oldest First"}</option>
                                    <option value="duration_desc">{"Longest First"}</option>
                                    <option value="duration_asc">{"Shortest First"}</option>
                                </select>
                            </div>

                            // Max Episodes
                            <div>
                                <label class="block mb-2 text-sm font-medium">{"Max Episodes"}</label>
                                <input
                                    type="number"
                                    class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                    value={(*max_episodes).clone()}
                                    oninput={let max_episodes = max_episodes.clone(); Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        max_episodes.set(input.value());
                                    })}
                                />
                            </div>

                            <div class="flex items-center">
                                <input
                                    type="checkbox"
                                    class="mr-2"
                                    checked={*group_by_podcast}
                                    onchange={let group_by_podcast = group_by_podcast.clone(); Callback::from(move |e: Event| {
                                        if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                            group_by_podcast.set(input.checked());
                                        }
                                    })}
                                    onclick={Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation(); // Explicitly stop propagation for this input
                                    })}
                                />
                                <span>{"Group by Podcast"}</span>
                            </div>

                            <button
                                type="submit"
                                onclick={on_create_submit}
                                class="download-button w-full focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-5 py-2.5 text-center"
                                disabled={*loading}
                            >
                                {
                                    if *loading {
                                        html! {
                                            <div class="flex items-center justify-center">
                                                <i class="ph ph-circle-notch animate-spin mr-2"></i>
                                                {"Creating..."}
                                            </div>
                                        }
                                    } else {
                                        html! { "Create Playlist" }
                                    }
                                }
                            </button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    // Define modals
    let delete_modal = html! {
        <div class="modal-background fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50" onclick={on_modal_background_click.clone()}>
            <div class="modal-container relative rounded-lg shadow-lg max-w-md w-full mx-4" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                <div class="item-container rounded-lg p-6">
                    <h2 class="text-2xl font-bold mb-4 item_container-text">{"Delete Playlist"}</h2>
                    <p class="item_container-text mb-6">{"Are you sure you want to delete this playlist?"}</p>
                    <div class="flex justify-end space-x-4">
                        <button
                            class="item-container-button py-2 px-4 rounded"
                            onclick={on_modal_close.clone()}
                        >
                            {"Cancel"}
                        </button>
                        <button
                            class="item-container-button py-2 px-4 rounded"
                            onclick={on_delete_confirm.clone()}
                        >
                            {"Delete"}
                        </button>
                    </div>
                </div>
            </div>
        </div>
    };

    let bulk_delete_modal = html! {
        <div class="modal-background fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50" onclick={on_modal_background_click.clone()}>
            <div class="modal-container relative rounded-lg shadow-lg max-w-md w-full mx-4" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                <div class="item-container rounded-lg p-6">
                    <div class="flex flex-col space-y-4">
                        <h2 class="text-2xl font-bold item_container-text">{"Delete Playlists"}</h2>

                        <p class="item_container-text">
                            {format!("Are you sure you want to delete {} selected playlist{}?",
                                selected_playlists.len(),
                                if selected_playlists.len() == 1 { "" } else { "s" }
                            )}
                        </p>

                        <div class="flex justify-end space-x-4 mt-4">
                            <button
                                class="item-container-button py-2 px-4 rounded"
                                onclick={on_modal_close.clone()}
                                disabled={*is_loading_delete}
                            >
                                {"Cancel"}
                            </button>
                            <button
                                class="item-container-button py-2 px-4 rounded"
                                onclick={on_bulk_delete_confirm.clone()}
                                disabled={*is_loading_delete}
                            >
                                {
                                    if *is_loading_delete {
                                        html! {
                                            <div class="flex items-center justify-center">
                                                <i class="ph ph-circle-notch animate-spin mr-2"></i>
                                                {"Deleting..."}
                                            </div>
                                        }
                                    } else {
                                        html! { "Delete" }
                                    }
                                }
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    };

    let history = BrowserHistory::new();

    let tog_state = state.clone();
    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />

                // Header with action buttons - responsive layout
                <div class="flex flex-wrap justify-between items-center gap-4 mb-6">
                    <h1 class="text-2xl font-bold item_container-text">{"Smart Playlists"}</h1>
                    <div class="flex flex-wrap gap-2">
                        {
                            if *is_selection_mode {
                                html! {
                                    <>
                                        <button
                                            class="item-container-button py-2 px-4 rounded flex items-center"
                                            onclick={toggle_selection_mode.clone()}
                                        >
                                            <i class="ph ph-x text-xl mr-2"></i>
                                            {"Cancel"}
                                        </button>
                                        <button
                                            class="item-container-button py-2 px-4 rounded flex items-center"
                                            onclick={on_delete_selected.clone()}
                                            disabled={selected_playlists.is_empty()}
                                        >
                                            <i class="ph ph-trash text-xl mr-2"></i>
                                            {format!("Delete ({})", selected_playlists.len())}
                                        </button>
                                    </>
                                }
                            } else {
                                html! {
                                    <>
                                        <button
                                            class="item-container-button py-2 px-4 rounded flex items-center"
                                            onclick={toggle_selection_mode.clone()}
                                        >
                                            <i class="ph ph-selection-plus text-xl mr-2"></i>
                                            {"Select"}
                                        </button>
                                        <button
                                            class="item-container-button py-2 px-4 rounded flex items-center"
                                            onclick={on_create_click}
                                        >
                                            <i class="ph ph-plus text-xl mr-2"></i>
                                            {"Create Playlist"}
                                        </button>
                                    </>
                                }
                            }
                        }
                    </div>
                </div>

                // Info banner for selection mode
                {
                    if *is_selection_mode {
                        html! {
                            <div class="mb-4 p-3 bg-blue-100 text-blue-800 rounded-lg flex items-center">
                                <i class="ph ph-info text-xl mr-2"></i>
                                <span>{"System playlists cannot be deleted and are not selectable."}</span>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                // Playlists grid
                {
                    if let Some(playlists) = &tog_state.playlists {
                        if playlists.is_empty() {
                            empty_message(
                                "No Playlists",
                                "Create a new playlist to get started"
                            )
                        } else {
                            let playlists_snapshot = playlists.clone();
                            html! {
                                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
                                    {
                                        playlists_snapshot.iter().map(|playlist| {
                                            let history_clone = history.clone();
                                            let playlist_id = playlist.playlist_id;
                                            let selected_playlist_id = selected_playlist_id.clone();
                                            let modal_state = modal_state.clone();
                                            let is_selected = selected_playlists.contains(&playlist_id);
                                            let on_toggle_select = on_toggle_select_playlist.clone();
                                            let playlist_clone = playlist.clone();

                                            let on_delete = Callback::from(move |e: MouseEvent| {
                                                e.stop_propagation();
                                                // Don't allow deletion of system playlists
                                                if !playlist_clone.is_system_playlist {
                                                    selected_playlist_id.set(Some(playlist_id));
                                                    modal_state.set(ModalState::Delete);
                                                }
                                            });

                                            let is_selection_mode_clone = is_selection_mode.clone();
                                            let on_tog = on_toggle_select.clone();
                                            let on_select = Callback::from(move |_| {
                                                if !*is_selection_mode_clone {
                                                    let route = format!("/playlist/{}", playlist_id);
                                                    history_clone.push(route);
                                                } else {
                                                    // In selection mode, clicking the card toggles selection
                                                    on_tog.emit(playlist_id);
                                                }
                                            });

                                            html! {
                                                <PlaylistCard
                                                    playlist={playlist.clone()}
                                                    {on_delete}
                                                    {on_select}
                                                    is_selectable={*is_selection_mode && !playlist.is_system_playlist}
                                                    is_selected={is_selected}
                                                    on_toggle_select={on_toggle_select.clone()}
                                                />
                                            }
                                        }).collect::<Html>()
                                    }
                                </div>
                            }
                        }
                    } else {
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
                    }
                }

                // Modals
                {
                    match *modal_state {
                        ModalState::Create => create_modal,
                        ModalState::Delete => delete_modal,
                        ModalState::BulkDelete => bulk_delete_modal,
                        ModalState::Hidden => html! {},
                    }
                }

                // Audio player if something is playing
                if let Some(audio_props) = &audio_state.currently_playing {
                    <AudioPlayer
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
                    />
                }
            </div>
            <App_drawer />
        </>
    }
}
