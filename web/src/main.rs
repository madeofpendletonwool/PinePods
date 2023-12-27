// Custom Mods
mod components;
mod requests;
use components::routes::Route;
use components::login::Login;
use components::login::ChangeServer;
use components::login::LogOut;
use components::downloads::Downloads;
use components::history::PodHistory;
use components::queue::Queue;
use components::saved::Saved;
use components::search::Search;
use components::settings::Settings;
use components::user_stats::UserStats;
use components::home::Home;
use components::context;
use crate::requests::login_requests::{LoginServerRequest, GetUserDetails};
use web_sys::console;


// Yew Imports
use yew_router::prelude::*;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};

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
    // console::log_1(&format!("Initial User Context: {:?}", (*user_context).clone()).into());
    // console::log_1(&format!("Initial Auth Context: {:?}", (*user_auth_context).clone()).into());

    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<Main>::new().render();
}