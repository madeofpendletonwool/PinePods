use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(SearchNew)]
pub fn search_new() -> Html {
    html! {
        <div>
            <h1>{ "Search New" }</h1>
            <App_drawer />
        </div>
    }
}
