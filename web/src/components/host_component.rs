use crate::components::context::{AppState, NotificationState};
use crate::requests::people_req::{
    call_get_person_subscriptions, call_subscribe_to_person, call_unsubscribe_from_person,
};
use crate::requests::pod_req::Person;
use i18nrs::yew::use_translation;
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew::Properties;
use yew::{function_component, html, use_effect_with, Callback, Html};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[derive(Clone, PartialEq, Debug)]
#[allow(dead_code)]
pub struct Host {
    pub name: String,
    pub role: Option<String>,
    pub group: Option<String>,
    pub img: Option<String>,
    pub href: Option<String>,
    pub id: Option<i32>,        // podpeople id
    pub person_id: Option<i32>, // database personid
}

#[derive(Properties, PartialEq, Clone)]
pub struct HostDropdownProps {
    pub title: String,
    pub hosts: Vec<Person>,
    pub podcast_feed_url: String,
    pub podcast_id: i32,
    pub podcast_index_id: i32,
}

#[derive(Properties, PartialEq, Clone)]
pub struct HostItemProps {
    pub host: Person,
    pub server_name: String,
    pub podcast_feed_url: String,
    pub subscribed_hosts: HashMap<String, Vec<i32>>,
    pub podcast_id: i32,
    pub on_subscribe_toggle: Callback<Person>,
    pub on_host_click: Callback<MouseEvent>,
}

#[function_component(HostItem)]
fn host_item(props: &HostItemProps) -> Html {
    let (i18n, _) = use_translation();
    let HostItemProps {
        host,
        server_name,
        podcast_feed_url: _,
        subscribed_hosts,
        podcast_id: _,
        on_subscribe_toggle,
        on_host_click,
    } = props;

    let i18n_subscribe = i18n.t("host_component.subscribe").to_string();
    let i18n_unsubscribe = i18n.t("host_component.unsubscribe").to_string();

    // Subscription is global per person — name-based check regardless of which podcast we're on
    let is_subscribed = subscribed_hosts.contains_key(&host.name);

    let on_subscribe_click = {
        let host = host.clone();
        let on_subscribe_toggle = on_subscribe_toggle.clone();
        Callback::from(move |_| {
            on_subscribe_toggle.emit(host.clone());
        })
    };

    fn get_proxied_image_url(server_name: &str, original_url: &str) -> String {
        format!(
            "{}/api/proxy/image?url={}",
            server_name,
            urlencoding::encode(original_url)
        )
    }

    html! {
        <div class="flex flex-col items-center">
            <div class="flex flex-col items-center cursor-pointer" onclick={on_host_click.clone()}>
                { if let Some(img) = &host.img {
                    let proxied_url = get_proxied_image_url(&server_name.clone(), img);
                    html! { <img src={proxied_url} alt={host.name.clone()} class="w-12 h-12 rounded-full" /> }
                } else {
                    html! {}
                }}
                <span class="text-center text-blue-500 hover:underline mt-1">{ &host.name }</span>
            </div>
            <button
                onclick={on_subscribe_click}
                class={if is_subscribed {
                    "mt-2 px-2 py-1 bg-red-500 text-white rounded"
                } else {
                    "mt-2 px-2 py-1 bg-green-500 text-white rounded"
                }}
            >
                { if is_subscribed { &i18n_unsubscribe } else { &i18n_subscribe } }
            </button>
        </div>
    }
}

