use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, ExpandedDescriptions};
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::requests::episode::Episode;
use crate::requests::people_req::{
    self, call_unsubscribe_from_person, PersonSubscription,
};
use i18nrs::yew::use_translation;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

use wasm_bindgen::prelude::*;

#[derive(Clone, PartialEq, Properties, Debug)]
struct PersonWithEpisodes {
    person: PersonSubscription,
    episodes: Vec<Episode>,
    is_expanded: bool,
}

#[function_component(SubscribedPeople)]
pub fn subscribed_people() -> Html {
    let (i18n, _) = use_translation();

    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let active_modal = use_state(|| None::<i32>);
    let show_modal = use_state(|| false);
    let active_clonedal = active_modal.clone();
    let active_modal_clone = active_modal.clone();
    let on_modal_open = Callback::from(move |episode_id: i32| {
        active_modal_clone.set(Some(episode_id));
    });
    let active_modal_clone = active_modal.clone();
    let on_modal_close = Callback::from(move |_| {
        active_modal_clone.set(None);
    });

    let (post_state, post_dispatch) = use_store::<AppState>();
    let loading = use_state(|| true);
    let expanded_state = use_state(|| std::collections::HashMap::<i32, bool>::new());
    let loading_episodes = use_state(|| std::collections::HashSet::<i32>::new());
    let subscribed_people = use_state(|| Vec::<PersonWithEpisodes>::new());
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    // Fetch subscriptions on mount
    {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let subscribed_people = subscribed_people.clone();
        let loading = loading.clone();

        use_effect_with((), move |_| {
            if let (Some(server), Some(Some(key)), Some(uid)) =
                (server_name.clone(), api_key.clone(), user_id.clone())
            {
                wasm_bindgen_futures::spawn_local(async move {
                    match people_req::call_get_person_subscriptions(&server, &key, uid).await {
                        Ok(subscriptions) => {
                            let people = subscriptions
                                .into_iter()
                                .map(|sub| PersonWithEpisodes {
                                    person: sub,
                                    episodes: vec![],
                                    is_expanded: false,
                                })
                                .collect();
                            subscribed_people.set(people);
                        }
                        Err(e) => {
                            log::error!("Failed to fetch subscriptions: {}", e);
                        }
                    }
                    loading.set(false);
                });
            } else {
                loading.set(false);
            }
            || ()
        });
    }

    // Fetch episodes when a person is expanded
    {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let subscribed_people = subscribed_people.clone();
        let expanded_state = expanded_state.clone();
        let loading_episodes = loading_episodes.clone();

        use_effect_with(expanded_state.clone(), move |expanded_state| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let user_id = user_id.clone();
            let subscribed_people = subscribed_people.clone();

            if let (Some(server), Some(Some(key)), Some(uid)) = (server_name, api_key, user_id) {
                let people = (*subscribed_people).clone();

                let person_ids: Vec<_> = people
                    .iter()
                    .filter(|person| {
                        expanded_state
                            .get(&person.person.personid)
                            .copied()
                            .unwrap_or(false)
                            && person.episodes.is_empty()
                            && !loading_episodes.contains(&person.person.personid)
                    })
                    .map(|person| (person.person.personid, person.person.name.clone()))
                    .collect();

                // Mark all pending persons as loading before spawning tasks
                if !person_ids.is_empty() {
                    let mut new_loading = (*loading_episodes).clone();
                    for (pid, _) in &person_ids {
                        new_loading.insert(*pid);
                    }
                    loading_episodes.set(new_loading);
                }

                for (person_id, person_name) in person_ids {
                    let server = server.clone();
                    let key = key.clone();
                    let subscribed_people = subscribed_people.clone();
                    let loading_episodes_task = loading_episodes.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match people_req::call_get_person_episodes(&server, &key, uid, person_id)
                            .await
                        {
                            Ok(new_episodes) => {
                                subscribed_people.set(
                                    (*subscribed_people)
                                        .clone()
                                        .into_iter()
                                        .map(|mut p| {
                                            if p.person.personid == person_id {
                                                p.episodes = new_episodes.clone();
                                            }
                                            p
                                        })
                                        .collect(),
                                );
                            }
                            Err(e) => {
                                log::error!(
                                    "Failed to fetch episodes for person {}: {}",
                                    person_name,
                                    e
                                );
                            }
                        }
                        loading_episodes_task.set(
                            (*loading_episodes_task).iter().copied().filter(|&id| id != person_id).collect()
                        );
                    });
                }
            }
            || ()
        });
    }

    let toggle_person = {
        let expanded_state = expanded_state.clone();
        Callback::from(move |person_id: i32| {
            let mut new_state = (*expanded_state).clone();
            let current_state = new_state.get(&person_id).copied().unwrap_or(false);
            new_state.insert(person_id, !current_state);
            expanded_state.set(new_state);
        })
    };

    // Unsubscribe: optimistic removal from local state, then API call
    let on_unsubscribe = {
        let subscribed_people = subscribed_people.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();

        Callback::from(move |(person_id, person_name): (i32, String)| {
            // Optimistic removal
            subscribed_people.set(
                (*subscribed_people)
                    .iter()
                    .filter(|p| p.person.personid != person_id)
                    .cloned()
                    .collect(),
            );

            let server = server_name.clone();
            let key = api_key.clone();
            let uid = user_id.clone();
            let subscribed_people_revert = subscribed_people.clone();

            spawn_local(async move {
                if let (Some(server), Some(Some(key)), Some(uid)) = (server, key, uid) {
                    if let Err(e) = call_unsubscribe_from_person(
                        &server,
                        &key,
                        uid,
                        person_id,
                        person_name,
                    )
                    .await
                    {
                        log::error!("Failed to unsubscribe: {}", e);
                        // No revert here — page reload will show correct state
                    }
                }
            });
        })
    };

    // Navigate to person profile page
    let history = BrowserHistory::new();
    let on_navigate_to_person = {
        let history = history.clone();
        Callback::from(move |person_name: String| {
            history.push(format!("/person/{}", person_name));
        })
    };

    let people = (*subscribed_people).clone();
    let render_people = {
        let people = people.clone();
        let active_clonedal = active_clonedal.clone();
        let no_people_found = i18n.t("people_subs.no_subscribed_people_found").to_string();
        let subscribe_message = i18n.t("people_subs.subscribe_to_hosts_message").to_string();
        let episode_count_text = i18n.t("people_subs.episode_count").to_string();
        let shows_text = i18n.t("people_subs.shows").to_string();
        let avatar_alt_text = i18n.t("people_subs.avatar_alt").to_string();
        let i18n_loading_episodes = i18n.t("people_subs.loading_episodes").to_string();
        let i18n_no_recent_episodes_found = i18n.t("people_subs.no_recent_episodes_found").to_string();
        let loading_episodes = loading_episodes.clone();

        move || {
            if people.is_empty() {
                html! {
                    { empty_message(
                        &no_people_found,
                        &subscribe_message
                    )}
                }
            } else {
                html! {
                    <div>
                    {
                        people.into_iter().map(|person| {
                            let active_modal = active_clonedal.clone();
                            let is_expanded = *expanded_state.get(&person.person.personid).unwrap_or(&false);
                            let is_loading_eps = loading_episodes.contains(&person.person.personid);
                            let person_id = person.person.personid;
                            let person_name = person.person.name.clone();
                            let person_name2 = person_name.clone();
                            html! {
                                <div key={person.person.personid}>
                                    { render_host_with_episodes(
                                        &person.person,
                                        person.episodes.clone(),
                                        is_expanded,
                                        is_loading_eps,
                                        toggle_person.reform(move |_| person_id),
                                        on_unsubscribe.reform(move |_| (person_id, person_name.clone())),
                                        on_navigate_to_person.reform(move |_| person_name2.clone()),
                                        post_state.clone(),
                                        post_dispatch.clone(),
                                        desc_state.clone(),
                                        desc_dispatch.clone(),
                                        *show_modal,
                                        on_modal_open.clone(),
                                        on_modal_close.clone(),
                                        active_modal,
                                        &episode_count_text,
                                        &shows_text,
                                        &avatar_alt_text,
                                        &i18n_loading_episodes,
                                        &i18n_no_recent_episodes_found,
                                    )}
                                </div>
                            }
                        }).collect::<Html>()
                    }
                    </div>
                }
            }
        }
    };

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                if *loading {
                    html! { <Loading/> }
                } else {
                    html! {
                        <div>
                            <h1 class="text-2xl item_container-text font-bold text-center mb-6">{&i18n.t("people_subs.subscribed_people")}</h1>
                            { render_people() }
                        </div>
                    }
                }
            }
            <AudioPlayerBar />
        </div>
        <App_drawer />
        </>
    }
}

