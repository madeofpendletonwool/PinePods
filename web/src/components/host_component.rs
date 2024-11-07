use super::gen_components::LoadingModal;
use crate::components::context::AppState;
use crate::requests::people_req::{
    call_get_person_subscriptions, call_subscribe_to_person, call_unsubscribe_from_person,
};
use crate::requests::pod_req::{
    call_check_podcast, call_get_podcast_details, call_get_podcast_id, Person, Podcast,
    PodcastDetails, PodcastResponse,
};
use crate::requests::search_pods::call_get_person_info;
use crate::requests::search_pods::call_get_podcast_details_dynamic;
use futures::future::join_all;
use std::collections::HashMap;
use std::collections::HashSet;
use wasm_bindgen_futures::spawn_local;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew::Properties;
use yew::{function_component, html, use_effect_with, Callback, Html};
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[derive(Clone, PartialEq, Debug)]
pub struct Host {
    pub name: String,
    pub role: Option<String>,
    pub group: Option<String>,
    pub img: Option<String>,
    pub href: Option<String>,
    pub id: Option<i32>, // Changed to i32 to match Person struct
}

#[derive(Properties, PartialEq, Clone)]
pub struct HostDropdownProps {
    pub title: String,
    pub hosts: Vec<Person>,
    pub podcast_feed_url: String, // Add this to help create a unique identifier
    pub podcast_id: i32,
}

