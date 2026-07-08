use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, EpisodeStatusState, FilterState, NotificationState, PodcastFeedState};
use crate::components::context_menu_button::PageType;
use crate::components::episode_list_view::EpisodeListView;
use crate::components::gen_components::{
    empty_message, Search_nav, UseScrollToTop,
};
use crate::components::gen_funcs::{
    get_default_sort_direction, get_filter_preference, set_filter_preference,
};
use crate::components::loading::Loading;
use crate::pages::playlists::IconSelector;
use crate::requests::episode::Episode;
use crate::requests::pod_req;
use crate::requests::pod_req::{
    Collection, CreateCollectionRequest, UpdateCollectionRequest,
};
use gloo_timers::future::TimeoutFuture;
use i18nrs::yew::use_translation;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

#[function_component(Saved)]
pub fn saved() -> Html {
    let (i18n, _) = use_translation();
    let (filter_state, _filter_dispatch) = use_store::<FilterState>();
    let favorite_podcast_ids = use_selector(|state: &PodcastFeedState| {
        state.podcast_feed_return_extra
            .as_ref()
            .and_then(|pr| pr.pods.as_ref())
            .map(|pods| {
                pods.iter()
                    .filter(|p| p.is_favorite)
                    .map(|p| p.podcastid)
                    .collect::<std::collections::HashSet<i32>>()
            })
            .unwrap_or_default()
    });

    let api_key_sel = use_selector(|s: &AppState| {
        s.auth_details.as_ref().map(|ud| ud.api_key.clone())
    });
    let user_id_sel = use_selector(|s: &AppState| {
        s.user_details.as_ref().map(|ud| ud.UserID.clone())
    });
    let server_name_sel = use_selector(|s: &AppState| {
        s.auth_details.as_ref().map(|ud| ud.server_name.clone())
    });
    let api_key = (*api_key_sel).clone();
    let user_id = (*user_id_sel).clone();
    let server_name = (*server_name_sel).clone();

    // Collections (Saved is the pinned, undeletable default — always first)
    let collections = use_state(|| Vec::<Collection>::new());
    let active_collection = use_state(|| None as Option<Collection>);
    let collections_loading = use_state(|| true);

    let episodes = use_state(|| Rc::new(Vec::<Episode>::new()));
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading = use_state(|| true);
    let loading_more = use_state(|| false);

    let episode_search_term = use_state(|| String::new());

    // Sort/filter — persisted in localStorage (shared across collection tabs)
    let sort_pref = get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string());
    let sort_value = use_state(|| sort_pref.clone());
    let filter_value = use_state(|| {
        get_filter_preference("saved_filter").unwrap_or_else(|| "all".to_string())
    });

    // Derive API sort_by / sort_order from the sort_value string
    fn sort_to_params(sort: &str) -> (&'static str, &'static str) {
        match sort {
            "oldest"   => ("date", "asc"),
            "shortest" => ("duration", "asc"),
            "longest"  => ("duration", "desc"),
            "title_az" => ("title", "asc"),
            "title_za" => ("title", "desc"),
            _          => ("date", "desc"), // "newest" or default
        }
    }

    // Trigger for reloading episodes when sort or filter changes
    let reload_trigger = use_state(|| 0u32);

    // Collection create/edit modal state
    let show_new_modal = use_state(|| false);
    let show_edit_modal = use_state(|| false);
    let form_name = use_state(|| String::new());
    let form_desc = use_state(|| String::new());
    let form_icon = use_state(|| "ph-bookmark-simple".to_string());
    let form_categories = use_state(|| Vec::<String>::new());
    let form_backfill = use_state(|| false);
    let form_saving = use_state(|| false);
    // Distinct categories across the user's podcasts, for the auto-add picker
    let available_categories = use_state(|| Vec::<String>::new());

    // Fetch collections on mount
    {
        let collections = collections.clone();
        let active_collection = active_collection.clone();
        let collections_loading = collections_loading.clone();
        let available_categories = available_categories.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone()),
            move |_| {
                if let (Some(Some(api_key)), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let collections = collections.clone();
                    let active_collection = active_collection.clone();
                    let collections_loading = collections_loading.clone();
                    let available_categories = available_categories.clone();
                    spawn_local(async move {
                        match pod_req::call_get_collections(&server_name, &api_key, user_id).await {
                            Ok(cols) => {
                                let default = cols.iter().find(|c| c.is_default).cloned()
                                    .or_else(|| cols.first().cloned());
                                active_collection.set(default);
                                collections.set(cols);
                                collections_loading.set(false);
                            }
                            Err(_) => {
                                collections_loading.set(false);
                            }
                        }
                        // Load the category list for the auto-add picker (best-effort)
                        if let Ok(cats) = pod_req::call_get_user_categories(&server_name, &api_key, user_id).await {
                            available_categories.set(cats);
                        }
                    });
                }
                || ()
            },
        );
    }

    let active_collection_id = active_collection.as_ref().map(|c| c.collection_id);
    let active_is_default = active_collection.as_ref().map(|c| c.is_default).unwrap_or(true);

    // Fetch episodes for the active collection (and reload on sort/filter/tab change)
    {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading = loading.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let sort_value = sort_value.clone();
        let filter_value = filter_value.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone(), active_collection_id, active_is_default, *reload_trigger),
            move |(api_key, user_id, server_name, active_collection_id, active_is_default, _)| {
                if let (Some(Some(api_key)), Some(user_id), Some(server_name), Some(collection_id)) =
                    (api_key.clone(), user_id.clone(), server_name.clone(), *active_collection_id)
                {
                    let episodes = episodes.clone();
                    let total = total.clone();
                    let offset = offset.clone();
                    let loading = loading.clone();
                    let sort_str = (*sort_value).clone();
                    let filter_str = (*filter_value).clone();
                    let is_default = *active_is_default;
                    let api_key_opt = Some(api_key.clone());

                    episodes.set(Rc::new(Vec::new()));
                    offset.set(0);
                    total.set(0);
                    loading.set(true);

                    spawn_local(async move {
                        let (sort_by, sort_order) = sort_to_params(&sort_str);
                        let result = if is_default {
                            pod_req::call_get_saved_episodes_paged(
                                &server_name, &api_key_opt, &user_id, PAGE_SIZE, 0,
                                sort_by, sort_order, &filter_str,
                            ).await
                        } else {
                            pod_req::call_get_collection_episodes_paged(
                                &server_name, &api_key_opt, collection_id, PAGE_SIZE, 0,
                                sort_by, sort_order, &filter_str,
                            ).await
                        };
                        match result {
                            Ok(page) => {
                                let completed_ids: std::collections::HashSet<i32> = page
                                    .saved_episodes
                                    .iter()
                                    .filter(|ep| ep.completed)
                                    .map(|ep| ep.episodeid)
                                    .collect();
                                // Only episodes actually in the Saved bucket should drive
                                // the saved badge — a collection episode may not be saved.
                                let saved_eps: Vec<Episode> = page
                                    .saved_episodes
                                    .iter()
                                    .filter(|ep| ep.saved)
                                    .cloned()
                                    .collect();
                                Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                    s.saved_episodes = saved_eps;
                                    s.completed_episodes = completed_ids;
                                });

                                #[cfg(not(feature = "server_build"))]
                                {
                                    spawn_local(async move {
                                        if let Ok(mut local_episodes) =
                                            crate::pages::downloads_tauri::fetch_local_episodes().await
                                        {
                                            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                                s.downloaded_episodes.clear_local();
                                                for ep in local_episodes.drain(..) {
                                                    s.downloaded_episodes.push_local(ep);
                                                }
                                            });
                                        }
                                    });
                                }

                                let new_offset = page.saved_episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                episodes.set(Rc::new(page.saved_episodes));
                                loading.set(false);
                            }
                            Err(_) => {
                                loading.set(false);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    // Load-more handler (branches on whether the active collection is the default Saved one)
    let on_load_more = {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        let sort_value = sort_value.clone();
        let filter_value = filter_value.clone();
        use_callback((active_collection_id, active_is_default), move |_: (), (active_collection_id, active_is_default)| {
            if *loading_more {
                return;
            }
            let current_offset = *offset;
            let current_total = *total;
            if current_offset >= current_total {
                return;
            }
            let Some(Some(api_key)) = api_key.clone() else { return; };
            let Some(user_id) = user_id.clone() else { return; };
            let Some(server_name) = server_name.clone() else { return; };
            let Some(collection_id) = *active_collection_id else { return; };
            let is_default = *active_is_default;
            let sort_str = (*sort_value).clone();
            let filter_str = (*filter_value).clone();
            loading_more.set(true);
            let episodes = episodes.clone();
            let total = total.clone();
            let offset = offset.clone();
            let loading_more = loading_more.clone();
            spawn_local(async move {
                let (sort_by, sort_order) = sort_to_params(&sort_str);
                let api_key_opt = Some(api_key);
                let result = if is_default {
                    pod_req::call_get_saved_episodes_paged(
                        &server_name, &api_key_opt, &user_id, PAGE_SIZE, current_offset,
                        sort_by, sort_order, &filter_str,
                    ).await
                } else {
                    pod_req::call_get_collection_episodes_paged(
                        &server_name, &api_key_opt, collection_id, PAGE_SIZE, current_offset,
                        sort_by, sort_order, &filter_str,
                    ).await
                };
                if let Ok(page) = result {
                    TimeoutFuture::new(0).await;
                    let new_offset = current_offset + page.saved_episodes.len() as i64;
                    let mut all = (**episodes).clone();
                    all.extend(page.saved_episodes);
                    total.set(page.total);
                    offset.set(new_offset);
                    episodes.set(Rc::new(all));
                    TimeoutFuture::new(0).await;
                }
                loading_more.set(false);
            });
        })
    };

    // ---- Collection create / edit / delete handlers ----

    let open_new_modal = {
        let show_new_modal = show_new_modal.clone();
        let form_name = form_name.clone();
        let form_desc = form_desc.clone();
        let form_icon = form_icon.clone();
        let form_categories = form_categories.clone();
        let form_backfill = form_backfill.clone();
        Callback::from(move |_| {
            form_name.set(String::new());
            form_desc.set(String::new());
            form_icon.set("ph-bookmark-simple".to_string());
            form_categories.set(Vec::new());
            form_backfill.set(false);
            show_new_modal.set(true);
        })
    };

    let open_edit_modal = {
        let show_edit_modal = show_edit_modal.clone();
        let form_name = form_name.clone();
        let form_desc = form_desc.clone();
        let form_icon = form_icon.clone();
        let form_categories = form_categories.clone();
        let form_backfill = form_backfill.clone();
        let active_collection = active_collection.clone();
        Callback::from(move |_| {
            if let Some(c) = active_collection.as_ref() {
                form_name.set(c.name.clone());
                form_desc.set(c.description.clone().unwrap_or_default());
                form_icon.set(c.icon.clone());
                form_categories.set(c.auto_add_categories.clone().unwrap_or_default());
                form_backfill.set(false);
                show_edit_modal.set(true);
            }
        })
    };

    let on_create_submit = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let form_name = form_name.clone();
        let form_desc = form_desc.clone();
        let form_icon = form_icon.clone();
        let form_categories = form_categories.clone();
        let form_backfill = form_backfill.clone();
        let form_saving = form_saving.clone();
        let show_new_modal = show_new_modal.clone();
        let collections = collections.clone();
        let active_collection = active_collection.clone();
        Callback::from(move |_| {
            let name = (*form_name).trim().to_string();
            if name.is_empty() {
                Dispatch::<NotificationState>::global().reduce_mut(|s| {
                    s.error_message = Some("Collection name is required".to_string());
                });
                return;
            }
            let (Some(Some(api_key)), Some(user_id), Some(server_name)) =
                (api_key.clone(), user_id.clone(), server_name.clone()) else { return; };
            let desc = (*form_desc).trim().to_string();
            let categories = (*form_categories).clone();
            let req = CreateCollectionRequest {
                user_id,
                name,
                description: if desc.is_empty() { None } else { Some(desc) },
                icon: Some((*form_icon).clone()),
                auto_add_categories: Some(categories.clone()),
                backfill: Some(*form_backfill && !categories.is_empty()),
            };
            let form_saving = form_saving.clone();
            let show_new_modal = show_new_modal.clone();
            let collections = collections.clone();
            let active_collection = active_collection.clone();
            form_saving.set(true);
            spawn_local(async move {
                match pod_req::call_create_collection(&server_name, &api_key, req).await {
                    Ok(resp) => {
                        if let Ok(cols) = pod_req::call_get_collections(&server_name, &api_key, user_id).await {
                            let new_active = cols.iter().find(|c| c.collection_id == resp.collection_id).cloned();
                            collections.set(cols);
                            if new_active.is_some() {
                                active_collection.set(new_active);
                            }
                        }
                        show_new_modal.set(false);
                        Dispatch::<NotificationState>::global().reduce_mut(|s| {
                            s.info_message = Some("Collection created".to_string());
                        });
                    }
                    Err(e) => {
                        Dispatch::<NotificationState>::global().reduce_mut(|s| {
                            s.error_message = Some(format!("{}", e));
                        });
                    }
                }
                form_saving.set(false);
            });
        })
    };

    let on_edit_submit = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let form_name = form_name.clone();
        let form_desc = form_desc.clone();
        let form_icon = form_icon.clone();
        let form_categories = form_categories.clone();
        let form_backfill = form_backfill.clone();
        let form_saving = form_saving.clone();
        let show_edit_modal = show_edit_modal.clone();
        let collections = collections.clone();
        let active_collection = active_collection.clone();
        Callback::from(move |_| {
            let Some(current) = active_collection.as_ref().cloned() else { return; };
            let name = (*form_name).trim().to_string();
            if name.is_empty() {
                Dispatch::<NotificationState>::global().reduce_mut(|s| {
                    s.error_message = Some("Collection name is required".to_string());
                });
                return;
            }
            let (Some(Some(api_key)), Some(user_id), Some(server_name)) =
                (api_key.clone(), user_id.clone(), server_name.clone()) else { return; };
            let desc = (*form_desc).trim().to_string();
            let categories = (*form_categories).clone();
            let req = UpdateCollectionRequest {
                name: Some(name),
                description: Some(desc),
                icon: Some((*form_icon).clone()),
                auto_add_categories: Some(categories.clone()),
                backfill: Some(*form_backfill && !categories.is_empty()),
            };
            let form_saving = form_saving.clone();
            let show_edit_modal = show_edit_modal.clone();
            let collections = collections.clone();
            let active_collection = active_collection.clone();
            form_saving.set(true);
            spawn_local(async move {
                match pod_req::call_update_collection(&server_name, &api_key, current.collection_id, req).await {
                    Ok(_) => {
                        if let Ok(cols) = pod_req::call_get_collections(&server_name, &api_key, user_id).await {
                            let new_active = cols.iter().find(|c| c.collection_id == current.collection_id).cloned();
                            collections.set(cols);
                            if new_active.is_some() {
                                active_collection.set(new_active);
                            }
                        }
                        show_edit_modal.set(false);
                        Dispatch::<NotificationState>::global().reduce_mut(|s| {
                            s.info_message = Some("Collection updated".to_string());
                        });
                    }
                    Err(e) => {
                        Dispatch::<NotificationState>::global().reduce_mut(|s| {
                            s.error_message = Some(format!("{}", e));
                        });
                    }
                }
                form_saving.set(false);
            });
        })
    };

    let on_delete_collection = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let show_edit_modal = show_edit_modal.clone();
        let collections = collections.clone();
        let active_collection = active_collection.clone();
        Callback::from(move |_| {
            let Some(current) = active_collection.as_ref().cloned() else { return; };
            let (Some(Some(api_key)), Some(user_id), Some(server_name)) =
                (api_key.clone(), user_id.clone(), server_name.clone()) else { return; };
            let show_edit_modal = show_edit_modal.clone();
            let collections = collections.clone();
            let active_collection = active_collection.clone();
            spawn_local(async move {
                match pod_req::call_delete_collection(&server_name, &api_key, current.collection_id).await {
                    Ok(_) => {
                        if let Ok(cols) = pod_req::call_get_collections(&server_name, &api_key, user_id).await {
                            let default = cols.iter().find(|c| c.is_default).cloned()
                                .or_else(|| cols.first().cloned());
                            collections.set(cols);
                            active_collection.set(default);
                        }
                        show_edit_modal.set(false);
                        Dispatch::<NotificationState>::global().reduce_mut(|s| {
                            s.info_message = Some("Collection deleted".to_string());
                        });
                    }
                    Err(e) => {
                        Dispatch::<NotificationState>::global().reduce_mut(|s| {
                            s.error_message = Some(format!("{}", e));
                        });
                    }
                }
            });
        })
    };

    // Client-side filter (search + favorites)
    let search_term = (*episode_search_term).clone();
    let has_client_filter = !search_term.is_empty()
        || (filter_state.favorites_only && !favorite_podcast_ids.is_empty());
    let display_episodes_rc: Rc<Vec<Episode>> = if has_client_filter {
        let term = search_term.to_lowercase();
        Rc::new(
            (*episodes)
                .iter()
                .filter(|ep| {
                    if filter_state.favorites_only && !favorite_podcast_ids.contains(&ep.podcastid)
                    {
                        return false;
                    }
                    if term.is_empty() {
                        return true;
                    }
                    ep.episodetitle.to_lowercase().contains(&term)
                        || ep.episodedescription.to_lowercase().contains(&term)
                })
                .cloned()
                .collect(),
        )
    } else {
        (*episodes).clone()
    };
    let display_empty = display_episodes_rc.is_empty();
    let backend_can_load_more = *offset < *total;

    // ---- Tab bar ----
    let tab_bar = {
        let collections = collections.clone();
        let active_collection = active_collection.clone();
        let active_id = active_collection_id;
        let open_new_modal = open_new_modal.clone();
        html! {
            <div class="sp-chips pfb-chips collections-tabs">
                {
                    collections.iter().map(|c| {
                        let is_active = Some(c.collection_id) == active_id;
                        let onclick = {
                            let active_collection = active_collection.clone();
                            let c = c.clone();
                            Callback::from(move |_| active_collection.set(Some(c.clone())))
                        };
                        html! {
                            <button
                                class={classes!("sp-chip", if is_active { "is-active" } else { "" })}
                                {onclick}
                            >
                                <i class={classes!("ph", c.icon.clone())}></i>
                                <span>{ c.name.clone() }</span>
                            </button>
                        }
                    }).collect::<Html>()
                }
                <button class="sp-chip" onclick={open_new_modal} title={i18n.t("collections.new_collection")}>
                    <i class="ph ph-plus"></i>
                </button>
            </div>
        }
    };

    // ---- New / Edit modal ----
    let modal_form = |title: &str,
                      is_edit: bool,
                      form_name: UseStateHandle<String>,
                      form_desc: UseStateHandle<String>,
                      form_icon: UseStateHandle<String>,
                      form_categories: UseStateHandle<Vec<String>>,
                      form_backfill: UseStateHandle<bool>,
                      available_categories: UseStateHandle<Vec<String>>,
                      on_submit: Callback<MouseEvent>,
                      on_close: Callback<MouseEvent>,
                      on_delete: Option<Callback<MouseEvent>>,
                      saving: bool| -> Html {
        let stop = Callback::from(|e: MouseEvent| e.stop_propagation());
        html! {
            <div
                class="fixed inset-0 z-50 overflow-y-auto bg-black bg-opacity-25"
                onclick={on_close.clone()}
            >
                <div class="flex min-h-full items-center justify-center p-4">
                    <div class="modal-container relative w-full max-w-md rounded-lg shadow" onclick={stop}>
                        <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                            <h3 class="text-xl font-semibold">{ title.to_string() }</h3>
                            <button onclick={on_close.clone()} class="text-gray-400 bg-transparent rounded-lg text-sm w-8 h-8 inline-flex justify-center items-center">
                                <i class="ph ph-x text-xl"></i>
                            </button>
                        </div>
                        <div class="p-4 md:p-5">
                            <div class="space-y-4">
                                <div>
                                    <label class="block mb-2 text-sm font-medium">{ i18n.t("collections.collection_name") }</label>
                                    <input
                                        type="text"
                                        class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                        value={(*form_name).clone()}
                                        placeholder={i18n.t("collections.collection_name")}
                                        oninput={let form_name = form_name.clone(); Callback::from(move |e: InputEvent| {
                                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                form_name.set(input.value());
                                            }
                                        })}
                                    />
                                </div>
                                <div>
                                    <label class="block mb-2 text-sm font-medium">{ i18n.t("collections.icon") }</label>
                                    <IconSelector
                                        selected_icon={(*form_icon).clone()}
                                        on_select={let form_icon = form_icon.clone(); Callback::from(move |icon: String| form_icon.set(icon))}
                                    />
                                </div>
                                <div>
                                    <label class="block mb-2 text-sm font-medium">{ i18n.t("collections.description") }</label>
                                    <textarea
                                        class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                        value={(*form_desc).clone()}
                                        oninput={let form_desc = form_desc.clone(); Callback::from(move |e: InputEvent| {
                                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                            form_desc.set(input.value());
                                        })}
                                    />
                                </div>
                                <div>
                                    <label class="block mb-1 text-sm font-medium">{ i18n.t("collections.auto_add_categories") }</label>
                                    <p class="text-xs opacity-70 mb-2">{ i18n.t("collections.auto_add_help") }</p>
                                    {
                                        if available_categories.is_empty() {
                                            html! { <p class="text-xs opacity-70">{ i18n.t("collections.no_categories") }</p> }
                                        } else {
                                            html! {
                                                <div class="sp-chips" style="justify-content: flex-start; margin: 0;">
                                                    {
                                                        available_categories.iter().map(|cat| {
                                                            let cat = cat.clone();
                                                            let selected = form_categories.contains(&cat);
                                                            let onclick = {
                                                                let form_categories = form_categories.clone();
                                                                let cat = cat.clone();
                                                                Callback::from(move |_| {
                                                                    let mut next = (*form_categories).clone();
                                                                    if let Some(pos) = next.iter().position(|c| c == &cat) {
                                                                        next.remove(pos);
                                                                    } else {
                                                                        next.push(cat.clone());
                                                                    }
                                                                    form_categories.set(next);
                                                                })
                                                            };
                                                            html! {
                                                                <button
                                                                    type="button"
                                                                    class={classes!("sp-chip", if selected { "is-active" } else { "" })}
                                                                    {onclick}
                                                                >
                                                                    <span>{ cat }</span>
                                                                </button>
                                                            }
                                                        }).collect::<Html>()
                                                    }
                                                </div>
                                            }
                                        }
                                    }
                                    {
                                        if !form_categories.is_empty() {
                                            html! {
                                                <label class="flex items-center gap-2 mt-3 text-sm cursor-pointer">
                                                    <input
                                                        type="checkbox"
                                                        checked={*form_backfill}
                                                        onchange={let form_backfill = form_backfill.clone(); Callback::from(move |e: Event| {
                                                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                            form_backfill.set(input.checked());
                                                        })}
                                                    />
                                                    <span>{ i18n.t("collections.backfill_existing") }</span>
                                                </label>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                                <div class="flex items-center justify-between gap-2 pt-2">
                                    {
                                        if let Some(on_delete) = on_delete.clone() {
                                            html! {
                                                <button onclick={on_delete} class="text-sm font-medium text-red-500 hover:underline">
                                                    { i18n.t("collections.delete_collection") }
                                                </button>
                                            }
                                        } else {
                                            html! { <span></span> }
                                        }
                                    }
                                    <button onclick={on_submit} disabled={saving} class="download-button">
                                        {
                                            if saving {
                                                i18n.t("collections.saving")
                                            } else if is_edit {
                                                i18n.t("collections.save")
                                            } else {
                                                i18n.t("collections.create")
                                            }
                                        }
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        }
    };

    let edit_button = if !active_is_default && active_collection.is_some() {
        html! {
            <button class="sp-chip" onclick={open_edit_modal} title={i18n.t("collections.edit_collection")}>
                <i class="ph ph-pencil-simple"></i>
                <span>{ i18n.t("collections.edit_collection") }</span>
            </button>
        }
    } else {
        html! {}
    };

    // Search bar reflects the active collection's name + icon.
    let (active_name, active_icon) = active_collection
        .as_ref()
        .map(|c| (c.name.clone(), c.icon.clone()))
        .unwrap_or_else(|| ("Saved".to_string(), "ph-bookmark".to_string()));
    let search_placeholder = i18n
        .t("collections.search_placeholder")
        .replace("{name}", &active_name);

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                if *collections_loading {
                    html! { <Loading/> }
                } else {
                    html! {
                        <>
                            <div class="pfb-section">
                                { tab_bar }
                                <div class="pfb-bar">
                                    <div class="sp-input">
                                        <i class={classes!("ph", active_icon.clone(), "sp-search-ico")}></i>
                                        <input
                                            type="text"
                                            placeholder={search_placeholder.clone()}
                                            value={(*episode_search_term).clone()}
                                            oninput={let episode_search_term = episode_search_term.clone();
                                                Callback::from(move |e: InputEvent| {
                                                    if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                        episode_search_term.set(input.value());
                                                    }
                                                })
                                            }
                                        />
                                    </div>
                                    <div class="pfb-sort">
                                        <select
                                            class="pfb-sort-select"
                                            onchange={
                                                let sort_value = sort_value.clone();
                                                let reload_trigger = reload_trigger.clone();
                                                Callback::from(move |e: Event| {
                                                    let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                    let value = target.value();
                                                    set_filter_preference("saved", &value);
                                                    sort_value.set(value);
                                                    reload_trigger.set(*reload_trigger + 1);
                                                })
                                            }
                                        >
                                            <option value="newest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "newest"}>{i18n.t("saved.newest_first")}</option>
                                            <option value="oldest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "oldest"}>{i18n.t("saved.oldest_first")}</option>
                                            <option value="shortest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "shortest"}>{i18n.t("saved.shortest_first")}</option>
                                            <option value="longest" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "longest"}>{i18n.t("saved.longest_first")}</option>
                                            <option value="title_az" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_az"}>{i18n.t("saved.title_az")}</option>
                                            <option value="title_za" selected={get_filter_preference("saved").unwrap_or_else(|| get_default_sort_direction().to_string()) == "title_za"}>{i18n.t("saved.title_za")}</option>
                                        </select>
                                        <i class="ph ph-caret-down pfb-sort-arrow"></i>
                                    </div>
                                </div>
                                <div class="sp-chips pfb-chips">
                                    { edit_button }
                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                set_filter_preference("saved_filter", "all");
                                                filter_value.set("all".to_string());
                                                reload_trigger.set(*reload_trigger + 1);
                                            })
                                        }
                                        class="sp-chip"
                                    >
                                        <i class="ph ph-broom"></i>
                                        <span>{i18n.t("saved.clear_all")}</span>
                                    </button>
                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                let next = if *filter_value == "completed" { "all" } else { "completed" };
                                                set_filter_preference("saved_filter", next);
                                                filter_value.set(next.to_string());
                                                reload_trigger.set(*reload_trigger + 1);
                                            })
                                        }
                                        class={classes!("sp-chip", if *filter_value == "completed" { "is-active" } else { "" })}
                                    >
                                        <i class="ph ph-check-circle"></i>
                                        <span>{i18n.t("saved.completed")}</span>
                                    </button>
                                    <button
                                        onclick={
                                            let filter_value = filter_value.clone();
                                            let reload_trigger = reload_trigger.clone();
                                            Callback::from(move |_| {
                                                let next = if *filter_value == "in_progress" { "all" } else { "in_progress" };
                                                set_filter_preference("saved_filter", next);
                                                filter_value.set(next.to_string());
                                                reload_trigger.set(*reload_trigger + 1);
                                            })
                                        }
                                        class={classes!("sp-chip", if *filter_value == "in_progress" { "is-active" } else { "" })}
                                    >
                                        <i class="ph ph-hourglass-medium"></i>
                                        <span>{i18n.t("saved.in_progress")}</span>
                                    </button>
                                </div>
                            </div>

                            {
                                if *loading {
                                    html! { <Loading/> }
                                } else if display_empty {
                                    if active_is_default {
                                        empty_message(
                                            &i18n.t("saved.no_saved_episodes"),
                                            &i18n.t("saved.save_episodes_instructions")
                                        )
                                    } else {
                                        empty_message(
                                            &i18n.t("collections.empty_title"),
                                            &i18n.t("collections.empty_instructions")
                                        )
                                    }
                                } else {
                                    html! {
                                        <div class="flex-grow overflow-y-auto">
                                            <EpisodeListView
                                                episodes={display_episodes_rc}
                                                backend_can_load_more={backend_can_load_more}
                                                loading_more={*loading_more}
                                                on_load_more={on_load_more.clone()}
                                                page_type={PageType::Saved}
                                            />
                                        </div>
                                    }
                                }
                            }
                        </>
                    }
                }
            }
            <AudioPlayerBar />
        </div>
        {
            if *show_new_modal {
                modal_form(
                    &i18n.t("collections.new_collection"),
                    false,
                    form_name.clone(), form_desc.clone(), form_icon.clone(),
                    form_categories.clone(), form_backfill.clone(), available_categories.clone(),
                    on_create_submit.clone(),
                    { let show_new_modal = show_new_modal.clone(); Callback::from(move |_| show_new_modal.set(false)) },
                    None,
                    *form_saving,
                )
            } else { html! {} }
        }
        {
            if *show_edit_modal {
                modal_form(
                    &i18n.t("collections.edit_collection"),
                    true,
                    form_name.clone(), form_desc.clone(), form_icon.clone(),
                    form_categories.clone(), form_backfill.clone(), available_categories.clone(),
                    on_edit_submit.clone(),
                    { let show_edit_modal = show_edit_modal.clone(); Callback::from(move |_| show_edit_modal.set(false)) },
                    Some(on_delete_collection.clone()),
                    *form_saving,
                )
            } else { html! {} }
        }
        <App_drawer />
        </>
    }
}
