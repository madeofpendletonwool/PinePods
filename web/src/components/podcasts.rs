use super::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, ExpandedDescriptions, FilterState, UIState};
use crate::components::episodes_layout::SafeHtml;
use crate::components::gen_components::{Search_nav, UseScrollToTop};
use crate::requests::login_requests::use_check_authentication;
use crate::requests::pod_req;
use crate::requests::pod_req::Podcast;
use crate::requests::pod_req::{call_remove_podcasts, PodcastResponse, RemovePodcastValues};
use crate::requests::setting_reqs::call_add_custom_feed;
use gloo_timers::callback::Timeout;
use serde::Deserialize;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlInputElement;
use yew::prelude::*;
use yew::{function_component, html, Html};
use yew_router::history::BrowserHistory;
use yewdux::prelude::*;

// Add this enum to define the layout options
#[derive(Clone, PartialEq, Debug, Deserialize, Default)]
pub enum PodcastLayout {
    #[default]
    List,
    Grid,
}

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

fn render_layout_toggle(
    dispatch: Dispatch<AppState>,
    current_layout: Option<PodcastLayout>,
) -> Html {
    let onclick = dispatch.reduce_mut_callback(|state| {
        state.podcast_layout = match state.podcast_layout {
            Some(PodcastLayout::List) => Some(PodcastLayout::Grid),
            Some(PodcastLayout::Grid) => Some(PodcastLayout::List),
            None => Some(PodcastLayout::Grid),
        };
    });

    html! {
        <button
            class="download-button font-bold py-2 px-4 rounded inline-flex items-center"
            onclick={onclick}
        >
            <span class="material-icons icon-space">
                {match current_layout {
                    Some(PodcastLayout::List) => "grid_view",
                    Some(PodcastLayout::Grid) => "view_list",
                    None => "view_list",
                }}
            </span>
            <span class="text-lg">{"Layout"}</span>
        </button>
    }
}

