use super::app_drawer::App_drawer;
use super::gen_components::{
    empty_message, on_shownotes_click, person_episode_item, Search_nav, UseScrollToTop,
};
use super::virtual_list::PersonEpisodeVirtualList;
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, ExpandedDescriptions, UIState};
use crate::components::gen_funcs::sanitize_html_with_blank_target;
use crate::requests::people_req::PersonEpisode;
use crate::requests::people_req::{self, PersonSubscription};
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;
// use crate::components::gen_funcs::check_auth;
use crate::components::gen_funcs::format_datetime;
use crate::components::gen_funcs::match_date_format;
use crate::components::gen_funcs::parse_date;
use std::rc::Rc;

use wasm_bindgen::prelude::*;

#[derive(Clone, PartialEq, Properties, Debug)]
struct PersonWithEpisodes {
    person: PersonSubscription,
    episodes: Vec<PersonEpisode>,
    is_expanded: bool,
}

#[function_component(SubscribedPeople)]
pub fn subscribed_people() -> Html {
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
    let (audio_state, audio_dispatch) = use_store::<UIState>();
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
    let render_audio = audio_state.clone();
    let render_people = {
        let people = people.clone();
        let active_clonedal = active_clonedal.clone();
        move || {
            if people.is_empty() {
                html! {
                    { empty_message(
                        "No Subscribed People Found",
                        "Subscribe to podcast hosts and guests to see their latest episodes here!"
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
                                        render_audio.clone(),
                                        desc_state.clone(),
                                        desc_dispatch.clone(),
                                        audio_dispatch.clone(),
                                        *show_modal,
                                        on_modal_open.clone(),
                                        on_modal_close.clone(),
                                        active_modal,
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
                    html! {
                        <div class="loading-animation">
                            <div class="frame1"></div>
                            <div class="frame2"></div>
                            <div class="frame3"></div>
                            <div class="frame4"></div>
                            <div class="frame5"></div>
                            <div class="frame6"></div>
                        </div>
                    }
                } else {
                    html! {
                        <div>
                            <h1 class="text-2xl item_container-text font-bold text-center mb-6">{"Subscribed People"}</h1>
                            { render_people() }
                        </div>
                    }
                }
            }
            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! {
                        <AudioPlayer
                            src={audio_props.src.clone()}
                            title={audio_props.title.clone()}
                            description={audio_props.description.clone()}
                            release_date={audio_props.release_date.clone()}
                            artwork_url={audio_props.artwork_url.clone()}
                            duration={audio_props.duration.clone()}
                            episode_id={audio_props.episode_id.clone()}
                            duration_sec={audio_props.duration_sec.clone()}
                            start_pos_sec={audio_props.start_pos_sec.clone()}
                            end_pos_sec={audio_props.end_pos_sec.clone()}
                            offline={audio_props.offline.clone()}
                            is_youtube={audio_props.is_youtube.clone()}
                        />
                    }
                } else {
                    html! {}
                }
            }
        </div>
        <App_drawer />
        </>
    }
}

fn get_proxied_image_url(server_name: &str, original_url: String) -> String {
    let proxied_url = format!(
        "{}/api/proxy/image?url={}",
        server_name,
        urlencoding::encode(&original_url)
    );
    web_sys::console::log_1(&format!("Proxied URL: {}", proxied_url).into());
    proxied_url
}

fn render_host_with_episodes(
    person: &PersonSubscription,
    episodes: Vec<PersonEpisode>,
    is_expanded: bool,
    toggle_host_expanded: Callback<MouseEvent>,
    state: Rc<AppState>,
    dispatch: Dispatch<AppState>,
    audio_state: Rc<UIState>,
    desc_rc: Rc<ExpandedDescriptions>,
    desc_state: Dispatch<ExpandedDescriptions>,
    audio_dispatch: Dispatch<UIState>,
    _show_modal: bool,
    on_modal_open: Callback<i32>,
    on_modal_close: Callback<MouseEvent>,
    active_modal: UseStateHandle<Option<i32>>,
) -> Html {
    let episode_count = episodes.len();
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
                        alt={format!("Avatar for {}", person.name)}
                        class="person-avatar"
                    />
                </div>
                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                    <p class="item_container-text episode-title font-semibold cursor-pointer">
                        { &person.name }
                    </p>
                    <hr class="my-2 border-t hidden md:block"/>
                    <p class="item_container-text">{ format!("Episode Count: {}", person.episode_count) }</p>
                    <p class="item_container-text text-sm">{ format!("Shows: {}", person.associatedpodcasts) }</p>
                </div>
            </div>
            { if is_expanded {
                let episode_count = episodes.len();
                web_sys::console::log_1(&format!("Attempting to render {} episodes", episode_count).into());
                html! {
                    <div class="episodes-dropdown pl-4">
                        <PersonEpisodeVirtualList
                            episodes={episodes.clone()}
                            item_height={234.0}
                            search_state={state.clone()}
                            search_ui_state={audio_state.clone()}
                            dispatch={audio_dispatch.clone()}
                            search_dispatch={dispatch.clone()}
                            history={history_clone.clone()}
                            server_name={server_name.clone()}
                            user_id={user_id.clone()}
                            api_key={api_key.clone()}
                        />
                        // Old episode rendering code (disabled for virtual scrolling)
                        { if false { 
                            let _ = episodes.iter().map(|episode| {
                            let id_string = episode.episodeid.to_string();
                            let desc_expanded = desc_rc.expanded_descriptions.contains(&id_string);
                            let episode_url_for_closure = episode.episodeurl.clone();
                            let episode_title_for_closure = episode.episodetitle.clone();
                            let episode_description_for_closure = episode.episodedescription.clone();
                            let episode_artwork_for_closure = episode.episodeartwork.clone();
                            let episode_duration_for_closure = episode.episodeduration.clone();
                            let listener_duration_for_closure = episode.listenduration.clone();
                            let episode_id_for_closure = episode.episodeid.clone();
                            let episode_is_youtube = Some(episode.is_youtube.clone());
                            let _completed = false;
                            let user_id_play = user_id.clone();
                            let server_name_play = server_name.clone();
                            let api_key_play = api_key.clone();
                            let audio_dispatch = audio_dispatch.clone();

                            let is_current_episode = audio_state
                                .currently_playing
                                .as_ref()
                                .map_or(false, |current| current.episode_id == episode.episodeid);
                            let is_playing = audio_state.audio_playing.unwrap_or(false);


                            let date_format = match_date_format(state.date_format.as_deref());
                            let datetime = parse_date(&episode.episodepubdate, &state.user_tz);
                            let format_release = format!(
                                "{}",
                                format_datetime(&datetime, &state.hour_preference, date_format)
                            );

                            let on_play_pause = on_play_pause(
                                episode_url_for_closure.clone(),
                                episode_title_for_closure.clone(),
                                episode_description_for_closure.clone(),
                                format_release.clone(),
                                episode_artwork_for_closure.clone().unwrap(),
                                episode_duration_for_closure.clone(),
                                episode_id_for_closure.clone(),
                                Some(listener_duration_for_closure.clone()),
                                api_key_play.unwrap().unwrap(),
                                user_id_play.unwrap(),
                                server_name_play.unwrap(),
                                audio_dispatch.clone(),
                                audio_state.clone(),
                                None,
                                episode_is_youtube.clone(),
                            );

                            let on_shownotes_click = on_shownotes_click(
                                history_clone.clone(),
                                dispatch.clone(),
                                Some(episode_id_for_closure.clone()),
                                Some(String::from("Not needed")),
                                Some(String::from("Not needed")),
                                Some(String::from("Not needed")),
                                true,
                                Some(true),
                                Some(false),
                            );

                            #[wasm_bindgen]
                            extern "C" {
                                #[wasm_bindgen(js_namespace = window)]
                                fn toggleDescription(guid: &str, expanded: bool);
                            }
                            let toggle_expanded = {
                                let desc_dispatch = desc_state.clone();
                                let episode_guid = episode.episodeid.clone().to_string();

                                Callback::from(move |_: MouseEvent| {
                                    let guid = episode_guid.clone();
                                    desc_dispatch.reduce_mut(move |state| {
                                        if state.expanded_descriptions.contains(&guid) {
                                            state.expanded_descriptions.remove(&guid); // Collapse the description
                                            toggleDescription(&guid, false); // Call JavaScript function
                                        } else {
                                            state.expanded_descriptions.insert(guid.clone()); // Expand the description
                                            toggleDescription(&guid, true); // Call JavaScript function
                                        }
                                    });
                                })
                            };
                            let show_modal = *active_modal == Some(episode.episodeid);
                            person_episode_item(
                                Box::new(episode.clone()),
                                sanitize_html_with_blank_target(&episode.episodedescription),
                                desc_expanded,
                                &format_release,
                                on_play_pause,
                                on_shownotes_click,
                                toggle_expanded,
                                episode.episodeduration,
                                Some(episode.listenduration),
                                "people",
                                Callback::noop(),
                                false,
                                episode.episodeurl.clone(),
                                false,
                                show_modal,
                                on_modal_open.clone(),
                                on_modal_close.clone(),
                                is_current_episode,
                                is_playing,
                            )
                            }).collect::<Vec<_>>();
                            html! {}
                        } else {
                            html! {}
                        } }
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
    }
}
