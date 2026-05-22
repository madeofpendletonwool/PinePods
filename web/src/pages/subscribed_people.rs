use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::{AppState, ExpandedDescriptions};
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::requests::episode::Episode;
use crate::requests::people_req::{self, PersonSubscription};
use i18nrs::yew::use_translation;
use std::rc::Rc;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
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

    // let error = use_state(|| None);
    let (post_state, post_dispatch) = use_store::<AppState>();
    let loading = use_state(|| true);
    let expanded_state = use_state(|| std::collections::HashMap::<i32, bool>::new());
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

    // Effect to fetch subscriptions on component mount
    // Effect to fetch subscriptions on component mount
    //
    //
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

    {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let subscribed_people = subscribed_people.clone();
        let expanded_state = expanded_state.clone();

        use_effect_with(expanded_state.clone(), move |expanded_state| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let user_id = user_id.clone();
            let subscribed_people = subscribed_people.clone();

            if let (Some(server), Some(Some(key)), Some(uid)) = (server_name, api_key, user_id) {
                let people = (*subscribed_people).clone();

                // Move people IDs into the async block instead of the whole people Vec
                let person_ids: Vec<_> = people
                    .iter()
                    .filter(|person| {
                        expanded_state
                            .get(&person.person.personid)
                            .copied()
                            .unwrap_or(false)
                            && person.episodes.is_empty()
                    })
                    .map(|person| (person.person.personid, person.person.name.clone()))
                    .collect();

                for (person_id, person_name) in person_ids {
                    let server = server.clone();
                    let key = key.clone();
                    let subscribed_people = subscribed_people.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        // In the effect that loads episodes
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
                                                web_sys::console::log_1(
                                                    &format!(
                                                        "Setting episodes for person {}: {:?}",
                                                        person_id, new_episodes
                                                    )
                                                    .into(),
                                                );
                                                p.episodes = new_episodes.clone();
                                            }
                                            p
                                        })
                                        .collect(),
                                );
                                // Log the updated state
                                web_sys::console::log_1(
                                    &format!("Updated people state: {:?}", *subscribed_people)
                                        .into(),
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

    let people = (*subscribed_people).clone();
    let render_people = {
        let people = people.clone();
        let active_clonedal = active_clonedal.clone();
        let no_people_found = i18n.t("people_subs.no_subscribed_people_found").to_string();
        let subscribe_message = i18n.t("people_subs.subscribe_to_hosts_message").to_string();
        let episode_count_text = i18n.t("people_subs.episode_count").to_string();
        let shows_text = i18n.t("people_subs.shows").to_string();
        let avatar_alt_text = i18n.t("people_subs.avatar_alt").to_string();

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
                            html! {
                                <div key={person.person.personid}>
                                    { render_host_with_episodes(
                                        &person.person,
                                        person.episodes.clone(),
                                        is_expanded,
                                        toggle_person.reform(move |_| person.person.personid),
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

#[allow(dead_code)]
fn get_proxied_image_url(server_name: &str, original_url: String) -> String {
    let proxied_url = format!(
        "{}/api/proxy/image?url={}",
        server_name,
        urlencoding::encode(&original_url)
    );
    web_sys::console::log_1(&format!("Proxied URL: {}", proxied_url).into());
    proxied_url
}

#[allow(dead_code)]
fn render_host_with_episodes(
    person: &PersonSubscription,
    episodes: Vec<Episode>,
    is_expanded: bool,
    toggle_host_expanded: Callback<MouseEvent>,
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
) -> Html {
    let _episode_count = episodes.len();
    let history_clone = BrowserHistory::new();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let proxied_url = get_proxied_image_url(&server_name.clone().unwrap(), person.image.clone());

    let handle_click = {
        let toggle_host_expanded = toggle_host_expanded.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation(); // Stop event propagation
            e.prevent_default(); // Prevent default behavior
            toggle_host_expanded.emit(e);
        })
    };

    html! {
        <div key={person.personid}>
            <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full" onclick={handle_click}>
                <div class="flex flex-col w-auto object-cover pl-4">
                    <img
                        src={format!("{}", proxied_url)}
                        alt={format!("{} {}", avatar_alt_text, person.name)}
                        class="person-avatar"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <p class="item_container-text episode-title font-semibold cursor-pointer">
                        { &person.name }
                    </p>
                    <hr class="my-2 border-t hidden md:block"/>
                    <p class="item_container-text">{ format!("{}: {}", episode_count_text, person.episode_count) }</p>
                    <p class="item_container-text text-sm">{ format!("{}: {}", shows_text, person.associatedpodcasts) }</p>
                </div>
            </div>
            { if is_expanded {
                html! {
                    <div class="episodes-dropdown pl-4 flex-grow overflow-y-auto">
                        { for episodes.iter().map(|ep| html! {
                            <EpisodeListItem key={ep.episodeid} episode={ep.clone()} />
                        }) }
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
