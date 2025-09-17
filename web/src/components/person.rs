use super::app_drawer::App_drawer;
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::ExpandedDescriptions;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::AppStateMsg as EpisodeMsg;
use crate::components::gen_components::on_shownotes_click;
use crate::components::gen_components::{FallbackImage, Search_nav, UseScrollToTop};
use crate::components::gen_funcs::format_error_message;
use crate::components::gen_funcs::{
    format_datetime, format_time, match_date_format, parse_date, sanitize_html_with_blank_target,
    strip_images_from_html, truncate_description, unix_timestamp_to_datetime_string,
};
use crate::components::safehtml::SafeHtml;
use crate::requests::people_req::{
    call_get_person_subscriptions, call_subscribe_to_person, call_unsubscribe_from_person,
};
use crate::requests::pod_req::{
    call_add_podcast, call_remove_podcasts_name, Person as HostPerson, PodcastValues,
    RemovePodcastValuesName,
};
use base64::{engine::general_purpose, Engine as _};
use std::collections::HashMap;
use std::collections::HashSet;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::history::BrowserHistory;
use yew_router::prelude::*;
use yewdux::prelude::*;

fn generate_unique_id(podcast_id: Option<i32>, feed_url: &str) -> String {
    println!("Podcast ID: {:?}", podcast_id);
    match podcast_id {
        Some(id) if id != 0 => id.to_string(),
        _ => {
            // Adding a fallback in case of identical feed URLs or ID being zero
            let encoded_url = general_purpose::STANDARD.encode(feed_url);
            let sanitized_url = sanitize_for_css(&encoded_url);
            let fallback_unique = format!("{}", sanitized_url);
            println!("Generated fallback ID: {}", fallback_unique);
            fallback_unique
        }
    }
}

fn sanitize_for_css(input: &str) -> String {
    input.replace(|c: char| !c.is_alphanumeric() && c != '-', "_")
}

#[derive(Clone, PartialEq, Routable)]
pub enum Route {
    #[at("/person/:name")]
    Person { name: String },
    #[at("/")]
    Home,
}

#[derive(Clone, Properties, PartialEq)]
pub struct PersonProps {
    pub name: String,
}

