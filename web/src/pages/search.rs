use crate::components::app_drawer::App_drawer;
use crate::components::audio::on_play_pause;
use crate::components::audio::AudioPlayer;
use crate::components::context::{AppState, UIState};
use crate::components::episode_list_item::EpisodeListItem;
use crate::components::gen_components::{empty_message, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::requests::episode::Episode;
use crate::requests::search_pods::{call_search_database_paged, SearchRequest};
use async_std::task::sleep;
use gloo_events::EventListener;
use i18nrs::yew::use_translation;
use js_sys::Array;
use std::time::Duration;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use web_sys::HtmlElement;
use web_sys::HtmlInputElement;
use web_sys::{IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};
use yew::prelude::*;
use yew::{function_component, html, use_node_ref, Callback, Html, Properties};
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

#[derive(Properties, Clone, PartialEq)]
pub struct SearchProps {
    pub on_search: Callback<String>,
}

#[function_component(Search)]
pub fn search(_props: &SearchProps) -> Html {
    let (i18n, _) = use_translation();
    let (post_state, _dispatch) = use_store::<AppState>();
    let (audio_state, _audio_dispatch) = use_store::<UIState>();

    let input_ref = use_node_ref();
    let input_ref_clone1 = input_ref.clone();
    let input_ref_clone2 = input_ref.clone();
    let form_ref = NodeRef::default();
    let form_ref_clone1 = form_ref.clone();
    let container_ref = use_node_ref();
    let container_ref_clone1 = container_ref.clone();

    let api_key = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.api_key.clone());
    let user_id = post_state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    // Pagination state (local to this component)
    let episodes = use_state(|| Vec::<Episode>::new());
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading_more = use_state(|| false);
    let current_term = use_state(|| String::new());
    let sentinel_ref = use_node_ref();

    // Track screen size for responsive adjustments
    let is_mobile = use_state(|| false);

    {
        let is_mobile = is_mobile.clone();

        use_effect_with((), move |_| {
            let update_mobile_state = {
                let is_mobile = is_mobile.clone();

                Callback::from(move |_| {
                    if let Some(window) = window() {
                        if let Ok(width) = window.inner_width() {
                            if let Some(width) = width.as_f64() {
                                is_mobile.set(width <= 500.0);
                            }
                        }
                    }
                })
            };

            update_mobile_state.emit(());

            let window = window().unwrap();
            let listener = EventListener::new(&window, "resize", move |_| {
                update_mobile_state.emit(());
            });

            move || drop(listener)
        });
    }

    let api_key_submit = api_key.clone();
    let user_id_submit = user_id.clone();
    let server_name_submit = server_name.clone();

    let on_submit = {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        let current_term = current_term.clone();

        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            let container_ref_submit_clone1 = container_ref_clone1.clone();

            if let Some(form) = form_ref_clone1.cast::<HtmlElement>() {
                form.class_list().add_1("move-to-top").unwrap();
            }

            if let Some(form) = input_ref_clone1.cast::<HtmlElement>() {
                form.class_list().add_1("move-to-top").unwrap();
            }

            let server_name_submit = server_name_submit.clone();
            let api_key_submit = api_key_submit.clone();
            let user_id_submit = user_id_submit.clone();

            let search_term = match input_ref_clone2.cast::<HtmlInputElement>() {
                Some(el) => el.value(),
                None => return,
            };
            if search_term.trim().is_empty() {
                return;
            }

            // Reset state for the new search
            episodes.set(Vec::new());
            total.set(0);
            offset.set(0);
            current_term.set(search_term.clone());
            loading_more.set(true);

            let episodes = episodes.clone();
            let total = total.clone();
            let offset = offset.clone();
            let loading_more = loading_more.clone();

            let future = async move {
                sleep(Duration::from_secs(1)).await;
                if let Some(container) = container_ref_submit_clone1.cast::<HtmlElement>() {
                    container.class_list().add_1("shrink-input").unwrap();
                }

                if let (Some(server_name), Some(api_key), Some(user_id)) = (
                    server_name_submit,
                    api_key_submit.flatten(),
                    user_id_submit,
                ) {
                    let request = SearchRequest {
                        search_term,
                        user_id,
                    };
                    match call_search_database_paged(
                        &server_name,
                        &Some(api_key),
                        &request,
                        PAGE_SIZE,
                        0,
                    )
                    .await
                    {
                        Ok(page) => {
                            total.set(page.total);
                            offset.set(page.data.len() as i64);
                            episodes.set(page.data);
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Failed to search database: {:?}", e).into(),
                            );
                        }
                    }
                }
                loading_more.set(false);
            };
            spawn_local(future);
        })
    };

    let container_height = use_state(|| "221px".to_string());

    {
        let container_height = container_height.clone();
        use_effect_with((), move |_| {
            let update_height = {
                let container_height = container_height.clone();
                Callback::from(move |_| {
                    if let Some(window) = window() {
                        if let Ok(width) = window.inner_width() {
                            if let Some(width) = width.as_f64() {
                                let new_height = if width <= 530.0 {
                                    "122px"
                                } else if width <= 768.0 {
                                    "150px"
                                } else {
                                    "221px"
                                };
                                container_height.set(new_height.to_string());
                            }
                        }
                    }
                })
            };

            update_height.emit(());

            let listener = EventListener::new(&window().unwrap(), "resize", move |_| {
                update_height.emit(());
            });

            move || drop(listener)
        });
    }

    // IntersectionObserver for infinite scroll — mirrors feed.rs exactly
    {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        let current_term = current_term.clone();
        let sentinel_ref = sentinel_ref.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();

        use_effect_with(
            (sentinel_ref.clone(), *offset, *total, (*current_term).clone()),
            move |(sentinel_ref, _, _, _)| {
                let sentinel_el = match sentinel_ref.cast::<web_sys::Element>() {
                    Some(el) => el,
                    None => return Box::new(|| ()) as Box<dyn FnOnce()>,
                };

                let episodes = episodes.clone();
                let total = total.clone();
                let offset = offset.clone();
                let loading_more = loading_more.clone();
                let current_term = current_term.clone();
                let api_key = api_key.clone();
                let user_id = user_id.clone();
                let server_name = server_name.clone();

                let callback = Closure::<dyn Fn(Array)>::wrap(Box::new(move |entries: Array| {
                    let entry: IntersectionObserverEntry = entries.get(0).unchecked_into();
                    if !entry.is_intersecting() {
                        return;
                    }

                    let current_offset = *offset;
                    let current_total = *total;
                    if *loading_more || current_offset >= current_total {
                        return;
                    }

                    let search_term = (*current_term).clone();
                    if search_term.is_empty() {
                        return;
                    }

                    let episodes = episodes.clone();
                    let total = total.clone();
                    let offset = offset.clone();
                    let loading_more = loading_more.clone();
                    let current_term = current_term.clone();
                    let api_key = api_key.clone();
                    let user_id = user_id.clone();
                    let server_name = server_name.clone();

                    loading_more.set(true);
                    wasm_bindgen_futures::spawn_local(async move {
                        if let (Some(server_name), Some(api_key), Some(user_id)) =
                            (server_name, api_key.flatten(), user_id)
                        {
                            let request = SearchRequest {
                                search_term: search_term.clone(),
                                user_id,
                            };
                            if let Ok(page) = call_search_database_paged(
                                &server_name,
                                &Some(api_key),
                                &request,
                                PAGE_SIZE,
                                current_offset,
                            )
                            .await
                            {
                                // Discard stale results if the search term changed mid-flight
                                if *current_term != search_term {
                                    loading_more.set(false);
                                    return;
                                }
                                offset.set(current_offset + page.data.len() as i64);
                                total.set(page.total);
                                episodes.set({
                                    let mut all = (*episodes).clone();
                                    all.extend(page.data);
                                    all
                                });
                            }
                        }
                        loading_more.set(false);
                    });
                }));

                let mut opts = IntersectionObserverInit::new();
                opts.root_margin("200px");
                let observer = IntersectionObserver::new_with_options(
                    callback.as_ref().unchecked_ref(),
                    &opts,
                )
                .expect("IntersectionObserver creation failed");
                observer.observe(&sentinel_el);
                callback.forget();

                Box::new(move || observer.disconnect()) as Box<dyn FnOnce()>
            },
        );
    }

    // Placeholder text changes based on screen size
    let placeholder_text = if *is_mobile {
        i18n.t("search.search_podcasts")
    } else {
        i18n.t("search.search_for_podcast_episode_description")
    };

    // Pre-compute button text
    let go_text = i18n.t("search.go");
    let search_text = i18n.t("search.search");

    html! {
        <>
        <div class="search-page-container">
            <Search_nav />
            <UseScrollToTop />
            <div class="search-container" ref={container_ref.clone()}>
                <form class="search-page-input" onsubmit={on_submit} ref={form_ref.clone()}>
                    <label for="search" class="mb-2 text-sm font-medium text-gray-900 sr-only dark:text-white">{ &i18n.t("search.search") }</label>
                    <div class="relative">
                        <div class="absolute inset-y-0 start-0 flex items-center ps-3 pointer-events-none">
                            <svg class="w-4 h-4 text-gray-500 dark:text-gray-400" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 20 20">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m19 19-4-4m0-7A7 7 0 1 1 1 8a7 7 0 0 1 14 0Z"/>
                            </svg>
                        </div>
                        <input
                            type="search"
                            id="search"
                            class={if *is_mobile { "search-bar-input mobile-search-input block w-full p-3 ps-10 text-sm border rounded-lg" }
                                  else { "search-bar-input block w-full p-4 ps-10 text-sm border rounded-lg" }}
                            placeholder={placeholder_text}
                            ref={input_ref.clone()}
                        />
                        <button
                            class={if *is_mobile { "search-page-button mobile-search-button absolute end-2 bottom-2 focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-3 py-1.5" }
                                   else { "search-page-button absolute end-2.5 bottom-2.5 focus:ring-4 focus:outline-none font-medium rounded-lg text-sm px-4 py-2" }}
                        >
                            { if *is_mobile { &go_text } else { &search_text } }
                        </button>
                    </div>
                </form>
            </div>

            {
                if !(*current_term).is_empty() {
                    if (*episodes).is_empty() && !*loading_more {
                        empty_message(
                            &i18n.t("search.no_results_found"),
                            &i18n.t("search.try_different_search")
                        )
                    } else {
                        html! {
                            <div class={if *is_mobile { "search-results-container mobile-results" } else { "search-results-container" }}>
                                { for (*episodes).iter().map(|episode| {
                                    html! {
                                        <EpisodeListItem
                                            key={episode.episodeid}
                                            episode={episode.clone()}
                                        />
                                    }
                                }) }
                                <div ref={sentinel_ref.clone()} style="height: 1px;" />
                                if *loading_more {
                                    <Loading />
                                }
                            </div>
                        }
                    }
                } else {
                    html! {}
                }
            }

            <App_drawer />

            {
                if let Some(audio_props) = &audio_state.currently_playing {
                    html! {
                        <AudioPlayer
                            episode={audio_props.episode.clone()}
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
                            is_video={audio_props.is_video.clone()}
                        />
                     }
                } else {
                    html! {}
                }
            }
        </div>
        </>
    }
}
