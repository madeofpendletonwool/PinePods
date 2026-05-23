use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{call_get_startpage, call_set_startpage, call_set_global_podcast_cover_preference, call_get_podcast_cover_preference};
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

            if let (Some(api_key), Some(user_id), Some(server_name)) = (
                state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                state.user_details.as_ref().map(|d| d.UserID),
                state.auth_details.as_ref().map(|d| d.server_name.clone()),
            ) {
                let use_podcast_covers = use_podcast_covers.clone();
                let covers_loading = covers_loading.clone();
                
                spawn_local(async move {
                    match call_get_podcast_cover_preference(&server_name, &api_key, user_id, None).await {
                        Ok(current_preference) => {
                            use_podcast_covers.set(current_preference);
                            covers_loading.set(false);
                        }
                        Err(_e) => {
                            // If API call fails, default to false
                            use_podcast_covers.set(false);
                            covers_loading.set(false);
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
        let state = state.clone();
        let dispatch = _dispatch.clone();
        let success_msg = "Podcast cover preference updated successfully".to_string();
        let error_prefix = "Failed to update podcast cover preference: ".to_string();
        Callback::from(move |e: Event| {
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                let new_value = input.checked();
                use_podcast_covers.set(new_value);
                let success_msg = success_msg.clone();
                let error_prefix = error_prefix.clone();
                let dispatch = dispatch.clone();
                if let (Some(api_key), Some(user_id), Some(server_name)) = (
                    state.auth_details.as_ref().and_then(|d| d.api_key.clone()),
                    state.user_details.as_ref().map(|d| d.UserID),
                    state.auth_details.as_ref().map(|d| d.server_name.clone()),
                ) {
                    spawn_local(async move {
                        match call_set_global_podcast_cover_preference(&server_name, &api_key, user_id, new_value, None).await {
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
        <>
            <div class="settings-row">
                <div><div class="settings-row-label">{i18n.t("start_page_options.select_start_page")}</div></div>
                <div class="settings-row-control">
                    if *loading {
                        <i class="ph ph-spinner"></i>
                    } else {
                        <>
                            <select
                                onchange={on_change}
                                class="select"
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
                            <button onclick={on_submit} class="btn btn-primary">
                                {i18n.t("start_page_options.apply_start_page")}
                            </button>
                        </>
                    }
                </div>
            </div>

            <div class="settings-subsection-title">{"Podcast Cover Display"}</div>

            <div class="settings-row">
                <div><div class="settings-row-label">{"Always use podcast covers instead of episode covers"}</div></div>
                <div class="settings-row-control">
                    if *covers_loading {
                        <i class="ph ph-spinner"></i>
                    } else {
                        <label class="toggle">
                            <input
                                type="checkbox"
                                checked={*use_podcast_covers}
                                onchange={on_covers_change}
                            />
                            <span class="toggle-track"><span class="toggle-thumb"></span></span>
                        </label>
                    }
                </div>
            </div>
        </>
    }
}
