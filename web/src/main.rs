mod components;
mod pages;
mod requests;

use crate::components::navigation::NavigationHandler;
use crate::components::oauth_callback::OAuthCallback;
use crate::components::restore_overlay::RestoreOverlay;
use crate::components::collection_picker_modal::CollectionPickerModal;
use crate::pages::downloads::Downloads;
use crate::pages::episode::Episode;
use crate::pages::episode_layout::EpisodeLayout;
use crate::pages::feed::Feed;
use crate::pages::history::PodHistory;
use crate::pages::home::Home;
use crate::pages::internal_error::InternalError;
use crate::pages::not_found::NotFound;
use crate::pages::person::Person;
use crate::pages::playlist_detail::PlaylistDetail;
use crate::pages::playlists::Playlists;
use crate::pages::podcast_layout::PodLayout;
use crate::pages::podcasts::Podcasts;
use crate::pages::queue::Queue;
use crate::pages::routes::Route;
use crate::pages::saved::Saved;
use crate::pages::search::Search;
use crate::pages::search_new::SearchNew;
use crate::pages::settings::Settings;
use crate::pages::shared_episode::SharedEpisode;
use crate::pages::subscribed_people::SubscribedPeople;
use crate::pages::discover::Discover;
use crate::pages::user_stats::UserStats;
use crate::pages::youtube_layout::YouTubeLayout;

use yew_router::history::BrowserHistory;
use yew_router::history::History;

#[cfg(feature = "server_build")]
use pages::login::{ChangeServer, LogOut, Login};

#[cfg(not(feature = "server_build"))]
use {
    pages::downloads_tauri::Downloads as LocalDownloads,
    pages::login_tauri::{ChangeServer, LogOut, Login},
};

// Yew Imports
use crate::components::context::AppState;
use i18nrs::yew::use_translation;
use i18nrs::yew::{I18nProvider, I18nProviderConfig};
use requests::setting_reqs::call_get_server_default_language;
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;
use web_sys;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

fn switch(route: Route) -> Html {
    match route {
        Route::ChangeServer => html! { <ChangeServer /> },
        Route::Downloads => html! { <Downloads /> },
        Route::Episode => html! { <Episode /> },
        Route::EpisodeLayout => html! { <EpisodeLayout /> },
        Route::Feed => html! { <Feed /> },
        Route::Home => html! { <Home /> },
        Route::Login => html! { <Login /> },
        Route::LogOut => html! { <LogOut /> },
        Route::NotFound => html! { <NotFound /> },
        Route::OAuthCallback => html! { <OAuthCallback /> },
        Route::Person { name } => html! { <Person name={name.clone()} /> },
        Route::PlaylistDetail { id } => html! { <PlaylistDetail {id} /> },
        Route::Playlists => html! { <Playlists /> },
        Route::Podcasts => html! { <Podcasts /> },
        Route::PodHistory => html! { <PodHistory /> },
        Route::PodLayout => html! { <PodLayout /> },
        Route::Queue => html! { <Queue /> },
        Route::Saved => html! { <Saved /> },
        Route::Collections => html! { <Saved /> },
        Route::Search => html! { <Search on_search={Callback::from(move |_| {})} /> },
        Route::SearchNew => html! { <SearchNew /> },
        Route::Settings => html! { <Settings /> },
        Route::SharedEpisode { url_key } => html! { <SharedEpisode url_key={url_key.clone()} /> },
        Route::SubscribedPeople => html! { <SubscribedPeople /> },
        Route::Discover => html! { <Discover /> },
        Route::DiscoverHosts => html! { <Discover /> },
        Route::UserStats => html! { <UserStats /> },
        Route::YoutubeLayout => html! { <YouTubeLayout /> },
        #[cfg(not(feature = "server_build"))]
        Route::LocalDownloads => html! { <LocalDownloads /> },
        #[cfg(feature = "server_build")]
        Route::LocalDownloads => {
            html! { <div>{"Local downloads not available on the web version"}</div> } // i18n-ignore
        }
    }
}

