use crate::components::context::{AppState, EpisodeStatusState, NotificationState, UIState};
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
use web_sys::{window, Element, HtmlInputElement, MouseEvent};
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

    // Just use the original src without timestamps
    let image_src = use_state(|| props.src.clone());

    // Update src when props.src changes
    {
        let image_src = image_src.clone();
        let props_src = props.src.clone();
        use_effect_with(props_src, move |src| {
            image_src.set(src.clone());
            || ()
        });
    }

    // Create a proxied URL from the original source
    let proxied_url = {
        let original_url = props.src.clone();
        format!(
            "{}/api/proxy/image?url={}",
            server_name,
            urlencoding::encode(&original_url)
        )
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
    let search_index = use_state(|| "podcast_index".to_string()); // Default to "podcast_index"
    let is_submitting = use_state(|| false);

    let history_clone = history.clone();
    let podcast_value_clone = podcast_value.clone();
    let search_index_clone = search_index.clone();
    // State for toggling the dropdown in mobile view
    let mobile_dropdown_open = use_state(|| false);
    // State for animation - separate from actual visibility to allow animation to complete
    let mobile_dropdown_animating = use_state(|| false);

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
                Dispatch::<AppState>::global().reduce_mut(|state| state.is_loading = Some(true));
                if *search_index == "youtube" {
                    match call_youtube_search(&search_value, &api_url.unwrap()).await {
                        Ok(yt_results) => {
                            let search_results = YouTubeSearchResults {
                                channels: yt_results.results,
                                videos: Vec::new(),
                            };

                            Dispatch::<AppState>::global().reduce_mut(|state| {
                                state.youtube_search_results = Some(search_results);
                                state.is_loading = Some(false);
                            });

                            history.push("/youtube_layout");
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            Dispatch::<AppState>::global().reduce_mut(|state| {
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
                            Dispatch::<AppState>::global().reduce_mut(move |state| {
                                state.search_results = Some(search_results);
                                state.podcast_added = Some(false);
                            });
                            Dispatch::<AppState>::global().reduce_mut(|state| state.is_loading = Some(false));
                            history.push("/pod_layout");
                        }
                        Err(_) => {
                            Dispatch::<AppState>::global().reduce_mut(|state| state.is_loading = Some(false));
                        }
                    }
                }
                // Reset submission state after completion
                is_submitting_clone.set(false);
            });
        }
    };

    let on_submit = {
        let handle_submit = handle_submit.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
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

    let on_submit_click = {
        let handle_submit = handle_submit.clone();
        Callback::from(move |_: MouseEvent| {
            handle_submit();
        })
    };

    let on_search_click = {
        let handle_submit = handle_submit.clone();
        let mobile_dropdown_open = mobile_dropdown_open.clone();
        let mobile_dropdown_animating = mobile_dropdown_animating.clone();
        Callback::from(move |_: MouseEvent| {
            if web_sys::window()
                .unwrap()
                .inner_width()
                .unwrap()
                .as_f64()
                .unwrap()
                < 768.0
            {
                if !*mobile_dropdown_open {
                    // Opening the dropdown
                    mobile_dropdown_open.set(true);
                    mobile_dropdown_animating.set(true);
                } else {
                    // Closing the dropdown - start animation first
                    mobile_dropdown_animating.set(false);

                    // Set a timeout to actually close the dropdown after animation
                    let mobile_dropdown_open_clone = mobile_dropdown_open.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        // Wait for animation to complete (300ms)
                        let promise = js_sys::Promise::new(&mut |resolve, _| {
                            web_sys::window()
                                .unwrap()
                                .set_timeout_with_callback_and_timeout_and_arguments_0(
                                    &resolve, 300,
                                )
                                .unwrap();
                        });
                        let _ = wasm_bindgen_futures::JsFuture::from(promise).await;

                        // Now actually close the dropdown
                        mobile_dropdown_open_clone.set(false);
                    });
                }
            } else {
                handle_submit();
            }
        })
    };

    let dropdown_open = use_state(|| false);

    let toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |_: MouseEvent| {
            dropdown_open.set(!*dropdown_open);
        })
    };

    let on_dropdown_select = {
        let dropdown_open = dropdown_open.clone();
        let search_index = search_index.clone();
        move |category: &str| {
            search_index.set(category.to_string());
            dropdown_open.set(false);
        }
    };

    let on_dropdown_select_itunes = {
        let on_dropdown_select = on_dropdown_select.clone();
        Callback::from(move |_: MouseEvent| on_dropdown_select("itunes"))
    };

    let on_dropdown_select_podcast_index = {
        let on_dropdown_select = on_dropdown_select.clone();
        Callback::from(move |_: MouseEvent| on_dropdown_select("podcast_index"))
    };

    let on_dropdown_select_youtube = {
        let on_dropdown_select = on_dropdown_select.clone();
        Callback::from(move |_: MouseEvent| on_dropdown_select("youtube"))
    };

    let search_index_display = match search_index.as_str() {
        "podcast_index" => "Podcast Index",
        "itunes" => "iTunes",
        "youtube" => "youtube",
        _ => "Unknown",
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

    let src_dropdown_open = use_state(|| false);

    let toggle_src_dropdown = {
        let src_dropdown_open = src_dropdown_open.clone();
        Callback::from(move |_: MouseEvent| {
            src_dropdown_open.set(!*src_dropdown_open);
        })
    };

    let select_src = {
        let search_index = search_index.clone();
        let src_dropdown_open = src_dropdown_open.clone();
        move |val: &'static str| {
            search_index.set(val.to_string());
            src_dropdown_open.set(false);
        }
    };

    let select_podcast_index = { let s = select_src.clone(); Callback::from(move |_: MouseEvent| s("podcast_index")) };
    let select_itunes = { let s = select_src.clone(); Callback::from(move |_: MouseEvent| s("itunes")) };
    let select_youtube = { let s = select_src.clone(); Callback::from(move |_: MouseEvent| s("youtube")) };

    html! {
        <div class="episodes-container w-full">
            <div class="topbar">
                <div class="topbar-left">
                    // Left spacer — drawer-icon buttons float above this area at z-49
                </div>
                <form class="topbar-right" onsubmit={on_submit.clone()}>
                    <button
                        type="button"
                        class="iconbtn"
                        onclick={toggle_queue}
                        title="Queue"
                    >
                        <i class="ph ph-queue"></i>
                        if queue_count > 0 {
                            <span class="topbar-queue-badge">{ queue_count }</span>
                        }
                    </button>
                    <div style="position: relative;">
                        <button
                            type="button"
                            class="src-source"
                            onclick={toggle_src_dropdown}
                        >
                            <span>{ search_index_display }</span>
                            <i class="ph ph-caret-down"></i>
                        </button>
                        if *src_dropdown_open {
                            <div class="src-dropdown">
                                <button type="button" class={classes!("src-dropdown-item", (*search_index == "podcast_index").then_some("is-active"))} onclick={select_podcast_index}>
                                    { &i18n_podcast_index }
                                </button>
                                <button type="button" class={classes!("src-dropdown-item", (*search_index == "itunes").then_some("is-active"))} onclick={select_itunes}>
                                    {"iTunes"}
                                </button>
                                <button type="button" class={classes!("src-dropdown-item", (*search_index == "youtube").then_some("is-active"))} onclick={select_youtube}>
                                    {"YouTube"}
                                </button>
                            </div>
                        }
                    </div>
                    <div class="src-input">
                        <input
                            type="search"
                            placeholder="Search\u{2026}"
                            oninput={on_input_change.clone()}
                        />
                    </div>
                    <button
                        type="submit"
                        class="iconbtn"
                        onclick={on_search_click.clone()}
                        title="Search"
                    >
                        <i class="ph ph-magnifying-glass"></i>
                    </button>
                    <NotificationCenter />
                </form>
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
            if episode_id != 0 {
                history_clone.push(format!("/episode?episode_id={}", episode_id));
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

                dispatch_clone.reduce_mut(move |state| {
                    state.selected_episode_id = Some(episode_id);
                    state.selected_episode_url = Some(show_notes);
                    state.selected_episode_audio_url = Some(ep_aud);
                    state.selected_podcast_title = Some(pod_title);
                    state.person_episode = Some(person_episode);
                    state.selected_is_youtube = is_youtube;
                    state.fetched_episode = None;
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