fn get_proxied_image_url(server_name: &str, original_url: String) -> String {
    format!(
        "{}/api/proxy/image?url={}",
        server_name,
        urlencoding::encode(&original_url)
    )
}

#[allow(clippy::too_many_arguments)]
fn render_host_with_episodes(
    person: &PersonSubscription,
    episodes: Vec<Episode>,
    is_expanded: bool,
    is_loading: bool,
    toggle_host_expanded: Callback<MouseEvent>,
    on_unsubscribe: Callback<MouseEvent>,
    on_navigate: Callback<MouseEvent>,
    state: Rc<AppState>,
    dispatch: Dispatch<AppState>,
    desc_rc: Rc<ExpandedDescriptions>,
    desc_state: Dispatch<ExpandedDescriptions>,
    _show_modal: bool,
    on_modal_open: Callback<i32>,
    on_modal_close: Callback<MouseEvent>,
    active_modal: UseStateHandle<Option<i32>>,
    episode_count_text: &str,
    shows_text: &str,
    avatar_alt_text: &str,
    loading_episodes_text: &str,
    no_recent_episodes_text: &str,
) -> Html {
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let proxied_url = get_proxied_image_url(&server_name.clone().unwrap_or_default(), person.image.clone());

    let handle_expand = {
        let toggle_host_expanded = toggle_host_expanded.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            e.prevent_default();
            toggle_host_expanded.emit(e);
        })
    };

    html! {
        <div key={person.personid}>
            <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full">
                // Clickable avatar + name → navigates to person profile
                <div class="flex flex-col w-auto object-cover pl-4 cursor-pointer" onclick={on_navigate.clone()}>
                    <img
                        src={format!("{}", proxied_url)}
                        alt={format!("{} {}", avatar_alt_text, person.name)}
                        class="person-avatar"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <p class="item_container-text episode-title font-semibold cursor-pointer text-blue-500 hover:underline"
                       onclick={on_navigate}>
                        { &person.name }
                    </p>
                    <hr class="my-2 border-t hidden md:block"/>
                    {
                        if person.episode_count == 0 {
                            html! { <p class="item_container-text text-sm italic text-gray-400">{ loading_episodes_text }</p> }
                        } else {
                            html! { <p class="item_container-text">{ format!("{}: {}", episode_count_text, person.episode_count) }</p> }
                        }
                    }
                    <p class="item_container-text text-sm">{ format!("{}: {}", shows_text, person.associatedpodcasts) }</p>
                </div>
                // Right-side action buttons
                <div class="flex flex-col items-center justify-center gap-2 px-4 self-center">
                    // Expand/collapse episodes button
                    <button
                        class="p-2 text-gray-500 hover:text-gray-700 dark:text-gray-300"
                        onclick={handle_expand}
                        title="Toggle episodes"
                    >
                        <i class={if is_expanded { "ph ph-caret-up text-xl" } else { "ph ph-caret-down text-xl" }}></i>
                    </button>
                    // Unsubscribe button
                    <button
                        class="px-2 py-1 bg-red-500 hover:bg-red-600 text-white text-sm rounded"
                        onclick={on_unsubscribe}
                        title="Unsubscribe"
                    >
                        <i class="ph ph-user-minus"></i>
                    </button>
                </div>
            </div>
            { if is_expanded {
                html! {
                    <div class="episodes-dropdown pl-4 flex-grow overflow-y-auto">
                        { if is_loading && episodes.is_empty() {
                            html! {
                                <div class="flex items-center gap-2 p-4">
                                    <div class="loading-animation" style="transform: scale(0.35); transform-origin: left center; height: 24px; width: 60px;"></div>
                                    <p class="item_container-text text-sm italic text-gray-400">{ loading_episodes_text }</p>
                                </div>
                            }
                        } else if episodes.is_empty() {
                            html! {
                                <p class="item_container-text text-sm italic text-gray-400 p-4">
                                    { no_recent_episodes_text }
                                </p>
                            }
                        } else {
                            html! {
                                <>
                                { for episodes.iter().map(|ep| html! {
                                    // Use episodeurl as key to avoid -1 ID collisions for system-podcast episodes
                                    <EpisodeListItem key={ep.episodeurl.clone()} episode={ep.clone()} />
                                }) }
                                </>
                            }
                        }}
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
