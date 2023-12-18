// Custom Mods
mod components;
use components::login::Login;
use components::app_drawer::App_drawer;

// Yew Imports
use yew_router::prelude::*;
use yew::prelude::*;

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/login")]
    Login,
    #[at("/app")]
    App,
    #[not_found]
    #[at("/404")]
    NotFound,
}

#[function_component(Login_Page)]
fn login() -> Html {
    html! {
        <div>
            <Login />
            <App_drawer />
            // ... other components or HTML
        </div>
    }
}

#[function_component(Main_App)]
fn main_app() -> Html {
    // Main app component content
    html! {
        <div>
            {"App Page"}
            // Include your app drawer component here
        </div>
    }
}
// Define your switch function
fn switch(route: Route) -> Html {
    match route {
        Route::Login => html! { <LoginPage /> },
        Route::App => html! { <MainApp /> },
        Route::NotFound => html! { <div>{"404 Not Found"}</div> },
    }
}

// Define your root component
#[function_component(App)]
fn app() -> Html {
    html! {
        <BrowserRouter>
            <Switch<Route> render={Switch::render(switch)} />
        </BrowserRouter>
    }
}

fn main() {
    yew::Renderer::<Login_Page>::new().render();
}