fn map_podcast_details_to_podcast(details: PodcastDetails) -> Podcast {
    Podcast {
        podcastid: details.podcastid,
        podcastname: details.podcastname,
        artworkurl: Some(details.artworkurl),
        description: Some(details.description),
        episodecount: details.episodecount,
        websiteurl: Some(details.websiteurl),
        feedurl: details.feedurl,
        author: Some(details.author),
        categories: details.categories,
        explicit: details.explicit,
        podcastindexid: details.podcastindexid,
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct HostItemProps {
    pub host: Person,
    pub podcast_feed_url: String,
    pub subscribed_hosts: HashMap<String, Vec<i32>>,
    pub podcast_id: i32,
    pub on_subscribe_toggle: Callback<Person>,
    pub on_host_click: Callback<(MouseEvent)>,
}

#[function_component(HostItem)]
fn host_item(props: &HostItemProps) -> Html {
    let HostItemProps {
        host,
        podcast_feed_url,
        subscribed_hosts,
        podcast_id,
        on_subscribe_toggle,
        on_host_click,
    } = props;

    let is_subscribed = subscribed_hosts
        .get(&host.name)
        .map_or(false, |ids| ids.contains(podcast_id));

    let on_subscribe_click = {
        let host = host.clone();
        let on_subscribe_toggle = on_subscribe_toggle.clone();
        Callback::from(move |_| {
            on_subscribe_toggle.emit(host.clone());
        })
    };

    html! {
        <div class="flex flex-col items-center">
            <div class="flex flex-col items-center cursor-pointer" onclick={on_host_click.clone()}>
                { if let Some(img) = &host.img {
                    html! { <img src={img.clone()} alt={host.name.clone()} class="w-12 h-12 rounded-full" /> }
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
                { if is_subscribed { "Unsubscribe" } else { "Subscribe" } }
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
    }: &HostDropdownProps,
) -> Html {
    let (search_state, _search_dispatch) = use_store::<AppState>();
    let subscribed_hosts = use_state(|| HashMap::<String, Vec<i32>>::new());
    let api_key = search_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let server_name = search_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let api_url = search_state
        .server_details
        .as_ref()
        .map(|ud| ud.api_url.clone());
    let user_id = search_state
        .user_details
        .as_ref()
        .map(|ud| ud.UserID.clone());
    let is_open = use_state(|| false);
    let toggle = {
        let is_open = is_open.clone();
        Callback::from(move |_| is_open.set(!*is_open))
    };

    let history = BrowserHistory::new();

    {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let subscribed_hosts = subscribed_hosts.clone();
        let podcast_id = *podcast_id;

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
                                for sub in subs {
                                    let associated_podcasts = sub
                                        .associatedpodcasts
                                        .unwrap_or_default()
                                        .split(',')
                                        .filter_map(|s| s.parse::<i32>().ok())
                                        .collect::<Vec<i32>>();
                                    sub_map.insert(sub.name, associated_podcasts);
                                }
                                subscribed_hosts.set(sub_map);
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

    let arrow_rotation_class = if *is_open { "rotate-180" } else { "rotate-0" };

    let loading_modal_visible = use_state(|| false);
    let loading_name = use_state(|| String::new());

    let render_host = {
        let subscribed_hosts = subscribed_hosts.clone();
        let podcast_feed_url = podcast_feed_url.clone();
        let _search_dispatch = _search_dispatch.clone();
        let history = history.clone();
        let search_state = search_state.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let api_url = api_url.clone();
        let user_id = user_id.clone();
        let loading_modal_visible = loading_modal_visible.clone();
        let loading_name = loading_name.clone();
        move |host: &Person| {
            let host_name = host.name.clone();
            let host_id = host.id.unwrap_or(0);
            // let is_subscribed = (*subscribed_hosts).contains(&host_name);
            let composite_key = format!("{}:{}", host_name, podcast_feed_url);
            let is_subscribed = (*subscribed_hosts)
                .get(&host_name)
                .map_or(false, |podcasts| podcasts.contains(podcast_id));
            let history_clone = history.clone();

            let on_host_click = {
                let dispatch_clone = _search_dispatch.clone();
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let api_url = api_url.clone();
                let user_id = user_id.clone();
                let host_name = host_name.clone();
                let history = history_clone.clone();
                let search_state_call = search_state.clone();
                let loading_modal_visible = loading_modal_visible.clone();
                let loading_name = loading_name.clone();

                Callback::from(move |_: MouseEvent| {
                    let hostname = host_name.clone();
                    let api_url = api_url.clone();
                    let api_key = api_key.clone();
                    let server_name = server_name.clone();
                    let search_state = search_state_call.clone();
                    let dispatch = dispatch_clone.clone();
                    let history = history.clone();
                    let loading_modal_visible = loading_modal_visible.clone();
                    let loading_name = loading_name.clone();
                    loading_name.set(hostname.clone());
                    loading_modal_visible.set(true);

                    wasm_bindgen_futures::spawn_local(async move {
                        let target_url = format!("/person/{}", hostname);

                        // Fetch person info
                        if let Ok(person_search_result) = call_get_person_info(
                            &hostname,
                            &api_url.unwrap(),
                            &api_key.clone().unwrap().unwrap(),
                        )
                        .await
                        {
                            // Extract unique podcast feeds
                            let unique_feeds: HashSet<_> = person_search_result
                                .items
                                .iter()
                                .map(|item| (item.feedTitle.clone(), item.feedUrl.clone()))
                                .collect();

                            let podcast_futures: Vec<_> = unique_feeds
                                .into_iter()
                                .map(|(feed_title, feed_url)| {
                                    let server_name = server_name.clone();
                                    let api_key = api_key.clone();
                                    let user_id = user_id;
                                    let search_state = search_state.clone();

                                    async move {
                                        web_sys::console::log_1(
                                            &format!(
                                                "Checking podcast: {:?} - {:?}",
                                                feed_title, feed_url
                                            )
                                            .into(),
                                        );
                                        let podcast_exists = call_check_podcast(
                                            &server_name.clone().unwrap(),
                                            &api_key.clone().unwrap().unwrap(),
                                            user_id.unwrap(),
                                            &feed_title.clone().unwrap_or_default(),
                                            &feed_url.clone().unwrap_or_default(),
                                        )
                                        .await
                                        .unwrap_or_default()
                                        .exists;

                                        if podcast_exists {
                                            web_sys::console::log_1(
                                                &format!(
                                                    "Podcast exists: {:?} - {:?}",
                                                    feed_title, feed_url
                                                )
                                                .into(),
                                            );
                                            if let Ok(podcast_id) = call_get_podcast_id(
                                                &server_name.clone().unwrap(),
                                                &api_key.clone().unwrap(),
                                                &search_state.user_details.as_ref().unwrap().UserID,
                                                &feed_url.unwrap_or_default(),
                                                &feed_title.clone().unwrap_or_default(),
                                            )
                                            .await
                                            {
                                                if let Ok(podcast_details) =
                                                    call_get_podcast_details(
                                                        &server_name.clone().unwrap(),
                                                        &api_key.clone().unwrap().unwrap(),
                                                        search_state
                                                            .user_details
                                                            .as_ref()
                                                            .unwrap()
                                                            .UserID,
                                                        &podcast_id,
                                                    )
                                                    .await
                                                {
                                                    return Some(map_podcast_details_to_podcast(
                                                        podcast_details,
                                                    ));
                                                }
                                            }
                                        } else {
                                            web_sys::console::log_1(
                                                &format!(
                                                    "Podcast does not exist: {:?} - {:?}",
                                                    feed_title, feed_url
                                                )
                                                .into(),
                                            );
                                            if let Ok(clicked_feed_url) =
                                                call_get_podcast_details_dynamic(
                                                    &server_name.clone().unwrap(),
                                                    &api_key.clone().unwrap().unwrap(),
                                                    user_id.unwrap(),
                                                    &feed_title.clone().unwrap_or_default(),
                                                    &feed_url.clone().unwrap_or_default(),
                                                    false,
                                                    Some(true),
                                                )
                                                .await
                                            {
                                                web_sys::console::log_1(
                                                    &format!(
                                                        "Fetched Podcast Episode Count: {}",
                                                        clicked_feed_url.podcast_episode_count
                                                    )
                                                    .into(),
                                                );
                                                use rand::Rng;

                                                fn generate_monster_id() -> i32 {
                                                    let mut rng = rand::thread_rng();
                                                    1_000_000_000
                                                        + rng.gen_range(0..1_000_000_000) as i32
                                                }
                                                let unique_id = generate_monster_id();
                                                return Some(Podcast {
                                                    podcastid: unique_id,
                                                    podcastname: clicked_feed_url.podcast_title,
                                                    artworkurl: Some(
                                                        clicked_feed_url.podcast_artwork,
                                                    ),
                                                    description: Some(
                                                        clicked_feed_url.podcast_description,
                                                    ),
                                                    episodecount: clicked_feed_url
                                                        .podcast_episode_count,
                                                    websiteurl: Some(clicked_feed_url.podcast_link),
                                                    feedurl: clicked_feed_url.podcast_url,
                                                    author: Some(clicked_feed_url.podcast_author),
                                                    categories: clicked_feed_url
                                                        .podcast_categories
                                                        .map(|cat_map| {
                                                            cat_map
                                                                .values()
                                                                .cloned()
                                                                .collect::<Vec<_>>()
                                                                .join(", ")
                                                        })
                                                        .unwrap_or_else(|| "{}".to_string()),
                                                    explicit: clicked_feed_url.podcast_explicit,
                                                    podcastindexid: clicked_feed_url
                                                        .podcast_index_id,
                                                });
                                            }
                                        }
                                        None
                                    }
                                })
                                .collect();

                            let fetched_podcasts: Vec<_> = join_all(podcast_futures)
                                .await
                                .into_iter()
                                .filter_map(|p| p)
                                .collect();

                            // Update the state once with all the fetched podcasts
                            dispatch.reduce_mut(move |state| {
                                state.podcast_feed_return = Some(PodcastResponse {
                                    pods: Some(fetched_podcasts),
                                });
                                state.people_feed_results = Some(person_search_result);
                                state.is_loading = Some(false);
                            });
                            loading_modal_visible.set(false);
                            history.push(target_url);
                        } else {
                            // Handle error
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some("Failed to fetch person info".to_string());
                                state.is_loading = Some(false);
                            });
                        }
                    });
                })
            };
            let pod_id = podcast_id.clone();

            let on_subscribe_toggle = {
                let api_key = api_key.clone();
                let server_name = server_name.clone();
                let user_id = user_id.clone();
                let subscribed_hosts = subscribed_hosts.clone();
                let podcast_id = *podcast_id;

                Callback::from(move |host: Person| {
                    let api_key = api_key.clone();
                    let server_name = server_name.clone();
                    let user_id = user_id.clone();
                    let subscribed_hosts = subscribed_hosts.clone();
                    let host_name = host.name.clone();
                    let host_id = host.id.unwrap_or(0);

                    // Update UI immediately
                    subscribed_hosts.set({
                        let mut hosts = (*subscribed_hosts).clone();
                        hosts
                            .entry(host_name.clone())
                            .and_modify(|podcasts| {
                                if podcasts.contains(&podcast_id) {
                                    podcasts.retain(|&id| id != podcast_id);
                                } else {
                                    podcasts.push(podcast_id);
                                }
                            })
                            .or_insert_with(|| vec![podcast_id]);
                        // Remove the entry if the podcast list is empty
                        if let Some(podcasts) = hosts.get(&host_name) {
                            if podcasts.is_empty() {
                                hosts.remove(&host_name);
                            }
                        }
                        hosts
                    });

                    // Make API call
                    spawn_local(async move {
                        let is_subscribed = (*subscribed_hosts)
                            .get(&host_name)
                            .map_or(false, |podcasts| podcasts.contains(&podcast_id));
                        if is_subscribed {
                            if let Err(e) = call_unsubscribe_from_person(
                                &server_name.unwrap(),
                                &api_key.unwrap().unwrap(),
                                user_id.unwrap(),
                                host_id,
                                host_name.clone(),
                            )
                            .await
                            {
                                web_sys::console::log_1(
                                    &format!("Failed to unsubscribe: {:?}", e).into(),
                                );
                                // Revert UI change on error
                                subscribed_hosts.set({
                                    let mut hosts = (*subscribed_hosts).clone();
                                    hosts
                                        .entry(host_name.clone())
                                        .or_insert_with(Vec::new)
                                        .push(podcast_id);
                                    hosts
                                });
                            }
                        } else {
                            if let Err(e) = call_subscribe_to_person(
                                &server_name.unwrap(),
                                &api_key.unwrap().unwrap(),
                                user_id.unwrap(),
                                host_id,
                                &host_name,
                                podcast_id,
                            )
                            .await
                            {
                                web_sys::console::log_1(
                                    &format!("Failed to subscribe: {:?}", e).into(),
                                );
                                // Revert UI change on error
                                subscribed_hosts.set({
                                    let mut hosts = (*subscribed_hosts).clone();
                                    if let Some(podcasts) = hosts.get_mut(&host_name) {
                                        podcasts.retain(|&id| id != podcast_id);
                                    }
                                    if hosts
                                        .get(&host_name)
                                        .map_or(false, |podcasts| podcasts.is_empty())
                                    {
                                        hosts.remove(&host_name);
                                    }
                                    hosts
                                });
                            }
                        }
                    });
                })
            };

            html! {
            <>
                <HostItem
                    host={host.clone()}
                    podcast_feed_url={podcast_feed_url.clone()}
                    subscribed_hosts={(*subscribed_hosts).clone()}
                    podcast_id={podcast_id}
                    on_subscribe_toggle={on_subscribe_toggle}
                    on_host_click={on_host_click}
                />
                <LoadingModal name={(*loading_name).clone()} is_visible={*loading_modal_visible} />
            </>
            }
        }
    };

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
                <div class="flex space-x-4 mt-2">
                    { for hosts.iter().map(render_host) }
                </div>
            }
        </div>
    }
}
