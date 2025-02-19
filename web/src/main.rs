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
use yew::prelude::*;
use yew_router::prelude::*;

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
        Route::Episode => html! { <Episode /> },
        Route::Person { name } => html! { <Person name={name.clone()} /> },
        Route::OAuthCallback => html! { <OAuthCallback /> },
        Route::PlaylistDetail { id } => html! { <PlaylistDetail {id} /> },
        #[cfg(not(feature = "server_build"))]
        Route::LocalDownloads => html! { <LocalDownloads /> },
        #[cfg(feature = "server_build")]
        Route::LocalDownloads => html! { <div>{"Local downloads not available on the web"}</div> },
    }
}

#[function_component(Main)]
fn main_component() -> Html {
    // console::log_1(&format!("Initial User Context: {:?}", (*user_context).clone()).into());
    // console::log_1(&format!("Initial Auth Context: {:?}", (*user_auth_context).clone()).into());

    html! {
        <BrowserRouter>
            <NavigationHandler>
                <Switch<Route> render={switch} />
            </NavigationHandler>
        </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<Main>::new().render();
}
