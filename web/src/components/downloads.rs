use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(Downloads)]
pub fn downloads() -> Html {
    html! {
        <div>
            <h1>{ "Downloads" }</h1>
            <App_drawer />
        </div>
    }
}
