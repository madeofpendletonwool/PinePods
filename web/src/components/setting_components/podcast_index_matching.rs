use crate::components::context::AppState;
use crate::components::gen_components::FallbackImage;
use crate::requests::search_pods::{call_get_podcast_info, UnifiedPodcast};
use crate::requests::setting_reqs::{
    call_get_ignored_podcasts, call_get_unmatched_podcasts, call_ignore_podcast_index_id,
    call_update_podcast_index_id, UnmatchedPodcast,
};
use gloo_events::EventListener;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlElement;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(PodcastIndexMatching)]
pub fn podcast_index_matching() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());

    let unmatched_podcasts: UseStateHandle<Vec<UnmatchedPodcast>> = use_state(|| Vec::new());
    let ignored_podcasts: UseStateHandle<Vec<UnmatchedPodcast>> = use_state(|| Vec::new());
    let search_results: UseStateHandle<Vec<UnifiedPodcast>> = use_state(|| Vec::new());
    let selected_podcast_id: UseStateHandle<Option<i32>> = use_state(|| None);
    let is_searching = use_state(|| false);
    let loading = use_state(|| false);
    let show_ignored = use_state(|| false);
    let dropdown_ref = use_node_ref();

    let dispatch_effect = _dispatch.clone();

    // Fetch unmatched podcasts on component mount
    {
        let unmatched_podcasts = unmatched_podcasts.clone();
        let ignored_podcasts = ignored_podcasts.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let loading = loading.clone();

        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let unmatched_podcasts = unmatched_podcasts.clone();
                let ignored_podcasts = ignored_podcasts.clone();
                let loading = loading.clone();
                let api_key_cloned = api_key.clone().unwrap();
                let server_name_cloned = server_name.clone();

                spawn_local(async move {
                    if let (Some(api_key), Some(server_name), Some(user_id)) =
                        (api_key_cloned, server_name_cloned, user_id)
                    {
                        loading.set(true);

                        // Fetch unmatched podcasts
                        match call_get_unmatched_podcasts(
                            server_name.clone(),
                            api_key.clone(),
                            user_id,
                        )
                        .await
                        {
                            Ok(response) => {
                                unmatched_podcasts.set(response.podcasts);
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error fetching unmatched podcasts: {}", e).into(),
                                );
                            }
                        }

                        // Fetch ignored podcasts
                        match call_get_ignored_podcasts(server_name, api_key, user_id).await {
                            Ok(response) => {
                                ignored_podcasts.set(response.podcasts);
                            }
                            Err(e) => {
                                web_sys::console::log_1(
                                    &format!("Error fetching ignored podcasts: {}", e).into(),
                                );
                            }
                        }

                        loading.set(false);
                    }
                });
            },
        );
    }

    // Handle clicking outside dropdown to close it
    {
        let selected_podcast_id = selected_podcast_id.clone();
        let dropdown_ref = dropdown_ref.clone();

        use_effect_with(dropdown_ref.clone(), move |dropdown_ref| {
            let document = web_sys::window().unwrap().document().unwrap();
            let dropdown_element = dropdown_ref.cast::<HtmlElement>();

            let listener = EventListener::new(&document, "click", move |event| {
                if let Some(target) = event.target() {
                    if let Some(dropdown) = &dropdown_element {
                        if let Ok(node) = target.dyn_into::<web_sys::Node>() {
                            if !dropdown.contains(Some(&node)) {
                                selected_podcast_id.set(None);
                            }
                        }
                    }
                }
            });

            || drop(listener)
        });
    }

    let search_podcast_index = {
        let search_results = search_results.clone();
        let is_searching = is_searching.clone();
        let api_url = state.server_details.as_ref().map(|sd| sd.api_url.clone());
        let search_index = "podcast_index".to_string();

        Callback::from(move |podcast_name: String| {
            let search_results = search_results.clone();
            let is_searching = is_searching.clone();
            let api_url = api_url.clone().unwrap();
            let search_index = search_index.clone();

            spawn_local(async move {
                if let Some(api_url) = api_url {
                    is_searching.set(true);

                    match call_get_podcast_info(&podcast_name, &Some(api_url), &search_index).await
                    {
                        Ok(podcast_results) => {
                            let mut podcasts = Vec::new();

                            // Handle Podcast Index results
                            if let Some(feeds) = podcast_results.feeds {
                                for feed in feeds {
                                    let podcast = UnifiedPodcast::from(feed);
                                    podcasts.push(podcast);
                                }
                            }

                            // Handle iTunes results if using iTunes
                            if let Some(results) = podcast_results.results {
                                for result in results {
                                    let podcast = UnifiedPodcast::from(result);
                                    podcasts.push(podcast);
                                }
                            }

                            search_results.set(podcasts);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Error searching Podcast Index: {}", e).into(),
                            );
                        }
                    }

                    is_searching.set(false);
                }
            });
        })
    };

    let handle_podcast_click = |podcast_id: i32| {
        let selected_podcast_id = selected_podcast_id.clone();
        let search_results = search_results.clone();
        let search_podcast_index = search_podcast_index.clone();
        let unmatched_podcasts = unmatched_podcasts.clone();

        Callback::from(move |_: MouseEvent| {
            // Clear previous search results
            search_results.set(Vec::new());

            // Set selected podcast and trigger search
            selected_podcast_id.set(Some(podcast_id));

            // Find the podcast name and search
            if let Some(podcast) = (**unmatched_podcasts)
                .iter()
                .find(|p| p.podcast_id == podcast_id)
            {
                search_podcast_index.emit(podcast.podcast_name.clone());
            }
        })
    };

    let handle_match_selection = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let unmatched_podcasts = unmatched_podcasts.clone();
        let selected_podcast_id = selected_podcast_id.clone();
        let search_results = search_results.clone();
        let dispatch_effect = dispatch_effect.clone();

        Callback::from(move |(podcast_id, index_id): (i32, i32)| {
            let server_name = server_name.clone();
            let api_key = api_key.clone().unwrap();
            let user_id = user_id.clone();
            let unmatched_podcasts = unmatched_podcasts.clone();
            let selected_podcast_id = selected_podcast_id.clone();
            let search_results = search_results.clone();
            let dispatch_effect = dispatch_effect.clone();

            spawn_local(async move {
                if let (Some(server_name), Some(api_key), Some(user_id)) =
                    (server_name, api_key, user_id)
                {
                    match call_update_podcast_index_id(
                        server_name,
                        api_key,
                        user_id,
                        podcast_id,
                        index_id,
                    )
                    .await
                    {
                        Ok(_) => {
                            // Remove the matched podcast from the list
                            let updated_podcasts: Vec<UnmatchedPodcast> = (**unmatched_podcasts)
                                .iter()
                                .filter(|p| p.podcast_id != podcast_id)
                                .cloned()
                                .collect();
                            unmatched_podcasts.set(updated_podcasts);

                            // Clear selection and search results
                            selected_podcast_id.set(None);
                            search_results.set(Vec::new());

                            // Show success message
                            dispatch_effect.reduce_mut(|state| {
                                state.info_message = Some(
                                    "Podcast successfully matched to Podcast Index!".to_string(),
                                );
                            });
                        }
                        Err(e) => {
                            dispatch_effect.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error updating podcast index ID: {}", e));
                            });
                        }
                    }
                }
            });
        })
    };

    let handle_ignore_podcast = {
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let unmatched_podcasts = unmatched_podcasts.clone();
        let ignored_podcasts = ignored_podcasts.clone();
        let dispatch_effect = dispatch_effect.clone();

        Callback::from(move |(podcast_id, ignore): (i32, bool)| {
            let server_name = server_name.clone();
            let api_key = api_key.clone().unwrap();
            let user_id = user_id.clone();
            let unmatched_podcasts = unmatched_podcasts.clone();
            let ignored_podcasts = ignored_podcasts.clone();
            let dispatch_effect = dispatch_effect.clone();

            spawn_local(async move {
                if let (Some(server_name), Some(api_key), Some(user_id)) =
                    (server_name, api_key, user_id)
                {
                    match call_ignore_podcast_index_id(
                        server_name.clone(),
                        api_key.clone(),
                        user_id,
                        podcast_id,
                        ignore,
                    )
                    .await
                    {
                        Ok(_) => {
                            if ignore {
                                // Move podcast from unmatched to ignored
                                if let Some(podcast) = (**unmatched_podcasts)
                                    .iter()
                                    .find(|p| p.podcast_id == podcast_id)
                                    .cloned()
                                {
                                    let updated_unmatched: Vec<UnmatchedPodcast> =
                                        (**unmatched_podcasts)
                                            .iter()
                                            .filter(|p| p.podcast_id != podcast_id)
                                            .cloned()
                                            .collect();
                                    unmatched_podcasts.set(updated_unmatched);

                                    let mut updated_ignored = (**ignored_podcasts).to_vec();
                                    updated_ignored.push(podcast);
                                    ignored_podcasts.set(updated_ignored);
                                }

                                dispatch_effect.reduce_mut(|state| {
                                    state.info_message =
                                        Some("Podcast ignored from index matching".to_string());
                                });
                            } else {
                                // Move podcast from ignored to unmatched
                                if let Some(podcast) = (**ignored_podcasts)
                                    .iter()
                                    .find(|p| p.podcast_id == podcast_id)
                                    .cloned()
                                {
                                    let updated_ignored: Vec<UnmatchedPodcast> =
                                        (**ignored_podcasts)
                                            .iter()
                                            .filter(|p| p.podcast_id != podcast_id)
                                            .cloned()
                                            .collect();
                                    ignored_podcasts.set(updated_ignored);

                                    let mut updated_unmatched = (**unmatched_podcasts).to_vec();
                                    updated_unmatched.push(podcast);
                                    unmatched_podcasts.set(updated_unmatched);
                                }

                                dispatch_effect.reduce_mut(|state| {
                                    state.info_message =
                                        Some("Podcast restored to index matching".to_string());
                                });
                            }
                        }
                        Err(e) => {
                            dispatch_effect.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error updating podcast ignore status: {}", e));
                            });
                        }
                    }
                }
            });
        })
    };

    let toggle_ignored_view = {
        let show_ignored = show_ignored.clone();
        Callback::from(move |_: MouseEvent| {
            show_ignored.set(!*show_ignored);
        })
    };

    html! {
        <div class="settings_container" ref={dropdown_ref}>
            <h2 class="text_color_main font-bold text-lg mb-4">{"Match Podcasts to Podcast Index"}</h2>
            <p class="text_color_main mb-4">
                {"Podcasts imported from OPML files may not have Podcast Index IDs. Match them here to enable full functionality."}
            </p>
            <div class="import-box mb-6">
                <p class="item_container-text text-sm">
                    {"💡 Need to import podcasts? Visit Import OPML Settings to add podcasts from your favorite podcast apps."}
                </p>
            </div>

            if *loading {
                <div class="flex justify-center items-center p-8">
                    <div class="animate-spin rounded-full h-12 w-12 border-b-2 border-blue-500"></div>
                </div>
            } else if unmatched_podcasts.is_empty() {
                <div class="text-center p-8">
                    <p class="text_color_main text-lg">{"All podcasts are matched!"}</p>
                    <p class="text_color_main text-sm mt-2">{"No podcasts need Podcast Index matching."}</p>
                </div>
            } else {
                <div class="space-y-4">
                    {
                        unmatched_podcasts.iter().map(|podcast| {
                            let podcast_id = podcast.podcast_id;
                            let is_selected = *selected_podcast_id == Some(podcast_id);
                            let click_handler = handle_podcast_click(podcast_id);

                            html! {
                                <div key={podcast.podcast_id} class="border rounded-lg p-4 modal-container">
                                    <div
                                        class="flex items-start space-x-4 cursor-pointer hover:bg-opacity-80 transition-colors p-2 rounded"
                                        onclick={click_handler}
                                    >
                                        <FallbackImage
                                            src={podcast.artwork_url.clone().unwrap_or_else(|| "/static/assets/favicon.png".to_string())}
                                            alt={format!("Cover for {}", podcast.podcast_name)}
                                            class="w-16 h-16 rounded object-cover flex-shrink-0"
                                        />
                                        <div class="flex-grow min-w-0">
                                            <h3 class="text_color_main font-semibold text-base mb-1 truncate">
                                                {&podcast.podcast_name}
                                            </h3>
                                            {
                                                if let Some(author) = &podcast.author {
                                                    html! { <p class="text_color_accent text-sm mb-2">{author}</p> }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            <p class="text_color_accent text-xs">
                                                {"Click to search Podcast Index for matches"}
                                            </p>
                                        </div>
                                        <div class="flex-shrink-0 flex items-center space-x-2">
                                            <button
                                                class="px-3 py-1 text-xs bg-red-600 hover:bg-red-700 text-white rounded transition-colors"
                                                onclick={{
                                                    let handle_ignore_podcast = handle_ignore_podcast.clone();
                                                    let podcast_id = podcast_id;
                                                    Callback::from(move |e: MouseEvent| {
                                                        e.stop_propagation();
                                                        handle_ignore_podcast.emit((podcast_id, true));
                                                    })
                                                }}
                                            >
                                                {"Ignore"}
                                            </button>
                                            <i class="ph ph-magnifying-glass text-2xl text_color_accent"></i>
                                        </div>
                                    </div>

                                    if is_selected {
                                        <div class="absolute z-50 mt-1 w-full rounded-lg shadow-lg modal-container">
                                            if *is_searching {
                                                <div class="flex justify-center items-center p-4">
                                                    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500 mr-3"></div>
                                                    <span class="text_color_main">{"Searching Podcast Index..."}</span>
                                                </div>
                                            } else if search_results.is_empty() {
                                                <div class="text-center p-4">
                                                    <p class="text_color_accent">{"No matches found in Podcast Index"}</p>
                                                </div>
                                            } else {
                                                <div class="max-h-[400px] overflow-y-auto p-2 space-y-1">
                                                    {
                                                        search_results.iter().map(|result| {
                                                            let podcast_id = podcast_id;
                                                            let index_id = result.index_id as i32;
                                                            let match_handler = {
                                                                let handle_match_selection = handle_match_selection.clone();
                                                                Callback::from(move |_: MouseEvent| {
                                                                    handle_match_selection.emit((podcast_id, index_id));
                                                                })
                                                            };

                                                            html! {
                                                                <div
                                                                    key={result.id}
                                                                    onclick={match_handler}
                                                                    class={classes!(
                                                                        "flex",
                                                                        "items-center",
                                                                        "p-2",
                                                                        "rounded-lg",
                                                                        "cursor-pointer",
                                                                        "hover:bg-gray-700",
                                                                        "transition-colors"
                                                                    )}
                                                                >
                                                                    <FallbackImage
                                                                        src={result.image.clone()}
                                                                        alt={format!("Cover for {}", result.title)}
                                                                        class="w-12 h-12 rounded object-cover"
                                                                    />
                                                                    <div class="ml-3 flex-grow min-w-0">
                                                                        <div class="truncate text_color_main font-medium text-sm">
                                                                            {&result.title}
                                                                        </div>
                                                                        <div class="text_color_accent text-xs">{&result.author}</div>
                                                                        <div class="text_color_accent text-xs">
                                                                            {format!("Index ID: {}", result.index_id)}
                                                                        </div>
                                                                    </div>
                                                                    <i class="ph ph-check text-green-500 text-xl"></i>
                                                                </div>
                                                            }
                                                        }).collect::<Html>()
                                                    }
                                                </div>
                                            }
                                        </div>
                                    }
                                </div>
                            }
                        }).collect::<Html>()
                    }
                </div>
            }

            // Ignored podcasts section
            <div class="mt-8">
                <div class="flex items-center justify-between mb-4">
                    <h3 class="text_color_main font-semibold text-base">{"Ignored Podcasts"}</h3>
                    <button
                        class="text_color_accent hover:text_color_main transition-colors flex items-center space-x-1"
                        onclick={toggle_ignored_view}
                    >
                        <span class="text-sm">
                            {if *show_ignored { "Hide Ignored" } else { "Show Ignored" }}
                        </span>
                        <i class={if *show_ignored { "ph ph-chevron-up" } else { "ph ph-chevron-down" }}></i>
                    </button>
                </div>

                if *show_ignored {
                    if ignored_podcasts.is_empty() {
                        <div class="text-center p-4">
                            <p class="text_color_accent text-sm">{"No podcasts are ignored from index matching."}</p>
                        </div>
                    } else {
                        <div class="space-y-2">
                            {
                                ignored_podcasts.iter().map(|podcast| {
                                    let podcast_id = podcast.podcast_id;

                                    html! {
                                        <div key={podcast.podcast_id} class="border rounded-lg p-3 modal-container bg-opacity-50">
                                            <div class="flex items-center space-x-3">
                                                <FallbackImage
                                                    src={podcast.artwork_url.clone().unwrap_or_else(|| "/static/assets/favicon.png".to_string())}
                                                    alt={format!("Cover for {}", podcast.podcast_name)}
                                                    class="w-12 h-12 rounded object-cover flex-shrink-0 opacity-75"
                                                />
                                                <div class="flex-grow min-w-0">
                                                    <h4 class="text_color_main font-medium text-sm truncate opacity-75">
                                                        {&podcast.podcast_name}
                                                    </h4>
                                                    {
                                                        if let Some(author) = &podcast.author {
                                                            html! { <p class="text_color_accent text-xs opacity-75">{author}</p> }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>
                                                <button
                                                    class="px-3 py-1 text-xs bg-green-600 hover:bg-green-700 text-white rounded transition-colors"
                                                    onclick={{
                                                        let handle_ignore_podcast = handle_ignore_podcast.clone();
                                                        let podcast_id = podcast_id;
                                                        Callback::from(move |_: MouseEvent| {
                                                            handle_ignore_podcast.emit((podcast_id, false));
                                                        })
                                                    }}
                                                >
                                                    {"Restore"}
                                                </button>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Html>()
                            }
                        </div>
                    }
                }
            </div>
        </div>
    }
}
