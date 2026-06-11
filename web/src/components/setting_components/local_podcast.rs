use crate::components::context::{AppState, NotificationState};
use crate::components::gen_components::ImagePicker;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_add_local_podcast, call_add_local_podcast_artwork, call_detect_local_cover,
    call_list_local_directories, call_refresh_local_podcast, LocalDirEntry,
};
use web_sys::{FormData, HtmlInputElement, HtmlTextAreaElement};
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[function_component(LocalPodcast)]
pub fn local_podcast() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();

    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    let podcast_name = use_state(|| "".to_string());
    let directory_path = use_state(|| "".to_string());
    let description = use_state(|| "".to_string());
    let author = use_state(|| "".to_string());
    let explicit = use_state(|| false);
    let artwork_file: UseStateHandle<Option<web_sys::File>> = use_state(|| None);
    let is_loading = use_state(|| false);
    let added_podcast_id: UseStateHandle<Option<i32>> = use_state(|| None);
    let is_refreshing = use_state(|| false);
    // Bumped after a successful add to force-remount the ImagePicker (clears its preview).
    let form_version = use_state(|| 0u32);

    // Directory browser modal state
    let show_dir_browser = use_state(|| false);
    let browser_path = use_state(|| "".to_string());
    let browser_entries: UseStateHandle<Vec<LocalDirEntry>> = use_state(Vec::new);
    let browser_loading = use_state(|| false);

    // Cover art auto-detected from the chosen directory (full URL), shown as the default
    // artwork preview until the user overrides it with their own image.
    let detected_cover_url: UseStateHandle<Option<String>> = use_state(|| None);

    let update_podcast_name = {
        let podcast_name = podcast_name.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            podcast_name.set(input.value());
        })
    };

    let update_directory_path = {
        let directory_path = directory_path.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            directory_path.set(input.value());
        })
    };

    let update_description = {
        let description = description.clone();
        Callback::from(move |e: InputEvent| {
            // The description field is a <textarea>, so it must be cast to
            // HtmlTextAreaElement (not HtmlInputElement, which would never match).
            if let Some(input) = e.target_dyn_into::<HtmlTextAreaElement>() {
                description.set(input.value());
            }
        })
    };

    let update_author = {
        let author = author.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            author.set(input.value());
        })
    };

    let toggle_explicit = {
        let explicit = explicit.clone();
        Callback::from(move |_: MouseEvent| {
            explicit.set(!*explicit);
        })
    };

    let on_artwork_change = {
        let artwork_file = artwork_file.clone();
        Callback::from(move |file: Option<web_sys::File>| {
            artwork_file.set(file);
        })
    };

    // Fetch the directory listing for a given relative path and update the browser modal.
    let navigate_dir = {
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let browser_path = browser_path.clone();
        let browser_entries = browser_entries.clone();
        let browser_loading = browser_loading.clone();
        Callback::from(move |path: String| {
            let api_key = api_key.clone().unwrap_or_default();
            let server_name = server_name.clone();
            let browser_path = browser_path.clone();
            let browser_entries = browser_entries.clone();
            let browser_loading = browser_loading.clone();
            browser_loading.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                match call_list_local_directories(&server_name, &api_key, &path).await {
                    Ok(listing) => {
                        browser_path.set(listing.current_path);
                        browser_entries.set(listing.directories);
                    }
                    Err(e) => {
                        let formatted = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!("Failed to list directories: {}", formatted));
                        });
                    }
                }
                browser_loading.set(false);
            });
        })
    };

    // Fetch the auto-detected cover art for a directory and store its full URL.
    let detect_cover = {
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let detected_cover_url = detected_cover_url.clone();
        Callback::from(move |path: String| {
            let detected_cover_url = detected_cover_url.clone();
            if path.trim().is_empty() {
                detected_cover_url.set(None);
                return;
            }
            let api_key = api_key.clone().unwrap_or_default();
            let server_name = server_name.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_detect_local_cover(&server_name, &api_key, &path).await {
                    Ok(Some(rel_url)) => {
                        detected_cover_url.set(Some(format!("{}{}", server_name, rel_url)));
                    }
                    _ => {
                        detected_cover_url.set(None);
                    }
                }
            });
        })
    };

    let open_dir_browser = {
        let show_dir_browser = show_dir_browser.clone();
        let navigate_dir = navigate_dir.clone();
        Callback::from(move |_: MouseEvent| {
            show_dir_browser.set(true);
            navigate_dir.emit(String::new());
        })
    };

    // When the user finishes editing the manual path field, refresh the detected cover.
    let on_dir_blur = {
        let detect_cover = detect_cover.clone();
        let directory_path = directory_path.clone();
        Callback::from(move |_: FocusEvent| {
            detect_cover.emit((*directory_path).clone());
        })
    };

    let close_dir_browser = {
        let show_dir_browser = show_dir_browser.clone();
        Callback::from(move |_: MouseEvent| {
            show_dir_browser.set(false);
        })
    };

    let select_current_dir = {
        let show_dir_browser = show_dir_browser.clone();
        let directory_path = directory_path.clone();
        let browser_path = browser_path.clone();
        let detect_cover = detect_cover.clone();
        Callback::from(move |_: MouseEvent| {
            let selected = (*browser_path).clone();
            directory_path.set(selected.clone());
            show_dir_browser.set(false);
            detect_cover.emit(selected);
        })
    };

    let go_up_dir = {
        let navigate_dir = navigate_dir.clone();
        let browser_path = browser_path.clone();
        Callback::from(move |_: MouseEvent| {
            let current = (*browser_path).clone();
            let parent = match current.rfind('/') {
                Some(idx) => current[..idx].to_string(),
                None => String::new(),
            };
            navigate_dir.emit(parent);
        })
    };

    let add_local_podcast_cb = {
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let user_id = user_id.clone();
        let podcast_name = podcast_name.clone();
        let directory_path = directory_path.clone();
        let description = description.clone();
        let author = author.clone();
        let explicit = explicit.clone();
        let artwork_file = artwork_file.clone();
        let is_loading = is_loading.clone();
        let added_podcast_id = added_podcast_id.clone();
        let form_version = form_version.clone();
        let detected_cover_url = detected_cover_url.clone();
        let success_msg = i18n.t("local_podcast.podcast_added").to_string();
        let error_prefix = i18n.t("local_podcast.failed_to_add").to_string();

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let user_id_val = user_id.clone().unwrap_or_default();
            let name_val = (*podcast_name).clone();
            let dir_val = (*directory_path).clone();
            let description_opt = if description.is_empty() { None } else { Some((*description).clone()) };
            let author_opt = if author.is_empty() { None } else { Some((*author).clone()) };
            let explicit_val = *explicit;
            let artwork_file = artwork_file.clone();
            let is_loading = is_loading.clone();
            let added_podcast_id = added_podcast_id.clone();
            let success_msg = success_msg.clone();
            let error_prefix = error_prefix.clone();
            // Handles for resetting the form after a successful add
            let podcast_name = podcast_name.clone();
            let directory_path = directory_path.clone();
            let description = description.clone();
            let author = author.clone();
            let explicit_state = explicit.clone();
            let form_version = form_version.clone();
            let detected_cover_url = detected_cover_url.clone();

            if name_val.is_empty() || dir_val.is_empty() {
                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                    state.error_message = Some("Podcast name and directory path are required.".to_string());
                });
                return;
            }

            is_loading.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                let api_key_str = api_key.unwrap_or_default();
                match call_add_local_podcast(
                    &server_name,
                    user_id_val,
                    &dir_val,
                    &name_val,
                    description_opt,
                    author_opt,
                    Some(explicit_val),
                    &api_key_str,
                )
                .await
                {
                    Ok(podcast) => {
                        let podcast_id = podcast.podcastid;
                        added_podcast_id.set(Some(podcast_id));

                        // Upload artwork if a file was selected
                        if let Some(file) = (*artwork_file).clone() {
                            if let Ok(form_data) = FormData::new() {
                                let _ = form_data.append_with_str("podcast_id", &podcast_id.to_string());
                                let _ = form_data.append_with_str("user_id", &user_id_val.to_string());
                                let _ = form_data.append_with_blob_and_filename("artwork", &file, &file.name());
                                let _ = call_add_local_podcast_artwork(
                                    &server_name,
                                    &api_key_str,
                                    form_data,
                                )
                                .await;
                            }
                        }

                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(success_msg.clone());
                        });

                        // Reset the form so it's clear the add succeeded and ready for the next.
                        podcast_name.set(String::new());
                        directory_path.set(String::new());
                        description.set(String::new());
                        author.set(String::new());
                        explicit_state.set(false);
                        artwork_file.set(None);
                        detected_cover_url.set(None);
                        form_version.set(*form_version + 1);
                    }
                    Err(e) => {
                        let formatted = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!("{}{}", error_prefix, formatted));
                        });
                    }
                }
                is_loading.set(false);
            });
        })
    };

    let refresh_podcast_cb = {
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let user_id = user_id.clone();
        let added_podcast_id = added_podcast_id.clone();
        let dispatch = dispatch.clone();
        let is_refreshing = is_refreshing.clone();

        Callback::from(move |_| {
            let podcast_id = match *added_podcast_id {
                Some(id) => id,
                None => return,
            };
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let user_id_val = user_id.clone().unwrap_or_default();
            let dispatch = dispatch.clone();
            let is_refreshing = is_refreshing.clone();

            is_refreshing.set(true);
            wasm_bindgen_futures::spawn_local(async move {
                match call_refresh_local_podcast(
                    &server_name,
                    user_id_val,
                    podcast_id,
                    &api_key.unwrap_or_default(),
                )
                .await
                {
                    Ok(result) => {
                        let count = result["new_episodes"].as_i64().unwrap_or(0);
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.info_message = Some(format!("Refresh complete: {} new episode(s) added", count));
                        });
                    }
                    Err(e) => {
                        let formatted = format_error_message(&e.to_string());
                        Dispatch::<NotificationState>::global().reduce_mut(|state| {
                            state.error_message = Some(format!("Failed to refresh: {}", formatted));
                        });
                    }
                }
                is_refreshing.set(false);
            });
        })
    };

    html! {
        <>
            <div class="settings-row">
                <div><div class="settings-row-label">{i18n.t("local_podcast.podcast_name_placeholder")}</div></div>
                <div class="settings-row-control">
                    <input
                        id="local_podcast_name"
                        oninput={update_podcast_name}
                        value={(*podcast_name).clone()}
                        class="input"
                        placeholder={i18n.t("local_podcast.podcast_name_placeholder")}
                    />
                </div>
            </div>
            <div class="settings-row">
                <div>
                    <div class="settings-row-label">{i18n.t("local_podcast.directory_path_placeholder")}</div>
                    <div class="settings-row-desc">{i18n.t("local_podcast.directory_path_help")}</div>
                </div>
                <div class="settings-row-control" style="display:flex; gap:8px; align-items:center;">
                    <input
                        id="local_podcast_dir"
                        oninput={update_directory_path}
                        onblur={on_dir_blur}
                        value={(*directory_path).clone()}
                        class="input"
                        placeholder={i18n.t("local_podcast.directory_path_placeholder")}
                    />
                    <button
                        type="button"
                        onclick={open_dir_browser}
                        class="btn btn-secondary"
                        style="padding:6px 12px; white-space:nowrap;"
                    >
                        <i class="ph ph-folder-open"></i>
                        {"Browse"}
                    </button>
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{i18n.t("local_podcast.author_placeholder")}</div></div>
                <div class="settings-row-control">
                    <input
                        id="local_podcast_author"
                        oninput={update_author}
                        value={(*author).clone()}
                        class="input"
                        placeholder={i18n.t("local_podcast.author_placeholder")}
                    />
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{i18n.t("local_podcast.description_placeholder")}</div></div>
                <div class="settings-row-control">
                    <textarea
                        id="local_podcast_description"
                        oninput={update_description}
                        value={(*description).clone()}
                        class="input"
                        placeholder={i18n.t("local_podcast.description_placeholder")}
                        rows="3"
                    />
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{i18n.t("local_podcast.explicit_label")}</div></div>
                <div class="settings-row-control">
                    <label class="toggle">
                        <input
                            id="local_podcast_explicit"
                            type="checkbox"
                            onclick={toggle_explicit}
                            checked={*explicit}
                        />
                        <span class="toggle-track"><span class="toggle-thumb"></span></span>
                    </label>
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{i18n.t("local_podcast.artwork_label")}</div></div>
                <div class="settings-row-control">
                    <ImagePicker
                        key={format!("artwork-{}", *form_version)}
                        id={format!("local_podcast_artwork_{}", *form_version)}
                        on_change={on_artwork_change}
                        accept="image/jpeg,image/png,image/gif,image/webp"
                        supported_types="Supported formats: JPG, PNG, GIF, WebP"
                        default_preview={(*detected_cover_url).clone()}
                        default_preview_label="Auto-detected from folder"
                    />
                </div>
            </div>
            <div class="settings-row">
                <div></div>
                <div class="settings-row-control">
                    <button
                        onclick={add_local_podcast_cb}
                        class="btn btn-secondary"
                        style="padding:6px 12px;"
                        disabled={*is_loading}
                    >
                        <i class="ph ph-plus"></i>
                        {i18n.t("local_podcast.add_button")}
                    </button>
                </div>
            </div>

            if added_podcast_id.is_some() {
                <>
                    <div class="settings-subsection-title">{i18n.t("local_podcast.refresh")}</div>
                    <div class="settings-row">
                        <div>
                            <div class="settings-row-label">{i18n.t("local_podcast.refresh_button")}</div>
                            <div class="settings-row-desc">{i18n.t("local_podcast.refresh_description")}</div>
                        </div>
                        <div class="settings-row-control">
                            <button
                                onclick={refresh_podcast_cb}
                                class="btn btn-secondary"
                                style="padding:6px 12px;"
                                disabled={*is_refreshing}
                            >
                                <i class="ph ph-arrow-clockwise"></i>
                                {i18n.t("local_podcast.refresh_button")}
                            </button>
                        </div>
                    </div>
                </>
            }

            if *show_dir_browser {
                <div
                    class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25"
                    onclick={close_dir_browser.clone()}
                >
                    <div
                        class="modal-container relative p-4 w-full max-w-lg max-h-full rounded-lg shadow"
                        onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
                    >
                        <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
                            <h3 style="font-weight:600; margin:0;">{"Select a folder"}</h3>
                            <button type="button" class="btn btn-ghost" onclick={close_dir_browser.clone()} style="padding:4px 8px;">
                                <i class="ph ph-x"></i>
                            </button>
                        </div>

                        <div class="settings-row-desc" style="margin-bottom:8px;">
                            { format!("/opt/pinepods/local-media/{}", *browser_path) }
                        </div>

                        <div style="display:flex; gap:8px; margin-bottom:8px;">
                            <button
                                type="button"
                                class="btn btn-secondary"
                                style="padding:4px 10px;"
                                onclick={go_up_dir}
                                disabled={browser_path.is_empty()}
                            >
                                <i class="ph ph-arrow-up"></i>
                                {"Up"}
                            </button>
                        </div>

                        <div style="max-height:320px; overflow-y:auto; border:1px solid rgba(128,128,128,0.18); border-radius:8px;">
                            if *browser_loading {
                                <div style="padding:16px; text-align:center;">
                                    <i class="ph ph-spinner animate-spin" style="font-size:20px;"></i>
                                </div>
                            } else if browser_entries.is_empty() {
                                <div class="settings-row-desc" style="padding:16px; text-align:center;">
                                    {"No subfolders here."}
                                </div>
                            } else {
                                { for browser_entries.iter().map(|entry| {
                                    let nav = navigate_dir.clone();
                                    let path = entry.path.clone();
                                    let onclick = Callback::from(move |_: MouseEvent| nav.emit(path.clone()));
                                    html! {
                                        <div
                                            onclick={onclick}
                                            style="display:flex; justify-content:space-between; align-items:center; padding:10px 12px; cursor:pointer; border-bottom:1px solid rgba(128,128,128,0.12);"
                                        >
                                            <span style="display:flex; align-items:center; gap:8px; min-width:0;">
                                                <i class="ph ph-folder" style="color:var(--text-secondary-color);"></i>
                                                <span style="overflow:hidden; text-overflow:ellipsis; white-space:nowrap;">{&entry.name}</span>
                                            </span>
                                            <span class="settings-row-desc" style="white-space:nowrap; margin-left:12px;">
                                                { format!("{} audio file{}", entry.audio_count, if entry.audio_count == 1 { "" } else { "s" }) }
                                            </span>
                                        </div>
                                    }
                                }) }
                            }
                        </div>

                        <div style="display:flex; justify-content:flex-end; gap:8px; margin-top:14px;">
                            <button type="button" class="btn btn-ghost" onclick={close_dir_browser} style="padding:6px 12px;">
                                {"Cancel"}
                            </button>
                            <button type="button" class="btn btn-secondary" onclick={select_current_dir} style="padding:6px 12px;" disabled={browser_path.is_empty()}>
                                <i class="ph ph-check"></i>
                                {"Select this folder"}
                            </button>
                        </div>
                    </div>
                </div>
            }
        </>
    }
}
