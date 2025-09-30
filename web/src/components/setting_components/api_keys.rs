use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_create_api_key, call_delete_api_key, call_get_api_info, DeleteAPIRequest,
};
use yew::prelude::*;
use yewdux::prelude::*;
// use crate::gen_components::_ErrorMessageProps::error_message;
use wasm_bindgen::JsCast;
use i18nrs::yew::use_translation;

#[function_component(APIKeys)]
pub fn api_keys() -> Html {
    let (i18n, _) = use_translation();
    let (state, _dispatch) = use_store::<AppState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let api_infos = use_state(|| Vec::new());
    let new_api_key = use_state(|| String::new());
    let selected_api_key_id: UseStateHandle<Option<i32>> = use_state(|| None);
    let _error_message = state.error_message.clone();
    let _info_message = state.info_message.clone();
    let dispatch_effect = _dispatch.clone();
    let dispatch_call = _dispatch.clone();

    // Capture all i18n strings at function start to avoid borrow checker issues
    let i18n_error_getting_api_info = i18n.t("api_keys.error_getting_api_info").to_string();
    let i18n_error_getting_api_info_2 = i18n.t("api_keys.error_getting_api_info").to_string();
    let i18n_api_key_deleted_successfully = i18n.t("api_keys.api_key_deleted_successfully").to_string();
    let i18n_error_deleting_api_key = i18n.t("api_keys.error_deleting_api_key").to_string();
    let i18n_close_modal = i18n.t("api_keys.close_modal").to_string();
    let i18n_delete_api_key = i18n.t("api_keys.delete_api_key").to_string();
    let i18n_delete_api_key_confirmation = i18n.t("api_keys.delete_api_key_confirmation").to_string();
    let i18n_delete = i18n.t("api_keys.delete").to_string();
    let i18n_cancel = i18n.t("api_keys.cancel").to_string();
    let i18n_new_api_key_created = i18n.t("api_keys.new_api_key_created").to_string();
    let i18n_api_key_save_instructions = i18n.t("api_keys.api_key_save_instructions").to_string();
    let i18n_ok = i18n.t("api_keys.ok").to_string();
    let i18n_api_keys_title = i18n.t("api_keys.api_keys_title").to_string();
    let i18n_api_keys_description = i18n.t("api_keys.api_keys_description").to_string();
    let i18n_request_api_key = i18n.t("api_keys.request_api_key").to_string();
    let i18n_api_id = i18n.t("api_keys.api_id").to_string();
    let i18n_last_4_digits = i18n.t("api_keys.last_4_digits").to_string();
    let i18n_date_created = i18n.t("api_keys.date_created").to_string();
    let i18n_user = i18n.t("api_keys.user").to_string();
    // Define the type of user in the Vec
    // let users: UseStateHandle<Vec<SettingsUser>> = use_state(|| Vec::new());

    // Fetch the API keys when the component is first rendered or when api_key or server_name changes
    {
        let api_infos = api_infos.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();

        use_effect_with(
            (api_key.clone(), server_name.clone()),
            move |(api_key, server_name)| {
                let api_infos = api_infos.clone();
                let api_key_cloned = api_key.clone();
                let server_name_cloned = server_name.clone();

                let error_prefix = i18n_error_getting_api_info;
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(api_key) = api_key_cloned {
                        if let Some(server_name) = server_name_cloned {
                            match call_get_api_info(server_name, user_id.unwrap(), api_key.unwrap())
                                .await
                            {
                                Ok(response) => {
                                    api_infos.set(response.api_info);
                                }
                                Err(e) => {
                                    let formatted_error = format_error_message(&e.to_string());
                                    let error_msg = format!("{}{}", error_prefix, formatted_error);
                                    dispatch_effect.reduce_mut(|audio_state| {
                                        audio_state.error_message = Option::from(error_msg)
                                    });
                                }
                            }
                        }
                    }
                });

                || ()
            },
        );
    }

    let dispatch_refresh = _dispatch.clone();

    // Add a new `use_effect_with` to re-fetch the API keys when a new API key is added
    {
        let api_infos = api_infos.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let new_api_key = new_api_key.clone();

        use_effect_with(new_api_key.clone(), move |_| {
            let api_infos = api_infos.clone();
            let api_key_cloned = api_key.clone();
            let server_name_cloned = server_name.clone();
            let error_prefix = i18n_error_getting_api_info_2;

            wasm_bindgen_futures::spawn_local(async move {
                if !new_api_key.is_empty() {
                    if let Some(api_key) = api_key_cloned {
                        if let Some(server_name) = server_name_cloned {
                            match call_get_api_info(server_name, user_id.unwrap(), api_key.unwrap())
                                .await
                            {
                                Ok(response) => {
                                    api_infos.set(response.api_info);
                                }
                                Err(e) => {
                                    let formatted_error = format_error_message(&e.to_string());
                                    let error_msg = format!("{}{}", error_prefix, formatted_error);
                                    dispatch_refresh.reduce_mut(|audio_state| {
                                        audio_state.error_message = Option::from(error_msg)
                                    });
                                }
                            }
                        }
                    }
                }
            });

            || ()
        });
    }

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
        Delete,
    }

    // Define the initial state
    let page_state = use_state(|| PageState::Hidden);

    // Define the function to close the modal
    let close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
        })
    };

    let on_background_click = {
        let on_close_modal = close_modal.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            if element.tag_name() == "DIV" {
                on_close_modal.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    // Define the function to open the modal and request a new API key
    let request_state = state.clone();
    let request_api_key = {
        let page_state = page_state.clone();
        let new_api_key = new_api_key.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        Callback::from(move |_| {
            let _dispatch = _dispatch.clone();
            let api_key = api_key.clone();
            let user_id = request_state
                .user_details
                .as_ref()
                .map(|ud| ud.UserID.clone());
            let server_name = server_name.clone();
            let page_state = page_state.clone();
            let new_api_key = new_api_key.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_create_api_key(
                    &server_name.unwrap(),
                    user_id.unwrap(),
                    &api_key.unwrap().unwrap(),
                )
                .await
                {
                    Ok(response) => {
                        new_api_key.set(response.api_key);
                        page_state.set(PageState::Shown); // Move to the edit page state
                    }
                    Err(e) => {
                        _dispatch.reduce_mut(|audio_state| {
                            audio_state.error_message = Option::from(e.to_string())
                        });
                    }
                }
            });
        })
    };

    // Define the function to open the modal and request a new API key
    let delete_api_key = {
        let page_state = page_state.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let api_id = selected_api_key_id.clone();
        // Assume you have user_id and api_key from context or props
        let user_id = 1; // Example user_id
        Callback::from(move |_| {
            let dispatch = dispatch_call.clone();
            let api_key = api_key.clone();
            // let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
            let server_name = server_name.clone();
            let page_state = page_state.clone();
            let user_id = user_id.clone();
            let api_id = api_id.clone();
            let delete_body = DeleteAPIRequest {
                user_id: user_id.to_string(),
                api_id: api_id.unwrap().to_string(),
            };
            // Capture translated messages before async block
            let success_msg = i18n_api_key_deleted_successfully.clone();
            let error_prefix = i18n_error_deleting_api_key.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match call_delete_api_key(
                    &server_name.unwrap(),
                    delete_body,
                    &api_key.unwrap().unwrap(),
                )
                .await
                {
                    Ok(_) => {
                        dispatch.reduce_mut(|audio_state| {
                            audio_state.info_message = Option::from(success_msg)
                        });
                        // Update UI accordingly, e.g., remove the deleted API key from the list
                    }
                    Err(e) => {
                        let formatted_error = format_error_message(&e.to_string());
                        let error_msg = format!("{}{}", error_prefix, formatted_error);
                        dispatch.reduce_mut(|audio_state| {
                            audio_state.error_message = Option::from(error_msg)
                        });
                    }
                }
                page_state.set(PageState::Hidden); // Hide modal after deletion
            });
        })
    };
    let api_key_display = (*new_api_key).clone();

    let on_api_key_row_click = {
        let selected_api_key_id = selected_api_key_id.clone();
        let page_state = page_state.clone();
        move |api_key_id: i32| {
            Callback::from(move |_| {
                selected_api_key_id.set(Some(api_key_id));
                page_state.set(PageState::Delete); // Assuming you have a PageState enum value for showing the delete modal
            })
        }
    };

    let delete_api_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="relative rounded-lg shadow">
                    <div class="flex flex-col items-start justify-between p-4 md:p-5 border-b rounded-t">
                        <button onclick={close_modal.clone()} class="self-end text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{&i18n_close_modal}</span>
                        </button>
                        <h3 class="text-xl font-semibold item_container-text">
                            {&i18n_delete_api_key}
                        </h3>
                        <p class="text-m font-semibold">
                        {&i18n_delete_api_key_confirmation}
                        </p>
                        <div class="flex justify-between space-x-4">
                            <button onclick={delete_api_key} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                {&i18n_delete}
                            </button>
                            <button onclick={close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                {&i18n_cancel}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    };

    let create_api_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="flex flex-col items-start justify-between p-4 md:p-5 border-b rounded-t ">
                    <button onclick={close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                        <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                            <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                        </svg>
                        <span class="sr-only">{"Close modal"}</span>
                    </button>
                    <h3 class="item_container-text text-xl font-semibold">
                        {&i18n_new_api_key_created}
                    </h3>
                    <p class="text-m font-semibold item_container-text">
                    {&i18n_api_key_save_instructions}
                    </p>
                    <div class="mfa-code-box mt-4 p-4 rounded-md overflow-x-auto whitespace-nowrap max-w-full">
                        {api_key_display}
                    </div>
                    <button onclick={close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                        {&i18n_ok}
                    </button>
                </div>
            </div>
        </div>
    };

    html! {
        <>
        {
            match *page_state {
            PageState::Shown => create_api_modal,
            PageState::Delete => delete_api_modal,
            _ => html! {},
            }
        }
            <div class="p-4">
                <p class="item_container-text text-lg font-bold mb-4">{&i18n_api_keys_title}</p>
                <p class="item_container-text text-md mb-4">{&i18n_api_keys_description}</p>
                <button onclick={request_api_key} class="mt-4 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                    {&i18n_request_api_key}
                </button>
            </div>
            <div class="relative overflow-x-auto">
                <table class="w-full text-sm text-left rtl:text-right">
                    <thead class="text-xs uppercase table-header">
                        <tr>
                            <th scope="col" class="px-6 py-3">{&i18n_api_id}</th>
                            <th scope="col" class="px-6 py-3">{&i18n_last_4_digits}</th>
                            <th scope="col" class="px-6 py-3">{&i18n_date_created}</th>
                            <th scope="col" class="px-6 py-3">{&i18n_user}</th>
                        </tr>
                    </thead>
                    <tbody>
                    {
                        for (*api_infos).iter().map(|api_info| {
                            let on_api_key_row_click = on_api_key_row_click.clone();
                            let row_click_callback = on_api_key_row_click(api_info.apikeyid); // Capture the APIKeyID for the callback
                            html! {
                                <tr class="table-row border-b cursor-pointer" onclick={row_click_callback}>
                                    <td class="px-6 py-4">{ api_info.apikeyid }</td>
                                    <td class="px-6 py-4">{ &api_info.lastfourdigits }</td>
                                    <td class="px-6 py-4">{ &api_info.created }</td>
                                    <td class="px-6 py-4">{ &api_info.username }</td>
                                </tr>
                            }
                        })
                    }
                </tbody>
                </table>
            </div>
        </>
    }
}
