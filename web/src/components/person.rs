use super::app_drawer::App_drawer;
use crate::components::audio::on_play_click;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::ExpandedDescriptions;
use crate::components::context::{AppState, UIState};
use crate::components::episodes_layout::AppStateMsg as EpisodeMsg;
use crate::components::episodes_layout::SafeHtml;
use crate::components::gen_components::on_shownotes_click;
use crate::components::gen_components::{Search_nav, UseScrollToTop};
use crate::components::gen_funcs::{
    format_datetime, format_time, match_date_format, parse_date, sanitize_html_with_blank_target,
    truncate_description,
};
use crate::requests::login_requests::use_check_authentication;
use crate::requests::pod_req::{
    call_add_podcast, call_remove_podcasts_name, PodcastValues, RemovePodcastValuesName,
};
use base64::{engine::general_purpose, Engine as _};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::prelude::wasm_bindgen;
use yew::prelude::*;
use yew_router::history::BrowserHistory;
use yew_router::prelude::*;
use yewdux::prelude::*;

enum AppStateMsg {
    // ... other messages ...
    RemovePodcast(i32), // Add this line
}

impl Reducer<AppState> for AppStateMsg {
    fn apply(self, mut state: Rc<AppState>) -> Rc<AppState> {
        let state_mut = Rc::make_mut(&mut state);

        match self {
            // ... other cases ...
            AppStateMsg::RemovePodcast(podcast_id) => {
                if let Some(podcasts) = &mut state_mut.podcast_feed_return {
                    podcasts.pods = Some(
                        podcasts
                            .pods
                            .as_ref()
                            .unwrap_or(&vec![])
                            .iter()
                            .filter(|p| p.podcastid != podcast_id)
                            .cloned()
                            .collect(),
                    );
                }
            }
        }

        state
    }
}

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

    let session_dispatch = dispatch.clone();
    let session_state = state.clone();
    use_effect_with((), move |_| {
        // Check if the page reload action has already occurred to prevent redundant execution
        if session_state.reload_occured.unwrap_or(false) {
            // Logic for the case where reload has already been processed
        } else {
            // Normal effect logic for handling page reload
            let window = web_sys::window().expect("no global `window` exists");
            let performance = window.performance().expect("should have performance");
            let navigation_type = performance.navigation().type_();

            if navigation_type == 1 {
                // 1 stands for reload
                let session_storage = window.session_storage().unwrap().unwrap();
                session_storage
                    .set_item("isAuthenticated", "false")
                    .unwrap();
            }

            // Always check authentication status
            let current_route = window.location().href().unwrap_or_default();
            use_check_authentication(session_dispatch.clone(), &current_route);

            // Mark that the page reload handling has occurred
            session_dispatch.reduce_mut(|state| {
                state.reload_occured = Some(true);
                state.clone() // Return the modified state
            });
        }

        || ()
    });
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
    let error_message = audio_state.error_message.clone();
    let info_message = audio_state.info_message.clone();
    let history = BrowserHistory::new();
    let history_clone = history.clone();

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
    let toggle_podcast = {
        let added_podcasts = added_podcasts_state.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let dispatch = dispatch.clone();
        let state_callback = state.clone();

        Callback::from(move |podcast_id: i32| {
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
                                    state.error_message =
                                        Some(format!("Error removing podcast: {:?}", e));
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
                        let categories_og_hashmap =
                            convert_categories_to_hashmap(podcast.categories.clone());

                        // Add podcast logic
                        let podcast_values = PodcastValues {
                            pod_title: podcast.podcastname.clone(),
                            pod_artwork: podcast.artworkurl.clone().unwrap(),
                            pod_author: podcast.author.clone().unwrap(),
                            categories: categories_og_hashmap,
                            pod_description: podcast.description.clone().unwrap(),
                            pod_episode_count: podcast.episodecount,
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
                                let new_podcast_id = podcast_info.podcast_id.unwrap();

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
                                    state.error_message =
                                        Some(format!("Error adding podcast: {:?}", e));
                                    state.is_loading = Some(false);
                                });
                            }
                        }
                    }
                });
            }
        })
    };

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                <div class="p-4">
                    <h1 class="text-2xl item_container-text font-bold text-center mb-6">{ format!("Podcasts and Episodes featuring {}", name) }</h1>
                    <div class="mb-8">
                        <h2 class="item_container-text text-xl font-semibold">{"Podcasts this person appears in"}</h2>
                        {
                            if let Some(podcasts) = state.podcast_feed_return.clone() {
                                let int_podcasts = podcasts.clone();
                                if let Some(pods) = int_podcasts.pods.clone() {
                                    if pods.is_empty() {
                                                                // Render "No Recent Episodes Found" if episodes list is empty
                                        html! {
                                    <div class="empty-episodes-container">
                                        <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                        <h1>{ "No Podcasts Found" }</h1>
                                        <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                                    </div>
                                        }
                                    } else {
                                    pods.into_iter().map(|podcast| {
                                        let is_added = if podcast.podcastid > 1_000_000_000 {
                                            false
                                        } else {
                                            added_podcasts_state.contains(&podcast.podcastid)
                                        };

                                        let button_text = if is_added { "delete" } else { "add" };

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
                                            podcast.podcastindexid.clone(),
                                            podcast.podcastname.clone(),
                                            podcast.feedurl.clone(),
                                            podcast.description.clone().unwrap_or_else(|| String::from("No Description Provided")),
                                            podcast.author.clone().unwrap_or_else(|| String::from("Unknown Author")),
                                            podcast.artworkurl.clone().unwrap_or_else(|| String::from("default_artwork_url.png")),
                                            podcast.explicit.clone(),
                                            podcast.episodecount.clone(),
                                            Some(podcast.categories.clone()),
                                            podcast.websiteurl.clone().unwrap_or_else(|| String::from("No Website Provided")),

                                            user_id.unwrap(),
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

                                        let description_class = if desc_expanded {
                                            "desc-expanded".to_string()
                                        } else {
                                            "desc-collapsed".to_string()
                                        };

                                        html! {
                                            <div>
                                            <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full">
                                                    <div class="flex flex-col w-auto object-cover pl-4">
                                                        <img
                                                            src={podcast.artworkurl.clone()}
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
                                                                        <SafeHtml html={podcast_description_clone.unwrap_or_default()} />
                                                                    </div>
                                                                </div>
                                                            }
                                                        }

                                                        <p class="item_container-text">{ format!("Episode Count: {}", &podcast.episodecount) }</p>
                                                    </div>
                                                    // <button class={"item-container-button border selector-button font-bold py-2 px-4 rounded-full self-center mr-8"} style="width: 60px; height: 60px;">
                                                    //     <span class="material-icons" onclick={toggle_delete.reform(move |_| podcast_id_loop)}>{"delete"}</span>
                                                    // </button>
                                                    <button class={"item-container-button border selector-button font-bold py-2 px-4 rounded-full self-center mr-8"} style="width: 60px; height: 60px;">
                                                        <span class="material-icons" onclick={onclick}>{ button_text }</span>
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
                                            <h1>{ "No Podcasts Found" }</h1>
                                            <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                                        </div>
                                    }
                                }
                            } else {
                                html! {
                                    <div class="empty-episodes-container">
                                        <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                        <h1>{ "No Podcasts Found" }</h1>
                                        <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                                    </div>
                                }
                            }
                        }
                    </div>
                    <h2 class="item_container-text text-xl font-semibold">{"Episodes this person appears in"}</h2>
                    {
                        if let Some(results) = &state.people_feed_results {
                            html! {
                                <div>
                                    { for results.items.iter().map(|episode| {
                                        let history_clone = history.clone();
                                        let dispatch = audio_dispatch.clone();
                                        let search_dispatch = _post_dispatch.clone();
                                        let search_state_clone = post_state.clone(); // Clone search_state

                                        // Clone the variables outside the closure
                                        let podcast_link_clone = episode.feedUrl.clone().unwrap_or_default();
                                        let podcast_title = episode.feedTitle.clone().unwrap_or_default();
                                        let episode_url_clone = episode.enclosureUrl.clone().unwrap_or_default();
                                        let episode_title_clone = episode.title.clone().unwrap_or_default();
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

                                        let on_play_click = on_play_click(
                                            episode_url_clone.clone(),
                                            episode_title_clone.clone(),
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
                                        );

                                        let description_class = if is_expanded {
                                            "desc-expanded".to_string()
                                        } else {
                                            "desc-collapsed".to_string()
                                        };

                                        let date_format = match_date_format(search_state_clone.date_format.as_deref());
                                        let datetime_str = if let Some(timestamp) = episode.datePublished {
                                            if let Some(dt) = DateTime::<Utc>::from_timestamp(timestamp, 0) {
                                                dt.format("%a, %d %b %Y %H:%M:%S %z").to_string()
                                            } else {
                                                "Invalid Date".to_string()
                                            }
                                        } else {
                                            "No Date Provided".to_string()
                                        };
                                        let datetime = parse_date(&datetime_str, &search_state_clone.user_tz);
                                        let format_release = format!("{}", format_datetime(&datetime, &search_state_clone.hour_preference, date_format));
                                        let formatted_duration = format_time(episode_duration_clone.into());
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
                                                    onclick={on_shownotes_click(history_clone.clone(), search_dispatch.clone(), Some(episode_id_for_ep_item), Some(podcast_link_clone), Some(shownotes_episode_url), Some(podcast_title), db_added)}
                                                    >{ &episode.title.clone().unwrap_or_default() }</p>
                                                    // <p class="text-gray-600">{ &episode.description.clone().unwrap_or_default() }</p>
                                                    {
                                                        html! {
                                                            <div class="item-description-text hidden md:block">
                                                                <div
                                                                    class={format!("item_container-text episode-description-container {}", description_class)}
                                                                    onclick={toggle_expanded}  // Make the description container clickable
                                                                >
                                                                    <SafeHtml html={description} />
                                                                </div>
                                                            </div>
                                                        }
                                                    }
                                                    <span class="episode-time-badge inline-flex items-center px-2.5 py-0.5 rounded me-2">
                                                        <svg class="time-icon w-2.5 h-2.5 me-1.5" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="currentColor" viewBox="0 0 20 20">
                                                            <path d="M10 0a10 10 0 1 0 10 10A10.011 10.011 0 0 0 10 0Zm3.982 13.982a1 1 0 0 1-1.414 0l-3.274-3.274A1.012 1.012 0 0 1 9 10V6a1 1 0 0 1 2 0v3.586l2.982 2.982a1 1 0 0 1 0 1.414Z"/>
                                                        </svg>
                                                        { format_release }
                                                    </span>
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
                                                                    class="item-container-button border-solid border selector-button font-bold py-2 px-4 rounded-full flex items-center justify-center md:w-16 md:h-16 w-10 h-10"
                                                                    onclick={on_play_click}
                                                                >
                                                                <span class="material-bonus-color material-icons large-material-icons md:text-6xl text-4xl">{"play_arrow"}</span>
                                                                </button>
                                                                // {
                                                                //     if podcast_added {
                                                                //         let page_type = "episode_layout".to_string();

                                                                //         let context_button = html! {
                                                                //             <ContextButton episode={boxed_episode} page_type={page_type.clone()} />
                                                                //         };


                                                                //         context_button

                                                                //     } else {
                                                                //         html! {}
                                                                //     }
                                                                // }
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

                {
                    if let Some(audio_props) = &audio_state.currently_playing {
                        html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} /> }
                    } else {
                        html! {}
                    }
                }
                // Conditional rendering for the error banner
                if let Some(error) = error_message {
                    <div class="error-snackbar">{ error }</div>
                }
                if let Some(info) = info_message {
                    <div class="info-snackbar">{ info }</div>
                }
            </div>
            <App_drawer />
        </>
    }
}