#[function_component(Person)]
pub fn person(PersonProps { name }: &PersonProps) -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let person_ids = use_state(|| HashMap::<String, i32>::new());
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let is_expanded = use_state(|| true); // Start expanded by default
    let episodes_expanded = use_state(|| true); // Start expanded by default
    let subscribed_hosts = use_state(|| HashMap::<String, Vec<i32>>::new());
    let is_subscribed = use_state(|| false); // Add this state

    // Get the person data from the state
    let person_data = audio_state
        .podcast_people
        .as_ref()
        .and_then(|people| people.iter().find(|person| person.name == *name))
        .or_else(|| {
            audio_state
                .episode_page_people
                .as_ref()
                .and_then(|people| people.iter().find(|person| person.name == *name))
        });

    // Add debug logging to see which source we're getting the data from
    if person_data.is_some() {
        if audio_state
            .podcast_people
            .as_ref()
            .and_then(|people| people.iter().find(|person| person.name == *name))
            .is_some()
        {
            web_sys::console::log_1(&"Found person in podcast_people".into());
        } else {
            web_sys::console::log_1(&"Found person in episode_page_people".into());
        }
    }
    // Initialize the state for all podcasts
    let added_podcasts_state = use_state(|| {
        state
            .podcast_feed_return
            .as_ref()
            .map_or(HashSet::new(), |feed| {
                feed.pods.as_ref().map_or(HashSet::new(), |pods| {
                    pods.iter()
                        .filter(|podcast| podcast.podcastid != 0)
                        .map(|podcast| podcast.podcastid)
                        .collect::<HashSet<_>>()
                })
            })
    });

    {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let subscribed_hosts = subscribed_hosts.clone();
        let person_ids = person_ids.clone();
        let is_subscribed = is_subscribed.clone();
        let current_name = (*name).clone();

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
                                let mut found_subscription = false;

                                for sub in subs {
                                    let associated_podcasts = sub.associatedpodcasts.to_string()
                                        .split(',')
                                        .filter_map(|s| s.parse::<i32>().ok())
                                        .collect::<Vec<i32>>();

                                    // Check if this person is subscribed
                                    if sub.name == current_name && !associated_podcasts.is_empty() {
                                        found_subscription = true;
                                    }

                                    sub_map.insert(sub.name.clone(), associated_podcasts);
                                    pid_map.insert(sub.name, sub.personid);
                                }

                                subscribed_hosts.set(sub_map);
                                person_ids.set(pid_map);
                                is_subscribed.set(found_subscription);
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

    let toggle_podcast = {
        let added_podcasts = added_podcasts_state.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let dispatch = dispatch.clone();
        let state_callback = state.clone();

        Callback::from(move |podcast_id: i32| {
            dispatch.reduce_mut(|state| state.is_loading = Some(true));
            let added_pod_state = added_podcasts.clone();
            let server_name_callback = server_name.clone();
            let api_key_callback = api_key.clone();
            let user_id_callback = user_id.clone();
            let added_podcasts_callback = added_podcasts.clone();
            let dispatch_callback = dispatch.clone();
            // Extract the necessary data before the async block
            // let is_added = added_podcasts.contains(&podcast_id);
            // Extract the podcast ID from the event's dataset
            // Extract the podcast data
            let podcast = state_callback
                .podcast_feed_return
                .as_ref()
                .and_then(|feed| {
                    feed.pods
                        .as_ref()
                        .and_then(|pods| pods.iter().find(|pod| pod.podcastid == podcast_id))
                })
                .cloned();

            if let Some(mut podcast) = podcast {
                web_sys::console::log_1(&format!("Handling Podcast ID: {}", podcast_id).into());

                let is_added = podcast.podcastid <= 1_000_000_000
                    && added_podcasts.contains(&podcast.podcastid);

                let podcast_url = podcast.feedurl.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    if is_added {
                        // Remove podcast logic
                        let podcast_values = RemovePodcastValuesName {
                            podcast_name: podcast.podcastname.clone(),
                            podcast_url: podcast_url.clone(),
                            user_id: user_id.clone().unwrap(),
                        };
                        match call_remove_podcasts_name(
                            &server_name_callback.unwrap(),
                            &api_key_callback.unwrap(),
                            &podcast_values,
                        )
                        .await
                        {
                            Ok(_) => {
                                let mut new_set = (*added_podcasts_callback).clone();
                                new_set.remove(&podcast_id);
                                added_podcasts_callback.set(new_set);
                                dispatch_callback.reduce_mut(|state| {
                                    state.info_message =
                                        Some("Podcast successfully removed".to_string());
                                    state.is_loading = Some(false);
                                });
                            }
                            Err(e) => {
                                dispatch_callback.reduce_mut(|state| {
                                    let formatted_error = format_error_message(&e.to_string());
                                    state.error_message = Some(format!(
                                        "Error removing podcast: {:?}",
                                        formatted_error
                                    ));
                                    state.is_loading = Some(false);
                                });
                            }
                        }
                    } else {
                        fn convert_categories_to_hashmap(
                            categories: String,
                        ) -> HashMap<String, String> {
                            categories
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .map(|category| (category.clone(), category))
                                .collect()
                        }

                        // Assuming you have this inside the `toggle_podcast` closure:
                        let categories_og_hashmap = podcast.categories.clone().unwrap_or_default();

                        // Add podcast logic
                        let podcast_values = PodcastValues {
                            pod_title: podcast.podcastname.clone(),
                            pod_artwork: podcast.artworkurl.clone().unwrap(),
                            pod_author: podcast.author.clone().unwrap(),
                            categories: categories_og_hashmap,
                            pod_description: podcast.description.clone().unwrap(),
                            pod_episode_count: podcast.episodecount.clone().unwrap_or_else(|| 0),
                            pod_feed_url: podcast_url.clone(),
                            pod_website: podcast.websiteurl.clone().unwrap(),
                            pod_explicit: podcast.explicit,
                            user_id: user_id.clone().unwrap(),
                        };
                        match call_add_podcast(
                            &server_name_callback.unwrap(),
                            &api_key_callback.unwrap(),
                            user_id_callback.unwrap(),
                            &podcast_values,
                            Some(0),
                        )
                        .await
                        {
                            Ok(podcast_info) => {
                                let new_podcast_id = podcast_info.podcast_id;
                                // Update the podcast ID with the correct one from the DB
                                podcast.podcastid = new_podcast_id;
                                // Update Yewdux state
                                dispatch_callback.reduce_mut(|state| {
                                    if let Some(feed) = state.podcast_feed_return.as_mut() {
                                        if let Some(pods) = feed.pods.as_mut() {
                                            if let Some(existing_podcast) = pods
                                                .iter_mut()
                                                .find(|pod| pod.podcastid == podcast_id)
                                            {
                                                existing_podcast.podcastid = new_podcast_id;
                                            }
                                        }
                                    }
                                    state.info_message =
                                        Some("Podcast successfully added".to_string());
                                    state.is_loading = Some(false);
                                });
                                let mut new_set = (*added_podcasts_callback).clone();
                                new_set.insert(podcast.podcastid);
                                added_podcasts_callback.set(new_set.clone());
                                added_pod_state.set(new_set.clone());
                            }
                            Err(e) => {
                                dispatch_callback.reduce_mut(|state| {
                                    let formatted_error = format_error_message(&e.to_string());
                                    state.error_message = Some(format!(
                                        "Error adding podcast: {:?}",
                                        formatted_error
                                    ));
                                    state.is_loading = Some(false);
                                });
                            }
                        }
                    }
                });
            }
        })
    };
    let on_subscribe_toggle = {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let subscribed_hosts = subscribed_hosts.clone();
        let person_ids = person_ids.clone();
        let is_subscribed = is_subscribed.clone();

        Callback::from(move |person: HostPerson| {
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let user_id = user_id.clone();
            let subscribed_hosts = subscribed_hosts.clone();
            let host_name = person.name.clone();
            let host_img = person.img.clone();
            let host_id = person.id.unwrap_or(0);
            let person_ids = person_ids.clone();
            let podcast_id = 0;
            let is_subscribed = is_subscribed.clone();

            // Store current state before flipping it
            let current_subscribed = *is_subscribed;
            is_subscribed.set(!current_subscribed);

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

                if let Some(podcasts) = hosts.get(&host_name) {
                    if podcasts.is_empty() {
                        hosts.remove(&host_name);
                    }
                }
                hosts
            });

            spawn_local(async move {
                let id_to_use = (*person_ids).get(&host_name).copied().unwrap_or(host_id);

                // Use the stored state to determine action
                if current_subscribed {
                    if let Err(e) = call_unsubscribe_from_person(
                        &server_name.unwrap(),
                        &api_key.unwrap().unwrap(),
                        user_id.unwrap(),
                        id_to_use,
                        host_name.clone(),
                    )
                    .await
                    {
                        web_sys::console::log_1(&format!("Failed to unsubscribe: {:?}", e).into());
                        // Revert UI change on error
                        is_subscribed.set(true);
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
                            // Update person_ids with the new ID
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
                            is_subscribed.set(false);
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
                }
            });
        })
    };

    fn get_proxied_image_url(server_name: &str, original_url: &str) -> String {
        let proxied_url = format!(
            "{}/api/proxy/image?url={}",
            server_name,
            urlencoding::encode(original_url)
        );
        web_sys::console::log_1(&format!("Proxied URL: {}", proxied_url).into());
        proxied_url
    }

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                {
                    if let Some(person) = person_data {
                        web_sys::console::log_1(&format!("Image URL: {:?}", &person).into());
                        html! {
                            <div class="person-header bg-custom-light p-6 rounded-lg shadow-md mb-6">
                                <div class="flex items-center gap-6">
                                    // Image section with fallback
                                    <div class="w-24 h-24 rounded-full overflow-hidden flex-shrink-0">
                                        {
                                            if let Some(img_url) = &person.img {
                                                web_sys::console::log_1(&format!("Image URL: {}", img_url).into());
                                                let proxied_url = get_proxied_image_url(&server_name.clone().unwrap(), img_url);
                                                web_sys::console::log_1(&format!("Proxied URL: {}", proxied_url).into());
                                                html! {
                                                    <img
                                                        src={proxied_url}
                                                        alt={format!("{}'s profile", person.name)}
                                                        class="w-full h-full object-cover"
                                                    />
                                                }
                                            } else {
                                                html! {
                                                    <div class="w-full h-full bg-gray-300 flex items-center justify-center">
                                                        <i class="ph ph-user text-4xl text-gray-500"></i>
                                                    </div>
                                                }
                                            }
                                        }
                                    </div>

                                    // Person details
                                    <div class="flex-grow">
                                        <div class="flex items-center justify-between mb-4">
                                            <h1 class="text-2xl font-bold item_container-text">
                                                {&person.name}
                                            </h1>
                                            <button
                                                onclick={let person_clone = person.clone();
                                                    Callback::from(move |_| on_subscribe_toggle.emit(person_clone.clone()))}
                                                class={if *is_subscribed {
                                                    "px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600 transition-colors"
                                                } else {
                                                    "px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 transition-colors"
                                                }}
                                            >
                                                { if *is_subscribed { "Unsubscribe" } else { "Subscribe" } }
                                            </button>
                                        </div>

                                        <div class="flex flex-wrap gap-2 mb-2">
                                            {
                                                if let Some(role) = &person.role {
                                                    html! {
                                                        <span class="inline-block bg-blue-100 text-blue-800 text-sm px-2 py-1 rounded">
                                                            {role}
                                                        </span>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            {
                                                if let Some(group) = &person.group {
                                                    html! {
                                                        <span class="inline-block bg-green-100 text-green-800 text-sm px-2 py-1 rounded">
                                                            {group}
                                                        </span>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                        </div>

                                        {
                                            if let Some(description) = &person.description {
                                                html! {
                                                    <p class="text-sm item_container-text">
                                                        {description}
                                                    </p>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                </div>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="person-header bg-custom-light p-6 rounded-lg shadow-md mb-6">
                                <div class="flex items-center gap-6">
                                    <div class="w-24 h-24 rounded-full overflow-hidden flex-shrink-0">
                                        <div class="w-full h-full bg-gray-300 flex items-center justify-center">
                                            <i class="ph ph-user text-4xl text-gray-500"></i>
                                        </div>
                                    </div>
                                    <div class="flex-grow">
                                        <h1 class="text-2xl font-bold mb-2 item_container-text">
                                            {name}
                                        </h1>
                                    </div>
                                </div>
                            </div>
                        }
                    }
                }
                <div class="p-4">
                    <div class="mb-8">
                    <div class="flex justify-between items-center mb-4 cursor-pointer"
                         onclick={let is_expanded = is_expanded.clone();
                                 Callback::from(move |_| is_expanded.set(!*is_expanded))}>
                        <h2 class="item_container-text text-xl font-semibold">{"Podcasts this person appears in"}</h2>
                        <i class={classes!(
                            "ph",
                            "ph-caret-up",
                            "transition-transform",
                            "duration-300",
                            "item_container-text",
                            "text-2xl",
                            if *is_expanded { "rotate-180" } else { "rotate-0" }
                        )}></i>
                    </div>

                    // Content section with animation classes
                    <div class={classes!(
                        "transition-all",
                        "duration-300",
                        "overflow-hidden",
                        if *is_expanded { "max-h-full opacity-100" } else { "max-h-0 opacity-0" }
                    )}>
                        {

                            if let Some(podcasts) = state.podcast_feed_return.clone() {
                                let int_podcasts = podcasts.clone();
                                if let Some(pods) = int_podcasts.pods.clone() {
                                    if pods.is_empty() {
                                                                // Render "No Recent Episodes Found" if episodes list is empty
                                        html! {
                                    <div class="empty-episodes-container">
                                        <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                        <h1 class="page-subtitles">{ "No Podcasts Found" }</h1>
                                        <p class="page-paragraphs">{"This person doesn't seem to appear in any podcasts. Are you sure you got the right person?"}</p>
                                    </div>
                                        }
                                    } else {
                                    pods.into_iter().map(|podcast| {
                                        let is_added = if podcast.podcastid > 1_000_000_000 {
                                            false
                                        } else {
                                            added_podcasts_state.contains(&podcast.podcastid)
                                        };

                                        let button_text = if is_added { "ph ph-trash" } else { "ph ph-plus-circle" };

                                        let onclick = {
                                            let podcast_id = podcast.podcastid;
                                            let toggle_podcast = toggle_podcast.clone();
                                            Callback::from(move |_: MouseEvent| {
                                                toggle_podcast.emit(podcast_id);
                                            })
                                        };

                                        let api_key_iter = api_key.clone();
                                        let server_name_iter = server_name.clone().unwrap();
                                        let history = history_clone.clone();

                                        let dispatch = dispatch.clone();
                                        let podcast_description_clone = podcast.description.clone();

                                        let on_title_click = create_on_title_click(
                                            dispatch.clone(),
                                            server_name_iter,
                                            api_key_iter,
                                            &history,
                                            podcast.podcastindexid.clone().unwrap(),
                                            podcast.podcastname.clone(),
                                            podcast.feedurl.clone(),
                                            podcast.description.clone().unwrap_or_else(|| String::from("No Description Provided")),
                                            podcast.author.clone().unwrap_or_else(|| String::from("Unknown Author")),
                                            podcast.artworkurl.clone().unwrap_or_else(|| String::from("default_artwork_url.png")),
                                            podcast.explicit.clone(),
                                            podcast.episodecount.clone().unwrap_or_else(|| 0),
                                            podcast.categories.as_ref().map(|cats| cats.values().cloned().collect::<Vec<_>>().join(", ")),
                                            podcast.websiteurl.clone().unwrap_or_else(|| String::from("No Website Provided")),

                                            user_id.unwrap(),
                                            false,
                                        );

                                        let id_string = generate_unique_id(Some(podcast.podcastid.clone()), &podcast.feedurl.clone());
                                        let desc_expanded = desc_state.expanded_descriptions.contains(&id_string.clone());

                                        #[wasm_bindgen]
                                        extern "C" {
                                            #[wasm_bindgen(js_namespace = window)]
                                            fn toggleDescription(guid: &str, expanded: bool);
                                        }
                                        let toggle_expanded = {
                                            let desc_dispatch = desc_dispatch.clone();
                                            let episode_guid = id_string;
                                            // let episode_guid = podcast.podcastid.clone().to_string();

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
                                        let preview_description = strip_images_from_html(&podcast_description_clone.unwrap());


                                        let description_class = if desc_expanded {
                                            "desc-expanded".to_string()
                                        } else {
                                            "desc-collapsed".to_string()
                                        };

                                        html! {
                                            <div>
                                            <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full">
                                                    <div class="flex flex-col w-auto object-cover pl-4">
                                                        <FallbackImage
                                                            src={podcast.artworkurl.clone().unwrap()}
                                                            onclick={on_title_click.clone()}
                                                            alt={format!("Cover for {}", podcast.podcastname.clone())}
                                                            class="episode-image"
                                                        />
                                                    </div>
                                                    <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                                                        <p class="item_container-text episode-title font-semibold cursor-pointer" onclick={on_title_click}>
                                                            { &podcast.podcastname }
                                                        </p>
                                                        <hr class="my-2 border-t hidden md:block"/>
                                                        {
                                                            html! {
                                                                <div class="item-description-text hidden md:block">
                                                                    <div
                                                                        class={format!("item_container-text episode-description-container {}", description_class)}
                                                                        onclick={toggle_expanded}  // Make the description container clickable
                                                                        id={format!("desc-{}", podcast.podcastid)}
                                                                    >
                                                                        <SafeHtml html={preview_description} />
                                                                    </div>
                                                                </div>
                                                            }
                                                        }

                                                        <p class="item_container-text">{ format!("Episode Count: {}", &podcast.episodecount.unwrap_or_else(|| 0)) }</p>
                                                    </div>
                                                    <button class={"item-container-button selector-button font-bold py-2 px-4 rounded-full self-center mr-8"} style="width: 60px; height: 60px;">
                                                        <i class={classes!(
                                                            "ph",
                                                            button_text,
                                                            "text-4xl"
                                                        )} onclick={onclick}></i>
                                                    </button>

                                                </div>
                                            </div>
                                        }

                                    }).collect::<Html>()
                                    }
                                } else {
                                    html! {
                                        <div class="empty-episodes-container">
                                            <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                            <h1 class="page-subtitles">{ "No Podcasts Found" }</h1>
                                            <p class="page-paragraphs">{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                                        </div>
                                    }
                                }
                            } else {
                                html! {
                                    <div class="empty-episodes-container">
                                        <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                        <h1 class="page-subtitles">{ "No Podcasts Found" }</h1>
                                        <p class="page-paragraphs">{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                                    </div>
                                }
                            }
                        }
                        </div>
                    </div>
                    <div class="flex justify-between items-center mb-4 cursor-pointer"
                         onclick={let episodes_expanded = episodes_expanded.clone();
                                 Callback::from(move |_| episodes_expanded.set(!*episodes_expanded))}>
                        <h2 class="item_container-text text-xl font-semibold">{"Episodes this person appears in"}</h2>
                        <i class={classes!(
                            "ph",
                            "ph-caret-up",
                            "transition-transform",
                            "duration-300",
                            "item_container-text",
                            "text-2xl",
                            if *episodes_expanded { "rotate-180" } else { "rotate-0" }
                        )}></i>
                    </div>

                    // Episodes Content with animation
                    <div class={classes!(
                        "transition-all",
                        "duration-300",
                        "overflow-hidden",
                        if *episodes_expanded { "max-h-full opacity-100" } else { "max-h-0 opacity-0" }
                    )}>
                        {
                        if let Some(results) = &state.people_feed_results {
                            html! {
                                <div>
                                    { for results.items.iter().map(|episode| {
                                        let history_clone = history.clone();
                                        let dispatch = audio_dispatch.clone();
                                        let state = audio_state.clone();
                                        let search_dispatch = _post_dispatch.clone();
                                        let search_state_clone = post_state.clone(); // Clone search_state

                                        // Clone the variables outside the closure
                                        let podcast_link_clone = episode.feedUrl.clone().unwrap_or_default();
                                        let podcast_title = episode.feedTitle.clone().unwrap_or_default();
                                        let episode_url_clone = episode.enclosureUrl.clone().unwrap_or_default();
                                        let episode_title_clone = episode.title.clone().unwrap_or_default();
                                        let episode_description_clone = episode.description.clone().unwrap_or_default();
                                        let episode_pubdate_clone = episode.datePublished.clone().unwrap_or_default();
                                        let episode_artwork_clone = episode.feedImage.clone().unwrap_or_default();                                        let episode_duration_clone = episode.duration.clone().unwrap_or_default();

                                        let episode_id_clone = 0;
                                        let mut db_added = false;
                                        if episode_id_clone == 0 {
                                        } else {
                                            db_added = true;
                                        }
                                        // let episode_id_shownotes = episode_id_clone.clone();
                                        let server_name_play = server_name.clone();
                                        let user_id_play = user_id.clone();
                                        let api_key_play = api_key.clone();

                                        let is_current_episode = state
                                            .currently_playing
                                            .as_ref()
                                            .map_or(false, |current| {
                                                // Compare both title and URL for uniqueness since we don't have IDs
                                                current.title == episode.title.clone().unwrap_or_default() &&
                                                current.src == episode.enclosureUrl.clone().unwrap_or_default()
                                            });

                                        let is_playing = state.audio_playing.unwrap_or(false);

                                        let is_expanded = post_state.expanded_descriptions.contains(
                                            &episode.guid.clone().unwrap()
                                        );

                                        let sanitized_description = sanitize_html_with_blank_target(&episode.description.clone().unwrap_or_default());
                                        let (description, _is_truncated) = if is_expanded {
                                            (sanitized_description, false)
                                        } else {
                                            truncate_description(sanitized_description, 300)
                                        };

                                        let search_state_toggle = search_state_clone.clone();
                                        let toggle_expanded = {
                                            let search_dispatch_clone = search_dispatch.clone();
                                            let episode_guid = episode.guid.clone().unwrap();
                                            Callback::from(move |_: MouseEvent| {
                                                let guid_clone = episode_guid.clone();
                                                let search_dispatch_call = search_dispatch_clone.clone();

                                                if search_state_toggle.expanded_descriptions.contains(&guid_clone) {
                                                    search_dispatch_call.apply(EpisodeMsg::CollapseEpisode(guid_clone));
                                                } else {
                                                    search_dispatch_call.apply(EpisodeMsg::ExpandEpisode(guid_clone));
                                                }

                                            })
                                        };

                                        let formatted_date = unix_timestamp_to_datetime_string(episode_pubdate_clone);
                                        let date_format = match_date_format(search_state_clone.date_format.as_deref());
                                        let datetime = parse_date(&formatted_date, &search_state_clone.user_tz);
                                        let format_release = format!("{}", format_datetime(&datetime, &search_state_clone.hour_preference, date_format));
                                        let formatted_duration = format_time(episode_duration_clone.into());

                                        let on_play_pause = on_play_pause(
                                            episode_url_clone.clone(),
                                            episode_title_clone.clone(),
                                            episode_description_clone.clone(),
                                            formatted_duration.clone(),
                                            episode_artwork_clone.clone(),
                                            episode_duration_clone,
                                            episode_id_clone.clone(),
                                            Some(0),
                                            api_key_play.unwrap().unwrap(),
                                            user_id_play.unwrap(),
                                            server_name_play.unwrap(),
                                            dispatch.clone(),
                                            audio_state.clone(),
                                            None,
                                            Some(false),
                                        );

                                        let description_class = if is_expanded {
                                            "desc-expanded".to_string()
                                        } else {
                                            "desc-collapsed".to_string()
                                        };

                                        let episode_url_for_ep_item = episode_url_clone.clone();
                                        let episode_id_for_ep_item = 0;
                                        let shownotes_episode_url = episode_url_clone.clone();
                                        let should_show_buttons = !episode_url_for_ep_item.is_empty();
                                        html! {
                                            <div class="item-container flex items-center mb-4 shadow-md rounded-lg">
                                                <img
                                                    src={episode.feedImage.clone().unwrap_or_default()}
                                                    alt={format!("Cover for {}", &episode.title.clone().unwrap_or_default())}
                                                    class="episode-image"/>
                                                <div class="flex flex-col p-4 space-y-2 flex-grow md:w-7/12">
                                                    <p class="item_container-text episode-title font-semibold"
                                                    onclick={on_shownotes_click(history_clone.clone(), search_dispatch.clone(), Some(episode_id_for_ep_item), Some(podcast_link_clone), Some(shownotes_episode_url), Some(podcast_title), db_added, None, Some(false))}
                                                    >{ &episode.title.clone().unwrap_or_default() }</p>
                                                    // <p class="text-gray-600">{ &episode.description.clone().unwrap_or_default() }</p>
                                                    {
                                                        html! {
                                                            <div class="item-description-text hidden md:block">
                                                                <div
                                                                    class={format!("item_container-text episode-description-container line-clamp-2 {}", description_class)}
                                                                    onclick={toggle_expanded}  // Make the description container clickable
                                                                >
                                                                    <SafeHtml html={description} />
                                                                </div>
                                                            </div>
                                                        }
                                                    }
                                                    <div class="episode-time-badge-container" style="max-width: 100%; overflow: hidden;">
                                                        <span
                                                            class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2"
                                                            style="flex-grow: 0; flex-shrink: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;"
                                                        >
                                                            <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                                                <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                                                            </svg>
                                                            { format_release }
                                                        </span>
                                                    </div>
                                                    {
                                                            html! {
                                                                <span class="item_container-text">{ format!("{}", formatted_duration) }</span>
                                                            }
                                                    }
                                                </div>
                                                {
                                                    html! {
                                                        <div class="flex flex-col items-center h-full w-2/12 px-2 space-y-4 md:space-y-8 button-container" style="align-self: center;"> // Add align-self: center; heren medium and larger screens
                                                            if should_show_buttons {
                                                                <button
                                                                    class="item-container-button selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                                                    onclick={on_play_pause}
                                                                >
                                                                    {
                                                                        if is_current_episode && is_playing {
                                                                            html! { <i class="ph ph-pause-circle md:text-6xl text-4xl"></i> }
                                                                        } else {
                                                                            html! { <i class="ph ph-play-circle md:text-6xl text-4xl"></i> }
                                                                        }
                                                                    }
                                                                </button>
                                                            }
                                                        </div>
                                                    }
                                                }


                                            </div>
                                        }
                                    })}
                                </div>
                            }
                        } else {
                            html! {
                                <div class="empty-episodes-container" id="episode-container">
                                    <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                    <h1 class="page-subtitles">{ "No Episodes Found" }</h1>
                                    <p class="page-paragraphs">{"This podcast strangely doesn't have any episodes. Try a more mainstream one maybe?"}</p>
                                </div>
                            }
                        }
                    }
                    </div>
                </div>

                {
                    if let Some(audio_props) = &audio_state.currently_playing {
                        html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
                    } else {
                        html! {}
                    }
                }
            </div>
            <App_drawer />
        </>
    }
}
