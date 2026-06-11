use crate::components::context::{AppState, EpisodeDetailState, EpisodeNavigationState, EpisodeStatusState, NotificationState, PageLoadState, PodcastFeedState, SearchState, UIState};
#[cfg(not(feature = "server_build"))]
use crate::pages::downloads_tauri::{
    download_file, remove_episode_from_local_db, update_local_database, update_podcast_database,
};
use i18nrs::yew::use_translation;
use crate::requests::episode::Episode;

use crate::components::gen_funcs::format_error_message;
use crate::components::gen_funcs::format_time;
use crate::components::notification_center::{NotificationCenter, ToastNotification};
use crate::components::safehtml::SafeHtml;
use crate::requests::pod_req::{
    call_download_episode, call_mark_episode_completed, call_mark_episode_uncompleted,
    call_queue_episode, call_remove_downloaded_episode, call_remove_queued_episode,
    call_remove_saved_episode, call_save_episode, DownloadEpisodeRequest,
    MarkEpisodeCompletedRequest, QueuePodcastRequest, SavePodcastRequest,
};
#[cfg(not(feature = "server_build"))]
use crate::requests::pod_req::{
    call_get_episode_metadata, call_get_podcast_details, EpisodeRequest,
};
use crate::requests::search_pods::{
    call_get_podcast_info, call_youtube_search, test_connection, YouTubeSearchResults,
};
use gloo_events::EventListener;
use gloo_timers::callback::Timeout;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;
use web_sys::{window, Element, HtmlInputElement, KeyboardEvent, MouseEvent};
use yew::prelude::*;
use yew::Callback;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct FallbackImageProps {
    pub src: String,
    pub alt: String,
    pub class: Option<String>,
    #[prop_or_default]
    pub onclick: Option<Callback<MouseEvent>>,
    #[prop_or_default]
    pub style: Option<String>,
    #[prop_or("lazy".to_string())]
    pub loading: String,
    #[prop_or("async".to_string())]
    pub decoding: String,
}

