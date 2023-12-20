use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(Saved)]
pub fn saved() -> Html {
    html! {
        <div>
            <h1>{ "Saved" }</h1>
            <App_drawer />
        </div>
    }
}
