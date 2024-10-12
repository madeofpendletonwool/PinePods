use std::collections::HashMap;
use yew::prelude::*;
use wasm_bindgen::JsCast;
use yewdux::prelude::*;
use crate::components::context::{UIState, AppState};
use web_sys::{FileReader, HtmlInputElement};
use wasm_bindgen::closure::Closure;
use crate::components::gen_funcs::parse_opml;
use crate::requests::pod_req::{call_add_podcast, PodcastValues};
use crate::requests::search_pods::{call_parse_podcast_channel_info, PodcastInfo};
use gloo::timers::callback::Interval;
// use wasm_bindgen::JsValue;
use crate::requests::setting_reqs::{call_podcast_opml_import, fetch_import_progress};
use wasm_bindgen::JsValue;
use std::cell::RefCell;
use std::rc::Rc;


fn transform_feed_result_to_values(feed_result: PodcastInfo, podcast_to_add: &PodcastToAdd, user_id: i32) -> PodcastValues {
    let pod_title = podcast_to_add.title.clone();
    let pod_feed_url = podcast_to_add.xml_url.clone();


    // Simplified: Using first episode details or default values
    let pod_artwork = feed_result.artwork_url.unwrap_or_default();
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

#[derive(Clone, Debug)]
struct PodcastToImport {
    title: String,
    xml_url: String,
    selected: bool,
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
    let (_audio_state, audio_dispatch) = use_store::<UIState>();
    let import_progress = use_state(|| 0);
    let total_podcasts = use_state(|| 0);
    let current_podcast = use_state(String::default);


    let onclick = {
        let import_pods = import_pods.clone();
        let show_verification = show_verification.clone();
        Callback::from(move |e: Event| {
            // let server_name = server_name.clone();
            let show_verification = show_verification.clone();
            let import_pods = import_pods.clone();
            let file_list = e.target_unchecked_into::<HtmlInputElement>().files();
            if let Some(files) = file_list {
                if let Some(file) = files.get(0) {
                    let reader = FileReader::new().unwrap();
                    let onload = Closure::wrap(Box::new(move |e: ProgressEvent| {
                        let reader: FileReader = e.target().unwrap().dyn_into().unwrap();
                        if let Ok(text) = reader.result() {
                            let text = text.as_string().unwrap();
                            let import_data: Vec<PodcastToImport> = parse_opml(&text)
                                .into_iter()
                                .map(|(title, xml_url)| PodcastToImport { title, xml_url, selected: true })
                                .collect();
                            import_pods.set(import_data);
                            show_verification.set(true);
                        }
                    }) as Box<dyn FnMut(_)>);
                    reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                    reader.read_as_text(&file).unwrap();
                    onload.forget(); // This is necessary to avoid the closure being cleaned up
                }
            }
        })
    };
    
    let server_name_confirm = server_name.clone();
    let dispatch_wasm = _dispatch.clone();



    let on_confirm = {
        let import_pods = import_pods.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let user_id = user_id.clone();
        let import_progress = import_progress.clone();
        let total_podcasts = total_podcasts.clone();
        let current_podcast = current_podcast.clone();
        let dispatch_wasm_conf = dispatch_wasm.clone();
    
        Callback::from(move |_| {
            let dispatch_wasm_call = dispatch_wasm_conf.clone();
            let audio_dispatch_call = audio_dispatch.clone();
            dispatch_wasm_call.reduce_mut(|state| state.is_loading = Some(true));
            let selected_podcasts: Vec<String> = (*import_pods)
                .iter()
                .filter(|podcast| podcast.selected)
                .map(|podcast| podcast.xml_url.clone())
                .collect();
    
            total_podcasts.set(selected_podcasts.len());
    
            wasm_bindgen_futures::spawn_local({
                let server_name = server_name.clone();
                let api_key = api_key.clone();
                let user_id = user_id.clone();
                let import_progress = import_progress.clone();
                let current_podcast = current_podcast.clone();
                let total_podcasts = total_podcasts.clone();
    
                async move {
                    if let (Some(server_name), Some(api_key), Some(user_id)) = (server_name.clone(), api_key.clone(), user_id) {
                        match call_podcast_opml_import(&server_name, &Some(api_key.clone().unwrap()), user_id, selected_podcasts.clone()).await {
                            Ok(_) => {
                                let interval: Rc<RefCell<Option<Interval>>> = Rc::new(RefCell::new(None));
                                let interval_clone = interval.clone();
                                web_sys::console::log_1(&JsValue::from_str("opml import success"));
                            
                                let callback = Closure::wrap(Box::new(move || {
                                    let dispatch_wasm = dispatch_wasm_call.clone();
                                    let audio_dispatch = audio_dispatch_call.clone();
                                    let server_name = server_name.clone();
                                    let api_key = api_key.clone();
                                    let user_id = user_id;
                                    let import_progress = import_progress.clone();
                                    let current_podcast = current_podcast.clone();
                                    let total_podcasts = total_podcasts.clone();
                                    let interval = interval_clone.clone();
                                    web_sys::console::log_1(&JsValue::from_str("pod closure"));
                            
                                    wasm_bindgen_futures::spawn_local(async move {
                                        match fetch_import_progress(&server_name, &api_key, user_id).await {
                                            Ok((current, total, podcast)) => {
                                                web_sys::console::log_1(&JsValue::from_str("got progress"));
                                                import_progress.set(current);
                                                total_podcasts.set(total as usize);
                                                current_podcast.set(podcast);
                                                if current >= total {
                                                    // Import is complete, stop polling
                                                    if let Some(interval) = interval.borrow_mut().take() {
                                                        interval.cancel();
                                                    }
                                                    dispatch_wasm.reduce_mut(|state| state.is_loading = Some(false));
                                                    audio_dispatch.reduce_mut(|audio_state| audio_state.info_message = Option::from("OPML Import Completed!".to_string()));
                                                }
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(&JsValue::from_str("progress failed"));
                                                log::error!("Failed to fetch import progress: {:?}", e);
                                            }
                                        }
                                    });
                                }) as Box<dyn Fn()>);
                            
                                interval.borrow_mut().replace(Interval::new(5000, move || {
                                    callback.as_ref().unchecked_ref::<js_sys::Function>().call0(&JsValue::NULL).unwrap();
                                    // Return () explicitly
                                    ()
                                }));
                            }
                            Err(e) => {
                                log::error!("Failed to import OPML: {:?}", e);
                                dispatch_wasm_call.reduce_mut(|state| state.is_loading = Some(false));
                                audio_dispatch_call.reduce_mut(|audio_state| audio_state.info_message = Option::from("Failed to import OPML".to_string()));
                            }
                        }
                    }
                }
            });
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
            {
                if *show_verification {
                    html! {
                        <div class="import-box">
                            <div>
                                <p class="item_container-text">
                                    {"The following podcasts were found. Please unselect any podcasts you don't want to add, and then click the button below. A large amount of podcasts will take a little while to parse all the feeds and add them. The loading animation will disappear once all complete. Be patient!"}
                                </p>
                                <button class="settings-button" onclick={on_confirm}>{"Add them!"}</button>
                            </div>
                            <div class="mt-4">
                                <p class="item_container-text">
                                    {format!("Progress: {}/{}", *import_progress, *total_podcasts)}
                                </p>
                                <p class="item_container-text">
                                    {format!("Currently importing: {}", *current_podcast)}
                                </p>
                            </div>
                            {
                                for (*import_pods).iter().enumerate().map(|(index, podcast)| {
                                    let toggle_selection = {
                                        let import_pods = import_pods.clone();
                                        Callback::from(move |_| {
                                            let mut new_import_pods = (*import_pods).clone();
                                            new_import_pods[index].selected = !new_import_pods[index].selected;
                                            import_pods.set(new_import_pods);
                                        })
                                    };
                                
                                    html! {
                                        <div class="podcast import-list">
                                            <label onclick={toggle_selection}>
                                                <input type="checkbox" checked={podcast.selected} />
                                                <span class="item_container-text">{format!("{} - {}", podcast.title, podcast.xml_url)}</span>
                                            </label>
                                        </div>
                                    }
                                })                                
                                
                            }
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>
        
    }
}