#[function_component(LanguageHandler)]
fn language_handler() -> Html {
    // Set up translations with all available language files
    // IMPORTANT: English must be first for i18nrs fallback to work correctly
    let translations = HashMap::from([
        ("en", include_str!("translations/en.json")), // Keep English first for fallback
        ("ar", include_str!("translations/ar.json")),
        ("be", include_str!("translations/be.json")),
        ("bg", include_str!("translations/bg.json")),
        ("bn", include_str!("translations/bn.json")),
        ("ca", include_str!("translations/ca.json")),
        ("cs", include_str!("translations/cs.json")),
        ("da", include_str!("translations/da.json")),
        ("de", include_str!("translations/de.json")),
        ("es", include_str!("translations/es.json")),
        ("et", include_str!("translations/et.json")),
        ("eu", include_str!("translations/eu.json")),
        ("fa", include_str!("translations/fa.json")),
        ("fi", include_str!("translations/fi.json")),
        ("fr", include_str!("translations/fr.json")),
        ("gu", include_str!("translations/gu.json")),
        ("he", include_str!("translations/he.json")),
        ("hi", include_str!("translations/hi.json")),
        ("hr", include_str!("translations/hr.json")),
        ("hu", include_str!("translations/hu.json")),
        ("it", include_str!("translations/it.json")),
        ("ja", include_str!("translations/ja.json")),
        ("ko", include_str!("translations/ko.json")),
        ("lt", include_str!("translations/lt.json")),
        ("nb", include_str!("translations/nb.json")),
        ("nl", include_str!("translations/nl.json")),
        ("pl", include_str!("translations/pl.json")),
        ("pt", include_str!("translations/pt.json")),
        ("pt-BR", include_str!("translations/pt-BR.json")),
        ("ro", include_str!("translations/ro.json")),
        ("ru", include_str!("translations/ru.json")),
        ("sk", include_str!("translations/sk.json")),
        ("sl", include_str!("translations/sl.json")),
        ("sv", include_str!("translations/sv.json")),
        ("tr", include_str!("translations/tr.json")),
        ("uk", include_str!("translations/uk.json")),
        ("vi", include_str!("translations/vi.json")),
        ("zh-Hans", include_str!("translations/zh-Hans.json")),
        ("zh-Hant", include_str!("translations/zh-Hant.json")),
        ("test", include_str!("translations/test.json")),
    ]);

    let config = I18nProviderConfig {
        translations: translations,
        default_language: "en".to_string(), // Always default to English
        onerror: Callback::from(|error: String| {
            web_sys::console::log_1(&format!("i18nrs error: {}", error).into());
        }),
        ..Default::default()
    };

    html! {
        <I18nProvider ..config>
            <LanguageManager />
        </I18nProvider>
    }
}

#[function_component(LanguageManager)]
fn language_manager() -> Html {
    let (_i18n, set_language) = use_translation();

    // Only subscribe to the auth fields that actually affect language selection.
    // This prevents the entire app tree from re-rendering on episode save/download/etc.
    let auth_sel = use_selector(|state: &AppState| {
        (state.auth_details.clone(), state.user_details.clone())
    });

    {
        let set_language = set_language.clone();

        use_effect_with(auth_sel, move |auth_sel| {
            let set_language = set_language.clone();
            let (auth_details, user_details) = (**auth_sel).clone();

            spawn_local(async move {
                let server_name = web_sys::window()
                    .and_then(|w| w.location().origin().ok())
                    .unwrap_or_else(|| "".to_string());

                if let (Some(auth_details), Some(user_details)) = (auth_details, user_details) {
                    if let Some(api_key) = &auth_details.api_key {
                        match crate::requests::setting_reqs::call_get_user_language(
                            server_name,
                            api_key.clone(),
                            user_details.UserID,
                        )
                        .await
                        {
                            Ok(user_lang) => {
                                set_language.emit(user_lang);
                            }
                            Err(_) => {
                                if let Ok(server_lang) = call_get_server_default_language(
                                    auth_details.server_name.clone(),
                                )
                                .await
                                {
                                    set_language.emit(server_lang);
                                } else {
                                    set_language.emit("en".to_string());
                                }
                            }
                        }
                    }
                } else {
                    if !server_name.is_empty() {
                        match call_get_server_default_language(server_name).await {
                            Ok(server_lang) => {
                                set_language.emit(server_lang);
                            }
                            Err(_) => {
                                set_language.emit("en".to_string());
                            }
                        }
                    } else {
                        set_language.emit("en".to_string());
                    }
                }
            });
            || {}
        });
    }

    // Desktop only: load the set of episodes downloaded to THIS device once on startup,
    // so local-first playback works from any list (home, feed, search, playlists) and not
    // just the pages that already fetch local downloads (Downloads/Saved/Feed/Queue).
    #[cfg(not(feature = "server_build"))]
    {
        use_effect_with((), move |_| {
            spawn_local(async move {
                if let Ok(mut local_episodes) =
                    crate::pages::downloads_tauri::fetch_local_episodes().await
                {
                    Dispatch::<crate::components::context::EpisodeStatusState>::global()
                        .reduce_mut(move |s| {
                            s.downloaded_episodes.clear_local();
                            for ep in local_episodes.drain(..) {
                                s.downloaded_episodes.push_local(ep);
                            }
                        });
                }
            });
            || ()
        });
    }

    html! {
        <BrowserRouter>
            <NavigationHandler>
                <Switch<Route> render={switch} />
            </NavigationHandler>
            <RestoreOverlay />
            <CollectionPickerModal />
        </BrowserRouter>
    }
}

#[function_component(Main)]
fn main_component() -> Html {
    html! {
        <LanguageHandler />
    }
}

fn main() {
    yew::Renderer::<Main>::new().render();
}
