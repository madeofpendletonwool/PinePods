use std::collections::HashMap;
use yew::prelude::*;
use wasm_bindgen::JsCast;
use yewdux::prelude::*;
use crate::components::context::AppState;
use web_sys::{console, FileReader, HtmlInputElement};
use wasm_bindgen::closure::Closure;
use crate::components::gen_funcs::parse_opml;
use crate::requests::pod_req::{call_add_podcast, PodcastValues};
use crate::requests::search_pods::{call_parse_podcast_channel_info, PodcastInfo};


// use wasm_bindgen::JsValue;
// use crate::requests::setting_reqs::{call_backup_user};
fn transform_feed_result_to_values(feed_result: PodcastInfo, podcast_to_add: &PodcastToAdd, user_id: i32) -> PodcastValues {
    let pod_title = podcast_to_add.title.clone();
    let pod_feed_url = podcast_to_add.xml_url.clone();


    // Simplified: Using first episode details or default values
    let pod_artwork = feed_result.artwork_url.unwrap_or_default();
    console::log_1(&pod_artwork.clone().into());
    let pod_author = feed_result.author.clone();
    let pod_description = feed_result.description.clone();
    let pod_website = feed_result.website;
    let pod_explicit = feed_result.explicit;
    let pod_episode_count = feed_result.episode_count;


    // Placeholder for categories, as an example
    let categories = HashMap::new();

    PodcastValues {
        pod_title,
        pod_artwork,
        pod_author,
        categories,
        pod_description,
        pod_episode_count,
        pod_feed_url,
        pod_website,
        pod_explicit,
        user_id
    }
}


#[derive(Debug, Clone)]
pub struct PodcastToAdd {
    title: String,
    xml_url: String,
}

async fn add_podcasts(server_name: &str, api_key: &Option<String>, user_id: i32, podcasts: Vec<PodcastToAdd>) {
    for podcast in podcasts.into_iter() {
        // Parse podcast URL to get feed details
        match call_parse_podcast_channel_info(&podcast.xml_url).await {
            Ok(feed_result) => {
                let add_podcast = PodcastToAdd {
                    title: podcast.title.clone(),
                    xml_url: podcast.xml_url.clone()
                };
                // Assuming you transform `feed_result` into `PodcastValues` needed by `call_add_podcast`
                let podcast_values = transform_feed_result_to_values(feed_result, &add_podcast, user_id);

                // Add podcast to the server
                match call_add_podcast(server_name, api_key, user_id, &podcast_values).await {
                    Ok(_) => log::info!("Podcast added successfully: {}", podcast.title.clone()),
                    Err(e) => log::error!("Failed to add podcast {}: {:?}", podcast.title.clone(), e),
                }
            },
            Err(e) => log::error!("Failed to parse podcast URL {}: {:?}", podcast.xml_url, e),
        }
    }
}



#[function_component(ImportOptions)]
pub fn import_options() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let import_pods = use_state(|| Vec::new());
    let show_verification = use_state(|| false);


    // let onclick = {
    //     let import_pods = import_pods.clone();
    //     Callback::from(move |e: Event| {
    //         let import_pods = import_pods.clone();
    //         let input: HtmlInputElement = e.target_unchecked_into();
    //         if let Some(file_list) = input.files() {
    //             if let Some(file) = file_list.get(0) {
    //                 let reader = FileReader::new().unwrap();

    //                 // Create a closure for handling the file load event
    //                 let onload_closure = Closure::wrap(Box::new(move |event: web_sys::ProgressEvent| {
    //                     let reader: FileReader = event.target().unwrap().dyn_into().unwrap();
    //                     if reader.ready_state() == FileReader::DONE {
    //                         // Since `result` returns a `JsValue`, directly attempt to convert it to a string
    //                         if let Some(text) = reader.result().unwrap().as_string() {
    //                             // Parse the OPML content directly from the file reader result
    //                             let import_data = parse_opml(&text);
    //                             // Set the parsed podcasts directly
    //                             import_pods.set(import_data);
    //                         }
    //                     }
    //                 }) as Box<dyn FnMut(_)>);

    //                 reader.set_onload(Some(onload_closure.as_ref().unchecked_ref()));
    //                 reader.read_as_text(&file).unwrap();

    //                 // Forget the closure to keep it alive
    //                 onload_closure.forget();
    //             }
    //         }
    //     })
    // };
    let onclick = {
        let import_pods = import_pods.clone();
        let show_verification = show_verification.clone();
        Callback::from(move |e: Event| {
            let import_pods = import_pods.clone();
            let show_verification = show_verification.clone();
            let input: HtmlInputElement = e.target_unchecked_into();
            if let Some(file_list) = input.files() {
                if let Some(file) = file_list.get(0) {
                    let reader = FileReader::new().unwrap();
                    let reader_clone = reader.clone();
                    let onload_closure = Closure::wrap(Box::new(move |_event: ProgressEvent| {
                        let text = reader_clone.result().unwrap().as_string().unwrap();
                        let import_data = parse_opml(&text);
                        import_pods.set(import_data);
                        show_verification.set(true); // Show the verification prompt
                    }) as Box<dyn FnMut(_)>);

                    reader.set_onload(Some(onload_closure.as_ref().unchecked_ref()));
                    reader.read_as_text(&file).unwrap();
                    onload_closure.forget();
                }
            }
        })
    };

    let on_confirm = {
        let import_pods = import_pods.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        Callback::from(move |_| {
            let server_name = server_name.clone();
            let api_key = api_key.clone();
            let user_id = user_id.clone();
            let podcasts_tuples = (*import_pods).clone();
            console::log_1(&"Button clicked1".into());

            // Transform Vec<(String, String)> into Vec<PodcastToAdd>
            let podcasts: Vec<PodcastToAdd> = podcasts_tuples.into_iter().map(|(title, xml_url)| PodcastToAdd { title, xml_url }).collect();
            let podcasts_log = podcasts.clone();
            // Ensure to adjust your `add_podcasts` function call to be async or handle it appropriately
            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(server_name), Some(api_key), Some(user_id)) = (server_name.as_ref(), api_key.as_ref(), user_id) {
                    add_podcasts(server_name, &Some(api_key.clone().unwrap()), user_id, podcasts.clone()).await;
                }
            });

            log::info!("Adding podcasts: {:?}", podcasts_log);
        })
    };

    html! {
        <div class="p-4">
            <p class="item_container-text text-lg font-bold mb-4">{"Import Options:"}</p>
            <p class="item_container-text text-md mb-4">{"You can Import an OPML of podcasts here. If you're migrating from a different podcast app this is probably the solution you want. Most podcast apps allow you to export a backup of your saved podcasts to an OPML file and this option can easily import them into Pinepods."}</p>
            // <input class="settings-button" type="file" accept=".opml" onchange={onclick} />
            <label class="input-button-label" for="fileInput">{ "Choose File" }</label>
            <input id="fileInput" class="input-button" type="file" accept=".opml" onchange={onclick} />
            // Optionally display the content of the OPML file for debugging

            if *show_verification {
                <div>
                    <p class="item_container-text">{"These podcasts were found, are you sure you want to add them?"}</p>
                    <button class="settings-button" onclick={on_confirm}>{"Yes, add them"}</button>
                </div>
            }
            <div class="podcasts-list">
                {
                    for (*import_pods).iter().map(|(title, url)| {
                        html! {
                            <div class="podcast">
                                <p class="item_container-text">{format!("Title: {}", title)}</p>
                                <p class="item_container-text">{format!("URL: {}", url)}</p>
                            </div>
                        }
                    })
                }
            </div>
        </div>
    }
}