use super::app_drawer::App_drawer;
use crate::components::audio::AudioPlayer;
use crate::components::click_events::create_on_title_click;
use crate::components::context::{AppState, ExpandedDescriptions, FilterState, UIState};
use crate::components::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::gen_funcs::format_error_message;
use crate::components::safehtml::SafeHtml;
use crate::requests::pod_req;
use crate::requests::pod_req::PodcastExtra;
use crate::requests::pod_req::{call_remove_podcasts, PodcastResponseExtra, RemovePodcastValues};
use crate::requests::setting_reqs::call_add_custom_feed;
use i18nrs::yew::use_translation;
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
    Grid,
    List,
}

#[allow(dead_code)]
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
                if let Some(podcasts) = &mut state_mut.podcast_feed_return_extra {
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

#[allow(dead_code)]
fn render_layout_toggle(
    dispatch: Dispatch<AppState>,
    current_layout: Option<PodcastLayout>,
    i18n: &i18nrs::I18n,
) -> Html {
    let onclick = dispatch.reduce_mut_callback(|state| {
        state.podcast_layout = match state.podcast_layout {
            Some(PodcastLayout::List) => Some(PodcastLayout::Grid),
            Some(PodcastLayout::Grid) => Some(PodcastLayout::List),
            None => Some(PodcastLayout::Grid),
        };
    });

    let (icon, text) = match current_layout {
        Some(PodcastLayout::List) => ("ph ph-squares-four", &i18n.t("podcasts.grid_view")),
        Some(PodcastLayout::Grid) => ("ph ph-list-dashes", &i18n.t("podcasts.list_view")),
        None => ("ph ph-list-dashes", &i18n.t("podcasts.list_view")),
    };

    html! {
        <button class="filter-chip" {onclick}>
            <i class={classes!(icon, "text-lg")}></i>
            <span class="text-sm font-medium">{text}</span>
        </button>
    }
}

#[allow(dead_code)]
fn render_podcasts(
    podcasts: &[PodcastExtra],
    layout: Option<PodcastLayout>,
    dispatch: Dispatch<AppState>,
    history: &BrowserHistory,
    api_key: Option<Option<String>>,
    server_name: Option<String>,
    user_id: Option<i32>,
    desc_state: Rc<ExpandedDescriptions>,
    desc_dispatch: Dispatch<ExpandedDescriptions>,
    toggle_delete: Callback<(i32, std::string::String)>,
    i18n: &i18nrs::I18n,
) -> Html {
    // Add a debug log at the start of render function
    web_sys::console::log_1(&format!("Rendering {} podcasts", podcasts.len()).into());

    match layout {
        None | Some(PodcastLayout::List) => {
            html! {
                <div>
                    {podcasts.iter().enumerate().map(|(index, podcast)| {
                        // Log each podcast for debugging
                        web_sys::console::log_1(&format!("Rendering podcast #{}: {} with artwork: {:?}",
                                                       index,
                                                       podcast.podcastname,
                                                       podcast.artworkurl).into());

                        let api_key_iter = api_key.clone();
                        let server_name_iter = server_name.clone().unwrap();
                        let history_clone = history.clone();

                        let dispatch_clone = dispatch.clone();
                        let podcast_id_loop = podcast.podcastid.clone();
                        let podcast_feed_loop = podcast.feedurl.clone();
                        let podcast_description_clone = podcast.description.clone();
                        let episode_count = podcast.episodecount.clone().unwrap_or_else(|| 0);

                        // Always use the specific podcast's artwork URL directly from this podcast object
                        let podcast_artwork = podcast.artworkurl.clone()
                            .unwrap_or_else(|| String::from("/static/assets/favicon.png"));

                        // Create a key for this podcast to help React properly track it
                        let podcast_key = format!("podcast-{}-{}", podcast.podcastid, podcast.podcastname);

                        let on_title_click = create_on_title_click(
                            dispatch_clone.clone(),
                            server_name_iter,
                            api_key_iter,
                            &history_clone,
                            podcast.podcastindexid.clone(),
                            podcast.podcastname.clone(),
                            podcast.feedurl.clone(),
                            podcast.description.clone().unwrap_or_else(|| i18n.t("podcasts.no_description_provided").to_string()),
                            podcast.author.clone().unwrap_or_else(|| i18n.t("podcasts.unknown_author").to_string()),
                            podcast_artwork.clone(), // Use the saved artwork URL directly
                            podcast.explicit.clone(),
                            episode_count,
                            podcast.categories.as_ref().map(|cats| cats.values().cloned().collect::<Vec<_>>().join(", ")),
                            podcast.websiteurl.clone().unwrap_or_else(|| i18n.t("podcasts.no_website_provided").to_string()),
                            user_id.unwrap(),
                            podcast.is_youtube,
                        );

                        let id_string = &podcast.podcastid.clone().to_string();
                        let desc_expanded = desc_state.expanded_descriptions.contains(id_string);
                        #[wasm_bindgen]
                        extern "C" {
                            #[wasm_bindgen(js_namespace = window)]
                            fn toggleDescription(guid: &str, expanded: bool);
                        }
                        let podcast_feed_call = podcast_feed_loop.clone();
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
                            <div key={podcast_key} class="item-container border-solid border flex items-start mb-4 shadow-md rounded-lg h-full">
                                <div class="flex flex-col w-auto object-cover pl-4">
                                    <FallbackImage
                                        src={podcast_artwork} // Direct use of saved artwork URL
                                        onclick={on_title_click.clone()}
                                        alt={format!("{}{}", i18n.t("podcasts.cover_alt_text"), podcast.podcastname.clone())}
                                        class={"episode-image"}
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
                                    <p class="item_container-text">{ format!("{}{}", &i18n.t("podcasts.episode_count"), &podcast.episodecount.clone().unwrap_or_else(|| 0)) }</p>
                                </div>
                                <button
                                    class={"item-container-button selector-button font-bold py-2 px-4 rounded-full self-center mr-8"}
                                    style="width: 60px; height: 60px;"
                                    onclick={toggle_delete.reform(move |_| (podcast_id_loop, podcast_feed_call.clone()))}  // Pass both as a tuple
                                >
                                    <i class="ph ph-trash text-3xl"></i>
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
                    {podcasts.iter().enumerate().map(|(_index, podcast)| {
                        // Log each grid podcast for debugging

                        // Create a key for this podcast
                        let podcast_key = format!("grid-podcast-{}-{}", podcast.podcastid, podcast.podcastname);

                        // Always use the specific podcast's artwork URL directly
                        let podcast_artwork = podcast.artworkurl.clone()
                            .unwrap_or_else(|| String::from("/static/assets/favicon.png"));

                        let on_click = create_on_title_click(
                            dispatch.clone(),
                            server_name.clone().unwrap(),
                            api_key.clone(),
                            history,
                            podcast.podcastindexid.clone(),
                            podcast.podcastname.clone(),
                            podcast.feedurl.clone(),
                            podcast.description.clone().unwrap_or_else(|| i18n.t("podcasts.no_description_provided").to_string()),
                            podcast.author.clone().unwrap_or_else(|| i18n.t("podcasts.unknown_author").to_string()),
                            podcast_artwork.clone(), // Use the saved artwork URL directly
                            podcast.explicit.clone(),
                            podcast.episodecount.clone().unwrap_or_else(|| 0),
                            podcast.categories.as_ref().map(|cats| cats.values().cloned().collect::<Vec<_>>().join(", ")),
                            podcast.websiteurl.clone().unwrap_or_else(|| i18n.t("podcasts.no_website_provided").to_string()),
                            user_id.unwrap(),
                            podcast.is_youtube,
                        );

                        // Get episode count
                        let episode_count = podcast.episodecount.unwrap_or(0);

                        html! {
                            <div
                                key={podcast_key}
                                class="podcast-grid-item relative"
                                onclick={on_click}
                            >
                                // Episode count badge
                                <div class="absolute top-1 right-1 z-10 bg-opacity-80 bg-gray-800 text-white rounded-full px-2 py-1 text-xs font-bold">
                                    <i class="ph ph-broadcast inline-block mr-1"></i>
                                    {episode_count}
                                </div>

                                <div class="podcast-image-container">
                                    <FallbackImage
                                        src={podcast_artwork}
                                        alt={format!("{}{}", i18n.t("podcasts.cover_alt_text"), podcast.podcastname.clone())}
                                        class={"podcast-image"}
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
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();
    let (desc_state, desc_dispatch) = use_store::<ExpandedDescriptions>();
    let (filter_state, filter_dispatch) = use_store::<FilterState>();
    let history = BrowserHistory::new();
    let is_loading = use_state(|| false);
    let feed_url = use_state(|| "".to_string());
    let youtube_url = use_state(|| "".to_string());
    let pod_user = use_state(|| "".to_string());
    let pod_pass = use_state(|| "".to_string());
    let search_term = use_state(|| String::new());

    #[derive(Clone, PartialEq)]
    enum SortDirection {
        AlphaAsc,
        AlphaDesc,
        EpisodeCountHigh,
        EpisodeCountLow,
        OldestFirst,
        NewestFirst,
        MostPlayed,
        LeastPlayed,
    }
    let sort_direction = use_state(|| Some(SortDirection::AlphaAsc));

    // Capture all i18n strings at function start to avoid borrow checker issues
    let i18n_delete_podcast = i18n.t("podcasts.delete_podcast").to_string();
    let i18n_yes_delete_podcast = i18n.t("podcasts.yes_delete_podcast").to_string();
    let i18n_no_take_me_back = i18n.t("podcasts.no_take_me_back").to_string();
    let i18n_add_custom_podcast = i18n.t("podcasts.add_custom_podcast").to_string();
    let i18n_add_feed = i18n.t("podcasts.add_feed").to_string();
    let i18n_custom_feed = i18n.t("podcasts.custom_feed").to_string();
    let i18n_search_podcasts_placeholder =
        i18n.t("podcasts.search_podcasts_placeholder").to_string();
    let i18n_youtube_channel_removed = i18n.t("podcasts.youtube_channel_removed").to_string();
    let i18n_podcast_successfully_added = i18n.t("podcasts.podcast_successfully_added").to_string();
    let i18n_podcast_removed = i18n.t("podcasts.podcast_removed").to_string();
    let i18n_youtube_channel_remove_failed =
        i18n.t("podcasts.youtube_channel_remove_failed").to_string();
    let i18n_podcast_remove_failed = i18n.t("podcasts.podcast_remove_failed").to_string();
    let i18n_error_removing_content = i18n.t("podcasts.error_removing_content").to_string();
    let i18n_failed_to_add_podcast = i18n.t("podcasts.failed_to_add_podcast").to_string();
    let i18n_feed_url_placeholder = i18n.t("podcasts.feed_url_placeholder").to_string();

    // filter selections
    let selected_category = use_state(|| None as Option<String>);

    let dispatch_layout = dispatch.clone();
    use_effect_with((), move |_| {
        dispatch_layout.reduce_mut(|state| {
            if state.podcast_layout.is_none() {
                state.podcast_layout = Some(PodcastLayout::Grid);
            }
        });
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
                        match pod_req::call_get_podcasts_extra(&server_name, &api_key, &user_id)
                            .await
                        {
                            Ok(fetched_podcasts) => {
                                let fetch_casts = fetched_podcasts.clone();
                                dispatch.reduce_mut(move |state| {
                                    state.podcast_feed_return_extra = Some(PodcastResponseExtra {
                                        pods: Some(fetch_casts),
                                    });
                                });
                                // Extract unique categories
                                let mut categories = HashSet::new();
                                for podcast in &fetched_podcasts {
                                    if let Some(ref podcast_categories) = podcast.categories {
                                        if !podcast_categories.is_empty() {
                                            for category in podcast_categories.values() {
                                                categories.insert(category.trim().to_string());
                                            }
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
    let podcast_to_delete_feed = use_state(|| None::<String>);

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
        let podcast_to_delete_feed = podcast_to_delete_feed.clone();
        let user_id = user_id.clone();
        let api_key_rm = api_key.clone();
        let server_name = server_name.clone();
        let on_close_remove = on_close_modal.clone();

        Callback::from(move |_: MouseEvent| {
            let podcast_id = *podcast_to_delete;
            let feed_url = (*podcast_to_delete_feed).clone();

            if let (Some(pid), Some(url)) = (podcast_id, feed_url) {
                let dispatch_call = dispatch_remove.clone();
                let api_key_call = api_key_rm.clone();
                let server_name_call = server_name.clone();
                let user_id_call = user_id.unwrap();

                // Capture translated messages before async block
                let youtube_success_msg = i18n_youtube_channel_removed.clone();
                let podcast_success_msg = i18n_podcast_removed.clone();
                let youtube_error_msg = i18n_youtube_channel_remove_failed.clone();
                let podcast_error_msg = i18n_podcast_remove_failed.clone();
                let error_prefix = i18n_error_removing_content.clone();

                let remove_values = RemovePodcastValues {
                    podcast_id: pid,
                    user_id: user_id_call,
                    is_youtube: url.starts_with("https://www.youtube.com"),
                };

                wasm_bindgen_futures::spawn_local(async move {
                    let result = if url.starts_with("https://www.youtube.com") {
                        call_remove_podcasts(
                            &server_name_call.unwrap(),
                            &api_key_call.unwrap(),
                            &remove_values,
                        )
                        .await
                    } else {
                        call_remove_podcasts(
                            &server_name_call.unwrap(),
                            &api_key_call.unwrap(),
                            &remove_values,
                        )
                        .await
                    };

                    match result {
                        Ok(success) => {
                            if success {
                                dispatch_call.apply(AppStateMsg::RemovePodcast(pid));
                                dispatch_call.reduce_mut(|state| {
                                    state.info_message =
                                        Some(if url.starts_with("https://www.youtube.com") {
                                            youtube_success_msg.clone()
                                        } else {
                                            podcast_success_msg.clone()
                                        })
                                });
                            } else {
                                dispatch_call.reduce_mut(|state| {
                                    state.error_message =
                                        Some(if url.starts_with("https://www.youtube.com") {
                                            youtube_error_msg.clone()
                                        } else {
                                            podcast_error_msg.clone()
                                        })
                                });
                            }
                        }
                        Err(e) => {
                            let formatted_error = format_error_message(&e.to_string());
                            dispatch_call.reduce_mut(|state| {
                                state.error_message =
                                    Some(format!("{}{:?}", error_prefix, formatted_error))
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
                            {&i18n_delete_podcast}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{&i18n.t("podcasts.close_modal")}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="download_schedule" class="block mb-2 text-sm font-medium">{&i18n.t("podcasts.delete_confirmation_text")}</label>
                                <div class="flex justify-between space-x-4">
                                    <button onclick={on_remove_click} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {&i18n_yes_delete_podcast}
                                    </button>
                                    <button onclick={on_close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                        {&i18n_no_take_me_back}
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
        let podcast_to_delete_feed = podcast_to_delete_feed.clone();
        Callback::from(move |(podcast_id, feed_url): (i32, String)| {
            podcast_to_delete.set(Some(podcast_id));
            podcast_to_delete_feed.set(Some(feed_url));
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

    let update_youtube_url = {
        let youtube_url = youtube_url.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_dyn_into().unwrap();
            youtube_url.set(input.value());
        })
    };

    // Podcast feed addition callback
    let custom_loading = is_loading.clone();
    let add_custom_feed = {
        let dispatch_remove = dispatch.clone();
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let user_id = user_id;
        let feed_url = (*feed_url).clone();
        let is_loading_call = custom_loading.clone();
        // Clone i18n messages before move
        let success_msg = i18n_podcast_successfully_added.clone();
        let error_prefix = i18n_failed_to_add_podcast.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let dispatch_call = dispatch_remove.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let feed_url = feed_url.clone();
            is_loading_call.set(true);
            let is_loading_wasm = is_loading_call.clone();
            let unstate_pod_user = (*pod_user).clone();
            let unstate_pod_pass = (*pod_pass).clone();

            // Clone again for async block
            let success_msg = success_msg.clone();
            let error_prefix = error_prefix.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_add_custom_feed(
                    &server_name,
                    &feed_url,
                    &user_id.unwrap(),
                    &api_key.unwrap(),
                    Some(unstate_pod_user),
                    Some(unstate_pod_pass),
                    Some(false), // Not a YouTube channel
                    Some(30),
                )
                .await
                {
                    Ok(new_podcast) => {
                        dispatch_call.reduce_mut(|state| {
                            state.info_message = Some(success_msg);
                        });
                        dispatch_call.reduce_mut(move |state| {
                            if let Some(ref mut podcast_response) = state.podcast_feed_return_extra
                            {
                                if let Some(ref mut pods) = podcast_response.pods {
                                    pods.push(PodcastExtra::from(new_podcast.clone()));
                                } else {
                                    podcast_response.pods =
                                        Some(vec![PodcastExtra::from(new_podcast.clone())]);
                                }
                            } else {
                                state.podcast_feed_return_extra = Some(PodcastResponseExtra {
                                    pods: Some(vec![PodcastExtra::from(new_podcast)]),
                                });
                            }
                        });
                    }
                    Err(e) => {
                        dispatch_call.reduce_mut(|state| {
                            state.error_message = Some(format!("{}{}", error_prefix, e));
                        });
                    }
                }
                is_loading_wasm.set(false);
            });
        })
    };

    // YouTube channel addition callback
    let youtube_loading = is_loading.clone();
    let add_youtube_channel = {
        let dispatch_remove = dispatch.clone();
        let api_key = api_key.clone().unwrap_or_default();
        let server_name = server_name.clone().unwrap_or_default();
        let user_id = user_id;
        let youtube_url = (*youtube_url).clone();
        let is_loading_call = youtube_loading.clone();
        // Clone i18n messages before move
        let success_msg = i18n_podcast_successfully_added.clone();
        let error_prefix = i18n_failed_to_add_podcast.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            let dispatch_call = dispatch_remove.clone();
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let youtube_url = youtube_url.clone();
            is_loading_call.set(true);
            let is_loading_wasm = is_loading_call.clone();

            // Clone again for async block
            let success_msg = success_msg.clone();
            let error_prefix = error_prefix.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_add_custom_feed(
                    &server_name,
                    &youtube_url,
                    &user_id.unwrap(),
                    &api_key.unwrap(),
                    None, // No username for YouTube
                    None, // No password for YouTube
                    Some(true), // IS a YouTube channel
                    Some(30),
                )
                .await
                {
                    Ok(new_podcast) => {
                        dispatch_call.reduce_mut(|state| {
                            state.info_message = Some(success_msg);
                        });
                        dispatch_call.reduce_mut(move |state| {
                            if let Some(ref mut podcast_response) = state.podcast_feed_return_extra
                            {
                                if let Some(ref mut pods) = podcast_response.pods {
                                    pods.push(PodcastExtra::from(new_podcast.clone()));
                                } else {
                                    podcast_response.pods =
                                        Some(vec![PodcastExtra::from(new_podcast.clone())]);
                                }
                            } else {
                                state.podcast_feed_return_extra = Some(PodcastResponseExtra {
                                    pods: Some(vec![PodcastExtra::from(new_podcast)]),
                                });
                            }
                        });
                    }
                    Err(e) => {
                        dispatch_call.reduce_mut(|state| {
                            state.error_message = Some(format!("{}{}", error_prefix, e));
                        });
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
                            {&i18n_add_custom_podcast}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{&i18n.t("podcasts.close_modal")}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            // Podcast Feed Section
                            <div>
                                <label for="feed_url" class="block mb-2 text-sm font-medium">{&i18n.t("podcasts.custom_podcast_instructions")}</label>
                                <div class="justify-between space-x-4">
                                    <div>
                                        <input id="feed_url" oninput={update_feed.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder={i18n_feed_url_placeholder.clone()} />
                                    </div>
                                </div>
                                <div class="flex justify-between space-x-4">
                                    <div>
                                        <input id="username" oninput={update_pod_user.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5 mt-2" placeholder={i18n.t("podcasts.username_optional")} />
                                    </div>
                                    <div>
                                        <input id="password" type="password" oninput={update_pod_pass.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5 mt-2" placeholder={i18n.t("podcasts.password_optional")} />
                                    </div>
                                </div>
                                <div>
                                    <button onclick={add_custom_feed} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" disabled={*is_loading}>
                                    {&i18n_add_feed}
                                    if *is_loading {
                                        <span class="ml-2 spinner-border animate-spin inline-block w-4 h-4 border-2 rounded-full"></span>
                                    }
                                    </button>
                                </div>
                            </div>

                            <hr class="my-4 border-t"/>

                            // YouTube Channel Section
                            <div>
                                <label for="youtube_url" class="block mb-2 text-sm font-medium">{&i18n.t("podcasts.youtube_channel_instructions")}</label>
                                <div>
                                    <input id="youtube_url" oninput={update_youtube_url.clone()} class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder={i18n.t("podcasts.youtube_channel_url_placeholder")} />
                                </div>
                                <div>
                                    <button onclick={add_youtube_channel} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline" disabled={*is_loading}>
                                    {&i18n.t("podcasts.add_channel")}
                                    if *is_loading {
                                        <span class="ml-2 spinner-border animate-spin inline-block w-4 h-4 border-2 rounded-full"></span>
                                    }
                                    </button>
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

    // Replace the existing filtered_pods use_memo block with this improved version
    // Modify your filtered_pods use_memo with this version that adds logging
    // and ensures proper image consistency

    let filtered_pods = use_memo(
        (
            state.podcast_feed_return_extra.clone(),
            selected_category.clone(),
            search_term.clone(),
            sort_direction.clone(),
        ),
        |(podcasts, selected_cat, search, sort_dir)| {
            // Log for debugging
            web_sys::console::log_1(&"Filtering podcasts...".into());

            if let Some(pods) = podcasts.as_ref().and_then(|p| p.pods.as_ref()) {
                // Log the original podcasts
                web_sys::console::log_1(&format!("Original podcasts count: {}", pods.len()).into());

                // Create a deep clone of all podcasts first to ensure we have independent copies
                let all_podcasts = pods.clone();

                // Apply filtering while ensuring artwork URLs are preserved
                let mut filtered = all_podcasts
                    .into_iter()
                    .filter(|podcast| {
                        // Apply search term filter
                        let matches_search = if !search.is_empty() {
                            podcast
                                .podcastname
                                .to_lowercase()
                                .contains(&search.to_lowercase())
                        } else {
                            true
                        };

                        // Apply category filter
                        let matches_category = if let Some(cat) = selected_cat.as_ref() {
                            if let Some(ref categories) = podcast.categories {
                                categories.values().any(|c| c.trim() == cat)
                            } else {
                                false
                            }
                        } else {
                            true
                        };

                        // Both conditions must be true
                        matches_search && matches_category
                    })
                    .collect::<Vec<_>>();

                // Log filtered podcasts
                if !search.is_empty() {
                    web_sys::console::log_1(
                        &format!(
                            "Filtered by search '{:?}': {} podcasts",
                            search,
                            filtered.len()
                        )
                        .into(),
                    );
                    // Log each podcast name and artwork URL for debugging
                    for pod in &filtered {
                        web_sys::console::log_1(
                            &format!(
                                "Podcast: {}, Artwork: {:?}",
                                pod.podcastname, pod.artworkurl
                            )
                            .into(),
                        );
                    }
                }

                // Make sure each podcast has its artwork URL properly set
                for podcast in &mut filtered {
                    if podcast.artworkurl.is_none() {
                        podcast.artworkurl = Some("/static/assets/favicon.png".to_string());
                    }
                }

                // Apply sorting to our filtered list
                if let Some(direction) = (*sort_dir).as_ref() {
                    filtered.sort_by(|a, b| match direction {
                        SortDirection::AlphaAsc => a
                            .podcastname
                            .to_lowercase()
                            .cmp(&b.podcastname.to_lowercase()),
                        SortDirection::AlphaDesc => b
                            .podcastname
                            .to_lowercase()
                            .cmp(&a.podcastname.to_lowercase()),
                        SortDirection::EpisodeCountHigh => b.episodecount.cmp(&a.episodecount),
                        SortDirection::EpisodeCountLow => a.episodecount.cmp(&b.episodecount),
                        SortDirection::OldestFirst => {
                            a.oldest_episode_date.cmp(&b.oldest_episode_date)
                        }
                        SortDirection::NewestFirst => {
                            b.oldest_episode_date.cmp(&a.oldest_episode_date)
                        }
                        SortDirection::MostPlayed => b.play_count.cmp(&a.play_count),
                        SortDirection::LeastPlayed => a.play_count.cmp(&b.play_count),
                    });
                }

                filtered
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

    // Add this function to clear filters and force a complete re-render
    let reset_filter = {
        let selected_category = selected_category.clone();
        let search_term = search_term.clone();
        let sort_direction = sort_direction.clone();
        let force_update = use_state(|| 0); // Add this state to force re-rendering

        Callback::from(move |_| {
            web_sys::console::log_1(&"Clearing all filters and resetting podcast list...".into());

            // Clear all filters
            selected_category.set(None);
            search_term.set(String::new());
            sort_direction.set(Some(SortDirection::AlphaAsc)); // Reset to default sort

            // Force re-render by incrementing counter
            force_update.set(*force_update + 1);
        })
    };

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
                        // Modern mobile-friendly filter bar with tab-style page title
                        <div class="mb-6 space-y-4">
                            // Tab-style page indicator
                            <div class="flex gap-0 h-12 relative">
                                // <div class="page-tab-indicator">
                                //     <i class="ph ph-microphone tab-icon"></i>
                                //     {"Podcasts"}
                                // </div>
                                <div class="flex gap-2 ml-auto items-center">
                                    <button class="filter-chip" onclick={toggle_custom_modal}>
                                        <i class="ph ph-plus-circle text-lg"></i>
                                        <span class="text-sm font-medium">{&i18n_custom_feed}</span>
                                    </button>
                                    {render_layout_toggle(dispatch.clone(), state.podcast_layout.clone(), &i18n)}
                                </div>
                            </div>

                            // Combined search and sort bar
                            <div class="flex gap-0 h-12 relative">
                                // Search input (left half)
                                <div class="flex-1 relative">
                                    <input
                                        type="text"
                                        class="search-input"
                                        placeholder={i18n_search_podcasts_placeholder.clone()}
                                        value={(*search_term).clone()}
                                        oninput={let search_term = search_term.clone();
                                            Callback::from(move |e: InputEvent| {
                                                if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                    search_term.set(input.value());
                                                }
                                            })
                                        }
                                    />
                                    <i class="ph ph-magnifying-glass search-icon"></i>
                                </div>

                                // Sort dropdown (right half)
                                <div class="flex-shrink-0 relative min-w-[160px]">
                                    <select
                                        class="sort-dropdown"
                                        value="alpha_asc"
                                        onchange={
                                            let sort_direction = sort_direction.clone();
                                            Callback::from(move |e: Event| {
                                                let target = e.target_dyn_into::<web_sys::HtmlSelectElement>().unwrap();
                                                let value = target.value();
                                                match value.as_str() {
                                                    "alpha_asc" => sort_direction.set(Some(SortDirection::AlphaAsc)),
                                                    "alpha_desc" => sort_direction.set(Some(SortDirection::AlphaDesc)),
                                                    "episodes_high" => sort_direction.set(Some(SortDirection::EpisodeCountHigh)),
                                                    "episodes_low" => sort_direction.set(Some(SortDirection::EpisodeCountLow)),
                                                    "oldest" => sort_direction.set(Some(SortDirection::OldestFirst)),
                                                    "newest" => sort_direction.set(Some(SortDirection::NewestFirst)),
                                                    "most_played" => sort_direction.set(Some(SortDirection::MostPlayed)),
                                                    "least_played" => sort_direction.set(Some(SortDirection::LeastPlayed)),
                                                    _ => sort_direction.set(None),
                                                }
                                            })
                                        }
                                    >
                                        <option value="alpha_asc" selected=true>{&i18n.t("podcasts.sort_a_to_z_up")}</option>
                                        <option value="alpha_desc">{&i18n.t("podcasts.sort_z_to_a_down")}</option>
                                        <option value="episodes_high">{&i18n.t("podcasts.sort_most_episodes")}</option>
                                        <option value="episodes_low">{&i18n.t("podcasts.sort_least_episodes")}</option>
                                        <option value="oldest">{&i18n.t("podcasts.sort_oldest_first")}</option>
                                        <option value="newest">{&i18n.t("podcasts.sort_newest_first")}</option>
                                        <option value="most_played">{&i18n.t("podcasts.sort_most_played")}</option>
                                        <option value="least_played">{&i18n.t("podcasts.sort_least_played")}</option>
                                    </select>
                                    <i class="ph ph-caret-down dropdown-arrow"></i>
                                </div>
                            </div>

                            // Filter chips (horizontal scroll on mobile)
                            <div class="flex gap-3 overflow-x-auto pb-2 md:pb-0 scrollbar-hide">
                                // Clear all filters
                                <button
                                    onclick={reset_filter}
                                    class="filter-chip"
                                >
                                    <i class="ph ph-broom text-lg"></i>
                                    <span class="text-sm font-medium">{&i18n.t("podcasts.clear_all")}</span>
                                </button>

                                // Category filter chips (limited to prevent multiple lines)
                                {
                                    if let Some(categories) = &filter_state.category_filter_list {
                                        categories.iter().map(|category| {
                                            let category_clone = category.clone();
                                            let is_selected = selected_category.as_ref().map_or(false, |selected| selected == category);
                                            let on_filter_click_clone = on_filter_click.clone();

                                            html! {
                                                <button
                                                    onclick={Callback::from(move |_| {
                                                        on_filter_click_clone.emit(category_clone.clone());
                                                    })}
                                                    class={classes!(
                                                        "filter-chip",
                                                        if is_selected { "filter-chip-active" } else { "" }
                                                    )}
                                                >
                                                    <span class="text-sm font-medium">{category}</span>
                                                </button>
                                            }
                                        }).collect::<Html>()
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                        </div>
                    </div>
                }
            }



            {
                if let Some(podcasts) = state.podcast_feed_return_extra.clone() {
                    let int_podcasts = podcasts.clone();
                    if let Some(_pods) = int_podcasts.pods.clone() {
                        if filtered_pods.is_empty() {
                            empty_message(
                                &i18n.t("podcasts.no_podcasts_found"),
                                &i18n.t("podcasts.no_podcasts_found_description")
                            )
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
                                &i18n,
                            )
                        }


                    } else {
                        empty_message(
                            &i18n.t("podcasts.no_podcasts_found"),
                            &i18n.t("podcasts.no_podcasts_found_description")
                        )
                    }
                } else {
                    empty_message(
                        &i18n.t("podcasts.no_podcasts_found"),
                        &i18n.t("podcasts.no_podcasts_found_description")
                    )
                }
            }
        </div>
        {
            if let Some(audio_props) = &audio_state.currently_playing {
                html! { <AudioPlayer src={audio_props.src.clone()} title={audio_props.title.clone()} description={audio_props.description.clone()} release_date={audio_props.release_date.clone()} artwork_url={audio_props.artwork_url.clone()} duration={audio_props.duration.clone()} episode_id={audio_props.episode_id.clone()} duration_sec={audio_props.duration_sec.clone()} start_pos_sec={audio_props.start_pos_sec.clone()} end_pos_sec={audio_props.end_pos_sec.clone()} offline={audio_props.offline.clone()} is_youtube={audio_props.is_youtube.clone()} /> }
            } else {
                html! {}
            }
        }
        <App_drawer />
        </>
    }
}