#[function_component(FallbackImage)]
pub fn fallback_image(props: &FallbackImageProps) -> Html {
    let image_ref = use_node_ref();
    let has_error = use_state(|| false);
    let server_name_sel = use_selector(|state: &AppState| {
        state.auth_details.as_ref().map(|ud| ud.server_name.clone()).unwrap_or_default()
    });
    let server_name = (*server_name_sel).clone();

    const FALLBACK_IMAGE: &str = "/static/assets/favicon.png";

    // Just use the original src without timestamps; treat empty src as immediate fallback
    let image_src = use_state(|| {
        if props.src.is_empty() {
            FALLBACK_IMAGE.to_string()
        } else {
            props.src.clone()
        }
    });

    // Update src when props.src changes
    {
        let image_src = image_src.clone();
        let props_src = props.src.clone();
        use_effect_with(props_src, move |src| {
            if src.is_empty() {
                image_src.set(FALLBACK_IMAGE.to_string());
            } else {
                image_src.set(src.clone());
            }
            || ()
        });
    }

    // Create a proxied URL from the original source; skip proxy for empty URLs
    let proxied_url = {
        let original_url = props.src.clone();
        if original_url.is_empty() {
            FALLBACK_IMAGE.to_string()
        } else {
            format!(
                "{}/api/proxy/image?url={}",
                server_name,
                urlencoding::encode(&original_url)
            )
        }
    };

    // Handle image load error
    let on_error = {
        let has_error = has_error.clone();
        let image_src = image_src.clone();
        let proxied_url = proxied_url.clone();
        Callback::from(move |_: Event| {
            if !*has_error {
                // First error - switch to proxied URL
                has_error.set(true);
                image_src.set(proxied_url.clone());
                web_sys::console::log_1(
                    &format!("Image load failed, switching to proxy: {}", proxied_url).into(),
                );
            }
        })
    };

    // If we have a click handler, pass it through
    let onclick = props.onclick.clone();

    html! {
        <img
            ref={image_ref}
            src={(*image_src).clone()}
            alt={props.alt.clone()}
            class={props.class.clone().unwrap_or_default()}
            style={props.style.clone().unwrap_or_default()}
            loading={props.loading.clone()}
            decoding={props.decoding.clone()}
            // DON'T ADD CROSSORIGIN - it causes CORS issues with NPR's CDN
            onerror={on_error}
            onclick={onclick}
        />
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct ErrorMessageProps {
    pub error_message: UseStateHandle<Option<String>>,
}

#[function_component(UseScrollToTop)]
pub fn use_scroll_to_top() -> Html {
    let history = BrowserHistory::new();
    use_effect_with((), move |_| {
        // Create a closure that will be called on history change
        // Create a callback to scroll to the top of the page when the route changes
        let callback = history.listen(move || {
            web_sys::window().unwrap().scroll_to_with_x_and_y(0.0, 0.0);
        });

        // Cleanup function: This will be executed when the component unmounts
        // or the dependencies of the effect change.
        move || drop(callback)
    });

    html! {}
}

#[function_component(ErrorMessage)]
pub fn error_message(props: &ErrorMessageProps) -> Html {
    // Your existing logic here...
    let error_message = use_state(|| None::<String>);

    {
        let error_message = error_message.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let error_message_clone = error_message.clone();
            let closure = Closure::wrap(Box::new(move |_event: Event| {
                error_message_clone.set(None);
            }) as Box<dyn Fn(_)>);

            if error_message.is_some() {
                document
                    .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                    .unwrap();
            }

            // Return cleanup function
            move || {
                if error_message.is_some() {
                    document
                        .remove_event_listener_with_callback(
                            "click",
                            closure.as_ref().unchecked_ref(),
                        )
                        .unwrap();
                }
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }

    if let Some(error) = props.error_message.as_ref() {
        html! {
            <div class="error-snackbar">{ error }</div>
        }
    } else {
        html! {}
    }
}

#[allow(non_camel_case_types)]
#[function_component(Search_nav)]
pub fn search_bar() -> Html {
    let (i18n, _) = use_translation();
    let i18n_podcast_index = i18n.t("gen_components.podcast_index").to_string();
    let history = BrowserHistory::new();
    // Selective subscription — only re-render when server_details changes (login/logout),
    // not on every episode save/download/queue action.
    let server_details_sel = use_selector(|state: &AppState| state.server_details.clone());
    let server_details = (*server_details_sel).clone();
    let podcast_value = use_state(|| "".to_string());
    let search_index = use_state(|| "podcast_index".to_string());
    let is_submitting = use_state(|| false);

    // Mobile takeover state
    let is_mobile = use_state(|| false);
    let expanded = use_state(|| false);
    let src_menu_open = use_state(|| false);
    let mobile_input_ref = use_node_ref();

    let history_clone = history.clone();
    let podcast_value_clone = podcast_value.clone();
    let search_index_clone = search_index.clone();

    let handle_submit = {
        let is_submitting = is_submitting.clone();
        let server_details = server_details.clone();
        let history = history_clone.clone();
        let podcast_value = podcast_value_clone.clone();
        let search_index = search_index_clone.clone();

        move || {
            if *is_submitting {
                return;
            }
            is_submitting.set(true);
            let api_url = server_details.as_ref().map(|ud| ud.api_url.clone());
            let history = history.clone();
            let search_value = podcast_value.clone();
            let search_index = search_index.clone();
            let is_submitting_clone = is_submitting.clone();

            wasm_bindgen_futures::spawn_local(async move {
                Dispatch::<PageLoadState>::global().reduce_mut(|state| state.is_loading = Some(true));
                if *search_index == "youtube" {
                    match call_youtube_search(&search_value, &api_url.unwrap()).await {
                        Ok(yt_results) => {
                            let search_results = YouTubeSearchResults {
                                channels: yt_results.results,
                                videos: Vec::new(),
                            };
                            Dispatch::<SearchState>::global().reduce_mut(|state| {
                                state.youtube_search_results = Some(search_results);
                            });
                            Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                                state.is_loading = Some(false);
                            });
                            history.push("/youtube_layout");
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                                state.is_loading = Some(false);
                            });
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("YouTube search error: {}", formatted_error));
                            });
                        }
                    }
                } else {
                    match call_get_podcast_info(&search_value, &api_url.unwrap(), &search_index)
                        .await
                    {
                        Ok(search_results) => {
                            Dispatch::<SearchState>::global().reduce_mut(move |state| {
                                state.search_results = Some(search_results);
                            });
                            Dispatch::<PodcastFeedState>::global().reduce_mut(|state| {
                                state.podcast_added = Some(false);
                            });
                            Dispatch::<PageLoadState>::global()
                                .reduce_mut(|state| state.is_loading = Some(false));
                            history.push("/pod_layout");
                        }
                        Err(_) => {
                            Dispatch::<PageLoadState>::global()
                                .reduce_mut(|state| state.is_loading = Some(false));
                        }
                    }
                }
                is_submitting_clone.set(false);
            });
        }
    };

    let on_submit = {
        let handle_submit = handle_submit.clone();
        let expanded = expanded.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            expanded.set(false);
            handle_submit();
        })
    };

    let on_input_change = {
        let podcast_value = podcast_value.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            podcast_value.set(input.value());
        })
    };

    let on_clear_input = {
        let podcast_value = podcast_value.clone();
        Callback::from(move |_: MouseEvent| podcast_value.set("".to_string()))
    };

    let (_, ui_dispatch) = use_store::<UIState>();
    let queue_count_sel = use_selector(|state: &EpisodeStatusState| {
        state.queued_episodes.as_ref().map(|q| q.episodes.len()).unwrap_or(0)
    });
    let queue_count = *queue_count_sel;

    let toggle_queue = {
        let ui_dispatch = ui_dispatch.clone();
        Callback::from(move |_: MouseEvent| {
            ui_dispatch.reduce_mut(|s| s.queue_panel_open = !s.queue_panel_open);
        })
    };

    // Desktop source picker dropdown
    let src_dropdown_open = use_state(|| false);

    let toggle_src_dropdown = {
        let src_dropdown_open = src_dropdown_open.clone();
        Callback::from(move |_: MouseEvent| src_dropdown_open.set(!*src_dropdown_open))
    };

    let select_src = {
        let search_index = search_index.clone();
        let src_dropdown_open = src_dropdown_open.clone();
        move |val: &'static str| {
            search_index.set(val.to_string());
            src_dropdown_open.set(false);
        }
    };

    let select_podcast_index = {
        let s = select_src.clone();
        Callback::from(move |_: MouseEvent| s("podcast_index"))
    };
    let select_itunes = {
        let s = select_src.clone();
        Callback::from(move |_: MouseEvent| s("itunes"))
    };
    let select_youtube = {
        let s = select_src.clone();
        Callback::from(move |_: MouseEvent| s("youtube"))
    };

    // Mobile takeover callbacks
    let on_expand = {
        let expanded = expanded.clone();
        Callback::from(move |_: MouseEvent| expanded.set(true))
    };

    let on_collapse = {
        let expanded = expanded.clone();
        let src_menu_open = src_menu_open.clone();
        Callback::from(move |_: MouseEvent| {
            expanded.set(false);
            src_menu_open.set(false);
        })
    };

    let on_toggle_src_menu = {
        let src_menu_open = src_menu_open.clone();
        Callback::from(move |_: MouseEvent| src_menu_open.set(!*src_menu_open))
    };

    let on_src_menu_mouseleave = {
        let src_menu_open = src_menu_open.clone();
        Callback::from(move |_: MouseEvent| src_menu_open.set(false))
    };

    let select_podcast_index_mobile = {
        let search_index = search_index.clone();
        let src_menu_open = src_menu_open.clone();
        Callback::from(move |_: MouseEvent| {
            search_index.set("podcast_index".to_string());
            src_menu_open.set(false);
        })
    };

    let select_itunes_mobile = {
        let search_index = search_index.clone();
        let src_menu_open = src_menu_open.clone();
        Callback::from(move |_: MouseEvent| {
            search_index.set("itunes".to_string());
            src_menu_open.set(false);
        })
    };

    let select_youtube_mobile = {
        let search_index = search_index.clone();
        let src_menu_open = src_menu_open.clone();
        Callback::from(move |_: MouseEvent| {
            search_index.set("youtube".to_string());
            src_menu_open.set(false);
        })
    };

    let on_keydown_takeover = {
        let expanded = expanded.clone();
        let src_menu_open = src_menu_open.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Escape" {
                if *src_menu_open {
                    src_menu_open.set(false);
                } else {
                    expanded.set(false);
                }
            }
        })
    };

    // Viewport resize → update is_mobile (820px breakpoint)
    {
        let is_mobile = is_mobile.clone();
        use_effect_with((), move |_| {
            let w = web_sys::window().unwrap();
            let width = w.inner_width().unwrap().as_f64().unwrap_or(1920.0);
            is_mobile.set(width < 820.0);
            let is_mobile_r = is_mobile.clone();
            let w_r = w.clone();
            let listener = EventListener::new(&w, "resize", move |_| {
                let width = w_r.inner_width().unwrap().as_f64().unwrap_or(1920.0);
                is_mobile_r.set(width < 820.0);
            });
            move || drop(listener)
        });
    }

    // Reset expanded/src_menu_open when viewport returns to desktop width
    {
        let expanded = expanded.clone();
        let src_menu_open = src_menu_open.clone();
        let is_mobile_val = *is_mobile;
        use_effect_with(is_mobile_val, move |&mobile| {
            if !mobile {
                expanded.set(false);
                src_menu_open.set(false);
            }
            || ()
        });
    }

    // Focus mobile input ~60ms after expand to let the morph land
    {
        let mobile_input_ref = mobile_input_ref.clone();
        let expanded_val = *expanded;
        use_effect_with(expanded_val, move |&exp| {
            if exp {
                let r = mobile_input_ref.clone();
                Timeout::new(60, move || {
                    if let Some(input) = r.cast::<HtmlInputElement>() {
                        let _ = input.focus();
                    }
                })
                .forget();
            }
            || ()
        });
    }

    // Toggle body class so CSS can hide the fixed drawer-icon while expanded
    {
        let expanded_val = *expanded;
        use_effect_with(expanded_val, move |&exp| {
            if let Some(body) = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.body())
            {
                if exp {
                    let _ = body.class_list().add_1("search-takeover-open");
                } else {
                    let _ = body.class_list().remove_1("search-takeover-open");
                }
            }
            move || {
                if let Some(body) = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.body())
                {
                    let _ = body.class_list().remove_1("search-takeover-open");
                }
            }
        });
    }

    let search_index_display = match search_index.as_str() {
        "podcast_index" => "Podcast Index",
        "itunes" => "iTunes",
        "youtube" => "YouTube",
        _ => "Unknown",
    };

    let src_icon_class = match search_index.as_str() {
        "itunes" => "ph ph-apple-logo",
        "youtube" => "ph ph-youtube-logo",
        _ => "ph ph-globe-hemisphere-west",
    };

    let search_src_placeholder = match search_index.as_str() {
        "podcast_index" => "Search Podcast Index\u{2026}".to_string(),
        "itunes" => "Search iTunes\u{2026}".to_string(),
        "youtube" => "Search YouTube\u{2026}".to_string(),
        _ => "Search\u{2026}".to_string(),
    };

    html! {
        <div class="episodes-container w-full">
            <div class={classes!("topbar", (*expanded && *is_mobile).then_some("is-expanded"))}>
                if !*is_mobile {
                    // ── Desktop layout ──────────────────────────────────────
                    <div class="topbar-left">
                        // spacer — drawer-icon buttons are fixed-position at z-49
                    </div>
                    <form class="topbar-right" onsubmit={on_submit.clone()}>
                        <button type="button" class="iconbtn topbar-queue-btn"
                                onclick={toggle_queue.clone()} title="Queue">
                            <i class="ph ph-queue"></i>
                            if queue_count > 0 {
                                <span class="topbar-queue-badge">{ queue_count }</span>
                            }
                        </button>
                        <div style="position: relative;">
                            <button type="button" class="src-source"
                                    onclick={toggle_src_dropdown.clone()}>
                                <span>{ search_index_display }</span>
                                <i class="ph ph-caret-down"></i>
                            </button>
                            if *src_dropdown_open {
                                <div class="src-dropdown">
                                    <button type="button"
                                        class={classes!("src-dropdown-item", (*search_index == "podcast_index").then_some("is-active"))}
                                        onclick={select_podcast_index.clone()}>
                                        { &i18n_podcast_index }
                                    </button>
                                    <button type="button"
                                        class={classes!("src-dropdown-item", (*search_index == "itunes").then_some("is-active"))}
                                        onclick={select_itunes.clone()}>
                                        {"iTunes"}
                                    </button>
                                    <button type="button"
                                        class={classes!("src-dropdown-item", (*search_index == "youtube").then_some("is-active"))}
                                        onclick={select_youtube.clone()}>
                                        {"YouTube"}
                                    </button>
                                </div>
                            }
                        </div>
                        <div class="src-input">
                            <input
                                type="search"
                                placeholder="Search New Shows\u{2026}"
                                value={(*podcast_value).clone()}
                                oninput={on_input_change.clone()}
                            />
                            if !podcast_value.is_empty() {
                                <button type="button" class="src-input-clear"
                                        onclick={on_clear_input.clone()}>
                                    <i class="ph ph-x"></i>
                                </button>
                            }
                        </div>
                        <button type="submit" class="iconbtn" title="Search">
                            <i class="ph ph-magnifying-glass"></i>
                        </button>
                        <NotificationCenter />
                    </form>
                } else if !*expanded {
                    // ── Mobile idle ─────────────────────────────────────────
                    <div class="topbar-left">
                        // spacer
                    </div>
                    <div class="topbar-right">
                        <button type="button" class="iconbtn topbar-queue-btn"
                                onclick={toggle_queue.clone()} title="Queue">
                            <i class="ph ph-queue"></i>
                            if queue_count > 0 {
                                <span class="topbar-queue-badge">{ queue_count }</span>
                            }
                        </button>
                        <button type="button" class="iconbtn search-trigger"
                                onclick={on_expand.clone()}
                                title="Search" aria-label="Open search">
                            <i class="ph ph-magnifying-glass"></i>
                        </button>
                    </div>
                } else {
                    // ── Mobile expanded (takeover) ──────────────────────────
                    <form class="topbar-search-takeover"
                          onsubmit={on_submit.clone()}
                          onkeydown={on_keydown_takeover.clone()}>
                        <button type="button" class="iconbtn"
                                onclick={on_collapse.clone()}
                                title="Close search" aria-label="Close search">
                            <i class="ph ph-arrow-left"></i>
                        </button>
                        <div style="position: relative;">
                            <button type="button"
                                    class="src-source src-source-icon"
                                    onclick={on_toggle_src_menu.clone()}
                                    title={format!("Search source: {}. Click to change.", search_index_display)}
                                    aria-label={format!("Search source: {}. Click to change.", search_index_display)}>
                                <i class={src_icon_class}></i>
                                <i class="ph ph-caret-down src-combined-caret"></i>
                            </button>
                            if *src_menu_open {
                                <div class="src-menu src-menu-takeover"
                                     onmouseleave={on_src_menu_mouseleave.clone()}>
                                    <button type="button"
                                            class={classes!("src-menu-item", (*search_index == "podcast_index").then_some("is-active"))}
                                            onclick={select_podcast_index_mobile.clone()}
                                            aria-current={(*search_index == "podcast_index").then_some("true")}>
                                        <i class="ph ph-globe-hemisphere-west"></i>
                                        <span>{ &i18n_podcast_index }</span>
                                    </button>
                                    <button type="button"
                                            class={classes!("src-menu-item", (*search_index == "itunes").then_some("is-active"))}
                                            onclick={select_itunes_mobile.clone()}
                                            aria-current={(*search_index == "itunes").then_some("true")}>
                                        <i class="ph ph-apple-logo"></i>
                                        <span>{"iTunes"}</span>
                                    </button>
                                    <button type="button"
                                            class={classes!("src-menu-item", (*search_index == "youtube").then_some("is-active"))}
                                            onclick={select_youtube_mobile.clone()}
                                            aria-current={(*search_index == "youtube").then_some("true")}>
                                        <i class="ph ph-youtube-logo"></i>
                                        <span>{"YouTube"}</span>
                                    </button>
                                </div>
                            }
                        </div>
                        <div class="src-input">
                            <input
                                ref={mobile_input_ref.clone()}
                                type="text"
                                placeholder={search_src_placeholder}
                                value={(*podcast_value).clone()}
                                oninput={on_input_change.clone()}
                            />
                            if !podcast_value.is_empty() {
                                <button type="button" class="src-input-clear"
                                        onclick={on_clear_input.clone()}>
                                    <i class="ph ph-x"></i>
                                </button>
                            }
                        </div>
                        <button type="submit" class="iconbtn"
                                title="Search" aria-label="Submit search">
                            <i class="ph ph-magnifying-glass"></i>
                        </button>
                    </form>
                }
            </div>
            <ToastNotification />
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct FirstAdminModalProps {
    pub on_submit: Callback<AdminSetupData>,
}

#[derive(Clone, Debug)]
pub struct AdminSetupData {
    pub username: String,
    pub password: String,
    pub email: String,
    pub fullname: String,
}

#[function_component(FirstAdminModal)]
pub fn first_admin_modal(props: &FirstAdminModalProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_welcome_to_pinepods = i18n.t("gen_components.welcome_to_pinepods").to_string();
    let i18n_setup_admin_hint = i18n.t("gen_components.setup_admin_hint").to_string();
    let i18n_full_name = i18n.t("gen_components.full_name").to_string();
    let i18n_username = i18n.t("gen_components.username").to_string();
    let i18n_email = i18n.t("gen_components.email").to_string();
    let i18n_password = i18n.t("gen_components.password").to_string();
    let i18n_create_admin_account = i18n.t("gen_components.create_admin_account").to_string();
    let username = use_state(|| String::new());
    let password = use_state(|| String::new());
    let email = use_state(|| String::new());
    let fullname = use_state(|| String::new());
    let validation_message = use_state(|| None::<String>);

    let onsubmit = {
        let username = username.clone();
        let password = password.clone();
        let email = email.clone();
        let fullname = fullname.clone();
        let validation_message = validation_message.clone();
        let on_submit = props.on_submit.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            // Basic validation
            if username.is_empty() || password.is_empty() || email.is_empty() || fullname.is_empty()
            {
                validation_message.set(Some("All fields are required".to_string()));
                return;
            }

            if password.len() < 8 {
                validation_message.set(Some("Password must be at least 8 characters".to_string()));
                return;
            }

            // Email validation
            if !email.contains('@') {
                validation_message.set(Some("Please enter a valid email address".to_string()));
                return;
            }

            let data = AdminSetupData {
                username: (*username).clone(),
                password: (*password).clone(),
                email: (*email).clone(),
                fullname: (*fullname).clone(),
            };

            on_submit.emit(data);
        })
    };

    html! {
        <div class="fixed inset-0 bg-black bg-opacity-50 z-50 flex items-center justify-center">
            <div class="bg-container-background rounded-lg p-8 max-w-md w-full mx-4 shadow-xl">
                <h2 class="text-2xl font-bold mb-6 text-text-color">{ &i18n_welcome_to_pinepods }</h2>
                <p class="mb-6 text-text-color">{ &i18n_setup_admin_hint }</p>

                <form onsubmit={onsubmit} class="space-y-4">
                    <div>
                        <label for="fullname" class="block text-sm font-medium text-text-color mb-1">
                            { &i18n_full_name }
                        </label>
                        <input
                            type="text"
                            id="fullname"
                            value={(*fullname).clone()}
                            onchange={let fullname = fullname.clone(); move |e: Event| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    fullname.set(input.value());
                                }
                            }}
                            class="search-bar-input w-full px-3 py-2 border rounded-md focus:outline-none focus:ring focus:border-accent-color"
                            placeholder="John Doe"
                        />
                    </div>

                    <div>
                        <label for="username" class="block text-sm font-medium text-text-color mb-1">
                            { &i18n_username }
                        </label>
                        <input
                            type="text"
                            id="username"
                            value={(*username).clone()}
                            onchange={let username = username.clone(); move |e: Event| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    username.set(input.value());
                                }
                            }}
                            class="search-bar-input w-full px-3 py-2 border rounded-md focus:outline-none focus:ring focus:border-accent-color"
                            placeholder="johndoe"
                        />
                    </div>

                    <div>
                        <label for="email" class="block text-sm font-medium text-text-color mb-1">
                            { &i18n_email }
                        </label>
                        <input
                            type="email"
                            id="email"
                            value={(*email).clone()}
                            onchange={let email = email.clone(); move |e: Event| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    email.set(input.value());
                                }
                            }}
                            class="search-bar-input w-full px-3 py-2 border rounded-md focus:outline-none focus:ring focus:border-accent-color"
                            placeholder="john@example.com"
                        />
                    </div>

                    <div>
                        <label for="password" class="block text-sm font-medium text-text-color mb-1">
                            { &i18n_password }
                        </label>
                        <input
                            type="password"
                            id="password"
                            value={(*password).clone()}
                            onchange={let password = password.clone(); move |e: Event| {
                                if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                                    password.set(input.value());
                                }
                            }}
                            class="search-bar-input w-full px-3 py-2 border rounded-md focus:outline-none focus:ring focus:border-accent-color"
                            placeholder="••••••••"
                        />
                    </div>

                    if let Some(message) = &*validation_message {
                        <div class="text-error-color text-sm mt-2">
                            {message}
                        </div>
                    }

                    <div class="flex justify-end space-x-3 mt-6">
                        <button
                            type="submit"
                            class="px-4 py-2 bg-button-color text-button-text-color rounded-md hover:bg-hover-color focus:outline-none focus:ring transition-colors"
                        >
                            { &i18n_create_admin_account }
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}

