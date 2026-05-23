use crate::components::context::AppState;
use crate::components::gen_funcs::format_error_message;
use crate::requests::setting_reqs::{
    call_delete_shared_link, call_extend_shared_link, call_get_user_shared_links, SharedLink,
};
use i18nrs::yew::use_translation;
use wasm_bindgen::JsCast;
use yew::prelude::*;
use yewdux::prelude::*;

#[function_component(SharedLinks)]
pub fn shared_links() -> Html {
    let (i18n, _) = use_translation();
    let (state, dispatch) = use_store::<AppState>();
    let user_id = state.user_details.as_ref().map(|ud| ud.UserID.clone());
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());

    let links: UseStateHandle<Vec<SharedLink>> = use_state(Vec::new);
    let selected_code: UseStateHandle<Option<String>> = use_state(|| None);
    let extend_days: UseStateHandle<i64> = use_state(|| 30);
    let refresh_trigger = use_state(|| 0u32);

    #[derive(Clone, PartialEq)]
    enum ModalState {
        Hidden,
        ConfirmDelete,
        ExtendExpiry,
    }
    let modal_state = use_state(|| ModalState::Hidden);

    // Fetch shared links on mount and on refresh
    {
        let links = links.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let user_id = user_id.clone();
        let dispatch_err = dispatch.clone();
        let refresh_trigger = refresh_trigger.clone();

        use_effect_with(*refresh_trigger, move |_| {
            let links = links.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let user_id = user_id.clone();
            let dispatch_err = dispatch_err.clone();

            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(api_key), Some(server_name), Some(user_id)) =
                    (api_key, server_name, user_id)
                {
                    if let Some(api_key) = api_key {
                        match call_get_user_shared_links(&server_name, user_id, &api_key).await {
                            Ok(resp) => links.set(resp.shared_links),
                            Err(e) => {
                                let msg = format_error_message(&e.to_string());
                                dispatch_err.reduce_mut(|s| {
                                    s.error_message = Some(format!("Error loading shared links: {}", msg))
                                });
                            }
                        }
                    }
                }
            });
            || ()
        });
    }

    let close_modal = {
        let modal_state = modal_state.clone();
        Callback::from(move |_| modal_state.set(ModalState::Hidden))
    };

    let on_background_click = {
        let close_modal = close_modal.clone();
        Callback::from(move |e: MouseEvent| {
            let target = e.target().unwrap();
            let element = target.dyn_into::<web_sys::Element>().unwrap();
            if element.tag_name() == "DIV" {
                close_modal.emit(e);
            }
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| e.stop_propagation());

    // Delete confirm handler
    let confirm_delete = {
        let selected_code = selected_code.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let modal_state = modal_state.clone();
        let refresh_trigger = refresh_trigger.clone();
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            let code = (*selected_code).clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let modal_state = modal_state.clone();
            let refresh_trigger = refresh_trigger.clone();
            let dispatch = dispatch.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(code), Some(api_key), Some(server_name)) =
                    (code, api_key, server_name)
                {
                    if let Some(api_key) = api_key {
                        match call_delete_shared_link(&server_name, &code, &api_key).await {
                            Ok(_) => {
                                dispatch.reduce_mut(|s| {
                                    s.info_message = Some("Shared link deleted.".to_string())
                                });
                                refresh_trigger.set(*refresh_trigger + 1);
                            }
                            Err(e) => {
                                let msg = format_error_message(&e.to_string());
                                dispatch.reduce_mut(|s| {
                                    s.error_message =
                                        Some(format!("Error deleting link: {}", msg))
                                });
                            }
                        }
                    }
                }
                modal_state.set(ModalState::Hidden);
            });
        })
    };

    // Extend confirm handler
    let confirm_extend = {
        let selected_code = selected_code.clone();
        let extend_days = extend_days.clone();
        let api_key = api_key.clone();
        let server_name = server_name.clone();
        let modal_state = modal_state.clone();
        let refresh_trigger = refresh_trigger.clone();
        let dispatch = dispatch.clone();
        Callback::from(move |_| {
            let code = (*selected_code).clone();
            let days = *extend_days;
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let modal_state = modal_state.clone();
            let refresh_trigger = refresh_trigger.clone();
            let dispatch = dispatch.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let (Some(code), Some(api_key), Some(server_name)) =
                    (code, api_key, server_name)
                {
                    if let Some(api_key) = api_key {
                        match call_extend_shared_link(&server_name, &code, days, &api_key).await {
                            Ok(_) => {
                                dispatch.reduce_mut(|s| {
                                    s.info_message = Some(format!(
                                        "Shared link extended by {} days.",
                                        days
                                    ))
                                });
                                refresh_trigger.set(*refresh_trigger + 1);
                            }
                            Err(e) => {
                                let msg = format_error_message(&e.to_string());
                                dispatch.reduce_mut(|s| {
                                    s.error_message =
                                        Some(format!("Error extending link: {}", msg))
                                });
                            }
                        }
                    }
                }
                modal_state.set(ModalState::Hidden);
            });
        })
    };

    let on_days_change = {
        let extend_days = extend_days.clone();
        Callback::from(move |e: InputEvent| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            if let Ok(val) = input.value().parse::<i64>() {
                extend_days.set(val);
            }
        })
    };

    let delete_modal = html! {
        <div tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="relative rounded-lg shadow">
                    <div class="flex flex-col items-start justify-between p-4 md:p-5 border-b rounded-t">
                        <button onclick={close_modal.clone()} class="self-end text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                        </button>
                        <h3 class="text-xl font-semibold item_container-text">{i18n.t("shared_links.delete_link")}</h3>
                        <p class="text-m font-semibold">{i18n.t("shared_links.delete_link_confirm")}</p>
                        <div class="flex justify-between space-x-4">
                            <button onclick={confirm_delete} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                {i18n.t("shared_links.delete")}
                            </button>
                            <button onclick={close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                {i18n.t("shared_links.cancel")}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    };

    let extend_modal = html! {
        <div tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25" onclick={on_background_click.clone()}>
            <div class="modal-container relative p-4 w-full max-w-md max-h-full rounded-lg shadow" onclick={stop_propagation.clone()}>
                <div class="relative rounded-lg shadow">
                    <div class="flex flex-col items-start justify-between p-4 md:p-5 border-b rounded-t">
                        <button onclick={close_modal.clone()} class="self-end text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                        </button>
                        <h3 class="text-xl font-semibold item_container-text">{i18n.t("shared_links.extend_link")}</h3>
                        <p class="text-m font-semibold">{i18n.t("shared_links.extend_link_description")}</p>
                        <div class="mt-4 w-full">
                            <label class="block mb-2 text-sm font-medium">{i18n.t("shared_links.days_to_extend")}</label>
                            <input
                                type="number"
                                min="1"
                                max="365"
                                value={extend_days.to_string()}
                                oninput={on_days_change}
                                class="form-input w-full"
                            />
                        </div>
                        <div class="flex justify-between space-x-4">
                            <button onclick={confirm_extend} class="mt-4 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                {i18n.t("shared_links.extend")}
                            </button>
                            <button onclick={close_modal.clone()} class="mt-4 download-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                                {i18n.t("shared_links.cancel")}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    };

    html! {
        <>
        {
            match *modal_state {
                ModalState::ConfirmDelete => delete_modal,
                ModalState::ExtendExpiry => extend_modal,
                ModalState::Hidden => html! {},
            }
        }
        <div class="relative overflow-x-auto">
            <table class="w-full text-sm text-left rtl:text-right">
                <thead class="text-xs uppercase table-header">
                    <tr>
                        <th scope="col" class="px-6 py-3">{i18n.t("shared_links.episode")}</th>
                        <th scope="col" class="px-6 py-3">{i18n.t("shared_links.podcast")}</th>
                        <th scope="col" class="px-6 py-3">{i18n.t("shared_links.expires")}</th>
                        <th scope="col" class="px-6 py-3">{i18n.t("shared_links.actions")}</th>
                    </tr>
                </thead>
                <tbody>
                {
                    if (*links).is_empty() {
                        html! {
                            <tr class="table-row border-b">
                                <td colspan="4" class="px-6 py-4 text-center">{i18n.t("shared_links.no_links")}</td>
                            </tr>
                        }
                    } else {
                        html! {
                            { for (*links).iter().map(|link| {
                                let code_delete = link.share_code.clone();
                                let code_extend = link.share_code.clone();
                                let selected_code_delete = selected_code.clone();
                                let selected_code_extend = selected_code.clone();
                                let modal_delete = modal_state.clone();
                                let modal_extend = modal_state.clone();

                                let on_delete = Callback::from(move |_| {
                                    selected_code_delete.set(Some(code_delete.clone()));
                                    modal_delete.set(ModalState::ConfirmDelete);
                                });
                                let on_extend = Callback::from(move |_| {
                                    selected_code_extend.set(Some(code_extend.clone()));
                                    modal_extend.set(ModalState::ExtendExpiry);
                                });

                                html! {
                                    <tr class="table-row border-b">
                                        <td class="px-6 py-4">{ &link.episode_title }</td>
                                        <td class="px-6 py-4">{ &link.podcast_name }</td>
                                        <td class="px-6 py-4">{ &link.expiration_date }</td>
                                        <td class="px-6 py-4">
                                            <div class="flex items-center gap-2">
                                                <button onclick={on_extend} class="settings-button text-xs font-bold h-8 px-3 rounded" style="margin-top:0;margin-bottom:0;">
                                                    {i18n.t("shared_links.extend")}
                                                </button>
                                                <button onclick={on_delete} class="download-button text-xs font-bold h-8 px-3 rounded" style="margin-bottom:0;">
                                                    {i18n.t("shared_links.delete")}
                                                </button>
                                            </div>
                                        </td>
                                    </tr>
                                }
                            })}
                        }
                    }
                }
                </tbody>
            </table>
        </div>
        </>
    }
}
