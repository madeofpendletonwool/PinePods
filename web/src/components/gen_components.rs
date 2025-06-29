use crate::components::context::{AppState, UIState};
#[cfg(not(feature = "server_build"))]
use crate::components::downloads_tauri::{
    download_file, remove_episode_from_local_db, update_local_database, update_podcast_database,
};
use crate::components::gen_funcs::format_error_message;
use crate::components::gen_funcs::{format_time, strip_images_from_html};
use crate::components::notification_center::{NotificationCenter, ToastNotification};
use crate::components::safehtml::SafeHtml;
use crate::requests::people_req::PersonEpisode;
use crate::requests::pod_req::{
    call_download_episode, call_mark_episode_completed, call_mark_episode_uncompleted,
    call_queue_episode, call_remove_downloaded_episode, call_remove_queued_episode,
    call_remove_saved_episode, call_save_episode, DownloadEpisodeRequest, Episode, EpisodeDownload,
    HistoryEpisode, HomeEpisode, MarkEpisodeCompletedRequest, QueuePodcastRequest, QueuedEpisode,
    SavePodcastRequest, SavedEpisode,
};
#[cfg(not(feature = "server_build"))]
use crate::requests::pod_req::{
    call_get_episode_metadata, call_get_podcast_details, EpisodeRequest,
};
use crate::requests::search_pods::Episode as SearchNewEpisode;
use crate::requests::search_pods::SearchEpisode;
use crate::requests::search_pods::{
    call_get_podcast_info, call_youtube_search, test_connection, PeopleEpisode,
    YouTubeSearchResults,
};
use gloo_events::EventListener;
use gloo_timers::callback::Timeout;
use std::any::Any;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
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
    let (state, _dispatch) = use_store::<AppState>();

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
        let server_name = state.auth_details.as_ref().unwrap().server_name.clone();
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
    let (_state, _dispatch) = use_store::<AppState>();

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
    let history = BrowserHistory::new();
    let (state, _dispatch) = use_store::<AppState>();
    let podcast_value = use_state(|| "".to_string());
    let search_index = use_state(|| "podcast_index".to_string()); // Default to "podcast_index"
    let (_app_state, dispatch) = use_store::<AppState>();
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
        let state = state.clone();
        let history = history_clone.clone();
        let podcast_value = podcast_value_clone.clone();
        let search_index = search_index_clone.clone();
        let dispatch = dispatch.clone();

        move || {
            if *is_submitting {
                return;
            }
            is_submitting.set(true);

            let submit_state = state.clone();
            let api_url = state.server_details.as_ref().map(|ud| ud.api_url.clone());
            let history = history.clone();
            let search_value = podcast_value.clone();
            let search_index = search_index.clone();
            let dispatch = dispatch.clone();
            let is_submitting_clone = is_submitting.clone();

            wasm_bindgen_futures::spawn_local(async move {
                dispatch.reduce_mut(|state| state.is_loading = Some(true));

                match test_connection(&api_url.clone().unwrap()).await {
                    Ok(_) => {
                        if *search_index == "youtube" {
                            let server_name = submit_state
                                .auth_details
                                .as_ref()
                                .map(|ud| ud.server_name.clone())
                                .unwrap();
                            let api_key = submit_state
                                .auth_details
                                .as_ref()
                                .map(|ud| ud.api_key.clone())
                                .unwrap()
                                .unwrap();
                            let user_id = submit_state
                                .user_details
                                .as_ref()
                                .map(|ud| ud.UserID.clone())
                                .unwrap();

                            match call_youtube_search(
                                &server_name,
                                &api_key,
                                user_id,
                                &search_value,
                                "channel",
                                10,
                            )
                            .await
                            {
                                Ok(yt_results) => {
                                    let search_results = YouTubeSearchResults {
                                        channels: yt_results.results,
                                        videos: Vec::new(),
                                    };

                                    dispatch.reduce_mut(|state| {
                                        state.youtube_search_results = Some(search_results);
                                        state.is_loading = Some(false);
                                    });

                                    history.push("/youtube_layout");
                                }
                                Err(e) => {
                                    let formatted_error = format_error_message(&e.to_string());
                                    dispatch.reduce_mut(|state| {
                                        state.error_message = Some(format!(
                                            "YouTube search error: {}",
                                            formatted_error
                                        ));
                                        state.is_loading = Some(false);
                                    });
                                }
                            }
                        } else {
                            match call_get_podcast_info(
                                &search_value,
                                &api_url.unwrap(),
                                &search_index,
                            )
                            .await
                            {
                                Ok(search_results) => {
                                    dispatch.reduce_mut(move |state| {
                                        state.search_results = Some(search_results);
                                        state.podcast_added = Some(false);
                                    });
                                    dispatch.reduce_mut(|state| state.is_loading = Some(false));
                                    history.push("/pod_layout");
                                }
                                Err(_) => {
                                    dispatch.reduce_mut(|state| state.is_loading = Some(false));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Error testing connection: {}", e).into());
                        dispatch.reduce_mut(|state| state.is_loading = Some(false));
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
        Callback::from(move |_| on_dropdown_select("itunes"))
    };

    let on_dropdown_select_podcast_index = {
        let on_dropdown_select = on_dropdown_select.clone();
        Callback::from(move |_| on_dropdown_select("podcast_index"))
    };

    let on_dropdown_select_youtube = {
        let on_dropdown_select = on_dropdown_select.clone();
        Callback::from(move |_| on_dropdown_select("youtube"))
    };

    let search_index_display = match search_index.as_str() {
        "podcast_index" => "Podcast Index",
        "itunes" => "iTunes",
        "youtube" => "youtube",
        _ => "Unknown",
    };

    html! {
        <div class="episodes-container w-full search-background">
            <form class="search-bar-container flex justify-end w-full mx-auto border-solid border-b-2 border-color" onsubmit={on_submit}>
                <div class="relative inline-flex">
                    <button
                        id="dropdown-button"
                        onclick={toggle_dropdown}
                        class="dropdown-button hidden md:flex md:block flex-shrink-0 z-10 inline-flex items-center py-2.5 px-4 text-sm font-medium text-center border border-r-0 border-gray-300 dark:border-gray-700 rounded-l-lg focus:ring-4 focus:outline-none"
                        type="button"
                    >
                        {format!("{} ", search_index_display)}
                        <svg class="w-2.5 h-2.5 ms-2.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6">
                            <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 4 4 4-4"/>
                        </svg>
                    </button>
                    {
                        if *dropdown_open {
                            html! {
                                <div class="search-dropdown-content-class absolute z-10 divide-y rounded-lg shadow">
                                    <ul class="dropdown-container py-2 text-sm">
                                        <li class="dropdown-option" onclick={on_dropdown_select_itunes.clone()}>{ "iTunes" }</li>
                                        <li class="dropdown-option" onclick={on_dropdown_select_podcast_index.clone()}>{ "Podcast Index" }</li>
                                        <li class="dropdown-option" onclick={on_dropdown_select_youtube.clone()}>{ "YouTube" }</li>
                                    </ul>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }

                    <input
                        type="search"
                        id="search-dropdown"
                        class="search-input search-input-custom-border block p-2.5 w-full z-20 text-sm rounded-r-lg border hidden md:inline-flex"
                        placeholder="Search"
                        required=true
                        oninput={on_input_change.clone()}
                    />
                </div>
                <button
                    type="submit"
                    class="search-btn p-2.5 text-sm font-medium rounded-lg border focus:ring-4 focus:outline-none"
                    onclick={on_search_click.clone()}
                >
                    <svg class="w-4 h-4" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 20 20">
                        <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m19 19-4-4m0-7A7 7 0 1 1 1 8a7 7 0 0 1 14 0Z"/>
                    </svg>
                </button>
                {
                    if *mobile_dropdown_open {
                        html! {
                            <div class={if *mobile_dropdown_animating { "search-drop-solid dropdown-visible" } else { "search-drop-solid dropdown-hiding" }}>
                                <div class="flex justify-between mb-4" role="group">
                                    <button
                                        type="button"
                                        class={format!("p-2 rounded-lg search-drop-button flex items-center justify-center w-10 h-10 hover:bg-opacity-20 {}",
                                            if *search_index == "podcast_index" { "active" } else { "" })}
                                        onclick={on_dropdown_select_podcast_index}
                                    >
                                        <img
                                            src="/static/assets/logos/podcastindex.svg"
                                            alt="Podcast Index"
                                            class="w-6 h-6"
                                        />
                                    </button>

                                    <button
                                        type="button"
                                        class={format!("p-2 rounded-lg search-drop-button flex items-center justify-center w-10 h-10 hover:bg-opacity-20 {}",
                                            if *search_index == "itunes" { "active" } else { "" })}
                                        onclick={on_dropdown_select_itunes}
                                    >
                                        <img
                                            src="/static/assets/logos/itunes.png"
                                            alt="iTunes"
                                            class="w-6 h-6"
                                        />
                                    </button>

                                    <button
                                        type="button"
                                        class={format!("p-2 rounded-lg search-drop-button flex items-center justify-center w-10 h-10 hover:bg-opacity-20 {}",
                                            if *search_index == "youtube" { "active" } else { "" })}
                                        onclick={on_dropdown_select_youtube}
                                    >
                                        <img
                                            src="/static/assets/logos/youtube.png"
                                            alt="YouTube"
                                            class="w-6 h-6"
                                        />
                                    </button>
                                </div>

                                <input
                                    type="text"
                                    class="search-input search-input-dropdown-border block p-2 w-full text-sm rounded-lg mb-4"
                                    placeholder="Search"
                                    value={(*podcast_value).clone()}
                                    oninput={on_input_change.clone()}
                                    onkeydown={
                                        let handle_submit = handle_submit.clone();
                                        Callback::from(move |e: KeyboardEvent| {
                                            if e.key() == "Enter" {
                                                e.prevent_default();
                                                handle_submit();
                                            }
                                        })
                                    }
                                />

                                <button
                                    class="search-btn w-full font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline"
                                    onclick={on_submit_click.clone()}
                                >
                                    {"Search"}
                                </button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                // Add the NotificationCenter component here
                <div class="ml-2">
                    <NotificationCenter />
                </div>
            </form>
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
                <h2 class="text-2xl font-bold mb-6 text-text-color">{"Welcome to Pinepods!"}</h2>
                <p class="mb-6 text-text-color">{"Let's set up your administrator account to get started."}</p>

                <form onsubmit={onsubmit} class="space-y-4">
                    <div>
                        <label for="fullname" class="block text-sm font-medium text-text-color mb-1">
                            {"Full Name"}
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
                            {"Username"}
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
                            {"Email"}
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
                            {"Password"}
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
                            {"Create Admin Account"}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}

#[derive(Properties, Clone)]
pub struct ContextButtonProps {
    pub episode: Box<dyn EpisodeTrait>,
    pub page_type: String,
    #[prop_or(false)]
    pub show_menu_only: bool,
    #[prop_or(None)]
    pub position: Option<(i32, i32)>,
    #[prop_or(None)]
    pub on_close: Option<Callback<()>>,
}

#[function_component(ContextButton)]
pub fn context_button(props: &ContextButtonProps) -> Html {
    let dropdown_open = use_state(|| false);
    let (post_state, post_dispatch) = use_store::<AppState>();
    let (_ui_state, _ui_dispatch) = use_store::<UIState>();
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let dropdown_ref = use_node_ref();
    let button_ref = use_node_ref();

    // Update dropdown_open if show_menu_only prop changes
    {
        let dropdown_open = dropdown_open.clone();
        use_effect_with(props.show_menu_only, move |show_menu_only| {
            if *show_menu_only {
                dropdown_open.set(true);
            }
            || ()
        });
    }

    let toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            dropdown_open.set(!*dropdown_open);
        })
    };

    // Close dropdown when clicking outside
    {
        let dropdown_open = dropdown_open.clone();
        let dropdown_ref = dropdown_ref.clone();
        let button_ref = button_ref.clone();
        let on_close = props.on_close.clone();
        let show_menu_only = props.show_menu_only;

        use_effect_with((*dropdown_open, ()), move |_| {
            let document = window().unwrap().document().unwrap();
            let dropdown_open = dropdown_open.clone();
            let dropdown_ref = dropdown_ref.clone();
            let button_ref = button_ref.clone();
            let on_close = on_close.clone();
            let show_menu_only = show_menu_only;

            // Handle outside clicks/touches to dismiss menu
            let handle_outside_interaction = {
                let dropdown_open = dropdown_open.clone();
                let dropdown_ref = dropdown_ref.clone();
                let button_ref = button_ref.clone();
                let on_close = on_close.clone();
                
                move |event: &web_sys::Event| {
                    if *dropdown_open {
                        if let Ok(target) = event.target().unwrap().dyn_into::<HtmlElement>() {
                            if let Some(dropdown_element) = dropdown_ref.cast::<HtmlElement>() {
                                if let Some(button_element) = button_ref.cast::<HtmlElement>() {
                                    if !dropdown_element.contains(Some(&target))
                                        && !button_element.contains(Some(&target))
                                    {
                                        dropdown_open.set(false);
                                        // If this is a long press menu (show_menu_only is true),
                                        // call the on_close callback when clicked outside
                                        if show_menu_only {
                                            if let Some(on_close) = &on_close {
                                                on_close.emit(());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };

            // Add click listener for desktop
            let click_handler = handle_outside_interaction.clone();
            let click_listener = EventListener::new(&document, "click", move |event| {
                click_handler(event);
            });

            // Add touchend listener for mobile (more reliable than touchstart for outside clicks)
            let touch_handler = handle_outside_interaction.clone();
            let touch_listener = EventListener::new(&document, "touchend", move |event| {
                touch_handler(event);
            });

            move || {
                drop(click_listener);
                drop(touch_listener);
            }
        });
    }


    let check_episode_id = props.episode.get_episode_id(Some(0));

    let queue_api_key = api_key.clone();
    let queue_server_name = server_name.clone();
    let queue_post = post_dispatch.clone();
    // let server_name = server_name.clone();
    let on_add_to_queue = {
        let episode = props.episode.clone();
        Callback::from(move |_| {
            let server_name_copy = queue_server_name.clone();
            let api_key_copy = queue_api_key.clone();
            let queue_post = queue_post.clone();
            let episode_clone = episode.clone();
            let request = QueuePodcastRequest {
                episode_id: episode.get_episode_id(Some(0)),
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let _ = call_queue_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("Episode added to Queue!")));
                match call_queue_episode(&server_name.unwrap(), &api_key.flatten(), &request).await
                {
                    Ok(success_message) => {
                        queue_post.reduce_mut(|state| {
                            state.info_message = Option::from(format!("{}", success_message));
                            if let Some(ref mut queued_episodes) = state.queued_episode_ids {
                                queued_episodes.push(episode_clone.get_episode_id(Some(0)));
                            }
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        queue_post.reduce_mut(|state| {
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

    let remove_queue_api_key = api_key.clone();
    let remove_queue_server_name = server_name.clone();
    let dispatch_clone = post_dispatch.clone();
    // let server_name = server_name.clone();
    let on_remove_queued_episode = {
        let episode = props.episode.clone();
        let episode_id = props.episode.get_episode_id(Some(0));
        Callback::from(move |_| {
            let post_dispatch = dispatch_clone.clone();
            let server_name_copy = remove_queue_server_name.clone();
            let api_key_copy = remove_queue_api_key.clone();
            let request = QueuePodcastRequest {
                episode_id: episode.get_episode_id(Some(0)),
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                match call_remove_queued_episode(
                    &server_name.unwrap(),
                    &api_key.flatten(),
                    &request,
                )
                .await
                {
                    Ok(success_message) => {
                        let formatted_info = format_error_message(&success_message.to_string());

                        // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                        post_dispatch.reduce_mut(|state| {
                            // Here, you should remove the episode from the queued_episodes
                            if let Some(ref mut queued_episodes) = state.queued_episodes {
                                queued_episodes
                                    .episodes
                                    .retain(|ep| ep.get_episode_id(Some(0)) != episode_id);
                            }
                            if let Some(ref mut queued_episode_ids) = state.queued_episode_ids {
                                queued_episode_ids.retain(|&id| id != episode_id);
                            }
                            // Optionally, you can update the info_message with success message
                            state.info_message = Some(format!("{}", formatted_info).to_string());
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

    let is_queued = post_state
        .queued_episode_ids
        .as_ref()
        .unwrap_or(&vec![])
        .contains(&check_episode_id.clone());

    let on_toggle_queue = {
        let on_add_to_queue = on_add_to_queue.clone();
        let on_remove_queued_episode = on_remove_queued_episode.clone();
        Callback::from(move |_| {
            if is_queued {
                on_remove_queued_episode.emit(());
            } else {
                on_add_to_queue.emit(());
            }
        })
    };

    let saved_api_key = api_key.clone();
    let saved_server_name = server_name.clone();
    let save_post = post_dispatch.clone();
    let on_save_episode = {
        let episode = props.episode.clone();
        Callback::from(move |_| {
            let server_name_copy = saved_server_name.clone();
            let api_key_copy = saved_api_key.clone();
            let post_state = save_post.clone();
            let episode_clone = episode.clone();
            let request = SavePodcastRequest {
                episode_id: episode.get_episode_id(Some(0)), // changed from episode_title
                user_id: user_id.unwrap(),
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let return_mes = call_save_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode saved successfully")));
                match call_save_episode(&server_name.unwrap(), &api_key.flatten(), &request).await {
                    Ok(success_message) => {
                        let formatted_info = format_error_message(&success_message.to_string());
                        post_state.reduce_mut(|state| {
                            state.info_message = Option::from(format!("{}", formatted_info));
                            if let Some(ref mut saved_episodes) = state.saved_episode_ids {
                                saved_episodes.push(episode_clone.get_episode_id(Some(0)));
                            }
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        post_state.reduce_mut(|state| {
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

    let remove_saved_api_key = api_key.clone();
    let remove_saved_server_name = server_name.clone();
    let dispatch_clone = post_dispatch.clone();
    let on_remove_saved_episode = {
        let episode = props.episode.clone();
        let episode_id = props.episode.get_episode_id(Some(0));
        Callback::from(move |_| {
            let post_dispatch = dispatch_clone.clone();
            let server_name_copy = remove_saved_server_name.clone();
            let api_key_copy = remove_saved_api_key.clone();
            let request = SavePodcastRequest {
                episode_id: episode.get_episode_id(Some(0)),
                user_id: user_id.unwrap(),
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                match call_remove_saved_episode(&server_name.unwrap(), &api_key.flatten(), &request)
                    .await
                {
                    Ok(success_message) => {
                        let formatted_info = format_error_message(&success_message.to_string());

                        // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                        post_dispatch.reduce_mut(|state| {
                            // Here, you should remove the episode from the saved_episodes
                            if let Some(ref mut saved_episodes) = state.saved_episodes {
                                saved_episodes
                                    .episodes
                                    .retain(|ep| ep.get_episode_id(Some(0)) != episode_id);
                            }
                            if let Some(ref mut saved_episode_ids) = state.saved_episode_ids {
                                saved_episode_ids.retain(|&id| id != episode_id);
                            }
                            // Optionally, you can update the info_message with success message
                            state.info_message = Some(format!("{}", formatted_info).to_string());
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

    let is_saved = post_state
        .saved_episode_ids
        .as_ref()
        .unwrap_or(&vec![])
        .contains(&check_episode_id.clone());

    let on_toggle_save = {
        let on_save_episode = on_save_episode.clone();
        let on_remove_saved_episode = on_remove_saved_episode.clone();
        // let is_saved = post_state
        //     .saved_episode_ids
        //     .as_ref()
        //     .unwrap_or(&vec![])
        //     .contains(&props.episode.get_episode_id(Some(0)));
        Callback::from(move |_| {
            if is_saved {
                on_remove_saved_episode.emit(());
            } else {
                on_save_episode.emit(());
            }
        })
    };

    let download_api_key = api_key.clone();
    let download_server_name = server_name.clone();
    let download_post = post_dispatch.clone();
    let on_download_episode = {
        let episode = props.episode.clone();
        Callback::from(move |_| {
            let post_state = download_post.clone();
            let server_name_copy = download_server_name.clone();
            let api_key_copy = download_api_key.clone();
            let episode_clone = episode.clone();
            let request = DownloadEpisodeRequest {
                episode_id: episode.get_episode_id(Some(0)),
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                match call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request)
                    .await
                {
                    Ok(success_message) => {
                        post_state.reduce_mut(|state| {
                            state.info_message = Option::from(format!("{}", success_message));
                            if let Some(ref mut downloaded_episodes) = state.downloaded_episode_ids
                            {
                                downloaded_episodes.push(episode_clone.get_episode_id(Some(0)));
                            }
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        post_state.reduce_mut(|state| {
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

    let remove_download_api_key = api_key.clone();
    let remove_download_server_name = server_name.clone();
    let dispatch_clone = post_dispatch.clone();
    let on_remove_downloaded_episode = {
        let episode = props.episode.clone();
        let episode_id = props.episode.get_episode_id(Some(0));
        Callback::from(move |_| {
            let post_dispatch = dispatch_clone.clone();
            let server_name_copy = remove_download_server_name.clone();
            let api_key_copy = remove_download_api_key.clone();
            let request = DownloadEpisodeRequest {
                episode_id: episode.get_episode_id(Some(0)),
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                match call_remove_downloaded_episode(
                    &server_name.unwrap(),
                    &api_key.flatten(),
                    &request,
                )
                .await
                {
                    Ok(success_message) => {
                        let formatted_info = format_error_message(&success_message.to_string());

                        // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                        post_dispatch.reduce_mut(|state| {
                            // Here, you should remove the episode from the downloaded_episodes
                            if let Some(ref mut downloaded_episodes) = state.downloaded_episodes {
                                downloaded_episodes
                                    .episodes
                                    .retain(|ep| ep.get_episode_id(Some(0)) != episode_id);
                            }
                            if let Some(ref mut downloaded_episode_ids) =
                                state.downloaded_episode_ids
                            {
                                downloaded_episode_ids.retain(|&id| id != episode_id);
                            }
                            // Optionally, you can update the info_message with success message
                            state.info_message = Some(format!("{}", formatted_info).to_string());
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

    let is_downloaded = post_state
        .downloaded_episode_ids
        .as_ref()
        .unwrap_or(&vec![])
        .contains(&check_episode_id.clone());

    let on_toggle_download = {
        let on_download = on_download_episode.clone();
        let on_remove_download = on_remove_downloaded_episode.clone();
        // let is_queued = post_state
        //     .queued_episode_ids
        //     .as_ref()
        //     .unwrap_or(&vec![])
        //     .contains(&props.episode.get_episode_id(Some(0)));
        Callback::from(move |_| {
            if is_downloaded {
                on_remove_download.emit(());
            } else {
                on_download.emit(());
            }
        })
    };

    #[cfg(not(feature = "server_build"))]
    let on_local_episode_download = {
        let episode = props.episode.clone();
        let download_local_post = post_dispatch.clone();
        let server_name_copy = server_name.clone();
        let api_key_copy = api_key.clone();
        let user_id_copy = user_id.clone();

        Callback::from(move |_| {
            let post_state = download_local_post.clone();
            let episode_id = episode.get_episode_id(Some(0));
            let request = EpisodeRequest {
                episode_id,
                user_id: user_id_copy.unwrap(),
                person_episode: false,
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy.clone().unwrap();
            let ep_api_key = api_key_copy.clone().flatten();
            let api_key = api_key_copy.clone().flatten();

            let future = async move {
                match call_get_episode_metadata(&server_name, ep_api_key, &request).await {
                    Ok(episode_info) => {
                        let audio_url = episode_info.episodeurl.clone();
                        let artwork_url = episode_info.episodeartwork.clone();
                        let podcast_id = episode_info.podcastid.clone();
                        let filename = format!("episode_{}.mp3", episode_id);
                        let artwork_filename = format!("artwork_{}.jpg", episode_id);
                        post_state.reduce_mut(|state| {
                            state.info_message = Some(format!("Episode download queued!"))
                        });
                        // Download audio
                        match download_file(audio_url, filename.clone()).await {
                            Ok(_) => {}
                            Err(e) => {
                                post_state.reduce_mut(|state| {
                                    let formatted_error = format_error_message(&format!("{:?}", e));
                                    state.error_message = Some(format!(
                                        "Failed to download episode audio: {}",
                                        formatted_error.clone()
                                    ))
                                });
                                web_sys::console::log_1(&format!("audio fail: {:?}", e).into());
                            }
                        }

                        // Download artwork
                        if let Err(e) = download_file(artwork_url, artwork_filename.clone()).await {
                            post_state.reduce_mut(|state| {
                                let formatted_error = format_error_message(&format!("{:?}", e));
                                state.error_message = Some(format!(
                                    "Failed to download episode artwork: {}",
                                    formatted_error.clone()
                                ))
                            });
                            web_sys::console::log_1(&format!("art fail: {:?}", e).into());
                        }

                        // Update local JSON database
                        if let Err(e) = update_local_database(episode_info).await {
                            post_state.reduce_mut(|state| {
                                let formatted_error = format_error_message(&format!("{:?}", e));
                                state.error_message = Some(format!(
                                    "Failed to update local database: {}",
                                    formatted_error.clone()
                                ))
                            });
                            web_sys::console::log_1(
                                &format!("Unable to parse Podcasts: {:?}", e).into(),
                            );
                        }

                        // Fetch and update local podcast metadata
                        match call_get_podcast_details(
                            &server_name,
                            &api_key.unwrap(),
                            user_id_copy.unwrap(),
                            &podcast_id,
                        )
                        .await
                        {
                            Ok(podcast_details) => {
                                if let Err(e) = update_podcast_database(podcast_details).await {
                                    post_state.reduce_mut(|state| {
                                        let formatted_error =
                                            format_error_message(&format!("{:?}", e));
                                        state.error_message = Some(format!(
                                            "Failed to update podcast database: {}",
                                            formatted_error
                                        ))
                                    });
                                }
                            }
                            Err(e) => {
                                post_state.reduce_mut(|state| {
                                    let formatted_error = format_error_message(&e.to_string());
                                    state.error_message = Some(format!(
                                        "Failed to fetch podcast metadata: {:?}",
                                        formatted_error
                                    ))
                                });
                            }
                        }
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        post_state.reduce_mut(|state| {
                            state.error_message = Some(format!("s {:?}", formatted_error))
                        });
                    }
                }
            };

            wasm_bindgen_futures::spawn_local(future);
        })
    };

    #[cfg(not(feature = "server_build"))]
    let ui_dispatch = _ui_dispatch.clone();

    #[cfg(not(feature = "server_build"))]
    let on_remove_locally_downloaded_episode = {
        let episode = props.episode.clone();
        let download_ui_dispatch = ui_dispatch.clone();
        let download_local_post = post_dispatch.clone();

        Callback::from(move |_: MouseEvent| {
            let post_state = download_local_post.clone();
            let ui_state = download_ui_dispatch.clone();
            let episode_id = episode.get_episode_id(Some(0));

            let future = async move {
                let filename = format!("episode_{}.mp3", episode_id);

                // Download audio
                match remove_episode_from_local_db(episode_id).await {
                    Ok(_) => {
                        // Update info_message in post_state
                        post_state.reduce_mut(|state| {
                            state.info_message =
                                Some(format!("Episode {} downloaded locally!", filename));
                        });

                        // Update local_download_increment in ui_state
                        ui_state.reduce_mut(|state| {
                            if let Some(increment) = state.local_download_increment.as_mut() {
                                *increment += 1;
                            } else {
                                state.local_download_increment = Some(1);
                            }
                        });
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&format!("{:?}", e));
                        post_state.reduce_mut(|state| {
                            state.error_message = Some(format!(
                                "Failed to download episode audio: {}",
                                formatted_error
                            ))
                        });
                    }
                }
            };

            wasm_bindgen_futures::spawn_local(future);
        })
    };

    let uncomplete_api_key = api_key.clone();
    let uncomplete_server_name = server_name.clone();
    let uncomplete_dispatch_clone = post_dispatch.clone();
    let on_uncomplete_episode = {
        let episode = props.episode.clone();
        let episode_id = props.episode.get_episode_id(Some(0));
        Callback::from(move |_| {
            let post_dispatch = uncomplete_dispatch_clone.clone();
            let server_name_copy = uncomplete_server_name.clone();
            let api_key_copy = uncomplete_api_key.clone();
            let request = MarkEpisodeCompletedRequest {
                episode_id: episode.get_episode_id(Some(0)),
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                match call_mark_episode_uncompleted(
                    &server_name.unwrap(),
                    &api_key.flatten(),
                    &request,
                )
                .await
                {
                    Ok(success_message) => {
                        // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                        post_dispatch.reduce_mut(|state| {
                            if let Some(completed_episodes) = state.completed_episodes.as_mut() {
                                if let Some(pos) =
                                    completed_episodes.iter().position(|&id| id == episode_id)
                                {
                                    completed_episodes.remove(pos);
                                } else {
                                    completed_episodes.push(episode_id);
                                }
                            } else {
                                state.completed_episodes = Some(vec![episode_id]);
                            }
                            state.info_message = Some(format!("{}", success_message));
                        });
                    }
                    Err(e) => {
                        post_dispatch.reduce_mut(|state| {
                            state.error_message = Option::from(format!("{}", e))
                        });
                        // Handle error, e.g., display the error message
                    }
                }
            };
            wasm_bindgen_futures::spawn_local(future);
            // dropdown_open.set(false);
        })
    };

    let complete_api_key = api_key.clone();
    let complete_server_name = server_name.clone();
    let dispatch_clone = post_dispatch.clone();
    let on_complete_episode = {
        let episode = props.episode.clone();
        let episode_id = props.episode.get_episode_id(Some(0));
        Callback::from(move |_| {
            let post_dispatch = dispatch_clone.clone();
            let server_name_copy = complete_server_name.clone();
            let api_key_copy = complete_api_key.clone();
            let request = MarkEpisodeCompletedRequest {
                episode_id: episode.get_episode_id(Some(0)),
                user_id: user_id.unwrap(), // replace with the actual user ID
                is_youtube: episode.get_is_youtube(),
            };
            let server_name = server_name_copy; // replace with the actual server name
            let api_key = api_key_copy; // replace with the actual API key
            let future = async move {
                // let _ = call_download_episode(&server_name.unwrap(), &api_key.flatten(), &request).await;
                // post_state.reduce_mut(|state| state.info_message = Option::from(format!("Episode now downloading!")));
                match call_mark_episode_completed(
                    &server_name.unwrap(),
                    &api_key.flatten(),
                    &request,
                )
                .await
                {
                    Ok(success_message) => {
                        // queue_post.reduce_mut(|state| state.info_message = Option::from(format!("{}", success_message)));
                        post_dispatch.reduce_mut(|state| {
                            if let Some(completed_episodes) = state.completed_episodes.as_mut() {
                                if let Some(pos) =
                                    completed_episodes.iter().position(|&id| id == episode_id)
                                {
                                    completed_episodes.remove(pos);
                                } else {
                                    completed_episodes.push(episode_id);
                                }
                            } else {
                                state.completed_episodes = Some(vec![episode_id]);
                            }
                            state.info_message = Some(format!("{}", success_message));
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

    let is_completed = post_state
        .completed_episodes
        .as_ref()
        .unwrap_or(&vec![])
        .contains(&check_episode_id);

    let on_toggle_complete = {
        let on_complete_episode = on_complete_episode.clone();
        let on_uncomplete_episode = on_uncomplete_episode.clone();
        let is_completed = is_completed.clone();

        Callback::from(move |_| {
            if is_completed {
                on_uncomplete_episode.emit(());
            } else {
                on_complete_episode.emit(());
            }
        })
    };

    let close_dropdown = {
        let dropdown_open = dropdown_open.clone();
        let on_close = props.on_close.clone();
        let show_menu_only = props.show_menu_only;

        Callback::from(move |_| {
            dropdown_open.set(false);

            // If this is a long press menu, also call the on_close callback
            if show_menu_only {
                if let Some(on_close) = &on_close {
                    on_close.emit(());
                }
            }
        })
    };

    let wrap_action = |action: Callback<MouseEvent>| {
        let close = close_dropdown.clone();
        Callback::from(move |e: MouseEvent| {
            action.emit(e);
            close.emit(());
        })
    };

    #[cfg(feature = "server_build")]
    let download_button = html! {
        <li class="dropdown-option" onclick={wrap_action(on_toggle_download.clone())}>
            { if is_downloaded { "Remove Downloaded Episode" } else { "Download Episode" } }
        </li>
    };

    #[cfg(not(feature = "server_build"))]
    let download_button = html! {
        <>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_download.clone())}>
                { if is_downloaded { "Remove Downloaded Episode" } else { "Download Episode" } }
            </li>
            <li class="dropdown-option" onclick={wrap_action(on_local_episode_download.clone())}>{ "Local Download" }</li>
        </>
    };

    #[cfg(not(feature = "server_build"))]
    let local_download_options = html! {
        <>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                { if is_queued { "Remove from Queue" } else { "Queue Episode" } }
            </li>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                { if is_saved { "Remove from Saved Episodes" } else { "Save Episode" } }
            </li>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_download.clone())}>
                { if is_downloaded { "Remove Downloaded Episode" } else { "Download Episode" } }
            </li>
            <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>{ if is_completed { "Mark Episode Incomplete" } else { "Mark Episode Complete" } }</li>
        </>
    };

    #[cfg(feature = "server_build")]
    let local_download_options = html! {};

    let action_buttons = match props.page_type.as_str() {
        "saved" => html! {
            <>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                    { if is_queued { "Remove from Queue" } else { "Queue Episode" } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                    { "Remove from Saved Episodes" }
                </li>
                {
                    // Handle download_button as VNode
                    download_button.clone()
                }
                <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>
                    { if is_completed { "Mark Episode Incomplete" } else { "Mark Episode Complete" } }
                </li>
            </>
        },
        "queue" => html! {
            <>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                    { if is_saved { "Remove from Saved Episodes" } else { "Save Episode" } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                    { if is_queued { "Remove from Queue" } else { "Queue Episode" } }
                </li>
                {
                    download_button.clone()
                }
                <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>{ if is_completed { "Mark Episode Incomplete" } else { "Mark Episode Complete" } }</li>
            </>
        },
        "downloads" => html! {
            <>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                    { if is_queued { "Remove from Queue" } else { "Queue Episode" } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                    { if is_saved { "Remove from Saved Episodes" } else { "Save Episode" } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_download.clone())}>
                    { if is_downloaded { "Remove Downloaded Episode" } else { "Download Episode" } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>{ if is_completed { "Mark Episode Incomplete" } else { "Mark Episode Complete" } }</li>
            </>
        },
        "local_downloads" => html! {
            local_download_options
        },
        // Add more page types and their respective button sets as needed
        _ => html! {
            // Default set of buttons for other page types
            <>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_queue.clone())}>
                    { if is_queued { "Remove from Queue" } else { "Queue Episode" } }
                </li>
                <li class="dropdown-option" onclick={wrap_action(on_toggle_save.clone())}>
                    { if is_saved { "Remove from Saved Episodes" } else { "Save Episode" } }
                </li>
                {
                    download_button.clone()
                }
                <li class="dropdown-option" onclick={wrap_action(on_toggle_complete.clone())}>{ if is_completed { "Mark Episode Incomplete" } else { "Mark Episode Complete" } }</li>
            </>
        },
    };

    html! {
        <div class="context-button-wrapper">
            if !props.show_menu_only {
                <button
                    ref={button_ref.clone()}
                    onclick={toggle_dropdown.clone()}
                    class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                >
                    <i class="ph ph-dots-three-circle md:text-6xl text-4xl"></i>
                </button>
            }
            if *dropdown_open {
                <div
                    ref={dropdown_ref.clone()}
                    class="dropdown-content-class border border-solid absolute z-50 divide-y rounded-lg shadow w-48"
                    style={
                        if props.show_menu_only {
                            if let Some((x, y)) = props.position {
                                format!("position: fixed; top: {}px; left: {}px; z-index: 1000;", y, x)
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        }
                    }
                >
                    <ul class="dropdown-container py-2 text-sm text-gray-700">
                        { action_buttons }
                    </ul>
                </div>
            }
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

pub trait EpisodeTrait {
    fn get_episode_artwork(&self) -> String;
    fn get_episode_title(&self) -> String;
    fn get_episode_id(&self, fallback_id: Option<i32>) -> i32;
    fn get_is_youtube(&self) -> bool;
    fn clone_box(&self) -> Box<dyn EpisodeTrait>;
    // fn eq(&self, other: &dyn EpisodeTrait) -> bool;
    fn as_any(&self) -> &dyn Any;
}

impl PartialEq for ContextButtonProps {
    fn eq(&self, _other: &Self) -> bool {
        if let Some(other) = self.episode.as_any().downcast_ref::<Episode>() {
            if let Some(self_episode) = self.episode.as_any().downcast_ref::<Episode>() {
                return self_episode == other;
            }
        }

        if let Some(other) = self.episode.as_any().downcast_ref::<QueuedEpisode>() {
            if let Some(self_episode) = self.episode.as_any().downcast_ref::<QueuedEpisode>() {
                return self_episode == other;
            }
        }

        false
    }
}

impl Clone for Box<dyn EpisodeTrait> {
    fn clone(&self) -> Box<dyn EpisodeTrait> {
        self.clone_box()
    }
}

impl EpisodeTrait for Episode {
    fn get_episode_artwork(&self) -> String {
        self.episodeartwork.clone()
    }

    fn get_episode_title(&self) -> String {
        self.episodetitle.clone()
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube
    }

    fn get_episode_id(&self, _fallback_id: Option<i32>) -> i32 {
        self.episodeid.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
    // Implement other methods
}

impl EpisodeTrait for QueuedEpisode {
    fn get_episode_artwork(&self) -> String {
        self.episodeartwork.clone()
    }

    fn get_episode_title(&self) -> String {
        self.episodetitle.clone()
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube
    }

    fn get_episode_id(&self, _fallback_id: Option<i32>) -> i32 {
        self.episodeid.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EpisodeTrait for SavedEpisode {
    fn get_episode_artwork(&self) -> String {
        self.episodeartwork.clone()
    }

    fn get_episode_title(&self) -> String {
        self.episodetitle.clone()
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube
    }

    fn get_episode_id(&self, _fallback_id: Option<i32>) -> i32 {
        self.episodeid.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EpisodeTrait for HistoryEpisode {
    fn get_episode_artwork(&self) -> String {
        self.episodeartwork.clone()
    }

    fn get_episode_title(&self) -> String {
        self.episodetitle.clone()
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube
    }

    fn get_episode_id(&self, _fallback_id: Option<i32>) -> i32 {
        self.episodeid.clone()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EpisodeTrait for EpisodeDownload {
    fn get_episode_artwork(&self) -> String {
        self.episodeartwork.clone()
    }

    fn get_episode_title(&self) -> String {
        self.episodetitle.clone()
    }

    fn get_episode_id(&self, _fallback_id: Option<i32>) -> i32 {
        self.episodeid.clone()
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EpisodeTrait for SearchEpisode {
    fn get_episode_artwork(&self) -> String {
        self.episodeartwork.clone()
    }

    fn get_episode_title(&self) -> String {
        self.episodetitle.clone()
    }

    fn get_episode_id(&self, _fallback_id: Option<i32>) -> i32 {
        self.episodeid.clone()
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EpisodeTrait for SearchNewEpisode {
    fn get_episode_artwork(&self) -> String {
        self.artwork.clone().unwrap()
    }

    fn get_episode_title(&self) -> String {
        self.title.clone().unwrap()
    }

    fn get_episode_id(&self, fallback_id: Option<i32>) -> i32 {
        if let Some(id) = self.episode_id {
            id
        } else if let Some(fallback_id) = fallback_id {
            fallback_id
        } else {
            panic!("No episode ID available");
        }
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube.unwrap_or(false)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EpisodeTrait for PeopleEpisode {
    fn get_episode_artwork(&self) -> String {
        self.feedImage.clone().unwrap()
    }

    fn get_episode_title(&self) -> String {
        self.title.clone().unwrap()
    }

    fn get_episode_id(&self, fallback_id: Option<i32>) -> i32 {
        if let Some(id) = self.id {
            id
        } else if let Some(fallback_id) = fallback_id {
            fallback_id
        } else {
            panic!("No episode ID available");
        }
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EpisodeTrait for PersonEpisode {
    fn get_episode_artwork(&self) -> String {
        self.episodeartwork.clone().unwrap()
    }

    fn get_episode_title(&self) -> String {
        self.episodetitle.clone()
    }

    fn get_episode_id(&self, _fallback_id: Option<i32>) -> i32 {
        self.episodeid // Just return it directly since it's already an i32
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl EpisodeTrait for HomeEpisode {
    fn get_episode_artwork(&self) -> String {
        self.episodeartwork.clone()
    }

    fn get_episode_title(&self) -> String {
        self.episodetitle.clone()
    }

    fn get_episode_id(&self, _fallback_id: Option<i32>) -> i32 {
        self.episodeid
    }

    fn clone_box(&self) -> Box<dyn EpisodeTrait> {
        Box::new(self.clone())
    }

    fn get_is_youtube(&self) -> bool {
        self.is_youtube
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn on_shownotes_click(
    history: BrowserHistory,
    dispatch: Dispatch<AppState>,
    episode_id: Option<i32>,
    shownotes_episode_url: Option<String>,
    episode_audio_url: Option<String>,
    podcast_title: Option<String>,
    _db_added: bool,
    person_episode: Option<bool>,
    is_youtube: Option<bool>,
) -> Callback<MouseEvent> {
    Callback::from(move |_: MouseEvent| {
        web_sys::console::log_1(
            &format!("Executing shownotes click. is_youtube: {:?}", is_youtube).into(),
        );

        let show_notes = shownotes_episode_url.clone();
        let ep_aud = episode_audio_url.clone();
        let pod_tit = podcast_title.clone();

        let dispatch_clone = dispatch.clone();
        let history_clone = history.clone();

        wasm_bindgen_futures::spawn_local(async move {
            dispatch_clone.reduce_mut(move |state| {
                state.selected_episode_id = episode_id;
                state.selected_episode_url = show_notes;
                state.selected_episode_audio_url = ep_aud;
                state.selected_podcast_title = pod_tit;
                state.person_episode = person_episode;
                state.selected_is_youtube = is_youtube;
                state.fetched_episode = None;
            });

            history_clone.push("/episode");
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
    pub listen_duration_percentage: f64,
    pub is_youtube: bool,
}

#[function_component(EpisodeModal)]
pub fn episode_modal(props: &EpisodeModalProps) -> Html {
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
                        {"Go to Episode Page"}
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
                    let _ = element.style().set_property("-webkit-touch-callout", "none");
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

    (on_touch_start, on_touch_end, on_touch_move, *is_long_press, *is_pressing)
}

pub fn episode_item(
    episode: Box<dyn EpisodeTrait>,
    description: String,
    _is_expanded: bool,
    format_release: &str,
    on_play_pause: Callback<MouseEvent>,
    on_shownotes_click: Callback<MouseEvent>,
    _toggle_expanded: Callback<MouseEvent>,
    episode_duration: i32,
    listen_duration: Option<i32>,
    page_type: &str,
    on_checkbox_change: Callback<i32>,
    is_delete_mode: bool, // Add this line
    ep_url: String,
    completed: bool,
    show_modal: bool,
    on_modal_open: Callback<i32>,
    on_modal_close: Callback<MouseEvent>,
    container_height: String,
    is_current_episode: bool,
    is_playing: bool,
) -> Html {
    let span_duration = listen_duration.clone();
    let span_episode = episode_duration.clone();
    let formatted_duration = format_time(span_episode as f64);
    let duration_clone = formatted_duration.clone();
    let duration_again = formatted_duration.clone();
    let formatted_listen_duration = span_duration.map(|ld| format_time(ld as f64));
    // Calculate the percentage of the episode that has been listened to
    let listen_duration_percentage = listen_duration.map_or(0.0, |ld| {
        if episode_duration > 0 {
            (ld as f64 / episode_duration as f64) * 100.0
        } else {
            0.0 // Avoid division by zero
        }
    });

    // Check if viewport is narrow (< 500px)
    let is_narrow_viewport = {
        let window = web_sys::window().expect("no global window exists");
        window.inner_width().unwrap().as_f64().unwrap() < 500.0
    };

    let checkbox_ep = episode.get_episode_id(Some(0));
    let should_show_buttons = !ep_url.is_empty();
    let preview_description = strip_images_from_html(&description);

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggleDescription(guid: &str, expanded: bool);
    }
    html! {
        <div>
            <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg" style={format!("height: {}; overflow: hidden;", container_height)}>
                {if is_delete_mode {
                    html! {
                        <input type="checkbox" class="form-checkbox h-5 w-5 text-blue-600"
                            onchange={on_checkbox_change.reform(move |_| checkbox_ep)} /> // Modify this line
                    }
                } else {
                    html! {}
                }}
                <div class="flex flex-col w-auto object-cover pl-4">
                    <FallbackImage
                        src={episode.get_episode_artwork()}
                        alt={format!("Cover for {}", episode.get_episode_title())}
                        class="episode-image"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <div class="flex items-center space-x-2 cursor-pointer" onclick={on_shownotes_click.clone()}>
                    <p class="item_container-text episode-title font-semibold line-clamp-2">
                        { episode.get_episode_title() }
                    </p>
                        {
                            if completed.clone() {
                                html! {
                                    <i class="ph ph-check-circle text-2xl text-green-500"></i>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <hr class="my-2 border-t hidden md:block"/>
                    {
                        html! {
                            <div class="item-description-text cursor-pointer hidden md:block"
                                onclick={let episode_id = episode.get_episode_id(None);
                                        Callback::from(move |e: MouseEvent| {
                                            e.prevent_default();
                                            on_modal_open.emit(episode_id);
                                        })}>
                                <div class="item_container-text line-clamp-2">
                                    <SafeHtml html={preview_description} />
                                </div>
                            </div>
                        }
                    }

                    <div class="episode-time-badge-container" style="max-width: 100%; overflow: hidden;">
                        <span
                            class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2"
                            style="flex-grow: 0; flex-shrink: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
                        >
                            <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                            </svg>
                            { format_release }
                        </span>
                    </div>
                    {
                        if completed {
                            // For completed episodes
                            if is_narrow_viewport {
                                // In narrow viewports, just show "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{"Completed"}</span>
                                    </div>
                                }
                            } else {
                                // In wider viewports, show duration and "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{ duration_clone }</span>
                                        <span class="item_container-text">{ "-  Completed" }</span>
                                    </div>
                                }
                            }
                        } else {
                            // For in-progress episodes
                            if formatted_listen_duration.is_some() {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        // Only show current position in wider viewports
                                        {
                                            if !is_narrow_viewport {
                                                html! {
                                                    <span class="item_container-text">{ formatted_listen_duration.clone() }</span>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        <div class="progress-bar-container">
                                            <div class="progress-bar" style={ format!("width: {}%;", listen_duration_percentage) }></div>
                                        </div>
                                        <span class="item_container-text">{ duration_again }</span>
                                    </div>
                                }
                            } else {
                                // For episodes with no listen progress
                                html! {
                                    <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                                }
                            }
                        }
                    }
                </div>
                {
                    html! {
                        <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                            if should_show_buttons {
                                <button
                                    class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                    onclick={on_play_pause}
                                >
                                    {
                                        if is_current_episode && is_playing {
                                            html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                                        } else {
                                            html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                                        }
                                    }
                                </button>
                                <div class="hidden sm:block"> // This will hide the context button below 640px
                                    <ContextButton episode={episode.clone()} page_type={page_type.to_string()} />
                                </div>
                            }
                        </div>
                    }
                }
            </div>
            if show_modal {
                <EpisodeModal
                    episode_id={episode.get_episode_id(None)}
                    episode_url={ep_url}
                    episode_artwork={episode.get_episode_artwork()}
                    episode_title={episode.get_episode_title()}
                    description={description.clone()}
                    format_release={format_release.to_string()}
                    duration={episode_duration}
                    on_close={on_modal_close}
                    on_show_notes={on_shownotes_click}
                    listen_duration_percentage={listen_duration_percentage}
                    is_youtube={episode.get_is_youtube()}
                />
            }
        </div>
    }
}

pub fn virtual_episode_item(
    episode: Box<dyn EpisodeTrait>,
    description: String,
    _is_expanded: bool,
    format_release: &str,
    on_play_pause: Callback<MouseEvent>,
    on_shownotes_click: Callback<MouseEvent>,
    _toggle_expanded: Callback<MouseEvent>,
    episode_duration: i32,
    listen_duration: Option<i32>,
    page_type: &str,
    on_checkbox_change: Callback<i32>,
    is_delete_mode: bool,
    ep_url: String,
    completed: bool,
    show_modal: bool,
    on_modal_open: Callback<MouseEvent>,
    on_modal_close: Callback<MouseEvent>,
    container_height: String,
    is_current_episode: bool,
    is_playing: bool,
    // New parameters for long press functionality
    on_touch_start: Callback<TouchEvent>,
    on_touch_end: Callback<TouchEvent>,
    on_touch_move: Callback<TouchEvent>,
    show_context_menu: bool,
    context_menu_position: (i32, i32),
    close_context_menu: Callback<()>,
    context_button_ref: NodeRef,
    is_pressing: bool,
) -> Html {
    let span_duration = listen_duration.clone();
    let span_episode = episode_duration.clone();
    let formatted_duration = format_time(span_episode as f64);
    let duration_clone = formatted_duration.clone();
    let duration_again = formatted_duration.clone();
    let formatted_listen_duration = span_duration.map(|ld| format_time(ld as f64));

    // Calculate the percentage of the episode that has been listened to
    let listen_duration_percentage = listen_duration.map_or(0.0, |ld| {
        if episode_duration > 0 {
            (ld as f64 / episode_duration as f64) * 100.0
        } else {
            0.0 // Avoid division by zero
        }
    });

    // Check if viewport is narrow (< 500px)
    let is_narrow_viewport = {
        let window = web_sys::window().expect("no global window exists");
        window.inner_width().unwrap().as_f64().unwrap() < 500.0
    };

    let checkbox_ep = episode.get_episode_id(Some(0));
    let should_show_buttons = !ep_url.is_empty();
    let preview_description = strip_images_from_html(&description);

    // Handle context menu position
    let context_menu_style = if show_context_menu {
        format!(
            "position: fixed; top: {}px; left: {}px; z-index: 1000;",
            context_menu_position.1, context_menu_position.0
        )
    } else {
        String::new()
    };

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggleDescription(guid: &str, expanded: bool);
    }

    html! {
        <div>
            <div
                class={classes!(
                    "item-container", "border-solid", "border", "flex", "items-start", "mb-4", 
                    "shadow-md", "rounded-lg", "touch-manipulation", "transition-all", "duration-150",
                    if is_pressing {
                        "bg-accent-color bg-opacity-20 transform scale-[0.98]"
                    } else {
                        ""
                    }
                )}
                style={format!("height: {}; overflow: hidden; user-select: {};", 
                    container_height,
                    if is_pressing { "none" } else { "auto" }
                )}
                ontouchstart={on_touch_start}
                ontouchend={on_touch_end}
                ontouchmove={on_touch_move}
            >
                {if is_delete_mode {
                    html! {
                        <input type="checkbox" class="form-checkbox h-5 w-5 text-blue-600"
                            onchange={on_checkbox_change.reform(move |_| checkbox_ep)} />
                    }
                } else {
                    html! {}
                }}
                <div class="flex flex-col w-auto object-cover pl-4">
                    <FallbackImage
                        src={episode.get_episode_artwork()}
                        alt={format!("Cover for {}", episode.get_episode_title())}
                        class="episode-image"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <div class="flex items-center space-x-2 cursor-pointer" onclick={on_shownotes_click.clone()}>
                    <p class="item_container-text episode-title font-semibold line-clamp-2">
                        { episode.get_episode_title() }
                    </p>
                    {
                            if completed.clone() {
                                html! {
                                    <i class="ph ph-check-circle text-2xl text-green-500"></i>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <hr class="my-2 border-t hidden md:block"/>
                    {
                        html! {
                            <div class="item-description-text cursor-pointer hidden md:block"
                                 onclick={on_modal_open}>
                                <div class="item_container-text line-clamp-2">
                                    <SafeHtml html={preview_description} />
                                </div>
                            </div>
                        }
                    }

                    <div class="episode-time-badge-container" style="max-width: 100%; overflow: hidden;">
                        <span
                            class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2"
                            style="flex-grow: 0; flex-shrink: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
                        >
                            <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                            </svg>
                            { format_release }
                        </span>
                    </div>
                    {
                        if completed {
                            // For completed episodes
                            if is_narrow_viewport {
                                // In narrow viewports, just show "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{"Completed"}</span>
                                    </div>
                                }
                            } else {
                                // In wider viewports, show duration and "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{ duration_clone }</span>
                                        <span class="item_container-text">{ "-  Completed" }</span>
                                    </div>
                                }
                            }
                        } else {
                            // For in-progress episodes
                            if formatted_listen_duration.is_some() {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        // Only show current position in wider viewports
                                        {
                                            if !is_narrow_viewport {
                                                html! {
                                                    <span class="item_container-text">{ formatted_listen_duration.clone() }</span>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        <div class="progress-bar-container">
                                            <div class="progress-bar" style={ format!("width: {}%;", listen_duration_percentage) }></div>
                                        </div>
                                        <span class="item_container-text">{ duration_again }</span>
                                    </div>
                                }
                            } else {
                                // For episodes with no listen progress
                                html! {
                                    <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                                }
                            }
                        }
                    }
                </div>
                {
                    html! {
                        <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                            if should_show_buttons {
                                <button
                                    class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                    onclick={on_play_pause}
                                >
                                    {
                                        if is_current_episode && is_playing {
                                            html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                                        } else {
                                            html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                                        }
                                    }
                                </button>
                                <div class="hidden sm:block"> // Standard desktop context button
                                    <div ref={context_button_ref.clone()}>
                                        <ContextButton episode={episode.clone()} page_type={page_type.to_string()} />
                                    </div>
                                </div>
                            }
                        </div>
                    }
                }
            </div>

            // This shows the context menu via long press
            {
                if show_context_menu {
                    html! {
                        <ContextButton
                            episode={episode.clone()}
                            page_type={page_type.to_string()}
                            show_menu_only={true}
                            position={Some(context_menu_position)}
                            on_close={close_context_menu}
                        />
                    }
                } else {
                    html! {}
                }
            }

            if show_modal {
                <EpisodeModal
                    episode_id={episode.get_episode_id(None)}
                    episode_url={ep_url}
                    episode_artwork={episode.get_episode_artwork()}
                    episode_title={episode.get_episode_title()}
                    description={description.clone()}
                    format_release={format_release.to_string()}
                    duration={episode_duration}
                    on_close={on_modal_close}
                    on_show_notes={on_shownotes_click}
                    listen_duration_percentage={listen_duration_percentage}
                    is_youtube={episode.get_is_youtube()}
                />
            }
        </div>
    }
}

pub fn download_episode_item(
    episode: Box<dyn EpisodeTrait>,
    description: String,
    _is_expanded: bool,
    format_release: &str,
    on_play_pause: Callback<MouseEvent>,
    on_shownotes_click: Callback<MouseEvent>,
    _toggle_expanded: Callback<MouseEvent>,
    episode_duration: i32,
    listen_duration: Option<i32>,
    page_type: &str,
    on_checkbox_change: Callback<i32>,
    is_delete_mode: bool, // Add this line
    ep_url: String,
    completed: bool,
    show_modal: bool,
    on_modal_open: Callback<MouseEvent>,
    on_modal_close: Callback<MouseEvent>,
    is_current_episode: bool,
    is_playing: bool,
    state: Rc<AppState>,
) -> Html {
    let span_duration = listen_duration.clone();
    let span_episode = episode_duration.clone();
    let formatted_duration = format_time(span_episode as f64);
    let duration_clone = formatted_duration.clone();
    let duration_again = formatted_duration.clone();
    let formatted_listen_duration = span_duration.map(|ld| format_time(ld as f64));
    let listen_duration_percentage = listen_duration.map_or(0.0, |ld| {
        if episode_duration > 0 {
            (ld as f64 / episode_duration as f64) * 100.0
        } else {
            0.0
        }
    });

    // Check if viewport is narrow (< 500px)
    let is_narrow_viewport = {
        let window = web_sys::window().expect("no global window exists");
        window.inner_width().unwrap().as_f64().unwrap() < 500.0
    };

    let checkbox_ep = episode.get_episode_id(Some(0));
    let should_show_buttons = !ep_url.is_empty();
    let preview_description = strip_images_from_html(&description);

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggleDescription(guid: &str, expanded: bool);
    }

    html! {
        <div>
        <div class="item-container border-solid border flex items-center mb-4 shadow-md rounded-lg h-full">
                // For the checkbox itself
                {if is_delete_mode {
                    html! {
                        <div class="flex items-center pl-4">
                            <input
                                type="checkbox"
                                checked={state.selected_episodes_for_deletion.contains(&episode.get_episode_id(Some(0)))}
                                class="podcast-dropdown-checkbox h-5 w-5 rounded border-2 text-primary focus:ring-primary focus:ring-offset-0 cursor-pointer appearance-none checked:bg-primary checked:border-primary"
                                onchange={on_checkbox_change.reform(move |_| checkbox_ep)}
                            />
                        </div>
                    }
                } else {
                    html! {}
                }}
                <div class="flex flex-col w-auto object-cover pl-4 self-start">
                    <FallbackImage
                        src={episode.get_episode_artwork()}
                        alt={format!("Cover for {}", episode.get_episode_title())}
                        class="episode-image"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12 self-start">
                    <div class="flex items-center space-x-2 cursor-pointer" onclick={on_shownotes_click.clone()}>
                        <p class="item_container-text episode-title font-semibold line-clamp-2">
                            { episode.get_episode_title() }
                        </p>
                        {
                            if completed.clone() {
                                html! {
                                    <i class="ph ph-check-circle text-2xl text-green-500"></i>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <hr class="my-2 border-t hidden md:block"/>
                    {
                        html! {
                            <div class="item-description-text cursor-pointer hidden md:block"
                                 onclick={on_modal_open}>
                                <div class="item_container-text line-clamp-2">
                                    <SafeHtml html={preview_description} />
                                </div>
                            </div>
                        }
                    }
                    <div class="episode-time-badge-container" style="max-width: 100%; overflow: hidden;">
                        <span
                            class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2"
                            style="flex-grow: 0; flex-shrink: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
                        >
                            <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                            </svg>
                            { format_release }
                        </span>
                    </div>
                    {
                        if completed {
                            // For completed episodes
                            if is_narrow_viewport {
                                // In narrow viewports, just show "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{"Completed"}</span>
                                    </div>
                                }
                            } else {
                                // In wider viewports, show duration and "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{ duration_clone }</span>
                                        <span class="item_container-text">{ "-  Completed" }</span>
                                    </div>
                                }
                            }
                        } else {
                            // For in-progress episodes
                            if formatted_listen_duration.is_some() {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        // Only show current position in wider viewports
                                        {
                                            if !is_narrow_viewport {
                                                html! {
                                                    <span class="item_container-text">{ formatted_listen_duration.clone() }</span>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        <div class="progress-bar-container">
                                            <div class="progress-bar" style={ format!("width: {}%;", listen_duration_percentage) }></div>
                                        </div>
                                        <span class="item_container-text">{ duration_again }</span>
                                    </div>
                                }
                            } else {
                                // For episodes with no listen progress
                                html! {
                                    <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                                }
                            }
                        }
                    }
                </div>
                {
                    html! {
                        <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                            if should_show_buttons {
                                <button
                                    class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                    onclick={on_play_pause}
                                >
                                    {
                                        if is_current_episode && is_playing {
                                            html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                                        } else {
                                            html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                                        }
                                    }
                                </button>
                                <div class="show-on-large">
                                    <ContextButton episode={episode.clone()} page_type={page_type.to_string()} />
                                </div>
                            }
                        </div>
                    }
                }
            </div>
            if show_modal {
                <EpisodeModal
                    episode_id={episode.get_episode_id(None)}
                    episode_url={ep_url}
                    episode_artwork={episode.get_episode_artwork()}
                    episode_title={episode.get_episode_title()}
                    description={description.clone()}
                    format_release={format_release.to_string()}
                    duration={episode_duration}
                    on_close={on_modal_close}
                    on_show_notes={on_shownotes_click}
                    listen_duration_percentage={listen_duration_percentage}
                    is_youtube={episode.get_is_youtube()}
                />
            }
        </div>
    }
}

pub fn queue_episode_item(
    episode: Box<dyn EpisodeTrait>,
    description: String,
    _is_expanded: bool,
    format_release: &str,
    on_play_pause: Callback<MouseEvent>,
    on_shownotes_click: Callback<MouseEvent>,
    _toggle_expanded: Callback<MouseEvent>,
    episode_duration: i32,
    listen_duration: Option<i32>,
    page_type: &str,
    on_checkbox_change: Callback<i32>,
    is_delete_mode: bool, // Add this line
    ep_url: String,
    completed: bool,
    ondragstart: Callback<DragEvent>,
    ondragenter: Callback<DragEvent>,
    ondragover: Callback<DragEvent>,
    ondrop: Callback<DragEvent>,
    ontouchstart: Callback<TouchEvent>,
    ontouchmove: Callback<TouchEvent>,
    ontouchend: Callback<TouchEvent>,
    show_modal: bool,
    on_modal_open: Callback<i32>,
    on_modal_close: Callback<MouseEvent>,
    is_current_episode: bool,
    is_playing: bool,
) -> Html {
    let span_duration = listen_duration.clone();
    let span_episode = episode_duration.clone();
    let formatted_duration = format_time(span_episode as f64);
    let duration_clone = formatted_duration.clone();
    let duration_again = formatted_duration.clone();
    let formatted_listen_duration = span_duration.map(|ld| format_time(ld as f64));
    // Calculate the percentage of the episode that has been listened to
    let listen_duration_percentage = listen_duration.map_or(0.0, |ld| {
        if episode_duration > 0 {
            (ld as f64 / episode_duration as f64) * 100.0
        } else {
            0.0 // Avoid division by zero
        }
    });

    // Check if viewport is narrow (< 500px)
    let is_narrow_viewport = {
        let window = web_sys::window().expect("no global window exists");
        window.inner_width().unwrap().as_f64().unwrap() < 500.0
    };

    let checkbox_ep = episode.get_episode_id(Some(0));
    let should_show_buttons = !ep_url.is_empty();
    let preview_description = strip_images_from_html(&description);

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggleDescription(guid: &str, expanded: bool);
    }

    html! {
        <div>
            <div
                class="item-container border-solid border flex mb-4 shadow-md rounded-lg"
                draggable="true"
                ondragstart={ondragstart.clone()}
                ondragenter={ondragenter.clone()}
                ondragover={ondragover.clone()}
                ondrop={ondrop.clone()}
                ontouchstart={ontouchstart}
                ontouchmove={ontouchmove}
                ontouchend={ontouchend}
                data-id={episode.get_episode_id(Some(0)).to_string()}
            >
                <div class="drag-handle-wrapper flex items-center justify-center w-10 h-full touch-none">
                    <button class="drag-handle cursor-grab">
                        <i class="ph ph-dots-six-vertical text-2xl"></i>
                    </button>
                </div>
            {if is_delete_mode {
                    html! {
                        <input type="checkbox" class="form-checkbox h-5 w-5 text-blue-600"
                            onchange={on_checkbox_change.reform(move |_| checkbox_ep)} /> // Modify this line
                    }
                } else {
                    html! {}
                }}
                <div class="flex flex-col w-auto object-cover pl-4">
                    <FallbackImage
                        src={episode.get_episode_artwork()}
                        alt={format!("Cover for {}", episode.get_episode_title())}
                        class="episode-image"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <div class="flex items-center space-x-2 cursor-pointer" onclick={on_shownotes_click.clone()}>
                        <p class="item_container-text episode-title font-semibold line-clamp-2">
                            { episode.get_episode_title() }
                        </p>
                        {
                            if completed.clone() {
                                html! {
                                    <i class="ph ph-check-circle text-2xl text-green-500"></i>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <hr class="my-2 border-t hidden md:block"/>
                    {
                        html! {
                            <div class="item-description-text cursor-pointer hidden md:block"
                                onclick={let episode_id = episode.get_episode_id(None);
                                        Callback::from(move |e: MouseEvent| {
                                            e.prevent_default();
                                            on_modal_open.emit(episode_id);
                                        })}>
                                <div class="item_container-text line-clamp-2">
                                    <SafeHtml html={preview_description} />
                                </div>
                            </div>
                        }
                    }
                    <div class="episode-time-badge-container" style="max-width: 100%; overflow: hidden;">
                        <span
                            class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2"
                            style="flex-grow: 0; flex-shrink: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
                        >
                            <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                            </svg>
                            { format_release }
                        </span>
                    </div>
                    {
                        if completed {
                            // For completed episodes
                            if is_narrow_viewport {
                                // In narrow viewports, just show "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{"Completed"}</span>
                                    </div>
                                }
                            } else {
                                // In wider viewports, show duration and "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{ duration_clone }</span>
                                        <span class="item_container-text">{ "-  Completed" }</span>
                                    </div>
                                }
                            }
                        } else {
                            // For in-progress episodes
                            if formatted_listen_duration.is_some() {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        // Only show current position in wider viewports
                                        {
                                            if !is_narrow_viewport {
                                                html! {
                                                    <span class="item_container-text">{ formatted_listen_duration.clone() }</span>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        <div class="progress-bar-container">
                                            <div class="progress-bar" style={ format!("width: {}%;", listen_duration_percentage) }></div>
                                        </div>
                                        <span class="item_container-text">{ duration_again }</span>
                                    </div>
                                }
                            } else {
                                // For episodes with no listen progress
                                html! {
                                    <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                                }
                            }
                        }
                    }
                </div>
                {
                    html! {
                        <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                            if should_show_buttons {
                                <button
                                    class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                    onclick={on_play_pause}
                                >
                                    {
                                        if is_current_episode && is_playing {
                                            html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                                        } else {
                                            html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                                        }
                                    }
                                </button>
                                <ContextButton episode={episode.clone()} page_type={page_type.to_string()} />
                            }
                        </div>
                    }
                }

            </div>

            if show_modal {
                <EpisodeModal
                    episode_id={episode.get_episode_id(None)}
                    episode_url={ep_url}
                    episode_artwork={episode.get_episode_artwork()}
                    episode_title={episode.get_episode_title()}
                    description={description.clone()}
                    format_release={format_release.to_string()}
                    duration={episode_duration}
                    on_close={on_modal_close}
                    on_show_notes={on_shownotes_click}
                    listen_duration_percentage={listen_duration_percentage}
                    is_youtube={episode.get_is_youtube()}
                />
            }
        </div>
    }
}

pub fn person_episode_item(
    episode: Box<dyn EpisodeTrait>,
    description: String,
    _is_expanded: bool,
    format_release: &str,
    on_play_pause: Callback<MouseEvent>,
    on_shownotes_click: Callback<MouseEvent>,
    _toggle_expanded: Callback<MouseEvent>,
    episode_duration: i32,
    listen_duration: Option<i32>,
    page_type: &str,
    on_checkbox_change: Callback<i32>,
    is_delete_mode: bool, // Add this line
    ep_url: String,
    completed: bool,
    show_modal: bool,
    on_modal_open: Callback<i32>,
    on_modal_close: Callback<MouseEvent>,
    is_current_episode: bool,
    is_playing: bool,
) -> Html {
    let span_duration = listen_duration.clone();
    let span_episode = episode_duration.clone();
    let formatted_duration = format_time(span_episode as f64);
    let duration_clone = formatted_duration.clone();
    let duration_again = formatted_duration.clone();
    let formatted_listen_duration = span_duration
        .filter(|&duration| duration > 0)
        .map(|ld| format_time(ld as f64));

    // Check if viewport is narrow (< 500px)
    let is_narrow_viewport = {
        let window = web_sys::window().expect("no global window exists");
        window.inner_width().unwrap().as_f64().unwrap() < 500.0
    };

    let listen_duration_percentage = listen_duration.map_or(0.0, |ld| {
        if episode_duration > 0 {
            (ld as f64 / episode_duration as f64) * 100.0
        } else {
            0.0
        }
    });
    let checkbox_ep = episode.get_episode_id(Some(0));
    let should_show_buttons = !ep_url.is_empty();
    let preview_description = strip_images_from_html(&description);

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggleDescription(guid: &str, expanded: bool);
    }

    html! {
        <div>
            <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full">
                {if is_delete_mode {
                    html! {
                        <input type="checkbox" class="form-checkbox h-5 w-5 text-blue-600"
                            onchange={on_checkbox_change.reform(move |_| checkbox_ep)} />
                    }
                } else {
                    html! {}
                }}
                <div class="flex flex-col w-auto object-cover pl-4">
                    <FallbackImage
                        src={episode.get_episode_artwork()}
                        alt={format!("Cover for {}", episode.get_episode_title())}
                        class="episode-image"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <div class="flex items-center space-x-2 cursor-pointer" onclick={on_shownotes_click.clone()}>
                    <p class="item_container-text episode-title font-semibold line-clamp-2">
                            { episode.get_episode_title() }
                        </p>
                        {
                            if completed.clone() {
                                html! {
                                    <i class="ph ph-check-circle text-2xl text-green-500"></i>
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    <hr class="my-2 border-t hidden md:block"/>
                    {
                        html! {
                            <div class="item-description-text cursor-pointer hidden md:block"
                                onclick={let episode_id = episode.get_episode_id(None);
                                        Callback::from(move |e: MouseEvent| {
                                            e.prevent_default();
                                            on_modal_open.emit(episode_id);
                                        })}>
                                <div class="item_container-text line-clamp-2">
                                    <SafeHtml html={preview_description} />
                                </div>
                            </div>
                        }
                    }
                    <div class="episode-time-badge-container" style="max-width: 100%; overflow: hidden;">
                        <span
                            class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2"
                            style="flex-grow: 0; flex-shrink: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
                        >
                            <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                            </svg>
                            { format_release }
                        </span>
                    </div>
                    {
                        if completed {
                            // For completed episodes
                            if is_narrow_viewport {
                                // In narrow viewports, just show "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{"Completed"}</span>
                                    </div>
                                }
                            } else {
                                // In wider viewports, show duration and "Completed"
                                html! {
                                    <div class="flex items-center space-x-2">
                                        <span class="item_container-text">{ duration_clone }</span>
                                        <span class="item_container-text">{ "-  Completed" }</span>
                                    </div>
                                }
                            }
                        } else {
                            // For in-progress episodes
                            if formatted_listen_duration.is_some() {
                                html! {
                                    <div class="flex items-center space-x-2">
                                        // Only show current position in wider viewports
                                        {
                                            if !is_narrow_viewport {
                                                html! {
                                                    <span class="item_container-text">{ formatted_listen_duration.clone() }</span>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        <div class="progress-bar-container">
                                            <div class="progress-bar" style={ format!("width: {}%;", listen_duration_percentage) }></div>
                                        </div>
                                        <span class="item_container-text">{ duration_again }</span>
                                    </div>
                                }
                            } else {
                                // For episodes with no listen progress
                                html! {
                                    <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                                }
                            }
                        }
                    }
                </div>
                {
                    html! {
                        <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;">
                            if should_show_buttons {
                                <button
                                    class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                    onclick={on_play_pause}
                                >
                                    {
                                        if is_current_episode && is_playing {
                                            html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                                        } else {
                                            html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                                        }
                                    }
                                </button>
                                <div class="hidden sm:block">
                                    <ContextButton episode={episode.clone()} page_type={page_type.to_string()} />
                                </div>
                            }
                        </div>
                    }
                }
            </div>
            if show_modal {
                <EpisodeModal
                    episode_id={episode.get_episode_id(None)}
                    episode_url={ep_url}
                    episode_artwork={episode.get_episode_artwork()}
                    episode_title={episode.get_episode_title()}
                    description={description.clone()}
                    format_release={format_release.to_string()}
                    duration={episode_duration}
                    on_close={on_modal_close}
                    on_show_notes={on_shownotes_click}
                    listen_duration_percentage={listen_duration_percentage}
                    is_youtube={episode.get_is_youtube()}
                />
            }
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct LoadingModalProps {
    pub name: String,
    pub is_visible: bool,
}

#[function_component(LoadingModal)]
pub fn loading_modal(props: &LoadingModalProps) -> Html {
    if !props.is_visible {
        return html! {};
    }

    html! {
        <div class="modal-overlay flex items-center justify-center">
            <div class="modal-content text-center">
                <div class="spinner mx-auto mb-4"></div>
                <p class="modal-title">{ format!("Searching everywhere for {}...", props.name) }</p>
                <p class="modal-subtitle mt-2">{"This may take a moment"}</p>
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
