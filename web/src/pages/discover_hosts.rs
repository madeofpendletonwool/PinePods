use crate::components::app_drawer::App_drawer;
use crate::components::audio_player_bar::AudioPlayerBar;
use crate::components::context::AppState;
use crate::components::gen_components::{empty_message, FallbackImage, Search_nav, UseScrollToTop};
use crate::components::loading::Loading;
use crate::requests::people_req::{
    self, DiscoverHost, DiscoverPodcast, DiscoverStats,
};
use i18nrs::yew::use_translation;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use yewdux::prelude::*;

#[function_component(DiscoverHosts)]
pub fn discover_hosts() -> Html {
    let (i18n, _) = use_translation();
    let (post_state, _post_dispatch) = use_store::<AppState>();
    let loading = use_state(|| true);
    let top_hosts = use_state(Vec::<DiscoverHost>::new);
    let recent_hosts = use_state(Vec::<DiscoverHost>::new);
    let popular_podcasts = use_state(Vec::<DiscoverPodcast>::new);
    let stats = use_state(|| None::<DiscoverStats>);

    let api_key = post_state
        .auth_details
        .as_ref()
        .and_then(|ud| ud.api_key.clone());
    let server_name = post_state
        .auth_details
        .as_ref()
        .map(|ud| ud.server_name.clone());

    // Fetch all discovery lists on mount.
    {
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let loading = loading.clone();
        let top_hosts = top_hosts.clone();
        let recent_hosts = recent_hosts.clone();
        let popular_podcasts = popular_podcasts.clone();
        let stats = stats.clone();

        use_effect_with((), move |_| {
            if let (Some(server), Some(key)) = (server_name, api_key) {
                spawn_local(async move {
                    let (top, recent, popular, st) = futures::join!(
                        people_req::call_discover_top_hosts(&server, &key, 24),
                        people_req::call_discover_recent_hosts(&server, &key, 12),
                        people_req::call_discover_popular_podcasts(&server, &key, 12),
                        people_req::call_discover_stats(&server, &key),
                    );
                    if let Ok(v) = top {
                        top_hosts.set(v);
                    }
                    if let Ok(v) = recent {
                        recent_hosts.set(v);
                    }
                    if let Ok(v) = popular {
                        popular_podcasts.set(v);
                    }
                    if let Ok(v) = st {
                        stats.set(Some(v));
                    }
                    loading.set(false);
                });
            } else {
                loading.set(false);
            }
            || ()
        });
    }

    let history = BrowserHistory::new();
    let nav_to_person = {
        let history = history.clone();
        Callback::from(move |name: String| {
            history.push(format!("/person/{}", name));
        })
    };

    // Pre-capture translation strings (templates carry {count}/{hosts}/{podcasts} placeholders).
    let i18n_title = i18n.t("discover.title").to_string();
    let i18n_top_hosts = i18n.t("discover.top_hosts").to_string();
    let i18n_recently_added = i18n.t("discover.recently_added").to_string();
    let i18n_popular_podcasts = i18n.t("discover.popular_podcasts").to_string();
    let i18n_empty_title = i18n.t("discover.empty_title").to_string();
    let i18n_empty_message = i18n.t("discover.empty_message").to_string();
    let i18n_stats_summary = i18n.t("discover.stats_summary").to_string();
    let i18n_shows_count = i18n.t("discover.shows_count").to_string();
    let i18n_hosts_count = i18n.t("discover.hosts_count").to_string();

    let render_host = |host: &DiscoverHost, on_nav: &Callback<String>| -> Html {
        let name = host.name.clone();
        let onclick = {
            let on_nav = on_nav.clone();
            let name = name.clone();
            Callback::from(move |_: MouseEvent| on_nav.emit(name.clone()))
        };
        let sub = if host.podcast_count > 0 {
            html! { <p class="text-xs item_container-text opacity-70">{ i18n_shows_count.replace("{count}", &host.podcast_count.to_string()) }</p> }
        } else {
            html! {}
        };
        html! {
            <div class="discover-host-card flex flex-col items-center text-center p-3 cursor-pointer" onclick={onclick}>
                <div class="w-20 h-20 rounded-full overflow-hidden mb-2">
                    <FallbackImage src={host.img.clone()} alt={host.name.clone()} class={Some("w-full h-full object-cover".to_string())} />
                </div>
                <p class="item_container-text text-sm font-semibold">{ &host.name }</p>
                { sub }
            </div>
        }
    };

    html! {
        <>
        <div class="main-container">
            <Search_nav />
            <UseScrollToTop />
            {
                if *loading {
                    html! { <Loading/> }
                } else {
                    html! {
                        <div class="p-4">
                            <h1 class="text-2xl item_container-text font-bold text-center mb-2">{ &i18n_title }</h1>
                            {
                                if let Some(st) = (*stats).clone() {
                                    html! {
                                        <p class="text-center item_container-text opacity-70 mb-6">
                                            { i18n_stats_summary.replace("{hosts}", &st.total_hosts.to_string()).replace("{podcasts}", &st.total_podcasts.to_string()) }
                                        </p>
                                    }
                                } else { html! {} }
                            }

                            {
                                if top_hosts.is_empty() && recent_hosts.is_empty() && popular_podcasts.is_empty() {
                                    empty_message(
                                        &i18n_empty_title,
                                        &i18n_empty_message,
                                    )
                                } else {
                                    html! {
                                        <>
                                        {
                                            if !top_hosts.is_empty() {
                                                html! {
                                                    <div class="mb-8">
                                                        <h2 class="item_container-text text-xl font-semibold mb-4">{ &i18n_top_hosts }</h2>
                                                        <div class="grid grid-cols-3 md:grid-cols-6 gap-2">
                                                            { for top_hosts.iter().map(|h| render_host(h, &nav_to_person)) }
                                                        </div>
                                                    </div>
                                                }
                                            } else { html! {} }
                                        }
                                        {
                                            if !recent_hosts.is_empty() {
                                                html! {
                                                    <div class="mb-8">
                                                        <h2 class="item_container-text text-xl font-semibold mb-4">{ &i18n_recently_added }</h2>
                                                        <div class="grid grid-cols-3 md:grid-cols-6 gap-2">
                                                            { for recent_hosts.iter().map(|h| render_host(h, &nav_to_person)) }
                                                        </div>
                                                    </div>
                                                }
                                            } else { html! {} }
                                        }
                                        {
                                            if !popular_podcasts.is_empty() {
                                                html! {
                                                    <div class="mb-8">
                                                        <h2 class="item_container-text text-xl font-semibold mb-4">{ &i18n_popular_podcasts }</h2>
                                                        <div class="flex flex-col gap-2">
                                                            { for popular_podcasts.iter().map(|p| html! {
                                                                <div class="item-container border-solid border p-3 rounded-lg">
                                                                    <p class="item_container-text font-semibold">{ &p.title }</p>
                                                                    <p class="item_container-text text-sm opacity-70">{ i18n_hosts_count.replace("{count}", &p.host_count.to_string()) }</p>
                                                                </div>
                                                            }) }
                                                        </div>
                                                    </div>
                                                }
                                            } else { html! {} }
                                        }
                                        </>
                                    }
                                }
                            }
                        </div>
                    }
                }
            }
            <AudioPlayerBar />
        </div>
        <App_drawer />
        </>
    }
}
