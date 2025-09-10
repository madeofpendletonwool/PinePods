use crate::components::context::AppState;
use crate::components::gen_funcs::parse_opml;
use gloo::timers::callback::Interval;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{FileReader, HtmlInputElement};
use yew::prelude::*;
use yewdux::prelude::*;
// use wasm_bindgen::JsValue;
use crate::requests::setting_reqs::{call_podcast_opml_import, fetch_import_progress};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsValue;

#[derive(Clone, Debug)]
struct PodcastToImport {
    title: String,
    xml_url: String,
    selected: bool,
}

#[function_component(ImportOptions)]
pub fn import_options() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let import_pods = use_state(|| Vec::new());
    let show_verification = use_state(|| false);
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
                                .map(|(title, xml_url)| PodcastToImport {
                                    title,
                                    xml_url,
                                    selected: true,
                                })
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
                    if let (Some(server_name), Some(api_key), Some(user_id)) =
                        (server_name.clone(), api_key.clone(), user_id)
                    {
                        match call_podcast_opml_import(
                            &server_name,
                            &Some(api_key.clone().unwrap()),
                            user_id,
                            selected_podcasts.clone(),
                        )
                        .await
                        {
                            Ok(_) => {
                                let interval: Rc<RefCell<Option<Interval>>> =
                                    Rc::new(RefCell::new(None));
                                let interval_clone = interval.clone();

                                let callback = Closure::wrap(Box::new(move || {
                                    let dispatch_wasm = dispatch_wasm_call.clone();
                                    let server_name = server_name.clone();
                                    let api_key = api_key.clone();
                                    let user_id = user_id;
                                    let import_progress = import_progress.clone();
                                    let current_podcast = current_podcast.clone();
                                    let total_podcasts = total_podcasts.clone();
                                    let interval = interval_clone.clone();
                                    wasm_bindgen_futures::spawn_local(async move {
                                        match fetch_import_progress(&server_name, &api_key, user_id)
                                            .await
                                        {
                                            Ok((current, total, podcast)) => {
                                                import_progress.set(current);
                                                total_podcasts.set(total as usize);
                                                current_podcast.set(podcast);
                                                if current >= total {
                                                    // Import is complete, stop polling
                                                    if let Some(interval) =
                                                        interval.borrow_mut().take()
                                                    {
                                                        interval.cancel();
                                                    }
                                                    dispatch_wasm.reduce_mut(|state| {
                                                        state.is_loading = Some(false)
                                                    });
                                                    dispatch_wasm.reduce_mut(|audio_state| {
                                                        audio_state.info_message = Option::from(
                                                            "OPML Import Completed!".to_string(),
                                                        )
                                                    });
                                                }
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(&JsValue::from_str(
                                                    "progress failed",
                                                ));
                                                log::error!(
                                                    "Failed to fetch import progress: {:?}",
                                                    e
                                                );
                                            }
                                        }
                                    });
                                })
                                    as Box<dyn Fn()>);

                                interval.borrow_mut().replace(Interval::new(5000, move || {
                                    callback
                                        .as_ref()
                                        .unchecked_ref::<js_sys::Function>()
                                        .call0(&JsValue::NULL)
                                        .unwrap();
                                    // Return () explicitly
                                    ()
                                }));
                            }
                            Err(e) => {
                                log::error!("Failed to import OPML: {:?}", e);
                                dispatch_wasm_call.reduce_mut(|state| {
                                    state.is_loading = Some(false);
                                    state.info_message =
                                        Option::from("Failed to import OPML".to_string());
                                    state.clone()
                                });
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
            <p class="item_container-text text-md mb-4">{"You can Import an OPML of podcasts here. If you're migrating from a different podcast app this is probably the solution you want. Most podcast apps allow you to export a backup of your saved podcasts to an OPML file and this option can easily import them into Pinepods. Note that this process can take awhile and you don't have to stay on this page while it imports. If you think it might be stuck, it's probably not."}</p>
            // <input class="settings-button" type="file" accept=".opml" onchange={onclick} />
            <label class="input-button-label" for="fileInput">{ "Choose File" }</label>
            <input id="fileInput" class="input-button" type="file" accept=".opml" onchange={onclick} />
            // Optionally display the content of the OPML file for debugging
            {
                if *show_verification {
                    html! {
                        <div class="import-box space-y-6">
                            <div class="space-y-4">
                                <p class="item_container-text">
                                    {"The following podcasts were found. Please unselect any podcasts you don't want to add, and then click the button below. A large amount of podcasts will take a little while to parse all the feeds and add them. The loading animation will disappear once all complete. Be patient!"}
                                </p>
                                <button class="settings-button flex items-center gap-2" onclick={on_confirm}>
                                    <i class="ph ph-download-simple text-xl"></i>
                                    {"Add them!"}
                                </button>
                            </div>

                            // Progress section with improved styling
                            <div class="bg-opacity-10 bg-white p-4 rounded-lg border border-opacity-20 space-y-3">
                                <div class="flex justify-between items-center">
                                    <span class="item_container-text text-sm">{"Import Progress"}</span>
                                    <span class="item_container-text text-lg font-semibold">
                                        {format!("{}/{}", *import_progress, *total_podcasts)}
                                    </span>
                                </div>

                                // Progress bar
                                <div class="w-full bg-gray-700 rounded-full h-2.5">
                                    <div class="bg-pink-500 h-2.5 rounded-full transition-all duration-300"
                                         style={format!("width: {}%", (*import_progress as f32 / *total_podcasts as f32 * 100.0))}>
                                    </div>
                                </div>

                                <div class="flex items-center gap-2">
                                    <i class="ph ph-sync text-lg animate-spin"></i>
                                    <span class="item_container-text text-sm opacity-80">
                                        {format!("Currently importing: {}", *current_podcast)}
                                    </span>
                                </div>
                            </div>

                            // Podcast list
                            <div class="space-y-2">
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
                                            <div class="podcast import-list p-3 hover:bg-opacity-10 hover:bg-white rounded-lg transition-all">
                                                <label class="flex items-center gap-3 cursor-pointer w-full" onclick={toggle_selection}>
                                                    <input
                                                        type="checkbox"
                                                        checked={podcast.selected}
                                                        class="h-5 w-5 rounded border-2 border-gray-400 text-primary focus:ring-primary focus:ring-offset-0 cursor-pointer appearance-none checked:bg-primary checked:border-primary relative
                                                        before:content-[''] before:block before:w-full before:h-full before:checked:bg-[url('data:image/svg+xml;base64,PHN2ZyB2aWV3Qm94PScwIDAgMTYgMTYnIGZpbGw9JyNmZmYnIHhtbG5zPSdodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2Zyc+PHBhdGggZD0nTTEyLjIwNyA0Ljc5M2ExIDEgMCAwIDEgMCAxLjQxNGwtNSA1YTEgMSAwIDAgMS0xLjQxNCAwbC0yLTJhMSAxIDAgMCAxIDEuNDE0LTEuNDE0TDYuNSA5LjA4NmwzLjc5My0zLjc5M2ExIDEgMCAwIDEgMS40MTQgMHonLz48L3N2Zz4=')] before:checked:bg-no-repeat before:checked:bg-center"
                                                    />
                                                    <div class="space-y-1 flex-1">
                                                        <span class="item_container-text font-medium block">{&podcast.title}</span>
                                                        <span class="item_container-text text-sm opacity-60 block">{&podcast.xml_url}</span>
                                                    </div>
                                                </label>
                                            </div>
                                        }
                                    })
                                }
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>

    }
}
