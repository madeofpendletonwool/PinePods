use crate::components::context::{AppState, CollectionModalState, EpisodeStatusState, NotificationState};
use crate::requests::episode::Episode;
use crate::requests::pod_req::{
    self, Collection, CollectionEpisodeRequest, CreateCollectionRequest,
};
use i18nrs::yew::use_translation;
use std::collections::HashSet;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yewdux::prelude::*;

/// Global overlay (mounted once at the app root) that adds/removes the active episode
/// to/from collections. Checking a box commits the change immediately — no Save step.
#[function_component(CollectionPickerModal)]
pub fn collection_picker_modal() -> Html {
    let (modal_state, modal_dispatch) = use_store::<CollectionModalState>();
    let (app_state, _) = use_store::<AppState>();
    let (i18n, _) = use_translation();

    let server = app_state
        .auth_details
        .as_ref()
        .map(|d| d.server_name.clone())
        .unwrap_or_default();
    let api_key = app_state
        .auth_details
        .as_ref()
        .and_then(|d| d.api_key.clone())
        .unwrap_or_default();
    let user_id = app_state.user_details.as_ref().map(|d| d.UserID).unwrap_or(0);

    let collections = use_state(|| Vec::<Collection>::new());
    let members = use_state(|| HashSet::<i32>::new());
    let search = use_state(|| String::new());
    let loading = use_state(|| true);
    let show_new_input = use_state(|| false);
    let new_name = use_state(|| String::new());

    let is_open = modal_state.open;
    let episode: Option<Episode> = modal_state.episode.clone();
    let episode_key = episode.as_ref().map(|e| (e.episodeid, e.is_youtube));

    // (Re)load collections + membership whenever the modal opens for a (new) episode
    {
        let collections = collections.clone();
        let members = members.clone();
        let loading = loading.clone();
        let search = search.clone();
        let show_new_input = show_new_input.clone();
        let new_name = new_name.clone();
        let server = server.clone();
        let api_key = api_key.clone();
        use_effect_with((is_open, episode_key), move |(is_open, episode_key)| {
            if *is_open {
                if let Some((ep_id, is_yt)) = *episode_key {
                    // fresh state each open
                    search.set(String::new());
                    show_new_input.set(false);
                    new_name.set(String::new());
                    loading.set(true);
                    let collections = collections.clone();
                    let members = members.clone();
                    let loading = loading.clone();
                    let server = server.clone();
                    let api_key = api_key.clone();
                    spawn_local(async move {
                        let cols = pod_req::call_get_collections(&server, &api_key, user_id)
                            .await
                            .unwrap_or_default();
                        let mem = pod_req::call_get_episode_collections(
                            &server, &api_key, user_id, ep_id, is_yt,
                        )
                        .await
                        .unwrap_or_default();
                        collections.set(cols);
                        members.set(mem.into_iter().collect());
                        loading.set(false);
                    });
                }
            }
            || ()
        });
    }

    if !is_open {
        return html! {};
    }
    let Some(episode) = episode else { return html! {} };
    let ep_id = episode.episodeid;
    let is_yt = episode.is_youtube;

    let close = {
        let modal_dispatch = modal_dispatch.clone();
        Callback::from(move |_| {
            modal_dispatch.reduce_mut(|s| {
                s.open = false;
                s.episode = None;
            });
        })
    };

    // Immediate add/remove on toggle
    let make_toggle = {
        let members = members.clone();
        let collections = collections.clone();
        let server = server.clone();
        let api_key = api_key.clone();
        let episode = episode.clone();
        move |col_id: i32, is_member: bool| -> Callback<MouseEvent> {
            let members = members.clone();
            let is_default = collections
                .iter()
                .find(|c| c.collection_id == col_id)
                .map(|c| c.is_default)
                .unwrap_or(false);
            let server = server.clone();
            let api_key = api_key.clone();
            let episode = episode.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                let mut set = (*members).clone();
                let now_member = if is_member {
                    set.remove(&col_id);
                    false
                } else {
                    set.insert(col_id);
                    true
                };
                members.set(set);
                let req = CollectionEpisodeRequest { user_id, episode_id: ep_id, is_youtube: is_yt };
                let server = server.clone();
                let api_key = api_key.clone();
                let episode = episode.clone();
                spawn_local(async move {
                    let res = if now_member {
                        pod_req::call_add_episode_to_collection(&server, &api_key, col_id, &req).await
                    } else {
                        pod_req::call_remove_episode_from_collection(&server, &api_key, col_id, &req).await
                    };
                    if let Err(e) = res {
                        Dispatch::<NotificationState>::global().reduce_mut(|s| {
                            s.error_message = Some(format!("{}", e));
                        });
                    } else if is_default {
                        // Keep the saved badge in sync
                        if now_member {
                            let ep = episode.clone();
                            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                if !s.saved_episode_ids().any(|id| id == ep_id) {
                                    s.saved_episodes.push(ep);
                                }
                            });
                        } else {
                            Dispatch::<EpisodeStatusState>::global().reduce_mut(move |s| {
                                s.saved_episodes.retain(|e| e.episodeid != ep_id);
                            });
                        }
                    }
                });
            })
        }
    };

    let on_create_new = {
        let server = server.clone();
        let api_key = api_key.clone();
        let collections = collections.clone();
        let members = members.clone();
        let new_name = new_name.clone();
        let show_new_input = show_new_input.clone();
        let episode = episode.clone();
        Callback::from(move |_| {
            let name = (*new_name).trim().to_string();
            if name.is_empty() {
                return;
            }
            let server = server.clone();
            let api_key = api_key.clone();
            let collections = collections.clone();
            let members = members.clone();
            let new_name = new_name.clone();
            let show_new_input = show_new_input.clone();
            let episode = episode.clone();
            spawn_local(async move {
                let req = CreateCollectionRequest {
                    user_id,
                    name,
                    description: None,
                    icon: Some("ph-bookmark-simple".to_string()),
                    auto_add_categories: None,
                    backfill: None,
                };
                match pod_req::call_create_collection(&server, &api_key, req).await {
                    Ok(resp) => {
                        // Add the episode to the freshly created collection right away
                        let add_req = CollectionEpisodeRequest { user_id, episode_id: ep_id, is_youtube: is_yt };
                        let _ = pod_req::call_add_episode_to_collection(&server, &api_key, resp.collection_id, &add_req).await;
                        if let Ok(cols) = pod_req::call_get_collections(&server, &api_key, user_id).await {
                            collections.set(cols);
                        }
                        let mut set = (*members).clone();
                        set.insert(resp.collection_id);
                        members.set(set);
                        new_name.set(String::new());
                        show_new_input.set(false);
                        let _ = &episode;
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

    let stop = Callback::from(|e: MouseEvent| e.stop_propagation());

    let search_term = (*search).to_lowercase();
    let filtered: Vec<Collection> = collections
        .iter()
        .filter(|c| search_term.is_empty() || c.name.to_lowercase().contains(&search_term))
        .cloned()
        .collect();

    html! {
        <div class="fixed inset-0 z-50 overflow-y-auto bg-black bg-opacity-25" onclick={close.clone()}>
            <div class="flex min-h-full items-center justify-center p-4">
                <div class="modal-container relative w-full max-w-md rounded-lg shadow" onclick={stop}>
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">{ i18n.t("collections.add_to_collection") }</h3>
                        <button onclick={close.clone()} class="text-gray-400 bg-transparent rounded-lg text-sm w-8 h-8 inline-flex justify-center items-center">
                            <i class="ph ph-x text-xl"></i>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        {
                            if *loading {
                                html! { <div class="flex justify-center p-4"><i class="ph ph-spinner text-2xl"></i></div> }
                            } else {
                                html! {
                                    <>
                                        <div class="sp-input mb-3">
                                            <i class="ph ph-magnifying-glass sp-search-ico"></i>
                                            <input
                                                type="text"
                                                placeholder={i18n.t("collections.search_collections")}
                                                value={(*search).clone()}
                                                oninput={let search = search.clone(); Callback::from(move |e: InputEvent| {
                                                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                    search.set(input.value());
                                                })}
                                            />
                                        </div>
                                        <ul class="max-h-64 overflow-y-auto mb-2">
                                            {
                                                filtered.iter().map(|c| {
                                                    let is_member = members.contains(&c.collection_id);
                                                    let onclick = make_toggle(c.collection_id, is_member);
                                                    html! {
                                                        <li class="dropdown-option flex items-center gap-2" {onclick}>
                                                            <i class={classes!("ph", if is_member { "ph-check-square" } else { "ph-square" }, "text-xl")}></i>
                                                            <i class={classes!("ph", c.icon.clone())}></i>
                                                            <span>{ c.name.clone() }</span>
                                                            {
                                                                if c.is_default {
                                                                    html! { <span class="text-secondary text-sm">{ format!(" ({})", i18n.t("collections.default")) }</span> }
                                                                } else { html! {} }
                                                            }
                                                        </li>
                                                    }
                                                }).collect::<Html>()
                                            }
                                        </ul>
                                        {
                                            if *show_new_input {
                                                html! {
                                                    <div class="flex items-center gap-2">
                                                        <input
                                                            type="text"
                                                            class="search-bar-input border text-sm rounded-lg block w-full p-2.5"
                                                            placeholder={i18n.t("collections.collection_name")}
                                                            value={(*new_name).clone()}
                                                            oninput={let new_name = new_name.clone(); Callback::from(move |e: InputEvent| {
                                                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                                new_name.set(input.value());
                                                            })}
                                                        />
                                                        <button onclick={on_create_new} class="download-button">{ i18n.t("collections.create") }</button>
                                                    </div>
                                                }
                                            } else {
                                                html! {
                                                    <button
                                                        class="dropdown-option flex items-center gap-2 w-full"
                                                        onclick={let show_new_input = show_new_input.clone(); Callback::from(move |_| show_new_input.set(true))}
                                                    >
                                                        <i class="ph ph-plus"></i>
                                                        <span>{ i18n.t("collections.new_collection") }</span>
                                                    </button>
                                                }
                                            }
                                        }
                                    </>
                                }
                            }
                        }
                    </div>
                </div>
            </div>
        </div>
    }
}
