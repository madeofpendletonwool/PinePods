use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(PodHistory)]
pub fn history() -> Html {
    html! {
        <div>
            <h1>{ "History" }</h1>
            <App_drawer />
        </div>
    }
}
