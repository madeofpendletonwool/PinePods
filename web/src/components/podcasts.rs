use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(Podcasts)]
pub fn podcasts() -> Html {
    html! {
        <div>
            <h1>{ "podcasts" }</h1>
            <App_drawer />
        </div>
    }
}
