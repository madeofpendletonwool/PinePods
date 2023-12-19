// Custom Mods
mod components;
mod requests;
use components::login::Login;
use components::login::ChangeServer;
use components::app_drawer::App_drawer;
use requests::login_requests;


// Yew Imports
use yew_router::prelude::*;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};


#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Login,
    #[at("/home")]
    Home,
    #[not_found]
    #[at("/404")]
    NotFound,
    #[at("/change_server")]
    ChangeServer,
}

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