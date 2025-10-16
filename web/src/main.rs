// Custom Mods
mod components;
mod requests;

#[cfg(test)]
mod tests;

use components::routes::Route;
// use components::login::Login;
// use components::login::ChangeServer;
// use components::login::LogOut;
use components::downloads::Downloads;
use components::episode::Episode;
use components::episodes_layout::EpisodeLayout;
use components::feed::Feed;
use components::history::PodHistory;
use components::home::Home;
use components::navigation::NavigationHandler;
use components::oauth_callback::OAuthCallback;
use components::people_subs::SubscribedPeople;
use components::person::Person;
use components::playlist_detail::PlaylistDetail;
use components::playlists::Playlists;
use components::podcast_layout::PodLayout;
use components::podcasts::Podcasts;
use components::queue::Queue;
use components::saved::Saved;
use components::search::Search;
use components::search_new::SearchNew;
use components::settings::Settings;
use components::shared_episode::SharedEpisode;
use components::user_stats::UserStats;
use components::youtube_layout::YouTubeLayout;
use yew_router::history::BrowserHistory;
use yew_router::history::History;

#[cfg(feature = "server_build")]
use {components::login::ChangeServer, components::login::LogOut, components::login::Login};

#[cfg(not(feature = "server_build"))]
use {
    components::downloads_tauri::Downloads as LocalDownloads,
    components::login_tauri::ChangeServer, components::login_tauri::LogOut,
    components::login_tauri::Login,
};

// Yew Imports
use components::context::AppState;
use i18nrs::yew::use_translation;
use i18nrs::yew::{I18nProvider, I18nProviderConfig};
use requests::setting_reqs::call_get_server_default_language;
use std::collections::HashMap;
use wasm_bindgen_futures::spawn_local;
use web_sys;
use yew::prelude::*;
use yew_router::prelude::*;
use yewdux::prelude::*;

#[function_component(NotFound)]
pub fn not_found() -> Html {
    let on_home_click = Callback::from(|e: MouseEvent| {
        e.prevent_default();
        let history = BrowserHistory::new();
        history.push("/home");
    });
    html! {
        <div class="flex flex-col items-center justify-center min-h-screen p-8">
            <div class="flex flex-col items-center text-center max-w-md space-y-6">
                <div class="flex items-center gap-4 mb-4">
                    <i class="ph ph-warning-circle text-8xl item_container-text opacity-80" />
                    <span class="text-8xl font-bold item_container-text opacity-80">{"404"}</span>
                </div>

                <h1 class="text-3xl font-bold item_container-text">
                    {"Page Not Found"}
                </h1>

                <p class="text-lg item_container-text opacity-80">
                    {"Looks like we've wandered into uncharted territory!"}
                </p>

                <div class="flex items-center gap-2 text-lg item_container-text opacity-70">
                    <i class="ph ph-coffee-bean text-2xl" />
                    <span>{"Grab some coffee and try again"}</span>
                    <i class="ph ph-coffee text-2xl" />
                </div>

                <button
                    onclick={on_home_click}
                    class="flex items-center gap-2 px-6 py-3 mt-4 rounded-lg transition-all
                        item_container-text border-2 border-current hover:opacity-80
                        active:scale-95 text-lg font-medium"
                >
                    <i class="ph ph-house-line text-xl" />
                    {"Head back home"}
                </button>

                <img
                    src="static/assets/favicon.png"
                    alt="Pinepods Logo"
                    class="w-16 h-16 mt-8 opacity-60"
                />
            </div>
        </div>
    }
}
fn switch(route: Route) -> Html {
    match route {
        Route::Login => html! { <Login /> },
        Route::Home => html! { <Home /> },
        Route::Feed => html! { <Feed /> },
        Route::NotFound => html! { <NotFound /> },
        Route::ChangeServer => html! { <ChangeServer /> },
        Route::Queue => html! { <Queue /> },
        Route::Saved => html! { <Saved /> },
        Route::Settings => html! { <Settings /> },
        Route::PodHistory => html! { <PodHistory /> },
        Route::SubscribedPeople => html! { <SubscribedPeople /> },
        Route::Downloads => html! { <Downloads /> },
        Route::Search => html! { <Search on_search={Callback::from(move |_| {})} /> },
        Route::UserStats => html! { <UserStats /> },
        Route::LogOut => html! { <LogOut /> },
        Route::SearchNew => html! { <SearchNew /> },
        Route::PodLayout => html! { <PodLayout /> },
        Route::SharedEpisode { url_key } => html! { <SharedEpisode url_key={url_key.clone()} /> },
        Route::EpisodeLayout => html! { <EpisodeLayout /> },
        Route::Podcasts => html! { <Podcasts /> },
        Route::YoutubeLayout => html! { <YouTubeLayout /> },
        Route::Playlists => html! { <Playlists /> },
        Route::Episode => html! { <Episode /> },
        Route::Person { name } => html! { <Person name={name.clone()} /> },
        Route::OAuthCallback => html! { <OAuthCallback /> },
        Route::PlaylistDetail { id } => html! { <PlaylistDetail {id} /> },
        #[cfg(not(feature = "server_build"))]
        Route::LocalDownloads => html! { <LocalDownloads /> },
        #[cfg(feature = "server_build")]
        Route::LocalDownloads => {
            html! { <div>{"Local downloads not available on the web version"}</div> }
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
        ("zh", include_str!("translations/zh.json")),
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
    let (_state, _) = use_store::<AppState>();
    let (_i18n, set_language) = use_translation();

    // Load appropriate language based on auth state
    {
        let set_language = set_language.clone();
        let state = _state.clone();

        use_effect_with(state.clone(), move |state| {
            let set_language = set_language.clone();
            let state = state.clone();

            spawn_local(async move {
                let server_name = web_sys::window()
                    .and_then(|w| w.location().origin().ok())
                    .unwrap_or_else(|| "".to_string());

                // Check if user is authenticated
                if let (Some(auth_details), Some(user_details)) =
                    (&state.auth_details, &state.user_details)
                {
                    // User is logged in, get their language preference
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
                                // Fall back to server default
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
                    // User not logged in, use server default
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

    html! {
        <BrowserRouter>
            <NavigationHandler>
                <Switch<Route> render={switch} />
            </NavigationHandler>
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