fn render_podcasts(
    podcasts: &[Podcast],
    layout: Option<PodcastLayout>,
    dispatch: Dispatch<AppState>,
    history: &BrowserHistory,
    api_key: Option<Option<String>>,
    server_name: Option<String>,
    user_id: Option<i32>,
    desc_state: Rc<ExpandedDescriptions>,
    desc_dispatch: Dispatch<ExpandedDescriptions>,
    toggle_delete: Callback<i32>,
) -> Html {
    match layout {
        None | Some(PodcastLayout::List) => {
            html! {
                <div>
                    {podcasts.iter().map(|podcast| {
                        let api_key_iter = api_key.clone();
                        let server_name_iter = server_name.clone().unwrap();
                        let history_clone = history.clone();

                        let dispatch_clone = dispatch.clone();
                        let podcast_id_loop = podcast.podcastid.clone();
                        let podcast_description_clone = podcast.description.clone();

                        let on_title_click = create_on_title_click(
                            dispatch_clone.clone(),
                            server_name_iter,
                            api_key_iter,
                            &history_clone,
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

                        let id_string = &podcast.podcastid.clone().to_string();
                        let desc_expanded = desc_state.expanded_descriptions.contains(id_string);
                        #[wasm_bindgen]
                        extern "C" {
                            #[wasm_bindgen(js_namespace = window)]
                            fn toggleDescription(guid: &str, expanded: bool);
                        }
                        let toggle_expanded = {
                            let desc_dispatch = desc_dispatch.clone();
                            let episode_guid = podcast.podcastid.clone().to_string();

                            Callback::from(move |_: MouseEvent| {
                                let guid = episode_guid.clone();
                                desc_dispatch.reduce_mut(move |state| {
                                    if state.expanded_descriptions.contains(&guid) {
                                        state.expanded_descriptions.remove(&guid);
                                        toggleDescription(&guid, false);
                                    } else {
                                        state.expanded_descriptions.insert(guid.clone());
                                        toggleDescription(&guid, true);
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
                                    <div class="item-description-text hidden md:block">
                                        <div
                                            class={format!("item_container-text episode-description-container {}", description_class)}
                                            onclick={toggle_expanded}
                                            id={format!("desc-{}", podcast.podcastid)}
                                        >
                                            <SafeHtml html={podcast_description_clone.unwrap_or_default()} />
                                        </div>
                                    </div>
                                    <p class="item_container-text">{ format!("Episode Count: {}", &podcast.episodecount) }</p>
                                </div>
                                <button
                                    class={"item-container-button border selector-button font-bold py-2 px-4 rounded-full self-center mr-8"}
                                    style="width: 60px; height: 60px;"
                                    onclick={toggle_delete.reform(move |_| podcast_id_loop)}  // Use toggle_delete instead of direct state mutation
                                >
                                    <span class="material-icons">{"delete"}</span>
                                </button>
                            </div>
                        }
                    }).collect::<Html>()}
                </div>
            }
        }
        Some(PodcastLayout::Grid) => {
            html! {
                <div class="podcast-grid">
                    {podcasts.iter().map(|podcast| {
                        let on_click = create_on_title_click(
                            dispatch.clone(),
                            server_name.clone().unwrap(),
                            api_key.clone(),
                            history,
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
                        html! {
                            <div
                                class="podcast-grid-item"
                                onclick={on_click}
                            >
                                <div class="podcast-image-container">
                                    <img
                                        src={podcast.artworkurl.clone().unwrap_or_default()}
                                        alt={format!("Cover for {}", podcast.podcastname)}
                                        class="podcast-image"
                                    />
                                </div>
                                <div class="podcast-info">
                                    <h3 class="podcast-title-grid">{&podcast.podcastname}</h3>
                                </div>
                            </div>
                        }
                    }).collect::<Html>()}
                </div>
            }
        }
    }
}

#[function_component(Podcasts)]
pub fn podcasts() -> Html {
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let (filter_state, filter_dispatch) = use_store::<FilterState>();
    let history = BrowserHistory::new();
    let is_loading = use_state(|| false);
    let feed_url = use_state(|| "".to_string());
    let pod_user = use_state(|| "".to_string());
    let pod_pass = use_state(|| "".to_string());
    let error_message = use_state(|| None::<String>);
    let info_message = use_state(|| None::<String>);

    // filter selections
    let selected_category = use_state(|| None as Option<String>);

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

    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    // Fetch podcasts on component mount
    let filter_effect = filter_dispatch.clone();
    {
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        // let episodes = episodes.clone();

        let server_name_effect = server_name.clone();
        let user_id_effect = user_id.clone();
        let api_key_effect = api_key.clone();
        let effect_dispatch = dispatch.clone();

        use_effect_with(
            (api_key_effect, user_id_effect, server_name_effect),
            move |_| {
                // let episodes_clone = episodes.clone();
                // let error_clone = error.clone();

                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    let dispatch = effect_dispatch.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_podcasts(&server_name, &api_key, &user_id).await {
                            Ok(fetched_podcasts) => {
                                let fetch_casts = fetched_podcasts.clone();
                                dispatch.reduce_mut(move |state| {
                                    state.podcast_feed_return = Some(PodcastResponse {
                                        pods: Some(fetch_casts),
                                    });
                                });
                                // Extract unique categories
                                let mut categories = HashSet::new();
                                for podcast in &fetched_podcasts {
                                    if !podcast.categories.is_empty() {
                                        let podcast_categories: Vec<&str> =
                                            podcast.categories.split(',').collect();
                                        for cat in podcast_categories {
                                            categories.insert(cat.trim().to_string());
                                        }
                                    }
                                }

                                let category_list: Vec<String> = categories.into_iter().collect();

                                // Update the FilterState with the list of categories
                                filter_effect.reduce_mut(|filter_state| {
                                    filter_state.category_filter_list = Some(category_list);
                                });
                            }
                            Err(e) => web_sys::console::log_1(
                                &format!("Unable to parse Podcasts: {:?}", &e).into(),
                            ),
                        }
                    });
                }
                || ()
            },
        );
    }

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Delete,
        CustomPod,
    }

    let page_state = use_state(|| PageState::Hidden);
    let podcast_to_delete = use_state(|| None::<i32>);

    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
        })
    };

    let on_background_click = {
        let on_close_modal = on_close_modal.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            if element.tag_name() == "DIV" {
                on_close_modal.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let on_remove_click = {
        let dispatch_remove = dispatch.clone();
        let podcast_to_delete = podcast_to_delete.clone();
        let user_id = user_id.clone();
        let api_key_rm = api_key.clone();
        let server_name = server_name.clone();
        let on_close_remove = on_close_modal.clone();

        Callback::from(move |_: MouseEvent| {
            if let Some(podcast_id) = *podcast_to_delete {
                let dispatch_call = dispatch_remove.clone();
                let api_key_call = api_key_rm.clone();
                let server_name_call = server_name.clone();
                let user_id_call = user_id.unwrap();

                let remove_values = RemovePodcastValues {
                    podcast_id,
                    user_id: user_id_call,
                };

                wasm_bindgen_futures::spawn_local(async move {
                    match call_remove_podcasts(
                        &server_name_call.unwrap(),
                        &api_key_call.unwrap(),
                        &remove_values,
                    )
                    .await
                    {
                        Ok(success) => {
                            if success {
                                dispatch_call.apply(AppStateMsg::RemovePodcast(podcast_id));
                                dispatch_call.reduce_mut(|state| {
                                    state.info_message =
                                        Some("Podcast successfully removed".to_string())
                                });
                            } else {
                                dispatch_call.reduce_mut(|state| {
                                    state.error_message =
                                        Some("Failed to remove podcast".to_string())
                                });
                            }
                        }
                        Err(e) => {
                            dispatch_call.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("Error removing podcast: {:?}", e))
                            });
                        }
                    }
                });
            }
            on_close_remove.emit(MouseEvent::new("click").unwrap());
        })
    };

    // Define the modal components
    let delete_pod_model = html! {
        <div id="delete_pod_model" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Delete Podcast"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{"Are you sure you want to delete the podcast from the database? This will remove it from every aspect of the app. Meaning this will remove any saved, downloaded, or queued episodes for this podcast. It will also remove any history that includes it."}</label>
                                <div class="flex justify-between space-x-4">
                                    <button onclick={on_remove_click} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {"Yes, Delete Podcast"}
                                    </button>
                                    <button onclick={on_close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {"No, take me back"}
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    let toggle_delete = {
        let page_state = page_state.clone();
        let podcast_to_delete = podcast_to_delete.clone();
        Callback::from(move |podcast_id: i32| {
            podcast_to_delete.set(Some(podcast_id));
            page_state.set(PageState::Delete);
        })
    };

    // Correct setup for `on_password_change`
    let update_feed = {
        let feed_url = feed_url.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            feed_url.set(input.value());
        })
    };
    let update_pod_user = {
        let pod_user = pod_user.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            pod_user.set(input.value());
        })
    };
    let update_pod_pass = {
        let pod_pass = pod_pass.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            pod_pass.set(input.value());
        })
    };
    // Function to clear message
    let clear_error = {
        let error_message = error_message.clone();
        Callback::from(move |_| {
            error_message.set(None);
        })
    };

    let clear_info = {
        let info_message = info_message.clone();
        Callback::from(move |_| {
            info_message.set(None);
        })
    };

    // Ensure `onclick_restore` is correctly used
    let custom_loading = is_loading.clone();
    let add_custom_feed = {
        let dispatch_remove = dispatch.clone();
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let user_id = user_id;
        let feed_url = (*feed_url).clone();
        let error_message = error_message.clone();
        let info_message = info_message.clone();
        let clear_info = clear_info.clone();
        let clear_error = clear_error.clone();
        let is_loading_call = custom_loading.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let dispatch_call = dispatch_remove.clone();
            let clear_info = clear_info.clone();
            let clear_error = clear_error.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let feed_url = feed_url.clone();
            let error_message = error_message.clone();
            let info_message = info_message.clone();
            is_loading_call.set(true);
            let is_loading_wasm = is_loading_call.clone();
            let unstate_pod_user = (*pod_user).clone();
            let unstate_pod_pass = (*pod_pass).clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_add_custom_feed(
                    &server_name,
                    &feed_url,
                    &user_id.unwrap(),
                    &api_key.unwrap(),
                    Some(unstate_pod_user),
                    Some(unstate_pod_pass),
                )
                .await
                {
                    Ok(new_podcast) => {
                        info_message.set(Some("Podcast Successfully Added".to_string()));
                        dispatch_call.reduce_mut(move |state| {
                            if let Some(ref mut podcast_response) = state.podcast_feed_return {
                                if let Some(ref mut pods) = podcast_response.pods {
                                    web_sys::console::log_1(&JsValue::from_str("Adding Podcast"));
                                    pods.push(new_podcast.clone());
                                } else {
                                    web_sys::console::log_1(&JsValue::from_str("Creating Podcast"));
                                    podcast_response.pods = Some(vec![new_podcast.clone()]);
                                }
                            } else {
                                state.podcast_feed_return = Some(PodcastResponse {
                                    pods: Some(vec![new_podcast.clone()]),
                                });
                            }
                        });
                        Timeout::new(5000, move || clear_info.emit(())).forget();
                    }
                    Err(e) => {
                        error_message.set(Some(e.to_string()));
                        Timeout::new(5000, move || clear_error.emit(())).forget();
                    }
                }
                is_loading_wasm.set(false);
            });
        })
    };

    // Define the modal components
    let custom_pod_modal = html! {
        <div id="custom_pod_model" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="modal-container relative rounded-lg shadow">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t">
                        <h3 class="text-xl font-semibold">
                            {"Add Custom Podcast"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{"Simply enter the feed url, optional credentials, and click the button below. This is great in case you subscibe to premium podcasts and they aren't availble in The Pocast Index or other indexing services."}</label>
                                <div class="justify-between space-x-4">
                                    <div>
                                        <input id="feed_url" oninput={update_feed.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder="https://bestpodcast.com/feed.xml" />
                                    </div>
                                </div>
                                <div class="flex justify-between space-x-4">
                                    <div>
                                        <input id="username" oninput={update_pod_user.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5 mt-2" placeholder="Username (optional)" />
                                    </div>
                                    <div>
                                        <input id="password" type="password" oninput={update_pod_pass.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5 mt-2" placeholder="Password (optional)" />
                                    </div>
                                </div>
                                <div>
                                    <button onclick={add_custom_feed} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" disabled={*is_loading}>
                                    {"Add Feed"}
                                    if *is_loading {
                                        <span class="ml-2 spinner-border animate-spin inline-block w-4 h-4 border-2 rounded-full"></span>
                                    }
                                    </button>
                                </div>
                                <div>
                                if let Some(error) = &*error_message {
                                    <span class="text-red-600 text-xs">{ error }</span>
                                }
                                // Display informational message inline right below the text input
                                if let Some(info) = &*info_message {
                                    <span class="text-green-600 text-xs">{ info }</span>
                                }
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    let toggle_custom_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_: MouseEvent| {
            page_state.set(PageState::CustomPod);
        })
    };

    let show_filter_dropdown = use_state(|| false);
    let toggle_filter_dropdown = {
        let show_filter_dropdown = show_filter_dropdown.clone();
        Callback::from(move |_| {
            show_filter_dropdown.set(!*show_filter_dropdown);
        })
    };

    // Create a memoized filtered podcast list
    let filtered_pods = use_memo(
        (state.podcast_feed_return.clone(), selected_category.clone()),
        |(podcasts, selected_cat)| {
            if let Some(pods) = podcasts.as_ref().and_then(|p| p.pods.as_ref()) {
                if let Some(cat) = selected_cat.as_ref() {
                    pods.iter()
                        .filter(|podcast| podcast.categories.split(',').any(|c| c.trim() == cat))
                        .cloned()
                        .collect::<Vec<_>>()
                } else {
                    pods.clone()
                }
            } else {
                vec![]
            }
        },
    );

    let on_filter_click = {
        let selected_category = selected_category.clone();
        Callback::from(move |category: String| {
            selected_category.set(Some(category.clone()));
        })
    };

    let clear_filter = Callback::from(move |_| {
        selected_category.set(None);
        // Reset other filter states here as well
    });

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                match *page_state {
                PageState::Delete => delete_pod_model,
                PageState::CustomPod => custom_pod_modal,
                _ => html! {},
                }
            }
            {
                html! {
                    <div>
                        <div class="flex justify-between">
                            <div>
                                <button class="download-button font-bold py-2 px-4 rounded inline-flex items-center" onclick={toggle_filter_dropdown}>
                                    <span class="material-icons icon-space">{"filter_alt"}</span>
                                    <span class="text-lg">{"Filter"}</span>
                                </button>
                                {render_layout_toggle(dispatch.clone(), state.podcast_layout.clone())}
                            </div>
                            <button class="download-button font-bold py-2 px-4 rounded inline-flex items-center" onclick={toggle_custom_modal}>
                                <span class="material-icons icon-space">{"add_box"}</span>
                                <span class="text-lg">{"Add Custom Feed"}</span>
                            </button>
                        </div>
                    </div>
                }
            }
            {
                if *show_filter_dropdown {
                    html! {
                        <div class="filter-container flex items-center space-x-4">
                            // Clear Filter button
                            <button class="download-button font-bold py-2 px-4 rounded inline-flex items-center" onclick={clear_filter}>
                                <span class="material-icons icon-space">{"clear"}</span>
                                <span class="text-lg">{"Clear Filter"}</span>
                            </button>
                            // Category dropdown
                            <div class="filter-dropdown font-bold rounded">
                                {
                                    if let Some(categories) = &filter_state.category_filter_list {
                                        let categories_clone = categories.clone();
                                        html! {
                                            <>
                                                <select class="category-select" onchange={Callback::from(move |e: web_sys::Event| {
                                                    let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                    let selected_value = target.value();
                                                    on_filter_click.emit(selected_value);
                                                })}>
                                                    <option value="" disabled=true selected=true>{ "Select Category" }</option>
                                                    { for categories_clone.iter().map(|category| html! {
                                                        <option value={category.clone()}>{ category }</option>
                                                    }) }
                                                </select>
                                            </>
                                        }
                                    } else {
                                        html! { <p>{ "No categories available" }</p> }
                                    }
                                }
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }


            {
                if let Some(podcasts) = state.podcast_feed_return.clone() {
                    let int_podcasts = podcasts.clone();
                    if let Some(_pods) = int_podcasts.pods.clone() {
                        if filtered_pods.is_empty() {
                            // Render "No Podcasts Found" message
                            html! {
                                <div class="empty-episodes-container">
                                    <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                    <h1>{ "No Podcasts Found" }</h1>
                                    <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                                </div>
                                    }
                        } else {
                            // render_podcasts(&filtered_pods, state.podcast_layout.clone(), dispatch.clone(), &history)
                            render_podcasts(
                                &filtered_pods,
                                state.podcast_layout.clone(),
                                dispatch.clone(),
                                &history,
                                api_key.clone(),
                                server_name.clone(),
                                user_id,
                                desc_state,
                                desc_dispatch.clone(),
                                toggle_delete.clone(),
                            )
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
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} /> }
            } else {
                html! {}
            }
        }
        <App_drawer />
        </>
    }
}
