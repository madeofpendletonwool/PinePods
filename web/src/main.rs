mod components;
use components::login::Login;
use components::app_drawer::App_drawer;

use yew::prelude::*;

#[function_component(App)]
fn app() -> Html {
    html! {
        <div>
            <Login />
            <App_drawer />
            // ... other components or HTML
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}