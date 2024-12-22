use crate::components::context::{AppState, UIState};
use crate::requests::setting_reqs::call_backup_server;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::{window, Blob, BlobPropertyBag, Url};
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(BackupServer)]
pub fn backup_server() -> Html {
    let database_password = use_state(|| "".to_string());
    let is_loading = use_state(|| false);
    let (state, _dispatch) = use_store::<AppState>();
    let (_audio_state, audio_dispatch) = use_store::<UIState>();
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let blob_property_bag = BlobPropertyBag::new();

    let on_download_click = {
        let database_password = database_password.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let blob_property_bag = blob_property_bag.clone();
        let audio_dispatch_call = audio_dispatch.clone();
        let is_loading = is_loading.clone();

        Callback::from(move |_| {
            let audio_dispatch_call = audio_dispatch_call.clone();
            let db_pass = (*database_password).trim().to_string();
            if db_pass.is_empty() {
                audio_dispatch.reduce_mut(|audio_state| {
                    audio_state.error_message =
                        Option::from("Database password cannot be empty.".to_string())
                });
                return;
            }

            is_loading.set(true);
            let api_key = api_key.clone().unwrap_or_default();
            let server_name = server_name.clone().unwrap_or_default();
            let bloberty_bag = blob_property_bag.clone();
            let is_loading_clone = is_loading.clone();

            wasm_bindgen_futures::spawn_local(async move {
                match call_backup_server(&server_name, &db_pass, &api_key.unwrap()).await {
                    Ok(backup_data) => {
                        let array = js_sys::Array::new();
                        array.push(&JsValue::from_str(&backup_data));

                        let blob =
                            Blob::new_with_str_sequence_and_options(&array, &bloberty_bag).unwrap();
                        let url = Url::create_object_url_with_blob(&blob).unwrap();

                        if let Some(window) = window() {
                            let document = window.document().unwrap();
                            let a = document
                                .create_element("a")
                                .unwrap()
                                .dyn_into::<web_sys::HtmlAnchorElement>()
                                .unwrap();
                            a.set_href(&url);
                            a.set_download("server_backup.sql");
                            a.click();

                            Url::revoke_object_url(&url).unwrap();
                        }
                        is_loading_clone.set(false);
                    }
                    Err(e) => {
                        audio_dispatch_call.reduce_mut(|audio_state| {
                            audio_state.error_message = Option::from(
                                format!("Error backing up server - Maybe wrong password?: {}", e)
                                    .to_string(),
                            )
                        });
                        is_loading_clone.set(false);
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
            <div class="flex flex-col space-y-4 sm:flex-row sm:space-y-0 sm:space-x-4">
                <input
                    type="password"
                    id="db-pw"
                    disabled={*is_loading}
                    oninput={Callback::from(move |e: InputEvent| {
                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                        database_password.set(input.value());
                    })}
                    class={classes!(
                        "search-bar-input",
                        "border",
                        "text-sm",
                        "rounded-lg",
                        "block",
                        "w-full",
                        "p-2.5",
                        if *is_loading { "opacity-50 cursor-not-allowed" } else { "" }
                    )}
                    placeholder="mYDBp@ss!"
                />
                <button
                    onclick={on_download_click}
                    disabled={*is_loading}
                    class={classes!(
                        "settings-button",
                        "font-bold",
                        "py-2",
                        "px-6",
                        "rounded",
                        "focus:outline-none",
                        "focus:shadow-outline",
                        "inline-flex",
                        "items-center",
                        "justify-center",
                        "min-w-[120px]",
                        if *is_loading { "opacity-75 cursor-not-allowed" } else { "" }
                    )}
                >
                    if *is_loading {
                        <div class="inline-flex items-center">
                            <svg class="animate-spin -ml-1 mr-3 h-5 w-5" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                            {"Exporting..."}
                        </div>
                    } else {
                        {"Authenticate"}
                    }
                </button>
            </div>

            if *is_loading {
                <div class="mt-4 p-4 bg-blue-50 dark:bg-blue-900/20 rounded-lg">
                    <div class="flex items-center space-x-3">
                        <div class="flex-shrink-0">
                            <svg class="animate-spin h-5 w-5 text-blue-600 dark:text-blue-400" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
                                <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                                <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"></path>
                            </svg>
                        </div>
                        <div class="flex-1 min-w-0">
                            <p class="text-sm font-medium text-blue-900 dark:text-blue-100">
                                {"Backing up database..."}
                            </p>
                            <p class="text-sm text-blue-700 dark:text-blue-300">
                                {"This may take a few minutes for large databases. Please don't close this window."}
                            </p>
                        </div>
                    </div>
                </div>
            }
        </div>
    }
}
