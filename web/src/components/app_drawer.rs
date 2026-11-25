use crate::components::context::{AppState, FilterState, NotificationState, PageLoadState, UserPreferencesState, UserStatsStore};
use crate::components::queue_panel::QueuePanel;
use crate::components::navigation::use_back_button;
use crate::pages::routes::Route;
use crate::requests::pod_req::{call_get_pinepods_version, connect_to_episode_websocket};
use crate::components::i18n::use_translation;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::prelude::*;
use yew_router::prelude::{use_route, Link};
use yewdux::prelude::Dispatch;
use yewdux::use_store;

#[function_component(BackButton)]
pub fn back_button() -> Html {
    let on_back = use_back_button();

    html! {
        <button
            onclick={Callback::from(move |e: MouseEvent| {
                e.stop_propagation();  // Stop event from bubbling up
                on_back.emit(e);
            })}
            class="back-button flex items-center space-x-2 px-4 py-2 rounded-lg"
        >
            <div class="flex flex-col items-center">
                <i class="ph ph-arrow-bend-up-left md:text-4xl text-4xl"></i>
            </div>
        </button>
    }
}

#[allow(non_camel_case_types)]
#[function_component(App_drawer)]
pub fn app_drawer() -> Html {
    let (i18n, _) = use_translation();
    let (stats_state, stats_dispatch) = use_store::<UserStatsStore>();
    // let selection = use_state(|| "".to_string());
    // let (state, _dispatch) = use_store::<AppState>();

    // Capture i18n strings before they get moved
    let i18n_pinepods = i18n.t("app_drawer.pinepods").to_string();
    let i18n_home = i18n.t("navigation.home").to_string();
    let i18n_feed = i18n.t("app_drawer.feed").to_string();
    let i18n_inbox = i18n.t("app_drawer.inbox").to_string();
    let i18n_search = i18n.t("navigation.search").to_string();
    let i18n_collections = i18n.t("navigation.collections").to_string();
    let i18n_playlists = i18n.t("navigation.playlists").to_string();
    let i18n_history = i18n.t("navigation.history").to_string();
    let i18n_server_downloads = i18n.t("app_drawer.server_downloads").to_string();
    let i18n_local_downloads = i18n.t("app_drawer.local_downloads").to_string();
    let i18n_subscribed_people = i18n.t("app_drawer.subscribed_people").to_string();
    let i18n_discover = i18n.t("app_drawer.discover").to_string();
    let i18n_podcasts = i18n.t("navigation.podcasts").to_string();
    let i18n_settings = i18n.t("app_drawer.settings").to_string();
    let i18n_sign_out = i18n.t("app_drawer.sign_out").to_string();
    let i18n_loading = i18n.t("common.loading").to_string();
    let i18n_listen = i18n.t("app_drawer.section_listen").to_string();
    let i18n_library = i18n.t("app_drawer.section_library").to_string();
    let i18n_explore = i18n.t("app_drawer.section_explore").to_string();

    let is_drawer_open = use_state(|| false);
    let drawer_rotation = if *is_drawer_open {
        "rotate-90 transform"
    } else {
        ""
    };
    let (state, _dispatch) = use_store::<AppState>();
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (prefs_state, _) = use_store::<UserPreferencesState>();
    let (load_state, _) = use_store::<PageLoadState>();
    let (_, notif_dispatch) = use_store::<NotificationState>();
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let use_gravatar = post_state.user_details.as_ref().map_or(false, |ud| ud.UseGravatar);
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    // Fetch version on component mount if authenticated
    {
        let stats_dispatch = stats_dispatch.clone();
        let server_name_version = server_name.clone();
        let api_key_version = api_key.clone();

        use_effect_with((api_key.clone(), server_name.clone()), move |_| {
            if let (Some(api_key), Some(server_name)) =
                (api_key_version.clone(), server_name_version.clone())
            {
                let stats_dispatch = stats_dispatch.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Ok(version) =
                        call_get_pinepods_version(server_name.clone(), &api_key).await
                    {
                        stats_dispatch.reduce_mut(move |state| {
                            state.pinepods_version = Some(version);
                        });
                    }
                });
            }
            || ()
        });
    }
    // let session_state = state.clone();
    let username = state
        .user_details
        .as_ref()
        .map_or("Guest".to_string(), |ud| ud.Username.clone().unwrap());
    let toggle_drawer = {
        let is_drawer_open = is_drawer_open.clone();
        move |_event: MouseEvent| {
            is_drawer_open.set(!*is_drawer_open);
            if let Some(window) = web_sys::window() {
                let body = window.document().unwrap().body().unwrap();
                if !*is_drawer_open {
                    body.class_list().add_1("no-scroll").unwrap();
                } else {
                    body.class_list().remove_1("no-scroll").unwrap();
                }
            }
        }
    };
    let on_refresh_click = {
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let api_key = api_key.clone();
        let dispatch = _dispatch.clone();
        let notif_dispatch_refresh = notif_dispatch.clone();

        // Use Callback<MouseEvent> instead of just MouseEvent
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            event.stop_propagation();

            let server_name_call = server_name.clone();
            let user_id_call = user_id.clone();
            let api_key_call = api_key.clone();
            let _dispatch_clone = dispatch.clone();
            let notif_dispatch_clone = notif_dispatch_refresh.clone();

            // Set refreshing state before starting
            Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                state.is_refreshing = Some(true);
            });

            spawn_local(async move {
                web_sys::console::log_1(&"Starting refresh...".into());

                match connect_to_episode_websocket(
                    &server_name_call.unwrap(),
                    &user_id_call.unwrap(),
                    &api_key_call.unwrap().unwrap(),
                    false,
                    notif_dispatch_clone,
                )
                .await
                {
                    Ok(_) => {
                        web_sys::console::log_1(&"Refresh completed successfully".into());
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Refresh failed: {:?}", e).into());
                    }
                }

                // Reset the refreshing state after websocket completes
                Dispatch::<PageLoadState>::global().reduce_mut(|state| {
                    state.is_refreshing = Some(false);
                });
            });
        })
    };

    let (filter_state, filter_dispatch) = use_store::<FilterState>();
    let toggle_favorites_filter = {
        let filter_dispatch = filter_dispatch.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            filter_dispatch.reduce_mut(|state| {
                state.favorites_only = !state.favorites_only;
            });
        })
    };

    let current_path = window()
        .unwrap()
        .location()
        .pathname()
        .unwrap_or_else(|_| String::new());

    let show_home_button = current_path != "/home";
    let show_refresh_button = current_path == "/home";
    let show_back_button = ![
        "/login",
        "/home",
        "/queue",
        "/saved",
        "/downloads",
        "/people-subs",
        "/podcasts",
        "/user_stats",
        "/settings",
        "/search",
        "/local_downloads",
        "/people_subs",
        "/feed",
        "/inbox",
        "/playlists",
    ]
    .iter()
    .any(|&path| current_path == path);

    let current_route = use_route::<Route>();
    let nav_item = |route: Route, icon: &str, label: &str| -> Html {
        let active = current_route.as_ref() == Some(&route);
        html! {
            <div onclick={toggle_drawer.clone()}>
                <Link<Route> to={route} classes={classes!("sb-item", active.then_some("is-active"))}>
                    <i class={classes!("ph", icon.to_string())}></i>
                    <span>{label.to_string()}</span>
                </Link<Route>>
            </div>
        }
    };

    // The inbox count, refreshed whenever the drawer opens.
    let inbox_count = yewdux::use_selector(|s: &crate::components::labels::LabelState| s.inbox_count);
    {
        let open = *is_drawer_open;
        use_effect_with(open, move |open| {
            if *open || Dispatch::<crate::components::labels::LabelState>::global().get().inbox_count.is_none() {
                crate::components::labels::refresh_inbox_count();
            }
            || ()
        });
    }
    let inbox_item = {
        let active = current_route.as_ref() == Some(&Route::Inbox);
        html! {
            <div onclick={toggle_drawer.clone()}>
                <Link<Route> to={Route::Inbox} classes={classes!("sb-item", active.then_some("is-active"))}>
                    <i class="ph ph-tray"></i>
                    <span>{i18n_inbox.clone()}</span>
                    if let Some(n) = (*inbox_count).filter(|n| *n > 0) {
                        <span class="sb-badge">{ if n > 999 { "999+".to_string() } else { n.to_string() } }</span>
                    }
                </Link<Route>>
            </div>
        }
    };

    let local_download_link = if crate::desktop::is_desktop() {
        nav_item(Route::LocalDownloads, "ph-folder-open", &i18n_local_downloads)
    } else {
        html! {}
    };

    html! {
        <div class="relative">
            <QueuePanel />
            // Clicking anywhere outside the open drawer closes it
            if *is_drawer_open {
                <div class="sb-backdrop" onclick={toggle_drawer.clone()}></div>
            }
            // Sidebar drawer
            <div class={classes!("fixed", "drawer-background", "top-0", "left-0", "z-20", "h-full", "transition-all", "duration-300", "transform", "shadow-lg", "md:w-64", "w-less-full", (*is_drawer_open).then(|| "translate-x-0").unwrap_or("-translate-x-full"))}>
                // Account header: avatar, name, app name + version
                <div onclick={toggle_drawer.clone()} class="sb-account" style="margin-top: 60px;">
                    <Link<Route> to={Route::UserStats} classes={classes!("sb-account-link", (current_route == Some(Route::UserStats)).then_some("is-active"))}>
                        <img
                            src={prefs_state.gravatar_url.clone().filter(|_| use_gravatar).unwrap_or_else(|| "/static/assets/favicon.png".to_string())}
                            class="sb-avatar"
                            alt="User avatar"
                        />
                        <span class="sb-account-text">
                            <span class="sb-username">{username.clone()}</span>
                            <span class="sb-app">
                                {&i18n_pinepods}
                                { stats_state.pinepods_version.as_ref().map(|v| format!(" v{v}")).unwrap_or_default() }
                            </span>
                        </span>
                    </Link<Route>>
                </div>

                // Navigation links
                <nav class="sb-nav">
                    <div class="sb-section">{&i18n_listen}</div>
                    { nav_item(Route::Home, "ph-house", &i18n_home) }
                    { nav_item(Route::Feed, "ph-bell-ringing", &i18n_feed) }
                    { inbox_item }
                    { nav_item(Route::Collections, "ph-bookmark-simple", &i18n_collections) }
                    { nav_item(Route::Playlists, "ph-list-checks", &i18n_playlists) }
                    { nav_item(Route::PodHistory, "ph-clock-counter-clockwise", &i18n_history) }
                    { nav_item(Route::Downloads, "ph-download-simple", &i18n_server_downloads) }
                    { local_download_link }
                    <div class="sb-section">{&i18n_library}</div>
                    { nav_item(Route::Podcasts, "ph-microphone-stage", &i18n_podcasts) }
                    { nav_item(Route::SubscribedPeople, "ph-user", &i18n_subscribed_people) }
                    { nav_item(Route::Search, "ph-magnifying-glass", &i18n_search) }
                    <div class="sb-section">{&i18n_explore}</div>
                    { nav_item(Route::Discover, "ph-compass", &i18n_discover) }
                </nav>

                // Settings + sign out at bottom
                <div class="sb-bottom">
                    <hr class="sb-hr" />
                    { nav_item(Route::Settings, "ph-gear", &i18n_settings) }
                    { nav_item(Route::LogOut, "ph-sign-out", &i18n_sign_out) }
                </div>
            </div>

        <div class="drawer-icon flex items-center" onclick={toggle_drawer.clone()}>
            <button class="cursor-pointer flex items-center justify-center">
                <i class={classes!("ph", "ph-list", "text-4xl", "transition-transform", "duration-300", drawer_rotation)}></i>
            </button>
            <div class="w-8 h-8 ml-3 flex items-center">

                { if show_home_button {
                    html! {
                        <Link<Route> to={Route::Home} classes="rounded-lg cursor-pointer">
                            <div class="flex flex-col items-center">
                                <i class="ph ph-house md:text-4xl text-4xl"></i>
                            </div>
                        </Link<Route>>
                    }
                } else {
                    html! {}
                }}
                { if show_back_button {
                    html! {
                        <BackButton />
                    }
                } else {
                    html! {}
                }}
                { if show_refresh_button {
                    html! {
                        <button
                            onclick={on_refresh_click.clone()}
                            onmouseup={on_refresh_click.clone()}
                            class="ml-2 rounded-lg cursor-pointer touch-manipulation"
                        >
                            <div class="flex flex-col items-center">
                                {
                                    if load_state.is_refreshing.unwrap_or(false) {
                                        html! {
                                            <div class="flex flex-col items-center">
                                                <i class="ph ph-hourglass-medium md:text-4xl text-4xl"></i>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <i class="ph ph-arrows-clockwise md:text-4xl text-4xl"></i>
                                        }
                                    }
                                }
                            </div>
                        </button>
                    }
                } else {
                    html! {}
                }}
                { if !show_back_button {
                    let star_class = if filter_state.favorites_only {
                        classes!("ph", "ph-star", "md:text-4xl", "text-4xl", "text-yellow-400")
                    } else {
                        classes!("ph", "ph-star", "md:text-4xl", "text-4xl")
                    };
                    html! {
                        <button
                            onclick={toggle_favorites_filter.clone()}
                            class="ml-2 rounded-lg cursor-pointer"
                            title="Toggle favorites filter"
                        >
                            <div class="flex flex-col items-center">
                                <i class={star_class}></i>
                            </div>
                        </button>
                    }
                } else {
                    html! {}
                }}

                {
                    match load_state.is_loading {
                        Some(true) => html! {
                            <div role="status" class="ml-3">
                                <svg aria-hidden="true" class="w-8 h-8 text-gray-200 animate-spin dark:text-gray-600 fill-blue-600" viewBox="0 0 100 101" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    <path d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z" fill="currentColor"/>
                                    <path d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z" fill="currentFill"/>
                                </svg>
                                <span class="sr-only">{&i18n_loading}</span>
                            </div>
                        },
                        _ => html! {}, // Covers both Some(false) and None
                    }
                }
            </div>
        </div>
        </div>
    }
}
