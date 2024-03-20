use yew::prelude::*;
use wasm_bindgen::JsCast;
use yewdux::prelude::*;
use crate::components::context::AppState;
use web_sys::{window, Blob, Url, BlobPropertyBag};
use wasm_bindgen::JsValue;
use crate::requests::setting_reqs::{call_backup_user};

#[function_component(ExportOptions)]
pub fn export_options() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
        
    let mut blob_property_bag = BlobPropertyBag::new();
    blob_property_bag.type_("text/xml");
    
    let onclick = {
        let blob_property_bag = blob_property_bag.clone();
        Callback::from(move |_| {
            let bloberty_bag = blob_property_bag.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_backup_user(&server_name.unwrap(), user_id.unwrap(), &api_key.unwrap().unwrap()).await {
                    Ok(opml_content) => {
                        // Wrap the OPML content in an array and convert to JsValue
                        let array = js_sys::Array::new();
                        array.push(&JsValue::from_str(&opml_content));
                        
                        // Create a new blob from the OPML content
                        let blob = Blob::new_with_str_sequence_and_options(&array, &bloberty_bag).unwrap();
                        let url = Url::create_object_url_with_blob(&blob).unwrap();
    
                        // Trigger the download
                        if let Some(window) = window() {
                            let document = window.document().unwrap();
                            let a = document.create_element("a").unwrap().dyn_into::<web_sys::HtmlAnchorElement>().unwrap();
                            a.set_href(&url);
                            a.set_download("podcasts.opml");
                            a.click();
    
                            // Revoke the object URL to free up resources
                            Url::revoke_object_url(&url).unwrap();
                        }
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Error exporting OPML: {:?}", e).into());
                    }
                }
            });
        })
    };



    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{"Export Options:"}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{"You can export an OPML file containing your Podcasts here. This file can then be imported if you want to switch to a different podcast app or simply want a backup of your files just in case. Note, if you are exporting to add your podcasts to AntennaPod the Nextcloud Options below might better suit your needs. If you're an admin a full server backup might be a better solution as well on the Admin Settings Page."}</p> // Styled paragraph

            <button onclick={onclick} class="mt-4 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                {"Download/Export OPML"}
            </button>
        </div>
    }
}

