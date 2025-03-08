use super::app_drawer::App_drawer;
use super::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, ExpandedDescriptions, PodcastState, UIState};
use crate::components::episodes_layout::SafeHtml;
use crate::requests::pod_req::{
    call_add_podcast, call_check_podcast, call_remove_podcasts_name, PodcastDetails, PodcastValues,
    RemovePodcastValuesName,
};
use crate::requests::search_pods::{call_parse_podcast_url, UnifiedPodcast};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use web_sys::MouseEvent;
use yew::prelude::*;
use yew::{function_component, html, Callback, Html};
use yew_router::history::{BrowserHistory, History};
use yewdux::use_store;

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct ClickedFeedURL {
    pub podcastid: i64,      // Changed from podcast_id
    pub podcastname: String, // Changed from podcast_title
    pub feedurl: String,     // Changed from podcast_url
    pub description: String, // Changed from podcast_description
    pub author: String,      // Changed from podcast_author
    pub artworkurl: String,  // Changed from podcast_artwork
    pub explicit: bool,      // Changed from podcast_explicit
    pub episodecount: i32,   // Changed from podcast_episode_count
    pub categories: Option<HashMap<String, String>>,
    pub websiteurl: String,  // Changed from podcast_link
    pub podcastindexid: i64, // Changed from podcast_index_id
    pub is_youtube: Option<bool>,
}

// Add this function to convert between the types
impl ClickedFeedURL {
    pub fn into_podcast_details(self) -> PodcastDetails {
        PodcastDetails {
            podcastid: self.podcastid as i32, // Convert i64 to i32
            podcastname: self.podcastname,
            artworkurl: self.artworkurl,
            author: self.author,
            categories: self
                .categories
                .map(|cats| cats.values().cloned().collect::<Vec<_>>().join(", "))
                .unwrap_or_default(), // Convert HashMap to comma-separated string
            description: self.description,
            episodecount: self.episodecount,
            feedurl: self.feedurl,
            websiteurl: self.websiteurl,
            explicit: self.explicit,
            userid: 0, // Default value since it's not in ClickedFeedURL
            podcastindexid: Some(self.podcastindexid),
            is_youtube: self.is_youtube.unwrap_or(false),
        }
    }
}

#[function_component(PodLayout)]
pub fn pod_layout() -> Html {
    // let dispatch = Dispatch::<AppState>::global();
    // let state: Rc<AppState> = dispatch.get();
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let search_results = state.search_results.clone();

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />
                <h1 class="item_container-text text-2xl font-bold my-4 center-text">{ "Podcast Search Results" }</h1>
                {
                    if let Some(results) = search_results {
                        let podcasts = results.feeds.as_ref().map_or_else(
                            || results.results.as_ref().map(|r| r.iter().map(|item| item.clone().into()).collect::<Vec<UnifiedPodcast>>()),
                            |f| Some(f.iter().map(|item| item.clone().into()).collect::<Vec<UnifiedPodcast>>())
                        );

                        if let Some(podcasts) = podcasts {
                            html! {
                                <div>
                                    { for podcasts.iter().map(|podcast| html! {
                                        <PodcastItem podcast={podcast.clone()} />
                                    })}
                                </div>
                            }
                        } else {
                            empty_message(
                                "No Podcast Search Results Found",
                                "Try searching again with a different set of keywords."
                            )
                        }
                    } else {
                        empty_message(
                            "No Podcast Search Results Found",
                            "Try searching again with a different set of keywords."
                        )
                    }
                }
                <App_drawer />
            </div>
            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
                } else {
                    html! {}
                }
            }
        </>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct PodcastProps {
    pub podcast: UnifiedPodcast, // Assuming Podcast is a struct that holds podcast details
}

