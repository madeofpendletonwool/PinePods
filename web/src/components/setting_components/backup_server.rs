use yew::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{window, Blob, Url, BlobPropertyBag};
use wasm_bindgen::JsValue;
use yewdux::prelude::*;
use crate::components::audio;
use crate::components::context::{UIState, AppState};
use crate::requests::setting_reqs::{call_backup_server};

#[function_component(BackupServer)]
pub fn backup_server() -> Html {
    let database_password = use_state(|| "".to_string());
    let (state, _dispatch) = use_store::<AppState>();
    let (audio_state, audio_dispatch) = use_store::<UIState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let blob_property_bag = BlobPropertyBag::new();

    let on_download_click = {
        let database_password = database_password.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let blob_property_bag = blob_property_bag.clone();
        let audio_dispatch_call = audio_dispatch.clone();
        Callback::from(move |_| {
            let audio_dispatch_call = audio_dispatch_call.clone();
            let db_pass = (*database_password).trim().to_string();
            if db_pass.is_empty() {
                // Directly set the error message state if the password is empty
                audio_dispatch_call.reduce_mut(|audio_state| {
                    web_sys::console::log_1(&"Database password cannot be empty.".into());
                    audio_dispatch.reduce_mut(|audio_state| audio_state.error_message = Option::from("Database password cannot be empty.".to_string()));

                });
                // Optionally, return early or perform other actions
                return;
            }
            let api_key = api_key.clone().unwrap_or_default();
            let server_name = server_name.clone().unwrap_or_default();
            let bloberty_bag = blob_property_bag.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match call_backup_server(&server_name, &db_pass, &api_key.unwrap()).await {
                    Ok(backup_data) => {
                        let array = js_sys::Array::new();
                        array.push(&JsValue::from_str(&backup_data));
                        
                        // let blob_property_bag = BlobPropertyBag::new().type_("text/plain");
                        let blob = Blob::new_with_str_sequence_and_options(&array, &bloberty_bag).unwrap();
                        let url = Url::create_object_url_with_blob(&blob).unwrap();

                        if let Some(window) = window() {
                            let document = window.document().unwrap();
                            let a = document.create_element("a").unwrap().dyn_into::<web_sys::HtmlAnchorElement>().unwrap();
                            a.set_href(&url);
                            a.set_download("server_backup.sql");
                            a.click();

                            Url::revoke_object_url(&url).unwrap();
                        }
                    },
                    Err(e) => {
                        web_sys::console::log_1(&format!("Error backing up server: {:?}", e).into());
                        audio_dispatch_call.reduce_mut(|audio_state| audio_state.error_message = Option::from(format!("Error backing up server - Maybe wrong password?: {}", e).to_string()));
                    }
                }
            });
        })
    };

    html! {
        <div class="p-4">
            <p class="item_container-text text-lg font-bold mb-4">{"Backup Server Data:"}</p>
            <p class="item_container-text text-md mb-4">{"Download a backup of the entire server database here. This includes all users, podcasts, episodes, settings, and API keys. Use this to migrate to a new server or restore your current server."}</p>
            <br/>
            <div class="flex items-center">
                <input type="text" id="db=pw"                    
                oninput={Callback::from(move |e: InputEvent| {
                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                    database_password.set(input.value());
                })} 
                class="search-bar-input border text-sm rounded-lg block w-full p-2.5" placeholder="mYDBp@ss!" />
                <button onclick={on_download_click} class="mt-2 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                {"Authenticate"}
                </button>
            </div>
        </div>
    }
}
