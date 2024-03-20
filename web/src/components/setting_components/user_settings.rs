use yew::prelude::*;
use yewdux::prelude::*;
use crate::components::context::{UIState, AppState};
use yew::platform::spawn_local;
use crate::requests::setting_reqs::call_get_user_info;
use web_sys::console;
use std::borrow::Borrow;
use crate::requests::setting_reqs::{SettingsUser, call_add_user, AddSettingsUserRequest, call_set_password, call_set_email, call_set_fullname, call_set_username, call_check_admin, call_set_isadmin};
use crate::components::gen_funcs::encode_password;
use crate::components::gen_funcs::validate_user_input;
// use crate::gen_components::_ErrorMessageProps::error_message;


#[function_component(UserSettings)]
pub fn user_settings() -> Html {
    let (state, _dispatch) = use_store::<AppState>();
    let (_ui_state, ui_dispatch) = use_store::<UIState>();
    let server_name = state.auth_details.as_ref().map(|ud| ud.server_name.clone());
    let api_key = state.auth_details.as_ref().map(|ud| ud.api_key.clone());
    let new_username = use_state(|| "".to_string());
    let new_password = use_state(|| "".to_string());
    let email = use_state(|| "".to_string());
    let fullname = use_state(|| "".to_string());
    let admin_status = use_state(|| false);
    let selected_user_id = use_state(|| None);
    web_sys::console::log_1(&"testlog".into());
    // Define the type of user in the Vec
    let users: UseStateHandle<Vec<SettingsUser>> = use_state(|| Vec::new());

    let user_dispatch = ui_dispatch.clone();
    {
        let users = users.clone();
        use_effect_with((api_key.clone(), server_name.clone()), move |(api_key, server_name)| {
            let users = users.clone();
            let api_key = api_key.clone();
            let server_name = server_name.clone();
            let future = async move {
                if let (Some(api_key), Some(server_name)) = (api_key, server_name) {
                    let response = call_get_user_info(server_name, api_key.unwrap()).await;
                    match response {
                        Ok(user_info) => {
                            users.set(user_info);
                        },
                        Err(e) => {
                            user_dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error getting user info: {}", e).to_string()))
                        },
                    }
                }
            };
            spawn_local(future);
            // Return cleanup function
            || {}
        });
    }

    // Define the state of the application
    #[derive(Clone, PartialEq)]
    enum PageState {
        Hidden,
        Shown,
        Edit,
    }

    // Define the initial state
    let page_state = use_state(|| PageState::Hidden);


    // Define the callback function for closing the modal
    let on_close_modal = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Hidden);
        })
    };

    // Define the callback functions
    let on_create_new_user = {
        let page_state = page_state.clone();
        Callback::from(move |_| {
            page_state.set(PageState::Shown);
        })
    };

    let on_fullname_change = {
        let fullname = fullname.clone();
        Callback::from(move |e: InputEvent| {
            fullname.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_username_change = {
        let new_username = new_username.clone();
        Callback::from(move |e: InputEvent| {
            new_username.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_email_change = {
        let email = email.clone();
        Callback::from(move |e: InputEvent| {
            email.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };
    
    let on_password_change = {
        let new_password = new_password.clone();
        Callback::from(move |e: InputEvent| {
            new_password.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().value());
        })
    };

    let on_admin_change = {
        let admin_status = admin_status.clone();
        Callback::from(move |e: InputEvent| {
            admin_status.set(e.target_unchecked_into::<web_sys::HtmlInputElement>().checked());
        })
    };
    let create_dispatch = ui_dispatch.clone();
    let on_create_submit = {
        let page_state = page_state.clone();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let fullname = fullname.clone().to_string();
        let new_username = new_username.clone().to_string();
        let email = email.clone().to_string();
        let new_password = new_password.clone();
        let dispatch_wasm = create_dispatch.clone();
        Callback::from(move |e: MouseEvent| {
            let call_server = server_name.clone();
            let call_api = api_key.clone();
            let dispatch = dispatch_wasm.clone();
            let new_username = new_username.clone();
            let new_password = new_password.clone();
            let fullname = fullname.clone();
            let email = email.clone();
            e.prevent_default();
            page_state.set(PageState::Hidden);
            // Hash the password and generate a salt
            match validate_user_input(&new_username, &new_password, &email) {
                Ok(_) => {
                    match encode_password(&new_password) {
                        Ok(hash_pw) => {
                            let user_settings = AddSettingsUserRequest {
                                fullname: fullname.clone(),
                                username: new_username.clone(),
                                email: email.clone(),
                                hash_pw: hash_pw.clone(),
                            };
                            // You no longer need to clone here, just directly use the constructed request
                            let add_user_request = Some(user_settings);

                            // Directly pass `add_user_request` which is an Option and owned by the block
                            wasm_bindgen_futures::spawn_local(async move {
                                // Directly use the contained value if `add_user_request` is `Some`
                                if let Some(add_user_request_value) = add_user_request {
                                    match call_add_user(call_server.unwrap(), call_api.unwrap().unwrap(), &add_user_request_value).await {
                                        Ok(_success) => {
                                            // Handle success here, perhaps log or set some state
                                            console::log_1(&"User added successfully".into());
                                        },
                                        Err(e) => {
                                            // Handle error here, perhaps log or set some error state
                                            dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error adding user: {:?}", e).to_string()));
                                        },
                                    }
                                } else {
                                    dispatch.reduce_mut(|state| state.error_message = Option::from(("add_user_request is None, cannot proceed with adding user.").to_string()))
                                }
                            });
                            
                        },
                        Err(e) => {
                            // Handle the error here
                            dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Password Encoding Failed {:?}", e)));
                        }
                    }
                },
                Err(e) => {
                    dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Invalid User Input {:?}", e)));
                    return;
                }
            }


        })
    };

    // Define the modal components
    let create_user_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700">
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
                            {"Create New User"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="username" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Username"}</label>
                                <input oninput={on_username_change.clone()} type="text" id="username" name="username" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="fullname" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Full Name"}</label>
                                <input oninput={on_fullname_change.clone()} type="text" id="fullname" name="fullname" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="email" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Email"}</label>
                                <input oninput={on_email_change.clone()} type="email" id="email" name="email" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="password" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Password"}</label>
                                <input oninput={on_password_change.clone()} type="password" id="password" name="password" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <button type="submit" onclick={on_create_submit} class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Submit"}</button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };
    let user_dispatch = ui_dispatch.clone();
    let on_user_row_click = {
        let selected_user_id = selected_user_id.clone();
        let page_state = page_state.clone();
        move |select_user_id: i32| {
            console::log_1(&format!("Selected user ID: {:?}", select_user_id).into());
            Callback::from(move |_| {
                if select_user_id == 1 {
                    user_dispatch.reduce_mut(|state| state.error_message = Option::from("You cannot edit the guest user.".to_string()));
                    return;
                }
                selected_user_id.set(Some(Some(select_user_id))); // Wrap the value in double Some()
                page_state.set(PageState::Edit); // Move to the edit page state
            })
        }
    };
    

    let on_edit_submit = {
        let fullname = fullname.clone().to_string();
        let page_state = page_state.clone();
        let new_username = new_username.clone().to_string();
        let server_name = server_name.clone();
        let api_key = api_key.clone();
        let email = email.clone().to_string();
        let new_password = new_password.clone();
        let dispatch_wasm = ui_dispatch.clone();
        let edit_selected_user_id = selected_user_id.clone();
        Callback::from(move |e: MouseEvent| {
            let dispatch_wasm = dispatch_wasm.clone();
            let new_username = new_username.clone();
            let new_password = new_password.clone();
            let fullname = fullname.clone();
            let email = email.clone();
            let admin_status = admin_status.clone();
            let call_selected_user_id = edit_selected_user_id.clone();
            e.prevent_default();
            
            // Check if each field has input and call the corresponding API function
            let fullname_dispatch = dispatch_wasm.clone();
            if !fullname.is_empty() {
                wasm_bindgen_futures::spawn_local({
                    let server_name_cloned = server_name.clone();
                    let api_key_cloned = api_key.clone();
                    let name_cloned = fullname.clone();
                    let selected_user_id_cloned = call_selected_user_id.clone();
            
                    async move {
                        if let Some(server_name_unwrapped) = server_name_cloned {
                            if let Some(api_key_unwrapped) = api_key_cloned.as_ref().and_then(|key| key.as_ref()) {
                                if let Some(user_id) = *selected_user_id_cloned {
                                    match call_set_fullname(server_name_unwrapped, api_key_unwrapped.clone(), user_id.unwrap(), name_cloned).await {
                                        Ok(_) => console::log_1(&"Name updated successfully".into()),
                                        Err(e) => fullname_dispatch.reduce_mut(|state| state.error_message = Option::from(format!("Error updating name: {}", e).to_string())),
                                    }
                                } else {
                                    fullname_dispatch.reduce_mut(|state| state.error_message = Option::from("User ID not available for name update.".to_string()));
                                }
                            } else {
                                fullname_dispatch.reduce_mut(|state| state.error_message = Option::from("API key not available for name update.".to_string()));
                            }
                        } else {
                            fullname_dispatch.reduce_mut(|state| state.error_message = Option::from("Server name not available for name update.".to_string()));
                        }
                    }
                });
            }

            if !new_username.is_empty() {
                wasm_bindgen_futures::spawn_local({
                    let server_name_cloned = server_name.clone();
                    let api_key_cloned = api_key.clone();
                    let user_cloned = new_username.clone();
                    let selected_user_id_cloned = call_selected_user_id.clone();
            
                    async move {
                        if let Some(server_name_unwrapped) = server_name_cloned {
                            if let Some(api_key_unwrapped) = api_key_cloned.as_ref().and_then(|key| key.as_ref()) {
                                if let Some(user_id) = *selected_user_id_cloned {
                                    match call_set_username(server_name_unwrapped, api_key_unwrapped.clone(), user_id.unwrap(), user_cloned).await {
                                        Ok(_) => console::log_1(&"username updated successfully".into()),
                                        Err(e) => dispatch_wasm.reduce_mut(|state| state.error_message = Option::from(format!("Error updating username: {:?}", e).to_string())),
                                    }
                                } else {
                                    dispatch_wasm.reduce_mut(|state| state.error_message = Option::from("User ID not available for username update.".to_string()));
                                }
                            } else {
                                dispatch_wasm.reduce_mut(|state| state.error_message = Option::from("API key not available for username update.".to_string()));
                            }
                        } else {
                            dispatch_wasm.reduce_mut(|state| state.error_message = Option::from("Server name not available for username update.".to_string()));
                        }
                    }
                });
            }

            if !email.is_empty() {
                wasm_bindgen_futures::spawn_local({
                    let server_name_cloned = server_name.clone();
                    let api_key_cloned = api_key.clone();
                    let email_cloned = email.clone();
                    let selected_user_id_cloned = call_selected_user_id.clone();
            
                    async move {
                        if let Some(server_name_unwrapped) = server_name_cloned {
                            if let Some(api_key_unwrapped) = api_key_cloned.as_ref().and_then(|key| key.as_ref()) {
                                if let Some(user_id) = *selected_user_id_cloned {
                                    match call_set_email(server_name_unwrapped, api_key_unwrapped.clone(), user_id.unwrap(), email_cloned).await {
                                        Ok(_) => console::log_1(&"Email updated successfully".into()),
                                        Err(e) => console::log_1(&format!("Error updating email: {:?}", e).into()),
                                    }
                                } else {
                                    console::log_1(&"User ID not available for email update.".into());
                                }
                            } else {
                                console::log_1(&"API key not available for email update.".into());
                            }
                        } else {
                            console::log_1(&"Server name not available for email update.".into());
                        }
                    }
                });
            }
            
            
            
            if !new_password.is_empty() {
                wasm_bindgen_futures::spawn_local({
                    let server_name_cloned = server_name.clone();
                    let api_key_cloned = api_key.clone();
                    let new_password_cloned = new_password.clone();
                    let selected_user_id_cloned = (*call_selected_user_id).clone();
            
                    async move {
                        if let Some(server_name_unwrapped) = server_name_cloned {
                            if let Some(api_key_unwrapped) = api_key_cloned.as_ref().and_then(|key| key.as_ref()) {
                                if let Some(Some(user_id)) = selected_user_id_cloned {
                                    match encode_password(&new_password_cloned) {
                                        Ok(hash_pw) => {
                                            match call_set_password(server_name_unwrapped, api_key_unwrapped.clone(), user_id, hash_pw).await {
                                                Ok(_) => console::log_1(&"Password updated successfully".into()),
                                                Err(e) => console::log_1(&format!("Error updating password: {:?}", e).into()),
                                            }
                                        },
                                        Err(e) => {
                                            console::log_1(&format!("Password encoding failed: {:?}", e).into());
                                        }
                                    }
                                } else {
                                    console::log_1(&"User ID not available for password update.".into());
                                }
                            } else {
                                console::log_1(&"API key not available for password update.".into());
                            }
                        } else {
                            console::log_1(&"Server name not available for password update.".into());
                        }
                    }
                });
            }

            if *admin_status == true {
                wasm_bindgen_futures::spawn_local({
                    let server_name_cloned = server_name.clone();
                    let api_key_cloned = api_key.clone();
                    let admin_status_cloned = admin_status.clone();
                    let selected_user_id_cloned = (*call_selected_user_id).clone();
            
                    async move {
                        if let Some(server_name_unwrapped) = server_name_cloned {
                            if let Some(api_key_unwrapped) = api_key_cloned.as_ref().and_then(|key| key.as_ref()) {
                                if let Some(Some(user_id)) = selected_user_id_cloned {
                                    match call_set_isadmin(server_name_unwrapped, api_key_unwrapped.clone(), user_id, *admin_status_cloned).await {
                                        Ok(_) => console::log_1(&"Admin status updated successfully".into()),
                                        Err(e) => console::log_1(&format!("Error updating admin status: {:?}", e).into()),
                                    }
                                } else {
                                    console::log_1(&"User ID not available for admin status update.".into());
                                }
                            } else {
                                console::log_1(&"API key not available for admin status update.".into());
                            }
                        } else {
                            console::log_1(&"Server name not available for admin status update.".into());
                        }
                    }
                });
            }

            if *admin_status == false {
                wasm_bindgen_futures::spawn_local({
                    let server_name_cloned = server_name.clone();
                    let api_key_cloned = api_key.clone();
                    let admin_status_cloned = admin_status.clone();
                    let selected_user_id_cloned = (*call_selected_user_id).clone();
            
                    async move {
                        if let Some(server_name_unwrapped) = server_name_cloned {
                            if let Some(api_key_unwrapped) = api_key_cloned.as_ref().and_then(|key| key.as_ref()) {
                                if let Some(Some(user_id)) = selected_user_id_cloned {
                                    match call_check_admin(server_name_unwrapped.clone(), api_key_unwrapped.clone(), user_id).await {
                                        Ok(final_admin) => {
                                            if final_admin.final_admin == true {
                                                console::log_1(&"Unable to remove admin status from final administrator".into());
                                            } else {
                                                match call_set_isadmin(server_name_unwrapped, api_key_unwrapped.clone(), user_id, *admin_status_cloned).await {
                                                    Ok(_) => console::log_1(&"Admin status updated successfully".into()),
                                                    Err(e) => console::log_1(&format!("Error updating admin status: {:?}", e).into()),
                                                }
                                            }
                                        },
                                        Err(e) => console::log_1(&format!("Error checking admin status: {:?}", e).into()),
                                    }
                                } else {
                                    console::log_1(&"User ID not available for admin status update.".into());
                                }
                            } else {
                                console::log_1(&"API key not available for admin status update.".into());
                            }
                        } else {
                            console::log_1(&"Server name not available for admin status update.".into());
                        }
                    }
                });
            }

            page_state.set(PageState::Hidden);
    
            // Handle admin status change if applicable
        })
    };
    // Define the modal components
    let edit_user_modal = html! {
        <div id="create-user-modal" tabindex="-1" aria-hidden="true" class="fixed top-0 right-0 left-0 z-50 flex justify-center items-center w-full h-[calc(100%-1rem)] max-h-full bg-black bg-opacity-25">
            <div class="relative p-4 w-full max-w-md max-h-full bg-white rounded-lg shadow dark:bg-gray-700">
                <div class="relative bg-white rounded-lg shadow dark:bg-gray-700">
                    <div class="flex items-center justify-between p-4 md:p-5 border-b rounded-t dark:border-gray-600">
                        <h3 class="text-xl font-semibold text-gray-900 dark:text-white">
                            {"Edit Existing User"}
                        </h3>
                        <button onclick={on_close_modal.clone()} class="end-2.5 text-gray-400 bg-transparent hover:bg-gray-200 hover:text-gray-900 rounded-lg text-sm w-8 h-8 ms-auto inline-flex justify-center items-center dark:hover:bg-gray-600 dark:hover:text-white">
                            <svg class="w-3 h-3" aria-hidden="true" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 14 14">
                                <path stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="m1 1 6 6m0 0 6 6M7 7l6-6M7 7l-6 6"/>
                            </svg>
                            <span class="sr-only">{"Close modal"}</span>
                        </button>
                    </div>
                    <p class="text-m font-semibold text-gray-900 dark:text-white">
                    {"Change the fields below coresponding to the user details you want to edit. Do not add values to fields you don't want to change. Leave those blank."}
                    </p>
                    <div class="p-4 md:p-5">
                        <form class="space-y-4" action="#">
                            <div>
                                <label for="username" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Username"}</label>
                                <input oninput={on_username_change.clone()} type="text" id="username" name="username" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="fullname" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Full Name"}</label>
                                <input oninput={on_fullname_change} type="text" id="fullname" name="fullname" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="email" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Email"}</label>
                                <input oninput={on_email_change} type="email" id="email" name="email" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div>
                                <label for="password" class="block mb-2 text-sm font-medium text-gray-900 dark:text-white">{"Password"}</label>
                                <input oninput={on_password_change.clone()} type="password" id="password" name="password" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 block w-full p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <div class="flex items-center">
                                <label for="admin" class="mr-2 text-sm font-medium text-gray-900 dark:text-white">{"Admin User?"}</label>
                                <input oninput={on_admin_change} type="checkbox" id="admin" name="admin" class="bg-gray-50 border border-gray-300 text-gray-900 text-sm rounded-lg focus:ring-blue-500 focus:border-blue-500 p-2.5 dark:bg-gray-600 dark:border-gray-500 dark:placeholder-gray-400 dark:text-white" required=true />
                            </div>
                            <button type="submit" onclick={on_edit_submit} class="w-full text-white bg-blue-700 hover:bg-blue-800 focus:ring-4 focus:outline-none focus:ring-blue-300 font-medium rounded-lg text-sm px-5 py-2.5 text-center dark:bg-blue-600 dark:hover:bg-blue-700 dark:focus:ring-blue-800">{"Submit"}</button>
                        </form>
                    </div>
                </div>
            </div>
        </div>
    };

    html! {
        <>
        {
            match *page_state {
            PageState::Shown => create_user_modal,
            PageState::Edit => edit_user_modal,
            _ => html! {},
            }
        }
            <div class="p-4">
                <p class="item_container-text text-lg font-bold mb-4">{"User Management:"}</p>
                <p class="item_container-text text-md mb-4">{"You can manage users here. Click a user in the table to manage settings for that existing user or click 'Create New' to add a new user. Note that the guest user will always show regardless of whether it's enabled or not. View the Guest Settings Area to properly manage that."}</p>
                <button onclick={on_create_new_user} class="mt-4 settings-button font-bold py-2 px-4 rounded focus:outline-none focus:shadow-outline">
                    {"Create New User"}
                </button>
            </div>
            <div class="relative overflow-x-auto">
                <table class="w-full text-sm text-left rtl:text-right text-gray-500 dark:text-gray-400">
                    <thead class="text-xs uppercase table-header">
                        <tr>
                            <th scope="col" class="px-6 py-3">{"User ID"}</th>
                            <th scope="col" class="px-6 py-3">{"Fullname"}</th>
                            <th scope="col" class="px-6 py-3">{"Email"}</th>
                            <th scope="col" class="px-6 py-3">{"Username"}</th>
                            <th scope="col" class="px-6 py-3">{"Admin Status"}</th>
                        </tr>
                    </thead>
                    <tbody>
                        { for users.borrow().iter().map(|user| {
                            let user_row_copy = on_user_row_click.clone();
                            let user_row_click = user_row_copy(user.UserID);
                            {
                                html! {
                                    <tr class="table-row border-b cursor-pointer" onclick={user_row_click}> // Adjust this line accordingly
                                        <td class="px-6 py-4">{ user.UserID }</td>
                                        <td class="px-6 py-4">{ &user.Fullname }</td>
                                        <td class="px-6 py-4">{ &user.Email }</td>
                                        <td class="px-6 py-4">{ &user.Username }</td>
                                        <td class="px-6 py-4">{ if user.IsAdmin == 1 { "Yes" } else { "No" } }</td>
                                    </tr>
                                }
                            }

                        })}
                    </tbody>
                </table>
            </div>
        </>
    }
}