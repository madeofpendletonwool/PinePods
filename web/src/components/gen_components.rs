use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_router::history::{BrowserHistory, History};
use crate::requests::search_pods::{call_get_podcast_info, test_connection};
use web_sys::{HtmlInputElement, window};
use web_sys::HtmlSelectElement;
use yewdux::prelude::*;
use crate::components::context::{AppState};

#[derive(Properties, PartialEq)]
pub struct ErrorMessageProps {
    pub error_message: UseStateHandle<Option<String>>,
}


#[function_component(ErrorMessage)]
pub fn error_message(props: &ErrorMessageProps) -> Html {
    // Your existing logic here...
    let error_message = use_state(|| None::<String>);
    let (state, dispatch) = use_store::<AppState>();

    {
        let error_message = error_message.clone();
        use_effect(move || {
            let window = window().unwrap();
            let document = window.document().unwrap();

            let error_message_clone = error_message.clone();
            let closure = Closure::wrap(Box::new(move |_event: Event| {
                error_message_clone.set(None);
            }) as Box<dyn Fn(_)>);

            if error_message.is_some() {
                document.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
            }

            // Return cleanup function
            move || {
                if error_message.is_some() {
                    document.remove_event_listener_with_callback("click", closure.as_ref().unchecked_ref()).unwrap();
                }
                closure.forget(); // Prevents the closure from being dropped
            }
        });
    }

    let error_message_clone = error_message.clone();

    let on_error = {
        Callback::from(move |_: ()| {
            let error_message = error_message_clone.clone();

            wasm_bindgen_futures::spawn_local(async move {

            });
        })
    };

    // html! {
    //         // Conditional rendering for the error banner
    //         if let Some(error) = (*error_message).as_ref() {
    //             <div class="error-snackbar">{ error }</div>
    //         }
    // }

    // Use the error_message from props instead of a local state
    if let Some(error) = props.error_message.as_ref() {
        html! {
            <div class="error-snackbar">{ error }</div>
        }
    } else {
        html! {}
    }
}

#[allow(non_camel_case_types)]
#[function_component(Search_nav)]
pub fn search_bar() -> Html {
    let history = BrowserHistory::new();
    let dispatch = Dispatch::<AppState>::global();
    let state: Rc<AppState> = dispatch.get();
    let podcast_value = use_state(|| "".to_string());
    let search_index = use_state(|| "podcast_index".to_string()); // Default to "podcast_index"
    let (app_state, dispatch) = use_store::<AppState>();

    let history_clone = history.clone();
    let podcast_value_clone = podcast_value.clone();
    let search_index_clone = search_index.clone();
    let dispatch_clone = dispatch.clone();
    let on_submit = {
        Callback::from(move |_: ()| {
            let api_url = state.server_details.as_ref().map(|ud| ud.api_url.clone());
            let history = history_clone.clone();
            let search_value = podcast_value_clone.clone();
            let search_index = search_index_clone.clone();
            let dispatch = dispatch.clone();

            wasm_bindgen_futures::spawn_local(async move {
                dispatch.reduce_mut(|state| state.is_loading = Some(true));
                let cloned_api_url = &api_url.clone();
                match test_connection(&cloned_api_url.clone().unwrap()).await {
                    Ok(_) => {
                        match call_get_podcast_info(&search_value, &api_url.unwrap(), &search_index).await {
                            Ok(search_results) => {
                                web_sys::console::log_1(&format!("results: {:?}", search_results).into());
                                dispatch.reduce_mut(move |state| {
                                    state.search_results = Some(search_results);
                                });
                                dispatch.reduce_mut(|state| state.is_loading = Some(false));
                                history.push("/pod_layout"); // Use the route path
                            },
                            Err(e) => {
                                dispatch.reduce_mut(|state| state.is_loading = Some(false));
                                web_sys::console::log_1(&format!("Error: {}", e).into());
                            }
                        }
                    },
                    Err(err_msg) => {
                        dispatch.reduce_mut(|state| state.is_loading = Some(false));
                        web_sys::console::log_1(&format!("Error: {}", err_msg).into());
                    }
                }
            });
        })
    };

    let on_input_change = {
        let podcast_value = podcast_value.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            podcast_value.set(input.value());
        })
    };

    let on_select_change = {
        let search_index = search_index.clone();
        Callback::from(move |e: Event| {
            if let Some(select) = e.target_dyn_into::<HtmlSelectElement>() {
                search_index.set(select.value());
            }
        })
    };



    let on_submit_click = {
        let on_submit = on_submit.clone(); // Clone the existing on_submit logic
        Callback::from(move |_: MouseEvent| {
            on_submit.emit(()); // Invoke the existing on_submit logic
        })
    };

    html! {
        <div class="episodes-container">
            <div class="search-bar-container">
                <input
                    type="text"
                    placeholder="Search podcasts"
                    class="search-input"
                    oninput={on_input_change}
                />
                <select class="search-source" onchange={on_select_change}>
                    <option value="itunes">{"iTunes"}</option>
                    <option value="podcast_index">{"Podcast Index"}</option>
                </select>
                <button onclick={on_submit_click} class="search-btn">{"Search"}</button>
            </div>
        </div>
    }
}