pub fn empty_message(header: &str, paragraph: &str) -> Html {
    html! {
        <div class="empty-episodes-container">
            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
            <h1 class="page-paragraphs">{ header }</h1>
            <p class="page-paragraphs">{ paragraph }</p>
        </div>
    }
}

pub fn on_shownotes_click(
    history: BrowserHistory,
    dispatch: Dispatch<AppState>,
    episode_id: i32,
    shownotes_episode_url: String,
    episode_audio_url: String,
    podcast_title: String,
    _db_added: bool,
    person_episode: bool,
    is_youtube: bool,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        web_sys::console::log_1(
            &format!("Executing shownotes click. is_youtube: {:?}", is_youtube).into(),
        );

        let show_notes = shownotes_episode_url.clone();
        let ep_aud = episode_audio_url.clone();
        let pod_title = podcast_title.clone();

        let dispatch_clone = dispatch.clone();
        let history_clone = history.clone();

        wasm_bindgen_futures::spawn_local(async move {
            if episode_id > 0 {
                if is_youtube {
                    history_clone.push(format!("/episode?episode_id={}&youtube=true", episode_id));
                } else {
                    history_clone.push(format!("/episode?episode_id={}", episode_id));
                }
            } else {
                let mut new_url = "/episode".to_string();
                new_url.push_str("?podcast_title=");
                new_url.push_str(&urlencoding::encode(&pod_title));
                new_url.push_str("&episode_url=");
                new_url.push_str(&urlencoding::encode(&show_notes));
                new_url.push_str("&audio_url=");
                new_url.push_str(&urlencoding::encode(&ep_aud));
                new_url.push_str("&is_youtube=");
                new_url.push_str(&is_youtube.to_string());

                history_clone.push(new_url);

                Dispatch::<EpisodeNavigationState>::global().reduce_mut(move |s| {
                    s.selected_episode_id = Some(episode_id);
                    s.selected_episode_url = Some(show_notes);
                    s.selected_episode_audio_url = Some(ep_aud);
                    s.selected_podcast_title = Some(pod_title);
                    s.selected_is_youtube = is_youtube;
                });
                Dispatch::<EpisodeDetailState>::global().reduce_mut(move |s| {
                    s.person_episode = Some(person_episode);
                    s.fetched_episode = None;
                });
            }
        });
    })
}

