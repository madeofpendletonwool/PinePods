use crate::components::context::{AppState, NotificationState};
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_add_local_podcast, call_add_local_podcast_artwork, call_refresh_local_podcast,
};
use web_sys::{FormData, HtmlInputElement};
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
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            description.set(input.value());
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
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            if let Some(files) = input.files() {
                if files.length() > 0 {
                    artwork_file.set(files.get(0));
                }
            }
        })
    };

    let add_local_podcast_cb = {
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let user_id = user_id.clone();
        let podcast_name = (*podcast_name).clone();
        let directory_path = (*directory_path).clone();
        let description = (*description).clone();
        let author = (*author).clone();
        let explicit_val = *explicit;
        let artwork_file = artwork_file.clone();
        let dispatch = dispatch.clone();
        let is_loading = is_loading.clone();
        let added_podcast_id = added_podcast_id.clone();
        let success_msg = i18n.t("local_podcast.podcast_added").to_string();
        let error_prefix = i18n.t("local_podcast.failed_to_add").to_string();

        Callback::from(move |_| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let user_id_val = user_id.clone().unwrap_or_default();
            let podcast_name = podcast_name.clone();
            let directory_path = directory_path.clone();
            let description_opt = if description.is_empty() { None } else { Some(description.clone()) };
            let author_opt = if author.is_empty() { None } else { Some(author.clone()) };
            let artwork_file = artwork_file.clone();
            let dispatch = dispatch.clone();
            let is_loading = is_loading.clone();
            let added_podcast_id = added_podcast_id.clone();
            let success_msg = success_msg.clone();
            let error_prefix = error_prefix.clone();

            if podcast_name.is_empty() || directory_path.is_empty() {
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
                    &directory_path,
                    &podcast_name,
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
                <div class="settings-row-control">
                    <input
                        id="local_podcast_dir"
                        oninput={update_directory_path}
                        class="input"
                        placeholder={i18n.t("local_podcast.directory_path_placeholder")}
                    />
                </div>
            </div>
            <div class="settings-row">
                <div><div class="settings-row-label">{i18n.t("local_podcast.author_placeholder")}</div></div>
                <div class="settings-row-control">
                    <input
                        id="local_podcast_author"
                        oninput={update_author}
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
                    <input
                        id="local_podcast_artwork"
                        type="file"
                        accept="image/*"
                        onchange={on_artwork_change}
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
        </>
    }
}