// Assuming you have a PodcastItem component
#[function_component(PodcastItem)]
pub fn podcast_item(props: &PodcastProps) -> Html {
    // Local state to track if this particular podcast is added
    let podcast = props.podcast.clone();
    let podcast_url = podcast.url.clone();
    let podcast_title = podcast.title.clone();

    let (state, dispatch) = use_store::<AppState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let (podcast_state, podcast_dispatch) = use_store::<PodcastState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let history = BrowserHistory::new();
    let history_clone = history.clone();
    let podcast_state_button = podcast_state.clone();
    // let api_key_feed = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    // let server_name_feed = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    // Use a Set to track added podcast URLs for efficiency
    let set_loading = use_state(|| false);
    let toggle_key = api_key.clone();
    let toggle_name = server_name.clone();
    let effect_url = podcast_url.clone();
    let effect_title = podcast_title.clone();
    let effect_dispatch = podcast_dispatch.clone();

    // let api_key_mount = api_key.clone();
    use_effect_with(&(), move |_| {
        if let (Some(api_key), Some(user_id), Some(server_name)) =
            (toggle_key, user_id, toggle_name)
        {
            let podcast_dispatch = effect_dispatch.clone();

            wasm_bindgen_futures::spawn_local(async move {
                let added = call_check_podcast(
                    &server_name,
                    &api_key.unwrap(),
                    user_id,
                    &effect_title,
                    &effect_url,
                )
                .await
                .unwrap_or_default()
                .exists;

                podcast_dispatch.reduce_mut(|state| {
                    if added {
                        let mut new_set = state.added_podcast_urls.clone();
                        new_set.insert(effect_url);
                        state.added_podcast_urls = new_set;
                    }
                });
            });
        }
        || ()
    });

    {
        let podcast_state = podcast_state.clone();

        use_effect_with(podcast_state.clone(), move |_| || ());
    }

    let podcast_add = podcast.clone();

    let toggle_podcast = {
        let podcast_add = podcast_add.clone();
        let set_loading = set_loading.clone();
        let podcast_dispatch = podcast_dispatch.clone();

        let podcast_id_og = podcast_add.id.clone();
        let pod_title_og = podcast_add.title.clone();
        let pod_artwork_og = podcast_add.artwork.clone();
        let pod_author_og = podcast_add.author.clone();
        let categories_og = podcast_add.categories.unwrap_or_default().clone();
        let pod_description_og = podcast_add.description.clone();
        let pod_episode_count_og = podcast_add.episodeCount.clone();
        let pod_feed_url_og = podcast_add.url.clone();
        let pod_website_og = podcast_add.link.clone();
        let pod_explicit_og = podcast_add.explicit.clone();

        let api_key_clone = api_key.clone();
        let server_name_clone = server_name.clone();
        let user_id_clone = user_id.clone();

        let dispatch = dispatch.clone(); // Clone the dispatch for updating global state after removing
        let podcast_url = podcast.url.clone(); // The URL of the podcast to toggle
        let pod_title_og_clone = pod_title_og.clone();

        Callback::from(move |_: MouseEvent| {
            let pod_dis_call = podcast_dispatch.clone();
            let set_loading = set_loading.clone();
            // Create a new set from the current state for modifications.
            let user_id = user_id_clone.clone();
            let api_key = api_key_clone.clone();
            let server_name = server_name_clone.clone();

            let dispatch = dispatch.clone();
            let podcast_url = podcast_url.clone();
            set_loading.set(true);

            if podcast_state.added_podcast_urls.contains(&podcast_url) {
                // If the podcast was added, remove it from the set and call remove_podcast.
                // Call remove_podcast asynchronously.
                let pod_title_og = pod_title_og_clone.clone();
                let pod_feed_url_og = pod_feed_url_og.clone();
                let value_id = user_id.clone().unwrap();
                let podcast_url = podcast_url.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let pod_title = pod_title_og.clone();
                    let pod_feed_url = pod_feed_url_og.clone();
                    let podcast_url = podcast_url.clone();
                    let podcast_values = RemovePodcastValuesName {
                        podcast_name: pod_title,
                        podcast_url: pod_feed_url,
                        user_id: value_id,
                    };
                    match call_remove_podcasts_name(
                        &server_name.unwrap(),
                        &api_key.unwrap(),
                        &podcast_values,
                    )
                    .await
                    {
                        Ok(_) => {
                            // If successful, update the state to remove the podcast
                            pod_dis_call.reduce_mut(|state| {
                                state.added_podcast_urls.remove(&podcast_url);
                            });
                            dispatch.reduce_mut(|state| {
                                state.info_message =
                                    Some("Podcast successfully removed".to_string());
                            });
                            set_loading.set(false);
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error removing podcast: {:?}", e));
                            });
                            set_loading.set(false);
                        }
                    }
                });
            } else {
                // If the podcast was not added, add it to the set and call add_podcast.
                let podcast_id_og = podcast_id_og.clone();
                let pod_title_og = pod_title_og.clone();
                let pod_artwork_og = pod_artwork_og.clone();
                let pod_author_og = pod_author_og.clone();
                let categories_og = categories_og.clone();
                let pod_description_og = pod_description_og.clone();
                let pod_episode_count_og = pod_episode_count_og.clone();
                let pod_feed_url_og = pod_feed_url_og.clone();
                let pod_website_og = pod_website_og.clone();
                let pod_explicit_og = pod_explicit_og.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let pod_id = Some(podcast_id_og.clone());
                    let pod_title = pod_title_og.clone();
                    let pod_artwork = pod_artwork_og.clone();
                    let pod_author = pod_author_og.clone();
                    let categories = categories_og.clone();
                    let pod_description = pod_description_og.clone();
                    let pod_episode_count = pod_episode_count_og.clone();
                    let pod_feed_url = pod_feed_url_og.clone();
                    let pod_website = pod_website_og.clone();
                    let pod_explicit = pod_explicit_og.clone();
                    let value_id = user_id.clone().unwrap();
                    let podcast_values = PodcastValues {
                        pod_title,
                        pod_artwork,
                        pod_author,
                        categories,
                        pod_description,
                        pod_episode_count,
                        pod_feed_url,
                        pod_website,
                        pod_explicit,
                        user_id: value_id,
                    };
                    match call_add_podcast(
                        &server_name.unwrap(),
                        &api_key.unwrap(),
                        user_id.unwrap(),
                        &podcast_values,
                        pod_id,
                    )
                    .await
                    {
                        Ok(_) => {
                            let podcast_url = podcast_url.clone();
                            pod_dis_call.reduce_mut(|state| {
                                let mut new_set = state.added_podcast_urls.clone();
                                new_set.insert(podcast_url.clone());
                                state.added_podcast_urls = new_set;

                                // Set loading to false inside the same reducer to keep it atomic
                                let set_loading = set_loading.clone();
                                set_loading.set(false);
                            });
                        }
                        Err(e) => {
                            dispatch.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error adding podcast: {:?}", e));
                            });
                            set_loading.set(false);
                        }
                    }
                });
            }
        })
    };

    let podcast_id_clone = podcast.id.clone();
    let podcast_index_clone = podcast.index_id.clone();
    let podcast_title_clone = podcast.title.clone();
    let podcast_url_clone = podcast.url.clone();
    let podcast_description_clone = podcast.description.clone();
    let podcast_author_clone = podcast.author.clone();
    let podcast_artwork_clone = podcast.artwork.clone();
    let podcast_explicit_clone = podcast.explicit.clone();
    let podcast_episode_count_clone = podcast.episodeCount.clone();
    let podcast_categories_clone = podcast.categories.clone();
    let podcast_link_clone = podcast.link.clone();
    let history = history_clone.clone();
    let button_text = {
        if podcast_state_button
            .added_podcast_urls
            .contains(&podcast.url)
        {
            "trash"
        } else {
            "plus-circle"
        }
    };

    let on_title_click = {
        let dispatch = dispatch.clone();
        let history = history.clone(); // Clone history for use inside the closure

        Callback::from(move |e: MouseEvent| {
            dispatch.reduce_mut(|state| state.is_loading = Some(true));
            let server_name_click = server_name.clone();
            let api_key_click = api_key.clone();
            let podcast_id = podcast_id_clone.clone();
            let podcast_title = podcast_title_clone.clone();
            let podcast_url = podcast_url_clone.clone();
            let podcast_description = podcast_description_clone.clone();
            let podcast_author = podcast_author_clone.clone();
            let podcast_artwork = podcast_artwork_clone.clone();
            let podcast_explicit = podcast_explicit_clone.clone();
            let podcast_episode_count = podcast_episode_count_clone.clone();
            let podcast_categories = podcast_categories_clone.clone();
            let podcast_link = podcast_link_clone.clone();
            let podcast_index_id = podcast_index_clone.clone();
            e.prevent_default(); // Prevent the default anchor behavior
            let podcast_values = ClickedFeedURL {
                podcastid: podcast_id,
                podcastname: podcast_title,
                feedurl: podcast_url.clone(),
                description: podcast_description,
                author: podcast_author,
                artworkurl: podcast_artwork,
                explicit: podcast_explicit,
                episodecount: podcast_episode_count,
                categories: podcast_categories,
                websiteurl: podcast_link,
                podcastindexid: podcast_index_id,
                is_youtube: Some(false),
            };
            let dispatch = dispatch.clone();
            let history = history.clone(); // Clone again for use inside async block
            wasm_bindgen_futures::spawn_local(async move {
                match call_parse_podcast_url(
                    server_name_click.unwrap(),
                    &api_key_click.unwrap(),
                    &podcast_url,
                )
                .await
                {
                    Ok(podcast_feed_results) => {
                        dispatch.reduce_mut(move |state| {
                            state.podcast_feed_results = Some(podcast_feed_results);
                            state.clicked_podcast_info = Some(podcast_values);
                        });
                        dispatch.reduce_mut(|state| state.is_loading = Some(false));
                        history.push("/episode_layout"); // Navigate to episode_layout
                    }
                    Err(_e) => {
                        // web_sys::console::log_1(&format!("Error: {}", e).into());
                    }
                }
            });
        })
    };

    let id_string = &podcast.id.clone().to_string();
    let desc_expanded = desc_state.expanded_descriptions.contains(id_string);
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = window)]
        fn toggleDescription(guid: &str, expanded: bool);
    }

    let toggle_expanded = {
        let desc_dispatch = desc_dispatch.clone();
        let episode_guid = podcast.id.clone().to_string();

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
    let podcast_description_clone = podcast.description.clone();

    let description_class = if desc_expanded {
        "desc-expanded".to_string()
    } else {
        "desc-collapsed".to_string()
    };

    html! {
        <div>
            {
                html! {
                    <div class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full">
                        <div class="flex flex-col w-auto object-cover pl-4">
                            <FallbackImage
                                src={podcast.image.clone()}
                                onclick={on_title_click.clone()}
                                alt={format!("Cover for {}", podcast.title.clone())}
                                class="object-cover align-top-cover w-full item-container img"
                            />
                        </div>
                        <div class="flex items-start flex-col p-4 space-y-2 w-11/12">
                            <p class="item_container-text text-xl font-semibold cursor-pointer" onclick={on_title_click.clone()}>
                            { &podcast.title } </p>
                            // <p class="item_container-text">{ &podcast.description }</p>
                            {
                                html! {
                                    <div class="item-description-text hidden md:block">
                                        <div
                                            class={format!("item_container-text episode-description-container {}", description_class)}
                                            onclick={toggle_expanded}  // Make the description container clickable
                                            id={format!("desc-{}", podcast.id)}
                                        >
                                            <SafeHtml html={podcast_description_clone} />
                                        </div>
                                    </div>
                                }
                            }

                            <p class="header-text">{ format!("Episode Count: {}", &podcast.episodeCount) }</p>
                        </div>
                        <button
                            class="item-container-button selector-button font-bold rounded-full self-center mr-8 flex items-center justify-center"
                            style="width: 180px; height: 180px;"
                            onclick={toggle_podcast}
                            disabled={*set_loading}
                        >
                            {
                                if *set_loading {
                                    html! { <i class="ph ph-spinner-ball animate-spin text-4xl"></i> }
                                } else {
                                    html! { <i class={format!("ph ph-{} text-4xl", button_text)}></i> }
                                }
                            }
                        </button>
                    </div>
                }
            }
        </div>
    }
}
