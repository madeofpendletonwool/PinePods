use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{call_get_startpage, call_set_startpage, call_set_global_podcast_cover_preference};
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlSelectElement, HtmlInputElement};
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[function_component(StartPageOptions)]
pub fn startpage() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    // Use state to manage the selected start page
    let selected_startpage = use_state(|| "".to_string());
    let loading = use_state(|| true);
    
    // State for podcast cover preference
    let use_podcast_covers = use_state(|| false);
    let covers_loading = use_state(|| true);

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

    // Load podcast cover preference
    {
        let use_podcast_covers = use_podcast_covers.clone();
        let covers_loading = covers_loading.clone();
        let state = state.clone();

        use_effect_with((), move |_| {
            let use_podcast_covers = use_podcast_covers.clone();
            let covers_loading = covers_loading.clone();

            if let (Some(_api_key), Some(_user_id), Some(_server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                // TODO: Add API call to get current podcast cover preference
                // For now, default to false
                use_podcast_covers.set(false);
                covers_loading.set(false);
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
        let dispatch = _dispatch.clone();

        // Capture translated messages before move
        let success_msg = i18n.t("start_page_options.start_page_updated_successfully").to_string();
        let error_prefix = i18n.t("start_page_options.failed_to_update_start_page").to_string();
        Callback::from(move |_| {
            let success_msg = success_msg.clone();
            let error_prefix = error_prefix.clone();
            let dispatch = dispatch.clone();
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
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some(success_msg.clone());
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("{}{}", error_prefix.clone(), formatted_error));
                            });
                        }
                    }
                });
            }
        })
    };

    let on_covers_change = {
        let use_podcast_covers = use_podcast_covers.clone();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                use_podcast_covers.set(input.checked());
            }
        })
    };

    let on_covers_submit = {
        let use_podcast_covers = use_podcast_covers.clone();
        let state = state.clone();
        let dispatch = _dispatch.clone();

        // Capture translated messages before move
        let success_msg = "Podcast cover preference updated successfully".to_string();
        let error_prefix = "Failed to update podcast cover preference: ".to_string();
        Callback::from(move |_| {
            let success_msg = success_msg.clone();
            let error_prefix = error_prefix.clone();
            let dispatch = dispatch.clone();
            let covers_preference = *use_podcast_covers;

            // Update server
            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                spawn_local(async move {
                    match call_set_global_podcast_cover_preference(&server_name, &api_key, user_id, covers_preference)
                        .await
                    {
                        Ok(_) => {
                            dispatch.reduce_mut(|state| {
                                state.info_message = Some(success_msg.clone());
                            });
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch.reduce_mut(|state| {
                                state.error_message = Some(format!("{}{}", error_prefix.clone(), formatted_error));
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
        <div class="p-6 space-y-6">
            <div class="flex items-center gap-3 mb-6">
                <i class="ph ph-monitor text-2xl"></i>
                <h2 class="text-xl font-semibold item_container-text">{"Display Settings"}</h2>
            </div>

            // Start Page Settings Section
            <div class="border-b border-gray-200 pb-6">
                <div class="flex items-center gap-3 mb-4">
                    <i class="ph ph-house text-lg"></i>
                    <h3 class="text-lg font-medium item_container-text">{i18n.t("start_page_options.start_page_settings")}</h3>
                </div>

                <div class="mb-4">
                    <p class="item_container-text mb-2 text-sm">
                        {i18n.t("start_page_options.start_page_description")}
                    </p>
                </div>

                if *loading {
                    <div class="flex justify-center">
                        <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-500"></div>
                    </div>
                } else {
                    <div class="flex flex-col gap-3">
                        <div class="theme-select-container relative">
                            <select
                                onchange={on_change}
                                class="theme-select-dropdown w-full p-3 pr-10 rounded-lg border appearance-none cursor-pointer"
                                value={(*selected_startpage).clone()}
                            >
                                <option value="" disabled=true>{i18n.t("start_page_options.select_start_page")}</option>
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
                                <i class="ph ph-caret-down text-xl"></i>
                            </div>
                        </div>

                        <button
                            onclick={on_submit}
                            class="theme-submit-button w-full p-3 rounded-lg transition-colors duration-200 flex items-center justify-center gap-2"
                        >
                            <i class="ph ph-thumbs-up text-xl"></i>
                            {i18n.t("start_page_options.apply_start_page")}
                        </button>
                    </div>
                }
            </div>

            // Podcast Cover Settings Section
            <div>
                <div class="flex items-center gap-3 mb-4">
                    <i class="ph ph-image text-lg"></i>
                    <h3 class="text-lg font-medium item_container-text">{"Podcast Cover Display"}</h3>
                </div>

                <div class="mb-4">
                    <p class="item_container-text mb-2 text-sm">
                        {"When enabled, episodes will always show the podcast cover instead of the episode-specific artwork. This can help make episodes easier to identify by podcast."}
                    </p>
                </div>

                if *covers_loading {
                    <div class="flex justify-center">
                        <div class="animate-spin rounded-full h-6 w-6 border-b-2 border-gray-500"></div>
                    </div>
                } else {
                    <div class="flex flex-col gap-3">
                        <div class="flex items-center gap-3">
                            <input
                                type="checkbox"
                                id="use-podcast-covers"
                                checked={*use_podcast_covers}
                                onchange={on_covers_change}
                                class="podcast-dropdown-checkbox h-5 w-5 rounded border-2 text-primary focus:ring-primary focus:ring-offset-0 cursor-pointer appearance-none checked:bg-primary checked:border-primary"
                            />
                            <label for="use-podcast-covers" class="item_container-text text-sm">
                                {"Always use podcast covers instead of episode covers"}
                            </label>
                        </div>

                        <button
                            onclick={on_covers_submit}
                            class="theme-submit-button w-full p-3 rounded-lg transition-colors duration-200 flex items-center justify-center gap-2"
                        >
                            <i class="ph ph-thumbs-up text-xl"></i>
                            {"Apply Cover Settings"}
                        </button>
                    </div>
                }
            </div>
        </div>
    }
}
