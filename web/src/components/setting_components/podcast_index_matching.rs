use crate::components::context::{AppState, NotificationState};
use crate::components::gen_components::FallbackImage;
use crate::requests::search_pods::{call_get_podcast_info, UnifiedPodcast};
use crate::requests::setting_reqs::{
    call_get_ignored_podcasts, call_get_unmatched_podcasts, call_ignore_podcast_index_id,
    call_update_podcast_index_id, UnmatchedPodcast,
};
use gloo_events::EventListener;
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlElement;
use web_sys::{InputEvent, KeyboardEvent, MouseEvent};
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(PodcastIndexMatching)]
pub fn podcast_index_matching() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());

    // Capture i18n strings before they get moved
    let _i18n_podcast_index_matching = i18n
        .t("podcast_index_matching.podcast_index_matching")
        .to_string();
    let i18n_loading_podcasts = i18n.t("podcast_index_matching.loading_podcasts").to_string();
    let i18n_all_podcasts_matched = i18n.t("podcast_index_matching.all_podcasts_matched").to_string();
    let i18n_no_podcasts_need_matching = i18n.t("podcast_index_matching.no_podcasts_need_matching").to_string();
    let i18n_click_to_search = i18n.t("podcast_index_matching.click_to_search").to_string();
    let i18n_ignore = i18n.t("podcast_index_matching.ignore").to_string();
    let i18n_manual_search_options = i18n.t("podcast_index_matching.manual_search_options").to_string();
    let i18n_search_by_custom_terms = i18n.t("podcast_index_matching.search_by_custom_terms").to_string();
    let i18n_search = i18n.t("podcast_index_matching.search").to_string();
    let i18n_enter_podcast_id = i18n.t("podcast_index_matching.enter_podcast_id").to_string();
    let i18n_match = i18n.t("podcast_index_matching.match_btn").to_string();
    let i18n_searching = i18n.t("podcast_index_matching.searching").to_string();
    let i18n_no_matches_found = i18n.t("podcast_index_matching.no_matches_found").to_string();
    let i18n_try_manual_search = i18n.t("podcast_index_matching.try_manual_search").to_string();
    let i18n_search_results = i18n.t("podcast_index_matching.search_results").to_string();
    let i18n_ignored_podcasts = i18n.t("podcast_index_matching.ignored_podcasts").to_string();
    let i18n_showing_ignored = i18n.t("podcast_index_matching.showing_ignored").to_string();
    let i18n_podcasts_ignored = i18n.t("podcast_index_matching.podcasts_ignored").to_string();
    let i18n_hide = i18n.t("podcast_index_matching.hide").to_string();
    let i18n_show = i18n.t("podcast_index_matching.show").to_string();
    let i18n_no_ignored = i18n.t("podcast_index_matching.no_ignored").to_string();
    let i18n_restore = i18n.t("podcast_index_matching.restore").to_string();

    let unmatched_podcasts: UseStateHandle<Vec<UnmatchedPodcast>> = use_state(|| Vec::new());
    let ignored_podcasts: UseStateHandle<Vec<UnmatchedPodcast>> = use_state(|| Vec::new());
    let search_results: UseStateHandle<Vec<UnifiedPodcast>> = use_state(|| Vec::new());
    let selected_podcast_id: UseStateHandle<Option<i32>> = use_state(|| None);
    let is_searching = use_state(|| false);
    let loading = use_state(|| false);
    let show_ignored = use_state(|| false);
    let dropdown_ref = use_node_ref();
    let manual_search_term = use_state(String::new);
    let manual_podcast_id = use_state(String::new);

    let dispatch_effect = _dispatch.clone();

    // Fetch unmatched podcasts on component mount
    {
        let unmatched_podcasts = unmatched_podcasts.clone();
        let ignored_podcasts = ignored_podcasts.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let loading = loading.clone();

        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let unmatched_podcasts = unmatched_podcasts.clone();
                let ignored_podcasts = ignored_podcasts.clone();
                let loading = loading.clone();
                let api_key_cloned = api_key.clone().unwrap_or(None);
                let server_name_cloned = server_name.clone();

                spawn_local(async move {
                    if let (Some(api_key), Some(server_name), Some(user_id)) =
                        (api_key_cloned, server_name_cloned, user_id)
                    {
                        loading.set(true);

                        // Fetch unmatched podcasts
                        match call_get_unmatched_podcasts(
                            server_name.clone(),
                            api_key.clone(),
                            user_id,
                        )
                        .await
                        {
                            Ok(response) => {
                                unmatched_podcasts.set(response.podcasts);
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error fetching unmatched podcasts: {}", e).into(),
                                );
                            }
                        }

                        // Fetch ignored podcasts
                        match call_get_ignored_podcasts(server_name, api_key, user_id).await {
                            Ok(response) => {
                                ignored_podcasts.set(response.podcasts);
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error fetching ignored podcasts: {}", e).into(),
                                );
                            }
                        }

                        loading.set(false);
                    }
                });
            },
        );
    }

    // Handle clicking outside dropdown to close it
    {
        let selected_podcast_id = selected_podcast_id.clone();
        let dropdown_ref = dropdown_ref.clone();

        use_effect_with(dropdown_ref.clone(), move |dropdown_ref| {
            let document = web_sys::window().unwrap().document().unwrap();
            let dropdown_element = dropdown_ref.cast::<HtmlElement>();

            let listener = EventListener::new(&document, "click", move |event| {
                if let Some(target) = event.target() {
                    if let Some(dropdown) = &dropdown_element {
                        if let Ok(node) = target.dyn_into::<web_sys::Node>() {
                            if !dropdown.contains(Some(&node)) {
                                selected_podcast_id.set(None);
                            }
                        }
                    }
                }
            });

            || drop(listener)
        });
    }

    let search_podcast_index = {
        let search_results = search_results.clone();
        let is_searching = is_searching.clone();
        let server_name = state
            .auth_details
            .as_ref()
            .map(|ad| ad.server_name.clone())
            .unwrap_or_default();
        let api_key = state
            .auth_details
            .as_ref()
            .and_then(|ad| ad.api_key.clone())
            .unwrap_or_default();
        let search_index = "podcast_index".to_string();

        Callback::from(move |podcast_name: String| {
            let search_results = search_results.clone();
            let is_searching = is_searching.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let search_index = search_index.clone();

            spawn_local(async move {
                {
                    is_searching.set(true);

                    match call_get_podcast_info(&podcast_name, &server_name, &api_key, &search_index).await
                    {
                        Ok(podcast_results) => {
                            let mut podcasts = Vec::new();

                            // Handle Podcast Index results
                            if let Some(feeds) = podcast_results.feeds {
                                for feed in feeds {
                                    let podcast = UnifiedPodcast::from(feed);
                                    podcasts.push(podcast);
                                }
                            }

                            // Handle iTunes results if using iTunes
                            if let Some(results) = podcast_results.results {
                                for result in results {
                                    let podcast = UnifiedPodcast::from(result);
                                    podcasts.push(podcast);
                                }
                            }

                            search_results.set(podcasts);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error searching Podcast Index: {}", e).into(),
                            );
                        }
                    }

                    is_searching.set(false);
                }
            });
        })
    };

    let handle_podcast_click = |podcast_id: i32| {
        let selected_podcast_id = selected_podcast_id.clone();
        let search_results = search_results.clone();
        let search_podcast_index = search_podcast_index.clone();
        let unmatched_podcasts = unmatched_podcasts.clone();
        let manual_search_term = manual_search_term.clone();
        let manual_podcast_id = manual_podcast_id.clone();

        Callback::from(move |_: MouseEvent| {
            // Clear previous search results and manual input fields
            search_results.set(Vec::new());
            manual_search_term.set(String::new());
            manual_podcast_id.set(String::new());

            // Set selected podcast and trigger search
            selected_podcast_id.set(Some(podcast_id));

            // Find the podcast name and search
            if let Some(podcast) = (**unmatched_podcasts)
                .iter()
                .find(|p| p.podcast_id == podcast_id)
            {
                search_podcast_index.emit(podcast.podcast_name.clone());
            }
        })
    };

    let handle_match_selection = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let unmatched_podcasts = unmatched_podcasts.clone();
        let selected_podcast_id = selected_podcast_id.clone();
        let search_results = search_results.clone();
        let dispatch_effect = dispatch_effect.clone();
        let manual_search_term = manual_search_term.clone();
        let manual_podcast_id = manual_podcast_id.clone();

        Callback::from(move |(podcast_id, index_id): (i32, i32)| {
            let server_name = server_name.clone();
            let api_key = api_key.clone().unwrap();
            let user_id = user_id.clone();
            let unmatched_podcasts = unmatched_podcasts.clone();
            let selected_podcast_id = selected_podcast_id.clone();
            let search_results = search_results.clone();
            let _dispatch_effect = dispatch_effect.clone();
            let manual_search_term = manual_search_term.clone();
            let manual_podcast_id = manual_podcast_id.clone();

            spawn_local(async move {
                if let (Some(server_name), Some(api_key), Some(user_id)) =
                    (server_name, api_key, user_id)
                {
                    match call_update_podcast_index_id(
                        server_name,
                        api_key,
                        user_id,
                        podcast_id,
                        index_id,
                    )
                    .await
                    {
                        Ok(_) => {
                            // Remove the matched podcast from the list
                            let updated_podcasts: Vec<UnmatchedPodcast> = (**unmatched_podcasts)
                                .iter()
                                .filter(|p| p.podcast_id != podcast_id)
                                .cloned()
                                .collect();
                            unmatched_podcasts.set(updated_podcasts);

                            // Clear selection, search results, and manual input fields
                            selected_podcast_id.set(None);
                            search_results.set(Vec::new());
                            manual_search_term.set(String::new());
                            manual_podcast_id.set(String::new());

                            // Show success message
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.info_message = Some(
                                    "Podcast successfully matched to Podcast Index!".to_string(),
                                );
                            });
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error updating podcast index ID: {}", e));
                            });
                        }
                    }
                }
            });
        })
    };

    let handle_ignore_podcast = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let unmatched_podcasts = unmatched_podcasts.clone();
        let ignored_podcasts = ignored_podcasts.clone();
        let dispatch_effect = dispatch_effect.clone();

        Callback::from(move |(podcast_id, ignore): (i32, bool)| {
            let server_name = server_name.clone();
            let api_key = api_key.clone().unwrap();
            let user_id = user_id.clone();
            let unmatched_podcasts = unmatched_podcasts.clone();
            let ignored_podcasts = ignored_podcasts.clone();
            let _dispatch_effect = dispatch_effect.clone();

            spawn_local(async move {
                if let (Some(server_name), Some(api_key), Some(user_id)) =
                    (server_name, api_key, user_id)
                {
                    match call_ignore_podcast_index_id(
                        server_name.clone(),
                        api_key.clone(),
                        user_id,
                        podcast_id,
                        ignore,
                    )
                    .await
                    {
                        Ok(_) => {
                            if ignore {
                                // Move podcast from unmatched to ignored
                                if let Some(podcast) = (**unmatched_podcasts)
                                    .iter()
                                    .find(|p| p.podcast_id == podcast_id)
                                    .cloned()
                                {
                                    let updated_unmatched: Vec<UnmatchedPodcast> =
                                        (**unmatched_podcasts)
                                            .iter()
                                            .filter(|p| p.podcast_id != podcast_id)
                                            .cloned()
                                            .collect();
                                    unmatched_podcasts.set(updated_unmatched);

                                    let mut updated_ignored = (**ignored_podcasts).to_vec();
                                    updated_ignored.push(podcast);
                                    ignored_podcasts.set(updated_ignored);
                                }

                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.info_message =
                                        Some("Podcast ignored from index matching".to_string());
                                });
                            } else {
                                // Move podcast from ignored to unmatched
                                if let Some(podcast) = (**ignored_podcasts)
                                    .iter()
                                    .find(|p| p.podcast_id == podcast_id)
                                    .cloned()
                                {
                                    let updated_ignored: Vec<UnmatchedPodcast> =
                                        (**ignored_podcasts)
                                            .iter()
                                            .filter(|p| p.podcast_id != podcast_id)
                                            .cloned()
                                            .collect();
                                    ignored_podcasts.set(updated_ignored);

                                    let mut updated_unmatched = (**unmatched_podcasts).to_vec();
                                    updated_unmatched.push(podcast);
                                    unmatched_podcasts.set(updated_unmatched);
                                }

                                Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                    state.info_message =
                                        Some("Podcast restored to index matching".to_string());
                                });
                            }
                        }
                        Err(e) => {
                            Dispatch::<NotificationState>::global().reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error updating podcast ignore status: {}", e));
                            });
                        }
                    }
                }
            });
        })
    };

    let toggle_ignored_view = {
        let show_ignored = show_ignored.clone();
        Callback::from(move |_: MouseEvent| {
            show_ignored.set(!*show_ignored);
        })
    };

    let handle_manual_search = {
        let manual_search_term = manual_search_term.clone();
        let search_podcast_index = search_podcast_index.clone();
        let search_results = search_results.clone();

        Callback::from(move |_: MouseEvent| {
            let search_term = (*manual_search_term).trim();
            if !search_term.is_empty() {
                search_results.set(Vec::new());
                search_podcast_index.emit(search_term.to_string());
            }
        })
    };

    let handle_manual_id_select = {
        let manual_podcast_id = manual_podcast_id.clone();
        let selected_podcast_id = selected_podcast_id.clone();
        let handle_match_selection = handle_match_selection.clone();

        Callback::from(move |_: MouseEvent| {
            let id_str = (*manual_podcast_id).trim();
            if let (Ok(index_id), Some(podcast_id)) = (id_str.parse::<i32>(), *selected_podcast_id)
            {
                handle_match_selection.emit((podcast_id, index_id));
            }
        })
    };

    let on_manual_search_input = {
        let manual_search_term = manual_search_term.clone();
        Callback::from(move |e: InputEvent| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            manual_search_term.set(input.value());
        })
    };

    let on_manual_search_keydown = {
        let handle_manual_search = handle_manual_search.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                handle_manual_search.emit(MouseEvent::new("click").unwrap());
            }
        })
    };

    let on_manual_id_input = {
        let manual_podcast_id = manual_podcast_id.clone();
        Callback::from(move |e: InputEvent| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            manual_podcast_id.set(input.value());
        })
    };

    let on_manual_id_keydown = {
        let handle_manual_id_select = handle_manual_id_select.clone();
        let manual_podcast_id = manual_podcast_id.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                let id_str = (*manual_podcast_id).trim();
                if !id_str.is_empty() && id_str.parse::<i32>().is_ok() {
                    handle_manual_id_select.emit(MouseEvent::new("click").unwrap());
                }
            }
        })
    };

    html! {
        <div class="settings_container" ref={dropdown_ref}>
            if *loading {
                <div class="settings-row">
                    <div><div class="settings-row-label">{ &i18n_loading_podcasts }</div></div>
                    <div class="settings-row-control">
                        <i class="ph ph-spinner animate-spin" style="font-size:18px;color:var(--text-color);"></i>
                    </div>
                </div>
            } else if unmatched_podcasts.is_empty() {
                <div class="settings-row">
                    <div>
                        <div class="settings-row-label">{ &i18n_all_podcasts_matched }</div>
                        <div class="settings-row-desc">{ &i18n_no_podcasts_need_matching }</div>
                    </div>
                </div>
            } else {
                <div style="display:flex;flex-direction:column;gap:12px;">
                    {
                        unmatched_podcasts.iter().map(|podcast| {
                            let podcast_id = podcast.podcast_id;
                            let is_selected = *selected_podcast_id == Some(podcast_id);
                            let click_handler = handle_podcast_click(podcast_id);

                            html! {
                                <div key={podcast.podcast_id} class="border rounded-lg p-4 modal-container">
                                    <div
                                        style="display:flex;align-items:flex-start;gap:12px;cursor:pointer;padding:8px;border-radius:6px;"
                                        onclick={click_handler}
                                    >
                                        <FallbackImage
                                            src={podcast.artwork_url.clone().unwrap_or_else(|| "/static/assets/favicon.png".to_string())}
                                            alt={format!("Cover for {}", podcast.podcast_name)}
                                            class="w-16 h-16 rounded object-cover flex-shrink-0"
                                        />
                                        <div style="flex:1;min-width:0;">
                                            <div style="font-size:14px;font-weight:600;color:var(--text-color);margin-bottom:4px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">
                                                {&podcast.podcast_name}
                                            </div>
                                            {
                                                if let Some(author) = &podcast.author {
                                                    html! { <div style="font-size:12px;color:var(--text-secondary-color);margin-bottom:4px;">{author}</div> }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            <div style="font-size:11px;color:var(--text-secondary-color);">
                                                { &i18n_click_to_search }
                                            </div>
                                        </div>
                                        <div style="display:flex;align-items:center;gap:8px;flex-shrink:0;">
                                            <button
                                                class="btn btn-danger"
                                                style="padding:4px 10px;font-size:12px;"
                                                onclick={{
                                                    let handle_ignore_podcast = handle_ignore_podcast.clone();
                                                    let podcast_id = podcast_id;
                                                    Callback::from(move |e: MouseEvent| {
                                                        e.stop_propagation();
                                                        handle_ignore_podcast.emit((podcast_id, true));
                                                    })
                                                }}
                                            >
                                                { &i18n_ignore }
                                            </button>
                                            <i class="ph ph-magnifying-glass" style="font-size:22px;color:var(--text-secondary-color);"></i>
                                        </div>
                                    </div>

                                    if is_selected {
                                        <div style="margin-top:12px;border-radius:8px;overflow:hidden;" class="modal-container border">
                                            <div style="padding:16px;border-bottom:1px solid rgba(128,128,128,0.15);">
                                                <div style="font-size:13px;font-weight:500;color:var(--text-color);margin-bottom:12px;">{ &i18n_manual_search_options }</div>

                                                <div style="margin-bottom:10px;">
                                                    <div style="font-size:11px;color:var(--text-secondary-color);margin-bottom:6px;">{ &i18n_search_by_custom_terms }</div>
                                                    <div style="display:flex;gap:8px;flex-wrap:wrap;">
                                                        <input
                                                            type="text"
                                                            placeholder="Enter search terms (e.g., 'Skeptoid')"
                                                            value={(*manual_search_term).clone()}
                                                            oninput={on_manual_search_input.clone()}
                                                            onkeydown={on_manual_search_keydown.clone()}
                                                            class="input"
                                                            style="flex:1;min-width:160px;"
                                                        />
                                                        <button
                                                            onclick={handle_manual_search.clone()}
                                                            class="btn btn-secondary"
                                                            style="padding:6px 12px;"
                                                        >
                                                            <i class="ph ph-magnifying-glass"></i>
                                                            { &i18n_search }
                                                        </button>
                                                    </div>
                                                </div>

                                                <div>
                                                    <div style="font-size:11px;color:var(--text-secondary-color);margin-bottom:6px;">{ &i18n_enter_podcast_id }</div>
                                                    <div style="display:flex;gap:8px;flex-wrap:wrap;">
                                                        <input
                                                            type="text"
                                                            placeholder="Enter Podcast Index ID (e.g., 920666)"
                                                            value={(*manual_podcast_id).clone()}
                                                            oninput={on_manual_id_input.clone()}
                                                            onkeydown={on_manual_id_keydown.clone()}
                                                            class="input"
                                                            style="flex:1;min-width:160px;"
                                                        />
                                                        <button
                                                            onclick={handle_manual_id_select.clone()}
                                                            disabled={manual_podcast_id.trim().is_empty() || manual_podcast_id.parse::<i32>().is_err()}
                                                            class="btn btn-primary"
                                                            style="padding:6px 12px;"
                                                        >
                                                            <i class="ph ph-check"></i>
                                                            { &i18n_match }
                                                        </button>
                                                    </div>
                                                </div>
                                            </div>

                                            if *is_searching {
                                                <div style="display:flex;align-items:center;gap:8px;padding:16px;color:var(--text-color);">
                                                    <i class="ph ph-spinner animate-spin" style="font-size:18px;"></i>
                                                    <span style="font-size:13px;">{ &i18n_searching }</span>
                                                </div>
                                            } else if search_results.is_empty() {
                                                <div style="text-align:center;padding:16px;">
                                                    <div style="font-size:13px;color:var(--text-secondary-color);">{ &i18n_no_matches_found }</div>
                                                    <div style="font-size:11px;color:var(--text-secondary-color);margin-top:4px;">{ &i18n_try_manual_search }</div>
                                                </div>
                                            } else {
                                                <div>
                                                    <div style="padding:10px 12px;border-bottom:1px solid rgba(128,128,128,0.15);">
                                                        <span style="font-size:13px;font-weight:500;color:var(--text-color);">{ &i18n_search_results }</span>
                                                    </div>
                                                    <div style="max-height:300px;overflow-y:auto;padding:8px;">
                                                        {
                                                            search_results.iter().map(|result| {
                                                                let podcast_id = podcast_id;
                                                                let index_id = result.index_id as i32;
                                                                let match_handler = {
                                                                    let handle_match_selection = handle_match_selection.clone();
                                                                    Callback::from(move |_: MouseEvent| {
                                                                        handle_match_selection.emit((podcast_id, index_id));
                                                                    })
                                                                };

                                                                html! {
                                                                    <div
                                                                        key={result.id}
                                                                        onclick={match_handler}
                                                                        style="display:flex;align-items:center;padding:8px;border-radius:6px;cursor:pointer;gap:12px;"
                                                                    >
                                                                        <FallbackImage
                                                                            src={result.image.clone()}
                                                                            alt={format!("Cover for {}", result.title)}
                                                                            class="w-12 h-12 rounded object-cover"
                                                                        />
                                                                        <div style="flex:1;min-width:0;">
                                                                            <div style="font-size:13px;font-weight:500;color:var(--text-color);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">
                                                                                {&result.title}
                                                                            </div>
                                                                            <div style="font-size:11px;color:var(--text-secondary-color);">{&result.author}</div>
                                                                            <div style="font-size:11px;color:var(--text-secondary-color);">
                                                                                {format!("Index ID: {}", result.index_id)}
                                                                            </div>
                                                                        </div>
                                                                        <i class="ph ph-check" style="font-size:20px;color:var(--accent-color);flex-shrink:0;"></i>
                                                                    </div>
                                                                }
                                                            }).collect::<Html>()
                                                        }
                                                    </div>
                                                </div>
                                            }
                                        </div>
                                    }
                                </div>
                            }
                        }).collect::<Html>()
                    }
                </div>
            }

            <div class="settings-subsection-title" style="margin-top:20px;">{ &i18n_ignored_podcasts }</div>
            <div class="settings-row">
                <div><div class="settings-row-label">{ if *show_ignored { &i18n_showing_ignored } else { &i18n_podcasts_ignored } }</div></div>
                <div class="settings-row-control">
                    <button
                        class="btn btn-ghost"
                        style="padding:6px 12px;"
                        onclick={toggle_ignored_view}
                    >
                        <i class={if *show_ignored { "ph ph-chevron-up" } else { "ph ph-chevron-down" }}></i>
                        <span>{ if *show_ignored { &i18n_hide } else { &i18n_show } }</span>
                    </button>
                </div>
            </div>

            if *show_ignored {
                if ignored_podcasts.is_empty() {
                    <div class="settings-row">
                        <div><div class="settings-row-desc">{ &i18n_no_ignored }</div></div>
                    </div>
                } else {
                    <div style="padding:0 8px;">
                        {
                            ignored_podcasts.iter().map(|podcast| {
                                let podcast_id = podcast.podcast_id;

                                html! {
                                    <div key={podcast.podcast_id} style="display:flex;align-items:center;gap:12px;padding:8px 0;border-bottom:1px solid rgba(128,128,128,0.12);">
                                        <FallbackImage
                                            src={podcast.artwork_url.clone().unwrap_or_else(|| "/static/assets/favicon.png".to_string())}
                                            alt={format!("Cover for {}", podcast.podcast_name)}
                                            class="w-12 h-12 rounded object-cover flex-shrink-0"
                                        />
                                        <div style="flex:1;min-width:0;">
                                            <div style="font-size:13px;font-weight:500;color:var(--text-color);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;opacity:0.75;">
                                                {&podcast.podcast_name}
                                            </div>
                                            {
                                                if let Some(author) = &podcast.author {
                                                    html! { <div style="font-size:11px;color:var(--text-secondary-color);opacity:0.75;">{author}</div> }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>
                                        <button
                                            class="btn btn-secondary"
                                            style="padding:4px 10px;font-size:12px;flex-shrink:0;"
                                            onclick={{
                                                let handle_ignore_podcast = handle_ignore_podcast.clone();
                                                let podcast_id = podcast_id;
                                                Callback::from(move |_: MouseEvent| {
                                                    handle_ignore_podcast.emit((podcast_id, false));
                                                })
                                            }}
                                        >
                                            <i class="ph ph-arrow-counter-clockwise"></i>
                                            { &i18n_restore }
                                        </button>
                                    </div>
                                }
                            }).collect::<Html>()
                        }
                    </div>
                }
            }
        </div>
    }
}
