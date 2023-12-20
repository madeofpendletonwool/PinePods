// Custom Mods
mod components;
mod requests;
use components::routes::Route;
use components::login::Login;
use components::login::ChangeServer;
use components::login::LogOut;
use components::app_drawer::App_drawer;
use components::downloads::Downloads;
use components::history::PodHistory;
use components::queue::Queue;
use components::saved::Saved;
use components::search::Search;
use components::settings::Settings;
use components::user_stats::UserStats;
use requests::login_requests;


// Yew Imports
use yew_router::prelude::*;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};

#[function_component(Home)]
fn home() -> Html {
    html! {
        <div>
            <h1>{ "Home" }</h1>
            <App_drawer />
        </div>
    }
}

#[function_component(NotFound)]
fn not_found() -> Html {
    html! { <h1>{ "404 Not Found" }</h1> }
}

fn switch(route: Route) -> Html {
    match route {
        Route::Login => html! { <Login /> },
        Route::Home => html! { <Home /> },
        Route::NotFound => html! { <NotFound /> },
        Route::ChangeServer => html! { <ChangeServer /> },
        Route::Queue => html! { <Queue /> },
        Route::Saved => html! { <Saved /> },
        Route::Settings => html! { <Settings /> },
        Route::PodHistory => html! { <PodHistory /> },
        Route::Downloads => html! { <Downloads /> },
        Route::Search => html! { <Search /> },
        Route::UserStats => html! { <UserStats /> },
        Route::LogOut => html! { <LogOut /> },
    }
}


#[function_component(Main)]
fn main_component() -> Html {
    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<Main>::new().render();
}