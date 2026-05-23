use crate::components::context::AppState;
use crate::components::gen_funcs::parse_opml;
use gloo::timers::callback::Interval;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::{FileReader, HtmlInputElement};
use yew::prelude::*;
use yewdux::prelude::*;
use i18nrs::yew::use_translation;
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
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let import_pods = use_state(|| Vec::new());
    let show_verification = use_state(|| false);
    let import_progress = use_state(|| 0);
    let total_podcasts = use_state(|| 0);
    let current_podcast = use_state(String::default);

    // Capture all i18n strings at function start to avoid borrow checker issues
    let i18n_import_tip = i18n.t("import_options.import_tip").to_string();
    let i18n_choose_file = i18n.t("import_options.choose_file").to_string();
    let i18n_import_verification_text = i18n.t("import_options.import_verification_text").to_string();
    let i18n_add_them = i18n.t("import_options.add_them").to_string();
    let i18n_import_progress = i18n.t("import_options.import_progress").to_string();
    let i18n_currently_importing = i18n.t("import_options.currently_importing").to_string();

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

            // Use pre-captured translated messages
            let success_msg = i18n.t("import_options.opml_import_completed").to_string();
            let error_msg = i18n.t("import_options.failed_to_import_opml").to_string();
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

                                let success_msg_clone = success_msg.clone();
                                let callback = Closure::wrap(Box::new(move || {
                                    let dispatch_wasm = dispatch_wasm_call.clone();
                                    let server_name = server_name.clone();
                                    let api_key = api_key.clone();
                                    let user_id = user_id;
                                    let import_progress = import_progress.clone();
                                    let current_podcast = current_podcast.clone();
                                    let total_podcasts = total_podcasts.clone();
                                    let interval = interval_clone.clone();
                                    let success_msg_callback = success_msg_clone.clone();
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
                                                        audio_state.info_message = Option::from(success_msg_callback)
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
                                    state.info_message = Option::from(error_msg);
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
        <>
            <div class="settings-row">
                <div><div class="settings-row-label">{&i18n_import_tip}</div></div>
                <div class="settings-row-control">
                    <label class="btn btn-secondary" for="fileInput" style="padding:6px 12px; cursor:pointer;">
                        <i class="ph ph-upload-simple"></i>
                        <span>{&i18n_choose_file}</span>
                    </label>
                    <input id="fileInput" style="display:none;" type="file" accept=".opml" onchange={onclick} />
                </div>
            </div>
            {
                if *show_verification {
                    html! {
                        <>
                            <div class="settings-row">
                                <div><div class="settings-row-label">{&i18n_import_verification_text}</div></div>
                                <div class="settings-row-control">
                                    <button class="btn btn-secondary" onclick={on_confirm} style="padding:6px 12px;">
                                        <i class="ph ph-download-simple"></i>
                                        <span>{&i18n_add_them}</span>
                                    </button>
                                </div>
                            </div>
                            if *total_podcasts > 0 {
                                <div class="settings-row">
                                    <div>
                                        <div class="settings-row-label">{&i18n_import_progress}</div>
                                        <div class="settings-row-desc">{format!("{}{}", &i18n_currently_importing, *current_podcast)}</div>
                                    </div>
                                    <div class="settings-row-control">
                                        <span style="font-size:13px;color:var(--text-color);">
                                            {format!("{}/{}", *import_progress, *total_podcasts)}
                                        </span>
                                        <i class="ph ph-spinner animate-spin" style="font-size:18px;color:var(--text-color);"></i>
                                    </div>
                                </div>
                            }
                            <div style="padding: 0 20px;">
                                { for (*import_pods).iter().enumerate().map(|(index, podcast)| {
                                    let toggle_selection = {
                                        let import_pods = import_pods.clone();
                                        Callback::from(move |_| {
                                            let mut new_import_pods = (*import_pods).clone();
                                            new_import_pods[index].selected = !new_import_pods[index].selected;
                                            import_pods.set(new_import_pods);
                                        })
                                    };
                                    html! {
                                        <div class="podcast import-list" style="padding: 8px 0; border-bottom: 1px solid rgba(128,128,128,0.12);">
                                            <label class="toggle" onclick={toggle_selection} style="cursor:pointer; width:100%; justify-content:flex-start; gap:12px;">
                                                <input type="checkbox" checked={podcast.selected} />
                                                <span class="toggle-track"><span class="toggle-thumb"></span></span>
                                                <div>
                                                    <span style="font-size:13px;font-weight:500;color:var(--text-color);">{&podcast.title}</span>
                                                    <div style="font-size:11px;color:var(--text-secondary-color);margin-top:2px;">{&podcast.xml_url}</div>
                                                </div>
                                            </label>
                                        </div>
                                    }
                                })}
                            </div>
                        </>
                    }
                } else {
                    html! {}
                }
            }
        </>
    }
}
