use super::routes::Route;
use crate::components::context::AppState;
use crate::components::navigation::use_back_button;
use crate::requests::pod_req::connect_to_episode_websocket;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::prelude::*;
use yew_router::prelude::Link;
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
    // let selection = use_state(|| "".to_string());
    // let (state, _dispatch) = use_store::<AppState>();

    let is_drawer_open = use_state(|| false);
    let drawer_rotation = if *is_drawer_open {
        "rotate-90 transform"
    } else {
        ""
    };
    let (state, _dispatch) = use_store::<AppState>();
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
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

        // Use Callback<MouseEvent> instead of just MouseEvent
        Callback::from(move |event: MouseEvent| {
            event.prevent_default();
            event.stop_propagation();

            let server_name_call = server_name.clone();
            let user_id_call = user_id.clone();
            let api_key_call = api_key.clone();
            let dispatch_clone = dispatch.clone();

            // Set refreshing state before starting
            dispatch_clone.reduce_mut(|state| {
                state.is_refreshing = Some(true);
                state.clone()
            });

            spawn_local(async move {
                web_sys::console::log_1(&"Starting refresh...".into());

                if let Err(e) = connect_to_episode_websocket(
                    &server_name_call.unwrap(),
                    &user_id_call.unwrap(),
                    &api_key_call.unwrap().unwrap(),
                    false,
                    dispatch_clone.clone(),
                )
                .await
                {
                    web_sys::console::log_1(&format!("Refresh failed: {:?}", e).into());
                }

                // Always reset the refreshing state
                dispatch_clone.reduce_mut(|state| {
                    state.is_refreshing = Some(false);
                    state.clone()
                });
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
    ]
    .iter()
    .any(|&path| current_path == path);

    #[cfg(not(feature = "server_build"))]
    let local_download_link = html! {
        <div class="flex items-center space-x-3">
            <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                <Link<Route> to={Route::LocalDownloads}>
                    <div class="flex items-center">
                        <i class="ph ph-folder-open text-2xl mr-3"></i>
                        <span class="text-lg">{"Local Downloads"}</span>
                    </div>
                </Link<Route>>
            </div>
        </div>
    };
    #[cfg(feature = "server_build")]
    let local_download_link = html! {};

    html! {
        <div class="relative">
            // Drawer
            <div class={classes!("fixed", "drawer-background", "top-0", "left-0", "z-20", "h-full", "transition-all", "duration-500", "transform", "shadow-lg", "md:w-64", "w-full", "border-solid", "border-b-2", "border-r-2", "border-color", (*is_drawer_open).then(|| "translate-x-0").unwrap_or("-translate-x-full"))}>
                <div class="flex flex-col justify-between h-full overflow-y-auto">
                    <div class="px-6 py-4 mt-16">
                        <div class="flex items-center gap-2">
                            <img
                                src="/static/assets/favicon.png"
                                alt="Pinepods Logo"
                                class="w-6 h-6"
                            />
                            <h2 class="drawer-text text-lg font-semibold">{"Pinepods"}</h2>
                        </div>
                        <hr class="my-4 drawer-hr" />
                        <div class="space-y-4">
                            // User Account with Gravatar
                        <div class="flex items-center space-x-3">
                            <Link<Route> to={Route::UserStats}>
                                <div class="align-avatar space-x-3">
                                    <img
                                        src={state.gravatar_url.clone().unwrap_or_else(|| "/static/assets/favicon.png".to_string())}
                                        style="width: 25px; height: 25px;"
                                        class="icon-size rounded-full" // Added rounded-full for circular image
                                        alt="User Avatar"
                                    />
                                    <span class="drawer-text text-lg text-adjust" style="margin-top: 7px; margin-left: -2px;">
                                        {username} // Displaying the username
                                    </span>
                                </div>
                            </Link<Route>>
                        </div>

                            // Other Links
                            <div class="m-0 p-0 flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Home}>
                                        <div class="flex items-center">
                                            <i class="ph ph-house text-2xl mr-3"></i>
                                            <span class="text-lg">{"Home"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="m-0 p-0 flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Feed}>
                                        <div class="flex items-center">
                                            <i class="ph ph-bell-ringing text-2xl mr-3"></i>
                                            <span class="text-lg">{"Feed"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Search}>
                                        <div class="flex items-center">
                                            <i class="ph ph-magnifying-glass text-2xl mr-3"></i>
                                            <span class="text-lg">{"Search Podcasts"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Queue}>
                                        <div class="flex items-center">
                                            <i class="ph ph-queue icon-space text-2xl mr-3"></i>
                                            <span class="text-lg">{"Queue"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Saved}>
                                        <div class="flex items-center">
                                            <i class="ph ph-star text-2xl mr-3"></i>
                                            <span class="text-lg">{"Saved"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::PodHistory}>
                                        <div class="flex items-center">
                                            <i class="ph ph-clock-counter-clockwise text-2xl mr-3"></i>
                                            <span class="text-lg">{"History"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                            <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                <Link<Route> to={Route::Downloads}>
                                    <div class="flex items-center">
                                        <i class="ph ph-download-simple text-2xl mr-3"></i>
                                        <span class="text-lg">{"Server Downloads"}</span>
                                    </div>
                                </Link<Route>>
                            </div>
                        </div>
                        {
                            {
                                local_download_link
                            }
                        }
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::SubscribedPeople}>
                                        <div class="flex items-center">
                                            <i class="ph ph-user text-2xl mr-3"></i>
                                            <span class="text-lg">{"Subscribed People"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Podcasts}>
                                        <div class="flex items-center">
                                            <i class="ph ph-microphone-stage text-2xl mr-3"></i>
                                            <span class="text-lg">{"Podcasts"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex items-center space-x-3">
                                <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                                    <Link<Route> to={Route::Settings}>
                                        <div class="flex items-center">
                                            <i class="ph ph-gear text-2xl mr-3"></i>
                                            <span class="text-lg">{"Settings"}</span>
                                        </div>
                                    </Link<Route>>
                                </div>
                            </div>
                            <div class="flex-grow"></div>
                                    </div>
                            <div class="px-1 py-4">
                <div class="flex items-center space-x-3">
                    <div onclick={toggle_drawer.clone()} class="drawer-text flex items-center space-x-3 cursor-pointer">
                        <Link<Route> to={Route::LogOut}>
                            <div class="flex items-center">
                                <i class="ph ph-sign-out text-2xl mr-3"></i>
                                <span class="text-lg">{"Sign Out"}</span>
                            </div>
                        </Link<Route>>
                    </div>
                </div>

                        </div>
                    </div>
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
                            onmouseup={on_refresh_click.clone()}  // Add this for better mobile handling
                            class="rounded-lg cursor-pointer touch-manipulation"  // Add touch-manipulation
                        >
                            <div class="flex flex-col items-center">
                                {
                                    if state.is_refreshing.unwrap_or(false) {
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

                {
                    match state.is_loading {
                        Some(true) => html! {
                            <div role="status" class="ml-3">
                                <svg aria-hidden="true" class="w-8 h-8 text-gray-200 animate-spin dark:text-gray-600 fill-blue-600" viewBox="0 0 100 101" fill="none" xmlns="http://www.w3.org/2000/svg">
                                    <path d="M100 50.5908C100 78.2051 77.6142 100.591 50 100.591C22.3858 100.591 0 78.2051 0 50.5908C0 22.9766 22.3858 0.59082 50 0.59082C77.6142 0.59082 100 22.9766 100 50.5908ZM9.08144 50.5908C9.08144 73.1895 27.4013 91.5094 50 91.5094C72.5987 91.5094 90.9186 73.1895 90.9186 50.5908C90.9186 27.9921 72.5987 9.67226 50 9.67226C27.4013 9.67226 9.08144 27.9921 9.08144 50.5908Z" fill="currentColor"/>
                                    <path d="M93.9676 39.0409C96.393 38.4038 97.8624 35.9116 97.0079 33.5539C95.2932 28.8227 92.871 24.3692 89.8167 20.348C85.8452 15.1192 80.8826 10.7238 75.2124 7.41289C69.5422 4.10194 63.2754 1.94025 56.7698 1.05124C51.7666 0.367541 46.6976 0.446843 41.7345 1.27873C39.2613 1.69328 37.813 4.19778 38.4501 6.62326C39.0873 9.04874 41.5694 10.4717 44.0505 10.1071C47.8511 9.54855 51.7191 9.52689 55.5402 10.0491C60.8642 10.7766 65.9928 12.5457 70.6331 15.2552C75.2735 17.9648 79.3347 21.5619 82.5849 25.841C84.9175 28.9121 86.7997 32.2913 88.1811 35.8758C89.083 38.2158 91.5421 39.6781 93.9676 39.0409Z" fill="currentFill"/>
                                </svg>
                                <span class="sr-only">{"Loading..."}</span>
                            </div>
                        },
                        _ => html! {}, // Covers both Some(false) and None
                    }
                }
                {
                    if let Some(progress) = &state.refresh_progress {
                        if !progress.current_podcast.is_empty() {
                            html! {
                                <span class="ml-3 text-sm item_container-text whitespace-nowrap">
                                    {format!("Refreshing {}", progress.current_podcast)}
                                </span>
                            }
                        } else {
                            html! {}
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </div>
        </div>
    }
}