// First the modal component
#[derive(Properties, PartialEq)]
pub struct EpisodeModalProps {
    pub episode_id: i32, // Instead of Box<dyn EpisodeTrait>
    pub episode_url: String,
    pub episode_artwork: String,
    pub episode_title: String,
    pub description: String,
    pub format_release: String,
    pub duration: i32,
    pub on_close: Callback<MouseEvent>,
    pub on_show_notes: Callback<MouseEvent>,
    pub listen_duration_percentage: i32,
    pub is_youtube: bool,
    pub is_video: bool,
}

#[function_component(EpisodeModal)]
pub fn episode_modal(props: &EpisodeModalProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_go_to_episode_page = i18n.t("gen_components.go_to_episode_page").to_string();
    let onclick_outside = {
        let on_close = props.on_close.clone();
        Callback::from(move |e: MouseEvent| {
            if let Some(target) = e.target_dyn_into::<Element>() {
                if target.class_list().contains("modal-overlay") {
                    on_close.emit(e);
                }
            }
        })
    };
    let formatted_duration = format_time(props.duration.into());

    html! {
        <div class="modal-overlay fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50 p-4"
             onclick={onclick_outside}>
            <div class="bg-custom-light dark:bg-custom-dark w-11/12 max-w-2xl rounded-lg shadow-xl max-h-[90vh] flex flex-col">
                // Header with artwork and title - fixed at top
                <div class="flex items-start space-x-4 p-6 border-b border-custom-border">
                    <FallbackImage
                        src={props.episode_artwork.clone()}
                        alt="Episode artwork"
                        class="w-32 h-32 rounded-lg object-cover flex-shrink-0"
                    />
                    <div class="flex-1 min-w-0"> // min-w-0 helps with text truncation
                        <h2 class="text-xl font-bold mb-2 item_container-text truncate">
                            {props.episode_title.clone()}
                        </h2>
                        <p class="item_container-text text-sm">
                            {props.format_release.clone()}
                        </p>
                    </div>
                    <button onclick={props.on_close.clone()} class="hover:opacity-75 flex-shrink-0 item_container-text">
                        <i class="ph ph-arrow-u-left-down text-2xl"></i>
                    </button>
                </div>

                // Description - scrollable section
                <div class="flex-1 p-6 overflow-y-auto">
                    <div class="prose dark:prose-invert item_container-text max-w-none">
                        <div class="links-custom episode-description-container">
                            <SafeHtml
                                html={props.description.clone()}
                                episode_url={Some(props.episode_url.clone())}
                                episode_title={Some(props.episode_title.clone())}
                                episode_description={Some(props.description.clone())}
                                episode_release_date={Some(props.format_release.clone())}
                                episode_artwork={Some(props.episode_artwork.clone())}
                                episode_duration={Some(props.duration)}
                                episode_id={Some(props.episode_id)}
                                is_youtube={props.is_youtube}
                                is_video={Some(props.is_video)}
                            />
                        </div>
                    </div>
                </div>

                // Footer - fixed at bottom
                <div class="flex justify-between items-center p-6 border-t border-custom-border mt-auto">
                    <div class="flex items-center space-x-2">
                        <span class="item_container-text">{formatted_duration.clone()}</span>
                        <div class="progress-bar-container">
                            <div class="progress-bar"
                                 style={format!("width: {}%;", props.listen_duration_percentage)} />
                        </div>
                    </div>
                    <button onclick={props.on_show_notes.clone()}
                            class="bg-custom-primary hover:opacity-75 text-white px-4 py-2 rounded-lg">
                        { &i18n_go_to_episode_page }
                    </button>
                </div>
            </div>
        </div>
    }
}

