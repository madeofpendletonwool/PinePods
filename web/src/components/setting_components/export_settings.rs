use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::call_backup_user;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{window, Blob, BlobPropertyBag, Url};
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;

#[function_component(ExportOptions)]
pub fn export_options() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());

    let blob_property_bag = BlobPropertyBag::new();
    blob_property_bag.set_type("text/xml");

    let onclick = {
        let blob_property_bag = blob_property_bag.clone();
        // Capture translated message before move
        let error_prefix = i18n.t("export_settings.error_exporting_opml").to_string();
        Callback::from(move |_| {
            let error_prefix = error_prefix.clone();
            let _dispatch = _dispatch.clone();
            let bloberty_bag = blob_property_bag.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_backup_user(
                    &server_name.unwrap(),
                    user_id.unwrap(),
                    &api_key.unwrap().unwrap(),
                )
                .await
                {
                    Ok(opml_content) => {
                        // Wrap the OPML content in an array and convert to JsValue
                        let array = js_sys::Array::new();
                        array.push(&JsValue::from_str(&opml_content));

                        // Create a new blob from the OPML content
                        let blob =
                            Blob::new_with_str_sequence_and_options(&array, &bloberty_bag).unwrap();
                        let url = Url::create_object_url_with_blob(&blob).unwrap();

                        // Trigger the download
                        if let Some(window) = window() {
                            let document = window.document().unwrap();
                            let a = document
                                .create_element("a")
                                .unwrap()
                                .dyn_into::<web_sys::HtmlAnchorElement>()
                                .unwrap();
                            a.set_href(&url);
                            a.set_download("podcasts.opml");
                            a.click();

                            // Revoke the object URL to free up resources
                            Url::revoke_object_url(&url).unwrap();
                        }
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        _dispatch.reduce_mut(|audio_state| {
                            audio_state.error_message =
                                Option::from(format!("{}{}", error_prefix.clone(), formatted_error))
                        });
                    }
                }
            });
        })
    };

    html! {
        <div class="p-4"> // You can adjust the padding as needed
            <p class="item_container-text text-lg font-bold mb-4">{i18n.t("export_settings.export_options")}</p> // Styled paragraph
            <p class="item_container-text text-md mb-4">{i18n.t("export_settings.export_description")}</p> // Styled paragraph

            <button onclick={onclick} class="mt-4 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                {i18n.t("export_settings.download_export_opml")}
            </button>
        </div>
    }
}
