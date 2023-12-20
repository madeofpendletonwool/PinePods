use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(UserStats)]
pub fn user_stats() -> Html {
    html! {
        <div>
            <h1>{ "User Stats" }</h1>
            <App_drawer />
        </div>
    }
}
