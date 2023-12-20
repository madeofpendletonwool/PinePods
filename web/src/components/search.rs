use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(Search)]
pub fn search() -> Html {
    html! {
        <div>
            <h1>{ "Search" }</h1>
            <App_drawer />
        </div>
    }
}
