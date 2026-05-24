use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::AppState;
use crate::components::gen_components::{Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::components::episode_list_item::EpisodeListItem;
use crate::requests::episode::Episode;
use crate::requests::pod_req::{self, PlaylistInfo};
use i18nrs::yew::use_translation;
use js_sys::Array;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use web_sys::{IntersectionObserver, IntersectionObserverEntry, IntersectionObserverInit};
use yew::prelude::*;
use yewdux::prelude::*;

const PAGE_SIZE: i64 = 50;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub id: i32,
}

#[function_component(PlaylistDetail)]
pub fn playlist_detail(props: &Props) -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();

    let episodes = use_state(|| Vec::<Episode>::new());
    let playlist_info = use_state(|| Option::<PlaylistInfo>::None);
    let total = use_state(|| 0i64);
    let offset = use_state(|| 0i64);
    let loading = use_state(|| true);
    let loading_more = use_state(|| false);
    let error = use_state(|| None::<String>);
    let sentinel_ref = use_node_ref();

    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let playlist_id = props.id;

    // Initial page fetch
    {
        let episodes = episodes.clone();
        let playlist_info = playlist_info.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading = loading.clone();
        let error = error.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();

        use_effect_with(
            (api_key.clone(), user_id.clone(), server_name.clone(), playlist_id),
            move |_| {
                if let (Some(api_key), Some(user_id), Some(server_name)) =
                    (api_key.clone(), user_id.clone(), server_name.clone())
                {
                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_playlist_episodes_paged(
                            &server_name,
                            &api_key.unwrap_or_default(),
                            &user_id,
                            playlist_id,
                            PAGE_SIZE,
                            0,
                        )
                        .await
                        {
                            Ok(page) => {
                                let new_offset = page.episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                playlist_info.set(Some(page.playlist_info));
                                episodes.set(page.episodes);
                                loading.set(false);
                            }
                            Err(e) => {
                                error.set(Some(e.to_string()));
                                loading.set(false);
                            }
                        }
                    });
                }
                || ()
            },
        );
    }

    // IntersectionObserver: load next page when sentinel comes into view
    {
        let episodes = episodes.clone();
        let total = total.clone();
        let offset = offset.clone();
        let loading_more = loading_more.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let server_name = server_name.clone();
        let sentinel_ref = sentinel_ref.clone();

        use_effect_with(
            (sentinel_ref.clone(), *offset, *total),
            move |(sentinel_ref, _, _)| {
                let sentinel_el = match sentinel_ref.cast::<web_sys::Element>() {
                    Some(el) => el,
                    None => return Box::new(|| ()) as Box<dyn FnOnce()>,
                };

                let episodes = episodes.clone();
                let total = total.clone();
                let offset = offset.clone();
                let loading_more = loading_more.clone();
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

                    let episodes = episodes.clone();
                    let total = total.clone();
                    let offset = offset.clone();
                    let loading_more = loading_more.clone();
                    let api_key = api_key.clone();
                    let user_id = user_id.clone();
                    let server_name = server_name.clone();

                    if let (Some(api_key), Some(user_id), Some(server_name)) =
                        (api_key.clone(), user_id.clone(), server_name.clone())
                    {
                        loading_more.set(true);
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(page) = pod_req::call_get_playlist_episodes_paged(
                                &server_name,
                                &api_key.unwrap_or_default(),
                                &user_id,
                                playlist_id,
                                PAGE_SIZE,
                                current_offset,
                            )
                            .await
                            {
                                let new_offset = current_offset + page.episodes.len() as i64;
                                total.set(page.total);
                                offset.set(new_offset);
                                episodes.set({
                                    let mut all = (*episodes).clone();
                                    all.extend(page.episodes);
                                    all
                                });
                            }
                            loading_more.set(false);
                        });
                    }
                }));

                let mut opts = IntersectionObserverInit::new();
                opts.root_margin("200px");

                let observer =
                    IntersectionObserver::new_with_options(callback.as_ref().unchecked_ref(), &opts)
                        .expect("IntersectionObserver creation failed");
                observer.observe(&sentinel_el);
                callback.forget();

                let observer_clone = observer.clone();
                Box::new(move || {
                    observer_clone.disconnect();
                }) as Box<dyn FnOnce()>
            },
        );
    }

    html! {
        <>
            <div class="main-container">
                <Search_nav />
                <UseScrollToTop />

                if *loading {
                    { html! { <Loading/> } }
                } else {
                    if let Some(error_msg) = &*error {
                        <div class="error-message">
                            {error_msg}
                        </div>
                    } else {
                        <div class="section-container">
                            if let Some(info) = &*playlist_info {
                                <div class="playlist-header mb-6">
                                    <div class="flex items-start justify-between gap-4">
                                        <div class="flex items-center gap-4 flex-grow">
                                            <i class={classes!("text-4xl", info.icon_name.clone())}></i>
                                            <div>
                                                <h1 class="text-2xl font-bold item_container-text">{&info.name}</h1>
                                                if let Some(desc) = &info.description {
                                                    <p class="text-gray-600 dark:text-gray-400">{desc}</p>
                                                }
                                                <p class="text-sm item_container-text mt-1">
                                                    {format!("{} {}", info.episode_count.unwrap_or(0), &i18n.t("playlist_detail.episodes"))}
                                                </p>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            }

                            if (*episodes).is_empty() {
                                <div class="flex flex-col items-center justify-center p-8 mt-4 item-container rounded-lg shadow-md">
                                    <i class="ph ph-playlist-x text-6xl mb-4"></i>
                                    <h3 class="text-xl font-semibold mb-2 item_container-text">{&i18n.t("playlist_detail.no_episodes_found")}</h3>
                                    <p class="text-center item_container-text text-sm max-w-md">
                                        {
                                            match (*playlist_info).as_ref().map(|i| i.name.as_str()) {
                                                Some("Fresh Releases") => i18n.t("playlist_detail.fresh_releases_empty"),
                                                Some("Currently Listening") => i18n.t("playlist_detail.currently_listening_empty"),
                                                Some("Almost Done") => i18n.t("playlist_detail.almost_done_empty"),
                                                _ => i18n.t("playlist_detail.no_episodes_match_criteria")
                                            }
                                        }
                                    </p>
                                </div>
                            } else {
                                <div class="flex-grow overflow-y-auto">
                                    { for (*episodes).iter().map(|ep| html! {
                                        <EpisodeListItem key={ep.episodeid} episode={ep.clone()} />
                                    }) }
                                    <div ref={sentinel_ref.clone()} style="height: 1px;" />
                                    if *loading_more {
                                        <Loading />
                                    }
                                </div>
                            }
                        </div>
                    }
                }
                <AudioPlayerBar />
            </div>
            <App_drawer />
        </>
    }
}
