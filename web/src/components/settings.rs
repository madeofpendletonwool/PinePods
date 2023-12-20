use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(Settings)]
pub fn settings() -> Html {
    html! {
        <div>
            <h1>{ "Settings" }</h1>
            <App_drawer />
        </div>
    }
}
