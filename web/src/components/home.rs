use yew::{function_component, Html, html};
use yew::prelude::*;
use super::app_drawer::App_drawer;
use crate::requests::pod_req;
use web_sys::console;
use yewdux::prelude::*;
use crate::components::context::{AppState};
use std::rc::Rc;

// #[function_component(Home)]
// pub fn home() -> Html {
//     html! {
//         <div>
//             <h1>{ "Home" }</h1>
//             <App_drawer />
//         </div>
//     }
// }


#[function_component(Home)]
pub fn home() -> Html {
    // State to store episodes
    let episodes = use_state(|| Vec::new());
    let error = use_state(|| None);
    let dispatch = Dispatch::<AppState>::global();
    let state: Rc<AppState> = dispatch.get();
    console::log_1(&format!("User Context in Home: {:?}", &state.user_details).into());
    // Fetch episodes on component mount
    {
        let episodes = episodes.clone();
        let error = error.clone();
        let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
        let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
        let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        console::log_1(&"Test log on home".to_string().into());
        if let Some(api_key) = &api_key {
            console::log_1(&format!("API Key: {:?}", api_key).into());
        }
        if let Some(user_id) = user_id {
            console::log_1(&format!("User ID: {}", user_id).into());
        }
        if let Some(server_name) = &server_name {
            console::log_1(&format!("Server Name: {}", server_name).into());
        }

        use_effect(move || {
            if let (Some(api_key), Some(user_id)) = (api_key.clone(), user_id) {
                if let Some(server_name) = &server_name {
                    console::log_1(&format!("Server Name: {}", server_name).into());
                }
                wasm_bindgen_futures::spawn_local(async move {
                    match pod_req::call_get_recent_eps(server_name, api_key, user_id).await {
                        Ok(response) => episodes.set(response.episodes),
                        Err(e) => error.set(Some(e.to_string())),
                    }
                });
            }
            || ()
        })
    }

    html! {
    <>
        <div class="episodes-container">
            {
                for (*episodes).iter().map(|episode| html! {
                    <div>
                        <div class="episode">
                            // Add image, title, date, duration, and buttons here
                            <p>{ &episode.PodcastName }</p>
                            <p>{ &episode.EpisodeTitle }</p>
                            // ... other fields
                        </div>
                    </div>
                })
            }
            {
                if let Some(error_message) = &*error {
                    html! { <div class="error-snackbar">{ error_message }</div> }
                } else {
                    html! { <></> } // Empty fragment for the else case
                }
            }
        </div>
        <App_drawer />
    </>
}


}