#[hook]
pub fn use_long_press(
    on_long_press: Callback<TouchEvent>,
    delay_ms: Option<u32>,
) -> (
    Callback<TouchEvent>,
    Callback<TouchEvent>,
    Callback<TouchEvent>,
    bool,
    bool,
) {
    let timeout_handle = use_state(|| None::<Timeout>);
    let is_long_press = use_state(|| false);
    let start_position = use_state(|| None::<(i32, i32)>);
    let is_pressing = use_state(|| false);

    // Configure the threshold for movement that cancels a long press
    let movement_threshold = 10; // pixels
    let delay = delay_ms.unwrap_or(600); // Increased to 600ms for better iOS compatibility

    let on_touch_start = {
        let timeout_handle = timeout_handle.clone();
        let is_long_press = is_long_press.clone();
        let start_position = start_position.clone();
        let is_pressing = is_pressing.clone();
        let on_long_press = on_long_press.clone();

        Callback::from(move |event: TouchEvent| {
            // Don't prevent default on touch start - let iOS handle it naturally

            // Disable text selection for iOS
            if let Some(target) = event.target() {
                if let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() {
                    let _ = element.style().set_property("user-select", "none");
                    let _ = element.style().set_property("-webkit-user-select", "none");
                    let _ = element
                        .style()
                        .set_property("-webkit-touch-callout", "none");
                }
            }

            // Store the initial touch position
            if let Some(touch) = event.touches().get(0) {
                start_position.set(Some((touch.client_x(), touch.client_y())));
            }

            // Set pressing state for visual feedback
            is_pressing.set(true);

            // Reset long press state
            is_long_press.set(false);

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
        let is_pressing = is_pressing.clone();

        Callback::from(move |event: TouchEvent| {
            // Clear the timeout if the touch ends before the long press is triggered
            timeout_handle.set(None);

            // Clear pressing state
            is_pressing.set(false);

            // Re-enable text selection
            if let Some(target) = event.target() {
                if let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() {
                    let _ = element.style().remove_property("user-select");
                    let _ = element.style().remove_property("-webkit-user-select");
                    let _ = element.style().remove_property("-webkit-touch-callout");
                }
            }
        })
    };

    let on_touch_move = {
        let timeout_handle = timeout_handle.clone();
        let start_position = start_position.clone();
        let is_pressing = is_pressing.clone();

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
                        is_pressing.set(false);

                        // Re-enable text selection
                        if let Some(target) = event.target() {
                            if let Ok(element) = target.dyn_into::<web_sys::HtmlElement>() {
                                let _ = element.style().remove_property("user-select");
                                let _ = element.style().remove_property("-webkit-user-select");
                                let _ = element.style().remove_property("-webkit-touch-callout");
                            }
                        }
                    }
                }
            }
        })
    };

    (
        on_touch_start,
        on_touch_end,
        on_touch_move,
        *is_long_press,
        *is_pressing,
    )
}

