use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(Queue)]
pub fn queue() -> Html {
    html! {
        <div>
            <h1>{ "Queue" }</h1>
            <App_drawer />
        </div>
    }
}
