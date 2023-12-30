use yew::{function_component, Html, html};
use super::app_drawer::App_drawer;

#[function_component(EpisodeLayout)]
pub fn episode_layout() -> Html {
    html! {
        <div>
            <h1>{ "episode New" }</h1>
            <App_drawer />
        </div>
    }
}