#[derive(Properties, PartialEq)]
pub struct LoadingModalProps {
    pub name: String,
    pub is_visible: bool,
}

#[function_component(LoadingModal)]
pub fn loading_modal(props: &LoadingModalProps) -> Html {
    let (i18n, _) = use_translation();
    let i18n_this_may_take_a_moment = i18n.t("gen_components.this_may_take_a_moment").to_string();
    if !props.is_visible {
        return html! {};
    }

    html! {
        <div class="modal-overlay flex items-center justify-center">
            <div class="modal-content text-center">
                <div class="spinner mx-auto mb-4"></div>
                <p class="modal-title">{ format!("Searching everywhere for {}...", props.name) }</p>
                <p class="modal-subtitle mt-2">{ &i18n_this_may_take_a_moment }</p>
            </div>
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct RefreshProgressProps {
    pub current_podcast: Option<String>,
    pub progress: i32,
    pub total: i32,
}

#[function_component(RefreshProgress)]
pub fn refresh_progress(props: &RefreshProgressProps) -> Html {
    if props.current_podcast.is_none() {
        return html! {};
    }

    let percentage = if props.total > 0 {
        (props.progress as f32 / props.total as f32 * 100.0).round() as i32
    } else {
        0
    };

    let percentage_style = format!("width: {}%", percentage);

    html! {
        <div class="fixed bottom-24 left-1/2 transform -translate-x-1/2 z-50 w-11/12 max-w-md">
            <div class="item-container p-4 shadow-lg">
                <div class="space-y-2">
                    <div class="flex justify-between text-sm">
                        <span class="item_container-text">
                            {"Refreshing: "}{props.current_podcast.clone().unwrap_or_default()}
                        </span>
                        <span class="item_container-text">
                            {props.progress}{" / "}{props.total}
                        </span>
                    </div>
                    <div class="w-full bg-gray-200 rounded-full h-2.5">
                        <div
                            class="bg-blue-600 h-2.5 rounded-full transition-all duration-300"
                            style={percentage_style}
                        />
                    </div>
                </div>
            </div>
        </div>
    }
}

// Reusable themed image picker with live preview. Hides the native file input behind a
// styled label button, shows the selected image, and lists supported file types.
#[derive(Properties, PartialEq, Clone)]
pub struct ImagePickerProps {
    /// Unique DOM id so the styled <label> can target the hidden <input>.
    pub id: String,
    /// Fired with the selected File, or None when cleared.
    pub on_change: Callback<Option<web_sys::File>>,
    #[prop_or("image/*".to_string())]
    pub accept: String,
    /// Optional caption listing supported file types.
    #[prop_or_default]
    pub supported_types: Option<String>,
    /// Optional override for the button label.
    #[prop_or_default]
    pub button_label: Option<String>,
    /// Optional fully-qualified image URL shown as the default preview when the user
    /// hasn't picked their own file (e.g. cover art auto-detected from a folder).
    #[prop_or_default]
    pub default_preview: Option<String>,
    /// Optional caption shown beneath the default preview (e.g. "Detected from folder").
    #[prop_or_default]
    pub default_preview_label: Option<String>,
}

#[function_component(ImagePicker)]
pub fn image_picker(props: &ImagePickerProps) -> Html {
    let preview_url = use_state(|| None as Option<String>);
    let file_name = use_state(|| None as Option<String>);
    // Holds the current object URL so we can revoke it on change/unmount.
    let url_ref = use_mut_ref(|| None as Option<String>);

    // Revoke any outstanding object URL when the component is destroyed.
    {
        let url_ref = url_ref.clone();
        use_effect_with((), move |_| {
            move || {
                if let Some(url) = url_ref.borrow_mut().take() {
                    let _ = web_sys::Url::revoke_object_url(&url);
                }
            }
        });
    }

    let on_input_change = {
        let preview_url = preview_url.clone();
        let file_name = file_name.clone();
        let url_ref = url_ref.clone();
        let on_change = props.on_change.clone();
        Callback::from(move |e: Event| {
            let file = e
                .target_dyn_into::<HtmlInputElement>()
                .and_then(|input| input.files())
                .and_then(|files| files.get(0));

            // Revoke the previous preview URL before replacing it.
            if let Some(old) = url_ref.borrow_mut().take() {
                let _ = web_sys::Url::revoke_object_url(&old);
            }

            match file {
                Some(f) => {
                    let url = web_sys::Url::create_object_url_with_blob(&f).ok();
                    if let Some(ref u) = url {
                        *url_ref.borrow_mut() = Some(u.clone());
                    }
                    file_name.set(Some(f.name()));
                    preview_url.set(url);
                    on_change.emit(Some(f));
                }
                None => {
                    file_name.set(None);
                    preview_url.set(None);
                    on_change.emit(None);
                }
            }
        })
    };

    let on_remove = {
        let preview_url = preview_url.clone();
        let file_name = file_name.clone();
        let url_ref = url_ref.clone();
        let on_change = props.on_change.clone();
        let input_id = props.id.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(old) = url_ref.borrow_mut().take() {
                let _ = web_sys::Url::revoke_object_url(&old);
            }
            // Clear the underlying input so re-selecting the same file still fires onchange.
            if let Some(input) = window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id(&input_id))
                .and_then(|el| el.dyn_into::<HtmlInputElement>().ok())
            {
                input.set_value("");
            }
            file_name.set(None);
            preview_url.set(None);
            on_change.emit(None);
        })
    };

    let button_label = props
        .button_label
        .clone()
        .unwrap_or_else(|| "Choose Image".to_string());

    let has_local_file = file_name.is_some();
    // Show the user's pick if they made one; otherwise fall back to the provided default.
    let shown_preview = (*preview_url)
        .clone()
        .or_else(|| if has_local_file { None } else { props.default_preview.clone() });
    let showing_default = !has_local_file && props.default_preview.is_some();

    html! {
        <div class="image-picker">
            <div style="display:flex; align-items:center; gap:12px; flex-wrap:wrap;">
                <label class="btn btn-secondary" for={props.id.clone()} style="padding:6px 12px; cursor:pointer;">
                    <i class="ph ph-image"></i>
                    <span>{ button_label }</span>
                </label>
                <input
                    id={props.id.clone()}
                    type="file"
                    accept={props.accept.clone()}
                    style="display:none;"
                    onchange={on_input_change}
                />
                if let Some(name) = (*file_name).clone() {
                    <span style="font-size:13px; color: var(--text-secondary-color); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:220px;">{ name }</span>
                    <button type="button" class="btn btn-ghost" onclick={on_remove} style="padding:4px 8px;">
                        <i class="ph ph-x"></i>
                    </button>
                }
            </div>
            if let Some(url) = shown_preview {
                <div style="margin-top:10px;">
                    <img src={url} alt="preview" style="max-width:160px; max-height:160px; border-radius:8px; object-fit:cover; border:1px solid rgba(128,128,128,0.2);" />
                    if showing_default {
                        if let Some(label) = props.default_preview_label.clone() {
                            <div class="settings-row-desc" style="margin-top:4px;">{ label }</div>
                        }
                    }
                </div>
            }
            if let Some(types) = props.supported_types.clone() {
                <div class="settings-row-desc" style="margin-top:6px;">{ types }</div>
            }
        </div>
    }
}
