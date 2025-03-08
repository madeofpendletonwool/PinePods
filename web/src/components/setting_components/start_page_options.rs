use crate::components::context::{AppState, UIState};
use crate::requests::setting_reqs::{call_get_startpage, call_set_startpage, SetStartPageRequest};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlSelectElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(StartPageOptions)]
pub fn startpage() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let (_audio_state, audio_dispatch) = use_store::<UIState>();
    // Use state to manage the selected start page
    let selected_startpage = use_state(|| "".to_string());
    let loading = use_state(|| true);

    {
        let selected_startpage = selected_startpage.clone();
        let loading = loading.clone();
        let state = state.clone();

        use_effect_with((), move |_| {
            let selected_startpage = selected_startpage.clone();
            let loading = loading.clone();

            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                spawn_local(async move {
                    match call_get_startpage(&server_name, &api_key, &user_id).await {
                        Ok(startpage) => {
                            selected_startpage.set(startpage);
                            loading.set(false);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error fetching start page: {:?}", e).into(),
                            );
                            loading.set(false);
                        }
                    }
                });
            }
            || ()
        });
    }

    let on_change = {
        let selected_startpage = selected_startpage.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<HtmlSelectElement>() {
                selected_startpage.set(select.value());
            }
        })
    };

    let on_submit = {
        let selected_startpage = selected_startpage.clone();
        let state = state.clone();

        Callback::from(move |_| {
            let audio_dispatch = audio_dispatch.clone();
            let startpage = (*selected_startpage).clone();

            if startpage.is_empty() {
                return;
            }

            // Store in local storage
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.set_item("selected_startpage", &startpage);
                }
            }

            // Update server
            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                let startpage_value = startpage.clone();
                spawn_local(async move {
                    match call_set_startpage(&server_name, &api_key, &user_id, &startpage_value)
                        .await
                    {
                        Ok(_) => {
                            audio_dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Start page updated successfully!".to_string());
                            });
                        }
                        Err(e) => {
                            audio_dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Failed to update start page: {}", e));
                            });
                        }
                    }
                });
            }
        })
    };

    let startpage_options = vec![
        ("Home", "home"),
        ("Feed", "feed"),
        ("Search", "search"),
        ("Queue", "queue"),
        ("Saved", "saved"),
        ("Downloads", "downloads"),
        ("People Subscriptions", "people_subs"),
        ("Podcasts", "podcasts"),
    ];

    html! {
        <div class="p-6 space-y-4">
            <div class="flex items-center gap-3 mb-6">
                <i class="ph ph-house text-2xl"></i>
                <h2 class="text-xl font-semibold item_container-text">{"Start Page Settings"}</h2>
            </div>

            <div class="mb-6">
                <p class="item_container-text mb-2">
                    {"Choose your preferred start page. This is the page you'll see first when opening Pinepods."}
                </p>
            </div>

            if *loading {
                <div class="flex justify-center">
                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-gray-500"></div>
                </div>
            } else {
                <div class="theme-select-container relative">
                    <select
                            onchange={on_change}
                            class="theme-select-dropdown w-full p-3 pr-10 rounded-lg border appearance-none cursor-pointer"
                            value={(*selected_startpage).clone()}
                        >
                            <option value="" disabled=true>{"Select a start page"}</option>
                            {startpage_options.into_iter().map(|(display_name, route)| {
                                let current_page = (*selected_startpage).clone();
                                html! {
                                    <option value={route} selected={route == current_page}>
                                        {display_name}
                                    </option>
                                }
                            }).collect::<Html>()}
                        </select>
                    <div class="absolute inset-y-0 right-0 flex items-center px-3 pointer-events-none">
                        <i class="ph ph-caret-down text-2xl"></i>
                    </div>
                </div>

                <button
                    onclick={on_submit}
                    class="theme-submit-button mt-4 w-full p-3 rounded-lg transition-colors duration-200 flex items-center justify-center gap-2"
                >
                    <i class="ph ph-thumbs-up text-2xl"></i>
                    {"Apply Start Page"}
                </button>
            }
        </div>
    }
}

pub fn initialize_default_startpage() {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            // Check if a start page is already set
            match storage.get_item("selected_startpage") {
                Ok(Some(startpage)) => {
                    // Use existing start page
                    storage
                        .set_item("selected_startpage", &startpage)
                        .unwrap_or_default();
                }
                _ => {
                    // No start page found, set home as default
                    storage
                        .set_item("selected_startpage", "home")
                        .unwrap_or_default();
                }
            }
        }
    }
}
