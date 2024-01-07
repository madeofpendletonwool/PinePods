use yew::{function_component, Html, html};
use yew::prelude::*;
use super::app_drawer::{App_drawer};
use super::gen_components::Search_nav;
use crate::requests::pod_req;
use web_sys::console;
use yewdux::prelude::*;
use crate::components::context::{AppState};
use std::rc::Rc;
use serde::de::Unexpected::Option;


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

        // Dependencies for use_effect_with
        let dependencies = (
            state.auth_details.as_ref().map(|ud| ud.api_key.clone()),
            state.user_details.as_ref().map(|ud| ud.UserID.clone()),
            state.auth_details.as_ref().map(|ud| ud.server_name.clone()),
        );

        console::log_1(&format!("apikey: {:?}", &api_key).into());
        console::log_1(&format!("userid: {:?}", &user_id).into());
        console::log_1(&format!("servername: {:?}", &server_name).into());

        // if let (Some(api_key), Some(user_id), Some(server_name)) = (api_key.clone(), user_id.clone(), server_name.clone()) {
        //     console::log_1(&format!("Server Name: {}", server_name).into());
        //
        //     wasm_bindgen_futures::spawn_local(async move {
        //         match pod_req::call_get_recent_eps(&server_name, &api_key, user_id).await {
        //             Ok(fetched_episodes) => {
        //                 if fetched_episodes.is_empty() {
        //                     // If no episodes are returned, set episodes state to an empty vector
        //                     console::log_1(&format!("Server Name: {:?}", &episodes).into());
        //                     episodes.set(Vec::new());
        //                 } else {
        //                     // Set episodes state to the fetched episodes
        //                     console::log_1(&format!("Server Name: {:?}", &episodes).into());
        //                     episodes.set(fetched_episodes);
        //                 }
        //             },
        //             Err(e) => error.set(Some(e.to_string())),
        //         }
        //     });
        // }
        let server_name_effect = server_name.clone();
        let user_id_effect = user_id.clone();
        let api_key_effect = api_key.clone();

        use_effect_with(
            (api_key_effect, user_id_effect, server_name_effect),
            move |_| {
                console::log_1(&format!("User effect running: {:?}", &server_name).into());
                let episodes_clone = episodes.clone();
                let error_clone = error.clone();

                if let (Some(api_key), Some(user_id), Some(server_name)) = (api_key.clone(), user_id, server_name.clone()) {
                    console::log_1(&format!("in some: {:?}", &server_name).into());

                    wasm_bindgen_futures::spawn_local(async move {
                        match pod_req::call_get_recent_eps(&server_name, &api_key, &user_id).await {
                            Ok(fetched_episodes) => {
                                if fetched_episodes.is_empty() {
                                    console::log_1(&format!("episodes empty: {:?}", &server_name).into());
                                    episodes_clone.set(Vec::new());
                                } else {
                                    console::log_1(&format!("Getting episodes: {:?}", &server_name).into());
                                    episodes_clone.set(fetched_episodes);
                                }
                            },
                            Err(e) => error_clone.set(Some(e.to_string())),
                        }
                    });
                }
                || ()
            },
        );

    }

    html! {
    <>
        <div class="episodes-container">
            <Search_nav />
            {
                if episodes.is_empty() {
                    html! {
                        <>
                            <div class="empty-episodes-container">
                                <img src="static/assets/favicon.png" alt="Logo" class="logo"/>
                                <h1>{ "No Recent Episodes Found" }</h1>
                                <p>{"You can add new podcasts by using the search bar above. Search for your favorite podcast and click the plus button to add it."}</p>
                            </div>
                        </>
                    }
                } else {
                    episodes.iter().map(|episode| html! {
                        <div>
                            <div class="episode">
                                // Add image, title, date, duration, and buttons here
                                <p>{ &episode.PodcastName }</p>
                                <p>{ &episode.EpisodeTitle }</p>
                                // ... other fields
                            </div>
                        </div>
                    }).collect::<Html>()
                }
            }
            {
                if let Some(error_message) = &*error {
                    html! { <div class="error-snackbar">{ error_message }</div> }
                } else {
                    html! { <></> }
                }
            }
        </div>
        <App_drawer />
    </>
}


}