#[function_component(HostDropdown)]
pub fn host_dropdown(
    HostDropdownProps {
        title,
        hosts,
        podcast_feed_url,
        podcast_id,
        podcast_index_id,
    }: &HostDropdownProps,
) -> Html {
    let (i18n, _) = use_translation();
    let (search_state, _) = use_store::<AppState>();

    let i18n_no_hosts_found = i18n.t("host_component.no_hosts_found").to_string();
    let i18n_add_hosts_here = i18n.t("host_component.add_hosts_here").to_string();
    let subscribed_hosts = use_state(|| HashMap::<String, Vec<i32>>::new());
    let person_ids = use_state(|| HashMap::<String, i32>::new());
    let api_key = search_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let server_name = search_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let user_id = search_state
        .user_details
        .as_ref()
        .map(|ud| ud.UserID.clone());
    let podpeople_url = search_state
        .server_details
        .as_ref()
        .map(|ud| ud.people_url.clone());
    let is_open = use_state(|| false);
    let toggle = {
        let is_open = is_open.clone();
        Callback::from(move |_| is_open.set(!*is_open))
    };

    let has_no_known_hosts = hosts.len() == 1
        && hosts[0].name == "Unknown Host"
        && hosts[0].role == Some("Host".to_string());

    let history = BrowserHistory::new();

    {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let subscribed_hosts = subscribed_hosts.clone();
        let person_ids = person_ids.clone();

        use_effect_with(
            (api_key.clone(), server_name.clone(), user_id.clone()),
            move |_| {
                if let (Some(api_key), Some(server_name), Some(user_id)) =
                    (api_key, server_name, user_id)
                {
                    spawn_local(async move {
                        match call_get_person_subscriptions(
                            &server_name,
                            &api_key.unwrap(),
                            user_id,
                        )
                        .await
                        {
                            Ok(subs) => {
                                let mut sub_map = HashMap::new();
                                let mut pid_map = HashMap::new();
                                for sub in subs {
                                    let associated_podcasts = sub
                                        .associatedpodcasts
                                        .to_string()
                                        .split(',')
                                        .filter_map(|s| s.parse::<i32>().ok())
                                        .collect::<Vec<i32>>();
                                    sub_map.insert(sub.name.clone(), associated_podcasts);
                                    pid_map.insert(sub.name, sub.personid);
                                }
                                subscribed_hosts.set(sub_map);
                                person_ids.set(pid_map);
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Failed to fetch subscriptions: {:?}", e).into(),
                                );
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    let render_host = {
        let subscribed_hosts = subscribed_hosts.clone();
        let podcast_feed_url = podcast_feed_url.clone();
        let history = history.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        move |host: &Person| {
            let host_name = host.name.clone();
            let history_clone = history.clone();

            let on_host_click = {
                let host_name = host_name.clone();
                let history = history_clone.clone();
                Callback::from(move |_: MouseEvent| {
                    history.push(format!("/person/{}", host_name));
                })
            };

            let sub_person_id = person_ids.clone();
            let on_subscribe_toggle = {
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let user_id = user_id.clone();
                let subscribed_hosts = subscribed_hosts.clone();
                let person_ids = sub_person_id.clone();
                let podcast_id = *podcast_id;

                Callback::from(move |host: Person| {
                    let api_key = api_key.clone();
                    let server_name = server_name.clone();
                    let user_id = user_id.clone();
                    let subscribed_hosts = subscribed_hosts.clone();
                    let host_name = host.name.clone();
                    let host_img = host.img.clone();
                    let host_id = host.id.unwrap_or(0);
                    let person_ids = person_ids.clone();

                    // Determine action from current state before optimistic update
                    let currently_subscribed = (*subscribed_hosts).contains_key(&host_name);

                    // Optimistic UI update
                    if currently_subscribed {
                        subscribed_hosts.set({
                            let mut hosts = (*subscribed_hosts).clone();
                            hosts.remove(&host_name);
                            hosts
                        });
                    } else {
                        subscribed_hosts.set({
                            let mut hosts = (*subscribed_hosts).clone();
                            hosts.insert(host_name.clone(), vec![podcast_id]);
                            hosts
                        });
                    }

                    spawn_local(async move {
                        let id_to_use =
                            (*person_ids).get(&host_name).copied().unwrap_or(host_id);

                        if currently_subscribed {
                            if let Err(e) = call_unsubscribe_from_person(
                                &server_name.unwrap(),
                                &api_key.unwrap().unwrap(),
                                user_id.unwrap(),
                                id_to_use,
                                host_name.clone(),
                            )
                            .await
                            {
                                web_sys::console::log_1(
                                    &format!("Failed to unsubscribe: {:?}", e).into(),
                                );
                                // Revert: re-insert
                                subscribed_hosts.set({
                                    let mut hosts = (*subscribed_hosts).clone();
                                    hosts.insert(host_name.clone(), vec![podcast_id]);
                                    hosts
                                });
                            }
                        } else {
                            match call_subscribe_to_person(
                                &server_name.unwrap(),
                                &api_key.unwrap().unwrap(),
                                user_id.unwrap(),
                                host_id,
                                &host_name,
                                &host_img,
                                podcast_id,
                            )
                            .await
                            {
                                Ok(response) => {
                                    person_ids.set({
                                        let mut ids = (*person_ids).clone();
                                        ids.insert(host_name.clone(), response.person_id);
                                        ids
                                    });
                                }
                                Err(e) => {
                                    web_sys::console::log_1(
                                        &format!("Failed to subscribe: {:?}", e).into(),
                                    );
                                    // Revert: remove
                                    subscribed_hosts.set({
                                        let mut hosts = (*subscribed_hosts).clone();
                                        hosts.remove(&host_name);
                                        hosts
                                    });
                                }
                            }
                        }
                    });
                })
            };
            let server_name_clone = server_name.clone();
            html! {
                <HostItem
                    host={host.clone()}
                    podcast_feed_url={podcast_feed_url.clone()}
                    server_name={server_name_clone.unwrap()}
                    subscribed_hosts={(*subscribed_hosts).clone()}
                    podcast_id={podcast_id}
                    on_subscribe_toggle={on_subscribe_toggle}
                    on_host_click={on_host_click}
                />
            }
        }
    };
    let host_url = format!(
        "{}/podcast/{}",
        podpeople_url.unwrap().unwrap(),
        podcast_index_id
    );
    html! {
        <div class="inline-block">
            <button
                class="flex items-center text-gray-700 dark:text-gray-300 focus:outline-none"
                onclick={toggle}
            >
                <span class="header-text">{ title }</span>
                <svg
                    class={format!("w-3 h-3 transition-transform duration-300 accordion-arrow hosts-arrow {}", if *is_open { "rotate-180" } else { "rotate-0" })}
                    xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 10 6"
                >
                    <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5 5 1 1 5"/>
                </svg>
            </button>
            if *is_open {
                if has_no_known_hosts {
                    <div class="mt-2 p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
                        <p class="text-gray-700 dark:text-gray-300">
                            {&i18n_no_hosts_found}
                            <a
                                href={host_url}
                                target="_blank"
                                class="text-blue-500 hover:text-blue-600 hover:underline"
                            >
                                {&i18n_add_hosts_here}
                            </a>
                        </p>
                    </div>
                } else {
                    <div class="flex space-x-4 mt-2">
                        { for hosts.iter().map(render_host) }
                    </div>
                }
            }
        </div>
    }
}
